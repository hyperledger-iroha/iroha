"""Closed first-release SCCP discovery, artifact, and submission helpers."""

from __future__ import annotations

import base64
import hashlib
import json
import re
from dataclasses import dataclass
from types import MappingProxyType
from typing import Any, Dict, Mapping, Optional, Sequence, Tuple, Union

SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_ETH = 1
SCCP_DOMAIN_BSC = 2
SCCP_DOMAIN_TRON = 5

SCCP_CODEC_CANONICAL_TEXT = 1
SCCP_CODEC_EVM_ADDRESS20 = 2
SCCP_CODEC_TRON_ADDRESS21 = 5

SCCP_CODEC_KEYS = MappingProxyType(
    {
        SCCP_CODEC_CANONICAL_TEXT: "canonical_text",
        SCCP_CODEC_EVM_ADDRESS20: "evm_address20",
        SCCP_CODEC_TRON_ADDRESS21: "tron_address21",
    }
)
SCCP_PAYLOAD_KINDS = ("transfer",)

_SOURCE_EVENT_PREFIX = b"sccp:source:event:v1"
_MAX_WIRE_BYTES = 16 * 1024 * 1024
_BN254_BASE_FIELD_MODULUS = int(
    "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47", 16
)
_ROUTE_KEY = re.compile(r"[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?")

_NETWORKS: Mapping[str, Tuple[int, int, bool]] = MappingProxyType(
    {
        "sora-nexus": (0, SCCP_DOMAIN_SORA, True),
        "sora-taira": (1, SCCP_DOMAIN_SORA, True),
        "ethereum-mainnet": (2, SCCP_DOMAIN_ETH, False),
        "ethereum-sepolia": (3, SCCP_DOMAIN_ETH, False),
        "bsc-mainnet": (4, SCCP_DOMAIN_BSC, False),
        "bsc-testnet": (5, SCCP_DOMAIN_BSC, False),
        "tron-mainnet": (10, SCCP_DOMAIN_TRON, False),
        "tron-nile": (11, SCCP_DOMAIN_TRON, False),
        "tron-shasta": (12, SCCP_DOMAIN_TRON, False),
    }
)
SCCP_NETWORK_PROFILES = MappingProxyType(
    {
        profile: MappingProxyType(
            {"profile": profile, "tag": tag, "domain": domain, "sora": sora}
        )
        for profile, (tag, domain, sora) in _NETWORKS.items()
    }
)
_NETWORK_WIRE_NAMES = MappingProxyType(
    {profile.replace("-", "_"): profile for profile in _NETWORKS}
)
_NATIVE_BACKENDS: Mapping[str, frozenset[str]] = MappingProxyType(
    {
        "ethereum_beacon_v1": frozenset({"ethereum-mainnet", "ethereum-sepolia"}),
        "bsc_parlia_v1": frozenset({"bsc-mainnet", "bsc-testnet"}),
        "tron_dpos_v1": frozenset({"tron-mainnet", "tron-nile", "tron-shasta"}),
    }
)
_DESTINATION_BACKENDS = MappingProxyType(
    {"evm_groth16_bn254_v1": "evm", "tron_groth16_bn254_v1": "tron"}
)
_CAPABILITY_PATHS = MappingProxyType(
    {
        "registry_path": "/v1/sccp/registry",
        "message_bundle_path": "/v1/sccp/proofs/message/{message_id}",
        "proof_request_path": "/v1/sccp/proof-requests/{message_id}",
        "recent_messages_path": "/v1/sccp/messages/recent",
        "proof_submit_path": "/v1/bridge/proofs/submit",
        "native_message_submit_path": "/v1/bridge/messages",
    }
)

_BRIDGE_RESPONSE_FIELDS = frozenset(
    {
        "submitted",
        "payload_kind",
        "message_id_hex",
        "backend",
        "counterparty_domain",
        "counterparty_chain",
        "manifest_hash_hex",
        "range_start_height",
        "range_end_height",
        "creation_time_ms",
        "tx_hash_hex",
        "transaction_payload_b64",
        "signing_message_b64",
    }
)

_U64_MASK = (1 << 64) - 1
_KECCAK_RATE = 136
_KECCAK_ROUND_CONSTANTS = (
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808A,
    0x8000000080008000,
    0x000000000000808B,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008A,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000A,
    0x000000008000808B,
    0x800000000000008B,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800A,
    0x800000008000000A,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
)
_KECCAK_RHO_OFFSETS = (
    (0, 36, 3, 41, 18),
    (1, 44, 10, 45, 2),
    (62, 6, 43, 15, 61),
    (28, 55, 25, 21, 56),
    (27, 20, 39, 8, 14),
)


@dataclass(frozen=True)
class SccpCapabilities:
    """Closed SCCP endpoint capability snapshot."""

    version: int
    registry_revision: str
    registry_path: str
    message_bundle_path: str
    proof_request_path: str
    recent_messages_path: str
    proof_submit_path: Optional[str]
    native_message_submit_path: Optional[str]


@dataclass(frozen=True)
class SccpRegistry:
    """Authoritative typed SCCP registry."""

    version: int
    lanes: Tuple[Mapping[str, Any], ...]


@dataclass(frozen=True)
class SccpRecentMessages:
    """Newest-first SCCP message discovery page."""

    items: Tuple[Mapping[str, Any], ...]


@dataclass(frozen=True)
class SccpBridgeSubmitResponse:
    """Unified prepared-or-submitted SCCP transaction response."""

    submitted: bool
    payload_kind: str
    message_id_hex: str
    backend: str
    counterparty_domain: int
    counterparty_chain: str
    manifest_hash_hex: str
    range_start_height: int
    range_end_height: int
    creation_time_ms: int
    tx_hash_hex: Optional[str]
    transaction_payload_b64: Optional[str]
    signing_message_b64: Optional[str]


def _mapping(value: Any, label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{label} must be a mapping")
    if not all(isinstance(key, str) for key in value):
        raise TypeError(f"{label} keys must be strings")
    return value


def _exact_fields(
    value: Any,
    allowed: frozenset[str],
    label: str,
    required: Optional[frozenset[str]] = None,
) -> Mapping[str, Any]:
    record = _mapping(value, label)
    unknown = next((key for key in record if key not in allowed), None)
    if unknown is not None:
        raise ValueError(f"{label} contains unknown or retired field `{unknown}`")
    for field in allowed if required is None else required:
        if field not in record:
            raise ValueError(f"{label} is missing required field `{field}`")
    return record


def _text(value: Any, label: str, maximum_bytes: int = 4096) -> str:
    if not isinstance(value, str) or not value or value != value.strip():
        raise ValueError(f"{label} must be canonical nonempty text")
    if len(value.encode("utf-8")) > maximum_bytes:
        raise ValueError(f"{label} exceeds its byte-size bound")
    return value


def _integer(value: Any, label: str, minimum: int, maximum: int = (1 << 63) - 1) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise ValueError(f"{label} must be an integer in {minimum}..{maximum}")
    return value


def _boolean(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        raise TypeError(f"{label} must be boolean")
    return value


def _list(value: Any, label: str) -> Sequence[Any]:
    if not isinstance(value, list):
        raise TypeError(f"{label} must be an array")
    return value


def _binary(value: Any, label: str) -> bytes:
    if not isinstance(value, (bytes, bytearray, memoryview)):
        raise TypeError(f"{label} must be bytes-like")
    return bytes(value)


def _lower_hex(
    value: Any,
    label: str,
    byte_length: int,
    *,
    prefix: bool = False,
    nonzero: bool = True,
) -> str:
    pattern = rf"0x[0-9a-f]{{{byte_length * 2}}}" if prefix else rf"[0-9a-f]{{{byte_length * 2}}}"
    if not isinstance(value, str) or re.fullmatch(pattern, value) is None:
        raise ValueError(f"{label} must be canonical lowercase {byte_length}-byte hex")
    body = value[2:] if prefix else value
    if nonzero and set(body) == {"0"}:
        raise ValueError(f"{label} must be nonzero")
    return value


def _upper_hex(value: Any, label: str, byte_length: int, *, nonzero: bool = True) -> str:
    if not isinstance(value, str) or re.fullmatch(rf"[0-9A-F]{{{byte_length * 2}}}", value) is None:
        raise ValueError(f"{label} must be canonical uppercase {byte_length}-byte hex")
    if nonzero and set(value) == {"0"}:
        raise ValueError(f"{label} must be nonzero")
    return value


def _variable_hex(value: Any, label: str, *, maximum_bytes: int = _MAX_WIRE_BYTES) -> str:
    if (
        not isinstance(value, str)
        or re.fullmatch(r"0x(?:[0-9a-f]{2})+", value) is None
        or (len(value) - 2) // 2 > maximum_bytes
    ):
        raise ValueError(f"{label} must be canonical nonempty lowercase 0x-prefixed hex")
    return value


def _canonical_base64(
    value: Any, label: str, *, maximum_bytes: int = _MAX_WIRE_BYTES
) -> bytes:
    if (
        not isinstance(value, str)
        or not value
        or len(value) % 4
        or re.fullmatch(
            r"(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?",
            value,
        )
        is None
    ):
        raise ValueError(f"{label} must be canonical padded base64")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, TypeError) as exc:
        raise ValueError(f"{label} must be canonical padded base64") from exc
    if base64.b64encode(decoded).decode("ascii") != value:
        raise ValueError(f"{label} must be canonical padded base64")
    if not decoded or len(decoded) > maximum_bytes:
        raise ValueError(f"{label} is outside its byte-size bound")
    return decoded


def _path(value: Any, label: str) -> str:
    path = _text(value, label, 1024)
    if (
        not path.startswith("/")
        or "//" in path
        or "?" in path
        or "#" in path
        or "%" in path
        or "\\" in path
    ):
        raise ValueError(f"{label} must be a canonical absolute Torii path")
    return path


def _capability_path(value: Any, field: str, *, optional: bool = False) -> Optional[str]:
    if optional and value is None:
        return None
    path = _path(value, field)
    if path != _CAPABILITY_PATHS[field]:
        raise ValueError(f"{field} does not match the SCCP V1 endpoint")
    return path


def _profile(value: Any, label: str) -> Tuple[str, int, int, bool]:
    key = _text(value, label, 64)
    try:
        tag, domain, sora = _NETWORKS[key]
    except KeyError as exc:
        raise ValueError(f"{label} is unsupported or retired") from exc
    return key, tag, domain, sora


def _network(value: Any, label: str) -> Tuple[str, int, int, bool]:
    record = _exact_fields(value, frozenset({"network", "profile"}), label)
    if record["profile"] is not None:
        raise ValueError(f"{label}.profile must be null")
    wire = _text(record["network"], f"{label}.network", 64)
    try:
        profile = _NETWORK_WIRE_NAMES[wire]
    except KeyError as exc:
        raise ValueError(f"{label}.network is unsupported or retired") from exc
    return _profile(profile, f"{label}.network")


def _lane(value: Any, label: str) -> Tuple[Tuple[str, int, int, bool], Tuple[str, int, int, bool]]:
    record = _exact_fields(value, frozenset({"source", "target"}), label)
    source = _network(record["source"], f"{label}.source")
    target = _network(record["target"], f"{label}.target")
    if source[3] or target[0] != "sora-taira" or source[2] == target[2]:
        raise ValueError(f"{label} must be an exact supported external-to-Taira lane")
    return source, target


def _same_lane(left: Any, right: Any) -> bool:
    return left[0][0] == right[0][0] and left[1][0] == right[1][0]


def _family(network: Tuple[str, int, int, bool]) -> str:
    return "tron" if network[0].startswith("tron-") else "evm"


def _unit_backend(
    value: Any, label: str, content_field: str, allowed: Mapping[str, Any]
) -> str:
    record = _exact_fields(value, frozenset({"backend", content_field}), label)
    if record[content_field] is not None:
        raise ValueError(f"{label}.{content_field} must be null")
    backend = _text(record["backend"], f"{label}.backend", 64)
    if backend not in allowed:
        raise ValueError(f"{label}.backend is unsupported or retired")
    return backend


def _deep_freeze(value: Any) -> Any:
    if isinstance(value, Mapping):
        return MappingProxyType({key: _deep_freeze(entry) for key, entry in value.items()})
    if isinstance(value, list):
        return tuple(_deep_freeze(entry) for entry in value)
    return value


def _rotl64(value: int, shift: int) -> int:
    if shift == 0:
        return value & _U64_MASK
    return ((value << shift) | (value >> (64 - shift))) & _U64_MASK


def _keccak_f1600(state: Sequence[int]) -> Sequence[int]:
    lanes = list(state)
    for round_constant in _KECCAK_ROUND_CONSTANTS:
        columns = [
            lanes[x] ^ lanes[x + 5] ^ lanes[x + 10] ^ lanes[x + 15] ^ lanes[x + 20]
            for x in range(5)
        ]
        deltas = [columns[(x - 1) % 5] ^ _rotl64(columns[(x + 1) % 5], 1) for x in range(5)]
        for x in range(5):
            for y in range(5):
                lanes[x + 5 * y] ^= deltas[x]
        rotated = [0] * 25
        for x in range(5):
            for y in range(5):
                rotated[y + 5 * ((2 * x + 3 * y) % 5)] = _rotl64(
                    lanes[x + 5 * y], _KECCAK_RHO_OFFSETS[x][y]
                )
        for x in range(5):
            for y in range(5):
                lanes[x + 5 * y] = rotated[x + 5 * y] ^ (
                    (~rotated[(x + 1) % 5 + 5 * y]) & rotated[(x + 2) % 5 + 5 * y]
                )
        lanes[0] ^= round_constant
    return lanes


def _keccak_256(payload: bytes) -> bytes:
    state = [0] * 25
    padded = bytearray(payload)
    padded.append(0x01)
    padded.extend(b"\x00" * ((_KECCAK_RATE - len(padded) % _KECCAK_RATE) % _KECCAK_RATE))
    padded[-1] |= 0x80
    for offset in range(0, len(padded), _KECCAK_RATE):
        block = padded[offset : offset + _KECCAK_RATE]
        for index in range(_KECCAK_RATE // 8):
            state[index] ^= int.from_bytes(block[index * 8 : index * 8 + 8], "little")
        state = list(_keccak_f1600(state))
    output = bytearray()
    while len(output) < 32:
        for index in range(_KECCAK_RATE // 8):
            output.extend(state[index].to_bytes(8, "little"))
            if len(output) >= 32:
                break
        if len(output) < 32:
            state = list(_keccak_f1600(state))
    return bytes(output[:32])


def _g1(value: Any, label: str) -> Tuple[str, str]:
    record = _exact_fields(value, frozenset({"x", "y"}), label)
    coordinates = tuple(_upper_hex(record[field], f"{label}.{field}", 32) for field in ("x", "y"))
    for field, coordinate in zip(("x", "y"), coordinates):
        if int(coordinate, 16) >= _BN254_BASE_FIELD_MODULUS:
            raise ValueError(f"{label}.{field} is not a BN254 field element")
    return coordinates  # type: ignore[return-value]


def _g2(value: Any, label: str) -> Tuple[str, str, str, str]:
    fields = ("x_c0", "x_c1", "y_c0", "y_c1")
    record = _exact_fields(value, frozenset(fields), label)
    coordinates = tuple(_upper_hex(record[field], f"{label}.{field}", 32) for field in fields)
    for field, coordinate in zip(fields, coordinates):
        if int(coordinate, 16) >= _BN254_BASE_FIELD_MODULUS:
            raise ValueError(f"{label}.{field} is not a BN254 field element")
    return coordinates  # type: ignore[return-value]


def _verifying_key(value: Any, label: str) -> bytes:
    record = _exact_fields(
        value,
        frozenset({"version", "alpha1", "beta2", "gamma2", "delta2", "ic"}),
        label,
    )
    _integer(record["version"], f"{label}.version", 1, 1)
    words = [
        *_g1(record["alpha1"], f"{label}.alpha1"),
        *_g2(record["beta2"], f"{label}.beta2"),
        *_g2(record["gamma2"], f"{label}.gamma2"),
        *_g2(record["delta2"], f"{label}.delta2"),
    ]
    ic_fields = (
        "constant",
        "signal_0",
        "signal_1",
        "signal_2",
        "signal_3",
        "signal_4",
        "signal_5",
        "signal_6",
        "signal_7",
        "signal_8",
        "signal_9",
    )
    ic = _exact_fields(record["ic"], frozenset(ic_fields), f"{label}.ic")
    for field in ic_fields:
        words.extend(_g1(ic[field], f"{label}.ic.{field}"))
    if len(words) != 36:
        raise ValueError(f"{label} must contain exactly 36 ABI words")
    return bytes.fromhex("".join(words))


def normalize_sccp_codec_value(
    codec: int, value: Union[str, bytes, bytearray, memoryview]
) -> bytes:
    """Validate and normalize one closed SCCP V1 codec value."""

    if codec not in SCCP_CODEC_KEYS:
        raise ValueError("codec is unsupported or retired")
    if codec == SCCP_CODEC_CANONICAL_TEXT:
        text = _text(value, "canonical_text", 256)
        if re.fullmatch(r"[\x20-\x7e]+", text) is None:
            raise ValueError("canonical_text must contain printable ASCII only")
        return text.encode("ascii")
    raw = _binary(value, SCCP_CODEC_KEYS[codec])
    if not raw or not any(raw):
        raise ValueError(f"{SCCP_CODEC_KEYS[codec]} must be nonzero")
    if codec == SCCP_CODEC_EVM_ADDRESS20 and len(raw) != 20:
        raise ValueError("evm_address20 must contain exactly 20 bytes")
    if codec == SCCP_CODEC_TRON_ADDRESS21 and (
        len(raw) != 21 or raw[0] != 0x41 or not any(raw[1:])
    ):
        raise ValueError("tron_address21 must contain 0x41 and a nonzero 20-byte address")
    return raw


def sccp_source_event_digest(
    lane_hash: Union[str, bytes, bytearray, memoryview],
    message_id: Union[str, bytes, bytearray, memoryview],
    payload_hash: Union[str, bytes, bytearray, memoryview],
) -> str:
    """Return Keccak(`sccp:source:event:v1 || 0x01 || lane || message || payload`)."""

    roles = []
    for value, label in zip(
        (lane_hash, message_id, payload_hash), ("lane_hash", "message_id", "payload_hash")
    ):
        if isinstance(value, str):
            _lower_hex(value, label, 32)
            raw = bytes.fromhex(value)
        else:
            raw = _binary(value, label)
            if len(raw) != 32 or not any(raw):
                raise ValueError(f"{label} must be a nonzero 32-byte hash")
        roles.append(raw)
    if len(set(roles)) != len(roles):
        raise ValueError("SCCP lane, message, and payload hash roles must be distinct")
    return _keccak_256(_SOURCE_EVENT_PREFIX + b"\x01" + b"".join(roles)).hex()


def normalize_sccp_capabilities(value: Any) -> SccpCapabilities:
    """Normalize the closed SCCP endpoint capability snapshot."""

    allowed = frozenset(
        {
            "version",
            "registry_revision",
            "registry_path",
            "message_bundle_path",
            "proof_request_path",
            "recent_messages_path",
            "proof_submit_path",
            "native_message_submit_path",
        }
    )
    required = frozenset(
        {
            "version",
            "registry_revision",
            "registry_path",
            "message_bundle_path",
            "proof_request_path",
            "recent_messages_path",
        }
    )
    record = _exact_fields(value, allowed, "SCCP capabilities", required)
    return SccpCapabilities(
        version=_integer(record["version"], "SCCP capabilities.version", 1, 1),
        registry_revision=_lower_hex(
            record["registry_revision"], "SCCP capabilities.registry_revision", 32, prefix=True
        ),
        registry_path=_capability_path(record["registry_path"], "registry_path") or "",
        message_bundle_path=_capability_path(
            record["message_bundle_path"], "message_bundle_path"
        )
        or "",
        proof_request_path=_capability_path(record["proof_request_path"], "proof_request_path")
        or "",
        recent_messages_path=_capability_path(
            record["recent_messages_path"], "recent_messages_path"
        )
        or "",
        proof_submit_path=_capability_path(
            record.get("proof_submit_path"), "proof_submit_path", optional=True
        ),
        native_message_submit_path=_capability_path(
            record.get("native_message_submit_path"),
            "native_message_submit_path",
            optional=True,
        ),
    )


def _native_anchor(value: Any, lane: Any, label: str) -> None:
    if value is None:
        return
    record = _exact_fields(
        value, frozenset({"backend", "anchor_hash", "checkpoint_height"}), label
    )
    backend = _unit_backend(record["backend"], f"{label}.backend", "protocol", _NATIVE_BACKENDS)
    if lane[0][0] not in _NATIVE_BACKENDS[backend]:
        raise ValueError(f"{label}.backend does not match the lane source")
    _upper_hex(record["anchor_hash"], f"{label}.anchor_hash", 32)
    _integer(record["checkpoint_height"], f"{label}.checkpoint_height", 1)


def _activation(value: Any, label: str) -> str:
    record = _exact_fields(value, frozenset({"activation", "direction"}), label)
    if record["direction"] is not None:
        raise ValueError(f"{label}.direction must be null")
    activation = _text(record["activation"], f"{label}.activation", 32)
    if activation not in {"staged", "bidirectional", "inbound_only", "paused", "retired"}:
        raise ValueError(f"{label}.activation is unsupported")
    return activation


def _source_identity(value: Any, lane: Any, label: str) -> Tuple[str, str, str, str]:
    record = _exact_fields(value, frozenset({"lane", "emitter"}), label)
    if not _same_lane(_lane(record["lane"], f"{label}.lane"), lane):
        raise ValueError(f"{label}.lane does not match the route")
    emitter = _exact_fields(record["emitter"], frozenset({"emitter", "identity"}), f"{label}.emitter")
    family = _text(emitter["emitter"], f"{label}.emitter.emitter", 16)
    if family != _family(lane[0]):
        raise ValueError(f"{label}.emitter does not match the lane source")
    identity = _exact_fields(
        emitter["identity"],
        frozenset({"address", "runtime_code_hash", "route_config_hash"}),
        f"{label}.emitter.identity",
    )
    address = _upper_hex(identity["address"], f"{label}.emitter.identity.address", 20)
    runtime = _upper_hex(
        identity["runtime_code_hash"], f"{label}.emitter.identity.runtime_code_hash", 32
    )
    configuration = _upper_hex(
        identity["route_config_hash"], f"{label}.emitter.identity.route_config_hash", 32
    )
    if runtime == configuration:
        raise ValueError(f"{label} runtime and route-configuration hashes must be distinct")
    return family, address, runtime, configuration


def _destination(value: Any, lane: Any, label: str) -> Tuple[str, str, str]:
    record = _exact_fields(value, frozenset({"family", "deployment"}), label)
    family = _text(record["family"], f"{label}.family", 16)
    if family != _family(lane[0]):
        raise ValueError(f"{label}.family does not match the lane source")
    fields = frozenset(
        {
            "token_address",
            "token_code_hash",
            "verifier_address",
            "verifier_code_hash",
            "verifying_key",
            "verifier_key_hash",
            "route_address",
            "route_code_hash",
            "taira_to_token_multiplier",
        }
    )
    deployment = _exact_fields(record["deployment"], fields, f"{label}.deployment")
    addresses = tuple(
        _upper_hex(deployment[field], f"{label}.deployment.{field}", 20)
        for field in ("token_address", "verifier_address", "route_address")
    )
    hashes = tuple(
        _upper_hex(deployment[field], f"{label}.deployment.{field}", 32)
        for field in ("token_code_hash", "verifier_code_hash", "verifier_key_hash", "route_code_hash")
    )
    if len(set(addresses)) != len(addresses) or len(set(hashes)) != len(hashes):
        raise ValueError(f"{label}.deployment reuses a role-separated address or hash")
    key_bytes = _verifying_key(deployment["verifying_key"], f"{label}.deployment.verifying_key")
    if _keccak_256(key_bytes).hex().upper() != deployment["verifier_key_hash"]:
        raise ValueError(f"{label}.deployment.verifier_key_hash does not match verifying_key")
    _integer(
        deployment["taira_to_token_multiplier"],
        f"{label}.deployment.taira_to_token_multiplier",
        1_000_000_000,
        1_000_000_000,
    )
    return family, addresses[2], hashes[3]


def _settlement(value: Any, label: str) -> None:
    record = _exact_fields(
        value,
        frozenset({"asset_definition_id", "custody_account_id", "payload_amount_scale"}),
        label,
    )
    _text(record["asset_definition_id"], f"{label}.asset_definition_id", 512)
    authority = _text(record["custody_account_id"], f"{label}.custody_account_id", 512)
    from .client import _decode_i105_string

    _decode_i105_string(authority)
    _integer(record["payload_amount_scale"], f"{label}.payload_amount_scale", 9, 9)


def _route(value: Any, lane: Any, label: str) -> Tuple[str, str, int, str]:
    fields = frozenset(
        {
            "lane_id",
            "route_id",
            "asset_key",
            "revision",
            "activation",
            "source_identity",
            "destination",
            "settlement",
        }
    )
    record = _exact_fields(value, fields, label)
    if not _same_lane(_lane(record["lane_id"], f"{label}.lane_id"), lane):
        raise ValueError(f"{label}.lane_id does not match its lane")
    for field in ("route_id", "asset_key"):
        if not isinstance(record[field], str) or _ROUTE_KEY.fullmatch(record[field]) is None:
            raise ValueError(f"{label}.{field} must be canonical lowercase route text")
    revision = _integer(record["revision"], f"{label}.revision", 1, 0xFFFF_FFFF)
    activation = _activation(record["activation"], f"{label}.activation")
    source = _source_identity(record["source_identity"], lane, f"{label}.source_identity")
    destination = _destination(record["destination"], lane, f"{label}.destination")
    if source[0] != destination[0] or source[1] != destination[1] or source[2] != destination[2]:
        raise ValueError(f"{label} source emitter does not identify the destination route")
    _settlement(record["settlement"], f"{label}.settlement")
    lineage = f"{record['route_id']}\x00{record['asset_key']}"
    key = f"{lane[0][0]}\x00{lane[1][0]}\x00{lineage}\x00{revision}"
    return lineage, key, revision, activation


def normalize_sccp_registry(value: Any) -> SccpRegistry:
    """Validate the authoritative typed registry without treating it as a manifest."""

    record = _exact_fields(value, frozenset({"version", "lanes"}), "SCCP registry")
    version = _integer(record["version"], "SCCP registry.version", 1, 1)
    lanes = _list(record["lanes"], "SCCP registry.lanes")
    if len(lanes) > 16:
        raise ValueError("SCCP registry contains more than 16 lanes")
    lane_keys: set[Tuple[str, str]] = set()
    route_keys: set[str] = set()
    route_count = 0
    for lane_index, entry in enumerate(lanes):
        label = f"SCCP registry.lanes[{lane_index}]"
        lane_record = _exact_fields(
            entry, frozenset({"lane_id", "native_trust_anchor", "routes"}), label
        )
        lane = _lane(lane_record["lane_id"], f"{label}.lane_id")
        lane_key = (lane[0][0], lane[1][0])
        if lane_key in lane_keys:
            raise ValueError("SCCP registry contains a duplicate lane")
        lane_keys.add(lane_key)
        _native_anchor(lane_record["native_trust_anchor"], lane, f"{label}.native_trust_anchor")
        routes = _list(lane_record["routes"], f"{label}.routes")
        if not 1 <= len(routes) <= 8:
            raise ValueError(f"{label}.routes must contain 1..8 routes")
        route_count += len(routes)
        lineages: Dict[str, list[Tuple[int, str]]] = {}
        for route_index, route_value in enumerate(routes):
            lineage, route_key, revision, activation = _route(
                route_value, lane, f"{label}.routes[{route_index}]"
            )
            if route_key in route_keys:
                raise ValueError("SCCP registry contains a duplicate route")
            route_keys.add(route_key)
            lineages.setdefault(lineage, []).append((revision, activation))
        for revisions in lineages.values():
            revisions.sort()
            if [revision for revision, _ in revisions] != list(range(1, len(revisions) + 1)):
                raise ValueError("SCCP route revisions must start at one and contain no gaps")
            if sum(activation == "bidirectional" for _, activation in revisions) > 1:
                raise ValueError("SCCP registry enables multiple revisions of one route")
    if route_count > 64:
        raise ValueError("SCCP registry contains more than 64 routes")
    frozen = _deep_freeze(record)
    return SccpRegistry(version=version, lanes=tuple(frozen["lanes"]))


def normalize_sccp_recent_messages(value: Any) -> SccpRecentMessages:
    """Normalize newest-first discovery with only bundle and proof-request links."""

    root = _exact_fields(value, frozenset({"items"}), "SCCP recent messages")
    items = []
    allowed = frozenset(
        {
            "height",
            "message_id_hex",
            "kind",
            "source_profile",
            "target_profile",
            "destination_binding_hash",
            "route_configuration_hash",
            "target_domain",
            "asset_id",
            "route_id",
            "recipient",
            "amount",
            "payload_projection",
            "links",
        }
    )
    required = frozenset(
        {
            "height",
            "message_id_hex",
            "kind",
            "source_profile",
            "target_profile",
            "destination_binding_hash",
            "route_configuration_hash",
            "target_domain",
            "amount",
            "links",
        }
    )
    for index, entry in enumerate(_list(root["items"], "SCCP recent messages.items")):
        label = f"SCCP recent messages.items[{index}]"
        record = _exact_fields(entry, allowed, label, required)
        source = _profile(record["source_profile"], f"{label}.source_profile")
        target = _profile(record["target_profile"], f"{label}.target_profile")
        if source[0] != "sora-taira" or target[3] or record["kind"] != "transfer":
            raise ValueError(f"{label} must describe a Taira-origin external transfer")
        message_id = _lower_hex(record["message_id_hex"], f"{label}.message_id_hex", 32)
        links = _exact_fields(
            record["links"], frozenset({"bundle_path", "proof_request_path"}), f"{label}.links"
        )
        expected_bundle = f"/v1/sccp/proofs/message/{message_id}"
        expected_request = f"/v1/sccp/proof-requests/{message_id}"
        if (
            _path(links["bundle_path"], f"{label}.links.bundle_path") != expected_bundle
            or _path(links["proof_request_path"], f"{label}.links.proof_request_path")
            != expected_request
        ):
            raise ValueError(f"{label}.links do not identify this exact message")
        if _integer(record["target_domain"], f"{label}.target_domain", 1, 5) != target[2]:
            raise ValueError(f"{label} profile and domain fields disagree")

        def optional_text(field: str) -> Optional[str]:
            return None if record.get(field) is None else _text(record[field], f"{label}.{field}")

        amount = _text(record["amount"], f"{label}.amount")
        if re.fullmatch(r"[1-9][0-9]*", amount) is None:
            raise ValueError(f"{label}.amount must be a positive canonical decimal string")
        destination_binding_hash = _lower_hex(
            record["destination_binding_hash"],
            f"{label}.destination_binding_hash",
            32,
            prefix=True,
        )
        route_configuration_hash = _lower_hex(
            record["route_configuration_hash"],
            f"{label}.route_configuration_hash",
            32,
            prefix=True,
        )
        if destination_binding_hash == route_configuration_hash:
            raise ValueError(f"{label} binding and route-configuration hashes must be distinct")
        items.append(
            _deep_freeze(
                {
                    "height": _integer(record["height"], f"{label}.height", 1),
                    "message_id_hex": message_id,
                    "kind": "transfer",
                    "source_profile": source[0],
                    "target_profile": target[0],
                    "destination_binding_hash": destination_binding_hash,
                    "route_configuration_hash": route_configuration_hash,
                    "target_domain": target[2],
                    "asset_id": optional_text("asset_id"),
                    "route_id": optional_text("route_id"),
                    "recipient": optional_text("recipient"),
                    "amount": amount,
                    "payload_projection": (
                        None
                        if record.get("payload_projection") is None
                        else _mapping(record["payload_projection"], f"{label}.payload_projection")
                    ),
                    "links": {
                        "bundle_path": expected_bundle,
                        "proof_request_path": expected_request,
                    },
                }
            )
        )
    if any(items[index - 1]["height"] < items[index]["height"] for index in range(1, len(items))):
        raise ValueError("SCCP recent messages must be newest-first")
    return SccpRecentMessages(tuple(items))


def normalize_sccp_message_bundle(value: Any) -> Mapping[str, Any]:
    """Normalize one raw JSON ``NexusSccpMessageProofV1`` bundle."""

    fields = frozenset(
        {"version", "commitment_root", "commitment", "merkle_proof", "payload", "finality_proof"}
    )
    record = _exact_fields(value, fields, "SCCP message bundle")
    _integer(record["version"], "SCCP message bundle.version", 1, 1)
    _lower_hex(record["commitment_root"], "SCCP message bundle.commitment_root", 32, prefix=True)
    _mapping(record["commitment"], "SCCP message bundle.commitment")
    _mapping(record["merkle_proof"], "SCCP message bundle.merkle_proof")
    payload = _exact_fields(
        record["payload"], frozenset({"Transfer"}), "SCCP message bundle.payload"
    )
    _mapping(payload["Transfer"], "SCCP message bundle.payload.Transfer")
    _variable_hex(record["finality_proof"], "SCCP message bundle.finality_proof")
    return _deep_freeze(record)


def _public_inputs(value: Any, label: str) -> Mapping[str, Any]:
    fields = frozenset(
        {
            "version",
            "message_id",
            "payload_hash",
            "target_domain",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
        }
    )
    record = _exact_fields(value, fields, label)
    _integer(record["version"], f"{label}.version", 1, 1)
    for field in ("message_id", "payload_hash", "commitment_root", "finality_block_hash"):
        _lower_hex(record[field], f"{label}.{field}", 32, prefix=True)
    _integer(record["target_domain"], f"{label}.target_domain", 1, 5)
    height = record["finality_height"]
    if not isinstance(height, str) or re.fullmatch(r"[1-9][0-9]*", height) is None:
        raise ValueError(f"{label}.finality_height must be a positive canonical u64 string")
    if int(height) > 0xFFFF_FFFF_FFFF_FFFF:
        raise ValueError(f"{label}.finality_height exceeds u64")
    return record


def normalize_sccp_proof_request(value: Any) -> Mapping[str, Any]:
    """Normalize one query-free raw JSON ``SccpGroth16Bn254ProofRequestV1``."""

    fields = frozenset(
        {
            "version",
            "backend",
            "source_network",
            "target_network",
            "public_inputs",
            "verifying_key",
            "verifier_key_hash",
            "bundle_bytes",
            "statement_hash",
            "destination_binding_hash",
            "route_configuration_hash",
            "request_hash",
        }
    )
    record = _exact_fields(value, fields, "SCCP proof request")
    _integer(record["version"], "SCCP proof request.version", 1, 1)
    backend = _unit_backend(
        record["backend"], "SCCP proof request.backend", "family", _DESTINATION_BACKENDS
    )
    source = _network(record["source_network"], "SCCP proof request.source_network")
    target = _network(record["target_network"], "SCCP proof request.target_network")
    if source[0] != "sora-taira" or target[3]:
        raise ValueError("SCCP proof request must describe an exact Taira-to-external lane")
    if _DESTINATION_BACKENDS[backend] != _family(target):
        raise ValueError("SCCP proof request backend does not match target network")
    inputs = _public_inputs(record["public_inputs"], "SCCP proof request.public_inputs")
    if inputs["target_domain"] != target[2]:
        raise ValueError("SCCP proof request target domain does not match target network")
    key_bytes = _verifying_key(record["verifying_key"], "SCCP proof request.verifying_key")
    hashes = (
        "verifier_key_hash",
        "statement_hash",
        "destination_binding_hash",
        "route_configuration_hash",
        "request_hash",
    )
    for field in hashes:
        _lower_hex(record[field], f"SCCP proof request.{field}", 32, prefix=True)
    if "0x" + _keccak_256(key_bytes).hex() != record["verifier_key_hash"]:
        raise ValueError("SCCP proof request verifier_key_hash does not match verifying_key")
    if len({record[field] for field in hashes}) != len(hashes):
        raise ValueError("SCCP proof request reuses role-separated commitments")
    _variable_hex(record["bundle_bytes"], "SCCP proof request.bundle_bytes")
    return _deep_freeze(record)


def _authority(value: Any, label: str) -> str:
    authority = _text(value, label, 512)
    from .client import _decode_i105_string

    _decode_i105_string(authority)
    return authority


def normalize_bridge_proof_submit_payload(value: Any) -> Dict[str, Any]:
    """Build the sole supported destination-proof submission body."""

    record = _exact_fields(
        value,
        frozenset({"authority", "signature_b64", "destination_proof_b64", "creation_time_ms"}),
        "bridge proof submit",
        frozenset({"authority", "destination_proof_b64"}),
    )
    _canonical_base64(record["destination_proof_b64"], "bridge proof submit.destination_proof_b64")
    result: Dict[str, Any] = {
        "authority": _authority(record["authority"], "bridge proof submit.authority"),
        "destination_proof_b64": record["destination_proof_b64"],
    }
    if "signature_b64" in record:
        _canonical_base64(record["signature_b64"], "bridge proof submit.signature_b64", maximum_bytes=4096)
        result["signature_b64"] = record["signature_b64"]
    if "creation_time_ms" in record:
        result["creation_time_ms"] = _integer(
            record["creation_time_ms"], "bridge proof submit.creation_time_ms", 1
        )
    return result


def normalize_bridge_message_submit_payload(value: Any) -> Dict[str, Any]:
    """Build the sole supported native inbound message submission body."""

    record = _exact_fields(
        value,
        frozenset({"authority", "signature_b64", "native_proof_b64", "creation_time_ms"}),
        "bridge message submit",
        frozenset({"authority", "native_proof_b64"}),
    )
    _canonical_base64(record["native_proof_b64"], "bridge message submit.native_proof_b64")
    result: Dict[str, Any] = {
        "authority": _authority(record["authority"], "bridge message submit.authority"),
        "native_proof_b64": record["native_proof_b64"],
    }
    if "signature_b64" in record:
        _canonical_base64(
            record["signature_b64"], "bridge message submit.signature_b64", maximum_bytes=4096
        )
        result["signature_b64"] = record["signature_b64"]
    if "creation_time_ms" in record:
        result["creation_time_ms"] = _integer(
            record["creation_time_ms"], "bridge message submit.creation_time_ms", 1
        )
    return result


def _iroha_prehash(payload: bytes) -> bytes:
    digest = bytearray(hashlib.blake2b(payload, digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


def normalize_sccp_bridge_submit_response(
    value: Any, expectations: Optional[Mapping[str, Any]] = None
) -> SccpBridgeSubmitResponse:
    """Validate the unified exact prepared-or-submitted bridge response."""

    record = _exact_fields(value, _BRIDGE_RESPONSE_FIELDS, "bridge submit response")
    submitted = _boolean(record["submitted"], "bridge submit response.submitted")
    if record["payload_kind"] != "transfer":
        raise ValueError("bridge submit response.payload_kind must be transfer")
    counterparty = _profile(record["counterparty_chain"], "counterparty_chain")
    domain = _integer(record["counterparty_domain"], "counterparty_domain", 1, 5)
    if counterparty[3] or counterparty[2] != domain:
        raise ValueError("bridge submit response counterparty profile/domain disagree")
    backend = _text(record["backend"], "backend", 128)
    if re.fullmatch(r"bridge/[a-z0-9/_-]+", backend) is None:
        raise ValueError("bridge submit response.backend is not canonical")
    range_start = _integer(record["range_start_height"], "range_start_height", 1)
    range_end = _integer(record["range_end_height"], "range_end_height", range_start)
    creation_time = _integer(record["creation_time_ms"], "creation_time_ms", 1)
    tx_hash = (
        None
        if record["tx_hash_hex"] is None
        else _lower_hex(record["tx_hash_hex"], "tx_hash_hex", 32)
    )
    transaction = (
        None
        if record["transaction_payload_b64"] is None
        else _canonical_base64(record["transaction_payload_b64"], "transaction_payload_b64")
    )
    signing = (
        None
        if record["signing_message_b64"] is None
        else _canonical_base64(
            record["signing_message_b64"], "signing_message_b64", maximum_bytes=32
        )
    )
    if signing is not None and len(signing) != 32:
        raise ValueError("signing_message_b64 must contain exactly 32 bytes")
    if submitted:
        if tx_hash is None or transaction is not None or signing is not None:
            raise ValueError("submitted response must contain only tx_hash_hex signing state")
    elif tx_hash is not None or transaction is None or signing is None:
        raise ValueError("prepared response requires transaction payload and signing message")
    elif _iroha_prehash(transaction) != signing:
        raise ValueError("signing_message_b64 is not the transaction-payload prehash")
    response = SccpBridgeSubmitResponse(
        submitted=submitted,
        payload_kind="transfer",
        message_id_hex=_lower_hex(record["message_id_hex"], "message_id_hex", 32),
        backend=backend,
        counterparty_domain=domain,
        counterparty_chain=counterparty[0],
        manifest_hash_hex=_lower_hex(record["manifest_hash_hex"], "manifest_hash_hex", 32),
        range_start_height=range_start,
        range_end_height=range_end,
        creation_time_ms=creation_time,
        tx_hash_hex=tx_hash,
        transaction_payload_b64=record["transaction_payload_b64"],
        signing_message_b64=record["signing_message_b64"],
    )
    expected = {} if expectations is None else dict(_mapping(expectations, "bridge expectations"))
    _exact_fields(expected, frozenset({"creation_time_ms"}), "bridge expectations", frozenset())
    if expected.get("creation_time_ms") is not None and expected["creation_time_ms"] != creation_time:
        raise ValueError("bridge submit response.creation_time_ms does not match the request")
    return response


def parse_sccp_json_object(
    payload: Union[str, bytes, bytearray, memoryview], label: str = "SCCP response"
) -> Mapping[str, Any]:
    """Parse strict UTF-8 JSON and reject duplicate object keys."""

    if isinstance(payload, str):
        text = payload
    else:
        try:
            text = _binary(payload, label).decode("utf-8", "strict")
        except UnicodeDecodeError as exc:
            raise ValueError(f"{label} must be UTF-8 JSON") from exc
    if not text:
        raise ValueError(f"{label} must be nonempty JSON")

    def unique_object(pairs: Sequence[Tuple[str, Any]]) -> Dict[str, Any]:
        result: Dict[str, Any] = {}
        for key, entry in pairs:
            if key in result:
                raise ValueError(f"{label} contains duplicate field `{key}`")
            result[key] = entry
        return result

    try:
        value = json.loads(text, object_pairs_hook=unique_object)
    except json.JSONDecodeError as exc:
        raise ValueError(f"{label} must be valid JSON") from exc
    return _mapping(value, label)


def parse_sccp_bridge_submit_response_json(
    payload: Union[str, bytes, bytearray, memoryview],
    expectations: Optional[Mapping[str, Any]] = None,
) -> SccpBridgeSubmitResponse:
    """Parse and validate a strict SCCP bridge response."""

    return normalize_sccp_bridge_submit_response(
        parse_sccp_json_object(payload, "bridge submit response"), expectations
    )


__all__ = [
    "SCCP_DOMAIN_SORA",
    "SCCP_DOMAIN_ETH",
    "SCCP_DOMAIN_BSC",
    "SCCP_DOMAIN_TRON",
    "SCCP_CODEC_CANONICAL_TEXT",
    "SCCP_CODEC_EVM_ADDRESS20",
    "SCCP_CODEC_TRON_ADDRESS21",
    "SCCP_CODEC_KEYS",
    "SCCP_PAYLOAD_KINDS",
    "SCCP_NETWORK_PROFILES",
    "SccpCapabilities",
    "SccpRegistry",
    "SccpRecentMessages",
    "SccpBridgeSubmitResponse",
    "normalize_sccp_codec_value",
    "sccp_source_event_digest",
    "normalize_sccp_capabilities",
    "normalize_sccp_registry",
    "normalize_sccp_recent_messages",
    "normalize_sccp_message_bundle",
    "normalize_sccp_proof_request",
    "normalize_bridge_proof_submit_payload",
    "normalize_bridge_message_submit_payload",
    "normalize_sccp_bridge_submit_response",
    "parse_sccp_json_object",
    "parse_sccp_bridge_submit_response_json",
]
