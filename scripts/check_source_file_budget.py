#!/usr/bin/env python3
"""Enforce ratcheting line-count budgets for candidate source files."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import stat
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable


_PROVENANCE_MODULE_NAME = "_iroha_build_efficiency_provenance"
_PROVENANCE_PATH = Path(__file__).with_name("check_build_efficiency_provenance.py")
_PROVENANCE_SPEC = importlib.util.spec_from_file_location(
    _PROVENANCE_MODULE_NAME,
    _PROVENANCE_PATH,
)
if _PROVENANCE_SPEC is None or _PROVENANCE_SPEC.loader is None:
    raise RuntimeError(f"cannot load provenance checker from {_PROVENANCE_PATH}")
if _PROVENANCE_MODULE_NAME in sys.modules:
    provenance = sys.modules[_PROVENANCE_MODULE_NAME]
else:
    provenance = importlib.util.module_from_spec(_PROVENANCE_SPEC)
    sys.modules[_PROVENANCE_MODULE_NAME] = provenance
    _PROVENANCE_SPEC.loader.exec_module(provenance)


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
    mode.add_argument(
        "--base-ref",
        help=(
            "Compare the strict objective findings with a Git base commit and "
            "fail for new or worsened debt. Unchanged or reduced base-branch "
            "debt remains visible without blocking unrelated pull requests."
        ),
    )
    parser.add_argument(
        "--accepted-ref",
        help=(
            "Additional signed repair commit whose inherited debt may be used "
            "with --base-ref. The ref must match the build-efficiency "
            "provenance anchor and be an ancestor of HEAD."
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


def parse_budget(payload: Any) -> Budget:
    """Validate and decode one source budget payload."""
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
        if normalized in exceptions:
            raise ValueError(
                f"source budget repeats normalized exception path {normalized!r}"
            )
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


def load_budget(path: Path) -> Budget:
    """Load and strictly validate a source budget baseline."""
    payload = provenance.strict_json_file(path, str(path))
    return parse_budget(payload)


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


def is_rust_path(path: str) -> bool:
    """Return whether a tracked path is Rust source, ignoring suffix case."""
    return PurePosixPath(path).suffix.lower() == ".rs"


def is_test_path(path: str) -> bool:
    """Classify source paths that are tests, fixtures, examples, or benchmarks."""
    pure = PurePosixPath(path)
    parts = {part.lower() for part in pure.parts}
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


def resolve_git_commit(store: Any, ref: str, label: str) -> str:
    """Resolve one ref through the provenance checker's hardened Git reader."""
    if not isinstance(ref, str) or not ref or "\0" in ref:
        raise ValueError(f"{label} must be a non-empty Git ref")
    raw = store._run(  # The checker intentionally centralizes sanitized Git I/O.
        "rev-parse",
        "--verify",
        "--end-of-options",
        f"{ref}^{{commit}}",
    ).stdout
    try:
        commit = provenance.require_oid(raw.decode("ascii").strip(), label)
    except UnicodeError as error:
        raise ValueError(f"{label} resolved to a non-ASCII commit id") from error
    commit_bytes = store.object_bytes(commit, "commit")
    provenance.verify_object_id(commit, "commit", commit_bytes)
    return commit


def collect_counts_at_git_ref(
    root: Path,
    ref: str,
    excluded_prefixes: tuple[str, ...],
    *,
    store: Any | None = None,
) -> tuple[dict[str, int], str]:
    """Count governed source lines from one exact Git commit without checkout."""
    object_store = provenance.GitObjectStore(root) if store is None else store
    commit = resolve_git_commit(object_store, ref, "source budget ref")
    entries = [
        entry
        for entry in object_store.tree_entries(commit)
        if is_source_path(entry.path)
        and not is_excluded(entry.path, excluded_prefixes)
    ]
    for entry in entries:
        if (
            entry.object_type != "blob"
            or entry.mode not in provenance.REGULAR_FILE_MODES
        ):
            raise ValueError(
                f"{entry.path} is not a regular source blob at {commit}"
            )
    blobs = object_store.blob_bytes_many([entry.oid for entry in entries])
    counts: dict[str, int] = {}
    for entry in entries:
        payload = blobs[entry.oid]
        provenance.verify_object_id(entry.oid, "blob", payload)
        try:
            counts[entry.path] = len(payload.decode("utf-8").splitlines())
        except UnicodeError as error:
            raise ValueError(
                f"source budget source is not UTF-8 at {commit}: {entry.path}"
            ) from error
    return counts, commit


def load_budget_at_git_ref(store: Any, commit: str, relative: str) -> Budget:
    """Load an exact strict source budget from a commit tree."""
    path = provenance.require_safe_path(relative, "source budget path")
    entry = store.tree_entry(commit, path)
    if entry is None:
        raise ValueError(f"source budget path {path!r} is missing at {commit}")
    if (
        entry.object_type != "blob"
        or entry.mode not in provenance.REGULAR_FILE_MODES
    ):
        raise ValueError(f"source budget path {path!r} is not a regular blob")
    blob = store.object_bytes(entry.oid, "blob")
    provenance.verify_object_id(entry.oid, "blob", blob)
    try:
        text = blob.decode("utf-8")
    except UnicodeError as error:
        raise ValueError(f"source budget path {path!r} is not UTF-8") from error
    payload = provenance.strict_json_loads(text, f"{path} at {commit}")
    return parse_budget(payload)


def limit_for(path: str, budget: Budget) -> int:
    """Return the default limit for one source path."""
    return budget.test_limit if is_test_path(path) else budget.production_limit


def validate_candidate_budget_policy(candidate: Budget, floor: Budget) -> None:
    """Reject candidate policy changes that weaken a committed comparison floor."""
    if candidate.production_limit > floor.production_limit:
        raise ValueError("candidate production limit exceeds the comparison floor")
    if candidate.test_limit > floor.test_limit:
        raise ValueError("candidate test limit exceeds the comparison floor")

    added_exclusions = sorted(
        candidate_prefix
        for candidate_prefix in candidate.excluded_prefixes
        if not any(
            candidate_prefix.startswith(floor_prefix)
            for floor_prefix in floor.excluded_prefixes
        )
    )
    if added_exclusions:
        raise ValueError(
            "candidate source budget expands excluded prefixes: "
            f"{', '.join(added_exclusions)}"
        )

    added_exceptions = sorted(set(candidate.exceptions) - set(floor.exceptions))
    if added_exceptions:
        raise ValueError(
            "candidate source budget adds exceptions: "
            f"{', '.join(added_exceptions)}"
        )
    raised_exceptions = sorted(
        path
        for path, limit in candidate.exceptions.items()
        if limit > floor.exceptions[path]
    )
    if raised_exceptions:
        raise ValueError(
            "candidate source budget raises exceptions: "
            f"{', '.join(raised_exceptions)}"
        )

    candidate_aggregate = candidate.aggregate_rust
    floor_aggregate = floor.aggregate_rust
    if candidate_aggregate is None or floor_aggregate is None:
        raise ValueError("candidate and comparison budgets require aggregate_rust")
    if candidate_aggregate.baseline != floor_aggregate.baseline:
        raise ValueError("candidate aggregate Rust baseline changed")
    if candidate_aggregate.ceiling != floor_aggregate.ceiling:
        raise ValueError("candidate aggregate Rust ceiling changed")
    if candidate_aggregate.ratchet_ceiling > floor_aggregate.ratchet_ceiling:
        raise ValueError("candidate aggregate Rust ratchet ceiling increased")
    floor_target = floor_aggregate.working_target
    candidate_target = candidate_aggregate.working_target
    if floor_target is not None and (
        candidate_target is None or candidate_target > floor_target
    ):
        raise ValueError("candidate aggregate Rust working target increased")


def invalid_current_exception_paths(
    counts: dict[str, int], budget: Budget
) -> set[str]:
    """Return exception paths whose current tree is not their exact ratchet."""
    invalid: set[str] = set()
    for path, baseline in budget.exceptions.items():
        lines = counts.get(path)
        if (
            lines is None
            or baseline <= limit_for(path, budget)
            or lines != baseline
        ):
            invalid.add(path)
    return invalid


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
            lines for path, lines in counts.items() if is_rust_path(path)
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


def evaluate_against_base(
    counts: dict[str, int],
    comparison_counts: dict[str, int],
    budget: Budget,
    *,
    comparison_budget: Budget | None = None,
) -> tuple[list[Finding], list[Finding]]:
    """Compare the candidate with one topology-selected committed floor."""
    floor_budget = budget if comparison_budget is None else comparison_budget
    base_finding_paths = {
        finding.path
        for finding in evaluate(
            comparison_counts,
            floor_budget,
            require_objective=True,
        )
    }
    current = evaluate(counts, budget, require_objective=True)
    exact_exception_failures = invalid_current_exception_paths(counts, budget)
    current_rust_lines = sum(
        lines for path, lines in counts.items() if is_rust_path(path)
    )
    comparison_rust_lines = sum(
        lines for path, lines in comparison_counts.items() if is_rust_path(path)
    )
    candidate_only = []
    inherited = []
    for finding in current:
        if finding.path in exact_exception_failures:
            candidate_only.append(finding)
            continue
        if finding.path not in base_finding_paths:
            candidate_only.append(finding)
            continue
        if finding.path == "<aggregate Rust>":
            worsened = current_rust_lines > comparison_rust_lines
        else:
            current_lines = counts.get(finding.path)
            comparison_lines = comparison_counts.get(finding.path)
            worsened = (
                current_lines is None
                or comparison_lines is None
                or current_lines > comparison_lines
            )
        (candidate_only if worsened else inherited).append(finding)
    return candidate_only, inherited


def validate_accepted_ref(
    root: Path,
    ref: str,
    *,
    candidate_commit: str | None = None,
    store: Any | None = None,
) -> str:
    """Resolve an accepted ref after validating all pinned provenance."""
    manifest_path = root / "ci/build_efficiency_provenance.json"
    payload = provenance.strict_json_file(
        manifest_path,
        "ci/build_efficiency_provenance.json",
    )
    object_store = provenance.GitObjectStore(root) if store is None else store
    head = (
        object_store.head()
        if candidate_commit is None
        else provenance.require_oid(candidate_commit, "source-budget HEAD snapshot")
    )
    provenance.validate_provenance(
        root,
        payload,
        object_store,
        head_commit=head,
    )
    # Full schema and Git validation above makes this lookup trusted.
    expected = payload["lineage"]["signed_lock_anchor"]["commit"]
    commit = resolve_git_commit(object_store, ref, "accepted source-budget ref")
    if commit != expected:
        raise ValueError(
            "accepted source-budget ref does not match the signed lock anchor"
        )
    if commit == head or not object_store.is_ancestor(commit, head):
        raise ValueError(
            "accepted source-budget ref must be a strict ancestor of HEAD"
        )
    return commit


def validate_comparison_topology(
    store: Any,
    base_commit: str,
    accepted_commit: str | None = None,
    *,
    candidate_commit: str | None = None,
) -> str:
    """Return the newest comparable floor below the candidate HEAD."""
    head = (
        store.head()
        if candidate_commit is None
        else provenance.require_oid(candidate_commit, "source-budget HEAD snapshot")
    )
    if base_commit == head or not store.is_ancestor(base_commit, head):
        raise ValueError("source-budget base must be a strict ancestor of HEAD")
    if accepted_commit is None:
        return base_commit
    if accepted_commit == head or not store.is_ancestor(accepted_commit, head):
        raise ValueError(
            "accepted source-budget ref must be a strict ancestor of HEAD"
        )
    if base_commit == accepted_commit:
        return base_commit
    if store.is_ancestor(base_commit, accepted_commit):
        return accepted_commit
    if store.is_ancestor(accepted_commit, base_commit):
        return base_commit
    raise ValueError(
        "source-budget base and accepted ref must be comparable ancestors of HEAD"
    )


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
        if args.accepted_ref is not None and args.base_ref is None:
            raise ValueError("--accepted-ref requires --base-ref")
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

        base_commit = None
        accepted_commit = None
        comparison_commit = None
        inherited_findings: list[Finding] = []
        if args.base_ref is not None:
            object_store = provenance.GitObjectStore(root)
            candidate_commit = resolve_git_commit(
                object_store,
                "HEAD",
                "source-budget candidate ref",
            )
            base_commit = resolve_git_commit(
                object_store,
                args.base_ref,
                "source-budget base ref",
            )
            try:
                baseline_relative = baseline_path.resolve().relative_to(root).as_posix()
            except ValueError as error:
                raise ValueError(
                    "source budget baseline must be inside the repository for "
                    "Git comparison"
                ) from error
            budget = load_budget_at_git_ref(
                object_store,
                candidate_commit,
                baseline_relative,
            )
            counts, resolved_candidate_commit = collect_counts_at_git_ref(
                root,
                candidate_commit,
                budget.excluded_prefixes,
                store=object_store,
            )
            if resolved_candidate_commit != candidate_commit:
                raise ValueError("source-budget candidate commit identity changed")
            if args.accepted_ref is not None:
                accepted_commit = validate_accepted_ref(
                    root,
                    args.accepted_ref,
                    candidate_commit=candidate_commit,
                    store=object_store,
                )
            comparison_commit = validate_comparison_topology(
                object_store,
                base_commit,
                accepted_commit,
                candidate_commit=candidate_commit,
            )
            comparison_budget = load_budget_at_git_ref(
                object_store,
                comparison_commit,
                baseline_relative,
            )
            validate_candidate_budget_policy(budget, comparison_budget)
            comparison_counts, resolved_comparison_commit = collect_counts_at_git_ref(
                root,
                comparison_commit,
                comparison_budget.excluded_prefixes,
                store=object_store,
            )
            if resolved_comparison_commit != comparison_commit:
                raise ValueError("source-budget comparison commit identity changed")
            findings, inherited_findings = evaluate_against_base(
                counts,
                comparison_counts,
                budget,
                comparison_budget=comparison_budget,
            )
            if object_store.head() != candidate_commit:
                raise ValueError("HEAD changed during source-budget validation")
        else:
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
    except (OSError, ValueError) as err:
        print(f"ERROR: source file budget check failed: {err}", file=sys.stderr)
        return 2

    report = {
        "schema_version": SCHEMA_VERSION,
        "checked_files": len(counts),
        "exception_files": len(budget.exceptions),
        "production_limit": budget.production_limit,
        "test_limit": budget.test_limit,
        "rust_lines": sum(
            lines for path, lines in counts.items() if is_rust_path(path)
        ),
        "findings": [
            {"path": finding.path, "message": finding.message}
            for finding in findings
        ],
    }
    if base_commit is not None:
        report["base_comparison"] = {
            "commit": base_commit,
            "accepted_commit": accepted_commit,
            "floor_commit": comparison_commit,
            "inherited_finding_paths": sorted(
                {finding.path for finding in inherited_findings}
            ),
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
    if base_commit is not None:
        print(
            "base_comparison: "
            f"commit={base_commit} "
            f"accepted={accepted_commit or 'none'} "
            f"floor={comparison_commit} "
            f"inherited={len({finding.path for finding in inherited_findings})} "
            f"candidate_only={len(findings)}",
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
