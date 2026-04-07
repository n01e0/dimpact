#!/usr/bin/env python3
import argparse
import os
import sys
from pathlib import Path

SUCCESS_VALUES = {"success", "passed", "pass"}
FAILURE_VALUES = {"failure", "failed", "cancelled", "canceled", "timed_out", "timed-out", "error"}
SKIPPED_VALUES = {"", "skipped", "skip", "none", "null", "unset", "unknown"}


def normalize(value: str | None, *, retry_enabled: bool, is_retry: bool) -> str:
    raw = (value or "").strip().lower()
    if raw in SUCCESS_VALUES:
        return "success"
    if raw in FAILURE_VALUES:
        return "failure"
    if raw in SKIPPED_VALUES:
        if is_retry and not retry_enabled:
            return "skipped"
        return "skipped" if raw else ("skipped" if is_retry else "unknown")
    return raw


def resolve(result: str | None, outcome: str | None, *, retry_enabled: bool, is_retry: bool) -> str:
    explicit = normalize(result, retry_enabled=retry_enabled, is_retry=is_retry)
    if explicit in {"success", "failure"}:
        return explicit
    inferred = normalize(outcome, retry_enabled=retry_enabled, is_retry=is_retry)
    if inferred in {"success", "failure", "skipped"}:
        return inferred
    if is_retry and not retry_enabled:
        return "skipped"
    return explicit


def ok(*statuses: str) -> int:
    return 1 if any(status == "success" for status in statuses) else 0


def append_summary(path: Path, lines: list[str]) -> None:
    with path.open("a", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--summary-file", required=True)
    args = ap.parse_args()

    retry_enabled = (os.environ.get("RETRY_ENABLED") or "").strip() == "1"

    init_strict = resolve(
        os.environ.get("INIT_STRICT_RESULT"),
        os.environ.get("INIT_STRICT_OUTCOME"),
        retry_enabled=retry_enabled,
        is_retry=False,
    )
    retry_strict = resolve(
        os.environ.get("RETRY_STRICT_RESULT"),
        os.environ.get("RETRY_STRICT_OUTCOME"),
        retry_enabled=retry_enabled,
        is_retry=True,
    )
    init_grad = resolve(
        os.environ.get("INIT_GRAD_RESULT"),
        os.environ.get("INIT_GRAD_OUTCOME"),
        retry_enabled=retry_enabled,
        is_retry=False,
    )
    retry_grad = resolve(
        os.environ.get("RETRY_GRAD_RESULT"),
        os.environ.get("RETRY_GRAD_OUTCOME"),
        retry_enabled=retry_enabled,
        is_retry=True,
    )

    strict_ok = ok(init_strict, retry_strict)
    grad_ok = ok(init_grad, retry_grad)

    print(
        f"[finalize] strict initial={init_strict} strict retry={retry_strict} "
        f"grad initial={init_grad} grad retry={retry_grad} retry enabled={int(retry_enabled)} "
        f"strict_ok={strict_ok} grad_ok={grad_ok}"
    )

    append_summary(
        Path(args.summary_file),
        [
            "## retry finalize",
            f"- strict initial: {init_strict}",
            f"- strict retry: {retry_strict}",
            f"- graduation initial: {init_grad}",
            f"- graduation retry: {retry_grad}",
            f"- retry enabled: {int(retry_enabled)}",
            f"- strict final ok: {strict_ok}",
            f"- graduation final ok: {grad_ok}",
        ],
    )

    if not strict_ok or not grad_ok:
        print("nightly failed after applying retry policy", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
