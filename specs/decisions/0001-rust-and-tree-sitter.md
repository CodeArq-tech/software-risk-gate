# ADR-0001 — Rust e parsers Tree-sitter

## Decisão

O runtime será Rust. A primeira matriz de linguagens usará as crates oficiais
de gramática Tree-sitter, atrás de uma interface própria que converte os nós
para a AST comum do Risk Gate.

## Motivo

Isso mantém um único binário adequado para GitHub Actions, permite parsing
incremental no futuro e separa a gramática de cada linguagem do domínio de
risco. O domínio não depende diretamente de tipos específicos de uma gramática.

## Limites

A existência de uma árvore sintática não significa análise semântica completa.
Erros de sintaxe, linguagem sem suporte e nós sem nome devem ser reportados com
estado e evidência no relatório.
