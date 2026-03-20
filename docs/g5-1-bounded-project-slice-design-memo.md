# G5-1: direct-boundary scope の取りこぼし棚卸しと bounded project-slice 設計メモ

このメモは、G4 で入れた **direct-boundary scope expansion** を棚卸しし、
G5 で次に入れるべき **bounded project-slice** の設計単位を固定するためのもの。

G4 で入った実装は有用だった。
とくに `--with-propagation` では、changed/seed file から **直近の boundary file** まで Rust/Ruby の local DFG を広げられるようになり、
short multi-file bridge の false negative を 1 段減らせた。

ただし、それはあくまで **direct-boundary の最小拡張** であって、
G4-3 で設計上ほしかった

- first-class な scope planner
- bridge-completion を含む 2-hop policy
- cache update / local DFG / explanation の分離
- per-seed で説明可能な slice

までは、まだ実装されていない。

つまり G5 の仕事は「G4 の direct-boundary を少し広げる」ことではなく、
**direct-boundary を bounded project-slice という explicit な内部契約に置き換えること** になる。

---

## 1. G4 で実際に入ったもの

現状の build scope 拡張は、主に `src/bin/dimpact.rs` の

- `expand_related_local_dfg_paths()`
- `build_pdg_context()`

に入っている。

挙動を短く言うと次のとおり。

1. initial path は diff mode なら `changed.changed_files`、seed mode なら seed file 集合
2. `--with-propagation` のときだけ、current seeds に隣接する direct caller/callee symbol の所属 file を追加
3. その union に対して Rust/Ruby local DFG を build
4. call graph + local DFG + symbolic propagation を merge

この形で G4 が得た具体的な前進は、少なくとも次の 3 点だった。

### 1.1 direct callee 側の短い cross-file bridge

`tests/cli_pdg_propagation.rs::pdg_propagation_adds_cross_file_summary_bridge_for_direct_callee`
で固定されているように、
`main.rs -> callee.rs` のような **1-hop の cross-file call-site summary bridge** は
`--with-propagation` で回復できるようになった。

### 1.2 wrapper file 1 枚を跨ぐ return-ish bridge

`tests/cli_pdg_propagation.rs::pdg_propagation_maps_multi_file_wrapper_return_without_leaking_irrelevant_arg`
で固定されているように、
`main.rs -> wrapper.rs` までなら wrapper file の DFG node を scope に入れて
relevant arg の return-ish bridge を作れるようになった。

### 1.3 Ruby の split require-relative でも 2-file までは改善面がある

`tests/cli_pdg_propagation.rs::ruby_require_relative_alias_return_only_gains_symbolic_edges_with_propagation`
で固定されているように、
`app/runner.rb -> lib/service.rb` の 2-file 面では propagation-only の symbolic edge を足せる。

---

## 2. direct-boundary scope で残った取りこぼし

G4 の direct-boundary 拡張で効いたのは、
**seed/changed file のすぐ隣にある 1-hop file** を local DFG scope に入れるケースだった。

逆に言うと、残課題はほぼ全部
**その 1-hop 境界よりもう半歩先** にある。

ここでは、G5 で first-class に扱うべき取りこぼしを 5 つに分ける。

## 2.1 2-hop bridge completion が policy のまま止まっている

G4-3 では、1-hop direct boundary に加えて
**bridge completion を 1 回だけ許す** 方針を置いていた。

しかし実装はまだそこまで行っていない。

今の `expand_related_local_dfg_paths()` は、seed に隣接する symbol の file を 1 回足すだけで、
その boundary symbol 側からもう 1 段だけ必要な file を選ぶ契約を持たない。

### 典型例

- `main -> adapter -> core`
- `main -> service -> serializer`
- `main -> mid -> leaf`

のように、
**seed file から見た direct boundary file 自体が wrapper / alias / return-flow の中継点** になっているケース。

G4 の direct-boundary で `adapter.rs` / `mid.rb` は scope に入っても、
`core.rs` / `leaf.rb` を「bridge を閉じるために必要な file」として選ぶ policy が無い。

その結果、

- call graph の到達自体は見える
- wrapper file 側の一部 DFG は見える
- しかし 3 file 目の return / alias / summary が call-only explanation に戻りやすい

という半端な状態が残る。

## 2.2 imported-result alias continuation はまだ未固定

G4-2 で eval case として置いた
`rust-cross-file-imported-result-alias-chain` は、
G4 では regression としてまだ固定されていない。

これは direct-boundary scope だけでは十分でない可能性が高い。
理由は、見たいものが単なる `caller -> callee` ではなく、

- callee file 側の result summary
- caller file 側の alias chain (`y -> alias -> out`)
- 必要なら boundary-side のもう 1 段

を **ひとまとまりの slice** として扱う必要があるから。

direct-boundary は「隣接 file を足す」だけなので、
**imported result が caller 内 alias chain に入るための completion 条件** を表現できない。

## 2.3 `--with-pdg` と `--with-propagation` で scope policy が割れている

G4 で拡張が入ったのは `--with-propagation` 側であり、
`--with-pdg` 単体は今もほぼ file-local のまま。

これは一時的には合理的だったが、G5 では曖昧さの元になる。

同じ seed / diff に対して

- `--with-pdg`: seed/changed file ローカル寄り
- `--with-propagation`: direct boundary まで拡張

という差があると、
ユーザー視点では「PDG の scope」と「propagation の edge augment」が分離して見えにくい。

G5 では少なくとも

- scope planning は PDG layer の責務
- propagation はその slice 上で追加 bridge を作る責務

と切り分けた方がよい。

## 2.4 per-seed explanationに必要な slice reason が無い

`--per-seed` 出力自体は G3/G4 で前進したが、
PDG build scope は今も **seed ごとの plan object** を持っていない。

実際、diff + per-seed + PDG/propagation では、
先に union 的な PDG context を作り、そのあと impact を seed ごとに計算している。

この構造だと、後から

- なぜこの file が scope に入ったのか
- どの seed のために入ったのか
- その file は direct boundary なのか bridge completion なのか

を説明しにくい。

G5 で bounded project-slice を入れるなら、
**slice planning は per-seed で作り、実行時だけ union しても reason は失わない** 形にすべき。

## 2.5 cache update scope と local DFG scope が同じ配線に近い

今の `build_pdg_context()` は、
呼び出し側から受けた `cache_update_paths` と `local_dfg_paths` を扱い分けてはいるが、
実際の call site では両者が同じ集合にかなり寄っている。

G4-3 で本来やりたかったのは、少なくとも次の分離だった。

1. **cache update scope**: base graph を最新化する file 集合
2. **local DFG scope**: Rust/Ruby の局所 DFG を立てる file 集合
3. **explanation scope**: witness / debug で reason を返したい file 集合

bounded project-slice をやるなら、
この 3 つは最初から plan object の中で分離しておいた方がよい。

---

## 3. G4 eval / regression を G5 観点で並べ直す

G4 の成果と残課題を、G5 の観点で表にすると次のようになる。

| surface | G4 状態 | direct-boundary の限界 | G5 で必要なもの |
| --- | --- | --- | --- |
| Rust cross-file call-site summary | 改善済み | 1-hop short bridge まで | 非退行 control case として維持 |
| Rust wrapper / return-flow | 部分改善 | wrapper file までは入るが 3 file 目 completion は policy 不在 | bridge-completion tier |
| Rust imported-result alias chain | 未固定 | boundary-side alias continuation 条件が無い | alias-aware slice completion |
| Ruby split alias / return-flow | 2-file 面は改善 | `main -> mid -> leaf` のような slice を explicit に組めない | require-relative を含む 2-hop completion |
| Ruby dynamic target separation | guard はある | slice 成長時の FP 抑制 policy が plan object に無い | budget + reason + no-smear guard |
| `--with-pdg` vs `--with-propagation` | 非対称 | scope と edge augment が混ざる | shared slice planner |
| per-seed explainability | 弱い | union scope で reason を失う | per-seed slice plan |

この表から分かるのは、G5 の本体が「さらに 1 個 bridge を増やす」ことではなく、
**どの file 群を 1 個の bounded slice として扱うかを first-class にすること**だという点である。

---

## 4. G5 bounded project-slice の設計目標

G5 の bounded project-slice は、project-wide PDG ではない。
ほしいのは、seed/changed file を核にしつつ、
**必要な近傍 file だけを deterministic に選ぶ bounded subgraph** である。

設計目標は次の 6 つ。

### 4.1 1-hop direct-boundary を包含しつつ、2-hop completion を明示的に扱う

G4 の direct-boundary は捨てない。
むしろ bounded project-slice の **Tier 1** として内包する。

そのうえで G5 では、
Tier 1 で選ばれた boundary file から **bridge を閉じるために必要な 1 file** を
`Tier 2` として追加できるようにする。

### 4.2 scope planning を `--with-pdg` / `--with-propagation` で共有する

G5 では slice planning を先に行い、

- `--with-pdg`: その slice 上で local DFG + PDG を build
- `--with-propagation`: 同じ slice 上でさらに symbolic propagation を追加

という順に分ける。

これで「scope の差」と「edge augmentation の差」が混ざりにくくなる。

### 4.3 per-seed の reason を保持する

実行上は union してもよいが、plan object 自体は **per-seed** で持つ。

これにより後から

- この file はどの seed のために入ったか
- direct boundary と bridge completion のどちらだったか
- budget により何が落ちたか

を追えるようにする。

### 4.4 budget を first-class にする

bounded project-slice は「bounded」でなければ意味がない。
したがって G5 では budget を planner contract に入れる。

最低限必要なのは:

- cache update file budget
- local DFG file budget
- bridge completion budget（per seed）
- companion fallback budget

の 4 つ。

### 4.5 module / require-relative companion は fallback に留める

G4-3 と同様、graph-first が原則。
ただし Rust module split と Ruby `require_relative` は
path companion fallback があると安定しやすい。

G5 ではこれを **Tier 3 fallback** として残し、
main rule にしない。

### 4.6 witness / debug で slice reason を返せるようにする

G4 では witness path 自体は前進したが、
「その path がなぜ成立したか」を scope decision と結びつける面はまだ弱い。

G5 では少なくとも debug 面として

- selected paths
- selected reasons
- seed ごとの slice summary

を観測できるようにしておきたい。

---

## 5. 提案する planner contract

G5 では、`expand_related_local_dfg_paths()` の代わりに、
少なくとも概念上は次のような planner を置くのが自然。

```rust
struct PdgProjectSlicePlan {
    per_seed: Vec<SeedProjectSlicePlan>,
    union_cache_update_paths: Vec<String>,
    union_local_dfg_paths: Vec<String>,
}

struct SeedProjectSlicePlan {
    seed_symbol_id: String,
    cache_update_paths: Vec<String>,
    local_dfg_paths: Vec<String>,
    reasons_by_path: BTreeMap<String, Vec<PdgSliceReason>>,
}

enum PdgSliceReason {
    SeedFile,
    ChangedFile,
    DirectCallerFile { via_symbol_id: String },
    DirectCalleeFile { via_symbol_id: String },
    BridgeCompletionFile { via_symbol_id: String },
    ModuleCompanionFile { via_path: String },
}
```

重要なのは型名そのものではなく、次の 3 点。

1. **per-seed plan が first-class であること**
2. **cache update と local DFG が別 path list であること**
3. **path ごとに reason を持つこと**

---

## 6. 提案する selection policy

## 6.1 Tier 0: seed / changed files

常に含める。

- diff mode: changed files
- seed mode: seed 所属 file

ここは G4 と連続。

## 6.2 Tier 1: direction-aware direct boundary files

G4 の direct-boundary をそのまま planner の tier として昇格させる。

- callers: `to == seed` の隣接 symbol file
- callees: `from == seed` の隣接 symbol file
- both: その union

## 6.3 Tier 2: bridge completion file

Tier 1 で選ばれた boundary symbol / file から、
**bridge completion を 1 回だけ** 許す。

対象はまず次の 2 種類で十分。

1. `wrapper-return completion`
   - Tier 1 file が wrapper / adapter で、さらに別 file の callee summary を必要とする
2. `boundary-side alias completion`
   - Tier 1 file 側で imported result を alias chain / assigned-result chain に接続したい

G5 前半では、Tier 2 を再帰 closure にしない。
あくまで **1-hop の片側を閉じるための 1 追加** に留める。

## 6.4 Tier 3: module companion fallback

Graph-first で file 同定が痩せる場合だけ使う。

- Rust: `foo.rs` / `foo/mod.rs` / `mod.rs`
- Ruby: `require_relative` で決まる直近 companion

Tier 1/Tier 2 に紐づかない companion expansion はしない。

---

## 7. budget / stop condition の初期案

G5-1 の時点では数値を厳密化しなくてよいが、
初期案は置いておいた方が実装時に迷いにくい。

### 7.1 初期 budget 案

- `union_cache_update_paths`: 12 files まで
- `union_local_dfg_paths`: 8 files まで
- `bridge_completion_file`: per seed 1 file まで
- `module_companion_file`: per seed 1 file まで

この数値は G4-3 の budget 感覚をそのまま引き継ぐ。

### 7.2 prune 順序

budget 超過時は次の順で落とす。

1. Tier 3
2. Tier 2
3. Tier 1 の低優先候補
4. Tier 0 は最後まで保持

### 7.3 deterministic ordering

同 tier 内は少なくとも次で安定化する。

1. hop depth
2. reason priority
3. path lexical order

---

## 8. G5 でまず改善を狙うべきケース

bounded project-slice の最初の効果確認は、G4 の remaining weak cases から選ぶのが自然。

優先順は次がよい。

## 8.1 Rust wrapper-return completion

`main -> adapter -> core` 形を 3-file で固定し、
Tier 2 が実際に 3 file 目を選べることを regression にする。

ここは direct-boundary との差が一番説明しやすい。

## 8.2 Rust imported-result alias continuation

`value::make(x) -> y -> alias -> out` を cross-file で固定し、
Tier 2 の boundary-side alias completion が効くかを見る。

## 8.3 Ruby split `main -> mid -> leaf`

`require_relative` を跨ぐ 3-file chain を固定し、
mid file の temp alias と leaf file の return-ish flow を同じ bounded slice に入れられるかを見る。

## 8.4 Ruby dynamic target separation guard

slice が 2-hop まで伸びても、
`send` / `public_send` の target separation を壊さないことを non-regression にする。

---

## 9. 実装順の提案

G5-1 のあと、実装順は次が筋がよい。

### Step 1: fixed evaluation set の補強（G5-2）

- 2-hop wrapper-return
- imported-result alias continuation
- 3-file Ruby split chain

を regression 候補として固定する。

### Step 2: planner 導入（G5-3 / G5-4）

- `PdgProjectSlicePlan` 相当を導入
- direct-boundary を Tier 1 として移植
- per-seed reason を保持

### Step 3: Tier 2 completion を 1 点ずつ入れる（G5-5）

- まず Rust wrapper-return または imported-result alias continuation のどちらか一方
- その後 Ruby split chain

### Step 4: witness / debug と結び直す（G5-6 / G5-7）

- slice reason の観測面
- witness path と slice decision の結び付け
- CLI regression 追加

### Step 5: engine consistency baseline の再拡張（G5-8）

- shared slice planner の上で TS/LSP 差分を測る
- scope 差と base-graph 差を切り分ける

---

## 10. 固定しておきたい判断

### 判断 1

**G4 の direct-boundary は捨てずに Tier 1 として取り込む。**
G5 はその先の completion / explanation / budgeting を first-class にする段階である。

### 判断 2

**bounded project-slice は project-wide closure ではない。**
1-hop boundary + 1 回の completion を基本単位にする。

### 判断 3

**scope planning は `--with-pdg` / `--with-propagation` の共有層に置く。**
propagation は slice の上で bridge を足すだけにする。

### 判断 4

**per-seed plan を失わない。**
union 実行は許しても、reason と budget は seed 単位で持つ。

### 判断 5

**cache update / local DFG / explanation scope は plan object で分離する。**
G5 の bounded slice は path list の ad-hoc union ではない。

---

## 11. 一言まとめ

G4 の direct-boundary 拡張で、
`--with-propagation` は **1-hop の短い cross-file bridge** を回復できるようになった。

ただし残課題は、ほぼ全部
**その 1-hop 境界の半歩先をどう bounded に取るか** に集約されている。

したがって G5 の本体は、
local DFG scope を単に増やすことではなく、
**direct-boundary を内包した per-seed / reason-aware / budgeted な bounded project-slice planner を導入すること**
である。
