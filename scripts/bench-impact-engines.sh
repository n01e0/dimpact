#!/usr/bin/env bash
set -euo pipefail

# Benchmark Tree-Sitter vs LSP(strict) on the same diff input.
#
# Usage:
#   scripts/bench-impact-engines.sh [--base origin/main] [--diff-file /path/to.diff] [--runs 3] [--direction callers] [--lang rust] [--rpc-counts]
#                                  [--min-ts-changed N] [--min-ts-impacted N]
#                                  [--min-lsp-changed N] [--min-lsp-impacted N]
#
# Examples:
#   scripts/bench-impact-engines.sh --base origin/main --runs 5 --direction callers --lang rust
#   scripts/bench-impact-engines.sh --diff-file /tmp/dimpact-heavy.diff --runs 3 --lang rust
#   scripts/bench-impact-engines.sh --base origin/main --runs 1 --rpc-counts
#   scripts/bench-impact-engines.sh --diff-file /tmp/dimpact-heavy.diff --runs 1 --min-lsp-changed 40 --min-lsp-impacted 18
#   scripts/bench-impact-engines.sh --diff-file bench-fixtures/go-heavy.diff --runs 1 --direction callers --lang go
#   scripts/bench-impact-engines.sh --diff-file bench-fixtures/java-heavy.diff --runs 1 --direction callers --lang java

BASE_REF="origin/main"
DIFF_INPUT=""
RUNS=3
DIRECTION="callers"
LANG="rust"
RPC_COUNTS=0
MIN_TS_CHANGED=""
MIN_TS_IMPACTED=""
MIN_LSP_CHANGED=""
MIN_LSP_IMPACTED=""

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
    --rpc-counts)
      RPC_COUNTS=1
      shift
      ;;
    --min-ts-changed)
      MIN_TS_CHANGED="${2:?missing value for --min-ts-changed}"
      shift 2
      ;;
    --min-ts-impacted)
      MIN_TS_IMPACTED="${2:?missing value for --min-ts-impacted}"
      shift 2
      ;;
    --min-lsp-changed)
      MIN_LSP_CHANGED="${2:?missing value for --min-lsp-changed}"
      shift 2
      ;;
    --min-lsp-impacted)
      MIN_LSP_IMPACTED="${2:?missing value for --min-lsp-impacted}"
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

for v in "$MIN_TS_CHANGED" "$MIN_TS_IMPACTED" "$MIN_LSP_CHANGED" "$MIN_LSP_IMPACTED"; do
  if [[ -n "$v" && ! "$v" =~ ^[0-9]+$ ]]; then
    echo "expect values must be non-negative integers" >&2
    exit 2
  fi
done

if [[ -n "$DIFF_INPUT" && ! -f "$DIFF_INPUT" ]]; then
  echo "--diff-file not found: $DIFF_INPUT" >&2
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
LSP_DEBUG_LOG="$(mktemp)"
trap 'rm -f "$DIFF_FILE" "$TS_JSON" "$LSP_JSON" "$LSP_DEBUG_LOG"' EXIT

if [[ -n "$DIFF_INPUT" ]]; then
  cp "$DIFF_INPUT" "$DIFF_FILE"
else
  # Ensure base exists locally
  if [[ "$BASE_REF" == origin/* ]]; then
    git fetch origin "${BASE_REF#origin/}" >/dev/null 2>&1 || true
  fi
  git diff --no-ext-diff "${BASE_REF}"...HEAD > "$DIFF_FILE"
fi

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

if [[ -n "$DIFF_INPUT" ]]; then
  echo "diff_file=$DIFF_INPUT runs=$RUNS direction=$DIRECTION lang=$LANG rpc_counts=$RPC_COUNTS"
else
  echo "base=$BASE_REF runs=$RUNS direction=$DIRECTION lang=$LANG rpc_counts=$RPC_COUNTS"
fi

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

read -r TS_CHANGED TS_IMPACTED LSP_CHANGED LSP_IMPACTED < <(python3 - "$TS_JSON" "$LSP_JSON" <<'PY'
import json, sys
from pathlib import Path

ts=json.loads(Path(sys.argv[1]).read_text())
lsp=json.loads(Path(sys.argv[2]).read_text())
print(len(ts.get('changed_symbols',[])), len(ts.get('impacted_symbols',[])), len(lsp.get('changed_symbols',[])), len(lsp.get('impacted_symbols',[])))
PY
)

echo "ts changed=$TS_CHANGED impacted=$TS_IMPACTED"
echo "lsp(strict) changed=$LSP_CHANGED impacted=$LSP_IMPACTED"

echo

echo "[lang-summary]"
python3 - "$TS_JSON" "$LSP_JSON" <<'PY'
import json, sys
from collections import Counter
from pathlib import Path

ts = json.loads(Path(sys.argv[1]).read_text())
lsp = json.loads(Path(sys.argv[2]).read_text())

def by_lang(symbols):
    c = Counter((s.get("language") or "unknown") for s in symbols)
    if not c:
        return "-"
    return ", ".join(f"{lang}:{c[lang]}" for lang in sorted(c))

print(f"ts changed_by_lang: {by_lang(ts.get('changed_symbols', []))}")
print(f"ts impacted_by_lang: {by_lang(ts.get('impacted_symbols', []))}")
print(f"lsp(strict) changed_by_lang: {by_lang(lsp.get('changed_symbols', []))}")
print(f"lsp(strict) impacted_by_lang: {by_lang(lsp.get('impacted_symbols', []))}")
PY

check_min() {
  local name="$1" actual="$2" minv="$3"
  if [[ -n "$minv" && "$actual" -lt "$minv" ]]; then
    echo "THRESHOLD FAILED: $name actual=$actual min=$minv" >&2
    return 1
  fi
  return 0
}

check_min "ts.changed" "$TS_CHANGED" "$MIN_TS_CHANGED"
check_min "ts.impacted" "$TS_IMPACTED" "$MIN_TS_IMPACTED"
check_min "lsp.changed" "$LSP_CHANGED" "$MIN_LSP_CHANGED"
check_min "lsp.impacted" "$LSP_IMPACTED" "$MIN_LSP_IMPACTED"

if [[ "$RPC_COUNTS" -eq 1 ]]; then
  echo
  echo "[lsp-rpc-counts]"
  RUST_LOG=debug "$BIN" impact --engine lsp --engine-lsp-strict --direction "$DIRECTION" --lang "$LANG" -f json < "$DIFF_FILE" >/dev/null 2>"$LSP_DEBUG_LOG" || true
  if command -v rg >/dev/null 2>&1; then
    rg -o "method=[^ ]+" "$LSP_DEBUG_LOG" | sort | uniq -c | sort -nr | sed 's/^ *//'
  else
    grep -o "method=[^ ]*" "$LSP_DEBUG_LOG" | sort | uniq -c | sort -nr | sed 's/^ *//'
  fi
fi
