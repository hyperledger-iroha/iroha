#!/usr/bin/env python3
"""Compute a deterministic SHA-256 manifest of checkout source files.

The manifest covers tracked and untracked, non-ignored paths reported by Git,
including deletions, symlink targets, and executable bits. Build artifacts and
other ignored files are deliberately excluded.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import stat
import struct
import subprocess
from typing import Iterable


_DOMAIN = b"iroha-workspace-source-manifest-v1\0"


def _git_source_paths(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    return sorted(
        {
            os.fsdecode(raw)
            for raw in result.stdout.split(b"\0")
            if raw
        },
        key=os.fsencode,
    )


def _frame(hasher: "hashlib._Hash", payload: bytes) -> None:
    hasher.update(struct.pack(">Q", len(payload)))
    hasher.update(payload)


def _manifest_for_paths(root: Path, paths: Iterable[str]) -> str:
    hasher = hashlib.sha256(_DOMAIN)
    for relative in sorted(set(paths), key=os.fsencode):
        encoded_path = os.fsencode(relative)
        _frame(hasher, encoded_path)
        path = root / relative
        try:
            metadata = path.lstat()
        except FileNotFoundError:
            hasher.update(b"D")
            continue

        hasher.update(struct.pack(">I", stat.S_IMODE(metadata.st_mode)))
        if stat.S_ISLNK(metadata.st_mode):
            hasher.update(b"L")
            _frame(hasher, os.fsencode(os.readlink(path)))
        elif stat.S_ISREG(metadata.st_mode):
            hasher.update(b"F")
            hasher.update(struct.pack(">Q", metadata.st_size))
            with path.open("rb") as source:
                while chunk := source.read(1024 * 1024):
                    hasher.update(chunk)
        elif stat.S_ISDIR(metadata.st_mode):
            # Gitlinks/submodules appear as directory entries in the parent.
            hasher.update(b"G")
        else:
            hasher.update(b"O")
    return hasher.hexdigest()


def workspace_source_manifest(root: Path) -> str:
    """Return the checkout source manifest rooted at ``root``."""

    root = root.resolve()
    return _manifest_for_paths(root, _git_source_paths(root))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Git checkout root (defaults to this repository)",
    )
    args = parser.parse_args()
    print(workspace_source_manifest(args.root))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
