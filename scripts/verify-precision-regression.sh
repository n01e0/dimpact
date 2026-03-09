#!/usr/bin/env bash
set -euo pipefail

FN_THRESHOLD="${DIMPACT_PRECISION_FN_MAX:-0}"
FP_THRESHOLD="${DIMPACT_PRECISION_FP_MAX:-0}"
REPORT_PATH="precision-regression-report.json"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fn-threshold)
      FN_THRESHOLD="$2"
      shift 2
      ;;
    --fp-threshold)
      FP_THRESHOLD="$2"
      shift 2
      ;;
    --report)
      REPORT_PATH="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

python3 - "$FN_THRESHOLD" "$FP_THRESHOLD" "$REPORT_PATH" <<'PY'
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

fn_threshold = int(sys.argv[1])
fp_threshold = int(sys.argv[2])
report_path = Path(sys.argv[3])
repo = Path.cwd()

def run(cmd, cwd=None, input_text=None):
    return subprocess.run(
        cmd,
        cwd=cwd,
        input=input_text,
        text=True,
        capture_output=True,
        check=True,
    )

def git(cwd, *args, input_text=None):
    return run(["git", *args], cwd=cwd, input_text=input_text)

bin_path = repo / "target" / "debug" / "dimpact"
if not bin_path.exists():
    run(["cargo", "build", "-q", "--bin", "dimpact"], cwd=repo)

cases = [
    {
        "name": "typescript-hard-v73",
        "lang": "typescript",
        "file": "demo/a.ts",
        "fixture": "tests/fixtures/typescript/analyzer_hard_cases_dispatch_overload_optional_chain.ts",
        "replace": [
            (
                'return typeof v === "number" ? v : Number.parseInt(v, 10);',
                'return typeof v === "number" ? v : Number.parseInt(v, 10) + 1;',
                -1,
            )
        ],
        "expected_changed": {"parse"},
        "expected_impacted": {"run"},
    },
    {
        "name": "tsx-hard-v73",
        "lang": "tsx",
        "file": "demo/a.tsx",
        "fixture": "tests/fixtures/tsx/analyzer_hard_cases_component_callback_optional_chain.tsx",
        "replace": [
            (
                "return <section>{handle(props.item)}</section>;",
                "return <section>{handle(props.item)}!</section>;",
                -1,
            )
        ],
        "expected_changed": {"Panel"},
        "expected_impacted": set(),
    },
    {
        "name": "rust-hard-v73",
        "lang": "rust",
        "file": "demo/a.rs",
        "fixture": "tests/fixtures/rust/analyzer_hard_cases_trait_dispatch_method_value_generic.rs",
        "replace": [("self.worker.handle(first)", "self.worker.handle(first.clone())", -1)],
        "expected_changed": {"run"},
        "expected_impacted": set(),
    },
    {
        "name": "java-hard-v73",
        "lang": "java",
        "file": "demo/A.java",
        "fixture": "tests/fixtures/java/analyzer_hard_cases_lambda_methodref_overload.java",
        "replace": [
            (
                "return Integer.parseInt(s);",
                "return Integer.parseInt(s) + 1;",
                1,
            )
        ],
        "expected_changed": {"OverloadLab", "parse"},
        "expected_impacted": {"parse", "run"},
    },
    {
        "name": "go-hard-v73",
        "lang": "go",
        "file": "demo/a.go",
        "fixture": "tests/fixtures/go/analyzer_hard_cases_interface_dispatch_method_value_generic_receiver.go",
        "replace": [
            (
                "return b.inner.Handle(context.Background())",
                "return b.inner.Handle(context.Background()) // tweak",
                -1,
            )
        ],
        "expected_changed": {"Run"},
        "expected_impacted": set(),
    },
    {
        "name": "ruby-hard-v79",
        "lang": "ruby",
        "file": "demo/a.rb",
        "fixture": "tests/fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb",
        "replace": [(":ok", ":ok2", 1)],
        "expected_changed": {"DynamicDispatch", "target_sym"},
        "expected_impacted": {"execute"},
    },
    {
        "name": "python-hard-v79",
        "lang": "python",
        "file": "demo/a.py",
        "fixture": "tests/fixtures/python/analyzer_hard_cases_dynamic_getattr_setattr_getattribute.py",
        "replace": [("payload.strip()", "payload.rstrip()", -1)],
        "expected_changed": {"DynamicAccessor", "__getattr__"},
        "expected_impacted": {"__init__", "execute"},
    },
]

def run_case(case):
    before = (repo / case["fixture"]).read_text(encoding="utf-8")
    after = before
    for old, new, count in case["replace"]:
        if count == 1:
            after = after.replace(old, new, 1)
        else:
            after = after.replace(old, new)

    with tempfile.TemporaryDirectory() as td:
        d = Path(td)
        git(d, "init", "-q")
        git(d, "config", "user.email", "tester@example.com")
        git(d, "config", "user.name", "Tester")

        p = d / case["file"]
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(before, encoding="utf-8")
        git(d, "add", ".")
        git(d, "commit", "-m", "init", "-q")

        p.write_text(after, encoding="utf-8")

        diff = git(d, "diff", "--no-ext-diff", "--unified=0").stdout
        changed = run(
            [
                str(bin_path),
                "--mode",
                "changed",
                "--lang",
                case["lang"],
                "--format",
                "json",
            ],
            cwd=d,
            input_text=diff,
        )
        ch = json.loads(changed.stdout)
        changed_names = {
            s.get("name")
            for s in ch.get("changed_symbols", [])
            if isinstance(s, dict) and s.get("name")
        }

        diff2 = git(d, "diff", "--no-ext-diff", "--unified=0").stdout
        impacted = run(
            [
                str(bin_path),
                "--mode",
                "impact",
                "--direction",
                "callers",
                "--lang",
                case["lang"],
                "--format",
                "json",
            ],
            cwd=d,
            input_text=diff2,
        )
        im = json.loads(impacted.stdout)
        impacted_names = {
            s.get("name")
            for s in im.get("impacted_symbols", [])
            if isinstance(s, dict) and s.get("name")
        }

    exp_changed = set(case["expected_changed"])
    exp_impacted = set(case["expected_impacted"])

    fn_changed = sorted(exp_changed - changed_names)
    fp_changed = sorted(changed_names - exp_changed)
    fn_impacted = sorted(exp_impacted - impacted_names)
    fp_impacted = sorted(impacted_names - exp_impacted)

    return {
        "name": case["name"],
        "lang": case["lang"],
        "changed": sorted(changed_names),
        "impacted": sorted(impacted_names),
        "expected": {
            "changed": sorted(exp_changed),
            "impacted": sorted(exp_impacted),
        },
        "fn": {
            "changed": fn_changed,
            "impacted": fn_impacted,
            "total": len(fn_changed) + len(fn_impacted),
        },
        "fp": {
            "changed": fp_changed,
            "impacted": fp_impacted,
            "total": len(fp_changed) + len(fp_impacted),
        },
    }

results = [run_case(c) for c in cases]
fn_total = sum(r["fn"]["total"] for r in results)
fp_total = sum(r["fp"]["total"] for r in results)

report = {
    "fnThreshold": fn_threshold,
    "fpThreshold": fp_threshold,
    "totals": {
        "fn": fn_total,
        "fp": fp_total,
    },
    "cases": results,
}

report_path.parent.mkdir(parents=True, exist_ok=True)
report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

print("# precision regression gate")
print(f"FN total={fn_total} (threshold={fn_threshold})")
print(f"FP total={fp_total} (threshold={fp_threshold})")
for r in results:
    print(
        f"- {r['name']} ({r['lang']}): fn={r['fn']['total']} fp={r['fp']['total']} changed={r['changed']} impacted={r['impacted']}"
    )

if fn_total > fn_threshold or fp_total > fp_threshold:
    print("precision regression gate: FAILED", file=sys.stderr)
    sys.exit(1)

print("precision regression gate: PASSED")
PY
