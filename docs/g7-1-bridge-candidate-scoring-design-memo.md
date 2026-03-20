# G7-1: G6 時点の bridge candidate 誤選択パターン棚卸しと scoring 設計メモ

このメモは、G6 までに入った bounded slice / controlled 2-hop planner を前提に、
**bridge candidate がどこで誤選択しやすいか** を棚卸しし、
G7 で入れる `bridge-kind / evidence-kind scoring` の設計方針を固めるためのもの。

G6 は useful だった。

- `summary.slice_selection` で selected / pruned を見えるようにした
- controlled 2-hop を side-local budget 付きで入れた
- Rust の same-side wrapper-return 誤選択を 1 件潰した
- Ruby の short multi-file propagation を 1 件改善した
- witness と slice selection を軽く接続した

ただし G6-10 rollup でも整理した通り、
**現行の bridge candidate ranking はまだ「見えるようになった」段階であって、「十分に賢くなった」段階ではない。**

いまの Tier 2 ranking は主に `src/bin/dimpact.rs` の

- `infer_tier2_bridge_kind()`
- `compare_tier2_candidates()`
- `plan_bounded_slice()`

で決まっている。

ここで使っている signal はかなり少ない。
実質的には

- coarse な `bridge_kind` 推定
- call line
- lexical path tie-break

に寄っており、
G6 で直した 1 件の Rust case には十分でも、
今後の Rust/Ruby の mixed bridge competition には浅い。

したがって G7 の中心課題は scope をさらに広げることではなく、
**bounded のまま candidate の意味づけと優先順位づけを深くすること** になる。

---

## 1. 現行 G6 runtime が実際にやっていること

まず、docs ではなく runtime が何をしているかを短く固定しておく。

## 1.1 bridge kind 推定はかなり coarse

現行 `infer_tier2_bridge_kind()` は次に近いルールで動く。

1. boundary / completion のどちらかが `.rb` なら `require_relative_chain`
2. そうでなければ boundary symbol 名 / path 名に
   - `wrap`
   - `wrapper`
   - `adapter`
   - `service`
   が含まれていれば `wrapper_return`
3. それ以外は `boundary_alias_continuation`

つまり現在の `bridge_kind` は
**graph/evidence から導かれた semantic classification というより、名前ベースの近似ラベル** に近い。

## 1.2 same-side ranking は `bridge_kind -> call_line -> path`

現行 `compare_tier2_candidates()` は概ね次でソートする。

1. `bridge_kind` priority
   - `wrapper_return`
   - `boundary_alias_continuation`
   - `require_relative_chain`
2. call line（後ろの call を優先）
3. lexical path
4. `via_path`
5. `via_symbol_id`

この ranking は G6-6 の
「lexically earlier な noise helper より、後段の return-relevant leaf を選ぶ」
ケースには効く。

しかし逆に言うと、
**現行 ranking はほぼそのケースを通すための最小 ranking** であり、
多様な誤選択パターンを説明できる形にはまだなっていない。

## 1.3 `pruned_candidates` は outcome を見せるが、decision basis は薄い

G6 は

- `ranked_out`
- `bridge_budget_exhausted`

などの prune result を見えるようにした。

これは大きな前進だった。
ただし現状の `pruned_candidates` が教えてくれるのは主に

- どの file が落ちたか
- どの `bridge_kind` と `via_*` だったか
- なぜ落ちたか（rank/budget）

までで、
**なぜその rank になったか** までは分からない。

したがって G7 では
`selected vs pruned` を見えるようにするだけでなく、
**その差分の理由を score vector として残す** 必要がある。

---

## 2. G6 時点で見えている誤選択パターン

ここでは、G6 までの docs / code / tests から見える誤選択パターンを整理する。

重要なのは、
「現在も必ず failure として再現するもの」だけでなく、
**現行 scoring の弱さとして残っている structural pattern** も含めること。

## 2.1 Pattern A: 名前ベースの `bridge_kind` 推定が wrapper を過剰に昇格させる

### 症状

boundary symbol / path に `adapter` / `service` / `wrapper` が入っているだけで
`wrapper_return` に寄るため、
実際には alias continuation 的な case でも wrapper 系として扱われやすい。

### なぜ起きるか

現行 runtime では

- return continuity
- assigned-result continuity
- alias continuity
- require-relative continuity

を独立 signal として見ていない。
`bridge_kind` が**名前 heuristic と semantic priority を兼務**しているからである。

### 何が困るか

- Rust で `service.rs` / `adapter.rs` 命名の中に雑多な helper / alias path があると、wrapper 系が過剰に勝ちやすい
- alias continuation を本当に優先したい case でも、名前だけで wrapper 扱いになりうる
- `bridge_kind` が label 兼 score になってしまい、explanation が粗くなる

### G7 で必要なこと

`bridge_kind` は
**candidate が閉じようとしている continuation shape** を表す label に戻し、
実際の ranking は別の `evidence_kind` 群で支えるべきである。

---

## 2.2 Pattern B: neutral 名の return-relevant candidate が alias continuation 扱いに落ちやすい

### 症状

return-relevant な second hop でも、
path/name に wrapper 系のヒントが無いと `boundary_alias_continuation` 側へ落ちる。

### なぜ起きるか

現行 `infer_tier2_bridge_kind()` は

- return-ish な data continuity
- call result が caller-side def に戻る形
- boundary 側の function が pass-through / wrapper 的か

を中身では見ていない。
したがって neutral な名前の Rust file / symbol では
**semantic には wrapper-return でも alias 扱いに寄る**。

### 何が困るか

- `wrapper_return > boundary_alias_continuation` の固定 priority があるため、kind の誤推定がそのまま rank の誤りになる
- path/name だけで kind が落ちると、後段の evidence を積んでも逆転しにくい
- selected / pruned explanation で「なぜ alias continuation 扱いなのか」が説明できない

### G7 で必要なこと

`bridge_kind` の推定に

- boundary symbol が戻り値連結の中継点か
- assigned-result / direct return / param-to-return continuity があるか

を入れ、
**名前 heuristic は最後の補助 tie-break に落とす** 必要がある。

---

## 2.3 Pattern C: Ruby 候補がほぼ全部 `require_relative_chain` に潰れる

### 症状

`.rb` が関わるだけで `require_relative_chain` になるため、
Ruby 内では

- wrapper-return 的 continuation
- alias continuation
- narrow companion fallback
- 本当に require-relative split chain を閉じている case

の差が runtime 上ほぼ表現されない。

### なぜ起きるか

現行 runtime には
**Ruby bridge の semantic lane を切り分ける仕組みがない**。
`.rb` かどうかがほぼ唯一の分岐になっている。

### 何が困るか

- graph-first な Ruby 2-hop と fallback 的 Ruby companion が同列に見える
- Ruby 側で ranking を改善したくても、今の `bridge_kind` だけでは差が付かない
- `selected vs pruned` が見えても、Ruby では「何系の勝ち負けか」が説明しにくい

### G7 で必要なこと

Ruby 側は少なくとも

- `require_relative_chain`
- `boundary_alias_continuation`
- `wrapper_return`
- `module_companion_fallback`（または narrow fallback lane）

を区別し、
さらに `evidence_kind` で

- graph second hop
- require-relative edge
- alias chain
- return-flow recovery
- fallback companion

を分ける必要がある。

---

## 2.4 Pattern D: same-side ranking が call-line に寄りすぎる

### 症状

同じ boundary side 内では、
現状 `bridge_kind` の次にほぼ call line が支配的である。
そのため

- later call だが semantic には noise
- earlier call だが result/return continuity は強い

という競合で、later call が勝ちやすい。

### どこまで G6 で直せたか

G6-6 では
「lexically earlier noise helper より、後段 leaf を選ぶ」
ケースを通した。

これは良い fix だったが、同時に
**later call = stronger evidence ではない** ことも示している。
今の score はその両者をほぼ同一視している。

### 何が困るか

- call order の偶然が semantic evidence より強く働きうる
- wrapper-return / alias continuation 混在時に、本当に closure を強める candidate が落ちうる
- witness explanation に「後ろの call だったから選んだ」以上の説明を出せない

### G7 で必要なこと

call line は残してよいが、位置づけを下げるべきである。
少なくとも

1. evidence lane
2. evidence strength
3. fallback かどうか
4. call position
5. lexical order

くらいの順に下げるのが妥当である。

---

## 2.5 Pattern E: per-seed budget prune が coarse な global ordering に依存している

### 症状

side-local に 1 candidate ずつ選んだ後、
per-seed budget (`PER_SEED_TIER2_FILES_MAX`) を超えると
seed 全体で再度並べて落としている。

この時の比較軸も結局は

- `bridge_kind`
- call line
- path

である。

### なぜ問題か

異なる boundary side の candidate は、しばしば
**違う理由で必要** になる。
しかし現行 rank では

- alias continuation を閉じる callee-side candidate
- wrapper-return を閉じる別 side candidate

のような異質な候補を、かなり粗い基準で 1 本化している。

### 何が困るか

- budget prune が「強い semantic loss」なのか「単なる third-best drop」なのか区別しにくい
- future fixture で複数 bridge family が競合した時に、budget drop の説明が弱い
- `bridge_budget_exhausted` は見えるが、「何に負けたか」が分からない

### G7 で必要なこと

budget prune 前に各 candidate の
**lane / evidence summary / score tuple** を持たせ、
落とされた candidate には
「同 seed でどの lane / score の candidate に押し出されたか」を残せるようにしたい。

---

## 2.6 Pattern F: graph-first candidate と fallback candidate がまだ同じ土俵に乗りやすい

### 症状

G6 policy docs は Tier 3 fallback を narrow に扱う方針を持っているが、
runtime はまだ scoring 上その差を十分に持っていない。
今後 G7-6 で Ruby fallback を強めると、

- graph second hop で見つかった candidate
- companion / fallback 由来の candidate

が同じ rank 軸に乗りやすい。

### なぜ問題か

fallback は有用だが、bounded planner では本来
**graph-first candidate より一段弱い lane** として扱うべきである。
ここが曖昧だと planner が再び path heuristic 寄りに戻る。

### G7 で必要なこと

candidate profile に

- `source_kind = graph_second_hop | narrow_fallback`

を入れ、
同程度の semantic evidence なら
**graph-first を常に優先** する必要がある。

---

## 2.7 Pattern G: selected-vs-pruned explanation に「勝ち筋」が残っていない

### 症状

現行 output では

- selected candidate の `bridge_kind`
- pruned candidate の `bridge_kind`
- prune reason

は見えるが、
両者の差はまだ人間がコードを読んで推測する必要がある。

### 何が困るか

- G7-7 で witness explanation を強くしたいのに、ranking reason の原材料が薄い
- regression で「何を守りたいか」を fixture に固定しづらい
- docs / tests / runtime が再びズレやすい

### G7 で必要なこと

G7-7 で少なくとも

- selected candidate の score tuple 抜粋
- pruned candidate が負けた主要理由 1〜2 個
- lane / evidence の比較結果

を `slice_selection` / witness explanation へ繋げられるようにするべきである。

---

## 3. G7 で採るべき scoring 設計原則

G7 では「もっと賢い score」を入れたいが、
G2 の教訓どおり opaque weighted scoring に振りすぎるのは避けたい。

したがって G7 の scoring は
**学習された 1 個の数字** ではなく、
**reviewable な lexicographic score tuple** として設計するのがよい。

## 3.1 `bridge_kind` と `evidence_kind` を分離する

最低限、次を分離する。

### `bridge_kind`

candidate が閉じようとしている bridge family。

- `wrapper_return`
- `boundary_alias_continuation`
- `require_relative_chain`
- `module_companion_fallback`（必要なら追加）

### `evidence_kind`

candidate がその family に属すると言える根拠。
1 candidate が複数持ってよい。

例:

- `graph_second_hop`
- `return_flow`
- `assigned_result`
- `alias_chain`
- `require_relative_edge`
- `module_companion`
- `callsite_position_hint`
- `name_path_hint`

この分離で、
`bridge_kind` を label、`evidence_kind` を ranking material にできる。

## 3.2 score は「gate → lane → strength → tie-break」に分ける

G7 の ranking は 1 段で全部決めるのでなく、
少なくとも 4 層に分けるのがよい。

### Gate

candidate 化の最低条件。

- boundary side と direction に合っている
- root / direct boundary と重複しない
- graph second hop か narrow fallback の許可条件を満たす
- bridge family を説明する primary evidence を 1 つ以上持つ

### Lane

candidate の大分類。

推奨順は次。

1. graph-first + return / assigned-result continuity
2. graph-first + alias continuation
3. graph-first + require-relative chain
4. narrow fallback companion

ここで重要なのは、
**wrapper という名前かどうか** ではなく、
**どの continuity を閉じるか** を lane にすること。

### Strength

同 lane 内の強弱。

例:

- primary evidence の個数
- return / alias continuity の一致数
- boundary symbol と completion symbol の relation certainty
- explicit `require_relative` edge の有無
- caller/callee side の stop rule に対する適合度

### Tie-break

最後だけ deterministic にする。

- relevant call line
- path
- `via_path`
- `via_symbol_id`

## 3.3 name/path heuristic は primary evidence にしない

`wrap` / `service` / `adapter` / `.rb` は useful な hint ではある。
しかし G7 ではそれを

- gate
- primary lane
- decisive score

に使ってはいけない。

使うとしても

- no other semantic evidence 時の補助
- 同 lane・同 strength での weak hint
- explanation 用の human-facing annotation

くらいに留めるのが安全である。

## 3.4 graph-first と fallback を明確に分ける

Ruby fallback を足すならなおさら、
G7 は

- graph-first candidate
- narrow fallback candidate

を最初から別 lane に置くべきである。

これにより

- fallback が convenience として useful
- でも graph-first の説明可能性は壊さない

という G6 までの bounded philosophy を保てる。

## 3.5 score tuple は output / tests で観測可能にする

G7 の scoring は runtime 内の private logic だけで終わらせてはいけない。
少なくとも次のどこかで観測可能にするべきである。

- `summary.slice_selection.pruned_candidates[*]`
- selected file の `reasons[*]`
- witness explanation の candidate ranking summary
- focused planner unit test
- JSON fixture

見えない scoring に戻ると、また G5/G6 以前の「docs は語るが runtime は黙る」に戻る。

---

## 4. G7 の最小 scoring shape 提案

ここでは G7-2 に渡すため、schema 化しやすい最小 shape を置いておく。

```rust
struct BridgeCandidateScore {
    source_kind: CandidateSourceKind,
    lane: BridgeEvidenceLane,
    primary_evidence_count: u8,
    secondary_evidence_count: u8,
    call_position_rank: u32,
    lexical_tiebreak: String,
}

enum CandidateSourceKind {
    GraphSecondHop,
    NarrowFallback,
}

enum BridgeEvidenceLane {
    ReturnContinuation,
    AliasContinuation,
    RequireRelativeContinuation,
    ModuleCompanionFallback,
}
```

### 解釈

- `source_kind`
  - graph-first か fallback か
- `lane`
  - 何を閉じる candidate か
- `primary_evidence_count`
  - return/alias/require-relative など主要根拠の本数
- `secondary_evidence_count`
  - callsite hint / naming hint など補助根拠
- `call_position_rank`
  - 同 lane 内だけで使う positional hint
- `lexical_tiebreak`
  - 最後の deterministic order

この shape なら

- opaque な単一 weighted score にしなくて済む
- selected vs pruned の理由を人間が追える
- JSON にも簡単に出せる
- planner unit test で tuple 比較を固定しやすい

---

## 5. code / tests / docs の整理方針

G7 の done condition は
「選択優先度が code / tests / docs で整理される」ことだった。
G7-1 の時点では、その整理の土台を次で固定しておくのがよい。

## 5.1 code

G7-3 / G7-6 では少なくとも次を行う。

1. `infer_tier2_bridge_kind()` を semantic signal aware にする
2. `compare_tier2_candidates()` を direct compare から score-profile compare へ置き換える
3. candidate collection 時に `evidence_kind` 群を集める
4. selected / pruned metadata へ score summary を埋める

## 5.2 tests

現状 Rust 側には

- same-side noise vs relevant leaf
- per-seed budget prune

の fixture がある。
G7 ではこれに加えて、少なくとも次が要る。

1. **Rust alias-vs-wrapper competition**
   - wrapper name がある candidate と alias continuity が強い candidate が競合する case
2. **Ruby graph-first vs fallback competition**
   - require-relative second hop と narrow companion fallback が競合する case
3. **selected-vs-pruned explanation fixture**
   - selected と pruned の差が score summary に出る case

## 5.3 docs

G7-2 で schema を定義する時、少なくとも docs では

- `bridge_kind`
- `evidence_kind`
- score tuple の列
- selected-vs-pruned に何を出すか

を README より先に固定するべきである。

---

## 6. G7 task mapping

このメモから、tasklist の後続項目へどう繋ぐかを明示しておく。

## 6.1 G7-2: bridge-kind / evidence-kind schema

このメモの直後にやるべきこと。

決める対象:

- `bridge_kind` enum の見直し
- `evidence_kind` enum 新設
- score tuple の JSON shape
- selected / pruned metadata の追加項目

## 6.2 G7-3: Rust 側の誤選択 1 件改善

最初の実装ターゲットは Rust でよい。
理由は、既に G6-6 の fixture と planner tests があり、
**call-line 偏重から evidence-aware ranking へ移す差分が最も観測しやすい** からである。

## 6.3 G7-6: Ruby narrow fallback 改善

Ruby では scoring と fallback を別々に見るのでなく、
**fallback を ranking lane の一段下に置いたまま改善する** 形がよい。
これで graph-first を壊さずに Ruby を少し伸ばせる。

## 6.4 G7-7: witness explanation 強化

G7-7 は witness 単独の話ではなく、
G7-2/G7-3 で score profile が入ってからやるのが自然である。
selected-vs-pruned explanation はその output surface になる。

---

## 7. 固定しておきたい判断

### 判断 1

**G7 の本丸は scope widening ではなく candidate scoring である。**
G6 の bounded / controlled 2-hop model は維持する。

### 判断 2

**`bridge_kind` と ranking material を分離する。**
`bridge_kind` をそのまま score にしない。

### 判断 3

**name/path heuristic は補助へ降格する。**
primary evidence にはしない。

### 判断 4

**graph-first と fallback は別 lane にする。**
fallback が bounded planner を壊さないようにする。

### 判断 5

**selected-vs-pruned の差は output に残す。**
score tuple の観測面を tests/docs/runtime で揃える。

---

## 8. 一言まとめ

G6 時点の bridge candidate 誤選択は、個々の bug というより
**`bridge_kind` を名前 heuristic と ranking priority の両方に使い、same-side / cross-side / Ruby fallback の競合を `bridge_kind + call line + lexical path` だけで捌いていること** に起因している。

したがって G7 では、bounded な controlled 2-hop を保ったまま、
**`bridge_kind` を label、`evidence_kind` を score の材料に分離し、graph-first vs fallback を別 lane に置いた reviewable な score tuple へ移行する** のが最も筋がよい。