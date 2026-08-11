#!/usr/bin/env python3
"""Validate explicit ownership of foundational Cargo features.

Prerequisites: Python 3.11+, or Python 3.9/3.10 with the repository's pinned
``tomli`` dependency installed. The check is read-only and requires no
environment variables.
"""

from __future__ import annotations

import argparse
from pathlib import Path, PurePosixPath
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
        "consensus": ("std", "rand", "bls"),
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
        "runtime": ("json", "bls", "proofs-halo2"),
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
        "default": ("daemon",),
    },
    "iroha_cli": {
        "default": ("cli",),
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


def _member_manifest(root: Path, member: str | Path) -> Path:
    path = root / member
    return path if path.name == "Cargo.toml" else path / "Cargo.toml"


def _workspace_patterns(workspace: dict[str, Any], field: str) -> list[str]:
    raw_patterns = workspace.get(field, [])
    if not isinstance(raw_patterns, list):
        raise ValueError(f"`workspace.{field}` must be an array")

    patterns: list[str] = []
    for raw_pattern in raw_patterns:
        if not isinstance(raw_pattern, str):
            raise ValueError(f"`workspace.{field}` paths must be strings")
        normalized = raw_pattern.replace("\\", "/")
        path = PurePosixPath(normalized)
        if (
            not normalized
            or path.is_absolute()
            or ".." in path.parts
            or path.as_posix() != normalized
        ):
            raise ValueError(
                f"`workspace.{field}` contains an invalid path: {raw_pattern!r}"
            )
        patterns.append(normalized)
    return patterns


def _expand_member_patterns(
    root: Path,
    patterns: Iterable[str],
    *,
    require_match: bool,
) -> set[Path]:
    manifests: set[Path] = set()
    root = root.resolve()
    for pattern in patterns:
        matched_manifests: set[Path] = set()
        for candidate in root.glob(pattern):
            manifest = _member_manifest(root, candidate)
            if not manifest.is_file():
                continue
            resolved = manifest.resolve()
            try:
                resolved.relative_to(root)
            except ValueError as error:
                raise ValueError(
                    f"workspace member resolves outside the repository: {manifest}"
                ) from error
            matched_manifests.add(resolved)
        if require_match and not matched_manifests:
            raise ValueError(f"workspace member pattern matches no manifests: {pattern}")
        manifests.update(matched_manifests)
    return manifests


def workspace_member_manifests(
    root: Path, workspace: dict[str, Any]
) -> tuple[Path, ...]:
    """Expand, exclude, and deduplicate every Cargo workspace member manifest."""

    members = _workspace_patterns(workspace, "members")
    if not members:
        raise ValueError("`workspace.members` must contain at least one path")
    excludes = _workspace_patterns(workspace, "exclude")
    included = _expand_member_patterns(root, members, require_match=True)
    excluded = _expand_member_patterns(root, excludes, require_match=False)
    return tuple(sorted(included - excluded))


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
    if not isinstance(workspace, dict):
        return [f"{root_manifest_path}: missing [workspace] table"]
    workspace_dependencies = workspace.get("dependencies", {})
    if not isinstance(workspace_dependencies, dict):
        return [f"{root_manifest_path}: `workspace.dependencies` must be a table"]

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

    try:
        member_manifests = workspace_member_manifests(root, workspace)
    except ValueError as error:
        return [*errors, f"{root_manifest_path}: {error}"]

    for manifest_path in member_manifests:
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
            "every Cargo workspace member."
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
