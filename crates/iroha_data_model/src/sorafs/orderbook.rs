//! Authoritative on-chain SoraFS orderbook policy and audit records (SFM-2).
//!
//! Signed order, cancellation, and settlement payload schemas remain in
//! `sorafs_manifest::orderbook`. These records bind those canonical payloads to
//! a governance-controlled policy and to deterministic ledger admission state.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use sorafs_manifest::deal::XorQuantity;

use crate::{
    account::AccountId, asset::AssetDefinitionId, escrow::EscrowId,
    events::data::sorafs::SorafsOrderbookLedgerEvent, sorafs::capacity::ProviderId,
};

/// First-release schema version for [`OrderbookAdmissionPolicyV1`].
pub const ORDERBOOK_ADMISSION_POLICY_VERSION_V1: u16 = 1;
/// Maximum order lifetime governance may configure for the first release.
pub const ORDERBOOK_MAX_ORDER_LIFETIME_SECS_V1: u64 = 365 * 24 * 60 * 60;
/// Maximum settlement-receipt age governance may configure for the first release.
pub const ORDERBOOK_MAX_RECEIPT_AGE_SECS_V1: u64 = 30 * 24 * 60 * 60;
/// Maximum future clock skew governance may configure for signed receipts.
pub const ORDERBOOK_MAX_CLOCK_SKEW_SECS_V1: u64 = 60 * 60;
/// Maximum bytes represented by one first-release settlement receipt.
pub const ORDERBOOK_MAX_RECEIPT_BYTES_V1: u64 = 1 << 40;
/// Hard ceiling for receipt ranges retained in one channel index.
pub const ORDERBOOK_MAX_RECEIPTS_PER_CHANNEL_V1: u32 = 8_192;
/// Hard ceiling for open orders retained by the authoritative V1 book.
pub const ORDERBOOK_MAX_OPEN_ORDERS_V1: u32 = 4_096;
/// Hard ceiling for simultaneously open settlement channels.
pub const ORDERBOOK_MAX_OPEN_SETTLEMENT_CHANNELS_V1: u32 = 4_096;
/// Hard ceiling for fills committed by one deterministic matching instruction.
pub const ORDERBOOK_MAX_FILLS_PER_EXECUTION_V1: u32 = 64;
/// Hard ceiling for orders/channels examined by one maintenance instruction.
pub const ORDERBOOK_MAX_MAINTENANCE_ITEMS_V1: u32 = 512;
/// Hard ceiling for one authoritative orderbook read page.
pub const ORDERBOOK_QUERY_MAX_ITEMS_V1: u32 = 500;
/// Hard ceiling for stored records inspected by one filtered orderbook read page.
pub const ORDERBOOK_QUERY_MAX_INSPECTED_RECORDS_V1: u32 =
    ORDERBOOK_QUERY_MAX_ITEMS_V1 + ORDERBOOK_MAX_MAINTENANCE_ITEMS_V1;
/// Hard encoded-byte ceiling for one committed orderbook event page.
pub const ORDERBOOK_QUERY_MAX_EVENT_PAGE_BYTES_V1: usize = 512 * 1024;
/// Hard ceiling for encoded stored-record bytes inspected by one filtered read page.
pub const ORDERBOOK_QUERY_MAX_READ_BYTES_V1: usize = ORDERBOOK_QUERY_MAX_EVENT_PAGE_BYTES_V1 * 128;
/// Domain separator for policy digests.
pub const ORDERBOOK_ADMISSION_POLICY_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.orderbook.admission-policy.v1";
/// Domain separator for funded settlement locks derived from channel ids.
pub const ORDERBOOK_SETTLEMENT_ESCROW_ID_DOMAIN_V1: &[u8] =
    b"sorafs.orderbook.settlement-escrow-id.v1";
/// Domain separator for bid-order custody locks.
pub const ORDERBOOK_ORDER_ESCROW_ID_DOMAIN_V1: &[u8] = b"sorafs.orderbook.order-escrow-id.v1";
/// Reserved four-byte namespace tag for bid-order custody locks.
pub const ORDERBOOK_ORDER_ESCROW_ID_PREFIX_V1: [u8; 4] = *b"SFO1";
/// Reserved four-byte namespace tag for settlement-channel custody locks.
pub const ORDERBOOK_SETTLEMENT_ESCROW_ID_PREFIX_V1: [u8; 4] = *b"SFC1";

fn namespaced_orderbook_escrow_id(domain: &[u8], namespace: [u8; 4], id: [u8; 32]) -> EscrowId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&id);
    let mut digest = *hasher.finalize().as_bytes();
    digest[..namespace.len()].copy_from_slice(&namespace);
    EscrowId::new(iroha_crypto::Hash::prehashed(digest))
}

/// Derive the native asset-lock identifier that funds an admitted bid.
///
/// Four reserved namespace bytes prevent public escrow creators from
/// front-running this deterministic identifier while retaining 224 bits of
/// domain-separated digest material.
#[must_use]
pub fn orderbook_order_escrow_id(order_id: [u8; 32]) -> EscrowId {
    namespaced_orderbook_escrow_id(
        ORDERBOOK_ORDER_ESCROW_ID_DOMAIN_V1,
        ORDERBOOK_ORDER_ESCROW_ID_PREFIX_V1,
        order_id,
    )
}

/// Derive the native asset-lock identifier required for a settlement channel.
///
/// A receipt can move funds only from the generic native asset lock at this
/// identifier. The lock itself binds the buyer/funder, provider destination,
/// asset definition, settlement release authority, and remaining custody.
#[must_use]
pub fn orderbook_settlement_escrow_id(channel_id: [u8; 32]) -> EscrowId {
    namespaced_orderbook_escrow_id(
        ORDERBOOK_SETTLEMENT_ESCROW_ID_DOMAIN_V1,
        ORDERBOOK_SETTLEMENT_ESCROW_ID_PREFIX_V1,
        channel_id,
    )
}

/// Return whether `escrow_id` belongs to the reserved bid-order namespace.
#[must_use]
pub fn is_orderbook_order_escrow_id_v1(escrow_id: &EscrowId) -> bool {
    escrow_id
        .as_hash()
        .as_ref()
        .starts_with(&ORDERBOOK_ORDER_ESCROW_ID_PREFIX_V1)
}

/// Return whether `escrow_id` belongs to the reserved settlement-channel namespace.
#[must_use]
pub fn is_orderbook_settlement_escrow_id_v1(escrow_id: &EscrowId) -> bool {
    escrow_id
        .as_hash()
        .as_ref()
        .starts_with(&ORDERBOOK_SETTLEMENT_ESCROW_ID_PREFIX_V1)
}

/// Return whether public escrow creation must reject `escrow_id`.
#[must_use]
pub fn is_reserved_orderbook_escrow_id_v1(escrow_id: &EscrowId) -> bool {
    is_orderbook_order_escrow_id_v1(escrow_id) || is_orderbook_settlement_escrow_id_v1(escrow_id)
}

/// Governance-controlled order admission and receipt-retention policy.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookAdmissionPolicyV1 {
    /// Schema version; must equal [`ORDERBOOK_ADMISSION_POLICY_VERSION_V1`].
    pub version: u16,
    /// Monotonic policy revision, beginning at one.
    pub revision: u64,
    /// Digest of the immediately preceding policy, absent only for revision one.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub predecessor_policy_digest: Option<[u8; 32]>,
    /// Non-zero governance market identifier, immutable after first activation.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub market_id: [u8; 32],
    /// Exact account authorized to execute bounded matching and maintenance.
    ///
    /// Rotation is committed by a predecessor-linked policy revision.
    pub matcher_authority: AccountId,
    /// Exact account bound as release authority for newly created settlement
    /// channels and authorized to record their provider-signed receipts.
    ///
    /// Rotation is committed by a predecessor-linked policy revision after
    /// all channels bound to the preceding authority have closed.
    pub settlement_authority: AccountId,
    /// Whether new order submissions are paused.
    pub paused: bool,
    /// Smallest accepted order quantity in GiB.
    pub min_order_gib: u64,
    /// Largest accepted order quantity in GiB.
    pub max_order_gib: u64,
    /// Accepted price increment in micro-XOR per GiB.
    pub price_tick_micro_xor: u64,
    /// Largest accepted maker fee in basis points.
    pub max_maker_fee_bps: u16,
    /// Largest accepted taker fee in basis points.
    pub max_taker_fee_bps: u16,
    /// Maximum interval from admission to order expiry.
    pub max_order_lifetime_secs: u64,
    /// Maximum age of a signed settlement receipt at admission.
    pub max_receipt_age_secs: u64,
    /// Maximum tolerated future skew for a signed receipt timestamp.
    pub max_clock_skew_secs: u64,
    /// Maximum delivered bytes admitted in one receipt.
    pub max_receipt_bytes: u64,
    /// Maximum non-overlapping receipt ranges retained for one channel.
    pub max_receipts_per_channel: u32,
}

impl OrderbookAdmissionPolicyV1 {
    /// Validate all first-release policy bounds.
    ///
    /// # Errors
    ///
    /// Returns [`OrderbookPolicyValidationError`] for unsupported versions,
    /// invalid revision links, zero identifiers, or out-of-range limits.
    pub fn validate(&self) -> Result<(), OrderbookPolicyValidationError> {
        if self.version != ORDERBOOK_ADMISSION_POLICY_VERSION_V1 {
            return Err(OrderbookPolicyValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.revision == 0 {
            return Err(OrderbookPolicyValidationError::ZeroRevision);
        }
        match (self.revision, self.predecessor_policy_digest) {
            (1, None) => {}
            (1, Some(_)) => return Err(OrderbookPolicyValidationError::UnexpectedPredecessor),
            (_, Some(digest)) if digest != [0; 32] => {}
            _ => return Err(OrderbookPolicyValidationError::MissingPredecessor),
        }
        if self.market_id == [0; 32] {
            return Err(OrderbookPolicyValidationError::ZeroMarketId);
        }
        if self.min_order_gib == 0 || self.max_order_gib < self.min_order_gib {
            return Err(OrderbookPolicyValidationError::InvalidQuantityBounds {
                minimum: self.min_order_gib,
                maximum: self.max_order_gib,
            });
        }
        if self.price_tick_micro_xor == 0 {
            return Err(OrderbookPolicyValidationError::ZeroPriceTick);
        }
        if self.max_maker_fee_bps > 10_000 || self.max_taker_fee_bps > 10_000 {
            return Err(OrderbookPolicyValidationError::InvalidFeeBounds {
                maker_bps: self.max_maker_fee_bps,
                taker_bps: self.max_taker_fee_bps,
            });
        }
        if !(1..=ORDERBOOK_MAX_ORDER_LIFETIME_SECS_V1).contains(&self.max_order_lifetime_secs) {
            return Err(OrderbookPolicyValidationError::InvalidOrderLifetime {
                found: self.max_order_lifetime_secs,
            });
        }
        if !(1..=ORDERBOOK_MAX_RECEIPT_AGE_SECS_V1).contains(&self.max_receipt_age_secs) {
            return Err(OrderbookPolicyValidationError::InvalidReceiptAge {
                found: self.max_receipt_age_secs,
            });
        }
        if self.max_clock_skew_secs > ORDERBOOK_MAX_CLOCK_SKEW_SECS_V1 {
            return Err(OrderbookPolicyValidationError::InvalidClockSkew {
                found: self.max_clock_skew_secs,
            });
        }
        if !(1..=ORDERBOOK_MAX_RECEIPT_BYTES_V1).contains(&self.max_receipt_bytes) {
            return Err(OrderbookPolicyValidationError::InvalidReceiptBytes {
                found: self.max_receipt_bytes,
            });
        }
        if !(1..=ORDERBOOK_MAX_RECEIPTS_PER_CHANNEL_V1).contains(&self.max_receipts_per_channel) {
            return Err(OrderbookPolicyValidationError::InvalidReceiptCount {
                found: self.max_receipts_per_channel,
            });
        }
        Ok(())
    }

    /// Compute the canonical domain-separated digest of this policy.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical serialization fails.
    pub fn digest(&self) -> Result<[u8; 32], norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(ORDERBOOK_ADMISSION_POLICY_DIGEST_DOMAIN_V1);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }
}

/// Validation errors for the governed orderbook policy.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum OrderbookPolicyValidationError {
    /// Unsupported schema version.
    #[error("unsupported orderbook admission policy version {found}")]
    UnsupportedVersion {
        /// Version carried by the policy.
        found: u16,
    },
    /// Revision zero is invalid.
    #[error("orderbook admission policy revision must be non-zero")]
    ZeroRevision,
    /// Revision one unexpectedly carries a predecessor.
    #[error("orderbook admission policy revision one must not carry a predecessor")]
    UnexpectedPredecessor,
    /// A later revision lacks a non-zero predecessor digest.
    #[error("orderbook admission policy revision after one requires a non-zero predecessor")]
    MissingPredecessor,
    /// Market identifier is all zeroes.
    #[error("orderbook market id must not be zero")]
    ZeroMarketId,
    /// Order quantity bounds are empty or inverted.
    #[error("invalid orderbook quantity bounds {minimum}..={maximum}")]
    InvalidQuantityBounds {
        /// Configured minimum.
        minimum: u64,
        /// Configured maximum.
        maximum: u64,
    },
    /// Price tick is zero.
    #[error("orderbook price tick must be non-zero")]
    ZeroPriceTick,
    /// Fee bounds exceed 100 percent.
    #[error("invalid orderbook fee bounds maker={maker_bps} taker={taker_bps}")]
    InvalidFeeBounds {
        /// Maker fee ceiling.
        maker_bps: u16,
        /// Taker fee ceiling.
        taker_bps: u16,
    },
    /// Order lifetime is outside the hard first-release bound.
    #[error("invalid orderbook maximum order lifetime {found}")]
    InvalidOrderLifetime {
        /// Configured lifetime.
        found: u64,
    },
    /// Receipt age is outside the hard first-release bound.
    #[error("invalid orderbook maximum receipt age {found}")]
    InvalidReceiptAge {
        /// Configured age.
        found: u64,
    },
    /// Clock skew exceeds the hard first-release bound.
    #[error("invalid orderbook maximum clock skew {found}")]
    InvalidClockSkew {
        /// Configured skew.
        found: u64,
    },
    /// Per-receipt byte limit is invalid.
    #[error("invalid orderbook maximum receipt bytes {found}")]
    InvalidReceiptBytes {
        /// Configured byte limit.
        found: u64,
    },
    /// Per-channel receipt count is invalid.
    #[error("invalid orderbook maximum receipts per channel {found}")]
    InvalidReceiptCount {
        /// Configured count.
        found: u32,
    },
}

/// Activated governance policy together with ledger admission provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookAdmissionPolicyRecord {
    /// Policy body.
    pub policy: OrderbookAdmissionPolicyV1,
    /// Canonical digest of `policy`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Block timestamp at which the policy was activated.
    pub activated_at_unix: u64,
    /// Governance authority that activated the policy.
    pub activated_by: AccountId,
}

/// Authoritative lifecycle of an admitted order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", rename_all = "snake_case")
)]
pub enum OrderbookOrderStatusV1 {
    /// Order has not yet received a fill.
    Open,
    /// Order has received at least one fill and retains quantity.
    PartiallyFilled,
    /// Order has no remaining quantity.
    Filled,
    /// Owner cancellation has been committed.
    Cancelled,
    /// Ledger maintenance retired the order after its signed expiry.
    Expired,
    /// Ledger maintenance retired an ask after its admitted provider binding was revoked.
    ProviderRevoked,
}

/// Native custody created atomically with one admitted bid.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookBidEscrowBindingV1 {
    /// Deterministic order-scoped native lock identifier.
    pub escrow_id: EscrowId,
    /// Exact governed asset definition locked at admission.
    pub asset_definition: AssetDefinitionId,
    /// Conservative full-order amount initially moved into custody.
    pub initial_xor_locked: XorQuantity,
}

/// Canonical signed order and its authoritative ledger status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookOrderRecord {
    /// Canonical order identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub order_id: [u8; 32],
    /// Canonical owner account.
    pub owner: AccountId,
    /// Exact canonical `sorafs_manifest::orderbook::OrderRequestV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_order: Vec<u8>,
    /// Policy digest against which the order was admitted.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub admitted_policy_digest: [u8; 32],
    /// Block timestamp assigned at admission.
    pub admitted_at_unix: u64,
    /// Monotonic sequence assigned by the authoritative ledger at admission.
    pub admission_sequence: u64,
    /// Quantity not yet filled.
    pub remaining_gib: u64,
    /// Atomic native custody binding for bids; absent for non-custodial asks.
    pub bid_escrow: Option<OrderbookBidEscrowBindingV1>,
    /// Provider registry identity bound at ask admission; absent for bids.
    pub provider_id: Option<ProviderId>,
    /// Current authoritative lifecycle status.
    pub status: OrderbookOrderStatusV1,
    /// Block timestamp of the latest lifecycle mutation.
    pub updated_at_unix: u64,
    /// Exact canonical cancellation payload, present only after cancellation.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::base64_vec::option")
    )]
    pub canonical_cancel: Option<Vec<u8>>,
    /// Block timestamp assigned when cancellation was committed.
    pub cancelled_at_unix: Option<u64>,
    /// Active policy digest against which cancellation was admitted.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub cancelled_policy_digest: Option<[u8; 32]>,
}

/// Typed cancellation view returned by authoritative read queries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookCancellationRecord {
    /// Cancelled order identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub order_id: [u8; 32],
    /// Canonical owner of the cancelled order.
    pub owner: AccountId,
    /// Exact canonical signed cancellation payload.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_cancel: Vec<u8>,
    /// Block timestamp assigned to cancellation admission.
    pub cancelled_at_unix: u64,
    /// Policy digest active at cancellation admission.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub cancelled_policy_digest: [u8; 32],
}

/// Highest committed orderbook operation nonce for one ledger account.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookOwnerNonceRecord {
    /// Canonical account whose nonce namespace is tracked.
    pub owner: AccountId,
    /// Highest committed order or cancellation nonce.
    pub highest_nonce: u64,
}

/// Immutable accepted settlement receipt and ledger provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookSettlementReceiptRecord {
    /// Canonical receipt identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub receipt_id: [u8; 32],
    /// Settlement channel identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub channel_id: [u8; 32],
    /// Trade identifier carried by the receipt.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub trade_id: [u8; 32],
    /// Exact canonical `sorafs_manifest::orderbook::SettlementReceiptV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_receipt: Vec<u8>,
    /// Active policy digest against which the receipt was admitted.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub admitted_policy_digest: [u8; 32],
    /// Block timestamp assigned at admission.
    pub admitted_at_unix: u64,
    /// Relayer account that submitted the provider-signed receipt.
    ///
    /// Relayers are audit provenance only and are not trusted receipt signers
    /// or native custody release authorities.
    pub recorded_by: AccountId,
}

/// Immutable authoritative trade produced by deterministic matching.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookTradeRecord {
    /// Canonical trade identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub trade_id: [u8; 32],
    /// Maker order identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub maker_order_id: [u8; 32],
    /// Taker order identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub taker_order_id: [u8; 32],
    /// Deterministic sequence in the authoritative trade log.
    pub trade_sequence: u64,
    /// Exact canonical `sorafs_manifest::orderbook::TradeEventV1` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_trade: Vec<u8>,
    /// Settlement channel derived from this trade.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub channel_id: [u8; 32],
    /// Book revision committed by the matching transition.
    pub book_revision: u64,
    /// Block timestamp assigned to the fill.
    pub recorded_at_unix: u64,
}

/// Authoritative settlement-channel lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", rename_all = "snake_case")
)]
pub enum OrderbookSettlementChannelStatusV1 {
    /// The provider may submit signed delivery receipts.
    Open,
    /// All channel bytes and escrow were settled.
    Closed,
    /// The delivery deadline elapsed and remaining custody was refunded.
    Expired,
}

/// Authoritative settlement-channel state bound to native custody.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookSettlementChannelRecord {
    /// Settlement channel identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub channel_id: [u8; 32],
    /// Trade funded by this channel.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub trade_id: [u8; 32],
    /// Buyer whose admitted bid funded custody.
    pub buyer: AccountId,
    /// Provider account entitled to sign receipts and receive settlement.
    pub provider: AccountId,
    /// Provider registry identifier selected deterministically at matching.
    pub provider_id: ProviderId,
    /// Account bound as native release authority for the channel lock.
    pub settlement_authority: AccountId,
    /// Total byte capacity created by the fill.
    pub total_bytes: u64,
    /// Bytes not yet covered by accepted receipts.
    pub remaining_bytes: u64,
    /// Initial XOR partitioned from bid custody.
    pub initial_xor_locked: XorQuantity,
    /// XOR still held by channel custody.
    pub remaining_xor_locked: XorQuantity,
    /// Immutable maker-plus-taker fee custody derived from the trade.
    pub initial_fee_xor_locked: XorQuantity,
    /// Trade-derived fee custody not yet paid to treasury.
    pub remaining_fee_xor_locked: XorQuantity,
    /// Current channel lifecycle.
    pub status: OrderbookSettlementChannelStatusV1,
    /// Block timestamp assigned when matching opened the channel.
    pub opened_at_unix: u64,
    /// Inclusive channel delivery deadline.
    pub expires_at_unix: u64,
    /// Block timestamp of the latest channel mutation.
    pub updated_at_unix: u64,
}

/// One receipt range retained in a channel replay index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookSettlementRangeRecord {
    /// Receipt identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub receipt_id: [u8; 32],
    /// Inclusive byte-range start.
    pub start: u64,
    /// Exclusive byte-range end.
    pub end: u64,
    /// Signed receipt issuance time.
    pub issued_at_unix: u64,
}

/// Bounded, strictly range-ordered receipt replay index for one channel.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookSettlementIndexRecord {
    /// Settlement channel identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub channel_id: [u8; 32],
    /// Trade identifier shared by every indexed receipt.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub trade_id: [u8; 32],
    /// Non-overlapping ranges sorted by `(start, end, receipt_id)`.
    pub ranges: Vec<OrderbookSettlementRangeRecord>,
}

/// Constant-time authoritative orderbook ledger counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookLedgerStatusV1 {
    /// Number of unfilled open orders.
    pub open_orders: u64,
    /// Number of partially filled orders with remaining quantity.
    pub partially_filled_orders: u64,
    /// Number of fully filled terminal orders.
    pub filled_orders: u64,
    /// Number of terminal owner/governance cancellations.
    pub cancelled_orders: u64,
    /// Number of terminal expired orders.
    pub expired_orders: u64,
    /// Number of terminal asks retired after their admitted provider binding was revoked.
    pub provider_revoked_orders: u64,
    /// Number of immutable deterministic trades.
    pub trades: u64,
    /// Number of immutable settlement receipts.
    pub settlement_receipts: u64,
    /// Number of settlement channels created by fills.
    pub settlement_channels: u64,
    /// Number of open settlement channels.
    pub open_settlement_channels: u64,
    /// Revision of the authoritative book. Every order, fill, cancellation, or
    /// expiry mutation advances this value exactly once.
    pub book_revision: u64,
    /// Latest book revision exhaustively scanned by a bounded matcher.
    ///
    /// A value equal to `book_revision` seals a valid zero-fill or
    /// below-capacity terminal scan. Any order mutation or capped fill advances
    /// `book_revision` and makes another bounded pass eligible.
    pub last_match_scan_book_revision: u64,
    /// Next monotonic order admission sequence.
    pub next_admission_sequence: u64,
    /// Next monotonic trade sequence.
    pub next_trade_sequence: u64,
    /// Block timestamp of the most recent counter mutation.
    pub updated_at_unix: u64,
}

/// Finalized block anchor for one coherent orderbook query result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookFinalizedCursorV1 {
    /// Finalized block height observed by the immutable state view.
    pub height: u64,
    /// Finalized block hash resolved from that same state view.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
}

/// Cursor-bounded authoritative order page.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookOrderPageV1 {
    /// Finalized state anchor shared by every order in the page.
    pub finalized_cursor: OrderbookFinalizedCursorV1,
    /// Canonical records in ascending order-id order.
    pub orders: Vec<OrderbookOrderRecord>,
    /// Whether at least one further matching record exists.
    pub has_more: bool,
    /// Exclusive cursor for the next page, present only when `has_more` is true.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub next_after_order_id: Option<[u8; 32]>,
}

/// Cursor-bounded authoritative settlement-receipt page.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookSettlementReceiptPageV1 {
    /// Finalized state anchor shared by every receipt in the page.
    pub finalized_cursor: OrderbookFinalizedCursorV1,
    /// Canonical receipt records in ascending receipt-id order.
    pub receipts: Vec<OrderbookSettlementReceiptRecord>,
    /// Whether at least one further matching record exists.
    pub has_more: bool,
    /// Exclusive cursor for the next page, present only when `has_more` is true.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub next_after_receipt_id: Option<[u8; 32]>,
}

/// Cursor-bounded authoritative trade page.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookTradePageV1 {
    /// Finalized state anchor shared by every trade in the page.
    pub finalized_cursor: OrderbookFinalizedCursorV1,
    /// Canonical records in ascending trade-id order.
    pub trades: Vec<OrderbookTradeRecord>,
    /// Whether at least one further trade exists at this anchor.
    pub has_more: bool,
    /// Exclusive cursor for the next page, present only when `has_more` is true.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub next_after_trade_id: Option<[u8; 32]>,
}

/// Cursor-bounded authoritative settlement-channel page.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookSettlementChannelPageV1 {
    /// Finalized state anchor shared by every channel in the page.
    pub finalized_cursor: OrderbookFinalizedCursorV1,
    /// Canonical records in ascending channel-id order.
    pub channels: Vec<OrderbookSettlementChannelRecord>,
    /// Whether at least one further matching channel exists at this anchor.
    pub has_more: bool,
    /// Exclusive cursor for the next page, present only when `has_more` is true.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub next_after_channel_id: Option<[u8; 32]>,
}

/// Exclusive cursor for one committed orderbook event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookFinalizedEventCursorV1 {
    /// Monotonic orderbook-event sequence beginning at one.
    pub sequence: u64,
    /// Finalized block height containing the event.
    pub block_height: u64,
    /// Finalized block hash resolved only after the block commits.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Orderbook-event index within the committing block.
    pub event_index: u32,
}

/// Typed orderbook event with an unambiguous finalized-chain cursor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookFinalizedEventV1 {
    /// Monotonic orderbook-event sequence beginning at one.
    pub sequence: u64,
    /// Committing block height.
    pub block_height: u64,
    /// Committing block hash resolved from finalized state.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
    /// Orderbook-event index within the committing block.
    pub event_index: u32,
    /// Existing typed, payload-free native orderbook event.
    pub event: SorafsOrderbookLedgerEvent,
}

impl OrderbookFinalizedEventV1 {
    /// Return the exclusive cursor identifying this event.
    #[must_use]
    pub const fn cursor(&self) -> OrderbookFinalizedEventCursorV1 {
        OrderbookFinalizedEventCursorV1 {
            sequence: self.sequence,
            block_height: self.block_height,
            block_hash: self.block_hash,
            event_index: self.event_index,
        }
    }
}

/// Cursor-bounded page of typed committed orderbook events.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookFinalizedEventPageV1 {
    /// Finalized state anchor shared by every event in the page.
    pub finalized_cursor: OrderbookFinalizedCursorV1,
    /// Events in strictly increasing sequence and block/index order.
    pub events: Vec<OrderbookFinalizedEventV1>,
    /// Whether at least one later committed event exists at this anchor.
    pub has_more: bool,
    /// Exclusive continuation cursor, present only when `has_more` is true.
    pub next_after: Option<OrderbookFinalizedEventCursorV1>,
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::events::data::sorafs::SorafsOrderbookLedgerEventKind;

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
            .expect("nonzero deterministic Ed25519 seed");
        AccountId::new(keypair.public_key().clone())
    }

    fn assert_canonical_norito_round_trip<T>(value: &T)
    where
        T: core::fmt::Debug + PartialEq + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let encoded = norito::to_bytes(value).expect("encode canonical orderbook value");
        let decoded: T =
            norito::decode_from_bytes(&encoded).expect("decode canonical orderbook value");
        assert_eq!(&decoded, value);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode canonical orderbook value"),
            encoded
        );
    }

    fn policy() -> OrderbookAdmissionPolicyV1 {
        OrderbookAdmissionPolicyV1 {
            version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0xA5; 32],
            matcher_authority: account(0xB1),
            settlement_authority: account(0xB2),
            paused: false,
            min_order_gib: 1,
            max_order_gib: 1_024,
            price_tick_micro_xor: 10,
            max_maker_fee_bps: 100,
            max_taker_fee_bps: 200,
            max_order_lifetime_secs: 86_400,
            max_receipt_age_secs: 3_600,
            max_clock_skew_secs: 30,
            max_receipt_bytes: 1 << 30,
            max_receipts_per_channel: 1_024,
        }
    }

    #[test]
    fn policy_validation_and_digest_are_deterministic() {
        let policy = policy();
        policy.validate().expect("valid policy");
        let first = policy.digest().expect("digest policy");
        assert_eq!(first, policy.digest().expect("repeat digest"));

        let mut changed = policy.clone();
        changed.price_tick_micro_xor += 1;
        assert_ne!(first, changed.digest().expect("changed digest"));

        let mut rotated = policy;
        rotated.matcher_authority = account(0xB3);
        assert_ne!(
            first,
            rotated.digest().expect("rotated-authority policy digest")
        );
    }

    #[test]
    fn settlement_escrow_id_is_deterministic_and_channel_bound() {
        let channel = [0x42; 32];
        let first = orderbook_settlement_escrow_id(channel);
        assert_eq!(first, orderbook_settlement_escrow_id(channel));
        assert_ne!(first, orderbook_settlement_escrow_id([0x43; 32]));
        assert!(is_orderbook_settlement_escrow_id_v1(&first));
        assert!(is_reserved_orderbook_escrow_id_v1(&first));
        assert!(!is_orderbook_order_escrow_id_v1(&first));
        assert_ne!(first.as_hash().as_ref(), &[0; 32]);
    }

    #[test]
    fn order_and_channel_escrow_namespaces_are_reserved_and_disjoint() {
        let subject = [0x42; 32];
        let order = orderbook_order_escrow_id(subject);
        let channel = orderbook_settlement_escrow_id(subject);

        assert_eq!(order, orderbook_order_escrow_id(subject));
        assert_ne!(order, orderbook_order_escrow_id([0x43; 32]));
        assert_ne!(order, channel);
        assert!(is_orderbook_order_escrow_id_v1(&order));
        assert!(is_reserved_orderbook_escrow_id_v1(&order));
        assert!(!is_orderbook_settlement_escrow_id_v1(&order));
        assert_ne!(order.as_hash().as_ref(), &[0; 32]);
    }

    #[test]
    fn policy_rejects_revision_and_predecessor_abuse() {
        let mut candidate = policy();
        candidate.revision = 0;
        assert_eq!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::ZeroRevision)
        );

        let mut candidate = policy();
        candidate.predecessor_policy_digest = Some([1; 32]);
        assert_eq!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::UnexpectedPredecessor)
        );

        let mut candidate = policy();
        candidate.revision = 2;
        assert_eq!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::MissingPredecessor)
        );
        candidate.predecessor_policy_digest = Some([0; 32]);
        assert_eq!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::MissingPredecessor)
        );
    }

    #[test]
    fn policy_rejects_zero_and_inverted_bounds() {
        let mut candidate = policy();
        candidate.market_id = [0; 32];
        assert_eq!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::ZeroMarketId)
        );

        for (minimum, maximum) in [(0, 1), (2, 1)] {
            let mut candidate = policy();
            candidate.min_order_gib = minimum;
            candidate.max_order_gib = maximum;
            assert_eq!(
                candidate.validate(),
                Err(OrderbookPolicyValidationError::InvalidQuantityBounds { minimum, maximum })
            );
        }

        let mut candidate = policy();
        candidate.price_tick_micro_xor = 0;
        assert_eq!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::ZeroPriceTick)
        );
    }

    #[test]
    fn policy_rejects_resource_limit_extremes() {
        let mut candidate = policy();
        candidate.max_maker_fee_bps = 10_001;
        assert!(matches!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::InvalidFeeBounds { .. })
        ));

        let mut candidate = policy();
        candidate.max_order_lifetime_secs = ORDERBOOK_MAX_ORDER_LIFETIME_SECS_V1 + 1;
        assert!(matches!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::InvalidOrderLifetime { .. })
        ));

        let mut candidate = policy();
        candidate.max_receipt_age_secs = 0;
        assert!(matches!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::InvalidReceiptAge { .. })
        ));

        let mut candidate = policy();
        candidate.max_clock_skew_secs = ORDERBOOK_MAX_CLOCK_SKEW_SECS_V1 + 1;
        assert!(matches!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::InvalidClockSkew { .. })
        ));

        let mut candidate = policy();
        candidate.max_receipt_bytes = ORDERBOOK_MAX_RECEIPT_BYTES_V1 + 1;
        assert!(matches!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::InvalidReceiptBytes { .. })
        ));

        let mut candidate = policy();
        candidate.max_receipts_per_channel = ORDERBOOK_MAX_RECEIPTS_PER_CHANNEL_V1 + 1;
        assert!(matches!(
            candidate.validate(),
            Err(OrderbookPolicyValidationError::InvalidReceiptCount { .. })
        ));
    }

    #[test]
    fn finalized_query_pages_round_trip_as_exact_canonical_norito() {
        let finalized_cursor = OrderbookFinalizedCursorV1 {
            height: 7,
            block_hash: [0x71; 32],
        };
        let event = OrderbookFinalizedEventV1 {
            sequence: 11,
            block_height: 7,
            block_hash: finalized_cursor.block_hash,
            event_index: 3,
            event: SorafsOrderbookLedgerEvent {
                kind: SorafsOrderbookLedgerEventKind::TradeMatched,
                order_id: Some([0x11; 32]),
                trade_id: Some([0x22; 32]),
                channel_id: Some([0x33; 32]),
                receipt_id: None,
                provider_id: Some(ProviderId::new([0x44; 32])),
                book_revision: 9,
                authority: account(0x51),
                occurred_at_unix_ms: 12_345,
            },
        };
        let event_cursor = event.cursor();
        let event_page = OrderbookFinalizedEventPageV1 {
            finalized_cursor,
            events: vec![event],
            has_more: true,
            next_after: Some(event_cursor),
        };
        let order_page = OrderbookOrderPageV1 {
            finalized_cursor,
            orders: Vec::new(),
            has_more: false,
            next_after_order_id: None,
        };
        let receipt_page = OrderbookSettlementReceiptPageV1 {
            finalized_cursor,
            receipts: Vec::new(),
            has_more: false,
            next_after_receipt_id: None,
        };
        let trade_page = OrderbookTradePageV1 {
            finalized_cursor,
            trades: Vec::new(),
            has_more: false,
            next_after_trade_id: None,
        };
        let channel_page = OrderbookSettlementChannelPageV1 {
            finalized_cursor,
            channels: Vec::new(),
            has_more: false,
            next_after_channel_id: None,
        };

        assert_canonical_norito_round_trip(&finalized_cursor);
        assert_canonical_norito_round_trip(&event_cursor);
        assert_canonical_norito_round_trip(&event_page);
        assert_canonical_norito_round_trip(&order_page);
        assert_canonical_norito_round_trip(&receipt_page);
        assert_canonical_norito_round_trip(&trade_page);
        assert_canonical_norito_round_trip(&channel_page);

        #[cfg(feature = "json")]
        {
            let encoded =
                norito::json::to_vec(&event_page).expect("encode finalized event page JSON");
            let decoded: OrderbookFinalizedEventPageV1 =
                norito::json::from_slice(&encoded).expect("decode finalized event page JSON");
            assert_eq!(decoded, event_page);
        }
    }
}
