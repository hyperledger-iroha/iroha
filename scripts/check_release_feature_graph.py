#!/usr/bin/env python3
"""Reject development-only Iroha features from shipping dependency graphs."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


DEFAULT_PACKAGES = (
    "irohad",
    "iroha_cli",
    "iroha_genesis",
    "iroha_kagami",
    "ivm",
)
FORBIDDEN_FEATURES = (
    'iroha feature "test-fixtures"',
    'iroha_core feature "iroha-core-tests"',
    'iroha_data_model feature "test-fixtures"',
    'iroha_p2p feature "test-fixtures"',
    'iroha_sccp feature "test-fixtures"',
)


def feature_graph(repo: Path, package: str) -> str:
    """Return Cargo's normal/build feature graph for one shipping package."""

    completed = subprocess.run(
        [
            "cargo",
            "tree",
            "--locked",
            "--package",
            package,
            "--edges",
            "normal,build,features",
            "--prefix",
            "none",
        ],
        cwd=repo,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"cargo tree failed for {package}:\n{completed.stdout}{completed.stderr}"
        )
    return completed.stdout


def forbidden_features_in_graph(graph: str) -> tuple[str, ...]:
    """Return development-only feature markers present in a Cargo graph."""

    return tuple(feature for feature in FORBIDDEN_FEATURES if feature in graph)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--package",
        action="append",
        dest="packages",
        help="shipping package to inspect (repeatable)",
    )
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    packages = tuple(args.packages or DEFAULT_PACKAGES)
    failures: list[str] = []
    for package in packages:
        graph = feature_graph(repo, package)
        for forbidden in forbidden_features_in_graph(graph):
            failures.append(f"{package}: enabled {forbidden}")
    if failures:
        print("development-only features reached a shipping graph:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("shipping feature graphs exclude Iroha test fixtures")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
