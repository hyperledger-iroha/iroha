#!/usr/bin/env python3
"""Execute a command while retaining a secure, process-held advisory file lock."""

from __future__ import annotations

import fcntl
import os
import stat
import sys
from pathlib import Path
from typing import NoReturn


def _fail(message: str) -> NoReturn:
    print(f"[-] {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    if len(sys.argv) < 4:
        _fail("usage: exec_with_file_lock.py LOCK_FILE MARKER_ENV COMMAND [ARG ...]")
    lock_path = Path(sys.argv[1])
    marker = sys.argv[2]
    command = sys.argv[3:]
    if not lock_path.is_absolute() or lock_path.name in {"", ".", ".."}:
        _fail("lock path must be an absolute regular filename")
    if "=" not in marker:
        _fail("lock marker must be KEY=VALUE")

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
    try:
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(lock_fd)
        _fail(f"another builder holds {lock_path}")
    except OSError as error:
        os.close(lock_fd)
        _fail(f"unable to acquire lock {lock_path}: {error}")

    os.set_inheritable(lock_fd, True)
    key, value = marker.split("=", 1)
    environment = os.environ.copy()
    environment[key] = value
    try:
        os.execvpe(command[0], command, environment)
    except OSError as error:
        os.close(lock_fd)
        _fail(f"unable to execute {command[0]} while holding {lock_path}: {error}")


if __name__ == "__main__":
    main()
