#!/usr/bin/env python3
"""Guard the binaries selected by an ordinary workspace build.

The check runs ``cargo metadata --locked`` and compares the resolved default
feature graph with the first-release shipping inventory below. It also pins
the total declared binary count so developer generators, probes, benchmarks,
and evidence tools cannot silently disappear while remaining behind explicit
non-default features such as ``dev-tools``.
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path
from typing import Any


EXPECTED_DEFAULT_BINS = frozenset(
    {
        ("iroha_cli", "iroha"),
        ("iroha_kagami", "kagami"),
        ("iroha_monitor", "iroha_monitor"),
        ("iroha_python_rs", "iroha_privacy_wallet_worker"),
        ("iroha_torii", "attachment_sanitizer"),
        ("irohad", "iroha3d"),
        ("irohad", "iroha3d_taira"),
        ("irohad", "sorafs_governance_dag"),
        ("irohad", "taira_bootle_lantern_broker"),
        ("ivm", "koto"),
        ("izanami", "izanami"),
        ("mochi-ui", "mochi"),
        ("musubi", "musubi"),
        ("sora-vpn-backend", "sora-vpn-backend"),
        ("sora-vpn-helper", "sora-vpn-controller"),
        ("soradns-resolver", "soradns-resolver"),
        ("sorafs_car", "sorafs_fetch"),
        ("sorafs_car", "sorafs_manifest_builder"),
        ("sorafs_car", "sorafs_tx_stdin_builder"),
        ("sorafs_manifest", "sorafs-validate"),
        ("sorafs_node", "sorafs-node"),
        ("sorafs_orchestrator", "sorafs_cli"),
        ("soranet-puzzle-service", "soranet-puzzle-service"),
        ("soranet-relay", "directory"),
        ("soranet-relay", "soranet-relay"),
    }
)

FORBIDDEN_COMPATIBILITY_BINS = frozenset(
    {"iroha2", "iroha2d", "iroha3", "iroha_cli", "irohad"}
)
BASELINE_DEFAULT_BIN_COUNT = 92
MAX_DEFAULT_BIN_COUNT = 25
BASELINE_DECLARED_BIN_COUNT = 116
EXPECTED_DECLARED_BIN_COUNT = 112


def load_metadata(root: Path) -> dict[str, Any]:
    """Load the locked Cargo metadata graph for ``root``."""

    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return json.loads(result.stdout)


def resolved_default_bins(metadata: dict[str, Any]) -> set[tuple[str, str]]:
    """Return workspace binaries enabled by the resolved default feature graph."""

    workspace_members = set(metadata["workspace_members"])
    resolved_features = {
        node["id"]: set(node.get("features", ()))
        for node in metadata["resolve"]["nodes"]
    }
    enabled: set[tuple[str, str]] = set()
    for package in metadata["packages"]:
        package_id = package["id"]
        if package_id not in workspace_members:
            continue
        features = resolved_features.get(package_id, set())
        for target in package["targets"]:
            if "bin" not in target["kind"]:
                continue
            required = set(target.get("required-features") or ())
            if required <= features:
                enabled.add((package["name"], target["name"]))
    return enabled


def all_workspace_bins(metadata: dict[str, Any]) -> set[tuple[str, str]]:
    """Return every binary target declared by workspace packages."""

    workspace_members = set(metadata["workspace_members"])
    return {
        (package["name"], target["name"])
        for package in metadata["packages"]
        if package["id"] in workspace_members
        for target in package["targets"]
        if "bin" in target["kind"]
    }


def check_metadata(metadata: dict[str, Any]) -> list[str]:
    """Return deterministic target-inventory violations."""

    errors: list[str] = []
    actual = resolved_default_bins(metadata)
    missing = sorted(EXPECTED_DEFAULT_BINS - actual)
    unexpected = sorted(actual - EXPECTED_DEFAULT_BINS)
    if missing:
        errors.append(f"shipping binaries no longer enabled by default: {missing!r}")
    if unexpected:
        errors.append(f"non-shipping binaries enabled by default: {unexpected!r}")
    if len(actual) > MAX_DEFAULT_BIN_COUNT:
        errors.append(
            f"default binary count {len(actual)} exceeds {MAX_DEFAULT_BIN_COUNT} "
            f"(pre-refactor baseline: {BASELINE_DEFAULT_BIN_COUNT})"
        )

    declared = all_workspace_bins(metadata)
    if len(declared) != EXPECTED_DECLARED_BIN_COUNT:
        errors.append(
            f"declared binary count {len(declared)} differs from the expected "
            f"{EXPECTED_DECLARED_BIN_COUNT} after retiring obsolete aliases and "
            "adding the four reviewed first-release targets "
            f"(pre-refactor baseline: {BASELINE_DECLARED_BIN_COUNT})"
        )

    forbidden = sorted(
        target
        for target in declared
        if target[1] in FORBIDDEN_COMPATIBILITY_BINS
    )
    if forbidden:
        errors.append(f"retired compatibility binaries are declared: {forbidden!r}")
    return errors


def main() -> int:
    """Run the workspace target-inventory guard."""

    parser = argparse.ArgumentParser(
        description="Reject non-shipping default binaries and retired aliases."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root containing Cargo.toml (default: inferred)",
    )
    args = parser.parse_args()

    errors = check_metadata(load_metadata(args.root.resolve()))
    if errors:
        print("Workspace target inventory violations:")
        for error in errors:
            print(f"  - {error}")
        return 1

    reduction = BASELINE_DEFAULT_BIN_COUNT - len(EXPECTED_DEFAULT_BINS)
    print(
        "Workspace target inventory passed: "
        f"{len(EXPECTED_DEFAULT_BINS)} default binaries "
        f"({reduction} fewer than the {BASELINE_DEFAULT_BIN_COUNT}-target baseline), "
        f"{EXPECTED_DECLARED_BIN_COUNT} total declared binaries"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
