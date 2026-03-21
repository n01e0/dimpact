# G8-6: evidence-driven fixed evaluation set

このメモは、G8 で入った evidence-driven selection / true narrow fallback / winning-evidence explanation を
毎回戻って比較する **固定評価セット** を決めるためのもの。

G5/G7 までは主に bounded slice の広げ方と selected/pruned surface を固定していた。
G8 でまず固定したいのは scope widening ではなく、
**同じ bounded slice の中で何の evidence が勝敗を決め、どう explanation に出るか** である。

したがって G8-6 では、現在の HEAD にすでにある planner/unit/integration coverage のうち、
evidence-driven 振る舞いを最も直接に固定できる 4 ケースを採用する。

- Rust: 1 ケース
- Ruby: 3 ケース

machine-readable set: `docs/g8-6-evidence-driven-eval-set.json`

---

## 1. この評価セットで見るもの

G8-6 で見たいのは、主に次の 4 種類。

1. **semantic evidence が positional noise に勝つこと**
   - `param_to_return_flow` が取れた Rust leaf が、後ろにある neutral helper を rank で押し切れるか
2. **true narrow fallback が bounded に materialize されること**
   - Ruby `method_missing` companion が graph-first へ滲まず、narrow fallback lane として選ばれるか
3. **winner/pruned 差分が witness へそのまま出ること**
   - `winning_primary_evidence_kinds` / `winning_support` / `summary` が selected/pruned 差分として安定しているか
4. **CLI の selected/pruned witness surface が Ruby 競合ケースでも崩れないこと**
   - selected file と ranked-out helper の分離、説明 slice、summary 文面が保たれるか

---

## 2. 固定ルール

## 2.1 lane の扱い

この set では、**CLI lane と test lane を両方 first-class に扱う**。

- CLI lane:
  - `impact --with-pdg --format dot`
  - `impact --with-propagation --format json`
- test lane:
  - `cargo test --test cli_pdg_propagation ...`
  - `cargo test --bin dimpact ...`
  - `cargo test --lib ...`

理由:

- Rust competition と Ruby require_relative competition は CLI integration で固定済み
- Ruby true narrow fallback と winning-evidence summary の最小面は、現 HEAD では planner/unit test が最も直接

## 2.2 engine / format

- CLI lane の engine は `ts` 安定面を前提にする
- selected/pruned / scoring / witness explanation が本題のケースは `json`
- selected file の入り方自体を見たいケースだけ `dot` を併用する

## 2.3 更新ルール

この 4 ケースは G8 前半では入れ替えない。
置換するなら、「より直接で、現 HEAD に既に存在する coverage へ置き換える理由」を別メモに残す。

---

## 3. 採用ケース一覧

| case_id | lang | kind | primary view | ねらい |
| --- | --- | --- | --- | --- |
| rust-param-to-return-flow-competition | rust | rank regression | dot + json + unit | `param_to_return_flow` が later helper noise に勝つ Rust Tier 2 competition |
| ruby-method-missing-companion-narrow-fallback | ruby | narrow fallback FN | planner unit | `method_missing` companion を true narrow fallback lane で選ぶ Tier 3 case |
| ruby-winning-evidence-source-kind-explanation | ruby | explanation regression | lib unit | selected/pruned の勝ち筋 evidence/support が witness summary に出る case |
| ruby-require-relative-leaf-competition | ruby | rank regression | dot + json | selected/pruned witness surface を CLI で固定する require_relative competition |

---

## 4. 各ケースの固定意図

## 4.1 `rust-param-to-return-flow-competition`

### Source

- `docs/g8-3-rust-param-to-return-evidence.md`
- `tests/cli_pdg_propagation.rs::setup_cross_file_param_passthrough_competition_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_param_passthrough_leaf_over_later_neutral_helper`
- `src/bin/dimpact.rs::collect_rust_tier2_semantic_evidence_detects_param_to_return_flow`

### Fixture / repro shape

- `step.rs`
  - `input -> forwarded -> return` の param passthrough leaf
- `later.rs`
  - 同じ wrapper から呼ばれる neutral helper noise
- `wrapper.rs`
  - `step::step(a)` と `later::later(a)` を両方呼ぶ
- `main.rs`
  - `input = 1 -> 2` の diff

### Fixed lane

- CLI:
  - `impact --direction callees --with-pdg --format dot`
  - `impact --direction callees --with-propagation --format json`
- test:
  - `cargo test --test cli_pdg_propagation pdg_slice_selection_prefers_param_passthrough_leaf_over_later_neutral_helper -- --exact`
  - `cargo test --bin dimpact collect_rust_tier2_semantic_evidence_detects_param_to_return_flow -- --exact`

### Locked outcome

- selected files は `main.rs`, `step.rs`, `wrapper.rs`
- `later.rs` は `pruned_candidates[*].prune_reason = ranked_out`
- `step.rs` は `via_symbol_id = rust:wrapper.rs:fn:wrap:4` の Tier 2 bridge completion
- witness では `rust:step.rs:fn:step:1` 側に selected-vs-pruned explanation が付く

### Expected evidence / support / explanation fields

- selected scoring:
  - `source_kind = graph_second_hop`
  - `lane = return_continuation`
  - `primary_evidence_kinds = [assigned_result, param_to_return_flow, return_flow]`
  - `secondary_evidence_kinds = [name_path_hint]`
  - `support.local_dfg_support = true`
- pruned scoring (`later.rs`):
  - `primary_evidence_kinds = [assigned_result, return_flow]`
  - `secondary_evidence_kinds = [callsite_position_hint, name_path_hint]`
- witness reason:
  - `selected_better_by = primary_evidence_count`
  - `winning_primary_evidence_kinds = [param_to_return_flow]`
  - `winning_support.local_dfg_support = true`
  - summary:
    - `selected over later.rs because it had more primary evidence (3 > 2); winning primary evidence: param_to_return_flow; winning support: local_dfg_support`

### Regression to catch

- last-call / lexical noise が再び勝って `later.rs` が選ばれる
- `param_to_return_flow` が selected scoring から落ちる
- `winning_support.local_dfg_support` が witness explanation から消える

---

## 4.2 `ruby-method-missing-companion-narrow-fallback`

### Source

- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`
- `docs/g8-2-bridge-scoring-evidence-schema.json`
- `src/bin/dimpact.rs::bounded_slice_plan_selects_ruby_method_missing_companion_as_narrow_fallback`
- `tests/fixtures/ruby/analyzer_hard_cases_dynamic_dsl_method_missing_chain_v4.rb`

### Fixture / repro shape

planner unit test が temp repo を組み立てる。

- `app/main.rb`
  - `Router.new.run_created("alpha")`
- `lib/router.rb`
  - `require_relative "runtime"`
  - `@runtime.public_send("route_created", payload)`
- `lib/runtime.rb`
  - `RuntimeProxy#method_missing`
  - `respond_to_missing?`

### Fixed lane

- test:
  - `cargo test --bin dimpact bounded_slice_plan_selects_ruby_method_missing_companion_as_narrow_fallback -- --exact`

### Locked outcome

- selected narrow fallback file は `lib/runtime.rb`
- reason は Tier 3 `module_companion_file`
- `via_path` は temp repo 上の `lib/router.rb`
- `cache_update_paths` は temp repo 上の `app/main.rb`, `lib/router.rb`, `lib/runtime.rb`
- `pruned_candidates` は空

### Expected evidence / support / explanation fields

- boundary evidence:
  - `explicit_require_relative_loads` は temp repo 上の `lib/runtime.rb` を含む
  - `literal_dynamic_targets = { "route_created": 9 }`
- candidate evidence:
  - `matched_call_line = 9`
  - `edge_certainty = dynamic_fallback`
- selected scoring:
  - `source_kind = narrow_fallback`
  - `lane = module_companion_fallback`
  - `primary_evidence_kinds = [companion_file_match, dynamic_dispatch_literal_target, explicit_require_relative_load]`
  - `secondary_evidence_kinds = []`
  - `support.edge_certainty = dynamic_fallback`
- explanation surface:
  - selected/pruned explanation は不要
  - `pruned_candidates = []` のまま narrow fallback 単独選択で終わる

### Regression to catch

- true narrow fallback が materialize されず runtime companion を拾えない
- `companion_file_match` / `dynamic_dispatch_literal_target` / `explicit_require_relative_load` の 3 点が scoring に揃わない
- narrow fallback candidate が graph-first 由来の selected reason に化ける

---

## 4.3 `ruby-winning-evidence-source-kind-explanation`

### Source

- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`
- `docs/g8-2-bridge-scoring-evidence-schema.json`
- `src/impact.rs::selected_vs_pruned_reason_derives_winning_metadata_for_source_kind_explanations`

### Fixture / repro shape

`build_selected_vs_pruned_reasons()` を直接叩く unit case。

- selected path: `lib/leaf.rb`
- pruned path: `lib/helper.rb`
- `via_path = lib/service.rb`
- selected scoring:
  - `source_kind = graph_second_hop`
  - `lane = module_companion_fallback`
  - primary evidence は `companion_file_match + explicit_require_relative_load`
- pruned scoring:
  - `source_kind = narrow_fallback`
  - primary evidence は `companion_file_match` のみ

### Fixed lane

- test:
  - `cargo test --lib selected_vs_pruned_reason_derives_winning_metadata_for_source_kind_explanations -- --exact`

### Locked outcome

- selected は `lib/leaf.rb`
- pruned は `lib/helper.rb`
- `selected_better_by = source_kind`

### Expected evidence / support / explanation fields

- selected scoring support:
  - `symbolic_propagation_support = true`
  - `edge_certainty = confirmed`
- pruned scoring support:
  - `edge_certainty = dynamic_fallback`
- witness reason:
  - `winning_primary_evidence_kinds = [explicit_require_relative_load]`
  - `winning_support.symbolic_propagation_support = true`
  - `winning_support.edge_certainty = confirmed`
  - summary:
    - `selected over lib/helper.rb because graph_second_hop outranked narrow_fallback; winning primary evidence: explicit_require_relative_load; winning support: symbolic_propagation_support + edge_certainty=confirmed`

### Regression to catch

- `winning_primary_evidence_kinds` が selected/pruned 差分から導けなくなる
- `winning_support` が support 差分を落とす
- source-kind 勝敗の summary が evidence/support なしの薄い文面へ戻る

---

## 4.4 `ruby-require-relative-leaf-competition`

### Source

- `tests/cli_pdg_propagation.rs::setup_ruby_require_relative_competing_leaf_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_ruby_require_relative_leaf_over_later_helper_noise`

### Fixture / repro shape

- `lib/leaf.rb`
  - `alias_value = value; return alias_value`
- `lib/zzz_helper.rb`
  - later helper noise
- `lib/service.rb`
  - `require_relative 'leaf'`
  - `require_relative 'zzz_helper'`
  - `wrapped = finish(alias_value)` と `helper_noise(value)` の競合
- `app/runner.rb`
  - `prepared = seed + 1 -> seed + 2` の diff

### Fixed lane

- CLI:
  - `impact --direction callees --lang ruby --with-pdg --format dot`
  - `impact --direction callees --lang ruby --with-propagation --format json`
  - `impact --direction callees --lang ruby --with-propagation --format dot`
- test:
  - `cargo test --test cli_pdg_propagation pdg_slice_selection_prefers_ruby_require_relative_leaf_over_later_helper_noise -- --exact`

### Locked outcome

- selected files は `app/runner.rb`, `lib/leaf.rb`, `lib/service.rb`
- `lib/zzz_helper.rb` は `ranked_out`
- helper witness の `selected_files_on_path` には `lib/zzz_helper.rb` が入らない
- propagation dot では `app/runner.rb:use:prepared:5 -> app/runner.rb:def:reply:5` の bridge が出る

### Expected evidence / support / explanation fields

- selected scoring (`lib/leaf.rb`):
  - `source_kind = graph_second_hop`
  - `lane = return_continuation`
  - `primary_evidence_kinds = [assigned_result, return_flow]`
  - `secondary_evidence_kinds = [name_path_hint]`
- pruned scoring (`lib/zzz_helper.rb`):
  - `source_kind = graph_second_hop`
  - `lane = require_relative_continuation`
  - `primary_evidence_kinds = [require_relative_edge]`
  - `secondary_evidence_kinds = [callsite_position_hint]`
- witness reason:
  - `selected_better_by = lane`
  - `winning_primary_evidence_kinds = [assigned_result, return_flow]`
  - summary:
    - `selected over lib/zzz_helper.rb because return_continuation outranked require_relative_continuation; winning primary evidence: assigned_result + return_flow`

### Regression to catch

- helper noise が lane/position で勝って `lib/leaf.rb` を押しのける
- ranked-out helper が explanation slice に混ざる
- Ruby CLI witness surface から winning evidence の summary が落ちる
