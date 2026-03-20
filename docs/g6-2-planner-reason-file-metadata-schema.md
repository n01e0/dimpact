# G6-2: planner reason を file-level metadata として出す schema

対象: `dimpact impact --with-pdg` / `--with-propagation`

このメモは、G6-1 で整理した

- planner reason が runtime/output に露出していない
- per-seed attribution と prune/budget 診断が落ちている
- witness はあるが `why this file?` は答えられない

という問題に対して、**bounded slice planner の選定理由を file-level metadata として出す最小 schema** を定義するためのもの。

G6-2 の時点ではまだ実装しない。
G6-3 で、この schema をそのまま JSON/YAML 出力へ載せられる粒度まで固める。

machine-readable companion: `docs/g6-2-planner-reason-file-metadata-schema.json`

---

## 1. Goal / Non-goal

## Goal

- bounded slice planner が選んだ file を、**file-level metadata** として観測できるようにする
- 各 file について、少なくとも次を返せるようにする
  - なぜ入ったか
  - どの seed に紐づくか
  - cache update / local DFG / explanation のどの scope に入ったか
- selected file だけでなく、**pruned candidate の最小診断** も出せるようにする
- `--per-seed` でも diff/seed mode の通常出力でも、**同じ schema** を使えるようにする

## Non-goal

- G6-2 で controlled 2-hop policy そのものを決め切ること
- witness schema をここで拡張し切ること
- DOT / HTML / reporter まで同時に変えること
- planner を project-wide closure にすること

---

## 2. 置き場所

planner reason は `ImpactOutput` の file-level metadata なので、
`summary` 配下に置く。

```rust
pub struct ImpactSummary {
    pub by_depth: Vec<ImpactDepthBucket>,
    pub affected_modules: Vec<ImpactAffectedModule>,
    pub risk: Option<ImpactRiskSummary>,
    pub slice_selection: Option<ImpactSliceSelectionSummary>,
}
```

置き場所を `summary` にする理由は次の 4 つ。

1. 既存の `by_depth` / `affected_modules` / `risk` と同じ explainability surface に載せられる
2. 通常 JSON/YAML と `--per-seed` の両方で同じ nesting を使える
3. `impacted_files` と universe が違うことを明示しやすい
4. G6-8 で witness と軽く接続するときも `summary` 配下の文脈に置きやすい

ここで重要なのは、`summary.slice_selection.files[*].path` は
**planner が build scope/explanation scope に選んだ file** を表し、
`impacted_files` とは別 universe だという点である。

- `impacted_files`: 影響結果として impacted symbol を持つ file
- `slice_selection.files`: bounded slice planner が context/build/explanation 用に選んだ file

したがって、slice に入っても impacted ではない file があってよい。

---

## 3. 採用する shape

最小 contract は次の shape とする。

```rust
pub struct ImpactSliceSelectionSummary {
    pub planner: ImpactSlicePlannerKind,
    pub files: Vec<ImpactSliceFileMetadata>,
    pub pruned_candidates: Vec<ImpactSlicePrunedCandidate>,
}

pub enum ImpactSlicePlannerKind {
    BoundedSlice,
}

pub struct ImpactSliceFileMetadata {
    pub path: String,
    pub scopes: ImpactSliceScopes,
    pub reasons: Vec<ImpactSliceReasonMetadata>,
}

pub struct ImpactSliceScopes {
    pub cache_update: bool,
    pub local_dfg: bool,
    pub explanation: bool,
}

pub struct ImpactSliceReasonMetadata {
    pub seed_symbol_id: String,
    pub tier: u8,
    pub kind: ImpactSliceReasonKind,
    pub via_symbol_id: Option<String>,
    pub via_path: Option<String>,
    pub bridge_kind: Option<ImpactSliceBridgeKind>,
}

pub enum ImpactSliceReasonKind {
    SeedFile,
    ChangedFile,
    DirectCallerFile,
    DirectCalleeFile,
    BridgeCompletionFile,
    ModuleCompanionFile,
}

pub enum ImpactSliceBridgeKind {
    WrapperReturn,
    BoundaryAliasContinuation,
    RequireRelativeChain,
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
}

pub enum ImpactSlicePruneReason {
    AlreadySelected,
    BridgeBudgetExhausted,
    CacheUpdateBudgetExhausted,
    LocalDfgBudgetExhausted,
    RankedOut,
}
```

G6-2 では、この shape を **最小採用案** とする。

---

## 4. なぜこの shape にするか

## 4.1 file を first-class にする

この task の主題は symbol metadata ではなく、
**planner が選んだ file を first-class にすること** である。

そのためトップレベルは

- `files: Vec<ImpactSliceFileMetadata>`

を主とする。

ここで各 file が持つ最小情報は

- path
- scope split
- reasons

の 3 つで十分。

## 4.2 per-seed attribution は reason object に埋め込む

per-seed plan を別 block として丸ごと重複させる案もあるが、
G6-2 では **file-level metadata** が主題なので、
file object の `reasons[]` に `seed_symbol_id` を埋め込む。

これで通常出力でも

- この file は seed A 由来か
- seed B でも選ばれたか
- seed A では direct boundary、seed B では bridge completion か

を 1 つの file object 上で表現できる。

## 4.3 selected file と pruned candidate を分ける

`why this file?` を出すだけなら `files[]` だけでも足りる。
ただし G6-1 で整理した通り、失敗時には

- planner が候補を見つけられなかったのか
- 見つけたが budget で落としたのか
- ranking で他候補に負けたのか

を区別したい。

そのため G6-2 では、selected file とは別に

- `pruned_candidates: Vec<ImpactSlicePrunedCandidate>`

を持つ。

ただしここでは full trace ではなく、**最小診断** に留める。

## 4.4 `scopes` を bool 3 本で持つ

G5-1 / G5-3 / G6-1 で一貫して重要なのは、
次の 3 scope を分離することだった。

- cache update
- local DFG
- explanation

これを file-level metadata として見せるため、
`ImpactSliceFileMetadata` には

```rust
pub struct ImpactSliceScopes {
    pub cache_update: bool,
    pub local_dfg: bool,
    pub explanation: bool,
}
```

を持たせる。

配列より bool を選ぶ理由は、

- JSON consumer が単純に読める
- `.rs` / `.rb` 以外で `local_dfg=false` を自然に表現できる
- 将来 scope が増えたときも additive に拡張できる

からである。

---

## 5. 各 field の意味

## 5.1 `planner`

現時点では `"bounded_slice"` 固定とする。

これは feature flag ではなく、
**どの planner contract がこの metadata を生成したか** を示す discriminator である。

将来 planner が増えても、schema を壊さず共存できる。

## 5.2 `files[*].path`

planner が最終的に selected した file path。

- 相対 path
- `impacted_files` と同じ path representation を使う
- 重複なし
- file list は path 昇順

## 5.3 `files[*].scopes`

その file がどの runtime scope に入ったかを示す。

### `cache_update`

cache update 対象なら `true`。

### `local_dfg`

local DFG build 対象なら `true`。
`.rs` / `.rb` 以外では通常 `false` になりうる。

### `explanation`

`why this file?` / witness 接続など explanation surface に残す対象なら `true`。

G6-2 では、bounded slice planner が選んだ file は原則 `explanation=true` を想定する。
ただし将来、cache 都合だけで retained される file を分けたくなっても拡張可能なように bool を独立させる。

## 5.4 `files[*].reasons[]`

その file が selected された理由群。

重要なのは、これは **1 file に対して複数入ってよい** こと。

典型例:

- seed A では `changed_file`
- seed B では `direct_callee_file`
- seed C では `bridge_completion_file`

のように、同じ file が複数 seed / 複数 reason で入る場合がある。

### `seed_symbol_id`

その reason がどの seed に紐づくかを示す。

通常出力では per-seed attribution の担い手になり、
`--per-seed` 出力では多くの場合 1 seed に収束する。

### `tier`

planner tier を numeric に持つ。

- `0`: root (`seed_file` / `changed_file`)
- `1`: direct boundary (`direct_caller_file` / `direct_callee_file`)
- `2`: bridge completion (`bridge_completion_file`)
- `3`: module companion fallback (`module_companion_file`)

`kind` からある程度推測できるが、
G6-1 で「tier の概念が runtime に残っていない」と整理したため、
**tier 自体を first-class field として戻す。**

### `kind`

選定理由の種別。

- `seed_file`
- `changed_file`
- `direct_caller_file`
- `direct_callee_file`
- `bridge_completion_file`
- `module_companion_file`

この naming は G4-3 / G5-3 から連続にする。

### `via_symbol_id`

`direct_*` と `bridge_completion_file` で使う optional field。

- direct boundary: その file を選ぶ根拠になった adjacent symbol
- bridge completion: completion を起こした boundary-side symbol

`seed_file` / `changed_file` / `module_companion_file` では通常 `null`。

### `via_path`

`module_companion_file` のときに使う optional field。

- どの already-selected path の companion として入ったか

graph-first reason とは別に、fallback を small に使った痕跡を残す。

### `bridge_kind`

`bridge_completion_file` のときに使う optional field。

最小 enum は次の 3 つ。

- `wrapper_return`
- `boundary_alias_continuation`
- `require_relative_chain`

G6-2 では controlled 2-hop policy 本体までは決めないが、
**completion がどの kind の bridge を閉じるつもりだったか** は schema 上に先に置いてよい。

## 5.5 `pruned_candidates[]`

selected されなかった候補の最小診断。

`files[]` と違い、こちらは final output membership を持たない。

必要最小限として次を返す。

- `seed_symbol_id`
- `path`
- `tier`
- `kind`
- `via_symbol_id`
- `via_path`
- `bridge_kind`
- `prune_reason`

ここでは `scopes` は持たない。
理由は、pruned candidate は scope に入っていないため、
**落とした理由の方が本質** だからである。

### `prune_reason`

最小 enum は次を採用する。

- `already_selected`
- `bridge_budget_exhausted`
- `cache_update_budget_exhausted`
- `local_dfg_budget_exhausted`
- `ranked_out`

G6-2 ではこれ以上増やさない。
まずは

- 既に入っていた
- budget に当たった
- ranking に負けた

を区別できれば十分である。

---

## 6. JSON schema example

## 6.1 通常出力

```json
{
  "changed_symbols": [
    {
      "id": "rust:src/main.rs:fn:caller:3",
      "name": "caller",
      "kind": "function",
      "file": "src/main.rs",
      "range": { "start_line": 3, "end_line": 7 },
      "language": "rust"
    }
  ],
  "impacted_symbols": [
    {
      "id": "rust:src/adapter.rs:fn:wrap:1",
      "name": "wrap",
      "kind": "function",
      "file": "src/adapter.rs",
      "range": { "start_line": 1, "end_line": 3 },
      "language": "rust"
    }
  ],
  "impacted_files": ["src/adapter.rs"],
  "edges": [],
  "impacted_by_file": {
    "src/adapter.rs": [
      {
        "id": "rust:src/adapter.rs:fn:wrap:1",
        "name": "wrap",
        "kind": "function",
        "file": "src/adapter.rs",
        "range": { "start_line": 1, "end_line": 3 },
        "language": "rust"
      }
    ]
  },
  "impacted_witnesses": {},
  "summary": {
    "by_depth": [],
    "affected_modules": [],
    "slice_selection": {
      "planner": "bounded_slice",
      "files": [
        {
          "path": "src/adapter.rs",
          "scopes": {
            "cache_update": true,
            "local_dfg": true,
            "explanation": true
          },
          "reasons": [
            {
              "seed_symbol_id": "rust:src/main.rs:fn:caller:3",
              "tier": 1,
              "kind": "direct_callee_file",
              "via_symbol_id": "rust:src/adapter.rs:fn:wrap:1",
              "via_path": null,
              "bridge_kind": null
            }
          ]
        },
        {
          "path": "src/main.rs",
          "scopes": {
            "cache_update": true,
            "local_dfg": true,
            "explanation": true
          },
          "reasons": [
            {
              "seed_symbol_id": "rust:src/main.rs:fn:caller:3",
              "tier": 0,
              "kind": "changed_file",
              "via_symbol_id": null,
              "via_path": null,
              "bridge_kind": null
            }
          ]
        },
        {
          "path": "src/value.rs",
          "scopes": {
            "cache_update": true,
            "local_dfg": true,
            "explanation": true
          },
          "reasons": [
            {
              "seed_symbol_id": "rust:src/main.rs:fn:caller:3",
              "tier": 2,
              "kind": "bridge_completion_file",
              "via_symbol_id": "rust:src/adapter.rs:fn:wrap:1",
              "via_path": null,
              "bridge_kind": "boundary_alias_continuation"
            }
          ]
        }
      ],
      "pruned_candidates": [
        {
          "seed_symbol_id": "rust:src/main.rs:fn:caller:3",
          "path": "src/side.rs",
          "tier": 2,
          "kind": "bridge_completion_file",
          "via_symbol_id": "rust:src/adapter.rs:fn:wrap:1",
          "via_path": null,
          "bridge_kind": "boundary_alias_continuation",
          "prune_reason": "bridge_budget_exhausted"
        }
      ]
    }
  }
}
```

## 6.2 `--per-seed`

`--per-seed` では既存の nesting を保ち、
`output.summary.slice_selection` にそのまま入れる。

```json
[
  {
    "changed_symbol": {
      "id": "rust:src/main.rs:fn:caller:3",
      "name": "caller",
      "kind": "function",
      "file": "src/main.rs",
      "range": { "start_line": 3, "end_line": 7 },
      "language": "rust"
    },
    "impacts": [
      {
        "direction": "callees",
        "output": {
          "changed_symbols": [
            {
              "id": "rust:src/main.rs:fn:caller:3",
              "name": "caller",
              "kind": "function",
              "file": "src/main.rs",
              "range": { "start_line": 3, "end_line": 7 },
              "language": "rust"
            }
          ],
          "impacted_symbols": [],
          "impacted_files": [],
          "edges": [],
          "impacted_by_file": {},
          "impacted_witnesses": {},
          "summary": {
            "by_depth": [],
            "affected_modules": [],
            "slice_selection": {
              "planner": "bounded_slice",
              "files": [
                {
                  "path": "src/main.rs",
                  "scopes": {
                    "cache_update": true,
                    "local_dfg": true,
                    "explanation": true
                  },
                  "reasons": [
                    {
                      "seed_symbol_id": "rust:src/main.rs:fn:caller:3",
                      "tier": 0,
                      "kind": "seed_file",
                      "via_symbol_id": null,
                      "via_path": null,
                      "bridge_kind": null
                    }
                  ]
                }
              ],
              "pruned_candidates": []
            }
          }
        }
      }
    ]
  }
]
```

G6-2 では、通常出力と `--per-seed` で **schema は同一** とする。
違いは outer nesting だけで、`slice_selection` 自体は同じでよい。

---

## 7. optionality / 安定出力方針

## 7.1 `summary.slice_selection` 自体

- bounded slice planner が動かない通常 impact pathでは **省略可**
- `--with-pdg` / `--with-propagation` で bounded slice planner が動く場合は **常に出す**

つまり外部契約としては

- planner 未使用 → field omitted
- planner 使用 → field present

を採る。

## 7.2 `files` / `pruned_candidates`

`slice_selection` が present な場合は、次を常時出力する。

- `files`: 空でも `[]`
- `pruned_candidates`: 空でも `[]`

理由:

- consumer が optional 分岐を減らせる
- selected 0 / pruned 0 と field 未実装を区別しやすい
- by-depth と同じ「feature が動いたなら空配列でも field を出す」方針に揃えられる

---

## 8. 整列規則

schema は minimal にするが、順序は deterministic に固定する。

### `files`

- `path` 昇順

### `files[*].reasons`

- `seed_symbol_id` 昇順
- `tier` 昇順
- `kind` 昇順（enum の宣言順でなく snake_case lexical）
- `via_symbol_id` 昇順（`null` は最後）
- `via_path` 昇順（`null` は最後）
- `bridge_kind` 昇順（`null` は最後）

### `pruned_candidates`

- `seed_symbol_id` 昇順
- `tier` 昇順
- `path` 昇順
- `kind` 昇順
- `prune_reason` 昇順

重複する object は dedup する。

---

## 9. `with_edges` / witness / impacted_files との関係

## 9.1 `with_edges` とは独立

`slice_selection` は planner metadata なので、`with_edges` の true/false に依存しない。

- `with_edges=false` でも出す
- `with_edges=true` でも同じ schema を出す

理由は、slice reason は traversal edge dump ではなく、
**planner decision metadata** だからである。

## 9.2 witness とは別 object のままにする

G6-2 では witness と直接結合しない。

- `impacted_witnesses`: symbol/path explanation
- `summary.slice_selection`: file selection explanation

という責務分離を保つ。

G6-8 で必要になれば、witness 側から `path[*].edge.file` や symbol file と照合して
`slice_selection.files[*]` を参照できるようにする。

## 9.3 `impacted_files` とは混ぜない

`impacted_files` は result universe、`slice_selection.files` は planner universe なので混ぜない。

これにより、

- impacted ではないが explanation 用に retained された file
- selected されたが local DFG を建てない file

を素直に表現できる。

---

## 10. この schema で固定したいこと

G6-3 以降の実装では、少なくとも次を fixture/test で固定したい。

1. root / direct boundary / bridge completion / module companion の区別
2. file ごとの `scopes` split
3. aggregated 出力でも `seed_symbol_id` により per-seed attribution が残ること
4. `pruned_candidates` で budget / ranking / already-selected を区別できること
5. `--per-seed` で outer nesting だけが変わり、inner schema は同じこと

---

## 11. 一言まとめ

G6-2 の schema は、`summary.slice_selection` の下に

- selected file metadata (`files[]`)
- selected reason metadata (`files[*].reasons[]`)
- pruned candidate diagnostics (`pruned_candidates[]`)

を置き、**file を first-class にしつつ per-seed attribution と scope split を失わない最小契約** とする。
