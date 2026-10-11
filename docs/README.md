# Risk Gate

O Risk Gate é uma etapa Rust para GitHub Actions que classifica o risco de um
Pull Request a partir do código alterado, da AST completa do projeto e de um
mapa de risco versionado no próprio repositório.

## Início rápido

1. Crie `.risk-gate.yaml` na raiz seguindo [`specs/configuration.md`](../specs/configuration.md).
2. Configure a Action com permissão para ler o PR e atualizar comentários.
3. Defina comandos de validação reproduzíveis no repositório.
4. Execute localmente em modo offline para conferir o risco e o relatório.
5. Use a execução no PR para validar o caminho GitHub completo.

Exemplo mínimo:

```yaml
version: 1
riskMap:
  - path: src/payment/
    risk: danger
    description: "Pagamento"
  - path: tests/
    risk: minimal
    description: "Testes"
gateRules:
  danger:
    validations: [e2e]
validations:
  e2e:
    command: "./scripts/e2e.sh"
```

O risco final é o maior nível encontrado entre os arquivos do diff. O
comentário do PR explica quais paths e descrições causaram o resultado. Leia as
especificações para a semântica normativa e os limites da primeira versão.

Quando `newCodePolicy: ignore` for usado, o comentário também informa se o
arquivo novo ficou isolado ou se encontrou uma cadeia de imports/requires até
uma área de risco. Código conectado não é ignorado.

## Permissões do workflow

Use o token padrão com estas permissões mínimas no job que executa a Action:

```yaml
permissions:
  contents: read
  pull-requests: read
  issues: write
```

`pull-requests: read` permite consultar arquivos e reviews. A API de comentários
usa `issues: write`; sem ela a análise retorna falha de publicação, nunca uma
aprovação implícita. Para equipes privadas, o token também precisa ter acesso
à organização correspondente.

O modo local não chama GitHub e é adequado para validar a configuração e o
relatório antes de abrir um PR:

```bash
cargo run -- --config .risk-gate.yaml --root . --base "$BASE_SHA" --head "$HEAD_SHA"
```

Para analisar um PR remoto sem efeitos colaterais, use:

    cargo run -- --dry-run --config .risk-gate.yaml --root . \
      --repository CodeArq-tech/software-risk-gate \
      --pull-number 123 --token "$GITHUB_TOKEN"

Esse comando retorna JSON com risk, assessment e policy. O modo é somente
leitura: não executa validações, não consulta reviews e não publica comentário.

Para implementar o produto, siga o [plano de implementação](implementation-plan.md),
que organiza as tarefas por dependência e critério de aceite.

Os comandos e critérios de cobertura/mutation testing estão em
[quality.md](quality.md).
