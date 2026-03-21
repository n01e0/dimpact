# G8-2: bridge scoring evidence schema extension

対象: bounded slice planner の Tier 2 / Tier 3 scoring schema

このメモは、G7-2 で公開した bridge scoring schema を、
G8-1 の不足棚卸しに沿って **schema だけ先に拡張**するためのもの。

G8-2 では runtime evidence collection はまだ実装しない。
やるのは次だけである。

- public Rust type を将来の semantic evidence / support metadata を受けられる shape に広げる
- docs / machine-readable schema / tests でその契約を固定する
- 既存 runtime output は、新 field が明示的に埋まらない限り変えない

machine-readable companion: `docs/g8-2-bridge-scoring-evidence-schema.json`

設計 input:

- `docs/g7-2-bridge-scoring-schema.md`
- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`

---

## 1. Goal / Non-goal

## Goal

- G7-2 の `ImpactSliceCandidateScoringSummary` を後方互換で拡張する
- semantic continuity / narrow fallback 用の新 evidence kind を public enum に追加する
- evidence kind と別に、将来の G8-3 / G8-4 / G8-5 が使う support metadata の置き場を作る
- support が空のときは JSON へ出さず、既存 fixture / runtime output の安定性を保つ

## Non-goal

- G8-2 で `tier2_scoring_summary()` や Ruby fallback runtime を強化すること
- G8-2 で new evidence を実際に materialize すること
- witness explanation wording を G8-2 で確定すること
- score tuple の compare order を変えること

---

## 2. Schema delta

G8-2 の public delta は次である。

```rust
pub struct ImpactSliceCandidateScoringSummary {
    pub source_kind: ImpactSliceCandidateSourceKind,
    pub lane: ImpactSliceCandidateLane,
    pub primary_evidence_kinds: Vec<ImpactSliceEvidenceKind>,
    pub secondary_evidence_kinds: Vec<ImpactSliceEvidenceKind>,
    pub score_tuple: ImpactSliceScoreTuple,
    pub support: Option<ImpactSliceCandidateSupportMetadata>,
}

pub enum ImpactSliceEvidenceKind {
    ReturnFlow,
    AssignedResult,
    AliasChain,
    ParamToReturnFlow,
    RequireRelativeEdge,
    ExplicitRequireRelativeLoad,
    ModuleCompanion,
    CompanionFileMatch,
    DynamicDispatchLiteralTarget,
    CallsitePositionHint,
    NamePathHint,
}

pub struct ImpactSliceCandidateSupportMetadata {
    pub call_graph_support: bool,
    pub local_dfg_support: bool,
    pub symbolic_propagation_support: bool,
    pub edge_certainty: Option<ImpactSliceSupportEdgeCertainty>,
}

pub enum ImpactSliceSupportEdgeCertainty {
    Confirmed,
    Inferred,
    DynamicFallback,
}
```

`ImpactSliceReasonMetadata.scoring` と `ImpactSlicePrunedCandidate.scoring` の外形は変えない。
拡張点は `scoring` object の中だけに留める。

---

## 3. New evidence kinds

G8-1 で不足として挙がった最小追加は次の 4 つである。

### `param_to_return_flow`

callee param 由来の値が alias / assignment / return を通って外へ抜ける continuity。

使いどころ:

- Rust short wrapper / passthrough
- Ruby no-paren wrapper
- G8-5 witness の winning primary evidence

### `explicit_require_relative_load`

`require_relative` の明示 load 関係が観測できたこと。

使いどころ:

- Ruby graph-first continuation と helper noise の分離
- G8-4 narrow fallback の bounded rule explanation

### `companion_file_match`

basename / module companion / require-relative companion の narrow rule で candidate を拾えたこと。

使いどころ:

- `lane = module_companion_fallback` の runtime materialization
- fallback candidate の debug / review

### `dynamic_dispatch_literal_target`

`send(:sym)` / `public_send("name")` のような dynamic dispatch で literal target まで narrowing できたこと。

使いどころ:

- G8-4 dynamic-heavy Ruby fallback
- G8-5 witness での narrow fallback explanation

---

## 4. Support metadata

G8-1 の原則どおり、

- `evidence_kind` は continuity / fallback selection の事実
- `support` は provenance / strength / certainty

を別に持つ。

G8-2 では flatten せず、`scoring.support` の 1 object にまとめる。

```json
{
  "support": {
    "call_graph_support": true,
    "local_dfg_support": true,
    "symbolic_propagation_support": true,
    "edge_certainty": "confirmed"
  }
}
```

各 field の意味:

- `call_graph_support`
  - call graph adjacency が support として使われた
- `local_dfg_support`
  - local DFG / def-use continuity が support した
- `symbolic_propagation_support`
  - symbolic propagation が support した
- `edge_certainty`
  - winner/pruned explanation で certainty を短く表したいときの hook

この support object は **evidence kind の代用品ではない**。
たとえば `return_flow` と `local_dfg_support` は別物として保持する。

---

## 5. Stability / compatibility rules

既存 runtime JSON の安定性のため、G8-2 では次を固定する。

- `support` は `None` または empty object 相当なら serialize しない
- 既存 runtime が `support = None` のままなら、`scoring` の既存 field は G7-2 と同じ shape のまま出る
- `primary_evidence_kinds` / `secondary_evidence_kinds` の意味や compare order は G7-2 のまま
- new evidence kind は enum に増えるだけで、G8-2 runtime では未使用でもよい
- field order は既存 `source_kind` / `lane` / `primary_evidence_kinds` / `secondary_evidence_kinds` / `score_tuple` を保ち、`support` は末尾に付く

### Omitted example

```json
{
  "source_kind": "graph_second_hop",
  "lane": "return_continuation",
  "primary_evidence_kinds": ["return_flow"],
  "secondary_evidence_kinds": [],
  "score_tuple": {
    "source_rank": 0,
    "lane_rank": 0,
    "primary_evidence_count": 1,
    "secondary_evidence_count": 0,
    "call_position_rank": 3,
    "lexical_tiebreak": "leaf.rs"
  }
}
```

### Populated example

```json
{
  "source_kind": "narrow_fallback",
  "lane": "module_companion_fallback",
  "primary_evidence_kinds": [
    "companion_file_match",
    "dynamic_dispatch_literal_target",
    "explicit_require_relative_load",
    "param_to_return_flow"
  ],
  "secondary_evidence_kinds": ["name_path_hint"],
  "score_tuple": {
    "source_rank": 1,
    "lane_rank": 3,
    "primary_evidence_count": 4,
    "secondary_evidence_count": 1,
    "call_position_rank": 0,
    "lexical_tiebreak": "demo/helper.rb"
  },
  "support": {
    "call_graph_support": true,
    "local_dfg_support": true,
    "symbolic_propagation_support": true,
    "edge_certainty": "dynamic_fallback"
  }
}
```

---

## 6. Task mapping

### G8-2

- enum / struct / serde contract を追加する
- docs / tests で omit rule を lock する

### G8-3

- Rust runtime で `assigned_result` / `alias_chain` / `param_to_return_flow` を semantic fact 寄りに materialize する
- 必要なら `support.local_dfg_support` を埋める

### G8-4

- Ruby runtime で `explicit_require_relative_load`
- `companion_file_match`
- `dynamic_dispatch_literal_target`
- `source_kind = narrow_fallback`
- `lane = module_companion_fallback`

を bounded に materialize する

### G8-5

- selected/pruned diff と witness explanation を `primary_evidence_kinds` + `support` 差分から組み立てる

---

## 7. Conclusion

G8-2 で固定するべきなのは、runtime 実装の先回りではなく、
**future semantic evidence を載せられる public schema を後方互換で先に作ること** である。

そのための最小差分は、

- new primary evidence kinds 4 個の追加
- `scoring.support` の nested metadata object 追加
- empty support omission による backward compatibility の維持

で十分である。
