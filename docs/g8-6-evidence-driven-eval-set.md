# G8-6: evidence-driven fixed evaluation set

このメモは、G8 で入った evidence-driven selection / true narrow fallback / witness explanation を
毎回戻って比較する **固定評価セット** を定義し、
G9 で入った evidence conflict 系の改善をそこへ追加で固定するためのもの。

G5/G7 までは主に bounded slice の広げ方と selected/pruned surface を固定していた。
G8 では scope widening ではなく、
**同じ bounded slice の中で何の evidence が勝敗を決め、どう witness へ出るか** を固定した。
G9 ではその続きとして、
**negative / suppressing evidence、same-kind tie-break、dynamic fallback noise filter、losing-side reason**
を固定評価セットへ足す。

したがって現 HEAD の fixed set は、G8 の 4 ケースを維持しつつ、
G9 で増えた evidence conflict ケースを追加した **7 ケース** で構成する。

- Rust: 3 ケース
- Ruby: 4 ケース

machine-readable set: `docs/g8-6-evidence-driven-eval-set.json`

---

## 1. この評価セットで見るもの

現行の fixed set で見たいのは主に次の 7 種類。

1. **semantic evidence が positional noise に勝つこと**
   - `param_to_return_flow` が取れた Rust leaf が、後ろにある neutral helper を rank で押し切れるか
2. **negative / suppressing evidence が noisy helper を落とすこと**
   - return-ish helper noise が later callsite や lexical hint を持っていても selected されないか
3. **same-kind 候補で semantic support の強さが tie-break になること**
   - evidence kind 名が同じ Rust 候補同士でも、より強い semantic aggregation を持つ方が勝てるか
4. **true narrow fallback が bounded に materialize されること**
   - Ruby `method_missing` companion が graph-first へ滲まず narrow fallback lane として選ばれるか
5. **dynamic-send 系の generic runtime noise が fallback candidate へ残らないこと**
   - literal target family と関係のない generic runtime が filtering されるか
6. **winner/pruned 差分と losing-side reason が witness へそのまま出ること**
   - `winning_primary_evidence_kinds` / `winning_support` / `losing_side_reason` / `summary` が安定しているか
7. **CLI の selected/pruned witness surface が Ruby 競合ケースでも崩れないこと**
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

- Rust / Ruby の competition と filtering は CLI integration で固定するのが最も実態に近い
- true narrow fallback の raw boundary/candidate evidence は planner unit が最も直接
- witness explanation の最小面は `src/impact.rs` unit が最も直接

## 2.2 engine / format

- CLI lane の engine は `ts` 安定面を前提にする
- selected/pruned / scoring / witness explanation が本題のケースは `json`
- selected file の入り方自体を見たいケースだけ `dot` を併用する

## 2.3 更新ルール

この 7 ケースは、現行 evidence conflict surface の fixed baseline として扱う。
置換するなら、「より直接で、現 HEAD に既に存在する coverage へ置き換える理由」を別メモに残す。

---

## 3. 採用ケース一覧

| case_id | lang | kind | primary view | ねらい |
| --- | --- | --- | --- | --- |
| rust-param-to-return-flow-competition | rust | rank regression | dot + json + unit | `param_to_return_flow` が later helper noise に勝つ Rust Tier 2 competition |
| rust-returnish-helper-negative-evidence | rust | suppressing regression | dot + json + unit | `noisy_return_hint` が later return-ish helper を ranked-out に留める |
| rust-semantic-support-tiebreak | rust | tie-break regression | dot + json + unit | 同種 evidence の Rust 候補で `semantic_support_rank` が later callsite hint に勝つ |
| ruby-method-missing-companion-narrow-fallback | ruby | narrow fallback FN | planner unit | `method_missing` companion を true narrow fallback lane で選ぶ Tier 3 case |
| ruby-dynamic-runtime-target-family-filter | ruby | fallback noise filter | json + planner unit | generic dynamic runtime noise を literal target family hint で事前 filtering する |
| ruby-winning-evidence-source-kind-explanation | ruby | explanation regression | lib unit | selected/pruned の勝ち筋 evidence/support と losing-side reason が witness summary に出る |
| ruby-require-relative-leaf-competition | ruby | rank regression | dot + json | selected/pruned witness surface を CLI で固定する require_relative competition |

---

## 4. 各ケースの固定意図

## 4.1 `rust-param-to-return-flow-competition`

### Source

- `docs/g8-3-rust-param-to-return-evidence.md`
- `tests/cli_pdg_propagation.rs::setup_cross_file_param_passthrough_competition_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_param_passthrough_leaf_over_later_neutral_helper`
- `src/bin/dimpact.rs::collect_rust_tier2_semantic_evidence_detects_param_to_return_flow`

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
- selected は Tier 2 `bridge_completion_file` / `via_symbol_id = rust:wrapper.rs:fn:wrap:4`
- witness では `rust:step.rs:fn:step:1` 側に selected-vs-pruned explanation が付く

### Expected fields

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

## 4.2 `rust-returnish-helper-negative-evidence`

### Source

- `src/bin/dimpact.rs::bounded_slice_plan_penalizes_returnish_helper_noise_against_real_return_completion`
- `tests/cli_pdg_propagation.rs::setup_cross_file_returnish_helper_noise_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_penalizes_returnish_helper_noise_after_later_callsite`
- `src/impact.rs::selected_vs_pruned_reason_derives_losing_side_reason_from_negative_evidence`

### Fixed lane

- CLI:
  - `impact --direction callees --with-pdg --format dot`
  - `impact --direction callees --with-propagation --format json`
- test:
  - `cargo test --bin dimpact bounded_slice_plan_penalizes_returnish_helper_noise_against_real_return_completion -- --exact`
  - `cargo test --test cli_pdg_propagation pdg_slice_selection_penalizes_returnish_helper_noise_after_later_callsite -- --exact`

### Locked outcome

- selected files は `leaf.rs`, `main.rs`, `wrapper.rs`
- `zzz_final_helper.rs` は `ranked_out`
- selected/pruned は両方 `wrapper_return` competition だが、helper 側だけ `negative_evidence_kinds = [noisy_return_hint]` を持つ
- witness では compact な losing-side reason が出る

### Expected fields

- selected scoring (`leaf.rs`):
  - `source_kind = graph_second_hop`
  - `lane = return_continuation`
  - `primary_evidence_kinds = [assigned_result, return_flow]`
  - `secondary_evidence_kinds = [name_path_hint]`
- pruned scoring (`zzz_final_helper.rs`):
  - `primary_evidence_kinds = [assigned_result, return_flow]`
  - `secondary_evidence_kinds = [callsite_position_hint, name_path_hint]`
  - `negative_evidence_kinds = [noisy_return_hint]`
  - `score_tuple.negative_evidence_count = 1`
- witness reason:
  - `selected_better_by = negative_evidence_count`
  - `losing_side_reason = negative_evidence=noisy_return_hint`
  - summary:
    - `selected over zzz_final_helper.rs because it had less negative evidence (0 < 1); losing side: negative_evidence=noisy_return_hint`

### Regression to catch

- later return-ish helper が call position や lexical hint で再び勝つ
- `negative_evidence_kinds` が pruned metadata から落ちる
- losing-side reason が witness summary から消える

---

## 4.3 `rust-semantic-support-tiebreak`

### Source

- `src/bin/dimpact.rs::bounded_slice_plan_prefers_stronger_rust_semantic_support_over_later_callsite_hint`
- `tests/cli_pdg_propagation.rs::setup_cross_file_semantic_support_competition_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_stronger_rust_semantic_support_over_later_callsite_hint`

### Fixed lane

- CLI:
  - `impact --direction callees --with-pdg --format dot`
  - `impact --direction callees --with-propagation --format json`
- test:
  - `cargo test --bin dimpact bounded_slice_plan_prefers_stronger_rust_semantic_support_over_later_callsite_hint -- --exact`
  - `cargo test --test cli_pdg_propagation pdg_slice_selection_prefers_stronger_rust_semantic_support_over_later_callsite_hint -- --exact`

### Locked outcome

- selected files は `main.rs`, `steady.rs`, `wrapper.rs`
- `plain.rs` は `ranked_out`
- selected/pruned は両方 `return_continuation` かつ `primary_evidence_kinds = [assigned_result, param_to_return_flow, return_flow]`
- tie-break は `secondary_evidence_count` や `call_position_rank` より前に `semantic_support_rank` で決まる

### Expected fields

- selected scoring (`steady.rs`):
  - `primary_evidence_kinds = [assigned_result, param_to_return_flow, return_flow]`
  - `secondary_evidence_kinds = [name_path_hint]`
  - `score_tuple.semantic_support_rank = 3`
  - `support.local_dfg_support = true`
- pruned scoring (`plain.rs`):
  - `primary_evidence_kinds = [assigned_result, param_to_return_flow, return_flow]`
  - `secondary_evidence_kinds = [callsite_position_hint, name_path_hint]`
  - `score_tuple.semantic_support_rank = 2`
  - `support.local_dfg_support = true`
- witness reason:
  - `selected_better_by = semantic_support_rank`
  - summary:
    - `selected over plain.rs because it had stronger semantic support (3 > 2)`

### Regression to catch

- same-kind 候補が再び later callsite hint だけで決まる
- `semantic_support_rank` が score tuple から落ちる
- stronger semantic leaf が `plain.rs` に押し負ける

---

## 4.4 `ruby-method-missing-companion-narrow-fallback`

### Source

- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`
- `docs/g8-2-bridge-scoring-evidence-schema.json`
- `src/bin/dimpact.rs::bounded_slice_plan_selects_ruby_method_missing_companion_as_narrow_fallback`
- `tests/fixtures/ruby/analyzer_hard_cases_dynamic_dsl_method_missing_chain_v4.rb`

### Fixed lane

- test:
  - `cargo test --bin dimpact bounded_slice_plan_selects_ruby_method_missing_companion_as_narrow_fallback -- --exact`

### Locked outcome

- selected narrow fallback file は `lib/runtime.rb`
- reason は Tier 3 `module_companion_file`
- `via_path` は temp repo 上の `lib/router.rb`
- `cache_update_paths` は temp repo 上の `app/main.rb`, `lib/router.rb`, `lib/runtime.rb`
- `pruned_candidates` は空

### Expected fields

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

### Regression to catch

- true narrow fallback が materialize されず runtime companion を拾えない
- `companion_file_match` / `dynamic_dispatch_literal_target` / `explicit_require_relative_load` の 3 点が scoring に揃わない
- narrow fallback candidate が graph-first 由来の selected reason に化ける

---

## 4.5 `ruby-dynamic-runtime-target-family-filter`

### Source

- `src/bin/dimpact.rs::ruby_narrow_fallback_filters_generic_dynamic_runtime_without_target_family_hint`
- `tests/cli_pdg_propagation.rs::setup_ruby_dynamic_send_runtime_noise_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_filters_generic_ruby_dynamic_runtime_noise`

### Fixed lane

- CLI:
  - `impact --direction callees --lang ruby --with-propagation --format json`
- test:
  - `cargo test --bin dimpact ruby_narrow_fallback_filters_generic_dynamic_runtime_without_target_family_hint -- --exact`
  - `cargo test --test cli_pdg_propagation pdg_slice_selection_filters_generic_ruby_dynamic_runtime_noise -- --exact`

### Locked outcome

- selected files は `app/runner.rb`, `lib/route_runtime.rb`, `lib/service.rb`
- `pruned_candidates = []`
- `lib/aaa_runtime.rb` は candidate materialization 前に filtering される
- family-specific runtime だけが `via_path = lib/service.rb` を通って残る

### Expected fields

- boundary evidence:
  - `explicit_require_relative_loads` は `lib/aaa_runtime.rb`, `lib/route_runtime.rb` を両方含む
  - `literal_dynamic_target_hints = [route_, route_created]`
- candidate filtering:
  - `collect_ruby_narrow_fallback_candidate_evidence(lib/aaa_runtime.rb, ...) = None`
  - `collect_ruby_narrow_fallback_candidate_evidence(lib/route_runtime.rb, ...)` は `matched_call_line = 10` / `edge_certainty = dynamic_fallback`
- CLI outcome:
  - `lib/aaa_runtime.rb` は `impacted_files` に入らない
  - `lib/route_runtime.rb` は selected file に残る

### Regression to catch

- generic dynamic runtime noise が再び fallback candidate へ残る
- literal target family hint が boundary evidence から落ちる
- intended runtime (`route_runtime.rb`) まで filtering される

---

## 4.6 `ruby-winning-evidence-source-kind-explanation`

### Source

- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`
- `docs/g8-2-bridge-scoring-evidence-schema.json`
- `src/impact.rs::selected_vs_pruned_reason_derives_winning_metadata_for_source_kind_explanations`

### Fixed lane

- test:
  - `cargo test --lib selected_vs_pruned_reason_derives_winning_metadata_for_source_kind_explanations -- --exact`

### Locked outcome

- selected は `lib/leaf.rb`
- pruned は `lib/helper.rb`
- `selected_better_by = source_kind`

### Expected fields

- selected scoring support:
  - `symbolic_propagation_support = true`
  - `edge_certainty = confirmed`
- pruned scoring support:
  - `edge_certainty = dynamic_fallback`
- witness reason:
  - `winning_primary_evidence_kinds = [explicit_require_relative_load]`
  - `winning_support.symbolic_propagation_support = true`
  - `winning_support.edge_certainty = confirmed`
  - `losing_side_reason = fallback_only=narrow_fallback + edge_certainty=dynamic_fallback`
  - summary:
    - `selected over lib/helper.rb because graph_second_hop outranked narrow_fallback; winning primary evidence: explicit_require_relative_load; winning support: symbolic_propagation_support + edge_certainty=confirmed; losing side: fallback_only=narrow_fallback + edge_certainty=dynamic_fallback`

### Regression to catch

- `winning_primary_evidence_kinds` が selected/pruned 差分から導けなくなる
- `winning_support` が support 差分を落とす
- `losing_side_reason` が fallback-only / dynamic_fallback を落とす
- source-kind 勝敗の summary が evidence/support/losing-side なしの薄い文面へ戻る

---

## 4.7 `ruby-require-relative-leaf-competition`

### Source

- `tests/cli_pdg_propagation.rs::setup_ruby_require_relative_competing_leaf_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_ruby_require_relative_leaf_over_later_helper_noise`

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

### Expected fields

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

---

## 5. この set が G9 で追加で守るもの

G8 時点の fixed set は「改善が効いたこと」を押さえるには十分だったが、
evidence conflict の正規化面はまだ薄かった。
現行の 7 ケースでは、少なくとも次が固定される。

- `negative_evidence_kinds` が noisy helper suppression と witness summary の両方へ出ること
- `semantic_support_rank` が same-kind Rust competition の compare order に入ること
- Ruby true narrow fallback が literal dynamic target family によって bounded に絞られること
- witness が winning-side だけでなく losing-side の簡易理由も持てること

これにより、G9-3〜G9-6 で入った evidence conflict surface が
固定評価セットの上でも regression 監視できるようになる。
