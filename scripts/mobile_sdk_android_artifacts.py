#!/usr/bin/env python3
"""Resolve canonical Kotlin build outputs and collect their exact SDK SBOMs.

Python 3.12, standard library only. An explicitly configured external artifact
root never falls back to source-tree output. Collection creates a new directory;
it does not sign, publish, execute Gradle, or overwrite an earlier inventory.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import stat
import sys


SDK_MODULES = ("core-jvm", "client-android", "kagemusha-wallet-android")


def canonical_directory(value: str, label: str) -> Path:
    path = Path(value)
    if not value or not path.is_absolute() or any(c in value for c in "\n\r\0"):
        raise ValueError(f"{label} must be an absolute non-empty path")
    if str(path) != value or path.resolve(strict=True) != path or not path.is_dir():
        raise ValueError(f"{label} must be canonical and must not traverse symbolic links")
    return path


def build_root(repository: Path, external: str | None) -> Path:
    repository = canonical_directory(str(repository), "repository")
    if external is None:
        return repository / "kotlin"
    artifacts = canonical_directory(external, "MOBILE_SDK_ANDROID_ARTIFACT_DIR")
    if artifacts == repository or artifacts.is_relative_to(repository):
        raise ValueError("Android artifacts must be outside the reviewed source tree")
    return artifacts / "gradle-build/iroha_kotlin_sdk"


def module_build(root: Path, module: str, external: str | None) -> Path:
    if module not in SDK_MODULES:
        raise ValueError("not a canonical published Kotlin SDK module")
    return root / module if external is not None else root / module / "build"


def regular_file(path: Path) -> Path:
    if any(c in str(path) for c in "\n\r\0"):
        raise ValueError("artifact paths must not contain line separators")
    if path.resolve(strict=True) != path:
        raise ValueError(f"artifact must not traverse symbolic links: {path}")
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or metadata.st_size == 0:
        raise ValueError(f"artifact must be a non-empty single-link regular file: {path}")
    return path


def built_artifacts(repository: Path, external: str | None) -> tuple[Path, Path]:
    root = build_root(repository, external)
    libraries = module_build(root, "core-jvm", external) / "libs"
    jars = sorted(p for p in libraries.glob("core-jvm-*.jar")
                  if not p.name.endswith(("-sources.jar", "-javadoc.jar")))
    if len(jars) != 1:
        raise ValueError("exactly one canonical core-jvm runtime JAR is required")
    jar = regular_file(jars[0])
    aar = regular_file(module_build(root, "client-android", external)
                       / "outputs/aar/client-android-release.aar")
    return jar, aar


def collect_sboms(repository: Path, external: str | None, destination: Path,
                  version: str | None = None) -> None:
    root = build_root(repository, external)
    parent = canonical_directory(str(destination.parent), "SBOM output parent")
    if destination.name in ("", ".", "..") or destination != parent / destination.name:
        raise ValueError("SBOM destination must have one canonical directory name")
    if destination == repository or destination.is_relative_to(repository):
        raise ValueError("SBOM output must be outside the reviewed source tree")
    documents = {}
    for module in SDK_MODULES:
        path = regular_file(module_build(root, module, external) / "reports/bom/bom.json")
        if path.stat().st_size > 32 * 1024 * 1024:
            raise ValueError("SDK SBOM exceeds the 32 MiB bound")
        content = path.read_bytes()
        document = json.loads(content)
        component = document.get("metadata", {}).get("component", {})
        if document.get("bomFormat") != "CycloneDX" or not isinstance(document.get("components"), list):
            raise ValueError(f"missing CycloneDX component inventory for {module}")
        if component.get("group") != "org.hyperledger.iroha.sdk" or component.get("name") != module:
            raise ValueError(f"SBOM is not owned by canonical Kotlin module {module}")
        if not isinstance(component.get("version"), str) or not component["version"]:
            raise ValueError(f"SBOM module version is missing for {module}")
        if version is not None and component["version"] != version:
            raise ValueError(f"SBOM version disagrees with requested SDK version for {module}")
        documents[f"iroha-{module}.cyclonedx.json"] = content
    # Validate the complete inventory before creating any output. A prior
    # generation, symlink, or competing creator must never be overwritten.
    destination.mkdir(mode=0o700)
    for filename, content in documents.items():
        with (destination / filename).open("xb") as output:
            output.write(content)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--collect-sboms", type=Path)
    parser.add_argument("--print-build-root", action="store_true")
    parser.add_argument("--version")
    parser.add_argument("--artifact-dir", default=os.environ.get("MOBILE_SDK_ANDROID_ARTIFACT_DIR"))
    args = parser.parse_args()
    try:
        external = args.artifact_dir
        if args.print_build_root:
            print(build_root(args.root, external))
        elif args.collect_sboms is not None:
            collect_sboms(args.root, external, args.collect_sboms, args.version)
        else:
            for path in built_artifacts(args.root, external):
                print(path)
        return 0
    except (OSError, ValueError, TypeError, AttributeError) as error:
        print(f"[mobile-sdk-android] ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
