# G8-1: G7 時点で不足している evidence 種別の棚卸しと G8 設計メモ

対象: bounded slice planner / bridge scoring / true narrow fallback / witness explanation

このメモは、G7 終了時点の runtime / docs / fixtures を見直し、
**いま何の evidence が実際に出ていて、何が schema-only で、何がまだ表現不能なのか** を棚卸ししたうえで、
G8 で増やす evidence の設計方針を固定するためのもの。

前提として、G8 の目的は scope widening ではない。
やりたいのは **bounded slice を広げずに、選択精度と説明力を上げる evidence-driven selection を強くすること** である。

この棚卸しでは主に次を見直した。

- `docs/g7-1-bridge-candidate-scoring-design-memo.md`
- `docs/g7-2-bridge-scoring-schema.md`
- `docs/g7-4-planner-scope-responsibility-split.md`
- `src/impact.rs` の `ImpactSliceEvidenceKind` / `ImpactSliceCandidateLane`
- `src/bin/dimpact.rs` の `tier2_scoring_summary()` 周辺
- `tests/cli_pdg_propagation.rs` の bridge scoring / selected-vs-pruned fixture 群

---

## 1. 背景

G7 では `bridge_kind` と `evidence_kind` を分離し、
`source_kind` / `lane` / `primary_evidence_kinds` / `secondary_evidence_kinds` / `score_tuple`
から成る scoring object を導入した。

これは正しい進展だったが、G7 時点の runtime（主に `src/bin/dimpact.rs::tier2_scoring_summary()`）を実際に見ると、
次のズレが残っている。

1. **evidence の見た目は増えたが、中身はまだ lexical proxy が多い**
2. **schema にある fallback 系の evidence/lane が runtime ではほぼ未使用**
3. **winner が何で勝ったかを witness へそのまま渡せる粒度になっていない**
4. **Ruby dynamic-heavy / true narrow fallback に必要な evidence がまだ型として無い**

要するに G7 は「reviewable な scoring の骨組み」を作った段階であり、
G8 ではそこへ **実体のある evidence** を入れ直す必要がある。

---

## 2. G7 時点の evidence inventory

## 2.1 schema 上存在する種別

`src/impact.rs` の `ImpactSliceEvidenceKind` には次がある。

- `return_flow`
- `assigned_result`
- `alias_chain`
- `require_relative_edge`
- `module_companion`
- `callsite_position_hint`
- `name_path_hint`

また scoring の周辺には次もある。

- `source_kind = graph_second_hop | narrow_fallback`
- `lane = return_continuation | alias_continuation | require_relative_continuation | module_companion_fallback`

## 2.2 runtime で実際に materialize されているもの

G7 時点の runtime で実際に出ているのは、ほぼ次である。

### primary evidence

- `return_flow`
- `assigned_result`
- `alias_chain`
- `require_relative_edge`

### secondary evidence

- `callsite_position_hint`
- `name_path_hint`

### schema にはあるが実質未使用

- `module_companion`
- `source_kind = narrow_fallback`
- `lane = module_companion_fallback`

つまり G7 は、**bridge scoring の枠は作ったが、fallback evidence までは埋まっていない**。

## 2.3 いまの evidence がどこから来ているか

現状の `tier2_scoring_summary()` は、かなりの部分を次の proxy で決めている。

- symbol 名 / path 名に `wrap`, `adapter`, `service`, `leaf`, `alias`, `helper` などが含まれるか
- side 内で最後の call line か
- Ruby で `.rb` が絡むか

このため、名前は `return_flow` や `alias_chain` でも、
**実際には local DFG や symbolic propagation で continuity を見て付けているわけではない** 場面が多い。

ここが G8 で最初に直すべき本質である。

---

## 3. G7 時点で不足している evidence の棚卸し

以下では、「今の enum に無い」だけでなく、
**enum はあっても runtime が意味のある根拠としてまだ出せていないもの** も不足扱いにする。

## 3.1 bridge continuity を直接示す semantic evidence が足りない

G7 の `return_flow` / `assigned_result` / `alias_chain` は方向としては正しいが、
現在は主に lexical proxy で付いている。

G8 で欲しいのは、少なくとも次のような **実体のある continuity evidence** である。

### A. `boundary_result_assignment`

boundary call の結果が、その file 内で変数へ代入され、
その後の bridge / completion 側の接続に使われていることを示す。

必要理由:
- いまの `assigned_result` は「wrapper っぽい名前」でも付きうる
- Rust の wrapper-return / helper noise 競合を、名前でなく def-use で分けたい

### B. `return_passthrough`

boundary 側または completion 側の symbol が、受け取った値を return へ流していることを示す。

必要理由:
- いまの `return_flow` は `leaf` / `source` などの語でも立ってしまう
- 「return continuity が本当にある」ことを witness にも出したい

### C. `local_alias_flow`

同一 file 内で `a = b`, `alias = y`, 再代入チェーンなどの alias continuity が存在することを示す。

必要理由:
- Rust alias continuation の改善で最も効く根拠
- `alias_chain` を名前ベースでなく local DFG / def-use 由来にしたい

### D. `param_to_return_flow`

callee param が alias / assignment / return を経由して外へ抜けることを示す。

必要理由:
- Ruby no-paren wrapper / short bridge では、param continuity が選択理由の中核になる
- `return_passthrough` だけだと、引数由来かローカル生成値かを説明できない

## 3.2 Ruby chain / fallback を bounded に扱う evidence が足りない

G7 schema には `require_relative_edge` と `module_companion` があるが、
runtime は前者を粗く使い、後者は実質未使用である。

G8 では少なくとも次の粒度が欲しい。

### E. `explicit_require_relative_load`

`require_relative` の明示的な load 関係があることを示す。

必要理由:
- いまの `require_relative_edge` は「Ruby っぽい」candidate を広く吸いすぎる
- helper noise と actual continuation を分けるには、明示 load の観測が要る

### F. `companion_file_match`

fallback candidate が basename / module companion / require-relative companion の narrow 規則で見つかったことを示す。

必要理由:
- `module_companion` を実用化するには、どの companion rule で拾ったか最低限分かる必要がある
- true narrow fallback runtime を入れても、証拠が 1 種だと debug しにくい

### G. `dynamic_dispatch_literal_target`

`send(:sym)` / `public_send("name")` のような dynamic dispatch で、
literal target まで narrowing できたことを示す。

必要理由:
- G8-4 の dynamic-heavy case 改善で必要になる
- dynamic fallback を無差別に広げず、「literal が取れた narrow case だけ拾う」ための根拠になる

## 3.3 reliability / support strength を説明する evidence が足りない

G7 の scoring は count ベースなので、
同じ `return_flow` でも

- call graph だけで見えているのか
- local DFG が補強しているのか
- symbolic propagation まで届いているのか

が出ていない。

ただしこれは、全部を evidence enum に押し込めればよいわけではない。

G8 では次の考え方にするのがよい。

- **continuity の種類** は evidence kind で表す
- **その continuity を何が支えているか** は support metadata で表す

そのうえで最低限ほしい support は次。

### support metadata 候補

- `call_graph_support`
- `local_dfg_support`
- `symbolic_propagation_support`
- `certainty = confirmed | inferred | dynamic_fallback`

必要理由:
- winner/pruned の比較で「同じ alias continuity だが、片方は local DFG まである」と言えるようにしたい
- witness explanation に winning evidence を載せるとき、名前だけでなく強さも短く言いたい

## 3.4 witness explanation 用の「勝ち筋 evidence」が足りない

G7-7 で `selected_vs_pruned_reasons` は入ったが、
今は主に

- lane
- source kind
- evidence count
- callsite position
- lexical tiebreak

のどれで勝ったかしか言えない。

これだと

- **何の evidence が増えたから勝ったのか**
- **pruned 側に何が欠けていたのか**

がまだ弱い。

G8 では witness explanation へ、少なくとも次を返せる粒度が必要である。

### H. `winning_primary_evidence`

selected 側にあって pruned 側に無い primary evidence の要約。

例:
- `return_passthrough`
- `local_alias_flow`
- `explicit_require_relative_load`

### I. `winning_support`

同じ primary evidence でも、selected 側が持っていた support の差分。

例:
- `local_dfg_support`
- `symbolic_propagation_support`

これは厳密には evidence kind というより explanation surface だが、
G8-1 では **G8-5 が必要とする差分粒度** として先に固定しておく。

---

## 4. G8 で採る evidence 設計原則

## 4.1 evidence は「観測された事実」に寄せる

evidence kind は

- wrapper っぽい
- helper っぽい
- leaf っぽい

のような印象ラベルではなく、
**graph / DFG / propagation / narrow fallback rule で観測できた事実** を表すべきである。

## 4.2 source / lane / evidence / support を混ぜない

G7 で作った分離自体は正しいので、G8 でも保つ。

- `source_kind`: graph-first か narrow fallback か
- `lane`: return / alias / require-relative / fallback のどれを閉じるか
- `evidence_kind`: continuity や fallback selection の事実
- `support`: provenance / certainty / recovered edge の強さ

これを混ぜると、また compare と explanation が曖昧になる。

## 4.3 name/path hint は残すが、最後まで secondary に留める

`name_path_hint` は捨てなくてよい。
ただし G8 でもこれは

- human-readable annotation
- 同点時の弱い補助

に留めるべきで、primary evidence に戻してはいけない。

## 4.4 require-relative / companion / dynamic は narrow rule を明示する

Ruby fallback を強くするなら、
「Ruby だから」「近そうだから」ではなく、
**どの narrow rule に合致したから採ったか** を evidence にしないと bounded philosophy を壊す。

## 4.5 witness へ出す winning evidence は planner scoring の副産物であるべき

G8-5 の witness explanation は、別ロジックで作文するのでなく、
planner が持っている evidence / support の差分から組み立てるべきである。

---

## 5. G8 用の最小 evidence taxonomy 提案

ここでは G8-2 へ渡すため、最小の増分だけ先に置く。
まだ最終 enum 名を fix する段階ではないが、少なくとも次の束で整理するのがよい。

## 5.1 primary semantic evidence

### 既存を維持しつつ意味を強くするもの

- `return_flow`
  - G8 では lexical hint でなく、return continuity が観測できた時だけ立てる
- `assigned_result`
  - G8 では boundary result の代入・受け渡しが観測できた時だけ立てる
- `alias_chain`
  - G8 では local alias / reassignment continuity が観測できた時だけ立てる

### 新規で足したい候補

- `param_to_return_flow`
- `explicit_require_relative_load`
- `companion_file_match`
- `dynamic_dispatch_literal_target`

## 5.2 support metadata

- `call_graph_support`
- `local_dfg_support`
- `symbolic_propagation_support`
- `edge_certainty`

これは enum 追加というより `scoring` 内の補助 field に寄せるのが自然である。
少なくとも G8-2 では、「evidence array とは別に support を持てるか」を検討対象に入れるべきである。

## 5.3 secondary evidence / hint

継続利用するもの:

- `callsite_position_hint`
- `name_path_hint`

追加してもよいが優先度は低いもの:

- `symbol_kind_hint`
- `same_directory_hint`

ただし、これらは G8 の主眼ではない。
主眼は primary semantic evidence の実体化である。

---

## 6. task mapping

## 6.1 G8-2: evidence schema 拡張

このメモを踏まえ、G8-2 では最低限次を決める。

1. G7 既存 evidence の意味を lexical proxy から semantic fact に言い換える
2. 新規 evidence 候補（`param_to_return_flow`, `explicit_require_relative_load`, `companion_file_match`, `dynamic_dispatch_literal_target`）を schema に追加するか決める
3. evidence と support metadata の分離を決める
4. winner/pruned 差分に必要な surface を決める

## 6.2 G8-3: Rust evidence 強化

最初の実装ターゲットは Rust 側でよい。
特に効きやすいのは次。

- `assigned_result` を local def-use ベースに寄せる
- `alias_chain` を local alias/reassignment ベースに寄せる
- witness で `winning_primary_evidence` を最小表示する

## 6.3 G8-4: Ruby true narrow fallback runtime

Ruby では次を最小セットにするのが筋がよい。

- `explicit_require_relative_load`
- `companion_file_match`
- `dynamic_dispatch_literal_target`（dynamic-heavy case 用）
- `source_kind = narrow_fallback` の runtime materialization
- `lane = module_companion_fallback` の実装

## 6.4 G8-5: witness explanation 強化

G8-5 では少なくとも次を出したい。

- selected が持っていた winning primary evidence 1〜2 個
- pruned 側に欠けていた evidence 1 個
- 必要なら support 差分 1 個

例:

- `selected because it had return_passthrough + local_dfg_support`
- `selected because helper side only had name_path_hint, while leaf side had explicit_require_relative_load`

---

## 7. 先に固定しておきたい非目標

### 非目標 1

bounded slice の budget を広げて解決しない。
まず evidence を強くする。

### 非目標 2

broad path heuristic を fallback evidence にしない。
companion/fallback は narrow rule のみ許可する。

### 非目標 3

support metadata を evidence kind の数増やしで代用しない。
`local_dfg_support` と `return_flow` は別物である。

### 非目標 4

witness explanation 専用に別 scoring を作らない。
planner scoring の差分をそのまま使う。

---

## 8. 一言まとめ

G7 で足りなかったのは「evidence の数」そのものより、
**evidence が still lexical proxy に寄っていて、fallback / dynamic-heavy / winning explanation に必要な事実の種類がまだ型として揃っていないこと** である。

したがって G8 では、`return_flow` / `assigned_result` / `alias_chain` を実体化しつつ、
`param_to_return_flow` / `explicit_require_relative_load` / `companion_file_match` / `dynamic_dispatch_literal_target`
あたりを最小追加し、さらに evidence とは別に support metadata を持たせる設計が最も筋がよい。
