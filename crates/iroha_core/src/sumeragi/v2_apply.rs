//! Idempotent production application of a durable Sumeragi v2 decision.
//!
//! A CommitQC is written to the safety WAL before this module is invoked. The
//! application transaction then re-loads the exact validated body, advances
//! Kura and WSV at most once, and finally persists the canonical v2 finality
//! sidecar. Restart may observe Kura/WSV already at the decided height while
//! the sidecar is absent; that state is completed without re-applying the
//! block or validating it against a later state.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    sync::Arc,
    time::Duration,
};

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
        TagProjection, check_production_application_transition,
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

    let committed_recovered = recovered
        .iter()
        .filter_map(|reservation| {
            state
                .has_committed_transaction(reservation.signed_transaction_hash)
                .then_some(reservation.signed_transaction_hash)
        })
        .collect::<BTreeSet<_>>();
    let exact_committed = kura.committed_merge_queue_reservations(&committed_recovered)?;

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

fn prospective_application_refinement_projection(
    context: &wire::HeightContext,
    task: &ApplyTask,
    proposal_block_hash: HashOf<BlockHeader>,
    canonical_proposal_wire_hash: Hash,
    artifact: &wire::finality::V2FinalityArtifact,
) -> Option<ProductionApplicationTraceProjection> {
    let context_id = application_typed_identity(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        context.id().0,
    );
    let artifact_hash = HashOf::new(artifact);
    Some(ProductionApplicationTraceProjection {
        task_tag: TagProjection {
            height: task.tag().height(),
            view: task.tag().view(),
            generation: task.tag().generation().get(),
        },
        owner_tag: TagProjection {
            height: task.authorized_owner_tag().height(),
            view: task.authorized_owner_tag().view(),
            generation: task.authorized_owner_tag().generation().get(),
        },
        task_generation: task.tag().generation().get(),
        context_id,
        context_height: context.height,
        commit_qc: application_certificate_projection(task.certificate())?,
        validated_body: application_body_projection(task.validated_receipt()),
        validated_execution_commitment: application_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTION_COMMITMENT,
            HashOf::new(&task.validated_receipt().execution_commitment()),
        ),
        proposal_block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            proposal_block_hash,
        ),
        proposal_payload_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            canonical_proposal_wire_hash,
        ),
        committed_block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            task.subject().block_hash,
        ),
        executed_block_wire_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
            task.certificate()
                .execution_commitment
                .executed_block_wire_hash,
        ),
        kura_decision: application_decision_projection(task.certificate().as_ref()),
        kura_artifact_hash: application_typed_identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_FINALITY_ARTIFACT,
            artifact_hash,
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
            artifact_hash,
        ),
        state_height_after: context.height,
        task_work_id: task.id().get(),
        completion_work_id: task.id().get(),
    })
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
            // The durable body must be the exact same-round proposal body
            // authenticated by the CommitQC.
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
    provider_ingest_finalized_archive:
        Option<Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>>,
    reputation_finalized_archive:
        Option<Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>>,
    chain_id: ChainId,
    block_cadence: Duration,
    genesis_account: AccountId,
    events_sender: EventsSender,
    validator_set_pops: Vec<Vec<u8>>,
    #[cfg(test)]
    fail_after_kura_store: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_after_wsv_checkpoint: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_after_provider_ingest_archive_capture: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_after_reputation_archive_capture: std::sync::atomic::AtomicBool,
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
        provider_ingest_finalized_archive: Option<
            Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
        >,
        reputation_finalized_archive: Option<
            Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
        >,
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
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            chain_id,
            block_cadence,
            genesis_account,
            events_sender,
            validator_set_pops,
            #[cfg(test)]
            fail_after_kura_store: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_after_wsv_checkpoint: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_after_provider_ingest_archive_capture: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_after_reputation_archive_capture: std::sync::atomic::AtomicBool::new(false),
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
        let prospective_application = prospective_application_refinement_projection(
            context,
            task,
            proposal_block_hash,
            canonical_proposal_wire_hash,
            &artifact,
        )
        .ok_or_else(|| {
            V2ApplyError::Validation(
                "prospective application identity cannot be represented losslessly".to_owned(),
            )
        })?;
        let checked_application = check_production_application_transition(prospective_application)
            .ok_or_else(|| {
                V2ApplyError::Validation(
                    "prospective durable application does not refine its Decision completion"
                        .to_owned(),
                )
            })?;
        let prospective_application = checked_application.into_projection();

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

        // Repair or confirm the durable finality boundary before any derived
        // publication. Fresh application already crossed this boundary inside
        // `validate_and_apply`; these calls are deliberately idempotent so
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
        self.finish_durable_apply_completion_against(evidence, prospective_application)
    }

    #[cfg(test)]
    fn finish_durable_apply_completion(
        &self,
        evidence: DurableApplicationEvidence,
    ) -> Result<DurableApplyCompletion, V2ApplyError> {
        let application_trace = evidence
            .application_refinement_projection()
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"native application identity cannot be represented losslessly",
                )
            })?;
        let checked_application = check_production_application_transition(application_trace)
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"durable application does not refine its Decision completion",
                )
            })?;
        self.finish_durable_apply_completion_against(
            evidence,
            checked_application.into_projection(),
        )
    }

    fn finish_durable_apply_completion_against(
        &self,
        evidence: DurableApplicationEvidence,
        prospective_application: ProductionApplicationTraceProjection,
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
        if application_trace != prospective_application {
            return Err(V2ApplyError::committed_recovery_required(
                "application refinement evidence",
                &"durable application differs from its pre-authorized Decision completion",
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
        // The atomic State commit above is the authority that the merge batch
        // reached WSV. Publish its exact lane-application receipts only after
        // that check and before queue reservation finalization. The Kura
        // writer is idempotent, so a crash at any receipt/frontier boundary is
        // repaired by retrying this same committed carrier without replaying
        // economic effects.
        self.kura
            .persist_merge_lane_block_application_receipts_from_committed_log(&entry)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required(
                    "merge application receipt publication",
                    &error,
                )
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
        let pre_wsv_finality_receipt = if store_block {
            self.kura.store_block(committed_block.clone())?;
            #[cfg(test)]
            if self
                .fail_after_kura_store
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return Err(V2ApplyError::InjectedCrashAfterKuraStore);
            }
            let receipt = self
                .kura
                .store_v2_finality_artifact(artifact)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "pre-WSV v2 finality artifact",
                        &error,
                    )
                })?;
            Some(receipt)
        } else {
            None
        };
        let native_amx_prepublication = if store_block {
            Some(
                self.kura
                    .prepublish_native_amx_participant_application_evidence(
                        committed_block.as_ref(),
                    )
                    .map_err(|error| {
                        V2ApplyError::committed_recovery_required(
                            "pre-WSV Native AMX participant evidence publication",
                            &error,
                        )
                    })?,
            )
        } else {
            None
        };
        let native_amx_frontiers = State::native_amx_participant_frontier_markers(
            committed_block.as_ref(),
        )
        .map_err(|error| {
            V2ApplyError::committed_recovery_required(
                "pre-WSV Native AMX participant frontier projection",
                &error,
            )
        })?;
        let native_amx_prepublication_matches_state = match native_amx_prepublication.as_ref() {
            Some(token) => token.authenticates_state_frontiers(
                committed_block.as_ref(),
                &native_amx_manifest,
                artifact,
                &native_amx_frontiers,
            ),
            None => native_amx_frontiers.is_empty(),
        };
        if !native_amx_prepublication_matches_state {
            return Err(V2ApplyError::committed_recovery_required(
                "pre-WSV Native AMX participant evidence publication",
                &"read-back token differs from the exact State frontier projection",
            ));
        }

        // `apply_without_execution_with_verified_v2_finality` stages the
        // Native participant frontiers in the State overlay. Do not construct
        // that overlay until every canonical manifest leaf has a durable,
        // read-back-authenticated manifest/receipt/latest-index triple.
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
        #[cfg(test)]
        let staged_snapshot_bytes_for_test = store_block
            .then(|| crate::snapshot::canonical_staged_state_snapshot_bytes(&state_block));
        if store_block {
            let staged_checkpoint =
                crate::snapshot::canonical_staged_state_snapshot_hash(&state_block);
            self.kura
                .store_wsv_checkpoint(context.height, block_hash, staged_checkpoint)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required("pre-WSV recovery checkpoint", &error)
                })?;
        }
        #[cfg(test)]
        if self
            .fail_after_wsv_checkpoint
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return Err(V2ApplyError::InjectedCrashAfterWsvCheckpoint);
        }
        // TODO: Add an automatic governed retention controller and deployment
        // policy before treating this bounded archive as suitable for indefinite
        // node operation. Explicit Kura-authenticated, sealed-CAS-approved prefix
        // compaction is available; reaching a configured ceiling without an
        // authorized retention decision intentionally remains fail-stop.
        if let Some(archive) = self.provider_ingest_finalized_archive.as_ref() {
            let receipt = pre_wsv_finality_receipt.as_ref().ok_or_else(|| {
                V2ApplyError::CommittedRecoveryRequired {
                    stage: "provider-ingest finalized archive capture",
                    detail: "the exact pre-WSV Kura finality receipt is unavailable".to_owned(),
                }
            })?;
            archive
                .capture_kura_authenticated_view(&state_block, self.kura.as_ref(), receipt)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "provider-ingest finalized archive capture",
                        &error,
                    )
                })?;
            #[cfg(test)]
            if self
                .fail_after_provider_ingest_archive_capture
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return Err(V2ApplyError::InjectedCrashAfterProviderIngestArchiveCapture);
            }
        }
        if let Some(archive) = self.reputation_finalized_archive.as_ref() {
            let receipt = pre_wsv_finality_receipt.as_ref().ok_or_else(|| {
                V2ApplyError::CommittedRecoveryRequired {
                    stage: "reputation finalized archive capture",
                    detail: "the exact pre-WSV Kura finality receipt is unavailable".to_owned(),
                }
            })?;
            archive
                .capture_kura_authenticated_view(&state_block, self.kura.as_ref(), receipt)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "reputation finalized archive capture",
                        &error,
                    )
                })?;
            #[cfg(test)]
            if self
                .fail_after_reputation_archive_capture
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return Err(V2ApplyError::InjectedCrashAfterReputationArchiveCapture);
            }
        }
        state_block.commit().map_err(|error| {
            V2ApplyError::committed_recovery_required("WSV publication after Kura commit", &error)
        })?;
        #[cfg(test)]
        if let Some(staged) = staged_snapshot_bytes_for_test {
            let committed = crate::snapshot::canonical_state_snapshot_bytes(self.state.as_ref());
            if staged != committed {
                panic!(
                    "staged/committed WSV snapshot mismatch after block commit: {}",
                    snapshot_mismatch_context(&staged, &committed),
                );
            }
        }

        self.queue.remove_committed_hashes(
            committed_block
                .as_ref()
                .external_entrypoints_cloned()
                .map(|entrypoint| HashOf::from_untyped_unchecked(Hash::from(entrypoint.hash()))),
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

    #[cfg(test)]
    fn fail_after_provider_ingest_archive_capture_for_test(&self) {
        self.fail_after_provider_ingest_archive_capture
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    fn fail_after_reputation_archive_capture_for_test(&self) {
        self.fail_after_reputation_archive_capture
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
    /// Test-only crash boundary after the immutable provider-ingest
    /// projection is durable and before live State publication.
    #[cfg(test)]
    #[error("injected crash after provider-ingest archive capture and before WSV commit")]
    InjectedCrashAfterProviderIngestArchiveCapture,
    /// Test-only crash boundary after the immutable reputation projection is
    /// durable and before live State publication.
    #[cfg(test)]
    #[error("injected crash after reputation archive capture and before WSV commit")]
    InjectedCrashAfterReputationArchiveCapture,
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
            Self::InjectedCrashAfterKuraStore
            | Self::InjectedCrashAfterWsvCheckpoint
            | Self::InjectedCrashAfterProviderIngestArchiveCapture
            | Self::InjectedCrashAfterReputationArchiveCapture => true,
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
fn snapshot_mismatch_context(staged: &[u8], committed: &[u8]) -> String {
    let first_difference = staged
        .iter()
        .zip(committed)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| staged.len().min(committed.len()));
    let context_start = first_difference.saturating_sub(256);
    let staged_end = first_difference.saturating_add(768).min(staged.len());
    let committed_end = first_difference.saturating_add(768).min(committed.len());
    format!(
        "first_difference={first_difference}, staged_len={}, committed_len={}, \
         staged_context={:?}, committed_context={:?}",
        staged.len(),
        committed.len(),
        String::from_utf8_lossy(&staged[context_start..staged_end]),
        String::from_utf8_lossy(&committed[context_start..committed_end]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::v2_core::{EventTag, Generation};
    use crate::{
        block::BlockBuilder,
        governance::manifest::{
            GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
        },
        lane_consensus::LaneExecutablePayloadV1,
        query::{
            provider_ingest_finalized::{
                ProviderIngestFinalizedArchiveBoundsV1,
                ProviderIngestFinalizedArchiveInsertOutcomeV1, ProviderIngestFinalizedArchiveV1,
            },
            reputation_finalized::{
                ReputationFinalizedArchive, ReputationFinalizedArchiveBounds,
                ReputationFinalizedArchiveError, ReputationFinalizedArchiveInsertOutcome,
                ReputationFinalizedArchiveRetentionApprovalRecordV1,
                ReputationFinalizedArchiveRetentionAuthorityBindingV1,
                ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1,
                ReputationFinalizedArchiveRetentionAuthorityQualificationV1,
                ReputationFinalizedArchiveRetentionAuthorityV1,
            },
            store::LiveQueryStore,
        },
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
    use iroha_config::parameters::actual::{LaneConfig as RuntimeLaneConfig, Queue as QueueConfig};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        Level, Registrable, ValidationFail,
        account::Account,
        asset::{AssetDefinition, AssetDefinitionId},
        block::{
            BlockExecutionContextBundle, BlockHeader, BlockSignature, SignedBlock,
            consensus::{
                CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalV1,
                LaneBlockQcV1,
            },
            consensus_v2 as wire,
        },
        consensus::{ConsensusKeyRecord, ConsensusKeyStatus, VALIDATOR_SET_HASH_VERSION_V1},
        domain::{Domain, DomainId},
        isi::{
            InstructionBox, Log, SetParameter,
            sorafs::{
                SetSorafsOrderbookPolicy, SetSorafsReputationJournalAuthorityPolicy,
                SetSorafsReservePolicy,
            },
        },
        merge::{
            MergeExecutionBatch, MergeLaneExecution, MergeLedgerEntry, MergeQuorumCertificate,
        },
        nexus::{DataSpaceId, LaneId, LaneStorageProfile, LaneVisibility},
        parameter::{Parameter, system::SumeragiParameter},
        peer::PeerId,
        permission::{Permission, Permissions},
        sorafs::{
            orderbook::{ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyV1},
            reputation::{
                REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
                REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1, ReputationJournalAuthorityPolicyV1,
            },
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReservePolicyV1,
            },
        },
        transaction::{
            TransactionBuilder, TransactionEntrypoint,
            error::TransactionRejectionReason,
            signed::{TransactionResult, TransactionResultInner},
        },
        trigger::DataTriggerSequence,
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanManageSorafsReputationJournalPolicy, CanSetSorafsPricing, CanSetSorafsReservePolicy,
    };
    use mv::storage::StorageReadOnly;
    use norito::codec::Encode as _;
    use sorafs_manifest::XorQuantity;
    use std::{
        borrow::Cow,
        collections::BTreeMap,
        num::{NonZeroU64, NonZeroUsize},
        sync::{Arc, Mutex},
    };
    #[derive(Debug)]
    struct ReputationRetentionAuthorityForTest {
        qualification: ReputationFinalizedArchiveRetentionAuthorityQualificationV1,
        latest: Mutex<Option<ReputationFinalizedArchiveRetentionApprovalRecordV1>>,
        armed_load_failure_after_cas: Mutex<Option<usize>>,
        load_failure_countdown: Mutex<Option<usize>>,
    }

    impl ReputationRetentionAuthorityForTest {
        fn new() -> Self {
            Self {
                qualification: ReputationFinalizedArchiveRetentionAuthorityQualificationV1::new(
                    7, [0xA7; 32],
                ),
                latest: Mutex::new(None),
                armed_load_failure_after_cas: Mutex::new(None),
                load_failure_countdown: Mutex::new(None),
            }
        }

        fn binding(&self) -> ReputationFinalizedArchiveRetentionAuthorityBindingV1 {
            ReputationFinalizedArchiveRetentionAuthorityBindingV1::try_new(
                self.handle().to_owned(),
                self.qualification.revision(),
                self.qualification.policy_digest(),
            )
            .expect("valid reputation retention authority binding")
        }

        fn fail_nth_load_after_next_cas(&self, load_number: usize) {
            assert!(load_number != 0);
            *self
                .armed_load_failure_after_cas
                .lock()
                .expect("lock armed retention failure") = Some(load_number);
        }
    }

    impl ReputationFinalizedArchiveRetentionAuthorityV1 for ReputationRetentionAuthorityForTest {
        fn handle(&self) -> &str {
            "sealed.reputation.archive.v2-apply"
        }

        fn qualification(
            &self,
        ) -> Result<
            ReputationFinalizedArchiveRetentionAuthorityQualificationV1,
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1,
        > {
            Ok(self.qualification)
        }

        fn load_latest(
            &self,
            _chain_id: &ChainId,
        ) -> Result<
            Option<ReputationFinalizedArchiveRetentionApprovalRecordV1>,
            ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1,
        > {
            let mut countdown = self
                .load_failure_countdown
                .lock()
                .expect("lock retention load failure");
            if let Some(remaining) = *countdown {
                if remaining == 1 {
                    *countdown = None;
                    return Err(
                        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Unavailable,
                    );
                }
                *countdown = Some(remaining - 1);
            }
            drop(countdown);
            Ok(self.latest.lock().expect("lock retention approval").clone())
        }

        fn compare_and_swap_latest(
            &self,
            _chain_id: &ChainId,
            expected_revision: Option<[u8; 32]>,
            next: &ReputationFinalizedArchiveRetentionApprovalRecordV1,
        ) -> Result<(), ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1> {
            let mut latest = self.latest.lock().expect("lock retention approval");
            if latest
                .as_ref()
                .map(ReputationFinalizedArchiveRetentionApprovalRecordV1::revision)
                != expected_revision
            {
                return Err(ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::Rejected);
            }
            *latest = Some(next.clone());
            let armed = self
                .armed_load_failure_after_cas
                .lock()
                .expect("lock armed retention failure")
                .take();
            *self
                .load_failure_countdown
                .lock()
                .expect("lock retention load failure") = armed;
            Ok(())
        }
    }

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
        custody_account: AccountId,
        treasury_account: AccountId,
        include_projection_policies: bool,
    }

    fn direct_archive_tempdir() -> tempfile::TempDir {
        let root = std::env::current_dir().expect("resolve direct archive test root");
        tempfile::tempdir_in(root).expect("create direct archive test directory")
    }

    fn provider_ingest_archive_bounds(
        max_record_bytes: u64,
        max_total_bytes: u64,
    ) -> ProviderIngestFinalizedArchiveBoundsV1 {
        ProviderIngestFinalizedArchiveBoundsV1::try_new(
            max_record_bytes,
            16,
            max_total_bytes,
            16,
            16,
            64,
            16,
        )
        .expect("valid provider-ingest archive bounds")
    }

    fn fixture_reserve_asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("sorafs", "universal").expect("valid fixture settlement domain"),
            "xor".parse().expect("valid fixture settlement asset name"),
        )
    }

    fn fixture_orderbook_policy(authority: &AccountId) -> OrderbookAdmissionPolicyV1 {
        OrderbookAdmissionPolicyV1 {
            version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0xA5; 32],
            matcher_authority: authority.clone(),
            settlement_authority: authority.clone(),
            paused: false,
            min_order_gib: 2,
            max_order_gib: 1_024,
            price_tick_micro_xor: 10,
            max_maker_fee_bps: 100,
            max_taker_fee_bps: 200,
            max_order_lifetime_secs: 3_600,
            max_receipt_age_secs: 300,
            max_clock_skew_secs: 5,
            max_receipt_bytes: 1_024,
            max_receipts_per_channel: 2,
        }
    }

    fn fixture_reserve_policy(
        authority: &AccountId,
        custody_account: AccountId,
        treasury_account: AccountId,
    ) -> ReserveAuthorityPolicyV1 {
        ReserveAuthorityPolicyV1 {
            version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            economics: ReservePolicyV1::default(),
            asset_definition: fixture_reserve_asset_definition(),
            custody_account,
            treasury_account,
            operations_authority: authority.clone(),
            decision_authority: authority.clone(),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: XorQuantity::try_from_micro(1_000_000_000)
                .expect("valid fixture debt ceiling"),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        }
    }

    fn fixture_world(
        transaction_authority: &AccountId,
        custody_account: &AccountId,
        treasury_account: &AccountId,
        include_projection_policies: bool,
    ) -> World {
        let reserve_asset_definition = fixture_reserve_asset_definition();
        let reserve_domain =
            Domain::new(reserve_asset_definition.domain().clone()).build(transaction_authority);
        let reserve_asset = AssetDefinition::numeric(reserve_asset_definition)
            .with_name("XOR".to_owned())
            .build(transaction_authority);
        let mut world = World::with_assets(
            [reserve_domain],
            [
                Account::new(transaction_authority.clone()).build(transaction_authority),
                Account::new(custody_account.clone()).build(custody_account),
                Account::new(treasury_account.clone()).build(treasury_account),
            ],
            [reserve_asset],
            [],
            [],
        );
        if include_projection_policies {
            let mut authority_permissions = Permissions::new();
            authority_permissions.insert(Permission::from(CanManageSorafsReputationJournalPolicy));
            authority_permissions.insert(Permission::from(CanSetSorafsPricing));
            authority_permissions.insert(Permission::from(CanSetSorafsReservePolicy));
            world
                .account_permissions
                .insert(transaction_authority.clone(), authority_permissions);
        }
        world
    }

    fn install_fixture_validator_authority(
        state: &State,
        context: &wire::HeightContext,
        validator_set_pops: &[Vec<u8>],
    ) {
        assert_eq!(
            context.roster.len(),
            validator_set_pops.len(),
            "fixture roster and validator PoPs must remain positionally aligned"
        );

        let mut world_block = state.world.block();
        {
            let mut peers = world_block.peers_mut_for_testing().transaction();
            for validator in &context.roster {
                if !peers.iter().any(|peer| peer == &validator.validator) {
                    peers.push(validator.validator.clone());
                }
            }
            peers.apply();
        }
        for (validator, pop) in context.roster.iter().zip(validator_set_pops) {
            let public_key = validator.validator.public_key().clone();
            let id = crate::state::derive_validator_key_id(&public_key);
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key,
                pop: Some(pop.clone()),
                activation_height: 0,
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world_block
                .consensus_keys
                .insert(id.clone(), record.clone());
            world_block
                .consensus_keys_by_pk
                .insert(record.public_key.to_string(), vec![id]);
        }
        world_block.commit();

        let validators = context
            .roster
            .iter()
            .map(|validator| AccountId::new(validator.validator.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_bindings = validators
            .iter()
            .zip(&context.roster)
            .map(|(validator, power)| ManifestValidatorBinding {
                validator: validator.clone(),
                peer_id: power.validator.clone(),
                torii_url: None,
            })
            .collect();
        let status = LaneManifestStatus {
            lane: LaneId::SINGLE,
            alias: "default".to_owned(),
            dataspace: DataSpaceId::UNIVERSAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("sumeragi-v2-apply-fixture".to_owned()),
            manifest_path: Some(std::path::PathBuf::from(
                "/tmp/sumeragi-v2-apply-fixture-manifest.json",
            )),
            governance_rules: Some(GovernanceRules {
                validators,
                validator_bindings,
                ..GovernanceRules::default()
            }),
            privacy_commitments: Vec::new(),
        };
        state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(
            BTreeMap::from([(LaneId::SINGLE, status)]),
        )));

        let mut expected = context
            .roster
            .iter()
            .map(|validator| validator.validator.clone())
            .collect::<Vec<_>>();
        expected.sort();
        let mut actual =
            state.authoritative_lane_peer_ids_at_height(LaneId::SINGLE, context.height);
        actual.sort();
        assert_eq!(
            actual, expected,
            "fixture must expose every authenticated validator as lane authority"
        );
    }

    impl ApplyFixture {
        fn new() -> Self {
            Self::new_with_lane_payload(false)
        }

        fn new_with_lane_payload(include_lane_payload: bool) -> Self {
            Self::new_with_options(include_lane_payload, false)
        }

        fn new_with_reputation_archive() -> Self {
            Self::new_with_options(false, true)
        }

        fn new_with_options(include_lane_payload: bool, include_projection_policies: bool) -> Self {
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
            let custody_key = KeyPair::try_from_seed(vec![0xE8; 32], Algorithm::Ed25519)
                .expect("deterministic custody key");
            let treasury_key = KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::Ed25519)
                .expect("deterministic treasury key");
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
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
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
            let custody_account = AccountId::new(custody_key.public_key().clone());
            let treasury_account = AccountId::new(treasury_key.public_key().clone());
            let world = fixture_world(
                &transaction_authority,
                &custody_account,
                &treasury_account,
                include_projection_policies,
            );
            let state = Arc::new(State::new_with_chain_for_testing(
                world,
                Arc::clone(&kura),
                LiveQueryStore::start_test(),
                chain_id.clone(),
            ));
            let validator_set_pops = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("fixture validator PoP")
                })
                .collect::<Vec<_>>();
            install_fixture_validator_authority(&state, &context, &validator_set_pops);
            let mut commit_topology = state.commit_topology.block();
            commit_topology.clear();
            for validator in &context.roster {
                commit_topology.push(validator.validator.clone());
            }
            commit_topology.commit();
            let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
            let queue = Arc::new(Queue::from_config(
                QueueConfig::default(),
                events_sender.clone(),
            ));
            let service = V2ApplyService::new(
                Arc::clone(&state),
                Arc::clone(&queue),
                Arc::clone(&kura),
                None,
                None,
                chain_id.clone(),
                Duration::from_secs(1),
                transaction_authority.clone(),
                events_sender,
                validator_set_pops,
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
            let reputation_policy = ReputationJournalAuthorityPolicyV1 {
                version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
                revision: 1,
                predecessor_policy_digest: None,
                por_recorder_authority: transaction_authority.clone(),
                dispute_recorder_authority: transaction_authority.clone(),
                token_recorder_authority: transaction_authority.clone(),
                max_source_age_ms: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
            };
            let transaction_instructions = || {
                let mut instructions = vec![InstructionBox::from(SetParameter::new(
                    Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)),
                ))];
                if include_projection_policies {
                    instructions.push(InstructionBox::from(
                        SetSorafsReputationJournalAuthorityPolicy::new(reputation_policy.clone()),
                    ));
                    instructions.push(InstructionBox::from(SetSorafsOrderbookPolicy::new(
                        fixture_orderbook_policy(&transaction_authority),
                    )));
                    instructions.push(InstructionBox::from(SetSorafsReservePolicy::new(
                        fixture_reserve_policy(
                            &transaction_authority,
                            custody_account.clone(),
                            treasury_account.clone(),
                        ),
                    )));
                }
                instructions
            };
            let body = if include_lane_payload {
                let transaction = TransactionBuilder::new(
                    chain_id.clone(),
                    transaction_authority.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions(transaction_instructions())
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
                    transaction_authority.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions(transaction_instructions())
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
                custody_account,
                treasury_account,
                include_projection_policies,
            }
        }

        fn reopen_body_store(&self) -> V2BodyStore {
            V2BodyStore::open_with_policy(
                self.body_root.path(),
                self.context.clone(),
                BlockSignaturePolicy::GenesisAuthority(
                    self.service
                        .genesis_account
                        .expect_single_signatory()
                        .clone(),
                ),
            )
            .expect("reopen body store after crash")
        }

        fn restart_service_from_last_finalized_snapshot(&self) -> (V2ApplyService, Arc<State>) {
            let authority = self.service.genesis_account.clone();
            let world = fixture_world(
                &authority,
                &self.custody_account,
                &self.treasury_account,
                self.include_projection_policies,
            );
            let state = Arc::new(State::new_with_chain_for_testing(
                world,
                Arc::clone(&self.kura),
                LiveQueryStore::start_test(),
                self.service.chain_id.clone(),
            ));
            install_fixture_validator_authority(
                &state,
                &self.context,
                &self.service.validator_set_pops,
            );
            let mut commit_topology = state.commit_topology.block();
            commit_topology.clear();
            for validator in &self.context.roster {
                commit_topology.push(validator.validator.clone());
            }
            commit_topology.commit();
            let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
            let queue = Arc::new(Queue::from_config(
                QueueConfig::default(),
                events_sender.clone(),
            ));
            let service = V2ApplyService::new(
                Arc::clone(&state),
                queue,
                Arc::clone(&self.kura),
                self.service.provider_ingest_finalized_archive.clone(),
                self.service.reputation_finalized_archive.clone(),
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
            self.assert_complete_for_state(self.state.as_ref());
        }

        fn assert_complete_for_state(&self, state: &State) {
            assert_eq!(state.committed_height(), 1);
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
                state
                    .world_view()
                    .commit_qcs()
                    .get(&self.body.hash())
                    .is_none(),
                "Sumeragi v2 finality must not be projected into the legacy commit-QC store"
            );
            assert!(
                state
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

    struct SuccessorApplyFixture {
        context: wire::HeightContext,
        body: SignedBlock,
        task: ApplyTask,
        _body_root: tempfile::TempDir,
        store: V2BodyStore,
    }

    fn build_successor_apply_fixture(fixture: &ApplyFixture) -> SuccessorApplyFixture {
        assert_eq!(
            fixture.state.committed_height(),
            1,
            "successor fixture requires the committed parent"
        );
        let mut context = fixture.context.clone();
        context.height = 2;
        context.parent_commit_qc = Some(fixture.task.certificate().clone());
        context.validate().expect("valid successor height context");
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };

        let transaction = TransactionBuilder::new(
            context.chain_id.clone(),
            fixture.service.genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "reputation retained-capture successor".to_owned(),
        )])
        .sign(fixture.genesis_key.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
        let routing_plan = fixture
            .service
            .queue
            .route_plan_with_state(&accepted, fixture.state.as_ref())
            .expect("resolve successor transaction route");
        let leader_index = context.leader(round.view);
        let route = routing_plan.coordinator_route();
        let entrypoint_hash = Hash::from(accepted.hash_as_entrypoint());
        let lane_plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
            fixture.state.as_ref(),
            fixture.kura.as_ref(),
            &context,
            round.view,
            &context.roster[usize::try_from(leader_index).expect("successor leader index")]
                .validator,
            std::slice::from_ref(&route),
            std::slice::from_ref(&entrypoint_hash),
        )
        .expect("derive canonical successor lane plan");
        assert!(
            lane_plan.unavailable_indices.is_empty(),
            "successor fixture lane must be available"
        );
        let execution_context =
            BlockExecutionContextBundle::new(vec![execution_context_for_routing_plan(
                transaction.hash_as_entrypoint(),
                &routing_plan,
            )])
            .with_lane_payload_ownerships(lane_plan.ownerships);
        let creation_time_ms = fixture
            .body
            .header()
            .creation_time()
            .checked_add(fixture.service.block_cadence)
            .expect("successor logical time fits Duration")
            .max(
                transaction
                    .creation_time()
                    .checked_add(Duration::from_millis(1))
                    .expect("successor transaction floor fits Duration"),
            )
            .as_millis()
            .try_into()
            .expect("successor creation time fits u64");
        let mut header = BlockHeader::new(
            NonZeroU64::new(context.height).expect("non-zero successor height"),
            Some(fixture.body.hash()),
            None,
            None,
            creation_time_ms,
            0,
        );
        let confidential_features = {
            let state_view = fixture.state.view();
            let digest = crate::state::compute_confidential_feature_digest(
                state_view.world(),
                &state_view.zk,
                state_view.sccp_registry.as_ref(),
                context.height,
            );
            (!digest.is_empty()).then_some(digest)
        };
        header.set_confidential_features(confidential_features);
        let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
            &fixture.state.nexus_snapshot(),
            context.height,
        );
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic successor BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let leader = usize::try_from(leader_index).expect("successor leader index");
        assert_eq!(
            keys[leader].public_key(),
            context.roster[leader].validator.public_key(),
            "successor signer must be the rotating leader"
        );
        let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
        builder.push_transaction(transaction);
        builder.set_da_proof_policies(Some(proof_policy_bundle));
        builder.set_execution_context(Some(execution_context));
        let body = builder
            .try_build_with_signature(u64::from(leader_index), keys[leader].private_key())
            .expect("sign successor proposal")
            .canonical_resultless_proposal();
        let canonical_wire = body.encode_wire().expect("encode successor body");
        let subject = wire::BlockSubject {
            parent_block_hash: Some(fixture.body.hash()),
            block_hash: body.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest = wire::PayloadManifest::derive(
            &context,
            round,
            subject,
            u64::try_from(canonical_wire.len()).expect("successor body length"),
            std::slice::from_ref(&canonical_wire),
        )
        .expect("derive successor payload manifest");
        let execution_commitment = fixture
            .service
            .validate_candidate(&context, &body)
            .expect("derive successor execution commitment");
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
                    keys[usize::try_from(*index).expect("successor signer index")].private_key(),
                    &preimage,
                )
                .expect("sign successor Commit vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate successor Commit votes");

        let body_root = tempfile::tempdir().expect("successor body-store directory");
        let mut store = V2BodyStore::open(body_root.path(), context.clone())
            .expect("open successor rotating-leader body store");
        let durable = store
            .store(manifest, canonical_wire)
            .expect("persist exact successor body");
        let validated = store
            .validate(&durable, |candidate| {
                fixture.service.validate_candidate(&context, candidate)
            })
            .expect("persist successor validation marker");
        let task = ApplyTask::for_test(
            2,
            EventTag::new(2, 0, Generation::new(2)),
            subject,
            certificate,
            validated,
        );
        SuccessorApplyFixture {
            context,
            body,
            task,
            _body_root: body_root,
            store,
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
        assert_eq!(
            prospective_application_refinement_projection(
                &fixture.context,
                &fixture.task,
                fixture.body.hash(),
                fixture
                    .body
                    .canonical_proposal_wire_hash()
                    .expect("hash proposal wire"),
                evidence.artifact(),
            )
            .expect("prospective application projection"),
            evidence
                .application_refinement_projection()
                .expect("observed application projection"),
            "preflight and observed durable application identities must be exact"
        );
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
            queue_plan_admissions: Vec::new(),
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
        reservation: crate::queue::LaneQueueReservationKeyV2,
    ) -> (SignedBlock, MergeLedgerEntry) {
        let parent_key = KeyPair::try_from_seed(vec![0xC8; 32], Algorithm::BlsNormal)
            .expect("derive execution-carrier parent signer");
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let parent = BlockBuilder::new_with_time_source(Vec::new(), time_source)
            .chain(0, None)
            .try_sign_with_index(parent_key.private_key(), 0)
            .expect("sign execution-carrier parent")
            .unpack(|_| {});
        let parent = SignedBlock::from(parent);
        let application_block_header = BlockHeader::new(
            NonZeroU64::new(2).expect("non-zero fixture carrier height"),
            Some(parent.hash()),
            None,
            None,
            2,
            0,
        );
        let entrypoint_hashes = vec![Hash::from(entrypoint.hash())];
        let results = vec![TransactionResult::from(Ok(DataTriggerSequence::default()))];
        let result_hashes = results
            .iter()
            .map(|result| Hash::from(result.hash()))
            .collect::<Vec<_>>();
        let validator_set = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let validator_count =
            u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
        let min_quorum = wire::DualQuorum::count_threshold(validator_count)
            .expect("non-empty fixture validator set has a quorum");
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
            validator_count,
            min_quorum,
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
        crate::lane_consensus::validate_lane_block_proposal(&proposal)
            .expect("reservation fixture proposal must satisfy production ingress validation");
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
            application_block_header: application_block_header.clone(),
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
        entry.merge_qc.carrier_height = application_block_header.height().get();
        entry.merge_qc.carrier_parent_hash = application_block_header
            .prev_block_hash()
            .expect("non-genesis merge carrier has a parent");
        entry.merge_qc.view = application_block_header.view_change_index();
        entry.execution_batch = Some(batch);
        (parent, entry)
    }

    fn reserve_transaction_for_test(
        state: &State,
        queue: &Queue,
        transaction: iroha_data_model::transaction::SignedTransaction,
    ) -> (
        crate::queue::LaneQueueReservationKeyV2,
        TransactionEntrypoint,
    ) {
        let entrypoint = TransactionEntrypoint::External(transaction.clone());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
        let routing_plan = queue
            .route_plan_with_state(&accepted, state)
            .expect("resolve reservation fixture route");
        let admission_context = queue
            .plan_admission_context_with_state(state, &routing_plan)
            .expect("capture reservation fixture admission context");
        let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
            state.chain_id_ref(),
            accepted.entrypoint(),
            &routing_plan,
            admission_context,
            queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("build reservation fixture global admission binding");
        queue
            .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                accepted,
                state,
                routing_plan,
                &binding,
            )
            .expect("durably enqueue globally bound reservation fixture transaction");
        install_fixture_queue_plan_registry_value(state, &binding);
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

    fn install_fixture_queue_plan_registry_value(
        state: &State,
        binding: &crate::torii_proxy::QueuePlanAdmissionBindingV2,
    ) {
        let registry_key = binding.registry_key();
        let marker_key = format!(
            "queue_plan_admission_v2_{}_{}",
            hex::encode(registry_key.chain_id_digest.as_ref()),
            hex::encode(registry_key.entrypoint_hash.as_ref()),
        )
        .parse()
        .expect("reservation fixture registry marker key");
        let marker_value = crate::torii_proxy::QueuePlanAdmissionRegistryValueV2 {
            version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2,
            binding_hash: binding.canonical_hash(),
        };
        let marker_payload =
            norito::to_bytes(&marker_value).expect("encode reservation fixture registry marker");
        let mut world = state.world.block();
        world
            .smart_contract_state
            .insert(marker_key, marker_payload);
        world.commit();
    }

    fn reserve_autonomous_crash_batch(
        fixture: &ApplyFixture,
        queue: &Arc<Queue>,
        producer: &KeyPair,
    ) -> (LaneExecutablePayloadV1, Vec<HashOf<SignedTransaction>>) {
        let transactions = (0_u8..4)
            .map(|index| {
                TransactionBuilder::new(
                    fixture.context.chain_id.clone(),
                    fixture.service.genesis_account.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .with_instructions([Log::new(
                    Level::INFO,
                    format!("autonomous reservation crash boundary {index}"),
                )])
                .sign(fixture.genesis_key.private_key())
            })
            .collect::<Vec<_>>();
        let expected_fifo = transactions
            .iter()
            .map(|transaction| transaction.hash())
            .collect::<Vec<_>>();
        let entrypoints = transactions
            .iter()
            .take(3)
            .cloned()
            .map(TransactionEntrypoint::External)
            .collect::<Vec<_>>();
        let entrypoint_hashes = entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect::<Vec<_>>();
        let lane_incarnation = fixture
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, 1)
            .expect("default lane incarnation at autonomous proposal height");
        let validator_set = vec![PeerId::new(producer.public_key().clone())];
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation,
            proposal_height: 1,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::new(b"v2 autonomous crash reservation subject"),
            payload_ownership_hash: Hash::new(b"v2 autonomous crash reservation ownership"),
            rbc_instance_hash: Hash::new(b"v2 autonomous crash reservation RBC"),
            accepted_candidate_indices: (0_u64..3).collect(),
            accepted_transaction_hashes: entrypoint_hashes,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "permissioned:v2-autonomous-reservation-crash".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();

        for transaction in &transactions {
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
            let routing_plan = queue
                .route_plan_with_state(&accepted, fixture.state.as_ref())
                .expect("resolve autonomous crash reservation route");
            let admission_context = queue
                .plan_admission_context_with_state(fixture.state.as_ref(), &routing_plan)
                .expect("capture autonomous crash admission context");
            let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
                fixture.state.chain_id_ref(),
                accepted.entrypoint(),
                &routing_plan,
                admission_context,
                queue.queue_plan_admission_timestamp_ms(),
            )
            .expect("build autonomous crash global admission binding");
            queue
                .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                    accepted,
                    fixture.state.as_ref(),
                    routing_plan,
                    &binding,
                )
                .expect("durably enqueue autonomous crash reservation transaction");
            install_fixture_queue_plan_registry_value(fixture.state.as_ref(), &binding);
        }
        let scope = LaneQueueReservationScopeV1 {
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            proposal_height: proposal.descriptor.proposal_height,
            lane_block_height: proposal.descriptor.lane_block_height,
            lane_block_view: proposal.descriptor.lane_block_view,
            reservation_owner_hash: Hash::new(b"v2 autonomous crash reservation owner"),
            proposal_identity_hash: proposal.proposal_hash,
        };
        let reserved = queue
            .reserve_transactions_for_lane(
                fixture.state.as_ref(),
                scope,
                NonZeroUsize::new(3).expect("non-zero crash reservation count"),
            )
            .expect("reserve exact autonomous crash batch");
        assert_eq!(reserved.len(), 3);
        assert_eq!(
            reserved
                .iter()
                .map(|reserved| reserved.key().signed_transaction_hash)
                .collect::<Vec<_>>(),
            expected_fifo[..3],
            "fixture must reserve the original FIFO prefix"
        );
        let reservation_keys = reserved
            .iter()
            .map(|reserved| *reserved.key())
            .collect::<Vec<_>>();
        let routing_plans = reserved
            .iter()
            .map(|reserved| reserved.routing_plan().clone())
            .collect::<Vec<_>>();
        let chain_id_hash = Hash::new(fixture.context.chain_id.clone().into_inner().as_bytes());
        let epoch = {
            let world = fixture.state.world_view();
            crate::sumeragi::epoch_for_height_from_world(
                &world,
                proposal.descriptor.proposal_height,
            )
        };
        let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
            chain_id_hash,
            epoch,
            proposal,
            entrypoints,
            reservation_keys,
            routing_plans,
            vec![None; 3],
            validator_set[0].clone(),
            producer.private_key(),
        )
        .expect("build exact autonomous crash payload");
        (payload, expected_fifo)
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

    fn body_with_exact_merge_execution_header(entry: &MergeLedgerEntry) -> SignedBlock {
        let key = KeyPair::try_from_seed(vec![0xCA; 32], Algorithm::BlsNormal)
            .expect("derive execution-carrier signer");
        let header = entry
            .execution_batch
            .as_ref()
            .expect("execution merge entry")
            .application_block_header
            .clone();
        assert_eq!(
            entry.merge_qc.carrier_height,
            header.height().get(),
            "execution fixture QC must bind the carrier height"
        );
        assert_eq!(
            Some(entry.merge_qc.carrier_parent_hash),
            header.prev_block_hash(),
            "execution fixture QC must bind the carrier parent"
        );
        assert_eq!(
            entry.merge_qc.view,
            header.view_change_index(),
            "execution fixture QC must bind the carrier view"
        );
        let execution_context = BlockExecutionContextBundle::new(Vec::new())
            .with_merge_entry(CertifiedMergeLedgerReference::new(entry));
        let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
        builder.set_execution_context(Some(execution_context));
        let carrier = builder.build_with_signature(0, key.private_key());
        assert_eq!(
            crate::merge::merge_application_header_from_carrier(&carrier.header()),
            entry
                .execution_batch
                .as_ref()
                .expect("execution merge entry")
                .application_block_header,
            "signed carrier must preserve the certified application header"
        );
        carrier
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

    v2_apply_test!(
        live_merge_publication_persists_application_receipt_before_retry,
        {
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
                .install_plan_journal(
                    journal_dir.path().join("queue-plans.norito"),
                    1024 * 1024,
                    true,
                )
                .expect("install queue-plan journal");
            queue
                .install_lane_reservation_journal(
                    journal_dir.path().join("lane-reservations.norito"),
                    1024 * 1024,
                )
                .expect("install reservation journal");
            let (reservation, entrypoint) =
                reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
            let (parent, entry) =
                merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
            let carrier = body_with_exact_merge_execution_header(&entry);
            fixture
                .kura
                .store_block(Arc::new(parent.clone()))
                .expect("persist execution-carrier parent");
            fixture
                .kura
                .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
                .expect("persist exact execution carrier and merge log");
            fixture
                .state
                .seed_applied_merge_entry_for_v2_settlement_test(&entry)
                .expect("seed exact post-commit merge state");
            let mut block_hashes = fixture.state.block_hashes.block();
            block_hashes.push_for_tests(parent.hash());
            block_hashes.push_for_tests(carrier.hash());
            block_hashes.commit_for_tests();
            fixture
                .state
                .update_latest_block_header_cache_for_tests(carrier.header().clone());

            fixture
                .service
                .publish_committed_block_merge_entry(&carrier)
                .expect("publish live execution merge entry");
            let receipt = fixture
                .kura
                .read_lane_block_application_receipt(LaneId::SINGLE, 1)
                .expect("live post-WSV publication must persist the application receipt");
            assert_eq!(
                receipt.format,
                crate::kura::LaneBlockApplicationReceiptArtifactFormat::MergeExecution
            );
            let receipt_hash = HashOf::new(&receipt);

            fixture
                .service
                .publish_committed_block_merge_entry(&carrier)
                .expect("retry exact live execution merge publication");
            assert_eq!(
                fixture
                    .kura
                    .read_lane_block_application_receipt(LaneId::SINGLE, 1)
                    .as_ref()
                    .map(HashOf::new),
                Some(receipt_hash),
                "crash retry must preserve one byte-identical receipt"
            );
        }
    );

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
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let (reservation, entrypoint) =
            reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
        let (_parent, entry) =
            merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
        fixture
            .kura
            .append_merge_entry(&entry)
            .expect("persist committed merge history fixture");
        let carrier = body_with_exact_merge_execution_header(&entry);
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
            .install_plan_journal(
                journal_dir.path().join("queue-plans.norito"),
                1024 * 1024,
                true,
            )
            .expect("install queue-plan journal");
        queue
            .install_lane_reservation_journal(
                journal_dir.path().join("lane-reservations.norito"),
                1024 * 1024,
            )
            .expect("install reservation journal");
        let (reservation, entrypoint) =
            reserve_transaction_for_test(fixture.state.as_ref(), &queue, transaction);
        let (_parent, mut entry) =
            merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
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
                .install_plan_journal(
                    journal_dir.path().join("queue-plans.norito"),
                    1024 * 1024,
                    true,
                )
                .expect("install first-process queue-plan journal");
            first_queue
                .install_lane_reservation_journal(&journal_path, 1024 * 1024)
                .expect("install first-process reservation journal");
            let (reservation, entrypoint) =
                reserve_transaction_for_test(fixture.state.as_ref(), &first_queue, transaction);
            let (parent, entry) =
                merge_entry_with_reservation(&fixture.context, entrypoint, reservation);
            let carrier = body_with_exact_merge_execution_header(&entry);
            fixture
                .kura
                .store_block(Arc::new(parent))
                .expect("persist execution-carrier parent");
            fixture
                .kura
                .store_block_with_merge_entry(Arc::new(carrier), &entry)
                .expect("persist committed merge carrier and exact sidecar");
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
            fixture.kura.reset_merge_query_read_counters_for_test();

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
            let (full_history_scans, _, indexed_lookups) =
                fixture.kura.merge_query_read_counters_for_test();
            assert_eq!(
                full_history_scans, 0,
                "startup reservation reconciliation must not materialize merge history"
            );
            assert_eq!(
                indexed_lookups, 1,
                "startup reconciliation must decode only the exact committed reservation frame"
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
        autonomous_reservation_cross_store_crash_matrix_preserves_fifo_exactly_once,
        {
            for boundary in [
                "kura_release_pending",
                "queue_prepared_barrier",
                "kura_released",
                "queue_completion_forgotten",
            ] {
                let fixture = ApplyFixture::new();
                let producer = KeyPair::try_from_seed(vec![0xB7; 32], Algorithm::BlsNormal)
                    .expect("derive autonomous crash producer");
                let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
                let journal_dir =
                    tempfile::tempdir().expect("autonomous reservation crash journals");
                let plan_path = journal_dir.path().join("queue-plans.norito");
                let reservation_path = journal_dir.path().join("lane-reservations.norito");
                let queue = Arc::new(Queue::from_config(
                    QueueConfig::default(),
                    events_sender.clone(),
                ));
                queue
                    .install_plan_journal(&plan_path, 1024 * 1024, true)
                    .expect("install autonomous crash plan journal");
                queue
                    .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                    .expect("install autonomous crash reservation journal");
                let (payload, expected_fifo) =
                    reserve_autonomous_crash_batch(&fixture, &queue, &producer);
                let descriptor = &payload.origin_proposal.descriptor;
                let lane_config = RuntimeLaneConfig::default();
                fixture
                    .kura
                    .install_lane_incarnation_marker_for_test(
                        lane_config.primary(),
                        descriptor.lane_incarnation,
                        0,
                    )
                    .expect("install autonomous crash lane marker");
                fixture
                    .kura
                    .persist_lane_executable_payload(&payload, payload.chain_id_hash, payload.epoch)
                    .expect("persist autonomous crash payload");
                let retirement =
                    crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
                let barrier = retirement
                    .queue_release_barrier()
                    .expect("build autonomous crash release barrier");

                match boundary {
                    "kura_release_pending" => {
                        fixture
                            .kura
                            .persist_autonomous_lane_slot_retirement(
                                &retirement,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("persist Kura ReleasePending boundary");
                    }
                    "queue_prepared_barrier" => {
                        fixture
                            .kura
                            .persist_autonomous_lane_slot_retirement(
                                &retirement,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("persist Kura retirement before Queue barrier");
                        queue
                            .prepare_lane_reservation_release_barrier(&barrier)
                            .expect("persist Queue prepared barrier");
                    }
                    "kura_released" => {
                        fixture
                            .kura
                            .persist_autonomous_lane_slot_retirement(
                                &retirement,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("persist Kura retirement before released claims");
                        queue
                            .prepare_lane_reservation_release_barrier(&barrier)
                            .expect("persist Queue barrier before Kura Released");
                        fixture
                            .kura
                            .finalize_autonomous_lane_slot_release(
                                &retirement,
                                &barrier,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("persist Kura Released boundary");
                    }
                    "queue_completion_forgotten" => {
                        assert_eq!(
                            retire_autonomous_lane_slot_and_release_reservations(
                                fixture.kura.as_ref(),
                                queue.as_ref(),
                                &retirement,
                                payload.chain_id_hash,
                                payload.epoch,
                            )
                            .expect("complete production retirement hand-off"),
                            3
                        );
                    }
                    _ => unreachable!("enumerated crash boundary"),
                }
                assert_eq!(
                    fixture
                        .kura
                        .read_autonomous_lane_slot_retirement(
                            descriptor.lane_id,
                            descriptor.lane_block_height,
                            payload.chain_id_hash,
                            payload.epoch,
                        )
                        .expect("read crash-boundary retirement"),
                    Some(retirement.clone()),
                    "{boundary}: every crash image must retain the exact Kura retirement"
                );
                if boundary == "queue_completion_forgotten" {
                    assert!(queue.live_lane_reservations().is_empty());
                    assert!(queue.lane_reservation_release_barriers().is_empty());
                    assert_eq!(queue.queued_len(), 4);
                } else {
                    assert_eq!(queue.live_lane_reservations().len(), 3);
                    assert_eq!(queue.queued_len(), 1);
                    assert_eq!(
                        queue.lane_reservation_release_barriers(),
                        if boundary == "kura_release_pending" {
                            Vec::new()
                        } else {
                            vec![barrier.clone()]
                        },
                        "{boundary}: Queue must expose exactly its durable crash boundary"
                    );
                }
                drop(queue);

                let replayed_queue = Arc::new(Queue::from_config(
                    QueueConfig::default(),
                    events_sender.clone(),
                ));
                let replay = replayed_queue
                    .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                    .expect("replay first crash-boundary reservation journal");
                replayed_queue
                    .install_plan_journal(&plan_path, 1024 * 1024, true)
                    .expect("install first replay plan journal");
                replayed_queue
                    .replay_plan_journal(fixture.state.as_ref())
                    .expect("replay first crash-boundary queue plans");
                if boundary == "queue_completion_forgotten" {
                    assert_eq!(replay.restored, 0);
                    assert_eq!(replay.release_barriers, 0);
                    assert_eq!(replay.completed_releases, 0);
                } else {
                    assert_eq!(replay.restored, 3);
                    assert_eq!(
                        replay.release_barriers,
                        usize::from(boundary != "kura_release_pending")
                    );
                    assert_eq!(replay.completed_releases, 0);
                    assert_eq!(
                        replayed_queue.queued_len(),
                        1,
                        "{boundary}: replay must not make lane-owned work selectable"
                    );
                }
                assert_eq!(
                    reconcile_lane_reservation_ownership(
                        fixture.state.as_ref(),
                        replayed_queue.as_ref(),
                        fixture.kura.as_ref(),
                        &fixture.context.chain_id,
                    )
                    .expect("reconcile first cross-store crash image"),
                    if boundary == "queue_completion_forgotten" {
                        (0, 0)
                    } else {
                        (0, 3)
                    },
                    "{boundary}: reconciliation must transfer each owner exactly once"
                );
                assert!(replayed_queue.live_lane_reservations().is_empty());
                assert!(
                    replayed_queue
                        .lane_reservation_release_barriers()
                        .is_empty()
                );
                assert_eq!(
                    replayed_queue.queued_len(),
                    4,
                    "{boundary}: release must restore all work without loss"
                );
                assert_eq!(
                    retire_autonomous_lane_slot_and_release_reservations(
                        fixture.kura.as_ref(),
                        replayed_queue.as_ref(),
                        &retirement,
                        payload.chain_id_hash,
                        payload.epoch,
                    )
                    .expect("retry complete production retirement hand-off"),
                    0,
                    "{boundary}: completed hand-off replay must be idempotent"
                );
                drop(replayed_queue);

                let replayed_again = Arc::new(Queue::from_config(
                    QueueConfig::default(),
                    events_sender.clone(),
                ));
                let terminal_replay = replayed_again
                    .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                    .expect("replay terminal reservation journal");
                assert_eq!(terminal_replay.restored, 0);
                assert_eq!(terminal_replay.release_barriers, 0);
                assert_eq!(terminal_replay.completed_releases, 0);
                replayed_again
                    .install_plan_journal(&plan_path, 1024 * 1024, true)
                    .expect("install terminal replay plan journal");
                replayed_again
                    .replay_plan_journal(fixture.state.as_ref())
                    .expect("replay terminal queue plans");
                assert_eq!(
                    reconcile_lane_reservation_ownership(
                        fixture.state.as_ref(),
                        replayed_again.as_ref(),
                        fixture.kura.as_ref(),
                        &fixture.context.chain_id,
                    )
                    .expect("repeat terminal ownership reconciliation"),
                    (0, 0),
                    "{boundary}: a second restart must not release or commit twice"
                );
                assert_eq!(replayed_again.active_len(), 4);
                assert_eq!(replayed_again.queued_len(), 4);
                assert!(replayed_again.live_lane_reservations().is_empty());

                let replacement_scope = LaneQueueReservationScopeV1 {
                    lane_id: descriptor.lane_id,
                    dataspace_id: descriptor.dataspace_id,
                    lane_incarnation: fixture
                        .state
                        .lane_incarnation_at_height(descriptor.lane_id, 2)
                        .expect("active incarnation for replacement proposal"),
                    proposal_height: 2,
                    lane_block_height: 2,
                    lane_block_view: 0,
                    reservation_owner_hash: Hash::new(
                        format!("replacement owner after {boundary}").as_bytes(),
                    ),
                    proposal_identity_hash: Hash::new(
                        format!("replacement proposal after {boundary}").as_bytes(),
                    ),
                };
                let replacement = replayed_again
                    .reserve_transactions_for_lane(
                        fixture.state.as_ref(),
                        replacement_scope,
                        NonZeroUsize::new(1).expect("one replacement reservation"),
                    )
                    .expect("reserve restored FIFO head under a fresh owner");
                assert_eq!(replacement.len(), 1);
                assert_eq!(
                    replacement[0].key().signed_transaction_hash,
                    expected_fifo[0],
                    "{boundary}: restored work must not be overtaken"
                );
                assert_ne!(
                    replacement[0].key(),
                    &barrier.ordered_keys[0],
                    "{boundary}: replacement ownership must have a fresh exact identity"
                );
                let stale_barrier =
                    replayed_again.prepare_lane_reservation_release_barrier(&barrier);
                assert!(
                    matches!(
                        &stale_barrier,
                        Err(LaneQueueReservationError::Conflict { .. })
                    ),
                    "{boundary}: stale release barrier must fail ABA-safe: {stale_barrier:?}"
                );
                assert_eq!(
                    replayed_again.live_lane_reservations(),
                    vec![*replacement[0].key()],
                    "{boundary}: stale barrier must not disturb replacement ownership"
                );
                assert_eq!(replayed_again.active_len(), 4);
                assert_eq!(replayed_again.queued_len(), 3);

                let mut remaining = Vec::new();
                replayed_again.get_transactions_for_block_with_state(
                    fixture.state.as_ref(),
                    NonZeroUsize::new(3).expect("remaining FIFO length"),
                    &mut remaining,
                );
                let remaining_hashes = remaining
                    .iter()
                    .map(|transaction| transaction.as_ref().hash())
                    .collect::<Vec<_>>();
                assert_eq!(
                    remaining_hashes,
                    expected_fifo[1..],
                    "{boundary}: release replay must preserve FIFO order behind the new owner"
                );
                let observed = std::iter::once(replacement[0].key().signed_transaction_hash)
                    .chain(remaining_hashes)
                    .collect::<BTreeSet<_>>();
                assert_eq!(observed.len(), 4);
                assert_eq!(
                    observed,
                    expected_fifo.iter().copied().collect(),
                    "{boundary}: restart/release must neither lose nor duplicate ownership"
                );
            }
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
        same_body_reproposal_commit_qc_applies_exact_reproposal_body,
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
            certificate.proposal_round = certificate.round;
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
                    .expect("sign same-round reproposal Commit vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate same-round reproposal Commit votes");
            let later_round = certificate.round;
            let later_tag = EventTag::new(
                fixture.context.height,
                later_round.view,
                fixture.task.tag().generation(),
            );
            let mut store = fixture.reopen_body_store();
            let canonical_wire = fixture
                .body
                .encode_wire()
                .expect("encode unchanged locked body");
            let reproposal_manifest = wire::PayloadManifest::derive(
                &fixture.context,
                later_round,
                fixture.task.subject(),
                u64::try_from(canonical_wire.len()).expect("body length"),
                std::slice::from_ref(&canonical_wire),
            )
            .expect("derive later-round manifest for unchanged locked body");
            let durable = store
                .store(reproposal_manifest, canonical_wire.clone())
                .expect("persist later-round manifest for unchanged locked body");
            let reproposal_receipt = store
                .validate(&durable, |candidate| {
                    assert_eq!(
                        candidate
                            .encode_wire()
                            .expect("encode reproposed candidate"),
                        canonical_wire,
                        "reproposal must retain the exact canonical locked body bytes"
                    );
                    fixture
                        .service
                        .validate_candidate(&fixture.context, candidate)
                })
                .expect("validate unchanged body under the later proposal round");
            assert_eq!(
                reproposal_receipt.durable().round(),
                certificate.proposal_round,
                "the durable receipt must bind the unchanged body to its later reproposal round"
            );
            assert_eq!(
                reproposal_receipt.execution_commitment(),
                fixture.task.validated_receipt().execution_commitment(),
                "view rotation must not change deterministic execution"
            );
            let task = ApplyTask::for_test(
                2,
                later_tag,
                fixture.task.subject(),
                certificate,
                reproposal_receipt,
            );
            fixture.task = task;

            fixture
                .execute(&mut store)
                .expect("reproposal CommitQC applies the exact unchanged body");
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

    v2_apply_test!(native_amx_prepublication_failure_leaves_wsv_unchanged, {
        let fixture = ApplyFixture::new();
        let baseline_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        fixture.kura.fail_next_native_amx_prepublication_for_tests();
        let mut store = fixture.reopen_body_store();
        let error = fixture
            .execute(&mut store)
            .expect_err("inject Native evidence publication failure before WSV staging");
        assert!(matches!(
            &error,
            V2ApplyError::CommittedRecoveryRequired {
                stage: "pre-WSV Native AMX participant evidence publication",
                ..
            }
        ));
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            baseline_state_hash,
            "prepublication failure must not leak the validated State overlay"
        );
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            fixture
                .kura
                .v2_finality_artifact(fixture.context.height)
                .expect("read pre-WSV finality")
                .is_some(),
            "durable finality must precede Native evidence prepublication"
        );
        assert!(
            fixture
                .kura
                .wsv_checkpoint(fixture.context.height)
                .expect("read absent pre-WSV checkpoint")
                .is_none()
        );
        assert!(
            fixture
                .kura
                .commit_manifest(fixture.context.height)
                .expect("read absent pre-WSV commit manifest")
                .is_none()
        );

        fixture
            .execute(&mut store)
            .expect("retry exact carrier after prepublication failure");
        fixture.assert_complete();
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
            assert!(
                fixture
                    .kura
                    .commit_manifest(fixture.context.height)
                    .expect("read absent manifest")
                    .is_none()
            );
            assert_eq!(
                fixture
                    .kura
                    .v2_finality_artifact(fixture.context.height)
                    .expect("read pre-WSV finality")
                    .expect("finality must precede the WSV checkpoint")
                    .block_hash,
                fixture.body.hash()
            );

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
        provider_ingest_archive_failure_after_kura_and_checkpoint_keeps_state_unpublished,
        {
            let mut fixture = ApplyFixture::new();
            let archive_root = direct_archive_tempdir();
            let archive = Arc::new(
                ProviderIngestFinalizedArchiveV1::try_open(
                    archive_root.path(),
                    provider_ingest_archive_bounds(32, 32),
                )
                .expect("open deliberately tiny provider-ingest archive"),
            );
            fixture.service.provider_ingest_finalized_archive = Some(Arc::clone(&archive));
            let mut store = fixture.reopen_body_store();

            let error = fixture
                .execute(&mut store)
                .expect_err("provider-ingest capture must exceed the tiny record bound");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::CommittedRecoveryRequired { stage, .. }
                        if *stage == "provider-ingest finalized archive capture"
                ),
                "unexpected committed recovery classification: {error:?}"
            );
            assert!(error.requires_restart_recovery());
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
            assert!(
                fixture
                    .kura
                    .wsv_checkpoint(1)
                    .expect("read staged checkpoint")
                    .is_some(),
                "WSV checkpoint must precede provider-ingest archive capture"
            );
            assert!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read durable finality")
                    .is_some(),
                "authenticated Kura finality must precede provider-ingest capture"
            );
            assert_eq!(
                fixture.state.committed_height(),
                0,
                "provider-ingest archive failure must precede live State publication"
            );
            assert!(
                archive
                    .activation_floor(&fixture.service.chain_id)
                    .expect("read empty activation floor")
                    .is_none()
            );
        }
    );

    v2_apply_test!(
        provider_ingest_archive_recovery_replays_exact_capture_without_duplicate_generation,
        {
            let mut fixture = ApplyFixture::new();
            let archive_root = direct_archive_tempdir();
            let archive = Arc::new(
                ProviderIngestFinalizedArchiveV1::try_open(
                    archive_root.path(),
                    provider_ingest_archive_bounds(4 * 1024 * 1024, 64 * 1024 * 1024),
                )
                .expect("open provider-ingest archive"),
            );
            fixture.service.provider_ingest_finalized_archive = Some(Arc::clone(&archive));
            fixture
                .service
                .fail_after_provider_ingest_archive_capture_for_test();
            let mut first_store = fixture.reopen_body_store();

            let error = fixture
                .execute(&mut first_store)
                .expect_err("inject crash after provider-ingest archive capture");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::InjectedCrashAfterProviderIngestArchiveCapture
                ),
                "unexpected provider-ingest capture result: {error:?}"
            );
            assert!(error.requires_restart_recovery());
            assert_eq!(fixture.state.committed_height(), 0);
            let generation_after_capture = archive
                .health_generation()
                .expect("read first provider-ingest archive generation");
            let qualification = archive
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("provider-ingest archive is exact at the durable Kura tip");
            assert_eq!(qualification.activation_floor().height, 1);
            assert_eq!(qualification.archive_tip().height, 1);
            assert_eq!(qualification.kura_tip_height(), 1);
            assert_eq!(qualification.lag_blocks(), 0);
            drop(first_store);

            let (restarted_service, restarted_state) =
                fixture.restart_service_from_last_finalized_snapshot();
            let mut restarted_store = fixture.reopen_body_store();
            restarted_service
                .execute(&fixture.context, &mut restarted_store, &fixture.task)
                .expect("restart replays the exact provider-ingest capture and publishes WSV");
            assert_eq!(restarted_state.committed_height(), 1);
            assert_eq!(
                archive
                    .health_generation()
                    .expect("read replayed provider-ingest archive generation"),
                generation_after_capture,
                "an exact replay must not publish another archive generation"
            );

            let (_, receipt) = restarted_service
                .kura
                .v2_finality_artifact_with_receipt(1)
                .expect("read exact finality receipt")
                .expect("height-one finality receipt");
            let restarted_view = restarted_state.query_view();
            let outcome = archive
                .capture_kura_authenticated_view(&restarted_view, fixture.kura.as_ref(), &receipt)
                .expect("same exact provider-ingest committed view is replay-safe");
            assert_eq!(
                outcome,
                ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
            );
        }
    );

    v2_apply_test!(
        reputation_archive_failure_after_kura_and_checkpoint_keeps_state_unpublished,
        {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds = ReputationFinalizedArchiveBounds::try_new(32, 16, 32)
                .expect("valid deliberately tiny archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open tiny archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
            let mut store = fixture.reopen_body_store();

            let error = fixture
                .execute(&mut store)
                .expect_err("archive capture must exceed the deliberately tiny bound");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::CommittedRecoveryRequired { stage, .. }
                        if *stage == "reputation finalized archive capture"
                ),
                "unexpected committed recovery classification: {error:?}"
            );
            assert!(error.requires_restart_recovery());
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
            assert!(
                fixture
                    .kura
                    .wsv_checkpoint(1)
                    .expect("read staged checkpoint")
                    .is_some(),
                "WSV checkpoint must precede archive capture"
            );
            assert!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read durable finality")
                    .is_some(),
                "authenticated Kura finality must precede archive capture"
            );
            assert_eq!(
                fixture.state.committed_height(),
                0,
                "archive failure must precede live State publication"
            );
            assert!(
                archive
                    .activation_floor(&fixture.service.chain_id)
                    .expect("read empty activation floor")
                    .is_none()
            );
        }
    );

    v2_apply_test!(
        crash_after_reputation_archive_capture_precedes_state_block_commit,
        {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds =
                ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                    .expect("valid archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
            fixture
                .service
                .fail_after_reputation_archive_capture_for_test();
            let mut store = fixture.reopen_body_store();

            let error = fixture
                .execute(&mut store)
                .expect_err("injected post-capture crash");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::InjectedCrashAfterReputationArchiveCapture
                ),
                "unexpected post-capture result: {error:?}"
            );
            assert_eq!(
                fixture.state.committed_height(),
                0,
                "injected archive-boundary crash must precede StateBlock::commit"
            );
            let qualification = archive
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("captured archive is exact at the durable Kura tip");
            assert_eq!(qualification.activation_floor().height, 1);
            assert_eq!(qualification.archive_tip().height, 1);
            assert_eq!(qualification.kura_tip_height(), 1);
            assert_eq!(qualification.lag_blocks(), 0);
            assert!(
                fixture
                    .kura
                    .commit_manifest(1)
                    .expect("read absent post-apply manifest")
                    .is_none(),
                "post-apply metadata must remain behind State publication"
            );
        }
    );

    v2_apply_test!(
        reputation_archive_recovery_is_idempotent_without_skipping_height,
        {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds =
                ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                    .expect("valid archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
            fixture
                .service
                .fail_after_reputation_archive_capture_for_test();
            let mut first_store = fixture.reopen_body_store();
            let error = fixture
                .execute(&mut first_store)
                .expect_err("injected post-capture crash");
            assert!(
                matches!(
                    &error,
                    V2ApplyError::InjectedCrashAfterReputationArchiveCapture
                ),
                "unexpected post-capture result: {error:?}"
            );
            let generation_after_capture = archive
                .health_generation()
                .expect("read first archive generation");
            let staged_checkpoint = fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read pre-retry staged checkpoint")
                .expect("archive-boundary crash retains its staged checkpoint");
            assert!(
                fixture
                    .kura
                    .commit_manifest(1)
                    .expect("read pre-retry commit manifest")
                    .is_none(),
                "archive-boundary crash must precede commit-manifest publication"
            );
            drop(first_store);

            let (restarted_service, restarted_state) =
                fixture.restart_service_from_last_finalized_snapshot();
            let mut restarted_store = fixture.reopen_body_store();
            if let Err(error) =
                restarted_service.execute(&fixture.context, &mut restarted_store, &fixture.task)
            {
                let checkpoint_after_retry = fixture.kura.wsv_checkpoint(1);
                let manifest_after_retry = fixture.kura.commit_manifest(1);
                let replay_state_hash =
                    crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref());
                panic!(
                    "exact durable replay reuses the archived height: {error:?}; \
                     staged_state_hash={:?}; replay_state_hash={replay_state_hash:?}; \
                     checkpoint_after_retry={checkpoint_after_retry:?}; \
                     manifest_after_retry={manifest_after_retry:?}",
                    staged_checkpoint.state_hash(),
                );
            }
            assert_eq!(restarted_state.committed_height(), 1);
            assert_eq!(
                archive
                    .health_generation()
                    .expect("read replayed archive generation"),
                generation_after_capture,
                "an exact replay must not publish a second archive generation"
            );
            let projection = archive
                .latest_at_or_before(&fixture.service.chain_id, 1)
                .expect("read recovered projection")
                .expect("height one remains archived");
            assert_eq!(projection.key.height, 1);
            let qualification = archive
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("recovered archive remains contiguous and exact");
            assert_eq!(qualification.activation_floor().height, 1);
            assert_eq!(qualification.archive_tip().height, 1);

            let (_, receipt) = restarted_service
                .kura
                .v2_finality_artifact_with_receipt(1)
                .expect("read exact finality receipt")
                .expect("height one finality receipt");
            let restarted_view = restarted_state.query_view();
            let outcome = archive
                .capture_kura_authenticated_view(&restarted_view, fixture.kura.as_ref(), &receipt)
                .expect("same exact committed view is replay-safe");
            assert_eq!(
                outcome,
                ReputationFinalizedArchiveInsertOutcome::ExactReplay
            );
        }
    );

    v2_apply_test!(
        reputation_archive_virtual_base_allows_commit_owned_successor_capture,
        {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds =
                ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                    .expect("valid retained-capture archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open retained-capture archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));

            let mut parent_store = fixture.reopen_body_store();
            fixture
                .execute(&mut parent_store)
                .expect("commit and archive the retention-floor block");
            drop(parent_store);
            let parent = archive
                .latest_at_or_before(&fixture.service.chain_id, 1)
                .expect("read retention-floor projection")
                .expect("height-one archive anchor");
            let fence = archive
                .retention_fence_for(&parent.key)
                .expect("freeze exact authenticated retention fence");
            let retention_authority = ReputationRetentionAuthorityForTest::new();
            let retention_binding = retention_authority.binding();
            let proposal = archive
                .prepare_kura_authenticated_compaction(&fence, fixture.kura.as_ref())
                .expect("prepare exact Kura-authenticated checkpoint");
            let compaction = archive
                .approve_and_install_kura_authenticated_compaction(
                    &proposal,
                    fixture.kura.as_ref(),
                    &retention_binding,
                    &retention_authority,
                )
                .expect("approve and compact the Kura-authenticated height-one prefix");
            assert_eq!(compaction.retention_floor(), &parent.key);
            assert_eq!(
                compaction.generation(),
                fence.expected_generation() + 1,
                "checkpoint-head publication advances the archive generation"
            );
            assert_eq!(
                archive
                    .retention_floor(&fixture.service.chain_id)
                    .expect("read active virtual base"),
                Some(parent.key.clone())
            );

            let mut successor = build_successor_apply_fixture(&fixture);
            let completion = match fixture.service.execute(
                &successor.context,
                &mut successor.store,
                &successor.task,
            ) {
                Ok(completion) => completion,
                Err(error) => {
                    assert!(
                        !matches!(
                            &error,
                            V2ApplyError::CommittedRecoveryRequired { stage, .. }
                                if *stage == "reputation finalized archive capture"
                        ),
                        "retained capture must not degrade to committed recovery: {error:?}"
                    );
                    panic!(
                        "commit-owned H+1 capture failed after virtual-base compaction: {error:?}"
                    );
                }
            };
            assert_eq!(completion.receipt().height(), 2);
            assert_eq!(fixture.state.committed_height(), 2);
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 2);
            assert_eq!(
                fixture
                    .kura
                    .get_durable_block_hash(NonZeroUsize::new(2).expect("height two")),
                Some(successor.body.hash())
            );
            let qualification = archive
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("qualify retained successor against exact Kura tip");
            assert_eq!(qualification.activation_floor().height, 1);
            assert_eq!(qualification.archive_tip().height, 2);
            assert_eq!(
                qualification.checkpoint_digest(),
                Some(compaction.checkpoint_digest())
            );
            assert_eq!(qualification.kura_tip_height(), 2);
            assert_eq!(qualification.lag_blocks(), 0);
            let generation = archive
                .health_generation()
                .expect("read post-successor archive generation");

            drop(successor);
            fixture.service.reputation_finalized_archive = None;
            drop(archive);
            let reopened = ReputationFinalizedArchive::try_open_with_retention_authority(
                archive_root.path(),
                bounds,
                &fixture.service.chain_id,
                fixture.kura.as_ref(),
                &retention_binding,
                &retention_authority,
            )
            .expect("reopen compacted successor archive through sealed authority");
            assert_eq!(
                reopened
                    .health_generation()
                    .expect("read reopened archive generation"),
                generation
            );
            assert_eq!(
                reopened
                    .retention_floor(&fixture.service.chain_id)
                    .expect("read reopened virtual base"),
                Some(parent.key)
            );
            let reopened_qualification = reopened
                .qualify_against_kura_tip(&fixture.service.chain_id, fixture.kura.as_ref(), 0)
                .expect("reopened archive preserves exact predecessor continuity");
            assert_eq!(reopened_qualification.archive_tip().height, 2);
            assert_eq!(
                reopened_qualification.checkpoint_digest(),
                Some(compaction.checkpoint_digest())
            );
            assert_eq!(reopened_qualification.kura_tip_height(), 2);

            let (_, receipt) = fixture
                .kura
                .v2_finality_artifact_with_receipt(2)
                .expect("read height-two finality receipt")
                .expect("height-two finality is durable");
            let view = fixture.state.query_view();
            assert_eq!(
                reopened
                    .capture_kura_authenticated_view(&view, fixture.kura.as_ref(), &receipt)
                    .expect("replay retained H+1 capture after reopen"),
                ReputationFinalizedArchiveInsertOutcome::ExactReplay
            );
            assert_eq!(
                reopened
                    .health_generation()
                    .expect("exact replay preserves archive generation"),
                generation
            );
        }
    );

    v2_apply_test!(
        reputation_retention_restart_recovers_both_cas_publication_boundaries,
        {
            for (failed_load_after_cas, checkpoint_was_published) in [(1, false), (3, true)] {
                let mut fixture = ApplyFixture::new_with_reputation_archive();
                let archive_root = direct_archive_tempdir();
                let bounds = ReputationFinalizedArchiveBounds::try_new(
                    4 * 1024 * 1024,
                    16,
                    64 * 1024 * 1024,
                )
                .expect("valid retention-recovery archive bounds");
                let archive = Arc::new(
                    ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                        .expect("open retention-recovery archive"),
                );
                fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));

                let mut parent_store = fixture.reopen_body_store();
                fixture
                    .execute(&mut parent_store)
                    .expect("commit and archive the retention-floor block");
                drop(parent_store);
                let parent = archive
                    .latest_at_or_before(&fixture.service.chain_id, 1)
                    .expect("read retention-floor projection")
                    .expect("height-one archive anchor");
                let fence = archive
                    .retention_fence_for(&parent.key)
                    .expect("freeze exact retention fence");
                let authority = ReputationRetentionAuthorityForTest::new();
                let binding = authority.binding();
                let proposal = archive
                    .prepare_kura_authenticated_compaction(&fence, fixture.kura.as_ref())
                    .expect("prepare exact retention proposal");
                authority.fail_nth_load_after_next_cas(failed_load_after_cas);

                assert!(matches!(
                    archive.approve_and_install_kura_authenticated_compaction(
                        &proposal,
                        fixture.kura.as_ref(),
                        &binding,
                        &authority,
                    ),
                    Err(ReputationFinalizedArchiveError::RetentionAuthorityCasAmbiguous)
                ));
                assert_eq!(
                    archive
                        .retention_floor(&fixture.service.chain_id)
                        .expect("read ambiguous local retention floor"),
                    checkpoint_was_published.then(|| parent.key.clone())
                );
                assert_eq!(
                    std::fs::read_dir(archive.root().join("anchors"))
                        .expect("read ambiguous anchor namespace")
                        .count(),
                    1,
                    "no physical anchor may be unlinked before the post-publication readback"
                );
                assert_eq!(
                    std::fs::read_dir(archive.root().join("checkpoints"))
                        .expect("read ambiguous checkpoint namespace")
                        .count(),
                    usize::from(checkpoint_was_published)
                );

                fixture.service.reputation_finalized_archive = None;
                drop(archive);
                let recovered = ReputationFinalizedArchive::try_open_with_retention_authority(
                    archive_root.path(),
                    bounds,
                    &fixture.service.chain_id,
                    fixture.kura.as_ref(),
                    &binding,
                    &authority,
                )
                .expect("recover exact externally approved retention state");
                assert_eq!(
                    recovered
                        .retention_floor(&fixture.service.chain_id)
                        .expect("read recovered retention floor"),
                    Some(parent.key.clone())
                );
                assert_eq!(
                    std::fs::read_dir(recovered.root().join("anchors"))
                        .expect("read recovered anchor namespace")
                        .count(),
                    0,
                    "recovery must clean the exact approved physical prefix"
                );
                assert_eq!(
                    std::fs::read_dir(recovered.root().join("checkpoints"))
                        .expect("read recovered checkpoint namespace")
                        .count(),
                    1
                );
                let recovered_generation = recovered
                    .health_generation()
                    .expect("read recovered archive generation");

                drop(recovered);
                let reopened = ReputationFinalizedArchive::try_open_with_retention_authority(
                    archive_root.path(),
                    bounds,
                    &fixture.service.chain_id,
                    fixture.kura.as_ref(),
                    &binding,
                    &authority,
                )
                .expect("repeat exact retention recovery");
                assert_eq!(
                    reopened
                        .health_generation()
                        .expect("read replayed archive generation"),
                    recovered_generation,
                    "recovery replay must be deterministic"
                );
                assert_eq!(
                    reopened
                        .retention_floor(&fixture.service.chain_id)
                        .expect("read replayed retention floor"),
                    Some(parent.key)
                );
            }
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
            assert_eq!(
                fixture
                    .kura
                    .v2_finality_artifact(1)
                    .expect("read pre-WSV finality")
                    .expect("finality must be durable before WSV publication")
                    .block_hash,
                fixture.body.hash()
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
            fixture.assert_complete_for_state(restarted_state.as_ref());
        }
    );

    v2_apply_test!(restart_recovers_manifest_after_pre_wsv_finality, {
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
                .is_some()
        );

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        fixture.execute(&mut reopened).expect("complete manifest");
        fixture.assert_complete();
    });

    v2_apply_test!(restart_recovers_kura_block_before_pre_wsv_finality, {
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
                    if *stage == "pre-WSV v2 finality artifact"
            ),
            "unexpected committed recovery classification: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read checkpoint")
                .is_none()
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
        fixture
            .execute(&mut reopened)
            .expect("complete pre-WSV finality and apply");
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
