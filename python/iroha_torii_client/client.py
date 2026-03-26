"""Torii client helpers for configuration, subscriptions, attachments, and prover reports.

The API mirrors the app-facing endpoints exposed by Torii:

* `/v1/subscriptions` and `/v1/subscriptions/plans` for subscription
  management and billing triggers.
* `/v1/configuration` for configuration snapshots and updates.
* `/v1/zk/attachments` for uploading, listing, fetching, and deleting
  proof attachments stored on the node.
* `/v1/zk/prover/reports` for querying background prover results.
* `/v1/telemetry/peers-info` for peer telemetry snapshots (connectivity,
  config, and connected peers).

Example
-------
>>> client = ToriiClient("http://localhost:8080")
>>> meta = client.upload_attachment(b"{}", content_type="application/json")
>>> meta["id"]
'ab01cdf...'
>>> client.delete_attachment(meta["id"])

The helper keeps dependencies minimal (``requests`` only) so it can be reused
from tests or scripts without pulling the full CLI.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import math
import time
from dataclasses import dataclass
from typing import (
    Any,
    Dict,
    Iterable,
    List,
    Mapping,
    MutableMapping,
    Optional,
    Sequence,
    Tuple,
    Union,
)
from urllib.parse import quote

import requests

I105_ASCII_ALPHABET = (
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "J",
    "K",
    "L",
    "M",
    "N",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
)
I105_KANA_ALPHABET = (
    "ｲ",
    "ﾛ",
    "ﾊ",
    "ﾆ",
    "ﾎ",
    "ﾍ",
    "ﾄ",
    "ﾁ",
    "ﾘ",
    "ﾇ",
    "ﾙ",
    "ｦ",
    "ﾜ",
    "ｶ",
    "ﾖ",
    "ﾀ",
    "ﾚ",
    "ｿ",
    "ﾂ",
    "ﾈ",
    "ﾅ",
    "ﾗ",
    "ﾑ",
    "ｳ",
    "ヰ",
    "ﾉ",
    "ｵ",
    "ｸ",
    "ﾔ",
    "ﾏ",
    "ｹ",
    "ﾌ",
    "ｺ",
    "ｴ",
    "ﾃ",
    "ｱ",
    "ｻ",
    "ｷ",
    "ﾕ",
    "ﾒ",
    "ﾐ",
    "ｼ",
    "ヱ",
    "ﾋ",
    "ﾓ",
    "ｾ",
    "ｽ",
)
I105_ALPHABET = I105_ASCII_ALPHABET + I105_KANA_ALPHABET
I105_INDEX = {symbol: idx for idx, symbol in enumerate(I105_ALPHABET)}
I105_BASE = len(I105_ALPHABET)
I105_CHECKSUM_LEN = 6
I105_BECH32M_CONST = 0x2BC830A3
I105_SENTINELS = ("sora", "test", "dev")
I105_NUMERIC_SENTINEL_PREFIX = "n"


def _decode_base_n(digits: Sequence[int], base: int) -> bytes:
    value = 0
    for digit in digits:
        value = value * base + digit
    if value == 0:
        decoded = b""
    else:
        pieces = bytearray()
        while value:
            pieces.append(value & 0xFF)
            value >>= 8
        decoded = bytes(reversed(pieces))
    pad = 0
    for digit in digits:
        if digit == 0:
            pad += 1
        else:
            break
    return b"\x00" * pad + decoded


def _convert_to_base32(data: bytes) -> List[int]:
    acc = 0
    bits = 0
    out: List[int] = []
    for byte in data:
        acc = (acc << 8) | byte
        bits += 8
        while bits >= 5:
            bits -= 5
            out.append((acc >> bits) & 0x1F)
    if bits:
        out.append((acc << (5 - bits)) & 0x1F)
    return out


def _bech32_polymod(values: Iterable[int]) -> int:
    generators = (0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)
    chk = 1
    for value in values:
        top = chk >> 25
        chk = ((chk & 0x1FF_FFFF) << 5) ^ value
        for idx, generator in enumerate(generators):
            if (top >> idx) & 1:
                chk ^= generator
    return chk


def _expand_hrp(hrp: str) -> List[int]:
    out: List[int] = []
    for ch in hrp:
        value = ord(ch)
        out.append(value >> 5)
    out.append(0)
    out.extend(ord(ch) & 0x1F for ch in hrp)
    return out


def _bech32m_checksum(data: Sequence[int]) -> List[int]:
    values = _expand_hrp("snx")
    values.extend(data)
    values.extend([0] * I105_CHECKSUM_LEN)
    polymod = _bech32_polymod(values) ^ I105_BECH32M_CONST
    return [(polymod >> (5 * (I105_CHECKSUM_LEN - 1 - i))) & 0x1F for i in range(I105_CHECKSUM_LEN)]


def _i105_checksum_digits(canonical: bytes) -> List[int]:
    return _bech32m_checksum(_convert_to_base32(canonical))


def _strip_i105_sentinel(encoded: str) -> str:
    for sentinel in I105_SENTINELS:
        if encoded.startswith(sentinel):
            return encoded[len(sentinel) :]
    if encoded.startswith(I105_NUMERIC_SENTINEL_PREFIX):
        index = len(I105_NUMERIC_SENTINEL_PREFIX)
        while index < len(encoded) and encoded[index].isdigit():
            index += 1
        if index > len(I105_NUMERIC_SENTINEL_PREFIX):
            return encoded[index:]
    raise ValueError("I105 address is missing the expected chain-discriminant sentinel")


def _decode_i105_string(encoded: str) -> bytes:
    payload = _strip_i105_sentinel(encoded)
    digits: List[int] = []
    for symbol in payload:
        try:
            digits.append(I105_INDEX[symbol])
        except KeyError as exc:
            raise ValueError("invalid character in I105 address") from exc
    if len(digits) <= I105_CHECKSUM_LEN:
        raise ValueError("I105 address too short")
    data_digits = digits[:-I105_CHECKSUM_LEN]
    checksum_digits = digits[-I105_CHECKSUM_LEN:]
    canonical = _decode_base_n(data_digits, I105_BASE)
    if checksum_digits != _i105_checksum_digits(canonical):
        raise ValueError("I105 checksum mismatch")
    return canonical

__all__ = [
    "ToriiClient",
    "decode_pdp_commitment_header",
    "CouncilMember",
    "CouncilCurrentStatus",
    "CouncilAuditMetadata",
    "GovernanceProposalStatus",
    "GovernanceLocksOverview",
    "GovernanceReferendumStatus",
    "GovernanceTallySummary",
    "GovernanceUnlockStats",
    "TransactionInstruction",
    "GovernanceInstructionDraft",
    "GovernanceProposalDraft",
    "BallotSubmitResult",
    "ProtectedNamespacesApplyResult",
    "ProtectedNamespacesStatus",
    "VrfCandidate",
    "CouncilPersistResult",
    "PeerInfo",
    "PeerTelemetryConfig",
    "PeerTelemetryLocation",
    "PeerTelemetryInfo",
    "TriggerRecord",
    "TriggerListPage",
    "LoggerConfig",
    "NetworkConfig",
    "QueueConfig",
    "ConfidentialGasSchedule",
    "ConfigurationSnapshot",
    "ExplorerAccountQr",
    "NetworkTimeSnapshot",
    "NetworkTimeSample",
    "NetworkTimeRttBucket",
    "NetworkTimeStatus",
    "NodeSmAcceleration",
    "NodeSmCapabilities",
    "NodeCurveCapabilities",
    "NodeCryptoCapabilities",
    "NodeCapabilities",
    "RuntimeAbiActive",
    "RuntimeAbiHash",
    "RuntimeUpgradeEventCounters",
    "RuntimeMetricsSnapshot",
    "RuntimeUpgradeManifest",
    "RuntimeUpgradeStatus",
    "RuntimeUpgradeRecord",
    "RuntimeUpgradeListItem",
    "RuntimeUpgradeTxResponse",
    "ConnectPerIpSessions",
    "ConnectStatusPolicy",
    "ConnectStatusSnapshot",
    "ConnectSessionInfo",
    "ConnectAppRecord",
    "ConnectAppRegistryPage",
    "ConnectAppPolicyControls",
    "ConnectAdmissionManifestEntry",
    "ConnectAdmissionManifest",
    "LaneCommitmentSnapshot",
    "DataspaceCommitmentSnapshot",
    "UaidPortfolioTotals",
    "UaidPortfolioAsset",
    "UaidPortfolioAccount",
    "UaidPortfolioDataspace",
    "UaidPortfolioResponse",
    "UaidBindingsDataspace",
    "UaidBindingsResponse",
    "UaidManifestRevocation",
    "UaidManifestLifecycle",
    "UaidManifestEntry",
    "UaidManifest",
    "UaidManifestRecord",
    "UaidManifestsResponse",
    "LaneRuntimeUpgradeHook",
    "LaneGovernanceSnapshot",
    "GovernanceProposalCounters",
    "GovernanceProtectedNamespaceStats",
    "GovernanceManifestAdmissionStats",
    "GovernanceManifestQuorumStats",
    "GovernanceManifestActivation",
    "GovernanceStatusSnapshot",
    "StatusMetrics",
    "StatusPayload",
    "SumeragiConsensusCaps",
    "StatusSnapshot",
    "SumeragiRbcSnapshot",
    "SumeragiRbcSessionsSnapshot",
    "SumeragiRbcSession",
    "SumeragiRbcDeliveryStatus",
    "SumeragiEvidenceRecord",
    "SumeragiEvidenceListPage",
    "KaigiRelaySummary",
    "KaigiRelaySummaryList",
    "KaigiRelayReportedCall",
    "KaigiRelayDomainMetrics",
    "KaigiRelayDetail",
    "KaigiRelayHealthSnapshot",
    "SumeragiQcEntry",
    "OfflineAllowanceDeadline",
    "OfflineAllowanceListItem",
    "OfflineAllowanceListPage",
    "OfflineTransferHistoryEntry",
    "OfflineTransferListItem",
    "OfflineTransferListPage",
    "OfflineSummaryListItem",
    "OfflineSummaryListPage",
    "SubscriptionPlanCreateResult",
    "SubscriptionPlanListItem",
    "SubscriptionPlanListPage",
    "SubscriptionCreateResult",
    "SubscriptionListItem",
    "SubscriptionListPage",
    "SubscriptionActionResult",
    "SumeragiQcSnapshot",
    "SumeragiPacemakerSnapshot",
    "SumeragiPhasesSnapshot",
    "SumeragiPhasesEma",
    "SumeragiPrfContext",
    "SumeragiLeaderSnapshot",
    "SumeragiCollectorEntry",
    "SumeragiCollectorsSnapshot",
    "SumeragiParamsSnapshot",
    "RbcSample",
    "RbcChunkSample",
    "RbcMerkleProof",
]

PDP_COMMITMENT_HEADER = "sora-pdp-commitment"


def decode_pdp_commitment_header(headers: Optional[Mapping[str, str]]) -> Optional[bytes]:
    """Decode the Norito-encoded PDP commitment advertised via HTTP headers.

    Parameters
    ----------
    headers:
        Mapping containing HTTP headers (case-insensitive). Accepts both the
        `requests`-style headers dictionary and raw dictionaries with string keys.
    """

    if headers is None:
        return None
    raw_value = _read_header_value(headers, PDP_COMMITMENT_HEADER)
    if raw_value is None:
        return None
    try:
        return base64.b64decode(raw_value, validate=True)
    except binascii.Error as exc:
        raise RuntimeError(f"Failed to decode {PDP_COMMITMENT_HEADER} header: {exc}") from exc


def _read_header_value(
    headers: Mapping[str, Any],
    name: str,
) -> Optional[str]:
    lowered = name.lower()
    getter = getattr(headers, "get", None)
    if callable(getter):
        direct = getter(name)
        if isinstance(direct, str):
            return direct
        fallback = getter(lowered)
        if isinstance(fallback, str):
            return fallback
    try:
        items = headers.items()
    except AttributeError:
        return None
    for key, value in items:
        if isinstance(key, str) and key.lower() == lowered and isinstance(value, str):
            return value
    return None

@dataclass(frozen=True)
class PeerInfo:
    """Online peer descriptor returned by ``GET /v1/peers``."""

    address: str
    public_key_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerInfo":
        if not isinstance(payload, Mapping):
            raise RuntimeError("peer payload must be an object")
        address = payload.get("address")
        if not isinstance(address, str) or not address:
            raise RuntimeError("peer payload missing `address`")
        identity = payload.get("id")
        if not isinstance(identity, Mapping):
            raise RuntimeError("peer payload missing `id` object")
        public_key = identity.get("public_key")
        if not isinstance(public_key, str) or not public_key:
            raise RuntimeError("peer payload missing `id.public_key`")
        return cls(address=address, public_key_hex=public_key)


@dataclass(frozen=True)
class PeerTelemetryConfig:
    """Configuration snapshot attached to a telemetry peer entry."""

    public_key_hex: str
    queue_capacity: Optional[int]
    network_block_gossip_size: Optional[int]
    network_block_gossip_period_ms: Optional[int]
    network_tx_gossip_size: Optional[int]
    network_tx_gossip_period_ms: Optional[int]


@dataclass(frozen=True)
class PeerTelemetryLocation:
    """Geolocation metadata for a telemetry peer."""

    lat: float
    lon: float
    country: str
    city: str


@dataclass(frozen=True)
class PeerTelemetryInfo:
    """Entry returned by ``GET /v1/telemetry/peers-info``."""

    url: str
    connected: bool
    telemetry_unsupported: bool
    config: Optional[PeerTelemetryConfig]
    location: Optional[PeerTelemetryLocation]
    connected_peers: Optional[List[str]]


@dataclass(frozen=True)
class LoggerConfig:
    """Logger configuration fragment exposed via ``/v1/configuration``."""

    level: str
    filter: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "LoggerConfig":
        if not isinstance(payload, Mapping):
            raise RuntimeError("logger section must be an object")
        level = payload.get("level")
        if not isinstance(level, str) or not level:
            raise RuntimeError("logger section missing `level`")
        filter_value = payload.get("filter")
        if filter_value is None:
            filter_str = None
        elif isinstance(filter_value, str):
            filter_str = filter_value
        else:
            raise RuntimeError("logger section `filter` must be a string when present")
        return cls(level=level, filter=filter_str)

    def to_payload(self) -> Dict[str, Any]:
        payload: Dict[str, Any] = {"level": self.level}
        payload["filter"] = self.filter
        return payload


@dataclass(frozen=True)
class NetworkConfig:
    """Network gossip configuration exposed via ``/v1/configuration``."""

    block_gossip_size: int
    block_gossip_period_ms: int
    transaction_gossip_size: int
    transaction_gossip_period_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NetworkConfig":
        if not isinstance(payload, Mapping):
            raise RuntimeError("network section must be an object")
        try:
            block_gossip_size = int(payload["block_gossip_size"])
            block_gossip_period_ms = int(payload["block_gossip_period_ms"])
            transaction_gossip_size = int(payload["transaction_gossip_size"])
            transaction_gossip_period_ms = int(payload["transaction_gossip_period_ms"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError("network section is missing numeric gossip fields") from exc
        return cls(
            block_gossip_size=block_gossip_size,
            block_gossip_period_ms=block_gossip_period_ms,
            transaction_gossip_size=transaction_gossip_size,
            transaction_gossip_period_ms=transaction_gossip_period_ms,
        )


@dataclass(frozen=True)
class QueueConfig:
    """Transaction queue configuration exposed via ``/v1/configuration``."""

    capacity: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "QueueConfig":
        if not isinstance(payload, Mapping):
            raise RuntimeError("queue section must be an object")
        try:
            capacity = int(payload["capacity"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError("queue section missing numeric `capacity`") from exc
        return cls(capacity=capacity)


@dataclass(frozen=True)
class ConfidentialGasSchedule:
    """Confidential verification gas schedule."""

    proof_base: int
    per_public_input: int
    per_proof_byte: int
    per_nullifier: int
    per_commitment: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConfidentialGasSchedule":
        if not isinstance(payload, Mapping):
            raise RuntimeError("confidential gas section must be an object")
        try:
            proof_base = int(payload["proof_base"])
            per_public_input = int(payload["per_public_input"])
            per_proof_byte = int(payload["per_proof_byte"])
            per_nullifier = int(payload["per_nullifier"])
            per_commitment = int(payload["per_commitment"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError("confidential gas section missing numeric fields") from exc
        return cls(
            proof_base=proof_base,
            per_public_input=per_public_input,
            per_proof_byte=per_proof_byte,
            per_nullifier=per_nullifier,
            per_commitment=per_commitment,
        )

    def to_payload(self) -> Dict[str, Any]:
        return {
            "proof_base": self.proof_base,
            "per_public_input": self.per_public_input,
            "per_proof_byte": self.per_proof_byte,
            "per_nullifier": self.per_nullifier,
            "per_commitment": self.per_commitment,
        }


@dataclass(frozen=True)
class TransportNoritoRpcConfig:
    """Norito-RPC transport summary exposed via ``/v1/configuration``."""

    enabled: bool
    stage: str
    require_mtls: bool
    canary_allowlist_size: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TransportNoritoRpcConfig":
        if not isinstance(payload, Mapping):
            raise RuntimeError("transport.norito_rpc section must be an object")
        enabled = payload.get("enabled")
        if not isinstance(enabled, bool):
            raise RuntimeError("transport.norito_rpc section missing `enabled`")
        stage = payload.get("stage")
        if not isinstance(stage, str) or not stage:
            raise RuntimeError("transport.norito_rpc section missing `stage`")
        require_mtls = payload.get("require_mtls")
        if not isinstance(require_mtls, bool):
            raise RuntimeError("transport.norito_rpc section missing `require_mtls`")
        try:
            canary_allowlist_size = int(payload["canary_allowlist_size"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError(
                "transport.norito_rpc section missing numeric `canary_allowlist_size`"
            ) from exc
        return cls(
            enabled=enabled,
            stage=stage,
            require_mtls=require_mtls,
            canary_allowlist_size=canary_allowlist_size,
        )


@dataclass(frozen=True)
class StreamingSoranetConfig:
    """SoraNet streaming defaults exposed via ``/v1/configuration``."""

    enabled: bool
    stream_tag: str
    exit_multiaddr: str
    padding_budget_ms: Optional[int]
    access_kind: str
    gar_category: str
    channel_salt: str
    provision_spool_dir: str
    provision_window_segments: int
    provision_queue_capacity: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "StreamingSoranetConfig":
        if not isinstance(payload, Mapping):
            raise RuntimeError("transport.streaming.soranet section must be an object")
        enabled = payload.get("enabled")
        if not isinstance(enabled, bool):
            raise RuntimeError("transport.streaming.soranet section missing `enabled`")
        stream_tag = payload.get("stream_tag")
        if not isinstance(stream_tag, str) or not stream_tag:
            raise RuntimeError("transport.streaming.soranet section missing `stream_tag`")
        exit_multiaddr = payload.get("exit_multiaddr")
        if not isinstance(exit_multiaddr, str) or not exit_multiaddr:
            raise RuntimeError("transport.streaming.soranet section missing `exit_multiaddr`")
        padding_value = payload.get("padding_budget_ms")
        if padding_value is None:
            padding_budget_ms = None
        else:
            try:
                padding_budget_ms = int(padding_value)
            except (TypeError, ValueError) as exc:
                raise RuntimeError(
                    "transport.streaming.soranet section `padding_budget_ms` must be numeric"
                ) from exc
        access_kind = payload.get("access_kind")
        if not isinstance(access_kind, str) or not access_kind:
            raise RuntimeError("transport.streaming.soranet section missing `access_kind`")
        gar_category = payload.get("gar_category")
        if not isinstance(gar_category, str) or not gar_category:
            raise RuntimeError("transport.streaming.soranet section missing `gar_category`")
        channel_salt = payload.get("channel_salt")
        if not isinstance(channel_salt, str):
            raise RuntimeError("transport.streaming.soranet section missing `channel_salt`")
        provision_spool_dir = payload.get("provision_spool_dir")
        if not isinstance(provision_spool_dir, str) or not provision_spool_dir:
            raise RuntimeError("transport.streaming.soranet section missing `provision_spool_dir`")
        try:
            provision_window_segments = int(payload["provision_window_segments"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError(
                "transport.streaming.soranet section missing numeric `provision_window_segments`"
            ) from exc
        try:
            provision_queue_capacity = int(payload["provision_queue_capacity"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError(
                "transport.streaming.soranet section missing numeric `provision_queue_capacity`"
            ) from exc
        return cls(
            enabled=enabled,
            stream_tag=stream_tag,
            exit_multiaddr=exit_multiaddr,
            padding_budget_ms=padding_budget_ms,
            access_kind=access_kind,
            gar_category=gar_category,
            channel_salt=channel_salt,
            provision_spool_dir=provision_spool_dir,
            provision_window_segments=provision_window_segments,
            provision_queue_capacity=provision_queue_capacity,
        )


@dataclass(frozen=True)
class StreamingTransportConfig:
    """Streaming transport configuration exposed via ``/v1/configuration``."""

    soranet: Optional[StreamingSoranetConfig]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "StreamingTransportConfig":
        if not isinstance(payload, Mapping):
            raise RuntimeError("transport.streaming section must be an object")
        soranet_section = payload.get("soranet")
        soranet = (
            StreamingSoranetConfig.from_payload(soranet_section)
            if isinstance(soranet_section, Mapping)
            else None
        )
        return cls(soranet=soranet)


@dataclass(frozen=True)
class TransportConfig:
    """Transport configuration exposed via ``/v1/configuration``."""

    norito_rpc: Optional[TransportNoritoRpcConfig]
    streaming: Optional[StreamingTransportConfig]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TransportConfig":
        if not isinstance(payload, Mapping):
            raise RuntimeError("transport section must be an object")
        norito_section = payload.get("norito_rpc")
        norito_rpc = (
            TransportNoritoRpcConfig.from_payload(norito_section)
            if isinstance(norito_section, Mapping)
            else None
        )
        streaming_section = payload.get("streaming")
        streaming = (
            StreamingTransportConfig.from_payload(streaming_section)
            if isinstance(streaming_section, Mapping)
            else None
        )
        return cls(norito_rpc=norito_rpc, streaming=streaming)


@dataclass(frozen=True)
class ConfigurationSnapshot:
    """Typed configuration payload returned by ``GET /v1/configuration``."""

    public_key_hex: str
    logger: LoggerConfig
    network: NetworkConfig
    queue: Optional[QueueConfig]
    confidential_gas: Optional[ConfidentialGasSchedule]
    transport: Optional[TransportConfig]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConfigurationSnapshot":
        if not isinstance(payload, Mapping):
            raise RuntimeError("configuration response must be an object")
        public_key = payload.get("public_key")
        if not isinstance(public_key, str) or not public_key:
            raise RuntimeError("configuration response missing `public_key`")
        logger = LoggerConfig.from_payload(payload.get("logger", {}))
        network = NetworkConfig.from_payload(payload.get("network", {}))
        queue_section = payload.get("queue")
        queue = QueueConfig.from_payload(queue_section) if isinstance(queue_section, Mapping) else None
        gas_section = payload.get("confidential_gas")
        confidential_gas = (
            ConfidentialGasSchedule.from_payload(gas_section)
            if isinstance(gas_section, Mapping)
            else None
        )
        transport_section = payload.get("transport")
        transport = (
            TransportConfig.from_payload(transport_section)
            if isinstance(transport_section, Mapping)
            else None
        )
        return cls(
            public_key_hex=public_key,
            logger=logger,
            network=network,
            queue=queue,
            confidential_gas=confidential_gas,
            transport=transport,
        )


@dataclass(frozen=True)
class ExplorerAccountQr:
    """QR metadata returned by ``GET /v1/explorer/accounts/{account_id}/qr``."""

    canonical_id: str
    literal: str
    network_prefix: int
    error_correction: str
    modules: int
    qr_version: int
    svg: str


@dataclass(frozen=True)
class NetworkTimeSnapshot:
    """Snapshot returned by ``GET /v1/time/now``."""

    now_ms: int
    offset_ms: int
    confidence_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NetworkTimeSnapshot":
        if not isinstance(payload, Mapping):
            raise RuntimeError("network time payload must be an object")
        try:
            now_ms = int(payload["now"])
            offset_ms = int(payload["offset_ms"])
            confidence_ms = int(payload["confidence_ms"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError("network time payload missing numeric fields") from exc
        return cls(now_ms=now_ms, offset_ms=offset_ms, confidence_ms=confidence_ms)


@dataclass(frozen=True)
class NetworkTimeRttBucket:
    """Histogram bucket describing RTT distribution."""

    upper_bound_ms: Optional[int]
    count: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NetworkTimeRttBucket":
        if not isinstance(payload, Mapping):
            raise RuntimeError("network time RTT bucket must be an object")
        le_value = payload.get("le")
        upper_bound = None if le_value is None else int(le_value)
        try:
            count = int(payload["count"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError("network time RTT bucket missing numeric `count`") from exc
        return cls(upper_bound_ms=upper_bound, count=count)


@dataclass(frozen=True)
class NetworkTimeSample:
    """Peer sampling metadata returned by ``GET /v1/time/status``."""

    peer: str
    last_offset_ms: int
    last_rtt_ms: int
    count: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NetworkTimeSample":
        if not isinstance(payload, Mapping):
            raise RuntimeError("network time sample must be an object")
        peer = payload.get("peer")
        if not isinstance(peer, str) or not peer:
            raise RuntimeError("network time sample missing `peer`")
        try:
            offset_ms = int(payload["last_offset_ms"])
            rtt_ms = int(payload["last_rtt_ms"])
            count = int(payload["count"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RuntimeError("network time sample missing numeric fields") from exc
        return cls(peer=peer, last_offset_ms=offset_ms, last_rtt_ms=rtt_ms, count=count)


@dataclass(frozen=True)
class NetworkTimeStatus:
    """Diagnostics payload returned by ``GET /v1/time/status``."""

    peers: int
    samples: List[NetworkTimeSample]
    rtt_buckets: List[NetworkTimeRttBucket]
    rtt_sum_ms: int
    rtt_count: int
    note: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NetworkTimeStatus":
        if not isinstance(payload, Mapping):
            raise RuntimeError("network time status must be an object")
        try:
            peers = int(payload.get("peers", 0))
        except (TypeError, ValueError) as exc:
            raise RuntimeError("network time status `peers` must be numeric") from exc
        raw_samples = payload.get("samples", [])
        if not isinstance(raw_samples, list):
            raise RuntimeError("network time status `samples` must be a list")
        samples = [NetworkTimeSample.from_payload(entry) for entry in raw_samples]
        rtt_section = payload.get("rtt", {})
        if rtt_section and not isinstance(rtt_section, Mapping):
            raise RuntimeError("network time status `rtt` must be an object")
        buckets: List[NetworkTimeRttBucket] = []
        rtt_sum_ms = 0
        rtt_count = 0
        if isinstance(rtt_section, Mapping):
            bucket_payload = rtt_section.get("buckets", [])
            if bucket_payload is not None:
                if not isinstance(bucket_payload, list):
                    raise RuntimeError("network time status `rtt.buckets` must be a list")
                buckets = [NetworkTimeRttBucket.from_payload(entry) for entry in bucket_payload]
            try:
                rtt_sum_ms = int(rtt_section.get("sum_ms", 0))
                rtt_count = int(rtt_section.get("count", 0))
            except (TypeError, ValueError) as exc:
                raise RuntimeError("network time status `rtt` summary must be numeric") from exc
        note_value = payload.get("note")
        note = str(note_value) if note_value is not None else None
        return cls(
            peers=peers,
            samples=samples,
            rtt_buckets=buckets,
            rtt_sum_ms=rtt_sum_ms,
            rtt_count=rtt_count,
            note=note,
        )


@dataclass(frozen=True)
class NodeSmAcceleration:
    """Acceleration profile advertised by SM capability payloads."""

    scalar: bool
    neon_sm3: bool
    neon_sm4: bool
    policy: str


@dataclass(frozen=True)
class NodeSmCapabilities:
    """SM2/SM3 capability advert returned by `/v1/node/capabilities`."""

    enabled: bool
    default_hash: Optional[str]
    allowed_signing: List[str]
    sm2_distid_default: Optional[str]
    openssl_preview: bool
    acceleration: NodeSmAcceleration


@dataclass(frozen=True)
class NodeCurveCapabilities:
    """Curve registry advert returned by `/v1/node/capabilities`."""

    registry_version: int
    allowed_curve_ids: List[int]
    allowed_curve_bitmap: List[int]


@dataclass(frozen=True)
class NodeCryptoCapabilities:
    """Aggregated crypto capability advert."""

    sm: NodeSmCapabilities
    curves: NodeCurveCapabilities


@dataclass(frozen=True)
class NodeCapabilities:
    """Capability advert returned by ``GET /v1/node/capabilities``."""

    abi_version: int
    data_model_version: int
    crypto: NodeCryptoCapabilities


@dataclass(frozen=True)
class RuntimeAbiActive:
    """Active ABI version advertised by the runtime."""

    abi_version: int


@dataclass(frozen=True)
class RuntimeAbiHash:
    """Canonical ABI hash advertised by the runtime."""

    policy: str
    abi_hash_hex: str


@dataclass(frozen=True)
class RuntimeUpgradeEventCounters:
    """Upgrade event counters grouped by status."""

    proposed: int
    activated: int
    canceled: int


@dataclass(frozen=True)
class RuntimeMetricsSnapshot:
    """Runtime upgrade metrics returned by `/v1/runtime/metrics`."""

    abi_version: int
    upgrade_events_total: RuntimeUpgradeEventCounters


@dataclass(frozen=True)
class RuntimeUpgradeManifest:
    """Manifest describing a runtime upgrade."""

    name: str
    description: str
    abi_version: int
    abi_hash_hex: str
    added_syscalls: List[int]
    added_pointer_types: List[int]
    start_height: int
    end_height: int


@dataclass(frozen=True)
class RuntimeUpgradeStatus:
    """Lifecycle status of a runtime upgrade record."""

    kind: str
    activated_height: Optional[int]


@dataclass(frozen=True)
class RuntimeUpgradeRecord:
    """Runtime upgrade record returned by the API."""

    manifest: RuntimeUpgradeManifest
    status: RuntimeUpgradeStatus
    proposer: str
    created_height: int


@dataclass(frozen=True)
class RuntimeUpgradeListItem:
    """Entry returned by `/v1/runtime/upgrades`."""

    id_hex: str
    record: RuntimeUpgradeRecord


@dataclass(frozen=True)
class RuntimeUpgradeTxResponse:
    """Instruction bundle returned by runtime upgrade helpers."""

    ok: bool
    tx_instructions: List[TransactionInstruction]


@dataclass(frozen=True)
class ConnectPerIpSessions:
    """Per-IP session counter inside a Connect status snapshot."""

    ip: str
    sessions: int


@dataclass(frozen=True)
class ConnectStatusPolicy:
    """Policy knobs currently enforced by the Connect service."""

    relay_enabled: Optional[bool]
    ws_max_sessions: Optional[int]
    ws_per_ip_max_sessions: Optional[int]
    ws_rate_per_ip_per_min: Optional[int]
    session_ttl_ms: Optional[int]
    frame_max_bytes: Optional[int]
    session_buffer_max_bytes: Optional[int]
    heartbeat_interval_ms: Optional[int]
    heartbeat_miss_tolerance: Optional[int]
    heartbeat_min_interval_ms: Optional[int]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectStatusSnapshot:
    """Aggregate Connect status metrics."""

    enabled: bool
    sessions_total: int
    sessions_active: int
    per_ip_sessions: List[ConnectPerIpSessions]
    buffered_sessions: int
    total_buffer_bytes: int
    dedupe_size: int
    frames_in_total: int
    frames_out_total: int
    ciphertext_total: int
    dedupe_drops_total: int
    buffer_drops_total: int
    plaintext_control_drops_total: int
    monotonic_drops_total: int
    ping_miss_total: int
    policy: Optional[ConnectStatusPolicy]


@dataclass(frozen=True)
class ConnectSessionInfo:
    """Session tokens returned by ``POST /v1/connect/session``."""

    sid: str
    wallet_uri: str
    app_uri: str
    token_app: str
    token_wallet: str
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAppRecord:
    """Registered Connect application descriptor."""

    app_id: str
    display_name: Optional[str]
    description: Optional[str]
    icon_url: Optional[str]
    namespaces: List[str]
    metadata: Dict[str, Any]
    policy: Dict[str, Any]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAppRegistryPage:
    """Paginated Connect app registry response."""

    items: List[ConnectAppRecord]
    total: Optional[int]
    next_cursor: Optional[str]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAppPolicyControls:
    """Mutable Connect app policy toggles."""

    relay_enabled: Optional[bool]
    ws_max_sessions: Optional[int]
    ws_per_ip_max_sessions: Optional[int]
    ws_rate_per_ip_per_min: Optional[int]
    session_ttl_ms: Optional[int]
    frame_max_bytes: Optional[int]
    session_buffer_max_bytes: Optional[int]
    heartbeat_interval_ms: Optional[int]
    heartbeat_miss_tolerance: Optional[int]
    heartbeat_min_interval_ms: Optional[int]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAdmissionManifestEntry:
    """Admission manifest entry describing permitted Connect apps."""

    app_id: str
    namespaces: List[str]
    metadata: Dict[str, Any]
    policy: Dict[str, Any]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class ConnectAdmissionManifest:
    """Admission manifest contents returned by Connect governance endpoints."""

    version: Optional[int]
    manifest_hash: Optional[str]
    updated_at: Optional[str]
    entries: List[ConnectAdmissionManifestEntry]
    extra: Dict[str, Any]


@dataclass(frozen=True)
class SumeragiRbcSnapshot:
    """Aggregated RBC metrics exposed via ``GET /v1/sumeragi/rbc``."""

    sessions_active: int
    sessions_pruned_total: int
    ready_broadcasts_total: int
    deliver_broadcasts_total: int
    payload_bytes_delivered_total: int


@dataclass(frozen=True)
class SumeragiRbcSession:
    """Active RBC session metadata returned by ``GET /v1/sumeragi/rbc/sessions``."""

    block_hash: Optional[str]
    height: int
    view: int
    total_chunks: int
    received_chunks: int
    ready_count: int
    delivered: bool
    invalid: bool
    payload_hash: Optional[str]
    recovered: bool


@dataclass(frozen=True)
class SumeragiRbcSessionsSnapshot:
    """Snapshot response for ``GET /v1/sumeragi/rbc/sessions``."""

    sessions_active: int
    items: List[SumeragiRbcSession]


@dataclass(frozen=True)
class SumeragiRbcDeliveryStatus:
    """Delivery lookup payload returned by ``GET /v1/sumeragi/rbc/delivered/{height}/{view}``."""

    height: int
    view: int
    delivered: bool
    present: bool
    block_hash: Optional[str]
    ready_count: int
    received_chunks: int
    total_chunks: int


SUMERAGI_EVIDENCE_KIND_FILTERS = {
    "DoublePrepare",
    "DoubleCommit",
    "InvalidQc",
    "InvalidProposal",
    "Censorship",
}
SUMERAGI_EVIDENCE_PHASES = {"Prepare", "Commit", "NewView"}


@dataclass(frozen=True)
class SumeragiEvidenceRecord:
    """Evidence payload returned by ``GET /v1/sumeragi/evidence``."""

    kind: str
    recorded_height: int
    recorded_view: int
    recorded_ms: int
    phase: Optional[str] = None
    height: Optional[int] = None
    view: Optional[int] = None
    epoch: Optional[int] = None
    signer: Optional[str] = None
    block_hash: Optional[str] = None
    block_hash_1: Optional[str] = None
    block_hash_2: Optional[str] = None
    parent_state_root: Optional[str] = None
    post_state_root_1: Optional[str] = None
    post_state_root_2: Optional[str] = None
    subject_block_hash: Optional[str] = None
    payload_hash: Optional[str] = None
    tx_hash: Optional[str] = None
    receipt_count: Optional[int] = None
    min_height: Optional[int] = None
    max_height: Optional[int] = None
    signers: Optional[List[str]] = None
    reason: Optional[str] = None
    detail: Optional[str] = None


@dataclass(frozen=True)
class SumeragiEvidenceListPage:
    """Paginated evidence listing."""

    items: List[SumeragiEvidenceRecord]
    total: int


_KAIGI_HEALTH_STATUSES = {"healthy", "degraded", "unavailable"}


@dataclass(frozen=True)
class KaigiRelaySummary:
    """Summary entry returned by ``GET /v1/kaigi/relays``."""

    relay_id: str
    domain: str
    bandwidth_class: int
    hpke_fingerprint_hex: str
    status: Optional[str]
    reported_at_ms: Optional[int]


@dataclass(frozen=True)
class KaigiRelaySummaryList:
    """Response envelope for ``GET /v1/kaigi/relays``."""

    total: int
    items: List[KaigiRelaySummary]


@dataclass(frozen=True)
class KaigiRelayReportedCall:
    """Call metadata reported alongside Kaigi relay health snapshots."""

    domain_id: str
    call_name: str


@dataclass(frozen=True)
class KaigiRelayDomainMetrics:
    """Per-domain metrics emitted by Kaigi relay endpoints."""

    domain: str
    registrations_total: int
    manifest_updates_total: int
    failovers_total: int
    health_reports_total: int


@dataclass(frozen=True)
class KaigiRelayDetail:
    """Detailed relay metadata returned by ``GET /v1/kaigi/relays/{relay_id}``."""

    relay: KaigiRelaySummary
    hpke_public_key_b64: str
    reported_call: Optional[KaigiRelayReportedCall]
    reported_by: Optional[str]
    notes: Optional[str]
    metrics: Optional[KaigiRelayDomainMetrics]


@dataclass(frozen=True)
class KaigiRelayHealthSnapshot:
    """Aggregated relay health counters returned by ``GET /v1/kaigi/relays/health``."""

    healthy_total: int
    degraded_total: int
    unavailable_total: int
    reports_total: int
    registrations_total: int
    failovers_total: int
    domains: List[KaigiRelayDomainMetrics]


@dataclass(frozen=True)
class SumeragiQcEntry:
    """`HighestQC`/`LockedQC` entry returned by ``GET /v1/sumeragi/qc``."""

    height: int
    view: int
    subject_block_hash: Optional[str]


@dataclass(frozen=True)
class SumeragiQcSnapshot:
    """QC snapshot returned by ``GET /v1/sumeragi/qc``."""

    highest_qc: SumeragiQcEntry
    locked_qc: SumeragiQcEntry


@dataclass(frozen=True)
class SumeragiPacemakerSnapshot:
    """Pacemaker metrics returned by ``GET /v1/sumeragi/pacemaker``."""

    backoff_ms: int
    rtt_floor_ms: int
    jitter_ms: int
    backoff_multiplier: int
    rtt_floor_multiplier: int
    max_backoff_ms: int
    jitter_frac_permille: int
    round_elapsed_ms: int
    view_timeout_target_ms: int
    view_timeout_remaining_ms: int


@dataclass(frozen=True)
class SumeragiPhasesEma:
    """Smoothed latency metrics returned alongside ``/v1/sumeragi/phases``."""

    propose_ms: int
    collect_da_ms: int
    collect_prevote_ms: int
    collect_precommit_ms: int
    collect_aggregator_ms: int
    commit_ms: int
    pipeline_total_ms: int


@dataclass(frozen=True)
class SumeragiPhasesSnapshot:
    """Latest latency counters returned by ``GET /v1/sumeragi/phases``."""

    propose_ms: int
    collect_da_ms: int
    collect_prevote_ms: int
    collect_precommit_ms: int
    collect_aggregator_ms: int
    commit_ms: int
    pipeline_total_ms: int
    collect_aggregator_gossip_total: int
    block_created_dropped_by_lock_total: int
    block_created_hint_mismatch_total: int
    block_created_proposal_mismatch_total: int
    ema_ms: SumeragiPhasesEma


@dataclass(frozen=True)
class SumeragiPrfContext:
    """PRF state returned by Sumeragi inspection endpoints."""

    height: int
    view: int
    epoch_seed: Optional[str]


@dataclass(frozen=True)
class SumeragiLeaderSnapshot:
    """Leader metadata returned by ``GET /v1/sumeragi/leader``."""

    leader_index: int
    prf: SumeragiPrfContext


@dataclass(frozen=True)
class SumeragiCollectorEntry:
    """Collector slot assignment returned by ``GET /v1/sumeragi/collectors``."""

    index: int
    peer_id: str


@dataclass(frozen=True)
class SumeragiCollectorsSnapshot:
    """Collector selection snapshot returned by ``GET /v1/sumeragi/collectors``."""

    consensus_mode: str
    mode: str
    topology_len: int
    min_votes_for_commit: int
    proxy_tail_index: int
    height: int
    view: int
    collectors_k: int
    redundant_send_r: int
    epoch_seed: Optional[str]
    collectors: List[SumeragiCollectorEntry]
    prf: SumeragiPrfContext


@dataclass(frozen=True)
class SumeragiParamsSnapshot:
    """Consensus parameter snapshot returned by ``GET /v1/sumeragi/params``."""

    block_time_ms: int
    commit_time_ms: int
    max_clock_drift_ms: int
    collectors_k: int
    redundant_send_r: int
    da_enabled: bool
    next_mode: Optional[str]
    mode_activation_height: Optional[int]
    chain_height: int


@dataclass(frozen=True)
class RbcMerkleProof:
    """Merkle proof for a sampled RBC chunk."""

    leaf_index: int
    depth: Optional[int]
    audit_path: List[Optional[str]]


@dataclass(frozen=True)
class RbcChunkSample:
    """Chunk digest/proof pair returned by ``POST /v1/sumeragi/rbc/sample``."""

    index: int
    chunk_hex: str
    digest_hex: str
    proof: RbcMerkleProof


@dataclass(frozen=True)
class RbcSample:
    """RBC chunk sample set for a particular block/view."""

    block_hash: str
    height: int
    view: int
    total_chunks: int
    chunk_root: str
    payload_hash: Optional[str]
    samples: List[RbcChunkSample]


@dataclass(frozen=True)
class OfflineAllowanceDeadline:
    """Deadline summary advertised by `/v1/offline/allowances`."""

    kind: str
    state: str
    deadline_ms: int
    ms_remaining: int


@dataclass(frozen=True)
class OfflineAllowanceListItem:
    """Offline allowance descriptor returned by list/query endpoints."""

    certificate_id_hex: str
    controller_id: str
    controller_display: str
    asset_id: str
    asset_definition_id: str
    asset_definition_name: str
    asset_definition_alias: Optional[str]
    registered_at_ms: int
    expires_at_ms: int
    policy_expires_at_ms: int
    refresh_at_ms: Optional[int]
    verdict_id_hex: Optional[str]
    attestation_nonce_hex: Optional[str]
    remaining_amount: str
    deadline: Optional[OfflineAllowanceDeadline]
    record: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineAllowanceListItem":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline allowance entry must be an object")

        def require_str(key: str) -> str:
            value = payload.get(key)
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"offline allowance entry missing `{key}`")
            return value

        def optional_str(key: str) -> Optional[str]:
            value = payload.get(key)
            if value is None:
                return None
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"offline allowance `{key}` must be a non-empty string")
            return value

        def required_optional_str(key: str) -> Optional[str]:
            if key not in payload:
                raise RuntimeError(f"offline allowance entry missing `{key}`")
            return optional_str(key)

        def require_int(key: str) -> int:
            value = payload.get(key)
            if not isinstance(value, (int, float)):
                raise RuntimeError(f"offline allowance `{key}` must be numeric")
            return int(value)

        def optional_int(key: str) -> Optional[int]:
            value = payload.get(key)
            if value is None:
                return None
            if not isinstance(value, (int, float)):
                raise RuntimeError(f"offline allowance `{key}` must be numeric when present")
            return int(value)

        record_value = payload.get("record")
        if not isinstance(record_value, Mapping):
            raise RuntimeError("offline allowance entry missing `record` object")

        deadline: Optional[OfflineAllowanceDeadline] = None
        if "deadline_kind" in payload:
            kind = require_str("deadline_kind")
            state = require_str("deadline_state")
            deadline_ms = require_int("deadline_ms")
            ms_remaining = require_int("deadline_ms_remaining")
            deadline = OfflineAllowanceDeadline(
                kind=kind,
                state=state,
                deadline_ms=deadline_ms,
                ms_remaining=ms_remaining,
            )

        return cls(
            certificate_id_hex=require_str("certificate_id_hex"),
            controller_id=require_str("controller_id"),
            controller_display=require_str("controller_display"),
            asset_id=require_str("asset_id"),
            asset_definition_id=require_str("asset_definition_id"),
            asset_definition_name=require_str("asset_definition_name"),
            asset_definition_alias=required_optional_str("asset_definition_alias"),
            registered_at_ms=require_int("registered_at_ms"),
            expires_at_ms=require_int("expires_at_ms"),
            policy_expires_at_ms=require_int("policy_expires_at_ms"),
            refresh_at_ms=optional_int("refresh_at_ms"),
            verdict_id_hex=optional_str("verdict_id_hex"),
            attestation_nonce_hex=optional_str("attestation_nonce_hex"),
            remaining_amount=require_str("remaining_amount"),
            deadline=deadline,
            record=dict(record_value),
        )


@dataclass(frozen=True)
class OfflineAllowanceListPage:
    """Paginated list of offline allowances."""

    items: List[OfflineAllowanceListItem]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineAllowanceListPage":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline allowance page must be an object")
        items_value = payload.get("items", [])
        if items_value is None:
            items_value = []
        if not isinstance(items_value, list):
            raise RuntimeError("offline allowance `items` must be a list")
        try:
            total = int(payload.get("total", len(items_value)))
        except (TypeError, ValueError) as exc:
            raise RuntimeError("offline allowance `total` must be numeric") from exc
        items = [OfflineAllowanceListItem.from_payload(entry) for entry in items_value]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class OfflineCertificateIssueResponse:
    """Issued offline certificate response."""

    certificate_id_hex: str
    certificate: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineCertificateIssueResponse":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline certificate issue response must be an object")
        certificate_id_hex = payload.get("certificate_id_hex")
        if not isinstance(certificate_id_hex, str) or not certificate_id_hex:
            raise RuntimeError("offline certificate issue response missing `certificate_id_hex`")
        certificate = payload.get("certificate")
        if not isinstance(certificate, Mapping):
            raise RuntimeError("offline certificate issue response missing `certificate`")
        return cls(
            certificate_id_hex=certificate_id_hex,
            certificate=dict(certificate),
        )


@dataclass(frozen=True)
class OfflineAllowanceRegisterResponse:
    """Response payload for registering or renewing an offline allowance."""

    certificate_id_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineAllowanceRegisterResponse":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline allowance register response must be an object")
        certificate_id_hex = payload.get("certificate_id_hex")
        if not isinstance(certificate_id_hex, str) or not certificate_id_hex:
            raise RuntimeError("offline allowance register response missing `certificate_id_hex`")
        return cls(certificate_id_hex=certificate_id_hex)


@dataclass(frozen=True)
class OfflineTopUpResponse:
    """Aggregated result for offline allowance top-up (issue + register/renew)."""

    certificate: OfflineCertificateIssueResponse
    registration: OfflineAllowanceRegisterResponse


@dataclass(frozen=True)
class OfflineTransferHistoryEntry:
    """Status transition entry for an offline transfer bundle."""

    status: str
    transitioned_at_ms: int
    verdict_snapshot: Optional[Dict[str, Any]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineTransferHistoryEntry":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline transfer history entry must be an object")
        status = payload.get("status")
        if not isinstance(status, str) or not status:
            raise RuntimeError("offline transfer history entry missing `status`")
        transitioned = payload.get("transitioned_at_ms")
        if not isinstance(transitioned, (int, float)):
            raise RuntimeError("offline transfer history entry missing `transitioned_at_ms`")
        verdict = payload.get("verdict_snapshot")
        if verdict is not None:
            if not isinstance(verdict, Mapping):
                raise RuntimeError("offline transfer history `verdict_snapshot` must be an object")
            verdict_value: Optional[Dict[str, Any]] = dict(verdict)
        else:
            verdict_value = None
        return cls(status=status, transitioned_at_ms=int(transitioned), verdict_snapshot=verdict_value)


@dataclass(frozen=True)
class OfflineTransferListItem:
    """Offline transfer descriptor returned by list/query endpoints."""

    bundle_id_hex: str
    controller_id: str
    controller_display: str
    receiver_id: str
    receiver_display: str
    deposit_account_id: str
    deposit_account_display: str
    asset_id: Optional[str]
    certificate_id_hex: Optional[str]
    certificate_expires_at_ms: Optional[int]
    policy_expires_at_ms: Optional[int]
    refresh_at_ms: Optional[int]
    verdict_id_hex: Optional[str]
    attestation_nonce_hex: Optional[str]
    receipt_count: int
    total_amount: str
    status: str
    recorded_at_ms: int
    recorded_at_height: int
    archived_at_height: Optional[int]
    status_transitions: List[OfflineTransferHistoryEntry]
    claimed_delta: str
    platform_policy: Optional[str]
    platform_token_snapshot: Optional[Dict[str, Any]]
    verdict_snapshot: Optional[Dict[str, Any]]
    transfer: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineTransferListItem":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline transfer entry must be an object")

        def optional_str(key: str) -> Optional[str]:
            value = payload.get(key)
            if value is None:
                return None
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"offline transfer `{key}` must be a non-empty string")
            return value

        def optional_int(key: str) -> Optional[int]:
            value = payload.get(key)
            if value is None:
                return None
            if not isinstance(value, (int, float)):
                raise RuntimeError(f"offline transfer `{key}` must be numeric when present")
            return int(value)

        def require_str(key: str) -> str:
            value = payload.get(key)
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"offline transfer entry missing `{key}`")
            return value

        def require_int(key: str) -> int:
            value = payload.get(key)
            if not isinstance(value, (int, float)):
                raise RuntimeError(f"offline transfer `{key}` must be numeric")
            return int(value)

        transfer_value = payload.get("transfer")
        if not isinstance(transfer_value, Mapping):
            raise RuntimeError("offline transfer entry missing `transfer` object")
        history_value = payload.get("status_transitions", [])
        if history_value is None:
            history_value = []
        if not isinstance(history_value, list):
            raise RuntimeError("offline transfer `status_transitions` must be a list")
        platform_snapshot = payload.get("platform_token_snapshot")
        if platform_snapshot is not None and not isinstance(platform_snapshot, Mapping):
            raise RuntimeError("offline transfer `platform_token_snapshot` must be an object")
        verdict_snapshot = payload.get("verdict_snapshot")
        if verdict_snapshot is not None and not isinstance(verdict_snapshot, Mapping):
            raise RuntimeError("offline transfer `verdict_snapshot` must be an object")

        history = [
            OfflineTransferHistoryEntry.from_payload(entry) for entry in history_value
        ]

        platform_value = dict(platform_snapshot) if isinstance(platform_snapshot, Mapping) else None
        verdict_value = dict(verdict_snapshot) if isinstance(verdict_snapshot, Mapping) else None

        return cls(
            bundle_id_hex=require_str("bundle_id_hex"),
            controller_id=require_str("controller_id"),
            controller_display=require_str("controller_display"),
            receiver_id=require_str("receiver_id"),
            receiver_display=require_str("receiver_display"),
            deposit_account_id=require_str("deposit_account_id"),
            deposit_account_display=require_str("deposit_account_display"),
            asset_id=optional_str("asset_id"),
            certificate_id_hex=optional_str("certificate_id_hex"),
            certificate_expires_at_ms=optional_int("certificate_expires_at_ms"),
            policy_expires_at_ms=optional_int("policy_expires_at_ms"),
            refresh_at_ms=optional_int("refresh_at_ms"),
            verdict_id_hex=optional_str("verdict_id_hex"),
            attestation_nonce_hex=optional_str("attestation_nonce_hex"),
            receipt_count=require_int("receipt_count"),
            total_amount=require_str("total_amount"),
            status=require_str("status"),
            recorded_at_ms=require_int("recorded_at_ms"),
            recorded_at_height=require_int("recorded_at_height"),
            archived_at_height=optional_int("archived_at_height"),
            status_transitions=history,
            claimed_delta=require_str("claimed_delta"),
            platform_policy=optional_str("platform_policy"),
            platform_token_snapshot=platform_value,
            verdict_snapshot=verdict_value,
            transfer=dict(transfer_value),
        )


@dataclass(frozen=True)
class OfflineTransferListPage:
    """Paginated list of offline transfers."""

    items: List[OfflineTransferListItem]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineTransferListPage":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline transfer page must be an object")
        items_value = payload.get("items", [])
        if items_value is None:
            items_value = []
        if not isinstance(items_value, list):
            raise RuntimeError("offline transfer `items` must be a list")
        try:
            total = int(payload.get("total", len(items_value)))
        except (TypeError, ValueError) as exc:
            raise RuntimeError("offline transfer `total` must be numeric") from exc
        items = [OfflineTransferListItem.from_payload(entry) for entry in items_value]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class OfflineSummaryListItem:
    """Controller counter summary returned by offline summaries endpoints."""

    certificate_id_hex: str
    controller_id: str
    controller_display: str
    summary_hash_hex: str
    apple_key_counters: Dict[str, int]
    android_series_counters: Dict[str, int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineSummaryListItem":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline summary entry must be an object")
        certificate_id_hex = payload.get("certificate_id_hex")
        if not isinstance(certificate_id_hex, str) or not certificate_id_hex:
            raise RuntimeError("offline summary entry missing `certificate_id_hex`")
        controller_id = payload.get("controller_id")
        if not isinstance(controller_id, str) or not controller_id:
            raise RuntimeError("offline summary entry missing `controller_id`")
        controller_display = payload.get("controller_display")
        if not isinstance(controller_display, str) or not controller_display:
            raise RuntimeError("offline summary entry missing `controller_display`")
        summary_hash = payload.get("summary_hash_hex")
        if not isinstance(summary_hash, str) or not summary_hash:
            raise RuntimeError("offline summary entry missing `summary_hash_hex`")

        def parse_counter_map(obj: Any, field: str) -> Dict[str, int]:
            if obj is None:
                return {}
            if not isinstance(obj, Mapping):
                raise RuntimeError(f"offline summary `{field}` must be an object")
            result: Dict[str, int] = {}
            for key, value in obj.items():
                if not isinstance(key, str) or not isinstance(value, (int, float)):
                    raise RuntimeError(f"offline summary `{field}` must map strings to numbers")
                result[key] = int(value)
            return result

        apple = parse_counter_map(payload.get("apple_key_counters"), "apple_key_counters")
        android = parse_counter_map(payload.get("android_series_counters"), "android_series_counters")

        return cls(
            certificate_id_hex=certificate_id_hex,
            controller_id=controller_id,
            controller_display=controller_display,
            summary_hash_hex=summary_hash,
            apple_key_counters=apple,
            android_series_counters=android,
        )


@dataclass(frozen=True)
class OfflineSummaryListPage:
    """Paginated list of offline counter summaries."""

    items: List[OfflineSummaryListItem]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineSummaryListPage":
        if not isinstance(payload, Mapping):
            raise RuntimeError("offline summary page must be an object")
        items_value = payload.get("items", [])
        if items_value is None:
            items_value = []
        if not isinstance(items_value, list):
            raise RuntimeError("offline summary `items` must be a list")
        try:
            total = int(payload.get("total", len(items_value)))
        except (TypeError, ValueError) as exc:
            raise RuntimeError("offline summary `total` must be numeric") from exc
        items = [OfflineSummaryListItem.from_payload(entry) for entry in items_value]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class SubscriptionPlanCreateResult:
    """Response from ``POST /v1/subscriptions/plans``."""

    ok: bool
    plan_id: str
    tx_hash_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionPlanCreateResult":
        if not isinstance(payload, Mapping):
            raise RuntimeError("subscription plan create response must be an object")

        def require_str(key: str) -> str:
            value = payload.get(key)
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"subscription plan create response missing `{key}`")
            return value

        ok_value = payload.get("ok")
        if not isinstance(ok_value, bool):
            raise RuntimeError("subscription plan create response missing `ok`")
        return cls(
            ok=ok_value,
            plan_id=require_str("plan_id"),
            tx_hash_hex=require_str("tx_hash_hex"),
        )


@dataclass(frozen=True)
class SubscriptionPlanListItem:
    """Subscription plan record returned from ``GET /v1/subscriptions/plans``."""

    plan_id: str
    plan: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionPlanListItem":
        if not isinstance(payload, Mapping):
            raise RuntimeError("subscription plan list item must be an object")
        plan_id = payload.get("plan_id")
        if not isinstance(plan_id, str) or not plan_id:
            raise RuntimeError("subscription plan list item missing `plan_id`")
        plan_value = payload.get("plan")
        if not isinstance(plan_value, Mapping):
            raise RuntimeError("subscription plan list item missing `plan` object")
        return cls(plan_id=plan_id, plan=dict(plan_value))


@dataclass(frozen=True)
class SubscriptionPlanListPage:
    """Paginated list of subscription plans."""

    items: List[SubscriptionPlanListItem]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionPlanListPage":
        if not isinstance(payload, Mapping):
            raise RuntimeError("subscription plan list response must be an object")
        items_value = payload.get("items", [])
        if items_value is None:
            items_value = []
        if not isinstance(items_value, list):
            raise RuntimeError("subscription plan list `items` must be a list")
        try:
            total = int(payload.get("total", len(items_value)))
        except (TypeError, ValueError) as exc:
            raise RuntimeError("subscription plan list `total` must be numeric") from exc
        items = [SubscriptionPlanListItem.from_payload(entry) for entry in items_value]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class SubscriptionCreateResult:
    """Response from ``POST /v1/subscriptions``."""

    ok: bool
    subscription_id: str
    billing_trigger_id: str
    usage_trigger_id: Optional[str]
    first_charge_ms: int
    tx_hash_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionCreateResult":
        if not isinstance(payload, Mapping):
            raise RuntimeError("subscription create response must be an object")

        def require_str(key: str) -> str:
            value = payload.get(key)
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"subscription create response missing `{key}`")
            return value

        ok_value = payload.get("ok")
        if not isinstance(ok_value, bool):
            raise RuntimeError("subscription create response missing `ok`")
        first_charge = payload.get("first_charge_ms")
        if isinstance(first_charge, bool) or not isinstance(first_charge, (int, float)):
            raise RuntimeError("subscription create response missing `first_charge_ms`")
        usage_trigger = payload.get("usage_trigger_id")
        if usage_trigger is None:
            usage_value = None
        elif isinstance(usage_trigger, str) and usage_trigger:
            usage_value = usage_trigger
        else:
            raise RuntimeError("subscription create response `usage_trigger_id` must be a string when present")
        return cls(
            ok=ok_value,
            subscription_id=require_str("subscription_id"),
            billing_trigger_id=require_str("billing_trigger_id"),
            usage_trigger_id=usage_value,
            first_charge_ms=int(first_charge),
            tx_hash_hex=require_str("tx_hash_hex"),
        )


@dataclass(frozen=True)
class SubscriptionListItem:
    """Subscription record returned by list/get endpoints."""

    subscription_id: str
    subscription: Dict[str, Any]
    invoice: Optional[Dict[str, Any]]
    plan: Optional[Dict[str, Any]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionListItem":
        if not isinstance(payload, Mapping):
            raise RuntimeError("subscription item must be an object")
        subscription_id = payload.get("subscription_id")
        if not isinstance(subscription_id, str) or not subscription_id:
            raise RuntimeError("subscription item missing `subscription_id`")
        subscription_value = payload.get("subscription")
        if not isinstance(subscription_value, Mapping):
            raise RuntimeError("subscription item missing `subscription` object")

        def optional_object(key: str) -> Optional[Dict[str, Any]]:
            value = payload.get(key)
            if value is None:
                return None
            if isinstance(value, Mapping):
                return dict(value)
            raise RuntimeError(f"subscription item `{key}` must be an object when present")

        return cls(
            subscription_id=subscription_id,
            subscription=dict(subscription_value),
            invoice=optional_object("invoice"),
            plan=optional_object("plan"),
        )


@dataclass(frozen=True)
class SubscriptionListPage:
    """Paginated list of subscriptions."""

    items: List[SubscriptionListItem]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionListPage":
        if not isinstance(payload, Mapping):
            raise RuntimeError("subscription list response must be an object")
        items_value = payload.get("items", [])
        if items_value is None:
            items_value = []
        if not isinstance(items_value, list):
            raise RuntimeError("subscription list `items` must be a list")
        try:
            total = int(payload.get("total", len(items_value)))
        except (TypeError, ValueError) as exc:
            raise RuntimeError("subscription list `total` must be numeric") from exc
        items = [SubscriptionListItem.from_payload(entry) for entry in items_value]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class SubscriptionActionResult:
    """Response from subscription pause/resume/cancel/usage actions."""

    ok: bool
    subscription_id: str
    tx_hash_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionActionResult":
        if not isinstance(payload, Mapping):
            raise RuntimeError("subscription action response must be an object")

        def require_str(key: str) -> str:
            value = payload.get(key)
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"subscription action response missing `{key}`")
            return value

        ok_value = payload.get("ok")
        if not isinstance(ok_value, bool):
            raise RuntimeError("subscription action response missing `ok`")
        return cls(
            ok=ok_value,
            subscription_id=require_str("subscription_id"),
            tx_hash_hex=require_str("tx_hash_hex"),
        )


@dataclass(frozen=True)
class TriggerRecord:
    """Trigger definition surfaced by listing/query endpoints."""

    id: str
    action: Dict[str, Any]
    metadata: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TriggerRecord":
        if not isinstance(payload, Mapping):
            raise RuntimeError("trigger record must be an object")
        trigger_id = payload.get("id")
        if not isinstance(trigger_id, str) or not trigger_id:
            raise RuntimeError("trigger record missing `id`")
        action = payload.get("action")
        if not isinstance(action, Mapping):
            raise RuntimeError("trigger record missing `action` object")
        metadata_value = payload.get("metadata", {})
        if metadata_value is None:
            metadata: Dict[str, Any] = {}
        elif isinstance(metadata_value, Mapping):
            metadata = dict(metadata_value)
        else:
            raise RuntimeError("trigger record `metadata` must be an object when present")
        return cls(
            id=trigger_id,
            action=dict(action),
            metadata=metadata,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class TriggerListPage:
    """Paginated trigger listing."""

    items: List[TriggerRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TriggerListPage":
        if not isinstance(payload, Mapping):
            raise RuntimeError("trigger listing payload must be an object")
        items_value = payload.get("items", [])
        if items_value is None:
            items_value = []
        if not isinstance(items_value, list):
            raise RuntimeError("trigger listing `items` must be a list")
        total_value = payload.get("total", len(items_value))
        try:
            total = int(total_value)
        except (TypeError, ValueError) as exc:
            raise RuntimeError("trigger listing `total` must be numeric") from exc
        items = [TriggerRecord.from_payload(entry) for entry in items_value]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class CouncilMember:
    """Single council member descriptor."""

    account_id: str


@dataclass(frozen=True)
class CouncilCurrentStatus:
    """Snapshot returned by ``GET /v1/gov/council/current``."""

    epoch: int
    members: List[CouncilMember]


@dataclass(frozen=True)
class CouncilAuditMetadata:
    """Seed and beacon metadata from ``GET /v1/gov/council/audit``."""

    epoch: int
    seed_hex: str
    beacon_hex: str
    chain_id: str
    members_count: int
    candidate_count: int


@dataclass(frozen=True)
class GovernanceProposalStatus:
    """Result returned by ``GET /v1/gov/proposals/{id}``."""

    found: bool
    proposal: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class GovernanceLocksOverview:
    """Locks/escrow view returned by ``GET /v1/gov/locks/{referendum}``."""

    found: bool
    referendum_id: str
    locks: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class GovernanceReferendumStatus:
    """Referendum lookup response."""

    found: bool
    referendum: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class GovernanceTallySummary:
    """Quadratic tally summary for a referendum."""

    referendum_id: str
    approve: int
    reject: int
    abstain: int


@dataclass(frozen=True)
class GovernanceUnlockStats:
    """Aggregated unlock metrics surfaced via ``/v1/gov/unlocks/stats``."""

    height_current: int
    expired_locks_now: int
    referenda_with_expired: int
    last_sweep_height: int


@dataclass(frozen=True)
class TransactionInstruction:
    """Instruction skeleton emitted by governance helpers."""

    wire_id: str
    payload_hex: str


@dataclass(frozen=True)
class GovernanceInstructionDraft:
    """Instruction bundle returned by finalize/enact helpers."""

    ok: bool
    tx_instructions: List[TransactionInstruction]


@dataclass(frozen=True)
class GovernanceProposalDraft:
    """Result returned by ``POST /v1/gov/proposals/deploy-contract``."""

    ok: bool
    proposal_id: str
    tx_instructions: List[TransactionInstruction]


@dataclass(frozen=True)
class BallotSubmitResult:
    """Response to ``/v1/gov/ballots/*`` submissions."""

    ok: bool
    accepted: bool
    reason: Optional[str]
    tx_instructions: List[TransactionInstruction]


@dataclass(frozen=True)
class ProtectedNamespacesApplyResult:
    """Outcome of ``POST /v1/gov/protected-namespaces``."""

    ok: bool
    applied: int


@dataclass(frozen=True)
class ProtectedNamespacesStatus:
    """Current protected namespace list from ``GET /v1/gov/protected-namespaces``."""

    found: bool
    namespaces: List[str]


@dataclass(frozen=True)
class VrfCandidate:
    """VRF candidate descriptor for council persistence."""

    account_id: str
    variant: str
    pk_b64: str
    proof_b64: str

    def to_payload(self) -> Dict[str, str]:
        return {
            "account_id": self.account_id,
            "variant": self.variant,
            "pk_b64": self.pk_b64,
            "proof_b64": self.proof_b64,
        }


@dataclass(frozen=True)
class CouncilPersistResult:
    """Response from ``POST /v1/gov/council/persist``."""

    epoch: int
    members: List[CouncilMember]
    total_candidates: int
    verified: int


@dataclass(frozen=True)
class LaneCommitmentSnapshot:
    """Aggregated TEU commitment for a Nexus lane."""

    block_height: int
    lane_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash: str


@dataclass(frozen=True)
class DataspaceCommitmentSnapshot:
    """Aggregated TEU commitment for a Nexus dataspace."""

    block_height: int
    lane_id: int
    dataspace_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash: str


@dataclass(frozen=True)
class UaidPortfolioTotals:
    """Aggregate counts returned by ``/v1/accounts/{uaid}/portfolio``."""

    accounts: int
    positions: int


@dataclass(frozen=True)
class UaidPortfolioAsset:
    """Asset holding entry attached to a UAID portfolio."""

    asset_id: str
    asset_definition_id: str
    quantity: str


@dataclass(frozen=True)
class UaidPortfolioAccount:
    """Account entry included in a UAID portfolio."""

    account_id: str
    label: Optional[str]
    assets: List[UaidPortfolioAsset]


@dataclass(frozen=True)
class UaidPortfolioDataspace:
    """Dataspace grouping returned by the UAID portfolio endpoint."""

    dataspace_id: int
    dataspace_alias: Optional[str]
    accounts: List[UaidPortfolioAccount]


@dataclass(frozen=True)
class UaidPortfolioResponse:
    """Typed response for ``GET /v1/accounts/{uaid}/portfolio``."""

    uaid: str
    totals: UaidPortfolioTotals
    dataspaces: List[UaidPortfolioDataspace]


@dataclass(frozen=True)
class UaidBindingsDataspace:
    """Dataspace binding entry surfaced by Space Directory."""

    dataspace_id: int
    dataspace_alias: Optional[str]
    accounts: List[str]


@dataclass(frozen=True)
class UaidBindingsResponse:
    """Typed response for ``GET /v1/space-directory/uaids/{uaid}``."""

    uaid: str
    dataspaces: List[UaidBindingsDataspace]


@dataclass(frozen=True)
class UaidManifestRevocation:
    """Revocation record attached to a UAID manifest lifecycle."""

    epoch: int
    reason: Optional[str]


@dataclass(frozen=True)
class UaidManifestLifecycle:
    """Lifecycle summary for a UAID manifest."""

    activated_epoch: Optional[int]
    expired_epoch: Optional[int]
    revocation: Optional[UaidManifestRevocation]


@dataclass(frozen=True)
class UaidManifestEntry:
    """Scope/effect tuple inside a UAID Space Directory manifest."""

    scope: Dict[str, Any]
    effect: Dict[str, Any]
    notes: Optional[str]


@dataclass(frozen=True)
class UaidManifest:
    """Full manifest payload tracked by Space Directory."""

    version: str
    uaid: str
    dataspace: int
    issued_ms: int
    activation_epoch: int
    expiry_epoch: Optional[int]
    entries: List[UaidManifestEntry]


@dataclass(frozen=True)
class UaidManifestRecord:
    """Space Directory manifest record attached to a UAID."""

    dataspace_id: int
    dataspace_alias: Optional[str]
    manifest_hash: str
    status: str
    lifecycle: UaidManifestLifecycle
    accounts: List[str]
    manifest: UaidManifest


@dataclass(frozen=True)
class UaidManifestsResponse:
    """Typed response for ``GET /v1/space-directory/uaids/{uaid}/manifests``."""

    uaid: str
    manifests: List[UaidManifestRecord]


UAID_MANIFEST_STATUS_VALUES = {"Pending", "Active", "Expired", "Revoked"}


@dataclass(frozen=True)
class LaneRuntimeUpgradeHook:
    """Runtime upgrade policy toggles enforced per lane."""

    allow: bool
    require_metadata: bool
    metadata_key: Optional[str]
    allowed_ids: List[str]


@dataclass(frozen=True)
class LaneGovernanceSnapshot:
    """Lane governance metadata referenced by `/v1/status`."""

    lane_id: int
    alias: str
    dataspace_id: int
    visibility: str
    storage_profile: str
    governance: Optional[str]
    manifest_required: bool
    manifest_ready: bool
    manifest_path: Optional[str]
    validator_ids: List[str]
    quorum: Optional[int]
    protected_namespaces: List[str]
    runtime_upgrade: Optional[LaneRuntimeUpgradeHook]


@dataclass(frozen=True)
class GovernanceProposalCounters:
    """Proposal lifecycle counters inside the status payload."""

    proposed: int
    approved: int
    rejected: int
    enacted: int


@dataclass(frozen=True)
class GovernanceProtectedNamespaceStats:
    """Protected namespace policy counters."""

    total_checks: int
    allowed: int
    rejected: int


@dataclass(frozen=True)
class GovernanceManifestAdmissionStats:
    """Manifest admission outcomes."""

    total_checks: int
    allowed: int
    missing_manifest: int
    non_validator_authority: int
    quorum_rejected: int
    protected_namespace_rejected: int
    runtime_hook_rejected: int


@dataclass(frozen=True)
class GovernanceManifestQuorumStats:
    """Manifest quorum satisfaction metrics."""

    total_checks: int
    satisfied: int
    rejected: int


@dataclass(frozen=True)
class GovernanceManifestActivation:
    """Recent manifest activation record."""

    namespace: str
    contract_id: str
    code_hash_hex: str
    abi_hash_hex: Optional[str]
    height: int
    activated_at_ms: int


@dataclass(frozen=True)
class GovernanceStatusSnapshot:
    """Governance breakdown inside `/v1/status`."""

    proposals: GovernanceProposalCounters
    protected_namespace: GovernanceProtectedNamespaceStats
    manifest_admission: GovernanceManifestAdmissionStats
    manifest_quorum: GovernanceManifestQuorumStats
    recent_manifest_activations: List[GovernanceManifestActivation]


@dataclass(frozen=True)
class StatusMetrics:
    """Derived deltas between successive status snapshots."""

    commit_latency_ms: int
    queue_size: int
    queue_delta: int
    da_reschedule_delta: int
    tx_approved_delta: int
    tx_rejected_delta: int
    view_change_delta: int
    has_activity: bool


@dataclass(frozen=True)
class StatusPayload:
    """Raw `/v1/status` payload with typed fields."""

    mode_tag: Optional[str]
    staged_mode_tag: Optional[str]
    staged_mode_activation_height: Optional[int]
    mode_activation_lag_blocks: Optional[int]
    consensus_caps: Optional["SumeragiConsensusCaps"]
    peers: int
    queue_size: int
    commit_time_ms: int
    da_reschedule_total: int
    txs_approved: int
    txs_rejected: int
    view_changes: int
    governance: Optional[GovernanceStatusSnapshot]
    lane_commitments: List[LaneCommitmentSnapshot]
    dataspace_commitments: List[DataspaceCommitmentSnapshot]
    lane_governance: List[LaneGovernanceSnapshot]
    lane_governance_sealed_total: int
    lane_governance_sealed_aliases: List[str]
    raw: Dict[str, Any]


@dataclass(frozen=True)
class SumeragiConsensusCaps:
    """Consensus handshake configuration caps."""

    collectors_k: int
    redundant_send_r: int
    da_enabled: bool
    rbc_chunk_max_bytes: int
    rbc_session_ttl_ms: int
    rbc_store_max_sessions: int
    rbc_store_soft_sessions: int
    rbc_store_max_bytes: int
    rbc_store_soft_bytes: int


@dataclass(frozen=True)
class StatusSnapshot:
    """Typed Torii status snapshot with derived metrics."""

    timestamp_ms: float
    status: StatusPayload
    metrics: StatusMetrics


class ToriiClient:
    """HTTP helper for Torii attachments, prover, and governance endpoints."""

    def __init__(self, base_url: str, session: Optional[requests.Session] = None) -> None:
        self._base_url = base_url.rstrip("/")
        self._session = session or requests.Session()
        self._status_state = _StatusMetricsState()

    # ------------------------------------------------------------------
    # Attachments
    # ------------------------------------------------------------------
    def upload_attachment(self, data: bytes, *, content_type: str) -> Mapping[str, Any]:
        """Upload an attachment via ``POST /v1/zk/attachments`` and return metadata."""

        response = self._request(
            "POST",
            "/v1/zk/attachments",
            data=data,
            headers={"Content-Type": content_type},
        )
        self._expect_status(response, {201})
        return response.json()

    def list_attachments(self) -> List[Mapping[str, Any]]:
        """Return metadata for all stored attachments."""

        response = self._request("GET", "/v1/zk/attachments")
        self._expect_status(response, {200})
        return response.json()

    def get_attachment(self, attachment_id: str) -> Tuple[bytes, Optional[str]]:
        """Fetch raw attachment bytes and the optional content type."""

        response = self._request("GET", f"/v1/zk/attachments/{attachment_id}")
        self._expect_status(response, {200})
        return response.content, response.headers.get("Content-Type")

    def delete_attachment(self, attachment_id: str) -> None:
        """Delete an attachment by id."""

        response = self._request("DELETE", f"/v1/zk/attachments/{attachment_id}")
        self._expect_status(response, {204})

    # ------------------------------------------------------------------
    # Prover reports
    # ------------------------------------------------------------------
    def list_prover_reports(self, **filters: Any) -> List[Mapping[str, Any]]:
        """List prover reports applying optional filters."""

        response = self._request(
            "GET",
            "/v1/zk/prover/reports",
            params=self._encode_prover_filters(filters),
        )
        self._expect_status(response, {200})
        return response.json()

    def get_prover_report(self, report_id: str) -> Mapping[str, Any]:
        """Fetch a single prover report by id."""

        response = self._request("GET", f"/v1/zk/prover/reports/{report_id}")
        self._expect_status(response, {200})
        return response.json()

    def delete_prover_report(self, report_id: str) -> None:
        """Delete a prover report by id."""

        response = self._request("DELETE", f"/v1/zk/prover/reports/{report_id}")
        self._expect_status(response, {204})

    def count_prover_reports(self, **filters: Any) -> int:
        """Return the count of prover reports matching filters."""

        response = self._request(
            "GET",
            "/v1/zk/prover/reports/count",
            params=self._encode_prover_filters(filters),
        )
        self._expect_status(response, {200})
        payload = response.json()
        if not isinstance(payload, Mapping) or "count" not in payload:
            raise RuntimeError("invalid prover count payload")
        return int(payload["count"])

    # ------------------------------------------------------------------
    # Admin & telemetry surfaces
    # ------------------------------------------------------------------
    def list_peers(self) -> List[PeerInfo]:
        """Return online peers exposed by ``GET /v1/peers``."""

        response = self._request("GET", "/v1/peers")
        self._expect_status(response, {200})
        payload = response.json()
        if not isinstance(payload, list):
            raise RuntimeError("/v1/peers response must be a list")
        return [PeerInfo.from_payload(entry) for entry in payload]

    def list_telemetry_peers_info(self) -> List[PeerTelemetryInfo]:
        """Return telemetry metadata exposed by ``GET /v1/telemetry/peers-info``."""

        response = self._request(
            "GET",
            "/v1/telemetry/peers-info",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = response.json()
        if not isinstance(payload, list):
            raise RuntimeError("/v1/telemetry/peers-info response must be a list")
        peers: List[PeerTelemetryInfo] = []
        for index, entry in enumerate(payload):
            peers.append(
                self._parse_telemetry_peer_info(
                    entry,
                    context=f"telemetry peers[{index}]",
                )
            )
        return peers

    def get_health_status(self) -> str:
        """Return the plain-text payload exposed by ``GET /v1/health``."""

        response = self._request("GET", "/v1/health")
        self._expect_status(response, {200})
        text = response.text.strip()
        if not text:
            raise RuntimeError("/v1/health response was empty")
        return text

    def get_node_version(self) -> str:
        """Return the node version string exposed by ``GET /v1/version``."""

        response = self._request("GET", "/v1/version")
        self._expect_status(response, {200})
        text = response.text.strip()
        if not text:
            raise RuntimeError("/v1/version response was empty")
        return text

    # ------------------------------------------------------------------
    # Explorer helpers
    # ------------------------------------------------------------------
    def get_explorer_account_qr(
        self,
        account_id: str,
    ) -> ExplorerAccountQr:
        """Fetch QR metadata for an account (`GET /v1/explorer/accounts/{account_id}/qr`)."""

        canonical = self._require_non_empty_string(account_id, "account_id")
        response = self._request(
            "GET",
            f"/v1/explorer/accounts/{quote(canonical, safe='')}/qr",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("explorer account QR endpoint returned no payload")
        mapping = self._ensure_mapping(payload, "explorer account QR response")
        return self._parse_explorer_account_qr(mapping, context="explorer account QR response")

    # ------------------------------------------------------------------
    # Configuration & time surfaces
    # ------------------------------------------------------------------
    def get_configuration(self) -> ConfigurationSnapshot:
        """Return the node configuration snapshot (`GET /v1/configuration`)."""

        response = self._request(
            "GET",
            "/v1/configuration",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = response.json()
        mapping = self._ensure_mapping(payload, "configuration response")
        return ConfigurationSnapshot.from_payload(mapping)

    def update_configuration(self, payload: Mapping[str, Any]) -> Mapping[str, Any]:
        """Update node configuration (`POST /v1/configuration`)."""

        response = self._request(
            "POST",
            "/v1/configuration",
            headers={"Content-Type": "application/json"},
            data=json.dumps(dict(payload)).encode("utf-8"),
        )
        self._expect_status(response, {200, 202})
        if not response.content:
            return {}
        body = response.json()
        return self._ensure_mapping(body, "configuration update response")

    def get_confidential_gas_schedule(self) -> Optional[ConfidentialGasSchedule]:
        """Return the advertised confidential verification gas schedule."""

        snapshot = self.get_configuration()
        return snapshot.confidential_gas

    def set_confidential_gas_schedule(
        self,
        *,
        proof_base: int,
        per_public_input: int,
        per_proof_byte: int,
        per_nullifier: int,
        per_commitment: int,
    ) -> Mapping[str, Any]:
        """Update confidential gas schedule while preserving the logger settings."""

        snapshot = self.get_configuration()
        logger_payload = snapshot.logger.to_payload()
        schedule = ConfidentialGasSchedule(
            proof_base=int(proof_base),
            per_public_input=int(per_public_input),
            per_proof_byte=int(per_proof_byte),
            per_nullifier=int(per_nullifier),
            per_commitment=int(per_commitment),
        )
        return self.update_configuration(
            {
                "logger": logger_payload,
                "confidential_gas": schedule.to_payload(),
            }
        )

    def get_time_now(self) -> NetworkTimeSnapshot:
        """Fetch the Network Time Service snapshot (`GET /v1/time/now`)."""

        response = self._request(
            "GET",
            "/v1/time/now",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = response.json()
        mapping = self._ensure_mapping(payload, "network time response")
        return NetworkTimeSnapshot.from_payload(mapping)

    def get_time_status(self) -> NetworkTimeStatus:
        """Fetch Network Time Service diagnostics (`GET /v1/time/status`)."""

        response = self._request(
            "GET",
            "/v1/time/status",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = response.json()
        mapping = self._ensure_mapping(payload, "network time status response")
        return NetworkTimeStatus.from_payload(mapping)

    def get_node_capabilities(self) -> NodeCapabilities:
        """Fetch the node capability advert (`GET /v1/node/capabilities`)."""

        payload = self._get_json_object(
            "/v1/node/capabilities",
            context="node capabilities",
        )
        return self._parse_node_capabilities(payload, context="node capabilities")

    def get_runtime_abi_active(self) -> RuntimeAbiActive:
        """Fetch the active ABI version (`GET /v1/runtime/abi/active`)."""

        payload = self._get_json_object(
            "/v1/runtime/abi/active",
            context="runtime abi active response",
        )
        return self._parse_runtime_abi_active(payload, context="runtime abi active response")

    def get_runtime_abi_hash(self) -> RuntimeAbiHash:
        """Fetch the canonical ABI hash (`GET /v1/runtime/abi/hash`)."""

        payload = self._get_json_object(
            "/v1/runtime/abi/hash",
            context="runtime abi hash response",
        )
        return self._parse_runtime_abi_hash(payload, context="runtime abi hash response")

    def get_runtime_metrics(self) -> RuntimeMetricsSnapshot:
        """Fetch runtime upgrade metrics (`GET /v1/runtime/metrics`)."""

        payload = self._get_json_object(
            "/v1/runtime/metrics",
            context="runtime metrics response",
        )
        return self._parse_runtime_metrics(payload, context="runtime metrics response")

    def list_runtime_upgrades(self) -> List[RuntimeUpgradeListItem]:
        """List runtime upgrade records (`GET /v1/runtime/upgrades`)."""

        payload = self._get_json_object(
            "/v1/runtime/upgrades",
            context="runtime upgrades list response",
        )
        entries = payload.get("items", [])
        if entries is None:
            entries = []
        if not isinstance(entries, list):
            raise RuntimeError("runtime upgrades list response.items must be a list")
        return [
            self._parse_runtime_upgrade_item(
                entry,
                index,
                context="runtime upgrades list response.items",
            )
            for index, entry in enumerate(entries)
        ]

    def propose_runtime_upgrade(self, manifest: Mapping[str, Any]) -> RuntimeUpgradeTxResponse:
        """Propose a runtime upgrade manifest (`POST /v1/runtime/upgrades/propose`)."""

        normalized = self._normalize_runtime_manifest_payload(
            manifest,
            context="runtime upgrade manifest",
        )
        body = self._post_json(
            "/v1/runtime/upgrades/propose",
            normalized,
            context="runtime upgrade propose response",
            expected_status=(200,),
        )
        return self._parse_runtime_upgrade_tx_response(body, context="runtime upgrade propose response")

    def activate_runtime_upgrade(self, upgrade_id_hex: Union[str, bytes, bytearray]) -> RuntimeUpgradeTxResponse:
        """Build activation instructions for a runtime upgrade (`POST /v1/runtime/upgrades/activate/{id}`)."""

        identifier = self._normalize_hex_string(
            upgrade_id_hex,
            context="runtime upgrade activate id",
            expected_length=64,
        )
        response = self._request(
            "POST",
            f"/v1/runtime/upgrades/activate/0x{identifier}",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._ensure_mapping(
            self._maybe_json(response),
            "runtime upgrade activate response",
        )
        return self._parse_runtime_upgrade_tx_response(payload, context="runtime upgrade activate response")

    def cancel_runtime_upgrade(self, upgrade_id_hex: Union[str, bytes, bytearray]) -> RuntimeUpgradeTxResponse:
        """Build cancellation instructions for a runtime upgrade (`POST /v1/runtime/upgrades/cancel/{id}`)."""

        identifier = self._normalize_hex_string(
            upgrade_id_hex,
            context="runtime upgrade cancel id",
            expected_length=64,
        )
        response = self._request(
            "POST",
            f"/v1/runtime/upgrades/cancel/0x{identifier}",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._ensure_mapping(
            self._maybe_json(response),
            "runtime upgrade cancel response",
        )
        return self._parse_runtime_upgrade_tx_response(payload, context="runtime upgrade cancel response")

    def get_status_snapshot(self) -> StatusSnapshot:
        """Fetch Torii status snapshot (`GET /v1/status`)."""

        payload = self._get_json_object(
            "/v1/status",
            context="status snapshot",
        )
        status_payload = self._parse_status_payload(payload, context="status snapshot")
        metrics = self._status_state.record(status_payload)
        return StatusSnapshot(
            timestamp_ms=_monotonic_millis(),
            status=status_payload,
            metrics=metrics,
        )

    # ------------------------------------------------------------------
    # Connect helpers
    # ------------------------------------------------------------------
    def get_connect_status(self) -> ConnectStatusSnapshot:
        """Fetch Connect runtime status (`GET /v1/connect/status`)."""

        payload = self._get_json_object(
            "/v1/connect/status",
            context="connect status",
        )
        return self._parse_connect_status(payload, context="connect status")

    def create_connect_session(self, payload: Mapping[str, Any]) -> ConnectSessionInfo:
        """Create a Connect session (`POST /v1/connect/session`)."""

        body = self._post_json(
            "/v1/connect/session",
            dict(payload),
            context="connect session",
        )
        return self._parse_connect_session(body, context="connect session")

    def delete_connect_session(self, sid: str) -> bool:
        """Delete a Connect session (`DELETE /v1/connect/session/{sid}`)."""

        response = self._request("DELETE", f"/v1/connect/session/{sid}")
        self._expect_status(response, {200, 202, 204, 404})
        return response.status_code != 404

    def list_connect_apps(
        self,
        *,
        limit: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> ConnectAppRegistryPage:
        """List registered Connect applications."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if cursor:
            params["cursor"] = cursor
        payload = self._get_json_object(
            "/v1/connect/app/apps",
            params=params or None,
            context="connect app registry",
        )
        return self._parse_connect_app_page(payload, context="connect app registry")

    def iterate_connect_apps(
        self,
        *,
        limit: Optional[int] = None,
        page_size: Optional[int] = None,
        cursor: Optional[str] = None,
    ):
        """Yield Connect applications by chasing the cursor returned from `/v1/connect/app/apps`."""

        remaining = None if limit is None else int(limit)
        next_cursor = cursor
        while True:
            effective_limit = page_size
            if effective_limit is None and remaining is not None:
                effective_limit = remaining
            page = self.list_connect_apps(limit=effective_limit, cursor=next_cursor)
            for record in page.items:
                if remaining is not None and remaining <= 0:
                    return
                yield record
                if remaining is not None:
                    remaining -= 1
            if not page.next_cursor or (remaining is not None and remaining <= 0):
                return
            next_cursor = page.next_cursor

    def get_connect_app(self, app_id: str) -> Optional[ConnectAppRecord]:
        """Fetch a single Connect app definition."""

        response = self._request(
            "GET",
            f"/v1/connect/app/apps/{app_id}",
            headers={"Accept": "application/json"},
        )
        if response.status_code == 404:
            return None
        self._expect_status(response, {200})
        body = self._ensure_mapping(response.json(), "connect app response")
        return self._parse_connect_app_record(body, context="connect app response")

    def register_connect_app(self, record: Mapping[str, Any]) -> ConnectAppRecord:
        """Register or update a Connect app (`POST /v1/connect/app/apps`)."""

        body = self._post_json(
            "/v1/connect/app/apps",
            dict(record),
            context="connect app registration",
            expected_status=(200, 201),
        )
        return self._parse_connect_app_record(body, context="connect app registration")

    def delete_connect_app(self, app_id: str) -> bool:
        """Delete a Connect app record."""

        response = self._request("DELETE", f"/v1/connect/app/apps/{app_id}")
        self._expect_status(response, {200, 202, 204, 404})
        return response.status_code != 404

    def get_connect_app_policy(self) -> ConnectAppPolicyControls:
        """Fetch Connect app policy controls."""

        body = self._get_json_object(
            "/v1/connect/app/policy",
            context="connect app policy",
        )
        return self._parse_connect_app_policy(body, context="connect app policy")

    def update_connect_app_policy(
        self,
        updates: Mapping[str, Any],
    ) -> ConnectAppPolicyControls:
        """Update Connect app policy controls (`POST /v1/connect/app/policy`)."""

        body = self._post_json(
            "/v1/connect/app/policy",
            dict(updates),
            context="connect app policy",
            expected_status=(200, 202),
        )
        return self._parse_connect_app_policy(body, context="connect app policy")

    def get_connect_admission_manifest(self) -> ConnectAdmissionManifest:
        """Fetch the Connect admission manifest."""

        body = self._get_json_object(
            "/v1/connect/app/manifest",
            context="connect admission manifest",
        )
        return self._parse_connect_manifest(body, context="connect admission manifest")

    def set_connect_admission_manifest(
        self,
        manifest: Mapping[str, Any],
    ) -> ConnectAdmissionManifest:
        """Replace the Connect admission manifest (`PUT /v1/connect/app/manifest`)."""

        response = self._request(
            "PUT",
            "/v1/connect/app/manifest",
            headers={"Content-Type": "application/json"},
            data=json.dumps(dict(manifest)).encode("utf-8"),
        )
        self._expect_status(response, {200})
        body = self._ensure_mapping(response.json(), "connect admission manifest")
        return self._parse_connect_manifest(body, context="connect admission manifest")

    # ------------------------------------------------------------------
    # Subscriptions
    # ------------------------------------------------------------------
    def list_subscription_plans(
        self,
        *,
        provider: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> SubscriptionPlanListPage:
        """List subscription plans via ``GET /v1/subscriptions/plans``."""

        params: Dict[str, Any] = {}
        if provider is not None:
            params["provider"] = self._normalize_optional_string(
                provider,
                "subscriptions.plans.provider",
            )
        limit_value = self._normalize_optional_int(limit, "subscriptions.plans.limit")
        if limit_value is not None:
            params["limit"] = limit_value
        offset_value = self._normalize_optional_int(
            offset,
            "subscriptions.plans.offset",
            allow_zero=True,
        )
        if offset_value is not None:
            params["offset"] = offset_value
        response = self._request(
            "GET",
            "/v1/subscriptions/plans",
            params=self._clean_params(params),
        )
        self._expect_status(response, {200})
        return SubscriptionPlanListPage.from_payload(response.json())

    def create_subscription_plan(
        self,
        *,
        authority: str,
        private_key: str,
        plan_id: str,
        plan: Mapping[str, Any],
    ) -> SubscriptionPlanCreateResult:
        """Create a subscription plan via ``POST /v1/subscriptions/plans``."""

        payload = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription plan authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "subscription plan private_key",
            ),
            "plan_id": self._require_non_empty_string(
                plan_id,
                "subscription plan plan_id",
            ),
            "plan": self._clone_json_payload(plan, context="subscription plan"),
        }
        body = self._post_json(
            "/v1/subscriptions/plans",
            payload,
            context="subscription plan create response",
            expected_status=(200,),
        )
        return SubscriptionPlanCreateResult.from_payload(body)

    def list_subscriptions(
        self,
        *,
        owned_by: Optional[str] = None,
        provider: Optional[str] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> SubscriptionListPage:
        """List subscriptions via ``GET /v1/subscriptions``."""

        params: Dict[str, Any] = {}
        if owned_by is not None:
            params["owned_by"] = self._normalize_optional_string(
                owned_by,
                "subscriptions.owned_by",
            )
        if provider is not None:
            params["provider"] = self._normalize_optional_string(
                provider,
                "subscriptions.provider",
            )
        if status is not None:
            params["status"] = self._normalize_subscription_status(
                status,
                "subscriptions.status",
            )
        limit_value = self._normalize_optional_int(limit, "subscriptions.limit")
        if limit_value is not None:
            params["limit"] = limit_value
        offset_value = self._normalize_optional_int(
            offset,
            "subscriptions.offset",
            allow_zero=True,
        )
        if offset_value is not None:
            params["offset"] = offset_value
        response = self._request(
            "GET",
            "/v1/subscriptions",
            params=self._clean_params(params),
        )
        self._expect_status(response, {200})
        return SubscriptionListPage.from_payload(response.json())

    def create_subscription(
        self,
        *,
        authority: str,
        private_key: str,
        subscription_id: str,
        plan_id: str,
        billing_trigger_id: Optional[str] = None,
        usage_trigger_id: Optional[str] = None,
        first_charge_ms: Optional[int] = None,
        grant_usage_to_provider: Optional[bool] = None,
    ) -> SubscriptionCreateResult:
        """Create a subscription via ``POST /v1/subscriptions``."""

        payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "subscription private_key",
            ),
            "subscription_id": self._require_non_empty_string(
                subscription_id,
                "subscription subscription_id",
            ),
            "plan_id": self._require_non_empty_string(
                plan_id,
                "subscription plan_id",
            ),
        }
        billing_value = self._normalize_optional_string(
            billing_trigger_id,
            "subscription billing_trigger_id",
        )
        if billing_value is not None:
            payload["billing_trigger_id"] = billing_value
        usage_value = self._normalize_optional_string(
            usage_trigger_id,
            "subscription usage_trigger_id",
        )
        if usage_value is not None:
            payload["usage_trigger_id"] = usage_value
        charge_value = self._normalize_optional_int(
            first_charge_ms,
            "subscription first_charge_ms",
            allow_zero=True,
        )
        if charge_value is not None:
            payload["first_charge_ms"] = charge_value
        if grant_usage_to_provider is not None:
            payload["grant_usage_to_provider"] = self._coerce_bool(
                grant_usage_to_provider,
                "subscription grant_usage_to_provider",
            )
        body = self._post_json(
            "/v1/subscriptions",
            payload,
            context="subscription create response",
            expected_status=(200,),
        )
        return SubscriptionCreateResult.from_payload(body)

    def get_subscription(self, subscription_id: str) -> Optional[SubscriptionListItem]:
        """Fetch a single subscription (`GET /v1/subscriptions/{subscription_id}`)."""

        normalized_id = self._require_non_empty_string(
            subscription_id,
            "subscription_id",
        )
        encoded_id = quote(normalized_id, safe="")
        response = self._request("GET", f"/v1/subscriptions/{encoded_id}")
        if response.status_code == 404:
            return None
        self._expect_status(response, {200})
        payload = self._ensure_mapping(response.json(), "subscription get response")
        return SubscriptionListItem.from_payload(payload)

    def pause_subscription(
        self,
        subscription_id: str,
        *,
        authority: str,
        private_key: str,
    ) -> SubscriptionActionResult:
        """Pause a subscription (`POST /v1/subscriptions/{subscription_id}/pause`)."""

        normalized_id = self._require_non_empty_string(
            subscription_id,
            "subscription_id",
        )
        encoded_id = quote(normalized_id, safe="")
        payload = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription pause authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "subscription pause private_key",
            ),
        }
        body = self._post_json(
            f"/v1/subscriptions/{encoded_id}/pause",
            payload,
            context="subscription pause response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def resume_subscription(
        self,
        subscription_id: str,
        *,
        authority: str,
        private_key: str,
        charge_at_ms: Optional[int] = None,
    ) -> SubscriptionActionResult:
        """Resume a subscription (`POST /v1/subscriptions/{subscription_id}/resume`)."""

        normalized_id = self._require_non_empty_string(
            subscription_id,
            "subscription_id",
        )
        encoded_id = quote(normalized_id, safe="")
        payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription resume authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "subscription resume private_key",
            ),
        }
        charge_value = self._normalize_optional_int(
            charge_at_ms,
            "subscription resume charge_at_ms",
            allow_zero=True,
        )
        if charge_value is not None:
            payload["charge_at_ms"] = charge_value
        body = self._post_json(
            f"/v1/subscriptions/{encoded_id}/resume",
            payload,
            context="subscription resume response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def cancel_subscription(
        self,
        subscription_id: str,
        *,
        authority: str,
        private_key: str,
    ) -> SubscriptionActionResult:
        """Cancel a subscription (`POST /v1/subscriptions/{subscription_id}/cancel`)."""

        normalized_id = self._require_non_empty_string(
            subscription_id,
            "subscription_id",
        )
        encoded_id = quote(normalized_id, safe="")
        payload = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription cancel authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "subscription cancel private_key",
            ),
        }
        body = self._post_json(
            f"/v1/subscriptions/{encoded_id}/cancel",
            payload,
            context="subscription cancel response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def keep_subscription(
        self,
        subscription_id: str,
        *,
        authority: str,
        private_key: str,
    ) -> SubscriptionActionResult:
        """Keep a subscription (`POST /v1/subscriptions/{subscription_id}/keep`)."""

        normalized_id = self._require_non_empty_string(
            subscription_id,
            "subscription_id",
        )
        encoded_id = quote(normalized_id, safe="")
        payload = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription keep authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "subscription keep private_key",
            ),
        }
        body = self._post_json(
            f"/v1/subscriptions/{encoded_id}/keep",
            payload,
            context="subscription keep response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def charge_subscription_now(
        self,
        subscription_id: str,
        *,
        authority: str,
        private_key: str,
        charge_at_ms: Optional[int] = None,
    ) -> SubscriptionActionResult:
        """Charge a subscription now (`POST /v1/subscriptions/{subscription_id}/charge-now`)."""

        normalized_id = self._require_non_empty_string(
            subscription_id,
            "subscription_id",
        )
        encoded_id = quote(normalized_id, safe="")
        payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription charge authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "subscription charge private_key",
            ),
        }
        charge_value = self._normalize_optional_int(
            charge_at_ms,
            "subscription charge_at_ms",
            allow_zero=True,
        )
        if charge_value is not None:
            payload["charge_at_ms"] = charge_value
        body = self._post_json(
            f"/v1/subscriptions/{encoded_id}/charge-now",
            payload,
            context="subscription charge-now response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def record_subscription_usage(
        self,
        subscription_id: str,
        *,
        authority: str,
        private_key: str,
        unit_key: str,
        delta: Any,
        usage_trigger_id: Optional[str] = None,
    ) -> SubscriptionActionResult:
        """Record usage for a subscription (`POST /v1/subscriptions/{subscription_id}/usage`)."""

        normalized_id = self._require_non_empty_string(
            subscription_id,
            "subscription_id",
        )
        encoded_id = quote(normalized_id, safe="")
        payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription usage authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "subscription usage private_key",
            ),
            "unit_key": self._require_non_empty_string(
                unit_key,
                "subscription usage unit_key",
            ),
            "delta": self._normalize_numeric_literal(
                delta,
                "subscription usage delta",
            ),
        }
        usage_value = self._normalize_optional_string(
            usage_trigger_id,
            "subscription usage usage_trigger_id",
        )
        if usage_value is not None:
            payload["usage_trigger_id"] = usage_value
        body = self._post_json(
            f"/v1/subscriptions/{encoded_id}/usage",
            payload,
            context="subscription usage response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    # ------------------------------------------------------------------
    # UAID & Space Directory surfaces
    # ------------------------------------------------------------------
    def get_uaid_portfolio(
        self,
        uaid: str,
        *,
        asset_id: Optional[str] = None,
    ) -> UaidPortfolioResponse:
        """Fetch aggregated holdings for a UAID (`GET /v1/accounts/{uaid}/portfolio`).

        Provide ``asset_id`` to restrict the response to matching positions.
        """

        canonical = self._normalize_uaid_literal(uaid, context="uaid")
        params: Dict[str, Any] = {}
        if asset_id is not None:
            params["asset_id"] = self._normalize_optional_string(
                asset_id,
                "uaid portfolio asset_id",
            )
        response = self._request(
            "GET",
            f"/v1/accounts/{quote(canonical, safe='')}/portfolio",
            headers={"Accept": "application/json"},
            params=self._clean_params(params),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("uaid portfolio endpoint returned no payload")
        mapping = self._ensure_mapping(payload, "uaid portfolio response")
        return self._parse_uaid_portfolio_response(mapping, context="uaid portfolio response")

    def get_uaid_bindings(
        self,
        uaid: str,
    ) -> UaidBindingsResponse:
        """Fetch dataspace bindings for a UAID (`GET /v1/space-directory/uaids/{uaid}`)."""

        canonical = self._normalize_uaid_literal(uaid, context="uaid")
        response = self._request(
            "GET",
            f"/v1/space-directory/uaids/{quote(canonical, safe='')}",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("uaid bindings endpoint returned no payload")
        mapping = self._ensure_mapping(payload, "uaid bindings response")
        return self._parse_uaid_bindings_response(mapping, context="uaid bindings response")

    def get_uaid_manifests(
        self,
        uaid: str,
        *,
        dataspace_id: Optional[int] = None,
    ) -> UaidManifestsResponse:
        """Fetch Space Directory manifests for a UAID (`GET /v1/space-directory/uaids/{uaid}/manifests`)."""

        canonical = self._normalize_uaid_literal(uaid, context="uaid")
        params: Dict[str, Any] = {}
        if dataspace_id is not None:
            params["dataspace"] = self._coerce_unsigned(dataspace_id, "get_uaid_manifests.dataspace_id")
        response = self._request(
            "GET",
            f"/v1/space-directory/uaids/{quote(canonical, safe='')}/manifests",
            params=self._clean_params(params),
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("uaid manifests endpoint returned no payload")
        mapping = self._ensure_mapping(payload, "uaid manifests response")
        return self._parse_uaid_manifests_response(mapping, context="uaid manifests response")

    def publish_space_directory_manifest(
        self,
        *,
        authority: str,
        private_key: str,
        manifest: Mapping[str, Any],
        reason: Optional[str] = None,
    ) -> Optional[Mapping[str, Any]]:
        """Publish (or rotate) a Space Directory manifest (`POST /v1/space-directory/manifests`)."""

        payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "publish_space_directory_manifest.authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "publish_space_directory_manifest.private_key",
            ),
            "manifest": self._clone_json_payload(
                manifest,
                context="publish_space_directory_manifest.manifest",
            ),
        }
        if reason is not None:
            payload["reason"] = self._require_string(
                reason,
                "publish_space_directory_manifest.reason",
            )
        response = self._request(
            "POST",
            "/v1/space-directory/manifests",
            headers={
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
            data=json.dumps(payload).encode("utf-8"),
        )
        self._expect_status(response, {202})
        ack = self._maybe_json(response)
        if not ack:
            return None
        return self._ensure_mapping(ack, "space directory manifest publish response")

    def revoke_space_directory_manifest(
        self,
        *,
        authority: str,
        private_key: str,
        uaid: str,
        dataspace: int,
        revoked_epoch: int,
        reason: Optional[str] = None,
    ) -> Optional[Mapping[str, Any]]:
        """Revoke an active Space Directory manifest (`POST /v1/space-directory/manifests/revoke`)."""

        payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "revoke_space_directory_manifest.authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "revoke_space_directory_manifest.private_key",
            ),
            "uaid": self._normalize_uaid_literal(
                uaid,
                context="revoke_space_directory_manifest.uaid",
            ),
            "dataspace": self._coerce_unsigned(
                dataspace,
                "revoke_space_directory_manifest.dataspace",
            ),
            "revoked_epoch": self._coerce_unsigned(
                revoked_epoch,
                "revoke_space_directory_manifest.revoked_epoch",
            ),
        }
        if reason is not None:
            payload["reason"] = self._require_string(
                reason,
                "revoke_space_directory_manifest.reason",
            )
        response = self._request(
            "POST",
            "/v1/space-directory/manifests/revoke",
            headers={
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
            data=json.dumps(payload).encode("utf-8"),
        )
        self._expect_status(response, {202})
        ack = self._maybe_json(response)
        if not ack:
            return None
        return self._ensure_mapping(ack, "space directory manifest revoke response")

    # ------------------------------------------------------------------
    # Offline allowances and transfers
    # ------------------------------------------------------------------
    def issue_offline_certificate(
        self,
        certificate: Mapping[str, Any],
    ) -> OfflineCertificateIssueResponse:
        """Issue a signed offline certificate via ``POST /v1/offline/certificates/issue``."""

        payload = {
            "certificate": self._clone_json_payload(
                certificate,
                context="offline certificate",
            )
        }
        body = self._post_json(
            "/v1/offline/certificates/issue",
            payload,
            context="offline certificate issue response",
        )
        return OfflineCertificateIssueResponse.from_payload(body)

    def issue_offline_certificate_renewal(
        self,
        certificate_id_hex: str,
        certificate: Mapping[str, Any],
    ) -> OfflineCertificateIssueResponse:
        """Issue a renewal certificate via ``POST /v1/offline/certificates/{id}/renew/issue``."""

        normalized_id = self._require_non_empty_string(
            certificate_id_hex,
            "offline certificate renewal id",
        )
        payload = {
            "certificate": self._clone_json_payload(
                certificate,
                context="offline certificate renewal",
            )
        }
        body = self._post_json(
            f"/v1/offline/certificates/{normalized_id}/renew/issue",
            payload,
            context="offline certificate renewal response",
        )
        return OfflineCertificateIssueResponse.from_payload(body)

    def register_offline_allowance(
        self,
        *,
        certificate: Mapping[str, Any],
        authority: str,
        private_key: str,
    ) -> OfflineAllowanceRegisterResponse:
        """Register an offline allowance via ``POST /v1/offline/allowances``."""

        payload = {
            "authority": self._require_non_empty_string(
                authority,
                "offline allowance authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "offline allowance private_key",
            ),
            "certificate": self._clone_json_payload(
                certificate,
                context="offline allowance certificate",
            ),
        }
        body = self._post_json(
            "/v1/offline/allowances",
            payload,
            context="offline allowance register response",
        )
        return OfflineAllowanceRegisterResponse.from_payload(body)

    def renew_offline_allowance(
        self,
        *,
        certificate_id_hex: str,
        certificate: Mapping[str, Any],
        authority: str,
        private_key: str,
    ) -> OfflineAllowanceRegisterResponse:
        """Renew an offline allowance via ``POST /v1/offline/allowances/{id}/renew``."""

        normalized_id = self._require_non_empty_string(
            certificate_id_hex,
            "offline allowance renewal id",
        )
        payload = {
            "authority": self._require_non_empty_string(
                authority,
                "offline allowance renewal authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "offline allowance renewal private_key",
            ),
            "certificate": self._clone_json_payload(
                certificate,
                context="offline allowance renewal certificate",
            ),
        }
        body = self._post_json(
            f"/v1/offline/allowances/{normalized_id}/renew",
            payload,
            context="offline allowance renewal response",
        )
        return OfflineAllowanceRegisterResponse.from_payload(body)

    def top_up_offline_allowance(
        self,
        *,
        certificate: Mapping[str, Any],
        authority: str,
        private_key: str,
    ) -> OfflineTopUpResponse:
        """Issue and register an offline allowance in one call."""

        issued = self.issue_offline_certificate(certificate)
        registration = self.register_offline_allowance(
            certificate=issued.certificate,
            authority=authority,
            private_key=private_key,
        )
        if issued.certificate_id_hex.strip().lower() != registration.certificate_id_hex.strip().lower():
            raise RuntimeError(
                "offline top-up certificate mismatch "
                f"(issued {issued.certificate_id_hex}, registered {registration.certificate_id_hex})"
            )
        return OfflineTopUpResponse(certificate=issued, registration=registration)

    def top_up_offline_allowance_renewal(
        self,
        *,
        certificate_id_hex: str,
        certificate: Mapping[str, Any],
        authority: str,
        private_key: str,
    ) -> OfflineTopUpResponse:
        """Issue and register a renewed offline allowance in one call."""

        issued = self.issue_offline_certificate_renewal(
            certificate_id_hex,
            certificate,
        )
        registration = self.renew_offline_allowance(
            certificate_id_hex=certificate_id_hex,
            certificate=issued.certificate,
            authority=authority,
            private_key=private_key,
        )
        if issued.certificate_id_hex.strip().lower() != registration.certificate_id_hex.strip().lower():
            raise RuntimeError(
                "offline top-up renewal certificate mismatch "
                f"(issued {issued.certificate_id_hex}, registered {registration.certificate_id_hex})"
            )
        return OfflineTopUpResponse(certificate=issued, registration=registration)

    def list_offline_allowances(self, **params: Any) -> OfflineAllowanceListPage:
        """List offline allowances via ``GET /v1/offline/allowances``."""

        response = self._request(
            "GET",
            "/v1/offline/allowances",
            params=self._clean_params(params),
        )
        self._expect_status(response, {200})
        return OfflineAllowanceListPage.from_payload(response.json())

    def query_offline_allowances(
        self,
        envelope: Mapping[str, Any],
    ) -> OfflineAllowanceListPage:
        """Query offline allowances via ``POST /v1/offline/allowances/query``."""

        payload = self._post_json(
            "/v1/offline/allowances/query",
            dict(envelope),
            context="offline allowances query",
        )
        return OfflineAllowanceListPage.from_payload(payload)

    def list_offline_transfers(self, **params: Any) -> OfflineTransferListPage:
        """List offline transfers via ``GET /v1/offline/transfers``."""

        response = self._request(
            "GET",
            "/v1/offline/transfers",
            params=self._clean_params(params),
        )
        self._expect_status(response, {200})
        return OfflineTransferListPage.from_payload(response.json())

    def query_offline_transfers(
        self,
        envelope: Mapping[str, Any],
    ) -> OfflineTransferListPage:
        """Query offline transfers via ``POST /v1/offline/transfers/query``."""

        payload = self._post_json(
            "/v1/offline/transfers/query",
            dict(envelope),
            context="offline transfers query",
        )
        return OfflineTransferListPage.from_payload(payload)

    def list_offline_summaries(self, **params: Any) -> OfflineSummaryListPage:
        """List offline counter summaries via ``GET /v1/offline/summaries``."""

        response = self._request(
            "GET",
            "/v1/offline/summaries",
            params=self._clean_params(params),
        )
        self._expect_status(response, {200})
        return OfflineSummaryListPage.from_payload(response.json())

    def query_offline_summaries(
        self,
        envelope: Mapping[str, Any],
    ) -> OfflineSummaryListPage:
        """Query offline counter summaries via ``POST /v1/offline/summaries/query``."""

        payload = self._post_json(
            "/v1/offline/summaries/query",
            dict(envelope),
            context="offline summaries query",
        )
        return OfflineSummaryListPage.from_payload(payload)

    # ------------------------------------------------------------------
    # Sumeragi telemetry & RBC helpers
    # ------------------------------------------------------------------
    def get_sumeragi_rbc(self) -> SumeragiRbcSnapshot:
        """Return aggregated RBC counters (`GET /v1/sumeragi/rbc`)."""

        payload = self._get_json_object(
            "/v1/sumeragi/rbc",
            context="sumeragi rbc snapshot",
        )
        return self._parse_sumeragi_rbc_snapshot(payload, context="sumeragi rbc snapshot")

    def get_sumeragi_rbc_sessions(self) -> SumeragiRbcSessionsSnapshot:
        """Return active RBC sessions (`GET /v1/sumeragi/rbc/sessions`)."""

        payload = self._get_json_object(
            "/v1/sumeragi/rbc/sessions",
            context="sumeragi rbc sessions",
        )
        return self._parse_sumeragi_rbc_sessions(payload, context="sumeragi rbc sessions")

    def get_sumeragi_rbc_delivered(self, height: Any, view: Any) -> Optional[SumeragiRbcDeliveryStatus]:
        """Lookup delivery state for a specific session (`GET /v1/sumeragi/rbc/delivered/..`)."""

        normalized_height = self._coerce_unsigned(height, "sumeragi height")
        normalized_view = self._coerce_unsigned(view, "sumeragi view")
        response = self._request(
            "GET",
            f"/v1/sumeragi/rbc/delivered/{normalized_height}/{normalized_view}",
            headers={"Accept": "application/json"},
        )
        if response.status_code == 404:
            return None
        self._expect_status(response, {200})
        payload = self._ensure_mapping(response.json(), "sumeragi rbc delivered response")
        return self._parse_sumeragi_rbc_delivery_status(payload, context="sumeragi rbc delivered response")

    def sample_rbc_chunks(
        self,
        *,
        block_hash: str,
        height: Any,
        view: Any,
        count: Optional[Any] = None,
        seed: Optional[Any] = None,
        api_token: Optional[str] = None,
    ) -> Optional[RbcSample]:
        """Request chunk proofs for an RBC session (`POST /v1/sumeragi/rbc/sample`)."""

        payload: Dict[str, Any] = {
            "block_hash": self._require_hex_string(block_hash, "block_hash"),
            "height": self._coerce_unsigned(height, "rbc_sample.height"),
            "view": self._coerce_unsigned(view, "rbc_sample.view"),
        }
        if count is not None:
            count_value = self._coerce_unsigned(count, "rbc_sample.count")
            if count_value == 0:
                raise RuntimeError("rbc_sample.count must be greater than zero")
            payload["count"] = count_value
        if seed is not None:
            payload["seed"] = self._coerce_unsigned(seed, "rbc_sample.seed")
        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
        }
        if api_token:
            headers["X-API-Token"] = str(api_token)
        response = self._request(
            "POST",
            "/v1/sumeragi/rbc/sample",
            headers=headers,
            data=json.dumps(payload).encode("utf-8"),
        )
        if response.status_code == 404:
            return None
        if response.status_code == 401:
            raise RuntimeError("RBC sampling requires a valid X-API-Token")
        self._expect_status(response, {200})
        body = self._ensure_mapping(response.json(), "sumeragi rbc sample response")
        return self._parse_rbc_sample(body, context="sumeragi rbc sample response")

    # ------------------------------------------------------------------
    # Kaigi relay helpers
    # ------------------------------------------------------------------
    def list_kaigi_relays(self) -> KaigiRelaySummaryList:
        """Return registered Kaigi relays via ``GET /v1/kaigi/relays``."""

        response = self._request(
            "GET",
            "/v1/kaigi/relays",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._ensure_mapping(response.json(), "kaigi relay summary response")
        return self._parse_kaigi_relay_summary_list(payload, context="kaigi relay summary response")

    def get_kaigi_relay(self, relay_id: str) -> Optional[KaigiRelayDetail]:
        """Return detailed metadata for a specific relay via ``GET /v1/kaigi/relays/{relay_id}``."""

        response = self._request(
            "GET",
            f"/v1/kaigi/relays/{relay_id}",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200, 404})
        if response.status_code == 404 or not response.content:
            return None
        payload = self._ensure_mapping(response.json(), "kaigi relay detail response")
        return self._parse_kaigi_relay_detail(payload, context="kaigi relay detail response")

    def get_kaigi_relays_health(self) -> KaigiRelayHealthSnapshot:
        """Return aggregated Kaigi relay health metrics via ``GET /v1/kaigi/relays/health``."""

        response = self._request(
            "GET",
            "/v1/kaigi/relays/health",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._ensure_mapping(response.json(), "kaigi relay health snapshot")
        return self._parse_kaigi_relay_health_snapshot(payload, context="kaigi relay health snapshot")

    def get_sumeragi_qc(self) -> SumeragiQcSnapshot:
        """Fetch HighestQC/LockedQC snapshot (`GET /v1/sumeragi/qc`)."""

        payload = self._ensure_mapping(
            self._request("GET", "/v1/sumeragi/qc").json(),
            "sumeragi qc",
        )
        highest = self._parse_sumeragi_qc_entry(payload.get("highest_qc"), context="sumeragi qc.highest_qc")
        locked = self._parse_sumeragi_qc_entry(payload.get("locked_qc"), context="sumeragi qc.locked_qc")
        return SumeragiQcSnapshot(highest_qc=highest, locked_qc=locked)

    def get_sumeragi_pacemaker(self) -> SumeragiPacemakerSnapshot:
        """Fetch pacemaker configuration snapshot (`GET /v1/sumeragi/pacemaker`)."""

        payload = self._ensure_mapping(
            self._request("GET", "/v1/sumeragi/pacemaker").json(),
            "sumeragi pacemaker",
        )
        return self._parse_sumeragi_pacemaker(payload, context="sumeragi pacemaker")

    def get_sumeragi_phases(self) -> SumeragiPhasesSnapshot:
        """Fetch phase latency counters (`GET /v1/sumeragi/phases`)."""

        payload = self._ensure_mapping(
            self._request("GET", "/v1/sumeragi/phases").json(),
            "sumeragi phases",
        )
        return self._parse_sumeragi_phases(payload, context="sumeragi phases")

    def get_sumeragi_leader(self) -> SumeragiLeaderSnapshot:
        """Fetch leader/PRF state (`GET /v1/sumeragi/leader`)."""

        payload = self._ensure_mapping(
            self._request("GET", "/v1/sumeragi/leader").json(),
            "sumeragi leader",
        )
        leader_index = self._coerce_unsigned(payload.get("leader_index"), "sumeragi leader.leader_index")
        prf = self._parse_sumeragi_prf(payload.get("prf"), context="sumeragi leader.prf")
        return SumeragiLeaderSnapshot(leader_index=leader_index, prf=prf)

    def get_sumeragi_collectors(self) -> SumeragiCollectorsSnapshot:
        """Fetch collector selection snapshot (`GET /v1/sumeragi/collectors`)."""

        payload = self._ensure_mapping(
            self._request("GET", "/v1/sumeragi/collectors").json(),
            "sumeragi collectors",
        )
        return self._parse_sumeragi_collectors(payload, context="sumeragi collectors")

    def get_sumeragi_params(self) -> SumeragiParamsSnapshot:
        """Fetch on-chain Sumeragi parameters (`GET /v1/sumeragi/params`)."""

        payload = self._ensure_mapping(
            self._request("GET", "/v1/sumeragi/params").json(),
            "sumeragi params",
        )
        return self._parse_sumeragi_params(payload, context="sumeragi params")

    def get_sumeragi_bls_keys(self) -> Dict[str, Optional[str]]:
        """Return mapping of network keys to optional BLS public keys (`GET /v1/sumeragi/bls_keys`)."""

        payload = self._request("GET", "/v1/sumeragi/bls_keys").json()
        if not isinstance(payload, Mapping):
            raise RuntimeError("sumeragi bls_keys response must be an object")
        result: Dict[str, Optional[str]] = {}
        for key, value in payload.items():
            if not isinstance(key, str) or not key:
                raise RuntimeError("sumeragi bls_keys keys must be non-empty strings")
            if value is None:
                result[key] = None
            elif isinstance(value, str):
                stripped = value.strip()
                if not stripped:
                    raise RuntimeError(f"sumeragi bls_keys[{key}] must be a hex string or null")
                try:
                    bytes.fromhex(stripped)
                except ValueError as exc:
                    raise RuntimeError(f"sumeragi bls_keys[{key}] must be a hex string or null") from exc
                result[key] = stripped
            else:
                raise RuntimeError(f"sumeragi bls_keys[{key}] must be a string or null")
        return result

    def list_sumeragi_evidence(
        self,
        *,
        limit: Optional[Any] = None,
        offset: Optional[Any] = None,
        kind: Optional[str] = None,
    ) -> SumeragiEvidenceListPage:
        """List recorded consensus evidence (`GET /v1/sumeragi/evidence`)."""

        params: Dict[str, Any] = {}
        if limit is not None:
            normalized_limit = self._coerce_unsigned(limit, "sumeragi evidence limit")
            if normalized_limit <= 0:
                raise RuntimeError("sumeragi evidence limit must be positive")
            if normalized_limit > 1000:
                raise RuntimeError("sumeragi evidence limit must be <= 1000")
            params["limit"] = normalized_limit
        if offset is not None:
            params["offset"] = self._coerce_unsigned(offset, "sumeragi evidence offset")
        if kind is not None:
            literal = self._require_non_empty_string(kind, "sumeragi evidence kind")
            if literal not in SUMERAGI_EVIDENCE_KIND_FILTERS:
                allowed = ", ".join(sorted(SUMERAGI_EVIDENCE_KIND_FILTERS))
                raise RuntimeError(f"sumeragi evidence kind must be one of: {allowed}")
            params["kind"] = literal
        payload = self._get_json_object(
            "/v1/sumeragi/evidence",
            context="sumeragi evidence listing",
            params=params or None,
        )
        return self._parse_sumeragi_evidence_page(payload, context="sumeragi evidence listing")

    def get_sumeragi_evidence_count(self) -> int:
        """Return number of evidence entries observed by the node (`GET /v1/sumeragi/evidence/count`)."""

        payload = self._ensure_mapping(
            self._request("GET", "/v1/sumeragi/evidence/count").json(),
            "sumeragi evidence count",
        )
        return self._coerce_unsigned(payload.get("count"), "sumeragi evidence count.count")

    # ------------------------------------------------------------------
    # Governance & council helpers
    # ------------------------------------------------------------------
    def get_governance_proposal(self, proposal_id: str) -> GovernanceProposalStatus:
        """Fetch proposal metadata via ``GET /v1/gov/proposals/{id}``."""

        payload = self._get_json_object(
            f"/v1/gov/proposals/{proposal_id}",
            context="governance proposal",
        )
        found = bool(payload.get("found"))
        proposal = self._optional_mapping(payload.get("proposal"), context="governance proposal body")
        return GovernanceProposalStatus(found=found, proposal=proposal)

    def get_governance_locks(self, referendum_id: str) -> GovernanceLocksOverview:
        """Return lock escrow information for a referendum."""

        payload = self._get_json_object(
            f"/v1/gov/locks/{referendum_id}",
            context="governance locks",
        )
        rid = str(payload.get("referendum_id") or referendum_id)
        found = bool(payload.get("found"))
        locks = self._optional_mapping(payload.get("locks"), context="locks payload")
        return GovernanceLocksOverview(found=found, referendum_id=rid, locks=locks)

    def get_governance_referendum(self, referendum_id: str) -> GovernanceReferendumStatus:
        """Fetch referendum status via ``GET /v1/gov/referenda/{id}``."""

        payload = self._get_json_object(
            f"/v1/gov/referenda/{referendum_id}",
            context="governance referendum",
        )
        found = bool(payload.get("found"))
        referendum = self._optional_mapping(payload.get("referendum"), context="referendum payload")
        return GovernanceReferendumStatus(found=found, referendum=referendum)

    def get_governance_tally(self, referendum_id: str) -> GovernanceTallySummary:
        """Return the quadratic tally summary for a referendum."""

        payload = self._get_json_object(
            f"/v1/gov/tally/{referendum_id}",
            context="governance tally",
        )
        rid = str(payload.get("referendum_id") or referendum_id)
        approve = self._coerce_int(payload.get("approve"), "tally.approve")
        reject = self._coerce_int(payload.get("reject"), "tally.reject")
        abstain = self._coerce_int(payload.get("abstain"), "tally.abstain")
        return GovernanceTallySummary(
            referendum_id=rid,
            approve=approve,
            reject=reject,
            abstain=abstain,
        )

    def get_governance_unlock_stats(self) -> GovernanceUnlockStats:
        """Return aggregate unlock metrics (operator view)."""

        payload = self._get_json_object("/v1/gov/unlocks/stats", context="unlock stats")
        return GovernanceUnlockStats(
            height_current=self._coerce_int(payload.get("height_current"), "unlock.height_current"),
            expired_locks_now=self._coerce_int(payload.get("expired_locks_now"), "unlock.expired_locks_now"),
            referenda_with_expired=self._coerce_int(
                payload.get("referenda_with_expired"),
                "unlock.referenda_with_expired",
            ),
            last_sweep_height=self._coerce_int(payload.get("last_sweep_height"), "unlock.last_sweep_height"),
        )

    def get_council_current(self) -> CouncilCurrentStatus:
        """Return the latest council roster."""

        payload = self._get_json_object("/v1/gov/council/current", context="council current")
        epoch = self._coerce_int(payload.get("epoch"), "council_current.epoch")
        members_value = payload.get("members", [])
        members = self._parse_council_members(members_value)
        return CouncilCurrentStatus(epoch=epoch, members=members)

    def get_council_audit(self, *, epoch: Optional[int] = None) -> CouncilAuditMetadata:
        """Expose seed/beacon metadata for the council derivation process."""

        params = {"epoch": epoch} if epoch is not None else None
        payload = self._get_json_object(
            "/v1/gov/council/audit",
            params=params,
            context="council audit",
        )
        epoch_value = epoch if epoch is not None else payload.get("epoch")
        return CouncilAuditMetadata(
            epoch=self._coerce_int(epoch_value, "council_audit.epoch"),
            seed_hex=str(payload.get("seed_hex", "")),
            beacon_hex=str(payload.get("beacon_hex", "")),
            chain_id=str(payload.get("chain_id", "")),
            members_count=self._coerce_int(payload.get("members_count"), "council_audit.members_count"),
            candidate_count=self._coerce_int(payload.get("candidate_count"), "council_audit.candidate_count"),
        )

    def propose_contract_deploy(
        self,
        *,
        contract_id: str,
        namespace: str,
        abi_version: str,
        code_hash: str,
        abi_hash: str,
        window: Optional[Tuple[int, int]] = None,
        mode: Optional[str] = None,
        limits: Optional[Mapping[str, Any]] = None,
    ) -> GovernanceProposalDraft:
        """Draft a deploy-contract proposal via ``POST /v1/gov/proposals/deploy-contract``."""

        payload: Dict[str, Any] = {
            "contract_id": contract_id,
            "namespace": namespace,
            "abi_version": abi_version,
            "code_hash": code_hash,
            "abi_hash": abi_hash,
        }
        if window is not None:
            payload["window"] = {"lower": int(window[0]), "upper": int(window[1])}
        if mode:
            payload["mode"] = mode
        if limits is not None:
            payload["limits"] = dict(limits)
        body = self._post_json(
            "/v1/gov/proposals/deploy-contract",
            payload,
            context="deploy-contract proposal",
        )
        ok = bool(body.get("ok"))
        proposal_id = str(body.get("proposal_id") or "")
        if not proposal_id:
            raise RuntimeError("deploy-contract response missing proposal_id")
        instructions = self._parse_tx_instructions(body.get("tx_instructions"))
        return GovernanceProposalDraft(ok=ok, proposal_id=proposal_id, tx_instructions=instructions)

    def finalize_referendum(
        self,
        *,
        referendum_id: str,
        proposal_id: str,
    ) -> GovernanceInstructionDraft:
        """Draft a FinalizeReferendum transaction via ``POST /v1/gov/finalize``."""

        payload = {"referendum_id": referendum_id, "proposal_id": proposal_id}
        body = self._post_json(
            "/v1/gov/finalize",
            payload,
            context="governance finalize",
            expected_status=(200, 202, 204),
        )
        return GovernanceInstructionDraft(
            ok=bool(body.get("ok")),
            tx_instructions=self._parse_tx_instructions(body.get("tx_instructions")),
        )

    def enact_proposal(
        self,
        *,
        proposal_id: str,
        preimage_hash: Optional[str] = None,
        window: Optional[Tuple[int, int]] = None,
    ) -> GovernanceInstructionDraft:
        """Draft an EnactReferendum transaction via ``POST /v1/gov/enact``."""

        payload: Dict[str, Any] = {"proposal_id": proposal_id}
        if preimage_hash:
            payload["preimage_hash"] = preimage_hash
        if window is not None:
            payload["window"] = {"lower": int(window[0]), "upper": int(window[1])}
        body = self._post_json(
            "/v1/gov/enact",
            payload,
            context="governance enact",
            expected_status=(200, 202, 204),
        )
        return GovernanceInstructionDraft(
            ok=bool(body.get("ok")),
            tx_instructions=self._parse_tx_instructions(body.get("tx_instructions")),
        )

    def submit_plain_ballot(
        self,
        *,
        authority: str,
        chain_id: str,
        referendum_id: str,
        owner: str,
        amount: Union[str, int],
        duration_blocks: int,
        direction: str,
        public: Optional[Mapping[str, Any]] = None,
    ) -> BallotSubmitResult:
        """Submit a quadratic ballot via ``POST /v1/gov/ballots/plain``."""

        payload: Dict[str, Any] = {
            "authority": authority,
            "chain_id": chain_id,
            "referendum_id": referendum_id,
            "owner": owner,
            "amount": self._stringify_amount(amount),
            "duration_blocks": int(duration_blocks),
            "direction": direction,
        }
        if public is not None:
            payload["public"] = public
        body = self._post_json(
            "/v1/gov/ballots/plain",
            payload,
            context="plain ballot",
        )
        return BallotSubmitResult(
            ok=bool(body.get("ok")),
            accepted=bool(body.get("accepted")),
            reason=body.get("reason"),
            tx_instructions=self._parse_tx_instructions(body.get("tx_instructions")),
        )

    def submit_zk_ballot(
        self,
        *,
        authority: str,
        chain_id: str,
        election_id: str,
        proof_b64: str,
        public: Optional[Mapping[str, Any]] = None,
    ) -> BallotSubmitResult:
        """Submit a zk ballot via ``POST /v1/gov/ballots/zk``."""

        payload: Dict[str, Any] = {
            "authority": authority,
            "chain_id": chain_id,
            "election_id": election_id,
            "proof_b64": proof_b64,
        }
        public_inputs = self._normalize_governance_zk_public_inputs(
            public,
            context="zk ballot public inputs",
        )
        if public_inputs is not None:
            payload["public"] = public_inputs
        body = self._post_json(
            "/v1/gov/ballots/zk",
            payload,
            context="zk ballot",
        )
        return BallotSubmitResult(
            ok=bool(body.get("ok")),
            accepted=bool(body.get("accepted")),
            reason=body.get("reason"),
            tx_instructions=self._parse_tx_instructions(body.get("tx_instructions")),
        )

    def submit_zk_ballot_v1(
        self,
        *,
        authority: str,
        chain_id: str,
        election_id: str,
        backend: str,
        envelope_b64: str,
        root_hint: Optional[str] = None,
        owner: Optional[str] = None,
        amount: Optional[str] = None,
        duration_blocks: Optional[int] = None,
        direction: Optional[str] = None,
        nullifier: Optional[str] = None,
    ) -> BallotSubmitResult:
        """Submit a BallotProof-style payload via ``POST /v1/gov/ballots/zk-v1``.

        Optional hints mirror BallotProof fields: root_hint, owner, amount,
        duration_blocks, direction, and nullifier.
        """

        payload: Dict[str, Any] = {
            "authority": authority,
            "chain_id": chain_id,
            "election_id": election_id,
            "backend": backend,
            "envelope_b64": envelope_b64,
        }
        self._ensure_governance_lock_hints_complete(
            owner,
            amount,
            duration_blocks,
            context="zk ballot v1",
        )
        self._ensure_governance_owner_canonical(owner, context="zk ballot v1")
        if root_hint is not None:
            payload["root_hint"] = root_hint
        if owner is not None:
            payload["owner"] = owner
        if amount is not None:
            payload["amount"] = amount
        if duration_blocks is not None:
            payload["duration_blocks"] = duration_blocks
        if direction:
            payload["direction"] = direction
        if nullifier is not None:
            payload["nullifier"] = nullifier
        self._normalize_governance_public_hex_hint(
            payload,
            "root_hint",
            context="zk ballot v1",
        )
        self._normalize_governance_public_hex_hint(
            payload,
            "nullifier",
            context="zk ballot v1",
        )
        body = self._post_json(
            "/v1/gov/ballots/zk-v1",
            payload,
            context="zk ballot v1",
        )
        return BallotSubmitResult(
            ok=bool(body.get("ok")),
            accepted=bool(body.get("accepted")),
            reason=body.get("reason"),
            tx_instructions=self._parse_tx_instructions(body.get("tx_instructions")),
        )

    def apply_protected_namespaces(
        self,
        namespaces: Sequence[str],
    ) -> ProtectedNamespacesApplyResult:
        """Apply the `gov_protected_namespaces` parameter."""

        cleaned = [ns.strip() for ns in namespaces if ns and ns.strip()]
        body = self._post_json(
            "/v1/gov/protected-namespaces",
            {"namespaces": cleaned},
            context="protected namespaces apply",
        )
        return ProtectedNamespacesApplyResult(
            ok=bool(body.get("ok")),
            applied=self._coerce_int(body.get("applied"), "protected.applied"),
        )

    def get_protected_namespaces(self) -> ProtectedNamespacesStatus:
        """Fetch the current protected namespace setting."""

        body = self._get_json_object(
            "/v1/gov/protected-namespaces",
            context="protected namespaces",
        )
        raw_namespaces = body.get("namespaces", [])
        if not isinstance(raw_namespaces, list):
            raise RuntimeError("protected namespaces payload must include a list")
        namespaces: List[str] = []
        for entry in raw_namespaces:
            if isinstance(entry, str):
                namespaces.append(entry)
        found = bool(body.get("found"))
        return ProtectedNamespacesStatus(found=found, namespaces=namespaces)

    def persist_council(
        self,
        *,
        committee_size: int,
        candidates: Sequence[VrfCandidate],
        epoch: Optional[int] = None,
        authority: Optional[str] = None,
        private_key: Optional[str] = None,
    ) -> CouncilPersistResult:
        """Persist a VRF-derived council via ``POST /v1/gov/council/persist``."""

        payload: Dict[str, Any] = {
            "committee_size": int(committee_size),
            "candidates": [candidate.to_payload() for candidate in candidates],
        }
        if epoch is not None:
            payload["epoch"] = int(epoch)
        if authority:
            payload["authority"] = authority
        if private_key:
            payload["private_key"] = private_key
        body = self._post_json(
            "/v1/gov/council/persist",
            payload,
            context="council persist",
        )
        return CouncilPersistResult(
            epoch=self._coerce_int(body.get("epoch"), "council.epoch"),
            members=self._parse_council_members(body.get("members", [])),
            total_candidates=self._coerce_int(body.get("total_candidates"), "council.total_candidates"),
            verified=self._coerce_int(body.get("verified"), "council.verified"),
        )

    # ------------------------------------------------------------------
    # Trigger registry helpers
    # ------------------------------------------------------------------
    def list_triggers(
        self,
        *,
        namespace: Optional[str] = None,
        authority: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> TriggerListPage:
        """List triggers via ``GET /v1/triggers`` applying optional filters."""

        params: Dict[str, Any] = {}
        namespace_value = self._normalize_optional_string(namespace, "triggers.namespace")
        if namespace_value is not None:
            params["namespace"] = namespace_value
        authority_value = self._normalize_optional_string(authority, "triggers.authority")
        if authority_value is not None:
            params["authority"] = authority_value
        limit_value = self._normalize_optional_int(limit, "triggers.limit")
        if limit_value is not None:
            params["limit"] = limit_value
        offset_value = self._normalize_optional_int(offset, "triggers.offset", allow_zero=True)
        if offset_value is not None:
            params["offset"] = offset_value
        payload = self._get_json_object(
            "/v1/triggers",
            params=params or None,
            context="trigger listing",
        )
        return TriggerListPage.from_payload(payload)

    def get_trigger(self, trigger_id: str) -> Optional[TriggerRecord]:
        """Fetch a trigger definition, returning ``None`` when missing."""

        normalized_id = self._require_non_empty_string(trigger_id, "trigger_id")
        response = self._request("GET", f"/v1/triggers/{normalized_id}")
        self._expect_status(response, {200, 404})
        if response.status_code == 404 or not response.content:
            return None
        payload = self._ensure_mapping(response.json(), "trigger lookup")
        return TriggerRecord.from_payload(payload)

    def register_trigger(self, trigger: Mapping[str, Any]) -> Mapping[str, Any]:
        """Register or update a trigger via ``POST /v1/triggers``."""

        if not isinstance(trigger, Mapping):
            raise TypeError("trigger must be a mapping")
        return self._post_json(
            "/v1/triggers",
            dict(trigger),
            context="trigger registration",
            expected_status=(200, 201, 202),
        )

    def delete_trigger(self, trigger_id: str) -> bool:
        """Delete a trigger by id, returning ``False`` when already absent."""

        response = self._request("DELETE", f"/v1/triggers/{trigger_id}")
        self._expect_status(response, {200, 202, 204, 404})
        return response.status_code != 404

    def query_triggers(
        self,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
    ) -> TriggerListPage:
        """Query triggers via ``POST /v1/triggers/query``."""

        payload: Dict[str, Any] = {}
        if filter is not None:
            if not isinstance(filter, Mapping):
                raise TypeError("triggers.query.filter must be a mapping")
            payload["filter"] = dict(filter)
        if sort is not None:
            payload["sort"] = sort
        limit_value = self._normalize_optional_int(limit, "triggers.query.limit")
        if limit_value is not None:
            payload["limit"] = limit_value
        offset_value = self._normalize_optional_int(
            offset,
            "triggers.query.offset",
            allow_zero=True,
        )
        if offset_value is not None:
            payload["offset"] = offset_value
        fetch_size_value = self._normalize_optional_int(
            fetch_size,
            "triggers.query.fetch_size",
        )
        if fetch_size_value is not None:
            payload["fetch_size"] = fetch_size_value
        query_name_value = self._normalize_optional_string(
            query_name,
            "triggers.query.query_name",
        )
        if query_name_value is not None:
            payload["query_name"] = query_name_value
        response = self._post_json(
            "/v1/triggers/query",
            payload,
            context="trigger query",
        )
        return TriggerListPage.from_payload(response)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------
    def _request(
        self,
        method: str,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[MutableMapping[str, str]] = None,
        data: Optional[bytes] = None,
    ) -> requests.Response:
        url = f"{self._base_url}{path}"
        response = self._session.request(method, url, params=params, headers=headers, data=data)
        return response

    @staticmethod
    def _expect_status(response: requests.Response, expected: Iterable[int]) -> None:
        expected_set = set(expected)
        if response.status_code in expected_set:
            return
        message = response.text
        raise RuntimeError(
            f"unexpected status {response.status_code}; expected {sorted(expected_set)}; body={message}"
        )

    @staticmethod
    def _encode_prover_filters(filters: Mapping[str, Any]) -> Dict[str, Any]:
        if not filters:
            return {}
        params: Dict[str, Any] = {}
        for key, value in filters.items():
            if value in (None, False):
                continue
            if value is True:
                params[_FILTER_MAPPING.get(key, key)] = "true"
            else:
                params[_FILTER_MAPPING.get(key, key)] = value
        return params

    @staticmethod
    def _clean_params(params: Optional[Mapping[str, Any]]) -> Optional[Dict[str, Any]]:
        if not params:
            return None
        cleaned: Dict[str, Any] = {}
        for key, value in params.items():
            if value is None:
                continue
            cleaned[key] = value
        return cleaned or None

    @staticmethod
    def _require_non_empty_string(value: Any, context: str) -> str:
        if not isinstance(value, str):
            raise TypeError(f"{context} must be a string")
        trimmed = value.strip()
        if not trimmed:
            raise ValueError(f"{context} must be a non-empty string")
        return trimmed

    @classmethod
    def _normalize_optional_string(cls, value: Any, context: str) -> Optional[str]:
        if value is None:
            return None
        return cls._require_non_empty_string(value, context)

    @classmethod
    def _normalize_optional_int(
        cls, value: Any, context: str, *, allow_zero: bool = False
    ) -> Optional[int]:
        if value is None:
            return None
        number = cls._coerce_int(value, context)
        if number < 0 or (number == 0 and not allow_zero):
            raise ValueError(
                f"{context} must be {'non-negative' if allow_zero else 'positive'}"
            )
        return number

    @staticmethod
    def _normalize_numeric_literal(
        value: Any, context: str, *, allow_negative: bool = False
    ) -> str:
        if isinstance(value, bool):
            raise TypeError(f"{context} must be a numeric literal")
        if isinstance(value, str):
            raw = value.strip()
        elif isinstance(value, (int, float)):
            if isinstance(value, float) and not math.isfinite(value):
                raise ValueError(f"{context} must be a finite number")
            raw = str(value)
        else:
            raise TypeError(f"{context} must be a numeric literal")
        if not raw:
            raise ValueError(f"{context} must be a numeric literal")
        sign = raw[0]
        if sign in ("-", "+"):
            if sign == "-" and not allow_negative:
                raise ValueError(f"{context} must be non-negative")
            digits = raw[1:]
        else:
            digits = raw
        if not digits:
            raise ValueError(f"{context} must be a numeric literal")
        seen_dot = False
        seen_digit = False
        for ch in digits:
            if ch == ".":
                if seen_dot:
                    raise ValueError(f"{context} must be a numeric literal")
                seen_dot = True
                continue
            if not ch.isdigit():
                raise ValueError(f"{context} must be a numeric literal")
            seen_digit = True
        if not seen_digit:
            raise ValueError(f"{context} must be a numeric literal")
        return raw

    @staticmethod
    def _normalize_subscription_status(value: Any, context: str) -> str:
        if not isinstance(value, str):
            raise TypeError(f"{context} must be a string")
        normalized = value.strip().lower()
        if not normalized:
            raise ValueError(f"{context} must be a non-empty string")
        if normalized not in _SUBSCRIPTION_STATUSES:
            raise ValueError(f"{context} must be one of {sorted(_SUBSCRIPTION_STATUSES)}")
        return normalized

    def _post_json(
        self,
        path: str,
        payload: Mapping[str, Any],
        *,
        context: str,
        expected_status: Iterable[int] = (200,),
    ) -> Mapping[str, Any]:
        headers = {"Content-Type": "application/json"}
        data = json.dumps(payload).encode("utf-8")
        response = self._request("POST", path, headers=headers, data=data)
        self._expect_status(response, expected_status)
        if response.status_code == 204:
            return {}
        body = response.json()
        return self._ensure_mapping(body, context)

    @staticmethod
    def _maybe_json(response: requests.Response) -> Optional[Any]:
        if not response.content:
            return None
        try:
            return response.json()
        except ValueError as exc:
            raise RuntimeError("response payload was not valid JSON") from exc

    def _get_json_object(
        self,
        path: str,
        *,
        context: str,
        params: Optional[Mapping[str, Any]] = None,
    ) -> Mapping[str, Any]:
        response = self._request("GET", path, params=params)
        self._expect_status(response, {200})
        payload = response.json()
        return self._ensure_mapping(payload, context)

    @staticmethod
    def _ensure_mapping(payload: Any, context: str) -> Mapping[str, Any]:
        if isinstance(payload, Mapping):
            return payload
        raise RuntimeError(f"{context} response must be a JSON object")

    @staticmethod
    def _optional_mapping(
        value: Any,
        *,
        context: str,
    ) -> Optional[Dict[str, Any]]:
        if value is None:
            return None
        if isinstance(value, Mapping):
            return dict(value)
        raise RuntimeError(f"{context} must be a JSON object when present")

    @staticmethod
    def _clone_json_payload(value: Mapping[str, Any], *, context: str) -> Dict[str, Any]:
        if not isinstance(value, Mapping):
            raise RuntimeError(f"{context} must be a JSON object")
        try:
            encoded = json.dumps(value)
        except (TypeError, ValueError) as exc:
            raise RuntimeError(f"{context} must be JSON-serialisable") from exc
        return json.loads(encoded)

    @staticmethod
    def _coerce_int(value: Any, context: str) -> int:
        if isinstance(value, bool):
            return int(value)
        if isinstance(value, (int, float)):
            return int(value)
        if isinstance(value, str):
            stripped = value.strip()
            if not stripped:
                return 0
            return int(stripped, 10)
        if value is None:
            return 0
        raise RuntimeError(f"{context} must be numeric")

    @staticmethod
    def _coerce_optional_int(value: Any, context: str) -> Optional[int]:
        if value is None:
            return None
        if isinstance(value, bool):
            return int(value)
        if isinstance(value, (int, float)):
            return int(value)
        if isinstance(value, str):
            stripped = value.strip()
            if not stripped:
                return None
            return int(stripped, 10)
        raise RuntimeError(f"{context} must be numeric when provided")

    @staticmethod
    def _parse_council_members(value: Any) -> List[CouncilMember]:
        if not isinstance(value, list):
            raise RuntimeError("council members payload must be a list")
        members: List[CouncilMember] = []
        for entry in value:
            if not isinstance(entry, Mapping):
                raise RuntimeError("council member entry must be an object")
            account_id = entry.get("account_id")
            if not isinstance(account_id, str) or not account_id:
                raise RuntimeError("council member missing account_id")
            members.append(CouncilMember(account_id=account_id))
        return members

    @staticmethod
    def _parse_tx_instructions(value: Any) -> List[TransactionInstruction]:
        if value is None:
            return []
        if not isinstance(value, list):
            raise RuntimeError("tx_instructions must be a list when present")
        instructions: List[TransactionInstruction] = []
        for entry in value:
            if not isinstance(entry, Mapping):
                raise RuntimeError("tx instruction entry must be an object")
            wire_id = entry.get("wire_id")
            payload_hex = entry.get("payload_hex")
            if not isinstance(wire_id, str) or not isinstance(payload_hex, str):
                raise RuntimeError("tx instruction missing wire_id or payload_hex")
            instructions.append(TransactionInstruction(wire_id=wire_id, payload_hex=payload_hex))
        return instructions

    @staticmethod
    def _parse_explorer_account_qr(
        value: Mapping[str, Any],
        *,
        context: str,
    ) -> ExplorerAccountQr:
        record = ToriiClient._ensure_mapping(value, context)
        canonical = ToriiClient._require_non_empty_string(
            record.get("canonical_id", record.get("canonicalId")),
            f"{context}.canonical_id",
        )
        literal = ToriiClient._require_non_empty_string(
            record.get("literal"),
            f"{context}.literal",
        )
        network_prefix = ToriiClient._coerce_int(
            record.get("network_prefix", record.get("networkPrefix")),
            f"{context}.network_prefix",
        )
        error_correction = ToriiClient._require_non_empty_string(
            record.get("error_correction", record.get("errorCorrection")),
            f"{context}.error_correction",
        )
        modules = ToriiClient._coerce_int(record.get("modules"), f"{context}.modules")
        qr_version = ToriiClient._coerce_int(
            record.get("qr_version", record.get("qrVersion")),
            f"{context}.qr_version",
        )
        svg = ToriiClient._require_non_empty_string(
            record.get("svg"),
            f"{context}.svg",
        )
        return ExplorerAccountQr(
            canonical_id=canonical,
            literal=literal,
            network_prefix=network_prefix,
            error_correction=error_correction,
            modules=modules,
            qr_version=qr_version,
            svg=svg,
        )

    def _parse_status_payload(
        self,
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> StatusPayload:
        record = self._ensure_mapping(payload, context)
        governance = self._parse_status_governance(record.get("governance"))
        lane_commitments = self._parse_lane_commitments(
            record.get("lane_commitments"),
            context=f"{context}.lane_commitments",
        )
        dataspace_commitments = self._parse_dataspace_commitments(
            record.get("dataspace_commitments"),
            context=f"{context}.dataspace_commitments",
        )
        lane_governance = self._parse_lane_governance(
            record.get("lane_governance"),
            context=f"{context}.lane_governance",
        )
        sealed_aliases = self._parse_string_array(
            record.get("lane_governance_sealed_aliases"),
            context=f"{context}.lane_governance_sealed_aliases",
        )
        consensus_caps = self._parse_consensus_caps(record.get("consensus_caps"))
        raw = dict(record)
        return StatusPayload(
            mode_tag=None if record.get("mode_tag") is None else str(record.get("mode_tag")),
            staged_mode_tag=None if record.get("staged_mode_tag") is None else str(record.get("staged_mode_tag")),
            staged_mode_activation_height=self._coerce_optional_int(
                record.get("staged_mode_activation_height"),
                f"{context}.staged_mode_activation_height",
            ),
            mode_activation_lag_blocks=self._coerce_optional_int(
                record.get("mode_activation_lag_blocks"),
                f"{context}.mode_activation_lag_blocks",
            ),
            consensus_caps=consensus_caps,
            peers=self._coerce_int(record.get("peers"), f"{context}.peers"),
            queue_size=self._coerce_int(record.get("queue_size"), f"{context}.queue_size"),
            commit_time_ms=self._coerce_int(record.get("commit_time_ms"), f"{context}.commit_time_ms"),
            da_reschedule_total=self._coerce_int(
                record.get("da_reschedule_total"),
                f"{context}.da_reschedule_total",
            ),
            txs_approved=self._coerce_int(record.get("txs_approved"), f"{context}.txs_approved"),
            txs_rejected=self._coerce_int(record.get("txs_rejected"), f"{context}.txs_rejected"),
            view_changes=self._coerce_int(record.get("view_changes"), f"{context}.view_changes"),
            governance=governance,
            lane_commitments=lane_commitments,
            dataspace_commitments=dataspace_commitments,
            lane_governance=lane_governance,
            lane_governance_sealed_total=self._coerce_int(
                record.get("lane_governance_sealed_total"),
                f"{context}.lane_governance_sealed_total",
            ),
            lane_governance_sealed_aliases=sealed_aliases,
            raw=raw,
        )

    def _parse_consensus_caps(self, value: Any) -> Optional[SumeragiConsensusCaps]:
        if value is None:
            return None
        record = self._ensure_mapping(value, "status.consensus_caps")
        return SumeragiConsensusCaps(
            collectors_k=self._coerce_int(record.get("collectors_k"), "status.consensus_caps.collectors_k"),
            redundant_send_r=self._coerce_int(record.get("redundant_send_r"), "status.consensus_caps.redundant_send_r"),
            da_enabled=self._coerce_bool(record.get("da_enabled"), "status.consensus_caps.da_enabled"),
            rbc_chunk_max_bytes=self._coerce_int(record.get("rbc_chunk_max_bytes"), "status.consensus_caps.rbc_chunk_max_bytes"),
            rbc_session_ttl_ms=self._coerce_int(record.get("rbc_session_ttl_ms"), "status.consensus_caps.rbc_session_ttl_ms"),
            rbc_store_max_sessions=self._coerce_int(record.get("rbc_store_max_sessions"), "status.consensus_caps.rbc_store_max_sessions"),
            rbc_store_soft_sessions=self._coerce_int(record.get("rbc_store_soft_sessions"), "status.consensus_caps.rbc_store_soft_sessions"),
            rbc_store_max_bytes=self._coerce_int(record.get("rbc_store_max_bytes"), "status.consensus_caps.rbc_store_max_bytes"),
            rbc_store_soft_bytes=self._coerce_int(record.get("rbc_store_soft_bytes"), "status.consensus_caps.rbc_store_soft_bytes"),
        )

    def _parse_status_governance(
        self,
        value: Any,
    ) -> Optional[GovernanceStatusSnapshot]:
        if value is None:
            return None
        record = self._ensure_mapping(value, "governance payload")
        proposals = self._ensure_mapping(record.get("proposals"), "governance.proposals")
        protected_namespace = self._ensure_mapping(
            record.get("protected_namespace"),
            "governance.protected_namespace",
        )
        manifest_admission = self._ensure_mapping(
            record.get("manifest_admission"),
            "governance.manifest_admission",
        )
        manifest_quorum = self._ensure_mapping(
            record.get("manifest_quorum"),
            "governance.manifest_quorum",
        )
        activations = self._parse_governance_activations(record.get("recent_manifest_activations"))
        return GovernanceStatusSnapshot(
            proposals=GovernanceProposalCounters(
                proposed=self._coerce_int(proposals.get("proposed"), "governance.proposals.proposed"),
                approved=self._coerce_int(proposals.get("approved"), "governance.proposals.approved"),
                rejected=self._coerce_int(proposals.get("rejected"), "governance.proposals.rejected"),
                enacted=self._coerce_int(proposals.get("enacted"), "governance.proposals.enacted"),
            ),
            protected_namespace=GovernanceProtectedNamespaceStats(
                total_checks=self._coerce_int(
                    protected_namespace.get("total_checks"),
                    "governance.protected_namespace.total_checks",
                ),
                allowed=self._coerce_int(
                    protected_namespace.get("allowed"),
                    "governance.protected_namespace.allowed",
                ),
                rejected=self._coerce_int(
                    protected_namespace.get("rejected"),
                    "governance.protected_namespace.rejected",
                ),
            ),
            manifest_admission=GovernanceManifestAdmissionStats(
                total_checks=self._coerce_int(
                    manifest_admission.get("total_checks"),
                    "governance.manifest_admission.total_checks",
                ),
                allowed=self._coerce_int(
                    manifest_admission.get("allowed"),
                    "governance.manifest_admission.allowed",
                ),
                missing_manifest=self._coerce_int(
                    manifest_admission.get("missing_manifest"),
                    "governance.manifest_admission.missing_manifest",
                ),
                non_validator_authority=self._coerce_int(
                    manifest_admission.get("non_validator_authority"),
                    "governance.manifest_admission.non_validator_authority",
                ),
                quorum_rejected=self._coerce_int(
                    manifest_admission.get("quorum_rejected"),
                    "governance.manifest_admission.quorum_rejected",
                ),
                protected_namespace_rejected=self._coerce_int(
                    manifest_admission.get("protected_namespace_rejected"),
                    "governance.manifest_admission.protected_namespace_rejected",
                ),
                runtime_hook_rejected=self._coerce_int(
                    manifest_admission.get("runtime_hook_rejected"),
                    "governance.manifest_admission.runtime_hook_rejected",
                ),
            ),
            manifest_quorum=GovernanceManifestQuorumStats(
                total_checks=self._coerce_int(
                    manifest_quorum.get("total_checks"),
                    "governance.manifest_quorum.total_checks",
                ),
                satisfied=self._coerce_int(
                    manifest_quorum.get("satisfied"),
                    "governance.manifest_quorum.satisfied",
                ),
                rejected=self._coerce_int(
                    manifest_quorum.get("rejected"),
                    "governance.manifest_quorum.rejected",
                ),
            ),
            recent_manifest_activations=activations,
        )

    def _parse_governance_activations(
        self,
        payload: Any,
    ) -> List[GovernanceManifestActivation]:
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise RuntimeError("governance.recent_manifest_activations must be a list")
        activations: List[GovernanceManifestActivation] = []
        for index, entry in enumerate(payload):
            record = self._ensure_mapping(entry, f"governance.recent_manifest_activations[{index}]")
            namespace = "" if record.get("namespace") is None else str(record.get("namespace"))
            contract_id = "" if record.get("contract_id") is None else str(record.get("contract_id"))
            code_hash = "" if record.get("code_hash_hex") is None else str(record.get("code_hash_hex"))
            abi_hash = (
                None
                if record.get("abi_hash_hex") is None
                else str(record.get("abi_hash_hex"))
            )
            activations.append(
                GovernanceManifestActivation(
                    namespace=namespace,
                    contract_id=contract_id,
                    code_hash_hex=code_hash,
                    abi_hash_hex=abi_hash,
                    height=self._coerce_int(
                        record.get("height"),
                        f"governance.recent_manifest_activations[{index}].height",
                    ),
                    activated_at_ms=self._coerce_int(
                        record.get("activated_at_ms"),
                        f"governance.recent_manifest_activations[{index}].activated_at_ms",
                    ),
                )
            )
        return activations

    def _parse_lane_commitments(
        self,
        payload: Any,
        *,
        context: str,
    ) -> List[LaneCommitmentSnapshot]:
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise RuntimeError(f"{context} must be a list")
        snapshots: List[LaneCommitmentSnapshot] = []
        for index, entry in enumerate(payload):
            record = self._ensure_mapping(entry, f"{context}[{index}]")
            snapshots.append(
                LaneCommitmentSnapshot(
                    block_height=self._coerce_int(record.get("block_height"), f"{context}[{index}].block_height"),
                    lane_id=self._coerce_int(record.get("lane_id"), f"{context}[{index}].lane_id"),
                    tx_count=self._coerce_int(record.get("tx_count"), f"{context}[{index}].tx_count"),
                    total_chunks=self._coerce_int(record.get("total_chunks"), f"{context}[{index}].total_chunks"),
                    rbc_bytes_total=self._coerce_int(
                        record.get("rbc_bytes_total"),
                        f"{context}[{index}].rbc_bytes_total",
                    ),
                    teu_total=self._coerce_int(record.get("teu_total"), f"{context}[{index}].teu_total"),
                    block_hash="" if record.get("block_hash") is None else str(record.get("block_hash")),
                )
            )
        return snapshots

    def _parse_dataspace_commitments(
        self,
        payload: Any,
        *,
        context: str,
    ) -> List[DataspaceCommitmentSnapshot]:
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise RuntimeError(f"{context} must be a list")
        snapshots: List[DataspaceCommitmentSnapshot] = []
        for index, entry in enumerate(payload):
            record = self._ensure_mapping(entry, f"{context}[{index}]")
            snapshots.append(
                DataspaceCommitmentSnapshot(
                    block_height=self._coerce_int(record.get("block_height"), f"{context}[{index}].block_height"),
                    lane_id=self._coerce_int(record.get("lane_id"), f"{context}[{index}].lane_id"),
                    dataspace_id=self._coerce_int(record.get("dataspace_id"), f"{context}[{index}].dataspace_id"),
                    tx_count=self._coerce_int(record.get("tx_count"), f"{context}[{index}].tx_count"),
                    total_chunks=self._coerce_int(record.get("total_chunks"), f"{context}[{index}].total_chunks"),
                    rbc_bytes_total=self._coerce_int(
                        record.get("rbc_bytes_total"),
                        f"{context}[{index}].rbc_bytes_total",
                    ),
                    teu_total=self._coerce_int(record.get("teu_total"), f"{context}[{index}].teu_total"),
                    block_hash="" if record.get("block_hash") is None else str(record.get("block_hash")),
                )
            )
        return snapshots

    @staticmethod
    def _parse_uaid_portfolio_response(payload: Mapping[str, Any], *, context: str) -> UaidPortfolioResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        uaid_literal = ToriiClient._normalize_uaid_literal(record.get("uaid"), context=f"{context}.uaid")
        totals_record = ToriiClient._ensure_mapping(record.get("totals") or {}, context=f"{context}.totals")
        totals = UaidPortfolioTotals(
            accounts=ToriiClient._coerce_unsigned(
                totals_record.get("accounts", 0),
                f"{context}.totals.accounts",
            ),
            positions=ToriiClient._coerce_unsigned(
                totals_record.get("positions", 0),
                f"{context}.totals.positions",
            ),
        )
        dataspaces_value = record.get("dataspaces") or []
        if not isinstance(dataspaces_value, list):
            raise RuntimeError(f"{context}.dataspaces must be a list")
        dataspaces = [
            ToriiClient._parse_uaid_portfolio_dataspace(entry, context=f"{context}.dataspaces[{index}]")
            for index, entry in enumerate(dataspaces_value)
        ]
        return UaidPortfolioResponse(uaid=uaid_literal, totals=totals, dataspaces=dataspaces)

    @staticmethod
    def _parse_uaid_portfolio_dataspace(value: Any, *, context: str) -> UaidPortfolioDataspace:
        record = ToriiClient._ensure_mapping(value, context)
        accounts_value = record.get("accounts") or []
        if not isinstance(accounts_value, list):
            raise RuntimeError(f"{context}.accounts must be a list")
        accounts = [
            ToriiClient._parse_uaid_portfolio_account(entry, context=f"{context}.accounts[{index}]")
            for index, entry in enumerate(accounts_value)
        ]
        return UaidPortfolioDataspace(
            dataspace_id=ToriiClient._coerce_unsigned(record.get("dataspace_id"), f"{context}.dataspace_id"),
            dataspace_alias=ToriiClient._coerce_optional_string(
                record.get("dataspace_alias"),
                context=f"{context}.dataspace_alias",
            ),
            accounts=accounts,
        )

    @staticmethod
    def _parse_uaid_portfolio_account(value: Any, *, context: str) -> UaidPortfolioAccount:
        record = ToriiClient._ensure_mapping(value, context)
        assets_value = record.get("assets") or []
        if not isinstance(assets_value, list):
            raise RuntimeError(f"{context}.assets must be a list")
        assets = [
            ToriiClient._parse_uaid_portfolio_asset(entry, context=f"{context}.assets[{index}]")
            for index, entry in enumerate(assets_value)
        ]
        return UaidPortfolioAccount(
            account_id=ToriiClient._require_string(record.get("account_id"), f"{context}.account_id"),
            label=ToriiClient._coerce_optional_string(record.get("label"), context=f"{context}.label"),
            assets=assets,
        )

    @staticmethod
    def _parse_uaid_portfolio_asset(value: Any, *, context: str) -> UaidPortfolioAsset:
        record = ToriiClient._ensure_mapping(value, context)
        return UaidPortfolioAsset(
            asset_id=ToriiClient._require_string(record.get("asset_id"), f"{context}.asset_id"),
            asset_definition_id=ToriiClient._require_string(
                record.get("asset_definition_id"),
                f"{context}.asset_definition_id",
            ),
            quantity=ToriiClient._require_string(record.get("quantity"), f"{context}.quantity"),
        )

    @staticmethod
    def _parse_uaid_bindings_response(payload: Mapping[str, Any], *, context: str) -> UaidBindingsResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        uaid_literal = ToriiClient._normalize_uaid_literal(record.get("uaid"), context=f"{context}.uaid")
        dataspaces_value = record.get("dataspaces") or []
        if not isinstance(dataspaces_value, list):
            raise RuntimeError(f"{context}.dataspaces must be a list")
        dataspaces = [
            ToriiClient._parse_uaid_bindings_dataspace(entry, context=f"{context}.dataspaces[{index}]")
            for index, entry in enumerate(dataspaces_value)
        ]
        return UaidBindingsResponse(uaid=uaid_literal, dataspaces=dataspaces)

    @staticmethod
    def _parse_uaid_bindings_dataspace(value: Any, *, context: str) -> UaidBindingsDataspace:
        record = ToriiClient._ensure_mapping(value, context)
        return UaidBindingsDataspace(
            dataspace_id=ToriiClient._coerce_unsigned(record.get("dataspace_id"), f"{context}.dataspace_id"),
            dataspace_alias=ToriiClient._coerce_optional_string(
                record.get("dataspace_alias"),
                context=f"{context}.dataspace_alias",
            ),
            accounts=ToriiClient._parse_string_list(record.get("accounts"), context=f"{context}.accounts"),
        )

    @staticmethod
    def _parse_uaid_manifests_response(payload: Mapping[str, Any], *, context: str) -> UaidManifestsResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        uaid_literal = ToriiClient._normalize_uaid_literal(record.get("uaid"), context=f"{context}.uaid")
        manifests_value = record.get("manifests") or []
        if not isinstance(manifests_value, list):
            raise RuntimeError(f"{context}.manifests must be a list")
        manifests = [
            ToriiClient._parse_uaid_manifest_record(entry, context=f"{context}.manifests[{index}]")
            for index, entry in enumerate(manifests_value)
        ]
        return UaidManifestsResponse(uaid=uaid_literal, manifests=manifests)

    @staticmethod
    def _parse_uaid_manifest_record(value: Any, *, context: str) -> UaidManifestRecord:
        record = ToriiClient._ensure_mapping(value, context)
        status = ToriiClient._require_string(record.get("status"), f"{context}.status")
        if status not in UAID_MANIFEST_STATUS_VALUES:
            allowed = ", ".join(sorted(UAID_MANIFEST_STATUS_VALUES))
            raise RuntimeError(f"{context}.status must be one of {allowed}")
        lifecycle = ToriiClient._parse_uaid_manifest_lifecycle(record.get("lifecycle"), context=f"{context}.lifecycle")
        manifest = ToriiClient._parse_uaid_manifest(record.get("manifest"), context=f"{context}.manifest")
        manifest_hash_value = ToriiClient._require_string(
            record.get("manifest_hash"),
            f"{context}.manifest_hash",
        )
        manifest_hash = ToriiClient._normalize_hex_string(
            manifest_hash_value,
            context=f"{context}.manifest_hash",
            expected_length=64,
        )
        return UaidManifestRecord(
            dataspace_id=ToriiClient._coerce_unsigned(record.get("dataspace_id"), f"{context}.dataspace_id"),
            dataspace_alias=ToriiClient._coerce_optional_string(
                record.get("dataspace_alias"),
                context=f"{context}.dataspace_alias",
            ),
            manifest_hash="0x" + manifest_hash,
            status=status,
            lifecycle=lifecycle,
            accounts=ToriiClient._parse_string_list(record.get("accounts"), context=f"{context}.accounts"),
            manifest=manifest,
        )

    @staticmethod
    def _parse_uaid_manifest_lifecycle(value: Any, *, context: str) -> UaidManifestLifecycle:
        record = ToriiClient._ensure_mapping(value or {}, context)
        revocation_value = record.get("revocation")
        revocation = None
        if revocation_value is not None:
            revocation = ToriiClient._parse_uaid_manifest_revocation(revocation_value, context=f"{context}.revocation")
        return UaidManifestLifecycle(
            activated_epoch=ToriiClient._coerce_optional_unsigned(
                record.get("activated_epoch"),
                context=f"{context}.activated_epoch",
            ),
            expired_epoch=ToriiClient._coerce_optional_unsigned(
                record.get("expired_epoch"),
                context=f"{context}.expired_epoch",
            ),
            revocation=revocation,
        )

    @staticmethod
    def _parse_uaid_manifest_revocation(value: Any, *, context: str) -> UaidManifestRevocation:
        record = ToriiClient._ensure_mapping(value, context)
        return UaidManifestRevocation(
            epoch=ToriiClient._coerce_unsigned(record.get("epoch"), f"{context}.epoch"),
            reason=ToriiClient._coerce_optional_string(record.get("reason"), context=f"{context}.reason"),
        )

    @staticmethod
    def _parse_uaid_manifest(value: Any, *, context: str) -> UaidManifest:
        record = ToriiClient._ensure_mapping(value, context)
        entries_value = record.get("entries") or []
        if not isinstance(entries_value, list):
            raise RuntimeError(f"{context}.entries must be a list")
        entries = [
            ToriiClient._parse_uaid_manifest_entry(entry, context=f"{context}.entries[{index}]")
            for index, entry in enumerate(entries_value)
        ]
        return UaidManifest(
            version=ToriiClient._require_string(record.get("version"), f"{context}.version"),
            uaid=ToriiClient._normalize_uaid_literal(record.get("uaid"), context=f"{context}.uaid"),
            dataspace=ToriiClient._coerce_unsigned(record.get("dataspace"), f"{context}.dataspace"),
            issued_ms=ToriiClient._coerce_unsigned(record.get("issued_ms"), f"{context}.issued_ms"),
            activation_epoch=ToriiClient._coerce_unsigned(
                record.get("activation_epoch"),
                f"{context}.activation_epoch",
            ),
            expiry_epoch=ToriiClient._coerce_optional_unsigned(
                record.get("expiry_epoch"),
                context=f"{context}.expiry_epoch",
            ),
            entries=entries,
        )

    @staticmethod
    def _parse_uaid_manifest_entry(value: Any, *, context: str) -> UaidManifestEntry:
        record = ToriiClient._ensure_mapping(value, context)
        scope_value = record.get("scope")
        effect_value = record.get("effect")
        if not isinstance(scope_value, Mapping):
            raise RuntimeError(f"{context}.scope must be an object")
        if not isinstance(effect_value, Mapping):
            raise RuntimeError(f"{context}.effect must be an object")
        notes = ToriiClient._coerce_optional_string(record.get("notes"), context=f"{context}.notes")
        return UaidManifestEntry(scope=dict(scope_value), effect=dict(effect_value), notes=notes)

    def _parse_lane_governance(
        self,
        payload: Any,
        *,
        context: str,
    ) -> List[LaneGovernanceSnapshot]:
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise RuntimeError(f"{context} must be a list")
        snapshots: List[LaneGovernanceSnapshot] = []
        for index, entry in enumerate(payload):
            record = self._ensure_mapping(entry, f"{context}[{index}]")
            runtime_payload = record.get("runtime_upgrade")
            if runtime_payload is None:
                runtime_hook = None
            else:
                runtime_record = self._ensure_mapping(runtime_payload, f"{context}[{index}].runtime_upgrade")
                runtime_hook = LaneRuntimeUpgradeHook(
                    allow=self._coerce_bool(
                        runtime_record.get("allow"),
                        f"{context}[{index}].runtime_upgrade.allow",
                    ),
                    require_metadata=self._coerce_bool(
                        runtime_record.get("require_metadata"),
                        f"{context}[{index}].runtime_upgrade.require_metadata",
                    ),
                    metadata_key=None if runtime_record.get("metadata_key") is None else str(runtime_record.get("metadata_key")),
                    allowed_ids=self._parse_string_array(
                        runtime_record.get("allowed_ids"),
                        context=f"{context}[{index}].runtime_upgrade.allowed_ids",
                    ),
                )
            snapshots.append(
                LaneGovernanceSnapshot(
                    lane_id=self._coerce_int(record.get("lane_id"), f"{context}[{index}].lane_id"),
                    alias="" if record.get("alias") is None else str(record.get("alias")),
                    dataspace_id=self._coerce_int(record.get("dataspace_id"), f"{context}[{index}].dataspace_id"),
                    visibility="" if record.get("visibility") is None else str(record.get("visibility")),
                    storage_profile="" if record.get("storage_profile") is None else str(record.get("storage_profile")),
                    governance=None if record.get("governance") is None else str(record.get("governance")),
                    manifest_required=self._coerce_bool(
                        record.get("manifest_required"),
                        f"{context}[{index}].manifest_required",
                    ),
                    manifest_ready=self._coerce_bool(
                        record.get("manifest_ready"),
                        f"{context}[{index}].manifest_ready",
                    ),
                    manifest_path=None if record.get("manifest_path") is None else str(record.get("manifest_path")),
                    validator_ids=self._parse_string_array(
                        record.get("validator_ids"),
                        context=f"{context}[{index}].validator_ids",
                    ),
                    quorum=self._coerce_optional_unsigned(
                        record.get("quorum"),
                        context=f"{context}[{index}].quorum",
                    ),
                    protected_namespaces=self._parse_string_array(
                        record.get("protected_namespaces"),
                        context=f"{context}[{index}].protected_namespaces",
                    ),
                    runtime_upgrade=runtime_hook,
                )
            )
        return snapshots

    @staticmethod
    def _parse_string_array(value: Any, *, context: str) -> List[str]:
        if value is None:
            return []
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list")
        result: List[str] = []
        for index, entry in enumerate(value):
            if not isinstance(entry, str):
                raise RuntimeError(f"{context}[{index}] must be a string")
            if entry:
                result.append(entry)
        return result

    @staticmethod
    def _parse_connect_status(payload: Mapping[str, Any], *, context: str) -> ConnectStatusSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)
        per_ip_raw = record.get("per_ip_sessions") or []
        if not isinstance(per_ip_raw, list):
            raise RuntimeError(f"{context}.per_ip_sessions must be a list")
        per_ip_sessions = [
            ToriiClient._parse_connect_per_ip(entry, context=f"{context}.per_ip_sessions[{index}]")
            for index, entry in enumerate(per_ip_raw)
        ]
        policy_value = record.get("policy")
        policy = (
            ToriiClient._parse_connect_status_policy(policy_value, context=f"{context}.policy")
            if isinstance(policy_value, Mapping)
            else None
        )
        return ConnectStatusSnapshot(
            enabled=bool(record.get("enabled")),
            sessions_total=ToriiClient._coerce_int(record.get("sessions_total"), f"{context}.sessions_total"),
            sessions_active=ToriiClient._coerce_int(record.get("sessions_active"), f"{context}.sessions_active"),
            per_ip_sessions=per_ip_sessions,
            buffered_sessions=ToriiClient._coerce_int(record.get("buffered_sessions"), f"{context}.buffered_sessions"),
            total_buffer_bytes=ToriiClient._coerce_int(
                record.get("total_buffer_bytes"),
                f"{context}.total_buffer_bytes",
            ),
            dedupe_size=ToriiClient._coerce_int(record.get("dedupe_size"), f"{context}.dedupe_size"),
            frames_in_total=ToriiClient._coerce_int(record.get("frames_in_total"), f"{context}.frames_in_total"),
            frames_out_total=ToriiClient._coerce_int(record.get("frames_out_total"), f"{context}.frames_out_total"),
            ciphertext_total=ToriiClient._coerce_int(record.get("ciphertext_total"), f"{context}.ciphertext_total"),
            dedupe_drops_total=ToriiClient._coerce_int(
                record.get("dedupe_drops_total"),
                f"{context}.dedupe_drops_total",
            ),
            buffer_drops_total=ToriiClient._coerce_int(
                record.get("buffer_drops_total"),
                f"{context}.buffer_drops_total",
            ),
            plaintext_control_drops_total=ToriiClient._coerce_int(
                record.get("plaintext_control_drops_total"),
                f"{context}.plaintext_control_drops_total",
            ),
            monotonic_drops_total=ToriiClient._coerce_int(
                record.get("monotonic_drops_total"),
                f"{context}.monotonic_drops_total",
            ),
            ping_miss_total=ToriiClient._coerce_int(record.get("ping_miss_total"), f"{context}.ping_miss_total"),
            policy=policy,
        )

    @staticmethod
    def _parse_connect_per_ip(payload: Any, *, context: str) -> ConnectPerIpSessions:
        record = ToriiClient._ensure_mapping(payload, context)
        ip_value = record.get("ip")
        if not isinstance(ip_value, str) or not ip_value:
            raise RuntimeError(f"{context} missing `ip`")
        sessions = ToriiClient._coerce_unsigned(record.get("sessions"), f"{context}.sessions")
        return ConnectPerIpSessions(ip=ip_value, sessions=sessions)

    @staticmethod
    def _parse_connect_status_policy(payload: Mapping[str, Any], *, context: str) -> ConnectStatusPolicy:
        record = ToriiClient._ensure_mapping(payload, context)

        return ConnectStatusPolicy(
            relay_enabled=ToriiClient._optional_bool(record.get("relay_enabled"), f"{context}.relay_enabled"),
            ws_max_sessions=ToriiClient._coerce_optional_unsigned(
                record.get("ws_max_sessions"),
                context=f"{context}.ws_max_sessions",
            ),
            ws_per_ip_max_sessions=ToriiClient._coerce_optional_unsigned(
                record.get("ws_per_ip_max_sessions"),
                context=f"{context}.ws_per_ip_max_sessions",
            ),
            ws_rate_per_ip_per_min=ToriiClient._coerce_optional_unsigned(
                record.get("ws_rate_per_ip_per_min"),
                context=f"{context}.ws_rate_per_ip_per_min",
            ),
            session_ttl_ms=ToriiClient._coerce_optional_unsigned(
                record.get("session_ttl_ms"),
                context=f"{context}.session_ttl_ms",
            ),
            frame_max_bytes=ToriiClient._coerce_optional_unsigned(
                record.get("frame_max_bytes"),
                context=f"{context}.frame_max_bytes",
            ),
            session_buffer_max_bytes=ToriiClient._coerce_optional_unsigned(
                record.get("session_buffer_max_bytes"),
                context=f"{context}.session_buffer_max_bytes",
            ),
            heartbeat_interval_ms=ToriiClient._coerce_optional_unsigned(
                record.get("heartbeat_interval_ms"),
                context=f"{context}.heartbeat_interval_ms",
            ),
            heartbeat_miss_tolerance=ToriiClient._coerce_optional_unsigned(
                record.get("heartbeat_miss_tolerance"),
                context=f"{context}.heartbeat_miss_tolerance",
            ),
            heartbeat_min_interval_ms=ToriiClient._coerce_optional_unsigned(
                record.get("heartbeat_min_interval_ms"),
                context=f"{context}.heartbeat_min_interval_ms",
            ),
            extra={
                key: value
                for key, value in record.items()
                if key
                not in {
                    "relay_enabled",
                    "ws_max_sessions",
                    "ws_per_ip_max_sessions",
                    "ws_rate_per_ip_per_min",
                    "session_ttl_ms",
                    "frame_max_bytes",
                    "session_buffer_max_bytes",
                    "heartbeat_interval_ms",
                    "heartbeat_miss_tolerance",
                    "heartbeat_min_interval_ms",
                }
            },
        )

    @staticmethod
    def _parse_connect_session(payload: Mapping[str, Any], *, context: str) -> ConnectSessionInfo:
        record = ToriiClient._ensure_mapping(payload, context)
        sid = ToriiClient._require_string(record.get("sid"), f"{context}.sid")
        wallet_uri = ToriiClient._require_string(record.get("wallet_uri"), f"{context}.wallet_uri")
        app_uri = ToriiClient._require_string(record.get("app_uri"), f"{context}.app_uri")
        token_app = ToriiClient._require_string(record.get("token_app"), f"{context}.token_app")
        token_wallet = ToriiClient._require_string(record.get("token_wallet"), f"{context}.token_wallet")
        known = {
            "sid",
            "wallet_uri",
            "app_uri",
            "token_app",
            "token_wallet",
        }
        extra = {key: value for key, value in record.items() if key not in known}
        return ConnectSessionInfo(
            sid=sid,
            wallet_uri=wallet_uri,
            app_uri=app_uri,
            token_app=token_app,
            token_wallet=token_wallet,
            extra=extra,
        )

    @staticmethod
    def _parse_connect_app_record(payload: Mapping[str, Any], *, context: str) -> ConnectAppRecord:
        record = ToriiClient._ensure_mapping(payload, context)
        app_id = ToriiClient._require_string(record.get("app_id"), f"{context}.app_id")
        display_name = ToriiClient._optional_string(
            record.get("display_name"),
            f"{context}.display_name",
        )
        description = ToriiClient._optional_string(record.get("description"), f"{context}.description")
        icon_url = ToriiClient._optional_string(record.get("icon_url"), f"{context}.icon_url")
        namespace_source = record.get("namespaces") or []
        namespaces = ToriiClient._parse_string_list(namespace_source, context=f"{context}.namespaces")
        metadata = ToriiClient._require_plain_object(record.get("metadata", {}), f"{context}.metadata")
        policy = ToriiClient._require_plain_object(record.get("policy", {}), f"{context}.policy")
        known = {
            "app_id",
            "display_name",
            "description",
            "icon_url",
            "namespaces",
            "metadata",
            "policy",
        }
        extra = {key: value for key, value in record.items() if key not in known}
        return ConnectAppRecord(
            app_id=app_id,
            display_name=display_name,
            description=description,
            icon_url=icon_url,
            namespaces=namespaces,
            metadata=metadata,
            policy=policy,
            extra=extra,
        )

    @staticmethod
    def _parse_connect_app_page(payload: Mapping[str, Any], *, context: str) -> ConnectAppRegistryPage:
        record = ToriiClient._ensure_mapping(payload, context)
        items_value = record.get("items")
        if not isinstance(items_value, list):
            raise RuntimeError(f"{context}.items must be a list")
        items = [
            ToriiClient._parse_connect_app_record(entry, context=f"{context}.items[{index}]")
            for index, entry in enumerate(items_value)
        ]
        total_value = record.get("total")
        total = None
        if total_value is not None:
            total = ToriiClient._coerce_unsigned(total_value, f"{context}.total")
        cursor = ToriiClient._optional_string(record.get("next_cursor"), f"{context}.next_cursor")
        known = {"items", "total", "next_cursor"}
        extra = {key: value for key, value in record.items() if key not in known}
        return ConnectAppRegistryPage(items=items, total=total, next_cursor=cursor, extra=extra)

    @staticmethod
    def _parse_connect_app_policy(payload: Mapping[str, Any], *, context: str) -> ConnectAppPolicyControls:
        record = ToriiClient._ensure_mapping(payload, context)
        policy_record = record.get("policy")
        if isinstance(policy_record, Mapping):
            source = policy_record
        else:
            source = record

        return ConnectAppPolicyControls(
            relay_enabled=ToriiClient._optional_bool(
                source.get("relay_enabled"),
                f"{context}.relay_enabled",
            ),
            ws_max_sessions=ToriiClient._coerce_optional_unsigned(
                source.get("ws_max_sessions"),
                context=f"{context}.ws_max_sessions",
            ),
            ws_per_ip_max_sessions=ToriiClient._coerce_optional_unsigned(
                source.get("ws_per_ip_max_sessions"),
                context=f"{context}.ws_per_ip_max_sessions",
            ),
            ws_rate_per_ip_per_min=ToriiClient._coerce_optional_unsigned(
                source.get("ws_rate_per_ip_per_min"),
                context=f"{context}.ws_rate_per_ip_per_min",
            ),
            session_ttl_ms=ToriiClient._coerce_optional_unsigned(
                source.get("session_ttl_ms"),
                context=f"{context}.session_ttl_ms",
            ),
            frame_max_bytes=ToriiClient._coerce_optional_unsigned(
                source.get("frame_max_bytes"),
                context=f"{context}.frame_max_bytes",
            ),
            session_buffer_max_bytes=ToriiClient._coerce_optional_unsigned(
                source.get("session_buffer_max_bytes"),
                context=f"{context}.session_buffer_max_bytes",
            ),
            heartbeat_interval_ms=ToriiClient._coerce_optional_unsigned(
                source.get("heartbeat_interval_ms"),
                context=f"{context}.heartbeat_interval_ms",
            ),
            heartbeat_miss_tolerance=ToriiClient._coerce_optional_unsigned(
                source.get("heartbeat_miss_tolerance"),
                context=f"{context}.heartbeat_miss_tolerance",
            ),
            heartbeat_min_interval_ms=ToriiClient._coerce_optional_unsigned(
                source.get("heartbeat_min_interval_ms"),
                context=f"{context}.heartbeat_min_interval_ms",
            ),
            extra={
                key: value
                for key, value in source.items()
                if key
                not in {
                    "relay_enabled",
                    "ws_max_sessions",
                    "ws_per_ip_max_sessions",
                    "ws_rate_per_ip_per_min",
                    "session_ttl_ms",
                    "frame_max_bytes",
                    "session_buffer_max_bytes",
                    "heartbeat_interval_ms",
                    "heartbeat_miss_tolerance",
                    "heartbeat_min_interval_ms",
                }
            },
        )

    @staticmethod
    def _parse_connect_manifest(payload: Mapping[str, Any], *, context: str) -> ConnectAdmissionManifest:
        record = ToriiClient._ensure_mapping(payload, context)
        entries_value = record.get("entries")
        if not isinstance(entries_value, list):
            raise RuntimeError(f"{context}.entries must be a list")
        entries = [
            ToriiClient._parse_connect_manifest_entry(entry, context=f"{context}.entries[{index}]")
            for index, entry in enumerate(entries_value)
        ]
        recognized = {"entries", "version", "manifest_hash", "updated_at"}
        extra = {key: value for key, value in record.items() if key not in recognized}
        return ConnectAdmissionManifest(
            version=ToriiClient._coerce_optional_unsigned(record.get("version"), context=f"{context}.version"),
            manifest_hash=ToriiClient._optional_string(record.get("manifest_hash"), f"{context}.manifest_hash"),
            updated_at=ToriiClient._optional_string(record.get("updated_at"), f"{context}.updated_at"),
            entries=entries,
            extra=extra,
        )

    @staticmethod
    def _parse_connect_manifest_entry(payload: Mapping[str, Any], *, context: str) -> ConnectAdmissionManifestEntry:
        record = ToriiClient._ensure_mapping(payload, context)
        app_id = ToriiClient._require_string(record.get("app_id"), f"{context}.app_id")
        namespaces = ToriiClient._parse_string_list(record.get("namespaces", []), context=f"{context}.namespaces")
        metadata = ToriiClient._require_plain_object(record.get("metadata", {}), f"{context}.metadata")
        policy = ToriiClient._require_plain_object(record.get("policy", {}), f"{context}.policy")
        known = {"app_id", "namespaces", "metadata", "policy"}
        extra = {key: value for key, value in record.items() if key not in known}
        return ConnectAdmissionManifestEntry(
            app_id=app_id,
            namespaces=namespaces,
            metadata=metadata,
            policy=policy,
            extra=extra,
        )

    @staticmethod
    def _parse_sumeragi_rbc_snapshot(payload: Mapping[str, Any], *, context: str) -> SumeragiRbcSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)
        return SumeragiRbcSnapshot(
            sessions_active=ToriiClient._coerce_int(record.get("sessions_active"), f"{context}.sessions_active"),
            sessions_pruned_total=ToriiClient._coerce_int(
                record.get("sessions_pruned_total"),
                f"{context}.sessions_pruned_total",
            ),
            ready_broadcasts_total=ToriiClient._coerce_int(
                record.get("ready_broadcasts_total"),
                f"{context}.ready_broadcasts_total",
            ),
            deliver_broadcasts_total=ToriiClient._coerce_int(
                record.get("deliver_broadcasts_total"),
                f"{context}.deliver_broadcasts_total",
            ),
            payload_bytes_delivered_total=ToriiClient._coerce_int(
                record.get("payload_bytes_delivered_total"),
                f"{context}.payload_bytes_delivered_total",
            ),
        )

    @staticmethod
    def _parse_sumeragi_rbc_sessions(payload: Mapping[str, Any], *, context: str) -> SumeragiRbcSessionsSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)
        raw_items = record.get("items") or []
        if not isinstance(raw_items, list):
            raise RuntimeError(f"{context}.items must be a list")
        items = [
            ToriiClient._parse_sumeragi_rbc_session(entry, context=f"{context}.items[{index}]")
            for index, entry in enumerate(raw_items)
        ]
        return SumeragiRbcSessionsSnapshot(
            sessions_active=ToriiClient._coerce_int(record.get("sessions_active"), f"{context}.sessions_active"),
            items=items,
        )

    @staticmethod
    def _parse_sumeragi_rbc_session(payload: Any, *, context: str) -> SumeragiRbcSession:
        record = ToriiClient._ensure_mapping(payload, context)
        block_hash_value = record.get("block_hash")
        block_hash = (
            ToriiClient._require_hex_string(block_hash_value, f"{context}.block_hash")
            if isinstance(block_hash_value, str)
            else None
        )
        payload_hash_value = record.get("payload_hash")
        payload_hash = (
            ToriiClient._require_hex_string(payload_hash_value, f"{context}.payload_hash")
            if isinstance(payload_hash_value, str)
            else None
        )
        return SumeragiRbcSession(
            block_hash=block_hash,
            height=ToriiClient._coerce_int(record.get("height"), f"{context}.height"),
            view=ToriiClient._coerce_int(record.get("view"), f"{context}.view"),
            total_chunks=ToriiClient._coerce_int(record.get("total_chunks"), f"{context}.total_chunks"),
            received_chunks=ToriiClient._coerce_int(record.get("received_chunks"), f"{context}.received_chunks"),
            ready_count=ToriiClient._coerce_int(record.get("ready_count"), f"{context}.ready_count"),
            delivered=ToriiClient._coerce_bool(record.get("delivered"), f"{context}.delivered"),
            invalid=ToriiClient._coerce_bool(record.get("invalid"), f"{context}.invalid"),
            payload_hash=payload_hash,
            recovered=ToriiClient._coerce_bool(record.get("recovered"), f"{context}.recovered"),
        )

    @staticmethod
    def _parse_sumeragi_rbc_delivery_status(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> SumeragiRbcDeliveryStatus:
        record = ToriiClient._ensure_mapping(payload, context)
        block_hash_value = record.get("block_hash")
        block_hash = (
            ToriiClient._require_hex_string(block_hash_value, f"{context}.block_hash")
            if isinstance(block_hash_value, str)
            else None
        )
        return SumeragiRbcDeliveryStatus(
            height=ToriiClient._coerce_int(record.get("height"), f"{context}.height"),
            view=ToriiClient._coerce_int(record.get("view"), f"{context}.view"),
            delivered=ToriiClient._coerce_bool(record.get("delivered"), f"{context}.delivered"),
            present=ToriiClient._coerce_bool(record.get("present"), f"{context}.present"),
            block_hash=block_hash,
            ready_count=ToriiClient._coerce_int(record.get("ready_count"), f"{context}.ready_count"),
            received_chunks=ToriiClient._coerce_int(
                record.get("received_chunks"),
                f"{context}.received_chunks",
            ),
            total_chunks=ToriiClient._coerce_int(record.get("total_chunks"), f"{context}.total_chunks"),
        )

    @staticmethod
    def _parse_sumeragi_evidence_page(payload: Mapping[str, Any], *, context: str) -> SumeragiEvidenceListPage:
        record = ToriiClient._ensure_mapping(payload, context)
        raw_items = record.get("items", [])
        if raw_items is None:
            raw_items = []
        if not isinstance(raw_items, list):
            raise RuntimeError(f"{context}.items must be a list")
        items = [
            ToriiClient._parse_sumeragi_evidence_record(entry, context=f"{context}.items[{index}]")
            for index, entry in enumerate(raw_items)
        ]
        total_value = record.get("total", len(items))
        try:
            total = int(total_value)
        except (TypeError, ValueError) as exc:
            raise RuntimeError(f"{context}.total must be numeric") from exc
        if total < 0:
            raise RuntimeError(f"{context}.total must be non-negative")
        return SumeragiEvidenceListPage(items=items, total=total)

    @staticmethod
    def _parse_sumeragi_evidence_record(payload: Any, *, context: str) -> SumeragiEvidenceRecord:
        record = ToriiClient._ensure_mapping(payload, context)

        def pick(primary: str, alternate: str) -> Any:
            if primary in record:
                return record.get(primary)
            return record.get(alternate)

        kind = ToriiClient._require_non_empty_string(record.get("kind"), f"{context}.kind")
        recorded_height = ToriiClient._coerce_unsigned(
            pick("recorded_height", "recordedHeight"),
            f"{context}.recorded_height",
        )
        recorded_view = ToriiClient._coerce_unsigned(
            pick("recorded_view", "recordedView"),
            f"{context}.recorded_view",
        )
        recorded_ms = ToriiClient._coerce_unsigned(
            pick("recorded_ms", "recordedMs"),
            f"{context}.recorded_ms",
        )

        if kind in {"DoublePrepare", "DoubleCommit"}:
            phase_value = pick("phase", "phase")
            phase_literal = ToriiClient._require_non_empty_string(phase_value, f"{context}.phase")
            if phase_literal not in SUMERAGI_EVIDENCE_PHASES:
                allowed = ", ".join(sorted(SUMERAGI_EVIDENCE_PHASES))
                raise RuntimeError(f"{context}.phase must be one of: {allowed}")
            return SumeragiEvidenceRecord(
                kind=kind,
                recorded_height=recorded_height,
                recorded_view=recorded_view,
                recorded_ms=recorded_ms,
                phase=phase_literal,
                height=ToriiClient._coerce_unsigned(pick("height", "height"), f"{context}.height"),
                view=ToriiClient._coerce_unsigned(pick("view", "view"), f"{context}.view"),
                epoch=ToriiClient._coerce_unsigned(pick("epoch", "epoch"), f"{context}.epoch"),
                signer=ToriiClient._require_non_empty_string(pick("signer", "signer"), f"{context}.signer"),
                block_hash_1=ToriiClient._require_hex_string(
                    pick("block_hash_1", "blockHash1"),
                    f"{context}.block_hash_1",
                ),
                block_hash_2=ToriiClient._require_hex_string(
                    pick("block_hash_2", "blockHash2"),
                    f"{context}.block_hash_2",
                ),
            )
        if kind == "InvalidQc":
            phase_literal = ToriiClient._require_non_empty_string(pick("phase", "phase"), f"{context}.phase")
            if phase_literal not in SUMERAGI_EVIDENCE_PHASES:
                allowed = ", ".join(sorted(SUMERAGI_EVIDENCE_PHASES))
                raise RuntimeError(f"{context}.phase must be one of: {allowed}")
            return SumeragiEvidenceRecord(
                kind=kind,
                recorded_height=recorded_height,
                recorded_view=recorded_view,
                recorded_ms=recorded_ms,
                phase=phase_literal,
                height=ToriiClient._coerce_unsigned(pick("height", "height"), f"{context}.height"),
                view=ToriiClient._coerce_unsigned(pick("view", "view"), f"{context}.view"),
                epoch=ToriiClient._coerce_unsigned(pick("epoch", "epoch"), f"{context}.epoch"),
                subject_block_hash=ToriiClient._require_hex_string(
                    pick("subject_block_hash", "subjectBlockHash"),
                    f"{context}.subject_block_hash",
                ),
                reason=ToriiClient._require_non_empty_string(pick("reason", "reason"), f"{context}.reason"),
            )
        if kind == "InvalidProposal":
            return SumeragiEvidenceRecord(
                kind=kind,
                recorded_height=recorded_height,
                recorded_view=recorded_view,
                recorded_ms=recorded_ms,
                height=ToriiClient._coerce_unsigned(pick("height", "height"), f"{context}.height"),
                view=ToriiClient._coerce_unsigned(pick("view", "view"), f"{context}.view"),
                epoch=ToriiClient._coerce_unsigned(pick("epoch", "epoch"), f"{context}.epoch"),
                subject_block_hash=ToriiClient._require_hex_string(
                    pick("subject_block_hash", "subjectBlockHash"),
                    f"{context}.subject_block_hash",
                ),
                payload_hash=ToriiClient._require_hex_string(
                    pick("payload_hash", "payloadHash"),
                    f"{context}.payload_hash",
                ),
                reason=ToriiClient._require_non_empty_string(pick("reason", "reason"), f"{context}.reason"),
            )
        if kind == "Censorship":
            signers_value = record.get("signers")
            signers = (
                [ToriiClient._require_non_empty_string(item, f"{context}.signers") for item in signers_value]
                if isinstance(signers_value, list)
                else None
            )
            return SumeragiEvidenceRecord(
                kind=kind,
                recorded_height=recorded_height,
                recorded_view=recorded_view,
                recorded_ms=recorded_ms,
                tx_hash=ToriiClient._require_hex_string(pick("tx_hash", "txHash"), f"{context}.tx_hash"),
                receipt_count=ToriiClient._coerce_unsigned(pick("receipt_count", "receiptCount"), f"{context}.receipt_count"),
                min_height=ToriiClient._coerce_unsigned(pick("min_height", "minHeight"), f"{context}.min_height"),
                max_height=ToriiClient._coerce_unsigned(pick("max_height", "maxHeight"), f"{context}.max_height"),
                signers=signers,
            )
        detail_value = record.get("detail")
        detail = (
            ToriiClient._require_non_empty_string(detail_value, f"{context}.detail")
            if isinstance(detail_value, str)
            else None
        )
        return SumeragiEvidenceRecord(
            kind=kind,
            recorded_height=recorded_height,
            recorded_view=recorded_view,
            recorded_ms=recorded_ms,
            detail=detail,
        )

    @staticmethod
    def _parse_rbc_sample(payload: Mapping[str, Any], *, context: str) -> RbcSample:
        record = ToriiClient._ensure_mapping(payload, context)
        samples_value = record.get("samples") or []
        if not isinstance(samples_value, list):
            raise RuntimeError(f"{context}.samples must be a list")
        samples = [
            ToriiClient._parse_rbc_chunk(entry, context=f"{context}.samples[{index}]")
            for index, entry in enumerate(samples_value)
        ]
        payload_hash_value = record.get("payload_hash")
        payload_hash = (
            ToriiClient._require_hex_string(payload_hash_value, f"{context}.payload_hash")
            if isinstance(payload_hash_value, str)
            else None
        )
        return RbcSample(
            block_hash=ToriiClient._require_hex_string(record.get("block_hash"), f"{context}.block_hash"),
            height=ToriiClient._coerce_int(record.get("height"), f"{context}.height"),
            view=ToriiClient._coerce_int(record.get("view"), f"{context}.view"),
            total_chunks=ToriiClient._coerce_int(record.get("total_chunks"), f"{context}.total_chunks"),
            chunk_root=ToriiClient._require_hex_string(record.get("chunk_root"), f"{context}.chunk_root"),
            payload_hash=payload_hash,
            samples=samples,
        )

    @staticmethod
    def _parse_rbc_chunk(payload: Any, *, context: str) -> RbcChunkSample:
        record = ToriiClient._ensure_mapping(payload, context)
        return RbcChunkSample(
            index=ToriiClient._coerce_unsigned(record.get("index"), f"{context}.index"),
            chunk_hex=ToriiClient._require_hex_string(record.get("chunk_hex"), f"{context}.chunk_hex"),
            digest_hex=ToriiClient._require_hex_string(record.get("digest_hex"), f"{context}.digest_hex"),
            proof=ToriiClient._parse_rbc_merkle_proof(record.get("proof"), context=f"{context}.proof"),
        )

    @staticmethod
    def _parse_rbc_merkle_proof(payload: Any, *, context: str) -> RbcMerkleProof:
        record = ToriiClient._ensure_mapping(payload, context)
        audit_path_value = record.get("audit_path") or []
        if not isinstance(audit_path_value, list):
            raise RuntimeError(f"{context}.audit_path must be a list")
        audit_path: List[Optional[str]] = []
        for index, entry in enumerate(audit_path_value):
            if entry is None:
                audit_path.append(None)
            else:
                audit_path.append(
                    ToriiClient._require_hex_string(entry, f"{context}.audit_path[{index}]"),
                )
        depth_value = record.get("depth")
        depth = None
        if depth_value is not None:
            depth = ToriiClient._coerce_unsigned(depth_value, f"{context}.depth")
        return RbcMerkleProof(
            leaf_index=ToriiClient._coerce_unsigned(record.get("leaf_index"), f"{context}.leaf_index"),
            depth=depth,
            audit_path=audit_path,
        )

    @staticmethod
    def _parse_kaigi_relay_summary_list(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> KaigiRelaySummaryList:
        record = ToriiClient._ensure_mapping(payload, context)
        items_value = record.get("items") or []
        if not isinstance(items_value, list):
            raise RuntimeError(f"{context}.items must be a list")
        items = [
            ToriiClient._parse_kaigi_relay_summary(entry, context=f"{context}.items[{index}]")
            for index, entry in enumerate(items_value)
        ]
        total = record.get("total", len(items))
        return KaigiRelaySummaryList(
            total=ToriiClient._coerce_unsigned(total, f"{context}.total"),
            items=items,
        )

    @staticmethod
    def _parse_kaigi_relay_summary(payload: Any, *, context: str) -> KaigiRelaySummary:
        record = ToriiClient._ensure_mapping(payload, context)
        relay_id = record.get("relay_id")
        domain = record.get("domain")
        bandwidth_value = record.get("bandwidth_class") or 0
        fingerprint_value = record.get("hpke_fingerprint_hex")
        status_value = record.get("status")
        status: Optional[str] = None
        if status_value is not None:
            status_literal = ToriiClient._require_non_empty_string(status_value, f"{context}.status").lower()
            if status_literal not in _KAIGI_HEALTH_STATUSES:
                raise RuntimeError(
                    f"{context}.status must be one of {sorted(_KAIGI_HEALTH_STATUSES)}"
                )
            status = status_literal
        reported_at = record.get("reported_at_ms")
        reported_at_ms = (
            ToriiClient._coerce_unsigned(reported_at, f"{context}.reported_at_ms")
            if reported_at is not None
            else None
        )
        return KaigiRelaySummary(
            relay_id=ToriiClient._require_non_empty_string(relay_id, f"{context}.relay_id"),
            domain=ToriiClient._require_non_empty_string(domain, f"{context}.domain"),
            bandwidth_class=ToriiClient._coerce_unsigned(bandwidth_value, f"{context}.bandwidth_class"),
            hpke_fingerprint_hex=ToriiClient._normalize_hex_string(
                ToriiClient._require_hex_string(fingerprint_value, f"{context}.hpke_fingerprint_hex"),
                context=f"{context}.hpke_fingerprint_hex",
            ),
            status=status,
            reported_at_ms=reported_at_ms,
        )

    @staticmethod
    def _parse_kaigi_relay_detail(payload: Mapping[str, Any], *, context: str) -> KaigiRelayDetail:
        record = ToriiClient._ensure_mapping(payload, context)
        relay_summary = ToriiClient._parse_kaigi_relay_summary(record.get("relay"), context=f"{context}.relay")
        hpke_public_key = record.get("hpke_public_key_b64")
        reported_call_value = record.get("reported_call")
        metrics_value = record.get("metrics")
        reported_by_value = record.get("reported_by")
        notes_value = record.get("notes")
        return KaigiRelayDetail(
            relay=relay_summary,
            hpke_public_key_b64=ToriiClient._require_non_empty_string(
                hpke_public_key,
                f"{context}.hpke_public_key_b64",
            ),
            reported_call=ToriiClient._parse_kaigi_relay_reported_call(
                reported_call_value,
                context=f"{context}.reported_call",
            )
            if reported_call_value is not None
            else None,
            reported_by=ToriiClient._optional_string(reported_by_value, f"{context}.reported_by")
            if reported_by_value is not None
            else None,
            notes=str(notes_value) if notes_value is not None else None,
            metrics=ToriiClient._parse_kaigi_relay_domain_metrics(
                metrics_value,
                context=f"{context}.metrics",
            )
            if metrics_value is not None
            else None,
        )

    @staticmethod
    def _parse_kaigi_relay_reported_call(payload: Any, *, context: str) -> KaigiRelayReportedCall:
        record = ToriiClient._ensure_mapping(payload, context)
        domain = record.get("domain_id")
        name = record.get("call_name")
        return KaigiRelayReportedCall(
            domain_id=ToriiClient._require_non_empty_string(domain, f"{context}.domain_id"),
            call_name=ToriiClient._require_non_empty_string(name, f"{context}.call_name"),
        )

    @staticmethod
    def _parse_kaigi_relay_domain_metrics(payload: Any, *, context: str) -> KaigiRelayDomainMetrics:
        record = ToriiClient._ensure_mapping(payload, context)
        domain = record.get("domain")
        return KaigiRelayDomainMetrics(
            domain=ToriiClient._require_non_empty_string(domain, f"{context}.domain"),
            registrations_total=ToriiClient._coerce_unsigned(
                record.get("registrations_total"),
                f"{context}.registrations_total",
            ),
            manifest_updates_total=ToriiClient._coerce_unsigned(
                record.get("manifest_updates_total"),
                f"{context}.manifest_updates_total",
            ),
            failovers_total=ToriiClient._coerce_unsigned(
                record.get("failovers_total"),
                f"{context}.failovers_total",
            ),
            health_reports_total=ToriiClient._coerce_unsigned(
                record.get("health_reports_total"),
                f"{context}.health_reports_total",
            ),
        )

    @staticmethod
    def _parse_kaigi_relay_health_snapshot(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> KaigiRelayHealthSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)
        domains_value = record.get("domains") or []
        if not isinstance(domains_value, list):
            raise RuntimeError(f"{context}.domains must be a list")
        domains = [
            ToriiClient._parse_kaigi_relay_domain_metrics(entry, context=f"{context}.domains[{index}]")
            for index, entry in enumerate(domains_value)
        ]
        return KaigiRelayHealthSnapshot(
            healthy_total=ToriiClient._coerce_unsigned(record.get("healthy_total"), f"{context}.healthy_total"),
            degraded_total=ToriiClient._coerce_unsigned(record.get("degraded_total"), f"{context}.degraded_total"),
            unavailable_total=ToriiClient._coerce_unsigned(
                record.get("unavailable_total"),
                f"{context}.unavailable_total",
            ),
            reports_total=ToriiClient._coerce_unsigned(record.get("reports_total"), f"{context}.reports_total"),
            registrations_total=ToriiClient._coerce_unsigned(
                record.get("registrations_total"),
                f"{context}.registrations_total",
            ),
            failovers_total=ToriiClient._coerce_unsigned(record.get("failovers_total"), f"{context}.failovers_total"),
            domains=domains,
        )

    @staticmethod
    def _parse_sumeragi_qc_entry(value: Any, *, context: str) -> SumeragiQcEntry:
        record = ToriiClient._ensure_mapping(value, context)
        height = ToriiClient._coerce_unsigned(record.get("height"), f"{context}.height")
        view = ToriiClient._coerce_unsigned(record.get("view"), f"{context}.view")
        subject_block_hash = record.get("subject_block_hash")
        if subject_block_hash is not None and not isinstance(subject_block_hash, str):
            raise RuntimeError(f"{context}.subject_block_hash must be a string or null")
        return SumeragiQcEntry(height=height, view=view, subject_block_hash=subject_block_hash)

    @staticmethod
    def _parse_sumeragi_pacemaker(payload: Mapping[str, Any], *, context: str) -> SumeragiPacemakerSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)

        def require_unsigned(key: str) -> int:
            if key not in record:
                raise RuntimeError(f"{context} missing `{key}`")
            return ToriiClient._coerce_unsigned(record.get(key), f"{context}.{key}")

        return SumeragiPacemakerSnapshot(
            backoff_ms=require_unsigned("backoff_ms"),
            rtt_floor_ms=require_unsigned("rtt_floor_ms"),
            jitter_ms=require_unsigned("jitter_ms"),
            backoff_multiplier=require_unsigned("backoff_multiplier"),
            rtt_floor_multiplier=require_unsigned("rtt_floor_multiplier"),
            max_backoff_ms=require_unsigned("max_backoff_ms"),
            jitter_frac_permille=require_unsigned("jitter_frac_permille"),
            round_elapsed_ms=require_unsigned("round_elapsed_ms"),
            view_timeout_target_ms=require_unsigned("view_timeout_target_ms"),
            view_timeout_remaining_ms=require_unsigned("view_timeout_remaining_ms"),
        )

    @staticmethod
    def _parse_sumeragi_phases(payload: Mapping[str, Any], *, context: str) -> SumeragiPhasesSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)

        def require_unsigned(key: str) -> int:
            if key not in record:
                raise RuntimeError(f"{context} missing `{key}`")
            return ToriiClient._coerce_unsigned(record.get(key), f"{context}.{key}")

        ema = ToriiClient._parse_sumeragi_phases_ema(record.get("ema_ms"), context=f"{context}.ema_ms")
        return SumeragiPhasesSnapshot(
            propose_ms=require_unsigned("propose_ms"),
            collect_da_ms=require_unsigned("collect_da_ms"),
            collect_prevote_ms=require_unsigned("collect_prevote_ms"),
            collect_precommit_ms=require_unsigned("collect_precommit_ms"),
            collect_aggregator_ms=require_unsigned("collect_aggregator_ms"),
            commit_ms=require_unsigned("commit_ms"),
            pipeline_total_ms=require_unsigned("pipeline_total_ms"),
            collect_aggregator_gossip_total=require_unsigned("collect_aggregator_gossip_total"),
            block_created_dropped_by_lock_total=require_unsigned("block_created_dropped_by_lock_total"),
            block_created_hint_mismatch_total=require_unsigned("block_created_hint_mismatch_total"),
            block_created_proposal_mismatch_total=require_unsigned("block_created_proposal_mismatch_total"),
            ema_ms=ema,
        )

    @staticmethod
    def _parse_sumeragi_phases_ema(payload: Any, *, context: str) -> SumeragiPhasesEma:
        record = ToriiClient._ensure_mapping(payload, context)

        def require_unsigned(key: str) -> int:
            if key not in record:
                raise RuntimeError(f"{context} missing `{key}`")
            return ToriiClient._coerce_unsigned(record.get(key), f"{context}.{key}")

        return SumeragiPhasesEma(
            propose_ms=require_unsigned("propose_ms"),
            collect_da_ms=require_unsigned("collect_da_ms"),
            collect_prevote_ms=require_unsigned("collect_prevote_ms"),
            collect_precommit_ms=require_unsigned("collect_precommit_ms"),
            collect_aggregator_ms=require_unsigned("collect_aggregator_ms"),
            commit_ms=require_unsigned("commit_ms"),
            pipeline_total_ms=require_unsigned("pipeline_total_ms"),
        )

    @staticmethod
    def _parse_sumeragi_prf(payload: Any, *, context: str) -> SumeragiPrfContext:
        record = ToriiClient._ensure_mapping(payload, context)
        height = ToriiClient._coerce_unsigned(record.get("height"), f"{context}.height")
        view = ToriiClient._coerce_unsigned(record.get("view"), f"{context}.view")
        epoch_seed = record.get("epoch_seed")
        if epoch_seed is not None and not isinstance(epoch_seed, str):
            raise RuntimeError(f"{context}.epoch_seed must be a string or null")
        return SumeragiPrfContext(height=height, view=view, epoch_seed=epoch_seed)

    @staticmethod
    def _parse_sumeragi_collectors(payload: Mapping[str, Any], *, context: str) -> SumeragiCollectorsSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)

        def require_str(key: str) -> str:
            value = record.get(key)
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"{context}.{key} must be a non-empty string")
            return value

        def require_unsigned(key: str) -> int:
            if key not in record:
                raise RuntimeError(f"{context} missing `{key}`")
            return ToriiClient._coerce_unsigned(record.get(key), f"{context}.{key}")

        collectors_value = record.get("collectors") or []
        if not isinstance(collectors_value, list):
            raise RuntimeError(f"{context}.collectors must be a list")
        collectors = [
            ToriiClient._parse_sumeragi_collector_entry(entry, context=f"{context}.collectors[{index}]")
            for index, entry in enumerate(collectors_value)
        ]
        epoch_seed = record.get("epoch_seed")
        if epoch_seed is not None and not isinstance(epoch_seed, str):
            raise RuntimeError(f"{context}.epoch_seed must be a string or null")
        prf = ToriiClient._parse_sumeragi_prf(record.get("prf"), context=f"{context}.prf")
        return SumeragiCollectorsSnapshot(
            consensus_mode=require_str("consensus_mode"),
            mode=require_str("mode"),
            topology_len=require_unsigned("topology_len"),
            min_votes_for_commit=require_unsigned("min_votes_for_commit"),
            proxy_tail_index=require_unsigned("proxy_tail_index"),
            height=require_unsigned("height"),
            view=require_unsigned("view"),
            collectors_k=require_unsigned("collectors_k"),
            redundant_send_r=require_unsigned("redundant_send_r"),
            epoch_seed=epoch_seed,
            collectors=collectors,
            prf=prf,
        )

    @staticmethod
    def _parse_sumeragi_collector_entry(payload: Any, *, context: str) -> SumeragiCollectorEntry:
        record = ToriiClient._ensure_mapping(payload, context)
        index = ToriiClient._coerce_unsigned(record.get("index"), f"{context}.index")
        peer_id = record.get("peer_id")
        if not isinstance(peer_id, str) or not peer_id:
            raise RuntimeError(f"{context}.peer_id must be a non-empty string")
        return SumeragiCollectorEntry(index=index, peer_id=peer_id)

    @staticmethod
    def _parse_sumeragi_params(payload: Mapping[str, Any], *, context: str) -> SumeragiParamsSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)

        def require_unsigned(key: str) -> int:
            if key not in record:
                raise RuntimeError(f"{context} missing `{key}`")
            return ToriiClient._coerce_unsigned(record.get(key), f"{context}.{key}")

        def require_bool(key: str) -> bool:
            value = record.get(key)
            if not isinstance(value, bool):
                raise RuntimeError(f"{context}.{key} must be a boolean")
            return value

        next_mode_value = record.get("next_mode")
        if next_mode_value is not None and not isinstance(next_mode_value, str):
            raise RuntimeError(f"{context}.next_mode must be a string or null")

        return SumeragiParamsSnapshot(
            block_time_ms=require_unsigned("block_time_ms"),
            commit_time_ms=require_unsigned("commit_time_ms"),
            max_clock_drift_ms=require_unsigned("max_clock_drift_ms"),
            collectors_k=require_unsigned("collectors_k"),
            redundant_send_r=require_unsigned("redundant_send_r"),
            da_enabled=require_bool("da_enabled"),
            next_mode=next_mode_value,
            mode_activation_height=ToriiClient._coerce_optional_unsigned(
                record.get("mode_activation_height"),
                context=f"{context}.mode_activation_height",
            ),
            chain_height=require_unsigned("chain_height"),
        )

    @staticmethod
    def _parse_telemetry_peer_info(value: Any, *, context: str) -> PeerTelemetryInfo:
        if not isinstance(value, Mapping):
            raise RuntimeError(f"{context} must be a JSON object")
        url = value.get("url")
        if not isinstance(url, str) or not url:
            raise RuntimeError(f"{context} missing `url`")
        connected = ToriiClient._coerce_bool(value.get("connected"), f"{context}.connected")
        telemetry_flag = value.get("telemetry_unsupported", value.get("telemetryUnsupported", False))
        telemetry_unsupported = ToriiClient._coerce_bool(
            telemetry_flag,
            f"{context}.telemetry_unsupported",
        )
        config_value = value.get("config")
        config = (
            ToriiClient._parse_telemetry_peer_config(config_value, context=f"{context}.config")
            if config_value is not None
            else None
        )
        location_value = value.get("location")
        location = (
            ToriiClient._parse_telemetry_peer_location(location_value, context=f"{context}.location")
            if location_value is not None
            else None
        )
        connected_peers_value = value.get("connected_peers")
        if connected_peers_value is None:
            peers_list: Optional[List[str]] = None
        else:
            if not isinstance(connected_peers_value, list):
                raise RuntimeError(f"{context}.connected_peers must be a list")
            peers_list = []
            for idx, peer in enumerate(connected_peers_value):
                if not isinstance(peer, str) or not peer:
                    raise RuntimeError(f"{context}.connected_peers[{idx}] must be a non-empty string")
                peers_list.append(peer)
        return PeerTelemetryInfo(
            url=url,
            connected=connected,
            telemetry_unsupported=telemetry_unsupported,
            config=config,
            location=location,
            connected_peers=peers_list,
        )

    @staticmethod
    def _parse_telemetry_peer_config(value: Any, *, context: str) -> PeerTelemetryConfig:
        if not isinstance(value, Mapping):
            raise RuntimeError(f"{context} must be a JSON object")
        public_key = value.get("public_key")
        if not isinstance(public_key, str) or not public_key:
            raise RuntimeError(f"{context} missing `public_key`")
        queue_capacity = ToriiClient._coerce_optional_unsigned(
            value.get("queue_capacity"),
            context=f"{context}.queue_capacity",
        )
        block_size = ToriiClient._coerce_optional_unsigned(
            value.get("network_block_gossip_size"),
            context=f"{context}.network_block_gossip_size",
        )
        tx_size = ToriiClient._coerce_optional_unsigned(
            value.get("network_tx_gossip_size"),
            context=f"{context}.network_tx_gossip_size",
        )
        block_period = ToriiClient._parse_optional_duration_ms(
            value.get("network_block_gossip_period"),
            context=f"{context}.network_block_gossip_period",
        )
        tx_period = ToriiClient._parse_optional_duration_ms(
            value.get("network_tx_gossip_period"),
            context=f"{context}.network_tx_gossip_period",
        )
        return PeerTelemetryConfig(
            public_key_hex=public_key,
            queue_capacity=queue_capacity,
            network_block_gossip_size=block_size,
            network_block_gossip_period_ms=block_period,
            network_tx_gossip_size=tx_size,
            network_tx_gossip_period_ms=tx_period,
        )

    @staticmethod
    def _parse_telemetry_peer_location(value: Any, *, context: str) -> PeerTelemetryLocation:
        if not isinstance(value, Mapping):
            raise RuntimeError(f"{context} must be a JSON object")
        lat = ToriiClient._coerce_finite_float(value.get("lat"), f"{context}.lat")
        lon = ToriiClient._coerce_finite_float(value.get("lon"), f"{context}.lon")
        country = value.get("country")
        city = value.get("city")
        if not isinstance(country, str) or not country:
            raise RuntimeError(f"{context}.country must be a non-empty string")
        if not isinstance(city, str) or not city:
            raise RuntimeError(f"{context}.city must be a non-empty string")
        return PeerTelemetryLocation(lat=lat, lon=lon, country=country, city=city)

    @staticmethod
    def _parse_optional_duration_ms(value: Any, *, context: str) -> Optional[int]:
        if value is None:
            return None
        if not isinstance(value, Mapping):
            raise RuntimeError(f"{context} must be a JSON object")
        ms_value = value.get("ms")
        return ToriiClient._coerce_optional_unsigned(ms_value, context=f"{context}.ms")

    @staticmethod
    def _coerce_bool(value: Any, context: str) -> bool:
        if isinstance(value, bool):
            return value
        raise RuntimeError(f"{context} must be a boolean")

    @staticmethod
    def _coerce_optional_unsigned(value: Any, *, context: str) -> Optional[int]:
        if value is None:
            return None
        return ToriiClient._coerce_unsigned(value, context)

    @staticmethod
    def _coerce_positive_int(value: Any, *, context: str) -> int:
        result = ToriiClient._coerce_int(value, context)
        if result <= 0:
            raise RuntimeError(f"{context} must be a positive integer")
        return result

    @staticmethod
    def _coerce_finite_float(value: Any, context: str) -> float:
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise RuntimeError(f"{context} must be a finite number")
        result = float(value)
        if not math.isfinite(result):
            raise RuntimeError(f"{context} must be a finite number")
        return result

    @staticmethod
    def _stringify_amount(amount: Union[str, int]) -> str:
        if isinstance(amount, int):
            return str(amount)
        if isinstance(amount, str):
            trimmed = amount.strip()
            if not trimmed:
                raise ValueError("amount string cannot be empty")
            return trimmed
        raise TypeError("amount must be str or int")

    @staticmethod
    def _coerce_unsigned(value: Any, context: str) -> int:
        result = ToriiClient._coerce_int(value, context)
        if result < 0:
            raise RuntimeError(f"{context} must be non-negative")
        return result

    @staticmethod
    def _require_hex_string(value: Any, context: str) -> str:
        if not isinstance(value, str) or not value:
            raise RuntimeError(f"{context} must be a non-empty hex string")
        stripped = value.strip()
        try:
            bytes.fromhex(stripped)
        except ValueError as exc:
            raise RuntimeError(f"{context} must be a valid hex string") from exc
        return stripped

    @staticmethod
    def _normalize_hex_string(
        value: Union[str, bytes, bytearray, memoryview],
        *,
        context: str,
        expected_length: Optional[int] = None,
    ) -> str:
        if isinstance(value, (bytes, bytearray, memoryview)):
            literal = bytes(value).hex()
        elif isinstance(value, str):
            literal = value.strip()
            if literal.startswith(("0x", "0X")):
                literal = literal[2:]
        else:
            raise RuntimeError(f"{context} must be a hex string")
        if not literal:
            raise RuntimeError(f"{context} must be a non-empty hex string")
        normalized = literal.lower()
        if expected_length is not None and len(normalized) != expected_length:
            raise RuntimeError(f"{context} must contain {expected_length} hex characters")
        try:
            bytes.fromhex(normalized)
        except ValueError as exc:
            raise RuntimeError(f"{context} must contain valid hexadecimal characters") from exc
        return normalized

    @staticmethod
    def _normalize_uaid_literal(value: Any, *, context: str) -> str:
        if not isinstance(value, str):
            raise RuntimeError(f"{context} must be a UAID string")
        literal = value.strip()
        if not literal:
            raise RuntimeError(f"{context} must be a UAID string")
        if literal.lower().startswith("uaid:"):
            hex_portion = literal[5:]
        else:
            hex_portion = literal
        normalized = hex_portion.strip()
        if len(normalized) != 64:
            raise RuntimeError(f"{context} must contain 64 hex characters")
        try:
            bytes.fromhex(normalized)
        except ValueError as exc:
            raise RuntimeError(f"{context} must contain valid hexadecimal characters") from exc
        if int(normalized[-1], 16) % 2 == 0:
            raise RuntimeError(f"{context} must have least significant bit set to 1")
        return f"uaid:{normalized.lower()}"

    @staticmethod
    def _require_string(value: Any, context: str) -> str:
        if not isinstance(value, str):
            raise RuntimeError(f"{context} must be a string")
        stripped = value.strip()
        if not stripped:
            raise RuntimeError(f"{context} must not be empty")
        return stripped

    @staticmethod
    def _optional_string(value: Any, context: str) -> Optional[str]:
        if value is None:
            return None
        return ToriiClient._require_string(value, context)

    @staticmethod
    def _coerce_optional_string(value: Any, *, context: str) -> Optional[str]:
        if value is None:
            return None
        if not isinstance(value, str):
            raise RuntimeError(f"{context} must be a string when present")
        stripped = value.strip()
        return stripped or None

    @staticmethod
    def _optional_bool(value: Any, context: str) -> Optional[bool]:
        if value is None:
            return None
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            normalized = value.strip().lower()
            if normalized in {"true", "1"}:
                return True
            if normalized in {"false", "0"}:
                return False
        raise RuntimeError(f"{context} must be a boolean when present")

    @staticmethod
    def _require_plain_object(value: Any, context: str) -> Dict[str, Any]:
        if value is None:
            return {}
        if isinstance(value, Mapping):
            return dict(value)
        raise RuntimeError(f"{context} must be an object")

    @staticmethod
    def _reject_governance_public_input_key(
        target: MutableMapping[str, Any],
        key: str,
        canonical_key: str,
        *,
        context: str,
    ) -> None:
        if key not in target:
            return
        raise RuntimeError(
            f"{context} must use {canonical_key} (unsupported key {key})"
        )

    @staticmethod
    def _normalize_governance_public_hex_hint(
        target: MutableMapping[str, Any],
        key: str,
        *,
        context: str,
    ) -> None:
        if key not in target:
            return
        value = target[key]
        if value is None:
            return
        if isinstance(value, (bytes, bytearray, memoryview)):
            raw = ToriiClient._normalize_hex_string(
                value,
                context=f"{context}.{key}",
                expected_length=64,
            )
            target[key] = raw.lower()
            return
        if isinstance(value, (list, tuple)):
            try:
                raw = bytes(value).hex()
            except (TypeError, ValueError) as exc:
                raise RuntimeError(
                    f"{context}.{key} must be a 32-byte hex string"
                ) from exc
            if len(raw) != 64:
                raise RuntimeError(f"{context}.{key} must be a 32-byte hex string")
            target[key] = raw.lower()
            return
        if not isinstance(value, str):
            raise RuntimeError(f"{context}.{key} must be a 32-byte hex string")
        raw = value.strip()
        if ":" in raw:
            scheme, rest = raw.split(":", 1)
            if scheme and scheme.lower() != "blake2b32":
                raise RuntimeError(f"{context}.{key} must be a 32-byte hex string")
            raw = rest.strip()
        if raw.startswith(("0x", "0X")):
            raw = raw[2:]
        if len(raw) != 64 or any(ch not in "0123456789abcdefABCDEF" for ch in raw):
            raise RuntimeError(f"{context}.{key} must be a 32-byte hex string")
        target[key] = raw.lower()

    @staticmethod
    def _ensure_governance_lock_hints_complete(
        owner: Any,
        amount: Any,
        duration_blocks: Any,
        *,
        context: str,
    ) -> None:
        has_owner = owner is not None
        has_amount = amount is not None
        has_duration = duration_blocks is not None
        has_any = has_owner or has_amount or has_duration
        if has_any and not (has_owner and has_amount and has_duration):
            raise RuntimeError(
                f"{context} must include owner, amount, duration_blocks when providing lock hints"
            )

    @staticmethod
    def _ensure_governance_owner_canonical(owner: Any, *, context: str) -> None:
        if owner is None:
            return
        if not isinstance(owner, str):
            raise RuntimeError(f"{context}.owner must be a canonical Katakana i105 account id")
        trimmed = owner.strip()
        if not trimmed or trimmed != owner:
            raise RuntimeError(f"{context}.owner must be a canonical Katakana i105 account id")
        if any(ch.isspace() for ch in trimmed):
            raise RuntimeError(f"{context}.owner must be a canonical Katakana i105 account id")
        if "@" in trimmed:
            raise RuntimeError(f"{context}.owner must be a canonical Katakana i105 account id")
        if trimmed.lower().startswith("0x"):
            raise RuntimeError(f"{context}.owner must be a canonical Katakana i105 account id")
        try:
            _decode_i105_string(trimmed)
        except ValueError as exc:
            raise RuntimeError(f"{context}.owner must be a canonical Katakana i105 account id") from exc

    @classmethod
    def _normalize_governance_zk_public_inputs(
        cls,
        value: Optional[Mapping[str, Any]],
        *,
        context: str,
    ) -> Optional[Dict[str, Any]]:
        if value is None:
            return None
        if not isinstance(value, Mapping):
            raise RuntimeError(f"{context} must be an object")
        normalized = dict(value)
        cls._reject_governance_public_input_key(
            normalized,
            "durationBlocks",
            "duration_blocks",
            context=context,
        )
        cls._reject_governance_public_input_key(
            normalized,
            "root_hint_hex",
            "root_hint",
            context=context,
        )
        cls._reject_governance_public_input_key(
            normalized,
            "rootHintHex",
            "root_hint",
            context=context,
        )
        cls._reject_governance_public_input_key(
            normalized,
            "rootHint",
            "root_hint",
            context=context,
        )
        cls._reject_governance_public_input_key(
            normalized,
            "nullifier_hex",
            "nullifier",
            context=context,
        )
        cls._reject_governance_public_input_key(
            normalized,
            "nullifierHex",
            "nullifier",
            context=context,
        )
        cls._normalize_governance_public_hex_hint(
            normalized,
            "root_hint",
            context=context,
        )
        cls._normalize_governance_public_hex_hint(
            normalized,
            "nullifier",
            context=context,
        )
        cls._ensure_governance_lock_hints_complete(
            normalized.get("owner"),
            normalized.get("amount"),
            normalized.get("duration_blocks"),
            context=context,
        )
        cls._ensure_governance_owner_canonical(
            normalized.get("owner"),
            context=context,
        )
        return normalized

    @staticmethod
    def _parse_string_list(value: Any, *, context: str) -> List[str]:
        if value is None:
            return []
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list of strings")
        result: List[str] = []
        for index, entry in enumerate(value):
            if not isinstance(entry, str):
                raise RuntimeError(f"{context}[{index}] must be a string")
            stripped = entry.strip()
            if stripped:
                result.append(stripped)
        return result

    @staticmethod
    def _parse_int_list(value: Any, *, context: str) -> List[int]:
        if value is None:
            return []
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list")
        numbers: List[int] = []
        for index, entry in enumerate(value):
            try:
                numbers.append(int(entry))
            except (TypeError, ValueError) as exc:
                raise RuntimeError(f"{context}[{index}] must be numeric") from exc
        return numbers

    @staticmethod
    def _parse_node_capabilities(payload: Mapping[str, Any], *, context: str) -> NodeCapabilities:
        record = ToriiClient._ensure_mapping(payload, context)
        abi_version = ToriiClient._coerce_unsigned(
            record.get("abi_version"),
            f"{context}.abi_version",
        )
        if abi_version == 0:
            raise RuntimeError(f"{context}.abi_version must be greater than zero")
        data_model_version = ToriiClient._coerce_unsigned(
            record.get("data_model_version"),
            f"{context}.data_model_version",
        )
        if data_model_version == 0:
            raise RuntimeError(f"{context}.data_model_version must be greater than zero")
        crypto_record = ToriiClient._ensure_mapping(record.get("crypto", {}), f"{context}.crypto")
        sm = ToriiClient._parse_node_sm_capabilities(
            crypto_record.get("sm"),
            context=f"{context}.crypto.sm",
        )
        curves = ToriiClient._parse_node_curve_capabilities(
            crypto_record.get("curves"),
            context=f"{context}.crypto.curves",
        )
        return NodeCapabilities(
            abi_version=abi_version,
            data_model_version=data_model_version,
            crypto=NodeCryptoCapabilities(sm=sm, curves=curves),
        )

    @staticmethod
    def _parse_node_sm_capabilities(value: Any, *, context: str) -> NodeSmCapabilities:
        record = ToriiClient._ensure_mapping(value or {}, context)
        enabled = ToriiClient._coerce_bool(record.get("enabled"), f"{context}.enabled")
        default_hash = ToriiClient._coerce_optional_string(
            record.get("default_hash"),
            context=f"{context}.default_hash",
        )
        allowed_signing = ToriiClient._parse_string_list(
            record.get("allowed_signing"),
            context=f"{context}.allowed_signing",
        )
        sm2_distid_default = ToriiClient._coerce_optional_string(
            record.get("sm2_distid_default"),
            context=f"{context}.sm2_distid_default",
        )
        openssl_preview = record.get("openssl_preview", False)
        if not isinstance(openssl_preview, bool):
            raise RuntimeError(f"{context}.openssl_preview must be a boolean")
        acceleration = ToriiClient._parse_node_sm_acceleration(
            record.get("acceleration"),
            context=f"{context}.acceleration",
        )
        return NodeSmCapabilities(
            enabled=enabled,
            default_hash=default_hash,
            allowed_signing=allowed_signing,
            sm2_distid_default=sm2_distid_default,
            openssl_preview=openssl_preview,
            acceleration=acceleration,
        )

    @staticmethod
    def _parse_node_sm_acceleration(value: Any, *, context: str) -> NodeSmAcceleration:
        record = ToriiClient._ensure_mapping(value or {}, context)
        scalar = record.get("scalar", True)
        neon_sm3 = record.get("neon_sm3", False)
        neon_sm4 = record.get("neon_sm4", False)
        if not isinstance(scalar, bool) or not isinstance(neon_sm3, bool) or not isinstance(neon_sm4, bool):
            raise RuntimeError(f"{context} acceleration flags must be boolean")
        policy = ToriiClient._require_string(record.get("policy", ""), f"{context}.policy")
        return NodeSmAcceleration(
            scalar=scalar,
            neon_sm3=neon_sm3,
            neon_sm4=neon_sm4,
            policy=policy,
        )

    @staticmethod
    def _parse_node_curve_capabilities(value: Any, *, context: str) -> NodeCurveCapabilities:
        record = ToriiClient._ensure_mapping(value or {}, context)
        version_value = record.get("registry_version")
        if version_value is None:
            registry_version = 1
        else:
            registry_version = ToriiClient._coerce_positive_int(version_value, context=f"{context}.registry_version")
        allowed = ToriiClient._parse_int_list(
            record.get("allowed_curve_ids"),
            context=f"{context}.allowed_curve_ids",
        )
        bitmap = ToriiClient._parse_int_list(
            record.get("allowed_curve_bitmap"),
            context=f"{context}.allowed_curve_bitmap",
        )
        return NodeCurveCapabilities(
            registry_version=registry_version,
            allowed_curve_ids=allowed,
            allowed_curve_bitmap=bitmap,
        )

    @staticmethod
    def _parse_runtime_abi_active(payload: Mapping[str, Any], *, context: str) -> RuntimeAbiActive:
        record = ToriiClient._ensure_mapping(payload, context)
        abi_version = ToriiClient._coerce_unsigned(record.get("abi_version"), f"{context}.abi_version")
        if abi_version == 0:
            raise RuntimeError(f"{context}.abi_version must be greater than zero")
        return RuntimeAbiActive(abi_version=abi_version)

    @staticmethod
    def _parse_runtime_abi_hash(payload: Mapping[str, Any], *, context: str) -> RuntimeAbiHash:
        record = ToriiClient._ensure_mapping(payload, context)
        policy = ToriiClient._require_string(record.get("policy"), f"{context}.policy")
        abi_hash_value = ToriiClient._require_string(record.get("abi_hash_hex"), f"{context}.abi_hash_hex")
        abi_hash = ToriiClient._normalize_hex_string(
            abi_hash_value,
            context=f"{context}.abi_hash_hex",
            expected_length=64,
        )
        return RuntimeAbiHash(policy=policy, abi_hash_hex=abi_hash)

    @staticmethod
    def _parse_runtime_metrics(payload: Mapping[str, Any], *, context: str) -> RuntimeMetricsSnapshot:
        record = ToriiClient._ensure_mapping(payload, context)
        abi_version = ToriiClient._coerce_unsigned(record.get("abi_version"), f"{context}.abi_version")
        counters_record = ToriiClient._ensure_mapping(
            record.get("upgrade_events_total", {}),
            f"{context}.upgrade_events_total",
        )
        proposed = ToriiClient._coerce_unsigned(
            counters_record.get("proposed", 0),
            f"{context}.upgrade_events_total.proposed",
        )
        activated = ToriiClient._coerce_unsigned(
            counters_record.get("activated", 0),
            f"{context}.upgrade_events_total.activated",
        )
        canceled = ToriiClient._coerce_unsigned(
            counters_record.get("canceled", 0),
            f"{context}.upgrade_events_total.canceled",
        )
        return RuntimeMetricsSnapshot(
            abi_version=abi_version,
            upgrade_events_total=RuntimeUpgradeEventCounters(
                proposed=proposed,
                activated=activated,
                canceled=canceled,
            ),
        )

    @staticmethod
    def _parse_runtime_upgrade_item(value: Any, index: int, *, context: str) -> RuntimeUpgradeListItem:
        record = ToriiClient._ensure_mapping(value, f"{context}[{index}]")
        identifier_value = ToriiClient._require_string(
            record.get("id_hex"),
            f"{context}[{index}].id_hex",
        )
        identifier = ToriiClient._normalize_hex_string(
            identifier_value,
            context=f"{context}[{index}].id_hex",
            expected_length=64,
        )
        record_payload = ToriiClient._ensure_mapping(
            record.get("record"),
            context=f"{context}[{index}].record",
        )
        normalized_record = ToriiClient._parse_runtime_upgrade_record(
            record_payload,
            context=f"{context}[{index}].record",
        )
        return RuntimeUpgradeListItem(id_hex=identifier, record=normalized_record)

    @staticmethod
    def _parse_runtime_upgrade_record(payload: Mapping[str, Any], *, context: str) -> RuntimeUpgradeRecord:
        record = ToriiClient._ensure_mapping(payload, context)
        manifest = ToriiClient._parse_runtime_upgrade_manifest(record.get("manifest"), context=f"{context}.manifest")
        status = ToriiClient._parse_runtime_upgrade_status(record.get("status"), context=f"{context}.status")
        proposer = ToriiClient._require_string(record.get("proposer"), f"{context}.proposer")
        created_height = ToriiClient._coerce_unsigned(
            record.get("created_height"),
            f"{context}.created_height",
        )
        return RuntimeUpgradeRecord(
            manifest=manifest,
            status=status,
            proposer=proposer,
            created_height=created_height,
        )

    @staticmethod
    def _parse_runtime_upgrade_manifest(value: Any, *, context: str) -> RuntimeUpgradeManifest:
        record = ToriiClient._ensure_mapping(value, context)
        name = ToriiClient._require_string(record.get("name"), f"{context}.name")
        description = ToriiClient._require_string(record.get("description"), f"{context}.description")
        abi_version = ToriiClient._coerce_unsigned(
            record.get("abi_version"),
            f"{context}.abi_version",
        )
        if abi_version == 0:
            raise RuntimeError(f"{context}.abi_version must be greater than zero")
        abi_hash_value = ToriiClient._require_string(record.get("abi_hash"), f"{context}.abi_hash")
        abi_hash = ToriiClient._normalize_hex_string(
            abi_hash_value,
            context=f"{context}.abi_hash",
            expected_length=64,
        )
        added_syscalls = ToriiClient._parse_int_list(
            record.get("added_syscalls"),
            context=f"{context}.added_syscalls",
        )
        added_pointer_types = ToriiClient._parse_int_list(
            record.get("added_pointer_types"),
            context=f"{context}.added_pointer_types",
        )
        start_height = ToriiClient._coerce_unsigned(
            record.get("start_height"),
            f"{context}.start_height",
        )
        end_height = ToriiClient._coerce_unsigned(
            record.get("end_height"),
            f"{context}.end_height",
        )
        if end_height <= start_height:
            raise RuntimeError(f"{context}.end_height must be greater than start_height")
        ToriiClient._validate_first_release_runtime_manifest(
            abi_version=abi_version,
            added_syscalls=added_syscalls,
            added_pointer_types=added_pointer_types,
            context=context,
        )
        return RuntimeUpgradeManifest(
            name=name,
            description=description,
            abi_version=abi_version,
            abi_hash_hex=abi_hash,
            added_syscalls=added_syscalls,
            added_pointer_types=added_pointer_types,
            start_height=start_height,
            end_height=end_height,
        )

    @staticmethod
    def _parse_runtime_upgrade_status(value: Any, *, context: str) -> RuntimeUpgradeStatus:
        record = ToriiClient._ensure_mapping(value or {}, context)
        if len(record) != 1:
            raise RuntimeError(f"{context} must contain exactly one status entry")
        kind, payload = next(iter(record.items()))
        if kind in {"Proposed", "Canceled"}:
            return RuntimeUpgradeStatus(kind=kind, activated_height=None)
        if kind == "ActivatedAt":
            height = ToriiClient._coerce_unsigned(payload, f"{context}.ActivatedAt")
            return RuntimeUpgradeStatus(kind=kind, activated_height=height)
        raise RuntimeError(f"{context} contains unsupported variant {kind}")

    @staticmethod
    def _parse_runtime_upgrade_tx_response(payload: Mapping[str, Any], *, context: str) -> RuntimeUpgradeTxResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        ok_flag = record.get("ok")
        if not isinstance(ok_flag, bool):
            raise RuntimeError(f"{context}.ok must be a boolean")
        tx_instructions = ToriiClient._parse_tx_instructions(record.get("tx_instructions"))
        return RuntimeUpgradeTxResponse(ok=ok_flag, tx_instructions=tx_instructions)

    @staticmethod
    def _normalize_runtime_manifest_payload(value: Mapping[str, Any], *, context: str) -> Dict[str, Any]:
        record = ToriiClient._ensure_mapping(value, context)
        name = ToriiClient._require_string(record.get("name"), f"{context}.name")
        description = ToriiClient._require_string(record.get("description"), f"{context}.description")
        abi_version_value = record.get("abi_version")
        abi_hash_value = record.get("abi_hash")
        if abi_version_value is None:
            raise RuntimeError(f"{context}.abi_version is required")
        if abi_hash_value is None:
            raise RuntimeError(f"{context}.abi_hash is required")
        start_value = record.get("start_height")
        end_value = record.get("end_height")
        if start_value is None or end_value is None:
            raise RuntimeError(f"{context}.start_height and {context}.end_height are required")
        start_height = ToriiClient._coerce_unsigned(start_value, f"{context}.start_height")
        end_height = ToriiClient._coerce_unsigned(end_value, f"{context}.end_height")
        if end_height <= start_height:
            raise RuntimeError(f"{context}.end_height must be greater than start_height")
        added_syscalls = ToriiClient._parse_int_list(
            record.get("added_syscalls"),
            context=f"{context}.added_syscalls",
        )
        added_pointer_types = ToriiClient._parse_int_list(
            record.get("added_pointer_types"),
            context=f"{context}.added_pointer_types",
        )
        abi_version = ToriiClient._coerce_unsigned(abi_version_value, f"{context}.abi_version")
        if abi_version == 0:
            raise RuntimeError(f"{context}.abi_version must be greater than zero")
        ToriiClient._validate_first_release_runtime_manifest(
            abi_version=abi_version,
            added_syscalls=added_syscalls,
            added_pointer_types=added_pointer_types,
            context=context,
        )
        abi_hash = ToriiClient._normalize_hex_string(
            abi_hash_value,
            context=f"{context}.abi_hash",
            expected_length=64,
        )
        return {
            "name": name,
            "description": description,
            "abi_version": abi_version,
            "abi_hash": abi_hash,
            "added_syscalls": added_syscalls,
            "added_pointer_types": added_pointer_types,
            "start_height": start_height,
            "end_height": end_height,
        }

    @staticmethod
    def _validate_first_release_runtime_manifest(
        *,
        abi_version: int,
        added_syscalls: list[int],
        added_pointer_types: list[int],
        context: str,
    ) -> None:
        if abi_version != 1:
            raise RuntimeError(f"{context}.abi_version must be 1 in the first release")
        if added_syscalls:
            raise RuntimeError(f"{context}.added_syscalls must be empty in the first release")
        if added_pointer_types:
            raise RuntimeError(f"{context}.added_pointer_types must be empty in the first release")


class _StatusMetricsState:
    """Tracks the previous snapshot to compute delta metrics."""

    def __init__(self) -> None:
        self._previous: Optional[StatusPayload] = None

    def record(self, current: StatusPayload) -> StatusMetrics:
        metrics = _compute_status_metrics(self._previous, current)
        self._previous = current
        return metrics


def _compute_status_metrics(
    previous: Optional[StatusPayload],
    current: StatusPayload,
) -> StatusMetrics:
    queue_delta = 0 if previous is None else current.queue_size - previous.queue_size
    da_delta = (
        0
        if previous is None
        else max(0, current.da_reschedule_total - previous.da_reschedule_total)
    )
    approved_delta = (
        0 if previous is None else max(0, current.txs_approved - previous.txs_approved)
    )
    rejected_delta = (
        0 if previous is None else max(0, current.txs_rejected - previous.txs_rejected)
    )
    view_delta = (
        0 if previous is None else max(0, current.view_changes - previous.view_changes)
    )
    has_activity = any(
        value
        for value in (
            queue_delta,
            da_delta,
            approved_delta,
            rejected_delta,
            view_delta,
        )
    )
    return StatusMetrics(
        commit_latency_ms=current.commit_time_ms,
        queue_size=current.queue_size,
        queue_delta=queue_delta,
        da_reschedule_delta=da_delta,
        tx_approved_delta=approved_delta,
        tx_rejected_delta=rejected_delta,
        view_change_delta=view_delta,
        has_activity=bool(has_activity),
    )


def _monotonic_millis() -> float:
    """Return a monotonic timestamp in milliseconds."""

    return time.perf_counter() * 1000.0


_FILTER_MAPPING: Dict[str, str] = {
    "ok_only": "ok_only",
    "failed_only": "failed_only",
    "errors_only": "errors_only",
    "content_type": "content_type",
    "has_tag": "has_tag",
    "limit": "limit",
    "since_ms": "since_ms",
    "before_ms": "before_ms",
    "ids_only": "ids_only",
    "order": "order",
    "offset": "offset",
    "latest": "latest",
    "messages_only": "messages_only",
    "id": "id",
}

_SUBSCRIPTION_STATUSES = frozenset(
    {
        "active",
        "paused",
        "past_due",
        "canceled",
        "suspended",
    }
)
