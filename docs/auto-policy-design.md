# Auto policy option design (AU54-1)

This document defines the **option design** for Auto engine policy expansion while keeping backward compatibility.

## Goal

Add an Auto-policy option that supports two behaviors:

- **compat (default)**: preserve current behavior
- **strict-if-available**: prefer strict LSP when safely available, otherwise fall back to TS

This task is design-only. Runtime implementation is handled in AU54-2.

## Scope

- Applies when `--engine auto` is selected.
- Does not change behavior for explicit `--engine ts` or `--engine lsp`.

## Proposed option surface

### CLI

```bash
--auto-policy <compat|strict-if-available>
```

- default: `compat`

### Env override (optional but recommended)

```bash
DIMPACT_AUTO_POLICY=compat|strict-if-available
```

Priority:
1. explicit CLI (`--auto-policy`)
2. env (`DIMPACT_AUTO_POLICY`)
3. default (`compat`)

## Policy behavior definition

### `compat` (default)

- Keep current behavior exactly:
  - `--engine auto` resolves to TS path (current baseline behavior).
- No behavior change for existing users.

### `strict-if-available`

- Prefer strict LSP only when selection is unambiguous and safe.
- If strict LSP cannot be safely selected, fall back to TS (non-breaking).

#### Selection rules (design)

1. **Explicit language mode** (`--lang` set to one language)
   - If corresponding strict LSP server is available: use LSP strict.
   - Otherwise: fallback to TS.

2. **`--lang auto` with file-based inference**
   - If changed files are unambiguously one supported language and server is available: use LSP strict.
   - If mixed/ambiguous languages: fallback to TS.

3. **Failure handling**
   - Any capability/server mismatch in auto-policy path should fallback to TS (not hard-fail).

## Logging expectations

Add policy decision logs so users can understand selection:

- policy + selected engine
- fallback reason (missing server / mixed language / capability mismatch)

Example:

```text
engine:auto policy=strict-if-available selected=lsp(strict) lang=python
engine:auto policy=strict-if-available fallback=ts reason=missing-server lang=ruby
```

## Backward compatibility requirements

- Default remains `compat`.
- Existing invocations without `--auto-policy` must remain behavior-compatible.
- Existing CI and local scripts must not require changes.

## Notes for follow-up tasks

- **AU54-2**: implement selection logic and option wiring.
- **AU54-3**: finalize user-facing fallback/error messages.
- **AU54-4**: benchmark policy difference (`compat` vs `strict-if-available`).
- **AU54-5**: document usage in README / README_ja.
