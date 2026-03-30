#!/usr/bin/env python3
"""Collect a fixed snapshot of the public JSON schema registry."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT = ROOT / "docs" / "s1-10-schema-registry-snapshot.json"

RESOLVE_CASES = [
    ["diff"],
    ["changed", "--lang", "rust"],
    ["impact"],
    ["impact", "--with-edges"],
    ["impact", "--per-seed", "--with-pdg"],
    ["impact", "--per-seed", "--with-edges", "--with-propagation"],
    ["id"],
]


def run_json(*args: str):
    proc = subprocess.run(
        ["cargo", "run", "--quiet", "--bin", "dimpact", "--", *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    return proc.stdout, json.loads(proc.stdout)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    ns = parser.parse_args()

    list_stdout, schema_list = run_json("schema", "--list")

    documents = []
    for item in schema_list:
        doc_stdout, doc_json = run_json("schema", "--id", item["schema_id"])
        documents.append(
            {
                "schema_id": item["schema_id"],
                "schema_path": item["schema_path"],
                "title": doc_json.get("title"),
                "status": doc_json.get("x-dimpact", {}).get("status"),
                "sha256": hashlib.sha256(doc_stdout.encode("utf-8")).hexdigest(),
            }
        )

    resolve_cases = []
    for argv in RESOLVE_CASES:
        _, resolved = run_json("schema", "resolve", *argv)
        resolve_cases.append({"argv": argv, "result": resolved})

    snapshot = {
        "generated_by": "scripts/collect-schema-snapshot.py",
        "snapshot_scope": {
            "captures": [
                "schema --list",
                "schema --id <schema-id>",
                "schema resolve <command> [flags...]",
            ],
            "excludes": [
                "runtime JSON output from diff -f json",
                "runtime JSON output from changed -f json",
                "runtime JSON output from impact -f json",
                "runtime JSON output from id -f json",
            ],
        },
        "schema_count": len(schema_list),
        "schema_list": schema_list,
        "resolve_cases": resolve_cases,
        "documents": documents,
    }

    out_path = Path(ns.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(snapshot, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
