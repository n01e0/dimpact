# impact 出力構造の棚卸しと summary layer 拡張ポイント (G1-1)

対象: `dimpact impact`

このメモは **現状の出力 shape を壊さずに summary layer を足すには、どこを触るべきか** を整理するための棚卸し。
G1-1 では実装は入れず、現状把握と拡張ポイントの特定に限定する。

## 1. 現在の出力 shape

### 1.1 通常の JSON / YAML

ベースの出力本体は `ImpactOutput` で、現在のトップレベル要素は 5 つ。

- `changed_symbols`
- `impacted_symbols`
- `impacted_files`
- `edges`
- `impacted_by_file`

定義元:
- `src/impact.rs:40` 付近の `ImpactOutput`
- JSON/YAML 出力ラッパ: `src/bin/dimpact.rs:112` 付近の `ImpactOutputRendered`

JSON shape の概略:

```json
{
  "changed_symbols": [Symbol],
  "impacted_symbols": [Symbol],
  "impacted_files": ["src/..."],
  "edges": [Reference],
  "impacted_by_file": {
    "src/foo.rs": [Symbol]
  }
}
```

### 1.2 confidence filter 付き JSON / YAML

`--min-confidence` / `--exclude-dynamic-fallback` / `--op-profile` を使うと、
通常の `ImpactOutput` に加えてトップレベルへ `confidence_filter` が追加される。

```json
{
  "changed_symbols": [Symbol],
  "impacted_symbols": [Symbol],
  "impacted_files": ["src/..."],
  "edges": [Reference],
  "impacted_by_file": {...},
  "confidence_filter": {
    "min_confidence": "inferred",
    "exclude_dynamic_fallback": false,
    "input_edge_count": 156,
    "kept_edge_count": 120
  }
}
```

定義元:
- `src/bin/dimpact.rs:94` 付近の `ConfidenceFilterSummary`
- `src/bin/dimpact.rs:202` 以降の `print_impact_output`

重要なのは、`confidence_filter` は **`ImpactOutput` 本体の field ではなく、CLI 直列化時のラッパ field** という点。

### 1.3 `--per-seed` の JSON

`--per-seed` はトップレベル shape が別物で、`Vec<PerSeedOutput>` をそのまま JSON 化している。

```json
[
  {
    "changed_symbol": Symbol,
    "impacts": [
      {
        "direction": "callers",
        "output": ImpactOutput,
        "confidence_filter": {
          "min_confidence": "inferred",
          "exclude_dynamic_fallback": false,
          "input_edge_count": 10,
          "kept_edge_count": 8
        }
      }
    ]
  }
]
```

定義元:
- `src/bin/dimpact.rs:909` 付近の `PerSeedImpact`
- `src/bin/dimpact.rs:917` 付近の `PerSeedOutput`

つまり **通常出力と `--per-seed` 出力では、summary を差し込む場所が揃っていない**。
ここは最初に押さえるべき拡張ポイント。

### 1.4 DOT / HTML

DOT / HTML は JSON/YAML のトップレベル wrapper を通らず、`ImpactOutput` を直接レンダしている。

- DOT: `src/bin/dimpact.rs:231`
- HTML: `src/bin/dimpact.rs:232`
- HTML renderer: `src/render.rs:312` 以降

現状 HTML が見ているのは主に以下。

- `changed_symbols`
- `impacted_symbols`
- `impacted_files`
- `edges`

したがって summary field を追加しても、**最初は HTML/DOT 側で無視する** 方針が取りやすい。

## 2. ネストされた要素の shape

### 2.1 `Symbol`

`Symbol` の field は以下。

- `id`
- `name`
- `kind`
- `file`
- `range`
- `language`

定義元: `src/ir.rs:22` 以降

```json
{
  "id": "rust:src/impact.rs:fn:path_is_ignored:51",
  "name": "path_is_ignored",
  "kind": "function",
  "file": "src/impact.rs",
  "range": {
    "start_line": 51,
    "end_line": 75
  },
  "language": "rust"
}
```

### 2.2 `Reference`

`edges` の各要素は `Reference`。

- `from`
- `to`
- `kind`
- `file`
- `line`
- `certainty`
- `confidence`

定義元: `src/ir/reference.rs:27` 以降

注意点:
- 実体 field は `certainty`
- 互換性のため serialize 時に `confidence` も同じ値で出している
- deserialize 時は `certainty` / `confidence` のどちらでも受ける

つまり edge 系 summary を足すときは、**`certainty` を正として扱い、`confidence` は既存互換 alias とみなす** のが安全。

## 3. どこで `ImpactOutput` が組み立てられているか

### 3.1 Tree-Sitter / cache 経由の主経路

主経路は `compute_impact()`。

- `src/impact.rs:542` 以降

ここでやっていること:
- 参照グラフから BFS
- `impacted_symbols` の sort / dedup
- `impacted_files` 生成
- `with_edges` 時の edge 抽出
- `impacted_by_file` 生成

つまり、**現在の summary 未満の「基本集計」はここに集まっている**。

ただし制約もある。

- traversal 中の深さは `seen` に落としており、最終出力に depth map を残していない
- `by_depth` を後付けしたい場合、現状の `ImpactOutput` だけでは材料不足
- とくに `--with-edges=false` 時は、出力だけから depth を復元できない

G1-3 / G1-4 で `by_depth` を入れるなら、**`compute_impact()` か、その直後の richer な内部表現** に depth 情報を残す必要がある。

### 3.2 CLI の後段ラップ

CLI 側では以下の後処理がある。

- `apply_confidence_filter()` (`src/bin/dimpact.rs:152`)
- `print_impact_output()` (`src/bin/dimpact.rs:202`)

特に `apply_confidence_filter()` は、filtered edge から **もう一度 `compute_impact()` を回して** 出力を再構成している。

これは summary 設計上かなり重要で、
**summary は confidence filter 適用前ではなく適用後の出力にぶら下げないと、件数や depth がずれる**。

### 3.3 LSP 経路の重複組み立て

LSP 側には `ImpactOutput` の組み立てが複数箇所ある。

- `src/engine/lsp.rs:1256` 付近の手組み return
- `src/engine/lsp.rs:1624` 付近の空出力 return
- `src/engine/lsp.rs:1774` 付近の BFS 出力 return
- `src/engine/lsp.rs:2263` 付近の `build_impact_output()`

つまり、summary を `compute_impact()` のみに足しても **LSP の一部経路で summary が欠ける** 可能性がある。

ここは今回の棚卸しで一番大きい発見で、
**summary layer を安定して出すには `ImpactOutput` の最終整形処理を共通化する必要がある**。

## 4. 既存 consumer から見た互換性

コードベース内の consumer をざっと見ると、以下の読み方が多い。

### 4.1 テスト

多くの CLI テストは以下のように個別 key を読むだけ。

- `v["changed_symbols"]`
- `v["impacted_symbols"]`
- `v["confidence_filter"]`

例:
- `tests/cli_impact.rs`
- `tests/cli_impact_opts.rs`
- `tests/cli_go.rs`
- `tests/cli_java.rs`
- `tests/cli_python.rs`

このため、**トップレベルへの optional 追加 field は比較的入れやすい**。

### 4.2 スクリプト

既存スクリプトも多くは `changed_symbols` / `impacted_symbols` を直接参照している。

例:
- `scripts/bench-impact-engines.sh`
- `scripts/compare-impact-vs-lsp-oracle.sh`
- `scripts/verify-precision-regression.sh`

したがって、`summary` を **追加のみ** で入れる限り、既存 consumer への破壊的影響は小さい。

## 5. summary layer の拡張ポイント

### 拡張ポイント A: `ImpactOutput` 自体へ `summary` を追加する

候補:

```rust
pub struct ImpactOutput {
    pub changed_symbols: Vec<Symbol>,
    pub impacted_symbols: Vec<Symbol>,
    pub impacted_files: Vec<String>,
    pub edges: Vec<Reference>,
    pub impacted_by_file: HashMap<String, Vec<Symbol>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ImpactSummary>,
}
```

利点:
- 通常 JSON/YAML と `--per-seed` の両方で同じ位置に出せる
- lib API と renderer が同じデータモデルを共有できる
- 将来 HTML 側で summary を使いたくなったときも載せ替え不要

注意点:
- `ImpactOutput` の手組み箇所を全部追う必要がある
- LSP の重複構築を放置すると抜け漏れが出やすい

### 拡張ポイント B: `ImpactOutputRendered` の wrapper だけに `summary` を足す

候補:
- `src/bin/dimpact.rs` の `ImpactOutputRendered` に `summary` field を追加

利点:
- 通常 JSON/YAML だけなら最小変更
- DOT/HTML や lib API を触らずに済む

欠点:
- `--per-seed` 出力にそのまま乗らない
- engine 層やライブラリ API から summary を参照できない
- 「impact 出力の一部」ではなく「CLI 表示用付加情報」になってしまう

結論:
- **confidence_filter と同じ性格の薄いメタ情報** なら成立
- しかし `by_depth` / `risk` / `affected_*` は継続利用したいので、置き場所としては弱い

### 拡張ポイント C: `ImpactOutput` の最終整形 helper を新設する

推奨。

たとえば以下の責務を 1 か所へ寄せる。

- sort / dedup
- `impacted_files` 生成
- `impacted_by_file` 生成
- optional `summary` 生成

イメージ:

```rust
fn finalize_impact_output(
    changed_symbols: Vec<Symbol>,
    impacted_symbols: Vec<Symbol>,
    edges: Vec<Reference>,
    opts: &ImpactOptions,
    summary_opts: &ImpactSummaryOptions,
) -> ImpactOutput
```

これを
- `compute_impact()`
- `apply_confidence_filter()` 後
- `engine/lsp.rs` の各手組み経路

から共通利用すれば、summary の付け忘れを減らせる。

## 6. feature ごとの入り口

### 6.1 `by_depth`

必要なもの:
- changed seed から各 impacted symbol までの最短 depth
- depth ごとの件数 / symbol / file 集計

現状の問題:
- `compute_impact()` は `seen` は持つが、最終 depth map を保持しない
- `ImpactOutput` だけでは `with_edges=false` 時に復元不能

結論:
- `by_depth` は **探索中に depth を保持する設計変更** が必要
- 置き場所は `compute_impact()` 内、または `TraversalResult` のような中間構造

### 6.2 `risk`

初期版の `risk` は以下から作りやすい。

- direct hits（depth=1）
- transitive hits（depth>=2）
- impacted file 数
- impacted symbol 数

結論:
- `risk` は単独で先に入れるより、**`by_depth` と同じ土台に乗せる** 方が素直
- G1-5 / G1-6 では `by_depth` の上に計算するのが自然

### 6.3 `affected_processes`

軽量版なら以下の post-process が候補。

- impacted symbol / file の path prefix から entrypoint 候補へ寄せる
- call graph の direct/transitive 数を補助指標にする

結論:
- 最小版なら traversal 本体を大きく変えずに、`impacted_symbols` / `impacted_files` から後段集計で足せる
- ただし repo 依存 heuristic が強いので、最初は summary layer の optional field として隔離するのがよい

### 6.4 `affected_modules`

軽量版なら以下が候補。

- path prefix (`src/foo/...`) ベース
- import namespace / module path ベース
- call graph reachability の件数を補助指標にする

結論:
- `affected_modules` も **後段集計型** に向いている
- `by_depth` ほど traversal 本体への侵襲は要らない

## 7. G1-1 時点の推奨方針

G1-1 の結論としては、次の順がいちばん事故が少ない。

1. `ImpactOutput` に optional `summary` を置く
2. `ImpactOutput` の最終整形処理を helper に寄せる
3. summary は **confidence filter 適用後** の出力に対して生成する
4. `--per-seed` では各 `impacts[].output.summary` に入れる
5. HTML/DOT は初期段階では summary を無視する

## 8. 次タスクへの引き継ぎメモ

### G1-2 でやると良いこと

- GitNexus 側 summary 候補を
  - traversal 本体が必要なもの
  - 後段集計で足せるもの
  に分ける
- 最初の実装順を
  - `by_depth`
  - `risk`
  - `affected_modules`
  - `affected_processes`
  の順で評価する

### G1-3 で固めるべき schema

最低限、以下のような shape を検討するとよい。

```json
{
  "summary": {
    "by_depth": [
      {
        "depth": 1,
        "symbol_count": 3,
        "file_count": 2
      }
    ],
    "risk": {
      "level": "medium",
      "direct_hits": 3,
      "transitive_hits": 7
    }
  }
}
```

この shape なら、`risk` や `affected_*` をあとから足しても拡張しやすい。

## 9. 一言まとめ

現状の `impact` 出力は `ImpactOutput` を核にしているが、
**CLI wrapper (`confidence_filter`) と `--per-seed` の別 shape、さらに LSP 側の重複組み立て** がある。

なので summary layer の本命の拡張ポイントは:

- `ImpactOutput` へ optional `summary` を追加すること
- `ImpactOutput` の最終整形を共通 helper に寄せること
- `by_depth` のために traversal 中の depth 情報を保持すること

この 3 点。
