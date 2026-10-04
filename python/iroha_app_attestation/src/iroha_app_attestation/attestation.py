"""Independent, bounded raw Apple App Attest and Android KeyMint evidence checks.

This module does not issue KAGEMUSHA certificates. A raw attestation proves neither
the exact distributed app binary nor a current Android revocation state. The
certificate signer must stay disconnected until those inputs and issuer-signed
preparation verification are implemented and tested.
"""

from __future__ import annotations

import base64
import datetime
import hashlib
import os
import sqlite3
import stat
import subprocess
import tempfile
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
from typing import Any


MAX_OBJECT = 64 * 1024
MAX_APPLE_ASSERTION = 4 * 1024
MAX_APPLE_CLIENT_DATA = 4 * 1024
MAX_CERT = 16 * 1024
MAX_CHAIN = 8
MAX_ANDROID_CHAIN_ENVELOPE = 128 * 1024
ANDROID_CHAIN_MAGIC = b"KMCA\x01"
TRANSCRIPT_DOMAIN = b"iroha:kagemusha:v1:app-device-attestation-challenge\0"
DEVICE_KEY_REFERENCE_DOMAIN = b"iroha:kagemusha:v1:device-key-reference\0"
PREPARATION_DOMAIN = b"iroha:kagemusha:v1:app-enrollment-preparation\0"
PREPARATION_BYTES = 1 + 8 + 8 + 6 * 32 + 64
PREPARATION_TTL_MS = 120_000
APPLE_NONCE_OID = "1.2.840.113635.100.8.2"
ANDROID_KEY_DESCRIPTION_OID = "1.3.6.1.4.1.11129.2.1.17"
KEYMINT_VERSIONS = {100, 200, 300, 400, 500}
# Ordinary persistent app identity has no finite-use KeyMint requirement. Version1
# is absent because its schema has no attestationApplicationId/package signer pin.
# https://source.android.com/docs/security/features/keystore/attestation
ORDINARY_PERSISTENT_ANDROID_VERSION_PAIRS = frozenset({(2, 3), (3, 4), (4, 41)} |
    {(version, version) for version in KEYMINT_VERSIONS})
# Google's explicitly listed 2016 factory root remains trusted after expiry
# when the exact chain is checked and the current revocation list is clean.
# https://developer.android.com/privacy-and-security/security-key-attestation
GOOGLE_FACTORY_2016_ROOT_SHA256 = bytes.fromhex(
    "c1984a3ef45c1e2a918551de10603c86f7051b2249c4891cae3230eabd0c97d5"
)
GOOGLE_FACTORY_2016_VERIFICATION_TIME_MS = 1_779_494_400_000  # 2026-05-23 UTC
# KeyMint 100-500 AuthorizationList fields documented by AOSP. Non-attestation
# tags and key types inconsistent with the required EC/P-256 signing key stay
# rejected. Optional, documented OS/vendor/device fields are parsed rather
# than making real attestations fail merely because a brand adds those fields.
KEYMINT_SET_TAGS = {1, 5}
KEYMINT_INTEGER_TAGS = {2, 3, 10, 400, 401, 402, 405, 504, 505, 701, 702, 705, 706, 718, 719}
KEYMINT_NULL_TAGS = {303, 305, 503, 506, 507, 508, 509, 720}
KEYMINT_OCTET_TAGS = {709, 710, 711, 712, 713, 714, 715, 716, 717, 723, 724}
KEYMINT_AUTH_TAGS = (KEYMINT_SET_TAGS | KEYMINT_INTEGER_TAGS
                     | KEYMINT_NULL_TAGS | KEYMINT_OCTET_TAGS | {704})
HARDWARE_ONLY_TAGS = {1, 2, 3, 5, 10, 303, 405, 702, 704}
ANDROID_APPROVAL_REQUIRED_HARDWARE_TAGS = {1, 2, 3, 5, 10, 702, 704}
# KeyMint AuthorizationList OS version and patch-level tags. Only their
# hardware-enforced copies describe the attested device.
ANDROID_OS_VERSION_TAG = 705
ANDROID_OS_PATCH_LEVEL_TAG = 706
ANDROID_VENDOR_PATCH_LEVEL_TAG = 718
ANDROID_BOOT_PATCH_LEVEL_TAG = 719


class AttestationRejected(ValueError):
    """Raw evidence does not match the independently selected scope."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AttestationRejected(message)


def fixed32(value: bytes, name: str) -> bytes:
    require(len(value) == 32 and any(value), f"invalid {name}")
    return value


def app_attest_release_digest(category: int, bundle_version: str) -> bytes:
    """Match the model's exact signed App Attest release-digest preimage."""
    require(type(category) is int and category in (2, 3, 4, 5)
            and type(bundle_version) is str,
            "invalid Apple signed distribution values")
    try:
        version = bundle_version.encode("utf-8")
    except UnicodeEncodeError as error:
        raise AttestationRejected("invalid Apple signed bundle version") from error
    require(0 < len(version) <= 128 and b"\0" not in version,
            "invalid Apple signed bundle version")
    return hashlib.sha256(
        b"iroha:kagemusha:v1:app-attest-release\0"
        + category.to_bytes(4, "little")
        + len(version).to_bytes(2, "little") + version
    ).digest()


def device_key_reference(public_key_sec1: bytes) -> bytes:
    """Derive the canonical Iroha device-key reference from its P-256 point."""
    require(len(public_key_sec1) == 65 and public_key_sec1[0] == 4,
            "invalid device public key")
    return hashlib.sha256(DEVICE_KEY_REFERENCE_DOMAIN + public_key_sec1).digest()


@dataclass(frozen=True)
class Selection:
    """Values selected from authenticated release and issuer preparation, never evidence."""

    client_nonce: bytes
    server_nonce: bytes
    release_id: bytes
    hardware_profile_id: bytes
    attested_key_id: bytes
    lane_id: bytes

    def transcript(self) -> bytes:
        parts = (
            fixed32(self.client_nonce, "client nonce"),
            fixed32(self.server_nonce, "server nonce"),
            fixed32(self.release_id, "release"),
            fixed32(self.hardware_profile_id, "profile"),
            self.attested_key_id,
            fixed32(self.lane_id, "lane"),
        )
        require(len(parts[4]) == 32, "invalid attested key ID")
        require(parts[0] != parts[1], "repeated challenge nonce")
        return TRANSCRIPT_DOMAIN + b"".join(parts)


@dataclass(frozen=True)
class AndroidPatchLevels:
    """Hardware-enforced OS facts of the KeyDescription the verifier selected.

    Values are the exact signed integers (``None`` when the hardware list omits
    the tag). They describe the device when the key was generated, not a live
    examination. Software-enforced copies are never read.
    """

    attestation_version: int
    os_version: int | None
    os_patch_level: int | None
    vendor_patch_level: int | None
    boot_patch_level: int | None


@dataclass(frozen=True)
class RawPlatformProof:
    """Only checked platform key/app identity; deliberately not an issuance token."""

    evidence_sha256: bytes
    attested_public_key_sec1: bytes
    device_key_reference: bytes
    platform: str
    apple_validation_category: int | None = None
    apple_bundle_version: str | None = None
    android_security_level: int | None = None
    android_patch_levels: AndroidPatchLevels | None = None


def patch_level_yyyymm(value: int) -> int | None:
    """Normalize a KeyMint patch level (YYYYMM or YYYYMMDD) to YYYYMM.

    Returns ``None`` for any other value. Zero means "not reported" and is
    handled by the caller.
    """
    if type(value) is not int:
        return None
    if 19000101 <= value <= 99991231:
        value //= 100
    if 190001 <= value <= 999912 and 1 <= value % 100 <= 12:
        return value
    return None


def require_android_patch_floor(levels: AndroidPatchLevels, floor_yyyymm: int) -> None:
    """Apply an authenticated enrollment patch floor to verified patch levels.

    The hardware-enforced OS patch level must be present. Every reported OS,
    vendor or boot patch level must parse and be at least ``floor_yyyymm``;
    a vendor or boot level of zero or an absent tag means "not reported".
    ``levels`` must come from a ``RawPlatformProof`` whose chain verified.

    TODO: carry the floor in the Native-selected Android policy (the Rust
    hardware-profile owner and ``native_policy_projection.py``) and call this
    from ``ordinary_provider.GovernedOrdinaryEvidenceProvider.prepare_raw``;
    spec §2.2 requires an enrollment patch policy.
    """
    require(type(levels) is AndroidPatchLevels, "verified Android patch levels absent")
    require(type(floor_yyyymm) is int and patch_level_yyyymm(floor_yyyymm) == floor_yyyymm,
            "invalid Android patch floor")
    require(type(levels.os_patch_level) is int and levels.os_patch_level != 0,
            "hardware-enforced Android OS patch level absent")
    for name, level in (("OS", levels.os_patch_level), ("vendor", levels.vendor_patch_level),
                        ("boot", levels.boot_patch_level)):
        if level is None or level == 0:
            continue
        normalized = patch_level_yyyymm(level)
        require(normalized is not None, f"unparsable Android {name} patch level")
        require(normalized >= floor_yyyymm, f"Android {name} patch level is below the enrollment floor")


@dataclass(frozen=True)
class IssuerPreparation:
    """An issuer-signed nonce pair; release credential fields remain separately selected."""

    client_nonce: bytes
    server_nonce: bytes
    issued_at_ms: int
    expires_at_ms: int


@dataclass(frozen=True)
class AppleAssertion:
    """Verified Apple assertion metadata, not a certificate-issuance token.

    The caller must atomically store ``counter`` against its previously attested
    key and consume the independently issued challenge. This value alone does
    not establish a distributed app release or offline monetary state.
    """

    counter: int
    assertion_sha256: bytes
    client_data_sha256: bytes
    validation_category: int | None
    bundle_version: str | None


class CborReader:
    def __init__(self, data: bytes) -> None:
        require(len(data) <= MAX_OBJECT, "CBOR object too large")
        self.data = data
        self.offset = 0

    def read(self, depth: int = 0) -> Any:
        require(depth <= 8 and self.offset < len(self.data), "invalid CBOR nesting")
        initial = self.data[self.offset]
        self.offset += 1
        major, argument = initial >> 5, initial & 31
        require(argument != 31, "indefinite CBOR is unsupported")
        length_bytes = (0, 1, 2, 4, 8)
        if argument < 24:
            length = argument
        else:
            require(argument <= 27, "invalid CBOR length")
            count = length_bytes[argument - 23]
            require(self.offset + count <= len(self.data), "truncated CBOR length")
            length = int.from_bytes(self.data[self.offset : self.offset + count], "big")
            self.offset += count
            require(length >= (24 if count == 1 else 1 << ((count // 2) * 8)), "nonminimal CBOR length")
        if major in (0, 1):
            return length if major == 0 else -1 - length
        if major in (2, 3):
            require(self.offset + length <= len(self.data), "truncated CBOR value")
            raw = self.data[self.offset : self.offset + length]
            self.offset += length
            if major == 2:
                return raw
            try:
                return raw.decode("utf-8")
            except UnicodeDecodeError as error:
                raise AttestationRejected("invalid CBOR text") from error
        if major in (4, 5):
            require(length <= 64, "CBOR container too large")
            if major == 4:
                return [self.read(depth + 1) for _ in range(length)]
            result: dict[Any, Any] = {}
            for _ in range(length):
                key = self.read(depth + 1)
                require(isinstance(key, (str, int, bytes)) and key not in result, "duplicate or invalid CBOR map key")
                result[key] = self.read(depth + 1)
            return result
        raise AttestationRejected("unsupported CBOR type")


def cbor_exact(data: bytes) -> Any:
    reader = CborReader(data)
    value = reader.read()
    require(reader.offset == len(data), "trailing CBOR bytes")
    return value


@dataclass(frozen=True)
class DerValue:
    tag_class: int
    constructed: bool
    number: int
    value: bytes


def der_one(data: bytes) -> DerValue:
    value, end = der_at(data, 0)
    require(end == len(data), "trailing DER bytes")
    return value


def der_at(data: bytes, offset: int) -> tuple[DerValue, int]:
    require(offset < len(data), "truncated DER tag")
    first = data[offset]
    offset += 1
    tag_class, constructed, number = first >> 6, bool(first & 32), first & 31
    if number == 31:
        number = 0
        for index in range(4):
            require(offset < len(data), "truncated DER tag number")
            part = data[offset]
            offset += 1
            require(index != 0 or part != 0x80, "nonminimal DER tag number")
            number = (number << 7) | (part & 127)
            if part < 128:
                break
        else:
            raise AttestationRejected("DER tag number too large")
        require(number >= 31, "nonminimal DER tag number")
    require(offset < len(data), "truncated DER length")
    first_length = data[offset]
    offset += 1
    if first_length < 128:
        length = first_length
    else:
        count = first_length & 127
        require(1 <= count <= 4 and offset + count <= len(data), "invalid DER length")
        require(data[offset] != 0, "nonminimal DER length")
        length = int.from_bytes(data[offset : offset + count], "big")
        offset += count
        require(length >= 128, "nonminimal DER length")
    require(offset + length <= len(data), "truncated DER value")
    return DerValue(tag_class, constructed, number, data[offset : offset + length]), offset + length


def children(value: DerValue, number: int = 16, tag_class: int = 0) -> list[DerValue]:
    require(value.tag_class == tag_class and value.number == number and value.constructed, "unexpected DER container")
    result = []
    offset = 0
    while offset < len(value.value):
        child, offset = der_at(value.value, offset)
        result.append(child)
        require(len(result) <= 64, "too many DER children")
    return result


def primitive(value: DerValue, number: int) -> bytes:
    require(value.tag_class == 0 and value.number == number and not value.constructed, "unexpected DER primitive")
    return value.value


def positive_integer(value: DerValue, number: int = 2) -> int:
    raw = primitive(value, number)
    require(bool(raw) and raw[0] < 128 and (len(raw) == 1 or raw[0] != 0 or raw[1] >= 128), "invalid DER integer")
    return int.from_bytes(raw, "big")


def oid(value: DerValue) -> str:
    raw = primitive(value, 6)
    require(bool(raw), "empty DER OID")
    numbers = []
    current = 0
    component_start = True
    for byte in raw:
        require(not (component_start and byte == 0x80), "nonminimal DER OID component")
        current = (current << 7) | (byte & 127)
        require(current < 1 << 40, "DER OID component too large")
        if byte < 128:
            numbers.append(current)
            current = 0
            component_start = True
        else:
            component_start = False
    require(raw[-1] < 128 and bool(numbers), "truncated DER OID")
    first = numbers.pop(0)
    head = (0, first) if first < 40 else (1, first - 40) if first < 80 else (2, first - 80)
    return ".".join(str(part) for part in (*head, *numbers))


def certificate_spki_extensions(der: bytes) -> tuple[DerValue, dict[str, bytes]]:
    """Parse a bounded X.509 certificate without assuming its key algorithm."""
    require(0 < len(der) <= MAX_CERT, "certificate outside bound")
    cert = children(der_one(der))
    require(len(cert) == 3, "invalid X.509 certificate")
    tbs = children(cert[0])
    require(bool(tbs), "truncated X.509 TBS")
    start = 1 if tbs[0].tag_class == 2 and tbs[0].number == 0 else 0
    version = 0
    if start:
        encoded_version = children(tbs[0], number=0, tag_class=2)
        require(len(encoded_version) == 1, "invalid X.509 version")
        version = positive_integer(encoded_version[0])
        require(version in (1, 2), "invalid X.509 version")
    require(len(tbs) >= start + 6, "truncated X.509 TBS")
    spki = tbs[start + 5]
    extensions: dict[str, bytes] = {}
    prior_optional = 0
    for field in tbs[start + 6 :]:
        # RFC 5280 permits issuerUniqueID [1], subjectUniqueID [2], then one
        # extensions [3] field. Accepting a second [3] lets a different X.509
        # parser and this verifier disagree on which attestation claim was signed.
        require(field.tag_class == 2 and field.number in (1, 2, 3)
                and field.number > prior_optional, "duplicate or misplaced X.509 optional field")
        prior_optional = field.number
        if field.number in (1, 2):
            require(not field.constructed and bool(field.value)
                    and field.value[0] <= 7
                    and (len(field.value) > 1 or field.value[0] == 0)
                    and (field.value[0] == 0 or field.value[-1] & ((1 << field.value[0]) - 1) == 0),
                    "invalid X.509 unique identifier")
            continue
        require(version == 2, "X.509 extensions require v3")
        enclosing = children(field, number=3, tag_class=2)
        require(len(enclosing) == 1, "invalid X.509 extensions")
        listed = children(enclosing[0])
        require(bool(listed), "empty X.509 extensions")
        for extension in listed:
            fields = children(extension)
            require(len(fields) in (2, 3), "invalid X.509 extension")
            name = oid(fields[0])
            if len(fields) == 3:
                require(primitive(fields[1], 1) == b"\xff",
                        "invalid X.509 extension critical flag")
            require(name not in extensions, "duplicate X.509 extension")
            extensions[name] = primitive(fields[-1], 4)
    return spki, extensions


def certificate_key_extensions(der: bytes) -> tuple[bytes, dict[str, bytes]]:
    """Return the P-256 subject point and extensions of one certificate."""
    spki_value, extensions = certificate_spki_extensions(der)
    spki = children(spki_value)
    require(len(spki) == 2, "invalid subject public key")
    algorithm = children(spki[0])
    require(len(algorithm) == 2 and oid(algorithm[0]) == "1.2.840.10045.2.1"
            and oid(algorithm[1]) == "1.2.840.10045.3.1.7", "not P-256 EC public key")
    bit_string = primitive(spki[1], 3)
    require(len(bit_string) == 66 and bit_string[0] == 0 and bit_string[1] == 4, "invalid P-256 point")
    return bit_string[1:], extensions


def pem(der: bytes) -> bytes:
    encoded = base64.b64encode(der)
    lines = b"\n".join(encoded[index : index + 64] for index in range(0, len(encoded), 64))
    return b"-----BEGIN CERTIFICATE-----\n" + lines + b"\n-----END CERTIFICATE-----\n"


def public_key_pem(spki_der: bytes) -> bytes:
    encoded = base64.b64encode(spki_der)
    lines = b"\n".join(encoded[index : index + 64] for index in range(0, len(encoded), 64))
    return b"-----BEGIN PUBLIC KEY-----\n" + lines + b"\n-----END PUBLIC KEY-----\n"


def verify_issuer_preparation(
    token: bytes, selection: Selection, account_canonical: str,
    issuer_policy_id: bytes,
    issuer_public_spki_der: bytes, issuer_public_spki_sha256: bytes,
    trusted_time_ms: int, openssl_path: Path,
    *, require_fresh: bool = True,
) -> IssuerPreparation:
    """Verify the 273-byte preparation using only pre-attestation values."""
    require(len(token) == PREPARATION_BYTES and token[0] == 1, "invalid preparation frame")
    selection.transcript()
    fixed32(issuer_policy_id, "issuer policy")
    require(account_canonical and len(account_canonical.encode("utf-8")) <= 512,
            "invalid prepared account")
    require(len(issuer_public_spki_der) <= 512 and hashlib.sha256(issuer_public_spki_der).digest()
            == fixed32(issuer_public_spki_sha256, "issuer key pin"), "untrusted issuer key")
    spki = children(der_one(issuer_public_spki_der))
    require(len(spki) == 2, "invalid issuer public key")
    algorithm = children(spki[0])
    require(len(algorithm) == 1 and oid(algorithm[0]) == "1.3.101.112"
            and len(primitive(spki[1], 3)) == 33 and primitive(spki[1], 3)[0] == 0,
            "issuer key is not Ed25519")
    issued = int.from_bytes(token[1:9], "little")
    expires = int.from_bytes(token[9:17], "little")
    client_nonce, server_nonce = token[17:49], token[49:81]
    require(issued > 0 and expires == issued + PREPARATION_TTL_MS
            and issued <= trusted_time_ms
            and (not require_fresh or trusted_time_ms < expires)
            and client_nonce == selection.client_nonce
            and server_nonce == selection.server_nonce
            and token[81:209] == (selection.release_id + selection.hardware_profile_id
                                  + selection.attested_key_id + selection.lane_id),
            "expired or substituted preparation")
    message = (PREPARATION_DOMAIN + token[1:209] + issuer_policy_id
               + hashlib.sha256(account_canonical.encode("utf-8")).digest())
    require(openssl_path.is_absolute() and openssl_path.is_file(), "invalid verification environment")
    with tempfile.TemporaryDirectory(prefix="kagemusha-preparation-") as temporary:
        directory = Path(temporary)
        (directory / "issuer.pem").write_bytes(public_key_pem(issuer_public_spki_der))
        (directory / "message.bin").write_bytes(message)
        (directory / "signature.bin").write_bytes(token[209:])
        result = subprocess.run(
            [str(openssl_path), "pkeyutl", "-verify", "-pubin", "-inkey", str(directory / "issuer.pem"),
             "-rawin", "-in", str(directory / "message.bin"), "-sigfile", str(directory / "signature.bin")],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False,
        )
        require(result.returncode == 0, "issuer preparation signature rejected")
    return IssuerPreparation(client_nonce, server_nonce, issued, expires)


def _certificate_valid_at(der: bytes, trusted_time_ms: int) -> bool:
    """Check the two canonical RFC 5280 UTC validity fields at a trusted time."""
    tbs = children(der_one(der))[0]
    fields = children(tbs)
    start = 1 if fields[0].tag_class == 2 and fields[0].number == 0 else 0
    require(len(fields) >= start + 6, "truncated X.509 validity")
    interval = children(fields[start + 3])
    require(len(interval) == 2, "invalid X.509 validity interval")

    def instant(value: DerValue) -> int:
        require(value.tag_class == 0 and not value.constructed
                and value.number in (23, 24), "invalid X.509 time type")
        raw = value.value
        digits = raw[:-1]
        expected = 12 if value.number == 23 else 14
        require(len(raw) == expected + 1 and raw[-1:] == b"Z"
                and all(48 <= byte <= 57 for byte in digits), "invalid X.509 time")
        if value.number == 23:
            year = int(digits[:2])
            year += 1900 if year >= 50 else 2000
            offset = 2
        else:
            year = int(digits[:4])
            offset = 4
        try:
            date = datetime.datetime(
                year, *(int(digits[index:index + 2])
                        for index in range(offset, expected, 2)),
                tzinfo=datetime.timezone.utc,
            )
        except ValueError as error:
            raise AttestationRejected("invalid X.509 calendar time") from error
        return int(date.timestamp() * 1000)

    before, after = (instant(value) for value in interval)
    require(before < after, "invalid X.509 validity order")
    return before <= trusted_time_ms <= after


def verify_pinned_chain(
    chain_der: list[bytes], root_der: bytes, root_sha256: bytes,
    trusted_time_ms: int, openssl_path: Path,
    *, allow_google_factory_expired_root: bool = False,
) -> None:
    require(2 <= len(chain_der) <= MAX_CHAIN and all(0 < len(cert) <= MAX_CERT for cert in chain_der), "certificate chain outside bound")
    require(0 < len(root_der) <= MAX_CERT and hashlib.sha256(root_der).digest() == fixed32(root_sha256, "root pin"), "untrusted root")
    require(trusted_time_ms > 0 and openssl_path.is_absolute() and openssl_path.is_file(), "invalid verification environment")
    # X.509 path builders treat -untrusted as a pool, not an ordered chain. Do
    # not interpret an extension from a supplied certificate that OpenSSL could
    # have ignored. Require every supplied certificate to be one exact
    # leaf-to-root path, allowing the pinned root to be supplied or implicit.
    require(len(set(chain_der)) == len(chain_der)
            and root_der not in chain_der[:-1], "duplicate or misplaced certificate")
    if allow_google_factory_expired_root:
        require(root_sha256 == GOOGLE_FACTORY_2016_ROOT_SHA256,
                "factory expiry exception requires Google's exact 2016 root")
        require(_certificate_valid_at(root_der,
                                      min(trusted_time_ms, GOOGLE_FACTORY_2016_VERIFICATION_TIME_MS)),
                "factory root was not valid before expiry")
        require(all(_certificate_valid_at(cert, trusted_time_ms)
                    for cert in chain_der if cert != root_der),
                "expired or not-yet-valid factory-chain certificate")
    path_der = chain_der if chain_der[-1] == root_der else [*chain_der, root_der]
    with tempfile.TemporaryDirectory(prefix="kagemusha-attestation-") as temporary:
        directory = Path(temporary)
        root = directory / "root.pem"
        leaf = directory / "leaf.pem"
        intermediates = directory / "intermediates.pem"
        root.write_bytes(pem(root_der))
        leaf.write_bytes(pem(chain_der[0]))
        intermediates.write_bytes(b"".join(pem(cert) for cert in chain_der[1:]))
        time_option = (["-no_check_time"] if allow_google_factory_expired_root
                       else ["-attime", str(trusted_time_ms // 1000)])
        result = subprocess.run(
            [str(openssl_path), "verify", "-no-CAfile", "-no-CApath", "-no-CAstore",
             "-trusted", str(root), "-untrusted", str(intermediates),
             *time_option, str(leaf)],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False,
        )
        require(result.returncode == 0, "certificate chain rejected")
        for index, (child_der, issuer_der) in enumerate(zip(path_der, path_der[1:])):
            child_path = directory / f"path-{index}-child.pem"
            issuer_path = directory / f"path-{index}-issuer.pem"
            child_path.write_bytes(pem(child_der))
            issuer_path.write_bytes(pem(issuer_der))
            linked = subprocess.run(
                [str(openssl_path), "verify", "-no-CAfile", "-no-CApath", "-no-CAstore",
                 "-partial_chain", "-trusted", str(issuer_path),
                 *time_option, str(child_path)],
                stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False,
            )
            require(linked.returncode == 0, "certificate chain order or link rejected")


def encode_android_chain(chain_der: list[bytes]) -> bytes:
    """Encode one exact, bounded Android raw-evidence envelope for audit/retry.

    Layout: ``KMCA`` + version 1 + certificate count u8 + repeated
    ``DER length u32 big-endian | DER bytes``. The list is leaf first. The
    envelope, rather than an app-provided digest, is retained and hashed.
    """
    require(2 <= len(chain_der) <= MAX_CHAIN, "Android certificate count outside bound")
    frame = bytearray(ANDROID_CHAIN_MAGIC + bytes([len(chain_der)]))
    for certificate in chain_der:
        require(0 < len(certificate) <= MAX_CERT,
                "Android certificate outside DER bound")
        children(der_one(certificate))
        frame.extend(len(certificate).to_bytes(4, "big"))
        frame.extend(certificate)
        require(len(frame) <= MAX_ANDROID_CHAIN_ENVELOPE,
                "Android certificate envelope outside bound")
    return bytes(frame)


def decode_android_chain(envelope: bytes) -> list[bytes]:
    """Reject alternative/trailing frame layouts and return leaf-first DER."""
    require(0 < len(envelope) <= MAX_ANDROID_CHAIN_ENVELOPE
            and envelope.startswith(ANDROID_CHAIN_MAGIC) and len(envelope) >= 6,
            "invalid Android certificate envelope")
    count = envelope[5]
    require(2 <= count <= MAX_CHAIN, "Android certificate count outside bound")
    offset = 6
    chain = []
    for _ in range(count):
        require(offset + 4 <= len(envelope), "truncated Android certificate length")
        length = int.from_bytes(envelope[offset:offset + 4], "big")
        offset += 4
        require(0 < length <= MAX_CERT and offset + length <= len(envelope),
                "Android certificate outside bound")
        chain.append(envelope[offset:offset + length])
        offset += length
    require(offset == len(envelope) and encode_android_chain(chain) == envelope,
            "noncanonical Android certificate envelope")
    return chain


def verify_apple_raw(
    attestation_object: bytes, key_id: bytes, app_id: str, environment: str,
    selection: Selection,
    root_der: bytes, root_sha256: bytes, trusted_time_ms: int, openssl_path: Path,
    *, expected_validation_category: int | None, expected_bundle_version: str | None,
) -> RawPlatformProof:
    """Check Apple's documented App Attest raw object under a pinned root.

    The caller must verify Apple's receipt and independently select governed
    app distribution policy. This raw result does not measure an exact build.
    """
    require(len(attestation_object) <= MAX_OBJECT and len(key_id) == 32, "invalid App Attest object or key ID")
    require(app_id and len(app_id.encode("utf-8")) <= 255, "invalid pinned App ID")
    require(environment in ("production", "development"), "invalid pinned App Attest environment")
    require((expected_validation_category is None) == (expected_bundle_version is None),
            "incomplete Apple distribution policy")
    if expected_validation_category is not None:
        require(expected_validation_category in (2, 3, 4, 5)
                and isinstance(expected_bundle_version, str) and 0 < len(expected_bundle_version.encode("utf-8")) <= 128,
                "invalid Apple distribution policy")
    decoded = cbor_exact(attestation_object)
    require(isinstance(decoded, dict) and set(decoded) == {"fmt", "attStmt", "authData"}, "invalid App Attest frame")
    require(decoded["fmt"] == "apple-appattest" and isinstance(decoded["authData"], bytes), "wrong App Attest format")
    statement = decoded["attStmt"]
    require(isinstance(statement, dict) and set(statement) == {"x5c", "receipt"}, "invalid App Attest statement")
    chain = statement["x5c"]
    require(isinstance(chain, list) and all(isinstance(cert, bytes) for cert in chain)
            and isinstance(statement["receipt"], bytes) and bool(statement["receipt"]), "invalid App Attest evidence")
    verify_pinned_chain(chain, root_der, root_sha256, trusted_time_ms, openssl_path)
    point, extensions = certificate_key_extensions(chain[0])
    auth = decoded["authData"]
    require(len(auth) >= 55 + 32, "truncated App Attest authenticator")
    require(auth[:32] == hashlib.sha256(app_id.encode("utf-8")).digest(), "App ID mismatch")
    require(auth[32] & 0x40 != 0 and auth[33:37] == b"\0" * 4, "invalid App Attest flags or counter")
    aaguid = b"appattest" + b"\0" * 7 if environment == "production" else b"appattestdevelop"
    require(auth[37:53] == aaguid, "App Attest environment mismatch")
    credential_length = int.from_bytes(auth[53:55], "big")
    require(credential_length == 32 and auth[55:87] == key_id, "App Attest credential ID mismatch")
    reader = CborReader(auth[87:])
    cose = reader.read()
    validation_category: int | None = None
    bundle_version: str | None = None
    if reader.offset < len(reader.data):
        # Apple's attested extension map can be present even when the ED flag is
        # clear. The entire authData, including this map, is bound by the nonce.
        authenticator_extensions = reader.read()
        require(isinstance(authenticator_extensions, dict)
                and set(authenticator_extensions) == {"apple_validation_category_01", "apple_bundle_version_01"},
                "unsupported App Attest authenticator extension")
        raw_category = authenticator_extensions["apple_validation_category_01"]
        bundle_version = authenticator_extensions["apple_bundle_version_01"]
        require(isinstance(raw_category, bytes) and len(raw_category) == 4
                and isinstance(bundle_version, str) and 0 < len(bundle_version.encode("utf-8")) <= 128,
                "invalid Apple distribution extension")
        validation_category = int.from_bytes(raw_category, "little")
        require(validation_category in (2, 3, 4, 5), "unapproved Apple app distribution category")
    else:
        require(auth[32] & 0x80 == 0, "missing App Attest authenticator extension")
    if expected_validation_category is not None:
        require(validation_category == expected_validation_category
                and bundle_version == expected_bundle_version, "Apple distribution policy mismatch")
    require(reader.offset == len(reader.data), "trailing App Attest authenticator bytes")
    require(isinstance(cose, dict) and cose.get(1) == 2 and cose.get(3) == -7 and cose.get(-1) == 1
            and isinstance(cose.get(-2), bytes) and isinstance(cose.get(-3), bytes), "invalid App Attest COSE key")
    require(point == b"\x04" + cose[-2] + cose[-3] and hashlib.sha256(point).digest() == key_id,
            "App Attest key mismatch")
    require(selection.attested_key_id == key_id,
            "App Attest prepared key ID mismatch")
    extension = extensions.get(APPLE_NONCE_OID)
    require(extension is not None, "missing App Attest nonce extension")
    nonce_sequence = children(der_one(extension))
    require(len(nonce_sequence) == 1, "invalid App Attest nonce extension")
    tagged_nonce = children(nonce_sequence[0], number=1, tag_class=2)
    require(len(tagged_nonce) == 1, "invalid App Attest tagged nonce")
    expected_nonce = hashlib.sha256(auth + hashlib.sha256(selection.transcript()).digest()).digest()
    require(primitive(tagged_nonce[0], 4) == expected_nonce, "App Attest challenge mismatch")
    return RawPlatformProof(hashlib.sha256(attestation_object).digest(), point,
                            device_key_reference(point), "apple_app_attest",
                            validation_category, bundle_version)


def require_apple_counter_advance(previous: int, observed: int) -> int:
    """Accept only a strictly increasing App Attest assertion counter.

    Apple permits gaps between assertions. This check must be paired with an
    atomic, durable compare-and-update keyed by the attested public key when
    assertions are accepted concurrently or across server processes.
    """
    require(type(previous) is int and type(observed) is int
            and 0 <= previous < (1 << 32) and 0 < observed < (1 << 32)
            and observed > previous,
            "App Attest assertion counter did not advance")
    return observed


def verify_apple_assertion(
    assertion_object: bytes, client_data: bytes, expected_client_data: bytes,
    attested_public_key_sec1: bytes, attested_key_id: bytes, app_id: str,
    previous_counter: int, openssl_path: Path,
    *, expected_validation_category: int | None, expected_bundle_version: str | None,
) -> AppleAssertion:
    """Verify a bounded App Attest assertion against a previously attested key.

    ``expected_client_data`` must come from an independently selected,
    single-use server challenge and operation scope. The caller must consume
    that challenge and atomically persist the returned counter; this helper
    neither owns server state nor authenticates Apple receipts or app release.
    """
    require(type(assertion_object) is bytes and 0 < len(assertion_object) <= MAX_APPLE_ASSERTION,
            "App Attest assertion outside bound")
    require(type(client_data) is bytes and type(expected_client_data) is bytes
            and 0 < len(client_data) <= MAX_APPLE_CLIENT_DATA
            and client_data == expected_client_data,
            "App Attest client data differs from server challenge")
    require(type(attested_public_key_sec1) is bytes and len(attested_public_key_sec1) == 65
            and attested_public_key_sec1[0] == 4 and type(attested_key_id) is bytes
            and hashlib.sha256(attested_public_key_sec1).digest() == attested_key_id,
            "App Attest assertion key differs from attested key ID")
    require(type(app_id) is str and 0 < len(app_id.encode("utf-8")) <= 255,
            "invalid pinned App ID")
    require((expected_validation_category is None) == (expected_bundle_version is None),
            "incomplete Apple distribution policy")
    if expected_validation_category is not None:
        require(type(expected_validation_category) is int
                and expected_validation_category in (2, 3, 4, 5)
                and type(expected_bundle_version) is str
                and 0 < len(expected_bundle_version.encode("utf-8")) <= 128,
                "invalid Apple distribution policy")
    assertion = cbor_exact(assertion_object)
    require(isinstance(assertion, dict)
            and set(assertion) == {"signature", "authenticatorData"},
            "invalid App Attest assertion frame")
    signature = assertion["signature"]
    auth = assertion["authenticatorData"]
    require(type(signature) is bytes and 8 <= len(signature) <= 72
            and type(auth) is bytes and 37 <= len(auth) <= MAX_APPLE_ASSERTION,
            "invalid App Attest assertion fields")
    signature_fields = children(der_one(signature))
    require(len(signature_fields) == 2
            and all(0 < positive_integer(field) < (1 << 256) for field in signature_fields),
            "invalid App Attest assertion signature")
    # Apple's documented assertion checks bind RP ID and counter, not a fixed
    # flag value. A physical iPhone assertion used flag 0x40 with exactly 37
    # authenticator bytes; the complete signed body remains authenticated.
    require(auth[:32] == hashlib.sha256(app_id.encode("utf-8")).digest(),
            "App Attest assertion App ID mismatch")
    counter = require_apple_counter_advance(previous_counter,
                                            int.from_bytes(auth[33:37], "big"))
    validation_category: int | None = None
    bundle_version: str | None = None
    if len(auth) > 37:
        extensions = cbor_exact(auth[37:])
        require(isinstance(extensions, dict)
                and set(extensions) == {"validationCategory", "bundleVersion"},
                "unsupported App Attest assertion extension")
        raw_category = extensions["validationCategory"]
        bundle_version = extensions["bundleVersion"]
        require(type(raw_category) is bytes and len(raw_category) == 4
                and type(bundle_version) is str
                and 0 < len(bundle_version.encode("utf-8")) <= 128,
                "invalid App Attest assertion distribution extension")
        validation_category = int.from_bytes(raw_category, "little")
        require(validation_category in (2, 3, 4, 5),
                "unapproved Apple app distribution category")
    if expected_validation_category is not None:
        require(validation_category == expected_validation_category
                and bundle_version == expected_bundle_version,
                "Apple assertion distribution policy mismatch")
    require(openssl_path.is_absolute() and openssl_path.is_file(),
            "invalid assertion verification environment")
    # SubjectPublicKeyInfo ::= id-ecPublicKey, prime256v1, uncompressed point.
    spki = bytes.fromhex("3059301306072a8648ce3d020106082a8648ce3d030107034200") + attested_public_key_sec1
    with tempfile.TemporaryDirectory(prefix="kagemusha-apple-assertion-") as temporary:
        directory = Path(temporary)
        (directory / "public.pem").write_bytes(public_key_pem(spki))
        # App Attest signs the nonce, which is itself the SHA-256 digest of
        # authenticatorData || clientDataHash. ECDSA-SHA256 hashes that nonce
        # once more as its message; verifying the concatenation is a different
        # signature equation and rejects physical App Attest assertions.
        nonce = hashlib.sha256(auth + hashlib.sha256(client_data).digest()).digest()
        (directory / "signed.bin").write_bytes(nonce)
        (directory / "signature.bin").write_bytes(signature)
        result = subprocess.run(
            [str(openssl_path), "dgst", "-sha256", "-verify", str(directory / "public.pem"),
             "-signature", str(directory / "signature.bin"), str(directory / "signed.bin")],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False,
        )
        require(result.returncode == 0, "App Attest assertion signature rejected")
    return AppleAssertion(counter, hashlib.sha256(assertion_object).digest(),
                          hashlib.sha256(client_data).digest(),
                          validation_category, bundle_version)


class DurableAppleAssertionCounterStore:
    """Single-host, owner-only register for previously verified App Attest keys.

    A trusted caller must supply ``RawPlatformProof`` returned by
    ``verify_apple_raw``. The register never resets an existing counter, and
    one exclusive transaction verifies each assertion, consumes its exact
    client-data digest, and advances the counter. It does not authenticate a
    release, verify Apple's receipt, or prevent offline wallet-state rollback.
    Multi-host services need a shared store with equivalent transactions.
    """

    def __init__(self, path: Path) -> None:
        directory = path.parent
        mode = directory.stat()
        require(path.is_absolute() and not directory.is_symlink() and directory.is_dir()
                and stat.S_ISDIR(mode.st_mode) and mode.st_uid == os.getuid()
                and mode.st_mode & 0o077 == 0,
                "Apple counter store directory must be owner-only")
        require(not path.is_symlink(), "Apple counter store cannot be a symlink")
        if path.exists():
            existing = path.stat()
            require(stat.S_ISREG(existing.st_mode) and existing.st_uid == os.getuid()
                    and existing.st_mode & 0o077 == 0,
                    "Apple counter store must be an owner-only regular file")
        else:
            descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            os.close(descriptor)
        self.path = path
        with closing(self._connect()) as connection:
            connection.execute("""CREATE TABLE IF NOT EXISTS apple_keys (
                key_id BLOB PRIMARY KEY NOT NULL,
                point BLOB NOT NULL,
                app_id TEXT NOT NULL,
                environment TEXT NOT NULL,
                counter INTEGER NOT NULL CHECK(counter >= 0 AND counter < 4294967296)
            )""")
            connection.execute("""CREATE TABLE IF NOT EXISTS apple_client_data (
                client_data_sha256 BLOB PRIMARY KEY NOT NULL,
                key_id BLOB NOT NULL
            )""")
            # A server challenge is single-use across the entire register, not
            # merely once per attested key. Reject an older per-key layout.
            columns = connection.execute("PRAGMA table_info(apple_client_data)").fetchall()
            require([(row[1], row[2], row[3], row[5]) for row in columns] == [
                ("client_data_sha256", "BLOB", 1, 1),
                ("key_id", "BLOB", 1, 0),
            ], "unsupported Apple challenge register layout")

    def _connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.path, isolation_level=None, timeout=30)
        connection.execute("PRAGMA journal_mode=DELETE")
        connection.execute("PRAGMA synchronous=FULL")
        return connection

    def register_verified_key(self, proof: RawPlatformProof, app_id: str,
                              environment: str) -> bytes:
        """Insert one verified Apple key without resetting an existing counter."""
        point = proof.attested_public_key_sec1
        require(proof.platform == "apple_app_attest"
                and type(point) is bytes and len(point) == 65 and point[0] == 4
                and proof.device_key_reference == device_key_reference(point)
                and type(app_id) is str and 0 < len(app_id.encode("utf-8")) <= 255
                and environment in ("production", "development"),
                "invalid verified Apple key registration")
        key_id = hashlib.sha256(point).digest()
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            try:
                row = connection.execute(
                    "SELECT point, app_id, environment FROM apple_keys WHERE key_id = ?",
                    (key_id,),
                ).fetchone()
                if row is None:
                    connection.execute(
                        "INSERT INTO apple_keys (key_id, point, app_id, environment, counter) "
                        "VALUES (?, ?, ?, ?, 0)",
                        (key_id, point, app_id, environment),
                    )
                else:
                    require(row == (point, app_id, environment),
                            "Apple key already registered with different scope")
                connection.execute("COMMIT")
            except Exception:
                connection.execute("ROLLBACK")
                raise
        return key_id

    def verify_and_advance(
        self, assertion_object: bytes, client_data: bytes,
        expected_client_data: bytes, key_id: bytes, app_id: str,
        environment: str, openssl_path: Path,
        *, expected_validation_category: int | None,
        expected_bundle_version: str | None,
    ) -> AppleAssertion:
        """Verify, consume client data once, and durably advance one key counter."""
        require(type(key_id) is bytes and len(key_id) == 32,
                "invalid registered Apple key ID")
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            try:
                row = connection.execute(
                    "SELECT point, app_id, environment, counter FROM apple_keys WHERE key_id = ?",
                    (key_id,),
                ).fetchone()
                require(row is not None and row[1] == app_id
                        and row[2] == environment,
                        "unregistered Apple assertion key or scope")
                checked = verify_apple_assertion(
                    assertion_object, client_data, expected_client_data,
                    row[0], key_id, app_id, row[3], openssl_path,
                    expected_validation_category=expected_validation_category,
                    expected_bundle_version=expected_bundle_version,
                )
                require(connection.execute(
                    "SELECT 1 FROM apple_client_data WHERE client_data_sha256 = ?",
                    (checked.client_data_sha256,),
                ).fetchone() is None, "App Attest client data already consumed")
                connection.execute(
                    "INSERT INTO apple_client_data (client_data_sha256, key_id) VALUES (?, ?)",
                    (checked.client_data_sha256, key_id),
                )
                changed = connection.execute(
                    "UPDATE apple_keys SET counter = ? WHERE key_id = ? AND counter = ?",
                    (checked.counter, key_id, row[3]),
                ).rowcount
                require(changed == 1, "Apple assertion counter race")
                connection.execute("COMMIT")
            except Exception:
                connection.execute("ROLLBACK")
                raise
        return checked


def explicit_tags(value: DerValue, *, ordinary_version: int | None = None) -> dict[int, DerValue]:
    result: dict[int, DerValue] = {}
    prior = 0
    accepted = KEYMINT_AUTH_TAGS
    if ordinary_version == 2:
        accepted = accepted - {303, 305, 405, 507, 508, 509, 718, 719, 720, 723, 724} | {703}
    elif ordinary_version == 3:
        accepted = accepted - {305, 405, 720, 723, 724}
    elif ordinary_version == 4:
        accepted = accepted - {405, 723, 724}
    for field in children(value):
        require(field.tag_class == 2 and field.constructed and field.number in accepted
                and field.number > prior, "unsupported, duplicate or unsorted KeyMint authorization tag")
        item = der_one(field.value)
        if field.number in KEYMINT_SET_TAGS:
            members = children(item, number=17)
            require(all(member.tag_class == 0 and member.number == 2
                        and not member.constructed for member in members),
                    "invalid KeyMint integer set")
            for member in members:
                positive_integer(member)
        elif field.number in KEYMINT_INTEGER_TAGS:
            positive_integer(item)
        elif field.number in KEYMINT_NULL_TAGS or ordinary_version == 2 and field.number == 703:
            require(primitive(item, 5) == b"", "invalid KeyMint NULL authorization")
        elif field.number in KEYMINT_OCTET_TAGS:
            primitive(item, 4)
        # RootOfTrust is checked in full by verify_android_raw.
        result[field.number] = item
        prior = field.number
    return result


def keymint_integer_set(value: DerValue, expected: int) -> bool:
    members = children(value, number=17)
    return len(members) == 1 and positive_integer(members[0]) == expected


def verify_android_raw(
    chain_der: list[bytes], selection: Selection,
    package_name: str, package_version: int, signing_certificate_sha256: bytes,
    root_der: bytes, root_sha256: bytes, trusted_time_ms: int, openssl_path: Path,
    *, allowed_security_levels: frozenset[int],
) -> RawPlatformProof:
    """Check an app-owned persistent key under the authenticated hardware policy.

    The caller must check current revocation against the authenticated root's
    authority and independently select governed app distribution policy. This
    key authenticates app approval; it supplies no monotonic monetary journal,
    rollback-resistant wallet state, trusted clock or hardware one-use grant.
    """
    return _verify_android_raw(chain_der, selection, package_name, package_version,
        signing_certificate_sha256, root_der, root_sha256, trusted_time_ms, openssl_path,
        allowed_security_levels=allowed_security_levels, ordinary_persistent=False)


def verify_android_persistent_app_key_raw(
    chain_der: list[bytes], selection: Selection,
    package_name: str, package_version: int, signing_certificate_sha256: bytes,
    root_der: bytes, root_sha256: bytes, trusted_time_ms: int, openssl_path: Path,
    *, allowed_security_levels: frozenset[int],
) -> RawPlatformProof:
    """Verify only ordinary persistent app identity, including genuine Keymaster TEE.

    Trust, revocation by the caller, exact C challenge, package/signing pin, hardware
    P-256 generation and locked verified boot remain required. Keymaster3's original
    RootOfTrust has no verifiedBootHash; no value is fabricated for it. This proof
    supplies no finite-use, rollback-protected money state or monetary authority.
    """
    return _verify_android_raw(chain_der, selection, package_name, package_version,
        signing_certificate_sha256, root_der, root_sha256, trusted_time_ms, openssl_path,
        allowed_security_levels=allowed_security_levels, ordinary_persistent=True)


def _verify_android_raw(
    chain_der: list[bytes], selection: Selection,
    package_name: str, package_version: int, signing_certificate_sha256: bytes,
    root_der: bytes, root_sha256: bytes, trusted_time_ms: int, openssl_path: Path,
    *, allowed_security_levels: frozenset[int], ordinary_persistent: bool,
) -> RawPlatformProof:
    require(type(allowed_security_levels) is frozenset and allowed_security_levels
            and all(type(item) is int for item in allowed_security_levels)
            and allowed_security_levels <= {1, 2},
            "invalid authenticated Android security-level selection")
    require(type(package_name) is str and package_name
            and len(package_name.encode("utf-8")) <= 255
            and type(package_version) is int and package_version >= 0,
            "invalid pinned Android package")
    fixed32(signing_certificate_sha256, "Android signer")
    verify_pinned_chain(
        chain_der, root_der, root_sha256, trusted_time_ms, openssl_path,
        allow_google_factory_expired_root=(root_sha256 == GOOGLE_FACTORY_2016_ROOT_SHA256),
    )
    # Android requires the KeyDescription nearest the root. A certificate
    # farther toward the leaf can carry an attacker-added copy. The attested
    # point is the subject key of the selected certificate, not automatically
    # the first key in an extended chain.
    selected_certificate = next(
        (certificate for certificate in reversed(chain_der)
         if certificate != root_der
         and ANDROID_KEY_DESCRIPTION_OID in certificate_spki_extensions(certificate)[1]),
        None,
    )
    require(selected_certificate is not None, "missing KeyMint extension")
    point, extensions = certificate_key_extensions(selected_certificate)
    leaf_point, _ = certificate_key_extensions(chain_der[0])
    require(leaf_point == point,
            "KeyMint-attested point differs from app-controlled leaf key")
    require(selection.attested_key_id == b"\0" * 32,
            "KeyMint generation challenge must use the empty-key sentinel")
    extension = extensions.get(ANDROID_KEY_DESCRIPTION_OID)
    require(extension is not None, "missing KeyMint extension")
    description = children(der_one(extension))
    require(len(description) == 8, "invalid KeyMint key description")
    version = positive_integer(description[0])
    level = positive_integer(description[1], 10)
    keymint_version = positive_integer(description[2])
    keymint_level = positive_integer(description[3], 10)
    supported_version = ((version, keymint_version) in ORDINARY_PERSISTENT_ANDROID_VERSION_PAIRS
                         if ordinary_persistent else version in KEYMINT_VERSIONS and keymint_version == version)
    require(supported_version and (version != 2 or level == 1)
            and level in allowed_security_levels and keymint_level == level,
            "approval key security level differs from authenticated hardware policy")
    require(primitive(description[4], 4) == hashlib.sha256(selection.transcript()).digest(), "KeyMint challenge mismatch")
    ordinary_version = version if ordinary_persistent else None
    software = explicit_tags(description[6], ordinary_version=ordinary_version)
    hardware = explicit_tags(description[7], ordinary_version=ordinary_version)
    require(not (HARDWARE_ONLY_TAGS | ({703} if ordinary_version == 2 else set())).intersection(software), "hardware KeyMint authorization is software-enforced")
    require(ANDROID_APPROVAL_REQUIRED_HARDWARE_TAGS.issubset(hardware),
            "missing hardware KeyMint authorization")
    require(keymint_integer_set(hardware[1], 2) and positive_integer(hardware[2]) == 3
            and positive_integer(hardware[3]) == 256 and keymint_integer_set(hardware[5], 4)
            and positive_integer(hardware[10]) == 1 and positive_integer(hardware[702]) == 0,
            "KeyMint key is not hardware-generated P-256 SIGN/SHA-256")
    # A usage-limited key cannot serve as the persistent app approval key.
    # Ordinary Pixel StrongBox keys do not need either tag 303 or tag 405.
    # Optional rollback-resistance claims never become wallet-state authority.
    require(405 not in hardware and 405 not in software,
            "usage-limited key cannot be the persistent app approval key")
    require(709 in software and 709 not in hardware and 704 in hardware, "KeyMint app or boot trust missing")
    app_id = children(der_one(primitive(software[709], 4)))
    require(len(app_id) == 2, "invalid KeyMint app ID")
    packages = children(app_id[0], number=17)
    signers = children(app_id[1], number=17)
    require(len(packages) == 1 and len(signers) == 1, "shared-UID or multi-signer app is not approved")
    package = children(packages[0])
    require(len(package) == 2 and primitive(package[0], 4) == package_name.encode("utf-8")
            and positive_integer(package[1]) == package_version, "Android package mismatch")
    require(primitive(signers[0], 4) == signing_certificate_sha256, "Android signing identity mismatch")
    boot = children(hardware[704])
    # KeyMint 1.0+ requires verifiedBootHash to be a 32-byte value. Older HALs
    # did not strictly specify verifiedBootKey length, so preserve that field's
    # nonzero check for valid older OEM attestations.
    # https://android.googlesource.com/platform/hardware/interfaces/+/6a88f79b6426e80e0f72a8eaf429b010b183c12e/security/keymint/aidl/android/hardware/security/keymint/KeyCreationResult.aidl
    boot_fields = 3 if ordinary_persistent and version == 2 else 4
    require(len(boot) == boot_fields and primitive(boot[1], 1) == b"\xff"
            and positive_integer(boot[2], 10) == 0 and any(primitive(boot[0], 4))
            and (boot_fields == 3 or len(primitive(boot[3], 4)) == 32
                 and any(primitive(boot[3], 4))), "Android boot state is not locked and verified")

    def hardware_integer(tag: int) -> int | None:
        return positive_integer(hardware[tag]) if tag in hardware else None

    patch_levels = AndroidPatchLevels(
        version, hardware_integer(ANDROID_OS_VERSION_TAG),
        hardware_integer(ANDROID_OS_PATCH_LEVEL_TAG),
        hardware_integer(ANDROID_VENDOR_PATCH_LEVEL_TAG),
        hardware_integer(ANDROID_BOOT_PATCH_LEVEL_TAG))
    digest = hashlib.sha256(encode_android_chain(chain_der)).digest()
    return RawPlatformProof(digest, point, device_key_reference(point), "android_keymint",
                            android_security_level=level, android_patch_levels=patch_levels)
