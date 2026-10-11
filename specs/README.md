# Especificações do Risk Gate

Esta pasta define os contratos do produto. Os documentos são normativos para
implementação e testes; exemplos em `docs/` são ilustrativos, mas devem seguir
estes contratos.

- [Requisitos e fluxo](requirements.md)
- [Contrato de configuração](configuration.md)
- [Arquitetura](architecture.md)
- [GitHub Action e estados](github-action.md)

O trabalho de implementação está decomposto em tarefas no
[`docs/implementation-plan.md`](../docs/implementation-plan.md).

## Termos

- **arquivo alterado**: caminho retornado pelo diff do Pull Request;
- **módulo**: unidade semântica descoberta na AST, associada a um arquivo e a
  um caminho lógico;
- **path rule**: entrada do `riskMap` que relaciona um prefixo de caminho a um
  nível e, opcionalmente, a uma descrição;
- **gate**: comportamento aplicado após o risco máximo ser calculado;
- **validação**: comando nomeado que precisa terminar com sucesso quando for
  exigido.

As palavras “deve” e “não pode” são obrigatórias. “Pode” indica extensão
compatível, sem alterar o comportamento definido.
