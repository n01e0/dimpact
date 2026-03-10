#!/usr/bin/env python3
"""Render Markdown trend summary between two strict E2E residual JSON snapshots."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


CATEGORY_KEYS = ["env", "server", "capability", "other"]
SUMMARY_KEYS = [
    "operationalResidualLanes",
    "capabilityResidualLanes",
    "actionableResidualLanes",
]


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def val_category(doc: dict, key: str) -> int:
    return int((doc.get("categoryTotals") or {}).get(key, 0))


def val_summary(doc: dict, key: str) -> int:
    return int((doc.get("summary") or {}).get(key, 0))


def fmt_delta(cur: int, prev: int) -> str:
    d = cur - prev
    return f"{d:+d}"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("before_json", type=Path)
    ap.add_argument("after_json", type=Path)
    ap.add_argument("--title", default="strict real-LSP residual trend")
    args = ap.parse_args()

    before = load(args.before_json)
    after = load(args.after_json)

    out: list[str] = []
    out.append(f"## {args.title}")
    out.append("")
    out.append(f"- baseline: `{args.before_json}`")
    out.append(f"- current: `{args.after_json}`")
    out.append("")
    out.append("| metric | baseline | current | delta |")
    out.append("| --- | ---: | ---: | ---: |")

    out.append(
        "| totalSkipPrints | "
        f"{int(before.get('totalSkipPrints', 0))} | "
        f"{int(after.get('totalSkipPrints', 0))} | "
        f"{fmt_delta(int(after.get('totalSkipPrints', 0)), int(before.get('totalSkipPrints', 0)))} |"
    )

    for key in CATEGORY_KEYS:
        b = val_category(before, key)
        a = val_category(after, key)
        out.append(f"| category `{key}` | {b} | {a} | {fmt_delta(a, b)} |")

    for key in SUMMARY_KEYS:
        b = val_summary(before, key)
        a = val_summary(after, key)
        out.append(f"| summary `{key}` | {b} | {a} | {fmt_delta(a, b)} |")

    print("\n".join(out))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
