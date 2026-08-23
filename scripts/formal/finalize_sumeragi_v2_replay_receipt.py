#!/usr/bin/env python3
"""Finalize one externally SSH-signed Sumeragi V2 replay V1 receipt.

The finalizer never signs and accepts no private-key argument. It publishes a
create-only evidence directory only after independently checking the receipt
and detached SSHSIG with checksum-pinned public verification inputs.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
from typing import Any

import check_sumeragi_v2_replay_receipt as receipt_checker
from sumeragi_v2_replay_signing import (
    MAX_RECEIPT_BYTES,
    SSHSIG_NAMESPACE,
    SIGNATURE_FORMAT,
    SignatureInputs,
    SigningError,
    read_snapshot,
    require_sha256,
    require_unchanged,
    verify_external_signature,
    verify_exact_signature_bytes,
)


ATTESTATION_SCHEMA = "iroha-sumeragi-v2-replay-release-attestation-v1"
WORKSPACE_ROOT = Path(__file__).resolve().parents[2]
PAYLOAD_NAMES = (
    "receipt.json",
    "receipt.json.sig",
    "ssh-keygen.release-tool",
    "allowed_signers",
    "revocation.krl",
)
ATTESTATION_NAME = "release-attestation.json"
OUTPUT_NAMES = (*PAYLOAD_NAMES, ATTESTATION_NAME)
MAX_ATTESTATION_BYTES = 256 * 1024


class FinalizationError(RuntimeError):
    """The signed replay evidence could not be verified or published."""


@dataclass
class OwnedOutputRoot:
    """Held identities for one create-only release publication."""

    path: Path
    parent_path: Path
    name: str
    parent_descriptor: int
    descriptor: int
    parent_identity: tuple[int, int]
    identity: tuple[int, int]
    created: dict[str, tuple[int, int]] = field(default_factory=dict)
    parent_mtime_ns: int = 0
    parent_ctime_ns: int = 0
    root_mtime_ns: int = 0
    root_ctime_ns: int = 0


def canonical_json(value: Any) -> bytes:
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


def _record(path: str, data: bytes, mode: int) -> dict[str, Any]:
    return {
        "path": path,
        "sha256": hashlib.sha256(data).hexdigest(),
        "size_bytes": len(data),
        "mode": mode,
        "nlink": 1,
    }


def release_attestation(
    artifacts: tuple[tuple[str, bytes, int], ...],
    *,
    principal: str,
    fingerprint: str,
    verification_stdout_sha256: str,
) -> dict[str, Any]:
    """Derive the closed release-attestation value from verified inputs."""

    if tuple(name for name, _data, _mode in artifacts) != PAYLOAD_NAMES:
        raise FinalizationError("release artifact ordering differs")
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
            "verification_stdout_sha256": verification_stdout_sha256,
        },
        "artifacts": [
            _record(name, data, mode) for name, data, mode in artifacts
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


def _directory_flags() -> int:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _same_identity(metadata: os.stat_result, identity: tuple[int, int]) -> bool:
    return (metadata.st_dev, metadata.st_ino) == identity


def _require_private_directory(
    metadata: os.stat_result, label: str
) -> tuple[int, int]:
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or metadata.st_uid != os.geteuid()
    ):
        raise FinalizationError(f"{label} must be an owner-owned 0700 directory")
    return (metadata.st_dev, metadata.st_ino)


def _prepare_output_root(path: Path) -> OwnedOutputRoot:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        raise FinalizationError("--output-root must already be an absolute clean path")
    if path == WORKSPACE_ROOT or WORKSPACE_ROOT in path.parents:
        raise FinalizationError("--output-root must be outside the workspace")
    if path.name in {"", ".", ".."} or "/" in path.name:
        raise FinalizationError("--output-root has an invalid final component")
    parent_path = path.parent
    descriptor = -1
    created_identity: tuple[int, int] | None = None
    try:
        if parent_path.resolve(strict=True) != parent_path:
            raise FinalizationError("finalizer output parent is not canonical")
        parent_link = parent_path.lstat()
        parent_descriptor = os.open(parent_path, _directory_flags())
    except OSError as error:
        raise FinalizationError("finalizer output parent is unavailable") from error
    try:
        parent_opened = os.fstat(parent_descriptor)
        parent_identity = _require_private_directory(
            parent_opened, "finalizer output parent"
        )
        if (
            not _same_identity(parent_link, parent_identity)
            or _require_private_directory(
                parent_link, "finalizer output parent"
            )
            != parent_identity
        ):
            raise FinalizationError("finalizer output parent identity differs")
        try:
            os.stat(path.name, dir_fd=parent_descriptor, follow_symlinks=False)
        except FileNotFoundError:
            pass
        except OSError as error:
            raise FinalizationError(
                "finalizer output root could not be inspected"
            ) from error
        else:
            raise FinalizationError("finalizer output root is create-only")

        os.mkdir(path.name, 0o700, dir_fd=parent_descriptor)
        created_link = os.stat(
            path.name, dir_fd=parent_descriptor, follow_symlinks=False
        )
        created_identity = _require_private_directory(
            created_link, "finalizer output root"
        )
        descriptor = os.open(
            path.name, _directory_flags(), dir_fd=parent_descriptor
        )
        opened = os.fstat(descriptor)
        if (
            _require_private_directory(opened, "finalizer output root")
            != created_identity
        ):
            raise FinalizationError("finalizer output root identity differs")
        return OwnedOutputRoot(
            path=path,
            parent_path=parent_path,
            name=path.name,
            parent_descriptor=parent_descriptor,
            descriptor=descriptor,
            parent_identity=parent_identity,
            identity=created_identity,
        )
    except Exception:
        removable = False
        if created_identity is not None:
            try:
                current = os.stat(
                    path.name,
                    dir_fd=parent_descriptor,
                    follow_symlinks=False,
                )
                removable = _same_identity(current, created_identity)
                if descriptor >= 0:
                    removable = removable and not os.listdir(descriptor)
            except OSError:
                removable = False
        if descriptor >= 0:
            os.close(descriptor)
        if removable:
            try:
                os.rmdir(path.name, dir_fd=parent_descriptor)
                os.fsync(parent_descriptor)
            except OSError:
                pass
        os.close(parent_descriptor)
        raise


def _require_owned_root(output: OwnedOutputRoot) -> None:
    try:
        parent_opened = os.fstat(output.parent_descriptor)
        root_opened = os.fstat(output.descriptor)
        parent_link = output.parent_path.lstat()
        root_link = os.stat(
            output.name,
            dir_fd=output.parent_descriptor,
            follow_symlinks=False,
        )
        lexical_root = output.path.lstat()
        canonical_parent = output.parent_path.resolve(strict=True)
    except OSError as error:
        raise FinalizationError(
            "finalizer output ownership changed during publication"
        ) from error
    if (
        canonical_parent != output.parent_path
        or _require_private_directory(
            parent_opened, "finalizer output parent"
        )
        != output.parent_identity
        or _require_private_directory(parent_link, "finalizer output parent")
        != output.parent_identity
        or _require_private_directory(root_opened, "finalizer output root")
        != output.identity
        or _require_private_directory(root_link, "finalizer output root")
        != output.identity
        or _require_private_directory(lexical_root, "finalizer output root")
        != output.identity
        or (
            output.parent_mtime_ns != 0
            and (
                parent_opened.st_mtime_ns != output.parent_mtime_ns
                or parent_opened.st_ctime_ns != output.parent_ctime_ns
            )
        )
        or (
            output.root_mtime_ns != 0
            and (
                root_opened.st_mtime_ns != output.root_mtime_ns
                or root_opened.st_ctime_ns != output.root_ctime_ns
            )
        )
    ):
        raise FinalizationError(
            "finalizer output parent or root was renamed or replaced"
        )


def _seal_owned_root(output: OwnedOutputRoot) -> None:
    parent = os.fstat(output.parent_descriptor)
    root = os.fstat(output.descriptor)
    if output.parent_mtime_ns == 0:
        output.parent_mtime_ns = parent.st_mtime_ns
        output.parent_ctime_ns = parent.st_ctime_ns
    elif (
        parent.st_mtime_ns != output.parent_mtime_ns
        or parent.st_ctime_ns != output.parent_ctime_ns
    ):
        raise FinalizationError("finalizer output parent changed after sealing")
    output.root_mtime_ns = root.st_mtime_ns
    output.root_ctime_ns = root.st_ctime_ns
    _require_owned_root(output)


def _write_create_only(
    output: OwnedOutputRoot, name: str, data: bytes, mode: int
) -> None:
    _require_owned_root(output)
    if name not in OUTPUT_NAMES or name in output.created:
        raise FinalizationError("release artifact name or ordering differs")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(
            name, flags, mode, dir_fd=output.descriptor
        )
    except OSError as error:
        raise FinalizationError(f"{name} could not be created exclusively") from error
    try:
        offset = 0
        while offset < len(data):
            written = os.write(descriptor, data[offset:])
            if written <= 0:
                raise FinalizationError(f"{name} could not be written completely")
            offset += written
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or stat.S_IMODE(metadata.st_mode) != mode
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or metadata.st_size != len(data)
        ):
            raise FinalizationError(f"{name} publication metadata differs")
        output.created[name] = (metadata.st_dev, metadata.st_ino)
    finally:
        os.close(descriptor)


def _read_held_file(
    output: OwnedOutputRoot,
    name: str,
    *,
    maximum_bytes: int,
) -> tuple[bytes, int, tuple[int, int]]:
    try:
        before = os.stat(
            name, dir_fd=output.descriptor, follow_symlinks=False
        )
    except OSError as error:
        raise FinalizationError(f"{name} is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or before.st_nlink != 1
        or before.st_size > maximum_bytes
    ):
        raise FinalizationError(f"{name} is not a bounded single-link file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=output.descriptor)
    try:
        opened = os.fstat(descriptor)
        if not _same_identity(opened, (before.st_dev, before.st_ino)):
            raise FinalizationError(f"{name} changed while opening")
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(
                descriptor, min(1024 * 1024, maximum_bytes + 1 - total)
            )
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise FinalizationError(f"{name} exceeds its closed byte limit")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    linked = os.stat(name, dir_fd=output.descriptor, follow_symlinks=False)
    stable_fields = (
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
        or any(getattr(opened, key) != getattr(after, key) for key in stable_fields)
        or any(getattr(after, key) != getattr(linked, key) for key in stable_fields)
    ):
        raise FinalizationError(f"{name} changed while reading")
    return (
        b"".join(chunks),
        stat.S_IMODE(after.st_mode),
        (after.st_dev, after.st_ino),
    )


def _validate_exact_output(
    output: OwnedOutputRoot,
    expected: dict[str, tuple[bytes, int]],
) -> dict[str, tuple[bytes, int, tuple[int, int]]]:
    _require_owned_root(output)
    try:
        names = os.listdir(output.descriptor)
    except OSError as error:
        raise FinalizationError("finalizer output could not be enumerated") from error
    if len(names) != len(set(names)) or set(names) != set(expected):
        raise FinalizationError("finalizer output path set differs")
    observed: dict[str, tuple[bytes, int, tuple[int, int]]] = {}
    for name in sorted(expected):
        data, mode, identity = _read_held_file(
            output,
            name,
            maximum_bytes=(
                MAX_ATTESTATION_BYTES
                if name == ATTESTATION_NAME
                else max(len(expected[name][0]), 1)
            ),
        )
        expected_data, expected_mode = expected[name]
        if (
            data != expected_data
            or mode != expected_mode
            or output.created.get(name) != identity
        ):
            raise FinalizationError(f"{name} final bytes or identity differ")
        observed[name] = (data, mode, identity)
    _require_owned_root(output)
    return observed


def _cleanup_owned_root(output: OwnedOutputRoot) -> None:
    failures: list[str] = []
    try:
        for name in reversed(tuple(output.created)):
            try:
                metadata = os.stat(
                    name,
                    dir_fd=output.descriptor,
                    follow_symlinks=False,
                )
                if (
                    not stat.S_ISREG(metadata.st_mode)
                    or metadata.st_nlink != 1
                    or not _same_identity(metadata, output.created[name])
                ):
                    failures.append(name)
                    continue
                os.unlink(name, dir_fd=output.descriptor)
            except FileNotFoundError:
                continue
            except OSError:
                failures.append(name)
        try:
            os.fsync(output.descriptor)
        except OSError:
            failures.append("<root-fsync>")
        try:
            remaining = os.listdir(output.descriptor)
        except OSError:
            remaining = ["<unreadable>"]
        if remaining:
            failures.extend(str(name) for name in remaining)
        try:
            root_link = os.stat(
                output.name,
                dir_fd=output.parent_descriptor,
                follow_symlinks=False,
            )
            if not _same_identity(root_link, output.identity):
                failures.append("<root-link>")
            elif not remaining:
                os.rmdir(output.name, dir_fd=output.parent_descriptor)
                os.fsync(output.parent_descriptor)
        except OSError:
            failures.append("<root-link>")
    finally:
        os.close(output.descriptor)
        os.close(output.parent_descriptor)
    if failures:
        raise FinalizationError(
            "owned finalizer output could not be reclaimed: "
            + ", ".join(sorted(set(failures)))
        )


def _close_success(output: OwnedOutputRoot) -> None:
    try:
        _require_owned_root(output)
    finally:
        os.close(output.descriptor)
        os.close(output.parent_descriptor)


def finalize(args: argparse.Namespace) -> Path:
    receipt_path = Path(os.path.abspath(args.receipt))
    receipt = read_snapshot(
        receipt_path, "canonical receipt", maximum_bytes=MAX_RECEIPT_BYTES
    )
    if receipt.mode != 0o600:
        raise FinalizationError("receipt.json mode must be 0600")
    receipt_checker._check_structure(receipt_path)
    require_unchanged(
        receipt, "canonical receipt", maximum_bytes=MAX_RECEIPT_BYTES
    )

    expected = {
        "signature": require_sha256(
            args.expected_signature_sha256,
            "expected detached SSHSIG digest",
        ),
        "ssh_keygen": require_sha256(
            args.expected_ssh_keygen_sha256,
            "expected ssh-keygen digest",
        ),
        "allowed": require_sha256(
            args.expected_allowed_signers_sha256,
            "expected allowed-signers digest",
        ),
        "revocation": require_sha256(
            args.expected_revocation_sha256,
            "expected revocation digest",
        ),
    }
    source_inputs = SignatureInputs(
        signature=args.signature,
        expected_signature_sha256=expected["signature"],
        ssh_keygen=args.ssh_keygen_bin,
        expected_ssh_keygen_sha256=expected["ssh_keygen"],
        allowed_signers=args.allowed_signers,
        expected_allowed_signers_sha256=expected["allowed"],
        revocation_file=args.revocation_file,
        expected_revocation_sha256=expected["revocation"],
        principal=args.principal,
        expected_signer_fingerprint=args.expected_signer_fingerprint,
    )
    verified_sources = verify_external_signature(receipt, source_inputs)
    artifacts = (
        ("receipt.json", receipt.data, 0o400),
        ("receipt.json.sig", verified_sources.signature.data, 0o400),
        (
            "ssh-keygen.release-tool",
            verified_sources.ssh_keygen.data,
            0o500,
        ),
        ("allowed_signers", verified_sources.allowed_signers.data, 0o400),
        ("revocation.krl", verified_sources.revocation_file.data, 0o400),
    )

    output = _prepare_output_root(args.output_root)
    try:
        expected_files: dict[str, tuple[bytes, int]] = {}
        for name, data, mode in artifacts:
            _write_create_only(output, name, data, mode)
            expected_files[name] = (data, mode)
        os.fsync(output.descriptor)
        _seal_owned_root(output)
        archived_files = _validate_exact_output(output, expected_files)

        archived_verification = verify_exact_signature_bytes(
            receipt=archived_files["receipt.json"][0],
            signature=archived_files["receipt.json.sig"][0],
            ssh_keygen=archived_files["ssh-keygen.release-tool"][0],
            allowed_signers=archived_files["allowed_signers"][0],
            revocation_file=archived_files["revocation.krl"][0],
            expected_signature_sha256=expected["signature"],
            expected_ssh_keygen_sha256=expected["ssh_keygen"],
            expected_allowed_signers_sha256=expected["allowed"],
            expected_revocation_sha256=expected["revocation"],
            principal=args.principal,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
        )
        if _validate_exact_output(output, expected_files) != archived_files:
            raise FinalizationError(
                "archived release inputs changed during verification"
            )

        attestation = release_attestation(
            artifacts,
            principal=args.principal,
            fingerprint=archived_verification.signer_fingerprint,
            verification_stdout_sha256=archived_verification.stdout_sha256,
        )
        attestation_bytes = canonical_json(attestation)
        _write_create_only(
            output, ATTESTATION_NAME, attestation_bytes, 0o400
        )
        expected_files[ATTESTATION_NAME] = (attestation_bytes, 0o400)
        os.fsync(output.descriptor)
        os.fsync(output.parent_descriptor)
        _seal_owned_root(output)
        _validate_exact_output(output, expected_files)
        # This identity check is intentionally the final filesystem operation
        # before success; the attestation marker was the final created node.
        _close_success(output)
        return output.path / ATTESTATION_NAME
    except BaseException as error:
        try:
            _cleanup_owned_root(output)
        except FinalizationError as cleanup_error:
            raise cleanup_error from error
        raise


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--signature", type=Path, required=True)
    parser.add_argument("--expected-signature-sha256", required=True)
    parser.add_argument("--ssh-keygen-bin", type=Path, required=True)
    parser.add_argument("--expected-ssh-keygen-sha256", required=True)
    parser.add_argument("--allowed-signers", type=Path, required=True)
    parser.add_argument("--expected-allowed-signers-sha256", required=True)
    parser.add_argument("--revocation-file", type=Path, required=True)
    parser.add_argument("--expected-revocation-sha256", required=True)
    parser.add_argument("--principal", required=True)
    parser.add_argument("--expected-signer-fingerprint", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    return parser


def main() -> int:
    try:
        result = finalize(_parser().parse_args())
    except (
        FinalizationError,
        SigningError,
        receipt_checker.ReceiptError,
        OSError,
        UnicodeError,
        ValueError,
    ) as error:
        print(f"Sumeragi V2 replay finalization failed: {error}", file=sys.stderr)
        return 2
    print(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
