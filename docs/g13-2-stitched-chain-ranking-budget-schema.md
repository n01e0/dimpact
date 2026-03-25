# G13-2: stitched-chain ranking / budget schema

このメモは、G13-1 で棚卸しした post-G12 stitched continuation の over-candidate / mis-selection を、
**runtime 実装前に schema として固定する** ためのもの。

paired machine-readable schema: `docs/g13-2-stitched-chain-ranking-budget-schema.json`

G12 までで dimpact は、bounded scope の上で

- nested multi-input continuation
- alias-result stitching
- Ruby `require_relative` continuation
- compact bridge-execution provenance

をある程度扱えるようになった。

ただし current main の ranking / budgeting はまだかなり file-local で、
次が残っている。

- return-looking candidate が alias-result closer を押しのけやすい
- per-anchor / per-seed の early cap が chain winner ではなく file winner を残しやすい
- same-path suppression では same-chain duplicate を潰し切れない
- provenance が winning chain というより observed stitched step union に寄りやすい

G13-2 で決めたいのは、これを直すための **schema** である。
ここでの主眼は実装ではなく、後続 task が同じ vocabulary を共有できるようにすることにある。

---

## 1. Goal / Non-goal

## Goal

- stitched continuation の ranking / budget / suppress を file-local candidate とは別 schema で表現できるようにする
- `Tier2Candidate` / `BridgeContinuationFile` の上に、**`StitchedChainCandidate`** という representative 単位を定義する
- ranking を lane-first ではなく closure-first に寄せるための score tuple を定義する
- same-path duplicate だけではなく same-chain duplicate を区別できるようにする
- per-family / per-anchor / per-seed の stitched-chain budget 語彙を固定する
- provenance を winning chain と supporting observation に分けるための compact schema を決める

## Non-goal

- project-wide recursive continuation search
- full argument-binding generalization
- 全言語 parity の即時達成
- 既存 file budget をこの task だけで全面置換すること
- reporter/UI の見た目変更を主眼にすること

G13-2 は、scope planner を捨てる task ではない。
**現在の bounded file frontier を前提に、その上で compare/budget する chain representative schema を定義する task**
として扱う。

---

## 2. 基本方針

## 2.1 compare の主語を file candidate から stitched chain representative へ 1 段ずらす

current planner は `Tier2Candidate` を直接 compare している。
これは G10/G11/G12 までの段階では合理的だったが、
post-G12 では次のズレが出る。

- local evidence が強い file と、actual closure quality が高い chain が一致しない
- return/alias/mixed family の勝敗が lane rank に引っ張られやすい
- same-chain duplicate が path 違いで survive する

そこで G13-2 では、**ranking / budget / provenance の主語を `StitchedChainCandidate` に寄せる**。

ただしこれは file selection の手前で full graph search をするという意味ではない。
前提はあくまで

- selected root / boundary / bridge completion / bridge continuation file
- bounded propagation / witness context

の上で、**reconstruct できる representative chain を compare する** というもの。

## 2.2 ranking は closure-first、budget は family-aware、provenance は winning-chain-first

G13-2 で固定したい最小の mental model は次の 3 つ。

1. **ranking**
   - どれだけ自然に closing chain を作れているかを先に見る
2. **budget**
   - file 数だけでなく stitched family representative を主語にする
3. **provenance**
   - selected path 上で観測された step 全体ではなく、winning chain を先に出す

---

## 3. `StitchedChainCandidate` schema

G13 の ranking/budget で compare する最小単位を `StitchedChainCandidate` と呼ぶ。

## 3.1 required fields

最低限、次を持つ。

- `seed_symbol_id`
- `entry_boundary_symbol_id`
- `entry_boundary_path`
- `anchor_symbol_id`
- `anchor_path`
- `terminal_symbol_id`
- `terminal_path`
- `family`
- `step_families`
- `closure`
- `evidence`
- `penalties`
- `duplicate_chain_key`
- `origin_reasons`

ここで重要なのは、`path` や `anchor` だけで compare せず、
**chain がどこから入り、どこへ閉じ、どういう stitched family と step 列で構成されたか** を
first-class に持つこと。

## 3.2 family vocabulary

coarse family は次で固定する。

- `return_continuation`
- `alias_result_stitch`
- `require_relative_continuation`
- `mixed_require_relative_alias_stitch`
- `nested_multi_input_continuation`

この coarse family は

- ranking の family-fit 比較
- family-local budget
- provenance compact 表示
- selected-vs-pruned chain explanation

の共通 vocabulary として使う。

## 3.3 step family vocabulary

step family は G12-3/G12-6 と揃えて次で固定する。

- `callsite_input_binding`
- `summary_return_bridge`
- `nested_summary_bridge`
- `alias_result_stitch`
- `require_relative_load`

`StitchedChainCandidate.step_families` は、winning chain を compact に表す ordered list とする。

## 3.4 closure fields

`closure` は chain quality の中心なので、最低限次を持つ。

- `reaches_caller_result: bool`
- `reaches_nested_continuation: bool`
- `has_alias_result_stitch: bool`
- `has_require_relative_load: bool`
- `binding_quality: exact | reordered | partial | weak_or_unknown`
- `closure_target_kind: caller_result | wrapper_result | nested_result | supporting_only`
- `relevant_binding_count: number`
- `irrelevant_binding_leak: bool`

ここで重要なのは、family label だけでなく
**その chain が実際に caller-side closing chain を作れているか** を独立に持つこと。

## 3.5 evidence fields

`evidence` には current main の candidate scoring を受け継げる面を残す。

- `primary_evidence_kinds`
- `secondary_evidence_kinds`
- `negative_evidence_kinds`
- `semantic_support_rank`
- `call_position_rank`
- `lexical_tiebreak`

つまり G13 は G10/G12 の evidence world を捨てない。
ただし、それを **closure / penalty の後ろ** に置く。

## 3.6 penalty fields

G13 で新しく重要になるのがこれである。

- `duplicate_penalty: none | weak_same_chain_duplicate | merged_same_chain_duplicate`
- `overreach_penalty: none | helper_only_stitch | weak_mixed_label | irrelevant_arg_leak`
- `budget_pressure: none | anchor_budget_edge | family_budget_edge | seed_budget_edge`

これを持つと、

- same-path ではなく same-chain duplicate
- helper-only stitched branch
- mixed family の盛りすぎ
- irrelevant arg leak

を ranking/budget/provenance に同じ語彙で流せる。

## 3.7 origin reason linkage

`origin_reasons` は、既存の file-level reason と stitched-chain schema を接続するために持つ。

最低限:

- `selected_reason_refs[]`
- `pruned_reason_refs[]`
- `observed_supporting_reason_refs[]`

これにより、後続 task で

- file-level summary.slice_selection
- witness-side winning/pruned chain explanation

を無理なくつなげられる。

---

## 4. ranking schema

## 4.1 ranking tuple

G13-2 では、stitched chain compare の score tuple を conceptually 次で固定する。

1. `closure_rank`
2. `overreach_penalty_rank`
3. `duplicate_penalty_rank`
4. `family_fit_rank`
5. `semantic_support_rank`
6. `primary_evidence_count`
7. `negative_evidence_count`
8. `secondary_evidence_count`
9. `call_position_rank`
10. `lexical_tiebreak`

ここで最重要なのは、lane-first をやめて **closure-first** にすること。

## 4.2 closure rank

`closure_rank` は少なくとも次で比較する。

高い順に:

1. caller result まで閉じる
2. wrapper/nested result まで閉じるが caller result はまだ support
3. stitched step は見えるが closing target が support-only

加点条件として:

- relevant binding が explicit
- alias-result stitch が wrapper/caller の両 locality をまたいで連続する
- mixed Ruby chain で require_relative + alias stitch が実際に closing chain を形成する

## 4.3 overreach penalty rank

`overreach_penalty_rank` は低い方がよい。
対象は少なくとも次。

- irrelevant arg leak
- helper-only stitch
- weak mixed label
- non-closing alias chain masquerading as closer

G13-1 で問題化した「local evidence は多いが actual closure quality は低い」ケースは、
ここで落とす。

## 4.4 duplicate penalty rank

`duplicate_penalty_rank` は same-path ではなく same-chain duplicate に使う。

- winner chain と同じ `duplicate_chain_key` を持つ weaker chain は負ける
- path 違いでも same closure key なら duplicate penalty を受ける
- complete duplicate は merge/drop 対象にする

## 4.5 family fit rank

family fit は coarse family の優先度そのものではなく、
**current chain closure に対して family label がどれだけ自然か** を表す。

例:

- caller-result closer が alias stitch 中心なのに `return_continuation` 扱いなら減点
- selected path に require_relative step があるだけで `mixed_require_relative_alias_stitch` を名乗るなら減点
- nested relevant binding が closing chain に寄与していれば `nested_multi_input_continuation` は加点

これは G12-6 の representative family 判定を、そのまま presence union で使い続けないための schema である。

## 4.6 evidence / lexical tiebreak

current main の evidence count / semantic support / call position / lexical tiebreak は残す。
ただし役割は最後の deterministic tiebreak 側へ寄せる。

要するに G13 の compare は

1. actual closure quality
2. overreach / duplicate penalty
3. family fit
4. 既存 local evidence

の順で読む。

---

## 5. duplicate / suppress taxonomy

G13 で必要な judgement taxonomy は file-level と chain-level を分ける方がよい。

## 5.1 chain admission result

- `admitted`
- `merged_same_chain_duplicate`
- `dropped_before_chain_compare`

## 5.2 chain prune result

- `weaker_same_chain_duplicate`
- `weaker_same_family_chain`
- `family_budget_exhausted`
- `stitched_chain_budget_exhausted`
- `support_only_not_winner`
- `overreach_penalized_out`

ここでのポイントは、G10/G11/G12 の

- `weaker_same_path_duplicate`
- `weaker_same_family_sibling`
- `bridge_budget_exhausted`

に加えて、**chain-level loser** を distinct に持つこと。

## 5.3 file-level prune との関係

file-level reason は従来どおり残す。
ただし G13 以降は

- file-level reason: どの file が scope representative だったか
- chain-level reason: その file 上のどの stitched chain が winner だったか

を分けて解釈する。

---

## 6. budget schema

G13-2 では、budget を 3 層で分ける。

## 6.1 per-anchor per-family budget

最小 schema:

- `per_anchor_per_family = 1`

意味:

- 同じ anchor から同じ family の thin variation を何本も残さない
- per-anchor `take(1)` を file candidate ではなく chain family representative にずらす

## 6.2 per-seed family budget

最小 schema:

- `return_continuation = 1`
- `alias_result_stitch = 1`
- `require_relative_continuation = 1`
- `nested_multi_input_continuation = 1`
- `mixed_require_relative_alias_stitch = 1`

ここで大事なのは raw cap を増やすことではない。
**return family が alias-result family を最初から食い潰さないようにすること** が主眼である。

## 6.3 final per-seed stitched representative budget

最終的な explanation/selected winner surface としては、
別に `final_per_seed_stitched_representatives_max` を持つ。

初期値は conservative に

- `2`

を推奨する。

理由:

- winner + alternate/support まで読める
- current bounded 性を壊しにくい
- return と alias の coexistence を 1 seed で最低限観測できる

## 6.4 budget application order

適用順は次で固定する。

1. same-chain duplicate merge/suppress
2. per-anchor per-family representative selection
3. per-seed family budget
4. final per-seed stitched representative budget
5. existing file-level hard cap reconciliation

この順序にすると、G10 で問題だった
**same-family / same-chain variation が raw cap を食う** 問題を減らせる。

## 6.5 relation to existing file budgets

G13-2 は file budget をただちに捨てる schema ではない。

- current file budgets remain structural guardrails
- stitched-chain budgets sit above them as representative selection logic
- if the final winner chain requires a file that was never selected, that remains a planner/scope problem

つまり G13-2 は、**file budget の代替ではなく file budget 上の chain representative schema** である。

---

## 7. provenance / output schema

## 7.1 split winning chain from observed supporting steps

G12 の `bridge_execution_chain_compact` は useful だが、winning chain と observed step union が少し混ざりやすい。
G13-2 では provenance を少なくとも次の 2 層に分ける。

- `winning_bridge_execution_chain_compact`
- `observed_supporting_steps_compact`

前者は ranking/budget で勝った representative chain、後者は selected path 上に見えた補助 stitched step である。

## 7.2 winning chain compact fields

最低限:

- `family`
- `step_family`
- `anchor_symbol_id`
- `anchor_path`
- `terminal_symbol_id`
- `terminal_path`
- `closure_target_kind`
- `summary`

必要なら optional に:

- `binding`
- `stitch`
- `selected_better_by`

## 7.3 observed supporting steps compact fields

最低限:

- `family`
- `step_family`
- `anchor_symbol_id`
- `anchor_path`
- `support_role: support | duplicate | pruned_alternate`

これにより Ruby mixed case でも

- mixed chain が本当に勝ったのか
- require_relative step が support として見えているだけか

を分けられる。

## 7.4 selected-vs-pruned chain reasoning

chain-level compact reasoning には少なくとも次を持たせたい。

- `selected_chain_family`
- `pruned_chain_family`
- `selected_better_by`
- `prune_reason`
- `summary`

`selected_better_by` は初期値として次を想定する。

- `closure_rank`
- `overreach_penalty`
- `duplicate_penalty`
- `family_fit`
- `semantic_support`
- `primary_evidence_count`

---

## 8. known failure families への対応

## 8.1 return-looking helper vs alias closer

効く schema:

- closure-first ranking
- family fit rank
- final per-seed stitched representatives max = 2
- chain-level selected/pruned explanation

## 8.2 competing continuation anchors

効く schema:

- per-anchor per-family representative
- per-seed family budget
- chain representative compare before final compression

## 8.3 same-chain duplicate across different localities

効く schema:

- `duplicate_chain_key`
- `weaker_same_chain_duplicate`
- `merged_same_chain_duplicate`

## 8.4 Ruby mixed-family provenance overstatement

効く schema:

- winning chain vs observed supporting steps split
- family fit rank
- `support_role`

## 8.5 nested multi-input path with noisy alternate

効く schema:

- closure rank with binding quality
- irrelevant arg leak penalty
- support-only-not-winner chain prune

---

## 9. implementation guidance for later tasks

G13-2 は docs/schema task なので、実装順はこの task では固定しない。
ただし後続 task では次の順が自然である。

1. Rust 側で `StitchedChainCandidate` 的な internal representative を最小実装する
2. weaker stitched chain suppress / ranking 改善を 1 件入れる
3. per-family / per-anchor budget を最小実装で入れる
4. provenance を winning-chain-first に寄せる
5. Ruby mixed-family explanation / mis-selection を 1 件改善する

---

## 10. fixed decisions

G13-2 で固定したい判断を最後に短くまとめる。

1. **ranking / budget / provenance の主語は `Tier2Candidate` 単体ではなく `StitchedChainCandidate` に寄せる。**
2. **ranking は lane-first ではなく closure-first にする。**
3. **same-path duplicate だけではなく same-chain duplicate を distinct に扱う。**
4. **budget は raw file count だけでなく per-anchor / per-family / per-seed stitched representative を持つ。**
5. **return family と alias-result family を最初から同一 1 枠に押し込めない。**
6. **provenance は winning chain と observed supporting steps を分ける。**
7. **file budget は structural guardrail として残しつつ、その上で chain representative compare を行う。**

要するに G13-2 の schema は、

**「選ばれた file 群の上で見えている stitched continuation を、
file-local evidence で早押しするのではなく、chain closure quality / duplicate suppression / family-aware budget / winning-chain provenance
で compare する」**

ための最小 vocabulary である。
