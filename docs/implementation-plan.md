# Risk Gate Implementation Plan

This plan turns the specifications into executable work. It is retained as a
traceable backlog even though the first implementation already covers the core
domain, configuration, AST, diff, gates, report, CLI, and Action.

## Product completion criteria

A first release is complete when a real GitHub Actions execution can:

1. load and validate .risk-gate.yaml;
2. build a complete AST for Python, JavaScript, TypeScript, and Elixir;
3. map changed paths and modules from the PR diff;
4. calculate and explain the highest risk;
5. execute configured gates, validations, and approvals;
6. create or update one idempotent PR comment;
7. emit outputs and exit codes consistent with instantMerge;
8. preserve parsing errors, partial analysis, and limitations;
9. include a Mermaid graph of direct and transitive impact.

## Phase 0 — Foundation

- T-001: establish the Rust workspace, library, binary, and test layout.
- T-002: define fmt, Clippy, test, coverage, mutation, and dependency checks.
- T-003: record dependency decisions and cross-compilation constraints.
- T-004: define domain types for risk, status, paths, modules, diff,
  assessment, gate, validation, approval, and report.

Acceptance: a clean checkout runs fmt and the complete test suite.

## Phase 1 — Configuration

- T-010: load YAML from the repository root or an explicit path.
- T-011: validate version, risk levels, unknown keys, references, and limits.
- T-012: normalize relative paths without allowing .. to escape the repository.
- T-013: support canonical and shorthand riskMap entries.
- T-014: add contract tests for valid and invalid configurations.
- T-015: keep examples synchronized with specs/configuration.md.
- T-016: implement risk-gate init with commented permissive defaults.

Acceptance: invalid configuration fails explicitly and generated configuration
loads successfully.

## Phase 2 — Project discovery and AST

- T-020: walk the project with explicit exclusions for .git, build artifacts,
  vendored dependencies, and node_modules.
- T-021: define the common parser adapter and preserve file, type, name, range,
  children, language, and parse errors.
- T-022: implement Python parsing and import extraction.
- T-023: implement JavaScript parsing and import/export/require extraction.
- T-024: implement TypeScript parsing and type/module extraction.
- T-025: implement Elixir parsing and alias/import/require/use extraction.
- T-026: report unsupported files and parser errors instead of dropping them.
- T-027: add fixtures for all four parsers.
- T-028: prove that complete AST results are stable regardless of filesystem
  traversal order.

Acceptance: the complete scoped project is parsed with bounded resources.

## Phase 3 — Diff, modules, and risk

- T-030: implement a local Git diff provider for added, modified, deleted, and
  renamed files.
- T-031: implement a paginated GitHub diff provider.
- T-032: map changed paths and discovered modules to every applicable rule.
- T-033: apply specificity and maximum RiskLevel precedence.
- T-034: evaluate old and new paths for renames and preserve deleted-file risk.
- T-035: fail explicitly on an empty PR diff.
- T-036: test multiple rules, ties, subpaths, unknown paths, new files, removed
  files, renames, and isolated code.
- T-037: build a bidirectional dependency graph and transitive impact paths.
- T-038: render impact order as Mermaid in the report.

Acceptance: the assessment explains both direct rule matches and graph impact.

## Phase 4 — Gates and validations

- T-040: select the exact gateRules entry for final risk.
- T-041: execute commands with explicit directory, environment, timeout, output
  capture, and output limits.
- T-042: distinguish success, non-zero exit, timeout, missing command,
  interruption, and not-started states.
- T-043: resolve GitHub team and explicit-user approval groups.
- T-044: implement instantMerge without hiding technical failures.
- T-045: document stable exit codes.
- T-046: test gates with fake executors and GitHub adapters.

Acceptance: every required validation succeeds before a blocking gate succeeds.

## Phase 5 — Reporting

- T-050: include risk, evidence, paths, modules, descriptions, graph impact,
  Mermaid, gates, validations, approvals, AST counts, and limitations.
- T-051: safely escape code, configuration, commands, and GitHub content.
- T-052: update the comment marked <!-- risk-gate:report --> instead of
  creating duplicates.
- T-053: publish risk, status, merge_blocked, and report_url outputs.
- T-054: expose authentication and permission failures without downgrading risk.
- T-055: test success, high risk, failed validation, partial analysis, missing
  approval, and large reports.

## Phase 6 — Action and release

- T-060: define Action inputs, outputs, permissions, runtime, and binary version.
- T-061: run fmt, Clippy, tests, coverage, mutation, and dependency checks in CI.
- T-062: validate the Action against controlled PR fixtures.
- T-063: publish checksummed Linux, macOS, and Windows binaries concurrently.
- T-064: validate minimum GitHub permissions.
- T-065: protect master with required PR review, rebase, and quality checks.

Acceptance: a release is usable by another repository without compiling Rust.

## Phase 7 — Security, performance, and observability

- T-070: audit shell execution, environment, timeouts, output, signals, and
  token exposure.
- T-071: audit Markdown and API escaping, URLs, IDs, headers, and payloads.
- T-072: bound files, AST nodes, concurrency, memory, duration, and comment size.
- T-073: benchmark small, medium, and large projects.
- T-074: record execution correlation without leaking credentials.
- T-075: test cancellation, timeout, unavailable network, pagination, broken
  parser, and full disk.

## Phase 8 — Documentation and maintenance

- T-080: document installation, local modes, environment variables, and exit
  codes.
- T-081: document GitHub permissions, tokens, teams, users, forks, and private
  Action access.
- T-082: maintain the changelog and identify incompatible changes.
- T-083: automate release artifacts, checksums, tags, and Action updates.
- T-084: run a release checklist with parser fixtures and an E2E PR.
- T-085: document known syntax, monorepo, parser, and GitHub limitations.

Agents may split these tasks while respecting their dependencies. Every agent
must deliver code, tests, and English documentation for its task. Before a
phase is integrated, its acceptance criteria must be run and limitations must
be recorded rather than marking partial work complete.
