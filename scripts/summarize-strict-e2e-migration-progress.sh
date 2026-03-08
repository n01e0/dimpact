#!/usr/bin/env bash
set -euo pipefail

SRC="${1:-tests/engine_lsp.rs}"
OUT_MD="${2:-}"

python3 - "$SRC" "$OUT_MD" <<'PY'
import re
import sys
from pathlib import Path

src = Path(sys.argv[1])
out_md = Path(sys.argv[2]) if sys.argv[2] else None

if not src.exists():
    raise SystemExit(f"source not found: {src}")

lines = src.read_text(encoding="utf-8", errors="replace").splitlines()

functions = []
cur_fn = None
buf = []
brace = 0

for line in lines:
    m = re.match(r"\s*fn\s+([A-Za-z0-9_]+)\s*\(", line)
    if cur_fn is None and m:
        cur_fn = m.group(1)
        buf = [line]
        brace = line.count("{") - line.count("}")
        if brace <= 0:
            functions.append((cur_fn, "\n".join(buf)))
            cur_fn = None
            buf = []
        continue

    if cur_fn is not None:
        buf.append(line)
        brace += line.count("{") - line.count("}")
        if brace <= 0:
            functions.append((cur_fn, "\n".join(buf)))
            cur_fn = None
            buf = []


def infer_lang(fn: str) -> str:
    f = fn.lower()
    for lang in ["tsx", "typescript", "javascript", "ruby", "go", "java", "python", "rust"]:
        if lang in f:
            return lang
    if "strict_callers_chain_is_stable" in f or "methods_chain" in f:
        return "rust"
    return "unknown"


def infer_direction(fn: str) -> str:
    f = fn.lower()
    for d in ["callers", "callees", "both"]:
        if d in f:
            return d
    return "all"


def lane_name(fn: str) -> str:
    return f"{infer_lang(fn)}/{infer_direction(fn)}"


def is_strict_lane_fn(fn: str) -> bool:
    f = fn.lower()
    return f.startswith("lsp_engine_strict_") and "when_available" in f

failfast_promoted = []
skip_safe_remaining = []

for fn, body in functions:
    if not is_strict_lane_fn(fn):
        continue

    lane = lane_name(fn)

    env_promoted = (
        "require_env_gate_callers_lane(" in body
        or "require_strict_lsp_env_gate_for_lane(" in body
    )
    server_promoted = any(
        k in body
        for k in [
            "require_typescript_lsp_server_for_lane(",
            "require_ruby_lsp_server_for_lane(",
            "require_gopls_for_lane(",
            "require_jdtls_for_lane(",
            "require_python_lsp_server_for_lane(",
        ]
    )

    if env_promoted or server_promoted:
        detail = []
        if env_promoted:
            detail.append("env-gate")
        if server_promoted:
            detail.append("server-preflight")
        failfast_promoted.append((lane, fn, "+".join(detail)))

    if "eprintln!(\"skip:" in body:
        reason = []
        if "set DIMPACT_E2E_STRICT_LSP" in body:
            reason.append("env-gate skip")
        if "not found" in body or "not available" in body:
            reason.append("server missing skip")
        if "unavailable in this env" in body:
            reason.append("impact unavailable skip")
        if "did not report" in body:
            reason.append("not-reported skip")
        if not reason:
            reason.append("other skip")
        skip_safe_remaining.append((lane, fn, ", ".join(sorted(set(reason)))))

# de-dup & stable order
failfast_promoted = sorted(set(failfast_promoted), key=lambda x: (x[0], x[1]))
skip_safe_remaining = sorted(set(skip_safe_remaining), key=lambda x: (x[0], x[1]))

out = []
out.append("## strict real-LSP migration snapshot")
out.append("")
out.append(f"source: `{src}`")
out.append("")
out.append("### fail-fast 昇格済み")
out.append("")
out.append(f"- lanes: **{len(failfast_promoted)}**")
if failfast_promoted:
    out.append("- details:")
    for lane, fn, kind in failfast_promoted:
        out.append(f"  - `{lane}` (`{fn}`): {kind}")
else:
    out.append("- none")

out.append("")
out.append("### skip-safe 残件")
out.append("")
out.append(f"- lanes: **{len(skip_safe_remaining)}**")
if skip_safe_remaining:
    out.append("- details:")
    for lane, fn, why in skip_safe_remaining:
        out.append(f"  - `{lane}` (`{fn}`): {why}")
else:
    out.append("- none")

text = "\n".join(out) + "\n"
if out_md is not None:
    out_md.parent.mkdir(parents=True, exist_ok=True)
    out_md.write_text(text, encoding="utf-8")
    print(f"wrote {out_md}")
else:
    print(text, end="")
PY
