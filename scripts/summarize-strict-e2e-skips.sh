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
        if "eprintln!(" in line:
            blob = line
            j = i
            while ");" not in blob and j < len(lines):
                j += 1
                blob += lines[j - 1]
            parts = re.findall(r'"([^"]*)"', blob)
            msg = " ".join(parts)
            if "skip:" in msg:
                entries.append({"fn": cur_fn, "line": i, "message": msg})
        if brace <= 0:
            cur_fn = None

LANGS = ["tsx", "typescript", "javascript", "ruby", "go", "java", "python", "rust"]
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
    if "rust-analyzer not available" in m:
        return "server-missing"
    if "not found" in m and (
        "server" in m
        or "-lsp" in m
        or "gopls" in m
        or "jdtls" in m
        or "rust-analyzer" in m
    ):
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

# F2 policy: keep only minimal, reasoned residuals.
# - operational prerequisites: env-gate-disabled / server-missing
# - actionable residual: anything else
OPERATIONAL_REASONS = {"env-gate-disabled", "server-missing"}

actionable = []
operational = []
for key in sorted(bucket.keys()):
    reasons = bucket[key]
    non_operational = {k: v for k, v in reasons.items() if k not in OPERATIONAL_REASONS}
    op_only = {k: v for k, v in reasons.items() if k in OPERATIONAL_REASONS}

    if non_operational:
        actionable.append(
            {
                "lane": key,
                "reasons": non_operational,
                "note": "contains non-operational skip reason(s); needs follow-up",
            }
        )
    elif op_only:
        note = []
        if "env-gate-disabled" in op_only:
            note.append("env gate opt-in")
        if "server-missing" in op_only:
            note.append("server missing on host")
        operational.append(
            {
                "lane": key,
                "reasons": op_only,
                "note": ", ".join(note),
            }
        )

report = {
    "source": str(src),
    "totalSkipPrints": len(entries),
    "lanes": {k: dict(v) for k, v in sorted(bucket.items())},
    "samples": {k: v for k, v in sorted(samples.items())},
    "actionableResidual": actionable,
    "operationalResidual": operational,
    "summary": {
        "actionableResidualLanes": len(actionable),
        "operationalResidualLanes": len(operational),
    },
    "policy": {
        "phase": "F2",
        "rule": "residual is minimal when only env-gate-disabled/server-missing remain",
    },
}

out_json.parent.mkdir(parents=True, exist_ok=True)
out_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

md = []
md.append("# strict real-LSP E2E skip-safe residual report (F2)")
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
md.append("## 2) actionable residual (non-operational)")
md.append("")
md.append(f"- lanes: **{len(actionable)}**")
if not actionable:
    md.append("- none (0)")
else:
    for a in actionable:
        md.append(f"- `{a['lane']}`: {a['note']}")

md.append("")
md.append("## 3) minimal residual with reasons (operational prerequisites)")
md.append("")
md.append(f"- lanes: **{len(operational)}**")
if not operational:
    md.append("- none")
else:
    for o in operational:
        md.append(f"- `{o['lane']}`: {o['note']}")

md.append("")
md.append("## 4) policy used")
md.append("")
md.append("- residual is acceptable when only `env-gate-disabled` / `server-missing` remain")
md.append("- any other reason is treated as actionable residual")

out_md.parent.mkdir(parents=True, exist_ok=True)
out_md.write_text("\n".join(md) + "\n", encoding="utf-8")
print(f"wrote {out_json}")
print(f"wrote {out_md}")
PY
