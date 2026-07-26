#!/usr/bin/env python3
"""Build one bounded deterministic tar.gz from an exact staged inventory."""

from __future__ import annotations

import argparse
import gzip
import json
import os
import sys
import tarfile
from pathlib import Path, PurePosixPath

from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_relative_path,
    exclusive_output_fd,
    parse_source_date_epoch,
    scan_inventory_paths,
    stable_hash_relative,
    stable_open_relative,
)


MAX_FILE_SIZE = 512 * 1024 * 1024
MAX_TOTAL_SIZE = 1024 * 1024 * 1024


def _directory_paths(files: list[str]) -> set[str]:
    directories: set[str] = set()
    for relative in files:
        parts = PurePosixPath(relative).parts
        for length in range(1, len(parts)):
            directories.add(PurePosixPath(*parts[:length]).as_posix())
    return directories


def _tar_info(
    name: str,
    *,
    mode: int,
    mtime: int,
    size: int = 0,
    directory: bool,
) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = mtime
    info.mode = mode
    info.size = size
    info.pax_headers = {}
    if directory:
        info.type = tarfile.DIRTYPE
    return info


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage-root", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--prefix", required=True)
    parser.add_argument("--source-date-epoch", required=True)
    parser.add_argument("--file", action="append", default=[], required=True)
    parser.add_argument("--executable", action="append", default=[])
    args = parser.parse_args()

    try:
        epoch = parse_source_date_epoch(args.source_date_epoch)
        prefix = canonical_relative_path(args.prefix)
        if "/" in prefix:
            raise ReleaseArtifactError("archive prefix must be one path component")
        files = [canonical_relative_path(value) for value in args.file]
        if len(files) != len(set(files)):
            raise ReleaseArtifactError("archive file inventory contains duplicates")
        files.sort()
        executables = {
            canonical_relative_path(value) for value in args.executable
        }
        if not executables <= set(files):
            raise ReleaseArtifactError(
                "archive executable inventory must be a subset of files"
            )

        stage_root = Path(args.stage_root)
        if scan_inventory_paths(stage_root) != files:
            raise ReleaseArtifactError(
                "staged archive inventory does not exactly match --file entries"
            )

        captured: dict[str, object] = {}
        total_size = 0
        for relative in files:
            info = stable_hash_relative(
                stage_root,
                relative,
                max_size=MAX_FILE_SIZE,
            )
            expected_mode = 0o755 if relative in executables else 0o644
            if info.mode != expected_mode:
                raise ReleaseArtifactError(
                    f"staged archive entry {relative!r} mode must be "
                    f"{expected_mode:04o}"
                )
            total_size += info.size
            if total_size > MAX_TOTAL_SIZE:
                raise ReleaseArtifactError(
                    f"staged archive exceeds the {MAX_TOTAL_SIZE}-byte limit"
                )
            captured[relative] = info

        if scan_inventory_paths(stage_root) != files:
            raise ReleaseArtifactError(
                "staged archive inventory changed before archive creation"
            )

        directories = _directory_paths(files)
        with exclusive_output_fd(Path(args.output), mode=0o644) as output_fd:
            with os.fdopen(os.dup(output_fd), "wb") as raw:
                with gzip.GzipFile(
                    filename="",
                    mode="wb",
                    compresslevel=9,
                    fileobj=raw,
                    mtime=epoch,
                ) as compressed:
                    with tarfile.open(
                        fileobj=compressed,
                        mode="w",
                        format=tarfile.PAX_FORMAT,
                    ) as archive:
                        archive.addfile(
                            _tar_info(
                                prefix,
                                mode=0o755,
                                mtime=epoch,
                                directory=True,
                            )
                        )
                        for relative in sorted(directories | set(files)):
                            archive_name = f"{prefix}/{relative}"
                            if relative in directories:
                                archive.addfile(
                                    _tar_info(
                                        archive_name,
                                        mode=0o755,
                                        mtime=epoch,
                                        directory=True,
                                    )
                                )
                                continue
                            info = captured[relative]
                            with stable_open_relative(
                                stage_root,
                                relative,
                                expected=info,
                            ) as source_fd:
                                with os.fdopen(
                                    os.dup(source_fd),
                                    "rb",
                                    closefd=True,
                                ) as source_file:
                                    archive.addfile(
                                        _tar_info(
                                            archive_name,
                                            mode=info.mode,
                                            mtime=epoch,
                                            size=info.size,
                                            directory=False,
                                        ),
                                        source_file,
                                    )
                            if (
                                stable_hash_relative(
                                    stage_root,
                                    relative,
                                    max_size=MAX_FILE_SIZE,
                                )
                                != info
                            ):
                                raise ReleaseArtifactError(
                                    f"staged archive entry {relative!r} changed "
                                    "during creation"
                                )
                raw.flush()
                os.fsync(raw.fileno())
            if scan_inventory_paths(stage_root) != files:
                raise ReleaseArtifactError(
                    "staged archive inventory changed during archive creation"
                )
            for relative, before in captured.items():
                if (
                    stable_hash_relative(
                        stage_root,
                        relative,
                        max_size=MAX_FILE_SIZE,
                    )
                    != before
                ):
                    raise ReleaseArtifactError(
                        f"staged archive entry {relative!r} changed during creation"
                    )

        digests = {
            relative: captured[relative].sha256 for relative in files
        }
        print(
            json.dumps(
                digests,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
                allow_nan=False,
            )
        )
    except (OSError, ReleaseArtifactError, tarfile.TarError) as exc:
        print(f"release tar.gz error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
