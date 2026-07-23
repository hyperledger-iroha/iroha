//! Idempotent production application of a durable Sumeragi v2 decision.
//!
//! A CommitQC is written to the safety WAL before this module is invoked. The
//! application transaction then re-loads the exact validated body, advances
//! Kura and WSV at most once, and finally persists the canonical v2 finality
//! sidecar. Restart may observe Kura/WSV already at the decided height while
//! the sidecar is absent; that state is completed without re-applying the
//! block or validating it against a later state.

use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc, time::Duration};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::{BlockHeader, CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire},
    events::EventBox,
    merge::MergeLedgerEntry,
    transaction::SignedTransaction,
};
use iroha_primitives::time::TimeSource;
use thiserror::Error;

use super::{
    network_topology::Topology,
    v2_body_store::{BodyValidationError, V2BodyStore, ValidatedBodyReceipt},
    v2_core::{
        CanonicalIdentityProjection, EventTag, IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_DOMAIN_DURABLE_ARTIFACT, IDENTITY_DOMAIN_PAYLOAD, IDENTITY_DOMAIN_SUBJECT,
        IDENTITY_KIND_BLOCK_HEADER, IDENTITY_KIND_CANONICAL_PAYLOAD,
        IDENTITY_KIND_DURABLE_BODY_FRAME, IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
        IDENTITY_KIND_EXECUTION_COMMITMENT, IDENTITY_KIND_FINALITY_ARTIFACT,
        IDENTITY_KIND_PAYLOAD_MANIFEST, IDENTITY_KIND_QUORUM_CERTIFICATE,
        IDENTITY_KIND_WIRE_BLOCK_SUBJECT, IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        ProductionApplicationTraceProjection, ProductionDecisionIdentityProjection,
        ProductionDurableBodyIdentityProjection, ProductionQuorumCertificateIdentityProjection,
        TagProjection, production_application_trace_refines_decision_completion_kernel,
    },
    v2_effects::{ApplyTask, DurableApplyCompletion, EffectWorkId},
};
use crate::{
    EventsSender,
    block::{BlockValidationError, ValidBlock},
    kura::{CommitManifest, Kura, KuraV2CommitReceipt},
    queue::{LaneQueueReservationError, LaneQueueReservationOutcome, Queue, RoutingDecision},
    state::{MergeLedgerCommitError, MergeLedgerPublicationMode, State},
};

/// Fail-closed error while consuming or recovering durable lane reservations.
#[derive(Debug, Error)]
pub(crate) enum V2ReservationLifecycleError {
    /// Canonical merge history could not be read.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// A committed merge batch contains malformed reservation evidence.
    #[error(transparent)]
    Merge(#[from] MergeLedgerCommitError),
    /// The reservation journal rejected an exact retain/release/commit operation.
    #[error(transparent)]
    Queue(#[from] LaneQueueReservationError),
    /// One transaction appears with two different committed reservation identities.
    #[error(
        "committed merge history binds transaction {transaction_hash} to conflicting reservations"
    )]
    ConflictingCommittedBinding {
        /// Transaction with ambiguous durable ownership.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// Committed State retains a reservation without matching merge evidence.
    #[error("committed transaction {transaction_hash} has no exact durable merge reservation")]
    MissingCommittedBinding {
        /// Committed transaction whose reservation cannot be authenticated.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// Journal ownership differs from the committed merge evidence.
    #[error("committed transaction {transaction_hash} has a conflicting live reservation")]
    CommittedBindingMismatch {
        /// Committed transaction with mismatched reservation ownership.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// A merge entry names a transaction that State did not commit.
    #[error("merge reservation transaction {transaction_hash} is absent from committed State")]
    UncommittedMergeTransaction {
        /// Transaction missing from committed membership.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// The canonical carrier lost its exact full merge entry.
    #[error("committed merge carrier lost sidecar {entry_hash}")]
    MissingCommittedMergeEntry {
        /// Hash committed by the carrier's compact reference.
        entry_hash: HashOf<MergeLedgerEntry>,
    },
    /// The full entry no longer matches the carrier's compact projection.
    #[error("committed merge sidecar {entry_hash} differs from its carrier reference")]
    CommittedMergeReferenceMismatch {
        /// Hash committed by the carrier's compact reference.
        entry_hash: HashOf<MergeLedgerEntry>,
    },
    /// Queue retained a release barrier whose exact Kura retirement is absent.
    #[error("queue release barrier {retirement_hash} has no exact durable Kura retirement")]
    MissingReleaseRetirement {
        /// Digest of the missing retirement identity.
        retirement_hash: Hash,
    },
    /// Queue and Kura disagree on a release barrier's full slot/payload binding.
    #[error("queue release barrier {retirement_hash} conflicts with durable Kura retirement")]
    ReleaseRetirementMismatch {
        /// Digest of the conflicting retirement identity.
        retirement_hash: Hash,
    },
}

fn finalize_certified_merge_reservations(
    state: &State,
    queue: &Queue,
    entry: &MergeLedgerEntry,
) -> Result<usize, V2ReservationLifecycleError> {
    let reservations = crate::state::certified_merge_queue_reservations(entry)?;
    let mut finalized = 0usize;
    for (transaction_hash, reservation) in reservations {
        if !state.has_committed_transaction(transaction_hash) {
            return Err(V2ReservationLifecycleError::UncommittedMergeTransaction {
                transaction_hash,
            });
        }
        if queue.commit_lane_reservation(&reservation)? == LaneQueueReservationOutcome::Finalized {
            finalized = finalized.saturating_add(1);
        }
    }
    Ok(finalized)
}

fn committed_block_merge_entry(
    kura: &Kura,
    block: &SignedBlock,
) -> Result<Option<MergeLedgerEntry>, V2ReservationLifecycleError> {
    let Some(reference) = block
        .execution_context()
        .and_then(|bundle| bundle.merge_entry.as_ref())
    else {
        return Ok(None);
    };
    let entry = kura.merge_entry_by_hash(reference.entry_hash)?.ok_or(
        V2ReservationLifecycleError::MissingCommittedMergeEntry {
            entry_hash: reference.entry_hash,
        },
    )?;
    if !reference.matches_entry(&entry) {
        return Err(
            V2ReservationLifecycleError::CommittedMergeReferenceMismatch {
                entry_hash: reference.entry_hash,
            },
        );
    }
    Ok(Some(entry))
}

fn finalize_committed_block_merge_reservations(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    block: &SignedBlock,
) -> Result<usize, V2ReservationLifecycleError> {
    let Some(entry) = committed_block_merge_entry(kura, block)? else {
        return Ok(0);
    };
    finalize_certified_merge_reservations(state, queue, &entry)
}

/// Execute or resume the complete crash-safe retirement/release hand-off.
///
/// This is the single production ordering implementation shared by live lane
/// work and startup reconciliation:
///
/// 1. Kura persists the exact slot retirement and `ReleasePending` claims.
/// 2. Queue persists the exact ordered barrier while reservations remain live.
/// 3. Kura changes the exact claims to `Released`.
/// 4. Queue completes ownership transfer, restores FIFO order, and forgets the
///    replay barrier.
pub(crate) fn retire_autonomous_lane_slot_and_release_reservations(
    kura: &Kura,
    queue: &Queue,
    retirement: &crate::kura::AutonomousLaneSlotRetirementV1,
    expected_chain_id_hash: Hash,
    expected_epoch: u64,
) -> Result<usize, V2ReservationLifecycleError> {
    kura.persist_autonomous_lane_slot_retirement(
        retirement,
        expected_chain_id_hash,
        expected_epoch,
    )?;
    let barrier = retirement.queue_release_barrier()?;
    let _ = queue.prepare_lane_reservation_release_barrier(&barrier)?;
    kura.finalize_autonomous_lane_slot_release(
        retirement,
        &barrier,
        expected_chain_id_hash,
        expected_epoch,
    )?;
    Ok(queue.finalize_lane_reservation_release_barrier(&barrier)?)
}

/// Reconcile replayed lane reservations against committed State and Kura.
///
/// Committed transactions are consumed before orphan release so a crash after
/// merge publication can never make already-applied work eligible again.
pub(crate) fn reconcile_lane_reservation_ownership(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    chain_id: &ChainId,
) -> Result<(usize, usize), V2ReservationLifecycleError> {
    let recovered = queue.live_lane_reservations();
    let recovered_release_barriers = queue.lane_reservation_release_barriers();
    if recovered.is_empty() && recovered_release_barriers.is_empty() {
        return Ok((0, 0));
    }

    let mut exact_committed = BTreeMap::new();
    for entry in kura.merge_ledger_all_entries()? {
        for (transaction_hash, reservation) in
            crate::state::certified_merge_queue_reservations(&entry)?
        {
            if let Some(existing) = exact_committed.insert(transaction_hash, reservation)
                && existing != reservation
            {
                return Err(V2ReservationLifecycleError::ConflictingCommittedBinding {
                    transaction_hash,
                });
            }
        }
    }

    let mut finalized_committed = 0usize;
    for reservation in &recovered {
        if !state.has_committed_transaction(reservation.signed_transaction_hash) {
            continue;
        }
        let committed = exact_committed
            .get(&reservation.signed_transaction_hash)
            .ok_or(V2ReservationLifecycleError::MissingCommittedBinding {
                transaction_hash: reservation.signed_transaction_hash,
            })?;
        if committed != reservation {
            return Err(V2ReservationLifecycleError::CommittedBindingMismatch {
                transaction_hash: reservation.signed_transaction_hash,
            });
        }
        if queue.commit_lane_reservation(reservation)? == LaneQueueReservationOutcome::Finalized {
            finalized_committed = finalized_committed.saturating_add(1);
        }
    }

    let remaining = queue.live_lane_reservations();
    let nexus = state.nexus_snapshot();
    let world = state.world_view();
    let chain_hash = Hash::new(chain_id.clone().into_inner().as_bytes());
    let mut retired_slots = BTreeMap::new();
    for reservation in &remaining {
        let epoch =
            crate::sumeragi::epoch_for_height_from_world(&world, reservation.proposal_height);
        let Some(retirement) =
            kura.autonomous_lane_retirement_matching_reservation(reservation, chain_hash, epoch)?
        else {
            continue;
        };
        retired_slots
            .entry(retirement.digest()?)
            .or_insert((retirement, epoch));
    }
    for barrier in recovered_release_barriers {
        if barrier.chain_id_hash != chain_hash {
            return Err(V2ReservationLifecycleError::ReleaseRetirementMismatch {
                retirement_hash: barrier.retirement_hash,
            });
        }
        let retirement = kura
            .read_autonomous_lane_slot_retirement(
                barrier.lane_id,
                barrier.lane_block_height,
                chain_hash,
                barrier.epoch,
            )?
            .ok_or(V2ReservationLifecycleError::MissingReleaseRetirement {
                retirement_hash: barrier.retirement_hash,
            })?;
        if retirement.queue_release_barrier()? != barrier {
            return Err(V2ReservationLifecycleError::ReleaseRetirementMismatch {
                retirement_hash: barrier.retirement_hash,
            });
        }
        retired_slots
            .entry(retirement.digest()?)
            .or_insert((retirement, barrier.epoch));
    }
    let mut released_retired = 0usize;
    for (retirement, epoch) in retired_slots.into_values() {
        released_retired =
            released_retired.saturating_add(retire_autonomous_lane_slot_and_release_reservations(
                kura,
                queue,
                &retirement,
                chain_hash,
                epoch,
            )?);
    }

    let remaining = queue.live_lane_reservations();
    let released_orphans =
        queue.reconcile_orphaned_lane_reservations(&remaining, |reservation| {
            state.lane_incarnation_at_height(reservation.lane_id, reservation.proposal_height)
                == Some(reservation.lane_incarnation)
                && crate::state::nexus_active_lane_dataspace_at_height(
                    reservation.lane_id,
                    &nexus,
                    reservation.proposal_height,
                ) == Some(reservation.dataspace_id)
                && kura.autonomous_lane_payload_matches_reservation(
                    reservation,
                    chain_hash,
                    crate::sumeragi::epoch_for_height_from_world(
                        &world,
                        reservation.proposal_height,
                    ),
                )
        })?;
    Ok((
        finalized_committed,
        released_retired.saturating_add(released_orphans),
    ))
}

fn application_typed_identity<T>(
    domain: u8,
    kind: u8,
    hash: HashOf<T>,
) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}

fn application_hash_identity(domain: u8, kind: u8, hash: Hash) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}

fn application_decision_projection(
    decision: wire::QuorumCertificateRef,
) -> ProductionDecisionIdentityProjection {
    ProductionDecisionIdentityProjection {
        context_id: application_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            decision.round.context_id.0,
        ),
        height: decision.round.height,
        view: decision.round.view,
        proposal_height: decision.proposal_round.height,
        proposal_view: decision.proposal_round.view,
        phase: decision.phase as u8,
        subject: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
            HashOf::new(&decision.subject),
        ),
        block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            decision.subject.block_hash,
        ),
        payload_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            decision.subject.payload_hash,
        ),
        execution_commitment: application_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTION_COMMITMENT,
            HashOf::new(&decision.execution_commitment),
        ),
        executed_block_wire_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
            decision.execution_commitment.executed_block_wire_hash,
        ),
    }
}

fn application_certificate_projection(
    certificate: &wire::QuorumCertificate,
) -> Option<ProductionQuorumCertificateIdentityProjection> {
    Some(ProductionQuorumCertificateIdentityProjection {
        decision: application_decision_projection(certificate.as_ref()),
        certificate: application_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_QUORUM_CERTIFICATE,
            HashOf::new(certificate),
        ),
        signer_count: u64::try_from(certificate.signers.len()).ok()?,
        aggregate_signature_len: u64::try_from(certificate.aggregate_signature.len()).ok()?,
    })
}

fn application_body_projection(
    receipt: &ValidatedBodyReceipt,
) -> ProductionDurableBodyIdentityProjection {
    let durable = receipt.durable();
    let subject = durable.subject();
    ProductionDurableBodyIdentityProjection {
        context_id: application_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            durable.context_id().0,
        ),
        height: durable.round().height,
        view: durable.round().view,
        subject: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
            HashOf::new(&subject),
        ),
        block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            subject.block_hash,
        ),
        payload_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            subject.payload_hash,
        ),
        manifest: application_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_PAYLOAD_MANIFEST,
            durable.manifest_hash(),
        ),
        frame: application_hash_identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_DURABLE_BODY_FRAME,
            durable.frame_hash(),
        ),
    }
}

/// Complete native identity crossing the durable application boundary.
///
/// The type is process-local and intentionally has no codec implementation.
/// It retains full typed consensus and durability evidence. Canonical proposal,
/// executed-block, body-frame, and artifact links use the existing native
/// 256-bit hash values without projection or truncation; those comparisons rely
/// on the repository's reviewed collision-resistance contract.
#[derive(Clone, Debug)]
#[must_use]
pub(crate) struct DurableApplicationEvidence {
    task_tag: EventTag,
    owner_tag: EventTag,
    task_generation: u64,
    task_work_id: EffectWorkId,
    context: wire::HeightContext,
    commit_qc: wire::QuorumCertificate,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
    validated_receipt: ValidatedBodyReceipt,
    validated_manifest_hash: HashOf<wire::PayloadManifest>,
    validated_body_frame_hash: Hash,
    proposal_block_hash: HashOf<BlockHeader>,
    canonical_proposal_wire_hash: Hash,
    committed_block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    kura_receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
    artifact_hash: HashOf<wire::finality::V2FinalityArtifact>,
    completion_work_id: EffectWorkId,
    state_height_after: usize,
}

impl DurableApplicationEvidence {
    /// Reducer incarnation which created the Apply task.
    pub(crate) const fn task_tag(&self) -> EventTag {
        self.task_tag
    }

    /// Reducer incarnation captured by the executor when it authorized Apply.
    pub(crate) const fn owner_tag(&self) -> EventTag {
        self.owner_tag
    }

    /// Actor-local task generation, distinct from consensus view.
    pub(crate) const fn task_generation(&self) -> u64 {
        self.task_generation
    }

    /// Stable asynchronous work owner assigned to the Apply task.
    pub(crate) const fn task_work_id(&self) -> EffectWorkId {
        self.task_work_id
    }

    /// Complete immutable height context governing application.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
    }

    /// Complete CommitQC, including canonical signers and aggregate signature.
    pub(crate) const fn commit_qc(&self) -> &wire::QuorumCertificate {
        &self.commit_qc
    }

    /// Exact round carried by the CommitQC.
    pub(crate) const fn commit_round(&self) -> wire::ConsensusRound {
        self.commit_qc.round
    }

    /// Exact phase carried by the CommitQC.
    pub(crate) const fn commit_phase(&self) -> wire::GlobalPhase {
        self.commit_qc.phase
    }

    /// Canonically ordered CommitQC signer indices.
    pub(crate) fn commit_signers(&self) -> &[wire::ValidatorIndex] {
        &self.commit_qc.signers
    }

    /// Complete CommitQC aggregate-signature evidence.
    pub(crate) fn commit_aggregate_signature(&self) -> &[u8] {
        &self.commit_qc.aggregate_signature
    }

    /// Exact decided subject repeated independently by the Apply task.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    /// Exact deterministic execution commitment authenticated by the CommitQC.
    pub(crate) const fn execution_commitment(&self) -> wire::ExecutionCommitment {
        self.execution_commitment
    }

    /// Durable validation receipt for the proposal bytes being applied.
    pub(crate) const fn validated_receipt(&self) -> &ValidatedBodyReceipt {
        &self.validated_receipt
    }

    /// Frozen context carried by the validated durable body receipt.
    pub(crate) const fn validated_context_id(&self) -> wire::HeightContextId {
        self.validated_receipt.durable().context_id()
    }

    /// Proposal round carried by the validated durable body receipt.
    pub(crate) const fn validated_round(&self) -> wire::ConsensusRound {
        self.validated_receipt.durable().round()
    }

    /// Proposal subject carried by the validated durable body receipt.
    pub(crate) const fn validated_subject(&self) -> wire::BlockSubject {
        self.validated_receipt.durable().subject()
    }

    /// Manifest identity carried by the validated durable body receipt.
    pub(crate) const fn validated_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.validated_manifest_hash
    }

    /// Hash of the complete checksummed body frame that passed validation.
    pub(crate) const fn validated_body_frame_hash(&self) -> Hash {
        self.validated_body_frame_hash
    }

    /// Header identity of the resultless proposal loaded from the body store.
    pub(crate) const fn proposal_block_hash(&self) -> HashOf<BlockHeader> {
        self.proposal_block_hash
    }

    /// Hash of the exact canonical resultless proposal wire.
    pub(crate) const fn canonical_proposal_wire_hash(&self) -> Hash {
        self.canonical_proposal_wire_hash
    }

    /// Header identity of the canonical result-bearing committed block.
    pub(crate) const fn committed_block_hash(&self) -> HashOf<BlockHeader> {
        self.committed_block_hash
    }

    /// Hash of the exact canonical result-bearing committed block wire.
    pub(crate) const fn executed_block_wire_hash(&self) -> Hash {
        self.executed_block_wire_hash
    }

    /// Complete non-forgeable Kura finality receipt.
    pub(crate) const fn kura_receipt(&self) -> &KuraV2CommitReceipt {
        &self.kura_receipt
    }

    /// Height durably acknowledged by Kura.
    pub(crate) fn kura_height(&self) -> u64 {
        self.kura_receipt.height()
    }

    /// Canonical block header hash durably acknowledged by Kura.
    pub(crate) fn kura_block_hash(&self) -> HashOf<BlockHeader> {
        self.kura_receipt.block_hash()
    }

    /// Frozen height-context identifier durably acknowledged by Kura.
    pub(crate) fn kura_context_id(&self) -> wire::HeightContextId {
        self.kura_receipt.context_id()
    }

    /// Exact subject durably acknowledged by Kura.
    pub(crate) fn kura_subject(&self) -> wire::BlockSubject {
        self.kura_receipt.subject()
    }

    /// Exact CommitQC reference durably acknowledged by Kura.
    pub(crate) fn kura_certificate(&self) -> wire::QuorumCertificateRef {
        self.kura_receipt.certificate()
    }

    /// Exact finality-artifact identity durably acknowledged by Kura.
    pub(crate) fn kura_artifact_hash(&self) -> HashOf<wire::finality::V2FinalityArtifact> {
        self.kura_receipt.artifact_hash()
    }

    /// Complete finality artifact stored beside the committed block.
    pub(crate) const fn artifact(&self) -> &wire::finality::V2FinalityArtifact {
        &self.artifact
    }

    /// Native typed hash of the complete finality artifact.
    pub(crate) const fn artifact_hash(&self) -> HashOf<wire::finality::V2FinalityArtifact> {
        self.artifact_hash
    }

    /// Work identifier carried by the typed completion.
    pub(crate) const fn completion_work_id(&self) -> EffectWorkId {
        self.completion_work_id
    }

    /// Exact committed State height observed after all durable publications.
    pub(crate) const fn state_height_after(&self) -> usize {
        self.state_height_after
    }

    /// Project each independently retained application identity into the pure
    /// production/Verus kernel. Cardinalities fail closed if they cannot be
    /// represented by the shared fixed-width surface.
    pub(crate) fn application_refinement_projection(
        &self,
    ) -> Option<ProductionApplicationTraceProjection> {
        let context_id = application_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            self.context().id().0,
        );
        let artifact = self.artifact();
        Some(ProductionApplicationTraceProjection {
            task_tag: TagProjection {
                height: self.task_tag().height(),
                view: self.task_tag().view(),
                generation: self.task_tag().generation().get(),
            },
            owner_tag: TagProjection {
                height: self.owner_tag().height(),
                view: self.owner_tag().view(),
                generation: self.owner_tag().generation().get(),
            },
            task_generation: self.task_generation(),
            context_id,
            context_height: self.context().height,
            commit_qc: application_certificate_projection(self.commit_qc())?,
            validated_body: application_body_projection(self.validated_receipt()),
            validated_execution_commitment: application_typed_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_EXECUTION_COMMITMENT,
                HashOf::new(&self.validated_receipt().execution_commitment()),
            ),
            proposal_block_hash: application_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER,
                self.proposal_block_hash(),
            ),
            proposal_payload_hash: application_hash_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_CANONICAL_PAYLOAD,
                self.canonical_proposal_wire_hash(),
            ),
            committed_block_hash: application_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER,
                self.committed_block_hash(),
            ),
            executed_block_wire_hash: application_hash_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
                self.executed_block_wire_hash(),
            ),
            kura_decision: application_decision_projection(self.kura_certificate()),
            kura_artifact_hash: application_typed_identity(
                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                IDENTITY_KIND_FINALITY_ARTIFACT,
                self.kura_artifact_hash(),
            ),
            artifact_context_id: application_typed_identity(
                IDENTITY_DOMAIN_CONTEXT,
                IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
                artifact.height_context.id().0,
            ),
            artifact_height: artifact.height,
            artifact_subject: application_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
                HashOf::new(&artifact.subject),
            ),
            artifact_block_hash: application_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER,
                artifact.block_hash,
            ),
            artifact_commit_qc: application_certificate_projection(&artifact.commit_qc)?,
            artifact_hash: application_typed_identity(
                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                IDENTITY_KIND_FINALITY_ARTIFACT,
                self.artifact_hash(),
            ),
            state_height_after: u64::try_from(self.state_height_after()).ok()?,
            task_work_id: self.task_work_id().get(),
            completion_work_id: self.completion_work_id().get(),
        })
    }

    /// Check every redundant task, wire, durability, and completion identity.
    pub(crate) fn is_exact(&self) -> bool {
        let context = self.context();
        let certificate = self.commit_qc();
        let artifact = self.artifact();
        let Ok(context_height) = usize::try_from(context.height) else {
            return false;
        };
        context.validate().is_ok()
            && certificate.validate(context).is_ok()
            && self.task_tag().height() == context.height
            // Lifecycle ownership is independent of the certificate's
            // intrinsic consensus round. The executor mints this owner only
            // after matching the effect tag to the current reducer tag.
            && self.task_tag() == self.owner_tag()
            && self.task_tag().generation().get() == self.task_generation()
            && self.commit_phase() == wire::GlobalPhase::Commit
            && self.commit_round().context_id == context.id()
            && self.commit_round().height == context.height
            && certificate.subject == self.subject()
            && certificate.execution_commitment == self.execution_commitment()
            && self.commit_signers() == artifact.commit_qc.signers.as_slice()
            && self.commit_aggregate_signature()
                == artifact.commit_qc.aggregate_signature.as_slice()
            && self.validated_context_id() == context.id()
            && self.validated_round().height == context.height
            // The durable body must be the exact immutable proposal origin
            // authenticated by the CommitQC, independently of its later
            // finality round.
            && self.validated_round() == certificate.proposal_round
            && self.validated_subject() == self.subject()
            && self.validated_manifest_hash() == self.validated_receipt().durable().manifest_hash()
            && self.validated_body_frame_hash() == self.validated_receipt().durable().frame_hash()
            && self.validated_receipt().execution_commitment() == self.execution_commitment()
            && self.proposal_block_hash() == self.subject().block_hash
            && self.canonical_proposal_wire_hash() == self.subject().payload_hash
            && self.committed_block_hash() == self.subject().block_hash
            && self.executed_block_wire_hash()
                == self.execution_commitment().executed_block_wire_hash
            && self.kura_receipt().height() == self.kura_height()
            && self.kura_height() == context.height
            && self.kura_context_id() == context.id()
            && self.kura_block_hash() == self.committed_block_hash()
            && self.kura_subject() == self.subject()
            && self.kura_certificate() == certificate.as_ref()
            && self.kura_artifact_hash() == self.artifact_hash()
            && &artifact.height_context == context
            && artifact.height == context.height
            && artifact.subject == self.subject()
            && artifact.block_hash == self.committed_block_hash()
            && &artifact.commit_qc == certificate
            && HashOf::new(artifact) == self.artifact_hash()
            && self.completion_work_id() == self.task_work_id()
            && self.state_height_after() == context_height
    }
}

/// Immutable dependencies of the single v2 application service.
pub(crate) struct V2ApplyService {
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    chain_id: ChainId,
    block_cadence: Duration,
    genesis_account: AccountId,
    events_sender: EventsSender,
    validator_set_pops: Vec<Vec<u8>>,
    #[cfg(test)]
    fail_after_kura_store: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_after_wsv_checkpoint: std::sync::atomic::AtomicBool,
}

impl V2ApplyService {
    fn classify_candidate_validation_error(
        merge_reference: Option<&CertifiedMergeLedgerReference>,
        failed_block: &SignedBlock,
        error: &BlockValidationError,
    ) -> V2ApplyError {
        if let BlockValidationError::MissingCertifiedMergeSidecar { entry_hash } = error {
            return match merge_reference {
                Some(reference) if reference.entry_hash == *entry_hash => {
                    V2ApplyError::MissingCertifiedMergeSidecar {
                        reference: reference.clone(),
                    }
                }
                _ => V2ApplyError::Validation(
                    "validator reported a missing certified merge sidecar that is not bound to the candidate execution context"
                        .to_owned(),
                ),
            };
        }
        let rejected_result_count = failed_block
            .has_results()
            .then(|| {
                failed_block
                    .results()
                    .filter(|result| result.is_err())
                    .count()
            })
            .unwrap_or(0);
        if rejected_result_count == 0 {
            V2ApplyError::Validation(error.to_string())
        } else {
            V2ApplyError::Validation(format!(
                "{error}; rejected transaction result count: {rejected_result_count}"
            ))
        }
    }

    fn validate_lane_payload_plan(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
        // Genesis instructions bootstrap the lane catalog itself and therefore cannot be routed
        // through a pre-existing committed lane plan. The canonical genesis validator below still
        // enforces its authority, chain, transaction, Merkle, and result invariants.
        if context.height == 1 && body.execution_context().is_none() {
            return Ok(());
        }
        let external_count = body.external_entrypoint_count();
        let Some(bundle) = body.execution_context() else {
            return if external_count == 0 {
                Ok(())
            } else {
                Err(V2ApplyError::Validation(
                    "Sumeragi v2 candidate has external entrypoints without execution context"
                        .to_owned(),
                ))
            };
        };
        if super::v2_lane_work::canonical_v2_lane_payload_matches_kura(
            self.state.as_ref(),
            self.kura.as_ref(),
            context,
            body,
        ) {
            return Ok(());
        }
        let routes = bundle
            .external
            .iter()
            .map(|entry| RoutingDecision::new(entry.lane_id, entry.dataspace_id))
            .collect::<Vec<_>>();
        let hashes = bundle
            .external
            .iter()
            .map(|entry| Hash::from(entry.entrypoint_hash))
            .collect::<Vec<_>>();
        let view = body.header().view_change_index();
        let leader = context
            .roster
            .get(usize::try_from(context.leader(view)).map_err(|_| {
                V2ApplyError::Validation("Sumeragi v2 leader index overflows usize".to_owned())
            })?)
            .ok_or_else(|| {
                V2ApplyError::Validation("Sumeragi v2 leader index is out of range".to_owned())
            })?;
        let expected = super::lane_planner::prepare_v2_lane_payload_plan(
            self.state.as_ref(),
            self.kura.as_ref(),
            context,
            view,
            &leader.validator,
            &routes,
            &hashes,
        )
        .map_err(|error| V2ApplyError::Validation(error.to_string()))?;
        if !expected.unavailable_indices.is_empty()
            || expected.ownerships != bundle.lane_payload_ownerships
        {
            return Err(V2ApplyError::Validation(
                "Sumeragi v2 lane ownerships differ from deterministic committed-state planning"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// Construct the serialized state/Kura application adapter.
    pub(crate) fn new(
        state: Arc<State>,
        queue: Arc<Queue>,
        kura: Arc<Kura>,
        chain_id: ChainId,
        block_cadence: Duration,
        genesis_account: AccountId,
        events_sender: EventsSender,
        validator_set_pops: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            state,
            queue,
            kura,
            chain_id,
            block_cadence,
            genesis_account,
            events_sender,
            validator_set_pops,
            #[cfg(test)]
            fail_after_kura_store: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_after_wsv_checkpoint: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Apply one exact CommitQC task or complete its interrupted sidecar write.
    pub(crate) fn execute(
        &self,
        context: &wire::HeightContext,
        body_store: &mut V2BodyStore,
        task: &ApplyTask,
    ) -> Result<DurableApplyCompletion, V2ApplyError> {
        context.validate()?;
        if task.subject() != task.certificate().subject
            || task.certificate().phase != wire::GlobalPhase::Commit
            || task.certificate().round.context_id != context.id()
            || task.certificate().round.height != context.height
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        task.certificate().execution_commitment.validate()?;
        if task.certificate().execution_commitment
            != task.validated_receipt().execution_commitment()
        {
            return Err(V2ApplyError::ExecutionCommitmentMismatch);
        }
        let body = body_store.load(task.validated_receipt().durable())?;
        let proposal_block_hash = body.hash();
        let canonical_proposal_wire_hash = body
            .canonical_proposal_wire_hash()
            .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;
        if !body.is_resultless_proposal()
            || proposal_block_hash != task.subject().block_hash
            || body.header().height().get() != context.height
            || body.header().prev_block_hash() != task.subject().parent_block_hash
            || canonical_proposal_wire_hash != task.subject().payload_hash
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        // Authenticate the exact durable decision and its association with the selected body
        // before pruning carrier sidecars or crossing either Kura/WSV commit boundary.
        // `ApplyTask` deliberately retains the wire certificate, so this adapter must not rely
        // only on the upstream reducer having verified it. A malformed decision remains a pure
        // rejection, never a crash image whose canonical block/state lacks valid finality.
        let artifact = wire::finality::V2FinalityArtifact::new(
            context.clone(),
            task.subject(),
            task.certificate().clone(),
            self.validator_set_pops.clone(),
        );
        artifact.validate_for_header(&body.header())?;
        artifact
            .verify()
            .map_err(V2ApplyError::FinalityCryptography)?;

        let height = usize::try_from(context.height).map_err(|_| V2ApplyError::HeightOverflow)?;
        let height = NonZeroUsize::new(height).ok_or(V2ApplyError::HeightOverflow)?;
        let state_height = self.state.committed_height();
        if state_height > height.get() {
            return Err(V2ApplyError::StateAhead {
                state_height,
                decision_height: height.get(),
            });
        }
        let durable_hash = self.kura.get_durable_block_hash(height);
        if durable_hash.is_some_and(|hash| hash != task.subject().block_hash) {
            return Err(V2ApplyError::KuraConflict);
        }

        if state_height < height.get() {
            if state_height.saturating_add(1) != height.get() {
                return Err(V2ApplyError::StateGap {
                    state_height,
                    decision_height: height.get(),
                });
            }
        } else if durable_hash.is_none() {
            // WSV cannot be ahead of its canonical block log. Continuing here
            // would manufacture a sidecar for state that Kura cannot identify.
            return Err(V2ApplyError::StateAheadOfKura);
        }

        // The durable CommitQC and exact validated body now identify the only
        // carrier that can ever apply at this height. Keep its immutable
        // compact reference (including an earlier lock origin view) and
        // release every losing pending sidecar before validation can defer on
        // a missing exact entry. A failure after this point remains safe: the
        // decided reference survives, while no losing carrier can become
        // canonical.
        self.retain_decided_merge_sidecar(context, &body)?;

        let committed_block = if state_height < height.get() {
            self.validate_and_apply(
                context,
                body,
                true,
                task.validated_receipt().execution_commitment(),
                &artifact,
            )?;
            self.kura
                .get_block(height)
                .ok_or(V2ApplyError::StateAheadOfKura)?
        } else {
            // WSV is already committed. The proposal body is deliberately
            // resultless, so recovery must authenticate and retain Kura's
            // canonical result-bearing execution image rather than replacing
            // it with the proposal carrier.
            let committed = self
                .kura
                .get_block(height)
                .ok_or(V2ApplyError::StateAheadOfKura)?;
            if committed
                .canonical_proposal_wire_hash()
                .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?
                != task.subject().payload_hash
                || committed
                    .executed_block_wire_hash()
                    .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?
                    != task
                        .certificate()
                        .execution_commitment
                        .executed_block_wire_hash
            {
                return Err(V2ApplyError::ExecutionCommitmentMismatch);
            }
            self.kura.store_block(Arc::clone(&committed))?;
            committed
        };
        let committed_block_hash = committed_block.hash();
        let executed_block_wire_hash = committed_block
            .executed_block_wire_hash()
            .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;

        // Repair or confirm the pre-WSV durable evidence boundary before any
        // derived publication. Fresh application already crossed this boundary
        // inside `validate_and_apply`; the calls are deliberately idempotent so
        // restart can repair each individual artifact.
        let receipt = self
            .kura
            .store_v2_finality_artifact(&artifact)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("v2 finality artifact", &error)
            })?;

        // The strict restart-repair path authenticates Native AMX evidence
        // against both finality and the post-WSV Kura metadata join. Publish
        // that join first on every fresh or recovery attempt, then repair or
        // confirm the exact manifests, receipts, and latest indexes while the
        // prune guard keeps their canonical carrier stable.
        self.persist_post_apply_metadata(context, task, &artifact)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("post-apply metadata", &error)
            })?;
        self.kura
            .repair_native_amx_participant_application_evidence(committed_block.as_ref())
            .map_err(|error| {
                V2ApplyError::committed_recovery_required(
                    "Native AMX participant manifest/receipt repair",
                    &error,
                )
            })?;

        self.publish_committed_block_merge_entry(committed_block.as_ref())?;

        // Queue ownership is a third durable boundary after Kura and WSV. An
        // exact retry reaches this point even when State already crossed its
        // commit boundary, so a crash cannot leave merge-applied transactions
        // permanently reserved or eligible for replay.
        finalize_committed_block_merge_reservations(
            self.state.as_ref(),
            self.queue.as_ref(),
            self.kura.as_ref(),
            committed_block.as_ref(),
        )
        .map_err(|error| {
            V2ApplyError::committed_recovery_required("merge reservation finalization", &error)
        })?;

        self.kura
            .promote_kagemusha_topup_finality_sidecar(&artifact, &receipt)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required(
                    "Kagemusha finality sidecar promotion",
                    &error,
                )
            })?;
        let artifact_hash = HashOf::new(&artifact);
        let evidence = DurableApplicationEvidence {
            task_tag: task.tag(),
            owner_tag: task.authorized_owner_tag(),
            task_generation: task.tag().generation().get(),
            task_work_id: task.id(),
            context: context.clone(),
            commit_qc: task.certificate().clone(),
            subject: task.subject(),
            execution_commitment: task.validated_receipt().execution_commitment(),
            validated_receipt: task.validated_receipt().clone(),
            validated_manifest_hash: task.validated_receipt().durable().manifest_hash(),
            validated_body_frame_hash: task.validated_receipt().durable().frame_hash(),
            proposal_block_hash,
            canonical_proposal_wire_hash,
            committed_block_hash,
            executed_block_wire_hash,
            kura_receipt: receipt,
            artifact,
            artifact_hash,
            completion_work_id: task.id(),
            state_height_after: self.state.committed_height(),
        };
        self.finish_durable_apply_completion(evidence)
    }

    fn finish_durable_apply_completion(
        &self,
        evidence: DurableApplicationEvidence,
    ) -> Result<DurableApplyCompletion, V2ApplyError> {
        if !evidence.is_exact() {
            return Err(V2ApplyError::committed_recovery_required(
                "exact application evidence",
                &"native identity mismatch after durable application",
            ));
        }
        let application_trace = evidence
            .application_refinement_projection()
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"native application identity cannot be represented losslessly",
                )
            })?;
        if !production_application_trace_refines_decision_completion_kernel(application_trace) {
            return Err(V2ApplyError::committed_recovery_required(
                "application refinement evidence",
                &"durable application does not refine its Decision completion",
            ));
        }
        Ok(DurableApplyCompletion::new(
            evidence.completion_work_id,
            evidence.kura_receipt,
            evidence.artifact,
        ))
    }

    fn publish_committed_block_merge_entry(
        &self,
        committed_block: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
        let entry =
            committed_block_merge_entry(self.kura.as_ref(), committed_block).map_err(|error| {
                V2ApplyError::committed_recovery_required("merge cache publication", &error)
            })?;
        let Some(entry) = entry else {
            return Ok(());
        };
        self.state
            .ensure_globally_committed_merge_entry_applied(&entry)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("merge cache publication", &error)
            })?;
        let (_, event) = self
            .state
            .record_globally_committed_merge_entry(&entry, MergeLedgerPublicationMode::LiveCommit)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("merge cache publication", &error)
            })?;
        if let Some(event) = event {
            let _ = self.events_sender.send(EventBox::Pipeline(event));
        }
        Ok(())
    }

    fn retain_decided_merge_sidecar(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
        let reference = body
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.as_ref());
        self.kura
            .retain_pending_certified_merge_entry_for_locked_carrier(context.height, reference)?;
        Ok(())
    }

    /// Run the exact production proposal validator without applying its state
    /// overlay.
    ///
    /// The body store calls this only after authenticating the immutable
    /// origin-view block signature. Dropping the returned `StateBlock` keeps
    /// Prepare validation side-effect free while exercising the same
    /// deterministic execution path used during application.
    pub(crate) fn validate_candidate(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<wire::ExecutionCommitment, V2ApplyError> {
        if !body.is_resultless_proposal() {
            return Err(V2ApplyError::ResultBearingProposal);
        }
        self.validate_lane_payload_plan(context, body)?;
        let merge_reference = body
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.as_ref());
        let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
        let mut voting_block = None;
        let result = ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
            body.clone(),
            &topology,
            &self.chain_id,
            &self.genesis_account,
            &TimeSource::new_system(),
            self.block_cadence,
            crate::block::valid::SumeragiV2ValidationContext::from_height_context(context),
            self.state.as_ref(),
            &mut voting_block,
        )
        .unpack(|_| {});
        let (valid, mut state_block) = result.map_err(|(failed_block, error)| {
            Self::classify_candidate_validation_error(
                merge_reference,
                failed_block.as_ref(),
                error.as_ref(),
            )
        })?;
        let witness = state_block
            .take_exec_witness()
            .ok_or(V2ApplyError::ExecutionCommitmentUnavailable)?;
        let native_amx_manifest =
            crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
                valid.as_ref(),
            )
            .map_err(V2ApplyError::ExecutionCommitment)?;
        crate::sumeragi::exec::execution_commitment_from_witness(&witness, &native_amx_manifest)
            .map_err(|error| V2ApplyError::ExecutionCommitment(error.to_owned()))
    }

    fn validate_and_apply(
        &self,
        context: &wire::HeightContext,
        body: iroha_data_model::block::SignedBlock,
        store_block: bool,
        expected_execution_commitment: wire::ExecutionCommitment,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), V2ApplyError> {
        if !body.is_resultless_proposal() {
            return Err(V2ApplyError::ResultBearingProposal);
        }
        self.validate_lane_payload_plan(context, &body)?;
        let block_hash = body.hash();
        let merge_reference = body
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.clone());
        let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
        let mut voting_block = None;
        let mut pipeline_events = Vec::new();
        let (valid_block, mut state_block) =
            ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
                body,
                &topology,
                &self.chain_id,
                &self.genesis_account,
                &TimeSource::new_system(),
                self.block_cadence,
                crate::block::valid::SumeragiV2ValidationContext::from_height_context(context),
                self.state.as_ref(),
                &mut voting_block,
            )
            .unpack(|event| pipeline_events.push(event))
            .map_err(|(failed_block, error)| {
                Self::classify_candidate_validation_error(
                    merge_reference.as_ref(),
                    failed_block.as_ref(),
                    error.as_ref(),
                )
            })?;
        let witness = state_block
            .take_exec_witness()
            .ok_or(V2ApplyError::ExecutionCommitmentUnavailable)?;
        let native_amx_manifest =
            crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
                valid_block.as_ref(),
            )
            .map_err(V2ApplyError::ExecutionCommitment)?;
        let actual_execution_commitment = crate::sumeragi::exec::execution_commitment_from_witness(
            &witness,
            &native_amx_manifest,
        )
        .map_err(|error| V2ApplyError::ExecutionCommitment(error.to_owned()))?;
        if actual_execution_commitment != expected_execution_commitment {
            return Err(V2ApplyError::ExecutionCommitmentMismatch);
        }
        // Persist the witness-derived leaf/path projection before either the
        // canonical block log or WSV advances. Promotion is deliberately
        // deferred until Kura has durably persisted the exact finality
        // artifact; a crash at any intermediate point leaves an idempotent
        // stage that restart can complete without replaying committed state.
        self.kura.stage_kagemusha_topup_finality_sidecar(
            context.height,
            block_hash,
            &witness,
            expected_execution_commitment,
        )?;
        let committed_block = valid_block
            .commit_with_verified_v2_artifact(artifact, actual_execution_commitment)
            .unpack(|event| pipeline_events.push(event))
            .map_err(|(_, error)| V2ApplyError::Commit(error.to_string()))?;

        // Kura owns the first irreversible commit point. This call is also the
        // idempotent repair boundary for a durable block whose merge
        // association was interrupted after its block fsync.
        if store_block {
            self.kura.store_block(committed_block.clone())?;
            #[cfg(test)]
            if self
                .fail_after_kura_store
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return Err(V2ApplyError::InjectedCrashAfterKuraStore);
            }
            let _ = self.kura.store_v2_finality_artifact(artifact)?;
            self.kura
                .persist_native_amx_participant_application_evidence(committed_block.as_ref())?;
        }
        let commit_topology = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect();
        let state_events = state_block
            .apply_without_execution_with_verified_v2_finality(&committed_block, commit_topology);

        // Stage the exact would-be committed WSV hash while the validated
        // `StateBlock` overlay is still available. Kura is already durable at
        // this point, so the checkpoint must cross its own fsync boundary
        // before live State can advance. This closes the otherwise
        // unrecoverable crash window where restart observes State at the Kura
        // tip but has no authenticated hash with which to distinguish the
        // exact committed overlay from stale or corrupted memory.
        //
        // The checkpoint deliberately remains unbound until
        // `persist_post_apply_metadata` publishes the complete commit
        // manifest. A crash before State commit replays the overlay and must
        // reproduce this byte-identical hash; a crash after State commit can
        // authenticate the already-applied tip directly.
        let staged_checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(&state_block);
        self.kura
            .store_wsv_checkpoint(context.height, block_hash, staged_checkpoint)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("pre-WSV recovery checkpoint", &error)
            })?;
        #[cfg(test)]
        if self
            .fail_after_wsv_checkpoint
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return Err(V2ApplyError::InjectedCrashAfterWsvCheckpoint);
        }
        state_block.commit().map_err(|error| {
            V2ApplyError::committed_recovery_required("WSV publication after Kura commit", &error)
        })?;

        self.queue.remove_committed_hashes(
            committed_block
                .as_ref()
                .external_transactions()
                .map(|transaction| transaction.hash()),
            None,
        );
        let nexus = self.state.nexus_snapshot();
        let compliance = self.queue.lane_compliance_engine();
        self.queue
            .reconfigure_nexus_with_state(&nexus, self.state.as_ref(), compliance);

        for event in pipeline_events {
            let _ = self.events_sender.send(EventBox::Pipeline(event));
        }
        for event in state_events {
            let _ = self.events_sender.send(event);
        }
        Ok(())
    }

    fn persist_post_apply_metadata(
        &self,
        context: &wire::HeightContext,
        task: &ApplyTask,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), V2ApplyError> {
        let block_hash = task.subject().block_hash;
        let checkpoint = crate::snapshot::canonical_state_snapshot_hash(self.state.as_ref());
        self.kura
            .store_wsv_checkpoint(context.height, block_hash, checkpoint)?;
        let manifest =
            CommitManifest::new(context.height, block_hash, None, None, checkpoint, None)
                .with_authenticated_v2_commit_authority(artifact);
        self.kura.store_commit_manifest(manifest)?;
        Ok(())
    }

    #[cfg(test)]
    fn fail_after_kura_store_for_test(&self) {
        self.fail_after_kura_store
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    fn fail_after_wsv_checkpoint_for_test(&self) {
        self.fail_after_wsv_checkpoint
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Fail-closed application or recovery failure.
#[derive(Debug, Error)]
pub(crate) enum V2ApplyError {
    /// Frozen wire input is malformed.
    #[error(transparent)]
    Wire(#[from] wire::ValidationError),
    /// Finality artifact is malformed.
    #[error(transparent)]
    Finality(#[from] wire::finality::V2FinalityValidationError),
    /// Frozen PoPs or the exact CommitQC failed cryptographic verification.
    #[error("invalid Sumeragi v2 durable finality cryptography: {0}")]
    FinalityCryptography(wire::finality::V2QuorumCertificateVerificationError),
    /// Exact-body loading or marker verification failed.
    #[error(transparent)]
    Body(#[from] super::v2_body_store::V2BodyStoreError),
    /// Kura persistence or canonical association failed.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// Apply task and frozen context do not identify one exact decision.
    #[error("Sumeragi v2 Apply task differs from its frozen context or body")]
    TaskMismatch,
    /// Height cannot be represented by local storage indexes.
    #[error("Sumeragi v2 decision height is not representable")]
    HeightOverflow,
    /// WSV is unexpectedly ahead of the decision.
    #[error("WSV height {state_height} is ahead of v2 decision height {decision_height}")]
    StateAhead {
        /// Current WSV height.
        state_height: usize,
        /// Decided height.
        decision_height: usize,
    },
    /// More than one unapplied height separates WSV and the decision.
    #[error("WSV height {state_height} has a gap before v2 decision height {decision_height}")]
    StateGap {
        /// Current WSV height.
        state_height: usize,
        /// Decided height.
        decision_height: usize,
    },
    /// Kura already contains a different block at the decided height.
    #[error("Kura contains a conflicting block at the Sumeragi v2 decision height")]
    KuraConflict,
    /// WSV reports application but Kura has no canonical block.
    #[error("WSV is ahead of Kura while completing a Sumeragi v2 decision")]
    StateAheadOfKura,
    /// Deterministic validation rejected the exact durable body.
    #[error("Sumeragi v2 application validation failed: {0}")]
    Validation(String),
    /// Proposal ingress carried execution results or a result-root commitment.
    #[error("Sumeragi v2 proposal body must be resultless")]
    ResultBearingProposal,
    /// Deterministic validation did not produce the StateBlock execution witness.
    #[error("Sumeragi v2 validation produced no execution witness")]
    ExecutionCommitmentUnavailable,
    /// Execution-witness projection itself was malformed.
    #[error("invalid Sumeragi v2 execution commitment: {0}")]
    ExecutionCommitment(String),
    /// A proposal or executed block could not be encoded canonically.
    #[error("invalid canonical Sumeragi v2 block: {0}")]
    CanonicalBlock(String),
    /// The signed or persisted execution result differs from deterministic replay.
    #[error("Sumeragi v2 execution commitment differs from deterministic validation")]
    ExecutionCommitmentMismatch,
    /// The candidate is otherwise valid but its exact certified merge sidecar
    /// has not reached durable local storage yet.
    #[error("certified merge sidecar `{}` is not available locally yet", reference.entry_hash)]
    MissingCertifiedMergeSidecar {
        /// Compact, certificate-bound reference used for bounded recovery.
        reference: CertifiedMergeLedgerReference,
    },
    /// Certificate-aware block commit conversion failed.
    #[error("Sumeragi v2 block commit conversion failed: {0}")]
    Commit(String),
    /// Kura or WSV crossed the canonical commit point but the complete durable transition failed.
    #[error("Sumeragi v2 committed transition requires restart recovery at {stage}: {detail}")]
    CommittedRecoveryRequired {
        /// Post-commit stage that could not be completed.
        stage: &'static str,
        /// Underlying persistence diagnostic.
        detail: String,
    },
    /// Test-only crash boundary after Kura commits and before WSV publication.
    #[cfg(test)]
    #[error("injected crash after Kura store and before WSV commit")]
    InjectedCrashAfterKuraStore,
    /// Test-only crash boundary after the staged WSV checkpoint and before
    /// live State publication.
    #[cfg(test)]
    #[error("injected crash after staged WSV checkpoint and before WSV commit")]
    InjectedCrashAfterWsvCheckpoint,
}

impl V2ApplyError {
    fn committed_recovery_required(stage: &'static str, error: &impl std::fmt::Display) -> Self {
        Self::CommittedRecoveryRequired {
            stage,
            detail: error.to_string(),
        }
    }

    /// Return whether the live consensus process must stop producing output until restart.
    #[must_use]
    pub(crate) const fn requires_restart_recovery(&self) -> bool {
        match self {
            Self::Kura(error) => error.requires_restart_recovery(),
            Self::CommittedRecoveryRequired { .. } => true,
            #[cfg(test)]
            Self::InjectedCrashAfterKuraStore | Self::InjectedCrashAfterWsvCheckpoint => true,
            _ => false,
        }
    }
}

impl BodyValidationError for V2ApplyError {
    fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        match self {
            Self::MissingCertifiedMergeSidecar { reference } => Some(reference),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::Cow,
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
    };

    use crate::sumeragi::v2_core::{EventTag, Generation};
    use iroha_config::parameters::actual::Queue as QueueConfig;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        Registrable, ValidationFail,
        account::Account,
        block::{
            BlockExecutionContextBundle, BlockHeader, BlockSignature, SignedBlock,
            consensus::{
                CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalV1,
                LaneBlockQcV1,
            },
            consensus_v2 as wire,
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        isi::SetParameter,
        merge::{
            MergeExecutionBatch, MergeLaneExecution, MergeLedgerEntry, MergeQuorumCertificate,
        },
        nexus::{DataSpaceId, LaneId},
        parameter::{Parameter, system::SumeragiParameter},
        peer::PeerId,
        transaction::{
            TransactionBuilder, TransactionEntrypoint,
            error::TransactionRejectionReason,
            signed::{TransactionResult, TransactionResultInner},
        },
        trigger::DataTriggerSequence,
    };
    use mv::storage::StorageReadOnly;
    use norito::codec::Encode as _;

    use super::*;
    use crate::{
        block::BlockBuilder,
        governance::manifest::LaneManifestRegistry,
        query::store::LiveQueryStore,
        queue::{LaneQueueReservationScopeV1, execution_context_for_routing_plan},
        state::{World, WorldReadOnly},
        sumeragi::{
            v2_body_store::{
                BlockSignaturePolicy, DurableBodyReceipt, V2BodyStore, ValidatedBodyReceipt,
            },
            v2_effects::ApplyTask,
        },
        tx::AcceptedTransaction,
    };

    #[test]
    fn restart_recovery_classification_distinguishes_commit_boundaries() {
        assert!(
            V2ApplyError::Kura(crate::kura::Error::DaBlockRewriteCommitStateUnknown {
                detail: "unknown marker".to_owned(),
            })
            .requires_restart_recovery()
        );
        assert!(
            V2ApplyError::Kura(
                crate::kura::Error::CanonicalBlockCommittedRecoveryRequired {
                    detail: "new marker won".to_owned(),
                }
            )
            .requires_restart_recovery()
        );
        assert!(
            V2ApplyError::committed_recovery_required(
                "post-apply metadata",
                &"injected persistence failure",
            )
            .requires_restart_recovery()
        );
        assert!(
            !V2ApplyError::Kura(crate::kura::Error::IO(
                std::io::Error::other("pre-marker retry"),
                std::path::PathBuf::from("pre-marker-stage"),
            ))
            .requires_restart_recovery()
        );
    }

    struct ApplyFixture {
        context: wire::HeightContext,
        body: SignedBlock,
        manifest: wire::PayloadManifest,
        task: ApplyTask,
        service: V2ApplyService,
        state: Arc<State>,
        kura: Arc<Kura>,
        body_root: tempfile::TempDir,
        genesis_key: KeyPair,
    }

    impl ApplyFixture {
        fn new() -> Self {
            Self::new_with_lane_payload(false)
        }

        fn new_with_lane_payload(include_lane_payload: bool) -> Self {
            let chain_id: ChainId = "sumeragi-v2-apply-crash-test".into();
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let transaction_key = KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::Ed25519)
                .expect("deterministic transaction key");
            let roster = keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let context = wire::HeightContext {
                chain_id: chain_id.clone(),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"apply crash fixture Nexus/AMX"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::Plain,
                    chunk_size_bytes: 2 * 1024 * 1024,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 2 * 1024 * 1024,
                    max_chunk_count: 1,
                },
                leader_seed: [0x63; 32],
            };
            context.validate().expect("valid fixture context");

            let kura = Kura::blank_kura_for_testing();
            let transaction_authority = AccountId::new(transaction_key.public_key().clone());
            let world = World::with(
                [],
                [Account::new(transaction_authority.clone()).build(&transaction_authority)],
                [],
            );
            let state = Arc::new(State::new_with_chain_for_testing(
                world,
                Arc::clone(&kura),
                LiveQueryStore::start_test(),
                chain_id.clone(),
            ));
            let nexus = state.nexus_snapshot();
            let lane_manifests = Arc::new(
                LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
            );
            state.install_lane_manifests(&lane_manifests);
            let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
            let queue = Arc::new(Queue::from_config(
                QueueConfig::default(),
                events_sender.clone(),
            ));
            let service = V2ApplyService::new(
                Arc::clone(&state),
                Arc::clone(&queue),
                Arc::clone(&kura),
                chain_id.clone(),
                Duration::from_secs(1),
                transaction_authority.clone(),
                events_sender,
                keys.iter()
                    .map(|key| {
                        iroha_crypto::bls_normal_pop_prove(key.private_key())
                            .expect("fixture validator PoP")
                    })
                    .collect(),
            );

            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            };
            let leader_index = context.leader(0);
            let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
                &state.nexus_snapshot(),
                context.height,
            );
            let confidential_features = {
                let state_view = state.view();
                let digest = crate::state::compute_confidential_feature_digest(
                    state_view.world(),
                    &state_view.zk,
                    state_view.sccp_registry.as_ref(),
                    context.height,
                );
                (!digest.is_empty()).then_some(digest)
            };
            let build_genesis_body =
                |transaction: iroha_data_model::transaction::signed::SignedTransaction,
                 execution_context: Option<BlockExecutionContextBundle>| {
                    let creation_time_ms = (transaction.creation_time() + Duration::from_millis(1))
                        .as_millis()
                        .try_into()
                        .expect("fixture creation time fits u64");
                    let mut header = BlockHeader::new(
                        NonZeroU64::new(1).expect("non-zero fixture height"),
                        None,
                        None,
                        None,
                        creation_time_ms,
                        0,
                    );
                    header.set_confidential_features(confidential_features);
                    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
                    builder.push_transaction(transaction);
                    builder.set_da_proof_policies(Some(proof_policy_bundle.clone()));
                    builder.set_execution_context(execution_context);
                    builder
                        .try_build_with_signature(0, transaction_key.private_key())
                        .expect("sign valid genesis fixture body")
                        .canonical_resultless_proposal()
                };
            let body = if include_lane_payload {
                let transaction = TransactionBuilder::new(
                    chain_id.clone(),
                    transaction_authority,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::MaxClockDriftMs(100),
                ))])
                .sign(transaction_key.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
                let routing_plan = queue
                    .route_plan_with_state(&accepted, state.as_ref())
                    .expect("resolve canonical fixture route");
                let route = routing_plan.coordinator_route();
                let entrypoint_hash = Hash::from(accepted.hash_as_entrypoint());
                let lane_plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
                    state.as_ref(),
                    kura.as_ref(),
                    &context,
                    0,
                    &context.roster[usize::try_from(leader_index).expect("leader index")].validator,
                    std::slice::from_ref(&route),
                    std::slice::from_ref(&entrypoint_hash),
                )
                .expect("derive canonical fixture lane plan");
                assert!(lane_plan.unavailable_indices.is_empty());
                assert_eq!(lane_plan.ownerships.len(), 1);
                let execution_context =
                    BlockExecutionContextBundle::new(vec![execution_context_for_routing_plan(
                        transaction.hash_as_entrypoint(),
                        &routing_plan,
                    )])
                    .with_lane_payload_ownerships(lane_plan.ownerships);
                build_genesis_body(transaction, Some(execution_context))
            } else {
                let transaction = TransactionBuilder::new(
                    chain_id.clone(),
                    transaction_authority,
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([SetParameter::new(Parameter::Sumeragi(
                    SumeragiParameter::MaxClockDriftMs(100),
                ))])
                .sign(transaction_key.private_key());
                build_genesis_body(transaction, None)
            };
            let canonical_wire = body.encode_wire().expect("canonical block wire");
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: body.hash(),
                payload_hash: Hash::new(&canonical_wire),
            };
            let manifest = wire::PayloadManifest::derive(
                &context,
                round,
                subject,
                u64::try_from(canonical_wire.len()).expect("body length"),
                std::slice::from_ref(&canonical_wire),
            )
            .expect("fixture manifest");
            let execution_commitment = service
                .validate_candidate(&context, &body)
                .expect("derive exact fixture execution commitment");
            let mut certificate = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: Vec::new(),
            };
            let preimage = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment,
                signer: 0,
                signature: Vec::new(),
            }
            .signature_preimage();
            let signatures = certificate
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                        &preimage,
                    )
                    .expect("sign fixture Commit vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate fixture Commit votes");

            let body_root = tempfile::tempdir().expect("body-store directory");
            let mut body_store = V2BodyStore::open_with_policy(
                body_root.path(),
                context.clone(),
                BlockSignaturePolicy::GenesisAuthority(transaction_key.public_key().clone()),
            )
            .expect("open body store");
            let durable = body_store
                .store(manifest.clone(), canonical_wire)
                .expect("persist exact body");
            let validated = body_store
                .validate(&durable, |candidate| {
                    service.validate_candidate(&context, candidate)
                })
                .expect("persist production validation marker");
            let task = ApplyTask::for_test(
                1,
                EventTag::new(1, 0, Generation::new(1)),
                subject,
                certificate,
                validated,
            );
            drop(body_store);

            Self {
                context,
                body,
                manifest,
                task,
                service,
                state,
                kura,
                body_root,
                genesis_key: transaction_key,
            }
        }

        fn reopen_body_store(&self) -> V2BodyStore {
            V2BodyStore::open_with_policy(
                self.body_root.path(),
                self.context.clone(),
                BlockSignaturePolicy::GenesisAuthority(
                    self.service.genesis_account.signatory().clone(),
                ),
            )
            .expect("reopen body store after crash")
        }

        fn restart_service_from_last_finalized_snapshot(&self) -> (V2ApplyService, Arc<State>) {
            let authority = self.service.genesis_account.clone();
            let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
            let state = Arc::new(State::new_with_chain_for_testing(
                world,
                Arc::clone(&self.kura),
                LiveQueryStore::start_test(),
                self.service.chain_id.clone(),
            ));
            let nexus = state.nexus_snapshot();
            let lane_manifests = Arc::new(
                LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
            );
            state.install_lane_manifests(&lane_manifests);
            let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
            let queue = Arc::new(Queue::from_config(
                QueueConfig::default(),
                events_sender.clone(),
            ));
            let service = V2ApplyService::new(
                Arc::clone(&state),
                queue,
                Arc::clone(&self.kura),
                self.service.chain_id.clone(),
                self.service.block_cadence,
                authority,
                events_sender,
                self.service.validator_set_pops.clone(),
            );
            (service, state)
        }

        fn execute(&self, store: &mut V2BodyStore) -> Result<(), V2ApplyError> {
            self.service
                .execute(&self.context, store, &self.task)
                .map(drop)
        }

        fn assert_no_post_apply_sidecars(&self) {
            assert!(
                self.kura
                    .wsv_checkpoint(self.context.height)
                    .expect("read checkpoint")
                    .is_none()
            );
            assert!(
                self.kura
                    .commit_manifest(self.context.height)
                    .expect("read manifest")
                    .is_none()
            );
            assert!(
                self.kura
                    .v2_finality_artifact(self.context.height)
                    .expect("read finality")
                    .is_none()
            );
        }

        fn assert_no_apply_mutation(&self) {
            assert_eq!(self.state.committed_height(), 0);
            assert_eq!(self.kura.exact_durable_blocks_count().unwrap(), 0);
            self.assert_no_post_apply_sidecars();
        }

        fn assert_complete(&self) {
            assert_eq!(self.state.committed_height(), 1);
            assert_eq!(self.kura.exact_durable_blocks_count().unwrap(), 1);
            assert_eq!(
                self.kura
                    .get_durable_block_hash(NonZeroUsize::new(1).expect("height")),
                Some(self.body.hash())
            );
            let durable = self
                .kura
                .get_block(NonZeroUsize::new(1).expect("height"))
                .expect("read complete durable block");
            assert!(durable.has_results());
            assert_eq!(
                durable.results().len(),
                self.body.external_entrypoint_count()
            );
            assert!(durable.results().all(|result| result.is_ok()));
            assert_eq!(durable.execution_context(), self.body.execution_context());
            assert!(
                self.kura
                    .wsv_checkpoint(self.context.height)
                    .expect("read checkpoint")
                    .is_some()
            );
            let commit_manifest = self
                .kura
                .commit_manifest(self.context.height)
                .expect("read manifest")
                .expect("commit manifest exists");
            let artifact = self
                .kura
                .v2_finality_artifact(self.context.height)
                .expect("read finality")
                .expect("finality exists");
            assert_eq!(artifact.height_context, self.context);
            assert_eq!(artifact.subject, self.manifest.subject);
            assert_eq!(artifact.commit_qc, self.task.certificate().clone());
            assert!(
                self.kura
                    .commit_manifest_has_wsv_binding(&commit_manifest)
                    .expect("read checkpoint-to-manifest binding")
            );
            assert!(
                commit_manifest.binds_authenticated_v2_commit_authority(&artifact),
                "manifest must retain the exact QC roots and complete v2 authority seal"
            );
            assert!(
                self.state
                    .world_view()
                    .commit_qcs()
                    .get(&self.body.hash())
                    .is_none(),
                "Sumeragi v2 finality must not be projected into the legacy commit-QC store"
            );
            assert!(
                self.state
                    .commit_roster_snapshot_for_block(self.context.height, self.body.hash())
                    .is_none(),
                "Sumeragi v2 finality must not populate the legacy commit-roster journal"
            );
            assert!(
                self.kura
                    .read_roster_metadata(self.context.height)
                    .is_none(),
                "Sumeragi v2 finality must not populate the legacy roster sidecar"
            );
        }
    }

    #[test]
    fn durable_application_evidence_rejects_identity_mutations() {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        let completion = fixture
            .service
            .execute(&fixture.context, &mut store, &fixture.task)
            .expect("apply exact fixture");
        let committed = fixture
            .kura
            .get_block(NonZeroUsize::new(1).expect("height"))
            .expect("load committed block");
        let artifact = completion.artifact().clone();
        let evidence = DurableApplicationEvidence {
            task_tag: fixture.task.tag(),
            owner_tag: fixture.task.authorized_owner_tag(),
            task_generation: fixture.task.tag().generation().get(),
            task_work_id: fixture.task.id(),
            context: fixture.context.clone(),
            commit_qc: fixture.task.certificate().clone(),
            subject: fixture.task.subject(),
            execution_commitment: fixture.task.validated_receipt().execution_commitment(),
            validated_receipt: fixture.task.validated_receipt().clone(),
            validated_manifest_hash: fixture.task.validated_receipt().durable().manifest_hash(),
            validated_body_frame_hash: fixture.task.validated_receipt().durable().frame_hash(),
            proposal_block_hash: fixture.body.hash(),
            canonical_proposal_wire_hash: fixture
                .body
                .canonical_proposal_wire_hash()
                .expect("hash proposal wire"),
            committed_block_hash: committed.hash(),
            executed_block_wire_hash: committed
                .executed_block_wire_hash()
                .expect("hash executed wire"),
            kura_receipt: completion.receipt().clone(),
            artifact_hash: HashOf::new(&artifact),
            artifact,
            completion_work_id: completion.work_id(),
            state_height_after: fixture.state.committed_height(),
        };
        assert!(evidence.is_exact());
        assert_eq!(evidence.task_tag(), fixture.task.tag());
        assert_eq!(evidence.owner_tag(), fixture.task.authorized_owner_tag());
        assert_eq!(
            evidence.task_generation(),
            fixture.task.tag().generation().get()
        );
        assert_eq!(evidence.task_work_id(), fixture.task.id());
        assert_eq!(evidence.context(), &fixture.context);
        assert_eq!(evidence.commit_qc(), fixture.task.certificate());
        assert_eq!(evidence.commit_round(), fixture.task.certificate().round);
        assert_eq!(evidence.commit_phase(), wire::GlobalPhase::Commit);
        assert_eq!(
            evidence.commit_signers(),
            fixture.task.certificate().signers.as_slice()
        );
        assert_eq!(
            evidence.commit_aggregate_signature(),
            fixture.task.certificate().aggregate_signature.as_slice()
        );
        assert_eq!(evidence.subject(), fixture.task.subject());
        assert_eq!(
            evidence.execution_commitment(),
            fixture.task.certificate().execution_commitment
        );
        assert_eq!(
            evidence.validated_receipt(),
            fixture.task.validated_receipt()
        );
        assert_eq!(
            evidence.validated_context_id(),
            fixture.task.validated_receipt().durable().context_id()
        );
        assert_eq!(
            evidence.validated_round(),
            fixture.task.validated_receipt().durable().round()
        );
        assert_eq!(evidence.validated_subject(), fixture.task.subject());
        assert_eq!(
            evidence.validated_manifest_hash(),
            fixture.task.validated_receipt().durable().manifest_hash()
        );
        assert_eq!(
            evidence.validated_body_frame_hash(),
            fixture.task.validated_receipt().durable().frame_hash()
        );
        assert_eq!(evidence.proposal_block_hash(), fixture.body.hash());
        assert_eq!(
            evidence.canonical_proposal_wire_hash(),
            fixture.manifest.subject.payload_hash
        );
        assert_eq!(evidence.committed_block_hash(), committed.hash());
        assert_eq!(
            evidence.executed_block_wire_hash(),
            fixture
                .task
                .certificate()
                .execution_commitment
                .executed_block_wire_hash
        );
        assert_eq!(evidence.kura_height(), fixture.context.height);
        assert_eq!(evidence.kura_block_hash(), committed.hash());
        assert_eq!(evidence.kura_context_id(), fixture.context.id());
        assert_eq!(evidence.kura_subject(), fixture.task.subject());
        assert_eq!(
            evidence.kura_certificate(),
            fixture.task.certificate().as_ref()
        );
        assert_eq!(evidence.kura_artifact_hash(), evidence.artifact_hash());
        assert_eq!(evidence.artifact(), completion.artifact());
        assert_eq!(evidence.completion_work_id(), completion.work_id());
        assert_eq!(evidence.state_height_after(), 1);
        assert!(
            fixture
                .service
                .finish_durable_apply_completion(evidence.clone())
                .is_ok(),
            "the exact native evidence must mint the typed completion"
        );

        let mut delayed_decision = evidence.clone();
        delayed_decision.task_tag = EventTag::new(
            delayed_decision.task_tag.height(),
            delayed_decision
                .task_tag
                .view()
                .checked_add(1)
                .expect("fixture lifecycle view increment"),
            Generation::new(
                delayed_decision
                    .task_generation
                    .checked_add(1)
                    .expect("fixture lifecycle generation increment"),
            ),
        );
        delayed_decision.owner_tag = delayed_decision.task_tag;
        delayed_decision.task_generation = delayed_decision.task_tag.generation().get();
        assert!(
            delayed_decision.is_exact(),
            "a current lifecycle owner must retain an exact historical CommitQC"
        );
        assert!(
            fixture
                .service
                .finish_durable_apply_completion(delayed_decision)
                .is_ok(),
            "a delayed CommitQC must mint the typed completion after a timeout fence"
        );

        let mut altered = evidence.clone();
        altered.owner_tag = EventTag::new(
            altered.task_tag.height(),
            altered
                .task_tag
                .view()
                .checked_add(1)
                .expect("fixture owner view increment"),
            altered.task_tag.generation(),
        );
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.task_generation = altered
            .task_generation
            .checked_add(1)
            .expect("fixture generation increment");
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.commit_qc.signers.swap(0, 1);
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.commit_qc.aggregate_signature.push(0xC1);
        assert!(!altered.is_exact());

        let alternate_durable = DurableBodyReceipt::for_test(
            fixture.context.id(),
            fixture.task.certificate().round,
            fixture.task.subject(),
            fixture.task.validated_receipt().durable().manifest_hash(),
        );
        assert_ne!(
            alternate_durable.frame_hash(),
            fixture.task.validated_receipt().durable().frame_hash()
        );
        let mut altered = evidence.clone();
        altered.validated_receipt = ValidatedBodyReceipt::for_test_with_commitment(
            alternate_durable,
            evidence.execution_commitment(),
        );
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.validated_manifest_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"altered validated manifest identity"));
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.validated_body_frame_hash = Hash::new(b"altered validated body frame identity");
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.canonical_proposal_wire_hash = Hash::new(b"altered proposal wire identity");
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.executed_block_wire_hash = Hash::new(b"altered executed wire identity");
        assert!(!altered.is_exact());

        let mut altered_artifact = evidence.artifact.clone();
        altered_artifact.block_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"altered Kura receipt block identity"));
        let mut altered = evidence.clone();
        altered.kura_receipt = KuraV2CommitReceipt::for_test(&altered_artifact);
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.artifact_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"altered finality artifact identity"));
        assert!(!altered.is_exact());

        let mut altered = evidence.clone();
        altered.completion_work_id = EffectWorkId::for_test(2);
        assert!(matches!(
            fixture.service.finish_durable_apply_completion(altered),
            Err(V2ApplyError::CommittedRecoveryRequired {
                stage: "exact application evidence",
                ..
            })
        ));

        let mut altered = evidence;
        altered.state_height_after = 2;
        assert!(!altered.is_exact());
    }

    fn pending_merge_entry(
        context: &wire::HeightContext,
        view: wire::View,
        label: &[u8],
    ) -> MergeLedgerEntry {
        let validator_set = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let mut bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
        for index in 0..validator_set.len() {
            bitmap[index / 8] |= 1 << (index % 8);
        }
        MergeLedgerEntry {
            version: MergeLedgerEntry::VERSION,
            epoch_id: context.epoch,
            lane_catalog_hash: Hash::new(b"v2 apply decided-sidecar catalog"),
            active_lanes: Vec::new(),
            incarnation_root: Hash::new(b"v2 apply decided-sidecar incarnations"),
            activation_root: Hash::new(b"v2 apply decided-sidecar activations"),
            lane_snapshots: Vec::new(),
            lane_drain_certificates: Vec::new(),
            execution_batch: None,
            global_state_root: Hash::new(label),
            merge_qc: MergeQuorumCertificate::new(
                view,
                context.epoch,
                context.height,
                HashOf::from_untyped_unchecked(Hash::new(b"v2 apply decided-sidecar parent")),
                Hash::new(b"v2 apply decided-sidecar chain"),
                VALIDATOR_SET_HASH_VERSION_V1,
                HashOf::new(&validator_set),
                validator_set,
                bitmap,
                Vec::new(),
                vec![0x5A; 96],
                Hash::new(label),
            ),
        }
    }

    fn merge_entry_with_reservation(
        context: &wire::HeightContext,
        entrypoint: TransactionEntrypoint,
        reservation: crate::queue::LaneQueueReservationKeyV1,
    ) -> MergeLedgerEntry {
        let entrypoint_hashes = vec![Hash::from(entrypoint.hash())];
        let results = vec![TransactionResult::from(Ok(DataTriggerSequence::default()))];
        let result_hashes = results
            .iter()
            .map(|result| Hash::from(result.hash()))
            .collect::<Vec<_>>();
        let validator_set = Vec::<PeerId>::new();
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: reservation.lane_id,
            dataspace_id: reservation.dataspace_id,
            lane_incarnation: reservation.lane_incarnation,
            proposal_height: reservation.proposal_height,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: reservation.lane_block_height,
            lane_block_view: reservation.lane_block_view,
            subject_hash: Hash::new(b"v2 reservation fixture subject"),
            payload_ownership_hash: Hash::new(b"v2 reservation fixture ownership"),
            rbc_instance_hash: Hash::new(b"v2 reservation fixture RBC"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: entrypoint_hashes.clone(),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: 0,
            min_quorum: 0,
            qc_mode_tag: "v2-reservation-lifecycle-test".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let lane_qc = |phase| LaneBlockQcV1 {
            body: proposal.vote_body(phase),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            signers_bitmap: Vec::new(),
            bls_aggregate_signature: Vec::new(),
            payload_availability_qc: None,
        };
        let settlement_commitment = LaneBlockCommitment {
            block_height: reservation.lane_block_height,
            lane_id: reservation.lane_id,
            lane_incarnation: reservation.lane_incarnation,
            dataspace_id: reservation.dataspace_id,
            tx_count: 0,
            total_local_amount: "0".parse().expect("valid settlement quantity"),
            total_xor_due: "0".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
            total_xor_variance: "0".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let routing_plan = crate::queue::RoutingPlan::single(RoutingDecision::new(
            reservation.lane_id,
            reservation.dataspace_id,
        ));
        assert_eq!(
            routing_plan.digest(),
            reservation.routing_plan_digest,
            "fixture routing plan must match the durable reservation"
        );
        let execution = MergeLaneExecution {
            source_bundle: vec![1],
            source_bundle_hash: Hash::new(b"v2 reservation fixture source"),
            proposal: proposal.clone(),
            origin_proposal: proposal.clone(),
            prepare_qc: lane_qc(CertPhase::Prepare),
            commit_qc: lane_qc(CertPhase::Commit),
            signer_proofs: Vec::new(),
            autonomous_chain_id_hash: Hash::new(b"v2 reservation fixture chain"),
            autonomous_epoch: 0,
            autonomous_payload_hash: Hash::new(b"v2 reservation fixture payload"),
            entrypoint_hashes,
            entrypoints: vec![entrypoint],
            reservation_keys: vec![
                norito::to_bytes(&reservation)
                    .expect("fixture reservation key has canonical framed Norito bytes"),
            ],
            routing_plans: vec![
                norito::to_bytes(&routing_plan)
                    .expect("fixture routing plan has canonical framed Norito bytes"),
            ],
            native_amx_receipts: vec![None],
            result_hashes,
            results,
            settlement_hash: iroha_data_model::nexus::compute_settlement_hash(
                &settlement_commitment,
            )
            .expect("fixture settlement hashes canonically"),
            settlement_commitment,
        };
        let lanes = vec![execution];
        let base_state_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"v2 reservation fixture base state"));
        let write_set_root = Hash::new(b"v2 reservation fixture write set");
        let mut batch = MergeExecutionBatch {
            version: 1,
            base_state_height: 0,
            base_state_hash,
            application_block_header: BlockHeader::new(
                NonZeroU64::new(1).expect("non-zero fixture carrier"),
                None,
                None,
                None,
                1,
                0,
            ),
            entrypoint_count: 1,
            entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(&lanes)
                .expect("fixture has one entrypoint"),
            result_merkle_root: crate::merge::merge_execution_result_merkle_root(&lanes)
                .expect("fixture has one result"),
            execution_root: crate::merge::merge_execution_root(&lanes),
            lanes,
            application_write_set_root: Hash::new(b"v2 reservation fixture application writes"),
            write_set_root,
            expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
                0,
                base_state_hash,
                write_set_root,
            ),
            batch_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
        let mut entry = pending_merge_entry(context, 0, b"v2 reservation fixture merge entry");
        entry.epoch_id = 1;
        entry.merge_qc.epoch_id = 1;
        entry.execution_batch = Some(batch);
        entry
    }

    fn reserve_transaction_for_test(
        state: &State,
        queue: &Queue,
        transaction: iroha_data_model::transaction::SignedTransaction,
    ) -> (
        crate::queue::LaneQueueReservationKeyV1,
        TransactionEntrypoint,
    ) {
        let entrypoint = TransactionEntrypoint::External(transaction.clone());
        queue
            .push_with_lane_with_state(
                AcceptedTransaction::new_unchecked(Cow::Owned(transaction)),
                state,
            )
            .expect("enqueue reservation fixture transaction");
        let scope = LaneQueueReservationScopeV1 {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation: state
                .lane_incarnation_at_height(LaneId::SINGLE, 1)
                .expect("default lane incarnation at first proposal height"),
            proposal_height: 1,
            lane_block_height: 1,
            lane_block_view: 0,
            reservation_owner_hash: Hash::new(b"v2 reservation fixture owner"),
            proposal_identity_hash: Hash::new(b"v2 reservation fixture proposal"),
        };
        let reserved = queue
            .reserve_transactions_for_lane(
                state,
                scope,
                NonZeroUsize::new(1).expect("non-zero reservation limit"),
            )
            .expect("reserve exact fixture transaction");
        assert_eq!(reserved.len(), 1);
        (*reserved[0].key(), entrypoint)
    }

    fn body_with_merge_reference(reference: CertifiedMergeLedgerReference) -> SignedBlock {
        let key = KeyPair::try_from_seed(vec![0xC9; 32], Algorithm::BlsNormal)
            .expect("derive decided-body signer");
        let execution_context =
            BlockExecutionContextBundle::new(Vec::new()).with_merge_entry(reference);
        let block = BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
            .chain(0, None)
            .with_execution_context(Some(execution_context))
            .try_sign_with_index(key.private_key(), 0)
            .expect("sign decided body")
            .unpack(|_| {});
        SignedBlock::from(block)
    }

    macro_rules! v2_apply_test {
        ($name:ident, $body:block) => {
            #[test]
            fn $name() {
                let handle = crate::sumeragi::sumeragi_thread_builder(concat!(
                    "sumeragi-v2-apply-test-",
                    stringify!($name)
                ))
                .spawn(move || $body)
                .expect("spawn v2 apply test on the production consensus stack");
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
            }
        };
    }

    v2_apply_test!(merge_publication_emits_once_across_exact_retry, {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.execute(&mut store).expect("commit carrier parent");

        let mut entry =
            pending_merge_entry(&fixture.context, 0, b"v2 apply live publication fixture");
        entry.epoch_id = 1;
        entry.merge_qc.epoch_id = 1;
        entry.merge_qc.carrier_height = 2;
        entry.merge_qc.carrier_parent_hash = fixture.body.hash();
        entry.merge_qc.view = 0;

        let execution_context = BlockExecutionContextBundle::new(Vec::new())
            .with_merge_entry(CertifiedMergeLedgerReference::new(&entry));
        let carrier = BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
            .chain(0, Some(&fixture.body))
            .with_execution_context(Some(execution_context))
            .try_sign_with_index(fixture.genesis_key.private_key(), 0)
            .expect("sign merge carrier")
            .unpack(|_| {});
        let carrier = SignedBlock::from(carrier);
        fixture
            .kura
            .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
            .expect("persist exact merge carrier and sidecar");
        fixture
            .state
            .seed_applied_merge_entry_for_v2_settlement_test(&entry)
            .expect("seed exact post-commit merge state");
        let mut block_hashes = fixture.state.block_hashes.block();
        block_hashes.push_for_tests(carrier.hash());
        block_hashes.commit_for_tests();
        fixture
            .state
            .update_latest_block_header_cache_for_tests(carrier.header().clone());

        let mut events = fixture.service.events_sender.subscribe();
        fixture
            .service
            .publish_committed_block_merge_entry(&carrier)
            .expect("publish live merge entry");
        let event = events.try_recv().expect("receive live merge event");
        let EventBox::Pipeline(iroha_data_model::events::pipeline::PipelineEventBox::Merge(event)) =
            event
        else {
            panic!("v2 apply must publish the merge-ledger event");
        };
        assert_eq!(event.entry, entry);
        assert_eq!(fixture.state.merge_ledger.snapshot().len(), 1);

        fixture
            .service
            .publish_committed_block_merge_entry(&carrier)
            .expect("retry exact live merge publication");
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        assert_eq!(fixture.state.merge_ledger.snapshot().len(), 1);
    });

    v2_apply_test!(committed_merge_reservation_is_finalized_exactly_once, {
        let fixture = ApplyFixture::new();
        let transaction = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("reservation journal directory");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let (reservation, entrypoint) =
            reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
        let entry = merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
        fixture
            .kura
            .append_merge_entry(&entry)
            .expect("persist committed merge history fixture");
        let carrier = body_with_merge_reference(CertifiedMergeLedgerReference::new(&entry));
        fixture.state.record_direct_committed_transactions(
            [reservation.signed_transaction_hash],
            NonZeroUsize::new(1).expect("committed height"),
        );

        assert_eq!(
            finalize_committed_block_merge_reservations(
                fixture.state.as_ref(),
                &queue,
                fixture.kura.as_ref(),
                &carrier,
            )
            .expect("finalize committed merge reservation"),
            1
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert_eq!(
            finalize_committed_block_merge_reservations(
                fixture.state.as_ref(),
                &queue,
                fixture.kura.as_ref(),
                &carrier,
            )
            .expect("repeat exact reservation finalization"),
            0,
            "the post-commit boundary must be idempotent"
        );
    });

    v2_apply_test!(committed_merge_reservation_rejects_bare_norito, {
        let fixture = ApplyFixture::new();
        let transaction = fixture
            .body
            .external_transactions()
            .next()
            .expect("fixture transaction")
            .clone();
        let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
        let queue = Queue::from_config(QueueConfig::default(), events_sender);
        let journal_dir = tempfile::tempdir().expect("reservation journal directory");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let (reservation, entrypoint) =
            reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
        let mut entry = merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
        let encoded = &mut entry
            .execution_batch
            .as_mut()
            .expect("fixture execution batch")
            .lanes[0]
            .reservation_keys[0];
        let bare = reservation.encode();
        assert_ne!(
            *encoded, bare,
            "framed and bare Norito must remain distinct"
        );
        *encoded = bare;
        fixture.state.record_direct_committed_transactions(
            [reservation.signed_transaction_hash],
            NonZeroUsize::new(1).expect("committed height"),
        );

        let error = finalize_certified_merge_reservations(fixture.state.as_ref(), &queue, &entry)
            .expect_err("bare reservation metadata must fail closed");
        let message = match error {
            V2ReservationLifecycleError::Merge(MergeLedgerCommitError::ExecutionBatchInvalid(
                message,
            )) => message,
            unexpected => panic!("unexpected bare-reservation error: {unexpected}"),
        };
        assert!(
            message.contains("framed Norito"),
            "diagnostic should identify the required framing: {message}"
        );
        assert_eq!(
            queue.live_lane_reservations(),
            vec![reservation],
            "malformed committed evidence must not consume queue ownership"
        );
    });

    v2_apply_test!(
        startup_reconciliation_consumes_replayed_committed_merge_reservation,
        {
            let fixture = ApplyFixture::new();
            let transaction = fixture
                .body
                .external_transactions()
                .next()
                .expect("fixture transaction")
                .clone();
            let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
            let journal_dir = tempfile::tempdir().expect("reservation journal directory");
            let journal_path = journal_dir.path().join("lane-reservations.norito");
            let first_queue = Queue::from_config(QueueConfig::default(), events_sender.clone());
            first_queue
                .install_lane_reservation_journal(&journal_path, 1024 * 1024)
                .expect("install first-process reservation journal");
            let (reservation, entrypoint) =
                reserve_transaction_for_test(fixture.state.as_ref(), &first_queue, transaction);
            let entry = merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
            fixture
                .kura
                .append_merge_entry(&entry)
                .expect("persist committed merge history fixture");
            fixture.state.record_direct_committed_transactions(
                [reservation.signed_transaction_hash],
                NonZeroUsize::new(1).expect("committed height"),
            );
            drop(first_queue);

            let replayed_queue = Queue::from_config(QueueConfig::default(), events_sender);
            let replay = replayed_queue
                .install_lane_reservation_journal(&journal_path, 1024 * 1024)
                .expect("replay first-process reservation journal");
            assert_eq!(replay.restored, 1);
            assert_eq!(replayed_queue.live_lane_reservations(), vec![reservation]);

            assert_eq!(
                reconcile_lane_reservation_ownership(
                    fixture.state.as_ref(),
                    &replayed_queue,
                    fixture.kura.as_ref(),
                    &fixture.context.chain_id,
                )
                .expect("reconcile replayed committed reservation"),
                (1, 0)
            );
            assert!(replayed_queue.live_lane_reservations().is_empty());
            assert_eq!(
                reconcile_lane_reservation_ownership(
                    fixture.state.as_ref(),
                    &replayed_queue,
                    fixture.kura.as_ref(),
                    &fixture.context.chain_id,
                )
                .expect("repeat startup reconciliation"),
                (0, 0)
            );
        }
    );

    v2_apply_test!(
        durable_decision_retains_exact_earlier_view_sidecar_and_prunes_losers,
        {
            let fixture = ApplyFixture::new();
            let exact = pending_merge_entry(&fixture.context, 1, b"exact earlier-view sidecar");
            let losing = pending_merge_entry(&fixture.context, 2, b"losing later-view sidecar");
            let exact_hash = fixture
                .kura
                .persist_pending_certified_merge_entry(&exact)
                .expect("persist exact decided sidecar");
            let losing_hash = fixture
                .kura
                .persist_pending_certified_merge_entry(&losing)
                .expect("persist losing sidecar");
            assert_ne!(exact_hash, losing_hash);

            let body = body_with_merge_reference(CertifiedMergeLedgerReference::new(&exact));
            fixture
                .service
                .retain_decided_merge_sidecar(&fixture.context, &body)
                .expect("bind exact sidecar from durable decided body");
            assert_eq!(
                fixture
                    .kura
                    .merge_entry_by_hash(exact_hash)
                    .expect("read exact sidecar after decision binding"),
                Some(exact),
                "the exact earlier-view reference remains protected until finalization"
            );
            assert!(
                fixture
                    .kura
                    .merge_entry_by_hash(losing_hash)
                    .expect("read losing sidecar after decision binding")
                    .is_none(),
                "a durable decision must release every non-referenced sidecar at its height"
            );

            fixture
                .kura
                .prune_finalized_pending_certified_merge_entries(fixture.context.height)
                .expect("finalized height retires the exact protected sidecar");
            assert!(
                fixture
                    .kura
                    .merge_entry_by_hash(exact_hash)
                    .expect("read exact sidecar after finalization")
                    .is_none()
            );
        }
    );

    v2_apply_test!(forged_commit_qc_is_rejected_before_any_durable_mutation, {
        let fixture = ApplyFixture::new();
        let pending = pending_merge_entry(
            &fixture.context,
            2,
            b"pending sidecar must survive unauthenticated Apply",
        );
        let pending_hash = fixture
            .kura
            .persist_pending_certified_merge_entry(&pending)
            .expect("persist pending sidecar before forged Apply");
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());

        let mut forged_certificate = fixture.task.certificate().clone();
        let first_signature_byte = forged_certificate
            .aggregate_signature
            .first_mut()
            .expect("fixture CommitQC aggregate signature");
        *first_signature_byte ^= 0x80;
        let forged_task = ApplyTask::for_test(
            2,
            fixture.task.tag(),
            fixture.task.subject(),
            forged_certificate,
            fixture.task.validated_receipt().clone(),
        );
        let mut store = fixture.reopen_body_store();

        assert!(matches!(
            fixture
                .service
                .execute(&fixture.context, &mut store, &forged_task),
            Err(V2ApplyError::FinalityCryptography(
                wire::finality::V2QuorumCertificateVerificationError::InvalidAggregateSignature
            ))
        ));
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "an unauthenticated decision must not mutate WSV"
        );
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
        fixture.assert_no_post_apply_sidecars();
        assert_eq!(
            fixture
                .kura
                .merge_entry_by_hash(pending_hash)
                .expect("read pending sidecar after forged Apply"),
            Some(pending),
            "finality verification must precede pending-sidecar pruning"
        );
    });

    v2_apply_test!(
        invalid_commit_aggregate_is_rejected_before_kura_or_wsv_mutation,
        {
            let fixture = ApplyFixture::new();
            let mut certificate = fixture.task.certificate().clone();
            certificate.aggregate_signature[0] ^= 0x80;
            let task = ApplyTask::for_test(
                2,
                fixture.task.tag(),
                fixture.task.subject(),
                certificate,
                fixture.task.validated_receipt().clone(),
            );
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
                fixture.service.execute(&fixture.context, &mut store, &task),
                Err(V2ApplyError::FinalityCryptography(
                    wire::finality::V2QuorumCertificateVerificationError::InvalidAggregateSignature
                ))
            ));
            fixture.assert_no_apply_mutation();
        }
    );

    v2_apply_test!(
        resigned_later_view_commit_qc_applies_exact_locked_origin_body,
        {
            let mut fixture = ApplyFixture::new();
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let mut certificate = fixture.task.certificate().clone();
            certificate.round.view = fixture.body.header().view_change_index().saturating_add(1);
            let preimage = wire::Vote {
                round: certificate.round,
                proposal_round: certificate.proposal_round,
                phase: certificate.phase,
                subject: certificate.subject,
                execution_commitment: certificate.execution_commitment,
                signer: certificate.signers[0],
                signature: Vec::new(),
            }
            .signature_preimage();
            let signatures = certificate
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                        &preimage,
                    )
                    .expect("sign later-view Commit vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate later-view Commit votes");
            let later_round = certificate.round;
            let later_tag = EventTag::new(
                fixture.context.height,
                later_round.view,
                fixture.task.tag().generation(),
            );
            let mut store = fixture.reopen_body_store();
            let canonical_wire = fixture.body.encode_wire().expect("encode locked body");
            let later_manifest = wire::PayloadManifest::derive(
                &fixture.context,
                later_round,
                fixture.task.subject(),
                u64::try_from(canonical_wire.len()).expect("locked body length fits u64"),
                std::slice::from_ref(&canonical_wire),
            )
            .expect("derive later-view manifest for the exact locked body");
            let later_durable = store
                .store(later_manifest, canonical_wire)
                .expect("bind the exact locked body to the later round");
            let later_validated = store
                .validate(&later_durable, |candidate| {
                    fixture
                        .service
                        .validate_candidate(&fixture.context, candidate)
                })
                .expect("validate the exact locked body under the later round");
            assert_eq!(
                later_validated.execution_commitment(),
                fixture.task.validated_receipt().execution_commitment(),
                "view rotation must not change deterministic execution"
            );
            let task = ApplyTask::for_test(
                2,
                later_tag,
                fixture.task.subject(),
                certificate,
                later_validated,
            );
            fixture.task = task;

            fixture
                .execute(&mut store)
                .expect("later-view CommitQC applies the exact locked origin body");
            fixture.assert_complete();
        }
    );

    v2_apply_test!(
        invalid_non_signer_durable_pop_is_rejected_before_kura_or_wsv_mutation,
        {
            let mut fixture = ApplyFixture::new();
            fixture.service.validator_set_pops[3][0] ^= 0x80;
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::FinalityCryptography(
                wire::finality::V2QuorumCertificateVerificationError::InvalidProofOfPossession {
                    index: 3
                }
            ))
        ));
            fixture.assert_no_apply_mutation();
        }
    );

    v2_apply_test!(block_write_failure_never_advances_wsv_and_retry_is_exact, {
        let fixture = ApplyFixture::new();
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_block_write_for_tests();
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::Kura(_))
        ));
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "a failed Kura write must not leak any WSV mutation"
        );
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
        fixture.assert_no_post_apply_sidecars();

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        fixture
            .execute(&mut reopened)
            .expect("retry exact apply after reopening the durable body store");
        fixture.assert_complete();
        let view = fixture.state.view();
        let sumeragi = view.world().parameters().sumeragi();
        assert_eq!(sumeragi.block_cadence_ms().get(), 100);
    });

    v2_apply_test!(height_one_lane_exemption_never_accepts_empty_genesis, {
        let fixture = ApplyFixture::new();
        let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
            &fixture.state.nexus_snapshot(),
            fixture.context.height,
        );
        let invalid = BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
            .chain(0, None)
            .with_da_proof_policies(Some(proof_policy_bundle))
            .try_sign_with_index(fixture.genesis_key.private_key(), 0)
            .expect("sign empty genesis negative fixture")
            .unpack(|_| {});
        let error = fixture
            .service
            .validate_candidate(&fixture.context, &SignedBlock::from(invalid))
            .expect_err("canonical genesis validation must reject an empty body");
        assert!(
            matches!(&error, V2ApplyError::Validation(message) if message.contains("must have 1 to 16 transactions")),
            "unexpected empty-genesis rejection: {error}"
        );
    });

    v2_apply_test!(
        validation_error_classification_handles_body_without_results,
        {
            let key = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::Ed25519)
                .expect("derive malformed-body signer");
            let body = SignedBlock::from(
                BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
                    .chain(0, None)
                    .try_sign_with_index(key.private_key(), 0)
                    .expect("sign no-results body")
                    .unpack(|_| {}),
            );
            assert!(!body.has_results());
            let error = V2ApplyService::classify_candidate_validation_error(
                None,
                &body,
                &BlockValidationError::EmptyBlock,
            );
            assert!(
                matches!(error, V2ApplyError::Validation(message) if message.contains("no committed overlays"))
            );
        }
    );

    v2_apply_test!(
        validation_error_classification_redacts_internal_result_details,
        {
            let fixture = ApplyFixture::new();
            let mut rejected = fixture.body.clone();
            let entry_hashes = rejected
                .external_entrypoints_cloned()
                .map(|entrypoint| entrypoint.hash())
                .collect::<Vec<_>>();
            let secret = "sensitive executor diagnostic";
            let result: TransactionResultInner = Err(TransactionRejectionReason::Validation(
                ValidationFail::InternalError(secret.to_owned()),
            ));
            rejected
                .set_transaction_results(Vec::new(), &entry_hashes, vec![result])
                .expect("attach one rejected result");
            let error = V2ApplyService::classify_candidate_validation_error(
                None,
                &rejected,
                &BlockValidationError::EmptyBlock,
            );
            let V2ApplyError::Validation(message) = error else {
                panic!("unexpected classification")
            };
            assert!(message.contains("rejected transaction result count: 1"));
            assert!(!message.contains(secret));
        }
    );

    v2_apply_test!(
        post_genesis_external_body_without_execution_context_is_rejected,
        {
            let fixture = ApplyFixture::new();
            let mut post_genesis_context = fixture.context.clone();
            post_genesis_context.height = 2;
            let error = fixture
                .service
                .validate_lane_payload_plan(&post_genesis_context, &fixture.body)
                .expect_err("the height-one lane-plan exemption must never apply post-genesis");
            assert!(
                matches!(&error, V2ApplyError::Validation(message) if message.contains("external entrypoints without execution context")),
                "unexpected post-genesis lane-plan rejection: {error}"
            );
        }
    );

    v2_apply_test!(restart_recovers_kura_block_written_before_wsv_commit, {
        let fixture = ApplyFixture::new();
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        let mut store = fixture.reopen_body_store();
        fixture.service.fail_after_kura_store_for_test();
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::InjectedCrashAfterKuraStore)
        ));
        drop(store);
        let durable = fixture
            .kura
            .get_block(NonZeroUsize::new(1).expect("height"))
            .expect("read production-validated Kura crash image");
        assert!(durable.has_results());
        assert_eq!(durable.results().len(), 1);
        assert!(durable.results().all(|result| result.is_ok()));
        let durable_wire = durable.encode_wire().expect("encode Kura crash image");
        fixture.assert_no_post_apply_sidecars();
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "the Kura-first crash boundary must not leak partial WSV state"
        );

        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("resume WSV application from exact durable body");
        fixture.assert_complete();
        assert_eq!(
            fixture
                .kura
                .get_block(NonZeroUsize::new(1).expect("height"))
                .expect("read recovered Kura block")
                .encode_wire()
                .expect("encode recovered Kura block"),
            durable_wire,
            "an exact retry must preserve the complete canonical Kura wire"
        );
    });

    v2_apply_test!(restart_recovers_kura_lane_body_written_before_wsv_commit, {
        let fixture = ApplyFixture::new_with_lane_payload(true);
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        let ownerships = fixture
            .body
            .execution_context()
            .expect("lane body execution context")
            .lane_payload_ownerships
            .clone();
        assert_eq!(ownerships.len(), 1, "fixture must carry lane ownership");
        let mut store = fixture.reopen_body_store();
        fixture.service.fail_after_kura_store_for_test();
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::InjectedCrashAfterKuraStore)
        ));
        drop(store);
        let durable = fixture
            .kura
            .get_block(NonZeroUsize::new(1).expect("height"))
            .expect("read production-validated Kura lane crash image");
        assert!(durable.has_results());
        assert_eq!(durable.results().len(), 1);
        assert!(durable.results().all(|result| result.is_ok()));
        let durable_wire = durable.encode_wire().expect("encode Kura lane crash image");
        fixture.assert_no_post_apply_sidecars();
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "the Kura-first lane crash boundary must not leak partial WSV state"
        );
        assert!(
            fixture
                .kura
                .read_lane_block_artifact(ownerships[0].lane_id, ownerships[0].lane_block_height,)
                .is_some(),
            "Kura crash image must include the exact lane sidecar"
        );

        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("resume exact lane-body WSV application after Kura-first crash");
        fixture.assert_complete();
        assert_eq!(
            fixture
                .kura
                .get_block(NonZeroUsize::new(1).expect("height"))
                .expect("read recovered Kura lane block")
                .encode_wire()
                .expect("encode recovered Kura lane block"),
            durable_wire,
            "an exact lane retry must preserve the complete canonical Kura wire"
        );
    });

    v2_apply_test!(
        conflicting_canonical_kura_block_fails_before_wsv_mutation,
        {
            let fixture = ApplyFixture::new();
            let conflicting_key =
                KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519).expect("conflict key");
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("height"),
                None,
                None,
                None,
                9_999,
                0,
            );
            let signature =
                SignatureOf::try_from_hash(conflicting_key.private_key(), header.hash())
                    .expect("sign conflicting block");
            let conflicting =
                SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
            assert_ne!(conflicting.hash(), fixture.body.hash());
            fixture
                .kura
                .store_block(conflicting)
                .expect("persist conflicting canonical block");
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
                fixture.execute(&mut store),
                Err(V2ApplyError::KuraConflict)
            ));
            assert_eq!(fixture.state.committed_height(), 0);
            fixture.assert_no_post_apply_sidecars();
        }
    );

    v2_apply_test!(wsv_without_its_canonical_kura_block_fails_closed, {
        let fixture = ApplyFixture::new();
        let artifact = wire::finality::V2FinalityArtifact::new(
            fixture.context.clone(),
            fixture.task.subject(),
            fixture.task.certificate().clone(),
            fixture.service.validator_set_pops.clone(),
        );
        fixture
            .service
            .validate_and_apply(
                &fixture.context,
                fixture.body.clone(),
                false,
                fixture.task.validated_receipt().execution_commitment(),
                &artifact,
            )
            .expect("model corrupted WSV-ahead crash image");
        assert_eq!(fixture.state.committed_height(), 1);
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
        let mut store = fixture.reopen_body_store();

        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::StateAheadOfKura)
        ));
        fixture.assert_no_post_apply_sidecars();
    });

    v2_apply_test!(
        apply_rejects_commit_qc_execution_commitment_drift_before_state_or_kura_write,
        {
            let fixture = ApplyFixture::new();
            let mut certificate = fixture.task.certificate().clone();
            certificate.execution_commitment = wire::ExecutionCommitment::without_topups(
                Hash::new(b"wrong parent state"),
                Hash::new(b"wrong post state"),
                Hash::new(b"wrong ordinary writes"),
                Hash::new(b"wrong executed block wire"),
            );
            let task = ApplyTask::for_test(
                2,
                fixture.task.tag(),
                fixture.task.subject(),
                certificate,
                fixture.task.validated_receipt().clone(),
            );
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
                fixture.service.execute(&fixture.context, &mut store, &task),
                Err(V2ApplyError::ExecutionCommitmentMismatch)
            ));
            assert_eq!(fixture.state.committed_height(), 0);
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 0);
            fixture.assert_no_post_apply_sidecars();
        }
    );

    v2_apply_test!(
        fresh_apply_recomputes_and_rejects_a_consistently_forged_marker_and_qc,
        {
            let fixture = ApplyFixture::new();
            let forged_commitment = wire::ExecutionCommitment::without_topups(
                Hash::new(b"forged parent state"),
                Hash::new(b"forged post state"),
                Hash::new(b"forged ordinary writes"),
                Hash::new(b"forged executed block wire"),
            );
            let mut certificate = fixture.task.certificate().clone();
            certificate.execution_commitment = forged_commitment;

            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let preimage = wire::Vote {
                round: certificate.round,
                proposal_round: certificate.proposal_round,
                phase: certificate.phase,
                subject: certificate.subject,
                execution_commitment: forged_commitment,
                signer: certificate.signers[0],
                signature: Vec::new(),
            }
            .signature_preimage();
            let signatures = certificate
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                        &preimage,
                    )
                    .expect("sign forged execution commitment")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate forged Commit votes");

            let forged_validation = ValidatedBodyReceipt::for_test_with_commitment(
                fixture.task.validated_receipt().durable().clone(),
                forged_commitment,
            );
            let task = ApplyTask::for_test(
                2,
                fixture.task.tag(),
                fixture.manifest.subject,
                certificate,
                forged_validation,
            );
            let mut store = fixture.reopen_body_store();

            assert!(matches!(
                fixture.service.execute(&fixture.context, &mut store, &task),
                Err(V2ApplyError::ExecutionCommitmentMismatch)
            ));
            fixture.assert_no_apply_mutation();
        }
    );

    v2_apply_test!(
        checkpoint_write_failure_keeps_wsv_behind_durable_kura_tip,
        {
            let fixture = ApplyFixture::new();
            let mut store = fixture.reopen_body_store();
            fixture.kura.fail_next_wsv_checkpoint_write_for_tests();
            let error = fixture
                .execute(&mut store)
                .expect_err("checkpoint failure follows the durable Kura boundary");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::CommittedRecoveryRequired { stage, .. }
                        if *stage == "pre-WSV recovery checkpoint"
                ),
                "unexpected committed recovery classification: {error:?}"
            );
            assert!(error.requires_restart_recovery());
            assert_eq!(
                fixture.state.committed_height(),
                0,
                "live WSV must not advance without its durable recovery checkpoint"
            );
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
            fixture.assert_no_post_apply_sidecars();

            drop(store);
            let mut reopened = fixture.reopen_body_store();
            assert!(
                reopened
                    .validated_recovery_catalog()
                    .contains_key(&(fixture.manifest.round, fixture.manifest.subject)),
                "restart must recover the exact durable validation marker"
            );
            fixture
                .execute(&mut reopened)
                .expect("replay the exact durable tip and publish WSV once");
            fixture.assert_complete();
        }
    );

    v2_apply_test!(
        crash_after_staged_checkpoint_replays_exact_tip_without_double_apply,
        {
            let fixture = ApplyFixture::new();
            let mut first_process_store = fixture.reopen_body_store();
            fixture.service.fail_after_wsv_checkpoint_for_test();
            let first_error = fixture
                .service
                .execute(&fixture.context, &mut first_process_store, &fixture.task)
                .expect_err("inject crash after checkpoint fsync and before WSV publication");
            assert!(matches!(
                &first_error,
                V2ApplyError::InjectedCrashAfterWsvCheckpoint
            ));
            assert!(first_error.requires_restart_recovery());
            assert_eq!(fixture.state.committed_height(), 0);
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
            let staged_checkpoint = fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read staged checkpoint")
                .expect("checkpoint must be durable before WSV publication");
            assert!(
                fixture
                    .kura
                    .commit_manifest(1)
                    .expect("read absent manifest")
                    .is_none(),
                "the pre-WSV checkpoint must remain unbound until State commits"
            );
            let staged_state_hash = staged_checkpoint.state_hash();
            drop(first_process_store);

            // Snapshot publication is gated on the complete
            // checkpoint/manifest/finality tuple, so a process crash reloads
            // the last finalized snapshot (height zero here). The exact
            // durable checkpoint authenticates the overlay replay before live
            // State can cross its commit boundary.
            let (restarted_service, restarted_state) =
                fixture.restart_service_from_last_finalized_snapshot();
            assert_eq!(restarted_state.committed_height(), 0);
            let mut restarted_store = fixture.reopen_body_store();
            restarted_service
                .execute(&fixture.context, &mut restarted_store, &fixture.task)
                .expect("authenticated WAL/body retry reapplies the sole Kura tip");
            assert_eq!(restarted_state.committed_height(), 1);
            let first_artifact = fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read recovered finality")
                .expect("recovery publishes finality");
            assert_eq!(first_artifact.block_hash, fixture.body.hash());
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref()),
                staged_state_hash,
                "recovery must reproduce the exact pre-commit checkpointed WSV"
            );

            let durable_state_hash =
                crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref());
            restarted_service
                .execute(&fixture.context, &mut restarted_store, &fixture.task)
                .expect("an exact post-finality retry is idempotent");
            assert_eq!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read repeated finality")
                    .as_ref(),
                Some(&first_artifact)
            );
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref()),
                durable_state_hash,
                "idempotent retry must not execute the block twice"
            );
            fixture.assert_complete();
        }
    );

    v2_apply_test!(restart_recovers_checkpoint_before_manifest_and_finality, {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_commit_manifest_write_for_tests();
        let error = fixture
            .execute(&mut store)
            .expect_err("manifest failure follows the irreversible commit boundary");
        assert!(
            matches!(
                &error,
                V2ApplyError::CommittedRecoveryRequired { stage, .. }
                    if *stage == "post-apply metadata"
            ),
            "unexpected committed recovery classification: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 1);
        assert!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read checkpoint")
                .is_some()
        );
        assert!(
            fixture
                .kura
                .commit_manifest(1)
                .expect("read manifest")
                .is_none()
        );
        assert!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read finality")
                .is_none()
        );

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        fixture.execute(&mut reopened).expect("complete manifest");
        fixture.assert_complete();
    });

    v2_apply_test!(restart_recovers_metadata_written_before_finality, {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_v2_finality_write_for_tests();
        let error = fixture
            .execute(&mut store)
            .expect_err("finality failure follows the irreversible commit boundary");
        assert!(
            matches!(
                &error,
                V2ApplyError::CommittedRecoveryRequired { stage, .. }
                    if *stage == "v2 finality artifact"
            ),
            "unexpected committed recovery classification: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 1);
        assert!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read checkpoint")
                .is_some()
        );
        assert!(
            fixture
                .kura
                .commit_manifest(1)
                .expect("read manifest")
                .is_some()
        );
        assert!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read finality")
                .is_none()
        );

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        fixture.execute(&mut reopened).expect("complete finality");
        fixture.assert_complete();
    });

    v2_apply_test!(
        complete_apply_replay_is_idempotent_and_never_advances_twice,
        {
            let fixture = ApplyFixture::new();
            let mut store = fixture.reopen_body_store();
            fixture.execute(&mut store).expect("initial apply");
            let state_hash = crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
            let artifact = fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read finality")
                .expect("finality exists");

            fixture.execute(&mut store).expect("idempotent replay");
            fixture.assert_complete();
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
                state_hash
            );
            assert_eq!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read repeated finality"),
                Some(artifact)
            );
        }
    );
}
