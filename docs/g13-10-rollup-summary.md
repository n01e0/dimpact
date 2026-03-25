# G13 rollup: stitched-chain ranking, bounded budgeting, winning-chain provenance

## What landed

This document closes out G13 work around post-G12 stitched continuation handling.

G13 was not about making the planner search farther.
The main goal was narrower and more operational:

**take the short stitched chains that G12 made visible, keep the bounded planner model, and make ranking / budgeting / provenance better at choosing and explaining the right winner.**

That work landed in small, reviewable slices:

- first inventory the new failure surface after G12
- then define a chain-first ranking / budget vocabulary
- then improve one Rust mis-selection and one Ruby mis-selection
- then apply the smallest family-aware continuation budget that still preserves boundedness
- then move witness provenance toward a winning-chain-first view
- finally lock the new behavior with regressions and docs

### Merged PRs

- `#547` docs: add G13 stitched continuation ranking memo
- `#548` docs: define G13 stitched-chain ranking schema
- `#549` Prefer semantic alias closers in stitched ranking
- `#550` Prefer Ruby alias stitched closers over return siblings
- `#551` Add per-bridge tier3 continuation budget
- `#552` Prefer winning bridge chains in witness provenance
- `#553` Add stitched-chain PDG and propagation regressions
- `#554` Lock planner and stitched ranking alignment
- `#555` docs: explain G13 stitched witness output

## What changed in practice

## 1. G13 fixed the problem statement after G12

G12 made bounded continuation/stitching noticeably stronger, especially for:

- short wrapper-return continuation
- short Ruby `require_relative` continuation
- alias-result stitching
- selected nested multi-input continuation

After that, the main issue was no longer just "can the scope reach one more file?"
It became:

- which stitched chain should represent the bounded winner
- which weaker / duplicate chain should stay pruned
- how should the output explain the winner without pretending every visible stitched step is part of one mixed route

That framing was captured in:

- `docs/g13-1-stitched-continuation-ranking-memo.md`
- `docs/g13-2-stitched-chain-ranking-budget-schema.md`
- `docs/g13-2-stitched-chain-ranking-budget-schema.json`

The most important mental shift is:

- G12: broaden short bounded continuation coverage
- G13: improve **selection, budget, and explanation** inside that bounded coverage

## 2. A chain-first ranking vocabulary now exists

G13 did not fully replace the file-local planner with a new chain object.
But it did establish the intended vocabulary for doing so.

The schema work defines a stitched-chain model around:

- chain family
- step families
- closure quality
- duplicate / overreach penalties
- family-aware budgets
- winning-chain-first provenance

In other words, the project now has a written target for how to evaluate:

- return continuation vs alias-result stitch
- require-relative support vs actual winner
- same-path duplicate vs same-chain duplicate
- file-local evidence vs actual stitched closure quality

That matters because future changes can now be judged against a stable G13 contract instead of ad-hoc heuristic drift.

## 3. One real Rust mis-selection family improved

G13 shipped a concrete Rust-side ranking improvement:

- semantic alias stitched closers are now allowed to beat purely return-looking siblings
- alias continuations keep semantic param-to-return support instead of being flattened into a weaker lexical tie

This is the practical result of `#549`.

The important outcome is not just "one test got greener".
It is that the planner is now a little less likely to prefer:

- a return-looking helper with shallow lexical cues

over:

- an alias/result continuation with stronger local semantic closure signals

That is exactly the kind of post-G12 mis-selection G13 set out to reduce.

## 4. One real Ruby mis-selection family improved

G13 also shipped a Ruby-side improvement:

- alias/result stitched candidates can beat return-looking required siblings when the alias/result signals are stronger
- witness family selection keeps `alias_result_stitch` as the winner even when `require_relative` support is also present on the path

This is the main result of `#550`.

Operationally, this means short Ruby stitched cases are less likely to be over-described as:

- pure return continuation
- or an undifferentiated require-relative/mixed union

when the real winner is an alias/result chain.

## 5. Tier-3 continuation budgeting is now less self-sabotaging

Before G13, tier-3 continuation admission effectively treated all continuation bridges as one tiny global bucket.
That made the bounded planner brittle in a familiar way:

- one continuation family could consume the only slot
- another distinct family would disappear before users could even compare the result

`#551` introduced the smallest useful correction:

- keep a small total tier-3 seed budget
- add a per-bridge-kind cap

Current effect:

- distinct continuation bridge families can coexist inside the same bounded seed slice
- second candidates from the same bridge family still stay budget-pruned
- boundedness remains explicit instead of silently widening

This is not a full chain-budget system yet.
But it is enough to stop collapsing all stitched continuation families into one winner-take-all slot.

## 6. Witness provenance now prefers the winning chain over the stitched-step union

This is the most visible G13 output change.

Before G13, bridge execution provenance was closer to:

- "all stitched-looking steps seen on the selected path"

than to:

- "the stitched chain that actually won"

`#552` changed that by splitting provenance into:

- `bridge_execution_family`
- `bridge_execution_chain_compact`
- `winning_bridge_execution_chain_compact`
- `observed_supporting_steps_compact`

Current reading model:

- `summary.slice_selection` answers why a bounded file was kept
- `winning_bridge_execution_chain_compact` answers which stitched chain actually won
- `observed_supporting_steps_compact` answers which extra stitched evidence was present but not winner-defining

This is the clearest G13 shift from "mixed stitched union" toward "winner plus support".

## 7. Planner/witness alignment is now explicitly locked

G13 did not stop at output cleanup.
It also added regression coverage to make sure the planner-side selected reason and the witness-side winning chain stay aligned.

The most important locked expectations are now:

- if a bounded file enters scope through `bridge_completion_file` or `bridge_continuation_file`, the winning stitched chain should point back to the same anchor/path/bridge family
- if a support-only `require_relative` step exists, it should stay visible as support without replacing the winner

This was covered by:

- `#553` stitched-chain PDG / propagation regressions
- `#554` planner vs stitched-ranking alignment regressions

That gives G13 a better answer to: "did the planner choose one story while the witness told another?"

## 8. README guidance now matches the G13 output model

`#555` updated `README.md` and `README_ja.md` so users have a stable reading order for the new stitched output.

The docs now explicitly explain:

- what `summary.slice_selection` means
- what `bridge_execution_family` means
- why `bridge_execution_chain_compact` is now winner-oriented
- when to read `winning_bridge_execution_chain_compact`
- when to read `observed_supporting_steps_compact`

This matters because G13 changed how the output should be interpreted, not just how the planner scores candidates internally.

## Improvement areas G13 materially helped

The clearest concrete wins from G13 are:

### 1. Rust alias/result closer vs return-looking sibling

Improved from:

- lexical return-ish candidate winning too easily

To:

- semantic alias stitched closer can win when its local closure signal is stronger

### 2. Ruby alias/result closer vs return-looking required sibling

Improved from:

- Ruby alias/result winner being flattened or mis-labeled by nearby return / require-relative structure

To:

- alias/result winner surviving as the selected stitched family while require-relative remains supporting context

### 3. Tier-3 continuation family coexistence

Improved from:

- all continuation bridge families sharing one tiny slot

To:

- small bounded total budget plus per-bridge-family survival

### 4. Witness explainability

Improved from:

- stitched-step union that could overstate mixed behavior

To:

- winner-oriented compact chain plus supporting observed steps

### 5. Planner/output consistency

Improved from:

- planner selected reasons and witness compact chain being easy to mentally disconnect

To:

- explicit regression locks that tie bounded admission and winning-chain provenance together

## What G13 intentionally did not solve

G13 was successful, but it remained intentionally bounded.

It did **not** attempt:

- project-wide recursive continuation search
- arbitrary-depth stitched closure across many files
- full same-chain duplicate reasoning across all families
- full expression normalization / argument binding generalization
- whole-language parity outside the current Rust/Ruby-heavy stitched cases
- a full replacement of file-local ranking with a first-class `StitchedChainCandidate` runtime object

Those non-goals were deliberate.
G13 aimed for better precision and explanation without breaking the bounded planner model.

## Remaining constraints after G13

Even after G13, several constraints remain visible.

### 1. The planner is still fundamentally file-first

The written G13 schema is more chain-first than the current runtime implementation.

Current reality:

- planner admission and most comparisons are still file candidate based
- chain vocabulary is strongest in docs and witness output
- runtime compare/budget logic only partially approximates chain-first ranking

This is acceptable for G13, but it is still the main architectural gap.

### 2. Per-bridge-kind budget is only a proxy for per-chain/per-family budget

The G13 budget implementation uses bridge kind as the smallest practical family proxy.
That is useful, but still coarse.

Examples of what it does **not** fully solve:

- same semantic chain represented through slightly different localities
- multiple distinct chains that happen to share one bridge kind
- budgeting by winner chain quality instead of bridge bucket

### 3. Same-chain duplicate suppression is still incomplete

G13 improved same-family and per-bridge handling, but it did not yet add a strong runtime notion of:

- duplicate chain key
- merged same-chain duplicate vs weaker same-family sibling

This means path-local and family-local suppression improved more than true chain-level deduplication.

### 4. Provenance is now winner-oriented, but still compact and representative

The new witness fields are much easier to read, but they are still not a full execution proof.

They still represent:

- one selected witness route
- one winning stitched explanation
- supporting observed stitched steps

They do **not** enumerate all viable competing chains or prove global optimality.

### 5. Ruby remains intentionally narrow

Ruby short-chain behavior is meaningfully better than before, but still intentionally narrow around:

- longer `require_relative` ladders
- dynamic-send-heavy flows
- broader runtime companion discovery
- richer mixed-family ranking across several adjacent candidates

## Good next-stage candidates after G13

If there is a G14 or equivalent follow-up phase, the most natural next candidates are:

### 1. Introduce a real runtime stitched-chain representative

Implement a small internal object closer to the G13 schema, so ranking/budgeting can operate on:

- chain family
- closure quality
- duplicate-chain key
- overreach penalty

instead of only file-level candidates with partial chain hints.

This is the cleanest next architectural step.

### 2. Add true same-chain duplicate suppression

Introduce a duplicate key that can detect when multiple candidates are really describing the same stitched closure.

That would let the planner distinguish:

- same-path duplicate
- same-family sibling
- same-chain duplicate

instead of compressing all of them into file/local-family heuristics.

### 3. Refine budget from per-bridge-kind to per-chain/per-family representative

The current G13 budget is a good minimal step, but the next precision gain likely comes from budgeting:

- one representative per family or per chain key
- then one small final winner/alternate cap per seed

That would better match the G13 schema without forcing broad planner expansion.

### 4. Improve selected-vs-pruned chain explanations

G13 improved output fields, but chain-level loser explanation is still thinner than file-level prune reporting.

A next phase could make it easier to answer:

- which chain lost
- why it lost
- whether it lost on closure quality, duplicate suppression, or budget pressure

### 5. Expand regression fixtures around mixed-family and duplicate-locality cases

The current G13 regressions are good, but still focused on a few strong representative cases.

Natural next additions:

- same semantic chain represented from two nearby files/localities
- mixed Ruby chain where require-relative support should stay support-only across more than one candidate
- nested multi-input cases where closure quality should beat lexical local evidence

## Why this closes G13

G13 was about turning post-G12 stitched continuation from a "we can sometimes see the right extra file" story into a more disciplined:

- choose the better stitched winner
- keep bounded family-aware budgets
- explain the winning chain more honestly
- lock the planner/witness contract with regressions

That goal is now met.

What remains after G13 is not foundational uncertainty.
It is the next refinement layer:

- make runtime ranking more truly chain-first
- make duplicate/budget logic more truly chain-aware
- widen only where regressions show bounded precision can stay under control

So G13 closes as a successful precision-and-explainability phase for stitched continuation ranking under the bounded planner model.
