//! Final capture of original State journals and immutable persistence projections.
//!
//! No mutable StateBlock survives this handoff. Membership admission consumes its
//! original writer and releases it with its exact predecessor identity retained.
//! Block hashes likewise move into an owned journal and release their read guard.
//! World and runtime journals are captured after one complete resource admission.
//! Archive plans retain original logical reservations and filesystem owners.
//! The private terminal consumer joins exact QC/Kura/Native custody and refuses
//! outstanding namespace/participant obligations. TODO: complete those geometry
//! and durability owners plus aggregate resource admission before live cutover.

use super::super::*;
use super::{PreparedCarrier, execution_prefix::ValidatedExecutionPrefix};
use crate::query::{
    provider_ingest_finalized::{PreparedProviderIngestCapture, ProviderIngestFinalizedArchiveV1},
    reputation_finalized::{PreparedReputationCapture, ReputationFinalizedArchive},
};

#[path = "runtime_journals.rs"]
mod runtime_journals;

#[path = "decision_binding.rs"]
pub(crate) mod decision_binding;
#[cfg(test)]
use runtime_journals::RuntimeJournalInputs;
use runtime_journals::RuntimeJournals;

/// Candidate journal admission distinguishes local archive failure from execution.
/// Archive inability is not a consensus verdict on the authenticated proposal.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CarrierJournalPreparationError<E> {
    /// The caller could not retain the complete original candidate journals.
    #[error("candidate journal resource admission failed")]
    JournalAdmission(E),
    /// The original World journals do not share one execution mode.
    #[error("candidate World capture: {0}")]
    WorldCapture(#[from] world_journals::CaptureError<std::convert::Infallible>),
    /// The retained local provider projection cannot currently be admitted.
    #[error("provider-ingest candidate capture: {0}")]
    Provider(
        #[from] crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1,
    ),
    /// The retained local reputation projection cannot currently be admitted.
    #[error("reputation candidate capture: {0}")]
    Reputation(#[from] crate::query::reputation_finalized::ReputationFinalizedArchiveError),
    /// The original transaction journal does not extend its owned predecessor.
    #[error("candidate transaction membership: {0}")]
    Membership(#[from] storage_transactions::TransactionsBlockError),
    /// The captured geometry inputs differ from their original runtime owner.
    #[error("candidate geometry identity: {0}")]
    Geometry(#[from] LaneLifecycleError),
}

/// Borrowed complete candidate before any archive, geometry or journal capture.
/// Source custody has already moved out of raw State and remains part of this
/// one admission, including the original witness and actual invocation owners.
pub(crate) struct CarrierJournalInputs<'owner, 'state> {
    /// The original complete staged State journals and deterministic tail.
    pub(crate) state: &'owner StateBlock<'state>,
    /// Exact validated execution owners retained before that tail changed World.
    pub(crate) prefix: &'owner ValidatedExecutionPrefix,
}

/// Original journals after candidate execution, deterministic tails and capture.
/// The default lifecycle is the original ValidBlock. Only the private consuming
/// decision binder changes it to CommittedBlock; dropping either publishes nothing.
pub(crate) struct PreparedCarrierJournals<
    Admission,
    Block = crate::block::ValidBlock,
    Components = DetachedCarrierComponents,
> {
    valid: Block,
    context: Arc<iroha_data_model::block::consensus_v2::HeightContext>,
    execution_prefix: iroha_data_model::block::consensus_v2::ExecutionCommitment,
    native_amx_manifest: crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    source_prefix: ValidatedExecutionPrefix,
    checkpoint: Hash,
    kura: Arc<Kura>,
    components: Components,
    world_effects: world_commit::PreparedWorldEffects,
    geometry: carrier_geometry_preparation::PreparedCarrierGeometry,
    provider_capture: Option<PreparedProviderIngestCapture>,
    reputation_capture: Option<PreparedReputationCapture>,
    publication_events: Vec<EventBox>,
    tiered_snapshot: tiered_publication::PreparedTieredSnapshot,
    effects: RetainedCarrierEffects,
    // Rust drops fields in declaration order. Capacity outlives every retained
    // journal, archive plan and deferred effect, including partial publication.
    admission: Admission,
}

/// The four storage families which must acquire one joint original predecessor.
/// This group has no publication authority independently of the complete carrier.
pub(crate) struct DetachedCarrierComponents {
    world: world_journals::DetachedWorld<()>,
    transactions: storage_transactions::DetachedTransactionsBlock,
    block_hashes: DetachedBlockHashes,
    runtime: RuntimeJournals<()>,
}

/// Deferred effects and original proof owners needed by the consuming publisher.
struct RetainedCarrierEffects {
    header: BlockHeader,
    nexus: iroha_config::parameters::actual::Nexus,
    runtime_policy: canonical_runtime::CapturedRuntimePolicy,
    sccp_registry: Arc<ValidatedSccpRegistryV1>,
    verified_lane_relay_records: Vec<VerifiedLaneRelayRecord>,
    pending_da_commitments: Option<PendingDaCommitmentBundle>,
    pending_autoscale_lifecycle: Option<PendingAutoscaleLaneLifecycle>,
    staged_merge_entry: Option<MergeLedgerEntry>,
    canonical_wsv_merge_commit_authorization: Option<CanonicalWsvMergeCommitAuthorization>,
    canonical_carrier_commit_metadata_authorization:
        Option<CanonicalCarrierCommitMetadataAuthorization>,
    merge_carrier_entrypoints: HashSet<HashOf<TransactionEntrypoint>>,
    pending_public_lane_slash_observability: Vec<PendingPublicLaneSlashObservability>,
    #[cfg(feature = "telemetry")]
    committed_parliament_attempt_counts: Option<ParliamentAttemptCountsV1>,
    #[cfg(feature = "telemetry")]
    committed_citizens_total: Option<u64>,
    #[cfg(feature = "telemetry")]
    pending_parliament_telemetry_events: Vec<(
        iroha_data_model::isi::governance::ParliamentLifecycleTransitionKindV1,
        Option<iroha_data_model::governance::types::ParliamentNoResultKindV1>,
    )>,
    authenticated_replay_commit: bool,
    replay_prevalidation: bool,
}

impl<'state> PreparedCarrier<'state> {
    /// Capture read-only projections, then consume the actual journals once.
    ///
    /// StateReadOnly is used only before decomposition. No surrogate State,
    /// reconstructed membership writer or second World tail is introduced.
    /// The required admission callback sees the complete original StateBlock and retained execution prefix
    /// before any final journal value is copied. Its returned reservation stays
    /// alive until all journals and deferred effects have been released.
    pub(crate) fn prepare_journals<Admission, E>(
        self,
        provider_archive: Option<&Arc<ProviderIngestFinalizedArchiveV1>>,
        reputation_archive: Option<&Arc<ReputationFinalizedArchive>>,
        admit_journals: impl FnOnce(CarrierJournalInputs<'_, 'state>) -> Result<Admission, E>,
    ) -> Result<PreparedCarrierJournals<Admission>, CarrierJournalPreparationError<E>> {
        // Declare before the original owners: reverse local drop order must
        // release them before capacity on every early error, including archive
        // admission before the StateBlock has been decomposed.
        let admission;
        let Self {
            valid,
            state,
            context,
            execution_prefix,
            native_amx_manifest,
            source_prefix,
            _world_effects: world_effects,
            _publication_events: publication_events,
            _tiered_snapshot: tiered_snapshot,
        } = self;
        // Admit capture overlap, retained originals/final values and eventual
        // installation before projecting geometry/archives or detaching a journal.
        // The callback can inspect the original typed World/runtime/source inputs; it
        // cannot mutate them or treat this local reservation as finality.
        admission = admit_journals(CarrierJournalInputs {
            state: &state,
            prefix: &source_prefix,
        })
        .map_err(CarrierJournalPreparationError::JournalAdmission)?;
        let geometry = state.prepare_carrier_geometry()?;
        // Fixed admission order: State journals -> provider -> reputation.
        // Each archive releases its writer once the exact insertion and logical
        // reservation are owned. Kura never takes an archive index writer.
        let provider_capture = provider_archive
            .map(|archive| archive.prepare_candidate_capture(state.as_ref(), &state.state_ref.kura))
            .transpose()?;
        let reputation_capture = reputation_archive
            .map(|archive| archive.prepare_candidate_capture(state.as_ref(), &state.state_ref.kura))
            .transpose()?;
        let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(&state);
        #[cfg(feature = "telemetry")]
        let committed_parliament_attempt_counts = state
            .world
            .parliament_attempt_counts
            .is_dirty()
            .then(|| *state.world.parliament_attempt_counts.get());
        #[cfg(feature = "telemetry")]
        let committed_citizens_total =
            state.world.citizens.is_dirty().then(|| {
                u64::try_from(state.world.citizens.len()).expect("citizen count fits u64")
            });
        let StateBlock {
            state_ref,
            runtime_policy,
            world,
            transactions,
            block_hashes,
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
            _curr_block: header,
            nexus,
            sccp_registry,
            verified_lane_relay_records,
            pending_da_commitments,
            pending_autoscale_lifecycle,
            staged_merge_entry,
            native_lane_stage: _,
            execution_output_plan: _,
            fastpq_source_inventory: _,
            canonical_wsv_merge_commit_authorization,
            canonical_carrier_commit_metadata_authorization,
            merge_carrier_entrypoints,
            exec_witness: _,
            fastpq_witness_context: _,
            parliament_timed_ovn_casting_bindings: _,
            pending_public_lane_slash_observability,
            #[cfg(feature = "telemetry")]
            pending_parliament_telemetry_events,
            authenticated_replay_commit,
            replay_prevalidation,
            ..
        } = *state;
        let world = world.try_detach_journals(|_| Ok::<(), std::convert::Infallible>(()))?;
        let runtime = RuntimeJournals::capture(
            canonical_runtime,
            commit_topology,
            prev_commit_topology,
            lane_consensus_contexts,
            |_| Ok::<(), std::convert::Infallible>(()),
        )
        .unwrap_or_else(|never| match never {});
        let transactions = transactions.prepare_commit()?.detach();
        let block_hashes = block_hashes.detach();
        Ok(PreparedCarrierJournals {
            valid,
            context,
            execution_prefix,
            native_amx_manifest,
            source_prefix,
            checkpoint,
            kura: Arc::clone(&state_ref.kura),
            components: DetachedCarrierComponents {
                world,
                transactions,
                block_hashes,
                runtime,
            },
            world_effects,
            provider_capture,
            reputation_capture,
            geometry,
            publication_events,
            tiered_snapshot,
            effects: RetainedCarrierEffects {
                header,
                nexus,
                runtime_policy,
                sccp_registry,
                verified_lane_relay_records,
                pending_da_commitments,
                pending_autoscale_lifecycle,
                staged_merge_entry,
                canonical_wsv_merge_commit_authorization,
                canonical_carrier_commit_metadata_authorization,
                merge_carrier_entrypoints,
                pending_public_lane_slash_observability,
                #[cfg(feature = "telemetry")]
                committed_parliament_attempt_counts,
                #[cfg(feature = "telemetry")]
                committed_citizens_total,
                #[cfg(feature = "telemetry")]
                pending_parliament_telemetry_events,
                authenticated_replay_commit,
                replay_prevalidation,
            },
            admission,
        })
    }
}

impl<Admission> PreparedCarrierJournals<Admission> {
    /// Inspect Native custody after every State writer has been released.
    #[cfg(test)]
    pub(in crate::state) fn native_source_for_test(&self) -> Option<&NativeExecutionCustody> {
        self.source_prefix.native_for_test()
    }

    /// Return the prefix authenticated before the complete deterministic tail.
    pub(crate) fn execution_prefix_commitment(
        &self,
    ) -> iroha_data_model::block::consensus_v2::ExecutionCommitment {
        self.execution_prefix
    }
}

impl<Admission, Block, Components> PreparedCarrierJournals<Admission, Block, Components> {
    /// Borrow the original source owners after State journal detachment.
    pub(super) fn source_prefix(&self) -> &ValidatedExecutionPrefix {
        &self.source_prefix
    }

    /// Move the same complete carrier around one consuming storage transition.
    /// Failed acquisition returns every original nonstorage owner unchanged too.
    fn try_map_components<Next, E>(
        self,
        acquire: impl FnOnce(Components) -> Result<Next, (Components, E)>,
    ) -> Result<PreparedCarrierJournals<Admission, Block, Next>, (Self, E)> {
        // Capacity must outlive values on a panic inside component preparation.
        let admission;
        let Self {
            valid,
            context,
            execution_prefix,
            native_amx_manifest,
            source_prefix,
            checkpoint,
            kura,
            world_effects,
            geometry,
            provider_capture,
            reputation_capture,
            publication_events,
            tiered_snapshot,
            effects,
            components,
            admission: original_admission,
        } = self;
        admission = original_admission;
        macro_rules! retain {
            ($components:expr) => {
                PreparedCarrierJournals {
                    valid,
                    context,
                    execution_prefix,
                    native_amx_manifest,
                    source_prefix,
                    checkpoint,
                    kura,
                    world_effects,
                    geometry,
                    provider_capture,
                    reputation_capture,
                    publication_events,
                    tiered_snapshot,
                    effects,
                    components: $components,
                    admission,
                }
            };
        }
        match acquire(components) {
            Ok(components) => Ok(retain!(components)),
            Err((components, error)) => Err((retain!(components), error)),
        }
    }
}

#[cfg(test)]
#[path = "journals_tests.rs"]
mod tests;
