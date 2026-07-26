#!/usr/bin/env python3
"""Copy one stable release input to one exclusive normalized output."""

from __future__ import annotations

import argparse
import stat
import sys
from pathlib import Path

from release_artifact_contract import (
    ReleaseArtifactError,
    exclusive_write_bytes,
    stable_read_path,
)

MAX_COPY_BYTES = 512 * 1024 * 1024


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--mode", choices=("0644", "0755"), required=True)
    parser.add_argument("--require-executable", action="store_true")
    args = parser.parse_args()
    try:
        info, payload = stable_read_path(
            Path(args.source),
            max_size=MAX_COPY_BYTES,
        )
        if args.require_executable and not info.mode & stat.S_IXUSR:
            raise ReleaseArtifactError("release input must be owner-executable")
        exclusive_write_bytes(
            Path(args.output),
            payload,
            mode=int(args.mode, 8),
        )
    except ReleaseArtifactError as exc:
        print(f"release copy error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
