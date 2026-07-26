#!/usr/bin/env python3
"""Write a closed canonical SHA256SUMS inventory from direct artifact bytes."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_relative_path,
    exclusive_write_bytes,
    scan_inventory_paths,
    stable_hash_relative,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts-dir", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--file", action="append", required=True)
    args = parser.parse_args()
    try:
        root = Path(args.artifacts_dir)
        output = Path(args.output)
        if output.name != "SHA256SUMS" or output.parent.absolute() != root.absolute():
            raise ReleaseArtifactError(
                "aggregate checksum output must be <artifacts-dir>/SHA256SUMS"
            )
        files = [canonical_relative_path(value) for value in args.file]
        if len(files) != len(set(files)):
            raise ReleaseArtifactError(
                "aggregate checksum inventory contains duplicate paths"
            )
        files.sort()
        if scan_inventory_paths(root) != files:
            raise ReleaseArtifactError(
                "aggregate checksum inventory does not exactly match artifact root"
            )
        captured = {
            relative: stable_hash_relative(root, relative) for relative in files
        }
        if scan_inventory_paths(root) != files:
            raise ReleaseArtifactError(
                "artifact root changed while aggregate checksums were computed"
            )
        for relative, before in captured.items():
            if stable_hash_relative(root, relative) != before:
                raise ReleaseArtifactError(
                    f"artifact {relative!r} changed while checksums were computed"
                )
        payload = "".join(
            f"{captured[relative].sha256}  {relative}\n"
            for relative in files
        ).encode("ascii")
        exclusive_write_bytes(output, payload, mode=0o644)
    except ReleaseArtifactError as exc:
        print(f"release checksum inventory error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
