# Contrato de `.risk-gate.yaml`

O formato é YAML. Chaves desconhecidas devem ser rejeitadas na versão inicial,
para evitar falsa sensação de configuração aplicada.

```yaml
version: 1

riskMap:
  - path: src/payment/
    risk: danger
    description: "Processamento de pagamentos e autorização financeira"
  - path: tests/
    risk: minimal
    description: "Testes automatizados"
  - path: infra/database/
    risk: danger
    description: "Migrações, acesso e infraestrutura do banco"
  - path: src/instructions/
    risk: normal
    description: "Instruções usadas pelo runtime"
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
```

## Forma mínima

`riskMap` é obrigatório e não pode ser vazio. Cada item contém `path`, `risk`
e pode conter `description` e `newCodePolicy`. Os níveis válidos são exatamente
`minimal`, `low`, `normal`, `high` e `danger`.

`newCodePolicy` controla código novo em uma regra: `include` (padrão) mantém o
risco da regra mesmo quando o arquivo novo é isolado; `ignore` reduz para
`minimal` somente quando não existe ligação de dependência com outro arquivo
que pertença a qualquer regra. Se houver uma ligação direta ou transitiva, o
risco da regra é aplicado e a cadeia aparece no relatório do PR.

`gateRules` pode conter qualquer nível. Os campos válidos de uma regra são:

- `instantMerge: boolean`;
- `requireApprovalGroup: string`;
- `validations: [string, ...]`.

`validations` é um mapa de nomes para objetos com `command` e opcionalmente
`timeoutSeconds`. Por compatibilidade futura, a implementação pode aceitar uma
string como atalho, mas deve normalizá-la para o mesmo modelo e documentar o
atalho em testes.

`approvalGroups` resolve nomes locais para uma equipe (`team: org/team`) ou
logins explícitos (`users: [alice, bob]`). `requireApprovalGroup` sem uma
entrada correspondente deve falhar na validação da configuração.

`limits` é opcional e possui valores seguros por padrão. `maxFiles` limita os
arquivos percorridos, `maxAstNodes` limita a árvore agregada e
`maxCommentBytes` limita o comentário publicado no PR. Todos devem ser
inteiros positivos; exceder um limite falha a análise de forma explícita.

## Semântica de paths

Paths são relativos à raiz, normalizados e comparados com separadores `/`.
Regras mais específicas vencem regras menos específicas quando apontam para o
mesmo arquivo; depois disso, o risco máximo entre todas as regras aplicáveis é
usado. Assim, uma área `danger` nunca é rebaixada por uma regra de diretório
mais ampla.

Exemplo: `src/payment/checkout.py` casa com `src/` e `src/payment/`; o maior
nível aplicável é usado e ambas as descrições podem ser incluídas no relatório.
