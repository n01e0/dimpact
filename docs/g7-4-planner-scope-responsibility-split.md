# G7-4: planner scope（cache / DFG / explanation）の責務分離方針

対象: bounded slice planner の file selection / scope annotation / witness 接続

このメモは、G7-3 までで bridge candidate scoring は入り始めたが、
**planner が選んだ file を cache_update / local_dfg / explanation のどの責務で持っているのかがまだ曖昧** な点を整理し、
G7-5 の最小実装で scope split を入れるための方針を固定するもの。

machine-readable companion: `docs/g7-4-planner-scope-responsibility-split.json`

---

## 1. 背景

G4-3 では既に

- `cache_update_paths`
- `local_dfg_paths`
- `summary.slice_selection.files[*].scopes`

を分ける発想自体は入っていた。

ただし G7-3 時点の runtime を見ると、`src/bin/dimpact.rs` の `SliceSelectionAccumulator::select_path()` は実質的に

1. `cache_update = true`
2. `explanation = true`
3. Rust/Ruby なら `local_dfg = true`

を一度に立てている。

つまり現状は

- `cache_update`
- `explanation`
- `local_dfg`

という 3 つの scope 名はあるが、
**selection responsibility はまだ 1 本の `select_path()` に強く束ねられている。**

この状態でも G7-3 の scoring work までは進められたが、
今後の G7-5 / G7-6 / G7-7 では次の問題が目立ってくる。

---

## 2. いま曖昧な点

## 2.1 explanation scope が cache scope の副作用になっている

現在の planner では file を選ぶと自動で `explanation = true` になるため、

- user-facing に残したい file
- 単に cache を更新したい file
- local DFG だけのために必要な file

が分離されていない。

その結果、
**「選ばれたから説明する」のか、「処理に必要だから一旦入れたが説明対象ではない」のか** が output から読みにくい。

## 2.2 witness 側が explanation scope をまだ厳密に見ていない

`attach_slice_selection_summary()` / `build_witness_slice_context()` は `slice_selection.files` に入っている file を witness path へ接続している。

これは G6/G7-3 までは十分だったが、
scope split 後に

- cache-only file
- local_dfg-only file
- explanation file

が混ざるようになると、
**witness へ載せたい file と planner 内部都合の file を区別する必要** が出る。

## 2.3 pruned candidate と selected file の責務が混ざりやすい

G7-3 で `pruned_candidates[*].scoring` は見えるようになった。
これは良い。

ただし今後 scope split を入れる際、
pruned candidate まで file-level scope に入れてしまうと、

- 選ばれなかった file が explanation file に見える
- cache/local_dfg の実際の build 対象と selected/pruned explanation が混ざる

という別の confusion が起きる。

## 2.4 G7-5 の過剰選択抑制は、scope split なしだと雑にしか入らない

G7-5 の目的は
**bounded slice の過剰選択を 1 件抑える** ことだが、
ここで file を外す対象は本来

- cache から外したいのか
- local DFG から外したいのか
- user-facing explanation から外したいのか

で意味が違う。

scope split を定義しないまま最適化すると、
効果が出ても「何を slim にしたのか」が曖昧なままになる。

---

## 3. この task で固定したいこと

G7-4 では runtime 実装はまだ行わない。
代わりに、G7-5/G7-7 が迷わないよう、planner scope の責務を次で固定する。

1. `cache_update` が何のための scope か
2. `local_dfg` が何のための scope か
3. `explanation` が何のための scope か
4. 3 scope の包含関係 / 非包含関係
5. `files[*]` / `pruned_candidates[*]` / witness context への反映規則
6. G7-5 で必要な最小 implementation split

---

## 4. scope ごとの責務定義

## 4.1 `cache_update`

### 役割

**symbol/reference cache を最新にするための execution scope**。

ここに入る file は、planner や propagation がその run で参照する前提として
「cache 上 stale では困る」もの。

### 何を守るか

- selected root / boundary / bridge file の symbol freshness
- local DFG build 前提の symbol consistency
- path companion / fallback 解決で参照する file の cache freshness

### 何を守らないか

- user-facing explanation の最小性
- witness 文面の見やすさ
- DFG edge の実体化そのもの

つまり `cache_update` は
**正しさ寄りの execution preparation** であって、
それ自体は説明面の契約ではない。

---

## 4.2 `local_dfg`

### 役割

**local file DFG を実際に構築するための materialization scope**。

ここに入る file は、run 中に local DFG を組み、
`local_dfg` / `symbolic_propagation` edge を materialize する対象である。

### 何を守るか

- propagation に必要な file-level DFG coverage
- selected bridge / fallback の local flow recovery
- Rust/Ruby の短距離 alias / return-flow stitching

### 何を守らないか

- cache freshness 全体
- user-facing explanation の最小性
- pruned candidate の観測性

`local_dfg` は
**edge recovery のための execution scope** であり、
cache とは目的が違う。

---

## 4.3 `explanation`

### 役割

**user-facing に「この run で意味のあった file」として残す explanation scope**。

ここに入る file は、少なくとも次のどれかを満たすべきである。

- slice selection の結果として実際に選ばれた root / boundary / bridge / fallback file
- witness path と接続したときに、selected file として意味のある file
- selected-vs-pruned の説明で「勝った側」として残したい file

### 何を守るか

- `summary.slice_selection.files[*]` の human-readable relevance
- witness slice context の見通し
- selected-vs-pruned explanation の土台

### 何を守らないか

- cache freshness
- DFG materialization coverage
- ranking 過程で観測した全候補の exhaustiveness

`explanation` は
**user-facing summary scope** であり、planner 内部の準備対象をそのまま見せる場所ではない。

---

## 5. scope 間の関係

G7 では次の関係を正式方針とする。

## 5.1 `local_dfg => cache_update`

local DFG を build する file は、その前提として cache freshness が必要なので、
`local_dfg` は必ず `cache_update` を含意する。

これは hard invariant。

## 5.2 `explanation => cache_update`

user-facing に selected file として残す file は、
少なくともその run の selected scope として cache 上も整っているべきなので、
G7 の初期方針では `explanation` も `cache_update` を含意する。

これも hard invariant にする。

理由は、初回 scope split 実装を複雑にしすぎないためである。
`explanation-only` file を許す設計も理論上はありえるが、G7 段階では採らない。

## 5.3 `local_dfg` と `explanation` は独立 subset

両者はともに `cache_update` の subset だが、互いを含意しない。

つまり次は許可する。

- `cache_update=true, local_dfg=true, explanation=false`
- `cache_update=true, local_dfg=false, explanation=true`

これが今回の scope split の核心である。

### 例 1: local_dfg-only helper

propagation を成立させるため local DFG は必要だが、
user-facing には勝ち筋として残したくない helper file。

### 例 2: explanation-only-from-DFG-view boundary

selected direct boundary file として説明には残したいが、
言語非対応や軽量 mode のため local DFG は build しない file。

## 5.4 `pruned_candidates` は file scope に入れない

pruned candidate は

- `files[*]` に昇格しない
- `explanation=true` の selected file にもならない
- 必要なら `pruned_candidates[*]` でだけ可視化する

を原則にする。

これで selected scope と candidate inventory を混ぜない。

---

## 6. output surface ごとの規則

## 6.1 `summary.slice_selection.files[*]`

`files[*]` は今後も **planner が何らかの scope で選んだ file の一覧** とする。

ただし G7-5 以降は、各 file の意味を `scopes` で区別する。

### 規則

- `cache_update=true`
  - cache refresh 対象
- `local_dfg=true`
  - local DFG build 対象
- `explanation=true`
  - user-facing selected file

### 重要

`files[*]` に載っていても `explanation=false` はありうる。
この点を G6/G7-3 までと変える。

---

## 6.2 `summary.slice_selection.pruned_candidates[*]`

`pruned_candidates` は引き続き
**selected されなかった candidate inventory** とする。

### 規則

- file-level scope flag は持たせない
- selected file として扱わない
- witness path に混ぜない
- `bridge_kind` / `scoring` / `prune_reason` の観測面として使う

selected-vs-pruned explanation はここを読む。
selected file list に混ぜてはいけない。

---

## 6.3 witness slice context

`build_witness_slice_context()` は G7-5/G7-7 で次の方針に寄せる。

### 規則

- witness path へ接続する selected file は `scopes.explanation == true` のものだけにする
- `cache_update=true, explanation=false` file は witness file context に出さない
- `local_dfg=true, explanation=false` file も witness file context に出さない

これで witness は planner 内部の build assist file に引きずられず、
**user-facing に意味のある selected file だけ** を保てる。

---

## 7. tier ごとの推奨責務

G7 の bounded slice tier を scope 責務に落とすと、初期方針は次になる。

| tier | 典型 reason | cache_update | local_dfg | explanation |
|---|---|---:|---:|---:|
| Tier 0 | `seed_file` / `changed_file` | yes | supported language なら yes | yes |
| Tier 1 | `direct_caller_file` / `direct_callee_file` | yes | supported language かつ propagation 必要時 yes | yes |
| Tier 2 selected | `bridge_completion_file` | yes | supported language なら通常 yes | yes |
| Tier 2 pruned | `pruned_candidates[*]` | no file scope | no file scope | no file scope |
| Tier 3 selected | `module_companion_file` | yes | supported language かつ actually used なら yes | yes or no |
| Tier 3 pruned | `pruned_candidates[*]` | no file scope | no file scope | no file scope |

ここで `Tier 3 selected` の `explanation` を `yes or no` としているのがポイントである。

narrow fallback は

- 実際に selected explanation として残したい fallback
- DFG/cache assist としては使うが、user-facing には前面に出したくない fallback

の両方がありうるため、`local_dfg` と `explanation` を固定連動させない。

---

## 8. planner API / runtime への落とし方

G7-5 で必要な最小変更は、責務ごとに mark 関数を分けること。

## 8.1 やってはいけないこと

現在の `select_path()` のように

- cache_update
- explanation
- local_dfg

を 1 回で全部立てる helper を責務の中心に据え続けること。

これは split 実装のあとも convenience helper として残してよいが、
**policy の本体にしてはいけない。**

## 8.2 最小 shape 提案

```rust
impl SliceSelectionAccumulator {
    fn mark_cache_update(&mut self, path: &str);
    fn mark_local_dfg(&mut self, path: &str);
    fn mark_explanation(&mut self, path: &str);
    fn add_reason(&mut self, path: &str, reason: ImpactSliceReasonMetadata);
    fn add_pruned_candidate(&mut self, candidate: ImpactSlicePrunedCandidate);
}
```

### promotion rule

- `mark_local_dfg(path)` は内部で `mark_cache_update(path)` を呼んでよい
- `mark_explanation(path)` も内部で `mark_cache_update(path)` を呼んでよい
- ただし `mark_cache_update(path)` は `mark_explanation(path)` を呼んではいけない
- `mark_local_dfg(path)` も `mark_explanation(path)` を自動で呼んではいけない

これで包含関係をコードに素直に反映できる。

## 8.3 reason attachment rule

`reasons[*]` は file-level selected reason の説明面なので、
原則として **`explanation=true` file に対して意味のある reason** を載せる。

一方で cache/local_dfg assist file が `files[*]` に残る場合は、
次のどちらかでよい。

1. same reason schema を使い続ける
2. 後で scope-specific annotation を足す

G7 では 1 で十分。
ただし witness では explanation scope だけを見る。

---

## 9. G7-5 / G7-7 への接続

## 9.1 G7-5

scope split の最小実装では少なくとも次をやるべき。

1. `select_path()` の責務を分離する
2. `files[*]` に `explanation=false` file が残りうるようにする
3. 過剰選択抑制は、まず `explanation` または `local_dfg` のどちらを slim にしたか明示する
4. `pruned_candidates` は selected file list に混ぜない

## 9.2 G7-7

witness explanation 強化では次を守るべき。

1. `selected_files_on_path` は `scopes.explanation=true` だけから組み立てる
2. selected-vs-pruned 比較は `pruned_candidates` と `files[*].reasons[*].scoring` の差分で語る
3. local_dfg-only helper を witness の main narration に出さない

---

## 10. 固定しておきたい判断

### 判断 1

**`cache_update` は execution preparation、`local_dfg` は edge materialization、`explanation` は user-facing summary である。**
3 scope は目的が違う。

### 判断 2

**`local_dfg` と `explanation` はともに `cache_update` の subset とする。**
ただし互いは独立でよい。

### 判断 3

**pruned candidate は file scope に昇格しない。**
観測面は `pruned_candidates[*]` に閉じる。

### 判断 4

**witness は explanation scope だけを見る。**
cache/local_dfg assist file をそのまま user-facing path に混ぜない。

### 判断 5

**G7-5 の最初の実装では `select_path()` 一発選択から、scope-specific mark API へ寄せる。**
これが最小で一番効く split である。

---

## 11. 一言まとめ

G7-4 の planner scope split 方針は、**`cache_update` を execution preparation、`local_dfg` を propagation edge materialization、`explanation` を user-facing selected file summary として明確に分離し、`local_dfg` と `explanation` をともに `cache_update` の subset だが互いには独立な scope とみなす** というものである。これにより、G7-5 では過剰選択を「どの責務から外したか」を明示しながら抑えられ、G7-7 では witness を explanation scope だけに絞って見通しを保てる。