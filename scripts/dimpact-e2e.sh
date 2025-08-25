#!/usr/bin/env bash
set -euo pipefail

# Usage: scripts/dimpact-e2e.sh [--format json|yaml]
# Runs `git diff --no-ext-diff` and pipes into dimpact.

FORMAT="json"
if [[ ${1-} == "--format" && -n ${2-} ]]; then
  FORMAT="$2"
fi

git diff --no-ext-diff | cargo run --quiet --bin dimpact -- --format "$FORMAT"

