#![allow(unexpected_cfgs)]

//! Deal, bond, and micropayment schemas for the SoraFS incentives engine.
//!
//! These payloads describe the lifecycle of storage and retrieval agreements
//! tracked under the SF-8 “Deal Engine & Incentives” roadmap item. They enable
//! deterministic Norito encoding for agreement terms, probabilistic
//! micropayment receipts, and audit-driven settlement records.

use std::collections::HashSet;

use iroha_schema::IntoSchema;
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

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

/// Number of micro-XOR units that make up a single whole XOR.
pub const MICRO_XOR_PER_XOR: u128 = 1_000_000;
/// Basis points per unit probability (10_000 = 100%).
pub const BASIS_POINTS_PER_UNIT: u16 = 10_000;

/// XOR-denominated amount expressed in micro units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, IntoSchema)]
pub struct XorAmount {
    micro: u128,
}

impl XorAmount {
    /// Construct from a micro-XOR quantity.
    #[must_use]
    pub const fn from_micro(micro: u128) -> Self {
        Self { micro }
    }

    /// Return the zero amount.
    #[must_use]
    pub const fn zero() -> Self {
        Self { micro: 0 }
    }

    /// Access the underlying micro-XOR value.
    #[must_use]
    pub const fn as_micro(self) -> u128 {
        self.micro
    }

    /// Add two amounts, returning an overflow error when the sum exceeds `u128`.
    pub fn checked_add(self, rhs: Self) -> Result<Self, DealAmountError> {
        self.micro
            .checked_add(rhs.micro)
            .map(Self::from_micro)
            .ok_or(DealAmountError::Overflow)
    }

    /// Subtract two amounts, returning an underflow error when `rhs > self`.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, DealAmountError> {
        self.micro
            .checked_sub(rhs.micro)
            .map(Self::from_micro)
            .ok_or(DealAmountError::Underflow)
    }

    /// Saturating subtraction helper.
    #[must_use]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        if self.micro <= rhs.micro {
            Self::zero()
        } else {
            Self::from_micro(self.micro - rhs.micro)
        }
    }

    /// Whether the amount is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.micro == 0
    }

    /// Return the minimum of the two amounts.
    #[must_use]
    pub const fn min(self, other: Self) -> Self {
        if self.micro <= other.micro {
            self
        } else {
            other
        }
    }

    /// Multiply the amount by an unsigned 64-bit scalar.
    ///
    /// # Errors
    ///
    /// Returns [`DealAmountError::Overflow`] if the product exceeds the `u128`
    /// capacity backing [`XorAmount`].
    pub fn checked_mul_u64(self, multiplier: u64) -> Result<Self, DealAmountError> {
        self.checked_mul_u128(u128::from(multiplier))
    }

    /// Multiply the amount by an unsigned 128-bit scalar.
    ///
    /// # Errors
    ///
    /// Returns [`DealAmountError::Overflow`] if the product exceeds the `u128`
    /// capacity backing [`XorAmount`].
    pub fn checked_mul_u128(self, multiplier: u128) -> Result<Self, DealAmountError> {
        self.micro
            .checked_mul(multiplier)
            .map(Self::from_micro)
            .ok_or(DealAmountError::Overflow)
    }

    /// Apply a basis-point ratio (`basis_points` / 10_000) to the amount.
    ///
    /// # Errors
    ///
    /// Returns [`DealAmountError::Overflow`] if the intermediate product exceeds
    /// the capacity of [`XorAmount`].
    pub fn checked_mul_basis_points(self, basis_points: u16) -> Result<Self, DealAmountError> {
        let numerator = u128::from(basis_points);
        let scaled = self
            .micro
            .checked_mul(numerator)
            .ok_or(DealAmountError::Overflow)?;
        Ok(Self::from_micro(scaled / u128::from(BASIS_POINTS_PER_UNIT)))
    }
}

impl From<u64> for XorAmount {
    fn from(value: u64) -> Self {
        Self::from_micro(u128::from(value))
    }
}

impl From<u128> for XorAmount {
    fn from(value: u128) -> Self {
        Self::from_micro(value)
    }
}

impl norito::json::JsonSerialize for XorAmount {
    fn json_serialize(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.as_micro(), out);
    }
}

impl norito::json::JsonDeserialize for XorAmount {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        u128::json_deserialize(parser).map(Self::from_micro)
    }
}

/// Errors raised while manipulating XOR-denominated amounts.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DealAmountError {
    /// Arithmetic overflow occurred.
    #[error("amount overflow")]
    Overflow,
    /// Arithmetic underflow occurred.
    #[error("amount underflow")]
    Underflow,
}

/// Probability and payout configuration for probabilistic micropayments.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct MicropaymentPolicyV1 {
    /// Schema version (`MICROPAYMENT_POLICY_VERSION_V1`).
    pub version: u8,
    /// Window length in seconds at which the deal engine evaluates payouts.
    pub window_secs: u32,
    /// Probability of emitting a payout per window (basis points, 10_000 = 100%).
    pub probability_bps: u16,
    /// Maximum XOR liability per window (micro-XOR units).
    pub max_window_liability: XorAmount,
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
    /// Client account identifier (IH58 bytes).
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
    pub bond_amount: XorAmount,
    /// Price expressed in micro-XOR per GiB-month.
    pub price_micro_per_gib_month: u64,
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
        if self.price_micro_per_gib_month == 0 {
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
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct DealMicropaymentV1 {
    /// Schema version (`DEAL_MICROPAYMENT_VERSION_V1`).
    pub version: u8,
    /// Associated deal identifier.
    pub deal_id: [u8; 32],
    /// Index of the micropayment window.
    pub window_index: u64,
    /// XOR amount transferred in this micropayment.
    pub amount: XorAmount,
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
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
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
    pub provider_accrual: XorAmount,
    /// Total XOR debited from the client (micro units).
    pub client_liability: XorAmount,
    /// Remaining locked bond amount.
    pub bond_locked: XorAmount,
    /// Total XOR slashed from the bond.
    pub bond_slashed: XorAmount,
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
        if self.provider_accrual.as_micro() > self.client_liability.as_micro() {
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
                if self.ledger.bond_slashed.as_micro() != 0 && self.audit_notes.is_none() {
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
            bond_amount: XorAmount::from_micro(10_000_000),
            price_micro_per_gib_month: 42_000,
            micropayment: MicropaymentPolicyV1 {
                version: MICROPAYMENT_POLICY_VERSION_V1,
                window_secs: 3_600,
                probability_bps: 2_500,
                max_window_liability: XorAmount::from_micro(1_000_000),
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
    fn xor_amount_checked_add_overflow() {
        let lhs = XorAmount::from_micro(u128::MAX - 5);
        let rhs = XorAmount::from_micro(10);
        let err = lhs.checked_add(rhs).expect_err("overflow");
        assert_eq!(err, DealAmountError::Overflow);
    }

    #[test]
    fn xor_amount_checked_sub_underflow() {
        let lhs = XorAmount::from_micro(5);
        let rhs = XorAmount::from_micro(10);
        let err = lhs.checked_sub(rhs).expect_err("underflow");
        assert_eq!(err, DealAmountError::Underflow);
    }

    #[test]
    fn micropayment_policy_validation_bounds() {
        let mut policy = MicropaymentPolicyV1 {
            version: MICROPAYMENT_POLICY_VERSION_V1,
            window_secs: 900,
            probability_bps: 5_000,
            max_window_liability: XorAmount::from_micro(1_000),
        };
        policy.validate().expect("valid policy");

        policy.probability_bps = BASIS_POINTS_PER_UNIT + 1;
        assert!(matches!(
            policy.validate(),
            Err(MicropaymentPolicyError::InvalidProbability { .. })
        ));
    }

    #[test]
    fn xor_amount_min_and_saturating_sub() {
        let larger = XorAmount::from_micro(1_500);
        let smaller = XorAmount::from_micro(500);
        assert_eq!(smaller, smaller.min(larger));
        assert_eq!(XorAmount::zero(), smaller.saturating_sub(larger));
    }

    #[test]
    fn xor_amount_checked_mul_helpers() {
        let base = XorAmount::from_micro(2_000);
        let doubled = base
            .checked_mul_u64(2)
            .expect("multiplication within bounds");
        assert_eq!(doubled.as_micro(), 4_000);

        let scaled = base
            .checked_mul_basis_points(2_500)
            .expect("basis-point scaling");
        // 2_000 * 0.25 = 500 micro
        assert_eq!(scaled.as_micro(), 500);

        let overflow = base.checked_mul_u128(u128::MAX);
        assert!(matches!(overflow, Err(DealAmountError::Overflow)));
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
            amount: XorAmount::from_micro(10_000),
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
            provider_accrual: XorAmount::from_micro(500),
            client_liability: XorAmount::from_micro(500),
            bond_locked: XorAmount::from_micro(1_000_000),
            bond_slashed: XorAmount::zero(),
            captured_at: 1_700_000_050,
        };
        ledger.validate().expect("valid ledger");
    }

    #[test]
    fn settlement_requires_audit_notes_when_slashed() {
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: [0xAA; 32],
            provider_id: [0xBB; 32],
            client_id: [0xCC; 32],
            provider_accrual: XorAmount::from_micro(100),
            client_liability: XorAmount::from_micro(200),
            bond_locked: XorAmount::from_micro(900_000),
            bond_slashed: XorAmount::from_micro(100_000),
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
            provider_accrual: XorAmount::from_micro(10),
            client_liability: XorAmount::from_micro(10),
            bond_locked: XorAmount::from_micro(1_000),
            bond_slashed: XorAmount::zero(),
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
