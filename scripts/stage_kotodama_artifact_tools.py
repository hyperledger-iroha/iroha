#!/usr/bin/env python3
"""Stage freshly built Kotodama tools for the unchanged single-link artifact guard.

Requires Python 3.11+ and POSIX descriptor operations on macOS/Linux. Pass the
Cargo profile directory and a new output directory outside the source checkout.
No environment variables are required. Existing destinations are never replaced;
failed staging remains available for inspection and must not be executed.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import stat

if __package__:
    from .check_native_sdk_artifact import (
        ArtifactContractError,
        stable_artifact_identity,
        stage_unique_artifact,
    )
else:
    from check_native_sdk_artifact import (
        ArtifactContractError,
        stable_artifact_identity,
        stage_unique_artifact,
    )


TOOLS = ("koto", "ivm_artifact_admit")


def revision(info: os.stat_result) -> tuple[int, ...]:
    """Bind a file's original object, owner, mode, bytes and link count."""
    return (
        info.st_dev,
        info.st_ino,
        info.st_uid,
        info.st_gid,
        info.st_mode,
        info.st_size,
        info.st_nlink,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def stage_tools(build_directory: Path, output: Path) -> None:
    """Copy both exact build outputs into a fresh owner-private executable directory."""
    output = output.parent.resolve(strict=True) / output.name
    output.mkdir(mode=0o700)
    directory = output.lstat()
    if directory.st_uid != os.getuid() or stat.S_IMODE(directory.st_mode) != 0o700:
        raise ArtifactContractError(
            "tool staging directory must be owner-private mode 700"
        )
    for name in TOOLS:
        source = build_directory / name
        before = source.lstat()
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_uid != os.getuid()
            or not before.st_mode & stat.S_IXUSR
            or before.st_mode & (0o022 | stat.S_ISUID | stat.S_ISGID | stat.S_ISVTX)
        ):
            raise ArtifactContractError(
                f"build tool must be an owned, unwritable-by-others executable: {source}"
            )
        # This existing descriptor-based copier accepts Cargo's hard links, pins
        # the source revision and proves the exclusive destination's digest/size.
        destination, digest, size = stage_unique_artifact(source, output / name)
        if revision(source.lstat()) != revision(before):
            raise ArtifactContractError(f"build tool changed during staging: {source}")
        descriptor = os.open(destination, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            opened = os.fstat(descriptor)
            if (
                not stat.S_ISREG(opened.st_mode)
                or opened.st_uid != os.getuid()
                or opened.st_nlink != 1
                or stat.S_IMODE(opened.st_mode) != 0o600
                or revision(opened) != revision(destination.lstat())
            ):
                raise ArtifactContractError(
                    "staged tool lost its private single-link identity"
                )
            os.fchmod(descriptor, 0o700)
            os.fsync(descriptor)
            executable = os.fstat(descriptor)
            if (
                stat.S_IMODE(executable.st_mode) != 0o700
                or executable.st_nlink != 1
                or revision(executable) != revision(destination.lstat())
                or stable_artifact_identity(destination) != (digest, size)
                or revision(os.fstat(descriptor)) != revision(executable)
                or revision(destination.lstat()) != revision(executable)
            ):
                raise ArtifactContractError(
                    "staged executable differs from its pinned build output"
                )
        finally:
            os.close(descriptor)
        current_directory = output.lstat()
        if (
            current_directory.st_dev,
            current_directory.st_ino,
            current_directory.st_uid,
            current_directory.st_mode,
        ) != (
            directory.st_dev,
            directory.st_ino,
            directory.st_uid,
            directory.st_mode,
        ):
            raise ArtifactContractError("private tool directory changed during staging")
        print(f"{name} sha256={digest} size={size} mode=0700 links=1")


def main() -> int:
    """Stage both named tools; refuse any ownership, source or destination mismatch."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-directory", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        stage_tools(args.build_directory, args.output)
    except (ArtifactContractError, OSError) as error:
        parser.exit(1, f"error: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
