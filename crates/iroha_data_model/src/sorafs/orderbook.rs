//! Authoritative on-chain SoraFS orderbook policy and audit records (SFM-2).
//!
//! Signed order, cancellation, and settlement payload schemas remain in
//! `sorafs_manifest::orderbook`. These records bind those canonical payloads to
//! a governance-controlled policy and to deterministic ledger admission state.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{account::AccountId, escrow::EscrowId};

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
/// Hard ceiling for one authoritative orderbook read page.
pub const ORDERBOOK_QUERY_MAX_ITEMS_V1: u32 = 500;
/// Domain separator for policy digests.
pub const ORDERBOOK_ADMISSION_POLICY_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.orderbook.admission-policy.v1";
/// Domain separator for funded settlement locks derived from channel ids.
pub const ORDERBOOK_SETTLEMENT_ESCROW_ID_DOMAIN_V1: &[u8] =
    b"sorafs.orderbook.settlement-escrow-id.v1";

/// Derive the native asset-lock identifier required for a settlement channel.
///
/// A receipt can move funds only from the generic native asset lock at this
/// identifier. The lock itself binds the buyer/funder, provider destination,
/// asset definition, settlement release authority, and remaining custody.
#[must_use]
pub fn orderbook_settlement_escrow_id(channel_id: [u8; 32]) -> EscrowId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(ORDERBOOK_SETTLEMENT_ESCROW_ID_DOMAIN_V1);
    hasher.update(&channel_id);
    EscrowId::new(iroha_crypto::Hash::prehashed(*hasher.finalize().as_bytes()))
}

/// Governance-controlled order admission and receipt-retention policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
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
    /// Order is available to an authoritative matcher.
    Open,
    /// Owner cancellation has been committed.
    Cancelled,
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
    /// Current authoritative lifecycle status.
    pub status: OrderbookOrderStatusV1,
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
    /// Permissioned settlement authority that recorded the receipt.
    pub recorded_by: AccountId,
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
    /// Number of currently open admitted orders.
    pub open_orders: u64,
    /// Number of terminal owner/governance cancellations.
    pub cancelled_orders: u64,
    /// Number of immutable settlement receipts.
    pub settlement_receipts: u64,
    /// Number of channels with at least one admitted receipt.
    pub settlement_channels: u64,
    /// Block timestamp of the most recent counter mutation.
    pub updated_at_unix: u64,
}

/// Cursor-bounded authoritative order page.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OrderbookOrderPageV1 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> OrderbookAdmissionPolicyV1 {
        OrderbookAdmissionPolicyV1 {
            version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0xA5; 32],
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

        let mut changed = policy;
        changed.price_tick_micro_xor += 1;
        assert_ne!(first, changed.digest().expect("changed digest"));
    }

    #[test]
    fn settlement_escrow_id_is_deterministic_and_channel_bound() {
        let channel = [0x42; 32];
        let first = orderbook_settlement_escrow_id(channel);
        assert_eq!(first, orderbook_settlement_escrow_id(channel));
        assert_ne!(first, orderbook_settlement_escrow_id([0x43; 32]));
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
}
