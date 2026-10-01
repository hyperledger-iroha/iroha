"""Sole model-owned raw-attestation issuer transport, before E/final identity.

Formatting this public protocol creates no SDK capability. Only the called
provider's actual platform originals, held deployment signer and durable issuer
reservation reach publication. Native independently authenticates the resulting
purpose-specific transport against its original C/current admitted policies.
"""
from __future__ import annotations

import hashlib
import subprocess
import tempfile
from pathlib import Path

from .attestation import fixed32, public_key_pem, require
from .ordinary_provider import VerifiedOrdinaryRawEvidence

DOMAIN = b"iroha:kagemusha:v1:raw-app-attestation-admission\0"
BODY_BYTES = 250
TRANSPORT_BYTES = 314
REQUEST_MAGIC = b"KRAC01"
REQUEST_BYTES = 288


def signing_body(evidence: VerifiedOrdinaryRawEvidence) -> bytes:
    """Exact250 public wire from checked original C and actual platform proof."""
    require(type(evidence) is VerifiedOrdinaryRawEvidence, "raw provider evidence absent")
    c, proof, policy = evidence.challenge, evidence.raw_proof, evidence.policy
    c.signing_bytes(); policy.validate()
    point = proof.attested_public_key_sec1
    key = hashlib.sha256(point).digest()
    require(point == evidence.request.attested_public_key_sec1
            and proof.evidence_sha256 == hashlib.sha256(evidence.request.raw_attestation).digest()
            and policy.profile_valid_from_ms <= c.issued_at_ms < c.expires_at_ms
            <= policy.profile_expires_at_ms, "raw original scope differs")
    if c.platform_class == 1:
        level = proof.android_security_level
        require(proof.platform == "android_keymint" and type(level) is int
                and level in policy.allowed_android_levels, "raw Android level differs")
    else:
        level = 3
        require(c.platform_class == 2 and proof.platform == "apple_app_attest"
                and not policy.allowed_android_levels, "raw Apple policy differs")
    body = b"\x01\x00\x01" + c.attestation_challenge() + c.app_authority_policy_digest
    body += bytes([c.platform_class, level]) + point
    body += key + proof.evidence_sha256 + policy.app_signing_identity_digest
    # The raw Apple verifier admitted the original counter0 attestation. This
    # slot is not an assertion counter, credential floor, or financial index.
    body += bytes(4) + c.issued_at_ms.to_bytes(8, "little") + c.expires_at_ms.to_bytes(8, "little")
    require(len(body) == BODY_BYTES, "raw admission body changed")
    return body


def signing_request(evidence: VerifiedOrdinaryRawEvidence) -> bytes:
    """Purpose-specific request for the pinned native model encoder only."""
    key = fixed32(evidence.policy.authority_public_key, "actual raw issuer public key")
    require(any(key), "raw issuer public key absent")
    request = REQUEST_MAGIC + signing_body(evidence) + key
    require(len(request) == REQUEST_BYTES, "raw admission signing request changed")
    return request


def authenticate_transport(original: bytes, evidence: VerifiedOrdinaryRawEvidence,
                           openssl_path: Path, *, fresh: bool) -> None:
    """Verify exact native transport/signature, never turn JSON into a capability.

    Historical recovery may return only its already-retained expired original.
    New signing/publication requires the original fresh interval, without renewal.
    """
    expected = signing_body(evidence)
    require(type(original) is bytes and len(original) == TRANSPORT_BYTES
            and original[:BODY_BYTES] == expected and type(fresh) is bool,
            "raw admission transport differs from original checked platform")
    c = evidence.challenge
    require(c.issued_at_ms <= evidence.trusted_time_ms
            and (not fresh or evidence.trusted_time_ms < c.expires_at_ms),
            "raw admission original expired or from the future")
    key = fixed32(evidence.policy.authority_public_key, "actual raw issuer public key")
    require(isinstance(openssl_path, Path) and openssl_path.is_absolute() and openssl_path.is_file(),
            "raw admission signature verification environment absent")
    with tempfile.TemporaryDirectory(prefix="iroha-raw-admission-verification-") as temporary:
        directory = Path(temporary)
        (directory / "public.pem").write_bytes(public_key_pem(bytes.fromhex("302a300506032b6570032100") + key))
        (directory / "message").write_bytes(DOMAIN + BODY_BYTES.to_bytes(8, "little") + expected)
        (directory / "signature").write_bytes(original[BODY_BYTES:])
        result = subprocess.run([str(openssl_path), "pkeyutl", "-verify", "-pubin",
            "-inkey", str(directory / "public.pem"), "-rawin", "-in", str(directory / "message"),
            "-sigfile", str(directory / "signature")], stdin=subprocess.DEVNULL,
            capture_output=True, timeout=5, check=False)
        require(result.returncode == 0, "raw admission issuer signature rejected")
