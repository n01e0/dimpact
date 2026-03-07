#!/usr/bin/env bash
set -euo pipefail

REPO="${1:-n01e0/dimpact}"
LIMIT="${2:-20}"
OUT_JSON="${3:-docs/bench-python-install-failure-patterns-v0.4.1.json}"
OUT_MD="${4:-docs/bench-python-install-failure-patterns-v0.4.1.md}"

python3 - "$REPO" "$LIMIT" "$OUT_JSON" "$OUT_MD" <<'PY'
import json
import subprocess
import sys
from pathlib import Path

repo = sys.argv[1]
limit = int(sys.argv[2])
out_json = Path(sys.argv[3])
out_md = Path(sys.argv[4])


def run(cmd):
    return subprocess.check_output(cmd, text=True)

runs = json.loads(
    run([
        "gh",
        "run",
        "list",
        "--repo",
        repo,
        "--workflow",
        "bench.yml",
        "--limit",
        str(limit),
        "--json",
        "databaseId,createdAt,conclusion,url,displayTitle",
    ])
)

items = []
for r in runs:
    rid = str(r["databaseId"])
    jobs = json.loads(
        run(["gh", "run", "view", rid, "--repo", repo, "--json", "jobs"]) 
    )["jobs"]
    py_jobs = [j for j in jobs if j.get("name") == "bench-python-strict-lsp"]
    if not py_jobs:
        continue
    j = py_jobs[0]
    jid = str(j["databaseId"])
    conc = j.get("conclusion") or "unknown"

    # Fetch plain-text log via API endpoint
    log = run(["gh", "api", f"repos/{repo}/actions/jobs/{jid}/logs"])

    pattern = "none"
    reason = ""
    if "Connection input stream is not set" in log:
        pattern = "healthcheck-transport-mismatch"
        reason = "pyright-langserver --help/--version exits non-zero without stdio/node-ipc/socket transport"
    elif "npm ERR!" in log or "ERR!" in log and "npm" in log:
        pattern = "npm-install-failure"
        reason = "npm global install failure (network/registry/package)"
    elif "pyright-langserver: command not found" in log:
        pattern = "binary-not-found"
        reason = "pyright-langserver unavailable in PATH after install step"
    elif conc == "failure":
        pattern = "unknown-install-failure"
        reason = "install step failed but no known signature matched"

    sample = ""
    for line in log.splitlines():
        if (
            "Connection input stream is not set" in line
            or "npm ERR!" in line
            or "command not found" in line
            or "##[error]Process completed with exit code" in line
        ):
            sample = line.strip()
            break

    items.append(
        {
            "runId": rid,
            "runUrl": r.get("url"),
            "createdAt": r.get("createdAt"),
            "runConclusion": r.get("conclusion"),
            "jobConclusion": conc,
            "pattern": pattern,
            "reason": reason,
            "sample": sample,
        }
    )

# aggregate
from collections import Counter
count_by_pattern = Counter(i["pattern"] for i in items)

report = {
    "repo": repo,
    "limit": limit,
    "totalRunsChecked": len(runs),
    "pythonJobsChecked": len(items),
    "patternCounts": dict(count_by_pattern),
    "items": items,
    "localRepro": {
        "badHealthcheck": [
            "npm install -g pyright",
            "pyright-langserver --help",
        ],
        "expectedSignature": "Connection input stream is not set",
    },
}

out_json.parent.mkdir(parents=True, exist_ok=True)
out_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

md = []
md.append("# bench-python-strict-lsp install failure patterns (PH66-1)")
md.append("")
md.append(f"repo: `{repo}`")
md.append(f"runs scanned: `{len(runs)}` (python jobs found: `{len(items)}`)")
md.append("")
md.append("## Pattern counts")
md.append("")
md.append("| pattern | count |")
md.append("|---|---:|")
for k, v in sorted(count_by_pattern.items()):
    md.append(f"| {k} | {v} |")

md.append("")
md.append("## Observed runs")
md.append("")
md.append("| runId | jobConclusion | pattern | reason | sample |")
md.append("|---|---|---|---|---|")
for i in items:
    sample = (i.get("sample") or "").replace("|", "\\|")
    reason = (i.get("reason") or "").replace("|", "\\|")
    md.append(
        f"| [{i['runId']}]({i.get('runUrl','')}) | {i['jobConclusion']} | {i['pattern']} | {reason} | {sample} |"
    )

md.append("")
md.append("## Local reproduction")
md.append("")
md.append("```bash")
md.append("npm install -g pyright")
md.append("pyright-langserver --help")
md.append("```")
md.append("")
md.append("Expected failure signature:")
md.append("- `Connection input stream is not set ... --node-ipc / --stdio / --socket`")

out_md.parent.mkdir(parents=True, exist_ok=True)
out_md.write_text("\n".join(md) + "\n", encoding="utf-8")
print(f"wrote {out_json}")
print(f"wrote {out_md}")
PY
