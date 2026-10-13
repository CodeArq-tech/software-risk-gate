# Functional Requirements

## FR-01 — Repository configuration

Every execution must look for .risk-gate.yaml at the checkout root. A missing
file, invalid YAML, unknown level, ambiguous duplicate rule, or unnamed command
must result in FAILED and a non-zero exit code.

## FR-02 — Complete language-independent AST

The analyzer must traverse the entire project within the configured scope and
build a common AST representation. Each common node must preserve, at minimum,
language, path, node type, qualified name when available, byte range, and
children.

The first support matrix is Python, JavaScript, TypeScript, and Elixir. A
missing parser or parse error must appear in the report with path and reason.
A partial tree must never be presented as complete analysis.

## FR-03 — Module mapping

Each riskMap entry must be compared with normalized file paths and discovered
modules. Mapping must identify modules under each path rule, including changes
to tests, configuration, or infrastructure without a corresponding AST module.

Paths are relative to the repository root, use /, and are normalized. A
directory rule ends with /; a file rule can be exact. The root rule ./ matches
the entire repository.

An optional ignorePaths list uses the same normalized path rules. Ignored paths
must be excluded from risk evidence and dependency-graph traversal, while
remaining available for technical parsing diagnostics.

## FR-04 — Pull Request risk

The diff must come from PR commits and include added, modified, renamed, and
removed files. For each changed file, the system calculates the maximum risk
of applicable rules. PR risk is the maximum across changed files.

If no changed file matches the map, risk is normal as a safe fallback. A PR
with no changed files is invalid for analysis and must fail.

### FR-04.1 — Cascade impact and isolated new code

The analyzer must build a reference graph between files and modules identified
by the parser. Impact search considers transitive chains in both directions:
new code using a critical area and existing code that starts using new code are
both impact evidence.

A rule may declare newCodePolicy: ignore. This exception applies only to a new
file with no graph path to any risk rule. Connected new code keeps its
applicable risk. The report must show the files and chain that caused risk;
ignored isolated code must also be explicit.

The report must include a navigable Mermaid diagram showing the changed module,
affected modules, and traversal order. To keep the PR comment readable, the
diagram must be a bounded summary: it prioritizes the highest risk found among
graph impacts, then includes the available cascade chains in risk order,
deduplicates equal chains, and displays the cascade paths from those impacts.
Lower-priority or duplicate chains may be omitted, but the report must state
how many were omitted. The diagram must use a
top-to-bottom layout with wrapped labels, readable spacing, and increased
visual scale. Evidence, AST diagnostics, validations, and approvals should be
collapsed in HTML details blocks so they do not compete with the graph.
The risk legend must be rendered as compact colored text immediately above the
Mermaid block, outside the diagram, using visible colored markers and black
labels; it must not add legend nodes or edges to the Mermaid graph.

## FR-05 — Risk gates

After maximum risk is calculated, the system selects gateRules for that exact
level. A higher-level rule does not automatically inherit lower-level rules.

instantMerge: true means the gate result does not block the merge and the
Action exits successfully for mergeability. The report must still show failed
or incomplete validations, and technical status must not be changed to
SUCCESS.

Without instantMerge, any required validation that does not exit zero or cannot
be executed must fail the gate.

## FR-06 — Group approval

requireApprovalGroup identifies a logical GitHub approval group. The GitHub
adapter must resolve the group to configured users or a team, verify valid PR
approvals, and report who approved and what is missing.

Insufficient permission to verify approval is FAILED, not implicit approval.
The first version must accept org/team and a list of logins.

## FR-07 — Configured validations

validations references named commands in the validations section. Each selected
command must run once with timeout and output captured. All required
validations must succeed for Risk Gate to return success. The result must
include name, identified command, duration, exit code, state, and a safe output
excerpt.

## FR-08 — Idempotent report

Every analysis must generate an idempotent PR comment containing:

1. final risk and ordered level;
2. files and modules that determined maximum risk;
3. path descriptions;
4. a bounded, highest-risk graph impact summary and navigable Mermaid cascade;
5. limitations, missing parsers, and PARTIAL or FAILED states;
6. validation and approval results;
7. Risk Gate identifier and version.

When the stable marker <!-- risk-gate:report --> already exists, the comment
must be updated instead of creating another message.

## FR-09 — Remote dry-run

The CLI must accept --dry-run --repository owner/name --pull-number N --token
TOKEN to analyze a remote PR without changing GitHub. The result must be JSON
and include risk, impact assessment, and the policy selected from gateRules,
including validations, timeout, approval group, instantMerge, and whether a
failure would block the merge.

Dry-run does not execute validations, query approvals, or create/update
comments. A valid analysis returns success regardless of whether the planned
policy is blocking; configuration, API, AST, or diff failures return an error.

## FR-10 — AST impact without configuration

The CLI must accept --ast-impact with --repository, --pull-number, --token,
and --root without requiring .risk-gate.yaml. It must query PR files, build
ASTs from the available checkout, identify modules and references, and return
dependency chains connecting each changed file to other modules.

The JSON result must include changed files, language, modules, references,
partial errors, and ordered impacts. It does not calculate risk, select
gateRules, execute validations, or publish comments.

## FR-11 — Configuration initialization

The CLI must accept the init command and create .risk-gate.yaml in the
repository directory. The initial configuration must be valid, contain
comments describing allowed values, and use a permissive policy: path ./, risk
minimal, newCodePolicy include, no validation or approval commands, and
instantMerge true for every level.

The command must accept --root, --output, and --force. Without --force it must
not overwrite an existing file.
