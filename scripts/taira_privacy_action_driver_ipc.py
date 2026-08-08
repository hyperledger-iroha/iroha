#!/usr/bin/env python3
"""Closed one-shot IPC codec for the non-networked Exact12 action driver.

The controller is the only network actor.  This module frames the public
context sent to the Rust action constructor and independently parses the
returned proof-bearing signed transaction bytes.  It never
accepts endpoints, credentials, private keys, witnesses, or outcome claims.
"""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence
from typing import NoReturn


REQUEST_SCHEMA = "iroha.taira.privacy_action_driver_request"
RESPONSE_SCHEMA = "iroha.taira.privacy_action_driver_response"
SCHEMA_VERSION = 1
OPERATION = "build-verange-action-v1"
PROTOCOL = "verange-transparent-range-v1"
REQUEST_ID_DOMAIN = b"iroha.taira.privacy_action_driver_request.v1\0"
MAX_REQUEST_BYTES = 16 * 1024
MAX_TRANSACTION_BYTES = 9 * 1024 * 1024
MAX_RESPONSE_BYTES = 2 * MAX_TRANSACTION_BYTES + MAX_REQUEST_BYTES
MAX_TTL_MILLIS = 2 * 60 * 60 * 1000
MAX_CREATION_TIME_MILLIS = 2**63 - 1
MAX_ASSET_DEFINITION_ID_BYTES = 1024
MAX_CHAIN_ID_BYTES = 128
SHA256_RE = re.compile(r"[0-9a-f]{64}")


class PrivacyActionDriverIpcError(RuntimeError):
    """The action-driver frame is noncanonical, unbounded, or substituted."""


def _fail(message: str) -> NoReturn:
    raise PrivacyActionDriverIpcError(message)


def _canonical(value: object) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=True,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise PrivacyActionDriverIpcError(
            f"action-driver frame is not canonically encodable: {exc}"
        ) from exc


def _object(payload: bytes, *, maximum: int, label: str) -> dict[str, object]:
    if not payload or len(payload) > maximum:
        _fail(f"{label} is empty or exceeds its {maximum}-byte bound")
    try:
        value = json.loads(payload, object_pairs_hook=_unique_object)
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as exc:
        raise PrivacyActionDriverIpcError(f"{label} is not JSON") from exc
    if not isinstance(value, dict) or _canonical(value) != payload:
        _fail(f"{label} is not one canonical closed JSON object")
    return value


def _unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate action-driver field {key!r}")
        value[key] = item
    return value


def _exact(value: Mapping[str, object], fields: set[str], label: str) -> None:
    if set(value) != fields:
        _fail(f"{label} fields are not exact")


def _sha256(value: object, label: str, *, nonzero: bool = False) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
    return value


def _positive_integer(value: object, label: str, maximum: int) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value <= 0
        or value > maximum
    ):
        _fail(f"{label} must be an integer in 1..={maximum}")
    return value


def _request_body(
    *,
    asset_definition_id: str,
    candidate_binding_sha256: str,
    chain_id: str,
    creation_time_millis: int,
    genesis_hash_hex: str,
    nonce: int,
    ttl_millis: int,
    values: Sequence[int],
) -> dict[str, object]:
    if (
        not isinstance(asset_definition_id, str)
        or not asset_definition_id
        or not asset_definition_id.isascii()
        or len(asset_definition_id) > MAX_ASSET_DEFINITION_ID_BYTES
    ):
        _fail("action-driver asset definition ID is not bounded ASCII")
    if (
        not isinstance(chain_id, str)
        or not chain_id
        or not chain_id.isascii()
        or len(chain_id) > MAX_CHAIN_ID_BYTES
    ):
        _fail("action-driver chain ID is not bounded ASCII")
    _sha256(candidate_binding_sha256, "candidate binding", nonzero=True)
    _sha256(genesis_hash_hex, "genesis hash", nonzero=True)
    _positive_integer(
        creation_time_millis, "creation time", MAX_CREATION_TIME_MILLIS
    )
    _positive_integer(nonce, "nonce", 2**32 - 1)
    _positive_integer(ttl_millis, "TTL", MAX_TTL_MILLIS)
    if (
        isinstance(values, (str, bytes))
        or not isinstance(values, Sequence)
        or not 1 <= len(values) <= 8
        or any(
            isinstance(value, bool)
            or not isinstance(value, int)
            or not 0 <= value <= 2**32 - 1
            for value in values
        )
    ):
        _fail("action-driver VeRange values must be 1..=8 unsigned 32-bit integers")
    return {
        "asset_definition_id": asset_definition_id,
        "candidate_binding_sha256": candidate_binding_sha256,
        "chain_id": chain_id,
        "creation_time_millis": creation_time_millis,
        "genesis_hash_hex": genesis_hash_hex,
        "nonce": nonce,
        "operation": OPERATION,
        "schema": REQUEST_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "ttl_millis": ttl_millis,
        "values": list(values),
    }


def build_verange_request(
    *,
    asset_definition_id: str,
    candidate_binding_sha256: str,
    chain_id: str,
    creation_time_millis: int,
    genesis_hash_hex: str,
    nonce: int,
    ttl_millis: int,
    values: Sequence[int],
) -> bytes:
    """Build one canonical request without accepting secret or network material."""

    body = _request_body(
        asset_definition_id=asset_definition_id,
        candidate_binding_sha256=candidate_binding_sha256,
        chain_id=chain_id,
        creation_time_millis=creation_time_millis,
        genesis_hash_hex=genesis_hash_hex,
        nonce=nonce,
        ttl_millis=ttl_millis,
        values=values,
    )
    digest = hashlib.sha256(REQUEST_ID_DOMAIN + _canonical(body)[:-1]).hexdigest()
    payload = _canonical({**body, "request_id": digest})
    if len(payload) > MAX_REQUEST_BYTES:
        _fail("action-driver request exceeds its canonical byte bound")
    return payload


def validate_request(payload: bytes) -> dict[str, object]:
    """Parse and rederive one request before it reaches the native driver."""

    request = _object(payload, maximum=MAX_REQUEST_BYTES, label="action-driver request")
    _exact(
        request,
        {
            "asset_definition_id",
            "candidate_binding_sha256",
            "chain_id",
            "creation_time_millis",
            "genesis_hash_hex",
            "nonce",
            "operation",
            "request_id",
            "schema",
            "schema_version",
            "ttl_millis",
            "values",
        },
        "action-driver request",
    )
    if (
        request["operation"] != OPERATION
        or request["schema"] != REQUEST_SCHEMA
        or request["schema_version"] != SCHEMA_VERSION
    ):
        _fail("action-driver request selects an unsupported contract")
    body = _request_body(
        asset_definition_id=request["asset_definition_id"],
        candidate_binding_sha256=request["candidate_binding_sha256"],
        chain_id=request["chain_id"],
        creation_time_millis=request["creation_time_millis"],
        genesis_hash_hex=request["genesis_hash_hex"],
        nonce=request["nonce"],
        ttl_millis=request["ttl_millis"],
        values=request["values"],
    )
    expected_id = hashlib.sha256(
        REQUEST_ID_DOMAIN + _canonical(body)[:-1]
    ).hexdigest()
    if _sha256(request["request_id"], "request ID", nonzero=True) != expected_id:
        _fail("action-driver request ID is not derived from the canonical body")
    return request


def _decode_hex_bytes(value: object, label: str, maximum: int) -> bytes:
    if (
        not isinstance(value, str)
        or not value
        or len(value) % 2
        or len(value) > maximum * 2
        or any(byte not in "0123456789abcdef" for byte in value)
    ):
        _fail(f"{label} is not bounded canonical lowercase hexadecimal")
    decoded = bytes.fromhex(value)
    if not decoded or decoded.hex() != value:
        _fail(f"{label} is empty or noncanonical")
    return decoded


def validate_response(
    payload: bytes,
    *,
    expected_request: Mapping[str, object],
) -> dict[str, object]:
    """Parse the native response and independently hash its action bytes."""

    expected = validate_request(_canonical(dict(expected_request)))

    response = _object(
        payload, maximum=MAX_RESPONSE_BYTES, label="action-driver response"
    )
    _exact(
        response,
        {
            "candidate_binding_sha256",
            "operation",
            "protocol",
            "request_id",
            "schema",
            "schema_version",
            "transaction_hash_hex",
            "transaction_norito_hex",
            "transaction_sha256",
        },
        "action-driver response",
    )
    if (
        response["schema"] != RESPONSE_SCHEMA
        or response["schema_version"] != SCHEMA_VERSION
        or response["operation"] != OPERATION
        or response["protocol"] != PROTOCOL
        or response["request_id"] != expected["request_id"]
        or response["candidate_binding_sha256"]
        != expected["candidate_binding_sha256"]
    ):
        _fail("action-driver response context differs from its exact request")
    transaction_hash = _sha256(
        response["transaction_hash_hex"], "Iroha transaction hash", nonzero=True
    )
    transaction = _decode_hex_bytes(
        response["transaction_norito_hex"],
        "proof-bearing transaction",
        MAX_TRANSACTION_BYTES,
    )
    if hashlib.sha256(transaction).hexdigest() != _sha256(
        response["transaction_sha256"], "transaction digest", nonzero=True
    ):
        _fail("action-driver transaction digest differs from its bytes")
    return {
        "protocol": PROTOCOL,
        "request_id": response["request_id"],
        "transaction_hash_hex": transaction_hash,
        "transaction_norito": transaction,
        "transaction_sha256": response["transaction_sha256"],
    }
