# strict-LSP before/after benchmark artifact (ALL62-3)

- Before commit: `c3fe33e` (pre ALL62-2 optimization)
- After commit: `c57b20f` (post ALL62-2 optimization)
- Command baseline: `scripts/bench-impact-engines.sh --rpc-counts` (callers direction)
- Scope: Rust / TypeScript / JavaScript / Ruby / Go / Java / Python

| Language | Before avg | After avg | Δ(avg) | Before changed/impacted | After changed/impacted | After top RPC (top3) | Status |
|---|---:|---:|---:|---|---|---|---|
| rust | 66.327s | 59.387s | -6.940s | 9/0 | 11/4 | textDocument/documentSymbol=2050, textDocument/references=782, textDocument/didOpen=85 | OK |
| typescript | 1.380s | 1.210s | -0.170s | 10/31 | 10/31 | callHierarchy/incomingCalls=41, textDocument/prepareCallHierarchy=11, textDocument/documentSymbol=2 | OK |
| javascript | 1.347s | 1.167s | -0.180s | 0/0 | 0/0 | callHierarchy/incomingCalls=41, textDocument/prepareCallHierarchy=11, textDocument/documentSymbol=2 | OK |
| ruby | - | - | - | - | - | - | BLOCKED/UNSTABLE |
| go | 10.417s | 0.267s | -10.150s | 10/31 | 10/31 | callHierarchy/incomingCalls=41, textDocument/prepareCallHierarchy=11, textDocument/documentSymbol=2 | OK |
| java | - | - | - | - | - | - | BLOCKED/UNSTABLE |
| python | 1.127s | 1.140s | +0.013s | 10/31 | 10/31 | callHierarchy/incomingCalls=41, textDocument/prepareCallHierarchy=11, textDocument/documentSymbol=2 | OK |

## Notes
- rust: Rust variance is still high; compare with caution.
- ruby: before: Error: lsp initialize timeout or invalid response / after: Error: lsp initialize timeout or invalid response
- java: before: Error: lsp initialize timeout or invalid response / after: Error: lsp initialize timeout or invalid response

## Raw artifacts
- `before/*.log`
- `after/*.log`
