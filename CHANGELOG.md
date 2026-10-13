# Changelog

## 0.1.9

- Limits the Mermaid cascade graph to the five most critical impacted nodes.
- Collapses remaining nodes into a smaller, risk-colored `Outros X` node.

## 0.1.8

- Propagates mapped risk through downstream modules in the connected dependency
  component so cascade effects remain visible in the bounded graph.
- Caps propagated evidence at 256 impacts to keep analysis resources bounded.

## 0.1.7

- Restores additional bounded cascade nodes by ordering all impact chains with
  the highest risk first instead of discarding lower-risk paths.
- Forces black Mermaid node and edge text and uses visible colored markers in
  the external risk legend.

## 0.1.6

- Moves the risk legend outside the Mermaid impact graph so the diagram stays
  focused on module relationships.
- Keeps risk colors in the report legend and preserves colored nodes in the
  graph.

## 0.1.0

- Implements AST analysis for Python, JavaScript, TypeScript, and Elixir.
- Adds riskMap classification, risk-based gates, validations, and GitHub team
  or user approvals.
- Publishes an idempotent PR report and applies file, AST-node, and comment
  size limits.
- Automates coverage above 95%, mutation testing, and release binary checksums.
