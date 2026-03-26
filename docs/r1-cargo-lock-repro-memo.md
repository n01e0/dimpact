# R1: Cargo.lock 差分の再現条件メモ

## 結論

`Cargo.lock` の差分は、`rusqlite = "0.37"` のままなのに lockfile だけが `rusqlite 0.38.0` 系へ更新された状態で main に入っているために起きる。

この状態では lockfile が `Cargo.toml` の要求と不整合なので、`cargo metadata` / `cargo check` / `cargo test` / `cargo clippy` のような **`--locked` を付けない Cargo コマンドを 1 回でも実行すると**、Cargo が「最新の互換版」へ再解決し、`Cargo.lock` を `rusqlite 0.37.0` 系へ自動で書き戻す。

つまり、いま見えている `Cargo.lock` 差分は「ローカル環境依存の揺れ」ではなく、**コミット済み lockfile 自体が manifest と食い違っていることの後追い修復差分**。

## 再現条件

クリーンな checkout から次を実行すると再現する。

```bash
cargo metadata --locked --format-version 1
cargo metadata --format-version 1
```

観測結果:

- `cargo metadata --locked` は失敗する
- 続けて `cargo metadata` を実行すると `Cargo.lock` が自動更新される
- 差分は `rusqlite 0.38.0 -> 0.37.0` を中心とした downgrade になる

実測ログ:

```text
$ cargo metadata --locked --format-version 1
Updating crates.io index
error: cannot update the lock file .../Cargo.lock because --locked was passed

$ cargo metadata --format-version 1
Updating crates.io index
 Locking 4 packages to latest compatible versions
 Downgrading foldhash v0.2.0 -> v0.1.5
 Downgrading hashlink v0.11.0 -> v0.10.0
 Downgrading libsqlite3-sys v0.36.0 -> v0.35.0
 Downgrading rusqlite v0.38.0 -> v0.37.0 (available: v0.39.0)
```

`cargo test -q` だけでも同じ差分が発生することを確認した。つまり、普段の開発コマンドで簡単に再現する。

## 直接原因

現在の `Cargo.toml` は次のまま。

```toml
rusqlite = { version = "0.37", features = ["bundled"] }
```

Rust/Cargo の semver では `0.x` 系は minor が互換境界なので、`"0.37"` は実質 `>=0.37.0, <0.38.0` を意味する。したがって `rusqlite 0.38.0` は互換版ではない。

にもかかわらず、`Cargo.lock` は現在 `rusqlite 0.38.0` とその新しい transitive dependency（`sqlite-wasm-rs`, `wasm-bindgen`, `hashlink 0.11.0`, `libsqlite3-sys 0.36.0` など）を保持している。

## 混入経路

`git show dd68b58` を確認すると、PR #343 `chore(deps): bump rusqlite from 0.37.0 to 0.38.0` では **`Cargo.lock` だけ** が更新され、`Cargo.toml` は変更されていない。

つまり main には:

- manifest: `rusqlite 0.37` を要求
- lockfile: `rusqlite 0.38.0` を固定

という矛盾した状態が入っている。

## なぜ CI で見逃されたか

現行の `.github/workflows/CI.yml` では主要な Cargo 実行がいずれも `--locked` なし。

- `cargo check --all-targets`
- `cargo clippy`
- `cargo test -q`

このため CI は checkout 後に lockfile をその場で再解決しても、その変更を failure として扱わない。結果として:

1. PR/CI は green になる
2. しかしローカルで Cargo を実行すると毎回 `Cargo.lock` 差分が出る
3. `--locked` を使う利用者だけが不整合にぶつかる

という状態になる。

## 影響範囲

- `cargo metadata --locked` が失敗する
- `cargo check --locked` / `cargo test --locked` 系の再現性が崩れる
- 開発者が普通に `cargo test` するだけで `Cargo.lock` に意図しない差分が乗る
- release 前確認で「lockfile が汚れる」ノイズになる

## R2 でやるべき修正方針

R1 の範囲では原因特定まで。次の R2 では少なくとも以下のどちらかを選ぶ必要がある。

### 方針 A: `rusqlite 0.38` を正式採用する

- `Cargo.toml` の direct dependency を `0.38` へ上げる
- `Cargo.lock` を再生成して整合させる
- 互換性/破壊的変更の確認を行う

### 方針 B: `rusqlite 0.37` に戻す

- `Cargo.lock` を manifest 準拠の `0.37.x` に戻す
- `Cargo.toml` は据え置く
- PR #343 起因の lockfile-only 更新を解消する

今回の症状だけを見るなら、**最小修正は方針 B**。ただし release 直前に 0.38 系を取り込みたい理由があるなら、manifest 側まで含めて明示的に上げる必要がある。

## 再発防止メモ

R2 以降で以下を入れると再発しにくい。

1. CI の少なくとも 1 ジョブで `cargo metadata --locked` または `cargo check --locked` を実行する
2. 必要なら Cargo 実行後に `git diff --exit-code Cargo.lock` を確認する
3. Dependabot の Cargo PR は manifest 更新の有無をレビュー観点に含める
4. `0.x` 系 dependency は minor bump が breaking であることを前提に扱う
