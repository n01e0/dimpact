#!/usr/bin/env python3
"""Render Markdown summary for precision regression report JSON."""

from __future__ import annotations

import json
import sys
from pathlib import Path


def main() -> int:
    report_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("precision-regression-report.json")
    if not report_path.exists():
        print(f"precision report not found: {report_path}", file=sys.stderr)
        return 1

    report = json.loads(report_path.read_text())

    totals = report.get("totals", {})
    print("### Totals")
    print(f"- FN: {totals.get('fn', 'n/a')} (threshold={report.get('fnThreshold', 'n/a')})")
    print(f"- FP: {totals.get('fp', 'n/a')} (threshold={report.get('fpThreshold', 'n/a')})")

    diff_summary = report.get("diffSummary", {})
    if diff_summary:
        fn = diff_summary.get("fn", {})
        fp = diff_summary.get("fp", {})
        print("\n### Diff summary")
        print(f"- FN changed: {fn.get('changed', 'n/a')}")
        print(f"- FN impacted: {fn.get('impacted', 'n/a')}")
        print(f"- FP changed: {fp.get('changed', 'n/a')}")
        print(f"- FP impacted: {fp.get('impacted', 'n/a')}")

    confidence = report.get("confidenceDistribution", {})
    if confidence:
        print("\n### Confidence distribution")
        for certainty in sorted(confidence):
            print(f"- {certainty}: {confidence[certainty]}")

    by_language = report.get("byLanguage", {})
    if by_language:
        print("\n### By language thresholds")
        for lang in sorted(by_language):
            row = by_language[lang] or {}
            th = row.get("threshold", {})
            print(
                f"- {lang}: fn={row.get('fn', 'n/a')} (th={th.get('fn', 'n/a')}), "
                f"fp={row.get('fp', 'n/a')} (th={th.get('fp', 'n/a')})"
            )

    cases = report.get("cases", [])
    hotspots = [
        c
        for c in cases
        if c.get("fn", {}).get("total", 0) > 0 or c.get("fp", {}).get("total", 0) > 0
    ]
    if hotspots:
        print("\n### Diff hotspots (FN/FP > 0)")
        for c in hotspots:
            print(
                f"- {c.get('name')} ({c.get('lang')}): "
                f"fn={c.get('fn', {}).get('total', 0)} "
                f"fp={c.get('fp', {}).get('total', 0)}"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
