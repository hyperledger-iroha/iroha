#!/usr/bin/env python3
"""Verify Android Norito fixtures match the committed manifest and payload sources."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Set, TextIO, Union

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from norito_fixture_frame import (
    SIGNED_TRANSACTION_SCHEMA,
    TRANSACTION_PAYLOAD_SCHEMA,
    decode_canonical_norito_frame,
)

DEFAULT_RESOURCES_DIR = Path("java/iroha_android/src/test/resources")
DEFAULT_FIXTURES_PATH = DEFAULT_RESOURCES_DIR / "transaction_payloads.json"
DEFAULT_MANIFEST_PATH = DEFAULT_RESOURCES_DIR / "transaction_fixtures.manifest.json"
DEFAULT_STATE_PATH = Path("artifacts/android_fixture_regen_state.json")
MAX_TRANSACTION_NONCE = 0xFFFF_FFFF
MAX_U64 = 0xFFFF_FFFF_FFFF_FFFF
NETWORK_ID_LITERAL = re.compile(r"hash:([0-9A-F]{64})#([0-9A-F]{4})")

PAYLOAD_FIXTURE_FIELDS = frozenset(
    {
        "authority",
        "network_id",
        "creation_time_ms",
        "name",
        "nonce",
        "payload",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
        "time_to_live_ms",
    }
)
PAYLOAD_FIELDS = frozenset(
    {
        "admission_intent",
        "authority",
        "network_id",
        "creation_time_ms",
        "executable",
        "fee_payment",
        "metadata",
        "nonce",
        "time_to_live_ms",
    }
)
ADMISSION_INTENT_FIELDS = frozenset({"intent", "value"})
EXECUTABLE_VARIANTS = frozenset({"Batch", "ContractCall", "Instructions", "Ivm"})
INSTRUCTION_FIELDS = frozenset({"payload_base64", "wire_name"})
CONTRACT_CALL_FIELDS = frozenset(
    {"arguments", "contract_address", "entrypoint", "expected_code_hash"}
)
MANIFEST_FIELDS = frozenset({"fixtures"})
MANIFEST_FIXTURE_FIELDS = frozenset(
    {
        "authority",
        "network_id",
        "creation_time_ms",
        "encoded_file",
        "encoded_len",
        "name",
        "nonce",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
        "signed_len",
        "time_to_live_ms",
    }
)


class DuplicateJsonKeyError(ValueError):
    """Raised when native JSON contains two equivalent object keys."""


def _reject_duplicate_json_keys(pairs: list[tuple[str, object]]) -> dict:
    result: dict = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateJsonKeyError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def parse_json_strict(raw: str, context: str) -> object:
    """Decode native JSON while rejecting duplicate object keys."""
    try:
        return json.loads(raw, object_pairs_hook=_reject_duplicate_json_keys)
    except DuplicateJsonKeyError as exc:
        raise ValueError(f"invalid JSON in {context}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {context}: {exc}") from exc


def require_exact_fields(
    record: dict, expected: frozenset[str], context: str
) -> None:
    actual = set(record)
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    if missing or unexpected:
        raise ValueError(
            f"{context} has invalid fields: missing={missing}, unexpected={unexpected}"
        )


def validate_executable(executable: object, context: str) -> bool:
    if not isinstance(executable, dict):
        raise ValueError(f"{context} must be an object")
    variants = list(executable)
    if len(variants) != 1:
        raise ValueError(f"{context} must contain exactly one executable variant")
    variant = variants[0]
    if variant not in EXECUTABLE_VARIANTS:
        raise ValueError(f"{context} has unknown variant {variant!r}")
    body = executable[variant]
    if variant == "Ivm":
        if not isinstance(body, str):
            raise ValueError(f"{context}.Ivm must be a base64 string")
        decode_base64(body, f"{context}.Ivm")
        return True
    if variant == "Instructions":
        if not isinstance(body, list):
            raise ValueError(f"{context}.Instructions must be an array")
        for index, instruction in enumerate(body):
            validate_instruction(instruction, f"{context}.Instructions[{index}]")
        return False
    if variant == "ContractCall":
        validate_contract_call(body, f"{context}.ContractCall")
        return True
    if not isinstance(body, list):
        raise ValueError(f"{context}.Batch must be an array")
    if not body:
        raise ValueError(f"{context}.Batch must contain at least one item")
    requires_gas_limit = False
    for index, item in enumerate(body):
        item_context = f"{context}.Batch[{index}]"
        if not isinstance(item, dict):
            raise ValueError(f"{item_context} must be an object")
        item_variants = list(item)
        if len(item_variants) != 1:
            raise ValueError(f"{item_context} must contain exactly one variant")
        item_variant = item_variants[0]
        if item_variant == "Instruction":
            validate_instruction(item[item_variant], f"{item_context}.Instruction")
        elif item_variant == "ContractCall":
            validate_contract_call(item[item_variant], f"{item_context}.ContractCall")
            requires_gas_limit = True
        else:
            raise ValueError(f"{item_context} has unknown variant {item_variant!r}")
    return requires_gas_limit


def validate_instruction(instruction: object, context: str) -> None:
    if not isinstance(instruction, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(instruction, INSTRUCTION_FIELDS, context)
    wire_name = instruction["wire_name"]
    if not isinstance(wire_name, str) or not wire_name:
        raise ValueError(f"{context}.wire_name must be a non-empty string")
    payload_base64 = instruction["payload_base64"]
    if not isinstance(payload_base64, str):
        raise ValueError(f"{context}.payload_base64 must be a base64 string")
    if not decode_base64(payload_base64, f"{context}.payload_base64"):
        raise ValueError(f"{context}.payload_base64 must encode non-empty bytes")


def validate_contract_call(contract_call: object, context: str) -> None:
    if not isinstance(contract_call, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(contract_call, CONTRACT_CALL_FIELDS, context)
    for field in ("contract_address", "expected_code_hash", "entrypoint"):
        value = contract_call[field]
        if not isinstance(value, str) or not value:
            raise ValueError(f"{context}.{field} must be a non-empty string")
    arguments = contract_call["arguments"]
    if arguments is None:
        return
    if not isinstance(arguments, list) or any(
        not isinstance(byte, int)
        or isinstance(byte, bool)
        or byte < 0
        or byte > 0xFF
        for byte in arguments
    ):
        raise ValueError(f"{context}.arguments must be null or an array of bytes")


def validate_fee_payment(value: object, context: str) -> Optional[int]:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(value, frozenset({"payer", "value"}), context)
    payer = value["payer"]
    if payer != "authority":
        raise ValueError(f"{context}.payer must be exactly authority")
    fee_value = value["value"]
    if not isinstance(fee_value, dict):
        raise ValueError(f"{context}.value must be an object")
    require_exact_fields(
        fee_value,
        frozenset({"charge_limits", "gas_limit"}),
        f"{context}.value",
    )
    if not isinstance(fee_value["charge_limits"], list):
        raise ValueError(f"{context}.value.charge_limits must be an array")
    gas_limit = fee_value["gas_limit"]
    if gas_limit is not None and (
        isinstance(gas_limit, bool)
        or not isinstance(gas_limit, int)
        or gas_limit <= 0
        or gas_limit > MAX_U64
    ):
        raise ValueError(
            f"{context}.value.gas_limit must be null or an integer in 1..={MAX_U64}"
        )
    return gas_limit


def validate_admission_intent(value: object, context: str) -> None:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(value, ADMISSION_INTENT_FIELDS, context)
    if value["intent"] != "ordinary" or value["value"] is not None:
        raise ValueError(
            f"{context} must be exactly {{'intent': 'ordinary', 'value': null}}"
        )


def validate_payload_descriptor(entry: dict, name: str, path: Path) -> None:
    payload = entry.get("payload")
    if not isinstance(payload, dict):
        raise ValueError(f"fixture entry {name} in {path} missing payload object")
    require_exact_fields(payload, PAYLOAD_FIELDS, f"fixture entry {name} payload in {path}")
    validate_transaction_metadata(entry, f"fixture entry {name} in {path}")
    validate_transaction_metadata(payload, f"fixture entry {name} payload in {path}")
    requires_gas_limit = validate_executable(
        payload["executable"], f"fixture entry {name} executable"
    )
    gas_limit = validate_fee_payment(
        payload["fee_payment"], f"fixture entry {name} fee_payment"
    )
    validate_admission_intent(
        payload["admission_intent"], f"fixture entry {name} admission_intent"
    )
    if requires_gas_limit and gas_limit is None:
        raise ValueError(
            f"fixture entry {name} fee_payment.value.gas_limit must be positive "
            "for Ivm, ContractCall, or a Batch containing ContractCall"
        )
    if not isinstance(payload["metadata"], dict):
        raise ValueError(f"fixture entry {name} metadata must be an object")
    for field in (
        "authority",
        "network_id",
        "creation_time_ms",
        "nonce",
        "time_to_live_ms",
    ):
        if payload[field] != entry[field]:
            raise ValueError(
                f"fixture entry {name} in {path} has mismatched payload {field}"
            )


def validate_encoded_file(name: str, encoded_file: str, context: str) -> None:
    expected = f"{name}.norito"
    if encoded_file != expected:
        raise ValueError(f"{context} encoded_file must be exactly {expected!r}")
    if (
        not name
        or name in {".", ".."}
        or "/" in name
        or "\\" in name
        or Path(encoded_file).name != encoded_file
    ):
        raise ValueError(f"{context} encoded_file must not traverse directories")


def is_valid_transaction_ttl(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def is_valid_transaction_nonce(value: object) -> bool:
    return value is None or (
        isinstance(value, int)
        and not isinstance(value, bool)
        and 1 <= value <= MAX_TRANSACTION_NONCE
    )


def _crc16_ccitt_false(payload: bytes) -> int:
    crc = 0xFFFF
    for byte in payload:
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return crc


def validate_network_id(value: object, context: str) -> str:
    """Require the exact canonical hash literal used by ``NetworkId``."""
    if not isinstance(value, str):
        raise ValueError(f"{context} has invalid network_id")
    matched = NETWORK_ID_LITERAL.fullmatch(value)
    if matched is None:
        raise ValueError(f"{context} has invalid canonical network_id")
    body, checksum = matched.groups()
    expected_checksum = _crc16_ccitt_false(f"hash:{body}".encode("ascii"))
    if checksum != f"{expected_checksum:04X}":
        raise ValueError(f"{context} has invalid canonical network_id checksum")
    if bytes.fromhex(body)[-1] & 1 != 1:
        raise ValueError(f"{context} has unmarked network_id hash")
    return value


def validate_transaction_metadata(record: dict, context: str) -> None:
    network_id = record.get("network_id")
    authority = record.get("authority")
    creation_time_ms = record.get("creation_time_ms")
    validate_network_id(network_id, context)
    if not isinstance(authority, str) or not authority.strip():
        raise ValueError(f"{context} has invalid authority")
    if (
        not isinstance(creation_time_ms, int)
        or isinstance(creation_time_ms, bool)
        or creation_time_ms < 0
    ):
        raise ValueError(f"{context} has invalid creation_time_ms")
    if not is_valid_transaction_ttl(record.get("time_to_live_ms")):
        raise ValueError(f"{context} has invalid time_to_live_ms")
    if not is_valid_transaction_nonce(record.get("nonce")):
        raise ValueError(f"{context} has invalid nonce")


def decode_base64(value: str, context: str) -> bytes:
    try:
        decoded = base64.b64decode(value, validate=True)
    except Exception as exc:  # pragma: no cover - defensive conversion
        raise ValueError(f"invalid base64 for {context}: {exc}") from exc
    canonical = base64.b64encode(decoded).decode("ascii")
    if canonical != value:
        raise ValueError(f"non-canonical base64 for {context}")
    return decoded


def iroha_hash(data: bytes) -> str:
    digest = bytearray(hashlib.blake2b(data, digest_size=32).digest())
    digest[-1] |= 1
    return digest.hex()


def compact_length(value: int) -> bytes:
    if value < 0:
        raise ValueError("compact length must be non-negative")
    output = bytearray()
    remaining = value
    while True:
        byte = remaining & 0x7F
        remaining >>= 7
        if remaining:
            byte |= 0x80
        output.append(byte)
        if not remaining:
            return bytes(output)


def decode_compact_length(data: bytes, offset: int) -> tuple[int, int]:
    """Decode one canonical unsigned LEB128 field length."""
    start = offset
    value = 0
    shift = 0
    while offset < len(data) and shift <= 63:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            if data[start:offset] != compact_length(value):
                raise ValueError("non-canonical compact field length")
            return value, offset
        shift += 7
    raise ValueError("truncated or overflowing compact field length")


def read_norito_field(data: bytes, offset: int, context: str) -> tuple[bytes, int]:
    """Read one length-delimited field from an adaptive-Norito struct."""
    length, payload_offset = decode_compact_length(data, offset)
    end = payload_offset + length
    if end > len(data):
        raise ValueError(f"truncated {context}")
    return data[payload_offset:end], end


def signed_transaction_payload(data: bytes) -> bytes:
    """Extract the signed intent from the canonical first-release envelope."""
    _, offset = read_norito_field(data, 0, "SignedTransaction.signature")
    payload, offset = read_norito_field(data, offset, "SignedTransaction.payload")
    _, offset = read_norito_field(
        data, offset, "SignedTransaction.multisig_signatures"
    )
    if offset != len(data):
        raise ValueError("SignedTransaction has trailing or legacy envelope fields")
    return payload


def transaction_payload_network_id(data: bytes, context: str) -> bytes:
    """Read the exact ``TransactionDomain::Network`` identity from a payload."""
    domain, _ = read_norito_field(data, 0, f"{context}.domain")
    if len(domain) < 4:
        raise ValueError(f"{context} has a truncated transaction domain")
    tag = int.from_bytes(domain[:4], "little")
    if tag == 1:
        raise ValueError(f"{context} uses the genesis-only transaction domain")
    if tag != 0:
        raise ValueError(f"{context} has an unknown transaction domain tag {tag}")
    network_id, offset = read_norito_field(
        domain, 4, f"{context}.domain.network_id"
    )
    if offset != len(domain) or len(network_id) != 32:
        raise ValueError(f"{context} has a malformed transaction network_id")
    return network_id


def require_transaction_network_id(
    payload: bytes, network_id: str, context: str
) -> None:
    expected = bytes.fromhex(network_id[5:69])
    if transaction_payload_network_id(payload, context) != expected:
        raise ValueError(f"{context} network_id does not match its descriptor")


def signed_transaction_entrypoint_hash(data: bytes) -> str:
    payload = signed_transaction_payload(data)
    entrypoint = b"\x00\x00\x00\x00" + compact_length(len(payload)) + payload
    return iroha_hash(entrypoint)


def normalize_authority(value: str) -> str:
    if not isinstance(value, str):
        return value
    trimmed = value.strip()
    if not trimmed:
        return trimmed
    at_index = trimmed.rfind("@")
    if at_index > 0:
        return trimmed[:at_index]
    return trimmed


@dataclass(frozen=True)
class PayloadFixture:
    payload_base64: str
    payload_hash: str
    signed_base64: str
    signed_hash: str
    network_id: str
    authority: str
    creation_time_ms: int
    time_to_live_ms: int
    nonce: Optional[int]


def load_payload_fixtures(path: Path) -> Dict[str, PayloadFixture]:
    payloads = parse_json_strict(path.read_text(), str(path))

    if not isinstance(payloads, list):
        raise ValueError(f"fixtures JSON at {path} must be a list")

    mapping: Dict[str, PayloadFixture] = {}
    seen_names: Set[str] = set()
    seen_payloads: Set[bytes] = set()
    seen_payload_hashes: Set[str] = set()
    seen_signed_payloads: Set[bytes] = set()
    seen_signed_hashes: Set[str] = set()
    for entry in payloads:
        if not isinstance(entry, dict):
            raise ValueError(f"fixture entry in {path} is not an object")
        name = entry.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError(f"fixture entry in {path} missing name string: {entry!r}")
        if "encoded" in entry:
            raise ValueError(
                f"fixture entry {name} in {path} contains retired encoded alias"
            )
        if "time_to_live_ms" not in entry:
            raise ValueError(
                f"fixture entry {name} in {path} missing time_to_live_ms field"
            )
        if "nonce" not in entry:
            raise ValueError(f"fixture entry {name} in {path} missing nonce field")
        require_exact_fields(
            entry, PAYLOAD_FIXTURE_FIELDS, f"fixture entry {name} in {path}"
        )
        validate_payload_descriptor(entry, name, path)
        if name in seen_names:
            raise ValueError(f"duplicate fixture name {name!r} in {path}")
        seen_names.add(name)
        payload_base64 = entry.get("payload_base64")
        if not isinstance(payload_base64, str):
            raise ValueError(
                f"fixture entry {name} in {path} missing payload_base64 string"
            )
        payload_bytes = decode_base64(payload_base64, f"{name} payload")
        payload_bare = decode_canonical_norito_frame(
            payload_bytes,
            f"{name} payload",
            expected_schema=TRANSACTION_PAYLOAD_SCHEMA,
        )
        network_id = validate_network_id(
            entry.get("network_id"), f"fixture entry {name} in {path}"
        )
        require_transaction_network_id(
            payload_bare, network_id, f"fixture entry {name} payload in {path}"
        )
        if payload_bytes in seen_payloads:
            raise ValueError(f"duplicate fixture payload bytes for {name!r} in {path}")
        seen_payloads.add(payload_bytes)
        payload_hash = entry.get("payload_hash")
        signed_base64 = entry.get("signed_base64")
        signed_hash = entry.get("signed_hash")
        if not isinstance(payload_hash, str):
            raise ValueError(f"fixture entry {name} in {path} missing payload_hash string")
        if not isinstance(signed_base64, str):
            raise ValueError(f"fixture entry {name} in {path} missing signed_base64 string")
        if not isinstance(signed_hash, str):
            raise ValueError(f"fixture entry {name} in {path} missing signed_hash string")
        if payload_hash != iroha_hash(payload_bytes):
            raise ValueError(f"fixture entry {name} in {path} payload_hash mismatch")
        signed_bytes = decode_base64(signed_base64, f"{name} signed payload")
        signed_bare = decode_canonical_norito_frame(
            signed_bytes,
            f"{name} signed payload",
            expected_schema=SIGNED_TRANSACTION_SCHEMA,
        )
        if signed_hash != signed_transaction_entrypoint_hash(signed_bare):
            raise ValueError(f"fixture entry {name} in {path} signed_hash mismatch")
        embedded_payload = signed_transaction_payload(signed_bare)
        if embedded_payload != payload_bare:
            raise ValueError(
                f"fixture entry {name} in {path} signed payload does not match payload_base64"
            )
        require_transaction_network_id(
            embedded_payload,
            network_id,
            f"fixture entry {name} signed payload in {path}",
        )
        if payload_hash in seen_payload_hashes:
            raise ValueError(f"duplicate fixture payload_hash {payload_hash!r} in {path}")
        seen_payload_hashes.add(payload_hash)
        if signed_bytes in seen_signed_payloads:
            raise ValueError(f"duplicate fixture signed bytes for {name!r} in {path}")
        seen_signed_payloads.add(signed_bytes)
        if signed_hash in seen_signed_hashes:
            raise ValueError(f"duplicate fixture signed_hash {signed_hash!r} in {path}")
        seen_signed_hashes.add(signed_hash)
        authority = entry.get("authority")
        creation_time_ms = entry.get("creation_time_ms")
        if "time_to_live_ms" not in entry:
            raise ValueError(
                f"fixture entry {name} in {path} missing time_to_live_ms field"
            )
        if "nonce" not in entry:
            raise ValueError(f"fixture entry {name} in {path} missing nonce field")
        time_to_live_ms = entry.get("time_to_live_ms")
        nonce = entry.get("nonce")
        if not isinstance(authority, str) or not authority.strip():
            raise ValueError(f"fixture entry {name} in {path} missing authority string")
        if not isinstance(creation_time_ms, int) or isinstance(creation_time_ms, bool):
            raise ValueError(
                f"fixture entry {name} in {path} missing creation_time_ms integer"
            )
        if not is_valid_transaction_ttl(time_to_live_ms):
            raise ValueError(
                f"fixture entry {name} in {path} has invalid time_to_live_ms"
            )
        if not is_valid_transaction_nonce(nonce):
            raise ValueError(f"fixture entry {name} in {path} has invalid nonce")
        mapping[name] = PayloadFixture(
            payload_base64=payload_base64,
            payload_hash=payload_hash,
            signed_base64=signed_base64,
            signed_hash=signed_hash,
            network_id=network_id,
            authority=authority,
            creation_time_ms=creation_time_ms,
            time_to_live_ms=time_to_live_ms,
            nonce=nonce,
        )
    return mapping


def load_manifest(path: Path) -> Dict[str, object]:
    payload = parse_json_strict(path.read_text(), str(path))
    if not isinstance(payload, dict):
        raise ValueError(f"manifest at {path} must be an object")
    require_exact_fields(payload, MANIFEST_FIELDS, f"manifest at {path}")
    return payload


def compare(
    resources_dir: Path,
    manifest: Dict[str, object],
    payload_map: Dict[str, PayloadFixture],
) -> List[str]:
    errors: List[str] = []

    try:
        require_exact_fields(manifest, MANIFEST_FIELDS, "manifest")
    except ValueError as exc:
        errors.append(str(exc))
        return errors

    fixtures = manifest.get("fixtures")
    if not isinstance(fixtures, list):
        errors.append("manifest missing 'fixtures' array")
        return errors

    seen_names: Set[str] = set()
    seen_files: Set[str] = set()
    seen_payload_hashes: Set[str] = set()
    seen_payload_bytes: Set[bytes] = set()
    seen_signed_hashes: Set[str] = set()
    seen_signed_bytes: Set[bytes] = set()

    for entry in fixtures:
        if not isinstance(entry, dict):
            errors.append(f"manifest fixture entry is not an object: {entry!r}")
            continue

        if "time_to_live_ms" not in entry:
            errors.append(f"manifest fixture missing time_to_live_ms field: {entry}")
            continue
        if "nonce" not in entry:
            errors.append(f"manifest fixture missing nonce field: {entry}")
            continue

        try:
            require_exact_fields(entry, MANIFEST_FIXTURE_FIELDS, "manifest fixture")
        except ValueError as exc:
            errors.append(str(exc))
            continue

        name = entry.get("name")
        encoded_file = entry.get("encoded_file")
        payload_base64 = entry.get("payload_base64")
        payload_hash = entry.get("payload_hash")
        encoded_len = entry.get("encoded_len")
        signed_base64 = entry.get("signed_base64")
        signed_hash = entry.get("signed_hash")
        signed_len = entry.get("signed_len")
        network_id = entry.get("network_id")
        authority = entry.get("authority")
        creation_time_ms = entry.get("creation_time_ms")
        if "time_to_live_ms" not in entry:
            errors.append(f"manifest fixture missing time_to_live_ms field: {entry}")
            continue
        if "nonce" not in entry:
            errors.append(f"manifest fixture missing nonce field: {entry}")
            continue
        time_to_live_ms = entry.get("time_to_live_ms")
        nonce = entry.get("nonce")

        if not all(
            isinstance(v, str)
            for v in (
                name,
                encoded_file,
                payload_base64,
                payload_hash,
                signed_base64,
                signed_hash,
                network_id,
                authority,
            )
        ):
            errors.append(f"manifest fixture missing required string fields: {entry}")
            continue
        try:
            network_id = validate_network_id(network_id, f"manifest fixture {name}")
        except ValueError as exc:
            errors.append(str(exc))
            continue
        if (
            not isinstance(encoded_len, int)
            or isinstance(encoded_len, bool)
            or encoded_len < 0
            or not isinstance(signed_len, int)
            or isinstance(signed_len, bool)
            or signed_len < 0
        ):
            errors.append(f"manifest fixture missing encoded_len/signed_len integers: {entry}")
            continue
        if not isinstance(creation_time_ms, int) or isinstance(creation_time_ms, bool):
            errors.append(f"manifest fixture missing creation_time_ms integer: {entry}")
            continue
        if not is_valid_transaction_ttl(time_to_live_ms):
            errors.append(f"manifest fixture has invalid time_to_live_ms: {entry}")
            continue
        if not is_valid_transaction_nonce(nonce):
            errors.append(f"manifest fixture has invalid nonce: {entry}")
            continue
        if not isinstance(name, str) or not isinstance(encoded_file, str):
            errors.append(f"manifest fixture missing name or encoded_file string: {entry}")
            continue
        try:
            validate_encoded_file(name, encoded_file, f"manifest fixture {name}")
        except ValueError as exc:
            errors.append(str(exc))
            continue

        if name in seen_names:
            errors.append(f"manifest contains duplicate fixture name: {name}")
        else:
            seen_names.add(name)
        if encoded_file in seen_files:
            errors.append(f"manifest contains duplicate encoded_file: {encoded_file}")
        else:
            seen_files.add(encoded_file)
        if payload_hash in seen_payload_hashes:
            errors.append(f"manifest contains duplicate payload_hash: {payload_hash}")
        else:
            seen_payload_hashes.add(payload_hash)
        payload_identity = decode_base64(payload_base64, f"{name} payload")
        try:
            payload_bare = decode_canonical_norito_frame(
                payload_identity,
                f"manifest fixture {name} payload",
                expected_schema=TRANSACTION_PAYLOAD_SCHEMA,
            )
        except ValueError as exc:
            errors.append(str(exc))
            payload_bare = b""
        try:
            require_transaction_network_id(
                payload_bare, network_id, f"manifest fixture {name} payload"
            )
        except ValueError as exc:
            errors.append(str(exc))
        if payload_identity in seen_payload_bytes:
            errors.append(f"manifest contains duplicate payload bytes: {name}")
        else:
            seen_payload_bytes.add(payload_identity)
        if signed_hash in seen_signed_hashes:
            errors.append(f"manifest contains duplicate signed_hash: {signed_hash}")
        else:
            seen_signed_hashes.add(signed_hash)
        signed_identity = decode_base64(signed_base64, f"{name} signed")
        try:
            signed_bare = decode_canonical_norito_frame(
                signed_identity,
                f"manifest fixture {name} signed",
                expected_schema=SIGNED_TRANSACTION_SCHEMA,
            )
        except ValueError as exc:
            errors.append(str(exc))
            signed_bare = b""
        try:
            embedded_payload = signed_transaction_payload(signed_bare)
            if embedded_payload != payload_bare:
                errors.append(
                    f"manifest signed payload does not match payload_base64 for {name}"
                )
            require_transaction_network_id(
                embedded_payload,
                network_id,
                f"manifest fixture {name} signed payload",
            )
        except ValueError as exc:
            errors.append(str(exc))
        if signed_identity in seen_signed_bytes:
            errors.append(f"manifest contains duplicate signed bytes: {name}")
        else:
            seen_signed_bytes.add(signed_identity)

        expected_payload_bytes = payload_identity
        expected_payload_hash = iroha_hash(expected_payload_bytes)
        if expected_payload_hash != payload_hash:
            errors.append(
                f"manifest payload_hash mismatch for {name}: manifest={payload_hash} computed={expected_payload_hash}"
            )

        payload_entry = payload_map.get(name)
        if payload_entry is None:
            errors.append(f"fixtures JSON missing entry for {name}")
        else:
            if payload_entry.payload_base64 != payload_base64:
                errors.append(
                    f"payload JSON for {name} does not match manifest payload_base64"
                )
            if payload_entry.payload_hash != payload_hash:
                errors.append(f"payload JSON payload_hash mismatch for {name}")
            if payload_entry.signed_base64 != signed_base64:
                errors.append(f"payload JSON signed_base64 mismatch for {name}")
            if payload_entry.signed_hash != signed_hash:
                errors.append(f"payload JSON signed_hash mismatch for {name}")
            if payload_entry.network_id != network_id:
                errors.append(
                    f"payload JSON network_id mismatch for {name}: "
                    f"payloads={payload_entry.network_id} manifest={network_id}"
                )
            normalized_payload_authority = normalize_authority(payload_entry.authority)
            normalized_manifest_authority = normalize_authority(authority)
            if normalized_payload_authority != normalized_manifest_authority:
                errors.append(
                    f"payload JSON authority mismatch for {name}: "
                    f"payloads={payload_entry.authority} manifest={authority}"
                )
            if payload_entry.creation_time_ms != creation_time_ms:
                errors.append(
                    f"payload JSON creation_time_ms mismatch for {name}: "
                    f"payloads={payload_entry.creation_time_ms} manifest={creation_time_ms}"
                )
            if payload_entry.time_to_live_ms != time_to_live_ms:
                errors.append(
                    f"payload JSON time_to_live_ms mismatch for {name}: "
                    f"payloads={payload_entry.time_to_live_ms} manifest={time_to_live_ms}"
                )
            if payload_entry.nonce != nonce:
                errors.append(
                    f"payload JSON nonce mismatch for {name}: "
                    f"payloads={payload_entry.nonce} manifest={nonce}"
                )

        fixture_path = resources_dir / encoded_file
        if not fixture_path.exists():
            errors.append(f"fixture file missing: {fixture_path}")
        else:
            actual_bytes = fixture_path.read_bytes()
            if actual_bytes != expected_payload_bytes:
                errors.append(f"fixture file differs from manifest payload for {encoded_file}")
            if len(actual_bytes) != encoded_len:
                errors.append(
                    f"fixture length mismatch for {encoded_file}: manifest={encoded_len} actual={len(actual_bytes)}"
                )
            actual_hash = iroha_hash(actual_bytes)
            if actual_hash != payload_hash:
                errors.append(
                    f"fixture hash mismatch for {encoded_file}: manifest={payload_hash} actual={actual_hash}"
                )

        signed_bytes = signed_identity
        if len(signed_bytes) != signed_len:
            errors.append(
                f"signed transaction length mismatch for {name}: manifest={signed_len} actual={len(signed_bytes)}"
            )
        if signed_bare:
            signed_digest = signed_transaction_entrypoint_hash(signed_bare)
            if signed_digest != signed_hash:
                errors.append(
                    f"signed transaction hash mismatch for {name}: manifest={signed_hash} actual={signed_digest}"
                )

    extra_payloads = sorted(set(payload_map) - seen_names)
    for name in extra_payloads:
        errors.append(f"fixtures JSON entry without manifest counterpart: {name}")

    for extra in sorted(p.name for p in resources_dir.glob("*.norito") if p.name not in seen_files):
        errors.append(f"unexpected fixture file present: {extra}")

    return errors


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check Android Norito fixtures against the committed manifest and payload definitions",
    )
    parser.add_argument(
        "--resources",
        type=Path,
        default=DEFAULT_RESOURCES_DIR,
        help=f"Directory containing Android fixture artifacts (default: {DEFAULT_RESOURCES_DIR})",
    )
    parser.add_argument(
        "--fixtures",
        type=Path,
        default=DEFAULT_FIXTURES_PATH,
        help=f"Path to transaction_payloads.json (default: {DEFAULT_FIXTURES_PATH})",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST_PATH,
        help=f"Path to transaction_fixtures.manifest.json (default: {DEFAULT_MANIFEST_PATH})",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress success output.",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write a parity summary JSON file (for dashboards/gates).",
    )
    parser.add_argument(
        "--state",
        type=Path,
        default=DEFAULT_STATE_PATH,
        help=f"Path to the Android fixture cadence state file (default: {DEFAULT_STATE_PATH})",
    )
    parser.add_argument(
        "--pipeline-metadata",
        type=str,
        help="Optional JSON file describing pipeline/test metadata (use '-' to read from stdin).",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def load_state_metadata(path: Path) -> Optional[dict]:
    if not path:
        return None
    try:
        payload = parse_json_strict(path.read_text(), f"state file {path}")
    except FileNotFoundError:
        return None
    if not isinstance(payload, dict):
        raise ValueError(f"state file {path} must contain a JSON object")
    return payload


def load_pipeline_metadata(
    source: Optional[Union[Path, str]],
    *,
    stdin: TextIO | None = None,
) -> Optional[dict]:
    if source is None:
        return None
    if isinstance(source, Path):
        try:
            raw = source.read_text(encoding="utf-8")
        except FileNotFoundError:
            raise ValueError(f"pipeline metadata file not found: {source}") from None
        except OSError as exc:  # pragma: no cover - defensive guard
            raise ValueError(f"failed to read pipeline metadata file {source}: {exc}") from exc
        source_label: Union[str, Path] = source
    else:
        if source != "-":
            raise ValueError("pipeline metadata source must be a path or '-' for stdin")
        stream = stdin if stdin is not None else sys.stdin
        raw = stream.read()
        if not raw.strip():
            raise ValueError("pipeline metadata from stdin was empty")
        source_label = "<stdin>"
    location = source_label if isinstance(source_label, str) else str(source_label)
    payload = parse_json_strict(raw, f"pipeline metadata file {location}")
    if not isinstance(payload, dict):
        raise ValueError(f"pipeline metadata in {location} must be a JSON object")
    return payload


def build_summary_payload(
    *,
    resources_dir: Path,
    fixtures_path: Path,
    manifest_path: Path,
    state: Optional[dict],
    errors: Sequence[str],
    pipeline_metadata: Optional[dict],
    pipeline_source: Optional[str],
    manifest: Optional[dict],
    payload_map: Dict[str, PayloadFixture],
) -> dict:
    timestamp = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    payload: Dict[str, object] = {
        "generated_at": timestamp,
        "resources_dir": str(resources_dir),
        "fixtures_path": str(fixtures_path),
        "manifest_path": str(manifest_path),
        "result": {
            "status": "ok" if not errors else "error",
            "error_count": len(errors),
        },
    }
    if errors:
        payload["result"]["errors"] = list(errors)
    payload["artifacts"] = build_artifact_metadata(
        resources_dir=resources_dir,
        fixtures_path=fixtures_path,
        manifest_path=manifest_path,
        manifest=manifest,
        payload_map=payload_map,
    )
    if state is not None:
        payload["state"] = state
    if pipeline_metadata is not None:
        pipeline_block: Dict[str, object] = {"metadata": pipeline_metadata}
        if pipeline_source is not None:
            pipeline_block["source_path"] = pipeline_source
        payload["pipeline"] = pipeline_block
    return payload


def write_summary(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def hash_encoded_directory(resources_dir: Path) -> str:
    """Compute a deterministic hash across encoded fixture files (.norito)."""
    digest = hashlib.sha256()
    for fixture_path in sorted(resources_dir.glob("*.norito")):
        digest.update(fixture_path.name.encode("utf-8"))
        digest.update(b"\0")
        digest.update(fixture_path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def build_artifact_metadata(
    *,
    resources_dir: Path,
    fixtures_path: Path,
    manifest_path: Path,
    manifest: Optional[dict],
    payload_map: Dict[str, PayloadFixture],
) -> Dict[str, object]:
    encoded_files = sorted(resources_dir.glob("*.norito"))
    manifest_fixtures = manifest.get("fixtures") if isinstance(manifest, dict) else None
    return {
        "manifest": {
          "path": str(manifest_path),
          "sha256": sha256_file(manifest_path),
          "fixture_count": len(manifest_fixtures) if isinstance(manifest_fixtures, list) else 0,
        },
        "payloads": {
            "path": str(fixtures_path),
            "sha256": sha256_file(fixtures_path),
            "entry_count": len(payload_map),
        },
        "encoded": {
          "dir": str(resources_dir),
          "file_count": len(encoded_files),
          "aggregate_sha256": hash_encoded_directory(resources_dir),
        },
    }


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)

    resources_dir = args.resources.resolve()
    fixtures_path = args.fixtures.resolve()
    manifest_path = args.manifest.resolve()
    summary_path: Optional[Path] = args.json_out.resolve() if args.json_out else None
    pipeline_arg = args.pipeline_metadata
    pipeline_source_label: Optional[str] = None

    try:
        state_metadata = load_state_metadata(args.state.resolve() if args.state else DEFAULT_STATE_PATH)
        pipeline_metadata = None
        if pipeline_arg:
            if pipeline_arg == "-":
                pipeline_source_label = "<stdin>"
                pipeline_metadata = load_pipeline_metadata("-", stdin=sys.stdin)
            else:
                pipeline_path = Path(pipeline_arg).resolve()
                pipeline_source_label = str(pipeline_path)
                pipeline_metadata = load_pipeline_metadata(pipeline_path)
    except ValueError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    missing_paths = [
        (resources_dir.exists(), f"[error] missing resources directory: {resources_dir}"),
        (fixtures_path.exists(), f"[error] missing fixtures JSON: {fixtures_path}"),
        (manifest_path.exists(), f"[error] missing manifest JSON: {manifest_path}"),
    ]
    has_missing = False
    for ok, message in missing_paths:
        if not ok:
            has_missing = True
            print(message, file=sys.stderr)
    if has_missing:
        return 1

    try:
        payload_map = load_payload_fixtures(fixtures_path)
        manifest = load_manifest(manifest_path)
    except ValueError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    errors = compare(resources_dir, manifest, payload_map)
    exit_code = 0
    if errors:
        for message in errors:
            print(f"[error] {message}", file=sys.stderr)
        exit_code = 1
    else:
        if not args.quiet:
            print(f"[ok] Android fixtures match manifest and payload JSON ({resources_dir})")

    if summary_path is not None:
        try:
            summary = build_summary_payload(
                resources_dir=resources_dir,
                fixtures_path=fixtures_path,
                manifest_path=manifest_path,
                state=state_metadata,
                errors=errors,
                pipeline_metadata=pipeline_metadata,
                pipeline_source=pipeline_source_label,
                manifest=manifest,
                payload_map=payload_map,
            )
            write_summary(summary_path, summary)
            if not args.quiet:
                print(f"[ok] wrote parity summary to {summary_path}")
        except Exception as exc:  # pragma: no cover - defensive guard
            print(f"[error] failed to write parity summary: {exc}", file=sys.stderr)
            exit_code = 1

    return exit_code


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
