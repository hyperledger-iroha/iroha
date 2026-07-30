#!/usr/bin/env python3
"""Report and optionally enforce Cargo compile-unit budget."""

from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from collections import Counter, deque
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run `cargo test --no-run --message-format=json` and count unique "
            "compiler artifacts. The command is locked by default so budget "
            "checks do not rewrite Cargo.lock."
        )
    )
    parser.add_argument(
        "--manifest-path",
        type=Path,
        default=Path("Cargo.toml"),
        help="Cargo manifest to test.",
    )
    parser.add_argument(
        "--target-dir",
        type=Path,
        help="Optional target directory for the check build.",
    )
    lock_mode = parser.add_mutually_exclusive_group()
    lock_mode.add_argument(
        "--locked",
        action="store_true",
        help="Require the existing Cargo.lock (the default; useful for explicit CI commands).",
    )
    lock_mode.add_argument(
        "--allow-lock-update",
        action="store_true",
        help="Omit --locked. This may rewrite Cargo.lock.",
    )
    parser.add_argument(
        "--workspace",
        action="store_true",
        help="Count the full workspace instead of the manifest default members.",
    )
    parser.add_argument(
        "-p",
        "--package",
        action="append",
        default=[],
        help="Package to count. May be repeated.",
    )
    parser.add_argument(
        "--lib",
        action="store_true",
        help="Only compile the selected package library test target.",
    )
    parser.add_argument(
        "--artifact-scope",
        choices=("all", "workspace"),
        default="all",
        help=(
            "Count every compiler artifact or only Cargo workspace members. "
            "The workspace scope avoids host-specific registry dependency drift."
        ),
    )
    parser.add_argument(
        "--max-compile-units",
        type=int,
        help="Fail if the unique compiler-artifact count is above this value.",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        help=(
            "Optional JSON baseline. Accepts either a report containing "
            "`compile_units` or a keyed object selected by --baseline-key."
        ),
    )
    parser.add_argument(
        "--baseline-key",
        help="Select this object from the baseline JSON before reading compile_units.",
    )
    parser.add_argument(
        "--budget-percent",
        type=float,
        default=2.0,
        help="Allowed percentage growth over --baseline (default: 2).",
    )
    parser.add_argument(
        "--budget-min-growth",
        type=int,
        default=3,
        help="Minimum compile-unit growth allowed over --baseline (default: 3).",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write the deterministic report as JSON; use `-` for stdout.",
    )
    return parser.parse_args()


def cargo_metadata(args: argparse.Namespace) -> dict:
    cmd = [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--manifest-path",
        str(args.manifest_path),
    ]
    if not args.allow_lock_update:
        cmd.append("--locked")
    return json.loads(subprocess.check_output(cmd, text=True))


def package_source(package: dict | None) -> str:
    if package is None:
        return "other"
    source = package.get("source")
    if source is None:
        return "path"
    if source.startswith("registry+"):
        return "registry"
    if source.startswith("git+"):
        return "git"
    return "other"


def artifact_in_scope(
    package_id: str, artifact_scope: str, workspace_members: set[str]
) -> bool:
    """Return whether a compiler artifact contributes to the enforced count."""

    if artifact_scope == "all":
        return True
    if artifact_scope == "workspace":
        return package_id in workspace_members
    raise ValueError(f"unsupported artifact scope: {artifact_scope}")


def compiler_diagnostic_lines(message: dict[str, Any]) -> tuple[str, ...]:
    """Extract rendered rustc diagnostics from one Cargo JSON message."""

    if message.get("reason") != "compiler-message":
        return ()
    rendered = (message.get("message") or {}).get("rendered")
    if not isinstance(rendered, str):
        return ()
    return tuple(rendered.rstrip().splitlines())


def cargo_test_command(args: argparse.Namespace) -> list[str]:
    cmd = [
        "cargo",
        "test",
        "--no-run",
        "--message-format=json",
        "--manifest-path",
        str(args.manifest_path),
    ]
    if not args.allow_lock_update:
        cmd.append("--locked")
    if args.target_dir is not None:
        cmd.extend(["--target-dir", str(args.target_dir)])
    if args.workspace:
        cmd.append("--workspace")
    for package in args.package:
        cmd.extend(["-p", package])
    if args.lib:
        cmd.append("--lib")
    return cmd


def baseline_limit(
    baseline: int,
    *,
    percent: float = 2.0,
    minimum_growth: int = 3,
) -> int:
    """Return the ratcheting limit for a checked-in compile-unit baseline."""
    if baseline < 0:
        raise ValueError("baseline must be non-negative")
    if percent < 0:
        raise ValueError("budget percent must be non-negative")
    if minimum_growth < 0:
        raise ValueError("minimum growth must be non-negative")
    percentage_growth = math.ceil(baseline * percent / 100.0)
    return baseline + max(minimum_growth, percentage_growth)


def load_baseline(path: Path, key: str | None) -> int:
    """Load one compile-unit baseline from a deterministic JSON report."""
    payload: Any = json.loads(path.read_text(encoding="utf-8"))
    if key is not None:
        if not isinstance(payload, dict) or key not in payload:
            raise ValueError(f"baseline key `{key}` is missing from {path}")
        payload = payload[key]
    if not isinstance(payload, dict):
        raise ValueError(f"baseline payload in {path} must be a JSON object")
    value = payload.get("compile_units")
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(
            f"baseline payload in {path} must contain a non-negative integer compile_units"
        )
    return value


def build_report(
    *,
    command: list[str],
    artifact_scope: str,
    artifacts: set[tuple[str, str, tuple[str, ...], str]],
    artifact_package_ids: set[str],
    source_counts: Counter[str],
    package_artifacts: Counter[str],
    baseline: int | None,
    limit: int | None,
) -> dict[str, Any]:
    """Build the stable JSON report emitted by the compile-unit guard."""
    compile_units = len(artifacts)
    return {
        "schema_version": 1,
        "command": command,
        "artifact_scope": artifact_scope,
        "compile_units": compile_units,
        "artifact_packages": len(artifact_package_ids),
        "package_sources": {
            "registry": source_counts["registry"],
            "path": source_counts["path"],
            "git": source_counts["git"],
            "other": source_counts["other"],
        },
        "top_packages": [
            {"name": name, "compile_units": count}
            for name, count in sorted(
                package_artifacts.items(), key=lambda item: (-item[1], item[0])
            )[:20]
        ],
        "baseline_compile_units": baseline,
        "limit_compile_units": limit,
        "within_budget": limit is None or compile_units <= limit,
    }


def write_json_report(report: dict[str, Any], target: Path) -> None:
    """Write a deterministic JSON report to a file or stdout."""
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if target == Path("-"):
        sys.stdout.write(rendered)
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(rendered, encoding="utf-8")


def write_human_report(report: dict[str, Any], stream: Any = sys.stdout) -> None:
    """Write the concise report intended for developers and CI logs."""
    print(f"compile_units={report['compile_units']}", file=stream)
    print(f"artifact_packages={report['artifact_packages']}", file=stream)
    print(f"registry_packages={report['package_sources']['registry']}", file=stream)
    print(f"path_packages={report['package_sources']['path']}", file=stream)
    print(f"git_packages={report['package_sources']['git']}", file=stream)
    if report["baseline_compile_units"] is not None:
        print(
            f"baseline_compile_units={report['baseline_compile_units']}",
            file=stream,
        )
    if report["limit_compile_units"] is not None:
        print(f"limit_compile_units={report['limit_compile_units']}", file=stream)
    print("top_packages:", file=stream)
    for entry in report["top_packages"]:
        print(f"  {entry['name']}: {entry['compile_units']}", file=stream)


def main() -> int:
    args = parse_args()
    if args.budget_percent < 0:
        print("ERROR: --budget-percent must be non-negative", file=sys.stderr)
        return 2
    if args.budget_min_growth < 0:
        print("ERROR: --budget-min-growth must be non-negative", file=sys.stderr)
        return 2

    baseline: int | None = None
    baseline_budget: int | None = None
    if args.baseline is not None:
        try:
            baseline = load_baseline(args.baseline, args.baseline_key)
            baseline_budget = baseline_limit(
                baseline,
                percent=args.budget_percent,
                minimum_growth=args.budget_min_growth,
            )
        except (OSError, ValueError, json.JSONDecodeError) as err:
            print(f"ERROR: failed to load compile-unit baseline: {err}", file=sys.stderr)
            return 2

    limits = [
        limit
        for limit in (args.max_compile_units, baseline_budget)
        if limit is not None
    ]
    effective_limit = min(limits) if limits else None

    try:
        metadata = cargo_metadata(args)
    except subprocess.CalledProcessError as err:
        return err.returncode

    packages = {package["id"]: package for package in metadata["packages"]}
    workspace_members = set(metadata["workspace_members"])
    artifacts: set[tuple[str, str, tuple[str, ...], str]] = set()
    package_artifacts: Counter[str] = Counter()

    cmd = cargo_test_command(args)
    process = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    assert process.stdout is not None

    cargo_output: deque[str] = deque(maxlen=200)
    for line in process.stdout:
        line = line.strip()
        if not line:
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            cargo_output.append(line)
            continue
        diagnostic_lines = compiler_diagnostic_lines(message)
        if diagnostic_lines:
            cargo_output.extend(diagnostic_lines)
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        package_id = message.get("package_id")
        target = message.get("target") or {}
        if not package_id or not target:
            continue
        if not artifact_in_scope(
            package_id, args.artifact_scope, workspace_members
        ):
            continue
        key = (
            package_id,
            target.get("name", ""),
            tuple(target.get("kind", [])),
            target.get("src_path", ""),
        )
        if key in artifacts:
            continue
        artifacts.add(key)
        package = packages.get(package_id)
        package_name = package["name"] if package else package_id
        package_artifacts[package_name] += 1

    return_code = process.wait()
    if return_code != 0:
        for line in cargo_output:
            print(line, file=sys.stderr)
        return return_code

    source_counts: Counter[str] = Counter()
    artifact_package_ids = {package_id for package_id, _, _, _ in artifacts}
    for package_id in artifact_package_ids:
        package = packages.get(package_id)
        source_counts[package_source(package)] += 1

    report = build_report(
        command=cmd,
        artifact_scope=args.artifact_scope,
        artifacts=artifacts,
        artifact_package_ids=artifact_package_ids,
        source_counts=source_counts,
        package_artifacts=package_artifacts,
        baseline=baseline,
        limit=effective_limit,
    )

    human_stream = (
        sys.stderr if args.json_out is not None and args.json_out == Path("-") else sys.stdout
    )
    write_human_report(report, human_stream)

    if args.json_out is not None:
        try:
            write_json_report(report, args.json_out)
        except OSError as err:
            print(f"ERROR: failed to write JSON report: {err}", file=sys.stderr)
            return 2

    if effective_limit is not None and len(artifacts) > effective_limit:
        print(
            f"ERROR: compile-unit count {len(artifacts)} exceeds budget "
            f"{effective_limit}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
