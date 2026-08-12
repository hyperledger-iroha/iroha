#!/usr/bin/env python3
"""Enforce focused, reproducible Cargo dependency budgets.

The default check reads Cargo manifests directly.  This keeps the pull-request
ratchet host-independent and makes it useful even before dependencies are
fetched.  ``--resolved`` retains a diagnostic view of Cargo's resolved graph;
tests can provide a captured response with ``--metadata-json``.

Prerequisites are Python 3.11+ or the repository's pinned ``tomli`` backport.
No environment variables are required. The default is read-only; only the
explicit ``--write-baseline`` option updates the selected JSON configuration.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from collections import Counter, deque
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 compatibility
    import tomli as tomllib  # type: ignore[no-redef]


DEFAULT_CONFIG = Path("ci/dependency_budget.json")
MEASUREMENT_KIND = "cargo-manifest-source-graph-v1"
METRIC_KEYS = (
    "required_local_packages",
    "required_workspace_packages",
    "required_path_packages",
    "required_external_packages",
    "required_dependency_edges",
    "required_external_dependency_edges",
    "declared_local_packages",
    "declared_workspace_packages",
    "declared_path_packages",
    "declared_external_packages",
    "declared_dependency_edges",
    "declared_external_dependency_edges",
)

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
    "proptest",
)


@dataclass(frozen=True)
class DependencyEdge:
    """One dependency declaration in a local Cargo manifest."""

    owner: Path
    alias: str
    package_name: str
    kind: str
    optional: bool
    target: str | None
    local_manifest: Path | None


@dataclass(frozen=True)
class LocalPackage:
    """A workspace member or a transitively referenced local path package."""

    name: str
    manifest: Path
    workspace_member: bool
    dependencies: tuple[DependencyEdge, ...]


@dataclass(frozen=True)
class ManifestGraph:
    """The deterministic local manifest graph used by the source ratchet."""

    repository_root: Path
    root_manifest: Path
    packages: Mapping[Path, LocalPackage]
    workspace_members: frozenset[Path]
    workspace_names: Mapping[str, Path]
    manifest_fingerprint: str


@dataclass(frozen=True)
class Closure:
    """One required or all-declared dependency closure."""

    local_packages: frozenset[Path]
    external_packages: frozenset[str]
    dependency_edges: int
    external_dependency_edges: int
    package_names: frozenset[str]


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Enforce checked-in focused manifest-graph budgets, or report an "
            "opt-in Cargo resolved graph."
        )
    )
    parser.add_argument(
        "--manifest-path",
        type=Path,
        default=Path("Cargo.toml"),
        help="Workspace manifest to inspect.",
    )
    parser.add_argument(
        "--config",
        type=Path,
        default=DEFAULT_CONFIG,
        help="Checked-in manifest-graph budget configuration.",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write deterministic evidence JSON; use `-` for stdout.",
    )
    parser.add_argument(
        "--write-baseline",
        action="store_true",
        help=(
            "Replace every configured limit with the current observation and "
            "refresh its manifest fingerprint. Review the diff before commit."
        ),
    )

    resolved = parser.add_argument_group("resolved Cargo metadata diagnostics")
    resolved.add_argument(
        "--resolved",
        action="store_true",
        help="Inspect Cargo's resolved graph instead of enforcing the source ratchet.",
    )
    resolved.add_argument(
        "--metadata-json",
        type=Path,
        help="Read a captured Cargo metadata response instead of invoking Cargo.",
    )
    resolved.add_argument(
        "--allow-lock-update",
        action="store_true",
        help="Omit --locked in resolved mode. This may rewrite Cargo.lock.",
    )
    resolved.add_argument(
        "-p",
        "--package",
        action="append",
        default=[],
        help="Resolved-graph root package. May be repeated.",
    )
    resolved.add_argument(
        "--workspace",
        action="store_true",
        help="Use every workspace member as a resolved-graph root.",
    )
    resolved.add_argument(
        "--max-registry-packages",
        type=int,
        help="Fail if the focused resolved registry count exceeds this value.",
    )
    resolved.add_argument(
        "--max-total-packages",
        type=int,
        help="Fail if the focused resolved package count exceeds this value.",
    )
    resolved.add_argument(
        "--watch",
        action="append",
        default=[],
        help="Additional resolved package name to report. May be repeated.",
    )
    resolved.add_argument(
        "--deny",
        action="append",
        default=[],
        help="Additional resolved package name that must be absent.",
    )
    return parser.parse_args(argv)


def _read_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as source:
            payload = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"failed to read {path}: {error}") from error
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a TOML table")
    return payload


def _manifest_for_path(path: Path) -> Path:
    candidate = path / "Cargo.toml" if path.name != "Cargo.toml" else path
    return candidate.resolve()


def workspace_member_manifests(
    repository_root: Path, workspace: Mapping[str, Any]
) -> tuple[Path, ...]:
    """Expand Cargo workspace member globs and exclusions deterministically."""

    members = workspace.get("members")
    if not isinstance(members, list) or not all(isinstance(row, str) for row in members):
        raise ValueError("[workspace].members must be a list of strings")
    excludes = workspace.get("exclude", [])
    if not isinstance(excludes, list) or not all(isinstance(row, str) for row in excludes):
        raise ValueError("[workspace].exclude must be a list of strings")

    excluded: set[Path] = set()
    for pattern in excludes:
        matches = repository_root.glob(pattern)
        excluded.update(_manifest_for_path(path) for path in matches)

    manifests: set[Path] = set()
    for pattern in members:
        matches = sorted(repository_root.glob(pattern))
        wildcard = any(character in pattern for character in "*?[")
        if not matches and not wildcard:
            matches = [repository_root / pattern]
        for path in matches:
            manifest = _manifest_for_path(path)
            # Cargo member globs commonly match README files alongside crate
            # directories. Cargo ignores those non-manifest paths.
            if manifest not in excluded and (manifest.is_file() or not wildcard):
                manifests.add(manifest)
    missing = sorted(path for path in manifests if not path.is_file())
    if missing:
        rendered = ", ".join(str(path) for path in missing)
        raise ValueError(f"workspace member manifests are missing: {rendered}")
    return tuple(sorted(manifests))


def _dependency_tables(
    payload: Mapping[str, Any],
) -> Iterable[tuple[str, str | None, Mapping[str, Any]]]:
    table_names = {
        "dependencies": "normal",
        "build-dependencies": "build",
        "dev-dependencies": "dev",
    }
    for table_name, kind in table_names.items():
        table = payload.get(table_name, {})
        if not isinstance(table, dict):
            raise ValueError(f"[{table_name}] must be a table")
        yield kind, None, table

    targets = payload.get("target", {})
    if not isinstance(targets, dict):
        raise ValueError("[target] must be a table")
    for target_name in sorted(targets):
        target = targets[target_name]
        if not isinstance(target, dict):
            raise ValueError(f"[target.{target_name}] must be a table")
        for table_name, kind in table_names.items():
            table = target.get(table_name, {})
            if not isinstance(table, dict):
                raise ValueError(f"[target.{target_name}.{table_name}] must be a table")
            yield kind, target_name, table


def _dependency_spec(value: Any, *, context: str) -> dict[str, Any]:
    if isinstance(value, str):
        return {"version": value}
    if isinstance(value, dict):
        return dict(value)
    raise ValueError(f"{context} must be a version string or table")


def _workspace_dependency_spec(
    alias: str,
    member_spec: Mapping[str, Any],
    workspace_dependencies: Mapping[str, Any],
) -> tuple[dict[str, Any], bool]:
    inherited = member_spec.get("workspace") is True
    if not inherited:
        return dict(member_spec), False
    if alias not in workspace_dependencies:
        raise ValueError(f"workspace dependency `{alias}` is not declared at the root")
    root_spec = _dependency_spec(
        workspace_dependencies[alias],
        context=f"workspace dependency `{alias}`",
    )
    merged = dict(root_spec)
    merged.update({key: value for key, value in member_spec.items() if key != "workspace"})
    return merged, True


def _path_manifest(base: Path, dependency_path: str, *, context: str) -> Path:
    path = Path(dependency_path)
    if not path.is_absolute():
        path = base / path
    manifest = _manifest_for_path(path)
    if not manifest.is_file():
        raise ValueError(f"{context} path manifest is missing: {manifest}")
    return manifest


def _path_patches(root_payload: Mapping[str, Any], repository_root: Path) -> dict[str, Path]:
    patches = root_payload.get("patch", {})
    if not isinstance(patches, dict):
        raise ValueError("[patch] must be a table")
    paths: dict[str, Path] = {}
    for registry in sorted(patches):
        table = patches[registry]
        if not isinstance(table, dict):
            raise ValueError(f"[patch.{registry}] must be a table")
        for alias in sorted(table):
            spec = _dependency_spec(table[alias], context=f"patch `{alias}`")
            dependency_path = spec.get("path")
            if not isinstance(dependency_path, str):
                continue
            package_name = spec.get("package", alias)
            if not isinstance(package_name, str):
                raise ValueError(f"patch `{alias}` package must be a string")
            paths[package_name] = _path_manifest(
                repository_root,
                dependency_path,
                context=f"patch `{alias}`",
            )
    return paths


def load_manifest_graph(manifest_path: Path) -> ManifestGraph:
    """Load workspace members plus every reachable local path dependency."""

    root_manifest = manifest_path.resolve()
    root_payload = _read_toml(root_manifest)
    workspace = root_payload.get("workspace")
    if not isinstance(workspace, dict):
        raise ValueError(f"{root_manifest} must contain [workspace]")
    repository_root = root_manifest.parent
    member_manifests = workspace_member_manifests(repository_root, workspace)
    member_set = frozenset(member_manifests)
    workspace_dependencies = root_payload.get("workspace", {}).get("dependencies", {})
    # Cargo stores [workspace.dependencies] inside the workspace table in TOML's
    # parsed representation. Keep a fallback for hand-built unit-test fixtures.
    if not isinstance(workspace_dependencies, dict):
        workspace_dependencies = root_payload.get("workspace.dependencies", {})
    if not isinstance(workspace_dependencies, dict):
        raise ValueError("[workspace.dependencies] must be a table")
    patches = _path_patches(root_payload, repository_root)

    packages: dict[Path, LocalPackage] = {}
    queued = deque(member_manifests)
    queued.extend(path for path in patches.values() if path not in member_set)
    while queued:
        package_manifest = queued.popleft().resolve()
        if package_manifest in packages:
            continue
        payload = _read_toml(package_manifest)
        package_table = payload.get("package")
        if not isinstance(package_table, dict) or not isinstance(
            package_table.get("name"), str
        ):
            raise ValueError(f"{package_manifest} must declare [package].name")

        dependencies: list[DependencyEdge] = []
        for kind, target, table in _dependency_tables(payload):
            for alias in sorted(table):
                raw_spec = _dependency_spec(
                    table[alias],
                    context=f"{package_manifest}: dependency `{alias}`",
                )
                spec, inherited = _workspace_dependency_spec(
                    alias, raw_spec, workspace_dependencies
                )
                package_name = spec.get("package", alias)
                if not isinstance(package_name, str):
                    raise ValueError(
                        f"{package_manifest}: dependency `{alias}` package must be a string"
                    )
                optional = spec.get("optional", False)
                if not isinstance(optional, bool):
                    raise ValueError(
                        f"{package_manifest}: dependency `{alias}` optional must be boolean"
                    )
                dependency_path = spec.get("path")
                local_manifest: Path | None = None
                if dependency_path is not None:
                    if not isinstance(dependency_path, str):
                        raise ValueError(
                            f"{package_manifest}: dependency `{alias}` path must be a string"
                        )
                    base = repository_root if inherited else package_manifest.parent
                    local_manifest = _path_manifest(
                        base,
                        dependency_path,
                        context=f"{package_manifest}: dependency `{alias}`",
                    )
                elif package_name in patches:
                    local_manifest = patches[package_name]
                dependencies.append(
                    DependencyEdge(
                        owner=package_manifest,
                        alias=alias,
                        package_name=package_name,
                        kind=kind,
                        optional=optional,
                        target=target,
                        local_manifest=local_manifest,
                    )
                )
                if local_manifest is not None and local_manifest not in packages:
                    queued.append(local_manifest)

        packages[package_manifest] = LocalPackage(
            name=package_table["name"],
            manifest=package_manifest,
            workspace_member=package_manifest in member_set,
            dependencies=tuple(dependencies),
        )

    workspace_names: dict[str, Path] = {}
    for manifest in member_manifests:
        package = packages[manifest]
        if package.name in workspace_names:
            raise ValueError(f"duplicate workspace package name `{package.name}`")
        workspace_names[package.name] = manifest

    digest = hashlib.sha256()
    manifests = {root_manifest, *packages.keys()}
    for path in sorted(manifests):
        try:
            label = path.relative_to(repository_root).as_posix()
        except ValueError:
            label = path.as_posix()
        digest.update(label.encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")

    return ManifestGraph(
        repository_root=repository_root,
        root_manifest=root_manifest,
        packages=packages,
        workspace_members=member_set,
        workspace_names=workspace_names,
        manifest_fingerprint=f"sha256:{digest.hexdigest()}",
    )


def _scope_roots(graph: ManifestGraph, roots: Sequence[str]) -> frozenset[Path]:
    if roots == ["*"] or roots == ("*",):
        return graph.workspace_members
    missing = sorted(set(roots).difference(graph.workspace_names))
    if missing:
        raise ValueError(f"unknown workspace root packages: {', '.join(missing)}")
    return frozenset(graph.workspace_names[name] for name in roots)


def dependency_closure(
    graph: ManifestGraph,
    *,
    roots: Sequence[str],
    include_optional: bool,
    include_root_dev_dependencies: bool,
) -> Closure:
    """Return a deterministic source closure for one focused package set.

    Development dependencies are considered only for explicit roots, matching
    Cargo's rule that dependency crates do not contribute their own dev graph.
    Target-specific declarations are included as a cross-platform upper bound.
    """

    root_manifests = _scope_roots(graph, roots)
    visited: set[Path] = set()
    external_packages: set[str] = set()
    package_names: set[str] = set()
    dependency_edges = 0
    external_dependency_edges = 0
    queue = deque(sorted(root_manifests))
    while queue:
        manifest = queue.popleft()
        if manifest in visited:
            continue
        visited.add(manifest)
        package = graph.packages[manifest]
        package_names.add(package.name)
        for dependency in package.dependencies:
            if dependency.kind == "dev" and (
                not include_root_dev_dependencies or manifest not in root_manifests
            ):
                continue
            if dependency.optional and not include_optional:
                continue
            dependency_edges += 1
            package_names.add(dependency.package_name)
            if dependency.local_manifest is None:
                external_dependency_edges += 1
                external_packages.add(dependency.package_name)
            elif dependency.local_manifest not in visited:
                queue.append(dependency.local_manifest)
    return Closure(
        local_packages=frozenset(visited),
        external_packages=frozenset(external_packages),
        dependency_edges=dependency_edges,
        external_dependency_edges=external_dependency_edges,
        package_names=frozenset(package_names),
    )


def _closure_metrics(prefix: str, graph: ManifestGraph, closure: Closure) -> dict[str, int]:
    workspace_count = sum(
        graph.packages[manifest].workspace_member
        for manifest in closure.local_packages
    )
    return {
        f"{prefix}_local_packages": len(closure.local_packages),
        f"{prefix}_workspace_packages": workspace_count,
        f"{prefix}_path_packages": len(closure.local_packages) - workspace_count,
        f"{prefix}_external_packages": len(closure.external_packages),
        f"{prefix}_dependency_edges": closure.dependency_edges,
        f"{prefix}_external_dependency_edges": closure.external_dependency_edges,
    }


def measure_scope(
    graph: ManifestGraph,
    *,
    roots: Sequence[str],
    include_root_dev_dependencies: bool,
) -> tuple[dict[str, int], Closure, Closure]:
    """Measure required and all-declared closures for a configured scope."""

    required = dependency_closure(
        graph,
        roots=roots,
        include_optional=False,
        include_root_dev_dependencies=include_root_dev_dependencies,
    )
    declared = dependency_closure(
        graph,
        roots=roots,
        include_optional=True,
        include_root_dev_dependencies=include_root_dev_dependencies,
    )
    metrics = {
        **_closure_metrics("required", graph, required),
        **_closure_metrics("declared", graph, declared),
    }
    return metrics, required, declared


def load_budget_config(path: Path) -> dict[str, Any]:
    try:
        config = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"failed to read {path}: {error}") from error
    if not isinstance(config, dict):
        raise ValueError("dependency budget config must be a JSON object")
    if config.get("schema_version") != 1:
        raise ValueError("dependency budget config schema_version must be 1")
    measurement = config.get("measurement")
    if not isinstance(measurement, dict) or measurement.get("kind") != MEASUREMENT_KIND:
        raise ValueError(f"measurement.kind must be `{MEASUREMENT_KIND}`")
    denied = config.get("denied_required_packages")
    if not isinstance(denied, list) or not all(isinstance(name, str) for name in denied):
        raise ValueError("denied_required_packages must be a list of strings")
    scopes = config.get("scopes")
    if not isinstance(scopes, dict) or not scopes:
        raise ValueError("dependency budget config must contain scopes")
    for scope_name, scope in scopes.items():
        if not isinstance(scope_name, str) or not isinstance(scope, dict):
            raise ValueError("each dependency budget scope must be an object")
        roots = scope.get("roots")
        if not isinstance(roots, list) or not roots or not all(
            isinstance(root, str) for root in roots
        ):
            raise ValueError(f"scope `{scope_name}` roots must be non-empty strings")
        if "*" in roots and roots != ["*"]:
            raise ValueError(f"scope `{scope_name}` wildcard root must stand alone")
        if not isinstance(scope.get("include_root_dev_dependencies"), bool):
            raise ValueError(
                f"scope `{scope_name}` include_root_dev_dependencies must be boolean"
            )
        limits = scope.get("limits")
        if not isinstance(limits, dict):
            raise ValueError(f"scope `{scope_name}` limits must be an object")
        missing = sorted(set(METRIC_KEYS).difference(limits))
        unknown = sorted(set(limits).difference(METRIC_KEYS))
        if missing or unknown:
            details = []
            if missing:
                details.append(f"missing {', '.join(missing)}")
            if unknown:
                details.append(f"unknown {', '.join(unknown)}")
            raise ValueError(f"scope `{scope_name}` limits: {'; '.join(details)}")
        for metric, limit in limits.items():
            if isinstance(limit, bool) or not isinstance(limit, int) or limit < 0:
                raise ValueError(
                    f"scope `{scope_name}` limit `{metric}` must be a non-negative integer"
                )
    return config


def build_source_report(
    graph: ManifestGraph, config: Mapping[str, Any]
) -> tuple[dict[str, Any], list[str]]:
    """Build the stable evidence report and all budget violations."""

    denied = frozenset(config["denied_required_packages"])
    violations: list[str] = []
    scope_reports: dict[str, Any] = {}
    for scope_name in sorted(config["scopes"]):
        scope = config["scopes"][scope_name]
        metrics, required, _declared = measure_scope(
            graph,
            roots=scope["roots"],
            include_root_dev_dependencies=scope["include_root_dev_dependencies"],
        )
        over_budget: dict[str, dict[str, int]] = {}
        for metric in METRIC_KEYS:
            limit = scope["limits"][metric]
            observed = metrics[metric]
            if observed > limit:
                over_budget[metric] = {"observed": observed, "limit": limit}
                violations.append(
                    f"{scope_name}: {metric} {observed} exceeds limit {limit}"
                )
        denied_present = sorted(denied.intersection(required.package_names))
        for package_name in denied_present:
            violations.append(
                f"{scope_name}: denied package `{package_name}` is required"
            )
        scope_reports[scope_name] = {
            "roots": scope["roots"],
            "include_root_dev_dependencies": scope[
                "include_root_dev_dependencies"
            ],
            "metrics": metrics,
            "limits": scope["limits"],
            "over_budget": over_budget,
            "denied_required_packages_present": denied_present,
            "within_budget": not over_budget and not denied_present,
        }

    baseline = config.get("baseline", {})
    baseline_fingerprint = (
        baseline.get("manifest_fingerprint") if isinstance(baseline, dict) else None
    )
    if (
        baseline_fingerprint is not None
        and baseline_fingerprint != graph.manifest_fingerprint
    ):
        violations.append(
            "manifest fingerprint differs from the reviewed dependency baseline: "
            f"{graph.manifest_fingerprint} != {baseline_fingerprint}"
        )
    report = {
        "schema_version": 1,
        "measurement_kind": MEASUREMENT_KIND,
        "manifest": graph.root_manifest.relative_to(graph.repository_root).as_posix(),
        "manifest_fingerprint": graph.manifest_fingerprint,
        "baseline_manifest_fingerprint": baseline_fingerprint,
        "fingerprint_matches_baseline": baseline_fingerprint
        == graph.manifest_fingerprint,
        "scopes": scope_reports,
        "within_budget": not violations,
    }
    return report, violations


def refreshed_config(
    config: Mapping[str, Any], graph: ManifestGraph, report: Mapping[str, Any]
) -> dict[str, Any]:
    """Return a deterministic exact no-growth baseline for current manifests."""

    refreshed = json.loads(json.dumps(config))
    for scope_name, scope_report in report["scopes"].items():
        refreshed["scopes"][scope_name]["limits"] = scope_report["metrics"]
    refreshed["baseline"] = {
        "manifest_fingerprint": graph.manifest_fingerprint,
        "refresh_command": (
            "python3 scripts/check_dependency_budget.py "
            "--config ci/dependency_budget.json --write-baseline"
        ),
    }
    return refreshed


def write_json(payload: Mapping[str, Any], target: Path) -> None:
    rendered = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if target == Path("-"):
        sys.stdout.write(rendered)
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(rendered, encoding="utf-8")


def write_source_human_report(
    report: Mapping[str, Any], stream: Any = sys.stdout
) -> None:
    print(f"measurement_kind={report['measurement_kind']}", file=stream)
    print(f"manifest_fingerprint={report['manifest_fingerprint']}", file=stream)
    for scope_name, scope in report["scopes"].items():
        print(f"scope={scope_name}", file=stream)
        for metric in METRIC_KEYS:
            print(
                f"  {metric}={scope['metrics'][metric]} "
                f"limit={scope['limits'][metric]}",
                file=stream,
            )


def cargo_metadata_command(args: argparse.Namespace) -> list[str]:
    command = [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--manifest-path",
        str(args.manifest_path),
    ]
    if not args.allow_lock_update:
        command.append("--locked")
    return command


def load_resolved_metadata(args: argparse.Namespace) -> dict[str, Any]:
    if args.metadata_json is not None:
        payload = json.loads(args.metadata_json.read_text(encoding="utf-8"))
    else:
        payload = json.loads(
            subprocess.check_output(cargo_metadata_command(args), text=True)
        )
    if not isinstance(payload, dict):
        raise ValueError("Cargo metadata must be a JSON object")
    return payload


def package_source(package: Mapping[str, Any]) -> str:
    source = package.get("source")
    if source is None:
        return "path"
    if isinstance(source, str) and source.startswith("registry+"):
        return "registry"
    if isinstance(source, str) and source.startswith("git+"):
        return "git"
    return "other"


def resolved_package_ids(
    metadata: Mapping[str, Any],
    *,
    root_names: Sequence[str],
    workspace: bool,
) -> frozenset[str]:
    """Return the resolved transitive closure for selected package roots."""

    packages = metadata.get("packages")
    resolve = metadata.get("resolve")
    if not isinstance(packages, list) or not isinstance(resolve, dict):
        raise ValueError("Cargo metadata must contain packages and resolve")
    nodes = resolve.get("nodes")
    if not isinstance(nodes, list):
        raise ValueError("Cargo metadata resolve must contain nodes")
    node_map = {node["id"]: node for node in nodes}
    if workspace:
        roots = metadata.get("workspace_members")
        if not isinstance(roots, list) or not all(isinstance(row, str) for row in roots):
            raise ValueError("Cargo metadata workspace_members must be strings")
    elif root_names:
        by_name: dict[str, list[str]] = {}
        for package in packages:
            by_name.setdefault(package["name"], []).append(package["id"])
        roots = []
        for name in root_names:
            matches = by_name.get(name, [])
            if len(matches) != 1:
                raise ValueError(
                    f"resolved root `{name}` matched {len(matches)} packages"
                )
            roots.append(matches[0])
    else:
        roots = list(node_map)

    closure: set[str] = set()
    queue = deque(roots)
    while queue:
        package_id = queue.popleft()
        if package_id in closure:
            continue
        if package_id not in node_map:
            raise ValueError(f"resolved root or dependency `{package_id}` has no node")
        closure.add(package_id)
        node = node_map[package_id]
        dependencies = node.get("deps")
        if isinstance(dependencies, list):
            queue.extend(dependency["pkg"] for dependency in dependencies)
        else:
            legacy = node.get("dependencies", [])
            if not isinstance(legacy, list):
                raise ValueError(f"resolved node `{package_id}` dependencies are invalid")
            queue.extend(legacy)
    return frozenset(closure)


def resolved_report(
    metadata: Mapping[str, Any],
    *,
    root_names: Sequence[str],
    workspace: bool,
    watched: Iterable[str],
) -> dict[str, Any]:
    package_ids = resolved_package_ids(
        metadata, root_names=root_names, workspace=workspace
    )
    packages = {
        package["id"]: package
        for package in metadata["packages"]
        if package["id"] in package_ids
    }
    sources = Counter(package_source(package) for package in packages.values())
    versions: dict[str, set[str]] = {name: set() for name in watched}
    for package in packages.values():
        if package["name"] in versions:
            versions[package["name"]].add(package["version"])
    return {
        "schema_version": 1,
        "measurement_kind": "cargo-metadata-resolved-graph-v1",
        "roots": sorted(root_names),
        "workspace_roots": workspace,
        "total_packages": len(packages),
        "package_sources": {
            "registry": sources["registry"],
            "path": sources["path"],
            "git": sources["git"],
            "other": sources["other"],
        },
        "watched_packages": {
            name: sorted(found_versions) for name, found_versions in sorted(versions.items())
        },
    }


def resolved_mode_requested(args: argparse.Namespace) -> bool:
    return bool(
        args.resolved
        or args.metadata_json is not None
        or args.package
        or args.workspace
        or args.max_registry_packages is not None
        or args.max_total_packages is not None
        or args.watch
        or args.deny
        or args.allow_lock_update
    )


def run_resolved_mode(args: argparse.Namespace) -> int:
    if args.write_baseline:
        print("ERROR: --write-baseline is only valid for the source ratchet", file=sys.stderr)
        return 2
    try:
        metadata = load_resolved_metadata(args)
        denied = sorted(set(DEFAULT_DENIED_PACKAGES).union(args.deny))
        watched = sorted(set(DEFAULT_WATCHED_PACKAGES).union(args.watch).union(denied))
        report = resolved_report(
            metadata,
            root_names=args.package,
            workspace=args.workspace,
            watched=watched,
        )
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"ERROR: failed to inspect Cargo metadata: {error}", file=sys.stderr)
        return 2
    except subprocess.CalledProcessError as error:
        return error.returncode

    human_stream = sys.stderr if args.json_out == Path("-") else sys.stdout
    print(f"total_packages={report['total_packages']}", file=human_stream)
    for source in ("registry", "path", "git", "other"):
        print(
            f"{source}_packages={report['package_sources'][source]}",
            file=human_stream,
        )
    print("watched_packages:", file=human_stream)
    for name, versions in report["watched_packages"].items():
        print(
            f"  {name}: {', '.join(versions) if versions else '-'}",
            file=human_stream,
        )

    violations = []
    for name in denied:
        versions = report["watched_packages"][name]
        if versions:
            violations.append(f"denied package present: {name} ({', '.join(versions)})")
    registry_packages = report["package_sources"]["registry"]
    if (
        args.max_registry_packages is not None
        and registry_packages > args.max_registry_packages
    ):
        violations.append(
            "registry package budget exceeded: "
            f"{registry_packages} > {args.max_registry_packages}"
        )
    if (
        args.max_total_packages is not None
        and report["total_packages"] > args.max_total_packages
    ):
        violations.append(
            "total package budget exceeded: "
            f"{report['total_packages']} > {args.max_total_packages}"
        )
    for violation in violations:
        print(violation, file=sys.stderr)
    if args.json_out is not None:
        write_json(report, args.json_out)
    return 1 if violations else 0


def run_source_mode(args: argparse.Namespace) -> int:
    try:
        config = load_budget_config(args.config)
        graph = load_manifest_graph(args.manifest_path)
        report, violations = build_source_report(graph, config)
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"ERROR: failed to enforce dependency budget: {error}", file=sys.stderr)
        return 2

    if args.write_baseline:
        denied_violations = [
            violation for violation in violations if "denied package" in violation
        ]
        if denied_violations:
            print(
                "ERROR: refusing to refresh a baseline with denied required packages",
                file=sys.stderr,
            )
            return 1
        refreshed = refreshed_config(config, graph, report)
        try:
            write_json(refreshed, args.config)
        except OSError as error:
            print(f"ERROR: failed to write dependency baseline: {error}", file=sys.stderr)
            return 2
        report, violations = build_source_report(graph, refreshed)

    human_stream = sys.stderr if args.json_out == Path("-") else sys.stdout
    write_source_human_report(report, human_stream)
    for violation in violations:
        print(f"ERROR: {violation}", file=sys.stderr)

    if args.json_out is not None:
        try:
            write_json(report, args.json_out)
        except OSError as error:
            print(f"ERROR: failed to write dependency report: {error}", file=sys.stderr)
            return 2
    return 1 if violations else 0


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if args.max_registry_packages is not None and args.max_registry_packages < 0:
        print("ERROR: --max-registry-packages must be non-negative", file=sys.stderr)
        return 2
    if args.max_total_packages is not None and args.max_total_packages < 0:
        print("ERROR: --max-total-packages must be non-negative", file=sys.stderr)
        return 2
    if resolved_mode_requested(args):
        return run_resolved_mode(args)
    return run_source_mode(args)


if __name__ == "__main__":
    raise SystemExit(main())
