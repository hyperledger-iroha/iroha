#!/usr/bin/env python3
"""Compute the source seal for production NoritoBridge Apple artifacts.

The seal follows the transitive local-package dependency closure of
``connect_norito_bridge`` for every packaged Apple target.  This keeps the
artifact bound to every source file that can affect it without making builds
depend on unrelated workspace tools such as Kagami or test-network helpers.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys
from collections.abc import Iterable


APPLE_TARGETS = (
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "x86_64-apple-ios",
    "aarch64-apple-darwin",
)
ROOT_INPUTS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "rust-toolchain",
    ".cargo",
    "vendor",
    "codec",
    "IrohaSwift/Package.swift",
    "IrohaSwift/Sources/IrohaSwift",
    "scripts/build_norito_xcframework.sh",
    "scripts/check_mobile_sdk_artifacts.sh",
    "scripts/norito_bridge_source_seal.py",
)


def run(root: pathlib.Path, args: list[str]) -> bytes:
    return subprocess.run(
        args,
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout


def metadata(root: pathlib.Path, target: str) -> dict[str, object]:
    output = run(
        root,
        [
            "cargo",
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--features",
            "connect_norito_bridge/privacy-production-enabled",
            "--filter-platform",
            target,
        ],
    )
    return json.loads(output)


def local_dependency_roots(root: pathlib.Path) -> set[str]:
    package_roots: set[pathlib.Path] = set()
    for target in APPLE_TARGETS:
        document = metadata(root, target)
        packages = {
            package["id"]: package
            for package in document["packages"]
            if isinstance(package, dict)
        }
        resolve = document.get("resolve")
        if not isinstance(resolve, dict):
            raise RuntimeError("cargo metadata did not return a resolve graph")
        nodes = {
            node["id"]: node
            for node in resolve.get("nodes", [])
            if isinstance(node, dict)
        }
        roots = [
            package_id
            for package_id, package in packages.items()
            if package.get("name") == "connect_norito_bridge"
            and pathlib.Path(str(package["manifest_path"])).resolve()
            == (root / "crates/connect_norito_bridge/Cargo.toml").resolve()
        ]
        if len(roots) != 1:
            raise RuntimeError(
                f"expected one connect_norito_bridge package for {target}, found {len(roots)}"
            )

        pending = roots
        visited: set[str] = set()
        while pending:
            package_id = pending.pop()
            if package_id in visited:
                continue
            visited.add(package_id)
            node = nodes.get(package_id)
            if node is None:
                raise RuntimeError(f"missing resolve node for {package_id}")
            for dependency in node.get("deps", []):
                if isinstance(dependency, dict) and isinstance(dependency.get("pkg"), str):
                    pending.append(dependency["pkg"])

        for package_id in visited:
            package = packages.get(package_id)
            if package is None:
                continue
            manifest = pathlib.Path(str(package["manifest_path"])).resolve()
            try:
                relative = manifest.parent.relative_to(root)
            except ValueError:
                continue
            package_roots.add(relative)

    return {path.as_posix() for path in package_roots}


def seal_inputs(root: pathlib.Path) -> list[str]:
    candidates = set(ROOT_INPUTS)
    candidates.update(local_dependency_roots(root))
    existing = [value for value in candidates if (root / value).exists()]
    return sorted(existing)


def listed_files(root: pathlib.Path, inputs: Iterable[str]) -> list[str]:
    output = run(
        root,
        [
            "git",
            "ls-files",
            "-co",
            "--exclude-standard",
            "--",
            *inputs,
        ],
    )
    return sorted(set(output.decode("utf-8").splitlines()))


def fingerprint(root: pathlib.Path, inputs: list[str]) -> str:
    digest = hashlib.sha256()
    for relative in listed_files(root, inputs):
        path = root / relative
        if not path.is_file() or path.is_symlink():
            continue
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def status(root: pathlib.Path, inputs: list[str]) -> str:
    output = run(
        root,
        [
            "git",
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            *inputs,
        ],
    )
    return output.decode("utf-8").rstrip("\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("fingerprint", "paths", "status"))
    parser.add_argument("--root", type=pathlib.Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    inputs = seal_inputs(root)
    if args.mode == "fingerprint":
        print(fingerprint(root, inputs))
    elif args.mode == "paths":
        print("\n".join(inputs))
    else:
        value = status(root, inputs)
        if value:
            print(value)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
        print(f"norito bridge source seal failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
