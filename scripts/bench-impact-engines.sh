#!/usr/bin/env bash
set -euo pipefail

# Benchmark Tree-Sitter baseline vs secondary policy/engine path on the same diff input.
#
# Default comparison:
#   TS vs LSP(strict)
#
# Optional comparison:
#   TS vs Auto(strict-if-available)  (enable with --compare-auto-strict-if-available)
#
# Usage:
#   scripts/bench-impact-engines.sh [--base origin/main] [--diff-file /path/to.diff] [--runs 3] [--direction callers] [--lang rust] [--rpc-counts]
#                                  [--compare-auto-strict-if-available]
#                                  [--min-ts-changed N] [--min-ts-impacted N]
#                                  [--min-lsp-changed N] [--min-lsp-impacted N]
#                                  [--save-ts-json /path/to/ts.json] [--save-lsp-json /path/to/secondary.json]
#   scripts/bench-impact-engines.sh --summary-ts-json /path/to/ts.json --summary-lsp-json /path/to/secondary.json [--summary-second-label LABEL]
#
# Examples:
#   scripts/bench-impact-engines.sh --base origin/main --runs 5 --direction callers --lang rust
#   scripts/bench-impact-engines.sh --base origin/main --runs 3 --lang rust --compare-auto-strict-if-available
#   scripts/bench-impact-engines.sh --diff-file /tmp/dimpact-heavy.diff --runs 3 --lang rust
#   scripts/bench-impact-engines.sh --base origin/main --runs 1 --rpc-counts
#   scripts/bench-impact-engines.sh --diff-file /tmp/dimpact-heavy.diff --runs 1 --min-lsp-changed 40 --min-lsp-impacted 18
#   scripts/bench-impact-engines.sh --diff-file bench-fixtures/go-heavy.diff --runs 1 --direction callers --lang go
#   scripts/bench-impact-engines.sh --diff-file bench-fixtures/java-heavy.diff --runs 1 --direction callers --lang java
#   scripts/bench-impact-engines.sh --diff-file bench-fixtures/python-heavy.diff --runs 1 --direction callers --lang python
#   scripts/bench-impact-engines.sh --diff-file bench-fixtures/go-heavy.diff --save-ts-json /tmp/go-ts.json --save-lsp-json /tmp/go-secondary.json
#   scripts/bench-impact-engines.sh --summary-ts-json /tmp/ts.json --summary-lsp-json /tmp/secondary.json

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
SAVE_TS_JSON=""
SAVE_LSP_JSON=""
SUMMARY_TS_JSON=""
SUMMARY_LSP_JSON=""
SUMMARY_SECOND_LABEL="lsp(strict)"
COMPARE_AUTO_STRICT_IF_AVAILABLE=0

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

print_lang_summary() {
  local ts_json="$1"
  local second_json="$2"
  local second_label="$3"
  python3 - "$ts_json" "$second_json" "$second_label" <<'PY'
import json, sys
from collections import Counter
from pathlib import Path

ts = json.loads(Path(sys.argv[1]).read_text())
second = json.loads(Path(sys.argv[2]).read_text())
second_label = sys.argv[3]

def by_lang(symbols):
    c = Counter((s.get("language") or "unknown") for s in symbols)
    if not c:
        return "-"
    return ", ".join(f"{lang}:{c[lang]}" for lang in sorted(c))

print(f"ts changed_by_lang: {by_lang(ts.get('changed_symbols', []))}")
print(f"ts impacted_by_lang: {by_lang(ts.get('impacted_symbols', []))}")
print(f"{second_label} changed_by_lang: {by_lang(second.get('changed_symbols', []))}")
print(f"{second_label} impacted_by_lang: {by_lang(second.get('impacted_symbols', []))}")
PY
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
    --compare-auto-strict-if-available)
      COMPARE_AUTO_STRICT_IF_AVAILABLE=1
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
    --save-ts-json)
      SAVE_TS_JSON="${2:?missing value for --save-ts-json}"
      shift 2
      ;;
    --save-lsp-json)
      SAVE_LSP_JSON="${2:?missing value for --save-lsp-json}"
      shift 2
      ;;
    --summary-ts-json)
      SUMMARY_TS_JSON="${2:?missing value for --summary-ts-json}"
      shift 2
      ;;
    --summary-lsp-json)
      SUMMARY_LSP_JSON="${2:?missing value for --summary-lsp-json}"
      shift 2
      ;;
    --summary-second-label)
      SUMMARY_SECOND_LABEL="${2:?missing value for --summary-second-label}"
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

if [[ -n "$SUMMARY_TS_JSON" || -n "$SUMMARY_LSP_JSON" ]]; then
  if [[ -z "$SUMMARY_TS_JSON" || -z "$SUMMARY_LSP_JSON" ]]; then
    echo "--summary-ts-json and --summary-lsp-json must be used together" >&2
    exit 2
  fi
  if [[ ! -f "$SUMMARY_TS_JSON" ]]; then
    echo "--summary-ts-json not found: $SUMMARY_TS_JSON" >&2
    exit 2
  fi
  if [[ ! -f "$SUMMARY_LSP_JSON" ]]; then
    echo "--summary-lsp-json not found: $SUMMARY_LSP_JSON" >&2
    exit 2
  fi
  echo "[lang-summary]"
  print_lang_summary "$SUMMARY_TS_JSON" "$SUMMARY_LSP_JSON" "$SUMMARY_SECOND_LABEL"
  exit 0
fi

if [[ -z "$DIFF_INPUT" ]] && ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "run inside git repository (or pass --diff-file)" >&2
  exit 1
fi

if [[ -n "$DIFF_INPUT" && ! -f "$DIFF_INPUT" ]]; then
  echo "--diff-file not found: $DIFF_INPUT" >&2
  exit 2
fi

BIN="./target/release/dimpact"
if [[ ! -x "$BIN" ]]; then
  echo "building release binary..." >&2
  cargo build --release --locked -q
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

SECOND_ENGINE="lsp"
SECOND_ARGS="--engine-lsp-strict"
SECOND_LABEL="lsp(strict)"
SECOND_SECTION="lsp-strict"
SECOND_METRIC_PREFIX="lsp"
if [[ "$COMPARE_AUTO_STRICT_IF_AVAILABLE" -eq 1 ]]; then
  SECOND_ENGINE="auto"
  SECOND_ARGS="--auto-policy strict-if-available"
  SECOND_LABEL="auto(strict-if-available)"
  SECOND_SECTION="auto-strict-if-available"
  SECOND_METRIC_PREFIX="auto.strict-if-available"
fi

if [[ -n "$DIFF_INPUT" ]]; then
  echo "diff_file=$DIFF_INPUT runs=$RUNS direction=$DIRECTION lang=$LANG rpc_counts=$RPC_COUNTS compare=$SECOND_LABEL"
else
  echo "base=$BASE_REF runs=$RUNS direction=$DIRECTION lang=$LANG rpc_counts=$RPC_COUNTS compare=$SECOND_LABEL"
fi

ts_times="$(measure_engine ts)"
second_times="$(measure_engine "$SECOND_ENGINE" "$SECOND_ARGS")"

# one saved output each (for symbol counts)
cat "$DIFF_FILE" | "$BIN" impact --engine ts --direction "$DIRECTION" --lang "$LANG" -f json > "$TS_JSON"
cat "$DIFF_FILE" | "$BIN" impact --engine "$SECOND_ENGINE" $SECOND_ARGS --direction "$DIRECTION" --lang "$LANG" -f json > "$LSP_JSON"

echo "[ts]"
printf "%s" "$ts_times"
echo
printf "%s" "$ts_times" | summarize
echo
echo
echo "[$SECOND_SECTION]"
printf "%s" "$second_times"
echo
printf "%s" "$second_times" | summarize
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
echo "$SECOND_LABEL changed=$LSP_CHANGED impacted=$LSP_IMPACTED"

echo

echo "[lang-summary]"
print_lang_summary "$TS_JSON" "$LSP_JSON" "$SECOND_LABEL"

if [[ -n "$SAVE_TS_JSON" ]]; then
  cp "$TS_JSON" "$SAVE_TS_JSON"
fi
if [[ -n "$SAVE_LSP_JSON" ]]; then
  cp "$LSP_JSON" "$SAVE_LSP_JSON"
fi

THRESHOLD_FAILURES=()

check_min() {
  local name="$1" actual="$2" minv="$3"
  if [[ -z "$minv" ]]; then
    return 0
  fi
  if [[ "$actual" -lt "$minv" ]]; then
    THRESHOLD_FAILURES+=("$name actual=$actual min=$minv")
    echo "[threshold] FAIL $name actual=$actual min=$minv"
  else
    echo "[threshold] PASS $name actual=$actual min=$minv"
  fi
  return 0
}

echo
if [[ -n "$MIN_TS_CHANGED$MIN_TS_IMPACTED$MIN_LSP_CHANGED$MIN_LSP_IMPACTED" ]]; then
  echo "[threshold-check]"
fi

check_min "ts.changed" "$TS_CHANGED" "$MIN_TS_CHANGED"
check_min "ts.impacted" "$TS_IMPACTED" "$MIN_TS_IMPACTED"
check_min "$SECOND_METRIC_PREFIX.changed" "$LSP_CHANGED" "$MIN_LSP_CHANGED"
check_min "$SECOND_METRIC_PREFIX.impacted" "$LSP_IMPACTED" "$MIN_LSP_IMPACTED"

if [[ "${#THRESHOLD_FAILURES[@]}" -gt 0 ]]; then
  echo
  echo "[threshold-check] RESULT=FAIL count=${#THRESHOLD_FAILURES[@]}"

  echo "[threshold-fail-summary]"
  echo "metric actual min shortage"

  SHORTAGE_LIST=()
  for f in "${THRESHOLD_FAILURES[@]}"; do
    echo "  - $f"
    echo "::error::THRESHOLD FAILED: $f"

    if [[ "$f" =~ ^([^[:space:]]+)[[:space:]]actual=([0-9]+)[[:space:]]min=([0-9]+)$ ]]; then
      metric="${BASH_REMATCH[1]}"
      actual="${BASH_REMATCH[2]}"
      minv="${BASH_REMATCH[3]}"
      shortage=$((minv - actual))
      echo "$metric $actual $minv +$shortage"
      SHORTAGE_LIST+=("$metric:+$shortage")
    fi
  done

  if [[ "${#SHORTAGE_LIST[@]}" -gt 0 ]]; then
    echo "[threshold-fail-at-a-glance] missing=${SHORTAGE_LIST[*]}"
  fi

  exit 1
fi

if [[ -n "$MIN_TS_CHANGED$MIN_TS_IMPACTED$MIN_LSP_CHANGED$MIN_LSP_IMPACTED" ]]; then
  echo "[threshold-check] RESULT=PASS"
fi

if [[ "$RPC_COUNTS" -eq 1 ]]; then
  echo
  echo "[$SECOND_SECTION-rpc-counts]"
  RUST_LOG=debug "$BIN" impact --engine "$SECOND_ENGINE" $SECOND_ARGS --direction "$DIRECTION" --lang "$LANG" -f json < "$DIFF_FILE" >/dev/null 2>"$LSP_DEBUG_LOG" || true
  if command -v rg >/dev/null 2>&1; then
    rg -o "method=[^ ]+" "$LSP_DEBUG_LOG" | sort | uniq -c | sort -nr | sed 's/^ *//'
  else
    grep -o "method=[^ ]*" "$LSP_DEBUG_LOG" | sort | uniq -c | sort -nr | sed 's/^ *//'
  fi
fi
