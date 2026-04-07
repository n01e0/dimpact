#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${1:-nightly-logs}"
OUT_MD="${2:-$LOG_DIR/nightly-failure-triage.md}"
CLASS_JSON="$LOG_DIR/nightly-flaky-classification.json"

python3 - "$LOG_DIR" "$CLASS_JSON" "$OUT_MD" <<'PY'
import json
import sys
from pathlib import Path

log_dir = Path(sys.argv[1])
class_json = Path(sys.argv[2])
out_md = Path(sys.argv[3])

CATEGORY_ORDER = ["install", "retry_absorbed", "startup", "logic", "capability", "timeout"]


def lang_norm(lang: str) -> str:
    return (lang or "unknown").strip().lower()


def gate_for_lang(lang: str) -> str:
    l = lang_norm(lang)
    if l in {"typescript", "typescript/javascript", "ts/js"}:
        return "DIMPACT_E2E_STRICT_LSP_TYPESCRIPT=1"
    if l in {"javascript"}:
        return "DIMPACT_E2E_STRICT_LSP_JAVASCRIPT=1"
    if l in {"python"}:
        return "DIMPACT_E2E_STRICT_LSP_PYTHON=1"
    if l in {"go"}:
        return "DIMPACT_E2E_STRICT_LSP_GO=1"
    if l in {"java"}:
        return "DIMPACT_E2E_STRICT_LSP_JAVA=1"
    if l in {"ruby"}:
        return "DIMPACT_E2E_STRICT_LSP_RUBY=1"
    if l in {"rust"}:
        return "DIMPACT_E2E_STRICT_LSP=1"
    return "DIMPACT_E2E_STRICT_LSP=1"


def repro_for(cat: str, lang: str) -> str:
    l = lang_norm(lang)
    if cat == "install":
        if l in {"typescript", "javascript", "typescript/javascript", "ts/js"}:
            return "npm install -g typescript typescript-language-server && typescript-language-server --help"
        if l == "python":
            return "npm install -g pyright && pyright-langserver --help"
        if l == "go":
            return "go install golang.org/x/tools/gopls@latest && gopls version"
        if l == "java":
            return "curl -fsSL <jdtls-tar.gz> -o /tmp/jdtls.tar.gz && jdtls --help"
        if l == "ruby":
            return "gem install ruby-lsp --no-document && ruby-lsp --help"
        return "rerun install/health-check step and inspect install-*.log"
    if cat == "startup":
        gate = gate_for_lang(lang)
        return f"{gate} cargo test -q --test engine_lsp"
    if cat == "retry_absorbed":
        if l == "ruby":
            return "gem install ruby-lsp --no-document && ruby-lsp --version && ruby-lsp --help"
        return "rerun workflow_dispatch and confirm retry-setup.log reports result=recovered"
    if cat == "logic":
        gate = gate_for_lang(lang)
        return f"env {gate} cargo test -q --test engine_lsp"
    if cat == "capability":
        gate = gate_for_lang(lang)
        return f"grep -n \"{gate.split('=')[0]}\" nightly-logs/engine-lsp-strict-preflight.log"
    if cat == "timeout":
        gate = gate_for_lang(lang)
        return f"timeout 900 env {gate} cargo test -q --test engine_lsp"
    return "inspect nightly logs and rerun workflow_dispatch"


def md_escape(s: str) -> str:
    return s.replace("|", "\\|")

if class_json.exists():
    data = json.loads(class_json.read_text(encoding="utf-8"))
else:
    data = {"totals": {}, "entries": {}}

entries = []
for cat in CATEGORY_ORDER:
    for e in data.get("entries", {}).get(cat, []):
        entries.append(
            {
                "category": cat,
                "language": e.get("language", "unknown"),
                "file": e.get("file", "?"),
                "line": e.get("line", 0),
                "snippet": e.get("snippet", ""),
            }
        )

# keep unique (category, language, snippet) and cap rows
seen = set()
rows = []
for e in entries:
    key = (e["category"], e["language"], e["snippet"])
    if key in seen:
        continue
    seen.add(key)
    rows.append(e)
rows = rows[:30]

md = []
md.append("## failure triage summary (cause / language / repro)")
md.append("")
md.append(f"log dir: `{log_dir}`")
md.append("")
md.append("| cause | language | evidence | repro step |")
md.append("|---|---|---|---|")

if not rows:
    md.append("| none | - | no classified flaky entries | rerun workflow_dispatch and inspect artifacts |")
else:
    for r in rows:
        evidence = f"`{r['file']}:{r['line']}` {r['snippet']}".strip()
        repro = repro_for(r["category"], r["language"])
        md.append(
            f"| {md_escape(r['category'])} | {md_escape(r['language'])} | {md_escape(evidence)} | `{md_escape(repro)}` |"
        )

md.append("")
md.append("### quick checklist")
md.append("1. Download artifact `nightly-strict-lsp-execution-logs`")
md.append("2. Verify flaky type totals in `nightly-flaky-classification.json`")
md.append("3. Execute the repro command above for the failing language/type")
md.append("4. Compare with `engine-lsp-strict-e2e.log` and retry logs")

out_md.parent.mkdir(parents=True, exist_ok=True)
out_md.write_text("\n".join(md) + "\n", encoding="utf-8")
print(f"wrote {out_md}")
PY
