# `affected_processes` の軽量版アプローチ判断 (G1-7)

対象: `dimpact impact`

このメモは、summary layer に `affected_processes` を足す場合の軽量版アプローチを比較し、
**G1 時点で実装に進むか、後回しにするか** を決めるための判断メモ。

結論を先に書くと、**G1 では `affected_processes` の本実装は後回しにする**。
ただし完全に捨てるのではなく、
**将来入れるなら「entrypoint 候補を明示抽出して、その到達集合を call graph で逆引きする」方式** を採るのが一番まし、というところまで方針を固定する。

## 1. なぜ今判断が必要か

`affected_processes` は GitNexus 側では人間にとってかなり分かりやすい summary になりうる。
一方で dimpact の現在の出力モデルは、基本的に次の 3 つしか持っていない。

- symbol
- file
- call edge

`by_depth` や `risk` はこの 3 つから自然に作れるが、`process` はそうではない。
`process` を言うには、少なくとも「何を entrypoint と呼ぶか」の定義が追加で必要になる。

## 2. 現状の repo / fixture から見えること

ざっと確認すると、この repo と fixture 群には次のような entrypoint 候補が混在している。

### 2.1 実リポジトリ本体

- 実 binary は `src/bin/dimpact.rs` が中心
- Rust 側は `src/bin/*` という分かりやすい CLI entrypoint がある

### 2.2 fixture / test world

fixture / test には複数言語の main-like 形がある。

- Rust: `fn main()`
- Go: `func main()`
- Python: `def main()` や `if __name__ == "__main__"`
- Java: `public static void main(String[] args)`
- 一部 fixture では `entry()` のような main ではない top-level callable も使う

つまり、**言語ごとに entrypoint 慣習がかなり違う**。
しかも fixture の `entry()` は「process」とは限らず、単なるテスト用の末端 callable でもある。

この時点で、`affected_processes` は `affected_modules` よりかなり heuristic 依存が強いと分かる。

## 3. 候補アプローチ

### A. path-only grouping

例:

- `src/bin/*` を 1 process
- `cmd/*` を 1 process
- `apps/*` を 1 process
- `main.rs` / `main.go` / `Main.java` を process 候補

#### 利点
- 実装は軽い
- call graph を追加で複雑化しなくてよい
- 一部の CLI リポジトリではそこそこ当たる

#### 欠点
- path が process 名を表さない repo で弱い
- 単一バイナリ repoだと情報量がほぼ 0 になる
- Python / JS / TS の実行起点は path 規約だけでは外しやすい
- `impact` の core value である call graph をほとんど使わない

#### 判断
- **軽いが、精度よりノイズが先に立つ**
- dimpact の summary としては弱い

### B. entrypoint 候補抽出 + call graph 到達判定

手順イメージ:

1. repo 内から entrypoint 候補 symbol を抽出する
2. impacted symbol から caller 方向へたどり、どの entrypoint に到達するかを見る
3. 到達した entrypoint 群を `affected_processes` として出す

entrypoint 候補の最小ルール案:

- Rust: `src/bin/*.rs` 内の top-level callable、または `fn main`
- Go: `package main` 内の `func main`
- Python: `def main` は候補だが、`__main__` ガード検出なしでは弱い
- Java: `public static void main(String[] args)`
- JS/TS: 最初から一般化しない。`src/bin/*` や `bin/*` など path ヒントがある場合のみ候補

#### 利点
- `affected_processes` らしい意味になる
- symbol / edge をちゃんと活かせる
- 単なる path grouping より説明力が高い

#### 欠点
- entrypoint 抽出ルールが言語ごとに必要
- 現在の IR / output に「この symbol は entrypoint」という明示 bit がない
- `impact` の direction / confidence filter / LSP 経路とも整合を取る必要がある
- `entry()` のような fixture 上の convenience function を process と誤認しやすい

#### 判断
- **将来やるならこの方式**
- ただし G1 時点では「軽量版」と呼ぶにはまだ重い

### C. repo 固有 heuristic を許して先に入れる

例:

- この repo なら `src/bin/dimpact.rs` だけ process とみなす
- fixture では `main` / `entry` をまとめて entrypoint 扱いする

#### 利点
- 今回の repoだけ見れば早い
- demo としては動かしやすい

#### 欠点
- 汎用ツールとしての dimpact に合わない
- 他 repo に持って行くと意味が壊れる
- fixture に最適化した見かけ倒しになりやすい

#### 判断
- **採らない**

## 4. G1 時点の最終判断

### 結論

**G1-8 では `affected_processes` を本実装しない方がよい。**

理由は 3 つ。

1. **entrypoint 定義がまだ summary layer の外にある**
   - 今の `ImpactOutput.summary` は `by_depth` / `risk` のような graph 集計には向く
   - でも `process` は graph の外部概念で、別の heuristic 層が必要

2. **多言語差が大きい**
   - Rust / Go / Java は比較的やりやすい
   - Python / JS / TS は entrypoint の慣習が広く、誤判定コストが高い

3. **誤った `process` 名は、未表示より害が大きい**
   - `risk` や `by_depth` が多少粗くても、人間は数字を読み替えられる
   - でも `affected_processes` が外れると、人間は「どの実行経路に影響したか」を誤解しやすい

## 5. ただし、将来やるなら方針は B に固定する

後回しにするとはいえ、次に掘るべき方向は決めておく。

### 採るべき基本戦略

**entrypoint 候補抽出 + reverse reachability**

つまり:

- まず process 名の元になる entrypoint symbol を集める
- impacted symbol から caller 側へ reverse reachability を取る
- 到達した entrypoint ごとに impact をまとめる

これなら、`affected_processes` は `affected_modules` と違って「path grouping の別名」ではなく、
**実行起点ベースの summary** として説明できる。

## 6. 将来の最小 schema 案

もし後で実装するなら、最初の schema はこれくらいで十分。

```json
{
  "summary": {
    "affected_processes": [
      {
        "process": "dimpact",
        "entry_symbol": "rust:src/bin/dimpact.rs:fn:main:471",
        "direct_hits": 1,
        "transitive_hits": 3,
        "impacted_files": 2
      }
    ]
  }
}
```

ポイント:

- `process` は表示名
- `entry_symbol` は検証用の実体 ID
- direct / transitive は `by_depth` / `risk` の土台に寄せて使う
- まずは symbol list 全出しではなく集計だけに留める

## 7. G1-8 に渡す TODO

G1-8 で実装を始める代わりに、次の TODO を残すのがよい。

1. entrypoint 候補抽出ルールを言語別に棚卸しする
   - Rust: `src/bin/*`, `fn main`
   - Go: `package main`, `func main`
   - Java: `public static void main`
   - Python: `__main__` ガード / console script は別途検討
   - JS/TS: path 規約なしでは無理に入れない

2. entrypoint を IR に持たせるか、後段 heuristic に留めるか決める

3. `affected_processes` は最初から default-on にせず、必要なら experimental 扱いにする

4. fixture は「process と言って良いケース」だけで作る
   - `entry()` のような補助関数を安易に process 代表にしない

## 8. 一言でいうと

- path-only は軽いが弱い
- repo 固有 heuristic は早いが汎用性がない
- **将来やるなら entrypoint 候補 + call graph reverse reachability**
- でも **G1 時点ではまだ重いので、`affected_processes` は後回し**

この判断で進めるのが、dimpact では一番まっとう。