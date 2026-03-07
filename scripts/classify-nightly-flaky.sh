#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${1:-nightly-logs}"
OUT_JSON="${2:-$LOG_DIR/nightly-flaky-classification.json}"
OUT_MD="${3:-$LOG_DIR/nightly-flaky-classification.md}"

if [[ ! -d "$LOG_DIR" ]]; then
  echo "log directory not found: $LOG_DIR" >&2
  exit 2
fi

python3 - "$LOG_DIR" "$OUT_JSON" "$OUT_MD" <<'PY'
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

log_dir = Path(sys.argv[1])
out_json = Path(sys.argv[2])
out_md = Path(sys.argv[3])

LANG_HINTS = {
    "ts/js": "typescript/javascript",
    "typescript": "typescript",
    "javascript": "javascript",
    "python": "python",
    "go": "go",
    "java": "java",
    "ruby": "ruby",
    "rust": "rust",
}

FILE_LANG_HINTS = [
    ("install-ts-js", "typescript/javascript"),
    ("install-python", "python"),
    ("install-go", "go"),
    ("install-java", "java"),
    ("install-ruby", "ruby"),
    ("preflight", "multi"),
    ("engine-lsp-strict", "multi"),
    ("graduation", "multi"),
]

INSTALL_PATTERNS = [
    re.compile(r"install failed", re.I),
    re.compile(r"failed to download", re.I),
    re.compile(r"failed to extract", re.I),
    re.compile(r"not found in PATH after install", re.I),
    re.compile(r"binary not found after extraction", re.I),
]

STARTUP_PATTERNS = [
    re.compile(r"--help failed", re.I),
    re.compile(r"gopls version failed", re.I),
    re.compile(r"initialize timeout or invalid response", re.I),
    re.compile(r"failed to start", re.I),
]

CAPABILITY_PATTERNS = [
    re.compile(r"capability missing", re.I),
    re.compile(r"skip-safe", re.I),
    re.compile(r"DIMPACT_E2E_STRICT_LSP_[A-Z_]+=0 reason=", re.I),
]

TIMEOUT_PATTERNS = [
    re.compile(r"\btimeout\b", re.I),
    re.compile(r"timed out", re.I),
    re.compile(r"exit_code=124", re.I),
]

CATEGORY_ORDER = ["install", "startup", "capability", "timeout"]


def detect_lang_from_file(path: Path) -> str:
    name = path.name.lower()
    for needle, lang in FILE_LANG_HINTS:
        if needle in name:
            return lang
    return "unknown"


def detect_lang_from_line(line: str) -> str | None:
    m = re.search(r"\[([^\]]+)\]", line)
    if not m:
        return None
    token = m.group(1).strip().lower()
    return LANG_HINTS.get(token)


def category_hits(line: str):
    hits = []
    for p in INSTALL_PATTERNS:
        if p.search(line):
            hits.append("install")
            break
    for p in STARTUP_PATTERNS:
        if p.search(line):
            hits.append("startup")
            break
    for p in CAPABILITY_PATTERNS:
        if p.search(line):
            hits.append("capability")
            break
    for p in TIMEOUT_PATTERNS:
        if p.search(line):
            hits.append("timeout")
            break
    # classify initialize-timeout primarily as startup + timeout
    if "initialize timeout or invalid response" in line.lower():
        if "startup" not in hits:
            hits.append("startup")
        if "timeout" not in hits:
            hits.append("timeout")
    return hits

entries_by_cat = defaultdict(list)
seen = set()

for path in sorted(log_dir.glob("*.log")):
    file_lang = detect_lang_from_file(path)
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except Exception:
        continue

    for i, line in enumerate(lines, start=1):
        cats = category_hits(line)
        if not cats:
            continue
        line_lang = detect_lang_from_line(line)
        lang = line_lang or file_lang
        snippet = line.strip()
        if len(snippet) > 220:
            snippet = snippet[:217] + "..."
        for cat in cats:
            key = (cat, path.name, i, snippet)
            if key in seen:
                continue
            seen.add(key)
            entries_by_cat[cat].append(
                {
                    "category": cat,
                    "language": lang,
                    "file": path.name,
                    "line": i,
                    "snippet": snippet,
                }
            )

summary = {
    "logDir": str(log_dir),
    "totals": {cat: len(entries_by_cat.get(cat, [])) for cat in CATEGORY_ORDER},
    "entries": {cat: entries_by_cat.get(cat, []) for cat in CATEGORY_ORDER},
}

out_json.parent.mkdir(parents=True, exist_ok=True)
out_json.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

md = []
md.append("## nightly flaky auto classification")
md.append("")
md.append(f"log dir: `{log_dir}`")
md.append("")
md.append("| category | count |")
md.append("|---|---:|")
for cat in CATEGORY_ORDER:
    md.append(f"| {cat} | {summary['totals'][cat]} |")

for cat in CATEGORY_ORDER:
    md.append("")
    md.append(f"### {cat}")
    rows = entries_by_cat.get(cat, [])
    if not rows:
        md.append("- none")
        continue
    md.append("| language | file:line | snippet |")
    md.append("|---|---|---|")
    for e in rows[:40]:
        snippet = e["snippet"].replace("|", "\\|")
        md.append(f"| {e['language']} | `{e['file']}:{e['line']}` | {snippet} |")
    if len(rows) > 40:
        md.append(f"| ... | ... | and {len(rows)-40} more |")

out_md.parent.mkdir(parents=True, exist_ok=True)
out_md.write_text("\n".join(md) + "\n", encoding="utf-8")
print(f"wrote {out_json}")
print(f"wrote {out_md}")
PY
