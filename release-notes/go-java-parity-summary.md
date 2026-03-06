# Go/Java parity summary (for release notes)

## Summary
- Go / Java support now reaches parity with existing strict-LSP coverage goals.
- Both languages are wired end-to-end across CLI, analyzer selection, strict-LSP routing, mock fixtures, and opt-in real-LSP E2E.

## What was added
- Language plumbing
  - `LanguageMode` / `LanguageKind` support for Go and Java
  - CLI `--lang` accepts `go` / `java`
  - path-based analyzer selection includes `.go` / `.java`
- Local analyzer path
  - Go analyzer + Java analyzer integration for changed-symbol and impact flows
  - CLI integration tests for Go/Java changed/impact paths
- Strict LSP routing
  - strict server selection supports `gopls` / `jdtls`
  - profile/path/languageId handling extended for Go/Java
  - Auto mode can resolve to Go/Java strict server paths
- Strict mock test coverage
  - callers / callees / both fixtures for Go and Java
  - refs-only capability route coverage for `callees` / `both`
  - stability assertions for changed/impacted outputs (including sort/dedup checks)
- Real-LSP E2E (opt-in, skip-safe)
  - Go fixture + strict callers/callees/both E2E
  - Java fixture + strict callers/callees/both E2E
  - environment-gated execution and tool-availability checks

## Runtime conditions (real-LSP E2E)
- Go
  - requires `gopls`
  - opt-in: `DIMPACT_E2E_STRICT_LSP_GO=1` or `DIMPACT_E2E_STRICT_LSP=1`
- Java
  - requires `jdtls`
  - opt-in: `DIMPACT_E2E_STRICT_LSP_JAVA=1` or `DIMPACT_E2E_STRICT_LSP=1`

## Validation status
- `cargo test -q --test engine_lsp` passed
- `cargo test -q` passed
- `cargo clippy -q --all-targets -- -D warnings` passed
