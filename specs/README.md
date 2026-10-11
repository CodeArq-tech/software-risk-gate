# Risk Gate Specifications

This directory defines product contracts. These documents are normative for
implementation and tests; examples in docs/ are illustrative but must follow
the same contracts.

## Documents

- configuration.md: the .risk-gate.yaml contract;
- requirements.md: functional requirements and acceptance rules;
- architecture.md: component boundaries and operational constraints;
- github-action.md: Action inputs, outputs, permissions, and CLI modes;
- decisions/: architecture decision records.

Implementation work is tracked in docs/implementation-plan.md.

## Normative vocabulary

- changed file: a path returned by the Pull Request diff;
- module: a semantic unit discovered in the AST and associated with a file and
  logical path;
- path rule: a riskMap entry that maps a path prefix to a level and optional
  description;
- gate: behavior applied after maximum risk is calculated;
- validation: a named command that must succeed when selected;
- approval group: a GitHub team or explicit user set required by a gate.

The words must and must not are mandatory requirements. May describes a
compatible extension that does not change defined behavior.
