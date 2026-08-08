#!/usr/bin/env python3
"""Publish one root-owned, immutable Taira qualification receipt handoff."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_json_bytes,
    create_fresh_directory,
    exclusive_write_bytes,
    scan_inventory_paths,
    stable_read_path,
)
from taira_rollout_admission import (
    MACOS_RECEIPT_SCHEMA,
    MACOS_RECEIPT_SCHEMA_VERSION,
    PRIVACY_PROTOCOL_RECEIPT_SCHEMA,
    PRIVACY_PROTOCOL_RECEIPT_SCHEMA_VERSION,
)

HANDOFF_MANIFEST = "handoff-inventory-v1.json"
RECEIPT_NAME = "four-peer-receipt-v2.json"
PRIVACY_PROTOCOL_RECEIPT_NAME = "privacy-protocol-four-peer-receipt-v1.json"
SOURCE_IDENTITY_NAME = "taira-source-identity-v1.json"
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")


class QualificationHandoffError(RuntimeError):
    """The qualification result could not cross the authority boundary."""


def _compact_json_bytes(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


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


def _write_frozen_at(
    directory_fd: int,
    name: str,
    payload: bytes,
    *,
    expected_uid: int,
    expected_gid: int,
) -> tuple[int, ...]:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(name, flags, 0o400, dir_fd=directory_fd)
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise QualificationHandoffError(
                    f"short qualification handoff write: {name}"
                )
            view = view[written:]
        os.fchmod(descriptor, 0o444)
        os.fsync(descriptor)
        installed = os.fstat(descriptor)
        if (
            not stat.S_ISREG(installed.st_mode)
            or installed.st_nlink != 1
            or installed.st_uid != expected_uid
            or installed.st_gid != expected_gid
            or stat.S_IMODE(installed.st_mode) != 0o444
            or installed.st_size != len(payload)
        ):
            raise QualificationHandoffError(
                f"qualification handoff output identity differs: {name}"
            )
    finally:
        os.close(descriptor)
    named = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    if _identity(named) != _identity(installed):
        raise QualificationHandoffError(
            f"qualification handoff output path changed: {name}"
        )
    return _identity(installed)


def _replay_frozen_at(
    directory_fd: int,
    name: str,
    payload: bytes,
    expected_identity: tuple[int, ...],
) -> None:
    named = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    if _identity(named) != expected_identity:
        raise QualificationHandoffError(
            f"qualification handoff output was replaced: {name}"
        )
    flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    descriptor = os.open(name, flags, dir_fd=directory_fd)
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != expected_identity:
            raise QualificationHandoffError(
                f"qualification handoff output changed while opening: {name}"
            )
        replay = bytearray()
        while len(replay) < len(payload):
            chunk = os.read(
                descriptor,
                min(64 * 1024, len(payload) - len(replay)),
            )
            if not chunk:
                raise QualificationHandoffError(
                    f"qualification handoff output was truncated: {name}"
                )
            replay.extend(chunk)
        if os.read(descriptor, 1):
            raise QualificationHandoffError(
                f"qualification handoff output grew while reading: {name}"
            )
        if bytes(replay) != payload or _identity(os.fstat(descriptor)) != expected_identity:
            raise QualificationHandoffError(
                f"qualification handoff output bytes changed: {name}"
            )
    finally:
        os.close(descriptor)
    if _identity(
        os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    ) != expected_identity:
        raise QualificationHandoffError(
            f"qualification handoff output path changed after replay: {name}"
        )


def _canonical_payload(
    path: Path,
    label: str,
    maximum: int,
    *,
    compact: bool,
) -> tuple[dict[str, object], bytes]:
    _info, payload = stable_read_path(path, max_size=maximum)
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise QualificationHandoffError(f"{label} is invalid JSON") from exc
    rendered = _compact_json_bytes(value) if compact else canonical_json_bytes(value)
    if not isinstance(value, dict) or rendered != payload:
        raise QualificationHandoffError(f"{label} is not canonical JSON")
    return value, payload


def close_handoff(
    receipt: Path,
    privacy_protocol_receipt: Path,
    source_identity: Path,
    output: Path,
) -> dict[str, object]:
    receipt_value, receipt_payload = _canonical_payload(
        receipt, "qualification receipt", 4 * 1024 * 1024, compact=False
    )
    identity_value, identity_payload = _canonical_payload(
        source_identity, "source identity", 1024 * 1024, compact=True
    )
    privacy_value, privacy_payload = _canonical_payload(
        privacy_protocol_receipt,
        "privacy protocol four-peer receipt",
        4 * 1024 * 1024,
        compact=False,
    )
    source = identity_value.get("source")
    if (
        set(identity_value) != {"source", "source_date_epoch"}
        or not isinstance(source, dict)
        or set(source)
        != {
            "cargo_lock_sha256",
            "commit",
            "dpn_validator_release_commit",
            "workspace_source_manifest_sha256",
        }
        or COMMIT_RE.fullmatch(str(source.get("commit", ""))) is None
        or COMMIT_RE.fullmatch(str(source.get("dpn_validator_release_commit", "")))
        is None
        or SHA256_RE.fullmatch(str(source.get("cargo_lock_sha256", ""))) is None
        or SHA256_RE.fullmatch(
            str(source.get("workspace_source_manifest_sha256", ""))
        )
        is None
        or isinstance(identity_value.get("source_date_epoch"), bool)
        or not isinstance(identity_value.get("source_date_epoch"), int)
        or int(identity_value["source_date_epoch"]) <= 0
    ):
        raise QualificationHandoffError(
            "source identity is not the exact first-release four-field identity"
        )
    if (
        receipt_value.get("schema") != MACOS_RECEIPT_SCHEMA
        or receipt_value.get("schema_version") != MACOS_RECEIPT_SCHEMA_VERSION
        or receipt_value.get("source") != source
        or SHA256_RE.fullmatch(str(receipt_value.get("artifact_handoff_sha256", "")))
        is None
        or SHA256_RE.fullmatch(str(receipt_value.get("receipt_id", ""))) is None
    ):
        raise QualificationHandoffError(
            "qualification receipt is not bound to the exact source identity and handoff"
        )
    privacy_candidate = privacy_value.get("candidate")
    if (
        privacy_value.get("schema") != PRIVACY_PROTOCOL_RECEIPT_SCHEMA
        or privacy_value.get("schema_version")
        != PRIVACY_PROTOCOL_RECEIPT_SCHEMA_VERSION
        or not isinstance(privacy_candidate, dict)
        or privacy_candidate.get("source") != source
        or privacy_candidate.get("validator_binary_sha256")
        != receipt_value.get("validator_binary_sha256")
        or SHA256_RE.fullmatch(str(privacy_value.get("receipt_id", ""))) is None
    ):
        raise QualificationHandoffError(
            "privacy protocol receipt is not bound to the exact source and validator"
        )
    output = create_fresh_directory(output, mode=0o700)
    directory_flags = (
        os.O_RDONLY
        | os.O_CLOEXEC
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    parent_fd = -1
    output_fd = -1
    committed = False
    try:
        expected_uid = os.geteuid()
        expected_gid = os.getegid()
        parent_before = output.parent.lstat()
        parent_fd = os.open(output.parent, directory_flags)
        if _directory_identity(os.fstat(parent_fd)) != _directory_identity(parent_before):
            raise QualificationHandoffError(
                "qualification handoff parent changed while opening"
            )
        output_before = os.stat(
            output.name,
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
        output_fd = os.open(output.name, directory_flags, dir_fd=parent_fd)
        output_opened = os.fstat(output_fd)
        if (
            not stat.S_ISDIR(output_opened.st_mode)
            or (output_opened.st_dev, output_opened.st_ino)
            != (output_before.st_dev, output_before.st_ino)
            or output_opened.st_uid != expected_uid
            or output_opened.st_gid != expected_gid
            or stat.S_IMODE(output_opened.st_mode) != 0o700
        ):
            raise QualificationHandoffError(
                "qualification handoff root changed while opening"
            )
        parent_after_create = os.fstat(parent_fd)
        payloads = {
            PRIVACY_PROTOCOL_RECEIPT_NAME: privacy_payload,
            RECEIPT_NAME: receipt_payload,
            SOURCE_IDENTITY_NAME: identity_payload,
        }
        rows: list[dict[str, object]] = []
        installed: dict[str, tuple[int, ...]] = {}
        for name, payload in sorted(payloads.items()):
            installed[name] = _write_frozen_at(
                output_fd,
                name,
                payload,
                expected_uid=expected_uid,
                expected_gid=expected_gid,
            )
            rows.append(
                {
                    "path": name,
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "size": len(payload),
                }
            )
        handoff = {
            "files": rows,
            "kind": "qualification-receipt",
            "schema": "iroha.taira.release_handoff",
            "schema_version": 1,
        }
        manifest_payload = _compact_json_bytes(handoff)
        installed[HANDOFF_MANIFEST] = _write_frozen_at(
            output_fd,
            HANDOFF_MANIFEST,
            manifest_payload,
            expected_uid=expected_uid,
            expected_gid=expected_gid,
        )
        expected = sorted([HANDOFF_MANIFEST, *payloads])
        if sorted(os.listdir(output_fd)) != expected:
            raise QualificationHandoffError(
                "qualification handoff inventory is not exactly closed"
            )
        for name, payload in {**payloads, HANDOFF_MANIFEST: manifest_payload}.items():
            _replay_frozen_at(output_fd, name, payload, installed[name])
        os.fchmod(output_fd, 0o555)
        os.fsync(output_fd)
        os.fsync(parent_fd)
        final_info = os.fstat(output_fd)
        final_named = os.stat(
            output.name,
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
        if (
            _directory_identity(final_info) != _directory_identity(final_named)
            or final_info.st_uid != expected_uid
            or final_info.st_gid != expected_gid
            or stat.S_IMODE(final_info.st_mode) != 0o555
            or _directory_identity(os.fstat(parent_fd))
            != _directory_identity(parent_after_create)
        ):
            raise QualificationHandoffError(
                "qualification handoff root ownership or mode differs"
            )
        reopened_fd = os.open(output.name, directory_flags, dir_fd=parent_fd)
        try:
            if _directory_identity(os.fstat(reopened_fd)) != _directory_identity(
                final_info
            ):
                raise QualificationHandoffError(
                    "qualification handoff root path changed after freeze"
                )
        finally:
            os.close(reopened_fd)
        committed = True
        return {
            "handoff_manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
            "output": str(output),
            "receipt_id": receipt_value["receipt_id"],
        }
    except BaseException:
        if output_fd >= 0:
            try:
                os.fchmod(output_fd, 0o700)
            except OSError:
                pass
        raise
    finally:
        if output_fd >= 0:
            os.close(output_fd)
        if parent_fd >= 0:
            os.close(parent_fd)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument(
        "--privacy-protocol-receipt", type=Path, required=True
    )
    parser.add_argument("--source-identity", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        result = close_handoff(
            args.receipt,
            args.privacy_protocol_receipt,
            args.source_identity,
            args.output,
        )
    except (OSError, ReleaseArtifactError, QualificationHandoffError) as exc:
        print(f"Taira qualification handoff refused: {exc}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(canonical_json_bytes(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
