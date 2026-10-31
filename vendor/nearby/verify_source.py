#!/usr/bin/env python3
"""Check reviewed Nearby upstream files and explicit local patches offline."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath


METADATA_SHA256 = "d3b6bff01d5c363ae9c4bacfaa1faadbe886eb67283bf03a42f822301a3d252c"
MANIFEST_SHA256 = "4cc45752bfac4984b1a1adf1cb198a5a66af409247eee51bdc20a64857db7985"
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
    expected_files = metadata["files"]
    for target in metadata["targets"]:
        for source in target["sources"]:
            name = (PurePosixPath(target["path"]) / relative_path(source)).as_posix()
            if name not in expected_files:
                raise ValueError(f"compile source missing from upstream census: {name}")
    for name, upstream in expected_files.items():
        expected = upstream.get("local_patch", upstream)
        file = root / relative_path(name)
        if file.is_symlink():
            raise ValueError(f"linked source file: {name}")
        data = file.read_bytes()
        blob = b"blob " + str(len(data)).encode("ascii") + b"\0" + data
        if (
            hashlib.sha256(data).hexdigest() != expected["sha256"]
            or hashlib.sha1(blob).hexdigest() != expected["git_blob"]
        ):
            raise ValueError(f"reviewed source file changed: {name}")
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
        parser.exit(1, f"Nearby source verification failed: {error}\n")
    print(f"Nearby source verification passed: {count} reviewed files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
