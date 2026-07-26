#!/usr/bin/env python3
"""Generate a deterministic manifest for repo evidence bundles (roadmap F1).

The repo lifecycle roadmap requires teams to attach a reproducible manifest to
every governance packet. This helper walks an evidence directory, records the
size and digests of each file, and emits a Norito-friendly JSON document that
auditors can replay later.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Sequence


def _isoformat(timestamp: float) -> str:
    """Render a UTC timestamp without microseconds."""
    return (
        datetime.fromtimestamp(timestamp, timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def _hash_file(path: Path) -> dict[str, object]:
    sha256 = hashlib.sha256()
    blake2b = hashlib.blake2b()
    size = 0
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            sha256.update(chunk)
            blake2b.update(chunk)
            size += len(chunk)
    stat = path.stat()
    return {
        "path": "",
        "size": size,
        "sha256": sha256.hexdigest(),
        "blake2b": blake2b.hexdigest(),
        "modified_at": _isoformat(stat.st_mtime),
    }


def build_manifest(
    root: Path,
    *,
    agreement_id: str | None = None,
    skip_paths: Iterable[Path] = (),
    exclude_patterns: Iterable[str] = (),
) -> dict[str, object]:
    """Return the manifest for every file under ``root``."""
    resolved_root = root.resolve(strict=True)
    if not resolved_root.is_dir():
        raise NotADirectoryError(f"{resolved_root} is not a directory")

    skipped = {path.resolve() for path in skip_paths}
    excludes = tuple(pattern for pattern in exclude_patterns if pattern)
    files: list[dict[str, object]] = []
    total_bytes = 0
    for path in sorted(
        (candidate for candidate in resolved_root.rglob("*") if candidate.is_file()),
        key=lambda candidate: candidate.relative_to(resolved_root).as_posix(),
    ):
        if path.resolve() in skipped:
            continue
        relative_posix = path.relative_to(resolved_root).as_posix()
        if excludes and any(fnmatch.fnmatch(relative_posix, pattern) for pattern in excludes):
            continue
        entry = _hash_file(path)
        entry = {
            "path": relative_posix,
            "size": entry["size"],
            "sha256": entry["sha256"],
            "blake2b": entry["blake2b"],
            "modified_at": entry["modified_at"],
        }
        total_bytes += int(entry["size"])
        files.append(entry)

    manifest = {
        "agreement_id": agreement_id or resolved_root.name,
        "generated_at": _isoformat(datetime.now(timezone.utc).timestamp()),
        "root": str(resolved_root),
        "file_count": len(files),
        "total_bytes": total_bytes,
        "files": files,
    }
    return manifest


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate a deterministic manifest for repo evidence bundles.",
    )
    parser.add_argument(
        "--root",
        required=True,
        help="Directory containing the evidence bundle.",
    )
    parser.add_argument(
        "--agreement-id",
        help="Override the agreement identifier recorded in the manifest.",
    )
    parser.add_argument(
        "--output",
        help="Optional path for writing the manifest as JSON (prints to stdout when omitted).",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        metavar="PATTERN",
        help=(
            "Glob pattern to skip (relative to --root). "
            "Repeat the option to ignore multiple files, e.g. --exclude 'scratch/*'."
        ),
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress the success message when --output is used.",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entrypoint."""
    try:
        args = _parse_args(argv)
        root = Path(args.root)
        output_path = Path(args.output).resolve() if args.output else None
        skip = [output_path] if output_path else []
        manifest = build_manifest(
            root,
            agreement_id=args.agreement_id,
            skip_paths=skip,
            exclude_patterns=args.exclude,
        )
    except FileNotFoundError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    except NotADirectoryError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1

    serialized = json.dumps(manifest, indent=2)
    if args.output:
        output_path = Path(args.output)
        output_path.write_text(serialized + "\n", encoding="utf-8")
        if not args.quiet:
            print(f"repo evidence manifest written to {output_path}", file=sys.stderr)
    else:
        print(serialized)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
