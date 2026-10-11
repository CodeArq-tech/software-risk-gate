# Plano de implementação do Risk Gate

Este documento transforma as especificações em tarefas executáveis. A ordem
abaixo reduz retrabalho: primeiro são estabilizados os contratos e o domínio;
depois entram os parsers, GitHub e distribuição.

## Estado da implementação

As fases de domínio, configuração, AST, diff, gates, relatório, CLI e Action
estão implementadas no código atual. A execução local validada cobre 39 testes,
95,82% de linhas no workspace e 97,01% de mutação. As caixas abaixo continuam
como backlog rastreável do plano original; a prova final de publicação em um
repositório GitHub real depende de credenciais, repository e PR externos.

## Critério de conclusão do produto

O produto estará pronto para a primeira versão quando conseguir, em uma
execução real de GitHub Actions:

1. ler e validar `.risk-gate.yaml`;
2. montar a AST completa de um projeto com Python, JavaScript, TypeScript e
   Elixir;
3. mapear paths e módulos alterados no diff do PR;
4. calcular e explicar o maior risco;
5. executar gates, validações e aprovação configurados;
6. publicar ou atualizar um único comentário no PR;
7. emitir outputs e exit code coerentes com `instantMerge`;
8. preservar evidências de erros, análise parcial e limitações.

## Fase 0 — Preparação e decisões técnicas

- [ ] **T-001 — Criar o esqueleto Rust.** Inicializar `Cargo.toml`, crate de
  biblioteca, binário CLI, `src/` e estrutura de testes.
  - Aceite: `cargo test` e `cargo fmt --check` executam em um checkout limpo.
- [ ] **T-002 — Definir toolchain e qualidade.** Fixar toolchain Rust, adicionar
  `rustfmt`, `clippy`, dependabot/atualização equivalente e comandos locais
  documentados.
  - Aceite: CI reproduz os mesmos comandos localmente e na Action.
- [ ] **T-003 — Registrar decisões de dependências.** Escolher e documentar
  parser(es) AST, cliente HTTP GitHub, YAML, logging e executor de processos.
  Avaliar suporte de licença, tamanho do binário, manutenção e cross-compile.
  - Aceite: ADR curto em `specs/decisions/` com alternativas rejeitadas.
- [ ] **T-004 — Definir modelo de domínio.** Criar tipos para `RiskLevel`,
  `AnalysisStatus`, paths, módulos, diff, avaliação, gate, validação,
  aprovação e relatório.
  - Aceite: a ordenação `minimal < low < normal < high < danger` é testada e
  não depende de strings espalhadas pelo código.

## Fase 1 — Configuração e contrato público

- [ ] **T-010 — Implementar carregamento do YAML.** Ler o arquivo na raiz ou no
  caminho explicitamente informado e produzir erros com localização no YAML.
- [ ] **T-011 — Implementar validação estrutural.** Validar `version`,
  `riskMap`, `gateRules`, `validations` e `approvalGroups`, rejeitando chaves
  desconhecidas e referências inexistentes.
- [ ] **T-012 — Implementar normalização de paths.** Normalizar separadores,
  prefixos, diretórios e caminhos relativos sem permitir que `..` escape da
  raiz.
- [ ] **T-013 — Implementar compatibilidade do formato.** Decidir se o atalho
  `riskMap: [{ "src/payment/": danger }]` será aceito além do formato canônico
  `path/risk`; se aceito, normalizar ambos para o mesmo modelo e testar.
- [ ] **T-014 — Criar testes de contrato.** Cobrir configuração válida, arquivo
  ausente, YAML inválido, risco inválido, regra ambígua, timeout inválido,
  validação referenciada mas inexistente e grupo não definido.
- [ ] **T-015 — Atualizar exemplos.** Manter exemplo mínimo e exemplo completo
  em `docs/`, sempre sincronizados com `specs/configuration.md`.

## Fase 2 — Descoberta do projeto e AST

- [ ] **T-020 — Implementar walker do projeto.** Percorrer a raiz com exclusões
  explícitas para `.git`, artefatos, dependências vendorizadas e paths definidos
  como ignorados; contabilizar arquivos examinados.
- [ ] **T-021 — Definir interface de parser.** Criar um trait/adaptador que
  converta cada linguagem para a AST comum, preservando arquivo, tipo, nome,
  intervalo e filhos.
- [ ] **T-022 — Implementar parser Python.** Cobrir módulos, funções, classes,
  imports e erros de sintaxe.
- [ ] **T-023 — Implementar parser JavaScript.** Cobrir módulos, funções,
  classes, imports/exports e erros de sintaxe.
- [ ] **T-024 — Implementar parser TypeScript.** Cobrir declarações de tipos,
  interfaces, módulos, funções, classes, imports/exports e erros de sintaxe.
- [ ] **T-025 — Implementar parser Elixir.** Cobrir módulos, funções, aliases,
  imports e macros de forma compatível com a AST comum.
- [ ] **T-026 — Tratar arquivos sem suporte.** Registrar linguagem desconhecida,
  parser ausente e erro de parsing como evidência; não descartá-los.
- [ ] **T-027 — Testar AST com fixtures.** Criar fixtures pequenas e projetos
  mistos, verificando contagem, nomes qualificados, ranges e nós aninhados.
- [ ] **T-028 — Testar AST completa.** Provar que arquivos não alterados também
  são analisados e que o resultado não depende da ordem do filesystem.

## Fase 3 — Diff, módulos e cálculo de risco

- [ ] **T-030 — Implementar `DiffProvider` local.** Obter arquivos adicionados,
  modificados, removidos e renomeados entre base e head do PR.
- [ ] **T-031 — Implementar `DiffProvider` GitHub.** Usar a API apenas quando
  necessário, paginar resultados e preservar SHA/base/head usados na análise.
- [ ] **T-032 — Implementar matching do `riskMap`.** Casar paths relativos e
  módulos da AST, mantendo todas as regras aplicáveis e suas descrições.
- [ ] **T-033 — Implementar risco máximo.** Aplicar a precedência de
  especificidade e depois o maior `RiskLevel`; usar `normal` para arquivo
  alterado sem correspondência.
- [ ] **T-034 — Tratar renomeações e remoções.** Avaliar o path antigo e o novo
  quando disponíveis; remoções sem AST devem continuar sendo risco analisável.
- [ ] **T-035 — Definir PR sem alterações.** Retornar erro explícito para diff
  vazio, sem classificá-lo como `minimal`.
- [ ] **T-036 — Testar matriz de risco.** Cobrir múltiplas regras, empate,
  subpaths, arquivo novo, arquivo removido, rename, path desconhecido e projeto
  misto.

## Fase 4 — Motor de gates e execução de validações

- [ ] **T-040 — Implementar seleção de `gateRules`.** Selecionar a regra exata
  do risco final e produzir um plano determinístico de execução.
- [ ] **T-041 — Implementar executor seguro.** Executar comandos configurados
  com diretório, ambiente, timeout, captura de stdout/stderr e limite de saída.
  Não interpolar dados do evento no comando.
- [ ] **T-042 — Implementar estados de validação.** Diferenciar sucesso, exit
  code diferente de zero, timeout, comando inexistente, interrupção e execução
  não iniciada.
- [ ] **T-043 — Implementar aprovação por grupo.** Resolver `team: org/team` e
  `users: [...]`, consultar reviews do PR, validar estado/aprovador e registrar
  aprovações faltantes.
- [ ] **T-044 — Implementar `instantMerge`.** Quando verdadeiro, manter todas
  as falhas no relatório, mas não bloquear o merge nem retornar falha de
  mergeability. Não converter o status técnico para `SUCCESS`.
- [ ] **T-045 — Definir exit codes.** Documentar códigos para configuração,
  análise, validação, aprovação, GitHub e erro interno; manter códigos estáveis.
- [ ] **T-046 — Testar gates sem rede.** Usar executores e GitHub fakes para
  provar que toda validação exigida precisa ter sucesso quando o gate deve
  bloquear.

## Fase 5 — Relatório e integração GitHub

- [ ] **T-050 — Criar modelo do relatório.** Incluir risco, evidências, paths,
  módulos, descrições, gates, validações, aprovações, contagens AST e
  limitações.
- [ ] **T-051 — Implementar renderer Markdown seguro.** Escapar conteúdo vindo
  do código, configuração, comandos e GitHub; manter formato legível e estável.
- [ ] **T-052 — Implementar comentário idempotente.** Procurar
  `<!-- risk-gate:report -->`, atualizar o comentário existente ou criar um
  novo, sem duplicar mensagens.
- [ ] **T-053 — Implementar outputs da Action.** Publicar `risk`, `status`,
  `merge_blocked` e identificador/URL do comentário.
- [ ] **T-054 — Tratar falhas de permissão.** Mostrar autenticação, escopo e
  endpoint que falharam; não transformar erro de comentário ou diff em risco
  baixo.
- [ ] **T-055 — Testar relatório.** Usar snapshots para cenários de sucesso,
  risco alto, validação falha, análise parcial, aprovação ausente e
  `instantMerge`.

## Fase 6 — GitHub Action e execução real

- [ ] **T-060 — Criar `action.yml`.** Declarar inputs, outputs, permissões,
  runtime e versão do binário.
- [ ] **T-061 — Criar workflow de CI do projeto.** Rodar fmt, clippy, testes,
  segurança de dependências e build em Linux.
- [ ] **T-062 — Criar workflow de validação do próprio Risk Gate.** Executar a
  Action contra fixtures/projetos de teste com PRs controlados.
- [ ] **T-063 — Definir empacotamento.** Escolher binário pré-compilado,
  container ou compilação no runner; cobrir Linux x86_64 e arquitetura adotada
  pelo projeto.
- [ ] **T-064 — Validar permissões mínimas.** Testar token com leitura do PR e
  atualização de comentário, registrando a configuração recomendada.
- [ ] **T-065 — Fazer teste E2E GitHub.** Abrir/usar PR de teste, verificar
  comentário atualizado, outputs, status da Action e comportamento de bloqueio.
  Registrar o SHA testado e separar resultado local de resultado remoto.

## Fase 7 — Segurança, performance e observabilidade

- [ ] **T-070 — Auditar execução de comandos.** Revisar shell, ambiente, timeout,
  tamanho de saída, sinais, diretório de trabalho e exposição de tokens.
- [ ] **T-071 — Auditar Markdown e API.** Escapar conteúdo, validar URLs/IDs e
  evitar log de secrets, cabeçalhos ou payloads sensíveis.
- [ ] **T-072 — Limitar recursos.** Definir limites para arquivos, nós AST,
  concorrência, memória, duração total e tamanho do comentário.
- [ ] **T-073 — Medir performance.** Criar benchmark de projeto pequeno, médio e
  grande; medir walker, parse, matching e publicação separadamente.
- [ ] **T-074 — Implementar logs estruturados.** Permitir modo normal e debug,
  com correlação por execução e sem vazar credenciais.
- [ ] **T-075 — Testar falhas e interrupções.** Cobrir cancelamento, timeout,
  rede indisponível, API paginada, parser quebrado e disco sem espaço.

## Fase 8 — Documentação, release e manutenção

- [ ] **T-080 — Documentar instalação e uso local.** Incluir modo offline,
  exemplos, variáveis de ambiente e interpretação dos exit codes.
- [ ] **T-081 — Documentar permissões GitHub.** Explicar `permissions`, token,
  equipes, usuários e limitações de forks.
- [ ] **T-082 — Criar changelog e versionamento.** Versionar contrato do YAML e
  indicar mudanças incompatíveis.
- [ ] **T-083 — Automatizar release.** Gerar artefato verificável, checksum,
  tag e atualização controlada da Action.
- [ ] **T-084 — Criar checklist de release.** Exigir fmt, clippy, testes,
  fixtures dos quatro parsers, teste E2E, inspeção de permissões e revisão do
  relatório antes de publicar.
- [ ] **T-085 — Documentar limitações conhecidas.** Registrar sintaxes não
  cobertas, parsing parcial, limites de monorepo e diferenças entre GitHub
  Enterprise e GitHub.com.

## Ordem recomendada de execução

As tarefas podem ser divididas entre agentes, mas respeitando estas
dependências:

```text
T-001..004
    ├── T-010..015
    ├── T-020..028
    └── T-030..036 (depende dos modelos e configuração)
             └── T-040..046
                      ├── T-050..055
                      └── T-060..065
                               └── T-070..085
```

Cada agente deve entregar código, testes e documentação da sua tarefa. Antes
de integrar uma fase, o agente responsável deve executar os critérios de aceite
da fase e registrar limitações em vez de marcar uma etapa parcialmente
executada como concluída.

## Backlog explicitamente posterior

Não bloquear a primeira versão com estes itens: mais linguagens, regras de
risco por linha/símbolo, herança entre `gateRules`, dashboard externo,
comentários em múltiplos idiomas, cache distribuído e análise semântica que
exija executar o projeto. Esses itens só entram depois que o contrato inicial,
os quatro parsers e a execução real no GitHub estiverem comprovados.
