#!/usr/bin/env python3
"""Admit only coherent additive dependency edges between existing local packages.

Python 3.11+, Git and Cargo are required. --base identifies the reviewed comparison
commit. This guard reads the current working tree (including staged edits) and
runs locked, offline Cargo metadata without compiling anything. It never edits
manifests, regenerates the lockfile, fetches packages or grants external dependency
approval. Every other lockfile change remains refused by the calling guard.
"""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tomllib


class EdgeError(ValueError):
    """The candidate is outside the existing-local-edge exception."""


def require(condition, message):
    if not condition:
        raise EdgeError(message)


def packages(lock):
    result = {}
    for package in lock.get("package", []):
        key = (package["name"], package["version"], package.get("source"))
        require(key not in result, "lockfile repeats a package identity")
        result[key] = package
    return result


def lock_edges(before, after):
    """Reject every lock delta except added names of already-locked local packages."""
    require({k: v for k, v in before.items() if k != "package"}
            == {k: v for k, v in after.items() if k != "package"}, "lockfile header or metadata changed")
    old, new = packages(before), packages(after)
    require(old.keys() == new.keys(), "lockfile package set, version or source changed")
    local = {key[0]: key for key in old if key[2] is None}
    require(len(local) == sum(key[2] is None for key in old), "local package names are ambiguous")
    edges = set()
    for key in old:
        a, b = old[key], new[key]
        require({k: v for k, v in a.items() if k != "dependencies"}
                == {k: v for k, v in b.items() if k != "dependencies"},
                f"lockfile package fields/checksum changed: {key[0]}")
        previous, current = a.get("dependencies", []), b.get("dependencies", [])
        require(len(previous) == len(set(previous)) and len(current) == len(set(current)), "duplicate lock dependency")
        require(set(previous) <= set(current), f"lock dependency removed: {key[0]}")
        added = set(current) - set(previous)
        if added:
            require(key[2] is None and "checksum" not in b, "external package dependency list changed")
        for target in added:
            require(target in local and target != key[0], f"added edge is not an unambiguous existing local package: {target}")
            require(sum(other[0] == target for other in old) == 1, "added local dependency name is ambiguous")
            edges.add((key[0], target))
    require(edges, "lockfile change contains no eligible additive local edges")
    return edges, local


def validate(before, after, manifests, metadata, root):
    """Match each allowed lock edge to its exact new path declaration and Cargo view."""
    edges, local = lock_edges(before, after)
    members = set(metadata["workspace_members"])
    local_packages = {p["name"]: p for p in metadata["packages"]
                      if p["id"] in members and p.get("source") is None}
    # no-deps metadata describes declarations without resolving the full graph.
    # Check cycles through normal/build edges; dev edges may legitimately cycle.
    graph = {name: {dep["name"] for dep in package["dependencies"]
                    if dep.get("kind") in (None, "build") and dep.get("path") and dep["name"] in local_packages}
             for name, package in local_packages.items()}
    for source, target in edges:
        pending, visited = [target], set()
        while pending:
            selected = pending.pop()
            require(selected != source, "added normal dependency creates a local package cycle")
            if selected not in visited:
                visited.add(selected)
                pending.extend(graph.get(selected, set()))
    declarations = set()
    allowed = {}
    for relative, (old, current) in manifests.items():
        previous = old.get("dependencies", {})
        actual = current.get("dependencies", {})
        added = set(actual) - set(previous)
        if not added:
            continue
        # This narrow exception cannot authorize unrelated manifest policy changes.
        remaining = copy.deepcopy(current)
        for name in added:
            del remaining["dependencies"][name]
        if not previous and "dependencies" not in old:
            remaining.pop("dependencies", None)
        require(remaining == old, f"local-edge manifest has additional changes: {relative}")
        source_name = old.get("package", {}).get("name")
        require(source_name in local_packages, "edge source is not an existing workspace package")
        source = local_packages[source_name]
        require(Path(source["manifest_path"]).resolve() == (root / relative).resolve(), "source manifest disagrees with Cargo metadata")
        require(source["version"] == local[source_name][1], "source package version disagrees with lockfile")
        for name in added:
            entry = actual[name]
            require(isinstance(entry, dict) and set(entry) == {"path"}
                    and isinstance(entry["path"], str), "only a plain local path dependency is eligible; aliases/features are not")
            require(not PurePosixPath(entry["path"]).is_absolute(), "local path must remain within the workspace")
            target_path = ((root / relative).parent / entry["path"] / "Cargo.toml").resolve()
            require(target_path.is_relative_to(root), "local dependency path escapes the workspace")
            require((source_name, name) in edges and name in local_packages,
                    "new declaration lacks its matching existing local lock edge")
            target = local_packages[name]
            require(Path(target["manifest_path"]).resolve() == target_path
                    and target["version"] == local[name][1], "target path/version disagrees with Cargo metadata")
            target_relative = target_path.relative_to(root).as_posix()
            require(target_relative in manifests and manifests[target_relative][0].get("package", {}).get("name") == name,
                    "target package did not exist at the comparison base")
            dependencies = [dep for dep in source["dependencies"] if dep["name"] == name]
            require(len(dependencies) == 1, "Cargo metadata does not have one matching direct dependency")
            dependency = dependencies[0]
            require(dependency.get("source") is None and dependency.get("kind") is None
                    and dependency.get("target") is None and dependency.get("rename") is None
                    and dependency.get("optional") is False and dependency.get("features") == []
                    and dependency.get("uses_default_features") is True
                    and Path(dependency.get("path", "")).resolve() == target_path.parent,
                    "Cargo metadata dependency differs from the plain local edge")
            declarations.add((source_name, name))
            allowed.setdefault(relative, []).append(name)
    require(declarations == edges, "lockfile additions do not exactly match newly declared local edges")
    return {path: sorted(names) for path, names in sorted(allowed.items())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    args = parser.parse_args()
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip()).resolve()
    git = lambda *parts: subprocess.check_output(["git", "-C", str(root), *parts])
    old_raw = git("show", f"{args.base}:Cargo.lock")
    current_raw = (root / "Cargo.lock").read_bytes()
    if old_raw == current_raw:
        print("{}")
        return 0
    before, after = tomllib.loads(old_raw.decode()), tomllib.loads(current_raw.decode())
    lock_edges(before, after)  # Refuse external changes before invoking Cargo.
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"], cwd=root))
    manifests = {}
    members = set(metadata["workspace_members"])
    for package in metadata["packages"]:
        if package["id"] not in members or package.get("source") is not None:
            continue
        path = Path(package["manifest_path"]).resolve()
        require(path.is_relative_to(root), "workspace package escapes the repository")
        relative = path.relative_to(root).as_posix()
        base = tomllib.loads(git("show", f"{args.base}:{relative}").decode())
        manifests[relative] = (base, tomllib.loads(path.read_text()))
    result = validate(before, after, manifests, metadata, root)
    require((root / "Cargo.lock").read_bytes() == current_raw, "lockfile changed during validation")
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (EdgeError, KeyError, TypeError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"local dependency edge guard refused: {error}", file=sys.stderr)
        raise SystemExit(1)
