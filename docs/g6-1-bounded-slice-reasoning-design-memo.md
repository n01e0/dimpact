# G6-1: bounded slice の未露出 planner reason と shallow-boundary 課題の棚卸しメモ

このメモは、G5 で入った bounded slice を出発点にして、
G6 で次に解くべき 2 つの課題を整理するためのもの。

1. **planner reason が runtime/output に露出していないこと**
2. **bridge completion が「1 回だけの浅い追加」に留まっていること**

G5 は useful だった。

- bounded slice という言葉と policy を docs に固定できた
- `--with-pdg` / `--with-propagation` で同じ slice builder を共有できた
- Rust の imported-result alias continuation を 1 件 real improvement として通せた
- witness path を ranked / compact にできた

ただし G5-10 rollup でも整理した通り、
**planner はまだ最小内部実装で、reason-aware な explainability surface にはなっていない。**
また、現在の scope growth は実質的に

- direct boundary
- + bridge completion 1 file

で止まっており、multi-file bridge をもう少し説明可能にするには shallow すぎる。

つまり G6 の本体は「もっと広くする」ことではない。
**bounded なまま、理由を見せられる planner にして、2-hop を controlled に扱うこと** である。

---

## 1. G5 で実際に入った bounded slice の状態

現在の bounded slice 実装は主に `src/bin/dimpact.rs` の

- `plan_bounded_slice()`
- `build_pdg_context()`

にある。

runtime の実態を短く言うと次の通り。

1. root file 集合を作る
2. seed から call-graph 上の direct boundary symbol を 1-hop 取る
3. 各 seed について bridge completion を 1 file だけ追加する
4. その union を cache update / local DFG build に流す
5. その後に impact / witness を通常通り返す

この設計で G5 が得たものは明確だった。

- plain PDG でも 3-file bounded slice を持てるようになった
- propagation-only だった scope widening が shared scope になった
- imported-result alias continuation が 1 件 real bridge improvement として landed した
- witness は compact になり、equal-depth tie も deterministic になった

一方で、この実装はあくまで **minimal builder** であり、
G5 の docs が語っていた richer planner contract をまだ runtime に持ち込んでいない。

---

## 2. 未露出 planner reason の棚卸し

G6 の最初の論点は、bounded slice の planner reason が
**設計文書にはあるのに runtime surface には無い** こと。

ここでは、そのギャップを 5 つに分ける。

## 2.1 tier の概念はあるが、現在の plan には残っていない

`src/bin/dimpact.rs` には

```rust
enum SliceSelectionTier {
    Root,
    DirectBoundary,
    BridgeCompletion,
}
```

がある。

しかし `add_slice_path()` では `tier` 引数が `_tier` として捨てられており、
最終的な `BoundedSlicePlan` は

```rust
struct BoundedSlicePlan {
    cache_update_paths: Vec<String>,
    local_dfg_paths: Vec<String>,
}
```

しか持たない。

つまり runtime から見ると、

- この path は root だったのか
- direct boundary だったのか
- bridge completion だったのか

が完全に落ちている。

G5 の docs / README は bounded slice を
reason-aware な scope model として説明しているが、
実装はまだ **path membership only** の段階に留まっている。

## 2.2 per-seed plan がなく、union 後の path list だけが残る

G5-1 / G5-3 では一貫して
**per-seed planning, union execution** を理想形としていた。

しかし current runtime では、planner の結果は union 済み path list に近い。
そのため後から

- どの seed のためにこの file が入ったのか
- 複数 seed のうちどれが primary な選定理由だったのか
- 同じ file が seed A では direct boundary、seed B では bridge completion なのか

を再構成できない。

これは `--per-seed` JSON と特に相性が悪い。
impact は per-seed で返せても、slice reason は per-seed で返せないからである。

## 2.3 explanation scope が witness と別れておらず、「why this file?」が答えられない

G5 の witness 改善で、`impacted_witnesses` はかなり見やすくなった。

- `path`
- `path_compact`
- `provenance_chain_compact`
- `kind_chain_compact`

は、"なぜこの symbol が impact されたか" を説明するには useful である。

ただし witness は **symbol path explanation** であって、
**slice selection explanation** ではない。

今まだ無いのは次の面である。

- なぜ `adapter.rs` が slice に入ったのか
- なぜ `leaf.rb` が completion 扱いなのか
- witness に出ていない file が scope には入っている理由は何か

つまり current output は

- `why this symbol/path?` は少し答えられる
- `why this file was selected?` はほぼ答えられない

という非対称を持っている。

## 2.4 prune / budget drop が観測できない

G5-3 policy は budget-aware だった。
本来は後から

- completion candidate が複数いたが 1 つに絞った
- module companion fallback を使わなかった
- budget の都合でこの file は落とした

を見える形で残すべきだった。

しかし現実の `BoundedSlicePlan` は selected path のみで、
**落とされた candidate が観測不能** である。

そのため失敗時に

- planner が file を見つけられなかったのか
- 見つけたが budget で落としたのか
- ranking の結果ほかの candidate が勝ったのか

を区別しにくい。

G6 で planner reason を出すなら、selected reason だけでなく
**pruned candidate の最小 trace** も必要になる。

## 2.5 tests が path inclusion までは固定しているが、reason surface は未固定

G5 のテストは大事な前進を固定している。

- third-file leaf/core が scope に入ること
- imported-result alias continuation が繋がること
- compact witness が stable なこと

しかし今の regression は主に

- path が入ったか
- edge / witness が出たか
- compact summary が安定か

を見ており、
**selection reason 自体の contract はまだ test されていない。**

そのため G6 で schema を入れるなら、
"出力に reason がある" だけでは足りず、
少なくとも

- root / direct boundary / bridge completion の区別
- per-seed attribution
- prune/budget の最小診断

を fixture 化して、docs と実装を再びズラさない必要がある。

---

## 3. shallow-boundary 課題の棚卸し

G6 のもう一つの論点は、今の bounded slice が
**small ではあるが shallow すぎる** こと。

ここでは current builder の弱点を 6 つに分ける。

## 3.1 bridge completion が「per seed で 1 回だけ」なので粗すぎる

`plan_bounded_slice()` では `bridge_completion_added` が seed ごとに 1 個だけあり、
1 度 completion file を足すと、その seed の残り boundary 候補はそこで打ち切られる。

これは implementation としては簡潔だが、policy としては粗い。

理由:

- boundary file が複数ある seed で side ごとの completion を持てない
- callers/callees/both の違いが budget に反映されない
- "seed に 1 つ" と "boundary side に 1 つ" の差を表せない

G5-3 が想定していたのは、もっと **controlled / reason-aware な Tier 2** であって、
単なる first-hit stop ではない。

## 3.2 completion 選択が bridge kind を持たず、first-match に寄りすぎる

current completion は、boundary symbol からもう 1 hop の call-graph 隣接を見て、

- root file ではない
- boundary file ではない
- まだ入っていない

を満たした最初の file を追加している。

ここにはまだ次の区別がない。

- wrapper-return completion
- boundary-side alias continuation
- require-relative chain completion
- dynamic-send guard を壊しにくい completion

つまり G5 docs の bridge kind は concept としてはあっても、
runtime selection ではまだ **scored / typed** されていない。

このため、今の Tier 2 は
"bridge を閉じる 1 file" ではなく、実装上はかなり
**first acceptable extra file** に近い。

## 3.3 same-direction の call-graph 2-hop に寄りすぎている

completion は `collect_related_call_symbols()` をもう一回回す形なので、
本質的には **same-direction call adjacency** に強く依存している。

これは short wrapper / alias continuation には効くが、
次のような面では shallow になりやすい。

- summary/return の理由はあるが call adjacency だけでは ranking が弱い
- Ruby `require_relative` 連鎖で graph-first と companion fallback の切り分けが要る
- callers 方向で boundary を越えた後の leaf 側 return-ish 説明が薄い

G6 でやるべきなのは project-wide 化ではなく、
**2-hop を許す条件を call adjacency 以外の bridge evidence と合わせて制御すること** である。

## 3.4 companion fallback が現実の builder にはまだ入っていない

G5-3 policy では、必要時だけ

- module companion
- require-relative companion

の fallback を small に入れる余地を持たせていた。

しかし current builder は

- root
- direct boundary
- bridge completion

までで止まっており、fallback tier は runtime 化されていない。

そのため Ruby 側では特に、
"graph-first では取り切れないが、companion fallback を small に使えば説明できる"
タイプの case がまだ宙に浮いている。

## 3.5 1-hop + 1 completion は longer multi-file stack ではすぐ限界に当たる

G5-10 rollup が整理した通り、今の bounded slice は
short 2-hop bridge には効くが、次のような面には足りない。

- recursive adapter chain
- serializer / service / wrapper stack
- longer return / alias continuation ladder

ここで重要なのは、G6 の target が無制限 closure ではないこと。
必要なのは

- 2-hop を project-wide へ開放すること

ではなく、

- **どの 2-hop を controlled に許すか**
- **どの 2-hop は shallow boundary のまま止めるか**

を policy と output の両方で説明可能にすることだ。

## 3.6 Ruby 側は「covered / guarded」先行で、real improvement はまだ薄い

G5 の strongest improvement は Rust imported-result alias continuation だった。
一方 Ruby 側はまだ

- `require_relative` chain は coverage / witness stabilization が先行
- dynamic-send target separation は guard case としての意味が強い

という状態に近い。

つまり shallow-boundary 課題は Ruby 側で特に残っている。
G6 で 2-hop policy を詰めるなら、
**Ruby 3-file chain を 1 件 real upgrade できるか** を acceptance surface に含めるのが自然である。

---

## 4. G6 で必要な設計目標

上の棚卸しから、G6 の設計目標は次の 5 つになる。

## 4.1 bounded slice を "見える planner" にする

G6 の第一目標は、planner reason を file-level metadata として持たせ、
少なくとも JSON/debug surface で観測可能にすること。

最低限見えるべきもの:

- selected file list
- reason per file
- per-seed attribution
- selected tier / bridge kind
- budget で落ちた candidate の最小診断

これは G6-2 / G6-3 の中心になる。

## 4.2 Tier 2 を "single extra file" から "controlled 2-hop" へ上げる

G6 の第二目標は、bridge completion を完全自由化することではない。
今の bounded model を壊さず、

- per seed
- per boundary side
- per bridge kind
- small budget

で制御された **controlled 2-hop** にすることが狙いになる。

ここで大事なのは、
"2-hop を許す" と "2-hop を全部取る" を混同しないこと。
G6 で欲しいのは前者だけである。

## 4.3 witness と slice reason を接続する

現在は

- witness = symbol/path explanation
- slice = hidden internal scope choice

に分かれている。

G6 では少なくとも、

- witness path に出てくる file が slice 上ではどう選ばれたか
- witness に出ないが scope に入った file はなぜ retained されたか

を辿れるようにしたい。

これは full proof graph を作る話ではない。
むしろ
**why-this-file / why-this-path を軽く接続する** くらいの bounded な explainability がちょうどよい。

## 4.4 cache update / local DFG / explanation を runtime struct でも分離する

G5 の docs ではこの分離をかなり前から望んでいた。
G6 ではこれを runtime struct にも反映したい。

理由は単純で、planner reason を出し始めると

- cache の都合で入った file
- local DFG build の都合で入った file
- explanation として見せたい file

を同じ意味で扱えなくなるからである。

## 4.5 Ruby 3-file case を one real improvement として取る

G6 の docs/policy だけを先に進めても、
Rust 側だけ improvement が進み Ruby 側は explainability だけ増える、
という偏りになりやすい。

そのため G6 は

- reason surface を作る
- controlled 2-hop を入れる
- Ruby 3-file chain を 1 件 real improvement で取る

までが揃って初めてバランスがよい。

---

## 5. G6 の進め方メモ

tasklist に並んでいる G6 の流れは、かなり自然である。

## 5.1 G6-2 / G6-3: planner reason の schema と露出

ここではまず

- file-level metadata の schema
- per-seed / union の持ち方
- selected reason / pruned candidate の出し方

を決める。

この段階ではまだ 2-hop policy を広げ過ぎない方がよい。
まず current builder の decisions をそのまま可視化し、
**現状を説明できること** を先に固定するのが筋である。

## 5.2 G6-4 / G6-5: controlled 2-hop policy と最小実装

reason surface が出たら、次に Tier 2 を controlled 2-hop に上げる。
ここで入れたいのは例えば次の制御である。

- completion budget を per-seed ではなく per-boundary-side で持つ
- bridge kind による priority を持つ
- callers / callees / both で stop rule を分ける
- Ruby `require_relative` 系は companion fallback を narrow に許す

これで初めて、"なぜこの 2-hop は許したのか" を output と policy で一致させられる。

## 5.3 G6-6 / G6-7: Rust / Ruby の acceptance case を 1 件ずつ前進させる

controlled 2-hop は policy だけでは意味が薄い。
少なくとも

- Rust 側で 1 件
- Ruby 側で 1 件

の real fixture improvement を取って、
reason surface と algorithm change が噛み合っていることを確認したい。

## 5.4 G6-8: witness への接続

最後に、slice reason と witness を軽く接続する。
ここでは full path overlay までは不要で、
次くらいで十分である。

- impacted witness が通った file の selection reason を併記できる
- why-this-file / why-this-path を seed 単位で参照できる

この程度でも、G5 で残っていた
"docs は bounded slice reasoning を語っているが runtime は黙っている"
問題はかなり改善する。

---

## 6. Non-goal

G6-1 の時点では、次はやらない前提で整理しておく。

- 無制限 project-wide PDG
- 全言語フル local DFG
- multi-candidate witness を全部見せる重い proof graph
- heavy SSA rewrite
- planner を import/path heuristic 主導へ戻すこと

G6 はあくまで **bounded slice を explainable にし、2-hop を controlled にする** フェーズである。

---

## 7. 一言まとめ

G5 の bounded slice は useful だが、まだ

- planner reason が path list の裏に隠れている
- bridge completion が first-match の shallow boundary に近い

という 2 つの弱点を持つ。

したがって G6 の中心課題は、
**bounded modelを保ったまま、file-level reason を露出し、Tier 2 を controlled 2-hop へ引き上げ、witness と slice reason を軽く接続すること** になる。
