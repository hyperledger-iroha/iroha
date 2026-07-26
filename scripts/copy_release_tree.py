#!/usr/bin/env python3
"""Copy one closed regular-file tree into a normalized release staging root."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_relative_path,
    exclusive_write_bytes,
    scan_inventory_paths,
    stable_hash_relative,
    stable_read_relative,
)


MAX_TREE_FILE_SIZE = 64 * 1024 * 1024
MAX_TREE_SIZE = 512 * 1024 * 1024


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True)
    parser.add_argument("--output-root", required=True)
    parser.add_argument("--destination-prefix", required=True)
    parser.add_argument("--ignore", action="append", default=[])
    args = parser.parse_args()
    try:
        source = Path(args.source_root)
        output = Path(args.output_root)
        prefix = canonical_relative_path(args.destination_prefix)
        ignored = {canonical_relative_path(value) for value in args.ignore}
        complete_source_paths = scan_inventory_paths(source)
        missing_ignored = ignored - set(complete_source_paths)
        if missing_ignored:
            raise ReleaseArtifactError(
                "ignored release source paths do not exist: "
                + ", ".join(sorted(missing_ignored))
            )
        source_paths = [
            relative
            for relative in complete_source_paths
            if relative not in ignored
        ]
        if not source_paths:
            raise ReleaseArtifactError("release source tree must not be empty")
        captured: dict[str, object] = {}
        outputs: list[str] = []
        total = 0
        for relative in source_paths:
            info, payload = stable_read_relative(
                source,
                relative,
                max_size=MAX_TREE_FILE_SIZE,
                return_payload=True,
            )
            assert payload is not None
            total += info.size
            if total > MAX_TREE_SIZE:
                raise ReleaseArtifactError(
                    f"release source tree exceeds {MAX_TREE_SIZE} bytes"
                )
            destination = canonical_relative_path(f"{prefix}/{relative}")
            destination_path = output / Path(*destination.split("/"))
            destination_path.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
            exclusive_write_bytes(destination_path, payload, mode=0o644)
            captured[relative] = info
            outputs.append(destination)
        if scan_inventory_paths(source) != complete_source_paths:
            raise ReleaseArtifactError("release source tree inventory changed")
        for relative, before in captured.items():
            if stable_hash_relative(source, relative) != before:
                raise ReleaseArtifactError(
                    f"release source tree entry {relative!r} changed"
                )
        print(
            json.dumps(
                sorted(outputs),
                separators=(",", ":"),
                ensure_ascii=True,
            )
        )
    except ReleaseArtifactError as exc:
        print(f"release tree copy error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
