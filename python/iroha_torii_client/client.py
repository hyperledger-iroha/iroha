"""Low-level Torii client for authenticated app APIs and authoritative evidence."""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import math
import re
import secrets
import time
import unicodedata
from dataclasses import dataclass
from decimal import Decimal
from typing import (
    Any,
    Callable,
    Dict,
    Iterable,
    List,
    Literal,
    Mapping,
    MutableMapping,
    Optional,
    Sequence,
    Tuple,
    TypedDict,
    Union,
    cast,
)
from urllib.parse import quote, urlencode, urlsplit

import requests
from blake3 import blake3

from . import connect_session as _connect_session, identifier_receipts as _identifier_receipts
from .attachment_client import authenticated_attachment_request
from .client_status_models import (
    SUMERAGI_EVIDENCE_EQUIVOCATION_CLASSES,
    SUMERAGI_EVIDENCE_KIND_FILTERS,
    SUMERAGI_EVIDENCE_PHASES,
    ConnectAdmissionManifest,
    ConnectAdmissionManifestEntry,
    ConnectAppPolicyControls,
    ConnectAppRecord,
    ConnectAppRegistryPage,
    ConnectPerIpSessions,
    ConnectSessionInfo,
    ConnectStatusPolicy,
    ConnectStatusSnapshot,
    ConfigurationSnapshot,
    ConfidentialGasSchedule,
    KaigiRelayDetail,
    KaigiRelayDomainMetrics,
    KaigiRelayHealthSnapshot,
    KaigiRelayReportedCall,
    KaigiRelaySummary,
    KaigiRelaySummaryList,
    LoggerConfig,
    NetworkConfig,
    PeerInfo,
    PeerTelemetryConfig,
    PeerTelemetryInfo,
    PeerTelemetryLocation,
    QueueConfig,
    SumeragiCensorshipEvidenceRecord,
    SumeragiDoubleVoteEvidenceRecord,
    SumeragiEvidenceListPage,
    SumeragiEvidenceRecord,
    SumeragiEvidenceRecordBase,
    SumeragiInvalidProposalEvidenceRecord,
    SumeragiInvalidQcEvidenceRecord,
    SumeragiLeaderSnapshot,
    SumeragiPacemakerSnapshot,
    SumeragiParamsSnapshot,
    SumeragiPhasesEma,
    SumeragiPhasesSnapshot,
    SumeragiPrfContext,
    SumeragiQcEntry,
    SumeragiQcSnapshot,
    SumeragiV2BlockSubject,
    SumeragiV2EquivocationEvidenceRecord,
    SumeragiV2ExecutionCommitment,
    SumeragiV2LaneFinalityManifestCommitment,
    SumeragiV2MergeCarrierCommitment,
    SumeragiV2QcReference,
    SumeragiV2Round,
    SumeragiV2TimeoutReference,
    StreamingSoranetConfig,
    StreamingTransportConfig,
    TransportConfig,
    TransportNoritoRpcConfig,
    _KAIGI_HEALTH_STATUSES,
    parse_sumeragi_json_object,
)
from .canonical_request_v1 import (
    CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1,
    CANONICAL_REQUEST_MAX_METHOD_BYTES_V1,
    CANONICAL_REQUEST_MAX_PATH_BYTES_V1,
    CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1,
    CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1,
    account_header_value as _canonical_account_header_value,
    canonical_query_string,
    require_account_literal as _require_canonical_account_literal,
    require_nonce as _require_canonical_nonce,
    require_signature_bytes as _require_canonical_signature_bytes,
    split_path_query as _split_path_query,
    validate_forwarded_witness_header as _validate_forwarded_witness_header,
    validate_target as _validate_canonical_request_target,
)
from .canonical_transport import (
    CanonicalRequestHeaderPlan as _CanonicalRequestHeaderPlan,
    OperatorRequestHeaderPlan as _OperatorRequestHeaderPlan,
    send_request as _send_request,
)
from .governance_ballot_client import create_governance_ballot_client_mixin
from .kaigi_relay_client import create_kaigi_relay_client_mixin
from .native_amx import (
    compute_native_amx_descriptor_hash,
    compute_native_amx_participant_settlement_hash,
    compute_native_amx_proposal_hash,
    compute_native_amx_validator_set_hash,
    validate_bls_normal_validator_set,
)
from .norito_frame import validate_norito_frame
from .orderbook_submission import (
    SorafsOrderbookSubmissionAmbiguousError,
    SorafsOrderbookSubmissionIdentity,
    SorafsOrderbookSubmissionMixin,
    SorafsOrderbookSubmissionReceipt,
    SorafsOrderbookSubmissionReceiptPayload,
    require_orderbook_chain_discriminant,
)
from .offline_models import (
    KagemushaArtifactBindingV4Json,
    OfflineAssetScale,
    OfflineAuthorizationJson,
    OfflineBranchClaimJson,
    OfflineBranchPathJson,
    OfflineLanePrivacyMerkleVariantJson,
    OfflineLanePrivacyMerkleWitnessJson,
    OfflineLanePrivacyProofJson,
    OfflineLanePrivacyWitnessJson,
    OfflineMerkleProofJson,
    OfflinePeerSplitTransitionJson,
    OfflinePeerSplitTransitionVariantJson,
    OfflineProofAttachmentJson,
    OfflineProofBackend,
    OfflineProofBoxJson,
    OfflineRecursiveSpendBundleJson,
    OfflineRecursiveSpendProofJson,
    OfflineRecursiveSpendStatementJson,
    OfflineRecursiveSpendTransitionJson,
    OfflineRedeemChangeJson,
    OfflineRedemptionChangeTransitionJson,
    OfflineRedemptionChangeTransitionVariantJson,
    OfflineRedemptionIntentJson,
    OfflineScaledAmountJson,
    OfflineSpendBranchJson,
    OfflineSpendableNoteJson,
    OfflineTopUpAnchorReferenceJson,
    OfflineTopUpShieldEvidenceJson,
    OfflineUnshieldPublicInputsJson,
    OfflineVerifiedFoldBundleJson,
    OfflineVerifiedFoldRecordBundleJson,
    OfflineVerifiedFoldStepJson,
    OfflineVerifiedFoldVerifierRecordJson,
    OfflineVerifierKeyIdJson,
    OfflineVerifierStatus,
    OfflineVerifyingKeyJson,
    OfflineVerifyingKeyRecordJson,
)
from .runtime_governance_auth import RuntimeGovernanceAuthMixin
from .sccp import (
    SccpBridgeSubmitResponse,
    SccpCapabilities,
    SccpRecentCursor,
    SccpRecentMessages,
    SccpRegistry,
    SccpRegistryLimits,
    SccpResourceLimits,
    normalize_bridge_message_submit_payload,
    normalize_bridge_proof_submit_payload,
    normalize_sccp_capabilities,
    normalize_sccp_message_bundle,
    normalize_sccp_proof_request,
    normalize_sccp_recent_messages,
    normalize_sccp_registry,
    parse_sccp_bridge_submit_response_json,
    parse_sccp_json_object,
)
from .space_directory_client import ToriiLocalSigningContext, create_space_directory_client_mixin
from .status_metrics import compute_status_metric_values as _compute_status_metric_values
from .subscription_auth import normalize_subscription_status, signed_subscription_post
from .subscription_models import SubscriptionActionResult, SubscriptionCreateResult
from .vpn_validation import (
    normalize_vpn_canonical_hex_input as _vpn_normalize_canonical_hex_input,
    parse_vpn_trust_fields as _vpn_parse_trust_fields,
    require_vpn_relay_endpoint as _vpn_require_relay_endpoint,
    require_vpn_relay_id as _vpn_require_relay_id,
    require_vpn_tls_server_name as _vpn_require_tls_server_name,
    require_vpn_trust_digest as _vpn_require_trust_digest,
)

# SCCP response limits apply to bytes yielded by Requests after transfer
# decoding. Content-Length remains an early rejection hint, never the sole
# authority, because it may be missing, dishonest, or describe encoded bytes.
_SCCP_CAPABILITIES_RESPONSE_MAX_BYTES = 64 * 1024
_SCCP_RECENT_RESPONSE_MAX_BYTES = 8 * 1024 * 1024
_SCCP_JSON_RESPONSE_MAX_BYTES = 64 * 1024 * 1024
_SCCP_SUBMIT_RESPONSE_MAX_BYTES = _SCCP_JSON_RESPONSE_MAX_BYTES
_SCCP_NATIVE_NORITO_RESPONSE_MAX_BYTES = 16 * 1024 * 1024
_SCCP_DESTINATION_NORITO_RESPONSE_MAX_BYTES = (
    _SCCP_NATIVE_NORITO_RESPONSE_MAX_BYTES + 64 * 1024
)
_SCCP_MESSAGE_BUNDLE_NORITO_TYPE_NAME = "iroha_sccp::TairaSccpMessageProofV1"
_SCCP_PROOF_REQUEST_NORITO_TYPE_NAME = (
    "iroha_sccp::SccpGroth16Bn254ProofRequestV1"
)

BASE58_ALPHABET = tuple("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz")
BASE58_INDEX = {symbol: idx for idx, symbol in enumerate(BASE58_ALPHABET)}
IROHA_POEM_KANA_HALFWIDTH = (
    "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ",
    "ﾚ", "ｿ", "ﾂ", "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ",
    "ｺ", "ｴ", "ﾃ", "ｱ", "ｻ", "ｷ", "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
)
I105_ALPHABET = BASE58_ALPHABET + IROHA_POEM_KANA_HALFWIDTH
I105_INDEX = {symbol: idx for idx, symbol in enumerate(I105_ALPHABET)}
I105_BASE = len(I105_ALPHABET)
I105_CHECKSUM_LEN = 6
I105_BECH32M_CONST = 0x2BC830A3
I105_SENTINELS = ("sora", "test", "dev")
I105_NUMERIC_SENTINEL_PREFIX = "n"
I105_DISCRIMINANT_MAX = 0xFFFF
I105_SENTINEL_DISCRIMINANTS = {"sora": 0x02F1, "test": 0x0171, "dev": 0}
I105_PROFILE_NAMES = {0x02F1: "minamoto", 0x0171: "taira", 0: "dev"}
_SORAFS_ORDERBOOK_QUERY_MAX_ITEMS = 500
_SORAFS_ORDERBOOK_ORDER_STATUS_VALUES = {
    "open",
    "partially_filled",
    "filled",
    "cancelled",
    "expired",
}
_SORAFS_ORDERBOOK_CHANNEL_STATUS_VALUES = {"open", "closed", "expired"}
_SORAFS_ORDERBOOK_EVENT_KIND_VALUES = {
    "policy_activated",
    "order_admitted",
    "order_cancelled",
    "trade_matched",
    "order_expired",
    "channel_expired",
    "receipt_recorded",
}
_SORAFS_XOR_QUANTITY_MAX_TEXT_LENGTH = 155
_QUANTITY_MAX_TEXT_LENGTH = 155
_QUANTITY_MAX_MANTISSA = (1 << 511) - 1
_VPN_HELPER_TICKET_BYTES = 664
_VPN_HELPER_TICKET_HEX_LENGTH = _VPN_HELPER_TICKET_BYTES * 2
_VPN_EXIT_CLASSES = frozenset({"standard", "low-latency", "high-security"})
_VPN_SESSION_STATUSES = frozenset({"active"})
_VPN_RECEIPT_STATUSES = frozenset({"disconnected", "expired", "replaced", "settled"})
_VPN_RECEIPT_SOURCES = frozenset({"torii", "relay", "wsv"})
_VPN_LEASE_SECONDS_MAX = (1 << 32) - 1
_VPN_UINT64_MAX = (1 << 64) - 1
_VPN_QUOTE_REQUEST_FIELDS = frozenset(
    {"exit_class", "exitClass", "metering_public_key_hex", "meteringPublicKeyHex"}
)
_VPN_SESSION_REQUEST_FIELDS = frozenset(
    {
        "exit_class",
        "exitClass",
        "quote_id",
        "quoteId",
        "payment_tx_hash",
        "paymentTxHash",
        "metering_public_key_hex",
        "meteringPublicKeyHex",
    }
)
_VPN_RECEIPT_REQUEST_FIELDS = frozenset(
    {
        "relay_receipt_hex",
        "relayReceiptHex",
        "client_voucher_hex",
        "clientVoucherHex",
        "lease_id_hex",
        "leaseIdHex",
    }
)
_VPN_TX_INSTRUCTION_FIELDS = frozenset({"wire_id", "payload_hex"})
_VPN_PROFILE_RESPONSE_FIELDS = frozenset(
    {
        "available",
        "relay_endpoint",
        "supported_exit_classes",
        "default_exit_class",
        "lease_secs",
        "dns_push_interval_secs",
        "meter_family",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "display_billing_label",
        "operator_account_id",
        "lease_fee",
        "settlement_grace_secs",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
    }
)
_VPN_QUOTE_RESPONSE_FIELDS = frozenset(
    {
        "quote_id",
        "lease_id_hex",
        "session_id_hex",
        "payment_reference",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "lease_secs",
        "quote_expires_at_ms",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "meter_family",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
        "metering_public_key_hex",
        "open_lease_instruction",
    }
)
_VPN_SESSION_RESPONSE_FIELDS = frozenset(
    {
        "session_id",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "lease_secs",
        "expires_at_ms",
        "connected_at_ms",
        "meter_family",
        "quote_id",
        "payment_reference",
        "payment_tx_hash",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "helper_ticket_hex",
        "bytes_in",
        "bytes_out",
        "status",
    }
)
_VPN_RECEIPT_RESPONSE_FIELDS = frozenset(
    {
        "session_id",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "meter_family",
        "connected_at_ms",
        "disconnected_at_ms",
        "duration_ms",
        "bytes_in",
        "bytes_out",
        "status",
        "receipt_source",
        "quote_id",
        "payment_tx_hash",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "earned_fee",
        "refunded_fee",
        "lease_id_hex",
        "settle_lease_instruction",
    }
)
_VPN_RECEIPT_LIST_RESPONSE_FIELDS = frozenset({"items", "total"})


def _canonical_quantity(value: Any, context: str) -> str:
    """Decode one canonical bounded non-negative Quantity JSON string."""

    if not isinstance(value, str):
        raise RuntimeError(f"{context} must be a quantity string")
    if len(value) > _QUANTITY_MAX_TEXT_LENGTH:
        raise RuntimeError(f"{context} quantity exceeds the text length bound")
    matched = re.fullmatch(r"(0|[1-9][0-9]*)(?:\.([0-9]{0,27}[1-9]))?", value)
    if matched is None:
        raise RuntimeError(f"{context} must be a canonical non-negative quantity")
    fraction = matched.group(2) or ""
    mantissa = int(matched.group(1) + fraction)
    if mantissa > _QUANTITY_MAX_MANTISSA:
        raise RuntimeError(f"{context} quantity exceeds the signed 512-bit domain")
    return value


_SORAFS_ORDERBOOK_STATUS_FIELDS = frozenset(
    {
        "open_orders",
        "partially_filled_orders",
        "filled_orders",
        "cancelled_orders",
        "expired_orders",
        "trades",
        "settlement_receipts",
        "settlement_channels",
        "open_settlement_channels",
        "book_revision",
        "next_admission_sequence",
        "next_trade_sequence",
        "updated_at_unix",
    }
)
_SORAFS_ORDERBOOK_FINALIZED_CURSOR_FIELDS = frozenset({"height", "block_hash"})
_SORAFS_ORDERBOOK_ORDER_PAGE_FIELDS = frozenset(
    {"finalized_cursor", "orders", "has_more", "next_after_order_id"}
)
_SORAFS_ORDERBOOK_ORDER_RECORD_FIELDS = frozenset(
    {
        "order_id",
        "owner",
        "canonical_order",
        "admitted_policy_digest",
        "admitted_at_unix",
        "admission_sequence",
        "remaining_gib",
        "status",
        "updated_at_unix",
        "canonical_cancel",
        "cancelled_at_unix",
        "cancelled_policy_digest",
    }
)
_SORAFS_ORDERBOOK_TRADE_PAGE_FIELDS = frozenset(
    {"finalized_cursor", "trades", "has_more", "next_after_trade_id"}
)
_SORAFS_ORDERBOOK_TRADE_RECORD_FIELDS = frozenset(
    {
        "trade_id",
        "maker_order_id",
        "taker_order_id",
        "trade_sequence",
        "canonical_trade",
        "channel_id",
        "book_revision",
        "recorded_at_unix",
    }
)
_SORAFS_ORDERBOOK_CHANNEL_PAGE_FIELDS = frozenset(
    {"finalized_cursor", "channels", "has_more", "next_after_channel_id"}
)
_SORAFS_ORDERBOOK_CHANNEL_RECORD_FIELDS = frozenset(
    {
        "channel_id",
        "trade_id",
        "buyer",
        "provider",
        "provider_id",
        "settlement_authority",
        "total_bytes",
        "remaining_bytes",
        "initial_xor_locked",
        "remaining_xor_locked",
        "status",
        "opened_at_unix",
        "expires_at_unix",
        "updated_at_unix",
    }
)
_SORAFS_ORDERBOOK_RECEIPT_PAGE_FIELDS = frozenset(
    {"finalized_cursor", "receipts", "has_more", "next_after_receipt_id"}
)
_SORAFS_ORDERBOOK_RECEIPT_RECORD_FIELDS = frozenset(
    {
        "receipt_id",
        "channel_id",
        "trade_id",
        "canonical_receipt",
        "admitted_policy_digest",
        "admitted_at_unix",
        "recorded_by",
    }
)
_SORAFS_ORDERBOOK_EVENT_CURSOR_FIELDS = frozenset(
    {"sequence", "block_height", "block_hash", "event_index"}
)
_SORAFS_ORDERBOOK_EVENT_PAGE_FIELDS = frozenset(
    {"finalized_cursor", "events", "has_more", "next_after"}
)
_SORAFS_ORDERBOOK_FINALIZED_EVENT_FIELDS = frozenset(
    {"sequence", "block_height", "block_hash", "event_index", "event"}
)
_SORAFS_ORDERBOOK_LEDGER_EVENT_FIELDS = frozenset(
    {
        "kind",
        "order_id",
        "trade_id",
        "channel_id",
        "receipt_id",
        "provider_id",
        "book_revision",
        "authority",
        "occurred_at_unix_ms",
    }
)
_SORAFS_ORDERBOOK_SUBMISSION_RECEIPT_FIELDS = frozenset({"payload", "signature"})
_SORAFS_ORDERBOOK_SUBMISSION_PAYLOAD_FIELDS = frozenset(
    {
        "tx_hash",
        "entrypoint_hash",
        "signed_transaction_hash",
        "submitted_at_ms",
        "submitted_at_height",
        "signer",
    }
)


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
    return _parse_i105_sentinel_and_payload(encoded)[2]


def _parse_i105_sentinel_and_payload(encoded: str) -> Tuple[str, int, str]:
    for sentinel in I105_SENTINELS:
        if encoded.startswith(sentinel):
            return sentinel, I105_SENTINEL_DISCRIMINANTS[sentinel], encoded[len(sentinel) :]
    if encoded.startswith(I105_NUMERIC_SENTINEL_PREFIX):
        index = len(I105_NUMERIC_SENTINEL_PREFIX)
        while index < len(encoded) and "0" <= encoded[index] <= "9":
            index += 1
        if index > len(I105_NUMERIC_SENTINEL_PREFIX):
            discriminant = int(encoded[len(I105_NUMERIC_SENTINEL_PREFIX):index])
            if discriminant > I105_DISCRIMINANT_MAX:
                raise ValueError(
                    "i105 chain discriminant must fit in an unsigned 16-bit integer"
                )
            return encoded[:index], discriminant, encoded[index:]
    raise ValueError("i105 address is missing the expected chain-discriminant sentinel")


def _decode_i105_string(encoded: str) -> bytes:
    payload = _strip_i105_sentinel(encoded)
    digits: List[int] = []
    for symbol in payload:
        try:
            digits.append(I105_INDEX[symbol])
        except KeyError as exc:
            raise ValueError("invalid character in i105 address") from exc
    if len(digits) <= I105_CHECKSUM_LEN:
        raise ValueError("i105 address too short")
    data_digits = digits[:-I105_CHECKSUM_LEN]
    checksum_digits = digits[-I105_CHECKSUM_LEN:]
    canonical = _decode_base_n(data_digits, I105_BASE)
    if checksum_digits != _i105_checksum_digits(canonical):
        raise ValueError("i105 checksum mismatch")
    return canonical


def _encode_i105_string(canonical: bytes, discriminant: int) -> str:
    """Render decoded address bytes with the one canonical I105 sentinel."""

    leading_zeroes = len(canonical) - len(canonical.lstrip(b"\x00"))
    value = int.from_bytes(canonical, "big")
    digits: List[int] = []
    while value:
        value, remainder = divmod(value, I105_BASE)
        digits.append(remainder)
    encoded_digits = [0] * leading_zeroes + list(reversed(digits))
    if not encoded_digits:
        encoded_digits = [0]

    sentinel = next(
        (
            name
            for name, known_discriminant in I105_SENTINEL_DISCRIMINANTS.items()
            if known_discriminant == discriminant
        ),
        f"{I105_NUMERIC_SENTINEL_PREFIX}{discriminant}",
    )
    return sentinel + "".join(
        I105_ALPHABET[digit]
        for digit in (*encoded_digits, *_i105_checksum_digits(canonical))
    )


def _decode_canonical_i105_string(encoded: str) -> bytes:
    """Parse an I105 literal and reject every non-canonical re-rendering."""

    _, discriminant, _ = _parse_i105_sentinel_and_payload(encoded)
    canonical = _decode_i105_string(encoded)
    if _encode_i105_string(canonical, discriminant) != encoded:
        raise ValueError("i105 address must use its exact canonical rendering")
    return canonical


@dataclass(frozen=True)
class I105NetworkPrefix:
    """Network prefix decoded from a canonical I105 account/address literal."""

    sentinel: str
    chain_discriminant: int
    profile: Optional[str]


def inspect_i105_network_prefix(
    encoded: str,
    *,
    expected_chain_discriminant: Optional[int] = None,
) -> I105NetworkPrefix:
    """Validate an I105 literal and report its strict network prefix.

    This helper intentionally does not convert between network prefixes. Account
    and address prefixes identify separate networks, so callers can inspect or
    enforce a prefix but must not silently rewrite it.
    """

    sentinel, discriminant, _ = _parse_i105_sentinel_and_payload(encoded)
    _decode_i105_string(encoded)
    if (
        expected_chain_discriminant is not None
        and discriminant != int(expected_chain_discriminant)
    ):
        raise ValueError(
            "i105 chain discriminant mismatch: "
            f"expected {int(expected_chain_discriminant)}, got {discriminant}"
        )
    return I105NetworkPrefix(
        sentinel=sentinel,
        chain_discriminant=discriminant,
        profile=I105_PROFILE_NAMES.get(discriminant),
    )

__all__ = [
    "ToriiClient",
    "SorafsOrderbookSubmissionAmbiguousError", "SorafsOrderbookSubmissionIdentity", "SorafsOrderbookSubmissionReceipt", "SorafsOrderbookSubmissionReceiptPayload",
    "decode_pdp_commitment_header",
    "inspect_i105_network_prefix",
    "I105NetworkPrefix",
    "CouncilMember",
    "CouncilCurrentStatus",
    "GovernanceProposalStatus",
    "GovernanceLockCustody",
    "GovernanceLockRecord",
    "GovernanceLocksOverview",
    "GovernanceReferendumStatus",
    "GovernanceTallySummary",
    "GovernanceUnlockStats",
    "TransactionInstruction",
    "GovernanceInstructionDraft",
    "GovernanceProposalDraft",
    "ContractOperationReceipt",
    "ContractCallResponse",
    "PipelineTransactionStatus",
    "PipelineTransactionStatusResponse",
    "MultisigResponse",
    "GovernanceContractResponse",
    "BallotSubmitResult",
    "ProtectedNamespacesApplyResult",
    "ProtectedNamespacesStatus",
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
    "SccpCapabilities",
    "SccpRegistryLimits",
    "SccpResourceLimits",
    "SccpRegistry",
    "SccpRecentCursor",
    "SccpRecentMessages",
    "SccpBridgeSubmitResponse",
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
    "DataspaceCatalogEntry",
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
    "SumeragiV2Round",
    "SumeragiV2BlockSubject",
    "SumeragiV2LaneFinalityManifestCommitment",
    "SumeragiV2MergeCarrierCommitment",
    "SumeragiV2ExecutionCommitment",
    "SumeragiV2QcReference",
    "SumeragiV2TimeoutReference",
    "SumeragiV2HeightContextStatus",
    "SumeragiV2CommitQcStatus",
    "SumeragiV2VoteQuorumStatus",
    "SumeragiV2TimeoutQuorumStatus",
    "SumeragiV2OutboundIntentStatus",
    "SumeragiV2WorkStatus",
    "SumeragiV2QueueLivenessStatus",
    "SumeragiV2ProgressTransitionStatus",
    "SumeragiV2IgnoreCount",
    "SumeragiV2LivenessStatus",
    "SumeragiV2AdapterQueueStatus",
    "SumeragiV2TxQueueStatus",
    "SumeragiV2OperatorStatus",
    "SumeragiSafetyHaltStatus",
    "SumeragiV2Status",
    "SumeragiPipelineExecutionStatus",
    "SumeragiNposDiagnostics",
    "SumeragiLaneCommitmentStatus",
    "SumeragiDataspaceCommitmentStatus",
    "SumeragiLaneGovernanceStatus",
    "SumeragiNativeAmxParticipantApplication",
    "SumeragiAutonomousLaneExecution",
    "SumeragiDiagnosticsStatus",
    "PipelinePreflightSumeragi",
    "PipelinePreflightAdmission",
    "PipelinePreflightBlock",
    "PipelinePreflightPipeline",
    "PipelinePreflightQueue",
    "PipelinePreflightFees",
    "PipelinePreflight",
    "SumeragiEvidenceRecordBase",
    "SumeragiDoubleVoteEvidenceRecord",
    "SumeragiInvalidQcEvidenceRecord",
    "SumeragiInvalidProposalEvidenceRecord",
    "SumeragiCensorshipEvidenceRecord",
    "SumeragiV2EquivocationEvidenceRecord",
    "SumeragiEvidenceRecord",
    "SumeragiEvidenceListPage",
    "KaigiRelaySummary",
    "KaigiRelaySummaryList",
    "KaigiRelayReportedCall",
    "KaigiRelayDomainMetrics",
    "KaigiRelayDetail",
    "KaigiRelayHealthSnapshot",
    "SumeragiQcEntry",
    "OfflineReadinessBlocker",
    "OfflineVerifierId",
    "OfflineActiveTransferVerifier",
    "OfflineActiveTopUpShieldVerifier",
    "OfflineActiveUnshieldVerifier",
    "OfflineActiveRecursiveStepEqVerifier",
    "OfflineActiveRecursiveStepEpVerifier",
    "OfflineAuthenticatedArtifactSet",
    "OfflineReadiness",
    "OfflineStatus",
    "OfflineAssetScale",
    "OfflineScaledAmountJson",
    "OfflineSpendableNoteJson",
    "OfflineAuthorizationJson",
    "OfflineVerifierKeyIdJson",
    "OfflineProofBoxJson",
    "OfflineVerifyingKeyJson",
    "OfflineProofBackend",
    "OfflineVerifierStatus",
    "OfflineVerifyingKeyRecordJson",
    "OfflineMerkleProofJson",
    "OfflineLanePrivacyMerkleWitnessJson",
    "OfflineLanePrivacyMerkleVariantJson",
    "OfflineLanePrivacyWitnessJson",
    "OfflineLanePrivacyProofJson",
    "OfflineVerifiedFoldStepJson",
    "OfflineVerifiedFoldBundleJson",
    "OfflineVerifiedFoldVerifierRecordJson",
    "OfflineVerifiedFoldRecordBundleJson",
    "OfflineProofAttachmentJson",
    "OfflineTopUpShieldEvidenceJson",
    "OfflineRecursiveSpendBundleJson",
    "OfflineTopUpAnchorReferenceJson",
    "OfflineBranchPathJson",
    "OfflineBranchClaimJson",
    "OfflineSpendBranchJson",
    "KagemushaArtifactBindingV4Json",
    "OfflinePeerSplitTransitionJson",
    "OfflineRedemptionChangeTransitionJson",
    "OfflinePeerSplitTransitionVariantJson",
    "OfflineRedemptionChangeTransitionVariantJson",
    "OfflineRecursiveSpendTransitionJson",
    "OfflineRecursiveSpendStatementJson",
    "OfflineRecursiveSpendProofJson",
    "OfflineUnshieldPublicInputsJson",
    "OfflineRedemptionIntentJson",
    "OfflineRedeemChangeJson",
    "KagemushaTopUpRequestV4",
    "KagemushaRedeemRequestV4",
    "OfflineOperationKind",
    "OfflinePendingState",
    "OfflineOperationReference",
    "OfflineScaledAmount",
    "OfflineSpendableNote",
    "OfflineVerifierKeyId",
    "KagemushaArtifactBindingV4",
    "OfflineTopUpAnchor",
    "OfflineTopUpFinalityProofAnchor",
    "OfflineTopUpFinalityConsensusMode",
    "OfflineTopUpFinalityPayloadEncoding",
    "OfflineTopUpFinalityGlobalPhase",
    "OfflineTopUpFinalityDataAvailabilityLayout",
    "OfflineTopUpFinalityHeightContextId",
    "OfflineTopUpFinalityConsensusRound",
    "OfflineTopUpFinalityBlockSubject",
    "OfflineTopUpFinalityMergeCarrierCommitment",
    "OfflineTopUpFinalityExecutionCommitment",
    "OfflineTopUpFinalityQuorumCertificate",
    "OfflineTopUpFinalityValidatorPower",
    "OfflineTopUpFinalityDualQuorum",
    "OfflineTopUpFinalityNextEpochSnapshot",
    "OfflineTopUpFinalitySnapshotBootstrapAnchor",
    "OfflineTopUpFinalityHeightContext",
    "OfflineTopUpFinalityCompactQc",
    "OfflineTopUpAnchorMerkleProof",
    "OfflineTopUpFinalityProof",
    "OfflineTopUpResult",
    "OfflineRedeemResult",
    "OfflineTopUpOperationResult",
    "OfflineRedeemOperationResult",
    "OfflineAppliedResult",
    "OfflineQueueErrorDetails",
    "OfflineAxtErrorDetails",
    "OfflineErrorDetails",
    "OfflineErrorEnvelope",
    "OfflinePendingOperation",
    "OfflineAppliedOperation",
    "OfflineRejectedOperation",
    "OfflineOperationStatus",
    "AppApiTransactionDraft",
    "SubscriptionPlanCreateResult",
    "SubscriptionPlanListItem",
    "SubscriptionPlanListPage",
    "SubscriptionCreateResult",
    "SubscriptionListItem",
    "SubscriptionListPage",
    "SubscriptionActionResult",
    "SubscriptionUsageDraft",
    "SumeragiQcSnapshot",
    "SumeragiPacemakerSnapshot",
    "SumeragiPhasesSnapshot",
    "SumeragiPhasesEma",
    "SumeragiPrfContext",
    "SumeragiLeaderSnapshot",
    "SumeragiParamsSnapshot",
    "ToriiCanonicalRequestAuth",
    "ToriiOperatorSigningContext",
    "ToriiLocalSigningContext",
    "CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1",
    "CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1",
    "CANONICAL_REQUEST_MAX_METHOD_BYTES_V1",
    "CANONICAL_REQUEST_MAX_PATH_BYTES_V1",
    "CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1",
    "canonical_query_string",
    "canonical_request_message",
    "canonical_network_request_signature_message",
    "operator_network_request_signature_message",
    "build_canonical_request_headers",
    "build_operator_request_headers",
    "encode_identifier_resolution_receipt_payload",
    "encode_identifier_resolution_receipt_attestation",
    "verify_identifier_resolution_receipt",
    "VpnQuoteCreateRequest",
    "VpnSessionCreateRequest",
    "VpnReceiptSubmitRequest",
    "VpnProfile",
    "VpnQuote",
    "VpnSession",
    "VpnReceipt",
    "VpnReceiptListResponse",
]

PDP_COMMITMENT_HEADER = "sora-pdp-commitment"
HEADER_ACCOUNT = "X-Iroha-Account"
HEADER_SIGNATURE = "X-Iroha-Signature"
HEADER_TIMESTAMP_MS = "X-Iroha-Timestamp-Ms"
HEADER_NONCE = "X-Iroha-Nonce"
HEADER_OPERATOR_PUBLIC_KEY = "X-Iroha-Operator-Public-Key"
HEADER_OPERATOR_TIMESTAMP_MS = "X-Iroha-Operator-Timestamp-Ms"
HEADER_OPERATOR_NONCE = "X-Iroha-Operator-Nonce"
HEADER_OPERATOR_SIGNATURE = "X-Iroha-Operator-Signature"
_OPERATOR_FORBIDDEN_AUTH_HEADERS = frozenset(
    {
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce",
        "x-iroha-witness",
        "x-iroha-operator-public-key",
        "x-iroha-operator-timestamp-ms",
        "x-iroha-operator-nonce",
        "x-iroha-operator-signature",
    }
)

def encode_identifier_resolution_receipt_payload(payload: Mapping[str, Any]) -> bytes:
    """Encode an identifier-resolution receipt payload with the shared canonical layout."""

    return _identifier_receipts.encode_identifier_resolution_receipt_payload(
        payload,
        decode_i105=_decode_i105_string,
    )


def encode_identifier_resolution_receipt_attestation(attestation: Mapping[str, Any]) -> bytes:
    """Encode an identifier-resolution receipt attestation with the shared canonical layout."""

    return _identifier_receipts.encode_identifier_resolution_receipt_attestation(attestation)


def verify_identifier_resolution_receipt(
    receipt: Mapping[str, Any],
    policy_summary: Mapping[str, Any],
) -> bool:
    """Verify a signed identifier-resolution receipt against a policy summary."""

    return _identifier_receipts.verify_identifier_resolution_receipt(
        receipt,
        policy_summary,
        decode_i105=_decode_i105_string,
    )


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
        lower_value = getter(lowered)
        if isinstance(lower_value, str):
            return lower_value
    try:
        items = headers.items()
    except AttributeError:
        return None
    for key, value in items:
        if isinstance(key, str) and key.lower() == lowered and isinstance(value, str):
            return value
    return None


def _canonical_body_bytes(body: Optional[Union[str, bytes, bytearray, memoryview]]) -> bytes:
    if body is None:
        return b""
    if isinstance(body, str):
        return body.encode("utf-8")
    if isinstance(body, (bytes, bytearray, memoryview)):
        return bytes(body)
    raise TypeError("canonical request body must be bytes-like or a string")


def _non_empty_string(value: Any) -> Optional[str]:
    if isinstance(value, str):
        trimmed = value.strip()
        return trimmed or None
    return None


def _error_body_message(value: Any) -> Optional[str]:
    if isinstance(value, str):
        return _non_empty_string(value)
    if isinstance(value, Sequence) and not isinstance(value, (bytes, bytearray, str)):
        for entry in value:
            nested = _error_body_message(entry)
            if nested:
                return nested
        return None
    if not isinstance(value, Mapping):
        return None
    for key in (
        "message",
        "error",
        "errors",
        "detail",
        "details",
        "reason",
        "rejection_reason",
        "description",
    ):
        for raw_key, nested_value in value.items():
            if isinstance(raw_key, str) and raw_key.lower() == key:
                nested = _error_body_message(nested_value)
                if nested:
                    return nested
    return None


def _error_body_reject_code(value: Any) -> Optional[str]:
    if isinstance(value, Sequence) and not isinstance(value, (bytes, bytearray, str)):
        for entry in value:
            nested = _error_body_reject_code(entry)
            if nested:
                return nested
        return None
    if not isinstance(value, Mapping):
        return None
    lowered = {
        raw_key.lower(): nested_value
        for raw_key, nested_value in value.items()
        if isinstance(raw_key, str)
    }
    direct = _non_empty_string(lowered.get("reject_code")) or _non_empty_string(
        lowered.get("rejectcode")
    )
    if direct:
        return direct
    details = lowered.get("details")
    if isinstance(details, Mapping):
        detail_values = {
            raw_key.lower(): nested_value
            for raw_key, nested_value in details.items()
            if isinstance(raw_key, str)
        }
        nested = _non_empty_string(detail_values.get("reject_code")) or _non_empty_string(
            detail_values.get("rejectcode")
        )
        if nested:
            return nested
        axt = detail_values.get("axt")
        if isinstance(axt, Mapping):
            for raw_key, nested_value in axt.items():
                if isinstance(raw_key, str) and raw_key.lower() == "code":
                    return _non_empty_string(nested_value)
    return None


def _format_error_body(text: str) -> str:
    stripped = text.strip()
    if not stripped:
        return ""
    try:
        payload = json.loads(stripped)
    except (TypeError, ValueError):
        return stripped
    message = _error_body_message(payload)
    reject_code = _error_body_reject_code(payload)
    if message and reject_code and reject_code not in message:
        return f"{message}; reject_code={reject_code}"
    if message:
        return message
    compact = json.dumps(payload, sort_keys=True, separators=(",", ":"))
    if reject_code:
        return f"{compact}; reject_code={reject_code}"
    return compact


def _read_bounded_sccp_response_body(
    response: requests.Response,
    maximum_body_bytes: int,
    context: str,
) -> bytes:
    """Drain one SCCP response through a strict actual-byte bound and close it."""

    if (
        isinstance(maximum_body_bytes, bool)
        or not isinstance(maximum_body_bytes, int)
        or maximum_body_bytes < 0
    ):
        raise ValueError(f"{context} response byte-size bound is invalid")

    try:
        raw_content_length = response.headers.get("Content-Length")
        if raw_content_length is not None:
            if not isinstance(raw_content_length, str) or re.fullmatch(
                r"(?:0|[1-9][0-9]*)", raw_content_length
            ) is None:
                raise ValueError(
                    f"{context} response Content-Length must be a canonical unsigned decimal integer"
                )
            maximum_literal = str(maximum_body_bytes)
            if len(raw_content_length) > len(maximum_literal) or (
                len(raw_content_length) == len(maximum_literal)
                and raw_content_length > maximum_literal
            ):
                raise ValueError(
                    f"{context} response exceeds its {maximum_body_bytes}-byte size bound"
                )

        body = bytearray()
        for chunk in response.iter_content(chunk_size=8192, decode_unicode=False):
            if not chunk:
                continue
            if not isinstance(chunk, (bytes, bytearray)):
                raise TypeError(f"{context} response body yielded a non-byte chunk")
            if len(chunk) > maximum_body_bytes - len(body):
                raise ValueError(
                    f"{context} response exceeds its {maximum_body_bytes}-byte size bound"
                )
            body.extend(chunk)
        return bytes(body)
    finally:
        response.close()


def canonical_request_message(
    method: str,
    path: str,
    body: Optional[Union[str, bytes, bytearray, memoryview]] = None,
) -> bytes:
    """Build the canonical request bytes accepted by Torii app endpoints.
    V1 method, path, query-byte, and query-pair limits are enforced before hashing.
    """

    path_part, query = _split_path_query(path)
    _validate_canonical_request_target(method, path_part)
    canonical_query = canonical_query_string(query)
    body_hash = hashlib.sha256(_canonical_body_bytes(body)).hexdigest()
    rendered = "\n".join(
        (
            method.upper(),
            path_part,
            canonical_query,
            body_hash,
        )
    )
    return rendered.encode("utf-8")


def canonical_network_request_signature_message(
    network_id: str,
    method: str,
    path: str,
    body: Optional[Union[str, bytes, bytearray, memoryview]] = None,
    *,
    timestamp_ms: int,
    nonce: str,
) -> bytes:
    """Build canonical freshness bytes bound to one exact genesis hash."""

    literal = _offline_hash_literal(network_id, "network_id")
    checked_timestamp = _require_u64(timestamp_ms, "timestamp_ms")
    checked_nonce = _require_canonical_nonce(nonce, "nonce")
    network_bytes = bytes.fromhex(literal[5:69])
    return b"".join(
        (
            b"iroha.app.request.network.v1\0",
            network_bytes,
            canonical_request_message(method, path, body),
            b"\n",
            str(checked_timestamp).encode("ascii"),
            b"\n",
            checked_nonce.encode("utf-8"),
        )
    )


@dataclass(frozen=True)
class ToriiCanonicalRequestAuth:
    """Signer configuration for app-facing Torii endpoints."""

    network_id: str
    account_id: str
    signer: Callable[[bytes], Union[bytes, bytearray, memoryview]]
    timestamp_ms: Optional[int] = None
    nonce: Optional[str] = None

    def __post_init__(self) -> None:
        _offline_hash_literal(self.network_id, "ToriiCanonicalRequestAuth.network_id")
        _require_canonical_account_literal(
            self.account_id,
            "ToriiCanonicalRequestAuth.account_id",
        )
        if not callable(self.signer):
            raise TypeError("ToriiCanonicalRequestAuth.signer must be callable")
        if self.timestamp_ms is not None:
            _require_u64(
                self.timestamp_ms,
                "ToriiCanonicalRequestAuth.timestamp_ms",
            )
        if self.nonce is not None:
            _require_canonical_nonce(
                self.nonce,
                "ToriiCanonicalRequestAuth.nonce",
            )


@dataclass(frozen=True)
class ToriiOperatorSigningContext:
    """Immutable signer pinned to one exact genesis-derived NetworkId."""

    network_id: str
    public_key: str
    signer: Callable[[bytes], Union[bytes, bytearray, memoryview]]

    def __post_init__(self) -> None:
        _offline_hash_literal(
            self.network_id,
            "ToriiOperatorSigningContext.network_id",
        )
        public_key = _require_exact_non_empty_string(
            self.public_key,
            "ToriiOperatorSigningContext.public_key",
        )
        if len(public_key) > 512 or re.fullmatch(r"[!-~]+", public_key) is None:
            raise ValueError(
                "ToriiOperatorSigningContext.public_key must be exact printable ASCII"
            )
        if not callable(self.signer):
            raise TypeError("ToriiOperatorSigningContext.signer must be callable")


def operator_network_request_signature_message(
    network_id: str,
    method: str,
    path: str,
    body: Optional[Union[str, bytes, bytearray, memoryview]] = None,
    *,
    timestamp_ms: int,
    nonce: str,
) -> bytes:
    """Build the exact NetworkId-bound operator request signature message."""

    literal = _offline_hash_literal(network_id, "network_id")
    checked_timestamp = _require_u64(timestamp_ms, "timestamp_ms")
    checked_nonce = _require_canonical_nonce(nonce, "nonce")
    return b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            bytes.fromhex(literal[5:69]),
            canonical_request_message(method, path, body),
            b"\n",
            str(checked_timestamp).encode("ascii"),
            b"\n",
            checked_nonce.encode("utf-8"),
        )
    )


def build_operator_request_headers(
    context: ToriiOperatorSigningContext,
    method: str,
    path: str,
    body: Optional[Union[str, bytes, bytearray, memoryview]] = None,
) -> Dict[str, str]:
    """Build one fresh operator signature quartet for Requests' wire target."""

    if not isinstance(context, ToriiOperatorSigningContext):
        raise TypeError("operator_signing_context must be ToriiOperatorSigningContext")
    timestamp_ms = _require_u64(int(time.time() * 1000), "timestamp_ms")
    nonce = secrets.token_hex(16)
    operator_network_request_signature_message(
        context.network_id,
        method,
        path,
        body,
        timestamp_ms=timestamp_ms,
        nonce=nonce,
    )
    prepared_target = requests.Request(
        method,
        f"https://canonical.invalid{path}",
    ).prepare().path_url
    message = operator_network_request_signature_message(
        context.network_id,
        method,
        prepared_target,
        body,
        timestamp_ms=timestamp_ms,
        nonce=nonce,
    )
    signature_bytes = _require_canonical_signature_bytes(
        context.signer(message), "operator signer"
    )
    return {
        HEADER_OPERATOR_PUBLIC_KEY: context.public_key,
        HEADER_OPERATOR_TIMESTAMP_MS: str(timestamp_ms),
        HEADER_OPERATOR_NONCE: nonce,
        HEADER_OPERATOR_SIGNATURE: base64.b64encode(signature_bytes).decode("ascii"),
    }


def build_canonical_request_headers(
    *,
    account_id: str,
    signer: Callable[[bytes], Union[bytes, bytearray, memoryview]],
    method: str,
    path: str,
    body: Optional[Union[str, bytes, bytearray, memoryview]] = None,
    network_id: str,
    timestamp_ms: Optional[int] = None,
    nonce: Optional[str] = None,
) -> Dict[str, str]:
    """Build exact-network headers for the target spelling Requests will send."""

    account = _require_canonical_account_literal(account_id, "account_id")
    if not callable(signer):
        raise TypeError("signer must be callable")
    effective_timestamp = _require_u64(
        timestamp_ms if timestamp_ms is not None else int(time.time() * 1000),
        "timestamp_ms",
    )
    effective_nonce = (
        _require_canonical_nonce(nonce, "nonce")
        if nonce is not None
        else secrets.token_hex(16)
    )
    # Validate the caller spelling before Requests can hide an unsafe dot
    # segment, then sign the platform-owned prepared target used on the wire.
    canonical_request_message(method, path, body)
    prepared_target = requests.Request(
        method,
        f"https://canonical.invalid{path}",
    ).prepare().path_url
    message = canonical_network_request_signature_message(
        network_id,
        method,
        prepared_target,
        body,
        timestamp_ms=effective_timestamp,
        nonce=effective_nonce,
    )
    signature = _require_canonical_signature_bytes(signer(message), "signer")
    return {
        HEADER_ACCOUNT: _canonical_account_header_value(account, _decode_canonical_i105_string),
        HEADER_SIGNATURE: base64.b64encode(signature).decode("ascii"),
        HEADER_TIMESTAMP_MS: str(effective_timestamp),
        HEADER_NONCE: effective_nonce,
    }


def _require_exact_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    stripped = value.strip()
    if not stripped:
        raise ValueError(f"{context} must be a non-empty string")
    if stripped != value:
        raise ValueError(f"{context} must not contain surrounding whitespace")
    return value


def _require_u64(value: Any, context: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{context} must be an unsigned 64-bit integer")
    if value < 0 or value > (1 << 64) - 1:
        raise ValueError(f"{context} must be an unsigned 64-bit integer")
    return value




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
class KagemushaTopUpRequestV4:
    """Canonical ABI-21/V4 Norito top-up request and operation identifier."""

    norito: bytes
    operation_id: str

    def __post_init__(self) -> None:
        _validate_kagemusha_norito_request(
            self.norito,
            _KAGEMUSHA_TOP_UP_MAX_NORITO_REQUEST_BYTES,
            "KagemushaTopUpRequestV4.norito",
        )
        object.__setattr__(self, "norito", bytes(self.norito))
        object.__setattr__(self, "operation_id", _require_offline_operation_id(self.operation_id))


@dataclass(frozen=True)
class KagemushaRedeemRequestV4:
    """Canonical ABI-21/V4 Norito redemption request and operation identifier."""

    norito: bytes
    operation_id: str

    def __post_init__(self) -> None:
        _validate_kagemusha_norito_request(
            self.norito,
            _KAGEMUSHA_REDEEM_MAX_NORITO_REQUEST_BYTES,
            "KagemushaRedeemRequestV4.norito",
        )
        object.__setattr__(self, "norito", bytes(self.norito))
        object.__setattr__(self, "operation_id", _require_offline_operation_id(self.operation_id))


_OFFLINE_READINESS_PATH = "/v1/offline/readiness"
_OFFLINE_TOP_UP_PATH = "/v1/offline/top-up"
_OFFLINE_REDEEM_PATH = "/v1/offline/redeem"
_OFFLINE_OPERATIONS_PATH = "/v1/offline/operations"
_OFFLINE_OPERATION_ID_RE = re.compile(r"^(?!0{64}$)[0-9a-f]{64}$")
_OFFLINE_TRANSACTION_HASH_RE = re.compile(r"^[0-9a-f]{64}$")
_OFFLINE_ERROR_CODE_RE = re.compile(r"^[a-z0-9][a-z0-9_]{0,63}$")
_OFFLINE_ASSET_DEFINITION_ID_RE = re.compile(r"^[1-9A-HJ-NP-Za-km-z]{28}$")
_OFFLINE_ASSET_ALIAS_RE = re.compile(
    r"^[a-z0-9]+(?:[._-][a-z0-9]+)*#[a-z0-9]+(?:-[a-z0-9]+)*(?:\.[a-z0-9]+(?:-[a-z0-9]+)*)?$"
)
_OFFLINE_MAX_U32 = (1 << 32) - 1
_OFFLINE_MAX_U64 = (1 << 64) - 1
_OFFLINE_MAX_U128 = (1 << 128) - 1
_OFFLINE_MAX_ASSET_SCALE = 28
_OFFLINE_TOP_UP_SHIELD_TREE_CAPACITY = 1 << 16
_OFFLINE_TOP_UP_SHIELD_MAX_PROOF_BYTES = 192 * 1024
_OFFLINE_TOP_UP_FINALITY_MAX_VALIDATORS = 4096
_OFFLINE_TOP_UP_FINALITY_MAX_ANCHORS_PER_BLOCK = 16
_OFFLINE_TOP_UP_FINALITY_MAX_SIBLINGS = 4
_OFFLINE_TOP_UP_FINALITY_PROOF_VERSION = 1
_OFFLINE_SUMERAGI_PROTOCOL_VERSION = 4
_SUMERAGI_MERGE_CARRIER_COMMITMENT_VERSION = 1
_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION = 1
_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES = 1024
_SUMERAGI_LANE_FINALITY_MANIFEST_MAX_LEAVES = 1024
_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT = (
    "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
)
_OFFLINE_BLS_PROOF_BYTES = 96
_OFFLINE_HASH_LITERAL_RE = re.compile(r"^hash:([0-9A-F]{64})#([0-9A-F]{4})$")
_OFFLINE_BLS_VALIDATOR_ID_RE = re.compile(r"^ea0130[0-9A-F]{96}$")
_OFFLINE_MAX_JSON_DEPTH = 128
_OFFLINE_MAX_JSON_RESPONSE_BYTES = 256 * 1024
_KAGEMUSHA_TOP_UP_MAX_NORITO_REQUEST_BYTES = 512 * 1024
_KAGEMUSHA_REDEEM_MAX_NORITO_REQUEST_BYTES = 48 * 1024 * 1024
_KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION = 22
_KAGEMUSHA_MAX_HOPS = 8
_KAGEMUSHA_CASH_HANDOFF_CAPABILITY = "cash_handoff_v1"
_KAGEMUSHA_RECURSIVE_PROOF_PAIR_MAX_BYTES_V4 = 16 * 1024 * 1024
_KAGEMUSHA_VERIFIER_BACKEND = "halo2/ipa"
_KAGEMUSHA_VERIFIER_ROLES = {
    "active_transfer_verifier": "confidential_transfer_v2_verifier_record",
    "active_topup_shield_verifier": "kagemusha_topup_shield_v2_verifier_record",
    "active_unshield_verifier": "confidential_unshield_v3_verifier_record",
    "active_recursive_step_eq_verifier": "kagemusha_recursive_step_eq_v4_verifier_record",
    "active_recursive_step_ep_verifier": "kagemusha_recursive_step_ep_v4_verifier_record",
}
_KAGEMUSHA_VERIFIER_CIRCUITS = {
    "active_transfer_verifier":
        "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
    "active_topup_shield_verifier":
        "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
    "active_unshield_verifier":
        "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
    "active_recursive_step_eq_verifier":
        "kagemusha-recursive-spend-step-eq-compact-layout-v5",
    "active_recursive_step_ep_verifier":
        "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
}


def _validate_kagemusha_norito_request(
    value: Any,
    maximum_bytes: int,
    context: str,
) -> None:
    if type(value) is not bytes:
        raise TypeError(f"{context} must be immutable bytes")
    if not value:
        raise ValueError(f"{context} must not be empty")
    if len(value) > maximum_bytes:
        raise ValueError(
            f"{context} exceeds {maximum_bytes} bytes"
        )


def _offline_exact_string(value: Any, context: str, *, non_empty: bool = True) -> str:
    if not isinstance(value, str):
        raise RuntimeError(f"{context} must be a string")
    if non_empty and not value:
        raise RuntimeError(f"{context} must not be empty")
    if value.strip() != value:
        raise RuntimeError(f"{context} must not contain surrounding whitespace")
    if any(0xD800 <= ord(character) <= 0xDFFF for character in value):
        raise RuntimeError(f"{context} must not contain Unicode surrogate code points")
    if any(ord(character) < 0x20 or 0x7F <= ord(character) <= 0x9F for character in value):
        raise RuntimeError(f"{context} must not contain control characters")
    return value


def _offline_asset_selector(value: Any, context: str) -> str:
    selector = _offline_exact_string(value, context)
    if "#" not in selector:
        return _offline_canonical_asset_definition_id(selector, context)
    if _OFFLINE_ASSET_ALIAS_RE.fullmatch(selector) is None:
        raise RuntimeError(
            f"{context} must be a canonical Base58 asset definition id or lowercase scoped asset alias"
        )
    return selector


def _offline_canonical_asset_definition_id(value: Any, context: str) -> str:
    asset_definition_id = _offline_exact_string(value, context)
    if _OFFLINE_ASSET_DEFINITION_ID_RE.fullmatch(asset_definition_id) is None:
        raise RuntimeError(
            f"{context} must be a canonical unprefixed Base58 asset definition id"
        )
    # Keep this complete validation synchronized with the normative Rust codec:
    # `iroha_data_model::asset::AssetDefinitionId::parse_address_literal`.
    payload = _decode_base_n(
        [BASE58_INDEX[symbol] for symbol in asset_definition_id],
        len(BASE58_ALPHABET),
    )
    uuid_bytes = payload[1:17]
    if (
        len(payload) != 21
        or payload[0] != 1
        or payload[17:] != blake3(payload[:17]).digest(length=4)
        or (uuid_bytes[6] >> 4) != 0b0100
        or (uuid_bytes[8] & 0b1100_0000) != 0b1000_0000
    ):
        raise RuntimeError(
            f"{context} must be a canonical checksummed UUIDv4 asset definition id"
        )
    return asset_definition_id


def _offline_required(mapping: Mapping[str, Any], field: str, context: str) -> Any:
    if field not in mapping:
        raise RuntimeError(f"{context}.{field} is required")
    return mapping[field]


def _offline_mapping(value: Any, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise RuntimeError(f"{context} must be an object")
    return value


def _offline_exact_object_fields(
    mapping: Mapping[str, Any],
    context: str,
    *,
    required: Sequence[str],
    optional: Sequence[str] = (),
) -> None:
    required_fields = set(required)
    allowed_fields = required_fields | set(optional)
    missing = required_fields - mapping.keys()
    if missing:
        field = min(missing)
        raise RuntimeError(f"{context}.{field} is required")
    unexpected = set(mapping) - allowed_fields
    if unexpected:
        field = min(unexpected)
        raise RuntimeError(f"{context}.{field} is not part of the first-release contract")


def _offline_unsigned(
    value: Any,
    context: str,
    maximum: int,
    *,
    positive: bool = False,
) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise RuntimeError(f"{context} must be an integer")
    if value < 0 or (positive and value == 0) or value > maximum:
        lower = 1 if positive else 0
        raise RuntimeError(f"{context} must be between {lower} and {maximum}")
    return value


def _snapshot_offline_json(
    value: Any,
    context: str,
    ancestors: Optional[set[int]] = None,
    depth: int = 0,
) -> Any:
    if depth > _OFFLINE_MAX_JSON_DEPTH:
        raise RuntimeError(f"{context} exceeds the maximum JSON nesting depth")
    if value is None or isinstance(value, (str, bool)):
        if isinstance(value, str):
            _offline_exact_string(value, context, non_empty=False)
        return value
    if isinstance(value, int):
        return _offline_unsigned(value, context, _OFFLINE_MAX_U128)
    if isinstance(value, Decimal):
        if not value.is_finite():
            raise RuntimeError(f"{context} must not contain non-finite numbers")
        return value
    if isinstance(value, float):
        raise RuntimeError(f"{context} must not contain floating-point numbers")

    active = ancestors if ancestors is not None else set()
    identity = id(value)
    if identity in active:
        raise RuntimeError(f"{context} must not contain a cycle")
    active.add(identity)
    try:
        if isinstance(value, Mapping):
            result: Dict[str, Any] = {}
            for key, item in value.items():
                if not isinstance(key, str):
                    raise RuntimeError(f"{context} keys must be strings")
                _offline_exact_string(key, f"{context} key", non_empty=False)
                result[key] = _snapshot_offline_json(
                    item,
                    f"{context}.{key}",
                    active,
                    depth + 1,
                )
            return result
        if isinstance(value, (list, tuple)):
            return [
                _snapshot_offline_json(item, f"{context}[{index}]", active, depth + 1)
                for index, item in enumerate(value)
            ]
    finally:
        active.remove(identity)
    raise RuntimeError(f"{context} contains an unsupported {type(value).__name__} value")


def _offline_json_object_without_duplicates(
    pairs: List[Tuple[str, Any]],
) -> Dict[str, Any]:
    result: Dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON object member `{key}`")
        result[key] = value
    return result


def _offline_reject_json_constant(token: str) -> Any:
    raise ValueError(f"non-finite JSON number `{token}` is not allowed")


def _offline_byte_array(value: Any, context: str, exact_length: Optional[int] = None) -> List[int]:
    if not isinstance(value, list):
        raise RuntimeError(f"{context} must be a JSON byte array")
    if exact_length is not None and len(value) != exact_length:
        raise RuntimeError(f"{context} must contain exactly {exact_length} bytes")
    for index, byte in enumerate(value):
        if isinstance(byte, bool) or not isinstance(byte, int) or not 0 <= byte <= 255:
            raise RuntimeError(f"{context}[{index}] must be an integer byte")
    return value


def _offline_operation_id_from_bytes(value: Any, context: str) -> str:
    raw = _offline_byte_array(value, context, 32)
    if not any(raw):
        raise RuntimeError(f"{context} must not be all zero")
    return bytes(raw).hex()


def _require_offline_operation_id(value: Any, context: str = "operation_id") -> str:
    if not isinstance(value, str) or _OFFLINE_OPERATION_ID_RE.fullmatch(value) is None:
        raise RuntimeError(
            f"{context} must be a non-zero lowercase 64-character hexadecimal string"
        )
    return value


def _offline_transaction_hash(value: Any, context: str) -> str:
    if not isinstance(value, str) or _OFFLINE_TRANSACTION_HASH_RE.fullmatch(value) is None:
        raise RuntimeError(f"{context} must be a lowercase 64-character hexadecimal string")
    return value


def _offline_crc16_ccitt_false(value: bytes) -> int:
    crc = 0xFFFF
    for byte in value:
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return crc


def _offline_hash_literal(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise RuntimeError(f"{context} must be a canonical Norito hash literal")
    match = _OFFLINE_HASH_LITERAL_RE.fullmatch(value)
    if match is None:
        raise RuntimeError(
            f"{context} must use canonical hash:<uppercase hex>#<CRC16> syntax"
        )
    body, checksum = match.groups()
    expected = _offline_crc16_ccitt_false(f"hash:{body}".encode("ascii"))
    if int(checksum, 16) != expected:
        raise RuntimeError(f"{context} hash checksum does not match its body")
    if int(body[-2:], 16) & 1 == 0:
        raise RuntimeError(f"{context} must set the Iroha hash marker bit")
    return value


def _offline_scaled_amount(value: Any, context: str) -> None:
    amount = _offline_mapping(value, context)
    _offline_unsigned(
        _offline_required(amount, "atomic_units", context),
        f"{context}.atomic_units",
        _OFFLINE_MAX_U128,
        positive=True,
    )
    _offline_unsigned(
        _offline_required(amount, "scale", context),
        f"{context}.scale",
        _OFFLINE_MAX_ASSET_SCALE,
    )


def _offline_top_up_shield_evidence_request(value: Any, context: str) -> None:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("initial_root", "finalized_root", "leaf_index", "proof"),
    )
    initial_root = _offline_fixed_bytes(
        _offline_required(record, "initial_root", context),
        f"{context}.initial_root",
        non_zero=True,
    )
    finalized_root = _offline_fixed_bytes(
        _offline_required(record, "finalized_root", context),
        f"{context}.finalized_root",
        non_zero=True,
    )
    if finalized_root == initial_root:
        raise RuntimeError(f"{context}.finalized_root must differ from initial_root")
    _offline_unsigned(
        _offline_required(record, "leaf_index", context),
        f"{context}.leaf_index",
        _OFFLINE_TOP_UP_SHIELD_TREE_CAPACITY - 1,
    )

    proof_context = f"{context}.proof"
    proof = _offline_mapping(_offline_required(record, "proof", context), proof_context)
    _offline_exact_object_fields(
        proof,
        proof_context,
        required=("backend", "proof", "vk_ref"),
        optional=("vk_commitment", "envelope_hash", "lane_privacy"),
    )
    backend = _offline_exact_string(
        _offline_required(proof, "backend", proof_context),
        f"{proof_context}.backend",
    )
    if len(backend.encode("utf-8")) > 256:
        raise RuntimeError(f"{proof_context}.backend must contain at most 256 UTF-8 bytes")

    proof_box_context = f"{proof_context}.proof"
    proof_box = _offline_mapping(
        _offline_required(proof, "proof", proof_context), proof_box_context
    )
    _offline_exact_object_fields(
        proof_box,
        proof_box_context,
        required=("backend", "bytes"),
    )
    proof_backend = _offline_exact_string(
        _offline_required(proof_box, "backend", proof_box_context),
        f"{proof_box_context}.backend",
    )
    if proof_backend != backend:
        raise RuntimeError(f"{proof_box_context}.backend must equal {proof_context}.backend")
    proof_bytes = _offline_byte_array(
        _offline_required(proof_box, "bytes", proof_box_context),
        f"{proof_box_context}.bytes",
    )
    if not 1 <= len(proof_bytes) <= _OFFLINE_TOP_UP_SHIELD_MAX_PROOF_BYTES:
        raise RuntimeError(
            f"{proof_box_context}.bytes must contain between 1 and "
            f"{_OFFLINE_TOP_UP_SHIELD_MAX_PROOF_BYTES} bytes"
        )
    _offline_verifier_key_id(
        _offline_required(proof, "vk_ref", proof_context), f"{proof_context}.vk_ref"
    )
    for field in ("vk_commitment", "envelope_hash"):
        if field in proof and proof[field] is not None:
            _offline_fixed_bytes(proof[field], f"{proof_context}.{field}", non_zero=True)
    if "lane_privacy" in proof and proof["lane_privacy"] is not None:
        _offline_mapping(proof["lane_privacy"], f"{proof_context}.lane_privacy")


@dataclass(frozen=True)
class OfflineReadinessBlocker:
    """Legacy command-specific proof-material diagnostic.

    This type never represents deployment, dataspace, or asset enrollment.
    """

    code: str
    message: str


@dataclass(frozen=True)
class OfflineVerifierId:
    """Stable registry identity of a verifier selected for Offline transfers."""

    backend: str
    name: str


@dataclass(frozen=True)
class OfflineActiveTransferVerifier:
    """Key-material-free transfer verifier active at the readiness snapshot."""

    id: OfflineVerifierId
    version: int
    circuit_id: str
    commitment: str
    public_inputs_schema_hash: str
    max_proof_bytes: int
    activation_height: int
    withdrawal_height: Optional[int]


@dataclass(frozen=True)
class OfflineAuthenticatedArtifactSet:
    """Exact authenticated ABI-21 V4 recursive release selected for readiness."""

    generation: str
    manifest_sha256: str
    release_policy_sha256: str
    release_attestation_sha256: str
    activation_height: int
    withdrawal_height: int
    max_proof_bytes: int
    asset_scale: int


# Every readiness role exposes the same key-material-free registry record
# shape. Distinct aliases keep role substitution visible at the API boundary.
OfflineActiveTopUpShieldVerifier = OfflineActiveTransferVerifier
OfflineActiveUnshieldVerifier = OfflineActiveTransferVerifier
OfflineActiveRecursiveStepEqVerifier = OfflineActiveTransferVerifier
OfflineActiveRecursiveStepEpVerifier = OfflineActiveTransferVerifier


@dataclass(frozen=True)
class OfflineStatus:
    """Universal, asset-neutral offline cash-handoff capability."""

    mandatory: bool
    cash_handoff_capability: str
    required_bridge_abi_version: int
    max_hops: int
    ready: bool
    assets: Tuple[()]
    blockers: Tuple[()]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OfflineStatus":
        """Decode the exact universally compiled capability projection."""

        context = "offline capability response"
        record = _offline_mapping(payload, context)
        _offline_exact_object_fields(
            record,
            context,
            required=(
                "mandatory",
                "cash_handoff_capability",
                "required_bridge_abi_version",
                "max_hops",
                "ready",
                "assets",
                "blockers",
            ),
        )
        mandatory = _offline_required(record, "mandatory", context)
        if mandatory is not False:
            raise RuntimeError(f"{context}.mandatory must be false")
        capability = _offline_exact_string(
            _offline_required(record, "cash_handoff_capability", context),
            f"{context}.cash_handoff_capability",
        )
        if capability != _KAGEMUSHA_CASH_HANDOFF_CAPABILITY:
            raise RuntimeError(
                f"{context}.cash_handoff_capability must be "
                f"{_KAGEMUSHA_CASH_HANDOFF_CAPABILITY}"
            )
        abi = _offline_unsigned(
            _offline_required(record, "required_bridge_abi_version", context),
            f"{context}.required_bridge_abi_version",
            _OFFLINE_MAX_U32,
            positive=True,
        )
        if abi != _KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION:
            raise RuntimeError(
                f"{context}.required_bridge_abi_version must be "
                f"{_KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION}"
            )
        max_hops = _offline_unsigned(
            _offline_required(record, "max_hops", context),
            f"{context}.max_hops",
            _OFFLINE_MAX_U32,
            positive=True,
        )
        if max_hops != _KAGEMUSHA_MAX_HOPS:
            raise RuntimeError(f"{context}.max_hops must be {_KAGEMUSHA_MAX_HOPS}")
        ready = _offline_required(record, "ready", context)
        if ready is not True:
            raise RuntimeError(f"{context}.ready must be true")
        assets = _offline_required(record, "assets", context)
        if not isinstance(assets, list) or assets:
            raise RuntimeError(f"{context}.assets must be an empty array")
        blockers = _offline_required(record, "blockers", context)
        if not isinstance(blockers, list) or blockers:
            raise RuntimeError(f"{context}.blockers must be an empty array")
        return cls(
            mandatory=False,
            cash_handoff_capability=_KAGEMUSHA_CASH_HANDOFF_CAPABILITY,
            required_bridge_abi_version=_KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION,
            max_hops=_KAGEMUSHA_MAX_HOPS,
            ready=True,
            assets=(),
            blockers=(),
        )


def _offline_active_transfer_verifier(
    value: Any,
    evaluated_block_height: int,
    context: str,
) -> OfflineActiveTransferVerifier:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "id",
            "version",
            "circuit_id",
            "commitment",
            "public_inputs_schema_hash",
            "max_proof_bytes",
            "activation_height",
            "withdrawal_height",
        ),
    )
    raw_id = _offline_mapping(_offline_required(record, "id", context), f"{context}.id")
    _offline_exact_object_fields(
        raw_id,
        f"{context}.id",
        required=("backend", "name"),
    )
    backend = _offline_exact_string(
        _offline_required(raw_id, "backend", f"{context}.id"),
        f"{context}.id.backend",
    )
    name = _offline_exact_string(
        _offline_required(raw_id, "name", f"{context}.id"),
        f"{context}.id.name",
    )
    if len(backend) > 256 or len(name) > 256:
        raise RuntimeError(f"{context}.id backend and name must not exceed 256 characters")
    version = _offline_unsigned(
        _offline_required(record, "version", context),
        f"{context}.version",
        _OFFLINE_MAX_U32,
        positive=True,
    )
    circuit_id = _offline_exact_string(
        _offline_required(record, "circuit_id", context),
        f"{context}.circuit_id",
    )
    commitment = _offline_transaction_hash(
        _offline_required(record, "commitment", context),
        f"{context}.commitment",
    )
    public_inputs_schema_hash = _offline_transaction_hash(
        _offline_required(record, "public_inputs_schema_hash", context),
        f"{context}.public_inputs_schema_hash",
    )
    if commitment == "0" * 64 or public_inputs_schema_hash == "0" * 64:
        raise RuntimeError(
            f"{context} commitment and public_inputs_schema_hash must be non-zero"
        )
    max_proof_bytes = _offline_unsigned(
        _offline_required(record, "max_proof_bytes", context),
        f"{context}.max_proof_bytes",
        _OFFLINE_MAX_U32,
        positive=True,
    )
    activation_height = _offline_unsigned(
        _offline_required(record, "activation_height", context),
        f"{context}.activation_height",
        _OFFLINE_MAX_U64,
    )
    raw_withdrawal_height = _offline_required(record, "withdrawal_height", context)
    withdrawal_height = (
        None
        if raw_withdrawal_height is None
        else _offline_unsigned(
            raw_withdrawal_height,
            f"{context}.withdrawal_height",
            _OFFLINE_MAX_U64,
            positive=True,
        )
    )
    if activation_height > evaluated_block_height:
        raise RuntimeError(f"{context}.activation_height is after the evaluated block")
    if withdrawal_height is not None and withdrawal_height <= evaluated_block_height:
        raise RuntimeError(f"{context}.withdrawal_height is not after the evaluated block")
    return OfflineActiveTransferVerifier(
        id=OfflineVerifierId(backend=backend, name=name),
        version=version,
        circuit_id=circuit_id,
        commitment=commitment,
        public_inputs_schema_hash=public_inputs_schema_hash,
        max_proof_bytes=max_proof_bytes,
        activation_height=activation_height,
        withdrawal_height=withdrawal_height,
    )


def _offline_authenticated_artifact_set(
    value: Any,
    evaluated_block_height: int,
    context: str,
) -> OfflineAuthenticatedArtifactSet:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "generation",
            "manifest_sha256",
            "release_policy_sha256",
            "release_attestation_sha256",
            "activation_height",
            "withdrawal_height",
            "max_proof_bytes",
            "asset_scale",
        ),
    )
    generation = _offline_exact_string(
        _offline_required(record, "generation", context),
        f"{context}.generation",
    )
    try:
        generation_bytes = generation.encode("ascii")
    except UnicodeEncodeError as exc:
        raise RuntimeError(
            f"{context}.generation must be a canonical portable artifact identifier"
        ) from exc
    if (
        len(generation_bytes) > 128
        or not generation_bytes[0:1].isalnum()
        or not generation_bytes[-1:].isalnum()
        or any(
            not (byte < 128 and (chr(byte).isalnum() or byte in b"._-"))
            for byte in generation_bytes
        )
    ):
        raise RuntimeError(
            f"{context}.generation must be a canonical portable artifact identifier"
        )
    basename = generation.split(".", 1)[0].lower()
    if basename in {"con", "prn", "aux", "nul"} or (
        len(basename) == 4
        and basename[:3] in {"com", "lpt"}
        and basename[3] in "123456789"
    ):
        raise RuntimeError(
            f"{context}.generation must not use a Windows reserved basename"
        )

    digests: Dict[str, str] = {}
    for field in (
        "manifest_sha256",
        "release_policy_sha256",
        "release_attestation_sha256",
    ):
        digest = _offline_required(record, field, context)
        if not isinstance(digest, str) or _OFFLINE_TRANSACTION_HASH_RE.fullmatch(digest) is None:
            raise RuntimeError(
                f"{context}.{field} must be a lowercase 64-character SHA-256 digest"
            )
        digests[field] = digest
    if any(digest == "0" * 64 for digest in digests.values()) or len(set(digests.values())) != 3:
        raise RuntimeError(f"{context} digests must be non-zero and pairwise distinct")

    activation_height = _offline_unsigned(
        _offline_required(record, "activation_height", context),
        f"{context}.activation_height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    withdrawal_height = _offline_unsigned(
        _offline_required(record, "withdrawal_height", context),
        f"{context}.withdrawal_height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if withdrawal_height <= activation_height:
        raise RuntimeError(f"{context}.withdrawal_height must be after activation_height")
    if activation_height > evaluated_block_height:
        raise RuntimeError(f"{context}.activation_height is after the evaluated block")
    if withdrawal_height <= evaluated_block_height:
        raise RuntimeError(f"{context}.withdrawal_height is not after the evaluated block")
    max_proof_bytes = _offline_unsigned(
        _offline_required(record, "max_proof_bytes", context),
        f"{context}.max_proof_bytes",
        _KAGEMUSHA_RECURSIVE_PROOF_PAIR_MAX_BYTES_V4,
        positive=True,
    )
    asset_scale = _offline_unsigned(
        _offline_required(record, "asset_scale", context),
        f"{context}.asset_scale",
        _OFFLINE_MAX_ASSET_SCALE,
    )
    return OfflineAuthenticatedArtifactSet(
        generation=generation,
        manifest_sha256=digests["manifest_sha256"],
        release_policy_sha256=digests["release_policy_sha256"],
        release_attestation_sha256=digests["release_attestation_sha256"],
        activation_height=activation_height,
        withdrawal_height=withdrawal_height,
        max_proof_bytes=max_proof_bytes,
        asset_scale=asset_scale,
    )


@dataclass(frozen=True)
class OfflineReadiness:
    """Legacy snapshot-bound proof-release diagnostics for one asset.

    Retained for historical wire compatibility and command diagnostics only.
    Capability discovery returns :class:`OfflineStatus`; this record must not
    be used to enroll or gate a deployment, dataspace, or asset.
    """

    required_bridge_abi_version: int
    max_hops: int
    asset_definition_id: str
    asset_scale: Optional[int]
    evaluated_block_height: int
    evaluated_block_hash: str
    active_transfer_verifier: Optional[OfflineActiveTransferVerifier]
    active_topup_shield_verifier: Optional[OfflineActiveTopUpShieldVerifier]
    active_unshield_verifier: Optional[OfflineActiveUnshieldVerifier]
    active_recursive_step_eq_verifier: Optional[OfflineActiveRecursiveStepEqVerifier]
    active_recursive_step_ep_verifier: Optional[OfflineActiveRecursiveStepEpVerifier]
    artifact_set: Optional[OfflineAuthenticatedArtifactSet]
    proof_backend_available: bool
    recursive_lineage_supported: bool
    ready: bool
    blockers: Tuple[OfflineReadinessBlocker, ...]

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
        requested_asset_selector: str,
    ) -> "OfflineReadiness":
        context = "offline readiness response"
        requested_selector = _offline_asset_selector(
            requested_asset_selector, "requested asset selector"
        )
        record = _offline_mapping(payload, context)
        _offline_exact_object_fields(
            record,
            context,
            required=(
                "required_bridge_abi_version",
                "max_hops",
                "asset_definition_id",
                "asset_scale",
                "evaluated_block_height",
                "evaluated_block_hash",
                "active_transfer_verifier",
                "active_topup_shield_verifier",
                "active_unshield_verifier",
                "active_recursive_step_eq_verifier",
                "active_recursive_step_ep_verifier",
                "artifact_set",
                "proof_backend_available",
                "recursive_lineage_supported",
                "ready",
                "blockers",
            ),
        )
        required_bridge_abi_version = _offline_unsigned(
            _offline_required(record, "required_bridge_abi_version", context),
            f"{context}.required_bridge_abi_version",
            _OFFLINE_MAX_U32,
            positive=True,
        )
        if required_bridge_abi_version != _KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION:
            raise RuntimeError(
                f"{context}.required_bridge_abi_version must be "
                f"{_KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION}"
            )
        max_hops = _offline_unsigned(
            _offline_required(record, "max_hops", context),
            f"{context}.max_hops",
            _OFFLINE_MAX_U32,
            positive=True,
        )
        if max_hops != _KAGEMUSHA_MAX_HOPS:
            raise RuntimeError(f"{context}.max_hops must be {_KAGEMUSHA_MAX_HOPS}")
        asset_definition_id = _offline_canonical_asset_definition_id(
            _offline_required(record, "asset_definition_id", context),
            f"{context}.asset_definition_id",
        )
        if "#" not in requested_selector and asset_definition_id != requested_selector:
            raise RuntimeError(
                f"{context}.asset_definition_id does not match the requested asset"
            )
        raw_asset_scale = _offline_required(record, "asset_scale", context)
        asset_scale = (
            None
            if raw_asset_scale is None
            else _offline_unsigned(
                raw_asset_scale,
                f"{context}.asset_scale",
                _OFFLINE_MAX_U32,
            )
        )
        evaluated_block_height = _offline_unsigned(
            _offline_required(record, "evaluated_block_height", context),
            f"{context}.evaluated_block_height",
            _OFFLINE_MAX_U64,
        )
        evaluated_block_hash = _offline_transaction_hash(
            _offline_required(record, "evaluated_block_hash", context),
            f"{context}.evaluated_block_hash",
        )
        raw_active_transfer_verifier = _offline_required(
            record, "active_transfer_verifier", context
        )
        active_transfer_verifier = (
            None
            if raw_active_transfer_verifier is None
            else _offline_active_transfer_verifier(
                raw_active_transfer_verifier,
                evaluated_block_height,
                f"{context}.active_transfer_verifier",
            )
        )
        raw_active_topup_shield_verifier = _offline_required(
            record, "active_topup_shield_verifier", context
        )
        active_topup_shield_verifier = (
            None
            if raw_active_topup_shield_verifier is None
            else _offline_active_transfer_verifier(
                raw_active_topup_shield_verifier,
                evaluated_block_height,
                f"{context}.active_topup_shield_verifier",
            )
        )
        parsed_verifiers: Dict[str, Optional[OfflineActiveTransferVerifier]] = {
            "active_transfer_verifier": active_transfer_verifier,
            "active_topup_shield_verifier": active_topup_shield_verifier,
        }
        for field in (
            "active_unshield_verifier",
            "active_recursive_step_eq_verifier",
            "active_recursive_step_ep_verifier",
        ):
            raw_verifier = _offline_required(record, field, context)
            parsed_verifiers[field] = (
                None
                if raw_verifier is None
                else _offline_active_transfer_verifier(
                    raw_verifier,
                    evaluated_block_height,
                    f"{context}.{field}",
                )
            )
        for field, verifier in parsed_verifiers.items():
            if verifier is None:
                continue
            if (
                verifier.id.backend != _KAGEMUSHA_VERIFIER_BACKEND
                or verifier.id.name != _KAGEMUSHA_VERIFIER_ROLES[field]
            ):
                raise RuntimeError(
                    f"{context}.{field}.id does not match its production Kagemusha role"
                )
            if verifier.circuit_id != _KAGEMUSHA_VERIFIER_CIRCUITS[field]:
                raise RuntimeError(
                    f"{context}.{field}.circuit_id does not match its Kagemusha role"
                )
        active_unshield_verifier = parsed_verifiers["active_unshield_verifier"]
        active_recursive_step_eq_verifier = parsed_verifiers[
            "active_recursive_step_eq_verifier"
        ]
        active_recursive_step_ep_verifier = parsed_verifiers[
            "active_recursive_step_ep_verifier"
        ]
        raw_artifact_set = _offline_required(record, "artifact_set", context)
        artifact_set = (
            None
            if raw_artifact_set is None
            else _offline_authenticated_artifact_set(
                raw_artifact_set,
                evaluated_block_height,
                f"{context}.artifact_set",
            )
        )
        active_records = [verifier for verifier in parsed_verifiers.values() if verifier is not None]
        if len({(record.id.backend, record.id.name) for record in active_records}) != len(active_records):
            raise RuntimeError(f"{context} must not reuse a verifier id across roles")
        if len({record.commitment for record in active_records}) != len(active_records):
            raise RuntimeError(f"{context} must not reuse a verifier commitment across roles")
        if len({record.public_inputs_schema_hash for record in active_records}) != len(
            active_records
        ):
            raise RuntimeError(
                f"{context} must not reuse a verifier public-input schema hash across roles"
            )
        recursive_verifiers = (
            active_recursive_step_eq_verifier,
            active_recursive_step_ep_verifier,
        )
        if (recursive_verifiers[0] is None) != (recursive_verifiers[1] is None):
            raise RuntimeError(
                f"{context} must report the ABI-21 V4 recursive verifier pair atomically"
            )
        if artifact_set is None:
            if any(verifier is not None for verifier in recursive_verifiers):
                raise RuntimeError(
                    f"{context}.artifact_set must accompany the ABI-21 V4 recursive verifier pair"
                )
        else:
            if any(verifier is None for verifier in recursive_verifiers):
                raise RuntimeError(
                    f"{context}.artifact_set requires the ABI-21 V4 recursive verifier pair"
                )
            if asset_scale != artifact_set.asset_scale:
                raise RuntimeError(
                    f"{context}.artifact_set.asset_scale must equal the authoritative asset scale"
                )
            for field, verifier in zip(
                ("active_recursive_step_eq_verifier", "active_recursive_step_ep_verifier"),
                recursive_verifiers,
            ):
                if verifier is None:
                    raise RuntimeError(
                        f"{context}.{field} is required with the ABI-21 V4 artifact set"
                    )
                if verifier.version == 0:
                    raise RuntimeError(f"{context}.{field}.version must be positive for ABI-21 V4")
                if verifier.max_proof_bytes != artifact_set.max_proof_bytes:
                    raise RuntimeError(
                        f"{context}.{field}.max_proof_bytes must equal artifact_set.max_proof_bytes"
                    )
                if verifier.activation_height != artifact_set.activation_height:
                    raise RuntimeError(
                        f"{context}.{field}.activation_height must equal artifact_set.activation_height"
                    )
                if verifier.withdrawal_height != artifact_set.withdrawal_height:
                    raise RuntimeError(
                        f"{context}.{field}.withdrawal_height must equal artifact_set.withdrawal_height"
                    )
        proof_backend_available = _offline_required(
            record, "proof_backend_available", context
        )
        if not isinstance(proof_backend_available, bool):
            raise RuntimeError(f"{context}.proof_backend_available must be a boolean")
        recursive_lineage_supported = _offline_required(
            record, "recursive_lineage_supported", context
        )
        if not isinstance(recursive_lineage_supported, bool):
            raise RuntimeError(f"{context}.recursive_lineage_supported must be a boolean")
        ready = _offline_required(record, "ready", context)
        if not isinstance(ready, bool):
            raise RuntimeError(f"{context}.ready must be a boolean")
        raw_blockers = _offline_required(record, "blockers", context)
        if not isinstance(raw_blockers, list):
            raise RuntimeError(f"{context}.blockers must be an array")
        blockers: List[OfflineReadinessBlocker] = []
        blocker_codes: set[str] = set()
        for index, raw in enumerate(raw_blockers):
            blocker_context = f"{context}.blockers[{index}]"
            blocker = _offline_mapping(raw, blocker_context)
            _offline_exact_object_fields(
                blocker,
                blocker_context,
                required=("code", "message"),
            )
            code = _offline_exact_string(
                _offline_required(blocker, "code", blocker_context),
                f"{blocker_context}.code",
            )
            if _OFFLINE_ERROR_CODE_RE.fullmatch(code) is None:
                raise RuntimeError(
                    f"{blocker_context}.code must be a stable lowercase code of 1 to 64 characters"
                )
            if code in blocker_codes:
                raise RuntimeError(f"{context}.blockers repeats blocker code {code}")
            blocker_codes.add(code)
            message = _offline_required(blocker, "message", blocker_context)
            if not isinstance(message, str):
                raise RuntimeError(f"{blocker_context}.message must be a string")
            _offline_exact_string(message, f"{blocker_context}.message")
            if len(message) > 1024:
                raise RuntimeError(
                    f"{blocker_context}.message must not exceed 1024 Unicode characters"
                )
            blockers.append(OfflineReadinessBlocker(code=code, message=message))
        recursive_registry_codes = blocker_codes & {
            "recursive_v4_registry_unavailable",
            "recursive_v4_registry_malformed",
        }
        if artifact_set is None:
            if len(recursive_registry_codes) != 1:
                raise RuntimeError(
                    f"{context}.artifact_set null requires exactly one ABI-21 V4 registry blocker"
                )
            if proof_backend_available:
                raise RuntimeError(
                    f"{context}.proof_backend_available requires an authenticated artifact_set"
                )
        elif recursive_registry_codes:
            raise RuntimeError(
                f"{context}.artifact_set contradicts the ABI-21 V4 registry blocker set"
            )
        if ("asset_scale_unavailable" in blocker_codes) != (asset_scale is None):
            raise RuntimeError(
                f"{context}.asset_scale_unavailable must be present exactly when asset_scale is null"
            )
        if ("asset_scale_unsupported" in blocker_codes) != (
            asset_scale is not None and asset_scale > _OFFLINE_MAX_ASSET_SCALE
        ):
            raise RuntimeError(
                f"{context}.asset_scale_unsupported must be present exactly when asset_scale exceeds 28"
            )
        if ("transfer_verifier_unavailable" in blocker_codes) != (
            active_transfer_verifier is None
        ):
            raise RuntimeError(
                f"{context}.transfer_verifier_unavailable must be present exactly when no active verifier is reported"
            )
        if ("topup_shield_verifier_unavailable" in blocker_codes) != (
            active_topup_shield_verifier is None
        ):
            raise RuntimeError(
                f"{context}.topup_shield_verifier_unavailable must be present exactly when no active top-up shield verifier is reported"
            )
        for field, blocker_code in (
            ("active_unshield_verifier", "unshield_verifier_unavailable"),
            (
                "active_recursive_step_eq_verifier",
                "recursive_step_eq_verifier_unavailable",
            ),
            ("active_recursive_step_ep_verifier", "recursive_step_ep_verifier_unavailable"),
        ):
            if (blocker_code in blocker_codes) != (parsed_verifiers[field] is None):
                raise RuntimeError(
                    f"{context}.{blocker_code} must be present exactly when {field} is null"
                )
        if ("proof_backend_unavailable" in blocker_codes) == proof_backend_available:
            raise RuntimeError(
                f"{context}.proof_backend_available contradicts the blocker set"
            )
        expected_recursive_lineage_supported = (
            proof_backend_available
            and artifact_set is not None
            and active_recursive_step_eq_verifier is not None
            and active_recursive_step_ep_verifier is not None
        )
        if recursive_lineage_supported != expected_recursive_lineage_supported:
            raise RuntimeError(
                f"{context}.recursive_lineage_supported must equal the exact authenticated ABI-21 lineage conjunction"
            )
        lineage_blocked = "recursive_lineage_unavailable" in blocker_codes
        if lineage_blocked == recursive_lineage_supported:
            raise RuntimeError(
                f"{context}.recursive_lineage_supported contradicts the blocker set"
            )
        expected_ready = (
            proof_backend_available
            and recursive_lineage_supported
            and artifact_set is not None
            and asset_scale is not None
            and asset_scale <= _OFFLINE_MAX_ASSET_SCALE
            and active_transfer_verifier is not None
            and active_topup_shield_verifier is not None
            and active_unshield_verifier is not None
            and active_recursive_step_eq_verifier is not None
            and active_recursive_step_ep_verifier is not None
            and not blockers
        )
        if ready != expected_ready:
            raise RuntimeError(
                f"{context}.ready must equal the complete ABI-21 runtime conjunction"
            )
        return cls(
            required_bridge_abi_version=required_bridge_abi_version,
            max_hops=max_hops,
            asset_definition_id=asset_definition_id,
            asset_scale=asset_scale,
            evaluated_block_height=evaluated_block_height,
            evaluated_block_hash=evaluated_block_hash,
            active_transfer_verifier=active_transfer_verifier,
            active_topup_shield_verifier=active_topup_shield_verifier,
            active_unshield_verifier=active_unshield_verifier,
            active_recursive_step_eq_verifier=active_recursive_step_eq_verifier,
            active_recursive_step_ep_verifier=active_recursive_step_ep_verifier,
            artifact_set=artifact_set,
            proof_backend_available=proof_backend_available,
            recursive_lineage_supported=recursive_lineage_supported,
            ready=ready,
            blockers=tuple(blockers),
        )


@dataclass(frozen=True)
class OfflineOperationKind:
    """Tagged Offline command kind from the public JSON contract."""

    kind: Literal["top_up", "redeem"]
    value: None = None


@dataclass(frozen=True)
class OfflinePendingState:
    """Tagged initial state returned by a successful command submission."""

    state: Literal["pending"] = "pending"
    value: None = None


@dataclass(frozen=True)
class OfflineOperationReference:
    """Reference returned by an accepted asynchronous Offline command."""

    operation_id: str
    kind: OfflineOperationKind
    state: OfflinePendingState
    transaction_hash: str
    status_uri: str
    submitted_at_ms: int


@dataclass(frozen=True)
class OfflineScaledAmount:
    """Lossless positive amount at the authoritative Offline asset scale."""

    atomic_units: int
    scale: OfflineAssetScale


@dataclass(frozen=True)
class OfflineSpendableNote:
    """Typed note descriptor embedded in a finalized top-up anchor."""

    network_id: str
    asset: str
    note_commitment: Tuple[int, ...]
    spend_nullifier: Tuple[int, ...]
    amount: OfflineScaledAmount


@dataclass(frozen=True)
class OfflineVerifierKeyId:
    """Backend and registry name of a verifier selected at finalization."""

    backend: str
    name: str


@dataclass(frozen=True)
class KagemushaArtifactBindingV4:
    """Content-addressed ABI-21/V4 recursive proof release."""

    version: Literal[4]
    generation: str
    manifest_sha256: Tuple[int, ...]


@dataclass(frozen=True)
class OfflineTopUpAnchor:
    """Closed, cross-checked finalized receipt returned by an applied top-up."""

    version: Literal[4]
    network_id: str
    payer: str
    asset: str
    asset_scale: OfflineAssetScale
    amount: OfflineScaledAmount
    initial_root: Tuple[int, ...]
    finalized_root: Tuple[int, ...]
    shield_leaf_index: int
    current_note: OfflineSpendableNote
    topup_operation_id: Tuple[int, ...]
    shield_verifier_id: OfflineVerifierKeyId
    shield_verifier_commitment: Tuple[int, ...]
    artifact_binding: KagemushaArtifactBindingV4
    finalized_height: int
    finalized_tx_hash: Tuple[int, ...]
    anchor_digest: Tuple[int, ...]


@dataclass(frozen=True)
class OfflineTopUpFinalityProofAnchor:
    """Exact top-up identity authenticated by a finality proof."""

    topup_operation_id: Tuple[int, ...]
    anchor_digest: Tuple[int, ...]


@dataclass(frozen=True)
class OfflineTopUpFinalityConsensusMode:
    """Adjacent-tag Sumeragi-v2 consensus mode."""

    mode: Literal["permissioned", "npos"]
    details: None = None


@dataclass(frozen=True)
class OfflineTopUpFinalityPayloadEncoding:
    """Adjacent-tag data-availability payload encoding."""

    encoding: Literal["reed_solomon16"]
    details: None = None


@dataclass(frozen=True)
class OfflineTopUpFinalityGlobalPhase:
    """Adjacent-tag Sumeragi-v2 voting phase."""

    phase: Literal["prepare", "commit"]
    details: None = None


@dataclass(frozen=True)
class OfflineTopUpFinalityDataAvailabilityLayout:
    """Frozen data-availability layout in a finality height context."""

    encoding: OfflineTopUpFinalityPayloadEncoding
    chunk_size_bytes: int
    data_shards: int
    parity_shards: int
    max_payload_size_bytes: int
    max_chunk_count: int


@dataclass(frozen=True)
class OfflineTopUpFinalityHeightContextId:
    """Typed hash of one complete immutable Sumeragi-v2 height context."""

    hash: str


@dataclass(frozen=True)
class OfflineTopUpFinalityConsensusRound:
    """Height-context-bound Sumeragi-v2 round."""

    context_id: OfflineTopUpFinalityHeightContextId
    height: int
    view: int


@dataclass(frozen=True)
class OfflineTopUpFinalityBlockSubject:
    """Exact parent, block, and payload hashes certified by a QC."""

    parent_block_hash: Optional[str]
    block_hash: str
    payload_hash: str


@dataclass(frozen=True)
class OfflineTopUpFinalityMergeCarrierCommitment:
    """Exact merge-ledger entry identity authenticated by finality."""

    version: Literal[1]
    entry_hash: str


@dataclass(frozen=True)
class OfflineTopUpFinalityExecutionCommitment:
    """Deterministic state transition authenticated by a QC."""

    parent_state_root: str
    post_state_root: str
    ordinary_writes_root: str
    topup_anchor_root: Optional[str]
    topup_anchor_count: int
    native_amx_application_manifest_version: int
    native_amx_application_manifest_root: str
    native_amx_application_manifest_count: int
    lane_finality_manifest: Optional[SumeragiV2LaneFinalityManifestCommitment]
    merge_carrier: Optional[OfflineTopUpFinalityMergeCarrierCommitment]
    executed_block_wire_len: int
    executed_block_wire_hash: str


@dataclass(frozen=True)
class OfflineTopUpFinalityQuorumCertificate:
    """Closed structural representation of a Sumeragi-v2 QC."""

    round: OfflineTopUpFinalityConsensusRound
    proposal_round: OfflineTopUpFinalityConsensusRound
    phase: OfflineTopUpFinalityGlobalPhase
    subject: OfflineTopUpFinalityBlockSubject
    execution_commitment: OfflineTopUpFinalityExecutionCommitment
    signers: Tuple[int, ...]
    aggregate_signature: Tuple[int, ...]


@dataclass(frozen=True)
class OfflineTopUpFinalityValidatorPower:
    """One BLS validator identity and its frozen positive voting power."""

    validator: str
    power: int


@dataclass(frozen=True)
class OfflineTopUpFinalityDualQuorum:
    """Canonical count-and-power quorum derived from a frozen roster."""

    min_signers: int
    total_power: int


@dataclass(frozen=True)
class OfflineTopUpFinalityNextEpochSnapshot:
    """Parent-authenticated complete next-epoch election snapshot."""

    epoch: int
    epoch_end_height: int
    mode: OfflineTopUpFinalityConsensusMode
    roster: Tuple[OfflineTopUpFinalityValidatorPower, ...]
    validator_set_pops: Tuple[Tuple[int, ...], ...]
    quorum: OfflineTopUpFinalityDualQuorum
    leader_seed: Tuple[int, ...]


@dataclass(frozen=True)
class OfflineTopUpFinalitySnapshotBootstrapAnchor:
    """Audited snapshot authority replacing an unavailable parent CommitQC."""

    snapshot_height: int
    snapshot_block_hash: str
    snapshot_block_creation_time_ms: int
    snapshot_state_hash: str


@dataclass(frozen=True)
class OfflineTopUpFinalityHeightContext:
    """Bounded projection of the immutable finality height context."""

    context_id: OfflineTopUpFinalityHeightContextId
    network_id: str
    protocol_version: Literal[4]
    height: int
    epoch: int
    epoch_end_height: int
    next_epoch_snapshot: Optional[OfflineTopUpFinalityNextEpochSnapshot]
    mode: OfflineTopUpFinalityConsensusMode
    parent_commit_qc: Optional[OfflineTopUpFinalityQuorumCertificate]
    snapshot_bootstrap: Optional[OfflineTopUpFinalitySnapshotBootstrapAnchor]
    nexus_amx_context_hash: str
    execution_policy_hash: str
    da_layout: OfflineTopUpFinalityDataAvailabilityLayout
    leader_seed: Tuple[int, ...]


@dataclass(frozen=True)
class OfflineTopUpFinalityCompactQc:
    """Projected height context and its exact persisted Commit certificate."""

    height_context: OfflineTopUpFinalityHeightContext
    certificate: OfflineTopUpFinalityQuorumCertificate


@dataclass(frozen=True)
class OfflineTopUpAnchorMerkleProof:
    """Canonical balanced-Merkle inclusion path for one top-up anchor."""

    leaf_index: int
    leaf_count: int
    siblings: Tuple[Tuple[int, ...], ...]


@dataclass(frozen=True)
class OfflineTopUpFinalityProof:
    """Closed typed Sumeragi-v2 finality proof for one applied top-up."""

    version: Literal[1]
    anchor: OfflineTopUpFinalityProofAnchor
    commit_qc: OfflineTopUpFinalityCompactQc
    anchor_path: OfflineTopUpAnchorMerkleProof


@dataclass(frozen=True)
class OfflineTopUpResult:
    """Terminal result of an applied top-up."""

    transaction_hash: str
    finalized_block_height: int
    server_time_ms: int
    anchor: OfflineTopUpAnchor
    finality_proof: OfflineTopUpFinalityProof


@dataclass(frozen=True)
class OfflineRedeemResult:
    """Terminal result of an applied redemption."""

    transaction_hash: str
    finalized_block_height: int
    server_time_ms: int


@dataclass(frozen=True)
class OfflineTopUpOperationResult:
    """Tagged applied top-up result."""

    result: OfflineTopUpResult
    kind: Literal["top_up"] = "top_up"


@dataclass(frozen=True)
class OfflineRedeemOperationResult:
    """Tagged applied redemption result."""

    result: OfflineRedeemResult
    kind: Literal["redeem"] = "redeem"


OfflineAppliedResult = Union[OfflineTopUpOperationResult, OfflineRedeemOperationResult]


@dataclass(frozen=True)
class OfflineQueueErrorDetails:
    """Queue-pressure metadata attached to an Offline rejection."""

    state: str
    queued: int
    capacity: int
    saturated: bool


@dataclass(frozen=True)
class OfflineAxtErrorDetails:
    """Closed AXT policy metadata attached to an Offline rejection."""

    code: Optional[str] = None
    reason: Optional[str] = None
    snapshot_version: Optional[int] = None
    dataspace: Optional[int] = None
    lane: Optional[int] = None
    active_handle_era: Optional[int] = None
    next_handle_counter: Optional[int] = None


@dataclass(frozen=True)
class OfflineErrorDetails:
    """Closed structured metadata carried by an Offline error envelope."""

    layer: Optional[str] = None
    reject_code: Optional[str] = None
    queue: Optional[OfflineQueueErrorDetails] = None
    retry_after_seconds: Optional[int] = None
    endpoint: Optional[str] = None
    field: Optional[str] = None
    expected: Optional[str] = None
    actual: Optional[str] = None
    profile: Optional[str] = None
    chain_discriminant: Optional[int] = None
    tx_hash: Optional[str] = None
    last_status: Optional[str] = None
    hint: Optional[str] = None
    axt: Optional[OfflineAxtErrorDetails] = None


@dataclass(frozen=True)
class OfflineErrorEnvelope:
    """Stable typed error attached to a rejected Offline operation."""

    code: str
    message: str
    details: Optional[OfflineErrorDetails] = None


@dataclass(frozen=True)
class OfflinePendingOperation:
    """Non-terminal Offline operation state."""

    operation_id: str
    kind: OfflineOperationKind
    transaction_hash: str
    submitted_at_ms: int
    state: Literal["pending"] = "pending"


@dataclass(frozen=True)
class OfflineAppliedOperation:
    """Applied terminal Offline operation state."""

    operation_id: str
    result: OfflineAppliedResult
    state: Literal["applied"] = "applied"


@dataclass(frozen=True)
class OfflineRejectedOperation:
    """Rejected terminal Offline operation state."""

    operation_id: str
    kind: OfflineOperationKind
    transaction_hash: str
    error: OfflineErrorEnvelope
    state: Literal["rejected"] = "rejected"


OfflineOperationStatus = Union[
    OfflinePendingOperation,
    OfflineAppliedOperation,
    OfflineRejectedOperation,
]


def _offline_operation_kind(value: Any, context: str) -> OfflineOperationKind:
    record = _offline_mapping(value, context)
    kind = _offline_required(record, "kind", context)
    if kind not in ("top_up", "redeem"):
        raise RuntimeError(f"{context}.kind must be top_up or redeem")
    if "value" in record and record["value"] is not None:
        raise RuntimeError(f"{context}.value must be null when present")
    return OfflineOperationKind(kind=kind)


def _offline_status_uri(operation_id: str) -> str:
    return f"{_OFFLINE_OPERATIONS_PATH}/{operation_id}"


def _offline_operation_reference(
    payload: Mapping[str, Any],
    *,
    expected_operation_id: str,
    expected_kind: Literal["top_up", "redeem"],
    location: Optional[str],
) -> OfflineOperationReference:
    context = "offline operation reference"
    record = _offline_mapping(payload, context)
    operation_id = _require_offline_operation_id(
        _offline_required(record, "operation_id", context), f"{context}.operation_id"
    )
    if operation_id != expected_operation_id:
        raise RuntimeError(f"{context}.operation_id does not match the submitted request")
    kind = _offline_operation_kind(_offline_required(record, "kind", context), f"{context}.kind")
    if kind.kind != expected_kind:
        raise RuntimeError(f"{context}.kind does not match the submitted command")
    raw_state = _offline_mapping(_offline_required(record, "state", context), f"{context}.state")
    if _offline_required(raw_state, "state", f"{context}.state") != "pending":
        raise RuntimeError(f"{context}.state.state must be pending")
    if "value" in raw_state and raw_state["value"] is not None:
        raise RuntimeError(f"{context}.state.value must be null when present")
    status_uri = _offline_required(record, "status_uri", context)
    expected_uri = _offline_status_uri(operation_id)
    if status_uri != expected_uri:
        raise RuntimeError(f"{context}.status_uri must equal {expected_uri}")
    if location != expected_uri:
        raise RuntimeError(f"Location header must equal {expected_uri}")
    return OfflineOperationReference(
        operation_id=operation_id,
        kind=kind,
        state=OfflinePendingState(),
        transaction_hash=_offline_transaction_hash(
            _offline_required(record, "transaction_hash", context),
            f"{context}.transaction_hash",
        ),
        status_uri=status_uri,
        submitted_at_ms=_offline_unsigned(
            _offline_required(record, "submitted_at_ms", context),
            f"{context}.submitted_at_ms",
            _OFFLINE_MAX_U64,
        ),
    )


def _offline_optional_error_string(
    record: Mapping[str, Any], field: str, context: str
) -> Optional[str]:
    value = record.get(field)
    if value is None:
        return None
    if not isinstance(value, str):
        raise RuntimeError(f"{context}.{field} must be a string")
    return _offline_exact_string(value, f"{context}.{field}", non_empty=False)


def _offline_optional_error_unsigned(
    record: Mapping[str, Any], field: str, context: str, maximum: int
) -> Optional[int]:
    value = record.get(field)
    if value is None:
        return None
    return _offline_unsigned(value, f"{context}.{field}", maximum)


def _offline_queue_error_details(value: Any, context: str) -> OfflineQueueErrorDetails:
    record = _offline_mapping(value, context)
    state = _offline_exact_string(
        _offline_required(record, "state", context), f"{context}.state"
    )
    saturated = _offline_required(record, "saturated", context)
    if type(saturated) is not bool:
        raise RuntimeError(f"{context}.saturated must be a boolean")
    return OfflineQueueErrorDetails(
        state=state,
        queued=_offline_unsigned(
            _offline_required(record, "queued", context),
            f"{context}.queued",
            _OFFLINE_MAX_U64,
        ),
        capacity=_offline_unsigned(
            _offline_required(record, "capacity", context),
            f"{context}.capacity",
            _OFFLINE_MAX_U64,
        ),
        saturated=saturated,
    )


def _offline_axt_error_details(value: Any, context: str) -> OfflineAxtErrorDetails:
    record = _offline_mapping(value, context)
    retired_fields = {"next_min_handle_era", "next_min_sub_nonce"}.intersection(record)
    if retired_fields:
        retired = ", ".join(sorted(retired_fields))
        raise RuntimeError(f"{context} uses retired AXT fields: {retired}")
    return OfflineAxtErrorDetails(
        code=_offline_optional_error_string(record, "code", context),
        reason=_offline_optional_error_string(record, "reason", context),
        snapshot_version=_offline_optional_error_unsigned(
            record, "snapshot_version", context, _OFFLINE_MAX_U64
        ),
        dataspace=_offline_optional_error_unsigned(
            record, "dataspace", context, _OFFLINE_MAX_U64
        ),
        lane=_offline_optional_error_unsigned(record, "lane", context, _OFFLINE_MAX_U32),
        active_handle_era=_offline_optional_error_unsigned(
            record, "active_handle_era", context, _OFFLINE_MAX_U64
        ),
        next_handle_counter=_offline_optional_error_unsigned(
            record, "next_handle_counter", context, _OFFLINE_MAX_U64
        ),
    )


def _offline_error_details(value: Any, context: str) -> OfflineErrorDetails:
    record = _offline_mapping(value, context)
    queue = None
    if record.get("queue") is not None:
        queue = _offline_queue_error_details(record["queue"], f"{context}.queue")
    axt = None
    if record.get("axt") is not None:
        axt = _offline_axt_error_details(record["axt"], f"{context}.axt")
    return OfflineErrorDetails(
        layer=_offline_optional_error_string(record, "layer", context),
        reject_code=_offline_optional_error_string(record, "reject_code", context),
        queue=queue,
        retry_after_seconds=_offline_optional_error_unsigned(
            record, "retry_after_seconds", context, _OFFLINE_MAX_U64
        ),
        endpoint=_offline_optional_error_string(record, "endpoint", context),
        field=_offline_optional_error_string(record, "field", context),
        expected=_offline_optional_error_string(record, "expected", context),
        actual=_offline_optional_error_string(record, "actual", context),
        profile=_offline_optional_error_string(record, "profile", context),
        chain_discriminant=_offline_optional_error_unsigned(
            record, "chain_discriminant", context, (1 << 16) - 1
        ),
        tx_hash=_offline_optional_error_string(record, "tx_hash", context),
        last_status=_offline_optional_error_string(record, "last_status", context),
        hint=_offline_optional_error_string(record, "hint", context),
        axt=axt,
    )


def _offline_error(value: Any, context: str) -> OfflineErrorEnvelope:
    record = _offline_mapping(value, context)
    code = _offline_exact_string(
        _offline_required(record, "code", context), f"{context}.code"
    )
    if _OFFLINE_ERROR_CODE_RE.fullmatch(code) is None:
        raise RuntimeError(
            f"{context}.code must be a stable lowercase code of 1 to 64 characters"
        )
    message = _offline_exact_string(
        _offline_required(record, "message", context), f"{context}.message"
    )
    details = None
    if record.get("details") is not None:
        details = _offline_error_details(record["details"], f"{context}.details")
    return OfflineErrorEnvelope(code=code, message=message, details=details)


def _offline_fixed_bytes(
    value: Any,
    context: str,
    *,
    non_zero: bool = False,
) -> Tuple[int, ...]:
    raw = _offline_byte_array(value, context, 32)
    if non_zero and not any(raw):
        raise RuntimeError(f"{context} must not be all zero")
    return tuple(raw)


def _offline_scaled_amount_model(value: Any, context: str) -> OfflineScaledAmount:
    record = _offline_mapping(value, context)
    return OfflineScaledAmount(
        atomic_units=_offline_unsigned(
            _offline_required(record, "atomic_units", context),
            f"{context}.atomic_units",
            _OFFLINE_MAX_U128,
            positive=True,
        ),
        scale=cast(
            OfflineAssetScale,
            _offline_unsigned(
                _offline_required(record, "scale", context),
                f"{context}.scale",
                _OFFLINE_MAX_ASSET_SCALE,
            ),
        ),
    )


def _offline_spendable_note(value: Any, context: str) -> OfflineSpendableNote:
    record = _offline_mapping(value, context)
    note_commitment = _offline_fixed_bytes(
        _offline_required(record, "note_commitment", context),
        f"{context}.note_commitment",
        non_zero=True,
    )
    spend_nullifier = _offline_fixed_bytes(
        _offline_required(record, "spend_nullifier", context),
        f"{context}.spend_nullifier",
        non_zero=True,
    )
    if spend_nullifier == note_commitment:
        raise RuntimeError(f"{context}.spend_nullifier must differ from note_commitment")
    return OfflineSpendableNote(
        network_id=_offline_hash_literal(
            _offline_required(record, "network_id", context), f"{context}.network_id"
        ),
        asset=_offline_exact_string(
            _offline_required(record, "asset", context), f"{context}.asset"
        ),
        note_commitment=note_commitment,
        spend_nullifier=spend_nullifier,
        amount=_offline_scaled_amount_model(
            _offline_required(record, "amount", context), f"{context}.amount"
        ),
    )


def _offline_verifier_key_id(value: Any, context: str) -> OfflineVerifierKeyId:
    record = _offline_mapping(value, context)
    backend = _offline_exact_string(
        _offline_required(record, "backend", context), f"{context}.backend"
    )
    name = _offline_exact_string(
        _offline_required(record, "name", context), f"{context}.name"
    )
    if len(backend.encode("utf-8")) > 256:
        raise RuntimeError(f"{context}.backend must contain at most 256 UTF-8 bytes")
    if len(name.encode("utf-8")) > 256:
        raise RuntimeError(f"{context}.name must contain at most 256 UTF-8 bytes")
    return OfflineVerifierKeyId(
        backend=backend,
        name=name,
    )


def _offline_top_up_finality_height_context_id(
    value: Any, context: str
) -> OfflineTopUpFinalityHeightContextId:
    if not isinstance(value, list) or len(value) != 1:
        raise RuntimeError(f"{context} must be a one-element typed-hash array")
    return OfflineTopUpFinalityHeightContextId(
        hash=_offline_hash_literal(value[0], f"{context}[0]")
    )


def _offline_top_up_finality_consensus_mode(
    value: Any, context: str
) -> OfflineTopUpFinalityConsensusMode:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("mode", "details"),
    )
    mode = _offline_required(record, "mode", context)
    if mode not in ("permissioned", "npos"):
        raise RuntimeError(f"{context}.mode must be permissioned or npos")
    if _offline_required(record, "details", context) is not None:
        raise RuntimeError(f"{context}.details must be null for a unit variant")
    return OfflineTopUpFinalityConsensusMode(mode=mode, details=None)


def _offline_top_up_finality_payload_encoding(
    value: Any, context: str
) -> OfflineTopUpFinalityPayloadEncoding:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("encoding", "details"),
    )
    encoding = _offline_required(record, "encoding", context)
    if encoding != "reed_solomon16":
        raise RuntimeError(f"{context}.encoding must be reed_solomon16")
    if _offline_required(record, "details", context) is not None:
        raise RuntimeError(f"{context}.details must be null for a unit variant")
    return OfflineTopUpFinalityPayloadEncoding(encoding=encoding, details=None)


def _offline_top_up_finality_phase(
    value: Any, context: str
) -> OfflineTopUpFinalityGlobalPhase:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("phase", "details"),
    )
    phase = _offline_required(record, "phase", context)
    if phase not in ("prepare", "commit"):
        raise RuntimeError(f"{context}.phase must be prepare or commit")
    if _offline_required(record, "details", context) is not None:
        raise RuntimeError(f"{context}.details must be null for a unit variant")
    return OfflineTopUpFinalityGlobalPhase(phase=phase, details=None)


def _offline_top_up_finality_da_layout(
    value: Any, context: str
) -> OfflineTopUpFinalityDataAvailabilityLayout:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "encoding",
            "chunk_size_bytes",
            "data_shards",
            "parity_shards",
            "max_payload_size_bytes",
            "max_chunk_count",
        ),
    )
    encoding = _offline_top_up_finality_payload_encoding(
        _offline_required(record, "encoding", context), f"{context}.encoding"
    )
    chunk_size_bytes = _offline_unsigned(
        _offline_required(record, "chunk_size_bytes", context),
        f"{context}.chunk_size_bytes",
        _OFFLINE_MAX_U32,
        positive=True,
    )
    data_shards = _offline_unsigned(
        _offline_required(record, "data_shards", context),
        f"{context}.data_shards",
        (1 << 16) - 1,
        positive=True,
    )
    parity_shards = _offline_unsigned(
        _offline_required(record, "parity_shards", context),
        f"{context}.parity_shards",
        (1 << 16) - 1,
        positive=True,
    )
    return OfflineTopUpFinalityDataAvailabilityLayout(
        encoding=encoding,
        chunk_size_bytes=chunk_size_bytes,
        data_shards=data_shards,
        parity_shards=parity_shards,
        max_payload_size_bytes=_offline_unsigned(
            _offline_required(record, "max_payload_size_bytes", context),
            f"{context}.max_payload_size_bytes",
            _OFFLINE_MAX_U64,
            positive=True,
        ),
        max_chunk_count=_offline_unsigned(
            _offline_required(record, "max_chunk_count", context),
            f"{context}.max_chunk_count",
            _OFFLINE_MAX_U32,
            positive=True,
        ),
    )


def _offline_top_up_finality_round(
    value: Any, context: str
) -> OfflineTopUpFinalityConsensusRound:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("context_id", "height", "view"),
    )
    return OfflineTopUpFinalityConsensusRound(
        context_id=_offline_top_up_finality_height_context_id(
            _offline_required(record, "context_id", context), f"{context}.context_id"
        ),
        height=_offline_unsigned(
            _offline_required(record, "height", context),
            f"{context}.height",
            _OFFLINE_MAX_U64,
            positive=True,
        ),
        view=_offline_unsigned(
            _offline_required(record, "view", context),
            f"{context}.view",
            _OFFLINE_MAX_U64,
        ),
    )


def _offline_top_up_finality_subject(
    value: Any,
    context: str,
    *,
    round_height: int,
) -> OfflineTopUpFinalityBlockSubject:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("block_hash", "payload_hash"),
        optional=("parent_block_hash",),
    )
    raw_parent = record.get("parent_block_hash")
    parent_block_hash = (
        None
        if raw_parent is None
        else _offline_hash_literal(raw_parent, f"{context}.parent_block_hash")
    )
    if (round_height == 1) != (parent_block_hash is None):
        raise RuntimeError(
            f"{context}.parent_block_hash must be absent only at genesis height"
        )
    return OfflineTopUpFinalityBlockSubject(
        parent_block_hash=parent_block_hash,
        block_hash=_offline_hash_literal(
            _offline_required(record, "block_hash", context), f"{context}.block_hash"
        ),
        payload_hash=_offline_hash_literal(
            _offline_required(record, "payload_hash", context), f"{context}.payload_hash"
        ),
    )


def _lane_finality_manifest_commitment(
    value: Any, context: str
) -> Optional[SumeragiV2LaneFinalityManifestCommitment]:
    if value is None:
        return None
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(record, context, required=("root", "leaf_count"))
    return SumeragiV2LaneFinalityManifestCommitment(
        root=_offline_hash_literal(
            _offline_required(record, "root", context), f"{context}.root"
        ),
        leaf_count=_offline_unsigned(
            _offline_required(record, "leaf_count", context),
            f"{context}.leaf_count",
            _SUMERAGI_LANE_FINALITY_MANIFEST_MAX_LEAVES,
            positive=True,
        ),
    )


def _offline_top_up_finality_execution_commitment(
    value: Any,
    context: str,
    *,
    require_topup: bool,
) -> OfflineTopUpFinalityExecutionCommitment:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "parent_state_root",
            "post_state_root",
            "ordinary_writes_root",
            "topup_anchor_count",
            "native_amx_application_manifest_version",
            "native_amx_application_manifest_root",
            "native_amx_application_manifest_count",
            "lane_finality_manifest",
            "merge_carrier",
            "executed_block_wire_len",
            "executed_block_wire_hash",
        ),
        optional=("topup_anchor_root",),
    )
    topup_anchor_count = _offline_unsigned(
        _offline_required(record, "topup_anchor_count", context),
        f"{context}.topup_anchor_count",
        _OFFLINE_TOP_UP_FINALITY_MAX_ANCHORS_PER_BLOCK,
    )
    raw_topup_root = record.get("topup_anchor_root")
    topup_anchor_root = (
        None
        if raw_topup_root is None
        else _offline_hash_literal(raw_topup_root, f"{context}.topup_anchor_root")
    )
    if (topup_anchor_count == 0) != (topup_anchor_root is None):
        raise RuntimeError(
            f"{context}.topup_anchor_root must be present exactly when topup_anchor_count is positive"
        )
    if require_topup and topup_anchor_count == 0:
        raise RuntimeError(
            f"{context}.topup_anchor_count must be positive for a top-up finality proof"
        )
    native_manifest_version = _offline_unsigned(
        _offline_required(
            record, "native_amx_application_manifest_version", context
        ),
        f"{context}.native_amx_application_manifest_version",
        (1 << 16) - 1,
    )
    if native_manifest_version != _SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION:
        raise RuntimeError(
            f"{context}.native_amx_application_manifest_version must equal "
            f"{_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION}"
        )
    native_manifest_root = _offline_hash_literal(
        _offline_required(record, "native_amx_application_manifest_root", context),
        f"{context}.native_amx_application_manifest_root",
    )
    native_manifest_count = _offline_unsigned(
        _offline_required(record, "native_amx_application_manifest_count", context),
        f"{context}.native_amx_application_manifest_count",
        _SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES,
    )
    if (native_manifest_count == 0) != (
        native_manifest_root
        == _SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT
    ):
        raise RuntimeError(
            f"{context}.native_amx_application_manifest_count must be zero exactly "
            "for the canonical empty root"
        )
    lane_finality_manifest = _lane_finality_manifest_commitment(
        _offline_required(record, "lane_finality_manifest", context),
        f"{context}.lane_finality_manifest",
    )
    raw_merge_carrier = _offline_required(record, "merge_carrier", context)
    merge_carrier: Optional[OfflineTopUpFinalityMergeCarrierCommitment]
    if raw_merge_carrier is None:
        merge_carrier = None
    else:
        merge_context = f"{context}.merge_carrier"
        merge_record = _offline_mapping(raw_merge_carrier, merge_context)
        _offline_exact_object_fields(
            merge_record,
            merge_context,
            required=("version", "entry_hash"),
        )
        merge_version = _offline_unsigned(
            _offline_required(merge_record, "version", merge_context),
            f"{merge_context}.version",
            (1 << 16) - 1,
        )
        if merge_version != _SUMERAGI_MERGE_CARRIER_COMMITMENT_VERSION:
            raise RuntimeError(
                f"{merge_context}.version must be "
                f"{_SUMERAGI_MERGE_CARRIER_COMMITMENT_VERSION}"
            )
        merge_carrier = OfflineTopUpFinalityMergeCarrierCommitment(
            version=1,
            entry_hash=_offline_hash_literal(
                _offline_required(merge_record, "entry_hash", merge_context),
                f"{merge_context}.entry_hash",
            ),
        )
    return OfflineTopUpFinalityExecutionCommitment(
        parent_state_root=_offline_hash_literal(
            _offline_required(record, "parent_state_root", context),
            f"{context}.parent_state_root",
        ),
        post_state_root=_offline_hash_literal(
            _offline_required(record, "post_state_root", context),
            f"{context}.post_state_root",
        ),
        ordinary_writes_root=_offline_hash_literal(
            _offline_required(record, "ordinary_writes_root", context),
            f"{context}.ordinary_writes_root",
        ),
        topup_anchor_root=topup_anchor_root,
        topup_anchor_count=topup_anchor_count,
        native_amx_application_manifest_version=native_manifest_version,
        native_amx_application_manifest_root=native_manifest_root,
        native_amx_application_manifest_count=native_manifest_count,
        lane_finality_manifest=lane_finality_manifest,
        merge_carrier=merge_carrier,
        executed_block_wire_len=_offline_unsigned(
            _offline_required(record, "executed_block_wire_len", context),
            f"{context}.executed_block_wire_len",
            _OFFLINE_MAX_U64,
            positive=True,
        ),
        executed_block_wire_hash=_offline_hash_literal(
            _offline_required(record, "executed_block_wire_hash", context),
            f"{context}.executed_block_wire_hash",
        ),
    )


def _offline_top_up_finality_qc(
    value: Any,
    context: str,
    *,
    require_topup: bool,
) -> OfflineTopUpFinalityQuorumCertificate:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "round",
            "proposal_round",
            "phase",
            "subject",
            "execution_commitment",
            "signers",
            "aggregate_signature",
        ),
    )
    round_ = _offline_top_up_finality_round(
        _offline_required(record, "round", context), f"{context}.round"
    )
    proposal_round = _offline_top_up_finality_round(
        _offline_required(record, "proposal_round", context),
        f"{context}.proposal_round",
    )
    if proposal_round != round_:
        raise RuntimeError(f"{context}.proposal_round must equal round")
    phase = _offline_top_up_finality_phase(
        _offline_required(record, "phase", context), f"{context}.phase"
    )
    if phase.phase != "commit":
        raise RuntimeError(f"{context}.phase must be commit in finality evidence")
    raw_signers = _offline_required(record, "signers", context)
    if not isinstance(raw_signers, list) or not (
        1 <= len(raw_signers) <= _OFFLINE_TOP_UP_FINALITY_MAX_VALIDATORS
    ):
        raise RuntimeError(
            f"{context}.signers must contain between 1 and "
            f"{_OFFLINE_TOP_UP_FINALITY_MAX_VALIDATORS} indices"
        )
    signers = tuple(
        _offline_unsigned(raw, f"{context}.signers[{index}]", _OFFLINE_MAX_U32)
        for index, raw in enumerate(raw_signers)
    )
    if any(left >= right for left, right in zip(signers, signers[1:])):
        raise RuntimeError(f"{context}.signers must be strictly increasing and unique")
    aggregate_signature = tuple(
        _offline_byte_array(
            _offline_required(record, "aggregate_signature", context),
            f"{context}.aggregate_signature",
            _OFFLINE_BLS_PROOF_BYTES,
        )
    )
    if not any(aggregate_signature):
        raise RuntimeError(f"{context}.aggregate_signature must not be all zero")
    return OfflineTopUpFinalityQuorumCertificate(
        round=round_,
        proposal_round=proposal_round,
        phase=phase,
        subject=_offline_top_up_finality_subject(
            _offline_required(record, "subject", context),
            f"{context}.subject",
            round_height=round_.height,
        ),
        execution_commitment=_offline_top_up_finality_execution_commitment(
            _offline_required(record, "execution_commitment", context),
            f"{context}.execution_commitment",
            require_topup=require_topup,
        ),
        signers=signers,
        aggregate_signature=aggregate_signature,
    )


def _offline_top_up_finality_validator_power(
    value: Any, context: str
) -> OfflineTopUpFinalityValidatorPower:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("validator", "power"),
    )
    validator = _offline_exact_string(
        _offline_required(record, "validator", context), f"{context}.validator"
    )
    if _OFFLINE_BLS_VALIDATOR_ID_RE.fullmatch(validator) is None:
        raise RuntimeError(
            f"{context}.validator must be a canonical uppercase BLS-normal peer id"
        )
    return OfflineTopUpFinalityValidatorPower(
        validator=validator,
        power=_offline_unsigned(
            _offline_required(record, "power", context),
            f"{context}.power",
            _OFFLINE_MAX_U64,
            positive=True,
        ),
    )


def _offline_top_up_finality_quorum(
    value: Any,
    context: str,
    roster: Tuple[OfflineTopUpFinalityValidatorPower, ...],
) -> OfflineTopUpFinalityDualQuorum:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("min_signers", "total_power"),
    )
    quorum = OfflineTopUpFinalityDualQuorum(
        min_signers=_offline_unsigned(
            _offline_required(record, "min_signers", context),
            f"{context}.min_signers",
            _OFFLINE_TOP_UP_FINALITY_MAX_VALIDATORS,
            positive=True,
        ),
        total_power=_offline_unsigned(
            _offline_required(record, "total_power", context),
            f"{context}.total_power",
            _OFFLINE_MAX_U64,
            positive=True,
        ),
    )
    expected_min_signers = len(roster) * 2 // 3 + 1
    expected_total_power = sum(entry.power for entry in roster)
    if expected_total_power > _OFFLINE_MAX_U64:
        raise RuntimeError(f"{context}.total_power overflows uint64")
    if quorum.min_signers != expected_min_signers:
        raise RuntimeError(f"{context}.min_signers is not canonical for its roster")
    if quorum.total_power != expected_total_power:
        raise RuntimeError(f"{context}.total_power does not equal its roster power")
    return quorum


def _offline_top_up_finality_next_epoch_snapshot(
    value: Any,
    context: str,
    *,
    current_epoch: int,
    successor_height: int,
    current_mode: OfflineTopUpFinalityConsensusMode,
) -> OfflineTopUpFinalityNextEpochSnapshot:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "epoch",
            "epoch_end_height",
            "mode",
            "roster",
            "validator_set_pops",
            "quorum",
            "leader_seed",
        ),
    )
    epoch = _offline_unsigned(
        _offline_required(record, "epoch", context),
        f"{context}.epoch",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if current_epoch == _OFFLINE_MAX_U64 or epoch != current_epoch + 1:
        raise RuntimeError(f"{context}.epoch must immediately follow the current epoch")
    epoch_end_height = _offline_unsigned(
        _offline_required(record, "epoch_end_height", context),
        f"{context}.epoch_end_height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if epoch_end_height < successor_height:
        raise RuntimeError(f"{context}.epoch_end_height precedes the successor height")
    mode = _offline_top_up_finality_consensus_mode(
        _offline_required(record, "mode", context), f"{context}.mode"
    )
    if mode != current_mode:
        raise RuntimeError(f"{context}.mode must equal the current consensus mode")
    raw_roster = _offline_required(record, "roster", context)
    if not isinstance(raw_roster, list) or not (
        1 <= len(raw_roster) <= _OFFLINE_TOP_UP_FINALITY_MAX_VALIDATORS
    ):
        raise RuntimeError(
            f"{context}.roster must contain between 1 and "
            f"{_OFFLINE_TOP_UP_FINALITY_MAX_VALIDATORS} validators"
        )
    roster = tuple(
        _offline_top_up_finality_validator_power(raw, f"{context}.roster[{index}]")
        for index, raw in enumerate(raw_roster)
    )
    if any(
        left.validator >= right.validator for left, right in zip(roster, roster[1:])
    ):
        raise RuntimeError(
            f"{context}.roster must be strictly ordered by unique validator id"
        )
    if mode.mode == "permissioned" and any(entry.power != 1 for entry in roster):
        raise RuntimeError(f"{context}.roster permissioned voting powers must all be one")
    raw_pops = _offline_required(record, "validator_set_pops", context)
    if not isinstance(raw_pops, list) or len(raw_pops) != len(roster):
        raise RuntimeError(
            f"{context}.validator_set_pops must align one-for-one with roster"
        )
    validator_set_pops = tuple(
        tuple(
            _offline_byte_array(
                raw,
                f"{context}.validator_set_pops[{index}]",
                _OFFLINE_BLS_PROOF_BYTES,
            )
        )
        for index, raw in enumerate(raw_pops)
    )
    if any(not any(proof) for proof in validator_set_pops):
        raise RuntimeError(f"{context}.validator_set_pops must not contain zero proofs")
    return OfflineTopUpFinalityNextEpochSnapshot(
        epoch=epoch,
        epoch_end_height=epoch_end_height,
        mode=mode,
        roster=roster,
        validator_set_pops=validator_set_pops,
        quorum=_offline_top_up_finality_quorum(
            _offline_required(record, "quorum", context), f"{context}.quorum", roster
        ),
        leader_seed=_offline_fixed_bytes(
            _offline_required(record, "leader_seed", context), f"{context}.leader_seed"
        ),
    )


def _offline_top_up_finality_snapshot_bootstrap_anchor(
    value: Any, context: str
) -> OfflineTopUpFinalitySnapshotBootstrapAnchor:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "snapshot_height",
            "snapshot_block_hash",
            "snapshot_block_creation_time_ms",
            "snapshot_state_hash",
        ),
    )
    return OfflineTopUpFinalitySnapshotBootstrapAnchor(
        snapshot_height=_offline_unsigned(
            _offline_required(record, "snapshot_height", context),
            f"{context}.snapshot_height",
            _OFFLINE_MAX_U64,
            positive=True,
        ),
        snapshot_block_hash=_offline_hash_literal(
            _offline_required(record, "snapshot_block_hash", context),
            f"{context}.snapshot_block_hash",
        ),
        snapshot_block_creation_time_ms=_offline_unsigned(
            _offline_required(record, "snapshot_block_creation_time_ms", context),
            f"{context}.snapshot_block_creation_time_ms",
            _OFFLINE_MAX_U64,
        ),
        snapshot_state_hash=_offline_hash_literal(
            _offline_required(record, "snapshot_state_hash", context),
            f"{context}.snapshot_state_hash",
        ),
    )


def _offline_top_up_finality_height_context(
    value: Any,
    context: str,
    *,
    expected_finalized_height: int,
) -> OfflineTopUpFinalityHeightContext:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "context_id",
            "network_id",
            "protocol_version",
            "height",
            "epoch",
            "epoch_end_height",
            "mode",
            "nexus_amx_context_hash",
            "execution_policy_hash",
            "da_layout",
            "leader_seed",
        ),
        optional=(
            "next_epoch_snapshot",
            "parent_commit_qc",
            "snapshot_bootstrap",
        ),
    )
    context_id = _offline_top_up_finality_height_context_id(
        _offline_required(record, "context_id", context), f"{context}.context_id"
    )
    network_id = _offline_hash_literal(
        _offline_required(record, "network_id", context), f"{context}.network_id"
    )
    protocol_version = _offline_unsigned(
        _offline_required(record, "protocol_version", context),
        f"{context}.protocol_version",
        (1 << 16) - 1,
    )
    if protocol_version != _OFFLINE_SUMERAGI_PROTOCOL_VERSION:
        raise RuntimeError(
            f"{context}.protocol_version must be {_OFFLINE_SUMERAGI_PROTOCOL_VERSION}"
        )
    height = _offline_unsigned(
        _offline_required(record, "height", context),
        f"{context}.height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if height != expected_finalized_height:
        raise RuntimeError(
            f"{context}.height does not match finalized_block_height"
        )
    epoch = _offline_unsigned(
        _offline_required(record, "epoch", context), f"{context}.epoch", _OFFLINE_MAX_U64
    )
    epoch_end_height = _offline_unsigned(
        _offline_required(record, "epoch_end_height", context),
        f"{context}.epoch_end_height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if epoch_end_height < height:
        raise RuntimeError(f"{context}.epoch_end_height must not precede height")
    mode = _offline_top_up_finality_consensus_mode(
        _offline_required(record, "mode", context), f"{context}.mode"
    )
    raw_next_snapshot = record.get("next_epoch_snapshot")
    if raw_next_snapshot is None:
        next_epoch_snapshot = None
    else:
        if height == _OFFLINE_MAX_U64:
            raise RuntimeError(f"{context}.height has no representable successor")
        next_epoch_snapshot = _offline_top_up_finality_next_epoch_snapshot(
            raw_next_snapshot,
            f"{context}.next_epoch_snapshot",
            current_epoch=epoch,
            successor_height=height + 1,
            current_mode=mode,
        )
    raw_parent_qc = record.get("parent_commit_qc")
    parent_commit_qc = (
        None
        if raw_parent_qc is None
        else _offline_top_up_finality_qc(
            raw_parent_qc,
            f"{context}.parent_commit_qc",
            require_topup=False,
        )
    )
    if parent_commit_qc is not None and parent_commit_qc.round.height + 1 != height:
        raise RuntimeError(
            f"{context}.parent_commit_qc.round.height must immediately precede height"
        )
    raw_snapshot_bootstrap = record.get("snapshot_bootstrap")
    snapshot_bootstrap = (
        None
        if raw_snapshot_bootstrap is None
        else _offline_top_up_finality_snapshot_bootstrap_anchor(
            raw_snapshot_bootstrap,
            f"{context}.snapshot_bootstrap",
        )
    )
    if parent_commit_qc is not None and snapshot_bootstrap is not None:
        raise RuntimeError(
            f"{context}.parent_commit_qc and snapshot_bootstrap are mutually exclusive"
        )
    if (
        snapshot_bootstrap is not None
        and snapshot_bootstrap.snapshot_height + 1 != height
    ):
        raise RuntimeError(
            f"{context}.snapshot_bootstrap.snapshot_height must immediately precede height"
        )
    return OfflineTopUpFinalityHeightContext(
        context_id=context_id,
        network_id=network_id,
        protocol_version=4,
        height=height,
        epoch=epoch,
        epoch_end_height=epoch_end_height,
        next_epoch_snapshot=next_epoch_snapshot,
        mode=mode,
        parent_commit_qc=parent_commit_qc,
        snapshot_bootstrap=snapshot_bootstrap,
        nexus_amx_context_hash=_offline_hash_literal(
            _offline_required(record, "nexus_amx_context_hash", context),
            f"{context}.nexus_amx_context_hash",
        ),
        execution_policy_hash=_offline_hash_literal(
            _offline_required(record, "execution_policy_hash", context),
            f"{context}.execution_policy_hash",
        ),
        da_layout=_offline_top_up_finality_da_layout(
            _offline_required(record, "da_layout", context), f"{context}.da_layout"
        ),
        leader_seed=_offline_fixed_bytes(
            _offline_required(record, "leader_seed", context), f"{context}.leader_seed"
        ),
    )


def _offline_top_up_anchor_merkle_proof(
    value: Any,
    context: str,
    *,
    expected_leaf_count: int,
) -> OfflineTopUpAnchorMerkleProof:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("leaf_index", "leaf_count", "siblings"),
    )
    leaf_count = _offline_unsigned(
        _offline_required(record, "leaf_count", context),
        f"{context}.leaf_count",
        _OFFLINE_TOP_UP_FINALITY_MAX_ANCHORS_PER_BLOCK,
        positive=True,
    )
    if leaf_count != expected_leaf_count:
        raise RuntimeError(
            f"{context}.leaf_count must equal commit_qc certificate topup_anchor_count"
        )
    leaf_index = _offline_unsigned(
        _offline_required(record, "leaf_index", context),
        f"{context}.leaf_index",
        _OFFLINE_TOP_UP_FINALITY_MAX_ANCHORS_PER_BLOCK - 1,
    )
    if leaf_index >= leaf_count:
        raise RuntimeError(f"{context}.leaf_index must be less than leaf_count")
    raw_siblings = _offline_required(record, "siblings", context)
    if not isinstance(raw_siblings, list):
        raise RuntimeError(f"{context}.siblings must be an array")
    expected_depth = (leaf_count - 1).bit_length()
    if len(raw_siblings) != expected_depth or len(raw_siblings) > _OFFLINE_TOP_UP_FINALITY_MAX_SIBLINGS:
        raise RuntimeError(
            f"{context}.siblings must contain the canonical {expected_depth}-level path"
        )
    siblings = tuple(
        _offline_fixed_bytes(raw, f"{context}.siblings[{index}]", non_zero=True)
        for index, raw in enumerate(raw_siblings)
    )
    return OfflineTopUpAnchorMerkleProof(
        leaf_index=leaf_index,
        leaf_count=leaf_count,
        siblings=siblings,
    )


def _offline_top_up_anchor(
    value: Any,
    context: str,
    *,
    expected_operation_id: str,
    expected_transaction_hash: str,
    expected_finalized_height: int,
) -> OfflineTopUpAnchor:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=(
            "version",
            "network_id",
            "payer",
            "asset",
            "asset_scale",
            "amount",
            "initial_root",
            "finalized_root",
            "shield_leaf_index",
            "current_note",
            "topup_operation_id",
            "shield_verifier_id",
            "shield_verifier_commitment",
            "artifact_binding",
            "finalized_height",
            "finalized_tx_hash",
            "anchor_digest",
        ),
    )
    version = _offline_unsigned(
        _offline_required(record, "version", context), f"{context}.version", (1 << 16) - 1
    )
    if version != 4:
        raise RuntimeError(f"{context}.version must be 4")
    amount = _offline_scaled_amount_model(
        _offline_required(record, "amount", context), f"{context}.amount"
    )
    asset_scale = cast(
        OfflineAssetScale,
        _offline_unsigned(
            _offline_required(record, "asset_scale", context),
            f"{context}.asset_scale",
            _OFFLINE_MAX_ASSET_SCALE,
        ),
    )
    if asset_scale != amount.scale:
        raise RuntimeError(f"{context}.asset_scale must equal amount.scale")

    initial_root = _offline_fixed_bytes(
        _offline_required(record, "initial_root", context),
        f"{context}.initial_root",
        non_zero=True,
    )
    finalized_root = _offline_fixed_bytes(
        _offline_required(record, "finalized_root", context),
        f"{context}.finalized_root",
        non_zero=True,
    )
    if initial_root == finalized_root:
        raise RuntimeError(f"{context}.finalized_root must differ from initial_root")

    shield_leaf_index = _offline_unsigned(
        _offline_required(record, "shield_leaf_index", context),
        f"{context}.shield_leaf_index",
        _OFFLINE_TOP_UP_SHIELD_TREE_CAPACITY - 1,
    )

    current_note = _offline_spendable_note(
        _offline_required(record, "current_note", context), f"{context}.current_note"
    )
    network_id = _offline_hash_literal(
        _offline_required(record, "network_id", context), f"{context}.network_id"
    )
    if current_note.network_id != network_id:
        raise RuntimeError(f"{context}.current_note.network_id must equal network_id")
    if current_note.amount != amount:
        raise RuntimeError(f"{context}.current_note.amount must equal amount")
    asset = _offline_exact_string(
        _offline_required(record, "asset", context), f"{context}.asset"
    )
    if current_note.asset != asset:
        raise RuntimeError(f"{context}.current_note.asset must equal asset")

    topup_operation_id = _offline_fixed_bytes(
        _offline_required(record, "topup_operation_id", context),
        f"{context}.topup_operation_id",
        non_zero=True,
    )
    if bytes(topup_operation_id).hex() != expected_operation_id:
        raise RuntimeError(f"{context}.topup_operation_id does not match the operation")
    finalized_height = _offline_unsigned(
        _offline_required(record, "finalized_height", context),
        f"{context}.finalized_height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if finalized_height != expected_finalized_height:
        raise RuntimeError(
            f"{context}.finalized_height does not match finalized_block_height"
        )
    finalized_tx_hash = _offline_fixed_bytes(
        _offline_required(record, "finalized_tx_hash", context),
        f"{context}.finalized_tx_hash",
        non_zero=True,
    )
    if bytes(finalized_tx_hash).hex() != expected_transaction_hash:
        raise RuntimeError(f"{context}.finalized_tx_hash does not match transaction_hash")
    artifact_context = f"{context}.artifact_binding"
    artifact_record = _offline_mapping(
        _offline_required(record, "artifact_binding", context), artifact_context
    )
    _offline_exact_object_fields(
        artifact_record,
        artifact_context,
        required=("version", "generation", "manifest_sha256"),
    )
    artifact_version = _offline_unsigned(
        _offline_required(artifact_record, "version", artifact_context),
        f"{artifact_context}.version",
        (1 << 16) - 1,
    )
    if artifact_version != 4:
        raise RuntimeError(f"{artifact_context}.version must be 4")
    artifact_generation = _offline_exact_string(
        _offline_required(artifact_record, "generation", artifact_context),
        f"{artifact_context}.generation",
    )
    if len(artifact_generation.encode("utf-8")) > 128:
        raise RuntimeError(
            f"{artifact_context}.generation must contain at most 128 UTF-8 bytes"
        )
    artifact_binding = KagemushaArtifactBindingV4(
        version=4,
        generation=artifact_generation,
        manifest_sha256=_offline_fixed_bytes(
            _offline_required(artifact_record, "manifest_sha256", artifact_context),
            f"{artifact_context}.manifest_sha256",
            non_zero=True,
        ),
    )

    return OfflineTopUpAnchor(
        version=4,
        network_id=network_id,
        payer=_offline_exact_string(
            _offline_required(record, "payer", context), f"{context}.payer"
        ),
        asset=asset,
        asset_scale=asset_scale,
        amount=amount,
        initial_root=initial_root,
        finalized_root=finalized_root,
        shield_leaf_index=shield_leaf_index,
        current_note=current_note,
        topup_operation_id=topup_operation_id,
        shield_verifier_id=_offline_verifier_key_id(
            _offline_required(record, "shield_verifier_id", context),
            f"{context}.shield_verifier_id",
        ),
        shield_verifier_commitment=_offline_fixed_bytes(
            _offline_required(record, "shield_verifier_commitment", context),
            f"{context}.shield_verifier_commitment",
            non_zero=True,
        ),
        artifact_binding=artifact_binding,
        finalized_height=finalized_height,
        finalized_tx_hash=finalized_tx_hash,
        anchor_digest=_offline_fixed_bytes(
            _offline_required(record, "anchor_digest", context),
            f"{context}.anchor_digest",
            non_zero=True,
        ),
    )


def _offline_top_up_finality_proof(
    value: Any,
    context: str,
    *,
    expected_operation_id: str,
    expected_anchor_digest: Tuple[int, ...],
    expected_finalized_height: int,
) -> OfflineTopUpFinalityProof:
    record = _offline_mapping(value, context)
    _offline_exact_object_fields(
        record,
        context,
        required=("version", "anchor", "commit_qc", "anchor_path"),
    )
    version = _offline_unsigned(
        _offline_required(record, "version", context),
        f"{context}.version",
        (1 << 16) - 1,
    )
    if version != 1:
        raise RuntimeError(f"{context}.version must be 1")

    anchor_context = f"{context}.anchor"
    raw_anchor = _offline_mapping(
        _offline_required(record, "anchor", context), anchor_context
    )
    _offline_exact_object_fields(
        raw_anchor,
        anchor_context,
        required=("topup_operation_id", "anchor_digest"),
    )
    topup_operation_id = _offline_fixed_bytes(
        _offline_required(raw_anchor, "topup_operation_id", anchor_context),
        f"{anchor_context}.topup_operation_id",
        non_zero=True,
    )
    if bytes(topup_operation_id).hex() != expected_operation_id:
        raise RuntimeError(
            f"{anchor_context}.topup_operation_id does not match the operation"
        )
    anchor_digest = _offline_fixed_bytes(
        _offline_required(raw_anchor, "anchor_digest", anchor_context),
        f"{anchor_context}.anchor_digest",
        non_zero=True,
    )
    if anchor_digest != expected_anchor_digest:
        raise RuntimeError(
            f"{anchor_context}.anchor_digest does not match the finalized anchor"
        )

    commit_qc_context = f"{context}.commit_qc"
    commit_qc = _offline_mapping(
        _offline_required(record, "commit_qc", context), commit_qc_context
    )
    _offline_exact_object_fields(
        commit_qc,
        commit_qc_context,
        required=("height_context", "certificate"),
    )
    height_context_context = f"{commit_qc_context}.height_context"
    height_context = _offline_top_up_finality_height_context(
        _offline_required(commit_qc, "height_context", commit_qc_context),
        height_context_context,
        expected_finalized_height=expected_finalized_height,
    )

    certificate_context = f"{commit_qc_context}.certificate"
    certificate = _offline_top_up_finality_qc(
        _offline_required(commit_qc, "certificate", commit_qc_context),
        certificate_context,
        require_topup=True,
    )
    if certificate.round.context_id != height_context.context_id:
        raise RuntimeError(
            f"{certificate_context}.round.context_id does not match height_context.context_id"
        )
    if certificate.round.height != height_context.height:
        raise RuntimeError(
            f"{certificate_context}.round.height does not match height_context.height"
        )

    anchor_path_context = f"{context}.anchor_path"
    anchor_path = _offline_top_up_anchor_merkle_proof(
        _offline_required(record, "anchor_path", context),
        anchor_path_context,
        expected_leaf_count=certificate.execution_commitment.topup_anchor_count,
    )
    return OfflineTopUpFinalityProof(
        version=1,
        anchor=OfflineTopUpFinalityProofAnchor(
            topup_operation_id=topup_operation_id,
            anchor_digest=anchor_digest,
        ),
        commit_qc=OfflineTopUpFinalityCompactQc(
            height_context=height_context,
            certificate=certificate,
        ),
        anchor_path=anchor_path,
    )


def _offline_applied_result(
    value: Any, context: str, operation_id: str
) -> OfflineAppliedResult:
    record = _offline_mapping(value, context)
    kind = _offline_required(record, "kind", context)
    if kind not in ("top_up", "redeem"):
        raise RuntimeError(f"{context}.kind must be top_up or redeem")
    result_context = f"{context}.result"
    result = _offline_mapping(_offline_required(record, "result", context), result_context)
    transaction_hash = _offline_transaction_hash(
        _offline_required(result, "transaction_hash", result_context),
        f"{result_context}.transaction_hash",
    )
    finalized_block_height = _offline_unsigned(
        _offline_required(result, "finalized_block_height", result_context),
        f"{result_context}.finalized_block_height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    server_time_ms = _offline_unsigned(
        _offline_required(result, "server_time_ms", result_context),
        f"{result_context}.server_time_ms",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if kind == "top_up":
        anchor = _offline_top_up_anchor(
            _offline_required(result, "anchor", result_context),
            f"{result_context}.anchor",
            expected_operation_id=operation_id,
            expected_transaction_hash=transaction_hash,
            expected_finalized_height=finalized_block_height,
        )
        finality_proof = _offline_top_up_finality_proof(
            _offline_required(result, "finality_proof", result_context),
            f"{result_context}.finality_proof",
            expected_operation_id=operation_id,
            expected_anchor_digest=anchor.anchor_digest,
            expected_finalized_height=finalized_block_height,
        )
        return OfflineTopUpOperationResult(
            OfflineTopUpResult(
                transaction_hash=transaction_hash,
                finalized_block_height=finalized_block_height,
                server_time_ms=server_time_ms,
                anchor=anchor,
                finality_proof=finality_proof,
            )
        )
    for top_up_only_field in ("anchor", "finality_proof"):
        if top_up_only_field in result:
            raise RuntimeError(
                f"{result_context}.{top_up_only_field} is invalid for a redeem result"
            )
    return OfflineRedeemOperationResult(
        OfflineRedeemResult(
            transaction_hash=transaction_hash,
            finalized_block_height=finalized_block_height,
            server_time_ms=server_time_ms,
        )
    )


def _offline_operation_status(
    payload: Mapping[str, Any], expected_operation_id: str
) -> OfflineOperationStatus:
    context = "offline operation status"
    record = _offline_mapping(payload, context)
    state = _offline_required(record, "state", context)
    if state not in ("pending", "applied", "rejected"):
        raise RuntimeError(f"{context}.state must be pending, applied, or rejected")
    value_context = f"{context}.value"
    value = _offline_mapping(_offline_required(record, "value", context), value_context)
    operation_id = _require_offline_operation_id(
        _offline_required(value, "operation_id", value_context),
        f"{value_context}.operation_id",
    )
    if operation_id != expected_operation_id:
        raise RuntimeError(
            f"{value_context}.operation_id does not match the requested operation"
        )
    if state == "pending":
        return OfflinePendingOperation(
            operation_id=operation_id,
            kind=_offline_operation_kind(
                _offline_required(value, "kind", value_context), f"{value_context}.kind"
            ),
            transaction_hash=_offline_transaction_hash(
                _offline_required(value, "transaction_hash", value_context),
                f"{value_context}.transaction_hash",
            ),
            submitted_at_ms=_offline_unsigned(
                _offline_required(value, "submitted_at_ms", value_context),
                f"{value_context}.submitted_at_ms",
                _OFFLINE_MAX_U64,
            ),
        )
    if state == "applied":
        return OfflineAppliedOperation(
            operation_id=operation_id,
            result=_offline_applied_result(
                _offline_required(value, "result", value_context),
                f"{value_context}.result",
                operation_id,
            ),
        )
    return OfflineRejectedOperation(
        operation_id=operation_id,
        kind=_offline_operation_kind(
            _offline_required(value, "kind", value_context), f"{value_context}.kind"
        ),
        transaction_hash=_offline_transaction_hash(
            _offline_required(value, "transaction_hash", value_context),
            f"{value_context}.transaction_hash",
        ),
        error=_offline_error(
            _offline_required(value, "error", value_context), f"{value_context}.error"
        ),
    )


def _parse_app_api_transaction_draft_fields(
    payload: Mapping[str, Any],
    *,
    context: str,
    additional_fields: Sequence[str] = (),
) -> Tuple[bool, str, str]:
    """Validate Torii's canonical, unsigned app-API transaction draft."""

    if not isinstance(payload, Mapping):
        raise RuntimeError(f"{context} must be an object")
    expected_fields = {
        "submitted",
        "transaction_payload_b64",
        "signing_message_b64",
        *additional_fields,
    }
    unknown_fields = set(payload) - expected_fields
    if unknown_fields:
        raise RuntimeError(
            f"{context} contains unsupported fields: {', '.join(sorted(unknown_fields))}"
        )
    missing_fields = expected_fields - set(payload)
    if missing_fields:
        raise RuntimeError(
            f"{context} is missing fields: {', '.join(sorted(missing_fields))}"
        )
    if payload.get("submitted") is not False:
        raise RuntimeError(f"{context}.submitted must be false")

    def decode_exact_base64(field: str) -> Tuple[str, bytes]:
        value = payload.get(field)
        if not isinstance(value, str) or not value or any(char.isspace() for char in value):
            raise RuntimeError(f"{context}.{field} must be exact standard-base64")
        try:
            decoded = base64.b64decode(value, validate=True)
        except (binascii.Error, ValueError) as exc:
            raise RuntimeError(
                f"{context}.{field} must be exact standard-base64"
            ) from exc
        if not decoded or base64.b64encode(decoded).decode("ascii") != value:
            raise RuntimeError(f"{context}.{field} must be exact standard-base64")
        return value, decoded

    transaction_payload_b64, transaction_payload = decode_exact_base64(
        "transaction_payload_b64"
    )
    signing_message_b64, signing_message = decode_exact_base64("signing_message_b64")
    if len(signing_message) != 32:
        raise RuntimeError(f"{context}.signing_message_b64 must decode to 32 bytes")
    expected_message = bytearray(
        hashlib.blake2b(transaction_payload, digest_size=32).digest()
    )
    expected_message[-1] |= 1
    if not secrets.compare_digest(signing_message, expected_message):
        raise RuntimeError(
            f"{context}.signing_message_b64 must match transaction_payload_b64"
        )
    return False, transaction_payload_b64, signing_message_b64


@dataclass(frozen=True)
class AppApiTransactionDraft:
    """Canonical transaction payload prepared by Torii for local signing."""

    submitted: bool
    transaction_payload_b64: str
    signing_message_b64: str

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
        *,
        context: str = "app API transaction draft",
    ) -> "AppApiTransactionDraft":
        submitted, transaction_payload_b64, signing_message_b64 = (
            _parse_app_api_transaction_draft_fields(payload, context=context)
        )
        return cls(
            submitted=submitted,
            transaction_payload_b64=transaction_payload_b64,
            signing_message_b64=signing_message_b64,
        )


@dataclass(frozen=True)
class SubscriptionPlanCreateResult(AppApiTransactionDraft):
    """Unsigned plan-registration draft from ``POST /v1/subscriptions/plans``."""

    plan_id: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionPlanCreateResult":
        submitted, transaction_payload_b64, signing_message_b64 = (
            _parse_app_api_transaction_draft_fields(
                payload,
                context="subscription plan create response",
                additional_fields=("plan_id",),
            )
        )
        plan_id = payload.get("plan_id")
        if not isinstance(plan_id, str) or not plan_id:
            raise RuntimeError("subscription plan create response missing `plan_id`")
        return cls(
            submitted=submitted,
            transaction_payload_b64=transaction_payload_b64,
            signing_message_b64=signing_message_b64,
            plan_id=plan_id,
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
class SubscriptionUsageDraft(AppApiTransactionDraft):
    """Unsigned usage-recording draft returned by Torii."""

    subscription_id: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionUsageDraft":
        submitted, transaction_payload_b64, signing_message_b64 = (
            _parse_app_api_transaction_draft_fields(
                payload,
                context="subscription usage response",
                additional_fields=("subscription_id",),
            )
        )
        subscription_id = payload.get("subscription_id")
        if not isinstance(subscription_id, str) or not subscription_id:
            raise RuntimeError("subscription usage response missing `subscription_id`")
        return cls(
            submitted=submitted,
            transaction_payload_b64=transaction_payload_b64,
            signing_message_b64=signing_message_b64,
            subscription_id=subscription_id,
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
class GovernanceProposalStatus:
    """Result returned by ``GET /v1/gov/proposals/{id}``."""

    found: bool
    proposal: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class GovernanceLockCustody:
    """Immutable asset custody retained with a governance lock."""

    escrowed: bool
    asset_definition_id: str
    bond_escrow_account: str
    slash_receiver_account: str

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> "GovernanceLockCustody":
        if not isinstance(payload, Mapping):
            raise RuntimeError(f"{context} must be a JSON object or null")
        expected_fields = {
            "escrowed",
            "asset_definition_id",
            "bond_escrow_account",
            "slash_receiver_account",
        }
        if set(payload) != expected_fields:
            raise RuntimeError(
                f"{context} must contain exactly `escrowed`, `asset_definition_id`, "
                "`bond_escrow_account`, and `slash_receiver_account`"
            )
        escrowed = payload["escrowed"]
        if not isinstance(escrowed, bool):
            raise RuntimeError(f"{context}.escrowed must be a boolean")

        identifiers: Dict[str, str] = {}
        for field in (
            "asset_definition_id",
            "bond_escrow_account",
            "slash_receiver_account",
        ):
            value = payload[field]
            if not isinstance(value, str) or not value:
                raise RuntimeError(f"{context}.{field} must be a non-empty string")
            if value.strip() != value:
                raise RuntimeError(
                    f"{context}.{field} must not contain surrounding whitespace"
                )
            identifiers[field] = value
        return cls(
            escrowed=escrowed,
            asset_definition_id=identifiers["asset_definition_id"],
            bond_escrow_account=identifiers["bond_escrow_account"],
            slash_receiver_account=identifiers["slash_receiver_account"],
        )


@dataclass(frozen=True)
class GovernanceLockRecord:
    """Strict governance lock record returned by Torii."""

    owner: str
    amount: str
    slashed: str
    expiry_height: int
    direction: int
    duration_blocks: int
    custody: Optional[GovernanceLockCustody]

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> "GovernanceLockRecord":
        if not isinstance(payload, Mapping):
            raise RuntimeError(f"{context} must be a JSON object")
        owner = payload.get("owner")
        if not isinstance(owner, str) or not owner:
            raise RuntimeError(f"{context}.owner must be a non-empty string")

        def unsigned(field: str, *, maximum: Optional[int] = None) -> int:
            value = payload.get(field)
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise RuntimeError(f"{context}.{field} must be an unsigned integer")
            if maximum is not None and value > maximum:
                raise RuntimeError(f"{context}.{field} exceeds its protocol bound")
            return value

        if "custody" not in payload:
            raise RuntimeError(f"{context}.custody must be present as an object or null")
        custody_payload = payload["custody"]
        custody = (
            None
            if custody_payload is None
            else GovernanceLockCustody.from_payload(
                custody_payload,
                context=f"{context}.custody",
            )
        )

        return cls(
            owner=owner,
            amount=_canonical_quantity(payload.get("amount"), f"{context}.amount"),
            slashed=_canonical_quantity(
                payload.get("slashed"),
                f"{context}.slashed",
            ),
            expiry_height=unsigned("expiry_height"),
            direction=unsigned("direction", maximum=0xFF),
            duration_blocks=unsigned("duration_blocks"),
            custody=custody,
        )


@dataclass(frozen=True)
class GovernanceLocksOverview:
    """Locks/escrow view returned by ``GET /v1/gov/locks/{referendum}``."""

    found: bool
    referendum_id: str
    locks: Optional[Dict[str, GovernanceLockRecord]]


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
class VpnQuoteCreateRequest:
    """Request body for creating a native Sora VPN lease quote."""

    metering_public_key_hex: Union[str, bytes, bytearray, memoryview]
    exit_class: Optional[str] = None

    def to_payload(self) -> Dict[str, Any]:
        exit_class = "" if self.exit_class is None else self.exit_class
        return {
            "exit_class": ToriiClient._require_vpn_enum(
                exit_class,
                _VPN_EXIT_CLASSES,
                "vpn quote exit_class",
            )
            if exit_class
            else "",
            "metering_public_key_hex": ToriiClient._normalize_hex_string(
                self.metering_public_key_hex,
                context="vpn quote metering_public_key_hex",
                expected_length=64,
            ),
        }


@dataclass(frozen=True)
class VpnSessionCreateRequest:
    """Request body for opening a native Sora VPN session from a paid quote."""

    quote_id: Union[str, bytes, bytearray, memoryview]
    payment_tx_hash: Union[str, bytes, bytearray, memoryview]
    metering_public_key_hex: Union[str, bytes, bytearray, memoryview]
    exit_class: Optional[str] = None

    def to_payload(self) -> Dict[str, Any]:
        exit_class = "" if self.exit_class is None else self.exit_class
        return {
            "exit_class": ToriiClient._require_vpn_enum(
                exit_class,
                _VPN_EXIT_CLASSES,
                "vpn session exit_class",
            )
            if exit_class
            else "",
            "quote_id": ToriiClient._normalize_vpn_canonical_hex_input(
                self.quote_id,
                context="vpn session quote_id",
                expected_length=64,
            ),
            "payment_tx_hash": ToriiClient._normalize_hex_string(
                self.payment_tx_hash,
                context="vpn session payment_tx_hash",
                expected_length=64,
            ),
            "metering_public_key_hex": ToriiClient._normalize_hex_string(
                self.metering_public_key_hex,
                context="vpn session metering_public_key_hex",
                expected_length=64,
            ),
        }


@dataclass(frozen=True)
class VpnReceiptSubmitRequest:
    """Request body for submitting a relay-signed native Sora VPN receipt."""

    relay_receipt_hex: Union[str, bytes, bytearray, memoryview]
    client_voucher_hex: Union[str, bytes, bytearray, memoryview]
    lease_id_hex: Optional[Union[str, bytes, bytearray, memoryview]] = None

    def to_payload(self) -> Dict[str, Any]:
        payload = {
            "relay_receipt_hex": ToriiClient._normalize_hex_string(
                self.relay_receipt_hex,
                context="vpn receipt relay_receipt_hex",
            ),
            "client_voucher_hex": ToriiClient._normalize_hex_string(
                self.client_voucher_hex,
                context="vpn receipt client_voucher_hex",
            ),
            "lease_id_hex": "",
        }
        if self.lease_id_hex is not None:
            payload["lease_id_hex"] = ToriiClient._normalize_hex_string(
                self.lease_id_hex,
                context="vpn receipt lease_id_hex",
                expected_length=64,
            )
        return payload


@dataclass(frozen=True)
class VpnProfile:
    """Sora VPN profile and native XOR lease parameters."""

    available: bool
    relay_endpoint: str
    supported_exit_classes: List[str]
    default_exit_class: str
    lease_secs: int
    dns_push_interval_secs: int
    meter_family: str
    route_pushes: List[str]
    excluded_routes: List[str]
    dns_servers: List[str]
    tunnel_addresses: List[str]
    mtu_bytes: int
    display_billing_label: str
    operator_account_id: str
    lease_fee: str
    settlement_grace_secs: int
    flow_label_bits: int
    padding_budget_ms: int
    relay_id_hex: str
    descriptor_commit_hex: str
    tls_server_name: str
    relay_tls_spki_sha256_hex: str
    relay_certificate_sha256_hex: str
    directory_snapshot_digest_hex: str


@dataclass(frozen=True)
class VpnQuote:
    """Native Sora VPN quote bound to a pending XOR escrow lease."""

    quote_id: str
    lease_id_hex: str
    session_id_hex: str
    payment_reference: str
    account_id: str
    exit_class: str
    relay_endpoint: str
    lease_secs: int
    quote_expires_at_ms: int
    fee_asset_id: str
    escrow_account_id: str
    operator_account_id: str
    lease_fee: str
    route_pushes: List[str]
    excluded_routes: List[str]
    dns_servers: List[str]
    tunnel_addresses: List[str]
    mtu_bytes: int
    meter_family: str
    flow_label_bits: int
    padding_budget_ms: int
    relay_id_hex: str
    descriptor_commit_hex: str
    tls_server_name: str
    relay_tls_spki_sha256_hex: str
    relay_certificate_sha256_hex: str
    directory_snapshot_digest_hex: str
    metering_public_key_hex: str
    open_lease_instruction: TransactionInstruction


@dataclass(frozen=True)
class VpnSession:
    """Active native Sora VPN session backed by a paid XOR lease."""

    session_id: str
    account_id: str
    exit_class: str
    relay_endpoint: str
    lease_secs: int
    expires_at_ms: int
    connected_at_ms: int
    meter_family: str
    quote_id: str
    payment_reference: str
    payment_tx_hash: str
    fee_asset_id: str
    escrow_account_id: str
    operator_account_id: str
    lease_fee: str
    flow_label_bits: int
    padding_budget_ms: int
    relay_id_hex: str
    descriptor_commit_hex: str
    tls_server_name: str
    relay_tls_spki_sha256_hex: str
    relay_certificate_sha256_hex: str
    directory_snapshot_digest_hex: str
    route_pushes: List[str]
    excluded_routes: List[str]
    dns_servers: List[str]
    tunnel_addresses: List[str]
    mtu_bytes: int
    helper_ticket_hex: str
    bytes_in: int
    bytes_out: int
    status: str


@dataclass(frozen=True)
class VpnReceipt:
    """Disconnected or settled native Sora VPN lease receipt."""

    session_id: str
    account_id: str
    exit_class: str
    relay_endpoint: str
    meter_family: str
    connected_at_ms: int
    disconnected_at_ms: int
    duration_ms: int
    bytes_in: int
    bytes_out: int
    status: str
    receipt_source: str
    quote_id: str
    payment_tx_hash: str
    fee_asset_id: str
    escrow_account_id: str
    operator_account_id: str
    lease_fee: str
    earned_fee: str
    refunded_fee: str
    lease_id_hex: str
    settle_lease_instruction: Optional[TransactionInstruction]


@dataclass(frozen=True)
class VpnReceiptListResponse:
    """Receipt page returned by the native Sora VPN receipt list endpoint."""

    items: List[VpnReceipt]
    total: int


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
class PipelineTransactionStatus:
    """Non-sensitive status metadata inside a public pipeline response."""

    kind: str
    block_height: Optional[int]


@dataclass(frozen=True)
class PipelineTransactionStatusResponse:
    """Metadata-only public pipeline response.

    Transaction contents, rejection diagnostics, trigger completions, and batch
    details require the separately authorized signed transaction-details query.
    """

    hash: str
    status: PipelineTransactionStatus
    scope: str
    resolved_from: str

    @property
    def is_terminal(self) -> bool:
        return self.status.kind in {"Committed", "Applied", "Rejected", "Expired"}

    @property
    def is_committed(self) -> bool:
        return self.status.kind in {"Committed", "Applied"}

    @property
    def is_rejected(self) -> bool:
        return self.status.kind == "Rejected"

@dataclass(frozen=True)
class ContractOperationReceipt:
    """Public normalized evidence for a contract operation."""

    operation_kind: str
    status: str
    transport: str
    dataspace: str
    contract_alias: Optional[str]
    contract_address: Optional[str]
    code_hash_hex: Optional[str]
    abi_hash_hex: Optional[str]
    tx_hash_hex: Optional[str]
    entrypoint: Optional[str]
    entrypoint_hash_hex: Optional[str]
    gas_limit: Optional[int]
    gas_used: Optional[int]
    fee_payment: Optional[Dict[str, Any]]
    payload_digest_hex: str


@dataclass(frozen=True)
class ContractCallResponse:
    """Result returned by ``POST /v1/contracts/call``."""

    ok: bool
    submitted: bool
    dataspace: str
    code_hash_hex: str
    abi_hash_hex: str
    creation_time_ms: int
    contract_address: Optional[str]
    tx_hash_hex: Optional[str]
    pipeline_status: Optional[PipelineTransactionStatusResponse]
    entrypoint: Optional[str]
    transaction_ttl_ms: Optional[int]
    entrypoint_hash_hex: Optional[str]
    transaction_payload_b64: Optional[str]
    signing_message_b64: Optional[str]
    operation_receipt: ContractOperationReceipt


@dataclass(frozen=True)
class MultisigResponse:
    """Result returned by multisig participation endpoints."""

    ok: bool
    resolved_multisig_account_id: str
    submitted: bool
    proposal_id: Optional[str]
    instructions_hash: Optional[str]
    tx_hash_hex: Optional[str]
    executed_tx_hash_hex: Optional[str]
    creation_time_ms: Optional[int]
    transaction_payload_b64: Optional[str]
    signing_message_b64: Optional[str]


@dataclass(frozen=True)
class GovernanceContractResponse:
    """Governance binding returned by ``GET /v1/gov/contracts/{contract_address}``."""

    found: bool
    contract_address: str
    dataspace: Optional[str]
    code_hash_hex: Optional[str]


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
class DataspaceCatalogEntry:
    """Configured Nexus dataspace joined with lane metadata from `/v1/status`."""

    lane_id: int
    lane_alias: str
    dataspace_id: int
    alias: str
    visibility: str
    storage_profile: str
    manifest_required: bool
    manifest_ready: bool
    sealed: bool
    manifest_path: Optional[str]
    protected_namespaces: List[str]


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

    contract_address: str
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
    queue_queued: int
    queue_inflight: int
    queue_delta: int
    time_since_last_block_ms: int
    time_since_last_non_empty_block_ms: int
    da_reschedule_delta: int
    tx_approved_delta: int
    tx_rejected_delta: int
    view_change_delta: int
    has_activity: bool


@dataclass(frozen=True)
class StatusPayload:
    """Raw `/v1/status` payload with typed fields."""

    observed_at_ms: int
    mode_tag: Optional[str]
    staged_mode_tag: Optional[str]
    staged_mode_activation_height: Optional[int]
    mode_activation_lag_blocks: Optional[int]
    consensus_caps: Optional["SumeragiConsensusCaps"]
    peers: int
    queue_size: int
    queue_queued: int
    queue_inflight: int
    last_block_committed_at_ms: int
    last_non_empty_block_committed_at_ms: int
    time_since_last_block_ms: int
    time_since_last_non_empty_block_ms: int
    commit_time_ms: int
    da_reschedule_total: int
    txs_approved: int
    txs_rejected: int
    view_changes: int
    governance: Optional[GovernanceStatusSnapshot]
    lane_commitments: List[LaneCommitmentSnapshot]
    dataspace_commitments: List[DataspaceCommitmentSnapshot]
    lane_governance: List[LaneGovernanceSnapshot]
    dataspace_catalog: List[DataspaceCatalogEntry]
    lane_governance_sealed_total: int
    lane_governance_sealed_aliases: List[str]
    raw: Dict[str, Any]

    def get_dataspace(self, alias: Union[str, int]) -> Optional[DataspaceCatalogEntry]:
        lookup = str(alias)
        for entry in self.dataspace_catalog:
            if entry.alias == lookup or str(entry.dataspace_id) == lookup:
                return entry
        return None

    def require_dataspace(self, alias: Union[str, int]) -> DataspaceCatalogEntry:
        entry = self.get_dataspace(alias)
        if entry is None:
            raise KeyError(f"dataspace not found in status catalog: {alias}")
        return entry

    def liveness_elapsed_ms(self) -> int:
        """Return the elapsed block time used for queue-aware stall checks."""

        if self.time_since_last_non_empty_block_ms > 0:
            return self.time_since_last_non_empty_block_ms
        return self.time_since_last_block_ms

    def is_queue_stalled(self, stall_threshold_ms: int) -> bool:
        """Classify stalls only when queued work exists and block progress exceeds the threshold."""

        return self.queue_size > 0 and self.liveness_elapsed_ms() > int(stall_threshold_ms)


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


@dataclass(frozen=True)
class SumeragiV2HeightContextStatus:
    """Frozen election and dual-quorum inputs governing one height."""

    epoch: int
    epoch_end_height: int
    mode: str
    epoch_seed: str
    validator_count: int
    min_signers: int
    total_power: int


@dataclass(frozen=True)
class SumeragiV2CommitQcStatus:
    """Latest durable CommitQC with exact count and voting-power totals."""

    certificate: SumeragiV2QcReference
    validator_count: int
    signer_count: int
    min_signers: int
    signed_power: int
    total_power: int


@dataclass(frozen=True)
class SumeragiV2VoteQuorumStatus:
    """Partial dual quorum for one exact round and proposal."""

    round: SumeragiV2Round
    proposal_round: SumeragiV2Round
    subject: SumeragiV2BlockSubject
    execution_commitment: SumeragiV2ExecutionCommitment
    signer_count: int
    signed_power: int
    min_signers: int
    total_power: int


@dataclass(frozen=True)
class SumeragiV2TimeoutQuorumStatus:
    """Partial timeout quorum for one exact round."""

    round: SumeragiV2Round
    signer_count: int
    signed_power: int
    min_signers: int
    total_power: int
    certificate_formed: bool


@dataclass(frozen=True)
class SumeragiV2OutboundIntentStatus:
    """Durable outbound protocol intent and its delivery stage."""

    kind: str
    round: SumeragiV2Round
    proposal_round: Optional[SumeragiV2Round]
    subject: Optional[SumeragiV2BlockSubject]
    execution_commitment: Optional[SumeragiV2ExecutionCommitment]
    stage: str


@dataclass(frozen=True)
class SumeragiV2WorkStatus:
    """Local terminating-work stages for the active height."""

    candidate: str
    body_recovery: str
    body_store: str
    validation: str
    application: str
    successor_height: str


@dataclass(frozen=True)
class SumeragiV2QueueLivenessStatus:
    """Occupancy and accumulated oldest-item service debt for one bounded queue."""

    queue: str
    depth: int
    capacity: int
    oldest_age_ms: Optional[int]
    service_debt: int


@dataclass(frozen=True)
class SumeragiV2ProgressTransitionStatus:
    """Last tracked reducer transition and its local age."""

    generation: int
    round: SumeragiV2Round
    transition: str
    age_ms: int


@dataclass(frozen=True)
class SumeragiV2IgnoreCount:
    """Per-height count for one closed reducer ignore reason."""

    reason: str
    count: int


@dataclass(frozen=True)
class SumeragiV2LivenessStatus:
    """Authoritative progress diagnostics for the active height."""

    generation: int
    prepare_quorums: List[SumeragiV2VoteQuorumStatus]
    commit_quorums: List[SumeragiV2VoteQuorumStatus]
    timeout_quorums: List[SumeragiV2TimeoutQuorumStatus]
    outbound_intents: List[SumeragiV2OutboundIntentStatus]
    work: SumeragiV2WorkStatus
    queues: List[SumeragiV2QueueLivenessStatus]
    last_progress: Optional[SumeragiV2ProgressTransitionStatus]
    no_progress_age_ms: int
    blocker: Optional[str]
    ignore_counts: List[SumeragiV2IgnoreCount]


@dataclass(frozen=True)
class SumeragiV2AdapterQueueStatus:
    """Bounded v2 reducer-adapter queue occupancy."""

    ingress_keys: int
    ingress_capacity: int
    deferred_completion: int
    deferred_progress: int
    deferred_progress_capacity: int
    deferred_normal: int
    deferred_normal_capacity: int


@dataclass(frozen=True)
class SumeragiV2TxQueueStatus:
    """Exact transaction-queue pressure sampled by the v2 runner."""

    tracked_transactions: int
    queued_transactions: int
    capacity: int
    retained_bytes: int
    max_retained_bytes: int
    oldest_queued_age_ms: int
    saturated_by_count: bool
    saturated_by_bytes: bool
    saturated_by_age: bool


@dataclass(frozen=True)
class SumeragiV2OperatorStatus:
    """Local diagnostics kept outside reducer-authoritative consensus state."""

    view_change_install_total: int
    busy_deferral_total: int
    adapter_queues: SumeragiV2AdapterQueueStatus
    tx_queue: SumeragiV2TxQueueStatus


@dataclass(frozen=True)
class SumeragiSafetyHaltStatus:
    """Fail-closed local consensus safety state reported with v2 status."""

    active: bool
    reason: Optional[str]
    height: int
    epoch: int
    first_block_hash: Optional[str]
    conflicting_block_hash: Optional[str]
    first_parent_state_root: Optional[str]
    first_post_state_root: Optional[str]
    conflicting_parent_state_root: Optional[str]
    conflicting_post_state_root: Optional[str]


@dataclass(frozen=True)
class SumeragiV2Status:
    """Authoritative reducer response from ``GET /v1/sumeragi/status``."""

    protocol_version: int
    node_fingerprint: str
    build_fingerprint: str
    config_fingerprint: str
    restart_required: bool
    height_context_id: Tuple[str]
    height: int
    view: int
    phase: str
    leader: int
    locked_prepare_qc: Optional[SumeragiV2QcReference]
    highest_prepare_qc: Optional[SumeragiV2QcReference]
    last_timeout_certificate: Optional[SumeragiV2TimeoutReference]
    body_state: str
    pending_persistence_id: Optional[int]
    last_committed_height: int
    last_committed_subject: Optional[SumeragiV2BlockSubject]
    height_context: SumeragiV2HeightContextStatus
    last_commit_qc: Optional[SumeragiV2CommitQcStatus]
    liveness: SumeragiV2LivenessStatus

    @classmethod
    def from_payload(cls, payload: Any) -> "SumeragiV2Status":
        """Validate and decode the authoritative v2 JSON response."""

        parsed = _SumeragiV2StatusParser.parse(payload)
        if not isinstance(parsed, cls):
            raise RuntimeError("sumeragi status parser returned an unexpected type")
        return parsed


@dataclass(frozen=True)
class SumeragiPipelineExecutionStatus:
    """Bounded execution counters returned by Sumeragi diagnostics."""

    tx_vertices_total: int
    tx_edges_total: int
    overlay_count_total: int
    overlay_instr_total: int
    overlay_bytes_total: int
    rbc_chunks_total: int
    rbc_bytes_total: int
    detached_prepared_total: int
    detached_merged_total: int
    detached_fallback_total: int
    detached_fallback_fee_postprocessing_total: int
    detached_fallback_user_executor_total: int
    detached_fallback_durable_state_total: int
    detached_fallback_unsupported_instruction_total: int
    detached_fallback_rejected_eval_total: int
    detached_fallback_overlay_error_total: int
    quarantine_executed_total: int


@dataclass(frozen=True)
class SumeragiNposDiagnostics:
    """Validated NPoS-only diagnostics."""

    epoch_length_blocks: int
    vrf_commit_deadline_offset: int
    vrf_reveal_deadline_offset: int
    epoch_seed: Tuple[int, ...]
    prf_height: int
    prf_view: int
    vrf_penalty_epoch: int
    vrf_committed_no_reveal_total: int
    vrf_no_participation_total: int
    vrf_late_reveals_total: int


@dataclass(frozen=True)
class SumeragiLaneCommitmentStatus:
    """Aggregated per-lane execution commitment."""

    block_height: int
    lane_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash: str


@dataclass(frozen=True)
class SumeragiDataspaceCommitmentStatus:
    """Aggregated per-dataspace execution commitment."""

    block_height: int
    lane_id: int
    dataspace_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash: str


@dataclass(frozen=True)
class SumeragiLaneGovernanceStatus:
    """Governance-manifest readiness for one lane."""

    lane_id: int
    alias: str
    governance: Optional[str]
    manifest_required: bool
    manifest_ready: bool
    manifest_path: Optional[str]
    validator_ids: List[str]
    quorum: Optional[int]
    protected_namespaces: List[str]
    runtime_upgrade: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class SumeragiNativeAmxParticipantApplication:
    """Durable participant-application evidence for one route incarnation."""

    lane_id: int
    dataspace_id: int
    lane_incarnation: str
    participant_height: int
    participant_view: int
    predecessor_height: int
    predecessor_descriptor_hash: Optional[str]
    descriptor_hash: str
    proposal_hash: str
    settlement_hash: str
    source_count: int
    application_block_height: Optional[int]
    application_block_hash: Optional[str]
    state: str


@dataclass(frozen=True)
class SumeragiAutonomousLaneExecution:
    """Restart-stable autonomous lane execution stage."""

    lane_id: int
    dataspace_id: int
    lane_incarnation: str
    lane_block_height: int
    lane_block_view: int
    proposal_height: int
    proposal_view: Optional[int]
    reservation_owner_hash: str
    proposal_identity_hash: str
    reservation_group_hash: str
    proposal_hash: Optional[str]
    descriptor_hash: Optional[str]
    executable_payload_hash: Optional[str]
    source_bundle_hash: Optional[str]
    merge_entry_hash: Optional[str]
    application_block_height: Optional[int]
    application_block_hash: Optional[str]
    reservation_count: int
    transaction_count: int
    highest_durable_stage: str
    stuck_reason: Optional[str]


@dataclass(frozen=True)
class SumeragiDiagnosticsStatus:
    """Non-authoritative response from ``GET /v1/sumeragi/diagnostics``."""

    pipeline_execution: SumeragiPipelineExecutionStatus
    tx_queue_depth: int
    tx_queue_capacity: int
    tx_queue_retained_bytes: int
    tx_queue_max_retained_bytes: int
    tx_queue_saturated: bool
    tx_queue_saturated_by_count: bool
    tx_queue_saturated_by_bytes: bool
    tx_queue_saturated_by_age: bool
    tx_queue_oldest_queued_age_ms: int
    npos: Optional[SumeragiNposDiagnostics]
    lane_commitments: List[SumeragiLaneCommitmentStatus]
    dataspace_commitments: List[SumeragiDataspaceCommitmentStatus]
    lane_settlement_commitments: List[Dict[str, Any]]
    lane_relay_envelopes: List[Dict[str, Any]]
    lane_payload_ownerships: List[Dict[str, Any]]
    committed_lane_blocks: List[Dict[str, Any]]
    lane_block_sessions: List[Dict[str, Any]]
    lane_governance_sealed_total: int
    lane_governance_sealed_aliases: List[str]
    lane_governance: List[SumeragiLaneGovernanceStatus]
    native_amx_participant_applications: List[
        SumeragiNativeAmxParticipantApplication
    ]
    autonomous_lane_executions: List[SumeragiAutonomousLaneExecution]

    @classmethod
    def from_payload(cls, payload: Any) -> "SumeragiDiagnosticsStatus":
        """Validate and decode one bounded diagnostics response."""

        return _SumeragiDiagnosticsParser.parse(payload)


@dataclass(frozen=True)
class PipelinePreflightSumeragi:
    """Consensus timing limits used by pipeline liveness helpers."""

    block_time_ms: int
    commit_time_ms: int
    stall_threshold_ms: int


@dataclass(frozen=True)
class PipelinePreflightAdmission:
    """Transaction admission limits advertised by Torii preflight."""

    max_signatures: int
    max_instructions: int
    max_tx_bytes: int
    max_decompressed_bytes: int
    max_metadata_depth: int


@dataclass(frozen=True)
class PipelinePreflightBlock:
    """Block assembly limits advertised by Torii preflight."""

    max_transactions: int


@dataclass(frozen=True)
class PipelinePreflightPipeline:
    """Pipeline execution and verification limits advertised by Torii preflight."""

    signature_batch_max: int
    signature_batch_max_ed25519: int
    signature_batch_max_secp256k1: int
    signature_batch_max_pqc: int
    signature_batch_max_bls: int
    overlay_max_instructions: int
    ivm_max_decoded_instructions: int


@dataclass(frozen=True)
class PipelinePreflightQueue:
    """Current queue occupancy advertised by Torii preflight."""

    size: int
    queued: int
    inflight: int


@dataclass(frozen=True)
class PipelinePreflightFees:
    """Nexus fee configuration advertised by Torii preflight."""

    fee_asset_id: str
    fee_sink_account_id: str
    base_fee: Any
    per_byte_fee: Any
    per_instruction_fee: Any
    per_gas_unit_fee: Any
    sponsor_vault_custody_account_id: str
    settlement_mode: str
    successful_claim_fee_exempt_authorities: List[str]


@dataclass(frozen=True)
class PipelinePreflight:
    """Typed response from `GET /v1/pipeline/preflight`."""

    schema_version: int
    chain_height: int
    sumeragi: PipelinePreflightSumeragi
    admission: PipelinePreflightAdmission
    block: PipelinePreflightBlock
    pipeline: PipelinePreflightPipeline
    queue: PipelinePreflightQueue
    fees: PipelinePreflightFees
    raw: Dict[str, Any]

    def is_status_stalled(self, status: StatusPayload) -> bool:
        """Classify status liveness using the preflight stall threshold."""

        return status.is_queue_stalled(self.sumeragi.stall_threshold_ms)


class _SumeragiV2StatusParser:
    """Fail-closed parser for the flattened authoritative v2 JSON projection."""

    MAX_CONSENSUS_VALIDATORS = 31
    MAX_LANE_VALIDATORS = 128
    MAX_LANE_SETTLEMENT_COMMITMENTS = 128
    MAX_LANE_RELAY_ENVELOPES = 64
    MAX_LANE_PAYLOAD_OWNERSHIPS = 128
    MAX_COMMITTED_LANE_BLOCKS = 128
    MAX_LANE_BLOCK_SESSIONS = 128
    MAX_NATIVE_AMX_PARTICIPANT_LEGS = 255
    MAX_NATIVE_AMX_PARTICIPANT_SETTLEMENT_RECEIPTS = 4096
    MAX_U32 = (1 << 32) - 1
    MAX_U64 = (1 << 64) - 1

    @classmethod
    def parse(cls, payload: Any) -> SumeragiV2Status:
        record = cls._mapping(payload, "sumeragi status")
        allowed_fields = {
            "protocol_version",
            "node_fingerprint",
            "build_fingerprint",
            "config_fingerprint",
            "restart_required",
            "height_context_id",
            "height",
            "view",
            "phase",
            "leader",
            "locked_prepare_qc",
            "highest_prepare_qc",
            "last_timeout_certificate",
            "body_state",
            "pending_persistence_id",
            "last_committed_height",
            "last_committed_subject",
            "height_context",
            "last_commit_qc",
            "liveness",
        }
        unknown_fields = set(record) - allowed_fields
        if unknown_fields:
            unknown = sorted(unknown_fields)[0]
            raise RuntimeError(f"sumeragi status contains unknown field {unknown}")
        protocol_version = cls._unsigned(
            record.get("protocol_version"),
            "sumeragi.protocol_version",
            maximum=0xFFFF,
        )
        if protocol_version != 4:
            raise RuntimeError("sumeragi.protocol_version must equal 4")

        height = cls._unsigned(record.get("height"), "sumeragi.height")
        view = cls._unsigned(record.get("view"), "sumeragi.view")
        height_context_id = cls._context_id(
            record.get("height_context_id"), "sumeragi.height_context_id"
        )
        leader = cls._unsigned(
            record.get("leader"), "sumeragi.leader", maximum=cls.MAX_U32
        )
        restart_required = cls._boolean(
            record.get("restart_required"), "sumeragi.restart_required"
        )
        height_context = cls._height_context(
            record.get("height_context"), context="sumeragi.height_context"
        )
        if height_context.epoch_end_height < height:
            raise RuntimeError(
                "sumeragi.height_context.epoch_end_height must cover height"
            )
        if leader >= height_context.validator_count:
            raise RuntimeError(
                "sumeragi.leader must index the frozen validator roster"
            )

        last_committed_height = cls._unsigned(
            record.get("last_committed_height"),
            "sumeragi.last_committed_height",
        )
        if last_committed_height > height:
            raise RuntimeError(
                "sumeragi.last_committed_height must not exceed height"
            )
        last_subject_value = record.get("last_committed_subject")
        last_subject = (
            None
            if last_subject_value is None
            else cls._subject(
                last_subject_value, context="sumeragi.last_committed_subject"
            )
        )
        last_commit_value = record.get("last_commit_qc")
        last_commit = (
            None
            if last_commit_value is None
            else cls._commit_qc(last_commit_value, context="sumeragi.last_commit_qc")
        )
        if last_committed_height == 0:
            if last_subject is not None or last_commit is not None:
                raise RuntimeError(
                    "sumeragi committed subject and QC must be absent at height zero"
                )
        else:
            if (last_subject is None) != (last_commit is None):
                raise RuntimeError(
                    "sumeragi committed subject and QC are required together when either is present after height zero"
                )
            if (
                last_subject is not None
                and last_commit is not None
                and (
                    last_commit.certificate.phase != "commit"
                    or last_commit.certificate.round.height != last_committed_height
                    or last_commit.certificate.subject != last_subject
                )
            ):
                raise RuntimeError(
                    "sumeragi.last_commit_qc does not certify the committed subject"
                )

        liveness = cls._liveness(
            record.get("liveness"),
            context="sumeragi.liveness",
            height=height,
            view=view,
            context_id=height_context_id,
            height_context=height_context,
        )

        return SumeragiV2Status(
            protocol_version=protocol_version,
            node_fingerprint=cls._hash(
                record.get("node_fingerprint"), "sumeragi.node_fingerprint"
            ),
            build_fingerprint=cls._hash(
                record.get("build_fingerprint"), "sumeragi.build_fingerprint"
            ),
            config_fingerprint=cls._hash(
                record.get("config_fingerprint"), "sumeragi.config_fingerprint"
            ),
            restart_required=restart_required,
            height_context_id=height_context_id,
            height=height,
            view=view,
            phase=cls._tagged(
                record.get("phase"),
                tag="phase",
                allowed={
                    "awaiting_proposal",
                    "reconstructing_payload",
                    "validating_payload",
                    "prepare",
                    "commit",
                    "pending_apply",
                },
                context="sumeragi.phase",
            ),
            leader=leader,
            locked_prepare_qc=cls._optional_qc(
                record.get("locked_prepare_qc"),
                context="sumeragi.locked_prepare_qc",
            ),
            highest_prepare_qc=cls._optional_qc(
                record.get("highest_prepare_qc"),
                context="sumeragi.highest_prepare_qc",
            ),
            last_timeout_certificate=cls._optional_timeout(
                record.get("last_timeout_certificate"),
                context="sumeragi.last_timeout_certificate",
            ),
            body_state=cls._tagged(
                record.get("body_state"),
                tag="state",
                allowed={
                    "missing",
                    "reconstructing",
                    "stored",
                    "validated",
                    "pending_apply",
                    "applied",
                },
                context="sumeragi.body_state",
            ),
            pending_persistence_id=cls._optional_unsigned(
                record.get("pending_persistence_id"),
                "sumeragi.pending_persistence_id",
            ),
            last_committed_height=last_committed_height,
            last_committed_subject=last_subject,
            height_context=height_context,
            last_commit_qc=last_commit,
            liveness=liveness,
        )

    @classmethod
    def _liveness(
        cls,
        value: Any,
        *,
        context: str,
        height: int,
        view: int,
        context_id: Tuple[str],
        height_context: SumeragiV2HeightContextStatus,
    ) -> SumeragiV2LivenessStatus:
        record = cls._mapping(value, context)
        liveness_fields = {
            "generation",
            "prepare_quorums",
            "commit_quorums",
            "timeout_quorums",
            "outbound_intents",
            "work",
            "queues",
            "last_progress",
            "no_progress_age_ms",
            "blocker",
            "ignore_counts",
        }
        unknown_liveness_fields = set(record) - liveness_fields
        if unknown_liveness_fields:
            raise RuntimeError(
                f"{context} contains unknown field {sorted(unknown_liveness_fields)[0]}"
            )
        missing_liveness_fields = (
            liveness_fields - {"last_progress", "blocker"} - set(record)
        )
        if missing_liveness_fields:
            raise RuntimeError(
                f"{context} is missing required field {sorted(missing_liveness_fields)[0]}"
            )
        generation = cls._unsigned(record.get("generation"), f"{context}.generation")

        def bound_round(raw: Any, round_context: str) -> SumeragiV2Round:
            parsed = cls._round(raw, context=round_context)
            if parsed.context_id != context_id or parsed.height != height:
                raise RuntimeError(f"{round_context} must match the active height context")
            return parsed

        def checked_round(raw: Any, round_context: str) -> SumeragiV2Round:
            parsed = bound_round(raw, round_context)
            if parsed.view > view:
                raise RuntimeError(f"{round_context}.view must not exceed the active view")
            return parsed

        def vote_quorum(
            raw: Any, quorum_context: str, *, phase: str
        ) -> SumeragiV2VoteQuorumStatus:
            quorum = cls._exact_mapping(
                raw,
                quorum_context,
                {
                    "round",
                    "proposal_round",
                    "subject",
                    "execution_commitment",
                    "signer_count",
                    "signed_power",
                    "min_signers",
                    "total_power",
                },
            )
            signer_count = cls._unsigned(
                quorum.get("signer_count"),
                f"{quorum_context}.signer_count",
                maximum=height_context.validator_count,
            )
            min_signers = cls._unsigned(
                quorum.get("min_signers"),
                f"{quorum_context}.min_signers",
                maximum=cls.MAX_U32,
            )
            signed_power = cls._unsigned(
                quorum.get("signed_power"), f"{quorum_context}.signed_power"
            )
            total_power = cls._unsigned(
                quorum.get("total_power"), f"{quorum_context}.total_power"
            )
            if (
                min_signers != height_context.min_signers
                or total_power != height_context.total_power
                or signed_power != signer_count
            ):
                raise RuntimeError(f"{quorum_context} disagrees with the frozen dual quorum")
            round_ = checked_round(quorum.get("round"), f"{quorum_context}.round")
            proposal_round = checked_round(
                quorum.get("proposal_round"), f"{quorum_context}.proposal_round"
            )
            cls._validate_proposal_round(
                proposal_round,
                round_,
                context=quorum_context,
            )
            return SumeragiV2VoteQuorumStatus(
                round=round_,
                proposal_round=proposal_round,
                subject=cls._subject(
                    quorum.get("subject"), context=f"{quorum_context}.subject"
                ),
                execution_commitment=cls._execution_commitment(
                    quorum.get("execution_commitment"),
                    context=f"{quorum_context}.execution_commitment",
                ),
                signer_count=signer_count,
                signed_power=signed_power,
                min_signers=min_signers,
                total_power=total_power,
            )

        def vote_quorums(field: str, *, phase: str) -> List[SumeragiV2VoteQuorumStatus]:
            raw_values = cls._array(
                record.get(field),
                f"{context}.{field}",
                maximum=(
                    cls.MAX_CONSENSUS_VALIDATORS + 1
                    if phase == "commit"
                    else cls.MAX_CONSENSUS_VALIDATORS
                ),
            )
            return [
                vote_quorum(item, f"{context}.{field}[{index}]", phase=phase)
                for index, item in enumerate(raw_values)
            ]

        raw_timeouts = cls._array(
            record.get("timeout_quorums"),
            f"{context}.timeout_quorums",
            maximum=cls.MAX_CONSENSUS_VALIDATORS,
        )
        timeout_quorums: List[SumeragiV2TimeoutQuorumStatus] = []
        for index, raw in enumerate(raw_timeouts):
            item_context = f"{context}.timeout_quorums[{index}]"
            item = cls._exact_mapping(
                raw,
                item_context,
                {
                    "round",
                    "signer_count",
                    "signed_power",
                    "min_signers",
                    "total_power",
                    "certificate_formed",
                },
            )
            signer_count = cls._unsigned(
                item.get("signer_count"),
                f"{item_context}.signer_count",
                maximum=height_context.validator_count,
            )
            signed_power = cls._unsigned(
                item.get("signed_power"), f"{item_context}.signed_power"
            )
            min_signers = cls._unsigned(
                item.get("min_signers"), f"{item_context}.min_signers"
            )
            total_power = cls._unsigned(
                item.get("total_power"), f"{item_context}.total_power"
            )
            formed = cls._boolean(
                item.get("certificate_formed"), f"{item_context}.certificate_formed"
            )
            if (
                min_signers != height_context.min_signers
                or total_power != height_context.total_power
                or signed_power != signer_count
                or (
                    formed
                    and (
                        signer_count < min_signers
                        or signed_power * 3 <= total_power * 2
                    )
                )
            ):
                raise RuntimeError(f"{item_context} is not a valid partial timeout quorum")
            timeout_quorums.append(
                SumeragiV2TimeoutQuorumStatus(
                    round=checked_round(item.get("round"), f"{item_context}.round"),
                    signer_count=signer_count,
                    signed_power=signed_power,
                    min_signers=min_signers,
                    total_power=total_power,
                    certificate_formed=formed,
                )
            )

        raw_outbound = cls._array(
            record.get("outbound_intents"), f"{context}.outbound_intents", maximum=7
        )
        outbound_intents: List[SumeragiV2OutboundIntentStatus] = []
        proposal_kinds = {
            "proposal",
            "prepare_vote",
            "commit_vote",
            "prepare_qc",
            "commit_qc",
        }
        for index, raw in enumerate(raw_outbound):
            item_context = f"{context}.outbound_intents[{index}]"
            item = cls._mapping(raw, item_context)
            outbound_fields = {
                "kind",
                "round",
                "proposal_round",
                "subject",
                "execution_commitment",
                "stage",
            }
            if set(item) - outbound_fields:
                raise RuntimeError(f"{item_context} contains an unknown field")
            if {"kind", "round", "stage"} - set(item):
                raise RuntimeError(f"{item_context} is missing a required field")
            kind = cls._tagged(
                item.get("kind"),
                tag="kind",
                allowed=proposal_kinds | {"timeout_vote", "timeout_certificate"},
                context=f"{item_context}.kind",
            )
            stage = cls._tagged(
                item.get("stage"),
                tag="stage",
                allowed={"pending_persistence", "pending_signature", "queued", "sent"},
                context=f"{item_context}.stage",
            )
            raw_subject = item.get("subject")
            raw_execution = item.get("execution_commitment")
            raw_proposal_round = item.get("proposal_round")
            carries_proposal_round = kind in proposal_kinds
            if carries_proposal_round != (raw_proposal_round is not None):
                raise RuntimeError(
                    f"{item_context} has inconsistent proposal_round for {kind}"
                )
            shape_is_valid = (
                (kind == "proposal" and raw_subject is not None and raw_execution is None)
                or (
                    kind in proposal_kinds - {"proposal"}
                    and raw_subject is not None
                    and raw_execution is not None
                )
                or (
                    kind not in proposal_kinds
                    and raw_subject is None
                    and raw_execution is None
                )
            )
            if not shape_is_valid:
                raise RuntimeError(f"{item_context} has inconsistent proposal fields")
            round_ = bound_round(item.get("round"), f"{item_context}.round")
            if kind != "commit_qc" and round_.view > view:
                raise RuntimeError(
                    f"{item_context}.round.view must not exceed the active view"
                )
            proposal_round = (
                None
                if raw_proposal_round is None
                else bound_round(
                    raw_proposal_round, f"{item_context}.proposal_round"
                )
            )
            if proposal_round is not None:
                cls._validate_proposal_round(
                    proposal_round,
                    round_,
                    context=item_context,
                )
            outbound_intents.append(
                SumeragiV2OutboundIntentStatus(
                    kind=kind,
                    round=round_,
                    proposal_round=proposal_round,
                    subject=(
                        None
                        if raw_subject is None
                        else cls._subject(raw_subject, context=f"{item_context}.subject")
                    ),
                    execution_commitment=(
                        None
                        if raw_execution is None
                        else cls._execution_commitment(
                            raw_execution,
                            context=f"{item_context}.execution_commitment",
                        )
                    ),
                    stage=stage,
                )
            )

        raw_work = cls._exact_mapping(
            record.get("work"),
            f"{context}.work",
            {
                "candidate",
                "body_recovery",
                "body_store",
                "validation",
                "application",
                "successor_height",
            },
        )
        work_stage_names = {"idle", "queued", "running", "complete"}
        parsed_work = {
            field: cls._tagged(
                raw_work.get(field),
                tag="stage",
                allowed=work_stage_names,
                context=f"{context}.work.{field}",
            )
            for field in raw_work
        }
        work = SumeragiV2WorkStatus(**parsed_work)

        raw_queues = cls._array(record.get("queues"), f"{context}.queues", maximum=10)
        queues: List[SumeragiV2QueueLivenessStatus] = []
        queue_names: set[str] = set()
        for index, raw in enumerate(raw_queues):
            item_context = f"{context}.queues[{index}]"
            item = cls._mapping(raw, item_context)
            queue_fields = {"queue", "depth", "capacity", "oldest_age_ms", "service_debt"}
            if set(item) - queue_fields:
                raise RuntimeError(f"{item_context} contains an unknown field")
            if {"queue", "depth", "capacity", "service_debt"} - set(item):
                raise RuntimeError(f"{item_context} is missing a required field")
            queue = cls._tagged(
                item.get("queue"),
                tag="queue",
                allowed={
                    "ingress",
                    "deferred_normal",
                    "deferred_progress",
                    "deferred_completion",
                    "runtime_normal",
                    "runtime_progress",
                    "runtime_completion",
                    "effect_completion",
                    "network_ingress",
                    "effect_dispatch",
                },
                context=f"{item_context}.queue",
            )
            if queue in queue_names:
                raise RuntimeError(f"{item_context}.queue is duplicated")
            queue_names.add(queue)
            depth = cls._unsigned(
                item.get("depth"), f"{item_context}.depth", maximum=cls.MAX_U32
            )
            capacity = cls._unsigned(
                item.get("capacity"),
                f"{item_context}.capacity",
                positive=True,
                maximum=cls.MAX_U32,
            )
            oldest_age = cls._optional_unsigned(
                item.get("oldest_age_ms"), f"{item_context}.oldest_age_ms"
            )
            if depth > capacity or (depth == 0) != (oldest_age is None):
                raise RuntimeError(f"{item_context} has inconsistent occupancy and age")
            queues.append(
                SumeragiV2QueueLivenessStatus(
                    queue=queue,
                    depth=depth,
                    capacity=capacity,
                    oldest_age_ms=oldest_age,
                    service_debt=cls._unsigned(
                        item.get("service_debt"), f"{item_context}.service_debt"
                    ),
                )
            )

        raw_progress = record.get("last_progress")
        last_progress: Optional[SumeragiV2ProgressTransitionStatus] = None
        if raw_progress is not None:
            progress = cls._exact_mapping(
                raw_progress,
                f"{context}.last_progress",
                {"generation", "round", "transition", "age_ms"},
            )
            progress_generation = cls._unsigned(
                progress.get("generation"), f"{context}.last_progress.generation"
            )
            if progress_generation > generation:
                raise RuntimeError(f"{context}.last_progress.generation is from the future")
            last_progress = SumeragiV2ProgressTransitionStatus(
                generation=progress_generation,
                round=checked_round(
                    progress.get("round"), f"{context}.last_progress.round"
                ),
                transition=cls._tagged(
                    progress.get("transition"),
                    tag="transition",
                    allowed={
                        "proposal_admitted",
                        "body_available",
                        "body_stored",
                        "body_validated",
                        "prepare_vote_admitted",
                        "commit_vote_admitted",
                        "timeout_vote_admitted",
                        "prepare_quorum",
                        "lock_installed",
                        "commit_quorum",
                        "timeout_certificate_installed",
                        "decision_persisted",
                        "applied",
                        "successor_height_activated",
                        "recovery_replayed",
                    },
                    context=f"{context}.last_progress.transition",
                ),
                age_ms=cls._unsigned(
                    progress.get("age_ms"), f"{context}.last_progress.age_ms"
                ),
            )

        raw_blocker = record.get("blocker")
        blocker = (
            None
            if raw_blocker is None
            else cls._tagged(
                raw_blocker,
                tag="blocker",
                allowed={
                    "missing_proposal",
                    "body_unavailable",
                    "prepare_quorum_missing",
                    "commit_quorum_missing",
                    "timeout_certificate_missing",
                    "scheduler_starvation",
                    "application_pending",
                    "successor_activation_pending",
                    "local_control_pending",
                },
                context=f"{context}.blocker",
            )
        )

        raw_ignore = cls._array(
            record.get("ignore_counts"), f"{context}.ignore_counts", maximum=12
        )
        ignore_counts: List[SumeragiV2IgnoreCount] = []
        ignore_reasons: set[str] = set()
        allowed_ignore_reasons = {
            "wrong_height",
            "wrong_view",
            "stale_generation",
            "busy",
            "duplicate",
            "no_matching_work",
            "observer",
            "view_closed",
            "already_decided",
            "recovery_pending",
            "irrelevant_view",
            "unsafe_proposal",
        }
        for index, raw in enumerate(raw_ignore):
            item_context = f"{context}.ignore_counts[{index}]"
            item = cls._exact_mapping(raw, item_context, {"reason", "count"})
            reason = cls._tagged(
                item.get("reason"),
                tag="reason",
                allowed=allowed_ignore_reasons,
                context=f"{item_context}.reason",
            )
            if reason in ignore_reasons:
                raise RuntimeError(f"{item_context}.reason is duplicated")
            ignore_reasons.add(reason)
            ignore_counts.append(
                SumeragiV2IgnoreCount(
                    reason=reason,
                    count=cls._unsigned(item.get("count"), f"{item_context}.count"),
                )
            )

        return SumeragiV2LivenessStatus(
            generation=generation,
            prepare_quorums=vote_quorums("prepare_quorums", phase="prepare"),
            commit_quorums=vote_quorums("commit_quorums", phase="commit"),
            timeout_quorums=timeout_quorums,
            outbound_intents=outbound_intents,
            work=work,
            queues=queues,
            last_progress=last_progress,
            no_progress_age_ms=cls._unsigned(
                record.get("no_progress_age_ms"), f"{context}.no_progress_age_ms"
            ),
            blocker=blocker,
            ignore_counts=ignore_counts,
        )

    @staticmethod
    def _mapping(value: Any, context: str) -> Mapping[str, Any]:
        if not isinstance(value, Mapping):
            raise RuntimeError(f"{context} must be a JSON object")
        return value

    @staticmethod
    def _array(
        value: Any,
        context: str,
        *,
        minimum: int = 0,
        maximum: Optional[int] = None,
    ) -> List[Any]:
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be an array")
        if len(value) < minimum:
            raise RuntimeError(f"{context} contains fewer than {minimum} items")
        if maximum is not None and len(value) > maximum:
            raise RuntimeError(f"{context} exceeds its protocol item bound")
        return value

    @classmethod
    def _exact_mapping(
        cls,
        value: Any,
        context: str,
        expected_fields: set[str],
    ) -> Mapping[str, Any]:
        record = cls._mapping(value, context)
        if set(record) != expected_fields:
            missing = sorted(expected_fields - set(record))
            unknown = sorted(set(record) - expected_fields)
            if missing:
                raise RuntimeError(f"{context} is missing required field {missing[0]}")
            raise RuntimeError(f"{context} contains unknown field {unknown[0]}")
        return record

    @staticmethod
    def _clone_mapping(value: Any, context: str) -> Dict[str, Any]:
        record = _SumeragiV2StatusParser._mapping(value, context)
        try:
            cloned = json.loads(json.dumps(record))
        except (TypeError, ValueError) as exc:
            raise RuntimeError(f"{context} must be JSON-serialisable") from exc
        if not isinstance(cloned, dict):
            raise RuntimeError(f"{context} must be a JSON object")
        return cloned

    @classmethod
    def _unsigned(
        cls,
        value: Any,
        context: str,
        *,
        positive: bool = False,
        maximum: Optional[int] = None,
    ) -> int:
        if isinstance(value, bool) or not isinstance(value, int):
            raise RuntimeError(f"{context} must be an integer")
        number = value
        if number < 0 or (positive and number == 0):
            qualifier = "positive" if positive else "non-negative"
            raise RuntimeError(f"{context} must be {qualifier}")
        if maximum is not None and number > maximum:
            raise RuntimeError(f"{context} exceeds its protocol bound")
        return number

    @classmethod
    def _exact_unsigned(
        cls,
        value: Any,
        context: str,
        *,
        positive: bool = False,
        maximum: Optional[int] = None,
    ) -> int:
        if isinstance(value, bool) or not isinstance(value, int):
            raise RuntimeError(f"{context} must be an unsigned integer")
        return cls._unsigned(
            value,
            context,
            positive=positive,
            maximum=maximum,
        )

    @classmethod
    def _optional_unsigned(cls, value: Any, context: str) -> Optional[int]:
        if value is None:
            return None
        return cls._unsigned(value, context)

    @staticmethod
    def _boolean(value: Any, context: str) -> bool:
        if not isinstance(value, bool):
            raise RuntimeError(f"{context} must be a boolean")
        return value

    @classmethod
    def _quantity(cls, value: Any, context: str) -> str:
        return _canonical_quantity(value, context)

    @staticmethod
    def _crc16(tag: str, body: str) -> int:
        crc = 0xFFFF
        for byte in f"{tag}:{body}".encode("ascii"):
            crc ^= byte << 8
            for _ in range(8):
                crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
        return crc

    @classmethod
    def _hash(cls, value: Any, context: str) -> str:
        if not isinstance(value, str):
            raise RuntimeError(f"{context} must be a canonical hash literal")
        match = re.fullmatch(r"hash:([0-9A-F]{64})#([0-9A-F]{4})", value)
        if match is None:
            raise RuntimeError(f"{context} must be a canonical hash literal")
        body, checksum = match.groups()
        if cls._crc16("hash", body) != int(checksum, 16):
            raise RuntimeError(f"{context} has an invalid hash checksum")
        if bytes.fromhex(body)[-1] & 1 == 0:
            raise RuntimeError(f"{context} has an invalid Iroha hash marker bit")
        return value

    @classmethod
    def _safety_halt(cls, value: Any, context: str) -> SumeragiSafetyHaltStatus:
        record = cls._mapping(value, context)
        allowed_fields = {
            "active",
            "reason",
            "height",
            "epoch",
            "first_block_hash",
            "conflicting_block_hash",
            "first_parent_state_root",
            "first_post_state_root",
            "conflicting_parent_state_root",
            "conflicting_post_state_root",
        }
        unknown_fields = set(record) - allowed_fields
        if unknown_fields:
            unknown = sorted(unknown_fields)[0]
            raise RuntimeError(f"{context} contains unknown field {unknown}")

        reason = record.get("reason")
        if reason is not None and not isinstance(reason, str):
            raise RuntimeError(f"{context}.reason must be a string or null")

        def optional_hash(field: str) -> Optional[str]:
            field_value = record.get(field)
            if field_value is None:
                return None
            return cls._hash(field_value, f"{context}.{field}")

        return SumeragiSafetyHaltStatus(
            active=cls._boolean(record.get("active"), f"{context}.active"),
            reason=reason,
            height=cls._unsigned(record.get("height"), f"{context}.height"),
            epoch=cls._unsigned(record.get("epoch"), f"{context}.epoch"),
            first_block_hash=optional_hash("first_block_hash"),
            conflicting_block_hash=optional_hash("conflicting_block_hash"),
            first_parent_state_root=optional_hash("first_parent_state_root"),
            first_post_state_root=optional_hash("first_post_state_root"),
            conflicting_parent_state_root=optional_hash("conflicting_parent_state_root"),
            conflicting_post_state_root=optional_hash("conflicting_post_state_root"),
        )

    @classmethod
    def _nonzero_hash(cls, value: Any, context: str) -> str:
        literal = cls._hash(value, context)
        body = literal[5:69]
        if int(body, 16) == 0:
            raise RuntimeError(f"{context} must not be the zero hash")
        return literal

    @classmethod
    def _byte32(cls, value: Any, context: str) -> str:
        if isinstance(value, str) and re.fullmatch(r"[0-9A-F]{64}", value):
            return value
        raise RuntimeError(f"{context} must be canonical uppercase 32-byte hex")

    @classmethod
    def _byte_vector(cls, value: Any, length: int, context: str) -> List[int]:
        items = cls._array(value, context, minimum=length, maximum=length)
        result: List[int] = []
        for index, item in enumerate(items):
            if isinstance(item, bool) or not isinstance(item, int) or not 0 <= item <= 255:
                raise RuntimeError(f"{context}[{index}] must be an integer byte")
            result.append(item)
        return result

    @classmethod
    def _context_id(cls, value: Any, context: str) -> Tuple[str]:
        items = cls._array(value, context)
        if len(items) != 1:
            raise RuntimeError(f"{context} must be a one-element hash tuple")
        return (cls._hash(items[0], f"{context}[0]"),)

    @classmethod
    def _tagged(
        cls, value: Any, *, tag: str, allowed: set[str], context: str
    ) -> str:
        return cls._tagged_content(
            value,
            tag=tag,
            content="details",
            allowed=allowed,
            context=context,
        )

    @classmethod
    def _tagged_content(
        cls,
        value: Any,
        *,
        tag: str,
        content: str,
        allowed: set[str],
        context: str,
    ) -> str:
        record = cls._mapping(value, context)
        unknown = set(record) - {tag, content}
        if unknown:
            raise RuntimeError(f"{context} contains an unknown tagged-enum field")
        if tag not in record:
            raise RuntimeError(f"{context}.{tag} is required")
        variant = record.get(tag)
        if not isinstance(variant, str) or variant not in allowed:
            raise RuntimeError(f"{context}.{tag} is not a supported v2 variant")
        if content not in record or record.get(content) is not None:
            raise RuntimeError(f"{context}.{content} must be explicitly null")
        return variant

    @classmethod
    def _round(cls, value: Any, *, context: str) -> SumeragiV2Round:
        record = cls._mapping(value, context)
        return SumeragiV2Round(
            context_id=cls._context_id(record.get("context_id"), f"{context}.context_id"),
            height=cls._unsigned(record.get("height"), f"{context}.height"),
            view=cls._unsigned(record.get("view"), f"{context}.view"),
        )

    @staticmethod
    def _validate_proposal_round(
        proposal_round: SumeragiV2Round,
        round_: SumeragiV2Round,
        *,
        context: str,
    ) -> None:
        if (
            proposal_round.context_id != round_.context_id
            or proposal_round.height != round_.height
        ):
            raise RuntimeError(
                f"{context}.proposal_round must match round context and height"
            )
        if proposal_round != round_:
            raise RuntimeError(f"{context}.proposal_round must equal round")

    @classmethod
    def _subject(cls, value: Any, *, context: str) -> SumeragiV2BlockSubject:
        record = cls._mapping(value, context)
        parent_value = record.get("parent_block_hash")
        return SumeragiV2BlockSubject(
            parent_block_hash=(
                None
                if parent_value is None
                else cls._hash(parent_value, f"{context}.parent_block_hash")
            ),
            block_hash=cls._hash(record.get("block_hash"), f"{context}.block_hash"),
            payload_hash=cls._hash(
                record.get("payload_hash"), f"{context}.payload_hash"
            ),
        )

    @classmethod
    def _qc(cls, value: Any, *, context: str) -> SumeragiV2QcReference:
        record = cls._mapping(value, context)
        round_ = cls._round(record.get("round"), context=f"{context}.round")
        proposal_round = cls._round(
            record.get("proposal_round"), context=f"{context}.proposal_round"
        )
        phase = cls._tagged(
            record.get("phase"),
            tag="phase",
            allowed={"prepare", "commit"},
            context=f"{context}.phase",
        )
        cls._validate_proposal_round(
            proposal_round,
            round_,
            context=context,
        )
        return SumeragiV2QcReference(
            round=round_,
            proposal_round=proposal_round,
            phase=phase,
            subject=cls._subject(record.get("subject"), context=f"{context}.subject"),
            execution_commitment=cls._execution_commitment(
                record.get("execution_commitment"),
                context=f"{context}.execution_commitment",
            ),
        )

    @classmethod
    def _execution_commitment(
        cls, value: Any, *, context: str
    ) -> SumeragiV2ExecutionCommitment:
        record = cls._mapping(value, context)
        allowed_fields = {
            "parent_state_root",
            "post_state_root",
            "ordinary_writes_root",
            "topup_anchor_root",
            "topup_anchor_count",
            "native_amx_application_manifest_version",
            "native_amx_application_manifest_root",
            "native_amx_application_manifest_count",
            "lane_finality_manifest",
            "merge_carrier",
            "executed_block_wire_len",
            "executed_block_wire_hash",
        }
        unknown = set(record) - allowed_fields
        if unknown:
            raise RuntimeError(f"{context} contains unknown field {sorted(unknown)[0]}")
        for required_field in ("lane_finality_manifest", "merge_carrier"):
            if required_field not in record:
                raise RuntimeError(f"{context}.{required_field} is required")
        topup_count = cls._unsigned(
            record.get("topup_anchor_count"),
            f"{context}.topup_anchor_count",
            maximum=16,
        )
        raw_topup_root = record.get("topup_anchor_root")
        topup_root = (
            None
            if raw_topup_root is None
            else cls._hash(raw_topup_root, f"{context}.topup_anchor_root")
        )
        if (topup_count == 0) != (topup_root is None):
            raise RuntimeError(
                f"{context}.topup_anchor_root must be present exactly when topup_anchor_count is positive"
            )
        native_manifest_version = cls._unsigned(
            record.get("native_amx_application_manifest_version"),
            f"{context}.native_amx_application_manifest_version",
            maximum=(1 << 16) - 1,
        )
        if native_manifest_version != _SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION:
            raise RuntimeError(
                f"{context}.native_amx_application_manifest_version must equal "
                f"{_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION}"
            )
        native_manifest_root = cls._hash(
            record.get("native_amx_application_manifest_root"),
            f"{context}.native_amx_application_manifest_root",
        )
        native_manifest_count = cls._unsigned(
            record.get("native_amx_application_manifest_count"),
            f"{context}.native_amx_application_manifest_count",
            maximum=_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES,
        )
        if (native_manifest_count == 0) != (
            native_manifest_root
            == _SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT
        ):
            raise RuntimeError(
                f"{context}.native_amx_application_manifest_count must be zero exactly "
                "for the canonical empty root"
            )
        lane_finality_manifest = _lane_finality_manifest_commitment(
            record["lane_finality_manifest"], f"{context}.lane_finality_manifest"
        )
        raw_merge_carrier = record["merge_carrier"]
        merge_carrier: Optional[SumeragiV2MergeCarrierCommitment]
        if raw_merge_carrier is None:
            merge_carrier = None
        else:
            merge_context = f"{context}.merge_carrier"
            merge_record = cls._mapping(raw_merge_carrier, merge_context)
            merge_fields = {"version", "entry_hash"}
            missing_merge = merge_fields - set(merge_record)
            if missing_merge:
                raise RuntimeError(
                    f"{merge_context}.{sorted(missing_merge)[0]} is required"
                )
            unknown_merge = set(merge_record) - merge_fields
            if unknown_merge:
                raise RuntimeError(
                    f"{merge_context} contains unknown field {sorted(unknown_merge)[0]}"
                )
            merge_version = cls._unsigned(
                merge_record["version"],
                f"{merge_context}.version",
                maximum=(1 << 16) - 1,
            )
            if merge_version != _SUMERAGI_MERGE_CARRIER_COMMITMENT_VERSION:
                raise RuntimeError(
                    f"{merge_context}.version must equal "
                    f"{_SUMERAGI_MERGE_CARRIER_COMMITMENT_VERSION}"
                )
            merge_carrier = SumeragiV2MergeCarrierCommitment(
                version=1,
                entry_hash=cls._hash(
                    merge_record["entry_hash"], f"{merge_context}.entry_hash"
                ),
            )
        return SumeragiV2ExecutionCommitment(
            parent_state_root=cls._hash(
                record.get("parent_state_root"), f"{context}.parent_state_root"
            ),
            post_state_root=cls._hash(
                record.get("post_state_root"), f"{context}.post_state_root"
            ),
            ordinary_writes_root=cls._hash(
                record.get("ordinary_writes_root"),
                f"{context}.ordinary_writes_root",
            ),
            topup_anchor_root=topup_root,
            topup_anchor_count=topup_count,
            native_amx_application_manifest_version=native_manifest_version,
            native_amx_application_manifest_root=native_manifest_root,
            native_amx_application_manifest_count=native_manifest_count,
            lane_finality_manifest=lane_finality_manifest,
            merge_carrier=merge_carrier,
            executed_block_wire_len=cls._unsigned(
                record.get("executed_block_wire_len"),
                f"{context}.executed_block_wire_len",
                positive=True,
                maximum=cls.MAX_U64,
            ),
            executed_block_wire_hash=cls._hash(
                record.get("executed_block_wire_hash"),
                f"{context}.executed_block_wire_hash",
            ),
        )

    @classmethod
    def _optional_qc(
        cls, value: Any, *, context: str
    ) -> Optional[SumeragiV2QcReference]:
        return None if value is None else cls._qc(value, context=context)

    @classmethod
    def _optional_timeout(
        cls, value: Any, *, context: str
    ) -> Optional[SumeragiV2TimeoutReference]:
        if value is None:
            return None
        record = cls._mapping(value, context)
        return SumeragiV2TimeoutReference(
            round=cls._round(record.get("round"), context=f"{context}.round"),
            highest_prepare_qc=cls._optional_qc(
                record.get("highest_prepare_qc"),
                context=f"{context}.highest_prepare_qc",
            ),
            certificate_hash=cls._hash(
                record.get("certificate_hash"), f"{context}.certificate_hash"
            ),
        )

    @classmethod
    def _height_context(
        cls, value: Any, *, context: str
    ) -> SumeragiV2HeightContextStatus:
        record = cls._mapping(value, context)
        validator_count = cls._unsigned(
            record.get("validator_count"),
            f"{context}.validator_count",
            positive=True,
            maximum=cls.MAX_CONSENSUS_VALIDATORS,
        )
        quorum = cls._mapping(record.get("quorum"), f"{context}.quorum")
        min_signers = cls._unsigned(
            quorum.get("min_signers"),
            f"{context}.quorum.min_signers",
            positive=True,
            maximum=cls.MAX_CONSENSUS_VALIDATORS,
        )
        total_power = cls._unsigned(
            quorum.get("total_power"),
            f"{context}.quorum.total_power",
            positive=True,
        )
        expected_min = validator_count * 2 // 3 + 1
        if (
            validator_count < 4
            or (validator_count - 1) % 3 != 0
            or min_signers != expected_min
            or total_power != validator_count
        ):
            raise RuntimeError(
                f"{context}.quorum is not canonical for validator_count"
            )
        mode = cls._tagged(
            record.get("mode"),
            tag="mode",
            allowed={"permissioned", "npos"},
            context=f"{context}.mode",
        )
        return SumeragiV2HeightContextStatus(
            epoch=cls._unsigned(record.get("epoch"), f"{context}.epoch"),
            epoch_end_height=cls._unsigned(
                record.get("epoch_end_height"), f"{context}.epoch_end_height"
            ),
            mode=mode,
            epoch_seed=cls._byte32(record.get("epoch_seed"), f"{context}.epoch_seed"),
            validator_count=validator_count,
            min_signers=min_signers,
            total_power=total_power,
        )

    @classmethod
    def _commit_qc(cls, value: Any, *, context: str) -> SumeragiV2CommitQcStatus:
        record = cls._mapping(value, context)
        validator_count = cls._unsigned(
            record.get("validator_count"),
            f"{context}.validator_count",
            positive=True,
            maximum=cls.MAX_CONSENSUS_VALIDATORS,
        )
        signer_count = cls._unsigned(record.get("signer_count"), f"{context}.signer_count")
        min_signers = cls._unsigned(
            record.get("min_signers"),
            f"{context}.min_signers",
            positive=True,
            maximum=cls.MAX_CONSENSUS_VALIDATORS,
        )
        signed_power = cls._unsigned(record.get("signed_power"), f"{context}.signed_power")
        total_power = cls._unsigned(
            record.get("total_power"), f"{context}.total_power", positive=True
        )
        if (
            validator_count < 4
            or (validator_count - 1) % 3 != 0
            or signer_count > validator_count
            or min_signers != validator_count * 2 // 3 + 1
            or signed_power != signer_count
            or total_power != validator_count
            or signer_count != min_signers
            or signed_power * 3 <= total_power * 2
        ):
            raise RuntimeError(f"{context} does not satisfy its exact frozen certificate quorum")
        return SumeragiV2CommitQcStatus(
            certificate=cls._qc(record.get("certificate"), context=f"{context}.certificate"),
            validator_count=validator_count,
            signer_count=signer_count,
            min_signers=min_signers,
            signed_power=signed_power,
            total_power=total_power,
        )

    @classmethod
    def _operator(cls, value: Any, *, context: str) -> SumeragiV2OperatorStatus:
        record = cls._mapping(value, context)
        adapter = cls._mapping(record.get("adapter_queues"), f"{context}.adapter_queues")
        adapter_status = SumeragiV2AdapterQueueStatus(
            ingress_keys=cls._unsigned(adapter.get("ingress_keys"), f"{context}.adapter_queues.ingress_keys"),
            ingress_capacity=cls._unsigned(adapter.get("ingress_capacity"), f"{context}.adapter_queues.ingress_capacity"),
            deferred_completion=cls._unsigned(adapter.get("deferred_completion"), f"{context}.adapter_queues.deferred_completion"),
            deferred_progress=cls._unsigned(adapter.get("deferred_progress"), f"{context}.adapter_queues.deferred_progress"),
            deferred_progress_capacity=cls._unsigned(adapter.get("deferred_progress_capacity"), f"{context}.adapter_queues.deferred_progress_capacity"),
            deferred_normal=cls._unsigned(adapter.get("deferred_normal"), f"{context}.adapter_queues.deferred_normal"),
            deferred_normal_capacity=cls._unsigned(adapter.get("deferred_normal_capacity"), f"{context}.adapter_queues.deferred_normal_capacity"),
        )
        if (
            adapter_status.ingress_keys > adapter_status.ingress_capacity
            or adapter_status.deferred_completion > adapter_status.deferred_progress_capacity
            or adapter_status.deferred_progress > adapter_status.deferred_progress_capacity
            or adapter_status.deferred_normal > adapter_status.deferred_normal_capacity
        ):
            raise RuntimeError(f"{context}.adapter_queues occupancy exceeds capacity")
        queue = cls._mapping(record.get("tx_queue"), f"{context}.tx_queue")
        tx_queue = SumeragiV2TxQueueStatus(
            tracked_transactions=cls._unsigned(queue.get("tracked_transactions"), f"{context}.tx_queue.tracked_transactions"),
            queued_transactions=cls._unsigned(queue.get("queued_transactions"), f"{context}.tx_queue.queued_transactions"),
            capacity=cls._unsigned(queue.get("capacity"), f"{context}.tx_queue.capacity", positive=True),
            retained_bytes=cls._unsigned(queue.get("retained_bytes"), f"{context}.tx_queue.retained_bytes"),
            max_retained_bytes=cls._unsigned(queue.get("max_retained_bytes"), f"{context}.tx_queue.max_retained_bytes", positive=True),
            oldest_queued_age_ms=cls._unsigned(queue.get("oldest_queued_age_ms"), f"{context}.tx_queue.oldest_queued_age_ms"),
            saturated_by_count=cls._boolean(queue.get("saturated_by_count"), f"{context}.tx_queue.saturated_by_count"),
            saturated_by_bytes=cls._boolean(queue.get("saturated_by_bytes"), f"{context}.tx_queue.saturated_by_bytes"),
            saturated_by_age=cls._boolean(queue.get("saturated_by_age"), f"{context}.tx_queue.saturated_by_age"),
        )
        if (
            tx_queue.queued_transactions > tx_queue.tracked_transactions
            or tx_queue.tracked_transactions > tx_queue.capacity
            or tx_queue.retained_bytes > tx_queue.max_retained_bytes
        ):
            raise RuntimeError(f"{context}.tx_queue occupancy exceeds capacity")
        return SumeragiV2OperatorStatus(
            view_change_install_total=cls._unsigned(
                record.get("view_change_install_total"),
                f"{context}.view_change_install_total",
            ),
            busy_deferral_total=cls._unsigned(
                record.get("busy_deferral_total"), f"{context}.busy_deferral_total"
            ),
            adapter_queues=adapter_status,
            tx_queue=tx_queue,
        )

    @classmethod
    def _settlements(cls, value: Any) -> List[Dict[str, Any]]:
        context = "sumeragi.lane_settlement_commitments"
        return [
            cls._settlement(entry, context=f"{context}[{index}]")
            for index, entry in enumerate(
                cls._array(
                    value,
                    context,
                    maximum=cls.MAX_LANE_SETTLEMENT_COMMITMENTS,
                )
            )
        ]

    @classmethod
    def _settlement(cls, value: Any, *, context: str) -> Dict[str, Any]:
        record = cls._exact_mapping(
            value,
            context,
            {
                "block_height",
                "lane_id",
                "lane_incarnation",
                "dataspace_id",
                "tx_count",
                "total_local_amount",
                "total_xor_due",
                "total_xor_after_haircut",
                "total_xor_variance",
                "swap_metadata",
                "receipts",
                "nexus_fee_receipts",
                "native_amx_receipts",
            },
        )
        block_height = cls._unsigned(
            record.get("block_height"), f"{context}.block_height"
        )
        lane_id = cls._unsigned(
            record.get("lane_id"), f"{context}.lane_id", maximum=cls.MAX_U32
        )
        lane_incarnation = cls._nonzero_hash(
            record.get("lane_incarnation"), f"{context}.lane_incarnation"
        )
        dataspace_id = cls._unsigned(
            record.get("dataspace_id"), f"{context}.dataspace_id"
        )
        receipts: List[Dict[str, Any]] = []
        for index, receipt_value in enumerate(
            cls._array(record.get("receipts"), f"{context}.receipts")
        ):
            receipt_context = f"{context}.receipts[{index}]"
            receipt = cls._exact_mapping(
                receipt_value,
                receipt_context,
                {
                    "source_id",
                    "local_amount",
                    "xor_due",
                    "xor_after_haircut",
                    "xor_variance",
                    "timestamp_ms",
                },
            )
            receipts.append(
                {
                    "source_id": cls._byte32(receipt.get("source_id"), f"{receipt_context}.source_id"),
                    "local_amount": cls._quantity(
                        receipt.get("local_amount"),
                        f"{receipt_context}.local_amount",
                    ),
                    "xor_due": cls._quantity(receipt.get("xor_due"), f"{receipt_context}.xor_due"),
                    "xor_after_haircut": cls._quantity(
                        receipt.get("xor_after_haircut"),
                        f"{receipt_context}.xor_after_haircut",
                    ),
                    "xor_variance": cls._quantity(
                        receipt.get("xor_variance"),
                        f"{receipt_context}.xor_variance",
                    ),
                    "timestamp_ms": cls._unsigned(receipt.get("timestamp_ms"), f"{receipt_context}.timestamp_ms"),
                }
            )
        swap_value = record.get("swap_metadata")
        if swap_value is None:
            swap_metadata = None
        else:
            swap_context = f"{context}.swap_metadata"
            swap = cls._exact_mapping(
                swap_value,
                swap_context,
                {
                    "epsilon_bps",
                    "twap_window_seconds",
                    "liquidity_profile",
                    "twap_local_per_xor",
                    "volatility_class",
                },
            )
            swap_metadata = {
                "epsilon_bps": cls._unsigned(
                    swap.get("epsilon_bps"),
                    f"{swap_context}.epsilon_bps",
                    maximum=0xFFFF,
                ),
                "twap_window_seconds": cls._unsigned(
                    swap.get("twap_window_seconds"),
                    f"{swap_context}.twap_window_seconds",
                    maximum=cls.MAX_U32,
                ),
                "liquidity_profile": {
                    "profile": cls._tagged_content(
                        swap.get("liquidity_profile"),
                        tag="profile",
                        content="state",
                        allowed={"Tier1", "Tier2", "Tier3"},
                        context=f"{swap_context}.liquidity_profile",
                    ),
                    "state": None,
                },
                "twap_local_per_xor": cls._non_empty_string(
                    swap.get("twap_local_per_xor"),
                    f"{swap_context}.twap_local_per_xor",
                ),
                "volatility_class": {
                    "bucket": cls._tagged_content(
                        swap.get("volatility_class"),
                        tag="bucket",
                        content="state",
                        allowed={"Stable", "Elevated", "Dislocated"},
                        context=f"{swap_context}.volatility_class",
                    ),
                    "state": None,
                },
            }
        nexus_fee_receipts = [
            cls._nexus_fee_receipt(item, context=f"{context}.nexus_fee_receipts[{index}]")
            for index, item in enumerate(
                cls._array(
                    record.get("nexus_fee_receipts"),
                    f"{context}.nexus_fee_receipts",
                )
            )
        ]
        native_amx_receipts = [
            cls._native_amx_receipt(
                item,
                context=f"{context}.native_amx_receipts[{index}]",
            )
            for index, item in enumerate(
                cls._array(
                    record.get("native_amx_receipts"),
                    f"{context}.native_amx_receipts",
                    maximum=cls.MAX_NATIVE_AMX_PARTICIPANT_SETTLEMENT_RECEIPTS,
                )
            )
        ]
        if len({item["source_id"] for item in nexus_fee_receipts}) != len(
            nexus_fee_receipts
        ):
            raise RuntimeError(f"{context} contains duplicate Nexus fee receipt sources")
        if len({item["source_id"] for item in native_amx_receipts}) != len(
            native_amx_receipts
        ):
            raise RuntimeError(f"{context} contains duplicate native AMX receipt sources")
        native_amx_sources = [item["source_id"] for item in native_amx_receipts]
        if any(
            native_amx_sources[index - 1] >= source_id
            for index, source_id in enumerate(native_amx_sources)
            if index > 0
        ):
            raise RuntimeError(
                f"{context} native AMX receipt sources must be strictly ordered"
            )
        if any(
            item["lane_id"] != lane_id
            or item["dataspace_id"] != dataspace_id
            or item["block_height"] != block_height
            for item in nexus_fee_receipts
        ):
            raise RuntimeError(f"{context} Nexus fee receipt coordinates do not match")
        if any(
            item["lane_id"] != lane_id
            or item["dataspace_id"] != dataspace_id
            or item["lane_incarnation"] != lane_incarnation
            or item["lane_block_height"] != block_height
            for item in native_amx_receipts
        ):
            raise RuntimeError(f"{context} native AMX receipt coordinates do not match")
        if any(
            [
                receipt["source_id"]
                for receipt in leg["participant_settlement"]["receipts"]
            ]
            != native_amx_sources
            for native_receipt in native_amx_receipts
            for leg in native_receipt["legs"]
        ):
            raise RuntimeError(
                f"{context} native AMX receipts do not bind the exact ordered "
                "source group"
            )
        return {
            "block_height": block_height,
            "lane_id": lane_id,
            "lane_incarnation": lane_incarnation,
            "dataspace_id": dataspace_id,
            "tx_count": cls._unsigned(record.get("tx_count"), f"{context}.tx_count"),
            "total_local_amount": cls._quantity(
                record.get("total_local_amount"), f"{context}.total_local_amount"
            ),
            "total_xor_due": cls._quantity(record.get("total_xor_due"), f"{context}.total_xor_due"),
            "total_xor_after_haircut": cls._quantity(
                record.get("total_xor_after_haircut"),
                f"{context}.total_xor_after_haircut",
            ),
            "total_xor_variance": cls._quantity(
                record.get("total_xor_variance"), f"{context}.total_xor_variance"
            ),
            "swap_metadata": swap_metadata,
            "receipts": receipts,
            "nexus_fee_receipts": nexus_fee_receipts,
            "native_amx_receipts": native_amx_receipts,
        }

    @classmethod
    def _nexus_fee_schedule(cls, value: Any, *, context: str) -> Dict[str, Any]:
        record = cls._exact_mapping(
            value,
            context,
            {
                "tx_bytes_len",
                "instruction_count",
                "gas_used",
                "base_fee",
                "per_byte_fee",
                "per_instruction_fee",
                "per_gas_unit_fee",
            },
        )
        return {
            "tx_bytes_len": cls._unsigned(
                record.get("tx_bytes_len"), f"{context}.tx_bytes_len"
            ),
            "instruction_count": cls._unsigned(
                record.get("instruction_count"), f"{context}.instruction_count"
            ),
            "gas_used": cls._unsigned(record.get("gas_used"), f"{context}.gas_used"),
            "base_fee": cls._quantity(record.get("base_fee"), f"{context}.base_fee"),
            "per_byte_fee": cls._quantity(
                record.get("per_byte_fee"), f"{context}.per_byte_fee"
            ),
            "per_instruction_fee": cls._quantity(
                record.get("per_instruction_fee"),
                f"{context}.per_instruction_fee",
            ),
            "per_gas_unit_fee": cls._quantity(
                record.get("per_gas_unit_fee"), f"{context}.per_gas_unit_fee"
            ),
        }

    @classmethod
    def _nexus_fee_receipt(cls, value: Any, *, context: str) -> Dict[str, Any]:
        record = cls._exact_mapping(
            value,
            context,
            {
                "version",
                "source_id",
                "dataspace_id",
                "lane_id",
                "block_height",
                "payer_account_id",
                "fee_asset_id",
                "fee_amount",
                "schedule",
            },
        )
        version = cls._unsigned(
            record.get("version"), f"{context}.version", maximum=0xFFFF
        )
        if version != 1:
            raise RuntimeError(f"{context}.version must equal 1")
        return {
            "version": version,
            "source_id": cls._byte32(record.get("source_id"), f"{context}.source_id"),
            "dataspace_id": cls._unsigned(
                record.get("dataspace_id"), f"{context}.dataspace_id"
            ),
            "lane_id": cls._unsigned(
                record.get("lane_id"), f"{context}.lane_id", maximum=cls.MAX_U32
            ),
            "block_height": cls._unsigned(
                record.get("block_height"), f"{context}.block_height"
            ),
            "payer_account_id": cls._non_empty_string(
                record.get("payer_account_id"), f"{context}.payer_account_id"
            ),
            "fee_asset_id": cls._non_empty_string(
                record.get("fee_asset_id"), f"{context}.fee_asset_id"
            ),
            "fee_amount": cls._quantity(
                record.get("fee_amount"), f"{context}.fee_amount"
            ),
            "schedule": cls._nexus_fee_schedule(
                record.get("schedule"), context=f"{context}.schedule"
            ),
        }

    @classmethod
    def _native_amx_body(cls, value: Any, *, context: str) -> Dict[str, Any]:
        record = cls._exact_mapping(
            value,
            context,
            {
                "round",
                "epoch",
                "network_id",
                "source_id",
                "tx_entrypoint_hash",
                "plan_digest",
                "phase",
                "coordinator_lane_id",
                "coordinator_dataspace_id",
                "coordinator_lane_incarnation",
                "participant_lane_id",
                "participant_dataspace_id",
                "participant_lane_incarnation",
                "participant_previous_block_height",
                "participant_previous_block_descriptor_hash",
                "participant_lane_block_height",
                "participant_lane_block_view",
                "participant_proposal_hash",
                "participant_settlement_commitment",
                "participant_validator_set_hash",
                "participant_validator_count",
                "participant_min_quorum",
                "authority_context_height",
                "planned_coordinator_block_height",
                "coordinator_lane_block_view",
                "coordinator_proposal_hash",
            },
        )
        round_record = cls._exact_mapping(
            record.get("round"),
            f"{context}.round",
            {"context_id", "height", "view"},
        )
        round_value = {
            "context_id": list(
                cls._context_id(
                    round_record.get("context_id"), f"{context}.round.context_id"
                )
            ),
            "height": cls._unsigned(
                round_record.get("height"), f"{context}.round.height", positive=True
            ),
            "view": cls._unsigned(
                round_record.get("view"), f"{context}.round.view"
            ),
        }
        phase = cls._tagged_content(
            record.get("phase"),
            tag="phase",
            content="detail",
            allowed={"prepare", "commit"},
            context=f"{context}.phase",
        )
        validator_count = cls._unsigned(
            record.get("participant_validator_count"),
            f"{context}.participant_validator_count",
            positive=True,
            maximum=cls.MAX_LANE_VALIDATORS,
        )
        min_quorum = cls._unsigned(
            record.get("participant_min_quorum"),
            f"{context}.participant_min_quorum",
            positive=True,
            maximum=cls.MAX_LANE_VALIDATORS,
        )
        expected_quorum = validator_count - (validator_count - 1) // 3
        authority_height = cls._unsigned(
            record.get("authority_context_height"),
            f"{context}.authority_context_height",
            positive=True,
        )
        planned_height = cls._unsigned(
            record.get("planned_coordinator_block_height"),
            f"{context}.planned_coordinator_block_height",
            positive=True,
        )
        coordinator_view = cls._unsigned(
            record.get("coordinator_lane_block_view"),
            f"{context}.coordinator_lane_block_view",
        )
        participant_previous_height = cls._exact_unsigned(
            record.get("participant_previous_block_height"),
            f"{context}.participant_previous_block_height",
            maximum=cls.MAX_U64,
        )
        previous_descriptor_value = record.get(
            "participant_previous_block_descriptor_hash"
        )
        previous_descriptor_hash = (
            None
            if previous_descriptor_value is None
            else cls._nonzero_hash(
                previous_descriptor_value,
                f"{context}.participant_previous_block_descriptor_hash",
            )
        )
        participant_height = cls._exact_unsigned(
            record.get("participant_lane_block_height"),
            f"{context}.participant_lane_block_height",
            positive=True,
            maximum=cls.MAX_U64,
        )
        participant_view = cls._exact_unsigned(
            record.get("participant_lane_block_view"),
            f"{context}.participant_lane_block_view",
            maximum=cls.MAX_U64,
        )
        source_id = cls._byte32(record.get("source_id"), f"{context}.source_id")
        entrypoint_hash = cls._hash(
            record.get("tx_entrypoint_hash"), f"{context}.tx_entrypoint_hash"
        )
        if (
            round_value["height"] != authority_height
            or participant_previous_height + 1 != participant_height
            or (participant_previous_height == 0)
            != (previous_descriptor_hash is None)
            or min_quorum != expected_quorum
        ):
            raise RuntimeError(f"{context} contains inconsistent round or quorum fields")
        return {
            "round": round_value,
            "epoch": cls._unsigned(record.get("epoch"), f"{context}.epoch"),
            "network_id": cls._hash(
                record.get("network_id"), f"{context}.network_id"
            ),
            "source_id": source_id,
            "tx_entrypoint_hash": entrypoint_hash,
            "plan_digest": cls._hash(
                record.get("plan_digest"), f"{context}.plan_digest"
            ),
            "phase": {"phase": phase, "detail": None},
            "coordinator_lane_id": cls._unsigned(
                record.get("coordinator_lane_id"),
                f"{context}.coordinator_lane_id",
                maximum=cls.MAX_U32,
            ),
            "coordinator_dataspace_id": cls._unsigned(
                record.get("coordinator_dataspace_id"),
                f"{context}.coordinator_dataspace_id",
            ),
            "coordinator_lane_incarnation": cls._nonzero_hash(
                record.get("coordinator_lane_incarnation"),
                f"{context}.coordinator_lane_incarnation",
            ),
            "participant_lane_id": cls._unsigned(
                record.get("participant_lane_id"),
                f"{context}.participant_lane_id",
                maximum=cls.MAX_U32,
            ),
            "participant_dataspace_id": cls._unsigned(
                record.get("participant_dataspace_id"),
                f"{context}.participant_dataspace_id",
            ),
            "participant_lane_incarnation": cls._nonzero_hash(
                record.get("participant_lane_incarnation"),
                f"{context}.participant_lane_incarnation",
            ),
            "participant_previous_block_height": participant_previous_height,
            "participant_previous_block_descriptor_hash": previous_descriptor_hash,
            "participant_lane_block_height": participant_height,
            "participant_lane_block_view": participant_view,
            "participant_proposal_hash": cls._nonzero_hash(
                record.get("participant_proposal_hash"),
                f"{context}.participant_proposal_hash",
            ),
            "participant_settlement_commitment": cls._nonzero_hash(
                record.get("participant_settlement_commitment"),
                f"{context}.participant_settlement_commitment",
            ),
            "participant_validator_set_hash": cls._hash(
                record.get("participant_validator_set_hash"),
                f"{context}.participant_validator_set_hash",
            ),
            "participant_validator_count": validator_count,
            "participant_min_quorum": min_quorum,
            "authority_context_height": authority_height,
            "planned_coordinator_block_height": planned_height,
            "coordinator_lane_block_view": coordinator_view,
            "coordinator_proposal_hash": cls._nonzero_hash(
                record.get("coordinator_proposal_hash"),
                f"{context}.coordinator_proposal_hash",
            ),
        }

    @staticmethod
    def _native_amx_body_identity(body: Mapping[str, Any]) -> Dict[str, Any]:
        return {key: value for key, value in body.items() if key != "phase"}

    @classmethod
    def _native_amx_qc(cls, value: Any, *, context: str) -> Dict[str, Any]:
        record = cls._exact_mapping(
            value,
            context,
            {
                "body",
                "validator_set_hash_version",
                "validator_set_hash",
                "validator_set",
                "validator_set_pops",
                "signers_bitmap",
                "bls_aggregate_signature",
            },
        )
        body = cls._native_amx_body(record.get("body"), context=f"{context}.body")
        version = cls._unsigned(
            record.get("validator_set_hash_version"),
            f"{context}.validator_set_hash_version",
            maximum=0xFFFF,
        )
        if version != 1:
            raise RuntimeError(f"{context}.validator_set_hash_version must equal 1")
        try:
            validators = list(
                validate_bls_normal_validator_set(
                    cls._array(
                        record.get("validator_set"),
                        f"{context}.validator_set",
                        minimum=1,
                        maximum=cls.MAX_LANE_VALIDATORS,
                    ),
                    f"{context}.validator_set",
                )
            )
        except (TypeError, ValueError) as exc:
            raise RuntimeError(str(exc)) from exc
        validator_hash = cls._hash(
            record.get("validator_set_hash"), f"{context}.validator_set_hash"
        )
        computed_validator_hash = compute_native_amx_validator_set_hash(validators)
        expected_quorum = len(validators) - (len(validators) - 1) // 3
        if (
            len(validators) != body["participant_validator_count"]
            or body["participant_min_quorum"] != expected_quorum
            or validator_hash != body["participant_validator_set_hash"]
            or validator_hash != computed_validator_hash
        ):
            raise RuntimeError(f"{context} committee fields differ from its signed body")
        pops_raw = cls._array(
            record.get("validator_set_pops"),
            f"{context}.validator_set_pops",
            minimum=len(validators),
            maximum=len(validators),
        )
        pops = [
            cls._byte_vector(item, 96, f"{context}.validator_set_pops[{index}]")
            for index, item in enumerate(pops_raw)
        ]
        if any(not any(pop) for pop in pops):
            raise RuntimeError(f"{context}.validator_set_pops contains an all-zero proof")
        bitmap_length = (len(validators) + 7) // 8
        bitmap = cls._byte_vector(
            record.get("signers_bitmap"), bitmap_length, f"{context}.signers_bitmap"
        )
        trailing_bits = len(validators) % 8
        if trailing_bits and bitmap[-1] & ~((1 << trailing_bits) - 1):
            raise RuntimeError(f"{context}.signers_bitmap addresses an unknown validator")
        if sum(bin(byte).count("1") for byte in bitmap) != expected_quorum:
            raise RuntimeError(f"{context}.signers_bitmap does not carry the exact quorum")
        signature = cls._byte_vector(
            record.get("bls_aggregate_signature"),
            96,
            f"{context}.bls_aggregate_signature",
        )
        if not any(signature):
            raise RuntimeError(f"{context}.bls_aggregate_signature must not be all zeroes")
        return {
            "body": body,
            "validator_set_hash_version": version,
            "validator_set_hash": validator_hash,
            "validator_set": validators,
            "validator_set_pops": pops,
            "signers_bitmap": bitmap,
            "bls_aggregate_signature": signature,
        }

    @classmethod
    def _native_amx_participant_proposal(cls, value: Any, *, context: str) -> Dict[str, Any]:
        proposal = cls._exact_mapping(
            value,
            context,
            {"descriptor", "proposal_hash"},
        )
        descriptor_context = f"{context}.descriptor"
        descriptor = cls._mapping(proposal.get("descriptor"), descriptor_context)
        required_fields = {
            "lane_id",
            "dataspace_id",
            "lane_incarnation",
            "proposal_height",
            "previous_lane_block_height",
            "lane_block_height",
            "lane_block_view",
            "subject_hash",
            "payload_ownership_hash",
            "rbc_instance_hash",
            "accepted_candidate_indices",
            "accepted_transaction_hashes",
            "validator_set_hash_version",
            "validator_set_hash",
            "validator_set",
            "validator_count",
            "min_quorum",
            "qc_mode_tag",
            "descriptor_hash",
        }
        allowed_fields = required_fields | {"previous_lane_block_descriptor_hash"}
        missing = sorted(required_fields - set(descriptor))
        unknown = sorted(set(descriptor) - allowed_fields)
        if missing:
            raise RuntimeError(f"{descriptor_context} is missing required field {missing[0]}")
        if unknown:
            raise RuntimeError(f"{descriptor_context} contains unknown field {unknown[0]}")

        previous_height = cls._exact_unsigned(
            descriptor.get("previous_lane_block_height"),
            f"{descriptor_context}.previous_lane_block_height",
            maximum=cls.MAX_U64,
        )
        if previous_height == 0:
            if "previous_lane_block_descriptor_hash" in descriptor:
                raise RuntimeError(f"{descriptor_context} must omit the genesis predecessor hash")
            previous_hash = None
        else:
            if descriptor.get("previous_lane_block_descriptor_hash") is None:
                raise RuntimeError(f"{descriptor_context} must carry a predecessor descriptor hash")
            previous_hash = cls._nonzero_hash(
                descriptor.get("previous_lane_block_descriptor_hash"),
                f"{descriptor_context}.previous_lane_block_descriptor_hash",
            )
        lane_height = cls._exact_unsigned(
            descriptor.get("lane_block_height"),
            f"{descriptor_context}.lane_block_height",
            positive=True,
            maximum=cls.MAX_U64,
        )
        if previous_height + 1 != lane_height:
            raise RuntimeError(f"{descriptor_context} lane-block heights are not contiguous")

        indices = [
            cls._exact_unsigned(
                item,
                f"{descriptor_context}.accepted_candidate_indices[{index}]",
                maximum=cls.MAX_U64,
            )
            for index, item in enumerate(
                cls._array(
                    descriptor.get("accepted_candidate_indices"),
                    f"{descriptor_context}.accepted_candidate_indices",
                    minimum=1,
                    maximum=4096,
                )
            )
        ]
        transaction_hashes = [
            cls._nonzero_hash(
                item,
                f"{descriptor_context}.accepted_transaction_hashes[{index}]",
            )
            for index, item in enumerate(
                cls._array(
                    descriptor.get("accepted_transaction_hashes"),
                    f"{descriptor_context}.accepted_transaction_hashes",
                    minimum=1,
                    maximum=4096,
                )
            )
        ]
        if (
            len(indices) != len(transaction_hashes)
            or len(set(indices)) != len(indices)
            or len(set(transaction_hashes)) != len(transaction_hashes)
        ):
            raise RuntimeError(f"{descriptor_context} accepted work is inconsistent")

        try:
            validators = list(
                validate_bls_normal_validator_set(
                    cls._array(
                        descriptor.get("validator_set"),
                        f"{descriptor_context}.validator_set",
                        minimum=1,
                        maximum=cls.MAX_LANE_VALIDATORS,
                    ),
                    f"{descriptor_context}.validator_set",
                )
            )
        except (TypeError, ValueError) as exc:
            raise RuntimeError(str(exc)) from exc
        validator_count = cls._exact_unsigned(
            descriptor.get("validator_count"),
            f"{descriptor_context}.validator_count",
            positive=True,
            maximum=cls.MAX_LANE_VALIDATORS,
        )
        min_quorum = cls._exact_unsigned(
            descriptor.get("min_quorum"),
            f"{descriptor_context}.min_quorum",
            positive=True,
            maximum=cls.MAX_LANE_VALIDATORS,
        )
        expected_quorum = len(validators) - (len(validators) - 1) // 3
        validator_hash_version = cls._exact_unsigned(
            descriptor.get("validator_set_hash_version"),
            f"{descriptor_context}.validator_set_hash_version",
            maximum=0xFFFF,
        )
        if (
            validator_hash_version != 1
            or validator_count != len(validators)
            or min_quorum != expected_quorum
        ):
            raise RuntimeError(f"{descriptor_context} committee fields are inconsistent")
        normalized_descriptor = {
            "lane_id": cls._exact_unsigned(
                descriptor.get("lane_id"),
                f"{descriptor_context}.lane_id",
                maximum=cls.MAX_U32,
            ),
            "dataspace_id": cls._exact_unsigned(
                descriptor.get("dataspace_id"),
                f"{descriptor_context}.dataspace_id",
                maximum=cls.MAX_U64,
            ),
            "lane_incarnation": cls._nonzero_hash(
                descriptor.get("lane_incarnation"),
                f"{descriptor_context}.lane_incarnation",
            ),
            "proposal_height": cls._exact_unsigned(
                descriptor.get("proposal_height"),
                f"{descriptor_context}.proposal_height",
                positive=True,
                maximum=cls.MAX_U64,
            ),
            "previous_lane_block_height": previous_height,
            **(
                {"previous_lane_block_descriptor_hash": previous_hash}
                if previous_hash is not None
                else {}
            ),
            "lane_block_height": lane_height,
            "lane_block_view": cls._exact_unsigned(
                descriptor.get("lane_block_view"),
                f"{descriptor_context}.lane_block_view",
                maximum=cls.MAX_U64,
            ),
            "subject_hash": cls._nonzero_hash(
                descriptor.get("subject_hash"),
                f"{descriptor_context}.subject_hash",
            ),
            "payload_ownership_hash": cls._nonzero_hash(
                descriptor.get("payload_ownership_hash"),
                f"{descriptor_context}.payload_ownership_hash",
            ),
            "rbc_instance_hash": cls._nonzero_hash(
                descriptor.get("rbc_instance_hash"),
                f"{descriptor_context}.rbc_instance_hash",
            ),
            "accepted_candidate_indices": indices,
            "accepted_transaction_hashes": transaction_hashes,
            "validator_set_hash_version": validator_hash_version,
            "validator_set_hash": cls._hash(
                descriptor.get("validator_set_hash"),
                f"{descriptor_context}.validator_set_hash",
            ),
            "validator_set": validators,
            "validator_count": validator_count,
            "min_quorum": min_quorum,
            "qc_mode_tag": cls._exact_non_empty_string(
                descriptor.get("qc_mode_tag"),
                f"{descriptor_context}.qc_mode_tag",
            ),
            "descriptor_hash": cls._nonzero_hash(
                descriptor.get("descriptor_hash"),
                f"{descriptor_context}.descriptor_hash",
            ),
        }
        if (
            normalized_descriptor["validator_set_hash"]
            != compute_native_amx_validator_set_hash(validators)
        ):
            raise RuntimeError(
                f"{descriptor_context}.validator_set_hash does not match "
                "the canonical committee"
            )
        if (
            normalized_descriptor["descriptor_hash"]
            != compute_native_amx_descriptor_hash(normalized_descriptor)
        ):
            raise RuntimeError(
                f"{descriptor_context}.descriptor_hash does not match "
                "its canonical preimage"
            )
        proposal_hash = cls._nonzero_hash(
            proposal.get("proposal_hash"), f"{context}.proposal_hash"
        )
        if proposal_hash != compute_native_amx_proposal_hash(normalized_descriptor):
            raise RuntimeError(
                f"{context}.proposal_hash does not match its canonical preimage"
            )
        return {
            "descriptor": normalized_descriptor,
            "proposal_hash": proposal_hash,
        }

    @classmethod
    def _native_amx_leg(cls, value: Any, *, context: str) -> Dict[str, Any]:
        record = cls._exact_mapping(
            value,
            context,
            {
                "lane_id",
                "dataspace_id",
                "participant_proposal",
                "participant_settlement",
                "participant_settlement_hash",
                "prepare_qc",
                "commit_qc",
            },
        )
        lane_id = cls._exact_unsigned(
            record.get("lane_id"), f"{context}.lane_id", maximum=cls.MAX_U32
        )
        dataspace_id = cls._exact_unsigned(
            record.get("dataspace_id"),
            f"{context}.dataspace_id",
            maximum=cls.MAX_U64,
        )
        proposal = cls._native_amx_participant_proposal(
            record.get("participant_proposal"),
            context=f"{context}.participant_proposal",
        )
        settlement_value = cls._mapping(
            record.get("participant_settlement"),
            f"{context}.participant_settlement",
        )
        if settlement_value.get("native_amx_receipts") != []:
            raise RuntimeError(f"{context}.participant_settlement must be terminal")
        if settlement_value.get("nexus_fee_receipts") != []:
            raise RuntimeError(f"{context}.participant_settlement cannot contain fee receipts")
        cls._array(
            settlement_value.get("receipts"),
            f"{context}.participant_settlement.receipts",
            minimum=1,
            maximum=cls.MAX_NATIVE_AMX_PARTICIPANT_SETTLEMENT_RECEIPTS,
        )
        settlement = cls._settlement(settlement_value, context=f"{context}.participant_settlement")
        settlement_hash = cls._nonzero_hash(
            record.get("participant_settlement_hash"),
            f"{context}.participant_settlement_hash",
        )
        if (
            settlement_hash
            != compute_native_amx_participant_settlement_hash(settlement)
        ):
            raise RuntimeError(
                f"{context}.participant_settlement_hash does not match "
                "its canonical commitment"
            )
        prepare = cls._native_amx_qc(record.get("prepare_qc"), context=f"{context}.prepare_qc")
        commit = cls._native_amx_qc(record.get("commit_qc"), context=f"{context}.commit_qc")
        if prepare["body"]["phase"]["phase"] != "prepare":
            raise RuntimeError(f"{context}.prepare_qc carries the wrong phase")
        if commit["body"]["phase"]["phase"] != "commit":
            raise RuntimeError(f"{context}.commit_qc carries the wrong phase")
        if cls._native_amx_body_identity(prepare["body"]) != cls._native_amx_body_identity(
            commit["body"]
        ):
            raise RuntimeError(f"{context} prepare and commit identities differ")
        for field in (
            "validator_set_hash_version",
            "validator_set_hash",
            "validator_set",
            "validator_set_pops",
        ):
            if prepare[field] != commit[field]:
                raise RuntimeError(f"{context} prepare and commit committees differ")
        body = prepare["body"]
        descriptor = proposal["descriptor"]
        if (
            body["participant_lane_id"] != lane_id
            or body["participant_dataspace_id"] != dataspace_id
            or descriptor["lane_id"] != lane_id
            or descriptor["dataspace_id"] != dataspace_id
            or descriptor["lane_incarnation"] != body["participant_lane_incarnation"]
            or descriptor["proposal_height"] != body["authority_context_height"]
            or descriptor["previous_lane_block_height"] != body["participant_previous_block_height"]
            or descriptor.get("previous_lane_block_descriptor_hash")
            != body["participant_previous_block_descriptor_hash"]
            or descriptor["lane_block_height"] != body["participant_lane_block_height"]
            or descriptor["lane_block_view"] != body["participant_lane_block_view"]
            or proposal["proposal_hash"] != body["participant_proposal_hash"]
            or descriptor["validator_set_hash_version"] != prepare["validator_set_hash_version"]
            or descriptor["validator_set_hash"] != prepare["validator_set_hash"]
            or descriptor["validator_set"] != prepare["validator_set"]
            or descriptor["validator_count"] != body["participant_validator_count"]
            or descriptor["min_quorum"] != body["participant_min_quorum"]
        ):
            raise RuntimeError(f"{context} participant proposal differs from its signed body")
        receipts = settlement["receipts"]
        receipt_sources = [receipt["source_id"] for receipt in receipts]
        if any(
            receipt_sources[index - 1] >= source_id
            for index, source_id in enumerate(receipt_sources)
            if index > 0
        ):
            raise RuntimeError(
                f"{context}.participant_settlement.receipts must be strictly "
                "ordered by source_id"
            )
        matching_entrypoints = [
            index
            for index, entrypoint_hash in enumerate(
                descriptor["accepted_transaction_hashes"]
            )
            if entrypoint_hash == body["tx_entrypoint_hash"]
        ]
        if len(matching_entrypoints) > 1:
            raise RuntimeError(
                f"{context} participant descriptor repeats the current "
                "transaction entrypoint"
            )
        requires_mixed_role_anchor_validation = not matching_entrypoints
        if (
            not requires_mixed_role_anchor_validation
            and (
                len(descriptor["accepted_candidate_indices"]) != len(receipts)
                or len(descriptor["accepted_transaction_hashes"]) != len(receipts)
                or receipt_sources[matching_entrypoints[0]] != body["source_id"]
            )
        ):
            raise RuntimeError(
                f"{context} participant descriptor and grouped settlement "
                "are not aligned"
            )
        if (
            settlement_hash != body["participant_settlement_commitment"]
            or settlement["block_height"] != body["participant_lane_block_height"]
            or settlement["lane_id"] != lane_id
            or settlement["dataspace_id"] != dataspace_id
            or settlement["lane_incarnation"] != body["participant_lane_incarnation"]
            or settlement["tx_count"] != len(receipts)
            or settlement["total_local_amount"] != "0"
            or settlement["total_xor_due"] != "0"
            or settlement["total_xor_after_haircut"] != "0"
            or settlement["total_xor_variance"] != "0"
            or settlement["swap_metadata"] is not None
            or len(set(receipt_sources)) != len(receipt_sources)
            or receipt_sources.count(body["source_id"]) != 1
            or any(
                receipt["local_amount"] != "0"
                or receipt["xor_due"] != "0"
                or receipt["xor_after_haircut"] != "0"
                or receipt["xor_variance"] != "0"
                or receipt["timestamp_ms"] != body["authority_context_height"]
                for receipt in receipts
            )
            or settlement["nexus_fee_receipts"]
            or settlement["native_amx_receipts"]
        ):
            raise RuntimeError(f"{context} participant settlement differs from its signed body")
        return {
            "lane_id": lane_id,
            "dataspace_id": dataspace_id,
            "participant_proposal": proposal,
            "participant_settlement": settlement,
            "participant_settlement_hash": settlement_hash,
            "prepare_qc": prepare,
            "commit_qc": commit,
            "requires_mixed_role_anchor_validation": (
                requires_mixed_role_anchor_validation
            ),
        }

    @classmethod
    def _native_amx_receipt(cls, value: Any, *, context: str) -> Dict[str, Any]:
        record = cls._exact_mapping(
            value,
            context,
            {
                "version",
                "source_id",
                "network_id",
                "plan_digest",
                "lane_id",
                "dataspace_id",
                "lane_incarnation",
                "authority_context_height",
                "lane_block_height",
                "lane_block_view",
                "coordinator_proposal_hash",
                "legs",
            },
        )
        version = cls._unsigned(
            record.get("version"), f"{context}.version", maximum=0xFFFF
        )
        if version != 2:
            raise RuntimeError(f"{context}.version must equal 2")
        source_id = cls._byte32(record.get("source_id"), f"{context}.source_id")
        network_id = cls._hash(
            record.get("network_id"), f"{context}.network_id"
        )
        plan_digest = cls._hash(
            record.get("plan_digest"), f"{context}.plan_digest"
        )
        lane_id = cls._unsigned(
            record.get("lane_id"), f"{context}.lane_id", maximum=cls.MAX_U32
        )
        dataspace_id = cls._unsigned(
            record.get("dataspace_id"), f"{context}.dataspace_id"
        )
        lane_incarnation = cls._nonzero_hash(
            record.get("lane_incarnation"), f"{context}.lane_incarnation"
        )
        authority_height = cls._unsigned(
            record.get("authority_context_height"),
            f"{context}.authority_context_height",
            positive=True,
        )
        lane_height = cls._unsigned(
            record.get("lane_block_height"),
            f"{context}.lane_block_height",
            positive=True,
        )
        lane_view = cls._unsigned(
            record.get("lane_block_view"), f"{context}.lane_block_view"
        )
        proposal_hash = cls._nonzero_hash(
            record.get("coordinator_proposal_hash"),
            f"{context}.coordinator_proposal_hash",
        )
        legs = [
            cls._native_amx_leg(item, context=f"{context}.legs[{index}]")
            for index, item in enumerate(
                cls._array(
                    record.get("legs"),
                    f"{context}.legs",
                    minimum=1,
                    maximum=cls.MAX_NATIVE_AMX_PARTICIPANT_LEGS,
                )
            )
        ]
        route_keys = {
            (item["lane_id"], item["dataspace_id"])
            for item in legs
        }
        if len(route_keys) != len(legs):
            raise RuntimeError(f"{context}.legs contains duplicate participant routes")
        first_body = legs[0]["prepare_qc"]["body"]
        entrypoint_hash = first_body["tx_entrypoint_hash"]
        for leg in legs:
            body = leg["prepare_qc"]["body"]
            if (
                body["round"] != first_body["round"]
                or body["epoch"] != first_body["epoch"]
                or body["round"]["height"] != authority_height
                or body["network_id"] != network_id
                or body["source_id"] != source_id
                or body["tx_entrypoint_hash"] != entrypoint_hash
                or body["plan_digest"] != plan_digest
                or body["coordinator_lane_id"] != lane_id
                or body["coordinator_dataspace_id"] != dataspace_id
                or body["coordinator_lane_incarnation"] != lane_incarnation
                or body["authority_context_height"] != authority_height
                or body["planned_coordinator_block_height"] != lane_height
                or body["coordinator_lane_block_view"] != lane_view
                or body["coordinator_proposal_hash"] != proposal_hash
                or (
                    leg["lane_id"] == lane_id
                    and leg["dataspace_id"] == dataspace_id
                    and (
                        leg["requires_mixed_role_anchor_validation"]
                        or leg["participant_proposal"]["descriptor"][
                            "lane_incarnation"
                        ]
                        != lane_incarnation
                        or leg["participant_proposal"]["descriptor"][
                            "lane_block_height"
                        ]
                        != lane_height
                        or leg["participant_proposal"]["descriptor"][
                            "lane_block_view"
                        ]
                        != lane_view
                        or leg["participant_proposal"]["proposal_hash"]
                        != proposal_hash
                    )
                )
            ):
                raise RuntimeError(f"{context}.legs contain mismatched signed identities")
        return {
            "version": version,
            "source_id": source_id,
            "network_id": network_id,
            "plan_digest": plan_digest,
            "lane_id": lane_id,
            "dataspace_id": dataspace_id,
            "lane_incarnation": lane_incarnation,
            "authority_context_height": authority_height,
            "lane_block_height": lane_height,
            "lane_block_view": lane_view,
            "coordinator_proposal_hash": proposal_hash,
            "legs": legs,
        }

    @classmethod
    def _relays(cls, value: Any) -> List[Dict[str, Any]]:
        context = "sumeragi.lane_relay_envelopes"
        relays: List[Dict[str, Any]] = []
        for index, relay_value in enumerate(
            cls._array(value, context, maximum=cls.MAX_LANE_RELAY_ENVELOPES)
        ):
            item_context = f"{context}[{index}]"
            record = cls._mapping(relay_value, item_context)
            lane_id = cls._unsigned(record.get("lane_id"), f"{item_context}.lane_id", maximum=cls.MAX_U32)
            lane_incarnation = cls._nonzero_hash(record.get("lane_incarnation"), f"{item_context}.lane_incarnation")
            dataspace_id = cls._unsigned(record.get("dataspace_id"), f"{item_context}.dataspace_id")
            block_height = cls._unsigned(record.get("block_height"), f"{item_context}.block_height")
            settlement = cls._settlement(record.get("settlement_commitment"), context=f"{item_context}.settlement_commitment")
            if (
                settlement["lane_id"] != lane_id
                or settlement["lane_incarnation"] != lane_incarnation
                or settlement["dataspace_id"] != dataspace_id
                or settlement["block_height"] != block_height
            ):
                raise RuntimeError(
                    f"{item_context}.settlement_commitment identity must match its relay"
                )
            qc_value = record.get("qc")
            manifest_value = record.get("manifest_root")
            proof_value = record.get("fastpq_proof")
            descriptor_value = record.get("lane_block_descriptor_hash")
            da_value = record.get("da_commitment_hash")
            relays.append(
                {
                    "lane_id": lane_id,
                    "lane_incarnation": lane_incarnation,
                    "dataspace_id": dataspace_id,
                    "block_height": block_height,
                    "block_header": cls._clone_mapping(record.get("block_header"), f"{item_context}.block_header"),
                    "qc": None if qc_value is None else cls._clone_mapping(qc_value, f"{item_context}.qc"),
                    "da_commitment_hash": None if da_value is None else cls._hash(da_value, f"{item_context}.da_commitment_hash"),
                    "lane_block_descriptor_hash": None if descriptor_value is None else cls._hash(descriptor_value, f"{item_context}.lane_block_descriptor_hash"),
                    "settlement_commitment": settlement,
                    "settlement_hash": cls._hash(record.get("settlement_hash"), f"{item_context}.settlement_hash"),
                    "rbc_bytes_total": cls._unsigned(record.get("rbc_bytes_total"), f"{item_context}.rbc_bytes_total"),
                    "manifest_root": None if manifest_value is None else cls._byte32(manifest_value, f"{item_context}.manifest_root"),
                    "fastpq_proof": None if proof_value is None else cls._fastpq_proof(proof_value, context=f"{item_context}.fastpq_proof"),
                }
            )
        return relays

    @classmethod
    def _fastpq_proof(cls, value: Any, *, context: str) -> Dict[str, Any]:
        record = cls._mapping(value, context)
        return {
            "proof_digest": cls._hash(record.get("proof_digest"), f"{context}.proof_digest"),
            "verified_at_height": cls._unsigned(record.get("verified_at_height"), f"{context}.verified_at_height"),
        }

    @classmethod
    def _ownerships(cls, value: Any) -> List[Dict[str, Any]]:
        context = "sumeragi.lane_payload_ownerships"
        ownerships: List[Dict[str, Any]] = []
        for index, ownership_value in enumerate(
            cls._array(value, context, maximum=cls.MAX_LANE_PAYLOAD_OWNERSHIPS)
        ):
            item_context = f"{context}[{index}]"
            record = cls._mapping(ownership_value, item_context)
            lane_height = cls._unsigned(record.get("lane_block_height"), f"{item_context}.lane_block_height", positive=True)
            indices = [
                cls._unsigned(item, f"{item_context}.accepted_candidate_indices[{offset}]")
                for offset, item in enumerate(cls._array(record.get("accepted_candidate_indices"), f"{item_context}.accepted_candidate_indices"))
            ]
            if not indices or any(left >= right for left, right in zip(indices, indices[1:])):
                raise RuntimeError(
                    f"{item_context}.accepted_candidate_indices must be non-empty and strictly ordered"
                )
            hashes = [
                cls._hash(item, f"{item_context}.accepted_transaction_hashes[{offset}]")
                for offset, item in enumerate(cls._array(record.get("accepted_transaction_hashes"), f"{item_context}.accepted_transaction_hashes"))
            ]
            if len(hashes) != len(indices):
                raise RuntimeError(f"{item_context} candidate/hash counts must match")
            validators = [
                cls._non_empty_string(item, f"{item_context}.lane_block_descriptor_validator_set[{offset}]")
                for offset, item in enumerate(cls._array(record.get("lane_block_descriptor_validator_set"), f"{item_context}.lane_block_descriptor_validator_set"))
            ]
            if (
                not validators
                or len(validators) > cls.MAX_LANE_VALIDATORS
                or len(set(validators)) != len(validators)
            ):
                raise RuntimeError(
                    f"{item_context}.lane_block_descriptor_validator_set must be non-empty and unique"
                )
            validator_count = cls._unsigned(record.get("lane_block_descriptor_validator_count"), f"{item_context}.lane_block_descriptor_validator_count", positive=True, maximum=cls.MAX_LANE_VALIDATORS)
            min_quorum = cls._unsigned(record.get("lane_block_descriptor_min_quorum"), f"{item_context}.lane_block_descriptor_min_quorum", positive=True, maximum=cls.MAX_LANE_VALIDATORS)
            if validator_count != len(validators) or min_quorum > validator_count:
                raise RuntimeError(f"{item_context} descriptor quorum does not match its validator set")
            previous_height = cls._unsigned(record.get("previous_lane_block_height"), f"{item_context}.previous_lane_block_height")
            if previous_height != lane_height - 1:
                raise RuntimeError(f"{item_context}.previous_lane_block_height must precede lane_block_height")
            previous_value = record.get("previous_lane_block_descriptor_hash")
            previous_hash = None if previous_value is None else cls._hash(previous_value, f"{item_context}.previous_lane_block_descriptor_hash")
            if previous_height == 0 and previous_hash is not None:
                raise RuntimeError(f"{item_context} genesis lane block must not name a predecessor descriptor")
            descriptor_value = record.get("lane_block_descriptor_hash")
            if descriptor_value is None:
                raise RuntimeError(f"{item_context}.lane_block_descriptor_hash is required")
            ownerships.append(
                {
                    "proposal_height": cls._unsigned(record.get("proposal_height"), f"{item_context}.proposal_height"),
                    "proposal_view": cls._unsigned(record.get("proposal_view"), f"{item_context}.proposal_view"),
                    "lane_id": cls._unsigned(record.get("lane_id"), f"{item_context}.lane_id", maximum=cls.MAX_U32),
                    "dataspace_id": cls._unsigned(record.get("dataspace_id"), f"{item_context}.dataspace_id"),
                    "lane_incarnation": cls._nonzero_hash(record.get("lane_incarnation"), f"{item_context}.lane_incarnation"),
                    "lane_block_height": lane_height,
                    "lane_block_view": cls._unsigned(record.get("lane_block_view"), f"{item_context}.lane_block_view"),
                    "subject_hash": cls._hash(record.get("subject_hash"), f"{item_context}.subject_hash"),
                    "qc_mode_tag": cls._non_empty_string(record.get("qc_mode_tag"), f"{item_context}.qc_mode_tag"),
                    "accepted_candidate_indices": indices,
                    "accepted_transaction_hashes": hashes,
                    "previous_lane_block_height": previous_height,
                    "previous_lane_block_descriptor_hash": previous_hash,
                    "lane_block_descriptor_hash": cls._hash(descriptor_value, f"{item_context}.lane_block_descriptor_hash"),
                    "lane_block_descriptor_validator_set": validators,
                    "lane_block_descriptor_validator_count": validator_count,
                    "lane_block_descriptor_min_quorum": min_quorum,
                    "payload_ownership_hash": cls._hash(record.get("payload_ownership_hash"), f"{item_context}.payload_ownership_hash"),
                    "rbc_instance_hash": cls._hash(record.get("rbc_instance_hash"), f"{item_context}.rbc_instance_hash"),
                }
            )
        return ownerships

    @classmethod
    def _committed_blocks(cls, value: Any) -> List[Dict[str, Any]]:
        context = "sumeragi.committed_lane_blocks"
        blocks: List[Dict[str, Any]] = []
        for index, block_value in enumerate(
            cls._array(value, context, maximum=cls.MAX_COMMITTED_LANE_BLOCKS)
        ):
            item_context = f"{context}[{index}]"
            record = cls._mapping(block_value, item_context)
            validator_count = cls._unsigned(record.get("validator_count"), f"{item_context}.validator_count", positive=True, maximum=cls.MAX_LANE_VALIDATORS)
            min_quorum = cls._unsigned(record.get("min_quorum"), f"{item_context}.min_quorum", positive=True, maximum=cls.MAX_LANE_VALIDATORS)
            prepare_count = cls._unsigned(record.get("prepare_qc_signer_count"), f"{item_context}.prepare_qc_signer_count", maximum=cls.MAX_U32)
            commit_count = cls._unsigned(record.get("commit_qc_signer_count"), f"{item_context}.commit_qc_signer_count", maximum=cls.MAX_U32)
            if min_quorum > validator_count or prepare_count != min_quorum or commit_count != min_quorum:
                raise RuntimeError(f"{item_context} carries an impossible certified quorum")
            execution_status = cls._non_empty_string(
                record.get("execution_status"), f"{item_context}.execution_status"
            )
            executable = cls._boolean(
                record.get("executable_payload_available"),
                f"{item_context}.executable_payload_available",
            )
            executable_statuses = {
                "payload_available_awaiting_executor",
                "payload_recovered_awaiting_state_application",
                "payload_preflighted_awaiting_state_application",
                "state_applied_by_canonical_block",
                "state_applied_by_direct_execution",
            }
            allowed_statuses = executable_statuses | {
                "awaiting_executable_payload",
                "payload_preflight_rejected_awaiting_state_application",
                "application_receipt_conflicts_with_preflight",
                "awaiting_predecessor_application",
            }
            if execution_status not in allowed_statuses or executable != (
                execution_status in executable_statuses
            ):
                raise RuntimeError(f"{item_context} carries an invalid execution status")
            blocks.append(
                {
                    "lane_id": cls._unsigned(record.get("lane_id"), f"{item_context}.lane_id", maximum=cls.MAX_U32),
                    "dataspace_id": cls._unsigned(record.get("dataspace_id"), f"{item_context}.dataspace_id"),
                    "lane_incarnation": cls._nonzero_hash(record.get("lane_incarnation"), f"{item_context}.lane_incarnation"),
                    "lane_block_height": cls._unsigned(record.get("lane_block_height"), f"{item_context}.lane_block_height", positive=True),
                    "lane_block_view": cls._unsigned(record.get("lane_block_view"), f"{item_context}.lane_block_view"),
                    "descriptor_hash": cls._hash(record.get("descriptor_hash"), f"{item_context}.descriptor_hash"),
                    "proposal_hash": cls._hash(record.get("proposal_hash"), f"{item_context}.proposal_hash"),
                    "execution_status": execution_status,
                    "executable_payload_available": executable,
                    "subject_hash": cls._hash(record.get("subject_hash"), f"{item_context}.subject_hash"),
                    "payload_ownership_hash": cls._hash(record.get("payload_ownership_hash"), f"{item_context}.payload_ownership_hash"),
                    "rbc_instance_hash": cls._hash(record.get("rbc_instance_hash"), f"{item_context}.rbc_instance_hash"),
                    "qc_mode_tag": cls._non_empty_string(record.get("qc_mode_tag"), f"{item_context}.qc_mode_tag"),
                    "validator_count": validator_count,
                    "min_quorum": min_quorum,
                    "prepare_qc_signer_count": prepare_count,
                    "commit_qc_signer_count": commit_count,
                }
            )
        return blocks

    @classmethod
    def _sessions(cls, value: Any) -> List[Dict[str, Any]]:
        context = "sumeragi.lane_block_sessions"
        sessions: List[Dict[str, Any]] = []
        for index, session_value in enumerate(
            cls._array(value, context, maximum=cls.MAX_LANE_BLOCK_SESSIONS)
        ):
            item_context = f"{context}[{index}]"
            record = cls._mapping(session_value, item_context)
            validator_count = cls._unsigned(record.get("validator_count"), f"{item_context}.validator_count", maximum=cls.MAX_LANE_VALIDATORS)
            min_quorum = cls._unsigned(record.get("min_quorum"), f"{item_context}.min_quorum", maximum=cls.MAX_LANE_VALIDATORS)
            prepare_count = cls._unsigned(record.get("prepare_vote_count"), f"{item_context}.prepare_vote_count", maximum=cls.MAX_U32)
            commit_count = cls._unsigned(record.get("commit_vote_count"), f"{item_context}.commit_vote_count", maximum=cls.MAX_U32)
            if validator_count == 0:
                invalid_counts = min_quorum != 0 or prepare_count != 0 or commit_count != 0
            else:
                invalid_counts = (
                    min_quorum == 0
                    or min_quorum > validator_count
                    or prepare_count > validator_count
                    or commit_count > validator_count
                )
            if invalid_counts:
                raise RuntimeError(f"{item_context} carries impossible session quorum counts")
            sessions.append(
                {
                    "lane_id": cls._unsigned(record.get("lane_id"), f"{item_context}.lane_id", maximum=cls.MAX_U32),
                    "dataspace_id": cls._unsigned(record.get("dataspace_id"), f"{item_context}.dataspace_id"),
                    "lane_incarnation": cls._nonzero_hash(record.get("lane_incarnation"), f"{item_context}.lane_incarnation"),
                    "lane_block_height": cls._unsigned(record.get("lane_block_height"), f"{item_context}.lane_block_height"),
                    "lane_block_view": cls._unsigned(record.get("lane_block_view"), f"{item_context}.lane_block_view"),
                    "proposal_hash": cls._hash(record.get("proposal_hash"), f"{item_context}.proposal_hash"),
                    "has_proposal": cls._boolean(record.get("has_proposal"), f"{item_context}.has_proposal"),
                    "prepare_vote_count": prepare_count,
                    "commit_vote_count": commit_count,
                    "has_prepare_qc": cls._boolean(record.get("has_prepare_qc"), f"{item_context}.has_prepare_qc"),
                    "has_commit_qc": cls._boolean(record.get("has_commit_qc"), f"{item_context}.has_commit_qc"),
                    "pending_commit_vote_request": cls._boolean(record.get("pending_commit_vote_request"), f"{item_context}.pending_commit_vote_request"),
                    "pending_committed_session_drain": cls._boolean(record.get("pending_committed_session_drain"), f"{item_context}.pending_committed_session_drain"),
                    "committed_session_drained": cls._boolean(record.get("committed_session_drained"), f"{item_context}.committed_session_drained"),
                    "validator_count": validator_count,
                    "min_quorum": min_quorum,
                }
            )
        return sessions

    @staticmethod
    def _non_empty_string(value: Any, context: str) -> str:
        if not isinstance(value, str) or not value.strip():
            raise RuntimeError(f"{context} must be a non-empty string")
        return value.strip()

    @staticmethod
    def _exact_non_empty_string(value: Any, context: str) -> str:
        if not isinstance(value, str) or not value:
            raise RuntimeError(f"{context} must be a non-empty string")
        if value.strip() != value:
            raise RuntimeError(f"{context} must not contain surrounding whitespace")
        return value


class _SumeragiDiagnosticsParser:
    """Fail-closed parser for non-authoritative Sumeragi diagnostics."""

    MAX_LANES = 128
    MAX_NATIVE_APPLICATIONS = 1_024
    MAX_AUTONOMOUS_EXECUTIONS = 128
    PIPELINE_FIELDS = (
        "tx_vertices_total",
        "tx_edges_total",
        "overlay_count_total",
        "overlay_instr_total",
        "overlay_bytes_total",
        "rbc_chunks_total",
        "rbc_bytes_total",
        "detached_prepared_total",
        "detached_merged_total",
        "detached_fallback_total",
        "detached_fallback_fee_postprocessing_total",
        "detached_fallback_user_executor_total",
        "detached_fallback_durable_state_total",
        "detached_fallback_unsupported_instruction_total",
        "detached_fallback_rejected_eval_total",
        "detached_fallback_overlay_error_total",
        "quarantine_executed_total",
    )
    NPOS_FIELDS = (
        "epoch_length_blocks",
        "vrf_commit_deadline_offset",
        "vrf_reveal_deadline_offset",
        "epoch_seed",
        "prf_height",
        "prf_view",
        "vrf_penalty_epoch",
        "vrf_committed_no_reveal_total",
        "vrf_no_participation_total",
        "vrf_late_reveals_total",
    )

    @classmethod
    def parse(cls, payload: Any) -> SumeragiDiagnosticsStatus:
        record = _SumeragiV2StatusParser._mapping(
            payload, "sumeragi diagnostics"
        )
        fields = {
            "pipeline_execution",
            "tx_queue_depth",
            "tx_queue_capacity",
            "tx_queue_retained_bytes",
            "tx_queue_max_retained_bytes",
            "tx_queue_saturated",
            "tx_queue_saturated_by_count",
            "tx_queue_saturated_by_bytes",
            "tx_queue_saturated_by_age",
            "tx_queue_oldest_queued_age_ms",
            "npos",
            "lane_commitments",
            "dataspace_commitments",
            "lane_settlement_commitments",
            "lane_relay_envelopes",
            "lane_payload_ownerships",
            "committed_lane_blocks",
            "lane_block_sessions",
            "lane_governance_sealed_total",
            "lane_governance_sealed_aliases",
            "lane_governance",
            "native_amx_participant_applications",
            "autonomous_lane_executions",
        }
        unknown = set(record) - fields
        if unknown:
            raise RuntimeError(
                "sumeragi diagnostics contains unknown field "
                f"{sorted(unknown)[0]}"
            )
        required = fields - {"npos"}
        missing = required - set(record)
        if missing:
            raise RuntimeError(
                "sumeragi diagnostics is missing required field "
                f"{sorted(missing)[0]}"
            )

        capacity = cls._unsigned(record, "tx_queue_capacity")
        depth = cls._unsigned(record, "tx_queue_depth")
        retained = cls._unsigned(record, "tx_queue_retained_bytes")
        max_retained = cls._unsigned(record, "tx_queue_max_retained_bytes")
        if depth > capacity:
            raise RuntimeError(
                "sumeragi diagnostics transaction queue depth exceeds capacity"
            )
        if retained > max_retained:
            raise RuntimeError(
                "sumeragi diagnostics retained queue bytes exceed the byte budget"
            )
        saturated = cls._boolean(record, "tx_queue_saturated")
        saturated_by_count = cls._boolean(
            record, "tx_queue_saturated_by_count"
        )
        saturated_by_bytes = cls._boolean(
            record, "tx_queue_saturated_by_bytes"
        )
        saturated_by_age = cls._boolean(record, "tx_queue_saturated_by_age")
        if saturated != (
            saturated_by_count or saturated_by_bytes or saturated_by_age
        ):
            raise RuntimeError(
                "sumeragi diagnostics.tx_queue_saturated disagrees with its causes"
            )

        aliases = cls._string_array(
            record.get("lane_governance_sealed_aliases"),
            "sumeragi diagnostics.lane_governance_sealed_aliases",
        )
        sealed_total = cls._unsigned(
            record,
            "lane_governance_sealed_total",
            maximum=_SumeragiV2StatusParser.MAX_U32,
        )
        if sealed_total != len(aliases) or len(set(aliases)) != len(aliases):
            raise RuntimeError(
                "sumeragi diagnostics sealed lane aliases must be unique and "
                "match lane_governance_sealed_total"
            )

        return SumeragiDiagnosticsStatus(
            pipeline_execution=cls._pipeline(record.get("pipeline_execution")),
            tx_queue_depth=depth,
            tx_queue_capacity=capacity,
            tx_queue_retained_bytes=retained,
            tx_queue_max_retained_bytes=max_retained,
            tx_queue_saturated=saturated,
            tx_queue_saturated_by_count=saturated_by_count,
            tx_queue_saturated_by_bytes=saturated_by_bytes,
            tx_queue_saturated_by_age=saturated_by_age,
            tx_queue_oldest_queued_age_ms=cls._unsigned(
                record, "tx_queue_oldest_queued_age_ms"
            ),
            npos=cls._npos(record.get("npos")),
            lane_commitments=cls._lane_commitments(
                record.get("lane_commitments")
            ),
            dataspace_commitments=cls._dataspace_commitments(
                record.get("dataspace_commitments")
            ),
            lane_settlement_commitments=_SumeragiV2StatusParser._settlements(
                record.get("lane_settlement_commitments")
            ),
            lane_relay_envelopes=_SumeragiV2StatusParser._relays(
                record.get("lane_relay_envelopes")
            ),
            lane_payload_ownerships=_SumeragiV2StatusParser._ownerships(
                record.get("lane_payload_ownerships")
            ),
            committed_lane_blocks=_SumeragiV2StatusParser._committed_blocks(
                record.get("committed_lane_blocks")
            ),
            lane_block_sessions=_SumeragiV2StatusParser._sessions(
                record.get("lane_block_sessions")
            ),
            lane_governance_sealed_total=sealed_total,
            lane_governance_sealed_aliases=aliases,
            lane_governance=cls._lane_governance(record.get("lane_governance")),
            native_amx_participant_applications=cls._native_applications(
                record.get("native_amx_participant_applications")
            ),
            autonomous_lane_executions=cls._autonomous_executions(
                record.get("autonomous_lane_executions")
            ),
        )

    @classmethod
    def _pipeline(cls, value: Any) -> SumeragiPipelineExecutionStatus:
        record = _SumeragiV2StatusParser._exact_mapping(
            value,
            "sumeragi diagnostics.pipeline_execution",
            set(cls.PIPELINE_FIELDS),
        )
        return SumeragiPipelineExecutionStatus(
            **{
                field: cls._unsigned(record, field, prefix="pipeline_execution")
                for field in cls.PIPELINE_FIELDS
            }
        )

    @classmethod
    def _npos(cls, value: Any) -> Optional[SumeragiNposDiagnostics]:
        if value is None:
            return None
        record = _SumeragiV2StatusParser._exact_mapping(
            value, "sumeragi diagnostics.npos", set(cls.NPOS_FIELDS)
        )
        epoch_length = cls._unsigned(
            record, "epoch_length_blocks", positive=True, prefix="npos"
        )
        commit = cls._unsigned(
            record, "vrf_commit_deadline_offset", positive=True, prefix="npos"
        )
        reveal = cls._unsigned(
            record, "vrf_reveal_deadline_offset", positive=True, prefix="npos"
        )
        if not commit < reveal <= epoch_length:
            raise RuntimeError(
                "sumeragi diagnostics NPoS windows must be strictly ordered "
                "within the epoch"
            )
        seed = tuple(
            _SumeragiV2StatusParser._byte_vector(
                record.get("epoch_seed"),
                32,
                "sumeragi diagnostics.npos.epoch_seed",
            )
        )
        if not any(seed):
            raise RuntimeError(
                "sumeragi diagnostics.npos.epoch_seed must not be zero"
            )
        return SumeragiNposDiagnostics(
            epoch_length_blocks=epoch_length,
            vrf_commit_deadline_offset=commit,
            vrf_reveal_deadline_offset=reveal,
            epoch_seed=seed,
            prf_height=cls._unsigned(record, "prf_height", prefix="npos"),
            prf_view=cls._unsigned(record, "prf_view", prefix="npos"),
            vrf_penalty_epoch=cls._unsigned(
                record, "vrf_penalty_epoch", prefix="npos"
            ),
            vrf_committed_no_reveal_total=cls._unsigned(
                record, "vrf_committed_no_reveal_total", prefix="npos"
            ),
            vrf_no_participation_total=cls._unsigned(
                record, "vrf_no_participation_total", prefix="npos"
            ),
            vrf_late_reveals_total=cls._unsigned(
                record, "vrf_late_reveals_total", prefix="npos"
            ),
        )

    @classmethod
    def _lane_commitments(
        cls, value: Any
    ) -> List[SumeragiLaneCommitmentStatus]:
        fields = {
            "block_height",
            "lane_id",
            "tx_count",
            "total_chunks",
            "rbc_bytes_total",
            "teu_total",
            "block_hash",
        }
        result: List[SumeragiLaneCommitmentStatus] = []
        for index, item in enumerate(
            _SumeragiV2StatusParser._array(
                value,
                "sumeragi diagnostics.lane_commitments",
                maximum=cls.MAX_LANES,
            )
        ):
            context = f"sumeragi diagnostics.lane_commitments[{index}]"
            record = _SumeragiV2StatusParser._exact_mapping(
                item, context, fields
            )
            result.append(
                SumeragiLaneCommitmentStatus(
                    block_height=cls._unsigned(
                        record, "block_height", prefix=context
                    ),
                    lane_id=cls._unsigned(
                        record,
                        "lane_id",
                        maximum=_SumeragiV2StatusParser.MAX_U32,
                        prefix=context,
                    ),
                    tx_count=cls._unsigned(record, "tx_count", prefix=context),
                    total_chunks=cls._unsigned(
                        record, "total_chunks", prefix=context
                    ),
                    rbc_bytes_total=cls._unsigned(
                        record, "rbc_bytes_total", prefix=context
                    ),
                    teu_total=cls._unsigned(record, "teu_total", prefix=context),
                    block_hash=_SumeragiV2StatusParser._hash(
                        record.get("block_hash"), f"{context}.block_hash"
                    ),
                )
            )
        return result

    @classmethod
    def _dataspace_commitments(
        cls, value: Any
    ) -> List[SumeragiDataspaceCommitmentStatus]:
        fields = {
            "block_height",
            "lane_id",
            "dataspace_id",
            "tx_count",
            "total_chunks",
            "rbc_bytes_total",
            "teu_total",
            "block_hash",
        }
        result: List[SumeragiDataspaceCommitmentStatus] = []
        for index, item in enumerate(
            _SumeragiV2StatusParser._array(
                value,
                "sumeragi diagnostics.dataspace_commitments",
                maximum=cls.MAX_LANES,
            )
        ):
            context = f"sumeragi diagnostics.dataspace_commitments[{index}]"
            record = _SumeragiV2StatusParser._exact_mapping(
                item, context, fields
            )
            result.append(
                SumeragiDataspaceCommitmentStatus(
                    block_height=cls._unsigned(
                        record, "block_height", prefix=context
                    ),
                    lane_id=cls._unsigned(
                        record,
                        "lane_id",
                        maximum=_SumeragiV2StatusParser.MAX_U32,
                        prefix=context,
                    ),
                    dataspace_id=cls._unsigned(
                        record, "dataspace_id", prefix=context
                    ),
                    tx_count=cls._unsigned(record, "tx_count", prefix=context),
                    total_chunks=cls._unsigned(
                        record, "total_chunks", prefix=context
                    ),
                    rbc_bytes_total=cls._unsigned(
                        record, "rbc_bytes_total", prefix=context
                    ),
                    teu_total=cls._unsigned(record, "teu_total", prefix=context),
                    block_hash=_SumeragiV2StatusParser._hash(
                        record.get("block_hash"), f"{context}.block_hash"
                    ),
                )
            )
        return result

    @classmethod
    def _lane_governance(
        cls, value: Any
    ) -> List[SumeragiLaneGovernanceStatus]:
        fields = {
            "lane_id",
            "alias",
            "governance",
            "manifest_required",
            "manifest_ready",
            "manifest_path",
            "validator_ids",
            "quorum",
            "protected_namespaces",
            "runtime_upgrade",
        }
        result: List[SumeragiLaneGovernanceStatus] = []
        for index, item in enumerate(
            _SumeragiV2StatusParser._array(
                value,
                "sumeragi diagnostics.lane_governance",
                maximum=cls.MAX_LANES,
            )
        ):
            context = f"sumeragi diagnostics.lane_governance[{index}]"
            record = _SumeragiV2StatusParser._exact_mapping(
                item, context, fields
            )
            validator_ids = cls._string_array(
                record.get("validator_ids"), f"{context}.validator_ids"
            )
            if len(set(validator_ids)) != len(validator_ids):
                raise RuntimeError(f"{context}.validator_ids contains duplicates")
            namespaces = cls._string_array(
                record.get("protected_namespaces"),
                f"{context}.protected_namespaces",
            )
            if len(set(namespaces)) != len(namespaces):
                raise RuntimeError(
                    f"{context}.protected_namespaces contains duplicates"
                )
            governance = cls._optional_string(
                record.get("governance"), f"{context}.governance"
            )
            manifest_path = cls._optional_string(
                record.get("manifest_path"), f"{context}.manifest_path"
            )
            quorum_value = record.get("quorum")
            quorum = (
                None
                if quorum_value is None
                else _SumeragiV2StatusParser._unsigned(
                    quorum_value,
                    f"{context}.quorum",
                    positive=True,
                    maximum=_SumeragiV2StatusParser.MAX_U32,
                )
            )
            if quorum is not None and quorum > len(validator_ids):
                raise RuntimeError(
                    f"{context}.quorum exceeds the validator roster"
                )
            runtime_value = record.get("runtime_upgrade")
            runtime = (
                None
                if runtime_value is None
                else cls._runtime_upgrade(runtime_value, f"{context}.runtime_upgrade")
            )
            result.append(
                SumeragiLaneGovernanceStatus(
                    lane_id=cls._unsigned(
                        record,
                        "lane_id",
                        maximum=_SumeragiV2StatusParser.MAX_U32,
                        prefix=context,
                    ),
                    alias=_SumeragiV2StatusParser._non_empty_string(
                        record.get("alias"), f"{context}.alias"
                    ),
                    governance=governance,
                    manifest_required=cls._boolean(
                        record, "manifest_required", prefix=context
                    ),
                    manifest_ready=cls._boolean(
                        record, "manifest_ready", prefix=context
                    ),
                    manifest_path=manifest_path,
                    validator_ids=validator_ids,
                    quorum=quorum,
                    protected_namespaces=namespaces,
                    runtime_upgrade=runtime,
                )
            )
        return result

    @classmethod
    def _runtime_upgrade(cls, value: Any, context: str) -> Dict[str, Any]:
        record = _SumeragiV2StatusParser._exact_mapping(
            value,
            context,
            {"allow", "require_metadata", "metadata_key", "allowed_ids"},
        )
        allowed_ids = cls._string_array(
            record.get("allowed_ids"), f"{context}.allowed_ids"
        )
        if len(set(allowed_ids)) != len(allowed_ids):
            raise RuntimeError(f"{context}.allowed_ids contains duplicates")
        return {
            "allow": cls._boolean(record, "allow", prefix=context),
            "require_metadata": cls._boolean(
                record, "require_metadata", prefix=context
            ),
            "metadata_key": cls._optional_string(
                record.get("metadata_key"), f"{context}.metadata_key"
            ),
            "allowed_ids": allowed_ids,
        }

    @classmethod
    def _native_applications(
        cls, value: Any
    ) -> List[SumeragiNativeAmxParticipantApplication]:
        allowed = {
            "lane_id",
            "dataspace_id",
            "lane_incarnation",
            "participant_height",
            "participant_view",
            "predecessor_height",
            "predecessor_descriptor_hash",
            "descriptor_hash",
            "proposal_hash",
            "settlement_hash",
            "source_count",
            "application_block_height",
            "application_block_hash",
            "state",
        }
        required = allowed - {
            "predecessor_descriptor_hash",
            "application_block_height",
            "application_block_hash",
        }
        result: List[SumeragiNativeAmxParticipantApplication] = []
        previous_key: Optional[Tuple[int, int, str]] = None
        for index, item in enumerate(
            _SumeragiV2StatusParser._array(
                value,
                "sumeragi diagnostics.native_amx_participant_applications",
                maximum=cls.MAX_NATIVE_APPLICATIONS,
            )
        ):
            context = (
                "sumeragi diagnostics.native_amx_participant_applications"
                f"[{index}]"
            )
            record = _SumeragiV2StatusParser._mapping(item, context)
            unknown = set(record) - allowed
            missing = required - set(record)
            if unknown or missing:
                field = sorted(unknown or missing)[0]
                problem = "unknown" if unknown else "missing required"
                raise RuntimeError(f"{context} contains {problem} field {field}")
            lane_id = cls._unsigned(
                record,
                "lane_id",
                maximum=_SumeragiV2StatusParser.MAX_U32,
                prefix=context,
            )
            dataspace_id = cls._unsigned(
                record, "dataspace_id", prefix=context
            )
            incarnation = _SumeragiV2StatusParser._nonzero_hash(
                record.get("lane_incarnation"), f"{context}.lane_incarnation"
            )
            key = (lane_id, dataspace_id, incarnation)
            if previous_key is not None and previous_key >= key:
                raise RuntimeError(
                    "sumeragi diagnostics native participant applications "
                    "must be strictly ordered by route and incarnation"
                )
            previous_key = key
            participant_height = cls._unsigned(
                record, "participant_height", positive=True, prefix=context
            )
            predecessor_height = cls._unsigned(
                record, "predecessor_height", prefix=context
            )
            predecessor_value = record.get("predecessor_descriptor_hash")
            predecessor_hash = (
                None
                if predecessor_value is None
                else _SumeragiV2StatusParser._nonzero_hash(
                    predecessor_value,
                    f"{context}.predecessor_descriptor_hash",
                )
            )
            if (
                predecessor_height + 1 != participant_height
                or (predecessor_height == 0) != (predecessor_hash is None)
            ):
                raise RuntimeError(
                    f"{context} contains inconsistent predecessor geometry"
                )
            application_height_value = record.get("application_block_height")
            application_hash_value = record.get("application_block_hash")
            application_height = (
                None
                if application_height_value is None
                else _SumeragiV2StatusParser._unsigned(
                    application_height_value,
                    f"{context}.application_block_height",
                    positive=True,
                )
            )
            application_hash = (
                None
                if application_hash_value is None
                else _SumeragiV2StatusParser._nonzero_hash(
                    application_hash_value,
                    f"{context}.application_block_hash",
                )
            )
            if (application_height is None) != (application_hash is None):
                raise RuntimeError(
                    f"{context} application block height and hash must appear together"
                )
            states = {
                "certified_pending_carrier",
                "committed_evidence_pending",
                "durably_applied",
                "conflict",
            }
            state = record.get("state")
            if not isinstance(state, str) or state not in states:
                raise RuntimeError(f"{context}.state has an unknown variant")
            requires_application_block = state in {
                "committed_evidence_pending",
                "durably_applied",
            }
            if (application_height is not None) != requires_application_block:
                raise RuntimeError(
                    f"{context} state and application block identity disagree"
                )
            result.append(
                SumeragiNativeAmxParticipantApplication(
                    lane_id=lane_id,
                    dataspace_id=dataspace_id,
                    lane_incarnation=incarnation,
                    participant_height=participant_height,
                    participant_view=cls._unsigned(
                        record, "participant_view", prefix=context
                    ),
                    predecessor_height=predecessor_height,
                    predecessor_descriptor_hash=predecessor_hash,
                    descriptor_hash=_SumeragiV2StatusParser._nonzero_hash(
                        record.get("descriptor_hash"),
                        f"{context}.descriptor_hash",
                    ),
                    proposal_hash=_SumeragiV2StatusParser._nonzero_hash(
                        record.get("proposal_hash"), f"{context}.proposal_hash"
                    ),
                    settlement_hash=_SumeragiV2StatusParser._nonzero_hash(
                        record.get("settlement_hash"),
                        f"{context}.settlement_hash",
                    ),
                    source_count=cls._unsigned(
                        record,
                        "source_count",
                        positive=True,
                        maximum=4096,
                        prefix=context,
                    ),
                    application_block_height=application_height,
                    application_block_hash=application_hash,
                    state=state,
                )
            )
        return result

    @classmethod
    def _autonomous_executions(
        cls, value: Any
    ) -> List[SumeragiAutonomousLaneExecution]:
        allowed = {
            "lane_id", "dataspace_id", "lane_incarnation", "lane_block_height",
            "lane_block_view", "proposal_height", "proposal_view",
            "reservation_owner_hash", "proposal_identity_hash",
            "reservation_group_hash", "proposal_hash", "descriptor_hash",
            "executable_payload_hash", "source_bundle_hash",
            "merge_entry_hash", "application_block_height", "application_block_hash",
            "reservation_count", "transaction_count", "highest_durable_stage",
            "stuck_reason",
        }
        optional = {
            "proposal_view", "proposal_hash", "descriptor_hash",
            "executable_payload_hash", "source_bundle_hash", "merge_entry_hash",
            "application_block_height", "application_block_hash", "stuck_reason",
        }
        stages = {
            "reservations_durable", "executable_payload_durable",
            "payload_availability_certified", "lane_certified",
            "certified_bundle_durable", "merge_candidate_durable",
            "global_carrier_committed", "kura_wsv_application_receipt_durable",
            "queue_finalized", "conflict",
        }
        reasons = {
            "awaiting_executable_payload", "awaiting_payload_availability",
            "awaiting_lane_certification",
            "certified_bundle_unavailable", "awaiting_merge_selection",
            "awaiting_global_carrier", "awaiting_application_receipt",
            "queue_finalization_unverifiable", "evidence_conflict",
        }
        result: List[SumeragiAutonomousLaneExecution] = []
        previous_key: Optional[Tuple[Any, ...]] = None
        rows = _SumeragiV2StatusParser._array(
            value,
            "sumeragi diagnostics.autonomous_lane_executions",
            maximum=cls.MAX_AUTONOMOUS_EXECUTIONS,
        )

        def optional_hash(
            record: Mapping[str, Any], context: str, field: str
        ) -> Optional[str]:
            value = record.get(field)
            if value is None:
                return None
            return _SumeragiV2StatusParser._nonzero_hash(
                value, f"{context}.{field}"
            )

        for index, item in enumerate(rows):
            context = f"sumeragi diagnostics.autonomous_lane_executions[{index}]"
            record = _SumeragiV2StatusParser._mapping(item, context)
            unknown = set(record) - allowed
            missing = (allowed - optional) - set(record)
            if unknown or missing:
                field = sorted(unknown or missing)[0]
                problem = "unknown" if unknown else "missing required"
                raise RuntimeError(f"{context} contains {problem} field {field}")
            lane_id = cls._unsigned(
                record, "lane_id", maximum=_SumeragiV2StatusParser.MAX_U32,
                prefix=context,
            )
            dataspace_id = cls._unsigned(record, "dataspace_id", prefix=context)
            incarnation = _SumeragiV2StatusParser._nonzero_hash(
                record.get("lane_incarnation"), f"{context}.lane_incarnation"
            )
            lane_height = cls._unsigned(
                record, "lane_block_height", positive=True, prefix=context
            )
            lane_view = cls._unsigned(record, "lane_block_view", prefix=context)
            proposal_height = cls._unsigned(
                record, "proposal_height", positive=True, prefix=context
            )
            proposal_view = (
                None
                if record.get("proposal_view") is None
                else cls._unsigned(record, "proposal_view", prefix=context)
            )
            reservation_owner_hash = _SumeragiV2StatusParser._nonzero_hash(
                record.get("reservation_owner_hash"),
                f"{context}.reservation_owner_hash",
            )
            proposal_identity_hash = _SumeragiV2StatusParser._nonzero_hash(
                record.get("proposal_identity_hash"),
                f"{context}.proposal_identity_hash",
            )
            reservation_group_hash = _SumeragiV2StatusParser._nonzero_hash(
                record.get("reservation_group_hash"),
                f"{context}.reservation_group_hash",
            )
            key = (
                lane_id, dataspace_id, incarnation, lane_height, lane_view,
                proposal_height, proposal_identity_hash,
            )
            if previous_key is not None and previous_key >= key:
                raise RuntimeError(
                    "sumeragi diagnostics autonomous lane executions must be "
                    "strictly ordered by exact identity"
                )
            previous_key = key
            application_height = (
                None if record.get("application_block_height") is None else
                _SumeragiV2StatusParser._unsigned(
                    record.get("application_block_height"),
                    f"{context}.application_block_height", positive=True,
                )
            )
            application_hash = optional_hash(
                record, context, "application_block_hash"
            )
            if (application_height is None) != (application_hash is None):
                raise RuntimeError(
                    f"{context} application block height and hash must appear together"
                )
            reservation_count = cls._unsigned(
                record, "reservation_count", maximum=4096, prefix=context
            )
            transaction_count = cls._unsigned(
                record, "transaction_count", positive=True, maximum=4096,
                prefix=context,
            )
            stage = record.get("highest_durable_stage")
            if not isinstance(stage, str) or stage not in stages:
                raise RuntimeError(f"{context}.highest_durable_stage has an unknown variant")
            reason_value = record.get("stuck_reason")
            if reason_value is not None and (
                not isinstance(reason_value, str) or reason_value not in reasons
            ):
                raise RuntimeError(f"{context}.stuck_reason has an unknown variant")
            reason = reason_value
            expected_reasons = {
                "reservations_durable": "awaiting_executable_payload",
                "executable_payload_durable": "awaiting_payload_availability",
                "payload_availability_certified": "awaiting_lane_certification",
                "lane_certified": "certified_bundle_unavailable",
                "certified_bundle_durable": "awaiting_merge_selection",
                "merge_candidate_durable": "awaiting_global_carrier",
                "global_carrier_committed": "awaiting_application_receipt",
                "kura_wsv_application_receipt_durable":
                    "queue_finalization_unverifiable",
                "queue_finalized": None,
                "conflict": "evidence_conflict",
            }
            if reason != expected_reasons[stage]:
                raise RuntimeError(f"{context} stage and stuck reason disagree")
            if stage != "conflict" and reservation_count != transaction_count:
                raise RuntimeError(f"{context} reservation and transaction counts disagree")
            proposal_hash = optional_hash(record, context, "proposal_hash")
            descriptor_hash = optional_hash(record, context, "descriptor_hash")
            if (proposal_hash is None) != (descriptor_hash is None):
                raise RuntimeError(
                    f"{context} proposal and descriptor hashes must appear together"
                )
            if stage != "conflict" and (
                (stage == "reservations_durable") != (proposal_hash is None)
            ):
                raise RuntimeError(
                    f"{context} finalized identity disagrees with durable stage"
                )
            if stage == "reservations_durable" and proposal_view is not None:
                raise RuntimeError(
                    f"{context} proposal view disagrees with durable stage"
                )
            payload_hash = optional_hash(
                record, context, "executable_payload_hash"
            )
            bundle_hash = optional_hash(record, context, "source_bundle_hash")
            merge_hash = optional_hash(record, context, "merge_entry_hash")
            if stage != "conflict":
                geometry = {
                    "reservations_durable": (False, False, False, False),
                    "executable_payload_durable": (True, False, False, False),
                    "payload_availability_certified": (True, False, False, False),
                    "lane_certified": (True, False, False, False),
                    "certified_bundle_durable": (True, True, False, False),
                    "merge_candidate_durable": (True, True, True, False),
                    "global_carrier_committed": (True, True, True, False),
                    "kura_wsv_application_receipt_durable": (True, True, True, True),
                    "queue_finalized": (True, True, True, True),
                }[stage]
                observed = (
                    payload_hash is not None,
                    bundle_hash is not None,
                    merge_hash is not None,
                    application_height is not None,
                )
                if observed != geometry:
                    raise RuntimeError(f"{context} evidence does not match durable stage")
            result.append(SumeragiAutonomousLaneExecution(
                lane_id=lane_id, dataspace_id=dataspace_id,
                lane_incarnation=incarnation, lane_block_height=lane_height,
                lane_block_view=lane_view, proposal_height=proposal_height,
                proposal_view=proposal_view,
                reservation_owner_hash=reservation_owner_hash,
                proposal_identity_hash=proposal_identity_hash,
                reservation_group_hash=reservation_group_hash,
                proposal_hash=proposal_hash,
                descriptor_hash=descriptor_hash,
                executable_payload_hash=payload_hash,
                source_bundle_hash=bundle_hash,
                merge_entry_hash=merge_hash,
                application_block_height=application_height,
                application_block_hash=application_hash,
                reservation_count=reservation_count,
                transaction_count=transaction_count,
                highest_durable_stage=stage, stuck_reason=reason,
            ))
        return result

    @staticmethod
    def _unsigned(
        record: Mapping[str, Any],
        field: str,
        *,
        positive: bool = False,
        maximum: int = _SumeragiV2StatusParser.MAX_U64,
        prefix: str = "sumeragi diagnostics",
    ) -> int:
        return _SumeragiV2StatusParser._unsigned(
            record.get(field),
            f"{prefix}.{field}",
            positive=positive,
            maximum=maximum,
        )

    @staticmethod
    def _boolean(
        record: Mapping[str, Any],
        field: str,
        *,
        prefix: str = "sumeragi diagnostics",
    ) -> bool:
        return _SumeragiV2StatusParser._boolean(
            record.get(field), f"{prefix}.{field}"
        )

    @staticmethod
    def _optional_string(value: Any, context: str) -> Optional[str]:
        if value is None:
            return None
        return _SumeragiV2StatusParser._non_empty_string(value, context)

    @classmethod
    def _string_array(cls, value: Any, context: str) -> List[str]:
        return [
            _SumeragiV2StatusParser._non_empty_string(
                item, f"{context}[{index}]"
            )
            for index, item in enumerate(
                _SumeragiV2StatusParser._array(
                    value, context, maximum=cls.MAX_LANES
                )
            )
        ]


_ToriiClientGovernanceBallotMixin = create_governance_ballot_client_mixin(
    canonical_auth_type=ToriiCanonicalRequestAuth,
    ballot_submit_result_type=BallotSubmitResult,
    offline_hash_literal=_offline_hash_literal,
    canonical_quantity=_canonical_quantity,
)
_ToriiClientSpaceDirectoryMixin = create_space_directory_client_mixin(
    canonical_auth_type=ToriiCanonicalRequestAuth, local_signing_context_type=ToriiLocalSigningContext,
    normalize_network_id=_offline_hash_literal, transaction_draft_type=AppApiTransactionDraft,
)
_ToriiClientKaigiRelayMixin = create_kaigi_relay_client_mixin(
    relay_summary_list_type=KaigiRelaySummaryList,
    relay_detail_type=KaigiRelayDetail,
    relay_health_type=KaigiRelayHealthSnapshot,
)


class ToriiClient(
    SorafsOrderbookSubmissionMixin,
    _ToriiClientKaigiRelayMixin,
    _ToriiClientSpaceDirectoryMixin,
    _ToriiClientGovernanceBallotMixin,
    RuntimeGovernanceAuthMixin,
):
    """HTTP helper for Torii attachments, prover, and governance endpoints."""
    def __init__(
        self,
        base_url: str,
        session: Optional[requests.Session] = None,
        *,
        local_signing_context: Optional[ToriiLocalSigningContext] = None,
        operator_signing_context: Optional[ToriiOperatorSigningContext] = None,
        orderbook_native_verifier: Any = None,
        orderbook_chain_discriminant: Optional[int] = None,
    ) -> None:
        if operator_signing_context is not None and not isinstance(
            operator_signing_context,
            ToriiOperatorSigningContext,
        ):
            raise TypeError(
                "operator_signing_context must be ToriiOperatorSigningContext"
            )
        self._base_url = base_url.rstrip("/")
        self._session = session or requests.Session()
        self._status_state = _StatusMetricsState()
        self._local_signing_context = local_signing_context
        self._operator_signing_context = operator_signing_context
        self._configure_sorafs_orderbook_native_verifier(orderbook_native_verifier)
        self._orderbook_chain_discriminant = orderbook_chain_discriminant

    def _sorafs_orderbook_expected_chain_discriminant(self, context: str) -> int:
        return require_orderbook_chain_discriminant(self._orderbook_chain_discriminant, context)

    # ------------------------------------------------------------------
    # Attachments
    # ------------------------------------------------------------------
    def upload_attachment(
        self, data: bytes, *, content_type: str, canonical_auth: ToriiCanonicalRequestAuth
    ) -> Mapping[str, Any]:
        """Upload an attachment via ``POST /v1/zk/attachments`` and return metadata."""

        response = authenticated_attachment_request(
            self, "POST", "/v1/zk/attachments", canonical_auth,
            data=data,
            headers={"Content-Type": content_type},
        )
        self._expect_status(response, {201})
        return response.json()

    def list_attachments(
        self, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> List[Mapping[str, Any]]:
        """Return metadata for all stored attachments."""

        response = authenticated_attachment_request(
            self, "GET", "/v1/zk/attachments", canonical_auth,
        )
        self._expect_status(response, {200})
        return response.json()

    def get_attachment(
        self, attachment_id: str, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> Tuple[bytes, Optional[str]]:
        """Fetch raw attachment bytes and the optional content type."""

        normalized_id = _require_exact_non_empty_string(attachment_id, "attachment_id")
        path = f"/v1/zk/attachments/{quote(normalized_id, safe='')}"
        response = authenticated_attachment_request(self, "GET", path, canonical_auth)
        self._expect_status(response, {200})
        return response.content, response.headers.get("Content-Type")

    def delete_attachment(
        self, attachment_id: str, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> None:
        """Delete an attachment by id."""

        normalized_id = _require_exact_non_empty_string(attachment_id, "attachment_id")
        path = f"/v1/zk/attachments/{quote(normalized_id, safe='')}"
        response = authenticated_attachment_request(self, "DELETE", path, canonical_auth)
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
        """Return the operator-authenticated node-local online peer snapshot."""

        response = self._operator_get("/v1/peers")
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
    # SoraFS orderbook
    # ------------------------------------------------------------------
    def get_sorafs_orderbook(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """Fetch one finalized native order page and authoritative ledger status."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/book",
            params=self._sorafs_orderbook_read_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_id_hex=after_id_hex,
                limit=limit,
                context="get_sorafs_orderbook",
            ),
            headers=self._sorafs_orderbook_headers(
                headers=headers,
                context="get_sorafs_orderbook",
            ),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook book endpoint returned no payload")
        return self._parse_sorafs_orderbook_book(payload, context="sorafs orderbook book response")

    def list_sorafs_orderbook_trades(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """List finalized native SoraFS orderbook trades."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/trades",
            params=self._sorafs_orderbook_read_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_id_hex=after_id_hex,
                limit=limit,
                context="list_sorafs_orderbook_trades",
            ),
            headers=self._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_trades",
            ),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook trades endpoint returned no payload")
        return self._parse_sorafs_orderbook_trade_page_response(
            payload,
            context="sorafs orderbook trades response",
        )

    def list_sorafs_orderbook_channels(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """List finalized native SoraFS settlement channels."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/channels",
            params=self._sorafs_orderbook_read_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_id_hex=after_id_hex,
                limit=limit,
                context="list_sorafs_orderbook_channels",
            ),
            headers=self._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_channels",
            ),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook channels endpoint returned no payload")
        return self._parse_sorafs_orderbook_channel_page_response(
            payload,
            context="sorafs orderbook channels response",
        )

    def list_sorafs_orderbook_receipts(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """List finalized native SoraFS settlement receipts."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/receipts",
            params=self._sorafs_orderbook_read_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_id_hex=after_id_hex,
                limit=limit,
                context="list_sorafs_orderbook_receipts",
            ),
            headers=self._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_receipts",
            ),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook receipts endpoint returned no payload")
        return self._parse_sorafs_orderbook_receipt_page_response(
            payload,
            context="sorafs orderbook receipts response",
        )

    def list_sorafs_orderbook_events(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_sequence: Optional[Any] = None,
        after_block_height: Optional[Any] = None,
        after_block_hash_hex: Optional[Any] = None,
        after_event_index: Optional[Any] = None,
        limit: Optional[Any] = None,
        if_none_match: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Optional[Dict[str, Any]]:
        """List replayable finalized native SoraFS orderbook events."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/events",
            params=self._sorafs_orderbook_event_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_sequence=after_sequence,
                after_block_height=after_block_height,
                after_block_hash_hex=after_block_hash_hex,
                after_event_index=after_event_index,
                limit=limit,
                context="list_sorafs_orderbook_events",
            ),
            headers=self._sorafs_orderbook_headers(
                if_none_match=if_none_match,
                headers=headers,
                context="list_sorafs_orderbook_events",
                cache=True,
            ),
        )
        self._expect_status(response, {200, 304})
        if response.status_code == 304:
            return None
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook events endpoint returned no payload")
        return self._parse_sorafs_orderbook_event_page_response(
            payload,
            context="sorafs orderbook events response",
        )

    # ------------------------------------------------------------------
    # Explorer helpers
    # ------------------------------------------------------------------
    def get_explorer_account_qr(
        self,
        account_id: str,
    ) -> ExplorerAccountQr:
        """Fetch QR metadata for an account (`GET /v1/explorer/accounts/{account_id}/qr`)."""

        canonical = self._normalize_canonical_account_id(account_id, "account_id")
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
        """Update mutable node configuration (`POST /v1/configuration`).

        Confidential gas is consensus-relevant startup state and is rejected here.
        """

        if "confidential_gas" in payload:
            raise ValueError(
                "confidential_gas is read-only runtime state; change the startup "
                "configuration and restart the node"
            )
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
        """Return the read-only confidential verification gas schedule."""

        snapshot = self.get_configuration()
        return snapshot.confidential_gas

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
        """Fetch operator-authenticated node-local Network Time diagnostics."""

        response = self._operator_get("/v1/time/status")
        self._expect_status(response, {200})
        payload = response.json()
        mapping = self._ensure_mapping(payload, "network time status response")
        return NetworkTimeStatus.from_payload(mapping)

    def get_sccp_capabilities(self) -> SccpCapabilities:
        """Fetch exact SCCP capability discovery (`GET /v1/sccp/capabilities`)."""

        payload = self._get_sccp_json_object(
            "/v1/sccp/capabilities",
            context="sccp capabilities",
            maximum_body_bytes=_SCCP_CAPABILITIES_RESPONSE_MAX_BYTES,
        )
        return normalize_sccp_capabilities(payload)

    def get_sccp_registry(self) -> SccpRegistry:
        """Fetch the authoritative typed SCCP registry (`GET /v1/sccp/registry`)."""

        payload = self._get_sccp_json_object(
            "/v1/sccp/registry",
            context="sccp registry",
            maximum_body_bytes=_SCCP_JSON_RESPONSE_MAX_BYTES,
        )
        return normalize_sccp_registry(payload)

    def get_sccp_message_bundle(
        self, message_id: str, *, format: str = "json"
    ) -> Union[Mapping[str, Any], bytes]:
        """Fetch one state-derived message/finality bundle by canonical message id.

        Native responses are preflighted as canonical uncompressed Norito frames bound to
        ``TairaSccpMessageProofV1``. The frame remains opaque, so this lightweight client does
        not independently bind the embedded message id to the request path.
        """

        return self._get_sccp_typed_object(
            f"/v1/sccp/proofs/message/{self._sccp_message_id(message_id)}",
            format=format,
            context="sccp message bundle",
            normalize=normalize_sccp_message_bundle,
            maximum_norito_body_bytes=_SCCP_NATIVE_NORITO_RESPONSE_MAX_BYTES,
            expected_norito_type_name=_SCCP_MESSAGE_BUNDLE_NORITO_TYPE_NAME,
        )

    def get_sccp_proof_request(
        self, message_id: str, *, format: str = "json"
    ) -> Union[Mapping[str, Any], bytes]:
        """Fetch one query-free state-derived Groth16 request by canonical message id.

        Native responses are preflighted as canonical uncompressed Norito frames bound to
        ``SccpGroth16Bn254ProofRequestV1``. The frame remains opaque, so this lightweight client
        does not independently bind the embedded message id to the request path.
        """

        return self._get_sccp_typed_object(
            f"/v1/sccp/proof-requests/{self._sccp_message_id(message_id)}",
            format=format,
            context="sccp proof request",
            normalize=normalize_sccp_proof_request,
            maximum_norito_body_bytes=_SCCP_DESTINATION_NORITO_RESPONSE_MAX_BYTES,
            expected_norito_type_name=_SCCP_PROOF_REQUEST_NORITO_TYPE_NAME,
        )

    def get_sccp_recent_messages(
        self,
        *,
        from_height: Optional[int] = None,
        after_index: Optional[int] = None,
        limit: Optional[int] = None,
    ) -> SccpRecentMessages:
        """Fetch newest-first SCCP messages (`GET /v1/sccp/messages/recent`)."""

        params_dict: Dict[str, str] = {}
        if from_height is not None:
            if (
                isinstance(from_height, bool)
                or not isinstance(from_height, int)
                or not 1 <= from_height <= 0xFFFF_FFFF_FFFF_FFFF
            ):
                raise ValueError("SCCP recent-message from_height must be a positive u64")
            params_dict["from"] = str(from_height)
        if after_index is not None:
            if from_height is None:
                raise ValueError(
                    "SCCP recent-message after_index requires the paired from_height"
                )
            if (
                isinstance(after_index, bool)
                or not isinstance(after_index, int)
                or not 0 <= after_index <= 511
            ):
                raise ValueError("SCCP recent-message after_index must be an integer in 0..511")
            params_dict["after_index"] = str(after_index)
        if limit is not None:
            if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= 50:
                raise ValueError("SCCP recent-message limit must be an integer in 1..50")
            params_dict["limit"] = str(limit)
        payload = self._get_sccp_json_object(
            "/v1/sccp/messages/recent",
            context="sccp recent messages",
            params=params_dict or None,
            maximum_body_bytes=_SCCP_RECENT_RESPONSE_MAX_BYTES,
        )
        return normalize_sccp_recent_messages(payload)

    def submit_bridge_proof(
        self,
        *,
        authority: str,
        destination_proof_b64: str,
        fee_payment: Mapping[str, Any],
        signature_b64: Optional[str] = None,
        transaction_payload_b64: Optional[str] = None,
        creation_time_ms: Optional[int] = None,
    ) -> SccpBridgeSubmitResponse:
        """Prepare or submit one exact SORA-origin proof.

        Signed submission requires the byte-identical prepared transaction payload, its detached
        signature, and the preparation response's creation timestamp.
        """

        candidate: Dict[str, Any] = {
            "authority": authority,
            "fee_payment": fee_payment,
            "destination_proof_b64": destination_proof_b64,
        }
        for key, value in (
            ("signature_b64", signature_b64),
            ("transaction_payload_b64", transaction_payload_b64),
            ("creation_time_ms", creation_time_ms),
        ):
            if value is not None:
                candidate[key] = value
        payload = normalize_bridge_proof_submit_payload(candidate)
        return self._submit_sccp_bridge(
            "/v1/bridge/proofs/submit",
            payload,
            context="bridge proof submit",
        )

    def submit_bridge_message(
        self,
        *,
        authority: str,
        native_proof_b64: str,
        fee_payment: Mapping[str, Any],
        signature_b64: Optional[str] = None,
        transaction_payload_b64: Optional[str] = None,
        creation_time_ms: Optional[int] = None,
    ) -> SccpBridgeSubmitResponse:
        """Prepare or submit one exact native inbound proof.

        Signed submission requires the byte-identical prepared transaction payload, its detached
        signature, and the preparation response's creation timestamp.
        """

        candidate: Dict[str, Any] = {
            "authority": authority,
            "fee_payment": fee_payment,
            "native_proof_b64": native_proof_b64,
        }
        for key, value in (
            ("signature_b64", signature_b64),
            ("transaction_payload_b64", transaction_payload_b64),
            ("creation_time_ms", creation_time_ms),
        ):
            if value is not None:
                candidate[key] = value
        payload = normalize_bridge_message_submit_payload(candidate)
        return self._submit_sccp_bridge(
            "/v1/bridge/messages",
            payload,
            context="bridge message submit",
        )

    @staticmethod
    def _sccp_message_id(value: Any) -> str:
        if (
            not isinstance(value, str)
            or re.fullmatch(r"[0-9a-f]{64}", value) is None
            or set(value) == {"0"}
        ):
            raise ValueError("SCCP message id must be canonical lowercase nonzero 32-byte hex")
        return value

    def _get_sccp_json_object(
        self,
        path: str,
        *,
        context: str,
        params: Optional[Mapping[str, str]] = None,
        maximum_body_bytes: int,
        parser: Callable[[bytes, str], Mapping[str, Any]] = parse_sccp_json_object,
    ) -> Mapping[str, Any]:
        response = self._request(
            "GET",
            path,
            params=params,
            headers={"Accept": "application/json"},
            stream=True,
        )
        self._expect_status(
            response,
            {200},
            maximum_body_bytes=maximum_body_bytes,
            context=context,
        )
        content_type = response.headers.get("Content-Type", "")
        if re.fullmatch(r"application/json(?:\s*;.*)?", content_type, re.IGNORECASE) is None:
            response.close()
            raise TypeError(f"{context} response must use application/json content type")
        body = _read_bounded_sccp_response_body(response, maximum_body_bytes, context)
        return parser(body, context)

    def _get_sumeragi_operator_json_object(
        self,
        path: str,
        *,
        context: str,
        params: Optional[Mapping[str, Any]] = None,
        maximum_body_bytes: int,
        parser: Callable[[bytes, str], Mapping[str, Any]] = parse_sumeragi_json_object,
    ) -> Mapping[str, Any]:
        query = urlencode(sorted(params.items()), doseq=True) if params else ""
        target = f"{path}?{query}" if query else path
        response = self._operator_get(target, stream=True)
        self._expect_status(
            response,
            {200},
            maximum_body_bytes=maximum_body_bytes,
            context=context,
        )
        content_type = response.headers.get("Content-Type", "")
        if re.fullmatch(
            r"application/json(?:\s*;.*)?",
            content_type,
            re.IGNORECASE,
        ) is None:
            response.close()
            raise TypeError(f"{context} response must use application/json content type")
        body = _read_bounded_sccp_response_body(
            response,
            maximum_body_bytes,
            context,
        )
        return parser(body, context)

    def _get_sccp_typed_object(
        self,
        path: str,
        *,
        format: str,
        context: str,
        normalize: Callable[[Any], Mapping[str, Any]],
        maximum_norito_body_bytes: int,
        expected_norito_type_name: str,
    ) -> Union[Mapping[str, Any], bytes]:
        if format not in {"json", "norito"}:
            raise ValueError("SCCP response format must be exactly `json` or `norito`")
        accept = "application/x-norito" if format == "norito" else "application/json"
        maximum_body_bytes = (
            maximum_norito_body_bytes
            if format == "norito"
            else _SCCP_JSON_RESPONSE_MAX_BYTES
        )
        response = self._request("GET", path, headers={"Accept": accept}, stream=True)
        self._expect_status(
            response,
            {200},
            maximum_body_bytes=maximum_body_bytes,
            context=context,
        )
        content_type = response.headers.get("Content-Type", "")
        if format == "norito":
            if re.fullmatch(r"application/x-norito(?:\s*;.*)?", content_type, re.IGNORECASE) is None:
                response.close()
                raise TypeError(f"{context} response must use application/x-norito content type")
            body = _read_bounded_sccp_response_body(
                response, maximum_body_bytes, context
            )
            validate_norito_frame(
                body,
                context=f"{context} response",
                expected_type_name=expected_norito_type_name,
                expected_padding_length=0,
            )
            return body
        if re.fullmatch(r"application/json(?:\s*;.*)?", content_type, re.IGNORECASE) is None:
            response.close()
            raise TypeError(f"{context} response must use application/json content type")
        body = _read_bounded_sccp_response_body(response, maximum_body_bytes, context)
        return normalize(parse_sccp_json_object(body, context))

    def _submit_sccp_bridge(
        self,
        path: str,
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> SccpBridgeSubmitResponse:
        headers = {"Content-Type": "application/json", "Accept": "application/json"}
        data = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        response = self._request(
            "POST", path, headers=headers, data=data, stream=True
        )
        self._expect_status(
            response,
            {200},
            maximum_body_bytes=_SCCP_SUBMIT_RESPONSE_MAX_BYTES,
            context=context,
        )
        content_type = response.headers.get("Content-Type", "")
        if re.fullmatch(r"application/json(?:\s*;.*)?", content_type, re.IGNORECASE) is None:
            response.close()
            raise TypeError(f"{context} response must use application/json content type")
        expectations: Dict[str, Any] = {"submitted": "signature_b64" in payload}
        if "creation_time_ms" in payload:
            expectations["creation_time_ms"] = payload["creation_time_ms"]
        body = _read_bounded_sccp_response_body(
            response, _SCCP_SUBMIT_RESPONSE_MAX_BYTES, context
        )
        return parse_sccp_bridge_submit_response_json(body, expectations)

    def get_runtime_abi_hash(self) -> RuntimeAbiHash:
        """Fetch the canonical ABI hash (`GET /v1/runtime/abi/hash`)."""

        payload = self._get_json_object(
            "/v1/runtime/abi/hash",
            context="runtime abi hash response",
        )
        return self._parse_runtime_abi_hash(payload, context="runtime abi hash response")

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

    def get_pipeline_preflight(self) -> PipelinePreflight:
        """Fetch operator-authenticated node-local pipeline preflight diagnostics."""

        response = self._operator_get("/v1/pipeline/preflight")
        self._expect_status(response, {200})
        payload = self._ensure_mapping(response.json(), "pipeline preflight")
        return self._parse_pipeline_preflight(payload, context="pipeline preflight")

    # ------------------------------------------------------------------
    # Sora VPN native lease helpers
    # ------------------------------------------------------------------
    def get_vpn_profile(self) -> VpnProfile:
        """Fetch native Sora VPN profile and XOR lease parameters."""

        payload = self._vpn_json_request(
            "GET",
            "/v1/vpn/profile",
            context="vpn profile",
        )
        return self._parse_vpn_profile(payload, context="vpn profile")

    def create_vpn_quote(
        self,
        request: Union[VpnQuoteCreateRequest, Mapping[str, Any]],
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> VpnQuote:
        """Create a VPN quote carrying the native `OpenVpnLeaseEscrow` instruction."""

        canonical_auth = self._require_canonical_auth(canonical_auth, "vpn quote")
        payload = self._normalize_vpn_quote_request(request)
        response = self._vpn_json_request(
            "POST",
            "/v1/vpn/quotes",
            body_payload=payload,
            canonical_auth=canonical_auth,
            headers=headers,
            context="vpn quote",
            expected_status=(201,),
        )
        return self._parse_vpn_quote(response, context="vpn quote")

    def create_vpn_session(
        self,
        request: Union[VpnSessionCreateRequest, Mapping[str, Any]],
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> VpnSession:
        """Open a VPN session from a paid quote and matching metering key."""

        canonical_auth = self._require_canonical_auth(canonical_auth, "vpn session")
        payload = self._normalize_vpn_session_request(request)
        response = self._vpn_json_request(
            "POST",
            "/v1/vpn/sessions",
            body_payload=payload,
            canonical_auth=canonical_auth,
            headers=headers,
            context="vpn session",
            expected_status=(201,),
        )
        return self._parse_vpn_session(response, context="vpn session")

    def get_vpn_session(
        self,
        session_id: Union[str, bytes, bytearray, memoryview],
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Optional[VpnSession]:
        """Fetch an active VPN session, returning `None` when absent."""

        canonical_auth = self._require_canonical_auth(canonical_auth, "vpn session")
        normalized = self._normalize_hex_string(
            session_id,
            context="vpn session_id",
            expected_length=64,
        )
        path = f"/v1/vpn/sessions/{quote(normalized, safe='')}"
        response = self._vpn_json_request(
            "GET",
            path,
            canonical_auth=canonical_auth,
            headers=headers,
            context="vpn session",
            expected_status=(200, 404),
        )
        if response is None:
            return None
        return self._parse_vpn_session(response, context="vpn session")

    def delete_vpn_session(
        self,
        session_id: Union[str, bytes, bytearray, memoryview],
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Optional[VpnReceipt]:
        """Disconnect a VPN session, returning the canonical lease receipt when present."""

        canonical_auth = self._require_canonical_auth(canonical_auth, "vpn receipt")
        normalized = self._normalize_hex_string(
            session_id,
            context="vpn session_id",
            expected_length=64,
        )
        path = f"/v1/vpn/sessions/{quote(normalized, safe='')}"
        response = self._vpn_json_request(
            "DELETE",
            path,
            canonical_auth=canonical_auth,
            headers=headers,
            context="vpn receipt",
            expected_status=(200, 404),
        )
        if response is None:
            return None
        return self._parse_vpn_receipt(response, context="vpn receipt")

    def submit_vpn_receipt(
        self,
        request: Union[VpnReceiptSubmitRequest, Mapping[str, Any]],
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> VpnReceipt:
        """Submit a relay receipt and receive the native `SettleVpnLease` instruction."""

        canonical_auth = self._require_canonical_auth(canonical_auth, "vpn receipt")
        payload = self._normalize_vpn_receipt_request(request)
        response = self._vpn_json_request(
            "POST",
            "/v1/vpn/receipts",
            body_payload=payload,
            canonical_auth=canonical_auth,
            headers=headers,
            context="vpn receipt",
            expected_status=(201,),
        )
        return self._parse_vpn_receipt(response, context="vpn receipt")

    def list_vpn_receipts(
        self,
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> VpnReceiptListResponse:
        """List recent disconnected or settled VPN lease receipts for the signed account."""

        canonical_auth = self._require_canonical_auth(canonical_auth, "vpn receipts")
        response = self._vpn_json_request(
            "GET",
            "/v1/vpn/receipts",
            canonical_auth=canonical_auth,
            headers=headers,
            context="vpn receipts",
        )
        return self._parse_vpn_receipt_list(response, context="vpn receipts")

    # ------------------------------------------------------------------
    # Connect helpers
    # ------------------------------------------------------------------
    def get_connect_status(self) -> ConnectStatusSnapshot:
        """Fetch the operator-authenticated Connect aggregate snapshot."""

        response = self._operator_get("/v1/connect/status/aggregate")
        self._expect_status(response, {200})
        return self._parse_connect_status(
            response.json(),
            context="connect aggregate status",
        )

    def create_connect_session(self, payload: Mapping[str, Any]) -> ConnectSessionInfo:
        """Create a Connect session (`POST /v1/connect/session`)."""

        request = _connect_session.normalize_connect_session_request(
            payload,
            hash_literal=_offline_hash_literal,
        )
        body = self._post_json(
            "/v1/connect/session",
            request,
            context="connect session",
        )
        session = self._parse_connect_session(body, context="connect session")
        _connect_session.ensure_connect_session_matches_request(session, request)
        return session

    def delete_connect_session(self, sid: str, token_management: str) -> bool:
        """Delete a Connect session (`DELETE /v1/connect/session/{sid}`).
        Both credentials must be canonical 32-byte unpadded base64url values.
        """

        sid = _connect_session.canonical_connect_sid(sid)
        token_management = _connect_session.canonical_connect_token(
            token_management, "Connect management token"
        )
        response = self._request(
            "DELETE",
            f"/v1/connect/session/{sid}",
            headers={"Authorization": f"Bearer {token_management}"},
        )
        self._expect_status(response, {204, 404})
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
        plan_id: str,
        plan: Mapping[str, Any],
        canonical_auth: ToriiCanonicalRequestAuth,
    ) -> SubscriptionPlanCreateResult:
        """Prepare a locally signed subscription-plan transaction draft."""
        normalized_plan_id = self._require_non_empty_string(
            plan_id,
            "subscription plan plan_id",
        )
        payload = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription plan authority",
            ),
            "plan_id": normalized_plan_id,
            "plan": self._clone_json_payload(plan, context="subscription plan"),
        }
        body = signed_subscription_post(
            self, "/v1/subscriptions/plans", payload,
            canonical_auth=canonical_auth,
            context="subscription plan create response",
        )
        result = SubscriptionPlanCreateResult.from_payload(body)
        if result.plan_id != normalized_plan_id:
            raise RuntimeError(
                "subscription plan create response plan_id does not match the request"
            )
        return result

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
            params["status"] = normalize_subscription_status(
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
        subscription_id: str,
        plan_id: str,
        billing_trigger_id: Optional[str] = None,
        usage_trigger_id: Optional[str] = None,
        first_charge_ms: Optional[int] = None,
        grant_usage_to_provider: Optional[bool] = None,
        canonical_auth: ToriiCanonicalRequestAuth,
    ) -> SubscriptionCreateResult:
        """Prepare an authority-bound subscription creation draft."""
        payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription authority",
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
        body = signed_subscription_post(
            self, "/v1/subscriptions", payload,
            canonical_auth=canonical_auth,
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
        canonical_auth: ToriiCanonicalRequestAuth,
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
        }
        body = signed_subscription_post(
            self, f"/v1/subscriptions/{encoded_id}/pause", payload,
            canonical_auth=canonical_auth,
            context="subscription pause response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def resume_subscription(
        self,
        subscription_id: str,
        *,
        authority: str,
        charge_at_ms: Optional[int] = None,
        canonical_auth: ToriiCanonicalRequestAuth,
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
        }
        charge_value = self._normalize_optional_int(
            charge_at_ms,
            "subscription resume charge_at_ms",
            allow_zero=True,
        )
        if charge_value is not None:
            payload["charge_at_ms"] = charge_value
        body = signed_subscription_post(
            self, f"/v1/subscriptions/{encoded_id}/resume", payload,
            canonical_auth=canonical_auth,
            context="subscription resume response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def cancel_subscription(
        self,
        subscription_id: str,
        *,
        authority: str,
        cancel_mode: str,
        canonical_auth: ToriiCanonicalRequestAuth,
    ) -> SubscriptionActionResult:
        """Cancel a subscription (`POST /v1/subscriptions/{subscription_id}/cancel`)."""
        normalized_id = self._require_non_empty_string(
            subscription_id,
            "subscription_id",
        )
        encoded_id = quote(normalized_id, safe="")
        normalized_mode = self._require_non_empty_string(
            cancel_mode,
            "subscription cancel_mode",
        ).lower()
        if normalized_mode not in {"immediate", "period_end"}:
            raise ValueError(
                "subscription cancel_mode must be immediate or period_end"
            )
        payload = {
            "authority": self._require_non_empty_string(
                authority,
                "subscription cancel authority",
            ),
            "cancel_mode": {"mode": normalized_mode, "value": None},
        }
        body = signed_subscription_post(
            self, f"/v1/subscriptions/{encoded_id}/cancel", payload,
            canonical_auth=canonical_auth,
            context="subscription cancel response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def keep_subscription(
        self,
        subscription_id: str,
        *,
        authority: str,
        canonical_auth: ToriiCanonicalRequestAuth,
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
        }
        body = signed_subscription_post(
            self, f"/v1/subscriptions/{encoded_id}/keep", payload,
            canonical_auth=canonical_auth,
            context="subscription keep response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def charge_subscription_now(
        self,
        subscription_id: str,
        *,
        authority: str,
        charge_at_ms: Optional[int] = None,
        canonical_auth: ToriiCanonicalRequestAuth,
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
        }
        charge_value = self._normalize_optional_int(
            charge_at_ms,
            "subscription charge_at_ms",
            allow_zero=True,
        )
        if charge_value is not None:
            payload["charge_at_ms"] = charge_value
        body = signed_subscription_post(
            self, f"/v1/subscriptions/{encoded_id}/charge-now", payload,
            canonical_auth=canonical_auth,
            context="subscription charge-now response",
            expected_status=(200,),
        )
        return SubscriptionActionResult.from_payload(body)

    def record_subscription_usage(
        self,
        subscription_id: str,
        *,
        authority: str,
        unit_key: str,
        delta: str,
        usage_trigger_id: Optional[str] = None,
        canonical_auth: ToriiCanonicalRequestAuth,
    ) -> SubscriptionUsageDraft:
        """Prepare a locally signed usage-recording transaction draft."""
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
            "unit_key": self._require_non_empty_string(
                unit_key,
                "subscription usage unit_key",
            ),
            "delta": _canonical_quantity(
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
        body = signed_subscription_post(
            self, f"/v1/subscriptions/{encoded_id}/usage", payload,
            canonical_auth=canonical_auth,
            context="subscription usage response",
            expected_status=(200,),
        )
        result = SubscriptionUsageDraft.from_payload(body)
        if result.subscription_id != normalized_id:
            raise RuntimeError(
                "subscription usage response subscription_id does not match the request"
            )
        return result

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
            params["asset_id"] = _require_exact_non_empty_string(
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

    # ------------------------------------------------------------------
    # First-release Kagemusha API
    # ------------------------------------------------------------------
    def get_offline_capability(self) -> OfflineStatus:
        """Fetch the universally compiled, asset-neutral offline capability."""

        response = self._request(
            "GET",
            _OFFLINE_READINESS_PATH,
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._offline_json_response(response, "offline capability response")
        return OfflineStatus.from_payload(payload)

    def submit_kagemusha_top_up(
        self, request: KagemushaTopUpRequestV4
    ) -> OfflineOperationReference:
        """Submit one canonical typed Norito Kagemusha top-up request."""

        if not isinstance(request, KagemushaTopUpRequestV4):
            raise TypeError("request must be KagemushaTopUpRequestV4")
        return self._submit_kagemusha_command(
            _OFFLINE_TOP_UP_PATH,
            "top_up",
            request.norito,
            request.operation_id,
        )

    def submit_kagemusha_redeem(
        self, request: KagemushaRedeemRequestV4
    ) -> OfflineOperationReference:
        """Submit one canonical typed Norito Kagemusha redemption request."""

        if not isinstance(request, KagemushaRedeemRequestV4):
            raise TypeError("request must be KagemushaRedeemRequestV4")
        return self._submit_kagemusha_command(
            _OFFLINE_REDEEM_PATH,
            "redeem",
            request.norito,
            request.operation_id,
        )

    def get_kagemusha_operation_status(
        self, operation_id: str
    ) -> OfflineOperationStatus:
        """Fetch the typed state of one Kagemusha operation."""

        canonical_id = _require_offline_operation_id(operation_id)
        response = self._request(
            "GET",
            f"{_OFFLINE_OPERATIONS_PATH}/{canonical_id}",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._offline_json_response(response, "offline operation status response")
        return _offline_operation_status(payload, canonical_id)

    def _submit_kagemusha_command(
        self,
        path: str,
        kind: Literal["top_up", "redeem"],
        body: bytes,
        operation_id: str,
    ) -> OfflineOperationReference:
        response = self._request(
            "POST",
            path,
            headers={
                "Accept": "application/json",
                "Content-Type": "application/x-norito",
                "Idempotency-Key": operation_id,
            },
            data=body,
        )
        self._expect_status(response, {202})
        payload = self._offline_json_response(response, "offline operation reference response")
        return _offline_operation_reference(
            payload,
            expected_operation_id=operation_id,
            expected_kind=kind,
            location=response.headers.get("Location"),
        )

    @staticmethod
    def _offline_json_response(
        response: requests.Response, context: str
    ) -> Mapping[str, Any]:
        content_type = response.headers.get("Content-Type", "")
        media_type = content_type.split(";", 1)[0].strip().lower()
        if media_type != "application/json":
            raise RuntimeError(f"{context} must use Content-Type application/json")
        body = response.content
        if len(body) > _OFFLINE_MAX_JSON_RESPONSE_BYTES:
            raise RuntimeError(
                f"{context} exceeds {_OFFLINE_MAX_JSON_RESPONSE_BYTES} bytes"
            )
        try:
            text = body.decode("utf-8")
        except UnicodeDecodeError as error:
            raise RuntimeError(f"{context} must be valid UTF-8 JSON") from error
        try:
            payload = json.loads(
                text,
                object_pairs_hook=_offline_json_object_without_duplicates,
                parse_float=Decimal,
                parse_constant=_offline_reject_json_constant,
            )
        except (ValueError, RecursionError) as error:
            raise RuntimeError(f"{context} contains invalid JSON: {error}") from error
        payload = _snapshot_offline_json(payload, context)
        if not isinstance(payload, Mapping):
            raise RuntimeError(f"{context} must be a JSON object")
        return payload

    # ------------------------------------------------------------------
    # Sumeragi telemetry
    # ------------------------------------------------------------------
    def get_sumeragi_status(self) -> SumeragiV2Status:
        """Fetch and validate authoritative v2 consensus status.

        This method is intentionally separate from :meth:`get_status_snapshot`:
        the general status route is operational telemetry, while this route is
        the fail-closed reducer projection.
        """
        payload = self._get_sumeragi_operator_json_object(
            "/v1/sumeragi/status",
            context="sumeragi status",
            maximum_body_bytes=1 * 1024 * 1024,
            parser=parse_sumeragi_json_object,
        )
        return _SumeragiV2StatusParser.parse(payload)
    def get_sumeragi_diagnostics(self) -> SumeragiDiagnosticsStatus:
        """Fetch and validate bounded operator and lane diagnostics."""
        payload = self._get_sumeragi_operator_json_object(
            "/v1/sumeragi/diagnostics",
            context="sumeragi diagnostics",
            maximum_body_bytes=16 * 1024 * 1024,
            parser=parse_sumeragi_json_object,
        )
        return _SumeragiDiagnosticsParser.parse(payload)
    def get_sumeragi_qc(self) -> SumeragiQcSnapshot:
        """Fetch HighestQC/LockedQC snapshot (`GET /v1/sumeragi/qc`)."""

        payload = self._ensure_mapping(
            self._operator_get("/v1/sumeragi/qc").json(),
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
            self._operator_get("/v1/sumeragi/leader").json(),
            "sumeragi leader",
        )
        leader_index = self._coerce_unsigned(payload.get("leader_index"), "sumeragi leader.leader_index")
        prf = self._parse_sumeragi_prf(payload.get("prf"), context="sumeragi leader.prf")
        return SumeragiLeaderSnapshot(leader_index=leader_index, prf=prf)

    def get_sumeragi_params(self) -> SumeragiParamsSnapshot:
        """Fetch on-chain Sumeragi parameters (`GET /v1/sumeragi/params`)."""

        payload = self._ensure_mapping(
            self._operator_get("/v1/sumeragi/params").json(),
            "sumeragi params",
        )
        return self._parse_sumeragi_params(payload, context="sumeragi params")

    def get_sumeragi_bls_keys(self) -> Dict[str, Optional[str]]:
        """Return mapping of network keys to optional BLS public keys (`GET /v1/sumeragi/bls-keys`)."""

        payload = self._operator_get("/v1/sumeragi/bls-keys").json()
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
        response = self._operator_get(
            "/v1/sumeragi/evidence",
            params=params or None,
        )
        self._expect_status(response, {200})
        payload = self._ensure_mapping(
            response.json(),
            "sumeragi evidence listing",
        )
        return self._parse_sumeragi_evidence_page(payload, context="sumeragi evidence listing")

    def get_sumeragi_evidence_count(self) -> int:
        """Return number of evidence entries observed by the node (`GET /v1/sumeragi/evidence/count`)."""

        payload = self._ensure_mapping(
            self._operator_get("/v1/sumeragi/evidence/count").json(),
            "sumeragi evidence count",
        )
        return self._coerce_unsigned(payload.get("count"), "sumeragi evidence count.count")

    # ------------------------------------------------------------------
    # Contract, governance, and council helpers
    # ------------------------------------------------------------------

    def quote_fees(
        self,
        unsigned_payload: Mapping[str, Any],
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
    ) -> Dict[str, Any]:
        """Quote the exact unsigned transaction via ``POST /v1/fees/quote``.

        The returned ``intent`` may replace only the draft's fee maxima before
        the exact payload is signed. It must not change the selected payer,
        sponsor-program revision, or executable gas bound.
        """

        if not isinstance(unsigned_payload, Mapping):
            raise TypeError("quote_fees.unsigned_payload must be an object")
        authority = self._require_exact_i105_account_id(
            unsigned_payload.get("authority"),
            "quote_fees.unsigned_payload.authority",
        )
        canonical_auth = self._require_canonical_auth(
            canonical_auth,
            "quote_fees",
        )
        if canonical_auth.account_id != authority:
            raise ValueError(
                "quote_fees.canonical_auth.account_id must equal the exact payload authority"
            )
        requested_intent = self._normalize_fee_payment_intent(
            unsigned_payload.get("fee_payment"),
            context="quote_fees.unsigned_payload.fee_payment",
        )
        payload = self._vpn_json_request(
            "POST",
            "/v1/fees/quote",
            body_payload={
                "payload": self._clone_json_value(
                    unsigned_payload,
                    context="quote_fees.unsigned_payload",
                )
            },
            canonical_auth=canonical_auth,
            context="fee quote response",
            expected_status=(200,),
        )
        if payload is None:
            raise RuntimeError("fee quote endpoint returned no payload")
        response = dict(self._ensure_mapping(payload, "fee quote response"))
        try:
            quoted_intent = self._normalize_fee_payment_intent(
                response.get("intent"),
                context="fee quote response.intent",
            )
        except (TypeError, ValueError) as exc:
            raise RuntimeError("fee quote response.intent is not canonical") from exc
        requested_value = requested_intent["value"]
        quoted_value = quoted_intent["value"]
        same_selection = (
            requested_intent["payer"] == quoted_intent["payer"]
            and requested_value["gas_limit"] == quoted_value["gas_limit"]
        )
        if same_selection and requested_intent["payer"] == "sponsor":
            same_selection = (
                requested_value["program_id"] == quoted_value["program_id"]
                and requested_value["program_revision"]
                == quoted_value["program_revision"]
            )
        if not same_selection:
            raise RuntimeError(
                "fee quote response changed the requested payer, sponsor revision, or gas bound"
            )
        return response

    def get_fee_sponsor_program(
        self,
        program_id: str,
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
    ) -> Dict[str, Any]:
        """Fetch one exact on-chain sponsor program by canonical identifier."""

        literal = self._require_non_empty_string(
            program_id,
            "get_fee_sponsor_program.program_id",
        )
        sponsor, separator, name = literal.partition("/")
        if (
            literal != program_id
            or separator != "/"
            or not name
            or "/" in name
            or any(char.isspace() for char in name)
        ):
            raise ValueError(
                "get_fee_sponsor_program.program_id must be an exact sponsor/program literal"
            )
        self._require_exact_i105_account_id(
            sponsor,
            "get_fee_sponsor_program.program_id.sponsor",
        )
        payload = self._vpn_json_request(
            "POST",
            "/v1/fee-sponsor-programs/by-id",
            body_payload={"program_id": literal},
            canonical_auth=self._require_canonical_auth(
                canonical_auth,
                "get_fee_sponsor_program",
            ),
            context="fee sponsor program response",
            expected_status=(200,),
        )
        if payload is None:
            raise RuntimeError("fee sponsor program endpoint returned no payload")
        response = dict(self._ensure_mapping(payload, "fee sponsor program response"))
        response_id = response.get("id")
        if not isinstance(response_id, Mapping) or set(response_id) != {"sponsor", "name"}:
            raise RuntimeError(
                "fee sponsor program response.id must contain only sponsor and name"
            )
        try:
            response_sponsor = self._require_exact_i105_account_id(
                response_id.get("sponsor"),
                "fee sponsor program response.id.sponsor",
            )
            response_name = _require_exact_non_empty_string(
                response_id.get("name"),
                "fee sponsor program response.id.name",
            )
        except (TypeError, ValueError) as exc:
            raise RuntimeError("fee sponsor program response.id is not canonical") from exc
        if response_sponsor != sponsor or response_name != name:
            raise RuntimeError(
                "fee sponsor program response.id does not match the requested program"
            )
        try:
            self._require_exact_i105_account_id(
                response.get("payout_account"),
                "fee sponsor program response.payout_account",
            )
        except (TypeError, ValueError) as exc:
            raise RuntimeError(
                "fee sponsor program response.payout_account is not canonical"
            ) from exc
        return response

    def prepare_contract_call(
        self,
        *,
        authority: str,
        fee_payment: Mapping[str, Any],
        entrypoint: str,
        contract_address: Optional[str] = None,
        contract_alias: Optional[str] = None,
        payload: Any = None,
    ) -> ContractCallResponse:
        """Prepare a contract-call scaffold for local detached signing."""

        request_payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "prepare_contract_call.authority",
            ),
        }
        request_payload.update(
            self._normalize_contract_selector(
                contract_address=contract_address,
                contract_alias=contract_alias,
                context="prepare_contract_call",
            )
        )
        request_payload["entrypoint"] = self._require_non_empty_string(
            entrypoint,
            "prepare_contract_call.entrypoint",
        )
        if payload is not None:
            request_payload["payload"] = self._clone_json_value(
                payload,
                context="prepare_contract_call.payload",
            )
        normalized_fee_payment = self._normalize_fee_payment_intent(
            fee_payment,
            context="prepare_contract_call.fee_payment",
            require_gas_limit=True,
        )
        request_payload["fee_payment"] = normalized_fee_payment
        response = self._request(
            "POST",
            "/v1/contracts/call",
            headers={
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
            data=json.dumps(request_payload).encode("utf-8"),
        )
        self._expect_status(response, {200})
        body = self._maybe_json(response)
        if body is None:
            raise RuntimeError("contract call endpoint returned no payload")
        record = self._ensure_mapping(body, "contract call response")
        result = self._parse_contract_call_response(
            record,
            context="contract call response",
        )
        self._validate_contract_call_draft(
            result,
            entrypoint=request_payload["entrypoint"],
            contract_address=request_payload.get("contract_address"),
            contract_alias=request_payload.get("contract_alias"),
        )
        return result

    def propose_multisig(
        self,
        *,
        multisig_account_id: Optional[str] = None,
        multisig_account_alias: Optional[str] = None,
        signer_account_id: str,
        instructions: Sequence[Any],
        fee_payment: Mapping[str, Any],
        public_key_hex: Optional[str] = None,
        signature_b64: Optional[str] = None,
        creation_time_ms: Optional[int] = None,
    ) -> MultisigResponse:
        """Propose a generic multisig instruction batch via ``POST /v1/multisig/propose``.

        Each instruction may be raw native Norito ``InstructionBox`` bytes or an already-base64
        encoded string carrying those bytes.
        """

        has_account_id = multisig_account_id is not None
        has_alias = multisig_account_alias is not None
        if has_account_id == has_alias:
            raise ValueError(
                "propose_multisig requires exactly one of multisig_account_id or multisig_account_alias"
            )
        if isinstance(instructions, (str, bytes, bytearray, memoryview)):
            raise TypeError("propose_multisig.instructions must be a sequence of instruction payloads")
        try:
            instruction_values = list(instructions)
        except TypeError as exc:
            raise TypeError(
                "propose_multisig.instructions must be a sequence of instruction payloads"
            ) from exc
        if not instruction_values:
            raise ValueError("propose_multisig.instructions must not be empty")

        request_payload: Dict[str, Any] = {
            "signer_account_id": self._normalize_canonical_account_id(
                signer_account_id,
                "propose_multisig.signer_account_id",
            ),
            "instructions": [
                self.multisig_instruction_b64(
                    value,
                    context=f"propose_multisig.instructions[{index}]",
                )
                for index, value in enumerate(instruction_values)
            ],
        }
        normalized_fee_payment = self._normalize_fee_payment_intent(
            fee_payment,
            context="propose_multisig.fee_payment",
        )
        request_payload["fee_payment"] = normalized_fee_payment
        if has_account_id:
            request_payload["multisig_account_id"] = self._normalize_canonical_account_id(
                multisig_account_id,
                "propose_multisig.multisig_account_id",
            )
        else:
            request_payload["multisig_account_alias"] = self._require_non_empty_string(
                multisig_account_alias,
                "propose_multisig.multisig_account_alias",
            )
        if public_key_hex is not None:
            request_payload["public_key_hex"] = self._normalize_hex_string(
                public_key_hex,
                context="propose_multisig.public_key_hex",
                expected_length=64,
            )
        if signature_b64 is not None:
            request_payload["signature_b64"] = self._normalize_required_exact_base64_payload(
                signature_b64,
                "propose_multisig.signature_b64",
            )
        normalized_creation_time = self._normalize_optional_int(
            creation_time_ms,
            "propose_multisig.creation_time_ms",
            allow_zero=True,
        )
        if normalized_creation_time is not None:
            request_payload["creation_time_ms"] = normalized_creation_time
        body = self._post_json(
            "/v1/multisig/propose",
            request_payload,
            context="multisig propose response",
        )
        return self._parse_multisig_response(
            body,
            context="multisig propose response",
        )

    def get_governance_contract(
        self,
        contract_address: str,
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
    ) -> GovernanceContractResponse:
        """Fetch one governed contract binding via ``GET /v1/gov/contracts/{contract_address}``."""

        normalized_address = self._require_non_empty_string(
            contract_address,
            "governance contract contract_address",
        )
        payload = self._account_json_request(
            "GET",
            f"/v1/gov/contracts/{quote(normalized_address, safe='')}",
            canonical_auth=canonical_auth,
            context="governance contract response",
        )
        return self._parse_governance_contract_response(
            payload,
            context="governance contract response",
        )

    def get_governance_proposal(
        self, proposal_id: str, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> GovernanceProposalStatus:
        """Fetch proposal metadata via ``GET /v1/gov/proposals/{id}``."""

        proposal_id = self._require_governance_proposal_id_v1(
            proposal_id,
            context="governance proposal proposal_id",
        )
        payload = self._account_json_request(
            "GET",
            f"/v1/gov/proposals/{quote(proposal_id, safe='')}",
            canonical_auth=canonical_auth,
            context="governance proposal",
        )
        found = bool(payload.get("found"))
        proposal = self._optional_mapping(payload.get("proposal"), context="governance proposal body")
        return GovernanceProposalStatus(found=found, proposal=proposal)

    def get_governance_locks(
        self, referendum_id: str, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> GovernanceLocksOverview:
        """Return lock escrow information for a referendum."""

        referendum_id = self._require_governance_selector_v1(
            referendum_id,
            context="governance locks referendum_id",
        )
        payload = self._account_json_request(
            "GET",
            f"/v1/gov/locks/{quote(referendum_id, safe='')}",
            canonical_auth=canonical_auth,
            context="governance locks",
        )
        rid = str(payload.get("referendum_id") or referendum_id)
        found = bool(payload.get("found"))
        locks_payload = self._optional_mapping(
            payload.get("locks"),
            context="locks payload",
        )
        locks: Optional[Dict[str, GovernanceLockRecord]] = None
        if locks_payload is not None:
            locks = {}
            for account_id, record in locks_payload.items():
                if not isinstance(account_id, str) or not account_id:
                    raise RuntimeError(
                        "governance locks keys must be non-empty account identifiers"
                    )
                locks[account_id] = GovernanceLockRecord.from_payload(
                    record,
                    context=f"governance locks[{account_id!r}]",
                )
        return GovernanceLocksOverview(found=found, referendum_id=rid, locks=locks)

    def get_governance_referendum(
        self, referendum_id: str, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> GovernanceReferendumStatus:
        """Fetch referendum status via ``GET /v1/gov/referenda/{id}``."""

        referendum_id = self._require_governance_selector_v1(
            referendum_id,
            context="governance referendum referendum_id",
        )
        payload = self._account_json_request(
            "GET",
            f"/v1/gov/referenda/{quote(referendum_id, safe='')}",
            canonical_auth=canonical_auth,
            context="governance referendum",
        )
        found = bool(payload.get("found"))
        referendum = self._optional_mapping(payload.get("referendum"), context="referendum payload")
        return GovernanceReferendumStatus(found=found, referendum=referendum)

    def get_governance_tally(
        self, referendum_id: str, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> GovernanceTallySummary:
        """Return the quadratic tally summary for a referendum."""

        referendum_id = self._require_governance_selector_v1(
            referendum_id,
            context="governance tally referendum_id",
        )
        payload = self._account_json_request(
            "GET",
            f"/v1/gov/tally/{quote(referendum_id, safe='')}",
            canonical_auth=canonical_auth,
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

    def get_governance_unlock_stats(
        self, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> GovernanceUnlockStats:
        """Return aggregate unlock metrics (operator view)."""

        payload = self._account_json_request(
            "GET", "/v1/gov/unlocks/stats", canonical_auth=canonical_auth, context="unlock stats"
        )
        return GovernanceUnlockStats(
            height_current=self._coerce_int(payload.get("height_current"), "unlock.height_current"),
            expired_locks_now=self._coerce_int(payload.get("expired_locks_now"), "unlock.expired_locks_now"),
            referenda_with_expired=self._coerce_int(
                payload.get("referenda_with_expired"),
                "unlock.referenda_with_expired",
            ),
            last_sweep_height=self._coerce_int(payload.get("last_sweep_height"), "unlock.last_sweep_height"),
        )

    def get_council_current(
        self, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> CouncilCurrentStatus:
        """Return the latest council roster."""

        payload = self._account_json_request(
            "GET", "/v1/gov/council/current", canonical_auth=canonical_auth, context="council current"
        )
        epoch = self._coerce_int(payload.get("epoch"), "council_current.epoch")
        members_value = payload.get("members", [])
        members = self._parse_council_members(members_value)
        return CouncilCurrentStatus(epoch=epoch, members=members)

    def propose_contract_deploy(
        self,
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        contract_address: Optional[str] = None,
        contract_alias: Optional[str] = None,
        abi_version: str,
        code_hash: str,
        abi_hash: str,
        window: Optional[Tuple[int, int]] = None,
        mode: Optional[Literal["Zk", "Plain"]] = None,
        limits: Optional[Mapping[str, Any]] = None,
    ) -> GovernanceProposalDraft:
        """Draft a deploy-contract proposal via ``POST /v1/gov/proposals/deploy-contract``."""

        if (contract_address is None) == (contract_alias is None):
            raise ValueError(
                "provide exactly one of contract_address or contract_alias",
            )
        payload: Dict[str, Any] = {
            "abi_version": abi_version,
            "code_hash": code_hash,
            "abi_hash": abi_hash,
        }
        if contract_address is not None:
            payload["contract_address"] = contract_address
        if contract_alias is not None:
            payload["contract_alias"] = contract_alias
        if window is not None:
            payload["window"] = {"lower": int(window[0]), "upper": int(window[1])}
        if mode is not None:
            if mode not in ("Zk", "Plain"):
                raise ValueError("mode must be exactly 'Zk' or 'Plain'")
            payload["mode"] = mode
        if limits is not None:
            payload["limits"] = dict(limits)
        body = self._account_json_request(
            "POST",
            "/v1/gov/proposals/deploy-contract",
            body_payload=payload,
            canonical_auth=canonical_auth,
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

        referendum_id = self._require_governance_proposal_id_v1(
            referendum_id,
            context="governance finalize referendum_id",
        )
        proposal_id = self._require_governance_proposal_id_v1(
            proposal_id,
            context="governance finalize proposal_id",
        )
        if referendum_id != proposal_id:
            raise RuntimeError(
                "governance finalize referendum_id must equal proposal_id"
            )
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
        canonical_auth: ToriiCanonicalRequestAuth,
        proposal_id: str,
        preimage_hash: Optional[str] = None,
        window: Optional[Tuple[int, int]] = None,
    ) -> GovernanceInstructionDraft:
        """Draft an EnactReferendum transaction via ``POST /v1/gov/enact``."""

        proposal_id = self._require_governance_proposal_id_v1(
            proposal_id,
            context="governance enact proposal_id",
        )
        payload: Dict[str, Any] = {"proposal_id": proposal_id}
        if preimage_hash:
            payload["preimage_hash"] = preimage_hash
        if window is not None:
            payload["window"] = {"lower": int(window[0]), "upper": int(window[1])}
        body = self._account_json_request(
            "POST",
            "/v1/gov/enact",
            body_payload=payload,
            canonical_auth=canonical_auth,
            context="governance enact",
            expected_status=(200, 202, 204),
        )
        return GovernanceInstructionDraft(
            ok=bool(body.get("ok")),
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

    def get_protected_namespaces(
        self, *, canonical_auth: ToriiCanonicalRequestAuth
    ) -> ProtectedNamespacesStatus:
        """Fetch the current protected namespace setting."""

        body = self._account_json_request(
            "GET",
            "/v1/gov/protected-namespaces",
            canonical_auth=canonical_auth,
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
    @staticmethod
    def _require_canonical_auth(
        canonical_auth: Optional[ToriiCanonicalRequestAuth],
        context: str,
    ) -> ToriiCanonicalRequestAuth:
        if canonical_auth is None:
            raise ValueError(f"{context}.canonical_auth is required")
        if not isinstance(canonical_auth, ToriiCanonicalRequestAuth):
            raise TypeError(f"{context}.canonical_auth must be ToriiCanonicalRequestAuth")
        return canonical_auth

    def _vpn_json_request(
        self,
        method: str,
        path: str,
        *,
        body_payload: Optional[Mapping[str, Any]] = None,
        canonical_auth: Optional[ToriiCanonicalRequestAuth] = None,
        headers: Optional[Mapping[str, str]] = None,
        context: str,
        expected_status: Iterable[int] = (200,),
    ) -> Optional[Mapping[str, Any]]:
        if urlsplit(self._base_url).scheme.lower() != "https":
            raise RuntimeError("Sora VPN requests require an HTTPS Torii base URL")
        data = self._encode_json_body(body_payload) if body_payload is not None else None
        final_headers = self._canonical_request_headers(
            method,
            path,
            data or b"",
            canonical_auth=canonical_auth,
            headers=headers,
            has_body=data is not None,
        )
        response = self._request(
            method,
            path,
            headers=final_headers,
            data=data,
            allow_retry=False,
            allow_redirects=False,
        )
        self._expect_status(response, expected_status)
        payload = self._maybe_json(response)
        if payload is None:
            return None
        return self._ensure_mapping(payload, context)

    @staticmethod
    def _encode_json_body(payload: Mapping[str, Any]) -> bytes:
        return json.dumps(payload, separators=(",", ":"), sort_keys=True).encode("utf-8")

    @staticmethod
    def _canonical_request_headers(
        method: str,
        path: str,
        body: bytes,
        *,
        canonical_auth: Optional[ToriiCanonicalRequestAuth],
        headers: Optional[Mapping[str, str]],
        has_body: bool,
    ) -> Dict[str, str]:
        final_headers: Dict[str, str] = {"Accept": "application/json"}
        if has_body:
            final_headers["Content-Type"] = "application/json"
        if headers:
            has_witness = _validate_forwarded_witness_header(headers)
            if has_witness and canonical_auth is not None:
                raise ValueError("canonical signature and witness authentication are exclusive")
            final_headers.update(dict(headers))
        if canonical_auth is not None:
            # Requests may rewrite a valid escaped path while preparing its URL.
            # Keep signer state attached to the base headers so `_request` signs
            # the PreparedRequest target and sends that same object exactly once.
            return _CanonicalRequestHeaderPlan(final_headers, canonical_auth)
        return final_headers

    @staticmethod
    def _to_payload_mapping(value: Any, *, context: str) -> Mapping[str, Any]:
        if isinstance(value, Mapping):
            return value
        to_payload = getattr(value, "to_payload", None)
        if callable(to_payload):
            payload = to_payload()
            if isinstance(payload, Mapping):
                return payload
        raise TypeError(f"{context} must be a mapping or request dataclass")

    @classmethod
    def _normalize_vpn_quote_request(
        cls,
        request: Union[VpnQuoteCreateRequest, Mapping[str, Any]],
    ) -> Dict[str, Any]:
        record = cls._to_payload_mapping(request, context="vpn quote request")
        cls._reject_unknown_fields(record, _VPN_QUOTE_REQUEST_FIELDS, "vpn quote request")
        exit_class = record.get("exit_class", record.get("exitClass", ""))
        if exit_class is None:
            exit_class = ""
        return {
            "exit_class": cls._require_vpn_enum(
                exit_class,
                _VPN_EXIT_CLASSES,
                "vpn quote exit_class",
            )
            if exit_class
            else "",
            "metering_public_key_hex": cls._normalize_hex_string(
                record.get("metering_public_key_hex", record.get("meteringPublicKeyHex")),
                context="vpn quote metering_public_key_hex",
                expected_length=64,
            ),
        }

    @classmethod
    def _normalize_vpn_session_request(
        cls,
        request: Union[VpnSessionCreateRequest, Mapping[str, Any]],
    ) -> Dict[str, Any]:
        record = cls._to_payload_mapping(request, context="vpn session request")
        cls._reject_unknown_fields(record, _VPN_SESSION_REQUEST_FIELDS, "vpn session request")
        exit_class = record.get("exit_class", record.get("exitClass", ""))
        if exit_class is None:
            exit_class = ""
        return {
            "exit_class": cls._require_vpn_enum(
                exit_class,
                _VPN_EXIT_CLASSES,
                "vpn session exit_class",
            )
            if exit_class
            else "",
            "quote_id": cls._normalize_vpn_canonical_hex_input(
                record.get("quote_id", record.get("quoteId")),
                context="vpn session quote_id",
                expected_length=64,
            ),
            "payment_tx_hash": cls._normalize_hex_string(
                record.get("payment_tx_hash", record.get("paymentTxHash")),
                context="vpn session payment_tx_hash",
                expected_length=64,
            ),
            "metering_public_key_hex": cls._normalize_hex_string(
                record.get("metering_public_key_hex", record.get("meteringPublicKeyHex")),
                context="vpn session metering_public_key_hex",
                expected_length=64,
            ),
        }

    @classmethod
    def _normalize_vpn_receipt_request(
        cls,
        request: Union[VpnReceiptSubmitRequest, Mapping[str, Any]],
    ) -> Dict[str, Any]:
        record = cls._to_payload_mapping(request, context="vpn receipt request")
        cls._reject_unknown_fields(record, _VPN_RECEIPT_REQUEST_FIELDS, "vpn receipt request")
        lease_id = record.get("lease_id_hex", record.get("leaseIdHex"))
        return {
            "relay_receipt_hex": cls._normalize_hex_string(
                record.get("relay_receipt_hex", record.get("relayReceiptHex")),
                context="vpn receipt relay_receipt_hex",
            ),
            "client_voucher_hex": cls._normalize_hex_string(
                record.get("client_voucher_hex", record.get("clientVoucherHex")),
                context="vpn receipt client_voucher_hex",
            ),
            "lease_id_hex": cls._normalize_hex_string(
                lease_id,
                context="vpn receipt lease_id_hex",
                expected_length=64,
            )
            if lease_id
            else "",
        }

    @staticmethod
    def _parse_vpn_tx_instruction(value: Any, *, context: str) -> TransactionInstruction:
        record = ToriiClient._ensure_mapping(value, context)
        ToriiClient._validate_exact_fields(record, _VPN_TX_INSTRUCTION_FIELDS, context)
        wire_id = ToriiClient._require_string(record.get("wire_id"), f"{context}.wire_id")
        payload_hex = ToriiClient._require_exact_lower_even_hex_string(
            record.get("payload_hex"),
            context=f"{context}.payload_hex",
        )
        return TransactionInstruction(wire_id=wire_id, payload_hex=payload_hex)

    @classmethod
    def _parse_optional_vpn_tx_instruction(
        cls,
        value: Any,
        *,
        context: str,
    ) -> Optional[TransactionInstruction]:
        if value is None:
            return None
        return cls._parse_vpn_tx_instruction(value, context=context)

    @staticmethod
    def _parse_vpn_string_list(value: Any, *, context: str) -> List[str]:
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list of strings")
        result: List[str] = []
        for index, entry in enumerate(value):
            if not isinstance(entry, str):
                raise RuntimeError(f"{context}[{index}] must be a string")
            result.append(entry)
        return result

    _parse_vpn_trust_fields = staticmethod(_vpn_parse_trust_fields)

    @classmethod
    def _parse_vpn_profile(cls, payload: Mapping[str, Any], *, context: str) -> VpnProfile:
        record = cls._ensure_mapping(payload, context)
        cls._validate_exact_fields(record, _VPN_PROFILE_RESPONSE_FIELDS, context)
        available = record.get("available")
        if not isinstance(available, bool):
            raise RuntimeError(f"{context}.available must be a boolean")
        trust_fields = cls._parse_vpn_trust_fields(
            record,
            context=context,
            allow_empty=not available,
        )
        if "dns_push_interval_secs" not in record:
            raise RuntimeError(f"{context}.dns_push_interval_secs is required")
        dns_push_interval_secs = cls._require_vpn_uint64(
            record["dns_push_interval_secs"],
            f"{context}.dns_push_interval_secs",
        )
        if dns_push_interval_secs < 30:
            raise RuntimeError(f"{context}.dns_push_interval_secs must be at least 30")
        supported_exit_classes = cls._require_vpn_profile_exit_classes(
            record.get("supported_exit_classes"),
            f"{context}.supported_exit_classes",
        )
        lease_secs = cls._require_vpn_unsigned_range(
            record.get("lease_secs"),
            f"{context}.lease_secs",
            minimum=1,
            maximum=_VPN_LEASE_SECONDS_MAX,
        )
        mtu_bytes = cls._require_vpn_unsigned_constant(
            record.get("mtu_bytes"),
            f"{context}.mtu_bytes",
            1280,
        )
        settlement_grace_secs = cls._require_vpn_unsigned_range(
            record.get("settlement_grace_secs"),
            f"{context}.settlement_grace_secs",
            minimum=1,
        )
        flow_label_bits = cls._require_vpn_unsigned_constant(
            record.get("flow_label_bits"),
            f"{context}.flow_label_bits",
            24,
        )
        padding_budget_ms = cls._require_vpn_unsigned_range(
            record.get("padding_budget_ms"),
            f"{context}.padding_budget_ms",
            minimum=1,
            maximum=65535,
        )
        return VpnProfile(
            available=available,
            relay_endpoint=cls._require_vpn_relay_endpoint(
                record.get("relay_endpoint"),
                context=f"{context}.relay_endpoint",
                allow_empty=not available,
            ),
            supported_exit_classes=supported_exit_classes,
            default_exit_class=cls._require_vpn_enum(
                record.get("default_exit_class"),
                _VPN_EXIT_CLASSES,
                f"{context}.default_exit_class",
            ),
            lease_secs=lease_secs,
            dns_push_interval_secs=dns_push_interval_secs,
            meter_family=cls._require_string(record.get("meter_family"), f"{context}.meter_family"),
            route_pushes=cls._parse_vpn_string_list(record.get("route_pushes"), context=f"{context}.route_pushes"),
            excluded_routes=cls._parse_vpn_string_list(
                record.get("excluded_routes"),
                context=f"{context}.excluded_routes",
            ),
            dns_servers=cls._parse_vpn_string_list(record.get("dns_servers"), context=f"{context}.dns_servers"),
            tunnel_addresses=cls._parse_vpn_string_list(
                record.get("tunnel_addresses"),
                context=f"{context}.tunnel_addresses",
            ),
            mtu_bytes=mtu_bytes,
            display_billing_label=cls._require_string(
                record.get("display_billing_label"),
                f"{context}.display_billing_label",
            ),
            operator_account_id=cls._require_string(
                record.get("operator_account_id"),
                f"{context}.operator_account_id",
            ),
            lease_fee=cls._quantity(record.get("lease_fee"), f"{context}.lease_fee"),
            settlement_grace_secs=settlement_grace_secs,
            flow_label_bits=flow_label_bits,
            padding_budget_ms=padding_budget_ms,
            **trust_fields,
        )

    @classmethod
    def _parse_vpn_quote(cls, payload: Mapping[str, Any], *, context: str) -> VpnQuote:
        record = cls._ensure_mapping(payload, context)
        cls._validate_exact_fields(record, _VPN_QUOTE_RESPONSE_FIELDS, context)
        trust_fields = cls._parse_vpn_trust_fields(record, context=context)
        lease_secs = cls._require_vpn_unsigned_range(
            record.get("lease_secs"),
            f"{context}.lease_secs",
            minimum=1,
            maximum=_VPN_LEASE_SECONDS_MAX,
        )
        mtu_bytes = cls._require_vpn_unsigned_constant(
            record.get("mtu_bytes"),
            f"{context}.mtu_bytes",
            1280,
        )
        flow_label_bits = cls._require_vpn_unsigned_constant(
            record.get("flow_label_bits"),
            f"{context}.flow_label_bits",
            24,
        )
        padding_budget_ms = cls._require_vpn_unsigned_range(
            record.get("padding_budget_ms"),
            f"{context}.padding_budget_ms",
            minimum=1,
            maximum=65535,
        )
        return VpnQuote(
            quote_id=cls._require_exact_lower_hex_string(
                record.get("quote_id"),
                context=f"{context}.quote_id",
                expected_length=64,
            ),
            lease_id_hex=cls._require_exact_lower_hex_string(
                record.get("lease_id_hex"),
                context=f"{context}.lease_id_hex",
                expected_length=64,
            ),
            session_id_hex=cls._require_exact_lower_hex_string(
                record.get("session_id_hex"),
                context=f"{context}.session_id_hex",
                expected_length=32,
            ),
            payment_reference=cls._require_string(
                record.get("payment_reference"),
                f"{context}.payment_reference",
            ),
            account_id=cls._require_string(record.get("account_id"), f"{context}.account_id"),
            exit_class=cls._require_vpn_enum(
                record.get("exit_class"),
                _VPN_EXIT_CLASSES,
                f"{context}.exit_class",
            ),
            relay_endpoint=cls._require_vpn_relay_endpoint(
                record.get("relay_endpoint"),
                context=f"{context}.relay_endpoint",
            ),
            lease_secs=lease_secs,
            quote_expires_at_ms=cls._require_vpn_uint64(
                record.get("quote_expires_at_ms"),
                f"{context}.quote_expires_at_ms",
            ),
            fee_asset_id=cls._require_string(record.get("fee_asset_id"), f"{context}.fee_asset_id"),
            escrow_account_id=cls._require_string(
                record.get("escrow_account_id"),
                f"{context}.escrow_account_id",
            ),
            operator_account_id=cls._require_string(
                record.get("operator_account_id"),
                f"{context}.operator_account_id",
            ),
            lease_fee=cls._quantity(record.get("lease_fee"), f"{context}.lease_fee"),
            route_pushes=cls._parse_vpn_string_list(record.get("route_pushes"), context=f"{context}.route_pushes"),
            excluded_routes=cls._parse_vpn_string_list(
                record.get("excluded_routes"),
                context=f"{context}.excluded_routes",
            ),
            dns_servers=cls._parse_vpn_string_list(record.get("dns_servers"), context=f"{context}.dns_servers"),
            tunnel_addresses=cls._parse_vpn_string_list(
                record.get("tunnel_addresses"),
                context=f"{context}.tunnel_addresses",
            ),
            mtu_bytes=mtu_bytes,
            meter_family=cls._require_string(record.get("meter_family"), f"{context}.meter_family"),
            flow_label_bits=flow_label_bits,
            padding_budget_ms=padding_budget_ms,
            **trust_fields,
            metering_public_key_hex=cls._require_exact_lower_hex_string(
                record.get("metering_public_key_hex"),
                context=f"{context}.metering_public_key_hex",
                expected_length=64,
            ),
            open_lease_instruction=cls._parse_vpn_tx_instruction(
                record.get("open_lease_instruction"),
                context=f"{context}.open_lease_instruction",
            ),
        )

    @classmethod
    def _parse_vpn_session(cls, payload: Mapping[str, Any], *, context: str) -> VpnSession:
        record = cls._ensure_mapping(payload, context)
        cls._validate_exact_fields(record, _VPN_SESSION_RESPONSE_FIELDS, context)
        trust_fields = cls._parse_vpn_trust_fields(record, context=context)
        lease_secs = cls._require_vpn_unsigned_range(
            record.get("lease_secs"),
            f"{context}.lease_secs",
            minimum=1,
            maximum=_VPN_LEASE_SECONDS_MAX,
        )
        flow_label_bits = cls._require_vpn_unsigned_constant(
            record.get("flow_label_bits"),
            f"{context}.flow_label_bits",
            24,
        )
        padding_budget_ms = cls._require_vpn_unsigned_range(
            record.get("padding_budget_ms"),
            f"{context}.padding_budget_ms",
            minimum=1,
            maximum=65535,
        )
        mtu_bytes = cls._require_vpn_unsigned_constant(
            record.get("mtu_bytes"),
            f"{context}.mtu_bytes",
            1280,
        )
        return VpnSession(
            session_id=cls._require_exact_lower_hex_string(
                record.get("session_id"),
                context=f"{context}.session_id",
                expected_length=64,
            ),
            account_id=cls._require_string(record.get("account_id"), f"{context}.account_id"),
            exit_class=cls._require_vpn_enum(
                record.get("exit_class"),
                _VPN_EXIT_CLASSES,
                f"{context}.exit_class",
            ),
            relay_endpoint=cls._require_vpn_relay_endpoint(
                record.get("relay_endpoint"),
                context=f"{context}.relay_endpoint",
            ),
            lease_secs=lease_secs,
            expires_at_ms=cls._require_vpn_uint64(record.get("expires_at_ms"), f"{context}.expires_at_ms"),
            connected_at_ms=cls._require_vpn_uint64(
                record.get("connected_at_ms"),
                f"{context}.connected_at_ms",
            ),
            meter_family=cls._require_string(record.get("meter_family"), f"{context}.meter_family"),
            quote_id=cls._require_exact_lower_hex_string(
                record.get("quote_id"),
                context=f"{context}.quote_id",
                expected_length=64,
            ),
            payment_reference=cls._require_string(
                record.get("payment_reference"),
                f"{context}.payment_reference",
            ),
            payment_tx_hash=cls._require_exact_lower_hex_string(
                record.get("payment_tx_hash"),
                context=f"{context}.payment_tx_hash",
                expected_length=64,
            ),
            fee_asset_id=cls._require_string(record.get("fee_asset_id"), f"{context}.fee_asset_id"),
            escrow_account_id=cls._require_string(
                record.get("escrow_account_id"),
                f"{context}.escrow_account_id",
            ),
            operator_account_id=cls._require_string(
                record.get("operator_account_id"),
                f"{context}.operator_account_id",
            ),
            lease_fee=cls._quantity(record.get("lease_fee"), f"{context}.lease_fee"),
            flow_label_bits=flow_label_bits,
            padding_budget_ms=padding_budget_ms,
            **trust_fields,
            route_pushes=cls._parse_vpn_string_list(record.get("route_pushes"), context=f"{context}.route_pushes"),
            excluded_routes=cls._parse_vpn_string_list(
                record.get("excluded_routes"),
                context=f"{context}.excluded_routes",
            ),
            dns_servers=cls._parse_vpn_string_list(record.get("dns_servers"), context=f"{context}.dns_servers"),
            tunnel_addresses=cls._parse_vpn_string_list(
                record.get("tunnel_addresses"),
                context=f"{context}.tunnel_addresses",
            ),
            mtu_bytes=mtu_bytes,
            helper_ticket_hex=cls._require_vpn_helper_ticket_hex(
                record.get("helper_ticket_hex"),
                context=f"{context}.helper_ticket_hex",
            ),
            bytes_in=cls._require_vpn_uint64(record.get("bytes_in"), f"{context}.bytes_in"),
            bytes_out=cls._require_vpn_uint64(record.get("bytes_out"), f"{context}.bytes_out"),
            status=cls._require_vpn_enum(
                record.get("status"),
                _VPN_SESSION_STATUSES,
                f"{context}.status",
            ),
        )

    @classmethod
    def _parse_vpn_receipt(cls, payload: Mapping[str, Any], *, context: str) -> VpnReceipt:
        record = cls._ensure_mapping(payload, context)
        cls._validate_exact_fields(record, _VPN_RECEIPT_RESPONSE_FIELDS, context)
        return VpnReceipt(
            session_id=cls._require_exact_lower_hex_string(
                record.get("session_id"),
                context=f"{context}.session_id",
                expected_length=64,
            ),
            account_id=cls._require_string(record.get("account_id"), f"{context}.account_id"),
            exit_class=cls._require_vpn_enum(
                record.get("exit_class"),
                _VPN_EXIT_CLASSES,
                f"{context}.exit_class",
            ),
            relay_endpoint=cls._require_string(record.get("relay_endpoint"), f"{context}.relay_endpoint"),
            meter_family=cls._require_string(record.get("meter_family"), f"{context}.meter_family"),
            connected_at_ms=cls._require_vpn_uint64(
                record.get("connected_at_ms"),
                f"{context}.connected_at_ms",
            ),
            disconnected_at_ms=cls._require_vpn_uint64(
                record.get("disconnected_at_ms"),
                f"{context}.disconnected_at_ms",
            ),
            duration_ms=cls._require_vpn_uint64(record.get("duration_ms"), f"{context}.duration_ms"),
            bytes_in=cls._require_vpn_uint64(record.get("bytes_in"), f"{context}.bytes_in"),
            bytes_out=cls._require_vpn_uint64(record.get("bytes_out"), f"{context}.bytes_out"),
            status=cls._require_vpn_enum(
                record.get("status"),
                _VPN_RECEIPT_STATUSES,
                f"{context}.status",
            ),
            receipt_source=cls._require_vpn_enum(
                record.get("receipt_source"),
                _VPN_RECEIPT_SOURCES,
                f"{context}.receipt_source",
            ),
            quote_id=cls._require_exact_lower_hex_string(
                record.get("quote_id"),
                context=f"{context}.quote_id",
                expected_length=64,
            ),
            payment_tx_hash=cls._require_exact_lower_hex_string(
                record.get("payment_tx_hash"),
                context=f"{context}.payment_tx_hash",
                expected_length=64,
            ),
            fee_asset_id=cls._require_string(record.get("fee_asset_id"), f"{context}.fee_asset_id"),
            escrow_account_id=cls._require_string(
                record.get("escrow_account_id"),
                f"{context}.escrow_account_id",
            ),
            operator_account_id=cls._require_string(
                record.get("operator_account_id"),
                f"{context}.operator_account_id",
            ),
            lease_fee=cls._quantity(record.get("lease_fee"), f"{context}.lease_fee"),
            earned_fee=cls._quantity(record.get("earned_fee"), f"{context}.earned_fee"),
            refunded_fee=cls._quantity(
                record.get("refunded_fee"),
                f"{context}.refunded_fee",
            ),
            lease_id_hex=cls._require_exact_lower_hex_string(
                record.get("lease_id_hex"),
                context=f"{context}.lease_id_hex",
                expected_length=64,
            ),
            settle_lease_instruction=cls._parse_optional_vpn_tx_instruction(
                record.get("settle_lease_instruction"),
                context=f"{context}.settle_lease_instruction",
            ),
        )

    @classmethod
    def _parse_vpn_receipt_list(
        cls,
        payload: Optional[Mapping[str, Any]],
        *,
        context: str,
    ) -> VpnReceiptListResponse:
        record = cls._ensure_mapping(payload, context)
        cls._validate_exact_fields(record, _VPN_RECEIPT_LIST_RESPONSE_FIELDS, context)
        items_payload = record.get("items")
        if not isinstance(items_payload, list):
            raise RuntimeError(f"{context}.items must be a list")
        if len(items_payload) > 24:
            raise RuntimeError(f"{context}.items must contain at most 24 receipts")
        total = cls._require_vpn_unsigned_range(
            record.get("total"),
            f"{context}.total",
            minimum=0,
            maximum=24,
        )
        return VpnReceiptListResponse(
            items=[
                cls._parse_vpn_receipt(entry, context=f"{context}.items[{index}]")
                for index, entry in enumerate(items_payload)
            ],
            total=total,
        )

    def _operator_get(
        self,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[Mapping[str, str]] = None,
        stream: bool = False,
    ) -> requests.Response:
        context = self._operator_signing_context
        if context is None:
            raise ValueError("operator GET requires an immutable ToriiOperatorSigningContext before dispatch")
        query = urlencode(sorted(params.items()), doseq=True) if params else ""
        exact_target = f"{path}?{query}" if query else path
        final_headers: Dict[str, str] = {"Accept": "application/json"}
        if headers:
            final_headers.update(dict(headers))
        session_headers = getattr(self._session, "headers", {})
        for source in (session_headers, final_headers):
            for name in source:
                if str(name).lower() in _OPERATOR_FORBIDDEN_AUTH_HEADERS:
                    raise ValueError(
                        "operator GETs reject token, canonical-account, "
                        f"witness, and precomputed operator authentication header {name}"
                    )
        if getattr(self._session, "auth", None) is not None:
            raise ValueError("operator GETs reject Session.auth token fallback")
        final_headers = _OperatorRequestHeaderPlan(final_headers, context)
        return self._request(
            "GET",
            exact_target,
            headers=final_headers,
            stream=stream,
            allow_retry=False,
            allow_redirects=False,
        )

    def _request(
        self,
        method: str,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[MutableMapping[str, str]] = None,
        data: Optional[bytes] = None,
        stream: bool = False,
        allow_retry: bool = True,
        allow_redirects: bool = True,
        timeout: Optional[float] = None,
    ) -> requests.Response:
        return _send_request(
            session=self._session,
            base_url=self._base_url,
            method=method,
            path=path,
            params=params,
            headers=headers,
            data=data,
            stream=stream,
            allow_retry=allow_retry,
            allow_redirects=allow_redirects,
            timeout=timeout,
            build_headers=build_canonical_request_headers,
            build_operator_headers=build_operator_request_headers,
        )

    @staticmethod
    def _expect_status(
        response: requests.Response,
        expected: Iterable[int],
        *,
        maximum_body_bytes: Optional[int] = None,
        context: str = "Torii",
    ) -> None:
        expected_set = set(expected)
        if response.status_code in expected_set:
            return
        if maximum_body_bytes is None:
            message = _format_error_body(response.text)
        else:
            body = _read_bounded_sccp_response_body(
                response, maximum_body_bytes, f"{context} error"
            )
            try:
                text = body.decode("utf-8", "strict")
            except UnicodeDecodeError as exc:
                raise ValueError(f"{context} error response body must be strict UTF-8") from exc
            message = _format_error_body(text)
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

    @classmethod
    def _sorafs_orderbook_headers(
        cls,
        *,
        if_none_match: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
        context: str,
        cache: bool = False,
    ) -> Dict[str, str]:
        if not cache and if_none_match is not None:
            raise ValueError(f"{context} does not accept cache validators")
        final_headers: Dict[str, str] = {}
        if headers is not None:
            if not isinstance(headers, Mapping):
                raise TypeError(f"{context}.headers must be a mapping")
            for key, value in headers.items():
                name = str(key)
                if name.lower() in {"accept", "if-none-match"}:
                    raise ValueError(
                        f"{context}.headers must not override the managed {name} header"
                    )
                final_headers[name] = str(value)
        final_headers["Accept"] = "application/json"
        if if_none_match is not None:
            final_headers["If-None-Match"] = cls._require_non_empty_string(
                if_none_match,
                f"{context}.if_none_match",
            )
        return final_headers

    @classmethod
    def _sorafs_orderbook_read_params(
        cls,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        context: str,
    ) -> Optional[Dict[str, Any]]:
        cls._validate_sorafs_orderbook_finalized_anchor(
            expected_finalized_height,
            expected_finalized_block_hash_hex,
            context=context,
        )
        params: Dict[str, Any] = {}
        if expected_finalized_height is not None:
            params["expected_finalized_height"] = cls._sorafs_orderbook_query_uint(
                expected_finalized_height,
                f"{context}.expected_finalized_height",
                minimum=1,
                maximum=(1 << 64) - 1,
            )
            params["expected_finalized_block_hash_hex"] = (
                cls._sorafs_orderbook_query_hex32(
                    expected_finalized_block_hash_hex,
                    f"{context}.expected_finalized_block_hash_hex",
                    nonzero=True,
                )
            )
        if after_id_hex is not None:
            params["after_id_hex"] = cls._sorafs_orderbook_query_hex32(
                after_id_hex,
                f"{context}.after_id_hex",
                nonzero=False,
            )
        if limit is not None:
            params["limit"] = cls._sorafs_orderbook_query_uint(
                limit,
                f"{context}.limit",
                minimum=1,
                maximum=_SORAFS_ORDERBOOK_QUERY_MAX_ITEMS,
            )
        return params or None

    @classmethod
    def _sorafs_orderbook_event_params(
        cls,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_sequence: Optional[Any] = None,
        after_block_height: Optional[Any] = None,
        after_block_hash_hex: Optional[Any] = None,
        after_event_index: Optional[Any] = None,
        limit: Optional[Any] = None,
        context: str,
    ) -> Optional[Dict[str, Any]]:
        cls._validate_sorafs_orderbook_finalized_anchor(
            expected_finalized_height,
            expected_finalized_block_hash_hex,
            context=context,
        )
        cursor_parts = (
            after_sequence,
            after_block_height,
            after_block_hash_hex,
            after_event_index,
        )
        supplied_cursor_parts = sum(part is not None for part in cursor_parts)
        if supplied_cursor_parts not in (0, len(cursor_parts)):
            raise ValueError(
                f"{context} requires all four finalized event cursor fields or none"
            )
        params: Dict[str, Any] = {}
        if expected_finalized_height is not None:
            params["expected_finalized_height"] = cls._sorafs_orderbook_query_uint(
                expected_finalized_height,
                f"{context}.expected_finalized_height",
                minimum=1,
                maximum=(1 << 64) - 1,
            )
            params["expected_finalized_block_hash_hex"] = (
                cls._sorafs_orderbook_query_hex32(
                    expected_finalized_block_hash_hex,
                    f"{context}.expected_finalized_block_hash_hex",
                    nonzero=True,
                )
            )
        if supplied_cursor_parts:
            params["after_sequence"] = cls._sorafs_orderbook_query_uint(
                after_sequence,
                f"{context}.after_sequence",
                minimum=1,
                maximum=(1 << 64) - 1,
            )
            params["after_block_height"] = cls._sorafs_orderbook_query_uint(
                after_block_height,
                f"{context}.after_block_height",
                minimum=1,
                maximum=(1 << 64) - 1,
            )
            params["after_block_hash_hex"] = cls._sorafs_orderbook_query_hex32(
                after_block_hash_hex,
                f"{context}.after_block_hash_hex",
                nonzero=True,
            )
            params["after_event_index"] = cls._sorafs_orderbook_query_uint(
                after_event_index,
                f"{context}.after_event_index",
                minimum=0,
                maximum=(1 << 32) - 1,
            )
        if limit is not None:
            params["limit"] = cls._sorafs_orderbook_query_uint(
                limit,
                f"{context}.limit",
                minimum=1,
                maximum=_SORAFS_ORDERBOOK_QUERY_MAX_ITEMS,
            )
        return params or None

    @classmethod
    def _validate_sorafs_orderbook_finalized_anchor(
        cls,
        height: Optional[Any],
        block_hash_hex: Optional[Any],
        *,
        context: str,
    ) -> None:
        if (height is None) != (block_hash_hex is None):
            raise ValueError(
                f"{context} requires expected_finalized_height and "
                "expected_finalized_block_hash_hex together"
            )

    @classmethod
    def _sorafs_orderbook_query_uint(
        cls,
        value: Any,
        context: str,
        *,
        minimum: int,
        maximum: int,
    ) -> int:
        if type(value) is not int:
            raise TypeError(f"{context} must be an integer")
        if not minimum <= value <= maximum:
            raise ValueError(f"{context} must be within {minimum}..={maximum}")
        return value

    @classmethod
    def _sorafs_orderbook_query_hex32(
        cls,
        value: Any,
        context: str,
        *,
        nonzero: bool,
    ) -> str:
        if type(value) is not str or re.fullmatch(r"[0-9a-f]{64}", value) is None:
            raise ValueError(f"{context} must be exactly 64 lowercase hexadecimal characters")
        if nonzero and value == "0" * 64:
            raise ValueError(f"{context} must not be the all-zero hash")
        return value

    @classmethod
    def _parse_sorafs_orderbook_book(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            frozenset({"source", "status", "orders"}),
            context,
        )
        return {
            "source": cls._require_sorafs_orderbook_source(
                record.get("source"),
                f"{context}.source",
            ),
            "status": cls._parse_sorafs_orderbook_ledger_status(
                record.get("status"),
                context=f"{context}.status",
            ),
            "orders": cls._parse_sorafs_orderbook_order_page(
                record.get("orders"),
                context=f"{context}.orders",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_trade_page_response(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            frozenset({"source", "trades"}),
            context,
        )
        return {
            "source": cls._require_sorafs_orderbook_source(
                record.get("source"),
                f"{context}.source",
            ),
            "trades": cls._parse_sorafs_orderbook_trade_page(
                record.get("trades"),
                context=f"{context}.trades",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_channel_page_response(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            frozenset({"source", "channels"}),
            context,
        )
        return {
            "source": cls._require_sorafs_orderbook_source(
                record.get("source"),
                f"{context}.source",
            ),
            "channels": cls._parse_sorafs_orderbook_channel_page(
                record.get("channels"),
                context=f"{context}.channels",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_receipt_page_response(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            frozenset({"source", "receipts"}),
            context,
        )
        return {
            "source": cls._require_sorafs_orderbook_source(
                record.get("source"),
                f"{context}.source",
            ),
            "receipts": cls._parse_sorafs_orderbook_receipt_page(
                record.get("receipts"),
                context=f"{context}.receipts",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_event_page_response(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            frozenset({"source", "events"}),
            context,
        )
        return {
            "source": cls._require_sorafs_orderbook_source(
                record.get("source"),
                f"{context}.source",
            ),
            "events": cls._parse_sorafs_orderbook_event_page(
                record.get("events"),
                context=f"{context}.events",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_submission_receipt(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_SUBMISSION_RECEIPT_FIELDS,
            context,
        )
        receipt_payload = cls._ensure_mapping(record.get("payload"), f"{context}.payload")
        cls._require_exact_sorafs_orderbook_fields(
            receipt_payload,
            _SORAFS_ORDERBOOK_SUBMISSION_PAYLOAD_FIELDS,
            f"{context}.payload",
        )
        return {
            "payload": {
                key: cls._require_sorafs_orderbook_string(
                    receipt_payload.get(key),
                    f"{context}.payload.{key}",
                )
                for key in (
                    "tx_hash",
                    "entrypoint_hash",
                    "signed_transaction_hash",
                )
            }
            | {
                "submitted_at_ms": cls._require_sorafs_orderbook_uint(
                    receipt_payload.get("submitted_at_ms"),
                    f"{context}.payload.submitted_at_ms",
                ),
                "submitted_at_height": cls._require_sorafs_orderbook_uint(
                    receipt_payload.get("submitted_at_height"),
                    f"{context}.payload.submitted_at_height",
                ),
                "signer": cls._require_sorafs_orderbook_string(
                    receipt_payload.get("signer"),
                    f"{context}.payload.signer",
                ),
            },
            "signature": cls._require_sorafs_orderbook_string(
                record.get("signature"),
                f"{context}.signature",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_ledger_status(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, int]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_STATUS_FIELDS,
            context,
        )
        return {
            field: cls._require_sorafs_orderbook_uint(
                record.get(field),
                f"{context}.{field}",
            )
            for field in (
                "open_orders",
                "partially_filled_orders",
                "filled_orders",
                "cancelled_orders",
                "expired_orders",
                "trades",
                "settlement_receipts",
                "settlement_channels",
                "open_settlement_channels",
                "book_revision",
                "next_admission_sequence",
                "next_trade_sequence",
                "updated_at_unix",
            )
        }

    @classmethod
    def _parse_sorafs_orderbook_finalized_cursor(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_FINALIZED_CURSOR_FIELDS,
            context,
        )
        return {
            "height": cls._require_sorafs_orderbook_uint(
                record.get("height"),
                f"{context}.height",
            ),
            "block_hash": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("block_hash"),
                context=f"{context}.block_hash",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_order_page(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_ORDER_PAGE_FIELDS,
            context,
        )
        has_more = cls._require_sorafs_orderbook_bool(
            record.get("has_more"),
            f"{context}.has_more",
        )
        next_after = cls._parse_optional_sorafs_orderbook_fixed_bytes32(
            record.get("next_after_order_id"),
            context=f"{context}.next_after_order_id",
        )
        cls._validate_sorafs_orderbook_page_cursor(
            has_more,
            next_after,
            context=f"{context}.next_after_order_id",
        )
        return {
            "finalized_cursor": cls._parse_sorafs_orderbook_finalized_cursor(
                record.get("finalized_cursor"),
                context=f"{context}.finalized_cursor",
            ),
            "orders": cls._parse_sorafs_orderbook_array(
                record.get("orders"),
                context=f"{context}.orders",
                normalizer=cls._parse_sorafs_orderbook_order_record,
            ),
            "has_more": has_more,
            "next_after_order_id": next_after,
        }

    @classmethod
    def _parse_sorafs_orderbook_trade_page(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_TRADE_PAGE_FIELDS,
            context,
        )
        has_more = cls._require_sorafs_orderbook_bool(
            record.get("has_more"),
            f"{context}.has_more",
        )
        next_after = cls._parse_optional_sorafs_orderbook_fixed_bytes32(
            record.get("next_after_trade_id"),
            context=f"{context}.next_after_trade_id",
        )
        cls._validate_sorafs_orderbook_page_cursor(
            has_more,
            next_after,
            context=f"{context}.next_after_trade_id",
        )
        return {
            "finalized_cursor": cls._parse_sorafs_orderbook_finalized_cursor(
                record.get("finalized_cursor"),
                context=f"{context}.finalized_cursor",
            ),
            "trades": cls._parse_sorafs_orderbook_array(
                record.get("trades"),
                context=f"{context}.trades",
                normalizer=cls._parse_sorafs_orderbook_trade_record,
            ),
            "has_more": has_more,
            "next_after_trade_id": next_after,
        }

    @classmethod
    def _parse_sorafs_orderbook_channel_page(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_CHANNEL_PAGE_FIELDS,
            context,
        )
        has_more = cls._require_sorafs_orderbook_bool(
            record.get("has_more"),
            f"{context}.has_more",
        )
        next_after = cls._parse_optional_sorafs_orderbook_fixed_bytes32(
            record.get("next_after_channel_id"),
            context=f"{context}.next_after_channel_id",
        )
        cls._validate_sorafs_orderbook_page_cursor(
            has_more,
            next_after,
            context=f"{context}.next_after_channel_id",
        )
        return {
            "finalized_cursor": cls._parse_sorafs_orderbook_finalized_cursor(
                record.get("finalized_cursor"),
                context=f"{context}.finalized_cursor",
            ),
            "channels": cls._parse_sorafs_orderbook_array(
                record.get("channels"),
                context=f"{context}.channels",
                normalizer=cls._parse_sorafs_orderbook_channel_record,
            ),
            "has_more": has_more,
            "next_after_channel_id": next_after,
        }

    @classmethod
    def _parse_sorafs_orderbook_receipt_page(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_RECEIPT_PAGE_FIELDS,
            context,
        )
        has_more = cls._require_sorafs_orderbook_bool(
            record.get("has_more"),
            f"{context}.has_more",
        )
        next_after = cls._parse_optional_sorafs_orderbook_fixed_bytes32(
            record.get("next_after_receipt_id"),
            context=f"{context}.next_after_receipt_id",
        )
        cls._validate_sorafs_orderbook_page_cursor(
            has_more,
            next_after,
            context=f"{context}.next_after_receipt_id",
        )
        return {
            "finalized_cursor": cls._parse_sorafs_orderbook_finalized_cursor(
                record.get("finalized_cursor"),
                context=f"{context}.finalized_cursor",
            ),
            "receipts": cls._parse_sorafs_orderbook_array(
                record.get("receipts"),
                context=f"{context}.receipts",
                normalizer=cls._parse_sorafs_orderbook_receipt_record,
            ),
            "has_more": has_more,
            "next_after_receipt_id": next_after,
        }

    @classmethod
    def _parse_sorafs_orderbook_event_page(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_EVENT_PAGE_FIELDS,
            context,
        )
        has_more = cls._require_sorafs_orderbook_bool(
            record.get("has_more"),
            f"{context}.has_more",
        )
        next_after_payload = record.get("next_after")
        next_after = (
            None
            if next_after_payload is None
            else cls._parse_sorafs_orderbook_event_cursor(
                next_after_payload,
                context=f"{context}.next_after",
            )
        )
        cls._validate_sorafs_orderbook_page_cursor(
            has_more,
            next_after,
            context=f"{context}.next_after",
        )
        return {
            "finalized_cursor": cls._parse_sorafs_orderbook_finalized_cursor(
                record.get("finalized_cursor"),
                context=f"{context}.finalized_cursor",
            ),
            "events": cls._parse_sorafs_orderbook_array(
                record.get("events"),
                context=f"{context}.events",
                normalizer=cls._parse_sorafs_orderbook_finalized_event,
            ),
            "has_more": has_more,
            "next_after": next_after,
        }

    @classmethod
    def _parse_sorafs_orderbook_order_record(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_ORDER_RECORD_FIELDS,
            context,
        )
        status = cls._parse_sorafs_orderbook_tagged_unit(
            record.get("status"),
            tag="status",
            content="value",
            allowed=_SORAFS_ORDERBOOK_ORDER_STATUS_VALUES,
            context=f"{context}.status",
        )
        canonical_cancel = cls._parse_optional_sorafs_orderbook_base64(
            record.get("canonical_cancel"),
            context=f"{context}.canonical_cancel",
        )
        cancelled_at_unix = cls._parse_optional_sorafs_orderbook_uint(
            record.get("cancelled_at_unix"),
            context=f"{context}.cancelled_at_unix",
        )
        cancelled_policy_digest = cls._parse_optional_sorafs_orderbook_fixed_bytes32(
            record.get("cancelled_policy_digest"),
            context=f"{context}.cancelled_policy_digest",
        )
        cancellation_values = (
            canonical_cancel,
            cancelled_at_unix,
            cancelled_policy_digest,
        )
        present_cancellation_values = sum(
            value is not None for value in cancellation_values
        )
        expected_cancellation_values = 3 if status["status"] == "cancelled" else 0
        if present_cancellation_values != expected_cancellation_values:
            raise ValueError(
                f"{context} cancellation fields must be present exactly for cancelled orders"
            )
        return {
            "order_id": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("order_id"),
                context=f"{context}.order_id",
            ),
            "owner": cls._require_sorafs_orderbook_string(
                record.get("owner"),
                f"{context}.owner",
            ),
            "canonical_order": cls._parse_sorafs_orderbook_base64(
                record.get("canonical_order"),
                context=f"{context}.canonical_order",
            ),
            "admitted_policy_digest": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("admitted_policy_digest"),
                context=f"{context}.admitted_policy_digest",
            ),
            "admitted_at_unix": cls._require_sorafs_orderbook_uint(
                record.get("admitted_at_unix"),
                f"{context}.admitted_at_unix",
            ),
            "admission_sequence": cls._require_sorafs_orderbook_uint(
                record.get("admission_sequence"),
                f"{context}.admission_sequence",
                minimum=1,
            ),
            "remaining_gib": cls._require_sorafs_orderbook_uint(
                record.get("remaining_gib"),
                f"{context}.remaining_gib",
            ),
            "status": status,
            "updated_at_unix": cls._require_sorafs_orderbook_uint(
                record.get("updated_at_unix"),
                f"{context}.updated_at_unix",
            ),
            "canonical_cancel": canonical_cancel,
            "cancelled_at_unix": cancelled_at_unix,
            "cancelled_policy_digest": cancelled_policy_digest,
        }

    @classmethod
    def _parse_sorafs_orderbook_trade_record(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_TRADE_RECORD_FIELDS,
            context,
        )
        return {
            field: cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get(field),
                context=f"{context}.{field}",
            )
            for field in (
                "trade_id",
                "maker_order_id",
                "taker_order_id",
                "channel_id",
            )
        } | {
            "trade_sequence": cls._require_sorafs_orderbook_uint(
                record.get("trade_sequence"),
                f"{context}.trade_sequence",
                minimum=1,
            ),
            "canonical_trade": cls._parse_sorafs_orderbook_base64(
                record.get("canonical_trade"),
                context=f"{context}.canonical_trade",
            ),
            "book_revision": cls._require_sorafs_orderbook_uint(
                record.get("book_revision"),
                f"{context}.book_revision",
            ),
            "recorded_at_unix": cls._require_sorafs_orderbook_uint(
                record.get("recorded_at_unix"),
                f"{context}.recorded_at_unix",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_channel_record(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_CHANNEL_RECORD_FIELDS,
            context,
        )
        return {
            "channel_id": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("channel_id"),
                context=f"{context}.channel_id",
            ),
            "trade_id": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("trade_id"),
                context=f"{context}.trade_id",
            ),
            "buyer": cls._require_sorafs_orderbook_string(
                record.get("buyer"),
                f"{context}.buyer",
            ),
            "provider": cls._require_sorafs_orderbook_string(
                record.get("provider"),
                f"{context}.provider",
            ),
            "provider_id": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("provider_id"),
                context=f"{context}.provider_id",
            ),
            "settlement_authority": cls._require_sorafs_orderbook_string(
                record.get("settlement_authority"),
                f"{context}.settlement_authority",
            ),
            "total_bytes": cls._require_sorafs_orderbook_uint(
                record.get("total_bytes"),
                f"{context}.total_bytes",
            ),
            "remaining_bytes": cls._require_sorafs_orderbook_uint(
                record.get("remaining_bytes"),
                f"{context}.remaining_bytes",
            ),
            "initial_xor_locked": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("initial_xor_locked"),
                f"{context}.initial_xor_locked",
            ),
            "remaining_xor_locked": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("remaining_xor_locked"),
                f"{context}.remaining_xor_locked",
            ),
            "status": cls._parse_sorafs_orderbook_tagged_unit(
                record.get("status"),
                tag="status",
                content="value",
                allowed=_SORAFS_ORDERBOOK_CHANNEL_STATUS_VALUES,
                context=f"{context}.status",
            ),
            "opened_at_unix": cls._require_sorafs_orderbook_uint(
                record.get("opened_at_unix"),
                f"{context}.opened_at_unix",
            ),
            "expires_at_unix": cls._require_sorafs_orderbook_uint(
                record.get("expires_at_unix"),
                f"{context}.expires_at_unix",
            ),
            "updated_at_unix": cls._require_sorafs_orderbook_uint(
                record.get("updated_at_unix"),
                f"{context}.updated_at_unix",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_receipt_record(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_RECEIPT_RECORD_FIELDS,
            context,
        )
        return {
            "receipt_id": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("receipt_id"),
                context=f"{context}.receipt_id",
            ),
            "channel_id": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("channel_id"),
                context=f"{context}.channel_id",
            ),
            "trade_id": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("trade_id"),
                context=f"{context}.trade_id",
            ),
            "canonical_receipt": cls._parse_sorafs_orderbook_base64(
                record.get("canonical_receipt"),
                context=f"{context}.canonical_receipt",
            ),
            "admitted_policy_digest": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("admitted_policy_digest"),
                context=f"{context}.admitted_policy_digest",
            ),
            "admitted_at_unix": cls._require_sorafs_orderbook_uint(
                record.get("admitted_at_unix"),
                f"{context}.admitted_at_unix",
            ),
            "recorded_by": cls._require_sorafs_orderbook_string(
                record.get("recorded_by"),
                f"{context}.recorded_by",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_event_cursor(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_EVENT_CURSOR_FIELDS,
            context,
        )
        return {
            "sequence": cls._require_sorafs_orderbook_uint(
                record.get("sequence"),
                f"{context}.sequence",
                minimum=1,
            ),
            "block_height": cls._require_sorafs_orderbook_uint(
                record.get("block_height"),
                f"{context}.block_height",
                minimum=1,
            ),
            "block_hash": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("block_hash"),
                context=f"{context}.block_hash",
            ),
            "event_index": cls._require_sorafs_orderbook_uint(
                record.get("event_index"),
                f"{context}.event_index",
                maximum=(1 << 32) - 1,
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_finalized_event(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_FINALIZED_EVENT_FIELDS,
            context,
        )
        return {
            "sequence": cls._require_sorafs_orderbook_uint(
                record.get("sequence"),
                f"{context}.sequence",
                minimum=1,
            ),
            "block_height": cls._require_sorafs_orderbook_uint(
                record.get("block_height"),
                f"{context}.block_height",
                minimum=1,
            ),
            "block_hash": cls._parse_sorafs_orderbook_fixed_bytes32(
                record.get("block_hash"),
                context=f"{context}.block_hash",
            ),
            "event_index": cls._require_sorafs_orderbook_uint(
                record.get("event_index"),
                f"{context}.event_index",
                maximum=(1 << 32) - 1,
            ),
            "event": cls._parse_sorafs_orderbook_ledger_event(
                record.get("event"),
                context=f"{context}.event",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_ledger_event(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_LEDGER_EVENT_FIELDS,
            context,
        )
        return {
            "kind": cls._parse_sorafs_orderbook_tagged_unit(
                record.get("kind"),
                tag="kind",
                content="detail",
                allowed=_SORAFS_ORDERBOOK_EVENT_KIND_VALUES,
                context=f"{context}.kind",
            ),
            "order_id": cls._parse_optional_sorafs_orderbook_fixed_bytes32(
                record.get("order_id"),
                context=f"{context}.order_id",
            ),
            "trade_id": cls._parse_optional_sorafs_orderbook_fixed_bytes32(
                record.get("trade_id"),
                context=f"{context}.trade_id",
            ),
            "channel_id": cls._parse_optional_sorafs_orderbook_fixed_bytes32(
                record.get("channel_id"),
                context=f"{context}.channel_id",
            ),
            "receipt_id": cls._parse_optional_sorafs_orderbook_fixed_bytes32(
                record.get("receipt_id"),
                context=f"{context}.receipt_id",
            ),
            "provider_id": cls._parse_optional_sorafs_orderbook_fixed_bytes32(
                record.get("provider_id"),
                context=f"{context}.provider_id",
            ),
            "book_revision": cls._require_sorafs_orderbook_uint(
                record.get("book_revision"),
                f"{context}.book_revision",
            ),
            "authority": cls._require_sorafs_orderbook_string(
                record.get("authority"),
                f"{context}.authority",
            ),
            "occurred_at_unix_ms": cls._require_sorafs_orderbook_uint(
                record.get("occurred_at_unix_ms"),
                f"{context}.occurred_at_unix_ms",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_tagged_unit(
        cls,
        payload: Any,
        *,
        tag: str,
        content: str,
        allowed: set[str],
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            frozenset({tag, content}),
            context,
        )
        label = cls._require_sorafs_orderbook_string(
            record.get(tag),
            f"{context}.{tag}",
        )
        if label not in allowed:
            raise ValueError(f"{context}.{tag} must be one of {', '.join(sorted(allowed))}")
        if record.get(content) is not None:
            raise ValueError(f"{context}.{content} must be null for a unit variant")
        return {tag: label, content: None}

    @classmethod
    def _parse_sorafs_orderbook_array(
        cls,
        value: Any,
        *,
        context: str,
        normalizer: Any,
    ) -> List[Any]:
        if type(value) is not list:
            raise TypeError(f"{context} must be a list")
        if len(value) > _SORAFS_ORDERBOOK_QUERY_MAX_ITEMS:
            raise ValueError(
                f"{context} must contain at most {_SORAFS_ORDERBOOK_QUERY_MAX_ITEMS} records"
            )
        return [
            normalizer(entry, context=f"{context}[{index}]")
            for index, entry in enumerate(value)
        ]

    @classmethod
    def _parse_sorafs_orderbook_fixed_bytes32(
        cls,
        value: Any,
        *,
        context: str,
    ) -> List[int]:
        if type(value) is not list or len(value) != 32:
            raise TypeError(f"{context} must be a JSON array of exactly 32 bytes")
        normalized: List[int] = []
        for index, byte in enumerate(value):
            if type(byte) is not int:
                raise TypeError(f"{context}[{index}] must be an integer byte")
            if not 0 <= byte <= 255:
                raise ValueError(f"{context}[{index}] must be within 0..=255")
            normalized.append(byte)
        return normalized

    @classmethod
    def _parse_optional_sorafs_orderbook_fixed_bytes32(
        cls,
        value: Any,
        *,
        context: str,
    ) -> Optional[List[int]]:
        if value is None:
            return None
        return cls._parse_sorafs_orderbook_fixed_bytes32(value, context=context)

    @classmethod
    def _parse_sorafs_orderbook_base64(
        cls,
        value: Any,
        *,
        context: str,
    ) -> str:
        if type(value) is not str or not value:
            raise TypeError(f"{context} must be a non-empty canonical base64 string")
        try:
            encoded = value.encode("ascii")
            decoded = base64.b64decode(encoded, validate=True)
        except (UnicodeEncodeError, binascii.Error, ValueError) as exc:
            raise ValueError(f"{context} must be canonical standard base64") from exc
        if not decoded or base64.b64encode(decoded).decode("ascii") != value:
            raise ValueError(f"{context} must be canonical non-empty standard base64")
        return value

    @classmethod
    def _parse_optional_sorafs_orderbook_base64(
        cls,
        value: Any,
        *,
        context: str,
    ) -> Optional[str]:
        if value is None:
            return None
        return cls._parse_sorafs_orderbook_base64(value, context=context)

    @classmethod
    def _require_sorafs_orderbook_uint(
        cls,
        value: Any,
        context: str,
        *,
        minimum: int = 0,
        maximum: int = (1 << 64) - 1,
    ) -> int:
        if type(value) is not int:
            raise TypeError(f"{context} must be an integer")
        if not minimum <= value <= maximum:
            raise ValueError(f"{context} must be within {minimum}..={maximum}")
        return value

    @classmethod
    def _parse_optional_sorafs_orderbook_uint(
        cls,
        value: Any,
        *,
        context: str,
    ) -> Optional[int]:
        if value is None:
            return None
        return cls._require_sorafs_orderbook_uint(value, context)

    @staticmethod
    def _require_sorafs_orderbook_bool(value: Any, context: str) -> bool:
        if type(value) is not bool:
            raise TypeError(f"{context} must be a boolean")
        return value

    @staticmethod
    def _require_sorafs_orderbook_string(value: Any, context: str) -> str:
        if type(value) is not str:
            raise TypeError(f"{context} must be a string")
        if not value or value != value.strip():
            raise ValueError(f"{context} must be a non-empty canonical string")
        return value

    @classmethod
    def _require_sorafs_orderbook_source(cls, value: Any, context: str) -> str:
        source = cls._require_sorafs_orderbook_string(value, context)
        if source != "finalized_chain":
            raise ValueError(f"{context} must be finalized_chain")
        return source

    @staticmethod
    def _validate_sorafs_orderbook_page_cursor(
        has_more: bool,
        next_after: Any,
        *,
        context: str,
    ) -> None:
        if has_more != (next_after is not None):
            raise ValueError(f"{context} presence must match has_more")

    @classmethod
    def _normalize_sorafs_orderbook_xor_quantity(cls, value: Any, context: str) -> str:
        if type(value) is not str:
            raise TypeError(f"{context} must be a canonical XOR quantity string")
        if len(value) > _SORAFS_XOR_QUANTITY_MAX_TEXT_LENGTH:
            raise ValueError(f"{context} exceeds the bounded XOR quantity text length")
        match = re.fullmatch(r"(0|[1-9][0-9]*)(?:\.([0-9]*[1-9]))?", value)
        if match is None:
            raise ValueError(f"{context} must be a canonical non-negative XOR quantity")
        fractional = match.group(2) or ""
        if len(fractional) > 9:
            raise ValueError(f"{context} must have at most 9 fractional decimal places")
        mantissa = int(f"{match.group(1)}{fractional}")
        if mantissa > (1 << 511) - 1:
            raise ValueError(f"{context} exceeds the 512-bit signed quantity domain")
        return value

    @staticmethod
    def _require_exact_sorafs_orderbook_fields(
        record: Mapping[str, Any],
        expected: frozenset[str],
        context: str,
    ) -> None:
        present = set(record)
        missing = expected.difference(present)
        unexpected = present.difference(expected)
        if not missing and not unexpected:
            return
        details: List[str] = []
        if missing:
            details.append(
                "missing " + ", ".join(sorted(str(field) for field in missing))
            )
        if unexpected:
            details.append(
                "unknown or retired "
                + ", ".join(sorted(str(field) for field in unexpected))
            )
        raise ValueError(f"{context} fields are not canonical: {'; '.join(details)}")

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
    def _normalize_canonical_account_id(value: Any, context: str) -> str:
        literal = ToriiClient._require_non_empty_string(value, context)
        if literal != value or any(ch.isspace() for ch in literal):
            raise ValueError(
                f"{context} must be a canonical I105 account id or on-chain account alias"
            )
        if "@" in literal:
            label, separator, scope = literal.partition("@")
            scope_parts = scope.split(".") if separator else []
            if (
                not label
                or not separator
                or not scope
                or len(scope_parts) not in (1, 2)
                or any(not part for part in scope_parts)
            ):
                raise ValueError(
                    f"{context} must use canonical I105 account id or account alias `name@dataspace` / `name@domain.dataspace`"
                )
            return literal
        try:
            _decode_i105_string(literal)
        except ValueError as exc:
            raise ValueError(
                f"{context} must be a canonical I105 account id or on-chain account alias"
            ) from exc
        return literal

    @staticmethod
    def _require_exact_i105_account_id(value: Any, context: str) -> str:
        literal = _require_exact_non_empty_string(value, context)
        if any(ch.isspace() for ch in literal) or "@" in literal:
            raise ValueError(f"{context} must be an exact canonical I105 account id")
        try:
            _decode_i105_string(literal)
        except ValueError as exc:
            raise ValueError(f"{context} must be an exact canonical I105 account id") from exc
        return literal

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
    def _normalize_contract_selector(
        *,
        contract_address: Optional[str],
        contract_alias: Optional[str],
        context: str,
    ) -> Dict[str, str]:
        has_contract_address = contract_address is not None
        has_contract_alias = contract_alias is not None
        if has_contract_address == has_contract_alias:
            raise ValueError(
                f"{context} requires exactly one of contract_address or contract_alias"
            )
        if has_contract_address:
            return {
                "contract_address": _require_exact_non_empty_string(
                    contract_address,
                    f"{context}.contract_address",
                )
            }
        return {
            "contract_alias": _require_exact_non_empty_string(
                contract_alias,
                f"{context}.contract_alias",
            )
        }

    @classmethod
    def _normalize_fee_payment_intent(
        cls,
        value: Mapping[str, Any],
        *,
        context: str,
        require_gas_limit: bool = False,
    ) -> Dict[str, Any]:
        if not isinstance(value, Mapping):
            raise TypeError(f"{context} must be an object")
        if set(value) != {"payer", "value"}:
            raise ValueError(f"{context} must contain only payer and value")
        payer = _require_exact_non_empty_string(value.get("payer"), f"{context}.payer")
        if payer not in {"authority", "sponsor"}:
            raise ValueError(f"{context}.payer must be authority or sponsor")
        payment = value.get("value")
        if not isinstance(payment, Mapping):
            raise TypeError(f"{context}.value must be an object")
        allowed = {"charge_limits", "gas_limit"}
        if payer == "sponsor":
            allowed.update({"program_id", "program_revision"})
        if set(payment) - allowed:
            raise ValueError(f"{context}.value contains unsupported fields")
        if "charge_limits" not in payment:
            raise ValueError(f"{context}.value.charge_limits is required")
        charge_limits = payment["charge_limits"]
        if isinstance(charge_limits, (str, bytes, bytearray, memoryview)) or not isinstance(
            charge_limits, Sequence
        ):
            raise TypeError(f"{context}.value.charge_limits must be an array")
        if len(charge_limits) > 2:
            raise ValueError(f"{context}.value.charge_limits contains too many entries")
        normalized_limits: List[Dict[str, Any]] = []
        previous_kind = -1
        for index, raw in enumerate(charge_limits):
            item_context = f"{context}.value.charge_limits[{index}]"
            if not isinstance(raw, Mapping):
                raise TypeError(f"{item_context} must be an object")
            if set(raw) != {"kind", "asset_definition_id", "max_amount"}:
                raise ValueError(f"{item_context} has unsupported or missing fields")
            tagged_kind = raw["kind"]
            if (
                not isinstance(tagged_kind, Mapping)
                or set(tagged_kind) != {"kind", "value"}
                or tagged_kind.get("value") is not None
            ):
                raise ValueError(f"{item_context}.kind must be a canonical tagged unit")
            kind_literal = tagged_kind.get("kind")
            kind = 0 if kind_literal == "nexus" else 1 if kind_literal == "pipeline_gas" else -1
            if kind < 0:
                raise ValueError(f"{item_context}.kind is unsupported")
            if kind <= previous_kind:
                raise ValueError(
                    f"{context}.value.charge_limits must be unique and canonically ordered"
                )
            previous_kind = kind
            asset_definition_id = _offline_canonical_asset_definition_id(
                raw["asset_definition_id"],
                f"{item_context}.asset_definition_id",
            )
            max_amount = cls._quantity(raw["max_amount"], f"{item_context}.max_amount")
            if max_amount == "0":
                raise ValueError(f"{item_context}.max_amount must be positive")
            normalized_limits.append(
                {
                    "kind": {"kind": kind_literal, "value": None},
                    "asset_definition_id": asset_definition_id,
                    "max_amount": max_amount,
                }
            )
        gas_limit = payment.get("gas_limit")
        if gas_limit is not None:
            gas_limit = cls._normalize_optional_int(
                gas_limit,
                f"{context}.value.gas_limit",
                allow_zero=False,
            )
        if require_gas_limit and gas_limit is None:
            raise ValueError(f"{context}.value.gas_limit is required")
        normalized_value: Dict[str, Any] = {
            "charge_limits": normalized_limits,
            "gas_limit": gas_limit,
        }
        if payer == "sponsor":
            program_id = payment.get("program_id")
            if not isinstance(program_id, Mapping) or set(program_id) != {"sponsor", "name"}:
                raise ValueError(f"{context}.value.program_id must contain sponsor and name")
            sponsor = cls._require_exact_i105_account_id(
                program_id.get("sponsor"),
                f"{context}.value.program_id.sponsor",
            )
            name = _require_exact_non_empty_string(
                program_id.get("name"),
                f"{context}.value.program_id.name",
            )
            if (
                unicodedata.normalize("NFC", name) != name
                or any(char.isspace() or ord(char) < 0x20 for char in name)
                or any(char in "@#$/" for char in name)
            ):
                raise ValueError(f"{context}.value.program_id.name must be canonical")
            revision = cls._normalize_optional_int(
                payment.get("program_revision"),
                f"{context}.value.program_revision",
                allow_zero=False,
            )
            if revision is None:
                raise ValueError(f"{context}.value.program_revision is required")
            normalized_value.update(
                {
                    "program_id": {"sponsor": sponsor, "name": name},
                    "program_revision": revision,
                }
            )
        return {"payer": payer, "value": normalized_value}

    @staticmethod
    def _normalize_required_base64_payload(value: Any, context: str) -> str:
        if not isinstance(value, str):
            raise TypeError(f"{context} must be a string")
        literal = value.strip()
        try:
            decoded = base64.b64decode(literal, validate=True)
        except (binascii.Error, ValueError) as exc:
            raise RuntimeError(f"{context} must be a valid base64 payload") from exc
        if not decoded:
            raise RuntimeError(f"{context} must not decode to empty bytes")
        return literal

    @staticmethod
    def _normalize_required_exact_base64_payload(value: Any, context: str) -> str:
        literal = _require_exact_non_empty_string(value, context)
        if any(char.isspace() for char in literal):
            raise ValueError(f"{context} must be exact standard-base64")
        try:
            decoded = base64.b64decode(literal, validate=True)
        except (binascii.Error, ValueError) as exc:
            raise RuntimeError(f"{context} must be a valid base64 payload") from exc
        if not decoded:
            raise RuntimeError(f"{context} must not decode to empty bytes")
        if base64.b64encode(decoded).decode("ascii") != literal:
            raise ValueError(f"{context} must be exact standard-base64")
        return literal

    @staticmethod
    def multisig_instruction_b64(value: Any, *, context: str = "instruction") -> str:
        """Return a base64 native Norito ``InstructionBox`` payload for multisig propose."""

        if isinstance(value, str):
            return ToriiClient._normalize_required_base64_payload(value, context)
        if isinstance(value, (bytes, bytearray, memoryview)):
            raw = bytes(value)
            if not raw:
                raise RuntimeError(f"{context} must not be empty")
            return base64.b64encode(raw).decode("ascii")
        raise TypeError(f"{context} must be bytes-like or base64 text")

    @staticmethod
    def _normalize_optional_base64_payload(
        value: Any,
        *,
        context: str,
    ) -> Optional[str]:
        if value is None:
            return None
        return ToriiClient._normalize_required_base64_payload(value, context)

    @staticmethod
    def _normalize_optional_exact_base64_payload(value: Any, context: str) -> Optional[str]:
        return None if value is None else ToriiClient._normalize_required_exact_base64_payload(value, context)

    @staticmethod
    def _normalize_transaction_response_pair(
        payload_value: Any, signing_value: Any, *, submitted: bool,
        transaction_hash: Optional[str], context: str,
    ) -> Tuple[Optional[str], Optional[str]]:
        payload = ToriiClient._normalize_optional_exact_base64_payload(payload_value, f"{context}.transaction_payload_b64")
        signing = ToriiClient._normalize_optional_exact_base64_payload(signing_value, f"{context}.signing_message_b64")

        if submitted:
            if transaction_hash is None or payload is not None or signing is not None:
                raise RuntimeError(f"{context} submitted response must contain only the final transaction hash")
            return None, None
        if transaction_hash is not None or payload is None or signing is None:
            raise RuntimeError(f"{context} unsigned response must contain exactly one payload and signing-message pair")
        expected = bytearray(hashlib.blake2b(base64.b64decode(payload), digest_size=32).digest())
        expected[-1] |= 1
        signing_bytes = base64.b64decode(signing)
        if len(signing_bytes) != 32 or not secrets.compare_digest(signing_bytes, expected):
            raise RuntimeError(f"{context}.signing_message_b64 must be the exact TransactionPayload hash")
        return payload, signing

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
    def _reject_unknown_fields(
        record: Mapping[str, Any],
        allowed_fields: Iterable[str],
        context: str,
    ) -> None:
        allowed = frozenset(allowed_fields)
        extras = [str(key) for key in record if key not in allowed]
        if extras:
            raise RuntimeError(
                f"{context} contains unsupported fields: {', '.join(sorted(extras))}"
            )

    @classmethod
    def _validate_exact_fields(
        cls,
        record: Mapping[str, Any],
        required_fields: Iterable[str],
        context: str,
    ) -> None:
        required = frozenset(required_fields)
        cls._reject_unknown_fields(record, required, context)
        missing = [field for field in required if field not in record]
        if missing:
            raise RuntimeError(
                f"{context} is missing required fields: {', '.join(sorted(missing))}"
            )

    @staticmethod
    def _require_vpn_enum(value: Any, allowed: Iterable[str], context: str) -> str:
        allowed_values = frozenset(allowed)
        if not isinstance(value, str) or value not in allowed_values:
            raise RuntimeError(
                f"{context} must be one of: {', '.join(sorted(allowed_values))}"
            )
        return value

    @classmethod
    def _require_vpn_profile_exit_classes(cls, value: Any, context: str) -> List[str]:
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list")
        exits = [
            cls._require_vpn_enum(entry, _VPN_EXIT_CLASSES, f"{context}[{index}]")
            for index, entry in enumerate(value)
        ]
        if len(exits) != 3 or len(set(exits)) != 3:
            raise RuntimeError(f"{context} must contain exactly three unique exit classes")
        return exits

    @classmethod
    def _require_vpn_unsigned_range(
        cls,
        value: Any,
        context: str,
        *,
        minimum: int = 0,
        maximum: Optional[int] = None,
    ) -> int:
        result = cls._require_vpn_uint64(value, context)
        if result < minimum:
            raise RuntimeError(f"{context} must be at least {minimum}")
        if maximum is not None and result > maximum:
            raise RuntimeError(f"{context} must be at most {maximum}")
        return result

    @classmethod
    def _require_vpn_unsigned_constant(cls, value: Any, context: str, expected: int) -> int:
        result = cls._require_vpn_uint64(value, context)
        if result != expected:
            raise RuntimeError(f"{context} must equal {expected}")
        return result

    @staticmethod
    def _require_vpn_uint64(value: Any, context: str) -> int:
        if type(value) is not int:
            raise RuntimeError(f"{context} must be a JSON uint64 integer")
        if value < 0 or value > _VPN_UINT64_MAX:
            raise RuntimeError(f"{context} must be between 0 and {_VPN_UINT64_MAX}")
        return value

    @staticmethod
    def _ensure_list(payload: Any, context: str) -> List[Any]:
        if isinstance(payload, list):
            return payload
        raise RuntimeError(f"{context} response must be a JSON array")

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
        cloned = ToriiClient._clone_json_value(value, context=context)
        if not isinstance(cloned, dict):
            raise RuntimeError(f"{context} must be a JSON object")
        return cloned

    @staticmethod
    def _clone_json_value(value: Any, *, context: str) -> Any:
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
        dataspace_catalog = self._parse_dataspace_catalog(
            record.get("dataspace_catalog"),
            context=f"{context}.dataspace_catalog",
        )
        sealed_aliases = self._parse_string_array(
            record.get("lane_governance_sealed_aliases"),
            context=f"{context}.lane_governance_sealed_aliases",
        )
        consensus_caps = self._parse_consensus_caps(record.get("consensus_caps"))
        raw = dict(record)
        return StatusPayload(
            observed_at_ms=self._coerce_int(record.get("observed_at_ms"), f"{context}.observed_at_ms"),
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
            queue_queued=self._coerce_int(record.get("queue_queued"), f"{context}.queue_queued"),
            queue_inflight=self._coerce_int(
                record.get("queue_inflight"),
                f"{context}.queue_inflight",
            ),
            last_block_committed_at_ms=self._coerce_int(
                record.get("last_block_committed_at_ms"),
                f"{context}.last_block_committed_at_ms",
            ),
            last_non_empty_block_committed_at_ms=self._coerce_int(
                record.get("last_non_empty_block_committed_at_ms"),
                f"{context}.last_non_empty_block_committed_at_ms",
            ),
            time_since_last_block_ms=self._coerce_int(
                record.get("time_since_last_block_ms"),
                f"{context}.time_since_last_block_ms",
            ),
            time_since_last_non_empty_block_ms=self._coerce_int(
                record.get("time_since_last_non_empty_block_ms"),
                f"{context}.time_since_last_non_empty_block_ms",
            ),
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
            dataspace_catalog=dataspace_catalog,
            lane_governance_sealed_total=self._coerce_int(
                record.get("lane_governance_sealed_total"),
                f"{context}.lane_governance_sealed_total",
            ),
            lane_governance_sealed_aliases=sealed_aliases,
            raw=raw,
        )

    def _parse_pipeline_preflight(
        self,
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> PipelinePreflight:
        record = self._ensure_mapping(payload, context)
        sumeragi = self._ensure_mapping(record.get("sumeragi"), f"{context}.sumeragi")
        admission = self._ensure_mapping(record.get("admission"), f"{context}.admission")
        block = self._ensure_mapping(record.get("block"), f"{context}.block")
        pipeline = self._ensure_mapping(record.get("pipeline"), f"{context}.pipeline")
        queue = self._ensure_mapping(record.get("queue"), f"{context}.queue")
        fees = self._ensure_mapping(record.get("fees"), f"{context}.fees")
        raw = self._clone_json_payload(record, context=context)

        return PipelinePreflight(
            schema_version=self._coerce_int(record.get("schema_version"), f"{context}.schema_version"),
            chain_height=self._coerce_int(record.get("chain_height"), f"{context}.chain_height"),
            sumeragi=PipelinePreflightSumeragi(
                block_time_ms=self._coerce_int(
                    sumeragi.get("block_time_ms"),
                    f"{context}.sumeragi.block_time_ms",
                ),
                commit_time_ms=self._coerce_int(
                    sumeragi.get("commit_time_ms"),
                    f"{context}.sumeragi.commit_time_ms",
                ),
                stall_threshold_ms=self._coerce_int(
                    sumeragi.get("stall_threshold_ms"),
                    f"{context}.sumeragi.stall_threshold_ms",
                ),
            ),
            admission=PipelinePreflightAdmission(
                max_signatures=self._coerce_int(
                    admission.get("max_signatures"),
                    f"{context}.admission.max_signatures",
                ),
                max_instructions=self._coerce_int(
                    admission.get("max_instructions"),
                    f"{context}.admission.max_instructions",
                ),
                max_tx_bytes=self._coerce_int(
                    admission.get("max_tx_bytes"),
                    f"{context}.admission.max_tx_bytes",
                ),
                max_decompressed_bytes=self._coerce_int(
                    admission.get("max_decompressed_bytes"),
                    f"{context}.admission.max_decompressed_bytes",
                ),
                max_metadata_depth=self._coerce_int(
                    admission.get("max_metadata_depth"),
                    f"{context}.admission.max_metadata_depth",
                ),
            ),
            block=PipelinePreflightBlock(
                max_transactions=self._coerce_int(
                    block.get("max_transactions"),
                    f"{context}.block.max_transactions",
                )
            ),
            pipeline=PipelinePreflightPipeline(
                signature_batch_max=self._coerce_int(
                    pipeline.get("signature_batch_max"),
                    f"{context}.pipeline.signature_batch_max",
                ),
                signature_batch_max_ed25519=self._coerce_int(
                    pipeline.get("signature_batch_max_ed25519"),
                    f"{context}.pipeline.signature_batch_max_ed25519",
                ),
                signature_batch_max_secp256k1=self._coerce_int(
                    pipeline.get("signature_batch_max_secp256k1"),
                    f"{context}.pipeline.signature_batch_max_secp256k1",
                ),
                signature_batch_max_pqc=self._coerce_int(
                    pipeline.get("signature_batch_max_pqc"),
                    f"{context}.pipeline.signature_batch_max_pqc",
                ),
                signature_batch_max_bls=self._coerce_int(
                    pipeline.get("signature_batch_max_bls"),
                    f"{context}.pipeline.signature_batch_max_bls",
                ),
                overlay_max_instructions=self._coerce_int(
                    pipeline.get("overlay_max_instructions"),
                    f"{context}.pipeline.overlay_max_instructions",
                ),
                ivm_max_decoded_instructions=self._coerce_int(
                    pipeline.get("ivm_max_decoded_instructions"),
                    f"{context}.pipeline.ivm_max_decoded_instructions",
                ),
            ),
            queue=PipelinePreflightQueue(
                size=self._coerce_int(queue.get("size"), f"{context}.queue.size"),
                queued=self._coerce_int(queue.get("queued"), f"{context}.queue.queued"),
                inflight=self._coerce_int(queue.get("inflight"), f"{context}.queue.inflight"),
            ),
            fees=PipelinePreflightFees(
                fee_asset_id=str(fees.get("fee_asset_id") or ""),
                fee_sink_account_id=str(fees.get("fee_sink_account_id") or ""),
                base_fee=self._clone_json_value(fees.get("base_fee"), context=f"{context}.fees.base_fee"),
                per_byte_fee=self._clone_json_value(
                    fees.get("per_byte_fee"),
                    context=f"{context}.fees.per_byte_fee",
                ),
                per_instruction_fee=self._clone_json_value(
                    fees.get("per_instruction_fee"),
                    context=f"{context}.fees.per_instruction_fee",
                ),
                per_gas_unit_fee=self._clone_json_value(
                    fees.get("per_gas_unit_fee"),
                    context=f"{context}.fees.per_gas_unit_fee",
                ),
                sponsor_vault_custody_account_id=str(
                    fees.get("sponsor_vault_custody_account_id") or ""
                ),
                settlement_mode=str(fees.get("settlement_mode") or ""),
                successful_claim_fee_exempt_authorities=self._parse_string_array(
                    fees.get("successful_claim_fee_exempt_authorities"),
                    context=f"{context}.fees.successful_claim_fee_exempt_authorities",
                ),
            ),
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
            contract_address = (
                ""
                if record.get("contract_address") is None
                else str(record.get("contract_address"))
            )
            code_hash = "" if record.get("code_hash_hex") is None else str(record.get("code_hash_hex"))
            abi_hash = (
                None
                if record.get("abi_hash_hex") is None
                else str(record.get("abi_hash_hex"))
            )
            activations.append(
                GovernanceManifestActivation(
                    contract_address=contract_address,
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

    def _parse_dataspace_catalog(
        self,
        payload: Any,
        *,
        context: str,
    ) -> List[DataspaceCatalogEntry]:
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise RuntimeError(f"{context} must be a list")
        entries: List[DataspaceCatalogEntry] = []
        for index, entry in enumerate(payload):
            record = self._ensure_mapping(entry, f"{context}[{index}]")
            manifest_required = self._coerce_bool(
                record.get("manifest_required"),
                f"{context}[{index}].manifest_required",
            )
            manifest_ready = self._coerce_bool(
                record.get("manifest_ready"),
                f"{context}[{index}].manifest_ready",
            )
            sealed_value = record.get("sealed")
            if sealed_value is None:
                sealed = manifest_required and not manifest_ready
            else:
                sealed = self._coerce_bool(sealed_value, f"{context}[{index}].sealed")
            entries.append(
                DataspaceCatalogEntry(
                    lane_id=self._coerce_int(record.get("lane_id"), f"{context}[{index}].lane_id"),
                    lane_alias="" if record.get("lane_alias") is None else str(record.get("lane_alias")),
                    dataspace_id=self._coerce_int(record.get("dataspace_id"), f"{context}[{index}].dataspace_id"),
                    alias="" if record.get("alias") is None else str(record.get("alias")),
                    visibility="" if record.get("visibility") is None else str(record.get("visibility")),
                    storage_profile="" if record.get("storage_profile") is None else str(record.get("storage_profile")),
                    manifest_required=manifest_required,
                    manifest_ready=manifest_ready,
                    sealed=sealed,
                    manifest_path=None if record.get("manifest_path") is None else str(record.get("manifest_path")),
                    protected_namespaces=self._parse_string_array(
                        record.get("protected_namespaces"),
                        context=f"{context}[{index}].protected_namespaces",
                    ),
                )
            )
        return entries

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
            sequence_violation_closes_total=ToriiClient._coerce_int(
                record.get("sequence_violation_closes_total"),
                f"{context}.sequence_violation_closes_total",
            ),
            role_direction_mismatch_total=ToriiClient._coerce_int(
                record.get("role_direction_mismatch_total"),
                f"{context}.role_direction_mismatch_total",
            ),
            ping_miss_total=ToriiClient._coerce_int(record.get("ping_miss_total"), f"{context}.ping_miss_total"),
            p2p_rebroadcasts_total=ToriiClient._coerce_int(
                record.get("p2p_rebroadcasts_total"),
                f"{context}.p2p_rebroadcasts_total",
            ),
            p2p_rebroadcast_skipped_total=ToriiClient._coerce_int(
                record.get("p2p_rebroadcast_skipped_total"),
                f"{context}.p2p_rebroadcast_skipped_total",
            ),
            p2p_auth_failures_total=ToriiClient._coerce_int(
                record.get("p2p_auth_failures_total"),
                f"{context}.p2p_auth_failures_total",
            ),
            p2p_ttl_drops_total=ToriiClient._coerce_int(
                record.get("p2p_ttl_drops_total"),
                f"{context}.p2p_ttl_drops_total",
            ),
            p2p_unknown_session_drops_total=ToriiClient._coerce_int(
                record.get("p2p_unknown_session_drops_total"),
                f"{context}.p2p_unknown_session_drops_total",
            ),
            p2p_session_claims_in_total=ToriiClient._coerce_int(
                record.get("p2p_session_claims_in_total"),
                f"{context}.p2p_session_claims_in_total",
            ),
            p2p_session_claims_installed_total=ToriiClient._coerce_int(
                record.get("p2p_session_claims_installed_total"),
                f"{context}.p2p_session_claims_installed_total",
            ),
            p2p_session_claim_conflicts_total=ToriiClient._coerce_int(
                record.get("p2p_session_claim_conflicts_total"),
                f"{context}.p2p_session_claim_conflicts_total",
            ),
            p2p_role_consumed_total=ToriiClient._coerce_int(
                record.get("p2p_role_consumed_total"),
                f"{context}.p2p_role_consumed_total",
            ),
            p2p_session_terminated_total=ToriiClient._coerce_int(
                record.get("p2p_session_terminated_total"),
                f"{context}.p2p_session_terminated_total",
            ),
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
            relay_strategy=ToriiClient._optional_string(record.get("relay_strategy"), f"{context}.relay_strategy"),
            relay_effective_strategy=ToriiClient._optional_string(
                record.get("relay_effective_strategy"),
                f"{context}.relay_effective_strategy",
            ),
            relay_p2p_attached=ToriiClient._optional_bool(
                record.get("relay_p2p_attached"),
                f"{context}.relay_p2p_attached",
            ),
            p2p_ttl_hops=ToriiClient._coerce_optional_unsigned(
                record.get("p2p_ttl_hops"),
                context=f"{context}.p2p_ttl_hops",
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
                    "relay_strategy",
                    "relay_effective_strategy",
                    "relay_p2p_attached",
                    "p2p_ttl_hops",
                    "heartbeat_interval_ms",
                    "heartbeat_miss_tolerance",
                    "heartbeat_min_interval_ms",
                }
            },
        )

    @staticmethod
    def _parse_connect_session(payload: Mapping[str, Any], *, context: str) -> ConnectSessionInfo:
        return _connect_session.parse_connect_session(
            payload,
            context=context,
            hash_literal=_offline_hash_literal,
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

        kind_value = record.get("kind")
        kind = ToriiClient._require_non_empty_string(kind_value, f"{context}.kind")
        if kind != kind_value:
            raise RuntimeError(f"{context}.kind must not contain surrounding whitespace")
        common_fields = {
            "kind",
            "recorded_height",
            "recorded_view",
            "recorded_ms",
            "consensus_admitted_height",
        }
        required_variant_fields: set[str]
        optional_variant_fields: set[str] = set()
        if kind in {"DoublePrepare", "DoubleCommit"}:
            required_variant_fields = {
                "phase",
                "height",
                "view",
                "epoch",
                "signer",
                "block_hash_1",
                "block_hash_2",
            }
        elif kind == "InvalidQc":
            required_variant_fields = {
                "height",
                "view",
                "epoch",
                "subject_block_hash",
                "phase",
                "reason",
            }
        elif kind == "InvalidProposal":
            required_variant_fields = {
                "height",
                "view",
                "epoch",
                "subject_block_hash",
                "payload_hash",
                "reason",
            }
        elif kind == "Censorship":
            required_variant_fields = {"tx_hash", "receipt_count", "signers"}
            optional_variant_fields = {
                "submitted_at_height_min",
                "submitted_at_height_max",
            }
        elif kind == "SumeragiV2Equivocation":
            required_variant_fields = {
                "class",
                "height",
                "view",
                "epoch",
                "signer",
                "context_id",
                "artifact_hash_1",
                "artifact_hash_2",
            }
        else:
            allowed = ", ".join(sorted(SUMERAGI_EVIDENCE_KIND_FILTERS))
            raise RuntimeError(f"{context}.kind must be one of: {allowed}")

        required_fields = common_fields | required_variant_fields
        allowed_fields = required_fields | optional_variant_fields
        actual_fields = set(record)
        missing = sorted(required_fields - actual_fields)
        unexpected = sorted(str(field) for field in actual_fields - allowed_fields)
        if missing or unexpected:
            details = []
            if missing:
                details.append(f"missing {', '.join(missing)}")
            if unexpected:
                details.append(f"unexpected {', '.join(unexpected)}")
            raise RuntimeError(
                f"{context} must use the exact server fields ({'; '.join(details)})"
            )

        def json_unsigned(field: str, *, maximum: Optional[int] = None) -> int:
            value = record[field]
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise RuntimeError(f"{context}.{field} must be a non-negative JSON integer")
            if maximum is not None and value > maximum:
                raise RuntimeError(f"{context}.{field} must be <= {maximum}")
            return value

        def evidence_hash(field: str) -> str:
            return ToriiClient._require_exact_lower_hex_string(
                record[field],
                context=f"{context}.{field}",
                expected_length=64,
            )

        def exact_non_empty_string_for_value(value: Any, field_context: str) -> str:
            literal = ToriiClient._require_non_empty_string(value, field_context)
            if literal != value:
                raise RuntimeError(
                    f"{field_context} must not contain surrounding whitespace"
                )
            return literal

        def exact_non_empty_string(field: str) -> str:
            return exact_non_empty_string_for_value(
                record[field], f"{context}.{field}"
            )

        admitted_value = record["consensus_admitted_height"]
        consensus_admitted_height = (
            None
            if admitted_value is None
            else json_unsigned("consensus_admitted_height")
        )
        common = {
            "kind": kind,
            "recorded_height": json_unsigned("recorded_height"),
            "recorded_view": json_unsigned("recorded_view"),
            "recorded_ms": json_unsigned("recorded_ms"),
            "consensus_admitted_height": consensus_admitted_height,
        }

        if kind in {"DoublePrepare", "DoubleCommit"}:
            phase = exact_non_empty_string("phase")
            if phase not in SUMERAGI_EVIDENCE_PHASES:
                allowed = ", ".join(sorted(SUMERAGI_EVIDENCE_PHASES))
                raise RuntimeError(f"{context}.phase must be one of: {allowed}")
            block_hash_1 = evidence_hash("block_hash_1")
            block_hash_2 = evidence_hash("block_hash_2")
            if block_hash_1 == block_hash_2:
                raise RuntimeError(f"{context} block hashes must identify distinct blocks")
            return SumeragiDoubleVoteEvidenceRecord(
                **common,
                phase=cast(Literal["Prepare", "Commit", "NewView"], phase),
                height=json_unsigned("height"),
                view=json_unsigned("view"),
                epoch=json_unsigned("epoch"),
                signer=json_unsigned("signer", maximum=0xFFFFFFFF),
                block_hash_1=block_hash_1,
                block_hash_2=block_hash_2,
            )
        if kind == "InvalidQc":
            phase = exact_non_empty_string("phase")
            if phase not in SUMERAGI_EVIDENCE_PHASES:
                allowed = ", ".join(sorted(SUMERAGI_EVIDENCE_PHASES))
                raise RuntimeError(f"{context}.phase must be one of: {allowed}")
            return SumeragiInvalidQcEvidenceRecord(
                **common,
                height=json_unsigned("height"),
                view=json_unsigned("view"),
                epoch=json_unsigned("epoch"),
                subject_block_hash=evidence_hash("subject_block_hash"),
                phase=cast(Literal["Prepare", "Commit", "NewView"], phase),
                reason=exact_non_empty_string("reason"),
            )
        if kind == "InvalidProposal":
            return SumeragiInvalidProposalEvidenceRecord(
                **common,
                height=json_unsigned("height"),
                view=json_unsigned("view"),
                epoch=json_unsigned("epoch"),
                subject_block_hash=evidence_hash("subject_block_hash"),
                payload_hash=evidence_hash("payload_hash"),
                reason=exact_non_empty_string("reason"),
            )
        if kind == "Censorship":
            signers_value = record["signers"]
            if not isinstance(signers_value, list):
                raise RuntimeError(f"{context}.signers must be a JSON array")
            signers = [
                exact_non_empty_string_for_value(
                    signer, f"{context}.signers[{index}]"
                )
                for index, signer in enumerate(signers_value)
            ]
            receipt_count = json_unsigned("receipt_count")
            if receipt_count != len(signers):
                raise RuntimeError(f"{context}.receipt_count must equal len(signers)")
            has_min = "submitted_at_height_min" in record
            has_max = "submitted_at_height_max" in record
            if (
                has_min != has_max
                or (receipt_count > 0 and not has_min)
                or (receipt_count == 0 and has_min)
            ):
                raise RuntimeError(
                    f"{context} must include both submitted_at_height bounds "
                    "exactly when receipts are present"
                )
            submitted_at_height_min = (
                json_unsigned("submitted_at_height_min") if has_min else None
            )
            submitted_at_height_max = (
                json_unsigned("submitted_at_height_max") if has_max else None
            )
            if (
                submitted_at_height_min is not None
                and submitted_at_height_max is not None
                and submitted_at_height_min > submitted_at_height_max
            ):
                raise RuntimeError(
                    f"{context}.submitted_at_height_min must be <= "
                    "submitted_at_height_max"
                )
            return SumeragiCensorshipEvidenceRecord(
                **common,
                tx_hash=evidence_hash("tx_hash"),
                receipt_count=receipt_count,
                signers=signers,
                submitted_at_height_min=submitted_at_height_min,
                submitted_at_height_max=submitted_at_height_max,
            )

        evidence_class = exact_non_empty_string("class")
        if evidence_class not in SUMERAGI_EVIDENCE_EQUIVOCATION_CLASSES:
            allowed = ", ".join(sorted(SUMERAGI_EVIDENCE_EQUIVOCATION_CLASSES))
            raise RuntimeError(f"{context}.class must be one of: {allowed}")
        artifact_hash_1 = evidence_hash("artifact_hash_1")
        artifact_hash_2 = evidence_hash("artifact_hash_2")
        if artifact_hash_1 == artifact_hash_2:
            raise RuntimeError(f"{context} artifact hashes must identify distinct artifacts")
        return SumeragiV2EquivocationEvidenceRecord(
            **common,
            class_=cast(
                Literal["proposal", "phase_vote", "timeout_vote"], evidence_class
            ),
            height=json_unsigned("height"),
            view=json_unsigned("view"),
            epoch=json_unsigned("epoch"),
            signer=json_unsigned("signer", maximum=0xFFFFFFFF),
            context_id=evidence_hash("context_id"),
            artifact_hash_1=artifact_hash_1,
            artifact_hash_2=artifact_hash_2,
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
    def _coerce_unsigned(value: Any, context: str) -> int:
        result = ToriiClient._coerce_int(value, context)
        if result < 0:
            raise RuntimeError(f"{context} must be non-negative")
        return result

    @staticmethod
    def _quantity(value: Any, context: str) -> str:
        return _canonical_quantity(value, context)

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
    def _require_exact_lower_hex_string(
        value: Any,
        *,
        context: str,
        expected_length: int,
    ) -> str:
        if (
            not isinstance(value, str)
            or len(value) != expected_length
            or re.fullmatch(r"[0-9a-f]+", value) is None
        ):
            raise RuntimeError(
                f"{context} must be an exact lowercase {expected_length // 2}-byte hex string"
            )
        return value

    _require_vpn_relay_id = staticmethod(_vpn_require_relay_id)
    _require_vpn_trust_digest = staticmethod(_vpn_require_trust_digest)
    _require_vpn_tls_server_name = staticmethod(_vpn_require_tls_server_name)
    _require_vpn_relay_endpoint = staticmethod(_vpn_require_relay_endpoint)
    _normalize_vpn_canonical_hex_input = staticmethod(
        _vpn_normalize_canonical_hex_input
    )

    @staticmethod
    def _require_exact_lower_even_hex_string(value: Any, *, context: str) -> str:
        if (
            not isinstance(value, str)
            or not value
            or len(value) % 2 != 0
            or re.fullmatch(r"[0-9a-f]+", value) is None
        ):
            raise RuntimeError(
                f"{context} must be an exact lowercase even-length hex string"
            )
        return value

    @staticmethod
    def _require_vpn_helper_ticket_hex(value: Any, *, context: str) -> str:
        if (
            not isinstance(value, str)
            or len(value) != _VPN_HELPER_TICKET_HEX_LENGTH
            or re.fullmatch(r"[0-9a-f]+", value) is None
        ):
            raise RuntimeError(
                f"{context} must contain exactly {_VPN_HELPER_TICKET_HEX_LENGTH} lowercase "
                f"hexadecimal characters ({_VPN_HELPER_TICKET_BYTES} bytes)"
            )
        return value

    @staticmethod
    def _require_exact_inline_hex_string(value: Any, *, context: str) -> None:
        if not isinstance(value, str):
            return
        if value.strip() != value:
            raise RuntimeError(f"{context} must be canonical hex")
        literal = value[2:] if value.startswith(("0x", "0X")) else value
        if any(character.isspace() for character in literal):
            raise RuntimeError(f"{context} must be canonical hex")

    @classmethod
    def _normalize_nonzero_hex_bytes(
        cls,
        value: Union[str, bytes, bytearray, memoryview],
        *,
        context: str,
        expected_byte_length: Optional[int] = None,
    ) -> str:
        normalized = cls._normalize_hex_string(value, context=context)
        if not any(bytes.fromhex(normalized)):
            raise RuntimeError(f"{context} must not be all zero")
        if expected_byte_length is not None and len(normalized) != expected_byte_length * 2:
            raise RuntimeError(f"{context} must be a {expected_byte_length}-byte hex string")
        return normalized

    @classmethod
    def _normalize_exact_nonzero_hex_bytes(
        cls,
        value: Union[str, bytes, bytearray, memoryview],
        *,
        context: str,
        expected_byte_length: Optional[int] = None,
    ) -> str:
        cls._require_exact_inline_hex_string(value, context=context)
        return cls._normalize_nonzero_hex_bytes(
            value,
            context=context,
            expected_byte_length=expected_byte_length,
        )

    @staticmethod
    def _normalize_uaid_literal(value: Any, *, context: str) -> str:
        if not isinstance(value, str):
            raise RuntimeError(f"{context} must be a UAID string")
        literal = value
        stripped = literal.strip()
        if not stripped:
            raise RuntimeError(f"{context} must be a UAID string")
        if stripped != literal:
            raise ValueError(f"{context} must not contain surrounding whitespace")
        if literal.lower().startswith("uaid:"):
            hex_portion = literal[5:]
        else:
            hex_portion = literal
        normalized = hex_portion
        if normalized.strip() != normalized:
            raise ValueError(f"{context} must not contain surrounding whitespace")
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
    def _require_governance_selector_v1(value: Any, *, context: str) -> str:
        if not isinstance(value, str):
            raise RuntimeError(f"{context} must be a string")
        try:
            encoded = value.encode("ascii")
        except UnicodeEncodeError as exc:
            raise RuntimeError(
                f"{context} must be a canonical governance selector V1"
            ) from exc
        if not encoded or len(encoded) > 128 or encoded[0] == ord("."):
            raise RuntimeError(
                f"{context} must be a canonical governance selector V1"
            )
        if any(
            not (
                ord("A") <= byte <= ord("Z")
                or ord("a") <= byte <= ord("z")
                or ord("0") <= byte <= ord("9")
                or byte in b"-._~"
            )
            for byte in encoded
        ):
            raise RuntimeError(
                f"{context} must be a canonical governance selector V1"
            )
        return value

    @classmethod
    def _require_governance_proposal_id_v1(cls, value: Any, *, context: str) -> str:
        return cls._require_exact_lower_hex_string(
            value,
            context=context,
            expected_length=64,
        )

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
            raise RuntimeError(f"{context}.owner must be a canonical I105 account id")
        trimmed = owner.strip()
        if not trimmed or trimmed != owner:
            raise RuntimeError(f"{context}.owner must be a canonical I105 account id")
        if any(ch.isspace() for ch in trimmed):
            raise RuntimeError(f"{context}.owner must be a canonical I105 account id")
        if "@" in trimmed:
            raise RuntimeError(f"{context}.owner must be a canonical I105 account id")
        if trimmed.lower().startswith("0x"):
            raise RuntimeError(f"{context}.owner must be a canonical I105 account id")
        try:
            _decode_i105_string(trimmed)
        except ValueError as exc:
            raise RuntimeError(f"{context}.owner must be a canonical I105 account id") from exc

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
    def _parse_mapping_list(value: Any, *, context: str) -> List[Mapping[str, Any]]:
        if value is None:
            return []
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list")
        return [ToriiClient._ensure_mapping(entry, f"{context}[{index}]") for index, entry in enumerate(value)]

    @staticmethod
    def _require_choice(value: Any, *, allowed: set[str], context: str) -> str:
        literal = ToriiClient._require_string(value, context)
        if literal not in allowed:
            allowed_sorted = ", ".join(sorted(allowed))
            raise RuntimeError(f"{context} must be one of: {allowed_sorted}")
        return literal

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
    def _parse_pipeline_status_response(
        payload: Any,
        *,
        context: str,
    ) -> PipelineTransactionStatusResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        ToriiClient._validate_exact_fields(
            record,
            {"hash", "status", "scope", "resolved_from"},
            context,
        )
        status_record = ToriiClient._ensure_mapping(record.get("status"), f"{context}.status")
        ToriiClient._reject_unknown_fields(
            status_record,
            {"kind", "block_height"},
            f"{context}.status",
        )
        if "kind" not in status_record:
            raise RuntimeError(f"{context}.status is missing required field kind")
        status_kind = ToriiClient._require_non_empty_string(
            status_record.get("kind"),
            f"{context}.status.kind",
        )
        if status_kind not in {"Queued", "Approved", "Committed", "Applied", "Rejected", "Expired"}:
            raise RuntimeError(f"{context}.status.kind is unsupported")
        scope = ToriiClient._require_non_empty_string(record.get("scope"), f"{context}.scope")
        if scope not in {"local", "auto", "global"}:
            raise RuntimeError(f"{context}.scope is unsupported")
        resolved_from = ToriiClient._require_non_empty_string(
            record.get("resolved_from"),
            f"{context}.resolved_from",
        )
        if resolved_from not in {"cache", "queue", "state"}:
            raise RuntimeError(f"{context}.resolved_from is unsupported")
        block_height = None
        if "block_height" in status_record:
            raw_block_height = status_record["block_height"]
            if (
                isinstance(raw_block_height, bool)
                or not isinstance(raw_block_height, int)
                or raw_block_height <= 0
            ):
                raise RuntimeError(
                    f"{context}.status.block_height must be a positive integer"
                )
            block_height = raw_block_height
        return PipelineTransactionStatusResponse(
            hash=ToriiClient._require_exact_lower_hex_string(
                record.get("hash"),
                context=f"{context}.hash",
                expected_length=64,
            ),
            status=PipelineTransactionStatus(
                kind=status_kind,
                block_height=block_height,
            ),
            scope=scope,
            resolved_from=resolved_from,
        )

    @staticmethod
    def _parse_optional_pipeline_status_response(
        payload: Any,
        *,
        context: str,
    ) -> Optional[PipelineTransactionStatusResponse]:
        if payload is None:
            return None
        return ToriiClient._parse_pipeline_status_response(payload, context=context)


    @staticmethod
    def _parse_contract_call_response(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> ContractCallResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        supported_fields = {
            "ok",
            "submitted",
            "dataspace",
            "contract_address",
            "code_hash_hex",
            "abi_hash_hex",
            "creation_time_ms",
            "transaction_ttl_ms",
            "tx_hash_hex",
            "pipeline_status",
            "entrypoint_hash_hex",
            "transaction_payload_b64",
            "signing_message_b64",
            "entrypoint",
            "operation_receipt",
        }
        unknown_fields = set(record) - supported_fields
        if unknown_fields:
            raise RuntimeError(
                f"{context} contains unsupported fields: "
                f"{', '.join(sorted(unknown_fields))}"
            )
        ok_value = record.get("ok")
        submitted_value = record.get("submitted")
        if not isinstance(ok_value, bool):
            raise RuntimeError(f"{context}.ok must be a boolean")
        if not isinstance(submitted_value, bool):
            raise RuntimeError(f"{context}.submitted must be a boolean")
        tx_hash_hex_value = record.get("tx_hash_hex")
        tx_hash_hex = None
        if tx_hash_hex_value is not None:
            tx_hash_hex = ToriiClient._normalize_hex_string(
                tx_hash_hex_value,
                context=f"{context}.tx_hash_hex",
                expected_length=64,
            )
        entrypoint_hash_hex_value = record.get("entrypoint_hash_hex")
        entrypoint_hash_hex = None
        if entrypoint_hash_hex_value is not None:
            entrypoint_hash_hex = ToriiClient._normalize_hex_string(
                entrypoint_hash_hex_value,
                context=f"{context}.entrypoint_hash_hex",
                expected_length=64,
            )
        operation_receipt = ToriiClient._parse_contract_operation_receipt(
            ToriiClient._ensure_mapping(
                record.get("operation_receipt"),
                f"{context}.operation_receipt",
            ),
            context=f"{context}.operation_receipt",
        )
        return ContractCallResponse(
            ok=ok_value,
            submitted=submitted_value,
            dataspace=ToriiClient._require_string(
                record.get("dataspace"),
                f"{context}.dataspace",
            ),
            code_hash_hex=ToriiClient._normalize_hex_string(
                record.get("code_hash_hex"),
                context=f"{context}.code_hash_hex",
                expected_length=64,
            ),
            abi_hash_hex=ToriiClient._normalize_hex_string(
                record.get("abi_hash_hex"),
                context=f"{context}.abi_hash_hex",
                expected_length=64,
            ),
            creation_time_ms=ToriiClient._coerce_unsigned(
                record.get("creation_time_ms"),
                f"{context}.creation_time_ms",
            ),
            contract_address=ToriiClient._coerce_optional_string(
                record.get("contract_address"),
                context=f"{context}.contract_address",
            ),
            tx_hash_hex=tx_hash_hex,
            pipeline_status=ToriiClient._parse_optional_pipeline_status_response(
                record.get("pipeline_status"),
                context=f"{context}.pipeline_status",
            ),
            entrypoint=ToriiClient._coerce_optional_string(
                record.get("entrypoint"),
                context=f"{context}.entrypoint",
            ),
            transaction_ttl_ms=ToriiClient._coerce_optional_unsigned(
                record.get("transaction_ttl_ms"),
                context=f"{context}.transaction_ttl_ms",
            ),
            entrypoint_hash_hex=entrypoint_hash_hex,
            transaction_payload_b64=ToriiClient._normalize_optional_exact_base64_payload(
                record.get("transaction_payload_b64"), f"{context}.transaction_payload_b64"
            ),
            signing_message_b64=ToriiClient._normalize_optional_exact_base64_payload(
                record.get("signing_message_b64"), f"{context}.signing_message_b64"
            ),
            operation_receipt=operation_receipt,
        )

    @staticmethod
    def _validate_contract_call_draft(
        response: ContractCallResponse,
        *,
        entrypoint: str,
        contract_address: Optional[str],
        contract_alias: Optional[str],
    ) -> None:
        """Fail closed unless a contract response is the requested unsigned draft."""

        receipt = response.operation_receipt
        if not response.ok or response.submitted:
            raise RuntimeError(
                "contract call response must be a successful unsubmitted draft"
            )
        if response.entrypoint_hash_hex is not None or response.pipeline_status is not None:
            raise RuntimeError(
                "contract call draft must not contain transaction submission state"
            )
        ToriiClient._normalize_transaction_response_pair(
            response.transaction_payload_b64,
            response.signing_message_b64,
            submitted=response.submitted,
            transaction_hash=response.tx_hash_hex,
            context="contract call draft",
        )
        if (
            response.entrypoint != entrypoint
            or receipt.operation_kind != "contract_call"
            or receipt.status != "pending_signature"
            or receipt.entrypoint != entrypoint
            or receipt.tx_hash_hex is not None
            or receipt.entrypoint_hash_hex is not None
        ):
            raise RuntimeError(
                "contract call draft is not bound to the requested entrypoint"
            )
        if contract_address is not None and (
            response.contract_address != contract_address
            or receipt.contract_address != contract_address
        ):
            raise RuntimeError(
                "contract call draft is not bound to the requested contract address"
            )
        if (
            contract_alias is not None
            and receipt.contract_alias != contract_alias
        ):
            raise RuntimeError(
                "contract call draft is not bound to the requested contract alias"
            )

    @staticmethod
    def _parse_contract_operation_receipt(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> ContractOperationReceipt:
        record = ToriiClient._ensure_mapping(payload, context)

        def optional_hash(field: str) -> Optional[str]:
            value = record.get(field)
            if value is None:
                return None
            return ToriiClient._normalize_hex_string(
                value,
                context=f"{context}.{field}",
                expected_length=64,
            )

        gas_limit = ToriiClient._coerce_optional_unsigned(
            record.get("gas_limit"),
            context=f"{context}.gas_limit",
        )
        if gas_limit is not None and gas_limit == 0:
            raise RuntimeError(f"{context}.gas_limit must be positive")

        return ContractOperationReceipt(
            operation_kind=ToriiClient._require_non_empty_string(
                record.get("operation_kind"),
                f"{context}.operation_kind",
            ),
            status=ToriiClient._require_non_empty_string(
                record.get("status"),
                f"{context}.status",
            ),
            transport=ToriiClient._require_non_empty_string(
                record.get("transport"),
                f"{context}.transport",
            ),
            dataspace=ToriiClient._require_non_empty_string(
                record.get("dataspace"),
                f"{context}.dataspace",
            ),
            contract_alias=ToriiClient._coerce_optional_string(
                record.get("contract_alias"),
                context=f"{context}.contract_alias",
            ),
            contract_address=ToriiClient._coerce_optional_string(
                record.get("contract_address"),
                context=f"{context}.contract_address",
            ),
            code_hash_hex=optional_hash("code_hash_hex"),
            abi_hash_hex=optional_hash("abi_hash_hex"),
            tx_hash_hex=optional_hash("tx_hash_hex"),
            entrypoint=ToriiClient._coerce_optional_string(
                record.get("entrypoint"),
                context=f"{context}.entrypoint",
            ),
            entrypoint_hash_hex=optional_hash("entrypoint_hash_hex"),
            gas_limit=gas_limit,
            gas_used=ToriiClient._coerce_optional_unsigned(
                record.get("gas_used"),
                context=f"{context}.gas_used",
            ),
            fee_payment=(
                ToriiClient._normalize_fee_payment_intent(
                    ToriiClient._ensure_mapping(
                        record.get("fee_payment"), f"{context}.fee_payment"
                    ),
                    context=f"{context}.fee_payment",
                    require_gas_limit=True,
                )
                if record.get("fee_payment") is not None
                else None
            ),
            payload_digest_hex=ToriiClient._normalize_hex_string(
                record.get("payload_digest_hex"),
                context=f"{context}.payload_digest_hex",
                expected_length=64,
            ),
        )

    @staticmethod
    def _parse_multisig_response(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> MultisigResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        if record.get("ok") is not True:
            raise RuntimeError(f"{context}.ok must be true")
        for retired in ("transaction_scaffold_b64", "signed_transaction_b64"):
            if retired in record:
                raise RuntimeError(f"{context} contains retired field `{retired}`")
        resolved_multisig_account_id = ToriiClient._require_exact_i105_account_id(
            record.get("resolved_multisig_account_id"),
            f"{context}.resolved_multisig_account_id",
        )

        def optional_hash(field: str) -> Optional[str]:
            raw = record.get(field)
            return None if raw is None else ToriiClient._normalize_hex_string(
                raw, context=f"{context}.{field}", expected_length=64
            )

        creation_raw = record.get("creation_time_ms")
        creation_time_ms = None if creation_raw is None else ToriiClient._coerce_unsigned(
            creation_raw, f"{context}.creation_time_ms"
        )
        instructions_hash = optional_hash("instructions_hash")
        tx_hash_hex = optional_hash("tx_hash_hex")
        executed_tx_hash_hex = optional_hash("executed_tx_hash_hex")
        submitted = record.get("submitted")
        if not isinstance(submitted, bool):
            raise TypeError(f"{context}.submitted must be a boolean")
        transaction_payload_b64, signing_message_b64 = ToriiClient._normalize_transaction_response_pair(
            record.get("transaction_payload_b64"),
            record.get("signing_message_b64"),
            submitted=submitted,
            transaction_hash=tx_hash_hex,
            context=context,
        )
        return MultisigResponse(
            ok=True,
            resolved_multisig_account_id=resolved_multisig_account_id,
            submitted=submitted,
            proposal_id=ToriiClient._coerce_optional_string(
                record.get("proposal_id"),
                context=f"{context}.proposal_id",
            ),
            instructions_hash=instructions_hash,
            tx_hash_hex=tx_hash_hex,
            executed_tx_hash_hex=executed_tx_hash_hex,
            creation_time_ms=creation_time_ms,
            transaction_payload_b64=transaction_payload_b64,
            signing_message_b64=signing_message_b64,
        )

    @staticmethod
    def _parse_governance_contract_response(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> GovernanceContractResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        code_hash_hex_value = record.get("code_hash_hex")
        code_hash_hex = None
        if code_hash_hex_value is not None:
            code_hash_hex = ToriiClient._normalize_hex_string(
                code_hash_hex_value,
                context=f"{context}.code_hash_hex",
                expected_length=64,
            )
        return GovernanceContractResponse(
            found=bool(record.get("found")),
            contract_address=ToriiClient._require_string(
                record.get("contract_address"),
                f"{context}.contract_address",
            ),
            dataspace=ToriiClient._coerce_optional_string(
                record.get("dataspace"),
                context=f"{context}.dataspace",
            ),
            code_hash_hex=code_hash_hex,
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
    return StatusMetrics(**_compute_status_metric_values(previous, current))


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
