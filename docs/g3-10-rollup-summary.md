# G3 rollup: PDG / propagation integration, guardrails, and next steps

## What landed

This document closes out the G3 work around `impact --with-pdg` / `--with-propagation`.

G3 was not just about "making PDG smarter".
The harder goal was:

**take a partly experimental local PDG / propagation path, make its behavior more explicit, improve at least one real weak dependency path, and bring the results closer to the main impact output / CLI contract.**

The work landed in small reviewable slices:

- first fix the implementation boundary and evaluation surface
- then improve one weak dependency path and tighten propagation precision
- then make PDG results carry more real metadata
- then lock regressions and remove one obvious CLI mismatch (`--per-seed`)
- finally document intended usage and current limits

### Merged PRs

- `#446` docs: inventory PDG integration boundaries
- `#447` docs: fix PDG evaluation set
- `#448` fix(pdg): keep same-line param flow in summaries
- `#449` fix(pdg): tighten propagation summary bridges
- `#450` fix(pdg): preserve merged edge metadata
- `#451` feat(impact): add per-symbol witness metadata
- `#452` test(pdg): fix CLI regression fixtures
- `#453` feat(pdg): support per-seed impact output
- `#454` docs: clarify PDG usage and limits

## What changed in practice

## 1. The PDG path now has a written contract

G3 started by fixing the vocabulary around the current implementation.

The important conclusion was:

- current PDG is **not** a project-wide PDG
- it is better described as **global call graph + local DFG augmentation**
- propagation is mostly **call-site / summary-oriented bridging**, not whole-program symbolic propagation

Relevant docs:

- `docs/g3-1-pdg-propagation-boundary-inventory.md`
- `docs/g3-2-pdg-eval-set.md`
- `docs/g3-2-pdg-eval-set.json`

That matters because later changes can now be judged against a stable model instead of hand-wavy expectations.

## 2. A fixed G3 evaluation surface now exists

G3 locked a compact regression set around five cases:

- Rust call-site summary bridge
- Rust alias / reassignment latest-def behavior
- Ruby callees-chain alias / return-flow case
- Ruby alias + `define_method` no-leak case
- Ruby `send` / `public_send` target-separation case

This gave G3 a stable answer to:

- what should improve
- what must not regress
- which languages are actually in scope for the current PDG path

The evaluation surface intentionally stayed Rust/Ruby-heavy because that is where the current local DFG path actually adds information.

## 3. One real weak dependency area was improved

G3 shipped two concrete behavior changes in the PDG / propagation core.

### 3.1 Same-line parameter flow no longer disappears from summaries

Single-line Rust callees such as `fn callee(a: i32) -> i32 { a + 1 }` could lose the parameter-use flow that summary bridging needed.

G3 fixed this so that:

- same-line parameter defs remain visible to summary building
- propagation can still emit the direct bridge from call-site arg use to assigned result def

Relevant PR / coverage:

- `#448`
- `tests/cli_pdg_propagation.rs`
- `src/dfg.rs` unit coverage

### 3.2 Propagation summary bridges are less eager to smear across arguments

The original propagation bridge logic could be too broad, especially around multi-arg calls.

G3 tightened this so that:

- summary inputs keep their order
- call-site uses map to summary inputs by position where possible
- irrelevant earlier args are less likely to leak into later impacted defs

Relevant PR / coverage:

- `#449`
- unit + CLI regressions for retained direct bridge and no-leak two-arg behavior

## 4. PDG results now carry more real metadata into main impact output

Before G3, the PDG path effectively threw away too much information when collapsing back into `Reference`.

G3 improved this in two layers.

### 4.1 Edge kind / certainty / provenance are now preserved more honestly

The merged PDG path now distinguishes:

- `call`
- `data`
- `control`

and attaches provenance such as:

- `call_graph`
- `local_dfg`
- `symbolic_propagation`

This is the key result of `#450`.

Net effect:

- the main impact result is less misleading
- `--with-edges` output better reflects why an edge exists
- PDG-enriched traversal is no longer forced to pretend every edge is just a plain call edge

### 4.2 Impacted symbols can now carry one minimal witness

`#451` added per-symbol witness metadata via `impacted_witnesses`.

Each impacted symbol can now point back to:

- the originating changed root
- the immediate predecessor symbol (`via_symbol_id`)
- the last-hop witness edge, including kind / certainty / provenance
- the shortest-path depth used for that witness

This is intentionally small, but it is enough to make PDG / propagation results more explainable than before.

## 5. The CLI contract is less inconsistent now

Two user-facing mismatches were cleaned up.

### 5.1 PDG / propagation regression coverage is now fixed in CLI tests

`#452` expanded `tests/cli_pdg_propagation.rs` so the G3 evaluation set is no longer just a note.

Locked areas now include:

- Rust alias / reassignment latest-def behavior
- Ruby chain improvement case
- Ruby no-leak dynamic alias / `define_method`
- Ruby dynamic target separation under propagation

This means the project now has actual fixture-backed guardrails for both:

- intended improvement cases
- explicit non-regression cases

### 5.2 `--per-seed` now works with PDG / propagation

At G3 start, `--per-seed` hard-failed with PDG / propagation.

`#453` removed that mismatch and made grouped output work for:

- diff-based PDG / propagation runs
- explicit-seed PDG / propagation runs

This matters because the PDG path now fits better into the same operational workflow as normal `impact`.
It is still not perfectly unified internally, but the CLI story is much less awkward.

## 6. README guidance now matches the implementation more closely

`#454` updated `README.md` and `README_ja.md` to explain:

- when to prefer normal impact vs `--with-pdg` vs `--with-propagation`
- that the current PDG path is local in scope
- that Rust/Ruby are the main languages where it currently adds value
- that `-f dot` changes meaning when PDG mode is enabled
- that `--per-seed` is part of the supported usage pattern

This is important because the old docs could easily overstate what PDG meant.

## Improvement cases G3 materially helped

The clearest concrete wins from G3 are:

### 1. Rust call-site summary bridge

Improved from:
- same-line callee parameter flow sometimes falling out of summaries

to:
- direct propagation bridge can survive through the summary path

### 2. Rust alias / reassignment latest-def behavior

Locked and protected so that:
- latest def is preferred over stale reassignment sources
- alias-chain behavior remains visible in PDG / propagation DOT regressions

### 3. Ruby dynamic no-leak guard cases

Locked so that propagation improvements do **not** casually widen into:
- unrelated `define_method` targets
- merged `send` / `public_send` targets that should stay separated

### 4. Explainability of PDG-enriched impact results

Improved from:
- almost everything flattening into generic `call/inferred` edges

to:
- edge kind + provenance preserved in output
- impacted symbols carrying one minimal witness edge

## Remaining limits

G3 improved the current path quite a bit, but it did **not** erase the architectural limits identified in G3-1.

### 1. PDG is still local, not project-wide

Current scope is still:

- changed files in diff mode
- seed files in seed mode
- Rust/Ruby for actual local DFG construction

Implications:

- cross-file summary growth is still limited
- non-Rust/Ruby languages still get much less PDG-specific value
- many project-wide data-flow expectations will still not hold

### 2. Engine integration is still partial

Normal impact still flows through the engine abstraction more cleanly than PDG / propagation does.

Implications:

- future LSP richness is not automatically inherited by PDG mode
- PDG path remains a distinct maintenance surface inside `src/bin/dimpact.rs`

### 3. Witnesses are intentionally minimal

`impacted_witnesses` is useful, but it is still only a last-hop explanation.

It does **not** yet provide:

- a full witness path
- multiple competing witnesses
- rich provenance labels like “summary bridge” vs “call-site bridge” vs “alias bridge”
- first-class human-readable narrative for why a symbol was reached

### 4. DOT semantics are still split

Today:

- normal `impact -f dot` means impact graph
- `impact --with-pdg -f dot` means raw PDG/DFG-style graph

That split is documented now, but it is still conceptually awkward.

### 5. Language coverage is still uneven by design

The current Rust/Ruby emphasis is honest, but it means G3 did not yet solve:

- Go / Java / Python / JS / TS / TSX local DFG parity
- multi-language propagation expectations
- semantic consistency of PDG options across all supported languages

## Why G3 can be considered complete

G3 was supposed to do three things:

1. make the current PDG / propagation boundary explicit
2. ship at least one real behavior improvement plus non-regression coverage
3. reduce the gap between PDG-path internals and the main impact output / CLI contract

That now exists.

Specifically:

- boundary and evaluation docs are written
- a fixed regression surface exists
- summary bridge precision improved
- edge metadata is preserved more honestly
- impacted-symbol witness metadata exists
- `--per-seed` works instead of bailing
- README guidance reflects the real scope and tradeoffs

So after G3, PDG / propagation is still not a full second analysis engine — but it is much less of an opaque side path.
It is now easier to evaluate, easier to trust in its intended scope, and easier to explain when it succeeds or fails.

## Next sensible moves after G3

If there is a G4 for this area, the most sensible order looks like this.

### 1. Project-wide scope before smarter heuristics everywhere

The biggest remaining limitation is still local scope.
Before adding many more clever bridge rules, the project should decide whether to:

- keep PDG intentionally local forever
- or move toward project-wide / multi-file DFG coverage for a smaller language set first

Without that decision, later improvements risk making the local path denser without making it broader.

### 2. Separate provenance classes more explicitly

Current provenance is already much better than before, but still fairly coarse.
A next step could distinguish:

- call graph edge
- local DFG edge
- call-site bridge
- summary bridge
- alias-derived bridge
- control-derived bridge

That would make witnesses and regression expectations substantially clearer.

### 3. Consider full-path witnesses only if they stay cheap and stable

A natural next ask is "show me the full path, not just one witness edge".
That is attractive, but it should only land if:

- output stays stable enough for fixtures
- filtered / per-seed / both-direction output remains readable
- path selection rules stay explainable

### 4. Revisit DOT / HTML semantics

The current split between impact graph DOT and raw PDG DOT is defensible, but not ideal.
A future pass could decide whether:

- raw PDG should move to a separate mode
- or impact DOT should gain an explicit PDG-enriched view without switching meaning silently

### 5. Grow the regression set conservatively

The five-case G3 set is good because it matches the current implementation boundary.
If the scope widens, new cases should be added deliberately rather than replacing the existing set too early.

## Short version

G3 did not turn PDG into a project-wide semantic graph engine.
What it did do is more practical:

- make the current boundary explicit
- improve one real weak dependency path
- tighten one real over-propagation case
- preserve edge metadata in main output
- add minimal witness support
- lock regressions around both improvements and no-leak constraints
- remove the `--per-seed` CLI mismatch
- document how to actually use the feature without overselling it

That is a solid close for this phase, and a much better base for any future PDG work than the project had at G3 start.
