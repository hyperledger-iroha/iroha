#!/usr/bin/env python3
"""Build one canonical, validated Kagemusha production App Attest policy."""

from __future__ import annotations

import argparse
import base64
import hashlib
import os
from pathlib import Path
import stat
import sys
import tempfile
from typing import Optional


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
REPOSITORY_ROOT = SCRIPT_DIRECTORY.parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import kagemusha_candidate_ios_evidence as candidate_evidence
import kagemusha_production_ios_evidence as production_evidence


PRODUCTION_BUNDLE_ID = "org.hyperledger.iroha.kagemusha.appattestlab"
DEFAULT_APPLE_ROOT = REPOSITORY_ROOT / "certs/apple_app_attestation_root.der"


def _identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_gid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _read_trusted_root(path: Path) -> bytes:
    """Read one bounded immutable root owned by the caller or root."""

    try:
        absolute = path.resolve(strict=True)
        before = absolute.lstat()
    except OSError as error:
        raise candidate_evidence.EvidenceError(
            f"trusted App Attest root could not be resolved: {path}"
        ) from error
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_uid not in {0, os.geteuid()}
        or stat.S_IMODE(before.st_mode) & 0o022
        or not 1 <= before.st_size <= production_evidence.MAX_CERTIFICATE_BYTES
    ):
        raise candidate_evidence.EvidenceError(
            "trusted App Attest root must be a bounded, non-writable, singly linked regular file"
        )
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(absolute, flags)
    try:
        if _identity(os.fstat(descriptor)) != _identity(before):
            raise candidate_evidence.EvidenceError(
                "trusted App Attest root changed while opening"
            )
        payload = b""
        while len(payload) <= production_evidence.MAX_CERTIFICATE_BYTES:
            chunk = os.read(
                descriptor,
                production_evidence.MAX_CERTIFICATE_BYTES + 1 - len(payload),
            )
            if not chunk:
                break
            payload += chunk
        try:
            after = absolute.lstat()
        except OSError as error:
            raise candidate_evidence.EvidenceError(
                "trusted App Attest root disappeared while reading"
            ) from error
        if (
            len(payload) != before.st_size
            or _identity(os.fstat(descriptor)) != _identity(before)
            or _identity(after) != _identity(before)
        ):
            raise candidate_evidence.EvidenceError(
                "trusted App Attest root changed while reading"
            )
        return payload
    finally:
        os.close(descriptor)


def build_policy(
    *,
    policy_id: str,
    app_id_prefix: str,
    bundle_versions: list[str],
    validation_categories: list[int],
    trusted_root_paths: list[Path],
    revoked_certificate_tbs_sha256: list[str],
) -> dict[str, object]:
    if not trusted_root_paths or len(trusted_root_paths) > 4:
        raise candidate_evidence.EvidenceError(
            "one to four trusted App Attest roots are required"
        )
    for values, label in (
        (bundle_versions, "bundle versions"),
        (validation_categories, "validation categories"),
        (revoked_certificate_tbs_sha256, "revoked certificate TBS digests"),
    ):
        if len(values) != len(set(values)):
            raise candidate_evidence.EvidenceError(
                f"production policy {label} must not contain duplicates"
            )
    roots = []
    for path in trusted_root_paths:
        payload = _read_trusted_root(path)
        roots.append(
            {
                "der_base64": base64.b64encode(payload).decode("ascii"),
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
        )
    roots.sort(key=lambda value: value["sha256"])
    value: dict[str, object] = {
        "schema": production_evidence.PRODUCTION_POLICY_SCHEMA,
        "version": 1,
        "policy_id": policy_id,
        "app_id_prefix": app_id_prefix,
        "bundle_id": PRODUCTION_BUNDLE_ID,
        "environment": "production",
        "allowed_validation_categories": sorted(set(validation_categories)),
        "allowed_bundle_versions": sorted(set(bundle_versions)),
        "trusted_app_attest_roots": roots,
        "revoked_certificate_tbs_sha256": sorted(
            set(revoked_certificate_tbs_sha256)
        ),
        "x509_validation_profile": production_evidence.X509_VALIDATION_PROFILE,
        "secure_enclave_key_profile": production_evidence.SECURE_ENCLAVE_KEY_PROFILE,
    }
    payload = candidate_evidence.canonical_json_bytes(value)
    errors: list[str] = []
    production_evidence._validate_policy(value, payload, errors)
    if errors:
        raise candidate_evidence.EvidenceError(
            "production iOS policy is invalid: " + "; ".join(errors)
        )
    return value


def _fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def publish_new_policy(output: Path, value: dict[str, object]) -> None:
    if not output.is_absolute() or output.name in {"", ".", ".."}:
        raise candidate_evidence.EvidenceError(
            "production policy output must be an absolute file path"
        )
    try:
        parent = output.parent.resolve(strict=True)
    except OSError as error:
        raise candidate_evidence.EvidenceError(
            "production policy output parent could not be resolved"
        ) from error
    candidate_evidence._validate_private_directory(
        parent, "production policy output parent"
    )
    target = parent / output.name
    if target.exists() or target.is_symlink():
        raise candidate_evidence.EvidenceError(
            "production policy output already exists"
        )
    payload = candidate_evidence.canonical_json_bytes(value)
    descriptor, temporary_text = tempfile.mkstemp(
        prefix=f".{target.name}.", dir=os.fspath(parent)
    )
    temporary = Path(temporary_text)
    linked = False
    try:
        os.fchmod(descriptor, 0o600)
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise OSError("short production policy write")
            offset += written
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        snapshot = candidate_evidence._snapshot_private_file(
            temporary,
            "staged production policy",
            maximum=production_evidence.MAX_POLICY_BYTES,
            retain_payload=True,
        )
        parsed = candidate_evidence.parse_strict_json(
            snapshot.payload, "staged production policy"
        )
        errors: list[str] = []
        production_evidence._validate_policy(parsed, snapshot.payload, errors)
        if errors:
            raise candidate_evidence.EvidenceError(
                "staged production policy failed validation: " + "; ".join(errors)
            )
        os.link(temporary, target, follow_symlinks=False)
        linked = True
        _fsync_directory(parent)
        temporary.unlink()
        _fsync_directory(parent)
        published = candidate_evidence._snapshot_private_file(
            target,
            "published production policy",
            maximum=production_evidence.MAX_POLICY_BYTES,
            retain_payload=True,
        )
        if published.payload != payload:
            raise candidate_evidence.EvidenceError(
                "published production policy bytes changed"
            )
    except FileExistsError as error:
        raise candidate_evidence.EvidenceError(
            "production policy output already exists"
        ) from error
    except candidate_evidence.EvidenceError:
        raise
    except OSError as error:
        message = (
            "production policy publication commit state is uncertain"
            if linked
            else "production policy could not be published"
        )
        raise candidate_evidence.EvidenceError(message) from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy-id", required=True)
    parser.add_argument("--app-id-prefix", required=True)
    parser.add_argument("--bundle-version", action="append", required=True)
    parser.add_argument(
        "--validation-category", action="append", required=True, type=int
    )
    parser.add_argument(
        "--trusted-root-der", action="append", type=Path, default=[]
    )
    parser.add_argument(
        "--revoked-certificate-tbs-sha256",
        action="append",
        default=[],
        help="SHA-256 of the exact raw DER encoding of a revoked TBSCertificate",
    )
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        value = build_policy(
            policy_id=args.policy_id,
            app_id_prefix=args.app_id_prefix,
            bundle_versions=args.bundle_version,
            validation_categories=args.validation_category,
            trusted_root_paths=args.trusted_root_der or [DEFAULT_APPLE_ROOT],
            revoked_certificate_tbs_sha256=args.revoked_certificate_tbs_sha256,
        )
        publish_new_policy(args.output, value)
    except candidate_evidence.EvidenceError as error:
        print(f"[kagemusha-production-ios-policy] ERROR: {error}", file=sys.stderr)
        return 1
    print(f"[kagemusha-production-ios-policy] production policy: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
