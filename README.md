# Risk Gate

Risk Gate is a Rust CLI and GitHub Action for estimating Pull Request merge
risk. It combines a repository-owned risk map, a complete project AST, the PR
diff, a dependency graph, configurable validations, approval groups, and an
idempotent report comment.

It does not replace technical review. It makes likely impact explicit and
applies a policy proportional to the affected area's criticality.

## How the analysis works

For a PR execution, Risk Gate:

1. Loads and validates .risk-gate.yaml.
2. Queries GitHub for changed, renamed, and removed PR files.
3. Walks the repository and builds the complete supported AST.
4. Extracts modules and references from Python, JavaScript, TypeScript, and
   Elixir.
5. Maps changed files and modules to riskMap rules.
6. Traverses the dependency graph for direct and transitive impact.
7. Selects the highest applicable risk.
8. Runs validations and checks configured approvals for that risk.
9. Creates or updates one compact PR comment with evidence and a Mermaid
   cascade graph.

The graph can raise the risk of a change outside a critical path when the
change depends on, or is depended on by, critical code. The report graph keeps
only the highest propagated risk chains, removes duplicates, and bounds the
number of nodes so the rendered Mermaid diagram remains readable and
navigable in the PR. It is rendered top-to-bottom, uses wrapped path labels and
larger spacing, and keeps evidence and gate details collapsed so the graph is
the visual focus. It reports how many lower-priority chains were omitted.
A new file can use
newCodePolicy: ignore when it is isolated; any direct or transitive connection
keeps it in the risk calculation.

## AST and language support

Risk Gate uses Tree-sitter grammars selected by file extension:

- .py: Python;
- .js, .jsx, .mjs, .cjs: JavaScript;
- .ts, .tsx: TypeScript;
- .ex, .exs: Elixir.

Each file becomes a common ParsedFile/AstNode model containing language, path,
node kind, byte range, optional name, child nodes, references, and parse
errors. The current implementation rebuilds the AST for the current checkout;
it does not yet calculate a structural AST diff. Changed paths come from Git
or the GitHub API and are mapped onto the current AST.

Python imports and relative imports are resolved to likely .py and
__init__.py paths. JavaScript and TypeScript imports, exports, and require
calls are resolved to likely .js, .ts, and index paths. Elixir alias, import,
require, and use forms are normalized to paths such as
lib/my_app/accounts/user.ex.

The analysis is structural rather than complete semantic analysis. Dynamic
imports, reflection, runtime-generated paths, complex TypeScript aliases,
bundler-only resolution, configuration-driven calls, advanced Elixir
metaprogramming, and dependencies outside the repository may not be detected.

## GitHub Action

The consumer repository needs .risk-gate.yaml and a workflow that can read the
PR and write its report comment:

~~~yaml
name: Risk Gate

on:
  pull_request:
    types: [opened, synchronize, reopened, ready_for_review]

permissions:
  contents: read
  pull-requests: read
  issues: write

jobs:
  risk-gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: CodeArq-tech/software-risk-gate@v0.1.0
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
~~~

The Action downloads the Linux x86_64 release binary and verifies its
SHA-256 checksum. The version input selects another published release:

~~~yaml
- uses: CodeArq-tech/software-risk-gate@v0.1.0
  with:
    config: .risk-gate.yaml
    token: ${{ secrets.GITHUB_TOKEN }}
    version: v0.1.0
~~~

Action outputs are risk, status, merge_blocked, and report_url.

## .risk-gate.yaml

The configuration has an explicit version and rejects unknown keys:

~~~yaml
version: 1

riskMap:
  - path: src/payment/
    risk: danger
    description: "Payment processing and financial authorization"
    newCodePolicy: include
  - path: tests/
    risk: minimal
    description: "Automated tests"
  - path: infra/database/
    risk: danger
    description: "Database migrations and access"
  - path: src/instructions/
    risk: normal
    description: "Runtime instructions"
    newCodePolicy: ignore

ignorePaths:
  - generated/
  - tests/                         # optional: exclude a path from risk and graph analysis

gateRules:
  minimal:
    instantMerge: true
  normal:
    instantMerge: false
  high:
    requireApprovalGroup: architects
  danger:
    validations: [e2e, mutationTesting]

validations:
  e2e:
    command: "./scripts/test-e2e.sh"
    timeoutSeconds: 1800
  mutationTesting:
    command: "cargo mutants --baseline run"
    timeoutSeconds: 3600

approvalGroups:
  architects:
    team: platform/architects
    users: [alice, bob]

limits:
  maxFiles: 10000
  maxAstNodes: 1000000
  maxCommentBytes: 60000
~~~

Risk levels are ordered:

minimal < low < normal < high < danger

Paths are repository-relative. path: ./ matches the entire repository. More
specific rules may coexist with broad rules; the highest applicable risk wins.
Descriptions are included in the PR report.

newCodePolicy include keeps a rule's risk for isolated new code. ignore reduces
isolated new code to minimal only when there is no graph path to any mapped
area. Connected code keeps its applicable risk and its chain appears in the
report.

gateRules may define instantMerge, requireApprovalGroup, and validations.
instantMerge: true reports failures without blocking mergeability; it does not
turn technical failure into SUCCESS. With instantMerge: false, every selected
validation must return zero and required approval must be present.

validations maps names to shell commands and optional timeouts.
approvalGroups maps names to GitHub teams or explicit users. limits bounds
files, AST nodes, and published comment size.

## Initialize configuration

Create a valid permissive configuration with:

~~~bash
cargo run -- init
~~~

The generated file uses path: ./, minimal, newCodePolicy: include, and
instantMerge: true for every level. Comments explain the available values.

Options:

~~~bash
risk-gate init --root /path/to/repository
risk-gate init --output config/risk-gate.yaml
risk-gate init --force
~~~

## CLI modes

Offline local Git range:

~~~bash
cargo run -- --config .risk-gate.yaml --root . +  --base "$BASE_SHA" --head "$HEAD_SHA"
~~~

Remote read-only policy preview:

~~~bash
cargo run -- --dry-run --config .risk-gate.yaml --root . +  --repository owner/repository --pull-number 123 --token "$GITHUB_TOKEN"
~~~

dry-run returns JSON with risk, evidence, changed files, and the policy the
Action would execute. It does not run validations, check approvals, or publish
comments.

AST and dependency analysis without configuration:

~~~bash
cargo run -- --ast-impact --root /path/to/repository +  --repository owner/repository --pull-number 123 --token "$GITHUB_TOKEN"
~~~

ast-impact returns changed files, languages, modules, references, and ordered
dependency chains. It does not calculate risk, execute gates, or publish
comments.

## Development and quality

~~~bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo llvm-cov --workspace --all-features --fail-under-lines 95.01
cargo mutants --baseline run --copy-target true --minimum-test-timeout 60 --no-shuffle -j 4
bash scripts/check-mutation-score.sh
~~~

Pull Request CI requires formatting, Clippy, all tests, line coverage strictly
above 95%, mutation score strictly above 95%, and a branch rebased on the
current master.

See specs/configuration.md for the normative configuration contract,
docs/implementation-plan.md for implementation work, and docs/quality.md for
quality policy.
