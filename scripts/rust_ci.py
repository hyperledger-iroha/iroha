#!/usr/bin/env python3
"""Classify and run affected Rust workspace validation lanes.

The classifier requires Python 3.9+ (and the pinned ``tomli`` on Python before
3.11), a locked Cargo workspace, and Git when paths are not supplied
explicitly. It never mutates tracked sources. Unknown, ambiguous, or deleted
Rust ownership fails closed to every lane.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
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
DEFAULT_MANIFEST = ROOT / "ci" / "rust_lanes.toml"
CHECK_NAMES = ("clippy", "build", "test", "doc")
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
    feature_definitions: dict[str, tuple[str, ...]]
    dependencies: tuple["WorkspaceDependency", ...]


@dataclass(frozen=True)
class WorkspaceDependency:
    """One Cargo dependency declaration targeting another workspace package."""

    alias: str
    package: str
    optional: bool
    uses_default_features: bool
    features: tuple[str, ...]
    kind: str
    target: str | None


@dataclass(frozen=True)
class FeatureExclusion:
    """A host-specific feature omitted from generic Clippy and docs."""

    features: tuple[str, ...]
    reason: str


@dataclass(frozen=True)
class LaneManifest:
    """Validated lane ownership and non-package path-routing policy."""

    lanes: dict[str, tuple[str, ...]]
    generated_patterns: tuple[str, ...]
    all_patterns: tuple[str, ...]
    ignore_patterns: tuple[str, ...]
    lane_patterns: dict[str, tuple[str, ...]]
    feature_exclusions: dict[str, FeatureExclusion] = field(default_factory=dict)

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
        return {
            "version": 1,
            "has_rust": self.has_rust,
            "full": self.full,
            "changed_paths": list(self.changed_paths),
            "changed_packages": list(self.changed_packages),
            "impacted_packages": list(self.impacted_packages),
            "lanes": [
                {"name": item["lane"], "packages": item["packages"].split(",")}
                for item in include
            ],
            "matrix": {"include": include},
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
    raw_workspace_packages: dict[str, dict[str, Any]] = {}
    names: dict[str, str] = {}
    for raw in metadata.get("packages", ()):
        if raw.get("id") not in member_ids:
            continue
        package_id = raw.get("id")
        name = raw.get("name")
        manifest_path = raw.get("manifest_path")
        if (
            not isinstance(package_id, str)
            or not isinstance(name, str)
            or not isinstance(manifest_path, str)
        ):
            raise ClassificationError(
                "Cargo metadata contains an invalid workspace package"
            )
        if name in names.values():
            raise ClassificationError(f"workspace package name is not unique: {name}")
        raw_workspace_packages[package_id] = raw
        names[package_id] = name
    missing_ids = member_ids - set(raw_workspace_packages)
    if missing_ids:
        raise ClassificationError(
            f"Cargo metadata omits workspace package records: {sorted(missing_ids)}"
        )

    resolved_targets: dict[tuple[str, str], set[str]] = defaultdict(set)
    resolve = metadata.get("resolve")
    if isinstance(resolve, dict) and isinstance(resolve.get("nodes"), list):
        for node in resolve["nodes"]:
            if not isinstance(node, dict):
                raise ClassificationError(
                    "Cargo metadata contains an invalid resolve node"
                )
            dependent_id = node.get("id")
            if dependent_id not in raw_workspace_packages:
                continue
            raw_deps = node.get("deps", ())
            if not isinstance(raw_deps, list):
                raise ClassificationError(
                    "Cargo metadata contains invalid resolved dependencies"
                )
            for dependency in raw_deps:
                if not isinstance(dependency, dict):
                    raise ClassificationError(
                        "Cargo metadata contains invalid resolved dependencies"
                    )
                alias = dependency.get("name")
                target = names.get(dependency.get("pkg"))
                if isinstance(alias, str) and alias and target is not None:
                    resolved_targets[(dependent_id, alias)].add(target)

    package_directories = {
        Path(raw["manifest_path"]).parent.resolve(): names[package_id]
        for package_id, raw in raw_workspace_packages.items()
    }
    packages: dict[str, WorkspacePackage] = {}
    workspace_names = set(names.values())
    for package_id, raw in raw_workspace_packages.items():
        name = names[package_id]
        manifest_path = raw["manifest_path"]
        directory = _repository_relative(Path(manifest_path).parent, root)
        raw_features = raw.get("features", {})
        if not isinstance(raw_features, dict) or not all(
            isinstance(feature, str)
            and feature
            and isinstance(members, list)
            and all(isinstance(member, str) and member for member in members)
            for feature, members in raw_features.items()
        ):
            raise ClassificationError(
                f"Cargo metadata contains invalid features for package {name!r}"
            )
        feature_definitions = {
            feature: tuple(members) for feature, members in raw_features.items()
        }
        raw_dependencies = raw.get("dependencies", [])
        if not isinstance(raw_dependencies, list):
            raise ClassificationError(
                f"Cargo metadata contains invalid dependencies for package {name!r}"
            )
        dependencies: list[WorkspaceDependency] = []
        for dependency in raw_dependencies:
            if not isinstance(dependency, dict):
                raise ClassificationError(
                    f"Cargo metadata contains invalid dependencies for package {name!r}"
                )
            dependency_name = dependency.get("name")
            rename = dependency.get("rename")
            optional = dependency.get("optional")
            uses_default_features = dependency.get("uses_default_features")
            dependency_features = dependency.get("features")
            kind = dependency.get("kind")
            target = dependency.get("target")
            path = dependency.get("path")
            if (
                not isinstance(dependency_name, str)
                or not dependency_name
                or (rename is not None and (not isinstance(rename, str) or not rename))
                or not isinstance(optional, bool)
                or not isinstance(uses_default_features, bool)
                or not isinstance(dependency_features, list)
                or not all(
                    isinstance(feature, str) and feature
                    for feature in dependency_features
                )
                or kind not in (None, "normal", "dev", "build")
                or (target is not None and not isinstance(target, str))
                or (path is not None and not isinstance(path, str))
            ):
                raise ClassificationError(
                    f"Cargo metadata contains invalid dependencies for package {name!r}"
                )
            alias = rename or dependency_name
            targets = set(resolved_targets.get((package_id, alias), ()))
            if path is not None:
                path_target = package_directories.get(Path(path).resolve())
                if path_target is not None:
                    targets.add(path_target)
            if (
                not targets
                and dependency_name in workspace_names
                and dependency.get("source") is None
            ):
                # Path dependencies normally carry both `path` and a resolve edge.
                # Retain this source-less fallback so disabled optional and
                # target-specific workspace edges cannot disappear from validation.
                targets.add(dependency_name)
            for dependency_target in sorted(targets):
                dependencies.append(
                    WorkspaceDependency(
                        alias=alias,
                        package=dependency_target,
                        optional=optional,
                        uses_default_features=uses_default_features,
                        features=tuple(dependency_features),
                        kind=kind or "normal",
                        target=target,
                    )
                )
        packages[name] = WorkspacePackage(
            package_id,
            name,
            directory,
            feature_definitions,
            tuple(dependencies),
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
    raw_feature_exclusions = raw.get("feature_exclusions", {})
    if not isinstance(raw_feature_exclusions, dict):
        raise ClassificationError("feature_exclusions must be a table")
    feature_exclusions: dict[str, FeatureExclusion] = {}
    for package, settings in raw_feature_exclusions.items():
        if not isinstance(settings, dict) or set(settings) != {"features", "reason"}:
            raise ClassificationError(
                f"feature_exclusions.{package} must contain exactly features and reason"
            )
        features = _feature_names(
            settings["features"], f"feature_exclusions.{package}.features"
        )
        reason = settings["reason"]
        if not isinstance(reason, str) or not reason.strip():
            raise ClassificationError(
                f"feature_exclusions.{package}.reason must be non-empty"
            )
        feature_exclusions[package] = FeatureExclusion(features, reason)
    return LaneManifest(
        lanes=lanes,
        generated_patterns=generated_patterns,
        all_patterns=all_patterns,
        ignore_patterns=ignore_patterns,
        lane_patterns=lane_patterns,
        feature_exclusions=feature_exclusions,
    )


def _patterns(raw: Any, field: str) -> tuple[str, ...]:
    """Validate and normalize a list of repository-relative glob patterns."""

    if not isinstance(raw, (list, tuple)) or not all(
        isinstance(pattern, str) and pattern and not pattern.startswith("/")
        for pattern in raw
    ):
        raise ClassificationError(f"{field} must contain relative glob strings")
    return tuple(raw)


def _feature_names(raw: Any, field: str) -> tuple[str, ...]:
    """Validate a non-empty list of unique Cargo feature names."""

    if not isinstance(raw, list) or not raw or not all(
        isinstance(feature, str)
        and feature
        and set(feature) <= PACKAGE_NAME_CHARACTERS
        for feature in raw
    ):
        raise ClassificationError(f"{field} must contain Cargo feature names")
    if len(set(raw)) != len(raw):
        raise ClassificationError(f"{field} must not contain duplicates")
    if "default" in raw:
        raise ClassificationError(f"{field} cannot exclude the default feature")
    return tuple(sorted(raw))


def _workspace_feature_closure(
    root_profiles: Mapping[str, Iterable[str]],
    packages: Mapping[str, WorkspacePackage],
) -> set[tuple[str, str]]:
    """Conservatively resolve Cargo features reachable from command roots.

    Generic Clippy uses all targets, and generic documentation is deliberately
    checked against the same upper bound. Consequently every normal, build,
    development, and target-specific workspace dependency declaration is
    considered. External dependencies cannot re-enable a workspace exclusion
    and are omitted when metadata is normalized.
    """

    dependencies_by_alias: dict[
        tuple[str, str], tuple[WorkspaceDependency, ...]
    ] = {}
    for package in packages.values():
        grouped: dict[str, list[WorkspaceDependency]] = defaultdict(list)
        for dependency in package.dependencies:
            grouped[dependency.alias].append(dependency)
        dependencies_by_alias.update(
            {
                (package.name, alias): tuple(dependencies)
                for alias, dependencies in grouped.items()
            }
        )

    active_packages: set[str] = set()
    active_dependencies: set[tuple[str, str]] = set()
    enabled_features: set[tuple[str, str]] = set()
    weak_features: dict[tuple[str, str], set[str]] = defaultdict(set)
    pending_packages: deque[str] = deque()
    pending_dependencies: deque[tuple[str, str]] = deque()
    pending_features: deque[tuple[str, str]] = deque()

    def activate_package(package_name: str) -> None:
        if package_name not in active_packages:
            active_packages.add(package_name)
            pending_packages.append(package_name)

    def activate_dependency(package_name: str, alias: str) -> None:
        key = (package_name, alias)
        if key in dependencies_by_alias and key not in active_dependencies:
            active_dependencies.add(key)
            pending_dependencies.append(key)

    def enable_feature(package_name: str, feature: str) -> None:
        key = (package_name, feature)
        if key not in enabled_features:
            enabled_features.add(key)
            pending_features.append(key)

    for root_package, root_features in root_profiles.items():
        activate_package(root_package)
        for feature in root_features:
            enable_feature(root_package, feature)

    while pending_packages or pending_dependencies or pending_features:
        while pending_packages:
            package_name = pending_packages.popleft()
            package = packages[package_name]
            for dependency in package.dependencies:
                if not dependency.optional:
                    activate_dependency(package_name, dependency.alias)

        while pending_dependencies:
            package_name, alias = pending_dependencies.popleft()
            for dependency in dependencies_by_alias[(package_name, alias)]:
                activate_package(dependency.package)
                definitions = packages[dependency.package].feature_definitions
                if dependency.uses_default_features and "default" in definitions:
                    enable_feature(dependency.package, "default")
                for feature in dependency.features:
                    enable_feature(dependency.package, feature)
                for feature in weak_features[(package_name, alias)]:
                    enable_feature(dependency.package, feature)

        while pending_features:
            package_name, feature = pending_features.popleft()
            package = packages[package_name]
            for member in package.feature_definitions.get(feature, ()):
                if member in package.feature_definitions:
                    enable_feature(package_name, member)
                    continue
                if member.startswith("dep:"):
                    activate_dependency(package_name, member.removeprefix("dep:"))
                    continue
                if "/" in member:
                    alias, dependency_feature = member.split("/", 1)
                    weak = alias.endswith("?")
                    alias = alias.removesuffix("?")
                    dependency_key = (package_name, alias)
                    if weak:
                        weak_features[dependency_key].add(dependency_feature)
                        if dependency_key not in active_dependencies:
                            continue
                    else:
                        activate_dependency(package_name, alias)
                    for dependency in dependencies_by_alias.get(dependency_key, ()):
                        enable_feature(dependency.package, dependency_feature)
                    continue
                # Cargo's legacy implicit optional-dependency feature syntax.
                activate_dependency(package_name, member)

    return enabled_features


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
    unknown_exclusion_packages = sorted(
        set(manifest.feature_exclusions) - workspace_names
    )
    if unknown_exclusion_packages:
        errors.append(
            "feature exclusions reference packages absent from workspace: "
            f"{unknown_exclusion_packages}"
        )
    for package_name, exclusion in manifest.feature_exclusions.items():
        package = packages.get(package_name)
        if package is None:
            continue
        definitions = package.feature_definitions
        unknown_features = sorted(set(exclusion.features) - set(definitions))
        if unknown_features:
            errors.append(
                f"feature exclusions for {package_name} are absent from Cargo metadata: "
                f"{unknown_features}"
            )
            continue
    valid_exclusions = {
        (package_name, feature)
        for package_name, exclusion in manifest.feature_exclusions.items()
        if package_name in packages
        for feature in exclusion.features
        if feature in packages[package_name].feature_definitions
    }
    reenabled_by_profile: list[str] = []

    def record_reenabled(profile: str, reached: set[tuple[str, str]]) -> None:
        reenabled_by_profile.extend(
            f"{profile} -> {package_name}/{feature}"
            for package_name, feature in sorted(valid_exclusions & reached)
        )

    for lane, lane_packages in manifest.lanes.items():
        ordinary_profiles = {
            package_name: set(packages[package_name].feature_definitions)
            for package_name in lane_packages
            if package_name in packages
            and package_name not in manifest.feature_exclusions
        }
        if not ordinary_profiles:
            continue
        ordinary_names = list(ordinary_profiles)
        profile = (
            ordinary_names[0]
            if len(ordinary_names) == 1
            else f"{lane} lane [{', '.join(ordinary_names)}]"
        )
        record_reenabled(
            profile,
            _workspace_feature_closure(ordinary_profiles, packages),
        )

    for package_name, exclusion in manifest.feature_exclusions.items():
        package = packages.get(package_name)
        if package is None:
            continue
        enabled_roots = set(package.feature_definitions) - {
            "default",
            *exclusion.features,
        }
        if "default" in package.feature_definitions:
            enabled_roots.add("default")
        record_reenabled(
            package_name,
            _workspace_feature_closure(
                {package_name: enabled_roots}, packages
            ),
        )
    if reenabled_by_profile:
        errors.append(
            "feature exclusions are re-enabled by generic command profiles: "
            f"{reenabled_by_profile}"
        )
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
    """Build a workspace-only reverse graph from Cargo's resolved dependency graph."""

    by_id = {package.package_id: package.name for package in packages.values()}
    reverse: dict[str, set[str]] = {name: set() for name in packages}
    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict) or not isinstance(resolve.get("nodes"), list):
        raise ClassificationError(
            "full Cargo metadata with a dependency resolve graph is required"
        )
    for node in resolve["nodes"]:
        dependent = by_id.get(node.get("id"))
        if dependent is None:
            continue
        for dependency in node.get("deps", ()):
            dependency_name = by_id.get(dependency.get("pkg"))
            if dependency_name is not None:
                reverse[dependency_name].add(dependent)
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
    for path in normalized_paths:
        if _matches(path, manifest.generated_patterns):
            continue
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
    lane_packages = {
        lane: tuple(
            sorted(
                package for package in impacted if package_lane[package] == lane
            )
        )
        for lane in manifest.lanes
        if any(package_lane[package] == lane for package in impacted)
    }
    return Classification(
        changed_paths=normalized_paths,
        changed_packages=tuple(sorted(seed_packages)),
        impacted_packages=tuple(sorted(impacted)),
        lane_packages=lane_packages,
        full=full,
        reasons=tuple(dict.fromkeys(reasons)),
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


def default_base(root: Path = ROOT) -> str | None:
    """Choose a discoverable local comparison base without network access."""

    for reference in ("origin/main", "@{upstream}", "HEAD^"):
        result = subprocess.run(
            ("git", "rev-parse", "--verify", "--quiet", reference),
            cwd=root,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            return reference
    return None


def commands_for_checks(
    packages: Sequence[str],
    checks: Sequence[str],
    *,
    feature_exclusions: Mapping[str, FeatureExclusion] | None = None,
    workspace: Mapping[str, WorkspacePackage] | None = None,
) -> list[list[str]]:
    """Build locked, package-scoped Cargo validation commands."""

    if not packages:
        return []
    invalid_packages = [
        package
        for package in packages
        if not package or not set(package) <= PACKAGE_NAME_CHARACTERS
    ]
    if invalid_packages:
        raise ClassificationError(f"invalid Cargo package names: {invalid_packages}")
    exclusions = feature_exclusions or {}
    if exclusions and workspace is None:
        raise ClassificationError(
            "workspace metadata is required when feature exclusions are configured"
        )
    selected_exclusions = {
        package: exclusions[package] for package in packages if package in exclusions
    }
    ordinary_packages = [
        package for package in packages if package not in selected_exclusions
    ]
    package_args = [
        argument for package in packages for argument in ("-p", package)
    ]
    ordinary_package_args = [
        argument for package in ordinary_packages for argument in ("-p", package)
    ]

    def feature_complete_commands(check: str) -> list[list[str]]:
        check_args = ["--all-targets"] if check == "clippy" else ["--no-deps"]
        suffix = ["--", "-D", "warnings"] if check == "clippy" else []
        commands = []
        if ordinary_packages:
            commands.append(
                [
                    "cargo",
                    check,
                    "--locked",
                    *check_args,
                    "--all-features",
                    *ordinary_package_args,
                    *suffix,
                ]
            )
        for package, exclusion in selected_exclusions.items():
            assert workspace is not None
            metadata = workspace.get(package)
            if metadata is None:
                raise ClassificationError(
                    f"selected Cargo package is absent from workspace metadata: {package}"
                )
            enabled = sorted(
                set(metadata.feature_definitions)
                - {"default", *exclusion.features}
            )
            feature_args = ["--features", ",".join(enabled)] if enabled else []
            commands.append(
                [
                    "cargo",
                    check,
                    "--locked",
                    *check_args,
                    "-p",
                    package,
                    *feature_args,
                    *suffix,
                ]
            )
        return commands

    commands: list[list[str]] = []
    for check in checks:
        if check == "clippy":
            commands.extend(feature_complete_commands("clippy"))
        elif check == "build":
            commands.append(["cargo", "build", "--locked", *package_args])
        elif check == "test":
            commands.append(
                ["cargo", "test", "--locked", "--no-fail-fast", *package_args]
            )
        elif check == "doc":
            commands.extend(feature_complete_commands("doc"))
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
    feature_exclusions: Mapping[str, FeatureExclusion] | None = None,
    workspace: Mapping[str, WorkspacePackage] | None = None,
) -> None:
    """Execute locked Cargo checks for a deterministic package set."""

    commands = commands_for_checks(
        packages,
        checks,
        feature_exclusions=feature_exclusions,
        workspace=workspace,
    )
    if not commands:
        print("No affected Rust packages; Cargo validation is not required.")
        return
    for command in commands:
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


def _parse_packages(raw: str) -> tuple[str, ...]:
    """Parse a comma-separated package list from a trusted classifier result."""

    return tuple(sorted({package.strip() for package in raw.split(",") if package.strip()}))


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
        if base is None:
            # A new repository without a comparison commit cannot prove that an
            # empty working tree is unaffected.
            changed_paths = (*changed_paths, "Cargo.toml")
    return classify_paths(
        changed_paths,
        metadata=metadata,
        manifest=manifest,
        root=ROOT,
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
    run.add_argument(
        "--packages", required=True, help="comma-separated Cargo package names"
    )
    run.add_argument(
        "--checks",
        default="clippy,build,test",
        help=f"comma-separated checks ({', '.join(CHECK_NAMES)})",
    )
    run.add_argument(
        "--dry-run", action="store_true", help="print commands without executing them"
    )
    run.add_argument("--manifest", default=str(DEFAULT_MANIFEST))
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
            metadata = load_cargo_metadata()
            packages = workspace_packages(metadata)
            manifest = load_lane_manifest(Path(args.manifest))
            validate_manifest(manifest, packages)
            run_checks(
                _parse_packages(args.packages),
                _parse_checks(args.checks),
                dry_run=args.dry_run,
                feature_exclusions=manifest.feature_exclusions,
                workspace=packages,
            )
        else:
            raise ClassificationError(f"unsupported command: {args.command}")
    except (ClassificationError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
