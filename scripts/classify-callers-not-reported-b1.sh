#!/usr/bin/env bash
set -euo pipefail

SRC="${1:-tests/engine_lsp.rs}"
OUT_JSON="${2:-docs/strict-real-lsp-callers-not-reported-b1.json}"
OUT_MD="${3:-docs/strict-real-lsp-callers-not-reported-b1.md}"

python3 - "$SRC" "$OUT_JSON" "$OUT_MD" <<'PY'
import json
import re
import sys
from pathlib import Path

src = Path(sys.argv[1])
out_json = Path(sys.argv[2])
out_md = Path(sys.argv[3])

if not src.exists():
    raise SystemExit(f"source not found: {src}")

text = src.read_text(encoding="utf-8", errors="replace")
lines = text.splitlines()

# collect function bodies
functions = {}
cur = None
buf = []
brace = 0
for line in lines:
    m = re.match(r"\s*fn\s+([A-Za-z0-9_]+)\s*\(", line)
    if cur is None and m:
        cur = m.group(1)
        buf = [line]
        brace = line.count("{") - line.count("}")
        if brace <= 0:
            functions[cur] = "\n".join(buf)
            cur = None
            buf = []
        continue
    if cur is not None:
        buf.append(line)
        brace += line.count("{") - line.count("}")
        if brace <= 0:
            functions[cur] = "\n".join(buf)
            cur = None
            buf = []

lanes = {
    "tsx": ["lsp_engine_strict_tsx_callers_chain_e2e_when_available"],
    "typescript": ["lsp_engine_strict_typescript_callers_chain_e2e_when_available"],
    "javascript": ["lsp_engine_strict_javascript_callers_chain_e2e_when_available"],
    "ruby": ["lsp_engine_strict_ruby_callers_chain_e2e_when_available"],
    "go": ["lsp_engine_strict_go_callers_chain_e2e_when_available"],
    "java": ["lsp_engine_strict_java_callers_chain_e2e_when_available"],
    "python": ["lsp_engine_strict_python_callers_chain_e2e_when_available"],
    "rust": [
        "lsp_engine_strict_callers_chain_is_stable_when_available",
        "lsp_engine_strict_methods_chain_resolves_callers_when_available",
    ],
}

next_task = {
    "tsx": "B2",
    "typescript": "B2",
    "javascript": "B3",
    "ruby": "B4",
    "go": "B5",
    "java": "B6",
    "python": "B7",
    "rust": "B8",
}


def classify(body: str):
    markers = {
        "env_gate_failfast_helper": (
            "require_env_gate_callers_lane(" in body
            or "require_strict_lsp_env_gate_for_lane(" in body
        ),
        "server_preflight_failfast": any(
            k in body
            for k in [
                "require_typescript_lsp_server_for_lane(",
                "require_ruby_lsp_server_for_lane(",
                "require_gopls_for_lane(",
                "require_jdtls_for_lane(",
                "require_python_lsp_server_for_lane(",
            ]
        ),
        "skip_callers_not_reported": "did not report callers" in body,
        "skip_changed_symbols_unavailable": "changed_symbols unavailable in this env" in body,
        "skip_callers_impact_unavailable": "callers impact unavailable in this env" in body,
        "skip_strict_lsp_unavailable": "strict LSP unavailable in this environment" in body,
        "skip_rust_analyzer_missing": "rust-analyzer not available" in body,
        "failfast_assert_expected_caller": "cause=logic expected impacted caller 'foo'" in body,
    }

    if markers["skip_callers_not_reported"]:
        status = "active-callers-not-reported"
        cause = "LSP callers result empty in current env (paired with unavailable markers)"
    elif markers["failfast_assert_expected_caller"]:
        status = "promoted-failfast-logic"
        cause = "not-reported skip removed; callers emptiness is treated as logic failure"
    elif markers["skip_strict_lsp_unavailable"] or markers["skip_rust_analyzer_missing"]:
        status = "env-server-dominated"
        cause = "no callers-not-reported marker; env/server availability dominates failure path"
    else:
        status = "unknown"
        cause = "needs manual inspection"

    return status, cause, markers

items = []
for lang, fn_list in lanes.items():
    merged = "\n".join(functions.get(fn, "") for fn in fn_list)
    status, cause, markers = classify(merged)
    items.append(
        {
            "language": lang,
            "functions": fn_list,
            "status": status,
            "causeSummary": cause,
            "markers": markers,
            "recommendedNextTask": next_task[lang],
        }
    )

active = [i for i in items if i["status"] == "active-callers-not-reported"]
promoted = [i for i in items if i["status"] == "promoted-failfast-logic"]
other = [i for i in items if i["status"] not in {"active-callers-not-reported", "promoted-failfast-logic"}]

report = {
    "source": str(src),
    "phase": "B1",
    "summary": {
        "languages": len(items),
        "activeCallersNotReported": len(active),
        "promotedFailfastLogic": len(promoted),
        "other": len(other),
    },
    "items": items,
}

out_json.parent.mkdir(parents=True, exist_ok=True)
out_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

md = []
md.append("# strict real-LSP callers-not-reported classification (B1)")
md.append("")
md.append(f"source: `{src}`")
md.append("")
md.append("## summary")
md.append("")
md.append(f"- languages: **{len(items)}** (TS/JS/Ruby/Go/Java/Python/Rust/TSX)")
md.append(f"- active callers-not-reported: **{len(active)}**")
md.append(f"- promoted to fail-fast logic: **{len(promoted)}**")
md.append(f"- other: **{len(other)}**")
md.append("")
md.append("## language classification")
md.append("")
md.append("| language | status | cause summary | next task |")
md.append("|---|---|---|---|")
for i in items:
    md.append(
        f"| {i['language']} | {i['status']} | {i['causeSummary']} | {i['recommendedNextTask']} |"
    )

md.append("")
md.append("## marker details")
md.append("")
for i in items:
    md.append(f"### {i['language']}")
    md.append(f"- functions: {', '.join('`'+f+'`' for f in i['functions'])}")
    md.append(f"- status: `{i['status']}`")
    md.append(f"- cause: {i['causeSummary']}")
    md.append(f"- next: `{i['recommendedNextTask']}`")
    md.append("- markers:")
    for k, v in sorted(i["markers"].items()):
        md.append(f"  - `{k}`: `{str(v).lower()}`")
    md.append("")

out_md.parent.mkdir(parents=True, exist_ok=True)
out_md.write_text("\n".join(md), encoding="utf-8")

print(f"wrote {out_json}")
print(f"wrote {out_md}")
PY
