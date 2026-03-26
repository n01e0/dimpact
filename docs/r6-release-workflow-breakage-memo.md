# R6: release workflow / action 破損の原因メモ

## 結論

現在の `Release` workflow（`.github/workflows/release.yml`）は、**job のセットアップ段階で毎回即死しており、実際の release 処理まで一度も進めていない**。

直接原因は、GitHub Actions の参照が誤っていて、存在しない action version を指していること。

- `docker/login-action@3`
- `docker/build-push-action@6`

GitHub Actions の解決エラーは次の通り。

```text
Unable to resolve action `docker/build-push-action@6`, unable to find version `6`.
Unable to resolve action `docker/login-action@3`, unable to find version `3`.
```

このため、release workflow は `Set up job` で失敗し、checkout 以降の step は一度も実行されない。

## 観測した失敗範囲

`gh run list --workflow release.yml` で確認した範囲では、少なくとも以下の tag release がすべて同じ壊れ方をしている。

- `v0.2.0`  → https://github.com/n01e0/dimpact/actions/runs/22750188513
- `v0.3.0`  → https://github.com/n01e0/dimpact/actions/runs/22759262493
- `v0.4.0`  → https://github.com/n01e0/dimpact/actions/runs/22779008783
- `v0.4.1`  → https://github.com/n01e0/dimpact/actions/runs/22797395488
- `v0.5.0`  → https://github.com/n01e0/dimpact/actions/runs/22859212644
- `v0.5.1`  → https://github.com/n01e0/dimpact/actions/runs/22894372812
- `v0.5.2`  → https://github.com/n01e0/dimpact/actions/runs/22903999190
- `v0.5.3`  → https://github.com/n01e0/dimpact/actions/runs/22923039311

つまり、release 運用は「最近壊れた」のではなく、**tag push ベースの release workflow が継続的に壊れたまま放置されていた** 状態。

## 直接原因

現在の `release.yml` は以下の action ref を使っている。

```yaml
- name: Log in to the Container registry
  uses: docker/login-action@3

- name: Build and push Docker image
  uses: docker/build-push-action@6
```

一方で、同じリポジトリ内の成功している workflow では、正しく `v` 付き参照を使っている。

- `.github/workflows/image.yml`
  - `docker/login-action@v3`
  - `docker/build-push-action@v6`
- `.github/workflows/ci_image.yml`
  - `docker/login-action@v3`
  - `docker/build-push-action@v6`

つまり release workflow だけが **`v` を落とした typo / copy-paste 崩れ** を持っている。

## 二次的な設計上の問題

`release.yml` の最後には次の step がある。

```yaml
- name: Release GitHub Actions
  uses: technote-space/release-github-actions@v8
```

この action 自体は「GitHub Actions を release するための action」であり、CLI ツール `dimpact` の通常 release を作る目的とはズレている。

外部 README 上でも、この action は **GitHub Actions を自動 release する用途**として説明されている。現状の `dimpact` release でやりたいことは、少なくとも次のどれかのはず。

1. tag に応じて Docker image を push する
2. GitHub Release を作る
3. 必要ならバイナリ asset を添付する

しかし `technote-space/release-github-actions` は 2/3 の汎用的な CLI release 実装としては不自然で、**仮に docker action typo を直しても、release 責務として噛み合っていない可能性が高い**。

## 破損の実態

いま壊れているのは大きく 2 段階ある。

### 1. 即時のハード障害

- action ref が無効なので workflow がセットアップ直後に失敗する
- これは確実な再現あり
- この状態では Docker push も Release step も一切走らない

### 2. 直した後に残る設計不整合

- release workflow の末尾が「GitHub Actions release 専用 action」依存になっている
- `dimpact` の release 責務（CLI / container / GitHub Release）と一致していない
- そのため、**ref typo を直すだけでは「release workflow が何を保証するか」がまだ曖昧**

## 修正方針

### 方針 A: 最小修正（まず壊れた workflow を動かす）

最低限、以下を行う。

```yaml
uses: docker/login-action@v3
uses: docker/build-push-action@v6
```

これで少なくとも release workflow は setup failure から脱出できる。

ただし、この修正だけでは「最後の Release step が適切か」は未解決。

### 方針 B: release 責務を明示して workflow を整理する

おすすめはこっち。

#### B-1. Docker publish と GitHub Release を分ける

- Docker publish: `docker/login-action` + `docker/build-push-action`
- GitHub Release: 専用の release action か `gh release create/upload`

#### B-2. `technote-space/release-github-actions` は削除または置換する

候補:

- Docker image だけで十分なら、この step 自体を削除する
- GitHub Release を作りたいなら、`gh release create` か一般的な release action に置換する
- 将来バイナリ asset を付けるなら、release asset upload を前提に組み直す

#### B-3. workflow 名と job 名も責務に合わせる

現状の job 名は `Build and Push docker image for CI` のままなので、release workflow としては意味がずれている。

例:

- workflow 名: `Release`
- job 名: `Publish release image`
- 別 job: `Create GitHub release`

## 推奨する実施順

1. **即時復旧**
   - `docker/login-action@v3`
   - `docker/build-push-action@v6`
2. **責務整理**
   - `technote-space/release-github-actions@v8` を削除 or 置換
3. **安全確認**
   - `workflow_dispatch` で dry-run 相当の確認
   - test tag / release candidate tag で実行確認
4. **必要なら asset 戦略を追加**
   - Docker image だけで十分か
   - GitHub Release 本文だけ作るか
   - CLI binary を添付するか

## 実務上の判断

今回の障害の主犯は **action version typo** で確定している。ここは迷わず直してよい。

そのうえで、release workflow の末尾にある `technote-space/release-github-actions` は、現行 repo の release 目的と責務がズレて見えるので、**次の修正タスクでは「直す」より「置き換える / 削る」を前提に再設計した方が安全**。
