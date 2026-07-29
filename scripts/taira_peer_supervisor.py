#!/usr/bin/env python3
"""Supervise exactly one Taira validator process.

This helper is installed by ``migrate_taira_peer_supervision.py`` and is not
intended to be started by hand.  It preserves the validated binary, config,
and storage-directory identities from the migration plan, forwards shutdown
signals to the validator, and applies bounded exponential restart backoff.
"""

from __future__ import annotations

import argparse
import hashlib
import math
import os
import signal
import stat
import subprocess
import sys
import time
from pathlib import Path
from types import FrameType


class IdentityError(RuntimeError):
    """Raised when a planned runtime path has changed identity."""


BINARY_STAT_SEAL_FIELDS = (
    "binary_device",
    "binary_inode",
    "binary_size",
    "binary_mtime_ns",
    "binary_ctime_ns",
)


def sha256_file(path: Path) -> str:
    """Return the SHA-256 digest of a regular file without following symlinks."""

    before = path.lstat()
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise IdentityError(f"expected a non-symlink regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        digest = hashlib.sha256()
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (before.st_dev, before.st_ino, before.st_size) != (
        after.st_dev,
        after.st_ino,
        after.st_size,
    ):
        raise IdentityError(f"file changed while hashing: {path}")
    return digest.hexdigest()


def binary_stat_seal(
    args: argparse.Namespace,
) -> tuple[int, int, int, int, int] | None:
    """Return the optional all-or-none binary stat seal."""

    values = tuple(getattr(args, field, None) for field in BINARY_STAT_SEAL_FIELDS)
    present = tuple(value is not None for value in values)
    if not any(present):
        return None
    if not all(present):
        raise IdentityError("binary stat seal fields must be provided together")
    device, inode, size, mtime_ns, ctime_ns = values
    if (
        not isinstance(device, int)
        or device < 0
        or not isinstance(inode, int)
        or inode <= 0
        or not isinstance(size, int)
        or size < 0
        or not isinstance(mtime_ns, int)
        or mtime_ns < 0
        or not isinstance(ctime_ns, int)
        or ctime_ns < 0
    ):
        raise IdentityError("binary stat seal metadata is invalid")
    return device, inode, size, mtime_ns, ctime_ns


def require_trusted_binary_path(path: Path) -> None:
    """Require a root-owned path that the runtime user cannot rename or rewrite."""

    if not path.is_absolute() or ".." in path.parts:
        raise IdentityError(
            f"stat-sealed validator binary path is not canonical and absolute: {path}"
        )
    components = [*reversed(path.parents), path]
    for index, component in enumerate(components):
        info = component.lstat()
        if stat.S_ISLNK(info.st_mode):
            raise IdentityError(
                f"stat-sealed validator binary path contains a symlink: {component}"
            )
        if index + 1 == len(components):
            if not stat.S_ISREG(info.st_mode):
                raise IdentityError(
                    f"stat-sealed validator binary is not a regular file: {component}"
                )
        elif not stat.S_ISDIR(info.st_mode):
            raise IdentityError(
                f"stat-sealed validator binary ancestor is not a directory: {component}"
            )
        if info.st_uid != 0:
            raise IdentityError(
                f"stat-sealed validator binary path is not root-owned: {component}"
            )
        if stat.S_IMODE(info.st_mode) & 0o022:
            raise IdentityError(
                "stat-sealed validator binary path is group/world writable: "
                f"{component}"
            )


def require_binary_stat_identity(
    path: Path, expected: tuple[int, int, int, int, int]
) -> None:
    """Validate an executable binary against an O(1) descriptor stat seal."""

    require_trusted_binary_path(path)
    before = path.lstat()
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise IdentityError(f"expected a non-symlink regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    before_identity = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )
    after_identity = (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    )
    if not stat.S_ISREG(after.st_mode) or before_identity != after_identity:
        raise IdentityError(f"validator binary changed during stat validation: {path}")
    if after_identity != expected:
        raise IdentityError(f"validator binary stat identity changed: {path}")
    if not after.st_mode & 0o111:
        raise IdentityError(f"validator binary is not executable: {path}")


def require_runtime_identity(args: argparse.Namespace) -> None:
    """Refuse when binary, config, working-directory, or storage identity drifted."""

    binary = Path(args.binary)
    config = Path(args.config)
    workdir = Path(args.workdir)
    storage_dir = Path(args.storage_dir)
    stat_seal = binary_stat_seal(args)
    if stat_seal is None:
        if sha256_file(binary) != args.binary_sha256:
            raise IdentityError(f"validator binary digest changed: {binary}")
        if not os.access(binary, os.X_OK):
            raise IdentityError(f"validator binary is not executable: {binary}")
    else:
        require_binary_stat_identity(binary, stat_seal)
    if sha256_file(config) != args.config_sha256:
        raise IdentityError(f"validator config digest changed: {config}")
    workdir_stat = workdir.lstat()
    if stat.S_ISLNK(workdir_stat.st_mode) or not stat.S_ISDIR(workdir_stat.st_mode):
        raise IdentityError(f"storage path is not a non-symlink directory: {workdir}")
    if (
        workdir_stat.st_dev != args.workdir_device
        or workdir_stat.st_ino != args.workdir_inode
    ):
        raise IdentityError(f"working directory identity changed: {workdir}")
    storage_stat = storage_dir.lstat()
    if stat.S_ISLNK(storage_stat.st_mode) or not stat.S_ISDIR(storage_stat.st_mode):
        raise IdentityError(
            f"storage path is not a non-symlink directory: {storage_dir}"
        )
    if (
        storage_stat.st_dev != args.storage_device
        or storage_stat.st_ino != args.storage_inode
    ):
        raise IdentityError(f"storage directory identity changed: {storage_dir}")


def atomic_write_pid(path: Path, pid: int) -> None:
    """Atomically publish the currently supervised validator PID."""

    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    descriptor = os.open(
        temporary,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        body = f"{pid}\n".encode("ascii")
        offset = 0
        while offset < len(body):
            offset += os.write(descriptor, body[offset:])
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.replace(temporary, path)


def remove_owned_pid(path: Path, pid: int) -> None:
    """Remove a PID file only when it still names this supervisor's child."""

    try:
        current = path.read_text(encoding="ascii").strip()
    except FileNotFoundError:
        return
    if current == str(pid):
        path.unlink(missing_ok=True)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse the launchd-owned single-peer supervisor arguments."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--binary-device", type=int)
    parser.add_argument("--binary-inode", type=int)
    parser.add_argument("--binary-size", type=int)
    parser.add_argument("--binary-mtime-ns", type=int)
    parser.add_argument("--binary-ctime-ns", type=int)
    parser.add_argument("--config", required=True)
    parser.add_argument("--config-sha256", required=True)
    parser.add_argument("--workdir", required=True)
    parser.add_argument("--workdir-device", required=True, type=int)
    parser.add_argument("--workdir-inode", required=True, type=int)
    parser.add_argument("--storage-dir", required=True)
    parser.add_argument("--storage-device", required=True, type=int)
    parser.add_argument("--storage-inode", required=True, type=int)
    parser.add_argument("--pid-file", required=True)
    parser.add_argument("--initial-backoff-seconds", type=float, default=1.0)
    parser.add_argument("--maximum-backoff-seconds", type=float, default=30.0)
    parser.add_argument("--stable-uptime-seconds", type=float, default=120.0)
    args = parser.parse_args(argv)
    try:
        binary_stat_seal(args)
    except IdentityError as exc:
        parser.error(str(exc))
    if (
        not math.isfinite(args.initial_backoff_seconds)
        or args.initial_backoff_seconds <= 0
    ):
        parser.error("--initial-backoff-seconds must be positive")
    if (
        not math.isfinite(args.maximum_backoff_seconds)
        or args.maximum_backoff_seconds < args.initial_backoff_seconds
    ):
        parser.error(
            "--maximum-backoff-seconds must be at least the initial backoff"
        )
    if (
        not math.isfinite(args.stable_uptime_seconds)
        or args.stable_uptime_seconds <= 0
    ):
        parser.error("--stable-uptime-seconds must be positive")
    return args


def run(argv: list[str] | None = None) -> int:
    """Run the per-peer restart loop until launchd asks it to stop."""

    args = parse_args(argv)
    pid_file = Path(args.pid_file)
    pid_file.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    stopping_signal: int | None = None
    child: subprocess.Popen[bytes] | None = None

    def request_stop(signum: int, _frame: FrameType | None) -> None:
        nonlocal stopping_signal
        stopping_signal = signum
        if child is not None and child.poll() is None:
            try:
                child.send_signal(signum)
            except ProcessLookupError:
                pass

    signal.signal(signal.SIGTERM, request_stop)
    signal.signal(signal.SIGINT, request_stop)
    signal.signal(signal.SIGHUP, request_stop)

    backoff = args.initial_backoff_seconds
    while stopping_signal is None:
        try:
            require_runtime_identity(args)
        except (IdentityError, OSError) as exc:
            print(f"taira supervisor identity refusal: {exc}", file=sys.stderr, flush=True)
            return 78

        if stopping_signal is not None:
            break
        started = time.monotonic()
        try:
            child = subprocess.Popen(
                [args.binary, "--sora", "--config", args.config],
                cwd=args.workdir,
            )
        except OSError as exc:
            print(
                "taira validator spawn failed "
                f"error={exc!s} restart_in_seconds={backoff:.3f}",
                file=sys.stderr,
                flush=True,
            )
            deadline = time.monotonic() + backoff
            while stopping_signal is None:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    break
                time.sleep(min(remaining, 0.25))
            backoff = min(args.maximum_backoff_seconds, backoff * 2)
            continue
        if stopping_signal is not None:
            try:
                child.send_signal(stopping_signal)
            except ProcessLookupError:
                pass
        try:
            atomic_write_pid(pid_file, child.pid)
        except OSError:
            try:
                child.terminate()
            except ProcessLookupError:
                pass
            child.wait()
            raise
        print(f"taira validator started pid={child.pid}", flush=True)
        return_code = child.wait()
        uptime = time.monotonic() - started
        remove_owned_pid(pid_file, child.pid)
        child = None

        if stopping_signal is not None:
            print(
                f"taira validator stopped signal={stopping_signal} rc={return_code}",
                flush=True,
            )
            return 0

        if uptime >= args.stable_uptime_seconds:
            backoff = args.initial_backoff_seconds
        print(
            "taira validator exited "
            f"rc={return_code} uptime_seconds={uptime:.3f} "
            f"restart_in_seconds={backoff:.3f}",
            file=sys.stderr,
            flush=True,
        )
        deadline = time.monotonic() + backoff
        while stopping_signal is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                break
            time.sleep(min(remaining, 0.25))
        backoff = min(args.maximum_backoff_seconds, backoff * 2)
    return 0


if __name__ == "__main__":
    raise SystemExit(run())
