"""Bounded, independent verification of Apple App Attest fraud receipts.

The receipt is a CMS SignedData object (Apple's example uses BER framing) with
a definite-length ASN.1 ``SET OF ReceiptAttribute`` payload. Apple's published
sample orders attributes by numeric field ID, not DER SET lexical order. This
module verifies it separately
from raw App Attest evidence and never grants certificate-issuance authority.
The root and trusted clock must be selected by the deployment, not the app.
"""

from __future__ import annotations

import hashlib
import re
import subprocess
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from .attestation import (
    AttestationRejected,
    certificate_key_extensions,
    certificate_spki_extensions,
    children,
    der_at,
    der_one,
    fixed32,
    pem,
    positive_integer,
    primitive,
    require,
)


MAX_RECEIPT = 64 * 1024
MAX_PAYLOAD = 32 * 1024
MAX_AGE_MS = 5 * 60 * 1000
# Present in the App Attest fraud-receipt signing certificate in Apple's
# published attestation-object example. A different Apple signer profile must
# be reviewed and deliberately selected; root trust alone is too broad.
APPLE_FRAUD_RECEIPT_SIGNER_OID = "1.2.840.113635.100.12.15"
_TIMESTAMP = re.compile(rb"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,6})?Z\Z")


@dataclass(frozen=True)
class VerifiedAppleReceipt:
    """Receipt facts after CMS and field checks; not an issuance token."""

    receipt_sha256: bytes
    receipt_type: str
    creation_time_ms: int
    risk_metric: int | None


def _timestamp_ms(raw: bytes) -> int:
    require(bool(_TIMESTAMP.fullmatch(raw)), "invalid Apple receipt timestamp")
    try:
        parsed = datetime.fromisoformat(raw.decode("ascii").replace("Z", "+00:00"))
    except ValueError as error:
        raise AttestationRejected("invalid Apple receipt timestamp") from error
    delta = parsed - datetime(1970, 1, 1, tzinfo=timezone.utc)
    return (delta.days * 86_400_000 + delta.seconds * 1000
            + delta.microseconds // 1000)


def _receipt_fields(payload: bytes) -> dict[int, bytes]:
    """Parse definite-length receipt attributes with unique ascending IDs."""
    require(0 < len(payload) <= MAX_PAYLOAD, "Apple receipt payload outside bound")
    outer = der_one(payload)
    require(outer.tag_class == 0 and outer.constructed and outer.number == 17,
            "Apple receipt payload is not SET OF")
    attributes: dict[int, bytes] = {}
    offset = 0
    last_number = 0
    while offset < len(outer.value):
        attribute, next_offset = der_at(outer.value, offset)
        fields = children(attribute)
        require(len(fields) == 3, "invalid Apple receipt attribute")
        number, version = positive_integer(fields[0]), positive_integer(fields[1])
        require(last_number < number < 1 << 16 and version == 1,
                "duplicate or nonascending Apple receipt attribute")
        attributes[number] = primitive(fields[2], 4)
        require(len(attributes) <= 32, "too many Apple receipt attributes")
        last_number, offset = number, next_offset
    return attributes


def verify_apple_receipt(
    receipt: bytes,
    app_id: str,
    attested_certificate_der: bytes,
    attested_public_key_sec1: bytes,
    root_der: bytes,
    root_sha256: bytes,
    trusted_time_ms: int,
    openssl_path: Path,
    *,
    expected_type: str = "ATTEST",
) -> VerifiedAppleReceipt:
    """Verify CMS, Apple signer role, app identity, key, type and freshness.

    ``attested_certificate_der`` must be the already-verified credCert leaf
    from the App Attest object. Matching it exactly to receipt field 3 prevents
    a valid receipt for a different attestation from being substituted. The
    caller still needs release governance and durable assertion-counter state.
    """
    require(type(receipt) is bytes and 0 < len(receipt) <= MAX_RECEIPT,
            "Apple receipt outside bound")
    require(type(app_id) is str and 0 < len(app_id.encode("utf-8")) <= 255,
            "invalid selected App ID")
    require(type(attested_certificate_der) is bytes and 0 < len(attested_certificate_der) <= 16 * 1024,
            "invalid attested certificate")
    point, _ = certificate_key_extensions(attested_certificate_der)
    require(type(attested_public_key_sec1) is bytes and point == attested_public_key_sec1,
            "attested certificate key mismatch")
    require(type(root_der) is bytes and 0 < len(root_der) <= 16 * 1024
            and hashlib.sha256(root_der).digest() == fixed32(root_sha256, "Apple receipt root pin"),
            "untrusted Apple receipt root")
    require(type(trusted_time_ms) is int and trusted_time_ms > 0
            and isinstance(openssl_path, Path) and openssl_path.is_absolute()
            and openssl_path.is_file(), "invalid Apple receipt verification environment")
    require(expected_type in ("ATTEST", "RECEIPT"), "invalid selected Apple receipt type")

    with tempfile.TemporaryDirectory(prefix="kagemusha-apple-receipt-") as temporary:
        directory = Path(temporary)
        receipt_path = directory / "receipt.cms"
        root_path = directory / "root.pem"
        payload_path = directory / "payload.der"
        signer_path = directory / "signer.pem"
        receipt_path.write_bytes(receipt)
        root_path.write_bytes(pem(root_der))
        result = subprocess.run(
            [str(openssl_path), "cms", "-verify", "-binary", "-inform", "DER",
             "-in", str(receipt_path), "-CAfile", str(root_path),
             "-no-CAfile", "-no-CApath", "-no-CAstore", "-purpose", "any",
             "-attime", str(trusted_time_ms // 1000), "-verify_retcode",
             "-out", str(payload_path), "-signer", str(signer_path)],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False,
        )
        require(result.returncode == 0, "Apple receipt CMS signature or chain rejected")
        payload = payload_path.read_bytes()
        signer_pem = signer_path.read_bytes()
        require(signer_pem.count(b"-----BEGIN CERTIFICATE-----") == 1
                and signer_pem.count(b"-----END CERTIFICATE-----") == 1,
                "Apple receipt requires one signer")
        signer_der = directory / "signer.der"
        converted = subprocess.run(
            [str(openssl_path), "x509", "-in", str(signer_path), "-outform", "DER",
             "-out", str(signer_der)],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False,
        )
        require(converted.returncode == 0 and signer_der.stat().st_size <= 16 * 1024,
                "invalid Apple receipt signer certificate")
        _, extensions = certificate_spki_extensions(signer_der.read_bytes())
        require(extensions.get(APPLE_FRAUD_RECEIPT_SIGNER_OID) == b"\x05\x00",
                "Apple receipt signer lacks fraud-receipt role")

    attributes = _receipt_fields(payload)
    require({2, 3, 4, 5, 6, 12} <= attributes.keys(), "incomplete Apple receipt")
    require(attributes[2] == app_id.encode("utf-8"), "Apple receipt App ID mismatch")
    require(attributes[3] == attested_certificate_der,
            "Apple receipt attested certificate mismatch")
    require(0 < len(attributes[4]) <= 1024 and 0 < len(attributes[5]) <= 4096,
            "invalid Apple receipt client hash or token")
    receipt_type = attributes[6]
    require(receipt_type in (b"ATTEST", b"RECEIPT")
            and receipt_type.decode("ascii") == expected_type,
            "Apple receipt type mismatch")
    created_ms = _timestamp_ms(attributes[12])
    require(0 <= trusted_time_ms - created_ms <= MAX_AGE_MS,
            "Apple receipt creation time is stale or in the future")
    risk_metric: int | None = None
    if receipt_type == b"ATTEST":
        require(17 not in attributes, "initial Apple receipt has a risk metric")
    else:
        raw_metric = attributes.get(17, b"")
        require(0 < len(raw_metric) <= 10 and raw_metric.isascii()
                and raw_metric.isdigit() and (raw_metric == b"0" or raw_metric[0] != 48),
                "invalid Apple receipt risk metric")
        risk_metric = int(raw_metric)
    if 21 in attributes:
        require(_timestamp_ms(attributes[21]) > trusted_time_ms,
                "expired Apple receipt")
    if 19 in attributes:
        require(_timestamp_ms(attributes[19]) >= created_ms,
                "invalid Apple receipt not-before time")
    return VerifiedAppleReceipt(hashlib.sha256(receipt).digest(), expected_type,
                                created_ms, risk_metric)
