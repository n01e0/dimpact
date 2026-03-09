#!/usr/bin/env bash
set -euo pipefail

# Compare impact output against strict LSP oracle output on the same diff.
#
# Default comparison target:
#   candidate: Tree-Sitter engine (`--engine ts`)
#   oracle:    strict LSP (`--engine lsp --engine-lsp-strict`)
#
# Usage:
#   scripts/compare-impact-vs-lsp-oracle.sh [--base origin/main] [--diff-file /path/to.diff]
#       [--candidate-engine ts|auto|lsp] [--direction callers|callees|both] [--lang auto|rust|...]
#       [--max-depth N] [--with-edges] [--with-pdg] [--with-propagation]
#       [--save-candidate-json /tmp/candidate.json] [--save-oracle-json /tmp/oracle.json]
#       [--report-json /tmp/report.json] [--fail-on-diff]

BASE_REF="origin/main"
DIFF_INPUT=""
CANDIDATE_ENGINE="ts"
DIRECTION="callers"
LANG="auto"
MAX_DEPTH=""
WITH_EDGES=0
WITH_PDG=0
WITH_PROPAGATION=0
SAVE_CANDIDATE_JSON=""
SAVE_ORACLE_JSON=""
REPORT_JSON=""
FAIL_ON_DIFF=0

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

usage() {
  sed -n '1,26p' "$0"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base)
      BASE_REF="${2:?missing value for --base}"
      shift 2
      ;;
    --diff-file)
      DIFF_INPUT="${2:?missing value for --diff-file}"
      shift 2
      ;;
    --candidate-engine)
      CANDIDATE_ENGINE="${2:?missing value for --candidate-engine}"
      shift 2
      ;;
    --direction)
      DIRECTION="${2:?missing value for --direction}"
      shift 2
      ;;
    --lang)
      LANG="${2:?missing value for --lang}"
      shift 2
      ;;
    --max-depth)
      MAX_DEPTH="${2:?missing value for --max-depth}"
      shift 2
      ;;
    --with-edges)
      WITH_EDGES=1
      shift
      ;;
    --with-pdg)
      WITH_PDG=1
      shift
      ;;
    --with-propagation)
      WITH_PROPAGATION=1
      shift
      ;;
    --save-candidate-json)
      SAVE_CANDIDATE_JSON="${2:?missing value for --save-candidate-json}"
      shift 2
      ;;
    --save-oracle-json)
      SAVE_ORACLE_JSON="${2:?missing value for --save-oracle-json}"
      shift 2
      ;;
    --report-json)
      REPORT_JSON="${2:?missing value for --report-json}"
      shift 2
      ;;
    --fail-on-diff)
      FAIL_ON_DIFF=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$DIFF_INPUT" ]] && ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "run inside git repository (or pass --diff-file)" >&2
  exit 1
fi
if [[ -n "$DIFF_INPUT" && ! -f "$DIFF_INPUT" ]]; then
  echo "--diff-file not found: $DIFF_INPUT" >&2
  exit 2
fi
if [[ -n "$MAX_DEPTH" && ! "$MAX_DEPTH" =~ ^[0-9]+$ ]]; then
  echo "--max-depth must be a non-negative integer" >&2
  exit 2
fi

BIN="./target/debug/dimpact"
if [[ ! -x "$BIN" ]]; then
  echo "building debug binary..." >&2
  cargo build -q --bin dimpact
fi

DIFF_FILE="$(mktemp)"
CANDIDATE_JSON="$(mktemp)"
ORACLE_JSON="$(mktemp)"
REPORT_TMP="$(mktemp)"
trap 'rm -f "$DIFF_FILE" "$CANDIDATE_JSON" "$ORACLE_JSON" "$REPORT_TMP"' EXIT

if [[ -n "$DIFF_INPUT" ]]; then
  cp "$DIFF_INPUT" "$DIFF_FILE"
else
  if [[ "$BASE_REF" == origin/* ]]; then
    git fetch origin "${BASE_REF#origin/}" >/dev/null 2>&1 || true
  fi
  git diff --no-ext-diff "${BASE_REF}"...HEAD > "$DIFF_FILE"
fi

candidate_args=(impact --engine "$CANDIDATE_ENGINE" --direction "$DIRECTION" --lang "$LANG" --format json)
oracle_args=(impact --engine lsp --engine-lsp-strict --direction "$DIRECTION" --lang "$LANG" --format json)

if [[ -n "$MAX_DEPTH" ]]; then
  candidate_args+=(--max-depth "$MAX_DEPTH")
  oracle_args+=(--max-depth "$MAX_DEPTH")
fi
if [[ "$WITH_EDGES" -eq 1 ]]; then
  candidate_args+=(--with-edges)
  oracle_args+=(--with-edges)
fi
if [[ "$WITH_PDG" -eq 1 ]]; then
  candidate_args+=(--with-pdg)
  oracle_args+=(--with-pdg)
fi
if [[ "$WITH_PROPAGATION" -eq 1 ]]; then
  candidate_args+=(--with-propagation)
  oracle_args+=(--with-propagation)
fi

"$BIN" "${candidate_args[@]}" < "$DIFF_FILE" > "$CANDIDATE_JSON"
"$BIN" "${oracle_args[@]}" < "$DIFF_FILE" > "$ORACLE_JSON"

python3 - "$CANDIDATE_JSON" "$ORACLE_JSON" "$REPORT_TMP" <<'PY'
import json
import sys
from pathlib import Path

candidate = json.loads(Path(sys.argv[1]).read_text())
oracle = json.loads(Path(sys.argv[2]).read_text())
out_path = Path(sys.argv[3])

def symbol_key(s: dict) -> str:
    sid = s.get("id", {})
    if isinstance(sid, dict):
        v = sid.get("0")
        if isinstance(v, str) and v:
            return v
    if isinstance(sid, str) and sid:
        return sid
    return f"{s.get('language','?')}:{s.get('file','?')}:{s.get('kind','?')}:{s.get('name','?')}:{s.get('range',{}).get('start_line',0)}"

def edge_key(e: dict) -> str:
    fr = e.get("from", {})
    to = e.get("to", {})
    fr_id = fr.get("0") if isinstance(fr, dict) else fr
    to_id = to.get("0") if isinstance(to, dict) else to
    return f"{fr_id} -> {to_id}"

cand_changed = {symbol_key(s) for s in candidate.get("changed_symbols", [])}
orc_changed = {symbol_key(s) for s in oracle.get("changed_symbols", [])}
cand_imp = {symbol_key(s) for s in candidate.get("impacted_symbols", [])}
orc_imp = {symbol_key(s) for s in oracle.get("impacted_symbols", [])}
cand_edges = {edge_key(e) for e in candidate.get("edges", [])}
orc_edges = {edge_key(e) for e in oracle.get("edges", [])}

report = {
    "summary": {
        "candidate": {
            "changed": len(cand_changed),
            "impacted": len(cand_imp),
            "edges": len(cand_edges),
        },
        "oracle": {
            "changed": len(orc_changed),
            "impacted": len(orc_imp),
            "edges": len(orc_edges),
        },
    },
    "diff": {
        "changed": {
            "missing_vs_oracle": sorted(orc_changed - cand_changed),
            "extra_vs_oracle": sorted(cand_changed - orc_changed),
        },
        "impacted": {
            "missing_vs_oracle": sorted(orc_imp - cand_imp),
            "extra_vs_oracle": sorted(cand_imp - orc_imp),
        },
        "edges": {
            "missing_vs_oracle": sorted(orc_edges - cand_edges),
            "extra_vs_oracle": sorted(cand_edges - orc_edges),
        },
    },
}
out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
PY

if [[ -n "$SAVE_CANDIDATE_JSON" ]]; then
  cp "$CANDIDATE_JSON" "$SAVE_CANDIDATE_JSON"
fi
if [[ -n "$SAVE_ORACLE_JSON" ]]; then
  cp "$ORACLE_JSON" "$SAVE_ORACLE_JSON"
fi
if [[ -n "$REPORT_JSON" ]]; then
  cp "$REPORT_TMP" "$REPORT_JSON"
fi

python3 - "$REPORT_TMP" <<'PY'
import json
import sys
from pathlib import Path

r = json.loads(Path(sys.argv[1]).read_text())

c = r["summary"]["candidate"]
o = r["summary"]["oracle"]
d = r["diff"]

print("[summary]")
print(f"candidate changed={c['changed']} impacted={c['impacted']} edges={c['edges']}")
print(f"oracle    changed={o['changed']} impacted={o['impacted']} edges={o['edges']}")

for bucket in ["changed", "impacted", "edges"]:
    miss = d[bucket]["missing_vs_oracle"]
    extra = d[bucket]["extra_vs_oracle"]
    print(f"[diff:{bucket}] missing={len(miss)} extra={len(extra)}")
    for item in miss[:10]:
        print(f"  - missing: {item}")
    for item in extra[:10]:
        print(f"  - extra: {item}")
    if len(miss) > 10:
        print(f"  ... and {len(miss)-10} more missing")
    if len(extra) > 10:
        print(f"  ... and {len(extra)-10} more extra")
PY

if [[ "$FAIL_ON_DIFF" -eq 1 ]]; then
  python3 - "$REPORT_TMP" <<'PY'
import json
import sys
from pathlib import Path

r = json.loads(Path(sys.argv[1]).read_text())
d = r["diff"]
count = 0
for k in ["changed", "impacted", "edges"]:
    count += len(d[k]["missing_vs_oracle"]) + len(d[k]["extra_vs_oracle"])
if count > 0:
    raise SystemExit(1)
PY
fi
