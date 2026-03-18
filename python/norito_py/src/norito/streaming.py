"""Adapters and helper types for Norito streaming manifests and telemetry."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from functools import lru_cache
from typing import Any, Callable, ClassVar, List, Optional, Tuple, Type, TypeVar, Union

from .codec import (
    StructAdapter,
    StructField,
    TypeAdapter,
    bool_,
    bytes_,
    fixed_bytes,
    option,
    seq,
    string,
    u8,
    u16,
    u32,
    u64,
    u128,
    i16,
    i32,
)
from .errors import DecodeError

HASH_LEN = 32
SIGNATURE_LEN = 64

T = TypeVar("T")
E = TypeVar("E", bound=Enum)


class NewtypeAdapter(TypeAdapter[T]):
    """Helper adapter that forwards encoding/decoding to an inner adapter."""

    def __init__(self, inner: TypeAdapter[Any], cls: Type[T], attr: str) -> None:
        self._inner = inner
        self._cls = cls
        self._attr = attr

    def encode(self, encoder, value: T) -> None:  # type: ignore[override]
        if not isinstance(value, self._cls):
            raise TypeError(f"expected {self._cls.__name__}, got {type(value)!r}")
        self._inner.encode(encoder, getattr(value, self._attr))

    def decode(self, decoder) -> T:  # type: ignore[override]
        inner_value = self._inner.decode(decoder)
        return self._cls(inner_value)  # type: ignore[arg-type]

    def fixed_size(self) -> Optional[int]:
        return self._inner.fixed_size()

    def is_self_delimiting(self) -> bool:
        return self._inner.is_self_delimiting()


class SimpleEnumAdapter(TypeAdapter[E]):
    """Encode enums as their declaration index (u32 discriminant)."""

    def __init__(self, enum_cls: Type[E]) -> None:
        self._enum_cls = enum_cls
        self._members = list(enum_cls)

    def encode(self, encoder, value: E) -> None:  # type: ignore[override]
        if not isinstance(value, self._enum_cls):
            raise TypeError(f"expected {self._enum_cls.__name__}, got {type(value)!r}")
        index = self._members.index(value)
        encoder.write_uint(index, 32)

    def decode(self, decoder) -> E:  # type: ignore[override]
        index = decoder.read_uint(32)
        if index >= len(self._members):
            raise DecodeError(f"invalid {self._enum_cls.__name__} discriminant {index}")
        return self._members[index]

    def fixed_size(self) -> int:
        return 4

    def is_self_delimiting(self) -> bool:
        return True


_HASH_ADAPTER = fixed_bytes(HASH_LEN)
_SIGNATURE_ADAPTER = fixed_bytes(SIGNATURE_LEN)
_BYTES_ADAPTER = bytes_()


@dataclass(frozen=True)
class ProfileId:
    value: int

    def __post_init__(self) -> None:
        if not 0 <= self.value <= 0xFFFF:
            raise ValueError("ProfileId must fit in u16")


@dataclass(frozen=True)
class CapabilityFlags:
    bits: int

    def __post_init__(self) -> None:
        if not 0 <= self.bits <= 0xFFFF_FFFF:
            raise ValueError("CapabilityFlags must fit in u32")


@dataclass(frozen=True)
class PrivacyCapabilities:
    bits: int

    def __post_init__(self) -> None:
        if not 0 <= self.bits <= 0xFFFF_FFFF:
            raise ValueError("PrivacyCapabilities must fit in u32")


class HpkeSuite(Enum):
    KYBER768_AUTH_PSK = 0x0001
    KYBER1024_AUTH_PSK = 0x0002

    @property
    def bit(self) -> int:
        return 0 if self is HpkeSuite.KYBER768_AUTH_PSK else 1


@dataclass(frozen=True)
class HpkeSuiteMask:
    bits: int

    def __post_init__(self) -> None:
        if not 0 <= self.bits <= 0xFFFF:
            raise ValueError("HpkeSuiteMask must fit in u16")

    def contains(self, suite: HpkeSuite) -> bool:
        return (self.bits & (1 << suite.bit)) != 0


class PrivacyBucketGranularity(Enum):
    STANDARD_V1 = 0


class FecScheme(Enum):
    RS12_10 = 0
    RS_WIN14_10 = 1
    RS18_14 = 2


class StorageClass(Enum):
    EPHEMERAL = 0
    PERMANENT = 1


class CapabilityRole(Enum):
    PUBLISHER = 0
    VIEWER = 1


class AudioLayout(Enum):
    MONO = 0
    STEREO = 1
    FIRST_ORDER_AMBISONICS = 2


class ErrorCode(Enum):
    UNKNOWN_CHUNK = 0
    ACCESS_DENIED = 1
    RATE_LIMITED = 2
    PROTOCOL_VIOLATION = 3


class ResolutionKind(Enum):
    R720P = 0
    R1080P = 1
    R1440P = 2
    R2160P = 3
    CUSTOM = 4


class ControlFrameVariant(Enum):
    MANIFEST_ANNOUNCE = 0
    CHUNK_REQUEST = 1
    CHUNK_ACKNOWLEDGE = 2
    TRANSPORT_CAPABILITIES = 3
    CAPABILITY_REPORT = 4
    CAPABILITY_ACK = 5
    FEEDBACK_HINT = 6
    RECEIVER_REPORT = 7
    KEY_UPDATE = 8
    CONTENT_KEY_UPDATE = 9
    PRIVACY_ROUTE_UPDATE = 10
    PRIVACY_ROUTE_ACK = 11
    ERROR = 12


@dataclass(frozen=True)
class X25519ChaCha20Poly1305Suite:
    fingerprint: bytes

    def __post_init__(self) -> None:
        if len(self.fingerprint) != HASH_LEN:
            raise ValueError("fingerprint must be 32 bytes long")


@dataclass(frozen=True)
class Kyber768XChaCha20Poly1305Suite:
    fingerprint: bytes

    def __post_init__(self) -> None:
        if len(self.fingerprint) != HASH_LEN:
            raise ValueError("fingerprint must be 32 bytes long")


EncryptionSuite = Union[X25519ChaCha20Poly1305Suite, Kyber768XChaCha20Poly1305Suite]


@dataclass(frozen=True)
class TransportCapabilities:
    hpke_suites: HpkeSuiteMask
    supports_datagram: bool
    max_segment_datagram_size: int
    fec_feedback_interval_ms: int
    privacy_bucket_granularity: PrivacyBucketGranularity


@dataclass(frozen=True)
class StreamMetadata:
    title: str
    description: Optional[str]
    access_policy_id: Optional[bytes]
    tags: List[str]


@dataclass(frozen=True)
class PrivacyRelay:
    relay_id: bytes
    endpoint: str
    key_fingerprint: bytes
    capabilities: PrivacyCapabilities


@dataclass(frozen=True)
class PrivacyRoute:
    route_id: bytes
    entry: PrivacyRelay
    exit: PrivacyRelay
    ticket_entry: bytes
    ticket_exit: bytes
    expiry_segment: int


@dataclass(frozen=True)
class NeuralBundle:
    bundle_id: str
    weights_sha256: bytes
    activation_scale: List[int]
    bias: List[int]
    metadata_signature: bytes
    metal_shader_sha256: Optional[bytes]
    cuda_ptx_sha256: Optional[bytes]


@dataclass(frozen=True)
class TicketCapabilities:
    bits: int

    LIVE: ClassVar[int] = 1 << 0
    VOD: ClassVar[int] = 1 << 1
    PREMIUM_PROFILE: ClassVar[int] = 1 << 2
    HDR: ClassVar[int] = 1 << 3
    SPATIAL_AUDIO: ClassVar[int] = 1 << 4

    def __post_init__(self) -> None:
        if not 0 <= self.bits <= 0xFFFF_FFFF:
            raise ValueError("TicketCapabilities must fit in u32")

    @classmethod
    def from_bits(cls, bits: int) -> "TicketCapabilities":
        return cls(bits)

    def contains(self, mask: int) -> bool:
        return (self.bits & mask) == mask

    def insert(self, mask: int) -> "TicketCapabilities":
        return TicketCapabilities(self.bits | mask)

    def remove(self, mask: int) -> "TicketCapabilities":
        return TicketCapabilities(self.bits & ~mask)


@dataclass(frozen=True)
class TicketPolicy:
    max_relays: int
    allowed_regions: List[str]
    max_bandwidth_kbps: Optional[int]

    def __post_init__(self) -> None:
        if not 0 <= self.max_relays <= 0xFFFF:
            raise ValueError("max_relays must fit in u16")
        if self.max_bandwidth_kbps is not None and not (
            0 <= self.max_bandwidth_kbps <= 0xFFFF_FFFF
        ):
            raise ValueError("max_bandwidth_kbps must fit in u32")


@dataclass(frozen=True)
class StreamingTicket:
    ticket_id: bytes
    owner: str
    dsid: int
    lane_id: int
    settlement_bucket: int
    start_slot: int
    expire_slot: int
    prepaid_teu: int
    chunk_teu: int
    fanout_quota: int
    key_commitment: bytes
    nonce: int
    contract_sig: bytes
    commitment: bytes
    nullifier: bytes
    proof_id: bytes
    issued_at: int
    expires_at: int
    policy: Optional[TicketPolicy]
    capabilities: TicketCapabilities

    def __post_init__(self) -> None:
        if len(self.ticket_id) != HASH_LEN:
            raise ValueError("ticket_id must be 32 bytes")
        if len(self.key_commitment) != HASH_LEN:
            raise ValueError("key_commitment must be 32 bytes")
        if len(self.commitment) != HASH_LEN:
            raise ValueError("commitment must be 32 bytes")
        if len(self.nullifier) != HASH_LEN:
            raise ValueError("nullifier must be 32 bytes")
        if len(self.proof_id) != HASH_LEN:
            raise ValueError("proof_id must be 32 bytes")
        if len(self.contract_sig) != SIGNATURE_LEN:
            raise ValueError("contract_sig must be 64 bytes")
        if not 0 <= self.lane_id <= 0xFF:
            raise ValueError("lane_id must fit in u8")
        if not 0 <= self.fanout_quota <= 0xFFFF:
            raise ValueError("fanout_quota must fit in u16")


@dataclass(frozen=True)
class TicketRevocation:
    ticket_id: bytes
    nullifier: bytes
    reason_code: int
    revocation_signature: bytes

    def __post_init__(self) -> None:
        if len(self.ticket_id) != HASH_LEN:
            raise ValueError("ticket_id must be 32 bytes")
        if len(self.nullifier) != HASH_LEN:
            raise ValueError("nullifier must be 32 bytes")
        if len(self.revocation_signature) != SIGNATURE_LEN:
            raise ValueError("revocation_signature must be 64 bytes")
        if not 0 <= self.reason_code <= 0xFFFF:
            raise ValueError("reason_code must fit in u16")


@dataclass(frozen=True)
class ChunkDescriptor:
    chunk_id: int
    offset: int
    length: int
    commitment: bytes
    parity: bool


@dataclass(frozen=True)
class ManifestV1:
    stream_id: bytes
    protocol_version: int
    segment_number: int
    published_at: int
    profile: ProfileId
    da_endpoint: str
    chunk_root: bytes
    content_key_id: int
    nonce_salt: bytes
    chunk_descriptors: List[ChunkDescriptor]
    transport_capabilities_hash: bytes
    encryption_suite: EncryptionSuite
    fec_suite: FecScheme
    privacy_routes: List[PrivacyRoute]
    neural_bundle: Optional[NeuralBundle]
    public_metadata: StreamMetadata
    capabilities: CapabilityFlags
    signature: bytes


@dataclass(frozen=True)
class ManifestAnnounceFrame:
    manifest: ManifestV1


@dataclass(frozen=True)
class ChunkRequestFrame:
    segment: int
    chunk_id: int


@dataclass(frozen=True)
class ChunkAcknowledgeFrame:
    segment: int
    chunk_id: int


@dataclass(frozen=True)
class TransportCapabilitiesFrame:
    endpoint_role: CapabilityRole
    capabilities: TransportCapabilities


@dataclass(frozen=True)
class CapabilityReport:
    stream_id: bytes
    endpoint_role: CapabilityRole
    protocol_version: int
    max_resolution: "Resolution"
    hdr_supported: bool
    capture_hdr: bool
    neural_bundles: List[str]
    audio_caps: "AudioCapability"
    feature_bits: CapabilityFlags
    max_datagram_size: int
    dplpmtud: bool


@dataclass(frozen=True)
class CapabilityAck:
    stream_id: bytes
    accepted_version: int
    negotiated_features: CapabilityFlags
    max_datagram_size: int
    dplpmtud: bool


@dataclass(frozen=True)
class AudioCapability:
    sample_rates: List[int]
    ambisonics: bool
    max_channels: int


@dataclass(frozen=True)
class AudioFrame:
    sequence: int
    timestamp_ns: int
    fec_level: int
    channel_layout: AudioLayout
    payload: bytes


@dataclass(frozen=True)
class ResolutionCustom:
    width: int
    height: int

    def __post_init__(self) -> None:
        if not (0 <= self.width <= 0xFFFF and 0 <= self.height <= 0xFFFF):
            raise ValueError("ResolutionCustom dimensions must fit in u16")


@dataclass(frozen=True)
class Resolution:
    kind: ResolutionKind
    custom: Optional[ResolutionCustom] = None

    def __post_init__(self) -> None:
        if self.kind is ResolutionKind.CUSTOM:
            if self.custom is None:
                raise ValueError("custom resolution requires parameters")
        elif self.custom is not None:
            raise ValueError("non-custom resolution must not carry parameters")


@dataclass(frozen=True)
class FeedbackHintFrame:
    stream_id: bytes
    loss_ewma_q16: int
    latency_gradient_q16: int
    observed_rtt_ms: int
    report_interval_ms: int
    parity_chunks: int


@dataclass(frozen=True)
class SyncDiagnostics:
    window_ms: int
    samples: int
    avg_audio_jitter_ms: int
    max_audio_jitter_ms: int
    avg_av_drift_ms: int
    max_av_drift_ms: int
    ewma_av_drift_ms: int
    violation_count: int

    def __post_init__(self) -> None:
        if not (0 <= self.window_ms <= 0xFFFF):
            raise ValueError("window_ms must fit in u16")
        if not (0 <= self.samples <= 0xFFFF):
            raise ValueError("samples must fit in u16")
        if not (0 <= self.avg_audio_jitter_ms <= 0xFFFF):
            raise ValueError("avg_audio_jitter_ms must fit in u16")
        if not (0 <= self.max_audio_jitter_ms <= 0xFFFF):
            raise ValueError("max_audio_jitter_ms must fit in u16")
        if not (-0x8000 <= self.avg_av_drift_ms <= 0x7FFF):
            raise ValueError("avg_av_drift_ms must fit in i16")
        if not (0 <= self.max_av_drift_ms <= 0xFFFF):
            raise ValueError("max_av_drift_ms must fit in u16")
        if not (-0x8000 <= self.ewma_av_drift_ms <= 0x7FFF):
            raise ValueError("ewma_av_drift_ms must fit in i16")
        if not (0 <= self.violation_count <= 0xFFFF):
            raise ValueError("violation_count must fit in u16")


@dataclass(frozen=True)
class ReceiverReport:
    stream_id: bytes
    latest_segment: int
    layer_mask: int
    measured_throughput_kbps: int
    rtt_ms: int
    loss_percent_x100: int
    decoder_buffer_ms: int
    active_resolution: Resolution
    hdr_active: bool
    ecn_ce_count: int
    jitter_ms: int
    delivered_sequence: int
    parity_applied: int
    fec_budget: int
    sync_diagnostics: Optional[SyncDiagnostics] = None


@dataclass(frozen=True)
class TelemetryEncodeStats:
    segment: int
    avg_latency_ms: int
    dropped_layers: int
    avg_audio_jitter_ms: int
    max_audio_jitter_ms: int


@dataclass(frozen=True)
class TelemetryDecodeStats:
    segment: int
    buffer_ms: int
    dropped_frames: int
    max_decode_queue_ms: int
    avg_av_drift_ms: int
    max_av_drift_ms: int


@dataclass(frozen=True)
class TelemetryNetworkStats:
    rtt_ms: int
    loss_percent_x100: int
    fec_repairs: int
    fec_failures: int
    datagram_reinjects: int


@dataclass(frozen=True)
class TelemetrySecurityStats:
    suite: EncryptionSuite
    rekeys: int
    gck_rotations: int


@dataclass(frozen=True)
class TelemetryEnergyStats:
    segment: int
    encoder_milliwatts: int
    decoder_milliwatts: int


@dataclass(frozen=True)
class TelemetryAuditOutcome:
    trace_id: str
    slot_height: int
    reviewer: str
    status: str
    mitigation_url: Optional[str] = None


@dataclass(frozen=True)
class KeyUpdate:
    session_id: bytes
    suite: EncryptionSuite
    protocol_version: int
    pub_ephemeral: bytes
    key_counter: int
    signature: bytes


@dataclass(frozen=True)
class ContentKeyUpdate:
    content_key_id: int
    gck_wrapped: bytes
    valid_from_segment: int


@dataclass(frozen=True)
class PrivacyRouteUpdate:
    route_id: bytes
    stream_id: bytes
    content_key_id: int
    valid_from_segment: int
    valid_until_segment: int
    exit_token: bytes


@dataclass(frozen=True)
class PrivacyRouteAckFrame:
    route_id: bytes


@dataclass(frozen=True)
class ControlErrorFrame:
    code: ErrorCode
    message: str


@dataclass(frozen=True)
class ControlFrame:
    variant: ControlFrameVariant
    payload: object


def hash_adapter() -> TypeAdapter[bytes]:
    return _HASH_ADAPTER


def signature_adapter() -> TypeAdapter[bytes]:
    return _SIGNATURE_ADAPTER


def bytes_adapter() -> TypeAdapter[bytes]:
    return _BYTES_ADAPTER


@lru_cache(maxsize=None)
def profile_id_adapter() -> TypeAdapter[ProfileId]:
    return NewtypeAdapter(u16(), ProfileId, "value")


@lru_cache(maxsize=None)
def capability_flags_adapter() -> TypeAdapter[CapabilityFlags]:
    return NewtypeAdapter(u32(), CapabilityFlags, "bits")


@lru_cache(maxsize=None)
def ticket_capabilities_adapter() -> TypeAdapter[TicketCapabilities]:
    return NewtypeAdapter(u32(), TicketCapabilities, "bits")


@lru_cache(maxsize=None)
def privacy_capabilities_adapter() -> TypeAdapter[PrivacyCapabilities]:
    return NewtypeAdapter(u32(), PrivacyCapabilities, "bits")


@lru_cache(maxsize=None)
def hpke_suite_mask_adapter() -> TypeAdapter[HpkeSuiteMask]:
    return NewtypeAdapter(u16(), HpkeSuiteMask, "bits")


@lru_cache(maxsize=None)
def ticket_policy_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("max_relays", u16()),
            StructField("allowed_regions", seq(string())),
            StructField("max_bandwidth_kbps", option(u32())),
        ],
        factory=TicketPolicy,
    )


@lru_cache(maxsize=None)
def streaming_ticket_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("ticket_id", hash_adapter()),
            StructField("owner", string()),
            StructField("dsid", u64()),
            StructField("lane_id", u8()),
            StructField("settlement_bucket", u64()),
            StructField("start_slot", u64()),
            StructField("expire_slot", u64()),
            StructField("prepaid_teu", u128()),
            StructField("chunk_teu", u32()),
            StructField("fanout_quota", u16()),
            StructField("key_commitment", hash_adapter()),
            StructField("nonce", u64()),
            StructField("contract_sig", signature_adapter()),
            StructField("commitment", hash_adapter()),
            StructField("nullifier", hash_adapter()),
            StructField("proof_id", hash_adapter()),
            StructField("issued_at", u64()),
            StructField("expires_at", u64()),
            StructField("policy", option(ticket_policy_adapter())),
            StructField("capabilities", ticket_capabilities_adapter()),
        ],
        factory=StreamingTicket,
    )


@lru_cache(maxsize=None)
def ticket_revocation_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("ticket_id", hash_adapter()),
            StructField("nullifier", hash_adapter()),
            StructField("reason_code", u16()),
            StructField("revocation_signature", signature_adapter()),
        ],
        factory=TicketRevocation,
    )


@lru_cache(maxsize=None)
def simple_enum_adapter(enum_cls: Type[E]) -> SimpleEnumAdapter[E]:
    return SimpleEnumAdapter(enum_cls)


class EncryptionSuiteAdapter(TypeAdapter[EncryptionSuite]):
    """Adapter mirroring the Rust Norito `EncryptionSuite` enum layout."""

    def encode(self, encoder, value: EncryptionSuite) -> None:  # type: ignore[override]
        if isinstance(value, X25519ChaCha20Poly1305Suite):
            encoder.write_uint(0, 32)
            hash_adapter().encode(encoder, value.fingerprint)
        elif isinstance(value, Kyber768XChaCha20Poly1305Suite):
            encoder.write_uint(1, 32)
            hash_adapter().encode(encoder, value.fingerprint)
        else:
            raise TypeError(f"unsupported EncryptionSuite variant {type(value)!r}")

    def decode(self, decoder) -> EncryptionSuite:  # type: ignore[override]
        tag = decoder.read_uint(32)
        if tag == 0:
            fingerprint = hash_adapter().decode(decoder)
            return X25519ChaCha20Poly1305Suite(fingerprint)
        if tag == 1:
            fingerprint = hash_adapter().decode(decoder)
            return Kyber768XChaCha20Poly1305Suite(fingerprint)
        raise DecodeError(f"invalid EncryptionSuite discriminant {tag}")

    def fixed_size(self) -> int:
        return 4 + HASH_LEN

    def is_self_delimiting(self) -> bool:
        return True


_ENCRYPTION_SUITE_ADAPTER = EncryptionSuiteAdapter()


def encryption_suite_adapter() -> TypeAdapter[EncryptionSuite]:
    return _ENCRYPTION_SUITE_ADAPTER


class ResolutionAdapter(TypeAdapter[Resolution]):
    def encode(self, encoder, value: Resolution) -> None:  # type: ignore[override]
        encoder.write_uint(value.kind.value, 32)
        if value.kind is ResolutionKind.CUSTOM:
            resolution_custom_adapter().encode(encoder, value.custom)

    def decode(self, decoder) -> Resolution:  # type: ignore[override]
        tag = decoder.read_uint(32)
        try:
            kind = ResolutionKind(tag)
        except ValueError as exc:  # pragma: no cover - defensive
            raise DecodeError(f"invalid Resolution discriminant {tag}") from exc
        custom = None
        if kind is ResolutionKind.CUSTOM:
            custom = resolution_custom_adapter().decode(decoder)
        return Resolution(kind, custom)

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


_RESOLUTION_ADAPTER = ResolutionAdapter()


def resolution_adapter() -> TypeAdapter[Resolution]:
    return _RESOLUTION_ADAPTER


@lru_cache(maxsize=None)
def resolution_custom_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("width", u16()),
            StructField("height", u16()),
        ],
        factory=ResolutionCustom,
    )


@lru_cache(maxsize=None)
def transport_capabilities_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("hpke_suites", hpke_suite_mask_adapter()),
            StructField("supports_datagram", bool_()),
            StructField("max_segment_datagram_size", u16()),
            StructField("fec_feedback_interval_ms", u16()),
            StructField(
                "privacy_bucket_granularity",
                simple_enum_adapter(PrivacyBucketGranularity),
            ),
        ],
        factory=TransportCapabilities,
    )


@lru_cache(maxsize=None)
def stream_metadata_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("title", string()),
            StructField("description", option(string())),
            StructField("access_policy_id", option(hash_adapter())),
            StructField("tags", seq(string())),
        ],
        factory=StreamMetadata,
    )


@lru_cache(maxsize=None)
def privacy_relay_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("relay_id", hash_adapter()),
            StructField("endpoint", string()),
            StructField("key_fingerprint", hash_adapter()),
            StructField("capabilities", privacy_capabilities_adapter()),
        ],
        factory=PrivacyRelay,
    )


@lru_cache(maxsize=None)
def privacy_route_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("route_id", hash_adapter()),
            StructField("entry", privacy_relay_adapter()),
            StructField("exit", privacy_relay_adapter()),
            StructField("ticket_entry", bytes_adapter()),
            StructField("ticket_exit", bytes_adapter()),
            StructField("expiry_segment", u64()),
        ],
        factory=PrivacyRoute,
    )


@lru_cache(maxsize=None)
def neural_bundle_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("bundle_id", string()),
            StructField("weights_sha256", hash_adapter()),
            StructField("activation_scale", seq(i16())),
            StructField("bias", seq(i32())),
            StructField("metadata_signature", signature_adapter()),
            StructField("metal_shader_sha256", option(hash_adapter())),
            StructField("cuda_ptx_sha256", option(hash_adapter())),
        ],
        factory=NeuralBundle,
    )


@lru_cache(maxsize=None)
def chunk_descriptor_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("chunk_id", u16()),
            StructField("offset", u32()),
            StructField("length", u32()),
            StructField("commitment", hash_adapter()),
            StructField("parity", bool_()),
        ],
        factory=ChunkDescriptor,
    )


@lru_cache(maxsize=None)
def manifest_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("stream_id", hash_adapter()),
            StructField("protocol_version", u16()),
            StructField("segment_number", u64()),
            StructField("published_at", u64()),
            StructField("profile", profile_id_adapter()),
            StructField("da_endpoint", string()),
            StructField("chunk_root", hash_adapter()),
            StructField("content_key_id", u64()),
            StructField("nonce_salt", hash_adapter()),
            StructField("chunk_descriptors", seq(chunk_descriptor_adapter())),
            StructField("transport_capabilities_hash", hash_adapter()),
            StructField("encryption_suite", encryption_suite_adapter()),
            StructField("fec_suite", simple_enum_adapter(FecScheme)),
            StructField("privacy_routes", seq(privacy_route_adapter())),
            StructField("neural_bundle", option(neural_bundle_adapter())),
            StructField("public_metadata", stream_metadata_adapter()),
            StructField("capabilities", capability_flags_adapter()),
            StructField("signature", signature_adapter()),
        ],
        factory=ManifestV1,
    )


@lru_cache(maxsize=None)
def manifest_announce_frame_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("manifest", manifest_adapter()),
        ],
        factory=ManifestAnnounceFrame,
    )


@lru_cache(maxsize=None)
def chunk_request_frame_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("segment", u64()),
            StructField("chunk_id", u16()),
        ],
        factory=ChunkRequestFrame,
    )


@lru_cache(maxsize=None)
def chunk_acknowledge_frame_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("segment", u64()),
            StructField("chunk_id", u16()),
        ],
        factory=ChunkAcknowledgeFrame,
    )


@lru_cache(maxsize=None)
def transport_capabilities_frame_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("endpoint_role", simple_enum_adapter(CapabilityRole)),
            StructField("capabilities", transport_capabilities_adapter()),
        ],
        factory=TransportCapabilitiesFrame,
    )


@lru_cache(maxsize=None)
def audio_capability_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("sample_rates", seq(u32())),
            StructField("ambisonics", bool_()),
            StructField("max_channels", u8()),
        ],
        factory=AudioCapability,
    )


@lru_cache(maxsize=None)
def capability_report_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("stream_id", hash_adapter()),
            StructField("endpoint_role", simple_enum_adapter(CapabilityRole)),
            StructField("protocol_version", u16()),
            StructField("max_resolution", resolution_adapter()),
            StructField("hdr_supported", bool_()),
            StructField("capture_hdr", bool_()),
            StructField("neural_bundles", seq(string())),
            StructField("audio_caps", audio_capability_adapter()),
            StructField("feature_bits", capability_flags_adapter()),
            StructField("max_datagram_size", u16()),
            StructField("dplpmtud", bool_()),
        ],
        factory=CapabilityReport,
    )


@lru_cache(maxsize=None)
def capability_ack_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("stream_id", hash_adapter()),
            StructField("accepted_version", u16()),
            StructField("negotiated_features", capability_flags_adapter()),
            StructField("max_datagram_size", u16()),
            StructField("dplpmtud", bool_()),
        ],
        factory=CapabilityAck,
    )


@lru_cache(maxsize=None)
def audio_frame_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("sequence", u64()),
            StructField("timestamp_ns", u64()),
            StructField("fec_level", u8()),
            StructField("channel_layout", simple_enum_adapter(AudioLayout)),
            StructField("payload", bytes_adapter()),
        ],
        factory=AudioFrame,
    )


@lru_cache(maxsize=None)
def feedback_hint_frame_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("stream_id", hash_adapter()),
            StructField("loss_ewma_q16", u32()),
            StructField("latency_gradient_q16", i32()),
            StructField("observed_rtt_ms", u16()),
            StructField("report_interval_ms", u16()),
            StructField("parity_chunks", u8()),
        ],
        factory=FeedbackHintFrame,
    )


@lru_cache(maxsize=None)
def sync_diagnostics_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("window_ms", u16()),
            StructField("samples", u16()),
            StructField("avg_audio_jitter_ms", u16()),
            StructField("max_audio_jitter_ms", u16()),
            StructField("avg_av_drift_ms", i16()),
            StructField("max_av_drift_ms", u16()),
            StructField("ewma_av_drift_ms", i16()),
            StructField("violation_count", u16()),
        ],
        factory=SyncDiagnostics,
    )


@lru_cache(maxsize=None)
def receiver_report_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("stream_id", hash_adapter()),
            StructField("latest_segment", u64()),
            StructField("layer_mask", u32()),
            StructField("measured_throughput_kbps", u32()),
            StructField("rtt_ms", u16()),
            StructField("loss_percent_x100", u16()),
            StructField("decoder_buffer_ms", u16()),
            StructField("active_resolution", resolution_adapter()),
            StructField("hdr_active", bool_()),
            StructField("ecn_ce_count", u32()),
            StructField("jitter_ms", u16()),
            StructField("delivered_sequence", u64()),
            StructField("parity_applied", u8()),
            StructField("fec_budget", u8()),
            StructField("sync_diagnostics", option(sync_diagnostics_adapter())),
        ],
        factory=ReceiverReport,
    )


@lru_cache(maxsize=None)
def telemetry_encode_stats_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("segment", u64()),
            StructField("avg_latency_ms", u16()),
            StructField("dropped_layers", u32()),
            StructField("avg_audio_jitter_ms", u16()),
            StructField("max_audio_jitter_ms", u16()),
        ],
        factory=TelemetryEncodeStats,
    )


@lru_cache(maxsize=None)
def telemetry_decode_stats_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("segment", u64()),
            StructField("buffer_ms", u16()),
            StructField("dropped_frames", u16()),
            StructField("max_decode_queue_ms", u16()),
            StructField("avg_av_drift_ms", i16()),
            StructField("max_av_drift_ms", u16()),
        ],
        factory=TelemetryDecodeStats,
    )


@lru_cache(maxsize=None)
def telemetry_network_stats_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("rtt_ms", u16()),
            StructField("loss_percent_x100", u16()),
            StructField("fec_repairs", u32()),
            StructField("fec_failures", u32()),
            StructField("datagram_reinjects", u32()),
        ],
        factory=TelemetryNetworkStats,
    )


@lru_cache(maxsize=None)
def telemetry_security_stats_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("suite", encryption_suite_adapter()),
            StructField("rekeys", u32()),
            StructField("gck_rotations", u32()),
        ],
        factory=TelemetrySecurityStats,
    )


@lru_cache(maxsize=None)
def telemetry_energy_stats_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("segment", u64()),
            StructField("encoder_milliwatts", u32()),
            StructField("decoder_milliwatts", u32()),
        ],
        factory=TelemetryEnergyStats,
    )


@lru_cache(maxsize=None)
def telemetry_audit_outcome_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("trace_id", string()),
            StructField("slot_height", u64()),
            StructField("reviewer", string()),
            StructField("status", string()),
            StructField("mitigation_url", option(string())),
        ],
        factory=TelemetryAuditOutcome,
    )


@dataclass(frozen=True)
class TelemetryEncodeEvent:
    stats: TelemetryEncodeStats


@dataclass(frozen=True)
class TelemetryDecodeEvent:
    stats: TelemetryDecodeStats


@dataclass(frozen=True)
class TelemetryNetworkEvent:
    stats: TelemetryNetworkStats


@dataclass(frozen=True)
class TelemetrySecurityEvent:
    stats: TelemetrySecurityStats


@dataclass(frozen=True)
class TelemetryEnergyEvent:
    stats: TelemetryEnergyStats


@dataclass(frozen=True)
class TelemetryAuditOutcomeEvent:
    stats: TelemetryAuditOutcome


TelemetryEvent = Union[
    TelemetryEncodeEvent,
    TelemetryDecodeEvent,
    TelemetryNetworkEvent,
    TelemetrySecurityEvent,
    TelemetryEnergyEvent,
    TelemetryAuditOutcomeEvent,
]


class TelemetryEventAdapter(TypeAdapter[TelemetryEvent]):
    """Adapter mirroring the Rust Norito `TelemetryEvent` enum layout."""

    def __init__(self) -> None:
        self._entries: Tuple[
            Tuple[Type[Any], StructAdapter], ...
        ] = (
            (TelemetryEncodeEvent, telemetry_encode_stats_adapter()),
            (TelemetryDecodeEvent, telemetry_decode_stats_adapter()),
            (TelemetryNetworkEvent, telemetry_network_stats_adapter()),
            (TelemetrySecurityEvent, telemetry_security_stats_adapter()),
            (TelemetryEnergyEvent, telemetry_energy_stats_adapter()),
            (TelemetryAuditOutcomeEvent, telemetry_audit_outcome_adapter()),
        )

    def encode(self, encoder, value: TelemetryEvent) -> None:  # type: ignore[override]
        for index, (variant_type, adapter) in enumerate(self._entries):
            if isinstance(value, variant_type):
                encoder.write_uint(index, 32)
                adapter.encode(encoder, value.stats)  # type: ignore[arg-type]
                return
        raise TypeError(f"unsupported TelemetryEvent variant {type(value)!r}")

    def decode(self, decoder) -> TelemetryEvent:  # type: ignore[override]
        tag = decoder.read_uint(32)
        if tag >= len(self._entries):
            raise DecodeError(f"invalid TelemetryEvent discriminant {tag}")
        variant_type, adapter = self._entries[tag]
        stats = adapter.decode(decoder)
        return variant_type(stats)  # type: ignore[call-arg]

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


_TELEMETRY_EVENT_ADAPTER = TelemetryEventAdapter()


def telemetry_event_adapter() -> TypeAdapter[TelemetryEvent]:
    return _TELEMETRY_EVENT_ADAPTER


@lru_cache(maxsize=None)
def key_update_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("session_id", hash_adapter()),
            StructField("suite", encryption_suite_adapter()),
            StructField("protocol_version", u16()),
            StructField("pub_ephemeral", bytes_adapter()),
            StructField("key_counter", u64()),
            StructField("signature", signature_adapter()),
        ],
        factory=KeyUpdate,
    )


@lru_cache(maxsize=None)
def content_key_update_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("content_key_id", u64()),
            StructField("gck_wrapped", bytes_adapter()),
            StructField("valid_from_segment", u64()),
        ],
        factory=ContentKeyUpdate,
    )


@lru_cache(maxsize=None)
def privacy_route_update_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("route_id", hash_adapter()),
            StructField("stream_id", hash_adapter()),
            StructField("content_key_id", u64()),
            StructField("valid_from_segment", u64()),
            StructField("valid_until_segment", u64()),
            StructField("exit_token", bytes_adapter()),
        ],
        factory=PrivacyRouteUpdate,
    )


@lru_cache(maxsize=None)
def privacy_route_ack_frame_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("route_id", hash_adapter()),
        ],
        factory=PrivacyRouteAckFrame,
    )


@lru_cache(maxsize=None)
def control_error_frame_adapter() -> StructAdapter:
    return StructAdapter(
        [
            StructField("code", simple_enum_adapter(ErrorCode)),
            StructField("message", string()),
        ],
        factory=ControlErrorFrame,
    )


class ControlFrameAdapter(TypeAdapter[ControlFrame]):
    """Encode control frames using the canonical Norito discriminant ordering."""

    def __init__(self) -> None:
        self._entries: Tuple[
            Tuple[ControlFrameVariant, Callable[[], TypeAdapter[Any]], Type[Any]],
            ...
        ] = (
            (ControlFrameVariant.MANIFEST_ANNOUNCE, manifest_announce_frame_adapter, ManifestAnnounceFrame),
            (ControlFrameVariant.CHUNK_REQUEST, chunk_request_frame_adapter, ChunkRequestFrame),
            (ControlFrameVariant.CHUNK_ACKNOWLEDGE, chunk_acknowledge_frame_adapter, ChunkAcknowledgeFrame),
            (
                ControlFrameVariant.TRANSPORT_CAPABILITIES,
                transport_capabilities_frame_adapter,
                TransportCapabilitiesFrame,
            ),
            (ControlFrameVariant.CAPABILITY_REPORT, capability_report_adapter, CapabilityReport),
            (ControlFrameVariant.CAPABILITY_ACK, capability_ack_adapter, CapabilityAck),
            (ControlFrameVariant.FEEDBACK_HINT, feedback_hint_frame_adapter, FeedbackHintFrame),
            (ControlFrameVariant.RECEIVER_REPORT, receiver_report_adapter, ReceiverReport),
            (ControlFrameVariant.KEY_UPDATE, key_update_adapter, KeyUpdate),
            (ControlFrameVariant.CONTENT_KEY_UPDATE, content_key_update_adapter, ContentKeyUpdate),
            (ControlFrameVariant.PRIVACY_ROUTE_UPDATE, privacy_route_update_adapter, PrivacyRouteUpdate),
            (ControlFrameVariant.PRIVACY_ROUTE_ACK, privacy_route_ack_frame_adapter, PrivacyRouteAckFrame),
            (ControlFrameVariant.ERROR, control_error_frame_adapter, ControlErrorFrame),
        )

    def encode(self, encoder, value: ControlFrame) -> None:  # type: ignore[override]
        if not isinstance(value.variant, ControlFrameVariant):
            raise TypeError("ControlFrame.variant must be a ControlFrameVariant")
        for index, (variant, adapter_fn, payload_type) in enumerate(self._entries):
            if value.variant is variant:
                if not isinstance(value.payload, payload_type):
                    raise TypeError(
                        f"variant {variant.name} expects payload {payload_type.__name__}"
                    )
                encoder.write_uint(index, 32)
                adapter_fn().encode(encoder, value.payload)  # type: ignore[arg-type]
                return
        raise ValueError(f"unsupported ControlFrame variant {value.variant}")

    def decode(self, decoder) -> ControlFrame:  # type: ignore[override]
        tag = decoder.read_uint(32)
        if tag >= len(self._entries):
            raise DecodeError(f"invalid ControlFrame discriminant {tag}")
        variant, adapter_fn, _payload_type = self._entries[tag]
        payload = adapter_fn().decode(decoder)
        return ControlFrame(variant, payload)

    def fixed_size(self) -> Optional[int]:
        return None

    def is_self_delimiting(self) -> bool:
        return True


_CONTROL_FRAME_ADAPTER = ControlFrameAdapter()


def control_frame_adapter() -> TypeAdapter[ControlFrame]:
    return _CONTROL_FRAME_ADAPTER


__all__ = [
    "HASH_LEN",
    "SIGNATURE_LEN",
    "ProfileId",
    "CapabilityFlags",
    "PrivacyCapabilities",
    "HpkeSuite",
    "HpkeSuiteMask",
    "PrivacyBucketGranularity",
    "FecScheme",
    "StorageClass",
    "CapabilityRole",
    "AudioLayout",
    "ErrorCode",
    "ResolutionKind",
    "Resolution",
    "ResolutionCustom",
    "TransportCapabilities",
    "StreamMetadata",
    "PrivacyRelay",
    "PrivacyRoute",
    "NeuralBundle",
    "TicketCapabilities",
    "TicketPolicy",
    "StreamingTicket",
    "TicketRevocation",
    "ChunkDescriptor",
    "ManifestV1",
    "ManifestAnnounceFrame",
    "ChunkRequestFrame",
    "ChunkAcknowledgeFrame",
    "TransportCapabilitiesFrame",
    "CapabilityReport",
    "CapabilityAck",
    "AudioCapability",
    "AudioFrame",
    "FeedbackHintFrame",
    "SyncDiagnostics",
    "ReceiverReport",
    "TelemetryEncodeStats",
    "TelemetryDecodeStats",
    "TelemetryNetworkStats",
    "TelemetrySecurityStats",
    "TelemetryEnergyStats",
    "TelemetryAuditOutcome",
    "TelemetryEncodeEvent",
    "TelemetryDecodeEvent",
    "TelemetryNetworkEvent",
    "TelemetrySecurityEvent",
    "TelemetryEnergyEvent",
    "TelemetryAuditOutcomeEvent",
    "TelemetryEvent",
    "KeyUpdate",
    "ContentKeyUpdate",
    "PrivacyRouteUpdate",
    "PrivacyRouteAckFrame",
    "ControlErrorFrame",
    "ControlFrameVariant",
    "ControlFrame",
    "X25519ChaCha20Poly1305Suite",
    "Kyber768XChaCha20Poly1305Suite",
    "EncryptionSuite",
    "hash_adapter",
    "signature_adapter",
    "bytes_adapter",
    "profile_id_adapter",
    "capability_flags_adapter",
    "ticket_capabilities_adapter",
    "privacy_capabilities_adapter",
    "hpke_suite_mask_adapter",
    "ticket_policy_adapter",
    "streaming_ticket_adapter",
    "ticket_revocation_adapter",
    "encryption_suite_adapter",
    "transport_capabilities_adapter",
    "stream_metadata_adapter",
    "privacy_relay_adapter",
    "privacy_route_adapter",
    "neural_bundle_adapter",
    "chunk_descriptor_adapter",
    "manifest_adapter",
    "manifest_announce_frame_adapter",
    "chunk_request_frame_adapter",
    "chunk_acknowledge_frame_adapter",
    "transport_capabilities_frame_adapter",
    "capability_report_adapter",
    "capability_ack_adapter",
    "audio_capability_adapter",
    "audio_frame_adapter",
    "resolution_adapter",
    "resolution_custom_adapter",
    "feedback_hint_frame_adapter",
    "sync_diagnostics_adapter",
    "receiver_report_adapter",
    "telemetry_encode_stats_adapter",
    "telemetry_decode_stats_adapter",
    "telemetry_network_stats_adapter",
    "telemetry_security_stats_adapter",
    "telemetry_energy_stats_adapter",
    "telemetry_event_adapter",
    "key_update_adapter",
    "content_key_update_adapter",
    "privacy_route_update_adapter",
    "privacy_route_ack_frame_adapter",
    "control_error_frame_adapter",
    "control_frame_adapter",
]
