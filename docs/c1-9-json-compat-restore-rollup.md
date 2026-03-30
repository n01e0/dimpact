# C1-9 rollup: JSON compatibility restore

C1 was about undoing one specific mistake cleanly:

> keep the `schema` subcommand family, but stop changing the normal JSON output shape.

The break had been introduced when runtime JSON started embedding `_schema` / `json_schema` / `data` around every payload.
That made `diff`, `impact --per-seed`, and `id` stop being top-level arrays, and made `changed` / `impact` gain an extra object layer.

C1 restores the pre-envelope contract while keeping the schema registry work.

## Source of truth

Two commits define the before/after boundary:

- `d25f87a` — schema registry + concrete schema files exist, but normal JSON output is still payload-only
- `3904eab` — runtime JSON starts embedding `_schema` / `json_schema` / `data`

C1 treats `d25f87a` as the compatibility target and rolls back the runtime-envelope part of `3904eab`.

## Merged PRs

- `#586` docs: add C1 JSON compatibility restore memo
- `#587` test: lock pre-envelope json shape fixtures
- `#588` fix: restore legacy top-level json output
- `#589` fix: restore payload-only non-impact schemas
- `#590` fix: restore payload-only impact schemas
- `#591` ci: drop json envelope compatibility shims
- `#592` test: separate schema snapshots from runtime json
- `#593` docs: clarify schema help vs runtime output

## What changed in practice

## 1. Normal JSON output is payload-only again

These commands now keep their historical top-level shapes again:

- `diff -f json` → top-level array
- `changed -f json` → top-level object
- `impact -f json` → top-level object
- `impact --per-seed -f json` → top-level array
- `id -f json` → top-level array

The default JSON contract no longer embeds:

- `_schema`
- `json_schema`
- `data`

That is the core compatibility recovery.

## 2. The schema layer still exists, but as an explicit lookup surface

C1 did **not** remove the schema work.
The following commands remain part of the CLI:

- `dimpact schema --list`
- `dimpact schema --id <schema-id>`
- `dimpact schema resolve <subcommand> ...`

The contract boundary is now explicit:

- runtime JSON commands return their normal payloads directly
- schema metadata is available through the `schema` subcommand family

In other words, schema is now documented and tested as a help / lookup layer, not as a transport envelope.

## 3. Concrete schema documents were restored to payload shapes

The schema registry did not just survive; its concrete documents were realigned with the restored payload contract.

That means:

- `diff/default` is again a top-level array schema
- `changed/default` is again a top-level object schema
- `id/default` is again a top-level array schema
- `impact/default/*` schemas describe payload objects directly
- `impact/per_seed/*` schemas describe payload arrays directly

So `schema --id` now returns documents that describe the actual payload shapes users receive from the corresponding JSON commands.

## 4. Regression coverage now locks the right thing

C1 added and updated two different regression surfaces on purpose.

### 4.1 Runtime JSON compatibility regressions

Committed fixtures now lock the pre-envelope shapes for:

- `diff`
- `changed`
- `impact`
- `id`

Those tests now parse runtime JSON directly instead of tolerating envelope wrappers.

### 4.2 Schema-layer regressions

Schema snapshot / regression coverage now focuses only on:

- `schema --list`
- `schema --id`
- `schema resolve`

The machine-readable snapshot explicitly excludes runtime JSON outputs, so runtime payload compatibility and schema registry drift are no longer conflated.

## 5. CI and scripts were brought back in line with the restored contract

C1 removed the compatibility shims that had been added to keep envelope-shaped JSON passing in automation.

Examples:

- the impact PR workflow no longer unwraps `.data`
- `scripts/verify-precision-regression.sh` no longer accepts `_schema/json_schema/data` wrappers
- shared test helpers no longer strip a runtime envelope before asserting payload shape

This matters because “support both” would have weakened regression detection.
C1 intentionally makes CI fail again if someone reintroduces the default envelope.

## 6. Public docs now say the right thing

README / README_ja / S1 rollup docs now describe the post-C1 contract correctly:

- schema is an explicit inspection surface
- normal JSON output shape does not change
- runtime JSON is payload-only
- schema documents live under `resources/schemas/json/v1/`

C1 also leaves a historical note in the earlier S1 design memo so the old envelope discussion is readable as design history rather than current contract.

## What was restored

C1 restores five separate compatibility boundaries together:

1. **runtime top-level JSON shape**
2. **schema documents matching real payloads**
3. **schema CLI behavior without runtime envelope assumptions**
4. **CI / scripts rejecting envelope regressions**
5. **docs describing schema as lookup, not wrapping**

That full bundle is important.
Just removing `_schema` from stdout would not have been enough if tests, workflows, schema docs, and docs text still encoded the envelope model.

## Final contract after C1

The practical rule is now simple:

- if you run `diff` / `changed` / `impact` / `id` with `-f json`, you get the payload directly
- if you need schema ids, schema paths, or concrete JSON Schema documents, use `dimpact schema ...`

That separation restores backward compatibility for existing JSON consumers while keeping the schema registry work useful and deterministic.

## Deliberate non-goals

C1 does **not** add:

- a new opt-in runtime envelope
- a YAML schema layer
- external schema hosting
- dual payload/envelope families that both need long-term support

The point was to recover compatibility, not to introduce another transport layer.

## Bottom line

C1 turns the schema work from “runtime JSON is wrapped now” into the healthier model:

- **payload shape is the runtime contract**
- **schema lookup is an explicit auxiliary surface**

That gets old consumers working again without throwing away the schema registry, concrete schema documents, or canonical profile resolution work.
