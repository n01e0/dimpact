# G2-8: `affected_processes` experimental 化の要否判断

このメモは、G2 で `risk` / `affected_modules` の評価と軽量改善を一周したあとで、
次段で `affected_processes` を **experimental として着手すべきか** を判断するための決定記録。

結論を先に書く。

## 結論

**現時点では `affected_processes` を experimental 化しない。**

つまり:

- G2 の次段で default-on 実装には進まない
- experimental field / opt-in flag としても、まだ着手優先度は上げない
- 代わりに、着手条件と最小設計をこのメモに固定して defer を継続する

判断としては **defer 継続** だが、G1 より一段具体的に
「何が揃ったら experimental 化してよいか」を定義する。

## 1. G2 を通して分かったこと

G2 では、summary layer のうち

- `by_depth`
- `risk`
- `affected_modules`

について、評価観点・固定ケース・校正・軽量改善まで進めた。

この過程で確認できたのは次の 2 点。

### 1.1 `affected_modules` は lightweight path summary としてまだ伸ばせる

G2-6 / G2-7 でやったのは、path grouping を壊さずに
entry-like file を親 directory / `(root)` へ正規化する、という軽い改善だった。

これは:

- 実装理由を説明しやすい
- CLI テストで固めやすい
- fixed eval set でも見え方の改善を確認しやすい

という意味で、summary layer の成熟に対して素直に効いた。

### 1.2 `affected_processes` はまだ同じノリで扱えない

一方で `affected_processes` は、G2 を一周してもなお
`affected_modules` と同じ軽さでは扱えない。

理由は単純で、`process` だけは path grouping の延長ではなく、
**entrypoint という repo/language 依存の概念** を前提にするから。

`risk` や `affected_modules` は、多少粗くても
「読む順の目安」としてまだ人間が補正できる。

でも `affected_processes` は違う。
間違った process 名を出すと、
利用者は「どの実行経路・バイナリ・起動点に影響したか」を誤解しやすい。

この性質は G1 時点の判断と変わっていない。

## 2. なぜ今 experimental に上げないのか

### 2.1 fixed eval set が process 妥当性をまだ支えられない

G2-3 の固定評価セットは、summary のうち

- direct / transitive の広がり
- risk 校正
- module grouping の見え方

を確認するには十分だった。

ただし `affected_processes` を評価するには、ケースの軸が足りない。

今のセットには次の不足がある。

- 明確な entrypoint を持つ fixture が少ない
- 「process と呼んでよい symbol」を ground truth として固定していない
- multi-binary / multi-entrypoint repo 形のケースがない
- Python / JS / TS の entrypoint 妥当性を比較できる fixture がない

この状態で experimental を入れると、
評価が「何となくそれっぽい」に寄りやすい。

### 2.2 entrypoint 候補抽出の言語差が大きい

G1-7 / G1-8 でも見えていたが、次の差は今も重い。

- Rust: `src/bin/*`, `fn main` は比較的やりやすい
- Go: `package main`, `func main` は比較的やりやすい
- Java: `public static void main(String[] args)` は比較的やりやすい
- Python: `if __name__ == "__main__"`, console script, framework bootstrap が混ざる
- JS/TS: `bin/*`, package.json scripts, framework entry, dev server entry などが混ざる

つまり、Rust/Go/Java だけ先にやるならまだしも、
summary field として一般化した瞬間に言語間の期待値が揃わない。

### 2.3 opt-in experimental でも誤誘導コストが高い

「default-on ではなく experimental だから雑でもよい」は成り立たない。

理由:

- summary は読む順を案内するためのものなので、opt-in でも誤誘導すると痛い
- 一度 field 名を出すと、利用者は「そこそこ信じてよい情報」と受け取りやすい
- 後で schema や意味を変えると、experimental でも移行コストが出る

つまり experimental 化の最低条件は、
**field 名を出したときに、外しても害が限定的であること**。
今はそこまで行っていない。

## 3. では、今どう位置づけるか

G2-8 時点の位置づけは次の通り。

### 判断ラベル

**not now / defer**

### 意味

- アイデア自体は捨てない
- ただし次タスクとして実装を始める段階ではない
- 実装前に、experimental 化の前提を先に満たす必要がある

これは「不要」ではなく、
**まだ出すには説明可能性が足りない** という判断。

## 4. experimental 化の着手条件

将来 `affected_processes` を experimental 化してよいのは、
最低でも次が揃ったとき。

### 条件 1: process 妥当性用 fixture セットがある

少なくとも次の種類が必要。

- Rust multi-bin (`src/bin/*`) ケース
- Go `package main` ケース
- Java `main` ケース
- Python `__main__` ガードありケース
- JS/TS は path ヒントありケースだけに限定するか、先に out-of-scope と明記する

重要なのは、単に caller chain があることではなく、
**どの symbol を process entry と見なすかの正解を fixture 側で言えること**。

### 条件 2: entrypoint 候補抽出 rule を first-class に文章化する

最低限、言語別に

- 何を候補にするか
- 何を候補にしないか
- false positive をどこで切るか

を design note で固定してから着手する必要がある。

### 条件 3: 初期スコープを language-limited にできる

最初から全言語対応にしない。

たとえば experimental v0 は:

- Rust / Go / Java のみ
- Python は `__main__` 明示ケースのみ
- JS/TS は未対応

のように、**対応範囲を狭く宣言できること** が必要。

### 条件 4: opt-in field / flag として切り出す

最初は通常 summary に常時混ぜない方がよい。

候補:

- `--experimental-affected-processes`
- `--summary-profile experimental-processes`
- あるいは top-level `summary.experimental.affected_processes`

少なくとも初回は、通常の `impact` 利用者が黙って受け取る形にはしない。

## 5. 将来着手するなら採る設計

G1 からの結論は変えない。
将来やるなら、設計はこれで行くべき。

### 5.1 基本方式

**entrypoint 候補抽出 + reverse reachability**

流れ:

1. repo から entrypoint 候補 symbol を抽出する
2. impacted symbol から caller 側へ逆到達を取る
3. 到達した entrypoint ごとに集計する
4. process label と entry symbol をまとめて出す

### 5.2 やらない方式

次は採らない。

- path-only grouping を process と呼ぶ
- repo 固有 heuristic で process 名を決め打ちする
- fixture の `entry()` など便利関数をそのまま process とみなす

これらは experimental でも説明力よりノイズが勝ちやすい。

## 6. 最小 schema の更新案

将来 experimental を始めるときの最小 schema は、
G1-8 の案をほぼ維持してよい。

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

補足:

- `process` は表示名
- `entry_symbol` は検証用のアンカー
- 最初は symbol list 全展開ではなく summary 集計に留める
- `risk` や `by_depth` と同様、まず小さく出す

## 7. 次に必要な作業

この判断を実装タスクに落とすなら、先に必要なのは次。

1. process 妥当性 fixture の新設
2. 言語別 entrypoint 抽出 rule の design note
3. initial support languages の明示
4. experimental flag / schema の切り方の決定

逆に言うと、この 4 つがないまま feature 実装に進むのは早い。

## 8. 最終判断

G2-8 の最終判断はこう。

- **`affected_processes` experimental 化は、今は不要**
- ただし長期的には候補として維持する
- 次に進むなら、まず process-ground-truth を持つ fixture と rule note を作る
- 実装着手はその後

要するに、G2 時点では
**`affected_modules` を軽く育てる方が費用対効果が高く、`affected_processes` はまだ design-first の段階**。

この順番で進めるのが、今の dimpact にはいちばん無理がない。
