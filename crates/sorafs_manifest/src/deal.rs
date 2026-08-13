#![allow(unexpected_cfgs)]

//! Governed settlement, bond, and micropayment schemas for SoraFS economics.
//!
//! These payloads describe the lifecycle of storage and retrieval agreements
//! projected from native ledger state. They enable deterministic Norito
//! encoding for agreement terms, probabilistic micropayment receipts, and
//! audit-driven settlement records without introducing a second authority.
#[cfg(test)]
use iroha_crypto::numeric::Quantity;
use norito::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;
pub use iroha_crypto::numeric::{XOR_QUANTITY_SCALE, XorQuantity, XorQuantityError as DealAmountError};
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
/// Maximum UTF-8 byte length of settlement audit notes.
pub const MAX_DEAL_SETTLEMENT_AUDIT_NOTES_BYTES: usize = 1_024;
/// Maximum canonical I105 account byte length accepted in deal terms.
pub const MAX_DEAL_CLIENT_ACCOUNT_BYTES: usize = 512;
/// Maximum canonical chunker profile-handle byte length.
pub const MAX_DEAL_PROFILE_HANDLE_BYTES: usize = 128;
/// Maximum number of metadata entries in one deal.
pub const MAX_DEAL_METADATA_ENTRIES: usize = 64;
/// Maximum metadata-key byte length.
pub const MAX_DEAL_METADATA_KEY_BYTES: usize = 64;
/// Maximum metadata-value byte length.
pub const MAX_DEAL_METADATA_VALUE_BYTES: usize = 1_024;
/// Probability and payout configuration for probabilistic micropayments.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct MicropaymentPolicyV1 {
    /// Schema version (`MICROPAYMENT_POLICY_VERSION_V1`).
    pub version: u8,
    /// Window length in seconds at which the governed settlement policy evaluates payouts.
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
        if self.probability_bps == 0 || self.probability_bps > BASIS_POINTS_PER_UNIT {
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
    pub fn validate(&self) -> Result<(), DealTermsValidationError> {
        if self.key.is_empty()
            || self.key.len() > MAX_DEAL_METADATA_KEY_BYTES
            || !self.key.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
        {
            return Err(DealTermsValidationError::InvalidMetadataKey);
        }
        if self.value.is_empty()
            || self.value.len() > MAX_DEAL_METADATA_VALUE_BYTES
            || self.value != self.value.trim()
            || self.value.chars().any(char::is_control)
        {
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
    /// Derive the domain-separated identifier that binds every deal term.
    pub fn derive_deal_id(&self) -> Result<[u8; 32], DealTermsValidationError> {
        let mut canonical = self.clone();
        canonical.deal_id = [0; 32];
        let bytes = norito::to_bytes(&canonical)
            .map_err(|error| DealTermsValidationError::Serialization(error.to_string()))?;
        let encoded_len = u64::try_from(bytes.len())
            .map_err(|_| DealTermsValidationError::EncodedLengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"sorafs-deal-terms-v1");
        hasher.update(&encoded_len.to_le_bytes());
        hasher.update(&bytes);
        Ok(*hasher.finalize().as_bytes())
    }
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
        if self.client_account.len() > MAX_DEAL_CLIENT_ACCOUNT_BYTES {
            return Err(DealTermsValidationError::ClientAccountTooLong {
                length: self.client_account.len(),
                max: MAX_DEAL_CLIENT_ACCOUNT_BYTES,
            });
        }
        if self.profile_handle.is_empty() {
            return Err(DealTermsValidationError::EmptyProfileHandle);
        }
        if self.profile_handle.len() > MAX_DEAL_PROFILE_HANDLE_BYTES {
            return Err(DealTermsValidationError::ProfileHandleTooLong {
                length: self.profile_handle.len(),
                max: MAX_DEAL_PROFILE_HANDLE_BYTES,
            });
        }
        let descriptor = crate::chunker_registry::lookup_by_handle(&self.profile_handle)
            .ok_or_else(|| DealTermsValidationError::UnknownProfileHandle {
                handle: self.profile_handle.clone(),
            })?;
        let canonical_profile = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        if self.profile_handle != canonical_profile {
            return Err(DealTermsValidationError::NonCanonicalProfileHandle {
                provided: self.profile_handle.clone(),
                canonical: canonical_profile,
            });
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
        if u64::from(self.micropayment.window_secs) > self.max_duration_secs {
            return Err(DealTermsValidationError::MicropaymentWindowExceedsDeal);
        }
        if self.valid_from == 0 {
            return Err(DealTermsValidationError::InvalidValidFrom);
        }
        if self.valid_until <= self.valid_from {
            return Err(DealTermsValidationError::InvalidValidityWindow);
        }
        let duration = self
            .valid_until
            .checked_sub(self.valid_from)
            .ok_or(DealTermsValidationError::InvalidValidityWindow)?;
        if duration < self.min_duration_secs || duration > self.max_duration_secs {
            return Err(DealTermsValidationError::ValidityOutsideDurationWindow {
                duration,
                min: self.min_duration_secs,
                max: self.max_duration_secs,
            });
        }
        if self.metadata.len() > MAX_DEAL_METADATA_ENTRIES {
            return Err(DealTermsValidationError::TooManyMetadataEntries {
                count: self.metadata.len(),
                max: MAX_DEAL_METADATA_ENTRIES,
            });
        }
        let mut previous_key: Option<&str> = None;
        for entry in &self.metadata {
            entry.validate()?;
            if let Some(previous) = previous_key {
                if previous == entry.key {
                    return Err(DealTermsValidationError::DuplicateMetadataKey {
                        key: entry.key.clone(),
                    });
                }
                if previous > entry.key.as_str() {
                    return Err(DealTermsValidationError::MetadataNotSorted);
                }
            }
            previous_key = Some(&entry.key);
        }
        let expected_deal_id = self.derive_deal_id()?;
        if self.deal_id != expected_deal_id {
            return Err(DealTermsValidationError::DealIdMismatch {
                expected: expected_deal_id,
                found: self.deal_id,
            });
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
        if self.issued_at == 0 {
            return Err(DealMicropaymentValidationError::InvalidIssuedAt);
        }
        if self.determinism_hint.iter().all(|&byte| byte == 0) {
            return Err(DealMicropaymentValidationError::MissingDeterminismHint);
        }
        Ok(())
    }
    /// Validate the receipt against its exact deal policy and deterministic window.
    pub fn validate_against_terms(
        &self,
        terms: &DealTermsV1,
    ) -> Result<(), DealMicropaymentValidationError> {
        self.validate()?;
        terms
            .validate()
            .map_err(|error| DealMicropaymentValidationError::InvalidTerms(error.to_string()))?;
        if self.deal_id != terms.deal_id {
            return Err(DealMicropaymentValidationError::DealIdMismatch);
        }
        if self.amount > terms.micropayment.max_window_liability {
            return Err(DealMicropaymentValidationError::LiabilityCapExceeded);
        }
        let window_offset = self
            .window_index
            .checked_mul(u64::from(terms.micropayment.window_secs))
            .ok_or(DealMicropaymentValidationError::WindowArithmeticOverflow)?;
        let window_start = terms
            .valid_from
            .checked_add(window_offset)
            .ok_or(DealMicropaymentValidationError::WindowArithmeticOverflow)?;
        if window_start >= terms.valid_until {
            return Err(DealMicropaymentValidationError::WindowOutsideDeal);
        }
        let window_end = window_start
            .checked_add(u64::from(terms.micropayment.window_secs))
            .ok_or(DealMicropaymentValidationError::WindowArithmeticOverflow)?
            .min(terms.valid_until);
        if self.issued_at < window_end || self.issued_at > terms.valid_until {
            return Err(DealMicropaymentValidationError::IssuedOutsideWindow {
                issued_at: self.issued_at,
                window_end,
                deal_end: terms.valid_until,
            });
        }
        let expected_hint = derive_micropayment_hint(
            self.deal_id,
            self.window_index,
            &self.amount,
            self.issued_at,
        )?;
        if self.determinism_hint != expected_hint {
            return Err(DealMicropaymentValidationError::DeterminismHintMismatch);
        }
        Ok(())
    }
}
/// Derive the deterministic hash committed by a deal micropayment receipt.
///
/// # Errors
///
/// Returns an error if the exact canonical amount cannot be encoded or its
/// encoded length cannot be represented in the V1 hash preimage.
pub fn derive_micropayment_hint(
    deal_id: [u8; 32],
    window_index: u64,
    amount: &XorQuantity,
    issued_at: u64,
) -> Result<[u8; 32], DealMicropaymentValidationError> {
    let canonical_amount = norito::to_bytes(amount)
        .map_err(|error| DealMicropaymentValidationError::Serialization(error.to_string()))?;
    let encoded_len = u64::try_from(canonical_amount.len())
        .map_err(|_| DealMicropaymentValidationError::EncodedLengthOverflow)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs-deal-micropayment-v1");
    hasher.update(&deal_id);
    hasher.update(&window_index.to_le_bytes());
    hasher.update(&encoded_len.to_le_bytes());
    hasher.update(&canonical_amount);
    hasher.update(&issued_at.to_le_bytes());
    Ok(*hasher.finalize().as_bytes())
}
/// Provider/client ledger snapshot tracked for audit purposes.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct DealLedgerSnapshotV1 {
    /// Schema version (`DEAL_LEDGER_VERSION_V1`).
    pub version: u8,
    /// Domain-separated digest of this canonical snapshot with this field zeroed.
    pub snapshot_id: [u8; 32],
    /// One-based settlement sequence for this deal.
    pub sequence: u64,
    /// Exact predecessor snapshot, or `None` for sequence one.
    pub previous_snapshot_id: Option<[u8; 32]>,
    /// Deal identifier.
    pub deal_id: [u8; 32],
    /// Digest of the immutable negotiated terms used by the runtime.
    pub terms_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Client identifier digest.
    pub client_id: [u8; 32],
    /// Inclusive first epoch of the negotiated deal.
    pub deal_start_epoch: u64,
    /// Inclusive final usage epoch of the negotiated deal.
    pub deal_end_epoch: u64,
    /// Exact number of epochs in every settlement transition.
    pub settlement_window_epochs: u64,
    /// Previous settlement epoch anchoring this window.
    pub window_start_epoch: u64,
    /// Settlement epoch closing this window.
    pub window_end_epoch: u64,
    /// Total exact XOR credited to the provider so far.
    pub provider_accrual: XorQuantity,
    /// Total exact deterministic charge accrued by the client.
    pub client_liability: XorQuantity,
    /// Total winning-ticket credit generated for the provider.
    pub micropayment_credit_generated: XorQuantity,
    /// Total ticket credit applied to deterministic charges.
    pub micropayment_credit_applied: XorQuantity,
    /// Winning-ticket credit carried into a later window.
    pub micropayment_credit_carry: XorQuantity,
    /// Total client balance debited at settlement.
    pub client_debit: XorQuantity,
    /// Charge still outstanding after credit, debit, and slashing.
    pub outstanding_liability: XorQuantity,
    /// Immutable bond amount locked when the deal opened.
    pub bond_total: XorQuantity,
    /// Remaining locked bond amount.
    pub bond_locked: XorQuantity,
    /// Total XOR slashed from the bond.
    pub bond_slashed: XorQuantity,
    /// Total XOR released from the bond back to the provider.
    pub bond_released: XorQuantity,
    /// Deterministic charge added by this settlement window.
    pub window_expected_charge: XorQuantity,
    /// Winning-ticket credit generated in this window.
    pub window_micropayment_generated: XorQuantity,
    /// Ticket credit applied in this window.
    pub window_micropayment_applied: XorQuantity,
    /// Client balance debited in this window.
    pub window_client_debit: XorQuantity,
    /// Bond slashed in this window.
    pub window_bond_slashed: XorQuantity,
    /// Bond released in this window.
    pub window_bond_released: XorQuantity,
    /// Timestamp when the snapshot was recorded.
    pub captured_at: u64,
}
impl DealLedgerSnapshotV1 {
    /// Derive the domain-separated identifier of this exact snapshot.
    pub fn derive_snapshot_id(&self) -> Result<[u8; 32], DealLedgerValidationError> {
        let mut canonical = self.clone();
        canonical.snapshot_id = [0; 32];
        let bytes = norito::to_bytes(&canonical)
            .map_err(|error| DealLedgerValidationError::Serialization(error.to_string()))?;
        let encoded_len = u64::try_from(bytes.len())
            .map_err(|_| DealLedgerValidationError::EncodedLengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"sorafs-deal-ledger-snapshot-v1");
        hasher.update(&encoded_len.to_le_bytes());
        hasher.update(&bytes);
        Ok(*hasher.finalize().as_bytes())
    }
    /// Validate snapshot invariants.
    pub fn validate(&self) -> Result<(), DealLedgerValidationError> {
        if self.version != DEAL_LEDGER_VERSION_V1 {
            return Err(DealLedgerValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.sequence == 0 {
            return Err(DealLedgerValidationError::InvalidSequence);
        }
        match (self.sequence, self.previous_snapshot_id) {
            (1, None) => {}
            (1, Some(_)) => return Err(DealLedgerValidationError::UnexpectedPredecessor),
            (_, Some(previous)) if previous != [0; 32] => {}
            _ => return Err(DealLedgerValidationError::MissingPredecessor),
        }
        if self.deal_id.iter().all(|&byte| byte == 0) {
            return Err(DealLedgerValidationError::InvalidDealId);
        }
        if self.terms_digest.iter().all(|&byte| byte == 0) {
            return Err(DealLedgerValidationError::InvalidTermsDigest);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(DealLedgerValidationError::InvalidProviderId);
        }
        if self.client_id.iter().all(|&byte| byte == 0) {
            return Err(DealLedgerValidationError::InvalidClientId);
        }
        if self.deal_start_epoch == 0 || self.deal_start_epoch > self.deal_end_epoch {
            return Err(DealLedgerValidationError::InvalidDealEpochs);
        }
        let expected_window_end = self
            .window_start_epoch
            .checked_add(self.settlement_window_epochs)
            .ok_or(DealLedgerValidationError::InvalidWindow)?;
        if self.settlement_window_epochs == 0
            || self.window_start_epoch < self.deal_start_epoch
            || self.window_start_epoch > self.deal_end_epoch
            || self.window_end_epoch != expected_window_end
        {
            return Err(DealLedgerValidationError::InvalidWindow);
        }
        if self.captured_at == 0 || self.captured_at != self.window_end_epoch {
            return Err(DealLedgerValidationError::InvalidCapturedAt);
        }
        let generated = self
            .micropayment_credit_applied
            .checked_add(&self.micropayment_credit_carry)
            .map_err(|_| DealLedgerValidationError::AccountingOverflow)?;
        if generated != self.micropayment_credit_generated {
            return Err(DealLedgerValidationError::MicropaymentAccountingMismatch);
        }
        let provider_accrual = self
            .micropayment_credit_generated
            .checked_add(&self.client_debit)
            .map_err(|_| DealLedgerValidationError::AccountingOverflow)?;
        if provider_accrual != self.provider_accrual {
            return Err(DealLedgerValidationError::ProviderAccrualMismatch);
        }
        let satisfied_liability = self
            .micropayment_credit_applied
            .checked_add(&self.client_debit)
            .and_then(|amount| amount.checked_add(&self.bond_slashed))
            .and_then(|amount| amount.checked_add(&self.outstanding_liability))
            .map_err(|_| DealLedgerValidationError::AccountingOverflow)?;
        if satisfied_liability != self.client_liability {
            return Err(DealLedgerValidationError::ClientLiabilityMismatch);
        }
        let accounted_bond = self
            .bond_locked
            .checked_add(&self.bond_slashed)
            .and_then(|amount| amount.checked_add(&self.bond_released))
            .map_err(|_| DealLedgerValidationError::BondAccountingOverflow)?;
        if accounted_bond != self.bond_total || self.bond_total.is_zero() {
            return Err(DealLedgerValidationError::BondConservationMismatch);
        }
        if self.window_expected_charge > self.client_liability
            || self.window_micropayment_generated > self.micropayment_credit_generated
            || self.window_micropayment_applied > self.micropayment_credit_applied
            || self.window_client_debit > self.client_debit
            || self.window_bond_slashed > self.bond_slashed
            || self.window_bond_released > self.bond_released
        {
            return Err(DealLedgerValidationError::WindowExceedsCumulativeTotals);
        }
        let expected_snapshot_id = self.derive_snapshot_id()?;
        if self.snapshot_id != expected_snapshot_id {
            return Err(DealLedgerValidationError::SnapshotIdMismatch {
                expected: expected_snapshot_id,
                found: self.snapshot_id,
            });
        }
        Ok(())
    }
    /// Validate this snapshot as the exact successor of `previous`.
    pub fn validate_transition(
        &self,
        previous: Option<&Self>,
    ) -> Result<(), DealLedgerTransitionError> {
        self.validate()
            .map_err(DealLedgerTransitionError::Snapshot)?;
        let baseline = previous.cloned().unwrap_or(Self {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: 0,
            previous_snapshot_id: None,
            deal_id: self.deal_id,
            terms_digest: self.terms_digest,
            provider_id: self.provider_id,
            client_id: self.client_id,
            deal_start_epoch: self.deal_start_epoch,
            deal_end_epoch: self.deal_end_epoch,
            settlement_window_epochs: self.settlement_window_epochs,
            window_start_epoch: self.window_start_epoch,
            window_end_epoch: self.window_start_epoch,
            provider_accrual: XorQuantity::zero(),
            client_liability: XorQuantity::zero(),
            micropayment_credit_generated: XorQuantity::zero(),
            micropayment_credit_applied: XorQuantity::zero(),
            micropayment_credit_carry: XorQuantity::zero(),
            client_debit: XorQuantity::zero(),
            outstanding_liability: XorQuantity::zero(),
            bond_total: self.bond_total.clone(),
            bond_locked: self.bond_total.clone(),
            bond_slashed: XorQuantity::zero(),
            bond_released: XorQuantity::zero(),
            window_expected_charge: XorQuantity::zero(),
            window_micropayment_generated: XorQuantity::zero(),
            window_micropayment_applied: XorQuantity::zero(),
            window_client_debit: XorQuantity::zero(),
            window_bond_slashed: XorQuantity::zero(),
            window_bond_released: XorQuantity::zero(),
            captured_at: self.window_start_epoch,
        });
        if let Some(previous) = previous {
            previous
                .validate()
                .map_err(DealLedgerTransitionError::PreviousSnapshot)?;
            if previous.sequence.checked_add(1) != Some(self.sequence) {
                return Err(DealLedgerTransitionError::SequenceGap);
            }
            if self.previous_snapshot_id != Some(previous.snapshot_id) {
                return Err(DealLedgerTransitionError::PredecessorMismatch);
            }
            if self.deal_id != previous.deal_id
                || self.terms_digest != previous.terms_digest
                || self.provider_id != previous.provider_id
                || self.client_id != previous.client_id
                || self.deal_start_epoch != previous.deal_start_epoch
                || self.deal_end_epoch != previous.deal_end_epoch
                || self.settlement_window_epochs != previous.settlement_window_epochs
                || self.bond_total != previous.bond_total
            {
                return Err(DealLedgerTransitionError::ImmutableBindingMismatch);
            }
            if self.window_start_epoch != previous.window_end_epoch
                || self.window_end_epoch <= previous.window_end_epoch
                || self.captured_at <= previous.captured_at
            {
                return Err(DealLedgerTransitionError::WindowGapOrTimestampRegression);
            }
        } else if self.sequence != 1 || self.previous_snapshot_id.is_some() {
            return Err(DealLedgerTransitionError::InvalidFirstSnapshot);
        }
        validate_cumulative_delta(
            &baseline.client_liability,
            &self.client_liability,
            &self.window_expected_charge,
        )?;
        validate_cumulative_delta(
            &baseline.micropayment_credit_generated,
            &self.micropayment_credit_generated,
            &self.window_micropayment_generated,
        )?;
        validate_cumulative_delta(
            &baseline.micropayment_credit_applied,
            &self.micropayment_credit_applied,
            &self.window_micropayment_applied,
        )?;
        validate_cumulative_delta(
            &baseline.client_debit,
            &self.client_debit,
            &self.window_client_debit,
        )?;
        validate_cumulative_delta(
            &baseline.bond_slashed,
            &self.bond_slashed,
            &self.window_bond_slashed,
        )?;
        validate_cumulative_delta(
            &baseline.bond_released,
            &self.bond_released,
            &self.window_bond_released,
        )?;
        let expected_locked = self
            .bond_locked
            .checked_add(&self.window_bond_slashed)
            .and_then(|amount| amount.checked_add(&self.window_bond_released))
            .map_err(|_| DealLedgerTransitionError::AccountingOverflow)?;
        if baseline.bond_locked != expected_locked {
            return Err(DealLedgerTransitionError::BondDeltaMismatch);
        }
        let credit_sources = baseline
            .micropayment_credit_carry
            .checked_add(&self.window_micropayment_generated)
            .map_err(|_| DealLedgerTransitionError::AccountingOverflow)?;
        let credit_uses = self
            .window_micropayment_applied
            .checked_add(&self.micropayment_credit_carry)
            .map_err(|_| DealLedgerTransitionError::AccountingOverflow)?;
        if credit_sources != credit_uses {
            return Err(DealLedgerTransitionError::WindowCreditMismatch);
        }
        let liability_sources = baseline
            .outstanding_liability
            .checked_add(&self.window_expected_charge)
            .map_err(|_| DealLedgerTransitionError::AccountingOverflow)?;
        let liability_uses = self
            .window_micropayment_applied
            .checked_add(&self.window_client_debit)
            .and_then(|amount| amount.checked_add(&self.window_bond_slashed))
            .and_then(|amount| amount.checked_add(&self.outstanding_liability))
            .map_err(|_| DealLedgerTransitionError::AccountingOverflow)?;
        if liability_sources != liability_uses {
            return Err(DealLedgerTransitionError::WindowLiabilityMismatch);
        }
        Ok(())
    }
}
fn validate_cumulative_delta(
    previous: &XorQuantity,
    current: &XorQuantity,
    window: &XorQuantity,
) -> Result<(), DealLedgerTransitionError> {
    let expected = previous
        .checked_add(window)
        .map_err(|_| DealLedgerTransitionError::AccountingOverflow)?;
    if current != &expected {
        return Err(DealLedgerTransitionError::CumulativeDeltaMismatch);
    }
    Ok(())
}
/// Canonical settlement record emitted after each deal billing window.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct DealSettlementV1 {
    /// Schema version (`DEAL_SETTLEMENT_VERSION_V1`).
    pub version: u8,
    /// Domain-separated digest of this settlement with this field zeroed.
    pub settlement_id: [u8; 32],
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
    /// Derive the identifier of this exact settlement payload.
    pub fn derive_settlement_id(&self) -> Result<[u8; 32], DealSettlementValidationError> {
        let mut canonical = self.clone();
        canonical.settlement_id = [0; 32];
        let bytes = norito::to_bytes(&canonical)
            .map_err(|error| DealSettlementValidationError::Serialization(error.to_string()))?;
        let encoded_len = u64::try_from(bytes.len())
            .map_err(|_| DealSettlementValidationError::EncodedLengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"sorafs-deal-settlement-v1");
        hasher.update(&encoded_len.to_le_bytes());
        hasher.update(&bytes);
        Ok(*hasher.finalize().as_bytes())
    }
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
        if self.deal_id.iter().all(|&byte| byte == 0) {
            return Err(DealSettlementValidationError::InvalidDealId);
        }
        if self.deal_id != self.ledger.deal_id {
            return Err(DealSettlementValidationError::DealIdMismatch);
        }
        if self.settled_at == 0 || self.settled_at != self.ledger.captured_at {
            return Err(DealSettlementValidationError::InvalidSettledAt);
        }
        if let Some(notes) = &self.audit_notes {
            if notes.is_empty() || notes != notes.trim() || notes.chars().any(char::is_control) {
                return Err(DealSettlementValidationError::EmptyAuditNotes);
            }
            if notes.len() > MAX_DEAL_SETTLEMENT_AUDIT_NOTES_BYTES {
                return Err(DealSettlementValidationError::AuditNotesTooLong {
                    length: notes.len(),
                    max: MAX_DEAL_SETTLEMENT_AUDIT_NOTES_BYTES,
                });
            }
        }
        let terminal_epoch = self.ledger.window_end_epoch >= self.ledger.deal_end_epoch;
        match self.status {
            DealSettlementStatusV1::WindowSettled => {
                if terminal_epoch || self.ledger.bond_locked.is_zero() {
                    return Err(DealSettlementValidationError::StatusFinalityMismatch);
                }
            }
            DealSettlementStatusV1::Completed => {
                if !terminal_epoch
                    || !self.ledger.bond_locked.is_zero()
                    || !self.ledger.outstanding_liability.is_zero()
                    || !self.ledger.micropayment_credit_carry.is_zero()
                {
                    return Err(DealSettlementValidationError::StatusFinalityMismatch);
                }
            }
            DealSettlementStatusV1::Defaulted => {
                if !self.ledger.bond_locked.is_zero()
                    || self.ledger.bond_slashed.is_zero()
                    || !self.ledger.micropayment_credit_carry.is_zero()
                {
                    return Err(DealSettlementValidationError::StatusFinalityMismatch);
                }
            }
            DealSettlementStatusV1::Cancelled => {
                if terminal_epoch
                    || !self.ledger.bond_locked.is_zero()
                    || !self.ledger.outstanding_liability.is_zero()
                    || !self.ledger.micropayment_credit_carry.is_zero()
                    || !self.ledger.window_bond_slashed.is_zero()
                {
                    return Err(DealSettlementValidationError::StatusFinalityMismatch);
                }
            }
        }
        let requires_notes = !self.ledger.window_bond_slashed.is_zero()
            || matches!(
                self.status,
                DealSettlementStatusV1::Cancelled | DealSettlementStatusV1::Defaulted
            );
        if requires_notes && self.audit_notes.is_none() {
            return Err(DealSettlementValidationError::MissingAuditNotes);
        }
        if !requires_notes && self.audit_notes.is_some() {
            return Err(DealSettlementValidationError::UnexpectedAuditNotes);
        }
        let expected_settlement_id = self.derive_settlement_id()?;
        if self.settlement_id != expected_settlement_id {
            return Err(DealSettlementValidationError::SettlementIdMismatch {
                expected: expected_settlement_id,
                found: self.settlement_id,
            });
        }
        Ok(())
    }
    /// Validate this settlement as the exact successor of `previous`.
    pub fn validate_transition(
        &self,
        previous: Option<&Self>,
    ) -> Result<(), DealSettlementTransitionError> {
        self.validate()
            .map_err(DealSettlementTransitionError::Settlement)?;
        if let Some(previous) = previous {
            previous
                .validate()
                .map_err(DealSettlementTransitionError::PreviousSettlement)?;
            if previous.status != DealSettlementStatusV1::WindowSettled {
                return Err(DealSettlementTransitionError::PreviousSettlementFinal);
            }
            self.ledger
                .validate_transition(Some(&previous.ledger))
                .map_err(DealSettlementTransitionError::LedgerTransition)?;
        } else {
            self.ledger
                .validate_transition(None)
                .map_err(DealSettlementTransitionError::LedgerTransition)?;
        }
        Ok(())
    }
}
/// Settlement outcome.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub enum DealSettlementStatusV1 {
    /// A non-terminal billing window was settled and the deal remains active.
    WindowSettled,
    /// Deal completed successfully.
    Completed,
    /// Deal was cancelled (bond unlocked, no slashing).
    Cancelled,
    /// Deal finalised after exhausting collateral; liability may remain outstanding.
    Defaulted,
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
    /// Probability must be within the 1..=10_000 range.
    #[error("probability {probability_bps} bps is outside 1..=10_000")]
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
    #[error("client account length {length} exceeds maximum {max}")]
    ClientAccountTooLong { length: usize, max: usize },
    #[error("profile handle must not be empty")]
    EmptyProfileHandle,
    #[error("profile handle length {length} exceeds maximum {max}")]
    ProfileHandleTooLong { length: usize, max: usize },
    #[error("unknown chunker profile handle `{handle}`")]
    UnknownProfileHandle { handle: String },
    #[error("noncanonical profile handle `{provided}`; expected `{canonical}`")]
    NonCanonicalProfileHandle { provided: String, canonical: String },
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
    #[error("micropayment window exceeds the maximum deal duration")]
    MicropaymentWindowExceedsDeal,
    #[error("valid from must be greater than zero")]
    InvalidValidFrom,
    #[error("valid until must be greater than valid from")]
    InvalidValidityWindow,
    #[error("deal validity duration {duration}s is outside {min}..={max}s")]
    ValidityOutsideDurationWindow { duration: u64, min: u64, max: u64 },
    #[error("deal metadata count {count} exceeds maximum {max}")]
    TooManyMetadataEntries { count: usize, max: usize },
    #[error("metadata key must not be empty")]
    InvalidMetadataKey,
    #[error("metadata value must not be empty")]
    InvalidMetadataValue,
    #[error("duplicate metadata key {key}")]
    DuplicateMetadataKey { key: String },
    #[error("deal metadata entries must be sorted by key")]
    MetadataNotSorted,
    #[error("deal identifier does not bind the canonical terms")]
    DealIdMismatch { expected: [u8; 32], found: [u8; 32] },
    #[error("deal terms encoded length overflow")]
    EncodedLengthOverflow,
    #[error("deal terms serialization failed: {0}")]
    Serialization(String),
}
/// Validation errors for [`DealMicropaymentV1`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DealMicropaymentValidationError {
    #[error("unsupported micropayment version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("deal identifier must not be zero")]
    InvalidDealId,
    #[error("micropayment amount must be > 0")]
    ZeroAmount,
    #[error("micropayment issued_at must be greater than zero")]
    InvalidIssuedAt,
    #[error("determinism hint must not be zero")]
    MissingDeterminismHint,
    #[error("deal terms are invalid: {0}")]
    InvalidTerms(String),
    #[error("micropayment deal identifier does not match the deal terms")]
    DealIdMismatch,
    #[error("micropayment exceeds the deal's per-window liability cap")]
    LiabilityCapExceeded,
    #[error("micropayment window arithmetic overflow")]
    WindowArithmeticOverflow,
    #[error("micropayment window lies outside the deal validity interval")]
    WindowOutsideDeal,
    #[error(
        "micropayment issued_at {issued_at} is outside window completion {window_end}..={deal_end}"
    )]
    IssuedOutsideWindow {
        issued_at: u64,
        window_end: u64,
        deal_end: u64,
    },
    #[error("micropayment determinism hint does not bind the receipt")]
    DeterminismHintMismatch,
    #[error("micropayment amount encoded length overflow")]
    EncodedLengthOverflow,
    #[error("micropayment amount serialization failed: {0}")]
    Serialization(String),
}
/// Validation errors for [`DealLedgerSnapshotV1`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DealLedgerValidationError {
    #[error("unsupported ledger snapshot version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("ledger sequence must be greater than zero")]
    InvalidSequence,
    #[error("ledger sequence one must not carry a predecessor")]
    UnexpectedPredecessor,
    #[error("ledger sequence greater than one requires a non-zero predecessor")]
    MissingPredecessor,
    #[error("deal identifier must not be zero")]
    InvalidDealId,
    #[error("deal terms digest must not be zero")]
    InvalidTermsDigest,
    #[error("provider identifier must not be zero")]
    InvalidProviderId,
    #[error("client identifier must not be zero")]
    InvalidClientId,
    #[error("deal ledger epoch bounds are invalid")]
    InvalidDealEpochs,
    #[error("deal ledger settlement window is invalid")]
    InvalidWindow,
    #[error("ledger captured_at must equal its non-zero window end")]
    InvalidCapturedAt,
    #[error("ledger amount accounting overflow")]
    AccountingOverflow,
    #[error("generated, applied, and carried micropayment credit disagree")]
    MicropaymentAccountingMismatch,
    #[error("provider accrual does not equal generated credit plus client debit")]
    ProviderAccrualMismatch,
    #[error("client liability does not equal satisfied plus outstanding amounts")]
    ClientLiabilityMismatch,
    #[error("bond accounting overflow")]
    BondAccountingOverflow,
    #[error("locked, slashed, and released bond do not conserve the initial bond")]
    BondConservationMismatch,
    #[error("a settlement-window amount exceeds its cumulative total")]
    WindowExceedsCumulativeTotals,
    #[error("ledger snapshot identifier does not bind the canonical snapshot")]
    SnapshotIdMismatch { expected: [u8; 32], found: [u8; 32] },
    #[error("ledger snapshot encoded length overflow")]
    EncodedLengthOverflow,
    #[error("ledger snapshot serialization failed: {0}")]
    Serialization(String),
}
/// Validation errors for a ledger predecessor transition.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DealLedgerTransitionError {
    #[error("ledger snapshot validation failed: {0}")]
    Snapshot(DealLedgerValidationError),
    #[error("previous ledger snapshot validation failed: {0}")]
    PreviousSnapshot(DealLedgerValidationError),
    #[error("first ledger snapshot must use sequence one without a predecessor")]
    InvalidFirstSnapshot,
    #[error("ledger sequence does not increment by exactly one")]
    SequenceGap,
    #[error("ledger predecessor does not match the previous canonical snapshot")]
    PredecessorMismatch,
    #[error("ledger immutable deal, terms, party, epoch, or bond binding changed")]
    ImmutableBindingMismatch,
    #[error("ledger window or capture timestamp is not exactly monotonic")]
    WindowGapOrTimestampRegression,
    #[error("ledger transition amount accounting overflow")]
    AccountingOverflow,
    #[error("ledger cumulative total does not equal predecessor plus window delta")]
    CumulativeDeltaMismatch,
    #[error("ledger locked-bond delta does not match slash and release deltas")]
    BondDeltaMismatch,
    #[error("ledger window credit sources and uses do not balance")]
    WindowCreditMismatch,
    #[error("ledger window liability sources and uses do not balance")]
    WindowLiabilityMismatch,
}
/// Validation errors for [`DealSettlementV1`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DealSettlementValidationError {
    #[error("unsupported settlement version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("ledger validation failed: {0}")]
    Ledger(DealLedgerValidationError),
    #[error("settlement identifier does not bind the canonical payload")]
    SettlementIdMismatch { expected: [u8; 32], found: [u8; 32] },
    #[error("settlement deal identifier must not be zero")]
    InvalidDealId,
    #[error("settlement deal identifier does not match its ledger snapshot")]
    DealIdMismatch,
    #[error("settlement settled_at must equal the non-zero ledger capture epoch")]
    InvalidSettledAt,
    #[error("settlement status does not match ledger finality")]
    StatusFinalityMismatch,
    #[error("audit notes are required for slashing, cancellation, and default")]
    MissingAuditNotes,
    #[error("settlement status/window does not permit audit notes")]
    UnexpectedAuditNotes,
    #[error("settlement audit notes must not be blank")]
    EmptyAuditNotes,
    #[error("settlement audit notes are {length} bytes; maximum is {max}")]
    AuditNotesTooLong { length: usize, max: usize },
    #[error("settlement encoded length overflow")]
    EncodedLengthOverflow,
    #[error("settlement serialization failed: {0}")]
    Serialization(String),
}
/// Validation errors for a settlement-chain transition.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DealSettlementTransitionError {
    #[error("settlement validation failed: {0}")]
    Settlement(DealSettlementValidationError),
    #[error("previous settlement validation failed: {0}")]
    PreviousSettlement(DealSettlementValidationError),
    #[error("previous settlement is final and cannot have a successor")]
    PreviousSettlementFinal,
    #[error("ledger transition validation failed: {0}")]
    LedgerTransition(DealLedgerTransitionError),
}
#[cfg(test)]
mod tests {
    use super::*;
    fn xor_nanos(value: u128) -> XorQuantity {
        let whole = value / 1_000_000_000;
        let fractional = value % 1_000_000_000;
        format!("{whole}.{fractional:09}")
            .parse()
            .expect("nano-XOR fixture is canonical")
    }
    fn sample_terms() -> DealTermsV1 {
        let mut terms = DealTermsV1 {
            version: DEAL_TERMS_VERSION_V1,
            deal_id: [0; 32],
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
        };
        terms.deal_id = terms.derive_deal_id().expect("derive sample deal id");
        terms
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
        policy.probability_bps = 0;
        assert!(matches!(
            policy.validate(),
            Err(MicropaymentPolicyError::InvalidProbability { .. })
        ));
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
        assert!(matches!(
            err,
            DealTermsValidationError::DuplicateMetadataKey { .. }
        ));
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
    fn deal_identifier_binds_every_canonical_term() {
        let terms = sample_terms();
        assert_eq!(
            terms.deal_id,
            terms.derive_deal_id().expect("derive canonical deal id")
        );
        let mut tampered = terms;
        tampered.price_per_gib_month = tampered
            .price_per_gib_month
            .checked_add(
                &XorQuantity::try_from_micro(1)
                    .expect("legacy micro-XOR increment is representable"),
            )
            .expect("tampered price remains representable");
        assert!(matches!(
            tampered.validate(),
            Err(DealTermsValidationError::DealIdMismatch { .. })
        ));
    }
    #[test]
    fn deal_terms_reject_noncanonical_and_unbounded_fields() {
        let mut terms = sample_terms();
        terms.valid_from = 0;
        assert_eq!(
            terms.validate(),
            Err(DealTermsValidationError::InvalidValidFrom)
        );
        let mut terms = sample_terms();
        terms.client_account = vec![0; MAX_DEAL_CLIENT_ACCOUNT_BYTES + 1];
        assert!(matches!(
            terms.validate(),
            Err(DealTermsValidationError::ClientAccountTooLong { .. })
        ));
        let mut terms = sample_terms();
        terms.profile_handle = " sorafs.sf1@1.0.0".into();
        assert!(matches!(
            terms.validate(),
            Err(DealTermsValidationError::UnknownProfileHandle { .. })
        ));
        let mut terms = sample_terms();
        terms.valid_until = terms.valid_from + terms.max_duration_secs + 1;
        assert!(matches!(
            terms.validate(),
            Err(DealTermsValidationError::ValidityOutsideDurationWindow { .. })
        ));
        let mut terms = sample_terms();
        terms.metadata = vec![
            DealMetadataEntry {
                key: "zeta".into(),
                value: "one".into(),
            },
            DealMetadataEntry {
                key: "alpha".into(),
                value: "two".into(),
            },
        ];
        assert_eq!(
            terms.validate(),
            Err(DealTermsValidationError::MetadataNotSorted)
        );
        let mut terms = sample_terms();
        terms.metadata = (0..=MAX_DEAL_METADATA_ENTRIES)
            .map(|index| DealMetadataEntry {
                key: format!("key-{index:02}"),
                value: "value".into(),
            })
            .collect();
        assert_eq!(
            terms.validate(),
            Err(DealTermsValidationError::TooManyMetadataEntries {
                count: MAX_DEAL_METADATA_ENTRIES + 1,
                max: MAX_DEAL_METADATA_ENTRIES,
            })
        );
    }
    #[test]
    fn metadata_rejects_padding_controls_and_oversize() {
        for key in ["", " Region", "region ", "REGION", "region/"] {
            assert_eq!(
                DealMetadataEntry {
                    key: key.into(),
                    value: "value".into(),
                }
                .validate(),
                Err(DealTermsValidationError::InvalidMetadataKey)
            );
        }
        for value in ["", " padded", "padded ", "line\nbreak", "null\0byte"] {
            assert_eq!(
                DealMetadataEntry {
                    key: "region".into(),
                    value: value.into(),
                }
                .validate(),
                Err(DealTermsValidationError::InvalidMetadataValue)
            );
        }
        assert_eq!(
            DealMetadataEntry {
                key: "region".into(),
                value: "x".repeat(MAX_DEAL_METADATA_VALUE_BYTES + 1),
            }
            .validate(),
            Err(DealTermsValidationError::InvalidMetadataValue)
        );
    }
    #[test]
    fn micropayment_receipt_is_bound_to_terms_window_cap_and_hint() {
        let terms = sample_terms();
        let window_index = 0;
        let issued_at = terms.valid_from + u64::from(terms.micropayment.window_secs);
        let amount =
            XorQuantity::try_from_micro(10_000).expect("legacy micro-XOR value is representable");
        let receipt = DealMicropaymentV1 {
            version: DEAL_MICROPAYMENT_VERSION_V1,
            deal_id: terms.deal_id,
            window_index,
            amount: amount.clone(),
            issued_at,
            determinism_hint: derive_micropayment_hint(
                terms.deal_id,
                window_index,
                &amount,
                issued_at,
            )
            .expect("derive receipt hint"),
        };
        receipt
            .validate_against_terms(&terms)
            .expect("terms-bound receipt");
        let mut tampered = receipt.clone();
        tampered.determinism_hint[0] ^= 1;
        assert_eq!(
            tampered.validate_against_terms(&terms),
            Err(DealMicropaymentValidationError::DeterminismHintMismatch)
        );
        let mut excessive = receipt.clone();
        excessive.amount = terms
            .micropayment
            .max_window_liability
            .checked_add(
                &XorQuantity::try_from_micro(1)
                    .expect("legacy micro-XOR increment is representable"),
            )
            .expect("excessive amount remains representable");
        excessive.determinism_hint = derive_micropayment_hint(
            excessive.deal_id,
            excessive.window_index,
            &excessive.amount,
            excessive.issued_at,
        )
        .expect("derive excessive receipt hint");
        assert_eq!(
            excessive.validate_against_terms(&terms),
            Err(DealMicropaymentValidationError::LiabilityCapExceeded)
        );
        let mut outside = receipt;
        outside.window_index = u64::MAX;
        assert_eq!(
            outside.validate_against_terms(&terms),
            Err(DealMicropaymentValidationError::WindowArithmeticOverflow)
        );
    }
    #[test]
    fn micropayment_hint_binds_exact_quantity_bytes() {
        let deal_id = [0xA5; 32];
        let sub_micro = "0.0000001"
            .parse::<XorQuantity>()
            .expect("canonical sub-micro XOR quantity");
        let adjacent = "0.000000101"
            .parse::<XorQuantity>()
            .expect("canonical adjacent XOR quantity");
        let wide = "340282366920938463463374607431768211456.000000001"
            .parse::<XorQuantity>()
            .expect("wide XOR quantity fits the exact domain");
        let sub_micro_hint = derive_micropayment_hint(deal_id, 7, &sub_micro, 1_700_000_000)
            .expect("derive sub-micro hint");
        assert_ne!(
            sub_micro_hint,
            derive_micropayment_hint(deal_id, 7, &adjacent, 1_700_000_000)
                .expect("derive adjacent hint")
        );
        assert_ne!(
            sub_micro_hint,
            derive_micropayment_hint(deal_id, 7, &wide, 1_700_000_000).expect("derive wide hint")
        );
    }
    fn seal_ledger(mut ledger: DealLedgerSnapshotV1) -> DealLedgerSnapshotV1 {
        ledger.snapshot_id = ledger.derive_snapshot_id().expect("derive ledger id");
        ledger
    }
    fn first_ledger() -> DealLedgerSnapshotV1 {
        seal_ledger(DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: 1,
            previous_snapshot_id: None,
            deal_id: [0xAA; 32],
            terms_digest: [0x44; 32],
            provider_id: [0xBB; 32],
            client_id: [0xCC; 32],
            deal_start_epoch: 100,
            deal_end_epoch: 114,
            settlement_window_epochs: 7,
            window_start_epoch: 100,
            window_end_epoch: 107,
            provider_accrual: xor_nanos(900),
            client_liability: xor_nanos(1_000),
            micropayment_credit_generated: xor_nanos(300),
            micropayment_credit_applied: xor_nanos(300),
            micropayment_credit_carry: XorQuantity::zero(),
            client_debit: xor_nanos(600),
            outstanding_liability: XorQuantity::zero(),
            bond_total: xor_nanos(5_000),
            bond_locked: xor_nanos(4_900),
            bond_slashed: xor_nanos(100),
            bond_released: XorQuantity::zero(),
            window_expected_charge: xor_nanos(1_000),
            window_micropayment_generated: xor_nanos(300),
            window_micropayment_applied: xor_nanos(300),
            window_client_debit: xor_nanos(600),
            window_bond_slashed: xor_nanos(100),
            window_bond_released: XorQuantity::zero(),
            captured_at: 107,
        })
    }
    fn completed_ledger(previous: &DealLedgerSnapshotV1) -> DealLedgerSnapshotV1 {
        seal_ledger(DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: 2,
            previous_snapshot_id: Some(previous.snapshot_id),
            deal_id: previous.deal_id,
            terms_digest: previous.terms_digest,
            provider_id: previous.provider_id,
            client_id: previous.client_id,
            deal_start_epoch: previous.deal_start_epoch,
            deal_end_epoch: previous.deal_end_epoch,
            settlement_window_epochs: previous.settlement_window_epochs,
            window_start_epoch: previous.window_end_epoch,
            window_end_epoch: 114,
            provider_accrual: xor_nanos(1_400),
            client_liability: xor_nanos(1_500),
            micropayment_credit_generated: xor_nanos(400),
            micropayment_credit_applied: xor_nanos(400),
            micropayment_credit_carry: XorQuantity::zero(),
            client_debit: xor_nanos(1_000),
            outstanding_liability: XorQuantity::zero(),
            bond_total: xor_nanos(5_000),
            bond_locked: XorQuantity::zero(),
            bond_slashed: xor_nanos(100),
            bond_released: xor_nanos(4_900),
            window_expected_charge: xor_nanos(500),
            window_micropayment_generated: xor_nanos(100),
            window_micropayment_applied: xor_nanos(100),
            window_client_debit: xor_nanos(400),
            window_bond_slashed: XorQuantity::zero(),
            window_bond_released: xor_nanos(4_900),
            captured_at: 114,
        })
    }
    fn seal_settlement(
        ledger: DealLedgerSnapshotV1,
        status: DealSettlementStatusV1,
        notes: Option<&str>,
    ) -> DealSettlementV1 {
        let mut settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            settlement_id: [0; 32],
            deal_id: ledger.deal_id,
            settled_at: ledger.captured_at,
            ledger,
            status,
            audit_notes: notes.map(str::to_owned),
        };
        settlement.settlement_id = settlement
            .derive_settlement_id()
            .expect("derive settlement id");
        settlement
    }
    #[test]
    fn ledger_snapshot_id_and_first_transition_are_canonical() {
        let ledger = first_ledger();
        ledger.validate().expect("valid ledger");
        ledger
            .validate_transition(None)
            .expect("valid first transition");
        let mut tampered = ledger.clone();
        tampered.client_debit = tampered
            .client_debit
            .checked_add(&xor_nanos(1))
            .expect("tampered debit remains representable");
        assert!(matches!(
            tampered.validate(),
            Err(DealLedgerValidationError::ProviderAccrualMismatch)
                | Err(DealLedgerValidationError::ClientLiabilityMismatch)
                | Err(DealLedgerValidationError::SnapshotIdMismatch { .. })
        ));
        let mut skipped_window = ledger;
        skipped_window.window_end_epoch += 1;
        skipped_window.captured_at += 1;
        skipped_window.snapshot_id = skipped_window.derive_snapshot_id().expect("reseal");
        assert_eq!(
            skipped_window.validate(),
            Err(DealLedgerValidationError::InvalidWindow)
        );
    }
    #[test]
    fn ledger_transition_binds_predecessor_sequence_parties_terms_and_window() {
        let first = first_ledger();
        let second = completed_ledger(&first);
        second
            .validate_transition(Some(&first))
            .expect("valid exact successor");
        let mut cases = Vec::new();
        let mut sequence_gap = second.clone();
        sequence_gap.sequence = 3;
        sequence_gap.snapshot_id = sequence_gap.derive_snapshot_id().expect("reseal");
        cases.push(sequence_gap);
        let mut fork = second.clone();
        fork.previous_snapshot_id = Some([0x55; 32]);
        fork.snapshot_id = fork.derive_snapshot_id().expect("reseal");
        cases.push(fork);
        let mut substitution = second.clone();
        substitution.provider_id[0] ^= 1;
        substitution.snapshot_id = substitution.derive_snapshot_id().expect("reseal");
        cases.push(substitution);
        let mut window_substitution = second.clone();
        window_substitution.settlement_window_epochs += 1;
        window_substitution.window_end_epoch += 1;
        window_substitution.captured_at += 1;
        window_substitution.snapshot_id = window_substitution.derive_snapshot_id().expect("reseal");
        cases.push(window_substitution);
        let mut gap = second;
        gap.window_start_epoch += 1;
        gap.snapshot_id = gap.derive_snapshot_id().expect("reseal");
        cases.push(gap);
        for tampered in cases {
            assert!(tampered.validate_transition(Some(&first)).is_err());
        }
    }
    #[test]
    fn ledger_transition_rejects_credit_liability_and_bond_forgery() {
        let first = first_ledger();
        let second = completed_ledger(&first);
        for mutate in [
            |ledger: &mut DealLedgerSnapshotV1| {
                ledger.window_client_debit = ledger
                    .window_client_debit
                    .checked_add(&xor_nanos(1))
                    .expect("tampered debit remains representable");
            },
            |ledger: &mut DealLedgerSnapshotV1| {
                ledger.outstanding_liability = ledger
                    .outstanding_liability
                    .checked_add(&xor_nanos(1))
                    .expect("tampered liability remains representable");
            },
            |ledger: &mut DealLedgerSnapshotV1| {
                ledger.window_bond_released = ledger
                    .window_bond_released
                    .checked_sub(&xor_nanos(1))
                    .expect("tampered release remains non-negative");
            },
        ] {
            let mut tampered = second.clone();
            mutate(&mut tampered);
            tampered.snapshot_id = tampered.derive_snapshot_id().expect("reseal");
            assert!(tampered.validate_transition(Some(&first)).is_err());
        }
    }
    #[test]
    fn settlement_chain_binds_ids_finality_and_terminal_status() {
        let first = seal_settlement(
            first_ledger(),
            DealSettlementStatusV1::WindowSettled,
            Some("bond slashed after failed proof"),
        );
        first.validate_transition(None).expect("first settlement");
        let final_settlement = seal_settlement(
            completed_ledger(&first.ledger),
            DealSettlementStatusV1::Completed,
            None,
        );
        final_settlement
            .validate_transition(Some(&first))
            .expect("terminal successor");
        let mut id_tamper = final_settlement.clone();
        id_tamper.settlement_id[0] ^= 1;
        assert!(matches!(
            id_tamper.validate(),
            Err(DealSettlementValidationError::SettlementIdMismatch { .. })
        ));
        assert_eq!(
            first.validate_transition(Some(&final_settlement)),
            Err(DealSettlementTransitionError::PreviousSettlementFinal)
        );
        let mut exhausted = first_ledger();
        exhausted.bond_total = xor_nanos(100);
        exhausted.bond_locked = XorQuantity::zero();
        exhausted.bond_slashed = xor_nanos(100);
        exhausted.snapshot_id = exhausted.derive_snapshot_id().expect("reseal");
        let early_default = seal_settlement(
            exhausted,
            DealSettlementStatusV1::Defaulted,
            Some("collateral exhausted before the terminal epoch"),
        );
        assert!(early_default.settled_at < early_default.ledger.deal_end_epoch);
        early_default
            .validate_transition(None)
            .expect("collateral exhaustion is immediately final");
        let mut cancellation_ledger = first_ledger();
        cancellation_ledger.provider_accrual = XorQuantity::zero();
        cancellation_ledger.client_liability = XorQuantity::zero();
        cancellation_ledger.micropayment_credit_generated = XorQuantity::zero();
        cancellation_ledger.micropayment_credit_applied = XorQuantity::zero();
        cancellation_ledger.client_debit = XorQuantity::zero();
        cancellation_ledger.bond_total = xor_nanos(5_000);
        cancellation_ledger.bond_locked = XorQuantity::zero();
        cancellation_ledger.bond_slashed = XorQuantity::zero();
        cancellation_ledger.bond_released = xor_nanos(5_000);
        cancellation_ledger.window_expected_charge = XorQuantity::zero();
        cancellation_ledger.window_micropayment_generated = XorQuantity::zero();
        cancellation_ledger.window_micropayment_applied = XorQuantity::zero();
        cancellation_ledger.window_client_debit = XorQuantity::zero();
        cancellation_ledger.window_bond_slashed = XorQuantity::zero();
        cancellation_ledger.window_bond_released = xor_nanos(5_000);
        cancellation_ledger.snapshot_id = cancellation_ledger
            .derive_snapshot_id()
            .expect("reseal cancellation ledger");
        let cancelled = seal_settlement(
            cancellation_ledger,
            DealSettlementStatusV1::Cancelled,
            Some("operator-approved termination"),
        );
        cancelled
            .validate_transition(None)
            .expect("non-terminal cancellation is canonical and final");
        let terminal_cancel = seal_settlement(
            completed_ledger(&first.ledger),
            DealSettlementStatusV1::Cancelled,
            Some("too late to cancel"),
        );
        assert_eq!(
            terminal_cancel.validate(),
            Err(DealSettlementValidationError::StatusFinalityMismatch)
        );
    }
    #[test]
    fn settlement_rejects_stale_time_blank_notes_and_status_substitution() {
        let first = seal_settlement(
            first_ledger(),
            DealSettlementStatusV1::WindowSettled,
            Some("bond slashed after failed proof"),
        );
        let mut stale = first.clone();
        stale.settled_at -= 1;
        assert_eq!(
            stale.validate(),
            Err(DealSettlementValidationError::InvalidSettledAt)
        );
        let mut blank = first.clone();
        blank.audit_notes = Some("   ".into());
        assert_eq!(
            blank.validate(),
            Err(DealSettlementValidationError::EmptyAuditNotes)
        );
        let mut wrong_status = first;
        wrong_status.status = DealSettlementStatusV1::Completed;
        wrong_status.settlement_id = wrong_status.derive_settlement_id().expect("reseal");
        assert_eq!(
            wrong_status.validate(),
            Err(DealSettlementValidationError::StatusFinalityMismatch)
        );
    }
    #[test]
    fn ledger_rejects_overflow_zero_ids_and_bond_nonconservation() {
        let mut overflow = first_ledger();
        overflow.micropayment_credit_applied =
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
                .parse()
                .expect("maximum XOR quantity");
        overflow.micropayment_credit_carry = xor_nanos(1);
        overflow.snapshot_id = overflow.derive_snapshot_id().expect("reseal");
        assert_eq!(
            overflow.validate(),
            Err(DealLedgerValidationError::AccountingOverflow)
        );
        let mut zero_party = first_ledger();
        zero_party.provider_id = [0; 32];
        zero_party.snapshot_id = zero_party.derive_snapshot_id().expect("reseal");
        assert_eq!(
            zero_party.validate(),
            Err(DealLedgerValidationError::InvalidProviderId)
        );
        let mut forged_bond = first_ledger();
        forged_bond.bond_locked = forged_bond
            .bond_locked
            .checked_sub(&xor_nanos(1))
            .expect("forged bond remains non-negative");
        forged_bond.snapshot_id = forged_bond.derive_snapshot_id().expect("reseal");
        assert_eq!(
            forged_bond.validate(),
            Err(DealLedgerValidationError::BondConservationMismatch)
        );
    }
}
