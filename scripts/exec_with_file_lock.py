#!/usr/bin/env python3
"""Execute a command while retaining authenticated advisory file locks."""

from __future__ import annotations

import fcntl
import os
import re
import stat
import sys
from pathlib import Path
from typing import NoReturn


def _fail(message: str) -> NoReturn:
    print(f"[-] {message}", file=sys.stderr)
    raise SystemExit(1)


def _open_lock(lock_path: Path) -> int:
    if (
        not lock_path.is_absolute()
        or lock_path != Path(os.path.abspath(lock_path))
        or lock_path.name in {"", ".", ".."}
    ):
        _fail("lock path must be an absolute canonical regular filename")
    try:
        canonical_parent = lock_path.parent.resolve(strict=True)
    except OSError as error:
        _fail(f"unable to resolve lock directory {lock_path.parent}: {error}")
    if canonical_parent != lock_path.parent:
        _fail(f"lock directory must be canonical and non-symbolic: {lock_path.parent}")

    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    directory_flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        directory_fd = os.open(lock_path.parent, directory_flags)
    except OSError as error:
        _fail(f"unable to open lock directory {lock_path.parent}: {error}")
    try:
        lock_flags = os.O_RDWR | os.O_CREAT | getattr(os, "O_NOFOLLOW", 0)
        lock_fd = os.open(lock_path.name, lock_flags, 0o600, dir_fd=directory_fd)
    except OSError as error:
        os.close(directory_fd)
        _fail(f"unable to open lock file {lock_path}: {error}")
    os.close(directory_fd)

    metadata = os.fstat(lock_fd)
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.geteuid()
    ):
        os.close(lock_fd)
        _fail(f"refusing unsafe lock file {lock_path}")
    return lock_fd


def main() -> None:
    try:
        separator = sys.argv.index("--")
    except ValueError:
        separator = -1
    if separator < 3 or separator == len(sys.argv) - 1:
        _fail(
            "usage: exec_with_file_lock.py FD_ENV LOCK_FILE [LOCK_FILE ...] "
            "-- COMMAND [ARG ...]"
        )
    descriptor_environment = sys.argv[1]
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", descriptor_environment) is None:
        _fail("lock descriptor environment name is not canonical")
    raw_paths = [Path(value) for value in sys.argv[2:separator]]
    lock_paths = sorted(raw_paths, key=os.fspath)
    if len(set(lock_paths)) != len(lock_paths):
        _fail("duplicate lock paths are not allowed")
    command = sys.argv[separator + 1 :]

    lock_descriptors: list[int] = []
    try:
        for lock_path in lock_paths:
            lock_fd = _open_lock(lock_path)
            try:
                fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                os.close(lock_fd)
                _fail(f"another process holds {lock_path}")
            except OSError as error:
                os.close(lock_fd)
                _fail(f"unable to acquire lock {lock_path}: {error}")
            os.set_inheritable(lock_fd, True)
            lock_descriptors.append(lock_fd)

        environment = os.environ.copy()
        environment[descriptor_environment] = ",".join(
            str(descriptor) for descriptor in lock_descriptors
        )
        os.execvpe(command[0], command, environment)
    except OSError as error:
        for descriptor in lock_descriptors:
            os.close(descriptor)
        _fail(f"unable to execute {command[0]} while holding {lock_path}: {error}")


if __name__ == "__main__":
    main()
