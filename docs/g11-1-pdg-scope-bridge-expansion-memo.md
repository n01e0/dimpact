# G11-1: 現在の PDG scope / bridge 限界棚卸しと G11 拡張設計メモ

このメモは、`impact --with-pdg` / `--with-propagation` の**現在地**を、
G3/G4 時点の設計メモではなく **main 時点の実装**に合わせて棚卸しし、
G11 でどこを伸ばすかを固定するためのもの。

結論を先に書くと、現在の PDG path はもう単純な
「changed/seed file だけの local DFG」ではない。

- `plan_bounded_slice(...)` により、root file / direct boundary file / bridge completion file / module companion fallback を **理由付きで選ぶ bounded slice planner** が入っている
- `attach_slice_selection_summary(...)` により、選ばれた file と pruned candidate を output/witness 側へ返せる
- `augment_symbolic_propagation(...)` も、direct call-site bridge だけでなく **one-hop completion bridge** まで持つ

一方で、まだ次の限界が強い。

- scope は依然として **bounded frontier** であり、project-wide PDG ではない
- multi-file bridge の多くが、まだ **file 選択 heuristic** と **file-local summary bridge** の組み合わせに依存している
- Rust/Ruby 以外は local DFG enrichment が薄い
- propagation は multi-hop / multi-arg / cross-file def-use を一般には解いておらず、
  現状は **1-hop continuation を賢く選ぶ設計** に寄っている
- engine consistency はなお不完全で、PDG path は base graph を cache から直接読む専用経路を持っている

したがって G11 の本丸は、bounded 前提を崩さずに

1. **scope を広げる理由**
2. **bridge を足す理由**
3. **その bridge が実際に impact recall を増やした理由**

を同じ語彙で扱えるようにすることになる。

---

## 1. 現在の PDG path で既に前進している点

## 1.1 Scope planner は既に first-class になっている

現在の `build_pdg_context()` は、昔のように単なる path list をそのまま DFG build に流すだけではない。

実装上は次の流れになっている。

1. root 側の `cache_update_paths` / `local_dfg_paths` / seed file を初期集合として cache を更新する
2. `cache::load_graph()` で symbol index / refs を読む
3. `plan_bounded_slice(...)` が bounded slice plan を作る
4. 追加で必要になった file だけ cache update する
5. `plan.local_dfg_paths` に対して Rust/Ruby local DFG を build する
6. `PdgBuilder::build()` + `augment_symbolic_propagation()` で PDG を組む

ここで planner が返すのは path 集合だけではなく、`ImpactSliceSelectionSummary` を通じた

- file ごとの selection reason
- tier
- bridge kind
- scoring
- pruned candidate

である。

これは G4-3 で欲しかった「scope は理由付きで決める」を、かなり実装に落とし込めている状態。

## 1.2 Scope は root / direct boundary / bridge completion / fallback に分かれた

`plan_bounded_slice(...)` は現在、概ね次の tier を持つ。

- **Root**
  - `SeedFile` / `ChangedFile`
- **Direct boundary**
  - `DirectCallerFile` / `DirectCalleeFile`
- **Tier2 bridge completion**
  - `BridgeCompletionFile`
  - bridge kind: `WrapperReturn`, `BoundaryAliasContinuation`, `RequireRelativeChain`
- **Fallback**
  - `ModuleCompanionFile`

加えて次の budget も入っている。

- per boundary side: Tier2 file 最大 1
- per seed: Tier2 file 最大 2

つまり現状は、G4-3 の「1-hop boundary + bounded bridge completion」が、
G9/G10 の evidence / pruning を含む形でかなり具体化された状態と言える。

## 1.3 Scope decision の説明力は G3/G4 よりかなり上がった

`attach_slice_selection_summary(...)` により、output 側には最低限次が返る。

- どの file が slice に選ばれたか
- cache_update / local_dfg / explanation のどの scope に属していたか
- どの seed に対する理由だったか
- 採用された bridge completion 候補と、落とされた候補

これは「なぜこの file が scope に入ったのか」という観点では、
G4-1 で問題視されていた状態より明確に前進している。

## 1.4 Propagation も 1 段だけだが multi-file continuation を持つ

`augment_symbolic_propagation(...)` は現在、少なくとも次をやっている。

- callsite use -> callee symbol
- callee symbol -> callsite def
- callee summary input -> impacted node 経由の callsite bridge
- function/method symbol -> in-span node bridge
- `push_one_hop_completion_bridges(...)` による **one-hop nested completion**

最後の one-hop completion は大きい。
これは「boundary callee の中でさらに 1 回呼ばれた先」を summary 経由で callsite def まで戻すための橋で、
現在の bounded slice planner が選んだ Tier2 file と噛み合う設計になっている。

つまり main 時点の PDG path は、
単なる local summary bridge からはすでに一段進んでいる。

---

## 2. 現在の PDG scope / bridge の本質的な限界

ここからが G11-1 の本題。

## 2.1 Scope は改善されたが、依然として「bounded frontier」でしかない

今の planner は explicit で explainable だが、基本的にはまだ

- root file
- そこから 1-hop の direct boundary file
- その先の Tier2 completion file を各 side 1 件、seed 全体でも 2 件まで

という **狭い frontier** に強く縛られている。

この boundedness 自体は悪くない。
G11 でも project-wide PDG にしない前提は維持すべき。

ただし現状の限界は明確で、次のようなケースに弱い。

- wrapper を 2 段以上跨いでやっと return / alias continuity が閉じるケース
- 1 つの seed に対して Tier2 候補が 3 件以上必要でも、per-seed 2 件で打ち切られるケース
- direct boundary から見て「どの side に 1 件選ぶか」では足りず、同じ side に 2 bridge family 必要なケース
- path ごとの budget は足りるのに、bridge kind ごとの budget が無いため ranking が歪むケース

要するに現在の bounded planner は、
**「深さ制御」はあるが「frontier の意味論」がまだ粗い**。

## 2.2 Tier2 selection がまだ lexical / callsite heuristics に強く依存している

Tier2 の scoring は、semantic evidence だけで決まっていない。
現状ではかなり次に依存する。

- symbol / path の名前ヒント (`wrap`, `leaf`, `alias`, `helper`, `noise` など)
- callsite の位置ヒント
- Ruby の `require_relative` 正規表現ヒント
- module companion fallback

もちろん semantic signal もある。
例えば Rust では `collect_rust_tier2_semantic_evidence(...)` 由来の
`param_to_return_flow` や local DFG support が入る。

ただ、なお大きいのは **候補 file の ranking が bridge の実証ではなく、候補らしさの scoring である** 点。

このため現在の planner は、
「bridge を実際に張れた file を選ぶ」というより、
**bridge が張れそうな file を bounded に選ぶ** 性格が強い。

これは G10 までの設計としては妥当だったが、G11 ではそろそろ
**bridge-ready frontier** をもう少し直接測る必要がある。

## 2.3 Local DFG enrichment はまだ Rust/Ruby に閉じている

`supports_local_dfg()` は現状 `.rs` / `.rb` のみを true にする。
したがって planner 自体は全言語の file を selection summary に出せても、
実際に local DFG が立って PDG enrichment を受けるのは Rust/Ruby 中心である。

これは README で既に caution している通りだが、G11 の設計として改めて重要なのは

- scope planner の多言語化
- bridge planner / propagation の多言語化
- local DFG builder の多言語化

が同じ問題ではない、ということ。

G11 ではここを混ぜない方がよい。
少なくとも G11 の主対象は **Rust / Ruby の multi-file continuation 強化** に絞るのが筋。

## 2.4 DFG は依然として file-local であり、cross-file def-use 自体を持っていない

planner が複数 file を選んでも、DFG builder 自体は file ごとに node/edge を作るだけで、
**cross-file の生 DFG edge** を構築しているわけではない。

今の multi-file continuity は主に

- global call graph ref
- file-local function summary
- symbolic propagation bridge
- bounded slice による関連 file 選択

の組み合わせで成立している。

この構造の帰結として、今の PDG はまだ
**global call graph + selected local DFG islands + symbolic bridges**
であり、project-sliced PDG ではあっても **cross-file DFG** ではない。

この限界は G11 でも正面から認めるべき。
G11 の目標は cross-file DFG を作ることではなく、
**file island の間に張る bridge の質を上げること** である。

## 2.5 Propagation bridge はまだ 1-hop / line-matched / summary-biased である

`augment_symbolic_propagation(...)` の現在の橋は有用だが、かなり条件が狭い。

特に強い制約は次。

- callsite use/def の発見が `(file, line)` に強く依存する
- summary bridge は `callsite_uses.len() == summary.inputs.len()` か、単入力 1 本の fallback に寄っている
- `push_one_hop_completion_bridges(...)` は
  - outer callee の impacted node と同じ `(file, line)` に nested call ref があること
  - nested summary が単入力であること
  - completion を 1-hop だけ見ること
  を前提にしている

このため今の propagation は、
**wrapper-return / assigned-result / param-to-return の短い continuation** には比較的強いが、
次の一般化にはまだ遠い。

- multi-hop continuation
- multi-input / reordered argument mapping
- field / member / container 経由の return stitching
- callsite line が分散する表現
- boundary を跨いだ def-use explanation の保持

G11 で bridge を強くするなら、ここを無視して planner だけ賢くしても頭打ちになる。

## 2.6 Planner と propagation が同じ bridge taxonomy をまだ共有していない

planner 側には既に

- `WrapperReturn`
- `BoundaryAliasContinuation`
- `RequireRelativeChain`
- `ModuleCompanionFile` fallback

という bridge family がある。

しかし propagation 側は、これと 1 対 1 に対応する bridge taxonomy を明示的には持っていない。
今の propagation は DFG edge を追加するが、
それが

- wrapper return を閉じる橋なのか
- alias continuation を閉じる橋なのか
- nested completion を閉じる橋なのか

を planner と同じ言葉で管理していない。

結果として、現在の設計は

- planner は bridge family を理由として語れる
- propagation は bridge family を edge contract としては持っていない

という半歩ズレた状態にある。

G11 の scope/bridge 拡張では、このズレがかなり重要になる。

## 2.7 Engine consistency はまだ未解決のまま残っている

現在も `build_pdg_context()` は

- cache を直接開く
- `cache::load_graph()` から base refs を読む
- その上に planner / PDG / propagation を載せる

という専用経路である。

通常 impact の `AnalysisEngine` contract と完全には揃っていないので、
次の限界が残る。

- `--engine` の意味が PDG path ではなお弱い
- changed symbol 決定と base graph 取得が engine policy と一体化していない
- 将来 engine 側で richer certainty / provenance を持っても、PDG 側へ自然には流れ込みにくい

これは G11 の主題ではないが、scope/bridge 拡張の天井を決める前提なので、
設計メモからは外さない方がよい。

## 2.8 Output explanation は「file を選んだ理由」には強いが、「edge を張れた理由」にはまだ弱い

`slice_selection_summary` は現在かなり useful だが、主に説明しているのは

- なぜこの file が slice に入ったか
- どの candidate が pruned されたか

である。

一方で最終的に impact result へ出る explanation は、まだ

- 実際に採用された symbolic bridge chain
- どの summary input/output が効いたか
- one-hop completion がどの nested call を跨いだか

までを first-class には返していない。

言い換えると、今の説明力は
**scope selection explanation** には強いが、
**bridge execution explanation** はまだ弱い。

G11 で bridge を強くするなら、この差はさらに目立ちやすくなる。

---

## 3. G11 で狙うべき設計の中心

G11 の基本方針は次でよい。

## 3.1 Project-wide には行かず、「bounded bridge-ready slice」を強くする

G11 でも project-wide PDG / whole-program symbolic execution は狙わない。
代わりに狙うのは、現在の bounded slice を

- より bridge-aware にする
- bridge family ごとの continuation を見分ける
- その結果を propagation と witness に反映する

ことである。

つまり G11 のテーマは
**scope widening** そのものではなく、
**bridge-ready frontier の精度向上** と置くのが正しい。

## 3.2 Planner と propagation を「同じ bridge family」で接続する

G11 で一番重要なのはこれ。

今の planner は bridge family を語れるのに、propagation はそれを edge contract にしていない。
G11 では少なくとも内部設計として、

- planner がどの bridge family を狙って file を選んだか
- propagation がどの bridge family を実際に成立させたか
- witness/output がどの bridge family で勝ったか

を同じ taxonomy で扱えるようにした方がよい。

候補は現状の延長で十分。

- `wrapper_return`
- `boundary_alias_continuation`
- `require_relative_chain`
- `callsite_summary_continuation`（新設候補）
- 必要なら `module_companion_fallback` を「bridge ではない fallback」として別枠で扱う

これにより、G11 では「scope を広げた」だけでなく
**どの bridge family を伸ばした結果 recall が増えたか** を固定しやすくなる。

## 3.3 G11 の改善単位は「file 追加」ではなく「boundary continuation 追加」にする

G4/G9/G10 の流れでは、設計単位がどうしても「どの file を選ぶか」に寄りやすかった。
しかし G11 では、より直接的に

- seed -> direct boundary
- direct boundary -> completion frontier
- completion frontier -> local DFG / summary / propagation continuation

という **boundary continuation** を単位にした方がよい。

こうしておくと、同じ file を入れる場合でも

- wrapper return のために入れたのか
- alias continuation のために入れたのか
- require_relative chain のために入れたのか

を混同せずに済む。

## 3.4 評価面は「scope」「bridge」「explanation」を分けて固定する

G11 では regression を次の 3 層で分けた方がよい。

1. **scope selection**
   - どの file が選ばれたか
   - どの candidate が pruned されたか
2. **bridge formation**
   - PDG dot / JSON edges に期待 bridge が立ったか
3. **impact recall / explanation**
   - impacted symbols が増えたか
   - witness / summary がその bridge family を説明できるか

現状は 1 がかなり強く、2 と 3 がまだ薄い。
G11 はそこを揃えるフェーズとして置くのが自然。

---

## 4. G11 で進める設計順

tasklist に沿うと、設計順は次が妥当。

## 4.1 G11-2: current weak cases を固定する

先に fixed evaluation set を置く。
ここでは特に

- Rust multi-file wrapper return continuation
- Rust boundary alias continuation
- Ruby require_relative / runtime continuation
- propagation と slice selection が噛み合わない case
- explanation は出るが bridge 自体が立たない case

を 3〜5 件に絞って固定する。

重要なのは、
**scope が足りない case** と **bridge が弱い case** を混ぜないこと。

## 4.2 G11-3: bounded planner を「bridge-ready frontier planner」として再定義する

ここで planner contract を少し整理する。

今の `BoundedSlicePlan` を壊す必要はないが、少なくとも概念上は

- root / boundary / completion
- completion の bridge family
- completion を選ぶ根拠（semantic / lexical / fallback）

を明示的に分けた方がよい。

ポイントは budget を path 単位だけでなく、必要なら
**bridge family 単位でも見直せる構造** にすること。

## 4.3 G11-4 / G11-5: Rust / Ruby で priority bridge を 1 つずつ強くする

ここでは breadth ではなく priority で攻める。

- Rust: wrapper return か boundary alias continuation のどちらかで、
  現在落ちる multi-file case を 1 件まず確実に改善する
- Ruby: require_relative chain か runtime continuation 周りで、
  fallback だけに頼らない case を 1 件強くする

この段階では bridge family を増やしすぎない方がよい。
まずは **既存 family の成立条件を実コードで強くする** のが先。

## 4.4 G11-6: propagation と planner の接合を見直す

ここが G11 の設計コア。

具体的には、planner が選んだ completion frontier を propagation が受け取って

- その frontier を閉じる summary bridge を優先する
- one-hop completion を bridge family と整合した条件で張る
- 必要なら bridge family を edge provenance / witness metadata に残す

という方向がよい。

少なくとも、G11 後は
「planner は wrapper return を選んだのに propagation は generic data edge を足しただけ」
というズレを縮めたい。

## 4.5 G11-7 以降: regression / docs / rollup で閉じる

最後に

- regression を増やす
- README/README_ja の current limits を main 実装に合わせて更新する
- G11 rollup で、bounded planner のままどこまで伸びたかを整理する

で閉じる。

---

## 5. G11 の非目標

この段階では、次は狙わない。

- 無制限の project-wide PDG
- Go/Java/Python/JS/TS/TSX の local DFG parity
- planner と engine の完全統合
- witness / reporter の UI 改修中心の作業
- bridge family を大量追加して複雑化すること

G11 は、bounded planner を捨てるフェーズではなく、
**bounded planner を bridge-aware に育てるフェーズ** と定義するのがよい。

---

## 6. G11 で固定したい設計判断

最後に、このメモで固定したい判断を短くまとめる。

1. **現在の PDG path は、もう単なる seed-file local DFG ではない。**
   bounded slice planner / evidence / pruned candidate / one-hop completion bridge を持つ。

2. **それでも本質はまだ bounded frontier + local DFG islands である。**
   project-wide PDG ではないし、cross-file DFG でもない。

3. **現在の弱点は「scope planner が無いこと」ではなく、planner と propagation が同じ bridge contract を持っていないこと。**

4. **G11 の主戦場は Rust/Ruby の multi-file continuation 強化であり、全言語 parity ではない。**

5. **G11 では file 選択の改善だけでなく、bridge family 単位で recall 増加を測る。**

6. **説明力も scope selection だけで止めず、bridge execution 側へ少し寄せる。**

要するに、G11 でやるべきことは

**「どの file を選ぶか」をさらに賢くすることだけではなく、
その file を選んだ理由と、そこで実際に閉じた bridge を同じ設計語彙に揃えること**

である。

これが揃うと、bounded な前提を維持したままでも
現在より一段自然な multi-file PDG / propagation に進める。