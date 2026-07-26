#!/usr/bin/env python3
"""Build a deterministic tar.zst from an exact normalized staged inventory."""

from __future__ import annotations

import argparse
import json
import os
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path, PurePosixPath

from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_relative_path,
    exclusive_output_fd,
    exclusive_write_bytes,
    parse_source_date_epoch,
    scan_inventory_paths,
    stable_hash_path,
    stable_hash_relative,
    stable_open_relative,
    stable_read_path,
)


MAX_FILE_SIZE = 512 * 1024 * 1024
MAX_TOTAL_SIZE = 2 * 1024 * 1024 * 1024
MAX_ZSTD_ERROR_SIZE = 64 * 1024
MAX_ZSTD_EXECUTABLE_SIZE = 256 * 1024 * 1024


def _directories(files: list[str]) -> set[str]:
    result: set[str] = set()
    for relative in files:
        parts = PurePosixPath(relative).parts
        for length in range(1, len(parts)):
            result.add(PurePosixPath(*parts[:length]).as_posix())
    return result


def _info(
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
    info.mode = mode
    info.mtime = mtime
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
    parser.add_argument("--zstd", required=True)
    parser.add_argument("--trusted-zstd-sha256", required=True)
    parser.add_argument("--file", action="append", default=[])
    parser.add_argument("--file-list-json", action="append", default=[])
    parser.add_argument("--executable", action="append", default=[])
    args = parser.parse_args()
    try:
        epoch = parse_source_date_epoch(args.source_date_epoch)
        prefix = canonical_relative_path(args.prefix)
        if "/" in prefix:
            raise ReleaseArtifactError("archive prefix must be one path component")
        files = [canonical_relative_path(value) for value in args.file]
        for raw in args.file_list_json:
            try:
                decoded = json.loads(raw)
            except json.JSONDecodeError as exc:
                raise ReleaseArtifactError(
                    f"archive file-list JSON is invalid: {exc}"
                ) from exc
            if not isinstance(decoded, list) or not all(
                isinstance(value, str) for value in decoded
            ):
                raise ReleaseArtifactError(
                    "archive file-list JSON must be an array of strings"
                )
            files.extend(canonical_relative_path(value) for value in decoded)
        if not files or len(files) != len(set(files)):
            raise ReleaseArtifactError(
                "archive inventory must be non-empty and duplicate-free"
            )
        files.sort()
        executables = {
            canonical_relative_path(value) for value in args.executable
        }
        if not executables <= set(files):
            raise ReleaseArtifactError(
                "archive executable inventory must be a subset of files"
            )
        if re.fullmatch(r"[0-9a-f]{64}", args.trusted_zstd_sha256) is None:
            raise ReleaseArtifactError(
                "trusted zstd SHA256 must be 64 lowercase hex characters"
            )
        zstd = Path(args.zstd)
        pinned_zstd, zstd_payload = stable_read_path(
            zstd,
            max_size=MAX_ZSTD_EXECUTABLE_SIZE,
        )
        if pinned_zstd.sha256 != args.trusted_zstd_sha256:
            raise ReleaseArtifactError("zstd executable SHA256 is not trusted")
        if not pinned_zstd.mode & stat.S_IXUSR:
            raise ReleaseArtifactError("zstd executable must be owner-executable")

        stage = Path(args.stage_root)
        if scan_inventory_paths(stage) != files:
            raise ReleaseArtifactError(
                "staged tar.zst inventory does not exactly match declared files"
            )
        captured: dict[str, object] = {}
        total = 0
        for relative in files:
            file_info = stable_hash_relative(
                stage,
                relative,
                max_size=MAX_FILE_SIZE,
            )
            expected_mode = 0o755 if relative in executables else 0o644
            if file_info.mode != expected_mode:
                raise ReleaseArtifactError(
                    f"staged entry {relative!r} mode must be {expected_mode:04o}"
                )
            total += file_info.size
            if total > MAX_TOTAL_SIZE:
                raise ReleaseArtifactError(
                    f"staged tar.zst exceeds {MAX_TOTAL_SIZE} bytes"
                )
            captured[relative] = file_info

        process: subprocess.Popen[bytes] | None = None
        temp_parent = os.path.realpath(tempfile.gettempdir())
        with tempfile.TemporaryDirectory(
            prefix="iroha-release-zstd.",
            dir=temp_parent,
        ) as tool_directory_raw:
            tool_directory = Path(tool_directory_raw)
            private_zstd = tool_directory / "zstd"
            exclusive_write_bytes(private_zstd, zstd_payload, mode=0o755)
            private_zstd_capture = stable_hash_path(private_zstd)
            if private_zstd_capture.sha256 != pinned_zstd.sha256:
                raise ReleaseArtifactError("private zstd copy digest mismatch")
            with exclusive_output_fd(Path(args.output), mode=0o644) as output_fd:
                try:
                    with tempfile.TemporaryFile() as error_file:
                        process = subprocess.Popen(
                            [
                                str(private_zstd),
                                "-19",
                                "--long=31",
                                "--threads=1",
                                "--no-progress",
                                "--stdout",
                            ],
                            stdin=subprocess.PIPE,
                            stdout=output_fd,
                            stderr=error_file,
                        )
                        assert process.stdin is not None
                        with tarfile.open(
                            fileobj=process.stdin,
                            mode="w|",
                            format=tarfile.PAX_FORMAT,
                        ) as archive:
                            archive.addfile(
                                _info(
                                    prefix,
                                    mode=0o755,
                                    mtime=epoch,
                                    directory=True,
                                )
                            )
                            for relative in sorted(_directories(files) | set(files)):
                                name = f"{prefix}/{relative}"
                                if relative not in captured:
                                    archive.addfile(
                                        _info(
                                            name,
                                            mode=0o755,
                                            mtime=epoch,
                                            directory=True,
                                        )
                                    )
                                    continue
                                file_info = captured[relative]
                                with stable_open_relative(
                                    stage,
                                    relative,
                                    expected=file_info,
                                ) as source_fd:
                                    with os.fdopen(
                                        os.dup(source_fd),
                                        "rb",
                                        closefd=True,
                                    ) as source_file:
                                        archive.addfile(
                                            _info(
                                                name,
                                                mode=file_info.mode,
                                                mtime=epoch,
                                                size=file_info.size,
                                                directory=False,
                                            ),
                                            source_file,
                                        )
                                if (
                                    stable_hash_relative(
                                        stage,
                                        relative,
                                        max_size=MAX_FILE_SIZE,
                                    )
                                    != file_info
                                ):
                                    raise ReleaseArtifactError(
                                        f"staged entry {relative!r} changed "
                                        "during archive creation"
                                    )
                        process.stdin.close()
                        returncode = process.wait(timeout=1800)
                        error_file.seek(0)
                        error = error_file.read(MAX_ZSTD_ERROR_SIZE + 1)
                        if len(error) > MAX_ZSTD_ERROR_SIZE:
                            raise ReleaseArtifactError(
                                "zstd error output exceeded its bound"
                            )
                        if returncode != 0:
                            rendered = error.decode(
                                "utf-8",
                                errors="replace",
                            ).strip()
                            raise ReleaseArtifactError(
                                f"zstd failed with status {returncode}: {rendered}"
                            )
                except BaseException:
                    if process is not None and process.poll() is None:
                        process.kill()
                        process.wait()
                    raise
                if scan_inventory_paths(stage) != files:
                    raise ReleaseArtifactError(
                        "staged tar.zst inventory changed during archive creation"
                    )
                for relative, before in captured.items():
                    if (
                        stable_hash_relative(
                            stage,
                            relative,
                            max_size=MAX_FILE_SIZE,
                        )
                        != before
                    ):
                        raise ReleaseArtifactError(
                            f"staged entry {relative!r} changed during archive "
                            "creation"
                        )
                if stable_hash_path(zstd) != pinned_zstd:
                    raise ReleaseArtifactError(
                        "zstd executable changed during archive creation"
                    )
                if stable_hash_path(private_zstd) != private_zstd_capture:
                    raise ReleaseArtifactError(
                        "private zstd copy changed during archive creation"
                    )
    except (
        OSError,
        ReleaseArtifactError,
        subprocess.SubprocessError,
        tarfile.TarError,
    ) as exc:
        print(f"release tar.zst error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
