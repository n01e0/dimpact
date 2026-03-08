# strict real-LSP callers-not-reported classification (B1)

source: `tests/engine_lsp.rs`

## summary

- languages: **8** (TS/JS/Ruby/Go/Java/Python/Rust/TSX)
- active callers-not-reported: **4**
- promoted to fail-fast logic: **3**
- other: **1**

## language classification

| language | status | cause summary | next task |
|---|---|---|---|
| tsx | active-callers-not-reported | LSP callers result empty in current env (paired with unavailable markers) | B2 |
| typescript | active-callers-not-reported | LSP callers result empty in current env (paired with unavailable markers) | B2 |
| javascript | active-callers-not-reported | LSP callers result empty in current env (paired with unavailable markers) | B3 |
| ruby | active-callers-not-reported | LSP callers result empty in current env (paired with unavailable markers) | B4 |
| go | promoted-failfast-logic | not-reported skip removed; callers emptiness is treated as logic failure | B5 |
| java | promoted-failfast-logic | not-reported skip removed; callers emptiness is treated as logic failure | B6 |
| python | promoted-failfast-logic | not-reported skip removed; callers emptiness is treated as logic failure | B7 |
| rust | env-server-dominated | no callers-not-reported marker; env/server availability dominates failure path | B8 |

## marker details

### tsx
- functions: `lsp_engine_strict_tsx_callers_chain_e2e_when_available`
- status: `active-callers-not-reported`
- cause: LSP callers result empty in current env (paired with unavailable markers)
- next: `B2`
- markers:
  - `env_gate_failfast_helper`: `true`
  - `failfast_assert_expected_caller`: `false`
  - `server_preflight_failfast`: `true`
  - `skip_callers_impact_unavailable`: `true`
  - `skip_callers_not_reported`: `true`
  - `skip_changed_symbols_unavailable`: `true`
  - `skip_rust_analyzer_missing`: `false`
  - `skip_strict_lsp_unavailable`: `false`

### typescript
- functions: `lsp_engine_strict_typescript_callers_chain_e2e_when_available`
- status: `active-callers-not-reported`
- cause: LSP callers result empty in current env (paired with unavailable markers)
- next: `B2`
- markers:
  - `env_gate_failfast_helper`: `true`
  - `failfast_assert_expected_caller`: `false`
  - `server_preflight_failfast`: `true`
  - `skip_callers_impact_unavailable`: `true`
  - `skip_callers_not_reported`: `true`
  - `skip_changed_symbols_unavailable`: `true`
  - `skip_rust_analyzer_missing`: `false`
  - `skip_strict_lsp_unavailable`: `false`

### javascript
- functions: `lsp_engine_strict_javascript_callers_chain_e2e_when_available`
- status: `active-callers-not-reported`
- cause: LSP callers result empty in current env (paired with unavailable markers)
- next: `B3`
- markers:
  - `env_gate_failfast_helper`: `true`
  - `failfast_assert_expected_caller`: `false`
  - `server_preflight_failfast`: `true`
  - `skip_callers_impact_unavailable`: `true`
  - `skip_callers_not_reported`: `true`
  - `skip_changed_symbols_unavailable`: `true`
  - `skip_rust_analyzer_missing`: `false`
  - `skip_strict_lsp_unavailable`: `false`

### ruby
- functions: `lsp_engine_strict_ruby_callers_chain_e2e_when_available`
- status: `active-callers-not-reported`
- cause: LSP callers result empty in current env (paired with unavailable markers)
- next: `B4`
- markers:
  - `env_gate_failfast_helper`: `true`
  - `failfast_assert_expected_caller`: `false`
  - `server_preflight_failfast`: `true`
  - `skip_callers_impact_unavailable`: `true`
  - `skip_callers_not_reported`: `true`
  - `skip_changed_symbols_unavailable`: `true`
  - `skip_rust_analyzer_missing`: `false`
  - `skip_strict_lsp_unavailable`: `false`

### go
- functions: `lsp_engine_strict_go_callers_chain_e2e_when_available`
- status: `promoted-failfast-logic`
- cause: not-reported skip removed; callers emptiness is treated as logic failure
- next: `B5`
- markers:
  - `env_gate_failfast_helper`: `true`
  - `failfast_assert_expected_caller`: `true`
  - `server_preflight_failfast`: `true`
  - `skip_callers_impact_unavailable`: `false`
  - `skip_callers_not_reported`: `false`
  - `skip_changed_symbols_unavailable`: `false`
  - `skip_rust_analyzer_missing`: `false`
  - `skip_strict_lsp_unavailable`: `false`

### java
- functions: `lsp_engine_strict_java_callers_chain_e2e_when_available`
- status: `promoted-failfast-logic`
- cause: not-reported skip removed; callers emptiness is treated as logic failure
- next: `B6`
- markers:
  - `env_gate_failfast_helper`: `true`
  - `failfast_assert_expected_caller`: `true`
  - `server_preflight_failfast`: `true`
  - `skip_callers_impact_unavailable`: `false`
  - `skip_callers_not_reported`: `false`
  - `skip_changed_symbols_unavailable`: `false`
  - `skip_rust_analyzer_missing`: `false`
  - `skip_strict_lsp_unavailable`: `false`

### python
- functions: `lsp_engine_strict_python_callers_chain_e2e_when_available`
- status: `promoted-failfast-logic`
- cause: not-reported skip removed; callers emptiness is treated as logic failure
- next: `B7`
- markers:
  - `env_gate_failfast_helper`: `true`
  - `failfast_assert_expected_caller`: `true`
  - `server_preflight_failfast`: `true`
  - `skip_callers_impact_unavailable`: `false`
  - `skip_callers_not_reported`: `false`
  - `skip_changed_symbols_unavailable`: `false`
  - `skip_rust_analyzer_missing`: `false`
  - `skip_strict_lsp_unavailable`: `false`

### rust
- functions: `lsp_engine_strict_callers_chain_is_stable_when_available`, `lsp_engine_strict_methods_chain_resolves_callers_when_available`
- status: `env-server-dominated`
- cause: no callers-not-reported marker; env/server availability dominates failure path
- next: `B8`
- markers:
  - `env_gate_failfast_helper`: `true`
  - `failfast_assert_expected_caller`: `false`
  - `server_preflight_failfast`: `false`
  - `skip_callers_impact_unavailable`: `false`
  - `skip_callers_not_reported`: `false`
  - `skip_changed_symbols_unavailable`: `false`
  - `skip_rust_analyzer_missing`: `true`
  - `skip_strict_lsp_unavailable`: `true`
