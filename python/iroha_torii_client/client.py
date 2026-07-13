"""Torii client helpers for configuration, contracts, subscriptions, attachments, and prover reports.

The API mirrors the app-facing endpoints exposed by Torii:

* `/v1/subscriptions` and `/v1/subscriptions/plans` for subscription
  management and billing triggers.
* `/v1/configuration` for configuration snapshots and updates.
* `/v1/contracts/*` for contract deploy/call helpers and governance bindings.
* `/v1/zk/attachments` for uploading, listing, fetching, and deleting
  proof attachments stored on the node.
* `/v1/zk/prover/reports` for querying background prover results.
* `/v1/offline/*` for asset readiness and idempotent asynchronous top-up and
  redemption operations using direct structured JSON.
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
import re
import secrets
import time
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
from urllib.parse import parse_qsl, quote, urlencode, urlsplit

import requests

from .norito_frame import validate_norito_frame
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
_SORAFS_ORDERBOOK_SIDE_VALUES = {"bid", "ask"}
_SORAFS_ORDERBOOK_TIER_VALUES = {"hot", "warm", "archive"}
_SORAFS_ORDERBOOK_CHANNEL_STATUS_VALUES = {
    "open",
    "closing",
    "closed",
    "breached",
    "refunded",
}
_SORAFS_ORDERBOOK_EVENT_KIND_VALUES = {
    "order_accepted",
    "order_cancelled",
    "settlement_receipt_accepted",
}
_SORAFS_XOR_QUANTITY_MAX_TEXT_LENGTH = 155
_SORAFS_ORDERBOOK_ORDER_FIELDS = frozenset(
    {
        "version",
        "order_id_hex",
        "side",
        "tier",
        "price_per_gib",
        "quantity_gib",
        "remaining_gib",
        "owner_account_hex",
        "expiry_unix",
        "nonce",
        "maker_fee_bps",
        "taker_fee_bps",
        "signature",
    }
)
_SORAFS_ORDERBOOK_FILL_FIELDS = frozenset(
    {"trade", "maker_remaining_gib", "taker_remaining_gib", "gross_value"}
)
_SORAFS_ORDERBOOK_TRADE_FIELDS = frozenset(
    {
        "version",
        "trade_id_hex",
        "maker_order_id_hex",
        "taker_order_id_hex",
        "tier",
        "price_per_gib",
        "filled_gib",
        "maker_fee",
        "taker_fee",
        "timestamp_unix",
    }
)
_SORAFS_ORDERBOOK_CHANNEL_FIELDS = frozenset(
    {
        "version",
        "channel_id_hex",
        "trade_id_hex",
        "buyer_account_hex",
        "provider_id_hex",
        "total_bytes",
        "remaining_bytes",
        "xor_locked",
        "status",
        "opened_at_unix",
        "updated_at_unix",
    }
)
_SORAFS_ORDERBOOK_RECEIPT_FIELDS = frozenset(
    {
        "version",
        "receipt_id_hex",
        "channel_id_hex",
        "trade_id_hex",
        "range",
        "chunk_hash_hex",
        "bytes_delivered",
        "xor_debited",
        "provider_credit",
        "fee_amount",
        "issued_at_unix",
        "settlement_signature",
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
    "decode_pdp_commitment_header",
    "inspect_i105_network_prefix",
    "I105NetworkPrefix",
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
    "ContractDeployContractReceipt",
    "ContractDeployHajimariCallReceipt",
    "ContractDeployAssertionReceipt",
    "ContractDeployResponse",
    "ContractOperationReceipt",
    "ContractCallResponse",
    "PipelineDiagnostic",
    "PipelineTransactionStatus",
    "PipelineTransactionStatusResponse",
    "MultisigResponse",
    "GovernanceContractResponse",
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
    "PipelinePreflightSumeragi",
    "PipelinePreflightAdmission",
    "PipelinePreflightBlock",
    "PipelinePreflightPipeline",
    "PipelinePreflightQueue",
    "PipelinePreflightFees",
    "PipelinePreflight",
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
    "OfflineReadiness",
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
    "OfflineLanePrivacySnarkWitnessJson",
    "OfflineLanePrivacyMerkleVariantJson",
    "OfflineLanePrivacySnarkVariantJson",
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
    "KagemushaArtifactBindingJson",
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
    "KagemushaTopUpRequestV2",
    "KagemushaRedeemRequestV2",
    "OfflineOperationKind",
    "OfflinePendingState",
    "OfflineOperationReference",
    "OfflineScaledAmount",
    "OfflineSpendableNote",
    "OfflineVerifierKeyId",
    "KagemushaArtifactBinding",
    "OfflineTopUpAnchor",
    "OfflineTopUpFinalityProofAnchor",
    "OfflineTopUpFinalityConsensusMode",
    "OfflineTopUpFinalityPayloadEncoding",
    "OfflineTopUpFinalityGlobalPhase",
    "OfflineTopUpFinalityDataAvailabilityLayout",
    "OfflineTopUpFinalityHeightContextId",
    "OfflineTopUpFinalityConsensusRound",
    "OfflineTopUpFinalityBlockSubject",
    "OfflineTopUpFinalityExecutionCommitment",
    "OfflineTopUpFinalityQuorumCertificate",
    "OfflineTopUpFinalityValidatorPower",
    "OfflineTopUpFinalityDualQuorum",
    "OfflineTopUpFinalityNextEpochSnapshot",
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
    "SumeragiParamsSnapshot",
    "ToriiCanonicalRequestAuth",
    "canonical_query_string",
    "canonical_request_message",
    "canonical_request_signature_message",
    "build_canonical_request_headers",
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

_IDENTIFIER_COMPACT_ALGORITHM_TAGS = {
    0x01: 0,  # Ed25519
    0x04: 1,  # secp256k1
    0x03: 2,  # BLS normal
    0x05: 3,  # BLS small
    0x02: 4,  # ML-DSA
    0x0A: 5,  # GOST R 34.10-2012 256 A
    0x0B: 6,  # GOST R 34.10-2012 256 B
    0x0C: 7,  # GOST R 34.10-2012 256 C
    0x0D: 8,  # GOST R 34.10-2012 512 A
    0x0E: 9,  # GOST R 34.10-2012 512 B
    0x0F: 10,  # SM2
}

_IDENTIFIER_PUBLIC_KEY_MULTICODEC = {
    0xED: "ed25519",
    0xEE: "ml-dsa",
    0xEA: "bls_normal",
    0xE7: "secp256k1",
    0xEB: "bls_small",
    0x1200: "gost3410-2012-256-paramset-a",
    0x1201: "gost3410-2012-256-paramset-b",
    0x1202: "gost3410-2012-256-paramset-c",
    0x1203: "gost3410-2012-512-paramset-a",
    0x1204: "gost3410-2012-512-paramset-b",
    0x1306: "sm2",
}


def _identifier_compact_length(value: int) -> bytes:
    if value < 0:
        raise ValueError("Norito compact length must be non-negative")
    remaining = int(value)
    out = bytearray()
    while remaining >= 0x80:
        out.append((remaining & 0x7F) | 0x80)
        remaining >>= 7
    out.append(remaining)
    return bytes(out)


def _identifier_sized_field(payload: Union[bytes, bytearray, memoryview]) -> bytes:
    data = bytes(payload)
    return _identifier_compact_length(len(data)) + data


def _identifier_u8(value: int, context: str) -> bytes:
    integer = _identifier_unsigned_integer(value, context)
    if integer > 0xFF:
        raise ValueError(f"{context} must fit in u8")
    return bytes((integer,))


def _identifier_u16(value: int, context: str) -> bytes:
    integer = _identifier_unsigned_integer(value, context)
    if integer > 0xFFFF:
        raise ValueError(f"{context} must fit in u16")
    return integer.to_bytes(2, "little")


def _identifier_u32(value: int, context: str) -> bytes:
    integer = _identifier_unsigned_integer(value, context)
    if integer > 0xFFFF_FFFF:
        raise ValueError(f"{context} must fit in u32")
    return integer.to_bytes(4, "little")


def _identifier_u64(value: Union[int, str], context: str) -> bytes:
    integer = _identifier_unsigned_integer(value, context)
    if integer > 0xFFFF_FFFF_FFFF_FFFF:
        raise ValueError(f"{context} must fit in u64")
    return integer.to_bytes(8, "little")


def _identifier_unsigned_integer(value: Union[int, str], context: str) -> int:
    if isinstance(value, bool):
        raise TypeError(f"{context} must be a non-negative integer")
    if isinstance(value, int):
        integer = value
    elif isinstance(value, str) and value.isdigit():
        integer = int(value, 10)
    else:
        raise TypeError(f"{context} must be a non-negative integer")
    if integer < 0:
        raise ValueError(f"{context} must be a non-negative integer")
    return integer


def _identifier_string(value: Any, context: str) -> bytes:
    text = _require_non_empty_string(value, context)
    data = text.encode("utf-8")
    return _identifier_compact_length(len(data)) + data


def _identifier_exact_string(value: Any, context: str) -> bytes:
    text = _require_exact_non_empty_string(value, context)
    data = text.encode("utf-8")
    return _identifier_compact_length(len(data)) + data


def _identifier_byte_vec(payload: Union[bytes, bytearray, memoryview]) -> bytes:
    data = bytes(payload)
    parts = [len(data).to_bytes(8, "little")]
    for byte in data:
        parts.append(_identifier_sized_field(bytes((byte,))))
    return b"".join(parts)


def _identifier_raw_byte_vec(payload: Union[bytes, bytearray, memoryview]) -> bytes:
    data = bytes(payload)
    return len(data).to_bytes(8, "little") + data


def _identifier_policy_id_payload(raw: Any) -> bytes:
    value = _require_exact_non_empty_string(raw, "payload.policy_id")
    parts = value.split("#", 1)
    if len(parts) != 2 or not parts[0] or not parts[1]:
        raise ValueError("payload.policy_id must use kind#rule")
    if parts[0].strip() != parts[0]:
        raise ValueError("payload.policy_id.kind must not contain surrounding whitespace")
    if parts[1].strip() != parts[1]:
        raise ValueError("payload.policy_id.rule must not contain surrounding whitespace")
    return b"".join(
        (
            _identifier_sized_field(_identifier_string(parts[0], "payload.policy_id.kind")),
            _identifier_sized_field(_identifier_string(parts[1], "payload.policy_id.rule")),
        )
    )


def _identifier_hash_bytes(raw: Any, context: str) -> bytes:
    value = _require_exact_non_empty_string(raw, context)
    body = value
    if body.lower().startswith("hash:"):
        body = body[5:]
    if body.startswith(("0x", "0X")):
        body = body[2:]
    if "#" in body:
        body = body.split("#", 1)[0]
    if len(body) != 64:
        raise ValueError(f"{context} must contain 32 bytes")
    try:
        return bytes.fromhex(body)
    except ValueError as exc:
        raise ValueError(f"{context} contains non-hex characters") from exc


def _identifier_hex_bytes(raw: Any, context: str) -> bytes:
    value = _require_non_empty_string(raw, context)
    body = value.strip()
    if body.startswith(("0x", "0X")):
        body = body[2:]
    if len(body) % 2 != 0:
        raise ValueError(f"{context} must contain an even number of hex characters")
    try:
        return bytes.fromhex(body)
    except ValueError as exc:
        raise ValueError(f"{context} contains non-hex characters") from exc


def _identifier_exact_hex_bytes(raw: Any, context: str) -> bytes:
    value = _require_exact_non_empty_string(raw, context)
    body = value[2:] if value.startswith(("0x", "0X")) else value
    if len(body) % 2 != 0:
        raise ValueError(f"{context} must contain an even number of hex characters")
    try:
        return bytes.fromhex(body)
    except ValueError as exc:
        raise ValueError(f"{context} contains non-hex characters") from exc


def _identifier_exact_tag(raw: Any, context: str) -> str:
    value = _require_exact_non_empty_string(raw, context)
    if value != value.lower():
        raise ValueError(f"{context} must be an exact lowercase RAM-LFE tag")
    return value


def _identifier_backend_tag(raw: Any) -> int:
    value = _identifier_exact_tag(raw, "payload.execution.backend")
    tags = {
        "hkdf-sha3-512-prf-v1": 0,
        "bfv-affine-sha3-256-v1": 1,
        "bfv-programmed-sha3-256-v1": 2,
    }
    try:
        return tags[value]
    except KeyError as exc:
        raise ValueError(f"unsupported RAM-LFE backend: {value}") from exc


def _identifier_verification_mode_tag(raw: Any) -> int:
    value = _identifier_exact_tag(raw, "payload.execution.verification_mode")
    tags = {"signed": 0, "proof": 1}
    try:
        return tags[value]
    except KeyError as exc:
        raise ValueError(f"unsupported RAM-LFE verification mode: {value}") from exc


def _identifier_optional_u64(value: Any, context: str) -> bytes:
    if value is None:
        return b"\x00"
    return b"\x01" + _identifier_sized_field(_identifier_u64(value, context))


def _identifier_program_id_payload(raw: Any, context: str) -> bytes:
    return _identifier_sized_field(_identifier_exact_string(raw, context))


def _identifier_prefixed_hash_payload(raw: Any, prefix: str, context: str) -> bytes:
    value = _require_exact_non_empty_string(raw, context)
    body = value[len(prefix):] if value.lower().startswith(prefix) else value
    digest = _identifier_hash_bytes(body, context)
    return _identifier_compact_length(len(digest)) + digest


def _identifier_execution_payload(execution: Mapping[str, Any]) -> bytes:
    record = _require_mapping(execution, "payload.execution")
    return b"".join(
        (
            _identifier_sized_field(_identifier_program_id_payload(record.get("program_id"), "payload.execution.program_id")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("program_digest"), "payload.execution.program_digest")),
            _identifier_sized_field(_identifier_u32(_identifier_backend_tag(record.get("backend")), "payload.execution.backend")),
            _identifier_sized_field(
                _identifier_u32(
                    _identifier_verification_mode_tag(record.get("verification_mode")),
                    "payload.execution.verification_mode",
                )
            ),
            _identifier_sized_field(_identifier_hash_bytes(record.get("input_ciphertext_hash"), "payload.execution.input_ciphertext_hash")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("output_ciphertext_hash"), "payload.execution.output_ciphertext_hash")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("parameter_digest"), "payload.execution.parameter_digest")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("evaluation_key_digest"), "payload.execution.evaluation_key_digest")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("output_hash"), "payload.execution.output_hash")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("associated_data_hash"), "payload.execution.associated_data_hash")),
            _identifier_sized_field(_identifier_u64(record.get("executed_at_ms"), "payload.execution.executed_at_ms")),
            _identifier_sized_field(_identifier_optional_u64(record.get("expires_at_ms"), "payload.execution.expires_at_ms")),
        )
    )


def _identifier_output_opening_payload(payload: Mapping[str, Any]) -> bytes:
    record = _require_mapping(payload, "payload.opening.payload")
    return b"".join(
        (
            _identifier_sized_field(
                _identifier_program_id_payload(record.get("program_id"), "payload.opening.payload.program_id")
            ),
            _identifier_sized_field(_identifier_hash_bytes(record.get("input_ciphertext_hash"), "payload.opening.payload.input_ciphertext_hash")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("output_ciphertext_hash"), "payload.opening.payload.output_ciphertext_hash")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("parameter_digest"), "payload.opening.payload.parameter_digest")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("evaluation_key_digest"), "payload.opening.payload.evaluation_key_digest")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("opened_output_hash"), "payload.opening.payload.opened_output_hash")),
            _identifier_sized_field(_identifier_u64(record.get("opened_at_ms"), "payload.opening.payload.opened_at_ms")),
            _identifier_sized_field(_identifier_optional_u64(record.get("expires_at_ms"), "payload.opening.payload.expires_at_ms")),
        )
    )


def _identifier_output_opening(opening: Mapping[str, Any]) -> bytes:
    record = _require_mapping(opening, "payload.opening")
    return b"".join(
        (
            _identifier_sized_field(_identifier_output_opening_payload(record.get("payload"))),
            _identifier_sized_field(_identifier_byte_vec(_identifier_exact_hex_bytes(record.get("signature"), "payload.opening.signature"))),
        )
    )


def _identifier_account_id_payload(account_id: Any) -> bytes:
    literal = _require_exact_non_empty_string(account_id, "payload.account_id")
    if "@" in literal:
        raise ValueError("payload.account_id must be a canonical I105 account id")
    canonical = _decode_i105_string(literal)
    if len(canonical) < 4:
        raise ValueError("payload.account_id contains an invalid account address payload")
    controller_tag = canonical[1]
    if controller_tag != 0:
        raise ValueError("payload.account_id multisig controllers are not supported by this verifier")
    curve_id = canonical[2]
    key_len = canonical[3]
    public_key = canonical[4:]
    if len(public_key) != key_len:
        raise ValueError("payload.account_id contains an invalid single-key controller")
    try:
        compact_tag = _IDENTIFIER_COMPACT_ALGORITHM_TAGS[curve_id]
    except KeyError as exc:
        raise ValueError(f"payload.account_id uses unsupported public-key curve {curve_id}") from exc
    public_key_payload = _identifier_byte_vec(bytes((compact_tag,)) + public_key)
    return _identifier_u32(0, "payload.account_id.controller") + _identifier_sized_field(public_key_payload)


def _identifier_normalize_attestation(attestation: Mapping[str, Any]) -> Dict[str, Any]:
    record = _require_mapping(attestation, "identifier receipt attestation")
    kind = _require_exact_non_empty_string(record.get("kind"), "identifier receipt attestation.kind")
    if kind == "signed":
        if record.get("proof_backend") is not None or record.get("proof_b64") is not None:
            raise ValueError("identifier receipt attestation signed attestation must not include proof fields")
        signature = _identifier_exact_hex_bytes(record.get("signature"), "identifier receipt attestation.signature")
        return {"kind": "signed", "signature": signature.hex().upper()}
    if kind == "proof":
        if record.get("signature") is not None:
            raise ValueError("identifier receipt attestation proof attestation must not include signature")
        proof_backend = _require_exact_non_empty_string(
            record.get("proof_backend"),
            "identifier receipt attestation.proof_backend",
        )
        proof_b64 = _require_exact_non_empty_string(record.get("proof_b64"), "identifier receipt attestation.proof_b64")
        return {"kind": "proof", "proof_backend": proof_backend, "proof_b64": proof_b64}
    raise ValueError("identifier receipt attestation.kind must be signed or proof")


def _identifier_proof_box_payload(attestation: Mapping[str, Any]) -> bytes:
    proof_backend = _require_exact_non_empty_string(attestation.get("proof_backend"), "attestation.proof_backend")
    try:
        proof = base64.b64decode(
            _require_exact_non_empty_string(attestation.get("proof_b64"), "attestation.proof_b64"),
            validate=True,
        )
    except binascii.Error as exc:
        raise ValueError("attestation.proof_b64 must be valid base64") from exc
    return b"".join(
        (
            _identifier_sized_field(_identifier_string(proof_backend, "attestation.proof_backend")),
            _identifier_sized_field(_identifier_raw_byte_vec(proof)),
        )
    )


def _identifier_decode_varint(data: bytes, offset: int, context: str) -> Tuple[int, int]:
    value = 0
    shift = 0
    index = offset
    while index < len(data):
        byte = data[index]
        value |= (byte & 0x7F) << shift
        index += 1
        if (byte & 0x80) == 0:
            return value, index
        shift += 7
        if shift > 63:
            raise ValueError(f"{context} contains an invalid multihash varint")
    raise ValueError(f"{context} contains a truncated multihash varint")


def _identifier_decode_public_key(value: Any, context: str) -> Tuple[str, bytes]:
    literal = _require_exact_non_empty_string(value, context)
    prefixed_algorithm: Optional[str] = None
    multihash_literal = literal
    if ":" in literal:
        prefix, multihash_literal = literal.split(":", 1)
        prefixed_algorithm = prefix.lower()
    if multihash_literal.startswith(("0x", "0X")):
        raise ValueError(f"{context} must be a bare multihash hex literal")
    if len(multihash_literal) % 2 != 0:
        raise ValueError(f"{context} must contain an even number of hex characters")
    try:
        data = bytes.fromhex(multihash_literal)
    except ValueError as exc:
        raise ValueError(f"{context} contains non-hex characters") from exc
    code, offset = _identifier_decode_varint(data, 0, context)
    digest_len, offset = _identifier_decode_varint(data, offset, context)
    payload = data[offset:]
    if len(payload) != digest_len:
        raise ValueError(f"{context} multihash payload length does not match its digest header")
    try:
        algorithm = _IDENTIFIER_PUBLIC_KEY_MULTICODEC[code]
    except KeyError as exc:
        raise ValueError(f"{context} uses unsupported multihash code 0x{code:x}") from exc
    if prefixed_algorithm and prefixed_algorithm != algorithm and not (
        prefixed_algorithm == "mldsa" and algorithm == "ml-dsa"
    ):
        raise ValueError(f"{context} algorithm prefix does not match the multihash payload")
    return algorithm, payload


def _identifier_iroha_prehash(message: bytes) -> bytes:
    try:
        from iroha_python.crypto import hash_blake2b_32
    except ImportError as exc:
        raise RuntimeError(
            "verify_identifier_resolution_receipt requires iroha_python crypto bindings"
        ) from exc
    digest = bytearray(hash_blake2b_32(message))
    digest[-1] |= 1
    return bytes(digest)


def _identifier_verify_ed25519(public_key: bytes, message: bytes, signature: bytes) -> bool:
    try:
        from iroha_python.crypto import verify_ed25519
    except ImportError as exc:
        raise RuntimeError(
            "verify_identifier_resolution_receipt requires iroha_python crypto bindings"
        ) from exc
    return bool(verify_ed25519(public_key, message, signature))


def _require_mapping(value: Any, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be an object")
    return value


def _require_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    stripped = value.strip()
    if not stripped:
        raise ValueError(f"{context} must be a non-empty string")
    return stripped


def encode_identifier_resolution_receipt_payload(payload: Mapping[str, Any]) -> bytes:
    """Encode an identifier-resolution receipt payload with the shared canonical layout."""

    record = _require_mapping(payload, "identifier resolution payload")
    return b"".join(
        (
            _identifier_sized_field(_identifier_policy_id_payload(record.get("policy_id"))),
            _identifier_sized_field(_identifier_execution_payload(record.get("execution"))),
            _identifier_sized_field(_identifier_output_opening(record.get("opening"))),
            _identifier_sized_field(_identifier_prefixed_hash_payload(record.get("opaque_id"), "opaque:", "payload.opaque_id")),
            _identifier_sized_field(_identifier_hash_bytes(record.get("receipt_hash"), "payload.receipt_hash")),
            _identifier_sized_field(_identifier_prefixed_hash_payload(record.get("uaid"), "uaid:", "payload.uaid")),
            _identifier_sized_field(_identifier_account_id_payload(record.get("account_id"))),
        )
    )


def encode_identifier_resolution_receipt_attestation(attestation: Mapping[str, Any]) -> bytes:
    """Encode an identifier-resolution receipt attestation with the shared canonical layout."""

    normalized = _identifier_normalize_attestation(attestation)
    if normalized["kind"] == "signed":
        return _identifier_u32(0, "attestation.kind") + _identifier_sized_field(
            _identifier_byte_vec(_identifier_hex_bytes(normalized["signature"], "attestation.signature"))
        )
    return _identifier_u32(1, "attestation.kind") + _identifier_sized_field(
        _identifier_proof_box_payload(normalized)
    )


def verify_identifier_resolution_receipt(
    receipt: Mapping[str, Any],
    policy_summary: Mapping[str, Any],
) -> bool:
    """Verify a signed identifier-resolution receipt against a policy summary.

    Proof attestations are intentionally not accepted here; they require an
    external verifier bound to the declared proof backend.
    """

    receipt_record = _require_mapping(receipt, "identifier resolution receipt")
    payload = _require_mapping(receipt_record.get("payload"), "identifier resolution receipt.payload")
    attestation = _identifier_normalize_attestation(receipt_record.get("attestation"))
    policy = _require_mapping(policy_summary, "identifier policy summary")
    _identifier_policy_id_payload(payload.get("policy_id"))
    receipt_policy_id = _require_exact_non_empty_string(payload.get("policy_id"), "receipt.payload.policy_id")
    policy_id = _require_exact_non_empty_string(policy.get("policy_id"), "policy.policy_id")
    _identifier_policy_id_payload(policy_id)
    if receipt_policy_id != policy_id:
        raise ValueError(
            f"verify_identifier_resolution_receipt: receipt policy {receipt_policy_id} does not match policy {policy_id}"
        )
    if attestation["kind"] != "signed":
        raise RuntimeError("verify_identifier_resolution_receipt: proof attestations require an external verifier")
    algorithm, public_key = _identifier_decode_public_key(
        policy.get("resolver_public_key"),
        "policy.resolver_public_key",
    )
    if algorithm != "ed25519":
        raise RuntimeError(
            f"verify_identifier_resolution_receipt: {algorithm} verification is not available in the Python SDK"
        )
    signed_payload = encode_identifier_resolution_receipt_payload(payload)
    prehash = _identifier_iroha_prehash(signed_payload)
    signature = _identifier_hex_bytes(attestation["signature"], "receipt.attestation.signature")
    return _identifier_verify_ed25519(public_key, prehash, signature)


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


def canonical_query_string(raw: Optional[str]) -> str:
    """Return Torii's canonical form for a raw query string."""

    if not raw:
        return ""
    pairs = parse_qsl(raw, keep_blank_values=True, strict_parsing=False)
    pairs.sort(key=lambda item: (item[0].encode("utf-8"), item[1].encode("utf-8")))
    return urlencode(pairs)


def _split_path_query(path: str) -> Tuple[str, str]:
    parsed = urlsplit(path)
    if parsed.scheme or parsed.netloc:
        return parsed.path or "/", parsed.query
    path_part, separator, query = path.partition("?")
    return path_part or "/", query if separator else ""


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
    """Build the canonical request bytes accepted by Torii app endpoints."""

    path_part, query = _split_path_query(path)
    body_hash = hashlib.sha256(_canonical_body_bytes(body)).hexdigest()
    rendered = "\n".join(
        (
            method.upper(),
            path_part,
            canonical_query_string(query),
            body_hash,
        )
    )
    return rendered.encode("utf-8")


def canonical_request_signature_message(
    method: str,
    path: str,
    body: Optional[Union[str, bytes, bytearray, memoryview]] = None,
    *,
    timestamp_ms: int,
    nonce: str,
) -> bytes:
    """Build canonical request bytes plus freshness metadata for signing."""

    checked_nonce = _require_exact_non_empty_string(nonce, "nonce")
    base = canonical_request_message(method, path, body)
    return b"\n".join((base, str(int(timestamp_ms)).encode("ascii"), checked_nonce.encode("utf-8")))


@dataclass(frozen=True)
class ToriiCanonicalRequestAuth:
    """Signer configuration for app-facing Torii endpoints."""

    account_id: str
    signer: Callable[[bytes], Union[bytes, bytearray, memoryview]]
    timestamp_ms: Optional[int] = None
    nonce: Optional[str] = None


def build_canonical_request_headers(
    *,
    account_id: str,
    signer: Callable[[bytes], Union[bytes, bytearray, memoryview]],
    method: str,
    path: str,
    body: Optional[Union[str, bytes, bytearray, memoryview]] = None,
    timestamp_ms: Optional[int] = None,
    nonce: Optional[str] = None,
) -> Dict[str, str]:
    """Build canonical `X-Iroha-*` headers for a request body."""

    account = _require_exact_non_empty_string(account_id, "account_id")
    if not callable(signer):
        raise TypeError("signer must be callable")
    effective_timestamp = int(timestamp_ms if timestamp_ms is not None else time.time() * 1000)
    effective_nonce = (
        _require_exact_non_empty_string(nonce, "nonce")
        if nonce is not None
        else secrets.token_hex(16)
    )
    message = canonical_request_signature_message(
        method,
        path,
        body,
        timestamp_ms=effective_timestamp,
        nonce=effective_nonce,
    )
    signature = signer(message)
    if not isinstance(signature, (bytes, bytearray, memoryview)):
        raise TypeError("signer must return bytes")
    return {
        HEADER_ACCOUNT: account,
        HEADER_SIGNATURE: base64.b64encode(bytes(signature)).decode("ascii"),
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
    relay_strategy: Optional[str]
    relay_effective_strategy: Optional[str]
    relay_p2p_attached: Optional[bool]
    p2p_ttl_hops: Optional[int]
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
    sequence_violation_closes_total: int
    role_direction_mismatch_total: int
    ping_miss_total: int
    p2p_rebroadcasts_total: int
    p2p_rebroadcast_skipped_total: int
    p2p_auth_failures_total: int
    p2p_ttl_drops_total: int
    p2p_unknown_session_drops_total: int
    p2p_session_claims_in_total: int
    p2p_session_claims_installed_total: int
    p2p_session_claim_conflicts_total: int
    p2p_role_consumed_total: int
    p2p_session_terminated_total: int
    policy: Optional[ConnectStatusPolicy]


@dataclass(frozen=True)
class ConnectSessionInfo:
    """Session tokens returned by ``POST /v1/connect/session``."""

    sid: str
    wallet_uri: str
    app_uri: str
    token_app: str
    token_wallet: str
    token_management: str
    token_relay: str
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


OfflineAssetScale = Literal[
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    12,
    13,
    14,
    15,
    16,
    17,
    18,
    19,
    20,
    21,
    22,
    23,
    24,
    25,
    26,
    27,
    28,
]


class OfflineScaledAmountJson(TypedDict):
    """Direct JSON shape of one positive, scale-bound Offline amount."""

    atomic_units: int
    scale: OfflineAssetScale


class OfflineSpendableNoteJson(TypedDict):
    """Direct JSON shape of one scale-, chain-, and asset-bound note."""

    chain_id: str
    asset: str
    note_commitment: List[int]
    spend_nullifier: List[int]
    amount: OfflineScaledAmountJson


class _OfflineAuthorizationJsonOptional(TypedDict, total=False):
    app_attest_evidence_sha256: Optional[List[int]]
    app_attest_evidence: Optional[List[int]]


class OfflineAuthorizationJson(_OfflineAuthorizationJsonOptional):
    """Self-contained device authorization embedded in an Offline command."""

    authority: str
    device_id: str
    operation_id: List[int]
    issued_at_ms: int
    expires_at_ms: int
    nonce: List[int]
    payload_digest: List[int]
    signature: str


class OfflineVerifierKeyIdJson(TypedDict):
    """Registry identity of one proof verifier."""

    backend: str
    name: str


class OfflineProofBoxJson(TypedDict):
    """Opaque proof bytes with their backend identity."""

    backend: str
    bytes: List[int]


class OfflineVerifyingKeyJson(TypedDict):
    """Opaque verifier bytes with their backend identity."""

    backend: str
    bytes: List[int]


OfflineProofBackend = Literal[
    "halo2-ipa-pasta",
    "halo2-bn254",
    "groth16",
    "stark",
    "unsupported",
    "halo2-ipa-orchard",
    "groth16-bls12-377",
    "fcmp-plus-plus-curve-tree",
    "lattice-pcs-sis",
    "miden-stark",
    "aztec-plonkish-private-kernel",
    "pq-masp-stark-fri",
    "anonymous-pgc",
    "verange",
    "zkat",
    "recursive-anonymous-admission",
    "vega-existing-credential-zk",
    "silent-threshold-anoncred",
    "zk-x509",
    "sis-with-hints",
]
OfflineVerifierStatus = Literal["Proposed", "Active", "Withdrawn"]


class _OfflineVerifyingKeyRecordJsonOptional(TypedDict, total=False):
    owner_manifest_id: Optional[str]
    gas_schedule_id: Optional[str]
    metadata_uri_cid: Optional[str]
    vk_bytes_cid: Optional[str]
    activation_height: Optional[int]
    withdraw_height: Optional[int]
    key: Optional[OfflineVerifyingKeyJson]


class OfflineVerifyingKeyRecordJson(_OfflineVerifyingKeyRecordJsonOptional):
    """Governance-managed verifier record submitted with Offline proofs."""

    version: int
    circuit_id: str
    namespace: str
    backend: OfflineProofBackend
    curve: str
    public_inputs_schema_hash: List[int]
    commitment: List[int]
    vk_len: int
    max_proof_bytes: int
    status: OfflineVerifierStatus


class OfflineMerkleProofJson(TypedDict):
    """Merkle authentication path carried by a lane-privacy witness."""

    leaf_index: int
    audit_path: List[Optional[str]]


class OfflineLanePrivacyMerkleWitnessJson(TypedDict):
    """Typed Merkle lane-privacy witness."""

    leaf: List[int]
    proof: OfflineMerkleProofJson


class OfflineLanePrivacySnarkWitnessJson(TypedDict):
    """Typed base64 SNARK lane-privacy witness."""

    public_inputs: str
    proof: str


class OfflineLanePrivacyMerkleVariantJson(TypedDict):
    """Merkle variant of a lane-privacy witness."""

    kind: Literal["merkle"]
    payload: OfflineLanePrivacyMerkleWitnessJson


class OfflineLanePrivacySnarkVariantJson(TypedDict):
    """SNARK variant of a lane-privacy witness."""

    kind: Literal["snark"]
    payload: OfflineLanePrivacySnarkWitnessJson


OfflineLanePrivacyWitnessJson = Union[
    OfflineLanePrivacyMerkleVariantJson,
    OfflineLanePrivacySnarkVariantJson,
]


class OfflineLanePrivacyProofJson(TypedDict):
    """Lane commitment identity and its typed privacy witness."""

    commitment_id: List[int]
    witness: OfflineLanePrivacyWitnessJson


class _OfflineProofAttachmentJsonOptional(TypedDict, total=False):
    vk_commitment: Optional[List[int]]
    envelope_hash: Optional[List[int]]
    lane_privacy: Optional[OfflineLanePrivacyProofJson]


class OfflineProofAttachmentJson(_OfflineProofAttachmentJsonOptional):
    """Typed proof attachment used by Offline commands."""

    backend: str
    proof: OfflineProofBoxJson
    vk_ref: OfflineVerifierKeyIdJson


class OfflineTopUpShieldEvidenceJson(TypedDict):
    """Public-to-confidential insertion proof for one online top-up."""

    initial_root: List[int]
    finalized_root: List[int]
    leaf_index: int
    proof: OfflineProofAttachmentJson


class OfflineVerifiedFoldStepJson(TypedDict):
    """One checked confidential-transfer proof step."""

    root_before: List[int]
    input_nullifiers: List[List[int]]
    output_commitments: List[List[int]]
    root_after: List[int]
    attachment: OfflineProofAttachmentJson
    verifier_key: OfflineVerifyingKeyJson


class OfflineVerifiedFoldBundleJson(TypedDict):
    """Chain- and asset-bound ordered transfer proof steps."""

    chain_id: str
    asset: str
    steps: List[OfflineVerifiedFoldStepJson]


class OfflineVerifiedFoldVerifierRecordJson(TypedDict):
    """Registry record selected by one checked fold step."""

    id: OfflineVerifierKeyIdJson
    record: OfflineVerifyingKeyRecordJson


class OfflineVerifiedFoldRecordBundleJson(TypedDict):
    """Checked one-hop proof bundle in direct Norito JSON form."""

    bundle: OfflineVerifiedFoldBundleJson
    verifier_records: List[OfflineVerifiedFoldVerifierRecordJson]


class OfflineTopUpAnchorReferenceJson(TypedDict):
    """Compact chain-resolvable identity of one finalized top-up."""

    topup_operation_id: List[int]
    anchor_digest: List[int]


class OfflineBranchPathJson(TypedDict):
    """Canonical branch coordinate inside one top-up lineage."""

    lineage_root: List[int]
    depth: int
    path_bits: List[int]


class OfflineBranchClaimJson(TypedDict):
    """Replay-safe conflict claim for one spendable lineage leaf."""

    path: OfflineBranchPathJson
    transition_tags: str


class _OfflineTaggedUnitJsonOptional(TypedDict, total=False):
    value: None


class OfflineSpendBranchJson(_OfflineTaggedUnitJsonOptional):
    """Recipient or sender-change output role."""

    branch: Literal["recipient", "change"]


class KagemushaArtifactBindingJson(TypedDict):
    """Identity of the one authenticated Kagemusha V3 artifact installation."""

    generation: str
    manifest_sha256: List[int]


class OfflinePeerSplitTransitionJson(TypedDict):
    """Proof-bound peer-split transition payload."""

    binding_digest: List[int]
    branch: OfflineSpendBranchJson
    recipient_request_digest: List[int]
    operation_id: List[int]
    parent_max_proof_step_count: int
    parent_max_peer_hop_count: int


class OfflineRedemptionChangeTransitionJson(TypedDict):
    """Proof-bound partial-redemption change transition payload."""

    binding_digest: List[int]
    parent_bundle_digest: List[int]
    operation_id: List[int]
    parent_proof_step_count: int
    parent_peer_hop_count: int


class OfflinePeerSplitTransitionVariantJson(TypedDict):
    """Tagged peer-split transition."""

    transition: Literal["peer_split"]
    value: OfflinePeerSplitTransitionJson


class OfflineRedemptionChangeTransitionVariantJson(TypedDict):
    """Tagged partial-redemption change transition."""

    transition: Literal["redemption_change"]
    value: OfflineRedemptionChangeTransitionJson


OfflineRecursiveSpendTransitionJson = Union[
    OfflinePeerSplitTransitionVariantJson,
    OfflineRedemptionChangeTransitionVariantJson,
]


class _OfflineRecursiveSpendStatementJsonOptional(TypedDict, total=False):
    transition: Optional[OfflineRecursiveSpendTransitionJson]


class OfflineRecursiveSpendStatementJson(_OfflineRecursiveSpendStatementJsonOptional):
    """Exact public statement bound by one recursive spend proof."""

    chain_id: str
    asset: str
    asset_scale: OfflineAssetScale
    final_root: List[int]
    topup_anchor_refs: List[OfflineTopUpAnchorReferenceJson]
    proof_step_count: int
    peer_hop_count: int
    current_note: OfflineSpendableNoteJson
    branch_claims: List[OfflineBranchClaimJson]
    artifact_binding: KagemushaArtifactBindingJson
    verifier_key_id: OfflineVerifierKeyIdJson


class OfflineRecursiveSpendProofJson(TypedDict):
    """Recursive proof and its exact verifier/public-statement bindings."""

    verifier_key_id: OfflineVerifierKeyIdJson
    public_statement_digest: List[int]
    proof: OfflineProofBoxJson


class OfflineRecursiveSpendBundleJson(TypedDict):
    """Scale-carrying recursive state submitted for redemption."""

    statement: OfflineRecursiveSpendStatementJson
    recursive_proof: OfflineRecursiveSpendProofJson


class OfflineUnshieldPublicInputsJson(TypedDict):
    """Canonical unshield public words bound by a redemption transition."""

    input_commitment_0: List[int]
    input_commitment_1: List[int]
    nullifier_0: List[int]
    nullifier_1: List[int]
    change_output_commitment: List[int]
    root: List[int]
    public_amount: List[int]
    asset_tag: List[int]
    chain_tag: List[int]


class _OfflineRedemptionIntentJsonOptional(TypedDict, total=False):
    change_output: Optional[OfflineSpendableNoteJson]
    change_artifact_binding: Optional[KagemushaArtifactBindingJson]


class OfflineRedemptionIntentJson(_OfflineRedemptionIntentJsonOptional):
    """Canonical public redemption intent covered by the authorization."""

    chain_id: str
    asset: str
    input_note: OfflineSpendableNoteJson
    parent_branch_claims: List[OfflineBranchClaimJson]
    parent_topup_anchor_refs: List[OfflineTopUpAnchorReferenceJson]
    parent_proof_step_count: int
    parent_peer_hop_count: int
    parent_bundle_digest: List[int]
    input_root: List[int]
    recipient: str
    public_amount: OfflineScaledAmountJson
    unshield_public_inputs: OfflineUnshieldPublicInputsJson
    unshield_public_inputs_digest: List[int]
    operation_id: List[int]


class OfflineRedeemChangeJson(TypedDict):
    """Proof-bound change branch retained after partial redemption."""

    output: OfflineSpendableNoteJson
    branch_claims: List[OfflineBranchClaimJson]
    bundle: OfflineRecursiveSpendBundleJson


@dataclass(frozen=True)
class KagemushaTopUpRequestV2:
    """Canonical Norito top-up request plus its embedded operation identifier."""

    norito: bytes
    operation_id: str

    def __post_init__(self) -> None:
        _validate_kagemusha_norito_request(self.norito, "KagemushaTopUpRequestV2.norito")
        object.__setattr__(self, "norito", bytes(self.norito))
        object.__setattr__(self, "operation_id", _require_offline_operation_id(self.operation_id))


@dataclass(frozen=True)
class KagemushaRedeemRequestV2:
    """Canonical Norito redemption request plus its embedded operation identifier."""

    norito: bytes
    operation_id: str

    def __post_init__(self) -> None:
        _validate_kagemusha_norito_request(self.norito, "KagemushaRedeemRequestV2.norito")
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
_OFFLINE_SUMERAGI_PROTOCOL_VERSION = 2
_OFFLINE_BLS_PROOF_BYTES = 96
_OFFLINE_HASH_LITERAL_RE = re.compile(r"^hash:([0-9A-F]{64})#([0-9A-F]{4})$")
_OFFLINE_BLS_VALIDATOR_ID_RE = re.compile(r"^ea0130[0-9A-F]{96}$")
_OFFLINE_MAX_JSON_DEPTH = 128
_OFFLINE_MAX_JSON_RESPONSE_BYTES = 256 * 1024
_KAGEMUSHA_MAX_NORITO_REQUEST_BYTES = 256 * 1024
_KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION = 19
_KAGEMUSHA_MAX_HOPS = 8
_KAGEMUSHA_VERIFIER_CIRCUITS = {
    "active_transfer_verifier":
        "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
    "active_topup_shield_verifier":
        "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
    "active_unshield_verifier":
        "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
    "active_recursive_step_eq_verifier":
        "kagemusha-recursive-spend-step-eq-two-parent-exact-state-v1",
    "active_recursive_step_ep_verifier":
        "kagemusha-recursive-spend-step-ep-two-parent-exact-state-v1",
}


def _validate_kagemusha_norito_request(value: Any, context: str) -> None:
    if type(value) is not bytes:
        raise TypeError(f"{context} must be immutable bytes")
    if not value:
        raise ValueError(f"{context} must not be empty")
    if len(value) > _KAGEMUSHA_MAX_NORITO_REQUEST_BYTES:
        raise ValueError(
            f"{context} exceeds {_KAGEMUSHA_MAX_NORITO_REQUEST_BYTES} bytes"
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
    pattern = (
        _OFFLINE_ASSET_ALIAS_RE
        if "#" in selector
        else _OFFLINE_ASSET_DEFINITION_ID_RE
    )
    if pattern.fullmatch(selector) is None:
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
    """One stable reason an asset is not ready for offline payments."""

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


# Every readiness role exposes the same key-material-free registry record
# shape. Distinct aliases keep role substitution visible at the API boundary.
OfflineActiveTopUpShieldVerifier = OfflineActiveTransferVerifier
OfflineActiveUnshieldVerifier = OfflineActiveTransferVerifier
OfflineActiveRecursiveStepEqVerifier = OfflineActiveTransferVerifier
OfflineActiveRecursiveStepEpVerifier = OfflineActiveTransferVerifier


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


@dataclass(frozen=True)
class OfflineReadiness:
    """Snapshot-bound offline readiness for one asset definition."""

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
            if verifier is not None and verifier.circuit_id != _KAGEMUSHA_VERIFIER_CIRCUITS[field]:
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
        active_records = [verifier for verifier in parsed_verifiers.values() if verifier is not None]
        if len({(record.id.backend, record.id.name) for record in active_records}) != len(active_records):
            raise RuntimeError(f"{context} must not reuse a verifier id across roles")
        if len({record.commitment for record in active_records}) != len(active_records):
            raise RuntimeError(f"{context} must not reuse a verifier commitment across roles")
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
        if ready != (len(blockers) == 0):
            raise RuntimeError(f"{context}.ready must be true exactly when blockers is empty")
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
        expected_recursive_lineage = (
            proof_backend_available
            and active_recursive_step_eq_verifier is not None
            and active_recursive_step_ep_verifier is not None
        )
        if recursive_lineage_supported != expected_recursive_lineage:
            raise RuntimeError(
                f"{context}.recursive_lineage_supported contradicts the recursive verifier state"
            )
        if ("recursive_lineage_unavailable" in blocker_codes) == recursive_lineage_supported:
            raise RuntimeError(
                f"{context}.recursive_lineage_supported contradicts the blocker set"
            )
        if ready and (
            asset_scale is None
            or asset_scale > _OFFLINE_MAX_ASSET_SCALE
            or active_transfer_verifier is None
            or active_topup_shield_verifier is None
            or active_unshield_verifier is None
            or not proof_backend_available
            or not recursive_lineage_supported
        ):
            raise RuntimeError(
                f"{context}.ready requires a supported scale and active transfer and top-up shield verifiers"
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

    chain_id: str
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
class KagemushaArtifactBinding:
    """Content-addressed recursive proof artifact installation."""

    generation: str
    manifest_sha256: Tuple[int, ...]


@dataclass(frozen=True)
class OfflineTopUpAnchor:
    """Closed, cross-checked finalized receipt returned by an applied top-up."""

    version: Literal[2]
    chain_id: str
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
    artifact_binding: KagemushaArtifactBinding
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

    encoding: Literal["plain", "reed_solomon16"]
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
class OfflineTopUpFinalityExecutionCommitment:
    """Deterministic state transition authenticated by a QC."""

    parent_state_root: str
    post_state_root: str
    ordinary_writes_root: str
    topup_anchor_root: Optional[str]
    topup_anchor_count: int


@dataclass(frozen=True)
class OfflineTopUpFinalityQuorumCertificate:
    """Closed structural representation of a Sumeragi-v2 QC."""

    round: OfflineTopUpFinalityConsensusRound
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
class OfflineTopUpFinalityHeightContext:
    """Bounded projection of the immutable finality height context."""

    context_id: OfflineTopUpFinalityHeightContextId
    chain_id: str
    protocol_version: Literal[2]
    height: int
    epoch: int
    epoch_end_height: int
    next_epoch_snapshot: Optional[OfflineTopUpFinalityNextEpochSnapshot]
    mode: OfflineTopUpFinalityConsensusMode
    parent_commit_qc: Optional[OfflineTopUpFinalityQuorumCertificate]
    nexus_amx_context_hash: str
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
    next_min_handle_era: Optional[int] = None
    next_min_sub_nonce: Optional[int] = None


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
        next_min_handle_era=_offline_optional_error_unsigned(
            record, "next_min_handle_era", context, _OFFLINE_MAX_U64
        ),
        next_min_sub_nonce=_offline_optional_error_unsigned(
            record, "next_min_sub_nonce", context, _OFFLINE_MAX_U64
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
        chain_id=_offline_exact_string(
            _offline_required(record, "chain_id", context), f"{context}.chain_id"
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
    encoding = _offline_required(record, "encoding", context)
    if encoding not in ("plain", "reed_solomon16"):
        raise RuntimeError(
            f"{context}.encoding must be plain or reed_solomon16"
        )
    if _offline_required(record, "details", context) is not None:
        raise RuntimeError(f"{context}.details must be null for a unit variant")
    return OfflineTopUpFinalityPayloadEncoding(encoding=encoding, details=None)


def _offline_top_up_finality_phase(
    value: Any, context: str
) -> OfflineTopUpFinalityGlobalPhase:
    record = _offline_mapping(value, context)
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
    )
    parity_shards = _offline_unsigned(
        _offline_required(record, "parity_shards", context),
        f"{context}.parity_shards",
        (1 << 16) - 1,
    )
    if encoding.encoding == "plain" and (data_shards != 0 or parity_shards != 0):
        raise RuntimeError(f"{context} plain encoding requires zero shard counts")
    if encoding.encoding == "reed_solomon16" and (
        data_shards == 0 or parity_shards == 0
    ):
        raise RuntimeError(
            f"{context} reed_solomon16 encoding requires positive shard counts"
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


def _offline_top_up_finality_execution_commitment(
    value: Any,
    context: str,
    *,
    require_topup: bool,
) -> OfflineTopUpFinalityExecutionCommitment:
    record = _offline_mapping(value, context)
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
    )


def _offline_top_up_finality_qc(
    value: Any,
    context: str,
    *,
    require_topup: bool,
) -> OfflineTopUpFinalityQuorumCertificate:
    record = _offline_mapping(value, context)
    round_ = _offline_top_up_finality_round(
        _offline_required(record, "round", context), f"{context}.round"
    )
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


def _offline_top_up_finality_height_context(
    value: Any,
    context: str,
    *,
    expected_finalized_height: int,
) -> OfflineTopUpFinalityHeightContext:
    record = _offline_mapping(value, context)
    context_id = _offline_top_up_finality_height_context_id(
        _offline_required(record, "context_id", context), f"{context}.context_id"
    )
    chain_id = _offline_exact_string(
        _offline_required(record, "chain_id", context), f"{context}.chain_id"
    )
    if len(chain_id.encode("utf-8")) > 128:
        raise RuntimeError(f"{context}.chain_id must contain at most 128 UTF-8 bytes")
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
    if (height == epoch_end_height) != (next_epoch_snapshot is not None):
        raise RuntimeError(
            f"{context}.next_epoch_snapshot must be present exactly at epoch end"
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
    if (height == 1) != (parent_commit_qc is None):
        raise RuntimeError(
            f"{context}.parent_commit_qc must be absent only at genesis height"
        )
    if parent_commit_qc is not None and parent_commit_qc.round.height + 1 != height:
        raise RuntimeError(
            f"{context}.parent_commit_qc.round.height must immediately precede height"
        )
    return OfflineTopUpFinalityHeightContext(
        context_id=context_id,
        chain_id=chain_id,
        protocol_version=2,
        height=height,
        epoch=epoch,
        epoch_end_height=epoch_end_height,
        next_epoch_snapshot=next_epoch_snapshot,
        mode=mode,
        parent_commit_qc=parent_commit_qc,
        nexus_amx_context_hash=_offline_hash_literal(
            _offline_required(record, "nexus_amx_context_hash", context),
            f"{context}.nexus_amx_context_hash",
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
            "chain_id",
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
    if version != 2:
        raise RuntimeError(f"{context}.version must be 2")
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
    chain_id = _offline_exact_string(
        _offline_required(record, "chain_id", context), f"{context}.chain_id"
    )
    if current_note.chain_id != chain_id:
        raise RuntimeError(f"{context}.current_note.chain_id must equal chain_id")
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
        required=("generation", "manifest_sha256"),
    )
    artifact_generation = _offline_exact_string(
        _offline_required(artifact_record, "generation", artifact_context),
        f"{artifact_context}.generation",
    )
    if len(artifact_generation.encode("utf-8")) > 128:
        raise RuntimeError(
            f"{artifact_context}.generation must contain at most 128 UTF-8 bytes"
        )
    artifact_binding = KagemushaArtifactBinding(
        generation=artifact_generation,
        manifest_sha256=_offline_fixed_bytes(
            _offline_required(artifact_record, "manifest_sha256", artifact_context),
            f"{artifact_context}.manifest_sha256",
            non_zero=True,
        ),
    )

    return OfflineTopUpAnchor(
        version=2,
        chain_id=chain_id,
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
    height_context_context = f"{commit_qc_context}.height_context"
    height_context = _offline_mapping(
        _offline_required(commit_qc, "height_context", commit_qc_context),
        height_context_context,
    )
    context_height = _offline_unsigned(
        _offline_required(height_context, "height", height_context_context),
        f"{height_context_context}.height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if context_height != expected_finalized_height:
        raise RuntimeError(
            f"{height_context_context}.height does not match finalized_block_height"
        )

    certificate_context = f"{commit_qc_context}.certificate"
    certificate = _offline_mapping(
        _offline_required(commit_qc, "certificate", commit_qc_context),
        certificate_context,
    )
    round_context = f"{certificate_context}.round"
    certificate_round = _offline_mapping(
        _offline_required(certificate, "round", certificate_context), round_context
    )
    certificate_height = _offline_unsigned(
        _offline_required(certificate_round, "height", round_context),
        f"{round_context}.height",
        _OFFLINE_MAX_U64,
        positive=True,
    )
    if certificate_height != expected_finalized_height:
        raise RuntimeError(
            f"{round_context}.height does not match finalized_block_height"
        )

    anchor_path_context = f"{context}.anchor_path"
    anchor_path = _offline_mapping(
        _offline_required(record, "anchor_path", context), anchor_path_context
    )
    return OfflineTopUpFinalityProof(
        version=1,
        anchor=OfflineTopUpFinalityProofAnchor(
            topup_operation_id=topup_operation_id,
            anchor_digest=anchor_digest,
        ),
        commit_qc=cast(
            Mapping[str, Any],
            _snapshot_offline_json(commit_qc, commit_qc_context),
        ),
        anchor_path=cast(
            Mapping[str, Any],
            _snapshot_offline_json(anchor_path, anchor_path_context),
        ),
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
class VpnQuoteCreateRequest:
    """Request body for creating a native Sora VPN lease quote."""

    metering_public_key_hex: Union[str, bytes, bytearray, memoryview]
    exit_class: Optional[str] = None

    def to_payload(self) -> Dict[str, Any]:
        exit_class = "" if self.exit_class is None else self.exit_class
        return {
            "exit_class": ToriiClient._require_string(exit_class, "vpn quote exit_class")
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
            "exit_class": ToriiClient._require_string(exit_class, "vpn session exit_class")
            if exit_class
            else "",
            "quote_id": ToriiClient._normalize_hex_string(
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
    fee_asset_id: str
    escrow_account_id: str
    operator_account_id: str
    lease_fee_nanos: int
    settlement_grace_secs: int
    flow_label_bits: int
    padding_budget_ms: int
    relay_tls_spki_sha256_hex: Optional[str]


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
    lease_fee_nanos: int
    route_pushes: List[str]
    excluded_routes: List[str]
    dns_servers: List[str]
    tunnel_addresses: List[str]
    mtu_bytes: int
    meter_family: str
    flow_label_bits: int
    padding_budget_ms: int
    relay_tls_spki_sha256_hex: Optional[str]
    metering_public_key_hex: str
    open_lease_instruction: Optional[TransactionInstruction]
    tx_instructions: List[TransactionInstruction]


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
    lease_fee_nanos: int
    flow_label_bits: int
    padding_budget_ms: int
    relay_tls_spki_sha256_hex: Optional[str]
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
    lease_fee_nanos: int
    earned_fee_nanos: int
    refunded_fee_nanos: int
    lease_id_hex: str
    settle_lease_instruction: Optional[TransactionInstruction]
    tx_instructions: List[TransactionInstruction]


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
class PipelineDiagnostic:
    """Structured diagnostic inside a pipeline status envelope."""

    category: str
    message: str
    code: Optional[str]
    decoded_reason: Optional[str]
    contract: Optional[str]
    entrypoint: Optional[str]
    trigger_id: Optional[str]
    step_index: Optional[int]
    vm_pc: Optional[int]
    function: Optional[str]
    source: Optional[str]
    opcode: Optional[str]
    syscall: Optional[str]
    raw_reason: Optional[str]


@dataclass(frozen=True)
class PipelineTransactionStatus:
    """Status details inside a pipeline status envelope."""

    kind: str
    block_height: Optional[int]
    rejection_reason: Any


@dataclass(frozen=True)
class PipelineTransactionStatusResponse:
    """Canonical transaction outcome envelope returned by pipeline-aware endpoints."""

    hash: str
    status: PipelineTransactionStatus
    summary: Optional[str]
    diagnostics: List[PipelineDiagnostic]
    scope: Optional[str]
    resolved_from: Optional[str]
    raw: Dict[str, Any]

    @property
    def is_terminal(self) -> bool:
        return self.status.kind in {"Committed", "Applied", "Rejected", "Expired"}

    @property
    def is_committed(self) -> bool:
        return self.status.kind in {"Committed", "Applied"}

    @property
    def is_rejected(self) -> bool:
        return self.status.kind == "Rejected"

    @property
    def primary_diagnostic(self) -> Optional[PipelineDiagnostic]:
        return self.diagnostics[0] if self.diagnostics else None


@dataclass(frozen=True)
class ContractDeployContractReceipt:
    """One contract receipt returned by ``POST /v1/contracts/deploy``."""

    name: str
    contract_alias: Optional[str]
    contract_address: Optional[str]
    previous_contract_address: Optional[str]
    kaizen: bool
    dataspace: Optional[str]
    deploy_nonce: Optional[int]
    tx_hash_hex: Optional[str]
    pipeline_status: Optional[PipelineTransactionStatusResponse]
    code_hash_hex: str
    abi_hash_hex: str
    status: str


@dataclass(frozen=True)
class ContractDeployHajimariCallReceipt:
    """One hajimari-call receipt returned by ``POST /v1/contracts/deploy``."""

    id: str
    contract_alias: Optional[str]
    entrypoint: Optional[str]
    tx_hash_hex: Optional[str]
    pipeline_status: Optional[PipelineTransactionStatusResponse]
    status: str


@dataclass(frozen=True)
class ContractDeployAssertionReceipt:
    """One assertion receipt returned by ``POST /v1/contracts/deploy``."""

    id: str
    contract_alias: Optional[str]
    entrypoint: Optional[str]
    status: str
    actual_result: Any
    expected_result: Any
    error: Optional[str]


@dataclass(frozen=True)
class ContractDeployResponse:
    """Canonical bundle receipt returned by ``POST /v1/contracts/deploy``."""

    ok: bool
    bundle_name: str
    bundle_digest: str
    chain_fingerprint: str
    dry_run: bool
    completed_stages: List[str]
    failure_point: Optional[str]
    contracts: List[ContractDeployContractReceipt]
    hajimari_calls: List[ContractDeployHajimariCallReceipt]
    assertions: List[ContractDeployAssertionReceipt]


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
    gas_asset_id: Optional[str]
    fee_sponsor: Optional[str]
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
    transaction_scaffold_b64: Optional[str]
    signed_transaction_b64: Optional[str]
    signing_message_b64: Optional[str]
    operation_receipt: ContractOperationReceipt


@dataclass(frozen=True)
class MultisigResponse:
    """Result returned by multisig participation endpoints."""

    ok: bool
    resolved_multisig_account_id: str
    submitted: Optional[bool]
    proposal_id: Optional[str]
    instructions_hash: Optional[str]
    tx_hash_hex: Optional[str]
    executed_tx_hash_hex: Optional[str]
    creation_time_ms: Optional[int]
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
    sponsorship_enabled: bool
    sponsor_max_fee: Any
    sponsor_verified_balance_safety_floor: Any
    canonical_sponsor_account_id: Optional[str]
    fee_receipts_activation_height: int
    external_settlement_enabled: bool
    burn_from_unix_timestamp_ms: int
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
    # SoraFS orderbook
    # ------------------------------------------------------------------
    def get_sorafs_orderbook(
        self,
        *,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """Fetch the local SoraFS orderbook mirror snapshot."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/book",
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
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """List trades emitted by the local SoraFS orderbook mirror."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/trades",
            headers=self._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_trades",
            ),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook trades endpoint returned no payload")
        return self._parse_sorafs_orderbook_list(
            payload,
            field="trades",
            normalizer=self._parse_sorafs_orderbook_trade,
            context="sorafs orderbook trades response",
        )

    def list_sorafs_orderbook_channels(
        self,
        *,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """List settlement channels opened by the local SoraFS orderbook mirror."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/channels",
            headers=self._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_channels",
            ),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook channels endpoint returned no payload")
        return self._parse_sorafs_orderbook_list(
            payload,
            field="channels",
            normalizer=self._parse_sorafs_orderbook_channel,
            context="sorafs orderbook channels response",
        )

    def list_sorafs_orderbook_receipts(
        self,
        *,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """List settlement receipts accepted by the local SoraFS orderbook mirror."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/receipts",
            headers=self._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_receipts",
            ),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook receipts endpoint returned no payload")
        return self._parse_sorafs_orderbook_list(
            payload,
            field="receipts",
            normalizer=self._parse_sorafs_orderbook_receipt,
            context="sorafs orderbook receipts response",
        )

    def submit_sorafs_orderbook_order(
        self,
        payload: Any,
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """Submit signed Norito ``OrderRequestV1`` bytes to the local mirror."""

        return self._submit_sorafs_orderbook_payload(
            "/v1/sorafs/orderbook/orders",
            payload,
            canonical_auth=canonical_auth,
            headers=headers,
            context="submit_sorafs_orderbook_order",
            normalizer=self._parse_sorafs_orderbook_submit_response,
        )

    def submit_sorafs_orderbook_cancel(
        self,
        payload: Any,
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """Submit signed Norito ``OrderCancelV1`` bytes to the local mirror."""

        return self._submit_sorafs_orderbook_payload(
            "/v1/sorafs/orderbook/cancel",
            payload,
            canonical_auth=canonical_auth,
            headers=headers,
            context="submit_sorafs_orderbook_cancel",
            normalizer=self._parse_sorafs_orderbook_cancel_response,
        )

    def submit_sorafs_orderbook_receipt(
        self,
        payload: Any,
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Dict[str, Any]:
        """Submit signed Norito ``SettlementReceiptV1`` bytes to the local mirror."""

        return self._submit_sorafs_orderbook_payload(
            "/v1/sorafs/orderbook/receipts",
            payload,
            canonical_auth=canonical_auth,
            headers=headers,
            context="submit_sorafs_orderbook_receipt",
            normalizer=self._parse_sorafs_orderbook_receipt_submit_response,
        )

    def _submit_sorafs_orderbook_payload(
        self,
        path: str,
        payload: Any,
        *,
        canonical_auth: ToriiCanonicalRequestAuth,
        headers: Optional[Mapping[str, str]],
        context: str,
        normalizer: Callable[..., Dict[str, Any]],
    ) -> Dict[str, Any]:
        body = self._sorafs_orderbook_payload_bytes(payload, f"{context}.payload")
        response = self._request(
            "POST",
            path,
            headers=self._sorafs_orderbook_submit_headers(
                method="POST",
                path=path,
                body=body,
                canonical_auth=canonical_auth,
                headers=headers,
                context=context,
            ),
            data=body,
        )
        self._expect_status(response, {200})
        response_payload = self._maybe_json(response)
        if response_payload is None:
            raise RuntimeError(f"{context} endpoint returned no payload")
        return normalizer(response_payload, context=f"{context} response")

    def list_sorafs_orderbook_events(
        self,
        *,
        since: Optional[Any] = None,
        limit: Optional[Any] = None,
        if_none_match: Optional[str] = None,
        etag: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Optional[Dict[str, Any]]:
        """List replayable local SoraFS orderbook events."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/events",
            params=self._sorafs_orderbook_event_params(
                since=since,
                limit=limit,
                context="list_sorafs_orderbook_events",
            ),
            headers=self._sorafs_orderbook_headers(
                if_none_match=if_none_match,
                etag=etag,
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
        return self._parse_sorafs_orderbook_events(
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
        return parse_sccp_json_object(body, context)

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

    def get_pipeline_preflight(self) -> PipelinePreflight:
        """Fetch pipeline preflight diagnostics (`GET /v1/pipeline/preflight`)."""

        payload = self._get_json_object(
            "/v1/pipeline/preflight",
            context="pipeline preflight",
        )
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
        canonical_auth: Optional[ToriiCanonicalRequestAuth] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> VpnQuote:
        """Create a VPN quote carrying the native `OpenVpnLeaseEscrow` instruction."""

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
        canonical_auth: Optional[ToriiCanonicalRequestAuth] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> VpnSession:
        """Open a VPN session from a paid quote and matching metering key."""

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
        canonical_auth: Optional[ToriiCanonicalRequestAuth] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Optional[VpnSession]:
        """Fetch an active VPN session, returning `None` when absent."""

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
        canonical_auth: Optional[ToriiCanonicalRequestAuth] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Optional[VpnReceipt]:
        """Disconnect a VPN session, returning the canonical lease receipt when present."""

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
        canonical_auth: Optional[ToriiCanonicalRequestAuth] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> VpnReceipt:
        """Submit a relay receipt and receive the native `SettleVpnLease` instruction."""

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
        canonical_auth: Optional[ToriiCanonicalRequestAuth] = None,
        headers: Optional[Mapping[str, str]] = None,
    ) -> VpnReceiptListResponse:
        """List recent disconnected or settled VPN lease receipts for the signed account."""

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

    def delete_connect_session(self, sid: str, token_management: str) -> bool:
        """Delete a Connect session (`DELETE /v1/connect/session/{sid}`)."""

        if not isinstance(token_management, str) or not token_management:
            raise TypeError("token_management must be a non-empty string")
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
    # First-release Kagemusha API
    # ------------------------------------------------------------------
    def get_kagemusha_readiness(self, asset_definition_id: str) -> OfflineReadiness:
        """Fetch Kagemusha readiness by canonical asset id or live asset alias."""

        asset = _offline_asset_selector(asset_definition_id, "asset_definition_id")
        response = self._request(
            "GET",
            _OFFLINE_READINESS_PATH,
            params={"asset_definition_id": asset},
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._offline_json_response(response, "offline readiness response")
        return OfflineReadiness.from_payload(payload, asset)

    def submit_kagemusha_top_up(
        self, request: KagemushaTopUpRequestV2
    ) -> OfflineOperationReference:
        """Submit one canonical typed Norito Kagemusha top-up request."""

        if not isinstance(request, KagemushaTopUpRequestV2):
            raise TypeError("request must be KagemushaTopUpRequestV2")
        return self._submit_kagemusha_command(
            _OFFLINE_TOP_UP_PATH,
            "top_up",
            request.norito,
            request.operation_id,
        )

    def submit_kagemusha_redeem(
        self, request: KagemushaRedeemRequestV2
    ) -> OfflineOperationReference:
        """Submit one canonical typed Norito Kagemusha redemption request."""

        if not isinstance(request, KagemushaRedeemRequestV2):
            raise TypeError("request must be KagemushaRedeemRequestV2")
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

        canonical = self._normalize_canonical_account_id(relay_id, "relay_id")
        response = self._request(
            "GET",
            f"/v1/kaigi/relays/{quote(canonical, safe='')}",
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

    def get_sumeragi_params(self) -> SumeragiParamsSnapshot:
        """Fetch on-chain Sumeragi parameters (`GET /v1/sumeragi/params`)."""

        payload = self._ensure_mapping(
            self._request("GET", "/v1/sumeragi/params").json(),
            "sumeragi params",
        )
        return self._parse_sumeragi_params(payload, context="sumeragi params")

    def get_sumeragi_bls_keys(self) -> Dict[str, Optional[str]]:
        """Return mapping of network keys to optional BLS public keys (`GET /v1/sumeragi/bls-keys`)."""

        payload = self._request("GET", "/v1/sumeragi/bls-keys").json()
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
    # Contract, governance, and council helpers
    # ------------------------------------------------------------------
    def deploy_contract(
        self,
        *,
        authority: str,
        private_key: str,
        code_b64: str,
        contract_alias: str,
        lease_expiry_ms: Optional[int] = None,
        gas_asset_id: Optional[str] = None,
        fee_sponsor: Optional[str] = None,
        gas_limit: Any = None,
    ) -> Optional[ContractDeployResponse]:
        """Deploy bytecode via ``POST /v1/contracts/deploy``."""

        payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "deploy_contract.authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "deploy_contract.private_key",
            ),
            "code_b64": self._normalize_required_base64_payload(
                code_b64,
                "deploy_contract.code_b64",
            ),
            "contract_alias": self._require_non_empty_string(
                contract_alias,
                "deploy_contract.contract_alias",
            ),
        }
        lease_expiry_value = self._normalize_optional_int(
            lease_expiry_ms,
            "deploy_contract.lease_expiry_ms",
            allow_zero=True,
        )
        if lease_expiry_value is not None:
            payload["lease_expiry_ms"] = lease_expiry_value
        if gas_asset_id is not None:
            payload["gas_asset_id"] = self._require_non_empty_string(
                gas_asset_id,
                "deploy_contract.gas_asset_id",
            )
        if fee_sponsor is not None:
            payload["fee_sponsor"] = self._normalize_canonical_account_id(
                fee_sponsor,
                "deploy_contract.fee_sponsor",
            )
        if gas_limit is not None:
            gas_limit_value = self._coerce_int(gas_limit, "deploy_contract.gas_limit")
            if gas_limit_value <= 0:
                raise ValueError("deploy_contract.gas_limit must be positive")
            payload["gas_limit"] = gas_limit_value
        response = self._request(
            "POST",
            "/v1/contracts/deploy",
            headers={
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
            data=json.dumps(payload).encode("utf-8"),
        )
        self._expect_status(response, {200, 202})
        body = self._maybe_json(response)
        if body is None:
            return None
        record = self._ensure_mapping(body, "contract deploy response")
        return self._parse_contract_deploy_response(
            record,
            context="contract deploy response",
        )

    def call_contract(
        self,
        *,
        authority: str,
        private_key: str,
        entrypoint: str,
        contract_address: Optional[str] = None,
        contract_alias: Optional[str] = None,
        payload: Any = None,
        gas_asset_id: Optional[str] = None,
        fee_sponsor: Optional[str] = None,
        gas_limit: Any,
    ) -> ContractCallResponse:
        """Invoke a deployed contract via ``POST /v1/contracts/call``."""

        request_payload: Dict[str, Any] = {
            "authority": self._require_non_empty_string(
                authority,
                "call_contract.authority",
            ),
            "private_key": self._require_non_empty_string(
                private_key,
                "call_contract.private_key",
            ),
        }
        request_payload.update(
            self._normalize_contract_selector(
                contract_address=contract_address,
                contract_alias=contract_alias,
                context="call_contract",
            )
        )
        request_payload["entrypoint"] = self._require_non_empty_string(
            entrypoint,
            "call_contract.entrypoint",
        )
        if payload is not None:
            request_payload["payload"] = self._clone_json_value(
                payload,
                context="call_contract.payload",
            )
        if gas_asset_id is not None:
            request_payload["gas_asset_id"] = self._require_non_empty_string(
                gas_asset_id,
                "call_contract.gas_asset_id",
            )
        if fee_sponsor is not None:
            request_payload["fee_sponsor"] = self._normalize_canonical_account_id(
                fee_sponsor,
                "call_contract.fee_sponsor",
            )
        gas_limit_value = self._coerce_int(gas_limit, "call_contract.gas_limit")
        if gas_limit_value <= 0:
            raise ValueError("call_contract.gas_limit must be positive")
        request_payload["gas_limit"] = gas_limit_value
        response = self._request(
            "POST",
            "/v1/contracts/call",
            headers={
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
            data=json.dumps(request_payload).encode("utf-8"),
        )
        self._expect_status(response, {200, 202})
        body = self._maybe_json(response)
        if body is None:
            raise RuntimeError("contract call endpoint returned no payload")
        record = self._ensure_mapping(body, "contract call response")
        return self._parse_contract_call_response(
            record,
            context="contract call response",
        )

    def propose_multisig(
        self,
        *,
        multisig_account_id: Optional[str] = None,
        multisig_account_alias: Optional[str] = None,
        signer_account_id: str,
        instructions: Sequence[Any],
        public_key_hex: Optional[str] = None,
        signature_b64: Optional[str] = None,
        creation_time_ms: Optional[int] = None,
        fee_sponsor: Optional[str] = None,
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
        if fee_sponsor is not None:
            request_payload["fee_sponsor"] = self._normalize_canonical_account_id(
                fee_sponsor,
                "propose_multisig.fee_sponsor",
            )

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
    ) -> GovernanceContractResponse:
        """Fetch one governed contract binding via ``GET /v1/gov/contracts/{contract_address}``."""

        normalized_address = self._require_non_empty_string(
            contract_address,
            "governance contract contract_address",
        )
        payload = self._get_json_object(
            f"/v1/gov/contracts/{quote(normalized_address, safe='')}",
            context="governance contract response",
        )
        return self._parse_governance_contract_response(
            payload,
            context="governance contract response",
        )

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
        data = self._encode_json_body(body_payload) if body_payload is not None else None
        final_headers = self._vpn_request_headers(
            method,
            path,
            data or b"",
            canonical_auth=canonical_auth,
            headers=headers,
            has_body=data is not None,
        )
        response = self._request(method, path, headers=final_headers, data=data)
        self._expect_status(response, expected_status)
        payload = self._maybe_json(response)
        if payload is None:
            return None
        return self._ensure_mapping(payload, context)

    @staticmethod
    def _encode_json_body(payload: Mapping[str, Any]) -> bytes:
        return json.dumps(payload, separators=(",", ":"), sort_keys=True).encode("utf-8")

    @staticmethod
    def _vpn_request_headers(
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
            final_headers.update(dict(headers))
        if canonical_auth is not None:
            final_headers.update(
                build_canonical_request_headers(
                    account_id=canonical_auth.account_id,
                    signer=canonical_auth.signer,
                    method=method,
                    path=path,
                    body=body,
                    timestamp_ms=canonical_auth.timestamp_ms,
                    nonce=canonical_auth.nonce,
                )
            )
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
        exit_class = record.get("exit_class", record.get("exitClass", ""))
        if exit_class is None:
            exit_class = ""
        return {
            "exit_class": cls._require_string(exit_class, "vpn quote exit_class")
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
        exit_class = record.get("exit_class", record.get("exitClass", ""))
        if exit_class is None:
            exit_class = ""
        return {
            "exit_class": cls._require_string(exit_class, "vpn session exit_class")
            if exit_class
            else "",
            "quote_id": cls._normalize_hex_string(
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
        wire_id = ToriiClient._require_string(record.get("wire_id"), f"{context}.wire_id")
        payload_hex = ToriiClient._normalize_hex_string(
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

    @classmethod
    def _parse_vpn_tx_instructions(
        cls,
        value: Any,
        *,
        context: str,
    ) -> List[TransactionInstruction]:
        if value is None:
            return []
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list")
        return [
            cls._parse_vpn_tx_instruction(entry, context=f"{context}[{index}]")
            for index, entry in enumerate(value)
        ]

    @classmethod
    def _parse_vpn_profile(cls, payload: Mapping[str, Any], *, context: str) -> VpnProfile:
        record = cls._ensure_mapping(payload, context)
        available = record.get("available")
        if not isinstance(available, bool):
            raise RuntimeError(f"{context}.available must be a boolean")
        return VpnProfile(
            available=available,
            relay_endpoint=cls._require_string(record.get("relay_endpoint"), f"{context}.relay_endpoint"),
            supported_exit_classes=cls._parse_string_list(
                record.get("supported_exit_classes"),
                context=f"{context}.supported_exit_classes",
            ),
            default_exit_class=cls._require_string(
                record.get("default_exit_class"),
                f"{context}.default_exit_class",
            ),
            lease_secs=cls._coerce_unsigned(record.get("lease_secs"), f"{context}.lease_secs"),
            dns_push_interval_secs=cls._coerce_unsigned(
                record.get("dns_push_interval_secs"),
                f"{context}.dns_push_interval_secs",
            ),
            meter_family=cls._require_string(record.get("meter_family"), f"{context}.meter_family"),
            route_pushes=cls._parse_string_list(record.get("route_pushes"), context=f"{context}.route_pushes"),
            excluded_routes=cls._parse_string_list(
                record.get("excluded_routes"),
                context=f"{context}.excluded_routes",
            ),
            dns_servers=cls._parse_string_list(record.get("dns_servers"), context=f"{context}.dns_servers"),
            tunnel_addresses=cls._parse_string_list(
                record.get("tunnel_addresses"),
                context=f"{context}.tunnel_addresses",
            ),
            mtu_bytes=cls._coerce_unsigned(record.get("mtu_bytes"), f"{context}.mtu_bytes"),
            display_billing_label=cls._require_string(
                record.get("display_billing_label"),
                f"{context}.display_billing_label",
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
            lease_fee_nanos=cls._coerce_unsigned(
                record.get("lease_fee_nanos"),
                f"{context}.lease_fee_nanos",
            ),
            settlement_grace_secs=cls._coerce_unsigned(
                record.get("settlement_grace_secs"),
                f"{context}.settlement_grace_secs",
            ),
            flow_label_bits=cls._coerce_unsigned(record.get("flow_label_bits"), f"{context}.flow_label_bits"),
            padding_budget_ms=cls._coerce_unsigned(
                record.get("padding_budget_ms"),
                f"{context}.padding_budget_ms",
            ),
            relay_tls_spki_sha256_hex=cls._coerce_optional_string(
                record.get("relay_tls_spki_sha256_hex"),
                context=f"{context}.relay_tls_spki_sha256_hex",
            ),
        )

    @classmethod
    def _parse_vpn_quote(cls, payload: Mapping[str, Any], *, context: str) -> VpnQuote:
        record = cls._ensure_mapping(payload, context)
        return VpnQuote(
            quote_id=cls._normalize_hex_string(record.get("quote_id"), context=f"{context}.quote_id", expected_length=64),
            lease_id_hex=cls._normalize_hex_string(
                record.get("lease_id_hex"),
                context=f"{context}.lease_id_hex",
                expected_length=64,
            ),
            session_id_hex=cls._normalize_hex_string(
                record.get("session_id_hex"),
                context=f"{context}.session_id_hex",
                expected_length=64,
            ),
            payment_reference=cls._require_string(
                record.get("payment_reference"),
                f"{context}.payment_reference",
            ),
            account_id=cls._require_string(record.get("account_id"), f"{context}.account_id"),
            exit_class=cls._require_string(record.get("exit_class"), f"{context}.exit_class"),
            relay_endpoint=cls._require_string(record.get("relay_endpoint"), f"{context}.relay_endpoint"),
            lease_secs=cls._coerce_unsigned(record.get("lease_secs"), f"{context}.lease_secs"),
            quote_expires_at_ms=cls._coerce_unsigned(
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
            lease_fee_nanos=cls._coerce_unsigned(
                record.get("lease_fee_nanos"),
                f"{context}.lease_fee_nanos",
            ),
            route_pushes=cls._parse_string_list(record.get("route_pushes"), context=f"{context}.route_pushes"),
            excluded_routes=cls._parse_string_list(
                record.get("excluded_routes"),
                context=f"{context}.excluded_routes",
            ),
            dns_servers=cls._parse_string_list(record.get("dns_servers"), context=f"{context}.dns_servers"),
            tunnel_addresses=cls._parse_string_list(
                record.get("tunnel_addresses"),
                context=f"{context}.tunnel_addresses",
            ),
            mtu_bytes=cls._coerce_unsigned(record.get("mtu_bytes"), f"{context}.mtu_bytes"),
            meter_family=cls._require_string(record.get("meter_family"), f"{context}.meter_family"),
            flow_label_bits=cls._coerce_unsigned(record.get("flow_label_bits"), f"{context}.flow_label_bits"),
            padding_budget_ms=cls._coerce_unsigned(
                record.get("padding_budget_ms"),
                f"{context}.padding_budget_ms",
            ),
            relay_tls_spki_sha256_hex=cls._coerce_optional_string(
                record.get("relay_tls_spki_sha256_hex"),
                context=f"{context}.relay_tls_spki_sha256_hex",
            ),
            metering_public_key_hex=cls._normalize_hex_string(
                record.get("metering_public_key_hex"),
                context=f"{context}.metering_public_key_hex",
                expected_length=64,
            ),
            open_lease_instruction=cls._parse_optional_vpn_tx_instruction(
                record.get("open_lease_instruction"),
                context=f"{context}.open_lease_instruction",
            ),
            tx_instructions=cls._parse_vpn_tx_instructions(
                record.get("tx_instructions"),
                context=f"{context}.tx_instructions",
            ),
        )

    @classmethod
    def _parse_vpn_session(cls, payload: Mapping[str, Any], *, context: str) -> VpnSession:
        record = cls._ensure_mapping(payload, context)
        return VpnSession(
            session_id=cls._normalize_hex_string(record.get("session_id"), context=f"{context}.session_id", expected_length=64),
            account_id=cls._require_string(record.get("account_id"), f"{context}.account_id"),
            exit_class=cls._require_string(record.get("exit_class"), f"{context}.exit_class"),
            relay_endpoint=cls._require_string(record.get("relay_endpoint"), f"{context}.relay_endpoint"),
            lease_secs=cls._coerce_unsigned(record.get("lease_secs"), f"{context}.lease_secs"),
            expires_at_ms=cls._coerce_unsigned(record.get("expires_at_ms"), f"{context}.expires_at_ms"),
            connected_at_ms=cls._coerce_unsigned(
                record.get("connected_at_ms"),
                f"{context}.connected_at_ms",
            ),
            meter_family=cls._require_string(record.get("meter_family"), f"{context}.meter_family"),
            quote_id=cls._normalize_hex_string(record.get("quote_id"), context=f"{context}.quote_id", expected_length=64),
            payment_reference=cls._require_string(
                record.get("payment_reference"),
                f"{context}.payment_reference",
            ),
            payment_tx_hash=cls._normalize_hex_string(
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
            lease_fee_nanos=cls._coerce_unsigned(
                record.get("lease_fee_nanos"),
                f"{context}.lease_fee_nanos",
            ),
            flow_label_bits=cls._coerce_unsigned(record.get("flow_label_bits"), f"{context}.flow_label_bits"),
            padding_budget_ms=cls._coerce_unsigned(
                record.get("padding_budget_ms"),
                f"{context}.padding_budget_ms",
            ),
            relay_tls_spki_sha256_hex=cls._coerce_optional_string(
                record.get("relay_tls_spki_sha256_hex"),
                context=f"{context}.relay_tls_spki_sha256_hex",
            ),
            route_pushes=cls._parse_string_list(record.get("route_pushes"), context=f"{context}.route_pushes"),
            excluded_routes=cls._parse_string_list(
                record.get("excluded_routes"),
                context=f"{context}.excluded_routes",
            ),
            dns_servers=cls._parse_string_list(record.get("dns_servers"), context=f"{context}.dns_servers"),
            tunnel_addresses=cls._parse_string_list(
                record.get("tunnel_addresses"),
                context=f"{context}.tunnel_addresses",
            ),
            mtu_bytes=cls._coerce_unsigned(record.get("mtu_bytes"), f"{context}.mtu_bytes"),
            helper_ticket_hex=cls._normalize_hex_string(
                record.get("helper_ticket_hex"),
                context=f"{context}.helper_ticket_hex",
            ),
            bytes_in=cls._coerce_unsigned(record.get("bytes_in"), f"{context}.bytes_in"),
            bytes_out=cls._coerce_unsigned(record.get("bytes_out"), f"{context}.bytes_out"),
            status=cls._require_string(record.get("status"), f"{context}.status"),
        )

    @classmethod
    def _parse_vpn_receipt(cls, payload: Mapping[str, Any], *, context: str) -> VpnReceipt:
        record = cls._ensure_mapping(payload, context)
        return VpnReceipt(
            session_id=cls._normalize_hex_string(record.get("session_id"), context=f"{context}.session_id", expected_length=64),
            account_id=cls._require_string(record.get("account_id"), f"{context}.account_id"),
            exit_class=cls._require_string(record.get("exit_class"), f"{context}.exit_class"),
            relay_endpoint=cls._require_string(record.get("relay_endpoint"), f"{context}.relay_endpoint"),
            meter_family=cls._require_string(record.get("meter_family"), f"{context}.meter_family"),
            connected_at_ms=cls._coerce_unsigned(
                record.get("connected_at_ms"),
                f"{context}.connected_at_ms",
            ),
            disconnected_at_ms=cls._coerce_unsigned(
                record.get("disconnected_at_ms"),
                f"{context}.disconnected_at_ms",
            ),
            duration_ms=cls._coerce_unsigned(record.get("duration_ms"), f"{context}.duration_ms"),
            bytes_in=cls._coerce_unsigned(record.get("bytes_in"), f"{context}.bytes_in"),
            bytes_out=cls._coerce_unsigned(record.get("bytes_out"), f"{context}.bytes_out"),
            status=cls._require_string(record.get("status"), f"{context}.status"),
            receipt_source=cls._require_string(
                record.get("receipt_source"),
                f"{context}.receipt_source",
            ),
            quote_id=cls._normalize_hex_string(record.get("quote_id"), context=f"{context}.quote_id", expected_length=64),
            payment_tx_hash=cls._normalize_hex_string(
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
            lease_fee_nanos=cls._coerce_unsigned(
                record.get("lease_fee_nanos"),
                f"{context}.lease_fee_nanos",
            ),
            earned_fee_nanos=cls._coerce_unsigned(
                record.get("earned_fee_nanos"),
                f"{context}.earned_fee_nanos",
            ),
            refunded_fee_nanos=cls._coerce_unsigned(
                record.get("refunded_fee_nanos"),
                f"{context}.refunded_fee_nanos",
            ),
            lease_id_hex=cls._normalize_hex_string(
                record.get("lease_id_hex"),
                context=f"{context}.lease_id_hex",
                expected_length=64,
            ),
            settle_lease_instruction=cls._parse_optional_vpn_tx_instruction(
                record.get("settle_lease_instruction"),
                context=f"{context}.settle_lease_instruction",
            ),
            tx_instructions=cls._parse_vpn_tx_instructions(
                record.get("tx_instructions"),
                context=f"{context}.tx_instructions",
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
        items_payload = record.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise RuntimeError(f"{context}.items must be a list")
        return VpnReceiptListResponse(
            items=[
                cls._parse_vpn_receipt(entry, context=f"{context}.items[{index}]")
                for index, entry in enumerate(items_payload)
            ],
            total=cls._coerce_unsigned(record.get("total"), f"{context}.total"),
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
    ) -> requests.Response:
        url = f"{self._base_url}{path}"
        response = self._session.request(
            method,
            url,
            params=params,
            headers=headers,
            data=data,
            stream=stream,
        )
        return response

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
    def _sorafs_orderbook_payload_bytes(cls, value: Any, context: str) -> bytes:
        if isinstance(value, (bytes, bytearray, memoryview)):
            payload = bytes(value)
        elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
            try:
                payload = bytes(int(entry) for entry in value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"{context} must be bytes-like or a sequence of byte values") from exc
        else:
            raise TypeError(f"{context} must be bytes-like or a sequence of byte values")
        if not payload:
            raise ValueError(f"{context} must not be empty")
        return payload

    @classmethod
    def _sorafs_orderbook_submit_headers(
        cls,
        *,
        method: str,
        path: str,
        body: bytes,
        canonical_auth: Optional[ToriiCanonicalRequestAuth],
        headers: Optional[Mapping[str, str]],
        context: str,
    ) -> Dict[str, str]:
        if canonical_auth is None:
            raise ValueError(f"{context}.canonical_auth is required")
        final_headers: Dict[str, str] = {
            "Accept": "application/json",
            "Content-Type": "application/octet-stream",
        }
        if headers is not None:
            if not isinstance(headers, Mapping):
                raise TypeError(f"{context}.headers must be a mapping")
            final_headers.update({str(key): str(value) for key, value in headers.items()})
        final_headers.update(
            build_canonical_request_headers(
                account_id=canonical_auth.account_id,
                signer=canonical_auth.signer,
                method=method,
                path=path,
                body=body,
                timestamp_ms=canonical_auth.timestamp_ms,
                nonce=canonical_auth.nonce,
            )
        )
        return final_headers

    @classmethod
    def _sorafs_orderbook_headers(
        cls,
        *,
        if_none_match: Optional[str] = None,
        etag: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
        context: str,
        cache: bool = False,
    ) -> Dict[str, str]:
        if if_none_match is not None and etag is not None:
            raise ValueError(f"{context} accepts only one of if_none_match or etag")
        if not cache and (if_none_match is not None or etag is not None):
            raise ValueError(f"{context} does not accept cache validators")
        final_headers: Dict[str, str] = {"Accept": "application/json"}
        if headers is not None:
            if not isinstance(headers, Mapping):
                raise TypeError(f"{context}.headers must be a mapping")
            final_headers.update({str(key): str(value) for key, value in headers.items()})
        validator = if_none_match if if_none_match is not None else etag
        if validator is not None:
            final_headers["If-None-Match"] = cls._require_non_empty_string(
                validator,
                f"{context}.if_none_match",
            )
        return final_headers

    @classmethod
    def _sorafs_orderbook_event_params(
        cls,
        *,
        since: Optional[Any] = None,
        limit: Optional[Any] = None,
        context: str,
    ) -> Optional[Dict[str, int]]:
        params: Dict[str, int] = {}
        if since is not None:
            params["since"] = cls._normalize_sorafs_orderbook_unsigned(
                since,
                f"{context}.since",
                allow_zero=True,
            )
        if limit is not None:
            params["limit"] = cls._normalize_sorafs_orderbook_unsigned(
                limit,
                f"{context}.limit",
                allow_zero=False,
            )
        return params or None

    @classmethod
    def _parse_sorafs_orderbook_book(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        return {
            "schema": cls._require_non_empty_string(record.get("schema"), f"{context}.schema"),
            "source": cls._require_non_empty_string(record.get("source"), f"{context}.source"),
            "generated_at_unix": cls._coerce_unsigned(
                record.get("generated_at_unix"),
                f"{context}.generated_at_unix",
            ),
            "next_sequence": cls._coerce_unsigned(
                record.get("next_sequence"),
                f"{context}.next_sequence",
            ),
            "open_order_count": cls._coerce_unsigned(
                record.get("open_order_count"),
                f"{context}.open_order_count",
            ),
            "trade_count": cls._coerce_unsigned(
                record.get("trade_count"),
                f"{context}.trade_count",
            ),
            "settlement_channel_count": cls._coerce_unsigned(
                record.get("settlement_channel_count"),
                f"{context}.settlement_channel_count",
            ),
            "settlement_receipt_count": cls._coerce_unsigned(
                record.get("settlement_receipt_count"),
                f"{context}.settlement_receipt_count",
            ),
            "depth": cls._parse_sorafs_orderbook_depth(record.get("depth"), context=f"{context}.depth"),
            "open_orders": cls._parse_sorafs_orderbook_array(
                record.get("open_orders"),
                context=f"{context}.open_orders",
                normalizer=cls._parse_sorafs_orderbook_entry,
            ),
            "trades": cls._parse_sorafs_orderbook_array(
                record.get("trades"),
                context=f"{context}.trades",
                normalizer=cls._parse_sorafs_orderbook_trade,
            ),
            "settlement_channels": cls._parse_sorafs_orderbook_array(
                record.get("settlement_channels"),
                context=f"{context}.settlement_channels",
                normalizer=cls._parse_sorafs_orderbook_channel,
            ),
            "settlement_receipts": cls._parse_sorafs_orderbook_array(
                record.get("settlement_receipts"),
                context=f"{context}.settlement_receipts",
                normalizer=cls._parse_sorafs_orderbook_receipt,
            ),
            "expired_order_ids_hex": cls._parse_sorafs_orderbook_hex_list(
                record.get("expired_order_ids_hex"),
                context=f"{context}.expired_order_ids_hex",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_list(
        cls,
        payload: Any,
        *,
        field: str,
        normalizer: Any,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        return {
            "count": cls._coerce_unsigned(record.get("count"), f"{context}.count"),
            field: cls._parse_sorafs_orderbook_array(
                record.get(field),
                context=f"{context}.{field}",
                normalizer=normalizer,
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_submit_response(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        return {
            "status": cls._normalize_sorafs_orderbook_status(
                record.get("status"),
                "accepted",
                f"{context}.status",
            ),
            "sequence": cls._coerce_unsigned(record.get("sequence"), f"{context}.sequence"),
            "open_order_count": cls._coerce_unsigned(
                record.get("open_order_count"),
                f"{context}.open_order_count",
            ),
            "accepted_order": cls._parse_sorafs_orderbook_order(
                record.get("accepted_order"),
                context=f"{context}.accepted_order",
            ),
            "fills": cls._parse_sorafs_orderbook_array(
                record.get("fills"),
                context=f"{context}.fills",
                normalizer=cls._parse_sorafs_orderbook_fill,
            ),
            "settlement_channels_opened": cls._parse_sorafs_orderbook_array(
                record.get("settlement_channels_opened"),
                context=f"{context}.settlement_channels_opened",
                normalizer=cls._parse_sorafs_orderbook_channel,
            ),
            "expired_order_ids_hex": cls._parse_sorafs_orderbook_hex_list(
                record.get("expired_order_ids_hex"),
                context=f"{context}.expired_order_ids_hex",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_cancel_response(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        return {
            "status": cls._normalize_sorafs_orderbook_status(
                record.get("status"),
                "cancelled",
                f"{context}.status",
            ),
            "reason": cls._require_non_empty_string(record.get("reason"), f"{context}.reason"),
            "open_order_count": cls._coerce_unsigned(
                record.get("open_order_count"),
                f"{context}.open_order_count",
            ),
            "cancelled_order": cls._parse_sorafs_orderbook_order(
                record.get("cancelled_order"),
                context=f"{context}.cancelled_order",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_receipt_submit_response(
        cls,
        payload: Any,
        *,
        context: str,
    ) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        return {
            "status": cls._normalize_sorafs_orderbook_status(
                record.get("status"),
                "accepted",
                f"{context}.status",
            ),
            "settlement_receipt_count": cls._coerce_unsigned(
                record.get("settlement_receipt_count"),
                f"{context}.settlement_receipt_count",
            ),
            "open_settlement_channel_count": cls._coerce_unsigned(
                record.get("open_settlement_channel_count"),
                f"{context}.open_settlement_channel_count",
            ),
            "accepted_receipt": cls._parse_sorafs_orderbook_receipt(
                record.get("accepted_receipt"),
                context=f"{context}.accepted_receipt",
            ),
            "updated_channel": cls._parse_sorafs_orderbook_channel(
                record.get("updated_channel"),
                context=f"{context}.updated_channel",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_events(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        since = record.get("since")
        next_since = record.get("next_since")
        return {
            "since": None
            if since is None
            else cls._coerce_unsigned(since, f"{context}.since"),
            "limit": cls._normalize_sorafs_orderbook_unsigned(
                record.get("limit"),
                f"{context}.limit",
                allow_zero=False,
            ),
            "count": cls._coerce_unsigned(record.get("count"), f"{context}.count"),
            "next_since": None
            if next_since is None
            else cls._coerce_unsigned(next_since, f"{context}.next_since"),
            "events": cls._parse_sorafs_orderbook_array(
                record.get("events"),
                context=f"{context}.events",
                normalizer=cls._parse_sorafs_orderbook_event,
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_depth(cls, payload: Any, *, context: str) -> Dict[str, int]:
        record = cls._ensure_mapping(payload, context)
        return {
            key: cls._coerce_unsigned(record.get(key), f"{context}.{key}")
            for key in (
                "hot_bid_gib",
                "hot_ask_gib",
                "warm_bid_gib",
                "warm_ask_gib",
                "archive_bid_gib",
                "archive_ask_gib",
            )
        }

    @classmethod
    def _parse_sorafs_orderbook_entry(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        return {
            "sequence": cls._coerce_unsigned(record.get("sequence"), f"{context}.sequence"),
            "order": cls._parse_sorafs_orderbook_order(record.get("order"), context=f"{context}.order"),
        }

    @classmethod
    def _parse_sorafs_orderbook_order(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_ORDER_FIELDS,
            context,
        )
        return {
            "version": cls._normalize_sorafs_orderbook_unsigned(
                record.get("version"),
                f"{context}.version",
                allow_zero=False,
            ),
            "order_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("order_id_hex"),
                f"{context}.order_id_hex",
            ),
            "side": cls._normalize_sorafs_orderbook_label(
                record.get("side"),
                _SORAFS_ORDERBOOK_SIDE_VALUES,
                f"{context}.side",
            ),
            "tier": cls._normalize_sorafs_orderbook_label(
                record.get("tier"),
                _SORAFS_ORDERBOOK_TIER_VALUES,
                f"{context}.tier",
            ),
            "price_per_gib": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("price_per_gib"),
                f"{context}.price_per_gib",
            ),
            "quantity_gib": cls._coerce_unsigned(record.get("quantity_gib"), f"{context}.quantity_gib"),
            "remaining_gib": cls._coerce_unsigned(record.get("remaining_gib"), f"{context}.remaining_gib"),
            "owner_account_hex": cls._normalize_sorafs_orderbook_hex_bytes(
                record.get("owner_account_hex"),
                f"{context}.owner_account_hex",
            ),
            "expiry_unix": cls._coerce_unsigned(record.get("expiry_unix"), f"{context}.expiry_unix"),
            "nonce": cls._coerce_unsigned(record.get("nonce"), f"{context}.nonce"),
            "maker_fee_bps": cls._coerce_unsigned(record.get("maker_fee_bps"), f"{context}.maker_fee_bps"),
            "taker_fee_bps": cls._coerce_unsigned(record.get("taker_fee_bps"), f"{context}.taker_fee_bps"),
            "signature": cls._parse_sorafs_orderbook_signature(
                record.get("signature"),
                context=f"{context}.signature",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_signature(cls, payload: Any, *, context: str) -> Dict[str, str]:
        record = cls._ensure_mapping(payload, context)
        return {
            "algorithm": cls._require_non_empty_string(record.get("algorithm"), f"{context}.algorithm"),
            "public_key_hex": cls._normalize_sorafs_orderbook_hex_bytes(
                record.get("public_key_hex"),
                f"{context}.public_key_hex",
            ),
            "signature_hex": cls._normalize_sorafs_orderbook_hex_bytes(
                record.get("signature_hex"),
                f"{context}.signature_hex",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_fill(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_FILL_FIELDS,
            context,
        )
        return {
            "trade": cls._parse_sorafs_orderbook_trade(record.get("trade"), context=f"{context}.trade"),
            "maker_remaining_gib": cls._coerce_unsigned(
                record.get("maker_remaining_gib"),
                f"{context}.maker_remaining_gib",
            ),
            "taker_remaining_gib": cls._coerce_unsigned(
                record.get("taker_remaining_gib"),
                f"{context}.taker_remaining_gib",
            ),
            "gross_value": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("gross_value"),
                f"{context}.gross_value",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_trade(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_TRADE_FIELDS,
            context,
        )
        return {
            "version": cls._normalize_sorafs_orderbook_unsigned(
                record.get("version"),
                f"{context}.version",
                allow_zero=False,
            ),
            "trade_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("trade_id_hex"),
                f"{context}.trade_id_hex",
            ),
            "maker_order_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("maker_order_id_hex"),
                f"{context}.maker_order_id_hex",
            ),
            "taker_order_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("taker_order_id_hex"),
                f"{context}.taker_order_id_hex",
            ),
            "tier": cls._normalize_sorafs_orderbook_label(
                record.get("tier"),
                _SORAFS_ORDERBOOK_TIER_VALUES,
                f"{context}.tier",
            ),
            "price_per_gib": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("price_per_gib"),
                f"{context}.price_per_gib",
            ),
            "filled_gib": cls._coerce_unsigned(record.get("filled_gib"), f"{context}.filled_gib"),
            "maker_fee": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("maker_fee"),
                f"{context}.maker_fee",
            ),
            "taker_fee": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("taker_fee"),
                f"{context}.taker_fee",
            ),
            "timestamp_unix": cls._coerce_unsigned(record.get("timestamp_unix"), f"{context}.timestamp_unix"),
        }

    @classmethod
    def _parse_sorafs_orderbook_channel(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_CHANNEL_FIELDS,
            context,
        )
        return {
            "version": cls._normalize_sorafs_orderbook_unsigned(
                record.get("version"),
                f"{context}.version",
                allow_zero=False,
            ),
            "channel_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("channel_id_hex"),
                f"{context}.channel_id_hex",
            ),
            "trade_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("trade_id_hex"),
                f"{context}.trade_id_hex",
            ),
            "buyer_account_hex": cls._normalize_sorafs_orderbook_hex_bytes(
                record.get("buyer_account_hex"),
                f"{context}.buyer_account_hex",
            ),
            "provider_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("provider_id_hex"),
                f"{context}.provider_id_hex",
            ),
            "total_bytes": cls._coerce_unsigned(record.get("total_bytes"), f"{context}.total_bytes"),
            "remaining_bytes": cls._coerce_unsigned(record.get("remaining_bytes"), f"{context}.remaining_bytes"),
            "xor_locked": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("xor_locked"),
                f"{context}.xor_locked",
            ),
            "status": cls._normalize_sorafs_orderbook_label(
                record.get("status"),
                _SORAFS_ORDERBOOK_CHANNEL_STATUS_VALUES,
                f"{context}.status",
            ),
            "opened_at_unix": cls._coerce_unsigned(record.get("opened_at_unix"), f"{context}.opened_at_unix"),
            "updated_at_unix": cls._coerce_unsigned(record.get("updated_at_unix"), f"{context}.updated_at_unix"),
        }

    @classmethod
    def _parse_sorafs_orderbook_receipt(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        cls._require_exact_sorafs_orderbook_fields(
            record,
            _SORAFS_ORDERBOOK_RECEIPT_FIELDS,
            context,
        )
        return {
            "version": cls._normalize_sorafs_orderbook_unsigned(
                record.get("version"),
                f"{context}.version",
                allow_zero=False,
            ),
            "receipt_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("receipt_id_hex"),
                f"{context}.receipt_id_hex",
            ),
            "channel_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("channel_id_hex"),
                f"{context}.channel_id_hex",
            ),
            "trade_id_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("trade_id_hex"),
                f"{context}.trade_id_hex",
            ),
            "range": cls._parse_sorafs_orderbook_byte_range(record.get("range"), context=f"{context}.range"),
            "chunk_hash_hex": cls._normalize_sorafs_orderbook_hex32(
                record.get("chunk_hash_hex"),
                f"{context}.chunk_hash_hex",
            ),
            "bytes_delivered": cls._coerce_unsigned(
                record.get("bytes_delivered"),
                f"{context}.bytes_delivered",
            ),
            "xor_debited": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("xor_debited"),
                f"{context}.xor_debited",
            ),
            "provider_credit": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("provider_credit"),
                f"{context}.provider_credit",
            ),
            "fee_amount": cls._normalize_sorafs_orderbook_xor_quantity(
                record.get("fee_amount"),
                f"{context}.fee_amount",
            ),
            "issued_at_unix": cls._coerce_unsigned(record.get("issued_at_unix"), f"{context}.issued_at_unix"),
            "settlement_signature": cls._parse_sorafs_orderbook_signature(
                record.get("settlement_signature"),
                context=f"{context}.settlement_signature",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_byte_range(cls, payload: Any, *, context: str) -> Dict[str, int]:
        record = cls._ensure_mapping(payload, context)
        return {
            "start": cls._coerce_unsigned(record.get("start"), f"{context}.start"),
            "end": cls._coerce_unsigned(record.get("end"), f"{context}.end"),
        }

    @classmethod
    def _parse_sorafs_orderbook_event(cls, payload: Any, *, context: str) -> Dict[str, Any]:
        record = cls._ensure_mapping(payload, context)
        order_id = record.get("order_id_hex")
        receipt_id = record.get("receipt_id_hex")
        return {
            "sequence": cls._coerce_unsigned(record.get("sequence"), f"{context}.sequence"),
            "kind": cls._normalize_sorafs_orderbook_label(
                record.get("kind"),
                _SORAFS_ORDERBOOK_EVENT_KIND_VALUES,
                f"{context}.kind",
            ),
            "generated_at_unix": cls._coerce_unsigned(
                record.get("generated_at_unix"),
                f"{context}.generated_at_unix",
            ),
            "order_id_hex": None
            if order_id is None
            else cls._normalize_sorafs_orderbook_hex32(order_id, f"{context}.order_id_hex"),
            "trade_ids_hex": cls._parse_sorafs_orderbook_hex_list(
                record.get("trade_ids_hex"),
                context=f"{context}.trade_ids_hex",
            ),
            "settlement_channel_ids_hex": cls._parse_sorafs_orderbook_hex_list(
                record.get("settlement_channel_ids_hex"),
                context=f"{context}.settlement_channel_ids_hex",
            ),
            "receipt_id_hex": None
            if receipt_id is None
            else cls._normalize_sorafs_orderbook_hex32(receipt_id, f"{context}.receipt_id_hex"),
            "expired_order_ids_hex": cls._parse_sorafs_orderbook_hex_list(
                record.get("expired_order_ids_hex"),
                context=f"{context}.expired_order_ids_hex",
            ),
            "open_order_count": cls._coerce_unsigned(
                record.get("open_order_count"),
                f"{context}.open_order_count",
            ),
            "open_settlement_channel_count": cls._coerce_unsigned(
                record.get("open_settlement_channel_count"),
                f"{context}.open_settlement_channel_count",
            ),
            "settlement_receipt_count": cls._coerce_unsigned(
                record.get("settlement_receipt_count"),
                f"{context}.settlement_receipt_count",
            ),
        }

    @classmethod
    def _parse_sorafs_orderbook_array(cls, value: Any, *, context: str, normalizer: Any) -> List[Any]:
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list")
        return [
            normalizer(entry, context=f"{context}[{index}]")
            for index, entry in enumerate(value)
        ]

    @classmethod
    def _parse_sorafs_orderbook_hex_list(cls, value: Any, *, context: str) -> List[str]:
        if not isinstance(value, list):
            raise RuntimeError(f"{context} must be a list")
        return [
            cls._normalize_sorafs_orderbook_hex32(entry, f"{context}[{index}]")
            for index, entry in enumerate(value)
        ]

    @classmethod
    def _normalize_sorafs_orderbook_label(
        cls,
        value: Any,
        allowed: set[str],
        context: str,
    ) -> str:
        label = cls._require_non_empty_string(value, context).lower()
        if label not in allowed:
            raise ValueError(f"{context} must be one of {', '.join(sorted(allowed))}")
        return label

    @classmethod
    def _normalize_sorafs_orderbook_status(
        cls,
        value: Any,
        expected: str,
        context: str,
    ) -> str:
        status = cls._require_non_empty_string(value, context)
        if status != expected:
            raise ValueError(f"{context} must be {expected}")
        return status

    @classmethod
    def _normalize_sorafs_orderbook_hex32(cls, value: Any, context: str) -> str:
        return cls._normalize_sorafs_orderbook_hex_bytes(value, context, expected_length=64)

    @classmethod
    def _normalize_sorafs_orderbook_hex_bytes(
        cls,
        value: Any,
        context: str,
        *,
        expected_length: Optional[int] = None,
    ) -> str:
        return cls._normalize_hex_string(
            value,
            context=context,
            expected_length=expected_length,
        )

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
        unexpected = set(record).difference(expected)
        if unexpected:
            labels = ", ".join(sorted(str(field) for field in unexpected))
            raise ValueError(f"{context} contains unknown or retired fields: {labels}")

    @classmethod
    def _normalize_sorafs_orderbook_unsigned(
        cls,
        value: Any,
        context: str,
        *,
        allow_zero: bool,
    ) -> int:
        if isinstance(value, bool):
            raise TypeError(f"{context} must be an integer")
        if isinstance(value, int):
            number = value
        elif isinstance(value, str):
            literal = value.strip()
            if not re.fullmatch(r"[+-]?\d+", literal):
                raise TypeError(f"{context} must be an integer")
            number = int(literal)
        else:
            raise TypeError(f"{context} must be an integer")
        if number < 0 or (number == 0 and not allow_zero):
            raise ValueError(f"{context} must be {'non-negative' if allow_zero else 'positive'}")
        return number

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
                sponsorship_enabled=self._coerce_bool(
                    fees.get("sponsorship_enabled"),
                    f"{context}.fees.sponsorship_enabled",
                ),
                sponsor_max_fee=self._clone_json_value(
                    fees.get("sponsor_max_fee"),
                    context=f"{context}.fees.sponsor_max_fee",
                ),
                sponsor_verified_balance_safety_floor=self._clone_json_value(
                    fees.get("sponsor_verified_balance_safety_floor"),
                    context=f"{context}.fees.sponsor_verified_balance_safety_floor",
                ),
                canonical_sponsor_account_id=(
                    None
                    if fees.get("canonical_sponsor_account_id") is None
                    else str(fees.get("canonical_sponsor_account_id"))
                ),
                fee_receipts_activation_height=self._coerce_int(
                    fees.get("fee_receipts_activation_height"),
                    f"{context}.fees.fee_receipts_activation_height",
                ),
                external_settlement_enabled=self._coerce_bool(
                    fees.get("external_settlement_enabled"),
                    f"{context}.fees.external_settlement_enabled",
                ),
                burn_from_unix_timestamp_ms=self._coerce_int(
                    fees.get("burn_from_unix_timestamp_ms"),
                    f"{context}.fees.burn_from_unix_timestamp_ms",
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
        record = ToriiClient._ensure_mapping(payload, context)
        sid = ToriiClient._require_string(record.get("sid"), f"{context}.sid")
        wallet_uri = ToriiClient._require_string(record.get("wallet_uri"), f"{context}.wallet_uri")
        app_uri = ToriiClient._require_string(record.get("app_uri"), f"{context}.app_uri")
        token_app = ToriiClient._require_string(record.get("token_app"), f"{context}.token_app")
        token_wallet = ToriiClient._require_string(record.get("token_wallet"), f"{context}.token_wallet")
        token_management = ToriiClient._require_string(
            record.get("token_management"),
            f"{context}.token_management",
        )
        token_relay = ToriiClient._require_string(record.get("token_relay"), f"{context}.token_relay")
        known = {
            "sid",
            "wallet_uri",
            "app_uri",
            "token_app",
            "token_wallet",
            "token_management",
            "token_relay",
        }
        extra = {key: value for key, value in record.items() if key not in known}
        return ConnectSessionInfo(
            sid=sid,
            wallet_uri=wallet_uri,
            app_uri=app_uri,
            token_app=token_app,
            token_wallet=token_wallet,
            token_management=token_management,
            token_relay=token_relay,
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
    def _parse_pipeline_diagnostic(payload: Any, *, context: str) -> PipelineDiagnostic:
        record = ToriiClient._ensure_mapping(payload, context)
        return PipelineDiagnostic(
            category=ToriiClient._require_non_empty_string(record.get("category"), f"{context}.category"),
            message=ToriiClient._require_non_empty_string(record.get("message"), f"{context}.message"),
            code=ToriiClient._coerce_optional_string(record.get("code"), context=f"{context}.code"),
            decoded_reason=ToriiClient._coerce_optional_string(
                record.get("decoded_reason"),
                context=f"{context}.decoded_reason",
            ),
            contract=ToriiClient._coerce_optional_string(record.get("contract"), context=f"{context}.contract"),
            entrypoint=ToriiClient._coerce_optional_string(record.get("entrypoint"), context=f"{context}.entrypoint"),
            trigger_id=ToriiClient._coerce_optional_string(record.get("trigger_id"), context=f"{context}.trigger_id"),
            step_index=ToriiClient._coerce_optional_unsigned(
                record.get("step_index"),
                context=f"{context}.step_index",
            ),
            vm_pc=ToriiClient._coerce_optional_unsigned(record.get("vm_pc"), context=f"{context}.vm_pc"),
            function=ToriiClient._coerce_optional_string(record.get("function"), context=f"{context}.function"),
            source=ToriiClient._coerce_optional_string(record.get("source"), context=f"{context}.source"),
            opcode=ToriiClient._coerce_optional_string(record.get("opcode"), context=f"{context}.opcode"),
            syscall=ToriiClient._coerce_optional_string(record.get("syscall"), context=f"{context}.syscall"),
            raw_reason=ToriiClient._coerce_optional_string(record.get("raw_reason"), context=f"{context}.raw_reason"),
        )

    @staticmethod
    def _parse_pipeline_status_response(
        payload: Any,
        *,
        context: str,
    ) -> PipelineTransactionStatusResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        status_record = ToriiClient._ensure_mapping(record.get("status"), f"{context}.status")
        diagnostics_value = record.get("diagnostics") or []
        if not isinstance(diagnostics_value, list):
            raise RuntimeError(f"{context}.diagnostics must be a list")
        raw_hash = record.get("hash", record.get("tx_hash_hex"))
        return PipelineTransactionStatusResponse(
            hash=ToriiClient._require_non_empty_string(raw_hash, f"{context}.hash"),
            status=PipelineTransactionStatus(
                kind=ToriiClient._require_non_empty_string(
                    status_record.get("kind"),
                    f"{context}.status.kind",
                ),
                block_height=ToriiClient._coerce_optional_unsigned(
                    status_record.get("block_height"),
                    context=f"{context}.status.block_height",
                ),
                rejection_reason=status_record.get("rejection_reason"),
            ),
            summary=ToriiClient._coerce_optional_string(record.get("summary"), context=f"{context}.summary"),
            diagnostics=[
                ToriiClient._parse_pipeline_diagnostic(
                    entry,
                    context=f"{context}.diagnostics[{index}]",
                )
                for index, entry in enumerate(diagnostics_value)
            ],
            scope=ToriiClient._coerce_optional_string(record.get("scope"), context=f"{context}.scope"),
            resolved_from=ToriiClient._coerce_optional_string(
                record.get("resolved_from"),
                context=f"{context}.resolved_from",
            ),
            raw=dict(record),
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
    def _parse_contract_deploy_contract_receipt(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> ContractDeployContractReceipt:
        record = ToriiClient._ensure_mapping(payload, context)
        tx_hash_hex_value = record.get("tx_hash_hex")
        tx_hash_hex = None
        if tx_hash_hex_value is not None:
            tx_hash_hex = ToriiClient._normalize_hex_string(
                tx_hash_hex_value,
                context=f"{context}.tx_hash_hex",
                expected_length=64,
            )
        deploy_nonce_value = record.get("deploy_nonce")
        deploy_nonce = None
        if deploy_nonce_value is not None:
            deploy_nonce = ToriiClient._coerce_unsigned(
                deploy_nonce_value,
                f"{context}.deploy_nonce",
            )
        return ContractDeployContractReceipt(
            name=ToriiClient._require_non_empty_string(record.get("name"), f"{context}.name"),
            contract_alias=ToriiClient._coerce_optional_string(
                record.get("contract_alias"),
                context=f"{context}.contract_alias",
            ),
            contract_address=ToriiClient._coerce_optional_string(
                record.get("contract_address"),
                context=f"{context}.contract_address",
            ),
            previous_contract_address=ToriiClient._coerce_optional_string(
                record.get("previous_contract_address"),
                context=f"{context}.previous_contract_address",
            ),
            kaizen=bool(record.get("kaizen")),
            dataspace=ToriiClient._coerce_optional_string(
                record.get("dataspace"),
                context=f"{context}.dataspace",
            ),
            deploy_nonce=deploy_nonce,
            tx_hash_hex=tx_hash_hex,
            pipeline_status=ToriiClient._parse_optional_pipeline_status_response(
                record.get("pipeline_status"),
                context=f"{context}.pipeline_status",
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
            status=ToriiClient._require_non_empty_string(
                record.get("status"),
                f"{context}.status",
            ),
        )

    @staticmethod
    def _parse_contract_deploy_call_receipt(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> ContractDeployHajimariCallReceipt:
        record = ToriiClient._ensure_mapping(payload, context)
        tx_hash_hex_value = record.get("tx_hash_hex")
        tx_hash_hex = None
        if tx_hash_hex_value is not None:
            tx_hash_hex = ToriiClient._normalize_hex_string(
                tx_hash_hex_value,
                context=f"{context}.tx_hash_hex",
                expected_length=64,
            )
        return ContractDeployHajimariCallReceipt(
            id=ToriiClient._require_non_empty_string(record.get("id"), f"{context}.id"),
            contract_alias=ToriiClient._coerce_optional_string(
                record.get("contract_alias"),
                context=f"{context}.contract_alias",
            ),
            entrypoint=ToriiClient._coerce_optional_string(
                record.get("entrypoint"),
                context=f"{context}.entrypoint",
            ),
            tx_hash_hex=tx_hash_hex,
            pipeline_status=ToriiClient._parse_optional_pipeline_status_response(
                record.get("pipeline_status"),
                context=f"{context}.pipeline_status",
            ),
            status=ToriiClient._require_non_empty_string(
                record.get("status"),
                f"{context}.status",
            ),
        )

    @staticmethod
    def _parse_contract_deploy_assertion_receipt(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> ContractDeployAssertionReceipt:
        record = ToriiClient._ensure_mapping(payload, context)
        return ContractDeployAssertionReceipt(
            id=ToriiClient._require_non_empty_string(record.get("id"), f"{context}.id"),
            contract_alias=ToriiClient._coerce_optional_string(
                record.get("contract_alias"),
                context=f"{context}.contract_alias",
            ),
            entrypoint=ToriiClient._coerce_optional_string(
                record.get("entrypoint"),
                context=f"{context}.entrypoint",
            ),
            status=ToriiClient._require_non_empty_string(
                record.get("status"),
                f"{context}.status",
            ),
            actual_result=record.get("actual_result"),
            expected_result=record.get("expected_result"),
            error=ToriiClient._coerce_optional_string(
                record.get("error"),
                context=f"{context}.error",
            ),
        )

    @staticmethod
    def _parse_contract_deploy_response(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> ContractDeployResponse:
        record = ToriiClient._ensure_mapping(payload, context)
        contracts = ToriiClient._ensure_list(
            record.get("contracts"),
            context=f"{context}.contracts",
        )
        hajimari_calls = ToriiClient._ensure_list(
            record.get("hajimari_calls"),
            context=f"{context}.hajimari_calls",
        )
        assertions = ToriiClient._ensure_list(
            record.get("assertions"),
            context=f"{context}.assertions",
        )
        return ContractDeployResponse(
            ok=bool(record.get("ok")),
            bundle_name=ToriiClient._require_non_empty_string(
                record.get("bundle_name"),
                f"{context}.bundle_name",
            ),
            bundle_digest=ToriiClient._require_non_empty_string(
                record.get("bundle_digest"),
                f"{context}.bundle_digest",
            ),
            chain_fingerprint=ToriiClient._require_non_empty_string(
                record.get("chain_fingerprint"),
                f"{context}.chain_fingerprint",
            ),
            dry_run=bool(record.get("dry_run")),
            completed_stages=ToriiClient._parse_string_list(
                record.get("completed_stages"),
                context=f"{context}.completed_stages",
            ),
            failure_point=ToriiClient._coerce_optional_string(
                record.get("failure_point"),
                context=f"{context}.failure_point",
            ),
            contracts=[
                ToriiClient._parse_contract_deploy_contract_receipt(
                    ToriiClient._ensure_mapping(
                        item,
                        f"{context}.contracts[{index}]",
                    ),
                    context=f"{context}.contracts[{index}]",
                )
                for index, item in enumerate(contracts)
            ],
            hajimari_calls=[
                ToriiClient._parse_contract_deploy_call_receipt(
                    ToriiClient._ensure_mapping(
                        item,
                        f"{context}.hajimari_calls[{index}]",
                    ),
                    context=f"{context}.hajimari_calls[{index}]",
                )
                for index, item in enumerate(hajimari_calls)
            ],
            assertions=[
                ToriiClient._parse_contract_deploy_assertion_receipt(
                    ToriiClient._ensure_mapping(
                        item,
                        f"{context}.assertions[{index}]",
                    ),
                    context=f"{context}.assertions[{index}]",
                )
                for index, item in enumerate(assertions)
            ],
        )

    @staticmethod
    def _parse_contract_call_response(
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> ContractCallResponse:
        record = ToriiClient._ensure_mapping(payload, context)
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
            ok=bool(record.get("ok")),
            submitted=bool(record.get("submitted")),
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
            transaction_scaffold_b64=ToriiClient._normalize_optional_base64_payload(
                record.get("transaction_scaffold_b64"),
                context=f"{context}.transaction_scaffold_b64",
            ),
            signed_transaction_b64=ToriiClient._normalize_optional_base64_payload(
                record.get("signed_transaction_b64"),
                context=f"{context}.signed_transaction_b64",
            ),
            signing_message_b64=ToriiClient._normalize_optional_base64_payload(
                record.get("signing_message_b64"),
                context=f"{context}.signing_message_b64",
            ),
            operation_receipt=operation_receipt,
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
            gas_asset_id=ToriiClient._coerce_optional_string(
                record.get("gas_asset_id"),
                context=f"{context}.gas_asset_id",
            ),
            fee_sponsor=ToriiClient._coerce_optional_string(
                record.get("fee_sponsor"),
                context=f"{context}.fee_sponsor",
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
        submitted_value = record.get("submitted")
        if submitted_value is None:
            submitted = None
        elif isinstance(submitted_value, bool):
            submitted = submitted_value
        else:
            raise TypeError(f"{context}.submitted must be a boolean")

        def optional_hash(field: str) -> Optional[str]:
            raw = record.get(field)
            if raw is None:
                return None
            return ToriiClient._normalize_hex_string(
                raw,
                context=f"{context}.{field}",
                expected_length=64,
            )

        creation_raw = record.get("creation_time_ms")
        creation_time_ms = None
        if creation_raw is not None:
            creation_time_ms = ToriiClient._coerce_unsigned(
                creation_raw,
                f"{context}.creation_time_ms",
            )
        if record.get("ok") is not True:
            raise RuntimeError(f"{context}.ok must be true")
        return MultisigResponse(
            ok=True,
            resolved_multisig_account_id=ToriiClient._require_exact_i105_account_id(
                record.get("resolved_multisig_account_id"),
                f"{context}.resolved_multisig_account_id",
            ),
            submitted=submitted,
            proposal_id=ToriiClient._coerce_optional_string(
                record.get("proposal_id"),
                context=f"{context}.proposal_id",
            ),
            instructions_hash=optional_hash("instructions_hash"),
            tx_hash_hex=optional_hash("tx_hash_hex"),
            executed_tx_hash_hex=optional_hash("executed_tx_hash_hex"),
            creation_time_ms=creation_time_ms,
            signing_message_b64=ToriiClient._normalize_optional_base64_payload(
                record.get("signing_message_b64"),
                context=f"{context}.signing_message_b64",
            ),
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
        queue_queued=current.queue_queued,
        queue_inflight=current.queue_inflight,
        queue_delta=queue_delta,
        time_since_last_block_ms=current.time_since_last_block_ms,
        time_since_last_non_empty_block_ms=current.time_since_last_non_empty_block_ms,
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
