# AGENTS.md

## Objetivo do repositório

Este repositório contém o `risk-gate`, um software escrito em Rust e executado
como etapa de GitHub Actions. A finalidade do software é analisar o risco de um
Pull Request, aplicar gates de validação compatíveis com o risco encontrado e
publicar um relatório auditável no PR.

O produto não é um linter genérico nem um serviço externo. A configuração é
versionada junto com o projeto analisado no arquivo `.risk-gate.yaml`.

## Regras obrigatórias para agentes

1. Implemente o produto em Rust. Não introduza uma implementação paralela em
   Python, Node.js ou Elixir. Ferramentas auxiliares podem usar outra linguagem
   somente quando isso for explicitamente necessário e permanecerem fora do
   runtime principal.
2. Preserve uma arquitetura de biblioteca + CLI. A biblioteca deve concentrar
   parsing, descoberta de linguagens, AST, mapeamento, cálculo de risco,
   regras e relatório; a CLI deve adaptar isso ao GitHub Actions.
3. Toda decisão de risco deve ser determinística, explicável e testável sem
   acesso à rede. O acesso ao GitHub deve ser uma borda substituível.
4. O arquivo `.risk-gate.yaml` é parte do contrato público. Mudanças
   incompatíveis exigem atualização da especificação, dos exemplos e dos
   testes de contrato.
5. Nunca trate uma validação não executada, interrompida, com comando ausente
   ou com saída inconclusiva como sucesso.
6. Não publique, aprove, faça merge ou altere configurações do GitHub fora do
   escopo explícito da execução. O comentário do relatório deve ser idempotente
   e atualizado por uma marca estável.
7. Não esconda falhas por meio de `instantMerge`. Essa opção somente impede que
   o gate bloqueie o merge; ela não converte validações falhas em sucesso nem
   altera o relatório.
8. Comandos de validação são configuração não confiável do repositório. Execute
   com diretório de trabalho, ambiente e timeout explícitos, registre o resultado
   e evite interpolação insegura de valores vindos de eventos do GitHub.
9. Separe sempre os estados `SUCCESS`, `PARTIAL` e `FAILED`. Um comentário,
   uma árvore AST parcial ou uma permissão ausente não prova sucesso completo.
10. Antes de declarar uma integração concluída, prove o caminho real: evento do
    PR, leitura da configuração, diff, análise, execução dos gates, comentário
    e código de saída da Action.

## Escopo inicial de linguagens

O modelo de AST deve ser independente de linguagem. A primeira entrega deve
suportar Python, JavaScript, TypeScript e Elixir, usando parsers concretos
atrás de uma interface comum. A árvore deve cobrir todo o projeto incluído no
escopo da execução, não apenas arquivos alterados. Arquivos sem parser devem
ser reportados como não analisados; nunca devem desaparecer silenciosamente.

## Modelo de risco

Os níveis ordenados, do menor para o maior, são:

`minimal < low < normal < high < danger`

O risco do PR é o maior risco aplicável aos módulos alcançados por arquivos
alterados no diff. Alterações sem correspondência no `riskMap` usam o risco
`normal` por padrão, para que uma área nova não passe despercebida. O fallback
deve ser configurável somente quando isso estiver especificado e validado.

## Fluxo esperado

`configuração -> árvore AST completa -> módulos/path map -> diff do PR -> risco
máximo -> gateRules -> validações -> comentário no PR -> exit code`

Cada etapa deve expor dados suficientes para diagnóstico e testes. Consulte os
documentos em `specs/` antes de alterar contratos e `docs/` para exemplos de
operação.

## Qualidade mínima

Antes de concluir uma mudança, execute os checks Rust disponíveis (`cargo fmt
--check`, `cargo clippy --all-targets --all-features -- -D warnings` e `cargo
test`, quando o projeto já os tiver). Adicione testes unitários para regras de
risco e testes de integração para configuração, diff, gates e relatório.

Toda alteração deve atualizar a documentação afetada. Não invente uma
capacidade do GitHub Actions que não esteja coberta por teste ou marcada como
limitação.
