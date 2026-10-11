# ADR-0001: Rust Runtime and Tree-sitter

## Decision

The runtime will be Rust. The first language matrix will use official
Tree-sitter grammar crates behind a project-owned interface that converts
language nodes into the common Risk Gate AST.

## Rationale

This keeps one binary suitable for GitHub Actions, allows incremental parsing
in the future, and separates each language grammar from the risk domain. The
domain does not depend directly on grammar-specific types.

The existence of a syntax tree does not mean complete semantic analysis.
Syntax errors, unsupported languages, and unnamed nodes must be reported with
state and evidence in the report.
