#!/usr/bin/env python3
"""Run one SCCP corridor phase and publish a bounded, secret-free transcript."""

from __future__ import annotations

import hashlib
import os
import stat
import subprocess
import sys
from collections.abc import Sequence

import sccp_release_common as common

MANIFEST_SCHEMA = "sccp-corridor-phase-log-v1"
RUNNER_FAILURE_STATUS = 125
_COPY_CHUNK_BYTES = 64 * 1024
_PHASE_LOG_LIMITS = {
    "rust-sccp": 256 * 1024 * 1024,
    "evidence-scripts": 64 * 1024 * 1024,
    "js-sdk": 64 * 1024 * 1024,
    "python-sdk": 64 * 1024 * 1024,
    "swift-sdk": 256 * 1024 * 1024,
    "kotlin-sdk": 256 * 1024 * 1024,
    "java-android": 256 * 1024 * 1024,
    "dotnet-sdk": 256 * 1024 * 1024,
    "contract-smoke": 64 * 1024 * 1024,
    "tvm-contract-smoke": 64 * 1024 * 1024,
    "core-admission": 256 * 1024 * 1024,
    "runtime-api": 256 * 1024 * 1024,
}


class PhaseLogError(RuntimeError):
    """A public-safe phase transcript publication failure."""


def _directory_flags() -> int:
    if not hasattr(os, "O_NOFOLLOW") or not hasattr(os, "O_DIRECTORY"):
        raise PhaseLogError(
            "required descriptor-relative filesystem controls are unavailable"
        )
    return os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)


def _file_flags() -> int:
    if not hasattr(os, "O_NOFOLLOW"):
        raise PhaseLogError(
            "required descriptor-relative filesystem controls are unavailable"
        )
    return (
        os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
    )


def _require_openat_support() -> None:
    required = (os.open, os.mkdir, os.stat, os.unlink)
    if any(operation not in os.supports_dir_fd for operation in required):
        raise PhaseLogError(
            "required descriptor-relative filesystem controls are unavailable"
        )


def _require_private_directory(descriptor: int) -> os.stat_result:
    metadata = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077
    ):
        raise PhaseLogError("the phase log directory must be owner-only")
    return metadata


def open_private_log_directory(path: str) -> int:
    """Create or open ``path`` without following any path-component symlink."""

    _require_openat_support()
    if not path or "\x00" in path:
        raise PhaseLogError("the phase log directory is invalid")
    common.reject_secret_material(os.fsencode(path), label="phase log directory path")
    absolute = os.path.isabs(path)
    components = [part for part in path.split(os.sep) if part not in ("", ".")]
    if not components or any(part == ".." for part in components):
        raise PhaseLogError("the phase log directory is invalid")

    descriptor = os.open(os.sep if absolute else ".", _directory_flags())
    try:
        for position, component in enumerate(components):
            last = position == len(components) - 1
            try:
                child = os.open(component, _directory_flags(), dir_fd=descriptor)
            except FileNotFoundError:
                if not last:
                    raise PhaseLogError(
                        "a phase log directory parent is unavailable"
                    ) from None
                try:
                    os.mkdir(component, mode=0o700, dir_fd=descriptor)
                except FileExistsError:
                    raise PhaseLogError(
                        "the phase log directory changed while opening"
                    ) from None
                except OSError:
                    raise PhaseLogError(
                        "the phase log directory could not be created safely"
                    ) from None
                try:
                    child = os.open(component, _directory_flags(), dir_fd=descriptor)
                except OSError:
                    raise PhaseLogError(
                        "the phase log directory could not be opened safely"
                    ) from None
                os.fchmod(child, 0o700)
                os.fsync(descriptor)
            except OSError:
                raise PhaseLogError(
                    "the phase log directory could not be opened safely"
                ) from None
            os.close(descriptor)
            descriptor = child
        _require_private_directory(descriptor)
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _open_new_private_file(directory_descriptor: int, name: str) -> int:
    try:
        descriptor = os.open(name, _file_flags(), 0o600, dir_fd=directory_descriptor)
    except FileExistsError:
        raise PhaseLogError(
            "phase log publication never overwrites existing output"
        ) from None
    except OSError:
        raise PhaseLogError("phase log output could not be created safely") from None
    opened = os.fstat(descriptor)
    identity = (opened.st_dev, opened.st_ino)
    try:
        os.fchmod(descriptor, 0o600)
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or metadata.st_mode & 0o077
        ):
            raise PhaseLogError("phase log output is not a private direct regular file")
        return descriptor
    except BaseException:
        os.close(descriptor)
        _unlink_if_inode_matches(directory_descriptor, name, identity)
        raise


def _write_all(descriptor: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        try:
            written = os.write(descriptor, view)
        except InterruptedError:
            continue
        except OSError:
            raise PhaseLogError(
                "phase log output could not be written safely"
            ) from None
        if written <= 0:
            raise PhaseLogError("phase log output write made no progress")
        view = view[written:]


def _unlink_if_inode_matches(
    directory_descriptor: int,
    name: str,
    identity: tuple[int, int],
) -> None:
    try:
        current = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
        if (current.st_dev, current.st_ino) == identity:
            os.unlink(name, dir_fd=directory_descriptor)
            os.fsync(directory_descriptor)
    except OSError:
        return


def _readback(
    directory_descriptor: int,
    descriptor: int,
    name: str,
    expected_size: int,
) -> tuple[bytes, str, tuple[int, int]]:
    before = os.fstat(descriptor)
    identity = (before.st_dev, before.st_ino)
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size != expected_size
        or before.st_uid != os.geteuid()
        or before.st_mode & 0o077
    ):
        raise PhaseLogError("phase log inode changed before readback")
    try:
        named = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
    except OSError:
        raise PhaseLogError("phase log name changed before readback") from None
    if (named.st_dev, named.st_ino) != identity:
        raise PhaseLogError("phase log name changed before readback")

    os.lseek(descriptor, 0, os.SEEK_SET)
    chunks: list[bytes] = []
    digest = hashlib.sha256()
    observed = 0
    while True:
        try:
            chunk = os.read(descriptor, _COPY_CHUNK_BYTES)
        except InterruptedError:
            continue
        if not chunk:
            break
        observed += len(chunk)
        if observed > expected_size:
            raise PhaseLogError("phase log grew during readback")
        chunks.append(chunk)
        digest.update(chunk)
    after = os.fstat(descriptor)
    if observed != expected_size or (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_nlink,
    ) != (before.st_dev, before.st_ino, before.st_size, before.st_nlink):
        raise PhaseLogError("phase log inode changed during readback")
    return b"".join(chunks), digest.hexdigest(), identity


def _command_hash(command: Sequence[str]) -> str:
    digest = hashlib.sha256(b"iroha:sccp:corridor-phase-command:v1\x00")
    for argument in command:
        encoded = os.fsencode(argument)
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
    return digest.hexdigest()


def _shell_status(return_code: int) -> tuple[int, int | None]:
    if return_code >= 0:
        return min(return_code, 255), None
    signal_number = -return_code
    return min(128 + signal_number, 255), signal_number


def _write_manifest(
    directory_descriptor: int,
    name: str,
    manifest: dict[str, object],
) -> None:
    data = common.canonical_json_file_bytes(manifest)
    common.reject_secret_material(data, label="phase log manifest")
    descriptor = _open_new_private_file(directory_descriptor, name)
    identity = (os.fstat(descriptor).st_dev, os.fstat(descriptor).st_ino)
    try:
        _write_all(descriptor, data)
        os.fsync(descriptor)
        readback, digest, _ = _readback(
            directory_descriptor,
            descriptor,
            name,
            len(data),
        )
        if readback != data or digest != hashlib.sha256(data).hexdigest():
            raise PhaseLogError("phase log manifest failed inode readback")
    except BaseException:
        _unlink_if_inode_matches(directory_descriptor, name, identity)
        raise
    finally:
        os.close(descriptor)
    os.fsync(directory_descriptor)


def run_phase(
    log_directory: str,
    phase: str,
    command: Sequence[str],
    *,
    maximum_bytes: int | None = None,
) -> int:
    """Run a known phase, publish its log and manifest, and return its status."""

    if phase not in _PHASE_LOG_LIMITS or not command:
        raise PhaseLogError("the phase log invocation is invalid")
    if maximum_bytes is None:
        maximum_bytes = _PHASE_LOG_LIMITS[phase]
    if type(maximum_bytes) is not int or maximum_bytes <= 0:
        raise PhaseLogError("the phase log byte limit is invalid")

    directory_descriptor = open_private_log_directory(log_directory)
    log_name = f"{phase}.log"
    manifest_name = f"{phase}.manifest.json"
    descriptor = -1
    log_identity: tuple[int, int] | None = None
    try:
        _require_private_directory(directory_descriptor)
        descriptor = _open_new_private_file(directory_descriptor, log_name)
        opened = os.fstat(descriptor)
        log_identity = (opened.st_dev, opened.st_ino)
        try:
            process = subprocess.Popen(
                list(command),
                stdin=None,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                close_fds=True,
                bufsize=0,
            )
        except OSError:
            raise PhaseLogError("the phase command could not be started") from None
        if process.stdout is None:
            raise PhaseLogError("the phase command output pipe is unavailable")

        observed = 0
        streamed_digest = hashlib.sha256()
        overflow = False
        write_failed = False
        while True:
            chunk = process.stdout.read(_COPY_CHUNK_BYTES)
            if not chunk:
                break
            observed += len(chunk)
            if observed > maximum_bytes:
                overflow = True
                continue
            if not write_failed:
                try:
                    _write_all(descriptor, chunk)
                    streamed_digest.update(chunk)
                except PhaseLogError:
                    # Drain the child without signalling or interrupting it;
                    # production phases can own long-running Cargo processes.
                    write_failed = True
        process.stdout.close()
        return_code = process.wait()
        if write_failed:
            raise PhaseLogError("the phase transcript could not be written safely")
        if overflow:
            raise PhaseLogError("the phase transcript exceeded its declared byte limit")

        os.fsync(descriptor)
        os.fsync(directory_descriptor)
        transcript, transcript_hash, _ = _readback(
            directory_descriptor,
            descriptor,
            log_name,
            observed,
        )
        if transcript_hash != streamed_digest.hexdigest():
            raise PhaseLogError(
                "phase transcript differs from the captured command stream"
            )
        common.reject_secret_material(transcript, label="phase transcript")
        status, signal_number = _shell_status(return_code)
        manifest = {
            "schema": MANIFEST_SCHEMA,
            "phase": phase,
            "log_file": log_name,
            "log_sha256_hex": transcript_hash,
            "size_bytes": observed,
            "maximum_size_bytes": maximum_bytes,
            "command_sha256_hex": _command_hash(command),
            "exit_status": status,
            "terminating_signal": signal_number,
        }

        # Output is withheld until the recursive secret scan succeeds. This
        # preserves tee-like terminal output without disclosing rejected bytes.
        sys.stdout.buffer.write(transcript)
        sys.stdout.buffer.flush()
        _write_manifest(directory_descriptor, manifest_name, manifest)
        return status
    except BaseException:
        if log_identity is not None:
            _unlink_if_inode_matches(directory_descriptor, log_name, log_identity)
        raise
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        os.close(directory_descriptor)


def _usage() -> str:
    return (
        "Usage: scripts/sccp_phase_log_runner.py --log-dir DIR "
        "--phase NAME -- COMMAND [ARG ...]"
    )


def _parse_arguments(arguments: Sequence[str]) -> tuple[str, str, tuple[str, ...]]:
    if tuple(arguments) in (("-h",), ("--help",)):
        print(_usage())
        raise SystemExit(0)
    if (
        len(arguments) < 6
        or arguments[0] != "--log-dir"
        or arguments[2] != "--phase"
        or arguments[4] != "--"
    ):
        raise PhaseLogError("the phase log invocation is invalid")
    return arguments[1], arguments[3], tuple(arguments[5:])


def main(arguments: Sequence[str] | None = None) -> int:
    """CLI entry point with secret-free diagnostics."""

    try:
        log_directory, phase, command = _parse_arguments(
            tuple(sys.argv[1:] if arguments is None else arguments)
        )
        return run_phase(log_directory, phase, command)
    except (PhaseLogError, common.SccpReleaseError) as error:
        print(f"SCCP phase log runner failed: {error}", file=sys.stderr)
        return RUNNER_FAILURE_STATUS
    except (OSError, ValueError, TypeError):
        print("SCCP phase log runner failed safely", file=sys.stderr)
        return RUNNER_FAILURE_STATUS


if __name__ == "__main__":
    raise SystemExit(main())
