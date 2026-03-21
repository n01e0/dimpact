# G9-1: G8 時点の evidence 利用箇所棚卸しと planner / fallback / witness gap memo

対象: bounded slice planner / Ruby narrow fallback / witness explanation

このメモは、G8 完了時点の runtime / docs / tests を見直し、
**evidence が実際にどこでどう使われているか** と、
**planner / fallback / witness の間でまだ揃っていない点** を整理して G9 の正規化入力にするためのもの。

見る対象は主に次。

- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`
- `docs/g8-2-bridge-scoring-evidence-schema.md`
- `docs/g8-9-rollup-summary.md`
- `src/bin/dimpact.rs`
  - `tier2_scoring_summary()`
  - `ruby_require_relative_scoring_summary()`
  - `collect_ruby_narrow_fallback_*()`
  - `ruby_narrow_fallback_scoring_summary()`
  - `compare_tier2_candidates()`
- `src/impact.rs`
  - `build_selected_vs_pruned_reasons()`
  - `selected_vs_pruned_*()`
  - `ImpactWitnessSliceSelectedVsPrunedReason`
- `tests/cli_pdg_propagation.rs`
- `README.md` / `README_ja.md`

前提として、G9 の目的は G8 の否定ではない。
G8 は実際に `param_to_return_flow`、Ruby の true narrow fallback、winning evidence/support の最小説明を入れた。
ただしその結果、**「evidence」という同じ語で呼んでいるものの中身が surface ごとにかなり違う** こともはっきりした。
G9 ではそのズレを正規化しないと、planner の score・fallback の bounded rule・witness の説明が同じ vocabulary を共有しているように見えて、実際には別物のまま残る。

---

## 1. G8 時点の evidence 利用面

G8 で evidence を使っている面は大きく 3 つある。

1. **planner scoring**
   - Tier 2 candidate の `source_kind / lane / primary_evidence_kinds / secondary_evidence_kinds / support / score_tuple` を作る
   - 実装中心は `src/bin/dimpact.rs`
2. **Ruby narrow fallback discovery**
   - graph-second-hop で拾えない Ruby companion を bounded に補う
   - 実装中心は `src/bin/dimpact.rs::collect_ruby_narrow_fallback_*()`
3. **witness explanation**
   - selected candidate が pruned candidate にどう勝ったかを compact に出す
   - 実装中心は `src/impact.rs::build_selected_vs_pruned_reasons()`

同じ `ImpactSliceEvidenceKind` と `ImpactSliceCandidateSupportMetadata` を共有しているので一見揃って見えるが、
実際には次の違いがある。

- planner は **ranking 用の proxy** と **観測 fact** が混在している
- fallback は planner より **観測 fact 寄り** だが、出す種類が狭い
- witness は planner/fallback が出した **差分だけ** を要約し、負け筋や shared evidence はほぼ捨てる

---

## 2. 現在の inventory

## 2.1 enum / schema 上の evidence

G8 時点の `ImpactSliceEvidenceKind` は次。

- `return_flow`
- `assigned_result`
- `alias_chain`
- `param_to_return_flow`
- `require_relative_edge`
- `explicit_require_relative_load`
- `module_companion`
- `companion_file_match`
- `dynamic_dispatch_literal_target`
- `callsite_position_hint`
- `name_path_hint`

support metadata は次。

- `call_graph_support`
- `local_dfg_support`
- `symbolic_propagation_support`
- `edge_certainty = confirmed | inferred | dynamic_fallback`

## 2.2 planner runtime が実際に materialize するもの

### graph-second-hop (`tier2_scoring_summary()`)

実際に出るのは主に次。

- primary
  - `return_flow`
  - `assigned_result`
  - `alias_chain`
  - `param_to_return_flow`
- secondary
  - `callsite_position_hint`
  - `name_path_hint`
- support
  - `local_dfg_support`（`param_to_return_flow` が立ったときだけ）

ただし中身は均一ではない。

- `param_to_return_flow` だけは Rust local DFG / function summary を読んだ semantic fact
- それ以外の多くは `wrap` / `adapter` / `leaf` / `value` / `helper` などの **name/path hint** と call position から作る proxy

つまり planner の graph-second-hop 側では、
**semantic evidence と lexical heuristic が同じ `primary_evidence_kinds` に並んでいる**。

### Ruby require-relative continuation (`ruby_require_relative_scoring_summary()`)

Ruby で strong semantic tier2 evidence が無い場合、planner は次へ落とす。

- primary
  - `require_relative_edge`
- secondary
  - `callsite_position_hint`（side 内で最後なら）
- support
  - なし

これは narrow fallback ではなく `graph_second_hop` のままの continuation だが、
実際の ranking 上は Ruby 用の弱い fallback lane として使われている。

## 2.3 Ruby narrow fallback runtime が materialize するもの

`collect_ruby_narrow_fallback_boundary_evidence()` と
`collect_ruby_narrow_fallback_candidate_evidence()` が観測するのは次。

- boundary 側
  - 明示的な `require_relative`
  - literal `send/public_send`
- candidate 側
  - literal target と method/function 名の一致
  - `method_missing` / `respond_to_missing?` / `define_method`

これを `ruby_narrow_fallback_scoring_summary()` が次へ変換する。

- primary
  - `companion_file_match`
  - `dynamic_dispatch_literal_target`
  - `explicit_require_relative_load`
- secondary
  - なし
- support
  - `edge_certainty = inferred | dynamic_fallback`

ここでは planner graph-second-hop 側より、evidence がかなり **観測 fact 寄り** になっている。

## 2.4 witness runtime が実際に使うもの

`build_selected_vs_pruned_reasons()` は selected/pruned の scoring から次だけを抜く。

- `selected_better_by`
  - `source_kind`
  - `lane`
  - `primary_evidence_count`
  - `secondary_evidence_count`
  - `callsite_position`
  - `lexical_tiebreak`
- `winning_primary_evidence_kinds`
  - selected にあり pruned に無い primary evidence の差分だけ
- `winning_support`
  - selected にだけある support、またはより強い `edge_certainty`

witness は次を使わない。

- shared primary evidence
- secondary evidence の具体的な kind 差分
- pruned 側にだけある evidence
- negative / suppressing evidence
- boundary 側で観測した raw fact

つまり witness は **planner/fallback の full inventory を説明しているわけではない**。
あくまで score を壊さずに出せる「勝ち筋の差分要約」だけを使っている。

---

## 3. surface ごとの役割とズレ

## 3.1 planner: ranking のための evidence

planner は `compare_tier2_candidates()` の比較順に合わせて scoring を作る。
実際に ranking へ効く順は次。

1. `source_rank`
2. `lane_rank`
3. `primary_evidence_count`
4. `secondary_evidence_count`
5. `call_position_rank`
6. `lexical_tiebreak`

ここで重要なのは、planner は **count-based** だということ。
`primary_evidence_kinds` の中に入った fact/proxy は多くの場合「1 個」としてしか扱われない。

結果として、G8 planner の evidence は次の性質を持つ。

- semantic fact の強さを count に潰す
- lexical proxy も semantic fact も同じ 1 票として積む
- negative / suppressing 情報は score tuple に入らない
- support は count に直接効かず、主に witness 向け metadata に近い

## 3.2 fallback: bounded discovery のための evidence

Ruby narrow fallback の evidence は、ranking より前の **candidate 発見条件** に強く結びついている。

- `explicit_require_relative_load` は boundary で load を見たか
- `dynamic_dispatch_literal_target` は literal target を narrow できたか
- `companion_file_match` は bounded companion 規則に乗ったか

つまり fallback evidence は planner evidence より、
**「候補が出現してよい理由」そのもの** に近い。

この違いのせいで、同じ `primary_evidence_kinds` でも planner と fallback では意味の密度が違う。

## 3.3 witness: human-facing diff のための evidence

witness は ranking の勝敗説明に必要な差分しか出さない。

- 何が勝ち筋だったか
- 何の basis で tie が割れたか

までは言えるが、

- どの observed fact が shared だったか
- 負け側は何が足りず、何が弱く、何が suppress されたか
- fallback で candidate が出たが採用されなかった理由

は基本的に言えない。

そのため witness は、planner/fallback の vocabulary を **完全には再現していない**。

---

## 4. G8 時点で見えている具体的な gap

## 4.1 lexical proxy と semantic fact が同じ primary evidence に混ざっている

これは G8 時点の最大のズレ。

`param_to_return_flow` は local DFG / function summary 由来の fact だが、
`return_flow` / `assigned_result` / `alias_chain` は多くのケースで name/path hint から立っている。
さらに `name_path_hint` は secondary へ落ちるが、実際には primary 側の付与条件そのものにも name/path heuristic が強く効いている。

結果として witness で

- `winning_primary_evidence: return_flow`
- `winning_primary_evidence: alias_chain`

と見えても、ユーザーが期待する「観測された flow fact」とは限らない。
G8 の vocabulary は統一されたが、**証拠の粒度と確からしさは未統一** である。

## 4.2 planner 側 support と fallback 側 support が別世界になっている

runtime で実際に埋まる support はかなり偏っている。

- Rust planner: `local_dfg_support` のみ
- Ruby narrow fallback: `edge_certainty` のみ
- `call_graph_support`: runtime 未使用
- `symbolic_propagation_support`: runtime 未使用

schema と witness unit test には `symbolic_propagation_support` が出てくるが、
G8 runtime はそれを実際には materialize していない。

つまり support metadata は G8 で shape は整ったが、
**surface 間で同じ support vocabulary を本番利用しているわけではない**。

## 4.3 `module_companion` は enum にあるが runtime では実質使っていない

`ImpactSliceEvidenceKind::ModuleCompanion` は schema 上残っているが、
G8 runtime の実質ルートは

- Ruby weak graph continuation なら `require_relative_edge`
- true narrow fallback なら `companion_file_match + dynamic_dispatch_literal_target + explicit_require_relative_load`

になっており、`module_companion` 自体は materialize されていない。

これは G7/G8 を跨いだ schema/runtime drift で、
G9 では次のどちらかを決める必要がある。

- `module_companion` を compatibility label として残す
- もう runtime では不要として、normalized taxonomy では別 category へ落とす

## 4.4 Ruby の weak continuation と true narrow fallback が別 vocabulary のまま

G8 Ruby には 2 つの「require_relative が絡む continuation」がある。

### A. graph_second_hop の弱い continuation

- `source_kind = graph_second_hop`
- `lane = require_relative_continuation`
- primary は `require_relative_edge`
- secondary に `callsite_position_hint`

### B. true narrow fallback

- `source_kind = narrow_fallback`
- `lane = module_companion_fallback`
- primary は `companion_file_match + dynamic_dispatch_literal_target + explicit_require_relative_load`
- support は `edge_certainty`

両方とも Ruby の multi-file continuation を表すが、
**片方は軽い lexical/structural hint、片方は bounded rule に基づく observed fact** である。

この 2 系統は G8 では「別 path を足した」状態で、まだ normalized relation がない。
G9-2 で `primary / support / fallback / negative` を整理するなら、ここをまず揃える必要がある。

## 4.5 witness の matcher が narrow fallback 競合を拾いにくい

`build_selected_vs_pruned_reasons()` が pruned candidate と突き合わせる条件は厳しく、
実質的に

- selected reason が `BridgeCompletionFile`
- pruned candidate も同 kind
- `RankedOut`
- seed/tier/via が一致

のケースだけを拾う。

一方で runtime 上の narrow fallback candidate は `candidate_reason_kind()` により
`ModuleCompanionFile` になる。

そのため、**graph-second-hop が narrow fallback に勝った実ランタイム競合** は、
schema 上は比較可能に見えても witness surface へ自然には出てこない。

`src/impact.rs` の unit test には `source_kind` 競合の説明ケースがあるが、
そこでは pruned 側を `BridgeCompletionFile` として合成しており、
runtime の `ModuleCompanionFile` とは揃っていない。

これは G8 で最も分かりやすい planner/fallback/witness drift の 1 つ。

## 4.6 witness は winning-side を言えるが losing-side をほぼ言えない

G8 witness は次は言える。

- `selected_better_by`
- `winning_primary_evidence_kinds`
- `winning_support`

しかし次は言えない。

- pruned 側にだけあった evidence
- suppress された evidence
- `dynamic_fallback` 側が負けた理由の短い losing-side summary
- same evidence count だが certainty/support が弱かった、という説明の体系化

G9 tasklist に `losing-side の簡易理由` が入っているのは、
G8 witness が **勝ち筋だけを露出する最小面** で止まっているからである。

## 4.7 negative / suppressing evidence の置き場がまだ無い

G8 の score tuple は全部「足し算」に寄っている。

- source kind が強い
- lane が強い
- primary/secondary evidence が多い
- call position が強い

という winner 側の加点しかなく、

- `helper/noise` 的だから減点した
- `dynamic_fallback` なので confirmed/inferred より後ろへ回した
- candidate は出せるが suppress した

のような **負の signal** は正規化されていない。

現在はその一部を

- lane の選択条件
- `edge_certainty`
- lexical tiebreak

へ押し込んでいるだけで、surface を跨いで共有できる negative/suppressing vocabulary にはなっていない。

---

## 5. G8 docs / tests と runtime のズレ

## 5.1 README は unified mental model を先に提示している

README / README_ja は G8 の public story として

- evidence-driven planner
- `primary_evidence_kinds` / `secondary_evidence_kinds`
- `selected_vs_pruned_reasons`

を一続きの review surface として説明している。

方向性は正しいが、runtime 実態はまだ次の通り。

- planner primary evidence の一部は lexical proxy
- fallback evidence はより factual
- witness は diff-only

したがって G9 では、README 的な「一貫した mental model」を runtime 実態に合わせて本当に成立させる必要がある。

## 5.2 unit test が schema 先行の理想状態を一部先取りしている

特に `selected_vs_pruned_reason_derives_winning_metadata_for_source_kind_explanations()` は、
`source_kind = graph_second_hop` vs `narrow_fallback` の競合説明を unit test で固定している。

ただしその fixture は pruned 側 `kind` を runtime の `ModuleCompanionFile` ではなく
`BridgeCompletionFile` として合成している。

つまりこの test は価値がある一方で、
**「schema 上こう説明したい」という理想を固定していて、現行 runtime の wiring そのものを保証してはいない**。

## 5.3 eval set は improvement surface を固定しているが、正規化面はまだ薄い

`docs/g8-6-evidence-driven-eval-set.md` は G8 の改善ケースを押さえているが、
今は主に

- Rust `param_to_return_flow`
- Ruby true narrow fallback
- witness winning evidence/support

の「効いたケース」を固定している。

G9 で必要なのはそれに加えて、

- same evidence name だが semantic/factual density が違うケース
- graph continuation と narrow fallback が競合したときの witness wiring
- negative / suppressing evidence を入れたときの compare order

を regression 面へ追加すること。

---

## 6. G9 へ持ち込む整理ポイント

## 6.1 evidence を 4 層に分ける

G8 の実態を見ると、少なくとも次は分ける必要がある。

1. **primary evidence**
   - candidate continuity / fallback selection を直接示す観測 fact
2. **support evidence**
   - call graph / local DFG / symbolic propagation / certainty の強さ
3. **fallback provenance**
   - bounded fallback 規則で candidate が出た理由
4. **negative / suppressing evidence**
   - candidate を後ろへ回した、または witness で losing-side reason に出すための signal

G8 は 1 と 2 を partly 分けたが、3 と 4 がまだ混線している。

## 6.2 planner と witness の比較単位を揃える

G9 では witness が planner の差分 surface を忠実に読める必要がある。
最低限ほしいのは次。

- `ModuleCompanionFile` を含む selected/pruned comparison を通す
- winning-side だけでなく losing-side の簡易理由を持つ
- secondary evidence 差分や suppressing evidence を summary に落とせるようにする

## 6.3 Ruby continuation を graph / fallback の 2 系統のまま放置しない

G8 の Ruby は

- graph-second-hop の weak require-relative continuation
- narrow_fallback の true companion selection

が並立している。

G9 ではこれを

- 同じ comparison table 上で review できるようにする
- どこまでが primary fact で、どこからが fallback provenance かを分ける
- `edge_certainty=dynamic_fallback` を negative/suppressing signal とどう関係づけるか決める

必要がある。

## 6.4 lexical proxy を evidence と support から分離する

G8 planner の `return_flow` / `assigned_result` / `alias_chain` は、現状では heuristic 由来がまだ多い。
G9 では少なくとも docs 上、そしてできれば runtime taxonomy 上も、

- semantic fact
- lexical hint
- suppressing/noise hint

を別 category に分けるべきである。

そうしないと witness が「evidence」と言ったときの意味が一貫しない。

---

## 7. 結論

G8 は evidence vocabulary を増やし、Rust/Ruby で 1 件ずつ real improvement を出し、witness に winning evidence/support を載せるところまでは進んだ。
ただし G8 完了時点の実態は次である。

- planner は lexical proxy と semantic fact を同じ evidence 面で扱っている
- fallback はより factual だが、planner との relation がまだ未正規化
- witness は winning-side の compact diff に特化しており、fallback 競合や losing-side reason を十分には扱えていない
- support metadata は schema は広いが runtime materialization は偏っている

したがって G9 の正規化でやるべきことは、単に enum を増やすことではない。
本質は **同じ evidence vocabulary が surface を跨いでも同じ密度で読めるようにすること**、
そして **planner / fallback / witness が同じ comparison story を共有できるようにすること** である。

このメモの整理をそのまま G9-2〜G9-6 の入力にすると、最低限次の順で進めるのが自然である。

1. evidence taxonomy を `primary / support / fallback provenance / negative` に分ける
2. planner scoring に negative / suppressing evidence を入れる
3. Ruby fallback と graph continuation の comparison surface を揃える
4. witness に losing-side 理由を追加し、runtime の narrow fallback 競合も通す
