# G2-1: G1 summary の評価観点と評価セット候補棚卸し

G1 で追加した `summary.by_depth` / `summary.risk` / `summary.affected_modules` を、
「出る」だけでなく **実運用で triage を速くするか** という観点で評価するためのメモ。

ここでは次の 2 点を先に固める。

1. 何を見れば G1 summary を良し悪し判定できるか
2. その観点を回せる `実 diff` / `fixture` 候補が、現状 repo のどこにあるか

G2-3 以降では、この棚卸し結果からケースを絞って固定評価セット化する。

## 1. 評価で答えたい問い

summary layer が本当に役立つなら、利用者は raw な `impacted_symbols` / `edges` を全部読む前に、
少なくとも次の 3 つを短時間で判断できるはず。

1. **広さ**: 影響は direct 中心か、transitive に伸びているか
2. **優先度**: 先にレビュー/CI ケアすべき diff か
3. **着手点**: どの directory / module から見始めるべきか

したがって G2 の評価観点は、実装内部の正しさだけでなく、
**人が diff を読む順番を短縮できるか** まで含める。

## 2. 評価観点

### 2.1 `by_depth`: direct / transitive の分離が読む順番に効くか

見る点:

- `depth=1` が「変更点に直接ぶら下がる caller 群」を素直に表しているか
- `depth>=2` が chain / fan-out の広がりを雑にでも掴めるか
- bucket 合計が `impacted_symbols` と食い違わないか
- `--with-edges` なし / `--per-seed` / confidence filter 後でも解釈が破綻しないか

良い状態:

- `depth=1` が 0〜少数なら「まず変更周辺だけ見ればよい」と読める
- `depth>=2` が増えるケースでは「レビュー範囲が呼び出し側へ波及している」と一目で読める

悪い状態:

- depth bucket が raw 出力と整合しない
- transitive が大きいのに direct が見えず、読む順番に結びつかない

### 2.2 `risk`: 過大/過小評価せず一次優先度付けに使えるか

見る点:

- direct/transitive/file/symbol の広さに対し level が自然か
- 小さい diff を不必要に `high` にしないか
- 広い diff や direct 多め diff を `low` に落としすぎないか
- confidence filter 後の縮退結果でも level が極端に不自然にならないか

良い状態:

- `low`: 小さい・局所的・transitive 薄め
- `medium`: direct あり or transitive が少し伸びるので注意したい
- `high`: direct/transitive/file 数が目立ち、レビューや追加確認を優先したい

悪い状態:

- 「ほぼ局所 diff」なのに `high`
- 「複数 module へ波及」なのに `low`

### 2.3 `affected_modules`: 次に開く場所の絞り込みに使えるか

見る点:

- module 名が path grouping として十分安定か
- `main.rs` のような単ファイル fallback が多すぎて、まとまりを失っていないか
- symbol_count / file_count の順序が「次に見る場所」を決めるのに使えるか
- 広い fan-out ケースで grouping が粗すぎたり細かすぎたりしないか

良い状態:

- 「まず `src` / `alpha` / `tests` を見る」のように着手点が自然に決まる
- 多少粗くても misleading ではない

悪い状態:

- 実際に見るべき場所より `main.rs` 等の fallback が前に来続ける
- path grouping が細かく割れすぎて summary の意味が薄れる

### 2.4 summary 全体の内部整合性

各ケースで最低限確認すること:

- `sum(summary.by_depth[*].symbol_count) == impacted_symbols.len()`
- `summary.risk.impacted_symbols == impacted_symbols.len()`
- `summary.risk.impacted_files == impacted_files.len()` 相当の読みと合う
- `summary.affected_modules[*].symbol_count` 合計が raw 集合を大きく取りこぼしていないか
- normal / YAML / `--per-seed` で summary の意味がぶれないか

### 2.5 実運用での可読性

各ケースで、summary だけ見て次を 10 秒以内に言えるかを確認する。

- これは局所 diff か、広がる diff か
- 先に強く見るべき diff か
- まずどの module / directory を開くか

この 3 問に summary だけで答えづらいケースは、
G2 では改善候補として扱う。

## 3. 評価記録フォーマット（最小）

G2-3 以降でケースを固定するときは、少なくとも次の列を残すと比較しやすい。

| 列 | 意味 |
| --- | --- |
| case_id | ケース識別子 |
| source | fixture / 実 diff の出所 |
| lang | Rust / Go / Java / Ruby / Python / TS / TSX など |
| shape | chain / fan-out / dynamic / local / multi-file |
| expected_triage | 人が summary から受け取りたい解釈 |
| observed_by_depth | 実際の `summary.by_depth` |
| observed_risk | 実際の `summary.risk.level` |
| observed_modules | 実際の `summary.affected_modules` 上位 |
| verdict | keep / overestimate / underestimate / grouping-noisy |
| notes | 次タスクへの改善メモ |

## 4. fixture 候補の棚卸し

### 4.1 まず固定しやすい summary 直結 fixture

既に summary 専用テストとして存在し、G2-3 の基礎セットにしやすい候補。

| 候補 | 参照元 | 主に見たいこと | 優先度 |
| --- | --- | --- | --- |
| Rust 1-hop / 2-hop chain | `tests/cli_impact_by_depth.rs` | `by_depth` と `risk` の最小整合性。direct=1, transitive=1 を素直に出せるか | 高 |
| confidence filter で空になるケース | `tests/cli_impact_by_depth.rs` | filter 後に `by_depth=[]`, `risk=low` へ縮退しても解釈が壊れないか | 高 |
| path grouping fan-out | `tests/cli_impact_affected_modules.rs` | `affected_modules` の ordering と fallback (`main.rs`) の見え方 | 高 |
| Rust hard fixture + inferred filter | `tests/cli_impact_risk.rs` / `tests/fixtures/rust/analyzer_hard_cases_confidence_compare.rs` | `risk` が局所変更に対して過大になりすぎないか | 高 |
| `--per-seed` nesting | `tests/cli_impact_by_depth.rs`, `tests/cli_impact_affected_modules.rs` | normal と per-seed で summary の意味がずれないか | 中 |
| YAML 出力 | `tests/cli_impact_risk.rs`, `tests/cli_impact_affected_modules.rs` | JSON/YAML 間で summary が崩れないか | 中 |

この層は **速い・再現容易・期待値が明確** なので、
G2-3 ではまずここを固定の最小ゲートにするのがよい。

### 4.2 言語横断 fixture 候補（既存 hard fixture 群）

`tests/changed_impacted_golden_baseline.rs` と `tests/fixtures/*` には、
summary の「見え方」を評価するのに向いた multi-language ケースが既にある。

| 候補 | 参照元 | shape | summary 観点 |
| --- | --- | --- | --- |
| TypeScript overload / optional chain | `tests/fixtures/typescript/analyzer_hard_cases_dispatch_overload_optional_chain.ts` | chain | `by_depth` の広がりと `risk` の中庸さ |
| TSX component callback | `tests/fixtures/tsx/analyzer_hard_cases_component_callback_optional_chain.tsx` | local / UI callback | 影響ゼロ〜局所時に summary が騒がしくないか |
| Rust trait dispatch / method value | `tests/fixtures/rust/analyzer_hard_cases_trait_dispatch_method_value_generic.rs` | local / dispatch | Rust analyzer の局所変更に対する `risk` の妥当性 |
| Go interface dispatch / method value | `tests/fixtures/go/analyzer_hard_cases_interface_dispatch_method_value_generic_receiver.go` | chain | direct/transitive と module grouping の見え方 |
| Go extraction FP v4 | `tests/fixtures/go/analyzer_hard_cases_extraction_fp_points_v4.go` | FP reduction | summary が comment/literal 由来ノイズで膨らまないか |
| Java overload / methodref / lambda | `tests/fixtures/java/analyzer_hard_cases_lambda_methodref_overload.java` | chain | `risk` の過大評価有無 |
| Java extraction FP v4 | `tests/fixtures/java/analyzer_hard_cases_extraction_fp_points_v4.java` | FP reduction | false positive 削減ケースで summary も縮むか |
| Ruby dynamic send/public_send | `tests/fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb` | dynamic | dynamic dispatch でも `affected_modules` が破綻しないか |
| Ruby DSL / method_missing v4 | `tests/fixtures/ruby/analyzer_hard_cases_dynamic_dsl_method_missing_chain_v4.rb` | dynamic / fan-out | transitive と grouping の読みやすさ |
| Python getattr/setattr/getattribute | `tests/fixtures/python/analyzer_hard_cases_dynamic_getattr_setattr_getattribute.py` | dynamic | `risk` 過大/過小の境目確認 |
| Python monkeypatch/metaclass/protocol v4 | `tests/fixtures/python/analyzer_hard_cases_dynamic_monkeypatch_metaclass_protocol_v4.py` | dynamic / fan-out | transitive 多めケースで `risk` と `affected_modules` が useful か |

この層は **summary の language bias** を洗うのに向く。
特に Ruby/Python の dynamic fixture は、raw graph が読みにくくなりがちなので、
summary の価値が出るかどうかを見やすい。

## 5. 実 diff 候補の棚卸し

### 5.1 すぐ回せる repo 内 diff fixture

`bench-fixtures/*-heavy.diff` は repo 内に diff ファイルとして置かれており、
追加の履歴復元なしで評価しやすい。

| 候補 | 参照元 | shape | 主に見たいこと |
| --- | --- | --- | --- |
| Go heavy diff | `bench-fixtures/go-heavy.diff` | wide fan-out | direct/transitive が大きい時に `risk` が上がるか |
| Java heavy diff | `bench-fixtures/java-heavy.diff` | wide fan-out | `by_depth` の段数感と `risk` の広がり |
| JS heavy diff | `bench-fixtures/js-heavy.diff` | wide fan-out | path grouping が単一 module に寄りすぎないか |
| TS heavy diff | `bench-fixtures/ts-heavy.diff` | wide fan-out | TS 系で `risk` / `affected_modules` が単調になりすぎないか |
| Python heavy diff | `bench-fixtures/python-heavy.diff` | wide fan-out | transitive 多めケースで `high` 境界を見る |
| Ruby heavy diff | `bench-fixtures/ruby-heavy.diff` | wide fan-out | grouping の粗さ・見やすさ |

この層は **summary のスケール感確認** に向く。
小さな fixture だけだと `risk` が `low/medium` に偏りやすいので、
heavy diff 群は `high` 側の校正候補として有用。

### 5.2 履歴ベースの実 diff 候補（再現は worktree 推奨）

実際に merge 済みの修正 PR は、
「dimpact 開発者が本当に読んだ diff」に対して summary が効くかを見る材料になる。

| 候補 | 参照元 | shape | 主に見たいこと |
| --- | --- | --- | --- |
| PR #393 Python resolver FN 改善 | commit `2531968` | single-language / multi-file | `src/languages/py_spec.rs` 周辺変更で、見るべき module が上位に来るか |
| PR #394 Go/Java comment-literal FP 改善 | commit `4501290` | multi-language / multi-file | analyzer 本体 + baseline test 変更で `risk` が中〜高へ自然に寄るか |
| PR #407 Ruby define_method/alias 改善 | commit `5f7bd86` | localized | 局所的 analyzer 変更を過大に `high` へ振らないか |
| PR #371 Java overload/methodref/lambda 改善 | commit `f2aa622` | localized but dense | 単ファイル大きめ diff で `risk` が妥当か |
| PR #373 Java declaration-line FP 改善 | commit `963467f` | localized + regression notes | analyzer 変更と周辺 artifacts の混在時に module grouping が役立つか |

注意:

- 履歴 diff は current HEAD へそのまま流すと line/context がずれる可能性がある
- 評価時は `git worktree` で対象 commit 前後に切った作業木を作り、
  その tree 上で `git diff --no-ext-diff --unified=0` を採る方が安全

この層は **「実際の開発 diff で summary が triage を速くするか」** を見る用途。
fixture より再現コストは高いが、採用価値の判断には重要。

## 6. G2-3 に持ち上げる第一候補

固定評価セット化の初手としては、次の 6 ケースがバランスよい。

1. `tests/cli_impact_by_depth.rs` の Rust chain
   - 最小の direct/transitive 基準点
2. `tests/cli_impact_by_depth.rs` の confidence filter 縮退ケース
   - summary の下限確認
3. `tests/cli_impact_affected_modules.rs` の fan-out grouping
   - `affected_modules` の見え方基準点
4. Python monkeypatch/metaclass/protocol v4
   - dynamic + transitive 多め
5. Ruby DSL/method_missing v4
   - dynamic + grouping ノイズ確認
6. `bench-fixtures/*-heavy.diff` から 1〜2 本（Go or Python 優先）
   - `high` 校正の基準点

理由:

- summary 3 要素 (`by_depth` / `risk` / `affected_modules`) を全部見られる
- low / medium / high 寄りのケースを混ぜやすい
- Rust 専用ではなく language bias を早めに炙れる
- fixture と実 diff の両輪になる

## 7. 現時点の判断

- **最小ゲート** は既存の summary 専用 fixture から作れる
- **language diversity** は `tests/fixtures/*` に十分な候補がある
- **high 側の校正** には `bench-fixtures/*-heavy.diff` を使うのが早い
- **本当に実運用で useful か** は、履歴ベース実 diff を少数でも混ぜた方がよい

つまり G2-3 では、

- まず fixture で安定した比較面を作り
- 次に heavy diff / 履歴 diff を薄く足して
- `risk` / `affected_modules` の改善議論を raw graph ではなく固定ケース上でやる

という順が自然。
