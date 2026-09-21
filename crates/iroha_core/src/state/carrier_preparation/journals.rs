//! Final capture of original State journals and immutable persistence projections.
//!
//! No mutable StateBlock survives this handoff. Membership admission consumes its
//! original writer and releases it with its exact predecessor identity retained.
//! Block hashes move their original private tree without a chain copy or physical lock.
//! World/runtime journals and tiered snapshots follow one resource admission.
//! Archive plans retain original logical reservations and filesystem owners.
//! The private terminal consumer joins exact QC/Kura/Native custody, retains the
//! original Queue for namespace retirement, and refuses retired participant
//! manifests. TODO: complete aggregate resource admission and carry these original
//! owners through live Validate/cache/Apply before retiring the old writer.

use super::super::*;
use super::{PreparedCarrier, execution_prefix::ValidatedExecutionPrefix};
#[cfg(test)]
use crate::query::{
    provider_ingest_finalized::ProviderIngestFinalizedArchiveV1,
    reputation_finalized::ReputationFinalizedArchive,
};
use crate::query::{
    provider_ingest_finalized::{PreparedProviderIngestCapture, ProviderCandidateCapture},
    reputation_finalized::{PreparedReputationCapture, ReputationCandidateCapture},
};

#[path = "runtime_journals.rs"]
mod runtime_journals;

#[path = "decision_binding.rs"]
pub(crate) mod decision_binding;
pub(crate) use decision_binding::{PublishedNativeApply, RetainedCarrier};
#[cfg(test)]
use runtime_journals::RuntimeJournalInputs;
use runtime_journals::RuntimeJournals;

/// Candidate journal admission distinguishes local archive failure from execution.
/// Archive inability is not a consensus verdict on the authenticated proposal.
#[derive(thiserror::Error)]
pub(crate) enum CarrierJournalPreparationError<'state, Admission, E> {
    /// The caller could not retain the complete original candidate journals.
    #[error("candidate journal resource admission failed")]
    JournalAdmission {
        /// The unmodified borrowed owner; callers must not retain it across a wait.
        carrier: PreparedCarrier<'state>,
        /// Original archive predecessors, still reserved for a synchronous retry.
        provider: Option<ProviderCandidateCapture>,
        /// Original reputation predecessor and its exact capture cursors.
        reputation: Option<ReputationCandidateCapture>,
        /// Original resource-admission refusal, before any allocating capture.
        error: E,
    },
    /// The original World journals do not share one execution mode.
    #[error("candidate World capture: {0}")]
    WorldCapture(#[from] world_journals::CaptureError<std::convert::Infallible>),
    /// State writers are released and all original journals survive archive refusal.
    #[error("candidate archive preparation: {error}")]
    ArchivePreparation {
        /// Exact lifetime-free carrier, including original captured archive material.
        carrier: Box<StagedCarrierCapture<Admission>>,
        /// Typed local dependency or recovery diagnostic, never proposal invalidity.
        error: CarrierArchivePreparationError,
    },
    /// The original transaction journal does not extend its owned predecessor.
    #[error("candidate transaction membership: {0}")]
    Membership(#[from] storage_transactions::TransactionsBlockError),
    /// The captured geometry inputs differ from their original runtime owner.
    #[error("candidate geometry identity: {0}")]
    Geometry(#[from] LaneLifecycleError),
}

impl<Admission, E: std::fmt::Debug> std::fmt::Debug
    for CarrierJournalPreparationError<'_, Admission, E>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JournalAdmission { error, .. } => {
                f.debug_tuple("JournalAdmission").field(error).finish()
            }
            Self::WorldCapture(error) => f.debug_tuple("WorldCapture").field(error).finish(),
            Self::Membership(error) => f.debug_tuple("Membership").field(error).finish(),
            Self::Geometry(error) => f.debug_tuple("Geometry").field(error).finish(),
            Self::ArchivePreparation { error, .. } => {
                f.debug_tuple("ArchivePreparation").field(error).finish()
            }
        }
    }
}

/// Cloneable diagnostic retaining the exact archive's refusal and release observation.
#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum CarrierArchivePreparationError {
    /// Provider capture or insertion admission remains locally unavailable.
    #[error("provider-ingest candidate capture: {0}")]
    Provider(Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1>),
    /// Reputation capture or insertion admission remains locally unavailable.
    #[error("reputation candidate capture: {0}")]
    Reputation(Arc<crate::query::reputation_finalized::ReputationFinalizedArchiveError>),
}

/// Detached execution with partially prepared archives; no State reference or writer survives.
/// Only successful completion exposes the existing decision-binding journal owner.
pub(crate) struct StagedCarrierCapture<Admission> {
    provider: Option<ProviderCandidateCapture>,
    reputation: Option<ReputationCandidateCapture>,
    // A failed original capture cannot be retried against another State view.
    capture_refusal: Option<CarrierArchivePreparationError>,
    // Capacity is inside this last field and therefore outlives archive payloads.
    journals: PreparedCarrierJournals<Admission>,
}

impl<Admission> StagedCarrierCapture<Admission> {
    /// Match the original executed proposal while archive capture remains incomplete.
    pub(crate) fn matches_candidate(
        &self,
        context: &iroha_data_model::block::consensus_v2::HeightContext,
        proposal: &iroha_data_model::block::SignedBlock,
    ) -> bool {
        self.journals
            .matches_validation_candidate(context, proposal)
    }

    /// Resume the exact boxed capture without replacing its allocation on refusal.
    pub(crate) fn try_complete(
        mut self: Box<Self>,
    ) -> Result<PreparedCarrierJournals<Admission>, (Box<Self>, CarrierArchivePreparationError)>
    {
        if let Err(error) = self.try_prepare_archives() {
            return Err((self, error));
        }
        Ok((*self).into_journals())
    }

    // Initial capture and boxed retries prepare the same retained archive owners.
    // This borrowed step never moves the large carrier or allocates another box.
    fn try_prepare_archives(&mut self) -> Result<(), CarrierArchivePreparationError> {
        if let Some(error) = &self.capture_refusal {
            return Err(error.clone());
        }
        if let Some(provider) = &mut self.provider {
            provider
                .try_prepare()
                .map_err(|error| CarrierArchivePreparationError::Provider(Arc::new(error)))?;
        }
        if let Some(reputation) = &mut self.reputation {
            reputation
                .try_prepare()
                .map_err(|error| CarrierArchivePreparationError::Reputation(Arc::new(error)))?;
        }
        Ok(())
    }

    // Both callers complete original insertion preparation before moving journals.
    fn into_journals(mut self) -> PreparedCarrierJournals<Admission> {
        self.journals.provider_capture = self.provider.take().map(|owner| {
            owner
                .into_prepared()
                .ok()
                .expect("successful exact provider preparation")
        });
        self.journals.reputation_capture = self.reputation.take().map(|owner| {
            owner
                .into_prepared()
                .ok()
                .expect("successful exact reputation preparation")
        });
        self.journals
    }
}

/// Borrowed complete candidate before original-State projection or journal capture.
/// Source custody has already moved out of raw State and remains part of this
/// one admission, including the original witness and actual invocation owners.
/// Archive predecessor owners were acquired earlier under their existing bounds;
/// this callback does not retroactively fund those preexecution allocations.
pub(crate) struct CarrierJournalInputs<'owner, 'state> {
    /// Original result-bearing block, including its retained validation state.
    pub(crate) valid: &'owner crate::block::ValidBlock,
    /// The original complete staged State journals and deterministic tail.
    pub(crate) state: &'owner StateBlock<'state>,
    /// Exact validated execution owners retained before that tail changed World.
    pub(crate) prefix: &'owner ValidatedExecutionPrefix,
    /// Original shared context allocation, not a reconstructed context identity.
    pub(crate) context: &'owner Arc<iroha_data_model::block::consensus_v2::HeightContext>,
    /// Commitment retained with these exact execution owners.
    pub(crate) execution_prefix: &'owner iroha_data_model::block::consensus_v2::ExecutionCommitment,
    /// Original manifest and its retained entry/result/tree allocations.
    pub(crate) native_amx_manifest: &'owner crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    /// Original deferred DA cache records, exposing capacity as well as contents.
    pub(crate) da_pins: &'owner Vec<DaPinIntentWithLocation>,
    /// Original event allocation, including unused capacity and nested payloads.
    pub(crate) publication_events: &'owner Vec<EventBox>,
    /// Exact preexecution provider owner; this is not a new archive observation.
    pub(crate) provider: Option<&'owner ProviderCandidateCapture>,
    /// Exact preexecution reputation predecessor and its retained capture cursors.
    pub(crate) reputation: Option<&'owner ReputationCandidateCapture>,
    /// Exact pointee layout of the effects Box allocated after this admission.
    /// This covers its inline storage; nested owners still require their own
    /// accounting within the complete original candidate admission.
    pub(crate) retained_effects_layout: std::alloc::Layout,
}

impl CarrierJournalInputs<'_, '_> {
    /// Exact World wrapper demand available to the aggregate capture admission.
    /// It matches the preexecution plan without reading or cloning State values.
    /// This does not fund nested values or the other original carrier owners.
    pub(crate) fn world_journal_shell_bytes(
        &self,
    ) -> Result<usize, mv::allocation::AllocationRefusal> {
        PreparedCarrier::world_journal_shell_bytes()
    }
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
    // Keep one admitted allocation across consuming phase transitions. Inline
    // policies and lifecycle effects otherwise multiply across every owned
    // success/refusal value, exhausting ordinary stacks during authentication.
    effects: Box<RetainedCarrierEffects>,
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
    da_commitments: Option<carrier_da_effects::PreparedDaCommitmentEffects>,
    lifecycle: Option<carrier_lifecycle_effects::PreparedLaneLifecycleEffects>,
    staged_merge_entry: Option<MergeLedgerEntry>,
    canonical_wsv_merge_commit_authorization: Option<CanonicalWsvMergeCommitAuthorization>,
    canonical_carrier_commit_metadata_authorization:
        Option<CanonicalCarrierCommitMetadataAuthorization>,
    merge_carrier_entrypoints: HashSet<HashOf<TransactionEntrypoint>>,
    pending_public_lane_slash_observability: Vec<PendingPublicLaneSlashObservability>,
    #[cfg(feature = "telemetry")]
    pending_parliament_telemetry_events: Vec<(
        iroha_data_model::isi::governance::ParliamentLifecycleTransitionKindV1,
        Option<iroha_data_model::governance::types::ParliamentNoResultKindV1>,
    )>,
    #[cfg(feature = "telemetry")]
    committed_parliament_attempt_counts: Option<ParliamentAttemptCountsV1>,
    #[cfg(feature = "telemetry")]
    committed_citizens_total: Option<u64>,
    #[cfg(feature = "telemetry")]
    committed_musubi_replication_shortfall_releases: u64,
    authenticated_replay_commit: bool,
    replay_prevalidation: bool,
}

impl<'state> PreparedCarrier<'state> {
    /// Capture original-State projections, detach the actual journals, then prepare archives.
    ///
    /// StateReadOnly is used only before decomposition. No surrogate State,
    /// reconstructed membership writer or second World tail is introduced.
    /// The required admission callback sees every retained candidate owner before
    /// projections, the retained-effects allocation and journal detachment. Its
    /// borrowed inputs preserve allocation capacities; serialized lengths alone
    /// do not account for retained memory.
    /// Detachment moves original MV allocations without cloning; execution's
    /// earlier allocations require their own prior admission. The reservation stays
    /// alive until all journals and deferred effects have been released. Archive
    /// arguments must be the exact predecessor owners reserved before execution.
    /// Admission refusal returns the original borrowed carrier and these owners
    /// for synchronous handling only; no State writer may cross an async wait.
    pub(crate) fn prepare_journals<Admission, E>(
        self,
        provider_capture: Option<ProviderCandidateCapture>,
        reputation_capture: Option<ReputationCandidateCapture>,
        admit_journals: impl FnOnce(CarrierJournalInputs<'_, 'state>) -> Result<Admission, E>,
    ) -> Result<
        PreparedCarrierJournals<Admission>,
        CarrierJournalPreparationError<'state, Admission, E>,
    > {
        // Exhaustively borrow the complete owner. Adding a retained field must
        // also update admission; a partial State/prefix projection is insufficient.
        let Self {
            valid,
            state,
            source_prefix,
            context,
            execution_prefix,
            native_amx_manifest,
            _world_effects,
            _publication_events,
        } = &self;
        // Declare before the original owners: reverse local drop order must
        // release them before capacity on every early error, including archive
        // admission before the StateBlock has been decomposed.
        let admission = match admit_journals(CarrierJournalInputs {
            valid,
            state,
            prefix: source_prefix,
            context,
            execution_prefix,
            native_amx_manifest,
            da_pins: _world_effects.admission_pins(),
            publication_events: _publication_events,
            provider: provider_capture.as_ref(),
            reputation: reputation_capture.as_ref(),
            retained_effects_layout: std::alloc::Layout::new::<RetainedCarrierEffects>(),
        }) {
            Ok(admission) => admission,
            Err(error) => {
                return Err(CarrierJournalPreparationError::JournalAdmission {
                    carrier: self,
                    provider: provider_capture,
                    reputation: reputation_capture,
                    error,
                });
            }
        };
        // Parameters drop after locals. Move archive payload owners into locals
        // declared after capacity so every capture/error/unwind releases them first.
        let mut provider_capture = provider_capture;
        let mut reputation_capture = reputation_capture;
        let Self {
            valid,
            mut state,
            context,
            execution_prefix,
            native_amx_manifest,
            source_prefix,
            _world_effects: world_effects,
            _publication_events: publication_events,
        } = self;
        // Admit capture overlap, retained originals/final values and eventual
        // installation before projecting geometry/archives or detaching a journal.
        // The callback can inspect the original typed World/runtime/source inputs; it
        // cannot mutate them or treat this local reservation as finality.
        // Preserve commit's dirty-only gauges from this exact overlay. Once
        // detached, reading live World would observe a different candidate.
        #[cfg(feature = "telemetry")]
        let committed_parliament_attempt_counts = state
            .world
            .parliament_attempt_counts
            .is_dirty()
            .then(|| *state.world.parliament_attempt_counts.get());
        #[cfg(feature = "telemetry")]
        let committed_citizens_total = state.world.citizens.is_dirty().then(|| {
            u64::try_from(state.world.citizens.len())
                .expect("committed Parliament citizen count must fit into u64")
        });
        #[cfg(feature = "telemetry")]
        let committed_musubi_replication_shortfall_releases =
            *state.world.musubi_replication_shortfall_releases.get();
        // A cold backend copies the complete tiered baseline. Capture only after
        // admission, from the same immutable World whose deterministic tail was
        // prepared above. The reservation outlives this payload on every exit.
        let tiered_snapshot = tiered_publication::PreparedTieredSnapshot::prepare(
            &state.world,
            &state.state_ref.tiered_snapshot_worker,
        );
        let geometry = state.prepare_carrier_geometry()?;
        // Reservations were acquired before execution. Original-State capture
        // never acquires an archive index; every refusal still detaches the
        // admitted execution before returning control to a possible waiter.
        let mut capture_refusal = provider_capture
            .as_mut()
            .and_then(|owner| owner.capture_original(state.as_ref()).err())
            .map(|error| CarrierArchivePreparationError::Provider(Arc::new(error)));
        if capture_refusal.is_none() {
            capture_refusal = reputation_capture
                .as_mut()
                .and_then(|owner| owner.capture_original(state.as_ref()).err())
                .map(|error| CarrierArchivePreparationError::Reputation(Arc::new(error)));
        }
        let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(&state);
        let lifecycle = state.pending_autoscale_lifecycle.as_ref().map(|pending| {
            carrier_lifecycle_effects::PreparedLaneLifecycleEffects::prepare(pending, &state.nexus)
        });
        let da_commitments = state.pending_da_commitments.take().map(|pending| {
            carrier_da_effects::PreparedDaCommitmentEffects::prepare(
                pending,
                &state.nexus,
                state.canonical_runtime.get(),
            )
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
            pending_da_commitments: _,
            pending_autoscale_lifecycle: _,
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
        let journals = PreparedCarrierJournals {
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
            provider_capture: None,
            reputation_capture: None,
            geometry,
            publication_events,
            tiered_snapshot,
            effects: {
                #[cfg(test)]
                tests::observe_effects_allocation_attempt();
                Box::new(RetainedCarrierEffects {
                    header,
                    nexus,
                    runtime_policy,
                    sccp_registry,
                    verified_lane_relay_records,
                    da_commitments,
                    lifecycle,
                    staged_merge_entry,
                    canonical_wsv_merge_commit_authorization,
                    canonical_carrier_commit_metadata_authorization,
                    merge_carrier_entrypoints,
                    pending_public_lane_slash_observability,
                    #[cfg(feature = "telemetry")]
                    pending_parliament_telemetry_events,
                    #[cfg(feature = "telemetry")]
                    committed_parliament_attempt_counts,
                    #[cfg(feature = "telemetry")]
                    committed_citizens_total,
                    #[cfg(feature = "telemetry")]
                    committed_musubi_replication_shortfall_releases,
                    authenticated_replay_commit,
                    replay_prevalidation,
                })
            },
            admission,
        };
        let mut carrier = StagedCarrierCapture {
            provider: provider_capture,
            reputation: reputation_capture,
            capture_refusal,
            journals,
        };
        if let Err(error) = carrier.try_prepare_archives() {
            return Err(CarrierJournalPreparationError::ArchivePreparation {
                carrier: Box::new(carrier),
                error,
            });
        }
        Ok(carrier.into_journals())
    }
}

impl<Admission, Block: AsRef<iroha_data_model::block::SignedBlock>, Components>
    PreparedCarrierJournals<Admission, Block, Components>
{
    /// Match a retry against the original context and resultless proposal wire.
    /// Results remain owned by this carrier; they are not supplied by a retry.
    pub(crate) fn matches_validation_candidate(
        &self,
        context: &iroha_data_model::block::consensus_v2::HeightContext,
        proposal: &iroha_data_model::block::SignedBlock,
    ) -> bool {
        if self.context.as_ref() != context {
            return false;
        }
        match (
            self.valid.as_ref().canonical_proposal_wire_hash(),
            proposal.canonical_proposal_wire_hash(),
        ) {
            (Ok(original), Ok(candidate)) => original == candidate,
            _ => false,
        }
    }
}

impl<Admission, Block, Components> PreparedCarrierJournals<Admission, Block, Components> {
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

    /// Borrow the original source owners after State journal detachment.
    pub(in crate::state) fn source_prefix(&self) -> &ValidatedExecutionPrefix {
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
