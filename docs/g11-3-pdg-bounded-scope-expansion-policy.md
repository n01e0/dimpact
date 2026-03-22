# G11-3: PDG build scope を bounded 前提のまま 1 段拡張する方針

このメモは、current main の `plan_bounded_slice(...)` を捨てずに、
**もう 1 段だけ scope を先へ進める** 方針を固定するもの。

G11-1 では、現在の PDG path がすでに

- root
- direct boundary
- bridge completion
- module companion fallback

を持つ bounded slice planner まで進んでいることを整理した。

G11-2 では、その current main に対してまだ落ちる failure case を 5 件に固定した。

そこから見えてくるのは、現在の主な弱点がもう
「boundary file を 1-hop 入れられるか」ではない、ということだ。

いま本当に足りないのは、
**選んだ bridge completion file の先へ、bridge family を崩さずにもう 1 段だけ進めること**
である。

結論を先に書くと、G11-3 では current planner を次のように拡張する。

- 既存の `Root -> DirectBoundary -> BridgeCompletion` は維持する
- その上で、**admit 済みの bridge completion file からだけ**
  `BridgeContinuationFile` を **1 hop だけ** 追加で選べるようにする
- expansion は fallback からは行わない
- expansion は bridge family を保持したまま行う
- 追加 budget は **per-seed 1 file** を初期値にして、bounded 性を壊さない

つまり G11-3 の一歩は project-wide 化ではなく、
**bounded frontier に対する continuation-aware one-step extension** である。

machine-readable policy: `docs/g11-3-pdg-bounded-scope-expansion-policy.json`

---

## 1. Goal / Non-goal

## Goal

- current main の bounded slice planner を前提に、**もう 1 段だけ continuation closure 用の file** を選べるようにする
- G11-2 のうち、scope が足りないために落ちている case を、propagation 全面改修なしで前進できるようにする
- `slice_selection_summary` に「なぜこの 1 段先 file が入ったか」を残せるようにする
- one-hop completion までは取れている現在地から、**two-hop continuation の入口** まで scope を広げる

## Non-goal

- project-wide PDG / recursive expansion
- bridge family を大量追加すること
- nested multi-input continuation をこの task だけで直すこと
- wrapper alias stitching の propagation 自体をこの task だけで直すこと
- per-seed 全体 budget の最終最適化をここで決め切ること

G11-3 は、scope と propagation を両方いきなり大改造する task ではない。
まずは **scope が届かないせいで bridge が組めない領域** を、bounded のまま 1 段だけ広げる。

---

## 2. current main の planner が止まっている場所

current main の `plan_bounded_slice(...)` は概ね次で止まる。

1. root file を入れる
2. direct caller / callee boundary file を入れる
3. boundary の片側について tier2 candidate を選ぶ
4. per-boundary-side 1 件、per-seed 2 件まで admit する
5. そこで止まる

この形は G4〜G10 の設計としては正しかった。
実際、次はすでに回収できている。

- direct boundary の cross-file summary bridge
- wrapper/leaf っぽい naming と semantic evidence を使った bridge completion 選択
- Ruby require_relative fallback の bounded 取り込み
- slice_selection / pruned_candidates による説明

ただし G11-2 の fixed set で残っている scope-side の穴は、次の型に集約される。

- `main -> wrap -> step -> leaf` のように、tier2 file の先に **もう 1 file 必要** な case
- require_relative split でも同じく **2-hop continuation** が必要な case

ここで重要なのは、現在の tier2 file が「間違っている」のではなく、
**tier2 file だけでは continuation が閉じない** 点である。

したがって G11-3 でやるべきことは、tier2 の ranking を全部やり直すことではなく、
**admit 済み tier2 candidate を anchor にして、その先へ 1 hop だけ進める policy** を足すことになる。

---

## 3. G11-3 の基本方針

## 3.1 current bounded planner を置き換えず、上に 1 tier 足す

G11-3 では `plan_bounded_slice(...)` の基本骨格は変えない。

維持するもの:

- root / direct boundary / bridge completion / fallback の考え方
- direct boundary collect の仕組み
- current tier2 scoring / suppress / prune の流れ
- `slice_selection_summary` を中心にした説明モデル

追加するもの:

- **BridgeContinuationFile** という新しい selection tier

つまり planner 全体を作り直すのではなく、
**admit 済み bridge completion file の先にだけ新 tier を挿す**。

## 3.2 expansion は「bridge completion の先」に限定する

新しい 1 段拡張は、どの selected file からでも辿ってよいわけではない。
対象は次に限定する。

- reason kind が `BridgeCompletionFile` であること
- `bridge_kind` があること
- fallback ではないこと
- その candidate が current planner で **admit 済み** であること

この制約が必要な理由は単純で、
current planner がまだ lexical / callsite heuristics をかなり使うからだ。

もし root や direct boundary から自由に 1 hop 追加を許すと、
「1 段だけ」のつもりがすぐ broad expansion になる。

## 3.3 expansion は bridge family を維持したまま行う

G11-1 の重要な結論は、planner と propagation が同じ bridge taxonomy を持てていないことだった。
G11-3 ではまだ完全統合しないにせよ、scope extension 側では少なくとも
**同じ bridge family の continuation** として file を選ぶ。

つまり BridgeContinuationFile は、「ただの 3 hop 目の file」ではない。

- `WrapperReturn` をさらに閉じる continuation なのか
- `RequireRelativeChain` をさらに閉じる continuation なのか
- `BoundaryAliasContinuation` の延長なのか

を reason metadata に保持する。

## 3.4 expansion は 1 hop で止める

これが G11-3 の bounded 性の中心。

- root から無制限には辿らない
- boundary から無制限には辿らない
- tier2 anchor から **1 hop だけ** continuation file を拾う
- その continuation file から再帰しない

したがって深さは最大でも

- root
- direct boundary
- bridge completion
- bridge continuation

までで止まる。

G11-3 は **two-hop continuation の入口** を取る task であって、
three-hop / recursive continuation へ行く task ではない。

---

## 4. 新しい tier: `BridgeContinuationFile`

## 4.1 定義

`BridgeContinuationFile` は、admit 済みの `BridgeCompletionFile` を anchor にして、
**同じ bridge family を閉じるために追加で必要な 1 file** を選ぶ tier とする。

最低限、次の metadata を持てる設計にする。

- `seed_symbol_id`
- `kind = BridgeContinuationFile`
- `tier = continuation`
- `via_symbol_id`（直前 boundary symbol）
- `via_path`（直前 boundary file）
- `bridge_kind`
- `anchor_symbol_id`（admit 済み bridge completion を正当化した symbol）
- `anchor_path`（admit 済み bridge completion file）
- `scoring`（continuation candidate を選んだ根拠）

ここで特に重要なのは `anchor_symbol_id` / `anchor_path` で、
current main の `Tier2Candidate` にはこれが無い。

G11-3 の 1 段拡張をやるなら、
「どの tier2 candidate を足場にして 1 hop 進んだのか」
を planner が覚えていないといけない。

## 4.2 current schema への影響

概念上は最低でも次の変更が必要になる。

### Reason kind の追加

```rust
enum ImpactSliceReasonKind {
    SeedFile,
    ChangedFile,
    DirectCallerFile,
    DirectCalleeFile,
    BridgeCompletionFile,
    BridgeContinuationFile,
    ModuleCompanionFile,
}
```

### Tier の追加

```rust
enum SliceSelectionTier {
    Root,
    DirectBoundary,
    BridgeCompletion,
    BridgeContinuation,
    ModuleCompanionFallback,
}
```

### Tier2/continuation anchor metadata

`Tier2Candidate` か、それに準ずる internal struct に
`completion_symbol_id` を残す必要がある。

例:

```rust
struct Tier2Candidate {
    path: String,
    via_symbol_id: String,
    via_path: String,
    completion_symbol_id: String,
    bridge_kind: Option<ImpactSliceBridgeKind>,
    scoring: ImpactSliceCandidateScoringSummary,
}
```

public JSON schema を一気に大きく増やしたくないなら、
まず internal anchor metadata と `BridgeContinuationFile` reason だけ先に足す方がよい。

---

## 5. selection policy

以下を G11-3 の正式方針とする。

## 5.1 Step A: current bounded slice をそのまま作る

まず現状どおり、次を実行する。

- root file 選択
- direct boundary file 選択
- tier2 candidate scoring
- suppress / sibling prune / same-path dedup
- per-boundary-side 1 件 admit
- per-seed 2 件 admit

この段階では current main と同じ `BoundedSlicePlan` を作る。

## 5.2 Step B: admit 済み bridge completion からだけ continuation anchor を作る

Step A の結果から、continuation expansion の anchor を抽出する。

anchor になる条件は次。

1. selected reason が `BridgeCompletionFile`
2. `bridge_kind` が `Some(...)`
3. candidate source は fallback ではない
4. selected path が root / direct boundary の再選択ではない
5. candidate が lexical-only ではなく、少なくとも family-aware primary evidence を持つ

G11-3 の初期対象 family は次に絞る。

- `WrapperReturn`
- `RequireRelativeChain`

`BoundaryAliasContinuation` は将来的には continuation 拡張対象にできるが、
G11-2 の current failure を見る限り、まず scope extension で直接効くのは上の 2 つである。

## 5.3 Step C: anchor symbol から同方向へ 1 hop だけ continuation candidate を作る

新 tier の candidate 生成は、anchor symbol を起点に **同じ direction で 1 hop** のみ行う。

- `callees` 方向なら anchor symbol からの outgoing call refs
- `callers` 方向なら anchor symbol への incoming call refs
- `both` はそれぞれ別々に扱い、union は最後に取る

候補 file には次の除外をかける。

- root file と同じ
- direct boundary file と同じ
- 既存 selected tier2 path と同じ
- すでに selected 済みの continuation path と同じ
- module companion fallback からしか辿れないもの

要するに continuation tier は、
**選び直しではなく、selected bridge completion の先の未選択 1 file** を探す層にする。

## 5.4 Step D: family-aware continuation ranking を使う

continuation candidate の ranking は current tier2 ranking のコピーでは足りない。
G11-3 では少なくとも次の順で優先度を置く。

1. **same bridge family continuity** があるもの
2. semantic-ready support があるもの
3. anchor file の終端 callsite に近いもの
4. lexical tiebreak

family ごとの初期ルールは次で十分。

### WrapperReturn

- anchor file 内で selected reason が `WrapperReturn`
- anchor symbol から先の callee file が存在する
- anchor scoring に `ReturnFlow` または `ParamToReturnFlow` がある
- `AssignedResult` だけの lexical-only candidate より優先する

### RequireRelativeChain

- anchor file 内で selected reason が `RequireRelativeChain`
- continuation file が explicit `require_relative` chain 上にある
- fallback-only companion より、明示 require-relative continuation を優先する

G11-3 ではここに新しい bridge family を増やさない。
まずは **selected tier2 family を 1 hop 延長できるか** に集中する。

## 5.5 Step E: continuation budget を独立で設ける

現在の tier2 budget に continuation をそのまま混ぜると、
「1 段拡張したいのに tier2 上限で相殺される」ので意味が薄い。

したがって G11-3 では次の budget を独立で持つ。

- `PER_SEED_TIER3_FILES_MAX = 1`
- `PER_ANCHOR_TIER3_FILES_MAX = 1`

意味は次の通り。

- 1 つの admit 済み tier2 anchor からは 1 file だけ進める
- 1 seed 全体でも追加は 1 file まで

初期値を 1 にする理由は、G11-2 で scope不足としてはっきり効いているのが
まず two-hop continuation 1 本分だから。

これは conservative だが、bounded 性を守るには正しい。

## 5.6 Step F: continuation から再帰しない

BridgeContinuationFile を選んだあと、それを新しい boundary としては扱わない。

- tier4 を作らない
- continuation file から module companion fallback を派生させない
- continuation file からさらに continuation file を作らない

この stop rule があるので、G11-3 の scope expansion は
**current planner に対する 1 段 extension** に留まる。

---

## 6. 何が改善対象で、何がまだ scope 外か

G11-3 の policy が効く case と、効かない case を先に分けておく。

## 6.1 この policy が直接効く case

### `rust-two-hop-wrapper-return-continuation`

現状:

- `wrap.rs` は direct boundary
- `step.rs` は bridge completion
- `leaf.rs` は scope に入らない

G11-3 後:

- `step.rs` を anchor に `leaf.rs` を `BridgeContinuationFile` として選べる

### `ruby-two-hop-require-relative-return-continuation`

現状:

- `lib/wrap.rb` は direct boundary
- `lib/step.rb` は bridge completion
- `lib/leaf.rb` は scope に入らない

G11-3 後:

- `lib/step.rb` を anchor に `lib/leaf.rb` を `BridgeContinuationFile` として選べる

## 6.2 この policy だけでは直らない case

### `rust-nested-two-arg-summary-continuation`

この case は `pair.rs` 自体はすでに scope 内に入りうる。
足りないのは scope ではなく、**multi-input nested continuation を caller result へ戻す propagation / summary mapping** である。

### `rust-cross-file-wrapper-return-alias-chain`

この case も `value.rs` は scope に入りうる。
足りないのは、selected file が無いことではなく、
**imported result -> wrapper alias -> caller result の stitching** である。

### `rust-three-boundary-bridge-budget-overflow`

これは continuation tier を足しても直接は直らない。
問題は per-seed tier2 budget の持ち方であり、
scope 深さより **family / seed 間の budget policy** の問題だから。

つまり G11-3 は G11-2 を全部解く policy ではない。
**scopeで解ける失敗だけを切り出して前進する policy** である。

---

## 7. budgets / pruning / reporting

## 7.1 追加 budget

G11-3 で増える budget は continuation 用だけにする。

```rust
const PER_BOUNDARY_SIDE_TIER2_FILES_MAX: usize = 1;
const PER_SEED_TIER2_FILES_MAX: usize = 2;
const PER_ANCHOR_TIER3_FILES_MAX: usize = 1;
const PER_SEED_TIER3_FILES_MAX: usize = 1;
```

これにより、今までの tier2 selection を不安定にせずに、
最後に 1 枚だけ continuation file を載せられる。

## 7.2 prune reason

public schema を増やしすぎたくなければ既存 prune reason を流用してもよいが、
G11-3 ではできれば continuation 専用の落ち理由を持った方がよい。

候補:

- `continuation_budget_exhausted`
- `not_continuation_ready`
- `weaker_same_anchor_sibling`

ただしここは実装で重く感じるなら、まずは internal logging だけでもよい。
重要なのは **selected されなかった理由が見えること** である。

## 7.3 summary への出し方

`slice_selection_summary` には少なくとも次を出せるようにしたい。

- `kind = bridge_continuation_file`
- `bridge_kind = wrapper_return | require_relative_chain`
- `via_symbol_id` / `via_path`
- `anchor_symbol_id` / `anchor_path`（内部か compact どちらでも可）

これにより、後続 task で
「なぜ leaf.rs / leaf.rb まで scope に入ったのか」
を JSON レベルで固定できる。

---

## 8. 実装 entrypoint の方針

G11-3 の実装は、概念上は次の順がよい。

1. current `plan_bounded_slice(...)` で plan を作る
2. selected tier2 reason から continuation anchor を抽出する
3. continuation candidate を 1 hop だけ集める
4. per-anchor / per-seed budget で admit する
5. `cache_update_paths` / `local_dfg_paths` / `slice_selection_summary` へ merge する

実装 point は `src/bin/dimpact.rs::plan_bounded_slice(...)` の延長で十分で、
別 planner を新設する必要は薄い。

ただし metadata の都合で、少なくとも internal には

- selected bridge completion を正当化した symbol id
- selected bridge completion の source lane / bridge family

を保持する構造が必要になる。

---

## 9. test surface

G11-3 の最初の regression 面は次でよい。

## 9.1 planner-level

- selected path に `BridgeContinuationFile` が 1 件だけ増える
- fallback candidate からは continuation が派生しない
- per-seed continuation budget が 1 を超えない

## 9.2 CLI-level

まずは G11-2 のうち scopeで直接効く 2 ケースを追加 fixture 化する。

- `rust-two-hop-wrapper-return-continuation`
- `ruby-two-hop-require-relative-return-continuation`

固定したい観測は次。

- `summary.slice_selection.files` に `leaf.rs` / `lib/leaf.rb` が増える
- その reason kind が `bridge_continuation_file` になる
- `bridge_kind` は anchor の family を引き継ぐ

この task では edge formation まで全部 fix できなくてもよい。
まずは **scope が届いたこと** を regression にする。

## 9.3 non-regression

既存の direct boundary success case を壊さないこと。

- cross-file summary bridge
- alias continuation の current guard
- Ruby dynamic fallback separation
- same-path duplicate / helper noise suppression

G11-3 の scope extension は conservative なので、ここを壊すようなら設計が広すぎる。

---

## 10. この taskで固定したい判断

最後に、G11-3 で固定したい判断を短くまとめる。

1. **current bounded planner を置き換えない。**
   root / boundary / bridge completion の骨格は維持する。

2. **新しく足すのは `BridgeContinuationFile` という 1 tier だけ。**
   目的は admit 済み bridge completion の先へ 1 hop 進むこと。

3. **expansion は admit 済み bridge completion からだけ行う。**
   root / boundary / fallback から自由に広げない。

4. **expansion は bridge family を保持する。**
   まずは `WrapperReturn` と `RequireRelativeChain` を初期対象にする。

5. **budget は独立に小さく持つ。**
   初期値は per-anchor 1、per-seed 1。

6. **continuation から再帰しない。**
   G11-3 は bounded な 1 段 extension に留める。

7. **この policy は G11-2 の scope不足 case だけを前進させる。**
   nested multi-input や alias stitching は別 task で解く。

要するに G11-3 の本質は、

**「selected bridge completion を足場に、その same-family continuation だけを 1 hop 延長する」**

という一点にある。

これなら current planner の説明力を保ったまま、two-hop continuation へ進める。