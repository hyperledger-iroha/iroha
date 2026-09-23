//! One original-owner construction kernel for ordinary, scratch, and replacement State blocks.

use super::*;

impl State {
    /// Prepare metadata while the exact acquired writers remain jointly armed.
    /// The finish continuation receives an already armed executing owner.
    pub(super) fn construct_acquired_block<'state, R>(
        &'state self,
        acquired: canonical_runtime::AcquiredRuntimeBlock<'state>,
        curr_block: BlockHeader,
        finish: impl FnOnce(StateBlock<'state>) -> R,
    ) -> R {
        // Initialized metadata is destroyed after the original acquisition on
        // preparation unwind. No additional metadata box/collection is needed.
        let mut finish = Some(finish);
        let mut gas_limit_per_block;
        let mut privacy_budget_in_block;
        let mut runtime_policy;
        let mut pipeline_ivm_prepared_cache;
        let mut query_ledger_time_ms;
        let mut soracloud_runtime;
        let mut accounts_snapshot_cache;
        let mut pipeline;
        let mut oracle;
        let mut crypto;
        let mut lane_compliance;
        let mut fraud_monitoring;
        let mut zk;
        let mut gov;
        let mut content;
        let mut settlement;
        let mut kagemusha_v1_runtime_verifier;
        let mut settlement_engine;
        let mut chain_id;
        let mut settlement_accumulator;
        let mut fastpq_transcripts;
        let mut fastpq_entry_dataspaces;
        let mut fastpq_source_captures;
        let mut axt_envelopes;
        let mut axt_next_handle_counters;
        let mut axt_authorization_transitioned;
        let mut batch_transfer_outcomes;
        let mut verified_lane_relay_records;
        let mut touched_lanes;
        let mut merge_carrier_entrypoints;
        let mut staged_queue_plan_admissions;
        let mut pending_nexus_fee_receipt_source_ids;
        #[cfg(feature = "telemetry")]
        let mut pending_parliament_telemetry_events;
        let mut pending_public_lane_slash_observability;
        let mut sccp_verifier_work_in_block;
        #[cfg(feature = "zk-preverify")]
        let mut zk_dedup;
        let mut original = Some(acquired);
        let acquired = original
            .as_ref()
            .expect("original acquired State block")
            .fields();
        gas_limit_per_block = Some(gas_limit_from_parameters(acquired.world.parameters()));
        privacy_budget_in_block = Some(
            crate::privacy::PrivacyBlockBudgetV1::new(
                acquired.world.privacy_consensus_policy.get().current_limits,
            )
            .expect("persisted privacy consensus policy was validated before block construction"),
        );
        runtime_policy = Some(canonical_runtime::CapturedRuntimePolicy::capture(
            &acquired.projection,
            &self.zk,
            self.lane_compliance.read().clone(),
        ));
        pipeline_ivm_prepared_cache = Some(self.pipeline_ivm_prepared_cache.read().clone());
        query_ledger_time_ms = Some(Some(
            u64::try_from(curr_block.creation_time().as_millis()).unwrap_or(u64::MAX),
        ));
        soracloud_runtime = Some(self.soracloud_runtime());
        accounts_snapshot_cache = Some(SyncOnceCell::new());
        pipeline = Some(self.pipeline.clone());
        oracle = Some(self.oracle.clone());
        crypto = Some(self.crypto());
        lane_compliance = Some(
            runtime_policy
                .as_ref()
                .expect("prepared runtime policy")
                .compliance
                .clone(),
        );
        fraud_monitoring = Some(self.fraud_monitoring.clone());
        zk = Some(self.zk.clone());
        gov = Some(self.gov.clone());
        content = Some(self.content.clone());
        settlement = Some(self.settlement.clone());
        kagemusha_v1_runtime_verifier = Some(Arc::clone(&self.kagemusha_v1_runtime_verifier));
        settlement_engine = Some(self.settlement_engine.clone());
        chain_id = Some(self.chain_id.clone());
        settlement_accumulator = Some(crate::settlement::SettlementAccumulator::default());
        fastpq_transcripts = Some(BTreeMap::new());
        fastpq_entry_dataspaces = Some(BTreeMap::new());
        fastpq_source_captures = Some(crate::fastpq::FastpqSourceCaptureAccumulator::default());
        axt_envelopes = Some(Vec::new());
        axt_next_handle_counters = Some(BTreeMap::new());
        axt_authorization_transitioned = Some(BTreeSet::new());
        batch_transfer_outcomes = Some(BTreeMap::new());
        verified_lane_relay_records = Some(Vec::new());
        touched_lanes = Some(BTreeSet::new());
        merge_carrier_entrypoints = Some(HashSet::new());
        staged_queue_plan_admissions = Some(Vec::new());
        pending_nexus_fee_receipt_source_ids = Some(BTreeSet::new());
        #[cfg(feature = "telemetry")]
        {
            pending_parliament_telemetry_events = Some(Vec::new());
        }
        pending_public_lane_slash_observability = Some(Vec::new());
        sccp_verifier_work_in_block = Some(SccpVerifierWorkV1::default());
        #[cfg(feature = "zk-preverify")]
        {
            zk_dedup = Some(Default::default());
        }
        // Every take below is checked while the joint owner is still armed.
        // The finishing closure borrows each Option, including metadata and the
        // caller continuation, rather than moving cleanup into its environment.
        assert!(original.is_some() && finish.is_some());
        assert!(
            gas_limit_per_block.is_some()
                && privacy_budget_in_block.is_some()
                && runtime_policy.is_some()
        );
        assert!(pipeline_ivm_prepared_cache.is_some());
        assert!(query_ledger_time_ms.is_some());
        assert!(soracloud_runtime.is_some());
        assert!(accounts_snapshot_cache.is_some());
        assert!(pipeline.is_some());
        assert!(oracle.is_some());
        assert!(crypto.is_some());
        assert!(lane_compliance.is_some());
        assert!(fraud_monitoring.is_some());
        assert!(zk.is_some());
        assert!(gov.is_some());
        assert!(content.is_some());
        assert!(settlement.is_some());
        assert!(kagemusha_v1_runtime_verifier.is_some());
        assert!(settlement_engine.is_some());
        assert!(chain_id.is_some());
        assert!(settlement_accumulator.is_some());
        assert!(fastpq_transcripts.is_some());
        assert!(fastpq_entry_dataspaces.is_some());
        assert!(fastpq_source_captures.is_some());
        assert!(axt_envelopes.is_some());
        assert!(axt_next_handle_counters.is_some());
        assert!(axt_authorization_transitioned.is_some());
        assert!(batch_transfer_outcomes.is_some());
        assert!(verified_lane_relay_records.is_some());
        assert!(touched_lanes.is_some());
        assert!(merge_carrier_entrypoints.is_some());
        assert!(staged_queue_plan_admissions.is_some());
        assert!(pending_nexus_fee_receipt_source_ids.is_some());
        #[cfg(feature = "telemetry")]
        assert!(pending_parliament_telemetry_events.is_some());
        assert!(pending_public_lane_slash_observability.is_some());
        assert!(sccp_verifier_work_in_block.is_some());
        #[cfg(feature = "zk-preverify")]
        assert!(zk_dedup.is_some());
        // No World-sized owner is copied into a closure payload. Its only
        // arbitrary caller operation receives an already armed StateBlock.
        finish_state_block_construction(|| {
            let canonical_runtime::AcquiredRuntimeBlockFields {
                world,
                transactions,
                commit_topology,
                prev_commit_topology,
                lane_consensus_contexts,
                canonical_runtime,
                projection,
                sccp_registry,
                block_hashes,
                da_rewind_releases,
            } = original
                .take()
                .expect("original acquired State block")
                .into_fields();
            let block = StateBlock::from_fields(StateBlockFields {
                state_ref: self,
                read_releases: StateViewReleases::new(self),
                da_rewind_releases,
                canonical_runtime: block_field::BlockField::new(canonical_runtime),
                block_hashes: block_hash_field::BlockHashField::new(block_hashes),
                world,
                merge_ledger: &self.merge_ledger,
                transactions: storage_transactions::TransactionsBlockField::new(transactions),
                commit_topology: block_field::BlockField::new(commit_topology),
                prev_commit_topology: block_field::BlockField::new(prev_commit_topology),
                lane_consensus_contexts: block_field::BlockField::new(lane_consensus_contexts),
                lane_consensus_contexts_seal: None,
                ivm: &self.ivm,
                pipeline_ivm_prepared_cache: pipeline_ivm_prepared_cache
                    .take()
                    .expect("prepared State input"),
                kura: &self.kura,
                query_handle: &self.query_handle,
                query_ledger_time_ms: query_ledger_time_ms.take().expect("prepared State input"),
                soracloud_runtime: soracloud_runtime.take().expect("prepared State input"),
                accounts_snapshot_cache: accounts_snapshot_cache
                    .take()
                    .expect("prepared State input"),
                pipeline: pipeline.take().expect("prepared State input"),
                oracle: oracle.take().expect("prepared State input"),
                crypto: crypto.take().expect("prepared State input"),
                nexus: projection.nexus,
                lane_incarnations: projection.incarnations,
                lane_incarnation_lineage: projection.lineage,
                lane_incarnation_activation_heights: projection.activation_heights,
                lane_manifests: projection.manifests,
                lane_privacy_registry: projection.privacy,
                lane_compliance: lane_compliance.take().expect("prepared State input"),
                runtime_policy: runtime_policy.take().expect("prepared State input"),
                fraud_monitoring: fraud_monitoring.take().expect("prepared State input"),
                zk: zk.take().expect("prepared State input"),
                sccp_registry,
                gov: gov.take().expect("prepared State input"),
                content: content.take().expect("prepared State input"),
                settlement: settlement.take().expect("prepared State input"),
                kagemusha_v1_runtime_verifier: kagemusha_v1_runtime_verifier
                    .take()
                    .expect("prepared State input"),
                settlement_engine: settlement_engine.take().expect("prepared State input"),
                chain_id: chain_id.take().expect("prepared State input"),
                network_id: self.network_id,
                settlement_accumulator: settlement_accumulator
                    .take()
                    .expect("prepared State input"),
                fastpq_transcripts: fastpq_transcripts.take().expect("prepared State input"),
                fastpq_tx_set_hash: None,
                fastpq_entry_dataspaces: fastpq_entry_dataspaces
                    .take()
                    .expect("prepared State input"),
                fastpq_source_context: None,
                fastpq_source_captures: fastpq_source_captures
                    .take()
                    .expect("prepared State input"),
                fastpq_source_inventory: None,
                fastpq_witness_context: None,
                axt_envelopes: axt_envelopes.take().expect("prepared State input"),
                axt_block_start_snapshot: None,
                axt_next_handle_counters: axt_next_handle_counters
                    .take()
                    .expect("prepared State input"),
                axt_authorization_transitioned: axt_authorization_transitioned
                    .take()
                    .expect("prepared State input"),
                batch_transfer_outcomes: batch_transfer_outcomes
                    .take()
                    .expect("prepared State input"),
                verified_lane_relay_records: verified_lane_relay_records
                    .take()
                    .expect("prepared State input"),
                touched_lanes: touched_lanes.take().expect("prepared State input"),
                pending_da_commitments: None,
                pending_da_pin_intents: None,
                pending_autoscale_lifecycle: None,
                autoscale_sample_history: projection.samples,
                autoscale_sample_history_dirty: false,
                autoscale_evaluated_committed_fragment_count: None,
                autoscale_lifecycle_evaluated: false,
                merge_carrier_entrypoints: merge_carrier_entrypoints
                    .take()
                    .expect("prepared State input"),
                staged_merge_entry: None,
                native_lane_stage: None,
                staged_queue_plan_admissions: staged_queue_plan_admissions
                    .take()
                    .expect("prepared State input"),
                canonical_wsv_merge_commit_authorization: None,
                canonical_carrier_commit_metadata_authorization: None,
                pending_nexus_fee_receipt_source_ids: pending_nexus_fee_receipt_source_ids
                    .take()
                    .expect("prepared State input"),
                start_of_block_effects_applied: false,
                applied_npos_consensus_effects_hash: None,
                axt_policy_transition_ratchets_finalized: false,
                exec_witness: None,
                parliament_timed_ovn_casting_bindings: None,
                #[cfg(feature = "telemetry")]
                pending_parliament_telemetry_events: pending_parliament_telemetry_events
                    .take()
                    .expect("prepared State input"),
                pending_public_lane_slash_observability: pending_public_lane_slash_observability
                    .take()
                    .expect("prepared State input"),
                gas_used_in_block: 0,
                confidential_gas_used_in_block: 0,
                zk_confidential_ops_in_block: 0,
                zk_verify_calls_in_block: 0,
                zk_proof_bytes_in_block: 0,
                sccp_verifier_work_in_block: sccp_verifier_work_in_block
                    .take()
                    .expect("prepared State input"),
                privacy_budget_in_block: privacy_budget_in_block
                    .take()
                    .expect("prepared State input"),
                implicit_account_creations_in_block: 0,
                gas_limit_per_block: gas_limit_per_block.take().expect("prepared State input"),
                frozen_execution_output_capacity: None,
                execution_output_plan: None,
                #[cfg(feature = "telemetry")]
                telemetry: &self.telemetry,
                state_write_lock: &self.state_write_lock,
                da_commitments: &self.da_commitments,
                da_receipt_cursors: &self.da_receipt_cursors,
                da_shard_cursors: &self.da_shard_cursors,
                da_pin_intents: &self.da_pin_intents,
                _curr_block: curr_block,
                #[cfg(feature = "zk-preverify")]
                zk_dedup: zk_dedup.take().expect("prepared State input"),
                committed_fragments: 0,
                authenticated_replay_commit: false,
                replay_prevalidation: false,
            });
            finish.take().expect("original State finish continuation")(block)
        })
    }
}

// Do not overlap final State field-move temporaries with the metadata preparation
// frame. The original acquisition stays in the caller until the inert transfer.
#[inline(never)]
fn finish_state_block_construction<R>(finish: impl FnOnce() -> R) -> R {
    finish()
}
