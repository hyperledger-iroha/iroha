"""Strict native-backed SoraFS orderbook submission helpers."""

from __future__ import annotations

import json
import re
from typing import Any, Dict, Mapping

ORDERBOOK_TRANSACTION_MAX_BYTES_V1 = 2 * 1024 * 1024
ORDERBOOK_RECEIPT_MAX_BYTES_V1 = 1024 * 1024
_HASH_HEX = re.compile(r"[0-9a-f]{64}")
_IDENTITY_KEYS = frozenset({"tx_hash", "entrypoint_hash", "signed_transaction_hash"})
_RECEIPT_KEYS = frozenset({"payload", "signature"})
_PAYLOAD_KEYS = frozenset(
    {
        "tx_hash",
        "entrypoint_hash",
        "signed_transaction_hash",
        "submitted_at_ms",
        "submitted_at_height",
        "signer",
    }
)
_FIXED_HEADERS = frozenset({"accept", "accept-encoding", "content-type", "prefer"})


class SorafsOrderbookSubmissionAmbiguousError(RuntimeError):
    """Dispatch began, so callers must reconcile identity and never resubmit blindly."""

    def __init__(self, route: str, expected_identity: Mapping[str, str]) -> None:
        self.route = route
        self.expected_identity = dict(expected_identity)
        super().__init__(
            "SoraFS orderbook submission outcome is ambiguous after dispatch; "
            "do not resubmit automatically, reconcile the expected transaction identity"
        )


def _require_native_function(native: Any, name: str) -> Any:
    function = getattr(native, name, None)
    if not callable(function):
        raise RuntimeError(
            f"native verifier is missing {name}; install/rebuild iroha_python for this SDK version"
        )
    return function


def _require_exact_mapping(value: Any, keys: frozenset[str], context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or set(value) != keys:
        raise RuntimeError(f"{context} must contain exactly {', '.join(sorted(keys))}")
    return value


def _require_hash_hex(value: Any, context: str) -> str:
    if not isinstance(value, str) or _HASH_HEX.fullmatch(value) is None:
        raise RuntimeError(f"{context} must be exactly 32 lowercase hexadecimal bytes")
    return value


def validate_fixed_request_headers(
    headers: Mapping[str, str] | None,
    *,
    context: str,
    allow_default_json_accept: bool = False,
) -> Dict[str, str]:
    result: Dict[str, str] = {}
    if headers is None:
        return result
    if not isinstance(headers, Mapping):
        raise TypeError(f"{context}.headers must be a mapping")
    for raw_name, raw_value in headers.items():
        name, value = str(raw_name), str(raw_value)
        lowered = name.lower()
        if lowered in _FIXED_HEADERS:
            if allow_default_json_accept and lowered == "accept" and value == "application/json":
                continue
            raise ValueError(f"{context}.headers must not override {name}")
        result[name] = value
    return result


def prepare_orderbook_submission(
    *,
    native: Any,
    route: str,
    signed_transaction: Any,
    expected_network_id: Any,
    expected_receipt_signer: Any,
    context: str,
) -> tuple[bytes, Dict[str, str]]:
    inspect = _require_native_function(native, "inspect_sorafs_orderbook_submission_v1")
    _require_native_function(native, "verify_sorafs_orderbook_submission_receipt_v1")
    if not isinstance(signed_transaction, (bytes, bytearray, memoryview)):
        raise TypeError(f"{context}.signed_transaction must be bytes-like")
    body = bytes(signed_transaction)
    if not body or len(body) > ORDERBOOK_TRANSACTION_MAX_BYTES_V1:
        raise ValueError(
            f"{context}.signed_transaction must contain 1..{ORDERBOOK_TRANSACTION_MAX_BYTES_V1} bytes"
        )
    if expected_network_id is None:
        raise ValueError(f"{context}.expected_network_id is required")
    if not isinstance(expected_receipt_signer, str) or not expected_receipt_signer:
        raise ValueError(f"{context}.expected_receipt_signer is required")
    identity = _require_exact_mapping(
        inspect(route, expected_network_id, expected_receipt_signer, body),
        _IDENTITY_KEYS,
        "native orderbook submission identity",
    )
    return body, {
        key: _require_hash_hex(identity[key], f"native orderbook submission identity.{key}")
        for key in _IDENTITY_KEYS
    }


def response_header(response: Any, name: str, context: str) -> str | None:
    raw = getattr(response, "raw", None)
    raw_headers = getattr(raw, "headers", None)
    getlist = getattr(raw_headers, "getlist", None)
    if callable(getlist):
        values = getlist(name)
        if len(values) > 1:
            raise RuntimeError(f"{context} response contains duplicate {name} headers")
    value = response.headers.get(name)
    if isinstance(value, str) and "," in value:
        raise RuntimeError(f"{context} response contains a coalesced {name} header")
    return value


def validate_response_headers(response: Any, identity: Mapping[str, str], context: str) -> None:
    content_type = response_header(response, "Content-Type", context)
    if content_type != "application/x-norito":
        raise RuntimeError(f"{context} response Content-Type must be exactly application/x-norito")
    content_encoding = response_header(response, "Content-Encoding", context)
    if content_encoding not in (None, "identity"):
        raise RuntimeError(f"{context} response Content-Encoding must be absent or identity")
    for header, key in (
        ("x-iroha-transaction-hash", "tx_hash"),
        ("x-iroha-entrypoint-hash", "entrypoint_hash"),
        ("x-iroha-signed-transaction-hash", "signed_transaction_hash"),
    ):
        value = response_header(response, header, context)
        if value is None or _HASH_HEX.fullmatch(value) is None:
            raise RuntimeError(f"{context} response {header} must be one lowercase 32-byte hash")
        if value != identity[key]:
            raise RuntimeError(f"{context} response {header} does not match the submitted transaction")


def verify_receipt(
    *,
    native: Any,
    receipt_norito: bytes,
    identity: Mapping[str, str],
    expected_receipt_signer: str,
    context: str,
) -> Dict[str, Any]:
    if not receipt_norito:
        raise RuntimeError(f"{context} response returned an empty receipt")
    verify = _require_native_function(native, "verify_sorafs_orderbook_submission_receipt_v1")
    raw = verify(
        receipt_norito,
        identity["tx_hash"],
        identity["entrypoint_hash"],
        identity["signed_transaction_hash"],
        expected_receipt_signer,
    )
    if not isinstance(raw, str):
        raise RuntimeError("native orderbook receipt verifier returned non-text JSON")
    try:
        receipt = json.loads(raw)
    except (TypeError, ValueError) as error:
        raise RuntimeError("native orderbook receipt verifier returned invalid JSON") from error
    receipt = _require_exact_mapping(receipt, _RECEIPT_KEYS, "verified orderbook receipt")
    payload = _require_exact_mapping(
        receipt["payload"], _PAYLOAD_KEYS, "verified orderbook receipt.payload"
    )
    if payload["signer"] != expected_receipt_signer:
        raise RuntimeError("verified orderbook receipt signer changed at the native boundary")
    return dict(receipt)
