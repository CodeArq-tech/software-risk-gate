# Agent Instructions

## Repository purpose

This repository contains risk-gate, a Rust application executed as a GitHub
Actions step. It analyzes Pull Request risk, applies validation gates that
match the detected risk, and publishes an auditable PR report.

The product is not a generic linter or an external service. Configuration is
versioned with the analyzed project in .risk-gate.yaml.

## Mandatory rules for agents

1. Implement the product in Rust. Do not introduce a parallel implementation
   in Python, Node.js, or Elixir. Helper tools may use another language only
   when explicitly necessary and kept outside the product runtime.
2. Preserve the library plus CLI architecture. The library owns parsing,
   language discovery, AST construction, mapping, risk calculation, gates, and
   reporting; the CLI adapts those capabilities to GitHub Actions.
3. Every risk decision must be deterministic, explainable, and testable without
   network access. GitHub access must remain replaceable at the boundary.
4. .risk-gate.yaml is part of the public contract. Incompatible changes require
   updates to the specification, examples, and tests.
5. Never treat a validation that was not executed, was interrupted, has a
   missing command, or produced inconclusive output as success.
6. Do not publish, approve, merge, or change GitHub settings outside the
   explicit scope of the task. Reports must be idempotent and updated through
   a stable marker.
7. Do not hide failures through instantMerge. This option only prevents the
   gate from blocking the merge; it does not turn failed validations into
   success or alter the report.
8. Validation commands are untrusted repository configuration. Run them with
   explicit working directory, environment, and timeout; record the result and
   avoid unsafe interpolation of GitHub event values.
9. Always distinguish SUCCESS, PARTIAL, and FAILED. A comment, partial AST, or
   missing permission does not prove complete success.
10. Before declaring an integration complete, prove the real path: PR event,
    configuration loading, diff, analysis, gate execution, comment, and Action
    exit code.
11. All documentation, comments, CLI messages, reports, examples, and other
    user-facing project text must be written in English. Preserve exact
    identifiers, YAML keys, API fields, exit codes, and code literals.

## AST and language model

The AST model must be language-independent. The first supported languages are
Python, JavaScript, TypeScript, and Elixir behind a common interface. The tree
must cover the complete project included in the execution scope, not only
changed files. Files without a parser must be reported as unanalyzed and must
never disappear silently.

## Risk model

The ordered levels, from lowest to highest, are:

minimal < low < normal < high < danger

PR risk is the highest risk applicable to modules reached by changed files.
Changes that do not match riskMap use normal by default so that new areas are
not silently overlooked. A fallback may be configurable only when it is
specified and validated.

The execution flow is:

configuration -> complete AST -> module/path map -> PR diff -> maximum risk -> gateRules -> validations -> PR comment -> exit code

Every stage must expose enough data for diagnosis and testing. Read specs/
before changing contracts and docs/ for operational examples.

## Minimum quality

Before completing a change, run the available Rust checks:

~~~bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
~~~

Add unit tests for risk rules and integration tests for configuration, diff,
gates, and reporting. Every change must update affected documentation. Do not
invent GitHub Actions capabilities that are not covered by tests or explicitly
marked as limitations.
