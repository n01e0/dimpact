# Changelog

## 0.5.2 - 2026-03-10

_Change range: `v0.5.1..v0.5.2`._

### Major changes
- Completed the v0.5.2 precision-next loop (N52) and merged the full PR chain (`#367`..`#385`) under required-check gating.
- Tightened precision regression thresholds with per-language policy, including strict zero-FN/FP enforcement on stabilized lanes.
- Expanded strict real-LSP migration and residual-trend evidence in CI/nightly with clearer skip-safe/fail-fast operation reporting.
- Extended hard-fixture and non-regression coverage for dynamic/edge cases across Ruby, Python, Go, and Java lanes.

### Notes
- Tag/release published: `v0.5.2` (2026-03-10).
- Detailed evidence is recorded under `release-notes/0.5.2-*`.

## 0.5.1 - draft

_Change range: `v0.5.0..HEAD`._

### Major changes
- Completed post-release dependency intake with risk-first handling:
  - high-risk updates were validated first (`actions/setup-go`, `tree-sitter`, `rusqlite`)
  - dependency update PRs (`#338-#347`) were classified and merged under CI-green gating
- Added confidence-operation documentation and defaults for production use:
  - per-language confidence distribution sampling report
  - recommended `--min-confidence` default policy (`inferred` in current phase)
  - operational guide for `--exclude-dynamic-fallback` (precision/recall usage patterns)
- Expanded real-corpus quality tracking for Ruby/Python with fixtureized miss patterns:
  - corpus FN/FP measurement artifacts for python/ruby lanes
  - new hard fixtures for call-chain/alias-return miss patterns
- Fixed callees-direction impact/oracle mismatch and validated numeric improvement:
  - adjusted impacted-node inclusion and edge filtering behavior in `src/impact.rs`
  - python corpus re-measure improved from `FN=9, FP=1` to `FN=0, FP=0`

### Operational changes
- Added CI stability evidence for `precision_regression_gate` over recent main runs.
- Updated branch protection required checks on `main` to include `precision_regression_gate`.
- Verified gate enforcement on post-update PR flow (required checks include `precision_regression_gate` and must pass before merge).

### Notes
- Ruby strict-oracle corpus lane remained environment-sensitive (initialize-timeout in sampled runs); python lane was fully re-measured after analyzer fix.
- Release notes under `release-notes/0.5.1-*` capture detailed evidence and before/after measurement snapshots.

## 0.5.0 - draft

_Change range: `v0.4.1..HEAD`._

### Major changes
- Added confidence-threshold filtering controls for impact traversal/output:
  - `--min-confidence confirmed|inferred|dynamic-fallback`
  - `--exclude-dynamic-fallback`
- Reflected confidence filtering results in outputs:
  - JSON/YAML now include `confidence_filter` metadata when filtering is active
  - CLI logs include filtered-edge summary (`kept/input`)
- Extended PDG pipeline for higher precision and explainability:
  - lightweight alias propagation improvements (assignment chains / reassignments)
  - stabilized SSA-like branch join behavior for def-use
  - minimal function summary (`input -> impacted`) implementation
  - inter-procedural propagation connected to function summaries
  - PDG-path certainty unified to `confirmed/inferred`
- Expanded dynamic-language resolvers and hard fixtures:
  - Ruby: stronger `send/public_send`, `alias_method/define_method`, `method_missing/respond_to_missing?`, mixin and DSL hash-dispatch inference
  - Python: stronger `getattr/setattr`, descriptor/decorator-chain, importlib dynamic import, monkey-patch/metaclass/protocol dynamic cases
- Strengthened hard corpus coverage for Go/Java dynamic dispatch patterns and cross-language precision regression fixtures.

### Operational changes
- Added strict-LSP oracle diff comparison script:
  - `scripts/compare-impact-vs-lsp-oracle.sh`
- Precision gate evolved from global-only thresholds to per-language threshold control:
  - `DIMPACT_PRECISION_FN_MAX_BY_LANG`
  - `DIMPACT_PRECISION_FP_MAX_BY_LANG`
- CI precision summary now reports:
  - confidence distribution
  - per-language FN/FP changed-vs-impacted breakdown
  - per-language threshold deltas
  - gate-failure reproduction command
- Regression guardrails were reinforced with additional fixture-backed checks and release-prep test/clippy execution (`engine_lsp`, full `cargo test -q`, `clippy -D warnings`).

### Notes
- Dynamic-heavy Ruby/Python lanes now support language-specific threshold tuning while keeping stricter zero-threshold policy for TS/TSX/Rust/Go/Java lanes.
- Confidence filtering preserves default behavior when no filter flags are provided; filtering is opt-in.

## 0.4.1 - draft

### Major changes
- Added v0.4.1 difficult-case analyzer fixtures for Go / Java / Python and expanded fixture-driven precision tests:
  - Go: generics + chained call + embedded receiver
  - Java: overload + static import + nested type
  - Python: dynamic call/import edge
- Optimized strict-LSP callers/both hot paths by reducing redundant caller-site symbol lookups and reusing document-symbol resolution in references traversal.
- Added strict-LSP profiling and before/after benchmark artifacts for optimization validation.

### Operational changes
- Nightly strict-LSP workflow now auto-classifies flaky causes into `install` / `startup` / `capability` / `timeout`.
- Added flaky-type based auto-retry policy in nightly workflow:
  - `install` / `startup` / `timeout`: retry once
  - `capability` only: no auto-retry by default
- Added failure-time CI summary output with cause/language/evidence/repro command.
- Documented nightly operations flow for triage / retry / escalation.

### Notes
- Strict-LSP stability/performance confirmation is recorded from before/after artifacts with explicit blocked-lane reporting.
- Ruby/Java strict-LSP lanes may still show initialize-timeout instability depending on runtime environment.
- Rust before/after comparison includes input-scope caveats; interpret absolute deltas with caution.

## 0.4.0 - draft

### Major changes
- Completed Python non-LSP analyzer coverage for symbols, unresolved refs/imports, and explicit `--lang python` dispatch.
- Added Python fixture-backed CLI integration tests for both `changed` and `impact` flows.
- Introduced Auto policy option `--auto-policy compat|strict-if-available` for `--engine auto`.
- Added policy-difference benchmark mode to compare TS fixed vs Auto strict-if-available (`--compare-auto-strict-if-available`).

### Operational changes
- Auto strict-if-available now prefers the LSP path while preserving fallback-safe non-strict behavior when capabilities/session are insufficient.
- Capability-shortage diagnostics were reorganized by policy, with clearer strict errors and fallback-reason logs.
- README / README_ja now document Auto policy operations (CLI/env priority and practical commands).
- Release-prep regression gates were executed and passed:
  - `cargo test -q --test engine_lsp`
  - `cargo test -q`
  - `cargo clippy -q --all-targets -- -D warnings`

### Notes
- Default Auto behavior remains backward-compatible (`compat`).
- `strict-if-available` is preference-based and may fall back to TS depending on server/capability/session availability.

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
