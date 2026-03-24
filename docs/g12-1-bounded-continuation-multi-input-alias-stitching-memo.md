# G12-1: 現在の bounded continuation で弱い multi-input / alias-result stitching ケース棚卸しと G12 設計メモ

このメモは、main 時点の bounded continuation planner / propagation が
**どこまで出来るようになり、どこでまだ痩せるか** を G12 観点で棚卸しするためのもの。

G11 までで、bounded slice path はかなり前進した。
今の main にはすでに次がある。

- `plan_bounded_slice(...)` による root / direct boundary / bridge completion / bridge continuation の bounded planner
- `collect_bridge_continuation_candidates(...)` による same-family 1-hop continuation file の追加
- `PdgBuilder::augment_symbolic_propagation(...)` による callsite use/def bridge と nested completion bridge
- `slice_selection_summary` / witness slice context による selected/pruned 理由の説明

その結果、少なくとも次の success case は current main で regression 化されている。

- Rust two-hop wrapper return continuation
  - `tests/cli_pdg_propagation.rs::pdg_propagation_extends_two_hop_wrapper_return_through_rust_bridge_continuation_scope`
- Ruby two-hop require_relative wrapper return continuation
  - `tests/cli_pdg_propagation.rs::pdg_propagation_extends_two_hop_require_relative_wrapper_return_scope`
- imported result -> caller alias chain の代表ケース
  - `tests/cli_pdg_propagation.rs::pdg_propagation_extends_imported_result_into_caller_alias_chain`

つまり G12 の主題は、もう「2-hop continuation file を scope に入れられるか」だけではない。
今の main で本当に弱いのは、

1. **multi-input continuation をどう bind / stitch するか**
2. **alias-result stitching を return continuation から独立にどう扱うか**
3. **採用された continuation chain をどう provenance として残すか**

である。

---

## 1. current main の bounded continuation が実際にやっていること

G12 の棚卸しは、まず current main の contract を短く固定した方がよい。

## 1.1 planner 側の current contract

`plan_bounded_slice(...)` は現在、おおむね次の tier を持つ。

- **Root**
  - `SeedFile` / `ChangedFile`
- **Direct boundary**
  - `DirectCallerFile` / `DirectCalleeFile`
- **Tier2 bridge completion**
  - `BridgeCompletionFile`
  - `bridge_kind = wrapper_return | boundary_alias_continuation | require_relative_chain`
- **Tier3 bridge continuation**
  - `BridgeContinuationFile`
  - admit 済み tier2 candidate を anchor に、same-family 1 hop を追加

この tier3 は G11 までの大きい前進で、Rust/Ruby の two-hop wrapper-return case を
bounded なまま拾えるようになっている。

## 1.2 propagation 側の current contract

`PdgBuilder::augment_symbolic_propagation(...)` は現在、少なくとも次をやっている。

1. callsite use -> callee symbol
2. callee symbol -> callsite def
3. callee summary input -> impacted node -> callsite def
4. `push_nested_completion_bridges(...)` による nested completion bridge

ここで重要なのは、current main の continuation execution が
**planner が広げた bounded frontier** と
**summary bridge / callsite def attachment** の組み合わせで成立していること。

したがって今の path は、project-wide PDG ではなく

- bounded に選んだ file island
- local DFG
- call graph
- symbolic continuation bridge

を合成したものだと見た方が正確である。

## 1.3 G12 で見たいのは scope ではなく execution contract

G11 で解けた代表ケースを見ると、current bounded continuation は

- one relevant input
- callsite 近辺で use/def が拾える
- return continuation family が素直に閉じる

という shape にはかなり強い。

逆に、まだ弱いのは

- relevant input が 2 本以上ある／あるいは 1 本だけど multi-input function の中にいる
- imported result が alias chain と混ざる
- planner 上は alias family が見えているのに propagation/witness では generic bridge に潰れる

という shape である。

G12 はこの差を埋める task と定義するのが自然である。

---

## 2. current main の bounded continuation でまだ弱い面

ここからが G12-1 の本題。

以下は「理論上弱そう」ではなく、
**現在のコードの hard-coded contract から見て、まだ痩せやすい面**を整理したもの。

## 2.1 nested multi-input continuation はまだ本質的に弱い

一番明確なのはこれ。

`augment_symbolic_propagation(...)` の top-level summary bridge には、まだ次の強い前提がある。

- `callsite_uses.len() == summary.inputs.len()` なら zip して対応づける
- そうでなければ `summary.inputs.len() == 1 && callsite_uses.len() == 1` の fallback に寄る
- さらに nested completion へ進む `push_nested_completion_bridges(...)` は
  **`nested_summary.inputs.len() == 1` を必須条件**にしている

つまり current main は、top-level では multi-input を少し扱えても、
**nested continuation になると単入力 contract に戻ってしまう**。

この帰結として、次の shape がまだ弱い。

- `wrap(a, b)` の中で `pair(a, b)` を呼ぶ
- 真に relevant なのは `b` だけ
- `pair` 側の result を `v` 経由で caller result に戻したい

この case は G11-2 でも `rust-nested-two-arg-summary-continuation` として既に棚卸しされていたが、
current main の code contract を見る限り、まだ task として残っていると考えてよい。

### current weakness の中身

- nested summary が multi-input の時点で continuation bridge が止まる
- relevant arg だけを選んで caller result へ戻す binding が無い
- irrelevant arg を巻き込まない guarantee も generic zip 以上には持てない

したがって G12 では、
**multi-input continuation を “scope の問題” ではなく “input binding の問題” として扱う**必要がある。

## 2.2 callsite input binding が file/line + order heuristic に強く依存している

current main の `collect_callsite_uses(...)` / `collect_callsite_defs(...)` は、
callsite 周辺の `(file, line)` から use/def node を拾う実装になっている。

さらに summary input への対応づけは、基本的に
**集めた use node 列を summary.inputs と順番で zip する**形で行われる。

この contract で弱くなりやすいのは次のケース。

- 引数順が wrapper / nested callee で入れ替わる
- relevant arg 以外の use node が callsite line に混ざる
- multiline call / chained call / temporary binding で callsite node の並びが崩れる
- 同じ variable が複数回渡される
- 一部の引数が literal / field access / container read で、単純な use node 列に揃わない

要するに current main は、まだ
**“どの caller input がどの summary input に bind されたか” を first-class には持っていない**。

G11 までの one-hop return continuation ではこれで足りたが、
G12 の multi-input continuation ではこのままだと厳しい。

### current weakness の中身

- `summary.inputs` と `callsite_uses` の順序対応が semantic contract ではない
- partial binding / repeated binding / reordered binding を区別できない
- witness にも「どの input binding を採用したか」が残らない

そのため G12 では、
**line-based use collection** と **semantic input binding** を分けて考えた方がよい。

## 2.3 bounded continuation tier は alias family をほぼ延長できない

planner 側で次に大きいのがこれ。

current `collect_bridge_continuation_candidates(...)` は、
continuation anchor を `continuation_ready_wrapper_return_anchor(...)` で絞っている。
この helper は実質的に、次だけを continuation source にしている。

- `source_kind == graph_second_hop`
- `bridge_kind == wrapper_return`
- Rust/Ruby file

つまり current main の tier3 continuation は、
**wrapper_return family にかなり特化している**。

一方で tier2 planner 自体はすでに

- `boundary_alias_continuation`
- `require_relative_chain`

を持っている。

にもかかわらず、current tier3 はこれらを continuation anchor に出来ない。

### このズレが意味すること

- alias family が tier2 で勝っても、その先へ 1 hop 進む contract が無い
- require_relative chain も、wrapper-return 的 shape に落ちる case 以外は continuation しにくい
- planner vocabulary と continuation execution vocabulary が一致していない

これは G11-1 でも予告されていた「planner と propagation が同じ bridge contract を共有していない」問題の、
いま残っている一番具体的な形と言える。

G12 で alias-result stitching を主題にするなら、
この時点で **tier3 を return family 専用の extension と見なしたままでは足りない**。

## 2.4 alias-result stitching はまだ “callsite def へ戻す” 以上の contract を持っていない

current propagation は、imported result 系の成功ケースを 1 つ通せるようになっている。
これは良い前進で、G12 でも control case として維持すべき。

ただし今の stitching の本体は、まだかなり
**impacted node -> callsite_defs** に依存している。

このため次の shape がまだ弱い。

- imported result が wrapper 内 alias chain を通る
- そのあと caller 側でも別の alias/result chain に入る
- あるいは wrapper 側 alias が temp / reassignment / post-call line split を挟む

単純な `value -> y -> alias -> out` なら local DFG が助けるが、
current main には
**alias-result stitching 自体を first-class bridge family として表す contract** がまだ無い。

そのため今の path は、

- planner では alias continuation を語れる
- propagation では generic data bridge の積み上げとしてしか見えない
- witness でも採用された alias stitch chain が明示されない

という半歩ズレた状態にある。

### current weakness の中身

- alias bridge の成立条件が return continuation より曖昧
- imported result -> alias -> caller result を 1 本の execution family として扱えない
- 途中の alias zone が 2 箇所以上あると continuity が偶然 local DFG に依存しやすい

G12 で見るべきは、単に alias candidate を選べるかではなく、
**selected alias family を実際に stitch execution へ落とせるか** である。

## 2.5 same-path suppression / budget が alias family を潰しやすい

current planner には次の budget がある。

```rust
const PER_BOUNDARY_SIDE_TIER2_FILES_MAX: usize = 1;
const PER_SEED_TIER2_FILES_MAX: usize = 2;
const PER_SEED_TIER3_FILES_MAX: usize = 1;
```

さらに same-path duplicate suppression / same-family Rust sibling suppression があるため、
次のことが起きやすい。

- same path 上で return continuation が少し強いと alias continuation が suppressed される
- per-seed Tier2 が 2 枚しか無いため、return family が先に枠を使う
- continuation tier も 1 枚だけなので、alias family 用の representative が残らない

要するに current planner は、まだ
**「return family と alias-result family は補完的に両方要るかもしれない」**
という budget を持っていない。

G10 までの evidence-driven pruning としては合理的だったが、
G12 の stitching 改善対象としては少し強すぎる。

### current weakness の中身

- path budget が family budget を兼ねてしまっている
- alias family が “弱い代替候補” として扱われやすい
- 実際には return bridge と alias-result stitch の両方が必要な case を落としうる

G12 では、少なくとも evaluation 上は
**return family と alias-result family が同居しないと閉じない case** を 1 件以上固定した方がよい。

## 2.6 bridge execution provenance がまだ粗い

current witness はかなり改善されている。

- `path` / `path_compact`
- `provenance_chain_compact`
- `slice_context`
- `selected_vs_pruned_reasons`

このあたりは、G7/G8/G10/G11 の成果として十分 useful である。

ただし G12 の multi-input / alias-result stitching を進めるには、まだ粗い。
今の witness からは次が分からない。

- どの callsite input がどの summary input に bind されたか
- nested multi-input で、どの input subset を採用したか
- alias-result stitching のどの chain が実行されたか
- selected continuation anchor からどの bridge execution step が積み上がったか

今ある provenance は主に
**edge provenance (`call_graph` / `local_dfg` / `symbolic_propagation`)** であり、
**continuation execution provenance** ではない。

G12 で bridge chain を強くするなら、ここは避けて通れない。

---

## 3. G12 で固定したい failure family

G12 の後続 task では、failure family を少なくとも次の 5 つに整理して扱うのがよい。

## 3.1 nested multi-input continuation

代表 shape:

- `main -> wrap(a, b) -> pair(a, b)`
- relevant arg は `b`
- wrapper temp `v` を経て caller result `out` へ戻したい

見るべき点:

- nested summary が multi-input でも continuation が閉じるか
- irrelevant arg `a` を巻き込まないか
- witness に selected input binding を残せるか

## 3.2 reordered / partial input binding continuation

代表 shape:

- `wrap(a, b)` が `pair(b, a)` を呼ぶ
- あるいは `pair(a, 1, b)` のように literal を挟む
- relevant arg だけが result に効く

見るべき点:

- input position の並び替えに耐えるか
- partial binding を持てるか
- zip-by-order の accidental success に依存していないか

## 3.3 alias-result stitching across wrapper + caller

代表 shape:

- imported result -> wrapper temp -> wrapper alias -> caller temp -> caller out

見るべき点:

- alias zone が 2 箇所あっても continuity が閉じるか
- planner と propagation が同じ alias-result family を共有できるか
- witness が alias stitch chain を説明できるか

## 3.4 alias family continuation beyond tier2

代表 shape:

- tier2 で `boundary_alias_continuation` が選ばれる
- その先に same-family leaf / value / helper file がもう 1 hop 必要

見るべき点:

- current tier3 の wrapper-return 専用性が原因で落ちていないか
- alias family を continuation anchor に出来るか
- family-aware budget で representative を残せるか

## 3.5 require_relative + alias-result mixed stitching

代表 shape:

- Ruby `require_relative` split
- imported result が alias/temp を経て caller result へ戻る
- wrapper-return と alias-result stitching が混ざる

見るべき点:

- require_relative provenance と alias-result family が同時に必要な時に崩れないか
- Ruby が lexical fallback だけでなく actual stitch contract を持てるか

---

## 4. G12 の設計中心

G12 は project-wide PDG に行くフェーズではない。
ここで狙うべきなのは、今の bounded frontier を維持したまま
**continuation execution contract を 1 段強くすること** である。

## 4.1 G12 の主語は “file selection” ではなく “continuation execution step” にする

G11 までは、かなりの割合で
「どの file を scope に入れるか」が主設計語彙だった。
これは必要だったし、実際に効いた。

ただ、G12 で本当に足りないのは file selection そのものではない。
必要なのは、selected file 群の上で

- どの input binding を採用したか
- どの summary bridge を跨いだか
- どの alias-result stitch を採用したか

を step として表現することである。

したがって G12 では、内部設計として少なくとも
**continuation execution step** を first-class にした方がよい。

候補は次のような粒度で十分である。

- `callsite_input_binding`
- `summary_return_bridge`
- `nested_summary_bridge`
- `alias_result_stitch`
- `require_relative_load`

planner/witness へ public schema をすぐ全部出さなくてもよいが、
内部 contract にはこのくらいの step vocabulary が欲しい。

## 4.2 multi-input は “input binding map” として持つ

G12 の multi-input continuation は、
単に `summary.inputs.len() > 1` を許すだけでは足りない。

必要なのは、例えば次のような map である。

- caller use node / symbol
- callee summary input node
- binding kind
  - positional
  - reordered positional
  - repeated binding
  - partial / dropped
  - literal-or-unknown companion

これを持つと、次が出来る。

- relevant input subset だけを nested continuation へ通す
- irrelevant input を caller result bridge へ戻さない
- witness で「`main.rs:use:y` が `pair.rs:def:b` に bind された」と言える

要するに G12 の multi-input は、
**summary input count の問題ではなく binding map の問題** として扱うべきである。

## 4.3 alias-result stitching を独立 family として扱う

current main の vocabulary には `boundary_alias_continuation` があるが、
execution 側ではまだ return continuation の亜種に寄りがちである。

G12 では少なくとも内部 contract 上、
次を独立 step として扱う方がよい。

- imported result を temp/alias/result chain に入れる
- その chain を wrapper 内 / caller 内で継続する
- 必要なら return bridge と合流する

つまり G12 では
**alias-result stitching = selected file reason ではなく、実行される bridge family**
として定義し直した方がよい。

この設計にすると、次が整理しやすい。

- planner が alias family の representative を残す理由
- propagation が alias-result stitch edge を張る理由
- witness が return continuation と alias-result stitch を分けて説明する理由

## 4.4 continuation anchor を wrapper-return 専用にしない

G12 で planner 側に入れたい最小の判断はこれ。

- tier3 continuation anchor を `wrapper_return` だけに固定しない
- 少なくとも `boundary_alias_continuation` を continuation anchor 候補にする
- Ruby では `require_relative_chain` と alias-result stitching の混合も検討対象にする

ここで重要なのは、むやみに再帰を深くしないこと。
G12 でも boundedness は守る。

したがって方針は

- continuation depth は小さいまま
- ただし **anchor family** は増やす
- その代わり family-aware budget を持つ

でよい。

## 4.5 family-aware budget を入れる

current path budget は分かりやすいが、G12 の multi-input / alias-result stitching には少し粗い。

最小案は次。

- per-seed global cap は維持する
- ただし representative は family ごとに 1 つまでは残せるようにする
- `wrapper_return` と `alias_result_stitch` を相互排他の同一枠にしない

初期形としては、例えば

- `wrapper_return`: 1
- `alias_result_stitch`: 1
- `require_relative_chain`: 1

のような conservative budget で十分である。

大事なのは、G12 では path 数ではなく
**bridge family の意味**で budget を切ることだ。

## 4.6 bridge execution provenance を witness に少し載せる

G12 の最終 task では、少なくとも compact な形で
次を witness/edge metadata に残せるようにしたい。

- selected continuation anchor
- selected input binding map（全部でなく relevant 部分だけでもよい）
- selected bridge execution chain compact
- chain ごとの family / certainty

例えば compact form なら、次の程度で十分である。

```json
{
  "bridge_execution_chain_compact": [
    {
      "family": "nested_multi_input_continuation",
      "via_symbol_id": "rust:wrap.rs:fn:wrap:1",
      "binding": ["main.rs:use:y -> pair.rs:def:b"]
    },
    {
      "family": "alias_result_stitch",
      "via_symbol_id": "rust:wrap.rs:fn:wrap:1",
      "stitch": ["wrap.rs:def:v -> wrap.rs:def:alias -> main.rs:def:out"]
    }
  ]
}
```

public JSON をこのままにする必要はないが、
G12 のゴールは少なくとも
**どの bridge chain が採用されたか説明しやすい状態** である。

---

## 5. G12 で進める順番

tasklist に沿うなら、設計順は次でよい。

## 5.1 まず failure set を固定する

G12-2 では、少なくとも次の 3〜5 ケースを fixed set にする。

1. Rust nested two-arg summary continuation
2. Rust reordered / partial input binding continuation
3. Rust alias-result stitching across wrapper + caller
4. Rust alias-family continuation beyond tier2
5. Ruby require_relative + alias-result mixed stitching

ここでは
**scope が足りない case** と **execution contract が足りない case** を混ぜない方がよい。
G12 の中心は後者である。

## 5.2 propagation 側に binding map を入れる

最初のコード変更は planner より propagation の方がよい。
理由は、current main の未解決面の中心が

- nested multi-input
- alias-result stitching
- provenance

だからである。

まずは internal-only でもよいので

- callsite input binding
- nested summary binding
- alias-result stitch step

を構造化してから、その metadata を planner/witness へ流す方が自然である。

## 5.3 planner は family-aware continuation へ拡張する

次に planner 側で

- alias family を continuation anchor に含める
- family-aware representative budget を持つ
- selected continuation reason に family / anchor / execution-ready 情報を入れる

をやる。

G12 では planner を全面的に作り直す必要は無い。
今の `plan_bounded_slice(...)` の延長で十分である。

## 5.4 provenance を compact に返す

最後に witness/output へ次を返す。

- selected bridge execution chain
- input binding summary
- selected alias-result stitch reason

ここまで行くと、G12 は単なる recall 改善ではなく
**explainable continuation execution** として閉じられる。

---

## 6. G12 の非目標

この段階では、次は狙わない。

- project-wide PDG / whole-program SSA
- 任意個の multi-hop recursion
- field/member/container を全部 first-class にすること
- Go/Java/JS/TS/TSX まで一気に parity を取ること
- UI/reporting 見た目の改修中心の作業

G12 は、bounded continuation planner を捨てるフェーズではない。

**bounded continuation を、single-input return bridge から
multi-input continuation / alias-result stitching を持てる execution contract へ育てるフェーズ**
と定義するのがよい。

---

## 7. このメモで固定したい判断

最後に、G12-1 で固定したい判断を短くまとめる。

1. **current main は、two-hop wrapper-return scope の段階はもう越えている。**
   G11 で Rust/Ruby の代表 success case は通っている。

2. **現在の弱点は “file を足せないこと” より “continuation execution contract が単入力寄りなこと” にある。**

3. **nested multi-input continuation は、scope ではなく input binding の問題として解くべきである。**

4. **alias-result stitching は、return continuation の副産物ではなく独立 bridge family として扱った方がよい。**

5. **tier3 continuation を wrapper-return 専用にしたままでは、alias family の改善は頭打ちになる。**

6. **G12 では path budget だけでなく family-aware budget が必要になる。**

7. **bridge execution provenance を少し強くしないと、multi-input / alias-result stitching の改善理由を説明しにくい。**

要するに G12 でやるべきことは、

**「bounded な file frontier」をさらに大きくすることではなく、
その bounded frontier の上でどの input binding / alias-result stitch / continuation chain を採用したかを
実行 contract として持てるようにすること**

である。

これが揃うと、current main の bounded continuation は
return-centric な short bridge から、
より自然な multi-input / alias-result stitching へ 1 段進める。