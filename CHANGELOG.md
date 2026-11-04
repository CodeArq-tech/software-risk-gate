# Changelog

## 0.1.11

- Raises the risk of a changed file only through mapped modules that depend on
  it, directly or transitively. Code that the changed file merely uses, and
  modules that only share a dependency with it, no longer raise its risk.
- Unmapped modules in an impact chain no longer inherit the rule of the
  previous module, so the report only cites rules that really match.
- Impact chain details now read "is used by" to state the dependency direction.
- newCodePolicy: ignore keeps its contract: a new file stays isolated only when
  no graph path in either direction reaches mapped code.

## 0.1.10

- Applies a `risk: <level>` label to the pull request, colored with the report
  risk palette, and removes labels for other risk levels on each run.
- Creates the repository label when missing and restores its color when it
  drifted.
- Adds the `riskLabels` configuration key (default `true`) and the
  `risk_label` Action output. Label failures fail the run instead of being
  reported as success.

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
