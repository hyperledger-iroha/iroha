//! Exact local publication surface retained around verified finality preparation.
//!
//! The original MV scopes retain the immutable predecessor. This seal binds their
//! actual net changes, bounded runtime state and deferred publication effects
//! without scanning historical World or transaction membership. It is not finality
//! authority; only the output owner can retain it after verification.

use super::world_projection::{WorldPublicationDelta, hash_value};
use super::*;

/// Move-only comparison surface for one actual execution carrier.
#[derive(Debug, PartialEq)]
pub(in crate::state) struct FinalizedPublicationSurface {
    chain_id: iroha_model_base::chain::ChainId,
    network_id: iroha_data_model::NetworkId,
    transactions: storage_transactions::TransactionsPublicationSurface,
    block_hashes: BlockHashSurface,
    runtime: SnapshotNexusRuntime,
    runtime_preimage: Option<SnapshotNexusRuntime>,
    state_journals: [mv::BlockPublicationIdentity; 4],
    state_preimages: [Option<Hash>; 3],
    lane_contexts: Hash,
    lane_contexts_seal: Option<Hash>,
    world: WorldPublicationDelta,
    world_journals: Vec<mv::BlockPublicationIdentity>,
    world_dataspaces: DataSpaceCatalog,
    external_events: Hash,
    header: BlockHeader,
    previous_topology: Vec<PeerId>,
    next_topology: Vec<PeerId>,
    execution_policy: [u8; 32],
    lane_catalog: LaneCatalog,
    dataspaces: DataSpaceCatalog,
    lane_config: iroha_config::parameters::actual::LaneConfig,
    routing: LaneRoutingPolicy,
    last_autoscale_transition: u64,
    lane_incarnations: BTreeMap<LaneId, Hash>,
    lane_lineage: BTreeMap<LaneId, LaneIncarnationLineage>,
    lane_activation_heights: BTreeMap<LaneId, u64>,
    manifests: [u8; 32],
    privacy: LanePrivacyRegistryHandle,
    sccp_registry: [u8; 32],
    da_commitments: Option<(u64, Hash)>,
    da_pins: Option<(u64, Hash, Hash)>,
    lifecycle: Option<LifecycleSurface>,
    sample_history: VecDeque<AutoscaleSampleRecord>,
    sample_history_dirty: bool,
    evaluated_fragment_count: Option<u64>,
    relay_records: Hash,
    merge_entry: Option<HashOf<MergeLedgerEntry>>,
    merge_entrypoints: BTreeSet<HashOf<TransactionEntrypoint>>,
    native_identity: Option<Hash>,
    axt_counters: Hash,
    axt_transitions: BTreeSet<DataSpaceId>,
    axt_ratchets_finalized: bool,
    fee_receipt_sources: BTreeSet<[u8; 32]>,
    slash_observability: Hash,
    #[cfg(feature = "telemetry")]
    parliament_observability: Hash,
    authenticated_replay: bool,
    replay_prevalidation: bool,
}

/// Retained identity compares allocations, never the contents of a shared owner.
struct PublicationIdentity<T>(Arc<T>);

impl<T> std::fmt::Debug for PublicationIdentity<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PublicationIdentity")
    }
}

impl<T> PartialEq for PublicationIdentity<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Original immutable hash-log prefix plus this block's exact appended suffix.
#[derive(Debug, PartialEq)]
struct BlockHashSurface {
    owner: PublicationIdentity<BlockHashOwner>,
    publication: PublicationIdentity<BlockHashPublication>,
    mode: mv::BlockMode,
    visible_len: usize,
    pending: Vec<HashOf<BlockHeader>>,
}

impl BlockHashSurface {
    fn capture(block: &BlockHashesBlock<'_>, expected: &BlockHashes) -> Result<Self, String> {
        // The owner only exposes appends. Its original read guard freezes the
        // historical prefix, so checking the appended suffix is sufficient.
        if !Arc::ptr_eq(&block.owner, &expected.owner)
            || block.guard.is_none()
            || block.visible.get(block.visible_len..) != Some(block.pending.as_slice())
        {
            return Err(
                "publication hash journal lost its original prefix or pending suffix".into(),
            );
        }
        Ok(Self {
            owner: PublicationIdentity(Arc::clone(&block.owner)),
            publication: PublicationIdentity(Arc::clone(&block.publication)),
            mode: block.mode,
            visible_len: block.visible_len,
            pending: block.pending.clone(),
        })
    }
}

/// All inputs to the deferred catalog/geometry publication, including its undo.
#[derive(Debug, PartialEq)]
struct LifecycleSurface {
    previous_catalog: LaneCatalog,
    previous_dataspaces: DataSpaceCatalog,
    updated_dataspaces: DataSpaceCatalog,
    previous_routing: LaneRoutingPolicy,
    previous_autoscale: Vec<u64>,
    updated_catalog: LaneCatalog,
    previous_lane_config: iroha_config::parameters::actual::LaneConfig,
    updated_lane_config: iroha_config::parameters::actual::LaneConfig,
    previous_incarnations: BTreeMap<LaneId, Hash>,
    updated_incarnations: BTreeMap<LaneId, Hash>,
    previous_lineage: BTreeMap<LaneId, LaneIncarnationLineage>,
    updated_lineage: BTreeMap<LaneId, LaneIncarnationLineage>,
    previous_activation_heights: BTreeMap<LaneId, u64>,
    updated_activation_heights: BTreeMap<LaneId, u64>,
    lanes_to_reset: BTreeSet<LaneId>,
    replaced_lanes: BTreeSet<LaneId>,
    manifests: [u8; 32],
    plan: Hash,
    transition: PendingAutoscaleTransition,
    transition_height: u64,
    incarnation_root: Hash,
    runtime_catalog: Option<Hash>,
}

impl LifecycleSurface {
    fn capture(pending: &PendingAutoscaleLaneLifecycle) -> Result<Self, String> {
        // Exhaustive destructuring makes a new publication input a compile-time
        // obligation, rather than silently omitting it from this local seal.
        let PendingAutoscaleLaneLifecycle {
            catalog_update,
            updated_lane_manifests,
            plan,
            transition,
            transition_height,
            expected_incarnation_root,
            runtime_catalog,
        } = pending;
        let LaneLifecycleCatalogUpdate {
            previous_catalog,
            previous_dataspace_catalog,
            updated_dataspace_catalog,
            previous_routing_policy,
            previous_autoscale,
            updated_catalog,
            previous_lane_config,
            updated_lane_config,
            previous_lane_incarnations,
            updated_lane_incarnations,
            previous_lane_incarnation_lineage,
            updated_lane_incarnation_lineage,
            previous_lane_incarnation_activation_heights,
            updated_lane_incarnation_activation_heights,
            lanes_to_reset,
            replaced_lane_ids,
        } = catalog_update;
        let iroha_config::parameters::actual::Autoscale {
            enabled,
            min_lane_id,
            max_lane_id_exclusive,
            target_block_ms,
            scale_out_latency_ratio,
            scale_in_latency_ratio,
            scale_out_utilization_ratio,
            scale_in_utilization_ratio,
            scale_out_window_blocks,
            scale_in_window_blocks,
            cooldown_blocks,
            per_lane_target_tps,
            last_transition_height,
        } = previous_autoscale;
        Ok(Self {
            previous_catalog: previous_catalog.clone(),
            previous_dataspaces: previous_dataspace_catalog.clone(),
            updated_dataspaces: updated_dataspace_catalog.clone(),
            previous_routing: previous_routing_policy.clone(),
            previous_autoscale: vec![
                u64::from(*enabled),
                u64::from(min_lane_id.get()),
                u64::from(max_lane_id_exclusive.get()),
                target_block_ms.get(),
                scale_out_latency_ratio.to_bits(),
                scale_in_latency_ratio.to_bits(),
                scale_out_utilization_ratio.to_bits(),
                scale_in_utilization_ratio.to_bits(),
                u64::from(scale_out_window_blocks.get()),
                u64::from(scale_in_window_blocks.get()),
                u64::from(cooldown_blocks.get()),
                u64::from(per_lane_target_tps.get()),
                *last_transition_height,
            ],
            updated_catalog: updated_catalog.clone(),
            previous_lane_config: previous_lane_config.clone(),
            updated_lane_config: updated_lane_config.clone(),
            previous_incarnations: previous_lane_incarnations.clone(),
            updated_incarnations: updated_lane_incarnations.clone(),
            previous_lineage: previous_lane_incarnation_lineage.clone(),
            updated_lineage: updated_lane_incarnation_lineage.clone(),
            previous_activation_heights: previous_lane_incarnation_activation_heights.clone(),
            updated_activation_heights: updated_lane_incarnation_activation_heights.clone(),
            lanes_to_reset: lanes_to_reset.clone(),
            replaced_lanes: replaced_lane_ids.clone(),
            manifests: updated_lane_manifests.consensus_policy_digest(),
            plan: hash_value(plan)?,
            transition: transition.clone(),
            transition_height: *transition_height,
            incarnation_root: *expected_incarnation_root,
            runtime_catalog: runtime_catalog.as_ref().map(hash_value).transpose()?,
        })
    }
}

impl FinalizedPublicationSurface {
    /// Capture actual journals and deferred effects before or after metadata.
    /// Event bytes remain separate because they are outside the World journals.
    pub(in crate::state) fn capture(block: &StateBlock<'_>) -> Result<Self, String> {
        let expected = block.state_ref;
        let mode = block.block_hashes.mode;
        if !block.transactions.belongs_to(&expected.transactions)
            || block.transactions.mode() != mode
        {
            return Err("publication membership journal has foreign owner or mode".into());
        }
        macro_rules! state_journal {
            ($field:ident) => {{
                if !block.$field.belongs_to(&expected.$field) || block.$field.mode() != mode {
                    return Err(concat!(
                        "publication State journal has foreign owner or mode: ",
                        stringify!($field)
                    )
                    .into());
                }
                block.$field.publication_identity()
            }};
        }
        let state_journals = [
            state_journal!(canonical_runtime),
            state_journal!(commit_topology),
            state_journal!(prev_commit_topology),
            state_journal!(lane_consensus_contexts),
        ];
        let world_journals = block.world.publication_identities(&expected.world, mode)?;
        let da_commitments = block
            .pending_da_commitments
            .as_ref()
            .map(|pending| {
                let PendingDaCommitmentBundle {
                    block_height,
                    bundle,
                } = pending;
                Ok::<_, String>((*block_height, hash_value(bundle)?))
            })
            .transpose()?;
        let da_pins = block
            .pending_da_pin_intents
            .as_ref()
            .map(|pending| {
                let PendingDaPinIntentBundle {
                    block_height,
                    intents,
                    quota_writes,
                } = pending;
                Ok::<_, String>((
                    *block_height,
                    hash_value(intents)?,
                    hash_value(quota_writes)?,
                ))
            })
            .transpose()?;
        let mut slashes = Vec::with_capacity(block.pending_public_lane_slash_observability.len());
        for slash in &block.pending_public_lane_slash_observability {
            let PendingPublicLaneSlashObservability {
                lane_id,
                #[cfg(feature = "telemetry")]
                previous_status,
                #[cfg(feature = "telemetry")]
                slashed_status,
                bonded_amount,
                pending_unbond_amount,
            } = slash;
            let parts = vec![
                hash_value(lane_id)?,
                hash_value(bonded_amount)?,
                hash_value(pending_unbond_amount)?,
            ];
            #[cfg(feature = "telemetry")]
            let parts = [
                parts,
                vec![hash_value(previous_status)?, hash_value(slashed_status)?],
            ]
            .concat();
            slashes.push(hash_value(&parts)?);
        }
        Ok(Self {
            chain_id: block.chain_id.clone(),
            network_id: block.network_id,
            transactions: block.transactions.publication_surface(),
            block_hashes: BlockHashSurface::capture(&block.block_hashes, &expected.block_hashes)?,
            runtime: block.canonical_runtime.get().clone(),
            runtime_preimage: block
                .canonical_runtime
                .touched_value()
                .map(|value| value.before.clone()),
            state_journals,
            state_preimages: [
                block
                    .commit_topology
                    .touched_value()
                    .map(|value| hash_value(value.before))
                    .transpose()?,
                block
                    .prev_commit_topology
                    .touched_value()
                    .map(|value| hash_value(value.before))
                    .transpose()?,
                block
                    .lane_consensus_contexts
                    .touched_value()
                    .map(|value| value.before.canonical_hash())
                    .transpose()
                    .map_err(|error| error.to_string())?,
            ],
            lane_contexts: block
                .lane_consensus_contexts
                .get()
                .canonical_hash()
                .map_err(|error| error.to_string())?,
            lane_contexts_seal: block.lane_consensus_contexts_seal,
            world: block.world.publication_state_delta()?,
            world_journals,
            world_dataspaces: block.world.dataspace_catalog.clone(),
            external_events: hash_value(&block.world.external_event_buf)?,
            header: block._curr_block,
            previous_topology: block.prev_commit_topology.get().clone(),
            next_topology: block.commit_topology.get().clone(),
            execution_policy: compute_execution_policy_digest_v1(
                &block.pipeline,
                &block.oracle,
                &block.crypto,
                &block.nexus,
                &block.lane_manifests,
                block.lane_compliance.as_deref(),
                &block.fraud_monitoring,
                &block.zk,
                &block.gov,
                &block.content,
                &block.settlement,
            )
            .map_err(|error| error.to_string())?,
            lane_catalog: block.nexus.lane_catalog.clone(),
            dataspaces: block.nexus.dataspace_catalog.clone(),
            lane_config: block.nexus.lane_config.clone(),
            routing: block.nexus.routing_policy.clone(),
            last_autoscale_transition: block.nexus.autoscale.last_transition_height,
            lane_incarnations: block.lane_incarnations.clone(),
            lane_lineage: block.lane_incarnation_lineage.clone(),
            lane_activation_heights: block.lane_incarnation_activation_heights.clone(),
            manifests: block.lane_manifests.consensus_policy_digest(),
            privacy: Arc::clone(&block.lane_privacy_registry),
            sccp_registry: block.sccp_registry.policy_hash(),
            da_commitments,
            da_pins,
            lifecycle: block
                .pending_autoscale_lifecycle
                .as_ref()
                .map(LifecycleSurface::capture)
                .transpose()?,
            sample_history: block.autoscale_sample_history.clone(),
            sample_history_dirty: block.autoscale_sample_history_dirty,
            evaluated_fragment_count: block.autoscale_evaluated_committed_fragment_count,
            relay_records: hash_value(&block.verified_lane_relay_records)?,
            merge_entry: block
                .staged_merge_entry
                .as_ref()
                .map(MergeLedgerEntry::canonical_hash),
            merge_entrypoints: block.merge_carrier_entrypoints.iter().copied().collect(),
            native_identity: block.native_output_publication_identity()?,
            axt_counters: hash_value(&block.axt_next_handle_counters)?,
            axt_transitions: block.axt_authorization_transitioned.clone(),
            axt_ratchets_finalized: block.axt_policy_transition_ratchets_finalized,
            fee_receipt_sources: block.pending_nexus_fee_receipt_source_ids.clone(),
            slash_observability: hash_value(&slashes)?,
            #[cfg(feature = "telemetry")]
            parliament_observability: hash_value(&block.pending_parliament_telemetry_events)?,
            authenticated_replay: block.authenticated_replay_commit,
            replay_prevalidation: block.replay_prevalidation,
        })
    }

    /// Reject any change to the original prepared journals or deferred effects.
    pub(in crate::state) fn verify(&self, block: &StateBlock<'_>) -> Result<(), String> {
        if *self != Self::capture(block)? {
            return Err("finalized execution publication surface changed".into());
        }
        Ok(())
    }
}

impl StateBlock<'_> {
    /// Complete idempotent deterministic tails before retaining the final seal.
    /// No Kura I/O, publication lock, or permission to publish is acquired here.
    pub(in crate::state) fn prepare_finalized_publication_surface(
        &mut self,
    ) -> Result<FinalizedPublicationSurface, String> {
        if !self.world.external_event_buf.is_empty() {
            return Err("finalized publication retained undelivered external events".into());
        }
        self.finalize_axt_asset_incarnations()?;
        self.finalize_axt_policy_transition_ratchets()
            .map_err(|error| error.to_string())?;
        if !self
            .staged_merge_entry
            .as_ref()
            .is_some_and(|entry| entry.execution_batch.is_some())
        {
            self.prune_axt_replay_ledger(
                current_axt_slot_from_block(&self._curr_block, self.nexus.axt.slot_length_ms),
                self.nexus.axt.replay_retention_slots.get(),
            );
        }
        // This is also the preview used before replay's checkpoint comparison.
        // Repeating it later cannot mutate the retained net publication surface.
        self.prepare_replay_checkpoint_preview();
        FinalizedPublicationSurface::capture(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        )
    }

    fn header() -> BlockHeader {
        BlockHeader::new(std::num::NonZeroU64::MIN, None, None, 1, 0)
    }

    #[test]
    fn final_publication_surface_binds_topologies_omitted_from_checkpoint() {
        let state = state();
        let mut block = state.merge_preexecution_block(header());
        let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(&block);
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        surface.verify(&block).unwrap();
        block.commit_topology.get_mut().push(PeerId::new(
            iroha_test_samples::ALICE_KEYPAIR.public_key().clone(),
        ));
        assert_eq!(
            checkpoint,
            crate::snapshot::canonical_staged_state_snapshot_hash(&block)
        );
        assert!(surface.verify(&block).is_err());
        block.commit_topology.get_mut().clear();
        assert!(surface.verify(&block).is_err());
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        block.prev_commit_topology.get_mut().push(PeerId::new(
            iroha_test_samples::ALICE_KEYPAIR.public_key().clone(),
        ));
        assert!(surface.verify(&block).is_err());
    }

    #[test]
    fn final_publication_surface_binds_deferred_da_and_replay_authority() {
        let state = state();
        let mut block = state.merge_preexecution_block(header());
        let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(&block);
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        block.pending_da_commitments = Some(PendingDaCommitmentBundle {
            block_height: 1,
            bundle: iroha_data_model::da::commitment::DaCommitmentBundle::new(Vec::new()),
        });
        assert_eq!(
            checkpoint,
            crate::snapshot::canonical_staged_state_snapshot_hash(&block)
        );
        assert!(surface.verify(&block).is_err());
        block.pending_da_commitments = None;
        surface.verify(&block).unwrap();
        block.authenticated_replay_commit = true;
        assert!(surface.verify(&block).is_err());
    }

    #[test]
    fn final_publication_surface_rejects_new_delivery_events() {
        let state = state();
        let mut block = state.merge_preexecution_block(header());
        let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(&block);
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        block.world.external_event_buf.push(
            BlockEvent {
                header: header(),
                status: BlockStatus::Applied,
            }
            .into(),
        );
        assert_eq!(
            checkpoint,
            crate::snapshot::canonical_staged_state_snapshot_hash(&block)
        );
        assert!(surface.verify(&block).is_err());
    }

    #[test]
    fn final_publication_surface_binds_projection_history_and_activation_inputs() {
        let state = state();
        let mut block = state.merge_preexecution_block(header());
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        block.autoscale_sample_history_dirty = true;
        assert!(surface.verify(&block).is_err());
        block.autoscale_sample_history_dirty = false;
        surface.verify(&block).unwrap();
        block
            .lane_incarnation_activation_heights
            .insert(LaneId::SINGLE, 4);
        assert!(surface.verify(&block).is_err());
    }

    #[test]
    fn final_publication_surface_binds_runtime_and_pending_journals() {
        let state = state();
        let mut block = state.merge_preexecution_block(header());
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        let version = block.canonical_runtime.get().version;
        block.canonical_runtime.get_mut().version = version.wrapping_add(1);
        assert!(surface.verify(&block).is_err());
        block.canonical_runtime.get_mut().version = version;
        assert!(surface.verify(&block).is_err());
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        block.block_hashes.push(header().hash());
        assert!(surface.verify(&block).is_err());
        let with_hash = FinalizedPublicationSurface::capture(&block).unwrap();
        block
            .transactions
            .insert_block(HashSet::new(), NonZeroUsize::MIN);
        assert!(with_hash.verify(&block).is_err());
    }

    #[test]
    fn final_publication_surface_binds_noop_undo_changes_omitted_from_net_delta() {
        let state = state();
        let mut block = state.merge_preexecution_block(header());
        let semantic_delta = block.world.net_state_delta().unwrap();
        let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(&block);
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        block
            .world
            .smart_contract_state
            .remove("publication/absent".parse::<StatePath>().unwrap());
        assert_eq!(semantic_delta, block.world.net_state_delta().unwrap());
        assert_ne!(
            checkpoint,
            crate::snapshot::canonical_staged_state_snapshot_hash(&block)
        );
        assert!(surface.verify(&block).is_err());
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        let _ = block.world.soradns_last_publish_ms.get_mut();
        assert_eq!(semantic_delta, block.world.net_state_delta().unwrap());
        assert!(surface.verify(&block).is_err());
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        let _ = block.lane_consensus_contexts.get_mut();
        assert!(surface.verify(&block).is_err());
    }

    #[test]
    fn final_publication_surface_rejects_foreign_original_journals() {
        let state = state();
        let foreign_state = self::state();
        let block = state.merge_preexecution_block(header());
        let foreign_block = foreign_state.merge_preexecution_block(header());
        assert_eq!(
            crate::snapshot::canonical_staged_state_snapshot_hash(&block),
            crate::snapshot::canonical_staged_state_snapshot_hash(&foreign_block),
        );
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        assert!(surface.verify(&foreign_block).is_err());
    }

    #[test]
    fn final_publication_surface_rejects_identical_foreign_world_journals_at_first_capture() {
        let state = state();
        let foreign_state = self::state();
        let mut block = state.merge_preexecution_block(header());
        let mut foreign_block = foreign_state.merge_preexecution_block(header());
        let semantic_delta = block.world.net_state_delta().unwrap();
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        macro_rules! reject_foreign {
            ($field:ident) => {
                std::mem::swap(&mut block.world.$field, &mut foreign_block.world.$field);
                assert_eq!(semantic_delta, block.world.net_state_delta().unwrap());
                assert!(FinalizedPublicationSurface::capture(&block).is_err());
                assert!(surface.verify(&block).is_err());
                std::mem::swap(&mut block.world.$field, &mut foreign_block.world.$field);
                surface.verify(&block).unwrap();
            };
        }
        reject_foreign!(parameters);
        reject_foreign!(smart_contract_state);
        reject_foreign!(triggers);
    }

    #[test]
    fn final_publication_surface_rejects_identical_foreign_state_journals_at_first_capture() {
        let state = state();
        let foreign_state = self::state();
        let mut block = state.merge_preexecution_block(header());
        let mut foreign_block = foreign_state.merge_preexecution_block(header());
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        macro_rules! reject_foreign {
            ($field:ident) => {
                std::mem::swap(&mut block.$field, &mut foreign_block.$field);
                assert!(FinalizedPublicationSurface::capture(&block).is_err());
                assert!(surface.verify(&block).is_err());
                std::mem::swap(&mut block.$field, &mut foreign_block.$field);
                surface.verify(&block).unwrap();
            };
        }
        reject_foreign!(canonical_runtime);
        reject_foreign!(commit_topology);
        reject_foreign!(prev_commit_topology);
        reject_foreign!(lane_consensus_contexts);
        reject_foreign!(transactions);
        reject_foreign!(block_hashes);
    }

    #[test]
    fn final_publication_surface_rejects_changed_world_predecessor_and_mixed_modes() {
        let state = state();
        let foreign_state = self::state();
        let mut block = state.merge_preexecution_block(header());
        let semantic_delta = block.world.net_state_delta().unwrap();
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        let original = std::mem::replace(
            &mut block.world.smart_contract_state,
            foreign_state.world.smart_contract_state.block(),
        );
        original.commit();
        block.world.smart_contract_state = state.world.smart_contract_state.block();
        assert_eq!(semantic_delta, block.world.net_state_delta().unwrap());
        assert!(surface.verify(&block).is_err());
        let ordinary = std::mem::replace(
            &mut block.world.smart_contract_state,
            foreign_state.world.smart_contract_state.block(),
        );
        drop(ordinary);
        block.world.smart_contract_state = state.world.smart_contract_state.block_and_revert();
        assert!(FinalizedPublicationSurface::capture(&block).is_err());
    }

    #[test]
    fn hash_publication_surface_rejects_detached_or_inconsistent_suffix() {
        let hashes = BlockHashes::default();
        let mut block = hashes.block();
        let empty = BlockHashSurface::capture(&block, &hashes).unwrap();
        block.push(header().hash());
        assert_ne!(empty, BlockHashSurface::capture(&block, &hashes).unwrap());
        block.visible.pop();
        assert!(BlockHashSurface::capture(&block, &hashes).is_err());
        block.visible.push(header().hash());
        BlockHashSurface::capture(&block, &hashes).unwrap();
        block.prepare_commit();
        assert!(BlockHashSurface::capture(&block, &hashes).is_err());
    }

    #[test]
    fn final_publication_surface_binds_deferred_geometry_inputs() {
        let state = state();
        let mut block = state.merge_preexecution_block(header());
        // A no-op deferred plan isolates the local authorization surface from
        // replicated World changes. This never grants lifecycle publication.
        block.pending_autoscale_lifecycle = Some(PendingAutoscaleLaneLifecycle {
            catalog_update: LaneLifecycleCatalogUpdate {
                previous_catalog: block.nexus.lane_catalog.clone(),
                previous_dataspace_catalog: block.nexus.dataspace_catalog.clone(),
                updated_dataspace_catalog: block.nexus.dataspace_catalog.clone(),
                previous_routing_policy: block.nexus.routing_policy.clone(),
                previous_autoscale: block.nexus.autoscale,
                updated_catalog: block.nexus.lane_catalog.clone(),
                previous_lane_config: block.nexus.lane_config.clone(),
                updated_lane_config: block.nexus.lane_config.clone(),
                previous_lane_incarnations: block.lane_incarnations.clone(),
                updated_lane_incarnations: block.lane_incarnations.clone(),
                previous_lane_incarnation_lineage: block.lane_incarnation_lineage.clone(),
                updated_lane_incarnation_lineage: block.lane_incarnation_lineage.clone(),
                previous_lane_incarnation_activation_heights: block
                    .lane_incarnation_activation_heights
                    .clone(),
                updated_lane_incarnation_activation_heights: block
                    .lane_incarnation_activation_heights
                    .clone(),
                lanes_to_reset: BTreeSet::new(),
                replaced_lane_ids: BTreeSet::new(),
            },
            updated_lane_manifests: Arc::clone(&block.lane_manifests),
            plan: iroha_data_model::nexus::LaneLifecyclePlan {
                additions: Vec::new(),
                retire: Vec::new(),
            },
            transition: PendingAutoscaleTransition::Manual,
            transition_height: 1,
            expected_incarnation_root: Hash::new(b"deferred-geometry-test"),
            runtime_catalog: None,
        });
        let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(&block);
        let surface = FinalizedPublicationSurface::capture(&block).unwrap();
        surface.verify(&block).unwrap();
        block
            .pending_autoscale_lifecycle
            .as_mut()
            .unwrap()
            .transition_height = 2;
        assert_eq!(
            checkpoint,
            crate::snapshot::canonical_staged_state_snapshot_hash(&block)
        );
        assert!(surface.verify(&block).is_err());
    }

    #[test]
    fn final_publication_preparation_is_idempotent_and_requires_drained_events() {
        let state = state();
        let mut block = state.merge_preexecution_block(header());
        let first = block.prepare_finalized_publication_surface().unwrap();
        first.verify(&block).unwrap();
        let repeated = block.prepare_finalized_publication_surface().unwrap();
        assert_eq!(first, repeated);
        block.world.external_event_buf.push(
            BlockEvent {
                header: header(),
                status: BlockStatus::Applied,
            }
            .into(),
        );
        assert!(block.prepare_finalized_publication_surface().is_err());
    }
}
