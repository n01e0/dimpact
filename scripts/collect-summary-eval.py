#!/usr/bin/env python3
"""Collect a fixed summary-evaluation snapshot for G2 work.

This script builds a compact, reproducible set of `impact` summary outputs so
G2 work can compare `by_depth` / `risk` / `affected_modules` without reading the
full raw symbol/edge JSON for every fixture.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_JSON_OUT = REPO_ROOT / "docs/g2-3-summary-eval-set.json"
DEFAULT_MD_OUT = REPO_ROOT / "docs/g2-3-summary-eval-set.md"


def run(cmd: list[str], cwd: Path, *, input_bytes: bytes | None = None) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )


def git(cwd: Path, *args: str) -> None:
    run(["git", *args], cwd)


def ensure_binary(repo_root: Path, *, skip_build: bool) -> Path:
    binary = repo_root / "target/debug/dimpact"
    if skip_build and binary.exists():
        return binary
    run(["cargo", "build", "--quiet", "--bin", "dimpact"], repo_root)
    if not binary.exists():
        raise FileNotFoundError(f"dimpact binary not found after build: {binary}")
    return binary


def diff_text(repo: Path) -> bytes:
    return run(["git", "diff", "--no-ext-diff", "--unified=0"], repo).stdout


def compact_summary(output: dict[str, Any]) -> dict[str, Any]:
    summary = output["summary"]
    return {
        "by_depth": [
            {
                "depth": bucket["depth"],
                "symbol_count": bucket["symbol_count"],
                "file_count": bucket["file_count"],
            }
            for bucket in summary["by_depth"]
        ],
        "risk": {
            "level": summary["risk"]["level"],
            "direct_hits": summary["risk"]["direct_hits"],
            "transitive_hits": summary["risk"]["transitive_hits"],
            "impacted_files": summary["risk"]["impacted_files"],
            "impacted_symbols": summary["risk"]["impacted_symbols"],
        },
        "affected_modules": [
            {
                "module": item["module"],
                "symbol_count": item["symbol_count"],
                "file_count": item["file_count"],
            }
            for item in summary["affected_modules"]
        ],
    }


def run_impact(binary: Path, repo: Path, diff: bytes, lang: str, cli_args: list[str]) -> dict[str, Any]:
    cmd = [
        str(binary),
        "impact",
        "--direction",
        "callers",
        "--lang",
        lang,
        "--format",
        "json",
        *cli_args,
    ]
    raw = run(cmd, repo, input_bytes=diff).stdout
    return json.loads(raw)


def with_temp_repo(builder, fn):
    with tempfile.TemporaryDirectory(prefix="dimpact-summary-eval-") as td:
        repo = Path(td)
        git(repo, "init", "-q")
        git(repo, "config", "user.email", "tester@example.com")
        git(repo, "config", "user.name", "Tester")
        builder(repo)
        git(repo, "add", ".")
        git(repo, "commit", "-m", "init", "-q")
        mutate = getattr(builder, "mutate")
        mutate(repo)
        return fn(repo, diff_text(repo))


def build_rust_chain(repo: Path) -> None:
    (repo / "main.rs").write_text(
        "fn leaf() {}\nfn mid() { leaf(); }\nfn top() { mid(); }\n",
        encoding="utf-8",
    )


def mutate_rust_chain(repo: Path) -> None:
    (repo / "main.rs").write_text(
        "fn leaf() { let _x = 1; }\nfn mid() { leaf(); }\nfn top() { mid(); }\n",
        encoding="utf-8",
    )


build_rust_chain.mutate = mutate_rust_chain  # type: ignore[attr-defined]


def build_rust_module_fanout(repo: Path) -> None:
    (repo / "alpha").mkdir(parents=True, exist_ok=True)
    (repo / "beta").mkdir(parents=True, exist_ok=True)
    (repo / "main.rs").write_text(
        """mod alpha;
mod beta;
mod leaf;

fn root_one() {
    crate::leaf::leaf();
}

fn main() {
    crate::alpha::first::alpha_one();
    crate::alpha::second::alpha_two();
    crate::beta::first::beta_one();
    root_one();
}
""",
        encoding="utf-8",
    )
    (repo / "alpha/mod.rs").write_text("pub mod first;\npub mod second;\n", encoding="utf-8")
    (repo / "alpha/first.rs").write_text(
        "pub fn alpha_one() { crate::leaf::leaf(); }\n", encoding="utf-8"
    )
    (repo / "alpha/second.rs").write_text(
        "pub fn alpha_two() { crate::leaf::leaf(); }\n", encoding="utf-8"
    )
    (repo / "beta/mod.rs").write_text("pub mod first;\n", encoding="utf-8")
    (repo / "beta/first.rs").write_text(
        "pub fn beta_one() { crate::leaf::leaf(); }\n", encoding="utf-8"
    )
    (repo / "leaf.rs").write_text("pub fn leaf() {}\n", encoding="utf-8")


def mutate_rust_module_fanout(repo: Path) -> None:
    (repo / "leaf.rs").write_text("pub fn leaf() { let _x = 1; }\n", encoding="utf-8")


build_rust_module_fanout.mutate = mutate_rust_module_fanout  # type: ignore[attr-defined]


def build_fixture_case(rel_fixture: str, repo_file: str, before_replace: str, after_replace: str):
    fixture = (REPO_ROOT / rel_fixture).read_text(encoding="utf-8")

    def builder(repo: Path) -> None:
        target = repo / repo_file
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(fixture, encoding="utf-8")

    def mutate(repo: Path) -> None:
        target = repo / repo_file
        target.write_text(fixture.replace(before_replace, after_replace, 1), encoding="utf-8")

    builder.mutate = mutate  # type: ignore[attr-defined]
    return builder


RUST_HARD_BUILDER = build_fixture_case(
    "tests/fixtures/rust/analyzer_hard_cases_confidence_compare.rs",
    "main.rs",
    "x + 1",
    "x + 2",
)

PYTHON_MONKEY_V4_BUILDER = build_fixture_case(
    "tests/fixtures/python/analyzer_hard_cases_dynamic_monkeypatch_metaclass_protocol_v4.py",
    "demo/monkey_v4.py",
    "return payload.strip().lower()",
    "return payload.strip().upper()",
)

RUBY_DSL_V4_BUILDER = build_fixture_case(
    "tests/fixtures/ruby/analyzer_hard_cases_dynamic_dsl_method_missing_chain_v4.rb",
    "demo/ruby_v4.rb",
    "send(target, payload)",
    "send(target, payload.to_s)",
)


def collect_cases(binary: Path) -> dict[str, Any]:
    cases = []

    def add_temp_case(
        *,
        case_id: str,
        lang: str,
        shape: str,
        anchor_band: str,
        focus: str,
        source: str,
        cli_args: list[str],
        builder,
    ) -> None:
        def collect(repo: Path, diff: bytes) -> None:
            output = run_impact(binary, repo, diff, lang, cli_args)
            cases.append(
                {
                    "case_id": case_id,
                    "lang": lang,
                    "shape": shape,
                    "anchor_band": anchor_band,
                    "focus": focus,
                    "source": source,
                    "cli_args": cli_args,
                    "summary": compact_summary(output),
                }
            )

        with_temp_repo(builder, collect)

    add_temp_case(
        case_id="rust-callers-chain",
        lang="rust",
        shape="chain",
        anchor_band="medium",
        focus="minimal direct/transitive baseline",
        source="synthetic fixture mirrored from tests/cli_impact_by_depth.rs",
        cli_args=["--min-confidence", "inferred"],
        builder=build_rust_chain,
    )
    add_temp_case(
        case_id="rust-confidence-filter-empty",
        lang="rust",
        shape="filtered-empty",
        anchor_band="low",
        focus="confidence filter shrink-to-empty floor",
        source="synthetic fixture mirrored from tests/cli_impact_by_depth.rs",
        cli_args=["--min-confidence", "confirmed"],
        builder=build_rust_chain,
    )
    add_temp_case(
        case_id="rust-module-fanout",
        lang="rust",
        shape="fan-out",
        anchor_band="high",
        focus="affected_modules ordering and grouping baseline",
        source="synthetic fixture mirrored from tests/cli_impact_affected_modules.rs",
        cli_args=["--min-confidence", "inferred"],
        builder=build_rust_module_fanout,
    )
    add_temp_case(
        case_id="rust-confidence-hard",
        lang="rust",
        shape="local-dispatch",
        anchor_band="medium",
        focus="localized hard fixture with small caller spread",
        source="tests/fixtures/rust/analyzer_hard_cases_confidence_compare.rs",
        cli_args=["--min-confidence", "inferred"],
        builder=RUST_HARD_BUILDER,
    )
    add_temp_case(
        case_id="python-monkeypatch-v4",
        lang="python",
        shape="dynamic-fan-out",
        anchor_band="high",
        focus="dynamic caller spread and high-risk anchor",
        source="tests/fixtures/python/analyzer_hard_cases_dynamic_monkeypatch_metaclass_protocol_v4.py",
        cli_args=["--min-confidence", "inferred"],
        builder=PYTHON_MONKEY_V4_BUILDER,
    )
    add_temp_case(
        case_id="ruby-method-missing-v4",
        lang="ruby",
        shape="dynamic",
        anchor_band="high",
        focus="dynamic Ruby boundary case for underestimation checks",
        source="tests/fixtures/ruby/analyzer_hard_cases_dynamic_dsl_method_missing_chain_v4.rb",
        cli_args=["--min-confidence", "inferred"],
        builder=RUBY_DSL_V4_BUILDER,
    )

    go_heavy_diff = (REPO_ROOT / "bench-fixtures/go-heavy.diff").read_bytes()
    go_heavy_output = run_impact(
        binary,
        REPO_ROOT,
        go_heavy_diff,
        "go",
        ["--min-confidence", "inferred"],
    )
    cases.append(
        {
            "case_id": "go-heavy-diff",
            "lang": "go",
            "shape": "wide-fan-out",
            "anchor_band": "high",
            "focus": "high-risk scale anchor from repo diff fixture",
            "source": "bench-fixtures/go-heavy.diff",
            "cli_args": ["--min-confidence", "inferred"],
            "summary": compact_summary(go_heavy_output),
        }
    )

    return {
        "generated_by": "scripts/collect-summary-eval.py",
        "repo_root": str(REPO_ROOT),
        "defaults": {
            "command": "dimpact impact --direction callers --format json",
            "note": "case-specific confidence flags are fixed per case for stable comparison",
        },
        "cases": cases,
    }


def render_by_depth(case: dict[str, Any]) -> str:
    buckets = case["summary"]["by_depth"]
    if not buckets:
        return "[]"
    if len(buckets) <= 6:
        return ", ".join(
            f"d{b['depth']}:{b['symbol_count']}s/{b['file_count']}f" for b in buckets
        )
    risk = case["summary"]["risk"]
    return (
        f"{len(buckets)} buckets; direct={risk['direct_hits']}; "
        f"transitive={risk['transitive_hits']}; max_depth={buckets[-1]['depth']}"
    )


def render_modules(case: dict[str, Any]) -> str:
    modules = case["summary"]["affected_modules"]
    if not modules:
        return "[]"
    return ", ".join(
        f"{m['module']}({m['symbol_count']}/{m['file_count']})" for m in modules[:3]
    )


def render_md(report: dict[str, Any]) -> str:
    lines = [
        "# G2-3: fixed summary evaluation set",
        "",
        "この表は、G2 で固定した summary 評価ケースについて、",
        "`impact` の生 JSON を全部読む代わりに比較しやすい compact snapshot を並べたもの。",
        "",
        "- command baseline: `dimpact impact --direction callers --format json`",
        "- confidence は case ごとに固定 (`--min-confidence inferred` が基本、縮退ケースのみ `confirmed`)",
        "- machine-readable snapshot: `docs/g2-3-summary-eval-set.json`",
        "- regenerate: `python3 scripts/collect-summary-eval.py --out-json docs/g2-3-summary-eval-set.json --out-md docs/g2-3-summary-eval-set.md`",
        "",
        "| case_id | lang | shape | anchor | observed risk | by_depth view | affected_modules (top) | focus |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for case in report["cases"]:
        risk = case["summary"]["risk"]
        lines.append(
            "| {case_id} | {lang} | {shape} | {anchor} | {risk} "
            "(d={direct}, t={transitive}, f={files}, s={symbols}) | {by_depth} | {modules} | {focus} |".format(
                case_id=case["case_id"],
                lang=case["lang"],
                shape=case["shape"],
                anchor=case["anchor_band"],
                risk=risk["level"],
                direct=risk["direct_hits"],
                transitive=risk["transitive_hits"],
                files=risk["impacted_files"],
                symbols=risk["impacted_symbols"],
                by_depth=render_by_depth(case),
                modules=render_modules(case),
                focus=case["focus"],
            )
        )

    lines.extend(
        [
            "",
            "## Notes",
            "",
            "- `anchor` は G2-2 baseline で置いた評価帯 (`low` / `medium` / `high`) で、現在の `risk.level` の正誤を確定するものではない。",
            "- `observed risk` と `anchor` のズレは、G2-4 の閾値調整候補を見つけるために残している。",
            "- `affected_modules` は top 3 のみ表示。完全な summary は JSON snapshot を参照。",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out-json", type=Path, default=DEFAULT_JSON_OUT)
    parser.add_argument("--out-md", type=Path, default=DEFAULT_MD_OUT)
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    if shutil.which("cargo") is None:
        raise SystemExit("cargo not found")

    binary = ensure_binary(REPO_ROOT, skip_build=args.skip_build)
    report = collect_cases(binary)

    args.out_json.parent.mkdir(parents=True, exist_ok=True)
    args.out_md.parent.mkdir(parents=True, exist_ok=True)
    args.out_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    args.out_md.write_text(render_md(report), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
