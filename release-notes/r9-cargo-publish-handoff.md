# R9 cargo publish handoff note

Date: 2026-03-26

## Scope

This note summarizes the minimum pre-`cargo publish` checkpoints requested for the release handoff:

- version
- lockfile consistency
- release workflow status
- install guidance

## 1. Version checkpoint

### Current repository version

`Cargo.toml` currently declares:

```toml
version = "0.5.3"
```

### Current git release/tag state

Latest git tag / GitHub Release is also `v0.5.3`.

### Current crates.io state

`crates.io` still reports `dimpact = 0.1.1`.

Observed with:

```bash
cargo search dimpact --limit 5
cargo info --registry crates-io dimpact
```

### Handoff interpretation

- If the next publish target is **the current repository state as `0.5.3`**, the version number is still publishable from the crates.io side because crates.io is behind.
- If the intention is to cut a **new release after these cleanup fixes**, bumping to `0.5.4` (or another next version) before publish would be cleaner.
- In other words, the important decision left for the publisher is: **publish current code as `0.5.3`, or bump before publish.**

## 2. Lockfile checkpoint

The `Cargo.lock` / dependency drift issue that was causing local rewrites has already been addressed operationally.

Current verification results:

```bash
cargo metadata --locked --format-version 1
```

Result: **PASS**

Also verified:

```bash
cargo install --locked --path . --root /tmp/r9-install
/tmp/r9-install/bin/dimpact --help
```

Result: **PASS**

### Handoff interpretation

- lockfile and manifest are currently aligned enough for locked resolution
- local install from the repository works with `--locked`
- this is a good sign for `cargo publish` preconditions, although this task did **not** run the actual publish command

## 3. Release workflow checkpoint

The broken release workflow was fixed in PR #564.

Key state now:

- `.github/workflows/release.yml` uses valid action refs
  - `docker/login-action@v3`
  - `docker/build-push-action@v6`
- tag pushes create/publish the release image
- tag pushes create a normal GitHub Release via `gh release create`
- `workflow_dispatch` is now a safe smoke path that builds without pushing packages

Manual smoke evidence:

- Release workflow dispatch run: https://github.com/n01e0/dimpact/actions/runs/23593262112
- Result: **SUCCESS**

### Handoff interpretation

- the release workflow is no longer stuck at `Set up job`
- there is fresh evidence that the repaired workflow executes successfully
- this does **not** prove an end-to-end tagged publish yet, but it removes the known hard blocker

## 4. Install guidance checkpoint

### English README

`README.md` now includes:

- source build instructions
- `cargo install --locked --path .`
- Docker install / run guidance via `ghcr.io/n01e0/dimpact:latest`

### Japanese README

`README_ja.md` currently includes:

- source build instructions
- `cargo install --path .`

It does **not** currently mirror the Docker install section from `README.md`.

### Handoff interpretation

- English install guidance is in acceptable shape for publish handoff
- Japanese install guidance is still thinner than English
- this is not a hard `cargo publish` blocker, but if the release wants full EN/JA parity, `README_ja.md` should be synced in a follow-up

## Publish handoff checklist

| Item | Status | Notes |
| --- | --- | --- |
| Cargo.toml version identified | PASS | repo version is `0.5.3` |
| crates.io current published version identified | PASS | crates.io shows `0.1.1` |
| Cargo.lock resolves with `--locked` | PASS | `cargo metadata --locked` succeeded |
| local locked install works | PASS | `cargo install --locked --path .` succeeded |
| release workflow hard failure fixed | PASS | PR #564 landed |
| release workflow smoke run green | PASS | run `23593262112` succeeded |
| English install docs include cargo + Docker | PASS | `README.md` updated in #562 |
| Japanese install docs parity with Docker section | PARTIAL | `README_ja.md` still lags |

## Suggested next operator steps

1. Decide whether to publish **`0.5.3` as-is** or bump to the next version first.
2. If version stays `0.5.3`, run the actual publish/tag flow from the current mainline.
3. If documentation parity matters before release, sync Docker install guidance into `README_ja.md`.
4. After publish, verify:
   - crates.io version
   - docs.rs build
   - GitHub Release presence
   - GHCR image tags

## Bottom line

For the requested pre-publish items, the repository is mostly in good shape:

- version state is understood
- lockfile is healthy under `--locked`
- release workflow is repaired and smoke-tested
- install guidance exists, though EN/JA parity is not perfect

The last real decision is versioning strategy for the upcoming publish (`0.5.3` vs next bump).
