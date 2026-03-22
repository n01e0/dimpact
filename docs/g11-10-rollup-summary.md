# G11 rollup: bounded continuation slices, short multi-file Rust/Ruby bridge gains, tighter propagation attachment, and clearer remaining gaps

## What landed

This document closes out the main G11 work around bounded PDG scope expansion.

G10 tightened admission and loser bookkeeping inside the bounded planner.
G11 was about using that bounded planner to recover **one more step of real multi-file continuity** without breaking the non-project-wide contract.

The practical goal was:

**keep the planner bounded, inventory the real current limits, define a one-step continuation policy, land at least one real Rust and one real Ruby multi-file improvement, make propagation attach more naturally to the bounded PDG edges, lock the new behavior with regressions, and update the public docs so the mental model matches runtime reality.**

In practice, G11 landed in ten layers:

- first, inventory the current PDG scope / bridge limits against main instead of the older G3/G4 baseline
- next, define a fixed failure eval set for the still-missing multi-file PDG cases
- then, write the bounded one-step scope-expansion policy
- next, land one real Rust multi-file wrapper-return improvement
- then, land one real Ruby `require_relative` / wrapper-return improvement
- next, relax propagation attachment so PDG-backed bridges connect more naturally at call sites
- then, add regression coverage for the new multi-file behavior
- next, align the bounded planner metadata with the expanded PDG surface
- then, update README / README_ja so the public explanation matches G11 runtime behavior
- finally, summarize what G11 actually changed and identify the next-phase candidates

## Merged PRs

- `#526` docs: add G11 PDG scope/bridge expansion memo
- `#527` docs: add G11 PDG failure eval set
- `#528` docs: add G11 bounded scope expansion policy
- `#529` pdg: extend rust two-hop wrapper return propagation
- `#530` pdg: extend ruby require_relative return bridging
- `#531` pdg: relax propagation callsite line attachment
- `#532` test: lock G11 multi-file PDG regressions
- `#533` pdg: label continuation slice reasons explicitly
- `#534` docs: refresh G11 PDG scope notes

---

## What changed in practice

## 1. G11 re-baselined the PDG discussion around current main

`#526` matters because the old language for PDG scope was too stale.
By the time G11 started, the runtime was already beyond the simple:

- changed/seed file only
- local DFG only
- one generic propagation pass

story.

The G11 inventory memo made the current baseline explicit:

- bounded slice planning already existed
- direct boundary files were already first-class
- bridge-completion files already existed as a bounded second hop
- one-hop symbolic completion bridges already existed
- selection/pruning explanations were already visible in JSON/witness surfaces

That re-baseline was necessary because G11 was **not** about inventing bounded planning from scratch.
It was about deciding how far that bounded planning could be pushed without turning into project-wide PDG.

## 2. G11 fixed the target around real current failures, not historical ones

`#527` gave G11 a stronger contract than “multi-file is weak somewhere.”
It locked five current-main failure families, including:

- Rust two-hop wrapper-return continuation
- nested multi-input continuation weakness
- cross-file wrapper-return alias stitching gaps
- Ruby two-hop `require_relative` continuation
- bridge-budget overflow under bounded selection

That matters because the main missing problems were no longer just:

> can the planner include one more file?

They had become more specific:

- where does bounded scope still stop too early?
- where does propagation fail even when scope is already enough?
- where does budget policy distort an otherwise valid bounded slice?

This gave G11 a clearer split between:

- scope problems
- propagation problems
- metadata/explanation problems

## 3. G11 defined a bounded continuation policy instead of chasing broad expansion

`#528` is the design center of the phase.
It defined the planner extension as:

- keep the current root/direct-boundary/bridge-completion skeleton
- add only one new tier conceptually: `BridgeContinuationFile`
- derive it only from **already admitted** bridge-completion anchors
- keep the same bridge family while extending
- stop after one extra hop

That is the most important conceptual outcome of G11.
The phase did **not** choose project-wide PDG or recursive slice growth.
It chose:

> a bounded continuation step

This is a much cleaner next-step model because it preserves the key bounded invariants:

- limited depth
- explicit selection reasons
- small per-seed budgets
- explainable loser bookkeeping

## 4. Rust now handles one real two-hop wrapper-return case that used to fall short

`#529` is the clearest Rust proof point.
It improved the short multi-file shape:

- `main -> wrap -> step -> leaf`

Before G11, Rust could often reach the middle file but still fail to recover the final caller-side continuation naturally.
After `#529`:

- the bounded planner can keep the needed continuation file in scope
- propagation can recurse one more nested completion step
- the caller-side result bridge closes more naturally in the short two-hop wrapper-return case

This is a real recall gain, not just a metadata change.
It proves the bounded continuation policy can buy a real multi-file improvement without opening the search globally.

## 5. Ruby now has one matching two-hop `require_relative` / wrapper-return proof point

`#530` is the Ruby-side companion win.
It applied the same general G11 direction to a Ruby shape where short multi-file continuity mattered:

- `main -> wrap -> step -> leaf`
- split by `require_relative`

The important runtime gain is not “Ruby became broadly solved.”
It is narrower and more valuable than that:

- bounded continuation can now keep the relevant downstream file in scope
- propagation can still carry a short return-oriented bridge even when Ruby callsite capture is weaker than the Rust case
- no-paren / short wrapper-return style flows are materially better than before

This is the right kind of Ruby win for G11:

- concrete
- bounded
- regression-backed
- still honest about the remaining limits

## 6. Propagation attaches to PDG-backed call sites more naturally now

`#531` is the attachment improvement.
Before it, propagation still depended too much on exact `(file, line)` matches at the callsite boundary.
That made short multiline calls unnecessarily brittle.

After `#531`:

- propagation can search a small nearby-line window for caller-side use/def nodes
- multiline wrapper calls attach more naturally to the callee summary/bridge
- short PDG-backed propagation no longer depends as strictly on exact line coincidence

This is strategically important because better bounded scope alone was not enough.
If propagation could not attach to the already-selected PDG nodes, the new scope would still feel brittle.

G11 therefore improved both:

- what files get selected
- how the selected callsite nodes connect into symbolic propagation

## 7. The new behavior is now locked by direct regression coverage

`#532` matters because G11 would be hard to trust without compact regression coverage for the new shapes.
The new tests lock:

- Rust two-hop wrapper-return in PDG/propagation paths
- Ruby two-hop `require_relative` propagation behavior
- compact per-seed witness path expectations
- selection attribution for continuation-tier files

This is the right regression surface for G11.
The point was not only “an extra file appears.”
The point was also:

- the witness stays compact
- the selected path remains understandable
- the continuation file is attributed correctly in per-seed output

## 8. Planner metadata and expanded PDG are now better aligned

`#533` is small but important.
Once G11 added the conceptual continuation tier, leaving those files labeled as generic `bridge_completion_file` was starting to blur the runtime story.

After `#533`:

- continuation-tier slice reasons are labeled separately as `bridge_continuation_file`
- selected-vs-pruned reasoning can match continuation-tier losers against continuation-tier winners
- JSON / witness surfaces show the G11 scope extension more honestly

This is the correct kind of cleanup for the phase.
It does not widen behavior.
It makes the merged planner/PDG story internally consistent.

## 9. README / README_ja now describe the actual G11 runtime model

`#534` updated the public docs so they stop describing the PDG path like an older “controlled 2-hop” approximation.

The important public wording shifts are:

- bounded continuation is now the scope model
- short Rust/Ruby multi-file improvements are called out explicitly
- `bridge_continuation_file` is documented as visible metadata
- multiline callsite attachment is documented as improved but still narrow
- the docs state more clearly what still does **not** exist:
  - no project-wide PDG
  - no true cross-file DFG
  - no broad symbolic execution

That matters because G11 changed the mental model more than the command surface.
Without these doc updates, the public story would under-describe the runtime in exactly the place G11 worked on.

---

## Improvement cases G11 materially helped

The clearest practical wins from G11 are:

### 1. Rust short two-hop wrapper-return continuation

Improved from:

- boundary and mid-hop selection existing, but caller-side return closure still falling short in a short `main -> wrap -> step -> leaf` shape

To:

- continuation file admitted under bounded scope
- nested completion bridge recursed one more step
- caller-side return/result continuity recovered in propagation output

Locked surface:

- `tests/cli_pdg_propagation.rs`
- per-seed compact witness expectations
- continuation-tier selection metadata

### 2. Ruby short two-hop `require_relative` wrapper-return continuation

Improved from:

- short downstream file selection and continuation being too weak or brittle

To:

- bounded continuation file retained for the Ruby path
- propagation capable of carrying the short return-oriented flow back to the caller-side def
- regression-backed witness/path coverage

Locked surface:

- two-hop Ruby propagation regression
- compact per-seed witness expectations
- continuation-tier slice reasons visible in JSON

### 3. Multiline callsite attachment for short wrapper flows

Improved from:

- exact-line callsite matching making short multiline calls brittle

To:

- nearby-line tolerance for callsite use/def pickup
- more natural propagation attachment for split-line wrapper calls

Locked surface:

- dedicated multiline Rust wrapper regression

### 4. Planner/PDG metadata coherence for continuation-tier files

Improved from:

- G11 continuation behavior existing, but still labeled like ordinary bridge-completion selection

To:

- explicit `bridge_continuation_file` labeling
- better selected-vs-pruned continuation reasoning
- public JSON/witness surfaces aligned with the actual bounded planner shape

---

## What G11 did **not** do

This is just as important as what it landed.

G11 did **not** turn the PDG path into:

- a project-wide PDG
- a recursive whole-repo slice planner
- a true cross-file DFG
- a broad symbolic executor
- a full parity implementation across Go/Java/Python/JS/TS/TSX

It also did **not** fully solve the harder remaining families from the eval set, especially:

- nested multi-input continuation mapping
- broader alias/result stitching across files
- richer budget policy than the current small bounded caps
- deeper `require_relative` ladders or dynamic-send-heavy Ruby flows

That is a healthy outcome.
G11’s value is that it moved one bounded step forward without pretending to have solved the broader problem class.

---

## Remaining gaps after G11

The most important remaining gaps are now clearer than before.

## 1. Multi-input continuation is still the biggest propagation gap

The strongest next technical target is still:

- multi-input nested summary mapping
- reordered or selective argument continuation
- preserving the relevant arg across nested wrapper chains without over-propagating unrelated inputs

G11 mostly improved the **single-input / short wrapper-return** side of the space.
The multi-input case remains the best next propagation target.

## 2. Cross-file alias/result stitching is still narrower than wrapper-return continuation

G11 helped wrapper-return continuation more than general alias stitching.
There is still a gap around:

- imported result -> local temp -> alias -> caller result
- cross-file return stitching when the local chain is not just a simple wrapper-return

That means alias-family work is still a distinct next step, not automatically solved by bounded continuation.

## 3. Budget policy is still coarse

The planner is better, but the budget contract is still intentionally small and somewhat blunt.
The eval-set budget-overflow case remains relevant.

Future work likely needs to look at:

- per-seed vs per-anchor continuation budget
- family-aware budget allocation
- when a third valid continuation should beat a weaker earlier survivor

This is planner work, not propagation work.
It deserves its own phase instead of being mixed into the current G11 wins.

## 4. Explanation is better, but bridge execution provenance can still improve

G11 made file-selection provenance clearer, especially with `bridge_continuation_file`.
But explanation is still stronger for:

- why this file was selected

than for:

- why this exact symbolic bridge chain was the one that closed the result

A future phase could make bridge-family execution provenance more explicit in witness or summary output without bloating the public surface too much.

## 5. Non-Rust/Ruby parity is still intentionally absent

G11 did the right thing by not pretending to solve everything at once.
But the remaining language story is still:

- Rust/Ruby get the meaningful bounded PDG/propagation improvements
- other languages are mostly still normal call-graph plus limited local augmentation

That is okay for now, but it should remain explicit as planning continues.

---

## Best next-phase candidates

If G12 follows naturally from G11, the strongest candidates look like this:

## Candidate A: multi-input continuation mapping

Why first:

- it is the clearest remaining propagation false-negative family
- it is already called out by the eval set
- it builds directly on the bounded continuation foundation G11 created

Likely goal:

- preserve relevant input selection across one more nested summary layer
- avoid over-propagating unrelated args
- keep witness output compact

## Candidate B: cross-file alias/result stitching

Why next:

- wrapper-return is now stronger than alias/result stitching
- this would improve the second major missing family without requiring project-wide scope growth

Likely goal:

- tighten imported-result -> alias -> caller-result continuity
- keep the improvement tied to bounded family-aware bridges

## Candidate C: budget-policy refinement

Why later but important:

- planner depth is better, but budget policy is still blunt
- some remaining misses are about which valid candidate loses, not whether continuation exists at all

Likely goal:

- preserve boundedness
- make continuation admission less arbitrary across same-seed competition
- keep prune reasons explicit

## Candidate D: bridge execution provenance in witness surfaces

Why useful:

- current selection provenance is good enough to debug scope
- current bridge-execution explanation is still relatively implicit

Likely goal:

- expose just enough “which bridge family actually closed the result” metadata to help humans debug surprising outputs
- avoid dumping full planner state

---

## Final assessment

G11 was a good phase.
It did not overreach.
It made the bounded PDG model more capable **without** abandoning the constraint that makes the feature reviewable.

The phase succeeded because it moved in a disciplined order:

- first inventory current behavior
- then define the bounded expansion rule
- then land one Rust proof point and one Ruby proof point
- then tighten propagation attachment
- then lock the new behavior with regressions
- then align metadata and docs with runtime reality

The net result is:

- bounded slice planning is now better described as **bounded continuation**
- short Rust/Ruby multi-file wrapper-return style bridges are materially better than before
- propagation attaches more naturally to short PDG-backed call boundaries
- JSON/witness surfaces better reflect continuation-tier selection
- the remaining problem space is now split more cleanly into:
  - propagation mapping gaps
  - planner budget gaps
  - explanation/provenance gaps

That is exactly what a healthy phase should do:

**ship a real bounded recall gain, keep the runtime explainable, and make the next unsolved layers smaller and clearer than before.**
