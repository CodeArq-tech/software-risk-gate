# Contrato da GitHub Action

## Entradas

A Action deve aceitar, no mínimo, caminho do arquivo de configuração, modo de
obter o diff (`local` ou `github`) e timeout padrão. O token e o contexto do PR
devem vir das entradas/ambiente oficial do GitHub Actions, sem serem gravados
no relatório.

## Saídas

Devem ser publicados outputs consumíveis por jobs posteriores:

- `risk`: nível final;
- `status`: `SUCCESS`, `PARTIAL` ou `FAILED`;
- `merge_blocked`: `true` ou `false`;
- `report_url` ou identificador do comentário, quando disponível.

O processo deve sair com código zero somente quando todas as condições abaixo
forem verdadeiras: configuração válida, análise não fatal, todas as
validações exigidas com sucesso, aprovações exigidas satisfeitas ou
`instantMerge: true`, e comentário publicado (exceto em modo offline de
teste). `instantMerge` altera apenas `merge_blocked`/exit code do bloqueio; não
transforma `FAILED` técnico em análise correta.

Falha de autenticação, permissão, comentário ou obtenção do diff deve ser
visível e não pode ser apresentada como risco `minimal`.

## Permissões esperadas

A documentação da Action deve declarar as permissões necessárias para ler
Pull Requests e publicar/atualizar comentários. O sistema deve explicar no
relatório quando a permissão não está disponível, sem tentar contornar a
política do repositório.

O manifesto da primeira Action baixa o binário Linux correspondente à versão
informada em `version` e executa-o com o contexto do evento. Releases devem
publicar o asset `risk-gate-x86_64-unknown-linux-gnu` antes que essa versão seja
usada por outros repositórios.
