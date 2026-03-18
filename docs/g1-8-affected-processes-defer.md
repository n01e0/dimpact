# G1-8: `affected_processes` 最小実装は見送る (defer) + TODO

対象: `dimpact impact`

このメモは、G1-8 のタスク

- 「もし軽量導入可能なら `affected_processes` の最小実装を入れる。重ければ design note と TODO を残す」

に対する最終判断を記録するもの。

結論はシンプルで、**G1-8 では `affected_processes` を実装しない**。
その代わり、見送り理由と、後続タスクで着手するなら何を先に揃えるべきかを TODO として固定する。

## 1. 結論

### 判断

**defer（見送り）**

### 理由

G1-7 の検討で、`affected_processes` の軽量版候補は主に次の 3 つに分かれた。

1. path-only grouping
2. entrypoint 候補抽出 + call graph reverse reachability
3. repo 固有 heuristic

このうち:

- 1 は軽いが説明力が弱い
- 3 は早いが汎用性がない
- 2 だけが筋が良いが、G1 で「軽量」と呼ぶにはまだ重い

したがって、**今この時点で無理に最小実装を入れるより、defer を明文化した方が良い**。

## 2. なぜ今は入れないのか

### 2.1 `process` は graph の外部概念

`by_depth` や `risk` は、既にある次の情報から自然に作れる。

- symbol
- file
- edge
- 到達深さ

しかし `affected_processes` はそうではない。
少なくとも「どの symbol を entrypoint と呼ぶか」という追加規則が必要で、
これは現在の `ImpactOutput.summary` が直接表現している世界の外にある。

### 2.2 言語差が大きい

現時点で見えている entrypoint 候補は、言語ごとにかなり違う。

- Rust: `src/bin/*`, `fn main`
- Go: `package main`, `func main`
- Java: `public static void main(String[] args)`
- Python: `__main__` ガード、`def main()`, console script
- JS/TS: `bin/*`, `src/bin/*`, package.json scripts, framework entrypoint など

Rust / Go / Java だけならまだしも、Python / JS / TS を含めると、
最小版 heuristic でも false positive / false negative をかなり出しやすい。

### 2.3 間違った process 名は害が大きい

`risk` が少し粗くても、「件数の目安」としてまだ読み替えられる。
`by_depth` も bucket 集計なら人間が解釈し直せる。

でも `affected_processes` は違う。
誤った process 名が出ると、人間は「どの実行経路が壊れうるか」を誤解しやすい。

つまりこの feature は、**未実装であることより、雑に実装されることの方が危ない**。

## 3. 今回あえて採らない案

### 案 A: path-only で `src/bin/*` や `main.*` を process 扱いする

却下。

理由:
- 単一 CLI repo では情報量がほとんど増えない
- 多言語 repoで path 規約が揃わない
- call graph を使わないので、dimpact の強みを活かしていない

### 案 B: repo 固有 heuristic を許して、この repo専用に出す

却下。

理由:
- 汎用ツールとしての dimpact に合わない
- fixture 最適化で「動いて見えるだけ」になりやすい
- 他 repo に持っていくと挙動の意味が壊れる

## 4. 将来やるなら採るべき方式

将来 `affected_processes` をやるなら、採るべき方向はこれ。

### entrypoint 候補抽出 + reverse reachability

手順はこう。

1. entrypoint 候補 symbol を言語別 rule で抽出する
2. impacted symbol から callers 側に逆到達を取る
3. 到達した entrypoint ごとに impact を集計する
4. `summary.affected_processes` として出す

この方式なら、`affected_processes` は単なる path grouping ではなく、
**実行起点ベースの summary** として説明できる。

## 5. G1-8 時点の TODO

実装を見送る代わりに、次の TODO を後続タスクへ残す。

### TODO 1: entrypoint 抽出ルールを言語別に定義する

最低限、以下を切り分ける。

- Rust
  - `src/bin/*`
  - `fn main`
- Go
  - `package main`
  - `func main`
- Java
  - `public static void main(String[] args)`
- Python
  - `if __name__ == "__main__"`
  - `def main()` 単独では弱いので補助条件が必要
- JS/TS
  - `bin/*` / `src/bin/*` など path ヒントありの場合だけ候補化するか検討

### TODO 2: entrypoint を IR に持つか決める

選択肢は 2 つ。

- IR / symbol metadata に `entrypoint` 的な属性を持たせる
- summary layer 側の後段 heuristic として完結させる

G1 時点ではまだ決め打ちしないが、実装を始める前にはここを決める必要がある。

### TODO 3: default-on にしない

`affected_processes` は、最初から通常の `impact` 出力で常時出すより、
少なくとも初期段階では以下のどちらかが無難。

- experimental field として扱う
- 明示 opt-in にする

理由は、heuristic の粗さを十分にテストで抑え込む前に default-on にすると、
誤誘導コストが高いから。

### TODO 4: fixture を process 妥当性ベースで作る

fixture 側でも、単なる `entry()` やテスト用の top-level callable を安易に process と見なさない。

process 系 fixture は、少なくとも次のどれかを満たすべき。

- 明確な `main`
- `src/bin/*`
- 実行起点として説明できる bootstrap 関数

### TODO 5: 最初の schema は集計だけに留める

将来の最小 schema は、最初はこれくらいで十分。

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

最初から symbol 一覧や path 一覧を大量に抱えず、
`by_depth` / `risk` と同じく **小さい summary 集計** として始める方が良い。

## 6. G1-8 の成果として何が残るか

G1-8 の成果は「未実装」ではなく、次の 3 点を明確化したことにある。

1. 今は入れない方が安全だという判断
2. 将来やるなら取るべき方式
3. 実装前に必要な TODO

つまり、G1-8 は **無理に雑な minimal implementation を入れず、設計負債を増やさない** ための stop point になっている。

## 7. 一言まとめ

- `affected_processes` は有用だが、今の dimpact にはまだ重い
- path-only は弱く、repo 固有 heuristic は汎用性がない
- 将来やるなら **entrypoint 候補抽出 + reverse reachability** 一択
- なので G1-8 では **実装見送り + design note + TODO** が正解
