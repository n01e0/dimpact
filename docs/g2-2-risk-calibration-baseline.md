# G2-2: `risk` 校正 baseline メモ

このメモは、G1 で入れた `summary.risk` を G2 で校正するための **判定基準の土台** を定義する。

目的は 2 つ。

1. 現在の `risk` が何を表しているかを、実装依存の暗黙知ではなく文章で固定する
2. G2-4 以降で閾値や重みを触るときに、**何を過大評価 / 過小評価と呼ぶか** をぶらさないようにする

ここでいう `risk` は、障害確率や本番影響度の予言ではない。
**diff を読んだ直後の triage 優先度** を縮約して示すための label として扱う。

## 1. `risk` が担う役割

`summary.risk` は、少なくとも次の問いに 5〜10 秒で答えられる状態を目指す。

- これは局所 diff か、広がる diff か
- 先に追加レビュー / CI 注意 / fixture 確認を寄せるべき diff か
- raw graph を全部読む前に、どれくらい慎重に見るべきか

つまり `risk` は **severity** というより **triage priority** に近い。

- `low`: まずは局所 diff とみなしてよい
- `medium`: 変更点の近傍だけでは終わらない可能性があるので注意したい
- `high`: 先に広がりを疑って見にいくべき

## 2. 現行実装の基準線

現行の `risk` は `summary.by_depth` と出力サイズから、次の rule で決まる。

- `direct_hits = depth == 1` の symbol 数
- `transitive_hits = depth >= 2` の symbol 数合計
- `impacted_files`
- `impacted_symbols`

`src/impact.rs` の現行条件は次の通り。

### `high`

以下のどれかを満たすと `high`。

- `direct_hits >= 3`
- `direct_hits >= 2 && transitive_hits >= 2`
- `direct_hits >= 1 && transitive_hits >= 4`
- `impacted_files >= 4`
- `impacted_symbols >= 8`

### `medium`

`high` ではなく、以下のどれかを満たすと `medium`。

- `direct_hits >= 1`
- `transitive_hits >= 3`
- `impacted_files >= 2`
- `impacted_symbols >= 4`

### `low`

上のどれにも当たらなければ `low`。

## 3. 校正の前提: 何を「正しい risk」とみなすか

G2 では、`risk` の良し悪しを **人間の読み順に効くか** で判定する。

したがって「正しい `risk`」とは、統計的に厳密というより、
次の判断を大きく外さない label である。

1. 変更点の近傍だけを見れば十分そうか
2. caller 側への波及を先に疑うべきか
3. 追加テストや fixture 確認を優先すべき diff か

このため、厳密な 1 ラベル正解主義ではなく、
**許容帯** と **明確な誤判定** を分けて扱う。

## 4. baseline としての label 意味づけ

### 4.1 `low`

`low` は「安全」を意味しない。
意味するのは、**今の出力から見る限り、まずは局所 diff として読んでよい** ということ。

典型像:

- direct なし、またはあってもごく少数
- transitive が薄い、またはほぼない
- file/symbol の広がりが小さい
- `affected_modules` も 1 箇所に寄る

人間の読み方:

- まず変更ファイルと直近 caller だけ見る
- 追加の広域調査は、raw graph を見て必要ならやる

### 4.2 `medium`

`medium` は **G1/G2 の基準では標準的な注意喚起 label**。

典型像:

- direct が 1 以上ある
- あるいは transitive が少し伸びる
- あるいは file/symbol 数が局所 diff より一段広い
- ただし、まだ「広域に確実に燃えている」とまでは言い切らない

人間の読み方:

- 変更近傍を見たあと、caller 側の 1〜2 段先まで確認候補に入れる
- review / CI / fixture を少し優先する

### 4.3 `high`

`high` は「本番重大事故」ではなく、
**最初から波及前提で読んだ方がよい diff** を指す。

典型像:

- direct が複数ある
- transitive が明確に伸びている
- file/symbol 数だけでも、既に広域 diff と読める
- `affected_modules` も複数塊に分かれる可能性が高い

人間の読み方:

- raw graph / per-seed / fixture を早めに確認する
- 局所修正前提で読むのをやめ、波及の説明責任を先に取りにいく

## 5. 過大評価 / 過小評価の判定ルール

G2 では、各ケースを `keep / overestimate / underestimate` のいずれかでまず粗く判定する。

### 5.1 過大評価 (`overestimate`)

次のようなケースで `risk` が高すぎるとき、過大評価とみなす。

### 強い過大評価

以下に近いのに `high` が出る。

- 実質 single-file / single-module の局所 diff
- direct が 0〜1、transitive も 0〜1 程度
- summary を見ても「まず変更点近傍だけ見ればよい」と読める
- raw graph を開いても調査範囲がほぼ増えない

例:

- confidence filter 後にほぼ空へ縮退するケース
- 局所変更で caller 1 本しか増えないケース
- analyzer hard fixture でも影響が 1 file / 少数 symbol に閉じるケース

### 弱い過大評価

`medium` で十分なケースに `high` が出る。

代表パターン:

- `impacted_files` や `impacted_symbols` だけで閾値を踏み、
  direct/transitive の形はそこまで強くない
- `affected_modules` が実質 1 塊なのに、size 指標だけで `high` へ寄る
- レビュー順としては「変更近傍 + 近い caller」で足りるのに、
  summary が広域調査を強く促しすぎる

### 過大評価の具体基準

少なくとも次のどれかを満たすと、`high` は疑ってよい。

- direct/transitive のどちらも薄く、広がりの説明が file/symbol 数頼み
- 複数 file に見えても実質同一 module / 同一 chain に閉じる
- 実際に追加確認したくなる対象が 1〜2 箇所しかない

### 5.2 過小評価 (`underestimate`)

次のようなケースで `risk` が低すぎるとき、過小評価とみなす。

### 強い過小評価

以下に近いのに `low` が出る、または `medium` 止まりになる。

- direct が複数ある
- transitive が明確に伸びる
- 複数 module / directory に調査先が分かれる
- raw graph を開く前から「局所 diff とみなすのは危ない」と読める

例:

- fan-out が広い heavy diff
- dynamic dispatch 系 fixture で caller 側が複数経路へ伸びるケース
- analyzer 本体変更で fixture / tests 側も含めて見る先が複数出るケース

### 弱い過小評価

`medium` では弱く、`high` の方が読み順に合うケース。

代表パターン:

- direct が 1 本でも transitive が厚く、波及を先に確認すべき
- file/symbol 数はそこまで大きくなくても、graph の形が fan-out 型
- dynamic / heuristic-heavy なケースで「数字以上に読むコスト」が高い

### 過小評価の具体基準

少なくとも次のどれかを満たすと、`low` は疑ってよい。

- direct が 1 以上あり、さらに transitive も見えているのに `low`
- `affected_modules` が複数塊に割れ、見る順を絞る必要があるのに `low`
- 人間の判断では fixture / per-seed / raw graph 追加確認がほぼ必須なのに `low`

## 6. 許容帯（gray zone）

`risk` は rule-based summary なので、すべてを厳密に 1 段階に固定しすぎない方がよい。
G2 では次を **許容帯** として扱う。

### 6.1 `low` と `medium` の境目

次のようなケースは `low/medium` の揺れを許容してよい。

- direct が 0 で transitive も 1〜2 程度
- file/symbol が小さく、見に行く先も 1 塊
- raw graph を読めば分かるが、summary だけだと広がりがまだ薄い

### 6.2 `medium` と `high` の境目

次のようなケースは `medium/high` の揺れを許容してよい。

- direct は 1〜2 だが transitive がじわっと伸びる
- file/symbol は閾値近傍だが、module grouping はまだ局所寄り
- dynamic case で読みコストは高いが、出力サイズはまだ中規模

ただし、次は許容しない。

- 明らかな局所 diff を `high` にする
- 明らかな広域 fan-out diff を `low` にする

## 7. baseline 用アンカーケース

G2-1 で棚卸しした候補のうち、G2-2 の baseline として最初に使いやすい anchor は次の通り。

### `low` 寄り anchor

- `tests/cli_impact_by_depth.rs` の confidence filter 縮退ケース
  - 空またはほぼ空出力で `low` になるべき
- TSX local callback 系 fixture
  - 影響が局所またはゼロに近いとき、`medium/high` に騒がしく寄らないことを確認したい

### `medium` 寄り anchor

- `tests/cli_impact_by_depth.rs` の Rust 1-hop / 2-hop chain
  - direct=1, transitive=1 は `medium` の基準点として扱いやすい
- `tests/cli_impact_risk.rs` の Rust hard fixture
  - 局所変更だが caller 波及が少しあるケースの基準点

### `high` 寄り anchor

- `bench-fixtures/*-heavy.diff`
  - wide fan-out で `high` 側校正の基準にしやすい
- Python monkeypatch/metaclass/protocol v4
- Ruby DSL / method_missing v4
  - dynamic + transitive 多めケースとして、`medium` で弱すぎないかを見やすい

注意:

- ここではまだ各ケースの実測 verdict を固定しない
- G2-3 で出力を並べ、G2-4 で閾値を触る前に anchor を固める

## 8. G2-4 以降で見るべき論点

この baseline を踏まえると、次に見る論点は主に 4 つ。

1. **direct 優先が強すぎる / 弱すぎるか**
   - direct 1 本を常に `medium` にする現行 rule が妥当か
2. **size 起因の `high` が過大でないか**
   - `impacted_files >= 4` / `impacted_symbols >= 8` 単独で `high` にする妥当性
3. **dynamic case の読みコストを size 以外で拾う必要があるか**
   - Ruby/Python の難ケースで `medium` が弱すぎないか
4. **module 分散を `risk` に混ぜるべきか**
   - `affected_modules` が複数塊に割れることを、`risk` に反映する必要があるか

G2 の目的は、`risk` を複雑化することではない。
あくまで **説明できる閾値で、読み順の外し方を減らす** ことを優先する。

## 9. このメモの使い方

G2-3 以降で各ケースを評価するときは、少なくとも次を残す。

- 現在の `risk` label
- 人間の期待 label
- 過大評価 / 過小評価 / keep の判定
- そう判断した理由
  - direct/transitive の形
  - file/symbol の広がり
  - `affected_modules` の分散
  - raw graph を読む必要性

これで、G2-4 の閾値変更は
「何となく良さそう」ではなく、
**どの誤判定を減らしたい変更か** で説明できるようになる。
