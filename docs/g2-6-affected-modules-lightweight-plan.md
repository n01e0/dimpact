# G2-6: `affected_modules` の粗さ洗い出しと軽量改善方針

このメモは、G1 で入れた `summary.affected_modules` について、
現状の naming / grouping の粗さをどこに感じるかを整理し、
G2-7 で入れる **軽量な改善方針** を先に固定するためのもの。

結論だけ先に書くと、G2-7 では **path grouping 自体は維持しつつ、表示名の正規化を 1 段だけ足す** のがよい。
call graph clustering や言語別 namespace 解釈までは入れない。

## 1. 現在の実装がやっていること

`src/impact.rs` の `affected_module_for_file()` は、かなり単純な rule になっている。

- `./` を落として path を正規化
- 親ディレクトリがあればその path を module 名にする
- 親ディレクトリがなければ file path そのものを module 名にする

つまり現在の `affected_modules` は、**親ディレクトリ fallback + root file 名そのまま** で作られている。

この簡潔さは G1 としては妥当だったが、G2 の固定評価セットで見ると、
「次にどこを開くか」の導線として少し粗い箇所が見えてきた。

## 2. 固定評価セットから見えた粗さ

G2-3 の固定セット (`docs/g2-3-summary-eval-set.md`) から、主に次の粗さが確認できる。

### 2.1 root file fallback が directory grouping と粒度混在する

代表例:

- `rust-callers-chain` → `main.rs(2/1)`
- `rust-confidence-hard` → `main.rs(2/1)`
- `rust-module-fanout` → `alpha(2/2), main.rs(2/1), beta(1/1)`

ここで気になるのは、同じ list の中に

- `alpha` / `beta` のような **directory 粒度**
- `main.rs` のような **単一 file 粒度**

が混在していること。

特に `rust-module-fanout` では、
利用者が知りたいのは「alpha / beta / root 周辺のどこから見るか」であって、
`main.rs` という file 名そのものが前面に出る必要はそこまで高くない。

### 2.2 root entry-like file 名が misleading になりやすい

`main.rs` / `lib.rs` / `mod.rs` / `index.ts` / `__init__.py` のような file は、
実体としては「その directory / package の入口」寄りで、
module 名として file 名をそのまま見せると少し説明力が落ちる。

例:

- `src/main.rs` を `src` ではなく `src/main.rs` 的に見せたいのか
- repo root の `main.rs` を module 名として見せたいのか
- `pkg/__init__.py` を package `pkg` として見せたいのか

この手の file は、**file 名そのものより root / package を見せた方が triage に効く** ことが多い。

### 2.3 単一 top-level dir へ寄る fixture では grouping が粗くなりやすい

代表例:

- `python-monkeypatch-v4` → `demo(6/1)`
- `ruby-method-missing-v4` → `demo(1/1)`

fixture 上は妥当ではあるが、`demo` だけだと
「何の塊か」は分かっても、少し抽象的すぎる。

ただし、ここで細かくしすぎると今度は
`demo/foo/bar/baz.py` のように分割しすぎて summary の意味が薄くなる。

つまりこの問題は、
**今すぐ namespace / graph clustering で解くべき問題ではなく、
lightweight path 表示のままどこまで正規化するか** の問題として扱うのがよい。

### 2.4 bench / fixture artifact では人間向け表示としてやや不自然なことがある

代表例:

- `go-heavy-diff` → `bench-fixtures/go-heavy(31/1)`

これは fixed eval 上では許容できるが、
人間に見せる summary としては「path の切り方がそのまま出すぎている」感じがある。

ただし bench-fixture 固有の見え方を理由に、
本番向け grouping rule を重くしすぎるのは避けたい。

## 3. 粗さをどう分類するか

G2-7 で潰す対象は、次の 2 種類に分けるのがよい。

### 3.1 今すぐ軽く直せる粗さ

- root file fallback が directory 粒度と混ざる
- main / lib / mod / index / `__init__` のような入口 file が file 名のまま出る
- root bucket の表示名が file 名依存でぶれる

これらは **grouping algorithm をほぼ維持したまま、label 正規化だけで改善できる**。

### 3.2 今は直さない粗さ

- Python/Ruby dynamic case で package 名だけだと少し粗い
- namespace ベース (`crate::foo`, `pkg.foo`, `demo.foo`) の方が自然な言語がある
- graph 的には 1 つの塊だが path 的には分かれて見えるケース

これらは軽量版の範囲を超えやすい。
G2-7 で無理に解くと、`affected_modules` が path summary ではなく別物になってしまう。

## 4. 比較した選択肢

### 選択肢 A: 現状維持

長所:

- 実装が最も単純
- 既存テストと説明をそのまま維持できる

短所:

- `main.rs` のような fallback が list に混ざり続ける
- directory と file の粒度が混在したままで、読む順の案内として少しノイジー

G2 では、この案は弱い。
評価セットで既に気になる点が見えているから。

### 選択肢 B: `module_path_for_file()` 相当の強い正規化へ寄せる

長所:

- `mod.rs` / `lib.rs` / `main.rs` / `index.ts` / `__init__.py` などを directory / package 表現に寄せやすい
- 言語別 module path へ将来つなぎやすい

短所:

- `foo.rs` → `foo` のような file-stem 化まで入ると、path grouping より意味が強くなりすぎる
- Rust/Go/Java/Python/Ruby/TS 系で見え方の期待値が変わり、G2-7 のスコープを越えやすい
- 既存の `affected_modules` が「path そのまま」の summary でなくなる

これは将来候補としては悪くないが、今の task には重い。

### 選択肢 C: path grouping は維持し、display label だけ正規化する

長所:

- 現行の grouping の軽さを保てる
- root fallback の粗さだけを重点的に下げられる
- G2-7 でテストしやすい

短所:

- `demo` のような coarse bucket 自体は残る
- namespace 的な自然さまでは得られない

G2-7 の対象としては、この案が一番バランスがよい。

## 5. 採用方針

**選択肢 C を採る。**

すなわち、G2-7 では

- grouping の元データは引き続き path ベース
- ただし module label にだけ軽い正規化を入れる
- graph clustering / namespace 解釈 / repo 固有 rule は入れない

という形にする。

## 6. G2-7 に渡す具体ルール

### 6.1 基本方針

module 名は「まずどの directory / package から開くか」を示す label として扱う。
したがって、**入口 file をそのまま file 名で見せるより、親 directory 側へ畳む** 方を優先する。

### 6.2 正規化対象

次の file 名は **entry-like file** とみなして正規化する。

- Rust: `main.rs`, `lib.rs`, `mod.rs`
- JS/TS: `index.js`, `index.ts`, `index.tsx`
- Python: `__init__.py`

### 6.3 label rule

#### rule 1: 親 directory がある entry-like file は親 directory を label にする

例:

- `src/main.rs` → `src`
- `src/lib.rs` → `src`
- `src/engine/mod.rs` → `src/engine`
- `pkg/__init__.py` → `pkg`
- `web/index.ts` → `web`

#### rule 2: repo root の entry-like file は専用 root label に寄せる

例:

- `main.rs` → `(root)`
- `lib.rs` → `(root)`
- `index.ts` → `(root)`

`main.rs` のまま見せるより、root bucket として明示した方が
`alpha`, `beta`, `tests` のような directory 群と並べたときの意味が揃う。

#### rule 3: それ以外の file は現状維持

例:

- `src/engine/lsp.rs` → `src/engine`
- `tests/cli_impact_risk.rs` → `tests`
- root 直下の `foo.rs` → `foo.rs`

ここで root 直下の一般 file まで無理に stem 化しない。
それを始めると「path summary」から意味がずれていくため。

### 6.4 sort rule は基本維持、ただし root label は同点時に後ろ寄せを検討してよい

現行の

- `symbol_count desc`
- 同点なら `module asc`

は基本維持でよい。

ただし `(root)` は directory 名より少し説明力が弱いので、
**同点時だけ non-root bucket を先に出す** のは検討価値がある。

これは optional だが、`alpha(2), (root)(2), beta(1)` のような並びの方が
`alpha, main.rs, beta` より読みやすい可能性が高い。

## 7. この方針でどう見え方が変わるか

G2-3 のケースに当てると、おおむね次の変化を狙うことになる。

| case | 現状 | 目標イメージ |
| --- | --- | --- |
| rust-callers-chain | `main.rs(2/1)` | `(root)(2/1)` |
| rust-confidence-hard | `main.rs(2/1)` | `(root)(2/1)` |
| rust-module-fanout | `alpha(2/2), main.rs(2/1), beta(1/1)` | `alpha(2/2), (root)(2/1), beta(1/1)` |
| python-monkeypatch-v4 | `demo(6/1)` | 基本据え置き |
| ruby-method-missing-v4 | `demo(1/1)` | 基本据え置き |
| go-heavy-diff | `bench-fixtures/go-heavy(31/1)` | 基本据え置き |

つまり G2-7 では、**全部を賢くするのではなく、今いちばんノイジーな root fallback だけをまず揃える**。

## 8. 今回やらないこと

G2-7 では次はやらない。

- `foo.rs` → `foo` のような全面的 file-stem 化
- `crate::foo`, `pkg.foo`, `demo.foo` のような namespace 表示
- path ではなく call graph で cluster を作る
- repository 固有の alias / grouping rule を持つ
- `affected_modules` から module ごとの risk を再計算する

これらは、`affected_modules` の lightweight さを崩しやすい。

## 9. 一言まとめ

- 現状の粗さの中心は **root fallback が file 名のまま出ること**
- それ以外の coarse bucket は、今の段階では大きな欠陥というより lightweight path grouping の限界
- G2-7 は **entry-like file を親 directory / `(root)` に正規化する軽量改善** に絞る
- namespace 化や clustering は後段に送る
