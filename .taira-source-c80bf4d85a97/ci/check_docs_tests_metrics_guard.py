#!/usr/bin/env python3
"""
Guardrail that enforces docs + tests + metrics updates alongside roadmap changes.

When `roadmap.md` changes, this script requires at least one documentation change,
one test change, and one metrics/telemetry/dashboard change in the same diff.
Use `DOC_TEST_METRIC_GUARD_ALLOW=1` to bypass when absolutely necessary.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import subprocess
import sys
from dataclasses import dataclass
from typing import Iterable, List, Sequence, Tuple

DEFAULT_BASE_CANDIDATES = ("origin/main", "origin/master", "main", "master")
DEFAULT_ROOT = pathlib.Path(__file__).resolve().parent.parent


def run_git(args: Sequence[str], *, cwd: pathlib.Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
    )


def resolve_base_ref(requested: str | None, *, root: pathlib.Path = DEFAULT_ROOT) -> str:
    if requested:
        if run_git(["rev-parse", "--verify", requested], cwd=root).returncode == 0:
            return requested
        print(f"warning: AGENTS_BASE_REF={requested} not found; falling back to defaults.", file=sys.stderr)

    for candidate in DEFAULT_BASE_CANDIDATES:
        if run_git(["rev-parse", "--verify", candidate], cwd=root).returncode == 0:
            return candidate

    return "HEAD^"


def resolve_base_commit(base_ref: str, *, root: pathlib.Path = DEFAULT_ROOT) -> str:
    merge_base = run_git(["merge-base", base_ref, "HEAD"], cwd=root)
    if merge_base.returncode == 0 and merge_base.stdout.strip():
        return merge_base.stdout.strip()

    resolved = run_git(["rev-parse", base_ref], cwd=root)
    if resolved.returncode == 0 and resolved.stdout.strip():
        return resolved.stdout.strip()

    return "HEAD"


def changed_files(base_commit: str, *, root: pathlib.Path = DEFAULT_ROOT) -> List[str]:
    diff = run_git(["diff", "--name-only", f"{base_commit}...HEAD"], cwd=root)
    return [line for line in diff.stdout.splitlines() if line.strip()]


def roadmap_changed(base_commit: str, *, root: pathlib.Path = DEFAULT_ROOT) -> bool:
    diff = run_git(["diff", "--name-only", f"{base_commit}...HEAD", "--", "roadmap.md"], cwd=root)
    return any(line.strip() for line in diff.stdout.splitlines())


def is_doc_path(path: pathlib.Path) -> bool:
    if "docs" in path.parts:
        return True
    return path.suffix.lower() in {".md", ".mdx", ".rst", ".adoc"}


def is_test_path(path: pathlib.Path) -> bool:
    name = path.name.lower()
    if "test" in name or "spec" in name:
        return True
    return any(part in {"tests", "test", "__tests__"} for part in path.parts)


def is_metrics_path(path: pathlib.Path) -> bool:
    lowered_parts = [part.lower() for part in path.parts]
    if any(part in {"dashboards", "grafana", "telemetry"} for part in lowered_parts):
        return True
    name = path.name.lower()
    return "metrics" in name or "dashboard" in name


@dataclass(frozen=True)
class Categories:
    docs: Tuple[str, ...]
    tests: Tuple[str, ...]
    metrics: Tuple[str, ...]


def categorize(paths: Iterable[str]) -> Categories:
    docs: List[str] = []
    tests: List[str] = []
    metrics: List[str] = []

    for raw in paths:
        rel = pathlib.Path(raw)
        if rel == pathlib.Path("roadmap.md"):
            continue
        if is_doc_path(rel):
            docs.append(raw)
        if is_test_path(rel):
            tests.append(raw)
        if is_metrics_path(rel):
            metrics.append(raw)

    return Categories(tuple(docs), tuple(tests), tuple(metrics))


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Enforce docs/tests/metrics updates when roadmap changes.")
    parser.add_argument("--base", help="Git ref to diff against (defaults to AGENTS_BASE_REF or main/master).")
    parser.add_argument(
        "--repo-root",
        type=pathlib.Path,
        default=DEFAULT_ROOT,
        help="Repository root (defaults to repo root inferred from this script).",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    if os.environ.get("DOC_TEST_METRIC_GUARD_ALLOW") == "1":
        print("docs/tests/metrics guard bypassed via DOC_TEST_METRIC_GUARD_ALLOW=1")
        return 0

    args = parse_args(argv)
    root = args.repo_root.resolve()

    base_ref = args.base or os.environ.get("AGENTS_BASE_REF")
    base_ref = resolve_base_ref(base_ref, root=root)
    base_commit = resolve_base_commit(base_ref, root=root)

    if not roadmap_changed(base_commit, root=root):
        print("roadmap.md unchanged; docs/tests/metrics guard skipped.")
        return 0

    files = changed_files(base_commit, root=root)
    categories = categorize(files)

    missing = []
    if not categories.docs:
        missing.append("docs (docs/, *.md|*.rst|*.adoc)")
    if not categories.tests:
        missing.append("tests (tests/ directories or *test*/ *spec* files)")
    if not categories.metrics:
        missing.append("metrics/telemetry/dashboards (dashboards/, *metrics*, *dashboard*)")

    if missing:
        print("error: roadmap.md changed but required categories are missing:", file=sys.stderr)
        for item in missing:
            print(f" - {item}", file=sys.stderr)
        if files:
            print("changed files:", file=sys.stderr)
            for path in files:
                print(f"   - {path}", file=sys.stderr)
        print(
            "Set DOC_TEST_METRIC_GUARD_ALLOW=1 to bypass in emergencies, but prefer adding docs/tests/metrics updates.",
            file=sys.stderr,
        )
        return 1

    print("docs/tests/metrics guard passed: roadmap changes accompanied by docs, tests, and metrics updates.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
