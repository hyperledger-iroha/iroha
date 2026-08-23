#!/usr/bin/env python3
"""Independently rederive and verify a finalized Sumeragi V2 replay release."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
from typing import Any

import check_sumeragi_v2_replay_receipt as receipt_checker
from sumeragi_v2_replay_signing import (
    MAX_POLICY_BYTES,
    MAX_RECEIPT_BYTES,
    MAX_SIGNATURE_BYTES,
    MAX_TOOL_BYTES,
    SSHSIG_NAMESPACE,
    SIGNATURE_FORMAT,
    SigningError,
    read_snapshot,
    require_unchanged,
    validate_signing_contract,
    verify_exact_signature_bytes,
)


ATTESTATION_SCHEMA = "iroha-sumeragi-v2-replay-release-attestation-v1"
WORKSPACE_ROOT = Path(__file__).resolve().parents[2]
FILE_MODES = {
    "receipt.json": 0o400,
    "receipt.json.sig": 0o400,
    "ssh-keygen.release-tool": 0o500,
    "allowed_signers": 0o400,
    "revocation.krl": 0o400,
    "release-attestation.json": 0o400,
}
FILE_BOUNDS = {
    "receipt.json": MAX_RECEIPT_BYTES,
    "receipt.json.sig": MAX_SIGNATURE_BYTES,
    "ssh-keygen.release-tool": MAX_TOOL_BYTES,
    "allowed_signers": MAX_POLICY_BYTES,
    "revocation.krl": MAX_POLICY_BYTES,
    "release-attestation.json": 256 * 1024,
}
PAYLOAD_NAMES = tuple(name for name in FILE_MODES if name != "release-attestation.json")


class ReleaseVerificationError(RuntimeError):
    """The finalized replay release bundle is invalid."""


def _canonical_json(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def _strict_json(data: bytes, label: str) -> dict[str, Any]:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate object member")
            result[key] = value
        return result

    try:
        text = data.decode("ascii")
        value = json.loads(
            text,
            object_pairs_hook=reject_duplicates,
            parse_constant=lambda _value: (_ for _ in ()).throw(
                ValueError("non-finite JSON number")
            ),
        )
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as error:
        raise ReleaseVerificationError(f"{label} is not strict ASCII JSON") from error
    if not isinstance(value, dict) or _canonical_json(value) != data:
        raise ReleaseVerificationError(f"{label} is not canonical JSON")
    return value


def _directory_flags() -> int:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _private_directory(
    metadata: os.stat_result, label: str
) -> tuple[int, int, int, int]:
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or metadata.st_uid != os.geteuid()
    ):
        raise ReleaseVerificationError(f"{label} is not owner-owned mode 0700")
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _open_release_root(
    root: Path,
) -> tuple[
    int,
    int,
    tuple[int, int, int, int],
    tuple[int, int, int, int],
]:
    if not root.is_absolute() or root != Path(os.path.abspath(root)):
        raise ReleaseVerificationError("release root must be an absolute clean path")
    if root == WORKSPACE_ROOT or WORKSPACE_ROOT in root.parents:
        raise ReleaseVerificationError("release root must be outside the workspace")
    try:
        if root.resolve(strict=True) != root or root.parent.resolve(strict=True) != root.parent:
            raise ReleaseVerificationError("release root is not canonical")
        parent_link = root.parent.lstat()
        root_link = root.lstat()
        parent_descriptor = os.open(root.parent, _directory_flags())
        descriptor = os.open(root.name, _directory_flags(), dir_fd=parent_descriptor)
    except OSError as error:
        raise ReleaseVerificationError("release root is unavailable") from error
    try:
        parent_identity = _private_directory(
            os.fstat(parent_descriptor), "release parent"
        )
        root_identity = _private_directory(os.fstat(descriptor), "release root")
        linked_through_parent = os.stat(
            root.name,
            dir_fd=parent_descriptor,
            follow_symlinks=False,
        )
        if (
            _private_directory(parent_link, "release parent") != parent_identity
            or _private_directory(root_link, "release root") != root_identity
            or _private_directory(linked_through_parent, "release root")
            != root_identity
        ):
            raise ReleaseVerificationError("release root identity differs")
        return parent_descriptor, descriptor, parent_identity, root_identity
    except Exception:
        os.close(descriptor)
        os.close(parent_descriptor)
        raise


def _require_root_identity(
    root: Path,
    parent_descriptor: int,
    descriptor: int,
    parent_identity: tuple[int, int, int, int],
    root_identity: tuple[int, int, int, int],
) -> None:
    try:
        parent_link = root.parent.lstat()
        root_link = root.lstat()
        linked_through_parent = os.stat(
            root.name,
            dir_fd=parent_descriptor,
            follow_symlinks=False,
        )
        if root.parent.resolve(strict=True) != root.parent:
            raise ReleaseVerificationError("release parent is no longer canonical")
    except OSError as error:
        raise ReleaseVerificationError("release root identity changed") from error
    if (
        _private_directory(os.fstat(parent_descriptor), "release parent")
        != parent_identity
        or _private_directory(parent_link, "release parent") != parent_identity
        or _private_directory(os.fstat(descriptor), "release root")
        != root_identity
        or _private_directory(root_link, "release root") != root_identity
        or _private_directory(linked_through_parent, "release root")
        != root_identity
    ):
        raise ReleaseVerificationError("release root was renamed or replaced")


def _read_file(descriptor: int, name: str) -> tuple[bytes, int, tuple[int, int]]:
    try:
        before = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
    except OSError as error:
        raise ReleaseVerificationError(f"release artifact {name} is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) != FILE_MODES[name]
        or before.st_size > FILE_BOUNDS[name]
    ):
        raise ReleaseVerificationError(f"release artifact {name} metadata differs")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    opened_descriptor = os.open(name, flags, dir_fd=descriptor)
    try:
        opened = os.fstat(opened_descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise ReleaseVerificationError(f"release artifact {name} changed")
        chunks: list[bytes] = []
        total = 0
        while True:
            block = os.read(
                opened_descriptor,
                min(1024 * 1024, FILE_BOUNDS[name] + 1 - total),
            )
            if not block:
                break
            chunks.append(block)
            total += len(block)
            if total > FILE_BOUNDS[name]:
                raise ReleaseVerificationError(
                    f"release artifact {name} exceeds its bound"
                )
        after = os.fstat(opened_descriptor)
    finally:
        os.close(opened_descriptor)
    linked = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
    stable = (
        "st_dev",
        "st_ino",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
        "st_mode",
        "st_nlink",
        "st_uid",
    )
    if (
        total != opened.st_size
        or any(getattr(opened, key) != getattr(after, key) for key in stable)
        or any(getattr(after, key) != getattr(linked, key) for key in stable)
    ):
        raise ReleaseVerificationError(f"release artifact {name} drifted")
    return (
        b"".join(chunks),
        stat.S_IMODE(after.st_mode),
        (after.st_dev, after.st_ino),
    )


def _read_bundle(
    root: Path,
    parent_descriptor: int,
    descriptor: int,
    parent_identity: tuple[int, int, int, int],
    root_identity: tuple[int, int, int, int],
) -> dict[str, tuple[bytes, int, tuple[int, int]]]:
    _require_root_identity(
        root,
        parent_descriptor,
        descriptor,
        parent_identity,
        root_identity,
    )
    names = os.listdir(descriptor)
    if len(names) != len(set(names)) or set(names) != set(FILE_MODES):
        raise ReleaseVerificationError("release artifact path set differs")
    result = {name: _read_file(descriptor, name) for name in sorted(names)}
    _require_root_identity(
        root,
        parent_descriptor,
        descriptor,
        parent_identity,
        root_identity,
    )
    return result


def _record(name: str, data: bytes, mode: int) -> dict[str, Any]:
    return {
        "path": name,
        "sha256": hashlib.sha256(data).hexdigest(),
        "size_bytes": len(data),
        "mode": mode,
        "nlink": 1,
    }


def _derived_attestation(
    files: dict[str, tuple[bytes, int, tuple[int, int]]],
    *,
    principal: str,
    fingerprint: str,
    stdout_sha256: str,
) -> dict[str, Any]:
    return {
        "schema": ATTESTATION_SCHEMA,
        "schema_version": 1,
        "signature": {
            "scheme": SIGNATURE_FORMAT,
            "provider": "openssh-sshsig",
            "namespace": SSHSIG_NAMESPACE,
            "payload": "receipt.json",
            "artifact": "receipt.json.sig",
            "principal": principal,
            "fingerprint": fingerprint,
            "verification_stdout_sha256": stdout_sha256,
        },
        "artifacts": [
            _record(name, files[name][0], files[name][1])
            for name in PAYLOAD_NAMES
        ],
        "publication": {
            "create_only": True,
            "marker_last": True,
            "unexpected_files_allowed": False,
            "symlinks_allowed": False,
            "hard_links_allowed": False,
            "partial": False,
        },
    }


def verify_release(args: argparse.Namespace) -> dict[str, Any]:
    root = args.release_root
    descriptors = _open_release_root(root)
    parent_descriptor, descriptor, parent_identity, root_identity = descriptors
    try:
        before = _read_bundle(
            root,
            parent_descriptor,
            descriptor,
            parent_identity,
            root_identity,
        )
        receipt_value = _strict_json(before["receipt.json"][0], "receipt.json")
        if (
            receipt_value.get("schema")
            != "iroha-sumeragi-v2-replay-receipt-v1"
            or receipt_value.get("schema_version") != 1
            or receipt_value.get("evidence_class") != "release-receipt"
            or receipt_value.get("mode") != "formal-only"
        ):
            raise ReleaseVerificationError("archived receipt V1 identity differs")
        try:
            validate_signing_contract(receipt_value.get("signing"))
        except SigningError as error:
            raise ReleaseVerificationError(str(error)) from error

        source_receipt = read_snapshot(
            args.source_receipt,
            "source canonical receipt",
            maximum_bytes=MAX_RECEIPT_BYTES,
        )
        receipt_checker._check_structure(source_receipt.path)
        require_unchanged(
            source_receipt,
            "source canonical receipt",
            maximum_bytes=MAX_RECEIPT_BYTES,
        )
        if source_receipt.data != before["receipt.json"][0]:
            raise ReleaseVerificationError(
                "archived receipt differs from the checked source evidence"
            )

        verification = verify_exact_signature_bytes(
            receipt=before["receipt.json"][0],
            signature=before["receipt.json.sig"][0],
            ssh_keygen=before["ssh-keygen.release-tool"][0],
            allowed_signers=before["allowed_signers"][0],
            revocation_file=before["revocation.krl"][0],
            expected_signature_sha256=args.expected_signature_sha256,
            expected_ssh_keygen_sha256=args.expected_ssh_keygen_sha256,
            expected_allowed_signers_sha256=(
                args.expected_allowed_signers_sha256
            ),
            expected_revocation_sha256=args.expected_revocation_sha256,
            principal=args.principal,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
        )
        after = _read_bundle(
            root,
            parent_descriptor,
            descriptor,
            parent_identity,
            root_identity,
        )
        if before != after:
            raise ReleaseVerificationError(
                "release artifact identities changed during verification"
            )
        observed_attestation = _strict_json(
            after["release-attestation.json"][0], "release-attestation.json"
        )
        expected_attestation = _derived_attestation(
            after,
            principal=args.principal,
            fingerprint=verification.signer_fingerprint,
            stdout_sha256=verification.stdout_sha256,
        )
        if observed_attestation != expected_attestation:
            raise ReleaseVerificationError(
                "release attestation differs from independently derived evidence"
            )
        _require_root_identity(
            root,
            parent_descriptor,
            descriptor,
            parent_identity,
            root_identity,
        )
        return observed_attestation
    finally:
        os.close(descriptor)
        os.close(parent_descriptor)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_receipt", type=Path)
    parser.add_argument("--release-root", type=Path, required=True)
    parser.add_argument("--expected-signature-sha256", required=True)
    parser.add_argument("--expected-ssh-keygen-sha256", required=True)
    parser.add_argument("--expected-allowed-signers-sha256", required=True)
    parser.add_argument("--expected-revocation-sha256", required=True)
    parser.add_argument("--principal", required=True)
    parser.add_argument("--expected-signer-fingerprint", required=True)
    return parser


def main() -> int:
    try:
        attestation = verify_release(_parser().parse_args())
    except (
        OSError,
        UnicodeError,
        ValueError,
        ReleaseVerificationError,
        SigningError,
        receipt_checker.ReceiptError,
    ) as error:
        print(f"Sumeragi V2 replay release verification failed: {error}", file=sys.stderr)
        return 2
    print(
        "verified finalized Sumeragi V2 replay release for "
        f"{attestation['signature']['fingerprint']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
