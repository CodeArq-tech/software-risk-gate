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

Para criar uma configuração inicial permissiva, com comentários explicando
as opções disponíveis:

```bash
cargo run -- init
```

O comando cria `.risk-gate.yaml`, usa `path: ./` com risco `minimal` e deixa
`instantMerge: true` em todos os níveis. Use `--force` para substituir um
arquivo existente; depois ajuste o mapa, gates, validações e aprovações ao
risco real do repositório.

```bash
cargo run -- --config .risk-gate.yaml --root . --base "$BASE_SHA" --head "$HEAD_SHA"
```

Para consultar um PR existente sem efeitos colaterais, use:

    cargo run -- --dry-run --config .risk-gate.yaml --root . \
      --repository CodeArq-tech/software-risk-gate \
      --pull-number 123 --token "$GITHUB_TOKEN"

O dry-run consulta os arquivos do PR e retorna JSON com o risco, a análise de
impacto e a política de gateRules selecionada pela Action. Ele não executa
validações, não consulta aprovações e não publica comentários.

Quando o repositório ainda não possui configuração, use o modo ast-impact
para analisar somente AST e dependências:

    cargo run -- --ast-impact --root /path/to/condominio \
      --repository Lar21/condominio --pull-number 31554 \
      --token "$GITHUB_TOKEN"

Esse modo não exige .risk-gate.yaml e retorna os arquivos alterados, módulos
identificados, referências e as cadeias de módulos afetados.

For the public configuration contract, see
[`specs/configuration.md`](specs/configuration.md). For implementation order,
see [`docs/implementation-plan.md`](docs/implementation-plan.md). Quality gates
and the 95% coverage/mutation requirements are in
[`docs/quality.md`](docs/quality.md).
