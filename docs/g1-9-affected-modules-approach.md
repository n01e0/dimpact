# `affected_modules` の軽量版アプローチ判断 (G1-9)

対象: `dimpact impact`

このメモは、summary layer に `affected_modules` を追加する場合に、
**path / import / call ベースのどれを最初の軽量版として採るべきか** を決めるための判断メモ。

結論から言うと、**G1 の最小導入は path ベースで進める** のが一番自然。
import ベースと call/community ベースは価値はあるが、初手としては重い。

## 1. まず結論

### 採る方針

**path prefix ベースの lightweight grouping を最初の `affected_modules` とする。**

### 見送るもの

初手では以下を入れない。

- import graph ベースの grouping
- call graph community ベースの grouping
- 言語ごとの厳密 module 解釈

### 理由

- `impacted_symbols` / `impacted_files` から後段集計だけで作れる
- `by_depth` / `risk` のように traversal 本体を増築しなくてよい
- repo や言語が違っても、最低限の説明力を保ちやすい
- 間違った grouping が出ても、`affected_processes` ほど致命的に誤誘導しにくい

## 2. 候補アプローチ

### A. path ベース

例:

- `src/engine/lsp.rs` → `src/engine`
- `src/bin/dimpact.rs` → `src/bin`
- `tests/cli_impact_by_depth.rs` → `tests`

#### 利点
- 実装が軽い
- 既にある `impacted_files` / `impacted_symbols` から作れる
- confidence filter 後の最終出力にも自然に載せられる
- JSON/YAML / per-seed への反映が簡単

#### 欠点
- path が module 構造をうまく表さない repo では粗い
- 単一ディレクトリに多様な責務が混在していると grouping が雑になる
- 言語の namespace 概念までは拾えない

#### 判断
- **最初の lightweight 版として採用**

### B. import / namespace ベース

例:

- Rust の `crate::engine::lsp`
- Python の `pkg.subpkg.module`
- Java の package 名
- TS/JS の import path

#### 利点
- 人間がコード上で認識している module 概念に近い
- path よりもリネームやレイアウト差に強い場合がある
- 言語によってはかなり自然な grouping になる

#### 欠点
- 言語差が大きい
- 現在の dimpact 出力モデルでは import namespace が summary 用に揃っていない
- analyzer ごとのデータ整備が必要になりやすい
- path ベースより一段重いのに、最初の説明力差がそこまで大きくない場合がある

#### 判断
- **第二段階の改善候補**
- 初手では採らない

### C. call graph / community ベース

例:

- 相互参照が密な symbol 群を 1 community とみなす
- 到達性や edge 密度から cluster を切る

#### 利点
- GitNexus 的な「実際に一緒に揺れる塊」に一番近い可能性がある
- path や namespace に縛られない grouping ができる

#### 欠点
- 最も重い
- cluster 命名が難しい
- 同じ repoでも diff ごとに cluster の見え方がぶれやすい
- テストの安定性が path grouping より低い
- 最初の summary としては説明コストが高い

#### 判断
- **初期導入では採らない**
- これは軽量版ではなく、後続の高度化テーマ

## 3. なぜ path ベースが最初に向くのか

`affected_modules` は `affected_processes` と違って、
「実行起点」ではなく「まとまり」を見せる summary なので、多少粗い grouping でも成立しやすい。

その点で path ベースは次のバランスが良い。

1. **後段集計だけで作れる**
   - `impacted_files`
   - `impacted_symbols`
   - `by_depth` / `risk` の計数
   だけで十分出発できる

2. **既存出力との整合が取りやすい**
   - summary の中に 1 block 足せばよい
   - engine / traversal / confidence filter の大改造がいらない

3. **人間が読みやすい**
   - `src/engine`
   - `src/bin`
   - `tests`
   のような path grouping は、そのままレビュー導線になる

## 4. G1 時点で採る具体ルール

最小版の grouping ルールは、まず次の程度に留めるのがよい。

### 基本ルール

- file path の親ディレクトリを module 名の基本単位とする
- ただし root 直下の file は、その file 名を含む 1 段深い表示名に寄せてもよい
- 同じ module に属する impacted symbol を数え上げる
- file 数も別で集計する

### 例

- `src/engine/lsp.rs` → `src/engine`
- `src/engine/mod.rs` → `src/engine`
- `src/bin/dimpact.rs` → `src/bin`
- `tests/cli_impact_risk.rs` → `tests`
- `main.rs` → `.` または `main.rs` 相当の root group

ここで大事なのは、**最初から賢くしすぎない** こと。
`src/foo/bar/baz.rs` をどこまで畳むか、`mod.rs` / `lib.rs` をどう扱うかは、最初は単純でいい。

## 5. 将来の拡張順

`affected_modules` は次の順で育てるのが自然。

### Phase 1: path grouping

最小 schema 例:

```json
{
  "summary": {
    "affected_modules": [
      {
        "module": "src/engine",
        "symbol_count": 4,
        "file_count": 2
      },
      {
        "module": "src/bin",
        "symbol_count": 1,
        "file_count": 1
      }
    ]
  }
}
```

### Phase 2: import / namespace の補助

必要なら path 表示名に対して、言語ごとの namespace を補助的に載せる。

例:

- Rust: `crate::engine`
- Python: `pkg.engine`
- Java: `demo.engine`

ただし最初から必須にはしない。

### Phase 3: call/community の高度化

call graph の密度や到達性を使った cluster 化は、
path grouping の次の世代として考える。

ただしここまで行くと、もはや lightweight ではない。

## 6. G1-10 に引き継ぐ実装メモ

G1-10 で最小実装を入れるなら、以下で十分。

1. `ImpactSummary` に optional `affected_modules` を追加
2. `impacted_symbols` / `impacted_files` から path prefix で group を作る
3. 各 group について最低限これを出す
   - `module`
   - `symbol_count`
   - `file_count`
4. sort は `symbol_count desc`、同点なら `module asc`
5. confidence filter 後・per-seed 出力でも同じ summary builder を通す

## 7. 今回あえてやらないこと

G1-9 では次は決めない。

- import 解決の cross-language 正規化
- graph clustering アルゴリズム
- repo 固有の grouping rule
- module ごとの risk score 再計算

これらは G1-10 以降の話。

## 8. 一言まとめ

- `affected_modules` は `affected_processes` より軽量導入しやすい
- path / import / call の3案では、**最初は path ベースが最適**
- import は第二段階、call/community は後続の高度化
- G1-10 は path grouping の最小実装で進めるのが妥当
