"""Fail-closed validation substrate for production iOS Kagemusha evidence.

The candidate lab contract deliberately proves an offline physical-device run
without using App Attest.  This module defines a separate production envelope
which binds that exact run to a governed policy and verifies the App Attest
assertion cryptographically.  The final Apple X.509 chain/nonce validation is
not implemented here yet; validation therefore always reports the explicit
``PLATFORM_TRUST_BLOCKER`` after checking every substrate invariant.

Keeping that blocker is intentional.  A signed lab statement, a Boolean, or an
unverified certificate array must never be promoted as Secure Enclave proof.
"""

from __future__ import annotations

import base64
from dataclasses import dataclass
import hashlib
import re
from pathlib import Path
from typing import Any, Optional


PRODUCTION_SIGNED_EVIDENCE_SCHEMA = (
    "iroha.kagemusha.ios_device_lab.production_signed_evidence.v1"
)
PRODUCTION_POLICY_SCHEMA = "iroha.kagemusha.ios.production_device_policy.v1"
PLATFORM_EVIDENCE_SCHEMA = "iroha.kagemusha.ios.app_attest_evidence.v1"
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
PLATFORM_TRUST_BLOCKER = (
    "production App Attest trust remains blocked: the reviewed validator does "
    "not yet authenticate the Apple X.509 chain, certificate validity and "
    "revocation, the leaf nonce extension against the attestation challenge, "
    "or independently issued freshness/replay state"
)

MAX_POLICY_BYTES = 1024 * 1024
MAX_PLATFORM_OBJECT_BYTES = 128 * 1024
MAX_CERTIFICATE_BYTES = 64 * 1024
MAX_RECEIPT_BYTES = 64 * 1024
MAX_AUTHENTICATOR_DATA_BYTES = 4 * 1024
P256_PUBLIC_KEY_BYTES = 65
P256_SIGNATURE_DER_MAX_BYTES = 80

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
        "revoked_certificate_sha256",
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
            return tuple(self.value(depth + 1) for _ in range(argument))
        if major == 5:
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
    policy: dict[str, Any], policy_bytes: bytes, errors: list[str]
) -> None:
    if _exact_fields(policy, PRODUCTION_POLICY_FIELDS, "production iOS policy", errors) is None:
        return
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
            if der is not None and not der.startswith(b"\x30"):
                errors.append(f"{label}.der_base64 is not a DER certificate envelope")
            if digest is not None:
                root_digests.append(digest)
        if root_digests != sorted(set(root_digests)):
            errors.append("production iOS trusted App Attest roots must be ordered by unique digest")

    revoked = policy.get("revoked_certificate_sha256")
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
        errors.append("production iOS revoked_certificate_sha256 must be a sorted unique digest list")
    elif set(root_digests) & set(revoked):
        errors.append("production iOS trusted root must not also be revoked")
    if len(policy_bytes) > MAX_POLICY_BYTES:
        errors.append("production iOS policy exceeds its byte limit")


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


def _validate_extensions(
    value: Any,
    *,
    attestation: bool,
    policy: dict[str, Any],
) -> None:
    category_key = "apple_validation_category_01" if attestation else "validationCategory"
    version_key = "apple_bundle_version_01" if attestation else "bundleVersion"
    extensions = _cbor_object(value, {category_key, version_key}, "App Attest extensions")
    category = extensions[category_key]
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
    if flags & 0x40 == 0 or flags & 0x80 == 0 or flags & ~(0x01 | 0x04 | 0x40 | 0x80):
        raise ValueError("App Attest attestation flags must carry AT and production extensions")
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
    key_id: bytes,
    public_key: bytes,
    policy: dict[str, Any],
    errors: list[str],
) -> None:
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
    except ValueError as error:
        errors.append(str(error))


def _parse_assertion_object(
    payload: bytes,
    client_data: bytes,
    public_key: bytes,
    policy: dict[str, Any],
    errors: list[str],
) -> None:
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
        if not 37 < len(auth_data) <= MAX_AUTHENTICATOR_DATA_BYTES:
            raise ValueError("App Attest assertion authenticatorData length is outside bounds")
        app_id = f"{policy['app_id_prefix']}.{policy['bundle_id']}".encode("ascii")
        if auth_data[:32] != hashlib.sha256(app_id).digest():
            raise ValueError("App Attest assertion RP ID does not match production policy")
        if auth_data[32] != 0x80:
            raise ValueError("App Attest assertion must contain only the extension-data flag")
        if int.from_bytes(auth_data[33:37], "big") == 0:
            raise ValueError("App Attest assertion counter must be positive")
        _validate_extensions(
            _decode_cbor(auth_data[37:], "App Attest assertion extensions"),
            attestation=False,
            policy=policy,
        )
        client_data_hash = hashlib.sha256(client_data).digest()
        _verify_p256_signature(public_key, auth_data + client_data_hash, signature)
    except ValueError as error:
        errors.append(str(error))


def _challenge_bindings(
    artifact_digests: dict[str, Any],
    *,
    schema: str,
    domain: str,
    policy_id: str,
    policy_sha256: str,
    release_manifest_sha256: str,
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
        evaluated_at_unix_ms=evaluated_at_unix_ms,
        nonce_base64=value.get("nonce_base64"),
    )
    if assertion:
        expected["attestation_object_sha256"] = attestation_object_sha256
        expected["key_id"] = key_id
    if value != expected:
        errors.append(f"{label} does not bind the exact production policy and benchmark artifacts")
    _require_base64(value.get("nonce_base64"), f"{label} nonce_base64", 32, errors)
    return value


def _validate_platform_evidence(
    platform: Any,
    policy: dict[str, Any],
    policy_sha256: str,
    release_manifest_sha256: str,
    artifact_digests: dict[str, Any],
    raw_snapshot: Any,
    candidate_module: Any,
    errors: list[str],
) -> None:
    if _exact_fields(platform, PLATFORM_EVIDENCE_FIELDS, "production platform evidence", errors) is None:
        return
    if platform.get("schema") != PLATFORM_EVIDENCE_SCHEMA:
        errors.append(f"production platform evidence schema must be {PLATFORM_EVIDENCE_SCHEMA}")
    if platform.get("version") != 1 or isinstance(platform.get("version"), bool):
        errors.append("production platform evidence version must be integer 1")
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
    if attestation_object is not None and key_id is not None and public_key is not None:
        _parse_attestation_object(attestation_object, key_id, public_key, policy, errors)
    if assertion_object is not None and assertion_client_data is not None and public_key is not None:
        _parse_assertion_object(assertion_object, assertion_client_data, public_key, policy, errors)


def validate_production_signed_evidence(
    evidence_path: Path,
    artifact_root: Path,
    trusted_key_id: str,
    trusted_public_key_path: Path,
    production_policy_path: Path,
    candidate_module: Any,
) -> list[str]:
    """Validate the production envelope and return an explicit trust blocker.

    The returned list can be empty only after this module gains reviewed Apple
    X.509 chain, leaf nonce, time, and revocation verification.  Until then a
    structurally and cryptographically sound fixture returns exactly the
    platform trust blocker.
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
        return errors + [PLATFORM_TRUST_BLOCKER]
    _validate_policy(policy, policy_snapshot.payload, errors)
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
    if evidence.get("production_policy_id") != policy.get("policy_id"):
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
        return errors + [PLATFORM_TRUST_BLOCKER]
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
        isinstance(policy, dict)
        and set(policy) == PRODUCTION_POLICY_FIELDS
        and release_manifest_sha256 is not None
        and artifact_digests
        and raw_snapshot is not None
    ):
        _validate_platform_evidence(
            evidence.get("platform_evidence"),
            policy,
            policy_digest,
            release_manifest_sha256,
            artifact_digests,
            raw_snapshot,
            candidate_module,
            errors,
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

    # TODO: Replace this blocker only with direct, reviewed validation matching
    # `validate_ios_app_attest_report` in iroha_core: pinned Apple roots, the
    # entire X.509 chain, validity/revocation, leaf public key and nonce OID.
    errors.append(PLATFORM_TRUST_BLOCKER)
    return errors
