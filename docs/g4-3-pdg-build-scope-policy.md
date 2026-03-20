# G4-3: PDG build scope を関連 file を含む最小集合へ広げる方針

このメモは、現在の `impact --with-pdg` / `--with-propagation` が
**changed/seed file 単体**に強く寄っている build scope を、
**関連 file を含む最小集合**へ広げるための方針を固定するもの。

G4-1 では「scope planner が不在なこと」が本質課題だと整理し、
G4-2 ではその不足が出やすい multi-file evaluation set を 5 ケースに固定した。

G4-3 で決めるべきことは、実装より先に次の 3 点である。

1. **どの file を scope に入れるか**
2. **どの file を cache update 対象にし、どの file だけ local DFG build 対象にするか**
3. **なぜその file が選ばれたかを、後続テストで追える形にするか**

結論を先に書くと、G4 では

- project-wide DFG build には行かない
- base graph（symbol/ref）は engine 側が決める
- PDG 側はその base graph の**1-hop 境界 + bridge completion 用の最小追加**だけを選ぶ

という方針を採るのが最も筋がよい。

machine-readable policy: `docs/g4-3-pdg-build-scope-policy.json`

---

## 1. Goal / Non-goal

## Goal

- seed/changed file 単体では弱い multi-file PDG ケースに対して、
  **最小限の related files** を deterministic に選べるようにする
- `build_pdg_context()` に渡す即席の path list ではなく、
  **意味を持つ plan object** を導入できる設計にする
- G4-2 の fixed set に対して、
  「なぜその file を含めたか」を説明できる build policy を固定する

## Non-goal

- 全 project file の DFG を常時 build すること
- Go / Java / Python / JS / TS / TSX の local DFG parity を先に取ること
- engine policy の完全統合（これは G4-8 以降の論点）
- witness schema の最終形をここで決め切ること

---

## 2. 現状から何を変えるか

現状の PDG path は、おおむね次の入力をそのまま使っている。

- diff mode: `changed.changed_files`
- seed mode: `seeds` が属する file 集合

つまり build scope は「変更が入った file」または「seed が置かれた file」の列挙で止まっている。

G4-3 ではこれを、最低でも次の 3 層に分ける。

1. **base graph scope**
   - engine / cache から読み出す symbol/ref universe
2. **cache update scope**
   - その実行で cache を更新する file 集合
3. **local DFG scope**
   - 実際に Rust/Ruby local DFG を build する file 集合

ここで決めたいのは主に 2 と 3 の policy であり、
1 は engine 側の責務として扱う。

---

## 3. 基本方針

## 3.1 Graph-first, file-second

scope 選択は raw import / raw filesystem heuristic から始めない。
まず

- seed symbols
- changed symbols
- base refs
- symbol index

から **symbol 境界** を決め、そのあと file へ落とす。

理由:

- multi-file で欲しいのは「関係ありそうな file 全部」ではなく、**今の boundary を越えるために必要な file** だから
- import 文だけで広げると、unused import や broad namespace import で scope が膨らみやすい
- G4-2 の case はすべて「symbol boundary を 1〜2 段閉じる」と説明した方が自然

## 3.2 1-hop boundary を基本、2-hop は bridge completion のみ

最初の scope expansion は **1-hop boundary** に固定する。

- callers 方向なら seed symbols の direct callers
- callees 方向なら seed symbols の direct callees
- both なら両方の union

そのうえで、必要な場合だけ **bridge completion** として 1 回だけ追加する。

これにより、結果として最大でも

- seed/changed files
- その 1-hop 近傍 files
- bridge completion で足す 1 段

までに抑える。

## 3.3 local DFG scope は「全部入れる」のではなく「効く file だけ」に寄せる

base graph scope は全言語でもよいが、local DFG scope は当面次に限定する。

- `.rs`
- `.rb`

つまり planner は language-agnostic に file を選んでよいが、
**DFG enrichment を実際に受ける file は Rust/Ruby 中心** にする。

この分離は重要で、

- cache update scope は広くてもよい
- local DFG scope は狭くてよい

を同時に成立させる。

## 3.4 scope は理由付きで決める

planner の出力には path list だけでなく、
各 file に対して最低 1 つの **selection reason** を持たせる。

最低限の reason 種別は次で足りる。

- `seed_file`
- `changed_file`
- `direct_caller_file`
- `direct_callee_file`
- `bridge_completion_file`
- `module_companion_file`

これがあると、G4-2/G4-7 の fixture で
「なぜこの file まで DFG build 対象なのか」をテスト/ログで追える。

---

## 4. Planner contract

G4-3 で導入したい planner は、概念上次の contract を持つ。

## 4.1 入力

- `seed_symbols: &[Symbol]`
- `changed_files: &[String]`
- `index: &SymbolIndex`
- `refs: &[Reference]`
- `direction: ImpactDirection`
- `with_propagation: bool`

必要なら将来的に:

- `engine_kind`
- `language_mode`
- `file_budget`
- `bridge_budget`

を足せる形にする。

## 4.2 出力

少なくとも次を返す plan object が必要。

```rust
struct PdgBuildScopePlan {
    cache_update_paths: Vec<String>,
    local_dfg_paths: Vec<String>,
    reasons_by_path: BTreeMap<String, Vec<PdgScopeReason>>,
}
```

`PdgScopeReason` は enum で十分。

```rust
enum PdgScopeReason {
    SeedFile,
    ChangedFile,
    DirectCallerFile { via_symbol_id: String },
    DirectCalleeFile { via_symbol_id: String },
    BridgeCompletionFile { via_symbol_id: String },
    ModuleCompanionFile { via_path: String },
}
```

ここで重要なのは、**cache update** と **local DFG build** を別ベクトルで持つこと。

---

## 5. 採用する selection policy

以下を G4 の正式方針とする。

## 5.1 Tier 0: seed / changed files は常に含める

### Rule

次は必ず plan に含める。

- diff mode なら `changed_files`
- seed mode なら `seed_symbols` の所属 file

### Why

これは現状の挙動と連続であり、最小 scope の核でもある。
ここを外す理由はない。

### Plan への反映

- `cache_update_paths`: 含める
- `local_dfg_paths`: Rust/Ruby 拡張子なら含める
- reason: `seed_file` または `changed_file`

## 5.2 Tier 1: direction-aware direct boundary files を追加する

### Rule

seed symbols を起点に base refs を 1-hop だけ辿り、
隣接 symbol の所属 file を選ぶ。

- `callers`: `to == seed` となる edge の `from` symbol file
- `callees`: `from == seed` となる edge の `to` symbol file
- `both`: 両方

### Why

G4-2 の multi-file weakness はほぼ全部ここに載る。

- cross-file callsite summary bridge
- cross-file wrapper return-flow
- cross-file imported result stitching

はいずれも seed file 単体ではなく、**直近の boundary file** が必要だから。

### Plan への反映

- `cache_update_paths`: 追加する
- `local_dfg_paths`: Rust/Ruby 拡張子なら追加する
- reason:
  - callers なら `direct_caller_file`
  - callees なら `direct_callee_file`

## 5.3 Tier 2: bridge completion として 1 回だけ追加する

### Rule

Tier 1 で入れた boundary symbols / files について、
**bridge を閉じるために必要な 1 追加 file** だけ許可する。

bridge completion を入れる条件は次のどちらか。

1. **wrapper-return completion**
   - Tier 1 で選ばれた function/method file 内に temp alias / assigned-result がありそうで、
     その function/method がさらに別 file symbol を直接呼んでいる
2. **boundary-side alias completion**
   - Tier 1 file 側で imported result が local alias chain に入るケースを観測したく、
     その callee/ caller の直近 file がもう 1 つ必要

言い換えると、Tier 2 は project-wide expansion ではなく、
**1-hop boundary の片側にある summary/return/alias bridge を閉じるための補助** に限定する。

### Why

G4-2 の `rust-cross-file-wrapper-return-flow` は、
seed file + direct boundary file だけでは足りず、
`main -> adapter -> core` の 3 file 目が必要になる。

ただしこれを一般化しすぎると scope explosion になるので、
**Tier 2 は 1 回だけ** に固定する。

### Plan への反映

- `cache_update_paths`: 追加する
- `local_dfg_paths`: Rust/Ruby 拡張子なら追加する
- reason: `bridge_completion_file`

## 5.4 Tier 3: module companion は language-specific fallback としてだけ使う

### Rule

symbol graph だけでは file 同定が痩せるケースに限って、
module companion fallback を許す。

対象は当面:

- Rust: `foo.rs` / `foo/mod.rs` / `mod.rs` の companion
- Ruby: `require_relative` で直近に解決できる companion

ただし fallback は **Tier 1 / Tier 2 で選ばれた path に紐づく場合だけ** 許可する。
import 文を起点に無制限に広げない。

### Why

scope policy は graph-first が原則だが、
Rust module split と Ruby require-relative は path companion を見た方が安定する場面がある。

ただしこれを主ルールにすると scope が膨らむので、
**graph selection の補助**に限定する。

### Plan への反映

- `cache_update_paths`: 必要時のみ追加
- `local_dfg_paths`: Rust/Ruby 拡張子なら追加
- reason: `module_companion_file`

---

## 6. Budget / stop conditions

最小集合を保つため、planner には budget と stop 条件が必要。

## 6.1 file budget

G4 の初期値は次で固定するのがよい。

- `cache_update_paths`: 12 file まで
- `local_dfg_paths`: 8 file まで
- `bridge_completion_file`: per seed 1 file まで

数値は暫定だが、考え方は重要。

- cache update は少し広くてよい
- local DFG build はより厳しく絞る

## 6.2 deterministic ordering

scope の安定性のため、path の選択順は deterministic にする。

優先順は次。

1. Tier 0
2. Tier 1
3. Tier 2
4. Tier 3

同 tier 内では:

- seed からの hop depth
- selected reason の優先度
- path の lexical order

で決める。

## 6.3 no recursive closure

Tier 2 の先をさらに再帰的に広げない。

理由:

- G4-3 の目的は project-wide planner ではない
- まずは「1-hop boundary + 1 bridge completion」で G4-2 set の改善可否を見たい

---

## 7. diff / seed / per-seed ごとの適用方針

## 7.1 diff mode

### Initial seeds

- `compute_changed_symbols()` または将来の engine-provided changed symbols

### Tier 0

- `changed_files`

### Tier 1

- changed symbols から direction-aware に 1-hop files を選ぶ

### Tier 2

- G4-2 の wrapper / alias / split Ruby chain に対応する bridge completion を 1 回だけ許す

## 7.2 explicit seed mode

### Initial seeds

- CLI seed symbols

### Tier 0

- seed file

### Tier 1

- seed symbols に隣接する direct caller / callee files

### Tier 2

- seed symbol が wrapper or imported-result boundary にいる場合だけ bridge completion

## 7.3 per-seed mode

`--per-seed` では seed ごとに独立した plan を作る。

理由:

- grouped output なのに scope だけ共有すると、
  「どの seed のためにこの file を入れたのか」が不明瞭になる
- witness と同様、scope も seed 単位で閉じた方が説明しやすい

必要なら最終的に union plan を使って実行してもよいが、
少なくとも内部的には **per-seed scope reason** を保てる形にするべき。

---

## 8. どの file を cache update し、どの file だけ DFG build するか

この分離は G4-3 の重要点なので明示しておく。

## 8.1 cache update scope

cache update は、base graph の鮮度を落とさないために少し広めでよい。

含める対象:

- Tier 0
- Tier 1
- Tier 2
- 必要時の Tier 3

ただし budget 超過時は

- Tier 3 を先に落とす
- 次に Tier 2 を落とす
- Tier 1/Tier 0 は最後まで残す

## 8.2 local DFG scope

local DFG build はより保守的にする。

含める対象:

- Tier 0 の Rust/Ruby files
- Tier 1 の Rust/Ruby files
- Tier 2 の Rust/Ruby files
- Tier 3 は原則 budget に余裕があるときだけ

つまり policy としては

**cache update scope ⊇ local DFG scope**

であるべき。

---

## 9. G4-2 fixed set への対応づけ

この policy が G4-2 の 5 ケースにどう効くかを短く固定する。

## 9.1 rust-cross-file-callsite-summary-bridge

- Tier 0: `main.rs`
- Tier 1: `callee.rs`
- Tier 2: 不要

これで「changed caller file + direct callee file」が scope に入る。

## 9.2 rust-cross-file-wrapper-return-flow

- Tier 0: `main.rs`
- Tier 1: `adapter.rs`
- Tier 2: `core.rs`

これで wrapper-return completion を 1 回だけ許す。

## 9.3 rust-cross-file-imported-result-alias-chain

- Tier 0: `main.rs`
- Tier 1: `value.rs`
- Tier 2: 原則不要

caller 側 alias chain は Tier 0 内で見え、imported result 側の source file を Tier 1 で足す。

## 9.4 ruby-cross-file-callees-chain-alias-return

- Tier 0: changed file
- Tier 1: `mid.rb` or `main.rb` の direct boundary
- Tier 2: `leaf.rb`

split fixture の temp alias / return-flow を閉じるのに対応。

## 9.5 ruby-cross-file-dynamic-send-target-separation

- Tier 0: dispatch file
- Tier 1: target definition file
- Tier 2: 原則不要

これで scope 拡張後の FP guard を測れる。

---

## 10. 実装境界への落とし方

G4-3 は「方針決定」タスクなので、実装そのものではなく挿入点だけ固定する。

## 10.1 まず置き換えるべき箇所

現在の `build_pdg_context()` は、呼び出し側から

- `cache_update_paths`
- `local_dfg_paths`

をそのまま受け取っている。

この前に planner を挟むべき。

概念上は次の流れにする。

1. base graph resolve
2. `plan_pdg_build_scope(...)`
3. `build_local_dfg_for_paths(plan.local_dfg_paths)`
4. merge / propagation

## 10.2 planner は最初は pure function でよい

状態を持つ manager ではなく、まずは

```rust
fn plan_pdg_build_scope(...) -> PdgBuildScopePlan
```

の pure function にする方がよい。

理由:

- fixture test を書きやすい
- path / reason の snapshot を取りやすい
- G4-8 の engine baseline 差分とも分離しやすい

## 10.3 最初のテスト面

実装が入ったら、最低限次のテストが必要。

- planner unit test
  - seed / refs から期待 path set が出るか
- CLI regression
  - G4-2 case で selected file 理由が観測できるか（ログ or internal snapshot）
- budget test
  - Tier 3 / Tier 2 が budget 超過時に落ちる順が deterministic か

---

## 11. 固定しておきたい判断

### 判断 1

**scope planner は graph-first で選ぶ。import / path heuristic は fallback に留める。**

### 判断 2

**基本 expansion は 1-hop boundary まで。2-hop は bridge completion 1 回だけ許す。**

### 判断 3

**cache update scope と local DFG scope は分離する。**
同じ path list を使い回さない。

### 判断 4

**理由付き plan object を導入する。**
path list だけでは G4-2/G4-7/G4-8 の比較面が弱い。

### 判断 5

**G4-3 の段階では project-wide closure をしない。**
まずは G4-2 の 5 ケースを通すための最小集合を決める。

---

## 12. 一言まとめ

G4-3 の方針は次の一文に尽きる。

**PDG build scope は、seed/changed file を核に、base graph 上の direct boundary files を 1-hop だけ足し、必要な場合に限って bridge completion を 1 回だけ許す最小集合として決める。**

この方針なら

- cross-file call-site / return-flow / alias の弱点に刺さる
- scope explosion を避けられる
- cache update と local DFG build を分離できる
- 後続の G4-4 / G4-7 / G4-8 にそのまま繋がる

ので、G4 の最初の build-scope policy として十分実用的である。
