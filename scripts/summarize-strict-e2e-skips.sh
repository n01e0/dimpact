#!/usr/bin/env bash
set -euo pipefail

SRC="${1:-tests/engine_lsp.rs}"
OUT_JSON="${2:-docs/strict-real-lsp-skip-reasons-v0.4.1.json}"
OUT_MD="${3:-docs/strict-real-lsp-skip-reasons-v0.4.1.md}"

python3 - "$SRC" "$OUT_JSON" "$OUT_MD" <<'PY'
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

src = Path(sys.argv[1])
out_json = Path(sys.argv[2])
out_md = Path(sys.argv[3])

if not src.exists():
    raise SystemExit(f"source not found: {src}")

lines = src.read_text(encoding="utf-8", errors="replace").splitlines()

entries = []
cur_fn = None
brace = 0
in_test = False

for i, line in enumerate(lines, start=1):
    if line.strip().startswith("#[test]"):
        in_test = True
        continue

    m_fn = re.match(r"\s*fn\s+([A-Za-z0-9_]+)\s*\(", line)
    if in_test and m_fn:
        cur_fn = m_fn.group(1)
        brace = 0
        in_test = False

    if cur_fn:
        brace += line.count("{") - line.count("}")
        if "eprintln!(" in line and "skip:" in line:
            blob = line
            j = i
            while ");" not in blob and j < len(lines):
                j += 1
                blob += lines[j - 1]
            parts = re.findall(r'"([^"]*)"', blob)
            msg = " ".join(parts)
            entries.append({"fn": cur_fn, "line": i, "message": msg})
        if brace <= 0:
            cur_fn = None

LANGS = ["typescript", "javascript", "ruby", "go", "java", "python", "rust"]
DIRS = ["callers", "callees", "both"]


def infer_lang(fn_name: str, message: str) -> str:
    fn = fn_name.lower()
    msg = message.lower()
    for l in LANGS:
        if l in fn:
            return l
    # rust tests often omit explicit language in fn name
    if "rust" in msg or "rust-analyzer" in msg or "strict lsp" in msg:
        return "rust"
    return "unknown"


def infer_direction(fn_name: str, message: str) -> str:
    fn = fn_name.lower()
    msg = message.lower()
    for d in DIRS:
        if d in fn:
            return d
    if "callers" in msg:
        return "callers"
    if "callees" in msg:
        return "callees"
    if "both" in msg:
        return "both"
    return "all"


def reason_key(message: str) -> str:
    m = message.lower()
    if "set dimpact_e2e_strict_lsp" in m:
        return "env-gate-disabled"
    if "not found" in m and ("server" in m or "-lsp" in m or "gopls" in m or "jdtls" in m):
        return "server-missing"
    if "changed_symbols unavailable" in m:
        return "changed-symbols-unavailable"
    if "callers impact unavailable" in m:
        return "callers-impact-unavailable"
    if "callees impact unavailable" in m:
        return "callees-impact-unavailable"
    if "both impact unavailable" in m:
        return "both-impact-unavailable"
    if "did not report callers" in m:
        return "callers-not-reported"
    if "did not report callees" in m:
        return "callees-not-reported"
    if "did not report both-direction" in m:
        return "both-not-reported"
    if "strict lsp unavailable" in m:
        return "strict-lsp-unavailable"
    return "other"

bucket = defaultdict(lambda: defaultdict(int))
samples = defaultdict(dict)

for e in entries:
    lang = infer_lang(e["fn"], e["message"])
    direction = infer_direction(e["fn"], e["message"])
    reason = reason_key(e["message"])
    key = f"{lang}/{direction}"
    bucket[key][reason] += 1
    samples[key].setdefault(reason, e["message"])

# classification for fail-fast promotion candidates
# Policy used for this report:
# - evaluate callers lane first
# - ignore env-gate-disabled/server-missing (operational prerequisites)
# - hold if no-*-reported or strict-lsp-unavailable is present
candidates = []
holds = []
for key in sorted(bucket.keys()):
    lang, direction = key.split("/", 1)
    reasons = bucket[key]
    has_reporting_gap = any(
        r in reasons
        for r in ["callers-not-reported", "callees-not-reported", "both-not-reported"]
    )
    has_unavailable = any(
        r in reasons
        for r in [
            "strict-lsp-unavailable",
            "changed-symbols-unavailable",
            "callers-impact-unavailable",
            "callees-impact-unavailable",
            "both-impact-unavailable",
        ]
    )

    if direction == "callers" and not has_reporting_gap and lang in {"go", "java", "python"}:
        candidates.append(
            {
                "lane": key,
                "reasons": dict(reasons),
                "note": "callers lane has no explicit report-gap skip marker in current tests",
            }
        )
    else:
        why = []
        if direction != "callers":
            why.append("prioritize callers for phase-1 fail-fast migration")
        if has_reporting_gap:
            why.append("contains not-reported skip marker")
        if has_unavailable:
            why.append("contains unavailable skip marker")
        holds.append(
            {
                "lane": key,
                "reasons": dict(reasons),
                "note": "; ".join(why) if why else "keep in hold set",
            }
        )

report = {
    "source": str(src),
    "totalSkipPrints": len(entries),
    "lanes": {k: dict(v) for k, v in sorted(bucket.items())},
    "samples": {k: v for k, v in sorted(samples.items())},
    "promotionCandidates": candidates,
    "holdCandidates": holds,
    "policy": {
        "phase": "PH65-1",
        "rule": "callers lane first; env/server prerequisites ignored for promotion screening",
    },
}

out_json.parent.mkdir(parents=True, exist_ok=True)
out_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

md = []
md.append("# strict real-LSP E2E skip reason matrix (PH65-1)")
md.append("")
md.append(f"source: `{src}`")
md.append(f"total skip prints: **{len(entries)}**")
md.append("")
md.append("## 1) language/direction skip aggregation")
md.append("")
md.append("| lane | reason | count |")
md.append("|---|---|---:|")
for lane in sorted(bucket.keys()):
    rs = bucket[lane]
    first = True
    for r in sorted(rs.keys()):
        if first:
            md.append(f"| {lane} | {r} | {rs[r]} |")
            first = False
        else:
            md.append(f"|  | {r} | {rs[r]} |")

md.append("")
md.append("## 2) promotion candidates (fail-fast migration candidates)")
md.append("")
if not candidates:
    md.append("- none")
else:
    for c in candidates:
        md.append(f"- `{c['lane']}`: {c['note']}")

md.append("")
md.append("## 3) hold candidates")
md.append("")
if not holds:
    md.append("- none")
else:
    for h in holds:
        md.append(f"- `{h['lane']}`: {h['note']}")

md.append("")
md.append("## 4) screening policy used")
md.append("")
md.append("- callers lane first (phase-1)")
md.append("- `env-gate-disabled` / `server-missing` are treated as operational prerequisites")
md.append("- lanes with `*-not-reported` or `*-unavailable` markers are kept in hold set")

out_md.parent.mkdir(parents=True, exist_ok=True)
out_md.write_text("\n".join(md) + "\n", encoding="utf-8")
print(f"wrote {out_json}")
print(f"wrote {out_md}")
PY
