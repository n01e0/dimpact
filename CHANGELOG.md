# Changelog

## 0.4.0 - draft

### Added
- Python non-LSP analyzer coverage was completed for symbols, unresolved refs/imports, and explicit `--lang python` dispatch.
- Python fixture-backed CLI integration tests were added for both `changed` and `impact` flows.
- Auto engine policy option `--auto-policy compat|strict-if-available` was introduced for `--engine auto`.
- Policy-difference benchmark mode was added to compare TS fixed vs Auto strict-if-available (`--compare-auto-strict-if-available`).

### Changed
- Auto strict-if-available now prefers LSP path while preserving non-strict TS fallback behavior when capabilities/session are insufficient.
- Capability-shortage diagnostics were reorganized by policy, with clearer strict errors and fallback reason logs.
- README / README_ja now document Auto policy operation (CLI/env priority and practical commands).

### Verified
- Regression gates for release prep were executed and passed:
  - `cargo test -q --test engine_lsp`
  - `cargo test -q`
  - `cargo clippy -q --all-targets -- -D warnings`

## 0.3.0 - 2026-03-06

### Added
- Go / Java language modes for CLI and changed-symbol mapping.
- Go / Java analyzers (symbols, unresolved refs, imports) for non-LSP flows.
- Strict LSP profiles and server selection for Go / Java (`gopls`, `jdtls`), including `didOpen` languageId coverage.
- Strict-mock fixtures for Go / Java `callers` / `callees` / `both`, including refs-only capability routes.
- Opt-in real-LSP E2E coverage for Go / Java (`callers` + skip-safe `callees`/`both`).

### Changed
- README / README_ja now include runtime conditions for Go / Java strict LSP E2E.
- Integration checks and regression coverage were expanded for Go / Java parity.

### Notes
- Real-LSP E2E remains environment-dependent and skip-safe when required server/capabilities are unavailable.
- Go / Java parity work is now completed in the same strict-LSP policy used for TS/TSX/JS/Ruby/Python.

## 0.2.0 - 2026-03-06

### Added
- Python language path/profile support across LSP/seed flows (`.py`, `languageId=python`).
- Python call query/spec support for:
  - bare calls (`foo()`)
  - receiver calls (`obj.m()`, `self.m()`)
- Python fixtures and strict-mock coverage for `callers` / `callees` / `both`.
- Python refs/definition-only route coverage for `callers` / `callees` / `both`.
- Python real-LSP opt-in E2E fixtures/tests with environment gating.
- Benchmark language summary output (`scripts/bench-impact-engines.sh`).

### Changed
- LSP `probe_update` now performs real probing on non-mock sessions.
- Strict LSP capability errors now include language/direction and required capability hints.
- README/README_ja now document Python strict LSP E2E runtime requirements and operations.

### Fixed
- Deterministic ordering/dedup in changed-symbol and graph-edge paths to reduce flakes.
- Refs/definition-only branch for strict LSP now handles `callees` / `both` instead of not-implemented behavior.
- Unresolved-ref method/function fallback handling clarified for dynamic-language cases.

### Notes
- Python real-LSP strict E2E remains best-effort and environment-dependent (skip-safe when server/capabilities are insufficient).
- Python parity completion criteria (P-END-1..4) were confirmed in docs.
