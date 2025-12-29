#!/usr/bin/env python3
"""
Bundle Android Norito instruction examples into a reproducible archive.

This helper zips the files under `target-codex/android_codegen/instruction_examples`
and writes the resulting archive (plus SHA-256 digest and metadata) under
`artifacts/android/codegen_fixtures/<timestamp>/`.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import sys
import zipfile


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Bundle Android codegen instruction examples."
    )
    parser.add_argument(
        "--source",
        type=Path,
        default=Path("target-codex/android_codegen/instruction_examples"),
        help="Directory containing instruction example JSON files.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("artifacts/android/codegen_fixtures"),
        help="Root directory for generated archives.",
    )
    parser.add_argument(
        "--timestamp",
        help="Override ISO8601 timestamp (defaults to current UTC).",
    )
    return parser.parse_args()


def zip_examples(source: Path, destination: Path) -> None:
    with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for entry in sorted(source.rglob("*.json")):
            rel = entry.relative_to(source)
            arcname = Path("instruction_examples") / rel
            zf.write(entry, arcname)


def sha256sum(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def write_metadata(metadata_path: Path, payload: dict) -> None:
    metadata_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def main() -> int:
    args = parse_args()
    source = args.source
    if not source.is_dir():
        print(f"Source directory {source} does not exist", file=sys.stderr)
        return 1

    timestamp = (
        args.timestamp
        if args.timestamp
        else datetime.datetime.utcnow().strftime("%Y%m%dT%H%M%SZ")
    )
    output_root = args.out / timestamp
    output_root.mkdir(parents=True, exist_ok=True)

    archive_path = output_root / "instruction_examples.zip"
    zip_examples(source, archive_path)

    digest = sha256sum(archive_path)
    (archive_path.with_suffix(".zip.sha256")).write_text(digest + "\n", encoding="utf-8")

    metadata = {
        "generated_at": timestamp,
        "source": str(source),
        "archive": str(archive_path),
        "sha256": digest,
        "file_count": sum(1 for _ in source.rglob("*.json")),
        "cwd": os.getcwd(),
    }
    write_metadata(output_root / "metadata.json", metadata)
    print(f"Wrote {archive_path} ({metadata['file_count']} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
