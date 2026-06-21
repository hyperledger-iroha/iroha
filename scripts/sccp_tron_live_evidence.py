#!/usr/bin/env python3
"""Collect read-only SCCP TRON deployment evidence from a TRON HTTP API.

The helper never signs, broadcasts, deploys, or mutates chain state. It queries
constant contract views with `/wallet/triggerconstantcontract`, optionally reads
contract metadata with `/wallet/getcontract`, recomputes SCCP production hashes,
and prints JSON evidence plus the matching arguments for
`scripts/sccp_tron_source_bridge_evidence.py` and Torii SCCP artifact/job
destination query fields.

Prerequisites:
- A TRON full-node or TronGrid-compatible HTTP API URL.
- Deployed `SccpTronSourceBridge` and/or
  `SccpTronGroth16Bn254MessageVerifier` addresses.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path
from types import SimpleNamespace
from typing import Any, Callable


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import sccp_tron_source_bridge_evidence as evidence  # noqa: E402
from sccp_client_loader import load_sccp_module  # noqa: E402


sccp_client = load_sccp_module()


Urlopen = Callable[..., Any]

TRON_SOURCE_EVENT_ABI = b"SccpSourceEvent(bytes32)"
TRON_SOURCE_EVENT_TOPIC = evidence._keccak_256(TRON_SOURCE_EVENT_ABI)
TRON_MESSAGE_PROOF_ACCEPTED_ABI = (
    b"MessageProofAccepted(bytes32,uint32,bytes32,bytes32,bytes32,bytes32,bytes32,bytes32)"
)
TRON_MESSAGE_PROOF_ACCEPTED_TOPIC = evidence._keccak_256(
    TRON_MESSAGE_PROOF_ACCEPTED_ABI
)
TRON_ROUTE_CANARY_EVIDENCE_LABEL = b"iroha:sccp:tron-route-canary-evidence:v3"
TRON_SUBMIT_MESSAGE_PROOF_SELECTOR = bytes.fromhex("bd57826c")
TRON_GROTH16_PROOF_VERSION = 1
TRON_GROTH16_PROOF_ABI_BYTE_LENGTH = 32 * 12
TRON_TRIGGER_SMART_CONTRACT_TYPE_URL = (
    b"type.googleapis.com/protocol.TriggerSmartContract"
)
SECP256K1_FIELD_MODULUS = int(
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
    16,
)
SECP256K1_SCALAR_ORDER = int(
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
    16,
)
SECP256K1_SCALAR_HALF_ORDER = int(
    "7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0",
    16,
)
PROTOBUF_INT64_MAX = 0x7FFFFFFFFFFFFFFF
SECP256K1_GENERATOR = (
    int("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", 16),
    int("483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8", 16),
)
TRON_TRANSACTION_RET_CODES = {
    "SUCESS": 0,
    "FAILED": 1,
}
TRON_TRANSACTION_CONTRACT_RESULTS = {
    "DEFAULT": 0,
    "SUCCESS": 1,
    "REVERT": 2,
    "BAD_JUMP_DESTINATION": 3,
    "OUT_OF_MEMORY": 4,
    "PRECOMPILED_CONTRACT": 5,
    "STACK_TOO_SMALL": 6,
    "STACK_TOO_LARGE": 7,
    "ILLEGAL_OPERATION": 8,
    "STACK_OVERFLOW": 9,
    "OUT_OF_ENERGY": 10,
    "OUT_OF_TIME": 11,
    "JVM_STACK_OVER_FLOW": 12,
    "UNKNOWN": 13,
    "TRANSFER_FAILED": 14,
    "INVALID_CODE": 15,
}
TRON_MAX_SOLID_BLOCK_EXTRA_HEADERS = 64
TRON_API_MAX_RESPONSE_BYTES = 1024 * 1024
TRON_API_MAX_ERROR_BYTES = 4096
TRON_SAFE_FIELD_NAME_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_"
)
TRON_SENSITIVE_FIELD_NAME_MARKERS = (
    "secret-token",
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer",
    "authorization",
    "access-key",
    "access_key",
    "api-key",
    "api_key",
    "client-secret",
    "client_secret",
    "session",
    "token",
)


def _unsupported_tron_field_detail(field: Any) -> str:
    if not isinstance(field, str):
        return "non-string field name"
    lowered = field.lower()
    if any(marker in lowered for marker in TRON_SENSITIVE_FIELD_NAME_MARKERS):
        return "field with sensitive name"
    if (
        not field
        or not field.isascii()
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in field)
        or any(character not in TRON_SAFE_FIELD_NAME_CHARS for character in field)
    ):
        return "field with malformed name"
    return field


def _unsupported_tron_fields_message(label: str, fields: set[Any]) -> str:
    details = [
        _unsupported_tron_field_detail(field)
        for field in sorted(fields, key=lambda item: str(item))
    ]
    return f"{label} has unsupported fields: {', '.join(details)}"


def _strip_0x(value: str) -> str:
    return value[2:] if value.lower().startswith("0x") else value


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _parse_hex32(value: str, *, label: str) -> bytes:
    return evidence.parse_hex_bytes(value, label=label, byte_length=32)


def _parse_hex_blob(value: Any, *, label: str, nonzero: bool = True) -> bytes:
    return _parse_exact_hex_blob(value, label=label, nonzero=nonzero)


def _parse_exact_hex_blob(value: Any, *, label: str, nonzero: bool = True) -> bytes:
    if not isinstance(value, str):
        raise RuntimeError(f"{label} must be hex")
    if value != value.strip():
        raise RuntimeError(f"{label} must not contain surrounding whitespace")
    if value.startswith("0X"):
        raise RuntimeError(f"{label} must be canonical lowercase hex")
    text = value[2:] if value.startswith("0x") else value
    if any(symbol.isspace() for symbol in text):
        raise RuntimeError(f"{label} must not contain whitespace")
    if len(text) % 2 != 0:
        raise RuntimeError(f"{label} must contain an even number of hex digits")
    if any(symbol not in "0123456789abcdef" for symbol in text):
        raise RuntimeError(f"{label} must be canonical lowercase hex")
    parsed = bytes.fromhex(text)
    if nonzero and not any(parsed):
        raise RuntimeError(f"{label} must not be zero")
    return parsed


def _parse_exact_hex32(value: Any, *, label: str) -> bytes:
    return _parse_exact_hex32_blob(value, label=label)


def _parse_exact_hex32_blob(
    value: Any,
    *,
    label: str,
    nonzero: bool = True,
) -> bytes:
    parsed = _parse_exact_hex_blob(value, label=label, nonzero=False)
    if len(parsed) != 32:
        raise RuntimeError(f"{label} must be 32 bytes")
    if nonzero and not any(parsed):
        raise RuntimeError(f"{label} must not be zero")
    return parsed


def _parse_hex32_blob(value: Any, *, label: str, nonzero: bool = True) -> bytes:
    parsed = _parse_hex_blob(value, label=label, nonzero=nonzero)
    if len(parsed) != 32:
        raise RuntimeError(f"{label} must be 32 bytes")
    return parsed


def _parse_tron_payload_hex(value: Any, *, label: str) -> bytes:
    parsed = _parse_exact_hex_blob(value, label=label)
    if len(parsed) != 21 or parsed[0] != 0x41 or not any(parsed[1:]):
        raise RuntimeError(f"{label} must be a non-zero 0x41-prefixed TRON address")
    return parsed


def _protobuf_varint(value: int) -> bytes:
    if value < 0:
        raise ValueError("protobuf varint cannot encode negative values")
    out = bytearray()
    while value >= 0x80:
        out.append((value & 0x7F) | 0x80)
        value >>= 7
    out.append(value)
    return bytes(out)


def _protobuf_u64_field(field_number: int, value: int) -> bytes:
    if field_number <= 0 or value < 0 or value > 0xFFFFFFFFFFFFFFFF:
        raise ValueError("protobuf u64 field is out of range")
    return _protobuf_varint((field_number << 3) | 0) + _protobuf_varint(value)


def _protobuf_bytes_field(field_number: int, value: bytes) -> bytes:
    if field_number <= 0:
        raise ValueError("protobuf bytes field number is out of range")
    return _protobuf_varint((field_number << 3) | 2) + _protobuf_varint(len(value)) + value


def _protobuf_string_field(field_number: int, value: str) -> bytes:
    if not isinstance(value, str):
        raise RuntimeError("protobuf string field value must be a string")
    return _protobuf_bytes_field(field_number, value.encode("utf-8"))


def _read_protobuf_varint_at(data: bytes, cursor: int, *, label: str) -> tuple[int, int]:
    start = cursor
    value = 0
    shift = 0
    while cursor < len(data) and cursor - start < 10:
        byte = data[cursor]
        cursor += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            if value > 0xFFFFFFFFFFFFFFFF:
                raise RuntimeError(f"{label} protobuf varint exceeds u64")
            if _protobuf_varint(value) != data[start:cursor]:
                raise RuntimeError(f"{label} contains non-canonical protobuf varint")
            return value, cursor
        shift += 7
    raise RuntimeError(f"{label} contains truncated protobuf varint")


def _read_protobuf_bytes_field(
    data: bytes,
    cursor: int,
    *,
    label: str,
) -> tuple[bytes, int]:
    length, cursor = _read_protobuf_varint_at(data, cursor, label=label)
    end = cursor + length
    if end > len(data):
        raise RuntimeError(f"{label} contains truncated protobuf bytes field")
    return data[cursor:end], end


def _optional_hex32_arg(args: argparse.Namespace, name: str) -> bytes | None:
    value = getattr(args, name, None)
    if value is None:
        return None
    label = name.replace("_", " ")
    if isinstance(value, (bytes, bytearray)):
        return _parse_hex32(_hex(bytes(value)), label=label)
    return _parse_exact_hex32(str(value), label=label)


def _optional_hex_blob_arg(
    args: argparse.Namespace,
    name: str,
    *,
    nonzero: bool = True,
) -> bytes | None:
    value = getattr(args, name, None)
    if value is None:
        return None
    if isinstance(value, (bytes, bytearray)):
        value = _hex(bytes(value))
    return _parse_exact_hex_blob(
        str(value),
        label=name.replace("_", " "),
        nonzero=nonzero,
    )


def _base58check_encode(payload: bytes) -> str:
    checksum = hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    raw = payload + checksum
    numeric = int.from_bytes(raw, "big")
    encoded = ""
    while numeric:
        numeric, digit = divmod(numeric, 58)
        encoded = evidence.BASE58_ALPHABET[digit] + encoded
    leading_zeroes = len(raw) - len(raw.lstrip(b"\x00"))
    return ("1" * leading_zeroes) + (encoded or "1")


def tron_base58check_from_payload(payload: bytes) -> str:
    """Return a checksummed TRON base58 address from a 21-byte 0x41 payload."""

    if len(payload) != 21 or payload[0] != 0x41 or not any(payload[1:]):
        raise ValueError("TRON payload must be a non-zero 21-byte 0x41 address")
    return _base58check_encode(payload)


def tron_base58check_from_address20(address: bytes) -> str:
    """Return a checksummed TRON base58 address from trailing 20 address bytes."""

    if len(address) != 20 or not any(address):
        raise ValueError("TRON address must be a non-zero 20-byte value")
    return tron_base58check_from_payload(b"\x41" + address)


def parse_tron_address_payload(value: str, *, label: str) -> bytes:
    """Parse base58check, 0x41-prefixed hex, or 20-byte hex into a 21-byte payload."""

    if not isinstance(value, str) or not value:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain surrounding whitespace")
    text = value
    hex_text = _strip_0x(text)
    if len(hex_text) in {40, 42}:
        address20 = evidence.parse_tron_address(text, label=label)
        return b"\x41" + address20
    return evidence.parse_tron_base58check_payload(text, label=label)


def _is_nonzero_tron_address_payload(payload: bytes) -> bool:
    return len(payload) == 21 and payload[0] == 0x41 and any(payload[1:])


def _url(base_url: str, endpoint: str) -> str:
    return base_url.rstrip("/") + "/" + endpoint.lstrip("/")


def _tron_pro_api_key_token(value: Any, *, label: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{label} must be text")
    if not value:
        raise ValueError(f"{label} must not be empty")
    if value != value.strip() or any(ch.isspace() for ch in value):
        raise ValueError(f"{label} must not contain whitespace")
    try:
        value.encode("ascii")
    except UnicodeEncodeError:
        raise ValueError(f"{label} must be ASCII") from None
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise ValueError(f"{label} must not contain control characters")
    return value


def _json_object_without_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for key, value in pairs:
        if key in out:
            raise ValueError("duplicate JSON keys")
        out[key] = value
    return out


def _post_json(
    base_url: str,
    endpoint: str,
    payload: dict[str, Any],
    *,
    tron_pro_api_key: str | None = None,
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    headers = {
        "Accept": "application/json",
        "Content-Type": "application/json",
    }
    if tron_pro_api_key is not None:
        headers["TRON-PRO-API-KEY"] = _tron_pro_api_key_token(
            tron_pro_api_key,
            label="TRON-PRO-API-KEY",
        )
    request = urllib.request.Request(
        _url(base_url, endpoint),
        data=json.dumps(payload, separators=(",", ":")).encode("utf-8"),
        headers=headers,
        method="POST",
    )
    try:
        with opener(request, timeout=timeout) as response:
            raw = response.read(TRON_API_MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as exc:
        raise RuntimeError(
            f"TRON API {endpoint} failed with HTTP {exc.code}"
        ) from None
    except urllib.error.URLError:
        raise RuntimeError(f"TRON API {endpoint} request failed") from None
    if len(raw) > TRON_API_MAX_RESPONSE_BYTES:
        raise RuntimeError(
            f"TRON API {endpoint} response exceeds "
            f"{TRON_API_MAX_RESPONSE_BYTES} bytes"
        )
    try:
        decoded = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_json_object_without_duplicate_keys,
        )
    except (UnicodeDecodeError, json.JSONDecodeError):
        raise RuntimeError(f"TRON API {endpoint} returned invalid JSON") from None
    except ValueError as exc:
        if str(exc) == "duplicate JSON keys":
            raise RuntimeError(f"TRON API {endpoint} returned duplicate JSON keys") from None
        raise RuntimeError(f"TRON API {endpoint} returned invalid JSON") from None
    if not isinstance(decoded, dict):
        raise RuntimeError(f"TRON API {endpoint} returned a non-object response")
    if decoded.get("Error") is not None or decoded.get("error") is not None:
        raise RuntimeError(f"TRON API {endpoint} returned error response")
    return decoded


def _transaction_info(
    base_url: str,
    *,
    endpoint: str,
    transaction_id: bytes,
    tron_pro_api_key: str | None,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    return _post_json(
        base_url,
        endpoint,
        {"value": transaction_id.hex()},
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )


def _transaction_by_id(
    base_url: str,
    *,
    endpoint: str,
    transaction_id: bytes,
    tron_pro_api_key: str | None,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    return _post_json(
        base_url,
        endpoint,
        {"value": transaction_id.hex()},
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )


def _block_by_number(
    base_url: str,
    *,
    endpoint: str,
    block_number: int,
    tron_pro_api_key: str | None,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    if block_number <= 0:
        raise RuntimeError("source-event block number must be positive")
    return _post_json(
        base_url,
        endpoint,
        {"num": block_number},
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )


def _decode_tron_error_message(value: Any) -> str:
    if not isinstance(value, str):
        return ""
    text = _strip_0x(value)
    if len(text) % 2 == 0:
        try:
            return bytes.fromhex(text).decode("utf-8", "replace")
        except ValueError:
            pass
    return value


def _constant_word(
    base_url: str,
    *,
    endpoint: str,
    contract_address: str,
    function_selector: str,
    parameter: str = "",
    owner_address: str,
    tron_pro_api_key: str | None,
    opener: Urlopen,
    timeout: float,
) -> bytes:
    response = _post_json(
        base_url,
        endpoint,
        {
            "owner_address": owner_address,
            "contract_address": contract_address,
            "function_selector": function_selector,
            "parameter": parameter,
            "visible": True,
        },
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    result = response.get("result")
    if not isinstance(result, dict) or result.get("result") is not True:
        raise RuntimeError(f"TRON constant call {function_selector} failed")
    values = response.get("constant_result")
    if not isinstance(values, list) or len(values) != 1 or not isinstance(values[0], str):
        raise RuntimeError(f"TRON constant call {function_selector} returned no single word")
    try:
        word = _parse_exact_hex_blob(
            values[0],
            label=f"TRON constant call {function_selector} ABI word",
            nonzero=False,
        )
    except RuntimeError:
        raise RuntimeError(
            f"TRON constant call {function_selector} returned non-hex data"
        ) from None
    if len(word) != 32:
        raise RuntimeError(f"TRON constant call {function_selector} must return one ABI word")
    return word


def _word_u32(word: bytes, *, label: str) -> int:
    value = int.from_bytes(word, "big")
    if value > 0xFFFFFFFF:
        raise RuntimeError(f"{label} does not fit u32")
    return value


def _word_bool(word: bytes, *, label: str) -> bool:
    value = int.from_bytes(word, "big")
    if value not in (0, 1):
        raise RuntimeError(f"{label} must be an ABI-encoded bool")
    return value == 1


def _word_address20(word: bytes, *, label: str) -> bytes:
    if len(word) != 32 or any(word[:12]):
        raise RuntimeError(f"{label} must be an ABI-encoded address")
    address = word[12:]
    if not any(address):
        raise RuntimeError(f"{label} must not be zero")
    return address


def _parse_transaction_info_id(
    response: dict[str, Any],
    *,
    expected_transaction_id: bytes,
    label: str = "TRON transaction info",
) -> str:
    transaction_ids: dict[str, bytes] = {}
    for field in ("id", "txID", "txid"):
        if field not in response:
            continue
        raw_id = response[field]
        if not isinstance(raw_id, str):
            raise RuntimeError(f"{label} {field} must be a transaction id")
        transaction_ids[field] = _parse_exact_hex32(
            raw_id,
            label=f"{label} {field}",
        )
    if "id" not in transaction_ids:
        raise RuntimeError(f"{label} did not return id")
    if len(set(transaction_ids.values())) != 1:
        raise RuntimeError(f"{label} returned conflicting transaction id aliases")
    transaction_id = transaction_ids["id"]
    if transaction_id != expected_transaction_id:
        raise RuntimeError(f"{label} id does not match requested id")
    return _hex(transaction_id)


def _parse_transaction_id_field(
    response: dict[str, Any],
    *,
    expected_transaction_id: bytes,
    label: str = "TRON transaction",
) -> str:
    transaction_id = _parse_required_transaction_id_aliases(
        response,
        required_field="txID",
        label=label,
    )
    if transaction_id != expected_transaction_id:
        raise RuntimeError(f"{label} txID does not match requested id")
    return _hex(transaction_id)


def _parse_required_transaction_id_aliases(
    response: dict[str, Any],
    *,
    required_field: str,
    label: str,
) -> bytes:
    transaction_ids: dict[str, bytes] = {}
    for field in ("txID", "txid", "id"):
        if field not in response:
            continue
        raw_id = response[field]
        if not isinstance(raw_id, str):
            raise RuntimeError(f"{label} {field} must be a transaction id")
        transaction_ids[field] = _parse_exact_hex32(
            raw_id,
            label=f"{label} {field}",
        )
    if required_field not in transaction_ids:
        raise RuntimeError(f"{label} did not return {required_field}")
    if len(set(transaction_ids.values())) != 1:
        raise RuntimeError(f"{label} returned conflicting transaction id aliases")
    return transaction_ids[required_field]


def _parse_raw_data_tron_payload(value: bytes, *, label: str) -> bytes:
    if len(value) != 21 or value[0] != 0x41 or not any(value[1:]):
        raise RuntimeError(f"{label} must be a non-zero 21-byte TRON address payload")
    return value


def _source_event_trigger_from_raw_data_summary(
    trigger: bytes,
    *,
    source_bridge_payload: bytes,
    owner_payload: bytes,
    source_event_call_data: bytes,
) -> dict[str, Any]:
    cursor = 0
    owner = None
    contract_address = None
    call_data = None
    call_value_seen = False
    call_token_value_seen = False
    token_id_seen = False
    while cursor < len(trigger):
        key, cursor = _read_protobuf_varint_at(
            trigger,
            cursor,
            label="source-event transaction raw_data_hex TriggerSmartContract",
        )
        field_number = key >> 3
        wire_type = key & 0x07
        if field_number == 1 and wire_type == 2 and owner is None:
            raw_owner, cursor = _read_protobuf_bytes_field(
                trigger,
                cursor,
                label="source-event transaction raw_data_hex owner_address",
            )
            owner = _parse_raw_data_tron_payload(
                raw_owner,
                label="source-event transaction raw_data_hex owner_address",
            )
            if owner != owner_payload:
                raise RuntimeError(
                    "source-event transaction raw_data_hex owner_address does "
                    "not match source bridge owner"
                )
        elif field_number == 2 and wire_type == 2 and contract_address is None:
            raw_contract, cursor = _read_protobuf_bytes_field(
                trigger,
                cursor,
                label="source-event transaction raw_data_hex contract_address",
            )
            contract_address = _parse_raw_data_tron_payload(
                raw_contract,
                label="source-event transaction raw_data_hex contract_address",
            )
            if contract_address != source_bridge_payload:
                raise RuntimeError(
                    "source-event transaction raw_data_hex contract_address "
                    "does not match source bridge"
                )
        elif field_number == 3 and wire_type == 0 and not call_value_seen:
            call_value_seen = True
            value, cursor = _read_protobuf_varint_at(
                trigger,
                cursor,
                label="source-event transaction raw_data_hex call_value",
            )
            if value != 0:
                raise RuntimeError(
                    "source-event transaction raw_data_hex call_value must be zero"
                )
        elif field_number == 4 and wire_type == 2 and call_data is None:
            call_data, cursor = _read_protobuf_bytes_field(
                trigger,
                cursor,
                label="source-event transaction raw_data_hex data",
            )
            if call_data != source_event_call_data:
                raise RuntimeError(
                    "source-event transaction raw_data_hex calldata does not "
                    "match source-event digest"
                )
        elif field_number == 5 and wire_type == 0 and not call_token_value_seen:
            call_token_value_seen = True
            value, cursor = _read_protobuf_varint_at(
                trigger,
                cursor,
                label="source-event transaction raw_data_hex call_token_value",
            )
            if value != 0:
                raise RuntimeError(
                    "source-event transaction raw_data_hex call_token_value must be zero"
                )
        elif field_number == 6 and wire_type == 0 and not token_id_seen:
            token_id_seen = True
            value, cursor = _read_protobuf_varint_at(
                trigger,
                cursor,
                label="source-event transaction raw_data_hex token_id",
            )
            if value != 0:
                raise RuntimeError(
                    "source-event transaction raw_data_hex token_id must be zero"
                )
        else:
            raise RuntimeError(
                "source-event transaction raw_data_hex TriggerSmartContract "
                "contains unsupported field"
            )
    if owner is None or contract_address is None or call_data is None:
        raise RuntimeError(
            "source-event transaction raw_data_hex TriggerSmartContract is incomplete"
        )
    return {
        "raw_data_type_url": TRON_TRIGGER_SMART_CONTRACT_TYPE_URL.decode("ascii"),
        "raw_data_owner_address": _hex(owner),
        "raw_data_owner_base58": tron_base58check_from_payload(owner),
        "raw_data_contract_address": _hex(contract_address),
        "raw_data_contract_base58": tron_base58check_from_payload(contract_address),
        "raw_data_call_data": _hex(call_data),
    }


def _source_event_any_from_raw_data_summary(
    parameter: bytes,
    *,
    source_bridge_payload: bytes,
    owner_payload: bytes,
    source_event_call_data: bytes,
) -> dict[str, Any]:
    cursor = 0
    type_url = None
    value = None
    while cursor < len(parameter):
        key, cursor = _read_protobuf_varint_at(
            parameter,
            cursor,
            label="source-event transaction raw_data_hex Any",
        )
        field_number = key >> 3
        wire_type = key & 0x07
        if field_number == 1 and wire_type == 2 and type_url is None:
            type_url, cursor = _read_protobuf_bytes_field(
                parameter,
                cursor,
                label="source-event transaction raw_data_hex Any type_url",
            )
        elif field_number == 2 and wire_type == 2 and value is None:
            value, cursor = _read_protobuf_bytes_field(
                parameter,
                cursor,
                label="source-event transaction raw_data_hex Any value",
            )
        else:
            raise RuntimeError(
                "source-event transaction raw_data_hex Any contains unsupported field"
            )
    if type_url != TRON_TRIGGER_SMART_CONTRACT_TYPE_URL or value is None:
        raise RuntimeError("source-event transaction raw_data_hex Any type_url mismatch")
    return _source_event_trigger_from_raw_data_summary(
        value,
        source_bridge_payload=source_bridge_payload,
        owner_payload=owner_payload,
        source_event_call_data=source_event_call_data,
    )


def _source_event_contract_from_raw_data_summary(
    contract: bytes,
    *,
    source_bridge_payload: bytes,
    owner_payload: bytes,
    source_event_call_data: bytes,
) -> dict[str, Any]:
    cursor = 0
    contract_type = None
    parameter = None
    while cursor < len(contract):
        key, cursor = _read_protobuf_varint_at(
            contract,
            cursor,
            label="source-event transaction raw_data_hex Contract",
        )
        field_number = key >> 3
        wire_type = key & 0x07
        if field_number == 1 and wire_type == 0 and contract_type is None:
            contract_type, cursor = _read_protobuf_varint_at(
                contract,
                cursor,
                label="source-event transaction raw_data_hex Contract type",
            )
        elif field_number == 2 and wire_type == 2 and parameter is None:
            parameter, cursor = _read_protobuf_bytes_field(
                contract,
                cursor,
                label="source-event transaction raw_data_hex Contract parameter",
            )
        else:
            raise RuntimeError(
                "source-event transaction raw_data_hex Contract contains unsupported field"
            )
    if contract_type != 31 or parameter is None:
        raise RuntimeError(
            "source-event transaction raw_data_hex contract must be TriggerSmartContract"
        )
    return _source_event_any_from_raw_data_summary(
        parameter,
        source_bridge_payload=source_bridge_payload,
        owner_payload=owner_payload,
        source_event_call_data=source_event_call_data,
    )


def _source_event_raw_data_call_summary(
    raw_data: bytes,
    *,
    source_bridge_payload: bytes,
    owner_payload: bytes,
    source_event_call_data: bytes,
) -> dict[str, Any]:
    cursor = 0
    ref_block_bytes = None
    ref_block_num_seen = False
    ref_block_hash = None
    expiration = None
    timestamp = None
    fee_limit = None
    contract_count = 0
    contract_summary = None
    while cursor < len(raw_data):
        key, cursor = _read_protobuf_varint_at(
            raw_data,
            cursor,
            label="source-event transaction raw_data_hex",
        )
        field_number = key >> 3
        wire_type = key & 0x07
        if field_number == 1 and wire_type == 2 and ref_block_bytes is None:
            ref_block_bytes, cursor = _read_protobuf_bytes_field(
                raw_data,
                cursor,
                label="source-event transaction raw_data_hex ref_block_bytes",
            )
            if len(ref_block_bytes) != 2 or not any(ref_block_bytes):
                raise RuntimeError(
                    "source-event transaction raw_data_hex ref_block_bytes "
                    "must be non-zero 2-byte data"
                )
        elif field_number == 3 and wire_type == 0 and not ref_block_num_seen:
            ref_block_num_seen = True
            _, cursor = _read_protobuf_varint_at(
                raw_data,
                cursor,
                label="source-event transaction raw_data_hex ref_block_num",
            )
        elif field_number == 4 and wire_type == 2 and ref_block_hash is None:
            ref_block_hash, cursor = _read_protobuf_bytes_field(
                raw_data,
                cursor,
                label="source-event transaction raw_data_hex ref_block_hash",
            )
            if len(ref_block_hash) != 8 or not any(ref_block_hash):
                raise RuntimeError(
                    "source-event transaction raw_data_hex ref_block_hash "
                    "must be non-zero 8-byte data"
                )
        elif field_number == 8 and wire_type == 0 and expiration is None:
            expiration, cursor = _read_protobuf_varint_at(
                raw_data,
                cursor,
                label="source-event transaction raw_data_hex expiration",
            )
            if expiration == 0:
                raise RuntimeError(
                    "source-event transaction raw_data_hex expiration must be non-zero"
                )
        elif field_number == 11 and wire_type == 2:
            contract_count += 1
            if contract_count > 1:
                raise RuntimeError(
                    "source-event transaction raw_data_hex must contain one contract"
                )
            contract, cursor = _read_protobuf_bytes_field(
                raw_data,
                cursor,
                label="source-event transaction raw_data_hex contract",
            )
            contract_summary = _source_event_contract_from_raw_data_summary(
                contract,
                source_bridge_payload=source_bridge_payload,
                owner_payload=owner_payload,
                source_event_call_data=source_event_call_data,
            )
        elif field_number == 14 and wire_type == 0 and timestamp is None:
            timestamp, cursor = _read_protobuf_varint_at(
                raw_data,
                cursor,
                label="source-event transaction raw_data_hex timestamp",
            )
            if timestamp == 0:
                raise RuntimeError(
                    "source-event transaction raw_data_hex timestamp must be non-zero"
                )
        elif field_number == 18 and wire_type == 0 and fee_limit is None:
            fee_limit, cursor = _read_protobuf_varint_at(
                raw_data,
                cursor,
                label="source-event transaction raw_data_hex fee_limit",
            )
            if fee_limit == 0:
                raise RuntimeError(
                    "source-event transaction raw_data_hex fee_limit must be non-zero"
                )
        else:
            raise RuntimeError(
                "source-event transaction raw_data_hex contains unsupported field"
            )
    if (
        ref_block_bytes is None
        or ref_block_hash is None
        or expiration is None
        or timestamp is None
        or fee_limit is None
        or contract_count != 1
        or contract_summary is None
    ):
        raise RuntimeError("source-event transaction raw_data_hex is incomplete")
    if expiration <= timestamp:
        raise RuntimeError(
            "source-event transaction raw_data_hex expiration must be after timestamp"
        )
    return {
        "raw_data_source_call_matches": True,
        "raw_data_ref_block_bytes": _hex(ref_block_bytes),
        "raw_data_ref_block_hash": _hex(ref_block_hash),
        "raw_data_expiration": expiration,
        "raw_data_timestamp": timestamp,
        "raw_data_fee_limit": fee_limit,
        **contract_summary,
    }


def _source_event_transaction_raw_data_summary(
    response: dict[str, Any],
    *,
    transaction_id: bytes,
    source_bridge_payload: bytes,
    owner_payload: bytes,
    source_event_call_data: bytes,
) -> dict[str, Any]:
    raw_data = _parse_exact_hex_blob(
        response.get("raw_data_hex"),
        label="source-event transaction raw_data_hex",
    )
    raw_data_hash = hashlib.sha256(raw_data).digest()
    if raw_data_hash != transaction_id:
        raise RuntimeError(
            "source-event transaction raw_data_hex SHA-256 does not match txID"
        )
    return {
        "raw_data_hex": _hex(raw_data),
        "raw_data_sha256": _hex(raw_data_hash),
        "transaction_id_matches_raw_data": True,
        **_source_event_raw_data_call_summary(
            raw_data,
            source_bridge_payload=source_bridge_payload,
            owner_payload=owner_payload,
            source_event_call_data=source_event_call_data,
        ),
    }


def _tron_recoverable_signature_is_canonical(signature: bytes) -> bool:
    if len(signature) != 65 or signature[64] not in (*range(0, 4), *range(27, 31)):
        return False
    r = int.from_bytes(signature[:32], "big")
    s = int.from_bytes(signature[32:64], "big")
    return 0 < r < SECP256K1_SCALAR_ORDER and 0 < s <= SECP256K1_SCALAR_HALF_ORDER


def _require_canonical_tron_recoverable_signature(
    signature: bytes,
    *,
    label: str,
) -> None:
    if not _tron_recoverable_signature_is_canonical(signature):
        raise RuntimeError(
            f"{label} must be a canonical 65-byte "
            "TRON recoverable secp256k1 signature"
        )


def _parse_tron_transaction_signature(response: dict[str, Any], *, label: str) -> bytes:
    signatures = response.get("signature")
    if not isinstance(signatures, list) or len(signatures) != 1:
        raise RuntimeError(f"{label} transaction must contain exactly one signature")
    signature = _parse_exact_hex_blob(
        signatures[0],
        label=f"{label} transaction signature",
    )
    _require_canonical_tron_recoverable_signature(
        signature,
        label=f"{label} transaction signature",
    )
    return signature


def _parse_source_event_transaction_signature(response: dict[str, Any]) -> bytes:
    return _parse_tron_transaction_signature(response, label="source-event")


def _parse_route_canary_transaction_signature(response: dict[str, Any]) -> bytes:
    return _parse_tron_transaction_signature(response, label="route-canary")


def _secp256k1_point_add(
    left: tuple[int, int] | None,
    right: tuple[int, int] | None,
) -> tuple[int, int] | None:
    if left is None:
        return right
    if right is None:
        return left
    x1, y1 = left
    x2, y2 = right
    if x1 == x2 and (y1 + y2) % SECP256K1_FIELD_MODULUS == 0:
        return None
    if left == right:
        slope = (
            (3 * x1 * x1)
            * pow((2 * y1) % SECP256K1_FIELD_MODULUS, -1, SECP256K1_FIELD_MODULUS)
        ) % SECP256K1_FIELD_MODULUS
    else:
        slope = (
            (y2 - y1)
            * pow((x2 - x1) % SECP256K1_FIELD_MODULUS, -1, SECP256K1_FIELD_MODULUS)
        ) % SECP256K1_FIELD_MODULUS
    x3 = (slope * slope - x1 - x2) % SECP256K1_FIELD_MODULUS
    y3 = (slope * (x1 - x3) - y1) % SECP256K1_FIELD_MODULUS
    return x3, y3


def _secp256k1_scalar_mul(
    scalar: int,
    point: tuple[int, int] | None,
) -> tuple[int, int] | None:
    result = None
    current = point
    while scalar:
        if scalar & 1:
            result = _secp256k1_point_add(result, current)
        current = _secp256k1_point_add(current, current)
        scalar >>= 1
    return result


def _tron_recovered_signature_address20(
    message_hash: bytes,
    signature: bytes,
) -> bytes | None:
    if len(message_hash) != 32 or not _tron_recoverable_signature_is_canonical(signature):
        return None
    r = int.from_bytes(signature[:32], "big")
    s = int.from_bytes(signature[32:64], "big")
    recovery_id = signature[64] - 27 if signature[64] >= 27 else signature[64]
    x = r + (recovery_id // 2) * SECP256K1_SCALAR_ORDER
    if x >= SECP256K1_FIELD_MODULUS:
        return None
    alpha = (pow(x, 3, SECP256K1_FIELD_MODULUS) + 7) % SECP256K1_FIELD_MODULUS
    y = pow(alpha, (SECP256K1_FIELD_MODULUS + 1) // 4, SECP256K1_FIELD_MODULUS)
    if (y * y) % SECP256K1_FIELD_MODULUS != alpha:
        return None
    if (y & 1) != (recovery_id & 1):
        y = SECP256K1_FIELD_MODULUS - y
    r_point = (x, y)
    if _secp256k1_scalar_mul(SECP256K1_SCALAR_ORDER, r_point) is not None:
        return None
    e = int.from_bytes(message_hash, "big") % SECP256K1_SCALAR_ORDER
    s_r = _secp256k1_scalar_mul(s, r_point)
    e_g = _secp256k1_scalar_mul(e, SECP256K1_GENERATOR)
    if s_r is None or e_g is None:
        return None
    q = _secp256k1_scalar_mul(
        pow(r, -1, SECP256K1_SCALAR_ORDER),
        _secp256k1_point_add(s_r, (e_g[0], (-e_g[1]) % SECP256K1_FIELD_MODULUS)),
    )
    if q is None:
        return None
    public_key = q[0].to_bytes(32, "big") + q[1].to_bytes(32, "big")
    return evidence._keccak_256(public_key)[-20:]


def _source_event_transaction_signature_summary(
    signature: bytes,
    *,
    raw_data_hash: bytes,
    owner_payload: bytes,
) -> dict[str, Any]:
    recovered_address20 = _tron_recovered_signature_address20(raw_data_hash, signature)
    if recovered_address20 is None or recovered_address20 != owner_payload[1:]:
        raise RuntimeError(
            "source-event transaction signature does not recover to source bridge owner"
        )
    recovered_payload = b"\x41" + recovered_address20
    return {
        "signature_count": 1,
        "signature": _hex(signature),
        "signature_sha256": _hex(hashlib.sha256(signature).digest()),
        "signature_recovery_id": signature[64],
        "signature_recovered_address": _hex(recovered_payload),
        "signature_recovered_base58": tron_base58check_from_payload(recovered_payload),
        "signature_recovers_to_owner": True,
    }


def _route_canary_transaction_signature_summary(
    signature: bytes,
    *,
    raw_data_hash: bytes,
    owner_payload: bytes,
) -> dict[str, Any]:
    recovered_address20 = _tron_recovered_signature_address20(raw_data_hash, signature)
    if recovered_address20 is None or recovered_address20 != owner_payload[1:]:
        raise RuntimeError(
            "route-canary transaction signature does not recover to transaction owner"
        )
    recovered_payload = b"\x41" + recovered_address20
    return {
        "signature_count": 1,
        "signature": _hex(signature),
        "signature_sha256": _hex(hashlib.sha256(signature).digest()),
        "signature_recovery_id": signature[64],
        "signature_recovered_address": _hex(recovered_payload),
        "signature_recovered_base58": tron_base58check_from_payload(recovered_payload),
        "signature_recovers_to_owner": True,
    }


def _is_canonical_decimal_text(value: str) -> bool:
    return value.isascii() and value.isdecimal() and (len(value) == 1 or value[0] != "0")


def _parse_canonical_u32(value: Any, *, label: str) -> int:
    if isinstance(value, bool):
        raise ValueError(f"{label} must be a u32")
    if isinstance(value, int):
        parsed = value
    elif isinstance(value, str) and _is_canonical_decimal_text(value):
        parsed = int(value)
    else:
        raise ValueError(f"{label} must be a u32")
    if parsed < 0 or parsed > 0xFFFFFFFF:
        raise ValueError(f"{label} must be a u32")
    return parsed


def _required_transaction_info_block_number(
    response: dict[str, Any],
    *,
    label: str,
) -> int:
    block_number = response.get("blockNumber")
    if type(block_number) is not int or block_number <= 0:
        raise RuntimeError(f"{label} blockNumber must be a positive integer")
    return block_number


def _required_transaction_info_block_timestamp(
    response: dict[str, Any],
    *,
    label: str,
) -> int:
    block_timestamp = response.get("blockTimeStamp")
    if type(block_timestamp) is not int or block_timestamp < 0:
        raise RuntimeError(f"{label} blockTimeStamp must be a non-negative integer")
    return block_timestamp


def _summary_block_metadata(
    summary: dict[str, Any],
    *,
    label: str,
) -> tuple[int, int]:
    block_number = summary.get("block_number")
    if type(block_number) is not int or block_number <= 0:
        raise RuntimeError(f"{label} block_number must be a positive integer")
    block_timestamp = summary.get("block_timestamp")
    if type(block_timestamp) is not int or block_timestamp < 0:
        raise RuntimeError(f"{label} block_timestamp must be a non-negative integer")
    return block_number, block_timestamp


def _check_optional_log_index_field(
    log: dict[str, Any],
    *,
    expected_index: int,
    label: str,
) -> None:
    has_camel = "logIndex" in log
    has_snake = "log_index" in log
    if has_camel and has_snake:
        raise RuntimeError(f"{label} must not include both logIndex and log_index")
    if not has_camel and not has_snake:
        return
    field = "logIndex" if has_camel else "log_index"
    log_index = _parse_canonical_u32(log[field], label=f"{label} {field}")
    if log_index != expected_index:
        raise RuntimeError(
            f"{label} {field} does not match log list index: "
            f"expected {expected_index}, got {log_index}"
        )


def _parse_protobuf_nonnegative_int(value: Any, *, label: str) -> int:
    if isinstance(value, bool):
        raise RuntimeError(f"{label} must fit non-negative int64")
    if isinstance(value, int):
        parsed = value
    elif isinstance(value, str) and _is_canonical_decimal_text(value):
        parsed = int(value)
    else:
        raise RuntimeError(f"{label} must fit non-negative int64")
    if parsed < 0 or parsed > PROTOBUF_INT64_MAX:
        raise RuntimeError(f"{label} must fit non-negative int64")
    return parsed


def _parse_transaction_enum(
    value: Any,
    mapping: dict[str, int],
    *,
    label: str,
) -> int:
    if isinstance(value, bool):
        raise RuntimeError(f"{label} enum is unsupported")
    if isinstance(value, int):
        if value < 0 or value > PROTOBUF_INT64_MAX:
            raise RuntimeError(f"{label} enum is out of range")
        return value
    if isinstance(value, str) and value in mapping:
        return mapping[value]
    if isinstance(value, str) and _is_canonical_decimal_text(value):
        parsed = int(value)
        if parsed > PROTOBUF_INT64_MAX:
            raise RuntimeError(f"{label} enum is out of range")
        return parsed
    raise RuntimeError(f"{label} enum is unsupported")


def _tron_transaction_result_bytes(
    result: dict[str, Any],
    *,
    label: str,
) -> bytes:
    if not isinstance(result, dict):
        raise RuntimeError(f"{label} must be an object")
    supported_fields = {
        "fee",
        "ret",
        "contractRet",
        "assetIssueID",
        "withdraw_amount",
        "unfreeze_amount",
        "exchange_received_amount",
        "exchange_inject_another_amount",
        "exchange_withdraw_another_amount",
        "exchange_id",
        "shielded_transaction_fee",
        "orderId",
        "orderDetails",
        "withdraw_expire_amount",
        "cancel_unfreezeV2_amount",
        "cancelUnfreezeV2Amount",
    }
    unknown_fields = set(result) - supported_fields
    if unknown_fields:
        raise RuntimeError(_unsupported_tron_fields_message(label, unknown_fields))
    out = bytearray()
    if "fee" in result:
        out.extend(
            _protobuf_u64_field(
                1,
                _parse_protobuf_nonnegative_int(result["fee"], label=f"{label} fee"),
            )
        )
    if "ret" in result:
        out.extend(
            _protobuf_u64_field(
                2,
                _parse_transaction_enum(
                    result["ret"],
                    TRON_TRANSACTION_RET_CODES,
                    label=f"{label} ret",
                ),
            )
        )
    if "contractRet" in result:
        out.extend(
            _protobuf_u64_field(
                3,
                _parse_transaction_enum(
                    result["contractRet"],
                    TRON_TRANSACTION_CONTRACT_RESULTS,
                    label=f"{label} contractRet",
                ),
            )
        )
    if "assetIssueID" in result:
        out.extend(_protobuf_string_field(14, result["assetIssueID"]))
    scalar_fields_before_order = (
        (15, "withdraw_amount"),
        (16, "unfreeze_amount"),
        (18, "exchange_received_amount"),
        (19, "exchange_inject_another_amount"),
        (20, "exchange_withdraw_another_amount"),
        (21, "exchange_id"),
        (22, "shielded_transaction_fee"),
    )
    for field_number, field_name in scalar_fields_before_order:
        if field_name in result:
            out.extend(
                _protobuf_u64_field(
                    field_number,
                    _parse_protobuf_nonnegative_int(
                        result[field_name],
                        label=f"{label} {field_name}",
                    ),
                )
            )
    if "orderId" in result:
        out.extend(
            _protobuf_bytes_field(
                25,
                _parse_hex_blob(result["orderId"], label=f"{label} orderId"),
            )
        )
    if "orderDetails" in result:
        order_details = result["orderDetails"]
        if not isinstance(order_details, list):
            raise RuntimeError(f"{label} orderDetails must be a list")
        for index, detail in enumerate(order_details):
            out.extend(
                _protobuf_bytes_field(
                    26,
                    _tron_market_order_detail_bytes(
                        detail,
                        label=f"{label} orderDetails[{index}]",
                    ),
                )
            )
    if "withdraw_expire_amount" in result:
        out.extend(
            _protobuf_u64_field(
                27,
                _parse_protobuf_nonnegative_int(
                    result["withdraw_expire_amount"],
                    label=f"{label} withdraw_expire_amount",
                ),
            )
        )
    out.extend(_tron_cancel_unfreeze_v2_amount_bytes(result, label=label))
    return bytes(out)


def _tron_market_order_detail_bytes(detail: Any, *, label: str) -> bytes:
    if not isinstance(detail, dict):
        raise RuntimeError(f"{label} must be an object")
    supported_fields = {
        "makerOrderId",
        "takerOrderId",
        "fillSellQuantity",
        "fillBuyQuantity",
    }
    unknown_fields = set(detail) - supported_fields
    if unknown_fields:
        raise RuntimeError(_unsupported_tron_fields_message(label, unknown_fields))
    out = bytearray()
    if "makerOrderId" in detail:
        out.extend(
            _protobuf_bytes_field(
                1,
                _parse_hex_blob(
                    detail["makerOrderId"],
                    label=f"{label} makerOrderId",
                ),
            )
        )
    if "takerOrderId" in detail:
        out.extend(
            _protobuf_bytes_field(
                2,
                _parse_hex_blob(
                    detail["takerOrderId"],
                    label=f"{label} takerOrderId",
                ),
            )
        )
    if "fillSellQuantity" in detail:
        out.extend(
            _protobuf_u64_field(
                3,
                _parse_protobuf_nonnegative_int(
                    detail["fillSellQuantity"],
                    label=f"{label} fillSellQuantity",
                ),
            )
        )
    if "fillBuyQuantity" in detail:
        out.extend(
            _protobuf_u64_field(
                4,
                _parse_protobuf_nonnegative_int(
                    detail["fillBuyQuantity"],
                    label=f"{label} fillBuyQuantity",
                ),
            )
        )
    return bytes(out)


def _tron_cancel_unfreeze_v2_amount_bytes(
    result: dict[str, Any],
    *,
    label: str,
) -> bytes:
    has_snake = "cancel_unfreezeV2_amount" in result
    has_camel = "cancelUnfreezeV2Amount" in result
    if has_snake and has_camel:
        raise RuntimeError(
            f"{label} must not include both cancel_unfreezeV2_amount and "
            "cancelUnfreezeV2Amount"
        )
    if not has_snake and not has_camel:
        return b""
    field_name = "cancel_unfreezeV2_amount" if has_snake else "cancelUnfreezeV2Amount"
    values = result[field_name]
    if not isinstance(values, dict):
        raise RuntimeError(f"{label} {field_name} must be an object")
    out = bytearray()
    for key, value in values.items():
        if not isinstance(key, str):
            raise RuntimeError(f"{label} {field_name} keys must be strings")
        entry = _protobuf_string_field(1, key) + _protobuf_u64_field(
            2,
            _parse_protobuf_nonnegative_int(
                value,
                label=f"{label} {field_name}[{key!r}]",
            ),
        )
        out.extend(_protobuf_bytes_field(28, entry))
    return bytes(out)


def _source_event_transaction_result_success_bytes(result: dict[str, Any]) -> bytes:
    fee = result.get("fee")
    ret_code = result.get("ret")
    if ret_code is not None and not _source_event_ret_code_is_success(ret_code):
        raise RuntimeError("source-event transaction ret enum must be SUCESS")
    out = bytearray()
    if fee is not None:
        out.extend(
            _protobuf_u64_field(
                1,
                _parse_protobuf_nonnegative_int(
                    fee,
                    label="source-event transaction fee",
                ),
            )
        )
    if ret_code is not None:
        out.extend(_protobuf_u64_field(2, 0))
    out.extend(_protobuf_u64_field(3, 1))
    return bytes(out)


def _source_event_result_bytes_are_success(result_bytes: bytes) -> bool:
    cursor = 0
    previous_field_number = 0
    saw_contract_ret = False
    while cursor < len(result_bytes):
        try:
            key, cursor = _read_protobuf_varint_at(
                result_bytes,
                cursor,
                label="source-event transaction result bytes",
            )
            field_number = key >> 3
            wire_type = key & 0x07
            if (
                wire_type != 0
                or field_number not in (1, 2, 3)
                or field_number <= previous_field_number
            ):
                return False
            value, cursor = _read_protobuf_varint_at(
                result_bytes,
                cursor,
                label="source-event transaction result bytes",
            )
        except RuntimeError:
            return False
        previous_field_number = field_number
        if field_number == 2 and value != 0:
            return False
        if field_number == 3:
            if value != 1:
                return False
            saw_contract_ret = True
    return saw_contract_ret


def _source_event_ret_code_is_success(value: Any) -> bool:
    if isinstance(value, bool):
        return False
    return value in (0, "0", "SUCESS")


def _source_event_contract_ret_is_success(value: Any) -> bool:
    if isinstance(value, bool):
        return False
    return value in (1, "1", "SUCCESS")


def _tron_transaction_bytes_from_json(
    transaction: dict[str, Any],
    *,
    label: str,
) -> tuple[bytes, bytes]:
    raw_data = _parse_exact_hex_blob(
        transaction.get("raw_data_hex"),
        label=f"{label} raw_data_hex",
    )
    out = bytearray(_protobuf_bytes_field(1, raw_data))
    signatures = transaction.get("signature", [])
    if signatures is None:
        signatures = []
    if not isinstance(signatures, list):
        raise RuntimeError(f"{label} signature must be a list")
    for index, signature in enumerate(signatures):
        signature_bytes = _parse_exact_hex_blob(
            signature,
            label=f"{label} signature[{index}]",
        )
        out.extend(_protobuf_bytes_field(2, signature_bytes))
    results = transaction.get("ret", [])
    if results is None:
        results = []
    if not isinstance(results, list):
        raise RuntimeError(f"{label} ret must be a list")
    for index, result in enumerate(results):
        out.extend(
            _protobuf_bytes_field(
                5,
                _tron_transaction_result_bytes(
                    result,
                    label=f"{label} ret[{index}]",
                ),
            )
        )
    return bytes(out), raw_data


def _source_event_transaction_bytes_summary(
    raw_data: bytes,
    signature: bytes,
    result: dict[str, Any],
) -> dict[str, Any]:
    result_bytes = _source_event_transaction_result_success_bytes(result)
    transaction_bytes = b"".join(
        [
            _protobuf_bytes_field(1, raw_data),
            _protobuf_bytes_field(2, signature),
            _protobuf_bytes_field(5, result_bytes),
        ]
    )
    return {
        "source_proof_transaction_bytes": _hex(transaction_bytes),
        "source_proof_transaction_hash": _hex(hashlib.sha256(transaction_bytes).digest()),
        "source_proof_result_bytes": _hex(result_bytes),
        "source_proof_transaction_bytes_checked": True,
        "transaction_merkle_branch_required": True,
    }


def _tron_merkle_root(transaction_hashes: list[bytes]) -> bytes:
    if not transaction_hashes:
        return b"\x00" * 32
    level = list(transaction_hashes)
    while len(level) > 1:
        next_level = []
        for index in range(0, len(level), 2):
            if index + 1 >= len(level):
                next_level.append(level[index])
            else:
                next_level.append(
                    hashlib.sha256(level[index] + level[index + 1]).digest()
                )
        level = next_level
    return level[0]


def _tron_merkle_branch(transaction_hashes: list[bytes], transaction_index: int) -> list[bytes]:
    if transaction_index < 0 or transaction_index >= len(transaction_hashes):
        raise RuntimeError("transaction index is outside transaction hash list")
    branch = []
    index = transaction_index
    level = list(transaction_hashes)
    while len(level) > 1:
        if index & 1:
            branch.append(level[index - 1])
        elif index + 1 < len(level):
            branch.append(level[index + 1])
        next_level = []
        for pair_index in range(0, len(level), 2):
            if pair_index + 1 >= len(level):
                next_level.append(level[pair_index])
            else:
                next_level.append(
                    hashlib.sha256(level[pair_index] + level[pair_index + 1]).digest()
                )
        level = next_level
        index >>= 1
    return branch


def _tron_block_header_raw_data_bytes(
    raw_data: dict[str, Any],
    *,
    tx_trie_root_nonzero: bool = True,
) -> bytes:
    timestamp = _parse_protobuf_nonnegative_int(
        raw_data.get("timestamp"),
        label="source-event block timestamp",
    )
    tx_trie_root = _parse_exact_hex32_blob(
        raw_data.get("txTrieRoot"),
        label="source-event block txTrieRoot",
        nonzero=tx_trie_root_nonzero,
    )
    parent_hash = _parse_exact_hex32_blob(
        raw_data.get("parentHash"),
        label="source-event block parentHash",
    )
    number = _parse_protobuf_nonnegative_int(
        raw_data.get("number"),
        label="source-event block number",
    )
    out = bytearray()
    out.extend(_protobuf_u64_field(1, timestamp))
    out.extend(_protobuf_bytes_field(2, tx_trie_root))
    out.extend(_protobuf_bytes_field(3, parent_hash))
    out.extend(_protobuf_u64_field(7, number))
    if "witness_id" in raw_data:
        out.extend(
            _protobuf_u64_field(
                8,
                _parse_protobuf_nonnegative_int(
                    raw_data["witness_id"],
                    label="source-event block witness_id",
                ),
            )
        )
    witness_address = _parse_exact_hex_blob(
        raw_data.get("witness_address"),
        label="source-event block witness_address",
    )
    if not _is_nonzero_tron_address_payload(witness_address):
        raise RuntimeError("source-event block witness_address must be a non-zero TRON address")
    out.extend(_protobuf_bytes_field(9, witness_address))
    if "version" in raw_data:
        out.extend(
            _protobuf_u64_field(
                10,
                _parse_protobuf_nonnegative_int(
                    raw_data["version"],
                    label="source-event block version",
                ),
            )
        )
    if "accountStateRoot" in raw_data:
        out.extend(
            _protobuf_bytes_field(
                11,
                    _parse_exact_hex32_blob(
                        raw_data["accountStateRoot"],
                        label="source-event block accountStateRoot",
                        nonzero=False,
                ),
            )
        )
    return bytes(out)


def _parse_solid_block_header(
    response: dict[str, Any],
    *,
    label: str,
    expected_block_number: int,
    tx_trie_root_nonzero: bool,
) -> dict[str, Any]:
    block_id = _parse_exact_hex32_blob(response.get("blockID"), label=f"{label} blockID")
    header = response.get("block_header")
    if not isinstance(header, dict):
        raise RuntimeError(f"{label} did not return block_header")
    raw_data = header.get("raw_data")
    if not isinstance(raw_data, dict):
        raise RuntimeError(f"{label} did not return block_header.raw_data")
    number = _parse_protobuf_nonnegative_int(
        raw_data.get("number"),
        label=f"{label} block number",
    )
    if number != expected_block_number:
        raise RuntimeError(f"{label} block number does not match expected height")
    timestamp = _parse_protobuf_nonnegative_int(
        raw_data.get("timestamp"),
        label=f"{label} block timestamp",
    )
    if timestamp == 0:
        raise RuntimeError(f"{label} block timestamp must not be zero")
    tx_trie_root = _parse_exact_hex32_blob(
        raw_data.get("txTrieRoot"),
        label=f"{label} block txTrieRoot",
        nonzero=tx_trie_root_nonzero,
    )
    parent_hash = _parse_exact_hex32_blob(
        raw_data.get("parentHash"),
        label=f"{label} block parentHash",
        nonzero=number > 0,
    )
    witness_address = _parse_exact_hex_blob(
        raw_data.get("witness_address"),
        label=f"{label} block witness_address",
    )
    if not _is_nonzero_tron_address_payload(witness_address):
        raise RuntimeError(f"{label} block witness_address must be a non-zero TRON address")
    witness_signature = _parse_exact_hex_blob(
        header.get("witness_signature"),
        label=f"{label} block witness_signature",
    )
    _require_canonical_tron_recoverable_signature(
        witness_signature,
        label=f"{label} witness_signature",
    )
    header_raw_data_bytes = _tron_block_header_raw_data_bytes(
        raw_data,
        tx_trie_root_nonzero=tx_trie_root_nonzero,
    )
    header_raw_data_hash = hashlib.sha256(header_raw_data_bytes).digest()
    expected_block_id = number.to_bytes(8, "big") + header_raw_data_hash[8:]
    if block_id != expected_block_id:
        raise RuntimeError(f"{label} blockID does not match header raw_data hash")
    recovered_witness_address20 = _tron_recovered_signature_address20(
        header_raw_data_hash,
        witness_signature,
    )
    if (
        recovered_witness_address20 is None
        or recovered_witness_address20 != witness_address[1:]
    ):
        raise RuntimeError(
            f"{label} witness_signature does not recover to witness_address"
        )
    version = raw_data.get("version")
    if version is not None:
        version = _parse_protobuf_nonnegative_int(
            version,
            label=f"{label} block version",
        )
    account_state_root = None
    if "accountStateRoot" in raw_data:
        account_state_root = _parse_exact_hex32_blob(
            raw_data["accountStateRoot"],
            label=f"{label} block accountStateRoot",
            nonzero=False,
        )
    return {
        "block_id": block_id,
        "number": number,
        "timestamp": timestamp,
        "tx_trie_root": tx_trie_root,
        "parent_hash": parent_hash,
        "account_state_root": account_state_root,
        "witness_address": witness_address,
        "witness_signature": witness_signature,
        "witness_signature_recovered_address": b"\x41" + recovered_witness_address20,
        "header_raw_data_bytes": header_raw_data_bytes,
        "header_raw_data_hash": header_raw_data_hash,
        "version": version,
    }


def _source_event_solid_block_header_proof_summary(
    child_header: dict[str, Any],
    parent_header: dict[str, Any],
) -> dict[str, Any]:
    child_account_state_root = child_header.get("account_state_root")
    parent_account_state_root = parent_header.get("account_state_root")
    if not isinstance(child_account_state_root, bytes) or not any(
        child_account_state_root
    ):
        return {
            "solid_block_header_proof_ready": False,
            "solid_block_header_proof_blocker": (
                "child accountStateRoot missing or zero"
            ),
        }
    if not isinstance(parent_account_state_root, bytes) or not any(
        parent_account_state_root
    ):
        return {
            "solid_block_header_proof_ready": False,
            "solid_block_header_proof_blocker": (
                "parent accountStateRoot missing or zero"
            ),
        }
    if not any(parent_header["tx_trie_root"]):
        return {
            "solid_block_header_proof_ready": False,
            "solid_block_header_proof_blocker": "parent txTrieRoot missing or zero",
        }
    if type(child_header.get("version")) is not int or child_header["version"] == 0:
        return {
            "solid_block_header_proof_ready": False,
            "solid_block_header_proof_blocker": "child header version missing or zero",
        }
    if type(parent_header.get("version")) is not int or parent_header["version"] == 0:
        return {
            "solid_block_header_proof_ready": False,
            "solid_block_header_proof_blocker": "parent header version missing or zero",
        }
    proof_input = {
        "raw_data": child_header["header_raw_data_bytes"],
        "witness_signature": child_header["witness_signature"],
        "parent_raw_data": parent_header["header_raw_data_bytes"],
        "parent_witness_signature": parent_header["witness_signature"],
        "raw_data_hash": _hex(child_header["header_raw_data_hash"]),
        "parent_raw_data_hash": _hex(parent_header["header_raw_data_hash"]),
        "block_id": _hex(child_header["block_id"]),
        "tx_trie_root": _hex(child_header["tx_trie_root"]),
        "account_state_root": _hex(child_account_state_root),
        "parent_block_id": _hex(parent_header["block_id"]),
        "witness_address": _hex(child_header["witness_address"]),
        "timestamp_ms": child_header["timestamp"],
        "header_version": child_header["version"],
    }
    try:
        proof_bytes = sccp_client.canonical_tron_solid_block_header_proof_bytes(
            proof_input
        )
        proof_hash = sccp_client.tron_solid_block_header_proof_hash(proof_input)
    except (RuntimeError, TypeError, ValueError):
        return {
            "solid_block_header_proof_ready": False,
            "solid_block_header_proof_blocker": "solid block header proof is invalid",
        }
    return {
        "solid_block_header_proof_ready": True,
        "solid_block_header_proof_input": {
            "raw_data": _hex(child_header["header_raw_data_bytes"]),
            "witness_signature": _hex(child_header["witness_signature"]),
            "parent_raw_data": _hex(parent_header["header_raw_data_bytes"]),
            "parent_witness_signature": _hex(parent_header["witness_signature"]),
            "raw_data_hash": _hex(child_header["header_raw_data_hash"]),
            "parent_raw_data_hash": _hex(parent_header["header_raw_data_hash"]),
            "block_id": _hex(child_header["block_id"]),
            "tx_trie_root": _hex(child_header["tx_trie_root"]),
            "account_state_root": _hex(child_account_state_root),
            "parent_block_id": _hex(parent_header["block_id"]),
            "witness_address": _hex(child_header["witness_address"]),
            "timestamp_ms": child_header["timestamp"],
            "header_version": child_header["version"],
        },
        "solid_block_header_proof_bytes": _hex(proof_bytes),
        "solid_block_header_proof_hash": proof_hash,
    }


def _decode_tron_witness_schedule_payload(payload: bytes) -> list[tuple[bytes, int]]:
    if len(payload) < 5 or payload[0] != 1:
        raise RuntimeError(
            "witness schedule payload must be canonical "
            "sccp:tron:witness-schedule-payload:v1 bytes"
        )
    witness_count = int.from_bytes(payload[1:5], "little")
    if witness_count == 0 or witness_count > 64 or len(payload) != 5 + witness_count * 29:
        raise RuntimeError(
            "witness schedule payload must be canonical "
            "sccp:tron:witness-schedule-payload:v1 bytes"
        )
    seen = set()
    witnesses = []
    total_weight = 0
    cursor = 5
    for index in range(witness_count):
        address = payload[cursor : cursor + 21]
        cursor += 21
        if not _is_nonzero_tron_address_payload(address):
            raise RuntimeError(
                f"witness schedule payload witness {index} must be a non-zero TRON address"
            )
        if address in seen:
            raise RuntimeError(f"witness schedule payload witness {index} must be unique")
        seen.add(address)
        weight = int.from_bytes(payload[cursor : cursor + 8], "little")
        cursor += 8
        if weight == 0:
            raise RuntimeError(f"witness schedule payload witness {index} weight must not be zero")
        total_weight += weight
        if total_weight > 0xFFFFFFFFFFFFFFFF:
            raise RuntimeError("witness schedule payload total weight must fit u64")
        witnesses.append((address, weight))
    return witnesses


def _source_event_witness_schedule_summary(
    payload: bytes | None,
    *,
    child_witness_address: bytes,
    parent_witness_address: bytes,
    expected_schedule_hash: bytes | None,
    allow_expected_mismatch: bool = False,
) -> dict[str, Any]:
    if payload is None:
        return {
            "witness_schedule_proof_ready": False,
            "witness_schedule_proof_blocker": "active witness schedule payload required",
        }
    try:
        payload_hash = sccp_client.tron_witness_schedule_payload_hash(payload)
        schedule_hash = sccp_client.tron_witness_schedule_hash_from_payload(payload)
    except (RuntimeError, TypeError, ValueError):
        return {
            "witness_schedule_proof_ready": False,
            "witness_schedule_proof_blocker": "witness schedule payload is invalid",
        }
    schedule_hash_bytes = _parse_hex32_blob(
        schedule_hash,
        label="witness schedule hash",
    )
    expected_matches = (
        expected_schedule_hash is not None
        and schedule_hash_bytes == expected_schedule_hash
    )
    if (
        expected_schedule_hash is not None
        and not expected_matches
        and not allow_expected_mismatch
    ):
        raise RuntimeError(
            "witness schedule hash does not match expected witness schedule hash"
        )
    witnesses = _decode_tron_witness_schedule_payload(payload)
    weight_by_address = {address: weight for address, weight in witnesses}
    child_weight = weight_by_address.get(child_witness_address)
    if child_weight is None:
        raise RuntimeError("source-event block witness is not in active witness schedule")
    parent_weight = weight_by_address.get(parent_witness_address)
    if parent_weight is None:
        raise RuntimeError("source-event parent witness is not in active witness schedule")
    summary = {
        "witness_schedule_proof_ready": True,
        "witness_schedule_payload": _hex(payload),
        "witness_schedule_payload_hash": payload_hash,
        "witness_schedule_hash": schedule_hash,
        "witness_schedule_expected_hash_matches": expected_matches,
        "witness_schedule_witness_count": len(witnesses),
        "witness_schedule_total_weight": sum(weight for _address, weight in witnesses),
        "block_witness_in_schedule": True,
        "block_witness_weight": child_weight,
        "parent_block_witness_in_schedule": True,
        "parent_block_witness_weight": parent_weight,
    }
    if expected_schedule_hash is not None:
        summary["expected_witness_schedule_hash"] = _hex(expected_schedule_hash)
    return summary


_MISSING = object()


def _mapping_value(value: dict[str, Any], *names: str) -> Any:
    for name in names:
        if name in value:
            return value[name]
    raise RuntimeError(f"{names[0]} is required")


def _mapping_optional_value(value: dict[str, Any], *names: str) -> Any:
    for name in names:
        if name in value:
            return value[name]
    return _MISSING


def _witness_schedule_payload_from_value(value: Any, *, label: str) -> bytes:
    if isinstance(value, str):
        return _parse_exact_hex_blob(value, label=label)
    if isinstance(value, (bytes, bytearray)):
        payload = bytes(value)
        if not any(payload):
            raise RuntimeError(f"{label} must not be zero")
        return payload
    raise RuntimeError(f"{label} must be hex")


def _anchored_transition_block_hash(
    *,
    transition_block_number: int,
    child_header: dict[str, Any],
    parent_header: dict[str, Any],
    ancestor_headers: list[dict[str, Any]],
) -> bytes | None:
    if child_header["number"] == transition_block_number:
        return child_header["block_id"]
    if parent_header["number"] == transition_block_number:
        return parent_header["block_id"]
    for header in ancestor_headers:
        if header["number"] == transition_block_number:
            return header["block_id"]
    return None


def _source_event_witness_schedule_transition_chain_summary(
    transition_inputs: list[dict[str, Any]],
    *,
    active_witness_schedule_payload: bytes | None,
    expected_schedule_hash: bytes | None,
    child_header: dict[str, Any],
    parent_header: dict[str, Any],
    ancestor_headers: list[dict[str, Any]],
) -> dict[str, Any]:
    if not transition_inputs:
        return {
            "witness_schedule_transition_chain_ready": False,
            "witness_schedule_transition_chain_required": False,
            "witness_schedule_transition_count": 0,
        }
    if active_witness_schedule_payload is None:
        return {
            "witness_schedule_transition_chain_ready": False,
            "witness_schedule_transition_chain_required": True,
            "witness_schedule_transition_chain_blocker": (
                "active witness schedule payload required"
            ),
            "witness_schedule_transition_count": len(transition_inputs),
        }
    if expected_schedule_hash is None:
        return {
            "witness_schedule_transition_chain_ready": False,
            "witness_schedule_transition_chain_required": True,
            "witness_schedule_transition_chain_blocker": (
                "source trust-anchor witness schedule hash required"
            ),
            "witness_schedule_transition_count": len(transition_inputs),
        }

    active_schedule_hash = _parse_hex32_blob(
        sccp_client.tron_witness_schedule_hash_from_payload(
            active_witness_schedule_payload
        ),
        label="active witness schedule hash",
    )
    expected_parent_hash = expected_schedule_hash
    previous_to_epoch = None
    previous_block_number = None
    previous_next_payload = None
    proof_summaries = []

    for index, raw_transition in enumerate(transition_inputs):
        if not isinstance(raw_transition, dict):
            raise RuntimeError(f"witness schedule transition {index} must be an object")
        parent_payload_value = _mapping_optional_value(
            raw_transition,
            "parentWitnessSchedulePayload",
            "parent_witness_schedule_payload",
        )
        if parent_payload_value is _MISSING:
            if previous_next_payload is None:
                raise RuntimeError(
                    f"witness schedule transition {index} parent payload is required"
                )
            parent_payload = previous_next_payload
        else:
            parent_payload = _witness_schedule_payload_from_value(
                parent_payload_value,
                label=f"witness schedule transition {index} parent payload",
            )
        parent_schedule_hash = _parse_hex32_blob(
            sccp_client.tron_witness_schedule_hash_from_payload(parent_payload),
            label=f"witness schedule transition {index} parent schedule hash",
        )
        if parent_schedule_hash != expected_parent_hash:
            raise RuntimeError(
                f"witness schedule transition {index} parent schedule hash does not match chain"
            )

        next_payload = _witness_schedule_payload_from_value(
            _mapping_value(
                raw_transition,
                "nextWitnessSchedulePayload",
                "next_witness_schedule_payload",
            ),
            label=f"witness schedule transition {index} next payload",
        )
        next_schedule_hash = _parse_hex32_blob(
            sccp_client.tron_witness_schedule_hash_from_payload(next_payload),
            label=f"witness schedule transition {index} next schedule hash",
        )
        next_payload_hash = _parse_hex32_blob(
            sccp_client.tron_witness_schedule_payload_hash(next_payload),
            label=f"witness schedule transition {index} next payload hash",
        )
        from_epoch = _parse_protobuf_nonnegative_int(
            _mapping_value(
                raw_transition,
                "fromWitnessScheduleEpoch",
                "from_witness_schedule_epoch",
            ),
            label=f"witness schedule transition {index} from epoch",
        )
        to_epoch = _parse_protobuf_nonnegative_int(
            _mapping_value(
                raw_transition,
                "toWitnessScheduleEpoch",
                "to_witness_schedule_epoch",
            ),
            label=f"witness schedule transition {index} to epoch",
        )
        if previous_to_epoch is not None and from_epoch != previous_to_epoch:
            raise RuntimeError(
                f"witness schedule transition {index} epoch does not continue chain"
            )
        transition_block_number = _parse_protobuf_nonnegative_int(
            _mapping_value(
                raw_transition,
                "transitionBlockNumber",
                "transition_block_number",
            ),
            label=f"witness schedule transition {index} block number",
        )
        anchored_hash = _anchored_transition_block_hash(
            transition_block_number=transition_block_number,
            child_header=child_header,
            parent_header=parent_header,
            ancestor_headers=ancestor_headers,
        )
        if anchored_hash is None:
            raise RuntimeError(
                f"witness schedule transition {index} block is not anchored"
            )
        supplied_transition_hash = _mapping_optional_value(
            raw_transition,
            "transitionBlockHash",
            "transition_block_hash",
        )
        transition_block_hash = anchored_hash
        if supplied_transition_hash is not _MISSING:
            transition_block_hash = _parse_exact_hex32_blob(
                supplied_transition_hash,
                label=f"witness schedule transition {index} block hash",
            )
            if transition_block_hash != anchored_hash:
                raise RuntimeError(
                    f"witness schedule transition {index} block hash is not anchored"
                )
        if (
            previous_block_number is not None
            and transition_block_number <= previous_block_number
        ):
            raise RuntimeError(
                f"witness schedule transition {index} block number is not increasing"
            )

        message_input = {
            "source_domain": sccp_client.SCCP_DOMAIN_TRON,
            "from_witness_schedule_epoch": from_epoch,
            "to_witness_schedule_epoch": to_epoch,
            "transition_block_number": transition_block_number,
            "transition_block_hash": _hex(transition_block_hash),
            "parent_witness_schedule_hash": _hex(parent_schedule_hash),
            "next_witness_schedule_hash": _hex(next_schedule_hash),
            "next_witness_schedule_payload": _hex(next_payload),
            "next_witness_schedule_payload_hash": _hex(next_payload_hash),
        }
        try:
            message_bytes = (
                sccp_client.canonical_tron_witness_schedule_transition_message_bytes(
                    message_input
                )
            )
            message_hash = (
                sccp_client.tron_witness_schedule_transition_message_hash(
                    message_input
                )
            )
        except (RuntimeError, TypeError, ValueError):
            return {
                "witness_schedule_transition_chain_ready": False,
                "witness_schedule_transition_chain_required": True,
                "witness_schedule_transition_chain_blocker": (
                    f"witness schedule transition {index} message is invalid"
                ),
                "witness_schedule_transition_count": len(transition_inputs),
            }
        supplied_message_hash = _mapping_optional_value(
            raw_transition,
            "transitionMessageHash",
            "transition_message_hash",
        )
        if supplied_message_hash is not _MISSING and _parse_exact_hex32_blob(
            supplied_message_hash,
            label=f"witness schedule transition {index} message hash",
        ) != _parse_hex32_blob(message_hash, label="witness schedule transition message hash"):
            raise RuntimeError(
                f"witness schedule transition {index} message hash does not match"
            )

        parent_witnesses = _decode_tron_witness_schedule_payload(parent_payload)
        parent_weights = [weight for _address, weight in parent_witnesses]
        signers_bitmap = _parse_exact_hex_blob(
            _mapping_value(raw_transition, "signersBitmap", "signers_bitmap"),
            label=f"witness schedule transition {index} signers bitmap",
        )
        signer_indices = _signer_indices_from_bitmap(
            signers_bitmap,
            len(parent_witnesses),
        )
        raw_signatures = _mapping_value(raw_transition, "signatures", "signatures_hex")
        if not isinstance(raw_signatures, list):
            raise RuntimeError(
                f"witness schedule transition {index} signatures must be a list"
            )
        signatures = [
            _parse_exact_hex_blob(
                signature,
                label=f"witness schedule transition {index} signature {signature_index}",
            )
            for signature_index, signature in enumerate(raw_signatures)
        ]
        if len(signatures) != len(signer_indices):
            raise RuntimeError(
                f"witness schedule transition {index} signature count does not match signer bitmap"
            )
        signed_weight = sum(parent_weights[signer_index] for signer_index in signer_indices)
        total_weight = sum(parent_weights)
        if signed_weight * 3 <= total_weight * 2:
            raise RuntimeError(
                f"witness schedule transition {index} signed weight does not exceed two thirds"
            )
        transition_message_hash_bytes = _parse_hex32_blob(
            message_hash,
            label=f"witness schedule transition {index} message hash",
        )
        recovered_addresses = _verify_tron_witness_signature_signers(
            message_hash=transition_message_hash_bytes,
            signatures=signatures,
            witness_addresses=[address for address, _weight in parent_witnesses],
            signer_indices=signer_indices,
            label=f"witness schedule transition {index}",
        )
        seal_input = {
            **message_input,
            "transition_message_hash": message_hash,
            "seal_proof": {
                "version": 1,
                "total_weight": total_weight,
                "signed_weight": signed_weight,
                "solid_block_message_hash": message_hash,
                "witness_addresses": [_hex(address) for address, _weight in parent_witnesses],
                "witness_weights": parent_weights,
                "signers_bitmap": _hex(signers_bitmap),
                "signatures": [_hex(signature) for signature in signatures],
            },
        }
        try:
            seal_bytes = (
                sccp_client.canonical_tron_witness_schedule_transition_seal_bytes(
                    seal_input
                )
            )
            seal_hash = sccp_client.tron_witness_schedule_transition_seal_hash(
                seal_input
            )
        except (RuntimeError, TypeError, ValueError):
            return {
                "witness_schedule_transition_chain_ready": False,
                "witness_schedule_transition_chain_required": True,
                "witness_schedule_transition_chain_blocker": (
                    f"witness schedule transition {index} seal is invalid"
                ),
                "witness_schedule_transition_count": len(transition_inputs),
            }
        supplied_seal_hash = _mapping_optional_value(
            raw_transition,
            "transitionSealHash",
            "transition_seal_hash",
        )
        if supplied_seal_hash is not _MISSING and _parse_exact_hex32_blob(
            supplied_seal_hash,
            label=f"witness schedule transition {index} seal hash",
        ) != _parse_hex32_blob(seal_hash, label="witness schedule transition seal hash"):
            raise RuntimeError(
                f"witness schedule transition {index} seal hash does not match"
            )

        proof_summaries.append(
            {
                "version": 1,
                "source_domain": sccp_client.SCCP_DOMAIN_TRON,
                "from_witness_schedule_epoch": from_epoch,
                "to_witness_schedule_epoch": to_epoch,
                "transition_block_number": transition_block_number,
                "transition_block_hash": _hex(transition_block_hash),
                "parent_witness_schedule_hash": _hex(parent_schedule_hash),
                "next_witness_schedule_hash": _hex(next_schedule_hash),
                "next_witness_schedule_payload": _hex(next_payload),
                "next_witness_schedule_payload_hash": _hex(next_payload_hash),
                "transition_message_bytes": _hex(message_bytes),
                "transition_message_hash": message_hash,
                "transition_seal_bytes": _hex(seal_bytes),
                "transition_seal_hash": seal_hash,
                "signer_indices": signer_indices,
                "signer_addresses": [
                    _hex(parent_witnesses[signer_index][0])
                    for signer_index in signer_indices
                ],
                "recovered_addresses": [
                    _hex(address) for address in recovered_addresses
                ],
                "signed_weight": signed_weight,
                "total_weight": total_weight,
                "threshold_checked": True,
            }
        )
        expected_parent_hash = next_schedule_hash
        previous_to_epoch = to_epoch
        previous_block_number = transition_block_number
        previous_next_payload = next_payload

    if expected_parent_hash != active_schedule_hash:
        raise RuntimeError("witness schedule transition chain does not end at active schedule")
    return {
        "witness_schedule_transition_chain_ready": True,
        "witness_schedule_transition_chain_required": True,
        "witness_schedule_transition_count": len(proof_summaries),
        "witness_schedule_transition_anchor_hash": _hex(expected_schedule_hash),
        "witness_schedule_transition_final_hash": _hex(active_schedule_hash),
        "witness_schedule_transition_proofs": proof_summaries,
    }


def _signer_indices_from_bitmap(bitmap: bytes, roster_len: int) -> list[int]:
    if roster_len <= 0 or len(bitmap) != (roster_len + 7) // 8:
        raise RuntimeError("witness seal signers bitmap length does not match schedule")
    indices = []
    for byte_index, value in enumerate(bitmap):
        for bit_index in range(8):
            if ((value >> bit_index) & 1) == 0:
                continue
            index = byte_index * 8 + bit_index
            if index >= roster_len:
                raise RuntimeError(
                    "witness seal signers bitmap sets a bit outside the schedule"
                )
            indices.append(index)
    if not indices:
        raise RuntimeError("witness seal signers bitmap selects no witnesses")
    return indices


def _verify_tron_witness_signature_signers(
    *,
    message_hash: bytes,
    signatures: list[bytes],
    witness_addresses: list[bytes],
    signer_indices: list[int],
    label: str,
) -> list[bytes]:
    if len(signatures) != len(signer_indices):
        raise RuntimeError(f"{label} signature count does not match signer bitmap")
    recovered_addresses = []
    for signature_index, signer_index in enumerate(signer_indices):
        signature = signatures[signature_index]
        _require_canonical_tron_recoverable_signature(
            signature,
            label=f"{label} signature {signature_index}",
        )
        recovered = _tron_recovered_signature_address20(message_hash, signature)
        if recovered is None:
            raise RuntimeError(f"{label} signature {signature_index} does not recover")
        recovered_payload = b"\x41" + recovered
        if recovered_payload != witness_addresses[signer_index]:
            raise RuntimeError(
                f"{label} signature {signature_index} does not recover to selected witness"
            )
        recovered_addresses.append(recovered_payload)
    return recovered_addresses


def _source_event_witness_seal_summary(
    payload: bytes | None,
    *,
    block_number: int,
    block_hash: bytes,
    transaction_root: bytes,
    receipt_root: bytes | None,
    receipt_proof_hash: bytes | None,
    signers_bitmap: bytes | None,
    signatures: list[bytes],
    expected_seal_hash: bytes | None,
) -> dict[str, Any]:
    blockers = []
    if payload is None:
        blockers.append("active witness schedule payload required")
    if receipt_root is None:
        blockers.append("receipt root required")
    if receipt_proof_hash is None:
        blockers.append("receipt proof hash required")
    if signers_bitmap is None:
        blockers.append("witness seal signers bitmap required")
    if not signatures:
        blockers.append("witness seal signatures required")
    if blockers:
        return {
            "witness_seal_proof_ready": False,
            "witness_seal_proof_blocker": "; ".join(blockers),
        }
    assert payload is not None
    assert receipt_root is not None
    assert receipt_proof_hash is not None
    assert signers_bitmap is not None

    witnesses = _decode_tron_witness_schedule_payload(payload)
    witness_addresses = [address for address, _weight in witnesses]
    witness_weights = [weight for _address, weight in witnesses]
    signer_indices = _signer_indices_from_bitmap(signers_bitmap, len(witnesses))
    if len(signatures) != len(signer_indices):
        raise RuntimeError("witness seal signature count does not match signer bitmap")
    witness_schedule_hash = sccp_client.tron_witness_schedule_hash_from_payload(payload)
    solid_block_message_input = {
        "source_domain": sccp_client.SCCP_DOMAIN_TRON,
        "solid_block_number": block_number,
        "block_hash": _hex(block_hash),
        "witness_schedule_hash": witness_schedule_hash,
        "receipt_root": _hex(receipt_root),
        "transaction_root": _hex(transaction_root),
        "receipt_proof_hash": _hex(receipt_proof_hash),
    }
    try:
        solid_block_message_bytes = (
            sccp_client.canonical_tron_solid_block_message_bytes(
                solid_block_message_input
            )
        )
        solid_block_message_hash = sccp_client.tron_solid_block_message_hash(
            solid_block_message_input
        )
    except (RuntimeError, TypeError, ValueError):
        return {
            "witness_seal_proof_ready": False,
            "witness_seal_proof_blocker": "witness seal solid-block message is invalid",
        }
    total_weight = sum(witness_weights)
    signed_weight = sum(witness_weights[index] for index in signer_indices)
    if signed_weight * 3 <= total_weight * 2:
        raise RuntimeError("witness seal signed weight does not exceed two thirds")
    solid_message_hash_bytes = _parse_hex32_blob(
        solid_block_message_hash,
        label="solid block message hash",
    )
    recovered_addresses = _verify_tron_witness_signature_signers(
        message_hash=solid_message_hash_bytes,
        signatures=signatures,
        witness_addresses=witness_addresses,
        signer_indices=signer_indices,
        label="witness seal",
    )
    seal_input = {
        "version": 1,
        "total_weight": total_weight,
        "signed_weight": signed_weight,
        "solid_block_message_hash": solid_block_message_hash,
        "witness_addresses": [_hex(address) for address in witness_addresses],
        "witness_weights": witness_weights,
        "signers_bitmap": _hex(signers_bitmap),
        "signatures": [_hex(signature) for signature in signatures],
    }
    try:
        seal_bytes = sccp_client.canonical_tron_witness_seal_bytes(seal_input)
        seal_hash = sccp_client.tron_witness_seal_hash(seal_input)
    except (RuntimeError, TypeError, ValueError):
        return {
            "witness_seal_proof_ready": False,
            "witness_seal_proof_blocker": "witness seal proof is invalid",
        }
    seal_hash_bytes = _parse_hex32_blob(seal_hash, label="witness seal hash")
    if expected_seal_hash is not None and seal_hash_bytes != expected_seal_hash:
        raise RuntimeError("witness seal hash does not match expected witness seal hash")
    return {
        "witness_seal_proof_ready": True,
        "solid_block_message_input": solid_block_message_input,
        "solid_block_message_bytes": _hex(solid_block_message_bytes),
        "solid_block_message_hash": solid_block_message_hash,
        "witness_seal_proof_input": seal_input,
        "witness_seal_proof_bytes": _hex(seal_bytes),
        "witness_seal_hash": seal_hash,
        "witness_seal_expected_hash_matches": expected_seal_hash is not None,
        "witness_seal_signer_indices": signer_indices,
        "witness_seal_signer_addresses": [
            _hex(witness_addresses[index]) for index in signer_indices
        ],
        "witness_seal_recovered_addresses": [
            _hex(address) for address in recovered_addresses
        ],
        "witness_seal_signed_weight": signed_weight,
        "witness_seal_total_weight": total_weight,
        "witness_seal_threshold_checked": True,
    }


def _signed_block_header_proof_input(header: dict[str, Any]) -> dict[str, Any]:
    account_state_root = header.get("account_state_root")
    if not isinstance(account_state_root, bytes) or not any(account_state_root):
        raise RuntimeError("signed block header accountStateRoot missing or zero")
    if not any(header["tx_trie_root"]):
        raise RuntimeError("signed block header txTrieRoot missing or zero")
    if type(header.get("version")) is not int or header["version"] == 0:
        raise RuntimeError("signed block header version missing or zero")
    return {
        "version": 1,
        "raw_data": _hex(header["header_raw_data_bytes"]),
        "witness_signature": _hex(header["witness_signature"]),
        "raw_data_hash": _hex(header["header_raw_data_hash"]),
        "block_id": _hex(header["block_id"]),
        "tx_trie_root": _hex(header["tx_trie_root"]),
        "account_state_root": _hex(account_state_root),
        "parent_block_id": _hex(header["parent_hash"]),
        "witness_address": _hex(header["witness_address"]),
        "timestamp_ms": header["timestamp"],
        "header_version": header["version"],
    }


def _source_event_ancestor_headers_summary(
    ancestor_headers: list[dict[str, Any]],
    *,
    parent_header: dict[str, Any],
    witness_weights: dict[bytes, int] | None,
) -> dict[str, Any]:
    if not ancestor_headers:
        return {
            "solid_block_ancestor_headers_ready": False,
            "solid_block_ancestor_headers_blocker": (
                "at least one signed ancestor header required for "
                "non-placeholder TRON material"
            ),
            "solid_block_ancestor_header_count": 0,
        }
    if witness_weights is None:
        return {
            "solid_block_ancestor_headers_ready": False,
            "solid_block_ancestor_headers_blocker": (
                "active witness schedule payload required"
            ),
            "solid_block_ancestor_header_count": len(ancestor_headers),
        }
    expected_block_id = parent_header["parent_hash"]
    previous_number = parent_header["number"]
    previous_timestamp = parent_header["timestamp"]
    proof_inputs = []
    for header in ancestor_headers:
        if header["block_id"] != expected_block_id:
            raise RuntimeError("solid-block ancestor header does not link to parent chain")
        if header["number"] + 1 != previous_number:
            raise RuntimeError("solid-block ancestor header height is not contiguous")
        if header["timestamp"] >= previous_timestamp:
            raise RuntimeError("solid-block ancestor header timestamp is not before child")
        if header["witness_address"] not in witness_weights:
            raise RuntimeError("solid-block ancestor witness is not in active schedule")
        proof_inputs.append(_signed_block_header_proof_input(header))
        expected_block_id = header["parent_hash"]
        previous_number = header["number"]
        previous_timestamp = header["timestamp"]
    return {
        "solid_block_ancestor_headers_ready": True,
        "solid_block_ancestor_header_count": len(proof_inputs),
        "solid_block_ancestor_header_proofs": proof_inputs,
    }


def _source_event_confirmation_headers_summary(
    confirmation_headers: list[dict[str, Any]],
    *,
    child_header: dict[str, Any],
    witness_weights: dict[bytes, int] | None,
) -> dict[str, Any]:
    if not confirmation_headers:
        return {
            "solid_block_confirmation_headers_ready": False,
            "solid_block_confirmation_headers_blocker": (
                "confirmation headers required for non-placeholder TRON material"
            ),
            "solid_block_confirmation_header_count": 0,
        }
    if witness_weights is None:
        return {
            "solid_block_confirmation_headers_ready": False,
            "solid_block_confirmation_headers_blocker": (
                "active witness schedule payload required"
            ),
            "solid_block_confirmation_header_count": len(confirmation_headers),
        }
    expected_parent_block_id = child_header["block_id"]
    previous_number = child_header["number"]
    previous_timestamp = child_header["timestamp"]
    proof_inputs = []
    unique_witnesses = set()
    approval_weight = 0
    for header in confirmation_headers:
        if header["parent_hash"] != expected_parent_block_id:
            raise RuntimeError(
                "solid-block confirmation header parentHash does not link forward"
            )
        if previous_number + 1 != header["number"]:
            raise RuntimeError("solid-block confirmation header height is not contiguous")
        if header["timestamp"] <= previous_timestamp:
            raise RuntimeError("solid-block confirmation header timestamp is not after parent")
        weight = witness_weights.get(header["witness_address"])
        if weight is None:
            raise RuntimeError("solid-block confirmation witness is not in active schedule")
        if header["witness_address"] not in unique_witnesses:
            unique_witnesses.add(header["witness_address"])
            approval_weight += weight
        proof_inputs.append(_signed_block_header_proof_input(header))
        expected_parent_block_id = header["block_id"]
        previous_number = header["number"]
        previous_timestamp = header["timestamp"]
    total_weight = sum(witness_weights.values())
    if approval_weight * 3 <= total_weight * 2:
        return {
            "solid_block_confirmation_headers_ready": False,
            "solid_block_confirmation_headers_blocker": (
                "confirmation witness weight does not exceed two thirds"
            ),
            "solid_block_confirmation_header_count": len(proof_inputs),
            "solid_block_confirmation_unique_witness_count": len(unique_witnesses),
            "solid_block_confirmation_signed_weight": approval_weight,
            "solid_block_confirmation_total_weight": total_weight,
            "solid_block_confirmation_header_proofs": proof_inputs,
        }
    return {
        "solid_block_confirmation_headers_ready": True,
        "solid_block_confirmation_header_count": len(proof_inputs),
        "solid_block_confirmation_unique_witness_count": len(unique_witnesses),
        "solid_block_confirmation_signed_weight": approval_weight,
        "solid_block_confirmation_total_weight": total_weight,
        "solid_block_confirmation_header_proofs": proof_inputs,
    }


def _source_event_transaction_production_readiness(
    solid_block: dict[str, Any],
) -> dict[str, Any]:
    blockers: list[str] = []

    def require_ready(flag: str, label: str) -> None:
        if solid_block.get(flag) is True:
            return
        blocker_key = flag.removesuffix("_ready") + "_blocker"
        blocker = solid_block.get(blocker_key)
        if isinstance(blocker, str) and blocker:
            blockers.append(f"{label}: {blocker}")
        else:
            blockers.append(f"{label} required")

    require_ready("transaction_source_proof_ready", "transaction source proof")
    require_ready("solid_block_header_proof_ready", "solid block header proof")
    require_ready("witness_schedule_proof_ready", "witness schedule proof")
    if (
        solid_block.get("witness_schedule_proof_ready") is True
        and solid_block.get("witness_schedule_expected_hash_matches") is not True
        and solid_block.get("witness_schedule_transition_chain_ready") is not True
    ):
        blockers.append(
            "witness schedule must match expected source trust-anchor hash or "
            "carry a valid transition chain"
        )
    require_ready("witness_seal_proof_ready", "witness seal proof")
    require_ready("solid_block_ancestor_headers_ready", "solid block ancestor headers")
    require_ready(
        "solid_block_confirmation_headers_ready",
        "solid block confirmation headers",
    )
    if blockers:
        return {
            "source_event_transaction_production_ready": False,
            "source_event_transaction_production_blockers": blockers,
        }
    return {"source_event_transaction_production_ready": True}


def _effective_expected_witness_schedule_hash(
    summary: dict[str, Any],
    explicit_expected_hash: bytes | None,
) -> bytes | None:
    source_trust_anchor_hash = None
    source_record_inputs = summary.get("source_record_inputs")
    if isinstance(source_record_inputs, dict):
        raw_source_trust_anchor_hash = source_record_inputs.get(
            "source_trust_anchor_hash"
        )
        if isinstance(raw_source_trust_anchor_hash, str):
            source_trust_anchor_hash = _parse_hex32(
                raw_source_trust_anchor_hash,
                label="source record source_trust_anchor_hash",
            )
    if (
        explicit_expected_hash is not None
        and source_trust_anchor_hash is not None
        and explicit_expected_hash != source_trust_anchor_hash
    ):
        raise ValueError(
            "--expected-witness-schedule-hash must match "
            "--source-trust-anchor-hash when source record preflight is supplied"
        )
    return explicit_expected_hash or source_trust_anchor_hash


def _source_event_transaction_source_proof_summary(
    *,
    source_event_digest: bytes,
    receipt_root: bytes | None,
    transaction_root: bytes,
    transaction_index: int,
    transaction_count: int,
    transaction_bytes: bytes,
    transaction_merkle_branch: list[bytes],
    source_inclusion_branch: list[bytes],
    source_bridge_address20: bytes,
    owner_address20: bytes,
    expected_receipt_proof_hash: bytes | None,
) -> tuple[dict[str, Any], bytes | None]:
    blockers = []
    if receipt_root is None:
        blockers.append("receipt root required")
    if not source_inclusion_branch:
        blockers.append("source inclusion branch required")
    if blockers:
        return (
            {
                "transaction_source_proof_ready": False,
                "transaction_source_proof_blocker": "; ".join(blockers),
            },
            None,
        )
    assert receipt_root is not None
    proof_input = {
        "source_event_digest": _hex(source_event_digest),
        "receipt_root": _hex(receipt_root),
        "transaction_root": _hex(transaction_root),
        "transaction_index": transaction_index,
        "transaction_count": transaction_count,
        "transaction_bytes": _hex(transaction_bytes),
        "transaction_merkle_branch": [
            _hex(sibling) for sibling in transaction_merkle_branch
        ],
        "inclusion_branch": [_hex(sibling) for sibling in source_inclusion_branch],
        "source_bridge_emitter_address": _hex(source_bridge_address20),
        "source_bridge_owner_address": _hex(owner_address20),
    }
    try:
        proof_bytes = sccp_client.canonical_tron_sccp_transaction_source_proof_bytes(
            proof_input
        )
        proof_hash = sccp_client.tron_sccp_transaction_source_proof_hash(
            proof_input
        )
    except (RuntimeError, TypeError, ValueError):
        return (
            {
                "transaction_source_proof_ready": False,
                "transaction_source_proof_blocker": "transaction source proof is invalid",
            },
            None,
        )
    proof_hash_bytes = _parse_hex32_blob(
        proof_hash,
        label="transaction source proof hash",
    )
    if (
        expected_receipt_proof_hash is not None
        and proof_hash_bytes != expected_receipt_proof_hash
    ):
        raise RuntimeError(
            "transaction source proof hash does not match receipt proof hash"
        )
    return (
        {
            "transaction_source_proof_ready": True,
            "transaction_source_proof_input": proof_input,
            "transaction_source_proof_bytes": _hex(proof_bytes),
            "transaction_source_proof_hash": proof_hash,
            "transaction_source_proof_hash_matches_receipt_proof_hash": (
                expected_receipt_proof_hash is not None
            ),
        },
        proof_hash_bytes,
    )


def _source_event_solid_block_summary(
    response: dict[str, Any],
    *,
    parent_response: dict[str, Any],
    ancestor_responses: list[dict[str, Any]],
    confirmation_responses: list[dict[str, Any]],
    active_witness_schedule_payload: bytes | None,
    expected_witness_schedule_hash: bytes | None,
    receipt_root: bytes | None,
    receipt_proof_hash: bytes | None,
    witness_seal_signers_bitmap: bytes | None,
    witness_seal_signatures: list[bytes],
    expected_witness_seal_hash: bytes | None,
    witness_schedule_transition_inputs: list[dict[str, Any]],
    source_event_digest: bytes,
    source_inclusion_branch: list[bytes],
    source_bridge_address20: bytes,
    owner_address20: bytes,
    transaction_id: bytes,
    block_number: int,
    source_transaction_bytes: bytes,
    source_transaction_hash: bytes,
) -> dict[str, Any]:
    child_header = _parse_solid_block_header(
        response,
        label="source-event block",
        expected_block_number=block_number,
        tx_trie_root_nonzero=True,
    )
    if block_number <= 1:
        raise RuntimeError("source-event block parent height must be positive")
    parent_header = _parse_solid_block_header(
        parent_response,
        label="source-event parent block",
        expected_block_number=block_number - 1,
        tx_trie_root_nonzero=False,
    )
    if child_header["parent_hash"] != parent_header["block_id"]:
        raise RuntimeError("source-event block parentHash does not match parent blockID")
    if parent_header["timestamp"] >= child_header["timestamp"]:
        raise RuntimeError("source-event parent block timestamp must be before child")
    ancestor_headers = [
        _parse_solid_block_header(
            ancestor_response,
            label=f"source-event ancestor block[{index}]",
            expected_block_number=block_number - 2 - index,
            tx_trie_root_nonzero=True,
        )
        for index, ancestor_response in enumerate(ancestor_responses)
    ]
    confirmation_headers = [
        _parse_solid_block_header(
            confirmation_response,
            label=f"source-event confirmation block[{index}]",
            expected_block_number=block_number + 1 + index,
            tx_trie_root_nonzero=True,
        )
        for index, confirmation_response in enumerate(confirmation_responses)
    ]
    witness_weights = None
    if active_witness_schedule_payload is not None:
        witness_weights = {
            address: weight
            for address, weight in _decode_tron_witness_schedule_payload(
                active_witness_schedule_payload
            )
        }
    tx_trie_root = child_header["tx_trie_root"]
    transactions = response.get("transactions")
    if not isinstance(transactions, list) or not transactions:
        raise RuntimeError("source-event block did not include transactions")
    transaction_hashes = []
    transaction_index = None
    for index, transaction in enumerate(transactions):
        if not isinstance(transaction, dict):
            raise RuntimeError("source-event block transaction must be an object")
        tx_id = _parse_required_transaction_id_aliases(
            transaction,
            required_field="txID",
            label=f"source-event block transaction[{index}]",
        )
        transaction_bytes, raw_data_bytes = _tron_transaction_bytes_from_json(
            transaction,
            label=f"source-event block transaction[{index}]",
        )
        if hashlib.sha256(raw_data_bytes).digest() != tx_id:
            raise RuntimeError("source-event block transaction txID does not match raw_data")
        transaction_hash = hashlib.sha256(transaction_bytes).digest()
        transaction_hashes.append(transaction_hash)
        if tx_id == transaction_id:
            if transaction_index is not None:
                raise RuntimeError("source-event block contains duplicate transaction id")
            if transaction_bytes != source_transaction_bytes:
                raise RuntimeError(
                    "source-event block transaction bytes do not match transaction readback"
                )
            if transaction_hash != source_transaction_hash:
                raise RuntimeError(
                    "source-event block transaction hash does not match source proof hash"
                )
            transaction_index = index
    if transaction_index is None:
        raise RuntimeError("source-event block does not contain transaction id")
    calculated_root = _tron_merkle_root(transaction_hashes)
    if calculated_root != tx_trie_root:
        raise RuntimeError("source-event block txTrieRoot does not match transactions")
    transaction_merkle_branch = _tron_merkle_branch(
        transaction_hashes,
        transaction_index,
    )
    transaction_source_proof_summary, computed_receipt_proof_hash = (
        _source_event_transaction_source_proof_summary(
            source_event_digest=source_event_digest,
            receipt_root=receipt_root,
            transaction_root=tx_trie_root,
            transaction_index=transaction_index,
            transaction_count=len(transactions),
            transaction_bytes=source_transaction_bytes,
            transaction_merkle_branch=transaction_merkle_branch,
            source_inclusion_branch=source_inclusion_branch,
            source_bridge_address20=source_bridge_address20,
            owner_address20=owner_address20,
            expected_receipt_proof_hash=receipt_proof_hash,
        )
    )
    effective_receipt_proof_hash = receipt_proof_hash or computed_receipt_proof_hash
    return {
        "block_id": _hex(child_header["block_id"]),
        "block_number": child_header["number"],
        "block_timestamp": child_header["timestamp"],
        "block_parent_hash": _hex(child_header["parent_hash"]),
        "block_tx_trie_root": _hex(tx_trie_root),
        "block_account_state_root": (
            _hex(child_header["account_state_root"])
            if isinstance(child_header.get("account_state_root"), bytes)
            else None
        ),
        "block_witness_address": _hex(child_header["witness_address"]),
        "block_witness_signature": _hex(child_header["witness_signature"]),
        "block_witness_signature_recovered_address": _hex(
            child_header["witness_signature_recovered_address"]
        ),
        "block_header_raw_data_bytes": _hex(child_header["header_raw_data_bytes"]),
        "block_header_raw_data_hash": _hex(child_header["header_raw_data_hash"]),
        "block_id_matches_header": True,
        "parent_block_id": _hex(parent_header["block_id"]),
        "parent_block_number": parent_header["number"],
        "parent_block_timestamp": parent_header["timestamp"],
        "parent_block_tx_trie_root": _hex(parent_header["tx_trie_root"]),
        "parent_block_account_state_root": (
            _hex(parent_header["account_state_root"])
            if isinstance(parent_header.get("account_state_root"), bytes)
            else None
        ),
        "parent_block_witness_address": _hex(parent_header["witness_address"]),
        "parent_block_witness_signature": _hex(parent_header["witness_signature"]),
        "parent_block_witness_signature_recovered_address": _hex(
            parent_header["witness_signature_recovered_address"]
        ),
        "parent_block_header_raw_data_bytes": _hex(
            parent_header["header_raw_data_bytes"]
        ),
        "parent_block_header_raw_data_hash": _hex(
            parent_header["header_raw_data_hash"]
        ),
        "parent_block_id_matches_header": True,
        "parent_block_link_checked": True,
        "parent_timestamp_before_child": True,
        "transaction_count": len(transactions),
        "transaction_index": transaction_index,
        "transaction_merkle_branch": [
            _hex(sibling) for sibling in transaction_merkle_branch
        ],
        "transaction_merkle_branch_length": len(transaction_merkle_branch),
        "source_proof_transaction_hash": _hex(source_transaction_hash),
        "calculated_tx_trie_root": _hex(calculated_root),
        "tx_trie_root_matches": True,
        "block_transaction_root_checked": True,
        **transaction_source_proof_summary,
        **_source_event_solid_block_header_proof_summary(
            child_header,
            parent_header,
        ),
        **_source_event_witness_schedule_summary(
            active_witness_schedule_payload,
            child_witness_address=child_header["witness_address"],
            parent_witness_address=parent_header["witness_address"],
            expected_schedule_hash=expected_witness_schedule_hash,
            allow_expected_mismatch=bool(witness_schedule_transition_inputs),
        ),
        **_source_event_witness_schedule_transition_chain_summary(
            witness_schedule_transition_inputs,
            active_witness_schedule_payload=active_witness_schedule_payload,
            expected_schedule_hash=expected_witness_schedule_hash,
            child_header=child_header,
            parent_header=parent_header,
            ancestor_headers=ancestor_headers,
        ),
        **_source_event_witness_seal_summary(
            active_witness_schedule_payload,
            block_number=child_header["number"],
            block_hash=child_header["block_id"],
            transaction_root=tx_trie_root,
            receipt_root=receipt_root,
            receipt_proof_hash=effective_receipt_proof_hash,
            signers_bitmap=witness_seal_signers_bitmap,
            signatures=witness_seal_signatures,
            expected_seal_hash=expected_witness_seal_hash,
        ),
        **_source_event_ancestor_headers_summary(
            ancestor_headers,
            parent_header=parent_header,
            witness_weights=witness_weights,
        ),
        **_source_event_confirmation_headers_summary(
            confirmation_headers,
            child_header=child_header,
            witness_weights=witness_weights,
        ),
        "signed_header_proof_required": True,
    }


def _parse_log_address20(value: Any, *, label: str) -> bytes:
    if not isinstance(value, str):
        raise RuntimeError(f"{label} must be hex")
    raw_address = _parse_exact_hex_blob(value, label=label)
    if len(raw_address) == 21 and raw_address[0] == 0x41:
        address = raw_address[1:]
    else:
        address = raw_address
    if len(address) != 20:
        raise RuntimeError(f"{label} must be a 20-byte TRON log address")
    if not any(address):
        raise RuntimeError(f"{label} must not be zero")
    return address


def _parse_transaction_address_payload(value: Any, *, label: str) -> bytes:
    if not isinstance(value, str):
        raise RuntimeError(f"{label} must be a TRON address")
    try:
        return parse_tron_address_payload(value, label=label)
    except (argparse.ArgumentTypeError, TypeError, ValueError):
        raise RuntimeError(f"{label} is not a valid TRON address") from None


def _source_event_trigger_contract_summary(
    response: dict[str, Any],
    *,
    transaction_id: bytes,
    source_bridge_payload: bytes,
    owner_payload: bytes,
    source_event_call_data: bytes,
) -> dict[str, Any]:
    parsed_id = _parse_transaction_id_field(
        response,
        expected_transaction_id=transaction_id,
        label="source-event transaction",
    )
    raw_data = _parse_exact_hex_blob(
        response.get("raw_data_hex"),
        label="source-event transaction raw_data_hex",
    )
    signature = _parse_source_event_transaction_signature(response)
    raw_data_summary = _source_event_transaction_raw_data_summary(
        {"raw_data_hex": _hex(raw_data)},
        transaction_id=transaction_id,
        source_bridge_payload=source_bridge_payload,
        owner_payload=owner_payload,
        source_event_call_data=source_event_call_data,
    )
    signature_summary = _source_event_transaction_signature_summary(
        signature,
        raw_data_hash=transaction_id,
        owner_payload=owner_payload,
    )
    ret = response.get("ret")
    if not isinstance(ret, list) or len(ret) != 1 or not isinstance(ret[0], dict):
        raise RuntimeError("source-event transaction must contain one ret result")
    contract_ret = ret[0].get("contractRet")
    if not _source_event_contract_ret_is_success(contract_ret):
        raise RuntimeError("source-event transaction contractRet must be SUCCESS")
    contract_ret = "SUCCESS"
    transaction_bytes_summary = _source_event_transaction_bytes_summary(
        raw_data,
        signature,
        ret[0],
    )
    raw_data = response.get("raw_data")
    if not isinstance(raw_data, dict):
        raise RuntimeError("TRON transaction did not return raw_data")
    contracts = raw_data.get("contract")
    if not isinstance(contracts, list) or len(contracts) != 1:
        raise RuntimeError("source-event transaction must contain one contract")
    contract = contracts[0]
    if not isinstance(contract, dict):
        raise RuntimeError("source-event transaction contract must be an object")
    if contract.get("type") != "TriggerSmartContract":
        raise RuntimeError("source-event transaction must be TriggerSmartContract")
    parameter = contract.get("parameter")
    if not isinstance(parameter, dict):
        raise RuntimeError("source-event transaction contract parameter is missing")
    type_url = parameter.get("type_url")
    if type_url != "type.googleapis.com/protocol.TriggerSmartContract":
        raise RuntimeError("source-event transaction TriggerSmartContract type_url mismatch")
    value = parameter.get("value")
    if not isinstance(value, dict):
        raise RuntimeError("source-event transaction TriggerSmartContract value is missing")
    owner = _parse_transaction_address_payload(
        value.get("owner_address"),
        label="source-event transaction owner_address",
    )
    if owner != owner_payload:
        raise RuntimeError("source-event transaction owner_address does not match source bridge owner")
    contract_address = _parse_transaction_address_payload(
        value.get("contract_address"),
        label="source-event transaction contract_address",
    )
    if contract_address != source_bridge_payload:
        raise RuntimeError("source-event transaction contract_address does not match source bridge")
    data = value.get("data")
    if not isinstance(data, str):
        raise RuntimeError("source-event transaction data must be hex")
    call_data = _parse_exact_hex_blob(data, label="source-event transaction data")
    if call_data != source_event_call_data:
        raise RuntimeError("source-event transaction calldata does not match source-event digest")
    return {
        "transaction_id": parsed_id,
        **raw_data_summary,
        **signature_summary,
        **transaction_bytes_summary,
        "contract_ret": contract_ret,
        "contract_type": "TriggerSmartContract",
        "type_url": type_url,
        "owner_address": _hex(owner),
        "owner_base58": tron_base58check_from_payload(owner),
        "contract_address": _hex(contract_address),
        "contract_base58": tron_base58check_from_payload(contract_address),
        "call_data": _hex(call_data),
        "call_matches": True,
    }


def _source_event_transaction_summary(
    response: dict[str, Any],
    *,
    transaction_id: bytes,
    source_bridge_address20: bytes,
    source_event_digest: bytes,
) -> dict[str, Any]:
    parsed_id = _parse_transaction_info_id(
        response,
        expected_transaction_id=transaction_id,
        label="source-event transaction info",
    )
    receipt = response.get("receipt")
    receipt_status = receipt.get("result") if isinstance(receipt, dict) else None
    if receipt_status != "SUCCESS":
        raise RuntimeError("source-event transaction receipt status must be SUCCESS")
    logs = response.get("log")
    if not isinstance(logs, list):
        raise RuntimeError("source-event transaction info returned no log list")
    matching_summary: dict[str, Any] | None = None
    for index, log in enumerate(logs):
        if not isinstance(log, dict):
            continue
        try:
            log_address = _parse_log_address20(
                log.get("address"),
                label="source-event log address",
            )
        except RuntimeError:
            continue
        topics = log.get("topics")
        if not isinstance(topics, list) or len(topics) != 2:
            continue
        if not all(isinstance(topic, str) for topic in topics):
            continue
        try:
            topic0 = _parse_exact_hex32(topics[0], label="source-event log topic0")
            topic1 = _parse_exact_hex32(topics[1], label="source-event log topic1")
        except (argparse.ArgumentTypeError, TypeError, RuntimeError, ValueError):
            continue
        data = log.get("data", "")
        if not isinstance(data, str):
            continue
        if data != data.strip():
            continue
        try:
            event_data = _parse_exact_hex_blob(
                data,
                label="source-event log data",
                nonzero=False,
            )
        except RuntimeError:
            continue
        if event_data != b"":
            continue
        if (
            log_address == source_bridge_address20
            and topic0 == TRON_SOURCE_EVENT_TOPIC
            and topic1 == source_event_digest
        ):
            _check_optional_log_index_field(
                log,
                expected_index=index,
                label="source-event log",
            )
            summary: dict[str, Any] = {
                "transaction_id": parsed_id,
                "receipt_status": receipt_status,
                "log_index": index,
                "event_address": _hex(log_address),
                "event_topic0": _hex(topic0),
                "source_event_digest": _hex(topic1),
                "event_data": _hex(event_data),
                "event_matches": True,
            }
            if matching_summary is not None:
                raise RuntimeError(
                    "source-event transaction log must contain exactly one "
                    "matching SccpSourceEvent(bytes32) event"
                )
            matching_summary = summary
    if matching_summary is None:
        raise RuntimeError(
            "source-event transaction log did not contain the expected "
            "SccpSourceEvent(bytes32) event"
        )
    matching_summary["block_number"] = _required_transaction_info_block_number(
        response,
        label="source-event transaction info",
    )
    matching_summary["block_timestamp"] = _required_transaction_info_block_timestamp(
        response,
        label="source-event transaction info",
    )
    return matching_summary


def _parse_abi_data_words(value: Any, *, label: str, word_count: int) -> tuple[bytes, ...]:
    data = _parse_exact_hex_blob(value, label=label, nonzero=False)
    expected_len = 32 * word_count
    if len(data) != expected_len:
        raise RuntimeError(f"{label} must contain {word_count} ABI words")
    return tuple(data[index : index + 32] for index in range(0, expected_len, 32))


def _require_nonzero_bytes(value: bytes, *, label: str, byte_length: int) -> bytes:
    if not isinstance(value, (bytes, bytearray)):
        raise RuntimeError(f"{label} must be {byte_length} bytes")
    raw = bytes(value)
    if len(raw) != byte_length:
        raise RuntimeError(f"{label} must be {byte_length} bytes")
    if not any(raw):
        raise RuntimeError(f"{label} must not be zero")
    return raw


def _require_nonzero_word(word: bytes, *, label: str) -> bytes:
    return _require_nonzero_bytes(word, label=label, byte_length=32)


def _require_tron_payload_address(value: bytes, *, label: str) -> bytes:
    raw = _require_nonzero_bytes(value, label=label, byte_length=21)
    if raw[0] != 0x41 or not any(raw[1:]):
        raise RuntimeError(f"{label} must be a non-zero 0x41-prefixed TRON address")
    return raw


def _word_u256(word: bytes, *, label: str) -> int:
    if len(word) != 32:
        raise RuntimeError(f"{label} must be 32 bytes")
    return int.from_bytes(word, "big")


def _require_u32_value(value: int, *, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise RuntimeError(f"{label} must be a u32")
    if value < 0 or value > 0xFFFF_FFFF:
        raise RuntimeError(f"{label} must be a u32")
    return value


def _require_u64_value(value: int, *, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise RuntimeError(f"{label} must be a u64")
    if value < 0 or value > 0xFFFF_FFFF_FFFF_FFFF:
        raise RuntimeError(f"{label} must be a u64")
    return value


def _tron_route_canary_transaction_evidence_hash(
    *,
    route_allowlist_hash: bytes,
    transaction_id: bytes,
    transaction_owner_address: bytes,
    block_number: int,
    block_timestamp: int,
    log_index: int,
    verifier_address20: bytes,
    call_data_sha256: bytes,
    message_id: bytes,
    source_domain: int,
    target_domain: int,
    payload_hash: bytes,
    commitment_root: bytes,
    finality_height: bytes,
    finality_block_hash: bytes,
    statement_hash: bytes,
    proof_version: int,
    proof_source_domain: int,
    destination_binding_hash: bytes,
    verifier_backend_hash: bytes,
    proof_family_hash: bytes,
    network_id: bytes,
    used_message_proof: bool,
    raw_data_owner_matches_transaction: bool,
    signature_sha256: bytes,
    signature_recovered_address: bytes,
    signature_recovers_to_owner: bool,
) -> bytes:
    route_allowlist_hash = _require_nonzero_word(
        route_allowlist_hash,
        label="route-canary route allowlist hash",
    )
    transaction_id = _require_nonzero_word(
        transaction_id,
        label="route-canary transaction id",
    )
    transaction_owner_address = _require_tron_payload_address(
        transaction_owner_address,
        label="route-canary transaction owner address",
    )
    block_number = _require_u64_value(
        block_number,
        label="route-canary block number",
    )
    if block_number == 0:
        raise RuntimeError("route-canary block number must be a positive u64")
    block_timestamp = _require_u64_value(
        block_timestamp,
        label="route-canary block timestamp",
    )
    log_index = _require_u32_value(
        log_index,
        label="route-canary log index",
    )
    verifier_address20 = _require_nonzero_bytes(
        verifier_address20,
        label="route-canary verifier address",
        byte_length=20,
    )
    call_data_sha256 = _require_nonzero_word(
        call_data_sha256,
        label="route-canary call data SHA-256",
    )
    message_id = _require_nonzero_word(
        message_id,
        label="route-canary message id",
    )
    source_domain = _require_u32_value(
        source_domain,
        label="route-canary source domain",
    )
    target_domain = _require_u32_value(
        target_domain,
        label="route-canary target domain",
    )
    if source_domain != evidence.SCCP_DOMAIN_SORA:
        raise RuntimeError("route-canary source domain must be SORA")
    if target_domain != evidence.SCCP_DOMAIN_TRON:
        raise RuntimeError("route-canary target domain must be TRON")
    payload_hash = _require_nonzero_word(
        payload_hash,
        label="route-canary payload hash",
    )
    commitment_root = _require_nonzero_word(
        commitment_root,
        label="route-canary commitment root",
    )
    finality_height = _require_nonzero_word(
        finality_height,
        label="route-canary finality height",
    )
    finality_block_hash = _require_nonzero_word(
        finality_block_hash,
        label="route-canary finality block hash",
    )
    statement_hash = _require_nonzero_word(
        statement_hash,
        label="route-canary statement hash",
    )
    proof_version = _require_u32_value(
        proof_version,
        label="route-canary proof version",
    )
    proof_source_domain = _require_u32_value(
        proof_source_domain,
        label="route-canary proof source domain",
    )
    if proof_version != TRON_GROTH16_PROOF_VERSION:
        raise RuntimeError("route-canary proof version must be 1")
    if proof_source_domain != evidence.SCCP_DOMAIN_SORA:
        raise RuntimeError("route-canary proof source domain must be SORA")
    destination_binding_hash = _require_nonzero_word(
        destination_binding_hash,
        label="route-canary destination binding hash",
    )
    verifier_backend_hash = _require_nonzero_word(
        verifier_backend_hash,
        label="route-canary verifier backend hash",
    )
    proof_family_hash = _require_nonzero_word(
        proof_family_hash,
        label="route-canary proof family hash",
    )
    network_id = _require_nonzero_word(
        network_id,
        label="route-canary network id",
    )
    if used_message_proof is not True:
        raise RuntimeError("route-canary usedMessageProofs witness must be true")
    if raw_data_owner_matches_transaction is not True:
        raise RuntimeError("route-canary raw_data owner must match transaction owner")
    signature_sha256 = _require_nonzero_word(
        signature_sha256,
        label="route-canary signature hash",
    )
    signature_recovered_address = _require_tron_payload_address(
        signature_recovered_address,
        label="route-canary signature recovered address",
    )
    if signature_recovered_address != transaction_owner_address:
        raise RuntimeError(
            "route-canary signature recovered address must match transaction owner"
        )
    if signature_recovers_to_owner is not True:
        raise RuntimeError("route-canary signature recovery witness must be true")

    payload = bytearray()
    evidence._push_u8(payload, 3)
    payload.extend(route_allowlist_hash)
    payload.extend(b"\x41" + verifier_address20)
    payload.extend(transaction_id)
    payload.extend(transaction_owner_address)
    evidence._push_u64(payload, block_number)
    evidence._push_u64(payload, block_timestamp)
    evidence._push_u32(payload, log_index)
    payload.extend(call_data_sha256)
    payload.extend(message_id)
    evidence._push_u32(payload, source_domain)
    evidence._push_u32(payload, target_domain)
    payload.extend(payload_hash)
    payload.extend(commitment_root)
    payload.extend(finality_height)
    payload.extend(finality_block_hash)
    payload.extend(statement_hash)
    evidence._push_u32(payload, proof_version)
    evidence._push_u32(payload, proof_source_domain)
    payload.extend(destination_binding_hash)
    payload.extend(verifier_backend_hash)
    payload.extend(proof_family_hash)
    payload.extend(network_id)
    evidence._push_u8(payload, 1 if used_message_proof else 0)
    evidence._push_u8(payload, 1 if raw_data_owner_matches_transaction else 0)
    payload.extend(signature_sha256)
    payload.extend(signature_recovered_address)
    evidence._push_u8(payload, 1 if signature_recovers_to_owner else 0)
    return evidence._prefixed_blake2b(TRON_ROUTE_CANARY_EVIDENCE_LABEL, payload)


def _route_canary_message_proof_event_summary(
    log: dict[str, Any],
    *,
    log_index: int,
    transaction_id: bytes,
    route_allowlist_hash: bytes,
    verifier_address20: bytes,
    expected_source_domain: int,
    expected_destination_binding_hash: bytes,
    expected_verifier_backend_hash: bytes,
    expected_proof_family_hash: bytes,
    expected_network_id: bytes,
) -> dict[str, Any] | None:
    try:
        log_address = _parse_log_address20(
            log.get("address"),
            label="route-canary log address",
        )
    except RuntimeError:
        return None
    topics = log.get("topics")
    if not isinstance(topics, list) or not topics:
        return None
    if not all(isinstance(topic, str) for topic in topics):
        return None
    try:
        topic0 = _parse_exact_hex32(topics[0], label="route-canary log topic0")
    except (argparse.ArgumentTypeError, TypeError, RuntimeError, ValueError):
        return None
    if log_address != verifier_address20 or topic0 != TRON_MESSAGE_PROOF_ACCEPTED_TOPIC:
        return None
    if len(topics) != 3:
        raise RuntimeError(
            "route-canary MessageProofAccepted log must contain exactly three topics"
        )
    _check_optional_log_index_field(
        log,
        expected_index=log_index,
        label="route-canary MessageProofAccepted log",
    )
    message_id = _require_nonzero_word(
        _parse_exact_hex32(topics[1], label="route-canary messageId topic"),
        label="route-canary messageId",
    )
    source_domain_word = _parse_exact_hex32_blob(
        topics[2],
        label="route-canary sourceDomain topic",
        nonzero=False,
    )
    source_domain = _word_u32(
        source_domain_word,
        label="route-canary sourceDomain topic",
    )
    if source_domain != expected_source_domain:
        raise RuntimeError(
            "route-canary MessageProofAccepted sourceDomain does not match "
            "expectedSourceDomain(): "
            f"expected {expected_source_domain}, got {source_domain}"
        )
    (
        commitment_root,
        statement_hash,
        destination_binding_hash,
        verifier_backend_hash,
        proof_family_hash,
        network_id,
    ) = _parse_abi_data_words(
        log.get("data"),
        label="route-canary MessageProofAccepted data",
        word_count=6,
    )
    commitment_root = _require_nonzero_word(
        commitment_root,
        label="route-canary commitmentRoot",
    )
    statement_hash = _require_nonzero_word(
        statement_hash,
        label="route-canary statementHash",
    )
    destination_binding_hash = _require_nonzero_word(
        destination_binding_hash,
        label="route-canary destinationBindingHash",
    )
    verifier_backend_hash = _require_nonzero_word(
        verifier_backend_hash,
        label="route-canary verifierBackendHash",
    )
    proof_family_hash = _require_nonzero_word(
        proof_family_hash,
        label="route-canary proofFamilyHash",
    )
    network_id = _require_nonzero_word(network_id, label="route-canary networkId")
    if destination_binding_hash != expected_destination_binding_hash:
        raise RuntimeError(
            "route-canary MessageProofAccepted destinationBindingHash does not "
            "match live destinationBindingHash()"
        )
    if verifier_backend_hash != expected_verifier_backend_hash:
        raise RuntimeError(
            "route-canary MessageProofAccepted verifierBackendHash does not "
            "match verifierBackendHash()"
        )
    if proof_family_hash != expected_proof_family_hash:
        raise RuntimeError(
            "route-canary MessageProofAccepted proofFamilyHash does not match "
            "proofFamilyHash()"
        )
    if network_id != expected_network_id:
        raise RuntimeError(
            "route-canary MessageProofAccepted networkId does not match networkId()"
        )
    return {
        "transaction_id": _hex(transaction_id),
        "log_index": log_index,
        "event_address": _hex(log_address),
        "event_topic0": _hex(topic0),
        "message_id": _hex(message_id),
        "source_domain": source_domain,
        "commitment_root": _hex(commitment_root),
        "statement_hash": _hex(statement_hash),
        "destination_binding_hash": _hex(destination_binding_hash),
        "verifier_backend_hash": _hex(verifier_backend_hash),
        "proof_family_hash": _hex(proof_family_hash),
        "network_id": _hex(network_id),
        "route_allowlist_hash": _hex(route_allowlist_hash),
        "event_matches": True,
    }


def _route_canary_submit_call_data_summary(
    call_data: bytes,
    *,
    event_summary: dict[str, Any],
    expected_source_domain: int,
    expected_target_domain: int,
) -> dict[str, Any]:
    if not call_data.startswith(TRON_SUBMIT_MESSAGE_PROOF_SELECTOR):
        raise RuntimeError(
            "route-canary transaction calldata must call "
            "submitSccpMessageProof(bytes,bytes32[6],bytes32)"
        )
    call_data_sha256 = hashlib.sha256(call_data).digest()
    body = call_data[len(TRON_SUBMIT_MESSAGE_PROOF_SELECTOR) :]
    if len(body) < 32 * 9 or len(body) % 32 != 0:
        raise RuntimeError("route-canary submit calldata has invalid ABI length")
    offset = _word_u256(body[0:32], label="route-canary proofBytes offset")
    if offset != 32 * 8:
        raise RuntimeError(
            "route-canary submit calldata proofBytes offset must be 256 bytes"
        )
    if offset + 32 > len(body):
        raise RuntimeError("route-canary submit calldata proofBytes is truncated")
    public_inputs = tuple(body[index : index + 32] for index in range(32, 32 * 7, 32))
    statement_hash = body[32 * 7 : 32 * 8]
    proof_len = _word_u256(
        body[offset : offset + 32],
        label="route-canary proofBytes length",
    )
    proof_start = offset + 32
    proof_end = proof_start + proof_len
    if proof_end > len(body):
        raise RuntimeError("route-canary submit calldata proofBytes is truncated")
    padding_len = (32 - (proof_len % 32)) % 32
    if proof_end + padding_len != len(body):
        raise RuntimeError("route-canary submit calldata has trailing ABI data")
    if any(body[proof_end:]):
        raise RuntimeError("route-canary submit calldata proofBytes padding must be zero")
    proof_bytes = body[proof_start:proof_end]
    if proof_len != TRON_GROTH16_PROOF_ABI_BYTE_LENGTH:
        raise RuntimeError("route-canary proofBytes must be a 384-byte Groth16 tuple")
    if not any(proof_bytes):
        raise RuntimeError("route-canary proofBytes must not be all zero")
    message_id = _parse_hex32(
        str(event_summary["message_id"]),
        label="route-canary event message id",
    )
    commitment_root = _parse_hex32(
        str(event_summary["commitment_root"]),
        label="route-canary event commitment root",
    )
    event_statement_hash = _parse_hex32(
        str(event_summary["statement_hash"]),
        label="route-canary event statement hash",
    )
    if public_inputs[0] != message_id:
        raise RuntimeError(
            "route-canary submit calldata publicInputs[0] must match event messageId"
        )
    _require_nonzero_word(public_inputs[1], label="route-canary payloadHash")
    target_domain = _word_u32(
        public_inputs[2],
        label="route-canary publicInputs targetDomain",
    )
    if target_domain != expected_target_domain:
        raise RuntimeError(
            "route-canary submit calldata targetDomain does not match "
            "expectedTargetDomain()"
        )
    if public_inputs[3] != commitment_root:
        raise RuntimeError(
            "route-canary submit calldata publicInputs[3] must match event commitmentRoot"
        )
    _require_nonzero_word(public_inputs[4], label="route-canary finalityHeight")
    _require_nonzero_word(public_inputs[5], label="route-canary finalityBlockHash")
    if statement_hash != event_statement_hash:
        raise RuntimeError(
            "route-canary submit calldata statementHash must match accepted event"
        )
    proof_words = tuple(
        proof_bytes[index : index + 32]
        for index in range(0, TRON_GROTH16_PROOF_ABI_BYTE_LENGTH, 32)
    )
    proof_version = _word_u256(
        proof_words[0],
        label="route-canary proof version",
    )
    if proof_version != TRON_GROTH16_PROOF_VERSION:
        raise RuntimeError("route-canary proof version must be 1")
    if proof_words[1] != message_id:
        raise RuntimeError("route-canary proof message id must match event messageId")
    proof_source_domain = _word_u32(
        proof_words[2],
        label="route-canary proof sourceDomain",
    )
    if proof_source_domain != expected_source_domain:
        raise RuntimeError(
            "route-canary proof sourceDomain does not match expectedSourceDomain()"
        )
    if proof_words[3] != commitment_root:
        raise RuntimeError(
            "route-canary proof commitmentRoot must match accepted event"
        )
    return {
        "function_selector": _hex(TRON_SUBMIT_MESSAGE_PROOF_SELECTOR),
        "function_signature": "submitSccpMessageProof(bytes,bytes32[6],bytes32)",
        "call_data_sha256": _hex(call_data_sha256),
        "call_data_matches_event": True,
        "proof_bytes_length": proof_len,
        "proof_version": proof_version,
        "proof_source_domain": proof_source_domain,
        "public_inputs_message_id": _hex(public_inputs[0]),
        "public_inputs_payload_hash": _hex(public_inputs[1]),
        "public_inputs_target_domain": target_domain,
        "public_inputs_commitment_root": _hex(public_inputs[3]),
        "public_inputs_finality_height": _hex(public_inputs[4]),
        "public_inputs_finality_block_hash": _hex(public_inputs[5]),
        "statement_hash": _hex(statement_hash),
        "call_data": _hex(call_data),
    }


def _route_canary_trigger_from_raw_data_summary(
    trigger: bytes,
    *,
    verifier_payload: bytes,
    expected_call_data: bytes,
) -> dict[str, Any]:
    cursor = 0
    owner = None
    contract_address = None
    call_data = None
    call_value_seen = False
    call_token_value_seen = False
    token_id_seen = False
    while cursor < len(trigger):
        key, cursor = _read_protobuf_varint_at(
            trigger,
            cursor,
            label="route-canary transaction raw_data_hex TriggerSmartContract",
        )
        field_number = key >> 3
        wire_type = key & 0x07
        if field_number == 1 and wire_type == 2 and owner is None:
            raw_owner, cursor = _read_protobuf_bytes_field(
                trigger,
                cursor,
                label="route-canary transaction raw_data_hex owner_address",
            )
            owner = _parse_raw_data_tron_payload(
                raw_owner,
                label="route-canary transaction raw_data_hex owner_address",
            )
        elif field_number == 2 and wire_type == 2 and contract_address is None:
            raw_contract, cursor = _read_protobuf_bytes_field(
                trigger,
                cursor,
                label="route-canary transaction raw_data_hex contract_address",
            )
            contract_address = _parse_raw_data_tron_payload(
                raw_contract,
                label="route-canary transaction raw_data_hex contract_address",
            )
            if contract_address != verifier_payload:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex contract_address "
                    "does not match destination verifier"
                )
        elif field_number == 3 and wire_type == 0 and not call_value_seen:
            call_value_seen = True
            value, cursor = _read_protobuf_varint_at(
                trigger,
                cursor,
                label="route-canary transaction raw_data_hex call_value",
            )
            if value != 0:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex call_value must be zero"
                )
        elif field_number == 4 and wire_type == 2 and call_data is None:
            call_data, cursor = _read_protobuf_bytes_field(
                trigger,
                cursor,
                label="route-canary transaction raw_data_hex data",
            )
            if call_data != expected_call_data:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex calldata does not "
                    "match submitSccpMessageProof call"
                )
        elif field_number == 5 and wire_type == 0 and not call_token_value_seen:
            call_token_value_seen = True
            value, cursor = _read_protobuf_varint_at(
                trigger,
                cursor,
                label="route-canary transaction raw_data_hex call_token_value",
            )
            if value != 0:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex call_token_value must be zero"
                )
        elif field_number == 6 and wire_type == 0 and not token_id_seen:
            token_id_seen = True
            value, cursor = _read_protobuf_varint_at(
                trigger,
                cursor,
                label="route-canary transaction raw_data_hex token_id",
            )
            if value != 0:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex token_id must be zero"
                )
        else:
            raise RuntimeError(
                "route-canary transaction raw_data_hex TriggerSmartContract "
                "contains unsupported field"
            )
    if owner is None or contract_address is None or call_data is None:
        raise RuntimeError(
            "route-canary transaction raw_data_hex TriggerSmartContract is incomplete"
        )
    return {
        "raw_data_type_url": TRON_TRIGGER_SMART_CONTRACT_TYPE_URL.decode("ascii"),
        "raw_data_owner_address": _hex(owner),
        "raw_data_owner_base58": tron_base58check_from_payload(owner),
        "raw_data_contract_address": _hex(contract_address),
        "raw_data_contract_base58": tron_base58check_from_payload(contract_address),
        "raw_data_call_data": _hex(call_data),
    }


def _route_canary_any_from_raw_data_summary(
    parameter: bytes,
    *,
    verifier_payload: bytes,
    expected_call_data: bytes,
) -> dict[str, Any]:
    cursor = 0
    type_url = None
    value = None
    while cursor < len(parameter):
        key, cursor = _read_protobuf_varint_at(
            parameter,
            cursor,
            label="route-canary transaction raw_data_hex Any",
        )
        field_number = key >> 3
        wire_type = key & 0x07
        if field_number == 1 and wire_type == 2 and type_url is None:
            type_url, cursor = _read_protobuf_bytes_field(
                parameter,
                cursor,
                label="route-canary transaction raw_data_hex Any type_url",
            )
        elif field_number == 2 and wire_type == 2 and value is None:
            value, cursor = _read_protobuf_bytes_field(
                parameter,
                cursor,
                label="route-canary transaction raw_data_hex Any value",
            )
        else:
            raise RuntimeError(
                "route-canary transaction raw_data_hex Any contains unsupported field"
            )
    if type_url != TRON_TRIGGER_SMART_CONTRACT_TYPE_URL or value is None:
        raise RuntimeError("route-canary transaction raw_data_hex Any type_url mismatch")
    return _route_canary_trigger_from_raw_data_summary(
        value,
        verifier_payload=verifier_payload,
        expected_call_data=expected_call_data,
    )


def _route_canary_contract_from_raw_data_summary(
    contract: bytes,
    *,
    verifier_payload: bytes,
    expected_call_data: bytes,
) -> dict[str, Any]:
    cursor = 0
    contract_type = None
    parameter = None
    while cursor < len(contract):
        key, cursor = _read_protobuf_varint_at(
            contract,
            cursor,
            label="route-canary transaction raw_data_hex Contract",
        )
        field_number = key >> 3
        wire_type = key & 0x07
        if field_number == 1 and wire_type == 0 and contract_type is None:
            contract_type, cursor = _read_protobuf_varint_at(
                contract,
                cursor,
                label="route-canary transaction raw_data_hex Contract type",
            )
        elif field_number == 2 and wire_type == 2 and parameter is None:
            parameter, cursor = _read_protobuf_bytes_field(
                contract,
                cursor,
                label="route-canary transaction raw_data_hex Contract parameter",
            )
        else:
            raise RuntimeError(
                "route-canary transaction raw_data_hex Contract contains unsupported field"
            )
    if contract_type != 31 or parameter is None:
        raise RuntimeError(
            "route-canary transaction raw_data_hex contract must be TriggerSmartContract"
        )
    return _route_canary_any_from_raw_data_summary(
        parameter,
        verifier_payload=verifier_payload,
        expected_call_data=expected_call_data,
    )


def _route_canary_raw_data_call_summary(
    raw_data: bytes,
    *,
    verifier_payload: bytes,
    expected_call_data: bytes,
) -> dict[str, Any]:
    cursor = 0
    ref_block_bytes = None
    ref_block_num_seen = False
    ref_block_hash = None
    expiration = None
    timestamp = None
    fee_limit = None
    contract_count = 0
    contract_summary = None
    while cursor < len(raw_data):
        key, cursor = _read_protobuf_varint_at(
            raw_data,
            cursor,
            label="route-canary transaction raw_data_hex",
        )
        field_number = key >> 3
        wire_type = key & 0x07
        if field_number == 1 and wire_type == 2 and ref_block_bytes is None:
            ref_block_bytes, cursor = _read_protobuf_bytes_field(
                raw_data,
                cursor,
                label="route-canary transaction raw_data_hex ref_block_bytes",
            )
            if len(ref_block_bytes) != 2 or not any(ref_block_bytes):
                raise RuntimeError(
                    "route-canary transaction raw_data_hex ref_block_bytes "
                    "must be non-zero 2-byte data"
                )
        elif field_number == 3 and wire_type == 0 and not ref_block_num_seen:
            ref_block_num_seen = True
            _, cursor = _read_protobuf_varint_at(
                raw_data,
                cursor,
                label="route-canary transaction raw_data_hex ref_block_num",
            )
        elif field_number == 4 and wire_type == 2 and ref_block_hash is None:
            ref_block_hash, cursor = _read_protobuf_bytes_field(
                raw_data,
                cursor,
                label="route-canary transaction raw_data_hex ref_block_hash",
            )
            if len(ref_block_hash) != 8 or not any(ref_block_hash):
                raise RuntimeError(
                    "route-canary transaction raw_data_hex ref_block_hash "
                    "must be non-zero 8-byte data"
                )
        elif field_number == 8 and wire_type == 0 and expiration is None:
            expiration, cursor = _read_protobuf_varint_at(
                raw_data,
                cursor,
                label="route-canary transaction raw_data_hex expiration",
            )
            if expiration == 0:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex expiration must be non-zero"
                )
        elif field_number == 11 and wire_type == 2:
            contract_count += 1
            if contract_count > 1:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex must contain one contract"
                )
            contract, cursor = _read_protobuf_bytes_field(
                raw_data,
                cursor,
                label="route-canary transaction raw_data_hex contract",
            )
            contract_summary = _route_canary_contract_from_raw_data_summary(
                contract,
                verifier_payload=verifier_payload,
                expected_call_data=expected_call_data,
            )
        elif field_number == 14 and wire_type == 0 and timestamp is None:
            timestamp, cursor = _read_protobuf_varint_at(
                raw_data,
                cursor,
                label="route-canary transaction raw_data_hex timestamp",
            )
            if timestamp == 0:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex timestamp must be non-zero"
                )
        elif field_number == 18 and wire_type == 0 and fee_limit is None:
            fee_limit, cursor = _read_protobuf_varint_at(
                raw_data,
                cursor,
                label="route-canary transaction raw_data_hex fee_limit",
            )
            if fee_limit == 0:
                raise RuntimeError(
                    "route-canary transaction raw_data_hex fee_limit must be non-zero"
                )
        else:
            raise RuntimeError(
                "route-canary transaction raw_data_hex contains unsupported field"
            )
    if (
        ref_block_bytes is None
        or ref_block_hash is None
        or expiration is None
        or timestamp is None
        or fee_limit is None
        or contract_count != 1
        or contract_summary is None
    ):
        raise RuntimeError("route-canary transaction raw_data_hex is incomplete")
    if expiration <= timestamp:
        raise RuntimeError(
            "route-canary transaction raw_data_hex expiration must be after timestamp"
        )
    return {
        "raw_data_submit_call_matches": True,
        "raw_data_ref_block_bytes": _hex(ref_block_bytes),
        "raw_data_ref_block_hash": _hex(ref_block_hash),
        "raw_data_expiration": expiration,
        "raw_data_timestamp": timestamp,
        "raw_data_fee_limit": fee_limit,
        **contract_summary,
    }


def _route_canary_transaction_raw_data_summary(
    response: dict[str, Any],
    *,
    transaction_id: bytes,
    verifier_payload: bytes,
    expected_call_data: bytes,
) -> dict[str, Any]:
    raw_data = _parse_exact_hex_blob(
        response.get("raw_data_hex"),
        label="route-canary transaction raw_data_hex",
    )
    raw_data_hash = hashlib.sha256(raw_data).digest()
    if raw_data_hash != transaction_id:
        raise RuntimeError(
            "route-canary transaction raw_data_hex SHA-256 does not match txID"
        )
    return {
        "raw_data_hex": _hex(raw_data),
        "raw_data_sha256": _hex(raw_data_hash),
        "transaction_id_matches_raw_data": True,
        **_route_canary_raw_data_call_summary(
            raw_data,
            verifier_payload=verifier_payload,
            expected_call_data=expected_call_data,
        ),
    }


def _route_canary_trigger_contract_summary(
    response: dict[str, Any],
    *,
    transaction_id: bytes,
    destination_verifier: dict[str, Any],
    event_summary: dict[str, Any],
) -> dict[str, Any]:
    parsed_id = _parse_transaction_id_field(
        response,
        expected_transaction_id=transaction_id,
        label="route-canary transaction",
    )
    verifier_payload = parse_tron_address_payload(
        str(destination_verifier["address"]),
        label="destination verifier address",
    )
    expected_source_domain = destination_verifier.get("destination_source_domain")
    expected_target_domain = destination_verifier.get("destination_target_domain")
    if type(expected_source_domain) is not int or type(expected_target_domain) is not int:
        raise RuntimeError("destination verifier domains must be integers")
    raw_data = _parse_exact_hex_blob(
        response.get("raw_data_hex"),
        label="route-canary transaction raw_data_hex",
    )
    signature = _parse_route_canary_transaction_signature(response)
    ret = response.get("ret")
    if not isinstance(ret, list) or len(ret) != 1 or not isinstance(ret[0], dict):
        raise RuntimeError("route-canary transaction must contain one ret result")
    contract_ret = ret[0].get("contractRet")
    if not _source_event_contract_ret_is_success(contract_ret):
        raise RuntimeError("route-canary transaction contractRet must be SUCCESS")
    raw_data_obj = response.get("raw_data")
    if not isinstance(raw_data_obj, dict):
        raise RuntimeError("TRON route-canary transaction did not return raw_data")
    contracts = raw_data_obj.get("contract")
    if not isinstance(contracts, list) or len(contracts) != 1:
        raise RuntimeError("route-canary transaction must contain one contract")
    contract = contracts[0]
    if not isinstance(contract, dict):
        raise RuntimeError("route-canary transaction contract must be an object")
    if contract.get("type") != "TriggerSmartContract":
        raise RuntimeError("route-canary transaction must be TriggerSmartContract")
    parameter = contract.get("parameter")
    if not isinstance(parameter, dict):
        raise RuntimeError("route-canary transaction contract parameter is missing")
    type_url = parameter.get("type_url")
    if type_url != "type.googleapis.com/protocol.TriggerSmartContract":
        raise RuntimeError(
            "route-canary transaction TriggerSmartContract type_url mismatch"
        )
    value = parameter.get("value")
    if not isinstance(value, dict):
        raise RuntimeError(
            "route-canary transaction TriggerSmartContract value is missing"
        )
    owner = _parse_transaction_address_payload(
        value.get("owner_address"),
        label="route-canary transaction owner_address",
    )
    contract_address = _parse_transaction_address_payload(
        value.get("contract_address"),
        label="route-canary transaction contract_address",
    )
    if contract_address != verifier_payload:
        raise RuntimeError(
            "route-canary transaction contract_address does not match destination verifier"
        )
    data = value.get("data")
    if not isinstance(data, str):
        raise RuntimeError("route-canary transaction data must be hex")
    call_data = _parse_exact_hex_blob(data, label="route-canary transaction data")
    call_summary = _route_canary_submit_call_data_summary(
        call_data,
        event_summary=event_summary,
        expected_source_domain=expected_source_domain,
        expected_target_domain=expected_target_domain,
    )
    raw_data_summary = _route_canary_transaction_raw_data_summary(
        {"raw_data_hex": _hex(raw_data)},
        transaction_id=transaction_id,
        verifier_payload=verifier_payload,
        expected_call_data=call_data,
    )
    raw_data_owner = _parse_raw_data_tron_payload(
        _parse_exact_hex_blob(
            raw_data_summary["raw_data_owner_address"],
            label="route-canary transaction raw_data_hex owner_address",
        ),
        label="route-canary transaction raw_data_hex owner_address",
    )
    if raw_data_owner != owner:
        raise RuntimeError(
            "route-canary transaction owner_address does not match "
            "raw_data_hex owner_address"
        )
    signature_summary = _route_canary_transaction_signature_summary(
        signature,
        raw_data_hash=transaction_id,
        owner_payload=owner,
    )
    return {
        "transaction_id": parsed_id,
        **raw_data_summary,
        **signature_summary,
        "raw_data_call_matches": raw_data_summary["raw_data_submit_call_matches"],
        "raw_data_owner_matches_transaction": True,
        "contract_ret": "SUCCESS",
        "contract_type": "TriggerSmartContract",
        "type_url": type_url,
        "owner_address": _hex(owner),
        "owner_base58": tron_base58check_from_payload(owner),
        "contract_address": _hex(contract_address),
        "contract_base58": tron_base58check_from_payload(contract_address),
        **call_summary,
        "call_matches": True,
    }


def _route_canary_transaction_summary(
    response: dict[str, Any],
    *,
    transaction_id: bytes,
    route_allowlist_hash: bytes,
    destination_verifier: dict[str, Any],
) -> dict[str, Any]:
    parsed_id = _parse_transaction_info_id(
        response,
        expected_transaction_id=transaction_id,
        label="route-canary transaction info",
    )
    receipt = response.get("receipt")
    receipt_status = receipt.get("result") if isinstance(receipt, dict) else None
    if receipt_status != "SUCCESS":
        raise RuntimeError("route-canary transaction receipt status must be SUCCESS")
    verifier_payload = parse_tron_address_payload(
        str(destination_verifier["address"]),
        label="destination verifier address",
    )
    verifier_address20 = verifier_payload[1:]
    expected_source_domain = destination_verifier.get("destination_source_domain")
    if type(expected_source_domain) is not int:
        raise RuntimeError("destination verifier source domain must be an integer")
    expected_destination_binding_hash = _parse_hex32(
        str(destination_verifier["destination_binding_hash"]),
        label="destination binding hash",
    )
    expected_verifier_backend_hash = _parse_hex32(
        str(destination_verifier["verifier_backend_hash"]),
        label="verifier backend hash",
    )
    expected_proof_family_hash = _parse_hex32(
        str(destination_verifier["proof_family_hash"]),
        label="proof family hash",
    )
    expected_network_id = _parse_hex32(
        str(destination_verifier["network_id"]),
        label="destination verifier network id",
    )
    logs = response.get("log")
    if not isinstance(logs, list):
        raise RuntimeError("route-canary transaction info returned no log list")
    matching_summary: dict[str, Any] | None = None
    for index, log in enumerate(logs):
        if not isinstance(log, dict):
            continue
        summary = _route_canary_message_proof_event_summary(
            log,
            log_index=index,
            transaction_id=transaction_id,
            route_allowlist_hash=route_allowlist_hash,
            verifier_address20=verifier_address20,
            expected_source_domain=expected_source_domain,
            expected_destination_binding_hash=expected_destination_binding_hash,
            expected_verifier_backend_hash=expected_verifier_backend_hash,
            expected_proof_family_hash=expected_proof_family_hash,
            expected_network_id=expected_network_id,
        )
        if summary is None:
            continue
        summary["transaction_id"] = parsed_id
        summary["receipt_status"] = receipt_status
        if matching_summary is not None:
            raise RuntimeError(
                "route-canary transaction log must contain exactly one "
                "matching MessageProofAccepted event"
            )
        matching_summary = summary
    if matching_summary is None:
        raise RuntimeError(
            "route-canary transaction log did not contain the expected "
            "MessageProofAccepted event"
        )
    matching_summary["block_number"] = _required_transaction_info_block_number(
        response,
        label="route-canary transaction info",
    )
    matching_summary["block_timestamp"] = _required_transaction_info_block_timestamp(
        response,
        label="route-canary transaction info",
    )
    return matching_summary


def _route_canary_used_message_proof_summary(
    base_url: str,
    *,
    constant_endpoint: str,
    destination_verifier: dict[str, Any],
    message_id: bytes,
    caller_address: str | None,
    tron_pro_api_key: str | None,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    verifier_address = str(destination_verifier["address"])
    verifier_payload = parse_tron_address_payload(
        verifier_address,
        label="destination verifier address",
    )
    verifier_base58 = tron_base58check_from_payload(verifier_payload)
    owner_for_call = caller_address or verifier_base58
    message_proof_used = _word_bool(
        _constant_word(
            base_url,
            endpoint=constant_endpoint,
            contract_address=verifier_base58,
            function_selector="usedMessageProofs(bytes32)",
            parameter=message_id.hex(),
            owner_address=owner_for_call,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=timeout,
        ),
        label="usedMessageProofs(bytes32)",
    )
    if not message_proof_used:
        raise RuntimeError(
            "route-canary verifier usedMessageProofs(bytes32) is false for "
            "the accepted messageId"
        )
    return {
        "used_message_proofs_checked": True,
        "message_proof_used": True,
        "used_message_proofs_function": "usedMessageProofs(bytes32)",
        "used_message_proofs_parameter": _hex(message_id),
    }


def _contract_metadata(
    base_url: str,
    *,
    address: str,
    tron_pro_api_key: str | None,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    return _post_json(
        base_url,
        "wallet/getcontract",
        {"value": address, "visible": True},
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )


def _metadata_runtime_bytecode(metadata: dict[str, Any], *, label: str) -> bytes | None:
    bytecode = metadata.get("bytecode")
    if not isinstance(bytecode, str) or not bytecode.strip():
        return None
    try:
        return _parse_exact_hex_blob(
            bytecode,
            label=f"{label} bytecode",
        )
    except (argparse.ArgumentTypeError, TypeError, RuntimeError, ValueError):
        raise RuntimeError(
            f"/wallet/getcontract returned malformed {label} bytecode"
        ) from None


def _check_contract_metadata_address(
    metadata: dict[str, Any],
    *,
    expected_payload: bytes,
    label: str,
) -> None:
    value = metadata.get("contract_address")
    if not isinstance(value, str) or not value.strip():
        raise RuntimeError(f"/wallet/getcontract did not return {label} contract_address")
    try:
        observed_payload = parse_tron_address_payload(
            value,
            label=f"{label} contract_address",
        )
    except (argparse.ArgumentTypeError, TypeError, ValueError):
        raise RuntimeError(
            f"/wallet/getcontract returned malformed {label} contract_address"
        ) from None
    if observed_payload != expected_payload:
        raise RuntimeError(
            f"/wallet/getcontract {label} contract_address does not match the queried address"
        )


def collect_source_bridge_evidence(
    base_url: str,
    *,
    source_bridge_address: str,
    caller_address: str | None,
    tron_pro_api_key: str | None,
    constant_endpoint: str,
    include_contract_metadata: bool,
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    """Collect and verify read-only source bridge evidence."""

    bridge_payload = parse_tron_address_payload(
        source_bridge_address,
        label="source bridge address",
    )
    bridge_base58 = tron_base58check_from_payload(bridge_payload)
    owner_for_call = caller_address or bridge_base58
    network_id = _constant_word(
        base_url,
        endpoint=constant_endpoint,
        contract_address=bridge_base58,
        function_selector="networkId()",
        owner_address=owner_for_call,
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    source_domain = _word_u32(
        _constant_word(
            base_url,
            endpoint=constant_endpoint,
            contract_address=bridge_base58,
            function_selector="sourceDomain()",
            owner_address=owner_for_call,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=timeout,
        ),
        label="sourceDomain()",
    )
    target_domain = _word_u32(
        _constant_word(
            base_url,
            endpoint=constant_endpoint,
            contract_address=bridge_base58,
            function_selector="targetDomain()",
            owner_address=owner_for_call,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=timeout,
        ),
        label="targetDomain()",
    )
    owner_address = _word_address20(
        _constant_word(
            base_url,
            endpoint=constant_endpoint,
            contract_address=bridge_base58,
            function_selector="owner()",
            owner_address=owner_for_call,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=timeout,
        ),
        label="owner()",
    )
    observed_config_hash = _constant_word(
        base_url,
        endpoint=constant_endpoint,
        contract_address=bridge_base58,
        function_selector="sourceBridgeConfigHash()",
        owner_address=owner_for_call,
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    recomputed_config_hash = evidence.tron_source_bridge_config_hash(
        bridge_address=bridge_payload[1:],
        network_id=network_id,
        source_domain=source_domain,
        target_domain=target_domain,
        owner_address=owner_address,
    )
    if observed_config_hash != recomputed_config_hash:
        raise RuntimeError(
            "sourceBridgeConfigHash() does not match the canonical TRON -> SORA "
            "source bridge config hash"
        )

    metadata: dict[str, Any] | None = None
    bytecode_hash = None
    metadata_code_hash = None
    if include_contract_metadata:
        metadata = _contract_metadata(
            base_url,
            address=bridge_base58,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=timeout,
        )
        _check_contract_metadata_address(
            metadata,
            expected_payload=bridge_payload,
            label="source bridge",
        )
        runtime_bytecode = _metadata_runtime_bytecode(metadata, label="source bridge")
        if runtime_bytecode is None:
            raise RuntimeError(
                "/wallet/getcontract did not return source bridge bytecode; "
                "pass --no-getcontract only after independently pinning the "
                "deployed source bridge runtime code hash"
            )
        bytecode_hash = _hex(evidence.runtime_bytecode_hash(runtime_bytecode))
        output_bytecode_hex = _hex(runtime_bytecode)
        raw_code_hash = metadata.get("code_hash")
        if isinstance(raw_code_hash, str) and raw_code_hash.strip():
            metadata_code_hash = raw_code_hash.strip()

    output = {
        "address": bridge_base58,
        "address_hex": _hex(bridge_payload),
        "source_bridge_emitter_address": _hex(bridge_payload[1:]),
        "source_bridge_network_id": _hex(network_id),
        "source_domain": source_domain,
        "target_domain": target_domain,
        "source_bridge_owner_address": _hex(owner_address),
        "source_bridge_owner_base58": tron_base58check_from_address20(owner_address),
        "source_bridge_config_hash": _hex(observed_config_hash),
        "recomputed_source_bridge_config_hash": _hex(recomputed_config_hash),
        "config_hash_matches": True,
    }
    if include_contract_metadata:
        output["tron_getcontract_metadata_checked"] = True
        output["tron_getcontract_bytecode_hash_available"] = bytecode_hash is not None
    if bytecode_hash is not None:
        output["source_bridge_emitter_code_hash"] = bytecode_hash
        output["source_bridge_runtime_bytecode_hex"] = output_bytecode_hex
    if metadata_code_hash is not None:
        output["tron_getcontract_code_hash"] = metadata_code_hash
    return output


def collect_destination_verifier_evidence(
    base_url: str,
    *,
    destination_verifier_address: str,
    caller_address: str | None,
    tron_pro_api_key: str | None,
    constant_endpoint: str,
    include_contract_metadata: bool,
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    """Collect and verify read-only TRON destination verifier evidence."""

    verifier_payload = parse_tron_address_payload(
        destination_verifier_address,
        label="destination verifier address",
    )
    verifier_base58 = tron_base58check_from_payload(verifier_payload)
    owner_for_call = caller_address or verifier_base58
    network_id = _constant_word(
        base_url,
        endpoint=constant_endpoint,
        contract_address=verifier_base58,
        function_selector="networkId()",
        owner_address=owner_for_call,
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    source_domain = _word_u32(
        _constant_word(
            base_url,
            endpoint=constant_endpoint,
            contract_address=verifier_base58,
            function_selector="expectedSourceDomain()",
            owner_address=owner_for_call,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=timeout,
        ),
        label="expectedSourceDomain()",
    )
    target_domain = _word_u32(
        _constant_word(
            base_url,
            endpoint=constant_endpoint,
            contract_address=verifier_base58,
            function_selector="expectedTargetDomain()",
            owner_address=owner_for_call,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=timeout,
        ),
        label="expectedTargetDomain()",
    )
    verifier_code_hash = _constant_word(
        base_url,
        endpoint=constant_endpoint,
        contract_address=verifier_base58,
        function_selector="verifierCodeHash()",
        owner_address=owner_for_call,
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    verifier_key_hash = _constant_word(
        base_url,
        endpoint=constant_endpoint,
        contract_address=verifier_base58,
        function_selector="verifierKeyHash()",
        owner_address=owner_for_call,
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    verifier_backend_hash = _constant_word(
        base_url,
        endpoint=constant_endpoint,
        contract_address=verifier_base58,
        function_selector="verifierBackendHash()",
        owner_address=owner_for_call,
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    proof_family_hash = _constant_word(
        base_url,
        endpoint=constant_endpoint,
        contract_address=verifier_base58,
        function_selector="proofFamilyHash()",
        owner_address=owner_for_call,
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    observed_binding_hash = _constant_word(
        base_url,
        endpoint=constant_endpoint,
        contract_address=verifier_base58,
        function_selector="destinationBindingHash()",
        owner_address=owner_for_call,
        tron_pro_api_key=tron_pro_api_key,
        opener=opener,
        timeout=timeout,
    )
    expected_backend_hash = evidence._keccak_256(
        evidence.TRON_GROTH16_BACKEND.encode("utf-8")
    )
    if verifier_backend_hash != expected_backend_hash:
        raise RuntimeError("verifierBackendHash() is not tron-groth16-bn254-v1")
    expected_proof_family_hash = evidence._keccak_256(
        evidence.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")
    )
    if proof_family_hash != expected_proof_family_hash:
        raise RuntimeError("proofFamilyHash() is not stark-fri-v1")
    recomputed_binding_hash = evidence.tron_destination_binding_hash(
        network_id=network_id,
        source_domain=source_domain,
        target_domain=target_domain,
        verifier_address=verifier_base58,
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )
    if observed_binding_hash != recomputed_binding_hash:
        raise RuntimeError(
            "destinationBindingHash() does not match the canonical SORA -> TRON "
            "destination binding hash"
        )
    binding_key = evidence.tron_destination_binding_key(
        network_id=network_id,
        source_domain=source_domain,
        target_domain=target_domain,
        verifier_address=verifier_base58,
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=verifier_key_hash,
    )

    metadata_code_hash = None
    bytecode_hash = None
    if include_contract_metadata:
        metadata = _contract_metadata(
            base_url,
            address=verifier_base58,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=timeout,
        )
        _check_contract_metadata_address(
            metadata,
            expected_payload=verifier_payload,
            label="destination verifier",
        )
        runtime_bytecode = _metadata_runtime_bytecode(metadata, label="destination verifier")
        if runtime_bytecode is None:
            raise RuntimeError(
                "/wallet/getcontract did not return destination verifier bytecode; "
                "pass --no-getcontract only after independently pinning the "
                "deployed verifier runtime code hash"
            )
        bytecode_hash = _hex(evidence.runtime_bytecode_hash(runtime_bytecode))
        output_bytecode_hex = _hex(runtime_bytecode)
        if bytecode_hash != _hex(verifier_code_hash):
            raise RuntimeError(
                "/wallet/getcontract runtime bytecode hash does not match "
                "verifierCodeHash(): "
                f"expected {_hex(verifier_code_hash)}, got {bytecode_hash}"
            )
        raw_code_hash = metadata.get("code_hash")
        if isinstance(raw_code_hash, str) and raw_code_hash.strip():
            metadata_code_hash = raw_code_hash.strip()

    output = {
        "address": verifier_base58,
        "address_hex": _hex(verifier_payload),
        "network_id": _hex(network_id),
        "destination_source_domain": source_domain,
        "destination_target_domain": target_domain,
        "destination_verifier_code_hash": _hex(verifier_code_hash),
        "destination_verifier_key_hash": _hex(verifier_key_hash),
        "verifier_backend_hash": _hex(verifier_backend_hash),
        "proof_family_hash": _hex(proof_family_hash),
        "verifier_backend_hash_matches": True,
        "proof_family_hash_matches": True,
        "destination_binding_hash": _hex(observed_binding_hash),
        "recomputed_destination_binding_hash": _hex(recomputed_binding_hash),
        "destination_binding_key": binding_key,
        "destination_binding_hash_matches": True,
    }
    if bytecode_hash is not None:
        output["tron_getcontract_bytecode_hash"] = bytecode_hash
        output["destination_verifier_runtime_bytecode_hex"] = output_bytecode_hex
        output["bytecode_hash_matches_verifier_code_hash"] = True
    if metadata_code_hash is not None:
        output["tron_getcontract_code_hash"] = metadata_code_hash
    return output


_SOURCE_RECORD_HASH_FIELDS = (
    "source_trust_anchor_hash",
    "consensus_verifier_hash",
    "message_inclusion_verifier_hash",
    "finality_policy_hash",
    "deployment_receipt_hash",
)


def _source_record_preflight_requested(args: argparse.Namespace) -> bool:
    return any(
        getattr(args, name, None) is not None
        for name in (
            *_SOURCE_RECORD_HASH_FIELDS,
            "source_bridge_emitter_code_hash",
            "expected_source_bridge_config_hash",
            "adapter_verifier_vk_hash",
            "expected_source_verifier_material_hash",
            "expected_source_adapter_engine_deployment_hash",
            "expected_tron_dpos_source_gate_hash",
            "source_event_digest",
        )
    )


def _source_record_material_preflight_requested(args: argparse.Namespace) -> bool:
    return any(
        getattr(args, name, None) is not None
        for name in (
            *_SOURCE_RECORD_HASH_FIELDS,
            "source_bridge_emitter_code_hash",
            "adapter_verifier_vk_hash",
            "expected_source_verifier_material_hash",
            "expected_source_adapter_engine_deployment_hash",
            "expected_tron_dpos_source_gate_hash",
        )
    )


def _build_source_record_args(
    source: dict[str, Any],
    args: argparse.Namespace,
) -> argparse.Namespace | None:
    supplied_fields = {
        name: _optional_hex32_arg(args, name) for name in _SOURCE_RECORD_HASH_FIELDS
    }
    supplied_fields["source_bridge_emitter_code_hash"] = _optional_hex32_arg(
        args,
        "source_bridge_emitter_code_hash",
    )
    expected_material_hash = _optional_hex32_arg(
        args,
        "expected_source_verifier_material_hash",
    )
    expected_deployment_hash = _optional_hex32_arg(
        args,
        "expected_source_adapter_engine_deployment_hash",
    )
    expected_gate_hash = _optional_hex32_arg(
        args,
        "expected_tron_dpos_source_gate_hash",
    )
    adapter_verifier_vk_hash = _optional_hex32_arg(args, "adapter_verifier_vk_hash")
    source_record_requested = _source_record_material_preflight_requested(args)

    observed_code_hash = source.get("source_bridge_emitter_code_hash")
    if isinstance(observed_code_hash, str):
        observed = _parse_hex32(
            observed_code_hash,
            label="source bridge getcontract bytecode hash",
        )
        supplied_code_hash = supplied_fields["source_bridge_emitter_code_hash"]
        if supplied_code_hash is not None and supplied_code_hash != observed:
            raise ValueError(
                "--source-bridge-emitter-code-hash does not match "
                "/wallet/getcontract runtime bytecode: "
                f"expected {_hex(supplied_code_hash)}, got {_hex(observed)}"
            )
        supplied_fields["source_bridge_emitter_code_hash"] = observed
    elif supplied_fields["source_bridge_emitter_code_hash"] is not None:
        source["source_bridge_emitter_code_hash"] = _hex(
            supplied_fields["source_bridge_emitter_code_hash"]
        )

    if not source_record_requested:
        return None
    if (
        source.get("tron_getcontract_metadata_checked") is True
        and observed_code_hash is None
    ):
        raise ValueError(
            "TRON live source record hash preflight requires "
            "/wallet/getcontract bytecode for the source bridge when metadata "
            "lookup is enabled; pass --no-getcontract only after independently "
            "auditing --source-bridge-emitter-code-hash"
        )

    missing = [name for name, value in supplied_fields.items() if value is None]
    if missing:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing)
        raise ValueError(
            "TRON live source record hash preflight requires " + formatted
        )

    record_args = SimpleNamespace(
        source_domain=source["source_domain"],
        target_domain=source["target_domain"],
        bridge_address=evidence.parse_tron_address(
            str(source["address"]),
            label="source bridge address",
        ),
        owner_address=evidence.parse_tron_address(
            str(source["source_bridge_owner_base58"]),
            label="source bridge owner address",
        ),
        network_id=_parse_hex32(
            str(source["source_bridge_network_id"]),
            label="source bridge network id",
        ),
        source_trust_anchor_hash=supplied_fields["source_trust_anchor_hash"],
        consensus_verifier_hash=supplied_fields["consensus_verifier_hash"],
        message_inclusion_verifier_hash=supplied_fields[
            "message_inclusion_verifier_hash"
        ],
        source_bridge_emitter_code_hash=supplied_fields[
            "source_bridge_emitter_code_hash"
        ],
        finality_policy_hash=supplied_fields["finality_policy_hash"],
        adapter_verifier_vk_hash=adapter_verifier_vk_hash,
        deployment_receipt_hash=supplied_fields["deployment_receipt_hash"],
        expected_source_verifier_material_hash=expected_material_hash,
        expected_source_adapter_engine_deployment_hash=expected_deployment_hash,
        expected_tron_dpos_source_gate_hash=expected_gate_hash,
    )
    evidence.apply_source_adapter_verifier_vk_hash(record_args)
    return record_args


def _collect_source_record_hashes(
    source: dict[str, Any],
    args: argparse.Namespace,
) -> dict[str, Any] | None:
    record_args = _build_source_record_args(source, args)
    if record_args is None:
        return None
    config_hash = _parse_hex32(str(source["source_bridge_config_hash"]), label="config hash")
    material_hash = evidence.tron_source_verifier_material_record_hash(
        record_args,
        config_hash,
    )
    deployment_hash = evidence.tron_source_adapter_engine_deployment_record_hash(
        record_args,
        config_hash,
    )
    gate_hash = evidence.tron_dpos_source_gate_hash(record_args, config_hash)
    if (
        record_args.expected_source_verifier_material_hash is not None
        and material_hash != record_args.expected_source_verifier_material_hash
    ):
        raise ValueError(
            "--expected-source-verifier-material-hash does not match live "
            "source record inputs: "
            f"expected {_hex(record_args.expected_source_verifier_material_hash)}, "
            f"got {_hex(material_hash)}"
        )
    if (
        record_args.expected_source_adapter_engine_deployment_hash is not None
        and deployment_hash != record_args.expected_source_adapter_engine_deployment_hash
    ):
        raise ValueError(
            "--expected-source-adapter-engine-deployment-hash does not match "
            "live source record inputs: "
            f"expected {_hex(record_args.expected_source_adapter_engine_deployment_hash)}, "
            f"got {_hex(deployment_hash)}"
        )
    if (
        record_args.expected_tron_dpos_source_gate_hash is not None
        and gate_hash != record_args.expected_tron_dpos_source_gate_hash
    ):
        raise ValueError(
            "--expected-tron-dpos-source-gate-hash does not match live "
            "source record inputs: "
            f"expected {_hex(record_args.expected_tron_dpos_source_gate_hash)}, "
            f"got {_hex(gate_hash)}"
        )
    output = {
        "adapter_verifier_vk_hash": _hex(record_args.adapter_verifier_vk_hash),
        "source_verifier_material_hash": _hex(material_hash),
        "source_adapter_engine_deployment_hash": _hex(deployment_hash),
        "tron_dpos_source_gate_hash": _hex(gate_hash),
    }
    if record_args.expected_source_verifier_material_hash is not None:
        output["expected_source_verifier_material_hash_matches"] = True
    if record_args.expected_source_adapter_engine_deployment_hash is not None:
        output["expected_source_adapter_engine_deployment_hash_matches"] = True
    if record_args.expected_tron_dpos_source_gate_hash is not None:
        output["expected_tron_dpos_source_gate_hash_matches"] = True
    return output


def _check_expected_source_config_hash(
    source: dict[str, Any],
    args: argparse.Namespace,
) -> None:
    expected_config_hash = _optional_hex32_arg(
        args,
        "expected_source_bridge_config_hash",
    )
    if expected_config_hash is None:
        return
    observed_config_hash = _parse_hex32(
        str(source["source_bridge_config_hash"]),
        label="source bridge config hash",
    )
    if expected_config_hash != observed_config_hash:
        raise ValueError(
            "--expected-source-bridge-config-hash does not match live "
            "sourceBridgeConfigHash(): "
            f"expected {_hex(expected_config_hash)}, got {_hex(observed_config_hash)}"
        )
    source["expected_source_bridge_config_hash_matches"] = True


def _check_source_destination_network_id_match(summary: dict[str, Any]) -> None:
    source = summary.get("source_bridge")
    destination = summary.get("destination_verifier")
    if not isinstance(source, dict) or not isinstance(destination, dict):
        return
    source_network_id = _parse_hex32(
        str(source.get("source_bridge_network_id")),
        label="source bridge network id",
    )
    destination_network_id = _parse_hex32(
        str(destination.get("network_id")),
        label="destination verifier network id",
    )
    if source_network_id != destination_network_id:
        raise ValueError(
            "destination verifier networkId() does not match source bridge "
            "networkId(): "
            f"source {_hex(source_network_id)}, destination {_hex(destination_network_id)}"
        )
    destination["source_bridge_network_id_matches"] = True


def _check_expected_destination_binding_hash(
    destination: dict[str, Any],
    args: argparse.Namespace,
) -> None:
    expected_binding_hash = _optional_hex32_arg(
        args,
        "expected_destination_binding_hash",
    )
    if expected_binding_hash is None:
        return
    observed_binding_hash = _parse_hex32(
        str(destination["destination_binding_hash"]),
        label="destination binding hash",
    )
    if expected_binding_hash != observed_binding_hash:
        raise ValueError(
            "--expected-destination-binding-hash does not match live "
            "destinationBindingHash(): "
            f"expected {_hex(expected_binding_hash)}, got {_hex(observed_binding_hash)}"
        )
    destination["expected_destination_binding_hash_matches"] = True


def _validate_route_allowlist_hash(
    *,
    supplied_hash: bytes,
    route_canary_evidence_hash: bytes | None,
    source_records: Any,
    destination_verifier: Any,
    destination_binding_pinned: bool,
) -> dict[str, Any]:
    if not isinstance(destination_verifier, dict):
        raise ValueError("--route-allowlist-hash requires --destination-verifier-address")
    if not isinstance(source_records, dict):
        raise ValueError(
            "--route-allowlist-hash requires complete source record preflight "
            "arguments"
        )
    if not destination_binding_pinned:
        raise ValueError(
            "--route-allowlist-hash requires --expected-destination-binding-hash"
        )
    destination_binding_hash = _parse_hex32(
        str(destination_verifier.get("destination_binding_hash")),
        label="destination binding hash",
    )
    source_verifier_material_hash = _parse_hex32(
        str(source_records.get("source_verifier_material_hash")),
        label="source verifier material hash",
    )
    source_adapter_engine_deployment_hash = _parse_hex32(
        str(source_records.get("source_adapter_engine_deployment_hash")),
        label="source adapter engine deployment hash",
    )
    expected_hash = evidence.tron_route_allowlist_hash(
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if supplied_hash != expected_hash:
        raise ValueError(
            "--route-allowlist-hash does not match canonical source, "
            "deployment, and destination evidence: "
            f"expected {_hex(expected_hash)}, got {_hex(supplied_hash)}"
        )
    summary = {
        "route_allowlist_hash": _hex(supplied_hash),
        "expected_route_allowlist_hash": _hex(expected_hash),
        "expected_route_allowlist_hash_matches": True,
    }
    if route_canary_evidence_hash is not None:
        summary["route_canary"] = evidence._route_canary_summary(
            argparse.Namespace(route_canary_evidence_hash=route_canary_evidence_hash),
            route_allowlist_hash=supplied_hash,
            destination_binding_hash=destination_binding_hash,
            source_verifier_material_hash=source_verifier_material_hash,
            source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
        )
    return summary


def _offline_args(summary: dict[str, Any]) -> list[str]:
    args: list[str] = []
    source = summary.get("source_bridge")
    if isinstance(source, dict):
        args.extend(
            [
                "--bridge-address",
                str(source["address"]),
                "--owner-address",
                str(source["source_bridge_owner_base58"]),
                "--network-id",
                str(source["source_bridge_network_id"]),
            ]
        )
        if source.get("expected_source_bridge_config_hash_matches") is True:
            args.extend(
                [
                    "--expected-config-hash",
                    str(source["source_bridge_config_hash"]),
                ]
            )
        code_hash = source.get("source_bridge_emitter_code_hash")
        if isinstance(code_hash, str):
            args.extend(["--source-bridge-emitter-code-hash", code_hash])
        runtime_bytecode = source.get("source_bridge_runtime_bytecode_hex")
        if isinstance(runtime_bytecode, str):
            args.extend(["--source-bridge-runtime-bytecode-hex", runtime_bytecode])
    source_record_inputs = summary.get("source_record_inputs")
    if isinstance(source_record_inputs, dict):
        for key in (
            "source_trust_anchor_hash",
            "consensus_verifier_hash",
            "message_inclusion_verifier_hash",
            "finality_policy_hash",
            "deployment_receipt_hash",
            "adapter_verifier_vk_hash",
            "expected_source_verifier_material_hash",
            "expected_source_adapter_engine_deployment_hash",
            "expected_tron_dpos_source_gate_hash",
        ):
            value = source_record_inputs.get(key)
            if isinstance(value, str):
                args.extend([f"--{key.replace('_', '-')}", value])
    destination = summary.get("destination_verifier")
    if isinstance(destination, dict):
        args.extend(
            [
                "--destination-verifier-address",
                str(destination["address"]),
                "--destination-verifier-code-hash",
                str(destination["destination_verifier_code_hash"]),
                "--destination-verifier-key-hash",
                str(destination["destination_verifier_key_hash"]),
            ]
        )
        runtime_bytecode = destination.get("destination_verifier_runtime_bytecode_hex")
        if isinstance(runtime_bytecode, str):
            args.extend(["--destination-verifier-runtime-bytecode-hex", runtime_bytecode])
        if destination.get("expected_destination_binding_hash_matches") is True:
            args.extend(
                [
                    "--expected-destination-binding-hash",
                    str(destination["destination_binding_hash"]),
                ]
            )
    route_allowlist_hash = summary.get("route_allowlist_hash")
    if (
        isinstance(route_allowlist_hash, str)
        and isinstance(destination, dict)
        and destination.get("expected_destination_binding_hash_matches") is True
    ):
        args.extend(["--route-allowlist-hash", route_allowlist_hash])
        route_canary = summary.get("route_canary")
        if isinstance(route_canary, dict):
            args.extend(
                [
                    "--route-canary-evidence-hash",
                    str(route_canary["evidence_hash"]),
                ]
            )
        route_canary_transaction = summary.get("route_canary_transaction")
        if isinstance(route_canary_transaction, dict):
            trigger_contract = route_canary_transaction.get("trigger_contract")
            args.extend(
                [
                    "--route-canary-transaction-id",
                    str(route_canary_transaction["transaction_id"]),
                    "--route-canary-block-number",
                    str(route_canary_transaction["block_number"]),
                    "--route-canary-block-timestamp",
                    str(route_canary_transaction["block_timestamp"]),
                    "--route-canary-log-index",
                    str(route_canary_transaction["log_index"]),
                    "--route-canary-message-id",
                    str(route_canary_transaction["message_id"]),
                    "--route-canary-call-data-sha256",
                    str(trigger_contract["call_data_sha256"]),
                    "--route-canary-payload-hash",
                    str(trigger_contract["public_inputs_payload_hash"]),
                    "--route-canary-target-domain",
                    str(trigger_contract["public_inputs_target_domain"]),
                    "--route-canary-statement-hash",
                    str(route_canary_transaction["statement_hash"]),
                    "--route-canary-commitment-root",
                    str(route_canary_transaction["commitment_root"]),
                    "--route-canary-finality-height",
                    str(trigger_contract["public_inputs_finality_height"]),
                    "--route-canary-finality-block-hash",
                    str(trigger_contract["public_inputs_finality_block_hash"]),
                    "--route-canary-proof-version",
                    str(trigger_contract["proof_version"]),
                    "--route-canary-proof-source-domain",
                    str(trigger_contract["proof_source_domain"]),
                    "--route-canary-used-message-proof",
                    "--route-canary-raw-data-owner-matches-transaction",
                ]
            )
            if isinstance(trigger_contract, dict):
                signature_sha256 = trigger_contract.get("signature_sha256")
                signature_recovered_address = trigger_contract.get(
                    "signature_recovered_address"
                )
                owner_address = trigger_contract.get("owner_address")
                if isinstance(signature_sha256, str) and isinstance(
                    signature_recovered_address,
                    str,
                ) and isinstance(owner_address, str):
                    args.extend(
                        [
                            "--route-canary-transaction-owner-address",
                            owner_address,
                            "--route-canary-signature-sha256",
                            signature_sha256,
                            "--route-canary-signature-recovered-address",
                            signature_recovered_address,
                        ]
                    )
                if trigger_contract.get("signature_recovers_to_owner") is True:
                    args.append("--route-canary-signature-recovers-to-owner")
    return args


def _offline_source_event_args(summary: dict[str, Any]) -> list[str] | None:
    if not _source_event_call_verified(summary):
        return None
    source_event_call = summary.get("source_event_call")
    if not isinstance(source_event_call, dict):
        return None
    source_event_digest = str(source_event_call["source_event_digest"])
    return [
        *_offline_args(summary),
        "--source-event-digest",
        source_event_digest,
    ]


def _source_event_trigger_request_verified(
    trigger_request: dict[str, Any],
    *,
    source_bridge_payload: bytes,
    owner_payload: bytes,
    source_event_call_data: bytes,
) -> bool:
    expected_keys = {
        "endpoint",
        "owner_address",
        "contract_address",
        "function_selector",
        "parameter",
        "visible",
        "call_value",
    }
    if set(trigger_request) != expected_keys:
        return False
    try:
        owner_address = parse_tron_address_payload(
            str(trigger_request["owner_address"]),
            label="source-event trigger owner address",
        )
        contract_address = parse_tron_address_payload(
            str(trigger_request["contract_address"]),
            label="source-event trigger contract address",
        )
    except (argparse.ArgumentTypeError, TypeError, ValueError):
        return False
    return (
        trigger_request.get("endpoint") == "wallet/triggersmartcontract"
        and owner_address == owner_payload
        and contract_address == source_bridge_payload
        and trigger_request.get("function_selector")
        == evidence.TRON_SOURCE_MESSAGE_CALL_ABI.decode("ascii")
        and trigger_request.get("parameter") == source_event_call_data[4:].hex()
        and trigger_request.get("visible") is True
        and trigger_request.get("call_value") == 0
    )


def _source_event_transaction_verified(
    summary: dict[str, Any],
    *,
    source_bridge_payload: bytes,
    owner_payload: bytes,
    source_event_digest: bytes,
    source_event_call_data: bytes,
) -> bool:
    transaction = summary.get("source_event_transaction")
    if not isinstance(transaction, dict):
        return False
    trigger_contract = transaction.get("trigger_contract")
    if not isinstance(trigger_contract, dict):
        return False
    try:
        transaction_id = _parse_hex32(
            str(transaction["transaction_id"]),
            label="source-event transaction id",
        )
        block_number, block_timestamp = _summary_block_metadata(
            transaction,
            label="source-event transaction",
        )
        solid_block = transaction.get("solid_block")
        if not isinstance(solid_block, dict):
            return False
        solid_block_number, solid_block_timestamp = _summary_block_metadata(
            solid_block,
            label="source-event solid block",
        )
        if (
            solid_block_number != block_number
            or solid_block_timestamp != block_timestamp
        ):
            return False
        raw_data = _parse_exact_hex_blob(
            trigger_contract.get("raw_data_hex"),
            label="source-event transaction raw_data_hex",
        )
        raw_summary = _source_event_transaction_raw_data_summary(
            {"raw_data_hex": _hex(raw_data)},
            transaction_id=transaction_id,
            source_bridge_payload=source_bridge_payload,
            owner_payload=owner_payload,
            source_event_call_data=source_event_call_data,
        )
        signature = _parse_exact_hex_blob(
            trigger_contract.get("signature"),
            label="source-event transaction signature",
        )
        if not _tron_recoverable_signature_is_canonical(signature):
            return False
        signature_summary = _source_event_transaction_signature_summary(
            signature,
            raw_data_hash=transaction_id,
            owner_payload=owner_payload,
        )
        source_proof_result_bytes = _parse_exact_hex_blob(
            trigger_contract.get("source_proof_result_bytes"),
            label="source-event source proof result bytes",
            nonzero=False,
        )
        if not _source_event_result_bytes_are_success(source_proof_result_bytes):
            return False
        source_proof_transaction_bytes = b"".join(
            [
                _protobuf_bytes_field(1, raw_data),
                _protobuf_bytes_field(2, signature),
                _protobuf_bytes_field(5, source_proof_result_bytes),
            ]
        )
        source_proof_transaction_hash = hashlib.sha256(
            source_proof_transaction_bytes
        ).digest()
    except (
        argparse.ArgumentTypeError,
        KeyError,
        RuntimeError,
        TypeError,
        ValueError,
    ):
        return False

    for key, expected in {**raw_summary, **signature_summary}.items():
        if trigger_contract.get(key) != expected:
            return False

    return (
        transaction.get("receipt_status") == "SUCCESS"
        and transaction.get("event_matches") is True
        and type(transaction.get("log_index")) is int
        and transaction["log_index"] >= 0
        and transaction.get("event_address") == _hex(source_bridge_payload[1:])
        and transaction.get("event_topic0") == _hex(TRON_SOURCE_EVENT_TOPIC)
        and transaction.get("source_event_digest") == _hex(source_event_digest)
        and transaction.get("event_data") == "0x"
        and trigger_contract.get("transaction_id") == _hex(transaction_id)
        and trigger_contract.get("contract_ret") == "SUCCESS"
        and trigger_contract.get("contract_type") == "TriggerSmartContract"
        and trigger_contract.get("type_url")
        == TRON_TRIGGER_SMART_CONTRACT_TYPE_URL.decode("ascii")
        and trigger_contract.get("owner_address") == _hex(owner_payload)
        and trigger_contract.get("contract_address") == _hex(source_bridge_payload)
        and trigger_contract.get("call_data") == _hex(source_event_call_data)
        and trigger_contract.get("source_proof_transaction_bytes")
        == _hex(source_proof_transaction_bytes)
        and trigger_contract.get("source_proof_transaction_hash")
        == _hex(source_proof_transaction_hash)
        and trigger_contract.get("source_proof_transaction_bytes_checked") is True
        and trigger_contract.get("transaction_merkle_branch_required") is True
    )


def _source_event_call_verified(summary: dict[str, Any]) -> bool:
    source = summary.get("source_bridge")
    source_event_call = summary.get("source_event_call")
    if not isinstance(source, dict) or not isinstance(source_event_call, dict):
        return False
    try:
        source_bridge_payload = parse_tron_address_payload(
            str(source["address"]),
            label="source bridge address",
        )
        source_event_bridge_payload = parse_tron_address_payload(
            str(source_event_call["source_bridge_address"]),
            label="source-event source bridge address",
        )
        owner_payload = parse_tron_address_payload(
            str(source["source_bridge_owner_base58"]),
            label="source bridge owner address",
        )
        owner_payload_from_hex = parse_tron_address_payload(
            str(source["source_bridge_owner_address"]),
            label="source bridge owner address",
        )
        source_event_owner_payload = parse_tron_address_payload(
            str(source_event_call["source_bridge_owner_address"]),
            label="source-event source bridge owner address",
        )
        source_event_owner_base58_payload = parse_tron_address_payload(
            str(source_event_call["source_bridge_owner_base58"]),
            label="source-event source bridge owner base58",
        )
        source_domain = _parse_canonical_u32(
            source["source_domain"],
            label="source bridge source domain",
        )
        target_domain = _parse_canonical_u32(
            source["target_domain"],
            label="source bridge target domain",
        )
        source_event_source_domain = _parse_canonical_u32(
            source_event_call["source_domain"],
            label="source-event source domain",
        )
        source_event_target_domain = _parse_canonical_u32(
            source_event_call["target_domain"],
            label="source-event target domain",
        )
        source_event_digest = _parse_hex32(
            str(source_event_call["source_event_digest"]),
            label="source-event digest",
        )
        source_event_call_data = _parse_exact_hex_blob(
            source_event_call.get("source_event_call_data"),
            label="source-event call data",
        )
        expected_call_data = evidence.tron_source_message_call_data(
            source_domain=source_domain,
            target_domain=target_domain,
            source_event_digest=source_event_digest,
        )
    except (
        argparse.ArgumentTypeError,
        KeyError,
        RuntimeError,
        TypeError,
        ValueError,
    ):
        return False

    if (
        source_event_bridge_payload != source_bridge_payload
        or owner_payload_from_hex != owner_payload
        or source_event_owner_payload != owner_payload
        or source_event_owner_base58_payload != owner_payload
        or source_event_source_domain != source_domain
        or source_event_target_domain != target_domain
        or source_event_call_data != expected_call_data
        or source_event_call.get("submitted_source_events_checked") is not True
    ):
        return False
    source_event_already_submitted = source_event_call.get(
        "source_event_already_submitted"
    )
    transaction_required = source_event_call.get("transaction_required")
    if (
        type(source_event_already_submitted) is not bool
        or type(transaction_required) is not bool
        or transaction_required != (not source_event_already_submitted)
    ):
        return False
    trigger_request = source_event_call.get("trigger_request")
    if transaction_required:
        return isinstance(trigger_request, dict) and _source_event_trigger_request_verified(
            trigger_request,
            source_bridge_payload=source_bridge_payload,
            owner_payload=owner_payload,
            source_event_call_data=source_event_call_data,
        )
    return (
        trigger_request is None
        and _source_event_transaction_verified(
            summary,
            source_bridge_payload=source_bridge_payload,
            owner_payload=owner_payload,
            source_event_digest=source_event_digest,
            source_event_call_data=source_event_call_data,
        )
    )


def _route_canary_transaction_verified(summary: dict[str, Any]) -> bool:
    route_canary = summary.get("route_canary")
    transaction = summary.get("route_canary_transaction")
    if not isinstance(route_canary, dict) or not isinstance(transaction, dict):
        return False
    destination = summary.get("destination_verifier")
    if not isinstance(destination, dict):
        return False
    trigger_contract = transaction.get("trigger_contract")
    if not isinstance(trigger_contract, dict):
        return False
    owner_address = trigger_contract.get("owner_address")
    raw_data_owner_address = trigger_contract.get("raw_data_owner_address")
    signature_sha256 = trigger_contract.get("signature_sha256")
    signature_recovered_address = trigger_contract.get("signature_recovered_address")
    try:
        route_allowlist_hash = _parse_hex32(
            str(summary.get("route_allowlist_hash")),
            label="route canary route allowlist hash",
        )
        transaction_id = _parse_hex32(
            str(transaction.get("transaction_id")),
            label="route canary transaction id",
        )
        _summary_block_metadata(
            transaction,
            label="route canary transaction",
        )
        transaction_source_domain = _parse_canonical_u32(
            transaction["source_domain"],
            label="route canary source domain",
        )
        destination_source_domain = _parse_canonical_u32(
            destination["destination_source_domain"],
            label="destination verifier source domain",
        )
        destination_target_domain = _parse_canonical_u32(
            destination["destination_target_domain"],
            label="destination verifier target domain",
        )
        transaction_destination_binding_hash = _parse_hex32(
            str(transaction.get("destination_binding_hash")),
            label="route canary destination binding hash",
        )
        destination_binding_hash = _parse_hex32(
            str(destination.get("destination_binding_hash")),
            label="destination binding hash",
        )
        transaction_verifier_backend_hash = _parse_hex32(
            str(transaction.get("verifier_backend_hash")),
            label="route canary verifier backend hash",
        )
        destination_verifier_backend_hash = _parse_hex32(
            str(destination.get("verifier_backend_hash")),
            label="destination verifier backend hash",
        )
        transaction_proof_family_hash = _parse_hex32(
            str(transaction.get("proof_family_hash")),
            label="route canary proof family hash",
        )
        destination_proof_family_hash = _parse_hex32(
            str(destination.get("proof_family_hash")),
            label="destination proof family hash",
        )
        transaction_network_id = _parse_hex32(
            str(transaction.get("network_id")),
            label="route canary network id",
        )
        destination_network_id = _parse_hex32(
            str(destination.get("network_id")),
            label="destination verifier network id",
        )
        verifier_payload = parse_tron_address_payload(
            str(destination["address"]),
            label="destination verifier address",
        )
        owner_payload = _parse_tron_payload_hex(
            owner_address,
            label="route canary transaction owner address",
        )
        call_data = _parse_exact_hex_blob(
            trigger_contract.get("call_data"),
            label="route canary transaction call data",
        )
        call_summary = _route_canary_submit_call_data_summary(
            call_data,
            event_summary=transaction,
            expected_source_domain=destination_source_domain,
            expected_target_domain=destination_target_domain,
        )
        raw_data = _parse_exact_hex_blob(
            trigger_contract.get("raw_data_hex"),
            label="route canary transaction raw_data_hex",
        )
        raw_summary = _route_canary_transaction_raw_data_summary(
            {"raw_data_hex": _hex(raw_data)},
            transaction_id=transaction_id,
            verifier_payload=verifier_payload,
            expected_call_data=call_data,
        )
        signature = _parse_exact_hex_blob(
            trigger_contract.get("signature"),
            label="route canary transaction signature",
        )
        if not _tron_recoverable_signature_is_canonical(signature):
            return False
        signature_summary = _route_canary_transaction_signature_summary(
            signature,
            raw_data_hash=transaction_id,
            owner_payload=owner_payload,
        )
        block_number, block_timestamp = _summary_block_metadata(
            transaction,
            label="route canary transaction",
        )
        recomputed_hash = _hex(
            _tron_route_canary_transaction_evidence_hash(
                route_allowlist_hash=route_allowlist_hash,
                transaction_id=transaction_id,
                transaction_owner_address=owner_payload,
                block_number=block_number,
                block_timestamp=block_timestamp,
                log_index=transaction["log_index"],
                verifier_address20=verifier_payload[1:],
                call_data_sha256=_parse_hex32(
                    str(call_summary["call_data_sha256"]),
                    label="route canary call data SHA-256",
                ),
                message_id=_parse_hex32(
                    str(transaction.get("message_id")),
                    label="route canary message id",
                ),
                source_domain=transaction_source_domain,
                target_domain=call_summary["public_inputs_target_domain"],
                payload_hash=_parse_hex32(
                    str(call_summary["public_inputs_payload_hash"]),
                    label="route canary payload hash",
                ),
                commitment_root=_parse_hex32(
                    str(transaction.get("commitment_root")),
                    label="route canary commitment root",
                ),
                finality_height=_parse_hex32(
                    str(call_summary["public_inputs_finality_height"]),
                    label="route canary finality height",
                ),
                finality_block_hash=_parse_hex32(
                    str(call_summary["public_inputs_finality_block_hash"]),
                    label="route canary finality block hash",
                ),
                statement_hash=_parse_hex32(
                    str(transaction.get("statement_hash")),
                    label="route canary statement hash",
                ),
                proof_version=call_summary["proof_version"],
                proof_source_domain=call_summary["proof_source_domain"],
                destination_binding_hash=transaction_destination_binding_hash,
                verifier_backend_hash=transaction_verifier_backend_hash,
                proof_family_hash=transaction_proof_family_hash,
                network_id=transaction_network_id,
                used_message_proof=transaction.get("message_proof_used") is True,
                raw_data_owner_matches_transaction=trigger_contract.get(
                    "raw_data_owner_matches_transaction",
                )
                is True,
                signature_sha256=_parse_hex32(
                    str(signature_sha256),
                    label="route canary signature hash",
                ),
                signature_recovered_address=_parse_tron_payload_hex(
                    signature_recovered_address,
                    label="route canary signature recovered address",
                ),
                signature_recovers_to_owner=trigger_contract.get(
                    "signature_recovers_to_owner",
                )
                is True,
            )
        )
    except (
        argparse.ArgumentTypeError,
        KeyError,
        RuntimeError,
        TypeError,
        ValueError,
    ):
        return False
    for key, expected in {**raw_summary, **call_summary, **signature_summary}.items():
        if trigger_contract.get(key) != expected:
            return False
    return (
        route_canary.get("evidence_source")
        == "tron_message_proof_accepted_transaction"
        and route_canary.get("transaction") == transaction
        and transaction.get("receipt_status") == "SUCCESS"
        and transaction.get("event_matches") is True
        and transaction.get("event_topic0") == _hex(TRON_MESSAGE_PROOF_ACCEPTED_TOPIC)
        and transaction.get("route_allowlist_hash") == _hex(route_allowlist_hash)
        and transaction_source_domain == destination_source_domain
        and transaction_destination_binding_hash == destination_binding_hash
        and transaction_verifier_backend_hash == destination_verifier_backend_hash
        and transaction_proof_family_hash == destination_proof_family_hash
        and transaction_network_id == destination_network_id
        and transaction.get("used_message_proofs_checked") is True
        and transaction.get("message_proof_used") is True
        and isinstance(route_canary.get("evidence_hash"), str)
        and route_canary.get("evidence_hash")
        == transaction.get("route_canary_evidence_hash")
        and route_canary.get("evidence_hash") == recomputed_hash
        and transaction.get("route_canary_evidence_hash") == recomputed_hash
        and trigger_contract.get("call_matches") is True
        and trigger_contract.get("raw_data_call_matches") is True
        and trigger_contract.get("raw_data_owner_matches_transaction") is True
        and trigger_contract.get("signature_recovers_to_owner") is True
        and trigger_contract.get("call_data_matches_event") is True
        and trigger_contract.get("function_selector")
        == _hex(TRON_SUBMIT_MESSAGE_PROOF_SELECTOR)
        and trigger_contract.get("function_signature")
        == "submitSccpMessageProof(bytes,bytes32[6],bytes32)"
        and trigger_contract.get("proof_bytes_length")
        == TRON_GROTH16_PROOF_ABI_BYTE_LENGTH
        and trigger_contract.get("proof_version") == TRON_GROTH16_PROOF_VERSION
        and trigger_contract.get("proof_source_domain")
        == transaction_source_domain
        and trigger_contract.get("public_inputs_target_domain")
        == destination.get("destination_target_domain")
        and trigger_contract.get("public_inputs_message_id")
        == transaction.get("message_id")
        and trigger_contract.get("public_inputs_commitment_root")
        == transaction.get("commitment_root")
        and trigger_contract.get("statement_hash") == transaction.get("statement_hash")
        and trigger_contract.get("signature_count") == 1
        and isinstance(owner_address, str)
        and isinstance(raw_data_owner_address, str)
        and raw_data_owner_address == owner_address
        and isinstance(signature_sha256, str)
        and isinstance(signature_recovered_address, str)
        and signature_recovered_address == owner_address
    )


def _offline_full_toml_args(summary: dict[str, Any]) -> list[str] | None:
    source = summary.get("source_bridge")
    if not isinstance(source, dict) or (
        source.get("expected_source_bridge_config_hash_matches") is not True
    ):
        return None
    if not (
        source.get("tron_getcontract_metadata_checked") is True
        and isinstance(source.get("source_bridge_emitter_code_hash"), str)
        and isinstance(source.get("source_bridge_runtime_bytecode_hex"), str)
    ):
        return None
    source_records = summary.get("source_records")
    if not isinstance(source_records, dict):
        return None
    if (
        source_records.get("expected_source_verifier_material_hash_matches") is not True
        or source_records.get(
            "expected_source_adapter_engine_deployment_hash_matches"
        )
        is not True
        or source_records.get("expected_tron_dpos_source_gate_hash_matches")
        is not True
    ):
        return None
    source_record_inputs = summary.get("source_record_inputs")
    if not isinstance(source_record_inputs, dict):
        return None
    if not isinstance(
        source_record_inputs.get("expected_source_verifier_material_hash"),
        str,
    ) or not isinstance(
        source_record_inputs.get("expected_source_adapter_engine_deployment_hash"),
        str,
    ) or not isinstance(
        source_record_inputs.get("expected_tron_dpos_source_gate_hash"),
        str,
    ):
        return None
    if not isinstance(summary.get("destination_verifier"), dict):
        return None
    destination = summary["destination_verifier"]
    if _destination_bytecode_metadata_error(destination) is not None:
        return None
    if destination.get("expected_destination_binding_hash_matches") is not True:
        return None
    if not isinstance(summary.get("route_allowlist_hash"), str):
        return None
    if not _route_canary_transaction_verified(summary):
        return None
    return [*_offline_args(summary), "--full-toml"]


def _full_toml_ready_except_destination_bytecode(
    summary: dict[str, Any],
    destination: dict[str, Any],
) -> bool:
    source = summary.get("source_bridge")
    if not isinstance(source, dict) or (
        source.get("expected_source_bridge_config_hash_matches") is not True
    ):
        return False
    if not (
        source.get("tron_getcontract_metadata_checked") is True
        and isinstance(source.get("source_bridge_emitter_code_hash"), str)
        and isinstance(source.get("source_bridge_runtime_bytecode_hex"), str)
    ):
        return False
    source_records = summary.get("source_records")
    if not isinstance(source_records, dict):
        return False
    if (
        source_records.get("expected_source_verifier_material_hash_matches") is not True
        or source_records.get(
            "expected_source_adapter_engine_deployment_hash_matches"
        )
        is not True
        or source_records.get("expected_tron_dpos_source_gate_hash_matches")
        is not True
    ):
        return False
    source_record_inputs = summary.get("source_record_inputs")
    if not isinstance(source_record_inputs, dict):
        return False
    if not isinstance(
        source_record_inputs.get("expected_source_verifier_material_hash"),
        str,
    ) or not isinstance(
        source_record_inputs.get("expected_source_adapter_engine_deployment_hash"),
        str,
    ) or not isinstance(
        source_record_inputs.get("expected_tron_dpos_source_gate_hash"),
        str,
    ):
        return False
    if destination.get("expected_destination_binding_hash_matches") is not True:
        return False
    if not isinstance(summary.get("route_allowlist_hash"), str):
        return False
    return _route_canary_transaction_verified(summary)


def _toml_comment(key: str, value: str) -> str:
    return "# " + key + " = " + json.dumps(value)


def _insert_comments_before_section(
    toml: str,
    *,
    section: str,
    comments: list[str],
) -> str:
    lines = toml.splitlines()
    for index, line in enumerate(lines):
        if line.strip() == section:
            existing_keys = set()
            for existing_line in lines[:index]:
                existing = existing_line.strip()
                if not existing.startswith("#") or "=" not in existing:
                    continue
                key, _value = existing[1:].split("=", 1)
                existing_keys.add(key.strip())
            missing_comments = []
            for comment in comments:
                key, _value = comment[1:].split("=", 1)
                if key.strip() not in existing_keys:
                    missing_comments.append(comment)
            return "\n".join([*lines[:index], *missing_comments, *lines[index:]]) + "\n"
    raise RuntimeError(f"generated TRON TOML is missing {section}")


def _destination_bytecode_metadata_error(destination: dict[str, Any]) -> str | None:
    destination_code_hash = destination.get("tron_getcontract_bytecode_hash")
    destination_view_code_hash = destination.get("destination_verifier_code_hash")
    destination_bytecode = destination.get("destination_verifier_runtime_bytecode_hex")
    if not isinstance(destination_code_hash, str):
        return (
            "TRON full TOML requires live /wallet/getcontract bytecode metadata "
            "for the destination verifier"
        )
    if not isinstance(destination_bytecode, str):
        return (
            "TRON full TOML requires live /wallet/getcontract runtime bytecode "
            "preimage for the destination verifier"
        )
    if destination.get("bytecode_hash_matches_verifier_code_hash") is not True:
        return (
            "TRON full TOML requires destination /wallet/getcontract bytecode "
            "to match verifierCodeHash()"
        )
    if not isinstance(destination_view_code_hash, str):
        return "TRON full TOML requires destination verifierCodeHash() evidence"
    try:
        runtime = evidence.parse_runtime_bytecode_hex(
            destination_bytecode,
            label="destination verifier runtime bytecode",
        )
    except (argparse.ArgumentTypeError, TypeError, ValueError):
        return "TRON destination verifier runtime bytecode metadata is invalid"
    recomputed_hash = _hex(evidence.runtime_bytecode_hash(runtime))
    if recomputed_hash != destination_code_hash:
        return (
            "TRON full TOML destination runtime bytecode hash does not match "
            f"/wallet/getcontract bytecode hash: expected {destination_code_hash}, "
            f"got {recomputed_hash}"
        )
    if destination_code_hash != destination_view_code_hash:
        return (
            "TRON full TOML destination bytecode metadata does not match "
            "verifierCodeHash(): "
            f"expected {destination_view_code_hash}, got {destination_code_hash}"
        )
    return None


def _annotate_full_toml_with_live_metadata(
    toml: str,
    summary: dict[str, Any],
) -> str:
    source = summary.get("source_bridge")
    destination = summary.get("destination_verifier")
    if not isinstance(source, dict) or not isinstance(destination, dict):
        raise ValueError("TRON full TOML requires source and destination evidence")
    source_code_hash = source.get("source_bridge_emitter_code_hash")
    source_bytecode = source.get("source_bridge_runtime_bytecode_hex")
    destination_code_hash = destination.get("tron_getcontract_bytecode_hash")
    if not (
        source.get("tron_getcontract_metadata_checked") is True
        and isinstance(source_code_hash, str)
        and isinstance(source_bytecode, str)
    ):
        raise ValueError(
            "TRON full TOML requires live /wallet/getcontract bytecode metadata "
            "and runtime bytecode preimage for the source bridge"
        )
    destination_error = _destination_bytecode_metadata_error(destination)
    if destination_error is not None:
        raise ValueError(destination_error) from None

    toml = _insert_comments_before_section(
        toml,
        section="[[zk.sccp_source_verifier_materials]]",
        comments=[
            _toml_comment("sccp_tron_source_bridge_address", str(source["address"])),
            _toml_comment(
                "sccp_tron_source_bridge_runtime_code_hash",
                source_code_hash,
            ),
            _toml_comment(
                "sccp_tron_source_bridge_runtime_bytecode_hex",
                source_bytecode,
            ),
            _toml_comment(
                "sccp_tron_source_bridge_config_hash",
                str(source["source_bridge_config_hash"]),
            ),
        ],
    )
    toml = _insert_comments_before_section(
        toml,
        section="[[zk.sccp_destination_rollouts]]",
        comments=[
            _toml_comment(
                "sccp_tron_destination_verifier_address",
                str(destination["address"]),
            ),
            _toml_comment(
                "sccp_tron_destination_verifier_runtime_code_hash",
                destination_code_hash,
            ),
            _toml_comment(
                "sccp_tron_destination_verifier_runtime_bytecode_hex",
                str(destination["destination_verifier_runtime_bytecode_hex"]),
            ),
            _toml_comment(
                "sccp_tron_destination_verifier_key_hash",
                str(destination["destination_verifier_key_hash"]),
            ),
            _toml_comment(
                "sccp_tron_destination_verifier_backend_hash",
                str(destination["verifier_backend_hash"]),
            ),
            _toml_comment(
                "sccp_tron_destination_proof_family_hash",
                str(destination["proof_family_hash"]),
            ),
        ],
    )
    route_canary_transaction = summary.get("route_canary_transaction")
    if isinstance(route_canary_transaction, dict):
        toml = _insert_comments_before_section(
            toml,
            section="[[zk.sccp_route_allowlists]]",
            comments=[
                _toml_comment(
                    "sccp_tron_route_canary_transaction_id",
                    str(route_canary_transaction["transaction_id"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_transaction_owner_address",
                    str(route_canary_transaction["trigger_contract"]["owner_address"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_block_number",
                    str(route_canary_transaction["block_number"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_block_timestamp",
                    str(route_canary_transaction["block_timestamp"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_log_index",
                    str(route_canary_transaction["log_index"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_message_id",
                    str(route_canary_transaction["message_id"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_call_data_sha256",
                    str(route_canary_transaction["trigger_contract"]["call_data_sha256"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_payload_hash",
                    str(
                        route_canary_transaction["trigger_contract"][
                            "public_inputs_payload_hash"
                        ]
                    ),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_target_domain",
                    str(
                        route_canary_transaction["trigger_contract"][
                            "public_inputs_target_domain"
                        ]
                    ),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_statement_hash",
                    str(route_canary_transaction["statement_hash"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_commitment_root",
                    str(route_canary_transaction["commitment_root"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_finality_height",
                    str(
                        route_canary_transaction["trigger_contract"][
                            "public_inputs_finality_height"
                        ]
                    ),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_finality_block_hash",
                    str(
                        route_canary_transaction["trigger_contract"][
                            "public_inputs_finality_block_hash"
                        ]
                    ),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_proof_version",
                    str(route_canary_transaction["trigger_contract"]["proof_version"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_proof_source_domain",
                    str(
                        route_canary_transaction["trigger_contract"][
                            "proof_source_domain"
                        ]
                    ),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_used_message_proof",
                    (
                        "true"
                        if route_canary_transaction["message_proof_used"]
                        else "false"
                    ),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_raw_data_owner_matches_transaction",
                    (
                        "true"
                        if route_canary_transaction["trigger_contract"][
                            "raw_data_owner_matches_transaction"
                        ]
                        else "false"
                    ),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_signature_sha256",
                    str(route_canary_transaction["trigger_contract"]["signature_sha256"]),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_signature_recovered_address",
                    str(
                        route_canary_transaction["trigger_contract"][
                            "signature_recovered_address"
                        ]
                    ),
                ),
                _toml_comment(
                    "sccp_tron_route_canary_signature_recovers_to_owner",
                    (
                        "true"
                        if route_canary_transaction["trigger_contract"][
                            "signature_recovers_to_owner"
                        ]
                        else "false"
                    ),
                ),
            ],
        )
    return toml


def render_offline_full_toml(summary: dict[str, Any]) -> str:
    """Render full governance TOML from a complete live-evidence summary."""

    args = _offline_full_toml_args(summary)
    if args is None:
        destination = summary.get("destination_verifier")
        if isinstance(
            destination, dict
        ) and _full_toml_ready_except_destination_bytecode(summary, destination):
            destination_error = _destination_bytecode_metadata_error(destination)
            if destination_error is not None:
                raise ValueError(destination_error) from None
        raise ValueError(
            "full TOML output requires --expected-source-bridge-config-hash, "
            "complete source records, destination verifier evidence, expected "
            "source record hashes, --expected-tron-dpos-source-gate-hash, "
            "--expected-destination-binding-hash, and --route-allowlist-hash "
            "plus a verified "
            "--route-canary-transaction-id with raw_data owner binding"
        )
    offline_parser = evidence.build_parser()
    try:
        offline_args = offline_parser.parse_args(args)
    except SystemExit:
        raise RuntimeError(
            "generated offline full TOML arguments are invalid"
        ) from None
    evidence.apply_runtime_bytecode_hashes(offline_args)
    config_hash = evidence.tron_source_bridge_config_hash(
        bridge_address=offline_args.bridge_address,
        network_id=offline_args.network_id,
        source_domain=offline_args.source_domain,
        target_domain=offline_args.target_domain,
        owner_address=offline_args.owner_address,
    )
    return _annotate_full_toml_with_live_metadata(
        evidence.render_full_toml(offline_args, config_hash),
        summary,
    )


def _torii_destination_query_params(summary: dict[str, Any]) -> dict[str, str] | None:
    destination = summary.get("destination_verifier")
    if not isinstance(destination, dict):
        return None
    if destination.get("expected_destination_binding_hash_matches") is not True:
        return None
    if destination.get("destination_binding_hash_matches") is not True:
        return None
    if destination.get("verifier_backend_hash_matches") is not True:
        return None
    if destination.get("proof_family_hash_matches") is not True:
        return None
    if _destination_bytecode_metadata_error(destination) is not None:
        return None
    try:
        source_domain = _parse_canonical_u32(
            destination["destination_source_domain"],
            label="destination source domain",
        )
        target_domain = _parse_canonical_u32(
            destination["destination_target_domain"],
            label="destination target domain",
        )
        network_id = _parse_hex32(str(destination["network_id"]), label="network id")
        verifier_code_hash = _parse_hex32(
            str(destination["destination_verifier_code_hash"]),
            label="destination verifier code hash",
        )
        verifier_key_hash = _parse_hex32(
            str(destination["destination_verifier_key_hash"]),
            label="destination verifier key hash",
        )
        destination_binding_hash = _parse_hex32(
            str(destination["destination_binding_hash"]),
            label="destination binding hash",
        )
        recomputed_binding_hash = evidence.tron_destination_binding_hash(
            network_id=network_id,
            source_domain=source_domain,
            target_domain=target_domain,
            verifier_address=str(destination["address"]),
            verifier_code_hash=verifier_code_hash,
            verifier_key_hash=verifier_key_hash,
        )
    except (KeyError, TypeError, ValueError, argparse.ArgumentTypeError):
        return None
    if recomputed_binding_hash != destination_binding_hash:
        return None
    return {
        "network_id_hex": str(destination["network_id"]),
        "tron_verifier_address": str(destination["address"]),
        "verifier_code_hash_hex": str(destination["destination_verifier_code_hash"]),
        "verifier_key_hash_hex": str(destination["destination_verifier_key_hash"]),
        "expected_destination_binding_hash_hex": str(destination["destination_binding_hash"]),
    }


def _runtime_tron_pro_api_key(args: argparse.Namespace) -> str | None:
    inline_key = getattr(args, "tron_pro_api_key", None)
    key_file = getattr(args, "tron_pro_api_key_file", None)
    if inline_key is not None and key_file is not None:
        raise ValueError("--tron-pro-api-key and --tron-pro-api-key-file cannot both be supplied")
    from_file = key_file is not None
    if key_file is not None:
        try:
            inline_key = Path(key_file).expanduser().read_text(encoding="utf-8")
        except OSError:
            raise ValueError("--tron-pro-api-key-file cannot be read") from None
    if inline_key is None:
        return None
    return _tron_pro_api_key_token(
        inline_key.rstrip("\r\n") if from_file else inline_key,
        label="TRON-PRO-API-KEY",
    )


def _runtime_witness_schedule_payload(args: argparse.Namespace) -> bytes | None:
    inline_payload = getattr(args, "witness_schedule_payload_hex", None)
    payload_file = getattr(args, "witness_schedule_payload_file", None)
    if inline_payload is not None and payload_file is not None:
        raise ValueError(
            "--witness-schedule-payload-hex and "
            "--witness-schedule-payload-file cannot both be supplied"
        )
    from_file = payload_file is not None
    if from_file:
        try:
            inline_payload = Path(payload_file).expanduser().read_text(
                encoding="utf-8"
            )
        except OSError:
            raise ValueError("--witness-schedule-payload-file cannot be read") from None
    if inline_payload is None:
        return None
    if not from_file:
        return _parse_exact_hex_blob(
            str(inline_payload),
            label="witness schedule payload",
        )
    return _parse_hex_blob(
        "".join(str(inline_payload).split()),
        label="witness schedule payload",
    )


def _runtime_witness_seal_signatures(args: argparse.Namespace) -> list[bytes]:
    signatures = []
    for index, value in enumerate(getattr(args, "witness_seal_signature_hex", []) or []):
        signatures.append(
            _parse_exact_hex_blob(
                value,
                label=f"witness seal signature {index}",
            )
        )
    return signatures


def _runtime_witness_schedule_transitions(
    args: argparse.Namespace,
) -> list[dict[str, Any]]:
    transitions = []
    values = getattr(args, "witness_schedule_transition_json", []) or []
    for index, value in enumerate(values):
        if not isinstance(value, str):
            raise ValueError(
                f"--witness-schedule-transition-json {index} must be JSON text"
            )
        text = value
        if text.startswith("@"):
            try:
                text = Path(text[1:]).expanduser().read_text(encoding="utf-8")
            except OSError:
                raise ValueError(
                    f"--witness-schedule-transition-json {index} file cannot be read"
                ) from None
        try:
            parsed = json.loads(
                text,
                object_pairs_hook=_json_object_without_duplicate_keys,
            )
        except json.JSONDecodeError:
            raise ValueError(
                f"--witness-schedule-transition-json {index} must be JSON"
            ) from None
        except ValueError:
            raise ValueError(
                f"--witness-schedule-transition-json {index} must not contain "
                "duplicate JSON keys"
            ) from None
        if not isinstance(parsed, dict):
            raise ValueError(
                f"--witness-schedule-transition-json {index} must be a JSON object"
            )
        transitions.append(parsed)
    return transitions


def _runtime_hex32_list_arg(args: argparse.Namespace, name: str) -> list[bytes]:
    values = getattr(args, name, []) or []
    return [
        _parse_exact_hex32_blob(
            value,
            label=f"{name.replace('_', ' ')} {index}",
            nonzero=False,
        )
        for index, value in enumerate(values)
    ]


def _bounded_header_depth(args: argparse.Namespace, name: str) -> int:
    value = getattr(args, name, 0)
    if value is None:
        value = 0
    if type(value) is not int:
        raise ValueError(f"--{name.replace('_', '-')} must be an integer")
    if value < 0 or value > TRON_MAX_SOLID_BLOCK_EXTRA_HEADERS:
        raise ValueError(
            f"--{name.replace('_', '-')} must be in "
            f"0..{TRON_MAX_SOLID_BLOCK_EXTRA_HEADERS}"
        )
    return value


def _constant_endpoint(args: argparse.Namespace) -> str:
    if getattr(args, "solid", False):
        return "walletsolidity/triggerconstantcontract"
    return "wallet/triggerconstantcontract"


def _transaction_info_endpoint(args: argparse.Namespace) -> str:
    if getattr(args, "solid", False):
        return "walletsolidity/gettransactioninfobyid"
    return "wallet/gettransactioninfobyid"


def _transaction_endpoint(args: argparse.Namespace) -> str:
    if getattr(args, "solid", False):
        return "walletsolidity/gettransactionbyid"
    return "wallet/gettransactionbyid"


def _block_endpoint(args: argparse.Namespace) -> str:
    if getattr(args, "solid", False):
        return "walletsolidity/getblockbynum"
    return "wallet/getblockbynum"


def collect_live_evidence(
    args: argparse.Namespace,
    *,
    opener: Urlopen = urllib.request.urlopen,
) -> dict[str, Any]:
    """Collect all requested evidence and return a JSON-serializable summary."""

    caller_address = None
    if args.caller_address is not None:
        caller_payload = parse_tron_address_payload(args.caller_address, label="caller address")
        caller_address = tron_base58check_from_payload(caller_payload)
    source_event_digest = _optional_hex32_arg(args, "source_event_digest")
    source_event_transaction_id = _optional_hex32_arg(
        args,
        "source_event_transaction_id",
    )
    if source_event_transaction_id is not None and source_event_digest is None:
        raise ValueError("--source-event-transaction-id requires --source-event-digest")
    if args.source_bridge_address is None and source_event_digest is not None:
        raise ValueError("--source-event-digest requires --source-bridge-address")
    if args.source_bridge_address is None and _source_record_preflight_requested(args):
        raise ValueError(
            "source record/config hash preflight requires --source-bridge-address"
        )
    if source_event_digest is not None and getattr(args, "full_toml", False):
        raise ValueError("--source-event-digest is only supported for JSON evidence output")
    if (
        _optional_hex32_arg(args, "expected_destination_binding_hash") is not None
        and args.destination_verifier_address is None
    ):
        raise ValueError(
            "--expected-destination-binding-hash requires "
            "--destination-verifier-address"
        )
    tron_pro_api_key = _runtime_tron_pro_api_key(args)
    active_witness_schedule_payload = _runtime_witness_schedule_payload(args)
    expected_witness_schedule_hash = _optional_hex32_arg(
        args,
        "expected_witness_schedule_hash",
    )
    receipt_root = _optional_hex32_arg(args, "receipt_root")
    receipt_proof_hash = _optional_hex32_arg(args, "receipt_proof_hash")
    witness_seal_signers_bitmap = _optional_hex_blob_arg(
        args,
        "witness_seal_signers_bitmap_hex",
    )
    witness_seal_signatures = _runtime_witness_seal_signatures(args)
    witness_schedule_transition_inputs = _runtime_witness_schedule_transitions(args)
    expected_witness_seal_hash = _optional_hex32_arg(
        args,
        "expected_witness_seal_hash",
    )
    source_inclusion_branch = _runtime_hex32_list_arg(
        args,
        "source_inclusion_branch_hex",
    )
    solid_block_ancestor_depth = _bounded_header_depth(
        args,
        "solid_block_ancestor_depth",
    )
    solid_block_confirmation_depth = _bounded_header_depth(
        args,
        "solid_block_confirmation_depth",
    )
    summary: dict[str, Any] = {
        "tron_node_url": args.tron_node_url.rstrip("/"),
        "read_only": True,
        "constant_endpoint": _constant_endpoint(args),
        "transaction_info_endpoint": _transaction_info_endpoint(args),
        "transaction_endpoint": _transaction_endpoint(args),
        "block_endpoint": _block_endpoint(args),
    }
    if args.source_bridge_address is not None:
        summary["source_bridge"] = collect_source_bridge_evidence(
            args.tron_node_url,
            source_bridge_address=args.source_bridge_address,
            caller_address=caller_address,
            tron_pro_api_key=tron_pro_api_key,
            constant_endpoint=str(summary["constant_endpoint"]),
            include_contract_metadata=not args.no_getcontract,
            opener=opener,
            timeout=args.timeout,
        )
        _check_expected_source_config_hash(summary["source_bridge"], args)
        source_records = _collect_source_record_hashes(summary["source_bridge"], args)
        if source_records is not None:
            summary["source_records"] = source_records
            summary["source_record_inputs"] = {
                key: _hex(_optional_hex32_arg(args, key))
                for key in _SOURCE_RECORD_HASH_FIELDS
            }
            summary["source_record_inputs"]["source_bridge_emitter_code_hash"] = str(
                summary["source_bridge"]["source_bridge_emitter_code_hash"]
            )
            summary["source_record_inputs"]["adapter_verifier_vk_hash"] = str(
                source_records["adapter_verifier_vk_hash"]
            )
            expected_material_hash = _optional_hex32_arg(
                args,
                "expected_source_verifier_material_hash",
            )
            if expected_material_hash is not None:
                summary["source_record_inputs"][
                    "expected_source_verifier_material_hash"
                ] = _hex(expected_material_hash)
            expected_deployment_hash = _optional_hex32_arg(
                args,
                "expected_source_adapter_engine_deployment_hash",
            )
            if expected_deployment_hash is not None:
                summary["source_record_inputs"][
                    "expected_source_adapter_engine_deployment_hash"
                ] = _hex(expected_deployment_hash)
            expected_gate_hash = _optional_hex32_arg(
                args,
                "expected_tron_dpos_source_gate_hash",
            )
            if expected_gate_hash is not None:
                summary["source_record_inputs"][
                    "expected_tron_dpos_source_gate_hash"
                ] = _hex(expected_gate_hash)
        if source_event_digest is not None:
            effective_expected_witness_schedule_hash = (
                _effective_expected_witness_schedule_hash(
                    summary,
                    expected_witness_schedule_hash,
                )
            )
            source_domain = _parse_canonical_u32(
                summary["source_bridge"]["source_domain"],
                label="source bridge source domain",
            )
            target_domain = _parse_canonical_u32(
                summary["source_bridge"]["target_domain"],
                label="source bridge target domain",
            )
            source_event_owner_for_call = caller_address or str(
                summary["source_bridge"]["source_bridge_owner_base58"]
            )
            source_event_call_data = evidence.tron_source_message_call_data(
                source_domain=source_domain,
                target_domain=target_domain,
                source_event_digest=source_event_digest,
            )
            already_submitted = _word_bool(
                _constant_word(
                    args.tron_node_url,
                    endpoint=str(summary["constant_endpoint"]),
                    contract_address=str(summary["source_bridge"]["address"]),
                    function_selector="submittedSourceEvents(bytes32)",
                    parameter=source_event_digest.hex(),
                    owner_address=source_event_owner_for_call,
                    tron_pro_api_key=tron_pro_api_key,
                    opener=opener,
                    timeout=args.timeout,
                ),
                label="submittedSourceEvents(bytes32)",
            )
            if already_submitted:
                if source_event_transaction_id is None:
                    raise ValueError(
                        "--source-event-digest has already been submitted on the "
                        "queried source bridge"
                    )
            elif source_event_transaction_id is not None:
                raise ValueError(
                    "--source-event-transaction-id was supplied, but "
                    "submittedSourceEvents(bytes32) is false for the digest"
                )
            source_event_call: dict[str, Any] = {
                "source_bridge_address": str(summary["source_bridge"]["address"]),
                "source_bridge_owner_address": str(
                    summary["source_bridge"]["source_bridge_owner_address"]
                ),
                "source_bridge_owner_base58": str(
                    summary["source_bridge"]["source_bridge_owner_base58"]
                ),
                "source_domain": source_domain,
                "target_domain": target_domain,
                "source_event_digest": _hex(source_event_digest),
                "source_event_call_data": _hex(source_event_call_data),
                "submitted_source_events_checked": True,
                "source_event_already_submitted": already_submitted,
                "transaction_required": source_event_transaction_id is None,
            }
            if source_event_transaction_id is None:
                source_event_call["trigger_request"] = {
                    "endpoint": "wallet/triggersmartcontract",
                    "owner_address": str(
                        summary["source_bridge"]["source_bridge_owner_base58"]
                    ),
                    "contract_address": str(summary["source_bridge"]["address"]),
                    "function_selector": evidence.TRON_SOURCE_MESSAGE_CALL_ABI.decode(
                        "ascii"
                    ),
                    "parameter": source_event_call_data[4:].hex(),
                    "visible": True,
                    "call_value": 0,
                }
            else:
                transaction = _transaction_by_id(
                    args.tron_node_url,
                    endpoint=str(summary["transaction_endpoint"]),
                    transaction_id=source_event_transaction_id,
                    tron_pro_api_key=tron_pro_api_key,
                    opener=opener,
                    timeout=args.timeout,
                )
                transaction_info = _transaction_info(
                    args.tron_node_url,
                    endpoint=str(summary["transaction_info_endpoint"]),
                    transaction_id=source_event_transaction_id,
                    tron_pro_api_key=tron_pro_api_key,
                    opener=opener,
                    timeout=args.timeout,
                )
                bridge_payload = parse_tron_address_payload(
                    str(summary["source_bridge"]["address"]),
                    label="source bridge address",
                )
                owner_payload = parse_tron_address_payload(
                    str(summary["source_bridge"]["source_bridge_owner_base58"]),
                    label="source bridge owner address",
                )
                transaction_summary = _source_event_transaction_summary(
                    transaction_info,
                    transaction_id=source_event_transaction_id,
                    source_bridge_address20=bridge_payload[1:],
                    source_event_digest=source_event_digest,
                )
                block_number = transaction_summary.get("block_number")
                if type(block_number) is not int or block_number <= 0:
                    raise RuntimeError(
                        "source-event transaction info must include positive blockNumber"
                    )
                trigger_contract = _source_event_trigger_contract_summary(
                    transaction,
                    transaction_id=source_event_transaction_id,
                    source_bridge_payload=bridge_payload,
                    owner_payload=owner_payload,
                    source_event_call_data=source_event_call_data,
                )
                block = _block_by_number(
                    args.tron_node_url,
                    endpoint=str(summary["block_endpoint"]),
                    block_number=block_number,
                    tron_pro_api_key=tron_pro_api_key,
                    opener=opener,
                    timeout=args.timeout,
                )
                parent_block = _block_by_number(
                    args.tron_node_url,
                    endpoint=str(summary["block_endpoint"]),
                    block_number=block_number - 1,
                    tron_pro_api_key=tron_pro_api_key,
                    opener=opener,
                    timeout=args.timeout,
                )
                if (
                    solid_block_ancestor_depth > 0
                    and block_number - 1 - solid_block_ancestor_depth <= 0
                ):
                    raise RuntimeError(
                        "solid-block ancestor depth reaches before block 1"
                    )
                ancestor_blocks = [
                    _block_by_number(
                        args.tron_node_url,
                        endpoint=str(summary["block_endpoint"]),
                        block_number=block_number - 2 - index,
                        tron_pro_api_key=tron_pro_api_key,
                        opener=opener,
                        timeout=args.timeout,
                    )
                    for index in range(solid_block_ancestor_depth)
                ]
                confirmation_blocks = [
                    _block_by_number(
                        args.tron_node_url,
                        endpoint=str(summary["block_endpoint"]),
                        block_number=block_number + 1 + index,
                        tron_pro_api_key=tron_pro_api_key,
                        opener=opener,
                        timeout=args.timeout,
                    )
                    for index in range(solid_block_confirmation_depth)
                ]
                transaction_summary["trigger_contract"] = trigger_contract
                solid_block = _source_event_solid_block_summary(
                    block,
                    parent_response=parent_block,
                    ancestor_responses=ancestor_blocks,
                    confirmation_responses=confirmation_blocks,
                    active_witness_schedule_payload=active_witness_schedule_payload,
                    expected_witness_schedule_hash=(
                        effective_expected_witness_schedule_hash
                    ),
                    receipt_root=receipt_root,
                    receipt_proof_hash=receipt_proof_hash,
                    witness_seal_signers_bitmap=witness_seal_signers_bitmap,
                    witness_seal_signatures=witness_seal_signatures,
                    expected_witness_seal_hash=expected_witness_seal_hash,
                    witness_schedule_transition_inputs=(
                        witness_schedule_transition_inputs
                    ),
                    source_event_digest=source_event_digest,
                    source_inclusion_branch=source_inclusion_branch,
                    source_bridge_address20=bridge_payload[1:],
                    owner_address20=owner_payload[1:],
                    transaction_id=source_event_transaction_id,
                    block_number=block_number,
                    source_transaction_bytes=_parse_hex_blob(
                        trigger_contract["source_proof_transaction_bytes"],
                        label="source proof transaction bytes",
                    ),
                    source_transaction_hash=_parse_hex32(
                        trigger_contract["source_proof_transaction_hash"],
                        label="source proof transaction hash",
                    ),
                )
                if transaction_summary["block_timestamp"] != solid_block["block_timestamp"]:
                    raise RuntimeError(
                        "source-event transaction info blockTimeStamp does not "
                        "match block header timestamp"
                    )
                transaction_summary["solid_block"] = solid_block
                transaction_summary.update(
                    _source_event_transaction_production_readiness(
                        transaction_summary["solid_block"]
                    )
                )
                summary["source_event_transaction"] = transaction_summary
            summary["source_event_call"] = source_event_call
    if args.destination_verifier_address is not None:
        summary["destination_verifier"] = collect_destination_verifier_evidence(
            args.tron_node_url,
            destination_verifier_address=args.destination_verifier_address,
            caller_address=caller_address,
            tron_pro_api_key=tron_pro_api_key,
            constant_endpoint=str(summary["constant_endpoint"]),
            include_contract_metadata=not args.no_getcontract,
            opener=opener,
            timeout=args.timeout,
        )
        _check_expected_destination_binding_hash(
            summary["destination_verifier"],
            args,
        )
    _check_source_destination_network_id_match(summary)
    route_allowlist_hash = _optional_hex32_arg(args, "route_allowlist_hash")
    route_canary_evidence_hash = _optional_hex32_arg(args, "route_canary_evidence_hash")
    route_canary_transaction_id = _optional_hex32_arg(
        args,
        "route_canary_transaction_id",
    )
    if route_canary_evidence_hash is not None and route_allowlist_hash is None:
        raise ValueError("--route-canary-evidence-hash requires --route-allowlist-hash")
    if route_canary_transaction_id is not None and route_allowlist_hash is None:
        raise ValueError("--route-canary-transaction-id requires --route-allowlist-hash")
    if (
        route_canary_transaction_id is not None
        and not isinstance(summary.get("destination_verifier"), dict)
    ):
        raise ValueError(
            "--route-canary-transaction-id requires --destination-verifier-address"
        )
    route_canary_transaction = None
    if route_canary_transaction_id is not None:
        response = _transaction_info(
            args.tron_node_url,
            endpoint=str(summary["transaction_info_endpoint"]),
            transaction_id=route_canary_transaction_id,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=args.timeout,
        )
        route_canary_transaction = _route_canary_transaction_summary(
            response,
            transaction_id=route_canary_transaction_id,
            route_allowlist_hash=route_allowlist_hash,
            destination_verifier=summary["destination_verifier"],
        )
        route_canary_message_id = _parse_hex32(
            str(route_canary_transaction["message_id"]),
            label="route-canary accepted message id",
        )
        route_canary_transaction.update(
            _route_canary_used_message_proof_summary(
                args.tron_node_url,
                constant_endpoint=str(summary["constant_endpoint"]),
                destination_verifier=summary["destination_verifier"],
                message_id=route_canary_message_id,
                caller_address=caller_address,
                tron_pro_api_key=tron_pro_api_key,
                opener=opener,
                timeout=args.timeout,
            )
        )
        transaction = _transaction_by_id(
            args.tron_node_url,
            endpoint=str(summary["transaction_endpoint"]),
            transaction_id=route_canary_transaction_id,
            tron_pro_api_key=tron_pro_api_key,
            opener=opener,
            timeout=args.timeout,
        )
        route_canary_transaction["trigger_contract"] = (
            _route_canary_trigger_contract_summary(
                transaction,
                transaction_id=route_canary_transaction_id,
                destination_verifier=summary["destination_verifier"],
                event_summary=route_canary_transaction,
            )
        )
        trigger_contract = route_canary_transaction["trigger_contract"]
        derived_canary_hash_bytes = _tron_route_canary_transaction_evidence_hash(
            route_allowlist_hash=route_allowlist_hash,
            transaction_id=route_canary_transaction_id,
            transaction_owner_address=_parse_tron_payload_hex(
                trigger_contract["owner_address"],
                label="route-canary transaction owner address",
            ),
            block_number=route_canary_transaction["block_number"],
            block_timestamp=route_canary_transaction["block_timestamp"],
            log_index=route_canary_transaction["log_index"],
            verifier_address20=parse_tron_address_payload(
                str(summary["destination_verifier"]["address"]),
                label="destination verifier address",
            )[1:],
            call_data_sha256=_parse_hex32(
                str(trigger_contract["call_data_sha256"]),
                label="route-canary call data SHA-256",
            ),
            message_id=_parse_hex32(
                str(route_canary_transaction["message_id"]),
                label="route-canary accepted message id",
            ),
            source_domain=route_canary_transaction["source_domain"],
            target_domain=trigger_contract["public_inputs_target_domain"],
            payload_hash=_parse_hex32(
                str(trigger_contract["public_inputs_payload_hash"]),
                label="route-canary payload hash",
            ),
            commitment_root=_parse_hex32(
                str(route_canary_transaction["commitment_root"]),
                label="route-canary commitment root",
            ),
            finality_height=_parse_hex32(
                str(trigger_contract["public_inputs_finality_height"]),
                label="route-canary finality height",
            ),
            finality_block_hash=_parse_hex32(
                str(trigger_contract["public_inputs_finality_block_hash"]),
                label="route-canary finality block hash",
            ),
            statement_hash=_parse_hex32(
                str(route_canary_transaction["statement_hash"]),
                label="route-canary statement hash",
            ),
            proof_version=trigger_contract["proof_version"],
            proof_source_domain=trigger_contract["proof_source_domain"],
            destination_binding_hash=_parse_hex32(
                str(route_canary_transaction["destination_binding_hash"]),
                label="route-canary destination binding hash",
            ),
            verifier_backend_hash=_parse_hex32(
                str(route_canary_transaction["verifier_backend_hash"]),
                label="route-canary verifier backend hash",
            ),
            proof_family_hash=_parse_hex32(
                str(route_canary_transaction["proof_family_hash"]),
                label="route-canary proof family hash",
            ),
            network_id=_parse_hex32(
                str(route_canary_transaction["network_id"]),
                label="route-canary network id",
            ),
            used_message_proof=route_canary_transaction["message_proof_used"] is True,
            raw_data_owner_matches_transaction=trigger_contract[
                "raw_data_owner_matches_transaction"
            ]
            is True,
            signature_sha256=_parse_hex32(
                str(trigger_contract["signature_sha256"]),
                label="route-canary signature hash",
            ),
            signature_recovered_address=_parse_tron_payload_hex(
                trigger_contract["signature_recovered_address"],
                label="route-canary signature recovered address",
            ),
            signature_recovers_to_owner=trigger_contract["signature_recovers_to_owner"]
            is True,
        )
        route_canary_transaction["route_canary_evidence_hash"] = _hex(
            derived_canary_hash_bytes
        )
        derived_canary_hash = _parse_hex32(
            str(route_canary_transaction["route_canary_evidence_hash"]),
            label="derived route canary evidence hash",
        )
        if (
            route_canary_evidence_hash is not None
            and route_canary_evidence_hash != derived_canary_hash
        ):
            raise ValueError(
                "--route-canary-evidence-hash does not match the "
                "MessageProofAccepted transaction evidence hash: "
                f"expected {_hex(derived_canary_hash)}, "
                f"got {_hex(route_canary_evidence_hash)}"
            )
        route_canary_evidence_hash = derived_canary_hash
        summary["route_canary_transaction"] = route_canary_transaction
    if route_allowlist_hash is not None:
        summary.update(
            _validate_route_allowlist_hash(
                supplied_hash=route_allowlist_hash,
                route_canary_evidence_hash=route_canary_evidence_hash,
                source_records=summary.get("source_records"),
                destination_verifier=summary.get("destination_verifier"),
                destination_binding_pinned=summary.get(
                    "destination_verifier",
                    {},
                ).get("expected_destination_binding_hash_matches")
                is True,
            )
        )
        if (
            route_canary_transaction is not None
            and isinstance(summary.get("route_canary"), dict)
        ):
            summary["route_canary"]["evidence_source"] = (
                "tron_message_proof_accepted_transaction"
            )
            summary["route_canary"]["transaction"] = route_canary_transaction
    summary["offline_evidence_args"] = _offline_args(summary)
    offline_source_event_args = _offline_source_event_args(summary)
    if offline_source_event_args is not None:
        summary["offline_source_event_args"] = offline_source_event_args
    offline_full_toml_args = _offline_full_toml_args(summary)
    summary["full_toml_ready"] = offline_full_toml_args is not None
    if offline_full_toml_args is not None:
        summary["offline_full_toml_args"] = offline_full_toml_args
        offline_full_toml = render_offline_full_toml(summary)
        summary["offline_full_toml_sha256"] = hashlib.sha256(
            offline_full_toml.encode("utf-8")
        ).hexdigest()
    torii_destination_query_params = _torii_destination_query_params(summary)
    if torii_destination_query_params is not None:
        summary["torii_destination_query_params"] = torii_destination_query_params
        summary["torii_destination_query_proof_bytes_hex_required"] = True
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Read TRON SCCP deployment view functions and recompute production "
            "source/destination evidence hashes."
        ),
    )
    parser.add_argument(
        "--tron-node-url",
        default="https://api.trongrid.io",
        help="TRON full-node or TronGrid-compatible HTTP API URL.",
    )
    parser.add_argument(
        "--source-bridge-address",
        help="Deployed SccpTronSourceBridge address, as base58check or hex.",
    )
    parser.add_argument(
        "--destination-verifier-address",
        help=(
            "Deployed SccpTronGroth16Bn254MessageVerifier address, as "
            "base58check or hex."
        ),
    )
    parser.add_argument(
        "--caller-address",
        help=(
            "Optional caller address for constant calls. Defaults to the "
            "queried contract address."
        ),
    )
    parser.add_argument(
        "--tron-pro-api-key",
        help=(
            "Runtime-only TronGrid API key sent as TRON-PRO-API-KEY. The key is "
            "never printed in evidence output."
        ),
    )
    parser.add_argument(
        "--tron-pro-api-key-file",
        help=(
            "Path to a runtime-only file containing the TronGrid API key to "
            "send as TRON-PRO-API-KEY."
        ),
    )
    parser.add_argument(
        "--solid",
        action="store_true",
        help=(
            "Read view functions from /walletsolidity/triggerconstantcontract "
            "instead of the non-solid full-node endpoint."
        ),
    )
    parser.add_argument(
        "--no-getcontract",
        action="store_true",
        help=(
            "Skip /wallet/getcontract bytecode metadata lookups. This is a "
            "diagnostic JSON path only; live --full-toml requires source and "
            "destination bytecode metadata."
        ),
    )
    parser.add_argument(
        "--source-trust-anchor-hash",
        type=lambda value: _parse_hex32(value, label="source trust anchor hash"),
        help="Governed non-zero TRON source trust-anchor hash for source record preflight.",
    )
    parser.add_argument(
        "--consensus-verifier-hash",
        type=lambda value: _parse_hex32(value, label="consensus verifier hash"),
        help="Governed non-zero TRON solid-block verifier hash for source record preflight.",
    )
    parser.add_argument(
        "--message-inclusion-verifier-hash",
        type=lambda value: _parse_hex32(value, label="message inclusion verifier hash"),
        help="Governed non-zero TRON receipt/message verifier hash for source record preflight.",
    )
    parser.add_argument(
        "--source-bridge-emitter-code-hash",
        type=lambda value: _parse_hex32(value, label="source bridge emitter code hash"),
        help=(
            "Expected deployed source bridge code hash. When /wallet/getcontract "
            "is enabled, it must match the collected runtime bytecode hash."
        ),
    )
    parser.add_argument(
        "--expected-source-bridge-config-hash",
        "--expected-config-hash",
        dest="expected_source_bridge_config_hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected source bridge config hash",
        ),
        help=(
            "Governed SourceBridgeConfigured/sourceBridgeConfigHash value to "
            "compare against live source bridge state. Optional for JSON "
            "dry-runs and required for live --full-toml."
        ),
    )
    parser.add_argument(
        "--source-event-digest",
        type=lambda value: _parse_hex32(value, label="source event digest"),
        help=(
            "Optional non-zero SCCP source event digest. JSON output includes "
            "the owner-call calldata for submitSccpSourceEvent(uint32,uint32,bytes32)."
        ),
    )
    parser.add_argument(
        "--source-event-transaction-id",
        type=lambda value: _parse_hex32(value, label="source event transaction id"),
        help=(
            "Optional post-submit TRON transaction id. Requires "
            "--source-event-digest and verifies the successful "
            "SccpSourceEvent(bytes32) log plus TriggerSmartContract calldata "
            "through read-only transaction endpoints; JSON also reports "
            "source_event_transaction_production_ready once all TRON proof "
            "material is present."
        ),
    )
    parser.add_argument(
        "--witness-schedule-payload-hex",
        help=(
            "Canonical sccp:tron:witness-schedule-payload:v1 bytes as hex. "
            "When supplied with --source-event-transaction-id, the live helper "
            "verifies child and parent block witnesses are members."
        ),
    )
    parser.add_argument(
        "--witness-schedule-payload-file",
        help=(
            "File containing canonical witness-schedule payload hex. Mutually "
            "exclusive with --witness-schedule-payload-hex."
        ),
    )
    parser.add_argument(
        "--expected-witness-schedule-hash",
        type=lambda value: _parse_hex32(value, label="expected witness schedule hash"),
        help=(
            "Optional expected sccp:tron:witness-schedule:v1 hash for the "
            "supplied witness-schedule payload. Production-ready source-event "
            "JSON requires this hash, unless source record preflight supplies "
            "the same value via --source-trust-anchor-hash."
        ),
    )
    parser.add_argument(
        "--receipt-root",
        type=lambda value: _parse_hex32(value, label="receipt root"),
        help=(
            "Canonical non-zero receipt root bound into the "
            "sccp:tron:solid-block-message:v1 witness-seal transcript."
        ),
    )
    parser.add_argument(
        "--receipt-proof-hash",
        type=lambda value: _parse_hex32(value, label="receipt proof hash"),
        help=(
            "Canonical non-zero TRON receipt/message proof hash bound into the "
            "solid-block witness message."
        ),
    )
    parser.add_argument(
        "--source-inclusion-branch-hex",
        action="append",
        default=[],
        help=(
            "32-byte source message inclusion branch sibling for the "
            "sccp:tron:transaction-source-proof:v1 transcript. Repeat in "
            "leaf-to-root order. When supplied with --receipt-root, the live "
            "helper derives the receipt proof hash and compares "
            "--receipt-proof-hash when present."
        ),
    )
    parser.add_argument(
        "--witness-seal-signers-bitmap-hex",
        help=(
            "Little-endian witness bitmap for the active schedule. Required "
            "with --witness-seal-signature-hex to emit the canonical "
            "sccp:tron:witness-seal:v1 hash."
        ),
    )
    parser.add_argument(
        "--witness-seal-signature-hex",
        action="append",
        default=[],
        help=(
            "Canonical 65-byte recoverable secp256k1 witness signature over "
            "the solid-block message hash. Repeat once per selected bitmap bit "
            "in ascending schedule order."
        ),
    )
    parser.add_argument(
        "--expected-witness-seal-hash",
        type=lambda value: _parse_hex32(value, label="expected witness seal hash"),
        help=(
            "Optional expected sccp:tron:witness-seal:v1 hash for the supplied "
            "schedule, message inputs, bitmap, and signatures."
        ),
    )
    parser.add_argument(
        "--witness-schedule-transition-json",
        action="append",
        default=[],
        help=(
            "JSON object, or @file containing one, for a canonical "
            "sccp:tron:witness-schedule-transition seal. Repeat in chain "
            "order from source trust-anchor schedule to active schedule. Each "
            "object supplies from/to epochs, transition block number/hash, "
            "parent/next schedule payloads, signer bitmap, and signatures."
        ),
    )
    parser.add_argument(
        "--solid-block-ancestor-depth",
        type=int,
        default=0,
        help=(
            "Number of signed ancestor headers to fetch before the solid "
            f"block's parent (0..{TRON_MAX_SOLID_BLOCK_EXTRA_HEADERS}). "
            "Non-placeholder TRON material needs at least one."
        ),
    )
    parser.add_argument(
        "--solid-block-confirmation-depth",
        type=int,
        default=0,
        help=(
            "Number of signed confirmation headers to fetch after the solid "
            f"block (0..{TRON_MAX_SOLID_BLOCK_EXTRA_HEADERS}). The unique "
            "confirmation witness weight must exceed two thirds of the active "
            "schedule."
        ),
    )
    parser.add_argument(
        "--finality-policy-hash",
        type=lambda value: _parse_hex32(value, label="finality policy hash"),
        help="Governed non-zero TRON finality-policy hash for source record preflight.",
    )
    parser.add_argument(
        "--deployment-receipt-hash",
        type=lambda value: _parse_hex32(value, label="deployment receipt hash"),
        help="Governed non-zero source-adapter deployment receipt hash.",
    )
    parser.add_argument(
        "--adapter-verifier-vk-hash",
        type=lambda value: _parse_hex32(value, label="adapter verifier vk hash"),
        help=(
            "Optional TRON source-adapter OpenVerify vk hash. If supplied, it "
            "must match the canonical TRON -> SORA verifier profile."
        ),
    )
    parser.add_argument(
        "--expected-source-verifier-material-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected source verifier material hash",
        ),
        help=(
            "Expected canonical TRON source verifier material record hash. "
            "Optional for JSON dry-runs and required for live --full-toml."
        ),
    )
    parser.add_argument(
        "--expected-source-adapter-engine-deployment-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected source adapter engine deployment hash",
        ),
        help=(
            "Expected canonical TRON source-adapter deployment record hash. "
            "Optional for JSON dry-runs and required for live --full-toml."
        ),
    )
    parser.add_argument(
        "--expected-tron-dpos-source-gate-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected TRON DPoS source gate hash",
        ),
        help=(
            "Expected canonical TRON DPoS source gate hash. Optional for JSON "
            "dry-runs and required for live --full-toml."
        ),
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: _parse_hex32(value, label="route allowlist hash"),
        help=(
            "Governed route allowlist hash. Must match the canonical TRON "
            "source material, source adapter deployment, and destination "
            "binding tuple, and requires --expected-destination-binding-hash "
            "before offline full rollout TOML can be emitted."
        ),
    )
    parser.add_argument(
        "--route-canary-evidence-hash",
        type=lambda value: _parse_hex32(value, label="route canary evidence hash"),
        help="Post-deploy route canary evidence hash for all-lanes TOML metadata.",
    )
    parser.add_argument(
        "--route-canary-transaction-id",
        type=lambda value: _parse_hex32(value, label="route canary transaction id"),
        help=(
            "TRON transaction id for a successful destination verifier "
            "MessageProofAccepted canary. The helper reads the transaction log, "
            "checks it against the live verifier binding/backend/family/network "
            "views, and derives --route-canary-evidence-hash."
        ),
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected destination binding hash",
        ),
        help=(
            "Expected governed TRON destination binding hash. Required before "
            "live --full-toml can emit destination rollout TOML."
        ),
    )
    parser.add_argument(
        "--full-toml",
        action="store_true",
        help=(
            "Print verified full governance TOML instead of JSON. Requires "
            "complete source-record, destination-verifier, expected source "
            "config/source-record/DPoS-gate hashes, destination binding, route "
            "allowlist, and transaction-derived route canary evidence."
        ),
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=15.0,
        help="HTTP timeout in seconds.",
    )
    return parser


SENSITIVE_CLI_ERROR_MARKERS = (
    "secret-token",
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer ",
    "authorization",
    "access-key",
    "access_key",
    "api-key",
    "api_key",
    "client-secret",
    "client_secret",
    "session=",
    "token=",
)


def _cli_error_detail(exc: BaseException, *, fallback: str) -> str:
    if isinstance(exc, OSError):
        return fallback
    text = str(exc)
    if not text:
        return fallback
    lowered = text.lower()
    if any(marker in lowered for marker in SENSITIVE_CLI_ERROR_MARKERS):
        return fallback
    if any((ord(ch) < 0x20 and ch not in "\n\t") or ord(ch) == 0x7F for ch in text):
        return fallback
    return text


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.source_bridge_address is None and args.destination_verifier_address is None:
        parser.error(
            "at least one of --source-bridge-address or "
            "--destination-verifier-address is required"
        )
    try:
        summary = collect_live_evidence(args)
        if args.full_toml:
            sys.stdout.write(render_offline_full_toml(summary))
            return 0
    except (
        OSError,
        RuntimeError,
        TypeError,
        ValueError,
        argparse.ArgumentTypeError,
    ) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP TRON live evidence collection failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
