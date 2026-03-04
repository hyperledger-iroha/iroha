#!/usr/bin/env python3
"""Generate a combined Android test + fixture-parity report."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, Optional

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REPORT = REPO_ROOT / "artifacts" / "android" / "test_report.json"
DEFAULT_TEST_SUMMARY = REPO_ROOT / "artifacts" / "android" / "test_summary.json"
DEFAULT_FIXTURE_SUMMARY = REPO_ROOT / "artifacts" / "android" / "fixture_parity_summary.json"


def iso_timestamp() -> str:
    """Return a UTC timestamp without sub-second precision."""
    return (
        datetime.now(timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def relative(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def load_json(path: Path, *, label: str) -> Dict[str, object]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"{label} not found at {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"{label} at {path} contained invalid JSON: {exc}") from exc
    if not isinstance(data, dict):
        raise ValueError(f"{label} at {path} must be a JSON object")
    return data


def detect_git_commit(root: Path) -> Optional[str]:
    try:
        output = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=root, stderr=subprocess.DEVNULL
        )
    except Exception:
        return None
    return output.decode().strip() or None


def parse_test_summary(data: Dict[str, object]) -> Dict[str, object]:
    failures = data.get("failures")
    total = data.get("total")
    if failures is None or not isinstance(failures, int):
        raise ValueError("test summary missing integer 'failures'")
    if total is None or not isinstance(total, int):
        raise ValueError("test summary missing integer 'total'")
    results = data.get("results")
    if results is not None and not isinstance(results, list):
        raise ValueError("test summary 'results' must be a list when present")
    return data


def test_status_from_summary(summary: Dict[str, object], exit_code: int) -> str:
    if exit_code != 0:
        return "failed"
    failures = summary.get("failures")
    if isinstance(failures, int) and failures == 0:
        return "ok"
    return "failed"


def run_android_tests(
    root: Path,
    *,
    summary_path: Path,
    tests: Optional[str],
) -> Dict[str, object]:
    env = os.environ.copy()
    env["ANDROID_TEST_SUMMARY_OUT"] = str(summary_path)
    cmd = [str(root / "ci" / "run_android_tests.sh")]
    if tests:
        env["ANDROID_HARNESS_MAINS"] = tests

    start = time.time()
    result = subprocess.run(cmd, cwd=root, env=env)
    duration_ms = int((time.time() - start) * 1000)

    summary_data: Optional[Dict[str, object]] = None
    summary_error: Optional[str] = None
    try:
        summary_data = parse_test_summary(load_json(summary_path, label="test summary"))
    except Exception as exc:  # pragma: no cover - defensive guard
        summary_error = str(exc)

    status = "failed"
    if summary_data is not None:
        status = test_status_from_summary(summary_data, result.returncode)

    return {
        "status": status if summary_data is not None else "failed",
        "exit_code": result.returncode,
        "duration_ms": duration_ms,
        "summary_path": relative(summary_path),
        "summary": summary_data,
        "summary_error": summary_error,
    }


def run_fixture_check(root: Path, *, summary_path: Path) -> Dict[str, object]:
    cmd = [
        "python3",
        str(root / "scripts" / "check_android_fixtures.py"),
        "--json-out",
        str(summary_path),
        "--quiet",
    ]
    result = subprocess.run(cmd, cwd=root)

    summary_data: Optional[Dict[str, object]] = None
    summary_error: Optional[str] = None
    try:
        summary_data = load_json(summary_path, label="fixture summary")
    except Exception as exc:  # pragma: no cover - defensive guard
        summary_error = str(exc)

    status = "failed"
    if summary_data is not None:
        summary_status = summary_data.get("result", {}).get("status")
        status = "ok" if summary_status == "ok" and result.returncode == 0 else "failed"

    return {
        "status": status if summary_data is not None else "failed",
        "exit_code": result.returncode,
        "summary_path": relative(summary_path),
        "summary": summary_data,
        "summary_error": summary_error,
    }


def build_report(
    *,
    tests: Dict[str, object],
    fixtures: Dict[str, object],
    git_commit: Optional[str],
    metadata: Optional[Dict[str, object]],
) -> Dict[str, object]:
    overall_ok = tests.get("status") == "ok" and fixtures.get("status") == "ok"
    report: Dict[str, object] = {
        "schema_version": 1,
        "generated_at": iso_timestamp(),
        "overall_status": "ok" if overall_ok else "failed",
        "tests": tests,
        "fixtures": fixtures,
    }
    if git_commit:
        report["git_commit"] = git_commit
    if metadata:
        report["metadata"] = metadata
    return report


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate a combined Android test + fixture report",
    )
    parser.add_argument(
        "--run-tests",
        action="store_true",
        help="Execute ci/run_android_tests.sh instead of reading --summary",
    )
    parser.add_argument(
        "--tests",
        metavar="LIST",
        help="Comma-separated test mains to pass via ANDROID_HARNESS_MAINS when --run-tests is set",
    )
    parser.add_argument(
        "--summary",
        type=Path,
        help="Existing test summary JSON to embed (required when --run-tests is not set)",
    )
    parser.add_argument(
        "--fixtures-summary",
        type=Path,
        help="Existing fixture parity summary JSON to embed (otherwise check_android_fixtures.py will run)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_REPORT,
        help=f"Where to write the combined report (default: {DEFAULT_REPORT})",
    )
    parser.add_argument(
        "--allow-failures",
        action="store_true",
        help="Exit 0 even if the combined status is failed",
    )
    parser.add_argument(
        "--metadata",
        type=Path,
        help="Optional JSON file to merge into the report metadata block",
    )
    parser.add_argument(
        "--git-commit",
        help="Optional git commit hash to record (default: auto-detect via git)",
    )
    parser.add_argument(
        "--test-summary-out",
        type=Path,
        default=DEFAULT_TEST_SUMMARY,
        help="Path to write the test summary when --run-tests is enabled",
    )
    parser.add_argument(
        "--fixture-summary-out",
        type=Path,
        default=DEFAULT_FIXTURE_SUMMARY,
        help="Path to write the fixture parity summary when not supplied",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    root = REPO_ROOT

    metadata: Optional[Dict[str, object]] = None
    if args.metadata:
        metadata = load_json(args.metadata, label="metadata")

    if args.run_tests:
        args.test_summary_out.parent.mkdir(parents=True, exist_ok=True)
        tests_block = run_android_tests(
            root,
            summary_path=args.test_summary_out.resolve(),
            tests=args.tests,
        )
    else:
        if not args.summary:
            raise SystemExit("--summary is required unless --run-tests is set")
        tests_data = parse_test_summary(load_json(args.summary, label="test summary"))
        tests_block = {
            "status": test_status_from_summary(tests_data, exit_code=0),
            "exit_code": 0,
            "summary_path": relative(args.summary.resolve()),
            "summary": tests_data,
            "duration_ms": None,
        }

    if args.fixtures_summary:
        fixtures_block = {
            "status": "failed",
            "exit_code": None,
            "summary_path": relative(args.fixtures_summary.resolve()),
            "summary": None,
            "summary_error": None,
        }
        try:
            fixtures_data = load_json(args.fixtures_summary, label="fixture summary")
            summary_status = fixtures_data.get("result", {}).get("status")
            fixtures_block["summary"] = fixtures_data
            fixtures_block["status"] = "ok" if summary_status == "ok" else "failed"
        except Exception as exc:
            fixtures_block["summary_error"] = str(exc)
    else:
        args.fixture_summary_out.parent.mkdir(parents=True, exist_ok=True)
        fixtures_block = run_fixture_check(
            root,
            summary_path=args.fixture_summary_out.resolve(),
        )

    commit = args.git_commit or detect_git_commit(root)
    report = build_report(
        tests=tests_block,
        fixtures=fixtures_block,
        git_commit=commit,
        metadata=metadata,
    )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"[ok] wrote Android test report to {args.output}")

    if report["overall_status"] != "ok" and not args.allow_failures:
        return 1
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
