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
    stake_snapshot::strict_v2_voting_roster,
    v2_body_store::{BodyValidationError, V2BodyStore},
    v2_effects::{ApplyTask, DurableApplyCompletion},
};
use crate::{
    EventsSender,
    block::{BlockValidationError, ValidBlock},
    kura::{CommitManifest, Kura},
    queue::{Queue, RoutingDecision},
    state::{MergeLedgerCommitError, MergeLedgerPublicationMode, StakeSnapshot, State},
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
}

impl V2ApplyService {
    fn classify_candidate_validation_error(
        merge_reference: Option<&CertifiedMergeLedgerReference>,
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
        V2ApplyError::Validation(error.to_string())
    }

    fn validate_lane_payload_plan(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
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
        let expected = super::main_loop::lane_scheduler::prepare_v2_lane_payload_plan(
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
        let body = body_store.load(task.validated_receipt().durable())?;
        if body.hash() != task.subject().block_hash
            || body.header().height().get() != context.height
            || body.header().prev_block_hash() != task.subject().parent_block_hash
        {
            return Err(V2ApplyError::TaskMismatch);
        }

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

        if state_height < height.get() {
            self.validate_and_apply(context, body.clone(), durable_hash.is_none())?;
        }

        // This is deliberately outside `validate_and_apply`: WSV commit and
        // the merge settlement, Kura checkpoint/manifest, and finality artifact
        // are separate durable systems. A crash after WSV commit must retry
        // these idempotent associations even though executing the block a
        // second time is forbidden.
        self.persist_post_apply_merge_settlement(&body)?;
        self.persist_post_apply_metadata(context, task)?;

        let next_epoch_snapshot = self.finalized_next_epoch_snapshot(context)?;
        let artifact = wire::finality::V2FinalityArtifact::new(
            context.clone(),
            task.subject(),
            task.certificate().clone(),
            next_epoch_snapshot,
        );
        artifact.validate()?;
        let receipt = self.kura.store_v2_finality_artifact(&artifact)?;
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
    ) -> Result<(), V2ApplyError> {
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
        let (_valid, state_block) = result.map_err(|(_, error)| {
            Self::classify_candidate_validation_error(merge_reference, error.as_ref())
        })?;
        drop(state_block);
        Ok(())
    }

    fn validate_and_apply(
        &self,
        context: &wire::HeightContext,
        body: iroha_data_model::block::SignedBlock,
        store_block: bool,
    ) -> Result<(), V2ApplyError> {
        self.validate_lane_payload_plan(context, &body)?;
        super::v2_npos::validate_candidate_records(
            context,
            self.state.as_ref(),
            &self.npos_config,
            body.npos_consensus_effects(),
        )?;
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
            .map_err(|(_, error)| {
                Self::classify_candidate_validation_error(merge_reference.as_ref(), error.as_ref())
            })?;
        let committed_block = valid_block
            .commit_with_certificate()
            .unpack(|event| pipeline_events.push(event))
            .map_err(|(_, error)| V2ApplyError::Commit(error.to_string()))?;

        if store_block {
            self.kura.store_block(committed_block.clone())?;
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
        Ok(())
    }

    fn persist_post_apply_metadata(
        &self,
        context: &wire::HeightContext,
        task: &ApplyTask,
    ) -> Result<(), V2ApplyError> {
        let block_hash = task.subject().block_hash;
        let checkpoint = crate::snapshot::canonical_state_snapshot_hash(self.state.as_ref());
        self.kura
            .store_wsv_checkpoint(context.height, block_hash, checkpoint)?;
        let manifest = CommitManifest::new(
            context.height,
            block_hash,
            None,
            None,
            checkpoint,
            Some(Hash::new(task.certificate().encode())),
        );
        self.kura.store_commit_manifest(manifest)?;
        Ok(())
    }

    fn finalized_next_epoch_snapshot(
        &self,
        context: &wire::HeightContext,
    ) -> Result<Option<wire::finality::FinalizedNextEpochSnapshot>, V2ApplyError> {
        if context.height != context.epoch_end_height {
            return Ok(None);
        }
        let epoch = context
            .epoch
            .checked_add(1)
            .ok_or(V2ApplyError::EpochOverflow)?;
        let view = self.state.view();
        let roster = match context.mode {
            wire::ConsensusMode::Permissioned => context.roster.clone(),
            wire::ConsensusMode::Npos => {
                let elected = StakeSnapshot::epoch_validator_peer_ids(&view, epoch)
                    .ok_or(V2ApplyError::MissingFinalizedEpochRoster)?;
                let nexus = self.state.nexus_snapshot();
                let active_lanes = nexus
                    .enabled
                    .then(|| crate::state::nexus_active_lane_ids(&nexus));
                strict_v2_voting_roster(view.world(), &elected, active_lanes.as_ref())?
            }
        };
        let quorum = wire::DualQuorum::from_roster(&roster)?;
        let leader_seed = match context.mode {
            wire::ConsensusMode::Permissioned => {
                let mut preimage = b"sumeragi-v2:permissioned-next-epoch".to_vec();
                preimage.extend_from_slice(&context.leader_seed);
                preimage.extend_from_slice(&context.height.to_le_bytes());
                Hash::new(preimage).into()
            }
            wire::ConsensusMode::Npos => super::npos_seed_for_height(
                &view,
                context
                    .height
                    .checked_add(1)
                    .ok_or(V2ApplyError::HeightOverflow)?,
            ),
        };
        Ok(Some(wire::finality::FinalizedNextEpochSnapshot {
            epoch,
            mode: context.mode,
            roster,
            quorum,
            leader_seed,
        }))
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
    /// Exact-body loading or marker verification failed.
    #[error(transparent)]
    Body(#[from] super::v2_body_store::V2BodyStoreError),
    /// Kura persistence or canonical association failed.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// NPoS stake snapshot could not be frozen exactly.
    #[error(transparent)]
    Stake(#[from] super::stake_snapshot::StrictV2StakeSnapshotError),
    /// Authenticated NPoS VRF record validation failed.
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
    /// Epoch arithmetic overflowed.
    #[error("Sumeragi v2 epoch number overflowed")]
    EpochOverflow,
    /// NPoS boundary lacks a finalized election roster.
    #[error("Sumeragi v2 NPoS epoch boundary lacks a finalized next-epoch roster")]
    MissingFinalizedEpochRoster,
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
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        Registrable,
        account::Account,
        block::{
            BlockExecutionContextBundle, BlockHeader, BlockSignature, SignedBlock,
            consensus::{
                CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1,
                LaneBlockProposalPayloadHintV1, LaneBlockProposalV1, LaneBlockQcV1,
                SumeragiLanePayloadOwnership,
            },
            consensus_v2 as wire,
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        events::pipeline::PipelineEventBox,
        isi::Log,
        merge::{
            MergeExecutionBatch, MergeLaneExecution, MergeLedgerEntry, MergeQuorumCertificate,
        },
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::{
            TransactionBuilder,
            signed::{TransactionEntrypoint, TransactionResult, TransactionResultInner},
        },
    };
    use iroha_logger::Level;

    use super::*;
    use crate::{
        block::BlockBuilder,
        query::store::LiveQueryStore,
        queue::execution_context_for_routing_plan,
        state::World,
        sumeragi::{
            v2_body_store::{BlockSignaturePolicy, V2BodyStore},
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
            );

            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            };
            let leader_index = context.leader(0);
            let leader = &keys[usize::try_from(leader_index).expect("leader index")];
            let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
                &state.nexus_snapshot(),
                context.height,
            );
            let body = if include_lane_payload {
                let transaction = TransactionBuilder::new(chain_id.clone(), transaction_authority)
                    .with_instructions([Log::new(
                        Level::INFO,
                        "v2 lane apply recovery fixture".to_owned(),
                    )])
                    .sign(transaction_key.private_key());
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
                let routing_plan = queue
                    .route_plan_with_state(&accepted, state.as_ref())
                    .expect("resolve canonical fixture route");
                let route = routing_plan.coordinator_route();
                let entrypoint_hash = Hash::from(accepted.hash_as_entrypoint());
                let lane_plan =
                    super::super::main_loop::lane_scheduler::prepare_v2_lane_payload_plan(
                        state.as_ref(),
                        &context,
                        0,
                        &context.roster[usize::try_from(leader_index).expect("leader index")]
                            .validator,
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
                let block =
                    BlockBuilder::new_with_time_source(vec![accepted], TimeSource::new_system())
                        .chain(0, None)
                        .with_da_proof_policies(Some(proof_policy_bundle.clone()))
                        .with_execution_context(Some(execution_context))
                        .try_sign_with_index(leader.private_key(), u64::from(leader_index))
                        .expect("sign fixture lane body")
                        .unpack(|_| {});
                SignedBlock::from(block)
            } else {
                let block =
                    BlockBuilder::new_with_time_source(Vec::new(), TimeSource::new_system())
                        .chain(0, None)
                        .with_da_proof_policies(Some(proof_policy_bundle))
                        .try_sign_with_index(leader.private_key(), u64::from(leader_index))
                        .expect("sign fixture block")
                        .unpack(|_| {});
                SignedBlock::from(block)
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
            let certificate = wire::QuorumCertificate {
                round,
                phase: wire::GlobalPhase::Commit,
                subject,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA7; 48],
            };

            let body_root = tempfile::tempdir().expect("body-store directory");
            let mut body_store = V2BodyStore::open_with_policy(
                body_root.path(),
                context.clone(),
                BlockSignaturePolicy::RotatingLeader,
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
            }
        }

        fn reopen_body_store(&self) -> V2BodyStore {
            V2BodyStore::open_with_policy(
                self.body_root.path(),
                self.context.clone(),
                BlockSignaturePolicy::RotatingLeader,
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

        fn assert_complete(&self) {
            assert_eq!(self.state.committed_height(), 1);
            assert_eq!(self.kura.durable_blocks_count(), 1);
            assert_eq!(
                self.kura
                    .get_durable_block_hash(NonZeroUsize::new(1).expect("height")),
                Some(self.body.hash())
            );
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
                context.height,
                context.epoch,
                view,
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
            let mut builder = BlockBuilder::new(carrier_header);
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
            let validated = body_store
                .validate(&durable, |_| Ok::<(), V2ApplyError>(()))
                .expect("mark already-applied recovery carrier validated");
            let task = ApplyTask::for_test(
                2,
                EventTag::new(2, 0, Generation::new(1)),
                subject,
                wire::QuorumCertificate {
                    round,
                    phase: wire::GlobalPhase::Commit,
                    subject,
                    signers: vec![0, 1, 2],
                    aggregate_signature: vec![0xA8; 48],
                },
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

    #[test]
    fn durable_decision_retains_exact_earlier_view_sidecar_and_prunes_losers() {
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

    #[test]
    fn block_write_failure_never_advances_wsv_and_retry_is_exact() {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_block_write_for_tests();
        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::Kura(_))
        ));
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(fixture.kura.durable_blocks_count(), 0);
        fixture.assert_no_post_apply_sidecars();

        fixture.execute(&mut store).expect("retry exact apply");
        fixture.assert_complete();
    }

    #[test]
    fn restart_recovers_kura_block_written_before_wsv_commit() {
        let fixture = ApplyFixture::new();
        fixture
            .kura
            .store_block(fixture.body.clone())
            .expect("model durable Kura block before crash");
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(fixture.kura.durable_blocks_count(), 1);
        fixture.assert_no_post_apply_sidecars();

        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("resume WSV application from exact durable body");
        fixture.assert_complete();
    }

    #[test]
    fn restart_recovers_kura_lane_body_written_before_wsv_commit() {
        let fixture = ApplyFixture::new_with_lane_payload(true);
        let ownerships = fixture
            .body
            .execution_context()
            .expect("lane body execution context")
            .lane_payload_ownerships
            .clone();
        assert_eq!(ownerships.len(), 1, "fixture must carry lane ownership");
        fixture
            .kura
            .store_block(fixture.body.clone())
            .expect("model durable Kura lane body before crash");
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(fixture.kura.durable_blocks_count(), 1);
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
    }

    #[test]
    fn conflicting_canonical_kura_block_fails_before_wsv_mutation() {
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
        let signature = SignatureOf::try_from_hash(conflicting_key.private_key(), header.hash())
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

    #[test]
    fn wsv_without_its_canonical_kura_block_fails_closed() {
        let fixture = ApplyFixture::new();
        fixture
            .service
            .validate_and_apply(&fixture.context, fixture.body.clone(), false)
            .expect("model corrupted WSV-ahead crash image");
        assert_eq!(fixture.state.committed_height(), 1);
        assert_eq!(fixture.kura.durable_blocks_count(), 0);
        let mut store = fixture.reopen_body_store();

        assert!(matches!(
            fixture.execute(&mut store),
            Err(V2ApplyError::StateAheadOfKura)
        ));
        fixture.assert_no_post_apply_sidecars();
    }

    #[test]
    fn restart_recovers_wsv_before_checkpoint_manifest_and_finality() {
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

    #[test]
    fn restart_recovers_checkpoint_before_manifest_and_finality() {
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
    }

    #[test]
    fn restart_recovers_metadata_written_before_finality() {
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
    }

    #[test]
    fn complete_apply_replay_is_idempotent_and_never_advances_twice() {
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
}
