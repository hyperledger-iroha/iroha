#!/usr/bin/env python3
"""Verify Swift Norito fixture parity and cadence metadata."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from norito_fixture_frame import (
    SIGNED_TRANSACTION_SCHEMA,
    TRANSACTION_PAYLOAD_SCHEMA,
    decode_canonical_norito_frame,
)

DEFAULT_SOURCE = Path("fixtures/norito_rpc")
DEFAULT_TARGET = Path("IrohaSwift/Fixtures")
DEFAULT_STATE = Path("artifacts/swift_fixture_regen_state.json")
DEFAULT_ODD_OWNER = "android-foundations"
DEFAULT_EVEN_OWNER = "swift-lead"
DEFAULT_CADENCE_LABEL = "weekly-wed-1700utc"
MANAGED_FIXTURES = (
    Path("transaction_payloads.json"),
    Path("transaction_fixtures.manifest.json"),
)
SWIFT_PAYLOADS = Path("swift_parity_payloads.json")
SWIFT_MANIFEST = Path("swift_parity_manifest.json")
EXPECTED_SWIFT_FIXTURES = frozenset(
    {
        "swift_transfer_asset_basic",
        "swift_mint_asset_basic",
        "swift_burn_asset_basic",
    }
)
CANONICAL_DEV_NETWORK_ID = (
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
)
SWIFT_INSTRUCTION_SEMANTICS = {
    "swift_burn_asset_basic": ("Burn", "BurnAsset"),
    "swift_mint_asset_basic": ("Mint", "MintAsset"),
    "swift_transfer_asset_basic": ("Transfer", "TransferAsset"),
}
MAX_FIXTURE_BYTES = 16 * 1024 * 1024
MAX_TRANSACTION_NONCE = 0xFFFF_FFFF

SHARED_PAYLOAD_ENTRY_FIELDS = frozenset(
    {
        "authority",
        "creation_time_ms",
        "name",
        "network_id",
        "nonce",
        "payload",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
        "time_to_live_ms",
    }
)
SWIFT_PAYLOAD_ENTRY_FIELDS = frozenset({"name", "payload"})
SHARED_PAYLOAD_FIELDS = frozenset(
    {
        "admission_intent",
        "authority",
        "creation_time_ms",
        "executable",
        "fee_payment",
        "metadata",
        "network_id",
        "nonce",
        "time_to_live_ms",
    }
)
SWIFT_PAYLOAD_FIELDS = frozenset(
    {
        "admission_intent",
        "authority",
        "creation_time_ms",
        "executable",
        "fee_payment",
        "metadata",
        "network_id",
        "nonce",
        "time_to_live_ms",
    }
)
MANIFEST_ENTRY_FIELDS = frozenset(
    {
        "authority",
        "creation_time_ms",
        "encoded_file",
        "encoded_len",
        "name",
        "network_id",
        "nonce",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
        "signed_len",
        "time_to_live_ms",
    }
)
SHARED_MANIFEST_FIELDS = frozenset({"fixtures"})
SWIFT_MANIFEST_FIELDS = frozenset({"fixtures"})
SWIFT_MANIFEST_ENTRY_FIELDS = frozenset(
    {"name", "payload_base64", "payload_hash", "signed_base64", "signed_hash"}
)


class DuplicateJsonKeyError(ValueError):
    """Raised before a last-wins native JSON decoder can hide duplicate keys."""


def _reject_duplicate_json_keys(pairs: list[tuple[str, object]]) -> dict:
    result: dict = {}
    for key, value in pairs:
        if key in result:
            raise DuplicateJsonKeyError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def parse_json_strict(path: Path) -> object:
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except (DuplicateJsonKeyError, json.JSONDecodeError) as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc


def require_exact_fields(record: dict, expected: frozenset[str], context: str) -> None:
    actual = set(record)
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    if missing or unexpected:
        raise ValueError(
            f"{context} has invalid fields: missing={missing}, unexpected={unexpected}"
        )


def require_nonempty_string(value: object, context: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context} must be a non-empty string")
    return value


def require_network_id(value: object, context: str) -> str:
    if value != CANONICAL_DEV_NETWORK_ID:
        raise ValueError(
            f"{context} must be exactly the canonical Iroha3 dev network identity "
            f"{CANONICAL_DEV_NETWORK_ID!r}"
        )
    return CANONICAL_DEV_NETWORK_ID


def require_uint(value: object, context: str, *, minimum: int = 0, maximum: int) -> int:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < minimum
        or value > maximum
    ):
        raise ValueError(
            f"{context} must be an integer in the range {minimum}...{maximum}"
        )
    return value


def validate_nonce(value: object, context: str) -> Optional[int]:
    if value is None:
        return None
    return require_uint(value, context, minimum=1, maximum=MAX_TRANSACTION_NONCE)


def decode_canonical_base64(value: object, context: str) -> bytes:
    if not isinstance(value, str):
        raise ValueError(f"{context} must be a base64 string")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, base64.binascii.Error) as exc:
        raise ValueError(f"{context} is invalid base64: {exc}") from exc
    if base64.b64encode(decoded).decode("ascii") != value:
        raise ValueError(f"{context} is not canonical base64")
    return decoded


def require_lower_hex(value: object, length: int, context: str) -> str:
    text = require_nonempty_string(value, context)
    if len(text) != length or any(ch not in "0123456789abcdef" for ch in text):
        raise ValueError(f"{context} must be exactly {length} lowercase hexadecimal digits")
    return text


def iroha_hash(data: bytes) -> str:
    digest = bytearray(hashlib.blake2b(data, digest_size=32).digest())
    digest[-1] |= 1
    return digest.hex()


def compact_length(value: int) -> bytes:
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
    length, payload_offset = decode_compact_length(data, offset)
    end = payload_offset + length
    if end > len(data):
        raise ValueError(f"truncated {context}")
    return data[payload_offset:end], end


def signed_transaction_payload(data: bytes) -> bytes:
    _, offset = read_norito_field(data, 0, "SignedTransaction.signature")
    payload, offset = read_norito_field(data, offset, "SignedTransaction.payload")
    _, offset = read_norito_field(
        data, offset, "SignedTransaction.multisig_signatures"
    )
    if offset != len(data):
        raise ValueError("SignedTransaction has trailing or legacy envelope fields")
    return payload


def signed_transaction_entrypoint_hash(data: bytes) -> str:
    payload = signed_transaction_payload(data)
    preimage = b"\x00\x00\x00\x00" + compact_length(len(payload)) + payload
    return iroha_hash(preimage)


def validate_encoded_file(name: str, encoded_file: object, context: str) -> str:
    file_name = require_nonempty_string(encoded_file, f"{context}.encoded_file")
    expected = f"{name}.norito"
    if file_name != expected:
        raise ValueError(f"{context}.encoded_file must be exactly {expected!r}")
    if (
        not name
        or name in {".", ".."}
        or "/" in name
        or "\\" in name
        or Path(file_name).name != file_name
    ):
        raise ValueError(f"{context}.encoded_file must not traverse directories")
    return file_name


def validate_contract_call(value: object, context: str) -> None:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(
        value,
        frozenset(
            {"arguments", "contract_address", "entrypoint", "expected_code_hash"}
        ),
        context,
    )
    for field in ("contract_address", "entrypoint", "expected_code_hash"):
        require_nonempty_string(value[field], f"{context}.{field}")
    arguments = value["arguments"]
    if arguments is not None:
        if not isinstance(arguments, list):
            raise ValueError(f"{context}.arguments must be null or a byte array")
        for index, byte in enumerate(arguments):
            require_uint(byte, f"{context}.arguments[{index}]", maximum=255)


def validate_instruction(value: object, context: str, *, shared: bool) -> None:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    if shared:
        require_exact_fields(
            value, frozenset({"payload_base64", "wire_name"}), context
        )
        decode_canonical_base64(value["payload_base64"], f"{context}.payload_base64")
        require_nonempty_string(value["wire_name"], f"{context}.wire_name")
        return
    require_exact_fields(value, frozenset({"arguments", "kind"}), context)
    require_nonempty_string(value["kind"], f"{context}.kind")
    arguments = value["arguments"]
    if not isinstance(arguments, dict):
        raise ValueError(f"{context}.arguments must be an object")
    require_exact_fields(
        arguments,
        frozenset({"action", "asset_definition_id", "destination", "quantity"}),
        f"{context}.arguments",
    )
    for key, argument in arguments.items():
        require_nonempty_string(key, f"{context}.arguments key")
        if not isinstance(argument, str):
            raise ValueError(f"{context}.arguments[{key!r}] must be a string")


def validate_executable(value: object, context: str, *, shared: bool) -> bool:
    if not isinstance(value, dict) or len(value) != 1:
        raise ValueError(f"{context} must contain exactly one executable variant")
    variant, body = next(iter(value.items()))
    if not shared and variant != "Instructions":
        raise ValueError(f"{context} Swift parity fixtures require Instructions")
    if variant == "Ivm":
        if not decode_canonical_base64(body, f"{context}.Ivm"):
            raise ValueError(f"{context}.Ivm must not be empty")
        return True
    elif variant == "Instructions":
        if not isinstance(body, list):
            raise ValueError(f"{context}.Instructions must be an array")
        if not shared and len(body) != 1:
            raise ValueError(
                f"{context}.Instructions must contain exactly one instruction"
            )
        for index, instruction in enumerate(body):
            validate_instruction(
                instruction, f"{context}.Instructions[{index}]", shared=shared
            )
        return False
    elif variant == "ContractCall":
        validate_contract_call(body, f"{context}.ContractCall")
        return True
    elif variant == "Batch":
        if not isinstance(body, list):
            raise ValueError(f"{context}.Batch must be an array")
        requires_gas_limit = False
        for index, item in enumerate(body):
            item_context = f"{context}.Batch[{index}]"
            if not isinstance(item, dict) or len(item) != 1:
                raise ValueError(
                    f"{item_context} must contain exactly one executable variant"
                )
            item_variant, item_body = next(iter(item.items()))
            if item_variant == "Instruction":
                validate_instruction(item_body, f"{item_context}.Instruction", shared=shared)
            elif item_variant == "ContractCall":
                validate_contract_call(item_body, f"{item_context}.ContractCall")
                requires_gas_limit = True
            else:
                raise ValueError(f"{item_context} has unknown variant {item_variant!r}")
        return requires_gas_limit
    else:
        raise ValueError(f"{context} has unknown variant {variant!r}")


def validate_fee_payment(
    value: object, context: str, *, shared: bool
) -> Optional[int]:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(value, frozenset({"payer", "value"}), context)
    payer = value["payer"]
    if payer != "authority":
        raise ValueError(f"{context}.payer must be exactly 'authority'")
    fee_value = value["value"]
    if not isinstance(fee_value, dict):
        raise ValueError(f"{context}.value must be an object")
    require_exact_fields(
        fee_value,
        frozenset({"charge_limits", "gas_limit"}),
        f"{context}.value",
    )
    limits = fee_value["charge_limits"]
    if not isinstance(limits, list):
        raise ValueError(f"{context}.value.charge_limits must be an array")
    if not shared and limits:
        raise ValueError(
            f"{context}.value.charge_limits must be exactly the empty array"
        )
    for index, limit in enumerate(limits):
        limit_context = f"{context}.value.charge_limits[{index}]"
        if not isinstance(limit, dict):
            raise ValueError(f"{limit_context} must be an object")
        require_exact_fields(
            limit,
            frozenset({"asset_definition_id", "kind", "max_amount"}),
            limit_context,
        )
        require_nonempty_string(
            limit["asset_definition_id"], f"{limit_context}.asset_definition_id"
        )
        require_nonempty_string(limit["max_amount"], f"{limit_context}.max_amount")
        kind = limit["kind"]
        if not isinstance(kind, dict):
            raise ValueError(f"{limit_context}.kind must be an object")
        require_exact_fields(kind, frozenset({"kind", "value"}), f"{limit_context}.kind")
        require_nonempty_string(kind["kind"], f"{limit_context}.kind.kind")
    gas_limit = fee_value["gas_limit"]
    if gas_limit is not None:
        require_uint(
            gas_limit,
            f"{context}.value.gas_limit",
            minimum=1,
            maximum=2**64 - 1,
        )
    if not shared and gas_limit is not None:
        raise ValueError(f"{context}.value.gas_limit must be exactly null")
    return gas_limit


def validate_admission_intent(
    value: object, context: str, *, shared: bool
) -> None:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(value, frozenset({"intent", "value"}), context)
    expected = "ordinary" if shared else "queue_plan_synced"
    if value["intent"] != expected or value["value"] is not None:
        raise ValueError(
            f"{context} must be exactly {{'intent': '{expected}', 'value': null}}"
        )


@dataclass(frozen=True)
class PayloadRecord:
    name: str
    authority: str
    network_id: str
    creation_time_ms: int
    time_to_live_ms: int
    nonce: Optional[int]
    payload_base64: Optional[str] = None
    payload_hash: Optional[str] = None
    signed_base64: Optional[str] = None
    signed_hash: Optional[str] = None


@dataclass(frozen=True)
class ManifestRecord:
    name: str
    authority: str
    network_id: str
    creation_time_ms: int
    time_to_live_ms: int
    nonce: Optional[int]
    encoded_file: str
    encoded_len: int
    signed_len: int
    payload_base64: str
    payload_hash: str
    signed_base64: str
    signed_hash: str
    payload_bytes: bytes
    signed_bytes: bytes


def validate_payload_body(value: object, context: str, *, shared: bool) -> PayloadRecord:
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    require_exact_fields(
        value, SHARED_PAYLOAD_FIELDS if shared else SWIFT_PAYLOAD_FIELDS, context
    )
    authority = require_nonempty_string(value["authority"], f"{context}.authority")
    network_id = require_network_id(value["network_id"], f"{context}.network_id")
    creation_time_ms = require_uint(
        value["creation_time_ms"], f"{context}.creation_time_ms", maximum=2**64 - 1
    )
    time_to_live_ms = require_uint(
        value["time_to_live_ms"],
        f"{context}.time_to_live_ms",
        minimum=1,
        maximum=2**64 - 1,
    )
    nonce = validate_nonce(value["nonce"], f"{context}.nonce")
    if not shared and nonce is None:
        raise ValueError(f"{context}.nonce must be an explicit positive integer")
    if not isinstance(value["metadata"], dict):
        raise ValueError(f"{context}.metadata must be an explicit object")
    gas_limit = validate_fee_payment(
        value["fee_payment"], f"{context}.fee_payment", shared=shared
    )
    validate_admission_intent(
        value["admission_intent"], f"{context}.admission_intent", shared=shared
    )
    requires_gas_limit = validate_executable(
        value["executable"], f"{context}.executable", shared=shared
    )
    if requires_gas_limit and gas_limit is None:
        raise ValueError(
            f"{context}.fee_payment.value.gas_limit must be positive for Ivm, "
            "ContractCall, or a Batch containing ContractCall"
        )
    return PayloadRecord(
        name="",
        authority=authority,
        network_id=network_id,
        creation_time_ms=creation_time_ms,
        time_to_live_ms=time_to_live_ms,
        nonce=nonce,
    )


def validate_swift_fixture_semantics(name: str, payload: dict, context: str) -> None:
    expected = SWIFT_INSTRUCTION_SEMANTICS.get(name)
    if expected is None:
        raise ValueError(f"{context} has unsupported Swift fixture name {name!r}")
    instruction = payload["executable"]["Instructions"][0]
    kind = instruction["kind"]
    action = instruction["arguments"]["action"]
    expected_kind, expected_action = expected
    if kind != expected_kind or action != expected_action:
        raise ValueError(
            f"{context} must use kind/action {expected_kind}/{expected_action}; "
            f"got {kind}/{action}"
        )


def load_payload_records(path: Path, *, shared: bool) -> Dict[str, PayloadRecord]:
    document = parse_json_strict(path)
    if not isinstance(document, list):
        raise ValueError(f"payload descriptor at {path} must be an array")
    records: Dict[str, PayloadRecord] = {}
    for index, entry in enumerate(document):
        context = f"{path} fixture[{index}]"
        if not isinstance(entry, dict):
            raise ValueError(f"{context} must be an object")
        require_exact_fields(
            entry,
            SHARED_PAYLOAD_ENTRY_FIELDS if shared else SWIFT_PAYLOAD_ENTRY_FIELDS,
            context,
        )
        name = require_nonempty_string(entry["name"], f"{context}.name")
        if name in records:
            raise ValueError(f"duplicate fixture name {name!r} in {path}")
        body = validate_payload_body(
            entry["payload"], f"{context}.payload", shared=shared
        )
        if not shared:
            validate_swift_fixture_semantics(name, entry["payload"], context)
        if shared:
            for field in (
                "authority",
                "creation_time_ms",
                "network_id",
                "time_to_live_ms",
                "nonce",
            ):
                if entry[field] != entry["payload"][field]:
                    raise ValueError(f"{context}.{field} does not match payload.{field}")
            payload_bytes = decode_canonical_base64(
                entry["payload_base64"], f"{context}.payload_base64"
            )
            signed_bytes = decode_canonical_base64(
                entry["signed_base64"], f"{context}.signed_base64"
            )
            payload_bare = decode_canonical_norito_frame(
                payload_bytes,
                f"{context}.payload_base64",
                expected_schema=TRANSACTION_PAYLOAD_SCHEMA,
            )
            signed_bare = decode_canonical_norito_frame(
                signed_bytes,
                f"{context}.signed_base64",
                expected_schema=SIGNED_TRANSACTION_SCHEMA,
            )
            payload_hash = require_lower_hex(
                entry["payload_hash"], 64, f"{context}.payload_hash"
            )
            signed_hash = require_lower_hex(
                entry["signed_hash"], 64, f"{context}.signed_hash"
            )
            if iroha_hash(payload_bytes) != payload_hash:
                raise ValueError(f"{context}.payload_hash does not match payload bytes")
            if signed_transaction_payload(signed_bare) != payload_bare:
                raise ValueError(f"{context}.signed_base64 does not contain payload bytes")
            if signed_transaction_entrypoint_hash(signed_bare) != signed_hash:
                raise ValueError(f"{context}.signed_hash does not match signed bytes")
            body = PayloadRecord(
                name=name,
                authority=body.authority,
                network_id=body.network_id,
                creation_time_ms=body.creation_time_ms,
                time_to_live_ms=body.time_to_live_ms,
                nonce=body.nonce,
                payload_base64=entry["payload_base64"],
                payload_hash=payload_hash,
                signed_base64=entry["signed_base64"],
                signed_hash=signed_hash,
            )
        else:
            body = PayloadRecord(
                name=name,
                authority=body.authority,
                network_id=body.network_id,
                creation_time_ms=body.creation_time_ms,
                time_to_live_ms=body.time_to_live_ms,
                nonce=body.nonce,
            )
        records[name] = body
    return records


def load_manifest_records(path: Path, *, swift: bool) -> Dict[str, ManifestRecord]:
    document = parse_json_strict(path)
    if not isinstance(document, dict):
        raise ValueError(f"manifest at {path} must be an object")
    require_exact_fields(
        document, SWIFT_MANIFEST_FIELDS if swift else SHARED_MANIFEST_FIELDS, str(path)
    )
    fixtures = document["fixtures"]
    if not isinstance(fixtures, list):
        raise ValueError(f"{path}.fixtures must be an array")
    records: Dict[str, ManifestRecord] = {}
    encoded_files: set[str] = set()
    payload_hashes: set[str] = set()
    payload_identities: set[bytes] = set()
    signed_hashes: set[str] = set()
    signed_identities: set[bytes] = set()
    for index, entry in enumerate(fixtures):
        context = f"{path}.fixtures[{index}]"
        if not isinstance(entry, dict):
            raise ValueError(f"{context} must be an object")
        require_exact_fields(
            entry,
            SWIFT_MANIFEST_ENTRY_FIELDS if swift else MANIFEST_ENTRY_FIELDS,
            context,
        )
        name = require_nonempty_string(entry["name"], f"{context}.name")
        if name in records:
            raise ValueError(f"duplicate fixture name {name!r} in {path}")
        encoded_file = (
            f"{name}.norito"
            if swift
            else validate_encoded_file(name, entry["encoded_file"], context)
        )
        if encoded_file in encoded_files:
            raise ValueError(f"duplicate encoded_file {encoded_file!r} in {path}")
        encoded_files.add(encoded_file)
        if swift:
            authority = ""
            network_id = ""
            creation_time_ms = 0
            time_to_live_ms = 0
            nonce = None
        else:
            authority = require_nonempty_string(entry["authority"], f"{context}.authority")
            network_id = require_network_id(
                entry["network_id"], f"{context}.network_id"
            )
            creation_time_ms = require_uint(
                entry["creation_time_ms"],
                f"{context}.creation_time_ms",
                maximum=2**64 - 1,
            )
            time_to_live_ms = require_uint(
                entry["time_to_live_ms"],
                f"{context}.time_to_live_ms",
                minimum=1,
                maximum=2**64 - 1,
            )
            nonce = validate_nonce(entry["nonce"], f"{context}.nonce")
        payload_bytes = decode_canonical_base64(
            entry["payload_base64"], f"{context}.payload_base64"
        )
        signed_bytes = decode_canonical_base64(
            entry["signed_base64"], f"{context}.signed_base64"
        )
        payload_hash = require_lower_hex(
            entry["payload_hash"], 64, f"{context}.payload_hash"
        )
        signed_hash = require_lower_hex(
            entry["signed_hash"], 64, f"{context}.signed_hash"
        )
        encoded_len = len(payload_bytes) if swift else require_uint(
            entry["encoded_len"],
            f"{context}.encoded_len",
            minimum=1,
            maximum=MAX_FIXTURE_BYTES,
        )
        signed_len = len(signed_bytes) if swift else require_uint(
            entry["signed_len"],
            f"{context}.signed_len",
            minimum=1,
            maximum=MAX_FIXTURE_BYTES,
        )
        if not 1 <= encoded_len <= MAX_FIXTURE_BYTES or not 1 <= signed_len <= MAX_FIXTURE_BYTES:
            raise ValueError(f"{context} fixture bytes exceed the accepted length bounds")
        if len(payload_bytes) != encoded_len:
            raise ValueError(f"{context}.encoded_len does not match payload_base64")
        if len(signed_bytes) != signed_len:
            raise ValueError(f"{context}.signed_len does not match signed_base64")
        if iroha_hash(payload_bytes) != payload_hash:
            raise ValueError(f"{context}.payload_hash does not match payload bytes")
        payload_codec_bytes = payload_bytes
        signed_codec_bytes = signed_bytes
        if not swift:
            payload_codec_bytes = decode_canonical_norito_frame(
                payload_bytes,
                f"{context}.payload_base64",
                expected_schema=TRANSACTION_PAYLOAD_SCHEMA,
            )
            signed_codec_bytes = decode_canonical_norito_frame(
                signed_bytes,
                f"{context}.signed_base64",
                expected_schema=SIGNED_TRANSACTION_SCHEMA,
            )
        if signed_transaction_payload(signed_codec_bytes) != payload_codec_bytes:
            raise ValueError(f"{context}.signed_base64 does not contain payload bytes")
        if signed_transaction_entrypoint_hash(signed_codec_bytes) != signed_hash:
            raise ValueError(f"{context}.signed_hash does not match signed bytes")
        for identity, seen, label in (
            (payload_hash, payload_hashes, "payload_hash"),
            (payload_bytes, payload_identities, "payload bytes"),
            (signed_hash, signed_hashes, "signed_hash"),
            (signed_bytes, signed_identities, "signed bytes"),
        ):
            if identity in seen:
                raise ValueError(f"duplicate {label} for {name!r} in {path}")
            seen.add(identity)
        records[name] = ManifestRecord(
            name=name,
            authority=authority,
            network_id=network_id,
            creation_time_ms=creation_time_ms,
            time_to_live_ms=time_to_live_ms,
            nonce=nonce,
            encoded_file=encoded_file,
            encoded_len=encoded_len,
            signed_len=signed_len,
            payload_base64=entry["payload_base64"],
            payload_hash=payload_hash,
            signed_base64=entry["signed_base64"],
            signed_hash=signed_hash,
            payload_bytes=payload_bytes,
            signed_bytes=signed_bytes,
        )
    return records


def validate_fixture_set(
    root: Path,
    payload_path: Path,
    manifest_path: Path,
    *,
    shared: bool,
    expected_names: Optional[frozenset[str]] = None,
    require_blobs: bool,
) -> None:
    payloads = load_payload_records(payload_path, shared=shared)
    manifests = load_manifest_records(manifest_path, swift=not shared)
    if set(payloads) != set(manifests):
        raise ValueError(
            f"payload/manifest name mismatch: payloads={sorted(payloads)} "
            f"manifests={sorted(manifests)}"
        )
    if expected_names is not None and set(payloads) != expected_names:
        raise ValueError(
            f"Swift fixture names must be exactly {sorted(expected_names)}, "
            f"got {sorted(payloads)}"
        )
    for name, payload in payloads.items():
        manifest = manifests[name]
        if shared:
            for field in (
                "authority",
                "creation_time_ms",
                "network_id",
                "time_to_live_ms",
                "nonce",
            ):
                if getattr(payload, field) != getattr(manifest, field):
                    raise ValueError(f"{name} manifest/payload mismatch for {field}")
            for field in (
                "payload_base64",
                "payload_hash",
                "signed_base64",
                "signed_hash",
            ):
                if getattr(payload, field) != getattr(manifest, field):
                    raise ValueError(f"{name} manifest/payload mismatch for {field}")
        if require_blobs:
            blob_path = root / manifest.encoded_file
            if not blob_path.is_file():
                raise FileNotFoundError(f"missing fixture blob: {blob_path}")
            if blob_path.read_bytes() != manifest.payload_bytes:
                raise ValueError(f"fixture blob differs from manifest: {blob_path}")
    if require_blobs:
        expected_files = {record.encoded_file for record in manifests.values()}
        if shared:
            actual_files = {path.name for path in root.glob("*.norito") if path.is_file()}
        else:
            actual_files = {
                path.name
                for path in root.glob("swift_*.norito")
                if path.is_file()
            }
        if actual_files != expected_files:
            raise ValueError(
                f"fixture blob set mismatch: expected={sorted(expected_files)} "
                f"actual={sorted(actual_files)}"
            )


def fingerprint(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def compare(source: Path, target: Path) -> Tuple[List[Path], List[Path], List[Tuple[Path, Path]]]:
    for root in (source, target):
        if not root.is_dir():
            raise FileNotFoundError(f"missing directory: {root}")

    source_map = {}
    target_map = {}
    for relative in MANAGED_FIXTURES:
        source_path = source / relative
        if not source_path.is_file():
            raise FileNotFoundError(f"missing canonical fixture: {source_path}")
        source_map[relative] = source_path
        target_path = target / relative
        if target_path.is_file():
            target_map[relative] = target_path

    missing = sorted(rel for rel in source_map if rel not in target_map)
    extra = sorted(
        relative
        for path in target.rglob("*.norito")
        if path.is_file()
        for relative in (path.relative_to(target),)
        if relative.parent != Path(".") or not relative.name.startswith("swift_")
    )

    diffs: List[Tuple[Path, Path]] = []
    for rel, src_path in source_map.items():
        tgt_path = target_map.get(rel)
        if tgt_path is None:
            continue
        if fingerprint(src_path) != fingerprint(tgt_path):
            diffs.append((src_path, tgt_path))

    validate_fixture_set(
        source,
        source / MANAGED_FIXTURES[0],
        source / MANAGED_FIXTURES[1],
        shared=True,
        require_blobs=True,
    )
    if not missing and not diffs:
        # Parse the mirrored descriptors independently so duplicate-key and
        # closed-schema enforcement applies on both sides of the parity gate.
        validate_fixture_set(
            source,
            target / MANAGED_FIXTURES[0],
            target / MANAGED_FIXTURES[1],
            shared=True,
            require_blobs=False,
        )

    swift_payloads = target / SWIFT_PAYLOADS
    swift_manifest = target / SWIFT_MANIFEST
    for required in (swift_payloads, swift_manifest):
        if not required.is_file():
            raise FileNotFoundError(f"missing Swift-owned fixture descriptor: {required}")
    validate_fixture_set(
        target,
        swift_payloads,
        swift_manifest,
        shared=False,
        expected_names=EXPECTED_SWIFT_FIXTURES,
        require_blobs=True,
    )
    return missing, extra, diffs


@dataclass(frozen=True)
class RotationRoster:
    odd_weeks: str = DEFAULT_ODD_OWNER
    even_weeks: str = DEFAULT_EVEN_OWNER

    def expected_owner(self, iso_week: int) -> str:
        return self.odd_weeks if iso_week % 2 else self.even_weeks


@dataclass(frozen=True)
class StateInfo:
    rotation_owner: str
    auto_owner: str
    owner_source: str
    trigger: str
    cadence_label: str
    slot_start: datetime
    next_slot: datetime
    generated_at: datetime
    age_hours: float


def isoformat_utc(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_timestamp(value: str) -> datetime:
    try:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    except ValueError as exc:  # pragma: no cover - defensive
        raise RuntimeError(f"invalid timestamp '{value}': {exc}") from exc


def scheduled_slot(now: datetime, *, label: str = DEFAULT_CADENCE_LABEL, interval_hours: float = 48.0) -> datetime:
    """Return the most recent cadence slot at or before `now`."""
    normalized = (label or DEFAULT_CADENCE_LABEL).strip().lower() or DEFAULT_CADENCE_LABEL
    if normalized == "rolling-48h":
        return rolling_slot(now, interval_hours=interval_hours or 48.0)
    if normalized == "fallback-mon-thu-utc":
        return fallback_slot(now)
    return weekly_slot(now, weekday=3, hour=17)


def weekly_slot(now: datetime, *, weekday: int, hour: int, minute: int = 0) -> datetime:
    now_utc = now.astimezone(timezone.utc)
    iso_year, iso_week, _ = now_utc.isocalendar()
    monday = datetime.strptime(f"{iso_year} {iso_week} 1", "%G %V %u").replace(tzinfo=timezone.utc)
    slot = monday + timedelta(days=weekday - 1, hours=hour, minutes=minute)
    while slot > now_utc:
        slot -= timedelta(days=7)
    return slot


def fallback_slot(now: datetime) -> datetime:
    monday = weekly_slot(now, weekday=1, hour=17)
    thursday = weekly_slot(now, weekday=4, hour=17)
    return max(monday, thursday)


def rolling_slot(now: datetime, *, interval_hours: float) -> datetime:
    anchor = datetime(2026, 1, 1, tzinfo=timezone.utc)
    seconds = interval_hours * 3600.0
    if seconds <= 0:
        return now
    offset = (now - anchor).total_seconds()
    steps = int(offset // seconds)
    return anchor + timedelta(seconds=steps * seconds)


def next_slot(slot: datetime, *, label: str, interval_hours: float) -> datetime:
    if label == "rolling-48h":
        return slot + timedelta(hours=interval_hours or 48.0)
    if label == "fallback-mon-thu-utc":
        if slot.weekday() == 0:
            return slot + timedelta(days=3)
        return slot + timedelta(days=4)
    return slot + timedelta(days=7)


def load_state(path: Path) -> dict:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"cadence state file missing at {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"failed to parse cadence state JSON: {exc}") from exc
    return data


def validate_state(
    state_path: Path,
    roster: RotationRoster,
    *,
    max_age_hours: float,
    schedule_tolerance_hours: float,
    allowed_cadence_labels: Optional[Sequence[str]] = None,
    now: Optional[datetime] = None,
) -> StateInfo:
    state = load_state(state_path)

    generated_at_raw = state.get("generated_at")
    if not isinstance(generated_at_raw, str):
        raise RuntimeError("cadence state missing 'generated_at' field")
    generated_at = parse_timestamp(generated_at_raw)

    current = now or datetime.now(timezone.utc)
    age_hours = (current - generated_at).total_seconds() / 3600.0
    if age_hours > max_age_hours:
        raise RuntimeError(f"fixtures are {age_hours:.1f}h old (limit {max_age_hours:.1f}h)")

    trigger = str(state.get("trigger", "scheduled")).lower()
    if trigger not in {"scheduled", "event"}:
        raise RuntimeError(f"unexpected trigger value '{trigger}' in cadence state")

    raw_label = state.get("cadence")
    cadence_label = (raw_label or DEFAULT_CADENCE_LABEL).strip().lower() or DEFAULT_CADENCE_LABEL
    allowed = [lbl.strip().lower() for lbl in allowed_cadence_labels or [] if lbl.strip()]
    if allowed and cadence_label not in allowed:
        raise RuntimeError(
            f"cadence label '{cadence_label}' is not in the allowed set {allowed}"
        )

    interval_hours = float(state.get("cadence_interval_hours", 48.0))

    window = state.get("window", {}) or {}
    slot_start_raw = window.get("slot_start")
    slot_start = (
        parse_timestamp(slot_start_raw)
        if isinstance(slot_start_raw, str)
        else scheduled_slot(generated_at, label=cadence_label, interval_hours=interval_hours)
    )
    next_slot_raw = window.get("next_slot")
    next_slot_value = (
        parse_timestamp(next_slot_raw)
        if isinstance(next_slot_raw, str)
        else next_slot(slot_start, label=cadence_label, interval_hours=interval_hours)
    )
    window_hours = float(state.get("cadence_window_hours", schedule_tolerance_hours))
    slot_end = (
        parse_timestamp(window.get("slot_end"))
        if isinstance(window.get("slot_end"), str)
        else slot_start + timedelta(hours=window_hours)
    )

    iso_week = int(window.get("iso_week", slot_start.isocalendar()[1]))
    expected_owner = roster.expected_owner(iso_week)

    rotation_owner = str(state.get("rotation_owner", "")).strip()
    if not rotation_owner:
        raise RuntimeError("cadence state missing 'rotation_owner'")
    auto_owner = str(state.get("rotation_owner_auto", expected_owner)).strip() or expected_owner
    owner_source = str(state.get("rotation_owner_source", "auto")).strip().lower() or "auto"

    if auto_owner.lower() != expected_owner.lower():
        raise RuntimeError(
            f"auto rotation owner mismatch (expected '{expected_owner}', got '{auto_owner}')"
        )

    if owner_source == "auto" and rotation_owner.lower() != expected_owner.lower():
        raise RuntimeError(
            f"rotation owner mismatch (expected '{expected_owner}', got '{rotation_owner}')"
        )

    tolerance = timedelta(hours=schedule_tolerance_hours)
    if trigger == "scheduled":
        if generated_at < slot_start - tolerance:
            raise RuntimeError(
                f"fixtures regenerated before scheduled window (slot {isoformat_utc(slot_start)}, generated {isoformat_utc(generated_at)})"
            )
        if generated_at > slot_end + tolerance:
            raise RuntimeError(
                f"fixtures regenerated after scheduled window (slot {isoformat_utc(slot_start)}, generated {isoformat_utc(generated_at)})"
        )

    return StateInfo(
        rotation_owner=rotation_owner,
        auto_owner=auto_owner,
        owner_source=owner_source,
        trigger=trigger,
        cadence_label=cadence_label,
        slot_start=slot_start,
        next_slot=next_slot_value,
        generated_at=generated_at,
        age_hours=age_hours,
    )


def format_state_summary(info: StateInfo, max_age_hours: float) -> str:
    return (
        "[swift-fixtures] cadence ok: "
        f"age={info.age_hours:.1f}h (limit {max_age_hours:.1f}h) "
        f"owner={info.rotation_owner} (auto={info.auto_owner}, source={info.owner_source}) "
        f"trigger={info.trigger} cadence={info.cadence_label} "
        f"slot_start={isoformat_utc(info.slot_start)} "
        f"next_slot={isoformat_utc(info.next_slot)}"
    )


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check Swift Norito fixture parity and cadence metadata")
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE,
                        help=f"Canonical fixture directory (default: {DEFAULT_SOURCE})")
    parser.add_argument("--target", type=Path, default=DEFAULT_TARGET,
                        help=f"Swift fixture directory (default: {DEFAULT_TARGET})")
    parser.add_argument("--quiet", action="store_true", help="Suppress success output")
    parser.add_argument("--state", action="store_true",
                        help=f"Validate cadence state metadata using {DEFAULT_STATE}")
    parser.add_argument("--state-file", type=Path,
                        help="Path to cadence state JSON (implies --state)")
    parser.add_argument("--max-age-hours", type=float, default=48.0,
                        help="Maximum allowed fixture age when validating cadence state (default: 48)")
    parser.add_argument("--odd-week-owner", default=DEFAULT_ODD_OWNER,
                        help=f"Expected rotation owner for odd ISO weeks (default: {DEFAULT_ODD_OWNER})")
    parser.add_argument("--even-week-owner", default=DEFAULT_EVEN_OWNER,
                        help=f"Expected rotation owner for even ISO weeks (default: {DEFAULT_EVEN_OWNER})")
    parser.add_argument("--schedule-tolerance-hours", type=float, default=6.0,
                        help="Allowed lead/lag (hours) around the scheduled slot when --state is used (default: 6)")
    parser.add_argument(
        "--cadence-label",
        action="append",
        dest="cadence_labels",
        help="Allowed cadence label (default: weekly-wed-1700utc). Pass multiple times to allow more than one label.",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    try:
        missing, extra, diffs = compare(args.source, args.target)
    except (FileNotFoundError, ValueError) as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    has_error = False
    if missing:
        has_error = True
        print("[error] missing files in target:")
        for rel in missing:
            print(f"    {rel}")
    if extra:
        has_error = True
        print("[error] unexpected files in target:")
        for rel in extra:
            print(f"    {rel}")
    if diffs:
        has_error = True
        print("[error] content mismatches:")
        for src, tgt in diffs:
            rel = tgt.relative_to(args.target)
            print(f"    {rel} (source={src}, target={tgt})")

    if has_error:
        return 1

    if not args.quiet:
        print(f"[ok] Fixtures match between {args.source} and {args.target}")

    state_path: Optional[Path] = args.state_file
    if args.state:
        state_path = state_path or DEFAULT_STATE

    if state_path is not None:
        roster = RotationRoster(args.odd_week_owner, args.even_week_owner)
        allowed_labels = args.cadence_labels or [DEFAULT_CADENCE_LABEL]
        try:
            info = validate_state(
                state_path,
                roster,
                max_age_hours=args.max_age_hours,
                schedule_tolerance_hours=args.schedule_tolerance_hours,
                allowed_cadence_labels=allowed_labels,
            )
        except RuntimeError as exc:
            print(f"[error] {exc}", file=sys.stderr)
            return 1
        print(format_state_summary(info, args.max_age_hours))
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
