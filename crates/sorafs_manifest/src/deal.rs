#![allow(unexpected_cfgs)]

//! Deal, bond, and micropayment schemas for the SoraFS incentives engine.
//!
//! These payloads describe the lifecycle of storage and retrieval agreements
//! tracked under the SF-8 “Deal Engine & Incentives” roadmap item. They enable
//! deterministic Norito encoding for agreement terms, probabilistic
//! micropayment receipts, and audit-driven settlement records.

use std::collections::HashSet;

#[cfg(test)]
use iroha_crypto::numeric::Quantity;
use norito::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

pub use iroha_crypto::numeric::{
    XOR_QUANTITY_SCALE, XorQuantity, XorQuantityError as DealAmountError,
};

/// Schema version for [`DealTermsV1`].
pub const DEAL_TERMS_VERSION_V1: u8 = 1;
/// Schema version for [`MicropaymentPolicyV1`].
pub const MICROPAYMENT_POLICY_VERSION_V1: u8 = 1;
/// Schema version for [`DealMicropaymentV1`].
pub const DEAL_MICROPAYMENT_VERSION_V1: u8 = 1;
/// Schema version for [`DealLedgerSnapshotV1`].
pub const DEAL_LEDGER_VERSION_V1: u8 = 1;
/// Schema version for [`DealSettlementV1`].
pub const DEAL_SETTLEMENT_VERSION_V1: u8 = 1;

/// Basis points per unit probability (10_000 = 100%).
pub const BASIS_POINTS_PER_UNIT: u16 = 10_000;
/// Legacy micro-XOR scale used only by exact migration adapters and fixtures.
pub const MICRO_XOR_PER_XOR: u128 = 1_000_000;

/// Probability and payout configuration for probabilistic micropayments.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct MicropaymentPolicyV1 {
    /// Schema version (`MICROPAYMENT_POLICY_VERSION_V1`).
    pub version: u8,
    /// Window length in seconds at which the deal engine evaluates payouts.
    pub window_secs: u32,
    /// Probability of emitting a payout per window (basis points, 10_000 = 100%).
    pub probability_bps: u16,
    /// Maximum exact XOR liability per window.
    pub max_window_liability: XorQuantity,
}

impl MicropaymentPolicyV1 {
    /// Validate policy constraints.
    pub fn validate(&self) -> Result<(), MicropaymentPolicyError> {
        if self.version != MICROPAYMENT_POLICY_VERSION_V1 {
            return Err(MicropaymentPolicyError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.window_secs == 0 {
            return Err(MicropaymentPolicyError::ZeroWindow);
        }
        if self.probability_bps > BASIS_POINTS_PER_UNIT {
            return Err(MicropaymentPolicyError::InvalidProbability {
                probability_bps: self.probability_bps,
            });
        }
        if self.max_window_liability.is_zero() {
            return Err(MicropaymentPolicyError::ZeroLiabilityCap);
        }
        Ok(())
    }
}

/// Deal metadata entry used for telemetry or policy hints.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct DealMetadataEntry {
    /// Metadata key (ASCII lowercase recommended).
    pub key: String,
    /// Metadata value.
    pub value: String,
}

impl DealMetadataEntry {
    /// Validate the metadata entry.
    fn validate(&self) -> Result<(), DealTermsValidationError> {
        if self.key.trim().is_empty() {
            return Err(DealTermsValidationError::InvalidMetadataKey);
        }
        if self.value.is_empty() {
            return Err(DealTermsValidationError::InvalidMetadataValue);
        }
        Ok(())
    }
}

/// Storage or retrieval agreement recorded by governance.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct DealTermsV1 {
    /// Schema version (`DEAL_TERMS_VERSION_V1`).
    pub version: u8,
    /// Unique deal identifier (BLAKE3-256 digest).
    pub deal_id: [u8; 32],
    /// Provider identifier backing the agreement.
    pub provider_id: [u8; 32],
    /// Client account identifier (I105 bytes).
    pub client_account: Vec<u8>,
    /// Chunker profile handle associated with the deal.
    pub profile_handle: String,
    /// Maximum GiB covered by the agreement.
    pub committed_gib: u64,
    /// Minimum retention window for stored content (seconds).
    pub min_duration_secs: u64,
    /// Maximum retention window for stored content (seconds).
    pub max_duration_secs: u64,
    /// XOR-denominated bond that remains locked for the lifetime of the deal.
    pub bond_amount: XorQuantity,
    /// Exact XOR price per GiB-month.
    pub price_per_gib_month: XorQuantity,
    /// Micropayment scheduling policy.
    pub micropayment: MicropaymentPolicyV1,
    /// Unix timestamp (seconds) indicating when the deal becomes active.
    pub valid_from: u64,
    /// Unix timestamp (seconds) indicating when the deal expires.
    pub valid_until: u64,
    /// Auxiliary metadata entries.
    #[norito(default)]
    pub metadata: Vec<DealMetadataEntry>,
}

impl DealTermsV1 {
    /// Validate the agreement against registry policy.
    pub fn validate(&self) -> Result<(), DealTermsValidationError> {
        if self.version != DEAL_TERMS_VERSION_V1 {
            return Err(DealTermsValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.deal_id.iter().all(|&byte| byte == 0) {
            return Err(DealTermsValidationError::InvalidDealId);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(DealTermsValidationError::InvalidProviderId);
        }
        if self.client_account.is_empty() {
            return Err(DealTermsValidationError::EmptyClientAccount);
        }
        if self.profile_handle.trim().is_empty() {
            return Err(DealTermsValidationError::EmptyProfileHandle);
        }
        if self.committed_gib == 0 {
            return Err(DealTermsValidationError::ZeroCommittedCapacity);
        }
        if self.min_duration_secs == 0 {
            return Err(DealTermsValidationError::ZeroMinDuration);
        }
        if self.max_duration_secs < self.min_duration_secs {
            return Err(DealTermsValidationError::InvalidDurationWindow);
        }
        if self.bond_amount.is_zero() {
            return Err(DealTermsValidationError::ZeroBondAmount);
        }
        if self.price_per_gib_month.is_zero() {
            return Err(DealTermsValidationError::ZeroPrice);
        }
        self.micropayment
            .validate()
            .map_err(DealTermsValidationError::Micropayment)?;
        if self.valid_until <= self.valid_from {
            return Err(DealTermsValidationError::InvalidValidityWindow);
        }
        let mut keys = HashSet::new();
        for entry in &self.metadata {
            entry.validate()?;
            if !keys.insert(entry.key.clone()) {
                return Err(DealTermsValidationError::DuplicateMetadataKey {
                    key: entry.key.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Micropayment issued for a successful storage window.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct DealMicropaymentV1 {
    /// Schema version (`DEAL_MICROPAYMENT_VERSION_V1`).
    pub version: u8,
    /// Associated deal identifier.
    pub deal_id: [u8; 32],
    /// Index of the micropayment window.
    pub window_index: u64,
    /// XOR amount transferred in this micropayment.
    pub amount: XorQuantity,
    /// Timestamp when the micropayment was issued.
    pub issued_at: u64,
    /// Deterministic proof binding the micropayment window (BLAKE3 hash).
    pub determinism_hint: [u8; 32],
}

impl DealMicropaymentV1 {
    /// Validates the micropayment payload.
    pub fn validate(&self) -> Result<(), DealMicropaymentValidationError> {
        if self.version != DEAL_MICROPAYMENT_VERSION_V1 {
            return Err(DealMicropaymentValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.deal_id.iter().all(|&byte| byte == 0) {
            return Err(DealMicropaymentValidationError::InvalidDealId);
        }
        if self.amount.is_zero() {
            return Err(DealMicropaymentValidationError::ZeroAmount);
        }
        if self.determinism_hint.iter().all(|&byte| byte == 0) {
            return Err(DealMicropaymentValidationError::MissingDeterminismHint);
        }
        Ok(())
    }
}

/// Provider/client ledger snapshot tracked for audit purposes.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct DealLedgerSnapshotV1 {
    /// Schema version (`DEAL_LEDGER_VERSION_V1`).
    pub version: u8,
    /// Deal identifier.
    pub deal_id: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Client identifier digest.
    pub client_id: [u8; 32],
    /// Total XOR credited to the provider so far (micro units).
    pub provider_accrual: XorQuantity,
    /// Total XOR debited from the client (micro units).
    pub client_liability: XorQuantity,
    /// Remaining locked bond amount.
    pub bond_locked: XorQuantity,
    /// Total XOR slashed from the bond.
    pub bond_slashed: XorQuantity,
    /// Timestamp when the snapshot was recorded.
    pub captured_at: u64,
}

impl DealLedgerSnapshotV1 {
    /// Validate snapshot invariants.
    pub fn validate(&self) -> Result<(), DealLedgerValidationError> {
        if self.version != DEAL_LEDGER_VERSION_V1 {
            return Err(DealLedgerValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.deal_id.iter().all(|&byte| byte == 0) {
            return Err(DealLedgerValidationError::InvalidDealId);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(DealLedgerValidationError::InvalidProviderId);
        }
        if self.client_id.iter().all(|&byte| byte == 0) {
            return Err(DealLedgerValidationError::InvalidClientId);
        }
        if self.provider_accrual > self.client_liability {
            return Err(DealLedgerValidationError::ProviderExceedsClient);
        }
        Ok(())
    }
}

/// Settlement record emitted when a deal completes or is slashed.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct DealSettlementV1 {
    /// Schema version (`DEAL_SETTLEMENT_VERSION_V1`).
    pub version: u8,
    /// Deal identifier.
    pub deal_id: [u8; 32],
    /// Final ledger state captured at settlement.
    pub ledger: DealLedgerSnapshotV1,
    /// Settlement status.
    pub status: DealSettlementStatusV1,
    /// Timestamp when the settlement occurred.
    pub settled_at: u64,
    /// Optional auditor rationale for slashing.
    #[norito(default)]
    pub audit_notes: Option<String>,
}

impl DealSettlementV1 {
    /// Validate settlement consistency.
    pub fn validate(&self) -> Result<(), DealSettlementValidationError> {
        if self.version != DEAL_SETTLEMENT_VERSION_V1 {
            return Err(DealSettlementValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        self.ledger
            .validate()
            .map_err(DealSettlementValidationError::Ledger)?;
        match self.status {
            DealSettlementStatusV1::Completed => {
                if !self.ledger.bond_slashed.is_zero() && self.audit_notes.is_none() {
                    return Err(DealSettlementValidationError::MissingAuditNotes);
                }
            }
            DealSettlementStatusV1::Slashed => {
                if self.audit_notes.is_none() {
                    return Err(DealSettlementValidationError::MissingAuditNotes);
                }
            }
            DealSettlementStatusV1::Cancelled => {}
        }
        Ok(())
    }
}

/// Settlement outcome.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub enum DealSettlementStatusV1 {
    /// Deal completed successfully.
    Completed,
    /// Deal was cancelled (bond unlocked, no slashing).
    Cancelled,
    /// Deal was slashed following an audit.
    Slashed,
}

/// Errors raised during micropayment policy validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum MicropaymentPolicyError {
    /// Unsupported schema version.
    #[error("unsupported micropayment policy version {found}")]
    UnsupportedVersion { found: u8 },
    /// Window duration must be > 0.
    #[error("micropayment window must be non-zero")]
    ZeroWindow,
    /// Probability must be within the 0..=10_000 range.
    #[error("probability {probability_bps} bps is outside 0..=10_000")]
    InvalidProbability { probability_bps: u16 },
    /// Liability cap must be non-zero.
    #[error("max window liability must be non-zero")]
    ZeroLiabilityCap,
}

/// Validation errors for [`DealTermsV1`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DealTermsValidationError {
    #[error("unsupported deal terms version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("deal identifier must not be zero")]
    InvalidDealId,
    #[error("provider identifier must not be zero")]
    InvalidProviderId,
    #[error("client account must not be empty")]
    EmptyClientAccount,
    #[error("profile handle must not be empty")]
    EmptyProfileHandle,
    #[error("committed capacity must be non-zero")]
    ZeroCommittedCapacity,
    #[error("minimum duration must be non-zero")]
    ZeroMinDuration,
    #[error("max duration must be >= min duration")]
    InvalidDurationWindow,
    #[error("bond amount must be non-zero")]
    ZeroBondAmount,
    #[error("price must be non-zero")]
    ZeroPrice,
    #[error("micropayment policy invalid: {0}")]
    Micropayment(#[from] MicropaymentPolicyError),
    #[error("valid until must be greater than valid from")]
    InvalidValidityWindow,
    #[error("metadata key must not be empty")]
    InvalidMetadataKey,
    #[error("metadata value must not be empty")]
    InvalidMetadataValue,
    #[error("duplicate metadata key {key}")]
    DuplicateMetadataKey { key: String },
}

/// Validation errors for [`DealMicropaymentV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DealMicropaymentValidationError {
    #[error("unsupported micropayment version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("deal identifier must not be zero")]
    InvalidDealId,
    #[error("micropayment amount must be > 0")]
    ZeroAmount,
    #[error("determinism hint must not be zero")]
    MissingDeterminismHint,
}

/// Validation errors for [`DealLedgerSnapshotV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DealLedgerValidationError {
    #[error("unsupported ledger snapshot version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("deal identifier must not be zero")]
    InvalidDealId,
    #[error("provider identifier must not be zero")]
    InvalidProviderId,
    #[error("client identifier must not be zero")]
    InvalidClientId,
    #[error("provider accrual exceeds client liability")]
    ProviderExceedsClient,
}

/// Validation errors for [`DealSettlementV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DealSettlementValidationError {
    #[error("unsupported settlement version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("ledger validation failed: {0}")]
    Ledger(DealLedgerValidationError),
    #[error("audit notes must be supplied when a bond is slashed")]
    MissingAuditNotes,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_terms() -> DealTermsV1 {
        DealTermsV1 {
            version: DEAL_TERMS_VERSION_V1,
            deal_id: [0xAA; 32],
            provider_id: [0xBB; 32],
            client_account: vec![0x01, 0x55, 0x01],
            profile_handle: "sorafs.sf1@1.0.0".to_string(),
            committed_gib: 256,
            min_duration_secs: 86_400,
            max_duration_secs: 86_400 * 30,
            bond_amount: XorQuantity::try_from_micro(10_000_000)
                .expect("legacy micro-XOR value is representable"),
            price_per_gib_month: XorQuantity::try_from_micro(42_000)
                .expect("legacy fixture value is representable"),
            micropayment: MicropaymentPolicyV1 {
                version: MICROPAYMENT_POLICY_VERSION_V1,
                window_secs: 3_600,
                probability_bps: 2_500,
                max_window_liability: XorQuantity::try_from_micro(1_000_000)
                    .expect("legacy micro-XOR value is representable"),
            },
            valid_from: 1_700_000_000,
            valid_until: 1_700_086_400,
            metadata: vec![DealMetadataEntry {
                key: "region".to_string(),
                value: "eu-west".to_string(),
            }],
        }
    }

    #[test]
    fn xor_quantity_checked_add_overflow() {
        let lhs: XorQuantity =
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
                .parse()
                .expect("maximum positive numeric value");
        let rhs: XorQuantity = "1".parse().expect("canonical XOR quantity");
        let err = lhs.checked_add(&rhs).expect_err("overflow");
        assert_eq!(err, DealAmountError::Overflow);
    }

    #[test]
    fn xor_quantity_legacy_micro_projection_is_exact_and_checked() {
        let sub_micro: XorQuantity = "0.0000001".parse().expect("canonical quantity");
        assert_eq!(
            sub_micro.try_to_micro(),
            Err(DealAmountError::InexactMicroProjection)
        );

        let too_wide: XorQuantity = "340282366920938463463374607431768211456"
            .parse()
            .expect("value fits the 512-bit quantity domain");
        assert_eq!(too_wide.try_to_micro(), Err(DealAmountError::Overflow));
    }

    #[test]
    fn xor_quantity_rejects_negative_input_without_relabeling_it_overflow() {
        assert_eq!(
            "-1".parse::<XorQuantity>(),
            Err(DealAmountError::NegativeQuantity)
        );
    }

    #[test]
    fn xor_quantity_enforces_nine_digit_scale_at_every_decode_boundary() {
        let nano: XorQuantity = "0.000000001".parse().expect("nano-XOR is canonical");
        assert_eq!(nano.to_string(), "0.000000001");

        let too_precise = "0.0000000001".parse::<Quantity>().expect("valid quantity");
        assert_eq!(
            XorQuantity::try_from_quantity(too_precise.clone()),
            Err(DealAmountError::ScaleOverflow { scale: 10, max: 9 })
        );
        assert_eq!(
            "0.0000000001".parse::<XorQuantity>(),
            Err(DealAmountError::ScaleOverflow { scale: 10, max: 9 })
        );
        assert!(norito::json::from_str::<XorQuantity>("\"0.0000000001\"").is_err());

        let bytes = norito::to_bytes(&too_precise).expect("encode raw quantity");
        assert!(norito::decode_from_bytes::<XorQuantity>(&bytes).is_err());
    }

    #[test]
    fn xor_quantity_roundtrips_with_canonical_string_json() {
        let amount: XorQuantity = "1.25".parse().expect("canonical quantity");
        let json = norito::json::to_string(&amount).expect("serialize JSON");
        assert_eq!(json, "\"1.25\"");
        let decoded: XorQuantity = norito::json::from_str(&json).expect("deserialize JSON");
        assert_eq!(decoded, amount);

        let bytes = norito::to_bytes(&amount).expect("serialize Norito");
        let decoded = norito::decode_from_bytes::<XorQuantity>(&bytes).expect("decode Norito");
        assert_eq!(decoded, amount);
    }

    #[test]
    fn xor_amount_checked_sub_underflow() {
        let lhs = XorQuantity::try_from_micro(5).expect("legacy micro-XOR value is representable");
        let rhs = XorQuantity::try_from_micro(10).expect("legacy micro-XOR value is representable");
        let err = lhs.checked_sub(&rhs).expect_err("underflow");
        assert_eq!(err, DealAmountError::Underflow);
    }

    #[test]
    fn micropayment_policy_validation_bounds() {
        let mut policy = MicropaymentPolicyV1 {
            version: MICROPAYMENT_POLICY_VERSION_V1,
            window_secs: 900,
            probability_bps: 5_000,
            max_window_liability: XorQuantity::try_from_micro(1_000)
                .expect("legacy micro-XOR value is representable"),
        };
        policy.validate().expect("valid policy");

        policy.probability_bps = BASIS_POINTS_PER_UNIT + 1;
        assert!(matches!(
            policy.validate(),
            Err(MicropaymentPolicyError::InvalidProbability { .. })
        ));
    }

    #[test]
    fn xor_quantity_min_and_checked_sub() {
        let larger =
            XorQuantity::try_from_micro(1_500).expect("legacy micro-XOR value is representable");
        let smaller =
            XorQuantity::try_from_micro(500).expect("legacy micro-XOR value is representable");
        assert_eq!(smaller, XorQuantity::min(&smaller, &larger));
        assert_eq!(
            smaller.checked_sub(&larger),
            Err(DealAmountError::Underflow)
        );
    }

    #[test]
    fn xor_amount_checked_mul_helpers() {
        let base =
            XorQuantity::try_from_micro(2_000).expect("legacy micro-XOR value is representable");
        let doubled = base
            .checked_mul_u64(2)
            .expect("multiplication within bounds");
        assert_eq!(
            doubled
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            4_000
        );

        let scaled = base
            .checked_mul_basis_points(2_500)
            .expect("basis-point scaling");
        // 2_000 * 0.25 = 500 micro
        assert_eq!(
            scaled
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            500
        );

        let maximum: XorQuantity =
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
                .parse()
                .expect("maximum positive numeric value");
        let overflow = maximum.checked_mul_u64(2);
        assert!(matches!(overflow, Err(DealAmountError::Overflow)));
    }

    #[test]
    fn basis_point_scaling_preserves_sub_micro_and_nano_amounts() {
        let one_micro: XorQuantity = "0.000001".parse().expect("canonical XOR quantity");
        assert_eq!(
            one_micro
                .checked_mul_basis_points(1_000)
                .expect("ten percent is representable")
                .to_string(),
            "0.0000001"
        );

        let one_tenth_micro: XorQuantity = "0.00000001".parse().expect("canonical XOR quantity");
        assert_eq!(
            one_tenth_micro
                .checked_mul_basis_points(1_000)
                .expect("nano-XOR result is representable")
                .to_string(),
            "0.000000001"
        );
    }

    #[test]
    fn deal_terms_validation_succeeds() {
        let terms = sample_terms();
        terms.validate().expect("valid terms");
    }

    #[test]
    fn deal_terms_rejects_duplicate_metadata() {
        let mut terms = sample_terms();
        terms.metadata.push(DealMetadataEntry {
            key: "region".to_string(),
            value: "us-east".to_string(),
        });
        let err = terms.validate().expect_err("duplicate key");
        matches!(err, DealTermsValidationError::DuplicateMetadataKey { .. });
    }

    #[test]
    fn deal_micropayment_validation() {
        let micropayment = DealMicropaymentV1 {
            version: DEAL_MICROPAYMENT_VERSION_V1,
            deal_id: [0xAA; 32],
            window_index: 42,
            amount: XorQuantity::try_from_micro(10_000)
                .expect("legacy micro-XOR value is representable"),
            issued_at: 1_700_000_100,
            determinism_hint: [0x11; 32],
        };
        micropayment.validate().expect("valid micropayment");
    }

    #[test]
    fn ledger_snapshot_validation() {
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: [0xAA; 32],
            provider_id: [0xBB; 32],
            client_id: [0xCC; 32],
            provider_accrual: XorQuantity::try_from_micro(500)
                .expect("legacy micro-XOR value is representable"),
            client_liability: XorQuantity::try_from_micro(500)
                .expect("legacy micro-XOR value is representable"),
            bond_locked: XorQuantity::try_from_micro(1_000_000)
                .expect("legacy micro-XOR value is representable"),
            bond_slashed: XorQuantity::zero(),
            captured_at: 1_700_000_050,
        };
        ledger.validate().expect("valid ledger");
    }

    #[test]
    fn ledger_and_settlement_validation_preserve_sub_micro_precision() {
        let one_tenth_micro: XorQuantity =
            "0.0000001".parse().expect("canonical sub-micro quantity");
        let two_tenths_micro: XorQuantity =
            "0.0000002".parse().expect("canonical sub-micro quantity");
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: [0xAA; 32],
            provider_id: [0xBB; 32],
            client_id: [0xCC; 32],
            provider_accrual: two_tenths_micro.clone(),
            client_liability: one_tenth_micro.clone(),
            bond_locked: two_tenths_micro,
            bond_slashed: one_tenth_micro,
            captured_at: 1_700_000_050,
        };
        assert_eq!(
            ledger.validate(),
            Err(DealLedgerValidationError::ProviderExceedsClient)
        );

        let mut settlement_ledger = ledger;
        settlement_ledger.provider_accrual = XorQuantity::zero();
        let settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            deal_id: settlement_ledger.deal_id,
            ledger: settlement_ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at: 1_700_000_051,
            audit_notes: None,
        };
        assert_eq!(
            settlement.validate(),
            Err(DealSettlementValidationError::MissingAuditNotes)
        );
    }

    #[test]
    fn settlement_requires_audit_notes_when_slashed() {
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: [0xAA; 32],
            provider_id: [0xBB; 32],
            client_id: [0xCC; 32],
            provider_accrual: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            client_liability: XorQuantity::try_from_micro(200)
                .expect("legacy micro-XOR value is representable"),
            bond_locked: XorQuantity::try_from_micro(900_000)
                .expect("legacy micro-XOR value is representable"),
            bond_slashed: XorQuantity::try_from_micro(100_000)
                .expect("legacy micro-XOR value is representable"),
            captured_at: 1_700_000_999,
        };
        let settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            deal_id: [0xAA; 32],
            ledger,
            status: DealSettlementStatusV1::Slashed,
            settled_at: 1_700_001_000,
            audit_notes: Some("failed PoR window".to_string()),
        };
        settlement.validate().expect("valid slashed settlement");
    }

    #[test]
    fn ledger_snapshot_rejects_zero_identifiers() {
        let mut ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: [0x11; 32],
            provider_id: [0x22; 32],
            client_id: [0x33; 32],
            provider_accrual: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            client_liability: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            bond_locked: XorQuantity::try_from_micro(1_000)
                .expect("legacy micro-XOR value is representable"),
            bond_slashed: XorQuantity::zero(),
            captured_at: 1_700_100_000,
        };
        ledger.validate().expect("valid ledger");

        ledger.provider_id = [0; 32];
        let err = ledger.validate().expect_err("missing provider id");
        matches!(err, DealLedgerValidationError::InvalidProviderId);

        ledger.provider_id = [0x22; 32];
        ledger.client_id = [0; 32];
        let err = ledger.validate().expect_err("missing client id");
        matches!(err, DealLedgerValidationError::InvalidClientId);
    }
}
