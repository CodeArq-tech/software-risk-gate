# Requisitos funcionais

## RF-01 — Configuração no repositório

Em cada execução, o programa deve procurar `.risk-gate.yaml` na raiz do
checkout. A ausência do arquivo, YAML inválido, nível desconhecido, regra
duplicada ambígua ou comando sem nome deve resultar em `FAILED` e exit code
não zero.

## RF-02 — AST completa e independente de linguagem

O analisador deve percorrer o projeto inteiro dentro do escopo configurado e
montar uma representação comum de AST. O nó comum deve conservar, no mínimo,
linguagem, caminho, tipo do nó, nome qualificado quando disponível, intervalo
de origem e filhos.

A primeira matriz de suporte é Python, JavaScript, TypeScript e Elixir. Parser
ausente ou erro de parsing deve aparecer no relatório com caminho e motivo.
Uma árvore parcial não pode ser apresentada como análise completa.

## RF-03 — Mapeamento de módulos

Cada entrada de `riskMap` deve ser comparada ao caminho normalizado do arquivo e
dos módulos descobertos. O mapeamento precisa permitir identificar quais
módulos vivem sob cada path rule, inclusive quando a alteração é em um arquivo
de teste, configuração ou infraestrutura sem módulo AST correspondente.

O caminho é relativo à raiz do repositório, usa `/` e não começa com `./`.
Uma regra de diretório termina em `/`; uma regra de arquivo pode ser exata.

## RF-04 — Risco do Pull Request

O diff deve ser obtido dos commits do PR, incluindo arquivos adicionados,
modificados, renomeados e removidos. Para cada arquivo alterado, o sistema
calcula o maior risco das regras aplicáveis. O risco do PR é o máximo entre
esses resultados.

Se nenhum arquivo alterado casar com o mapa, o risco é `normal` (fallback
seguro). PR sem arquivos alterados é inválido para análise e deve falhar.

### RF-04.1 — Impacto em cascata e código novo isolado

O analisador deve construir um grafo de referências entre os arquivos
suportados. Arestas podem representar imports, requires, aliases ou usos
equivalentes identificados pelo parser. A busca de impacto considera a cadeia
transitiva em ambas as direções: código novo que usa uma área crítica e código
existente que passa a usar código novo são evidências de impacto.

Uma regra pode declarar `newCodePolicy: ignore`. Essa exceção só se aplica a
arquivo novo sem caminho no grafo até qualquer regra de risco. Código novo
conectado conserva o risco aplicável. O relatório deve mostrar os arquivos e a
cadeia que causaram o risco; código isolado ignorado também deve ser explícito.

## RF-05 — Gate rules

Depois do risco máximo, o sistema deve selecionar `gateRules[risco]`. Uma regra
de nível superior não herda automaticamente a regra de níveis inferiores. A
configuração pode usar `extends` no futuro, mas isso precisa ser especificado
antes de ser implementado.

`instantMerge: true` significa que o resultado do gate não bloqueia o merge e a
Action termina com sucesso para fins de mergeability. O relatório ainda deve
mostrar validações falhas ou incompletas, e a execução nunca pode omitir seus
resultados.

Sem `instantMerge`, qualquer validação exigida que não termine com exit code 0,
ou que não possa ser executada, faz o gate falhar.

## RF-06 — Aprovação por grupo

`requireApprovalGroup` identifica um grupo lógico de aprovação no GitHub. O
adaptador GitHub deve resolver o grupo para usuários/equipe configurados,
verificar aprovações válidas no PR e relatar quem aprovou e o que faltou.
Permissão insuficiente para verificar aprovação é `FAILED`, não aprovação
implícita. A sintaxe de resolução do grupo deve ser documentada pelo
adaptador; a primeira versão deve aceitar `org/team` e uma lista de logins.

## RF-07 — Validações

`validations` referencia comandos nomeados na seção `validations`. Cada comando
selecionado deve ser executado uma vez, com timeout e saída capturados. Todas
as validações exigidas devem retornar sucesso para o Risk Gate retornar
sucesso. O resultado deve incluir nome, comando identificado, duração, exit
code, estado e trecho de saída seguro para log.

## RF-08 — Relatório no PR

Toda análise deve gerar um comentário idempotente no PR contendo:

1. risco final e nível ordenado;
2. arquivos e módulos que determinaram o maior risco;
3. descrição configurada nas regras correspondentes;
4. gates selecionados, aprovações e validações;
5. limitações, parsers ausentes e estados `PARTIAL`/`FAILED`;
6. identificador e versão do risk-gate.

O comentário deve ser atualizado quando já existir a marca
`<!-- risk-gate:report -->`; não deve criar uma nova mensagem a cada execução.
