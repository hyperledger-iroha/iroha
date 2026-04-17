#!/usr/bin/env python3
"""Report and optionally enforce Cargo compile-unit budget."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter, deque
from pathlib import Path


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
    parser.add_argument(
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
        "--max-compile-units",
        type=int,
        help="Fail if the unique compiler-artifact count is above this value.",
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
    return cmd


def main() -> int:
    args = parse_args()
    try:
        metadata = cargo_metadata(args)
    except subprocess.CalledProcessError as err:
        return err.returncode

    packages = {package["id"]: package for package in metadata["packages"]}
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
        if message.get("reason") != "compiler-artifact":
            continue
        package_id = message.get("package_id")
        target = message.get("target") or {}
        if not package_id or not target:
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

    print(f"compile_units={len(artifacts)}")
    print(f"artifact_packages={len(artifact_package_ids)}")
    print(f"registry_packages={source_counts['registry']}")
    print(f"path_packages={source_counts['path']}")
    print(f"git_packages={source_counts['git']}")
    print("top_packages:")
    for name, count in package_artifacts.most_common(20):
        print(f"  {name}: {count}")

    if (
        args.max_compile_units is not None
        and len(artifacts) > args.max_compile_units
    ):
        print(
            f"ERROR: compile-unit count {len(artifacts)} exceeds budget "
            f"{args.max_compile_units}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
