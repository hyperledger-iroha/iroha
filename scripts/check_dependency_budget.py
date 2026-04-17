#!/usr/bin/env python3
"""Report and optionally enforce the workspace dependency budget."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path


DEFAULT_WATCHED_PACKAGES = (
    "cpal",
    "criterion",
    "egui_plot",
    "eframe",
    "trybuild",
    "qrcode",
    "image",
    "openssl",
    "serde_json",
    "proptest",
    "quinn",
    "rcgen",
)

DEFAULT_DENIED_PACKAGES = (
    "cpal",
    "egui_plot",
    "eframe",
    "qrcode",
    "image",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run cargo metadata and report the resolved package graph. "
            "The command is locked by default so dependency budget checks do "
            "not rewrite Cargo.lock."
        )
    )
    parser.add_argument(
        "--manifest-path",
        type=Path,
        default=Path("Cargo.toml"),
        help="Cargo manifest to inspect.",
    )
    parser.add_argument(
        "--allow-lock-update",
        action="store_true",
        help="Omit --locked. This may rewrite Cargo.lock.",
    )
    parser.add_argument(
        "--max-registry-packages",
        type=int,
        help="Fail if the resolved crates.io package count is above this value.",
    )
    parser.add_argument(
        "--max-total-packages",
        type=int,
        help="Fail if the resolved total package count is above this value.",
    )
    parser.add_argument(
        "--watch",
        action="append",
        default=[],
        help="Additional package name to report. May be repeated.",
    )
    parser.add_argument(
        "--deny",
        action="append",
        default=[],
        help=(
            "Package name that must not appear in the default resolved graph. "
            "May be repeated. Defaults also deny UI/media stacks moved behind "
            "explicit features."
        ),
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
    try:
        output = subprocess.check_output(cmd, text=True)
    except subprocess.CalledProcessError as err:
        print(err, file=sys.stderr)
        return {}
    return json.loads(output)


def package_source(package: dict) -> str:
    source = package.get("source")
    if source is None:
        return "path"
    if source.startswith("registry+"):
        return "registry"
    if source.startswith("git+"):
        return "git"
    return "other"


def main() -> int:
    args = parse_args()
    metadata = cargo_metadata(args)
    if not metadata:
        return 1

    resolved_ids = {node["id"] for node in metadata.get("resolve", {}).get("nodes", [])}
    packages = [
        package
        for package in metadata["packages"]
        if not resolved_ids or package["id"] in resolved_ids
    ]
    source_counts = Counter(package_source(package) for package in packages)
    denied = sorted(set(DEFAULT_DENIED_PACKAGES).union(args.deny))
    watched = sorted(set(DEFAULT_WATCHED_PACKAGES).union(args.watch).union(denied))

    print(f"total_packages={len(packages)}")
    print(f"registry_packages={source_counts['registry']}")
    print(f"path_packages={source_counts['path']}")
    print(f"git_packages={source_counts['git']}")

    by_name: dict[str, list[str]] = {name: [] for name in watched}
    for package in packages:
        name = package["name"]
        if name in by_name:
            by_name[name].append(package["version"])

    print("watched_packages:")
    for name in watched:
        versions = sorted(set(by_name[name]))
        rendered = ", ".join(versions) if versions else "-"
        print(f"  {name}: {rendered}")

    failed = False
    for name in denied:
        versions = sorted(set(by_name[name]))
        if versions:
            print(
                f"denied package present: {name} ({', '.join(versions)})",
                file=sys.stderr,
            )
            failed = True
    if (
        args.max_registry_packages is not None
        and source_counts["registry"] > args.max_registry_packages
    ):
        print(
            "registry package budget exceeded: "
            f"{source_counts['registry']} > {args.max_registry_packages}",
            file=sys.stderr,
        )
        failed = True
    if args.max_total_packages is not None and len(packages) > args.max_total_packages:
        print(
            f"total package budget exceeded: {len(packages)} > {args.max_total_packages}",
            file=sys.stderr,
        )
        failed = True

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
