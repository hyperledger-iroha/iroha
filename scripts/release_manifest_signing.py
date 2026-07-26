#!/usr/bin/env python3
"""Sign aggregate release manifests and verify them with ``sorafs-validate``.

The production signing command never accepts a private key. It invokes a
reviewed external signer with two positional arguments:

    external-signer MANIFEST_PATH NEW_RAW_SIGNATURE_PATH

The signer must create exactly one raw 64-byte Ed25519 signature. The wrapper
executes an owner-private snapshot of the inspected signer against an
owner-private snapshot of the canonical manifest, so path substitution cannot
select a different executable or signing payload. Verification is delegated to
the canonical native contract:

    sorafs-validate release-manifest \
      --manifest MANIFEST_PATH \
      --public-key RAW_32_BYTE_PUBLIC_KEY_PATH \
      --public-key-fingerprint REVIEWED_SHA256 \
      --signature RAW_64_BYTE_SIGNATURE_PATH

The native verifier must be supplied by direct path together with an
independently reviewed SHA-256 digest. This wrapper snapshots the exact
executable into an owner-private directory, invokes that snapshot, and rejects
path, permission, hard-link, digest, or identity drift. The verifier receives
owner-private snapshots of the inspected manifest, key, and signature under a
minimal environment, never their mutable source paths.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Dict, List, Optional, Tuple


ED25519_PUBLIC_KEY_SIZE = 32
ED25519_SIGNATURE_SIZE = 64
ED25519_FIELD_MODULUS = (1 << 255) - 19
ED25519_SCALAR_ORDER = (1 << 252) + 27742317777372353535851937790883648493
MAX_MANIFEST_SIZE = 1024 * 1024
SHA256_RE = re.compile(r"[0-9a-f]{64}")
NATIVE_VERIFIER_PROTOCOL = "sorafs-validate-release-manifest-v1"


class ReleaseManifestSignatureError(RuntimeError):
    """Raised when aggregate release-manifest signing or verification fails."""


FileIdentity = Tuple[int, int, int, int, int, int]


def _absolute(path: Path) -> Path:
    return Path(os.path.abspath(os.fspath(path)))


def _identity(metadata: os.stat_result) -> FileIdentity:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
        metadata.st_nlink,
    )


def _reject_symlink_chain(
    path: Path,
    label: str,
    *,
    leaf_may_be_missing: bool = False,
) -> None:
    components = list(reversed(path.parents)) + [path]
    for index, component in enumerate(components):
        try:
            metadata = component.lstat()
        except FileNotFoundError as exc:
            if leaf_may_be_missing and index == len(components) - 1:
                return
            raise ReleaseManifestSignatureError(
                f"{label} path component is missing: {component}"
            ) from exc
        except OSError as exc:
            raise ReleaseManifestSignatureError(
                f"cannot inspect {label} path component {component}: {exc}"
            ) from exc
        if stat.S_ISLNK(metadata.st_mode):
            raise ReleaseManifestSignatureError(
                f"{label} must not contain a symlink path component: {component}"
            )


def _inspect_regular(
    path: Path,
    label: str,
    *,
    executable: bool = False,
) -> os.stat_result:
    _reject_symlink_chain(path, label)
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise ReleaseManifestSignatureError(f"cannot inspect {label}: {exc}") from exc
    if not stat.S_ISREG(metadata.st_mode):
        raise ReleaseManifestSignatureError(f"{label} must be a regular file")
    if metadata.st_nlink != 1:
        raise ReleaseManifestSignatureError(f"{label} must have exactly one hard link")
    if metadata.st_mode & 0o022:
        raise ReleaseManifestSignatureError(
            f"{label} must not be group- or world-writable"
        )
    allowed_owners = {os.getuid(), 0} if hasattr(os, "getuid") else {metadata.st_uid}
    if metadata.st_uid not in allowed_owners:
        raise ReleaseManifestSignatureError(
            f"{label} must be owned by the invoking user or root"
        )
    if executable and not os.access(path, os.X_OK):
        raise ReleaseManifestSignatureError(f"{label} must be executable")
    return metadata


def _stable_read(
    path: Path,
    label: str,
    *,
    exact_size: Optional[int] = None,
    max_size: Optional[int] = None,
) -> Tuple[bytes, FileIdentity]:
    before = _inspect_regular(path, label)
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise ReleaseManifestSignatureError(f"cannot open {label}: {exc}") from exc

    opened: Optional[os.stat_result] = None
    closed: Optional[os.stat_result] = None
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise ReleaseManifestSignatureError(f"{label} changed while it was opened")
        chunks: List[bytes] = []
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if exact_size is not None and total > exact_size:
                raise ReleaseManifestSignatureError(
                    f"{label} must contain exactly {exact_size} raw bytes"
                )
            if max_size is not None and total > max_size:
                raise ReleaseManifestSignatureError(
                    f"{label} exceeds the {max_size}-byte size limit"
                )
        closed = os.fstat(descriptor)
    finally:
        os.close(descriptor)

    assert opened is not None
    assert closed is not None
    opened_identity = _identity(opened)
    if opened_identity != _identity(closed):
        raise ReleaseManifestSignatureError(f"{label} changed while it was read")
    payload = b"".join(chunks)
    if exact_size is not None and len(payload) != exact_size:
        raise ReleaseManifestSignatureError(
            f"{label} must contain exactly {exact_size} raw bytes"
        )
    return payload, opened_identity


def _assert_unchanged(
    path: Path,
    label: str,
    expected_payload: bytes,
    expected_identity: FileIdentity,
    *,
    exact_size: Optional[int] = None,
    max_size: Optional[int] = None,
) -> None:
    payload, identity = _stable_read(
        path,
        label,
        exact_size=exact_size,
        max_size=max_size,
    )
    if payload != expected_payload or identity != expected_identity:
        raise ReleaseManifestSignatureError(f"{label} changed during verification")


def _stable_digest(
    path: Path,
    label: str,
    *,
    executable: bool = False,
) -> Tuple[str, FileIdentity]:
    before = _inspect_regular(path, label, executable=executable)
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise ReleaseManifestSignatureError(f"cannot open {label}: {exc}") from exc

    digest = hashlib.sha256()
    opened: Optional[os.stat_result] = None
    closed: Optional[os.stat_result] = None
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise ReleaseManifestSignatureError(f"{label} changed while it was opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        closed = os.fstat(descriptor)
    finally:
        os.close(descriptor)

    assert opened is not None
    assert closed is not None
    opened_identity = _identity(opened)
    if opened_identity != _identity(closed):
        raise ReleaseManifestSignatureError(f"{label} changed while it was hashed")
    return digest.hexdigest(), opened_identity


def _assert_digest_unchanged(
    path: Path,
    label: str,
    expected_digest: str,
    expected_identity: FileIdentity,
    *,
    executable: bool = False,
) -> None:
    digest, identity = _stable_digest(path, label, executable=executable)
    if digest != expected_digest or identity != expected_identity:
        raise ReleaseManifestSignatureError(f"{label} changed during execution")


def _validate_sha256(value: str, label: str) -> None:
    if SHA256_RE.fullmatch(value) is None:
        raise ReleaseManifestSignatureError(
            f"{label} must be exactly 64 lowercase hexadecimal characters"
        )


def _validate_ed25519_public_key_encoding(raw_public_key: bytes) -> None:
    """Reject encodings that cannot be canonical compressed Edwards points."""

    if len(raw_public_key) != ED25519_PUBLIC_KEY_SIZE:
        raise ReleaseManifestSignatureError(
            "Ed25519 public key must contain exactly 32 raw bytes"
        )
    if not any(raw_public_key):
        raise ReleaseManifestSignatureError("Ed25519 public key must not be all zero")
    encoded_y = bytearray(raw_public_key)
    encoded_y[-1] &= 0x7F
    if int.from_bytes(encoded_y, "little") >= ED25519_FIELD_MODULUS:
        raise ReleaseManifestSignatureError(
            "Ed25519 public key has a non-canonical point encoding"
        )


def _validate_ed25519_signature_encoding(signature: bytes, label: str) -> None:
    """Reject malformed Ed25519 encodings before invoking the native verifier."""

    if len(signature) != ED25519_SIGNATURE_SIZE:
        raise ReleaseManifestSignatureError(
            f"{label} must contain exactly {ED25519_SIGNATURE_SIZE} raw bytes"
        )
    if not any(signature):
        raise ReleaseManifestSignatureError(f"{label} must not be all zero")

    encoded_r = bytearray(signature[:ED25519_PUBLIC_KEY_SIZE])
    encoded_r[-1] &= 0x7F
    if int.from_bytes(encoded_r, "little") >= ED25519_FIELD_MODULUS:
        raise ReleaseManifestSignatureError(
            f"{label} has a non-canonical Ed25519 R encoding"
        )
    scalar = int.from_bytes(signature[ED25519_PUBLIC_KEY_SIZE:], "little")
    if scalar >= ED25519_SCALAR_ORDER:
        raise ReleaseManifestSignatureError(
            f"{label} has a non-canonical Ed25519 scalar"
        )


def _validate_canonical_manifest(manifest_payload: bytes) -> None:
    def reject_non_finite(value: str) -> None:
        raise ValueError(f"non-finite JSON number {value}")

    try:
        manifest = json.loads(
            manifest_payload,
            parse_constant=reject_non_finite,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as exc:
        raise ReleaseManifestSignatureError(
            f"aggregate release manifest is not valid strict JSON: {exc}"
        ) from exc
    if not isinstance(manifest, dict):
        raise ReleaseManifestSignatureError(
            "aggregate release manifest root must be an object"
        )
    canonical = (
        json.dumps(manifest, indent=2, sort_keys=True, allow_nan=False) + "\n"
    ).encode("utf-8")
    if manifest_payload != canonical:
        raise ReleaseManifestSignatureError(
            "aggregate release manifest is not canonical deterministic JSON"
        )


def _require_new_output(path: Path, label: str) -> None:
    _reject_symlink_chain(path.parent, f"{label} parent")
    if not path.parent.is_dir():
        raise ReleaseManifestSignatureError(f"{label} parent must be a directory")
    try:
        path.lstat()
    except FileNotFoundError:
        return
    except OSError as exc:
        raise ReleaseManifestSignatureError(f"cannot inspect {label}: {exc}") from exc
    raise ReleaseManifestSignatureError(f"{label} already exists")


def _install_exclusive(
    path: Path,
    payload: bytes,
    label: str,
    *,
    mode: int = 0o644,
) -> FileIdentity:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, mode)
    except OSError as exc:
        raise ReleaseManifestSignatureError(f"cannot create {label}: {exc}") from exc
    installed: Optional[os.stat_result] = None
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise ReleaseManifestSignatureError(
                    f"short write while creating {label}"
                )
            view = view[written:]
        os.fsync(descriptor)
        installed = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    assert installed is not None
    return _identity(installed)


def _unlink_if_identity(path: Path, expected_identity: FileIdentity) -> None:
    try:
        metadata = path.lstat()
    except OSError:
        return
    if stat.S_ISREG(metadata.st_mode) and _identity(metadata) == expected_identity:
        try:
            path.unlink()
        except OSError:
            pass


def _snapshot_executable(
    source: Path,
    destination: Path,
    expected_sha256: str,
    *,
    label: str,
    mismatch_message: str,
) -> Tuple[str, FileIdentity]:
    before = _inspect_regular(
        source,
        label,
        executable=True,
    )
    snapshot_label = f"{label} snapshot"
    _require_new_output(destination, snapshot_label)
    read_flags = os.O_RDONLY
    write_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        read_flags |= os.O_NOFOLLOW
        write_flags |= os.O_NOFOLLOW
    read_descriptor = -1
    write_descriptor = -1
    snapshot_identity: Optional[FileIdentity] = None
    try:
        read_descriptor = os.open(source, read_flags)
        opened = os.fstat(read_descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise ReleaseManifestSignatureError(
                f"{label} changed while it was opened"
            )
        write_descriptor = os.open(destination, write_flags, 0o700)
        snapshot_identity = _identity(os.fstat(write_descriptor))
        digest = hashlib.sha256()
        while True:
            chunk = os.read(read_descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(write_descriptor, view)
                if written <= 0:
                    raise ReleaseManifestSignatureError(
                        f"short write while snapshotting {label}"
                    )
                view = view[written:]
        os.fsync(write_descriptor)
        closed = os.fstat(read_descriptor)
        if _identity(opened) != _identity(closed):
            raise ReleaseManifestSignatureError(
                f"{label} changed while it was snapshotted"
            )
        actual_sha256 = digest.hexdigest()
        if actual_sha256 != expected_sha256:
            raise ReleaseManifestSignatureError(mismatch_message)
    except BaseException:
        if snapshot_identity is not None:
            _unlink_if_identity(destination, snapshot_identity)
        raise
    finally:
        if read_descriptor >= 0:
            os.close(read_descriptor)
        if write_descriptor >= 0:
            os.close(write_descriptor)
    return actual_sha256, _identity(opened)


def _snapshot_native_verifier(
    source: Path,
    destination: Path,
    trusted_sha256: str,
) -> Tuple[str, FileIdentity]:
    return _snapshot_executable(
        source,
        destination,
        trusted_sha256,
        label="native release-manifest verifier",
        mismatch_message=(
            "native release-manifest verifier does not match the reviewed SHA256"
        ),
    )


def _native_snapshot_path(temp_dir: Path, source: Path) -> Path:
    suffix = ".exe" if source.suffix.lower() == ".exe" else ""
    return temp_dir / f"sorafs-validate-pinned{suffix}"


def _signer_snapshot_path(temp_dir: Path, source: Path) -> Path:
    suffix = ".exe" if source.suffix.lower() == ".exe" else ""
    return temp_dir / f"external-ed25519-signer-pinned{suffix}"


def _native_verifier_environment() -> Dict[str, str]:
    """Return the minimal environment accepted by the native verifier."""

    environment = {"PATH": os.defpath}
    if os.name == "nt":
        for key in ("SYSTEMROOT", "WINDIR"):
            value = os.environ.get(key)
            if value:
                environment[key] = value
    return environment


def _invoke_native_verifier(
    verifier: Path,
    manifest: Path,
    public_key: Path,
    fingerprint: str,
    signature: Path,
) -> None:
    try:
        completed = subprocess.run(
            [
                str(verifier),
                "release-manifest",
                "--manifest",
                str(manifest),
                "--public-key",
                str(public_key),
                "--public-key-fingerprint",
                fingerprint,
                "--signature",
                str(signature),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
            timeout=120,
            env=_native_verifier_environment(),
        )
    except subprocess.TimeoutExpired as exc:
        raise ReleaseManifestSignatureError(
            "native release-manifest verifier timed out"
        ) from exc
    except OSError as exc:
        raise ReleaseManifestSignatureError(
            f"cannot execute native release-manifest verifier: {exc}"
        ) from exc
    if completed.returncode != 0:
        raise ReleaseManifestSignatureError(
            "native release-manifest Ed25519 verification failed "
            f"with status {completed.returncode}"
        )


def _read_verification_inputs(
    manifest: Path,
    signature_file: Path,
    public_key_file: Path,
    trusted_fingerprint: str,
) -> Tuple[
    bytes,
    FileIdentity,
    bytes,
    FileIdentity,
    bytes,
    FileIdentity,
]:
    manifest_payload, manifest_identity = _stable_read(
        manifest,
        "aggregate release manifest",
        max_size=MAX_MANIFEST_SIZE,
    )
    _validate_canonical_manifest(manifest_payload)
    signature, signature_identity = _stable_read(
        signature_file,
        "aggregate release-manifest signature",
        exact_size=ED25519_SIGNATURE_SIZE,
    )
    _validate_ed25519_signature_encoding(
        signature,
        "aggregate release-manifest signature",
    )
    raw_public_key, public_key_identity = _stable_read(
        public_key_file,
        "aggregate release-manifest raw public key",
        exact_size=ED25519_PUBLIC_KEY_SIZE,
    )
    _validate_ed25519_public_key_encoding(raw_public_key)
    actual_fingerprint = hashlib.sha256(raw_public_key).hexdigest()
    if actual_fingerprint != trusted_fingerprint:
        raise ReleaseManifestSignatureError(
            "aggregate release-manifest public key does not match the reviewed fingerprint"
        )
    return (
        manifest_payload,
        manifest_identity,
        signature,
        signature_identity,
        raw_public_key,
        public_key_identity,
    )


def _verify_bytes_with_pinned_native(
    *,
    temp_parent: Path,
    native_verifier: Path,
    trusted_verifier_sha256: str,
    manifest_payload: bytes,
    raw_public_key: bytes,
    trusted_fingerprint: str,
    signature: bytes,
) -> str:
    """Verify immutable snapshots of the already-inspected input bytes."""

    with tempfile.TemporaryDirectory(
        prefix="iroha-release-manifest-native-verify-",
        dir=str(temp_parent),
    ) as temp_raw:
        temp_dir = Path(temp_raw)
        manifest_snapshot = temp_dir / "release_manifest.json"
        public_key_snapshot = temp_dir / "release_manifest.ed25519.pub"
        signature_snapshot = temp_dir / "release_manifest.ed25519.sig"
        manifest_snapshot_identity = _install_exclusive(
            manifest_snapshot,
            manifest_payload,
            "native manifest snapshot",
            mode=0o400,
        )
        public_key_snapshot_identity = _install_exclusive(
            public_key_snapshot,
            raw_public_key,
            "native public-key snapshot",
            mode=0o400,
        )
        signature_snapshot_identity = _install_exclusive(
            signature_snapshot,
            signature,
            "native signature snapshot",
            mode=0o400,
        )

        verifier_snapshot = _native_snapshot_path(temp_dir, native_verifier)
        verifier_digest, verifier_source_identity = _snapshot_native_verifier(
            native_verifier,
            verifier_snapshot,
            trusted_verifier_sha256,
        )
        snapshot_digest, verifier_snapshot_identity = _stable_digest(
            verifier_snapshot,
            "native verifier snapshot",
            executable=True,
        )
        if snapshot_digest != verifier_digest:
            raise ReleaseManifestSignatureError(
                "native verifier snapshot does not match the reviewed executable"
            )

        _invoke_native_verifier(
            verifier_snapshot,
            manifest_snapshot,
            public_key_snapshot,
            trusted_fingerprint,
            signature_snapshot,
        )
        _assert_digest_unchanged(
            verifier_snapshot,
            "native verifier snapshot",
            snapshot_digest,
            verifier_snapshot_identity,
            executable=True,
        )
        _assert_digest_unchanged(
            native_verifier,
            "native release-manifest verifier",
            verifier_digest,
            verifier_source_identity,
            executable=True,
        )
        _assert_unchanged(
            manifest_snapshot,
            "native manifest snapshot",
            manifest_payload,
            manifest_snapshot_identity,
            max_size=MAX_MANIFEST_SIZE,
        )
        _assert_unchanged(
            public_key_snapshot,
            "native public-key snapshot",
            raw_public_key,
            public_key_snapshot_identity,
            exact_size=ED25519_PUBLIC_KEY_SIZE,
        )
        _assert_unchanged(
            signature_snapshot,
            "native signature snapshot",
            signature,
            signature_snapshot_identity,
            exact_size=ED25519_SIGNATURE_SIZE,
        )
    return verifier_digest


def verify_release_manifest(
    manifest_path: Path,
    signature_path: Path,
    public_key_path: Path,
    trusted_fingerprint: str,
    release_manifest_verifier_path: Path,
    trusted_release_manifest_verifier_sha256: str,
) -> Dict[str, object]:
    """Verify an aggregate manifest through the pinned native verifier."""

    _validate_sha256(trusted_fingerprint, "trusted signing fingerprint")
    _validate_sha256(
        trusted_release_manifest_verifier_sha256,
        "trusted native verifier SHA256",
    )
    manifest = _absolute(manifest_path)
    signature_file = _absolute(signature_path)
    public_key_file = _absolute(public_key_path)
    native_verifier = _absolute(release_manifest_verifier_path)
    if len({manifest, signature_file, public_key_file, native_verifier}) != 4:
        raise ReleaseManifestSignatureError(
            "manifest, signature, public key, and native verifier paths must be distinct"
        )

    (
        manifest_payload,
        manifest_identity,
        signature,
        signature_identity,
        raw_public_key,
        public_key_identity,
    ) = _read_verification_inputs(
        manifest,
        signature_file,
        public_key_file,
        trusted_fingerprint,
    )

    verifier_digest = _verify_bytes_with_pinned_native(
        temp_parent=manifest.parent,
        native_verifier=native_verifier,
        trusted_verifier_sha256=trusted_release_manifest_verifier_sha256,
        manifest_payload=manifest_payload,
        raw_public_key=raw_public_key,
        trusted_fingerprint=trusted_fingerprint,
        signature=signature,
    )

    _assert_unchanged(
        manifest,
        "aggregate release manifest",
        manifest_payload,
        manifest_identity,
        max_size=MAX_MANIFEST_SIZE,
    )
    _assert_unchanged(
        signature_file,
        "aggregate release-manifest signature",
        signature,
        signature_identity,
        exact_size=ED25519_SIGNATURE_SIZE,
    )
    _assert_unchanged(
        public_key_file,
        "aggregate release-manifest raw public key",
        raw_public_key,
        public_key_identity,
        exact_size=ED25519_PUBLIC_KEY_SIZE,
    )

    return {
        "manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
        "manifest_size": len(manifest_payload),
        "signature_algorithm": "ed25519",
        "public_key_format": "raw-ed25519-32",
        "signer_fingerprint_sha256": trusted_fingerprint,
        "signature_verified": True,
        "native_verifier_protocol": NATIVE_VERIFIER_PROTOCOL,
        "native_verifier_path": str(native_verifier),
        "native_verifier_sha256": verifier_digest,
    }


def sign_release_manifest(
    manifest_path: Path,
    external_signer: Path,
    raw_public_key_path: Path,
    trusted_fingerprint: str,
    signature_output_path: Path,
    public_key_output_path: Path,
    release_manifest_verifier_path: Path,
    trusted_release_manifest_verifier_sha256: str,
    verification_summary_output_path: Optional[Path] = None,
) -> Dict[str, object]:
    """Sign via a pinned external signer and verify through ``sorafs-validate``."""

    _validate_sha256(trusted_fingerprint, "trusted signing fingerprint")
    _validate_sha256(
        trusted_release_manifest_verifier_sha256,
        "trusted native verifier SHA256",
    )
    manifest = _absolute(manifest_path)
    signer = _absolute(external_signer)
    raw_public_key_file = _absolute(raw_public_key_path)
    signature_output = _absolute(signature_output_path)
    public_key_output = _absolute(public_key_output_path)
    verification_summary_output = (
        _absolute(verification_summary_output_path)
        if verification_summary_output_path is not None
        else None
    )
    native_verifier = _absolute(release_manifest_verifier_path)

    release_outputs = {signature_output, public_key_output}
    if verification_summary_output is not None:
        release_outputs.add(verification_summary_output)
    if len(release_outputs) != (
        3 if verification_summary_output is not None else 2
    ):
        raise ReleaseManifestSignatureError(
            "signature, public-key, and verification-summary outputs "
            "must be different paths"
        )
    signing_inputs = {manifest, signer, raw_public_key_file, native_verifier}
    outputs = [
        (signature_output, "aggregate signature output"),
        (public_key_output, "aggregate public-key output"),
    ]
    if verification_summary_output is not None:
        outputs.append(
            (
                verification_summary_output,
                "aggregate verification-summary output",
            )
        )
    for output, label in outputs:
        if output in signing_inputs:
            raise ReleaseManifestSignatureError(
                f"{label} must not overwrite a signing input"
            )
        _require_new_output(output, label)

    signer_digest, signer_identity = _stable_digest(
        signer,
        "external signer",
        executable=True,
    )
    raw_public_key, raw_public_key_identity = _stable_read(
        raw_public_key_file,
        "raw Ed25519 public key",
        exact_size=ED25519_PUBLIC_KEY_SIZE,
    )
    _validate_ed25519_public_key_encoding(raw_public_key)
    actual_fingerprint = hashlib.sha256(raw_public_key).hexdigest()
    if actual_fingerprint != trusted_fingerprint:
        raise ReleaseManifestSignatureError(
            "raw Ed25519 public key does not match the reviewed fingerprint"
        )
    manifest_payload, manifest_identity = _stable_read(
        manifest,
        "aggregate release manifest",
        max_size=MAX_MANIFEST_SIZE,
    )
    _validate_canonical_manifest(manifest_payload)

    with tempfile.TemporaryDirectory(
        prefix="iroha-release-manifest-sign-",
        dir=str(manifest.parent),
    ) as signer_temp_raw:
        signer_temp = Path(signer_temp_raw)
        signature_temp = signer_temp / "release_manifest.json.sig"
        signer_manifest_snapshot = signer_temp / "release_manifest.json"
        signer_manifest_snapshot_identity = _install_exclusive(
            signer_manifest_snapshot,
            manifest_payload,
            "external signer manifest snapshot",
            mode=0o600,
        )
        signer_snapshot = _signer_snapshot_path(signer_temp, signer)
        snapshot_digest, signer_snapshot_source_identity = _snapshot_executable(
            signer,
            signer_snapshot,
            signer_digest,
            label="external signer",
            mismatch_message="external signer changed before it could be snapshotted",
        )
        if signer_snapshot_source_identity != signer_identity:
            raise ReleaseManifestSignatureError(
                "external signer changed before it could be snapshotted"
            )
        signer_snapshot_digest, signer_snapshot_identity = _stable_digest(
            signer_snapshot,
            "external signer snapshot",
            executable=True,
        )
        if signer_snapshot_digest != snapshot_digest:
            raise ReleaseManifestSignatureError(
                "external signer snapshot does not match the inspected executable"
            )
        try:
            completed = subprocess.run(
                [
                    str(signer_snapshot),
                    str(signer_manifest_snapshot),
                    str(signature_temp),
                ],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
                timeout=120,
            )
        except subprocess.TimeoutExpired as exc:
            raise ReleaseManifestSignatureError(
                "external Ed25519 signer timed out"
            ) from exc
        except OSError as exc:
            raise ReleaseManifestSignatureError(
                f"cannot execute external Ed25519 signer: {exc}"
            ) from exc
        if completed.returncode != 0:
            raise ReleaseManifestSignatureError(
                f"external Ed25519 signer exited with status {completed.returncode}"
            )
        signature, signature_temp_identity = _stable_read(
            signature_temp,
            "external aggregate Ed25519 signature",
            exact_size=ED25519_SIGNATURE_SIZE,
        )
        _validate_ed25519_signature_encoding(
            signature,
            "external aggregate Ed25519 signature",
        )

        _assert_unchanged(
            manifest,
            "aggregate release manifest",
            manifest_payload,
            manifest_identity,
            max_size=MAX_MANIFEST_SIZE,
        )
        _assert_unchanged(
            signer_manifest_snapshot,
            "external signer manifest snapshot",
            manifest_payload,
            signer_manifest_snapshot_identity,
            max_size=MAX_MANIFEST_SIZE,
        )
        _assert_digest_unchanged(
            signer_snapshot,
            "external signer snapshot",
            signer_snapshot_digest,
            signer_snapshot_identity,
            executable=True,
        )
        _assert_digest_unchanged(
            signer,
            "external signer",
            signer_digest,
            signer_identity,
            executable=True,
        )
        _assert_unchanged(
            raw_public_key_file,
            "raw Ed25519 public key",
            raw_public_key,
            raw_public_key_identity,
            exact_size=ED25519_PUBLIC_KEY_SIZE,
        )

        verifier_digest = _verify_bytes_with_pinned_native(
            temp_parent=manifest.parent,
            native_verifier=native_verifier,
            trusted_verifier_sha256=trusted_release_manifest_verifier_sha256,
            manifest_payload=manifest_payload,
            raw_public_key=raw_public_key,
            trusted_fingerprint=trusted_fingerprint,
            signature=signature,
        )
        _assert_unchanged(
            manifest,
            "aggregate release manifest",
            manifest_payload,
            manifest_identity,
            max_size=MAX_MANIFEST_SIZE,
        )
        _assert_unchanged(
            raw_public_key_file,
            "raw Ed25519 public key",
            raw_public_key,
            raw_public_key_identity,
            exact_size=ED25519_PUBLIC_KEY_SIZE,
        )
        _assert_unchanged(
            signature_temp,
            "external aggregate Ed25519 signature",
            signature,
            signature_temp_identity,
            exact_size=ED25519_SIGNATURE_SIZE,
        )

        installed: List[Tuple[Path, FileIdentity]] = []
        try:
            public_identity = _install_exclusive(
                public_key_output,
                raw_public_key,
                "aggregate raw public-key output",
            )
            installed.append((public_key_output, public_identity))
            signature_identity = _install_exclusive(
                signature_output,
                signature,
                "aggregate signature output",
            )
            installed.append((signature_output, signature_identity))
            verification = verify_release_manifest(
                manifest,
                signature_output,
                public_key_output,
                trusted_fingerprint,
                native_verifier,
                trusted_release_manifest_verifier_sha256,
            )
            verification.update(
                {
                    "manifest": str(manifest),
                    "signature": str(signature_output),
                    "public_key": str(public_key_output),
                }
            )
            if verification_summary_output is not None:
                verification_payload = (
                    json.dumps(
                        verification,
                        indent=2,
                        sort_keys=True,
                        allow_nan=False,
                    )
                    + "\n"
                ).encode("utf-8")
                verification_identity = _install_exclusive(
                    verification_summary_output,
                    verification_payload,
                    "aggregate verification-summary output",
                    mode=0o600,
                )
                installed.append(
                    (verification_summary_output, verification_identity)
                )
        except BaseException:
            for installed_path, installed_identity in reversed(installed):
                _unlink_if_identity(installed_path, installed_identity)
            raise

    return verification


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    sign = subparsers.add_parser(
        "sign",
        help="Sign an aggregate manifest via a reviewed external Ed25519 signer",
    )
    sign.add_argument("--manifest", required=True)
    sign.add_argument("--external-signer", required=True)
    sign.add_argument("--signing-public-key", required=True)
    sign.add_argument("--trusted-signing-fingerprint", required=True)
    sign.add_argument("--signature-output", required=True)
    sign.add_argument("--public-key-output", required=True)
    sign.add_argument("--verification-summary-output")
    sign.add_argument("--release-manifest-verifier", required=True)
    sign.add_argument("--trusted-release-manifest-verifier-sha256", required=True)

    verify = subparsers.add_parser(
        "verify",
        help="Verify an aggregate manifest through the pinned native verifier",
    )
    verify.add_argument("--manifest", required=True)
    verify.add_argument("--signature", required=True)
    verify.add_argument("--public-key", required=True)
    verify.add_argument("--trusted-signing-fingerprint", required=True)
    verify.add_argument("--release-manifest-verifier", required=True)
    verify.add_argument("--trusted-release-manifest-verifier-sha256", required=True)
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        if args.command == "sign":
            result = sign_release_manifest(
                Path(args.manifest),
                Path(args.external_signer),
                Path(args.signing_public_key),
                args.trusted_signing_fingerprint,
                Path(args.signature_output),
                Path(args.public_key_output),
                Path(args.release_manifest_verifier),
                args.trusted_release_manifest_verifier_sha256,
                (
                    Path(args.verification_summary_output)
                    if args.verification_summary_output is not None
                    else None
                ),
            )
        else:
            result = verify_release_manifest(
                Path(args.manifest),
                Path(args.signature),
                Path(args.public_key),
                args.trusted_signing_fingerprint,
                Path(args.release_manifest_verifier),
                args.trusted_release_manifest_verifier_sha256,
            )
    except ReleaseManifestSignatureError as exc:
        print(f"release manifest signing error: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
