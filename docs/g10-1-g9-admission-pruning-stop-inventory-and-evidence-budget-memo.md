# G10-1: G9 時点の admission / pruning / stop 判断棚卸しと evidence-budget 設計メモ

対象: bounded slice planner / Ruby narrow fallback / witness slice summary

このメモは、G9 完了時点の runtime / docs / tests を見直し、
**いま planner がどこで candidate を admission し、どこで prune し、どこで stop しているか** を整理したうえで、
G10 の `evidence-budgeted admission` をどう設計するかを固定するためのもの。

G9 で進んだのは主に次だった。

- evidence vocabulary を `primary / support / fallback / negative` に正規化した
- planner compare に suppressing signal を 1 つ入れた
- Rust/Ruby で 1 件ずつ real improvement を出した
- witness に compact な losing-side reason を足した

ただし G9 は、**evidence の意味付け** はかなり揃えた一方で、
**admission / pruning / stop の判断そのもの** はまだ fixed-count / post-hoc / surface-dependent なまま残っている。

G10 の主題はそこを進めること、つまり
**normalized evidence を「candidate が存在してよいか」「どこで落とすか」「どこで打ち切るか」の判断へ前倒しで使うこと** である。

---

## design input

主に次を見た。

- `docs/g5-1-bounded-project-slice-design-memo.md`
- `docs/g5-3-bounded-project-slice-policy.md`
- `docs/g9-1-g8-evidence-usage-inventory-and-gap-memo.md`
- `docs/g9-2-evidence-normalization-rules.md`
- `docs/g9-10-rollup-summary.md`
- `src/bin/dimpact.rs`
  - `collect_ruby_narrow_fallback_*()`
  - `tier2_scoring_summary()`
  - `compare_tier2_candidates()`
  - `plan_bounded_slice()`
- `src/impact.rs`
  - `selected_reason_ranking_basis()`
  - `selected_vs_pruned_losing_side_reason()`
  - `build_selected_vs_pruned_reasons()`
- `tests/cli_pdg_propagation.rs`
- `README.md` / `README_ja.md`

---

## 1. G10 で見直すべき前提

G5 で固定した bounded slice の中心 contract は今も正しい。

- seed/root file を保持する
- direct boundary を 1-hop だけ取る
- さらにその先は bounded な completion/fallback だけを少数追加する
- project-wide closure や recursive expansion はしない

G9 もこの bounded model 自体は壊していない。
問題は、bounded であることの中身がまだかなり **count-based** だという点である。

現在の runtime は、

- `PER_BOUNDARY_SIDE_TIER2_FILES_MAX = 1`
- `PER_SEED_TIER2_FILES_MAX = 2`

という固定 budget を持ち、
候補の比較も大半は `source_kind / lane / evidence count / negative count / support rank / secondary count` の順で決まる。

これは G9 までなら十分だったが、G10 では次が不足している。

1. **candidate を compare 前に落とす admission/suppression 判断**
2. **同系統の候補を family 単位で budget 化する判断**
3. **「もう十分なのでここで止める」を evidence 側から説明する判断**

つまり G9 までは、
**evidence は mostly compare metadata であって、planner control surface ではまだない**。
G10 ではそこを変える。

---

## 2. G9 時点の admission / pruning / stop 判断棚卸し

## 2.1 admission: どこで candidate が「存在してよい」ことになるか

### A. graph-second-hop candidate の admission

`plan_bounded_slice()` は direct boundary ごとに call-graph side ref を集め、
次を満たす file/symbol を Tier 2 candidate 候補にする。

- root file ではない
- boundary file ではない
- direct boundary path 集合にも入っていない

ここで重要なのは、graph-second-hop 側では
**候補はまず call graph adjacency によって materialize され、evidence はその後で scoring に載る** ということ。

つまり admission の最初の関門はまだかなり structural である。

### B. graph-second-hop 内の lane 決定

`tier2_scoring_summary()` は admission 済み candidate に対して

- `ReturnContinuation`
- `AliasContinuation`
- Ruby の弱い `RequireRelativeContinuation`

を割り当てる。

ただしここでの lane 決定は、次がかなり混ざっている。

- semantic fact (`param_to_return_flow`)
- name/path heuristic (`wrapper`, `return`, `alias`, `helper` 系)
- positional signal (`call_line == side_max_call_line`)

そのため、G9 では evidence category は正規化されていても、
**candidate admission の直後に使う lane 分類はまだ mixed-role** である。

### C. same-path candidate の side 内 admission

同じ boundary side で同じ `completion_file` に複数候補が出た場合、
`side_candidates: BTreeMap<path, Tier2Candidate>` で **その path の best 1 件だけ** を保持する。

これは事実上、最初の duplicate suppression である。
ただしこの suppression は現状、

- same-path candidate を明示的に「落とした」と記録しない
- なぜ loser が負けたかを witness/pruned に残さない
- duplicate / sibling / same-family の区別を持たない

という性質を持つ。

つまり G9 runtime は、
**duplicate suppression を already doing ではあるが、policy としてはまだ explicit でない**。

### D. Ruby narrow fallback candidate の admission

`collect_ruby_narrow_fallback_candidates()` は graph-second-hop より admission が厳しい。
大まかには次を要求する。

- boundary 側に explicit `require_relative` がある
- boundary 側に literal dynamic target がある
- candidate file が root/boundary/direct-boundary と衝突しない
- candidate 側で `collect_ruby_narrow_fallback_candidate_evidence()` が `Some(...)` を返す

ここでは graph-side より明確に、
**raw observation を満たした candidate だけが admission される**。

つまり G9 時点でも、Ruby narrow fallback だけは partly `suppress-before-admit` 的に動いている。
ただしこの discipline は planner 全体へはまだ広がっていない。

---

## 2.2 pruning: どこで candidate が落ちるか

### A. boundary side 内の ranked-out

各 boundary side で candidate を sort した後、
`PER_BOUNDARY_SIDE_TIER2_FILES_MAX = 1` を超えたものは `RankedOut` になる。

これは最も明示的な prune surface で、`pruned_candidates[*].prune_reason = ranked_out` に残る。

ただしここで落ちるのは、あくまで **side に materialize された後の候補** である。

- admission 前に弾かれたもの
- same-path overwrite で消えたもの
- Ruby fallback で `None` になったもの

は `pruned_candidates` に乗らない。

### B. per-seed bridge budget の超過

boundary side をまたいで集めた Tier 2 candidates をさらに sort し、
`PER_SEED_TIER2_FILES_MAX = 2` を超えた候補は `BridgeBudgetExhausted` になる。

これは G5 以来の「bounded で止める」ための主 budget だが、
現状では **evidence family を見ずに raw file count で止める**。

そのため次のズレが残る。

- noisy sibling が strong candidate と同じ 1 slot を消費する
- fallback candidate と graph-first candidate が同じ raw cap を取り合う
- same family の薄い variation が先に残ると、別 family の有益 candidate が budget で落ちうる

### C. selected path 重複時の非-prune merge

per-seed 選抜後、すでに `selected_tier2_paths` にある path が再び来た場合、
runtime はそれを prune せず **同 path へ reason を追加** する。

これは妥当な merge だが、G10 観点では次を意味する。

- path duplicate は budget 消費を避けたい
- ただし duplicate の中でも strong/weak/sibling 差がある
- 「同じ file だから merge」で終わると、admission 競合の情報は消える

したがって G10 では、
**merge してよい duplicate** と **落とすべき weaker sibling** を policy 上分ける必要がある。

### D. witness へ出る prune の範囲

`build_selected_vs_pruned_reasons()` は現状、ほぼ次だけを扱う。

- `RankedOut`
- `reason.kind == BridgeCompletionFile` と matching する pruned candidate

そのため witness は、

- budget exhaustion
- dropped-before-admit
- module companion fallback の side-local loser
- same-path duplicate suppression

を十分には語れない。

G9-6 の losing-side reason は価値があったが、
依然として **post-admit / ranked-out world に強く寄っている**。

---

## 2.3 stop: どこで planner が「ここまででよい」と打ち切るか

### A. no recursive closure

最大の stop rule は変わらずこれである。

- direct boundary の先へ recursive に広げない
- Tier 2 の先の Tier 2.5 / Tier 3 call graph closure をしない
- fallback も bounded companion/runtime rule に留める

これは G10 でも維持すべきコア制約である。

### B. fixed numeric stop

G9 runtime の stop は主に数値 cap である。

- per boundary side: 1
- per seed Tier 2 explanation files: 2

ここでは「何の evidence family をもう確保したか」は見ていない。
したがって stop は今のところ、

- enough evidence だから止める
- weak family だけが残っているから止める
- suppressing profile しか残っていないから止める

ではなく、
**とにかく cap に達したから止める** に近い。

### C. negative evidence は stop 条件になっていない

G9 で `negative_evidence_kinds = [noisy_return_hint]` が入り、
compare 上は「弱い helper を負けさせる」ことができるようになった。

ただしそれでも negative は主に **rank compare の材料** であり、

- side admission を止める
- duplicate/sibling materialization を止める
- further family expansion を止める

ための control にはまだ使っていない。

### D. Ruby fallback だけ部分的に stop-aware

Ruby narrow fallback だけは、

- boundary evidence が不足していれば candidate collection 自体をしない
- literal target family に合わない runtime file を materialize しない

という意味で、部分的に `stop before widen` が入っている。

ただしこれは Ruby fallback 専用 rule であり、
**planner 全体の stop contract にはまだ昇格していない**。

---

## 3. G9 が解いたこと / まだ解いていないこと

## 3.1 G9 が解いたこと

G9 で固定できたのは次である。

1. evidence role を `primary / support / fallback / negative` で読むこと
2. helper noise のような suppressing signal を compare へ入れること
3. same-kind Rust competition を `semantic_support_rank` で改善すること
4. Ruby narrow fallback を broad rescue ではなく bounded admission に寄せること
5. witness が losing-side の最小理由を言えるようにすること

これは G10 の前提として十分大きい。

## 3.2 G9 がまだ解いていないこと

一方で、次はまだ残っている。

1. **admission と pruning の境界が曖昧**
   - same-path overwrite は prune だが surface に出ない
   - fallback `None` は drop だが policy 名がない
2. **budget が family-blind**
   - return / alias / weak require-relative / narrow fallback が同じ raw count を奪い合う
3. **stop が evidence-blind**
   - enough evidence / only weak leftovers / only suppressing leftovers を区別しない
4. **witness が post-admit loser 中心**
   - dropped-before-admit / family-budget drop / duplicate suppression の説明面がない

要するに G9 は、
**evidence の vocabulary はかなり揃えたが、planner control loop はまだ G5/G8 由来の fixed-count world に強く残っている**。

---

## 4. G10 で解くべき gap

## 4.1 admission gap

### gap A: compare に入るまで弱い candidate が多すぎる

現在は graph-side candidate が materialize されてから compare で落ちることが多い。
そのため、明らかに weak な sibling/noise candidate も一度 slot 争いへ入る。

G10 では、少なくとも一部は
**compare 前に落とす suppress-before-admit** が必要である。

### gap B: duplicate / sibling / same-family variation が 1 つの path overwrite に潰れている

現状の same-path best-only は便利だが、次を区別できない。

- 同じ file への duplicate entry
- 同じ family の weaker sibling
- 別 family だが同 path へ converged した competing explanation

G10 では、この collapse を explicit policy に昇格する必要がある。

### gap C: Ruby だけ admission discipline が先行している

Ruby narrow fallback は raw evidence を満たさないと admission されない一方、
graph-second-hop 側は structural admission が先行している。

G10 では、planner 全体を

- raw observation
- normalized admission profile
- suppress gate
- admitted candidate

の段に揃えるべきである。

---

## 4.2 pruning gap

### gap A: prune reason が足りない

現状の主要 prune は

- `ranked_out`
- `bridge_budget_exhausted`

だが、G10 で必要なのは少なくとも次である。

- duplicate/same-path suppression
- weaker sibling suppression
- dropped-before-admit by negative/suppressing evidence
- family budget exhausted
- fallback admission mismatch

ここを増やさないと、evidence-budget 化しても debug/witness が追えない。

### gap B: family ごとの prune がない

G9 では `return_continuation` と `module_companion_fallback` が conceptually 別の役割を持っていても、
prune は基本的に同じ raw pool で起きる。

G10 では、少なくとも conceptually
**same-family の弱い候補を先に落としてから cross-family compare する**
必要がある。

### gap C: witness が budget/exhaustion/drop を十分に拾えない

G9 witness は ranked-out loser の short story には強くなったが、

- `bridge_budget_exhausted`
- dropped-before-admit
- family-budget exhausted

はまだ薄い。

G10-6 で最小 surface を足す前提として、
G10-1 ではまず **prune reason taxonomy を planner 側で切る必要がある**。

---

## 4.3 stop gap

### gap A: cap-first で止まっている

G9 runtime は raw cap に達した時点で止まる。
しかし G10 が欲しいのは、

- 必要 family を 1 件ずつ確保した
- 残りは duplicate か suppressing leftover しかない
- narrow fallback は bounded admission を満たさない

なら **その時点で止める** ことである。

### gap B: enough-evidence stop がない

bounded planner の stop は本来、
「これ以上見ても precision gain より scope risk が大きい」
ところで止まるべきである。

現状はそれを raw count で近似しているにすぎない。
G10 では、少なくとも side/family 単位で
**もう十分に強い代表がいるから追加探索しない**
という判断が必要になる。

### gap C: negative/suppressing leftover stop がない

残候補がすべて

- helper-noise
- fallback-only without continuity
- weaker certainty sibling

なら、その side/family はそこで打ち切ってよい。
G9 にはこの stop rule がまだない。

---

## 5. G10 の設計原則

## 5.1 bounded model は維持する

G10 は project-wide expansion ではない。
変えるのは planner の広さではなく、
**admission / pruning / stop の判断密度** である。

- no recursive closure は維持
- direct boundary + bounded completion/fallback も維持
- precision を scope widening で稼がない、も維持

## 5.2 evidence は ranking metadata ではなく control input にする

G10 の要点はこれである。

- `primary`: continuation を直接示す
- `support`: strength / certainty / weak positive hint
- `fallback`: bounded admission provenance
- `negative`: suppress / demote / stop signal

この 4 category を、compare 表示だけでなく

- admit するか
- merge/drop するか
- family slot を使うか
- ここで stop してよいか

に使う。

## 5.3 raw candidate count ではなく family budget を先に見る

G10 では raw file count budget を完全に消す必要はない。
ただし final cap の前に、少なくとも conceptually 次を挟むべきである。

- family-local representative selection
- same-family duplicate suppression
- family-level budget prune

これにより、strong representative を残しつつ weak sibling を早めに落とせる。

## 5.4 dropped-before-admit も first-class にする

今の planner は「出てきてから落ちた候補」しかよく見えない。
G10 では

- 出す前に止めた
- side へ入れる前に suppress した
- same-path weaker sibling を merge/drop した

も first-class judgement にする必要がある。

## 5.5 witness へ出す surface は最小のまま保つ

G10 は full proof trace を作る段階ではない。
必要なのは、debug full dump ではなく **最小の理由 surface** である。

したがって planner 側では詳細を持っても、public/witness 側は次程度でよい。

- selected/pruned の winning reason
- loser の compact reason
- dropped-before-admit の compact reason
- budget exhausted の compact reason

---

## 6. G10 evidence-budgeted admission の提案

G10 の runtime pipeline は conceptually 次の 6 段に分けるのが筋がよい。

## 6.1 Stage 0: raw observation collection

既存の collector は基本的に活かす。

- graph side ref
- Rust semantic observation
- Ruby narrow fallback boundary/candidate observation

ここではまだ candidate を採用しない。
**raw fact を集めるだけ** に留める。

## 6.2 Stage 1: normalized admission profile materialization

raw observation を受けて、candidate ごとに少なくとも conceptually 次を作る。

- `primary` family/profile
- `support` family/profile
- `fallback` family/profile
- `negative` family/profile
- candidate family (`return`, `alias`, `weak require-relative`, `module companion fallback` など)

G10-2 はこの schema task である。
ここでは enum の最終形より、
**admission / suppression / budget が読める profile にすること** が重要。

## 6.3 Stage 2: suppress-before-admit

次のような candidate は final candidate pool へ入れる前に落としてよい。

- explicit suppressing negative だけが強く、primary/fallback が薄い
- same-path で stronger representative が既にある
- same-family の weaker sibling で、selected side を改善しない
- Ruby fallback で bounded admission provenance が不足する

ここで落としたものは `dropped_before_admit` 系の reason を持たせる。

## 6.4 Stage 3: family-local merge / budget

admitted candidate を side 単位で次のように整える。

1. same-path duplicate merge
2. same-family sibling compare
3. family budget apply

ここではまだ global per-seed budget を使い切らない。
先に **family-local representative** を作る。

たとえば conceptually 次のように読む。

- return family から最良 1 件
- alias family から最良 1 件
- weak require-relative family は strong semantic family が無ければ残す
- narrow fallback family は admission provenance を満たすものだけ残す

## 6.5 Stage 4: cross-family selection under bounded seed budget

family-local representative を作ったあとで、従来の per-seed budget へ通す。

ここではじめて raw explanation-file budget を使う。
つまり G10 の順序は、

- まず family を整理する
- それから final explanation slots を争わせる

である。

これにより、
**same-family の薄い variation で budget を食う** 問題を減らせる。

## 6.6 Stage 5: evidence-aware stop

stop は次のいずれかで起きるべきである。

1. bounded structural limit に達した
   - no recursive closure
2. family-local representatives が揃った
   - 追加候補は duplicate/suppressing leftover しかない
3. final seed budget に達したうえで、未採用候補が family representative を更新しない
4. fallback admission provenance が足りず、これ以上 bounded に出せない

これが G10 の `evidence-budgeted stop` の最小像である。

---

## 7. G10 で必要な judgement taxonomy

G10-2 以降で最終化すべきだが、G10-1 の時点で必要なのは少なくとも次である。

## 7.1 admission result

- `admitted`
- `dropped_before_admit`
- `merged_into_existing`

## 7.2 prune result

- `ranked_out`
- `family_budget_exhausted`
- `bridge_budget_exhausted`
- `fallback_admission_mismatch`
- `weaker_same_path_duplicate`
- `weaker_same_family_sibling`

最終 enum 名は後で調整してよいが、概念としてはこの差が必要。

## 7.3 stop result

- `structural_stop`
- `family_representatives_satisfied`
- `only_suppressing_leftovers`
- `bounded_budget_reached`

いまの runtime はこれらを mostly 1 つの raw cap に潰している。
G10 はそこを分解する段階である。

---

## 8. 既存 surface への影響

## 8.1 planner runtime

主な変化点は次。

- `plan_bounded_slice()` の前半で candidate profile/materialization 層を明確化する
- same-path overwrite を explicit decision にする
- side-local ranked-out の前に family-local suppress/budget を入れる
- Ruby fallback の admission discipline を graph-side と vocabulary 上で揃える

## 8.2 slice summary

`summary.slice_selection` は最終的に次を追える必要がある。

- selected file とその reasons
- ranked-out / budget-pruned candidate
- dropped-before-admit の最小診断
- family budget の結果

ただし public JSON を一気に肥大化させる必要はない。
compact surface と internal full surface を分けてもよい。

## 8.3 witness

witness で最終的に欲しいのは full trace ではなく、少なくとも次。

- winning primary/support/fallback
- losing negative
- dropped-before-admit の compact 理由
- budget/stop により「なぜ入れなかったか」の短い理由

G10-6 はこの面を担当する。

---

## 9. G10 後続タスクとの対応

## G10-2

evidence family ごとの admission / suppression / budget schema 定義。
G10-1 の中核成果はここへの入力である。

## G10-3

suppress-before-admit 実装。
特に graph-side weak candidate を compare 前に落とせるようにする。

## G10-4

Rust duplicate / sibling candidate 過剰選択の抑制。
G10-1 で整理した same-path / same-family gap の具体化先。

## G10-5

Ruby fallback admission refinement。
現行 narrow fallback の discipline を 1 段一般化する場所。

## G10-6

witness / slice summary に dropped-before-admit の最小理由を追加。
G10-1 で切った judgement taxonomy を user-facing 最小 surface へ落とす。

## G10-7 / G10-8

eval set / regression 拡張。
特に

- admission conflict
- family budget exhaustion
- duplicate suppression
- non-widening guard

を固定する。

## G10-9

README 更新。
G9 の evidence normalization mental model を、G10 では admission/budget/stop mental model に進める。

---

## 10. G10 の acceptance shape

G10 がうまくいったと言える最小条件は次だと考える。

1. **Rust**
   - duplicate/sibling/noisy helper の少なくとも 1 件が compare 前または family budget で抑えられる
2. **Ruby**
   - fallback admission が 1 段 refine され、dynamic-heavy noise が materialization 前に落ちる
3. **witness / slice summary**
   - ranked-out だけでなく、dropped-before-admit または family-budget drop を短く説明できる
4. **regression**
   - 改善が単なる scope widening でないことを selected_files_on_path / pruned_candidates / witness context で固定できる

つまり G10 の成功条件は、
**強い候補を勝たせること** だけではない。
**弱い候補を「そもそも入れない」「同系統 budget の中で早めに落とす」「止める理由を残す」こと** まで含む。

---

## 11. 一言まとめ

G9 は evidence の意味を揃えた段階であり、
G10 の本体はその normalized evidence を
**ranking metadata から planner control surface へ進めること** である。

より具体的には、
G10 でやるべきなのは budget を増やすことではなく、
**admit する前に弱い候補を suppress し、family ごとに代表を選び、enough-evidence なら bounded に止まる planner にすること** である。
