# GitHub Action Contract

The Action must accept, at minimum, a configuration file path, a diff mode
(local or GitHub), and a default timeout. The token and PR context must come
from official GitHub Actions inputs or environment variables and must never be
written to the report.

## Outputs

The Action must publish outputs consumable by later jobs:

- risk: final risk level;
- status: technical analysis status;
- merge_blocked: whether configured gates block mergeability;
- report_url or a comment identifier, when available.

The process may exit with code zero only when configuration is valid, analysis
is non-fatal, all required validations succeed, required approvals are
satisfied or instantMerge is true, and the comment is published, except in
offline test mode. instantMerge changes only merge_blocked and the blocking
exit code; it does not turn a technical FAILED status into a correct analysis.

Authentication, permission, comment, or diff failures must be visible and must
not be presented as minimal risk.

## Expected permissions

Action documentation must declare the permissions required to read Pull
Requests and publish or update comments. When a permission is unavailable, the
system must explain it in the report without attempting to bypass repository
policy.

The initial Action downloads the Linux binary matching the version supplied in
version and runs it with the event context. Releases must publish the asset
risk-gate-x86_64-unknown-linux-gnu before another repository uses that version.

## CLI dry-run mode

The binary must accept --dry-run with --repository, --pull-number, and --token.
This mode queries PR files, runs configuration, AST, diff, and graph analysis,
and returns JSON containing risk, assessment, and the selected policy.

Dry-run must not publish or update comments, query reviews or team members, or
execute commands declared in validations. Configuration or query errors return
a non-zero code; a valid analysis returns zero even when the planned policy
would block the merge.

## CLI AST-impact mode

The binary must accept --ast-impact with --repository, --pull-number, --token,
and --root without requiring .risk-gate.yaml. It must query PR files, build
ASTs from files available in the checkout, identify modules and references, and
return dependency chains connecting changed files to other modules.

The JSON result must include changed files, language, modules, references,
partial errors, and ordered impacts. This mode does not calculate risk, select
gateRules, execute validations, or publish comments.
