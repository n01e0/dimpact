#!/usr/bin/env bash
set -euo pipefail

# Verify graduation checklist in docs/lsp-experimental-graduation.md
#
# Default: run full verification (includes cargo regression commands).
# Use --skip-regression to run static/config verification only.

RUN_REGRESSION=1
if [[ "${1:-}" == "--skip-regression" ]]; then
  RUN_REGRESSION=0
fi
if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'EOF'
Usage:
  scripts/verify-lsp-graduation.sh [--skip-regression]

Options:
  --skip-regression   Skip running cargo test/clippy commands and only run static checks.
EOF
  exit 0
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

DOC="docs/lsp-experimental-graduation.md"
ENGINE_LSP_TEST="tests/engine_lsp.rs"
CI_YML=".github/workflows/CI.yml"
BENCH_YML=".github/workflows/bench.yml"
BENCH_SCRIPT="scripts/bench-impact-engines.sh"
README_EN="README.md"
README_JA="README_ja.md"

PASS=0
FAIL=0

check_file() {
  local f="$1"
  if [[ ! -f "$f" ]]; then
    echo "[FAIL] missing file: $f"
    FAIL=$((FAIL+1))
    return 1
  fi
  return 0
}

check_contains() {
  local file="$1"
  local pattern="$2"
  local label="$3"
  if rg -n --fixed-strings "$pattern" "$file" >/dev/null; then
    echo "[PASS] $label"
    PASS=$((PASS+1))
  else
    echo "[FAIL] $label"
    FAIL=$((FAIL+1))
  fi
}

run_check() {
  local label="$1"
  shift
  if "$@"; then
    echo "[PASS] $label"
    PASS=$((PASS+1))
  else
    echo "[FAIL] $label"
    FAIL=$((FAIL+1))
  fi
}

echo "== verify: docs/lsp-experimental-graduation.md =="
check_file "$DOC" || true
check_file "$ENGINE_LSP_TEST" || true
check_file "$CI_YML" || true
check_file "$BENCH_YML" || true
check_file "$BENCH_SCRIPT" || true
check_file "$README_EN" || true
check_file "$README_JA" || true

echo
printf "== checklist-1: strict E2E 6 languages x 3 directions ==\n"
for lang in go java typescript javascript ruby python; do
  for dir in callers callees both; do
    fn="fn lsp_engine_strict_${lang}_${dir}_chain_e2e_when_available()"
    check_contains "$ENGINE_LSP_TEST" "$fn" "${lang}/${dir} strict real-LSP E2E exists"
  done
done

echo
printf "== checklist-2: integration regression green gate ==\n"
check_contains "$CI_YML" "cargo test -q --test engine_lsp" "CI has engine_lsp regression job"
check_contains "$CI_YML" "cargo test -q" "CI has cargo test job"
check_contains "$CI_YML" "cargo clippy" "CI has clippy in check path"

if [[ "$RUN_REGRESSION" -eq 1 ]]; then
  run_check "local cargo test -q --test engine_lsp" cargo test -q --test engine_lsp
  run_check "local cargo test -q" cargo test -q
  run_check "local cargo clippy -q --all-targets -- -D warnings" cargo clippy -q --all-targets -- -D warnings
else
  echo "[SKIP] local cargo regression commands (requested --skip-regression)"
fi

echo
printf "== checklist-3: bench artifact + threshold failure readability ==\n"
for lang in rust go java python typescript javascript ruby; do
  case "$lang" in
    rust)
      check_contains "$BENCH_YML" "bench-rust-ts.json" "bench artifact includes rust ts json"
      check_contains "$BENCH_YML" "bench-rust-auto-strict-if-available.json" "bench artifact includes rust secondary json"
      check_contains "$BENCH_YML" "bench-report.txt" "bench artifact includes rust txt report"
      ;;
    *)
      check_contains "$BENCH_YML" "bench-${lang}-ts.json" "bench artifact includes ${lang} ts json"
      check_contains "$BENCH_YML" "bench-${lang}-lsp.json" "bench artifact includes ${lang} lsp json"
      check_contains "$BENCH_YML" "bench-${lang}-report.txt" "bench artifact includes ${lang} txt report"
      ;;
  esac
done
check_contains "$BENCH_SCRIPT" "[threshold] FAIL" "threshold fail line is structured"
check_contains "$BENCH_SCRIPT" "[threshold-check] RESULT=FAIL" "threshold fail summary is printed"
check_contains "$BENCH_SCRIPT" "::error::THRESHOLD FAILED:" "GitHub error annotation is emitted"

echo
printf "== checklist-4: README / README_ja strict E2E sync ==\n"
for var in \
  DIMPACT_E2E_STRICT_LSP_TYPESCRIPT \
  DIMPACT_E2E_STRICT_LSP_JAVASCRIPT \
  DIMPACT_E2E_STRICT_LSP_RUBY \
  DIMPACT_E2E_STRICT_LSP_GO \
  DIMPACT_E2E_STRICT_LSP_JAVA \
  DIMPACT_E2E_STRICT_LSP_PYTHON; do
  check_contains "$README_EN" "$var" "README.md includes $var"
  check_contains "$README_JA" "$var" "README_ja.md includes $var"
done
check_contains "$README_EN" "strict real-LSP target languages" "README.md target language section exists"
check_contains "$README_JA" "strict real-LSP の対象言語" "README_ja.md target language section exists"

echo
printf "== summary ==\n"
printf "PASS=%d FAIL=%d\n" "$PASS" "$FAIL"

if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
