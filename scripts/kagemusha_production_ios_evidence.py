"""Fail-closed validation substrate for production iOS Kagemusha evidence.

The candidate lab contract deliberately proves an offline physical-device run
without using App Attest.  This module defines a separate production envelope
which binds that exact run to a governed policy and verifies the App Attest
certificate chain, attestation nonce, and assertion cryptographically.  A
separately signed online-authority receipt is also verified as an exact claim
of current Apple revocation status and one-time freshness consumption. Ordinary
validation requires that original receipt to remain current. Promotion instead
validates it as immutable consumption history and requires a separate current,
promotion-scoped exact-catalog receipt under an independently pinned authority
key.

The repository authority in
``kagemusha_app_attest_freshness_authority.py`` supplies durable challenge,
counter, Apple receipt-refresh/risk, and crash-recovery state. Provisioning and
auditing its live deployment and key lifecycle remain operator responsibilities:
a signature proves what the provisioned authority attested, not that an
arbitrary signer maintained that state. A signed lab statement, a Boolean, or
an unverified certificate array never satisfies this validator. The exact v1
``apple_revocation_*`` labels mean current Apple App Attest receipt acceptance
plus the static policy's exact-`TBSCertificate`-DER digest revocation check; Apple's
``attestationData`` endpoint is not represented as a general CRL/OCSP service.
"""

from __future__ import annotations

import base64
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import re
from pathlib import Path
import time
from typing import Any, Optional


PRODUCTION_SIGNED_EVIDENCE_SCHEMA = (
    "iroha.kagemusha.ios_device_lab.production_signed_evidence.v1"
)
PRODUCTION_POLICY_SCHEMA = "iroha.kagemusha.ios.production_device_policy.v1"
PLATFORM_EVIDENCE_SCHEMA = "iroha.kagemusha.ios.app_attest_evidence.v1"
CAPTURE_APP_CODE_SIGN_MEASUREMENTS_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_capture_code_sign_measurements.v1"
)
ATTESTATION_CHALLENGE_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_attestation_challenge.v1"
)
ASSERTION_CHALLENGE_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_assertion_challenge.v1"
)
ATTESTATION_CHALLENGE_DOMAIN = (
    "iroha:kagemusha:ios:production:app-attest:attestation:v1"
)
ASSERTION_CHALLENGE_DOMAIN = (
    "iroha:kagemusha:ios:production:app-attest:assertion:v1"
)
X509_VALIDATION_PROFILE = "apple-app-attest-x509-chain-and-nonce-v1"
SECURE_ENCLAVE_KEY_PROFILE = "dcappattest-generated-secure-enclave-key-v1"
FRESHNESS_RECEIPT_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_online_freshness_consumption_receipt.v1"
)
CATALOG_REVALIDATION_BINDING_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_catalog_revalidation_binding.v1"
)
CATALOG_REVALIDATION_RECEIPT_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_catalog_revalidation_receipt.v1"
)
ONLINE_REVOCATION_SOURCE = "apple-app-attest-online-status-authority-v1"
MISSING_FRESHNESS_RECEIPT = (
    "production App Attest requires a separately signed online-authority "
    "freshness/consumption receipt"
)

MAX_POLICY_BYTES = 1024 * 1024
MAX_FRESHNESS_RECEIPT_BYTES = 64 * 1024
MAX_CATALOG_REVALIDATION_RECEIPT_BYTES = 256 * 1024
MAX_CATALOG_REVALIDATION_RELEASES = 16
MAX_PLATFORM_OBJECT_BYTES = 128 * 1024
MAX_CERTIFICATE_BYTES = 64 * 1024
MAX_RECEIPT_BYTES = 64 * 1024
MAX_X509_CHAIN_CERTIFICATES = 4
MAX_X509_EXTENSIONS = 64
MAX_ONLINE_RECEIPT_LIFETIME_MS = 5 * 60 * 1000
MAX_ONLINE_REVOCATION_AGE_MS = 5 * 60 * 1000
MAX_ONLINE_CLOCK_SKEW_MS = 30 * 1000
ASSERTION_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES = 37
MIN_ASSERTION_AUTHENTICATOR_DATA_BYTES = (
    ASSERTION_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES + 1
)
MAX_AUTHENTICATOR_DATA_BYTES = 4 * 1024
MAX_CBOR_ARRAY_ITEMS = 1024
MAX_CBOR_MAP_ITEMS = 64
P256_PUBLIC_KEY_BYTES = 65
P256_SIGNATURE_DER_MAX_BYTES = 80
P384_PUBLIC_KEY_BYTES = 97
X509_SIGNATURE_DER_MAX_BYTES = 128

OID_EC_PUBLIC_KEY = "1.2.840.10045.2.1"
OID_ECDSA_WITH_SHA256 = "1.2.840.10045.4.3.2"
OID_ECDSA_WITH_SHA384 = "1.2.840.10045.4.3.3"
OID_PRIME256V1 = "1.2.840.10045.3.1.7"
OID_SECP384R1 = "1.3.132.0.34"
OID_BASIC_CONSTRAINTS = "2.5.29.19"
OID_KEY_USAGE = "2.5.29.15"
OID_SUBJECT_KEY_IDENTIFIER = "2.5.29.14"
OID_AUTHORITY_KEY_IDENTIFIER = "2.5.29.35"
OID_APP_ATTEST_NONCE = "1.2.840.113635.100.8.2"

SHA256_RE = re.compile(r"[0-9a-f]{64}")
POLICY_ID_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:-]{0,127}")
TEAM_ID_RE = re.compile(r"[A-Z0-9]{10}")
BUNDLE_ID_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9.-]{0,254}")

PRODUCTION_SIGNED_EVIDENCE_FIELDS = frozenset(
    {
        "schema",
        "version",
        "release_manifest_sha256",
        "production_policy_id",
        "production_policy_sha256",
        "platform_evidence",
        "artifact_digests",
        "signer_key_id",
        "signer_public_key_sha256",
        "signature_algorithm",
        "signature_payload_sha256",
        "signature",
    }
)
PRODUCTION_POLICY_FIELDS = frozenset(
    {
        "schema",
        "version",
        "policy_id",
        "app_id_prefix",
        "bundle_id",
        "environment",
        "allowed_validation_categories",
        "allowed_bundle_versions",
        "trusted_app_attest_roots",
        "revoked_certificate_tbs_sha256",
        "x509_validation_profile",
        "secure_enclave_key_profile",
    }
)
ROOT_FIELDS = frozenset({"der_base64", "sha256"})
PLATFORM_EVIDENCE_FIELDS = frozenset(
    {
        "schema",
        "version",
        "evaluated_at_unix_ms",
        "key_id",
        "assertion_public_key_sec1_base64",
        "attestation_client_data_base64",
        "attestation_object_base64",
        "assertion_client_data_base64",
        "assertion_object_base64",
        "capture_app_code_sign_measurements",
    }
)
CAPTURE_APP_CODE_SIGN_MEASUREMENTS_FIELDS = frozenset(
    {
        "schema",
        "version",
        "bundle_id",
        "bundle_version",
        "team_id",
        "application_identifier",
        "app_attest_environment",
        "executable_sha256",
        "cdhash",
    }
)
ATTESTATION_CHALLENGE_FIELDS = frozenset(
    {
        "schema",
        "version",
        "domain",
        "evaluated_at_unix_ms",
        "policy_id",
        "policy_sha256",
        "release_manifest_sha256",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "session_sha256",
        "native_library_sha256",
        "code_sign_measurements_sha256",
        "native_transcript_sha256",
        "proof_launch_receipt_sha256",
        "restart_launch_receipt_sha256",
        "capture_app_code_sign_measurements_sha256",
        "nonce_base64",
    }
)
ASSERTION_CHALLENGE_FIELDS = frozenset(
    set(ATTESTATION_CHALLENGE_FIELDS)
    | {
        "attestation_object_sha256",
        "key_id",
    }
)

FRESHNESS_RECEIPT_FIELDS = frozenset(
    {
        "schema",
        "version",
        "receipt_id",
        "consumption_id",
        "issued_at_unix_ms",
        "consumed_at_unix_ms",
        "expires_at_unix_ms",
        "status",
        "apple_revocation_checked_at_unix_ms",
        "apple_revocation_status",
        "apple_revocation_source",
        "evidence_sha256",
        "production_policy_sha256",
        "release_manifest_sha256",
        "platform_evidence_sha256",
        "attestation_client_data_sha256",
        "attestation_object_sha256",
        "assertion_client_data_sha256",
        "assertion_object_sha256",
        "attestation_challenge_nonce_sha256",
        "assertion_challenge_nonce_sha256",
        "attestation_nonce_sha256",
        "assertion_nonce_sha256",
        "key_id",
        "previous_assertion_counter",
        "assertion_counter",
        "certificate_chain_sha256",
        "signer_key_id",
        "signer_public_key_sha256",
        "signature_algorithm",
        "signature_payload_sha256",
        "signature",
    }
)
CATALOG_REVALIDATION_BINDING_FIELDS = frozenset(
    {
        "release_manifest_sha256",
        "evidence_sha256",
        "consumption_receipt_sha256",
    }
)
CATALOG_REVALIDATION_RELEASE_STATUS_FIELDS = frozenset(
    set(CATALOG_REVALIDATION_BINDING_FIELDS)
    | {
        "app_attest_key_id",
        "apple_status_checked_at_unix_ms",
        "apple_status",
        "apple_status_source",
        "refreshed_apple_receipt_sha256",
        "risk_metric",
    }
)
CATALOG_REVALIDATION_RECEIPT_FIELDS = frozenset(
    {
        "schema",
        "version",
        "receipt_id",
        "promotion_id",
        "catalog_sha256",
        "issued_at_unix_ms",
        "expires_at_unix_ms",
        "status",
        "release_statuses",
        "signer_key_id",
        "signer_public_key_sha256",
        "signature_algorithm",
        "signature_payload_sha256",
        "signature",
    }
)

ARTIFACT_CHALLENGE_BINDINGS = {
    "candidate_record_sha256": "input/candidate-v4.norito",
    "candidate_manifest_sha256": "input/candidate-manifest-v4.norito",
    "session_sha256": "input/session-v1.json",
    "native_library_sha256": "build/libNoritoBridgeCandidateLab.a",
    "code_sign_measurements_sha256": "build/code-sign-measurements-v1.json",
    "native_transcript_sha256": "output/native-transcript-v1.json",
    "proof_launch_receipt_sha256": "output/proof-launch-receipt-v1.json",
    "restart_launch_receipt_sha256": "output/restart-launch-receipt-v1.json",
}

# NIST P-256 parameters.  The verifier is deliberately small and independent
# of PATH-selected crypto tooling; the production gate authenticates this
# source before loading it.
P256_P = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF
P256_A = P256_P - 3
P256_B = 0x5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B
P256_N = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
P256_G = (
    0x6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296,
    0x4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5,
)

P384_P = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFFFF0000000000000000FFFFFFFF
P384_A = P384_P - 3
P384_B = 0xB3312FA7E23EE7E4988E056BE3F82D19181D9C6EFE8141120314088F5013875AC656398D8A2ED19D2A85C8EDD3EC2AEF
P384_N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC7634D81F4372DDF581A0DB248B0A77AECEC196ACCC52973
P384_G = (
    0xAA87CA22BE8B05378EB1C71EF320AD746E1D3B628BA79B9859F741E082542A385502F25DBF55296C3A545E3872760AB7,
    0x3617DE4A96262C6F5D9E98BF9292DC29F8F41DBD289A147CE9DA3113B5F0B8C00A60B1CE1D7E819D7A431D7C90EA0E5F,
)


@dataclass(frozen=True)
class _EcCurve:
    name: str
    oid: str
    coordinate_bytes: int
    p: int
    a: int
    b: int
    n: int
    generator: tuple[int, int]


P256_CURVE = _EcCurve(
    "P-256", OID_PRIME256V1, 32, P256_P, P256_A, P256_B, P256_N, P256_G
)
P384_CURVE = _EcCurve(
    "P-384", OID_SECP384R1, 48, P384_P, P384_A, P384_B, P384_N, P384_G
)
EC_CURVES_BY_OID = {curve.oid: curve for curve in (P256_CURVE, P384_CURVE)}


@dataclass(frozen=True)
class _X509Extension:
    critical: bool
    value: bytes


@dataclass(frozen=True)
class _X509Certificate:
    der: bytes
    tbs_der: bytes
    serial: int
    signature_algorithm_oid: str
    signature_der: bytes
    issuer_der: bytes
    subject_der: bytes
    not_before_unix_ms: int
    not_after_unix_ms: int
    public_key_curve: _EcCurve
    public_key: bytes
    extensions: dict[str, _X509Extension]


@dataclass(frozen=True)
class _PlatformEvidenceFacts:
    evaluated_at_unix_ms: int
    key_id: str
    attestation_client_data: bytes
    attestation_object: bytes
    assertion_client_data: bytes
    assertion_object: bytes
    attestation_challenge_nonce: bytes
    assertion_challenge_nonce: bytes
    attestation_nonce: bytes
    assertion_nonce: bytes
    assertion_counter: int
    certificate_chain: tuple[bytes, ...]


@dataclass(frozen=True)
class _CborMap:
    pairs: tuple[tuple[Any, Any], ...]


class _CborDecoder:
    def __init__(self, payload: bytes) -> None:
        self.payload = payload
        self.offset = 0

    def _read(self, length: int) -> bytes:
        end = self.offset + length
        if length < 0 or end > len(self.payload):
            raise ValueError("CBOR item exceeds input bounds")
        result = self.payload[self.offset:end]
        self.offset = end
        return result

    def _argument(self, additional: int) -> int:
        if additional < 24:
            return additional
        widths = {24: 1, 25: 2, 26: 4, 27: 8}
        width = widths.get(additional)
        if width is None:
            raise ValueError("indefinite or reserved CBOR length is forbidden")
        value = int.from_bytes(self._read(width), "big")
        minimum = {1: 24, 2: 256, 4: 65536, 8: 1 << 32}[width]
        if value < minimum:
            raise ValueError("CBOR integer or length is not shortest-form")
        return value

    def value(self, depth: int = 0) -> Any:
        if depth > 16:
            raise ValueError("CBOR nesting exceeds 16 levels")
        initial = self._read(1)[0]
        major = initial >> 5
        argument = self._argument(initial & 0x1F)
        if major == 0:
            return argument
        if major == 1:
            return -1 - argument
        if major == 2:
            return self._read(argument)
        if major == 3:
            try:
                return self._read(argument).decode("utf-8")
            except UnicodeDecodeError as error:
                raise ValueError("CBOR text is not UTF-8") from error
        if major == 4:
            if argument > MAX_CBOR_ARRAY_ITEMS:
                raise ValueError("CBOR array exceeds its item-count bound")
            return tuple(self.value(depth + 1) for _ in range(argument))
        if major == 5:
            if argument > MAX_CBOR_MAP_ITEMS:
                raise ValueError("CBOR map exceeds its item-count bound")
            pairs: list[tuple[Any, Any]] = []
            for _ in range(argument):
                key = self.value(depth + 1)
                if any(existing == key for existing, _ in pairs):
                    raise ValueError("CBOR map contains a duplicate key")
                pairs.append((key, self.value(depth + 1)))
            return _CborMap(tuple(pairs))
        raise ValueError("unsupported CBOR major type")


def _decode_cbor(payload: bytes, label: str) -> Any:
    try:
        decoder = _CborDecoder(payload)
        value = decoder.value()
        if decoder.offset != len(payload):
            raise ValueError("trailing CBOR bytes")
        return value
    except ValueError as error:
        raise ValueError(f"{label} is not strict definite-length CBOR: {error}") from error


def _cbor_object(value: Any, fields: set[Any], label: str) -> dict[Any, Any]:
    if not isinstance(value, _CborMap):
        raise ValueError(f"{label} must be a CBOR map")
    result = dict(value.pairs)
    if set(result) != fields:
        raise ValueError(f"{label} must contain the exact fields {sorted(map(str, fields))}")
    return result


def _decode_base64(value: Any, label: str, maximum: int) -> Optional[bytes]:
    if not isinstance(value, str) or not value:
        return None
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, base64.binascii.Error):
        return None
    if not decoded or len(decoded) > maximum or base64.b64encode(decoded).decode() != value:
        return None
    return decoded


def _require_base64(
    value: Any, label: str, maximum: int, errors: list[str]
) -> Optional[bytes]:
    decoded = _decode_base64(value, label, maximum)
    if decoded is None:
        errors.append(f"{label} must be nonempty canonical standard Base64 within {maximum} bytes")
    return decoded


def _exact_fields(
    value: Any, fields: frozenset[str], label: str, errors: list[str]
) -> Optional[dict[str, Any]]:
    if not isinstance(value, dict):
        errors.append(f"{label} must be an object")
        return None
    observed = set(value)
    if observed != fields:
        errors.append(
            f"{label} fields are not exact (missing={sorted(fields - observed)}, "
            f"extra={sorted(observed - fields)})"
        )
        return None
    return value


def _nonzero_digest(value: Any, label: str, errors: list[str]) -> Optional[str]:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None or value == "0" * 64:
        errors.append(f"{label} must be a nonzero lowercase SHA-256")
        return None
    return value


def _canonical_ascii(value: Any, label: str, maximum: int, errors: list[str]) -> Optional[str]:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum
        or not value.isascii()
        or value.strip() != value
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
    ):
        errors.append(f"{label} must be nonempty canonical ASCII within {maximum} bytes")
        return None
    return value


def _validate_policy(
    policy: Any, policy_bytes: bytes, errors: list[str]
) -> bool:
    """Validate the policy and report whether later typed access is safe."""

    starting_errors = len(errors)
    if _exact_fields(policy, PRODUCTION_POLICY_FIELDS, "production iOS policy", errors) is None:
        return False
    if policy.get("schema") != PRODUCTION_POLICY_SCHEMA:
        errors.append(f"production iOS policy schema must be {PRODUCTION_POLICY_SCHEMA}")
    if policy.get("version") != 1 or isinstance(policy.get("version"), bool):
        errors.append("production iOS policy version must be integer 1")
    policy_id = policy.get("policy_id")
    if not isinstance(policy_id, str) or POLICY_ID_RE.fullmatch(policy_id) is None:
        errors.append("production iOS policy_id is not canonical")
    app_id_prefix = policy.get("app_id_prefix")
    if not isinstance(app_id_prefix, str) or TEAM_ID_RE.fullmatch(app_id_prefix) is None:
        errors.append("production iOS app_id_prefix must be a 10-character Apple identifier")
    bundle_id = policy.get("bundle_id")
    if (
        not isinstance(bundle_id, str)
        or BUNDLE_ID_RE.fullmatch(bundle_id) is None
        or ".." in bundle_id
        or bundle_id.endswith(".")
    ):
        errors.append("production iOS bundle_id is not canonical")
    if policy.get("environment") != "production":
        errors.append("production iOS policy environment must be production")
    if policy.get("x509_validation_profile") != X509_VALIDATION_PROFILE:
        errors.append(f"production iOS policy x509_validation_profile must be {X509_VALIDATION_PROFILE}")
    if policy.get("secure_enclave_key_profile") != SECURE_ENCLAVE_KEY_PROFILE:
        errors.append(
            "production iOS policy secure_enclave_key_profile must require the "
            "DCAppAttest-generated Secure Enclave key"
        )

    categories = policy.get("allowed_validation_categories")
    if (
        not isinstance(categories, list)
        or not categories
        or any(isinstance(item, bool) or not isinstance(item, int) for item in categories)
        or categories != sorted(set(categories))
        or len(categories) > 7
        or any(item not in {1, 2, 3, 4, 5, 6, 10} for item in categories)
    ):
        errors.append("production iOS allowed_validation_categories must be a sorted unique supported list")
    versions = policy.get("allowed_bundle_versions")
    if (
        not isinstance(versions, list)
        or not versions
        or len(versions) > 64
        or any(not isinstance(item, str) for item in versions)
        or versions != sorted(set(versions))
    ):
        errors.append("production iOS allowed_bundle_versions must be a nonempty sorted unique list")
    else:
        for index, version in enumerate(versions):
            _canonical_ascii(version, f"production iOS allowed_bundle_versions[{index}]", 64, errors)

    roots = policy.get("trusted_app_attest_roots")
    root_digests: list[str] = []
    root_tbs_digests: list[str] = []
    if not isinstance(roots, list) or not 1 <= len(roots) <= 4:
        errors.append("production iOS policy must contain one to four trusted App Attest roots")
    else:
        for index, root in enumerate(roots):
            label = f"production iOS trusted_app_attest_roots[{index}]"
            if _exact_fields(root, ROOT_FIELDS, label, errors) is None:
                continue
            der = _require_base64(root.get("der_base64"), f"{label}.der_base64", MAX_CERTIFICATE_BYTES, errors)
            digest = _nonzero_digest(root.get("sha256"), f"{label}.sha256", errors)
            if der is not None and digest is not None and hashlib.sha256(der).hexdigest() != digest:
                errors.append(f"{label}.sha256 does not match DER bytes")
            if der is not None:
                try:
                    certificate = _parse_x509_certificate(der, label)
                    root_tbs_digests.append(
                        hashlib.sha256(certificate.tbs_der).hexdigest()
                    )
                    is_ca, _ = _x509_basic_constraints(certificate, label)
                    _, key_cert_sign = _x509_key_usage(certificate, label)
                    if not is_ca or not key_cert_sign:
                        raise ValueError(
                            f"{label} must be a certificate-signing CA"
                        )
                    if certificate.issuer_der != certificate.subject_der:
                        raise ValueError(f"{label} must be self-issued")
                    _verify_x509_signature(certificate, certificate, label)
                except ValueError as error:
                    errors.append(str(error))
            if digest is not None:
                root_digests.append(digest)
        if root_digests != sorted(set(root_digests)):
            errors.append("production iOS trusted App Attest roots must be ordered by unique digest")

    revoked = policy.get("revoked_certificate_tbs_sha256")
    if (
        not isinstance(revoked, list)
        or len(revoked) > 128
        or any(
            not isinstance(item, str)
            or SHA256_RE.fullmatch(item) is None
            or item == "0" * 64
            for item in revoked
        )
        or revoked != sorted(set(revoked))
    ):
        errors.append(
            "production iOS revoked_certificate_tbs_sha256 must be a sorted unique digest list"
        )
    elif set(root_tbs_digests) & set(revoked):
        errors.append("production iOS trusted root must not also be revoked")
    if len(policy_bytes) > MAX_POLICY_BYTES:
        errors.append("production iOS policy exceeds its byte limit")
    return len(errors) == starting_errors


def _point_on_curve(point: tuple[int, int]) -> bool:
    x, y = point
    return 0 <= x < P256_P and 0 <= y < P256_P and (y * y - x * x * x - P256_A * x - P256_B) % P256_P == 0


def _point_add(
    left: Optional[tuple[int, int]], right: Optional[tuple[int, int]]
) -> Optional[tuple[int, int]]:
    if left is None:
        return right
    if right is None:
        return left
    x1, y1 = left
    x2, y2 = right
    if x1 == x2 and (y1 + y2) % P256_P == 0:
        return None
    if left == right:
        if y1 == 0:
            return None
        slope = (3 * x1 * x1 + P256_A) * pow(2 * y1, -1, P256_P) % P256_P
    else:
        slope = (y2 - y1) * pow((x2 - x1) % P256_P, -1, P256_P) % P256_P
    x3 = (slope * slope - x1 - x2) % P256_P
    return x3, (slope * (x1 - x3) - y1) % P256_P


def _scalar_multiply(scalar: int, point: tuple[int, int]) -> Optional[tuple[int, int]]:
    result: Optional[tuple[int, int]] = None
    addend: Optional[tuple[int, int]] = point
    while scalar:
        if scalar & 1:
            result = _point_add(result, addend)
        addend = _point_add(addend, addend)
        scalar >>= 1
    return result


def _parse_p256_public_key(payload: bytes) -> tuple[int, int]:
    if len(payload) != P256_PUBLIC_KEY_BYTES or payload[0] != 0x04:
        raise ValueError("App Attest assertion public key must be uncompressed P-256 SEC1")
    point = (int.from_bytes(payload[1:33], "big"), int.from_bytes(payload[33:], "big"))
    if not _point_on_curve(point) or _scalar_multiply(P256_N, point) is not None:
        raise ValueError("App Attest assertion public key is not a valid P-256 point")
    return point


def _der_length(payload: bytes, offset: int) -> tuple[int, int]:
    if offset >= len(payload):
        raise ValueError("truncated DER length")
    first = payload[offset]
    offset += 1
    if first < 0x80:
        return first, offset
    width = first & 0x7F
    if width == 0 or width > 2 or offset + width > len(payload):
        raise ValueError("invalid DER length")
    raw = payload[offset : offset + width]
    if raw[0] == 0 or int.from_bytes(raw, "big") < 0x80:
        raise ValueError("noncanonical DER length")
    return int.from_bytes(raw, "big"), offset + width


def _der_integer(payload: bytes, offset: int) -> tuple[int, int]:
    if offset >= len(payload) or payload[offset] != 0x02:
        raise ValueError("ECDSA signature component is not a DER INTEGER")
    length, start = _der_length(payload, offset + 1)
    end = start + length
    if length == 0 or end > len(payload):
        raise ValueError("truncated ECDSA DER INTEGER")
    raw = payload[start:end]
    if raw[0] & 0x80 or (len(raw) > 1 and raw[0] == 0 and raw[1] < 0x80):
        raise ValueError("ECDSA DER INTEGER is not canonical positive form")
    return int.from_bytes(raw, "big"), end


def _parse_ecdsa_der(payload: bytes) -> tuple[int, int]:
    if not 8 <= len(payload) <= P256_SIGNATURE_DER_MAX_BYTES or payload[0] != 0x30:
        raise ValueError("App Attest assertion signature is not bounded DER ECDSA")
    length, offset = _der_length(payload, 1)
    if offset + length != len(payload):
        raise ValueError("App Attest assertion signature has trailing DER bytes")
    r, offset = _der_integer(payload, offset)
    s, offset = _der_integer(payload, offset)
    if offset != len(payload) or not (1 <= r < P256_N and 1 <= s < P256_N):
        raise ValueError("App Attest assertion signature is invalid")
    return r, min(s, P256_N - s)


def _verify_p256_signature(public_key: bytes, message: bytes, signature_der: bytes) -> None:
    point = _parse_p256_public_key(public_key)
    r, s = _parse_ecdsa_der(signature_der)
    z = int.from_bytes(hashlib.sha256(message).digest(), "big")
    inverse = pow(s, -1, P256_N)
    candidate = _point_add(
        _scalar_multiply((z * inverse) % P256_N, P256_G),
        _scalar_multiply((r * inverse) % P256_N, point),
    )
    if candidate is None or candidate[0] % P256_N != r:
        raise ValueError("App Attest assertion signature verification failed")


def _ec_point_on_curve(point: tuple[int, int], curve: _EcCurve) -> bool:
    x, y = point
    return (
        0 <= x < curve.p
        and 0 <= y < curve.p
        and (y * y - x * x * x - curve.a * x - curve.b) % curve.p == 0
    )


def _ec_point_add(
    left: Optional[tuple[int, int]],
    right: Optional[tuple[int, int]],
    curve: _EcCurve,
) -> Optional[tuple[int, int]]:
    if left is None:
        return right
    if right is None:
        return left
    x1, y1 = left
    x2, y2 = right
    if x1 == x2 and (y1 + y2) % curve.p == 0:
        return None
    if left == right:
        if y1 == 0:
            return None
        slope = (3 * x1 * x1 + curve.a) * pow(2 * y1, -1, curve.p) % curve.p
    else:
        slope = (y2 - y1) * pow((x2 - x1) % curve.p, -1, curve.p) % curve.p
    x3 = (slope * slope - x1 - x2) % curve.p
    return x3, (slope * (x1 - x3) - y1) % curve.p


def _ec_scalar_multiply(
    scalar: int, point: tuple[int, int], curve: _EcCurve
) -> Optional[tuple[int, int]]:
    result: Optional[tuple[int, int]] = None
    addend: Optional[tuple[int, int]] = point
    while scalar:
        if scalar & 1:
            result = _ec_point_add(result, addend, curve)
        addend = _ec_point_add(addend, addend, curve)
        scalar >>= 1
    return result


def _parse_ec_public_key(payload: bytes, curve: _EcCurve, label: str) -> tuple[int, int]:
    expected_length = 1 + 2 * curve.coordinate_bytes
    if len(payload) != expected_length or payload[0] != 0x04:
        raise ValueError(
            f"{label} must be an uncompressed {curve.name} SEC1 public key"
        )
    point = (
        int.from_bytes(payload[1 : 1 + curve.coordinate_bytes], "big"),
        int.from_bytes(payload[1 + curve.coordinate_bytes :], "big"),
    )
    if (
        not _ec_point_on_curve(point, curve)
        or _ec_scalar_multiply(curve.n, point, curve) is not None
    ):
        raise ValueError(f"{label} is not a valid {curve.name} point")
    return point


def _parse_ecdsa_der_for_order(
    payload: bytes, order: int, maximum: int, label: str
) -> tuple[int, int]:
    if not 8 <= len(payload) <= maximum or payload[0] != 0x30:
        raise ValueError(f"{label} is not bounded DER ECDSA")
    length, offset = _der_length(payload, 1)
    if offset + length != len(payload):
        raise ValueError(f"{label} has trailing DER bytes")
    r, offset = _der_integer(payload, offset)
    s, offset = _der_integer(payload, offset)
    if offset != len(payload) or not (1 <= r < order and 1 <= s < order):
        raise ValueError(f"{label} has invalid ECDSA components")
    return r, s


def _verify_ec_signature(
    public_key: bytes,
    curve: _EcCurve,
    message: bytes,
    signature_der: bytes,
    hash_name: str,
    label: str,
) -> None:
    point = _parse_ec_public_key(public_key, curve, f"{label} issuer public key")
    r, s = _parse_ecdsa_der_for_order(
        signature_der, curve.n, X509_SIGNATURE_DER_MAX_BYTES, label
    )
    digest = hashlib.new(hash_name, message).digest()
    digest_bits = len(digest) * 8
    order_bits = curve.n.bit_length()
    z = int.from_bytes(digest, "big")
    if digest_bits > order_bits:
        z >>= digest_bits - order_bits
    inverse = pow(s, -1, curve.n)
    candidate = _ec_point_add(
        _ec_scalar_multiply((z * inverse) % curve.n, curve.generator, curve),
        _ec_scalar_multiply((r * inverse) % curve.n, point, curve),
        curve,
    )
    if candidate is None or candidate[0] % curve.n != r:
        raise ValueError(f"{label} verification failed")


@dataclass(frozen=True)
class _DerElement:
    tag: int
    content: bytes
    encoded: bytes


class _DerReader:
    def __init__(self, payload: bytes, label: str) -> None:
        self.payload = payload
        self.label = label
        self.offset = 0

    def peek_tag(self) -> Optional[int]:
        return self.payload[self.offset] if self.offset < len(self.payload) else None

    def element(self, expected_tag: Optional[int] = None) -> _DerElement:
        start = self.offset
        if start >= len(self.payload):
            raise ValueError(f"{self.label} is truncated")
        tag = self.payload[start]
        if tag & 0x1F == 0x1F:
            raise ValueError(f"{self.label} uses an unsupported high-tag-number DER item")
        length, content_start = _der_length(self.payload, start + 1)
        end = content_start + length
        if end > len(self.payload):
            raise ValueError(f"{self.label} DER item exceeds input bounds")
        if expected_tag is not None and tag != expected_tag:
            raise ValueError(
                f"{self.label} has DER tag 0x{tag:02x}, expected 0x{expected_tag:02x}"
            )
        self.offset = end
        return _DerElement(
            tag=tag,
            content=self.payload[content_start:end],
            encoded=self.payload[start:end],
        )

    def finish(self) -> None:
        if self.offset != len(self.payload):
            raise ValueError(f"{self.label} has trailing DER bytes")


def _der_single(payload: bytes, tag: int, label: str) -> _DerElement:
    reader = _DerReader(payload, label)
    value = reader.element(tag)
    reader.finish()
    return value


def _der_positive_integer(payload: bytes, label: str, *, allow_zero: bool) -> int:
    if not payload or payload[0] & 0x80:
        raise ValueError(f"{label} must be a positive DER INTEGER")
    if len(payload) > 1 and payload[0] == 0 and payload[1] < 0x80:
        raise ValueError(f"{label} DER INTEGER is not minimally encoded")
    value = int.from_bytes(payload, "big")
    if value == 0 and not allow_zero:
        raise ValueError(f"{label} must be nonzero")
    return value


def _der_oid(payload: bytes, label: str) -> str:
    if not payload:
        raise ValueError(f"{label} OID is empty")
    components: list[int] = []
    value = 0
    width = 0
    for byte in payload:
        if width == 0 and byte == 0x80:
            raise ValueError(f"{label} OID has a noncanonical component")
        value = (value << 7) | (byte & 0x7F)
        width += 1
        if width > 10:
            raise ValueError(f"{label} OID component is too large")
        if byte & 0x80 == 0:
            components.append(value)
            value = 0
            width = 0
    if width or not components:
        raise ValueError(f"{label} OID is truncated")
    first_value = components[0]
    if first_value < 40:
        first, second = 0, first_value
    elif first_value < 80:
        first, second = 1, first_value - 40
    else:
        first, second = 2, first_value - 80
    return ".".join(str(item) for item in (first, second, *components[1:]))


def _der_algorithm_identifier(payload: bytes, label: str) -> tuple[str, Optional[str]]:
    sequence = _der_single(payload, 0x30, label)
    reader = _DerReader(sequence.content, label)
    algorithm = _der_oid(reader.element(0x06).content, f"{label} algorithm")
    parameter = None
    if reader.peek_tag() is not None:
        parameter = _der_oid(reader.element(0x06).content, f"{label} parameter")
    reader.finish()
    return algorithm, parameter


def _der_time(payload: bytes, tag: int, label: str) -> int:
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as error:
        raise ValueError(f"{label} is not ASCII") from error
    if tag == 0x17:
        if re.fullmatch(r"[0-9]{12}Z", text) is None:
            raise ValueError(f"{label} UTCTime is not canonical")
        year = int(text[:2])
        year += 2000 if year < 50 else 1900
        components = (year, int(text[2:4]), int(text[4:6]), int(text[6:8]), int(text[8:10]), int(text[10:12]))
    elif tag == 0x18:
        if re.fullmatch(r"[0-9]{14}Z", text) is None:
            raise ValueError(f"{label} GeneralizedTime is not canonical")
        components = (int(text[:4]), int(text[4:6]), int(text[6:8]), int(text[8:10]), int(text[10:12]), int(text[12:14]))
    else:
        raise ValueError(f"{label} has an unsupported ASN.1 time tag")
    try:
        value = datetime(*components, tzinfo=timezone.utc)
    except ValueError as error:
        raise ValueError(f"{label} contains an invalid calendar time") from error
    return int(value.timestamp()) * 1000


def _parse_x509_extensions(payload: bytes, label: str) -> dict[str, _X509Extension]:
    outer = _der_single(payload, 0xA3, label)
    sequence = _der_single(outer.content, 0x30, label)
    reader = _DerReader(sequence.content, label)
    extensions: dict[str, _X509Extension] = {}
    while reader.peek_tag() is not None:
        if len(extensions) >= MAX_X509_EXTENSIONS:
            raise ValueError(f"{label} exceeds {MAX_X509_EXTENSIONS} entries")
        extension = reader.element(0x30)
        item = _DerReader(extension.content, f"{label} entry")
        oid = _der_oid(item.element(0x06).content, f"{label} entry")
        critical = False
        if item.peek_tag() == 0x01:
            boolean = item.element(0x01).content
            if boolean != b"\xff":
                raise ValueError(f"{label} {oid} critical flag is not canonical TRUE")
            critical = True
        value = item.element(0x04).content
        item.finish()
        if oid in extensions:
            raise ValueError(f"{label} contains duplicate extension {oid}")
        if critical and oid not in {OID_BASIC_CONSTRAINTS, OID_KEY_USAGE}:
            raise ValueError(f"{label} contains unsupported critical extension {oid}")
        extensions[oid] = _X509Extension(critical=critical, value=value)
    return extensions


def _parse_x509_certificate(payload: bytes, label: str) -> _X509Certificate:
    if not payload or len(payload) > MAX_CERTIFICATE_BYTES:
        raise ValueError(f"{label} size is outside its bound")
    certificate = _der_single(payload, 0x30, label)
    reader = _DerReader(certificate.content, label)
    tbs = reader.element(0x30)
    outer_algorithm_element = reader.element(0x30)
    outer_algorithm = _der_algorithm_identifier(
        outer_algorithm_element.encoded, f"{label} signature algorithm"
    )
    signature_bits = reader.element(0x03).content
    reader.finish()
    if not signature_bits or signature_bits[0] != 0:
        raise ValueError(f"{label} signature BIT STRING is invalid")

    tbs_reader = _DerReader(tbs.content, f"{label} TBSCertificate")
    version = tbs_reader.element(0xA0)
    version_value = _der_positive_integer(
        _der_single(version.content, 0x02, f"{label} version").content,
        f"{label} version",
        allow_zero=True,
    )
    if version_value != 2:
        raise ValueError(f"{label} must be an X.509 v3 certificate")
    serial = _der_positive_integer(
        tbs_reader.element(0x02).content, f"{label} serial", allow_zero=False
    )
    if serial.bit_length() > 160:
        raise ValueError(f"{label} serial exceeds 20 octets")
    tbs_algorithm_element = tbs_reader.element(0x30)
    tbs_algorithm = _der_algorithm_identifier(
        tbs_algorithm_element.encoded, f"{label} TBSCertificate signature algorithm"
    )
    if tbs_algorithm != outer_algorithm:
        raise ValueError(f"{label} signature algorithms do not match")
    algorithm_oid, algorithm_parameter = outer_algorithm
    if algorithm_parameter is not None or algorithm_oid not in {
        OID_ECDSA_WITH_SHA256,
        OID_ECDSA_WITH_SHA384,
    }:
        raise ValueError(f"{label} uses an unsupported certificate signature algorithm")
    issuer = tbs_reader.element(0x30)
    validity = tbs_reader.element(0x30)
    validity_reader = _DerReader(validity.content, f"{label} validity")
    not_before_element = validity_reader.element()
    not_after_element = validity_reader.element()
    validity_reader.finish()
    not_before = _der_time(
        not_before_element.content, not_before_element.tag, f"{label} notBefore"
    )
    not_after = _der_time(
        not_after_element.content, not_after_element.tag, f"{label} notAfter"
    )
    if not_before > not_after:
        raise ValueError(f"{label} validity interval is inverted")
    subject = tbs_reader.element(0x30)
    spki = tbs_reader.element(0x30)
    spki_reader = _DerReader(spki.content, f"{label} subjectPublicKeyInfo")
    spki_algorithm_element = spki_reader.element(0x30)
    spki_algorithm, curve_oid = _der_algorithm_identifier(
        spki_algorithm_element.encoded, f"{label} public-key algorithm"
    )
    if spki_algorithm != OID_EC_PUBLIC_KEY or curve_oid not in EC_CURVES_BY_OID:
        raise ValueError(f"{label} public key must be P-256 or P-384 EC")
    curve = EC_CURVES_BY_OID[curve_oid]
    public_key_bits = spki_reader.element(0x03).content
    spki_reader.finish()
    if not public_key_bits or public_key_bits[0] != 0:
        raise ValueError(f"{label} public key BIT STRING is invalid")
    public_key = public_key_bits[1:]
    _parse_ec_public_key(public_key, curve, f"{label} public key")
    if tbs_reader.peek_tag() != 0xA3:
        raise ValueError(f"{label} must contain one X.509 v3 extension sequence")
    extensions_element = tbs_reader.element(0xA3)
    tbs_reader.finish()
    extensions = _parse_x509_extensions(
        extensions_element.encoded, f"{label} extensions"
    )
    return _X509Certificate(
        der=payload,
        tbs_der=tbs.encoded,
        serial=serial,
        signature_algorithm_oid=algorithm_oid,
        signature_der=signature_bits[1:],
        issuer_der=issuer.encoded,
        subject_der=subject.encoded,
        not_before_unix_ms=not_before,
        not_after_unix_ms=not_after,
        public_key_curve=curve,
        public_key=public_key,
        extensions=extensions,
    )


def _x509_basic_constraints(
    certificate: _X509Certificate, label: str
) -> tuple[bool, Optional[int]]:
    extension = certificate.extensions.get(OID_BASIC_CONSTRAINTS)
    if extension is None:
        return False, None
    if not extension.critical:
        raise ValueError(f"{label} basic constraints must be critical")
    sequence = _der_single(extension.value, 0x30, f"{label} basic constraints")
    reader = _DerReader(sequence.content, f"{label} basic constraints")
    is_ca = False
    if reader.peek_tag() == 0x01:
        value = reader.element(0x01).content
        if value != b"\xff":
            raise ValueError(f"{label} basic constraints CA flag is not canonical TRUE")
        is_ca = True
    path_length = None
    if reader.peek_tag() == 0x02:
        path_length = _der_positive_integer(
            reader.element(0x02).content,
            f"{label} basic constraints path length",
            allow_zero=True,
        )
    reader.finish()
    if path_length is not None and not is_ca:
        raise ValueError(f"{label} has a CA path length without CA=true")
    return is_ca, path_length


def _x509_key_usage(certificate: _X509Certificate, label: str) -> tuple[bool, bool]:
    extension = certificate.extensions.get(OID_KEY_USAGE)
    if extension is None or not extension.critical:
        raise ValueError(f"{label} key usage must be present and critical")
    bit_string = _der_single(extension.value, 0x03, f"{label} key usage").content
    if len(bit_string) < 2 or bit_string[0] > 7:
        raise ValueError(f"{label} key usage BIT STRING is invalid")
    unused = bit_string[0]
    bits = bit_string[1:]
    if unused and bits[-1] & ((1 << unused) - 1):
        raise ValueError(f"{label} key usage has nonzero unused bits")
    digital_signature = bool(bits[0] & 0x80)
    key_cert_sign = bool(bits[0] & 0x04)
    return digital_signature, key_cert_sign


def _validate_x509_time(
    certificate: _X509Certificate, evaluation_time_unix_ms: int, label: str
) -> None:
    if not (
        certificate.not_before_unix_ms
        <= evaluation_time_unix_ms
        <= certificate.not_after_unix_ms
    ):
        raise ValueError(f"{label} is not valid at the evidence evaluation time")


def _verify_x509_signature(
    certificate: _X509Certificate, issuer: _X509Certificate, label: str
) -> None:
    hash_name = {
        OID_ECDSA_WITH_SHA256: "sha256",
        OID_ECDSA_WITH_SHA384: "sha384",
    }[certificate.signature_algorithm_oid]
    _verify_ec_signature(
        issuer.public_key,
        issuer.public_key_curve,
        certificate.tbs_der,
        certificate.signature_der,
        hash_name,
        f"{label} certificate signature",
    )


def _extract_app_attest_nonce(payload: bytes) -> bytes:
    sequence = _der_single(payload, 0x30, "App Attest nonce extension")
    sequence_reader = _DerReader(
        sequence.content, "App Attest nonce extension sequence"
    )
    explicit = sequence_reader.element(0xA1)
    sequence_reader.finish()
    nonce = _der_single(
        explicit.content, 0x04, "App Attest nonce extension explicit value"
    ).content
    if len(nonce) != 32:
        raise ValueError("App Attest nonce extension must contain exactly 32 bytes")
    return nonce


def _validate_attestation_certificate_chain(
    chain_der: tuple[bytes, ...],
    policy: dict[str, Any],
    evaluation_time_unix_ms: int,
    expected_public_key: bytes,
    auth_data: bytes,
    attestation_client_data_hash: bytes,
) -> bytes:
    if not 2 <= len(chain_der) <= MAX_X509_CHAIN_CERTIFICATES:
        raise ValueError("App Attest x5c must contain a bounded leaf/intermediate chain")
    revoked = set(policy["revoked_certificate_tbs_sha256"])
    seen: set[str] = set()
    chain: list[_X509Certificate] = []
    for index, der in enumerate(chain_der):
        certificate = _parse_x509_certificate(der, f"App Attest x5c[{index}]")
        tbs_digest = hashlib.sha256(certificate.tbs_der).hexdigest()
        if tbs_digest in revoked:
            raise ValueError("App Attest certificate is revoked by static production policy")
        if tbs_digest in seen:
            raise ValueError("App Attest certificate chain contains duplicate certificates")
        seen.add(tbs_digest)
        _validate_x509_time(certificate, evaluation_time_unix_ms, f"App Attest x5c[{index}]")
        chain.append(certificate)

    leaf = chain[0]
    leaf_is_ca, _ = _x509_basic_constraints(leaf, "App Attest leaf certificate")
    leaf_digital_signature, _ = _x509_key_usage(leaf, "App Attest leaf certificate")
    if leaf_is_ca or not leaf_digital_signature:
        raise ValueError("App Attest leaf must be an end-entity signing certificate")
    if leaf.public_key_curve != P256_CURVE or leaf.public_key != expected_public_key:
        raise ValueError("App Attest leaf public key does not match the assertion key")

    for index, (certificate, issuer) in enumerate(zip(chain, chain[1:])):
        issuer_is_ca, path_length = _x509_basic_constraints(
            issuer, f"App Attest x5c[{index + 1}]"
        )
        _, issuer_key_cert_sign = _x509_key_usage(
            issuer, f"App Attest x5c[{index + 1}]"
        )
        if not issuer_is_ca or not issuer_key_cert_sign:
            raise ValueError("App Attest issuer must be a certificate-signing CA")
        if path_length is not None and path_length < index:
            raise ValueError("App Attest certificate chain exceeds issuer path length")
        if certificate.issuer_der != issuer.subject_der:
            raise ValueError("App Attest certificate issuer chain is invalid")
        _verify_x509_signature(certificate, issuer, f"App Attest x5c[{index}]")

    tail = chain[-1]
    anchored = False
    for root_index, root_value in enumerate(policy["trusted_app_attest_roots"]):
        root_der = base64.b64decode(root_value["der_base64"], validate=True)
        root = _parse_x509_certificate(root_der, f"trusted App Attest root[{root_index}]")
        if hashlib.sha256(root.tbs_der).hexdigest() in revoked:
            continue
        _validate_x509_time(
            root, evaluation_time_unix_ms, f"trusted App Attest root[{root_index}]"
        )
        root_is_ca, root_path_length = _x509_basic_constraints(
            root, f"trusted App Attest root[{root_index}]"
        )
        _, root_key_cert_sign = _x509_key_usage(
            root, f"trusted App Attest root[{root_index}]"
        )
        if not root_is_ca or not root_key_cert_sign:
            continue
        if root_path_length is not None and root_path_length < len(chain) - 1:
            continue
        if tail.der == root.der:
            if tail.issuer_der != tail.subject_der:
                raise ValueError("App Attest trusted root is not self-issued")
            _verify_x509_signature(tail, tail, "App Attest trusted root")
            anchored = True
            break
        if tail.issuer_der == root.subject_der:
            _verify_x509_signature(tail, root, "App Attest chain tail")
            anchored = True
            break
    if not anchored:
        raise ValueError("App Attest certificate chain is not anchored in a policy root")

    nonce_extension = leaf.extensions.get(OID_APP_ATTEST_NONCE)
    if nonce_extension is None:
        raise ValueError("App Attest leaf certificate is missing the nonce extension")
    nonce = _extract_app_attest_nonce(nonce_extension.value)
    expected_nonce = hashlib.sha256(
        auth_data + attestation_client_data_hash
    ).digest()
    if nonce != expected_nonce:
        raise ValueError("App Attest leaf nonce does not bind the attestation challenge")
    return nonce


def _validate_extensions(
    value: Any,
    *,
    attestation: bool,
    policy: dict[str, Any],
) -> None:
    category_key = "apple_validation_category_01" if attestation else "validationCategory"
    version_key = "apple_bundle_version_01" if attestation else "bundleVersion"
    extensions = _cbor_object(value, {category_key, version_key}, "App Attest extensions")
    category_value = extensions[category_key]
    # Apple's published 2026 attestation object encodes the documented UInt32
    # validation category as a four-byte little-endian CBOR byte string.  Some
    # WebAuthn decoders expose the same logical value as a CBOR unsigned integer,
    # so normalize exactly those two unambiguous representations.
    if isinstance(category_value, int) and not isinstance(category_value, bool):
        if not 0 <= category_value <= 0xFFFF_FFFF:
            raise ValueError("App Attest validation category is not UInt32")
        category = category_value
    elif isinstance(category_value, bytes) and len(category_value) == 4:
        category = int.from_bytes(category_value, "little")
    else:
        raise ValueError("App Attest validation category is not UInt32")
    version = extensions[version_key]
    if category not in policy["allowed_validation_categories"]:
        raise ValueError("App Attest validation category is not allowed by production policy")
    if version not in policy["allowed_bundle_versions"]:
        raise ValueError("App Attest bundle version is not allowed by production policy")


def _validate_attestation_auth_data(
    auth_data: bytes,
    key_id: bytes,
    public_key: bytes,
    policy: dict[str, Any],
) -> None:
    if not 55 <= len(auth_data) <= MAX_AUTHENTICATOR_DATA_BYTES:
        raise ValueError("App Attest attestation authData length is outside bounds")
    app_id = f"{policy['app_id_prefix']}.{policy['bundle_id']}".encode("ascii")
    if auth_data[:32] != hashlib.sha256(app_id).digest():
        raise ValueError("App Attest attestation RP ID does not match production policy")
    flags = auth_data[32]
    # Apple's production attestation vector carries AT (0x40) without ED
    # (0x80), even though Apple appends its validation-category and bundle-
    # version CBOR map after the credential public key.  Parse that mandatory
    # trailing map below instead of treating WebAuthn's ED bit as authoritative
    # for Apple's App Attest wire format.
    if flags & 0x40 == 0 or flags & ~(0x01 | 0x04 | 0x40 | 0x80):
        raise ValueError("App Attest attestation flags must carry AT and no reserved bits")
    if int.from_bytes(auth_data[33:37], "big") != 0:
        raise ValueError("App Attest attestation counter must start at zero")
    if auth_data[37:53] != b"appattest" + b"\0" * 7:
        raise ValueError("App Attest attestation AAGUID is not production")
    credential_length = int.from_bytes(auth_data[53:55], "big")
    credential_end = 55 + credential_length
    if credential_end > len(auth_data) or auth_data[55:credential_end] != key_id:
        raise ValueError("App Attest attestation credential id does not match key id")
    decoder = _CborDecoder(auth_data[credential_end:])
    cose = _cbor_object(decoder.value(), {1, 3, -1, -2, -3}, "App Attest COSE key")
    if cose[1] != 2 or cose[3] != -7 or cose[-1] != 1:
        raise ValueError("App Attest COSE key is not ES256 P-256")
    if cose[-2] != public_key[1:33] or cose[-3] != public_key[33:]:
        raise ValueError("App Attest COSE key does not match assertion public key")
    extension_bytes = auth_data[credential_end + decoder.offset :]
    _validate_extensions(_decode_cbor(extension_bytes, "App Attest attestation extensions"), attestation=True, policy=policy)


def _parse_attestation_object(
    payload: bytes,
    client_data: bytes,
    key_id: bytes,
    public_key: bytes,
    policy: dict[str, Any],
    evaluation_time_unix_ms: int,
    errors: list[str],
) -> Optional[tuple[tuple[bytes, ...], bytes]]:
    try:
        value = _cbor_object(
            _decode_cbor(payload, "App Attest attestation object"),
            {"fmt", "attStmt", "authData"},
            "App Attest attestation object",
        )
        if value["fmt"] != "apple-appattest" or not isinstance(value["authData"], bytes):
            raise ValueError("App Attest attestation object format is invalid")
        statement = _cbor_object(value["attStmt"], {"x5c", "receipt"}, "App Attest attStmt")
        chain = statement["x5c"]
        receipt = statement["receipt"]
        if (
            not isinstance(chain, tuple)
            or not 2 <= len(chain) <= 4
            or any(not isinstance(cert, bytes) or not cert or len(cert) > MAX_CERTIFICATE_BYTES for cert in chain)
        ):
            raise ValueError("App Attest x5c must contain a bounded leaf/intermediate chain")
        if not isinstance(receipt, bytes) or not receipt or len(receipt) > MAX_RECEIPT_BYTES:
            raise ValueError("App Attest receipt is missing or oversized")
        _validate_attestation_auth_data(value["authData"], key_id, public_key, policy)
        certificate_chain = tuple(chain)
        attestation_nonce = _validate_attestation_certificate_chain(
            certificate_chain,
            policy,
            evaluation_time_unix_ms,
            public_key,
            value["authData"],
            hashlib.sha256(client_data).digest(),
        )
        return certificate_chain, attestation_nonce
    except ValueError as error:
        errors.append(str(error))
        return None


def _parse_assertion_object(
    payload: bytes,
    client_data: bytes,
    public_key: bytes,
    policy: dict[str, Any],
    errors: list[str],
) -> Optional[tuple[int, bytes]]:
    try:
        value = _cbor_object(
            _decode_cbor(payload, "App Attest assertion object"),
            {"signature", "authenticatorData"},
            "App Attest assertion object",
        )
        signature = value["signature"]
        auth_data = value["authenticatorData"]
        if not isinstance(signature, bytes) or not isinstance(auth_data, bytes):
            raise ValueError("App Attest assertion fields must be byte strings")
        if not (
            MIN_ASSERTION_AUTHENTICATOR_DATA_BYTES
            <= len(auth_data)
            <= MAX_AUTHENTICATOR_DATA_BYTES
        ):
            raise ValueError("App Attest assertion authenticatorData length is outside bounds")
        app_id = f"{policy['app_id_prefix']}.{policy['bundle_id']}".encode("ascii")
        if auth_data[:32] != hashlib.sha256(app_id).digest():
            raise ValueError("App Attest assertion RP ID does not match production policy")
        assertion_flags = auth_data[32]
        # App Attest appends its mandatory assertion extension map after the
        # fixed authenticator-data header.  As with Apple's published
        # attestation vector, the map's presence is not reliably advertised by
        # WebAuthn's ED bit.  Reject attested-credential/reserved bits and parse
        # the trailing map itself below.
        if assertion_flags & 0x40 or assertion_flags & ~(0x01 | 0x04 | 0x80):
            raise ValueError("App Attest assertion flags contain AT or reserved bits")
        assertion_counter = int.from_bytes(auth_data[33:37], "big")
        if assertion_counter == 0:
            raise ValueError("App Attest assertion counter must be positive")
        _validate_extensions(
            _decode_cbor(
                auth_data[ASSERTION_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES:],
                "App Attest assertion extensions",
            ),
            attestation=False,
            policy=policy,
        )
        client_data_hash = hashlib.sha256(client_data).digest()
        assertion_message = auth_data + client_data_hash
        _verify_p256_signature(public_key, assertion_message, signature)
        return assertion_counter, hashlib.sha256(assertion_message).digest()
    except ValueError as error:
        errors.append(str(error))
        return None


def _challenge_bindings(
    artifact_digests: dict[str, Any],
    *,
    schema: str,
    domain: str,
    policy_id: str,
    policy_sha256: str,
    release_manifest_sha256: str,
    capture_app_code_sign_measurements_sha256: str,
    evaluated_at_unix_ms: int,
    nonce_base64: str,
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "schema": schema,
        "version": 1,
        "domain": domain,
        "evaluated_at_unix_ms": evaluated_at_unix_ms,
        "policy_id": policy_id,
        "policy_sha256": policy_sha256,
        "release_manifest_sha256": release_manifest_sha256,
        "capture_app_code_sign_measurements_sha256": (
            capture_app_code_sign_measurements_sha256
        ),
        "nonce_base64": nonce_base64,
    }
    for field, artifact in ARTIFACT_CHALLENGE_BINDINGS.items():
        binding = artifact_digests.get(artifact)
        result[field] = binding.get("sha256") if isinstance(binding, dict) else None
    return result


def _validate_challenge(
    payload: bytes,
    *,
    assertion: bool,
    artifact_digests: dict[str, Any],
    policy_id: str,
    policy_sha256: str,
    release_manifest_sha256: str,
    capture_app_code_sign_measurements_sha256: str,
    evaluated_at_unix_ms: int,
    attestation_object_sha256: Optional[str],
    key_id: Optional[str],
    candidate_module: Any,
    errors: list[str],
) -> Optional[dict[str, Any]]:
    label = "App Attest assertion client data" if assertion else "App Attest attestation client data"
    try:
        value = candidate_module.parse_strict_json(payload, label)
    except candidate_module.EvidenceError as error:
        errors.append(str(error))
        return None
    fields = ASSERTION_CHALLENGE_FIELDS if assertion else ATTESTATION_CHALLENGE_FIELDS
    if _exact_fields(value, fields, label, errors) is None:
        return None
    expected = _challenge_bindings(
        artifact_digests,
        schema=ASSERTION_CHALLENGE_SCHEMA if assertion else ATTESTATION_CHALLENGE_SCHEMA,
        domain=ASSERTION_CHALLENGE_DOMAIN if assertion else ATTESTATION_CHALLENGE_DOMAIN,
        policy_id=policy_id,
        policy_sha256=policy_sha256,
        release_manifest_sha256=release_manifest_sha256,
        capture_app_code_sign_measurements_sha256=(
            capture_app_code_sign_measurements_sha256
        ),
        evaluated_at_unix_ms=evaluated_at_unix_ms,
        nonce_base64=value.get("nonce_base64"),
    )
    if assertion:
        expected["attestation_object_sha256"] = attestation_object_sha256
        expected["key_id"] = key_id
    if value != expected:
        errors.append(f"{label} does not bind the exact production policy and benchmark artifacts")
    nonce = _require_base64(
        value.get("nonce_base64"), f"{label} nonce_base64", 32, errors
    )
    if nonce is not None and len(nonce) != 32:
        errors.append(f"{label} nonce_base64 must decode to exactly 32 bytes")
    return value


def _validate_capture_app_code_sign_measurements(
    value: Any,
    policy: dict[str, Any],
    candidate_module: Any,
    errors: list[str],
) -> Optional[str]:
    """Validate and digest the exact prepared App Attest capture executable."""

    starting_errors = len(errors)
    label = "production App Attest capture-app code-sign measurements"
    if (
        _exact_fields(
            value,
            CAPTURE_APP_CODE_SIGN_MEASUREMENTS_FIELDS,
            label,
            errors,
        )
        is None
    ):
        return None
    if value.get("schema") != CAPTURE_APP_CODE_SIGN_MEASUREMENTS_SCHEMA:
        errors.append(
            f"{label} schema must be {CAPTURE_APP_CODE_SIGN_MEASUREMENTS_SCHEMA}"
        )
    if value.get("version") != 1 or isinstance(value.get("version"), bool):
        errors.append(f"{label} version must be integer 1")
    expected_app_id = f"{policy.get('app_id_prefix')}.{policy.get('bundle_id')}"
    expected = {
        "team_id": policy.get("app_id_prefix"),
        "bundle_id": policy.get("bundle_id"),
        "application_identifier": expected_app_id,
        "app_attest_environment": "production",
    }
    for field, expected_value in expected.items():
        if value.get(field) != expected_value:
            errors.append(f"{label} {field} does not match production policy")
    allowed_bundle_versions = policy.get("allowed_bundle_versions")
    if (
        not isinstance(allowed_bundle_versions, list)
        or value.get("bundle_version") not in allowed_bundle_versions
    ):
        errors.append(f"{label} bundle_version is not allowed by production policy")
    executable_sha256 = value.get("executable_sha256")
    if (
        not isinstance(executable_sha256, str)
        or SHA256_RE.fullmatch(executable_sha256) is None
        or executable_sha256 == "0" * 64
    ):
        errors.append(f"{label} executable_sha256 must be a nonzero SHA-256")
    cdhash = value.get("cdhash")
    if (
        not isinstance(cdhash, str)
        or re.fullmatch(r"[0-9a-f]{40}", cdhash) is None
        or cdhash == "0" * 40
    ):
        errors.append(f"{label} cdhash must be nonzero lowercase 40-character hex")
    if len(errors) != starting_errors:
        return None
    try:
        canonical = candidate_module.canonical_json_bytes(value)
    except candidate_module.EvidenceError as error:
        errors.append(str(error))
        return None
    return hashlib.sha256(canonical).hexdigest()


def _validate_platform_evidence(
    platform: Any,
    policy: dict[str, Any],
    policy_sha256: str,
    release_manifest_sha256: str,
    artifact_digests: dict[str, Any],
    raw_snapshot: Any,
    candidate_module: Any,
    errors: list[str],
) -> Optional[_PlatformEvidenceFacts]:
    starting_errors = len(errors)
    if _exact_fields(platform, PLATFORM_EVIDENCE_FIELDS, "production platform evidence", errors) is None:
        return None
    if platform.get("schema") != PLATFORM_EVIDENCE_SCHEMA:
        errors.append(f"production platform evidence schema must be {PLATFORM_EVIDENCE_SCHEMA}")
    if platform.get("version") != 1 or isinstance(platform.get("version"), bool):
        errors.append("production platform evidence version must be integer 1")
    capture_app_measurements_sha256 = _validate_capture_app_code_sign_measurements(
        platform.get("capture_app_code_sign_measurements"),
        policy,
        candidate_module,
        errors,
    )
    try:
        code_sign = candidate_module.parse_strict_json(
            raw_snapshot.json_payloads["build/code-sign-measurements-v1.json"],
            "production code-sign measurements",
        )
        app_identity = code_sign.get("app")
        if not isinstance(app_identity, dict):
            raise candidate_module.EvidenceError(
                "production code-sign measurements app identity is missing"
            )
        if app_identity.get("team_id") != policy["app_id_prefix"]:
            errors.append(
                "production App Attest policy prefix must match the measured app Team ID"
            )
        if app_identity.get("bundle_id") != policy["bundle_id"]:
            errors.append(
                "production App Attest policy bundle must match the measured app bundle"
            )
        if app_identity.get("build") not in policy["allowed_bundle_versions"]:
            errors.append(
                "measured app build is not allowed by the production App Attest policy"
            )
    except (KeyError, candidate_module.EvidenceError) as error:
        errors.append(str(error))
    evaluated_at = platform.get("evaluated_at_unix_ms")
    if isinstance(evaluated_at, bool) or not isinstance(evaluated_at, int) or evaluated_at <= 0:
        errors.append("production platform evidence evaluated_at_unix_ms must be positive")
        evaluated_at = 0
    key_id_text = platform.get("key_id")
    key_id = _require_base64(key_id_text, "production App Attest key_id", 64, errors)
    public_key = _require_base64(
        platform.get("assertion_public_key_sec1_base64"),
        "production App Attest assertion public key",
        P256_PUBLIC_KEY_BYTES,
        errors,
    )
    if public_key is not None:
        try:
            _parse_p256_public_key(public_key)
        except ValueError as error:
            errors.append(str(error))
    if key_id is not None and public_key is not None and key_id != hashlib.sha256(public_key).digest():
        errors.append("production App Attest key_id must equal SHA-256 of the assertion public key")

    attestation_client_data = _require_base64(
        platform.get("attestation_client_data_base64"),
        "App Attest attestation client data",
        MAX_PLATFORM_OBJECT_BYTES,
        errors,
    )
    attestation_object = _require_base64(
        platform.get("attestation_object_base64"),
        "App Attest attestation object",
        MAX_PLATFORM_OBJECT_BYTES,
        errors,
    )
    assertion_client_data = _require_base64(
        platform.get("assertion_client_data_base64"),
        "App Attest assertion client data",
        MAX_PLATFORM_OBJECT_BYTES,
        errors,
    )
    assertion_object = _require_base64(
        platform.get("assertion_object_base64"),
        "App Attest assertion object",
        MAX_PLATFORM_OBJECT_BYTES,
        errors,
    )
    attestation_digest = (
        hashlib.sha256(attestation_object).hexdigest()
        if attestation_object is not None
        else None
    )
    attestation_challenge = None
    assertion_challenge = None
    if attestation_client_data is not None:
        attestation_challenge = _validate_challenge(
            attestation_client_data,
            assertion=False,
            artifact_digests=artifact_digests,
            policy_id=policy["policy_id"],
            policy_sha256=policy_sha256,
            release_manifest_sha256=release_manifest_sha256,
            capture_app_code_sign_measurements_sha256=(
                capture_app_measurements_sha256 or ""
            ),
            evaluated_at_unix_ms=evaluated_at,
            attestation_object_sha256=None,
            key_id=None,
            candidate_module=candidate_module,
            errors=errors,
        )
    if assertion_client_data is not None:
        assertion_challenge = _validate_challenge(
            assertion_client_data,
            assertion=True,
            artifact_digests=artifact_digests,
            policy_id=policy["policy_id"],
            policy_sha256=policy_sha256,
            release_manifest_sha256=release_manifest_sha256,
            capture_app_code_sign_measurements_sha256=(
                capture_app_measurements_sha256 or ""
            ),
            evaluated_at_unix_ms=evaluated_at,
            attestation_object_sha256=attestation_digest,
            key_id=key_id_text if isinstance(key_id_text, str) else None,
            candidate_module=candidate_module,
            errors=errors,
        )
    if (
        attestation_challenge is not None
        and assertion_challenge is not None
        and attestation_challenge.get("nonce_base64") == assertion_challenge.get("nonce_base64")
    ):
        errors.append("App Attest attestation and assertion challenges must use distinct nonces")
    certificate_chain = None
    attestation_statement_nonce = None
    if (
        attestation_object is not None
        and attestation_client_data is not None
        and key_id is not None
        and public_key is not None
    ):
        attestation_result = _parse_attestation_object(
            attestation_object,
            attestation_client_data,
            key_id,
            public_key,
            policy,
            evaluated_at,
            errors,
        )
        if attestation_result is not None:
            certificate_chain, attestation_statement_nonce = attestation_result
    assertion_counter = None
    assertion_statement_nonce = None
    if assertion_object is not None and assertion_client_data is not None and public_key is not None:
        assertion_result = _parse_assertion_object(
            assertion_object, assertion_client_data, public_key, policy, errors
        )
        if assertion_result is not None:
            assertion_counter, assertion_statement_nonce = assertion_result
    if (
        len(errors) != starting_errors
        or not isinstance(key_id_text, str)
        or attestation_client_data is None
        or attestation_object is None
        or assertion_client_data is None
        or assertion_object is None
        or attestation_challenge is None
        or assertion_challenge is None
        or assertion_counter is None
        or assertion_statement_nonce is None
        or attestation_statement_nonce is None
        or certificate_chain is None
    ):
        return None
    attestation_challenge_nonce = _decode_base64(
        attestation_challenge.get("nonce_base64"), "attestation nonce", 32
    )
    assertion_challenge_nonce = _decode_base64(
        assertion_challenge.get("nonce_base64"), "assertion nonce", 32
    )
    if (
        attestation_challenge_nonce is None
        or len(attestation_challenge_nonce) != 32
        or assertion_challenge_nonce is None
        or len(assertion_challenge_nonce) != 32
    ):
        return None
    return _PlatformEvidenceFacts(
        evaluated_at_unix_ms=evaluated_at,
        key_id=key_id_text,
        attestation_client_data=attestation_client_data,
        attestation_object=attestation_object,
        assertion_client_data=assertion_client_data,
        assertion_object=assertion_object,
        attestation_challenge_nonce=attestation_challenge_nonce,
        assertion_challenge_nonce=assertion_challenge_nonce,
        attestation_nonce=attestation_statement_nonce,
        assertion_nonce=assertion_statement_nonce,
        assertion_counter=assertion_counter,
        certificate_chain=certificate_chain,
    )


def _positive_unix_ms(value: Any, label: str, errors: list[str]) -> Optional[int]:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        errors.append(f"{label} must be a positive integer Unix millisecond timestamp")
        return None
    return value


def catalog_revalidation_binding(
    release_manifest_sha256: str,
    evidence_payload: bytes,
    consumption_receipt_payload: bytes,
) -> dict[str, str]:
    """Return the immutable identity of one release's App Attest evidence.

    The short-lived catalog receipt deliberately binds digests of the already
    authenticated immutable envelope and its original one-time-consumption
    receipt. It never replaces or refreshes either historical object.
    """

    if (
        not isinstance(release_manifest_sha256, str)
        or SHA256_RE.fullmatch(release_manifest_sha256) is None
        or release_manifest_sha256 == "0" * 64
    ):
        raise ValueError("catalog release manifest digest must be nonzero lowercase SHA-256")
    if not isinstance(evidence_payload, bytes) or not evidence_payload:
        raise ValueError("catalog signed evidence bytes must be nonempty")
    if (
        not isinstance(consumption_receipt_payload, bytes)
        or not consumption_receipt_payload
    ):
        raise ValueError("catalog consumption receipt bytes must be nonempty")
    return {
        "release_manifest_sha256": release_manifest_sha256,
        "evidence_sha256": hashlib.sha256(evidence_payload).hexdigest(),
        "consumption_receipt_sha256": hashlib.sha256(
            consumption_receipt_payload
        ).hexdigest(),
    }


def catalog_revalidation_digest(
    bindings: list[dict[str, str]], candidate_module: Any
) -> str:
    """Hash one canonical, ordered, duplicate-free immutable release catalog."""

    if not isinstance(bindings, list) or not (
        1 <= len(bindings) <= MAX_CATALOG_REVALIDATION_RELEASES
    ):
        raise ValueError(
            "catalog revalidation binding must contain between 1 and "
            f"{MAX_CATALOG_REVALIDATION_RELEASES} releases"
        )
    normalized: list[dict[str, str]] = []
    prior_manifest = ""
    observed_evidence: set[str] = set()
    observed_consumptions: set[str] = set()
    for index, binding in enumerate(bindings):
        if not isinstance(binding, dict) or set(binding) != set(
            CATALOG_REVALIDATION_BINDING_FIELDS
        ):
            raise ValueError(
                f"catalog revalidation binding[{index}] fields are not exact"
            )
        normalized_binding: dict[str, str] = {}
        for field in sorted(CATALOG_REVALIDATION_BINDING_FIELDS):
            value = binding.get(field)
            if (
                not isinstance(value, str)
                or SHA256_RE.fullmatch(value) is None
                or value == "0" * 64
            ):
                raise ValueError(
                    f"catalog revalidation binding[{index}].{field} must be "
                    "nonzero lowercase SHA-256"
                )
            normalized_binding[field] = value
        manifest = normalized_binding["release_manifest_sha256"]
        evidence = normalized_binding["evidence_sha256"]
        consumption = normalized_binding["consumption_receipt_sha256"]
        if manifest <= prior_manifest:
            raise ValueError(
                "catalog revalidation releases must be strictly ordered by manifest digest"
            )
        if evidence in observed_evidence:
            raise ValueError("catalog revalidation reuses signed evidence bytes")
        if consumption in observed_consumptions:
            raise ValueError("catalog revalidation reuses a consumption receipt")
        prior_manifest = manifest
        observed_evidence.add(evidence)
        observed_consumptions.add(consumption)
        normalized.append(normalized_binding)
    payload = {
        "schema": CATALOG_REVALIDATION_BINDING_SCHEMA,
        "version": 1,
        "releases": normalized,
    }
    return hashlib.sha256(candidate_module.canonical_json_bytes(payload)).hexdigest()


def _validate_online_freshness_receipt(
    receipt_path: Path,
    trusted_key_id: str,
    trusted_public_key_path: Path,
    *,
    evidence_sha256: str,
    policy_sha256: str,
    release_manifest_sha256: str,
    platform: dict[str, Any],
    facts: _PlatformEvidenceFacts,
    lab_signer_key_id: str,
    lab_signer_public_key_sha256: str,
    evaluation_time_unix_ms: int,
    require_current_at_evaluation: bool,
    candidate_module: Any,
    errors: list[str],
) -> None:
    """Verify a stateless claim made by a distinct online authority.

    The signed claim binds current Apple revocation status and states that the
    authority issued and consumed this receipt exactly once.  Verifying the
    claim does not prove the authority's backing state; production operations
    must provision and audit that service before trusting its pinned key. In
    this exact v1 schema, the revocation-labeled status means a current accepted
    Apple App Attest receipt refresh/risk result plus static policy TBS revocations,
    not an independent Apple PKI CRL/OCSP response.
    """

    receipt_snapshot = None
    public_key_snapshot = None
    try:
        candidate_module._validate_key_id(
            trusted_key_id, "trusted online freshness authority key id"
        )
        receipt_snapshot = candidate_module._snapshot_private_file(
            receipt_path.resolve(strict=True),
            "online freshness/consumption receipt",
            maximum=MAX_FRESHNESS_RECEIPT_BYTES,
            retain_payload=True,
        )
        receipt = candidate_module.parse_strict_json(
            receipt_snapshot.payload, "online freshness/consumption receipt"
        )
        public_key_snapshot = candidate_module._snapshot_key_file(
            trusted_public_key_path,
            "online freshness authority public key",
            private=False,
        )
        public_key_der = candidate_module._public_key_der_from_payload(
            public_key_snapshot.payload
        )
    except (OSError, candidate_module.EvidenceError) as error:
        errors.append(str(error))
        return

    if (
        _exact_fields(
            receipt,
            FRESHNESS_RECEIPT_FIELDS,
            "online freshness/consumption receipt",
            errors,
        )
        is None
    ):
        return
    if receipt.get("schema") != FRESHNESS_RECEIPT_SCHEMA:
        errors.append(
            f"online freshness/consumption receipt schema must be {FRESHNESS_RECEIPT_SCHEMA}"
        )
    if receipt.get("version") != 1 or isinstance(receipt.get("version"), bool):
        errors.append("online freshness/consumption receipt version must be integer 1")
    receipt_id = _nonzero_digest(
        receipt.get("receipt_id"),
        "online freshness/consumption receipt receipt_id",
        errors,
    )
    consumption_id = _nonzero_digest(
        receipt.get("consumption_id"),
        "online freshness/consumption receipt consumption_id",
        errors,
    )
    if receipt_id is not None and receipt_id == consumption_id:
        errors.append(
            "online freshness/consumption receipt receipt_id and consumption_id must be distinct"
        )
    issued_at = _positive_unix_ms(
        receipt.get("issued_at_unix_ms"),
        "online freshness/consumption receipt issued_at_unix_ms",
        errors,
    )
    consumed_at = _positive_unix_ms(
        receipt.get("consumed_at_unix_ms"),
        "online freshness/consumption receipt consumed_at_unix_ms",
        errors,
    )
    expires_at = _positive_unix_ms(
        receipt.get("expires_at_unix_ms"),
        "online freshness/consumption receipt expires_at_unix_ms",
        errors,
    )
    revocation_checked_at = _positive_unix_ms(
        receipt.get("apple_revocation_checked_at_unix_ms"),
        "online freshness/consumption receipt apple_revocation_checked_at_unix_ms",
        errors,
    )
    if receipt.get("status") != "issued-and-consumed-once":
        errors.append(
            "online freshness/consumption receipt must attest issued-and-consumed-once"
        )
    if receipt.get("apple_revocation_status") != "good":
        errors.append(
            "online freshness/consumption receipt must attest good Apple revocation status"
        )
    if receipt.get("apple_revocation_source") != ONLINE_REVOCATION_SOURCE:
        errors.append(
            "online freshness/consumption receipt has an unsupported Apple revocation source"
        )
    previous_counter = receipt.get("previous_assertion_counter")
    assertion_counter = receipt.get("assertion_counter")
    if (
        isinstance(previous_counter, bool)
        or not isinstance(previous_counter, int)
        or previous_counter < 0
        or previous_counter > 0xFFFFFFFF
    ):
        errors.append(
            "online freshness/consumption receipt previous_assertion_counter must be a u32"
        )
    if (
        isinstance(assertion_counter, bool)
        or not isinstance(assertion_counter, int)
        or not 1 <= assertion_counter <= 0xFFFFFFFF
    ):
        errors.append(
            "online freshness/consumption receipt assertion_counter must be a positive u32"
        )
    if (
        isinstance(previous_counter, int)
        and not isinstance(previous_counter, bool)
        and isinstance(assertion_counter, int)
        and not isinstance(assertion_counter, bool)
        and assertion_counter <= previous_counter
    ):
        errors.append(
            "online freshness/consumption receipt assertion counter must strictly increase"
        )
    if assertion_counter != facts.assertion_counter:
        errors.append(
            "online freshness/consumption receipt assertion_counter does not bind authenticatorData"
        )
    if (
        issued_at is not None
        and consumed_at is not None
        and expires_at is not None
    ):
        if not issued_at <= consumed_at < expires_at:
            errors.append(
                "online freshness/consumption receipt issuance, consumption, and expiry order is invalid"
            )
        if expires_at - issued_at > MAX_ONLINE_RECEIPT_LIFETIME_MS:
            errors.append("online freshness/consumption receipt lifetime exceeds five minutes")
        if consumed_at > evaluation_time_unix_ms + MAX_ONLINE_CLOCK_SKEW_MS:
            errors.append("online freshness/consumption receipt consumption is in the future")
        if require_current_at_evaluation and evaluation_time_unix_ms > expires_at:
            errors.append("online freshness/consumption receipt is expired")
        evidence_delay = issued_at - facts.evaluated_at_unix_ms
        if not -MAX_ONLINE_CLOCK_SKEW_MS <= evidence_delay <= MAX_ONLINE_RECEIPT_LIFETIME_MS:
            errors.append(
                "online freshness/consumption receipt was not issued promptly for this evidence"
            )
    if issued_at is not None and revocation_checked_at is not None:
        revocation_delay = issued_at - revocation_checked_at
        if not -MAX_ONLINE_CLOCK_SKEW_MS <= revocation_delay <= MAX_ONLINE_REVOCATION_AGE_MS:
            errors.append(
                "online Apple revocation status is not fresh at receipt issuance"
            )

    expected_digests = {
        "evidence_sha256": evidence_sha256,
        "production_policy_sha256": policy_sha256,
        "release_manifest_sha256": release_manifest_sha256,
        "platform_evidence_sha256": hashlib.sha256(
            candidate_module.canonical_json_bytes(platform)
        ).hexdigest(),
        "attestation_client_data_sha256": hashlib.sha256(
            facts.attestation_client_data
        ).hexdigest(),
        "attestation_object_sha256": hashlib.sha256(
            facts.attestation_object
        ).hexdigest(),
        "assertion_client_data_sha256": hashlib.sha256(
            facts.assertion_client_data
        ).hexdigest(),
        "assertion_object_sha256": hashlib.sha256(
            facts.assertion_object
        ).hexdigest(),
        "attestation_challenge_nonce_sha256": hashlib.sha256(
            facts.attestation_challenge_nonce
        ).hexdigest(),
        "assertion_challenge_nonce_sha256": hashlib.sha256(
            facts.assertion_challenge_nonce
        ).hexdigest(),
        "attestation_nonce_sha256": facts.attestation_nonce.hex(),
        "assertion_nonce_sha256": facts.assertion_nonce.hex(),
    }
    for field, expected in expected_digests.items():
        observed = _nonzero_digest(
            receipt.get(field), f"online freshness/consumption receipt {field}", errors
        )
        if observed is not None and observed != expected:
            errors.append(
                f"online freshness/consumption receipt {field} does not bind exact evidence"
            )
    if receipt.get("key_id") != facts.key_id:
        errors.append(
            "online freshness/consumption receipt key_id does not bind exact App Attest key"
        )
    expected_chain = [hashlib.sha256(value).hexdigest() for value in facts.certificate_chain]
    observed_chain = receipt.get("certificate_chain_sha256")
    if observed_chain != expected_chain:
        errors.append(
            "online freshness/consumption receipt certificate_chain_sha256 does not bind exact x5c"
        )
    if receipt.get("signer_key_id") != trusted_key_id:
        errors.append(
            "online freshness/consumption receipt signer_key_id must match trusted authority"
        )
    authority_public_key_sha256 = hashlib.sha256(public_key_der).hexdigest()
    if receipt.get("signer_public_key_sha256") != authority_public_key_sha256:
        errors.append(
            "online freshness/consumption receipt public key digest must match trusted authority"
        )
    if (
        trusted_key_id == lab_signer_key_id
        or authority_public_key_sha256 == lab_signer_public_key_sha256
    ):
        errors.append(
            "online freshness authority must be cryptographically independent from the lab signer"
        )
    if receipt.get("signature_algorithm") != "ed25519":
        errors.append("online freshness/consumption receipt signature must be ed25519")
    try:
        signature_payload = candidate_module.canonical_signature_payload(receipt)
    except candidate_module.EvidenceError as error:
        errors.append(str(error))
        return
    if receipt.get("signature_payload_sha256") != hashlib.sha256(
        signature_payload
    ).hexdigest():
        errors.append(
            "online freshness/consumption receipt signature_payload_sha256 mismatch"
        )
    signature_text = receipt.get("signature")
    if not isinstance(signature_text, str) or re.fullmatch(
        r"[0-9a-f]{128}", signature_text
    ) is None:
        errors.append(
            "online freshness/consumption receipt signature must be 64 lowercase hex bytes"
        )
    else:
        try:
            candidate_module._verify_ed25519_bytes(
                public_key_der[len(candidate_module.ED25519_SPKI_PREFIX) :],
                signature_payload,
                bytes.fromhex(signature_text),
            )
        except candidate_module.EvidenceError as error:
            errors.append(str(error))

    if receipt_snapshot is not None:
        try:
            candidate_module._require_private_file_snapshot_unchanged(
                receipt_snapshot,
                "online freshness/consumption receipt",
                maximum=MAX_FRESHNESS_RECEIPT_BYTES,
            )
        except candidate_module.EvidenceError as error:
            errors.append(str(error))
    if public_key_snapshot is not None:
        try:
            candidate_module._require_key_snapshot_unchanged(
                public_key_snapshot,
                "online freshness authority public key",
                private=False,
            )
        except candidate_module.EvidenceError as error:
            errors.append(str(error))


def validate_catalog_revalidation_receipt(
    receipt_path: Path,
    trusted_key_id: str,
    trusted_public_key_path: Path,
    expected_promotion_id: str,
    expected_bindings: list[dict[str, str]],
    lab_signer_key_id: str,
    lab_signer_public_key_path: Path,
    candidate_module: Any,
    *,
    evaluation_time_unix_ms: Optional[int] = None,
) -> list[str]:
    """Validate current status for one exact immutable multi-release catalog.

    This receipt is intentionally separate from each release's durable
    one-time-consumption receipt. The latter proves capture-time freshness and
    replay protection; this object proves current Apple status for one unique
    promotion and the complete immutable catalog without rewriting history.
    """

    errors: list[str] = []
    receipt_snapshot = None
    authority_key_snapshot = None
    lab_key_snapshot = None
    try:
        candidate_module._validate_key_id(
            trusted_key_id, "trusted catalog revalidation authority key id"
        )
        if (
            not isinstance(expected_promotion_id, str)
            or SHA256_RE.fullmatch(expected_promotion_id) is None
            or expected_promotion_id == "0" * 64
        ):
            errors.append(
                "expected catalog promotion id must be nonzero lowercase SHA-256"
            )
            return errors
        expected_catalog_sha256 = catalog_revalidation_digest(
            expected_bindings, candidate_module
        )
        receipt_snapshot = candidate_module._snapshot_private_file(
            receipt_path.resolve(strict=True),
            "catalog revalidation receipt",
            maximum=MAX_CATALOG_REVALIDATION_RECEIPT_BYTES,
            retain_payload=True,
        )
        receipt = candidate_module.parse_strict_json(
            receipt_snapshot.payload, "catalog revalidation receipt"
        )
        authority_key_snapshot = candidate_module._snapshot_key_file(
            trusted_public_key_path,
            "catalog revalidation authority public key",
            private=False,
        )
        authority_public_der = candidate_module._public_key_der_from_payload(
            authority_key_snapshot.payload
        )
        lab_key_snapshot = candidate_module._snapshot_key_file(
            lab_signer_public_key_path,
            "lab signer public key",
            private=False,
        )
        lab_public_der = candidate_module._public_key_der_from_payload(
            lab_key_snapshot.payload
        )
    except (OSError, ValueError, candidate_module.EvidenceError) as error:
        return errors + [str(error)]

    if (
        _exact_fields(
            receipt,
            CATALOG_REVALIDATION_RECEIPT_FIELDS,
            "catalog revalidation receipt",
            errors,
        )
        is None
    ):
        return errors
    if receipt.get("schema") != CATALOG_REVALIDATION_RECEIPT_SCHEMA:
        errors.append(
            "catalog revalidation receipt schema must be "
            f"{CATALOG_REVALIDATION_RECEIPT_SCHEMA}"
        )
    if receipt.get("version") != 1 or isinstance(receipt.get("version"), bool):
        errors.append("catalog revalidation receipt version must be integer 1")
    receipt_id = _nonzero_digest(
        receipt.get("receipt_id"), "catalog revalidation receipt receipt_id", errors
    )
    promotion_id = _nonzero_digest(
        receipt.get("promotion_id"),
        "catalog revalidation receipt promotion_id",
        errors,
    )
    if receipt_id is not None and receipt_id == promotion_id:
        errors.append("catalog revalidation receipt id must differ from promotion id")
    if promotion_id is not None and promotion_id != expected_promotion_id:
        errors.append("catalog revalidation receipt does not bind this promotion id")
    catalog_sha256 = _nonzero_digest(
        receipt.get("catalog_sha256"),
        "catalog revalidation receipt catalog_sha256",
        errors,
    )
    if catalog_sha256 is not None and catalog_sha256 != expected_catalog_sha256:
        errors.append("catalog revalidation receipt does not bind the exact release catalog")
    issued_at = _positive_unix_ms(
        receipt.get("issued_at_unix_ms"),
        "catalog revalidation receipt issued_at_unix_ms",
        errors,
    )
    expires_at = _positive_unix_ms(
        receipt.get("expires_at_unix_ms"),
        "catalog revalidation receipt expires_at_unix_ms",
        errors,
    )
    if evaluation_time_unix_ms is None:
        evaluation_time_unix_ms = time.time_ns() // 1_000_000
    if (
        isinstance(evaluation_time_unix_ms, bool)
        or not isinstance(evaluation_time_unix_ms, int)
        or evaluation_time_unix_ms <= 0
    ):
        errors.append("catalog revalidation evaluation time must be positive Unix milliseconds")
    if issued_at is not None and expires_at is not None:
        if not issued_at < expires_at:
            errors.append("catalog revalidation receipt issuance/expiry order is invalid")
        if expires_at - issued_at > MAX_ONLINE_RECEIPT_LIFETIME_MS:
            errors.append("catalog revalidation receipt lifetime exceeds five minutes")
        if (
            isinstance(evaluation_time_unix_ms, int)
            and not isinstance(evaluation_time_unix_ms, bool)
        ):
            if issued_at > evaluation_time_unix_ms + MAX_ONLINE_CLOCK_SKEW_MS:
                errors.append("catalog revalidation receipt issuance is in the future")
            if evaluation_time_unix_ms > expires_at:
                errors.append("catalog revalidation receipt is expired")
    if receipt.get("status") != "catalog-revalidated-for-one-promotion":
        errors.append(
            "catalog revalidation receipt must attest catalog-revalidated-for-one-promotion"
        )

    statuses = receipt.get("release_statuses")
    if not isinstance(statuses, list) or len(statuses) != len(expected_bindings):
        errors.append(
            "catalog revalidation receipt release statuses must exactly cover the catalog"
        )
        statuses = []
    for index, status in enumerate(statuses):
        label = f"catalog revalidation receipt release_statuses[{index}]"
        if not isinstance(status, dict) or set(status) != set(
            CATALOG_REVALIDATION_RELEASE_STATUS_FIELDS
        ):
            errors.append(f"{label} fields are not exact")
            continue
        expected = expected_bindings[index]
        for field in CATALOG_REVALIDATION_BINDING_FIELDS:
            observed = _nonzero_digest(status.get(field), f"{label}.{field}", errors)
            if observed is not None and observed != expected.get(field):
                errors.append(f"{label}.{field} does not bind the exact immutable release")
        key_id = status.get("app_attest_key_id")
        if not isinstance(key_id, str) or not key_id or len(key_id) > 1024:
            errors.append(f"{label}.app_attest_key_id is invalid")
        checked_at = _positive_unix_ms(
            status.get("apple_status_checked_at_unix_ms"),
            f"{label}.apple_status_checked_at_unix_ms",
            errors,
        )
        if status.get("apple_status") != "good":
            errors.append(f"{label}.apple_status must be good")
        if status.get("apple_status_source") != ONLINE_REVOCATION_SOURCE:
            errors.append(f"{label}.apple_status_source is unsupported")
        _nonzero_digest(
            status.get("refreshed_apple_receipt_sha256"),
            f"{label}.refreshed_apple_receipt_sha256",
            errors,
        )
        risk_metric = status.get("risk_metric")
        if (
            isinstance(risk_metric, bool)
            or not isinstance(risk_metric, int)
            or not 0 <= risk_metric <= 0x7FFFFFFF
        ):
            errors.append(f"{label}.risk_metric is invalid")
        if checked_at is not None and issued_at is not None:
            age = issued_at - checked_at
            if not -MAX_ONLINE_CLOCK_SKEW_MS <= age <= MAX_ONLINE_REVOCATION_AGE_MS:
                errors.append(f"{label} Apple status is not fresh at catalog issuance")

    authority_public_key_sha256 = hashlib.sha256(authority_public_der).hexdigest()
    lab_public_key_sha256 = hashlib.sha256(lab_public_der).hexdigest()
    if receipt.get("signer_key_id") != trusted_key_id:
        errors.append("catalog revalidation receipt signer key id is not trusted")
    if receipt.get("signer_public_key_sha256") != authority_public_key_sha256:
        errors.append("catalog revalidation receipt signer public key is not trusted")
    if (
        trusted_key_id == lab_signer_key_id
        or authority_public_key_sha256 == lab_public_key_sha256
    ):
        errors.append(
            "catalog revalidation authority must be cryptographically independent from the lab signer"
        )
    if receipt.get("signature_algorithm") != "ed25519":
        errors.append("catalog revalidation receipt signature must be ed25519")
    try:
        signature_payload = candidate_module.canonical_signature_payload(receipt)
    except candidate_module.EvidenceError as error:
        errors.append(str(error))
        return errors
    if receipt.get("signature_payload_sha256") != hashlib.sha256(
        signature_payload
    ).hexdigest():
        errors.append("catalog revalidation receipt signature payload digest mismatch")
    signature_text = receipt.get("signature")
    if not isinstance(signature_text, str) or re.fullmatch(
        r"[0-9a-f]{128}", signature_text
    ) is None:
        errors.append("catalog revalidation receipt signature must be 64 lowercase hex bytes")
    else:
        try:
            candidate_module._verify_ed25519_bytes(
                authority_public_der[len(candidate_module.ED25519_SPKI_PREFIX) :],
                signature_payload,
                bytes.fromhex(signature_text),
            )
        except candidate_module.EvidenceError as error:
            errors.append(str(error))

    for snapshot, label, maximum in (
        (
            receipt_snapshot,
            "catalog revalidation receipt",
            MAX_CATALOG_REVALIDATION_RECEIPT_BYTES,
        ),
    ):
        if snapshot is not None:
            try:
                candidate_module._require_private_file_snapshot_unchanged(
                    snapshot, label, maximum=maximum
                )
            except candidate_module.EvidenceError as error:
                errors.append(str(error))
    for snapshot, label in (
        (authority_key_snapshot, "catalog revalidation authority public key"),
        (lab_key_snapshot, "lab signer public key"),
    ):
        if snapshot is not None:
            try:
                candidate_module._require_key_snapshot_unchanged(
                    snapshot, label, private=False
                )
            except candidate_module.EvidenceError as error:
                errors.append(str(error))
    return errors


def _validate_production_signed_evidence(
    evidence_path: Path,
    artifact_root: Path,
    trusted_key_id: str,
    trusted_public_key_path: Path,
    production_policy_path: Path,
    candidate_module: Any,
    *,
    freshness_receipt_path: Optional[Path] = None,
    trusted_freshness_key_id: Optional[str] = None,
    trusted_freshness_public_key_path: Optional[Path] = None,
    evaluation_time_unix_ms: Optional[int] = None,
    require_current_freshness_receipt: bool,
) -> list[str]:
    """Shared implementation for current and catalog-historical validation.

    Repository-local validation covers the complete static X.509 path, leaf
    nonce, certificate time, policy revocations, and exact online receipt
    signature/bindings.  A valid receipt and independently pinned authority key
    are the success path. Production operations remain responsible for
    reviewing the authority's durable issuance/consumption state. The public
    production entrypoint always requires current freshness. The
    distinct historical entrypoint exists only for the online authority and
    promotion gate, which separately require a current exact-catalog receipt.
    """

    errors: list[str] = []
    evidence_snapshot = None
    policy_snapshot = None
    public_key_snapshot = None
    raw_snapshot = None
    try:
        candidate_module._validate_key_id(trusted_key_id, "trusted key id")
        evidence_absolute = evidence_path.resolve(strict=True)
        root_absolute = artifact_root.resolve(strict=True)
        policy_absolute = production_policy_path.resolve(strict=True)
        for path, label in (
            (evidence_absolute, "signed production evidence"),
            (policy_absolute, "production policy"),
        ):
            try:
                path.relative_to(root_absolute)
            except ValueError:
                pass
            else:
                errors.append(f"{label} must stay outside artifact root")
        evidence_snapshot = candidate_module._snapshot_private_file(
            evidence_absolute,
            "signed production evidence",
            maximum=candidate_module.MAX_JSON_BYTES,
            retain_payload=True,
        )
        policy_snapshot = candidate_module._snapshot_private_file(
            policy_absolute,
            "production policy",
            maximum=MAX_POLICY_BYTES,
            retain_payload=True,
        )
        evidence = candidate_module.parse_strict_json(
            evidence_snapshot.payload, "signed production evidence"
        )
        policy = candidate_module.parse_strict_json(
            policy_snapshot.payload, "production policy"
        )
        public_key_snapshot = candidate_module._snapshot_key_file(
            trusted_public_key_path, "public key", private=False
        )
        trusted_public_der = candidate_module._public_key_der_from_payload(
            public_key_snapshot.payload
        )
    except (OSError, candidate_module.EvidenceError) as error:
        return errors + [str(error)]

    if _exact_fields(
        evidence,
        PRODUCTION_SIGNED_EVIDENCE_FIELDS,
        "signed production evidence",
        errors,
    ) is None:
        return errors
    policy_valid = _validate_policy(policy, policy_snapshot.payload, errors)
    policy_object = policy if isinstance(policy, dict) else {}
    policy_digest = hashlib.sha256(policy_snapshot.payload).hexdigest()
    if evidence.get("schema") != PRODUCTION_SIGNED_EVIDENCE_SCHEMA:
        errors.append(
            f"signed production evidence schema must be {PRODUCTION_SIGNED_EVIDENCE_SCHEMA}"
        )
    if evidence.get("version") != 1 or isinstance(evidence.get("version"), bool):
        errors.append("signed production evidence version must be integer 1")
    release_manifest_sha256 = _nonzero_digest(
        evidence.get("release_manifest_sha256"),
        "signed production evidence release_manifest_sha256",
        errors,
    )
    if evidence.get("production_policy_id") != policy_object.get("policy_id"):
        errors.append("signed production evidence production_policy_id must match policy")
    if evidence.get("production_policy_sha256") != policy_digest:
        errors.append("signed production evidence production_policy_sha256 must match exact policy bytes")
    if evidence.get("signer_key_id") != trusted_key_id:
        errors.append("signed production evidence signer_key_id must match trusted key id")
    if evidence.get("signature_algorithm") != "ed25519":
        errors.append("signed production evidence signature_algorithm must be ed25519")
    trusted_digest = hashlib.sha256(trusted_public_der).hexdigest()
    if evidence.get("signer_public_key_sha256") != trusted_digest:
        errors.append("signed production evidence public key digest must match trusted key")

    try:
        payload = candidate_module.canonical_signature_payload(evidence)
    except candidate_module.EvidenceError as error:
        errors.append(str(error))
        return errors
    if evidence.get("signature_payload_sha256") != hashlib.sha256(payload).hexdigest():
        errors.append("signed production evidence signature_payload_sha256 mismatch")
    signature_text = evidence.get("signature")
    signature = None
    if isinstance(signature_text, str) and re.fullmatch(r"[0-9a-f]{128}", signature_text):
        signature = bytes.fromhex(signature_text)
    else:
        errors.append("signed production evidence signature must be 64 lowercase hex bytes")

    try:
        raw_snapshot = candidate_module.snapshot_raw_artifacts(root_absolute)
        digests = raw_snapshot.digests
        sizes = raw_snapshot.sizes
    except candidate_module.EvidenceError as error:
        errors.append(str(error))
        digests, sizes = {}, {}
    artifact_digests = evidence.get("artifact_digests")
    expected_artifacts = {
        relative: {"size_bytes": sizes[relative], "sha256": digest}
        for relative, digest in digests.items()
    }
    if not isinstance(artifact_digests, dict):
        errors.append("signed production evidence artifact_digests must be an object")
        artifact_digests = {}
    elif artifact_digests != expected_artifacts:
        errors.append("signed production evidence artifact_digests must equal the exact raw tree")
    if raw_snapshot is not None:
        errors.extend(candidate_module.validate_raw_evidence(raw_snapshot))
    if signature is not None:
        try:
            candidate_module._verify_ed25519_bytes(
                trusted_public_der[len(candidate_module.ED25519_SPKI_PREFIX) :],
                payload,
                signature,
            )
        except candidate_module.EvidenceError as error:
            errors.append(str(error))
    if (
        policy_valid
        and release_manifest_sha256 is not None
        and artifact_digests
        and raw_snapshot is not None
    ):
        platform_facts = _validate_platform_evidence(
            evidence.get("platform_evidence"),
            policy_object,
            policy_digest,
            release_manifest_sha256,
            artifact_digests,
            raw_snapshot,
            candidate_module,
            errors,
        )
    else:
        platform_facts = None

    freshness_parameters = (
        freshness_receipt_path,
        trusted_freshness_key_id,
        trusted_freshness_public_key_path,
    )
    if any(value is None for value in freshness_parameters):
        errors.append(MISSING_FRESHNESS_RECEIPT)
    elif platform_facts is not None and release_manifest_sha256 is not None:
        assert freshness_receipt_path is not None
        assert trusted_freshness_key_id is not None
        assert trusted_freshness_public_key_path is not None
        try:
            freshness_absolute = freshness_receipt_path.resolve(strict=True)
            freshness_absolute.relative_to(root_absolute)
        except ValueError:
            pass
        except OSError as error:
            errors.append(
                f"online freshness/consumption receipt could not be resolved: {error}"
            )
        else:
            errors.append(
                "online freshness/consumption receipt must stay outside artifact root"
            )
        if evaluation_time_unix_ms is None:
            evaluation_time_unix_ms = time.time_ns() // 1_000_000
        if (
            isinstance(evaluation_time_unix_ms, bool)
            or not isinstance(evaluation_time_unix_ms, int)
            or evaluation_time_unix_ms <= 0
        ):
            errors.append("online receipt evaluation time must be positive Unix milliseconds")
        else:
            platform_value = evidence.get("platform_evidence")
            if not isinstance(platform_value, dict):
                errors.append("production platform evidence must be an object")
            else:
                _validate_online_freshness_receipt(
                    freshness_receipt_path,
                    trusted_freshness_key_id,
                    trusted_freshness_public_key_path,
                    evidence_sha256=evidence_snapshot.sha256,
                    policy_sha256=policy_digest,
                    release_manifest_sha256=release_manifest_sha256,
                    platform=platform_value,
                    facts=platform_facts,
                    lab_signer_key_id=trusted_key_id,
                    lab_signer_public_key_sha256=trusted_digest,
                    evaluation_time_unix_ms=evaluation_time_unix_ms,
                    require_current_at_evaluation=require_current_freshness_receipt,
                    candidate_module=candidate_module,
                    errors=errors,
                )

    if raw_snapshot is not None:
        try:
            candidate_module._require_raw_snapshot_unchanged(raw_snapshot)
        except candidate_module.EvidenceError as error:
            errors.append(str(error))
    for snapshot, label, maximum in (
        (evidence_snapshot, "signed production evidence", candidate_module.MAX_JSON_BYTES),
        (policy_snapshot, "production policy", MAX_POLICY_BYTES),
    ):
        if snapshot is not None:
            try:
                candidate_module._require_private_file_snapshot_unchanged(
                    snapshot, label, maximum=maximum
                )
            except candidate_module.EvidenceError as error:
                errors.append(str(error))
    if public_key_snapshot is not None:
        try:
            candidate_module._require_key_snapshot_unchanged(
                public_key_snapshot, "public key", private=False
            )
        except candidate_module.EvidenceError as error:
            errors.append(str(error))

    return errors


def validate_production_signed_evidence(
    evidence_path: Path,
    artifact_root: Path,
    trusted_key_id: str,
    trusted_public_key_path: Path,
    production_policy_path: Path,
    candidate_module: Any,
    *,
    freshness_receipt_path: Optional[Path] = None,
    trusted_freshness_key_id: Optional[str] = None,
    trusted_freshness_public_key_path: Optional[Path] = None,
    evaluation_time_unix_ms: Optional[int] = None,
) -> list[str]:
    """Validate production evidence with a currently valid consumption receipt."""

    return _validate_production_signed_evidence(
        evidence_path,
        artifact_root,
        trusted_key_id,
        trusted_public_key_path,
        production_policy_path,
        candidate_module,
        freshness_receipt_path=freshness_receipt_path,
        trusted_freshness_key_id=trusted_freshness_key_id,
        trusted_freshness_public_key_path=trusted_freshness_public_key_path,
        evaluation_time_unix_ms=evaluation_time_unix_ms,
        require_current_freshness_receipt=True,
    )


def validate_historical_production_evidence_for_catalog_revalidation(
    evidence_path: Path,
    artifact_root: Path,
    trusted_key_id: str,
    trusted_public_key_path: Path,
    production_policy_path: Path,
    candidate_module: Any,
    *,
    freshness_receipt_path: Path,
    trusted_freshness_key_id: str,
    trusted_freshness_public_key_path: Path,
    evaluation_time_unix_ms: Optional[int] = None,
) -> list[str]:
    """Authenticate immutable history before requiring fresh catalog status.

    This does not make a stale receipt current. It retains every signature,
    prompt-issuance, lifetime, exact-binding, and one-time-consumption check so
    the authority/gate can then require a separate current catalog receipt.
    """

    return _validate_production_signed_evidence(
        evidence_path,
        artifact_root,
        trusted_key_id,
        trusted_public_key_path,
        production_policy_path,
        candidate_module,
        freshness_receipt_path=freshness_receipt_path,
        trusted_freshness_key_id=trusted_freshness_key_id,
        trusted_freshness_public_key_path=trusted_freshness_public_key_path,
        evaluation_time_unix_ms=evaluation_time_unix_ms,
        require_current_freshness_receipt=False,
    )


def build_production_signed_evidence(
    artifact_root: Path,
    platform_evidence_path: Path,
    production_policy_path: Path,
    release_manifest_sha256: str,
    private_key_path: Path,
    public_key_path: Path,
    signer_key_id: str,
    candidate_module: Any,
) -> dict[str, Any]:
    """Validate and sign one exact release-bound production envelope.

    The online authority receipt is intentionally not accepted here: it must
    be issued only after this immutable envelope exists and can be digested by
    the independently operated freshness/consumption authority.
    """

    candidate_module._validate_key_id(signer_key_id)
    if (
        SHA256_RE.fullmatch(release_manifest_sha256) is None
        or release_manifest_sha256 == "0" * 64
    ):
        raise candidate_module.EvidenceError(
            "release manifest digest must be nonzero lowercase SHA-256"
        )
    try:
        root_absolute = artifact_root.resolve(strict=True)
        platform_absolute = platform_evidence_path.resolve(strict=True)
        policy_absolute = production_policy_path.resolve(strict=True)
    except OSError as error:
        raise candidate_module.EvidenceError(
            "production envelope inputs could not be resolved"
        ) from error
    for path, label in (
        (platform_absolute, "production platform evidence"),
        (policy_absolute, "production policy"),
    ):
        try:
            path.relative_to(root_absolute)
        except ValueError:
            pass
        else:
            raise candidate_module.EvidenceError(
                f"{label} must stay outside artifact root"
            )

    platform_snapshot = candidate_module._snapshot_private_file(
        platform_absolute,
        "production platform evidence",
        maximum=candidate_module.MAX_JSON_BYTES,
        retain_payload=True,
    )
    policy_snapshot = candidate_module._snapshot_private_file(
        policy_absolute,
        "production policy",
        maximum=MAX_POLICY_BYTES,
        retain_payload=True,
    )
    private_key = candidate_module._snapshot_key_file(
        private_key_path,
        "private key",
        private=True,
    )
    public_key = candidate_module._snapshot_key_file(
        public_key_path,
        "public key",
        private=False,
    )
    platform = candidate_module.parse_strict_json(
        platform_snapshot.payload, "production platform evidence"
    )
    policy = candidate_module.parse_strict_json(
        policy_snapshot.payload, "production policy"
    )
    private_seed = candidate_module._private_seed_from_payload(private_key.payload)
    public_der = candidate_module._public_key_der_from_payload(public_key.payload)
    public_bytes = public_der[len(candidate_module.ED25519_SPKI_PREFIX) :]
    if candidate_module._ed25519_public_from_seed(private_seed) != public_bytes:
        raise candidate_module.EvidenceError(
            "private and public Ed25519 keys do not match"
        )

    raw_snapshot = candidate_module.snapshot_raw_artifacts(root_absolute)
    errors = candidate_module.validate_raw_evidence(raw_snapshot)
    policy_valid = _validate_policy(policy, policy_snapshot.payload, errors)
    artifact_digests = {
        relative: {
            "size_bytes": raw_snapshot.sizes[relative],
            "sha256": digest,
        }
        for relative, digest in raw_snapshot.digests.items()
    }
    if policy_valid:
        _validate_platform_evidence(
            platform,
            policy,
            policy_snapshot.sha256,
            release_manifest_sha256,
            artifact_digests,
            raw_snapshot,
            candidate_module,
            errors,
        )
    if errors:
        raise candidate_module.EvidenceError("; ".join(errors))

    evidence: dict[str, Any] = {
        "schema": PRODUCTION_SIGNED_EVIDENCE_SCHEMA,
        "version": 1,
        "release_manifest_sha256": release_manifest_sha256,
        "production_policy_id": policy["policy_id"],
        "production_policy_sha256": policy_snapshot.sha256,
        "platform_evidence": platform,
        "artifact_digests": artifact_digests,
        "signer_key_id": signer_key_id,
        "signer_public_key_sha256": hashlib.sha256(public_der).hexdigest(),
        "signature_algorithm": "ed25519",
    }
    payload = candidate_module.canonical_signature_payload(evidence)
    signature = candidate_module._sign_ed25519_seed(private_seed, payload)
    candidate_module._verify_ed25519_bytes(public_bytes, payload, signature)
    evidence["signature_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    evidence["signature"] = signature.hex()

    candidate_module._require_raw_snapshot_unchanged(raw_snapshot)
    candidate_module._require_private_file_snapshot_unchanged(
        platform_snapshot,
        "production platform evidence",
        maximum=candidate_module.MAX_JSON_BYTES,
    )
    candidate_module._require_private_file_snapshot_unchanged(
        policy_snapshot,
        "production policy",
        maximum=MAX_POLICY_BYTES,
    )
    candidate_module._require_key_snapshot_unchanged(
        private_key, "private key", private=True
    )
    candidate_module._require_key_snapshot_unchanged(
        public_key, "public key", private=False
    )
    return evidence
