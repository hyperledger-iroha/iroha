//! Recoverable KAGEMUSHA three-message lifecycle and physical receiver/sender capacity ledgers.
//!
//! A sender durably prepares the request-bound transition, verifies and
//! persists its private state candidate proof, and only then lets qualified hardware atomically
//! install the successor and issue a recoverable commit certificate. The compact terminal payment
//! or redemption proof and canonical envelope are persisted before exposure. Recovery resumes the
//! exact durable stage and every retry returns the originally persisted bytes.

use std::{cell::Cell, collections::BTreeMap};

#[cfg(feature = "zk-halo2-ipa")]
use std::ops::Range;

use iroha_data_model::kagemusha::{
    KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, KAGEMUSHA_OUTBOX_RETRY_METADATA_MAX_BYTES_V1,
    KAGEMUSHA_PAYMENT_MAX_BYTES_V1, KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1,
    KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1, KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaAppAttestHardwareTransitionSelectionV1,
    KagemushaAppAttestationAuthorityPolicyV1, KagemushaAuthenticatedReleaseV1,
    KagemushaCommitCertificateV1, KagemushaHardwareCredentialV1, KagemushaHardwareProfileV1,
    KagemushaHardwareTerminalBodyV1, KagemushaHardwareTransitionSelectionExpectedV1,
    KagemushaLifecycleBindingV1, KagemushaOperationKindV1, KagemushaOutboxReservationV1,
    KagemushaPairedProofV1, KagemushaPaymentOutputV1, KagemushaPaymentRequestV1,
    KagemushaPaymentV1, KagemushaRedemptionProofV1, KagemushaRedemptionStatementV1,
    KagemushaRedemptionVoucherV1, KagemushaSignedHardwareTransitionSelectionV1,
    KagemushaVerifiedAppAttestSelectionV1, kagemusha_ciphertext_digest_v1,
    kagemusha_payment_body_digest_v1, kagemusha_prepared_transfer_digest_v1,
};
use norito::codec::{Decode, Encode};

use super::{
    DigestV1, HardwareTransitionStatementV1, KAGEMUSHA_STATE_VERSION_V1,
    KagemushaOutgoingOperationIndexErrorV1, KagemushaOutgoingOperationIndexV1,
    KagemushaOutgoingOperationPhaseV1, KagemushaOutgoingOperationPrepareOutcomeV1,
    KagemushaStateErrorV1, KagemushaStateProofReleaseV1, KagemushaStateV1,
    TransitionProofStatementV1, VerifiedKagemushaRedemptionReleaseV1, canonical_sha256_digest,
};
use crate::zk::kagemusha_v1_recursion::{
    KagemushaPastaParityV1, KagemushaPreparedIntentCommitmentsV1, KagemushaRecursionArtifactsV1,
    KagemushaRecursivePublicOutputV1, KagemushaRecursiveVerifierV1,
    KagemushaStateRelationPublicInputsV1, canonical_incoming_payment_claims_binding_v1,
    canonical_prepared_transition_binding_digest_v1, canonical_sender_state_pair_digest_v1,
    canonical_terminal_send_output_binding_v1, kagemusha_candidate_envelope_digest_v1,
    verify_kagemusha_recursive_proof_v1, verify_kagemusha_state_proof_v1,
};

const PREPARATION_ID_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:outgoing-preparation";
const SEALED_TRANSITION_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:sealed-transition-inputs";
const SEALED_RECOVERY_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:sealed-recovery-seeds";
const OUTGOING_ENVELOPE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:terminal-envelope";
const TERMINAL_JOURNAL_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:terminal-private-journal";
const TERMINAL_RECOVERY_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:terminal-private-recovery";

// These are local physical provisioning floors, not protocol history/count limits. Implementations
// may provision arbitrarily larger stores and every insertion is charged by its actual canonical
// snapshot bytes.
const MINIMUM_DURABLE_INBOX_BYTES_V1: u64 = 512 * 1024;
const LIVE_OUTBOX_SLOT_BYTES_V1: u64 = 192 * 1024;
const MINIMUM_DURABLE_OUTBOX_BYTES_V1: u64 = LIVE_OUTBOX_SLOT_BYTES_V1 + 4 * 1024;
const PREPARED_OUTGOING_INTENT_MAX_BYTES_V1: u64 = 64 * 1024;

/// Physical durable-storage budget assigned to one Kagemusha lane.
///
/// These byte budgets are local resource bounds only. Neither value limits payment history,
/// accepted-credit count, proof depth, ancestry, or fan-in.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::KagemushaDurableCapacityV1"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaDurableCapacityV1 {
    /// Bytes durably available for accepted credits and byte-identical acknowledgements.
    pub inbox_bytes: u64,
    /// Bytes durably available for transition intents and byte-identical terminal retries.
    pub outbox_bytes: u64,
}

impl KagemushaDurableCapacityV1 {
    /// Conservative physical floor for one complete receiver staging record.
    pub const MINIMUM_INBOX_BYTES: u64 = MINIMUM_DURABLE_INBOX_BYTES_V1;
    /// Conservative physical floor for one complete outgoing terminal operation.
    pub const MINIMUM_OUTBOX_BYTES: u64 = MINIMUM_DURABLE_OUTBOX_BYTES_V1;

    /// Validate that the lane can complete at least one receive and one terminal operation.
    pub fn validate(self) -> Result<(), KagemushaStateErrorV1> {
        if self.inbox_bytes < Self::MINIMUM_INBOX_BYTES
            || self.outbox_bytes < Self::MINIMUM_OUTBOX_BYTES
        {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(())
    }
}

/// Receiver-owned physical inbox ledger.
///
/// Cumulative accepted receipts and consumed identities live in authenticated external history
/// and are never admission limits. This component owns physical-byte conservation only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::KagemushaReceiverInboxCapacityV1"
)]
pub struct KagemushaReceiverInboxCapacityV1 {
    total_inbox_bytes: u64,
    committed_inbox_bytes: u64,
    pending_credit_entry_bytes: u64,
    pending_receipt_entry_bytes: u64,
    // Mint reservations and retained records use the same physical pool as peer credits.
    mint_inbox_bytes: u64,
}

impl KagemushaReceiverInboxCapacityV1 {
    /// Create an empty physical receiver-inbox ledger.
    #[must_use]
    pub const fn new(total_inbox_bytes: u64) -> Self {
        Self {
            total_inbox_bytes,
            committed_inbox_bytes: 0,
            pending_credit_entry_bytes: 0,
            pending_receipt_entry_bytes: 0,
            mint_inbox_bytes: 0,
        }
    }

    /// Return the total locally provisioned durable inbox bytes.
    #[must_use]
    pub const fn total_inbox_bytes(&self) -> u64 {
        self.total_inbox_bytes
    }

    /// Return the exact canonical receiver-snapshot bytes currently charged.
    #[must_use]
    pub const fn committed_inbox_bytes(&self) -> u64 {
        self.committed_inbox_bytes
    }

    /// Return physical bytes not committed to staged peer or mint credits.
    #[must_use]
    pub const fn available_inbox_bytes(&self) -> u64 {
        self.total_inbox_bytes
            .saturating_sub(self.committed_inbox_bytes)
            .saturating_sub(self.mint_inbox_bytes)
    }

    /// Bytes allocated to pre-debit mint reservations and durable mint records.
    #[must_use]
    pub const fn mint_inbox_bytes(&self) -> u64 {
        self.mint_inbox_bytes
    }

    /// Install the exact mint-journal charge without borrowing any issued peer allocation.
    pub(super) fn with_mint_inbox_bytes(&self, bytes: u64) -> Result<Self, KagemushaStateErrorV1> {
        let mut next = self.clone();
        next.mint_inbox_bytes = bytes;
        next.reconcile_receiver_snapshot_usage()?;
        Ok(next)
    }

    /// Reconcile the mint journal against its hardware-anchored physical allocation.
    pub(super) fn validate_mint_inbox_bytes(
        &self,
        expected: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.mint_inbox_bytes != expected {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    /// Return a checked capacity successor after one durable inbound credit is staged.
    pub(super) fn receiver_snapshot_staged_successor(
        &self,
        pending_credit_entry_bytes: u64,
        pending_receipt_entry_bytes: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let mut next = self.clone();
        next.pending_credit_entry_bytes = next
            .pending_credit_entry_bytes
            .checked_add(pending_credit_entry_bytes)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        next.pending_receipt_entry_bytes = next
            .pending_receipt_entry_bytes
            .checked_add(pending_receipt_entry_bytes)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        next.reconcile_receiver_snapshot_usage()?;
        Ok(next)
    }

    /// Return a checked capacity successor after one pending credit becomes consumed.
    pub(super) fn receiver_snapshot_folded_successor(
        &self,
        pending_credit_entry_bytes: u64,
        pending_receipt_entry_bytes: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let mut next = self.clone();
        next.pending_credit_entry_bytes = next
            .pending_credit_entry_bytes
            .checked_sub(pending_credit_entry_bytes)
            .ok_or(KagemushaStateErrorV1::StateInvariant)?;
        next.pending_receipt_entry_bytes = next
            .pending_receipt_entry_bytes
            .checked_sub(pending_receipt_entry_bytes)
            .ok_or(KagemushaStateErrorV1::StateInvariant)?;
        next.reconcile_receiver_snapshot_usage()?;
        Ok(next)
    }

    pub(super) fn validate_recovered_with_snapshot_usage(
        &self,
        snapshot_bytes: u64,
        pending_credit_entry_bytes: u64,
        pending_receipt_entry_bytes: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.committed_inbox_bytes != snapshot_bytes
            || self.pending_credit_entry_bytes != pending_credit_entry_bytes
            || self.pending_receipt_entry_bytes != pending_receipt_entry_bytes
            || self.committed_inbox_bytes > self.total_inbox_bytes
            || self.mint_inbox_bytes > self.total_inbox_bytes - self.committed_inbox_bytes
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn reconcile_receiver_snapshot_usage(&mut self) -> Result<(), KagemushaStateErrorV1> {
        let snapshot_bytes = receiver_snapshot_usage_from_entry_bytes(
            self.pending_credit_entry_bytes,
            self.pending_receipt_entry_bytes,
        )?;
        if snapshot_bytes > self.total_inbox_bytes
            || self.mint_inbox_bytes > self.total_inbox_bytes - snapshot_bytes
        {
            return Err(KagemushaStateErrorV1::ReceiverCapacityExhausted);
        }
        self.committed_inbox_bytes = snapshot_bytes;
        Ok(())
    }
}

pub(super) fn receiver_snapshot_usage_from_entry_bytes(
    pending_credit_entry_bytes: u64,
    pending_receipt_entry_bytes: u64,
) -> Result<u64, KagemushaStateErrorV1> {
    [pending_credit_entry_bytes, pending_receipt_entry_bytes]
        .into_iter()
        .try_fold(0_u64, |total, entry_bytes| {
            total
                .checked_add(receiver_vector_usage_from_entry_bytes(entry_bytes)?)
                .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)
        })
}

fn receiver_vector_usage_from_entry_bytes(entry_bytes: u64) -> Result<u64, KagemushaStateErrorV1> {
    const SEQUENCE_COUNT_BYTES: u64 = 8;
    let payload_bytes = SEQUENCE_COUNT_BYTES
        .checked_add(entry_bytes)
        .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
    let baseline_prefix_bytes = canonical_length_prefix_bytes(SEQUENCE_COUNT_BYTES)?;
    let prefix_bytes = canonical_length_prefix_bytes(payload_bytes)?;
    prefix_bytes
        .checked_add(payload_bytes)
        .and_then(|total| total.checked_sub(baseline_prefix_bytes + SEQUENCE_COUNT_BYTES))
        .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)
}

fn canonical_length_prefix_bytes(payload_bytes: u64) -> Result<u64, KagemushaStateErrorV1> {
    let mut encoded = Vec::with_capacity(10);
    norito::core::write_len_with_flags(
        &mut encoded,
        payload_bytes,
        norito::core::default_encode_flags(),
    )
    .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    u64::try_from(encoded.len()).map_err(|_| KagemushaStateErrorV1::ArithmeticOverflow)
}

fn validate_outbox_reservation(
    reservation: KagemushaOutboxReservationV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    reservation
        .validate()
        .map_err(|_| KagemushaStateErrorV1::SenderOutboxCapacityExhausted)?;
    if u64::from(reservation.reserved_outbox_bytes) < LIVE_OUTBOX_SLOT_BYTES_V1 {
        return Err(KagemushaStateErrorV1::SenderOutboxCapacityExhausted);
    }
    reservation
        .canonical_commitment()
        .map_err(|_| KagemushaStateErrorV1::SenderOutboxCapacityExhausted)
}

/// Return the implementation slot floor for one terminal operation.
#[cfg(test)]
pub(super) const fn implementation_live_outbox_slot_bytes_v1(
    operation_kind: KagemushaOperationKindV1,
) -> Option<u64> {
    match operation_kind {
        KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit => {
            Some(LIVE_OUTBOX_SLOT_BYTES_V1)
        }
        _ => None,
    }
}

/// Result of reserving sender durable-outbox capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SenderOutboxReservationOutcomeV1 {
    /// A new one-use reservation was installed.
    Reserved,
    /// The byte-identical reservation already existed during recovery.
    AlreadyReserved,
}

/// Opaque authority to ask qualified hardware to commit one exact staged transition intent.
///
/// The capability is neither cloneable nor serializable. Recovery reissues it only while the
/// canonical journal still contains the same uncommitted intent.
#[derive(Debug, PartialEq, Eq)]
pub struct KagemushaOutgoingCommitCapabilityV1 {
    preparation_id: DigestV1,
    reservation_commitment: DigestV1,
    _non_clone_seal: Cell<()>,
}

impl KagemushaOutgoingCommitCapabilityV1 {
    pub(super) fn for_prepared(
        prepared: &PreparedOutgoingCandidateV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        Ok(Self {
            preparation_id: prepared.preparation_id,
            reservation_commitment: validate_outbox_reservation(prepared.outbox_reservation)?,
            _non_clone_seal: Cell::new(()),
        })
    }

    pub(super) fn authorizes(
        &self,
        prepared: &PreparedOutgoingCandidateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self != &Self::for_prepared(prepared)? {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }
}

/// Core-derived sender inputs durably sealed before hardware consumes a payment predecessor.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PreparedSendMaterialV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct PreparedSendMaterialV1 {
    pub(super) proof_statement: TransitionProofStatementV1,
    pub(super) lifecycle: KagemushaLifecycleBindingV1,
    pub(super) output: KagemushaPaymentOutputV1,
    pub(super) request: KagemushaPaymentRequestV1,
    pub(super) encrypted_credit: Vec<u8>,
    pub(super) outbox_reservation: KagemushaOutboxReservationV1,
    pub(super) prepared_one_use_authorization_digest: DigestV1,
    pub(super) sealed_transition_inputs: Vec<u8>,
    pub(super) sealed_recovery_seeds: Vec<u8>,
    pub(super) normalized_guard_statement_digest: DigestV1,
}

/// Core-derived redeemer inputs sealed before hardware consumes a redemption predecessor.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PreparedRedemptionMaterialV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct PreparedRedemptionMaterialV1 {
    pub(super) proof_statement: TransitionProofStatementV1,
    pub(super) statement: KagemushaRedemptionStatementV1,
    pub(super) outbox_reservation: KagemushaOutboxReservationV1,
    pub(super) prepared_one_use_authorization_digest: DigestV1,
    pub(super) artifact_manifest_digest: DigestV1,
    pub(super) sealed_transition_inputs: Vec<u8>,
    pub(super) sealed_recovery_seeds: Vec<u8>,
    pub(super) normalized_guard_statement_digest: DigestV1,
}

#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PreparedSendPublicProjectionV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct PreparedSendPublicProjectionV1 {
    request: KagemushaPaymentRequestV1,
    lifecycle: KagemushaLifecycleBindingV1,
    output: KagemushaPaymentOutputV1,
    encrypted_credit: Vec<u8>,
}

#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PreparedRedemptionPublicProjectionV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct PreparedRedemptionPublicProjectionV1 {
    statement: KagemushaRedemptionStatementV1,
    artifact_manifest_digest: DigestV1,
}

#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PreparedPublicProjectionV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
enum PreparedPublicProjectionV1 {
    Send(Box<PreparedSendPublicProjectionV1>),
    Redemption(PreparedRedemptionPublicProjectionV1),
}

/// Read-only, operation-specific public material retained in a prepared outgoing record.
///
/// Every reference borrows the original persisted object or byte slice. This view does not
/// reconstruct ciphertext, expose unsealed secrets, authenticate a snapshot, or authorize a
/// monetary transition. A provider must independently authenticate recovery and retain all
/// required private witnesses before using these inputs to resume proof generation.
///
/// Ciphertext cannot be changed through the view:
///
/// ```compile_fail
/// use iroha_core::zk::kagemusha_v1_state::PreparedOutgoingRecoveryViewV1;
/// fn replace_ciphertext(view: PreparedOutgoingRecoveryViewV1<'_>) {
///     if let PreparedOutgoingRecoveryViewV1::Send { encrypted_credit, .. } = view {
///         encrypted_credit[0] = 0;
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedOutgoingRecoveryViewV1<'a> {
    /// Exact request-bound public inputs of a prepared peer payment.
    Send {
        /// Original signed receiver request.
        request: &'a KagemushaPaymentRequestV1,
        /// Full separately bound lifecycle of the private sender transition.
        lifecycle: &'a KagemushaLifecycleBindingV1,
        /// Original derived compact payment output.
        output: &'a KagemushaPaymentOutputV1,
        /// Original encrypted-credit bytes; recovery must reuse these exactly.
        encrypted_credit: &'a [u8],
    },
    /// Exact public inputs of a prepared chain-facing redemption.
    Redemption {
        /// Original redemption statement, including the full lifecycle.
        statement: &'a KagemushaRedemptionStatementV1,
        /// Original artifact manifest pinned before commitment.
        artifact_manifest_digest: &'a DigestV1,
    },
}

impl PreparedPublicProjectionV1 {
    fn lifecycle(&self) -> &KagemushaLifecycleBindingV1 {
        match self {
            Self::Send(projection) => &projection.lifecycle,
            Self::Redemption(projection) => &projection.statement.lifecycle,
        }
    }

    fn semantic_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        match self {
            Self::Send(projection) => {
                kagemusha_payment_body_digest_v1(&projection.output, &projection.encrypted_credit)
                    .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)
            }
            Self::Redemption(projection) => projection
                .statement
                .canonical_digest()
                .map_err(|_| KagemushaStateErrorV1::InvalidRedemption),
        }
    }

    const fn operation(&self) -> KagemushaOperationKindV1 {
        match self {
            Self::Send(_) => KagemushaOperationKindV1::SendSplit,
            Self::Redemption(_) => KagemushaOperationKindV1::RedeemSplit,
        }
    }
}

// Every field has a fixed width. The byte order below is also the future recursive SHA
// transcript: the candidate envelope cannot be included because it is computed after this ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreparationIdCommitmentsV1 {
    operation_tag: u8,
    predecessor_state_commitment: DigestV1,
    successor_state_commitment: DigestV1,
    state_transition_digest: DigestV1,
    prepared_transition_binding_digest: DigestV1,
    projection_semantic_digest: DigestV1,
    lifecycle_binding_digest: DigestV1,
    request_digest: DigestV1,
    artifact_manifest_digest: DigestV1,
    normalized_guard_statement_digest: DigestV1,
    outbox_reservation_commitment: DigestV1,
    prepared_one_use_authorization_digest: DigestV1,
    sealed_transition_inputs_len: u64,
    sealed_transition_inputs_digest: DigestV1,
    sealed_recovery_seeds_len: u64,
    sealed_recovery_seeds_digest: DigestV1,
}

/// Hash the fixed, domain-separated pre-proof transcript selected by an outgoing preparation.
///
/// A later circuit must derive these fields from its State/Guard relation and verify openings of
/// both length-prefixed sealed-stream digests before this ID can grant terminal authority.
/// TODO: recompute this transcript from authenticated State/Guard sources in both terminal
/// parities and link both sealed-stream openings to the carried candidate digests.
fn hash_preparation_id_commitments_v1(fields: PreparationIdCommitmentsV1) -> DigestV1 {
    use sha2::{Digest as _, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(PREPARATION_ID_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
    hasher.update([fields.operation_tag]);
    for digest in [
        fields.predecessor_state_commitment,
        fields.successor_state_commitment,
        fields.state_transition_digest,
        fields.prepared_transition_binding_digest,
        fields.projection_semantic_digest,
        fields.lifecycle_binding_digest,
        fields.request_digest,
        fields.artifact_manifest_digest,
        fields.normalized_guard_statement_digest,
        fields.outbox_reservation_commitment,
        fields.prepared_one_use_authorization_digest,
    ] {
        hasher.update(digest);
    }
    hasher.update(fields.sealed_transition_inputs_len.to_le_bytes());
    hasher.update(fields.sealed_transition_inputs_digest);
    hasher.update(fields.sealed_recovery_seeds_len.to_le_bytes());
    hasher.update(fields.sealed_recovery_seeds_digest);
    hasher.finalize().into()
}

/// Reduce the validated prepared record to a fixed-width pre-proof commitment.
#[allow(clippy::too_many_arguments)]
fn preparation_id_v1(
    predecessor_state: &KagemushaStateV1,
    successor_state: &KagemushaStateV1,
    state_transition_digest: DigestV1,
    proof_statement: &TransitionProofStatementV1,
    projection: &PreparedPublicProjectionV1,
    outbox_reservation: KagemushaOutboxReservationV1,
    prepared_one_use_authorization_digest: DigestV1,
    sealed_transition_inputs: &[u8],
    sealed_recovery_seeds: &[u8],
    normalized_guard_statement_digest: DigestV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    validate_recovery_material(
        sealed_transition_inputs,
        sealed_recovery_seeds,
        normalized_guard_statement_digest,
    )?;
    let (operation_tag, request_digest, artifact_manifest_digest) = match projection {
        PreparedPublicProjectionV1::Send(projection) => (
            2,
            projection
                .request
                .canonical_digest()
                .map_err(|_| KagemushaStateErrorV1::InvalidPaymentRequest)?,
            [0; 32],
        ),
        PreparedPublicProjectionV1::Redemption(projection) => {
            (4, [0; 32], projection.artifact_manifest_digest)
        }
    };
    let sealed_transition_inputs_len = u64::try_from(sealed_transition_inputs.len())
        .map_err(|_| KagemushaStateErrorV1::InvalidRecoveryMaterial)?;
    let sealed_recovery_seeds_len = u64::try_from(sealed_recovery_seeds.len())
        .map_err(|_| KagemushaStateErrorV1::InvalidRecoveryMaterial)?;
    Ok(hash_preparation_id_commitments_v1(
        PreparationIdCommitmentsV1 {
            operation_tag,
            predecessor_state_commitment: predecessor_state.state_commitment,
            successor_state_commitment: successor_state.state_commitment,
            state_transition_digest,
            prepared_transition_binding_digest: proof_statement.prepared_transition_binding_digest,
            projection_semantic_digest: projection.semantic_digest()?,
            lifecycle_binding_digest: projection
                .lifecycle()
                .canonical_digest()
                .map_err(|_| KagemushaStateErrorV1::InvalidCandidateStage)?,
            request_digest,
            artifact_manifest_digest,
            normalized_guard_statement_digest,
            outbox_reservation_commitment: validate_outbox_reservation(outbox_reservation)?,
            prepared_one_use_authorization_digest,
            sealed_transition_inputs_len,
            sealed_transition_inputs_digest: digest_raw_bytes(
                SEALED_TRANSITION_DIGEST_DOMAIN_V1,
                sealed_transition_inputs,
            ),
            sealed_recovery_seeds_len,
            sealed_recovery_seeds_digest: digest_raw_bytes(
                SEALED_RECOVERY_DIGEST_DOMAIN_V1,
                sealed_recovery_seeds,
            ),
        },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::TerminalJournalCommitmentPreimageV1"
)]
struct TerminalJournalCommitmentPreimageV1 {
    // Spell the byte-array type explicitly: Norito's derive treats an opaque type alias as a
    // generic array and otherwise emits two bytes per digest byte in this fixed-frame preimage.
    preparation_id: [u8; 32],
    candidate_envelope_digest: [u8; 32],
    state_transition_digest: [u8; 32],
    outbox_reservation_commitment: [u8; 32],
    journal_revision_after: u128,
}

/// Native commitment for the exact durable journal record selected by a prepared intent.
pub(crate) fn terminal_journal_commitment_v1(
    preparation_id: DigestV1,
    candidate_envelope_digest: DigestV1,
    state_transition_digest: DigestV1,
    outbox_reservation_commitment: DigestV1,
    journal_revision_after: u128,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(
        TERMINAL_JOURNAL_COMMITMENT_DOMAIN_V1,
        &TerminalJournalCommitmentPreimageV1 {
            preparation_id,
            candidate_envelope_digest,
            state_transition_digest,
            outbox_reservation_commitment,
            journal_revision_after,
        },
    )
}

/// Canonical fixed-frame template and semantic offsets for the journal circuit opening.
///
/// Only the five semantic fields and the derived Norito checksum are holes. The template comes
/// from the same encoder and schema as the native commitment, so any framing drift fails closed.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn terminal_journal_canonical_layout_v1()
-> Result<(Vec<Option<u8>>, [Range<usize>; 5]), KagemushaStateErrorV1> {
    let zero = TerminalJournalCommitmentPreimageV1 {
        preparation_id: [0; 32],
        candidate_envelope_digest: [0; 32],
        state_transition_digest: [0; 32],
        outbox_reservation_commitment: [0; 32],
        journal_revision_after: 0,
    };
    let frame =
        norito::encode_canonical(&zero).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let payload_len = u64::from_le_bytes(
        frame
            .get(23..31)
            .ok_or(KagemushaStateErrorV1::CanonicalEncoding)?
            .try_into()
            .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?,
    );
    let payload_len =
        usize::try_from(payload_len).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let payload_start = frame
        .len()
        .checked_sub(payload_len)
        .ok_or(KagemushaStateErrorV1::CanonicalEncoding)?;
    // Default Norito V1 uses one compact length byte before each of these five fields.
    if payload_len != 4 * (1 + 32) + 1 + 16
        || payload_start != 48
        || frame.len() != 197
        || frame[39] != 0x02
    {
        return Err(KagemushaStateErrorV1::CanonicalEncoding);
    }
    let mut cursor = payload_start;
    let mut ranges = Vec::with_capacity(5);
    for width in [32_usize, 32, 32, 32, 16] {
        if frame.get(cursor) != Some(&(width as u8)) {
            return Err(KagemushaStateErrorV1::CanonicalEncoding);
        }
        let start = cursor + 1;
        cursor = start + width;
        ranges.push(start..cursor);
    }
    if cursor != frame.len() {
        return Err(KagemushaStateErrorV1::CanonicalEncoding);
    }
    let ranges: [Range<usize>; 5] = ranges
        .try_into()
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let mut template = frame.into_iter().map(Some).collect::<Vec<_>>();
    template[31..39].fill(None);
    for field in &ranges {
        if template[field.clone()]
            .iter()
            .any(|value| *value != Some(0))
        {
            return Err(KagemushaStateErrorV1::CanonicalEncoding);
        }
        template[field.clone()].fill(None);
    }
    Ok((template, ranges))
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::TerminalRecoveryCommitmentPreimageV1"
)]
struct TerminalRecoveryCommitmentPreimageV1 {
    // Keep the native recovery digest's fixed bytes identical to its recursive opening.
    preparation_id: [u8; 32],
    prepared_one_use_authorization_digest: [u8; 32],
    sealed_transition_inputs: Vec<u8>,
    sealed_recovery_seeds: Vec<u8>,
}

/// Native digest of the exact sealed recovery material retained by one prepared outgoing intent.
pub(crate) fn terminal_recovery_commitment_v1(
    preparation_id: DigestV1,
    prepared_one_use_authorization_digest: DigestV1,
    sealed_transition_inputs: &[u8],
    sealed_recovery_seeds: &[u8],
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(
        TERMINAL_RECOVERY_COMMITMENT_DOMAIN_V1,
        &TerminalRecoveryCommitmentPreimageV1 {
            preparation_id,
            prepared_one_use_authorization_digest,
            sealed_transition_inputs: sealed_transition_inputs.to_vec(),
            sealed_recovery_seeds: sealed_recovery_seeds.to_vec(),
        },
    )
}

/// The encoder-owned fixed header for a bounded recursive opening of recovery material.
///
/// Only the payload length and checksum are left open. Payload fields are assembled from
/// constrained digest cells and exact compact-length byte streams by the circuit.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn terminal_recovery_canonical_frame_prefix_v1()
-> Result<Vec<Option<u8>>, KagemushaStateErrorV1> {
    let zero = TerminalRecoveryCommitmentPreimageV1 {
        preparation_id: [0; 32],
        prepared_one_use_authorization_digest: [0; 32],
        sealed_transition_inputs: Vec::new(),
        sealed_recovery_seeds: Vec::new(),
    };
    let frame =
        norito::encode_canonical(&zero).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let mut payload = Vec::new();
    norito::codec::encode_adaptive_into(&zero, &mut payload)
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    let payload_start = frame
        .len()
        .checked_sub(payload.len())
        .ok_or(KagemushaStateErrorV1::CanonicalEncoding)?;
    let expected_payload = [
        &[32_u8][..],
        &[0_u8; 32],
        &[32_u8][..],
        &[0_u8; 32],
        &[8_u8][..],
        &[0_u8; 8],
        &[8_u8][..],
        &[0_u8; 8],
    ]
    .concat();
    if payload_start < norito::core::Header::SIZE
        || frame.get(payload_start..) != Some(payload.as_slice())
        || payload != expected_payload
        || frame.get(23..31) != Some(&(payload.len() as u64).to_le_bytes())
        || frame[39] != norito::core::header_flags::COMPACT_LEN
        || frame[norito::core::Header::SIZE..payload_start]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(KagemushaStateErrorV1::CanonicalEncoding);
    }
    let mut prefix = frame[..payload_start]
        .iter()
        .copied()
        .map(Some)
        .collect::<Vec<_>>();
    prefix[23..39].fill(None);
    Ok(prefix)
}

#[cfg(all(test, feature = "zk-halo2-ipa"))]
mod terminal_recovery_layout_tests {
    use super::*;
    use sha2::{Digest as _, Sha256};

    fn compact_length(value: usize) -> Vec<u8> {
        assert!(value <= 16_383);
        if value < 128 {
            vec![value as u8]
        } else {
            vec![(value as u8 & 0x7f) | 0x80, (value >> 7) as u8]
        }
    }

    #[test]
    fn recovery_norito_vec_fields_match_circuit_layout_across_compact_boundaries() {
        let prefix =
            terminal_recovery_canonical_frame_prefix_v1().expect("canonical recovery frame prefix");
        for (transition_len, seeds_len) in [(1, 1), (119, 120), (120, 119), (2048, 512)] {
            let transition = vec![0xa5; transition_len];
            let seeds = vec![0x5a; seeds_len];
            let value = TerminalRecoveryCommitmentPreimageV1 {
                preparation_id: [1; 32],
                prepared_one_use_authorization_digest: [2; 32],
                sealed_transition_inputs: transition.clone(),
                sealed_recovery_seeds: seeds.clone(),
            };
            let mut expected_payload = Vec::new();
            expected_payload.push(32);
            expected_payload.extend_from_slice(&[1; 32]);
            expected_payload.push(32);
            expected_payload.extend_from_slice(&[2; 32]);
            expected_payload.extend(compact_length(8 + transition_len));
            expected_payload.extend_from_slice(&(transition_len as u64).to_le_bytes());
            expected_payload.extend_from_slice(&transition);
            expected_payload.extend(compact_length(8 + seeds_len));
            expected_payload.extend_from_slice(&(seeds_len as u64).to_le_bytes());
            expected_payload.extend_from_slice(&seeds);
            let mut payload = Vec::new();
            norito::codec::encode_adaptive_into(&value, &mut payload)
                .expect("native bare recovery payload");
            assert_eq!(payload, expected_payload);
            let frame = norito::encode_canonical(&value).expect("native recovery frame");
            assert_eq!(&frame[prefix.len()..], payload.as_slice());
            for (actual, pinned) in frame.iter().zip(&prefix) {
                if let Some(pinned) = pinned {
                    assert_eq!(actual, pinned);
                }
            }
            let mut message = Vec::new();
            message.extend_from_slice(
                &(TERMINAL_RECOVERY_COMMITMENT_DOMAIN_V1.len() as u64).to_be_bytes(),
            );
            message.extend_from_slice(TERMINAL_RECOVERY_COMMITMENT_DOMAIN_V1);
            message.extend_from_slice(&(frame.len() as u64).to_be_bytes());
            message.extend_from_slice(&frame);
            let expected_digest: [u8; 32] = Sha256::digest(&message).into();
            assert_eq!(
                terminal_recovery_commitment_v1([1; 32], [2; 32], &transition, &seeds)
                    .expect("native recovery digest"),
                expected_digest,
            );
        }
    }
}

/// Durable sender-local transition intent staged before the hardware commit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PreparedOutgoingCandidateV1"
)]
pub struct PreparedOutgoingCandidateV1 {
    /// State-machine version.
    pub version: u16,
    /// Canonical identity of the exact sealed intent.
    pub preparation_id: DigestV1,
    /// Private aggregate predecessor consumed exactly once by hardware.
    pub(crate) predecessor_state: KagemushaStateV1,
    /// Private aggregate successor installed at hardware commit.
    pub(crate) successor_state: KagemushaStateV1,
    /// Digest of the exact private recursive state-transition statement.
    pub state_transition_digest: DigestV1,
    /// Exact verifier-reconstructed transition statement retained for recovery.
    pub proof_statement: TransitionProofStatementV1,
    projection: PreparedPublicProjectionV1,
    /// One-use physical outbox reservation.
    pub outbox_reservation: KagemushaOutboxReservationV1,
    /// Exact hardware-prepared one-use predecessor authorization bound by the candidate proof.
    pub prepared_one_use_authorization_digest: DigestV1,
    /// Hardware-sealed transition inputs.
    pub sealed_transition_inputs: Vec<u8>,
    /// Hardware-sealed deterministic proof/envelope recovery seeds.
    pub sealed_recovery_seeds: Vec<u8>,
    /// Exact normalized hardware relation consumed by the recursive proof.
    pub normalized_guard_statement_digest: DigestV1,
}

impl PreparedOutgoingCandidateV1 {
    /// Borrow the exact operation-specific public material needed during recovery.
    ///
    /// This is a projection only: it neither validates the record nor replaces authenticated
    /// hardware recovery, candidate verification, or the separately retained private witness.
    #[must_use]
    pub fn recovery_view(&self) -> PreparedOutgoingRecoveryViewV1<'_> {
        match &self.projection {
            PreparedPublicProjectionV1::Send(projection) => PreparedOutgoingRecoveryViewV1::Send {
                request: &projection.request,
                lifecycle: &projection.lifecycle,
                output: &projection.output,
                encrypted_credit: &projection.encrypted_credit,
            },
            PreparedPublicProjectionV1::Redemption(projection) => {
                PreparedOutgoingRecoveryViewV1::Redemption {
                    statement: &projection.statement,
                    artifact_manifest_digest: &projection.artifact_manifest_digest,
                }
            }
        }
    }

    /// Build one already-derived sender payment intent.
    pub(super) fn send(
        predecessor_state: KagemushaStateV1,
        successor_state: KagemushaStateV1,
        state_transition_digest: DigestV1,
        material: PreparedSendMaterialV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        validate_private_state_link(
            &predecessor_state,
            &successor_state,
            material.request.amount,
            KagemushaOperationKindV1::SendSplit,
        )?;
        validate_recovery_material(
            &material.sealed_transition_inputs,
            &material.sealed_recovery_seeds,
            material.normalized_guard_statement_digest,
        )?;
        material
            .request
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidPaymentRequest)?;
        material
            .output
            .peer_credit_context_against(&material.request)
            .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
        validate_request_against_state(&material.request, &predecessor_state)?;
        material
            .lifecycle
            .validate()
            .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
        if material.encrypted_credit.is_empty()
            || material.encrypted_credit.len() > KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1
            || material.lifecycle.network_id != material.request.network_id
            || material.lifecycle.asset != material.request.asset
            || material.lifecycle.asset_incarnation != material.request.asset_incarnation
            || material.lifecycle.scale != material.request.scale
            || material.lifecycle.liability_pool_id != material.request.liability_pool_id
            || material.lifecycle.release_id != material.request.release_id
            || material.lifecycle.operation_kind != KagemushaOperationKindV1::SendSplit
            || material.lifecycle.request_id != material.request.request_id
            || material.lifecycle.receiver_lane_commitment
                != material.request.hardware_credential.lane_commitment
            || material.lifecycle.credit_id != material.output.credit_id
            || material.lifecycle.ciphertext_digest
                != kagemusha_ciphertext_digest_v1(&material.encrypted_credit)
            || material.outbox_reservation.operation_kind != KagemushaOperationKindV1::SendSplit
            || material.output.commit_evidence.validate().is_err()
            || material.prepared_one_use_authorization_digest == [0; 32]
        {
            return Err(KagemushaStateErrorV1::InvalidPeerCredit);
        }
        Self::new(
            predecessor_state,
            successor_state,
            state_transition_digest,
            material.proof_statement,
            PreparedPublicProjectionV1::Send(Box::new(PreparedSendPublicProjectionV1 {
                request: material.request,
                lifecycle: material.lifecycle,
                output: material.output,
                encrypted_credit: material.encrypted_credit,
            })),
            material.outbox_reservation,
            material.prepared_one_use_authorization_digest,
            material.sealed_transition_inputs,
            material.sealed_recovery_seeds,
            material.normalized_guard_statement_digest,
        )
    }

    /// Build one already-derived partial or full redemption intent.
    pub(super) fn redemption(
        predecessor_state: KagemushaStateV1,
        successor_state: KagemushaStateV1,
        state_transition_digest: DigestV1,
        material: PreparedRedemptionMaterialV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        validate_private_state_link(
            &predecessor_state,
            &successor_state,
            material.statement.amount,
            KagemushaOperationKindV1::RedeemSplit,
        )?;
        validate_recovery_material(
            &material.sealed_transition_inputs,
            &material.sealed_recovery_seeds,
            material.normalized_guard_statement_digest,
        )?;
        material
            .statement
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidRedemption)?;
        if material.outbox_reservation.operation_kind != KagemushaOperationKindV1::RedeemSplit
            || material.statement.commit_evidence.validate().is_err()
            || material.prepared_one_use_authorization_digest == [0; 32]
            || material.artifact_manifest_digest == [0; 32]
        {
            return Err(KagemushaStateErrorV1::InvalidRedemption);
        }
        Self::new(
            predecessor_state,
            successor_state,
            state_transition_digest,
            material.proof_statement,
            PreparedPublicProjectionV1::Redemption(PreparedRedemptionPublicProjectionV1 {
                statement: material.statement,
                artifact_manifest_digest: material.artifact_manifest_digest,
            }),
            material.outbox_reservation,
            material.prepared_one_use_authorization_digest,
            material.sealed_transition_inputs,
            material.sealed_recovery_seeds,
            material.normalized_guard_statement_digest,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        predecessor_state: KagemushaStateV1,
        successor_state: KagemushaStateV1,
        state_transition_digest: DigestV1,
        proof_statement: TransitionProofStatementV1,
        projection: PreparedPublicProjectionV1,
        outbox_reservation: KagemushaOutboxReservationV1,
        prepared_one_use_authorization_digest: DigestV1,
        sealed_transition_inputs: Vec<u8>,
        sealed_recovery_seeds: Vec<u8>,
        normalized_guard_statement_digest: DigestV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        validate_outbox_reservation(outbox_reservation)?;
        if prepared_one_use_authorization_digest == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        validate_prepared_transition_statement(
            &predecessor_state,
            &successor_state,
            state_transition_digest,
            &proof_statement,
            &projection,
            outbox_reservation,
            prepared_one_use_authorization_digest,
            normalized_guard_statement_digest,
        )?;
        let preparation_id = preparation_id_v1(
            &predecessor_state,
            &successor_state,
            state_transition_digest,
            &proof_statement,
            &projection,
            outbox_reservation,
            prepared_one_use_authorization_digest,
            &sealed_transition_inputs,
            &sealed_recovery_seeds,
            normalized_guard_statement_digest,
        )?;
        let prepared = Self {
            version: KAGEMUSHA_STATE_VERSION_V1,
            preparation_id,
            predecessor_state,
            successor_state,
            state_transition_digest,
            proof_statement,
            projection,
            outbox_reservation,
            prepared_one_use_authorization_digest,
            sealed_transition_inputs,
            sealed_recovery_seeds,
            normalized_guard_statement_digest,
        };
        if prepared.canonical_storage_bytes()? > PREPARED_OUTGOING_INTENT_MAX_BYTES_V1
            || prepared.canonical_storage_bytes()?
                > u64::from(prepared.outbox_reservation.reserved_outbox_bytes)
        {
            return Err(KagemushaStateErrorV1::InvalidRecoveryMaterial);
        }
        Ok(prepared)
    }

    /// Return the direct public lifecycle bound by this private state transition.
    #[must_use]
    pub fn lifecycle(&self) -> &KagemushaLifecycleBindingV1 {
        self.projection.lifecycle()
    }

    /// Return the semantic digest shared by the state candidate and final commit proof.
    pub fn semantic_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.projection.semantic_digest()
    }

    /// Return the three digest carriers selected before either candidate proof is generated.
    ///
    /// The carrier gains authority only after the terminal proof opens its pre-proof transcript
    /// and exact sealed streams against the recursively verified State candidate.
    #[must_use]
    pub(crate) fn prepared_intent_commitments(&self) -> KagemushaPreparedIntentCommitmentsV1 {
        KagemushaPreparedIntentCommitmentsV1 {
            preparation_id: self.preparation_id,
            sealed_transition_inputs_digest: digest_raw_bytes(
                SEALED_TRANSITION_DIGEST_DOMAIN_V1,
                &self.sealed_transition_inputs,
            ),
            sealed_recovery_seeds_digest: digest_raw_bytes(
                SEALED_RECOVERY_DIGEST_DOMAIN_V1,
                &self.sealed_recovery_seeds,
            ),
        }
    }

    /// Borrow the derived compact payment output when this is a `SendSplit` candidate.
    #[must_use]
    pub fn send_output(&self) -> Option<&KagemushaPaymentOutputV1> {
        match &self.projection {
            PreparedPublicProjectionV1::Send(projection) => Some(&projection.output),
            PreparedPublicProjectionV1::Redemption(_) => None,
        }
    }

    /// Borrow the derived public redemption statement when this is a `RedeemSplit` candidate.
    #[must_use]
    pub fn redemption_statement(&self) -> Option<&KagemushaRedemptionStatementV1> {
        match &self.projection {
            PreparedPublicProjectionV1::Send(_) => None,
            PreparedPublicProjectionV1::Redemption(projection) => Some(&projection.statement),
        }
    }

    /// Borrow sender-local aggregate heads for proving and recovery only.
    #[must_use]
    pub(crate) fn private_state_link(&self) -> (&KagemushaStateV1, &KagemushaStateV1) {
        (&self.predecessor_state, &self.successor_state)
    }

    pub(super) fn validate_recipient_against_release(
        &self,
        proof_release: &KagemushaStateProofReleaseV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        proof_release.validate_state_context(self.predecessor_state.context())?;
        if let PreparedPublicProjectionV1::Send(projection) = &self.projection {
            proof_release.validate_payment_request(&self.predecessor_state, &projection.request)?;
        } else if let PreparedPublicProjectionV1::Redemption(projection) = &self.projection
            && projection.artifact_manifest_digest
                != proof_release.artifacts.artifact_manifest_digest
        {
            return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
        }
        Ok(())
    }

    /// Reconstruct the exact hardware statement committed after this intent is durable.
    pub fn hardware_statement(&self) -> HardwareTransitionStatementV1 {
        HardwareTransitionStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            kind: self.proof_statement.kind,
            amount: self.proof_statement.amount,
            lane: self.predecessor_state.lane.clone(),
            predecessor_commitment: self.predecessor_state.state_commitment,
            successor_commitment: self.successor_state.state_commitment,
            predecessor_sequence: self.predecessor_state.logical_sequence,
            successor_sequence: self.successor_state.logical_sequence,
            predecessor_epoch: self.predecessor_state.hardware_epoch,
            successor_epoch: self.successor_state.hardware_epoch,
            predecessor_device_policy_binding: self.predecessor_state.device_policy_binding,
            successor_device_policy_binding: self.successor_state.device_policy_binding,
            predecessor_state_nonce_commitment: self.predecessor_state.state_nonce_commitment,
            successor_state_nonce_commitment: self.successor_state.state_nonce_commitment,
            journal_revision_before: self.proof_statement.journal_revision_before,
            journal_revision_after: self.proof_statement.journal_revision_after,
            state_transition_digest: self.state_transition_digest,
            normalized_guard_statement_digest: self.normalized_guard_statement_digest,
        }
    }

    pub(crate) fn candidate_public_inputs(
        &self,
        artifacts: KagemushaRecursionArtifactsV1,
        proof: &KagemushaPairedProofV1,
    ) -> Result<KagemushaStateRelationPublicInputsV1, String> {
        let statement = &self.proof_statement;
        Ok(KagemushaStateRelationPublicInputsV1 {
            operation: statement.kind.into(),
            predecessor: Some(self.predecessor_state.clone()),
            successor: self.successor_state.clone(),
            amount: statement.amount,
            journal_revision_before: statement.journal_revision_before,
            journal_revision_after: statement.journal_revision_after,
            transition_effect_digest: statement.effect_digest,
            mint_finality_semantic_digest: statement.mint_finality_semantic_digest,
            mint_finality_proof_binding_digest: statement.mint_finality_proof_binding_digest,
            peer_credit_id: statement.peer_credit_id,
            recipient_encryption_key_binding: statement.recipient_encryption_key_binding,
            lifecycle_binding_digest: statement.lifecycle_binding_digest,
            prepared_transition_binding_digest: statement.prepared_transition_binding_digest,
            prepared_intent: Some(self.prepared_intent_commitments()),
            receive_credit_binding_digest: statement.receive_credit_binding_digest,
            transport_semantic_digest: self.semantic_digest().map_err(|error| error.to_string())?,
            guard_statement_digest: self.normalized_guard_statement_digest,
            eq_protocol_digest: artifacts.eq_protocol_digest,
            ep_protocol_digest: artifacts.ep_protocol_digest,
            guard_eq_protocol_digest: artifacts
                .guard_bundle_protocol_digest(KagemushaPastaParityV1::Eq)
                .map_err(|error| error.to_string())?,
            guard_ep_protocol_digest: artifacts
                .guard_bundle_protocol_digest(KagemushaPastaParityV1::Ep)
                .map_err(|error| error.to_string())?,
            mint_eq_protocol_digest: artifacts
                .mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)
                .map_err(|error| error.to_string())?,
            mint_ep_protocol_digest: artifacts
                .mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)
                .map_err(|error| error.to_string())?,
            mint_authorization_eq_protocol_digest: artifacts.mint_authorization_eq_protocol_digest,
            mint_authorization_ep_protocol_digest: artifacts.mint_authorization_ep_protocol_digest,
            commit_wrapper_eq_protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
            commit_wrapper_ep_protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
            guard_eq_credential_audit: proof.guard_eq_credential_audit,
            guard_ep_credential_audit: proof.guard_ep_credential_audit,
            eq_deferred_audit: proof.eq_deferred_audit,
            ep_deferred_audit: proof.ep_deferred_audit,
        })
    }

    fn canonical_storage_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        canonical_len(self)
    }

    pub(super) fn validate_recovered(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.version != KAGEMUSHA_STATE_VERSION_V1 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let amount = match &self.projection {
            PreparedPublicProjectionV1::Send(projection) => projection.request.amount,
            PreparedPublicProjectionV1::Redemption(projection) => projection.statement.amount,
        };
        validate_private_state_link(
            &self.predecessor_state,
            &self.successor_state,
            amount,
            self.projection.operation(),
        )?;
        let reconstructed = Self::new(
            self.predecessor_state.clone(),
            self.successor_state.clone(),
            self.state_transition_digest,
            self.proof_statement.clone(),
            self.projection.clone(),
            self.outbox_reservation,
            self.prepared_one_use_authorization_digest,
            self.sealed_transition_inputs.clone(),
            self.sealed_recovery_seeds.clone(),
            self.normalized_guard_statement_digest,
        )?;
        if reconstructed != *self {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PersistedOutgoingProofAuthorityV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
enum PersistedOutgoingProofAuthorityV1 {
    Send(KagemushaPairedProofV1),
    Redemption(KagemushaPairedProofV1),
}

/// Durably verified outgoing proof authority. Hardware may consume the predecessor only after
/// this exact record, including its canonical candidate digest, is installed.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PersistedOutgoingCandidateV1"
)]
pub struct PersistedOutgoingCandidateV1 {
    /// Immutable prepared operation and sealed recovery material.
    pub prepared: PreparedOutgoingCandidateV1,
    /// Private paired state candidate proof verified before either monetary commitment.
    proof_authority: PersistedOutgoingProofAuthorityV1,
    /// Canonical parity-invariant candidate public-input digest bound by terminal hardware.
    pub candidate_envelope_digest: DigestV1,
}

/// Read-only public operation material and exact persisted private state-candidate proof.
///
/// The proof is sender-local recovery material, not a replacement message or proof of hardware
/// unsealing. This view carries no authority and performs no proof regeneration or allocation.
/// Copying the view copies references only, never the underlying proof or recovery secrets.
///
/// The original candidate proof cannot be modified through the view:
///
/// ```compile_fail
/// use iroha_core::zk::kagemusha_v1_state::PersistedOutgoingRecoveryViewV1;
/// fn replace_candidate(view: PersistedOutgoingRecoveryViewV1<'_>) {
///     view.candidate_proof.eq_proof.clear();
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistedOutgoingRecoveryViewV1<'a> {
    /// Original operation-specific material, with send and redemption kept distinct.
    pub prepared: PreparedOutgoingRecoveryViewV1<'a>,
    /// Exact paired private state-candidate proof persisted before hardware commitment.
    pub candidate_proof: &'a KagemushaPairedProofV1,
    /// Original candidate public-input digest bound by the terminal hardware certificate.
    pub candidate_envelope_digest: &'a DigestV1,
}

impl PersistedOutgoingCandidateV1 {
    /// Borrow the exact persisted candidate and operation-specific public inputs.
    ///
    /// The operation tag must agree with the stored proof variant. This consistency check is
    /// not proof verification or authenticated snapshot recovery; callers must still use the
    /// normal verified journal restore and hardware recovery paths.
    ///
    /// # Errors
    ///
    /// Rejects a record whose prepared operation and persisted proof variant disagree.
    pub fn recovery_view(
        &self,
    ) -> Result<PersistedOutgoingRecoveryViewV1<'_>, KagemushaStateErrorV1> {
        let candidate_proof = match (&self.prepared.projection, &self.proof_authority) {
            (
                PreparedPublicProjectionV1::Send(_),
                PersistedOutgoingProofAuthorityV1::Send(proof),
            )
            | (
                PreparedPublicProjectionV1::Redemption(_),
                PersistedOutgoingProofAuthorityV1::Redemption(proof),
            ) => proof,
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        };
        Ok(PersistedOutgoingRecoveryViewV1 {
            prepared: self.prepared.recovery_view(),
            candidate_proof,
            candidate_envelope_digest: &self.candidate_envelope_digest,
        })
    }

    /// Verify and persist the request-bound private sender state candidate before commitment.
    pub fn verify_and_persist_send<R: KagemushaRecursiveVerifierV1>(
        prepared: PreparedOutgoingCandidateV1,
        candidate_proof: KagemushaPairedProofV1,
        artifacts: KagemushaRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<Self, KagemushaStateErrorV1> {
        if !matches!(&prepared.projection, PreparedPublicProjectionV1::Send(_)) {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let public_inputs = prepared
            .candidate_public_inputs(artifacts, &candidate_proof)
            .map_err(KagemushaStateErrorV1::ProofRejected)?;
        verify_kagemusha_state_proof_v1(verifier, artifacts, &public_inputs, &candidate_proof)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let candidate_envelope_digest = kagemusha_candidate_envelope_digest_v1(&public_inputs)
            .map_err(KagemushaStateErrorV1::ProofRejected)?;
        Self::materialize(
            prepared,
            PersistedOutgoingProofAuthorityV1::Send(candidate_proof),
            candidate_envelope_digest,
        )
    }

    /// Verify and persist one private redemption candidate proof before hardware commitment.
    pub fn verify_and_persist_redemption<R: KagemushaRecursiveVerifierV1>(
        prepared: PreparedOutgoingCandidateV1,
        candidate_proof: KagemushaPairedProofV1,
        artifacts: KagemushaRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<Self, KagemushaStateErrorV1> {
        if !matches!(
            &prepared.projection,
            PreparedPublicProjectionV1::Redemption(_)
        ) {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let public_inputs = prepared
            .candidate_public_inputs(artifacts, &candidate_proof)
            .map_err(KagemushaStateErrorV1::ProofRejected)?;
        verify_kagemusha_state_proof_v1(verifier, artifacts, &public_inputs, &candidate_proof)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let candidate_envelope_digest = kagemusha_candidate_envelope_digest_v1(&public_inputs)
            .map_err(KagemushaStateErrorV1::ProofRejected)?;
        Self::materialize(
            prepared,
            PersistedOutgoingProofAuthorityV1::Redemption(candidate_proof),
            candidate_envelope_digest,
        )
    }

    fn materialize(
        prepared: PreparedOutgoingCandidateV1,
        proof_authority: PersistedOutgoingProofAuthorityV1,
        candidate_envelope_digest: DigestV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let candidate = Self {
            prepared,
            proof_authority,
            candidate_envelope_digest,
        };
        if candidate.candidate_envelope_digest == [0; 32]
            || candidate.canonical_storage_bytes()?
                > u64::from(candidate.prepared.outbox_reservation.reserved_outbox_bytes)
        {
            return Err(KagemushaStateErrorV1::InvalidRecoveryMaterial);
        }
        Ok(candidate)
    }

    /// Verify a signed hardware selection against this exact locally verified candidate.
    ///
    /// This prepares a digest for a future foldable proof. It does not authorize an outgoing
    /// monetary commit: the physical secure-index semantics and recursive signature verification
    /// still require release-pinned platform and proof qualification. The expected app binding
    /// is reconstructed from the authenticated app policy and release, and the consumed secure
    /// index from the committed predecessor state, not from caller-supplied values.
    /// Testnet experiments may continue through their separately identified non-hardware corridor.
    pub fn verify_signed_hardware_selection_for_proof(
        &self,
        selection: &KagemushaSignedHardwareTransitionSelectionV1,
        credential: &KagemushaHardwareCredentialV1,
        release: &KagemushaAuthenticatedReleaseV1,
        app_policy: &KagemushaAppAttestationAuthorityPolicyV1,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        let (profile, expected) =
            self.hardware_selection_context_for_proof(credential, release, app_policy)?;
        selection
            .verify_against(credential, &profile, app_policy, expected)
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)
    }

    /// Verify an App Attest assertion against this exact locally verified candidate.
    ///
    /// This returns evidence for a future recursive fold only. The original assertion
    /// CBOR is parsed and its signature checked here; the result explicitly says whether
    /// signed app-release extensions were present. iOS 26's extension-free assertion does not
    /// independently measure the app version. The complete Apple enrollment attestation must
    /// also pass the release-pinned platform verifier. This check cannot authorize an outgoing
    /// monetary commit by itself.
    pub fn verify_app_attest_hardware_selection_for_proof(
        &self,
        selection: &KagemushaAppAttestHardwareTransitionSelectionV1,
        credential: &KagemushaHardwareCredentialV1,
        release: &KagemushaAuthenticatedReleaseV1,
        app_policy: &KagemushaAppAttestationAuthorityPolicyV1,
    ) -> Result<KagemushaVerifiedAppAttestSelectionV1, KagemushaStateErrorV1> {
        let (profile, expected) =
            self.hardware_selection_context_for_proof(credential, release, app_policy)?;
        selection
            .verify_signature_and_counter_against(credential, &profile, expected, app_policy)
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)
    }

    fn hardware_selection_context_for_proof(
        &self,
        credential: &KagemushaHardwareCredentialV1,
        release: &KagemushaAuthenticatedReleaseV1,
        app_policy: &KagemushaAppAttestationAuthorityPolicyV1,
    ) -> Result<
        (
            KagemushaHardwareProfileV1,
            KagemushaHardwareTransitionSelectionExpectedV1,
        ),
        KagemushaStateErrorV1,
    > {
        let prepared = &self.prepared;
        let predecessor = &prepared.predecessor_state;
        if predecessor.secure_index.checked_add(1) != Some(prepared.successor_state.secure_index) {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let enabled = release
            .enabled_profile(prepared.lifecycle().hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        credential
            .validate_app_policy_binding_for_release(
                &enabled.hardware_profile,
                release.release_id(),
                app_policy,
            )
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        if release.release_id() != prepared.lifecycle().release_id
            || enabled.suite_id != credential.suite_id
            || credential.device_key_reference
                != predecessor.device_policy_binding.device_key_reference
            || predecessor.device_policy_binding.hardware_policy_id
                != release.provider_policy_root()
            || credential.hardware_profile_id != predecessor.hardware_profile_id
            || credential.policy_epoch != predecessor.policy_epoch
            || credential.network_id != prepared.proof_statement.lane.network_id
            || credential.lane_commitment != prepared.proof_statement.lane.device_lane_id
            || u128::from(credential.hardware_epoch_generation)
                != predecessor.hardware_epoch.generation
            || credential.hardware_epoch_id != predecessor.hardware_epoch.epoch_id
        {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let expected = KagemushaHardwareTransitionSelectionExpectedV1 {
            release_id: release.release_id(),
            provider_policy_root: release.provider_policy_root(),
            app_policy_digest: credential.app_policy_binding_digest,
            operation_kind: prepared.lifecycle().operation_kind,
            transition_statement_digest: prepared.proof_statement.digest()?,
            candidate_envelope_digest: self.candidate_envelope_digest,
            terminal_body_commitment: self
                .hardware_terminal_body()?
                .canonical_commitment()
                .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?,
            secure_index_before: predecessor.secure_index,
        };
        Ok((enabled.hardware_profile, expected))
    }

    /// Build the exact self-free terminal body which qualified hardware must commit atomically.
    ///
    /// The three private commitments bind the verified successor, rollback-resistant journal
    /// entry, and sealed recovery material without exposing predecessor/successor state links in
    /// the eventual payment or redemption envelope.
    pub fn hardware_terminal_body(
        &self,
    ) -> Result<KagemushaHardwareTerminalBodyV1, KagemushaStateErrorV1> {
        let prepared = &self.prepared;
        let (commit_evidence, transition_nullifier) = match &prepared.projection {
            PreparedPublicProjectionV1::Send(projection) => (
                projection.output.commit_evidence,
                projection.output.transition_nullifier,
            ),
            PreparedPublicProjectionV1::Redemption(projection) => (
                projection.statement.commit_evidence,
                projection.statement.terminal_nullifier,
            ),
        };
        let lifecycle_binding_digest = prepared
            .lifecycle()
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        let outbox_reservation_commitment =
            validate_outbox_reservation(prepared.outbox_reservation)?;
        let private_journal_commitment = terminal_journal_commitment_v1(
            prepared.preparation_id,
            self.candidate_envelope_digest,
            prepared.state_transition_digest,
            outbox_reservation_commitment,
            prepared.proof_statement.journal_revision_after,
        )?;
        let private_recovery_commitment = terminal_recovery_commitment_v1(
            prepared.preparation_id,
            prepared.prepared_one_use_authorization_digest,
            &prepared.sealed_transition_inputs,
            &prepared.sealed_recovery_seeds,
        )?;
        Ok(KagemushaHardwareTerminalBodyV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            candidate_envelope_digest: self.candidate_envelope_digest,
            lifecycle_binding_digest,
            transition_nullifier,
            outbox_reservation_commitment,
            commit_evidence,
            hardware_profile_id: prepared.lifecycle().hardware_profile_id,
            policy_epoch: prepared.lifecycle().policy_epoch,
            private_successor_commitment: prepared.successor_state.state_commitment,
            private_journal_commitment,
            private_recovery_commitment,
        })
    }

    fn canonical_storage_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        canonical_len(self)
    }

    fn validate_recovered<R: KagemushaRecursiveVerifierV1>(
        &self,
        artifacts: KagemushaRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.prepared.validate_recovered()?;
        let reconstructed = match &self.proof_authority {
            PersistedOutgoingProofAuthorityV1::Send(proof) => Self::verify_and_persist_send(
                self.prepared.clone(),
                proof.clone(),
                artifacts,
                verifier,
            )?,
            PersistedOutgoingProofAuthorityV1::Redemption(proof) => {
                Self::verify_and_persist_redemption(
                    self.prepared.clone(),
                    proof.clone(),
                    artifacts,
                    verifier,
                )?
            }
        };
        if reconstructed != *self {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

/// Persisted candidate plus the recoverable terminal certificate returned by atomic hardware
/// commit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::CommittedOutgoingCandidateV1"
)]
pub struct CommittedOutgoingCandidateV1 {
    /// Candidate that was durably verified before the predecessor was consumed.
    pub candidate: PersistedOutgoingCandidateV1,
    /// Atomic, recoverable terminal commit certificate.
    pub commit_certificate: KagemushaCommitCertificateV1,
    /// Canonical digest consumed by the redemption proof.
    pub commit_certificate_digest: DigestV1,
}

impl CommittedOutgoingCandidateV1 {
    /// Bind one recovered hardware certificate to the sole persisted candidate.
    pub fn from_hardware_commit(
        candidate: PersistedOutgoingCandidateV1,
        commit_certificate: KagemushaCommitCertificateV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let prepared = &candidate.prepared;
        let (evidence, nullifier) = match &prepared.projection {
            PreparedPublicProjectionV1::Send(projection) => (
                projection.output.commit_evidence,
                projection.output.transition_nullifier,
            ),
            PreparedPublicProjectionV1::Redemption(projection) => (
                projection.statement.commit_evidence,
                projection.statement.terminal_nullifier,
            ),
        };
        commit_certificate
            .validate_against(prepared.lifecycle(), evidence, nullifier)
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        let reservation_commitment = validate_outbox_reservation(prepared.outbox_reservation)?;
        let expected_terminal_commitment = candidate
            .hardware_terminal_body()?
            .canonical_commitment()
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        if commit_certificate.candidate_envelope_digest != candidate.candidate_envelope_digest
            || commit_certificate.outbox_reservation_commitment != reservation_commitment
            || commit_certificate.hardware_terminal_commitment != expected_terminal_commitment
        {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let commit_certificate_digest = commit_certificate
            .canonical_digest_against(prepared.lifecycle(), evidence, nullifier)
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        Ok(Self {
            candidate,
            commit_certificate,
            commit_certificate_digest,
        })
    }

    /// Return the six exact SHA messages needed to open a retained outgoing preparation.
    ///
    /// This is producer material only. The terminal circuit must assign these bytes, SHA-open
    /// them against the recursively verified State carriers, and prove the complete 32-job Eq/Ep
    /// claim before they can authorize an outgoing payment or redemption. In particular, this
    /// method does not verify the persisted candidate proof or grant a hardware commitment.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "native committed-candidate export is not yet wired into the coordinator"
        )
    )]
    pub(crate) fn canonical_outgoing_opening_sha_messages_v1(
        &self,
    ) -> Result<[Vec<u8>; 6], KagemushaStateErrorV1> {
        use sha2::{Digest as _, Sha256};

        let prepared = &self.candidate.prepared;
        prepared.validate_recovered()?;
        let _ = self.candidate.recovery_view()?;
        if Self::from_hardware_commit(self.candidate.clone(), self.commit_certificate.clone())?
            != *self
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let (operation_tag, request_digest, artifact_manifest_digest) = match &prepared.projection {
            PreparedPublicProjectionV1::Send(projection) => (
                2,
                projection
                    .request
                    .canonical_digest()
                    .map_err(|_| KagemushaStateErrorV1::InvalidPaymentRequest)?,
                [0; 32],
            ),
            PreparedPublicProjectionV1::Redemption(projection) => {
                (4, [0; 32], projection.artifact_manifest_digest)
            }
        };
        let body = self.candidate.hardware_terminal_body()?;
        let digest = |message: &[u8]| -> DigestV1 { Sha256::digest(message).into() };
        let sealed_message =
            |domain: &[u8], bytes: &[u8]| -> Result<Vec<u8>, KagemushaStateErrorV1> {
                let length = u64::try_from(bytes.len())
                    .map_err(|_| KagemushaStateErrorV1::InvalidRecoveryMaterial)?;
                let mut message = Vec::with_capacity(domain.len() + 1 + 8 + bytes.len());
                message.extend_from_slice(domain);
                message.push(0);
                message.extend_from_slice(&length.to_le_bytes());
                message.extend_from_slice(bytes);
                Ok(message)
            };
        let transition = sealed_message(
            SEALED_TRANSITION_DIGEST_DOMAIN_V1,
            &prepared.sealed_transition_inputs,
        )?;
        let seeds = sealed_message(
            SEALED_RECOVERY_DIGEST_DOMAIN_V1,
            &prepared.sealed_recovery_seeds,
        )?;
        let transition_digest = digest(&transition);
        let seeds_digest = digest(&seeds);
        let lifecycle_digest = prepared
            .projection
            .lifecycle()
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidCandidateStage)?;
        let reservation_digest = validate_outbox_reservation(prepared.outbox_reservation)?;
        let fields = PreparationIdCommitmentsV1 {
            operation_tag,
            predecessor_state_commitment: prepared.predecessor_state.state_commitment,
            successor_state_commitment: prepared.successor_state.state_commitment,
            state_transition_digest: prepared.state_transition_digest,
            prepared_transition_binding_digest: prepared
                .proof_statement
                .prepared_transition_binding_digest,
            projection_semantic_digest: prepared.projection.semantic_digest()?,
            lifecycle_binding_digest: lifecycle_digest,
            request_digest,
            artifact_manifest_digest,
            normalized_guard_statement_digest: prepared.normalized_guard_statement_digest,
            outbox_reservation_commitment: reservation_digest,
            prepared_one_use_authorization_digest: prepared.prepared_one_use_authorization_digest,
            sealed_transition_inputs_len: u64::try_from(prepared.sealed_transition_inputs.len())
                .map_err(|_| KagemushaStateErrorV1::InvalidRecoveryMaterial)?,
            sealed_transition_inputs_digest: transition_digest,
            sealed_recovery_seeds_len: u64::try_from(prepared.sealed_recovery_seeds.len())
                .map_err(|_| KagemushaStateErrorV1::InvalidRecoveryMaterial)?,
            sealed_recovery_seeds_digest: seeds_digest,
        };
        let mut preparation = Vec::with_capacity(475);
        preparation.extend_from_slice(PREPARATION_ID_DOMAIN_V1);
        preparation.push(0);
        preparation.extend_from_slice(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
        preparation.push(fields.operation_tag);
        for value in [
            fields.predecessor_state_commitment,
            fields.successor_state_commitment,
            fields.state_transition_digest,
            fields.prepared_transition_binding_digest,
            fields.projection_semantic_digest,
            fields.lifecycle_binding_digest,
            fields.request_digest,
            fields.artifact_manifest_digest,
            fields.normalized_guard_statement_digest,
            fields.outbox_reservation_commitment,
            fields.prepared_one_use_authorization_digest,
        ] {
            preparation.extend_from_slice(&value);
        }
        preparation.extend_from_slice(&fields.sealed_transition_inputs_len.to_le_bytes());
        preparation.extend_from_slice(&fields.sealed_transition_inputs_digest);
        preparation.extend_from_slice(&fields.sealed_recovery_seeds_len.to_le_bytes());
        preparation.extend_from_slice(&fields.sealed_recovery_seeds_digest);
        if preparation.len() != 475
            || digest(&preparation) != prepared.preparation_id
            || hash_preparation_id_commitments_v1(fields) != prepared.preparation_id
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let framed = |domain: &[u8], frame: &[u8]| -> Result<Vec<u8>, KagemushaStateErrorV1> {
            let domain_len = u64::try_from(domain.len())
                .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
            let frame_len =
                u64::try_from(frame.len()).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
            let mut message = Vec::with_capacity(16 + domain.len() + frame.len());
            message.extend_from_slice(&domain_len.to_be_bytes());
            message.extend_from_slice(domain);
            message.extend_from_slice(&frame_len.to_be_bytes());
            message.extend_from_slice(frame);
            Ok(message)
        };
        let journal_frame = norito::encode_canonical(&TerminalJournalCommitmentPreimageV1 {
            preparation_id: prepared.preparation_id,
            candidate_envelope_digest: self.candidate.candidate_envelope_digest,
            state_transition_digest: prepared.state_transition_digest,
            outbox_reservation_commitment: reservation_digest,
            journal_revision_after: prepared.proof_statement.journal_revision_after,
        })
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
        let journal = framed(TERMINAL_JOURNAL_COMMITMENT_DOMAIN_V1, &journal_frame)?;
        let recovery_frame = norito::encode_canonical(&TerminalRecoveryCommitmentPreimageV1 {
            preparation_id: prepared.preparation_id,
            prepared_one_use_authorization_digest: prepared.prepared_one_use_authorization_digest,
            sealed_transition_inputs: prepared.sealed_transition_inputs.clone(),
            sealed_recovery_seeds: prepared.sealed_recovery_seeds.clone(),
        })
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
        let recovery = framed(TERMINAL_RECOVERY_COMMITMENT_DOMAIN_V1, &recovery_frame)?;
        let terminal_body_domain = b"iroha:kagemusha:v1:hardware-terminal-body";
        let body_bytes = body.commitment_preimage_bytes();
        let mut terminal_body =
            Vec::with_capacity(terminal_body_domain.len() + 1 + 8 + body_bytes.len());
        terminal_body.extend_from_slice(terminal_body_domain);
        terminal_body.push(0);
        terminal_body.extend_from_slice(
            &u64::try_from(body_bytes.len())
                .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?
                .to_le_bytes(),
        );
        terminal_body.extend_from_slice(&body_bytes);
        if digest(&journal) != body.private_journal_commitment
            || digest(&recovery) != body.private_recovery_commitment
            || digest(&terminal_body) != self.commit_certificate.hardware_terminal_commitment
        {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        Ok([
            transition,
            seeds,
            preparation,
            journal,
            recovery,
            terminal_body,
        ])
    }

    /// Return the exact unlinkable terminal output bound by the hardware certificate.
    pub fn public_output(&self) -> Result<KagemushaRecursivePublicOutputV1, KagemushaStateErrorV1> {
        let prepared = &self.candidate.prepared;
        let semantic_digest = prepared.semantic_digest()?;
        let candidate_digest = self.candidate.candidate_envelope_digest;
        let certificate_digest = self.commit_certificate_digest;
        let (
            transition_nullifier,
            request_digest,
            receiver_binding_digest,
            ciphertext_commitment,
            amount,
            terminal_binding,
        ) = match &prepared.projection {
            PreparedPublicProjectionV1::Send(projection) => {
                let output = &projection.output;
                let request_digest = projection
                    .request
                    .canonical_digest()
                    .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
                let receiver_binding_digest = projection.request.hardware_credential.credential_id;
                let amount = projection.request.amount;
                let output_digest = output
                    .canonical_digest_against(&projection.request)
                    .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
                let terminal_binding = canonical_terminal_send_output_binding_v1(
                    output.credit_id,
                    projection.request.recipient_encryption_key,
                    projection.request.hardware_credential.lane_commitment,
                    kagemusha_prepared_transfer_digest_v1(
                        &projection.request,
                        output.sender_before_commitment,
                        output.sender_after_commitment,
                        output.transition_nullifier,
                        output.ciphertext_commitment,
                    )
                    .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?,
                    output_digest,
                    canonical_incoming_payment_claims_binding_v1([
                        request_digest,
                        receiver_binding_digest,
                        canonical_sender_state_pair_digest_v1(
                            output.sender_before_commitment,
                            output.sender_after_commitment,
                        ),
                        output_digest,
                        kagemusha_ciphertext_digest_v1(&projection.encrypted_credit),
                        candidate_digest,
                        certificate_digest,
                    ]),
                );
                (
                    output.transition_nullifier,
                    request_digest,
                    receiver_binding_digest,
                    output.ciphertext_commitment,
                    amount,
                    terminal_binding,
                )
            }
            PreparedPublicProjectionV1::Redemption(projection) => (
                projection.statement.terminal_nullifier,
                [0; 32],
                [0; 32],
                [0; 32],
                projection.statement.amount,
                projection.statement.redemption_commitment,
            ),
        };
        KagemushaRecursivePublicOutputV1::new(
            prepared.lifecycle().clone(),
            semantic_digest,
            candidate_digest,
            certificate_digest,
            transition_nullifier,
            request_digest,
            receiver_binding_digest,
            ciphertext_commitment,
            amount,
            terminal_binding,
        )
        .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))
    }

    fn canonical_storage_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        canonical_len(self)
    }

    fn validate_recovered<R: KagemushaRecursiveVerifierV1>(
        &self,
        artifacts: KagemushaRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.candidate.validate_recovered(artifacts, verifier)?;
        let reconstructed =
            Self::from_hardware_commit(self.candidate.clone(), self.commit_certificate.clone())?;
        if reconstructed != *self {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

/// Final terminal wire envelope retained by the authenticated retry outbox.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::KagemushaOutgoingEnvelopeV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum KagemushaOutgoingEnvelopeV1 {
    /// Receiver-bound payment.
    Payment(KagemushaPaymentV1),
    /// Chain-facing redemption voucher.
    Redemption(KagemushaRedemptionVoucherV1),
}

impl KagemushaOutgoingEnvelopeV1 {
    fn canonical_bytes(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        match self {
            Self::Payment(payment) => norito::encode_canonical(payment),
            Self::Redemption(voucher) => norito::encode_canonical(voucher),
        }
        .map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)
    }
}

/// Complete terminal retry record installed before terminal bytes are exposed.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::DurableOutgoingEnvelopeV1"
)]
pub struct DurableOutgoingEnvelopeV1 {
    /// Private exact-once hardware commit retained for recovery.
    pub committed: CommittedOutgoingCandidateV1,
    /// Sole public payment or redemption envelope.
    pub envelope: KagemushaOutgoingEnvelopeV1,
    /// Exact canonical bytes returned by every retry.
    pub canonical_envelope_bytes: Vec<u8>,
    /// Domain-separated digest of those exact bytes.
    pub envelope_digest: DigestV1,
    /// Bounded authenticated transport retry metadata.
    pub retry_metadata: Vec<u8>,
}

impl DurableOutgoingEnvelopeV1 {
    /// Authenticate and install the compact terminal payment produced by committed sender hardware.
    pub fn finalize_payment<R: KagemushaRecursiveVerifierV1>(
        committed: CommittedOutgoingCandidateV1,
        payment: KagemushaPaymentV1,
        retry_metadata: Vec<u8>,
        artifacts: KagemushaRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<Self, KagemushaStateErrorV1> {
        Self::validate_retry_metadata(&retry_metadata)?;
        let PreparedPublicProjectionV1::Send(projection) = &committed.candidate.prepared.projection
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        payment
            .validate_shape_against(&projection.request)
            .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
        if payment.output != projection.output
            || payment.encrypted_credit != projection.encrypted_credit
            || payment.commit_certificate != committed.commit_certificate
            || payment.proof.candidate_envelope_digest
                != committed.candidate.candidate_envelope_digest
            || payment.proof.commit_certificate_digest != committed.commit_certificate_digest
            || payment.proof.semantic_digest != committed.candidate.prepared.semantic_digest()?
            || payment.proof.eq_protocol_digest != artifacts.commit_wrapper_eq_protocol_digest
            || payment.proof.ep_protocol_digest != artifacts.commit_wrapper_ep_protocol_digest
        {
            return Err(KagemushaStateErrorV1::InvalidPeerCredit);
        }
        verifier
            .verify_payment_and_decide(&projection.request, &payment)
            .map_err(KagemushaStateErrorV1::ProofRejected)?;
        Self::from_envelope(
            committed,
            KagemushaOutgoingEnvelopeV1::Payment(payment),
            retry_metadata,
        )
    }

    /// Verify and install one chain-facing redemption proof and voucher.
    pub fn finalize_redemption<R: KagemushaRecursiveVerifierV1>(
        committed: CommittedOutgoingCandidateV1,
        proof: KagemushaRedemptionProofV1,
        retry_metadata: Vec<u8>,
        artifacts: KagemushaRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<Self, KagemushaStateErrorV1> {
        Self::validate_retry_metadata(&retry_metadata)?;
        let PreparedPublicProjectionV1::Redemption(projection) =
            &committed.candidate.prepared.projection
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        let public_output = committed.public_output()?;
        let verified =
            verify_kagemusha_recursive_proof_v1(verifier, artifacts, public_output.clone(), &proof)
                .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        if verified.public_output() != public_output {
            return Err(KagemushaStateErrorV1::ProofRejected(
                "direct transition output substitution".to_owned(),
            ));
        }
        let voucher = KagemushaRedemptionVoucherV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement: projection.statement.clone(),
            commit_certificate: committed.commit_certificate.clone(),
            proof,
            artifact_manifest_digest: projection.artifact_manifest_digest,
        };
        voucher
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidRedemption)?;
        Self::from_envelope(
            committed,
            KagemushaOutgoingEnvelopeV1::Redemption(voucher),
            retry_metadata,
        )
    }

    fn validate_retry_metadata(retry_metadata: &[u8]) -> Result<(), KagemushaStateErrorV1> {
        if retry_metadata.len()
            > usize::try_from(KAGEMUSHA_OUTBOX_RETRY_METADATA_MAX_BYTES_V1).unwrap_or(usize::MAX)
        {
            return Err(KagemushaStateErrorV1::InvalidRecoveryMaterial);
        }
        Ok(())
    }

    fn from_envelope(
        committed: CommittedOutgoingCandidateV1,
        envelope: KagemushaOutgoingEnvelopeV1,
        retry_metadata: Vec<u8>,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let canonical_envelope_bytes = envelope.canonical_bytes()?;
        let maximum = match &envelope {
            KagemushaOutgoingEnvelopeV1::Payment(_) => KAGEMUSHA_PAYMENT_MAX_BYTES_V1,
            KagemushaOutgoingEnvelopeV1::Redemption(_) => KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1,
        };
        if canonical_envelope_bytes.len() > maximum {
            return Err(KagemushaStateErrorV1::InvalidProofBundle);
        }
        let envelope_digest = digest_raw_bytes(
            OUTGOING_ENVELOPE_DIGEST_DOMAIN_V1,
            &canonical_envelope_bytes,
        );
        let finalized = Self {
            committed,
            envelope,
            canonical_envelope_bytes,
            envelope_digest,
            retry_metadata,
        };
        finalized.validate_storage_bound()?;
        Ok(finalized)
    }

    /// Return the exact canonical bytes occupied by this durable retry record.
    pub(super) fn canonical_storage_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        canonical_len(self)
    }

    fn validate_storage_bound(&self) -> Result<(), KagemushaStateErrorV1> {
        let reservation = self.committed.candidate.prepared.outbox_reservation;
        let variant_matches_operation = matches!(
            (reservation.operation_kind, &self.envelope),
            (
                KagemushaOperationKindV1::SendSplit,
                KagemushaOutgoingEnvelopeV1::Payment(_)
            ) | (
                KagemushaOperationKindV1::RedeemSplit,
                KagemushaOutgoingEnvelopeV1::Redemption(_)
            )
        );
        if !variant_matches_operation
            || self.canonical_storage_bytes()? > u64::from(reservation.reserved_outbox_bytes)
        {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        Ok(())
    }

    /// Borrow the exact byte-identical terminal retry envelope.
    #[must_use]
    pub fn retry_bytes(&self) -> &[u8] {
        &self.canonical_envelope_bytes
    }

    /// Borrow the private successor installed at the hardware commit boundary.
    #[must_use]
    pub(crate) fn successor_state(&self) -> &KagemushaStateV1 {
        &self.committed.candidate.prepared.successor_state
    }

    fn validate_recovered<R>(
        &self,
        artifacts: KagemushaRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<(), KagemushaStateErrorV1>
    where
        R: KagemushaRecursiveVerifierV1,
    {
        self.validate_storage_bound()
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        self.committed.validate_recovered(artifacts, verifier)?;
        let reconstructed = match &self.envelope {
            KagemushaOutgoingEnvelopeV1::Payment(payment) => Self::finalize_payment(
                self.committed.clone(),
                payment.clone(),
                self.retry_metadata.clone(),
                artifacts,
                verifier,
            )?,
            KagemushaOutgoingEnvelopeV1::Redemption(voucher) => Self::finalize_redemption(
                self.committed.clone(),
                voucher.proof.clone(),
                self.retry_metadata.clone(),
                artifacts,
                verifier,
            )?,
        };
        if reconstructed != *self {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

/// Durable stage of the sole outgoing transition on one serialized monetary lane.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::KagemushaOutgoingJournalStageV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum KagemushaOutgoingJournalStageV1 {
    /// No active outgoing predecessor exists.
    Empty,
    /// Transition inputs and full terminal capacity are durable; proof generation may resume.
    Prepared(PreparedOutgoingCandidateV1),
    /// The private state candidate proof was verified and persisted; hardware may commit once.
    Candidate(PersistedOutgoingCandidateV1),
    /// Hardware consumed the predecessor; terminal envelope persistence may resume.
    Committed(CommittedOutgoingCandidateV1),
}

/// Recoverable prepare → proof authority → hardware commit → terminal envelope → exposure journal.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::KagemushaOutgoingCandidateJournalV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaOutgoingCandidateJournalV1 {
    stage: KagemushaOutgoingJournalStageV1,
    finalized_outbox: BTreeMap<DigestV1, DurableOutgoingEnvelopeV1>,
    released_envelopes: BTreeMap<DigestV1, DigestV1>,
    operation_index: KagemushaOutgoingOperationIndexV1,
}

impl Default for KagemushaOutgoingCandidateJournalV1 {
    fn default() -> Self {
        Self {
            stage: KagemushaOutgoingJournalStageV1::Empty,
            finalized_outbox: BTreeMap::new(),
            released_envelopes: BTreeMap::new(),
            operation_index: KagemushaOutgoingOperationIndexV1::default(),
        }
    }
}

impl KagemushaOutgoingCandidateJournalV1 {
    /// Borrow the current recoverable stage.
    #[must_use]
    pub const fn stage(&self) -> &KagemushaOutgoingJournalStageV1 {
        &self.stage
    }

    /// Borrow the snapshot-bound caller-operation recovery index.
    #[must_use]
    pub const fn operation_index(&self) -> &KagemushaOutgoingOperationIndexV1 {
        &self.operation_index
    }

    #[cfg(test)]
    pub(super) fn operation_index_mut_for_test(
        &mut self,
    ) -> &mut KagemushaOutgoingOperationIndexV1 {
        &mut self.operation_index
    }

    /// Stage one caller-indexed operation and its exact Core preparation atomically.
    pub(super) fn prepare_indexed(
        &mut self,
        operation_id: DigestV1,
        authenticated_credential_id: DigestV1,
        authenticated_core_authorization_key_reference: DigestV1,
        prepared: PreparedOutgoingCandidateV1,
    ) -> Result<KagemushaOutgoingOperationPrepareOutcomeV1, KagemushaStateErrorV1> {
        let (next_index, outcome) = self
            .operation_index
            .prepare_successor(
                operation_id,
                authenticated_credential_id,
                authenticated_core_authorization_key_reference,
                &prepared,
            )
            .map_err(map_operation_index_error)?;
        let mut next = self.clone();
        next.prepare(prepared)?;
        next.operation_index = next_index;
        *self = next;
        Ok(outcome)
    }

    /// Atomically stage an exact transition intent before hardware commit.
    fn prepare(
        &mut self,
        prepared: PreparedOutgoingCandidateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        match &self.stage {
            KagemushaOutgoingJournalStageV1::Empty => {
                self.stage = KagemushaOutgoingJournalStageV1::Prepared(prepared);
                Ok(())
            }
            KagemushaOutgoingJournalStageV1::Prepared(existing) if existing == &prepared => Ok(()),
            KagemushaOutgoingJournalStageV1::Prepared(_)
            | KagemushaOutgoingJournalStageV1::Candidate(_)
            | KagemushaOutgoingJournalStageV1::Committed(_) => {
                Err(KagemushaStateErrorV1::CandidateConflict)
            }
        }
    }

    /// Persist the sole verified operation proof authority before hardware may consume state.
    pub(super) fn persist_candidate(
        &mut self,
        candidate: PersistedOutgoingCandidateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        match &self.stage {
            KagemushaOutgoingJournalStageV1::Prepared(prepared)
                if prepared == &candidate.prepared =>
            {
                if candidate.canonical_storage_bytes()?
                    > u64::from(candidate.prepared.outbox_reservation.reserved_outbox_bytes)
                {
                    return Err(KagemushaStateErrorV1::StateInvariant);
                }
                let next_index = self
                    .operation_index
                    .candidate_successor(&candidate)
                    .map_err(map_operation_index_error)?;
                self.stage = KagemushaOutgoingJournalStageV1::Candidate(candidate);
                self.operation_index = next_index;
                Ok(())
            }
            KagemushaOutgoingJournalStageV1::Prepared(_) => {
                Err(KagemushaStateErrorV1::CandidateConflict)
            }
            _ => Err(KagemushaStateErrorV1::InvalidCandidateStage),
        }
    }

    /// Install the sole hardware commit; a second successor cannot be attached to the candidate.
    pub(super) fn commit(
        &mut self,
        committed: CommittedOutgoingCandidateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        match &self.stage {
            KagemushaOutgoingJournalStageV1::Candidate(candidate)
                if candidate == &committed.candidate =>
            {
                if committed.canonical_storage_bytes()?
                    > u64::from(
                        committed
                            .candidate
                            .prepared
                            .outbox_reservation
                            .reserved_outbox_bytes,
                    )
                {
                    return Err(KagemushaStateErrorV1::StateInvariant);
                }
                let next_index = self
                    .operation_index
                    .commit_successor(&committed)
                    .map_err(map_operation_index_error)?;
                self.stage = KagemushaOutgoingJournalStageV1::Committed(committed);
                self.operation_index = next_index;
                Ok(())
            }
            KagemushaOutgoingJournalStageV1::Candidate(_) => {
                Err(KagemushaStateErrorV1::CandidateConflict)
            }
            _ => Err(KagemushaStateErrorV1::InvalidCandidateStage),
        }
    }

    /// Persist a verified terminal envelope and clear only the active stage.
    pub(super) fn install_finalized(
        &mut self,
        finalized: DurableOutgoingEnvelopeV1,
        outbox: &mut KagemushaSenderOutboxCapacityV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let reservation = finalized.committed.candidate.prepared.outbox_reservation;
        let reservation_id = reservation.reservation_id;
        if let Some(existing) = self.finalized_outbox.get(&reservation_id) {
            return if existing == &finalized {
                Ok(())
            } else {
                Err(KagemushaStateErrorV1::CandidateConflict)
            };
        }
        match &self.stage {
            KagemushaOutgoingJournalStageV1::Committed(committed)
                if committed == &finalized.committed =>
            {
                let mut next = self.clone();
                let mut next_outbox = outbox.clone();
                next.operation_index = next
                    .operation_index
                    .install_successor(&finalized)
                    .map_err(map_operation_index_error)?;
                next_outbox.bind_terminal_envelope(reservation, finalized.envelope_digest)?;
                next.finalized_outbox.insert(reservation_id, finalized);
                next.stage = KagemushaOutgoingJournalStageV1::Empty;
                next_outbox.reconcile_capacity_meters(&next)?;
                *self = next;
                *outbox = next_outbox;
                Ok(())
            }
            KagemushaOutgoingJournalStageV1::Committed(_) => {
                Err(KagemushaStateErrorV1::CandidateConflict)
            }
            _ => Err(KagemushaStateErrorV1::InvalidCandidateStage),
        }
    }

    /// Return terminal bytes only after authorization and durable installation.
    pub fn expose(&self, reservation_id: DigestV1) -> Result<&[u8], KagemushaStateErrorV1> {
        self.finalized_outbox
            .get(&reservation_id)
            .map(DurableOutgoingEnvelopeV1::retry_bytes)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }

    /// Borrow one durable terminal envelope for retry or settlement processing.
    #[must_use]
    pub fn finalized_envelope(
        &self,
        reservation_id: DigestV1,
    ) -> Option<&DurableOutgoingEnvelopeV1> {
        self.finalized_outbox.get(&reservation_id)
    }

    /// Return the number of currently retained retry envelopes.
    #[must_use]
    pub fn finalized_outbox_count(&self) -> usize {
        self.finalized_outbox.len()
    }

    /// Verify a peer ACK and atomically retain the indexed release tombstone.
    pub(super) fn release_indexed_payment(
        &mut self,
        operation_id: DigestV1,
        acknowledgement_bytes: &[u8],
        outbox: &mut KagemushaSenderOutboxCapacityV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let record = self
            .operation_index
            .lookup(operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if record.phase == KagemushaOutgoingOperationPhaseV1::Released {
            return record
                .validate_released_acknowledgement_retry(acknowledgement_bytes)
                .map_err(map_operation_index_error);
        }
        let finalized = self
            .finalized_outbox
            .get(&record.outbox_reservation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let terminal_receipt_digest = record
            .verified_payment_terminal_receipt_digest(finalized, acknowledgement_bytes)
            .map_err(map_operation_index_error)?;
        let reservation_id = record.outbox_reservation_id;
        let envelope_digest = finalized.envelope_digest;
        let next_index = self
            .operation_index
            .release_successor(reservation_id, envelope_digest, terminal_receipt_digest)
            .map_err(map_operation_index_error)?;
        let mut next = self.clone();
        let mut next_outbox = outbox.clone();
        next.operation_index = next_index;
        next.release_finalized_inner(reservation_id, envelope_digest, &mut next_outbox)?;
        *self = next;
        *outbox = next_outbox;
        Ok(())
    }

    /// Consume a Core-authenticated settlement capability and retain the exact redemption
    /// release tombstone in the same successor which removes the retry envelope.
    pub(super) fn release_indexed_redemption(
        &mut self,
        verified: VerifiedKagemushaRedemptionReleaseV1,
        outbox: &mut KagemushaSenderOutboxCapacityV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let operation_id = verified.operation_id();
        let record = self
            .operation_index
            .lookup(operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if record.phase == KagemushaOutgoingOperationPhaseV1::Released {
            return verified.validate_against_record(record, None);
        }
        let finalized = self
            .finalized_outbox
            .get(&record.outbox_reservation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        verified.validate_against_record(record, Some(finalized))?;
        let reservation_id = record.outbox_reservation_id;
        let envelope_digest = verified.envelope_digest();
        let terminal_receipt_digest = verified.terminal_receipt_digest();
        let next_index = self
            .operation_index
            .release_successor(reservation_id, envelope_digest, terminal_receipt_digest)
            .map_err(map_operation_index_error)?;
        let mut next = self.clone();
        let mut next_outbox = outbox.clone();
        next.operation_index = next_index;
        next.release_finalized_inner(reservation_id, envelope_digest, &mut next_outbox)?;
        *self = next;
        *outbox = next_outbox;
        Ok(())
    }

    fn release_finalized_inner(
        &mut self,
        reservation_id: DigestV1,
        expected_envelope_digest: DigestV1,
        outbox: &mut KagemushaSenderOutboxCapacityV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if let Some(existing) = self.released_envelopes.get(&reservation_id) {
            if *existing != expected_envelope_digest {
                return Err(KagemushaStateErrorV1::CandidateConflict);
            }
            return Ok(());
        }
        let finalized = self
            .finalized_outbox
            .get(&reservation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if finalized.envelope_digest != expected_envelope_digest {
            return Err(KagemushaStateErrorV1::CandidateConflict);
        }
        let mut next = self.clone();
        let mut next_outbox = outbox.clone();
        next.finalized_outbox.remove(&reservation_id);
        next.released_envelopes
            .insert(reservation_id, expected_envelope_digest);
        next_outbox.mark_terminal_released(reservation_id, expected_envelope_digest)?;
        next_outbox.reconcile_capacity_meters(&next)?;
        *self = next;
        *outbox = next_outbox;
        Ok(())
    }

    pub(crate) fn validate_recovered<R>(
        &self,
        state: &KagemushaStateV1,
        journal_revision: u128,
        outbox: &KagemushaSenderOutboxCapacityV1,
        proof_release: &KagemushaStateProofReleaseV1,
        artifacts: KagemushaRecursionArtifactsV1,
        verifier: &R,
    ) -> Result<(), KagemushaStateErrorV1>
    where
        R: KagemushaRecursiveVerifierV1,
    {
        state.validate()?;
        self.operation_index
            .validate_recovered(state)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        outbox.validate_recovered(self)?;
        let active_reservation = match &self.stage {
            KagemushaOutgoingJournalStageV1::Empty => None,
            KagemushaOutgoingJournalStageV1::Prepared(prepared) => {
                prepared.validate_recovered()?;
                prepared.validate_recipient_against_release(proof_release)?;
                if &prepared.predecessor_state != state
                    || prepared.proof_statement.journal_revision_before != journal_revision
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                Some(prepared.outbox_reservation)
            }
            KagemushaOutgoingJournalStageV1::Candidate(candidate) => {
                candidate.validate_recovered(artifacts, verifier)?;
                candidate
                    .prepared
                    .validate_recipient_against_release(proof_release)?;
                if &candidate.prepared.predecessor_state != state
                    || candidate.prepared.proof_statement.journal_revision_before
                        != journal_revision
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                Some(candidate.prepared.outbox_reservation)
            }
            KagemushaOutgoingJournalStageV1::Committed(committed) => {
                committed.validate_recovered(artifacts, verifier)?;
                committed
                    .candidate
                    .prepared
                    .validate_recipient_against_release(proof_release)?;
                if &committed.candidate.prepared.successor_state != state
                    || committed
                        .candidate
                        .prepared
                        .proof_statement
                        .journal_revision_after
                        != journal_revision
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                Some(committed.candidate.prepared.outbox_reservation)
            }
        };
        if let Some(reservation) = active_reservation {
            outbox.validate_active_recovered_reservation(reservation)?;
        }
        for (reservation_id, finalized) in &self.finalized_outbox {
            finalized.validate_recovered(artifacts, verifier)?;
            finalized
                .committed
                .candidate
                .prepared
                .validate_recipient_against_release(proof_release)?;
            let reservation = finalized.committed.candidate.prepared.outbox_reservation;
            if *reservation_id != reservation.reservation_id
                || self.released_envelopes.contains_key(reservation_id)
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            outbox
                .validate_finalized_recovered_reservation(reservation, finalized.envelope_digest)?;
            validate_installed_successor_not_ahead(state, finalized.successor_state())?;
        }
        for (reservation_id, envelope_digest) in &self.released_envelopes {
            if *envelope_digest == [0; 32] || self.finalized_outbox.contains_key(reservation_id) {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            outbox.validate_released_recovered_reservation(*reservation_id, *envelope_digest)?;
        }
        for (reservation_id, record) in &outbox.reservations {
            let active = active_reservation
                .is_some_and(|reservation| reservation.reservation_id == *reservation_id);
            let finalized = self.finalized_outbox.contains_key(reservation_id);
            let released = self.released_envelopes.contains_key(reservation_id);
            if usize::from(active) + usize::from(finalized) + usize::from(released) != 1
                || record.released != released
                || record.terminal_envelope_digest.is_some() != (finalized || released)
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        for record in self.operation_index.records() {
            match record.phase {
                KagemushaOutgoingOperationPhaseV1::Prepared => {
                    let KagemushaOutgoingJournalStageV1::Prepared(prepared) = &self.stage else {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    };
                    record
                        .validate_against_prepared(prepared)
                        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
                }
                KagemushaOutgoingOperationPhaseV1::CandidatePersisted => {
                    let KagemushaOutgoingJournalStageV1::Candidate(candidate) = &self.stage else {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    };
                    record
                        .validate_against_prepared(&candidate.prepared)
                        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
                    if record.candidate_digest != Some(candidate.candidate_envelope_digest) {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                }
                KagemushaOutgoingOperationPhaseV1::Committed => {
                    let KagemushaOutgoingJournalStageV1::Committed(committed) = &self.stage else {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    };
                    record
                        .validate_against_prepared(&committed.candidate.prepared)
                        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
                    if record.candidate_digest
                        != Some(committed.candidate.candidate_envelope_digest)
                        || record.commit_certificate_digest
                            != Some(committed.commit_certificate_digest)
                    {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                }
                KagemushaOutgoingOperationPhaseV1::Installed => {
                    let Some(finalized) = self.finalized_outbox.get(&record.outbox_reservation_id)
                    else {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    };
                    record
                        .validate_against_prepared(&finalized.committed.candidate.prepared)
                        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
                    if record.candidate_digest
                        != Some(finalized.committed.candidate.candidate_envelope_digest)
                        || record.commit_certificate_digest
                            != Some(finalized.committed.commit_certificate_digest)
                        || record.envelope_digest != Some(finalized.envelope_digest)
                    {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                }
                KagemushaOutgoingOperationPhaseV1::Released => {
                    if self.released_envelopes.get(&record.outbox_reservation_id)
                        != record.envelope_digest.as_ref()
                    {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::SenderOutboxReservationRecordV1"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
struct SenderOutboxReservationRecordV1 {
    reservation: KagemushaOutboxReservationV1,
    reservation_commitment: DigestV1,
    terminal_envelope_digest: Option<DigestV1>,
    released: bool,
}

/// Sender-owned working-capacity ledger for live recoverable terminal operations.
/// Released bindings remain in durable history and telemetry, outside the live admission budget.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::KagemushaSenderOutboxCapacityV1"
)]
pub struct KagemushaSenderOutboxCapacityV1 {
    total_outbox_bytes: u64,
    committed_outbox_bytes: u64,
    retained_metadata_bytes: u64,
    reservations: BTreeMap<DigestV1, SenderOutboxReservationRecordV1>,
}

impl KagemushaSenderOutboxCapacityV1 {
    /// Create an empty sender-capacity ledger.
    #[must_use]
    pub const fn new(total_outbox_bytes: u64) -> Self {
        Self {
            total_outbox_bytes,
            committed_outbox_bytes: 0,
            retained_metadata_bytes: 0,
            reservations: BTreeMap::new(),
        }
    }

    /// Return physical bytes still available for staged transition intents.
    #[must_use]
    pub const fn available_outbox_bytes(&self) -> u64 {
        self.total_outbox_bytes
            .saturating_sub(self.committed_outbox_bytes)
    }

    /// Return the total locally provisioned sender outbox bytes.
    #[must_use]
    pub const fn total_outbox_bytes(&self) -> u64 {
        self.total_outbox_bytes
    }

    /// Return live payload reservations and live reservation/index metadata.
    #[must_use]
    pub const fn committed_outbox_bytes(&self) -> u64 {
        self.committed_outbox_bytes
    }

    /// Return exact retained reservation, release and operation-index metadata bytes.
    #[must_use]
    pub const fn retained_metadata_bytes(&self) -> u64 {
        self.retained_metadata_bytes
    }

    /// Reserve a complete intent, proof authority, hardware certificate, envelope, and retry slot.
    pub fn reserve(
        &mut self,
        reservation: KagemushaOutboxReservationV1,
        journal: &KagemushaOutgoingCandidateJournalV1,
    ) -> Result<SenderOutboxReservationOutcomeV1, KagemushaStateErrorV1> {
        let commitment = validate_outbox_reservation(reservation)?;
        if let Some(existing) = self.reservations.get(&reservation.reservation_id) {
            if existing.reservation != reservation
                || existing.reservation_commitment != commitment
                || existing.terminal_envelope_digest.is_some()
                || existing.released
            {
                return Err(KagemushaStateErrorV1::CandidateConflict);
            }
            self.validate_capacity_meters(journal, false)?;
            return Ok(SenderOutboxReservationOutcomeV1::AlreadyReserved);
        }
        let mut next = self.clone();
        next.reservations.insert(
            reservation.reservation_id,
            SenderOutboxReservationRecordV1 {
                reservation,
                reservation_commitment: commitment,
                terminal_envelope_digest: None,
                released: false,
            },
        );
        next.reconcile_capacity_meters(journal)?;
        *self = next;
        Ok(SenderOutboxReservationOutcomeV1::Reserved)
    }

    pub(super) fn require_reservation(
        &self,
        reservation: KagemushaOutboxReservationV1,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        let commitment = validate_outbox_reservation(reservation)?;
        let existing = self
            .reservations
            .get(&reservation.reservation_id)
            .ok_or(KagemushaStateErrorV1::SenderOutboxCapacityExhausted)?;
        if existing.reservation != reservation
            || existing.reservation_commitment != commitment
            || existing.terminal_envelope_digest.is_some()
            || existing.released
        {
            return Err(KagemushaStateErrorV1::CandidateConflict);
        }
        Ok(commitment)
    }

    fn bind_terminal_envelope(
        &mut self,
        reservation: KagemushaOutboxReservationV1,
        envelope_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let record = self
            .reservations
            .get_mut(&reservation.reservation_id)
            .ok_or(KagemushaStateErrorV1::SenderOutboxCapacityExhausted)?;
        if record.reservation != reservation || record.released {
            return Err(KagemushaStateErrorV1::CandidateConflict);
        }
        match record.terminal_envelope_digest {
            None => record.terminal_envelope_digest = Some(envelope_digest),
            Some(existing) if existing == envelope_digest => {}
            Some(_) => return Err(KagemushaStateErrorV1::CandidateConflict),
        }
        Ok(())
    }

    fn mark_terminal_released(
        &mut self,
        reservation_id: DigestV1,
        expected_envelope_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let record = self
            .reservations
            .get_mut(&reservation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if record.terminal_envelope_digest != Some(expected_envelope_digest) {
            return Err(KagemushaStateErrorV1::CandidateConflict);
        }
        record.released = true;
        Ok(())
    }

    pub(super) fn reconcile_capacity_meters(
        &mut self,
        journal: &KagemushaOutgoingCandidateJournalV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let (committed, metadata) = sender_outbox_capacity_meters_v1(self, journal)?;
        if committed > self.total_outbox_bytes {
            return Err(KagemushaStateErrorV1::SenderOutboxCapacityExhausted);
        }
        self.committed_outbox_bytes = committed;
        self.retained_metadata_bytes = metadata;
        Ok(())
    }

    fn validate_capacity_meters(
        &self,
        journal: &KagemushaOutgoingCandidateJournalV1,
        snapshot: bool,
    ) -> Result<(), KagemushaStateErrorV1> {
        let (committed, metadata) = sender_outbox_capacity_meters_v1(self, journal)?;
        if committed != self.committed_outbox_bytes
            || metadata != self.retained_metadata_bytes
            || committed > self.total_outbox_bytes
        {
            return Err(if snapshot {
                KagemushaStateErrorV1::SnapshotIntegrity
            } else {
                KagemushaStateErrorV1::StateInvariant
            });
        }
        Ok(())
    }

    pub(crate) fn validate_recovered(
        &self,
        journal: &KagemushaOutgoingCandidateJournalV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        for (reservation_id, record) in &self.reservations {
            if *reservation_id != record.reservation.reservation_id
                || record.reservation_commitment != validate_outbox_reservation(record.reservation)?
                || record.released && record.terminal_envelope_digest.is_none()
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        self.validate_capacity_meters(journal, true)
    }

    fn validate_active_recovered_reservation(
        &self,
        reservation: KagemushaOutboxReservationV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let Some(record) = self.reservations.get(&reservation.reservation_id) else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        if record.reservation != reservation
            || record.terminal_envelope_digest.is_some()
            || record.released
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn validate_finalized_recovered_reservation(
        &self,
        reservation: KagemushaOutboxReservationV1,
        envelope_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let Some(record) = self.reservations.get(&reservation.reservation_id) else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        if record.reservation != reservation
            || record.terminal_envelope_digest != Some(envelope_digest)
            || record.released
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn validate_released_recovered_reservation(
        &self,
        reservation_id: DigestV1,
        envelope_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let Some(record) = self.reservations.get(&reservation_id) else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        if record.terminal_envelope_digest != Some(envelope_digest) || !record.released {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::SenderCapacityMetadataProjectionV1"
)]
struct SenderCapacityMetadataProjectionV1 {
    reservations: BTreeMap<DigestV1, SenderOutboxReservationRecordV1>,
    released_envelopes: BTreeMap<DigestV1, DigestV1>,
}

fn sender_outbox_capacity_meters_v1(
    outbox: &KagemushaSenderOutboxCapacityV1,
    journal: &KagemushaOutgoingCandidateJournalV1,
) -> Result<(u64, u64), KagemushaStateErrorV1> {
    let encoded_metadata = canonical_len(&SenderCapacityMetadataProjectionV1 {
        reservations: outbox.reservations.clone(),
        released_envelopes: journal.released_envelopes.clone(),
    })?;
    let baseline_metadata = canonical_len(&SenderCapacityMetadataProjectionV1 {
        reservations: BTreeMap::new(),
        released_envelopes: BTreeMap::new(),
    })?;
    let retained_index_bytes = journal
        .operation_index
        .retained_record_bytes()
        .map_err(map_operation_index_error)?;
    let metadata = encoded_metadata
        .checked_sub(baseline_metadata)
        .and_then(|bytes| bytes.checked_add(retained_index_bytes))
        .ok_or(KagemushaStateErrorV1::StateInvariant)?;
    let live_metadata = canonical_len(&SenderCapacityMetadataProjectionV1 {
        reservations: outbox
            .reservations
            .iter()
            .filter(|(_, record)| !record.released)
            .map(|(id, record)| (*id, *record))
            .collect(),
        released_envelopes: BTreeMap::new(),
    })?
    .checked_sub(baseline_metadata)
    .and_then(|bytes| bytes.checked_add(journal.operation_index.reserved_bytes()))
    .ok_or(KagemushaStateErrorV1::StateInvariant)?;
    let live = outbox
        .reservations
        .values()
        .filter(|record| !record.released)
        .try_fold(0_u64, |total, record| {
            total
                .checked_add(u64::from(record.reservation.reserved_outbox_bytes))
                .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)
        })?;
    let committed = live_metadata
        .checked_add(live)
        .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
    Ok((committed, metadata))
}

#[allow(clippy::too_many_arguments)]
fn validate_prepared_transition_statement(
    predecessor: &KagemushaStateV1,
    successor: &KagemushaStateV1,
    state_transition_digest: DigestV1,
    statement: &TransitionProofStatementV1,
    projection: &PreparedPublicProjectionV1,
    outbox_reservation: KagemushaOutboxReservationV1,
    prepared_one_use_authorization_digest: DigestV1,
    normalized_guard_statement_digest: DigestV1,
) -> Result<(), KagemushaStateErrorV1> {
    let lifecycle_digest = projection
        .lifecycle()
        .canonical_digest()
        .map_err(|_| KagemushaStateErrorV1::InvalidCandidateStage)?;
    let reservation_commitment = validate_outbox_reservation(outbox_reservation)?;
    let (request_digest, sender_before_commitment, sender_after_commitment, amount) =
        match projection {
            PreparedPublicProjectionV1::Send(projection) => (
                projection
                    .request
                    .canonical_digest()
                    .map_err(|_| KagemushaStateErrorV1::InvalidPaymentRequest)?,
                projection.output.sender_before_commitment,
                projection.output.sender_after_commitment,
                projection.request.amount,
            ),
            PreparedPublicProjectionV1::Redemption(projection) => {
                ([0; 32], [0; 32], [0; 32], projection.statement.amount)
            }
        };
    let prepared_transition_binding_digest = canonical_prepared_transition_binding_digest_v1(
        lifecycle_digest,
        request_digest,
        sender_before_commitment,
        sender_after_commitment,
        amount,
        reservation_commitment,
        prepared_one_use_authorization_digest,
    );
    let expected_hardware_statement = HardwareTransitionStatementV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        kind: statement.kind,
        amount: statement.amount,
        lane: predecessor.lane.clone(),
        predecessor_commitment: predecessor.state_commitment,
        successor_commitment: successor.state_commitment,
        predecessor_sequence: predecessor.logical_sequence,
        successor_sequence: successor.logical_sequence,
        predecessor_epoch: predecessor.hardware_epoch,
        successor_epoch: successor.hardware_epoch,
        predecessor_device_policy_binding: predecessor.device_policy_binding,
        successor_device_policy_binding: successor.device_policy_binding,
        predecessor_state_nonce_commitment: predecessor.state_nonce_commitment,
        successor_state_nonce_commitment: successor.state_nonce_commitment,
        journal_revision_before: statement.journal_revision_before,
        journal_revision_after: statement.journal_revision_after,
        state_transition_digest,
        normalized_guard_statement_digest,
    };
    expected_hardware_statement.validate_exact_next()?;
    let common_invalid = statement.version != KAGEMUSHA_STATE_VERSION_V1
        || statement.protocol_version != predecessor.protocol_version
        || statement.predecessor_suite_id != predecessor.suite_id
        || statement.predecessor_vk_digest != predecessor.vk_digest
        || statement.successor_suite_id != successor.suite_id
        || statement.successor_vk_digest != successor.vk_digest
        || statement.predecessor_release_id != predecessor.release_id
        || statement.release_id != successor.release_id
        || statement.asset_incarnation != predecessor.asset_incarnation
        || statement.liability_pool_id != predecessor.liability_pool_id
        || statement.hardware_profile_id != predecessor.hardware_profile_id
        || statement.policy_epoch != predecessor.policy_epoch
        || statement.lane != predecessor.lane
        || statement.predecessor_commitment != predecessor.state_commitment
        || statement.successor_commitment != successor.state_commitment
        || statement.predecessor_sequence != predecessor.logical_sequence
        || statement.successor_sequence != successor.logical_sequence
        || statement.predecessor_epoch != predecessor.hardware_epoch
        || statement.successor_epoch != successor.hardware_epoch
        || statement.predecessor_device_policy_binding != predecessor.device_policy_binding
        || statement.successor_device_policy_binding != successor.device_policy_binding
        || statement.predecessor_state_nonce_commitment != predecessor.state_nonce_commitment
        || statement.successor_state_nonce_commitment != successor.state_nonce_commitment
        || statement.journal_revision_after
            != statement
                .journal_revision_before
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?
        || statement.effect_digest != prepared_transition_binding_digest
        || statement.lifecycle_binding_digest != lifecycle_digest
        || statement.prepared_transition_binding_digest != prepared_transition_binding_digest
        || statement.mint_finality_semantic_digest != [0; 32]
        || statement.mint_finality_proof_binding_digest != [0; 32]
        || statement.receive_credit_binding_digest != [0; 32]
        || normalized_guard_statement_digest == [0; 32]
        || statement.digest()? != state_transition_digest;
    if common_invalid {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    let operation_valid = match projection {
        PreparedPublicProjectionV1::Send(projection) => {
            statement.kind == super::KagemushaTransitionKindV1::SendSplit
                && statement.amount == projection.request.amount
                && statement.peer_credit_id == projection.output.credit_id
                && statement.recipient_encryption_key_binding
                    == projection.request.recipient_encryption_key
        }
        PreparedPublicProjectionV1::Redemption(projection) => {
            statement.kind == super::KagemushaTransitionKindV1::RedeemSplit
                && statement.amount == projection.statement.amount
                && statement.peer_credit_id == [0; 32]
                && statement.recipient_encryption_key_binding == [0; 32]
        }
    };
    if !operation_valid || projection.operation() != projection.lifecycle().operation_kind {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    Ok(())
}

fn validate_private_state_link(
    predecessor: &KagemushaStateV1,
    successor: &KagemushaStateV1,
    amount: u128,
    operation: KagemushaOperationKindV1,
) -> Result<(), KagemushaStateErrorV1> {
    predecessor.validate()?;
    successor.validate()?;
    if amount == 0
        || predecessor.context() != successor.context()
        || predecessor.lane != successor.lane
        || predecessor.hardware_epoch != successor.hardware_epoch
        || predecessor.device_policy_binding != successor.device_policy_binding
        || predecessor.consumed_credit_root != successor.consumed_credit_root
        || predecessor.state_commitment == successor.state_commitment
        || successor.logical_sequence
            != predecessor
                .logical_sequence
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::SequenceOverflow)?
        || !matches!(
            operation,
            KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit
        )
        || successor.balance
            != predecessor
                .balance
                .checked_sub(amount)
                .ok_or(KagemushaStateErrorV1::InsufficientBalance)?
    {
        return Err(KagemushaStateErrorV1::StateInvariant);
    }
    Ok(())
}

pub(super) fn validate_request_against_state(
    request: &KagemushaPaymentRequestV1,
    state: &KagemushaStateV1,
) -> Result<(), KagemushaStateErrorV1> {
    if request.release_id != state.release_id
        || request.network_id != state.lane.network_id
        || request.asset != state.lane.asset
        || request.asset_incarnation != state.asset_incarnation
        || request.scale != state.lane.scale
        || request.liability_pool_id != state.liability_pool_id
        || request.hardware_credential.suite_id != state.suite_id
    {
        return Err(KagemushaStateErrorV1::InvalidPaymentRequest);
    }
    Ok(())
}

fn validate_recovery_material(
    sealed_transition_inputs: &[u8],
    sealed_recovery_seeds: &[u8],
    normalized_guard_statement_digest: DigestV1,
) -> Result<(), KagemushaStateErrorV1> {
    if sealed_transition_inputs.is_empty()
        || sealed_transition_inputs.len()
            > usize::try_from(KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1).unwrap_or(usize::MAX)
        || sealed_recovery_seeds.is_empty()
        || sealed_recovery_seeds.len()
            > usize::try_from(KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1).unwrap_or(usize::MAX)
        || normalized_guard_statement_digest == [0; 32]
    {
        return Err(KagemushaStateErrorV1::InvalidRecoveryMaterial);
    }
    Ok(())
}

pub(super) fn validate_installed_successor_not_ahead(
    current: &KagemushaStateV1,
    installed: &KagemushaStateV1,
) -> Result<(), KagemushaStateErrorV1> {
    if current.release_id != installed.release_id
        || current.asset_incarnation != installed.asset_incarnation
        || current.liability_pool_id != installed.liability_pool_id
        || current.lane != installed.lane
        || current.hardware_epoch.generation < installed.hardware_epoch.generation
        || (current.hardware_epoch.generation == installed.hardware_epoch.generation
            && current.hardware_epoch != installed.hardware_epoch)
        || (current.hardware_epoch == installed.hardware_epoch
            && current.logical_sequence < installed.logical_sequence)
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

fn canonical_len<T: norito::NoritoSerialize>(value: &T) -> Result<u64, KagemushaStateErrorV1> {
    let bytes =
        norito::encode_canonical(value).map_err(|_| KagemushaStateErrorV1::CanonicalEncoding)?;
    u64::try_from(bytes.len()).map_err(|_| KagemushaStateErrorV1::ArithmeticOverflow)
}

fn map_operation_index_error(
    error: KagemushaOutgoingOperationIndexErrorV1,
) -> KagemushaStateErrorV1 {
    match error {
        KagemushaOutgoingOperationIndexErrorV1::InvalidBinding
        | KagemushaOutgoingOperationIndexErrorV1::Conflict => {
            KagemushaStateErrorV1::CandidateConflict
        }
        KagemushaOutgoingOperationIndexErrorV1::InvalidStage
        | KagemushaOutgoingOperationIndexErrorV1::StalePage => {
            KagemushaStateErrorV1::InvalidCandidateStage
        }
        KagemushaOutgoingOperationIndexErrorV1::RevisionOverflow => {
            KagemushaStateErrorV1::ArithmeticOverflow
        }
        KagemushaOutgoingOperationIndexErrorV1::CanonicalEncoding => {
            KagemushaStateErrorV1::CanonicalEncoding
        }
        KagemushaOutgoingOperationIndexErrorV1::SnapshotIntegrity => {
            KagemushaStateErrorV1::StateInvariant
        }
    }
}

fn digest_raw_bytes(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    use sha2::{Digest as _, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(test)]
mod preparation_id_tests {
    use super::*;

    fn vector_fields() -> PreparationIdCommitmentsV1 {
        let transition = [0x11, 0x22, 0x33];
        let recovery = [0x44, 0x55];
        PreparationIdCommitmentsV1 {
            operation_tag: 2,
            predecessor_state_commitment: [1; 32],
            successor_state_commitment: [2; 32],
            state_transition_digest: [3; 32],
            prepared_transition_binding_digest: [4; 32],
            projection_semantic_digest: [5; 32],
            lifecycle_binding_digest: [6; 32],
            request_digest: [7; 32],
            artifact_manifest_digest: [0; 32],
            normalized_guard_statement_digest: [8; 32],
            outbox_reservation_commitment: [9; 32],
            prepared_one_use_authorization_digest: [10; 32],
            sealed_transition_inputs_len: transition.len() as u64,
            sealed_transition_inputs_digest: digest_raw_bytes(
                SEALED_TRANSITION_DIGEST_DOMAIN_V1,
                &transition,
            ),
            sealed_recovery_seeds_len: recovery.len() as u64,
            sealed_recovery_seeds_digest: digest_raw_bytes(
                SEALED_RECOVERY_DIGEST_DOMAIN_V1,
                &recovery,
            ),
        }
    }

    #[test]
    fn sealed_stream_digests_use_distinct_domains_and_exact_lengths() {
        let fields = vector_fields();
        assert_eq!(
            fields.sealed_transition_inputs_digest,
            [
                0x72, 0x0e, 0x23, 0x12, 0x88, 0xab, 0x5c, 0xf4, 0xd4, 0xfc, 0x02, 0xc5, 0xc2, 0xf3,
                0x21, 0x2e, 0xd1, 0x6a, 0x93, 0x9d, 0x11, 0xb1, 0xa3, 0x6c, 0x37, 0x3f, 0xc9, 0x24,
                0x6d, 0xc2, 0xea, 0xbe,
            ]
        );
        assert_eq!(
            fields.sealed_recovery_seeds_digest,
            [
                0x66, 0x3f, 0x71, 0x16, 0x93, 0x8b, 0x6d, 0xc9, 0xfa, 0xea, 0x77, 0x74, 0xd6, 0xaa,
                0xd9, 0xc0, 0x07, 0xbe, 0x12, 0x97, 0x1e, 0x42, 0x4b, 0xef, 0xf0, 0x53, 0x7e, 0xe9,
                0xa4, 0xde, 0xdc, 0xf8,
            ]
        );
        let same_bytes = [0x11, 0x22, 0x33];
        assert_ne!(
            digest_raw_bytes(SEALED_TRANSITION_DIGEST_DOMAIN_V1, &same_bytes),
            digest_raw_bytes(SEALED_RECOVERY_DIGEST_DOMAIN_V1, &same_bytes)
        );
        assert_ne!(
            digest_raw_bytes(SEALED_TRANSITION_DIGEST_DOMAIN_V1, &same_bytes),
            digest_raw_bytes(SEALED_TRANSITION_DIGEST_DOMAIN_V1, &[0x11, 0x22, 0x33, 0])
        );
        let max_transition = vec![0x11; KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize];
        let max_recovery = vec![0x44; KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize];
        validate_recovery_material(&max_transition, &max_recovery, [1; 32])
            .expect("the fixed 2048/512 capacities are accepted");
        let mut too_long_transition = max_transition.clone();
        too_long_transition.push(0);
        assert_eq!(
            validate_recovery_material(&too_long_transition, &max_recovery, [1; 32]),
            Err(KagemushaStateErrorV1::InvalidRecoveryMaterial)
        );
        let mut too_long_recovery = max_recovery.clone();
        too_long_recovery.push(0);
        assert_eq!(
            validate_recovery_material(&max_transition, &too_long_recovery, [1; 32]),
            Err(KagemushaStateErrorV1::InvalidRecoveryMaterial)
        );
    }

    #[test]
    fn preparation_id_fixed_vector_binds_every_preproof_field() {
        let fields = vector_fields();
        let expected = [
            0xe4, 0xc4, 0xd7, 0xd7, 0x55, 0xb8, 0x67, 0xf7, 0xb7, 0x09, 0x3c, 0x33, 0xfb, 0xaa,
            0x6c, 0x57, 0x72, 0xbf, 0x9d, 0x9b, 0x1b, 0x04, 0x8b, 0x54, 0xe1, 0x0e, 0x4d, 0x38,
            0xec, 0xf1, 0xdd, 0x4a,
        ];
        assert_eq!(hash_preparation_id_commitments_v1(fields), expected);
        for mutation in 0..16 {
            let mut changed = fields;
            match mutation {
                0 => changed.operation_tag ^= 1,
                1 => changed.predecessor_state_commitment[0] ^= 1,
                2 => changed.successor_state_commitment[0] ^= 1,
                3 => changed.state_transition_digest[0] ^= 1,
                4 => changed.prepared_transition_binding_digest[0] ^= 1,
                5 => changed.projection_semantic_digest[0] ^= 1,
                6 => changed.lifecycle_binding_digest[0] ^= 1,
                7 => changed.request_digest[0] ^= 1,
                8 => changed.artifact_manifest_digest[0] ^= 1,
                9 => changed.normalized_guard_statement_digest[0] ^= 1,
                10 => changed.outbox_reservation_commitment[0] ^= 1,
                11 => changed.prepared_one_use_authorization_digest[0] ^= 1,
                12 => changed.sealed_transition_inputs_len += 1,
                13 => changed.sealed_transition_inputs_digest[0] ^= 1,
                14 => changed.sealed_recovery_seeds_len += 1,
                15 => changed.sealed_recovery_seeds_digest[0] ^= 1,
                _ => unreachable!(),
            }
            assert_ne!(
                hash_preparation_id_commitments_v1(changed),
                expected,
                "mutation {mutation} preserved the preparation ID"
            );
        }
    }

    #[test]
    fn redemption_preparation_uses_zero_request_and_pinned_manifest() {
        let mut fields = vector_fields();
        fields.operation_tag = 4;
        fields.request_digest = [0; 32];
        fields.artifact_manifest_digest = [11; 32];
        assert_eq!(
            hash_preparation_id_commitments_v1(fields),
            [
                0x74, 0x9a, 0x17, 0x02, 0xc4, 0x9b, 0x34, 0x3c, 0xbd, 0x76, 0x5c, 0x2a, 0x02, 0x09,
                0x68, 0xd4, 0xf8, 0x0a, 0x39, 0x67, 0x9a, 0x2f, 0xc3, 0x63, 0x9b, 0x1e, 0x2c, 0xdd,
                0x74, 0xde, 0xbb, 0xfd,
            ],
            "native redemption transcript must match the isolated terminal SHA vector",
        );
    }
}

#[cfg(test)]
mod sender_capacity_tests {
    use super::*;

    #[test]
    fn released_history_does_not_consume_live_outbox_capacity_or_forget_ids() {
        let total = LIVE_OUTBOX_SLOT_BYTES_V1 + 4 * 1024;
        let mut outbox = KagemushaSenderOutboxCapacityV1::new(total);
        let mut journal = KagemushaOutgoingCandidateJournalV1::default();
        for tag in 1..=96_u8 {
            let reservation = KagemushaOutboxReservationV1 {
                reservation_id: [tag; 32],
                operation_kind: KagemushaOperationKindV1::RedeemSplit,
                reserved_outbox_bytes: LIVE_OUTBOX_SLOT_BYTES_V1 as u32,
                issued_at_ms: 100,
                expires_at_ms: 10_000,
            };
            assert_eq!(
                outbox.reserve(reservation, &journal),
                Ok(SenderOutboxReservationOutcomeV1::Reserved)
            );
            assert!(outbox.committed_outbox_bytes() > LIVE_OUTBOX_SLOT_BYTES_V1);
            let mut other = reservation;
            other.reservation_id = [255; 32];
            assert_eq!(
                outbox.reserve(other, &journal),
                Err(KagemushaStateErrorV1::SenderOutboxCapacityExhausted)
            );
            // Exercise the capacity reducer after the Core receipt-owning release path. This
            // accounting test does not mint a receipt or stand in for its signature checks.
            let envelope_digest = [tag.wrapping_add(1); 32];
            outbox
                .reservations
                .get_mut(&reservation.reservation_id)
                .unwrap()
                .terminal_envelope_digest = Some(envelope_digest);
            outbox
                .mark_terminal_released(reservation.reservation_id, envelope_digest)
                .unwrap();
            journal
                .released_envelopes
                .insert(reservation.reservation_id, envelope_digest);
            outbox.reconcile_capacity_meters(&journal).unwrap();
            assert_eq!(outbox.committed_outbox_bytes(), 0);
            assert_eq!(outbox.available_outbox_bytes(), total);
            assert_eq!(
                outbox.reserve(reservation, &journal),
                Err(KagemushaStateErrorV1::CandidateConflict)
            );
        }
        assert!(outbox.retained_metadata_bytes() > total - LIVE_OUTBOX_SLOT_BYTES_V1);
        assert_eq!(outbox.reservations.len(), 96);
        assert_eq!(journal.released_envelopes.len(), 96);
        let bytes = norito::encode_canonical(&outbox).unwrap();
        let restored: KagemushaSenderOutboxCapacityV1 = norito::decode_canonical(&bytes).unwrap();
        restored.validate_recovered(&journal).unwrap();
        assert_eq!(restored, outbox);
        let mut retired_accounting = restored;
        retired_accounting.committed_outbox_bytes = retired_accounting.retained_metadata_bytes;
        assert_eq!(
            retired_accounting.validate_capacity_meters(&journal, true),
            Err(KagemushaStateErrorV1::SnapshotIntegrity)
        );
    }
}

#[cfg(test)]
#[test]
fn captured_state_frame_owners() {
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        KagemushaReceiverInboxCapacityV1,
    >(
        "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::KagemushaReceiverInboxCapacityV1"
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        KagemushaSenderOutboxCapacityV1,
    >(
        "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::KagemushaSenderOutboxCapacityV1"
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        PreparedOutgoingCandidateV1,
    >("iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PreparedOutgoingCandidateV1");
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        PersistedOutgoingCandidateV1,
    >("iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::PersistedOutgoingCandidateV1");
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        TerminalJournalCommitmentPreimageV1,
    >(
        "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::TerminalJournalCommitmentPreimageV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        TerminalRecoveryCommitmentPreimageV1,
    >(
        "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::TerminalRecoveryCommitmentPreimageV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        CommittedOutgoingCandidateV1,
    >("iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::CommittedOutgoingCandidateV1");
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<DurableOutgoingEnvelopeV1>(
        "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::DurableOutgoingEnvelopeV1",
    );
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<
        SenderCapacityMetadataProjectionV1,
    >(
        "iroha_core::zk::kagemusha_v1_state::candidate_lifecycle::SenderCapacityMetadataProjectionV1",
    );
}
