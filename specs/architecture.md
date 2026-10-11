# Arquitetura de referência

O binário deve ser pequeno e adequado a uma Action, mas a lógica deve ser
testável como biblioteca.

```text
CLI
 ├─ input: workspace, event/PR, token e opções
 ├─ ConfigLoader
 ├─ ProjectWalker -> LanguageParser -> CommonAst
 ├─ DiffProvider
 ├─ RiskMapper -> RiskAssessment
 ├─ GateEngine -> ValidationRunner / ApprovalChecker
 └─ ReportRenderer -> GitHubCommenter + ActionOutputs
```

## Fronteiras

- `ConfigLoader`: lê e valida `.risk-gate.yaml`; não chama GitHub.
- `LanguageParser`: um adaptador por linguagem; retorna erros estruturados.
- `DiffProvider`: abstrai `git diff` local e API/evento do GitHub.
- `RiskMapper`: recebe AST, paths e diff; não executa comandos.
- `ValidationRunner`: executa apenas validações selecionadas e produz
  resultados; não decide risco.
- `ApprovalChecker` e `GitHubCommenter`: usam uma interface GitHub mockável.
- `GateEngine`: combina risco, aprovações e validações em um resultado
  determinístico.

O domínio não deve depender de variáveis de ambiente do GitHub. A adaptação de
`GITHUB_EVENT_PATH`, `GITHUB_REPOSITORY`, `GITHUB_TOKEN` e outputs deve ficar na
CLI.

## Segurança e operação

O token deve receber somente as permissões documentadas pela Action. Saídas de
comandos e conteúdo do PR devem ser escapados antes de virar Markdown. Comandos
devem rodar sem `shell -c` quando possível; se o shell for necessário para
suportar comandos configuráveis, o executor precisa registrar a decisão,
aplicar timeout e não concatenar dados do evento.

A árvore completa pode ser grande. O sistema pode cachear parsers e índices,
mas o cache não pode alterar o resultado nem ocultar erro de parsing. O
relatório deve informar contagens de arquivos, nós e parsers.
