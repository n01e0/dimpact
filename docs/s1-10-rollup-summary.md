# S1-10 rollup: schema subcommand and runtime schema identity

S1 started from a simple goal: every JSON surface should be able to say **which schema it is emitting**, and the CLI should be able to resolve and fetch that same schema deterministically.

This rollup closes that work.

## What shipped

S1 landed five user-visible capabilities.

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

4. **Runtime schema-tagged JSON output**
   - JSON outputs now carry `_schema.id`
   - JSON outputs now carry `json_schema`
   - all JSON payloads are wrapped in a common envelope with `data`

5. **Snapshot / regression coverage**
   - schema CLI integration tests cover list / id / resolve behavior
   - a fixed registry snapshot now locks canonical ids, resolve results, and schema document digests

## Final public contract

The public JSON contract is now:

```json
{
  "_schema": {
    "id": "dimpact:json/v1/..."
  },
  "json_schema": "resources/schemas/json/v1/...schema.json",
  "data": { ... }
}
```

This applies to JSON outputs for:

- `diff`
- `changed`
- `impact`
- `id` (JSON mode only; `--raw` stays out of scope)

The schema id is the stable contract identifier.
The schema path is the in-repo locator for the concrete JSON Schema document.

## Snapshot companion

Machine-readable registry snapshot:

- `docs/s1-10-schema-registry-snapshot.json`

That snapshot fixes three things together:

- the ordered `schema --list` result
- representative `schema resolve ...` outputs
- a digest for every registered schema document

In practice, this means we now have a regression surface for both:

- **canonical id stability**
- **schema document drift**

## README surface

The README now documents:

- the `schema` subcommand family
- the JSON envelope contract
- where schema documents live in the repository

That matters because the schema work is no longer just an internal implementation detail. It is now part of the user-visible CLI surface.

## What S1 deliberately does not do

S1 stops short of a few things on purpose.

- no separate YAML schema system
- no external schema hosting / published URLs yet
- no schema support for every subcommand in one pass
- no schema version bump policy automation beyond the documented rules

## Why this is a good stopping point

S1 now has the minimum full loop:

- resolve a canonical schema profile from CLI flags
- list registered ids
- fetch the concrete schema document for one id
- emit runtime JSON tagged with that id
- lock the whole surface with snapshot/regression coverage

That is enough to make downstream tooling, tests, and future migrations reason about JSON output in a deterministic way instead of by convention.
