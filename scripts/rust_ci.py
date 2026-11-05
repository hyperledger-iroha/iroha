#!/usr/bin/env python3
"""Classify and run affected Rust workspace validation lanes.

The classifier requires Python 3.9+ (and the pinned ``tomli`` on Python before
3.11), a locked Cargo workspace, and Git when paths are not supplied
explicitly. It never mutates tracked sources. Unknown, ambiguous, or deleted
Rust ownership fails closed to every lane.

Ordinary foundation-only changes retain affected library and local CLI checks;
network tests and daemon owners require a mixed source change or ``ci/full``.
Binary staging requires Cargo and uses separate target directories for the
shipping daemon and each test-feature daemon graph. Qualified corridor runners own
their source-bound bundles and never consume this downloaded PR artifact set.
"""

from __future__ import annotations

import argparse
import fnmatch
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
from collections import defaultdict, deque
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier use the pinned backport.
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[1]
_DOCS_SPEC = importlib.util.spec_from_file_location(
    "rust_ci_kotodama_documents", ROOT / "scripts" / "check_kotodama_docs.py"
)
assert _DOCS_SPEC is not None and _DOCS_SPEC.loader is not None
KOTODAMA_DOCS = importlib.util.module_from_spec(_DOCS_SPEC)
sys.modules[_DOCS_SPEC.name] = KOTODAMA_DOCS
_DOCS_SPEC.loader.exec_module(KOTODAMA_DOCS)
_SHARDS_SPEC = importlib.util.spec_from_file_location(
    "rust_ci_native_shards", ROOT / "scripts" / "rust_test_shards.py"
)
assert _SHARDS_SPEC is not None and _SHARDS_SPEC.loader is not None
NATIVE_SHARDS = importlib.util.module_from_spec(_SHARDS_SPEC)
sys.modules[_SHARDS_SPEC.name] = NATIVE_SHARDS
_SHARDS_SPEC.loader.exec_module(NATIVE_SHARDS)
DEFAULT_MANIFEST = ROOT / "ci" / "rust_lanes.toml"
CHECK_NAMES = ("clippy", "build", "test", "doctest", "doc")
# Native harnesses are partitioned from their actual compiled inventories. The
# owner row retains every other check, including Cargo-owned documentation tests.
NATIVE_TEST_SHARDS = {"iroha_core": ("node-core", 4), "iroha_core_privacy": ("execution-privacy", 8)}
NATIVE_TEST_SHARDS["integration_tests:sumeragi_npos_committee_transition"] = (
    "integration-sumeragi_npos_committee_transition", 8,
)
# Preserve logical ownership while bounding independent hosted execution jobs.
# The combined lane exceeded four hours before its proof tests had finished.
EXECUTION_JOB_PACKAGES = {
    "execution-model": frozenset({
        "iroha_data_model", "iroha_data_model_derive",
        "iroha_executor_data_model", "iroha_executor_data_model_derive",
    }),
    "execution-vm": frozenset({
        "ivm", "ivm_abi", "ivm_artifact_admission", "kotodama_lang",
        "kotodama_surface", "kotodama_toolchain",
    }),
    "execution-kagemusha": frozenset({"iroha_kagemusha_proof"}),
}


@dataclass(frozen=True)
class BinaryArtifact:
    """One executable with an explicit Cargo owner and feature isolation boundary."""

    package: str
    binary: str
    features: tuple[str, ...] = ()
    target_group: str = "shipping"


BINARY_ARTIFACTS = {
    "iroha3d": BinaryArtifact("irohad", "iroha3d"),
    "iroha3d_taira": BinaryArtifact("irohad", "iroha3d_taira"),
    "iroha": BinaryArtifact("iroha_cli", "iroha"),
    "kagami": BinaryArtifact("iroha_kagami", "kagami"),
    "koto": BinaryArtifact("kotodama_toolchain", "koto"),
    "sorafs_cli": BinaryArtifact(
        "sorafs_orchestrator", "sorafs_cli",
        ("cli-orchestrator", "moderation-grpc"), "sorafs-cli"
    ),
    "iroha_test_runtime_provider_broker": BinaryArtifact(
        "irohad", "iroha_test_runtime_provider_broker",
        ("test-network-disposable-broker",), "disposable-broker"
    ),
    "iroha3d_private_settlement_routes": BinaryArtifact(
        "irohad", "iroha3d", ("test-network-private-settlement-route-control",), "private-settlement-route-control"
    ),
}
PACKAGE_NAME_CHARACTERS = frozenset(
    "-_0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
)


class ClassificationError(RuntimeError):
    """Report invalid metadata, manifest, paths, or command-line inputs."""


@dataclass(frozen=True)
class WorkspacePackage:
    """A workspace package and its repository-relative owning directory."""

    package_id: str
    name: str
    directory: PurePosixPath


@dataclass(frozen=True)
class BinaryConsumer:
    """A routed check using downloaded binaries or its own qualified build protocol."""

    packages: tuple[str, ...]
    paths: tuple[str, ...]
    binaries: tuple[str, ...]
    kotodama_document_inventory: str | None = None
    qualified_runner: str | None = None


@dataclass(frozen=True)
class LaneManifest:
    """Validated lane ownership and non-package path-routing policy."""

    lanes: dict[str, tuple[str, ...]]
    generated_patterns: tuple[str, ...]
    all_patterns: tuple[str, ...]
    ignore_patterns: tuple[str, ...]
    lane_patterns: dict[str, tuple[str, ...]]
    package_binaries: dict[str, tuple[str, ...]] = field(default_factory=dict)
    consumers: dict[str, BinaryConsumer] = field(default_factory=dict)
    daemon_packages: tuple[str, ...] = ()

    @property
    def package_lane(self) -> dict[str, str]:
        """Return the unique primary lane for every configured package."""

        return {
            package: lane
            for lane, packages in self.lanes.items()
            for package in packages
        }


@dataclass(frozen=True)
class Classification:
    """Affected packages, lanes, and the evidence used to select them."""

    changed_paths: tuple[str, ...]
    changed_packages: tuple[str, ...]
    impacted_packages: tuple[str, ...]
    lane_packages: dict[str, tuple[str, ...]]
    full: bool
    reasons: tuple[str, ...]
    package_binaries: dict[str, tuple[str, ...]] = field(default_factory=dict)
    consumers: dict[str, bool] = field(default_factory=dict)
    binaries: tuple[str, ...] = ()
    foundation_only: bool = False
    deferred_packages: tuple[str, ...] = ()
    deferred_consumers: tuple[str, ...] = ()

    @property
    def has_rust(self) -> bool:
        """Return whether at least one Rust lane must run."""

        return bool(self.lane_packages)

    def as_dict(self) -> dict[str, Any]:
        """Return a deterministic JSON-compatible representation."""

        include = [
            {
                "lane": lane,
                "packages": ",".join(packages),
                "package_count": len(packages),
            }
            for lane, packages in self.lane_packages.items()
        ]
        matrices: dict[str, list[dict[str, Any]]] = {"binary_free": [], "binary": []}
        for lane, packages in self.lane_packages.items():
            for kind, requires_binaries in (("binary_free", False), ("binary", True)):
                selected = tuple(
                    package for package in packages
                    if bool(self.package_binaries.get(package)) == requires_binaries
                )
                for package in selected:
                    if package in NATIVE_TEST_SHARDS:
                        job, count = NATIVE_TEST_SHARDS[package]
                        matrices[kind].append({
                            "lane": job, "packages": package, "package_count": 1,
                            "checks": "clippy,build,doctest,doc",
                        })
                        matrices[kind].extend({
                            "lane": f"{job}-tests-{index}", "packages": package,
                            "package_count": 1, "checks": "test", "native_shard": index,
                        } for index in range(1, count + 1))
                selected = tuple(package for package in selected if package not in NATIVE_TEST_SHARDS)
                if selected:
                    row = {
                        "lane": lane,
                        "packages": ",".join(selected),
                        "package_count": len(selected),
                    }
                    if lane == "execution":
                        groups: dict[str, list[str]] = {}
                        for package in selected:
                            job = next((
                                name for name, members in EXECUTION_JOB_PACKAGES.items()
                                if package in members
                            ), lane)
                            groups.setdefault(job, []).append(package)
                        matrices[kind].extend({
                            "lane": job, "packages": ",".join(members),
                            "package_count": len(members),
                        } for job, members in groups.items())
                    elif kind == "binary" and "integration_tests" in selected:
                        others = tuple(package for package in selected if package != "integration_tests")
                        if others:
                            matrices[kind].append({**row, "packages": ",".join(others), "package_count": len(others)})
                        matrices[kind].append({
                            "lane": lane + "-library", "packages": "integration_tests",
                            "package_count": 1, "test_target": "library",
                        })
                        for target in integration_test_targets():
                            target_row = {
                                "lane": lane + "-" + target, "packages": "integration_tests",
                                "package_count": 1, "test_target": target, "checks": "test",
                            }
                            sharding = NATIVE_TEST_SHARDS.get("integration_tests:" + target)
                            if sharding:
                                job, count = sharding
                                matrices[kind].extend({
                                    **target_row, "lane": f"{job}-{index}", "native_shard": index,
                                } for index in range(1, count + 1))
                            else:
                                matrices[kind].append(target_row)
                    else:
                        matrices[kind].append(row)
        return {
            "version": 2,
            "has_rust": self.has_rust,
            "full": self.full,
            "foundation_only": self.foundation_only,
            "deferred_packages": list(self.deferred_packages),
            "deferred_consumers": list(self.deferred_consumers),
            "changed_paths": list(self.changed_paths),
            "changed_packages": list(self.changed_packages),
            "impacted_packages": list(self.impacted_packages),
            "lanes": [
                {"name": item["lane"], "packages": item["packages"].split(",")}
                for item in include
            ],
            "matrix": {"include": include},
            "binary_free_matrix": {"include": matrices["binary_free"]},
            "binary_matrix": {"include": matrices["binary"]},
            "has_binary_free_rust": bool(matrices["binary_free"]),
            "has_binary_rust": bool(matrices["binary"]),
            "has_binaries": bool(self.binaries),
            "binaries": list(self.binaries),
            "consumers": self.consumers,
            "reasons": list(self.reasons),
        }


def _run(
    command: Sequence[str],
    *,
    cwd: Path = ROOT,
    capture_output: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run one command and convert failures into concise classifier errors."""

    try:
        return subprocess.run(
            command,
            cwd=cwd,
            check=True,
            capture_output=capture_output,
            text=True,
        )
    except FileNotFoundError as error:
        raise ClassificationError(
            f"required executable is unavailable: {command[0]}"
        ) from error
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or error.stdout or "").strip()
        suffix = f": {detail}" if detail else ""
        raise ClassificationError(
            f"command failed ({' '.join(command)}){suffix}"
        ) from error


def load_cargo_metadata(
    *, root: Path = ROOT, metadata_path: Path | None = None
) -> dict[str, Any]:
    """Load full locked Cargo metadata, including the dependency resolve graph."""

    if metadata_path is not None:
        try:
            return json.loads(metadata_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise ClassificationError(
                f"cannot load Cargo metadata from {metadata_path}: {error}"
            ) from error
    result = _run(
        ("cargo", "metadata", "--locked", "--format-version", "1"),
        cwd=root,
    )
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ClassificationError(f"Cargo metadata is not valid JSON: {error}") from error


def _repository_relative(path: Path, root: Path) -> PurePosixPath:
    """Return a normalized repository-relative path or fail safely."""

    try:
        relative = path.resolve().relative_to(root.resolve())
    except ValueError as error:
        raise ClassificationError(f"path is outside the repository: {path}") from error
    return PurePosixPath(relative.as_posix())


def workspace_packages(
    metadata: dict[str, Any], *, root: Path = ROOT
) -> dict[str, WorkspacePackage]:
    """Extract unique workspace package ownership from Cargo metadata."""

    member_ids = set(metadata.get("workspace_members", ()))
    packages: dict[str, WorkspacePackage] = {}
    for raw in metadata.get("packages", ()):
        if raw.get("id") not in member_ids:
            continue
        name = raw.get("name")
        manifest_path = raw.get("manifest_path")
        if not isinstance(name, str) or not isinstance(manifest_path, str):
            raise ClassificationError(
                "Cargo metadata contains an invalid workspace package"
            )
        if name in packages:
            raise ClassificationError(f"workspace package name is not unique: {name}")
        directory = _repository_relative(Path(manifest_path).parent, root)
        packages[name] = WorkspacePackage(raw["id"], name, directory)
    missing_ids = member_ids - {package.package_id for package in packages.values()}
    if missing_ids:
        raise ClassificationError(
            f"Cargo metadata omits workspace package records: {sorted(missing_ids)}"
        )
    return packages


def load_lane_manifest(path: Path = DEFAULT_MANIFEST) -> LaneManifest:
    """Load the checked-in TOML lane manifest without third-party dependencies."""

    try:
        raw = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ClassificationError(f"cannot load lane manifest {path}: {error}") from error
    if raw.get("version") != 1:
        raise ClassificationError("lane manifest version must be 1")
    daemon_packages = raw.get("daemon_packages", [])
    if (
        not isinstance(daemon_packages, list)
        or not all(isinstance(package, str) and package for package in daemon_packages)
        or len(set(daemon_packages)) != len(daemon_packages)
    ):
        raise ClassificationError("daemon_packages must list unique package names")
    raw_lanes = raw.get("lanes")
    if not isinstance(raw_lanes, dict) or not raw_lanes:
        raise ClassificationError("lane manifest must define at least one lane")
    lanes: dict[str, tuple[str, ...]] = {}
    for lane, settings in raw_lanes.items():
        if not isinstance(settings, dict):
            raise ClassificationError(f"lane {lane!r} must be a table")
        packages = settings.get("packages")
        if not isinstance(packages, list) or not all(
            isinstance(package, str) and package for package in packages
        ):
            raise ClassificationError(f"lane {lane!r} must list package names")
        lanes[lane] = tuple(packages)

    raw_paths = raw.get("paths", {})
    if not isinstance(raw_paths, dict):
        raise ClassificationError("paths must be a table")
    generated_patterns = _patterns(raw_paths.get("generated", ()), "paths.generated")
    all_patterns = _patterns(raw_paths.get("all", ()), "paths.all")
    ignore_patterns = _patterns(raw_paths.get("ignore", ()), "paths.ignore")
    raw_lane_patterns = raw_paths.get("lanes", {})
    if not isinstance(raw_lane_patterns, dict):
        raise ClassificationError("paths.lanes must be a table")
    unknown_path_lanes = set(raw_lane_patterns) - set(lanes)
    if unknown_path_lanes:
        raise ClassificationError(
            f"path mappings reference unknown lanes: {sorted(unknown_path_lanes)}"
        )
    lane_patterns = {
        lane: _patterns(raw_lane_patterns.get(lane, ()), f"paths.lanes.{lane}")
        for lane in lanes
    }
    raw_binary_packages = raw.get("package_binaries", {})
    if not isinstance(raw_binary_packages, dict):
        raise ClassificationError("package_binaries must be a table")
    package_binaries = {
        package: _binary_names(names, f"package_binaries.{package}")
        for package, names in raw_binary_packages.items()
    }
    raw_consumers = raw.get("consumers", {})
    if not isinstance(raw_consumers, dict):
        raise ClassificationError("consumers must be a table")
    consumers = {}
    for name, consumer in raw_consumers.items():
        if (
            not name
            or not set(name) <= PACKAGE_NAME_CHARACTERS
            or not isinstance(consumer, dict)
        ):
            raise ClassificationError(f"invalid binary consumer: {name!r}")
        unknown = set(consumer) - {
            "packages", "paths", "binaries", "kotodama_document_inventory", "qualified_runner"
        }
        if unknown:
            raise ClassificationError(f"consumers.{name} has unknown fields: {sorted(unknown)}")
        consumer_packages = consumer.get("packages", [])
        if not isinstance(consumer_packages, list) or not all(
            isinstance(package, str) and package for package in consumer_packages
        ):
            raise ClassificationError(f"consumers.{name}.packages must list package names")
        inventory = consumer.get("kotodama_document_inventory")
        if inventory is not None and (
            not isinstance(inventory, str)
            or _normalize_changed_path(inventory) != inventory
            or not inventory.endswith(".json")
        ):
            raise ClassificationError(f"consumers.{name}.kotodama_document_inventory must name a repository JSON path")
        runner = consumer.get("qualified_runner")
        if runner is not None and (
            not isinstance(runner, str)
            or _normalize_changed_path(runner) != runner
            or not runner.startswith("ci/")
            or not runner.endswith(".sh")
        ):
            raise ClassificationError(f"consumers.{name}.qualified_runner must name a repository ci shell runner")
        binaries = _binary_names(
            consumer.get("binaries"), f"consumers.{name}.binaries", allow_empty=runner is not None
        )
        if runner is not None and binaries:
            raise ClassificationError(f"consumers.{name} cannot mix qualified and downloaded binary owners")
        consumers[name] = BinaryConsumer(
            packages=tuple(consumer_packages),
            paths=_patterns(consumer.get("paths", []), f"consumers.{name}.paths"),
            binaries=binaries,
            kotodama_document_inventory=inventory,
            qualified_runner=runner,
        )
    return LaneManifest(
        lanes=lanes,
        generated_patterns=generated_patterns,
        all_patterns=all_patterns,
        ignore_patterns=ignore_patterns,
        lane_patterns=lane_patterns,
        package_binaries=package_binaries,
        consumers=consumers,
        daemon_packages=tuple(daemon_packages),
    )


def _binary_names(raw: Any, field: str, *, allow_empty: bool = False) -> tuple[str, ...]:
    """Accept only explicit, unique artifact names and intentionally owned empty unions."""

    if (
        not isinstance(raw, (list, tuple))
        or (not raw and not allow_empty)
        or not all(isinstance(name, str) and name in BINARY_ARTIFACTS for name in raw)
        or len(set(raw)) != len(raw)
    ):
        raise ClassificationError(f"{field} must list unique binary artifacts")
    return tuple(sorted(raw))


def _patterns(raw: Any, field: str) -> tuple[str, ...]:
    """Validate and normalize a list of repository-relative glob patterns."""

    if not isinstance(raw, (list, tuple)) or not all(
        isinstance(pattern, str) and pattern and not pattern.startswith("/")
        for pattern in raw
    ):
        raise ClassificationError(f"{field} must contain relative glob strings")
    return tuple(raw)


def validate_manifest(
    manifest: LaneManifest, packages: dict[str, WorkspacePackage]
) -> None:
    """Require exact, unique coverage of every current workspace package."""

    configured = [
        package
        for lane_packages in manifest.lanes.values()
        for package in lane_packages
    ]
    duplicates = sorted(
        package for package in set(configured) if configured.count(package) > 1
    )
    workspace_names = set(packages)
    missing = sorted(workspace_names - set(configured))
    stale = sorted(set(configured) - workspace_names)
    errors = []
    if duplicates:
        errors.append(f"packages assigned to multiple lanes: {duplicates}")
    if missing:
        errors.append(f"workspace packages missing from lanes: {missing}")
    if stale:
        errors.append(f"lane packages absent from workspace: {stale}")
    binary_owners = set(manifest.package_binaries).union(manifest.daemon_packages)
    for name, consumer in manifest.consumers.items():
        binary_owners.update(consumer.packages)
        _binary_names(
            consumer.binaries, f"consumers.{name}.binaries",
            allow_empty=consumer.qualified_runner is not None,
        )
    for name, binaries in manifest.package_binaries.items():
        _binary_names(binaries, f"package_binaries.{name}")
    unknown_binary_owners = sorted(binary_owners - workspace_names)
    if unknown_binary_owners:
        errors.append(f"binary requirements reference unknown packages: {unknown_binary_owners}")
    pattern_owners: dict[str, list[str]] = defaultdict(list)
    for pattern in manifest.generated_patterns:
        pattern_owners[pattern].append("generated")
    for pattern in manifest.all_patterns:
        pattern_owners[pattern].append("all")
    for pattern in manifest.ignore_patterns:
        pattern_owners[pattern].append("ignore")
    for lane, patterns in manifest.lane_patterns.items():
        for pattern in patterns:
            pattern_owners[pattern].append(lane)
    duplicate_patterns = {
        pattern: owners
        for pattern, owners in pattern_owners.items()
        if len(owners) > 1
    }
    if duplicate_patterns:
        errors.append(f"path patterns have multiple owners: {duplicate_patterns}")
    if errors:
        raise ClassificationError("; ".join(errors))


def reverse_dependencies(
    metadata: dict[str, Any], packages: dict[str, WorkspacePackage]
) -> dict[str, set[str]]:
    """Project Cargo's reverse graph onto workspace owners, keeping external paths."""

    by_id = {package.package_id: package.name for package in packages.values()}
    reverse: dict[str, set[str]] = {name: set() for name in packages}
    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict) or not isinstance(resolve.get("nodes"), list):
        raise ClassificationError(
            "full Cargo metadata with a dependency resolve graph is required"
        )
    resolved_reverse: dict[str, set[str]] = defaultdict(set)
    for node in resolve["nodes"]:
        for dependency in node.get("deps", ()):
            resolved_reverse[dependency["pkg"]].add(node["id"])
    for package_id, name in by_id.items():
        visited = {package_id}
        queue = deque(sorted(resolved_reverse.get(package_id, ())))
        while queue:
            dependent_id = queue.popleft()
            if dependent_id in visited:
                continue
            visited.add(dependent_id)
            dependent_name = by_id.get(dependent_id)
            if dependent_name is not None:
                reverse[name].add(dependent_name)
                # The classifier's workspace closure continues from this owner.
                continue
            queue.extend(sorted(resolved_reverse.get(dependent_id, ())))
    return reverse


def _normalize_changed_path(raw_path: str) -> str:
    """Normalize one Git path and reject absolute or parent traversal paths."""

    normalized = raw_path.replace("\\", "/").removeprefix("./")
    path = PurePosixPath(normalized)
    if not normalized or path.is_absolute() or ".." in path.parts:
        raise ClassificationError(f"invalid changed path: {raw_path!r}")
    return path.as_posix()


def _matches(path: str, patterns: Iterable[str]) -> bool:
    """Return whether a repository path matches any configured glob."""

    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def _owning_package(
    path: str, packages: dict[str, WorkspacePackage]
) -> str | None:
    """Find the deepest workspace package directory that owns a changed path."""

    parts = PurePosixPath(path).parts
    candidates = []
    for package in packages.values():
        directory_parts = package.directory.parts
        if parts[: len(directory_parts)] == directory_parts:
            candidates.append((len(directory_parts), package.name))
    if not candidates:
        return None
    return max(candidates)[1]


def _closure(seeds: set[str], reverse: dict[str, set[str]]) -> set[str]:
    """Return seeds plus every transitive workspace reverse dependency."""

    impacted = set(seeds)
    queue = deque(sorted(seeds))
    while queue:
        package = queue.popleft()
        for dependent in sorted(reverse.get(package, ())):
            if dependent not in impacted:
                impacted.add(dependent)
                queue.append(dependent)
    return impacted


def classify_paths(
    changed_paths: Iterable[str],
    *,
    metadata: dict[str, Any],
    manifest: LaneManifest,
    root: Path = ROOT,
    base_revision: str = "HEAD",
) -> Classification:
    """Classify changed paths and expand package changes through reverse dependencies."""

    packages = workspace_packages(metadata, root=root)
    validate_manifest(manifest, packages)
    reverse = reverse_dependencies(metadata, packages)
    normalized_paths = tuple(
        sorted({_normalize_changed_path(path) for path in changed_paths})
    )
    seed_packages: set[str] = set()
    full = False
    reasons: list[str] = []
    # Prose inside packages or consumer directories is still documentation.
    # The executable document inventory below independently selects compiler
    # inputs, including deleted fences, without selecting unrelated binaries.
    source_paths = tuple(
        path for path in normalized_paths
        if not _matches(path, manifest.generated_patterns)
        and PurePosixPath(path).suffix.lower() not in (".md", ".mdx")
    )
    for path in source_paths:
        owner = _owning_package(path, packages)
        if owner is not None:
            seed_packages.add(owner)
            continue

        all_match = _matches(path, manifest.all_patterns)
        ignore_match = _matches(path, manifest.ignore_patterns)
        mapped_lanes = {
            lane
            for lane, patterns in manifest.lane_patterns.items()
            if _matches(path, patterns)
        }
        match_kinds = int(all_match) + int(ignore_match) + int(bool(mapped_lanes))
        if len(mapped_lanes) > 1:
            full = True
            reasons.append(
                f"ambiguous lane mapping ({', '.join(sorted(mapped_lanes))}): {path}"
            )
        elif match_kinds > 1:
            full = True
            reasons.append(f"ambiguous path mapping: {path}")
        elif all_match:
            full = True
            reasons.append(f"full-workspace input changed: {path}")
        elif mapped_lanes:
            for lane in mapped_lanes:
                seed_packages.update(manifest.lanes[lane])
        elif ignore_match:
            continue
        else:
            full = True
            reasons.append(f"unmapped path changed: {path}")

    if full:
        seed_packages = set(packages)
    impacted = _closure(seed_packages, reverse)
    package_lane = manifest.package_lane
    # Ordinary foundation-only changes retain every binary-free reverse
    # dependant, while node-launching checks belong to the explicit full tier.
    # A mixed source change or a direct consumer input still selects its checks.
    foundation_only = not full and bool(seed_packages) and all(
        package_lane[package] == "foundation" for package in seed_packages
    )
    deferred_packages = impacted.intersection(
        set(manifest.package_binaries).union(manifest.daemon_packages)
    ) if foundation_only else set()
    selected_packages = impacted - deferred_packages
    if foundation_only:
        reasons.append(
            "foundation-only source tier retains binary-free reverse dependants; "
            "package-triggered network checks require ci/full or mixed source changes"
        )
    lane_packages = {
        lane: tuple(
            sorted(
                package for package in selected_packages if package_lane[package] == lane
            )
        )
        for lane in manifest.lanes
        if any(package_lane[package] == lane for package in selected_packages)
    }
    package_binaries = {
        package: manifest.package_binaries[package]
        for package in sorted(selected_packages)
        if package in manifest.package_binaries
    }
    consumers = {
        name: full or (not foundation_only and bool(impacted.intersection(consumer.packages))) or any(
            _matches(path, consumer.paths) for path in source_paths
        ) or consumer.qualified_runner in source_paths
        for name, consumer in manifest.consumers.items()
    }
    for name, consumer in manifest.consumers.items():
        inventory = consumer.kotodama_document_inventory
        if consumers[name] or inventory is None:
            continue
        if inventory in normalized_paths:
            consumers[name] = True
            reasons.append(f"executable documentation inventory changed: {inventory}")
            continue
        try:
            documents = KOTODAMA_DOCS.changed_executable_documents(
                [Path(path) for path in normalized_paths
                 if not _matches(path, manifest.generated_patterns)],
                root, root / inventory, base_revision=base_revision,
            )
        except KOTODAMA_DOCS.DocumentationCheckError as error:
            consumers[name] = True
            reasons.append(f"executable documentation requires conservative validation: {error}")
        else:
            consumers[name] = bool(documents)
            reasons.extend(f"executable documentation source changed: {path}" for path in documents)
    binaries = {binary for names in package_binaries.values() for binary in names}
    for name, selected in consumers.items():
        if selected:
            binaries.update(manifest.consumers[name].binaries)
    return Classification(
        changed_paths=normalized_paths,
        changed_packages=tuple(sorted(seed_packages)),
        impacted_packages=tuple(sorted(impacted)),
        lane_packages=lane_packages,
        full=full,
        reasons=tuple(dict.fromkeys(reasons)),
        package_binaries=package_binaries,
        consumers=consumers,
        binaries=tuple(sorted(binaries)),
        foundation_only=foundation_only,
        deferred_packages=tuple(sorted(deferred_packages)),
        deferred_consumers=tuple(sorted(
            name for name, consumer in manifest.consumers.items()
            if foundation_only and not consumers[name] and impacted.intersection(consumer.packages)
        )),
    )


def git_changed_paths(base: str | None, *, root: Path = ROOT) -> tuple[str, ...]:
    """Return committed, staged, unstaged, and untracked paths for local routing."""

    paths: set[str] = set()
    if base:
        merge_base = _run(
            ("git", "merge-base", base, "HEAD"), cwd=root
        ).stdout.strip()
        if not merge_base:
            raise ClassificationError(f"cannot find merge base for {base!r}")
        paths.update(
            _git_nul_paths(
                (
                    "git",
                    "diff",
                    "--no-renames",
                    "--name-only",
                    "--diff-filter=ACDMRTUXB",
                    "-z",
                    f"{merge_base}...HEAD",
                ),
                root=root,
            )
        )
    paths.update(
        _git_nul_paths(
            (
                "git",
                "diff",
                "--no-renames",
                "--name-only",
                "--diff-filter=ACDMRTUXB",
                "-z",
                "HEAD",
            ),
            root=root,
        )
    )
    paths.update(
        _git_nul_paths(
            (
                "git",
                "diff",
                "--cached",
                "--no-renames",
                "--name-only",
                "--diff-filter=ACDMRTUXB",
                "-z",
                "HEAD",
            ),
            root=root,
        )
    )
    paths.update(
        _git_nul_paths(
            ("git", "ls-files", "--others", "--exclude-standard", "-z"),
            root=root,
        )
    )
    return tuple(sorted(path for path in paths if path))


def _git_nul_paths(command: Sequence[str], *, root: Path) -> tuple[str, ...]:
    """Read an unambiguous NUL-delimited path list from Git."""

    return tuple(path for path in _run(command, cwd=root).stdout.split("\0") if path)


def configured_base_ref(root: Path = ROOT) -> str | None:
    """Return the integration branch recorded in ci/default_base_ref, when there is one."""

    try:
        lines = (root / "ci" / "default_base_ref").read_text(encoding="utf-8").splitlines()
    except OSError:
        return None
    for line in lines:
        value = line.split("#", 1)[0].strip()
        if value:
            return value
    return None


def default_base(root: Path = ROOT) -> str | None:
    """Choose a discoverable local comparison base without network access."""

    configured = configured_base_ref(root)
    references = (*((configured,) if configured else ()), "origin/main", "@{upstream}", "HEAD^")
    for reference in references:
        result = subprocess.run(
            ("git", "rev-parse", "--verify", "--quiet", reference),
            cwd=root,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            return reference
    return None


# These owning features select spec §13.4 mutations and are deliberately rejected
# by ordinary non-test Model/Core/daemon/SDK/Deploy/Torii compilation (Sumeragi isolates its cfg
# through build.rs). Dedicated nightly mutation jobs own their valid libtests.
MUTATION_FEATURE_OWNERS = frozenset(
    {"iroha_data_model", "iroha_core", "irohad_lib", "iroha_sumeragi", "iroha", "iroha_deploy", "iroha_torii", "norito", "iroha_p2p"}
)


def workspace_check_features(root: Path = ROOT) -> dict[str, tuple[str, ...]]:
    """Read every supported lint/doc feature, including implicit optional edges.

    Only owning ``mutation-testing`` features are excluded. Fixture constructors,
    simulator and isolated test-network features remain part of these diagnostic
    checks; this matrix does not qualify a shipping feature graph. A new feature
    forwarding a mutation switch is refused rather than silently losing coverage.
    """

    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    excluded = {(root / path).resolve() for path in workspace.get("exclude", ())}
    workspace_dependencies = workspace.get("dependencies", {})
    inventory: dict[str, tuple[str, ...]] = {}
    for pattern in workspace["members"]:
        for directory in sorted(root.glob(pattern)):
            manifest = directory / "Cargo.toml"
            if directory.resolve() in excluded or not manifest.is_file():
                continue
            payload = tomllib.loads(manifest.read_text())
            package = payload["package"]["name"]
            if package in inventory:
                raise ClassificationError(f"duplicate workspace feature owner: {package}")
            features = payload.get("features", {})
            if not isinstance(features, dict) or any(
                not isinstance(name, str) or not isinstance(values, list)
                or not all(isinstance(value, str) for value in values)
                for name, values in features.items()
            ):
                raise ClassificationError(f"invalid feature table: {manifest}")
            if "mutation-testing" in features and package not in MUTATION_FEATURE_OWNERS:
                raise ClassificationError(f"unreviewed mutation feature owner: {package}")
            for name, values in features.items():
                if name == "mutation-testing":
                    continue
                if any(value == "mutation-testing" or value.endswith("/mutation-testing")
                       for value in values):
                    raise ClassificationError(
                        f"supported feature forwards a test-only mutation: {package}/{name}"
                    )
            dependency_tables = [payload]
            dependency_tables.extend(payload.get("target", {}).values())
            optional = set()
            for table in dependency_tables:
                for kind in ("dependencies", "build-dependencies", "dev-dependencies"):
                    for alias, spec in table.get(kind, {}).items():
                        if not isinstance(spec, dict):
                            continue
                        inherited = {}
                        if spec.get("workspace", False):
                            inherited = workspace_dependencies.get(alias)
                            if isinstance(inherited, str):
                                inherited = {}
                            elif not isinstance(inherited, dict):
                                raise ClassificationError(f"missing workspace dependency: {alias}")
                            selected_features = (*inherited.get("features", ()), *spec.get("features", ()))
                        else:
                            selected_features = spec.get("features", ())
                        dependency_owner = spec.get("package", inherited.get("package", alias))
                        if (kind == "dev-dependencies" and dependency_owner == package
                                and package in MUTATION_FEATURE_OWNERS):
                            raise ClassificationError(
                                f"self dev-dependency defeats owning mutation libtests: {package}/{alias}"
                            )
                        if "mutation-testing" in selected_features:
                            raise ClassificationError(
                                f"dependency forwards a test-only mutation: {package}/{alias}"
                            )
                        if spec.get("optional", False):
                            optional.add(alias)
            # `dep:alias` anywhere disables Cargo's implicit alias feature. Explicit
            # [features] keys still exist, even when their name equals that alias.
            hidden_optional = {value[4:] for values in features.values() for value in values
                               if value.startswith("dep:")}
            supported = set(features) | (optional - hidden_optional)
            supported.discard("mutation-testing")
            inventory[package] = tuple(sorted(supported))
    if not inventory:
        raise ClassificationError("workspace feature inventory is empty")
    return inventory


def _supported_feature_args(
    packages: Sequence[str], inventory: Mapping[str, Sequence[str]]
) -> list[str]:
    """Select the complete supported matrix without a non-test mutation switch."""

    missing = sorted(set(packages) - set(inventory))
    if missing:
        raise ClassificationError(f"missing workspace feature owners: {missing}")
    features = []
    for package in packages:
        names = inventory[package]
        if len(names) != len(set(names)) or any(
            not name or not set(name) <= PACKAGE_NAME_CHARACTERS or name == "mutation-testing"
            for name in names
        ):
            raise ClassificationError(f"invalid supported feature matrix: {package}")
        features.extend(f"{package}/{name}" for name in sorted(names))
    return ["--features", ",".join(features)] if features else []


def integration_test_targets(root: Path = ROOT) -> tuple[str, ...]:
    """Enumerate every explicit integration harness selected by Cargo's default features."""

    manifest = tomllib.loads((root / "integration_tests" / "Cargo.toml").read_text())
    package = manifest["package"]
    if package.get("autotests", True) or package.get("autobins", True):
        raise ClassificationError("integration sharding requires explicit test and binary targets")
    features = manifest.get("features", {})
    enabled = {"default"}
    while True:
        discovered = {
            item for name in enabled for item in features.get(name, ()) if item in features
        }
        if discovered <= enabled:
            break
        enabled.update(discovered)
    # Fail closed if Cargo gains another default target that these scoped test
    # commands would omit. Clippy still checks all declared targets separately.
    directory = root / "integration_tests"
    if (manifest.get("example") or manifest.get("bench")
            or (directory / "examples").is_dir() or (directory / "benches").is_dir()):
        raise ClassificationError("integration sharding requires explicit coverage for examples and benches")
    for target in manifest.get("bin", ()):
        required = target.get("required-features", ())
        if not all(isinstance(feature, str) and feature in features for feature in required):
            raise ClassificationError("unknown integration binary feature")
        if set(required) <= enabled:
            raise ClassificationError("integration sharding requires explicit coverage for enabled binaries")
    names = []
    declared = set()
    for target in manifest.get("test", ()):
        name = target.get("name", "")
        if not name or not set(name) <= PACKAGE_NAME_CHARACTERS or name == "library":
            raise ClassificationError("invalid explicit integration test target")
        if name in declared:
            raise ClassificationError("integration targets must be unique")
        declared.add(name)
        required = target.get("required-features", ())
        if not all(isinstance(feature, str) and feature in features for feature in required):
            raise ClassificationError(f"unknown integration target feature: {name}")
        if set(required) <= enabled:
            names.append(name)
    if not names or len(names) != len(set(names)):
        raise ClassificationError("integration targets must be nonempty and unique")
    return tuple(sorted(names))


def commands_for_checks(
    packages: Sequence[str], checks: Sequence[str],
    *, features_by_package: Mapping[str, Sequence[str]] | None = None,
    workspace: bool = False,
    test_target: str | None = None,
) -> list[list[str]]:
    """Build locked, package-scoped Cargo validation commands."""

    if not packages:
        return []
    if test_target and (
        workspace or tuple(packages) != ("integration_tests",)
        or test_target not in ("library", *integration_test_targets())
    ):
        raise ClassificationError("a scoped integration check requires one declared integration target")
    invalid_packages = [
        package
        for package in packages
        if not package or not set(package) <= PACKAGE_NAME_CHARACTERS
    ]
    if invalid_packages:
        raise ClassificationError(f"invalid Cargo package names: {invalid_packages}")
    package_args = (["--workspace"] if workspace else [
        argument for package in packages for argument in ("-p", package)
    ])
    feature_args = []
    if any(check in ("clippy", "doc") for check in checks):
        inventory = workspace_check_features() if features_by_package is None else features_by_package
        feature_args = _supported_feature_args(packages, inventory)
    # Preserve the original grouped node test graph when Core runs alone:
    # Torii/daemon dev-dependencies supplied these two non-default features.
    # Explicit developer binaries retain their unit tests in this test-only graph.
    # The same graph owns doctests; shipping builds remain unchanged.
    selected_features = [
        feature for package, feature in (
            ("iroha_core", "iroha_core/iroha-core-tests"),
            ("iroha_core", "iroha_core/expensive-telemetry"),
            ("iroha_core", "iroha_core/dev-tools"),
            ("iroha_torii", "iroha_torii/test-fixtures"),
            ("iroha_release_bundle", "iroha_release_bundle/dev-tools"),
            ("iroha_native_source_delivery", "iroha_native_source_delivery/dev-tools"),
            ("xtask", "xtask/dev-tools"),
        ) if package in packages
    ]
    test_features = (["--features", ",".join(selected_features)]
                     if selected_features else [])
    commands: list[list[str]] = []
    for check in checks:
        if check == "clippy":
            commands.append(
                [
                    "cargo",
                    "clippy",
                    "--locked",
                    "--all-targets",
                    *feature_args,
                    *package_args,
                    "--",
                    "-D",
                    "warnings",
                ]
            )
        elif check == "build":
            commands.append(["cargo", "build", "--locked", *package_args])
        elif check == "test":
            # Select package-owned executable tests explicitly while preserving
            # the ordinary build's shipping-only binary inventory.
            test = ["cargo", "test", "--locked", "--no-fail-fast", *test_features, *package_args]
            if test_target == "library":
                commands.extend([
                    [*test, "--lib", "--", "--test-threads=1"],
                    [*test, "--doc", "--", "--test-threads=1"],
                ])
            elif test_target:
                commands.append([*test, "--test", test_target, "--", "--test-threads=1"])
            else:
                commands.append(test)
        elif check == "doc":
            commands.append(
                [
                    "cargo",
                    "doc",
                    "--locked",
                    "--no-deps",
                    *feature_args,
                    *package_args,
                ]
            )
        elif check == "doctest":
            commands.append(["cargo", "test", "--locked", "--no-fail-fast", *test_features, "--doc", *package_args])
        else:
            raise ClassificationError(
                f"unknown check {check!r}; choose from {', '.join(CHECK_NAMES)}"
            )
    return commands


def run_checks(
    packages: Sequence[str],
    checks: Sequence[str],
    *,
    root: Path = ROOT,
    dry_run: bool = False,
    workspace: bool = False,
    local: bool = False,
    test_target: str | None = None,
    native_shard: int | None = None,
) -> None:
    """Execute locked checks, optionally reusing the local incremental build lane."""

    if native_shard is not None:
        selection = packages[0] + (":" + test_target if test_target else "") if len(packages) == 1 else ""
        if (workspace or local or tuple(checks) != ("test",)
                or selection not in NATIVE_TEST_SHARDS):
            raise ClassificationError("native sharding requires one declared package and only its native tests")
        command, = commands_for_checks(packages, ("test",))
        NATIVE_SHARDS.run_shard(root, packages[0], native_shard, NATIVE_TEST_SHARDS[selection][1],
                               test_target=test_target, cargo_test_command=command, dry_run=dry_run)
        return
    inventory = workspace_check_features(root)
    if workspace:
        packages = tuple(sorted(inventory))
    commands = commands_for_checks(
        packages, checks, features_by_package=inventory, workspace=workspace,
        test_target=test_target,
    )
    if not commands:
        print("No affected Rust packages; Cargo validation is not required.")
        return
    for command in commands:
        if local:
            command = [
                str(root / "scripts" / "cargo_fast.sh"),
                "--stable-local-metadata", "--incremental", "--", *command[1:],
            ]
        print("+", " ".join(command), flush=True)
        if not dry_run:
            _run(command, cwd=root, capture_output=False)


def _write_json(path: Path, result: Classification) -> None:
    """Write deterministic classifier JSON atomically."""

    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        delete=False,
    ) as handle:
        json.dump(result.as_dict(), handle, indent=2, sort_keys=True)
        handle.write("\n")
        temporary = Path(handle.name)
    temporary.replace(path)


def _write_github_output(path: Path, result: Classification) -> None:
    """Append compact values consumed by the PR workflow."""

    document = result.as_dict()
    with path.open("a", encoding="utf-8") as handle:
        handle.write(
            f"has_rust={'true' if result.has_rust else 'false'}\n"
            f"full={'true' if result.full else 'false'}\n"
            f"matrix={json.dumps(document['matrix'], separators=(',', ':'))}\n"
            f"package_count={len(result.impacted_packages)}\n"
        )
        for key in ("has_binary_free_rust", "has_binary_rust", "has_binaries"):
            handle.write(f"{key}={str(document[key]).lower()}\n")
        for key in ("binary_free_matrix", "binary_matrix"):
            handle.write(f"{key}={json.dumps(document[key], separators=(',', ':'))}\n")
        handle.write(f"binaries={','.join(result.binaries)}\n")
        for name, selected in result.consumers.items():
            handle.write(f"run_{name}={str(selected).lower()}\n")


def build_binaries(
    binaries: Sequence[str], output_dir: Path, *, root: Path = ROOT
) -> None:
    """Build each selected feature group separately and stage only its exact artifacts."""

    selected = _binary_names(binaries, "binaries")
    if output_dir.exists():
        raise ClassificationError(f"binary staging directory already exists: {output_dir}")
    groups: dict[str, list[str]] = defaultdict(list)
    for name in selected:
        groups[BINARY_ARTIFACTS[name].target_group].append(name)
    target_root = root / "target" / "ci-binaries"
    for group, names in sorted(groups.items()):
        command = [
            "cargo", "build", "--locked", "--release", "--target-dir", str(target_root / group)
        ]
        for name in names:
            artifact = BINARY_ARTIFACTS[name]
            command.extend(("-p", artifact.package, "--bin", artifact.binary))
            if artifact.features:
                command.extend(("--features", ",".join(
                    f"{artifact.package}/{feature}" for feature in artifact.features
                )))
        _run(command, cwd=root, capture_output=False)
    output_dir.mkdir(parents=True, exist_ok=False)
    for name in selected:
        artifact = BINARY_ARTIFACTS[name]
        shutil.copy2(
            target_root / artifact.target_group / "release" / artifact.binary,
            output_dir / name,
        )


def _parse_packages(raw: str) -> tuple[str, ...]:
    """Require at least one package in an explicit comma-separated selection."""

    packages = tuple(sorted({package.strip() for package in raw.split(",") if package.strip()}))
    if not packages:
        raise ClassificationError("packages must select at least one Cargo package")
    return packages


def _parse_checks(raw: str) -> tuple[str, ...]:
    """Parse and validate a comma-separated check list."""

    checks = tuple(check.strip() for check in raw.split(",") if check.strip())
    invalid = sorted(set(checks) - set(CHECK_NAMES))
    if invalid or not checks:
        raise ClassificationError(
            f"checks must be a non-empty subset of {', '.join(CHECK_NAMES)}"
        )
    return checks


def _classification_from_args(args: argparse.Namespace) -> Classification:
    """Load inputs and produce one classification for CLI commands."""

    metadata = load_cargo_metadata(
        root=ROOT,
        metadata_path=Path(args.metadata) if args.metadata else None,
    )
    manifest = load_lane_manifest(Path(args.manifest))
    comparison_revision = "HEAD"
    if args.all:
        changed_paths = ("Cargo.toml",)
    elif args.paths:
        changed_paths = tuple(args.paths)
    elif args.paths_file:
        if args.paths_file == "-":
            changed_paths = tuple(line.rstrip("\n") for line in sys.stdin if line.strip())
        else:
            changed_paths = tuple(
                Path(args.paths_file).read_text(encoding="utf-8").splitlines()
            )
    else:
        base = args.base if args.base is not None else default_base()
        changed_paths = git_changed_paths(base)
        if base is not None:
            comparison_revision = _run(("git", "merge-base", base, "HEAD")).stdout.strip()
        if base is None:
            # A new repository without a comparison commit cannot prove that an
            # empty working tree is unaffected.
            changed_paths = (*changed_paths, "Cargo.toml")
    return classify_paths(
        changed_paths,
        metadata=metadata,
        manifest=manifest,
        root=ROOT,
        base_revision=comparison_revision,
    )


def _add_classification_arguments(parser: argparse.ArgumentParser) -> None:
    """Add common classifier input switches to an argparse parser."""

    parser.add_argument(
        "--manifest",
        default=str(DEFAULT_MANIFEST),
        help="lane manifest (default: ci/rust_lanes.toml)",
    )
    parser.add_argument(
        "--metadata",
        help="read Cargo metadata JSON from this file instead of invoking Cargo",
    )
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--all", action="store_true", help="select every Rust lane")
    source.add_argument("--base", help="classify changes since this Git merge base")
    source.add_argument(
        "--paths", nargs="+", help="classify these repository-relative paths"
    )
    source.add_argument(
        "--paths-file", help="classify newline-delimited paths from a file or '-'"
    )


def build_parser() -> argparse.ArgumentParser:
    """Build the command-line interface."""

    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    validate = subparsers.add_parser(
        "validate", help="verify exhaustive lane ownership against Cargo metadata"
    )
    validate.add_argument("--manifest", default=str(DEFAULT_MANIFEST))
    validate.add_argument("--metadata")

    classify = subparsers.add_parser(
        "classify", help="classify changed paths and print affected lanes"
    )
    _add_classification_arguments(classify)
    classify.add_argument("--json-out", help="also write the complete JSON result")
    classify.add_argument(
        "--github-output", help="append matrix outputs to this GitHub output file"
    )

    run = subparsers.add_parser(
        "run", help="run locked Cargo checks for a classifier package list"
    )
    selection = run.add_mutually_exclusive_group(required=True)
    selection.add_argument(
        "--packages", help="comma-separated Cargo package names"
    )
    selection.add_argument(
        "--workspace", action="store_true", help="check every workspace feature owner"
    )
    run.add_argument(
        "--checks",
        default="clippy,build,test",
        help=f"comma-separated checks ({', '.join(CHECK_NAMES)})",
    )
    run.add_argument(
        "--dry-run", action="store_true", help="print commands without executing them"
    )
    run.add_argument(
        "--local", action="store_true",
        help="use cargo_fast with stable development metadata and incremental reuse",
    )
    run.add_argument(
        "--test-target", default="",
        help="one declared integration harness, or library for unit tests and doctests",
    )
    run.add_argument("--native-shard", default="", help="one-based native test shard for a declared package")
    verify = subparsers.add_parser("verify-native-shards", help="require complete disjoint native test receipts")
    verify.add_argument("--matrix", action="append", required=True, help="classifier matrix JSON")
    verify.add_argument("--receipts", type=Path, required=True)
    binaries = subparsers.add_parser(
        "build-binaries", help="build selected binaries with their declared feature isolation"
    )
    binaries.add_argument(
        "--binaries", required=True, help="comma-separated binary artifact names"
    )
    binaries.add_argument(
        "--output-dir", required=True, type=Path, help="new artifact staging directory"
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Run the CLI and return a process exit status."""

    args = build_parser().parse_args(argv)
    try:
        if args.command == "validate":
            metadata = load_cargo_metadata(
                metadata_path=Path(args.metadata) if args.metadata else None
            )
            packages = workspace_packages(metadata)
            manifest = load_lane_manifest(Path(args.manifest))
            validate_manifest(manifest, packages)
            print(
                f"Rust lane manifest covers {len(packages)} packages "
                f"across {len(manifest.lanes)} lanes."
            )
        elif args.command == "classify":
            result = _classification_from_args(args)
            document = result.as_dict()
            print(json.dumps(document, indent=2, sort_keys=True))
            if args.json_out:
                _write_json(Path(args.json_out), result)
            if args.github_output:
                _write_github_output(Path(args.github_output), result)
        elif args.command == "run":
            run_checks(
                () if args.workspace else _parse_packages(args.packages),
                _parse_checks(args.checks),
                dry_run=args.dry_run,
                workspace=args.workspace,
                local=args.local,
                **({"test_target": args.test_target} if args.test_target else {}),
                **({"native_shard": int(args.native_shard)} if args.native_shard else {}),
            )
        elif args.command == "verify-native-shards":
            NATIVE_SHARDS.verify_receipts(ROOT, args.receipts, [json.loads(value) for value in args.matrix], NATIVE_TEST_SHARDS)
        elif args.command == "build-binaries":
            build_binaries(_parse_packages(args.binaries), args.output_dir)
        else:
            raise ClassificationError(f"unsupported command: {args.command}")
    except (ClassificationError, OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
