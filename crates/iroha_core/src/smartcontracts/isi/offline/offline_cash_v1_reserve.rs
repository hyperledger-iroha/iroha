//! Pooled reserve accounting for clean-slate Offline Cash V1.
//!
//! One canonical `(network, asset)` pool records every top-up and redemption;
//! peer transfers never touch it.
//! Top-up execution persists a deterministic issuance request and exact
//! post-operation receipt before same-block finality exists. A later worker
//! deterministically joins that immutable record with finality and caches the
//! byte-exact result in a local outbox; finalized WSV is never retroactively
//! mutated.

use std::collections::BTreeMap;

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId, AssetValue},
    isi::{
        OfflineCashFinalityTrustAnchorV1, OfflineCashOperationKindV1,
        OfflineCashRedemptionRequestV1, OfflineCashReserveReceiptV1, OfflineCashTopUpRequestV1,
        OfflineCashTopUpResultV1,
    },
    nexus::AxtAssetIncarnationV1,
    offline::{
        OFFLINE_CASH_ASSET_SCALE_MAX_V1, OFFLINE_CASH_WIRE_VERSION_V1,
        OfflineCashLifecycleBindingV1, OfflineCashMintCreditStatementV1,
        offline_cash_ciphertext_digest_v1, offline_cash_liability_pool_id_v1,
    },
};
use iroha_primitives::numeric::{Numeric, Quantity};
use norito::codec::{Decode, Encode};
use norito::derive::{JsonDeserialize, JsonSerialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::VerifiedOfflineCashTopUpAuthorizationV1;
use crate::zk::offline_cash_v1_recursion::VerifiedOfflineCashRedemptionProofV1;

/// Version of pooled Offline Cash reserve state and durable records.
pub const OFFLINE_CASH_RESERVE_VERSION_V1: u16 = 1;

const TOP_UP_RESULT_WIRE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:reserve:top-up-result-wire\0";

/// Failure returned by deterministic Offline Cash V1 reserve accounting.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OfflineCashReserveErrorV1 {
    /// A persisted value or plan used a version other than V1.
    #[error("unsupported Offline Cash reserve version {actual}")]
    UnsupportedVersion {
        /// Encountered version.
        actual: u16,
    },
    /// An operation identifier was the forbidden all-zero value.
    #[error("Offline Cash operation id must be non-zero")]
    InvalidOperationId,
    /// The network-and-asset pool identity was not canonical.
    #[error("Offline Cash liability pool id does not match its network and asset")]
    InvalidLiabilityPool,
    /// An operation's scale differed from the scale already bound to its pool.
    #[error("Offline Cash operation scale {actual} does not match pool scale {expected}")]
    AssetScaleConflict {
        /// Scale already bound to the reserve pool.
        expected: u32,
        /// Scale carried by the new operation.
        actual: u32,
    },
    /// A chain-facing value was structurally invalid.
    #[error("invalid Offline Cash V1 chain value: {0}")]
    InvalidWire(String),
    /// Canonical Norito encoding failed.
    #[error("failed to encode canonical Offline Cash V1 value: {0}")]
    Encoding(String),
    /// The authenticated block execution context was incomplete.
    #[error("invalid Offline Cash reserve block execution context")]
    InvalidExecutionContext,
    /// Adding a top-up would exceed `u128` accounting.
    #[error("Offline Cash total top-ups overflow u128")]
    TotalTopUpsOverflow,
    /// Adding a redemption would exceed `u128` accounting.
    #[error("Offline Cash total redemptions overflow u128")]
    TotalRedemptionsOverflow,
    /// A redemption exceeded the currently accounted reserve.
    #[error("Offline Cash reserve underflow: requested {requested}, available {available}")]
    ReserveUnderflow {
        /// Reserve available before the attempted redemption.
        available: u128,
        /// Requested redemption amount.
        requested: u128,
    },
    /// Persisted reserve custody is smaller than the outstanding liability.
    #[error(
        "Offline Cash reserve custody is underfunded: liability {liability}, custody {custody}"
    )]
    ReserveCustodyUnderfunded {
        /// Aggregate outstanding liability for the custody account and asset.
        liability: Quantity,
        /// Aggregate transparent custody restored from world state.
        custody: Quantity,
    },
    /// An operation id was already bound to another request or operation kind.
    #[error("Offline Cash operation id is already bound to another operation")]
    OperationConflict {
        /// Conflicting operation identifier.
        operation_id: [u8; 32],
    },
    /// A mint credit was already emitted by another operation.
    #[error("Offline Cash mint credit id is already committed by another operation")]
    MintCreditConflict {
        /// Conflicting mint-credit identifier.
        credit_id: [u8; 32],
    },
    /// A finalized issuance was already consumed by another top-up.
    #[error("Offline Cash issuance commitment is already committed by another operation")]
    IssuanceConflict {
        /// Conflicting issuance commitment.
        issuance_commitment: [u8; 32],
    },
    /// Final mint-credit, request, receipt, or finality bytes did not match the intent.
    #[error("Offline Cash mint finality does not match its committed top-up intent")]
    MintFinalizationMismatch {
        /// Top-up operation whose attachment was rejected.
        operation_id: [u8; 32],
    },
    /// A top-up already carried a different terminal attachment.
    #[error("Offline Cash top-up is already finalized with different bytes")]
    MintFinalizationConflict {
        /// Conflicting top-up operation identifier.
        operation_id: [u8; 32],
    },
    /// A decoded terminal attachment could not resolve canonical chain context.
    #[error("canonical Offline Cash finality anchor is unavailable")]
    FinalityAnchorUnavailable {
        /// Operation whose anchor could not be resolved.
        operation_id: [u8; 32],
    },
    /// Typed consensus finality failed a stable validation boundary.
    #[error("invalid Offline Cash finality evidence: {reason}")]
    InvalidFinalityEvidence {
        /// Stable invariant label.
        reason: &'static str,
    },
    /// An exact redemption output identity was already used by another operation.
    #[error("Offline Cash redemption id is already committed by another operation")]
    RedemptionIdConflict {
        /// Conflicting redemption identifier.
        redemption_id: [u8; 32],
    },
    /// A predecessor state was already terminally consumed by another redemption.
    #[error("Offline Cash terminal nullifier is already committed by another operation")]
    TerminalNullifierConflict {
        /// Conflicting terminal nullifier.
        terminal_nullifier: [u8; 32],
    },
    /// Another operation committed after this plan was prepared.
    #[error("stale Offline Cash reserve plan: pool head changed")]
    StalePlan {
        /// Pool-head digest observed during planning, or `None` for an empty pool.
        expected: Option<[u8; 32]>,
        /// Pool-head digest observed during commit, or `None` for an empty pool.
        actual: Option<[u8; 32]>,
    },
    /// A sealed plan did not agree with its operation or projected pool.
    #[error("invalid Offline Cash reserve mutation plan")]
    InvalidPlan,
    /// Persisted reserve state violated a deterministic accounting invariant.
    #[error("invalid Offline Cash reserve state: {reason}")]
    StateInvariant {
        /// Stable invariant label.
        reason: &'static str,
    },
}

/// Canonical identity of the sole reserve for one network and asset.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, JsonDeserialize, JsonSerialize,
)]
pub struct OfflineCashReservePoolKeyV1 {
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Asset whose offline liabilities are pooled.
    pub asset: AssetDefinitionId,
    /// Exact authoritative asset-registration incarnation.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Domain-separated identity derived from network, asset, and incarnation.
    pub liability_pool_id: [u8; 32],
}

impl OfflineCashReservePoolKeyV1 {
    /// Derive the sole valid reserve key for a network and asset.
    ///
    /// # Errors
    ///
    /// Returns an encoding error if the pool identity cannot be derived.
    pub fn new(
        network_id: NetworkId,
        asset: AssetDefinitionId,
        asset_incarnation: AxtAssetIncarnationV1,
    ) -> Result<Self, OfflineCashReserveErrorV1> {
        let liability_pool_id =
            offline_cash_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
                .map_err(|error| OfflineCashReserveErrorV1::Encoding(error.to_string()))?;
        Ok(Self {
            network_id,
            asset,
            asset_incarnation,
            liability_pool_id,
        })
    }

    fn from_wire(
        network_id: NetworkId,
        asset: AssetDefinitionId,
        asset_incarnation: AxtAssetIncarnationV1,
        liability_pool_id: [u8; 32],
    ) -> Result<Self, OfflineCashReserveErrorV1> {
        let key = Self {
            network_id,
            asset,
            asset_incarnation,
            liability_pool_id,
        };
        key.validate()?;
        Ok(key)
    }

    /// Validate this key's canonical identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the carried pool identity is not canonical.
    pub fn validate(&self) -> Result<(), OfflineCashReserveErrorV1> {
        self.asset_incarnation
            .validate()
            .map_err(|error| OfflineCashReserveErrorV1::InvalidWire(error.to_string()))?;
        let expected = offline_cash_liability_pool_id_v1(
            &self.network_id,
            &self.asset,
            self.asset_incarnation,
        )
        .map_err(|error| OfflineCashReserveErrorV1::Encoding(error.to_string()))?;
        if self.liability_pool_id != expected {
            return Err(OfflineCashReserveErrorV1::InvalidLiabilityPool);
        }
        Ok(())
    }
}

/// Durable totals for one pooled Offline Cash reserve.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
pub struct OfflineCashReservePoolV1 {
    /// Reserve-state version.
    pub version: u16,
    /// Canonical network-and-asset pool.
    pub key: OfflineCashReservePoolKeyV1,
    /// Fixed scale shared by every operation in this pool.
    pub scale: u32,
    /// Checked sum of every committed top-up.
    pub total_topups: u128,
    /// Checked sum of every committed redemption.
    pub total_redemptions: u128,
    /// Complete latest receipt used for constant-size authenticated delta/CAS validation.
    ///
    /// Older receipts remain operation history and may be pruned only with an authenticated
    /// checkpoint; ordinary execution needs only this fixed-size head.
    pub latest_receipt: Option<OfflineCashReserveReceiptV1>,
}

impl OfflineCashReservePoolV1 {
    fn empty(key: OfflineCashReservePoolKeyV1, scale: u32) -> Self {
        Self {
            version: OFFLINE_CASH_RESERVE_VERSION_V1,
            key,
            scale,
            total_topups: 0,
            total_redemptions: 0,
            latest_receipt: None,
        }
    }

    /// Return the exact currently redeemable reserve.
    ///
    /// # Errors
    ///
    /// Returns an invariant error if redemptions exceed top-ups.
    pub fn available(&self) -> Result<u128, OfflineCashReserveErrorV1> {
        self.total_topups.checked_sub(self.total_redemptions).ok_or(
            OfflineCashReserveErrorV1::StateInvariant {
                reason: "total_redemptions_exceed_total_topups",
            },
        )
    }

    /// Validate version, identity, scale, and reserve conservation.
    ///
    /// # Errors
    ///
    /// Returns an error for any malformed persisted pool.
    pub fn validate(&self) -> Result<(), OfflineCashReserveErrorV1> {
        require_version(self.version)?;
        self.key.validate()?;
        if self.scale > OFFLINE_CASH_ASSET_SCALE_MAX_V1 {
            return Err(state_invariant("invalid_pool_asset_scale"));
        }
        self.available()?;
        match &self.latest_receipt {
            None if self.total_topups == 0 && self.total_redemptions == 0 => {}
            Some(receipt)
                if receipt.network_id == self.key.network_id
                    && receipt.asset == self.key.asset
                    && receipt.asset_incarnation == self.key.asset_incarnation
                    && receipt.scale == self.scale
                    && receipt.liability_pool_id == self.key.liability_pool_id
                    && receipt.total_topups == self.total_topups
                    && receipt.total_redemptions == self.total_redemptions =>
            {
                receipt.validate().map_err(map_chain_value_error)?;
            }
            _ => return Err(state_invariant("pool_head_or_totals_mismatch")),
        }
        Ok(())
    }

    fn head_digest(&self) -> Result<Option<[u8; 32]>, OfflineCashReserveErrorV1> {
        self.latest_receipt
            .as_ref()
            .map(|receipt| receipt.canonical_digest().map_err(map_chain_value_error))
            .transpose()
    }
}

/// Authenticated execution context supplied by the block executor.
///
/// It is intentionally not decodable and has no public field constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineCashReserveCommitContextV1 {
    transaction_hash: [u8; 32],
    committed_at_ms: u64,
}

impl OfflineCashReserveCommitContextV1 {
    /// Seal transaction identity and authoritative block time after executor verification.
    pub(in crate::smartcontracts::isi) fn after_block_context_verification(
        transaction_hash: [u8; 32],
        committed_at_ms: u64,
    ) -> Result<Self, OfflineCashReserveErrorV1> {
        let context = Self {
            transaction_hash,
            committed_at_ms,
        };
        context.validate()?;
        Ok(context)
    }

    fn validate(&self) -> Result<(), OfflineCashReserveErrorV1> {
        if self.transaction_hash == [0; 32] || self.committed_at_ms == 0 {
            return Err(OfflineCashReserveErrorV1::InvalidExecutionContext);
        }
        Ok(())
    }
}

/// Exact pre-finality top-up intent persisted with the atomic reserve effect.
///
/// No mint time or finality is client supplied. The mint time comes from the
/// committed receipt, and `request_digest` detects corrupted request bytes.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
pub struct OfflineCashTopUpIssuanceIntentV1 {
    /// Reserve intent layout version.
    pub version: u16,
    /// Complete validated chain-facing request.
    pub request: OfflineCashTopUpRequestV1,
    /// Canonical digest of `request`.
    pub request_digest: [u8; 32],
}

impl OfflineCashTopUpIssuanceIntentV1 {
    fn from_request(request: OfflineCashTopUpRequestV1) -> Result<Self, OfflineCashReserveErrorV1> {
        request.validate_shape().map_err(map_chain_value_error)?;
        let request_digest = request.canonical_digest().map_err(map_chain_value_error)?;
        let intent = Self {
            version: OFFLINE_CASH_RESERVE_VERSION_V1,
            request,
            request_digest,
        };
        intent.validate()?;
        Ok(intent)
    }

    fn validate(&self) -> Result<(), OfflineCashReserveErrorV1> {
        require_version(self.version)?;
        self.request
            .validate_shape()
            .map_err(map_chain_value_error)?;
        require_nonzero_operation(self.request.operation_id)?;
        if self.request_digest
            != self
                .request
                .canonical_digest()
                .map_err(map_chain_value_error)?
        {
            return Err(state_invariant("top_up_request_digest_mismatch"));
        }
        Ok(())
    }
}

/// Derived local mint result cached after consensus finalizes the top-up receipt.
///
/// `verified_anchor_identity` records which locally authenticated context admitted
/// the result. It cannot authenticate itself during snapshot hydration.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
pub struct OfflineCashMintFinalityAttachmentV1 {
    /// Attachment layout version.
    pub version: u16,
    /// Exact typed result returned for byte-identical recovery.
    pub result: OfflineCashTopUpResultV1,
    /// Domain-separated digest of the canonical result bytes.
    pub result_wire_digest: [u8; 32],
    /// Identity of the external canonical context used at admission.
    pub verified_anchor_identity: OfflineCashFinalityTrustAnchorV1,
}

/// Durable record of one atomic online debit and reserve top-up.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
pub struct OfflineCashTopUpRecordV1 {
    /// Reserve-record version.
    pub version: u16,
    /// Idempotency key of the public top-up operation.
    pub operation_id: [u8; 32],
    /// Canonical reserve credited by this operation.
    pub pool: OfflineCashReservePoolKeyV1,
    /// Positive atomic units credited to the reserve.
    pub amount: u128,
    /// Recursive-proof release identifier.
    pub release_id: [u8; 32],
    /// Authoritative asset scale.
    pub scale: u32,
    /// Unique online issuance intent.
    pub issuance_commitment: [u8; 32],
    /// Unique device-bound mint output.
    pub credit_id: [u8; 32],
    /// Online account atomically debited.
    pub payer: AccountId,
    /// Account that owns the offline credit.
    pub recipient: AccountId,
    /// Exact accepted pre-finality intent.
    pub issuance_intent: OfflineCashTopUpIssuanceIntentV1,
    /// Canonical post-operation totals written by execution.
    pub reserve_receipt: OfflineCashReserveReceiptV1,
}

/// Durable record of one atomic reserve debit and beneficiary credit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
pub struct OfflineCashRedemptionRecordV1 {
    /// Reserve-record version.
    pub version: u16,
    /// Idempotency key of the public redemption.
    pub operation_id: [u8; 32],
    /// Canonical reserve debited.
    pub pool: OfflineCashReservePoolKeyV1,
    /// Positive atomic units debited.
    pub amount: u128,
    /// Recursive-proof release identifier.
    pub release_id: [u8; 32],
    /// Authoritative asset scale.
    pub scale: u32,
    /// Public account atomically credited.
    pub beneficiary: AccountId,
    /// Identity of this exact full or partial redemption.
    pub redemption_id: [u8; 32],
    /// One-use terminal consumption of the predecessor.
    pub terminal_nullifier: [u8; 32],
    /// Exact validated request and voucher.
    pub redemption_request: OfflineCashRedemptionRequestV1,
    /// Canonical digest of the exact request and voucher.
    pub request_digest: [u8; 32],
    /// Canonical post-operation totals written by execution.
    pub reserve_receipt: OfflineCashReserveReceiptV1,
}

impl OfflineCashTopUpRecordV1 {
    fn validate_basic(&self) -> Result<(), OfflineCashReserveErrorV1> {
        require_version(self.version)?;
        require_nonzero_operation(self.operation_id)?;
        self.pool.validate()?;
        self.issuance_intent.validate()?;
        self.reserve_receipt
            .validate()
            .map_err(map_chain_value_error)?;
        let request = &self.issuance_intent.request;
        if self.amount == 0
            || self.release_id == [0; 32]
            || self.scale > OFFLINE_CASH_ASSET_SCALE_MAX_V1
            || self.issuance_commitment == [0; 32]
            || self.credit_id == [0; 32]
        {
            return Err(state_invariant("invalid_top_up_record"));
        }
        let expected_mint_statement =
            mint_statement_from_request(request, self.reserve_receipt.committed_at_ms)?;
        let expected_mint_statement_digest = expected_mint_statement
            .canonical_digest()
            .map_err(map_chain_value_error)?;
        if self.operation_id != request.operation_id
            || self.pool.network_id != request.network_id
            || self.pool.asset != request.asset
            || self.pool.asset_incarnation != request.asset_incarnation
            || self.pool.liability_pool_id != request.liability_pool_id
            || self.amount != request.amount
            || self.release_id != request.release_id
            || self.scale != request.scale
            || self.issuance_commitment != request.issuance_commitment
            || self.credit_id != request.credit_id
            || self.payer != request.payer
            || self.recipient != request.recipient
            || !receipt_matches(
                &self.reserve_receipt,
                OfflineCashOperationKindV1::TopUp,
                self.operation_id,
                self.issuance_intent.request_digest,
                expected_mint_statement_digest,
                &self.pool,
                self.scale,
                self.amount,
            )
        {
            return Err(state_invariant("top_up_record_intent_or_receipt_mismatch"));
        }
        Ok(())
    }

    fn same_request(&self, intent: &OfflineCashTopUpIssuanceIntentV1) -> bool {
        self.issuance_intent == *intent
    }
}

impl OfflineCashRedemptionRecordV1 {
    fn validate_basic(&self) -> Result<(), OfflineCashReserveErrorV1> {
        require_version(self.version)?;
        require_nonzero_operation(self.operation_id)?;
        self.pool.validate()?;
        self.redemption_request
            .validate_shape()
            .map_err(map_chain_value_error)?;
        self.reserve_receipt
            .validate()
            .map_err(map_chain_value_error)?;
        let statement = &self.redemption_request.voucher.statement;
        let lifecycle = &statement.lifecycle;
        if self.amount == 0
            || self.release_id == [0; 32]
            || self.scale > OFFLINE_CASH_ASSET_SCALE_MAX_V1
            || self.redemption_id == [0; 32]
            || self.terminal_nullifier == [0; 32]
        {
            return Err(state_invariant("invalid_redemption_record"));
        }
        if self.operation_id != self.redemption_request.operation_id
            || self.request_digest
                != self
                    .redemption_request
                    .canonical_digest()
                    .map_err(map_chain_value_error)?
            || self.pool.network_id != lifecycle.network_id
            || self.pool.asset != lifecycle.asset
            || self.pool.asset_incarnation != lifecycle.asset_incarnation
            || self.pool.liability_pool_id != lifecycle.liability_pool_id
            || self.amount != statement.amount
            || self.release_id != lifecycle.release_id
            || self.scale != lifecycle.scale
            || self.beneficiary != statement.beneficiary
            || self.redemption_id != statement.redemption_id
            || self.terminal_nullifier != statement.terminal_nullifier
            || !receipt_matches(
                &self.reserve_receipt,
                OfflineCashOperationKindV1::Redemption,
                self.operation_id,
                self.request_digest,
                [0; 32],
                &self.pool,
                self.scale,
                self.amount,
            )
        {
            return Err(state_invariant(
                "redemption_record_request_or_receipt_mismatch",
            ));
        }
        Ok(())
    }

    fn same_request(&self, request: &OfflineCashRedemptionRequestV1) -> bool {
        self.redemption_request == *request
    }
}

/// Durable status record for either reserve operation kind.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
#[norito(tag = "operation_kind", content = "record", deny_unknown_fields)]
pub enum OfflineCashReserveOperationRecordV1 {
    /// Applied top-up, optionally carrying its terminal mint result.
    TopUp(OfflineCashTopUpRecordV1),
    /// Applied full or partial redemption.
    Redemption(OfflineCashRedemptionRecordV1),
}

impl OfflineCashReserveOperationRecordV1 {
    /// Return the public idempotency key.
    #[must_use]
    pub fn operation_id(&self) -> [u8; 32] {
        match self {
            Self::TopUp(record) => record.operation_id,
            Self::Redemption(record) => record.operation_id,
        }
    }

    /// Return the affected canonical liability pool.
    #[must_use]
    pub fn pool(&self) -> &OfflineCashReservePoolKeyV1 {
        match self {
            Self::TopUp(record) => &record.pool,
            Self::Redemption(record) => &record.pool,
        }
    }

    /// Return the positive number of settled atomic units.
    #[must_use]
    pub fn amount(&self) -> u128 {
        match self {
            Self::TopUp(record) => record.amount,
            Self::Redemption(record) => record.amount,
        }
    }

    /// Return the exact authenticated reserve receipt.
    #[must_use]
    pub fn reserve_receipt(&self) -> &OfflineCashReserveReceiptV1 {
        match self {
            Self::TopUp(record) => &record.reserve_receipt,
            Self::Redemption(record) => &record.reserve_receipt,
        }
    }

    fn kind(&self) -> OfflineCashOperationKindV1 {
        match self {
            Self::TopUp(_) => OfflineCashOperationKindV1::TopUp,
            Self::Redemption(_) => OfflineCashOperationKindV1::Redemption,
        }
    }

    fn request_digest(&self) -> [u8; 32] {
        match self {
            Self::TopUp(record) => record.issuance_intent.request_digest,
            Self::Redemption(record) => record.request_digest,
        }
    }

    fn scale(&self) -> u32 {
        match self {
            Self::TopUp(record) => record.scale,
            Self::Redemption(record) => record.scale,
        }
    }

    fn validate_basic(&self) -> Result<(), OfflineCashReserveErrorV1> {
        match self {
            Self::TopUp(record) => record.validate_basic(),
            Self::Redemption(record) => record.validate_basic(),
        }
    }
}

/// Exact entries read from separately keyed World storage before a top-up plan.
///
/// This bounded read set lets World avoid cloning a monolithic reserve book.
#[derive(Clone, Copy, Debug)]
pub struct OfflineCashTopUpReadSetV1<'a> {
    /// Current pool entry, absent before the first operation.
    pub current_pool: Option<&'a OfflineCashReservePoolV1>,
    /// Existing operation with the requested idempotency key.
    pub existing_operation: Option<&'a OfflineCashReserveOperationRecordV1>,
    /// Operation currently owning the requested credit ID.
    pub credit_operation: Option<[u8; 32]>,
    /// Operation currently owning the requested issuance commitment.
    pub issuance_operation: Option<[u8; 32]>,
}

/// Exact entries read from separately keyed World storage before redemption.
#[derive(Clone, Copy, Debug)]
pub struct OfflineCashRedemptionReadSetV1<'a> {
    /// Current pool entry, absent only before the first operation.
    pub current_pool: Option<&'a OfflineCashReservePoolV1>,
    /// Existing operation with the requested idempotency key.
    pub existing_operation: Option<&'a OfflineCashReserveOperationRecordV1>,
    /// Operation currently owning the requested redemption ID.
    pub redemption_operation: Option<[u8; 32]>,
    /// Operation currently owning the terminal nullifier.
    pub terminal_nullifier_operation: Option<[u8; 32]>,
}

/// Sealed top-up mutation prepared against one exact pool-head digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashTopUpPlanV1 {
    version: u16,
    expected_pool_head: Option<[u8; 32]>,
    next_pool: OfflineCashReservePoolV1,
    record: OfflineCashTopUpRecordV1,
}

impl OfflineCashTopUpPlanV1 {
    /// Return the pool head that must still be present at commit.
    #[must_use]
    pub fn expected_pool_head(&self) -> Option<[u8; 32]> {
        self.expected_pool_head
    }

    /// Borrow the one projected pool entry to write.
    #[must_use]
    pub fn next_pool(&self) -> &OfflineCashReservePoolV1 {
        &self.next_pool
    }

    /// Borrow the one projected operation entry to write.
    #[must_use]
    pub fn record(&self) -> &OfflineCashTopUpRecordV1 {
        &self.record
    }
}

/// Sealed redemption mutation prepared against one exact pool-head digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashRedemptionPlanV1 {
    version: u16,
    expected_pool_head: Option<[u8; 32]>,
    next_pool: OfflineCashReservePoolV1,
    record: OfflineCashRedemptionRecordV1,
}

impl OfflineCashRedemptionPlanV1 {
    /// Return the pool head that must still be present at commit.
    #[must_use]
    pub fn expected_pool_head(&self) -> Option<[u8; 32]> {
        self.expected_pool_head
    }

    /// Borrow the one projected pool entry to write.
    #[must_use]
    pub fn next_pool(&self) -> &OfflineCashReservePoolV1 {
        &self.next_pool
    }

    /// Borrow the one projected operation entry to write.
    #[must_use]
    pub fn record(&self) -> &OfflineCashRedemptionRecordV1 {
        &self.record
    }
}

/// Closed set of reserve mutations accepted by the commit boundary.
///
/// Plans are intentionally not decodable. Only a successful planner can create one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OfflineCashReserveMutationPlanV1 {
    /// Credit a reserve after atomically debiting online funds.
    TopUp(OfflineCashTopUpPlanV1),
    /// Debit a reserve while atomically crediting a beneficiary.
    Redemption(OfflineCashRedemptionPlanV1),
}

/// Result of preparing an idempotent reserve operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OfflineCashReservePlanOutcomeV1 {
    /// A new mutation must join the surrounding atomic World transaction.
    Commit(OfflineCashReserveMutationPlanV1),
    /// The exact request is already durably committed.
    AlreadyCommitted(OfflineCashReserveOperationRecordV1),
}

/// Result of committing a prepared reserve operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OfflineCashReserveCommitOutcomeV1 {
    /// A new operation and authenticated pool head were installed.
    Committed(OfflineCashReserveOperationRecordV1),
    /// The exact request had already committed; no totals changed.
    AlreadyCommitted(OfflineCashReserveOperationRecordV1),
}

/// Result of attaching terminal consensus finality to an applied top-up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OfflineCashMintFinalizationOutcomeV1 {
    /// A new exact local result/outbox attachment must be persisted.
    Finalized(OfflineCashMintFinalityAttachmentV1),
    /// The byte-identical local attachment was already persisted.
    AlreadyFinalized(OfflineCashMintFinalityAttachmentV1),
}

/// Validated pre-finality issuance admitted by the top-up ISI.
#[derive(Clone, Debug)]
pub struct VerifiedOfflineCashTopUpIntentV1 {
    intent: OfflineCashTopUpIssuanceIntentV1,
    authorization: VerifiedOfflineCashTopUpAuthorizationV1,
}

impl VerifiedOfflineCashTopUpIntentV1 {
    /// Seal an exact request after non-accounting admission checks pass.
    pub(in crate::smartcontracts::isi) fn after_admission_verification(
        request: OfflineCashTopUpRequestV1,
        verified_authorization: VerifiedOfflineCashTopUpAuthorizationV1,
    ) -> Result<Self, OfflineCashReserveErrorV1> {
        Ok(Self {
            intent: OfflineCashTopUpIssuanceIntentV1::from_request(request)?,
            authorization: verified_authorization,
        })
    }

    /// Borrow the exact verified pre-finality intent.
    #[must_use]
    pub fn intent(&self) -> &OfflineCashTopUpIssuanceIntentV1 {
        &self.intent
    }

    fn mint_statement(
        &self,
        minted_at_ms: u64,
    ) -> Result<OfflineCashMintCreditStatementV1, OfflineCashReserveErrorV1> {
        self.authorization
            .mint_statement(&self.intent.request, minted_at_ms)
            .map_err(OfflineCashReserveErrorV1::InvalidWire)
    }

    /// Return the public operation idempotency key.
    #[must_use]
    pub fn operation_id(&self) -> [u8; 32] {
        self.intent.request.operation_id
    }
}

/// Fully admitted mint result and canonical finality identity used to verify it.
#[derive(Clone, Debug)]
pub struct VerifiedOfflineCashMintFinalizationV1 {
    result: OfflineCashTopUpResultV1,
    trusted_anchor_identity: OfflineCashFinalityTrustAnchorV1,
}

impl VerifiedOfflineCashMintFinalizationV1 {
    /// Seal a terminal result after verification against locally pinned consensus context.
    ///
    /// `trusted_anchor` must come from canonical Kura/consensus state, never the response.
    pub(in crate::smartcontracts::isi) fn after_full_verification(
        result: OfflineCashTopUpResultV1,
        trusted_anchor: OfflineCashFinalityTrustAnchorV1,
    ) -> Result<Self, OfflineCashReserveErrorV1> {
        result.validate_against(&trusted_anchor).map_err(|_| {
            OfflineCashReserveErrorV1::InvalidFinalityEvidence {
                reason: "terminal_result_failed_canonical_anchor_validation",
            }
        })?;
        Ok(Self {
            result,
            trusted_anchor_identity: trusted_anchor,
        })
    }

    /// Return the public top-up operation id.
    #[must_use]
    pub fn operation_id(&self) -> [u8; 32] {
        self.result.request.operation_id
    }

    /// Borrow the exact finalized top-up result.
    #[must_use]
    pub fn result(&self) -> &OfflineCashTopUpResultV1 {
        &self.result
    }

    /// Return the canonical finality identity used at admission.
    #[must_use]
    pub fn trusted_anchor_identity(&self) -> OfflineCashFinalityTrustAnchorV1 {
        self.trusted_anchor_identity
    }
}

/// Validated terminal redemption admitted by the redemption ISI.
#[derive(Clone, Debug)]
pub struct VerifiedOfflineCashRedemptionV1 {
    request: OfflineCashRedemptionRequestV1,
}

impl VerifiedOfflineCashRedemptionV1 {
    /// Consume the opaque capability returned by the governed paired recursive verifier.
    ///
    /// There is deliberately no constructor from a structural request. Only the verifier owns
    /// the capability's constructor, so reserve accounting cannot accidentally treat decodable
    /// proof bytes, a host-side signature, or a boolean assertion as monetary authority.
    pub(in crate::smartcontracts::isi) fn after_full_verification(
        verified: VerifiedOfflineCashRedemptionProofV1,
    ) -> Self {
        Self {
            request: verified.into_request(),
        }
    }

    /// Borrow the exact verified request and voucher.
    #[must_use]
    pub fn request(&self) -> &OfflineCashRedemptionRequestV1 {
        &self.request
    }

    /// Return the public operation idempotency key.
    #[must_use]
    pub fn operation_id(&self) -> [u8; 32] {
        self.request.operation_id
    }
}

/// Resolve a persisted finality identity from locally authenticated chain state.
pub trait OfflineCashFinalityAnchorResolverV1 {
    /// Resolve an identity only when canonical local consensus state trusts it.
    fn resolve(
        &self,
        identity: &OfflineCashFinalityTrustAnchorV1,
    ) -> Option<OfflineCashFinalityTrustAnchorV1>;
}

impl<F> OfflineCashFinalityAnchorResolverV1 for F
where
    F: Fn(&OfflineCashFinalityTrustAnchorV1) -> Option<OfflineCashFinalityTrustAnchorV1>,
{
    fn resolve(
        &self,
        identity: &OfflineCashFinalityTrustAnchorV1,
    ) -> Option<OfflineCashFinalityTrustAnchorV1> {
        self(identity)
    }
}

/// Complete reserve projection for snapshot validation and focused unit tests.
///
/// Production World integration must store pools, operations, and reverse indexes
/// in separately keyed `mv::Storage` maps. The exact-entry read sets and sealed
/// plans above keep each operation O(log n); placing this aggregate in one
/// clone-on-write `mv::Cell` would make each mutation O(total history).
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct OfflineCashReserveBookV1 {
    version: u16,
    pools: BTreeMap<OfflineCashReservePoolKeyV1, OfflineCashReservePoolV1>,
    operations: BTreeMap<[u8; 32], OfflineCashReserveOperationRecordV1>,
    mint_credit_operations: BTreeMap<[u8; 32], [u8; 32]>,
    issuance_operations: BTreeMap<[u8; 32], [u8; 32]>,
    redemption_id_operations: BTreeMap<[u8; 32], [u8; 32]>,
    terminal_nullifier_operations: BTreeMap<[u8; 32], [u8; 32]>,
}

impl Default for OfflineCashReserveBookV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl OfflineCashReserveBookV1 {
    /// Create an empty V1 reserve book.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: OFFLINE_CASH_RESERVE_VERSION_V1,
            pools: BTreeMap::new(),
            operations: BTreeMap::new(),
            mint_credit_operations: BTreeMap::new(),
            issuance_operations: BTreeMap::new(),
            redemption_id_operations: BTreeMap::new(),
            terminal_nullifier_operations: BTreeMap::new(),
        }
    }

    /// Return the reserve-state version.
    #[must_use]
    pub fn version(&self) -> u16 {
        self.version
    }

    /// Return the number of durably committed public operations.
    #[must_use]
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Look up one idempotent operation record.
    #[must_use]
    pub fn operation(
        &self,
        operation_id: &[u8; 32],
    ) -> Option<&OfflineCashReserveOperationRecordV1> {
        self.operations.get(operation_id)
    }

    /// Look up the operation that emitted a mint credit.
    #[must_use]
    pub fn operation_for_mint_credit(
        &self,
        credit_id: &[u8; 32],
    ) -> Option<&OfflineCashReserveOperationRecordV1> {
        self.mint_credit_operations
            .get(credit_id)
            .and_then(|operation_id| self.operations.get(operation_id))
    }

    /// Look up the operation that consumed an issuance commitment.
    #[must_use]
    pub fn operation_for_issuance(
        &self,
        issuance_commitment: &[u8; 32],
    ) -> Option<&OfflineCashReserveOperationRecordV1> {
        self.issuance_operations
            .get(issuance_commitment)
            .and_then(|operation_id| self.operations.get(operation_id))
    }

    /// Look up the operation that emitted a redemption identifier.
    #[must_use]
    pub fn operation_for_redemption(
        &self,
        redemption_id: &[u8; 32],
    ) -> Option<&OfflineCashReserveOperationRecordV1> {
        self.redemption_id_operations
            .get(redemption_id)
            .and_then(|operation_id| self.operations.get(operation_id))
    }

    /// Look up the operation that consumed a terminal nullifier.
    #[must_use]
    pub fn operation_for_terminal_nullifier(
        &self,
        terminal_nullifier: &[u8; 32],
    ) -> Option<&OfflineCashReserveOperationRecordV1> {
        self.terminal_nullifier_operations
            .get(terminal_nullifier)
            .and_then(|operation_id| self.operations.get(operation_id))
    }

    /// Look up one canonical pool without creating it.
    ///
    /// # Errors
    ///
    /// Returns an encoding error if the canonical key cannot be derived.
    pub fn pool(
        &self,
        network_id: NetworkId,
        asset: AssetDefinitionId,
        asset_incarnation: AxtAssetIncarnationV1,
    ) -> Result<Option<&OfflineCashReservePoolV1>, OfflineCashReserveErrorV1> {
        let key = OfflineCashReservePoolKeyV1::new(network_id, asset, asset_incarnation)?;
        Ok(self.pools.get(&key))
    }

    /// Return current reserve, or zero before the first top-up.
    ///
    /// # Errors
    ///
    /// Returns an error if key derivation fails or persisted totals underflow.
    pub fn available(
        &self,
        network_id: NetworkId,
        asset: AssetDefinitionId,
        asset_incarnation: AxtAssetIncarnationV1,
    ) -> Result<u128, OfflineCashReserveErrorV1> {
        Ok(self
            .pool(network_id, asset, asset_incarnation)?
            .map(OfflineCashReservePoolV1::available)
            .transpose()?
            .unwrap_or(0))
    }

    /// Plan one top-up using only its exact pool, operation, and reverse-index entries.
    ///
    /// # Errors
    ///
    /// Returns an error for conflicts, malformed context, scale mismatch, or overflow.
    pub(in crate::smartcontracts::isi) fn plan_top_up(
        &self,
        verified: &VerifiedOfflineCashTopUpIntentV1,
        context: OfflineCashReserveCommitContextV1,
    ) -> Result<OfflineCashReservePlanOutcomeV1, OfflineCashReserveErrorV1> {
        require_version(self.version)?;
        let request = &verified.intent.request;
        let pool = OfflineCashReservePoolKeyV1::from_wire(
            request.network_id,
            request.asset.clone(),
            request.asset_incarnation,
            request.liability_pool_id,
        )?;
        plan_top_up_from_entries(
            verified,
            context,
            OfflineCashTopUpReadSetV1 {
                current_pool: self.pools.get(&pool),
                existing_operation: self.operations.get(&request.operation_id),
                credit_operation: self.mint_credit_operations.get(&request.credit_id).copied(),
                issuance_operation: self
                    .issuance_operations
                    .get(&request.issuance_commitment)
                    .copied(),
            },
        )
    }

    /// Plan one full or partial redemption using exact separately keyed entries.
    ///
    /// # Errors
    ///
    /// Returns an error for replay, conflicts, insufficient reserve, or overflow.
    pub(in crate::smartcontracts::isi) fn plan_redemption(
        &self,
        verified: &VerifiedOfflineCashRedemptionV1,
        context: OfflineCashReserveCommitContextV1,
    ) -> Result<OfflineCashReservePlanOutcomeV1, OfflineCashReserveErrorV1> {
        require_version(self.version)?;
        let statement = &verified.request.voucher.statement;
        let lifecycle = &statement.lifecycle;
        let pool = OfflineCashReservePoolKeyV1::from_wire(
            lifecycle.network_id,
            lifecycle.asset.clone(),
            lifecycle.asset_incarnation,
            lifecycle.liability_pool_id,
        )?;
        plan_redemption_from_entries(
            verified,
            context,
            OfflineCashRedemptionReadSetV1 {
                current_pool: self.pools.get(&pool),
                existing_operation: self.operations.get(&verified.operation_id()),
                redemption_operation: self
                    .redemption_id_operations
                    .get(&statement.redemption_id)
                    .copied(),
                terminal_nullifier_operation: self
                    .terminal_nullifier_operations
                    .get(&statement.terminal_nullifier)
                    .copied(),
            },
        )
    }

    /// Commit one sealed mutation after exact-entry concurrency rechecks.
    ///
    /// All checks run before map mutation. World must apply the matching account movement and
    /// receipt write in the same authenticated transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale plan, conflict, replay, or invalid projection.
    pub(in crate::smartcontracts::isi) fn commit(
        &mut self,
        plan: OfflineCashReserveMutationPlanV1,
    ) -> Result<OfflineCashReserveCommitOutcomeV1, OfflineCashReserveErrorV1> {
        require_version(self.version)?;
        match plan {
            OfflineCashReserveMutationPlanV1::TopUp(plan) => {
                let read_set = OfflineCashTopUpReadSetV1 {
                    current_pool: self.pools.get(&plan.record.pool),
                    existing_operation: self.operations.get(&plan.record.operation_id),
                    credit_operation: self
                        .mint_credit_operations
                        .get(&plan.record.credit_id)
                        .copied(),
                    issuance_operation: self
                        .issuance_operations
                        .get(&plan.record.issuance_commitment)
                        .copied(),
                };
                if let Some(existing) = validate_top_up_commit_entries(&plan, read_set)? {
                    return Ok(OfflineCashReserveCommitOutcomeV1::AlreadyCommitted(
                        existing,
                    ));
                }
                let record = OfflineCashReserveOperationRecordV1::TopUp(plan.record.clone());
                self.pools
                    .insert(plan.next_pool.key.clone(), plan.next_pool.clone());
                self.operations
                    .insert(plan.record.operation_id, record.clone());
                self.mint_credit_operations
                    .insert(plan.record.credit_id, plan.record.operation_id);
                self.issuance_operations
                    .insert(plan.record.issuance_commitment, plan.record.operation_id);
                Ok(OfflineCashReserveCommitOutcomeV1::Committed(record))
            }
            OfflineCashReserveMutationPlanV1::Redemption(plan) => {
                let read_set = OfflineCashRedemptionReadSetV1 {
                    current_pool: self.pools.get(&plan.record.pool),
                    existing_operation: self.operations.get(&plan.record.operation_id),
                    redemption_operation: self
                        .redemption_id_operations
                        .get(&plan.record.redemption_id)
                        .copied(),
                    terminal_nullifier_operation: self
                        .terminal_nullifier_operations
                        .get(&plan.record.terminal_nullifier)
                        .copied(),
                };
                if let Some(existing) = validate_redemption_commit_entries(&plan, read_set)? {
                    return Ok(OfflineCashReserveCommitOutcomeV1::AlreadyCommitted(
                        existing,
                    ));
                }
                let record = OfflineCashReserveOperationRecordV1::Redemption(plan.record.clone());
                self.pools
                    .insert(plan.next_pool.key.clone(), plan.next_pool.clone());
                self.operations
                    .insert(plan.record.operation_id, record.clone());
                self.redemption_id_operations
                    .insert(plan.record.redemption_id, plan.record.operation_id);
                self.terminal_nullifier_operations
                    .insert(plan.record.terminal_nullifier, plan.record.operation_id);
                Ok(OfflineCashReserveCommitOutcomeV1::Committed(record))
            }
        }
    }

    /// Reconstruct every total, historical receipt, authenticated head, and replay index.
    ///
    /// # Errors
    ///
    /// Returns an invariant error for any missing, extra, duplicated, or mismatched record.
    pub fn validate(&self) -> Result<(), OfflineCashReserveErrorV1> {
        require_version(self.version)?;
        for (key, pool) in &self.pools {
            pool.validate()?;
            if key != &pool.key {
                return Err(state_invariant("pool_map_key_mismatch"));
            }
        }

        let mut history: BTreeMap<
            OfflineCashReservePoolKeyV1,
            BTreeMap<[u8; 32], &OfflineCashReserveOperationRecordV1>,
        > = BTreeMap::new();
        for (operation_id, record) in &self.operations {
            record.validate_basic()?;
            if operation_id != &record.operation_id() {
                return Err(state_invariant("operation_map_key_mismatch"));
            }
            let pool = self
                .pools
                .get(record.pool())
                .ok_or_else(|| state_invariant("operation_missing_pool"))?;
            require_matching_scale(pool.scale, record.scale())?;
            let predecessor = record.reserve_receipt().previous_pool_receipt_digest;
            if history
                .entry(record.pool().clone())
                .or_default()
                .insert(predecessor, record)
                .is_some()
            {
                return Err(state_invariant("duplicate_pool_receipt_predecessor"));
            }
            match record {
                OfflineCashReserveOperationRecordV1::TopUp(record) => {
                    self.ensure_top_up_indexes(record)?;
                }
                OfflineCashReserveOperationRecordV1::Redemption(record) => {
                    self.ensure_redemption_indexes(record)?;
                }
            }
        }

        for (key, pool) in &self.pools {
            let records = history
                .get(key)
                .ok_or_else(|| state_invariant("pool_without_operations"))?;
            let mut projected = OfflineCashReservePoolV1::empty(key.clone(), pool.scale);
            let mut predecessor = [0; 32];
            let mut previous_receipt = None;
            for _ in 0..records.len() {
                let record = records
                    .get(&predecessor)
                    .copied()
                    .ok_or_else(|| state_invariant("pool_receipt_chain_gap"))?;
                record
                    .reserve_receipt()
                    .validate_against_previous_receipt(previous_receipt)
                    .map_err(map_chain_value_error)?;
                projected = match record {
                    OfflineCashReserveOperationRecordV1::TopUp(record) => {
                        next_top_up_pool(&projected, record.amount)?
                    }
                    OfflineCashReserveOperationRecordV1::Redemption(record) => {
                        next_redemption_pool(&projected, record.amount)?
                    }
                };
                projected.latest_receipt = Some(record.reserve_receipt().clone());
                if !receipt_matches_pool_projection(
                    record.reserve_receipt(),
                    &projected,
                    record.kind(),
                    record.operation_id(),
                    record.request_digest(),
                    record.reserve_receipt().mint_statement_digest,
                    record.amount(),
                ) {
                    return Err(state_invariant("historical_reserve_receipt_mismatch"));
                }
                predecessor = record
                    .reserve_receipt()
                    .canonical_digest()
                    .map_err(map_chain_value_error)?;
                previous_receipt = Some(record.reserve_receipt());
            }
            if &projected != pool {
                return Err(state_invariant("pool_totals_or_head_mismatch"));
            }
        }
        self.validate_reverse_indexes()?;
        Ok(())
    }

    fn ensure_top_up_indexes(
        &self,
        record: &OfflineCashTopUpRecordV1,
    ) -> Result<(), OfflineCashReserveErrorV1> {
        if self.mint_credit_operations.get(&record.credit_id) != Some(&record.operation_id)
            || self.issuance_operations.get(&record.issuance_commitment)
                != Some(&record.operation_id)
        {
            return Err(state_invariant("top_up_reverse_index_mismatch"));
        }
        Ok(())
    }

    fn ensure_redemption_indexes(
        &self,
        record: &OfflineCashRedemptionRecordV1,
    ) -> Result<(), OfflineCashReserveErrorV1> {
        if self.redemption_id_operations.get(&record.redemption_id) != Some(&record.operation_id)
            || self
                .terminal_nullifier_operations
                .get(&record.terminal_nullifier)
                != Some(&record.operation_id)
        {
            return Err(state_invariant("redemption_reverse_index_mismatch"));
        }
        Ok(())
    }

    fn validate_reverse_indexes(&self) -> Result<(), OfflineCashReserveErrorV1> {
        for (credit_id, operation_id) in &self.mint_credit_operations {
            match self.operations.get(operation_id) {
                Some(OfflineCashReserveOperationRecordV1::TopUp(record))
                    if &record.credit_id == credit_id => {}
                _ => return Err(state_invariant("extra_or_invalid_mint_credit_index")),
            }
        }
        for (issuance_commitment, operation_id) in &self.issuance_operations {
            match self.operations.get(operation_id) {
                Some(OfflineCashReserveOperationRecordV1::TopUp(record))
                    if &record.issuance_commitment == issuance_commitment => {}
                _ => return Err(state_invariant("extra_or_invalid_issuance_index")),
            }
        }
        for (redemption_id, operation_id) in &self.redemption_id_operations {
            match self.operations.get(operation_id) {
                Some(OfflineCashReserveOperationRecordV1::Redemption(record))
                    if &record.redemption_id == redemption_id => {}
                _ => return Err(state_invariant("extra_or_invalid_redemption_id_index")),
            }
        }
        for (terminal_nullifier, operation_id) in &self.terminal_nullifier_operations {
            match self.operations.get(operation_id) {
                Some(OfflineCashReserveOperationRecordV1::Redemption(record))
                    if &record.terminal_nullifier == terminal_nullifier => {}
                _ => return Err(state_invariant("extra_or_invalid_terminal_nullifier_index")),
            }
        }
        Ok(())
    }
}

/// Reconstruct and validate the separately persisted World reserve maps.
///
/// Snapshot hydration uses this boundary to reject mismatched pool keys, missing operation
/// history, incorrect totals, and incomplete or extra replay indexes before the World is exposed.
pub(crate) fn validate_persisted_reserve_entries_v1<'a>(
    pools: impl IntoIterator<Item = (&'a [u8; 32], &'a OfflineCashReservePoolV1)>,
    operations: impl IntoIterator<Item = (&'a [u8; 32], &'a OfflineCashReserveOperationRecordV1)>,
    mint_credit_operations: impl IntoIterator<Item = (&'a [u8; 32], &'a [u8; 32])>,
    issuance_operations: impl IntoIterator<Item = (&'a [u8; 32], &'a [u8; 32])>,
    redemption_id_operations: impl IntoIterator<Item = (&'a [u8; 32], &'a [u8; 32])>,
    terminal_nullifier_operations: impl IntoIterator<Item = (&'a [u8; 32], &'a [u8; 32])>,
) -> Result<(), OfflineCashReserveErrorV1> {
    let mut reconstructed_pools = BTreeMap::new();
    for (storage_key, pool) in pools {
        pool.validate()?;
        if storage_key != &pool.key.liability_pool_id
            || reconstructed_pools
                .insert(pool.key.clone(), pool.clone())
                .is_some()
        {
            return Err(state_invariant("pool_storage_key_mismatch"));
        }
    }
    let book = OfflineCashReserveBookV1 {
        version: OFFLINE_CASH_RESERVE_VERSION_V1,
        pools: reconstructed_pools,
        operations: operations
            .into_iter()
            .map(|(key, value)| (*key, value.clone()))
            .collect(),
        mint_credit_operations: mint_credit_operations
            .into_iter()
            .map(|(key, value)| (*key, *value))
            .collect(),
        issuance_operations: issuance_operations
            .into_iter()
            .map(|(key, value)| (*key, *value))
            .collect(),
        redemption_id_operations: redemption_id_operations
            .into_iter()
            .map(|(key, value)| (*key, *value))
            .collect(),
        terminal_nullifier_operations: terminal_nullifier_operations
            .into_iter()
            .map(|(key, value)| (*key, *value))
            .collect(),
    };
    book.validate()
}

/// Verify that restored transparent custody covers every outstanding reserve liability.
///
/// Liabilities are aggregated across asset incarnations before comparison. This prevents a
/// malformed snapshot from reusing the same deterministic custody balance to back two live
/// incarnations. Dataspace-scoped balances owned by the same reserve account are summed.
pub(crate) fn validate_persisted_reserve_custody_v1<'a>(
    pools: impl IntoIterator<Item = &'a OfflineCashReservePoolV1>,
    assets: impl IntoIterator<Item = (&'a AssetId, &'a AssetValue)>,
) -> Result<(), OfflineCashReserveErrorV1> {
    let mut liabilities = BTreeMap::<(AssetDefinitionId, AccountId), Quantity>::new();
    for pool in pools {
        let available = pool.available()?;
        if available == 0 {
            continue;
        }
        let reserve_account =
            crate::smartcontracts::isi::domain::isi::offline_cash_reserve_account_id(
                &pool.key.network_id,
                &pool.key.asset,
            );
        let required = Quantity::try_from_numeric(Numeric::new(available, pool.scale))
            .map_err(|_| state_invariant("invalid_reserve_liability_quantity"))?;
        let entry = liabilities
            .entry((pool.key.asset.clone(), reserve_account))
            .or_default();
        *entry = entry
            .checked_add(&required)
            .map_err(|_| state_invariant("aggregate_reserve_liability_overflow"))?;
    }

    let mut custody = BTreeMap::<(AssetDefinitionId, AccountId), Quantity>::new();
    for (asset_id, value) in assets {
        let key = (asset_id.definition().clone(), asset_id.account().clone());
        if !liabilities.contains_key(&key) {
            continue;
        }
        let total = custody.entry(key).or_default();
        *total = total
            .checked_add(value.as_ref())
            .map_err(|_| state_invariant("aggregate_reserve_custody_overflow"))?;
    }

    for (key, liability) in liabilities {
        let custody = custody.remove(&key).unwrap_or_default();
        if custody < liability {
            return Err(OfflineCashReserveErrorV1::ReserveCustodyUnderfunded {
                liability,
                custody,
            });
        }
    }
    Ok(())
}

/// Plan a top-up from a bounded exact-entry read set.
///
/// This is the production seam for separate World storage maps. It touches no unrelated history.
///
/// # Errors
///
/// Returns an error for malformed input, conflicts, scale mismatch, or checked overflow.
pub(in crate::smartcontracts::isi) fn plan_top_up_from_entries(
    verified: &VerifiedOfflineCashTopUpIntentV1,
    context: OfflineCashReserveCommitContextV1,
    read_set: OfflineCashTopUpReadSetV1<'_>,
) -> Result<OfflineCashReservePlanOutcomeV1, OfflineCashReserveErrorV1> {
    verified.intent.validate()?;
    context.validate()?;
    let request = &verified.intent.request;
    let mint_statement_digest = verified
        .authorization
        .mint_statement_digest(request, context.committed_at_ms)
        .map_err(OfflineCashReserveErrorV1::InvalidWire)?;
    if let Some(existing) = read_set.existing_operation {
        return match existing {
            OfflineCashReserveOperationRecordV1::TopUp(existing)
                if existing.same_request(&verified.intent) =>
            {
                existing.validate_basic()?;
                if read_set.credit_operation != Some(existing.operation_id)
                    || read_set.issuance_operation != Some(existing.operation_id)
                {
                    return Err(state_invariant("top_up_reverse_index_mismatch"));
                }
                Ok(OfflineCashReservePlanOutcomeV1::AlreadyCommitted(
                    OfflineCashReserveOperationRecordV1::TopUp(existing.clone()),
                ))
            }
            _ => Err(OfflineCashReserveErrorV1::OperationConflict {
                operation_id: request.operation_id,
            }),
        };
    }
    ensure_entry_unbound(read_set.credit_operation, request.credit_id, |credit_id| {
        OfflineCashReserveErrorV1::MintCreditConflict { credit_id }
    })?;
    ensure_entry_unbound(
        read_set.issuance_operation,
        request.issuance_commitment,
        |issuance_commitment| OfflineCashReserveErrorV1::IssuanceConflict {
            issuance_commitment,
        },
    )?;
    let pool = OfflineCashReservePoolKeyV1::from_wire(
        request.network_id,
        request.asset.clone(),
        request.asset_incarnation,
        request.liability_pool_id,
    )?;
    let current = read_set
        .current_pool
        .cloned()
        .unwrap_or_else(|| OfflineCashReservePoolV1::empty(pool.clone(), request.scale));
    current.validate()?;
    if current.key != pool {
        return Err(state_invariant("top_up_read_set_pool_mismatch"));
    }
    require_matching_scale(current.scale, request.scale)?;
    let mut next_pool = next_top_up_pool(&current, request.amount)?;
    let reserve_receipt = receipt_for(
        OfflineCashOperationKindV1::TopUp,
        request.operation_id,
        verified.intent.request_digest,
        mint_statement_digest,
        request.amount,
        &current,
        &next_pool,
        context,
    )?;
    next_pool.latest_receipt = Some(reserve_receipt.clone());
    next_pool.validate()?;
    let record = OfflineCashTopUpRecordV1 {
        version: OFFLINE_CASH_RESERVE_VERSION_V1,
        operation_id: request.operation_id,
        pool,
        amount: request.amount,
        release_id: request.release_id,
        scale: request.scale,
        issuance_commitment: request.issuance_commitment,
        credit_id: request.credit_id,
        payer: request.payer.clone(),
        recipient: request.recipient.clone(),
        issuance_intent: verified.intent.clone(),
        reserve_receipt,
    };
    record.validate_basic()?;
    Ok(OfflineCashReservePlanOutcomeV1::Commit(
        OfflineCashReserveMutationPlanV1::TopUp(OfflineCashTopUpPlanV1 {
            version: OFFLINE_CASH_RESERVE_VERSION_V1,
            expected_pool_head: current.head_digest()?,
            next_pool,
            record,
        }),
    ))
}

/// Plan a redemption from a bounded exact-entry read set.
///
/// # Errors
///
/// Returns an error for malformed input, replay, conflicts, underflow, or overflow.
pub(in crate::smartcontracts::isi) fn plan_redemption_from_entries(
    verified: &VerifiedOfflineCashRedemptionV1,
    context: OfflineCashReserveCommitContextV1,
    read_set: OfflineCashRedemptionReadSetV1<'_>,
) -> Result<OfflineCashReservePlanOutcomeV1, OfflineCashReserveErrorV1> {
    verified
        .request
        .validate_shape()
        .map_err(map_chain_value_error)?;
    context.validate()?;
    if let Some(existing) = read_set.existing_operation {
        return match existing {
            OfflineCashReserveOperationRecordV1::Redemption(existing)
                if existing.same_request(&verified.request) =>
            {
                existing.validate_basic()?;
                if read_set.redemption_operation != Some(existing.operation_id)
                    || read_set.terminal_nullifier_operation != Some(existing.operation_id)
                {
                    return Err(state_invariant("redemption_reverse_index_mismatch"));
                }
                Ok(OfflineCashReservePlanOutcomeV1::AlreadyCommitted(
                    OfflineCashReserveOperationRecordV1::Redemption(existing.clone()),
                ))
            }
            _ => Err(OfflineCashReserveErrorV1::OperationConflict {
                operation_id: verified.operation_id(),
            }),
        };
    }
    let statement = &verified.request.voucher.statement;
    let lifecycle = &statement.lifecycle;
    ensure_entry_unbound(
        read_set.redemption_operation,
        statement.redemption_id,
        |redemption_id| OfflineCashReserveErrorV1::RedemptionIdConflict { redemption_id },
    )?;
    ensure_entry_unbound(
        read_set.terminal_nullifier_operation,
        statement.terminal_nullifier,
        |terminal_nullifier| OfflineCashReserveErrorV1::TerminalNullifierConflict {
            terminal_nullifier,
        },
    )?;
    let request_digest = verified
        .request
        .canonical_digest()
        .map_err(map_chain_value_error)?;
    let pool = OfflineCashReservePoolKeyV1::from_wire(
        lifecycle.network_id,
        lifecycle.asset.clone(),
        lifecycle.asset_incarnation,
        lifecycle.liability_pool_id,
    )?;
    let current = read_set
        .current_pool
        .cloned()
        .unwrap_or_else(|| OfflineCashReservePoolV1::empty(pool.clone(), lifecycle.scale));
    current.validate()?;
    if current.key != pool {
        return Err(state_invariant("redemption_read_set_pool_mismatch"));
    }
    require_matching_scale(current.scale, lifecycle.scale)?;
    let mut next_pool = next_redemption_pool(&current, statement.amount)?;
    let reserve_receipt = receipt_for(
        OfflineCashOperationKindV1::Redemption,
        verified.operation_id(),
        request_digest,
        [0; 32],
        statement.amount,
        &current,
        &next_pool,
        context,
    )?;
    next_pool.latest_receipt = Some(reserve_receipt.clone());
    next_pool.validate()?;
    let record = OfflineCashRedemptionRecordV1 {
        version: OFFLINE_CASH_RESERVE_VERSION_V1,
        operation_id: verified.operation_id(),
        pool,
        amount: statement.amount,
        release_id: lifecycle.release_id,
        scale: lifecycle.scale,
        beneficiary: statement.beneficiary.clone(),
        redemption_id: statement.redemption_id,
        terminal_nullifier: statement.terminal_nullifier,
        redemption_request: verified.request.clone(),
        request_digest,
        reserve_receipt,
    };
    record.validate_basic()?;
    Ok(OfflineCashReservePlanOutcomeV1::Commit(
        OfflineCashReserveMutationPlanV1::Redemption(OfflineCashRedemptionPlanV1 {
            version: OFFLINE_CASH_RESERVE_VERSION_V1,
            expected_pool_head: current.head_digest()?,
            next_pool,
            record,
        }),
    ))
}

/// Recheck a top-up plan against the exact entries read immediately before commit.
///
/// `Ok(None)` authorizes installing the plan's exposed pool, operation, and two reverse-index
/// entries atomically. `Ok(Some(record))` is an exact idempotent retry.
///
/// # Errors
///
/// Returns an error when concurrency changed the pool or an identity is conflicting.
pub(in crate::smartcontracts::isi) fn validate_top_up_commit_entries(
    plan: &OfflineCashTopUpPlanV1,
    read_set: OfflineCashTopUpReadSetV1<'_>,
) -> Result<Option<OfflineCashReserveOperationRecordV1>, OfflineCashReserveErrorV1> {
    require_version(plan.version)?;
    plan.record.validate_basic()?;
    plan.next_pool.validate()?;
    if let Some(existing) = read_set.existing_operation {
        return match existing {
            OfflineCashReserveOperationRecordV1::TopUp(existing)
                if existing.same_request(&plan.record.issuance_intent) =>
            {
                existing.validate_basic()?;
                if read_set.credit_operation != Some(existing.operation_id)
                    || read_set.issuance_operation != Some(existing.operation_id)
                {
                    return Err(state_invariant("top_up_reverse_index_mismatch"));
                }
                Ok(Some(OfflineCashReserveOperationRecordV1::TopUp(
                    existing.clone(),
                )))
            }
            _ => Err(OfflineCashReserveErrorV1::OperationConflict {
                operation_id: plan.record.operation_id,
            }),
        };
    }
    ensure_entry_unbound(
        read_set.credit_operation,
        plan.record.credit_id,
        |credit_id| OfflineCashReserveErrorV1::MintCreditConflict { credit_id },
    )?;
    ensure_entry_unbound(
        read_set.issuance_operation,
        plan.record.issuance_commitment,
        |issuance_commitment| OfflineCashReserveErrorV1::IssuanceConflict {
            issuance_commitment,
        },
    )?;
    let current = read_set.current_pool.cloned().unwrap_or_else(|| {
        OfflineCashReservePoolV1::empty(plan.record.pool.clone(), plan.record.scale)
    });
    current.validate()?;
    if current.key != plan.record.pool {
        return Err(state_invariant("top_up_read_set_pool_mismatch"));
    }
    require_matching_scale(current.scale, plan.record.scale)?;
    let current_head = current.head_digest()?;
    if current_head != plan.expected_pool_head {
        return Err(OfflineCashReserveErrorV1::StalePlan {
            expected: plan.expected_pool_head,
            actual: current_head,
        });
    }
    plan.record
        .reserve_receipt
        .validate_against_previous_receipt(current.latest_receipt.as_ref())
        .map_err(map_chain_value_error)?;
    let mut expected = next_top_up_pool(&current, plan.record.amount)?;
    expected.latest_receipt = Some(plan.record.reserve_receipt.clone());
    if expected != plan.next_pool
        || !receipt_matches_pool_projection(
            &plan.record.reserve_receipt,
            &expected,
            OfflineCashOperationKindV1::TopUp,
            plan.record.operation_id,
            plan.record.issuance_intent.request_digest,
            plan.record.reserve_receipt.mint_statement_digest,
            plan.record.amount,
        )
    {
        return Err(OfflineCashReserveErrorV1::InvalidPlan);
    }
    Ok(None)
}

/// Recheck a redemption plan against exact entries immediately before commit.
///
/// `Ok(None)` authorizes the four exposed entry writes; `Ok(Some(record))` is an exact retry.
///
/// # Errors
///
/// Returns an error when concurrency changed the pool or a replay identity is conflicting.
pub(in crate::smartcontracts::isi) fn validate_redemption_commit_entries(
    plan: &OfflineCashRedemptionPlanV1,
    read_set: OfflineCashRedemptionReadSetV1<'_>,
) -> Result<Option<OfflineCashReserveOperationRecordV1>, OfflineCashReserveErrorV1> {
    require_version(plan.version)?;
    plan.record.validate_basic()?;
    plan.next_pool.validate()?;
    if let Some(existing) = read_set.existing_operation {
        return match existing {
            OfflineCashReserveOperationRecordV1::Redemption(existing)
                if existing.same_request(&plan.record.redemption_request) =>
            {
                existing.validate_basic()?;
                if read_set.redemption_operation != Some(existing.operation_id)
                    || read_set.terminal_nullifier_operation != Some(existing.operation_id)
                {
                    return Err(state_invariant("redemption_reverse_index_mismatch"));
                }
                Ok(Some(OfflineCashReserveOperationRecordV1::Redemption(
                    existing.clone(),
                )))
            }
            _ => Err(OfflineCashReserveErrorV1::OperationConflict {
                operation_id: plan.record.operation_id,
            }),
        };
    }
    ensure_entry_unbound(
        read_set.redemption_operation,
        plan.record.redemption_id,
        |redemption_id| OfflineCashReserveErrorV1::RedemptionIdConflict { redemption_id },
    )?;
    ensure_entry_unbound(
        read_set.terminal_nullifier_operation,
        plan.record.terminal_nullifier,
        |terminal_nullifier| OfflineCashReserveErrorV1::TerminalNullifierConflict {
            terminal_nullifier,
        },
    )?;
    let current = read_set.current_pool.cloned().unwrap_or_else(|| {
        OfflineCashReservePoolV1::empty(plan.record.pool.clone(), plan.record.scale)
    });
    current.validate()?;
    if current.key != plan.record.pool {
        return Err(state_invariant("redemption_read_set_pool_mismatch"));
    }
    require_matching_scale(current.scale, plan.record.scale)?;
    let current_head = current.head_digest()?;
    if current_head != plan.expected_pool_head {
        return Err(OfflineCashReserveErrorV1::StalePlan {
            expected: plan.expected_pool_head,
            actual: current_head,
        });
    }
    plan.record
        .reserve_receipt
        .validate_against_previous_receipt(current.latest_receipt.as_ref())
        .map_err(map_chain_value_error)?;
    let mut expected = next_redemption_pool(&current, plan.record.amount)?;
    expected.latest_receipt = Some(plan.record.reserve_receipt.clone());
    if expected != plan.next_pool
        || !receipt_matches_pool_projection(
            &plan.record.reserve_receipt,
            &expected,
            OfflineCashOperationKindV1::Redemption,
            plan.record.operation_id,
            plan.record.request_digest,
            [0; 32],
            plan.record.amount,
        )
    {
        return Err(OfflineCashReserveErrorV1::InvalidPlan);
    }
    Ok(None)
}

/// Join one immutable consensus top-up record with finalized proof material.
///
/// This helper never changes finalized WSV. `existing_attachment` is a derived durable local
/// operation-result/outbox entry (or the result of a later explicitly modeled transaction), not a
/// field retroactively written into the consensus record. `Ok(Finalized(_))` instructs the caller
/// to cache the returned bytes; passing that cache on retry produces `AlreadyFinalized`.
///
/// # Errors
///
/// Returns an error for result/intent mismatch or different already-finalized bytes.
pub(in crate::smartcontracts::isi) fn finalize_mint_credit_record(
    existing: &OfflineCashTopUpRecordV1,
    existing_attachment: Option<&OfflineCashMintFinalityAttachmentV1>,
    verified: &VerifiedOfflineCashMintFinalizationV1,
) -> Result<OfflineCashMintFinalizationOutcomeV1, OfflineCashReserveErrorV1> {
    existing.validate_basic()?;
    if verified.operation_id() != existing.operation_id {
        return Err(OfflineCashReserveErrorV1::MintFinalizationMismatch {
            operation_id: existing.operation_id,
        });
    }
    let attachment = OfflineCashMintFinalityAttachmentV1 {
        version: OFFLINE_CASH_RESERVE_VERSION_V1,
        result: verified.result.clone(),
        result_wire_digest: canonical_wire_digest(
            TOP_UP_RESULT_WIRE_DIGEST_DOMAIN_V1,
            &verified.result,
        )?,
        verified_anchor_identity: verified.trusted_anchor_identity,
    };
    validate_mint_finality_attachment(existing, &attachment)?;
    if let Some(committed) = existing_attachment {
        return if committed == &attachment {
            Ok(OfflineCashMintFinalizationOutcomeV1::AlreadyFinalized(
                committed.clone(),
            ))
        } else {
            Err(OfflineCashReserveErrorV1::MintFinalizationConflict {
                operation_id: existing.operation_id,
            })
        };
    }
    Ok(OfflineCashMintFinalizationOutcomeV1::Finalized(attachment))
}

/// Re-authenticate a decoded local finalization attachment against canonical chain context.
///
/// # Errors
///
/// Returns an error when the immutable WSV record differs, the anchor cannot be resolved exactly,
/// or typed finality fails cryptographic verification.
pub fn validate_mint_finality_attachment_with_anchor(
    record: &OfflineCashTopUpRecordV1,
    attachment: &OfflineCashMintFinalityAttachmentV1,
    resolver: &impl OfflineCashFinalityAnchorResolverV1,
) -> Result<(), OfflineCashReserveErrorV1> {
    record.validate_basic()?;
    validate_mint_finality_attachment(record, attachment)?;
    let canonical = resolver
        .resolve(&attachment.verified_anchor_identity)
        .ok_or(OfflineCashReserveErrorV1::FinalityAnchorUnavailable {
            operation_id: record.operation_id,
        })?;
    if canonical != attachment.verified_anchor_identity {
        return Err(OfflineCashReserveErrorV1::InvalidFinalityEvidence {
            reason: "persisted_anchor_identity_is_not_canonical",
        });
    }
    attachment.result.validate_against(&canonical).map_err(|_| {
        OfflineCashReserveErrorV1::InvalidFinalityEvidence {
            reason: "terminal_result_failed_rehydration_validation",
        }
    })
}

fn require_version(version: u16) -> Result<(), OfflineCashReserveErrorV1> {
    if version != OFFLINE_CASH_RESERVE_VERSION_V1 {
        return Err(OfflineCashReserveErrorV1::UnsupportedVersion { actual: version });
    }
    Ok(())
}

fn require_nonzero_operation(operation_id: [u8; 32]) -> Result<(), OfflineCashReserveErrorV1> {
    if operation_id == [0; 32] {
        return Err(OfflineCashReserveErrorV1::InvalidOperationId);
    }
    Ok(())
}

fn require_matching_scale(expected: u32, actual: u32) -> Result<(), OfflineCashReserveErrorV1> {
    if expected != actual {
        return Err(OfflineCashReserveErrorV1::AssetScaleConflict { expected, actual });
    }
    Ok(())
}

fn next_top_up_pool(
    current: &OfflineCashReservePoolV1,
    amount: u128,
) -> Result<OfflineCashReservePoolV1, OfflineCashReserveErrorV1> {
    Ok(OfflineCashReservePoolV1 {
        version: OFFLINE_CASH_RESERVE_VERSION_V1,
        key: current.key.clone(),
        scale: current.scale,
        total_topups: current
            .total_topups
            .checked_add(amount)
            .ok_or(OfflineCashReserveErrorV1::TotalTopUpsOverflow)?,
        total_redemptions: current.total_redemptions,
        latest_receipt: current.latest_receipt.clone(),
    })
}

fn next_redemption_pool(
    current: &OfflineCashReservePoolV1,
    amount: u128,
) -> Result<OfflineCashReservePoolV1, OfflineCashReserveErrorV1> {
    let available = current.available()?;
    if amount > available {
        return Err(OfflineCashReserveErrorV1::ReserveUnderflow {
            available,
            requested: amount,
        });
    }
    Ok(OfflineCashReservePoolV1 {
        version: OFFLINE_CASH_RESERVE_VERSION_V1,
        key: current.key.clone(),
        scale: current.scale,
        total_topups: current.total_topups,
        total_redemptions: current
            .total_redemptions
            .checked_add(amount)
            .ok_or(OfflineCashReserveErrorV1::TotalRedemptionsOverflow)?,
        latest_receipt: current.latest_receipt.clone(),
    })
}

/// Reconstruct the exact public mint statement from the already verified top-up intent.
///
/// Credential/profile and precommit-proof authentication is represented by
/// `VerifiedOfflineCashTopUpAuthorizationV1`
/// before planning. Keeping this field adapter in one place makes schema evolution explicit and
/// prevents reserve accounting from silently omitting a lifecycle binding.
fn mint_statement_from_request(
    request: &OfflineCashTopUpRequestV1,
    minted_at_ms: u64,
) -> Result<OfflineCashMintCreditStatementV1, OfflineCashReserveErrorV1> {
    let authorization_context_digest = request
        .mint_authorization_context()
        .canonical_digest()
        .map_err(map_chain_value_error)?;
    let mint_authorization_digest = request
        .mint_authorization
        .as_ref()
        .ok_or_else(|| state_invariant("top_up_missing_mint_authorization"))?
        .canonical_digest()
        .map_err(map_chain_value_error)?;
    let statement = OfflineCashMintCreditStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        lifecycle: OfflineCashLifecycleBindingV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: request.network_id,
            protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
            suite_id: request.suite_id,
            vk_digest: request.vk_digest,
            release_id: request.release_id,
            asset: request.asset.clone(),
            asset_incarnation: request.asset_incarnation,
            scale: request.scale,
            liability_pool_id: request.liability_pool_id,
            hardware_profile_id: request.hardware_credential.hardware_profile_id,
            policy_epoch: request.hardware_credential.policy_epoch,
            operation_kind: iroha_data_model::offline::OfflineCashOperationKindV1::MintFold,
            request_id: [0; 32],
            acceptance_ticket_id: [0; 32],
            credit_id: request.credit_id,
            ciphertext_digest: offline_cash_ciphertext_digest_v1(&request.encrypted_credit),
        },
        recipient_credential_commitment: request.recipient_credential_commitment,
        authorization_context_digest,
        mint_authorization_digest,
        amount: request.amount,
        issuance_commitment: request.issuance_commitment,
        recipient: request.recipient.clone(),
        credit_commitment: request.credit_commitment,
        minted_at_ms,
    };
    statement.validate_shape().map_err(map_chain_value_error)?;
    Ok(statement)
}

fn receipt_for(
    kind: OfflineCashOperationKindV1,
    operation_id: [u8; 32],
    request_digest: [u8; 32],
    mint_statement_digest: [u8; 32],
    amount: u128,
    current: &OfflineCashReservePoolV1,
    next: &OfflineCashReservePoolV1,
    context: OfflineCashReserveCommitContextV1,
) -> Result<OfflineCashReserveReceiptV1, OfflineCashReserveErrorV1> {
    let receipt = OfflineCashReserveReceiptV1 {
        version: OFFLINE_CASH_RESERVE_VERSION_V1,
        operation_id,
        kind,
        request_digest,
        mint_statement_digest,
        network_id: next.key.network_id,
        asset: next.key.asset.clone(),
        asset_incarnation: next.key.asset_incarnation,
        scale: next.scale,
        liability_pool_id: next.key.liability_pool_id,
        amount,
        previous_pool_receipt_digest: current.head_digest()?.unwrap_or([0; 32]),
        total_topups: next.total_topups,
        total_redemptions: next.total_redemptions,
        transaction_hash: context.transaction_hash,
        committed_at_ms: context.committed_at_ms,
    };
    receipt
        .validate_against_previous_receipt(current.latest_receipt.as_ref())
        .map_err(map_chain_value_error)?;
    Ok(receipt)
}

fn receipt_matches(
    receipt: &OfflineCashReserveReceiptV1,
    kind: OfflineCashOperationKindV1,
    operation_id: [u8; 32],
    request_digest: [u8; 32],
    mint_statement_digest: [u8; 32],
    pool: &OfflineCashReservePoolKeyV1,
    scale: u32,
    amount: u128,
) -> bool {
    receipt.version == OFFLINE_CASH_RESERVE_VERSION_V1
        && receipt.operation_id == operation_id
        && receipt.kind == kind
        && receipt.request_digest == request_digest
        && receipt.mint_statement_digest == mint_statement_digest
        && receipt.network_id == pool.network_id
        && receipt.asset == pool.asset
        && receipt.asset_incarnation == pool.asset_incarnation
        && receipt.scale == scale
        && receipt.liability_pool_id == pool.liability_pool_id
        && receipt.amount == amount
}

fn receipt_matches_pool_projection(
    receipt: &OfflineCashReserveReceiptV1,
    next: &OfflineCashReservePoolV1,
    kind: OfflineCashOperationKindV1,
    operation_id: [u8; 32],
    request_digest: [u8; 32],
    mint_statement_digest: [u8; 32],
    amount: u128,
) -> bool {
    receipt_matches(
        receipt,
        kind,
        operation_id,
        request_digest,
        mint_statement_digest,
        &next.key,
        next.scale,
        amount,
    ) && receipt.total_topups == next.total_topups
        && receipt.total_redemptions == next.total_redemptions
        && next.latest_receipt.as_ref() == Some(receipt)
}

fn validate_mint_finality_attachment(
    record: &OfflineCashTopUpRecordV1,
    attachment: &OfflineCashMintFinalityAttachmentV1,
) -> Result<(), OfflineCashReserveErrorV1> {
    require_version(attachment.version)?;
    require_version(attachment.result.version)?;
    require_version(attachment.result.finality.version)?;
    attachment
        .result
        .request
        .validate_shape()
        .map_err(map_chain_value_error)?;
    attachment
        .result
        .mint_credit
        .validate_shape()
        .map_err(map_chain_value_error)?;
    attachment
        .result
        .finality
        .reserve_receipt_witness
        .receipt
        .validate()
        .map_err(map_chain_value_error)?;
    attachment
        .verified_anchor_identity
        .validate()
        .map_err(|_| OfflineCashReserveErrorV1::InvalidFinalityEvidence {
            reason: "invalid_persisted_anchor_identity",
        })?;
    let result = &attachment.result;
    let receipt = &result.finality.reserve_receipt_witness.receipt;
    let expected_statement = mint_statement_from_request(&result.request, receipt.committed_at_ms)?;
    if result.request != record.issuance_intent.request
        || receipt != &record.reserve_receipt
        || result.mint_credit.statement != expected_statement
        || result.mint_credit.encrypted_credit != result.request.encrypted_credit
        || result.mint_credit.artifact_manifest_digest != result.request.artifact_manifest_digest
        || attachment.result_wire_digest
            != canonical_wire_digest(TOP_UP_RESULT_WIRE_DIGEST_DOMAIN_V1, result)?
        || attachment.verified_anchor_identity.network_id != record.pool.network_id
        || attachment.verified_anchor_identity.block_height
            != result.finality.finality_artifact.height
        || attachment.verified_anchor_identity.height_context_id
            != result.finality.finality_artifact.context_id()
    {
        return Err(OfflineCashReserveErrorV1::MintFinalizationMismatch {
            operation_id: record.operation_id,
        });
    }
    Ok(())
}

fn canonical_wire_digest<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], OfflineCashReserveErrorV1> {
    let encoded = norito::encode_canonical(value)
        .map_err(|error| OfflineCashReserveErrorV1::Encoding(error.to_string()))?;
    let encoded_len = u64::try_from(encoded.len())
        .map_err(|_| OfflineCashReserveErrorV1::Encoding("wire length exceeds u64".to_owned()))?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(encoded_len.to_le_bytes());
    hasher.update(encoded);
    Ok(hasher.finalize().into())
}

fn ensure_entry_unbound<E>(
    bound_operation: Option<[u8; 32]>,
    identity: [u8; 32],
    conflict: impl FnOnce([u8; 32]) -> E,
) -> Result<(), E> {
    match bound_operation {
        None => Ok(()),
        Some(_) => Err(conflict(identity)),
    }
}

fn map_chain_value_error(error: impl core::fmt::Display) -> OfflineCashReserveErrorV1 {
    OfflineCashReserveErrorV1::InvalidWire(error.to_string())
}

fn state_invariant(reason: &'static str) -> OfflineCashReserveErrorV1 {
    OfflineCashReserveErrorV1::StateInvariant { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::halo2curves::{
        group::{Curve as _, Group as _},
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    };
    use iroha_crypto::{
        Algorithm, Hash, HashOf, KeyPair, Signature as IrohaSignature,
        bls_normal_aggregate_signatures, bls_normal_pop_prove,
    };
    use iroha_data_model::{
        IntoKeyValue,
        asset::Asset,
        block::{
            BlockHeader,
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
                QuorumCertificate, ValidatorPower, Vote,
                encode_offline_cash_consensus_signature_envelope_v1, finality::V2FinalityArtifact,
            },
        },
        domain::DomainId,
        isi::{
            OFFLINE_CASH_CHAIN_VERSION_V1, OFFLINE_CASH_RESERVE_RECEIPT_WITNESS_SIBLINGS_V1,
            OfflineCashMintFinalitySealBundleV1, OfflineCashMintFinalitySealMessageV1,
            OfflineCashMintFinalityValidatorSealV1, OfflineCashOperationFinalityV1,
            OfflineCashPastaSchnorrSignatureV1, OfflineCashReserveReceiptWitnessV1,
            OfflineCashTopUpMembershipWitnessV1, offline_cash_mint_finality_root_v1,
        },
        offline::{
            OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1,
            OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1, OFFLINE_CASH_WIRE_VERSION_V1,
            OfflineCashCommitCertificateV1, OfflineCashCommitEvidenceV1,
            OfflineCashCommitWrapperProofV1, OfflineCashDevicePublicKeyV1,
            OfflineCashDeviceSignatureV1, OfflineCashHardwareCredentialV1,
            OfflineCashHardwarePlatformClassV1, OfflineCashHardwareProfileV1,
            OfflineCashLifecycleBindingV1, OfflineCashMintCreditV1, OfflineCashOutboxReservationV1,
            OfflineCashPairedProofV1, OfflineCashRedemptionStatementV1,
            OfflineCashRedemptionVoucherV1, OfflineCashTrustedCommitTimeV1,
            offline_cash_device_key_reference_v1, offline_cash_suite_commitment_v1,
        },
        peer::PeerId,
    };
    use iroha_primitives::numeric::{Numeric, Quantity};
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
    use snark_verifier::{loader::native::NativeLoader, pcs::ipa::IpaAccumulator};

    use crate::zk::offline_cash_v1_recursion::{
        OFFLINE_CASH_RECURSION_IPA_K_V1, OfflineCashEpAccumulatorV1, OfflineCashEqAccumulatorV1,
        OfflineCashMintAuthorityPairBindingV1, OfflineCashMintAuthorityStepV1,
    };

    fn tagged_id(tag: u8, nonce: u64) -> [u8; 32] {
        let mut id = [tag; 32];
        id[24..].copy_from_slice(&nonce.to_be_bytes());
        id
    }

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"offline-cash-v1-reserve",
        )))
    }

    fn other_network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"offline-cash-v1-other-network",
        )))
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }

    fn asset_incarnation(seed: u8) -> AxtAssetIncarnationV1 {
        AxtAssetIncarnationV1::try_from_bytes(*Hash::new([seed]).as_ref())
            .expect("canonical asset incarnation")
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("P-256 signing key")
    }

    fn device_signing_key(device_nonce: u64) -> SigningKey {
        // Keep test devices genuinely distinct beyond 255 entries. The fixed nonzero prefix
        // keeps every test scalar below the P-256 group order while the suffix provides a
        // collision-free identity for the complete u64 fixture range.
        let mut seed = [1_u8; 32];
        seed[24..].copy_from_slice(&device_nonce.to_be_bytes());
        SigningKey::from_bytes((&seed).into()).expect("P-256 device signing key")
    }

    fn public_key(key: &SigningKey) -> OfflineCashDevicePublicKeyV1 {
        OfflineCashDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("device public key")
    }

    fn sign(key: &SigningKey, bytes: &[u8]) -> OfflineCashDeviceSignatureV1 {
        let signature: P256Signature = key.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        OfflineCashDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical device signature")
    }

    const fn suite_id() -> [u8; 32] {
        [0x51; 32]
    }

    fn hardware_profile() -> OfflineCashHardwareProfileV1 {
        OfflineCashHardwareProfileV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
            hardware_profile_id: [0; 32],
            provider_id: [0x52; 32],
            platform_class: OfflineCashHardwarePlatformClassV1::DedicatedSecureElement,
            product_class_digest: [0x53; 32],
            firmware_policy_digest: [0x54; 32],
            enrollment_attestation_verifier_digest: [0x55; 32],
            attestation_trust_roots_digest: [0x56; 32],
            allowed_suite_commitment: offline_cash_suite_commitment_v1(suite_id()),
            policy_epoch: 1,
            governance_credential_public_key: public_key(&signing_key(0x31)),
            capability_mask: OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1,
            qualification_report_digest: [0x57; 32],
            valid_from_ms: 1,
            expires_at_ms: 100_000,
        }
        .seal_hardware_profile_id()
        .expect("hardware profile id")
    }

    fn hardware_credential(
        profile: &OfflineCashHardwareProfileV1,
        device_nonce: u64,
    ) -> OfflineCashHardwareCredentialV1 {
        let device_key = device_signing_key(device_nonce);
        let device_public_key = public_key(&device_key);
        let governance_key = signing_key(0x31);
        let mut credential = OfflineCashHardwareCredentialV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credential_id: [0; 32],
            network_id: network(),
            hardware_profile_id: profile.hardware_profile_id,
            suite_id: suite_id(),
            firmware_policy_digest: profile.firmware_policy_digest,
            policy_epoch: profile.policy_epoch,
            lane_commitment: tagged_id(0x58, device_nonce),
            hardware_epoch_id: tagged_id(0x59, device_nonce),
            hardware_epoch_generation: 1,
            device_public_key,
            device_key_reference: offline_cash_device_key_reference_v1(&device_public_key),
            issued_at_ms: 500,
            expires_at_ms: 90_000,
            governance_signature: sign(&governance_key, b"placeholder"),
        }
        .seal_credential_id()
        .expect("credential id");
        credential.governance_signature = sign(
            &governance_key,
            &credential
                .canonical_signing_bytes()
                .expect("credential signing bytes"),
        );
        credential
            .validate_against_profile(profile)
            .expect("credential/profile binding");
        credential
    }

    fn paired_proof(semantic_digest: [u8; 32]) -> OfflineCashPairedProofV1 {
        let eq_history = OfflineCashEqAccumulatorV1::from_native(&IpaAccumulator::<
            EqAffine,
            NativeLoader,
        >::new(
            (0..OFFLINE_CASH_RECURSION_IPA_K_V1)
                .map(|round| Fp::from(u64::from(round) + 1))
                .collect(),
            (Eq::generator() * Fp::from(97)).to_affine(),
        ))
        .expect("canonical Eq mint-authority history");
        let ep_history = OfflineCashEpAccumulatorV1::from_native(&IpaAccumulator::<
            EpAffine,
            NativeLoader,
        >::new(
            (0..OFFLINE_CASH_RECURSION_IPA_K_V1)
                .map(|round| Fq::from(u64::from(round) + 1))
                .collect(),
            (Ep::generator() * Fq::from(193)).to_affine(),
        ))
        .expect("canonical Ep mint-authority history");
        OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: [0x81; 32],
            ep_protocol_digest: [0x92; 32],
            semantic_digest,
            guard_eq_credential_audit: [0x19; 32],
            guard_ep_credential_audit: [0x1A; 32],
            eq_deferred_audit: [0x13; 32],
            ep_deferred_audit: [0x14; 32],
            eq_proof: vec![0xA1; 128],
            ep_proof: vec![0xB2; 128],
            eq_history: eq_history.as_bytes().to_vec(),
            ep_history: ep_history.as_bytes().to_vec(),
        }
    }

    fn commit_certificate_digest(certificate: &OfflineCashCommitCertificateV1) -> [u8; 32] {
        let bytes = norito::encode_canonical(certificate).expect("canonical commit certificate");
        let mut hasher = Sha256::new();
        hasher.update(b"iroha:offline-cash:v1:commit-certificate");
        hasher.update([0]);
        hasher.update(
            u64::try_from(bytes.len())
                .expect("certificate length")
                .to_le_bytes(),
        );
        hasher.update(bytes);
        hasher.finalize().into()
    }

    fn top_up_request(
        operation_nonce: u64,
        device_nonce: u64,
        amount: u128,
        scale: u32,
    ) -> OfflineCashTopUpRequestV1 {
        let profile = hardware_profile();
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(1);
        let request = OfflineCashTopUpRequestV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            operation_id: tagged_id(0x11, operation_nonce),
            issuance_commitment: [0; 32],
            credit_id: [0; 32],
            release_id: tagged_id(0x12, device_nonce),
            suite_id: suite_id(),
            vk_digest: tagged_id(0x5A, device_nonce),
            network_id,
            asset: asset.clone(),
            asset_incarnation,
            scale,
            amount,
            liability_pool_id: offline_cash_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            payer: account(0xA5),
            recipient: account(0xB6),
            hardware_credential: hardware_credential(&profile, device_nonce),
            recipient_credential_commitment: tagged_id(0x13, device_nonce),
            credit_commitment: tagged_id(0x16, device_nonce),
            recipient_one_time_key: tagged_id(0x1A, device_nonce),
            encrypted_credit: tagged_id(0x17, device_nonce).to_vec(),
            artifact_manifest_digest: tagged_id(0x18, device_nonce),
            mint_authorization: None,
        }
        .seal_identifiers()
        .expect("seal top-up identifiers");
        let statement = request
            .mint_authorization_statement()
            .expect("mint authorization statement");
        let proof = paired_proof(
            statement
                .canonical_digest()
                .expect("mint authorization statement digest"),
        );
        request
            .attach_mint_authorization(iroha_data_model::offline::OfflineCashMintAuthorizationV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                statement,
                proof,
            })
            .expect("attach mint authorization")
    }

    #[test]
    fn persisted_separate_reserve_maps_validate_storage_keys() {
        let empty_pools = BTreeMap::<[u8; 32], OfflineCashReservePoolV1>::new();
        let empty_operations = BTreeMap::<[u8; 32], OfflineCashReserveOperationRecordV1>::new();
        let empty_indexes = BTreeMap::<[u8; 32], [u8; 32]>::new();
        validate_persisted_reserve_entries_v1(
            empty_pools.iter(),
            empty_operations.iter(),
            empty_indexes.iter(),
            empty_indexes.iter(),
            empty_indexes.iter(),
            empty_indexes.iter(),
        )
        .expect("empty first-release reserve state is valid");

        let key = OfflineCashReservePoolKeyV1::new(network(), asset(), asset_incarnation(1))
            .expect("pool key");
        let mut pools = BTreeMap::new();
        pools.insert([0xFF; 32], OfflineCashReservePoolV1::empty(key, 0));
        assert_eq!(
            validate_persisted_reserve_entries_v1(
                pools.iter(),
                empty_operations.iter(),
                empty_indexes.iter(),
                empty_indexes.iter(),
                empty_indexes.iter(),
                empty_indexes.iter(),
            ),
            Err(OfflineCashReserveErrorV1::StateInvariant {
                reason: "pool_storage_key_mismatch",
            })
        );
    }

    fn verified_top_up(
        operation_nonce: u64,
        device_nonce: u64,
        amount: u128,
    ) -> VerifiedOfflineCashTopUpIntentV1 {
        let request = top_up_request(operation_nonce, device_nonce, amount, 4);
        let profile = hardware_profile();
        let authorization = VerifiedOfflineCashTopUpAuthorizationV1 {
            request_digest: request.canonical_digest().expect("request digest"),
            mint_authorization_digest: request
                .mint_authorization
                .as_ref()
                .expect("mint authorization")
                .canonical_digest()
                .expect("mint authorization digest"),
            profile,
        };
        VerifiedOfflineCashTopUpIntentV1::after_admission_verification(request, authorization)
            .expect("verified top-up")
    }

    fn redemption_voucher(nonce: u64, amount: u128) -> OfflineCashRedemptionVoucherV1 {
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(1);
        let profile = hardware_profile();
        let commit_evidence =
            OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
                time_evidence_commitment: tagged_id(0x40, nonce),
            });
        let statement = OfflineCashRedemptionStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            lifecycle: OfflineCashLifecycleBindingV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                network_id,
                protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
                suite_id: suite_id(),
                vk_digest: tagged_id(0x41, nonce),
                release_id: tagged_id(0x21, nonce),
                asset: asset.clone(),
                asset_incarnation,
                scale: 4,
                liability_pool_id: offline_cash_liability_pool_id_v1(
                    &network_id,
                    &asset,
                    asset_incarnation,
                )
                .expect("liability pool"),
                hardware_profile_id: profile.hardware_profile_id,
                policy_epoch: profile.policy_epoch,
                operation_kind: iroha_data_model::offline::OfflineCashOperationKindV1::RedeemSplit,
                request_id: [0; 32],
                acceptance_ticket_id: [0; 32],
                credit_id: [0; 32],
                ciphertext_digest: [0; 32],
            },
            amount,
            beneficiary: account(0xC7),
            terminal_nullifier: tagged_id(0x25, nonce),
            redemption_commitment: tagged_id(0x27, nonce),
            redemption_id: [0; 32],
            commit_evidence,
        }
        .seal_redemption_id()
        .expect("seal redemption statement");
        let semantic_digest = statement.canonical_digest().expect("redemption digest");
        let commit_certificate = OfflineCashCommitCertificateV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            certificate_id: [0; 32],
            candidate_envelope_digest: tagged_id(0x43, nonce),
            lifecycle_binding_digest: statement
                .lifecycle
                .canonical_digest()
                .expect("lifecycle digest"),
            transition_nullifier: statement.terminal_nullifier,
            outbox_reservation_commitment: OfflineCashOutboxReservationV1 {
                reservation_id: tagged_id(0x44, nonce),
                operation_kind: iroha_data_model::offline::OfflineCashOperationKindV1::RedeemSplit,
                reserved_outbox_bytes: OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
                issued_at_ms: 8_000,
                expires_at_ms: 10_000,
            }
            .canonical_commitment()
            .expect("outbox commitment"),
            commit_evidence,
            hardware_profile_id: statement.lifecycle.hardware_profile_id,
            policy_epoch: statement.lifecycle.policy_epoch,
            hardware_terminal_commitment: tagged_id(0x45, nonce),
        }
        .seal_certificate_id()
        .expect("certificate id");
        let certificate_digest = commit_certificate_digest(&commit_certificate);
        let paired = paired_proof(semantic_digest);
        OfflineCashRedemptionVoucherV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            commit_certificate: commit_certificate.clone(),
            proof: OfflineCashCommitWrapperProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: paired.eq_protocol_digest,
                ep_protocol_digest: paired.ep_protocol_digest,
                semantic_digest,
                candidate_envelope_digest: commit_certificate.candidate_envelope_digest,
                commit_certificate_digest: certificate_digest,
                eq_deferred_audit: paired.eq_deferred_audit,
                ep_deferred_audit: paired.ep_deferred_audit,
                eq_proof: paired.eq_proof,
                ep_proof: paired.ep_proof,
                eq_history: paired.eq_history,
                ep_history: paired.ep_history,
            },
            artifact_manifest_digest: tagged_id(0x28, nonce),
        }
    }

    fn verified_redemption(
        operation_nonce: u64,
        voucher_nonce: u64,
        amount: u128,
    ) -> VerifiedOfflineCashRedemptionV1 {
        after_mock_recursive_verification(OfflineCashRedemptionRequestV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            operation_id: tagged_id(0x31, operation_nonce),
            voucher: redemption_voucher(voucher_nonce, amount),
        })
    }

    fn after_mock_recursive_verification(
        request: OfflineCashRedemptionRequestV1,
    ) -> VerifiedOfflineCashRedemptionV1 {
        let verified =
            VerifiedOfflineCashRedemptionProofV1::for_reserve_tests_after_mock_recursive_verification(
                request,
            )
            .expect("mock both recursive parities");
        VerifiedOfflineCashRedemptionV1::after_full_verification(verified)
    }

    fn commit_context(nonce: u64) -> OfflineCashReserveCommitContextV1 {
        OfflineCashReserveCommitContextV1::after_block_context_verification(
            tagged_id(0x41, nonce),
            20_000 + nonce,
        )
        .expect("block context")
    }

    fn expect_plan(outcome: OfflineCashReservePlanOutcomeV1) -> OfflineCashReserveMutationPlanV1 {
        match outcome {
            OfflineCashReservePlanOutcomeV1::Commit(plan) => plan,
            OfflineCashReservePlanOutcomeV1::AlreadyCommitted(_) => {
                panic!("expected a new plan")
            }
        }
    }

    fn commit_top_up(
        book: &mut OfflineCashReserveBookV1,
        verified: &VerifiedOfflineCashTopUpIntentV1,
        context_nonce: u64,
    ) {
        let plan = expect_plan(
            book.plan_top_up(verified, commit_context(context_nonce))
                .expect("plan top-up"),
        );
        assert!(matches!(
            book.commit(plan).expect("commit top-up"),
            OfflineCashReserveCommitOutcomeV1::Committed(
                OfflineCashReserveOperationRecordV1::TopUp(_)
            )
        ));
    }

    fn commit_redemption(
        book: &mut OfflineCashReserveBookV1,
        verified: &VerifiedOfflineCashRedemptionV1,
        context_nonce: u64,
    ) {
        let plan = expect_plan(
            book.plan_redemption(verified, commit_context(context_nonce))
                .expect("plan redemption"),
        );
        assert!(matches!(
            book.commit(plan).expect("commit redemption"),
            OfflineCashReserveCommitOutcomeV1::Committed(
                OfflineCashReserveOperationRecordV1::Redemption(_)
            )
        ));
    }

    fn finality_fixture(
        book: &OfflineCashReserveBookV1,
        verified: &VerifiedOfflineCashTopUpIntentV1,
        nonce: u8,
    ) -> (OfflineCashTopUpResultV1, OfflineCashFinalityTrustAnchorV1) {
        let record = match book.operation(&verified.operation_id()) {
            Some(OfflineCashReserveOperationRecordV1::TopUp(record)) => record,
            _ => panic!("committed top-up record"),
        };
        let request = verified.intent.request.clone();
        let statement = verified
            .mint_statement(record.reserve_receipt.committed_at_ms)
            .expect("authoritative mint statement");
        let proof = paired_proof(statement.canonical_digest().expect("mint statement digest"));
        let witness = OfflineCashReserveReceiptWitnessV1 {
            key: OfflineCashReserveReceiptWitnessV1::expected_key(request.operation_id),
            receipt: record.reserve_receipt.clone(),
            siblings: vec![Hash::new([nonce]); OFFLINE_CASH_RESERVE_RECEIPT_WITNESS_SIBLINGS_V1],
        };
        let ordinary_writes_root = witness.reconstructed_root().expect("receipt root");
        let top_up_leaf =
            crate::zk::offline_cash_v1_recursion::offline_cash_top_up_leaf_from_receipt_v1(
                &record.reserve_receipt,
            )
            .expect("top-up leaf");
        let top_up_membership_witness = OfflineCashTopUpMembershipWitnessV1 {
            leaf: top_up_leaf,
            leaf_index: 0,
            root: iroha_data_model::offline::OfflineCashPastaStateCommitmentV1 {
                eq: [0x31; 32],
                ep: [0x32; 32],
            },
            siblings: vec![
                iroha_data_model::offline::OfflineCashPastaStateCommitmentV1::ZERO;
                iroha_data_model::isi::offline_cash_v1::OFFLINE_CASH_MINT_FINALITY_TREE_DEPTH_V1
            ],
        };
        let top_up_root = offline_cash_mint_finality_root_v1(top_up_membership_witness.root);

        let mut validators = (1_u8..=4)
            .map(|index| {
                KeyPair::try_from_seed(
                    vec![nonce.wrapping_add(index).wrapping_add(80); 32],
                    Algorithm::BlsNormal,
                )
                .expect("BLS finality key")
            })
            .collect::<Vec<_>>();
        validators.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = validators
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let (mint_finality_epoch_id, mint_finality_roster) =
            crate::offline_cash_v1_test_fixtures::mint_finality_roster_and_id(
                request.network_id,
                0,
                &roster,
            );
        let eq_history = OfflineCashEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .expect("canonical Eq mint-authority history");
        let ep_history = OfflineCashEpAccumulatorV1::try_from_bytes(&proof.ep_history)
            .expect("canonical Ep mint-authority history");
        let finality_certificate_binding = proof.guard_eq_credential_audit;
        let finality_authority_head = proof.guard_ep_credential_audit;
        let finality_proof_binding_digest = OfflineCashMintAuthorityPairBindingV1 {
            step: OfflineCashMintAuthorityStepV1::FinalizedMint,
            semantic_digest: statement.canonical_digest().expect("mint statement digest"),
            amount: statement.amount,
            certificate_binding: finality_certificate_binding,
            authority_head: finality_authority_head,
            release_id: statement.lifecycle.release_id,
            genesis_roster_id: mint_finality_epoch_id,
            eq_protocol_digest: proof.eq_protocol_digest,
            ep_protocol_digest: proof.ep_protocol_digest,
            eq_deferred_audit: proof.eq_deferred_audit,
            ep_deferred_audit: proof.ep_deferred_audit,
            eq_history: eq_history.as_bytes(),
            ep_history: ep_history.as_bytes(),
        }
        .canonical_digest();
        let mint_credit = OfflineCashMintCreditV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            statement,
            proof,
            finality_certificate_binding,
            finality_authority_head,
            finality_genesis_roster_id: mint_finality_epoch_id,
            finality_proof_binding_digest,
            encrypted_credit: request.encrypted_credit.clone(),
            artifact_manifest_digest: request.artifact_manifest_digest,
        };
        let height = u64::from(nonce) + 100;
        let context = HeightContext {
            network_id: request.network_id,
            protocol_version: PROTOCOL_VERSION,
            height,
            epoch: 0,
            offline_cash_mint_finality_epoch_id: mint_finality_epoch_id,
            offline_cash_mint_finality_epoch_roster: mint_finality_roster,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("four-validator quorum"),
            roster,
            nexus_amx_context_hash: Hash::new([nonce, 1]),
            execution_policy_hash: Hash::new([nonce, 2]),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [nonce; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([nonce, 3])),
            payload_hash: Hash::new([nonce, 4]),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: 0,
        };
        let post_state_root = ExecutionCommitment::offline_cash_post_state_root_v1(
            1,
            ordinary_writes_root,
            top_up_root,
        );
        let execution_commitment = ExecutionCommitment::new_without_merge_carrier(
            Hash::new([nonce, 5]),
            post_state_root,
            ordinary_writes_root,
            Some(top_up_root),
            1,
            1,
            Hash::new([nonce, 7]),
        )
        .expect("top-up execution commitment");
        let preimage = Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = validators[..3]
            .iter()
            .map(|key| {
                IrohaSignature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mint_finality_message = OfflineCashMintFinalitySealMessageV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            finality_epoch_id: mint_finality_epoch_id,
            validator_count: 4,
            network_id: request.network_id,
            block_height: height,
            height_context_id: context.id(),
            subject_digest: [0x41; 32],
            execution_commitment_digest: [0x42; 32],
            offline_cash_top_up_root: top_up_root,
            offline_cash_top_up_count: 1,
            next_finality_epoch_id: None,
        };
        let mint_finality_bundle = OfflineCashMintFinalitySealBundleV1 {
            message: mint_finality_message,
            seals: (0..3)
                .map(|validator_index| OfflineCashMintFinalityValidatorSealV1 {
                    validator_index,
                    eq_proof_signature: OfflineCashPastaSchnorrSignatureV1 {
                        nonce_commitment: [0x51_u8.wrapping_add(validator_index as u8); 32],
                        response: [0x61_u8.wrapping_add(validator_index as u8); 32],
                    },
                    ep_proof_signature: OfflineCashPastaSchnorrSignatureV1 {
                        nonce_commitment: [0x71_u8.wrapping_add(validator_index as u8); 32],
                        response: [0x81_u8.wrapping_add(validator_index as u8); 32],
                    },
                })
                .collect(),
        };
        let aggregate_signature =
            bls_normal_aggregate_signatures(&share_refs).expect("aggregate CommitQC");
        let commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: encode_offline_cash_consensus_signature_envelope_v1(
                iroha_data_model::block::consensus_v2::OFFLINE_CASH_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1,
                &aggregate_signature,
                &mint_finality_bundle.encode(),
            )
            .expect("Offline Cash V1 CommitQC envelope"),
        };
        let validator_set_pops = validators
            .iter()
            .map(|key| bls_normal_pop_prove(key.private_key()).expect("validator PoP"))
            .collect();
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
        artifact.verify().expect("valid finality artifact");
        let anchor = OfflineCashFinalityTrustAnchorV1 {
            network_id: request.network_id,
            block_height: height,
            height_context_id: artifact.context_id(),
        };
        let result = OfflineCashTopUpResultV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            request,
            finality: OfflineCashOperationFinalityV1 {
                version: OFFLINE_CASH_CHAIN_VERSION_V1,
                finality_artifact: artifact,
                reserve_receipt_witness: witness,
                top_up_membership_witness: Some(top_up_membership_witness),
            },
            mint_credit,
        };
        result
            .validate_against(&anchor)
            .expect("valid terminal top-up result");
        (result, anchor)
    }

    fn verified_finalization(
        book: &OfflineCashReserveBookV1,
        verified: &VerifiedOfflineCashTopUpIntentV1,
        nonce: u8,
    ) -> VerifiedOfflineCashMintFinalizationV1 {
        let (result, anchor) = finality_fixture(book, verified, nonce);
        VerifiedOfflineCashMintFinalizationV1::after_full_verification(result, anchor)
            .expect("verified finalization")
    }

    #[test]
    fn independently_funded_credits_share_one_mixed_reserve() {
        let mut book = OfflineCashReserveBookV1::new();
        commit_top_up(&mut book, &verified_top_up(1, 1, 600), 1);
        commit_top_up(&mut book, &verified_top_up(2, 2, 400), 2);
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            1_000
        );

        commit_redemption(&mut book, &verified_redemption(3, 3, 750), 3);
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            250
        );
        commit_redemption(&mut book, &verified_redemption(4, 4, 250), 4);
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            0
        );
        assert_eq!(book.operation_count(), 4);
        book.validate().expect("valid mixed reserve");
    }

    #[test]
    fn restored_custody_must_cover_aggregate_reserve_liability() {
        let mut book = OfflineCashReserveBookV1::new();
        commit_top_up(&mut book, &verified_top_up(1, 1, 100), 1);
        let pool = book
            .pool(network(), asset(), asset_incarnation(1))
            .expect("pool lookup")
            .expect("funded pool");
        let reserve_account =
            crate::smartcontracts::isi::domain::isi::offline_cash_reserve_account_id(
                &pool.key.network_id,
                &pool.key.asset,
            );
        let reserve_asset_id = AssetId::new(pool.key.asset.clone(), reserve_account);
        let underfunded_quantity =
            Quantity::try_from_numeric(Numeric::new(99_u128, pool.scale)).expect("quantity");
        let (underfunded_id, underfunded_value) =
            Asset::new(reserve_asset_id.clone(), underfunded_quantity.clone()).into_key_value();
        assert_eq!(
            validate_persisted_reserve_custody_v1(
                book.pools.values(),
                [(&underfunded_id, &underfunded_value)],
            ),
            Err(OfflineCashReserveErrorV1::ReserveCustodyUnderfunded {
                liability: Quantity::try_from_numeric(Numeric::new(100_u128, pool.scale))
                    .expect("liability quantity"),
                custody: underfunded_quantity,
            })
        );

        let funded_quantity =
            Quantity::try_from_numeric(Numeric::new(100_u128, pool.scale)).expect("quantity");
        let (funded_id, funded_value) =
            Asset::new(reserve_asset_id, funded_quantity).into_key_value();
        validate_persisted_reserve_custody_v1(book.pools.values(), [(&funded_id, &funded_value)])
            .expect("exact custody covers the liability");

        commit_redemption(&mut book, &verified_redemption(2, 2, 100), 2);
        validate_persisted_reserve_custody_v1(
            book.pools.values(),
            core::iter::empty::<(&AssetId, &AssetValue)>(),
        )
        .expect("a fully redeemed pool needs no live custody balance");
    }

    #[test]
    fn one_thousand_independent_device_topups_are_one_redeemable_liability() {
        let mut book = OfflineCashReserveBookV1::new();
        let mut device_keys = std::collections::BTreeSet::new();

        for nonce in 1_u64..=1_000 {
            let top_up = verified_top_up(nonce, nonce, 1);
            assert!(
                device_keys.insert(
                    top_up
                        .intent
                        .request
                        .hardware_credential
                        .device_key_reference
                )
            );
            commit_top_up(&mut book, &top_up, nonce);
        }

        assert_eq!(device_keys.len(), 1_000);
        assert_eq!(book.operation_count(), 1_000);
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("mixed reserve"),
            1_000
        );
        let pool = book
            .pool(network(), asset(), asset_incarnation(1))
            .expect("pool lookup")
            .expect("funded pool");
        assert_eq!(pool.total_topups, 1_000);
        assert_eq!(pool.total_redemptions, 0);
        book.validate()
            .expect("one thousand independent receipts conserve value");

        // A single terminal voucher may redeem the complete aggregate even though its backing
        // arrived through 1,000 unrelated devices and operations.
        let mut full_redemption = book.clone();
        commit_redemption(
            &mut full_redemption,
            &verified_redemption(2_001, 2_001, 1_000),
            2_001,
        );
        assert_eq!(
            full_redemption
                .available(network(), asset(), asset_incarnation(1))
                .expect("fully redeemed reserve"),
            0
        );
        assert_eq!(full_redemption.operation_count(), 1_001);
        full_redemption
            .validate()
            .expect("single aggregate redemption conserves value");

        // Partial redemption is equally independent of the funding lineage. The remaining 600
        // stays pooled and can move peer-to-peer before a later terminal redemption.
        commit_redemption(&mut book, &verified_redemption(2_002, 2_002, 400), 2_002);
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("partially redeemed reserve"),
            600
        );
        commit_redemption(&mut book, &verified_redemption(2_003, 2_003, 600), 2_003);
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("fully redeemed reserve"),
            0
        );
        let pool = book
            .pool(network(), asset(), asset_incarnation(1))
            .expect("pool lookup")
            .expect("funded pool");
        assert_eq!(pool.total_topups, 1_000);
        assert_eq!(pool.total_redemptions, 1_000);
        assert_eq!(book.operation_count(), 1_002);
        book.validate()
            .expect("partial aggregate redemption path conserves value");
    }

    #[test]
    fn top_up_receipt_supplies_authoritative_mint_time_and_distinct_owners() {
        let mut book = OfflineCashReserveBookV1::new();
        let top_up = verified_top_up(1, 1, 50);
        commit_top_up(&mut book, &top_up, 9);
        let record = match book.operation(&top_up.operation_id()) {
            Some(OfflineCashReserveOperationRecordV1::TopUp(record)) => record,
            _ => panic!("top-up record"),
        };
        assert_eq!(record.reserve_receipt.committed_at_ms, 20_009);
        assert_eq!(record.payer, account(0xA5));
        assert_eq!(record.recipient, account(0xB6));
        assert_ne!(record.payer, record.recipient);
        assert_eq!(
            top_up
                .mint_statement(record.reserve_receipt.committed_at_ms)
                .expect("mint statement")
                .minted_at_ms,
            20_009
        );
    }

    #[test]
    fn finality_is_byte_idempotent_and_never_reapplies_totals() {
        let mut book = OfflineCashReserveBookV1::new();
        let top_up = verified_top_up(1, 1, 100);
        commit_top_up(&mut book, &top_up, 1);
        let pools = book.pools.clone();
        let reverse_indexes = (
            book.mint_credit_operations.clone(),
            book.issuance_operations.clone(),
        );
        let operation_count = book.operation_count();
        let finalization = verified_finalization(&book, &top_up, 1);
        let record = match book.operation(&top_up.operation_id()) {
            Some(OfflineCashReserveOperationRecordV1::TopUp(record)) => record,
            _ => panic!("top-up record"),
        };
        let attachment = match finalize_mint_credit_record(record, None, &finalization)
            .expect("derive local finalization")
        {
            OfflineCashMintFinalizationOutcomeV1::Finalized(attachment) => attachment,
            _ => panic!("new local attachment"),
        };
        assert_eq!(book.pools, pools);
        assert_eq!(book.operation_count(), operation_count);
        assert_eq!(
            (
                book.mint_credit_operations.clone(),
                book.issuance_operations.clone()
            ),
            reverse_indexes
        );
        assert!(matches!(
            finalize_mint_credit_record(record, Some(&attachment), &finalization)
                .expect("idempotent retry"),
            OfflineCashMintFinalizationOutcomeV1::AlreadyFinalized(_)
        ));
        assert_eq!(book.pools, pools);

        let different_valid_finality = verified_finalization(&book, &top_up, 2);
        assert_eq!(
            finalize_mint_credit_record(record, Some(&attachment), &different_valid_finality),
            Err(OfflineCashReserveErrorV1::MintFinalizationConflict {
                operation_id: top_up.operation_id(),
            })
        );
        assert_eq!(book.pools, pools);
        book.validate().expect("valid finalized accounting");
    }

    #[test]
    fn finalization_cannot_rebind_another_applied_intent() {
        let mut book = OfflineCashReserveBookV1::new();
        let first = verified_top_up(1, 1, 100);
        let second = verified_top_up(2, 2, 100);
        commit_top_up(&mut book, &first, 1);
        commit_top_up(&mut book, &second, 2);
        let second_finalization = verified_finalization(&book, &second, 3);
        let first_record = match book.operation(&first.operation_id()) {
            Some(OfflineCashReserveOperationRecordV1::TopUp(record)) => record,
            _ => panic!("first top-up record"),
        };
        assert_eq!(
            finalize_mint_credit_record(first_record, None, &second_finalization),
            Err(OfflineCashReserveErrorV1::MintFinalizationMismatch {
                operation_id: first.operation_id(),
            })
        );
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            200
        );
    }

    #[test]
    fn valid_finality_fails_under_different_network_or_height_context_anchor() {
        let mut book = OfflineCashReserveBookV1::new();
        let top_up = verified_top_up(1, 1, 100);
        commit_top_up(&mut book, &top_up, 1);
        let (result, anchor) = finality_fixture(&book, &top_up, 3);
        result
            .validate_against(&anchor)
            .expect("fixture is otherwise valid");

        let mut wrong_network = anchor;
        wrong_network.network_id = other_network();
        assert!(result.validate_against(&wrong_network).is_err());
        let mut wrong_height = anchor;
        wrong_height.block_height += 1;
        assert!(result.validate_against(&wrong_height).is_err());
        let mut wrong_context = anchor;
        wrong_context.height_context_id.0 =
            HashOf::from_untyped_unchecked(Hash::new(b"foreign height context"));
        assert!(result.validate_against(&wrong_context).is_err());
    }

    #[test]
    fn pending_top_up_roundtrips_without_authority_bearing_defaults() {
        let mut book = OfflineCashReserveBookV1::new();
        let top_up = verified_top_up(1, 1, 80);
        commit_top_up(&mut book, &top_up, 1);
        let bytes = norito::encode_canonical(&book).expect("encode pending book");
        let decoded: OfflineCashReserveBookV1 =
            norito::decode_canonical(&bytes).expect("decode pending book");
        decoded.validate().expect("valid decoded accounting");
        let record = match decoded.operation(&top_up.operation_id()) {
            Some(OfflineCashReserveOperationRecordV1::TopUp(record)) => record,
            _ => panic!("decoded top-up"),
        };
        assert_eq!(record.issuance_intent.request, top_up.intent.request);
        assert_eq!(
            record.issuance_intent.request_digest,
            top_up
                .intent
                .request
                .canonical_digest()
                .expect("request digest")
        );
        assert_eq!(
            decoded
                .available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            80
        );
    }

    #[test]
    fn local_finalization_cache_rehydrates_only_with_canonical_anchor_resolution() {
        let mut book = OfflineCashReserveBookV1::new();
        let top_up = verified_top_up(1, 1, 80);
        commit_top_up(&mut book, &top_up, 1);
        let finalization = verified_finalization(&book, &top_up, 4);
        let anchor = finalization.trusted_anchor_identity();
        let record = match book.operation(&top_up.operation_id()) {
            Some(OfflineCashReserveOperationRecordV1::TopUp(record)) => record,
            _ => panic!("top-up record"),
        };
        let attachment = match finalize_mint_credit_record(record, None, &finalization)
            .expect("derive finalization")
        {
            OfflineCashMintFinalizationOutcomeV1::Finalized(attachment) => attachment,
            _ => panic!("new local attachment"),
        };
        let bytes = norito::encode_canonical(&attachment).expect("encode local attachment");
        let decoded: OfflineCashMintFinalityAttachmentV1 =
            norito::decode_canonical(&bytes).expect("decode local attachment");
        assert_eq!(
            validate_mint_finality_attachment_with_anchor(
                record,
                &decoded,
                &|_: &OfflineCashFinalityTrustAnchorV1| None,
            ),
            Err(OfflineCashReserveErrorV1::FinalityAnchorUnavailable {
                operation_id: top_up.operation_id(),
            })
        );
        validate_mint_finality_attachment_with_anchor(
            record,
            &decoded,
            &move |identity: &OfflineCashFinalityTrustAnchorV1| {
                (*identity == anchor).then_some(anchor)
            },
        )
        .expect("canonical anchor re-authenticates result");
    }

    #[test]
    fn exact_top_up_retry_is_idempotent_but_rebinding_conflicts() {
        let mut book = OfflineCashReserveBookV1::new();
        let top_up = verified_top_up(1, 1, 50);
        let plan = expect_plan(book.plan_top_up(&top_up, commit_context(1)).expect("plan"));
        book.commit(plan.clone()).expect("commit");
        assert!(matches!(
            book.plan_top_up(&top_up, commit_context(99))
                .expect("idempotent plan"),
            OfflineCashReservePlanOutcomeV1::AlreadyCommitted(_)
        ));
        assert!(matches!(
            book.commit(plan).expect("idempotent commit"),
            OfflineCashReserveCommitOutcomeV1::AlreadyCommitted(_)
        ));
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            50
        );

        let changed_same_operation = verified_top_up(1, 2, 51);
        assert_eq!(
            book.plan_top_up(&changed_same_operation, commit_context(2)),
            Err(OfflineCashReserveErrorV1::OperationConflict {
                operation_id: top_up.operation_id(),
            })
        );
        let wrong_scale_request = top_up_request(2, 2, 1, 5);
        let wrong_scale_authorization = VerifiedOfflineCashTopUpAuthorizationV1 {
            request_digest: wrong_scale_request
                .canonical_digest()
                .expect("wrong-scale request digest"),
            mint_authorization_digest: wrong_scale_request
                .mint_authorization
                .as_ref()
                .expect("wrong-scale mint authorization")
                .canonical_digest()
                .expect("wrong-scale mint authorization digest"),
            profile: hardware_profile(),
        };
        let wrong_scale = VerifiedOfflineCashTopUpIntentV1::after_admission_verification(
            wrong_scale_request,
            wrong_scale_authorization,
        )
        .expect("wrong-scale request is internally valid");
        assert_eq!(
            book.plan_top_up(&wrong_scale, commit_context(3)),
            Err(OfflineCashReserveErrorV1::AssetScaleConflict {
                expected: 4,
                actual: 5,
            })
        );
        let cross_kind = after_mock_recursive_verification(OfflineCashRedemptionRequestV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            operation_id: top_up.operation_id(),
            voucher: redemption_voucher(9, 1),
        });
        assert_eq!(
            book.plan_redemption(&cross_kind, commit_context(4)),
            Err(OfflineCashReserveErrorV1::OperationConflict {
                operation_id: top_up.operation_id(),
            })
        );
    }

    #[test]
    fn exact_entry_planners_reject_index_collisions_without_history_scan() {
        let top_up = verified_top_up(1, 1, 50);
        let request = &top_up.intent.request;
        assert_eq!(
            plan_top_up_from_entries(
                &top_up,
                commit_context(1),
                OfflineCashTopUpReadSetV1 {
                    current_pool: None,
                    existing_operation: None,
                    credit_operation: Some(tagged_id(0xFF, 9)),
                    issuance_operation: None,
                },
            ),
            Err(OfflineCashReserveErrorV1::MintCreditConflict {
                credit_id: request.credit_id,
            })
        );
        assert_eq!(
            plan_top_up_from_entries(
                &top_up,
                commit_context(2),
                OfflineCashTopUpReadSetV1 {
                    current_pool: None,
                    existing_operation: None,
                    credit_operation: None,
                    issuance_operation: Some(tagged_id(0xFE, 9)),
                },
            ),
            Err(OfflineCashReserveErrorV1::IssuanceConflict {
                issuance_commitment: request.issuance_commitment,
            })
        );
        assert!(matches!(
            plan_top_up_from_entries(
                &top_up,
                commit_context(3),
                OfflineCashTopUpReadSetV1 {
                    current_pool: None,
                    existing_operation: None,
                    credit_operation: None,
                    issuance_operation: None,
                },
            )
            .expect("bounded plan"),
            OfflineCashReservePlanOutcomeV1::Commit(_)
        ));
    }

    #[test]
    fn terminal_nullifier_rejects_competing_partial_vouchers() {
        let mut book = OfflineCashReserveBookV1::new();
        commit_top_up(&mut book, &verified_top_up(1, 1, 100), 1);
        let first = verified_redemption(2, 9, 40);
        let mut competing_voucher = first.request().voucher.clone();
        competing_voucher.statement.amount = 60;
        competing_voucher.statement.redemption_id = [0; 32];
        competing_voucher.statement = competing_voucher
            .statement
            .seal_redemption_id()
            .expect("reseal competing voucher");
        competing_voucher.proof.semantic_digest = competing_voucher
            .statement
            .canonical_digest()
            .expect("competing digest");
        assert_eq!(
            competing_voucher.statement.terminal_nullifier,
            first.request().voucher.statement.terminal_nullifier
        );
        assert_ne!(
            competing_voucher.statement.redemption_id,
            first.request().voucher.statement.redemption_id
        );
        commit_redemption(&mut book, &first, 2);
        let competing = after_mock_recursive_verification(OfflineCashRedemptionRequestV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            operation_id: tagged_id(0x31, 3),
            voucher: competing_voucher,
        });
        assert_eq!(
            book.plan_redemption(&competing, commit_context(3)),
            Err(OfflineCashReserveErrorV1::TerminalNullifierConflict {
                terminal_nullifier: first.request().voucher.statement.terminal_nullifier,
            })
        );
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            60
        );
    }

    #[test]
    fn concurrent_plans_require_replanning_without_losing_value() {
        let mut book = OfflineCashReserveBookV1::new();
        let first = verified_top_up(1, 1, 10);
        let second = verified_top_up(2, 2, 20);
        let first_plan = expect_plan(
            book.plan_top_up(&first, commit_context(1))
                .expect("first plan"),
        );
        let stale_plan = expect_plan(
            book.plan_top_up(&second, commit_context(2))
                .expect("concurrent plan"),
        );
        book.commit(first_plan).expect("first commit");
        let before = book.clone();
        assert!(matches!(
            book.commit(stale_plan),
            Err(OfflineCashReserveErrorV1::StalePlan {
                expected: None,
                actual: Some(_),
            })
        ));
        assert_eq!(book, before);
        let replanned = expect_plan(
            book.plan_top_up(&second, commit_context(3))
                .expect("replan"),
        );
        book.commit(replanned).expect("second commit");
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            30
        );
    }

    #[test]
    fn underflow_and_u128_overflow_never_mutate_state() {
        let mut book = OfflineCashReserveBookV1::new();
        commit_top_up(&mut book, &verified_top_up(1, 1, 10), 1);
        let before_underflow = book.clone();
        assert_eq!(
            book.plan_redemption(&verified_redemption(2, 2, 11), commit_context(2)),
            Err(OfflineCashReserveErrorV1::ReserveUnderflow {
                available: 10,
                requested: 11,
            })
        );
        assert_eq!(book, before_underflow);

        let mut maximum = OfflineCashReserveBookV1::new();
        commit_top_up(&mut maximum, &verified_top_up(3, 3, u128::MAX), 3);
        let before_overflow = maximum.clone();
        assert_eq!(
            maximum.plan_top_up(&verified_top_up(4, 4, 1), commit_context(4)),
            Err(OfflineCashReserveErrorV1::TotalTopUpsOverflow)
        );
        assert_eq!(maximum, before_overflow);
    }

    #[test]
    fn historical_receipt_tampering_and_missing_indexes_fail_hydration() {
        let mut book = OfflineCashReserveBookV1::new();
        let top_up = verified_top_up(1, 1, 10);
        commit_top_up(&mut book, &top_up, 1);
        let mut wrong_receipt = book.clone();
        let record = match wrong_receipt.operations.get_mut(&top_up.operation_id()) {
            Some(OfflineCashReserveOperationRecordV1::TopUp(record)) => record,
            _ => panic!("top-up record"),
        };
        record.reserve_receipt.total_topups += 1;
        assert!(wrong_receipt.validate().is_err());

        let mut missing_index = book;
        missing_index
            .mint_credit_operations
            .remove(&top_up.intent.request.credit_id);
        assert_eq!(
            missing_index.validate(),
            Err(OfflineCashReserveErrorV1::StateInvariant {
                reason: "top_up_reverse_index_mismatch",
            })
        );
    }

    #[test]
    fn deterministic_property_style_run_preserves_every_receipt_and_total() {
        let mut book = OfflineCashReserveBookV1::new();
        let mut expected = 0_u128;
        for nonce in 1_u64..=96 {
            let amount = u128::from((nonce * 37) % 101 + 1);
            commit_top_up(&mut book, &verified_top_up(nonce, nonce, amount), nonce);
            expected = expected.checked_add(amount).expect("test top-up sum");
        }
        for nonce in 1_u64..=64 {
            let amount = u128::from((nonce * 19) % 47 + 1);
            commit_redemption(
                &mut book,
                &verified_redemption(1_000 + nonce, 1_000 + nonce, amount),
                1_000 + nonce,
            );
            expected = expected
                .checked_sub(amount)
                .expect("funded test redemption");
        }
        assert_eq!(
            book.available(network(), asset(), asset_incarnation(1))
                .expect("reserve"),
            expected
        );
        assert_eq!(book.operation_count(), 160);
        book.validate().expect("all historical receipts agree");
    }

    #[test]
    fn plan_accessors_expose_only_constant_count_entry_writes() {
        let top_up = verified_top_up(1, 1, 10);
        let plan = match plan_top_up_from_entries(
            &top_up,
            commit_context(1),
            OfflineCashTopUpReadSetV1 {
                current_pool: None,
                existing_operation: None,
                credit_operation: None,
                issuance_operation: None,
            },
        )
        .expect("plan")
        {
            OfflineCashReservePlanOutcomeV1::Commit(OfflineCashReserveMutationPlanV1::TopUp(
                plan,
            )) => plan,
            _ => panic!("new top-up plan"),
        };
        assert_eq!(plan.expected_pool_head(), None);
        assert!(plan.next_pool().latest_receipt.is_some());
        assert_eq!(plan.record().operation_id, top_up.operation_id());
        assert_eq!(
            plan.record().reserve_receipt.total_topups,
            plan.next_pool().total_topups
        );
    }
}
