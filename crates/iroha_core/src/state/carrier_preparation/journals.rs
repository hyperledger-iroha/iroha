//! Final capture of original State journals and immutable persistence projections.
//!
//! No mutable StateBlock survives this handoff. Membership admission consumes its
//! original writer and releases it with its exact predecessor identity retained.
//! Block hashes likewise move into an owned journal and release their read guard.
//! World and runtime journals are captured after one complete resource admission.
//! Archive plans retain original logical reservations and filesystem owners.
//! TODO: join complete geometry/resource admission and exact QC/Kura/Native
//! authorization before exposing the sole consuming publication operation.

use super::super::*;
use super::PreparedCarrier;
use crate::query::{
    provider_ingest_finalized::{PreparedProviderIngestCapture, ProviderIngestFinalizedArchiveV1},
    reputation_finalized::{PreparedReputationCapture, ReputationFinalizedArchive},
};

#[path = "runtime_journals.rs"]
mod runtime_journals;
#[cfg(test)]
use runtime_journals::RuntimeJournalInputs;
use runtime_journals::RuntimeJournals;

/// Candidate journal admission distinguishes local archive failure from execution.
/// Archive inability is not a consensus verdict on the authenticated proposal.
#[derive(Debug, thiserror::Error)]
pub(in crate::state) enum CarrierJournalPreparationError<E> {
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
    /// An internal preparation invariant was lost before the consuming handoff.
    #[error("prepared carrier lost its execution witness")]
    MissingWitness,
}

/// Original journals after candidate execution, deterministic tails and capture.
/// Construction grants no finality; dropping the owner publishes nothing.
pub(in crate::state) struct PreparedCarrierJournals<Admission> {
    valid: crate::block::ValidBlock,
    context: Arc<iroha_data_model::block::consensus_v2::HeightContext>,
    execution_prefix: iroha_data_model::block::consensus_v2::ExecutionCommitment,
    #[cfg_attr(
        test,
        expect(
            dead_code,
            reason = "TODO: consume retained journals and effects in the aggregate State publisher"
        )
    )]
    native_amx_manifest: crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    checkpoint: Hash,
    kura: Arc<Kura>,
    world: world_journals::DetachedWorld<()>,
    #[cfg_attr(
        test,
        expect(
            dead_code,
            reason = "TODO: consume retained journals and effects in the aggregate State publisher"
        )
    )]
    world_effects: world_commit::PreparedWorldEffects,
    transactions: storage_transactions::DetachedTransactionsBlock,
    block_hashes: DetachedBlockHashes,
    runtime: RuntimeJournals<()>,
    #[cfg_attr(
        test,
        expect(
            dead_code,
            reason = "TODO: consume retained journals and effects in the aggregate State publisher"
        )
    )]
    geometry: carrier_geometry_preparation::PreparedCarrierGeometry,
    provider_capture: Option<PreparedProviderIngestCapture>,
    reputation_capture: Option<PreparedReputationCapture>,
    publication_events: Vec<EventBox>,
    #[cfg_attr(
        test,
        expect(
            dead_code,
            reason = "TODO: consume retained journals and effects in the aggregate State publisher"
        )
    )]
    tiered_snapshot: tiered_publication::PreparedTieredSnapshot,
    effects: RetainedCarrierEffects,
    // Rust drops fields in declaration order. Capacity outlives every retained
    // journal, archive plan and deferred effect, including partial publication.
    admission: Admission,
}

/// Deferred effects and original proof owners needed by the consuming publisher.
#[cfg_attr(
    test,
    expect(
        dead_code,
        reason = "TODO: consume retained journals and effects in the aggregate State publisher"
    )
)]
struct RetainedCarrierEffects {
    header: BlockHeader,
    nexus: iroha_config::parameters::actual::Nexus,
    runtime_policy: canonical_runtime::CapturedRuntimePolicy,
    sccp_registry: Arc<ValidatedSccpRegistryV1>,
    verified_lane_relay_records: Vec<VerifiedLaneRelayRecord>,
    pending_da_commitments: Option<PendingDaCommitmentBundle>,
    pending_autoscale_lifecycle: Option<PendingAutoscaleLaneLifecycle>,
    staged_merge_entry: Option<MergeLedgerEntry>,
    native_lane_stage: Option<Box<lane_decision_batch::NativeLaneStageSealV1>>,
    execution_output_plan: Option<output_capacity::ExecutionOutputPlanState>,
    fastpq_source_inventory: Option<Result<Arc<FastpqSourceInventoryV1>, String>>,
    canonical_wsv_merge_commit_authorization: Option<CanonicalWsvMergeCommitAuthorization>,
    canonical_carrier_commit_metadata_authorization:
        Option<CanonicalCarrierCommitMetadataAuthorization>,
    merge_carrier_entrypoints: HashSet<HashOf<TransactionEntrypoint>>,
    witness: ExecWitness,
    fastpq_witness_context: Option<crate::fastpq::FastpqWitnessContext>,
    parliament_timed_ovn_casting_bindings: Option<
        Vec<iroha_data_model::parliament_casting::ParliamentTimedOvnCastingContextBindingV1>,
    >,
    pending_public_lane_slash_observability: Vec<PendingPublicLaneSlashObservability>,
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
    /// The required admission callback sees the complete original StateBlock
    /// before any final journal value is copied. Its returned reservation stays
    /// alive until all journals and deferred effects have been released.
    pub(in crate::state) fn prepare_journals<Admission, E>(
        self,
        provider_archive: Option<&Arc<ProviderIngestFinalizedArchiveV1>>,
        reputation_archive: Option<&Arc<ReputationFinalizedArchive>>,
        admit_journals: impl FnOnce(&StateBlock<'state>) -> Result<Admission, E>,
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
            _world_effects: world_effects,
            _publication_events: publication_events,
            _tiered_snapshot: tiered_snapshot,
        } = self;
        let geometry = state.prepare_carrier_geometry()?;
        // Admit capture overlap, retained originals/final values and eventual
        // installation before projecting archives or detaching any journal.
        // The callback can inspect the original typed World/runtime inputs; it
        // cannot mutate them or treat this local reservation as finality.
        admission =
            admit_journals(&state).map_err(CarrierJournalPreparationError::JournalAdmission)?;
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
            native_lane_stage,
            execution_output_plan,
            fastpq_source_inventory,
            canonical_wsv_merge_commit_authorization,
            canonical_carrier_commit_metadata_authorization,
            merge_carrier_entrypoints,
            exec_witness,
            fastpq_witness_context,
            parliament_timed_ovn_casting_bindings,
            pending_public_lane_slash_observability,
            #[cfg(feature = "telemetry")]
            pending_parliament_telemetry_events,
            authenticated_replay_commit,
            replay_prevalidation,
            ..
        } = *state;
        let witness = exec_witness.ok_or(CarrierJournalPreparationError::MissingWitness)?;
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
            checkpoint,
            kura: Arc::clone(&state_ref.kura),
            world,
            world_effects,
            transactions,
            block_hashes,
            runtime,
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
                native_lane_stage,
                execution_output_plan,
                fastpq_source_inventory,
                canonical_wsv_merge_commit_authorization,
                canonical_carrier_commit_metadata_authorization,
                merge_carrier_entrypoints,
                witness,
                fastpq_witness_context,
                parliament_timed_ovn_casting_bindings,
                pending_public_lane_slash_observability,
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
    /// Return the prefix authenticated before the complete deterministic tail.
    pub(crate) fn execution_prefix_commitment(
        &self,
    ) -> iroha_data_model::block::consensus_v2::ExecutionCommitment {
        self.execution_prefix
    }
}

#[cfg(test)]
#[path = "journals_tests.rs"]
mod tests;
