//! Closed request and authenticated-state projection types for Exact12 actions.
//!
//! These models describe already-signed transaction bytes and authenticated
//! operation status. They do not prove a transaction locally, submit it, or
//! grant network authority.

use super::{
    PrivacyExact12CapabilityManifestDigestV1, PrivacyLedgerEffectKindV1, PrivacyOperationSchemaV1,
    PrivacyProtocolIdV1, PrivacyStatementDigestV1, PrivacyTransactionIntentDigestV1,
};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{NetworkId, block::BlockHeader};
use iroha_crypto::{Hash, HashOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Permanent Norito schema identity for one Exact12 signed-action request.
pub const PRIVACY_EXACT12_ACTION_REQUEST_SCHEMA_NAME_V1: &str =
    "iroha.privacy.exact12-action-request.v1";
/// Permanent Norito schema identity for one Exact12 operation-state view.
pub const PRIVACY_ACTION_OPERATION_VIEW_SCHEMA_NAME_V1: &str =
    "iroha.privacy.action-operation-view.v1";
/// Permanent Norito schema identity for one finalized native execution receipt.
pub const PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_SCHEMA_NAME_V1: &str =
    "iroha.privacy.action-execution-receipt-view.v1";
/// Exact wire version of a finalized native Exact12 execution receipt.
pub const PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1: u16 = 1;
/// Maximum versioned signed-transaction bytes accepted by the V1 request model.
pub const PRIVACY_EXACT12_MAX_SIGNED_TRANSACTION_BYTES_V1: usize = 10 * 1024 * 1024;
/// Maximum UTF-8 bytes in one committed rejection reason.
pub const PRIVACY_ACTION_REJECTION_REASON_MAX_BYTES_V1: usize = 1_024;

fn is_zero_iroha_hash<T>(hash: &HashOf<T>) -> bool {
    hash.as_ref() == Hash::prehashed([0; Hash::LENGTH]).as_ref()
}

/// Public action spelling for the closed thirteen-operation Exact12 schema.
pub type PrivacyExact12ActionOperationV1 = PrivacyOperationSchemaV1;

/// Local lifecycle projection for one Exact12 action submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "local_state", content = "value", deny_unknown_fields)
)]
pub enum PrivacyActionLocalStateV1 {
    /// The signed transaction was submitted but no authenticated terminal result exists yet.
    #[cfg_attr(feature = "json", norito(rename = "submitted"))]
    Submitted,
    /// An authenticated terminal chain result is available.
    #[cfg_attr(feature = "json", norito(rename = "terminal"))]
    Terminal,
}

impl PrivacyActionLocalStateV1 {
    /// Return the sole public string spelling of this local state.
    #[must_use]
    pub const fn canonical_label(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Terminal => "terminal",
        }
    }
}

/// Authenticated terminal pipeline state for one Exact12 action submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "terminal_chain_state", content = "value", deny_unknown_fields)
)]
pub enum PrivacyActionTerminalChainStateV1 {
    /// The transaction is present in a committed block.
    #[cfg_attr(feature = "json", norito(rename = "Committed"))]
    Committed,
    /// The transaction committed and its instructions applied successfully.
    #[cfg_attr(feature = "json", norito(rename = "Applied"))]
    Applied,
    /// The transaction committed with an authenticated rejection reason.
    #[cfg_attr(feature = "json", norito(rename = "Rejected"))]
    Rejected,
    /// The locally tracked transaction expired without a committed carrier.
    #[cfg_attr(feature = "json", norito(rename = "Expired"))]
    Expired,
}

impl PrivacyActionTerminalChainStateV1 {
    /// Return the sole public string spelling of this terminal chain state.
    #[must_use]
    pub const fn canonical_label(self) -> &'static str {
        match self {
            Self::Committed => "Committed",
            Self::Applied => "Applied",
            Self::Rejected => "Rejected",
            Self::Expired => "Expired",
        }
    }
}

/// Fail-closed validation error for Exact12 request, status, and receipt models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyExact12ActionModelErrorV1 {
    /// Signed transaction bytes are empty or exceed the fixed V1 ceiling.
    #[error("Exact12 signed transaction length {actual} is outside 1..={maximum}")]
    InvalidSignedTransactionLength {
        /// Observed byte length.
        actual: usize,
        /// Exact V1 maximum.
        maximum: usize,
    },
    /// An optional expected capability-manifest digest used the zero sentinel.
    #[error("Exact12 expected capability-manifest digest is zero")]
    ZeroExpectedManifestDigest,
    /// The operation does not belong to the supplied protocol.
    #[error("Exact12 operation does not belong to the supplied protocol")]
    OperationProtocolMismatch,
    /// The operation does not produce the supplied typed ledger effect.
    #[error("Exact12 operation does not produce the supplied ledger-effect kind")]
    OperationLedgerEffectMismatch,
    /// The signed transaction hash used the zero sentinel.
    #[error("Exact12 transaction hash is zero")]
    ZeroTransactionHash,
    /// The transaction-intent digest used the zero sentinel.
    #[error("Exact12 transaction-intent digest is zero")]
    ZeroTransactionIntentDigest,
    /// The statement digest used the zero sentinel.
    #[error("Exact12 statement digest is zero")]
    ZeroStatementDigest,
    /// The proof-envelope hash used the zero sentinel.
    #[error("Exact12 proof-envelope hash is zero")]
    ZeroProofEnvelopeHash,
    /// The admitted capability-manifest digest used the zero sentinel.
    #[error("Exact12 capability-manifest digest is zero")]
    ZeroCapabilityManifestDigest,
    /// The authenticated capability snapshot height was zero.
    #[error("Exact12 capability committed height is zero")]
    ZeroCapabilityCommittedHeight,
    /// A present execution-time capability-manifest digest used the zero sentinel.
    #[error("Exact12 execution capability-manifest digest is zero")]
    ZeroExecutionCapabilityManifestDigest,
    /// A present execution-time capability snapshot height used the zero sentinel.
    #[error("Exact12 execution capability committed height is zero")]
    ZeroExecutionCapabilityCommittedHeight,
    /// A present execution-receipt finality height used the zero sentinel.
    #[error("Exact12 execution receipt finalized height is zero")]
    ZeroExecutionReceiptFinalizedHeight,
    /// A present committed height used the zero sentinel.
    #[error("Exact12 committed height is zero")]
    ZeroCommittedHeight,
    /// A terminal transaction predates the finalized capability snapshot used before submission.
    #[error("Exact12 terminal committed height precedes pre-submit capability finality")]
    TerminalHeightBeforeCapabilitySnapshot,
    /// A rejected operation omitted its canonical bounded reason.
    #[error("rejected Exact12 action has no canonical bounded rejection reason")]
    InvalidRejectionReason,
    /// Local and terminal fields form an impossible state combination.
    #[error("Exact12 local and terminal state fields form an impossible combination")]
    InvalidStateCombination,
    /// A finalized execution receipt used an unsupported version.
    #[error("Exact12 execution receipt version {actual} is unsupported")]
    UnsupportedExecutionReceiptVersion {
        /// Observed receipt version.
        actual: u16,
    },
    /// A finalized execution receipt used the zero `NetworkId` sentinel.
    #[error("Exact12 execution receipt NetworkId is zero")]
    ZeroExecutionReceiptNetworkId,
    /// A finalized execution receipt used an action index outside the transaction bound.
    #[error("Exact12 execution receipt action index {actual} is outside 0..{maximum_exclusive}")]
    ExecutionReceiptActionIndexOutOfRange {
        /// Observed zero-based action index.
        actual: u32,
        /// Exclusive V1 maximum.
        maximum_exclusive: u32,
    },
    /// Receipt capability/admission/finality heights do not form a valid order.
    #[error("Exact12 execution receipt heights are zero or out of order")]
    InvalidExecutionReceiptHeights,
    /// A finalized receipt omitted its exact block-hash binding.
    #[error("Exact12 execution receipt finalized block hash is zero")]
    ZeroExecutionReceiptFinalizedBlockHash,
}

/// One closed Exact12 operation and its already-signed versioned transaction wire.
///
/// Validation snapshots and bounds public wire bytes. It performs no local
/// proof acceptance and grants no capability or submission authority.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.exact12-action-request.v1")]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyExact12ActionRequestV1 {
    operation: PrivacyExact12ActionOperationV1,
    signed_transaction_versioned: Vec<u8>,
    /// Optional pre-submit observation check against the fresh manifest.
    ///
    /// This detached value is not a signed consensus execution precondition.
    /// The manifest actually admitted by native execution is recorded only in
    /// the finalized execution receipt.
    expected_manifest_digest: Option<PrivacyExact12CapabilityManifestDigestV1>,
}

impl PrivacyExact12ActionRequestV1 {
    /// Construct one validated signed-action request.
    ///
    /// # Errors
    ///
    /// Rejects empty or over-limit transaction bytes and a zero optional
    /// capability-manifest digest.
    pub fn try_new(
        operation: PrivacyExact12ActionOperationV1,
        signed_transaction_versioned: Vec<u8>,
        expected_manifest_digest: Option<PrivacyExact12CapabilityManifestDigestV1>,
    ) -> Result<Self, PrivacyExact12ActionModelErrorV1> {
        let request = Self {
            operation,
            signed_transaction_versioned,
            expected_manifest_digest,
        };
        request.validate()?;
        Ok(request)
    }

    /// Validate the complete request after decoding or construction.
    ///
    /// # Errors
    ///
    /// Rejects an invalid transaction length or zero optional manifest digest.
    pub fn validate(&self) -> Result<(), PrivacyExact12ActionModelErrorV1> {
        let actual = self.signed_transaction_versioned.len();
        if !(1..=PRIVACY_EXACT12_MAX_SIGNED_TRANSACTION_BYTES_V1).contains(&actual) {
            return Err(
                PrivacyExact12ActionModelErrorV1::InvalidSignedTransactionLength {
                    actual,
                    maximum: PRIVACY_EXACT12_MAX_SIGNED_TRANSACTION_BYTES_V1,
                },
            );
        }
        if self
            .expected_manifest_digest
            .is_some_and(|digest| digest.is_zero())
        {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroExpectedManifestDigest);
        }
        Ok(())
    }

    /// Return the exact closed operation selected by this request.
    #[must_use]
    pub const fn operation(&self) -> PrivacyExact12ActionOperationV1 {
        self.operation
    }

    /// Borrow the already-signed, versioned transaction wire.
    #[must_use]
    pub fn signed_transaction_versioned(&self) -> &[u8] {
        &self.signed_transaction_versioned
    }

    /// Return the optional pre-submit capability-manifest observation check.
    ///
    /// This does not pin the manifest later admitted by consensus execution.
    #[must_use]
    pub const fn expected_manifest_digest(
        &self,
    ) -> Option<PrivacyExact12CapabilityManifestDigestV1> {
        self.expected_manifest_digest
    }
}

/// Finalized consensus receipt for one successfully executed Exact12 action.
///
/// Core persists the receipt in the same state transaction as the native
/// ledger effect. A successful transaction without this receipt is therefore
/// not an applied Exact12 action, including for verification-only protocols.
/// The finality fields are added by the typed authenticated query from the
/// exact state snapshot that served it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.action-execution-receipt-view.v1")]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyActionExecutionReceiptViewV1 {
    /// Exact receipt wire version.
    pub version: u16,
    /// Genesis-derived identity of the network that executed the action.
    pub network_id: NetworkId,
    /// Closed protocol selected by the verified envelope.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact public operation executed inside that protocol.
    pub operation_schema: PrivacyExact12ActionOperationV1,
    /// Typed ledger-effect class applied by native execution.
    pub ledger_effect_kind: PrivacyLedgerEffectKindV1,
    /// Hash of the exact signed transaction containing the action.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transaction_hash: [u8; 32],
    /// Zero-based privacy-action position in the signed transaction.
    pub action_index: u32,
    /// Transaction intent authenticated by the verified public statement.
    pub transaction_intent_digest: PrivacyTransactionIntentDigestV1,
    /// Digest of the exact verified public statement.
    pub statement_digest: PrivacyStatementDigestV1,
    /// Iroha hash of the exact canonical proof envelope executed by Core.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub proof_envelope_hash: [u8; 32],
    /// Exact capability manifest observed by consensus execution.
    pub capability_manifest_digest: PrivacyExact12CapabilityManifestDigestV1,
    /// Height of the committed state used to derive that manifest.
    pub capability_committed_height: u64,
    /// Height at which the receipt and its ledger effect became durable.
    pub admitted_at_height: u64,
    /// Height of the finalized state snapshot that served this receipt.
    pub finalized_height: u64,
    /// Exact finalized block anchoring the served state snapshot.
    pub finalized_block_hash: HashOf<BlockHeader>,
}

impl PrivacyActionExecutionReceiptViewV1 {
    /// Validate every protocol, operation, digest, position, and finality binding.
    ///
    /// # Errors
    ///
    /// Rejects version or mapping drift, zero identities/digests, an impossible
    /// action index, unordered heights, or an absent finalized-block binding.
    pub fn validate(&self) -> Result<(), PrivacyExact12ActionModelErrorV1> {
        if self.version != PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1 {
            return Err(
                PrivacyExact12ActionModelErrorV1::UnsupportedExecutionReceiptVersion {
                    actual: self.version,
                },
            );
        }
        if self.network_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionReceiptNetworkId);
        }
        if self.protocol_id != self.operation_schema.protocol_id() {
            return Err(PrivacyExact12ActionModelErrorV1::OperationProtocolMismatch);
        }
        if self.ledger_effect_kind != self.operation_schema.ledger_effect_kind() {
            return Err(PrivacyExact12ActionModelErrorV1::OperationLedgerEffectMismatch);
        }
        if is_zero_32(&self.transaction_hash) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroTransactionHash);
        }
        if self.action_index >= super::TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1 {
            return Err(
                PrivacyExact12ActionModelErrorV1::ExecutionReceiptActionIndexOutOfRange {
                    actual: self.action_index,
                    maximum_exclusive: super::TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1,
                },
            );
        }
        if self.transaction_intent_digest.is_zero() {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroTransactionIntentDigest);
        }
        if self.statement_digest.is_zero() {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroStatementDigest);
        }
        if is_zero_32(&self.proof_envelope_hash) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroProofEnvelopeHash);
        }
        if self.capability_manifest_digest.is_zero() {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroCapabilityManifestDigest);
        }
        if self.capability_committed_height == 0
            || self.admitted_at_height < self.capability_committed_height
            || self.finalized_height < self.admitted_at_height
        {
            return Err(PrivacyExact12ActionModelErrorV1::InvalidExecutionReceiptHeights);
        }
        if is_zero_iroha_hash(&self.finalized_block_hash) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionReceiptFinalizedBlockHash);
        }
        Ok(())
    }
}

/// Immutable public state of one authenticated Exact12 action submission.
///
/// A controller must populate this view only from native signed-action
/// inspection, fresh capability admission, authenticated transaction queries,
/// and finalized typed state queries. The type itself does not authenticate a
/// detached caller-created value.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.action-operation-view.v1")]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyActionOperationViewV1 {
    protocol_id: PrivacyProtocolIdV1,
    operation_schema: PrivacyExact12ActionOperationV1,
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    transaction_hash: [u8; 32],
    transaction_intent_digest: PrivacyTransactionIntentDigestV1,
    statement_digest: PrivacyStatementDigestV1,
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    proof_envelope_hash: [u8; 32],
    local_state: PrivacyActionLocalStateV1,
    terminal_chain_state: Option<PrivacyActionTerminalChainStateV1>,
    committed_height: Option<u64>,
    rejection_reason: Option<String>,
    ledger_effect_kind: PrivacyLedgerEffectKindV1,
    /// Fresh finalized capability digest used only for pre-submit admission.
    capability_manifest_digest: PrivacyExact12CapabilityManifestDigestV1,
    /// Fresh finalized capability height used only for pre-submit admission.
    capability_committed_height: u64,
    /// Capability manifest actually admitted by native execution, when applied.
    execution_capability_manifest_digest: Option<PrivacyExact12CapabilityManifestDigestV1>,
    /// Height of the capability snapshot actually admitted by native execution.
    execution_capability_committed_height: Option<u64>,
    /// Finalized state height from the authenticated native execution receipt.
    execution_receipt_finalized_height: Option<u64>,
    /// Exact finalized block from the authenticated native execution receipt.
    execution_receipt_finalized_block_hash: Option<HashOf<BlockHeader>>,
}

impl PrivacyActionOperationViewV1 {
    /// Construct one validated operation-state projection.
    ///
    /// # Errors
    ///
    /// Rejects mapping drift, zero hashes/heights, a non-canonical rejection
    /// reason, or an impossible local/terminal state combination.
    #[expect(
        clippy::too_many_arguments,
        reason = "the public operation view deliberately keeps every authenticated binding explicit"
    )]
    pub fn try_new(
        protocol_id: PrivacyProtocolIdV1,
        operation_schema: PrivacyExact12ActionOperationV1,
        transaction_hash: [u8; 32],
        transaction_intent_digest: PrivacyTransactionIntentDigestV1,
        statement_digest: PrivacyStatementDigestV1,
        proof_envelope_hash: [u8; 32],
        local_state: PrivacyActionLocalStateV1,
        terminal_chain_state: Option<PrivacyActionTerminalChainStateV1>,
        committed_height: Option<u64>,
        rejection_reason: Option<String>,
        ledger_effect_kind: PrivacyLedgerEffectKindV1,
        capability_manifest_digest: PrivacyExact12CapabilityManifestDigestV1,
        capability_committed_height: u64,
        execution_capability_manifest_digest: Option<PrivacyExact12CapabilityManifestDigestV1>,
        execution_capability_committed_height: Option<u64>,
        execution_receipt_finalized_height: Option<u64>,
        execution_receipt_finalized_block_hash: Option<HashOf<BlockHeader>>,
    ) -> Result<Self, PrivacyExact12ActionModelErrorV1> {
        let view = Self {
            protocol_id,
            operation_schema,
            transaction_hash,
            transaction_intent_digest,
            statement_digest,
            proof_envelope_hash,
            local_state,
            terminal_chain_state,
            committed_height,
            rejection_reason,
            ledger_effect_kind,
            capability_manifest_digest,
            capability_committed_height,
            execution_capability_manifest_digest,
            execution_capability_committed_height,
            execution_receipt_finalized_height,
            execution_receipt_finalized_block_hash,
        };
        view.validate()?;
        Ok(view)
    }

    /// Validate the complete view after decoding or construction.
    ///
    /// # Errors
    ///
    /// Rejects any mapping, digest, height, reason, or state invariant drift.
    pub fn validate(&self) -> Result<(), PrivacyExact12ActionModelErrorV1> {
        self.validate_bound_fields()?;
        self.validate_state_combination()?;
        if self.local_state == PrivacyActionLocalStateV1::Terminal
            && self
                .committed_height
                .is_some_and(|height| height < self.capability_committed_height)
        {
            return Err(PrivacyExact12ActionModelErrorV1::TerminalHeightBeforeCapabilitySnapshot);
        }
        Ok(())
    }

    fn validate_bound_fields(&self) -> Result<(), PrivacyExact12ActionModelErrorV1> {
        if self.protocol_id != self.operation_schema.protocol_id() {
            return Err(PrivacyExact12ActionModelErrorV1::OperationProtocolMismatch);
        }
        if self.ledger_effect_kind != self.operation_schema.ledger_effect_kind() {
            return Err(PrivacyExact12ActionModelErrorV1::OperationLedgerEffectMismatch);
        }
        if is_zero_32(&self.transaction_hash) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroTransactionHash);
        }
        if self.transaction_intent_digest.is_zero() {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroTransactionIntentDigest);
        }
        if self.statement_digest.is_zero() {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroStatementDigest);
        }
        if is_zero_32(&self.proof_envelope_hash) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroProofEnvelopeHash);
        }
        if self.capability_manifest_digest.is_zero() {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroCapabilityManifestDigest);
        }
        if self.capability_committed_height == 0 {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroCapabilityCommittedHeight);
        }
        if self
            .execution_capability_manifest_digest
            .is_some_and(|digest| digest.is_zero())
        {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionCapabilityManifestDigest);
        }
        if self.execution_capability_committed_height == Some(0) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionCapabilityCommittedHeight);
        }
        if self.execution_receipt_finalized_height == Some(0) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionReceiptFinalizedHeight);
        }
        if self
            .execution_receipt_finalized_block_hash
            .as_ref()
            .is_some_and(is_zero_iroha_hash)
        {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionReceiptFinalizedBlockHash);
        }
        if self.committed_height == Some(0) {
            return Err(PrivacyExact12ActionModelErrorV1::ZeroCommittedHeight);
        }
        Ok(())
    }

    fn execution_evidence_shape(&self) -> (bool, bool) {
        let evidence = (
            self.execution_capability_manifest_digest.is_some(),
            self.execution_capability_committed_height.is_some(),
            self.execution_receipt_finalized_height.is_some(),
            self.execution_receipt_finalized_block_hash.is_some(),
        );
        (
            evidence.0 || evidence.1 || evidence.2 || evidence.3,
            evidence.0 && evidence.1 && evidence.2 && evidence.3,
        )
    }

    fn validate_state_combination(&self) -> Result<(), PrivacyExact12ActionModelErrorV1> {
        let (has_any_execution_evidence, has_complete_execution_evidence) =
            self.execution_evidence_shape();
        match (self.local_state, self.terminal_chain_state) {
            (PrivacyActionLocalStateV1::Submitted, None)
            | (
                PrivacyActionLocalStateV1::Terminal,
                Some(PrivacyActionTerminalChainStateV1::Expired),
            ) => {
                if self.committed_height.is_some()
                    || self.rejection_reason.is_some()
                    || has_any_execution_evidence
                {
                    return Err(PrivacyExact12ActionModelErrorV1::InvalidStateCombination);
                }
            }
            (
                PrivacyActionLocalStateV1::Terminal,
                Some(PrivacyActionTerminalChainStateV1::Committed),
            ) => {
                if self.committed_height.is_none()
                    || self.rejection_reason.is_some()
                    || has_any_execution_evidence
                {
                    return Err(PrivacyExact12ActionModelErrorV1::InvalidStateCombination);
                }
            }
            (
                PrivacyActionLocalStateV1::Terminal,
                Some(PrivacyActionTerminalChainStateV1::Applied),
            ) => {
                let Some(committed_height) = self.committed_height else {
                    return Err(PrivacyExact12ActionModelErrorV1::InvalidStateCombination);
                };
                if self.rejection_reason.is_some() || !has_complete_execution_evidence {
                    return Err(PrivacyExact12ActionModelErrorV1::InvalidStateCombination);
                }
                let capability_height = self
                    .execution_capability_committed_height
                    .expect("complete execution evidence checked above");
                let receipt_height = self
                    .execution_receipt_finalized_height
                    .expect("complete execution evidence checked above");
                if capability_height > committed_height || receipt_height < committed_height {
                    return Err(PrivacyExact12ActionModelErrorV1::InvalidExecutionReceiptHeights);
                }
            }
            (
                PrivacyActionLocalStateV1::Terminal,
                Some(PrivacyActionTerminalChainStateV1::Rejected),
            ) => {
                if self.committed_height.is_none() || has_any_execution_evidence {
                    return Err(PrivacyExact12ActionModelErrorV1::InvalidStateCombination);
                }
                if !self
                    .rejection_reason
                    .as_deref()
                    .is_some_and(canonical_rejection_reason_v1)
                {
                    return Err(PrivacyExact12ActionModelErrorV1::InvalidRejectionReason);
                }
            }
            (PrivacyActionLocalStateV1::Submitted, Some(_))
            | (PrivacyActionLocalStateV1::Terminal, None) => {
                return Err(PrivacyExact12ActionModelErrorV1::InvalidStateCombination);
            }
        }
        Ok(())
    }

    /// Return the protocol selected by this operation.
    #[must_use]
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        self.protocol_id
    }

    /// Return the closed public operation schema.
    #[must_use]
    pub const fn operation_schema(&self) -> PrivacyExact12ActionOperationV1 {
        self.operation_schema
    }

    /// Return the exact signed transaction hash.
    #[must_use]
    pub const fn transaction_hash(&self) -> [u8; 32] {
        self.transaction_hash
    }

    /// Return the transaction-intent digest authenticated by native inspection.
    #[must_use]
    pub const fn transaction_intent_digest(&self) -> PrivacyTransactionIntentDigestV1 {
        self.transaction_intent_digest
    }

    /// Return the typed statement digest authenticated by native inspection.
    #[must_use]
    pub const fn statement_digest(&self) -> PrivacyStatementDigestV1 {
        self.statement_digest
    }

    /// Return the exact canonical proof-envelope hash.
    #[must_use]
    pub const fn proof_envelope_hash(&self) -> [u8; 32] {
        self.proof_envelope_hash
    }

    /// Return the local lifecycle projection.
    #[must_use]
    pub const fn local_state(&self) -> PrivacyActionLocalStateV1 {
        self.local_state
    }

    /// Return the authenticated terminal chain state, when present.
    #[must_use]
    pub const fn terminal_chain_state(&self) -> Option<PrivacyActionTerminalChainStateV1> {
        self.terminal_chain_state
    }

    /// Return the committed transaction height, when the terminal state has one.
    #[must_use]
    pub const fn committed_height(&self) -> Option<u64> {
        self.committed_height
    }

    /// Borrow the authenticated rejection reason, when rejected.
    #[must_use]
    pub fn rejection_reason(&self) -> Option<&str> {
        self.rejection_reason.as_deref()
    }

    /// Return the typed native ledger-effect kind.
    #[must_use]
    pub const fn ledger_effect_kind(&self) -> PrivacyLedgerEffectKindV1 {
        self.ledger_effect_kind
    }

    /// Return the fresh admitted capability-manifest digest.
    #[must_use]
    pub const fn capability_manifest_digest(&self) -> PrivacyExact12CapabilityManifestDigestV1 {
        self.capability_manifest_digest
    }

    /// Return the committed height bound by the admitted capability manifest.
    #[must_use]
    pub const fn capability_committed_height(&self) -> u64 {
        self.capability_committed_height
    }

    /// Return the capability manifest actually admitted by native execution.
    #[must_use]
    pub const fn execution_capability_manifest_digest(
        &self,
    ) -> Option<PrivacyExact12CapabilityManifestDigestV1> {
        self.execution_capability_manifest_digest
    }

    /// Return the execution-time capability snapshot height, when applied.
    #[must_use]
    pub const fn execution_capability_committed_height(&self) -> Option<u64> {
        self.execution_capability_committed_height
    }

    /// Return the finalized height of the authenticated execution receipt.
    #[must_use]
    pub const fn execution_receipt_finalized_height(&self) -> Option<u64> {
        self.execution_receipt_finalized_height
    }

    /// Return the exact finalized block of the authenticated execution receipt.
    #[must_use]
    pub fn execution_receipt_finalized_block_hash(&self) -> Option<&HashOf<BlockHeader>> {
        self.execution_receipt_finalized_block_hash.as_ref()
    }
}

fn is_zero_32(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn canonical_rejection_reason_v1(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= PRIVACY_ACTION_REJECTION_REASON_MAX_BYTES_V1
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

// SECURITY: These are detached wire/state models, not authority tokens. Only a
// controller that performs native signed-action inspection, fresh manifest
// admission, authenticated transaction queries, and finalized typed state
// queries may describe a view as authoritative. Construction-only drivers must
// never mint that claim.

#[cfg(test)]
mod tests {
    use super::*;

    fn execution_receipt() -> PrivacyActionExecutionReceiptViewV1 {
        let operation = PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1;
        let genesis_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x71; 32]),
        );
        PrivacyActionExecutionReceiptViewV1 {
            version: PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1,
            network_id: NetworkId::from_genesis_hash(genesis_hash),
            protocol_id: operation.protocol_id(),
            operation_schema: operation,
            ledger_effect_kind: operation.ledger_effect_kind(),
            transaction_hash: [0x11; 32],
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x22; 32]),
            statement_digest: PrivacyStatementDigestV1::new([0x33; 32]),
            proof_envelope_hash: [0x44; 32],
            capability_manifest_digest: PrivacyExact12CapabilityManifestDigestV1::new([0x55; 32]),
            capability_committed_height: 40,
            admitted_at_height: 41,
            finalized_height: 42,
            finalized_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0x72; 32]),
            ),
        }
    }

    fn submitted_view() -> PrivacyActionOperationViewV1 {
        let operation = PrivacyOperationSchemaV1::OrchardNoteActionV1;
        PrivacyActionOperationViewV1::try_new(
            operation.protocol_id(),
            operation,
            [0x11; 32],
            PrivacyTransactionIntentDigestV1::new([0x22; 32]),
            PrivacyStatementDigestV1::new([0x33; 32]),
            [0x44; 32],
            PrivacyActionLocalStateV1::Submitted,
            None,
            None,
            None,
            operation.ledger_effect_kind(),
            PrivacyExact12CapabilityManifestDigestV1::new([0x55; 32]),
            41,
            None,
            None,
            None,
            None,
        )
        .expect("valid submitted operation view")
    }

    fn assert_stable_schema_wire<T>(value: &T, schema_name: &str, expected: [u8; 16])
    where
        T: norito::NoritoSerialize
            + for<'de> norito::NoritoDeserialize<'de>
            + PartialEq
            + core::fmt::Debug
            + 'static,
    {
        assert_eq!(norito::core::schema_hash_for_name(schema_name), expected);
        assert_eq!(<T as norito::NoritoSerialize>::schema_hash(), expected);
        assert_eq!(
            <T as norito::NoritoDeserialize<'static>>::schema_hash(),
            expected
        );
        let canonical = norito::encode_canonical(value).expect("canonical Exact12 model wire");
        assert_eq!(&canonical[6..22], expected.as_slice());
        assert_eq!(
            norito::decode_canonical::<T>(&canonical).expect("decode canonical Exact12 model"),
            *value
        );
    }

    #[test]
    fn signed_action_request_is_bounded_and_has_stable_norito_and_json() {
        let operation = PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1;
        let request = PrivacyExact12ActionRequestV1::try_new(
            operation,
            vec![0xAB; 64],
            Some(PrivacyExact12CapabilityManifestDigestV1::new([0x31; 32])),
        )
        .expect("valid signed action request");
        assert_eq!(request.operation(), operation);
        assert_eq!(request.signed_transaction_versioned(), &[0xAB; 64]);
        assert_stable_schema_wire(
            &request,
            PRIVACY_EXACT12_ACTION_REQUEST_SCHEMA_NAME_V1,
            [
                0x39, 0xA1, 0x9E, 0xC7, 0xEA, 0xBD, 0xBA, 0xAA, 0x47, 0xBB, 0xDC, 0xE8, 0x28, 0x32,
                0x06, 0x01,
            ],
        );
        let json = norito::json::to_json(&request).expect("request JSON");
        let decoded: PrivacyExact12ActionRequestV1 =
            norito::json::from_json(&json).expect("decode request JSON");
        assert_eq!(decoded, request);
        decoded.validate().expect("validate decoded request");
        let unknown = json.replacen('{', "{\"legacy_local_acceptance\":true,", 1);
        assert!(norito::json::from_json::<PrivacyExact12ActionRequestV1>(&unknown).is_err());

        assert!(matches!(
            PrivacyExact12ActionRequestV1::try_new(operation, Vec::new(), None),
            Err(PrivacyExact12ActionModelErrorV1::InvalidSignedTransactionLength { actual: 0, .. })
        ));
        assert!(matches!(
            PrivacyExact12ActionRequestV1::try_new(
                operation,
                vec![0x11; PRIVACY_EXACT12_MAX_SIGNED_TRANSACTION_BYTES_V1 + 1],
                None,
            ),
            Err(PrivacyExact12ActionModelErrorV1::InvalidSignedTransactionLength { .. })
        ));
        assert_eq!(
            PrivacyExact12ActionRequestV1::try_new(
                operation,
                vec![1],
                Some(PrivacyExact12CapabilityManifestDigestV1::new([0; 32])),
            ),
            Err(PrivacyExact12ActionModelErrorV1::ZeroExpectedManifestDigest)
        );
    }

    #[test]
    fn operation_view_enforces_protocol_effect_and_terminal_state_mappings() {
        let submitted = submitted_view();
        submitted.validate().expect("submitted view validates");
        assert_eq!(submitted.local_state().canonical_label(), "submitted");
        assert_eq!(
            submitted.operation_schema().protocol_id(),
            submitted.protocol_id()
        );
        assert_eq!(
            submitted.ledger_effect_kind(),
            PrivacyLedgerEffectKindV1::OrchardNoteStateTransition
        );

        PrivacyActionOperationViewV1::try_new(
            submitted.protocol_id(),
            submitted.operation_schema(),
            submitted.transaction_hash(),
            submitted.transaction_intent_digest(),
            submitted.statement_digest(),
            submitted.proof_envelope_hash(),
            PrivacyActionLocalStateV1::Terminal,
            Some(PrivacyActionTerminalChainStateV1::Committed),
            Some(42),
            None,
            submitted.ledger_effect_kind(),
            submitted.capability_manifest_digest(),
            submitted.capability_committed_height(),
            None,
            None,
            None,
            None,
        )
        .expect("committed view");
        let rejected = PrivacyActionOperationViewV1::try_new(
            submitted.protocol_id(),
            submitted.operation_schema(),
            submitted.transaction_hash(),
            submitted.transaction_intent_digest(),
            submitted.statement_digest(),
            submitted.proof_envelope_hash(),
            PrivacyActionLocalStateV1::Terminal,
            Some(PrivacyActionTerminalChainStateV1::Rejected),
            Some(42),
            Some("verified Orchard nullifier was already consumed".to_owned()),
            submitted.ledger_effect_kind(),
            submitted.capability_manifest_digest(),
            submitted.capability_committed_height(),
            None,
            None,
            None,
            None,
        )
        .expect("rejected view");
        assert_eq!(
            rejected.rejection_reason(),
            Some("verified Orchard nullifier was already consumed")
        );
        PrivacyActionOperationViewV1::try_new(
            submitted.protocol_id(),
            submitted.operation_schema(),
            submitted.transaction_hash(),
            submitted.transaction_intent_digest(),
            submitted.statement_digest(),
            submitted.proof_envelope_hash(),
            PrivacyActionLocalStateV1::Terminal,
            Some(PrivacyActionTerminalChainStateV1::Expired),
            None,
            None,
            submitted.ledger_effect_kind(),
            submitted.capability_manifest_digest(),
            submitted.capability_committed_height(),
            None,
            None,
            None,
            None,
        )
        .expect("expired view");
    }

    #[test]
    fn applied_operation_view_requires_exact_execution_capability_and_finality_evidence() {
        let submitted = submitted_view();
        let applied_block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x61; 32]),
        );
        let applied = PrivacyActionOperationViewV1::try_new(
            submitted.protocol_id(),
            submitted.operation_schema(),
            submitted.transaction_hash(),
            submitted.transaction_intent_digest(),
            submitted.statement_digest(),
            submitted.proof_envelope_hash(),
            PrivacyActionLocalStateV1::Terminal,
            Some(PrivacyActionTerminalChainStateV1::Applied),
            Some(42),
            None,
            submitted.ledger_effect_kind(),
            submitted.capability_manifest_digest(),
            submitted.capability_committed_height(),
            Some(PrivacyExact12CapabilityManifestDigestV1::new([0x62; 32])),
            Some(41),
            Some(43),
            Some(applied_block_hash),
        )
        .expect("applied view");
        assert_eq!(
            applied.execution_capability_manifest_digest(),
            Some(PrivacyExact12CapabilityManifestDigestV1::new([0x62; 32]))
        );
        assert_eq!(applied.execution_capability_committed_height(), Some(41));
        assert_eq!(applied.execution_receipt_finalized_height(), Some(43));
        assert_eq!(
            applied.execution_receipt_finalized_block_hash(),
            Some(&applied_block_hash)
        );

        let mut missing_execution_digest = applied.clone();
        missing_execution_digest.execution_capability_manifest_digest = None;
        assert_eq!(
            missing_execution_digest.validate(),
            Err(PrivacyExact12ActionModelErrorV1::InvalidStateCombination)
        );
        let mut future_execution_capability = applied.clone();
        future_execution_capability.execution_capability_committed_height = Some(43);
        assert_eq!(
            future_execution_capability.validate(),
            Err(PrivacyExact12ActionModelErrorV1::InvalidExecutionReceiptHeights)
        );
        let mut stale_execution_finality = applied.clone();
        stale_execution_finality.execution_receipt_finalized_height = Some(41);
        assert_eq!(
            stale_execution_finality.validate(),
            Err(PrivacyExact12ActionModelErrorV1::InvalidExecutionReceiptHeights)
        );
        let mut zero_execution_digest = applied.clone();
        zero_execution_digest.execution_capability_manifest_digest =
            Some(PrivacyExact12CapabilityManifestDigestV1::new([0; 32]));
        assert_eq!(
            zero_execution_digest.validate(),
            Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionCapabilityManifestDigest)
        );
        let mut zero_execution_block = applied.clone();
        zero_execution_block.execution_receipt_finalized_block_hash = Some(
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0; 32])),
        );
        assert_eq!(
            zero_execution_block.validate(),
            Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionReceiptFinalizedBlockHash)
        );
        let mut submitted_with_execution_evidence = submitted;
        submitted_with_execution_evidence.execution_capability_manifest_digest =
            applied.execution_capability_manifest_digest;
        assert_eq!(
            submitted_with_execution_evidence.validate(),
            Err(PrivacyExact12ActionModelErrorV1::InvalidStateCombination)
        );
        let mut terminal_before_preflight = applied;
        terminal_before_preflight.committed_height = Some(40);
        terminal_before_preflight.execution_capability_committed_height = Some(40);
        assert_eq!(
            terminal_before_preflight.validate(),
            Err(PrivacyExact12ActionModelErrorV1::TerminalHeightBeforeCapabilitySnapshot)
        );
    }

    #[test]
    fn operation_view_rejects_invalid_protocol_effect_hash_and_reason() {
        let submitted = submitted_view();
        assert_eq!(
            PrivacyActionOperationViewV1::try_new(
                PrivacyProtocolIdV1::PqMaspStarkV0,
                submitted.operation_schema(),
                submitted.transaction_hash(),
                submitted.transaction_intent_digest(),
                submitted.statement_digest(),
                submitted.proof_envelope_hash(),
                submitted.local_state(),
                None,
                None,
                None,
                submitted.ledger_effect_kind(),
                submitted.capability_manifest_digest(),
                submitted.capability_committed_height(),
                None,
                None,
                None,
                None,
            ),
            Err(PrivacyExact12ActionModelErrorV1::OperationProtocolMismatch)
        );
        assert_eq!(
            PrivacyActionOperationViewV1::try_new(
                submitted.protocol_id(),
                submitted.operation_schema(),
                submitted.transaction_hash(),
                submitted.transaction_intent_digest(),
                submitted.statement_digest(),
                submitted.proof_envelope_hash(),
                submitted.local_state(),
                None,
                None,
                None,
                PrivacyLedgerEffectKindV1::VerificationOnly,
                submitted.capability_manifest_digest(),
                submitted.capability_committed_height(),
                None,
                None,
                None,
                None,
            ),
            Err(PrivacyExact12ActionModelErrorV1::OperationLedgerEffectMismatch)
        );
        assert!(matches!(
            PrivacyActionOperationViewV1::try_new(
                submitted.protocol_id(),
                submitted.operation_schema(),
                [0; 32],
                submitted.transaction_intent_digest(),
                submitted.statement_digest(),
                submitted.proof_envelope_hash(),
                submitted.local_state(),
                None,
                None,
                None,
                submitted.ledger_effect_kind(),
                submitted.capability_manifest_digest(),
                submitted.capability_committed_height(),
                None,
                None,
                None,
                None,
            ),
            Err(PrivacyExact12ActionModelErrorV1::ZeroTransactionHash)
        ));
        assert!(matches!(
            PrivacyActionOperationViewV1::try_new(
                submitted.protocol_id(),
                submitted.operation_schema(),
                submitted.transaction_hash(),
                submitted.transaction_intent_digest(),
                submitted.statement_digest(),
                submitted.proof_envelope_hash(),
                PrivacyActionLocalStateV1::Terminal,
                Some(PrivacyActionTerminalChainStateV1::Rejected),
                Some(42),
                Some(" rejected ".to_owned()),
                submitted.ledger_effect_kind(),
                submitted.capability_manifest_digest(),
                submitted.capability_committed_height(),
                None,
                None,
                None,
                None,
            ),
            Err(PrivacyExact12ActionModelErrorV1::InvalidRejectionReason)
        ));
    }

    #[test]
    fn operation_view_has_stable_norito_and_json() {
        let view = submitted_view();
        assert_stable_schema_wire(
            &view,
            PRIVACY_ACTION_OPERATION_VIEW_SCHEMA_NAME_V1,
            [
                0xB0, 0x90, 0xA4, 0x15, 0x46, 0xD4, 0x05, 0xD5, 0xF5, 0x92, 0x94, 0xDB, 0xD1, 0xA1,
                0xF5, 0x5C,
            ],
        );
        let json = norito::json::to_json(&view).expect("operation view JSON");
        let decoded: PrivacyActionOperationViewV1 =
            norito::json::from_json(&json).expect("decode operation view JSON");
        assert_eq!(decoded, view);
        decoded.validate().expect("validate decoded operation view");
        let unknown = json.replacen('{', "{\"local_proof_accepted\":true,", 1);
        assert!(norito::json::from_json::<PrivacyActionOperationViewV1>(&unknown).is_err());
    }

    #[test]
    fn execution_receipt_requires_exact_semantic_and_finality_bindings() {
        let receipt = execution_receipt();
        receipt
            .validate()
            .expect("valid finalized execution receipt");

        let mut mutated = receipt;
        mutated.operation_schema = PrivacyOperationSchemaV1::ZkAmsBatchAdmissionActionV1;
        assert_eq!(
            mutated.validate(),
            Err(PrivacyExact12ActionModelErrorV1::OperationProtocolMismatch)
        );

        let mut mutated = receipt;
        mutated.action_index = super::super::TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1;
        assert!(matches!(
            mutated.validate(),
            Err(PrivacyExact12ActionModelErrorV1::ExecutionReceiptActionIndexOutOfRange { .. })
        ));

        let mut mutated = receipt;
        mutated.capability_committed_height = 42;
        assert_eq!(
            mutated.validate(),
            Err(PrivacyExact12ActionModelErrorV1::InvalidExecutionReceiptHeights)
        );

        let mut mutated = receipt;
        mutated.finalized_block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0; 32]));
        assert_eq!(
            mutated.validate(),
            Err(PrivacyExact12ActionModelErrorV1::ZeroExecutionReceiptFinalizedBlockHash)
        );
    }

    #[test]
    fn execution_receipt_has_stable_norito_and_json() {
        let receipt = execution_receipt();
        assert_stable_schema_wire(
            &receipt,
            PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_SCHEMA_NAME_V1,
            [
                0x89, 0xC3, 0x02, 0xBE, 0x32, 0x5E, 0x4D, 0x3C, 0x73, 0x5D, 0x6D, 0x91, 0x14, 0x8C,
                0xE3, 0x5B,
            ],
        );
        let json = norito::json::to_json(&receipt).expect("execution receipt JSON");
        let decoded: PrivacyActionExecutionReceiptViewV1 =
            norito::json::from_json(&json).expect("decode execution receipt JSON");
        assert_eq!(decoded, receipt);
        decoded
            .validate()
            .expect("validate decoded execution receipt");
        let unknown = json.replacen('{', "{\"http_acceptance\":true,", 1);
        assert!(norito::json::from_json::<PrivacyActionExecutionReceiptViewV1>(&unknown).is_err());
    }
}
