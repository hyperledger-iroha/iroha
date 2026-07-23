#!/usr/bin/env python3
"""Compute and verify source seals for production NoritoBridge artifacts.

The seal follows the transitive local-package dependency closure of
``connect_norito_bridge`` for every packaged target on the selected mobile
platform.  This keeps an artifact bound to every source file that can affect it
without making builds depend on unrelated workspace tools such as Kagami or
test-network helpers.
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
ANDROID_TARGETS = (
    "aarch64-linux-android",
    "x86_64-linux-android",
)
COMMON_ROOT_INPUTS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "rust-toolchain",
    ".cargo",
    "vendor",
    "codec",
    "scripts/check_mobile_sdk_artifacts.sh",
    "scripts/norito_bridge_source_seal.py",
)
APPLE_ROOT_INPUTS = (
    "IrohaSwift/Package.swift",
    "IrohaSwift/Package.resolved",
    "IrohaSwift/Sources/IrohaSwift",
    "IrohaSwift/Sources/IrohaSwiftMobileTransports",
    "scripts/build_norito_xcframework.sh",
)
ANDROID_ROOT_INPUTS = (
    "kotlin/client-android/build.gradle.kts",
    "kotlin/settings.gradle.kts",
    "kotlin/build.gradle.kts",
    "kotlin/gradle.properties",
    "kotlin/gradle/libs.versions.toml",
    "kotlin/gradle/wrapper/gradle-wrapper.properties",
    "kotlin/gradlew",
    "scripts/package_mobile_sdk_artifacts.sh",
)
PLATFORM_TARGETS = {
    "apple": APPLE_TARGETS,
    "android": ANDROID_TARGETS,
}
PLATFORM_ROOT_INPUTS = {
    "apple": APPLE_ROOT_INPUTS,
    "android": ANDROID_ROOT_INPUTS,
}
# Kept as a public union for callers/tests which construct their own input set.
ROOT_INPUTS = tuple(
    dict.fromkeys(COMMON_ROOT_INPUTS + APPLE_ROOT_INPUTS + ANDROID_ROOT_INPUTS)
)
SNAPSHOT_SCHEMA = "iroha.norito-bridge-source-seal.v1"


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


def local_dependency_roots(
    root: pathlib.Path, targets: Iterable[str] = APPLE_TARGETS
) -> set[str]:
    package_roots: set[pathlib.Path] = set()
    for target in targets:
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


def seal_inputs(root: pathlib.Path, platform: str = "apple") -> list[str]:
    try:
        targets = PLATFORM_TARGETS[platform]
        platform_inputs = PLATFORM_ROOT_INPUTS[platform]
    except KeyError as error:
        raise RuntimeError(f"unsupported source-seal platform: {platform}") from error
    candidates = set(COMMON_ROOT_INPUTS)
    candidates.update(platform_inputs)
    candidates.update(local_dependency_roots(root, targets))
    existing = [value for value in candidates if (root / value).exists()]
    return sorted(existing)


def listed_files(root: pathlib.Path, inputs: Iterable[str]) -> list[str]:
    input_set = set(inputs)
    output = run(
        root,
        [
            "git",
            "ls-files",
            "-z",
            "-co",
            "--exclude-standard",
            "--",
            *inputs,
        ],
    )
    listed = {
        value.decode("utf-8")
        for value in output.split(b"\0")
        if value
    }

    # Some workspace-wide build inputs are intentionally ignored by repository
    # policy (notably the root Cargo.lock). They still affect the bridge binary,
    # so an explicit ROOT_INPUT must be sealed even when `git ls-files -co
    # --exclude-standard` omits it. Do not recursively include arbitrary ignored
    # files below directory inputs: build outputs and local corpora remain outside
    # the production source seal unless named explicitly above.
    for relative in ROOT_INPUTS:
        if relative not in input_set:
            continue
        path = root / relative
        if path.is_symlink():
            raise RuntimeError(f"explicit source-seal input is symlinked: {relative}")
        if path.is_file():
            listed.add(relative)

    return sorted(listed)


def fingerprint(root: pathlib.Path, inputs: list[str]) -> str:
    digest = hashlib.sha256()
    for relative in listed_files(root, inputs):
        path = root / relative
        if path.is_symlink():
            raise RuntimeError(f"source-seal input is symlinked: {relative}")
        if not path.is_file():
            raise RuntimeError(f"source-seal input is not a regular file: {relative}")
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


def source_commit(root: pathlib.Path) -> str:
    value = run(root, ["git", "rev-parse", "--verify", "HEAD"]).decode("ascii").strip()
    if len(value) != 40 or any(character not in "0123456789abcdef" for character in value):
        raise RuntimeError("source commit is not a canonical lowercase Git SHA-1")
    return value


def snapshot(root: pathlib.Path, platform: str) -> dict[str, object]:
    """Return the canonical source state consumed by one platform build."""

    inputs = seal_inputs(root, platform)
    source_status = status(root, inputs)
    return {
        "schema": SNAPSHOT_SCHEMA,
        "platform": platform,
        "targets": list(PLATFORM_TARGETS[platform]),
        "source_commit": source_commit(root),
        "source_tree_dirty": bool(source_status),
        "source_status": source_status,
        "source_fingerprint_sha256": fingerprint(root, inputs),
    }


def snapshot_bytes(root: pathlib.Path, platform: str) -> bytes:
    return (
        json.dumps(snapshot(root, platform), sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def verify_snapshot(root: pathlib.Path, platform: str, snapshot_path: pathlib.Path) -> None:
    """Reject a missing, tampered, stale, or mixed-source build snapshot."""

    expected = snapshot_path.read_bytes()
    current = snapshot_bytes(root, platform)
    if expected != current:
        raise RuntimeError(
            f"{platform} NoritoBridge source changed after the build started"
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "mode", choices=("fingerprint", "paths", "snapshot", "status", "verify")
    )
    parser.add_argument("--root", type=pathlib.Path, required=True)
    parser.add_argument(
        "--platform", choices=tuple(PLATFORM_TARGETS), default="apple"
    )
    parser.add_argument(
        "--snapshot",
        type=pathlib.Path,
        help="Build-start snapshot to authenticate in verify mode.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    inputs = seal_inputs(root, args.platform)
    if args.mode == "fingerprint":
        print(fingerprint(root, inputs))
    elif args.mode == "paths":
        print("\n".join(inputs))
    elif args.mode == "status":
        value = status(root, inputs)
        if value:
            print(value)
    elif args.mode == "snapshot":
        sys.stdout.buffer.write(snapshot_bytes(root, args.platform))
    else:
        if args.snapshot is None:
            raise RuntimeError("verify mode requires --snapshot")
        verify_snapshot(root, args.platform, args.snapshot.resolve())
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
        print(f"norito bridge source seal failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
