#!/usr/bin/env python3
"""Verify ISO 20022 XMLDSig/XAdES operator trust bundles.

Purpose:
  This operator-side preflight validates JSON trust bundles before their
  profile trust material is merged into Torii ISO bridge configuration. It
  checks canonical SHA-256 pins, digest-bound DER blobs, duplicate material,
  revocation-material requirements, HTTPS provenance, and absence of
  secret-looking fields.

Prerequisites:
  Python 3.11+. No third party Python packages are required.

Safety:
  The script is read-only unless ``--emit-profile-json`` or ``--summary-out`` is
  supplied. It does not contact remote endpoints. Runtime secrets such as bearer
  tokens, private keys, and authorization headers are rejected if they appear in
  the bundle.
"""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import hashlib
import ipaddress
import json
import sys
import urllib.parse
from dataclasses import dataclass
from pathlib import Path
from typing import Any


BUNDLE_VERSION = 1
DEFAULT_POLICY = "require-verified"
MAX_DER_BLOBS = 8
MAX_DER_BYTES = 1024 * 1024
POLICIES = {"record-only", "reject-unsupported", "require-verified"}
REQUIRE_VERIFIED = "require-verified"
TOP_LEVEL_KEYS = {
    "version",
    "profile_id",
    "rail",
    "environment",
    "source",
    "embedded_signature_policy",
    "signature_public_key_sha256_pins",
    "trusted_public_key_sha256",
    "x509_trust_anchor_sha256_pins",
    "trusted_certificate_sha256",
    "x509_trust_anchors",
    "revoked_certificate_sha256",
    "revoked_certificates",
    "x509_required_certificate_policy_oids",
    "x509_require_crl_revocation_check",
    "x509_crls",
    "x509_require_ocsp_revocation_check",
    "x509_ocsp_responses",
}
SOURCE_KEYS = {"authority", "retrieved_at", "url", "version"}
DER_OBJECT_KEYS = {"label", "der_base64", "sha256"}
DER_KIND_CERTIFICATE = "X.509 certificate"
DER_KIND_CRL = "X.509 CRL"
DER_KIND_OCSP = "OCSP response"
OID_OCSP_BASIC_RESPONSE_DER = b"\x2b\x06\x01\x05\x05\x07\x30\x01\x01"
SUMMARY_DIGEST_FIELD = "summary_sha256"


class TrustBundleError(RuntimeError):
    """Raised when an ISO trust bundle is malformed or unsafe."""


@dataclass(frozen=True)
class DerElement:
    """One parsed DER TLV element."""

    tag: int
    header_len: int
    length: int
    start: int
    value_start: int
    end: int
    value: bytes


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _load_json(path: Path) -> Any:
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except FileNotFoundError as error:
        raise TrustBundleError(f"{path} does not exist") from error
    except json.JSONDecodeError as error:
        raise TrustBundleError(f"{path} is not valid JSON: {error}") from error


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise TrustBundleError(f"JSON object contains duplicate key {key!r}")
        seen.add(key)
        result[key] = value
    return result


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise TrustBundleError(f"{label} must be a JSON object")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise TrustBundleError(f"{label} contains unknown keys: {', '.join(unknown)}")


def _check_no_secret_material(value: Any, path: str = "$") -> None:
    forbidden = ("authorization", "bearer", "token", "secret", "private_key", "x-iroha-signature")
    if isinstance(value, dict):
        for key, child in value.items():
            lowered = str(key).lower()
            if any(word in lowered for word in forbidden):
                raise TrustBundleError(f"{path}.{key} is a forbidden secret-looking field")
            _check_no_secret_material(child, f"{path}.{key}")
    elif isinstance(value, list):
        for offset, child in enumerate(value):
            _check_no_secret_material(child, f"{path}[{offset}]")
    elif isinstance(value, str) and value.strip().lower().startswith("bearer "):
        raise TrustBundleError(f"{path} contains bearer-token material")


def _has_ascii_control(value: str) -> bool:
    return any(ord(char) < 0x20 or ord(char) == 0x7F for char in value)


def _reject_ascii_control(value: str, label: str) -> None:
    if _has_ascii_control(value):
        raise TrustBundleError(f"{label} must not contain ASCII control characters")


def _required_string(bundle: dict[str, Any], key: str, label: str) -> str:
    raw = bundle.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise TrustBundleError(f"{label}.{key} must be a non-empty string")
    _reject_ascii_control(raw, f"{label}.{key}")
    return raw.strip()


def _optional_string(bundle: dict[str, Any], key: str, label: str) -> str | None:
    raw = bundle.get(key)
    if raw is None:
        return None
    if not isinstance(raw, str) or not raw.strip():
        raise TrustBundleError(f"{label}.{key} must be a non-empty string when provided")
    _reject_ascii_control(raw, f"{label}.{key}")
    return raw.strip()


def _required_bool(bundle: dict[str, Any], key: str, label: str) -> bool:
    raw = bundle.get(key)
    if not isinstance(raw, bool):
        raise TrustBundleError(f"{label}.{key} must be a boolean")
    return raw


def _is_lower_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _validate_sha256(value: str, label: str) -> str:
    if not _is_lower_sha256(value):
        raise TrustBundleError(f"{label} must be canonical lowercase SHA-256 hex")
    if all(ch == "0" for ch in value):
        raise TrustBundleError(f"{label} must not be all zero")
    return value


def _sha256_list(bundle: dict[str, Any], key: str, label: str) -> list[str]:
    raw = bundle.get(key, [])
    if not isinstance(raw, list):
        raise TrustBundleError(f"{label}.{key} must be an array of SHA-256 strings")
    result: list[str] = []
    seen: set[str] = set()
    for offset, item in enumerate(raw):
        if not isinstance(item, str):
            raise TrustBundleError(f"{label}.{key}[{offset}] must be a SHA-256 string")
        digest = _validate_sha256(item.strip(), f"{label}.{key}[{offset}]")
        if digest in seen:
            raise TrustBundleError(f"{label}.{key}[{offset}] duplicates SHA-256 {digest}")
        seen.add(digest)
        result.append(digest)
    return result


def _oid_list(bundle: dict[str, Any], key: str, label: str) -> list[str]:
    raw = bundle.get(key, [])
    if not isinstance(raw, list):
        raise TrustBundleError(f"{label}.{key} must be an array of dotted numeric OIDs")
    result: list[str] = []
    seen: set[str] = set()
    for offset, item in enumerate(raw):
        if not isinstance(item, str):
            raise TrustBundleError(f"{label}.{key}[{offset}] must be a dotted numeric OID")
        value = item.strip()
        if not _valid_oid(value):
            raise TrustBundleError(f"{label}.{key}[{offset}] must be a dotted numeric OID")
        if value in seen:
            raise TrustBundleError(f"{label}.{key}[{offset}] duplicates OID {value}")
        seen.add(value)
        result.append(value)
    return result


def _valid_oid(value: str) -> bool:
    parts = value.split(".")
    if len(parts) < 2:
        return False
    for part in parts:
        if not part or not part.isascii() or not part.isdecimal():
            return False
        if len(part) > 1 and part.startswith("0"):
            return False
    first = int(parts[0])
    if first > 2:
        return False
    if first < 2 and int(parts[1]) > 39:
        return False
    return True


def _strict_base64_der(
    value: str,
    label: str,
    *,
    kind: str,
    allow_synthetic_der: bool,
) -> tuple[bytes, str]:
    raw = value.strip()
    try:
        der = base64.b64decode(raw, validate=True)
    except ValueError as error:
        raise TrustBundleError(f"{label} must be canonical base64 DER") from error
    if not der or len(der) > MAX_DER_BYTES:
        raise TrustBundleError(f"{label} must be non-empty DER no larger than {MAX_DER_BYTES} bytes")
    _require_der_sequence(der, label)
    if not allow_synthetic_der:
        _require_der_kind(der, label, kind)
    canonical = base64.b64encode(der).decode("ascii")
    if canonical != raw:
        raise TrustBundleError(f"{label} must be canonical padded base64")
    return der, canonical


def _require_der_sequence(der: bytes, label: str) -> None:
    root = _read_der_element(der, 0, label)
    if root.tag != 0x30:
        raise TrustBundleError(f"{label} must be a DER SEQUENCE")
    if root.end != len(der):
        raise TrustBundleError(f"{label} DER length does not consume the whole value")


def _read_der_element(data: bytes, offset: int, label: str) -> DerElement:
    if offset >= len(data):
        raise TrustBundleError(f"{label} has truncated DER")
    if len(data) - offset < 2:
        raise TrustBundleError(f"{label} has truncated DER length")
    tag = data[offset]
    length_byte = data[offset + 1]
    if length_byte < 0x80:
        length = length_byte
        header_len = 2
    else:
        length_len = length_byte & 0x7F
        if length_len == 0:
            raise TrustBundleError(f"{label} must not use BER indefinite length")
        if length_len > 4 or len(data) - offset < 2 + length_len:
            raise TrustBundleError(f"{label} has invalid DER length")
        length_bytes = data[offset + 2 : offset + 2 + length_len]
        if length_bytes[0] == 0:
            raise TrustBundleError(f"{label} has non-minimal DER length")
        length = int.from_bytes(length_bytes, "big")
        if length < 0x80:
            raise TrustBundleError(f"{label} must use short DER length form")
        header_len = 2 + length_len
    end = offset + header_len + length
    if end > len(data):
        raise TrustBundleError(f"{label} has truncated DER value")
    return DerElement(
        tag=tag,
        header_len=header_len,
        length=length,
        start=offset,
        value_start=offset + header_len,
        end=end,
        value=data[offset + header_len : end],
    )


def _der_children(element: DerElement, label: str) -> list[DerElement]:
    children: list[DerElement] = []
    offset = 0
    while offset < len(element.value):
        child = _read_der_element(element.value, offset, label)
        children.append(child)
        offset = child.end
    return children


def _root_children(der: bytes, label: str) -> list[DerElement]:
    root = _read_der_element(der, 0, label)
    if root.tag != 0x30:
        raise TrustBundleError(f"{label} must be a DER SEQUENCE")
    if root.end != len(der):
        raise TrustBundleError(f"{label} DER length does not consume the whole value")
    return _der_children(root, label)


def _require_der_kind(der: bytes, label: str, kind: str) -> None:
    if kind == DER_KIND_CERTIFICATE:
        if not _looks_like_x509_certificate(der, label):
            raise TrustBundleError(f"{label} must look like an X.509 certificate")
    elif kind == DER_KIND_CRL:
        if not _looks_like_x509_crl(der, label):
            raise TrustBundleError(f"{label} must look like an X.509 CRL")
    elif kind == DER_KIND_OCSP:
        if not _looks_like_ocsp_response(der, label):
            raise TrustBundleError(f"{label} must look like an OCSPResponse")
    else:  # pragma: no cover - internal caller bug.
        raise TrustBundleError(f"{label} has unsupported DER kind {kind}")


def _looks_like_algorithm_identifier(element: DerElement, label: str) -> bool:
    if element.tag != 0x30:
        return False
    children = _der_children(element, label)
    return bool(children) and children[0].tag == 0x06


def _looks_like_x509_certificate(der: bytes, label: str) -> bool:
    children = _root_children(der, label)
    if len(children) != 3 or children[0].tag != 0x30 or children[2].tag != 0x03:
        return False
    if not _looks_like_algorithm_identifier(children[1], label):
        return False
    tbs_children = _der_children(children[0], label)
    cursor = 1 if tbs_children and tbs_children[0].tag == 0xA0 else 0
    if len(tbs_children) < cursor + 6:
        return False
    return (
        tbs_children[cursor].tag == 0x02
        and _looks_like_algorithm_identifier(tbs_children[cursor + 1], label)
        and tbs_children[cursor + 2].tag == 0x30
        and tbs_children[cursor + 3].tag == 0x30
        and tbs_children[cursor + 4].tag == 0x30
        and tbs_children[cursor + 5].tag == 0x30
    )


def _looks_like_x509_crl(der: bytes, label: str) -> bool:
    children = _root_children(der, label)
    if len(children) != 3 or children[0].tag != 0x30 or children[2].tag != 0x03:
        return False
    if not _looks_like_algorithm_identifier(children[1], label):
        return False
    tbs_children = _der_children(children[0], label)
    cursor = 1 if tbs_children and tbs_children[0].tag == 0x02 else 0
    if len(tbs_children) < cursor + 3:
        return False
    this_update = tbs_children[cursor + 2]
    return (
        _looks_like_algorithm_identifier(tbs_children[cursor], label)
        and tbs_children[cursor + 1].tag == 0x30
        and this_update.tag in (0x17, 0x18)
    )


def _looks_like_ocsp_response(der: bytes, label: str) -> bool:
    children = _root_children(der, label)
    if not children or children[0].tag != 0x0A:
        return False
    if len(children) == 1:
        return True
    if len(children) != 2 or children[1].tag != 0xA0:
        return False
    response_bytes_children = _der_children(children[1], label)
    if len(response_bytes_children) != 1 or response_bytes_children[0].tag != 0x30:
        return False
    wrapped = _der_children(response_bytes_children[0], label)
    return (
        len(wrapped) == 2
        and wrapped[0].tag == 0x06
        and wrapped[0].value == OID_OCSP_BASIC_RESPONSE_DER
        and wrapped[1].tag == 0x04
    )


def _der_objects(
    bundle: dict[str, Any],
    key: str,
    label: str,
    *,
    kind: str,
    allow_synthetic_der: bool,
) -> tuple[list[dict[str, Any]], list[str]]:
    raw = bundle.get(key, [])
    if not isinstance(raw, list):
        raise TrustBundleError(f"{label}.{key} must be an array of DER objects")
    if len(raw) > MAX_DER_BLOBS:
        raise TrustBundleError(f"{label}.{key} must not contain more than {MAX_DER_BLOBS} entries")
    entries: list[dict[str, Any]] = []
    base64_values: list[str] = []
    seen: set[str] = set()
    seen_labels: set[str] = set()
    for offset, item in enumerate(raw):
        obj = _require_object(item, f"{label}.{key}[{offset}]")
        _reject_unknown_keys(obj, DER_OBJECT_KEYS, f"{label}.{key}[{offset}]")
        name = _optional_string(obj, "label", f"{label}.{key}[{offset}]")
        if name is not None:
            if len(name) > 128:
                raise TrustBundleError(f"{label}.{key}[{offset}].label must be no longer than 128 characters")
            if name in seen_labels:
                raise TrustBundleError(f"{label}.{key}[{offset}].label duplicates label {name!r}")
            seen_labels.add(name)
        der_b64 = _required_string(obj, "der_base64", f"{label}.{key}[{offset}]")
        der, canonical_b64 = _strict_base64_der(
            der_b64,
            f"{label}.{key}[{offset}].der_base64",
            kind=kind,
            allow_synthetic_der=allow_synthetic_der,
        )
        digest = sha256_hex(der)
        declared_digest = obj.get("sha256")
        if declared_digest is not None:
            if not isinstance(declared_digest, str):
                raise TrustBundleError(f"{label}.{key}[{offset}].sha256 must be a string")
            if _validate_sha256(declared_digest.strip(), f"{label}.{key}[{offset}].sha256") != digest:
                raise TrustBundleError(f"{label}.{key}[{offset}].sha256 does not match der_base64")
        if digest in seen:
            raise TrustBundleError(f"{label}.{key}[{offset}] duplicates DER SHA-256 {digest}")
        seen.add(digest)
        entries.append(
            {
                "label": name,
                "sha256": digest,
                "der_base64": canonical_b64,
                "byte_len": len(der),
            }
        )
        base64_values.append(canonical_b64)
    return entries, base64_values


def _source(bundle: dict[str, Any], label: str, allow_insecure_source_url: bool) -> dict[str, Any] | None:
    raw = bundle.get("source")
    if raw is None:
        return None
    source = _require_object(raw, f"{label}.source")
    _reject_unknown_keys(source, SOURCE_KEYS, f"{label}.source")
    normalized: dict[str, Any] = {}
    for key in ["authority", "version"]:
        value = _optional_string(source, key, f"{label}.source")
        if value is not None:
            normalized[key] = value
    retrieved_at = _optional_string(source, "retrieved_at", f"{label}.source")
    if retrieved_at is not None:
        _validate_retrieved_at(retrieved_at, f"{label}.source.retrieved_at")
        normalized["retrieved_at"] = retrieved_at
    url = _optional_string(source, "url", f"{label}.source")
    if url is not None:
        _validate_source_url(
            url,
            f"{label}.source.url",
            allow_insecure_source_url=allow_insecure_source_url,
        )
        normalized["url"] = url
    return normalized


def _validate_retrieved_at(value: str, label: str) -> None:
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    try:
        parsed = dt.datetime.fromisoformat(normalized)
    except ValueError as error:
        raise TrustBundleError(f"{label} must be an ISO 8601 timestamp with timezone") from error
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise TrustBundleError(f"{label} must include a timezone")
    now = dt.datetime.now(dt.UTC)
    if parsed.astimezone(dt.UTC) > now + dt.timedelta(minutes=5):
        raise TrustBundleError(f"{label} must not be in the future")


def _validate_source_url(
    url: str,
    label: str,
    *,
    allow_insecure_source_url: bool,
) -> None:
    _reject_ascii_control(url, label)
    try:
        parsed = urllib.parse.urlparse(url)
        hostname = parsed.hostname
    except ValueError as error:
        raise TrustBundleError(f"{label} is malformed") from error
    if parsed.scheme != "https" and not (parsed.scheme == "http" and allow_insecure_source_url):
        raise TrustBundleError(f"{label} must use HTTPS")
    if not parsed.netloc or hostname is None:
        raise TrustBundleError(f"{label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise TrustBundleError(f"{label} must not contain credentials")
    if parsed.params or parsed.query or parsed.fragment:
        raise TrustBundleError(f"{label} must not contain params, query, or fragment")
    hostname = hostname.strip().lower()
    if not hostname:
        raise TrustBundleError(f"{label} must include a host")
    if not allow_insecure_source_url:
        if hostname == "localhost" or hostname.endswith(".localhost"):
            raise TrustBundleError(f"{label} must not use localhost")
        try:
            address = ipaddress.ip_address(hostname)
        except ValueError:
            return
        if not address.is_global:
            raise TrustBundleError(f"{label} must not use local, private, or reserved IP addresses")


def _merge_unique(values: list[str], additions: list[str], label: str) -> list[str]:
    seen = set(values)
    result = list(values)
    for value in additions:
        if value in seen:
            raise TrustBundleError(f"{label} duplicates SHA-256 {value}")
        seen.add(value)
        result.append(value)
    return result


def _reject_overlap(left: list[str], right: list[str], label: str) -> None:
    overlap = sorted(set(left) & set(right))
    if overlap:
        raise TrustBundleError(f"{label} contains conflicting SHA-256 {overlap[0]}")


def verify_bundle(
    path: Path,
    *,
    allow_record_only: bool,
    allow_insecure_source_url: bool,
    allow_synthetic_der: bool,
) -> dict[str, Any]:
    """Verify one trust bundle and return a normalized summary."""

    bundle = _require_object(_load_json(path), str(path))
    _reject_unknown_keys(bundle, TOP_LEVEL_KEYS, str(path))
    _check_no_secret_material(bundle)
    if bundle.get("version") != BUNDLE_VERSION:
        raise TrustBundleError(f"{path}.version must be {BUNDLE_VERSION}")

    profile_id = _required_string(bundle, "profile_id", str(path))
    rail = _required_string(bundle, "rail", str(path))
    environment = _required_string(bundle, "environment", str(path))
    policy = bundle.get("embedded_signature_policy", DEFAULT_POLICY)
    if not isinstance(policy, str) or policy not in POLICIES:
        raise TrustBundleError(f"{path}.embedded_signature_policy is unsupported")
    if policy != REQUIRE_VERIFIED and not allow_record_only:
        raise TrustBundleError(
            f"{path}.embedded_signature_policy must be {REQUIRE_VERIFIED!r} for production bundles"
        )

    raw_public_pins = _sha256_list(bundle, "signature_public_key_sha256_pins", str(path))
    legacy_public_pins = _sha256_list(bundle, "trusted_public_key_sha256", str(path))
    anchor_pin_values = _sha256_list(bundle, "x509_trust_anchor_sha256_pins", str(path))
    legacy_anchor_pin_values = _sha256_list(bundle, "trusted_certificate_sha256", str(path))
    revoked_pin_values = _sha256_list(bundle, "revoked_certificate_sha256", str(path))
    policy_oids = _oid_list(bundle, "x509_required_certificate_policy_oids", str(path))

    trust_anchors, trust_anchor_der_values = _der_objects(
        bundle,
        "x509_trust_anchors",
        str(path),
        kind=DER_KIND_CERTIFICATE,
        allow_synthetic_der=allow_synthetic_der,
    )
    revoked_certificates, _revoked_der_values = _der_objects(
        bundle,
        "revoked_certificates",
        str(path),
        kind=DER_KIND_CERTIFICATE,
        allow_synthetic_der=allow_synthetic_der,
    )
    crls, crl_values = _der_objects(
        bundle,
        "x509_crls",
        str(path),
        kind=DER_KIND_CRL,
        allow_synthetic_der=allow_synthetic_der,
    )
    ocsp_responses, ocsp_values = _der_objects(
        bundle,
        "x509_ocsp_responses",
        str(path),
        kind=DER_KIND_OCSP,
        allow_synthetic_der=allow_synthetic_der,
    )

    x509_trust_anchor_sha256_pins = _merge_unique(
        anchor_pin_values,
        [entry["sha256"] for entry in trust_anchors],
        f"{path}.x509_trust_anchor_sha256_pins",
    )
    trusted_certificate_sha256 = _merge_unique(
        legacy_anchor_pin_values,
        [],
        f"{path}.trusted_certificate_sha256",
    )
    revoked_certificate_sha256 = _merge_unique(
        revoked_pin_values,
        [entry["sha256"] for entry in revoked_certificates],
        f"{path}.revoked_certificate_sha256",
    )
    _reject_overlap(
        raw_public_pins,
        legacy_public_pins,
        f"{path}.signature_public_key_sha256_pins/trusted_public_key_sha256",
    )
    _reject_overlap(
        x509_trust_anchor_sha256_pins,
        trusted_certificate_sha256,
        f"{path}.x509_trust_anchor_sha256_pins/trusted_certificate_sha256",
    )
    _reject_overlap(
        x509_trust_anchor_sha256_pins + trusted_certificate_sha256,
        revoked_certificate_sha256,
        f"{path}.trusted/revoked certificate pins",
    )

    crl_required = _required_bool(bundle, "x509_require_crl_revocation_check", str(path))
    ocsp_required = _required_bool(bundle, "x509_require_ocsp_revocation_check", str(path))
    if crl_required and not crl_values:
        raise TrustBundleError(f"{path} requires CRL revocation checking but has no x509_crls")
    if ocsp_required and not ocsp_values:
        raise TrustBundleError(f"{path} requires OCSP revocation checking but has no x509_ocsp_responses")
    if policy == REQUIRE_VERIFIED and not (
        raw_public_pins
        or legacy_public_pins
        or x509_trust_anchor_sha256_pins
        or trusted_certificate_sha256
    ):
        raise TrustBundleError(f"{path} has require-verified policy but no trust pins")

    source = _source(bundle, str(path), allow_insecure_source_url)
    profile_overrides = {
        "id": profile_id,
        "rail": rail,
        "embedded_signature_policy": policy,
        "signature_public_key_sha256_pins": raw_public_pins,
        "trusted_public_key_sha256": legacy_public_pins,
        "x509_trust_anchor_sha256_pins": x509_trust_anchor_sha256_pins,
        "trusted_certificate_sha256": trusted_certificate_sha256,
        "revoked_certificate_sha256": revoked_certificate_sha256,
        "x509_required_certificate_policy_oids": policy_oids,
        "x509_require_crl_revocation_check": crl_required,
        "x509_crl_der_base64": crl_values,
        "x509_require_ocsp_revocation_check": ocsp_required,
        "x509_ocsp_response_der_base64": ocsp_values,
    }
    material_summary = {
        "signature_public_key_pin_count": len(raw_public_pins) + len(legacy_public_pins),
        "x509_trust_anchor_pin_count": len(x509_trust_anchor_sha256_pins)
        + len(trusted_certificate_sha256),
        "revoked_certificate_pin_count": len(revoked_certificate_sha256),
        "x509_crl_count": len(crls),
        "x509_ocsp_response_count": len(ocsp_responses),
        "x509_required_certificate_policy_oid_count": len(policy_oids),
    }
    summary = {
        "path": str(path),
        "profile_id": profile_id,
        "rail": rail,
        "environment": environment,
        "source": source,
        "embedded_signature_policy": policy,
        "material": material_summary,
        "x509_trust_anchors": [
            {key: value for key, value in entry.items() if key != "der_base64"}
            for entry in trust_anchors
        ],
        "revoked_certificates": [
            {key: value for key, value in entry.items() if key != "der_base64"}
            for entry in revoked_certificates
        ],
        "x509_crls": [
            {key: value for key, value in entry.items() if key != "der_base64"}
            for entry in crls
        ],
        "x509_ocsp_responses": [
            {key: value for key, value in entry.items() if key != "der_base64"}
            for entry in ocsp_responses
        ],
        "profile_overrides": profile_overrides,
    }
    summary["bundle_sha256"] = sha256_hex(_canonical_json_bytes(bundle))
    return summary


def run(args: argparse.Namespace) -> int:
    if args.allow_synthetic_der and args.emit_profile_json is not None:
        raise TrustBundleError(
            "--allow-synthetic-der cannot be combined with --emit-profile-json; "
            "replace template DER with real rail material before emitting profile overrides"
        )
    summaries = [
        verify_bundle(
            path.resolve(),
            allow_record_only=args.allow_record_only,
            allow_insecure_source_url=args.allow_insecure_source_url,
            allow_synthetic_der=args.allow_synthetic_der,
        )
        for path in args.bundle
    ]
    output: dict[str, Any] = {
        "verified_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "verified_bundles": len(summaries),
        "allow_record_only": args.allow_record_only,
        "allow_insecure_source_url": args.allow_insecure_source_url,
        "allow_synthetic_der": args.allow_synthetic_der,
        "profile_json_emitted": args.emit_profile_json is not None,
        "profile_json_emittable": not args.allow_synthetic_der,
        "bundles": summaries,
    }
    output[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(output))
    text = json.dumps(output, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        args.summary_out.parent.mkdir(parents=True, exist_ok=True)
        args.summary_out.write_text(text, encoding="utf-8")
    print(text, end="")

    if args.emit_profile_json is not None:
        profile_config = [summary["profile_overrides"] for summary in summaries]
        args.emit_profile_json.parent.mkdir(parents=True, exist_ok=True)
        args.emit_profile_json.write_text(
            json.dumps(profile_config, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify ISO 20022 XMLDSig/XAdES operator trust bundle JSON."
    )
    parser.add_argument(
        "--bundle",
        action="append",
        required=True,
        type=Path,
        help="Trust bundle JSON file to verify; repeatable.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path to write the verification summary JSON.",
    )
    parser.add_argument(
        "--emit-profile-json",
        type=Path,
        help="Optional path to write Torii profile trust override JSON.",
    )
    parser.add_argument(
        "--allow-record-only",
        action="store_true",
        help="Allow non-production record-only or reject-unsupported policies.",
    )
    parser.add_argument(
        "--allow-insecure-source-url",
        action="store_true",
        help="Allow http:// provenance URLs for local tests.",
    )
    parser.add_argument(
        "--allow-synthetic-der",
        action="store_true",
        help="Allow DER SEQUENCE placeholders for checked-in templates; production bundles should omit this.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except TrustBundleError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
