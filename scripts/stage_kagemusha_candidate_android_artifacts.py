#!/usr/bin/env python3
"""Stream one authenticated Kagemusha V4 artifact set into app-private Android storage."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import shlex
import stat
import subprocess
import sys
from typing import Any, Mapping, Sequence

sys.path.insert(0, os.fspath(Path(__file__).resolve().parent))
from check_android_device_lab_slot import (
    KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4,
    validate_kagemusha_candidate_stage_manifest_v1,
)


PACKAGE = "org.hyperledger.iroha.sdk.kagemusha.candidate.lab"
REMOTE_BASE = "no_backup"
REMOTE_ROOT = "no_backup/kagemusha-candidate-artifacts-v1"
REMOTE_BINDING_FILE = "artifact-set-binding-v1.txt"
REMOTE_BINDING_SCHEMA = "iroha.kagemusha.android_candidate_artifact_set.v1"
MAX_ARTIFACT_BYTES = 5 * 1024 * 1024 * 1024
ARTIFACT_SPOOL_RESERVE_BYTES = 1024 * 1024 * 1024
MAX_REMOTE_METADATA_BYTES = 64 * 1024
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
SERIAL_RE = re.compile(r"[A-Za-z0-9._:+-]+")
SAFE_REMOTE_PATH_RE = re.compile(r"[A-Za-z0-9._/-]+")


class StageError(RuntimeError):
    """Raised when an artifact set cannot be staged without weakening its binding."""


@dataclass(frozen=True)
class ArtifactEntry:
    """One exact framed artifact declared by the authenticated stage manifest."""

    name: str
    path: Path
    size_bytes: int
    sha256: str


def _nonzero_sha256(value: str, label: str) -> str:
    if SHA256_RE.fullmatch(value) is None or value == "0" * 64:
        raise StageError(f"{label} must be non-zero lowercase SHA-256")
    return value


def _canonical_regular_executable(path: Path, label: str) -> Path:
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as error:
        raise StageError(f"{label} is unavailable") from error
    if not path.is_absolute() or resolved != path:
        raise StageError(f"{label} must be one canonical absolute path")
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise StageError(f"{label} must be one singly-linked regular file")
    if not os.access(path, os.X_OK):
        raise StageError(f"{label} must be executable")
    return path


def _canonical_stage_root(path: Path) -> Path:
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as error:
        raise StageError("--evidence-root is unavailable") from error
    if not path.is_absolute() or resolved != path:
        raise StageError("--evidence-root must be one canonical absolute directory")
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid not in {0, os.geteuid()}
        or metadata.st_mode & 0o022
    ):
        raise StageError("--evidence-root must be one trusted real directory")
    return path


def _validate_source_parent_chain(stage_root: Path) -> None:
    """Reject redirects or writable foreign parents below the authenticated root."""

    current = stage_root
    try:
        for component in ("evidence", "candidate", "artifacts"):
            current /= component
            metadata = current.lstat()
            if not stat.S_ISDIR(metadata.st_mode) or current.resolve(strict=True) != current:
                raise StageError("candidate artifact source parents must be real directories")
            if metadata.st_uid not in {0, os.geteuid()} or metadata.st_mode & 0o022:
                raise StageError(
                    "candidate artifact source parents must not be writable by others"
                )
    except OSError as error:
        raise StageError("candidate artifact source parents could not be authenticated") from error


def _artifact_inventory(
    stage_root: Path,
    manifest: Mapping[str, Any],
) -> tuple[ArtifactEntry, ...]:
    raw_entries = manifest.get("entries")
    if not isinstance(raw_entries, list):
        raise StageError("candidate stage manifest entries are absent")
    by_path: dict[str, Mapping[str, Any]] = {}
    for raw_entry in raw_entries:
        if not isinstance(raw_entry, dict) or not isinstance(raw_entry.get("path"), str):
            raise StageError("candidate stage manifest contains a malformed entry")
        relative = raw_entry["path"]
        if relative in by_path:
            raise StageError("candidate stage manifest repeats an artifact path")
        by_path[relative] = raw_entry

    artifact_prefix = "evidence/candidate/artifacts/"
    expected_artifact_paths = {
        f"{artifact_prefix}{name}"
        for name in KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4
    }
    observed_artifact_paths = {
        relative for relative in by_path if relative.startswith(artifact_prefix)
    }
    if observed_artifact_paths != expected_artifact_paths:
        raise StageError("candidate stage manifest artifact catalog is missing or extra")

    result: list[ArtifactEntry] = []
    for name in KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4:
        relative = f"evidence/candidate/artifacts/{name}"
        entry = by_path.get(relative)
        if entry is None:
            raise StageError(f"candidate stage manifest omits {relative}")
        size = entry.get("size_bytes")
        digest = entry.get("sha256")
        if (
            isinstance(size, bool)
            or not isinstance(size, int)
            or size <= 0
            or size > MAX_ARTIFACT_BYTES
        ):
            raise StageError(f"candidate artifact size is outside the V4 corridor: {name}")
        if not isinstance(digest, str):
            raise StageError(f"candidate artifact digest is absent: {name}")
        result.append(
            ArtifactEntry(
                name=name,
                path=stage_root / relative,
                size_bytes=size,
                sha256=_nonzero_sha256(digest, f"candidate artifact {name} digest"),
            )
        )
    return tuple(result)


def _remote_command(
    adb_prefix: Sequence[str],
    package: str,
    script: str,
) -> list[str]:
    if package != PACKAGE:
        raise StageError("the external artifact corridor accepts only the candidate lab package")
    remote = f"run-as {package} sh -c {shlex.quote(script)}"
    return [*adb_prefix, "shell", "-T", remote]


def _run_remote(
    adb_prefix: Sequence[str],
    package: str,
    script: str,
    *,
    input_bytes: bytes | None = None,
) -> None:
    completed = subprocess.run(
        _remote_command(adb_prefix, package, script),
        input=input_bytes,
        stdout=subprocess.DEVNULL,
        check=False,
    )
    if completed.returncode != 0:
        raise StageError("adb run-as command failed while staging the candidate artifact set")


def _capture_remote(
    adb_prefix: Sequence[str],
    package: str,
    script: str,
) -> str:
    completed = subprocess.run(
        _remote_command(adb_prefix, package, script),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise StageError("adb run-as metadata check failed for the candidate artifact set")
    if len(completed.stdout) > MAX_REMOTE_METADATA_BYTES:
        raise StageError("adb run-as metadata output exceeded its byte limit")
    try:
        return completed.stdout.decode("ascii").replace("\r", "")
    except UnicodeDecodeError as error:
        raise StageError("adb run-as metadata output was not ASCII") from error


def _device_available_bytes(adb_prefix: Sequence[str], package: str) -> int:
    lines = [
        line.split()
        for line in _capture_remote(adb_prefix, package, "df -Pk .").splitlines()
        if line.strip()
    ]
    if len(lines) < 2 or len(lines[-1]) < 4 or not lines[-1][3].isdigit():
        raise StageError("device free-space report is not canonical POSIX df output")
    return int(lines[-1][3]) * 1024


def _remote_artifact_measurement(
    adb_prefix: Sequence[str],
    package: str,
    remote_path: str,
) -> tuple[int, str]:
    remote_path = _safe_remote_path(remote_path)
    lines = [
        line.strip()
        for line in _capture_remote(
            adb_prefix,
            package,
            (
                f"test -f {remote_path} && test ! -L {remote_path} && "
                f"stat -c %s:%u:%a:%h {remote_path} && id -u && "
                f"sha256sum {remote_path}"
            ),
        ).splitlines()
        if line.strip()
    ]
    metadata = lines[0].split(":") if lines else []
    if (
        len(lines) != 3
        or len(metadata) != 4
        or any(not field.isdigit() for field in metadata)
        or not lines[1].isdigit()
    ):
        raise StageError("on-device artifact size/digest output is malformed")
    size, owner, mode, links = metadata
    if owner != lines[1] or mode != "600" or links != "1":
        raise StageError("on-device artifact ownership, mode, or link count is unsafe")
    digest_fields = lines[2].split()
    if (
        len(digest_fields) != 2
        or SHA256_RE.fullmatch(digest_fields[0]) is None
        or digest_fields[1] != remote_path
    ):
        raise StageError("on-device artifact SHA-256 output is malformed")
    return int(size), digest_fields[0]


def _safe_remote_path(path: str) -> str:
    components = path.split("/")
    if (
        SAFE_REMOTE_PATH_RE.fullmatch(path) is None
        or path.startswith("/")
        or any(not component or component in {".", ".."} for component in components)
    ):
        raise StageError("refusing an unsafe app-private artifact path")
    return path


def _remote_directory_guard(
    paths: Sequence[str],
    *,
    create_missing: bool,
) -> str:
    """Build a fixed-path guard for app-owned, mode-0700, non-symlink directories."""

    commands = ["umask 077", "uid=$(id -u)"]
    for raw_path in paths:
        path = _safe_remote_path(raw_path)
        if create_missing:
            commands.append(
                f"if test -L {path}; then exit 40; "
                f"elif test -e {path}; then "
                f"test -d {path} && test \"$(stat -c %u {path})\" = \"$uid\" "
                f"&& test \"$(stat -c %a {path})\" = 700 || exit 41; "
                f"else mkdir {path} && chmod 700 {path} || exit 42; fi"
            )
        else:
            commands.append(
                f"test ! -L {path} && test -d {path} "
                f"&& test \"$(stat -c %u {path})\" = \"$uid\" "
                f"&& test \"$(stat -c %a {path})\" = 700 || exit 43"
            )
    return "; ".join(commands)


def _constrained_cleanup_script(candidate_parent: str, incoming: str) -> str:
    """Unlink only names the stager can create, then remove the empty incoming dir."""

    candidate_parent = _safe_remote_path(candidate_parent)
    incoming = _safe_remote_path(incoming)
    created_names = []
    for name in KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4:
        created_names.extend((f"{incoming}/{name}", f"{incoming}/.{name}.tmp"))
    created_names.extend(
        (
            f"{incoming}/{REMOTE_BINDING_FILE}",
            f"{incoming}/.{REMOTE_BINDING_FILE}.tmp",
        )
    )
    paths = " ".join(_safe_remote_path(path) for path in created_names)
    guard = _remote_directory_guard(
        (REMOTE_BASE, REMOTE_ROOT, candidate_parent, incoming),
        create_missing=False,
    )
    return f"{guard}; rm -f {paths}; rmdir {incoming}"


def _stream_artifact(
    adb_prefix: Sequence[str],
    package: str,
    entry: ArtifactEntry,
    incoming: str,
) -> None:
    try:
        source_metadata = entry.path.lstat()
        resolved_source = entry.path.resolve(strict=True)
    except OSError as error:
        raise StageError(f"candidate artifact source is unavailable: {entry.name}") from error
    if (
        resolved_source != entry.path
        or not stat.S_ISREG(source_metadata.st_mode)
        or source_metadata.st_nlink != 1
        or source_metadata.st_uid not in {0, os.geteuid()}
        or stat.S_IMODE(source_metadata.st_mode) != 0o600
        or source_metadata.st_size != entry.size_bytes
    ):
        raise StageError(f"candidate artifact metadata changed before transfer: {entry.name}")

    remote_final = _safe_remote_path(f"{incoming}/{entry.name}")
    remote_temporary = _safe_remote_path(f"{incoming}/.{entry.name}.tmp")
    remote_script = (
        f"umask 077; test ! -e {remote_temporary} && test ! -L {remote_temporary} && "
        f"test ! -e {remote_final} && test ! -L {remote_final} && "
        f"cat > {remote_temporary} && chmod 600 {remote_temporary} && "
        f"test -f {remote_temporary} && test ! -L {remote_temporary} && "
        f"mv {remote_temporary} {remote_final}"
    )
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(entry.path, flags)
    except OSError as error:
        raise StageError(
            f"candidate artifact could not be securely opened: {entry.name}"
        ) from error
    digest = hashlib.sha256()
    size = 0
    transfer_error: BaseException | None = None
    process: subprocess.Popen[bytes] | None = None
    try:
        opened = os.fstat(descriptor)
        identity = (source_metadata.st_dev, source_metadata.st_ino)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or (opened.st_dev, opened.st_ino) != identity
            or opened.st_size != entry.size_bytes
        ):
            raise StageError(f"candidate artifact changed while opening: {entry.name}")
        process = subprocess.Popen(
            _remote_command(adb_prefix, package, remote_script),
            stdin=subprocess.PIPE,
            stdout=subprocess.DEVNULL,
        )
        if process.stdin is None:
            raise StageError(f"candidate artifact transfer pipe is unavailable: {entry.name}")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            size += len(chunk)
            if size > entry.size_bytes:
                raise StageError(f"candidate artifact grew during transfer: {entry.name}")
            digest.update(chunk)
            process.stdin.write(chunk)
        final_opened = os.fstat(descriptor)
        final_path = entry.path.lstat()
        final_resolved = entry.path.resolve(strict=True)
        if (
            (final_opened.st_dev, final_opened.st_ino) != identity
            or (final_path.st_dev, final_path.st_ino) != identity
            or final_resolved != entry.path
            or not stat.S_ISREG(final_opened.st_mode)
            or not stat.S_ISREG(final_path.st_mode)
            or final_opened.st_nlink != 1
            or final_path.st_nlink != 1
            or final_opened.st_uid != source_metadata.st_uid
            or final_path.st_uid != source_metadata.st_uid
            or stat.S_IMODE(final_opened.st_mode) != 0o600
            or stat.S_IMODE(final_path.st_mode) != 0o600
            or final_opened.st_size != entry.size_bytes
            or final_path.st_size != entry.size_bytes
            or final_path.st_mtime_ns != source_metadata.st_mtime_ns
            or final_path.st_ctime_ns != source_metadata.st_ctime_ns
        ):
            raise StageError(f"candidate artifact changed during transfer: {entry.name}")
        if size != entry.size_bytes or digest.hexdigest() != entry.sha256:
            raise StageError(f"candidate artifact failed its catalog binding: {entry.name}")
    except BaseException as error:  # Ensure the remote cat always receives EOF.
        transfer_error = error
    finally:
        os.close(descriptor)
        if process is not None and process.stdin is not None:
            try:
                process.stdin.close()
            except BrokenPipeError as error:
                if transfer_error is None:
                    transfer_error = error
    if process is None:
        if isinstance(transfer_error, StageError):
            raise transfer_error
        raise StageError(f"candidate artifact transfer failed: {entry.name}") from transfer_error
    returncode = process.wait()
    if transfer_error is not None:
        if isinstance(transfer_error, StageError):
            raise transfer_error
        raise StageError(f"candidate artifact transfer failed: {entry.name}") from transfer_error
    if returncode != 0:
        raise StageError(f"adb run-as rejected candidate artifact: {entry.name}")


def _binding_bytes(candidate_sha256: str, stage_sha256: str) -> bytes:
    return (
        f"{REMOTE_BINDING_SCHEMA}\n"
        f"{candidate_sha256}\n"
        f"{stage_sha256}\n"
        f"{len(KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4)}\n"
    ).encode("ascii")


def _required_device_free_bytes(inventory: Sequence[ArtifactEntry]) -> int:
    """Reserve one published copy, one native spool, and fixed working headroom."""

    artifact_bytes = sum(entry.size_bytes for entry in inventory)
    return artifact_bytes * 2 + ARTIFACT_SPOOL_RESERVE_BYTES


def stage_artifacts(
    *,
    adb: Path,
    serial: str | None,
    stage_root: Path,
    candidate_sha256: str,
    stage_sha256: str,
    source_commit: str,
    source_tree_sha256: str,
) -> None:
    """Authenticate and atomically publish the exact external artifact set on-device."""

    adb = _canonical_regular_executable(adb, "--adb")
    stage_root = _canonical_stage_root(stage_root)
    _nonzero_sha256(candidate_sha256, "--candidate-sha256")
    _nonzero_sha256(stage_sha256, "--stage-sha256")
    _nonzero_sha256(source_tree_sha256, "--source-tree-sha256")
    if COMMIT_RE.fullmatch(source_commit) is None:
        raise StageError("--source-commit must be lowercase git hex")
    if serial is not None and (
        SERIAL_RE.fullmatch(serial) is None or serial.startswith("-")
    ):
        raise StageError("--serial is invalid")

    try:
        manifest = validate_kagemusha_candidate_stage_manifest_v1(
            stage_root,
            candidate_sha256=candidate_sha256,
            stage_sha256=stage_sha256,
            source_commit=source_commit,
            source_tree_sha256=source_tree_sha256,
            verify_entry_digests=False,
        )
    except (OSError, ValueError) as error:
        raise StageError(f"candidate stage catalog is invalid: {error}") from error
    inventory = _artifact_inventory(stage_root, manifest)
    _validate_source_parent_chain(stage_root)

    adb_prefix = [os.fspath(adb)]
    if serial is not None:
        adb_prefix += ["-s", serial]
    candidate_parent = _safe_remote_path(f"{REMOTE_ROOT}/{candidate_sha256}")
    incoming = _safe_remote_path(f"{candidate_parent}/.incoming-{stage_sha256}")
    final = _safe_remote_path(f"{candidate_parent}/{stage_sha256}")
    required_free_bytes = _required_device_free_bytes(inventory)
    available_bytes = _device_available_bytes(adb_prefix, PACKAGE)
    if available_bytes < required_free_bytes:
        raise StageError(
            "device app-data filesystem lacks space for the external set, native spool, "
            "and 1 GiB reserve"
        )
    parent_guard = _remote_directory_guard(
        (REMOTE_BASE, REMOTE_ROOT, candidate_parent),
        create_missing=True,
    )
    setup = (
        f"{parent_guard}; test ! -e {incoming} && test ! -L {incoming} && "
        f"test ! -e {final} && test ! -L {final} && "
        f"mkdir {incoming} && chmod 700 {incoming}"
    )
    published = False
    try:
        _run_remote(adb_prefix, PACKAGE, setup)
        for entry in inventory:
            _stream_artifact(adb_prefix, PACKAGE, entry, incoming)
        binding_temporary = _safe_remote_path(f"{incoming}/.{REMOTE_BINDING_FILE}.tmp")
        binding_final = _safe_remote_path(f"{incoming}/{REMOTE_BINDING_FILE}")
        binding_bytes = _binding_bytes(candidate_sha256, stage_sha256)
        _run_remote(
            adb_prefix,
            PACKAGE,
            (
                f"umask 077; test ! -e {binding_temporary} && "
                f"test ! -L {binding_temporary} && test ! -e {binding_final} && "
                f"test ! -L {binding_final} && "
                f"cat > {binding_temporary} && chmod 600 {binding_temporary} && "
                f"test -f {binding_temporary} && test ! -L {binding_temporary} && "
                f"mv {binding_temporary} {binding_final}"
            ),
            input_bytes=binding_bytes,
        )
        staged_names = {
            line.strip()
            for line in _capture_remote(
                adb_prefix,
                PACKAGE,
                f"ls -1A {incoming}",
            ).splitlines()
            if line.strip()
        }
        expected_names = {
            *KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4,
            REMOTE_BINDING_FILE,
        }
        if staged_names != expected_names:
            raise StageError("app-private incoming artifact inventory is missing or extra")
        for entry in inventory:
            measured_size, measured_sha256 = _remote_artifact_measurement(
                adb_prefix,
                PACKAGE,
                f"{incoming}/{entry.name}",
            )
            if measured_size != entry.size_bytes or measured_sha256 != entry.sha256:
                raise StageError(
                    f"on-device candidate artifact failed its catalog binding: {entry.name}"
                )
        binding_size, binding_sha256 = _remote_artifact_measurement(
            adb_prefix,
            PACKAGE,
            binding_final,
        )
        if (
            binding_size != len(binding_bytes)
            or binding_sha256 != hashlib.sha256(binding_bytes).hexdigest()
        ):
            raise StageError("on-device artifact-set binding changed before publication")
        publish_guard = _remote_directory_guard(
            (REMOTE_BASE, REMOTE_ROOT, candidate_parent, incoming),
            create_missing=False,
        )
        _run_remote(
            adb_prefix,
            PACKAGE,
            (
                f"{publish_guard}; test ! -e {final} && test ! -L {final} "
                f"&& mv {incoming} {final}"
            ),
        )
        published = True
    finally:
        if not published:
            try:
                _run_remote(
                    adb_prefix,
                    PACKAGE,
                    _constrained_cleanup_script(candidate_parent, incoming),
                )
            except StageError:
                pass


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--adb", required=True, type=Path)
    parser.add_argument("--serial")
    parser.add_argument("--evidence-root", required=True, type=Path)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--stage-sha256", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--source-tree-sha256", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        stage_artifacts(
            adb=args.adb,
            serial=args.serial,
            stage_root=args.evidence_root,
            candidate_sha256=args.candidate_sha256,
            stage_sha256=args.stage_sha256,
            source_commit=args.source_commit,
            source_tree_sha256=args.source_tree_sha256,
        )
    except StageError as error:
        print(f"[kagemusha-candidate-artifact-stage] ERROR: {error}", file=sys.stderr)
        return 1
    print("[kagemusha-candidate-artifact-stage] exact app-private artifact set published")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
