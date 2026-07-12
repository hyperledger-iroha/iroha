//! Idempotent production application of a durable Sumeragi v2 decision.
//!
//! A CommitQC is written to the safety WAL before this module is invoked. The
//! application transaction then re-loads the exact validated body, advances
//! Kura and WSV at most once, and finally persists the canonical v2 finality
//! sidecar. Restart may observe Kura/WSV already at the decided height while
//! the sidecar is absent; that state is completed without re-applying the
//! block or validating it against a later state.

use std::{num::NonZeroUsize, sync::Arc, time::Duration};

use iroha_config::parameters::actual::SumeragiNpos;
use iroha_crypto::Hash;
use iroha_data_model::{
    ChainId, Encode as _,
    account::AccountId,
    block::{CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire},
    events::EventBox,
};
use iroha_primitives::time::TimeSource;
use thiserror::Error;

use super::{
    network_topology::Topology,
    v2_body_store::{BodyValidationError, V2BodyStore},
    v2_effects::{ApplyTask, DurableApplyCompletion},
};
use crate::{
    EventsSender,
    block::{BlockValidationError, ValidBlock},
    kura::{CommitManifest, Kura},
    queue::{Queue, RoutingDecision},
    state::{MergeLedgerCommitError, MergeLedgerPublicationMode, State},
};

/// Immutable dependencies of the single v2 application service.
pub(crate) struct V2ApplyService {
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    chain_id: ChainId,
    block_cadence: Duration,
    npos_config: SumeragiNpos,
    genesis_account: AccountId,
    events_sender: EventsSender,
    validator_set_pops: Vec<Vec<u8>>,
    #[cfg(test)]
    fail_after_kura_store: std::sync::atomic::AtomicBool,
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
        npos_config: SumeragiNpos,
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
            npos_config,
            genesis_account,
            events_sender,
            validator_set_pops,
            #[cfg(test)]
            fail_after_kura_store: std::sync::atomic::AtomicBool::new(false),
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
        if body.hash() != task.subject().block_hash
            || body.header().height().get() != context.height
            || body.header().prev_block_hash() != task.subject().parent_block_hash
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

        let canonical_body = if state_height < height.get() {
            self.validate_and_apply(
                context,
                body.clone(),
                true,
                task.validated_receipt().execution_commitment(),
            )?
        } else {
            // WSV is already committed, but Kura may have crashed after the
            // block commit marker and before publishing its merge log/carrier
            // association. Reload the exact executed canonical wire rather than
            // the pre-execution proposal body, then retry its association
            // idempotently before publishing any later recovery metadata.
            let canonical_body = self
                .kura
                .get_block_without_merge_sidecar(height)
                .ok_or(V2ApplyError::StateAheadOfKura)?;
            if canonical_body.hash() != task.subject().block_hash {
                return Err(V2ApplyError::KuraConflict);
            }
            self.kura.store_block(Arc::clone(&canonical_body))?;
            canonical_body
        };

        // This is deliberately outside `validate_and_apply`: WSV commit and
        // the merge settlement, Kura checkpoint/manifest, and finality artifact
        // are separate durable systems. A crash after WSV commit must retry
        // these idempotent associations even though executing the block a
        // second time is forbidden.
        self.persist_post_apply_merge_settlement(canonical_body.as_ref())?;
        self.persist_post_apply_metadata(context, task)?;

        let receipt = self.kura.store_v2_finality_artifact(&artifact)?;
        self.kura
            .promote_kagemusha_topup_finality_sidecar(&artifact, &receipt)?;
        self.kura
            .persist_native_amx_participant_application_receipts(canonical_body.as_ref())?;
        Ok(DurableApplyCompletion::new(task.id(), receipt, artifact))
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

    /// Publish the exact merge entry whose effects are already present in WSV.
    ///
    /// The full entry and sparse carrier record are reloaded through Kura's
    /// canonical carrier validator. State then verifies that its atomic block
    /// commit published the exact merge admission and deterministic markers
    /// before the rolling cache, live event, or lane receipts can be created.
    /// This ordering is idempotent across the WSV-before-metadata crash window:
    /// cache publication returns a live event only once, and receipt writes
    /// accept only byte-identical existing evidence.
    fn persist_post_apply_merge_settlement(&self, body: &SignedBlock) -> Result<(), V2ApplyError> {
        let Some(reference) = body
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.as_ref())
        else {
            return Ok(());
        };
        let carrier_height = body.header().height().get();
        let carrier_hash = body.hash();
        let entry = self
            .kura
            .merge_entry_for_carrier(carrier_height, carrier_hash)?
            .ok_or_else(|| {
                V2ApplyError::MergeSettlement(MergeLedgerCommitError::ExecutionStatePublication(
                    format!(
                        "v2 merge carrier {carrier_height} ({carrier_hash}) has no exact durable full entry"
                    ),
                ))
            })?;
        if !reference.matches_entry(&entry) {
            return Err(V2ApplyError::MergeSettlement(
                MergeLedgerCommitError::ExecutionStatePublication(
                    "v2 carrier compact merge reference differs from its durable full entry"
                        .to_owned(),
                ),
            ));
        }
        self.state
            .ensure_globally_committed_merge_entry_applied(&entry)?;
        let (_, merge_event) = self.state.record_globally_committed_merge_entry(
            &entry,
            MergeLedgerPublicationMode::LiveCommit,
        )?;
        if let Some(event) = merge_event {
            let _ = self.events_sender.send(EventBox::Pipeline(event));
        }
        self.kura.persist_merge_lane_block_application_receipts(
            &entry,
            carrier_height,
            carrier_hash,
        )?;
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
        self.validate_lane_payload_plan(context, body)?;
        super::v2_npos::validate_candidate_records(
            context,
            self.state.as_ref(),
            &self.npos_config,
            body.npos_consensus_effects(),
        )?;
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
        let (_valid, mut state_block) = result.map_err(|(failed_block, error)| {
            Self::classify_candidate_validation_error(
                merge_reference,
                failed_block.as_ref(),
                error.as_ref(),
            )
        })?;
        let witness = state_block
            .take_exec_witness()
            .ok_or(V2ApplyError::ExecutionCommitmentUnavailable)?;
        crate::sumeragi::exec::execution_commitment_from_witness(&witness)
            .map_err(|error| V2ApplyError::ExecutionCommitment(error.to_owned()))
    }

    fn validate_and_apply(
        &self,
        context: &wire::HeightContext,
        body: iroha_data_model::block::SignedBlock,
        store_block: bool,
        expected_execution_commitment: wire::ExecutionCommitment,
    ) -> Result<Arc<SignedBlock>, V2ApplyError> {
        self.validate_lane_payload_plan(context, &body)?;
        super::v2_npos::validate_candidate_records(
            context,
            self.state.as_ref(),
            &self.npos_config,
            body.npos_consensus_effects(),
        )?;
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
        let actual_execution_commitment =
            crate::sumeragi::exec::execution_commitment_from_witness(&witness)
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
            .commit_with_certificate()
            .unpack(|event| pipeline_events.push(event))
            .map_err(|(_, error)| V2ApplyError::Commit(error.to_string()))?;
        let canonical_body: Arc<SignedBlock> = committed_block.clone().into();

        // Kura owns the first irreversible commit point. This call is also the
        // idempotent repair boundary for a durable block whose merge
        // association was interrupted after its block fsync.
        if store_block {
            self.kura.store_block(Arc::clone(&canonical_body))?;
            #[cfg(test)]
            if self
                .fail_after_kura_store
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return Err(V2ApplyError::InjectedCrashAfterKuraStore);
            }
        }
        let commit_topology = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect();
        let state_events = state_block.apply_without_execution_with_commit_qc(
            &committed_block,
            commit_topology,
            None,
        );
        state_block
            .commit()
            .map_err(|error| V2ApplyError::StateCommit(error.to_string()))?;

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
        Ok(canonical_body)
    }

    fn persist_post_apply_metadata(
        &self,
        context: &wire::HeightContext,
        task: &ApplyTask,
    ) -> Result<(), V2ApplyError> {
        let block_hash = task.subject().block_hash;
        let checkpoint = crate::snapshot::canonical_state_snapshot_hash(self.state.as_ref());
        let execution_commitment = task.validated_receipt().execution_commitment();
        self.kura
            .store_wsv_checkpoint(context.height, block_hash, checkpoint)?;
        let manifest = CommitManifest::new(
            context.height,
            block_hash,
            Some(execution_commitment.parent_state_root),
            Some(execution_commitment.post_state_root),
            checkpoint,
            Some(Hash::new(task.certificate().encode())),
        );
        self.kura.store_commit_manifest(manifest)?;
        Ok(())
    }

    #[cfg(test)]
    fn fail_after_kura_store_for_test(&self) {
        self.fail_after_kura_store
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
    /// Authenticated NPoS candidate records failed deterministic validation.
    #[error(transparent)]
    Npos(#[from] super::v2_npos::V2NposError),
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
    /// Deterministic validation did not produce the StateBlock execution witness.
    #[error("Sumeragi v2 validation produced no execution witness")]
    ExecutionCommitmentUnavailable,
    /// Execution-witness projection itself was malformed.
    #[error("invalid Sumeragi v2 execution commitment: {0}")]
    ExecutionCommitment(String),
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
    /// WSV transaction could not commit.
    #[error("Sumeragi v2 state commit failed: {0}")]
    StateCommit(String),
    /// The durable merge carrier does not match the merge effects published in WSV.
    #[error(transparent)]
    MergeSettlement(#[from] MergeLedgerCommitError),
    /// Test-only crash boundary after Kura commits and before WSV publication.
    #[cfg(test)]
    #[error("injected crash after Kura store and before WSV commit")]
    InjectedCrashAfterKuraStore,
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
            builder::BlockBuilder as DataModelBlockBuilder,
            consensus::{
                CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1,
                LaneBlockProposalPayloadHintV1, LaneBlockProposalV1, LaneBlockQcV1,
                SumeragiLanePayloadOwnership,
            },
            consensus_v2 as wire,
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        events::pipeline::PipelineEventBox,
        isi::SetParameter,
        merge::{
            MergeExecutionBatch, MergeLaneExecution, MergeLedgerEntry, MergeQuorumCertificate,
        },
        nexus::{DataSpaceId, LaneId},
        parameter::{Parameter, system::SumeragiParameter},
        peer::PeerId,
        transaction::{
            TransactionBuilder,
            error::TransactionRejectionReason,
            signed::{TransactionEntrypoint, TransactionResult, TransactionResultInner},
        },
    };
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        block::BlockBuilder,
        governance::manifest::LaneManifestRegistry,
        query::store::LiveQueryStore,
        queue::execution_context_for_routing_plan,
        state::{World, WorldReadOnly},
        sumeragi::{
            v2_body_store::{BlockSignaturePolicy, V2BodyStore, ValidatedBodyReceipt},
            v2_effects::ApplyTask,
        },
        tx::AcceptedTransaction,
    };

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
                SumeragiNpos::default(),
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
                };
            let body = if include_lane_payload {
                let transaction = TransactionBuilder::new(chain_id.clone(), transaction_authority)
                    .with_instructions([
                        SetParameter::new(Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(
                            100,
                        ))),
                        SetParameter::new(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(100))),
                        SetParameter::new(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(
                            100,
                        ))),
                    ])
                    .sign(transaction_key.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
                let routing_plan = queue
                    .route_plan_with_state(&accepted, state.as_ref())
                    .expect("resolve canonical fixture route");
                let route = routing_plan.coordinator_route();
                let entrypoint_hash = Hash::from(accepted.hash_as_entrypoint());
                let lane_plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
                    state.as_ref(),
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
                let transaction = TransactionBuilder::new(chain_id.clone(), transaction_authority)
                    .with_instructions([
                        SetParameter::new(Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(
                            100,
                        ))),
                        SetParameter::new(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(100))),
                        SetParameter::new(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(
                            100,
                        ))),
                    ])
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
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: Vec::new(),
            };
            let preimage = wire::Vote {
                round,
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
            assert_eq!(self.kura.durable_blocks_count(), 0);
            self.assert_no_post_apply_sidecars();
        }

        fn assert_complete(&self) {
            assert_eq!(self.state.committed_height(), 1);
            assert_eq!(self.kura.durable_blocks_count(), 1);
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
            assert!(
                self.kura
                    .commit_manifest(self.context.height)
                    .expect("read manifest")
                    .is_some()
            );
            let artifact = self
                .kura
                .v2_finality_artifact(self.context.height)
                .expect("read finality")
                .expect("finality exists");
            assert_eq!(artifact.height_context, self.context);
            assert_eq!(artifact.subject, self.manifest.subject);
            assert_eq!(artifact.commit_qc, self.task.certificate().clone());
            assert!(
                self.state
                    .world_view()
                    .commit_qcs()
                    .get(&self.body.hash())
                    .is_none(),
                "Sumeragi v2 finality must not be projected into the legacy commit-QC store"
            );
        }
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
            epoch_id: context.epoch,
            lane_catalog_hash: Hash::new(b"v2 apply decided-sidecar catalog"),
            active_lanes: Vec::new(),
            incarnation_root: Hash::new(b"v2 apply decided-sidecar incarnations"),
            activation_root: Hash::new(b"v2 apply decided-sidecar activations"),
            lane_snapshots: Vec::new(),
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
            lane_drain_certificates: Vec::new(),
        }
    }

    struct MergeSettlementFixture {
        base: ApplyFixture,
        carrier: SignedBlock,
        entry: MergeLedgerEntry,
        proposal: LaneBlockProposalV1,
    }

    impl MergeSettlementFixture {
        fn new(seed_applied_marker: bool) -> Self {
            let base = ApplyFixture::new();
            let mut store = base.reopen_body_store();
            base.execute(&mut store)
                .expect("commit parent before merge-settlement fixture");

            let parent_hash = base.body.hash();
            let carrier_header = BlockHeader::new(
                NonZeroU64::new(2).expect("non-zero carrier height"),
                Some(parent_hash),
                None,
                None,
                2,
                0,
            );
            let transaction_key = KeyPair::try_from_seed(vec![0xB4; 32], Algorithm::Ed25519)
                .expect("merge execution transaction key");
            let transaction = TransactionBuilder::new(
                base.context.chain_id.clone(),
                AccountId::new(transaction_key.public_key().clone()),
            )
            .sign(transaction_key.private_key());
            let entrypoint = TransactionEntrypoint::External(transaction);
            let entrypoint_hash = Hash::from(entrypoint.hash());
            let result = TransactionResult::from(TransactionResultInner::Ok(Default::default()));
            let result_hash = Hash::from(result.hash());
            let validator_set = base
                .context
                .roster
                .iter()
                .map(|validator| validator.validator.clone())
                .collect::<Vec<_>>();
            let validator_count =
                u32::try_from(validator_set.len()).expect("fixture validator count");
            let min_quorum = u32::try_from(
                crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()),
            )
            .expect("fixture lane quorum");
            let lane_incarnation = Hash::new(b"v2 apply merge execution lane incarnation");
            let mut ownership = SumeragiLanePayloadOwnership {
                proposal_height: 1,
                proposal_view: 0,
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
                lane_incarnation,
                lane_block_height: 1,
                lane_block_view: 0,
                subject_hash: Hash::prehashed([0; Hash::LENGTH]),
                qc_mode_tag: "permissioned:v2-apply-merge-settlement".to_owned(),
                accepted_candidate_indices: vec![0],
                accepted_transaction_hashes: vec![entrypoint_hash],
                previous_lane_block_height: 0,
                previous_lane_block_descriptor_hash: None,
                lane_block_descriptor_hash: Some(Hash::prehashed([0; Hash::LENGTH])),
                lane_block_descriptor_validator_set: validator_set.clone(),
                lane_block_descriptor_validator_count: validator_count,
                lane_block_descriptor_min_quorum: min_quorum,
                payload_ownership_hash: Hash::prehashed([0; Hash::LENGTH]),
                rbc_instance_hash: Hash::prehashed([0; Hash::LENGTH]),
            };
            let replay = ownership
                .compute_replay_hashes()
                .expect("merge receipt ownership replay material");
            ownership.subject_hash = replay.subject_hash;
            ownership.payload_ownership_hash = replay.payload_ownership_hash;
            ownership.rbc_instance_hash = replay.rbc_instance_hash;
            ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
            let descriptor = LaneBlockDescriptorV1 {
                lane_id: ownership.lane_id,
                dataspace_id: ownership.dataspace_id,
                lane_incarnation: ownership.lane_incarnation,
                proposal_height: ownership.proposal_height,
                previous_lane_block_height: ownership.previous_lane_block_height,
                previous_lane_block_descriptor_hash: ownership.previous_lane_block_descriptor_hash,
                lane_block_height: ownership.lane_block_height,
                lane_block_view: ownership.lane_block_view,
                subject_hash: ownership.subject_hash,
                payload_ownership_hash: ownership.payload_ownership_hash,
                rbc_instance_hash: ownership.rbc_instance_hash,
                accepted_candidate_indices: ownership.accepted_candidate_indices.clone(),
                accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&ownership.lane_block_descriptor_validator_set),
                validator_set: ownership.lane_block_descriptor_validator_set.clone(),
                validator_count: ownership.lane_block_descriptor_validator_count,
                min_quorum: ownership.lane_block_descriptor_min_quorum,
                qc_mode_tag: ownership.qc_mode_tag.clone(),
                descriptor_hash: ownership
                    .lane_block_descriptor_hash
                    .expect("computed ownership descriptor hash"),
            };
            assert_eq!(
                descriptor.computed_descriptor_hash(),
                descriptor.descriptor_hash
            );
            let mut proposal = LaneBlockProposalV1 {
                descriptor,
                proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
                payload_block_hint: Some(LaneBlockProposalPayloadHintV1 {
                    proposal_height: ownership.proposal_height,
                    proposal_view: ownership.proposal_view,
                    proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"v2 apply autonomous proposal anchor",
                    )),
                }),
            };
            proposal.proposal_hash = proposal.computed_proposal_hash();
            let qc = |phase| LaneBlockQcV1 {
                body: proposal.vote_body(phase),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set: validator_set.clone(),
                signers_bitmap: vec![0x0F],
                bls_aggregate_signature: vec![0x5A; 96],
                payload_availability_qc: None,
            };
            let settlement_commitment = LaneBlockCommitment {
                block_height: 1,
                lane_id: LaneId::SINGLE,
                lane_incarnation,
                dataspace_id: DataSpaceId::UNIVERSAL,
                tx_count: 1,
                total_local_micro: 0,
                total_xor_due_micro: 0,
                total_xor_after_haircut_micro: 0,
                total_xor_variance_micro: 0,
                swap_metadata: None,
                receipts: Vec::new(),
                nexus_fee_receipts: Vec::new(),
                native_amx_receipts: Vec::new(),
            };
            let settlement_hash =
                iroha_data_model::nexus::compute_settlement_hash(&settlement_commitment)
                    .expect("merge receipt settlement hash");
            let source_bundle = vec![0x51];
            let execution = MergeLaneExecution {
                source_bundle_hash: Hash::new(&source_bundle),
                source_bundle,
                proposal: proposal.clone(),
                origin_proposal: proposal.clone(),
                prepare_qc: qc(CertPhase::Prepare),
                commit_qc: qc(CertPhase::Commit),
                signer_proofs: Vec::new(),
                autonomous_chain_id_hash: Hash::new(
                    base.context.chain_id.clone().into_inner().as_bytes(),
                ),
                autonomous_epoch: 0,
                autonomous_payload_hash: Hash::new(b"v2 apply autonomous payload"),
                entrypoint_hashes: vec![entrypoint_hash],
                entrypoints: vec![entrypoint],
                reservation_keys: vec![vec![0x61]],
                routing_plans: vec![vec![0x62]],
                native_amx_receipts: vec![None],
                result_hashes: vec![result_hash],
                results: vec![result],
                settlement_commitment,
                settlement_hash,
            };
            let lanes = vec![execution];
            let base_state_hash = base.state.lane_execution_state_hash();
            let mut batch = MergeExecutionBatch {
                version: 1,
                base_state_height: 1,
                base_state_hash,
                application_block_header: carrier_header.clone(),
                entrypoint_count: 1,
                entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(
                    &lanes,
                )
                .expect("merge entrypoint root"),
                result_merkle_root: crate::merge::merge_execution_result_merkle_root(&lanes)
                    .expect("merge result root"),
                execution_root: crate::merge::merge_execution_root(&lanes),
                lanes,
                application_write_set_root: Hash::new(b"v2 apply application write set"),
                write_set_root: Hash::new(b"v2 apply complete write set"),
                expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"v2 apply expected post state",
                )),
                batch_hash: Hash::prehashed([0; Hash::LENGTH]),
            };
            batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
            let validator_set_hash = HashOf::new(&validator_set);
            let entry_template = crate::merge::MergeLedgerCandidate {
                epoch_id: 1,
                view: 0,
                carrier_height: 2,
                carrier_parent_hash: parent_hash,
                lane_catalog_hash: Hash::new(b"v2 apply merge catalog"),
                active_lanes: Vec::new(),
                incarnation_root: Hash::new(b"v2 apply merge incarnations"),
                activation_root: Hash::new(b"v2 apply merge activations"),
                lane_snapshots: Vec::new(),
                execution_batch: Some(batch),
                lane_drain_certificates: Vec::new(),
                global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
            };
            let message_digest = crate::merge::merge_qc_message_digest(
                &base.context.chain_id,
                &entry_template,
                VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash,
            );
            let merge_qc = MergeQuorumCertificate::new(
                0,
                1,
                2,
                parent_hash,
                crate::merge::merge_chain_id_digest(&base.context.chain_id),
                VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash,
                validator_set,
                vec![0x0F],
                Vec::new(),
                vec![0x7A; 96],
                message_digest,
            );
            let entry = entry_template.into_entry(merge_qc);
            let mut builder = DataModelBlockBuilder::new(carrier_header);
            builder.set_execution_context(Some(
                BlockExecutionContextBundle::new(Vec::new())
                    .with_merge_entry(CertifiedMergeLedgerReference::new(&entry)),
            ));
            let mut carrier_keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic carrier key")
                })
                .collect::<Vec<_>>();
            carrier_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let mut successor_context = base.context.clone();
            successor_context.height = 2;
            successor_context.parent_commit_qc = Some(base.task.certificate().clone());
            let leader =
                usize::try_from(successor_context.leader(0)).expect("carrier leader index");
            let carrier = builder.build_with_signature(
                u64::try_from(leader).expect("carrier leader index fits u64"),
                carrier_keys[leader].private_key(),
            );
            base.kura
                .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
                .expect("persist exact merge carrier");

            let mut state_block = base.state.block(carrier.header().clone());
            state_block.block_hashes.push(carrier.hash());
            let block_height = carrier
                .header()
                .height()
                .try_into()
                .expect("carrier height fits transaction index");
            state_block
                .transactions
                .insert_block(std::collections::HashSet::new(), block_height);
            state_block
                .commit()
                .expect("commit carrier identity before auxiliary settlement");
            if seed_applied_marker {
                base.state
                    .seed_applied_merge_entry_for_v2_settlement_test(&entry)
                    .expect("seed exact post-commit WSV merge markers");
            }
            Self {
                base,
                carrier,
                entry,
                proposal,
            }
        }

        fn execute_recovery(&self) -> Result<(), V2ApplyError> {
            let mut context = self.base.context.clone();
            context.height = 2;
            context.parent_commit_qc = Some(self.base.task.certificate().clone());
            context.validate().expect("valid successor context");
            let canonical_wire = self
                .carrier
                .encode_wire()
                .expect("canonical recovery carrier wire");
            let subject = wire::BlockSubject {
                parent_block_hash: Some(self.base.body.hash()),
                block_hash: self.carrier.hash(),
                payload_hash: Hash::new(&canonical_wire),
            };
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            };
            let manifest = wire::PayloadManifest::derive(
                &context,
                round,
                subject,
                u64::try_from(canonical_wire.len()).expect("carrier wire length"),
                std::slice::from_ref(&canonical_wire),
            )
            .expect("recovery carrier manifest");
            let body_root = tempfile::tempdir().expect("recovery body store");
            let mut body_store = V2BodyStore::open_with_policy(
                body_root.path(),
                context.clone(),
                BlockSignaturePolicy::RotatingLeader,
            )
            .expect("open recovery body store");
            let durable = body_store
                .store(manifest, canonical_wire)
                .expect("store recovery carrier body");
            let execution_commitment = wire::ExecutionCommitment::without_topups(
                Hash::new(b"v2 recovery parent state"),
                Hash::new(b"v2 recovery post state"),
                Hash::new(b"v2 recovery ordinary writes"),
            );
            let validated = body_store
                .validate(&durable, |_| {
                    Ok::<wire::ExecutionCommitment, V2ApplyError>(execution_commitment)
                })
                .expect("mark already-applied recovery carrier validated");
            let mut certificate = wire::QuorumCertificate {
                round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: Vec::new(),
            };
            let preimage = wire::Vote {
                round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment,
                signer: 0,
                signature: Vec::new(),
            }
            .signature_preimage();
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic recovery key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let signatures = certificate
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        keys[usize::try_from(*index).expect("recovery signer index")].private_key(),
                        &preimage,
                    )
                    .expect("sign recovery Commit vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate recovery Commit votes");
            let task = ApplyTask::for_test(
                2,
                EventTag::new(2, 0, Generation::new(1)),
                subject,
                certificate,
                validated,
            );
            self.base
                .service
                .execute(&context, &mut body_store, &task)
                .map(drop)
        }
    }

    #[test]
    fn post_apply_merge_settlement_publishes_cache_event_and_receipt_once() {
        let fixture = MergeSettlementFixture::new(true);
        let mut events = fixture.base.service.events_sender.subscribe();

        fixture
            .base
            .service
            .persist_post_apply_merge_settlement(&fixture.carrier)
            .expect("publish exact post-apply merge settlement");
        assert_eq!(
            fixture.base.state.merge_ledger().latest().as_deref(),
            Some(&fixture.entry)
        );
        let receipt = fixture
            .base
            .kura
            .read_lane_block_application_receipt(
                fixture.proposal.descriptor.lane_id,
                fixture.proposal.descriptor.lane_block_height,
            )
            .expect("merge lane application receipt");
        assert_eq!(receipt.proposal, fixture.proposal);
        let event = events.try_recv().expect("one live merge event");
        assert!(matches!(
            event,
            EventBox::Pipeline(PipelineEventBox::Merge(event)) if event.entry == fixture.entry
        ));

        fixture
            .base
            .service
            .persist_post_apply_merge_settlement(&fixture.carrier)
            .expect("idempotent post-apply merge settlement retry");
        assert_eq!(fixture.base.state.merge_ledger().len(), 1);
        assert_eq!(
            fixture.base.kura.read_lane_block_application_receipt(
                fixture.proposal.descriptor.lane_id,
                fixture.proposal.descriptor.lane_block_height,
            ),
            Some(receipt)
        );
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn wsv_applied_v2_retry_repairs_merge_settlement_before_finality() {
        let fixture = MergeSettlementFixture::new(true);
        let mut events = fixture.base.service.events_sender.subscribe();

        fixture
            .execute_recovery()
            .expect("already-applied V2 decision repairs merge metadata");
        assert_eq!(fixture.base.state.merge_ledger().len(), 1);
        assert!(
            fixture
                .base
                .kura
                .lane_block_application_receipt_available(&fixture.proposal)
        );
        assert!(
            fixture
                .base
                .kura
                .wsv_checkpoint(2)
                .expect("read recovery checkpoint")
                .is_some()
        );
        assert!(
            fixture
                .base
                .kura
                .commit_manifest(2)
                .expect("read recovery manifest")
                .is_some()
        );
        assert!(
            fixture
                .base
                .kura
                .v2_finality_artifact(2)
                .expect("read recovery finality")
                .is_some()
        );
        assert!(matches!(
            events.try_recv(),
            Ok(EventBox::Pipeline(PipelineEventBox::Merge(event))) if event.entry == fixture.entry
        ));

        fixture
            .execute_recovery()
            .expect("already-complete V2 decision retry remains idempotent");
        assert_eq!(fixture.base.state.merge_ledger().len(), 1);
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn post_apply_merge_settlement_retry_repairs_receipt_without_duplicate_event() {
        let fixture = MergeSettlementFixture::new(true);
        let mut events = fixture.base.service.events_sender.subscribe();
        fixture
            .base
            .kura
            .fail_next_lane_block_application_receipt_write_for_tests();

        assert!(matches!(
            fixture
                .base
                .service
                .persist_post_apply_merge_settlement(&fixture.carrier),
            Err(V2ApplyError::Kura(_))
        ));
        assert_eq!(fixture.base.state.merge_ledger().len(), 1);
        assert!(
            fixture
                .base
                .kura
                .read_lane_block_application_receipt(
                    fixture.proposal.descriptor.lane_id,
                    fixture.proposal.descriptor.lane_block_height,
                )
                .is_none()
        );
        assert!(matches!(
            events.try_recv(),
            Ok(EventBox::Pipeline(PipelineEventBox::Merge(event))) if event.entry == fixture.entry
        ));

        fixture
            .base
            .service
            .persist_post_apply_merge_settlement(&fixture.carrier)
            .expect("repair receipt after cache publication crash boundary");
        assert!(
            fixture
                .base
                .kura
                .lane_block_application_receipt_available(&fixture.proposal)
        );
        assert_eq!(fixture.base.state.merge_ledger().len(), 1);
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn post_apply_merge_settlement_rejects_missing_wsv_marker() {
        let fixture = MergeSettlementFixture::new(false);
        let mut events = fixture.base.service.events_sender.subscribe();

        assert!(matches!(
            fixture
                .base
                .service
                .persist_post_apply_merge_settlement(&fixture.carrier),
            Err(V2ApplyError::MergeSettlement(
                MergeLedgerCommitError::ExecutionStatePublication(_)
            ))
        ));
        assert!(fixture.base.state.merge_ledger().is_empty());
        assert!(
            fixture
                .base
                .kura
                .read_lane_block_application_receipt(
                    fixture.proposal.descriptor.lane_id,
                    fixture.proposal.descriptor.lane_block_height,
                )
                .is_none()
        );
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn post_apply_merge_settlement_rejects_reference_drift() {
        let fixture = MergeSettlementFixture::new(true);
        let mut tampered_entry = fixture.entry.clone();
        tampered_entry.global_state_root = Hash::new(b"tampered v2 merge reference");
        let mut tampered = fixture.carrier.clone();
        tampered.set_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_merge_entry(CertifiedMergeLedgerReference::new(&tampered_entry)),
        ));

        assert!(matches!(
            fixture
                .base
                .service
                .persist_post_apply_merge_settlement(&tampered),
            Err(V2ApplyError::MergeSettlement(
                MergeLedgerCommitError::ExecutionStatePublication(_)
            ))
        ));
        assert!(fixture.base.state.merge_ledger().is_empty());
        assert!(
            !fixture
                .base
                .kura
                .lane_block_application_receipt_available(&fixture.proposal)
        );
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
        assert_eq!(fixture.kura.durable_blocks_count(), 0);
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
        resigned_commit_qc_with_wrong_header_view_is_rejected_without_mutation,
        {
            let fixture = ApplyFixture::new();
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
                    .expect("sign wrong-view Commit vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate wrong-view Commit votes");
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
                Err(V2ApplyError::Finality(
                    wire::finality::V2FinalityValidationError::AssociatedViewMismatch {
                        certificate: 1,
                        block: 0,
                    }
                ))
            ));
            fixture.assert_no_apply_mutation();
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
        assert_eq!(fixture.kura.durable_blocks_count(), 0);
        fixture.assert_no_post_apply_sidecars();

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        fixture
            .execute(&mut reopened)
            .expect("retry exact apply after reopening the durable body store");
        fixture.assert_complete();
        let view = fixture.state.view();
        let sumeragi = view.world().parameters().sumeragi();
        assert_eq!(sumeragi.min_finality_ms(), 100);
        assert_eq!(sumeragi.block_time_ms(), 100);
        assert_eq!(sumeragi.commit_time_ms(), 100);
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
        assert_eq!(fixture.kura.durable_blocks_count(), 1);
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

    v2_apply_test!(completed_apply_retry_reuses_exact_executed_kura_wire, {
        let fixture = ApplyFixture::new();
        let proposal_wire = fixture
            .body
            .encode_wire()
            .expect("encode pre-execution proposal body");
        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("complete initial V2 application");
        let height = NonZeroUsize::new(1).expect("height");
        let durable_wire = fixture
            .kura
            .get_block(height)
            .expect("read executed canonical Kura block")
            .encode_wire()
            .expect("encode executed canonical Kura block");
        assert_ne!(
            proposal_wire, durable_wire,
            "the recovery regression requires execution results to change the canonical wire"
        );

        fixture
            .execute(&mut store)
            .expect("idempotent completed-apply retry");
        assert_eq!(
            fixture
                .kura
                .get_block(height)
                .expect("read retried canonical Kura block")
                .encode_wire()
                .expect("encode retried canonical Kura block"),
            durable_wire,
            "recovery must re-store the exact executed Kura wire, not the proposal body"
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
        assert_eq!(fixture.kura.durable_blocks_count(), 1);
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
        fixture
            .service
            .validate_and_apply(
                &fixture.context,
                fixture.body.clone(),
                false,
                fixture.task.validated_receipt().execution_commitment(),
            )
            .expect("model corrupted WSV-ahead crash image");
        assert_eq!(fixture.state.committed_height(), 1);
        assert_eq!(fixture.kura.durable_blocks_count(), 0);
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
            assert_eq!(fixture.kura.durable_blocks_count(), 0);
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
        restart_recovers_wsv_before_checkpoint_manifest_and_finality,
        {
            let fixture = ApplyFixture::new();
            let mut store = fixture.reopen_body_store();
            fixture.kura.fail_next_wsv_checkpoint_write_for_tests();
            assert!(matches!(
                fixture.execute(&mut store),
                Err(V2ApplyError::Kura(_))
            ));
            assert_eq!(fixture.state.committed_height(), 1);
            assert_eq!(fixture.kura.durable_blocks_count(), 1);
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
                .expect("complete metadata without reapplying WSV");
            fixture.assert_complete();
        }
    );

    v2_apply_test!(restart_recovers_checkpoint_before_manifest_and_finality, {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_commit_manifest_write_for_tests();
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::Kura(_))
        ));
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
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::Kura(_))
        ));
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
