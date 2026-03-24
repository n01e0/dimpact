# G12-3: continuation chain を接続するための bounded stitching policy

このメモは、G12-1 / G12-2 で整理した
**multi-input continuation / alias-result stitching の current weakness** に対して、
current bounded planner を壊さずに continuation chain をどう接続するかを定義するもの。

machine-readable policy: `docs/g12-3-bounded-continuation-stitching-policy.json`

G11-3 では、scope 側に `BridgeContinuationFile` を足して
**selected bridge completion の先へ same-family で 1 hop 進む** policy を固定した。

ただし G12 の主題は、もう file scope だけではない。
今の main で弱いのは、主に次である。

- nested multi-input continuation の input binding
- reordered / partial binding を含む continuation mapping
- wrapper / caller をまたぐ alias-result stitching
- alias family を execution 側で first-class に扱えないこと
- bridge execution provenance が chain として残らないこと

結論を先に書くと、G12-3 では continuation chain を次のように扱う。

- file scope は **G11 までの bounded slice / bridge continuation** を前提にする
- その selected scope の上で、**stitch-ready anchor** から **bounded stitching chain** を組む
- chain は step family 単位で構成し、少なくとも
  - `callsite_input_binding`
  - `summary_return_bridge`
  - `nested_summary_bridge`
  - `alias_result_stitch`
  - `require_relative_load`
  を vocabulary として持つ
- multi-input continuation は input count ではなく **binding map** として扱う
- alias-result stitching は return continuation の副産物ではなく、**独立 bridge family** として扱う
- chain selection / reporting では **path budget ではなく family-aware budget** を使う
- chain は bounded に止め、project-wide recursive search にはしない

つまり G12-3 の本質は、

**「bounded に選ばれた file frontier の上で、どの input binding / summary bridge / alias-result stitch を採用して
continuation chain を閉じたのか」を execution policy として固定すること**

にある。

---

## 1. Goal / Non-goal

## Goal

- current bounded slice planner の上で、continuation chain の接続単位を定義する
- nested multi-input / reordered binding / alias-result stitching を同じ policy 語彙で扱えるようにする
- planner 側の bridge family と propagation / witness 側の execution family を寄せる
- later G12 code tasks が「何を繋げたら成功なのか」を docs / tests / output で固定しやすくする
- bridge execution provenance を残すための最小 vocabulary を定義する

## Non-goal

- project-wide PDG / whole-program SSA
- 無制限の recursive continuation exploration
- field / member / container / collection 全般をこの task だけで first-class にすること
- 全言語 parity を一気に取ること
- 現在の planner を全面的に置き換えること

G12-3 は、scope planner を捨てて別の engine を作る task ではない。
**既存の bounded frontier の上で stitching execution を bounded に formalize する task** である。

---

## 2. current main で policy が必要な理由

current main には、すでに次がある。

- `plan_bounded_slice(...)`
- `BridgeCompletionFile` / `BridgeContinuationFile`
- `PdgBuilder::augment_symbolic_propagation(...)`
- `slice_selection_summary` / witness slice context

そのため scope 側の two-hop wrapper-return control case はもう通っている。

一方で current main の propagation は、まだかなり次に依存している。

- callsite line 周辺から集めた use/def node
- `summary.inputs` と `callsite_uses` の zip-by-order
- nested summary では `inputs.len() == 1` 前提
- `impacted node -> callsite_defs` を中心にした generic bridge

この contract だと、G12-2 で固定した次の failure をきれいに説明しにくい。

1. nested multi-input continuation
2. reordered / partial input binding continuation
3. wrapper + caller double alias-result stitching
4. alias family continuation beyond tier2
5. Ruby require_relative + alias-result mixed stitching

したがって G12-3 では、file 選択ではなく
**continuation execution step を bounded policy として定義する必要**がある。

---

## 3. 用語

## 3.1 stitch-ready anchor

`stitch-ready anchor` は、bounded planner がすでに選んだ file / symbol 群のうち、
continuation chain を始めてよい起点を指す。

初期対象は次に限定する。

- selected direct boundary symbol
- selected bridge completion symbol
- selected bridge continuation symbol

ただし chain を組み始めるときの primary anchor は、
**selected bridge completion / bridge continuation 側**を優先する。
理由は、G12 の failure surface が direct boundary の有無ではなく、
その先の continuation closure にあるからである。

## 3.2 bounded stitching chain

`bounded stitching chain` は、selected bounded slice の上で
seed-side input と caller/callee result を結ぶために採用された execution step 列を指す。

chain は edge の生列ではなく、**step family 列**として扱う。

例:

```text
callsite_input_binding
  -> nested_summary_bridge
  -> alias_result_stitch
  -> summary_return_bridge
```

## 3.3 step family

G12-3 で扱う step family は次に固定する。

- `callsite_input_binding`
- `summary_return_bridge`
- `nested_summary_bridge`
- `alias_result_stitch`
- `require_relative_load`

この vocabulary は、planner の `bridge_kind` を置き換えるものではない。
むしろ

- planner = **どの family を狙って scope を残したか**
- stitching policy = **その scope の上でどの family の execution step を実行したか**

をつなぐための vocabulary である。

## 3.4 binding map

`binding map` は、caller 側 input と callee summary input の対応づけを表す。

最低限、次の情報を持てる前提にする。

- `caller_node_id`
- `callee_input_node_id`
- `binding_kind`

`binding_kind` は少なくとも次を持つ。

- `positional`
- `reordered_positional`
- `repeated`
- `partial`
- `literal_or_unknown_companion`

G12 の multi-input continuation は、
**summary.inputs の本数ではなく binding map の有無**で扱う。

## 3.5 chain family

step family と別に、chain 全体には coarse-grained な family を持たせる。
初期対象は次。

- `return_continuation`
- `alias_result_stitch`
- `require_relative_continuation`
- `mixed_require_relative_alias_stitch`
- `nested_multi_input_continuation`

この coarse family は、reporting / budget / witness compact 表示に使う。

---

## 4. 基本方針

## 4.1 scope と stitching を分ける

G12-3 で一番大事なのはこれ。

- **scope planner の責務**
  - どの file を bounded に選ぶか
  - どの bridge family の representative を残すか
- **stitching policy の責務**
  - selected file 群の上で、どの input binding / alias stitch / summary bridge を採用するか

この分離を守ると、G12-4 以降で

- scope が足りない failure
- stitching execution が足りない failure
- provenance/reporting が足りない failure

を混ぜずに扱える。

## 4.2 stitching は selected scope の外へ出ない

G12-3 の stitching policy は、selected bounded slice の外へ勝手に file を広げない。

許されるのは次だけ。

- planner がすでに selected に残した file / symbol を使う
- G11-3 の `BridgeContinuationFile` までで scope が止まっていることを前提にする

つまり G12-3 は **execution policy** であり、scope widening policy ではない。

## 4.3 stitching は family-aware に接続する

generic data bridge を積むだけではなく、
少なくとも内部 contract 上は次を区別する。

- return continuation の chain
- alias-result stitching の chain
- require_relative continuity を含む chain
- multi-input binding を含む chain

これにより、later tasks では

- どの family が recall 改善に効いたか
- どの family が budget で落ちたか
- witness に何を出すべきか

を比較しやすくなる。

---

## 5. chain 構築 policy

G12-3 の chain 構築は、概念上は次の 5 step で行う。

## 5.1 Step A: selected scope から stitch-ready anchor を抽出する

まず current bounded slice から anchor 候補を作る。

優先順位は次でよい。

1. `BridgeContinuationFile`
2. `BridgeCompletionFile`
3. direct boundary symbol

理由は、selected continuation / completion がある時点で、
planner がそこを continuation family の representative とみなしているからである。

同一 seed / same path / same family に複数 anchor がある場合は、
**selected_vs_pruned reasoning で勝った representative** を anchor にする。

## 5.2 Step B: chain に必要な input binding map を作る

anchor を起点に continuation を進める前に、
callsite input と summary input の対応づけを作る。

ルールは次。

1. single-input の時も binding map を省略しない
2. multi-input では zip-by-order を canonical contract にしない
3. reordered / repeated / partial binding を distinct kind として残す
4. literal slot や unknown slot は `literal_or_unknown_companion` として残す
5. later bridge step は binding map を参照して relevant input subset を選ぶ

この policy により、G12 の multi-input continuation は
**binding map が成功したかどうか**で評価できるようになる。

## 5.3 Step C: binding map を基に summary/nested bridge を作る

次に、binding map を参照して summary bridge を作る。

### top-level summary return

- selected binding map がある caller input だけを summary input へ接続する
- bound されなかった input は bridge 対象にしない
- `summary_return_bridge` は impacted output を caller def/result 側へ戻す

### nested summary bridge

- nested callee でも single-input 前提に戻さない
- selected binding map の relevant subset だけを nested summary へ渡す
- nested summary が multi-input の場合も、binding map を更新しながら chain を継続する

要するに G12-3 では、
**nested continuation を 1 本の chain として扱い、途中で単入力 fallback に潰さない**
ことを正式方針にする。

## 5.4 Step D: alias-result stitching を独立 step として挿す

result が temp / alias / reassignment / caller-local alias に入る箇所では、
`alias_result_stitch` を distinct step として挿す。

ルールは次。

1. imported result を wrapper-local alias zone に入れる step
2. wrapper-local alias zone を caller-local result/alias zone へ継続する step
3. 必要なら return continuation と alias-result stitch の両方を同一 chain 内に持つ
4. alias-result stitch の成功/不成功を summary return 成功と別に観測できるようにする

この区別があると、例えば
`tmp -> alias -> y -> out` が閉じたのか、
それとも偶然 generic bridge で見えているだけなのかを分けて扱える。

## 5.5 Step E: require_relative continuity は load provenance として chain に参加させる

Ruby では `require_relative_load` を step family として扱う。

これは data flow step そのものではないが、
次の理由で chain に参加させる意味がある。

- selected file が require_relative split の結果であることを説明できる
- `require_relative_chain` と alias-result stitch の mixed case を 1 本の chain として表現できる
- Ruby の continuation が lexical fallback だけに見えるのを避けられる

そのため mixed Ruby case では、少なくとも internal/provenance 上は

```text
require_relative_load
  -> summary_return_bridge
  -> alias_result_stitch
```

のように chain を表現できるようにする。

---

## 6. boundedness / budget policy

G12-3 では、chain execution も bounded に止める。

## 6.1 selected scope の外へ出ない

再掲だが、chain 構築は selected bounded slice の中だけで行う。

- pruned candidate を勝手に再評価しない
- non-selected file を runtime 的に reopen しない
- scope widening が必要なら planner task へ戻す

## 6.2 per-anchor chain representative は 1 本

1 つの stitch-ready anchor からは、同じ coarse chain family の representative を 1 本だけ残す。

例:

- nested multi-input continuation: 1 representative
- alias-result stitch: 1 representative
- mixed require_relative + alias-result: 1 representative

理由は、G12 の段階では chain enumeration よりも
**勝った representative を説明できること**が重要だから。

## 6.3 per-seed family budget を使う

current planner の path budget だけでは、return family が alias family を食いやすい。
そのため G12-3 では、少なくとも execution/reporting 上は次の family budget を持つ。

- `return_continuation`: 1
- `alias_result_stitch`: 1
- `require_relative_continuation`: 1
- `nested_multi_input_continuation`: 1
- `mixed_require_relative_alias_stitch`: 1

これは scope planner の final budget ではなく、
**stitching chain を代表として残す reporting/selection budget** と考えるのがよい。

## 6.4 chain depth を bounded にする

scope depth は G11-3 で bounded に止めているので、
execution chain も無制限にはしない。

初期方針としては次で十分。

- one selected binding map
- one nested summary expansion layer
- one alias-result stitching segment per locality
- one coarse family transition

言い換えると、G12-3 では
**arbitrary recursive chain search** ではなく
**selected representative chain の bounded reconstruction** を狙う。

---

## 7. reporting / provenance policy

G12-3 では、後続 task のために bridge execution provenance の最小 contract も決めておく。

## 7.1 path provenance と chain provenance を分ける

現在の witness は主に edge provenance を持つ。

- `call_graph`
- `local_dfg`
- `symbolic_propagation`

これは維持する。
ただし G12 ではそれに加えて、**chain provenance** を持つ。

chain provenance は少なくとも次を表せればよい。

- selected coarse chain family
- selected step family 列
- selected anchor
- selected binding map の relevant 部分

## 7.2 compact chain provenance 例

public schema は後続 task で調整してよいが、
concept としては次を出せるようにする。

```json
{
  "bridge_execution_chain_compact": [
    {
      "family": "nested_multi_input_continuation",
      "anchor_symbol_id": "rust:wrap.rs:fn:wrap:1",
      "step_family": "callsite_input_binding",
      "binding": ["main.rs:use:y -> pair.rs:def:b"]
    },
    {
      "family": "nested_multi_input_continuation",
      "anchor_symbol_id": "rust:pair.rs:fn:pair:1",
      "step_family": "nested_summary_bridge"
    },
    {
      "family": "alias_result_stitch",
      "anchor_symbol_id": "rust:wrap.rs:fn:wrap:1",
      "step_family": "alias_result_stitch",
      "stitch": ["wrap.rs:def:tmp -> wrap.rs:def:alias -> main.rs:def:out"]
    }
  ]
}
```

## 7.3 selected vs pruned reasoning に family を通す

現在の `selected_vs_pruned_reasons` は slice selection 側に強い。
G12-3 では、少なくとも内部的には次を区別できるようにしたい。

- selected return chain が勝った理由
- selected alias-result chain が勝った理由
- mixed chain が pure return chain より勝った理由
- nested multi-input chain が single-input fallback より勝った理由

これにより later tasks で
**なぜこの stitching chain が採用されたのか** を file selection 以外の語彙で説明できる。

---

## 8. G12-2 failure set に対して何が直接効くか

## 8.1 直接効く case

### `rust-nested-two-arg-summary-continuation`

効く点:

- binding map を first-class にする
- relevant arg subset だけを nested summary へ通す
- caller result bridge を single-input fallback に頼らず組む

### `rust-reordered-partial-input-binding-continuation`

効く点:

- reordered / partial binding kind を distinct に持つ
- literal companion を dropped/companion slot として扱う

### `rust-wrapper-caller-double-alias-result-stitching`

効く点:

- alias-result stitching を distinct step にする
- wrapper alias と caller alias を 1 本の chain で表現する

### `ruby-require-relative-alias-result-mixed-stitching`

効く点:

- require_relative provenance と alias-result stitching の mixed chain を定義できる

## 8.2 planner 側の追従が必要な case

### `rust-alias-family-continuation-beyond-tier2`

この case は stitching policy だけでは完結しない。
理由は、alias family の continuation representative を selected scope に残す必要があるからである。

したがってこの case には、G12-3 単体ではなく
**family-aware planner continuation** との整合が必要になる。

それでも G12-3 でこの case を policy に含める意味はある。
selected scope に alias family representative が存在したときに、
それをどう chain 化するかを先に固定できるからである。

---

## 9. test surface

G12-3 の最初の regression surface は次でよい。

## 9.1 policy / docs level

- step family vocabulary が固定されていること
- binding kind vocabulary が固定されていること
- family-aware budget の考え方が固定されていること
- selected scope の外へ出ない boundedness rule が明記されていること

## 9.2 later code/test level で固定したい観測

### multi-input continuation

- selected binding map がある
- relevant input だけが caller result へ戻る
- irrelevant input は bridge に乗らない

### alias-result stitching

- imported result -> alias -> caller result が distinct chain として説明される
- return continuation と alias-result stitching が分かれている

### mixed Ruby case

- `require_relative_load` と `alias_result_stitch` が両方 compact provenance に出る

### non-regression

- existing two-hop wrapper-return success case を壊さない
- existing imported-result basic control case を壊さない
- same-path suppression / helper-noise guard を壊さない

---

## 10. この task で固定したい判断

最後に、G12-3 で固定したい判断を短くまとめる。

1. **scope と stitching を分ける。**
   G12-3 は selected bounded slice の上で execution chain を定義する task である。

2. **multi-input continuation は input count ではなく binding map として扱う。**

3. **alias-result stitching は独立 bridge family / step family として扱う。**

4. **continuation chain は step family 列で表現する。**
   最低限 `callsite_input_binding` / `summary_return_bridge` / `nested_summary_bridge` / `alias_result_stitch` / `require_relative_load` を持つ。

5. **stitching は selected scope の外へ出ない。**
   scope widening は planner 側の責務に残す。

6. **representative の selection には family-aware budget を使う。**
   return family が alias family を常に食う形にしない。

7. **bridge execution provenance を compact chain として残す。**
   edge provenance だけでは G12 の改善理由を説明しきれない。

要するに G12-3 の本質は、

**「どの file を入れたか」ではなく、
その bounded scope の上でどの input binding / nested summary bridge / alias-result stitch を採用して
continuation chain を閉じたのかを、bounded policy として固定すること」**

にある。

これが揃うと、G12-4 以降では
multi-input continuation と alias-result stitching の改善を
“たまたま edge が増えた” ではなく **policy 通りに chain が閉じた** として扱える。