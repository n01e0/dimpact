# S1-10 rollup: schema subcommand and runtime schema identity

S1 started from a simple goal: JSON schema ids should resolve deterministically from CLI flags, and the CLI should be able to list and fetch the corresponding schema documents.

This rollup closes that work, with one important clarification after the C1 compatibility restore:

> the `schema` subcommand family is a help / lookup layer for JSON contracts, not a wrapper around normal runtime JSON output.

## What shipped

S1 ended up with four user-visible capabilities that remain part of the public CLI surface.

1. **Canonical schema profile resolution**
   - `SchemaProfile` is now the normalized source of truth for JSON schema families.
   - `impact` profiles are normalized across `layout`, `edge_detail`, and `graph_mode`.

2. **Schema registry commands**
   - `dimpact schema --list`
   - `dimpact schema --id <schema-id>`
   - `dimpact schema resolve <subcommand> ...`

3. **Concrete v1 JSON Schema documents**
   - `impact` default + all planned variants
   - `changed/default`
   - `diff/default`
   - `id/default`

4. **Snapshot / regression coverage for the schema layer**
   - schema CLI integration tests cover list / id / resolve behavior
   - a fixed registry snapshot now locks canonical ids, resolve results, and schema document digests

## Final public contract

The public contract is split into two surfaces.

### 1. Normal JSON output stays payload-only

This applies to runtime JSON outputs for:

- `diff`
- `changed`
- `impact`
- `impact --per-seed`
- `id` (JSON mode only; `--raw` stays out of scope)

These commands keep their historical top-level shapes.
They do **not** embed `_schema`, `json_schema`, or `data` wrapper fields.

### 2. The schema layer is explicit and opt-in

Use these commands when you want schema metadata:

- `dimpact schema --list`
- `dimpact schema --id <schema-id>`
- `dimpact schema resolve <subcommand> ...`

The schema id is the stable contract identifier.
The schema path is the in-repo locator for the concrete JSON Schema document.

## Snapshot companion

Machine-readable registry snapshot:

- `docs/s1-10-schema-registry-snapshot.json`

That snapshot intentionally fixes only the schema subcommand surface:

- the ordered `schema --list` result
- representative `schema resolve ...` outputs
- a digest for every registered schema document

It does **not** snapshot normal runtime JSON payloads.
Those are covered by separate JSON compatibility regressions.

In practice, this gives us regression coverage for both:

- **canonical id stability**
- **schema document drift**

without conflating that with runtime payload shape compatibility.

## README surface

The README now documents:

- the `schema` subcommand family
- that schema is a help / lookup layer
- that normal JSON output shape does not change
- where schema documents live in the repository

That matters because the schema work is still user-visible, but it is no longer described as a transport envelope layered onto every JSON command.

## What S1 deliberately does not do

S1 stops short of a few things on purpose.

- no separate YAML schema system
- no external schema hosting / published URLs yet
- no schema support for every subcommand in one pass
- no schema version bump policy automation beyond the documented rules
- no runtime JSON envelope by default

## Why this is a good stopping point

S1 now has the minimum full loop:

- resolve a canonical schema profile from CLI flags
- list registered ids
- fetch the concrete schema document for one id
- keep runtime JSON payloads stable
- lock the schema layer with snapshot/regression coverage

That is enough to make downstream tooling, tests, and future migrations reason about JSON contracts deterministically without forcing a wrapper onto normal JSON output.
