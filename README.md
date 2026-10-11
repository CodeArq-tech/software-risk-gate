# Risk Gate

Rust CLI and GitHub Action for classifying Pull Request risk from a versioned
`.risk-gate.yaml`, a complete project AST and the files changed by the PR.

The first parser matrix supports Python, JavaScript, TypeScript and Elixir.
The result is one of `minimal`, `low`, `normal`, `high` or `danger`, followed
by configured approvals and validations and an idempotent PR report.

The report also traces import/require relationships to show cascade impact. A
rule may set `newCodePolicy: ignore` when isolated new files should not inherit
the path risk; connected code always remains in the risk calculation.

## Local analysis

```bash
cargo run -- --config .risk-gate.yaml --root . --base "$BASE_SHA" --head "$HEAD_SHA"
```

For the public configuration contract, see
[`specs/configuration.md`](specs/configuration.md). For implementation order,
see [`docs/implementation-plan.md`](docs/implementation-plan.md). Quality gates
and the 95% coverage/mutation requirements are in
[`docs/quality.md`](docs/quality.md).
