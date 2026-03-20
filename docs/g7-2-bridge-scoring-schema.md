# G7-2: bridge-kind / evidence-kind の scoring schema

対象: bounded slice planner の Tier 2 / Tier 3 candidate ranking

このメモは、G7-1 で整理した

- `bridge_kind` が名前 heuristic と ranking priority を兼務している
- Ruby candidate が `require_relative_chain` に潰れやすい
- `selected vs pruned` の差が outcome しか見えない
- graph-first と fallback の lane が曖昧

という問題に対して、**bridge-kind / evidence-kind scoring を出力・実装・fixture で共有できる最小 schema** を定義するためのもの。

G7-2 の時点ではまだ runtime 実装は入れない。
G7-3 / G7-6 / G7-7 で、この schema を planner ranking・Ruby fallback・witness explanation に順番に落とす。

machine-readable companion: `docs/g7-2-bridge-scoring-schema.json`

---

## 1. Goal / Non-goal

## Goal

- G6-2 の `summary.slice_selection` schema を拡張し、Tier 2 / Tier 3 candidate の **ranking basis** を観測可能にする
- `bridge_kind` を bridge family label として残しつつ、**`evidence_kind` を scoring material として分離**する
- graph-first candidate と narrow fallback candidate を、同じ compare 関数で扱えても **別 lane** として説明できるようにする
- selected reason と pruned candidate の両方に、**同じ score profile** を載せられる shape を定義する
- G7-3 の Rust fixture、G7-6 の Ruby fallback fixture、G7-7 の selected-vs-pruned explanation が同じ field 群を使えるようにする

## Non-goal

- G7-2 で scoring runtime を実装すること
- G7-2 で witness explanation 文面まで確定すること
- opaque な単一 weighted score に寄せること
- project-wide closure や multi-path proof graph を設計すること

---

## 2. どこへ追加するか

G6-2 で `summary.slice_selection` は既に public schema になっている。
したがって G7-2 では新しい top-level object を増やさず、
**Tier 2 / Tier 3 の selected/pruned candidate object に `scoring` を追加**する。

具体的には次の 2 箇所で十分である。

```rust
pub struct ImpactSliceReasonMetadata {
    pub seed_symbol_id: String,
    pub tier: u8,
    pub kind: ImpactSliceReasonKind,
    pub via_symbol_id: Option<String>,
    pub via_path: Option<String>,
    pub bridge_kind: Option<ImpactSliceBridgeKind>,
    pub scoring: Option<ImpactSliceCandidateScoringSummary>,
}

pub struct ImpactSlicePrunedCandidate {
    pub seed_symbol_id: String,
    pub path: String,
    pub tier: u8,
    pub kind: ImpactSliceReasonKind,
    pub via_symbol_id: Option<String>,
    pub via_path: Option<String>,
    pub bridge_kind: Option<ImpactSliceBridgeKind>,
    pub prune_reason: ImpactSlicePruneReason,
    pub scoring: Option<ImpactSliceCandidateScoringSummary>,
}
```

この置き方にする理由は次の通り。

1. G6-2 の file-level metadata schema を壊さず拡張できる
2. selected / pruned の両方で同じ scoring shape を再利用できる
3. witness や compact explanation は後段で `slice_selection` を参照するだけでよい
4. Tier 0 / Tier 1 には scoring を出さず、**candidate competition がある Tier 2 / Tier 3 だけ**に限定できる

---

## 3. 採用する最小 shape

G7-2 で採る scoring object は次とする。

```rust
pub struct ImpactSliceCandidateScoringSummary {
    pub source_kind: ImpactSliceCandidateSourceKind,
    pub lane: ImpactSliceCandidateLane,
    pub primary_evidence_kinds: Vec<ImpactSliceEvidenceKind>,
    pub secondary_evidence_kinds: Vec<ImpactSliceEvidenceKind>,
    pub score_tuple: ImpactSliceScoreTuple,
}

pub enum ImpactSliceCandidateSourceKind {
    GraphSecondHop,
    NarrowFallback,
}

pub enum ImpactSliceCandidateLane {
    ReturnContinuation,
    AliasContinuation,
    RequireRelativeContinuation,
    ModuleCompanionFallback,
}

pub enum ImpactSliceEvidenceKind {
    ReturnFlow,
    AssignedResult,
    AliasChain,
    RequireRelativeEdge,
    ModuleCompanion,
    CallsitePositionHint,
    NamePathHint,
}

pub struct ImpactSliceScoreTuple {
    pub source_rank: u8,
    pub lane_rank: u8,
    pub primary_evidence_count: u8,
    pub secondary_evidence_count: u8,
    pub call_position_rank: u32,
    pub lexical_tiebreak: String,
}
```

これが G7 の **reviewable な lexicographic score profile** になる。

---

## 4. なぜこの shape にするか

## 4.1 `bridge_kind` は label、`scoring` は compare basis に分ける

G7-1 で整理した一番大きな問題は、
現行 runtime が `bridge_kind` に

- candidate family label
- semantic priority
- naming heuristic

を全部詰め込んでいることだった。

G7-2 ではこれを分離する。

- `bridge_kind`
  - candidate が閉じようとしている bridge family label
- `scoring`
  - ranking に使う source/lane/evidence/score tuple

これで

- `bridge_kind` は selected/pruned explanation の短い要約として残せる
- ranking は evidence の粒度で改善できる
- kind 誤推定がそのまま priority 誤りになる構造を避けられる

## 4.2 `source_kind` で graph-first と fallback を切り分ける

G7-1 では、Ruby fallback を足すと graph-first と同じ土俵に乗りやすいことを問題にした。
そのため source は最初から分ける。

- `graph_second_hop`
  - direct boundary から graph-first に収集された candidate
- `narrow_fallback`
  - module companion / require-relative companion など narrow fallback 由来の candidate

この区別は lane や evidence と独立に必要である。
理由は、同じ `alias_chain` 風の evidence があっても、
**graph-first の方を 1 段優先したい** からである。

## 4.3 lane は「どの continuity を閉じるか」で表す

lane は naming ではなく semantic closure の観点で置く。

- `return_continuation`
- `alias_continuation`
- `require_relative_continuation`
- `module_companion_fallback`

ここで重要なのは、
`wrapper_return` という bridge kind 名をそのまま lane にしないことである。
`wrapper` という naming hint ではなく、
**return / assigned-result continuity を閉じる candidate** として扱いたいからである。

## 4.4 evidence は primary / secondary に分ける

G7-1 の問題意識では、
call line や name/path hint は useful だが primary evidence ではない。
そのため evidence を 2 層に分ける。

### primary evidence

candidate がその lane に属すると説明する主要根拠。

- `return_flow`
- `assigned_result`
- `alias_chain`
- `require_relative_edge`
- `module_companion`

### secondary evidence

同 lane / 同 primary strength 内の tie-break に使う補助根拠。

- `callsite_position_hint`
- `name_path_hint`

これにより、
G6 で強すぎた

- call line
- wrapper/service/adapter の名前
- `.rb` だから全部 require-relative 扱い

といった heuristic を **補助 signal へ降格**できる。

## 4.5 `score_tuple` は比較順を露出するために持つ

G7 では単一 weighted score へ行かない。
代わりに、比較の順序そのものを output に出す。

そのため `score_tuple` には

- source rank
- lane rank
- evidence count
- positional hint
- deterministic lexical tie-break

を明示的に持たせる。

これで selected/pruned を見たときに、
「何が先に比較され、どこで負けたのか」を追いやすくなる。

---

## 5. 各 field の意味

## 5.1 `bridge_kind`

`bridge_kind` 自体は G6 の public enum を継続し、当面は次の 3 つとする。

- `wrapper_return`
- `boundary_alias_continuation`
- `require_relative_chain`

G7-2 での位置づけは
**short human-facing label** であり、score の本体ではない。

### bridge kind と lane の対応

最低限の対応は次とする。

- `wrapper_return` -> `return_continuation`
- `boundary_alias_continuation` -> `alias_continuation`
- `require_relative_chain` -> `require_relative_continuation`

`module_companion_fallback` は bridge family ではなく fallback lane なので、
当面 `kind = module_companion_file` と `scoring.lane = module_companion_fallback` で表し、
`bridge_kind` は `null` のままでよい。

## 5.2 `source_kind`

candidate の収集源。

### `graph_second_hop`

- direct boundary から graph-first に見つかった candidate
- G6 controlled 2-hop の本線
- 同程度の score なら narrow fallback より常に優先

### `narrow_fallback`

- `module_companion_file` など narrow assist candidate
- graph-first が無い、または semantic coverage が痩せるときの補助
- 同程度の score でも `graph_second_hop` に負ける

## 5.3 `lane`

candidate が何を閉じる lane か。

### `return_continuation`

- wrapper / adapter / service などを跨いだ return-ish continuation
- `return_flow` / `assigned_result` を主要根拠にしやすい

### `alias_continuation`

- boundary の先で alias / imported-result continuity を閉じる
- `alias_chain` / `assigned_result` を主要根拠にしやすい

### `require_relative_continuation`

- Ruby の split chain / return-flow / alias continuation を `require_relative` 境界越しに閉じる
- `require_relative_edge` とほかの primary evidence を併せて持つのが望ましい

### `module_companion_fallback`

- graph-first ではなく narrow companion fallback による補助 lane
- `module_companion` を primary evidence とする

## 5.4 `primary_evidence_kinds[]`

lane を支える主要根拠。
複数持ってよい。

### `return_flow`

boundary symbol と completion symbol の間に、return-ish data continuity が見える。

### `assigned_result`

call result / assigned def / pass-through def を介した continuity が見える。

### `alias_chain`

alias / reassignment / imported-result continuation の形が見える。

### `require_relative_edge`

Ruby で `require_relative` による narrow split chain 根拠がある。

### `module_companion`

Rust/Ruby の narrow companion fallback を正当化する根拠がある。

## 5.5 `secondary_evidence_kinds[]`

primary evidence が同程度の candidate を deterministic に分ける補助根拠。

### `callsite_position_hint`

relevant call position が後段 / 近接 / stop-rule 適合の面で有利。

### `name_path_hint`

path / symbol 名から wrapper / adapter / service / leaf / companion らしさが補助的に読める。
これは **決定打にはしない**。

## 5.6 `score_tuple`

ranking compare の lexicographic key。

### compare order

1. `source_rank` 昇順（低い方が強い）
2. `lane_rank` 昇順（低い方が強い）
3. `primary_evidence_count` 降順（多い方が強い）
4. `secondary_evidence_count` 降順（多い方が強い）
5. `call_position_rank` 降順（大きい方が強い）
6. `lexical_tiebreak` 昇順

### 固定値

`source_rank` は当面次で固定する。

- `graph_second_hop` = 0
- `narrow_fallback` = 1

`lane_rank` は当面次で固定する。

- `return_continuation` = 0
- `alias_continuation` = 1
- `require_relative_continuation` = 2
- `module_companion_fallback` = 3

この順は G7-1 の memo と整合する。
ただし実装で compare をこれと別ロジックにしてはいけない。
**docs / tests / runtime の比較順は一致**させる。

---

## 6. 出力規則

## 6.1 `files[*].reasons[*].scoring`

### present

- `tier = 2` かつ `kind = bridge_completion_file`
- `tier = 3` かつ `kind = module_companion_file` で ranking 対象だったもの

### omitted

- `tier = 0` (`seed_file` / `changed_file`)
- `tier = 1` (`direct_caller_file` / `direct_callee_file`)
- ranking を経ていない reason

## 6.2 `pruned_candidates[*].scoring`

`pruned_candidates` では、Tier 2 / Tier 3 candidate なら **常に present** とする。
理由は、selected されなかった candidate こそ ranking basis が必要だからである。

## 6.3 sort / stability

既存の G6-2 ordering を保ったうえで、`scoring` 内は次で固定する。

- `primary_evidence_kinds`: enum の snake_case lexical order
- `secondary_evidence_kinds`: enum の snake_case lexical order
- `score_tuple`: field 順固定

空配列は `[]` を出す。
`scoring` object があるのに evidence arrays を省略してはいけない。

---

## 7. JSON example

## 7.1 selected Tier 2 reason

```json
{
  "seed_symbol_id": "rust:main.rs:fn:caller:1",
  "tier": 2,
  "kind": "bridge_completion_file",
  "via_symbol_id": "rust:wrapper.rs:fn:wrap:3",
  "via_path": "wrapper.rs",
  "bridge_kind": "wrapper_return",
  "scoring": {
    "source_kind": "graph_second_hop",
    "lane": "return_continuation",
    "primary_evidence_kinds": [
      "assigned_result",
      "return_flow"
    ],
    "secondary_evidence_kinds": [
      "callsite_position_hint"
    ],
    "score_tuple": {
      "source_rank": 0,
      "lane_rank": 0,
      "primary_evidence_count": 2,
      "secondary_evidence_count": 1,
      "call_position_rank": 8,
      "lexical_tiebreak": "leaf.rs"
    }
  }
}
```

## 7.2 pruned candidate

```json
{
  "seed_symbol_id": "rust:main.rs:fn:caller:1",
  "path": "aaa_helper.rs",
  "tier": 2,
  "kind": "bridge_completion_file",
  "via_symbol_id": "rust:wrapper.rs:fn:wrap:3",
  "via_path": "wrapper.rs",
  "bridge_kind": "boundary_alias_continuation",
  "prune_reason": "ranked_out",
  "scoring": {
    "source_kind": "graph_second_hop",
    "lane": "alias_continuation",
    "primary_evidence_kinds": [
      "alias_chain"
    ],
    "secondary_evidence_kinds": [
      "name_path_hint"
    ],
    "score_tuple": {
      "source_rank": 0,
      "lane_rank": 1,
      "primary_evidence_count": 1,
      "secondary_evidence_count": 1,
      "call_position_rank": 4,
      "lexical_tiebreak": "aaa_helper.rs"
    }
  }
}
```

この例で重要なのは、
selected/pruned の差が

- `bridge_kind`
- `lane`
- evidence arrays
- `score_tuple`

の各層で追えることにある。

---

## 8. tests / implementation への接続方針

## 8.1 G7-3 Rust

Rust の最初の実装では少なくとも次を固定したい。

- same-side wrapper-return vs alias/noise competition
- `selected` に `scoring` が出ること
- `pruned_candidates[*].scoring` が selected と比較可能であること

## 8.2 G7-6 Ruby

Ruby では少なくとも次を固定したい。

- graph-first `require_relative_continuation` と `module_companion_fallback` が別 lane になること
- `.rb` だから全部 `require_relative_chain` 扱い、にならないこと
- fallback candidate が同程度の graph-first candidateを追い越さないこと

## 8.3 G7-7 witness explanation

G7-7 では scoring schema を新規発明せず、
この `scoring` object を参照して

- selected-vs-pruned の主要差分
- losing dimensions
- short explanation text

を組み立てる形にするのが自然である。

---

## 9. この schema で固定しておきたい判断

### 判断 1

**`bridge_kind` は compare basis ではなく family label に戻す。**
priority の本体は `scoring` に置く。

### 判断 2

**graph-first と fallback は `source_kind` で first-class に分ける。**
同点時は graph-first を勝たせる。

### 判断 3

**lane は naming ではなく continuity で置く。**
`wrapper` という語より `return_continuation` を優先する。

### 判断 4

**callsite / name/path は secondary evidence に降格する。**
G6 までより弱い位置づけにする。

### 判断 5

**selected と pruned の両方に同じ scoring object を載せる。**
そうしないと selected-vs-pruned explanation と fixture が安定しない。

---

## 10. 一言まとめ

G7-2 の scoring schema は、G6-2 の `summary.slice_selection` を土台に、Tier 2 / Tier 3 candidate に対して **`bridge_kind` を label として残しつつ、`source_kind` / `lane` / `evidence_kind` / `score_tuple` から成る reviewable な `scoring` object を selected/pruned の両方へ載せる** という契約である。