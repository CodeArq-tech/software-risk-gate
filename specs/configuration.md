# .risk-gate.yaml Contract

The format is YAML. Unknown keys must be rejected in the initial version to
avoid creating a false impression that a setting was applied.

~~~yaml
version: 1

riskMap:
  - path: src/payment/
    risk: danger
    description: "Payment processing and financial authorization"
  - path: tests/
    risk: minimal
    description: "Automated tests"
  - path: infra/database/
    risk: danger
    description: "Database migrations, access, and infrastructure"
  - path: src/instructions/
    risk: normal
    description: "Instructions used by the runtime"
    newCodePolicy: include

gateRules:
  normal:
    instantMerge: true
  high:
    requireApprovalGroup: architects
  danger:
    validations:
      - e2e
      - mutationTesting

validations:
  e2e:
    command: "./scripts/test-e2e.sh"
    timeoutSeconds: 1800
  mutationTesting:
    command: "mix mutation"
    timeoutSeconds: 3600

approvalGroups:
  architects:
    team: platform/architects

limits:
  maxFiles: 10000
  maxAstNodes: 1000000
  maxCommentBytes: 60000
~~~

## Minimal form

The risk-gate init command generates a permissive form for editing. The path:
./ rule represents the entire repository and can be replaced with more
specific rules. The generated configuration keeps instantMerge: true for every
level and defines no validations or approval groups.

riskMap is required and cannot be empty. Each item contains path and risk and
may contain description and newCodePolicy. Valid levels are exactly minimal,
low, normal, high, and danger.

newCodePolicy controls new code under a rule: include, the default, preserves
the rule risk even when a new file is isolated; ignore reduces it to minimal
only when there is no dependency link to another file belonging to any rule.
When a direct or transitive link exists, the rule risk applies and the chain
appears in the PR report.

ignorePaths is an optional list of repository-relative path prefixes excluded
from risk evidence and dependency-graph traversal. It is useful for generated
files or test trees that should not change merge risk. Directory entries end in
`/`; paths are normalized and must not escape the repository. Ignored files
may still be parsed and reported as technical AST input.

gateRules may contain any risk level. Valid fields are:

- instantMerge: boolean;
- requireApprovalGroup: string;
- validations: a list of names.

validations maps names to objects with command and optional timeoutSeconds.
For compatibility, a string shorthand may also be accepted and normalized to
the same model.

approvalGroups resolves local names to a team, such as org/team, or explicit
logins, such as users: [alice, bob]. A requireApprovalGroup entry without a
matching group must fail configuration validation.

limits is optional and has safe defaults. maxFiles limits traversed files,
maxAstNodes limits the aggregate tree, and maxCommentBytes limits the PR
comment. All values must be positive integers; exceeding a limit must fail the
analysis explicitly.

## Path semantics

Paths are relative to the repository root, normalized, and compared with /
separators. More specific rules win when they point to the same file; the
maximum risk among all applicable rules is then used. A danger area can never
be downgraded by a broader directory rule.

For example, src/payment/checkout.py matches src/ and src/payment/; the
highest applicable level is used and both descriptions may be included in the
report.
