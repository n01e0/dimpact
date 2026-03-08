#!/usr/bin/env bash
set -euo pipefail

SRC="${1:-tests/engine_lsp.rs}"
OUT_JSON="${2:-docs/strict-real-lsp-skip-safe-inventory-a1.json}"
OUT_MD="${3:-docs/strict-real-lsp-skip-safe-inventory-a1.md}"

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
            msg = " ".join(re.findall(r'"([^"]*)"', blob))
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
    if "not found" in m and ("server" in m or "-lsp" in m or "gopls" in m or "jdtls" in m or "rust-analyzer" in m):
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
    lane = f"{lang}/{direction}"
    bucket[lane][reason] += 1
    samples[lane].setdefault(reason, e["message"])

by_lang = defaultdict(int)
for lane, reasons in bucket.items():
    lang = lane.split("/", 1)[0]
    by_lang[lang] += sum(reasons.values())

report = {
    "source": str(src),
    "totalSkipPrints": len(entries),
    "lanes": {k: dict(v) for k, v in sorted(bucket.items())},
    "samples": {k: v for k, v in sorted(samples.items())},
    "byLanguageTotal": dict(sorted(by_lang.items())),
    "note": "A1 inventory: current skip-safe lanes (language x direction x reason)",
}

out_json.parent.mkdir(parents=True, exist_ok=True)
out_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

md = []
md.append("# strict real-LSP skip-safe lane inventory (A1)")
md.append("")
md.append(f"source: `{src}`")
md.append(f"total skip prints: **{len(entries)}**")
md.append("")
md.append("## 1) language × direction × reason")
md.append("")
md.append("| language | direction | reason | count | sample |")
md.append("|---|---|---|---:|---|")
for lane in sorted(bucket.keys()):
    lang, direction = lane.split("/", 1)
    reasons = bucket[lane]
    first = True
    for reason in sorted(reasons.keys()):
        count = reasons[reason]
        sample = samples[lane].get(reason, "").replace("|", "\\|")
        if len(sample) > 120:
            sample = sample[:117] + "..."
        if first:
            md.append(f"| {lang} | {direction} | {reason} | {count} | {sample} |")
            first = False
        else:
            md.append(f"|  |  | {reason} | {count} | {sample} |")

md.append("")
md.append("## 2) totals by language")
md.append("")
md.append("| language | skip prints |")
md.append("|---|---:|")
for lang in sorted(by_lang.keys()):
    md.append(f"| {lang} | {by_lang[lang]} |")

md.append("")
md.append("## 3) remarks")
md.append("")
md.append("- This inventory is generated from current `skip:` prints in `tests/engine_lsp.rs`.")
md.append("- Lanes without skip markers may already be fail-fast migrated or not covered by skip-safe code paths.")

out_md.parent.mkdir(parents=True, exist_ok=True)
out_md.write_text("\n".join(md) + "\n", encoding="utf-8")
print(f"wrote {out_json}")
print(f"wrote {out_md}")
PY
