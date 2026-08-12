#!/usr/bin/env python3
"""Report and optionally enforce Cargo compile-unit budget."""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import subprocess
import sys
from collections import Counter, deque
from pathlib import Path
from typing import Any


REPORT_SCHEMA_VERSION = 2
ARTIFACT_IDENTITY = "cargo-package-target-features-profile-v2"
ArtifactIdentity = tuple[
    str,
    str,
    tuple[str, ...],
    tuple[str, ...],
    str,
    tuple[str, ...],
    str,
]


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
            "Optional schema-v2 JSON baseline. Accepts either a complete report "
            "or a keyed measurement contract selected by --baseline-key."
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


def artifact_identity(message: dict[str, Any]) -> ArtifactIdentity | None:
    """Return the complete stable identity of one Cargo compiler artifact."""

    package_id = message.get("package_id")
    target = message.get("target")
    if not isinstance(package_id, str) or not isinstance(target, dict):
        return None
    profile = message.get("profile")
    features = message.get("features")
    if not isinstance(profile, dict) or not isinstance(features, list):
        return None
    if not all(isinstance(feature, str) for feature in features):
        return None

    def target_strings(field: str) -> tuple[str, ...] | None:
        values = target.get(field)
        if not isinstance(values, list) or not all(
            isinstance(value, str) for value in values
        ):
            return None
        return tuple(sorted(values))

    kinds = target_strings("kind")
    crate_types = target_strings("crate_types")
    name = target.get("name")
    source_path = target.get("src_path")
    if (
        kinds is None
        or crate_types is None
        or not isinstance(name, str)
        or not isinstance(source_path, str)
    ):
        return None
    try:
        profile_identity = json.dumps(
            profile,
            sort_keys=True,
            separators=(",", ":"),
        )
    except (TypeError, ValueError):
        return None
    return (
        package_id,
        name,
        kinds,
        crate_types,
        source_path,
        tuple(sorted(features)),
        profile_identity,
    )


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


def load_baseline(path: Path, key: str | None) -> dict[str, Any]:
    """Load one schema-v2 compile-unit measurement contract."""

    document: Any = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, dict):
        raise ValueError(f"baseline document in {path} must be a JSON object")
    if document.get("schema_version") != REPORT_SCHEMA_VERSION:
        raise ValueError(
            f"baseline document in {path} must use schema_version "
            f"{REPORT_SCHEMA_VERSION}"
        )
    payload: Any = document
    if key is not None:
        if key not in document:
            raise ValueError(f"baseline key `{key}` is missing from {path}")
        payload = document[key]
    if not isinstance(payload, dict):
        raise ValueError(f"baseline payload in {path} must be a JSON object")
    value = payload.get("compile_units")
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(
            f"baseline payload in {path} must contain a non-negative integer compile_units"
        )
    return payload


def parse_rustc_release(output: str) -> str:
    """Extract the exact release string from ``rustc --version --verbose``."""

    for line in output.splitlines():
        if line.startswith("release: "):
            release = line.removeprefix("release: ").strip()
            if re.fullmatch(
                r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", release
            ):
                return release
    first_line = output.splitlines()[0] if output.splitlines() else ""
    match = re.fullmatch(
        r"rustc ([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?) \(.+\)",
        first_line,
    )
    if match is None:
        raise ValueError("rustc output does not contain an exact release")
    return match.group(1)


def rustc_release() -> str:
    """Return the compiler release that Cargo will select in this checkout."""

    compiler = os.environ.get("RUSTC", "rustc")
    output = subprocess.check_output(
        [compiler, "--version", "--verbose"],
        text=True,
    )
    return parse_rustc_release(output)


def measurement_contract(
    args: argparse.Namespace,
    *,
    toolchain: str,
) -> dict[str, Any]:
    """Describe every input that makes a compile-unit baseline comparable."""

    return {
        "artifact_identity": ARTIFACT_IDENTITY,
        "artifact_scope": args.artifact_scope,
        "budget_min_growth": args.budget_min_growth,
        "budget_percent": (
            int(args.budget_percent)
            if args.budget_percent.is_integer()
            else args.budget_percent
        ),
        "cargo_locked": not args.allow_lock_update,
        "manifest_path": args.manifest_path.as_posix(),
        "packages": sorted(set(args.package)),
        "target": "lib" if args.lib else "all",
        "toolchain": toolchain,
        "workspace": args.workspace,
    }


def validate_baseline_contract(
    baseline: dict[str, Any],
    observed: dict[str, Any],
) -> None:
    """Reject a baseline recorded for a different compiler or Cargo scope."""

    errors = []
    for field, actual in observed.items():
        if field not in baseline:
            errors.append(f"missing `{field}`")
        elif (
            type(baseline[field]) is not type(actual)
            or baseline[field] != actual
        ):
            errors.append(
                f"`{field}` is {baseline[field]!r}, expected {actual!r}"
            )
    if errors:
        raise ValueError("baseline measurement contract mismatch: " + "; ".join(errors))


def build_report(
    *,
    command: list[str],
    artifacts: set[ArtifactIdentity],
    artifact_package_ids: set[str],
    source_counts: Counter[str],
    package_artifacts: Counter[str],
    baseline: int | None,
    limit: int | None,
    contract: dict[str, Any],
) -> dict[str, Any]:
    """Build the stable JSON report emitted by the compile-unit guard."""
    compile_units = len(artifacts)
    return {
        "schema_version": REPORT_SCHEMA_VERSION,
        **contract,
        "command": command,
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
    print(f"artifact_identity={report['artifact_identity']}", file=stream)
    print(f"toolchain={report['toolchain']}", file=stream)
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
    baseline_entry: dict[str, Any] | None = None
    baseline_budget: int | None = None
    if args.baseline is not None:
        try:
            baseline_entry = load_baseline(args.baseline, args.baseline_key)
            baseline = baseline_entry["compile_units"]
        except (OSError, ValueError, json.JSONDecodeError) as err:
            print(f"ERROR: failed to load compile-unit baseline: {err}", file=sys.stderr)
            return 2

    try:
        toolchain = rustc_release()
    except (OSError, ValueError, subprocess.CalledProcessError) as err:
        print(f"ERROR: failed to identify Rust toolchain: {err}", file=sys.stderr)
        return 2
    contract = measurement_contract(args, toolchain=toolchain)
    if baseline_entry is not None:
        try:
            validate_baseline_contract(baseline_entry, contract)
        except ValueError as err:
            print(f"ERROR: failed to load compile-unit baseline: {err}", file=sys.stderr)
            return 2
        assert baseline is not None
        baseline_budget = baseline_limit(
            baseline,
            percent=args.budget_percent,
            minimum_growth=args.budget_min_growth,
        )

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
    artifacts: set[ArtifactIdentity] = set()
    package_artifacts: Counter[str] = Counter()
    malformed_artifacts = 0

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
        key = artifact_identity(message)
        if key is None:
            malformed_artifacts += 1
            continue
        package_id = key[0]
        if not artifact_in_scope(
            package_id, args.artifact_scope, workspace_members
        ):
            continue
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
    if malformed_artifacts:
        print(
            "ERROR: Cargo emitted "
            f"{malformed_artifacts} compiler artifact(s) without the complete "
            "package/target/features/profile identity",
            file=sys.stderr,
        )
        return 2

    source_counts: Counter[str] = Counter()
    artifact_package_ids = {identity[0] for identity in artifacts}
    for package_id in artifact_package_ids:
        package = packages.get(package_id)
        source_counts[package_source(package)] += 1

    report = build_report(
        command=cmd,
        artifacts=artifacts,
        artifact_package_ids=artifact_package_ids,
        source_counts=source_counts,
        package_artifacts=package_artifacts,
        baseline=baseline,
        limit=effective_limit,
        contract=contract,
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
