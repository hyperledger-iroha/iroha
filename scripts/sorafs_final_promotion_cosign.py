"""Verify exact promotion subject bytes with independently pinned Sigstore tooling and trust.

The caller owns subject construction. This boundary accepts only the canonical Sigstore v0.3
bundle profile and never discovers trust from that bundle or from a verifier's default sources.
Successful cryptographic verification alone does not qualify hardware custody or promotion.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import tempfile
from pathlib import Path

from cryptography import x509
from cryptography.exceptions import UnsupportedAlgorithm
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.serialization import Encoding

from check_sorafs_production_readiness import canonical_public_provenance_url
from sorafs_evidence_json import decode_evidence_json, read_evidence_bytes
from sorafs_path_identity import resolve_path_identity
import sorafs_verifier_process as verifier_process

COSIGN_BUNDLE_MEDIA_TYPE = "application/vnd.dev.sigstore.bundle.v0.3+json"
MAX_SUBJECT_BYTES = 256 * 1024
MAX_BUNDLE_BYTES = 16 * 1024 * 1024
MAX_TRUSTED_ROOT_BYTES = 256 * 1024
FAILURE = "production promotion cosign verification failed for the exact subject and independent trust"


def validate_cosign_bundle(bundle: bytes) -> None:
    """Admit one leaf-certificate/message-signature profile; cosign owns proof semantics."""

    if not isinstance(bundle, bytes) or not 0 < len(bundle) <= MAX_BUNDLE_BYTES:
        raise ValueError("invalid cosign bundle size")
    payload = decode_evidence_json(bundle)
    if set(payload) != {"mediaType", "verificationMaterial", "messageSignature"} or payload.get("mediaType") != COSIGN_BUNDLE_MEDIA_TYPE:
        raise ValueError("unsupported cosign bundle format")
    material = payload["verificationMaterial"]
    if (
        not isinstance(material, dict)
        or set(material) != {"certificate", "tlogEntries", "timestampVerificationData"}
    ):
        raise ValueError("invalid cosign verification material profile")
    certificate, entries = material["certificate"], material["tlogEntries"]
    if not isinstance(certificate, dict) or set(certificate) != {"rawBytes"}:
        raise ValueError("invalid cosign leaf certificate profile")
    certificate_der = _canonical_base64(certificate["rawBytes"])
    try:
        leaf = x509.load_der_x509_certificate(certificate_der)
        public_key = leaf.public_key()
    except (ValueError, UnsupportedAlgorithm, x509.InvalidVersion):
        raise ValueError("invalid cosign leaf certificate") from None
    if (
        leaf.public_bytes(Encoding.DER) != certificate_der
        or not isinstance(public_key, ec.EllipticCurvePublicKey)
        or not isinstance(public_key.curve, ec.SECP256R1)
    ):
        raise ValueError("invalid cosign P256 leaf certificate")
    if (
        not isinstance(entries, list) or len(entries) != 1
        or not isinstance(entries[0], dict)
        or set(entries[0]) != {"logIndex", "logId", "kindVersion", "inclusionProof", "canonicalizedBody"}
        or entries[0].get("kindVersion") != {"kind": "hashedrekord", "version": "0.0.2"}
    ):
        raise ValueError("invalid cosign transparency entry profile")
    timestamps = material["timestampVerificationData"]
    if not isinstance(timestamps, dict) or set(timestamps) != {"rfc3161Timestamps"}:
        raise ValueError("invalid cosign timestamp material profile")
    signed_timestamps = timestamps["rfc3161Timestamps"]
    if not isinstance(signed_timestamps, list) or len(signed_timestamps) != 1:
        raise ValueError("invalid cosign signed timestamp list")
    for timestamp in signed_timestamps:
        if not isinstance(timestamp, dict) or set(timestamp) != {"signedTimestamp"}:
            raise ValueError("invalid cosign signed timestamp profile")
        _canonical_base64(timestamp["signedTimestamp"])
    signature = payload["messageSignature"]
    if not isinstance(signature, dict) or set(signature) != {"messageDigest", "signature"}:
        raise ValueError("invalid cosign message signature profile")
    _canonical_base64(signature["signature"])
    digest = signature["messageDigest"]
    if not isinstance(digest, dict) or set(digest) != {"algorithm", "digest"} or digest["algorithm"] != "SHA2_256":
        raise ValueError("invalid cosign message digest profile")
    if len(_canonical_base64(digest["digest"])) != 32:
        raise ValueError("invalid cosign message digest size")
    _validate_rekor2_consistency(entries[0], certificate, signature)


def _canonical_base64(value: object) -> bytes:
    if not isinstance(value, str) or not value:
        raise ValueError("invalid cosign binary field")
    raw = base64.b64decode(value, validate=True)
    if not raw or base64.b64encode(raw).decode("ascii") != value:
        raise ValueError("noncanonical cosign binary field")
    return raw


def _canonical_log_integer(value: object) -> int:
    if not isinstance(value, str) or re.fullmatch(r"0|[1-9][0-9]{0,18}", value) is None:
        raise ValueError("noncanonical cosign log integer")
    number = int(value)
    if number > (1 << 63) - 1:
        raise ValueError("cosign log integer exceeds int64")
    return number


def _validate_rekor2_consistency(entry: dict, certificate: dict, signature: dict) -> None:
    """Bind redundant wire fields to the exact values consumed by cosign's Rekor 2 verifier."""

    index = _canonical_log_integer(entry["logIndex"])
    log_id, proof = entry["logId"], entry["inclusionProof"]
    if not isinstance(log_id, dict) or set(log_id) != {"keyId"} or len(_canonical_base64(log_id["keyId"])) != 32:
        raise ValueError("invalid cosign log identifier")
    if not isinstance(proof, dict) or set(proof) != {"logIndex", "rootHash", "treeSize", "hashes", "checkpoint"}:
        raise ValueError("invalid cosign inclusion proof profile")
    size = _canonical_log_integer(proof["treeSize"])
    if _canonical_log_integer(proof["logIndex"]) != index or not index < size:
        raise ValueError("inconsistent cosign inclusion index")
    hashes = proof["hashes"]
    if not isinstance(hashes, list) or len(hashes) > 63 or any(len(_canonical_base64(value)) != 32 for value in hashes):
        raise ValueError("invalid cosign proof hashes")
    checkpoint = proof["checkpoint"]
    if not isinstance(checkpoint, dict) or set(checkpoint) != {"envelope"} or not isinstance(checkpoint["envelope"], str):
        raise ValueError("invalid cosign checkpoint profile")
    lines = checkpoint["envelope"].split("\n", 3)
    if (
        len(lines) != 4 or not lines[3].startswith("\n")
        or re.fullmatch(r"(?=.{1,253}\Z)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}", lines[0]) is None
        or _canonical_log_integer(lines[1]) != size
        or len(_canonical_base64(lines[2])) != 32
        or proof["rootHash"] != lines[2]
    ):
        raise ValueError("inconsistent cosign checkpoint metadata")
    expected_body = {
        "apiVersion": "0.0.2", "kind": "hashedrekord",
        "spec": {"hashedRekordV002": {
            "data": signature["messageDigest"],
            "signature": {
                "content": signature["signature"],
                "verifier": {"keyDetails": "PKIX_ECDSA_P256_SHA_256", "x509Certificate": certificate},
            },
        }},
    }
    canonical_body = json.dumps(expected_body, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")
    if _canonical_base64(entry["canonicalizedBody"]) != canonical_body:
        raise ValueError("cosign canonicalized body differs from verified fields")


def _require_digest(value: object) -> str:
    if (
        not isinstance(value, str)
        or re.fullmatch(r"[0-9a-f]{64}", value) is None
        or value == "0" * 64
    ):
        raise ValueError("invalid independent cosign input digest")
    return value


def verify_final_promotion_cosign(
    args: argparse.Namespace, subject: bytes, bundle: bytes,
) -> list[str]:
    """Run the pinned verifier on exact captured bytes and mandatory independently pinned roots."""

    try:
        executable_pin = _require_digest(args.provenance_cosign_verifier_sha256)
        root_pin = _require_digest(args.provenance_cosign_trusted_root_sha256)
        identity = canonical_public_provenance_url(args.provenance_certificate_identity)
        issuer = canonical_public_provenance_url(args.provenance_oidc_issuer)
        if identity is None or issuer is None:
            raise ValueError("invalid independent cosign identity")
        if not isinstance(subject, bytes) or not 0 < len(subject) <= MAX_SUBJECT_BYTES:
            raise ValueError("invalid cosign subject size")
        validate_cosign_bundle(bundle)
        trusted_root = read_evidence_bytes(
            args.provenance_cosign_trusted_root, MAX_TRUSTED_ROOT_BYTES,
        )
        if not trusted_root or hashlib.sha256(trusted_root).hexdigest() != root_pin:
            raise ValueError("cosign trusted root differs from independent pin")
        with tempfile.TemporaryDirectory(prefix="sorafs-promotion-cosign-") as temporary:
            root = resolve_path_identity(Path(temporary), [])
            if root is None:
                raise OSError("cosign private directory identity unavailable")
            executable = root / "cosign"
            verifier_process.snapshot_executable(
                args.provenance_cosign_verifier, executable, executable_pin,
            )
            for name, document in (
                ("subject", subject), ("bundle", bundle), ("trusted-root", trusted_root),
            ):
                verifier_process.write_private_input(root / name, document)
            result = verifier_process.run_verifier(
                [
                    str(executable), "verify-blob",
                    "--bundle", str(root / "bundle"),
                    "--trusted-root", str(root / "trusted-root"),
                    "--certificate-identity", identity,
                    "--certificate-oidc-issuer", issuer,
                    "--use-signed-timestamps",
                    str(root / "subject"),
                ],
                root,
                max_stdout_bytes=0,
                expected_stderr=b"Verified OK\n",
            )
            if result != b"":
                raise ValueError("unexpected cosign result")
    except (OSError, ValueError, TypeError, AttributeError, KeyError, RecursionError, RuntimeError):
        # Paths, candidate bytes, certificate contents and tool diagnostics never enter evidence.
        return [FAILURE]
    return []
