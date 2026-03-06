# Changelog

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
