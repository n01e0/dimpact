#!/usr/bin/env bash
set -euo pipefail

# Benchmark Tree-Sitter vs LSP(strict) on the same diff input.
#
# Usage:
#   scripts/bench-impact-engines.sh [--base origin/main] [--runs 3] [--direction callers] [--lang rust]
#
# Example:
#   scripts/bench-impact-engines.sh --base origin/main --runs 5 --direction callers --lang rust

BASE_REF="origin/main"
RUNS=3
DIRECTION="callers"
LANG="rust"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base)
      BASE_REF="${2:?missing value for --base}"
      shift 2
      ;;
    --runs)
      RUNS="${2:?missing value for --runs}"
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
    -h|--help)
      sed -n '1,30p' "$0"
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 2
      ;;
  esac
done

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "run inside git repository" >&2
  exit 1
fi

if ! [[ "$RUNS" =~ ^[0-9]+$ ]] || [[ "$RUNS" -lt 1 ]]; then
  echo "--runs must be >= 1" >&2
  exit 2
fi

BIN="./target/release/dimpact"
if [[ ! -x "$BIN" ]]; then
  echo "building release binary..." >&2
  cargo build --release -q
fi

DIFF_FILE="$(mktemp)"
TS_JSON="$(mktemp)"
LSP_JSON="$(mktemp)"
trap 'rm -f "$DIFF_FILE" "$TS_JSON" "$LSP_JSON"' EXIT

# Ensure base exists locally
if [[ "$BASE_REF" == origin/* ]]; then
  git fetch origin "${BASE_REF#origin/}" >/dev/null 2>&1 || true
fi

git diff --no-ext-diff "${BASE_REF}"...HEAD > "$DIFF_FILE"

measure_engine() {
  local mode="$1"; shift
  local args="$*"
  local out=""
  for ((i=1; i<=RUNS; i++)); do
    local t
    t=$(/usr/bin/time -f "%e" bash -lc "cat \"$DIFF_FILE\" | \"$BIN\" impact --engine $mode $args --direction \"$DIRECTION\" --lang \"$LANG\" -f json >/dev/null" 2>&1)
    out+="$t"$'\n'
  done
  printf "%s" "$out"
}

summarize() {
  awk 'NR==1{min=$1;max=$1}{s+=$1;if($1<min)min=$1;if($1>max)max=$1} END{printf("avg=%.3fs min=%.3fs max=%.3fs", s/NR, min, max)}'
}

echo "base=$BASE_REF runs=$RUNS direction=$DIRECTION lang=$LANG"

ts_times="$(measure_engine ts)"
lsp_times="$(measure_engine lsp "--engine-lsp-strict")"

# one saved output each (for symbol counts)
cat "$DIFF_FILE" | "$BIN" impact --engine ts --direction "$DIRECTION" --lang "$LANG" -f json > "$TS_JSON"
cat "$DIFF_FILE" | "$BIN" impact --engine lsp --engine-lsp-strict --direction "$DIRECTION" --lang "$LANG" -f json > "$LSP_JSON"

echo "[ts]"
printf "%s" "$ts_times"
echo
printf "%s" "$ts_times" | summarize
echo
echo
echo "[lsp-strict]"
printf "%s" "$lsp_times"
echo
printf "%s" "$lsp_times" | summarize
echo
echo

python3 - "$TS_JSON" "$LSP_JSON" <<'PY'
import json, sys
from pathlib import Path

ts=json.loads(Path(sys.argv[1]).read_text())
lsp=json.loads(Path(sys.argv[2]).read_text())
print(f"ts changed={len(ts.get('changed_symbols',[]))} impacted={len(ts.get('impacted_symbols',[]))}")
print(f"lsp(strict) changed={len(lsp.get('changed_symbols',[]))} impacted={len(lsp.get('impacted_symbols',[]))}")
PY
