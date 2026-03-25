# G13-1: G12 後の stitched continuation で起きる過剰候補 / 誤選択パターン棚卸しと ranking 設計メモ

このメモは、G12 までで入った

- nested multi-input continuation の最小改善
- alias-family continuation の tier3 拡張
- bridge-execution provenance
- Ruby `require_relative` short-chain cleanup

を前提に、**いまの `main` で stitched continuation がどこで過剰候補化し、どこで誤選択しやすいか** を棚卸しするためのもの。

G11/G12 で bounded continuation はかなり前進した。
もう主題は単純な「file を 1 枚先まで scope に入れられるか」だけではない。
現在の弱点は、むしろ

1. **入るようになった stitched chain の中で、どれを代表として残すか**
2. **弱い stitched chain / duplicate chain をどこで suppress するか**
3. **なぜその chain が勝ったかを provenance でどう説明するか**

に移っている。

要するに G13 は、G12 の continuation/stitching をさらに広げる phase ではなく、
**G12 で見えるようになった chain を bounded なまま rank / budget / compress する phase**
と捉えるのが自然である。

---

## 1. G12 後の current main が実際に持っている stitched continuation contract

まず、G13 が何の上に立っているかを短く固定する。

## 1.1 planner 側はすでに short stitched continuation を admit できる

current main の bounded planner には少なくとも次がある。

- root file
- direct boundary file
- tier2 `bridge_completion_file`
- tier3 `bridge_continuation_file`

さらに G12 で、tier3 continuation anchor は `wrapper_return` だけでなく
`boundary_alias_continuation` まで広がった。

したがって current main は、少なくとも次を bounded scope として admit できる。

- short Rust wrapper-return continuation
- short Ruby `require_relative` continuation
- narrow alias-family continuation beyond tier2

つまり G13 の主題は、scope がまったく足りないことではない。
**scope に入った stitched chain 候補同士の competition** の方が重要になっている。

## 1.2 propagation / witness 側は stitched chain を representative metadata として出せる

G12 で witness には次が入った。

- `bridge_execution_family`
- `bridge_execution_chain_compact`

step family も少なくとも次を持てる。

- `callsite_input_binding`
- `summary_return_bridge`
- `nested_summary_bridge`
- `alias_result_stitch`
- `require_relative_load`

これは大きい前進で、current main はもう
**「どの file が selected だったか」だけでなく「どの continuation/stitch family で閉じたか」**
をある程度言える。

ただし現状の provenance はまだ **winning chain の reconstruction** というより、
**selected path 上に見えた stitched step の representative union** に近い。
ここが G13 で見直したい中心の 1 つである。

## 1.3 ranking はまだ file-local / bridge-local scoring に強く寄っている

current planner の compare は、基本的に `compare_tier2_candidates(...)` に集約されている。
ここで見ているのは主に次である。

- `source_rank`
- `lane_rank`
- primary evidence count
- negative evidence count
- semantic support rank
- secondary evidence count
- call position
- lexical tiebreak

特に重要なのは、lane の順序がまだかなり hard-coded であること。

- `ReturnContinuation = 0`
- `AliasContinuation = 1`
- `RequireRelativeContinuation = 2`
- `ModuleCompanionFallback = 3`

つまり current main は、stitching が見えるようになっていても、
まだ **“どの stitched chain が実際に caller result まで自然に閉じるか” より先に、
局所 candidate の lane と evidence count で勝敗を決めやすい**。

G13 の課題はここから始まる。

---

## 2. G13 で見たいのは recall frontier ではなく selection frontier

G12 で改善した current green/control surface は少なくとも次である。

- Rust nested two-arg continuation without irrelevant-arg leak
- imported-result / alias-result stitching の基本面
- Ruby `require_relative` short continuation の narrow cleanup
- alias-family continuation beyond tier2

この状態だと、G13 で本当に固定すべき frontier は

- 「もう stitched candidate は見える」
- 「しかし弱い chain も一緒に見えやすくなった」
- 「その結果、winner selection が file-local heuristic に引っ張られる」

という面になる。

したがって G13 は、G12 の続きではあるが、主語が少し違う。

- G12 の主語: **continuation chain をどう接続するか**
- G13 の主語: **接続できるようになった chain をどう rank / budget / compress するか**

と切り分けるのがよい。

---

## 3. current main で起きやすい過剰候補 / 誤選択パターン

ここからが本題。

以下は「理論上ありそう」ではなく、現在のコード contract から見て
**G12 後に実際に起きやすい stitched continuation の over-candidate / mis-selection family**
を整理したもの。

## 3.1 return-looking chain が alias-result stitch を先に食いやすい

これは current ranking の最も分かりやすい偏りである。

現在の lane 順序は

- return continuation
- alias continuation
- require_relative continuation
- fallback

で固定されている。

このため、次の shape が起きやすい。

- wrapper 内に return-looking leaf がある
- 同時に imported-result / alias-result stitch の方が caller result closure には重要
- しかし local evidence 上は return lane の方が先に勝つ

G12 では alias-result family を vocabulary として持てるようになったが、
ranking はまだ **return lane を default winner に寄せたまま** である。

### 何が問題か

- alias-result stitch が「補助 family」扱いになりやすい
- selected path は閉じているが、実際に意味が強い chain ではない可能性がある
- current provenance でも alias family が support に回り、winner の説明が曖昧になりやすい

### G13 での設計含意

G13 では少なくとも、
**lane rank より前に “actual stitched closure quality” を見たい**。

具体的には、

- caller result まで閉じるか
- alias zone が wrapper / caller のどこまで連続しているか
- return-looking だが caller-side stitch を持たない chain ではないか

のような chain-local 指標が要る。

## 3.2 per-anchor best-of-one と per-seed tier3 = 1 が chain winner を歪めやすい

current continuation collection はかなり bounded で、これ自体は正しい。
ただし今は次が強い。

- anchor ごとに continuation candidate を実質 1 本だけ残す
- seed 全体でも tier3 continuation は 1 本しか残さない

そのため、次の shape が起きやすい。

- 複数 anchor がそれぞれ stitched continuation を持つ
- local score では A が少し強い
- しかし actual chain closure では B の方が caller-side closure / alias-result continuity を多く持つ
- それでも A が representative を取ってしまう

これは G11/G12 の「bounded に 1 hop 伸ばす」という contract では十分だったが、
G13 では **winner が “強い local file” なのか “強い stitched chain” なのかがズレやすい**。

### 何が問題か

- current cap が file representative と chain representative を兼ねてしまう
- per-anchor `take(1)` が early compression になり、後段で chain 比較できない
- per-seed tier3 = 1 が family coexistence ではなく accidental winner-take-all になりやすい

### G13 での設計含意

G13 では、file candidate をそのまま final representative にせず、
**anchor から復元できる stitched chain candidate を 1 回 compare してから代表を決める**
段が必要である。

## 3.3 same-path duplicate suppression では stitched duplicate を十分に潰せない

current duplicate suppression は主に `path` 単位で効く。
これは G10/G11 までは合理的だった。

ただ、G12 で stitched chain の語彙が増えた結果、次のズレが出る。

- path は違うが、実質同じ closure を語っている chain
- anchor は違うが、caller-side result で同じ stitch zone に収束する chain
- wrapper-local alias chain と caller-local alias chain が別 file representative として並ぶ chain

この場合、same-path suppression では足りない。

### 何が問題か

- semantically duplicate な stitched chain が複数 survive する
- final budget で落ちるまで candidate 数が膨らみやすい
- 落ちた理由も “duplicate chain” ではなく ranked_out / budget に見えやすい

### G13 での設計含意

G13 では `path` ではなく、少なくとも compact には

- entry boundary
- coarse family
- terminal closure target
- caller-result closure の有無

くらいを使った **chain key** を持った方がよい。

つまり G13 の duplicate suppression は、
**same-path duplicate から same-chain duplicate へ半歩進める** のが自然である。

## 3.4 stitched chain の “閉じ方” ではなく local evidence count が勝敗を決めやすい

current compare は primary/secondary evidence 数にかなり依存する。
これは G10 以降の evidence-budget world として一貫している。

ただ、G12 後の stitched continuation では、
**evidence が多い candidate と chain quality が高い candidate が一致しない** 場面が出てくる。

代表例は次のようなもの。

- callsite position / name hint は強いが caller-side closure が弱い
- return-ish な signal は多いが alias-result continuity が途中で切れる
- require_relative load はあるが mixed chain としての value/result closure は弱い
- nested bridge は見えるが relevant input subset が実際の closing chain を作っていない

### 何が問題か

- “情報量が多い file” が “実際に closing chain を作る file” を追い出しやすい
- selected_vs_pruned reasoning は読みやすいが、勝因が still local evidence 寄りになる
- G12 で欲しかった stitched chain の quality が ranking 軸に十分昇格していない

### G13 での設計含意

G13 では evidence count を消す必要はないが、
その前に **chain closure rank / overreach penalty / duplicate penalty** を置く方がよい。

少なくとも compare は

1. どこまで closing chain を作れたか
2. irrelevant / noisy branch を巻き込んでいないか
3. その上で local evidence はどれだけあるか

の順に寄せた方が自然である。

## 3.5 mixed Ruby chain は provenance 上で “混ざって見えすぎる” ことがある

current `bridge_execution_chain_compact` は、selected path 上に見えた step family を compact に積む。
さらに representative family は大まかに

- alias step がある
- require_relative step がある
- nested step がある

といった presence で決まる。

このため、Ruby では次のことが起きやすい。

- require_relative load が選択 path 上にある
- alias-result stitch 的な step も見える
- すると representative family が mixed に寄りやすい
- しかし実際には mixed chain が勝ったというより、selected path に両方の step が見えているだけ

### 何が問題か

- provenance が “winning chain” というより “visible stitched features” を表してしまう
- mixed family の勝因が分からない
- later task で Ruby mis-selection を直しても、出力面で改善理由が追いにくい

### G13 での設計含意

G13 では provenance の family 判定も、
**step の union ではなく winning chain representative** を主語にした方がよい。

少なくとも

- selected winner chain
- auxiliary observed step

を分けられると、かなり読みやすくなる。

## 3.6 selected_vs_pruned reasoning はまだ file-level で、chain-level loser を十分に見せない

current `selected_vs_pruned_reasons` は file selection の説明としてかなり useful である。
ただし stitched continuation ranking という観点では、まだ次が弱い。

- same-path duplicate は compact reason に沈みやすい
- budget loser は witness path の説明面に乗りにくい
- chain-local に「どの stitched family が何に負けたか」が出ない
- “selected file は正しいが selected chain は弱い” ケースを区別できない

### 何が問題か

- G13 の主題である ranking/budget/compress の debug surface が不足する
- file winner は見えるが chain winner が見えない
- regression が file presence 中心に寄り、mis-selection 修正の確認がしにくい

### G13 での設計含意

G13 では少なくとも compact に

- winning chain family
- pruned chain family
- selected_better_by (closure / duplicate / budget / overreach など)

を持てるようにしたい。

---

## 4. G13 で固定したい failure family

G13-2 以降へ進む前に、current failure family を少なくとも次の 5 つに整理して扱うのがよい。

## 4.1 return-looking helper が alias-result closer を押しのけるケース

代表 shape:

- wrapper 内に return-ish helper と alias-result path が両方ある
- caller result closure は alias path の方が強い
- しかし local lane/evidence では return side が勝つ

見るべき点:

- alias closer が representative を取れるか
- return-looking helper が support 止まりになるか
- witness に “なぜ alias chain が勝ったか” が出るか

## 4.2 複数 continuation anchor のうち local score winner が global stitched winner でないケース

代表 shape:

- tier2/tier3 anchor が複数ある
- A は local evidence が強い
- B は caller-side closure まで自然に閉じる
- current cap で A が勝ってしまう

見るべき点:

- per-anchor best-of-one をそのまま final にしないで済むか
- per-seed continuation budget が accidental winner-take-all にならないか

## 4.3 semantically duplicate な stitched chain が path 違いで両方残るケース

代表 shape:

- wrapper-local alias zone と caller-local alias zone の両側に candidate が立つ
- 実質同じ closure を別 representative として持ちやすい

見るべき点:

- same-chain duplicate を suppress できるか
- ranked_out / budget と duplicate suppression を区別できるか

## 4.4 mixed Ruby chain が representative family を盛りすぎるケース

代表 shape:

- `require_relative` load と alias-result stitch の両 step が path 上にある
- しかし実際に勝っているのは pure mixed chain とは言い切れない

見るべき点:

- winning chain family と observed step union を分けられるか
- Ruby の explanation が「混ざっている」だけで終わらないか

## 4.5 nested multi-input continuation で stitched closure quality より local hint が勝つケース

代表 shape:

- nested bridge は見える
- relevant input subset はある
- しかし stronger-looking helper / alternate step が local evidence で勝つ

見るべき点:

- relevant binding を使う chain が優先されるか
- irrelevant branch を巻き込む chain が penalty を受けるか

---

## 5. G13 の設計中心

G13 では project-wide search に行かない。
ここで狙うべきなのは、現在の bounded scope / stitched execution を維持したまま
**chain representative を file representative から少し分離すること** である。

## 5.1 G13 の主語は `Tier2Candidate` 単体ではなく `StitchedChainCandidate` に寄せる

current planner は file candidate を直接 rank している。
G13 ではこの前に、少なくとも concept として
**selected anchor から復元できる stitched chain representative** を作る方がよい。

最低限必要なのは次くらいで十分である。

- `entry_boundary_symbol_id`
- `anchor_symbol_id`
- `family`
- `step_families`
- `terminal_path_or_symbol`
- `reaches_caller_result`
- `reaches_nested_continuation`
- `has_require_relative_load`
- `duplicate_chain_key`
- `negative_chain_signals`

これを持つと、G13 の ranking/budget/provenance を file-local compare から少し剥がせる。

## 5.2 ranking は lane-first ではなく closure-first にする

G13 の ranking は、少なくとも concept 上は次の順がよい。

1. **closure quality**
   - caller result まで閉じるか
   - selected nested continuation が実際に closing chain へ寄与したか
   - mixed chain に必要な load/stitch が揃っているか
2. **overreach / noise penalty**
   - irrelevant arg leak
   - helper-only stitch
   - duplicate chain
   - weak mixed labeling
3. **family fit**
   - return / alias / mixed / nested のどれが actual closure と一致するか
4. **existing local evidence**
   - current primary / secondary / semantic support
5. **lexical / deterministic tiebreak**

この順にすると、G10/G12 の evidence world を壊さずに
**G13 の stitched-chain selection** を載せられる。

## 5.3 duplicate suppression は same-path から same-chain へ半歩広げる

G13 でいきなり完全な graph isomorphism は要らない。
ただし少なくとも、次を chain key に入れた suppress が欲しい。

- seed
- entry boundary
- family
- caller-result closure target
- anchor locality

これにより、

- path は違うが同じ stitched closure を語る candidate
- wrapper-local / caller-local の説明差だけを持つ candidate

を `weaker_same_chain_duplicate` のような形で落としやすくなる。

## 5.4 budget は file 数ではなく stitched family representative を主語にする

current budget はまだ次が強い。

- per-boundary side tier2 = 1
- per-seed tier2 = 2
- per-seed tier3 = 1

これは bounded 性としては分かりやすいが、G13 では file cap が chain cap を兼ねてしまう。

G13 の最小方針は次でよい。

- global bounded cap は維持する
- ただし chain representative は family ごとに 1 件まで残せる余地を作る
- return family と alias-result family を最初から同一 1 枠に押し込めない

ここで重要なのは budget を増やすことではない。
**何を 1 枠と数えるかを file から stitched representative へ寄せること** が大事である。

## 5.5 provenance は selected path ではなく winning chain を主語にする

G13 の provenance では、少なくとも concept 上
次の 2 層を分けた方がよい。

- `winning_bridge_execution_chain_compact`
- `observed_supporting_steps_compact`

前者は本当に勝った chain、後者は path 上で見えた補助 stitched step とみなす。

これにより、例えば Ruby mixed case でも

- mixed chain が本当に winner なのか
- require_relative load が support として見えただけなのか

を分けて説明しやすくなる。

---

## 6. G13 の進め方

tasklist に沿うなら、設計順は次でよい。

## 6.1 まず failure set を ranking/budget 観点で固定する

G13-2 では current stitched continuation の failure set を
**closure failure** ではなく **winner selection failure** として固定する。

少なくとも次を入れたい。

1. return-looking helper vs alias closer
2. competing continuation anchors
3. same-chain duplicate across different files/localities
4. Ruby mixed-family provenance overstatement
5. nested stitched chain with noisy alternate path

## 6.2 Rust で 1 件、chain suppress / mis-selection 改善を先に入れる

G13 はまず Rust の方が進めやすい。
理由は、現在の candidate scoring / lane ranking / continuation anchor が Rust で一番露出しているからである。

ここで狙うのは scope widening ではなく、
**明確な weaker stitched chain suppression** である。

## 6.3 Ruby は mixed-family mis-selection を 1 件だけ改善する

Ruby は G12 で duplicate callsite noise cleanup まで来ているので、
G13 ではそれ以上に scope を広げるより
**mixed stitched explanation / mis-selection の 1 件改善** に絞る方がよい。

## 6.4 per-family / per-chain budget を最小実装で入れる

本格 budget overhaul はまだ重い。
G13 では少なくとも

- winner chain representative
- same-family alternate
- duplicate chain loser

を区別できる最小 budget を入れれば十分である。

## 6.5 provenance compact 表示は最後に揃える

ranking/budget が固まってから provenance を合わせる。
順番を逆にすると、出力 schema だけ先に固まりやすい。

---

## 7. G13 の非目標

この段階では、次は狙わない。

- project-wide recursive chain search
- full argument-binding generalization
- 全言語 parity
- raw budget の単純拡大
- UI/reporting 見た目だけの刷新

G13 は、bounded continuation/stitching を広げる phase ではなく、
**bounded なまま winner selection を賢くする phase** と定義するのがよい。

---

## 8. このメモで固定したい判断

最後に、G13-1 で固定したい判断を短くまとめる。

1. **G12 後の主課題は、scope 追加ではなく stitched chain の winner selection である。**

2. **current ranking はまだ file-local / bridge-local evidence に強く寄っており、actual closure quality を直接見ていない。**

3. **return lane が alias-result stitch を先に食いやすい bias がまだ残っている。**

4. **per-anchor best-of-one と per-seed tier3 = 1 は、file representative と chain representative を混同しやすい。**

5. **same-path suppression だけでは、G12 後の semantically duplicate な stitched chain を十分に潰せない。**

6. **current provenance は selected path 上の stitched step union に近く、winning chain explanation としてはまだ粗い。**

7. **G13 では `Tier2Candidate` 単体ではなく stitched chain representative を compare / budget / explain の主語に寄せるべきである。**

要するに G13 でやるべきことは、

**「stitching できるようになった chain をこれ以上たくさん広げること」ではなく、
「bounded に見えている stitched chain の中から、どれを勝ち筋として残し、どれを duplicate / weaker chain として落とし、
その理由を provenance で短く説明できるようにすること」**

である。

これが揃うと、G12 で得た bounded continuation/stitching の前進を、
precision を落とさず使える ranking/budget/explanation の段へ進められる。
