"""Closed first-release SCCP discovery, artifact, and submission helpers."""

from __future__ import annotations

import base64
import hashlib
import json
import re
from dataclasses import dataclass
from types import MappingProxyType
from typing import Any, Dict, Mapping, NoReturn, Optional, Sequence, Tuple, Union

from .norito_frame import validate_norito_frame

SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_ETH = 1
SCCP_DOMAIN_BSC = 2
SCCP_DOMAIN_TRON = 5

SCCP_CODEC_CANONICAL_TEXT = 1
SCCP_CODEC_EVM_ADDRESS20 = 2
SCCP_CODEC_TRON_ADDRESS21 = 5
_SCCP_JSON_SAFE_INTEGER_MAX = (1 << 53) - 1

SCCP_CODEC_KEYS = MappingProxyType(
    {
        SCCP_CODEC_CANONICAL_TEXT: "canonical_text",
        SCCP_CODEC_EVM_ADDRESS20: "evm_address20",
        SCCP_CODEC_TRON_ADDRESS21: "tron_address21",
    }
)
SCCP_PAYLOAD_KINDS = ("transfer",)

_SOURCE_EVENT_PREFIX = b"sccp:source:event:v1"
_LANE_HASH_PREFIX = b"sccp:lane-id:v1"
_EVM_DESTINATION_BINDING_PREFIX = b"iroha:sccp:evm-destination-binding:v1"
_TRON_DESTINATION_BINDING_PREFIX = b"iroha:sccp:tron-destination-binding:v1"
_CONCRETE_ROUTE_CONFIG_PREFIX = b"sccp:concrete-route-config:v1"
_EVM_GROTH16_BACKEND = b"evm-groth16-bn254-v1"
_TRON_GROTH16_BACKEND = b"tron-groth16-bn254-v1"
_SEMANTIC_PROOF_PROFILE_PREFIX = b"sccp:semantic-proof-profile:v1"
_SORA_FINALITY_ANCHOR_PREFIX = b"sccp:sora-finality-anchor:v1"
_PUBLIC_SIGNAL_SCHEMA_HASH = bytes.fromhex(
    "7567439f41173d6745a3d51923cb70371acc7d66f23cefb4100d6d5d7a432cbb"
)
_SORA_TAIRA_CHAIN_ID_HASH = bytes.fromhex(
    "cf1cfc0f57b0bfa4c21882a9870317a1f4812f86533897095e3944be34c5bba7"
)
_SORA_TAIRA_CHAIN_ID = bytes.fromhex("fc56984b2be7431d840e21514d1883f0")
_MAX_WIRE_BYTES = 16 * 1024 * 1024
_MAX_DESTINATION_ARTIFACT_BYTES = _MAX_WIRE_BYTES + 64 * 1024
_MAX_DETACHED_SIGNATURE_BYTES = 16 * 1024
_DESTINATION_ARTIFACT_TYPE_NAME = "iroha_sccp::SccpGroth16Bn254ProofArtifactV1"
_NATIVE_INBOUND_PROOF_TYPE_NAME = (
    "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1"
)
_MAX_U64 = (1 << 64) - 1
_MAX_U128 = (1 << 128) - 1
_CLOSED_DOMAINS = frozenset(
    {SCCP_DOMAIN_SORA, SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON}
)
_BN254_BASE_FIELD_MODULUS = int(
    "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47", 16
)
_ROUTE_KEY = re.compile(r"[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?")

_NETWORKS: Mapping[str, Tuple[int, int, bool]] = MappingProxyType(
    {
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
        "route_configuration_hash_hex",
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
class SccpRegistryLimits:
    """Fixed SCCP V1 route-registry capacities."""

    max_governed_lanes: int
    max_live_governed_routes: int
    max_live_routes_per_lane: int
    max_retained_routes_per_lane: int
    max_retained_native_trust_anchors_per_lane: int


@dataclass(frozen=True)
class SccpResourceLimits:
    """Consensus-critical SCCP proof and deterministic verifier-work limits."""

    max_outbound_messages_per_block: int
    max_outbound_message_payload_bytes: int
    max_pending_outbound_messages: int
    max_pending_outbound_payload_bytes: int
    max_proofs_per_transaction: int
    max_proofs_per_block: int
    max_proof_bytes_per_proof: int
    max_proof_bytes_per_transaction: int
    max_proof_bytes_per_block: int
    max_native_headers_per_transaction: int
    max_native_headers_per_block: int
    max_ethereum_light_client_updates_per_transaction: int
    max_ethereum_light_client_updates_per_block: int
    max_native_header_bytes_per_transaction: int
    max_native_header_bytes_per_block: int
    max_secp256k1_recoveries_per_transaction: int
    max_secp256k1_recoveries_per_block: int
    max_bls_aggregate_checks_per_transaction: int
    max_bls_aggregate_checks_per_block: int
    max_bls_signer_contributions_per_transaction: int
    max_bls_signer_contributions_per_block: int
    max_bn254_pairing_checks_per_transaction: int
    max_bn254_pairing_checks_per_block: int


@dataclass(frozen=True)
class SccpCapabilities:
    """Closed SCCP endpoint capability snapshot."""

    version: int
    registry_revision: str
    registry_path: str
    message_bundle_path: str
    proof_request_path: str
    recent_messages_path: str
    registry_limits: SccpRegistryLimits
    resource_limits: SccpResourceLimits
    proof_submit_path: Optional[str]
    native_message_submit_path: Optional[str]


@dataclass(frozen=True)
class SccpRegistry:
    """Authoritative typed SCCP registry."""

    version: int
    lanes: Tuple[Mapping[str, Any], ...]


@dataclass(frozen=True)
class SccpRecentCursor:
    """Exact compound continuation for newest-first SCCP discovery."""

    from_height: int
    after_index: int


@dataclass(frozen=True)
class SccpRecentMessages:
    """Newest-first SCCP message discovery page."""

    items: Tuple[Mapping[str, Any], ...]
    next: Optional[SccpRecentCursor]


@dataclass(frozen=True)
class SccpBridgeSubmitResponse:
    """Unified prepared-or-submitted SCCP transaction response."""

    submitted: bool
    payload_kind: str
    message_id_hex: str
    backend: str
    counterparty_domain: int
    counterparty_chain: str
    route_configuration_hash_hex: str
    range_start_height: int
    range_end_height: int
    creation_time_ms: int
    tx_hash_hex: Optional[str]
    transaction_payload_b64: Optional[str]
    signing_message_b64: Optional[str]


@dataclass(frozen=True)
class _SccpDestinationDeployment:
    """Parsed destination roles and their exact first-release commitments."""

    family: str
    token_address: bytes
    token_code_hash: bytes
    verifier_address: bytes
    verifier_code_hash: bytes
    verifier_key_hash: bytes
    semantic_profile_hash: bytes
    finality_anchor_hash: bytes
    route_address: bytes
    route_code_hash: bytes
    taira_to_token_multiplier: int
    destination_binding_hash: bytes
    deployment_config_hash: bytes


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


def _protocol_domain(value: Any, label: str) -> int:
    domain = _integer(value, label, SCCP_DOMAIN_SORA, SCCP_DOMAIN_TRON)
    if domain not in _CLOSED_DOMAINS:
        raise ValueError(f"{label} is an unsupported or reserved SCCP domain")
    return domain


def _unsigned_decimal(
    value: Any, label: str, maximum: int, *, positive: bool = False
) -> str:
    pattern = r"[1-9][0-9]*" if positive else r"(?:0|[1-9][0-9]*)"
    if (
        not isinstance(value, str)
        or re.fullmatch(pattern, value) is None
        or int(value) > maximum
    ):
        width = 64 if maximum == _MAX_U64 else 128
        qualifier = "positive " if positive else ""
        raise ValueError(f"{label} must be a canonical {qualifier}u{width} decimal string")
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


def _outbound_lane(
    value: Any, label: str
) -> Tuple[Tuple[str, int, int, bool], Tuple[str, int, int, bool]]:
    record = _exact_fields(value, frozenset({"source", "target"}), label)
    source = _network(record["source"], f"{label}.source")
    target = _network(record["target"], f"{label}.target")
    if source[0] != "sora-taira" or target[3] or source[2] == target[2]:
        raise ValueError(f"{label} must be an exact supported Taira-to-external lane")
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


def _abi_word(value: int) -> bytes:
    return value.to_bytes(32, "big")


def _abi_address(value: bytes) -> bytes:
    return bytes(12) + value


def _abi_tron_address(value: bytes) -> bytes:
    return bytes(11) + b"\x41" + value


def _canonical_network_bytes(network: Tuple[str, int, int, bool]) -> bytes:
    profile, tag, domain, _ = network
    prefix = bytes((1, tag)) + domain.to_bytes(4, "little")
    if profile == "sora-taira":
        identity = _SORA_TAIRA_CHAIN_ID
    elif profile == "ethereum-mainnet":
        identity = (1).to_bytes(8, "little")
    elif profile == "ethereum-sepolia":
        identity = (11_155_111).to_bytes(8, "little")
    elif profile == "bsc-mainnet":
        identity = (56).to_bytes(8, "little")
    elif profile == "bsc-testnet":
        identity = (97).to_bytes(8, "little")
    elif profile == "tron-mainnet":
        identity = (0x2B66_53DC).to_bytes(4, "little")
    elif profile == "tron-nile":
        identity = (0xCD86_90DC).to_bytes(4, "little")
    elif profile == "tron-shasta":
        identity = (0x94A9_059E).to_bytes(4, "little")
    else:
        raise ValueError("SCCP route uses an unsupported exact network")
    return prefix + identity


def _lane_hash(
    source: Tuple[str, int, int, bool], target: Tuple[str, int, int, bool]
) -> bytes:
    source_bytes = _canonical_network_bytes(source)
    target_bytes = _canonical_network_bytes(target)
    canonical = (
        b"\x01"
        + len(source_bytes).to_bytes(4, "little")
        + source_bytes
        + len(target_bytes).to_bytes(4, "little")
        + target_bytes
    )
    return hashlib.blake2b(_LANE_HASH_PREFIX + canonical, digest_size=32).digest()


def _g1(value: Any, label: str) -> Tuple[str, str]:
    record = _exact_fields(value, frozenset({"x", "y"}), label)
    coordinates = tuple(
        _upper_hex(record[field], f"{label}.{field}", 32, nonzero=False)
        for field in ("x", "y")
    )
    if all(set(coordinate) == {"0"} for coordinate in coordinates):
        raise ValueError(f"{label} must not be the BN254 point at infinity")
    for field, coordinate in zip(("x", "y"), coordinates):
        if int(coordinate, 16) >= _BN254_BASE_FIELD_MODULUS:
            raise ValueError(f"{label}.{field} is not a BN254 field element")
    return coordinates  # type: ignore[return-value]


def _g2(value: Any, label: str) -> Tuple[str, str, str, str]:
    fields = ("x_c0", "x_c1", "y_c0", "y_c1")
    record = _exact_fields(value, frozenset(fields), label)
    coordinates = tuple(
        _upper_hex(record[field], f"{label}.{field}", 32, nonzero=False)
        for field in fields
    )
    if all(set(coordinate) == {"0"} for coordinate in coordinates):
        raise ValueError(f"{label} must not be the BN254 point at infinity")
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
        "signal_10",
    )
    ic = _exact_fields(record["ic"], frozenset(ic_fields), f"{label}.ic")
    for field in ic_fields:
        words.extend(_g1(ic[field], f"{label}.ic.{field}"))
    if len(words) != 38:
        raise ValueError(f"{label} must contain exactly 38 ABI words")
    return bytes.fromhex("".join(words))


def _semantic_proof_profile(value: Any, label: str) -> Tuple[bytes, Tuple[bytes, ...]]:
    record = _exact_fields(value, frozenset({"profile", "commitments"}), label)
    profile = _text(record["profile"], f"{label}.profile", 64)
    if profile != "sora_taira_finality_inclusion_groth16_bn254":
        raise ValueError(f"{label}.profile is unsupported or retired")
    commitments = _exact_fields(
        record["commitments"],
        frozenset(
            {
                "version",
                "circuit_commitment",
                "witness_generator_commitment",
                "public_signal_schema_hash",
            }
        ),
        f"{label}.commitments",
    )
    _integer(commitments["version"], f"{label}.commitments.version", 1, 1)
    roles = tuple(
        bytes.fromhex(_upper_hex(commitments[field], f"{label}.commitments.{field}", 32))
        for field in (
            "circuit_commitment",
            "witness_generator_commitment",
            "public_signal_schema_hash",
        )
    )
    if roles[2] != _PUBLIC_SIGNAL_SCHEMA_HASH:
        raise ValueError(f"{label} does not commit the exact eleven-signal schema")
    if len(set(roles)) != len(roles):
        raise ValueError(f"{label} reuses a semantic commitment role")
    canonical = b"\x01\x00\x01" + b"".join(roles)
    return _keccak_256(_SEMANTIC_PROOF_PROFILE_PREFIX + canonical), roles


def _sora_finality_anchor(value: Any, label: str) -> Tuple[bytes, Tuple[bytes, ...]]:
    record = _exact_fields(
        value,
        frozenset(
            {
                "version",
                "source_network",
                "protocol_version",
                "chain_id_hash",
                "checkpoint_height",
                "checkpoint_block_hash",
                "checkpoint_context_id",
                "checkpoint_finality_artifact_hash",
            }
        ),
        label,
    )
    _integer(record["version"], f"{label}.version", 1, 1)
    source = _network(record["source_network"], f"{label}.source_network")
    if source[0] != "sora-taira":
        raise ValueError(f"{label}.source_network must be SORA Taira")
    protocol_version = _integer(
        record["protocol_version"], f"{label}.protocol_version", 3, 4
    )
    chain_hash = bytes.fromhex(_upper_hex(record["chain_id_hash"], f"{label}.chain_id_hash", 32))
    if chain_hash != _SORA_TAIRA_CHAIN_ID_HASH:
        raise ValueError(f"{label}.chain_id_hash is not the Taira chain commitment")
    checkpoint_height = _integer(
        record["checkpoint_height"], f"{label}.checkpoint_height", 1, _U64_MASK
    )
    checkpoint_hash = bytes.fromhex(
        _upper_hex(record["checkpoint_block_hash"], f"{label}.checkpoint_block_hash", 32)
    )
    context_id = bytes.fromhex(
        _upper_hex(record["checkpoint_context_id"], f"{label}.checkpoint_context_id", 32)
    )
    finality_artifact_hash = bytes.fromhex(
        _upper_hex(
            record["checkpoint_finality_artifact_hash"],
            f"{label}.checkpoint_finality_artifact_hash",
            32,
        )
    )
    roles = (chain_hash, checkpoint_hash, context_id, finality_artifact_hash)
    if len(set(roles)) != len(roles):
        raise ValueError(f"{label} reuses a consensus hash role")
    canonical = (
        b"\x01\x01"
        + protocol_version.to_bytes(2, "little")
        + chain_hash
        + checkpoint_height.to_bytes(8, "little")
        + checkpoint_hash
        + context_id
        + finality_artifact_hash
    )
    return _keccak_256(_SORA_FINALITY_ANCHOR_PREFIX + canonical), roles


def _validate_proof_policy_roles(
    semantic_hash: bytes,
    semantic_roles: Tuple[bytes, ...],
    anchor_hash: bytes,
    anchor_roles: Tuple[bytes, ...],
    label: str,
) -> None:
    roles = (*semantic_roles, semantic_hash, *anchor_roles, anchor_hash)
    if any(not any(role) for role in roles) or len(set(roles)) != len(roles):
        raise ValueError(f"{label} reuses a proof-policy hash role")


def _outbound_proof_policy(value: Any, label: str) -> Tuple[bytes, bytes]:
    record = _exact_fields(
        value,
        frozenset({"version", "semantic_profile", "sora_finality_anchor"}),
        label,
    )
    _integer(record["version"], f"{label}.version", 1, 1)
    semantic_hash, semantic_roles = _semantic_proof_profile(
        record["semantic_profile"], f"{label}.semantic_profile"
    )
    anchor_hash, anchor_roles = _sora_finality_anchor(
        record["sora_finality_anchor"], f"{label}.sora_finality_anchor"
    )
    _validate_proof_policy_roles(
        semantic_hash, semantic_roles, anchor_hash, anchor_roles, label
    )
    return semantic_hash, anchor_hash


def normalize_sccp_codec_value(
    codec: int, value: Union[str, bytes, bytearray, memoryview]
) -> bytes:
    """Validate and normalize one closed SCCP V1 codec value."""

    if codec not in SCCP_CODEC_KEYS:
        raise ValueError("codec is unsupported or retired")
    if codec == SCCP_CODEC_CANONICAL_TEXT:
        text = _text(value, "canonical_text", 256)
        encoded = text.encode("utf-8")
        if re.fullmatch(r"[\x21-\x7e]+", text) is None:
            from .client import _decode_canonical_i105_string

            try:
                _decode_canonical_i105_string(text)
            except ValueError as exc:
                raise ValueError(
                    "canonical_text must contain printable ASCII or an exact canonical I105 account address"
                ) from exc
        return encoded
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


def _normalize_registry_limits(value: Any) -> SccpRegistryLimits:
    fields = frozenset(
        {
            "max_governed_lanes",
            "max_live_governed_routes",
            "max_live_routes_per_lane",
            "max_retained_routes_per_lane",
            "max_retained_native_trust_anchors_per_lane",
        }
    )
    record = _exact_fields(value, fields, "SCCP registry limits")
    limits = SccpRegistryLimits(
        **{
            field: _integer(record[field], f"SCCP registry limits.{field}", 1, (1 << 32) - 1)
            for field in fields
        }
    )
    if limits != SccpRegistryLimits(16, 64, 8, 64, 4_096):
        raise ValueError("SCCP registry limits must equal the fixed V1 capacities")
    return limits


def _normalize_resource_limits(value: Any) -> SccpResourceLimits:
    count_fields = frozenset(
        {
            "max_outbound_messages_per_block",
            "max_proofs_per_transaction",
            "max_proofs_per_block",
            "max_native_headers_per_transaction",
            "max_native_headers_per_block",
            "max_ethereum_light_client_updates_per_transaction",
            "max_ethereum_light_client_updates_per_block",
            "max_secp256k1_recoveries_per_transaction",
            "max_secp256k1_recoveries_per_block",
            "max_bls_aggregate_checks_per_transaction",
            "max_bls_aggregate_checks_per_block",
            "max_bls_signer_contributions_per_transaction",
            "max_bls_signer_contributions_per_block",
            "max_bn254_pairing_checks_per_transaction",
            "max_bn254_pairing_checks_per_block",
        }
    )
    byte_fields = frozenset(
        {
            "max_outbound_message_payload_bytes",
            "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block",
            "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block",
        }
    )
    json_safe_fields = frozenset(
        {
            "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes",
        }
    )
    fields = count_fields | byte_fields | json_safe_fields
    record = _exact_fields(value, fields, "SCCP resource limits")
    parsed = {
        field: _integer(
            record[field],
            f"SCCP resource limits.{field}",
            1,
            (1 << 32) - 1 if field in count_fields else _SCCP_JSON_SAFE_INTEGER_MAX,
        )
        for field in fields
    }
    limits = SccpResourceLimits(**parsed)
    if (
        limits.max_outbound_messages_per_block != 512
        or limits.max_outbound_message_payload_bytes != 4_096
    ):
        raise ValueError(
            "SCCP fixed outbound message limits must equal 512 messages and 4,096 payload bytes"
        )
    if limits.max_proof_bytes_per_proof > limits.max_proof_bytes_per_transaction:
        raise ValueError("SCCP per-proof byte limit exceeds its transaction limit")
    ordered_pairs = (
        (limits.max_proofs_per_transaction, limits.max_proofs_per_block),
        (limits.max_proof_bytes_per_transaction, limits.max_proof_bytes_per_block),
        (limits.max_native_headers_per_transaction, limits.max_native_headers_per_block),
        (
            limits.max_ethereum_light_client_updates_per_transaction,
            limits.max_ethereum_light_client_updates_per_block,
        ),
        (
            limits.max_native_header_bytes_per_transaction,
            limits.max_native_header_bytes_per_block,
        ),
        (
            limits.max_secp256k1_recoveries_per_transaction,
            limits.max_secp256k1_recoveries_per_block,
        ),
        (
            limits.max_bls_aggregate_checks_per_transaction,
            limits.max_bls_aggregate_checks_per_block,
        ),
        (
            limits.max_bls_signer_contributions_per_transaction,
            limits.max_bls_signer_contributions_per_block,
        ),
        (
            limits.max_bn254_pairing_checks_per_transaction,
            limits.max_bn254_pairing_checks_per_block,
        ),
    )
    if any(transaction > block for transaction, block in ordered_pairs):
        raise ValueError("SCCP transaction resource limits must not exceed block limits")
    return limits


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
            "registry_limits",
            "resource_limits",
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
            "registry_limits",
            "resource_limits",
        }
    )
    record = _exact_fields(value, allowed, "SCCP capabilities", required)
    proof_submit_path = _capability_path(
        record.get("proof_submit_path"), "proof_submit_path", optional=True
    )
    native_message_submit_path = _capability_path(
        record.get("native_message_submit_path"),
        "native_message_submit_path",
        optional=True,
    )
    if (proof_submit_path is None) != (native_message_submit_path is None):
        raise ValueError(
            "SCCP capabilities must advertise proof and native-message submit paths together"
        )
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
        registry_limits=_normalize_registry_limits(record["registry_limits"]),
        resource_limits=_normalize_resource_limits(record["resource_limits"]),
        proof_submit_path=proof_submit_path,
        native_message_submit_path=native_message_submit_path,
    )


def _native_anchor(
    value: Any, lane: Any, label: str
) -> Optional[Tuple[str, str, int]]:
    if value is None:
        return None
    record = _exact_fields(
        value, frozenset({"backend", "anchor_hash", "checkpoint_height"}), label
    )
    backend = _unit_backend(record["backend"], f"{label}.backend", "protocol", _NATIVE_BACKENDS)
    if lane[0][0] not in _NATIVE_BACKENDS[backend]:
        raise ValueError(f"{label}.backend does not match the lane source")
    anchor_hash = _upper_hex(record["anchor_hash"], f"{label}.anchor_hash", 32)
    checkpoint_height = _integer(
        record["checkpoint_height"], f"{label}.checkpoint_height", 1
    )
    return backend, anchor_hash, checkpoint_height


def _activation(value: Any, label: str) -> str:
    record = _exact_fields(value, frozenset({"activation", "direction"}), label)
    if record["direction"] is not None:
        raise ValueError(f"{label}.direction must be null")
    activation = _text(record["activation"], f"{label}.activation", 32)
    if activation not in {"staged", "bidirectional", "inbound_only", "paused", "retired"}:
        raise ValueError(f"{label}.activation is unsupported")
    return activation


def _inbound_finality_cutoff(
    value: Any, activation: str, label: str
) -> Optional[Tuple[str, int]]:
    if value is None:
        if activation == "retired":
            raise ValueError(f"{label} is required for a retired SCCP route")
        return None
    if activation != "retired":
        raise ValueError(f"{label} is allowed only for a retired SCCP route")
    record = _exact_fields(
        value,
        frozenset({"trust_anchor_hash", "max_anchor_interval_height"}),
        label,
    )
    return (
        _upper_hex(record["trust_anchor_hash"], f"{label}.trust_anchor_hash", 32),
        _integer(
            record["max_anchor_interval_height"],
            f"{label}.max_anchor_interval_height",
            1,
        ),
    )


def _source_identity(value: Any, lane: Any, label: str) -> Tuple[str, bytes, bytes, bytes]:
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
    address = bytes.fromhex(
        _upper_hex(identity["address"], f"{label}.emitter.identity.address", 20)
    )
    runtime = bytes.fromhex(
        _upper_hex(
            identity["runtime_code_hash"], f"{label}.emitter.identity.runtime_code_hash", 32
        )
    )
    configuration = bytes.fromhex(
        _upper_hex(
            identity["route_config_hash"], f"{label}.emitter.identity.route_config_hash", 32
        )
    )
    if runtime == configuration:
        raise ValueError(f"{label} runtime and route-configuration hashes must be distinct")
    return family, address, runtime, configuration


def _destination_binding_hash(
    network: Tuple[str, int, int, bool], destination: _SccpDestinationDeployment
) -> bytes:
    profile, _, target_domain, _ = network
    if destination.family == "tron":
        network_values = {
            "tron-mainnet": 0x2B66_53DC,
            "tron-nile": 0xCD86_90DC,
            "tron-shasta": 0x94A9_059E,
        }
        try:
            network_value = network_values[profile]
        except KeyError as exc:
            raise ValueError("TRON destination binding requires a TRON lane") from exc
        binding_prefix = _TRON_DESTINATION_BINDING_PREFIX
        backend = _TRON_GROTH16_BACKEND
        verifier_address = _abi_tron_address(destination.verifier_address)
        route_address = _abi_tron_address(destination.route_address)
    else:
        network_values = {
            "ethereum-mainnet": 1,
            "ethereum-sepolia": 11_155_111,
            "bsc-mainnet": 56,
            "bsc-testnet": 97,
        }
        try:
            network_value = network_values[profile]
        except KeyError as exc:
            raise ValueError("EVM destination binding requires an EVM lane") from exc
        binding_prefix = _EVM_DESTINATION_BINDING_PREFIX
        backend = _EVM_GROTH16_BACKEND
        verifier_address = _abi_address(destination.verifier_address)
        route_address = _abi_address(destination.route_address)
    payload = b"".join(
        (
            _keccak_256(binding_prefix),
            _keccak_256(backend),
            _abi_word(network_value),
            _abi_word(SCCP_DOMAIN_SORA),
            _abi_word(target_domain),
            verifier_address,
            route_address,
            destination.verifier_code_hash,
            destination.verifier_key_hash,
            destination.semantic_profile_hash,
            destination.finality_anchor_hash,
        )
    )
    return _keccak_256(payload)


def _destination(value: Any, lane: Any, label: str) -> _SccpDestinationDeployment:
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
            "outbound_proof_policy",
            "route_address",
            "route_code_hash",
            "taira_to_token_multiplier",
        }
    )
    deployment = _exact_fields(record["deployment"], fields, f"{label}.deployment")
    addresses = tuple(
        bytes.fromhex(_upper_hex(deployment[field], f"{label}.deployment.{field}", 20))
        for field in ("token_address", "verifier_address", "route_address")
    )
    hashes = tuple(
        bytes.fromhex(_upper_hex(deployment[field], f"{label}.deployment.{field}", 32))
        for field in ("token_code_hash", "verifier_code_hash", "verifier_key_hash", "route_code_hash")
    )
    if len(set(addresses)) != len(addresses) or len(set(hashes)) != len(hashes):
        raise ValueError(f"{label}.deployment reuses a role-separated address or hash")
    key_bytes = _verifying_key(deployment["verifying_key"], f"{label}.deployment.verifying_key")
    if _keccak_256(key_bytes) != hashes[2]:
        raise ValueError(f"{label}.deployment.verifier_key_hash does not match verifying_key")
    semantic_hash, anchor_hash = _outbound_proof_policy(
        deployment["outbound_proof_policy"], f"{label}.deployment.outbound_proof_policy"
    )
    deployment_hashes = (*hashes, semantic_hash, anchor_hash)
    if len(set(deployment_hashes)) != len(deployment_hashes):
        raise ValueError(f"{label}.deployment reuses a role-separated policy or code hash")
    multiplier = _integer(
        deployment["taira_to_token_multiplier"],
        f"{label}.deployment.taira_to_token_multiplier",
        1_000_000_000,
        1_000_000_000,
    )
    partial = _SccpDestinationDeployment(
        family=family,
        token_address=addresses[0],
        token_code_hash=hashes[0],
        verifier_address=addresses[1],
        verifier_code_hash=hashes[1],
        verifier_key_hash=hashes[2],
        semantic_profile_hash=semantic_hash,
        finality_anchor_hash=anchor_hash,
        route_address=addresses[2],
        route_code_hash=hashes[3],
        taira_to_token_multiplier=multiplier,
        destination_binding_hash=b"",
        deployment_config_hash=b"",
    )
    destination_binding_hash = _destination_binding_hash(lane[0], partial)
    deployment_config = b"".join(
        (
            _abi_address(partial.token_address),
            partial.token_code_hash,
            _abi_address(partial.verifier_address),
            partial.verifier_code_hash,
            partial.verifier_key_hash,
            partial.semantic_profile_hash,
            partial.finality_anchor_hash,
            destination_binding_hash if family == "tron" else b"",
        )
    )
    return _SccpDestinationDeployment(
        family=partial.family,
        token_address=partial.token_address,
        token_code_hash=partial.token_code_hash,
        verifier_address=partial.verifier_address,
        verifier_code_hash=partial.verifier_code_hash,
        verifier_key_hash=partial.verifier_key_hash,
        semantic_profile_hash=partial.semantic_profile_hash,
        finality_anchor_hash=partial.finality_anchor_hash,
        route_address=partial.route_address,
        route_code_hash=partial.route_code_hash,
        taira_to_token_multiplier=partial.taira_to_token_multiplier,
        destination_binding_hash=destination_binding_hash,
        deployment_config_hash=_keccak_256(deployment_config),
    )


def _settlement(value: Any, label: str) -> None:
    record = _exact_fields(
        value,
        frozenset({"asset_definition_id", "custody_account_id", "payload_amount_scale"}),
        label,
    )
    asset_definition_id = _text(
        record["asset_definition_id"], f"{label}.asset_definition_id", 512
    )
    if asset_definition_id != "6TEAJqbb8oEPmLncoNiMRbLEK6tw":
        raise ValueError(f"{label}.asset_definition_id must be canonical Taira XOR")
    authority = _text(record["custody_account_id"], f"{label}.custody_account_id", 512)
    from .client import _decode_canonical_i105_string

    _decode_canonical_i105_string(authority)
    _integer(record["payload_amount_scale"], f"{label}.payload_amount_scale", 9, 9)


def _route_configuration_hash(
    lane: Any,
    route_id: str,
    asset_key: str,
    revision: int,
    destination: _SccpDestinationDeployment,
) -> bytes:
    if asset_key != "xor":
        raise ValueError("SCCP V1 route asset must be xor")
    profile, network_tag, domain, _ = lane[0]
    network_values = {
        "ethereum-mainnet": ("taira_eth_xor", 1),
        "ethereum-sepolia": ("taira_eth_xor", 11_155_111),
        "bsc-mainnet": ("taira_bsc_xor", 56),
        "bsc-testnet": ("taira_bsc_xor", 97),
        "tron-mainnet": ("taira_tron_xor", 0x2B66_53DC),
        "tron-nile": ("taira_tron_xor", 0xCD86_90DC),
        "tron-shasta": ("taira_tron_xor", 0x94A9_059E),
    }
    try:
        expected_route_id, network_value = network_values[profile]
    except KeyError as exc:
        raise ValueError("SCCP route uses an unsupported external profile") from exc
    if route_id != expected_route_id:
        raise ValueError("SCCP route id does not match its exact deployment")

    source_lane_hash = _lane_hash(lane[0], lane[1])
    destination_lane_hash = _lane_hash(lane[1], lane[0])
    hash_roles = (
        source_lane_hash,
        destination_lane_hash,
        destination.token_code_hash,
        destination.verifier_code_hash,
        destination.verifier_key_hash,
        destination.semantic_profile_hash,
        destination.finality_anchor_hash,
    ) + ((destination.destination_binding_hash,) if destination.family == "tron" else ())
    if any(not any(role) for role in hash_roles) or len(set(hash_roles)) != len(hash_roles):
        raise ValueError("SCCP route reuses a role-separated hash")

    asset_route_config_hash = _keccak_256(
        b"".join(
            (
                _keccak_256(b"xor"),
                _keccak_256(route_id.encode("ascii")),
                _abi_word(revision),
                _abi_word(destination.taira_to_token_multiplier),
            )
        )
    )
    return _keccak_256(
        b"".join(
            (
                _keccak_256(_CONCRETE_ROUTE_CONFIG_PREFIX),
                _abi_word(domain),
                _abi_word(network_tag),
                _abi_word(network_value),
                source_lane_hash,
                destination_lane_hash,
                destination.deployment_config_hash,
                asset_route_config_hash,
            )
        )
    )


def _route(
    value: Any, lane: Any, native_anchor: Optional[str], label: str
) -> Tuple[str, str, int, str, Optional[Tuple[str, int]]]:
    fields = frozenset(
        {
            "lane_id",
            "route_id",
            "asset_key",
            "revision",
            "activation",
            "inbound_finality_cutoff",
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
    inbound_finality_cutoff = _inbound_finality_cutoff(
        record["inbound_finality_cutoff"],
        activation,
        f"{label}.inbound_finality_cutoff",
    )
    if activation in {"bidirectional", "inbound_only"} and native_anchor is None:
        raise ValueError(f"{label} enables inbound settlement without a native trust anchor")
    source = _source_identity(record["source_identity"], lane, f"{label}.source_identity")
    destination = _destination(record["destination"], lane, f"{label}.destination")
    if (
        source[0] != destination.family
        or source[1] != destination.route_address
        or source[2] != destination.route_code_hash
    ):
        raise ValueError(f"{label} source emitter does not identify the destination route")
    route_configuration_hash = _route_configuration_hash(
        lane, record["route_id"], record["asset_key"], revision, destination
    )
    if source[3] != route_configuration_hash:
        raise ValueError(
            f"{label} source route_config_hash does not match the immutable deployment"
        )
    _settlement(record["settlement"], f"{label}.settlement")
    lineage = f"{record['route_id']}\x00{record['asset_key']}"
    key = f"{lane[0][0]}\x00{lane[1][0]}\x00{lineage}\x00{revision}"
    return lineage, key, revision, activation, inbound_finality_cutoff


def normalize_sccp_registry(value: Any) -> SccpRegistry:
    """Validate the authoritative typed registry without treating it as a manifest."""

    record = _exact_fields(value, frozenset({"version", "lanes"}), "SCCP registry")
    version = _integer(record["version"], "SCCP registry.version", 1, 1)
    lanes = _list(record["lanes"], "SCCP registry.lanes")
    if len(lanes) > 16:
        raise ValueError("SCCP registry contains more than 16 lanes")
    lane_keys: set[Tuple[str, str]] = set()
    route_keys: set[str] = set()
    live_route_count = 0
    for lane_index, entry in enumerate(lanes):
        label = f"SCCP registry.lanes[{lane_index}]"
        lane_record = _exact_fields(
            entry,
            frozenset(
                {
                    "lane_id",
                    "native_trust_anchors",
                    "current_native_trust_anchor_hash",
                    "routes",
                }
            ),
            label,
        )
        lane = _lane(lane_record["lane_id"], f"{label}.lane_id")
        lane_key = (lane[0][0], lane[1][0])
        if lane_key in lane_keys:
            raise ValueError("SCCP registry contains a duplicate lane")
        lane_keys.add(lane_key)
        anchor_values = _list(
            lane_record["native_trust_anchors"], f"{label}.native_trust_anchors"
        )
        if len(anchor_values) > 4_096:
            raise ValueError(
                f"{label} contains more than 4,096 retained native trust anchors"
            )
        native_anchors: list[Tuple[str, str, int]] = []
        anchor_hashes: set[str] = set()
        for anchor_index, anchor_value in enumerate(anchor_values):
            anchor_label = f"{label}.native_trust_anchors[{anchor_index}]"
            anchor = _native_anchor(anchor_value, lane, anchor_label)
            if anchor is None:
                raise ValueError(f"{anchor_label} must not be null")
            if anchor[1] in anchor_hashes:
                raise ValueError(f"{label} contains a duplicate native trust-anchor hash")
            if native_anchors and (
                anchor[0] != native_anchors[-1][0]
                or anchor[2] <= native_anchors[-1][2]
            ):
                raise ValueError(
                    f"{label}.native_trust_anchors must advance monotonically within one backend"
                )
            anchor_hashes.add(anchor[1])
            native_anchors.append(anchor)
        current_value = lane_record["current_native_trust_anchor_hash"]
        current_anchor_hash = (
            None
            if current_value is None
            else _upper_hex(
                current_value, f"{label}.current_native_trust_anchor_hash", 32
            )
        )
        expected_current_anchor_hash = native_anchors[-1][1] if native_anchors else None
        if current_anchor_hash != expected_current_anchor_hash:
            raise ValueError(
                f"{label}.current_native_trust_anchor_hash must name the last retained anchor"
            )
        native_anchor = native_anchors[-1][0] if native_anchors else None
        routes = _list(lane_record["routes"], f"{label}.routes")
        if not routes:
            raise ValueError(f"{label}.routes must contain at least one route")
        if len(routes) > 64:
            raise ValueError(f"{label} contains more than 64 retained route revisions")
        lineages: Dict[str, list[Tuple[int, str]]] = {}
        lane_live_route_count = 0
        for route_index, route_value in enumerate(routes):
            lineage, route_key, revision, activation, cutoff = _route(
                route_value, lane, native_anchor, f"{label}.routes[{route_index}]"
            )
            if route_key in route_keys:
                raise ValueError("SCCP registry contains a duplicate route")
            route_keys.add(route_key)
            if activation != "retired":
                lane_live_route_count += 1
                live_route_count += 1
            if cutoff is not None:
                anchor_index = next(
                    (
                        index
                        for index, anchor in enumerate(native_anchors)
                        if anchor[1] == cutoff[0]
                    ),
                    None,
                )
                if (
                    anchor_index is None
                    or anchor_index + 1 >= len(native_anchors)
                    or native_anchors[anchor_index + 1][2] != cutoff[1]
                ):
                    raise ValueError(
                        f"{label}.routes[{route_index}].inbound_finality_cutoff "
                        "must close one complete retained anchor interval"
                    )
            lineages.setdefault(lineage, []).append((revision, activation))
        for revisions in lineages.values():
            revisions.sort()
            if [revision for revision, _ in revisions] != list(range(1, len(revisions) + 1)):
                raise ValueError("SCCP route revisions must start at one and contain no gaps")
            if sum(activation == "bidirectional" for _, activation in revisions) > 1:
                raise ValueError("SCCP registry enables multiple revisions of one route")
        if lane_live_route_count > 8:
            raise ValueError(f"{label} contains more than 8 live routes")
    if live_route_count > 64:
        raise ValueError("SCCP registry contains more than 64 live routes")
    frozen = _deep_freeze(record)
    return SccpRegistry(version=version, lanes=tuple(frozen["lanes"]))


def _projection_text(value: Any, label: str) -> str:
    tagged = _exact_fields(value, frozenset({"CanonicalText"}), label)
    payload = _exact_fields(
        tagged["CanonicalText"], frozenset({"value"}), f"{label}.CanonicalText"
    )
    return _text(payload["value"], f"{label}.CanonicalText.value", 512)


def _projection_recipient(value: Any, domain: int, label: str) -> None:
    tag = "TronAddress21" if domain == SCCP_DOMAIN_TRON else "EvmAddress20"
    byte_length = 21 if domain == SCCP_DOMAIN_TRON else 20
    tagged = _exact_fields(value, frozenset({tag}), label)
    payload = _exact_fields(tagged[tag], frozenset({"bytes"}), f"{label}.{tag}")
    address = _lower_hex(
        payload["bytes"], f"{label}.{tag}.bytes", byte_length, prefix=True
    )
    if domain == SCCP_DOMAIN_TRON and not address.startswith("0x41"):
        raise ValueError(f"{label}.TronAddress21.bytes must use the canonical 0x41 prefix")


def _payload_projection(value: Any, expected_domain: int, label: str) -> Any:
    tagged = _exact_fields(value, frozenset({"Transfer"}), label)
    transfer = _exact_fields(
        tagged["Transfer"],
        frozenset(
            {
                "version",
                "source_domain",
                "dest_domain",
                "nonce",
                "route_revision",
                "asset_home_domain",
                "asset_id",
                "amount",
                "sender",
                "recipient",
                "route_id",
            }
        ),
        f"{label}.Transfer",
    )
    _integer(transfer["version"], f"{label}.Transfer.version", 1, 1)
    _integer(
        transfer["source_domain"],
        f"{label}.Transfer.source_domain",
        SCCP_DOMAIN_SORA,
        SCCP_DOMAIN_SORA,
    )
    domain = _protocol_domain(transfer["dest_domain"], f"{label}.Transfer.dest_domain")
    if domain != expected_domain or domain == SCCP_DOMAIN_SORA:
        raise ValueError(f"{label}.Transfer.dest_domain does not match the discovery record")
    _integer(transfer["nonce"], f"{label}.Transfer.nonce", 0, _MAX_U64)
    _integer(
        transfer["route_revision"],
        f"{label}.Transfer.route_revision",
        1,
        0xFFFF_FFFF,
    )
    _integer(
        transfer["asset_home_domain"],
        f"{label}.Transfer.asset_home_domain",
        SCCP_DOMAIN_SORA,
        SCCP_DOMAIN_SORA,
    )
    if _projection_text(transfer["asset_id"], f"{label}.Transfer.asset_id") != "xor":
        raise ValueError(f"{label}.Transfer.asset_id must be canonical XOR")
    _integer(transfer["amount"], f"{label}.Transfer.amount", 1, _MAX_U128)
    _projection_text(transfer["sender"], f"{label}.Transfer.sender")
    _projection_recipient(transfer["recipient"], domain, f"{label}.Transfer.recipient")
    route_id = _projection_text(transfer["route_id"], f"{label}.Transfer.route_id")
    expected_route = {
        SCCP_DOMAIN_ETH: "taira_eth_xor",
        SCCP_DOMAIN_BSC: "taira_bsc_xor",
        SCCP_DOMAIN_TRON: "taira_tron_xor",
    }[domain]
    if route_id != expected_route:
        raise ValueError(f"{label}.Transfer.route_id does not match its destination domain")
    return _deep_freeze(tagged)


def normalize_sccp_recent_messages(value: Any) -> SccpRecentMessages:
    """Normalize newest-first discovery with only bundle and proof-request links."""

    root = _exact_fields(
        value,
        frozenset({"items", "next"}),
        "SCCP recent messages",
        frozenset({"items"}),
    )
    items = []
    message_ids = set()
    allowed = frozenset(
        {
            "height",
            "commitment_index",
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
            "commitment_index",
            "message_id_hex",
            "kind",
            "source_profile",
            "target_profile",
            "destination_binding_hash",
            "route_configuration_hash",
            "target_domain",
            "amount",
            "payload_projection",
            "links",
        }
    )
    raw_items = _list(root["items"], "SCCP recent messages.items")
    if len(raw_items) > 50:
        raise ValueError("SCCP recent messages must contain at most 50 items")
    for index, entry in enumerate(raw_items):
        label = f"SCCP recent messages.items[{index}]"
        record = _exact_fields(entry, allowed, label, required)
        source = _profile(record["source_profile"], f"{label}.source_profile")
        target = _profile(record["target_profile"], f"{label}.target_profile")
        if source[0] != "sora-taira" or target[3] or record["kind"] != "transfer":
            raise ValueError(f"{label} must describe a Taira-origin external transfer")
        message_id = _lower_hex(record["message_id_hex"], f"{label}.message_id_hex", 32)
        if message_id in message_ids:
            raise ValueError("SCCP recent messages contain duplicate message ids")
        message_ids.add(message_id)
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

        amount = _unsigned_decimal(record["amount"], f"{label}.amount", _MAX_U128, positive=True)
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
        payload_projection = _payload_projection(
            record["payload_projection"], target[2], f"{label}.payload_projection"
        )
        asset_id = optional_text("asset_id")
        route_id = optional_text("route_id")
        recipient = optional_text("recipient")
        transfer_projection = payload_projection["Transfer"]
        if (
            (
                asset_id is not None
                and asset_id != transfer_projection["asset_id"]["CanonicalText"]["value"]
            )
            or (
                route_id is not None
                and route_id != transfer_projection["route_id"]["CanonicalText"]["value"]
            )
            or recipient is not None
            or amount != str(transfer_projection["amount"])
        ):
            raise ValueError(f"{label} summary fields disagree with payload_projection")
        items.append(
            _deep_freeze(
                {
                    "height": _integer(record["height"], f"{label}.height", 1, _MAX_U64),
                    "commitment_index": _integer(
                        record["commitment_index"], f"{label}.commitment_index", 0, 511
                    ),
                    "message_id_hex": message_id,
                    "kind": "transfer",
                    "source_profile": source[0],
                    "target_profile": target[0],
                    "destination_binding_hash": destination_binding_hash,
                    "route_configuration_hash": route_configuration_hash,
                    "target_domain": target[2],
                    "asset_id": asset_id,
                    "route_id": route_id,
                    "recipient": recipient,
                    "amount": amount,
                    "payload_projection": payload_projection,
                    "links": {
                        "bundle_path": expected_bundle,
                        "proof_request_path": expected_request,
                    },
                }
            )
        )
    for index in range(1, len(items)):
        previous = items[index - 1]
        current = items[index]
        if current["height"] > previous["height"]:
            raise ValueError("SCCP recent messages must be newest-first")
        if current["height"] == previous["height"]:
            if current["commitment_index"] != previous["commitment_index"] + 1:
                raise ValueError(
                    "same-height SCCP recent messages must have contiguous ascending commitment indices"
                )
        elif current["commitment_index"] != 0:
            raise ValueError("an older SCCP block must begin at commitment index zero")
    cursor = None
    if "next" in root:
        next_record = _exact_fields(
            root["next"],
            frozenset({"from", "after_index"}),
            "SCCP recent messages.next",
        )
        cursor = SccpRecentCursor(
            from_height=_integer(
                next_record["from"], "SCCP recent messages.next.from", 1, _MAX_U64
            ),
            after_index=_integer(
                next_record["after_index"],
                "SCCP recent messages.next.after_index",
                0,
                511,
            ),
        )
        if not items:
            raise ValueError("an empty SCCP recent page must not advertise a continuation")
        if (
            cursor.from_height != items[-1]["height"]
            or cursor.after_index != items[-1]["commitment_index"]
        ):
            raise ValueError("SCCP recent continuation must identify the last returned item")
    return SccpRecentMessages(tuple(items), cursor)


def _validate_codec_value(
    record: Mapping[str, Any], codec_field: str, value_field: str, domain: Optional[int] = None
) -> None:
    codec = _integer(record[codec_field], f"SCCP transfer.{codec_field}", 1, 5)
    if codec not in SCCP_CODEC_KEYS:
        raise ValueError(f"SCCP transfer.{codec_field} is unsupported or retired")
    if domain is not None:
        expected = (
            SCCP_CODEC_CANONICAL_TEXT
            if domain == SCCP_DOMAIN_SORA
            else SCCP_CODEC_TRON_ADDRESS21
            if domain == SCCP_DOMAIN_TRON
            else SCCP_CODEC_EVM_ADDRESS20
        )
        if codec != expected:
            raise ValueError(f"SCCP transfer.{codec_field} does not match its protocol domain")
    encoded = _variable_hex(
        record[value_field], f"SCCP transfer.{value_field}", maximum_bytes=256
    )
    value = bytes.fromhex(encoded[2:])
    valid = (
        codec == SCCP_CODEC_CANONICAL_TEXT
        and len(value) <= 256
        and all(0x21 <= byte <= 0x7E for byte in value)
    ) or (
        codec == SCCP_CODEC_EVM_ADDRESS20 and len(value) == 20 and any(value)
    ) or (
        codec == SCCP_CODEC_TRON_ADDRESS21
        and len(value) == 21
        and value[0] == 0x41
        and any(value[1:])
    )
    if not valid:
        raise ValueError(f"SCCP transfer.{value_field} does not match its codec")


def _validate_transfer(
    value: Any,
    lane: Tuple[Tuple[str, int, int, bool], Tuple[str, int, int, bool]],
) -> None:
    fields = frozenset(
        {
            "version",
            "source_domain",
            "dest_domain",
            "nonce",
            "route_revision",
            "asset_home_domain",
            "asset_id_codec",
            "asset_id",
            "amount",
            "sender_codec",
            "sender",
            "recipient_codec",
            "recipient",
            "route_id_codec",
            "route_id",
        }
    )
    record = _exact_fields(value, fields, "SCCP transfer")
    _integer(record["version"], "SCCP transfer.version", 1, 1)
    source_domain = _protocol_domain(record["source_domain"], "SCCP transfer.source_domain")
    destination_domain = _protocol_domain(record["dest_domain"], "SCCP transfer.dest_domain")
    if source_domain != lane[0][2] or destination_domain != lane[1][2]:
        raise ValueError("SCCP transfer domains do not match its exact lane")
    _unsigned_decimal(record["nonce"], "SCCP transfer.nonce", _MAX_U64)
    _integer(record["route_revision"], "SCCP transfer.route_revision", 1, (1 << 32) - 1)
    _protocol_domain(record["asset_home_domain"], "SCCP transfer.asset_home_domain")
    _validate_codec_value(record, "asset_id_codec", "asset_id")
    _unsigned_decimal(record["amount"], "SCCP transfer.amount", _MAX_U128, positive=True)
    _validate_codec_value(record, "sender_codec", "sender", source_domain)
    _validate_codec_value(record, "recipient_codec", "recipient", destination_domain)
    _validate_codec_value(record, "route_id_codec", "route_id")


def normalize_sccp_message_bundle(value: Any) -> Mapping[str, Any]:
    """Normalize one raw JSON ``TairaSccpMessageProofV1`` bundle."""

    fields = frozenset(
        {"version", "commitment_root", "commitment", "merkle_proof", "payload", "finality_proof"}
    )
    record = _exact_fields(value, fields, "SCCP message bundle")
    _integer(record["version"], "SCCP message bundle.version", 1, 1)
    commitment_root = _lower_hex(
        record["commitment_root"], "SCCP message bundle.commitment_root", 32, prefix=True
    )
    commitment = _exact_fields(
        record["commitment"],
        frozenset({"version", "kind", "context", "message_id", "payload_hash"}),
        "SCCP message bundle.commitment",
    )
    _integer(commitment["version"], "SCCP message bundle.commitment.version", 1, 1)
    if commitment["kind"] != "Transfer":
        raise ValueError("SCCP message bundle commitment kind is unsupported or retired")
    context = _exact_fields(
        commitment["context"],
        frozenset({"lane", "destination_binding_hash", "route_configuration_hash"}),
        "SCCP message bundle.commitment.context",
    )
    lane = _outbound_lane(context["lane"], "SCCP message bundle.commitment.context.lane")
    destination_binding_hash = _lower_hex(
        context["destination_binding_hash"],
        "SCCP message bundle.commitment.context.destination_binding_hash",
        32,
        prefix=True,
    )
    route_configuration_hash = _lower_hex(
        context["route_configuration_hash"],
        "SCCP message bundle.commitment.context.route_configuration_hash",
        32,
        prefix=True,
    )
    message_id = _lower_hex(
        commitment["message_id"],
        "SCCP message bundle.commitment.message_id",
        32,
        prefix=True,
    )
    payload_hash = _lower_hex(
        commitment["payload_hash"],
        "SCCP message bundle.commitment.payload_hash",
        32,
        prefix=True,
    )
    hash_roles = (
        commitment_root,
        destination_binding_hash,
        route_configuration_hash,
        message_id,
        payload_hash,
    )
    if len(set(hash_roles)) != len(hash_roles):
        raise ValueError("SCCP message bundle reuses role-separated commitments")
    merkle = _exact_fields(
        record["merkle_proof"], frozenset({"steps"}), "SCCP message bundle.merkle_proof"
    )
    steps = _list(merkle["steps"], "SCCP message bundle.merkle_proof.steps")
    if len(steps) > 64:
        raise ValueError("SCCP message bundle Merkle proof exceeds 64 steps")
    for index, step in enumerate(steps):
        label = f"SCCP message bundle.merkle_proof.steps[{index}]"
        item = _exact_fields(step, frozenset({"sibling_hash", "sibling_is_left"}), label)
        _lower_hex(item["sibling_hash"], f"{label}.sibling_hash", 32, prefix=True)
        _boolean(item["sibling_is_left"], f"{label}.sibling_is_left")
    payload = _exact_fields(
        record["payload"], frozenset({"Transfer"}), "SCCP message bundle.payload"
    )
    _validate_transfer(payload["Transfer"], lane)
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
    _unsigned_decimal(height, f"{label}.finality_height", _MAX_U64, positive=True)
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
            "semantic_proof_profile",
            "semantic_proof_profile_hash",
            "sora_finality_anchor",
            "sora_finality_anchor_hash",
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
    semantic_hash, semantic_roles = _semantic_proof_profile(
        record["semantic_proof_profile"], "SCCP proof request.semantic_proof_profile"
    )
    anchor_hash, anchor_roles = _sora_finality_anchor(
        record["sora_finality_anchor"], "SCCP proof request.sora_finality_anchor"
    )
    _validate_proof_policy_roles(
        semantic_hash,
        semantic_roles,
        anchor_hash,
        anchor_roles,
        "SCCP proof request outbound policy",
    )
    hashes = (
        "verifier_key_hash",
        "semantic_proof_profile_hash",
        "sora_finality_anchor_hash",
        "statement_hash",
        "destination_binding_hash",
        "route_configuration_hash",
        "request_hash",
    )
    for field in hashes:
        _lower_hex(record[field], f"SCCP proof request.{field}", 32, prefix=True)
    if "0x" + _keccak_256(key_bytes).hex() != record["verifier_key_hash"]:
        raise ValueError("SCCP proof request verifier_key_hash does not match verifying_key")
    if "0x" + semantic_hash.hex() != record["semantic_proof_profile_hash"]:
        raise ValueError(
            "SCCP proof request semantic_proof_profile_hash does not match its typed profile"
        )
    if "0x" + anchor_hash.hex() != record["sora_finality_anchor_hash"]:
        raise ValueError(
            "SCCP proof request sora_finality_anchor_hash does not match its typed anchor"
        )
    public_hashes = tuple(
        inputs[field]
        for field in ("message_id", "payload_hash", "commitment_root", "finality_block_hash")
    )
    if len({*public_hashes, *(record[field] for field in hashes)}) != len(public_hashes) + len(
        hashes
    ):
        raise ValueError("SCCP proof request reuses role-separated commitments")
    _variable_hex(record["bundle_bytes"], "SCCP proof request.bundle_bytes")
    return _deep_freeze(record)


def _authority(value: Any, label: str) -> str:
    authority = _text(value, label, 512)
    from .client import _decode_canonical_i105_string

    _decode_canonical_i105_string(authority)
    return authority


def _fee_payment(value: Any, label: str) -> Dict[str, Any]:
    # Import lazily because the public client owns the shared typed fee-intent
    # normalizer and imports this SCCP module for its route codecs.
    from .client import ToriiClient

    return ToriiClient._normalize_fee_payment_intent(value, context=label)


def normalize_bridge_proof_submit_payload(value: Any) -> Dict[str, Any]:
    """Build the sole supported destination-proof submission body."""

    record = _exact_fields(
        value,
        frozenset(
            {
                "authority",
                "fee_payment",
                "signature_b64",
                "transaction_payload_b64",
                "destination_proof_b64",
                "creation_time_ms",
            }
        ),
        "bridge proof submit",
        frozenset({"authority", "fee_payment", "destination_proof_b64"}),
    )
    destination_proof = _canonical_base64(
        record["destination_proof_b64"],
        "bridge proof submit.destination_proof_b64",
        maximum_bytes=_MAX_DESTINATION_ARTIFACT_BYTES,
    )
    validate_norito_frame(
        destination_proof,
        context="bridge proof submit.destination_proof_b64",
        expected_type_name=_DESTINATION_ARTIFACT_TYPE_NAME,
        expected_padding_length=0,
    )
    creation_time = (
        None
        if "creation_time_ms" not in record
        else _integer(record["creation_time_ms"], "bridge proof submit.creation_time_ms", 1)
    )
    result: Dict[str, Any] = {
        "authority": _authority(record["authority"], "bridge proof submit.authority"),
        "fee_payment": _fee_payment(
            record["fee_payment"], "bridge proof submit.fee_payment"
        ),
        **_detached_signing_state(record, "bridge proof submit", creation_time),
        "destination_proof_b64": record["destination_proof_b64"],
    }
    if creation_time is not None:
        result["creation_time_ms"] = creation_time
    return result


def normalize_bridge_message_submit_payload(value: Any) -> Dict[str, Any]:
    """Build the sole supported native inbound message submission body."""

    record = _exact_fields(
        value,
        frozenset(
            {
                "authority",
                "fee_payment",
                "signature_b64",
                "transaction_payload_b64",
                "native_proof_b64",
                "creation_time_ms",
            }
        ),
        "bridge message submit",
        frozenset({"authority", "fee_payment", "native_proof_b64"}),
    )
    native_proof = _canonical_base64(
        record["native_proof_b64"], "bridge message submit.native_proof_b64"
    )
    validate_norito_frame(
        native_proof,
        context="bridge message submit.native_proof_b64",
        expected_type_name=_NATIVE_INBOUND_PROOF_TYPE_NAME,
        expected_padding_length=0,
    )
    creation_time = (
        None
        if "creation_time_ms" not in record
        else _integer(record["creation_time_ms"], "bridge message submit.creation_time_ms", 1)
    )
    result: Dict[str, Any] = {
        "authority": _authority(record["authority"], "bridge message submit.authority"),
        "fee_payment": _fee_payment(
            record["fee_payment"], "bridge message submit.fee_payment"
        ),
        **_detached_signing_state(record, "bridge message submit", creation_time),
        "native_proof_b64": record["native_proof_b64"],
    }
    if creation_time is not None:
        result["creation_time_ms"] = creation_time
    return result


def _detached_signing_state(
    record: Mapping[str, Any], label: str, creation_time: Optional[int]
) -> Dict[str, str]:
    has_signature = "signature_b64" in record
    has_transaction_payload = "transaction_payload_b64" in record
    if has_signature != has_transaction_payload:
        raise ValueError(
            f"{label} must omit both signature_b64 and transaction_payload_b64 for preparation "
            "or provide both for signed submission"
        )
    if not has_signature:
        return {}
    if creation_time is None:
        raise ValueError(f"{label}.creation_time_ms is required for signed submission")
    _canonical_base64(
        record["signature_b64"],
        f"{label}.signature_b64",
        maximum_bytes=_MAX_DETACHED_SIGNATURE_BYTES,
    )
    _canonical_base64(
        record["transaction_payload_b64"], f"{label}.transaction_payload_b64"
    )
    return {
        "signature_b64": record["signature_b64"],
        "transaction_payload_b64": record["transaction_payload_b64"],
    }


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
        route_configuration_hash_hex=_lower_hex(
            record["route_configuration_hash_hex"], "route_configuration_hash_hex", 32
        ),
        range_start_height=range_start,
        range_end_height=range_end,
        creation_time_ms=creation_time,
        tx_hash_hex=tx_hash,
        transaction_payload_b64=record["transaction_payload_b64"],
        signing_message_b64=record["signing_message_b64"],
    )
    expected = {} if expectations is None else dict(_mapping(expectations, "bridge expectations"))
    _exact_fields(
        expected,
        frozenset({"submitted", "creation_time_ms"}),
        "bridge expectations",
        frozenset(),
    )
    if (
        expected.get("creation_time_ms") is not None
        and expected["creation_time_ms"] != creation_time
    ):
        raise ValueError("bridge submit response.creation_time_ms does not match the request")
    if "submitted" in expected:
        expected_submitted = _boolean(expected["submitted"], "bridge expectations.submitted")
        if expected_submitted != submitted:
            raise ValueError(
                "bridge submit response.submitted does not match the request signing state"
            )
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

    def canonical_uint(token: str) -> int:
        if re.fullmatch(r"(?:0|[1-9][0-9]*)", token) is None:
            raise ValueError(f"{label} contains a noncanonical unsigned integer")
        return int(token)

    def reject_noninteger(token: str) -> NoReturn:
        raise ValueError(f"{label} contains a noncanonical numeric value `{token}`")

    try:
        value = json.loads(
            text,
            object_pairs_hook=unique_object,
            parse_int=canonical_uint,
            parse_float=reject_noninteger,
            parse_constant=reject_noninteger,
        )
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
    "SccpRegistryLimits",
    "SccpResourceLimits",
    "SccpCapabilities",
    "SccpRegistry",
    "SccpRecentMessages",
    "SccpRecentCursor",
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
