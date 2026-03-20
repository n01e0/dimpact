# G4 rollup: multi-file PDG footing, tighter propagation, and the next boundary

## What landed

This document closes out the G4 work around `impact --with-pdg` / `--with-propagation`.

G4 was not about turning the current implementation into a true project-wide PDG.
The real goal was narrower and more useful:

**make the current PDG / propagation path less file-local where it obviously hurt, make its output more explainable, tighten one over-propagation surface, and establish a cleaner baseline for engine-consistency work.**

G4 therefore landed in two layers:

- first, write down the design target and the evaluation surface
- then ship one minimal multi-file propagation expansion, tighten merge/output behavior, extend witness output, add multi-file regressions, and fix one concrete engine mismatch
- finally, update the public docs so the README matches the new behavior and its remaining limits

## Merged PRs

- `#456` docs: add G4 PDG build scope design memo
- `#457` docs: add G4 multi-file PDG eval set
- `#458` docs: define G4 PDG build scope policy
- `#459` feat(pdg): expand propagation scope to direct boundary files
- `#460` fix(pdg): prune callers-edge propagation fanout
- `#461` feat(impact): add witness path summaries
- `#462` test(pdg): add multi-file CLI regressions
- `#463` fix(pdg): honor engine selection in diff mode
- `#464` docs: update PDG scope and witness guidance

---

## What changed in practice

## 1. G4 now has a written target for multi-file PDG work

G4 started by making the remaining problem explicit.

The important design conclusions were:

- the big unresolved issue after G3 was not just “missing bridges”
- it was the combination of
  - **build scope** being too close to changed/seed files
  - **engine consistency** being weaker on the PDG path than on normal impact
- the desired shape is still **global call graph + local DFG augmentation**, but with a better rule for which nearby files are eligible for that augmentation

Relevant docs:

- `docs/g4-1-pdg-build-scope-engine-consistency-memo.md`
- `docs/g4-3-pdg-build-scope-policy.md`
- `docs/g4-3-pdg-build-scope-policy.json`

This matters because G4 stopped treating “project-wide PDG” as the immediate goal and instead fixed the vocabulary around a smaller, more testable step.

## 2. A multi-file evaluation surface now exists

G3’s fixed set was mostly file-local.
G4 added a new fixed evaluation set centered on multi-file weak spots:

- Rust cross-file call-site summary bridge
- Rust cross-file wrapper / return-flow
- Rust imported-result alias stitching
- Ruby split alias / return-flow chain
- Ruby dynamic target separation as a no-leak guard

Relevant docs:

- `docs/g4-2-pdg-multi-file-eval-set.md`
- `docs/g4-2-pdg-multi-file-eval-set.json`

This gave G4 a stable way to ask:

- which cases are weak because scope is too small
- which cases are weak because bridge logic is still thin
- which cases are vulnerable to over-propagation once scope grows

## 3. Propagation can now recover at least one real multi-file bridge

The first concrete code win in G4 was intentionally small.

`#459` changed propagation-mode PDG setup so that Rust/Ruby local DFG construction can widen from the changed/seed file to **direct boundary files adjacent to the current seeds**.

That does **not** make `--with-pdg` project-wide.
It does mean `--with-propagation` can now recover some short cross-file flows that previously collapsed back to plain call-graph reasoning.

The clearest locked regression is the Rust cross-file call-site summary bridge:

- plain PDG does not synthesize the cross-file `use(x) -> def(y)` bridge
- propagation now can, because the direct callee file is eligible for local DFG construction

Relevant coverage:

- `tests/cli_pdg_propagation.rs::pdg_propagation_adds_cross_file_summary_bridge_for_direct_callee`

## 4. One over-propagation surface was tightened instead of just adding more edges

G4 did not only widen scope.
It also tightened one case where extra propagation detail became noisy.

`#460` pruned callers-mode `--with-edges` output so symbolic/local propagation keeps:

- changed-symbol detail
- callsite-adjacent bridge detail

while dropping irrelevant caller-local fanout on non-callsite lines.

This matters because a multi-file propagation feature is only useful if the output still explains the interesting bridge instead of smearing across nearby defs/uses.

Relevant coverage:

- `tests/cli_pdg_propagation.rs::propagation_callers_edges_keep_cross_file_callsite_bridges_but_drop_irrelevant_symbol_fanout`
- existing no-leak guard cases still remain locked alongside it

## 5. Witnesses are no longer only “one last hop”

`#461` extended `impacted_witnesses` so that impacted symbols can now carry a minimal path summary, not just the selected final edge.

Each witness can now include:

- the existing last-hop fields (`edge`, `via_symbol_id`)
- `path`: one chosen hop-by-hop route from the root changed/seed symbol
- `provenance_chain`: where `call_graph`, `local_dfg`, or `symbolic_propagation` entered that route
- `kind_chain`: where `call`, `data`, or `control` edges appeared along that route

This is still intentionally small:

- it is one selected shortest-path explanation
- it is **not** a set of competing witnesses
- it is **not** an exhaustive proof of all available routes

But it is enough to make multi-file propagation output materially easier to read.

Relevant code / coverage:

- `src/impact.rs`
- unit tests around direct and multi-hop witness reconstruction
- `tests/cli_pdg_propagation.rs` per-seed JSON assertions

## 6. CLI regression coverage now matches the new multi-file goals better

`#462` expanded `tests/cli_pdg_propagation.rs` with more multi-file cases beyond the initial cross-file call-site coverage.

The new locked areas include:

- Rust multi-file wrapper / return-flow behavior
- Ruby `require_relative` split alias / return-flow behavior
- continued no-leak assertions, not just “more edges appeared” assertions

This matters because G4 now has fixture-backed guardrails for:

- improvement cases
- precision / no-smear cases
- JSON witness and edge-shape cases
- engine-consistency cases in PDG diff mode

## 7. One large engine-consistency mismatch was removed

Before `#463`, diff-based PDG / propagation used a direct TS changed-symbol path (`compute_changed_symbols()`) instead of the selected engine.

That meant a user could ask for strict LSP behavior and get it on the normal impact path, while PDG diff mode silently bypassed that contract.

G4 fixed that specific mismatch by making diff-mode PDG / propagation use:

- `engine.changed_symbols(...)`

instead of the hard-wired TS path.

This does **not** fully unify the PDG path with the selected engine.
But it does close one very visible mismatch:

- strict-LSP failure semantics in diff mode now stay aligned between plain impact and PDG / propagation

Relevant regression:

- `tests/cli_pdg_propagation.rs::pdg_diff_mode_respects_strict_lsp_engine_selection`

## 8. Public docs now describe the actual G4 behavior

`#464` updated `README.md` and `README_ja.md` so the public docs now reflect that:

- `--with-pdg` remains mostly file-local
- `--with-propagation` can widen Rust/Ruby local DFG scope to direct boundary files for short bridges
- `impacted_witnesses` now include `path`, `provenance_chain`, and `kind_chain`
- diff-mode changed-symbol discovery now honors the selected engine
- the path is still not project-wide and still does not preserve full engine-native edge richness

That doc update matters because the implementation is now a bit better than G3, but still far from “whole-program PDG”, and users need the README to say that plainly.

---

## Improvement cases G4 materially helped

The clearest practical wins from G4 are:

### 1. Rust cross-file call-site summary bridge

Improved from:

- single-file propagation bridge working
- equivalent cross-file case falling back toward plain call edges

to:

- propagation recovering the short cross-file bridge by widening local DFG scope to the direct callee-side file

### 2. Rust multi-file wrapper / return-flow fixture coverage

Improved from:

- wrapper / assigned-result multi-file behavior only being a design target in the eval-set note

to:

- a locked CLI regression surface that distinguishes plain PDG vs propagation and checks for no irrelevant bridge leakage

### 3. Ruby split `require_relative` alias / return-flow coverage

Improved from:

- Ruby local chain behavior mostly being exercised in single-file fixtures

to:

- explicit multi-file CLI coverage for propagation-only symbolic edges across split files

### 4. Explainability of multi-file impact results

Improved from:

- one minimal last-hop witness edge

to:

- a selected hop-by-hop witness path with provenance/kind summary

### 5. Engine semantics in diff-mode PDG

Improved from:

- PDG diff mode silently using a TS-only changed-symbol path

to:

- PDG diff mode honoring the selected engine for changed-symbol discovery

---

## What G4 did **not** solve

G4 was useful, but it left several important boundaries in place.

## 1. There is still no first-class scope planner object in the implementation

G4-3 defined the desired policy, but the code still does not expose a proper

- `PdgBuildScopePlan`
- explicit reason tracking per selected path
- budgeted Tier 0 / Tier 1 / Tier 2 planning object

What landed instead was a narrower behavior change:

- propagation can add direct boundary files in some Rust/Ruby diff/seed flows

That is a good step, but it is not the same thing as the full planner described in the design memo.

## 2. `--with-pdg` alone is still mostly file-local

The widened scope landed in propagation mode, not as a general PDG-scope planner.

So the current state is still:

- `--with-pdg`: mostly changed/seed file local DFG
- `--with-propagation`: may widen to direct adjacent boundary files for short bridges

This means G4 improved the more aggressive lane first, not the base PDG lane.

## 3. Bridge-completion / 2-hop scope policy is still not implemented as policy

G4-3 proposed:

- 1-hop direct boundary expansion
- one extra bridge-completion file when needed

The code landed the first half in limited form.
The second half is still largely a design target rather than a reusable implementation policy.

## 4. Base graph semantics are still coarse at the PDG entrance

The cache-backed PDG entry path still tends to flatten base refs toward coarse call-graph semantics.

So even after G4:

- engine-native richness is not fully preserved into the PDG build stage
- cache-derived base edges are still a limiting factor
- scope improvements and engine improvements are still not fully separable in the implementation structure

## 5. Witnesses are better, but still intentionally minimal

The new witness path summary is useful, but the current implementation still does not provide:

- multiple competing witness paths
- user-visible alternative routes
- a stable explanation for “why this path won over another candidate”
- richer scope-reason output tied directly to witness paths

## 6. Multi-language parity is still out of scope

Rust/Ruby remain the places where local DFG augmentation adds the most real value.

For Go / Java / Python / JS / TS / TSX:

- engine work may still improve the normal impact path
- but the current PDG / propagation lane should still be treated as experimental unless a fixture/regression says otherwise

---

## Recommended G5 candidates

If G4 was about getting a credible multi-file foothold, G5 should focus on turning that foothold into a more explicit internal contract.

### 1. Implement the real scope planner from G4-3

Priority: **highest**

Specifically:

- introduce a first-class `PdgBuildScopePlan`
- separate `cache_update_paths` from `local_dfg_paths`
- keep reason tags such as `seed_file`, `changed_file`, `direct_callee_file`, `direct_caller_file`, `bridge_completion_file`
- make the plan deterministic and fixture-testable

This is the missing structural piece behind most remaining scope work.

### 2. Add bridge-completion / 2-hop policy deliberately, not accidentally

Priority: **high**

The next concrete behavior step should probably be one of:

- wrapper-return completion (`main -> adapter -> core` style)
- imported-result alias continuation across files

But it should land through an explicit scope rule, not by piling more ad-hoc path growth into `build_pdg_context()`.

### 3. Preserve richer base-edge semantics into PDG entry

Priority: **high**

G4 improved changed-symbol engine alignment, but the bigger engine-consistency gap is still base-graph richness.

G5 should explore:

- how much certainty / provenance / engine-origin metadata can survive cache load
- whether PDG entry can distinguish cache-coarse refs from engine-richer refs
- how to compare TS/LSP differences without flattening them at the first merge boundary

### 4. Expose scope reasons and witness-path explanations more directly

Priority: **medium**

The current witness path is helpful, but G5 could make debugging substantially easier by adding:

- optional scope-reason logging / JSON debug output
- a way to see which file entered local DFG scope for which seed
- better linkage between witness paths and the scope expansion that made them possible

### 5. Decide whether `--with-pdg` should inherit some of the new scope behavior

Priority: **medium**

Right now the more advanced scope expansion landed only in propagation mode.
G5 should make that asymmetry a conscious product decision:

- either keep `--with-pdg` deliberately more local
- or give it some bounded multi-file scope widening too

### 6. Expand engine baselines beyond the one fixed diff-mode mismatch

Priority: **medium**

G4-8 fixed a real mismatch.
G5 should make that work systematic by adding more baseline cases for:

- normal impact vs PDG mode
- TS vs LSP vs auto(strict-if-available)
- diff mode vs seed mode

so future engine-integration changes can be measured instead of guessed.

---

## One-sentence summary

G4 did **not** deliver a project-wide PDG, but it did deliver a much more honest and useful intermediate state: **multi-file propagation now has a real foothold, witness output is more explainable, one major engine mismatch is gone, and the remaining work is finally split into explicit scope, merge, and engine-consistency layers instead of one blurry “PDG needs to be smarter” bucket.**
