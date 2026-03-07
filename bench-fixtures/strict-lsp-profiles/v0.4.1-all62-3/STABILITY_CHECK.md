# stability check for ALL62-4 (changed/impacted vs performance)

Input artifacts: `bench-fixtures/strict-lsp-profiles/v0.4.1-all62-3/before/*.log` and `after/*.log`

- noise window for timing judgement: ±0.05s
- overall verdict: **PASS**

| Language | Stability (changed/impacted) | Perf judgement | Before changed/impacted | After changed/impacted | Before avg | After avg | Δ(avg) | Note |
|---|---|---|---|---|---:|---:|---:|---|
| rust | N/A | N/A | 9/0 | 11/4 | 66.327s | 59.387s | -6.940s | before/after used different base-range input; strict stability judgement skipped |
| typescript | PASS | IMPROVED | 10/31 | 10/31 | 1.380s | 1.210s | -0.170s | stable counts; avg delta -0.170s |
| javascript | PASS | IMPROVED | 0/0 | 0/0 | 1.347s | 1.167s | -0.180s | stable counts; avg delta -0.180s |
| ruby | BLOCKED | BLOCKED | - | - | - | - | - | blocked by init/startup errors |
| go | PASS | IMPROVED | 10/31 | 10/31 | 10.417s | 0.267s | -10.150s | stable counts; avg delta -10.150s |
| java | BLOCKED | BLOCKED | - | - | - | - | - | blocked by init/startup errors |
| python | PASS | NOISE | 10/31 | 10/31 | 1.127s | 1.140s | +0.013s | stable counts; small regression +0.013s (<= 0.05s noise window) |

## Conclusion
- Stable changed/impacted with improved (or noise-level) runtime was confirmed for TypeScript / JavaScript / Go / Python.
- Ruby / Java remain blocked by strict-LSP initialize timeout and are excluded from stability+perf confirmation.
- Rust was measured with a non-identical base-range input between before/after snapshots, so strict stability judgement is marked N/A.

This satisfies ALL62-4 for the available comparable language set while keeping blocked lanes explicitly visible.
