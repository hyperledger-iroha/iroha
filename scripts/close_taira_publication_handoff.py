#!/usr/bin/env python3
"""Close one authority-owned Taira publication terminal for public upload.

The installed controller owns and removes the authority scratch directory it
created.  This helper only reads that directory and owns rollback of its exact
root-created destination, so cleanup authority cannot overlap.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

try:
    from . import taira_privacy_rollout_contract as rollout_observation
except ImportError:
    import taira_privacy_rollout_contract as rollout_observation

TERMINAL_FILES = (
    "publication-receipt-v1.json",
    "publication-receipt-v1.json.pub",
    "publication-receipt-v1.json.sig",
    "published-primary-digest",
    "published-receipt-digest",
    "taira-primary-oci-manifest.json",
    "taira-publication-receipt-oci-manifest.json",
)
FILE_LIMITS = {
    "publication-receipt-v1.json": 16 * 1024 * 1024,
    "publication-receipt-v1.json.pub": 32,
    "publication-receipt-v1.json.sig": 64,
    "published-primary-digest": 72,
    "published-receipt-digest": 72,
    "taira-primary-oci-manifest.json": 16 * 1024 * 1024,
    "taira-publication-receipt-oci-manifest.json": 16 * 1024 * 1024,
}
EXACT_SIZES = {
    "publication-receipt-v1.json.pub": 32,
    "publication-receipt-v1.json.sig": 64,
    "published-primary-digest": 72,
    "published-receipt-digest": 72,
}
SHA256_RE = re.compile(r"[0-9a-f]{64}")
OCI_DIGEST_RE = re.compile(r"sha256:[0-9a-f]{64}\n")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
OUTPUT_PREFIX = "publication-receipt-"


class PublicationHandoffError(RuntimeError):
    """The authority terminal could not cross the root-owned boundary."""


@dataclass(frozen=True)
class Captured:
    name: str
    payload: bytes
    sha256: str
    identity: tuple[int, ...]


def _fail(message: str) -> NoReturn:
    raise PublicationHandoffError(message)


def _require_authenticated_rollout_observation_authority() -> None:
    """Translate the independent observation provisioning barrier."""

    try:
        rollout_observation.require_authenticated_rollout_observation_authority_provisioned()
    except rollout_observation.RolloutContractError as exc:
        raise PublicationHandoffError(str(exc)) from exc


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _directory_identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
    )


def _absolute(path: Path, label: str) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must be one absolute lexical path")
    return path


def _reject_symlink_chain(path: Path, label: str) -> None:
    for component in [*reversed(path.parents), path]:
        try:
            info = component.lstat()
        except OSError as exc:
            raise PublicationHandoffError(
                f"cannot inspect {label} path component: {component}"
            ) from exc
        if stat.S_ISLNK(info.st_mode):
            _fail(f"{label} path contains a symlink component")


def _canonical_positive(value: str, label: str) -> int:
    if not value.isascii() or not value.isdecimal():
        _fail(f"{label} is noncanonical")
    number = int(value)
    if value != str(number) or number <= 0:
        _fail(f"{label} must be one positive canonical integer")
    return number


def _canonical_nonnegative(value: str, label: str) -> int:
    if not value.isascii() or not value.isdecimal():
        _fail(f"{label} is noncanonical")
    number = int(value)
    if value != str(number) or number < 0:
        _fail(f"{label} must be one nonnegative canonical integer")
    return number


def _open_bound_directory(
    path: Path,
    label: str,
    *,
    uid: int,
    gid: int,
    mode: int,
) -> tuple[int, tuple[int, ...]]:
    _absolute(path, label)
    _reject_symlink_chain(path, label)
    before = path.lstat()
    flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            _directory_identity(opened) != _directory_identity(before)
            or not stat.S_ISDIR(opened.st_mode)
            or stat.S_ISLNK(opened.st_mode)
            or opened.st_uid != uid
            or opened.st_gid != gid
            or stat.S_IMODE(opened.st_mode) != mode
        ):
            _fail(f"{label} ownership, mode, or inode differs")
    except BaseException:
        os.close(descriptor)
        raise
    return descriptor, _directory_identity(opened)


def _read_frozen_at(
    directory_fd: int,
    name: str,
    *,
    uid: int,
    gid: int,
) -> Captured:
    if name not in FILE_LIMITS:
        _fail("publication terminal file name is not allow-listed")
    before = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    exact_size = EXACT_SIZES.get(name)
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_uid != uid
        or before.st_gid != gid
        or stat.S_IMODE(before.st_mode) != 0o444
        or before.st_size <= 0
        or before.st_size > FILE_LIMITS[name]
        or (exact_size is not None and before.st_size != exact_size)
    ):
        _fail(f"publication terminal file identity differs: {name}")
    flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    descriptor = os.open(name, flags, dir_fd=directory_fd)
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            _fail(f"publication terminal file changed while opening: {name}")
        payload = bytearray()
        while len(payload) < before.st_size:
            chunk = os.read(
                descriptor,
                min(64 * 1024, before.st_size - len(payload)),
            )
            if not chunk:
                _fail(f"publication terminal file was truncated: {name}")
            payload.extend(chunk)
        if os.read(descriptor, 1):
            _fail(f"publication terminal file grew while reading: {name}")
        if _identity(os.fstat(descriptor)) != _identity(before):
            _fail(f"publication terminal file changed while reading: {name}")
    finally:
        os.close(descriptor)
    if _identity(
        os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    ) != _identity(before):
        _fail(f"publication terminal file path changed after reading: {name}")
    body = bytes(payload)
    return Captured(name, body, hashlib.sha256(body).hexdigest(), _identity(before))


def _replay_at(
    directory_fd: int,
    captured: Captured,
    *,
    uid: int,
    gid: int,
) -> None:
    replay = _read_frozen_at(directory_fd, captured.name, uid=uid, gid=gid)
    if (
        replay.identity != captured.identity
        or replay.sha256 != captured.sha256
        or replay.payload != captured.payload
    ):
        _fail(f"publication terminal file changed during close: {captured.name}")


def _strict_receipt(
    payload: bytes,
    receipt_id: str,
    *,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    expected_cargo_lock_sha256: str,
    expected_workspace_source_manifest_sha256: str,
) -> None:
    def unique(pairs: list[tuple[str, object]]) -> dict[str, object]:
        value: dict[str, object] = {}
        for key, item in pairs:
            if key in value:
                raise ValueError(f"duplicate key: {key}")
            value[key] = item
        return value

    def reject_nonfinite(value: str) -> NoReturn:
        raise ValueError(f"non-finite value: {value}")

    try:
        value = json.loads(
            payload,
            object_pairs_hook=unique,
            parse_constant=reject_nonfinite,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as exc:
        raise PublicationHandoffError(
            "publication receipt is not strict JSON"
        ) from exc
    canonical = (
        json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True, allow_nan=False)
        + "\n"
    ).encode("ascii")
    source = value.get("source") if isinstance(value, dict) else None
    if (
        payload != canonical
        or not isinstance(value, dict)
        or value.get("schema") != "iroha.taira.publication_receipt"
        or value.get("schema_version") != 1
        or value.get("qualification_receipt_id") != receipt_id
        or not isinstance(source, dict)
        or set(source)
        != {
            "cargo_lock_sha256",
            "commit",
            "dpn_validator_release_commit",
            "workspace_source_manifest_sha256",
        }
        or source.get("commit") != expected_source_commit
        or source.get("dpn_validator_release_commit")
        != expected_dpn_validator_release_commit
        or source.get("cargo_lock_sha256") != expected_cargo_lock_sha256
        or source.get("workspace_source_manifest_sha256")
        != expected_workspace_source_manifest_sha256
    ):
        _fail("publication receipt identity or source binding differs")


def _validate_payload_bindings(
    files: dict[str, Captured],
    receipt_id: str,
    fingerprint: str,
    *,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    expected_cargo_lock_sha256: str,
    expected_workspace_source_manifest_sha256: str,
) -> None:
    _strict_receipt(
        files["publication-receipt-v1.json"].payload,
        receipt_id,
        expected_source_commit=expected_source_commit,
        expected_dpn_validator_release_commit=(
            expected_dpn_validator_release_commit
        ),
        expected_cargo_lock_sha256=expected_cargo_lock_sha256,
        expected_workspace_source_manifest_sha256=(
            expected_workspace_source_manifest_sha256
        ),
    )
    if hashlib.sha256(
        files["publication-receipt-v1.json.pub"].payload
    ).hexdigest() != fingerprint:
        _fail("publication public key fingerprint differs")
    for digest_name, manifest_name in (
        ("published-primary-digest", "taira-primary-oci-manifest.json"),
        (
            "published-receipt-digest",
            "taira-publication-receipt-oci-manifest.json",
        ),
    ):
        digest_payload = files[digest_name].payload
        try:
            digest_text = digest_payload.decode("ascii")
        except UnicodeDecodeError as exc:
            raise PublicationHandoffError(
                f"publication OCI digest is noncanonical: {digest_name}"
            ) from exc
        if OCI_DIGEST_RE.fullmatch(digest_text) is None:
            _fail(f"publication OCI digest is noncanonical: {digest_name}")
        expected = digest_text.removesuffix("\n").removeprefix("sha256:")
        if files[manifest_name].sha256 != expected:
            _fail(f"publication OCI manifest digest differs: {manifest_name}")


def _write_frozen_at(
    directory_fd: int,
    captured: Captured,
    *,
    uid: int,
    gid: int,
) -> tuple[int, ...]:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(captured.name, flags, 0o600, dir_fd=directory_fd)
    try:
        view = memoryview(captured.payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail(f"short public handoff write: {captured.name}")
            view = view[written:]
        os.fchmod(descriptor, 0o444)
        os.fsync(descriptor)
        installed = os.fstat(descriptor)
        if (
            not stat.S_ISREG(installed.st_mode)
            or installed.st_nlink != 1
            or installed.st_uid != uid
            or installed.st_gid != gid
            or stat.S_IMODE(installed.st_mode) != 0o444
            or installed.st_size != len(captured.payload)
        ):
            _fail(f"public handoff output identity differs: {captured.name}")
    finally:
        os.close(descriptor)
    named = os.stat(captured.name, dir_fd=directory_fd, follow_symlinks=False)
    if _identity(named) != _identity(installed):
        _fail(f"public handoff output path changed: {captured.name}")
    return _identity(installed)


def _replay_output_at(
    directory_fd: int,
    captured: Captured,
    expected_identity: tuple[int, ...],
) -> None:
    named = os.stat(captured.name, dir_fd=directory_fd, follow_symlinks=False)
    if _identity(named) != expected_identity:
        _fail(f"public handoff output was replaced: {captured.name}")
    flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    descriptor = os.open(captured.name, flags, dir_fd=directory_fd)
    try:
        payload = bytearray()
        while len(payload) < len(captured.payload):
            chunk = os.read(
                descriptor,
                min(64 * 1024, len(captured.payload) - len(payload)),
            )
            if not chunk:
                _fail(f"public handoff output was truncated: {captured.name}")
            payload.extend(chunk)
        if os.read(descriptor, 1):
            _fail(f"public handoff output grew: {captured.name}")
        if (
            bytes(payload) != captured.payload
            or _identity(os.fstat(descriptor)) != expected_identity
        ):
            _fail(f"public handoff output bytes changed: {captured.name}")
    finally:
        os.close(descriptor)


def _remove_exact_directory_at(
    parent_fd: int,
    name: str,
    expected_inode: tuple[int, int],
    *,
    uid: int,
    gid: int,
) -> None:
    try:
        before = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return
    if (
        not stat.S_ISDIR(before.st_mode)
        or (before.st_dev, before.st_ino) != expected_inode
        or before.st_uid != uid
        or before.st_gid != gid
    ):
        _fail("refusing to clean a substituted publication handoff directory")
    flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    directory_fd = os.open(name, flags, dir_fd=parent_fd)
    try:
        os.fchmod(directory_fd, 0o700)
        entries = sorted(os.listdir(directory_fd))
        if any(entry not in TERMINAL_FILES for entry in entries):
            _fail("refusing to clean an unexpected publication handoff inventory")
        for entry in entries:
            info = os.stat(entry, dir_fd=directory_fd, follow_symlinks=False)
            if (
                not stat.S_ISREG(info.st_mode)
                or stat.S_ISLNK(info.st_mode)
                or info.st_nlink != 1
                or info.st_uid != uid
                or info.st_gid != gid
            ):
                _fail("refusing to clean a substituted publication handoff file")
        for entry in entries:
            os.unlink(entry, dir_fd=directory_fd)
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
    os.rmdir(name, dir_fd=parent_fd)
    os.fsync(parent_fd)


def _close_handoff(
    source_parent: Path,
    handoff_root: Path,
    *,
    expected_authority_uid: int,
    expected_authority_gid: int,
    expected_controller_uid: int,
    expected_controller_gid: int,
    expected_qualification_receipt_id: str,
    expected_signing_fingerprint: str,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    expected_cargo_lock_sha256: str,
    expected_workspace_source_manifest_sha256: str,
    _required_controller_uid: int,
) -> dict[str, object]:
    if (
        expected_controller_uid != _required_controller_uid
        or os.geteuid() != expected_controller_uid
    ):
        _fail("publication close helper must execute as the root controller")
    if os.getegid() != expected_controller_gid:
        _fail("publication close helper controller GID differs")
    if expected_authority_uid <= 0 or expected_authority_gid <= 0:
        _fail("publication close authority identity is invalid")
    if SHA256_RE.fullmatch(expected_qualification_receipt_id) is None:
        _fail("publication qualification receipt ID is noncanonical")
    if SHA256_RE.fullmatch(expected_signing_fingerprint) is None:
        _fail("publication signing fingerprint is noncanonical")
    if COMMIT_RE.fullmatch(expected_source_commit) is None:
        _fail("publication source commit is noncanonical")
    if COMMIT_RE.fullmatch(expected_dpn_validator_release_commit) is None:
        _fail("publication DPN validator release commit is noncanonical")
    if SHA256_RE.fullmatch(expected_cargo_lock_sha256) is None:
        _fail("publication Cargo.lock digest is noncanonical")
    if SHA256_RE.fullmatch(expected_workspace_source_manifest_sha256) is None:
        _fail("publication workspace source manifest digest is noncanonical")

    source_parent = _absolute(source_parent, "publication source parent")
    handoff_root = _absolute(handoff_root, "publication handoff root")
    if source_parent == handoff_root:
        _fail("publication source and public handoff roots must differ")
    source_parent_fd, source_parent_identity = _open_bound_directory(
        source_parent,
        "publication source parent",
        uid=expected_authority_uid,
        gid=expected_authority_gid,
        mode=0o700,
    )
    try:
        handoff_fd, _handoff_identity = _open_bound_directory(
            handoff_root,
            "publication handoff root",
            uid=expected_controller_uid,
            gid=expected_controller_gid,
            mode=0o711,
        )
    except BaseException:
        os.close(source_parent_fd)
        raise
    terminal_fd = -1
    staging_fd = -1
    staging_name: str | None = None
    staging_inode: tuple[int, int] | None = None
    output_name = OUTPUT_PREFIX + expected_qualification_receipt_id
    renamed = False
    committed = False
    try:
        terminal_before = os.stat(
            "terminal", dir_fd=source_parent_fd, follow_symlinks=False
        )
        directory_flags = (
            os.O_RDONLY
            | os.O_CLOEXEC
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        terminal_fd = os.open("terminal", directory_flags, dir_fd=source_parent_fd)
        terminal_opened = os.fstat(terminal_fd)
        if (
            _directory_identity(terminal_opened)
            != _directory_identity(terminal_before)
            or terminal_opened.st_uid != expected_authority_uid
            or terminal_opened.st_gid != expected_authority_gid
            or stat.S_IMODE(terminal_opened.st_mode) != 0o555
        ):
            _fail("publication terminal ownership, mode, or inode differs")
        if sorted(os.listdir(terminal_fd)) != sorted(TERMINAL_FILES):
            _fail("publication terminal inventory is not exactly seven files")
        captured = {
            name: _read_frozen_at(
                terminal_fd,
                name,
                uid=expected_authority_uid,
                gid=expected_authority_gid,
            )
            for name in sorted(TERMINAL_FILES)
        }
        _validate_payload_bindings(
            captured,
            expected_qualification_receipt_id,
            expected_signing_fingerprint,
            expected_source_commit=expected_source_commit,
            expected_dpn_validator_release_commit=(
                expected_dpn_validator_release_commit
            ),
            expected_cargo_lock_sha256=expected_cargo_lock_sha256,
            expected_workspace_source_manifest_sha256=(
                expected_workspace_source_manifest_sha256
            ),
        )
        try:
            os.stat(output_name, dir_fd=handoff_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            _fail("public publication handoff already exists")

        staging = Path(
            tempfile.mkdtemp(prefix=f".{output_name}.pending-", dir=handoff_root)
        )
        if staging.parent != handoff_root:
            _fail("public publication staging escaped the handoff root")
        staging_name = staging.name
        staging_before = os.stat(
            staging_name, dir_fd=handoff_fd, follow_symlinks=False
        )
        staging_fd = os.open(staging_name, directory_flags, dir_fd=handoff_fd)
        staging_opened = os.fstat(staging_fd)
        staging_inode = (staging_opened.st_dev, staging_opened.st_ino)
        if (
            _directory_identity(staging_opened)
            != _directory_identity(staging_before)
            or staging_opened.st_uid != expected_controller_uid
            or staging_opened.st_gid != expected_controller_gid
            or stat.S_IMODE(staging_opened.st_mode) != 0o700
        ):
            _fail("public publication staging identity differs")
        handoff_identity_with_staging = _directory_identity(os.fstat(handoff_fd))
        installed = {
            name: _write_frozen_at(
                staging_fd,
                captured[name],
                uid=expected_controller_uid,
                gid=expected_controller_gid,
            )
            for name in sorted(TERMINAL_FILES)
        }
        if sorted(os.listdir(staging_fd)) != sorted(TERMINAL_FILES):
            _fail("public publication staging inventory differs")
        for name in sorted(TERMINAL_FILES):
            _replay_output_at(staging_fd, captured[name], installed[name])
            _replay_at(
                terminal_fd,
                captured[name],
                uid=expected_authority_uid,
                gid=expected_authority_gid,
            )
        os.fchmod(staging_fd, 0o555)
        os.fsync(staging_fd)
        if _directory_identity(os.fstat(source_parent_fd)) != source_parent_identity:
            _fail("publication source parent changed during close")
        if (
            _directory_identity(os.fstat(handoff_fd))
            != handoff_identity_with_staging
        ):
            _fail("publication handoff root changed during close")
        os.rename(
            staging_name,
            output_name,
            src_dir_fd=handoff_fd,
            dst_dir_fd=handoff_fd,
        )
        renamed = True
        os.fsync(handoff_fd)
        final_fd = os.open(output_name, directory_flags, dir_fd=handoff_fd)
        try:
            final = os.fstat(final_fd)
            if (
                (final.st_dev, final.st_ino) != staging_inode
                or final.st_uid != expected_controller_uid
                or final.st_gid != expected_controller_gid
                or stat.S_IMODE(final.st_mode) != 0o555
                or sorted(os.listdir(final_fd)) != sorted(TERMINAL_FILES)
            ):
                _fail("public publication handoff final identity differs")
            for name in sorted(TERMINAL_FILES):
                _replay_output_at(final_fd, captured[name], installed[name])
                _replay_at(
                    terminal_fd,
                    captured[name],
                    uid=expected_authority_uid,
                    gid=expected_authority_gid,
                )
        finally:
            os.close(final_fd)
        committed = True
        return {
            "files": [
                {
                    "path": name,
                    "sha256": captured[name].sha256,
                    "size": len(captured[name].payload),
                }
                for name in sorted(TERMINAL_FILES)
            ],
            "output": str(handoff_root / output_name),
            "qualification_receipt_id": expected_qualification_receipt_id,
        }
    finally:
        if staging_fd >= 0:
            os.close(staging_fd)
        if not committed and staging_inode is not None:
            cleanup_name = output_name if renamed else staging_name
            if cleanup_name is not None:
                _remove_exact_directory_at(
                    handoff_fd,
                    cleanup_name,
                    staging_inode,
                    uid=expected_controller_uid,
                    gid=expected_controller_gid,
                )
        if terminal_fd >= 0:
            os.close(terminal_fd)
        os.close(handoff_fd)
        os.close(source_parent_fd)


def close_handoff(
    source_parent: Path,
    handoff_root: Path,
    *,
    expected_authority_uid: int,
    expected_authority_gid: int,
    expected_controller_uid: int,
    expected_controller_gid: int,
    expected_qualification_receipt_id: str,
    expected_signing_fingerprint: str,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    expected_cargo_lock_sha256: str,
    expected_workspace_source_manifest_sha256: str,
) -> dict[str, object]:
    """Close a publication terminal under the fixed root-controller contract."""

    _require_authenticated_rollout_observation_authority()
    return _close_handoff(
        source_parent,
        handoff_root,
        expected_authority_uid=expected_authority_uid,
        expected_authority_gid=expected_authority_gid,
        expected_controller_uid=expected_controller_uid,
        expected_controller_gid=expected_controller_gid,
        expected_qualification_receipt_id=expected_qualification_receipt_id,
        expected_signing_fingerprint=expected_signing_fingerprint,
        expected_source_commit=expected_source_commit,
        expected_dpn_validator_release_commit=(
            expected_dpn_validator_release_commit
        ),
        expected_cargo_lock_sha256=expected_cargo_lock_sha256,
        expected_workspace_source_manifest_sha256=(
            expected_workspace_source_manifest_sha256
        ),
        _required_controller_uid=0,
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--source-parent", type=Path, required=True)
    parser.add_argument("--handoff-root", type=Path, required=True)
    parser.add_argument("--expected-authority-uid", required=True)
    parser.add_argument("--expected-authority-gid", required=True)
    parser.add_argument("--expected-controller-uid", required=True)
    parser.add_argument("--expected-controller-gid", required=True)
    parser.add_argument("--expected-qualification-receipt-id", required=True)
    parser.add_argument("--expected-signing-fingerprint", required=True)
    parser.add_argument("--expected-source-commit", required=True)
    parser.add_argument("--expected-dpn-validator-release-commit", required=True)
    parser.add_argument("--expected-cargo-lock-sha256", required=True)
    parser.add_argument(
        "--expected-workspace-source-manifest-sha256", required=True
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        # Refuse before canonicalizing identities or inspecting terminal/output paths.
        _require_authenticated_rollout_observation_authority()
        result = _close_handoff(
            args.source_parent,
            args.handoff_root,
            expected_authority_uid=_canonical_positive(
                args.expected_authority_uid, "expected authority UID"
            ),
            expected_authority_gid=_canonical_positive(
                args.expected_authority_gid, "expected authority GID"
            ),
            expected_controller_uid=_canonical_nonnegative(
                args.expected_controller_uid, "expected controller UID"
            ),
            expected_controller_gid=_canonical_nonnegative(
                args.expected_controller_gid, "expected controller GID"
            ),
            expected_qualification_receipt_id=(
                args.expected_qualification_receipt_id
            ),
            expected_signing_fingerprint=args.expected_signing_fingerprint,
            expected_source_commit=args.expected_source_commit,
            expected_dpn_validator_release_commit=(
                args.expected_dpn_validator_release_commit
            ),
            expected_cargo_lock_sha256=args.expected_cargo_lock_sha256,
            expected_workspace_source_manifest_sha256=(
                args.expected_workspace_source_manifest_sha256
            ),
            _required_controller_uid=0,
        )
    except (OSError, PublicationHandoffError) as exc:
        print(f"Taira publication handoff refused: {exc}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(
        (
            json.dumps(result, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
            + "\n"
        ).encode("ascii")
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
