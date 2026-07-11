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
    block::{SignedBlock, consensus_v2 as wire},
    events::EventBox,
};
use iroha_primitives::time::TimeSource;
use thiserror::Error;

use super::{
    network_topology::Topology,
    stake_snapshot::strict_v2_voting_roster,
    v2_body_store::V2BodyStore,
    v2_effects::{ApplyTask, DurableApplyCompletion},
};
use crate::{
    EventsSender,
    block::ValidBlock,
    kura::{CommitManifest, Kura},
    queue::Queue,
    state::{StakeSnapshot, State},
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
            self.validate_and_apply(context, body, durable_hash.is_none())?;
        } else if durable_hash.is_none() {
            // WSV cannot be ahead of its canonical block log. Continuing here
            // would manufacture a sidecar for state that Kura cannot identify.
            return Err(V2ApplyError::StateAheadOfKura);
        }

        // This is deliberately outside `validate_and_apply`: WSV commit and
        // the Kura checkpoint/manifest are separate durable systems. A crash
        // after WSV commit must retry these idempotent associations even
        // though executing the block a second time is forbidden.
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
        super::v2_npos::validate_candidate_records(
            context,
            self.state.as_ref(),
            &self.npos_config,
            body.npos_consensus_effects(),
        )?;
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
        let (_valid, state_block) =
            result.map_err(|(_, error)| V2ApplyError::Validation(error.to_string()))?;
        drop(state_block);
        Ok(())
    }

    fn validate_and_apply(
        &self,
        context: &wire::HeightContext,
        body: iroha_data_model::block::SignedBlock,
        store_block: bool,
    ) -> Result<(), V2ApplyError> {
        super::v2_npos::validate_candidate_records(
            context,
            self.state.as_ref(),
            &self.npos_config,
            body.npos_consensus_effects(),
        )?;
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
            .map_err(|(_, error)| V2ApplyError::Validation(error.to_string()))?;
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
    /// Certificate-aware block commit conversion failed.
    #[error("Sumeragi v2 block commit conversion failed: {0}")]
    Commit(String),
    /// WSV transaction could not commit.
    #[error("Sumeragi v2 state commit failed: {0}")]
    StateCommit(String),
    /// Epoch arithmetic overflowed.
    #[error("Sumeragi v2 epoch number overflowed")]
    EpochOverflow,
    /// NPoS boundary lacks a finalized election roster.
    #[error("Sumeragi v2 NPoS epoch boundary lacks a finalized next-epoch roster")]
    MissingFinalizedEpochRoster,
}

#[cfg(test)]
mod tests {
    use std::{
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
    };

    use crate::sumeragi::v2_core::{EventTag, Generation};
    use iroha_config::parameters::actual::Queue as QueueConfig;
    use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
    use iroha_data_model::{
        block::{BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire},
        peer::PeerId,
    };

    use super::*;
    use crate::{
        query::store::LiveQueryStore,
        state::World,
        sumeragi::{
            v2_body_store::{BlockSignaturePolicy, V2BodyStore},
            v2_effects::ApplyTask,
        },
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
            let chain_id: ChainId = "sumeragi-v2-apply-crash-test".into();
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
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

            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            };
            let leader_index = context.leader(0);
            let leader = &keys[usize::try_from(leader_index).expect("leader index")];
            let header = BlockHeader::new(
                NonZeroU64::new(context.height).expect("non-zero height"),
                None,
                None,
                None,
                1_000,
                0,
            );
            let signature = SignatureOf::try_from_hash(leader.private_key(), header.hash())
                .expect("sign fixture block");
            let body = SignedBlock::presigned(
                BlockSignature::new(u64::from(leader_index), signature),
                header,
                Vec::new(),
            );
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

            let kura = Kura::blank_kura_for_testing();
            let state = Arc::new(State::new_with_chain_for_testing(
                World::new(),
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
                queue,
                Arc::clone(&kura),
                chain_id,
                Duration::from_secs(1),
                SumeragiNpos::default(),
                AccountId::new(keys[0].public_key().clone()),
                events_sender,
            );
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
