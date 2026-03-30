# C1-10 final verification: old JSON shape compatibility is restored

This note closes the C1 JSON compatibility restore work.

At this point, every planned C1 PR has landed on `main`, and the repository has been re-verified from the merged state.

## Merged C1 PRs

- `#586` docs: add C1 JSON compatibility restore memo
- `#587` test: lock pre-envelope json shape fixtures
- `#588` fix: restore legacy top-level json output
- `#589` fix: restore payload-only non-impact schemas
- `#590` fix: restore payload-only impact schemas
- `#591` ci: drop json envelope compatibility shims
- `#592` test: separate schema snapshots from runtime json
- `#593` docs: clarify schema help vs runtime output
- `#594` docs: add C1 compatibility restore rollup

## Final contract on `main`

The restored contract is now:

- `diff -f json` returns a top-level array
- `changed -f json` returns a top-level object
- `impact -f json` returns a top-level object
- `impact --per-seed -f json` returns a top-level array
- `id -f json` returns a top-level array

Default runtime JSON output does **not** embed:

- `_schema`
- `json_schema`
- `data`

Schema metadata remains available through the explicit schema lookup surface:

- `dimpact schema --list`
- `dimpact schema --id <schema-id>`
- `dimpact schema resolve <subcommand> ...`

## Final verification run

The final verification was run from merged `main` after PR `#594` landed.

Commands:

```bash
cargo test --test cli_json_compat_restore --test cli_schema_snapshot
cargo test
```

Results:

- `cli_json_compat_restore` passed
- `cli_schema_snapshot` passed
- full `cargo test` passed

These checks cover both sides of the restored boundary:

1. runtime JSON outputs still match the committed pre-envelope compatibility fixtures
2. schema snapshot / schema subcommand behavior stays consistent and separate from runtime payload output

## What this confirms

C1 is complete in the sense that all of the following are true at once:

1. **runtime JSON compatibility is restored**
   - old top-level shapes are back
   - runtime envelope fields are gone

2. **schema documents match real payloads again**
   - `schema --id` returns payload-shape documents
   - `impact` variants still map to deterministic canonical schema profiles

3. **CI and scripts now enforce the restored contract**
   - compatibility shims for runtime envelopes have been removed
   - reintroducing the default envelope should regress tests/automation again

4. **public docs describe the correct model**
   - schema is a help / lookup layer
   - normal JSON output shape does not change

## Closure

C1 started as a rollback of one breaking runtime change, but the real fix required lining up runtime output, schema docs, tests, CI, and docs.

That alignment is now back in place on `main`.
