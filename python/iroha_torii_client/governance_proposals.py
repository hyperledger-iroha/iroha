"""Closed typed readers for the first-release governance proposal surface."""

from __future__ import annotations

import base64
import hashlib
import re
import unicodedata
from collections.abc import Iterator, Mapping
from dataclasses import dataclass
from enum import Enum
from typing import Any, Optional, Union, cast

from ._account_id import decode_canonical_i105_account_id

_U64_MAX = (1 << 64) - 1
_U128_MAX = (1 << 128) - 1
_TON_MAX_COINS = (1 << 120) - 1
_U32_MAX = (1 << 32) - 1
_JSON_SAFE_UINT_MAX = (1 << 53) - 1
_BECH32M_CONST = 0x2BC830A3
_BECH32_CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
_ROUTE_TOKEN = re.compile(r"[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?")
_KEBAB = re.compile(r"[a-z0-9](?:[a-z0-9-]*[a-z0-9])?")
_PORTABLE_VK_ID = re.compile(r"[a-z0-9](?:[a-z0-9_/:.-]*[a-z0-9])?")
_BN254_BASE_FIELD_MODULUS = int(
    "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47", 16
)
_SCCP_PUBLIC_SIGNAL_SCHEMA_HASH = (
    "7567439F41173D6745A3D51923CB70371ACC7D66F23CEFB4100D6D5D7A432CBB"
)
_SCCP_BLS12381_PUBLIC_SIGNAL_SCHEMA_HASH = (
    "A4DB9F6AAC0ECD22AC107BFDAFBF30DD01087147517EFE285D345F3F1182B874"
)
_SCCP_TAIRA_CHAIN_ID_HASH = (
    "CF1CFC0F57B0BFA4C21882A9870317A1F4812F86533897095E3944BE34C5BBA7"
)
_SCCP_TAIRA_XOR_ASSET_DEFINITION_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
_SCCP_SORA_OUTBOUND_SEMANTICS = "ivm_proved_record_sccp_message_v1"
_SCCP_MAX_SORA_OUTBOUND_GAS_LIMIT = 1_000_000_000
_KECCAK256_EMPTY_HEX = (
    "C5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470"
)


def _exact(value: Any, fields: frozenset[str], context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or any(not isinstance(key, str) for key in value):
        raise TypeError(f"{context} must be an object with string field names")
    actual = set(value)
    if actual != fields:
        missing = sorted(fields - actual)
        unknown = sorted(actual - fields)
        if missing:
            raise TypeError(f"{context} is missing required field `{missing[0]}`")
        raise TypeError(f"{context} contains unknown field `{unknown[0]}`")
    return value


def _string(value: Any, context: str, *, nonempty: bool = True) -> str:
    if not isinstance(value, str) or value != value.strip() or any(ord(ch) < 32 or ord(ch) == 127 for ch in value):
        raise TypeError(f"{context} must be exact text without surrounding whitespace")
    if nonempty and not value:
        raise TypeError(f"{context} must be non-empty")
    return value


def _uint(value: Any, context: str, maximum: int = _U64_MAX, *, positive: bool = False) -> int:
    minimum = 1 if positive else 0
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise TypeError(f"{context} must be an integer in {minimum}..{maximum}")
    return value


def _decimal_u64(value: Any, context: str, *, positive: bool = False) -> int:
    pattern = r"[1-9][0-9]*" if positive else r"(?:0|[1-9][0-9]*)"
    if not isinstance(value, str) or re.fullmatch(pattern, value) is None:
        raise TypeError(f"{context} must be a canonical unsigned decimal string")
    parsed = int(value)
    if parsed > _U64_MAX:
        raise TypeError(f"{context} must fit in u64")
    return parsed


def _decimal_u128(
    value: Any,
    context: str,
    *,
    positive: bool = False,
    maximum: int = _U128_MAX,
) -> int:
    pattern = r"[1-9][0-9]*" if positive else r"(?:0|[1-9][0-9]*)"
    if not isinstance(value, str) or re.fullmatch(pattern, value) is None:
        raise TypeError(f"{context} must be a canonical unsigned decimal string")
    parsed = int(value)
    if parsed > maximum:
        raise TypeError(f"{context} is outside its exact unsigned range")
    return parsed


def _numeric(value: Any, context: str) -> str:
    if not isinstance(value, str) or re.fullmatch(r"(?:0|[1-9][0-9]*)(?:\.[0-9]*[1-9])?", value) is None:
        raise TypeError(f"{context} must be a canonical non-negative numeric string")
    return value


def _lower_hex32(value: Any, context: str, *, nonzero: bool = False) -> str:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise TypeError(f"{context} must be exactly 32 lowercase hexadecimal bytes")
    if nonzero and set(value) == {"0"}:
        raise TypeError(f"{context} must be non-zero")
    return value


def _bytes32(value: Any, context: str, *, nonzero: bool = False) -> tuple[int, ...]:
    if not isinstance(value, list) or len(value) != 32:
        raise TypeError(f"{context} must be an exact 32-byte JSON array")
    result = tuple(_uint(byte, f"{context}[{index}]", 255) for index, byte in enumerate(value))
    if nonzero and not any(result):
        raise TypeError(f"{context} must be non-zero")
    return result


def _upper_hex(value: Any, context: str, byte_length: int, *, nonzero: bool = True) -> str:
    """Decode Norito JSON's canonical fixed-byte-array representation."""

    if not isinstance(value, str) or re.fullmatch(
        rf"[0-9A-F]{{{byte_length * 2}}}", value
    ) is None:
        raise TypeError(
            f"{context} must be canonical uppercase {byte_length}-byte hexadecimal text"
        )
    if nonzero and set(value) == {"0"}:
        raise TypeError(f"{context} must be non-zero")
    return value


def _sccp_uint(
    value: Any,
    context: str,
    maximum: int = _JSON_SAFE_UINT_MAX,
    *,
    positive: bool = False,
) -> int:
    """Decode a SCCP integer without losing precision in any supported SDK."""

    return _uint(value, context, min(maximum, _JSON_SAFE_UINT_MAX), positive=positive)


def _proposal_exact_json_uint(
    value: Any,
    context: str,
    *,
    positive: bool = False,
) -> int:
    """Apply the Torii first-release exact public-JSON integer invariant."""

    return _uint(value, context, _JSON_SAFE_UINT_MAX, positive=positive)


def _sccp_route_token(value: Any, context: str) -> str:
    token = _string(value, context)
    if _ROUTE_TOKEN.fullmatch(token) is None:
        raise TypeError(f"{context} must be canonical lowercase SCCP route text")
    return token


def _portable_vk_id(value: Any, context: str) -> str:
    field = _string(value, context)
    forbidden = ("..", "//", ":::", "/:", ":/", "/.", "./", ":.", ".:")
    if (
        len(field.encode("utf-8")) > 256
        or _PORTABLE_VK_ID.fullmatch(field) is None
        or any(part in field for part in forbidden)
    ):
        raise TypeError(f"{context} must use portable verification-key registry syntax")
    return field


def _provider_id(value: Any, context: str) -> tuple[int, ...]:
    if not isinstance(value, list) or len(value) != 1:
        raise TypeError(f"{context} must be the exact one-field ProviderId tuple")
    return _bytes32(value[0], f"{context}[0]", nonzero=True)


def _string_tuple(value: Any, context: str) -> str:
    if not isinstance(value, list) or len(value) != 1:
        raise TypeError(f"{context} must be the exact one-field string tuple")
    return _string(value[0], f"{context}[0]")


def _ascii_kebab(value: str, context: str, maximum: int) -> str:
    if len(value.encode("utf-8")) > maximum or _KEBAB.fullmatch(value) is None:
        raise TypeError(f"{context} must be canonical lowercase ASCII kebab text")
    return value


def _iroha_name(value: Any, context: str) -> str:
    literal = _string(value, context)
    forbidden = {"@", "#", "$"}
    if (
        len(literal.encode("utf-8")) > 255
        or unicodedata.normalize("NFC", literal) != literal
        or any(
            char.isspace()
            or char in forbidden
            or unicodedata.category(char) == "Cc"
            for char in literal
        )
    ):
        raise TypeError(f"{context} must be a canonical Iroha Name")
    return literal


def _crc16(value: bytes) -> int:
    crc = 0xFFFF
    for byte in value:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def _network_id(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a canonical NetworkId")
    match = re.fullmatch(r"hash:([0-9A-F]{64})#([0-9A-F]{4})", value)
    if match is None:
        raise TypeError(f"{context} must use canonical hash:<uppercase hex>#<CRC16> syntax")
    body, checksum = match.groups()
    if _crc16(f"hash:{body}".encode("ascii")) != int(checksum, 16) or int(body[-2:], 16) & 1 == 0:
        raise TypeError(f"{context} is not a canonical Iroha hash")
    return value


def _bech32_polymod(values: list[int]) -> int:
    generators = (0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)
    check = 1
    for value in values:
        top = check >> 25
        check = ((check & 0x1FFFFFF) << 5) ^ value
        for index, generator in enumerate(generators):
            if (top >> index) & 1:
                check ^= generator
    return check


def _contract_address(value: Any, context: str) -> str:
    literal = _string(value, context)
    if literal != literal.lower() or not literal.startswith("irohac1"):
        raise TypeError(f"{context} must be a canonical lowercase irohac Bech32m address")
    data_text = literal[7:]
    try:
        data = [_BECH32_CHARSET.index(char) for char in data_text]
    except ValueError as exc:
        raise TypeError(f"{context} contains a non-Bech32 character") from exc
    hrp = "irohac"
    expanded = [ord(char) >> 5 for char in hrp] + [0] + [ord(char) & 31 for char in hrp]
    if len(data) < 7 or _bech32_polymod(expanded + data) != _BECH32M_CONST:
        raise TypeError(f"{context} has an invalid Bech32m checksum")
    accumulator = 0
    bits = 0
    decoded = bytearray()
    for digit in data[:-6]:
        accumulator = (accumulator << 5) | digit
        bits += 5
        while bits >= 8:
            bits -= 8
            decoded.append((accumulator >> bits) & 255)
    if bits >= 5 or (accumulator & ((1 << bits) - 1)) != 0 or len(decoded) != 29 or decoded[0] != 1:
        raise TypeError(f"{context} is not a canonical V1 contract address")
    return literal


def _account_id(value: Any, context: str) -> str:
    literal = _string(value, context)
    if "@" in literal:
        raise TypeError(f"{context} must be an exact canonical I105 account id")
    try:
        decode_canonical_i105_account_id(literal)
    except ValueError as exc:
        raise TypeError(f"{context} must be an exact canonical I105 account id") from exc
    return literal


def _asset_definition_id(value: Any, context: str) -> str:
    literal = _string(value, context)
    if literal.count("#") != 1 or any(not part for part in literal.split("#")):
        raise TypeError(f"{context} must be a canonical asset definition id")
    return literal


def _canonical_base64(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be canonical padded base64")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, TypeError) as exc:
        raise TypeError(f"{context} must be canonical padded base64") from exc
    if base64.b64encode(decoded).decode("ascii") != value:
        raise TypeError(f"{context} must be canonical padded base64")
    return value


@dataclass(frozen=True)
class GovernanceCanonicalObject(Mapping[str, Any]):
    """Recursively immutable object after variant-specific shape validation."""

    entries: tuple[tuple[str, Any], ...]

    def __getitem__(self, key: str) -> Any:
        for name, value in self.entries:
            if name == key:
                return value
        raise KeyError(key)

    def __iter__(self) -> Iterator[str]:
        return (name for name, _ in self.entries)

    def __len__(self) -> int:
        return len(self.entries)


def _freeze(value: Any) -> Any:
    if isinstance(value, Mapping):
        if any(not isinstance(key, str) for key in value):
            raise TypeError("canonical governance objects require string field names")
        return GovernanceCanonicalObject(tuple((key, _freeze(entry)) for key, entry in value.items()))
    if isinstance(value, list):
        return tuple(_freeze(entry) for entry in value)
    if value is None or isinstance(value, (str, int, bool)):
        return value
    raise TypeError("governance payload contains a non-JSON value")


@dataclass(frozen=True)
class GovernanceManifestProvenance:
    """One exact public manifest signature."""

    signer: str
    signature: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceManifestProvenance":
        record = _exact(value, frozenset({"signer", "signature"}), context)
        return cls(_string(record["signer"], f"{context}.signer"), _string(record["signature"], f"{context}.signature"))


@dataclass(frozen=True)
class GovernanceProposalDeployContract:
    """Canonical `DeployContractProposal` payload."""

    contract_address: str
    code_hash: str
    abi_hash: str
    abi_version: int
    manifest_provenance: Optional[GovernanceManifestProvenance]

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalDeployContract":
        context = "DeployContract payload"
        record = _exact(value, frozenset({"contract_address", "code_hash", "abi_hash", "abi_version", "manifest_provenance"}), context)
        abi_version = _uint(record["abi_version"], f"{context}.abi_version", 0xFFFF, positive=True)
        if abi_version != 1:
            raise TypeError(f"{context}.abi_version must be the integer 1")
        provenance = None if record["manifest_provenance"] is None else GovernanceManifestProvenance.from_payload(record["manifest_provenance"], f"{context}.manifest_provenance")
        return cls(_contract_address(record["contract_address"], f"{context}.contract_address"), _lower_hex32(record["code_hash"], f"{context}.code_hash"), _lower_hex32(record["abi_hash"], f"{context}.abi_hash"), abi_version, provenance)


@dataclass(frozen=True)
class GovernanceRuntimeUpgradeManifest:
    """Complete canonical first-release runtime-upgrade manifest."""

    name: str
    description: str
    abi_version: int
    abi_hash: tuple[int, ...]
    added_syscalls: tuple[int, ...]
    added_pointer_types: tuple[int, ...]
    start_height: int
    end_height: int
    sbom_digests: tuple[GovernanceCanonicalObject, ...]
    slsa_attestation: str
    provenance: tuple[GovernanceManifestProvenance, ...]

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceRuntimeUpgradeManifest":
        context = "RuntimeUpgrade payload.manifest"
        fields = frozenset({"name", "description", "abi_version", "abi_hash", "added_syscalls", "added_pointer_types", "start_height", "end_height", "sbom_digests", "slsa_attestation", "provenance"})
        record = _exact(value, fields, context)
        abi = _uint(record["abi_version"], f"{context}.abi_version", 0xFFFF, positive=True)
        if abi != 1 or record["added_syscalls"] != [] or record["added_pointer_types"] != []:
            raise TypeError(f"{context} must use ABI 1 with empty syscall and pointer-type deltas")
        start = _proposal_exact_json_uint(
            record["start_height"], f"{context}.start_height"
        )
        end = _proposal_exact_json_uint(record["end_height"], f"{context}.end_height")
        if end <= start:
            raise TypeError(f"{context}.end_height must be greater than start_height")
        if not isinstance(record["sbom_digests"], list) or not isinstance(record["provenance"], list):
            raise TypeError(f"{context} SBOM and provenance fields must be arrays")
        sboms = []
        for index, item in enumerate(record["sbom_digests"]):
            item_context = f"{context}.sbom_digests[{index}]"
            item_record = _exact(item, frozenset({"algorithm", "digest"}), item_context)
            sboms.append(_freeze({"algorithm": _string(item_record["algorithm"], f"{item_context}.algorithm"), "digest": _canonical_base64(item_record["digest"], f"{item_context}.digest")}))
        provenance = tuple(GovernanceManifestProvenance.from_payload(item, f"{context}.provenance[{index}]") for index, item in enumerate(record["provenance"]))
        return cls(_string(record["name"], f"{context}.name"), _string(record["description"], f"{context}.description", nonempty=False), abi, _bytes32(record["abi_hash"], f"{context}.abi_hash"), (), (), start, end, tuple(sboms), _canonical_base64(record["slsa_attestation"], f"{context}.slsa_attestation"), provenance)


@dataclass(frozen=True)
class GovernanceProposalRuntimeUpgrade:
    """Canonical `RuntimeUpgradeProposal` payload."""

    manifest: GovernanceRuntimeUpgradeManifest

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalRuntimeUpgrade":
        record = _exact(value, frozenset({"manifest"}), "RuntimeUpgrade payload")
        return cls(GovernanceRuntimeUpgradeManifest.from_payload(record["manifest"]))


class GovernanceSccpRouteActionKind(str, Enum):
    """Closed SCCP route-governance action tags."""

    REGISTER = "Register"
    SET_ACTIVATION = "SetActivation"
    SWITCH_REVISION = "SwitchRevision"
    INITIALIZE_TRUST_ANCHOR = "InitializeTrustAnchor"
    ADVANCE_TRUST_ANCHOR = "AdvanceTrustAnchor"
    REMOVE = "Remove"


class GovernanceSccpNetworkKind(str, Enum):
    """Closed `SccpNetworkV1` JSON tags."""

    SORA_TAIRA = "sora_taira"
    ETHEREUM_MAINNET = "ethereum_mainnet"
    BSC_MAINNET = "bsc_mainnet"
    TRON_MAINNET = "tron_mainnet"
    TON_MAINNET = "ton_mainnet"


_SCCP_EXTERNAL_NETWORK_FAMILY = {
    GovernanceSccpNetworkKind.ETHEREUM_MAINNET: "evm",
    GovernanceSccpNetworkKind.BSC_MAINNET: "evm",
    GovernanceSccpNetworkKind.TRON_MAINNET: "tron",
    GovernanceSccpNetworkKind.TON_MAINNET: "ton",
}

_SCCP_EXACT_ROUTE_ID = {
    GovernanceSccpNetworkKind.ETHEREUM_MAINNET: "taira_eth_xor",
    GovernanceSccpNetworkKind.BSC_MAINNET: "taira_bsc_xor",
    GovernanceSccpNetworkKind.TRON_MAINNET: "taira_tron_xor",
    GovernanceSccpNetworkKind.TON_MAINNET: "taira_ton_xor",
}


@dataclass(frozen=True)
class GovernanceSccpNetwork:
    """Exact adjacently tagged `SccpNetworkV1`."""

    network: GovernanceSccpNetworkKind
    profile: None

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpNetwork":
        record = _exact(value, frozenset({"network", "profile"}), context)
        if record["profile"] is not None:
            raise TypeError(f"{context}.profile must be null for a unit network variant")
        try:
            network = GovernanceSccpNetworkKind(record["network"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.network is unsupported or retired") from exc
        return cls(network, None)

    @property
    def family(self) -> Optional[str]:
        """Return the closed external family, or `None` for SORA Taira."""

        return _SCCP_EXTERNAL_NETWORK_FAMILY.get(self.network)


@dataclass(frozen=True)
class GovernanceSccpLaneId:
    """Exact supported external-to-Taira `SccpLaneIdV1`."""

    source: GovernanceSccpNetwork
    target: GovernanceSccpNetwork

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpLaneId":
        record = _exact(value, frozenset({"source", "target"}), context)
        source = GovernanceSccpNetwork.from_payload(record["source"], f"{context}.source")
        target = GovernanceSccpNetwork.from_payload(record["target"], f"{context}.target")
        if source.family is None or target.network is not GovernanceSccpNetworkKind.SORA_TAIRA:
            raise TypeError(f"{context} must be an exact supported external-to-Taira lane")
        return cls(source, target)


class GovernanceSccpRouteActivationKind(str, Enum):
    """Closed `SccpRouteActivationV1` JSON tags."""

    STAGED = "staged"
    BIDIRECTIONAL = "bidirectional"
    INBOUND_ONLY = "inbound_only"
    PAUSED = "paused"
    RETIRED = "retired"


@dataclass(frozen=True)
class GovernanceSccpRouteActivation:
    """Exact adjacently tagged SCCP activation state."""

    activation: GovernanceSccpRouteActivationKind
    direction: None

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpRouteActivation":
        record = _exact(value, frozenset({"activation", "direction"}), context)
        if record["direction"] is not None:
            raise TypeError(f"{context}.direction must be null for a unit activation variant")
        try:
            activation = GovernanceSccpRouteActivationKind(record["activation"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.activation is unsupported or retired") from exc
        return cls(activation, None)


@dataclass(frozen=True)
class GovernanceSccpInboundFinalityCutoff:
    """Authenticated delayed-claim cutoff for one retired route."""

    trust_anchor_hash: str
    max_anchor_interval_height: int

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpInboundFinalityCutoff":
        record = _exact(
            value,
            frozenset({"trust_anchor_hash", "max_anchor_interval_height"}),
            context,
        )
        return cls(
            _upper_hex(record["trust_anchor_hash"], f"{context}.trust_anchor_hash", 32),
            _sccp_uint(
                record["max_anchor_interval_height"],
                f"{context}.max_anchor_interval_height",
                positive=True,
            ),
        )


@dataclass(frozen=True)
class GovernanceSccpRouteKey:
    """Exact immutable lookup key for one governed SCCP route."""

    lane_id: GovernanceSccpLaneId
    route_id: str
    asset_key: str
    revision: int

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpRouteKey":
        record = _exact(
            value,
            frozenset({"lane_id", "route_id", "asset_key", "revision"}),
            context,
        )
        return cls(
            GovernanceSccpLaneId.from_payload(record["lane_id"], f"{context}.lane_id"),
            _sccp_route_token(record["route_id"], f"{context}.route_id"),
            _sccp_route_token(record["asset_key"], f"{context}.asset_key"),
            _sccp_uint(record["revision"], f"{context}.revision", _U32_MAX, positive=True),
        )


class GovernanceSccpNativeProofBackendKind(str, Enum):
    """Closed `BridgeNativeProofBackendV1` JSON tags."""

    ETHEREUM_BEACON = "ethereum_beacon_v1"
    BSC_PARLIA = "bsc_parlia_v1"
    TRON_DPOS = "tron_dpos_v1"
    TON_MASTERCHAIN = "ton_masterchain_v1"


_SCCP_NATIVE_BACKEND_NETWORKS = {
    GovernanceSccpNativeProofBackendKind.ETHEREUM_BEACON: frozenset(
        {GovernanceSccpNetworkKind.ETHEREUM_MAINNET}
    ),
    GovernanceSccpNativeProofBackendKind.BSC_PARLIA: frozenset(
        {GovernanceSccpNetworkKind.BSC_MAINNET}
    ),
    GovernanceSccpNativeProofBackendKind.TRON_DPOS: frozenset(
        {GovernanceSccpNetworkKind.TRON_MAINNET}
    ),
    GovernanceSccpNativeProofBackendKind.TON_MASTERCHAIN: frozenset(
        {GovernanceSccpNetworkKind.TON_MAINNET}
    ),
}


@dataclass(frozen=True)
class GovernanceSccpNativeProofBackend:
    """Exact adjacently tagged native proof backend."""

    backend: GovernanceSccpNativeProofBackendKind
    protocol: None

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpNativeProofBackend":
        record = _exact(value, frozenset({"backend", "protocol"}), context)
        if record["protocol"] is not None:
            raise TypeError(f"{context}.protocol must be null for a unit backend variant")
        try:
            backend = GovernanceSccpNativeProofBackendKind(record["backend"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.backend is unsupported or retired") from exc
        return cls(backend, None)


@dataclass(frozen=True)
class GovernanceSccpNativeTrustAnchor:
    """Governed native checkpoint for one SCCP lane."""

    backend: GovernanceSccpNativeProofBackend
    anchor_hash: str
    checkpoint_height: int

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpNativeTrustAnchor":
        record = _exact(
            value,
            frozenset({"backend", "anchor_hash", "checkpoint_height"}),
            context,
        )
        return cls(
            GovernanceSccpNativeProofBackend.from_payload(
                record["backend"], f"{context}.backend"
            ),
            _upper_hex(record["anchor_hash"], f"{context}.anchor_hash", 32),
            _sccp_uint(
                record["checkpoint_height"], f"{context}.checkpoint_height", positive=True
            ),
        )

    def supports(self, lane: GovernanceSccpLaneId) -> bool:
        """Return whether this backend is the one closed for the lane source."""

        return lane.source.network in _SCCP_NATIVE_BACKEND_NETWORKS[self.backend.backend]


@dataclass(frozen=True)
class GovernanceSccpEvmSourceEmitter:
    """Exact direct EVM source-emitter identity."""

    address: str
    runtime_code_hash: str
    route_config_hash: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpEvmSourceEmitter":
        record = _exact(
            value,
            frozenset({"address", "runtime_code_hash", "route_config_hash"}),
            context,
        )
        return cls(
            _upper_hex(record["address"], f"{context}.address", 20),
            _upper_hex(record["runtime_code_hash"], f"{context}.runtime_code_hash", 32),
            _upper_hex(record["route_config_hash"], f"{context}.route_config_hash", 32),
        )


@dataclass(frozen=True)
class GovernanceSccpTronSourceEmitter:
    """Exact governed direct TRON source-emitter identity."""

    address: str
    runtime_code_hash: str
    route_config_hash: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpTronSourceEmitter":
        record = _exact(
            value,
            frozenset({"address", "runtime_code_hash", "route_config_hash"}),
            context,
        )
        return cls(
            _upper_hex(record["address"], f"{context}.address", 20),
            _upper_hex(record["runtime_code_hash"], f"{context}.runtime_code_hash", 32),
            _upper_hex(record["route_config_hash"], f"{context}.route_config_hash", 32),
        )


@dataclass(frozen=True)
class GovernanceSccpTonAddress:
    """Canonical TON basechain account identity."""

    workchain: int
    account: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpTonAddress":
        record = _exact(value, frozenset({"workchain", "account"}), context)
        workchain = _uint(record["workchain"], f"{context}.workchain")
        if workchain != 0:
            raise TypeError(f"{context}.workchain must be the basechain integer 0")
        return cls(workchain, _upper_hex(record["account"], f"{context}.account", 32))


@dataclass(frozen=True)
class GovernanceSccpTonSourceEmitter:
    """Exact immutable TON source-contract identity."""

    address: GovernanceSccpTonAddress
    code_hash: str
    route_config_hash: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpTonSourceEmitter":
        record = _exact(
            value,
            frozenset({"address", "code_hash", "route_config_hash"}),
            context,
        )
        code_hash = _upper_hex(record["code_hash"], f"{context}.code_hash", 32)
        route_hash = _upper_hex(
            record["route_config_hash"], f"{context}.route_config_hash", 32
        )
        if code_hash == route_hash:
            raise TypeError(f"{context} reuses a TON source hash role")
        return cls(
            GovernanceSccpTonAddress.from_payload(
                record["address"], f"{context}.address"
            ),
            code_hash,
            route_hash,
        )


class GovernanceSccpSourceEmitterKind(str, Enum):
    """Closed `SccpSourceEmitterV1` JSON tags."""

    EVM = "evm"
    TRON = "tron"
    TON = "ton"


GovernanceSccpSourceEmitterIdentity = Union[
    GovernanceSccpEvmSourceEmitter,
    GovernanceSccpTronSourceEmitter,
    GovernanceSccpTonSourceEmitter,
]


@dataclass(frozen=True)
class GovernanceSccpSourceEmitter:
    """Exact adjacently tagged source-emitter union."""

    emitter: GovernanceSccpSourceEmitterKind
    identity: GovernanceSccpSourceEmitterIdentity

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpSourceEmitter":
        record = _exact(value, frozenset({"emitter", "identity"}), context)
        try:
            emitter = GovernanceSccpSourceEmitterKind(record["emitter"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.emitter is unsupported or retired") from exc
        parser = {
            GovernanceSccpSourceEmitterKind.EVM: GovernanceSccpEvmSourceEmitter.from_payload,
            GovernanceSccpSourceEmitterKind.TRON: GovernanceSccpTronSourceEmitter.from_payload,
            GovernanceSccpSourceEmitterKind.TON: GovernanceSccpTonSourceEmitter.from_payload,
        }[emitter]
        identity = cast(
            GovernanceSccpSourceEmitterIdentity,
            parser(record["identity"], f"{context}.identity"),
        )
        return cls(emitter, identity)


@dataclass(frozen=True)
class GovernanceSccpSourceIdentity:
    """Typed external-source identity bound to one inbound lane."""

    lane: GovernanceSccpLaneId
    emitter: GovernanceSccpSourceEmitter

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpSourceIdentity":
        record = _exact(value, frozenset({"lane", "emitter"}), context)
        lane = GovernanceSccpLaneId.from_payload(record["lane"], f"{context}.lane")
        emitter = GovernanceSccpSourceEmitter.from_payload(record["emitter"], f"{context}.emitter")
        if emitter.emitter.value != lane.source.family:
            raise TypeError(f"{context}.emitter does not match the lane source family")
        return cls(lane, emitter)


@dataclass(frozen=True)
class GovernanceSccpBn254G1Point:
    """Canonical non-infinity BN254 G1 point."""

    x: str
    y: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpBn254G1Point":
        record = _exact(value, frozenset({"x", "y"}), context)
        x = _upper_hex(record["x"], f"{context}.x", 32, nonzero=False)
        y = _upper_hex(record["y"], f"{context}.y", 32, nonzero=False)
        if int(x, 16) >= _BN254_BASE_FIELD_MODULUS or int(y, 16) >= _BN254_BASE_FIELD_MODULUS:
            raise TypeError(f"{context} contains a non-canonical BN254 field element")
        if int(x, 16) == 0 and int(y, 16) == 0:
            raise TypeError(f"{context} must not encode the BN254 point at infinity")
        return cls(x, y)


@dataclass(frozen=True)
class GovernanceSccpBn254G2Point:
    """Canonical non-infinity BN254 G2 point in Solidity limb order."""

    x_c0: str
    x_c1: str
    y_c0: str
    y_c1: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpBn254G2Point":
        fields = ("x_c0", "x_c1", "y_c0", "y_c1")
        record = _exact(value, frozenset(fields), context)
        limbs = tuple(
            _upper_hex(record[field], f"{context}.{field}", 32, nonzero=False)
            for field in fields
        )
        if any(int(limb, 16) >= _BN254_BASE_FIELD_MODULUS for limb in limbs):
            raise TypeError(f"{context} contains a non-canonical BN254 field element")
        if all(int(limb, 16) == 0 for limb in limbs):
            raise TypeError(f"{context} must not encode the BN254 point at infinity")
        return cls(*limbs)


@dataclass(frozen=True)
class GovernanceSccpGroth16Bn254Ic:
    """Fixed Groth16 IC vector for exactly eleven public signals."""

    constant: GovernanceSccpBn254G1Point
    signal_0: GovernanceSccpBn254G1Point
    signal_1: GovernanceSccpBn254G1Point
    signal_2: GovernanceSccpBn254G1Point
    signal_3: GovernanceSccpBn254G1Point
    signal_4: GovernanceSccpBn254G1Point
    signal_5: GovernanceSccpBn254G1Point
    signal_6: GovernanceSccpBn254G1Point
    signal_7: GovernanceSccpBn254G1Point
    signal_8: GovernanceSccpBn254G1Point
    signal_9: GovernanceSccpBn254G1Point
    signal_10: GovernanceSccpBn254G1Point

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpGroth16Bn254Ic":
        fields = ("constant",) + tuple(f"signal_{index}" for index in range(11))
        record = _exact(value, frozenset(fields), context)
        return cls(
            *(
                GovernanceSccpBn254G1Point.from_payload(
                    record[field], f"{context}.{field}"
                )
                for field in fields
            )
        )


@dataclass(frozen=True)
class GovernanceSccpGroth16Bn254VerifyingKey:
    """Closed fixed-shape BN254 Groth16 verifying key."""

    version: int
    alpha1: GovernanceSccpBn254G1Point
    beta2: GovernanceSccpBn254G2Point
    gamma2: GovernanceSccpBn254G2Point
    delta2: GovernanceSccpBn254G2Point
    ic: GovernanceSccpGroth16Bn254Ic

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpGroth16Bn254VerifyingKey":
        fields = frozenset({"version", "alpha1", "beta2", "gamma2", "delta2", "ic"})
        record = _exact(value, fields, context)
        version = _sccp_uint(record["version"], f"{context}.version", 0xFF, positive=True)
        if version != 1:
            raise TypeError(f"{context}.version must be the integer 1")
        return cls(
            version,
            GovernanceSccpBn254G1Point.from_payload(record["alpha1"], f"{context}.alpha1"),
            GovernanceSccpBn254G2Point.from_payload(record["beta2"], f"{context}.beta2"),
            GovernanceSccpBn254G2Point.from_payload(record["gamma2"], f"{context}.gamma2"),
            GovernanceSccpBn254G2Point.from_payload(record["delta2"], f"{context}.delta2"),
            GovernanceSccpGroth16Bn254Ic.from_payload(record["ic"], f"{context}.ic"),
        )


@dataclass(frozen=True)
class GovernanceSccpGroth16Bn254SemanticCircuit:
    """Immutable commitments for the audited eleven-signal circuit."""

    version: int
    circuit_commitment: str
    witness_generator_commitment: str
    public_signal_schema_hash: str

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpGroth16Bn254SemanticCircuit":
        record = _exact(
            value,
            frozenset(
                {
                    "version",
                    "circuit_commitment",
                    "witness_generator_commitment",
                    "public_signal_schema_hash",
                }
            ),
            context,
        )
        version = _sccp_uint(record["version"], f"{context}.version", 0xFF, positive=True)
        if version != 1:
            raise TypeError(f"{context}.version must be the integer 1")
        circuit = _upper_hex(
            record["circuit_commitment"], f"{context}.circuit_commitment", 32
        )
        witness = _upper_hex(
            record["witness_generator_commitment"],
            f"{context}.witness_generator_commitment",
            32,
        )
        schema = _upper_hex(
            record["public_signal_schema_hash"],
            f"{context}.public_signal_schema_hash",
            32,
        )
        if schema != _SCCP_PUBLIC_SIGNAL_SCHEMA_HASH:
            raise TypeError(f"{context}.public_signal_schema_hash is not the V1 schema")
        if len({circuit, witness, schema}) != 3:
            raise TypeError(f"{context} reuses a semantic commitment role")
        return cls(version, circuit, witness, schema)


@dataclass(frozen=True)
class GovernanceSccpGroth16Bls12381SemanticCircuit:
    """Immutable commitments for the audited TON eleven-signal circuit."""

    version: int
    circuit_commitment: str
    witness_generator_commitment: str
    public_signal_schema_hash: str

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpGroth16Bls12381SemanticCircuit":
        record = _exact(
            value,
            frozenset(
                {
                    "version",
                    "circuit_commitment",
                    "witness_generator_commitment",
                    "public_signal_schema_hash",
                }
            ),
            context,
        )
        version = _sccp_uint(
            record["version"], f"{context}.version", 0xFF, positive=True
        )
        if version != 1:
            raise TypeError(f"{context}.version must be the integer 1")
        circuit = _upper_hex(
            record["circuit_commitment"], f"{context}.circuit_commitment", 32
        )
        witness = _upper_hex(
            record["witness_generator_commitment"],
            f"{context}.witness_generator_commitment",
            32,
        )
        schema = _upper_hex(
            record["public_signal_schema_hash"],
            f"{context}.public_signal_schema_hash",
            32,
        )
        if schema != _SCCP_BLS12381_PUBLIC_SIGNAL_SCHEMA_HASH:
            raise TypeError(f"{context}.public_signal_schema_hash is not the TON V1 schema")
        if len({circuit, witness, schema}) != 3:
            raise TypeError(f"{context} reuses a semantic commitment role")
        return cls(version, circuit, witness, schema)


class GovernanceSccpSemanticProofProfileKind(str, Enum):
    """Closed semantic proof-profile tags."""

    SORA_TAIRA_FINALITY_INCLUSION_GROTH16_BN254 = (
        "sora_taira_finality_inclusion_groth16_bn254"
    )
    SORA_TAIRA_FINALITY_INCLUSION_GROTH16_BLS12381 = (
        "sora_taira_finality_inclusion_groth16_bls12381"
    )


@dataclass(frozen=True)
class GovernanceSccpSemanticProofProfile:
    """Exact adjacently tagged semantic proof profile."""

    profile: GovernanceSccpSemanticProofProfileKind
    commitments: Union[
        GovernanceSccpGroth16Bn254SemanticCircuit,
        GovernanceSccpGroth16Bls12381SemanticCircuit,
    ]

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpSemanticProofProfile":
        record = _exact(value, frozenset({"profile", "commitments"}), context)
        try:
            profile = GovernanceSccpSemanticProofProfileKind(record["profile"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.profile is unsupported or retired") from exc
        parser = (
            GovernanceSccpGroth16Bn254SemanticCircuit.from_payload
            if profile
            is GovernanceSccpSemanticProofProfileKind.SORA_TAIRA_FINALITY_INCLUSION_GROTH16_BN254
            else GovernanceSccpGroth16Bls12381SemanticCircuit.from_payload
        )
        return cls(profile, parser(record["commitments"], f"{context}.commitments"))


@dataclass(frozen=True)
class GovernanceSccpSoraFinalityAnchor:
    """Immutable Taira checkpoint anchoring an outbound proof policy."""

    version: int
    source_network: GovernanceSccpNetwork
    protocol_version: int
    chain_id_hash: str
    checkpoint_height: int
    checkpoint_block_hash: str
    checkpoint_context_id: str
    checkpoint_finality_artifact_hash: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpSoraFinalityAnchor":
        fields = frozenset(
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
        )
        record = _exact(value, fields, context)
        version = _sccp_uint(record["version"], f"{context}.version", 0xFF, positive=True)
        if version != 1:
            raise TypeError(f"{context}.version must be the integer 1")
        source = GovernanceSccpNetwork.from_payload(
            record["source_network"], f"{context}.source_network"
        )
        if source.network is not GovernanceSccpNetworkKind.SORA_TAIRA:
            raise TypeError(f"{context}.source_network must be SORA Taira")
        protocol = _sccp_uint(
            record["protocol_version"], f"{context}.protocol_version", 0xFFFF, positive=True
        )
        if protocol != 4:
            raise TypeError(f"{context}.protocol_version must be the integer 4")
        chain_hash = _upper_hex(record["chain_id_hash"], f"{context}.chain_id_hash", 32)
        if chain_hash != _SCCP_TAIRA_CHAIN_ID_HASH:
            raise TypeError(f"{context}.chain_id_hash is not the Taira chain commitment")
        checkpoint_height = _sccp_uint(
            record["checkpoint_height"], f"{context}.checkpoint_height", positive=True
        )
        checkpoint_hash = _upper_hex(
            record["checkpoint_block_hash"], f"{context}.checkpoint_block_hash", 32
        )
        context_id = _upper_hex(
            record["checkpoint_context_id"], f"{context}.checkpoint_context_id", 32
        )
        artifact_hash = _upper_hex(
            record["checkpoint_finality_artifact_hash"],
            f"{context}.checkpoint_finality_artifact_hash",
            32,
        )
        if len({chain_hash, checkpoint_hash, context_id, artifact_hash}) != 4:
            raise TypeError(f"{context} reuses a consensus hash role")
        return cls(
            version,
            source,
            protocol,
            chain_hash,
            checkpoint_height,
            checkpoint_hash,
            context_id,
            artifact_hash,
        )


@dataclass(frozen=True)
class GovernanceSccpOutboundProofPolicy:
    """Mandatory immutable outbound proof policy."""

    version: int
    semantic_profile: GovernanceSccpSemanticProofProfile
    sora_finality_anchor: GovernanceSccpSoraFinalityAnchor

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpOutboundProofPolicy":
        record = _exact(
            value,
            frozenset({"version", "semantic_profile", "sora_finality_anchor"}),
            context,
        )
        version = _sccp_uint(record["version"], f"{context}.version", 0xFF, positive=True)
        if version != 1:
            raise TypeError(f"{context}.version must be the integer 1")
        return cls(
            version,
            GovernanceSccpSemanticProofProfile.from_payload(
                record["semantic_profile"], f"{context}.semantic_profile"
            ),
            GovernanceSccpSoraFinalityAnchor.from_payload(
                record["sora_finality_anchor"], f"{context}.sora_finality_anchor"
            ),
        )


@dataclass(frozen=True)
class GovernanceSccpPortableVerifyingKeyRef:
    """Strict portable reference to one governance-registered IVM key."""

    backend: str
    name: str
    version: int
    commitment: str

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpPortableVerifyingKeyRef":
        record = _exact(value, frozenset({"backend", "name", "version", "commitment"}), context)
        return cls(
            _portable_vk_id(record["backend"], f"{context}.backend"),
            _portable_vk_id(record["name"], f"{context}.name"),
            _sccp_uint(record["version"], f"{context}.version", _U32_MAX, positive=True),
            _upper_hex(record["commitment"], f"{context}.commitment", 32),
        )


@dataclass(frozen=True)
class GovernanceSccpSoraOutboundExecutionPolicy:
    """Mandatory Taira-side execution policy for one outbound route."""

    version: int
    semantics: str
    contract_artifact_sha256: str
    vk_ref: GovernanceSccpPortableVerifyingKeyRef
    gas_limit: int

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpSoraOutboundExecutionPolicy":
        record = _exact(
            value,
            frozenset(
                {"version", "semantics", "contract_artifact_sha256", "vk_ref", "gas_limit"}
            ),
            context,
        )
        version = _sccp_uint(record["version"], f"{context}.version", 0xFF, positive=True)
        if version != 1:
            raise TypeError(f"{context}.version must be the integer 1")
        semantics = _string(record["semantics"], f"{context}.semantics")
        if semantics != _SCCP_SORA_OUTBOUND_SEMANTICS:
            raise TypeError(f"{context}.semantics is unsupported or retired")
        artifact = _upper_hex(
            record["contract_artifact_sha256"],
            f"{context}.contract_artifact_sha256",
            32,
        )
        vk_ref = GovernanceSccpPortableVerifyingKeyRef.from_payload(
            record["vk_ref"], f"{context}.vk_ref"
        )
        if artifact == vk_ref.commitment:
            raise TypeError(f"{context} reuses a governed hash role")
        return cls(
            version,
            semantics,
            artifact,
            vk_ref,
            _sccp_uint(
                record["gas_limit"],
                f"{context}.gas_limit",
                _SCCP_MAX_SORA_OUTBOUND_GAS_LIMIT,
                positive=True,
            ),
        )


@dataclass(frozen=True)
class GovernanceSccpEvmDestinationDeployment:
    """Exact EVM verifier, route, and ERC-20 deployment identity."""

    token_address: str
    token_code_hash: str
    verifier_address: str
    verifier_code_hash: str
    verifying_key: GovernanceSccpGroth16Bn254VerifyingKey
    verifier_key_hash: str
    outbound_proof_policy: GovernanceSccpOutboundProofPolicy
    route_address: str
    route_code_hash: str
    replay_verifier_address: str
    replay_verifier_code_hash: str
    mint_breaker_address: str
    mint_breaker_code_hash: str
    taira_to_token_multiplier: int
    max_wrapped_supply: int

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpEvmDestinationDeployment":
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
                "replay_verifier_address",
                "replay_verifier_code_hash",
                "mint_breaker_address",
                "mint_breaker_code_hash",
                "taira_to_token_multiplier",
                "max_wrapped_supply",
            }
        )
        record = _exact(value, fields, context)
        multiplier = _sccp_uint(
            record["taira_to_token_multiplier"],
            f"{context}.taira_to_token_multiplier",
            positive=True,
        )
        if multiplier != 1_000_000_000:
            raise TypeError(f"{context}.taira_to_token_multiplier must equal 1000000000")
        addresses = tuple(
            _upper_hex(record[field], f"{context}.{field}", 20)
            for field in (
                "token_address",
                "verifier_address",
                "route_address",
                "replay_verifier_address",
                "mint_breaker_address",
            )
        )
        hashes = tuple(
            _upper_hex(record[field], f"{context}.{field}", 32)
            for field in (
                "token_code_hash",
                "verifier_code_hash",
                "verifier_key_hash",
                "route_code_hash",
                "replay_verifier_code_hash",
                "mint_breaker_code_hash",
            )
        )
        if len(set(addresses)) != len(addresses) or len(set(hashes)) != len(hashes):
            raise TypeError(f"{context} reuses a role-separated address or hash")
        if any(hashes[index] == _KECCAK256_EMPTY_HEX for index in (0, 1, 3, 4, 5)):
            raise TypeError(f"{context} runtime code hash identifies empty bytecode")
        key = GovernanceSccpGroth16Bn254VerifyingKey.from_payload(
            record["verifying_key"], f"{context}.verifying_key"
        )
        policy = GovernanceSccpOutboundProofPolicy.from_payload(
            record["outbound_proof_policy"], f"{context}.outbound_proof_policy"
        )
        if (
            policy.semantic_profile.profile
            is not GovernanceSccpSemanticProofProfileKind.SORA_TAIRA_FINALITY_INCLUSION_GROTH16_BN254
        ):
            raise TypeError(f"{context}.outbound_proof_policy selects the wrong curve")
        return cls(
            addresses[0],
            hashes[0],
            addresses[1],
            hashes[1],
            key,
            hashes[2],
            policy,
            addresses[2],
            hashes[3],
            addresses[3],
            hashes[4],
            addresses[4],
            hashes[5],
            multiplier,
            _decimal_u128(
                record["max_wrapped_supply"],
                f"{context}.max_wrapped_supply",
                positive=True,
            ),
        )


@dataclass(frozen=True)
class GovernanceSccpTronDestinationDeployment:
    """Exact TRON verifier, route, and TRC-20 deployment identity."""

    token_address: str
    token_code_hash: str
    verifier_address: str
    verifier_code_hash: str
    verifying_key: GovernanceSccpGroth16Bn254VerifyingKey
    verifier_key_hash: str
    outbound_proof_policy: GovernanceSccpOutboundProofPolicy
    route_address: str
    route_code_hash: str
    replay_verifier_address: str
    replay_verifier_code_hash: str
    mint_breaker_address: str
    mint_breaker_code_hash: str
    taira_to_token_multiplier: int
    max_wrapped_supply: int

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpTronDestinationDeployment":
        parsed = GovernanceSccpEvmDestinationDeployment.from_payload(value, context)
        return cls(
            parsed.token_address,
            parsed.token_code_hash,
            parsed.verifier_address,
            parsed.verifier_code_hash,
            parsed.verifying_key,
            parsed.verifier_key_hash,
            parsed.outbound_proof_policy,
            parsed.route_address,
            parsed.route_code_hash,
            parsed.replay_verifier_address,
            parsed.replay_verifier_code_hash,
            parsed.mint_breaker_address,
            parsed.mint_breaker_code_hash,
            parsed.taira_to_token_multiplier,
            parsed.max_wrapped_supply,
        )


@dataclass(frozen=True)
class GovernanceSccpTonMintBreakerGuardianKeys:
    """Exact ordered five-key TON mint-breaker guardian set."""

    guardian_0: str
    guardian_1: str
    guardian_2: str
    guardian_3: str
    guardian_4: str

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpTonMintBreakerGuardianKeys":
        fields = frozenset(f"guardian_{index}" for index in range(5))
        record = _exact(value, fields, context)
        keys = tuple(
            _upper_hex(record[f"guardian_{index}"], f"{context}.guardian_{index}", 32)
            for index in range(5)
        )
        if any(left >= right for left, right in zip(keys, keys[1:])):
            raise TypeError(f"{context} must be strictly lexicographically increasing")
        return cls(*keys)

    def ordered(self) -> tuple[str, str, str, str, str]:
        """Return the exact commitment order."""

        return (
            self.guardian_0,
            self.guardian_1,
            self.guardian_2,
            self.guardian_3,
            self.guardian_4,
        )


@dataclass(frozen=True)
class GovernanceSccpTonDestinationDeployment:
    """Exact TON Jetton, route, verifier, and breaker deployment identity."""

    jetton_master_address: GovernanceSccpTonAddress
    jetton_master_code_hash: str
    jetton_master_initial_data_hash: str
    jetton_wallet_code_hash: str
    route_address: GovernanceSccpTonAddress
    route_code_hash: str
    route_initial_data_hash: str
    embedded_verifier_code_hash: str
    verifier_circuit_hash: str
    verifying_key: Mapping[str, Any]
    verifier_key_hash: str
    proof_profile_commitment: str
    mint_breaker_guardian_keys: GovernanceSccpTonMintBreakerGuardianKeys
    outbound_proof_policy: GovernanceSccpOutboundProofPolicy
    taira_to_token_multiplier: int
    max_wrapped_supply: int

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpTonDestinationDeployment":
        fields = frozenset(
            {
                "jetton_master_address",
                "jetton_master_code_hash",
                "jetton_master_initial_data_hash",
                "jetton_wallet_code_hash",
                "route_address",
                "route_code_hash",
                "route_initial_data_hash",
                "embedded_verifier_code_hash",
                "verifier_circuit_hash",
                "verifying_key",
                "verifier_key_hash",
                "proof_profile_commitment",
                "mint_breaker_guardian_keys",
                "outbound_proof_policy",
                "taira_to_token_multiplier",
                "max_wrapped_supply",
            }
        )
        record = _exact(value, fields, context)
        master = GovernanceSccpTonAddress.from_payload(
            record["jetton_master_address"], f"{context}.jetton_master_address"
        )
        route = GovernanceSccpTonAddress.from_payload(
            record["route_address"], f"{context}.route_address"
        )
        if master == route:
            raise TypeError(f"{context} reuses a TON contract address")
        hash_fields = (
            "jetton_master_code_hash",
            "jetton_master_initial_data_hash",
            "jetton_wallet_code_hash",
            "route_code_hash",
            "route_initial_data_hash",
            "embedded_verifier_code_hash",
            "verifier_circuit_hash",
            "verifier_key_hash",
            "proof_profile_commitment",
        )
        hashes = {
            field: _upper_hex(record[field], f"{context}.{field}", 32)
            for field in hash_fields
        }
        if len(set(hashes.values())) != len(hashes):
            raise TypeError(f"{context} reuses a TON deployment hash role")
        from . import sccp as _sccp

        key_bytes = _sccp._bls12381_verifying_key(  # noqa: SLF001
            record["verifying_key"], f"{context}.verifying_key"
        )
        if hashlib.sha256(key_bytes).hexdigest().upper() != hashes["verifier_key_hash"]:
            raise TypeError(f"{context}.verifier_key_hash does not match verifying_key")
        if (
            _sccp._ton_proof_profile_commitment().hex().upper()  # noqa: SLF001
            != hashes["proof_profile_commitment"]
        ):
            raise TypeError(f"{context}.proof_profile_commitment is not canonical")
        policy = GovernanceSccpOutboundProofPolicy.from_payload(
            record["outbound_proof_policy"], f"{context}.outbound_proof_policy"
        )
        if (
            policy.semantic_profile.profile
            is not GovernanceSccpSemanticProofProfileKind.SORA_TAIRA_FINALITY_INCLUSION_GROTH16_BLS12381
            or policy.semantic_profile.commitments.circuit_commitment
            != hashes["verifier_circuit_hash"]
        ):
            raise TypeError(f"{context} verifier circuit and proof profile disagree")
        multiplier = _sccp_uint(
            record["taira_to_token_multiplier"],
            f"{context}.taira_to_token_multiplier",
            positive=True,
        )
        if multiplier != 1:
            raise TypeError(f"{context}.taira_to_token_multiplier must equal 1")
        return cls(
            master,
            hashes["jetton_master_code_hash"],
            hashes["jetton_master_initial_data_hash"],
            hashes["jetton_wallet_code_hash"],
            route,
            hashes["route_code_hash"],
            hashes["route_initial_data_hash"],
            hashes["embedded_verifier_code_hash"],
            hashes["verifier_circuit_hash"],
            cast(Mapping[str, Any], _freeze(record["verifying_key"])),
            hashes["verifier_key_hash"],
            hashes["proof_profile_commitment"],
            GovernanceSccpTonMintBreakerGuardianKeys.from_payload(
                record["mint_breaker_guardian_keys"],
                f"{context}.mint_breaker_guardian_keys",
            ),
            policy,
            multiplier,
            _decimal_u128(
                record["max_wrapped_supply"],
                f"{context}.max_wrapped_supply",
                positive=True,
                maximum=_TON_MAX_COINS,
            ),
        )


class GovernanceSccpDestinationDeploymentKind(str, Enum):
    """Closed destination-deployment family tags."""

    EVM = "evm"
    TRON = "tron"
    TON = "ton"


GovernanceSccpDestinationDeploymentValue = Union[
    GovernanceSccpEvmDestinationDeployment,
    GovernanceSccpTronDestinationDeployment,
    GovernanceSccpTonDestinationDeployment,
]


@dataclass(frozen=True)
class GovernanceSccpDestinationDeployment:
    """Exact adjacently tagged destination-deployment union."""

    family: GovernanceSccpDestinationDeploymentKind
    deployment: GovernanceSccpDestinationDeploymentValue

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpDestinationDeployment":
        record = _exact(value, frozenset({"family", "deployment"}), context)
        try:
            family = GovernanceSccpDestinationDeploymentKind(record["family"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.family is unsupported or retired") from exc
        parser = {
            GovernanceSccpDestinationDeploymentKind.EVM: (
                GovernanceSccpEvmDestinationDeployment.from_payload
            ),
            GovernanceSccpDestinationDeploymentKind.TRON: (
                GovernanceSccpTronDestinationDeployment.from_payload
            ),
            GovernanceSccpDestinationDeploymentKind.TON: (
                GovernanceSccpTonDestinationDeployment.from_payload
            ),
        }[family]
        deployment = cast(
            GovernanceSccpDestinationDeploymentValue,
            parser(record["deployment"], f"{context}.deployment"),
        )
        return cls(family, deployment)


@dataclass(frozen=True)
class GovernanceSccpSoraSettlement:
    """Typed SORA-side derived-escrow liability policy for SCCP settlement."""

    asset_definition_id: str
    payload_amount_scale: int
    max_outstanding_liability: int

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpSoraSettlement":
        record = _exact(
            value,
            frozenset(
                {
                    "asset_definition_id",
                    "payload_amount_scale",
                    "max_outstanding_liability",
                }
            ),
            context,
        )
        asset = _string(record["asset_definition_id"], f"{context}.asset_definition_id")
        if asset != _SCCP_TAIRA_XOR_ASSET_DEFINITION_ID:
            raise TypeError(f"{context}.asset_definition_id is not the first-release Taira XOR")
        scale = _sccp_uint(
            record["payload_amount_scale"], f"{context}.payload_amount_scale", _U32_MAX
        )
        if scale != 9:
            raise TypeError(f"{context}.payload_amount_scale must be the integer 9")
        return cls(
            asset,
            scale,
            _decimal_u128(
                record["max_outstanding_liability"],
                f"{context}.max_outstanding_liability",
                positive=True,
            ),
        )


@dataclass(frozen=True)
class GovernanceSccpGovernedRoute:
    """One complete recursively typed immutable SCCP route."""

    lane_id: GovernanceSccpLaneId
    route_id: str
    asset_key: str
    revision: int
    activation: GovernanceSccpRouteActivation
    inbound_finality_cutoff: Optional[GovernanceSccpInboundFinalityCutoff]
    source_identity: GovernanceSccpSourceIdentity
    destination: GovernanceSccpDestinationDeployment
    sora_outbound_execution_policy: GovernanceSccpSoraOutboundExecutionPolicy
    settlement: GovernanceSccpSoraSettlement

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpGovernedRoute":
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
                "sora_outbound_execution_policy",
                "settlement",
            }
        )
        record = _exact(value, fields, context)
        lane = GovernanceSccpLaneId.from_payload(record["lane_id"], f"{context}.lane_id")
        route_id = _sccp_route_token(record["route_id"], f"{context}.route_id")
        asset_key = _sccp_route_token(record["asset_key"], f"{context}.asset_key")
        revision = _sccp_uint(
            record["revision"], f"{context}.revision", _U32_MAX, positive=True
        )
        if route_id != _SCCP_EXACT_ROUTE_ID[lane.source.network] or asset_key != "xor":
            raise TypeError(f"{context} does not identify the exact first-release XOR route")
        activation = GovernanceSccpRouteActivation.from_payload(
            record["activation"], f"{context}.activation"
        )
        cutoff = (
            None
            if record["inbound_finality_cutoff"] is None
            else GovernanceSccpInboundFinalityCutoff.from_payload(
                record["inbound_finality_cutoff"], f"{context}.inbound_finality_cutoff"
            )
        )
        if (activation.activation is GovernanceSccpRouteActivationKind.RETIRED) != (
            cutoff is not None
        ):
            raise TypeError(
                f"{context}.inbound_finality_cutoff must be present exactly for retired activation"
            )
        source = GovernanceSccpSourceIdentity.from_payload(
            record["source_identity"], f"{context}.source_identity"
        )
        if source.lane != lane:
            raise TypeError(f"{context}.source_identity.lane does not match lane_id")
        destination = GovernanceSccpDestinationDeployment.from_payload(
            record["destination"], f"{context}.destination"
        )
        if destination.family.value != lane.source.family:
            raise TypeError(f"{context}.destination family does not match lane_id")
        if source.emitter.emitter.value != destination.family.value:
            raise TypeError(f"{context} source and destination families disagree")
        if isinstance(source.emitter.identity, GovernanceSccpEvmSourceEmitter) and isinstance(
            destination.deployment, GovernanceSccpEvmDestinationDeployment
        ):
            if (
                source.emitter.identity.address != destination.deployment.route_address
                or source.emitter.identity.runtime_code_hash
                != destination.deployment.route_code_hash
            ):
                raise TypeError(f"{context} source emitter does not identify the route deployment")
        if isinstance(source.emitter.identity, GovernanceSccpTronSourceEmitter) and isinstance(
            destination.deployment, GovernanceSccpTronDestinationDeployment
        ):
            if (
                source.emitter.identity.address != destination.deployment.route_address
                or source.emitter.identity.runtime_code_hash
                != destination.deployment.route_code_hash
            ):
                raise TypeError(f"{context} source emitter does not identify the route deployment")
        if isinstance(source.emitter.identity, GovernanceSccpTonSourceEmitter) and isinstance(
            destination.deployment, GovernanceSccpTonDestinationDeployment
        ):
            if (
                source.emitter.identity.address != destination.deployment.route_address
                or source.emitter.identity.code_hash
                != destination.deployment.route_code_hash
            ):
                raise TypeError(f"{context} source emitter does not identify the route deployment")
        settlement = GovernanceSccpSoraSettlement.from_payload(
            record["settlement"], f"{context}.settlement"
        )
        if (
            settlement.max_outstanding_liability
            * destination.deployment.taira_to_token_multiplier
            != destination.deployment.max_wrapped_supply
        ):
            raise TypeError(
                f"{context} wrapped-supply cap does not match the liability cap"
            )
        return cls(
            lane,
            route_id,
            asset_key,
            revision,
            activation,
            cutoff,
            source,
            destination,
            GovernanceSccpSoraOutboundExecutionPolicy.from_payload(
                record["sora_outbound_execution_policy"],
                f"{context}.sora_outbound_execution_policy",
            ),
            settlement,
        )


@dataclass(frozen=True)
class GovernanceSccpRegisterRoute:
    """Atomic registration input for one complete staged route."""

    route: GovernanceSccpGovernedRoute
    native_trust_anchor: Optional[GovernanceSccpNativeTrustAnchor]

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpRegisterRoute":
        record = _exact(value, frozenset({"route", "native_trust_anchor"}), context)
        route = GovernanceSccpGovernedRoute.from_payload(record["route"], f"{context}.route")
        if route.activation.activation is not GovernanceSccpRouteActivationKind.STAGED:
            raise TypeError(f"{context}.route.activation must be staged at registration")
        anchor = (
            None
            if record["native_trust_anchor"] is None
            else GovernanceSccpNativeTrustAnchor.from_payload(
                record["native_trust_anchor"], f"{context}.native_trust_anchor"
            )
        )
        if anchor is not None and not anchor.supports(route.lane_id):
            raise TypeError(f"{context}.native_trust_anchor backend does not match the route lane")
        return cls(route, anchor)


@dataclass(frozen=True)
class GovernanceSccpSetRouteActivation:
    """Compare-and-swap activation update for one exact route."""

    key: GovernanceSccpRouteKey
    expected_current: GovernanceSccpRouteActivation
    next: GovernanceSccpRouteActivation
    inbound_finality_cutoff: Optional[GovernanceSccpInboundFinalityCutoff]

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpSetRouteActivation":
        record = _exact(
            value,
            frozenset({"key", "expected_current", "next", "inbound_finality_cutoff"}),
            context,
        )
        next_activation = GovernanceSccpRouteActivation.from_payload(
            record["next"], f"{context}.next"
        )
        cutoff = (
            None
            if record["inbound_finality_cutoff"] is None
            else GovernanceSccpInboundFinalityCutoff.from_payload(
                record["inbound_finality_cutoff"], f"{context}.inbound_finality_cutoff"
            )
        )
        if (next_activation.activation is GovernanceSccpRouteActivationKind.RETIRED) != (
            cutoff is not None
        ):
            raise TypeError(
                f"{context}.inbound_finality_cutoff must be present exactly for retired next state"
            )
        return cls(
            GovernanceSccpRouteKey.from_payload(record["key"], f"{context}.key"),
            GovernanceSccpRouteActivation.from_payload(
                record["expected_current"], f"{context}.expected_current"
            ),
            next_activation,
            cutoff,
        )


@dataclass(frozen=True)
class GovernanceSccpSwitchRouteRevision:
    """Atomic cutover between immutable route revisions."""

    previous_key: GovernanceSccpRouteKey
    expected_previous: GovernanceSccpRouteActivation
    previous_next: GovernanceSccpRouteActivation
    previous_inbound_finality_cutoff: Optional[GovernanceSccpInboundFinalityCutoff]
    successor_key: GovernanceSccpRouteKey
    successor_next: GovernanceSccpRouteActivation

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpSwitchRouteRevision":
        fields = frozenset(
            {
                "previous_key",
                "expected_previous",
                "previous_next",
                "previous_inbound_finality_cutoff",
                "successor_key",
                "successor_next",
            }
        )
        record = _exact(value, fields, context)
        previous_next = GovernanceSccpRouteActivation.from_payload(
            record["previous_next"], f"{context}.previous_next"
        )
        cutoff = (
            None
            if record["previous_inbound_finality_cutoff"] is None
            else GovernanceSccpInboundFinalityCutoff.from_payload(
                record["previous_inbound_finality_cutoff"],
                f"{context}.previous_inbound_finality_cutoff",
            )
        )
        if (previous_next.activation is GovernanceSccpRouteActivationKind.RETIRED) != (
            cutoff is not None
        ):
            raise TypeError(
                f"{context}.previous_inbound_finality_cutoff must be present exactly for retirement"
            )
        previous = GovernanceSccpRouteKey.from_payload(
            record["previous_key"], f"{context}.previous_key"
        )
        successor = GovernanceSccpRouteKey.from_payload(
            record["successor_key"], f"{context}.successor_key"
        )
        if (
            previous.lane_id != successor.lane_id
            or previous.route_id != successor.route_id
            or previous.asset_key != successor.asset_key
            or successor.revision != previous.revision + 1
        ):
            raise TypeError(f"{context}.successor_key is not the next revision of previous_key")
        successor_next = GovernanceSccpRouteActivation.from_payload(
            record["successor_next"], f"{context}.successor_next"
        )
        if successor_next.activation is not GovernanceSccpRouteActivationKind.BIDIRECTIONAL:
            raise TypeError(f"{context}.successor_next must be bidirectional")
        return cls(
            previous,
            GovernanceSccpRouteActivation.from_payload(
                record["expected_previous"], f"{context}.expected_previous"
            ),
            previous_next,
            cutoff,
            successor,
            successor_next,
        )


@dataclass(frozen=True)
class GovernanceSccpInitializeLaneTrustAnchor:
    """First native trust-anchor installation for one lane."""

    lane_id: GovernanceSccpLaneId
    expected_current: None
    initial: GovernanceSccpNativeTrustAnchor

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceSccpInitializeLaneTrustAnchor":
        record = _exact(value, frozenset({"lane_id", "expected_current", "initial"}), context)
        if record["expected_current"] is not None:
            raise TypeError(f"{context}.expected_current must be null for initialization")
        lane = GovernanceSccpLaneId.from_payload(record["lane_id"], f"{context}.lane_id")
        initial = GovernanceSccpNativeTrustAnchor.from_payload(
            record["initial"], f"{context}.initial"
        )
        if not initial.supports(lane):
            raise TypeError(f"{context}.initial backend does not match lane_id")
        return cls(lane, None, initial)


@dataclass(frozen=True)
class GovernanceSccpAdvanceLaneTrustAnchor:
    """Append-only native trust-anchor update for one lane."""

    lane_id: GovernanceSccpLaneId
    expected_current: GovernanceSccpNativeTrustAnchor
    next: GovernanceSccpNativeTrustAnchor

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceSccpAdvanceLaneTrustAnchor":
        record = _exact(value, frozenset({"lane_id", "expected_current", "next"}), context)
        lane = GovernanceSccpLaneId.from_payload(record["lane_id"], f"{context}.lane_id")
        expected = GovernanceSccpNativeTrustAnchor.from_payload(
            record["expected_current"], f"{context}.expected_current"
        )
        next_anchor = GovernanceSccpNativeTrustAnchor.from_payload(
            record["next"], f"{context}.next"
        )
        if (
            not expected.supports(lane)
            or not next_anchor.supports(lane)
            or expected.backend != next_anchor.backend
            or expected.anchor_hash == next_anchor.anchor_hash
            or next_anchor.checkpoint_height <= expected.checkpoint_height
        ):
            raise TypeError(f"{context} is not a monotonic same-backend anchor advance")
        return cls(lane, expected, next_anchor)


GovernanceSccpRouteActionValue = Union[
    GovernanceSccpRegisterRoute,
    GovernanceSccpSetRouteActivation,
    GovernanceSccpSwitchRouteRevision,
    GovernanceSccpInitializeLaneTrustAnchor,
    GovernanceSccpAdvanceLaneTrustAnchor,
    GovernanceSccpRouteKey,
]


@dataclass(frozen=True)
class GovernanceSccpRouteAction:
    """Closed canonical SCCP route action with a typed variant payload."""

    action: GovernanceSccpRouteActionKind
    route: GovernanceSccpRouteActionValue

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceSccpRouteAction":
        context = "SccpRouteGovernance payload.anchor.action"
        record = _exact(value, frozenset({"action", "route"}), context)
        try:
            action = GovernanceSccpRouteActionKind(record["action"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.action is not a first-release SCCP action") from exc
        parser = {
            GovernanceSccpRouteActionKind.REGISTER: GovernanceSccpRegisterRoute.from_payload,
            GovernanceSccpRouteActionKind.SET_ACTIVATION: (
                GovernanceSccpSetRouteActivation.from_payload
            ),
            GovernanceSccpRouteActionKind.SWITCH_REVISION: (
                GovernanceSccpSwitchRouteRevision.from_payload
            ),
            GovernanceSccpRouteActionKind.INITIALIZE_TRUST_ANCHOR: (
                GovernanceSccpInitializeLaneTrustAnchor.from_payload
            ),
            GovernanceSccpRouteActionKind.ADVANCE_TRUST_ANCHOR: (
                GovernanceSccpAdvanceLaneTrustAnchor.from_payload
            ),
            GovernanceSccpRouteActionKind.REMOVE: GovernanceSccpRouteKey.from_payload,
        }[action]
        route = cast(
            GovernanceSccpRouteActionValue,
            parser(record["route"], f"{context}.route"),
        )
        return cls(action, route)


@dataclass(frozen=True)
class GovernanceProposalSccpRouteGovernance:
    """Canonical `SccpRouteGovernanceProposal` payload."""

    network_id: str
    action: GovernanceSccpRouteAction

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalSccpRouteGovernance":
        payload = _exact(value, frozenset({"anchor"}), "SccpRouteGovernance payload")
        anchor = _exact(payload["anchor"], frozenset({"network_id", "action"}), "SccpRouteGovernance payload.anchor")
        return cls(_network_id(anchor["network_id"], "SccpRouteGovernance payload.anchor.network_id"), GovernanceSccpRouteAction.from_payload(anchor["action"]))


class GovernanceValidationFeeChargingMode(str, Enum):
    """Closed validation-fee charging modes."""

    DISABLED = "DISABLED"
    PER_QUALIFYING_TRANSFER_INSTRUCTION = "PER_QUALIFYING_TRANSFER_INSTRUCTION"


@dataclass(frozen=True)
class GovernanceValidationFeePayoutRecipient:
    """One immutable treasury-payout recipient."""

    account_id: str
    share: str


@dataclass(frozen=True)
class GovernanceValidationFeePayoutBinding:
    """Exact validation-fee payout lifecycle binding."""

    contract_address: str
    code_hash: tuple[int, ...]
    entrypoint: str
    treasury_account_id: str
    ds_asset_id: str
    xor_asset_id: str
    pool_vault_account_id: str
    batch_ds: str
    min_xor_out: str
    max_xor_out: str
    recipients: tuple[GovernanceValidationFeePayoutRecipient, ...]

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceValidationFeePayoutBinding":
        fields = frozenset({"contract_address", "code_hash", "entrypoint", "treasury_account_id", "ds_asset_id", "xor_asset_id", "pool_vault_account_id", "batch_ds", "min_xor_out", "max_xor_out", "recipients"})
        record = _exact(value, fields, context)
        if not isinstance(record["recipients"], list):
            raise TypeError(f"{context}.recipients must be an array")
        recipients = []
        for index, item in enumerate(record["recipients"]):
            item_context = f"{context}.recipients[{index}]"
            recipient = _exact(item, frozenset({"account_id", "share"}), item_context)
            recipients.append(GovernanceValidationFeePayoutRecipient(_account_id(recipient["account_id"], f"{item_context}.account_id"), _numeric(recipient["share"], f"{item_context}.share")))
        return cls(_contract_address(record["contract_address"], f"{context}.contract_address"), _bytes32(record["code_hash"], f"{context}.code_hash", nonzero=True), _string(record["entrypoint"], f"{context}.entrypoint"), _account_id(record["treasury_account_id"], f"{context}.treasury_account_id"), _asset_definition_id(record["ds_asset_id"], f"{context}.ds_asset_id"), _asset_definition_id(record["xor_asset_id"], f"{context}.xor_asset_id"), _account_id(record["pool_vault_account_id"], f"{context}.pool_vault_account_id"), _numeric(record["batch_ds"], f"{context}.batch_ds"), _numeric(record["min_xor_out"], f"{context}.min_xor_out"), _numeric(record["max_xor_out"], f"{context}.max_xor_out"), tuple(recipients))


@dataclass(frozen=True)
class GovernanceValidationFeePolicy:
    """Complete exact-network validation-fee policy."""

    schema_version: int
    network_id: str
    policy_version: int
    previous_policy_hash: Optional[tuple[int, ...]]
    ds_asset_id: str
    ds_scale: int
    fee: str
    treasury_account_id: str
    charging_mode: GovernanceValidationFeeChargingMode
    effective_from_height: int
    expires_after_height: Optional[int]
    exemption_classes: tuple[str, ...]
    treasury_payout_binding: Optional[GovernanceValidationFeePayoutBinding]

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceValidationFeePolicy":
        context = "ValidationFeePolicy payload.policy"
        fields = frozenset({"schema_version", "network_id", "policy_version", "previous_policy_hash", "ds_asset_id", "ds_scale", "fee", "treasury_account_id", "charging_mode", "effective_from_height", "expires_after_height", "exemption_classes", "treasury_payout_binding"})
        record = _exact(value, fields, context)
        if _uint(record["schema_version"], f"{context}.schema_version", 1) != 1:
            raise TypeError(f"{context}.schema_version must be 1")
        mode = _exact(record["charging_mode"], frozenset({"charging_mode", "value"}), f"{context}.charging_mode")
        if mode["value"] is not None:
            raise TypeError(f"{context}.charging_mode.value must be null")
        try:
            charging_mode = GovernanceValidationFeeChargingMode(mode["charging_mode"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.charging_mode is unsupported") from exc
        if not isinstance(record["exemption_classes"], list):
            raise TypeError(f"{context}.exemption_classes must be an array")
        previous = None if record["previous_policy_hash"] is None else _bytes32(record["previous_policy_hash"], f"{context}.previous_policy_hash")
        expires = None if record["expires_after_height"] is None else _decimal_u64(record["expires_after_height"], f"{context}.expires_after_height")
        binding = None if record["treasury_payout_binding"] is None else GovernanceValidationFeePayoutBinding.from_payload(record["treasury_payout_binding"], f"{context}.treasury_payout_binding")
        return cls(1, _network_id(record["network_id"], f"{context}.network_id"), _decimal_u64(record["policy_version"], f"{context}.policy_version", positive=True), previous, _asset_definition_id(record["ds_asset_id"], f"{context}.ds_asset_id"), _uint(record["ds_scale"], f"{context}.ds_scale", 255), _numeric(record["fee"], f"{context}.fee"), _account_id(record["treasury_account_id"], f"{context}.treasury_account_id"), charging_mode, _decimal_u64(record["effective_from_height"], f"{context}.effective_from_height"), expires, tuple(_string(item, f"{context}.exemption_classes[{index}]") for index, item in enumerate(record["exemption_classes"])), binding)


@dataclass(frozen=True)
class GovernanceProposalValidationFeePolicy:
    """Canonical `ValidationFeePolicyProposal` payload."""

    proposal_operator: str
    policy: GovernanceValidationFeePolicy
    payout_lifecycle_proposal_id: Optional[tuple[int, ...]]

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalValidationFeePolicy":
        context = "ValidationFeePolicy payload"
        record = _exact(value, frozenset({"proposal_operator", "policy", "payout_lifecycle_proposal_id"}), context)
        lifecycle = None if record["payout_lifecycle_proposal_id"] is None else _bytes32(record["payout_lifecycle_proposal_id"], f"{context}.payout_lifecycle_proposal_id")
        return cls(_account_id(record["proposal_operator"], f"{context}.proposal_operator"), GovernanceValidationFeePolicy.from_payload(record["policy"]), lifecycle)


@dataclass(frozen=True)
class GovernanceProposalValidationFeePayoutLifecycle:
    """Canonical `ValidationFeePayoutLifecycleProposal` payload."""

    proposal_operator: str
    payout_binding: GovernanceValidationFeePayoutBinding

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalValidationFeePayoutLifecycle":
        context = "ValidationFeePayoutLifecycle payload"
        record = _exact(value, frozenset({"proposal_operator", "payout_binding"}), context)
        return cls(_account_id(record["proposal_operator"], f"{context}.proposal_operator"), GovernanceValidationFeePayoutBinding.from_payload(record["payout_binding"], f"{context}.payout_binding"))


class GovernanceMusubiActionKind(str, Enum):
    """Closed Musubi Parliament action tags."""

    RECOVER_PACKAGE_OWNERS = "RecoverPackageOwners"
    RETARGET_ALIAS = "RetargetAlias"
    TAKEDOWN_ARTIFACT = "TakedownArtifact"
    SET_REGISTRY_POLICY = "SetRegistryPolicy"


class GovernanceMusubiPackageScopeKind(str, Enum):
    """Closed Musubi structural package scopes."""

    DATASPACE_ROOT = "DataspaceRoot"
    DOMAIN = "Domain"


@dataclass(frozen=True)
class GovernanceMusubiPackageScope:
    """One exact structural package scope."""

    kind: GovernanceMusubiPackageScopeKind
    value: Optional[str]

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceMusubiPackageScope":
        record = _exact(value, frozenset({"kind", "value"}), context)
        try:
            kind = GovernanceMusubiPackageScopeKind(record["kind"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.kind is unsupported") from exc
        if kind is GovernanceMusubiPackageScopeKind.DATASPACE_ROOT:
            if record["value"] is not None:
                raise TypeError(f"{context}.value must be null for DataspaceRoot")
            return cls(kind, None)
        return cls(kind, _iroha_name(record["value"], f"{context}.value"))


@dataclass(frozen=True)
class GovernanceMusubiPackageId:
    """Canonical stable structural package identifier."""

    home_dataspace: int
    scope: GovernanceMusubiPackageScope
    name: str

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceMusubiPackageId":
        record = _exact(value, frozenset({"home_dataspace", "scope", "name"}), context)
        name = _string_tuple(record["name"], f"{context}.name")
        return cls(
            _proposal_exact_json_uint(
                record["home_dataspace"], f"{context}.home_dataspace"
            ),
            GovernanceMusubiPackageScope.from_payload(record["scope"], f"{context}.scope"),
            _ascii_kebab(name, f"{context}.name[0]", 64),
        )


class GovernanceMusubiPrereleaseIdentifierKind(str, Enum):
    """Closed Musubi semantic-version prerelease identifier tags."""

    NUMERIC = "Numeric"
    ALPHA_NUMERIC = "AlphaNumeric"


@dataclass(frozen=True)
class GovernanceMusubiPrereleaseIdentifier:
    """One canonical Musubi semantic-version prerelease identifier."""

    kind: GovernanceMusubiPrereleaseIdentifierKind
    value: Union[int, str]

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceMusubiPrereleaseIdentifier":
        record = _exact(value, frozenset({"kind", "value"}), context)
        try:
            kind = GovernanceMusubiPrereleaseIdentifierKind(record["kind"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.kind is unsupported") from exc
        if kind is GovernanceMusubiPrereleaseIdentifierKind.NUMERIC:
            return cls(
                kind,
                _proposal_exact_json_uint(record["value"], f"{context}.value"),
            )
        literal = _string(record["value"], f"{context}.value")
        if (
            len(literal.encode("ascii", errors="ignore")) != len(literal)
            or len(literal) > 64
            or re.fullmatch(r"[A-Za-z0-9-]+", literal) is None
            or literal.isdigit()
        ):
            raise TypeError(f"{context}.value is not a canonical alphanumeric identifier")
        return cls(kind, literal)


@dataclass(frozen=True)
class GovernanceMusubiVersion:
    """Canonical structured Musubi semantic version."""

    major: int
    minor: int
    patch: int
    prerelease: tuple[GovernanceMusubiPrereleaseIdentifier, ...]

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceMusubiVersion":
        record = _exact(
            value, frozenset({"major", "minor", "patch", "prerelease"}), context
        )
        if not isinstance(record["prerelease"], list):
            raise TypeError(f"{context}.prerelease must be an array")
        if len(record["prerelease"]) > 16:
            raise TypeError(f"{context}.prerelease exceeds the V1 bound")
        prerelease = tuple(
            GovernanceMusubiPrereleaseIdentifier.from_payload(
                item, f"{context}.prerelease[{index}]"
            )
            for index, item in enumerate(record["prerelease"])
        )
        return cls(
            _proposal_exact_json_uint(record["major"], f"{context}.major"),
            _proposal_exact_json_uint(record["minor"], f"{context}.minor"),
            _proposal_exact_json_uint(record["patch"], f"{context}.patch"),
            prerelease,
        )


@dataclass(frozen=True)
class GovernanceMusubiReleaseId:
    """Exact structural Musubi release identifier."""

    package: GovernanceMusubiPackageId
    version: GovernanceMusubiVersion

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceMusubiReleaseId":
        record = _exact(value, frozenset({"package", "version"}), context)
        return cls(
            GovernanceMusubiPackageId.from_payload(record["package"], f"{context}.package"),
            GovernanceMusubiVersion.from_payload(record["version"], f"{context}.version"),
        )


class GovernanceMusubiRegistryAdmissionMode(str, Enum):
    """Closed Musubi registry admission modes."""

    CLOSED = "Closed"
    ALLOWLISTED = "Allowlisted"
    OPEN = "Open"


@dataclass(frozen=True)
class GovernanceMusubiAliasPricingPolicy:
    """Canonical prospective alias pricing policy."""

    revision: int
    length_1_xor: int
    length_2_xor: int
    length_3_xor: int
    length_4_xor: int
    length_5_to_32_xor: int

    @classmethod
    def from_payload(
        cls, value: Any, context: str
    ) -> "GovernanceMusubiAliasPricingPolicy":
        fields = (
            "revision",
            "length_1_xor",
            "length_2_xor",
            "length_3_xor",
            "length_4_xor",
            "length_5_to_32_xor",
        )
        record = _exact(value, frozenset(fields), context)
        return cls(
            *(
                _proposal_exact_json_uint(
                    record[field], f"{context}.{field}", positive=True
                )
                for field in fields
            )
        )


@dataclass(frozen=True)
class GovernanceMusubiRegistryPolicy:
    """Complete canonical first-release Musubi registry policy."""

    version: int
    revision: int
    mode: GovernanceMusubiRegistryAdmissionMode
    allowlisted_dataspaces: tuple[int, ...]
    alias_pricing: GovernanceMusubiAliasPricingPolicy

    @classmethod
    def from_payload(cls, value: Any, context: str) -> "GovernanceMusubiRegistryPolicy":
        fields = frozenset(
            {"version", "revision", "mode", "allowlisted_dataspaces", "alias_pricing"}
        )
        record = _exact(value, fields, context)
        if _uint(record["version"], f"{context}.version", 1) != 1:
            raise TypeError(f"{context}.version must be 1")
        mode_record = _exact(
            record["mode"], frozenset({"kind", "value"}), f"{context}.mode"
        )
        if mode_record["value"] is not None:
            raise TypeError(f"{context}.mode.value must be null")
        try:
            mode = GovernanceMusubiRegistryAdmissionMode(mode_record["kind"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.mode.kind is unsupported") from exc
        if not isinstance(record["allowlisted_dataspaces"], list):
            raise TypeError(f"{context}.allowlisted_dataspaces must be an array")
        allowlisted = tuple(
            _proposal_exact_json_uint(
                item, f"{context}.allowlisted_dataspaces[{index}]"
            )
            for index, item in enumerate(record["allowlisted_dataspaces"])
        )
        if len(allowlisted) > 1_024 or any(
            left >= right for left, right in zip(allowlisted, allowlisted[1:])
        ):
            raise TypeError(
                f"{context}.allowlisted_dataspaces must be bounded, sorted, and unique"
            )
        if mode is not GovernanceMusubiRegistryAdmissionMode.ALLOWLISTED and allowlisted:
            raise TypeError(f"{context}.allowlisted_dataspaces does not match mode")
        return cls(
            1,
            _proposal_exact_json_uint(
                record["revision"], f"{context}.revision", positive=True
            ),
            mode,
            allowlisted,
            GovernanceMusubiAliasPricingPolicy.from_payload(
                record["alias_pricing"], f"{context}.alias_pricing"
            ),
        )


@dataclass(frozen=True)
class GovernanceMusubiRecoverPackageOwners:
    """Canonical package-owner recovery payload."""

    package: GovernanceMusubiPackageId
    owners: tuple[str, ...]
    expected_revision: int


@dataclass(frozen=True)
class GovernanceMusubiRetargetAlias:
    """Canonical permanent-alias retarget payload."""

    alias: str
    target: GovernanceMusubiPackageId
    expected_revision: int


@dataclass(frozen=True)
class GovernanceMusubiTakedownArtifact:
    """Canonical immutable-artifact takedown payload."""

    release: GovernanceMusubiReleaseId
    reason: str
    expected_artifact_governance_revision: int


@dataclass(frozen=True)
class GovernanceMusubiSetRegistryPolicy:
    """Canonical prospective registry-policy replacement payload."""

    policy: GovernanceMusubiRegistryPolicy
    expected_revision: int


GovernanceMusubiActionValue = Union[
    GovernanceMusubiRecoverPackageOwners,
    GovernanceMusubiRetargetAlias,
    GovernanceMusubiTakedownArtifact,
    GovernanceMusubiSetRegistryPolicy,
]


@dataclass(frozen=True)
class GovernanceProposalMusubiRegistryGovernance:
    """Canonical closed Musubi Parliament action."""

    kind: GovernanceMusubiActionKind
    value: GovernanceMusubiActionValue

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalMusubiRegistryGovernance":
        context = "MusubiRegistryGovernance payload"
        record = _exact(value, frozenset({"kind", "value"}), context)
        try:
            kind = GovernanceMusubiActionKind(record["kind"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.kind is not a first-release Musubi action") from exc
        action_context = f"{context}.value"
        if kind is GovernanceMusubiActionKind.RECOVER_PACKAGE_OWNERS:
            action = _exact(
                record["value"],
                frozenset({"package", "owners", "expected_revision"}),
                action_context,
            )
            if not isinstance(action["owners"], list):
                raise TypeError(f"{action_context}.owners must be an array")
            owners = tuple(
                _account_id(owner, f"{action_context}.owners[{index}]")
                for index, owner in enumerate(action["owners"])
            )
            if not 1 <= len(owners) <= 64 or len(set(owners)) != len(owners):
                raise TypeError(f"{action_context}.owners must contain 1-64 unique accounts")
            payload: GovernanceMusubiActionValue = GovernanceMusubiRecoverPackageOwners(
                GovernanceMusubiPackageId.from_payload(
                    action["package"], f"{action_context}.package"
                ),
                owners,
                _proposal_exact_json_uint(
                    action["expected_revision"],
                    f"{action_context}.expected_revision",
                    positive=True,
                ),
            )
        elif kind is GovernanceMusubiActionKind.RETARGET_ALIAS:
            action = _exact(
                record["value"],
                frozenset({"alias", "target", "expected_revision"}),
                action_context,
            )
            alias = _string_tuple(action["alias"], f"{action_context}.alias")
            payload = GovernanceMusubiRetargetAlias(
                _ascii_kebab(alias, f"{action_context}.alias[0]", 32),
                GovernanceMusubiPackageId.from_payload(
                    action["target"], f"{action_context}.target"
                ),
                _proposal_exact_json_uint(
                    action["expected_revision"],
                    f"{action_context}.expected_revision",
                    positive=True,
                ),
            )
        elif kind is GovernanceMusubiActionKind.TAKEDOWN_ARTIFACT:
            action = _exact(
                record["value"],
                frozenset({"release", "reason", "expected_artifact_governance_revision"}),
                action_context,
            )
            reason = _string_tuple(action["reason"], f"{action_context}.reason")
            if len(reason.encode("utf-8")) > 1_024:
                raise TypeError(f"{action_context}.reason[0] exceeds the V1 bound")
            payload = GovernanceMusubiTakedownArtifact(
                GovernanceMusubiReleaseId.from_payload(
                    action["release"], f"{action_context}.release"
                ),
                reason,
                _proposal_exact_json_uint(
                    action["expected_artifact_governance_revision"],
                    f"{action_context}.expected_artifact_governance_revision",
                    positive=True,
                ),
            )
        else:
            action = _exact(
                record["value"],
                frozenset({"policy", "expected_revision"}),
                action_context,
            )
            expected_revision = _proposal_exact_json_uint(
                action["expected_revision"],
                f"{action_context}.expected_revision",
                positive=True,
            )
            policy = GovernanceMusubiRegistryPolicy.from_payload(
                action["policy"], f"{action_context}.policy"
            )
            if policy.revision != expected_revision + 1:
                raise TypeError(f"{action_context}.policy.revision must follow expected_revision")
            payload = GovernanceMusubiSetRegistryPolicy(policy, expected_revision)
        return cls(kind, payload)


class GovernanceSorafsProviderActionKind(str, Enum):
    """Closed SoraFS provider-owner action tags."""

    ESTABLISH = "establish"
    REBIND = "rebind"
    REMOVE = "remove"


@dataclass(frozen=True)
class GovernanceSorafsProviderAction:
    """One exact SoraFS provider-owner transition."""

    action: GovernanceSorafsProviderActionKind
    provider_id: tuple[int, ...]
    owner: Optional[str]
    expected_owner: Optional[str]
    next_owner: Optional[str]

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceSorafsProviderAction":
        context = "SorafsProviderGovernance payload.action"
        record = _exact(value, frozenset({"action", "value"}), context)
        try:
            action = GovernanceSorafsProviderActionKind(record["action"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.action is not a first-release provider action") from exc
        fields = {
            action.ESTABLISH: frozenset({"provider_id", "owner"}),
            action.REBIND: frozenset({"provider_id", "expected_owner", "next_owner"}),
            action.REMOVE: frozenset({"provider_id", "expected_owner"}),
        }[action]
        transition = _exact(record["value"], fields, f"{context}.value")
        return cls(action, _provider_id(transition["provider_id"], f"{context}.value.provider_id"), _account_id(transition["owner"], f"{context}.value.owner") if "owner" in transition else None, _account_id(transition["expected_owner"], f"{context}.value.expected_owner") if "expected_owner" in transition else None, _account_id(transition["next_owner"], f"{context}.value.next_owner") if "next_owner" in transition else None)


@dataclass(frozen=True)
class GovernanceProposalSorafsProviderGovernance:
    """Canonical `SorafsProviderGovernanceProposal` payload."""

    action: GovernanceSorafsProviderAction

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalSorafsProviderGovernance":
        record = _exact(value, frozenset({"action"}), "SorafsProviderGovernance payload")
        return cls(GovernanceSorafsProviderAction.from_payload(record["action"]))


class GovernanceContractLifecycleActionKind(str, Enum):
    """Closed contract-lifecycle Parliament action tags."""

    ACTIVATE = "Activate"
    DEACTIVATE = "Deactivate"
    OFFER_OWNERSHIP = "OfferOwnership"
    CANCEL_OWNERSHIP_OFFER = "CancelOwnershipOffer"
    ACCEPT_PARLIAMENT_OWNERSHIP = "AcceptParliamentOwnership"
    COMPLETE_EMERGENCY_HOLD_RETROSPECTIVE = "CompleteEmergencyHoldRetrospective"


@dataclass(frozen=True)
class GovernanceContractLifecycleActivate:
    """Exact governed contract activation payload."""

    code_hash: str
    abi_hash: str
    abi_version: int
    manifest_provenance: Optional[GovernanceManifestProvenance]


@dataclass(frozen=True)
class GovernanceContractLifecycleDeactivate:
    """Exact governed contract deactivation payload."""

    expected_code_hash: str
    reason: Optional[str]


@dataclass(frozen=True)
class GovernanceContractLifecycleOfferOwnership:
    """Exact governed contract ownership-offer payload."""

    new_owner: str


@dataclass(frozen=True)
class GovernanceContractLifecycleEmergencyHoldRetrospective:
    """Exact expired emergency-hold retrospective payload."""

    hold_proposal_content_id: tuple[int, ...]
    hold_governance_attempt_id: tuple[int, ...]
    incident_digest: tuple[int, ...]
    retrospective_finding_root: tuple[int, ...]


GovernanceContractLifecycleActionPayload = Union[
    GovernanceContractLifecycleActivate,
    GovernanceContractLifecycleDeactivate,
    GovernanceContractLifecycleOfferOwnership,
    GovernanceContractLifecycleEmergencyHoldRetrospective,
    None,
]


@dataclass(frozen=True)
class GovernanceContractLifecycleAction:
    """One exact adjacently-tagged lifecycle action."""

    action: GovernanceContractLifecycleActionKind
    payload: GovernanceContractLifecycleActionPayload

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceContractLifecycleAction":
        context = "ContractLifecycleGovernance payload.action"
        record = _exact(value, frozenset({"action", "payload"}), context)
        try:
            action = GovernanceContractLifecycleActionKind(record["action"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.action is not a first-release lifecycle action") from exc
        payload_context = f"{context}.payload"
        if action in {
            GovernanceContractLifecycleActionKind.CANCEL_OWNERSHIP_OFFER,
            GovernanceContractLifecycleActionKind.ACCEPT_PARLIAMENT_OWNERSHIP,
        }:
            if record["payload"] is not None:
                raise TypeError(f"{payload_context} must be null for {action.value}")
            return cls(action, None)
        if action is GovernanceContractLifecycleActionKind.ACTIVATE:
            payload = _exact(
                record["payload"],
                frozenset({"code_hash", "abi_hash", "abi_version", "manifest_provenance"}),
                payload_context,
            )
            abi_version = _proposal_exact_json_uint(
                payload["abi_version"], f"{payload_context}.abi_version", positive=True
            )
            if abi_version != 1:
                raise TypeError(f"{payload_context}.abi_version must be the integer 1")
            provenance = (
                None
                if payload["manifest_provenance"] is None
                else GovernanceManifestProvenance.from_payload(
                    payload["manifest_provenance"], f"{payload_context}.manifest_provenance"
                )
            )
            return cls(
                action,
                GovernanceContractLifecycleActivate(
                    _lower_hex32(payload["code_hash"], f"{payload_context}.code_hash"),
                    _lower_hex32(payload["abi_hash"], f"{payload_context}.abi_hash"),
                    abi_version,
                    provenance,
                ),
            )
        if action is GovernanceContractLifecycleActionKind.DEACTIVATE:
            payload = _exact(
                record["payload"],
                frozenset({"expected_code_hash", "reason"})
                if isinstance(record["payload"], Mapping) and "reason" in record["payload"]
                else frozenset({"expected_code_hash"}),
                payload_context,
            )
            reason_value = payload.get("reason")
            if reason_value is not None and not isinstance(reason_value, str):
                raise TypeError(f"{payload_context}.reason must be a string or null")
            reason = cast(Optional[str], reason_value)
            return cls(
                action,
                GovernanceContractLifecycleDeactivate(
                    _lower_hex32(
                        payload["expected_code_hash"], f"{payload_context}.expected_code_hash"
                    ),
                    reason,
                ),
            )
        if action is GovernanceContractLifecycleActionKind.OFFER_OWNERSHIP:
            payload = _exact(record["payload"], frozenset({"new_owner"}), payload_context)
            return cls(
                action,
                GovernanceContractLifecycleOfferOwnership(
                    _account_id(payload["new_owner"], f"{payload_context}.new_owner")
                ),
            )
        payload = _exact(
            record["payload"],
            frozenset(
                {
                    "hold_proposal_content_id",
                    "hold_governance_attempt_id",
                    "incident_digest",
                    "retrospective_finding_root",
                }
            ),
            payload_context,
        )
        return cls(
            action,
            GovernanceContractLifecycleEmergencyHoldRetrospective(
                _bytes32(
                    payload["hold_proposal_content_id"],
                    f"{payload_context}.hold_proposal_content_id",
                    nonzero=True,
                ),
                _bytes32(
                    payload["hold_governance_attempt_id"],
                    f"{payload_context}.hold_governance_attempt_id",
                    nonzero=True,
                ),
                _bytes32(
                    payload["incident_digest"],
                    f"{payload_context}.incident_digest",
                    nonzero=True,
                ),
                _bytes32(
                    payload["retrospective_finding_root"],
                    f"{payload_context}.retrospective_finding_root",
                    nonzero=True,
                ),
            ),
        )


@dataclass(frozen=True)
class GovernanceProposalContractLifecycleGovernance:
    """Canonical `ContractLifecycleGovernanceProposalV1` payload."""

    contract_address: str
    expected_revision: int
    action: GovernanceContractLifecycleAction

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalContractLifecycleGovernance":
        context = "ContractLifecycleGovernance payload"
        record = _exact(
            value, frozenset({"contract_address", "expected_revision", "action"}), context
        )
        return cls(
            _contract_address(record["contract_address"], f"{context}.contract_address"),
            _proposal_exact_json_uint(
                record["expected_revision"], f"{context}.expected_revision", positive=True
            ),
            GovernanceContractLifecycleAction.from_payload(record["action"]),
        )


@dataclass(frozen=True)
class GovernanceProposalContractEmergencyHold:
    """Canonical `ContractEmergencyHoldProposalV1` payload."""

    contract_address: str
    expected_revision: int
    expected_code_hash: str
    incident_digest: tuple[int, ...]
    reason: str
    duration_blocks: int

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalContractEmergencyHold":
        context = "ContractEmergencyHold payload"
        record = _exact(
            value,
            frozenset(
                {
                    "contract_address",
                    "expected_revision",
                    "expected_code_hash",
                    "incident_digest",
                    "reason",
                    "duration_blocks",
                }
            ),
            context,
        )
        if not isinstance(record["reason"], str):
            raise TypeError(f"{context}.reason must be a string")
        reason = record["reason"]
        if not reason.strip():
            raise TypeError(f"{context}.reason must not be blank")
        duration_blocks = _proposal_exact_json_uint(
            record["duration_blocks"], f"{context}.duration_blocks", positive=True
        )
        if duration_blocks > 3_600:
            raise TypeError(f"{context}.duration_blocks must be in 1..3600")
        return cls(
            _contract_address(record["contract_address"], f"{context}.contract_address"),
            _proposal_exact_json_uint(
                record["expected_revision"], f"{context}.expected_revision", positive=True
            ),
            _lower_hex32(record["expected_code_hash"], f"{context}.expected_code_hash"),
            _bytes32(record["incident_digest"], f"{context}.incident_digest", nonzero=True),
            reason,
            duration_blocks,
        )


class GovernanceGlobalDataTriggerPermissionAction(str, Enum):
    """Closed exact-account global data-trigger permission transition."""

    GRANT = "grant"
    REVOKE = "revoke"


@dataclass(frozen=True)
class GovernanceProposalGlobalDataTriggerPermissionGovernance:
    """Canonical exact-account global data-trigger permission proposal."""

    authority: str
    action: GovernanceGlobalDataTriggerPermissionAction

    @classmethod
    def from_payload(
        cls, value: Any
    ) -> "GovernanceProposalGlobalDataTriggerPermissionGovernance":
        context = "GlobalDataTriggerPermissionGovernance payload"
        record = _exact(value, frozenset({"authority", "action"}), context)
        action_record = _exact(
            record["action"], frozenset({"action", "value"}), f"{context}.action"
        )
        if action_record["value"] is not None:
            raise TypeError(f"{context}.action.value must be null")
        try:
            action = GovernanceGlobalDataTriggerPermissionAction(action_record["action"])
        except (TypeError, ValueError) as exc:
            raise TypeError(f"{context}.action.action must be grant or revoke") from exc
        return cls(_account_id(record["authority"], f"{context}.authority"), action)


GovernanceProposalPayload = Union[
    GovernanceProposalDeployContract,
    GovernanceProposalRuntimeUpgrade,
    GovernanceProposalSccpRouteGovernance,
    GovernanceProposalValidationFeePolicy,
    GovernanceProposalValidationFeePayoutLifecycle,
    GovernanceProposalMusubiRegistryGovernance,
    GovernanceProposalSorafsProviderGovernance,
    GovernanceProposalContractLifecycleGovernance,
    GovernanceProposalContractEmergencyHold,
    GovernanceProposalGlobalDataTriggerPermissionGovernance,
]


class GovernanceProposalKindTag(str, Enum):
    """Exactly the ten first-release `ProposalKind` tags."""

    DEPLOY_CONTRACT = "DeployContract"
    RUNTIME_UPGRADE = "RuntimeUpgrade"
    SCCP_ROUTE_GOVERNANCE = "SccpRouteGovernance"
    VALIDATION_FEE_POLICY = "ValidationFeePolicy"
    VALIDATION_FEE_PAYOUT_LIFECYCLE = "ValidationFeePayoutLifecycle"
    MUSUBI_REGISTRY_GOVERNANCE = "MusubiRegistryGovernance"
    SORAFS_PROVIDER_GOVERNANCE = "SorafsProviderGovernance"
    CONTRACT_LIFECYCLE_GOVERNANCE = "ContractLifecycleGovernance"
    CONTRACT_EMERGENCY_HOLD = "ContractEmergencyHold"
    GLOBAL_DATA_TRIGGER_PERMISSION_GOVERNANCE = "GlobalDataTriggerPermissionGovernance"


@dataclass(frozen=True)
class GovernanceProposalKind:
    """Closed adjacently-tagged Rust V1 `ProposalKind`."""

    kind: GovernanceProposalKindTag
    payload: GovernanceProposalPayload

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalKind":
        record = _exact(value, frozenset({"kind", "payload"}), "proposal kind")
        try:
            kind = GovernanceProposalKindTag(record["kind"])
        except (ValueError, TypeError) as exc:
            raise TypeError("proposal kind tag is not one of the ten first-release variants") from exc
        parser = {
            kind.DEPLOY_CONTRACT: GovernanceProposalDeployContract.from_payload,
            kind.RUNTIME_UPGRADE: GovernanceProposalRuntimeUpgrade.from_payload,
            kind.SCCP_ROUTE_GOVERNANCE: GovernanceProposalSccpRouteGovernance.from_payload,
            kind.VALIDATION_FEE_POLICY: GovernanceProposalValidationFeePolicy.from_payload,
            kind.VALIDATION_FEE_PAYOUT_LIFECYCLE: GovernanceProposalValidationFeePayoutLifecycle.from_payload,
            kind.MUSUBI_REGISTRY_GOVERNANCE: GovernanceProposalMusubiRegistryGovernance.from_payload,
            kind.SORAFS_PROVIDER_GOVERNANCE: GovernanceProposalSorafsProviderGovernance.from_payload,
            kind.CONTRACT_LIFECYCLE_GOVERNANCE: GovernanceProposalContractLifecycleGovernance.from_payload,
            kind.CONTRACT_EMERGENCY_HOLD: GovernanceProposalContractEmergencyHold.from_payload,
            kind.GLOBAL_DATA_TRIGGER_PERMISSION_GOVERNANCE: GovernanceProposalGlobalDataTriggerPermissionGovernance.from_payload,
        }[kind]
        payload = cast(GovernanceProposalPayload, parser(record["payload"]))
        return cls(kind, payload)


class GovernanceProposalLifecycleStatus(str, Enum):
    """Closed retained proposal lifecycle status."""

    PROPOSED = "Proposed"
    REJECTED = "Rejected"
    ENACTED = "Enacted"
    SUPERSEDED = "Superseded"
    EXECUTION_FAILED = "ExecutionFailed"


@dataclass(frozen=True)
class GovernanceProposalRecord:
    """Exact first-release retained governance proposal record."""

    proposer: str
    kind: GovernanceProposalKind
    created_height: int
    status: GovernanceProposalLifecycleStatus

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalRecord":
        context = "proposal record"
        record = _exact(value, frozenset({"proposer", "kind", "created_height", "status"}), context)
        try:
            status = GovernanceProposalLifecycleStatus(record["status"])
        except (ValueError, TypeError) as exc:
            raise TypeError(f"{context}.status is unsupported") from exc
        return cls(_account_id(record["proposer"], f"{context}.proposer"), GovernanceProposalKind.from_payload(record["kind"]), _uint(record["created_height"], f"{context}.created_height"), status)


@dataclass(frozen=True)
class GovernanceProposalResult:
    """Strict response from `GET /v1/gov/proposals/{id}`."""

    found: bool
    proposal: Optional[GovernanceProposalRecord]

    @classmethod
    def from_payload(cls, value: Any) -> "GovernanceProposalResult":
        if not isinstance(value, Mapping):
            raise TypeError("proposal response must be an object")
        found = value.get("found")
        if not isinstance(found, bool):
            raise TypeError("proposal response.found must be boolean")
        fields = frozenset({"found", "proposal"}) if found else frozenset({"found"})
        record = _exact(value, fields, "proposal response")
        if not found:
            return cls(False, None)
        return cls(True, GovernanceProposalRecord.from_payload(record["proposal"]))


__all__ = [name for name in globals() if name.startswith("Governance")]
