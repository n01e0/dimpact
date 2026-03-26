# R10 publish waiting / handoff note

Date: 2026-03-26

## Status

The repository is now in **publish handoff** state.

Actual `cargo publish` is intentionally **waiting on n01e0** and should not be treated as an agent-side pending implementation gap.

## Why this is waiting on n01e0

Publishing requires owner/operator intent that the agent should not assume on its own:

- final version decision (`0.5.3` as-is vs bump first)
- crates.io publish execution under the maintainer account
- final release timing / tag timing

Those are release-owner actions, not unattended cleanup actions.

## What is already ready

From the completed cleanup/handoff work:

- lockfile resolves under `--locked`
- local locked install works
- release workflow hard failure is fixed
- release workflow smoke run succeeded
- install guidance is present in README.md
- release / cleanup PR queue is drained
- pre-publish checkpoints are summarized in `release-notes/r9-cargo-publish-handoff.md`

## Required human follow-up (n01e0)

When ready to publish, n01e0 should:

1. choose publish version strategy
   - publish current code as `0.5.3`, or
   - bump to the next version first
2. run the actual publish flow
   - `cargo publish`
3. verify post-publish surfaces
   - crates.io version
   - docs.rs build
   - GitHub Release presence
   - GHCR image/tag state

## Run-state interpretation

This run should be considered **handed off / waiting on n01e0**.

There is no additional repository-side cleanup work remaining before that owner action.
