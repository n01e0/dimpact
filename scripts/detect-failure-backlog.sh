#!/usr/bin/env bash
set -euo pipefail

REPO=""
MAIN_BRANCH="main"
CI_WORKFLOW="CI"
NIGHTLY_WORKFLOW="nightly-strict-lsp.yml"
NIGHTLY_SAMPLE=4
OUT_JSON="failure-backlog-inventory.json"
OUT_MD="failure-backlog-inventory.md"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO="$2"
      shift 2
      ;;
    --main-branch)
      MAIN_BRANCH="$2"
      shift 2
      ;;
    --ci-workflow)
      CI_WORKFLOW="$2"
      shift 2
      ;;
    --nightly-workflow)
      NIGHTLY_WORKFLOW="$2"
      shift 2
      ;;
    --nightly-sample)
      NIGHTLY_SAMPLE="$2"
      shift 2
      ;;
    --out-json)
      OUT_JSON="$2"
      shift 2
      ;;
    --out-md)
      OUT_MD="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$REPO" ]]; then
  REPO="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"
fi

python3 - "$REPO" "$MAIN_BRANCH" "$CI_WORKFLOW" "$NIGHTLY_WORKFLOW" "$NIGHTLY_SAMPLE" "$OUT_JSON" "$OUT_MD" <<'PY'
import datetime
import json
import subprocess
import sys
from pathlib import Path

repo = sys.argv[1]
main_branch = sys.argv[2]
ci_workflow = sys.argv[3]
nightly_workflow = sys.argv[4]
nightly_sample = int(sys.argv[5])
out_json = Path(sys.argv[6])
out_md = Path(sys.argv[7])


def run(cmd):
    p = subprocess.run(cmd, text=True, capture_output=True)
    if p.returncode != 0:
        raise RuntimeError(f"command failed: {' '.join(cmd)}\nstdout={p.stdout}\nstderr={p.stderr}")
    return p.stdout


def gh_json(args):
    out = run(["gh", *args])
    return json.loads(out)


def get_required_contexts():
    try:
        data = gh_json([
            "api",
            f"repos/{repo}/branches/{main_branch}/protection/required_status_checks",
        ])
    except Exception:
        return []

    contexts = list(data.get("contexts") or [])
    checks = data.get("checks") or []
    for chk in checks:
        ctx = chk.get("context")
        if ctx and ctx not in contexts:
            contexts.append(ctx)
    return contexts


def latest_completed_ci_run():
    runs = gh_json([
        "-R",
        repo,
        "run",
        "list",
        "--workflow",
        ci_workflow,
        "--branch",
        main_branch,
        "--limit",
        "20",
        "--json",
        "databaseId,status,conclusion,displayTitle,headSha,createdAt,updatedAt,url",
    ])
    for r in runs:
        if r.get("status") == "completed":
            return r
    return runs[0] if runs else None


def statuses_on_run(run_id, required_contexts):
    if run_id is None:
        return [], []
    rv = gh_json([
        "-R",
        repo,
        "run",
        "view",
        str(run_id),
        "--json",
        "jobs",
    ])
    jobs = rv.get("jobs") or []
    statuses = []
    failing_or_missing = []
    for ctx in required_contexts:
        job = next((j for j in jobs if j.get("name") == ctx), None)
        status = "missing"
        url = None
        if job:
            status = job.get("conclusion") or job.get("status") or "unknown"
            url = job.get("url")
        row = {"context": ctx, "status": status, "url": url}
        statuses.append(row)
        if str(status).lower() != "success":
            failing_or_missing.append(row)
    return statuses, failing_or_missing


def completed_nightly_runs(sample):
    runs = gh_json([
        "-R",
        repo,
        "run",
        "list",
        "--workflow",
        nightly_workflow,
        "--branch",
        main_branch,
        "--limit",
        str(max(sample * 3, sample)),
        "--json",
        "databaseId,status,conclusion,headSha,createdAt,updatedAt,url,event",
    ])
    completed = [r for r in runs if r.get("status") == "completed"]
    return completed[:sample]


def failed_jobs_for_run(run_id):
    rv = gh_json([
        "-R",
        repo,
        "run",
        "view",
        str(run_id),
        "--json",
        "jobs",
    ])
    out = []
    for j in rv.get("jobs") or []:
        if j.get("conclusion") == "failure":
            out.append(
                {
                    "name": j.get("name"),
                    "conclusion": j.get("conclusion"),
                    "url": j.get("url"),
                }
            )
    return out


required_contexts = get_required_contexts()
latest_ci = latest_completed_ci_run()
ci_statuses, failing_or_missing = statuses_on_run(
    latest_ci.get("databaseId") if latest_ci else None, required_contexts
)

nightly_runs = completed_nightly_runs(nightly_sample)
nightly_failures = []
for r in nightly_runs:
    if r.get("conclusion") == "failure":
        nightly_failures.append(
            {
                "runId": r.get("databaseId"),
                "createdAt": r.get("createdAt"),
                "updatedAt": r.get("updatedAt"),
                "headSha": r.get("headSha"),
                "url": r.get("url"),
                "failedJobs": failed_jobs_for_run(r.get("databaseId")),
            }
        )

summary = {
    "mainRequiredFailures": len(failing_or_missing),
    "nightlyFailureRuns": len(nightly_failures),
    "failureBacklogTotal": len(failing_or_missing) + len(nightly_failures),
}

payload = {
    "generatedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "repo": repo,
    "mainBranch": main_branch,
    "mainRequiredChecks": {
        "requiredContexts": required_contexts,
        "latestMainCiRun": latest_ci,
        "statusesOnLatestMainCiRun": ci_statuses,
        "failingOrMissingCount": len(failing_or_missing),
        "failingOrMissing": failing_or_missing,
    },
    "nightlyStrictLsp": {
        "workflow": nightly_workflow,
        "sampledRuns": len(nightly_runs),
        "failureRuns": len(nightly_failures),
        "failures": nightly_failures,
    },
    "summary": summary,
}

out_json.parent.mkdir(parents=True, exist_ok=True)
out_json.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n")

lines = []
lines.append("# failure backlog inventory (main required + nightly)")
lines.append("")
lines.append(f"- repo: `{repo}`")
lines.append(f"- main branch: `{main_branch}`")
if latest_ci:
    lines.append(
        f"- latest main CI run: [{latest_ci['databaseId']}]({latest_ci['url']}) (`{latest_ci.get('conclusion')}`)"
    )
lines.append("")
lines.append("## 1) main required checks")
lines.append("| context | status |")
lines.append("| --- | --- |")
for st in ci_statuses:
    lines.append(f"| {st['context']} | {st['status']} |")
lines.append(f"- failing/missing required checks: **{len(failing_or_missing)}**")
lines.append("")
lines.append("## 2) nightly strict-lsp failures")
lines.append(f"- sampled nightly runs: `{len(nightly_runs)}`")
lines.append(f"- nightly failure runs: `{len(nightly_failures)}`")
if nightly_failures:
    lines.append("")
    lines.append("| Run | Head | Failed jobs |")
    lines.append("| --- | --- | --- |")
    for f in nightly_failures:
        failed_jobs = ", ".join(j.get("name") or "(unknown)" for j in f.get("failedJobs", [])) or "(none)"
        lines.append(
            f"| [{f['runId']}]({f['url']}) | `{(f.get('headSha') or '')[:8]}` | {failed_jobs} |"
        )
lines.append("")
lines.append("## 3) backlog summary")
lines.append(f"- main required failures: `{summary['mainRequiredFailures']}`")
lines.append(f"- nightly failure runs: `{summary['nightlyFailureRuns']}`")
lines.append(f"- current failure backlog: **{summary['failureBacklogTotal']}**")

out_md.parent.mkdir(parents=True, exist_ok=True)
out_md.write_text("\n".join(lines) + "\n")

print(str(out_json))
print(str(out_md))
print(json.dumps(summary))
PY