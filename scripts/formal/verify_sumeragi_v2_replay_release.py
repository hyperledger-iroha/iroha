#!/usr/bin/env python3
"""Independently rederive and verify a finalized Sumeragi V2 replay release."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
from typing import Any


def _load_local_module(module_name: str, filename: str) -> Any:
    """Load one authenticated sibling without relying on ambient import paths."""

    path = Path(__file__).absolute().parent / filename
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"replay support module is unavailable: {path}")
    canonical_path = path.resolve(strict=True)
    if canonical_path != path:
        raise RuntimeError(f"replay support module path is not canonical: {path}")
    path = canonical_path
    loaded = sys.modules.get(module_name)
    if loaded is not None:
        loaded_path = Path(getattr(loaded, "__file__", "")).resolve()
        if loaded_path != path:
            raise RuntimeError(
                f"replay support module identity differs: {loaded_path} != {path}"
            )
        return loaded
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load replay support module: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


receipt_checker = _load_local_module(
    "check_sumeragi_v2_replay_receipt",
    "check_sumeragi_v2_replay_receipt.py",
)
_REPLAY_SIGNING = receipt_checker._REPLAY_SIGNING
MAX_POLICY_BYTES = _REPLAY_SIGNING.MAX_POLICY_BYTES
MAX_RECEIPT_BYTES = _REPLAY_SIGNING.MAX_RECEIPT_BYTES
MAX_SIGNATURE_BYTES = _REPLAY_SIGNING.MAX_SIGNATURE_BYTES
MAX_TOOL_BYTES = _REPLAY_SIGNING.MAX_TOOL_BYTES
SSHSIG_NAMESPACE = _REPLAY_SIGNING.SSHSIG_NAMESPACE
SIGNATURE_FORMAT = _REPLAY_SIGNING.SIGNATURE_FORMAT
SigningError = _REPLAY_SIGNING.SigningError
read_snapshot = _REPLAY_SIGNING.read_snapshot
require_unchanged = _REPLAY_SIGNING.require_unchanged
validate_signing_contract = _REPLAY_SIGNING.validate_signing_contract
verify_exact_signature_bytes = _REPLAY_SIGNING.verify_exact_signature_bytes


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
PROJECTION_DIRECTORY_NAME = "tlapm-projection"
PROJECTION_FILE_NAMES = ("Folds.tla", "Functions.tla")
PROJECTION_LOGICAL_NAMES = tuple(
    f"{PROJECTION_DIRECTORY_NAME}/{name}" for name in PROJECTION_FILE_NAMES
)
PROJECTION_FILE_MODES = {
    logical: 0o444 for logical in PROJECTION_LOGICAL_NAMES
}
ARTIFACT_MODES = {**FILE_MODES, **PROJECTION_FILE_MODES}
FILE_BOUNDS = {
    "receipt.json": MAX_RECEIPT_BYTES,
    "receipt.json.sig": MAX_SIGNATURE_BYTES,
    "ssh-keygen.release-tool": MAX_TOOL_BYTES,
    "allowed_signers": MAX_POLICY_BYTES,
    "revocation.krl": MAX_POLICY_BYTES,
    "release-attestation.json": 256 * 1024,
    **{logical: 1024 * 1024 for logical in PROJECTION_LOGICAL_NAMES},
}
PAYLOAD_NAMES = (
    *(name for name in FILE_MODES if name != "release-attestation.json"),
    *PROJECTION_LOGICAL_NAMES,
)


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


def _read_only_projection_directory(
    metadata: os.stat_result, label: str
) -> tuple[int, int, int, int, int, int]:
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o555
        or metadata.st_uid != os.geteuid()
    ):
        raise ReleaseVerificationError(
            f"{label} is not an owner-owned read-only mode 0555 directory"
        )
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
        metadata.st_mode,
        metadata.st_uid,
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


def _read_file(
    descriptor: int, name: str, *, logical_name: str | None = None
) -> tuple[bytes, int, tuple[int, int]]:
    logical_name = name if logical_name is None else logical_name
    try:
        before = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
    except OSError as error:
        raise ReleaseVerificationError(
            f"release artifact {logical_name} is unavailable"
        ) from error
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) != ARTIFACT_MODES[logical_name]
        or before.st_size > FILE_BOUNDS[logical_name]
    ):
        raise ReleaseVerificationError(
            f"release artifact {logical_name} metadata differs"
        )
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    opened_descriptor = os.open(name, flags, dir_fd=descriptor)
    try:
        opened = os.fstat(opened_descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise ReleaseVerificationError(
                f"release artifact {logical_name} changed"
            )
        chunks: list[bytes] = []
        total = 0
        while True:
            block = os.read(
                opened_descriptor,
                min(1024 * 1024, FILE_BOUNDS[logical_name] + 1 - total),
            )
            if not block:
                break
            chunks.append(block)
            total += len(block)
            if total > FILE_BOUNDS[logical_name]:
                raise ReleaseVerificationError(
                    f"release artifact {logical_name} exceeds its bound"
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
        raise ReleaseVerificationError(
            f"release artifact {logical_name} drifted"
        )
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
    expected_root_names = {*FILE_MODES, PROJECTION_DIRECTORY_NAME}
    if len(names) != len(set(names)) or set(names) != expected_root_names:
        raise ReleaseVerificationError("release artifact path set differs")
    result = {
        name: _read_file(descriptor, name) for name in sorted(FILE_MODES)
    }
    try:
        projection_linked = os.stat(
            PROJECTION_DIRECTORY_NAME,
            dir_fd=descriptor,
            follow_symlinks=False,
        )
        projection_descriptor = os.open(
            PROJECTION_DIRECTORY_NAME,
            _directory_flags(),
            dir_fd=descriptor,
        )
    except OSError as error:
        raise ReleaseVerificationError(
            "release TLAPM projection is unavailable"
        ) from error
    try:
        projection_identity = _read_only_projection_directory(
            projection_linked, "release TLAPM projection"
        )
        if (
            _read_only_projection_directory(
                os.fstat(projection_descriptor), "release TLAPM projection"
            )
            != projection_identity
        ):
            raise ReleaseVerificationError(
                "release TLAPM projection identity differs"
            )
        projection_names = os.listdir(projection_descriptor)
        if (
            len(projection_names) != len(set(projection_names))
            or set(projection_names) != set(PROJECTION_FILE_NAMES)
        ):
            raise ReleaseVerificationError(
                "release TLAPM projection path set differs"
            )
        for logical in PROJECTION_LOGICAL_NAMES:
            name = logical.rsplit("/", 1)[-1]
            result[logical] = _read_file(
                projection_descriptor,
                name,
                logical_name=logical,
            )
        projection_after = os.stat(
            PROJECTION_DIRECTORY_NAME,
            dir_fd=descriptor,
            follow_symlinks=False,
        )
        if (
            _read_only_projection_directory(
                os.fstat(projection_descriptor), "release TLAPM projection"
            )
            != projection_identity
            or _read_only_projection_directory(
                projection_after, "release TLAPM projection"
            )
            != projection_identity
        ):
            raise ReleaseVerificationError(
                "release TLAPM projection changed while reading"
            )
    finally:
        os.close(projection_descriptor)
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
            "projection": {
                "path": PROJECTION_DIRECTORY_NAME,
                "mode": 0o555,
                "read_only": True,
                "files": list(PROJECTION_LOGICAL_NAMES),
            },
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
        tool_identity = receipt_value.get("tool_identity")
        tool_files = (
            tool_identity.get("files")
            if isinstance(tool_identity, dict)
            else None
        )
        if not isinstance(tool_files, list):
            raise ReleaseVerificationError(
                "archived receipt tool identity is absent"
            )
        projection_records = {
            record.get("path"): record
            for record in tool_files
            if isinstance(record, dict)
            and record.get("path") in PROJECTION_LOGICAL_NAMES
        }
        expected_projection_records = {
            logical: _record(
                logical,
                before[logical][0],
                before[logical][1],
            )
            for logical in PROJECTION_LOGICAL_NAMES
        }
        if projection_records != expected_projection_records:
            raise ReleaseVerificationError(
                "archived TLAPM projection differs from the signed receipt"
            )

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
