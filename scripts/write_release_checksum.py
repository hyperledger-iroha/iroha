#!/usr/bin/env python3
"""Write one canonical SHA-256 sidecar without replacing an existing path."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_relative_path,
    exclusive_write_bytes,
    stable_hash_path,
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--artifact")
    source.add_argument("--digest")
    parser.add_argument("--output", required=True)
    parser.add_argument("--listed-name", required=True)
    args = parser.parse_args()

    try:
        listed_name = canonical_relative_path(args.listed_name)
        if "/" in listed_name:
            raise ReleaseArtifactError(
                "checksum listed name must be one path component"
        )
        if args.artifact is not None:
            digest = stable_hash_path(Path(args.artifact)).sha256
        else:
            digest = args.digest
            if re.fullmatch(r"[0-9a-f]{64}", digest) is None:
                raise ReleaseArtifactError(
                    "checksum digest must be exactly 64 lowercase hex characters"
                )
        exclusive_write_bytes(
            Path(args.output),
            f"{digest}  {listed_name}\n".encode("ascii"),
            mode=0o644,
        )
        print(digest)
    except ReleaseArtifactError as exc:
        print(f"release checksum error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
