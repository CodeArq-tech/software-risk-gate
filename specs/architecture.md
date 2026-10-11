# Reference Architecture

The binary must be small and suitable for an Action, while its logic must be
testable as a library.

~~~text
CLI
 ├─ input: workspace, event/PR, token, and options
 ├─ ConfigLoader
 ├─ DiffProvider
 ├─ ProjectWalker + LanguageParsers
 ├─ RiskMapper + DependencyGraph
 ├─ GateEngine + ValidationRunner
 ├─ GithubAdapter
 └─ ReportRenderer + outputs
~~~

## Component boundaries

- ConfigLoader reads and validates .risk-gate.yaml and does not call GitHub.
- LanguageParser has one adapter per language and returns structured errors.
- RiskMapper receives ASTs, paths, and diff data and does not execute commands.
- ValidationRunner executes only selected validations and produces results; it
  does not decide risk.
- ApprovalChecker and GitHubCommenter use a mockable GitHub interface.
- GateEngine combines risk, approvals, and validations into a deterministic
  result.

The domain must not depend on GitHub environment variables. Adaptation of
GITHUB_EVENT_PATH, GITHUB_REPOSITORY, GITHUB_TOKEN, and outputs belongs in the
CLI and Action boundary.

## Security and operations

The token must receive only the permissions documented by the Action. Command
output and PR content must be escaped before becoming Markdown. Commands should
avoid shell -c when possible; when a shell is required for configurable
commands, the executor must record the decision, apply a timeout, and never
concatenate event data.

The complete tree can be large. The system may cache parsers and indexes, but
the cache must not change results or hide parsing errors. Reports must include
file, node, and parser counts.
