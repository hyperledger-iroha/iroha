#!/usr/bin/env python3
"""Validate explicit ownership of foundational Cargo features.

Prerequisites: Python 3.11+, or Python 3.9/3.10 with the repository's pinned
``tomli`` dependency installed. The check is read-only and requires no
environment variables.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any, Iterable

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - exercised on Python <3.11
    import tomli as tomllib


FOUNDATIONAL_DEPENDENCIES = frozenset(
    {
        "iroha_core",
        "iroha_crypto",
        "iroha_data_model",
        "iroha_torii",
        "ivm",
        "norito",
    }
)

# These aggregates are the stable ownership boundary for shipping builds. The
# implementation features remain available for focused tests and compatibility
# while callers depend on the smaller vocabulary below.
EXPECTED_FEATURES: dict[str, dict[str, tuple[str, ...]]] = {
    "norito": {
        "default": ("node-codec",),
        "base-codec": ("json", "strict-safe"),
        "node-codec": (
            "base-codec",
            "derive",
            "compression",
            "columnar",
            "json-std-io",
        ),
    },
    "iroha_crypto": {
        "default": ("application",),
        "application": ("std", "rand", "json", "ecc-batch", "bfv-accel", "pqc"),
        "consensus": ("std", "rand", "bls", "bls-multi-pairing"),
        "node-crypto": ("application", "consensus"),
    },
    "iroha_data_model": {
        "default": ("application-model",),
        "application-model": ("governance", "json", "pqc"),
    },
    "ivm": {
        "default": ("runtime",),
        "runtime": ("halo2",),
    },
    "iroha_core": {
        "default": ("node",),
        "runtime": ("json", "bls", "bls-multi-pairing", "fast_dsl", "proofs-halo2"),
        "node": ("runtime", "proofs-stark"),
        "proofs-halo2": ("zk-halo2", "zk-halo2-ipa", "zk-ipa-native", "circuit-params"),
        "proofs-stark": ("zk-stark",),
        "proofs-full": ("proofs-halo2", "proofs-stark"),
    },
    "iroha_torii": {
        "default": ("node-api",),
        "node-api": (
            "app_api",
            "transparent_api",
            "app_api_wss",
            "connect",
            "push",
            "circuit-params",
            "proofs-full",
            "ipa-commitment",
        ),
        "proofs-halo2": ("zk-halo2", "zk-halo2-ipa"),
        "proofs-stark": ("zk-stark",),
        "proofs-full": ("proofs-halo2", "proofs-stark"),
    },
    "irohad": {
        "default": ("daemon-i3",),
        "daemon-i2": ("daemon-common",),
        "daemon-i3": ("daemon-common",),
        "build-i2": ("daemon-i2",),
        "build-i3": ("daemon-i3",),
    },
    "iroha_cli": {
        "default": ("cli-i3",),
        "cli-i2": ("cli-common",),
        "cli-i3": ("cli-common",),
        "build-i2": ("cli-i2",),
        "build-i3": ("cli-i3",),
    },
}


def _load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as source:
        return tomllib.load(source)


def _dependency_tables(document: dict[str, Any]) -> Iterable[tuple[str, dict[str, Any]]]:
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = document.get(section)
        if isinstance(table, dict):
            yield section, table

    targets = document.get("target", {})
    if not isinstance(targets, dict):
        return
    for target_name, target in targets.items():
        if not isinstance(target, dict):
            continue
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            table = target.get(section)
            if isinstance(table, dict):
                yield f"target.{target_name}.{section}", table


def _member_manifest(root: Path, member: str) -> Path:
    path = root / member
    return path if path.name == "Cargo.toml" else path / "Cargo.toml"


def _check_expected_features(
    document: dict[str, Any], manifest_path: Path
) -> list[str]:
    package = document.get("package", {})
    package_name = package.get("name") if isinstance(package, dict) else None
    expected = EXPECTED_FEATURES.get(package_name)
    if expected is None:
        return []

    errors: list[str] = []
    actual_features = document.get("features", {})
    if not isinstance(actual_features, dict):
        return [f"{manifest_path}: missing [features] table"]

    for feature, expected_members in expected.items():
        actual_members = actual_features.get(feature)
        if not isinstance(actual_members, list):
            errors.append(f"{manifest_path}: missing feature aggregate `{feature}`")
            continue
        if tuple(actual_members) != expected_members:
            errors.append(
                f"{manifest_path}: feature `{feature}` must be "
                f"{list(expected_members)!r}, found {actual_members!r}"
            )
    return errors


def check_repository(root: Path) -> list[str]:
    """Return deterministic feature-hygiene violations for ``root``."""

    root = root.resolve()
    root_manifest_path = root / "Cargo.toml"
    root_manifest = _load_toml(root_manifest_path)
    workspace = root_manifest.get("workspace", {})
    workspace_dependencies = workspace.get("dependencies", {})

    errors: list[str] = []
    for dependency in sorted(FOUNDATIONAL_DEPENDENCIES):
        specification = workspace_dependencies.get(dependency)
        if not isinstance(specification, dict):
            errors.append(
                f"{root_manifest_path}: workspace dependency `{dependency}` "
                "must use a table with `default-features = false`"
            )
            continue
        if specification.get("default-features") is not False:
            errors.append(
                f"{root_manifest_path}: workspace dependency `{dependency}` "
                "must set `default-features = false`"
            )
        if "features" in specification:
            errors.append(
                f"{root_manifest_path}: workspace dependency `{dependency}` "
                "must not inject features"
            )

    default_members = workspace.get("default-members", [])
    if not isinstance(default_members, list):
        return [*errors, f"{root_manifest_path}: `workspace.default-members` must be an array"]

    for member in default_members:
        if not isinstance(member, str):
            errors.append(f"{root_manifest_path}: default member paths must be strings")
            continue
        manifest_path = _member_manifest(root, member)
        if not manifest_path.is_file():
            errors.append(f"{manifest_path}: default-member manifest does not exist")
            continue
        document = _load_toml(manifest_path)
        errors.extend(_check_expected_features(document, manifest_path))
        for section, dependencies in _dependency_tables(document):
            for dependency in sorted(FOUNDATIONAL_DEPENDENCIES & dependencies.keys()):
                specification = dependencies[dependency]
                if not isinstance(specification, dict) or specification.get(
                    "default-features"
                ) is not False:
                    errors.append(
                        f"{manifest_path}: [{section}] `{dependency}` must set "
                        "`default-features = false` and select features locally"
                    )

    return errors


def main() -> int:
    """Run the command-line feature-hygiene check."""

    parser = argparse.ArgumentParser(
        description=(
            "Reject workspace-level feature injection and implicit defaults in "
            "the default Cargo development surface."
        )
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root containing Cargo.toml (default: inferred)",
    )
    args = parser.parse_args()

    errors = check_repository(args.root)
    if errors:
        print("Cargo feature hygiene violations:")
        for error in errors:
            print(f"  - {error}")
        return 1

    print("Cargo feature hygiene check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
