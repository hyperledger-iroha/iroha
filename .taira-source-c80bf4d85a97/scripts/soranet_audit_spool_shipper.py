#!/usr/bin/env python3
"""Batch ship SoraNet relay compliance events from the local spool directory.

The compliance logger writes one JSON document per file under the spool path.
This helper collects pending files, concatenates them into JSONL archives, and
(optionally) invokes an external command to transmit the archive to a remote
backend. Archives are stored locally so operators can manage retention or
troubleshoot shipments.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, List

DEFAULT_BATCH = 200


def iter_spool_files(spool: Path) -> Iterable[Path]:
    if not spool.exists():
        return []
    return sorted(
        (p for p in spool.iterdir() if p.is_file() and p.suffix == ".json"),
        key=lambda path: path.stat().st_mtime,
    )


def write_archive(batch: List[Path], archive_dir: Path, dry_run: bool) -> Path:
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    archive_path = archive_dir / f"compliance-{timestamp}-{os.getpid()}.jsonl"
    if dry_run:
        return archive_path

    archive_dir.mkdir(parents=True, exist_ok=True)
    with archive_path.open("w", encoding="utf-8") as archive:
        for file_path in batch:
            with file_path.open("r", encoding="utf-8") as source:
                data = json.load(source)
            archive.write(json.dumps(data))
            archive.write("\n")
    return archive_path


def ship_archive(archive_path: Path, command: str, dry_run: bool) -> None:
    if dry_run or not command:
        return
    env = os.environ.copy()
    env["SORANET_AUDIT_ARCHIVE"] = str(archive_path)
    subprocess.run(command, shell=True, check=True, env=env)


def cleanup_batch(batch: List[Path], processed_dir: Path, dry_run: bool) -> None:
    if dry_run:
        return
    processed_dir.mkdir(parents=True, exist_ok=True)
    for file_path in batch:
        destination = processed_dir / file_path.name
        shutil.move(str(file_path), destination)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--spool-dir",
        default="/var/spool/soranet/audit",
        type=Path,
        help="Directory containing compliance JSON files",
    )
    parser.add_argument(
        "--archive-dir",
        default="./audit-archives",
        type=Path,
        help="Where combined JSONL archives should be stored",
    )
    parser.add_argument(
        "--processed-dir",
        default="./audit-processed",
        type=Path,
        help="Where original JSON files are moved after archiving",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=DEFAULT_BATCH,
        help="Maximum number of JSON files per archive",
    )
    parser.add_argument(
        "--ship-command",
        default="",
        help="Optional shell command invoked once per archive. The archive path is provided via $SORANET_AUDIT_ARCHIVE.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print actions without modifying files",
    )

    args = parser.parse_args()
    batch: List[Path] = []
    archives_created = 0
    shipped = 0

    for file_path in iter_spool_files(args.spool_dir):
        batch.append(file_path)
        if len(batch) < args.batch_size:
            continue
        archive_path = write_archive(batch, args.archive_dir, args.dry_run)
        ship_archive(archive_path, args.ship_command, args.dry_run)
        cleanup_batch(batch, args.processed_dir, args.dry_run)
        batch = []
        archives_created += 1
        if args.ship_command:
            shipped += 1

    if batch:
        archive_path = write_archive(batch, args.archive_dir, args.dry_run)
        ship_archive(archive_path, args.ship_command, args.dry_run)
        cleanup_batch(batch, args.processed_dir, args.dry_run)
        archives_created += 1
        if args.ship_command:
            shipped += 1

    print(
        f"Processed {archives_created} archive(s); shipped {shipped} via command"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
