#!/usr/bin/env python3
"""Check the exact reviewed BoringSSL SwiftPM source closure without network access."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath


METADATA_SHA256 = "fc65a265149b30dfa4f58e73c8ea0ebe45960820d8d5ebd8f368a9475fbdf6a3"
MANIFEST_SHA256 = "1225bebd04024daf79062f7a012ec0094c97d4d26d6946222208f9745b11cebf"
OWNED_FILES = {"Package.swift", "UPSTREAM.json", "IROHA-PATCH.md", "verify_source.py"}
LOCAL_BUILD_DIRECTORIES = {".build", ".swiftpm", "__pycache__"}


def relative_path(value: str) -> str:
    """Accept only normalized repository-relative POSIX file paths."""
    path = PurePosixPath(value)
    if (
        not value
        or "\\" in value
        or "\x00" in value
        or path.is_absolute()
        or (path.parts and ":" in path.parts[0])
        or ".." in path.parts
        or path.as_posix() != value
        or value == "."
    ):
        raise ValueError(f"invalid source path: {value!r}")
    return value


def validate(root: Path) -> int:
    """Reject source drift, missing files, links, and unexpected package inputs."""
    root = root.resolve()
    for name, expected in [("UPSTREAM.json", METADATA_SHA256), ("Package.swift", MANIFEST_SHA256)]:
        file = root / name
        if file.is_symlink() or hashlib.sha256(file.read_bytes()).hexdigest() != expected:
            raise ValueError(f"reviewed package metadata changed: {name}")
    metadata = json.loads((root / "UPSTREAM.json").read_text())
    groups = json.loads((root / "gen/sources.json").read_text())
    expected_sources = {"gen/sources.json", "LICENSE", "third_party/fiat/LICENSE"}
    for group in metadata["source_groups"]:
        for field in metadata["source_fields"]:
            expected_sources.update(relative_path(name) for name in groups[group].get(field, []))
    if set(metadata["files"]) != expected_sources:
        raise ValueError("upstream source census differs from generated build metadata")
    expected_files = dict(metadata["files"])
    expected_files.update(metadata["packaging_reference"]["files"])
    for name, expected in expected_files.items():
        file = root / relative_path(name)
        if file.is_symlink() or hashlib.sha256(file.read_bytes()).hexdigest() != expected:
            raise ValueError(f"upstream file changed: {name}")
    actual_files: set[str] = set()
    for directory, children, filenames in os.walk(root, followlinks=False):
        directory = Path(directory)
        for child in children:
            if (directory / child).is_symlink():
                raise ValueError(f"linked directory: {(directory / child).relative_to(root)}")
        if directory == root:
            children[:] = [child for child in children if child not in LOCAL_BUILD_DIRECTORIES]
        for filename in filenames:
            file = directory / filename
            if file.is_symlink():
                raise ValueError(f"linked file: {file.relative_to(root)}")
            actual_files.add(file.relative_to(root).as_posix())
    if actual_files != set(expected_files) | OWNED_FILES:
        raise ValueError("package file census differs from reviewed source and packaging inputs")
    return len(metadata["files"])


def main() -> int:
    """Validate this package or an explicitly selected qualification copy."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    try:
        count = validate(args.root)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"BoringSSL source verification failed: {error}\n")
    print(f"BoringSSL source verification passed: {count} exact upstream files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
