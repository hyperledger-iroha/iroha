#!/usr/bin/env python3
"""Enforce ratcheting line-count budgets for candidate source files."""

from __future__ import annotations

import argparse
import json
import os
import stat
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable


SCHEMA_VERSION = 1
DEFAULT_PRODUCTION_LIMIT = 5_000
DEFAULT_TEST_LIMIT = 3_000
SOURCE_SUFFIXES = frozenset(
    {
        ".c",
        ".cc",
        ".cpp",
        ".cs",
        ".go",
        ".h",
        ".hpp",
        ".java",
        ".js",
        ".kt",
        ".kts",
        ".m",
        ".mm",
        ".py",
        ".rs",
        ".sh",
        ".swift",
        ".ts",
        ".tsx",
    }
)
DEFAULT_EXCLUDED_PREFIXES = (
    "docs/portal/node_modules/",
    "target/",
    "vendor/",
)


@dataclass(frozen=True)
class AggregateRustBudget:
    """Repository-wide first-party Rust line budget."""

    baseline: int
    ceiling: int
    ratchet_ceiling: int
    working_target: int | None


@dataclass(frozen=True)
class Budget:
    """Checked-in source budget configuration."""

    production_limit: int
    test_limit: int
    excluded_prefixes: tuple[str, ...]
    exceptions: dict[str, int]
    aggregate_rust: AggregateRustBudget | None = None


@dataclass(frozen=True)
class Finding:
    """One source-budget violation."""

    path: str
    message: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Check tracked and non-ignored untracked source files against "
            "production/test line limits. "
            "Files already above a limit must have an exact no-growth baseline."
        )
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root (default: inferred from this script).",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        default=Path("ci/source_file_budget.json"),
        help="Budget baseline path, relative to --root by default.",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--write-baseline",
        action="store_true",
        help=(
            "Rewrite the baseline from the current tracked tree. Use this after "
            "reviewing intentional file splits or other line-count reductions."
        ),
    )
    mode.add_argument(
        "--require-objective",
        action="store_true",
        help=(
            "Fail unless the repository-wide Rust line count is at or below "
            "aggregate_rust.ceiling."
        ),
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write a machine-readable report; use `-` for stdout.",
    )
    return parser.parse_args()


def canonical_prefix(prefix: str) -> str:
    """Validate and normalize a repository-relative exclusion prefix."""
    normalized = prefix.replace("\\", "/").strip("/")
    path = PurePosixPath(normalized)
    if not normalized or path.is_absolute() or ".." in path.parts:
        raise ValueError(f"invalid excluded prefix: {prefix!r}")
    return f"{normalized}/"


def parse_non_negative_int(payload: Any, field: str) -> int:
    """Parse a non-negative integer while rejecting booleans."""
    if isinstance(payload, bool) or not isinstance(payload, int) or payload < 0:
        raise ValueError(f"{field} must be a non-negative integer")
    return payload


def load_budget(path: Path) -> Budget:
    """Load and validate a source budget baseline."""
    payload: Any = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("source budget must be a JSON object")
    if payload.get("schema_version") != SCHEMA_VERSION:
        raise ValueError(
            f"source budget schema_version must be {SCHEMA_VERSION}"
        )

    limits = payload.get("limits")
    if not isinstance(limits, dict):
        raise ValueError("source budget limits must be a JSON object")
    production_limit = parse_non_negative_int(
        limits.get("production"), "limits.production"
    )
    test_limit = parse_non_negative_int(limits.get("test"), "limits.test")
    if production_limit == 0 or test_limit == 0:
        raise ValueError("source budget limits must be greater than zero")

    raw_prefixes = payload.get("excluded_prefixes", [])
    if not isinstance(raw_prefixes, list) or not all(
        isinstance(prefix, str) for prefix in raw_prefixes
    ):
        raise ValueError("excluded_prefixes must be an array of strings")
    excluded_prefixes = tuple(
        sorted({canonical_prefix(prefix) for prefix in raw_prefixes})
    )

    raw_exceptions = payload.get("exceptions")
    if not isinstance(raw_exceptions, dict):
        raise ValueError("source budget exceptions must be a JSON object")
    exceptions: dict[str, int] = {}
    for raw_path, raw_limit in raw_exceptions.items():
        if not isinstance(raw_path, str):
            raise ValueError("source budget exception paths must be strings")
        normalized = normalize_relative_path(raw_path)
        exceptions[normalized] = parse_non_negative_int(
            raw_limit, f"exceptions.{normalized}"
        )

    raw_aggregate = payload.get("aggregate_rust")
    if not isinstance(raw_aggregate, dict):
        raise ValueError(
            "aggregate_rust must be a JSON object; the repository-wide "
            "Rust reduction contract is mandatory"
        )
    baseline = parse_non_negative_int(
        raw_aggregate.get("baseline"), "aggregate_rust.baseline"
    )
    ceiling = parse_non_negative_int(
        raw_aggregate.get("ceiling"), "aggregate_rust.ceiling"
    )
    ratchet_ceiling = parse_non_negative_int(
        raw_aggregate.get("ratchet_ceiling", ceiling),
        "aggregate_rust.ratchet_ceiling",
    )
    raw_working_target = raw_aggregate.get("working_target")
    working_target = (
        None
        if raw_working_target is None
        else parse_non_negative_int(
            raw_working_target, "aggregate_rust.working_target"
        )
    )
    if baseline == 0 or ceiling == 0:
        raise ValueError("aggregate_rust baseline and ceiling must be greater than zero")
    if ceiling > baseline:
        raise ValueError("aggregate_rust.ceiling must not exceed its baseline")
    if ceiling * 10 > baseline * 9:
        raise ValueError(
            "aggregate_rust.ceiling must require at least a 10% reduction "
            "from its baseline"
        )
    if ratchet_ceiling < ceiling:
        raise ValueError(
            "aggregate_rust.ratchet_ceiling must not be below its ceiling"
        )
    if working_target is not None and working_target > ceiling:
        raise ValueError(
            "aggregate_rust.working_target must not exceed its ceiling"
        )
    aggregate_rust = AggregateRustBudget(
        baseline=baseline,
        ceiling=ceiling,
        ratchet_ceiling=ratchet_ceiling,
        working_target=working_target,
    )

    return Budget(
        production_limit=production_limit,
        test_limit=test_limit,
        excluded_prefixes=excluded_prefixes,
        exceptions=exceptions,
        aggregate_rust=aggregate_rust,
    )


def normalize_relative_path(path: str | PurePosixPath) -> str:
    """Return one canonical, repository-relative POSIX path."""
    normalized = str(path).replace("\\", "/")
    pure = PurePosixPath(normalized)
    if not normalized or pure.is_absolute() or ".." in pure.parts:
        raise ValueError(f"invalid repository-relative path: {path!r}")
    return pure.as_posix()


def tracked_paths(root: Path) -> list[str]:
    """Return the full non-ignored candidate-tree paths in deterministic order."""
    output = subprocess.check_output(
        [
            "git",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        cwd=root,
    )
    decoded = os.fsdecode(output)
    paths = (
        normalize_relative_path(path)
        for path in decoded.split("\0")
        if path
    )
    # A refactor may remove tracked files before its commit exists. The budget
    # governs the candidate tree on disk; stale baseline entries still make a
    # later validation fail closed.
    return sorted(path for path in paths if os.path.lexists(root / path))


def is_source_path(path: str) -> bool:
    """Return whether a tracked path is governed as source code."""
    return PurePosixPath(path).suffix.lower() in SOURCE_SUFFIXES


def is_test_path(path: str) -> bool:
    """Classify source paths that are tests, fixtures, examples, or benchmarks."""
    pure = PurePosixPath(path)
    parts = set(pure.parts)
    name = pure.name.lower()
    return (
        bool(parts & {"benches", "examples", "fixtures", "test", "tests"})
        or name.startswith("test_")
        or "_test." in name
        or ".test." in name
        or name.endswith("tests.rs")
    )


def is_excluded(path: str, prefixes: tuple[str, ...]) -> bool:
    """Return whether a repository-relative path is under an excluded prefix."""
    return any(path.startswith(prefix) for prefix in prefixes)


def source_line_count(root: Path, relative: str) -> int:
    """Read one regular UTF-8 source file and count logical lines."""
    path = root / relative
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError(f"{relative} is not a regular file")
    text = path.read_text(encoding="utf-8")
    return len(text.splitlines())


def collect_counts(
    root: Path,
    paths: Iterable[str],
    excluded_prefixes: tuple[str, ...],
) -> dict[str, int]:
    """Count governed candidate sources, failing closed on unreadable inputs."""
    counts: dict[str, int] = {}
    for path in paths:
        if not is_source_path(path) or is_excluded(path, excluded_prefixes):
            continue
        counts[path] = source_line_count(root, path)
    return counts


def limit_for(path: str, budget: Budget) -> int:
    """Return the default limit for one source path."""
    return budget.test_limit if is_test_path(path) else budget.production_limit


def evaluate(
    counts: dict[str, int],
    budget: Budget,
    *,
    require_objective: bool = False,
) -> list[Finding]:
    """Compare observed line counts with exact ratchets and aggregate policy."""
    findings: list[Finding] = []
    for path, lines in sorted(counts.items()):
        default_limit = limit_for(path, budget)
        baseline = budget.exceptions.get(path)
        if baseline is None:
            if lines > default_limit:
                findings.append(
                    Finding(
                        path,
                        f"{lines} lines exceeds the {default_limit}-line "
                        f"{'test' if is_test_path(path) else 'production'} limit",
                    )
                )
            continue
        if baseline <= default_limit:
            findings.append(
                Finding(
                    path,
                    f"stale exception {baseline} is not above the {default_limit}-line limit",
                )
            )
        elif lines > baseline:
            findings.append(
                Finding(path, f"grew from baseline {baseline} to {lines} lines")
            )
        elif lines < baseline:
            findings.append(
                Finding(
                    path,
                    f"shrunk from baseline {baseline} to {lines} lines; "
                    "refresh the baseline to ratchet it down",
                )
            )

    for path in sorted(set(budget.exceptions) - set(counts)):
        findings.append(Finding(path, "stale exception for a missing or excluded source"))
    if budget.aggregate_rust is not None:
        rust_lines = sum(
            lines for path, lines in counts.items() if path.endswith(".rs")
        )
        aggregate_limit = (
            budget.aggregate_rust.ceiling
            if require_objective
            else budget.aggregate_rust.ratchet_ceiling
        )
        aggregate_limit_name = (
            "aggregate objective ceiling"
            if require_objective
            else "aggregate ratchet"
        )
        if rust_lines > aggregate_limit:
            findings.append(
                Finding(
                    "<aggregate Rust>",
                    f"{rust_lines} lines exceeds the {aggregate_limit_name} "
                    f"{aggregate_limit}",
                )
            )
    return findings


def baseline_payload(
    counts: dict[str, int],
    *,
    production_limit: int,
    test_limit: int,
    excluded_prefixes: tuple[str, ...],
    aggregate_rust: AggregateRustBudget | None = None,
) -> dict[str, Any]:
    """Build a deterministic exact baseline for currently oversized sources."""
    provisional = Budget(
        production_limit=production_limit,
        test_limit=test_limit,
        excluded_prefixes=excluded_prefixes,
        exceptions={},
        aggregate_rust=aggregate_rust,
    )
    exceptions = {
        path: lines
        for path, lines in sorted(counts.items())
        if lines > limit_for(path, provisional)
    }
    payload: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "limits": {
            "production": production_limit,
            "test": test_limit,
        },
        "excluded_prefixes": list(excluded_prefixes),
        "exceptions": exceptions,
    }
    if aggregate_rust is not None:
        payload["aggregate_rust"] = {
            "baseline": aggregate_rust.baseline,
            "ceiling": aggregate_rust.ceiling,
            "ratchet_ceiling": aggregate_rust.ratchet_ceiling,
        }
        if aggregate_rust.working_target is not None:
            payload["aggregate_rust"]["working_target"] = (
                aggregate_rust.working_target
            )
    return payload


def write_json(payload: dict[str, Any], target: Path) -> None:
    """Write stable JSON to a path or stdout."""
    rendered = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if target == Path("-"):
        sys.stdout.write(rendered)
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(rendered, encoding="utf-8")


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    baseline_path = (
        args.baseline
        if args.baseline.is_absolute()
        else root / args.baseline
    )

    try:
        if args.write_baseline:
            production_limit = DEFAULT_PRODUCTION_LIMIT
            test_limit = DEFAULT_TEST_LIMIT
            excluded_prefixes = DEFAULT_EXCLUDED_PREFIXES
            aggregate_rust = None
            if not baseline_path.exists():
                raise ValueError(
                    "refusing to create an unreviewed source budget without "
                    "the mandatory aggregate_rust provenance contract"
                )
            current = load_budget(baseline_path)
            production_limit = current.production_limit
            test_limit = current.test_limit
            excluded_prefixes = current.excluded_prefixes
            aggregate_rust = current.aggregate_rust
            counts = collect_counts(
                root,
                tracked_paths(root),
                excluded_prefixes,
            )
            write_json(
                baseline_payload(
                    counts,
                    production_limit=production_limit,
                    test_limit=test_limit,
                    excluded_prefixes=excluded_prefixes,
                    aggregate_rust=aggregate_rust,
                ),
                baseline_path,
            )
            print(
                f"updated {baseline_path.relative_to(root)} with "
                f"{sum(lines > (test_limit if is_test_path(path) else production_limit) for path, lines in counts.items())} exceptions"
            )
            return 0

        budget = load_budget(baseline_path)
        counts = collect_counts(
            root,
            tracked_paths(root),
            budget.excluded_prefixes,
        )
        findings = evaluate(
            counts,
            budget,
            require_objective=args.require_objective,
        )
    except (OSError, ValueError, json.JSONDecodeError, subprocess.CalledProcessError) as err:
        print(f"ERROR: source file budget check failed: {err}", file=sys.stderr)
        return 2

    report = {
        "schema_version": SCHEMA_VERSION,
        "checked_files": len(counts),
        "exception_files": len(budget.exceptions),
        "production_limit": budget.production_limit,
        "test_limit": budget.test_limit,
        "rust_lines": sum(
            lines for path, lines in counts.items() if path.endswith(".rs")
        ),
        "findings": [
            {"path": finding.path, "message": finding.message}
            for finding in findings
        ],
    }
    if budget.aggregate_rust is not None:
        rust_lines = report["rust_lines"]
        assert isinstance(rust_lines, int)
        report["aggregate_rust"] = {
            "baseline": budget.aggregate_rust.baseline,
            "ceiling": budget.aggregate_rust.ceiling,
            "ratchet_ceiling": budget.aggregate_rust.ratchet_ceiling,
            "working_target": budget.aggregate_rust.working_target,
            "reduction_from_baseline": budget.aggregate_rust.baseline - rust_lines,
            "objective_met": rust_lines <= budget.aggregate_rust.ceiling,
            "gap_to_ceiling": max(0, rust_lines - budget.aggregate_rust.ceiling),
            "headroom_to_ratchet": budget.aggregate_rust.ratchet_ceiling - rust_lines,
            "gap_to_working_target": (
                None
                if budget.aggregate_rust.working_target is None
                else rust_lines - budget.aggregate_rust.working_target
            ),
        }
    human_stream = (
        sys.stderr if args.json_out is not None and args.json_out == Path("-") else sys.stdout
    )
    print(
        f"source_file_budget: checked={len(counts)} "
        f"exceptions={len(budget.exceptions)} rust_lines={report['rust_lines']} "
        f"findings={len(findings)}",
        file=human_stream,
    )
    if budget.aggregate_rust is not None:
        aggregate_report = report["aggregate_rust"]
        print(
            "aggregate_rust: "
            f"baseline={aggregate_report['baseline']} "
            f"goal={aggregate_report['ceiling']} "
            f"ratchet={aggregate_report['ratchet_ceiling']} "
            f"objective_met={str(aggregate_report['objective_met']).lower()} "
            f"gap={aggregate_report['gap_to_ceiling']}",
            file=human_stream,
        )
    for finding in findings:
        print(f"ERROR: {finding.path}: {finding.message}", file=human_stream)

    if args.json_out is not None:
        try:
            target = (
                args.json_out
                if args.json_out == Path("-") or args.json_out.is_absolute()
                else root / args.json_out
            )
            write_json(report, target)
        except OSError as err:
            print(f"ERROR: failed to write source budget report: {err}", file=sys.stderr)
            return 2
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
