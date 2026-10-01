"""Independently check public iPhone App Attest XCTest attachments.

This checks the raw Apple attestation, its embedded fraud receipt, and two
exact-next assertions. It does not issue a certificate or admit offline money.
The attestation and receipt pins are the SHA-256 digests of Apple's published
App Attestation Root CA and Apple Root CA - G3 DER certificates, respectively.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import shutil
import time
from pathlib import Path

from .apple_receipt import verify_apple_receipt
from .attestation import Selection, cbor_exact, verify_apple_assertion, verify_apple_raw


APPLE_ROOT_SHA256 = bytes.fromhex(
    "1cb9823ba28ba6ad2d33a006941de2ae4f513ef1d4e831b9f7e0fa7b6242c932"
)
APPLE_RECEIPT_ROOT_SHA256 = bytes.fromhex(
    "63343abfb89a6a03ebb57e9b3f5fa7be7c4f5c756f3017b3a8c488c3653e9179"
)
SELECTION_DOMAIN = b"iroha:kagemusha:v1:hardware-transition-selection\0"
PHYSICAL_RELEASE_ID = b"\x33" * 32
PHYSICAL_PROFILE_ID = b"\x44" * 32
PHYSICAL_LANE_ID = b"\x55" * 32
ENROLLMENT_FIELDS = frozenset({
    "keyID", "rawAttestationBase64", "clientNonceBase64", "serverNonceBase64",
    "releaseIDBase64", "profileIDBase64", "attestedKeyIDBase64", "laneIDBase64",
    "expectedAppIDHashBase64", "expectedReleaseDigestBase64", "bundleVersion",
})
ASSERTION_FIELDS = frozenset({
    "keyID", "enrolledPublicKeyX963Base64", "expectedAppIDHashBase64",
    "firstSelectionBase64", "firstAssertionBase64", "secondSelectionBase64",
    "secondAssertionBase64",
})


def _unique_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate physical evidence field")
        result[key] = value
    return result


def _record(path: Path, fields: frozenset[str]) -> dict[str, str]:
    if not path.is_file() or path.stat().st_size > 256 * 1024:
        raise ValueError("physical evidence file missing or oversized")
    value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_unique_pairs)
    if not isinstance(value, dict) or set(value) != fields or not all(
        type(item) is str for item in value.values()
    ):
        raise ValueError("physical evidence fields differ from the XCTest contract")
    return value


def _bytes(value: str, maximum: int) -> bytes:
    if len(value) > ((maximum + 2) // 3) * 4:
        raise ValueError("oversized physical evidence field")
    decoded = base64.b64decode(value, validate=True)
    if not decoded or len(decoded) > maximum or base64.b64encode(decoded).decode("ascii") != value:
        raise ValueError("noncanonical physical evidence field")
    return decoded


def _release_digest(bundle_version: str) -> bytes:
    """Independently compute the physical development-app policy digest."""
    try:
        version = bundle_version.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ValueError("invalid physical bundle version") from error
    if not 0 < len(version) <= 128 or b"\0" in version:
        raise ValueError("invalid physical bundle version")
    return hashlib.sha256(
        b"iroha:kagemusha:v1:app-attest-release\0"
        + (3).to_bytes(4, "little")
        + len(version).to_bytes(2, "little") + version
    ).digest()


def _selection_identity(subject: bytes, release_id: bytes, profile_id: bytes,
                        lane_id: bytes, previous: int) -> bytes:
    """Check the exact physical MintFold shape and return its stable policy/epoch."""
    if (previous not in (0, 1)
            or len(subject) != 460 or subject[:49] != SELECTION_DOMAIN
            or int.from_bytes(subject[49:57], "little") != 403
            or int.from_bytes(subject[57:59], "little") != 1
            or subject[59:91] != release_id
            or subject[91:123] != b"\x21" * 32
            or subject[123:155] != b"\x22" * 32
            or subject[155:187] != b"\x23" * 32
            or subject[187:219] != b"\x24" * 32
            or subject[219:251] != lane_id
            or subject[251:283] != profile_id
            or subject[283:291] != (1).to_bytes(8, "little")
            or subject[291:323] != b"\x25" * 32
            or subject[323:331] != (1).to_bytes(8, "little")
            or subject[331] != 1
            or subject[332:364] != bytes([0x61 + previous]) * 32
            or subject[364:428] != bytes(64)
            or int.from_bytes(subject[428:444], "little") != previous
            or int.from_bytes(subject[444:460], "little") != previous + 1):
        raise ValueError("selection is not the exact-next V1 MintFold frame")
    return subject[59:331]


def verify(enrollment_path: Path, assertions_path: Path, root_path: Path,
           receipt_root_path: Path, app_id: str, environment: str, expected_bundle_version: str,
           openssl_path: Path) -> tuple[int, int, str]:
    """Verify Apple trust, receipt, challenge, app identity and exact 0→1→2 use."""
    root = root_path.read_bytes()
    if hashlib.sha256(root).digest() != APPLE_ROOT_SHA256:
        raise ValueError("Apple App Attestation root pin mismatch")
    receipt_root = receipt_root_path.read_bytes()
    if hashlib.sha256(receipt_root).digest() != APPLE_RECEIPT_ROOT_SHA256:
        raise ValueError("Apple fraud-receipt root pin mismatch")
    enrollment = _record(enrollment_path, ENROLLMENT_FIELDS)
    assertions = _record(assertions_path, ASSERTION_FIELDS)
    if enrollment["bundleVersion"] != expected_bundle_version:
        raise ValueError("physical bundle version differs from independently selected build")
    key_id = _bytes(enrollment["keyID"], 32)
    if len(key_id) != 32 or _bytes(enrollment["attestedKeyIDBase64"], 32) != key_id:
        raise ValueError("enrollment key ID mismatch")
    app_hash = hashlib.sha256(app_id.encode("utf-8")).digest()
    if (_bytes(enrollment["expectedAppIDHashBase64"], 32) != app_hash
            or _bytes(assertions["expectedAppIDHashBase64"], 32) != app_hash
            or assertions["keyID"] != enrollment["keyID"]):
        raise ValueError("physical evidence App ID or key differs")
    if _bytes(enrollment["expectedReleaseDigestBase64"], 32) != _release_digest(
            enrollment["bundleVersion"]):
        raise ValueError("physical evidence release digest differs from bundle version")
    selection = Selection(*(_bytes(enrollment[name], 32) for name in (
        "clientNonceBase64", "serverNonceBase64", "releaseIDBase64",
        "profileIDBase64", "attestedKeyIDBase64", "laneIDBase64",
    )))
    raw_attestation = _bytes(enrollment["rawAttestationBase64"], 64 * 1024)
    trusted_time_ms = time.time_ns() // 1_000_000
    proof = verify_apple_raw(
        raw_attestation, key_id,
        app_id, environment, selection, root, APPLE_ROOT_SHA256,
        trusted_time_ms, openssl_path,
        expected_validation_category=None, expected_bundle_version=None,
    )
    attestation = cbor_exact(raw_attestation)
    statement = attestation["attStmt"]
    receipt = verify_apple_receipt(
        statement["receipt"], app_id, statement["x5c"][0],
        proof.attested_public_key_sec1, receipt_root, APPLE_RECEIPT_ROOT_SHA256,
        trusted_time_ms, openssl_path, expected_type="ATTEST",
    )
    if _bytes(assertions["enrolledPublicKeyX963Base64"], 65) != proof.attested_public_key_sec1:
        raise ValueError("assertion key differs from attested enrollment")
    release_id = _bytes(enrollment["releaseIDBase64"], 32)
    profile_id = _bytes(enrollment["profileIDBase64"], 32)
    lane_id = _bytes(enrollment["laneIDBase64"], 32)
    if (release_id != PHYSICAL_RELEASE_ID or profile_id != PHYSICAL_PROFILE_ID
            or lane_id != PHYSICAL_LANE_ID
            or _bytes(enrollment["clientNonceBase64"], 32) != b"\x11" * 32
            or _bytes(enrollment["serverNonceBase64"], 32) != b"\x22" * 32):
        raise ValueError("physical evidence differs from independent XCTest vector")
    counters = []
    first_identity = None
    for label, previous in (("first", 0), ("second", 1)):
        subject = _bytes(assertions[f"{label}SelectionBase64"], 460)
        identity = _selection_identity(subject, release_id, profile_id, lane_id, previous)
        if first_identity is not None and identity != first_identity:
            raise ValueError("physical selections changed governed identity or hardware epoch")
        first_identity = identity
        observed = verify_apple_assertion(
            _bytes(assertions[f"{label}AssertionBase64"], 4096),
            subject, subject, proof.attested_public_key_sec1, key_id, app_id,
            previous, openssl_path,
            expected_validation_category=None, expected_bundle_version=None,
        ).counter
        if observed != previous + 1:
            raise ValueError("physical assertion skipped an exact-next counter")
        counters.append(observed)
    return counters[0], counters[1], receipt.receipt_sha256.hex()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--enrollment-json", type=Path, required=True)
    parser.add_argument("--assertions-json", type=Path, required=True)
    parser.add_argument("--root-der", type=Path, required=True)
    parser.add_argument("--receipt-root-der", type=Path, required=True)
    parser.add_argument("--app-id", required=True)
    parser.add_argument("--environment", choices=("development", "production"), required=True)
    parser.add_argument("--expected-bundle-version", required=True)
    args = parser.parse_args()
    executable = shutil.which("openssl")
    if executable is None:
        raise SystemExit("OpenSSL is unavailable")
    first, second, receipt_sha256 = verify(
        args.enrollment_json, args.assertions_json, args.root_der,
        args.receipt_root_der,
        args.app_id, args.environment, args.expected_bundle_version,
        Path(executable).resolve(),
    )
    print(f"Apple physical App Attest chain/receipt/challenge and exact counters {first}→{second} verified; receipt SHA-256 {receipt_sha256}")


if __name__ == "__main__":
    main()
