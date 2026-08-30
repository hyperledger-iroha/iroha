#!/usr/bin/env python3
"""Fail-closed macOS supervisor for the canonical ``iroha3d_taira`` launcher.

The daemon consumes (overwrites and truncates) the file inherited at descriptor
198.  A restartable service must therefore keep a distinct persistent source
and stage a new single-use launch copy before every exec.  This program is the
small, source-controlled LaunchAgent boundary for that operation.

The no-bind ``check-config`` corridor unlinks its empty staging inode before
writing signer bytes, then supplies that anonymous file at FD 198.  Supervision
wipes and truncates it after success, rejection, timeout, or spawn failure.  It
never consumes or modifies the persistent source.

It never accepts signer material through an environment variable and never
prints signer bytes, their digest, or their path contents.  Release tooling is
expected to seal every argument in the LaunchAgent plist.
"""

from __future__ import annotations

import argparse
import fcntl
import math
import os
import re
import stat
import subprocess
import sys
from pathlib import Path
from typing import NoReturn, Sequence


TAIRA_RUNTIME_SIGNER_FD = 198
TAIRA_RUNTIME_SIGNER_BYTES = 71
TAIRA_RUNTIME_SIGNER_MODE = 0o600
TAIRA_PRIVATE_DIRECTORY_MODE = 0o700
TAIRA_CHECK_CONFIG_TIMEOUT_SECONDS = 45.0

_BLAKE3_HEX_RE = re.compile(r"[0-9a-fA-F]{64}\Z")


class SupervisorError(RuntimeError):
    """A public, payload-free supervisor failure."""


def _absolute(path: str, label: str) -> Path:
    value = Path(path)
    if not value.is_absolute():
        raise SupervisorError(f"{label} must be an absolute path")
    if "\0" in path or path.startswith("//") or os.path.normpath(path) != path:
        raise SupervisorError(f"{label} must use a canonical absolute path")
    return value


def _reject_symlink_components(path: Path, label: str) -> None:
    """Reject a symlink at the target or in any existing path component."""

    current = Path(path.anchor)
    for component in path.parts[1:]:
        current /= component
        try:
            metadata = os.lstat(current)
        except FileNotFoundError:
            # Missing leaf components are handled by the caller.  No later
            # component can exist without this parent.
            return
        if stat.S_ISLNK(metadata.st_mode):
            raise SupervisorError(f"{label} contains a symlink component")


def _stable_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_uid,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _validate_private_directory(path: Path) -> None:
    _reject_symlink_components(path, "Taira private runtime directory")
    try:
        metadata = os.lstat(path)
    except FileNotFoundError as error:
        raise SupervisorError("Taira private runtime directory is missing") from error
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != TAIRA_PRIVATE_DIRECTORY_MODE
    ):
        raise SupervisorError(
            "Taira private runtime directory must be an owner-0700 directory"
        )


def _validate_signer_metadata(metadata: os.stat_result, label: str) -> None:
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != TAIRA_RUNTIME_SIGNER_MODE
        or metadata.st_nlink != 1
        or metadata.st_size != TAIRA_RUNTIME_SIGNER_BYTES
    ):
        raise SupervisorError(
            f"{label} must be an owner-0600, single-link, "
            f"{TAIRA_RUNTIME_SIGNER_BYTES}-byte regular file"
        )


def _open_source(path: Path) -> tuple[int, os.stat_result]:
    _reject_symlink_components(path, "persistent Taira runtime signer")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise SupervisorError("persistent Taira runtime signer is unavailable") from error
    try:
        metadata = os.fstat(descriptor)
        _validate_signer_metadata(metadata, "persistent Taira runtime signer")
        if _stable_identity(os.lstat(path)) != _stable_identity(metadata):
            raise SupervisorError("persistent Taira runtime signer path changed while opening")
        return descriptor, metadata
    except BaseException:
        os.close(descriptor)
        raise


def _relocate_reserved_source_descriptor(descriptor: int) -> int:
    """Keep a source open from colliding with the inherited launch descriptor."""

    if descriptor != TAIRA_RUNTIME_SIGNER_FD:
        return descriptor
    duplicate_command = getattr(fcntl, "F_DUPFD_CLOEXEC", fcntl.F_DUPFD)
    try:
        relocated = fcntl.fcntl(
            descriptor, duplicate_command, TAIRA_RUNTIME_SIGNER_FD + 1
        )
    except OSError as error:
        os.close(descriptor)
        raise SupervisorError("cannot relocate reserved Taira signer descriptor") from error
    if duplicate_command == fcntl.F_DUPFD:
        os.set_inheritable(relocated, False)
    os.close(descriptor)
    return relocated


def validate_inputs(source_path: Path, launch_path: Path) -> None:
    """Validate metadata only; do not read or copy signer bytes."""

    if source_path == launch_path:
        raise SupervisorError("persistent signer and FD198 launch paths must be distinct")
    _validate_private_directory(source_path.parent)
    _validate_private_directory(launch_path.parent)
    descriptor, source_metadata = _open_source(source_path)
    os.close(descriptor)
    try:
        stale = os.lstat(launch_path)
    except FileNotFoundError:
        return
    if (
        not stat.S_ISREG(stale.st_mode)
        or stale.st_uid != os.geteuid()
        or stat.S_IMODE(stale.st_mode) != TAIRA_RUNTIME_SIGNER_MODE
        or stale.st_nlink != 1
        or stale.st_size not in (0, TAIRA_RUNTIME_SIGNER_BYTES)
        or (stale.st_dev, stale.st_ino)
        == (source_metadata.st_dev, source_metadata.st_ino)
    ):
        raise SupervisorError("untrusted stale Taira FD198 launch file")


def _read_exact_signer(descriptor: int) -> bytearray:
    secret = bytearray(TAIRA_RUNTIME_SIGNER_BYTES)
    view = memoryview(secret)
    offset = 0
    try:
        while offset < len(secret):
            count = os.readv(descriptor, [view[offset:]])
            if count == 0:
                raise SupervisorError("short persistent Taira runtime signer")
            offset += count
        if os.read(descriptor, 1):
            raise SupervisorError("oversized persistent Taira runtime signer")
        return secret
    except BaseException:
        secret[:] = b"\0" * len(secret)
        raise
    finally:
        view.release()


def _write_all(descriptor: int, payload: bytearray) -> None:
    view = memoryview(payload)
    offset = 0
    try:
        while offset < len(payload):
            count = os.write(descriptor, view[offset:])
            if count == 0:
                raise SupervisorError("short Taira FD198 launch write")
            offset += count
    finally:
        view.release()


def _wipe_and_unlink_launch_copy(launch_path: Path, descriptor: int) -> None:
    """Destroy a staged signer copy while retaining no linked secret inode."""

    cleanup_failed = False
    zeros = bytearray(TAIRA_RUNTIME_SIGNER_BYTES)
    try:
        try:
            os.lseek(descriptor, 0, os.SEEK_SET)
            _write_all(descriptor, zeros)
            os.fsync(descriptor)
            os.ftruncate(descriptor, 0)
            os.fsync(descriptor)
        except (OSError, SupervisorError):
            cleanup_failed = True

        try:
            descriptor_metadata = os.fstat(descriptor)
            try:
                path_metadata = os.lstat(launch_path)
            except FileNotFoundError:
                if descriptor_metadata.st_nlink != 0:
                    cleanup_failed = True
            else:
                if (descriptor_metadata.st_dev, descriptor_metadata.st_ino) != (
                    path_metadata.st_dev,
                    path_metadata.st_ino,
                ):
                    cleanup_failed = True
                else:
                    os.unlink(launch_path)
                    directory_descriptor = os.open(
                        launch_path.parent,
                        os.O_RDONLY
                        | getattr(os, "O_DIRECTORY", 0)
                        | getattr(os, "O_CLOEXEC", 0)
                        | getattr(os, "O_NOFOLLOW", 0),
                    )
                    try:
                        os.fsync(directory_descriptor)
                    finally:
                        os.close(directory_descriptor)
        except OSError:
            cleanup_failed = True
    finally:
        zeros[:] = b"\0" * len(zeros)
        try:
            os.close(descriptor)
        except OSError:
            cleanup_failed = True
    if cleanup_failed:
        raise SupervisorError("cannot destroy disposable Taira FD198 launch copy")


def stage_fd198(
    source_path: Path, launch_path: Path, *, unlink_before_write: bool = False
) -> int:
    """Stage one consumable launch file and return it as inherited FD 198."""

    validate_inputs(source_path, launch_path)
    source_descriptor, source_before = _open_source(source_path)
    source_descriptor = _relocate_reserved_source_descriptor(source_descriptor)
    launch_descriptor: int | None = None
    launch_ready = False
    reserved_duplicate = False
    secret = bytearray()
    try:
        try:
            stale = os.lstat(launch_path)
        except FileNotFoundError:
            stale = None
        if stale is not None:
            # ``validate_inputs`` already proved this is an owned, single-link,
            # zero-or-71-byte regular file distinct from the persistent source.
            stale_flags = (
                os.O_RDWR
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0)
            )
            try:
                stale_descriptor = os.open(launch_path, stale_flags)
            except OSError as error:
                raise SupervisorError(
                    "stale Taira FD198 launch file changed before replacement"
                ) from error
            if _stable_identity(os.fstat(stale_descriptor)) != _stable_identity(stale):
                os.close(stale_descriptor)
                raise SupervisorError(
                    "stale Taira FD198 launch file changed before replacement"
                )
            _wipe_and_unlink_launch_copy(launch_path, stale_descriptor)

        flags = (
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        launch_descriptor = os.open(launch_path, flags, TAIRA_RUNTIME_SIGNER_MODE)
        os.fchmod(launch_descriptor, TAIRA_RUNTIME_SIGNER_MODE)
        secret = _read_exact_signer(source_descriptor)
        source_after = os.fstat(source_descriptor)
        if _stable_identity(source_before) != _stable_identity(source_after):
            raise SupervisorError("persistent Taira runtime signer changed while staging")
        try:
            source_path_after = os.lstat(source_path)
        except FileNotFoundError as error:
            raise SupervisorError(
                "persistent Taira runtime signer path changed while staging"
            ) from error
        if _stable_identity(source_before) != _stable_identity(source_path_after):
            raise SupervisorError(
                "persistent Taira runtime signer path changed while staging"
            )

        if unlink_before_write:
            empty_metadata = os.fstat(launch_descriptor)
            if (
                not stat.S_ISREG(empty_metadata.st_mode)
                or empty_metadata.st_uid != os.geteuid()
                or stat.S_IMODE(empty_metadata.st_mode) != TAIRA_RUNTIME_SIGNER_MODE
                or empty_metadata.st_nlink != 1
                or empty_metadata.st_size != 0
                or _stable_identity(os.lstat(launch_path))
                != _stable_identity(empty_metadata)
            ):
                raise SupervisorError("empty Taira FD198 staging path changed")
            os.unlink(launch_path)
            directory_descriptor = os.open(
                launch_path.parent,
                os.O_RDONLY
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
            )
            try:
                os.fsync(directory_descriptor)
            finally:
                os.close(directory_descriptor)
            if os.fstat(launch_descriptor).st_nlink != 0:
                raise SupervisorError("empty Taira FD198 staging inode remains linked")

        _write_all(launch_descriptor, secret)
        os.fsync(launch_descriptor)
        os.lseek(launch_descriptor, 0, os.SEEK_SET)
        launch_metadata = os.fstat(launch_descriptor)
        if unlink_before_write:
            if (
                not stat.S_ISREG(launch_metadata.st_mode)
                or launch_metadata.st_uid != os.geteuid()
                or stat.S_IMODE(launch_metadata.st_mode) != TAIRA_RUNTIME_SIGNER_MODE
                or launch_metadata.st_nlink != 0
                or launch_metadata.st_size != TAIRA_RUNTIME_SIGNER_BYTES
            ):
                raise SupervisorError(
                    "anonymous Taira FD198 launch file has unsafe metadata"
                )
            try:
                os.lstat(launch_path)
            except FileNotFoundError:
                pass
            else:
                raise SupervisorError("anonymous Taira FD198 launch path was recreated")
        else:
            _validate_signer_metadata(
                launch_metadata, "staged Taira FD198 launch file"
            )
            try:
                launch_path_metadata = os.lstat(launch_path)
            except FileNotFoundError as error:
                raise SupervisorError("staged Taira FD198 launch path changed") from error
            if _stable_identity(launch_metadata) != _stable_identity(
                launch_path_metadata
            ):
                raise SupervisorError("staged Taira FD198 launch path changed")
        if (source_before.st_dev, source_before.st_ino) == (
            launch_metadata.st_dev,
            launch_metadata.st_ino,
        ):
            raise SupervisorError("FD198 launch file aliases the persistent signer")

        directory_descriptor = os.open(
            launch_path.parent,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)

        if launch_descriptor == TAIRA_RUNTIME_SIGNER_FD:
            os.set_inheritable(launch_descriptor, True)
        else:
            os.dup2(launch_descriptor, TAIRA_RUNTIME_SIGNER_FD, inheritable=True)
            reserved_duplicate = True
        os.lseek(TAIRA_RUNTIME_SIGNER_FD, 0, os.SEEK_SET)
        launch_ready = True
        return TAIRA_RUNTIME_SIGNER_FD
    finally:
        if secret:
            secret[:] = b"\0" * len(secret)
        os.close(source_descriptor)
        if not launch_ready and launch_descriptor is not None:
            try:
                _wipe_and_unlink_launch_copy(launch_path, launch_descriptor)
            finally:
                if reserved_duplicate:
                    try:
                        os.close(TAIRA_RUNTIME_SIGNER_FD)
                    except OSError:
                        pass
        elif (
            launch_descriptor is not None
            and launch_descriptor != TAIRA_RUNTIME_SIGNER_FD
        ):
            os.close(launch_descriptor)


def _validate_public_input(path: Path, label: str, executable: bool = False) -> None:
    _reject_symlink_components(path, label)
    try:
        metadata = os.lstat(path)
    except FileNotFoundError as error:
        raise SupervisorError(f"{label} is missing") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise SupervisorError(f"{label} must be a regular file")
    if executable and not os.access(path, os.X_OK):
        raise SupervisorError(f"{label} is not executable")


def run_daemon(args: argparse.Namespace) -> NoReturn:
    binary = _absolute(args.binary, "iroha3d_taira binary")
    config = _absolute(args.config, "Taira validator config")
    genesis = _absolute(args.genesis_manifest, "Taira genesis manifest")
    source = _absolute(args.signer_source, "persistent Taira runtime signer")
    launch = _absolute(args.signer_launch, "Taira FD198 launch file")
    _validate_public_input(binary, "iroha3d_taira binary", executable=True)
    _validate_public_input(config, "Taira validator config")
    _validate_public_input(genesis, "Taira genesis manifest")
    descriptor = stage_fd198(source, launch)
    argv = [
        str(binary),
        "--sora",
        "--config",
        str(config),
        "--genesis-manifest-json",
        str(genesis),
    ]
    try:
        os.execve(binary, argv, os.environ.copy())
    except OSError as error:
        _wipe_and_unlink_launch_copy(launch, descriptor)
        raise SupervisorError("cannot execute the canonical iroha3d_taira binary") from error


def _check_config_timeout(value: str) -> float:
    try:
        timeout = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("timeout must be a finite number") from error
    if not math.isfinite(timeout) or timeout <= 0 or timeout > 300:
        raise argparse.ArgumentTypeError("timeout must be greater than 0 and at most 300")
    return timeout


def run_check_config(args: argparse.Namespace) -> int:
    """Run no-bind validation with a disposable inherited signer copy."""

    binary = _absolute(args.binary, "iroha3d_taira binary")
    config = _absolute(args.config, "Taira validator config")
    genesis = _absolute(args.genesis_manifest, "Taira genesis manifest")
    source = _absolute(args.signer_source, "persistent Taira runtime signer")
    launch = _absolute(args.signer_launch, "Taira FD198 launch file")
    _validate_public_input(binary, "iroha3d_taira binary", executable=True)
    _validate_public_input(config, "Taira validator config")
    _validate_public_input(genesis, "Taira genesis manifest")
    if args.config_blake3 is not None and not _BLAKE3_HEX_RE.fullmatch(
        args.config_blake3
    ):
        raise SupervisorError("Taira validator config BLAKE3 must be exactly 64 hex digits")

    descriptor = stage_fd198(source, launch, unlink_before_write=True)
    argv = [
        str(binary),
        "--sora",
        "--check-config",
        "--config",
        str(config),
    ]
    if args.config_blake3 is not None:
        argv.extend(("--config-blake3", args.config_blake3.lower()))
    argv.extend(("--genesis-manifest-json", str(genesis)))
    try:
        try:
            result = subprocess.run(
                argv,
                check=False,
                close_fds=True,
                env=os.environ.copy(),
                pass_fds=(descriptor,),
                timeout=args.timeout_seconds,
            )
        except subprocess.TimeoutExpired as error:
            raise SupervisorError("canonical iroha3d_taira config check timed out") from error
        except OSError as error:
            raise SupervisorError(
                "cannot execute the canonical iroha3d_taira config check"
            ) from error
    finally:
        _wipe_and_unlink_launch_copy(launch, descriptor)
    if result.returncode < 0:
        return 128 - result.returncode
    return result.returncode


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("validate", "check-config", "run"):
        command = subparsers.add_parser(name)
        launches_binary = name in ("check-config", "run")
        command.add_argument("--binary", required=launches_binary)
        command.add_argument("--config", required=launches_binary)
        command.add_argument("--genesis-manifest", required=launches_binary)
        command.add_argument("--signer-source", required=True)
        command.add_argument("--signer-launch", required=True)
        if name == "check-config":
            command.add_argument("--config-blake3")
            command.add_argument(
                "--timeout-seconds",
                type=_check_config_timeout,
                default=TAIRA_CHECK_CONFIG_TIMEOUT_SECONDS,
            )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "validate":
            validate_inputs(
                _absolute(args.signer_source, "persistent Taira runtime signer"),
                _absolute(args.signer_launch, "Taira FD198 launch file"),
            )
            return 0
        if args.command == "check-config":
            return run_check_config(args)
        run_daemon(args)
    except SupervisorError as error:
        print(f"taira-fd198-supervisor: {error}", file=sys.stderr)
        return 1
    except OSError:
        print(
            "taira-fd198-supervisor: operating system refused the sealed request",
            file=sys.stderr,
        )
        return 1
    raise AssertionError("unreachable")


if __name__ == "__main__":
    raise SystemExit(main())
