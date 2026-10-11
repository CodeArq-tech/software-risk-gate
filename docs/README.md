# Risk Gate Documentation

Risk Gate is a Rust step for GitHub Actions that classifies Pull Request risk
from changed code, the complete project AST, and a versioned repository risk
map.

## Quick start

1. Run risk-gate init to create .risk-gate.yaml.
2. Review the generated permissive configuration.
3. Configure the Action with permission to read the PR and update comments.
4. Define reproducible validation commands in the repository.
5. Run the local offline mode before opening a PR.
6. Validate the complete GitHub flow on a test PR.

Minimal example:

~~~yaml
version: 1
riskMap:
  - path: src/payment/
    risk: danger
    description: "Payment processing"
  - path: tests/
    risk: minimal
    description: "Automated tests"
gateRules:
  danger:
    validations: [e2e]
validations:
  e2e:
    command: "./scripts/e2e.sh"
~~~

The final risk is the highest level found among files in the diff. The PR
comment explains which paths and descriptions caused the result. When
newCodePolicy is ignore, the report states whether a new file was isolated or
connected to a risk area.

## Workflow permissions

Use the default token with these minimum job permissions:

~~~yaml
permissions:
  contents: read
  pull-requests: read
  issues: write
~~~

pull-requests: read allows file and review queries. Comment creation and
updates use issues: write. Without it, publication fails; it must never be
treated as implicit approval.

## Local modes

Offline local analysis:

~~~bash
cargo run -- --config .risk-gate.yaml --root . --base "$BASE_SHA" --head "$HEAD_SHA"
~~~

Remote read-only policy preview:

~~~bash
cargo run -- --dry-run --config .risk-gate.yaml --root . +  --repository owner/repository --pull-number 123 --token "$GITHUB_TOKEN"
~~~

AST and dependency analysis without configuration:

~~~bash
cargo run -- --ast-impact --root /path/to/repository +  --repository owner/repository --pull-number 123 --token "$GITHUB_TOKEN"
~~~

The implementation plan is in implementation-plan.md. Quality commands and
coverage/mutation requirements are in quality.md.
