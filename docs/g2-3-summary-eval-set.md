# G2-3: fixed summary evaluation set

この表は、G2 で固定した summary 評価ケースについて、
`impact` の生 JSON を全部読む代わりに比較しやすい compact snapshot を並べたもの。

- command baseline: `dimpact impact --direction callers --format json`
- confidence は case ごとに固定 (`--min-confidence inferred` が基本、縮退ケースのみ `confirmed`)
- machine-readable snapshot: `docs/g2-3-summary-eval-set.json`
- regenerate: `python3 scripts/collect-summary-eval.py --out-json docs/g2-3-summary-eval-set.json --out-md docs/g2-3-summary-eval-set.md`

| case_id | lang | shape | anchor | observed risk | by_depth view | affected_modules (top) | focus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| rust-callers-chain | rust | chain | medium | medium (d=1, t=1, f=1, s=2) | d1:1s/1f, d2:1s/1f | (root)(2/1) | minimal direct/transitive baseline |
| rust-confidence-filter-empty | rust | filtered-empty | low | low (d=0, t=0, f=0, s=0) | [] | [] | confidence filter shrink-to-empty floor |
| rust-module-fanout | rust | fan-out | high | high (d=4, t=1, f=4, s=5) | d1:4s/4f, d2:1s/1f | alpha(2/2), (root)(2/1), beta(1/1) | affected_modules ordering and grouping baseline |
| rust-confidence-hard | rust | local-dispatch | medium | medium (d=1, t=1, f=1, s=2) | d1:1s/1f, d2:1s/1f | (root)(2/1) | localized hard fixture with small caller spread |
| python-monkeypatch-v4 | python | dynamic-fan-out | high | high (d=3, t=3, f=1, s=6) | d1:3s/1f, d2:3s/1f | demo(6/1) | dynamic caller spread and high-risk anchor |
| ruby-method-missing-v4 | ruby | dynamic | high | medium (d=1, t=0, f=1, s=1) | d1:1s/1f | demo(1/1) | dynamic Ruby boundary case for underestimation checks |
| go-heavy-diff | go | wide-fan-out | high | high (d=1, t=30, f=1, s=31) | 31 buckets; direct=1; transitive=30; max_depth=31 | bench-fixtures/go-heavy(31/1) | high-risk scale anchor from repo diff fixture |

## Notes

- `anchor` は G2-2 baseline で置いた評価帯 (`low` / `medium` / `high`) で、現在の `risk.level` の正誤を確定するものではない。
- `observed risk` と `anchor` のズレは、G2-4 の閾値調整候補を見つけるために残している。
- `affected_modules` は top 3 のみ表示。完全な summary は JSON snapshot を参照。
