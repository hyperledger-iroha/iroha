"""Exact prepared execution and separate Native capacity ownership.

These bindings describe existing execution-prefix preparation and disk accounting.
They do not claim a complete prepared State publisher, pre-vote descriptor admission,
local-resource deferral, or historical Native target authority.
"""
from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable

from sumeragi_v2_multilane_geometry_evidence_contract import _code
from sumeragi_v2_multilane_reviewed_rust_source import _mask_rust_comments

MODEL = "SumeragiV2NativeApplicationEvidence"
APPLY = "crates/iroha_core/src/sumeragi/v2_apply.rs"
BLOCK = "crates/iroha_core/src/block/carrier_preparation.rs"
PREPARED = "crates/iroha_core/src/state/carrier_preparation.rs"
PREFIX = "crates/iroha_core/src/state/carrier_preparation/execution_prefix.rs"
JOURNALS = "crates/iroha_core/src/state/carrier_preparation/journals.rs"
OUTPUT = "crates/iroha_core/src/state/output_producer.rs"
SEAL = "crates/iroha_core/src/state/output_seal.rs"
TAIL = "crates/iroha_core/src/block/post_execution_tail.rs"
NATIVE_METADATA = "crates/iroha_core/src/block/native_execution_metadata.rs"
NATIVE_STAGE = "crates/iroha_core/src/state/lane_decision_batch.rs"
CONTROLS = "crates/iroha_core/src/block.rs"
NATIVE_SOURCE = "crates/iroha_core/src/state/native_lane_batch_replay.rs"
NATIVE_KERNEL = "crates/iroha_core/src/state/lane_decision_execution.rs"
NATIVE_CARRIER = "crates/iroha_core/src/block/native_lane_carrier.rs"
NATIVE_FINALIZED = "crates/iroha_core/src/kura/native_lane_batch_source.rs"
BODY_STORE = "crates/iroha_core/src/sumeragi/v2_body_store.rs"
ORDINARY = "crates/iroha_core/src/kura/lane_artifact_budget.rs"
CAPACITY = "crates/iroha_core/src/kura/native_amx_publication_capacity.rs"
DURABLE = "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs"
KURA = "crates/iroha_core/src/kura.rs"
AUTONOMOUS = "crates/iroha_core/src/kura/autonomous_terminal_capacity.rs"
AUTONOMOUS_TOKENS = ('additional_unreserved_stable_bytes: u64', 'additional_missing_terminal_identities: usize', 'additional_incomplete_terminal_identities: usize', 'allowed_view_temp: Option<&Path>', 'autonomous_global_terminal_reservation_counts_with_allowed_view_temp_locked(', 'allowed_view_temp', 'AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES', 'resulting_missing', 'resulting_incomplete', 'MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES', 'stable_terminal_reservations', 'shared_terminal_transient', 'consumes_terminal_cas_transient', 'self.lane_publication_budget_reserved_bytes()?', '.kura_disk_usage_bytes()?', 'bytes.checked_add(stable_terminal_reservations)', 'bytes.checked_add(lane_publication_reservations)', 'self.certified_bundle_capacity_reserved_bytes()?', 'bytes.checked_add(certified_bundle_reservations)', 'required > self.max_disk_usage_bytes')
CANDIDATE_TOKENS = (
    "ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block(",
    "SumeragiV2ValidationContext::from_height_context(context)",
    "prepared.native_amx_manifest()",
    "validate_native_amx_participant_application_evidence_byte_budget",
    "Ok(prepared.execution_prefix_commitment())",
)
ORDINARY_TOKENS = (
    "merge_entry: Option<&MergeLedgerEntry>",
    "self.merge_lane_application_artifact_required_bytes_for_block(block, merge_entry)?",
    "Self::maximum_index_growth_for_unresolved_sidecar_write(",
    "Self::lane_payload_ownership_is_durable(ownership)",
    "LaneBlockArtifact::new(block_hash, ownership.clone())",
    "artifact.encode_framed()?.len()",
    "Ok(total)",
)
ORDINARY_ORDERED = (
    "let mut total =",
    "self.merge_lane_application_artifact_required_bytes_for_block(block, merge_entry)?",
    "if let Some(bundle) = block.execution_context()",
    "Self::lane_payload_ownership_is_durable(ownership)",
    "artifact.encode_framed()?.len()",
    "Self::maximum_index_growth_for_unresolved_sidecar_write(",
    "Ok(total)",
)
# Recorded Native execution owns verified context and pristine controls, but
# remains disposable: none of these relations opens the live ValidBlock gate.
NATIVE_CONTROL_BINDINGS = (
    (BODY_STORE, "fn", "verify_origin_block_signature", (
        "context: &wire::HeightContext", "block: &SignedBlock", "policy: &BlockSignaturePolicy",
        "context.leader(block.header().view_change_index())", "context\n                .roster\n                .get(leader_index)",
        "let mut signatures = block.signatures()", "if signatures.next().is_some() || signature.index() != expected_index",
        "verify_hash(expected_key, block.hash())",
    )),
    (BODY_STORE, "method", "V2BodyStore::validate_envelope", (
        "envelope.manifest.validate(&self.context)?", "body_origin_view <= envelope.round.view",
        "header.height().get() != envelope.round.height", "header.prev_block_hash() != expected_parent",
        "verify_origin_block_signature(&self.context, &block, &self.signature_policy)?",
    )),
    (CONTROLS, "method", "ValidBlock::validate_execution_context_header", (
        "Self::checked_execution_context_header(block)?", "bundle.native_lane_decisions.is_some()",
        "return Err(Self::execution_context_error(",
    )),
    (CONTROLS, "method", "ValidBlock::validate_execution_context_with_state", (
        "matches!(\n                &validation_profile,\n                ConsensusValidationProfile::NativePreparation { .. }\n            )",
        "native_lane_batch_for_execution(block)", "Self::checked_execution_context_header(block)?",
        "Self::validate_execution_context_header(block)?", "Self::validate_execution_context_alignment(block, bundle)?",
    )),
    (BLOCK, "method", "ValidBlock::prepare_native_candidate", (
        "source: crate::state::PreparedNativeLaneBatchSourceV1<'state>",
        "context: crate::sumeragi::v2::VerifiedHeightContext",
        "ensure_state_access_without_exec_witness()",
        "let Some((state, body, generation)) = source.preparation_input() else {\n            return Ok(None);\n        };",
        "if !body.is_resultless_proposal()", "native_lane_batch_for_execution(body)",
        "body.validate_proposal_commitments()", "frozen.height != body.header().height().get()",
        "frozen.network_id != *state.network_id_ref()", "!= body.header().prev_block_hash()",
        "verify_origin_block_signature(", "BlockSignaturePolicy::RotatingLeader",
        "length > frozen.da_layout.max_payload_size_bytes", "ConsensusValidationProfile::NativePreparation",
        "Self::validate_static_state_dependent(", "Self::validate_static_with_snapshot(",
        "if generation != state.state_view_generation()", "source.record_execution(body, context)?",
        "recorded.into_preparation_parts()", "Arc::new(native.context().context().clone())",
        "PreparedCarrier::prepare(ValidatedCarrierPreparationInput", "native: Some(native)",
    )),
    (NATIVE_SOURCE, "method", "PreparedNativeLaneBatchSourceV1::prepare_candidate", (
        "self,", "context: crate::sumeragi::v2::VerifiedHeightContext",
        "ValidBlock::prepare_native_candidate(", "self,\n            context,\n            genesis_account,\n            time_source,\n            block_cadence",
    )),
    (NATIVE_SOURCE, "method", "PreparedNativeLaneBatchSourceV1::preparation_input", (
        "Option<(&'state State, &SignedBlock, u64)>",
        "self.is_current()\n            .then_some((self.state, &self.input, self.generation))",
    )),
    (NATIVE_STAGE, "struct", "RecordedNativeLaneBatchV1", (
        "prepared: PreparedLaneDecisionBatchV1<'state>", "carrier: iroha_data_model::block::SignedBlock",
        "context: crate::sumeragi::v2::VerifiedHeightContext",
    )),
    (NATIVE_STAGE, "struct", "NativeExecutionCustody", (
        "seal: Arc<NativeLaneStageSealV1>", "sources: Vec<VerifiedLaneDecisionGroupV1>",
        "executions: Vec<Execution>", "context: crate::sumeragi::v2::VerifiedHeightContext",
    )),
    (NATIVE_STAGE, "method", "RecordedNativeLaneBatchV1::into_preparation_parts", (
        "fn into_preparation_parts(\n        self,\n    )", "self.prepared\n            .verify_source_binding()",
        "verify_execution_output_seal(&self.carrier)", "validate_native_output_source(&self.carrier)",
        "let seal = Arc::clone(", "self.prepared\n                .overlay\n                .native_lane_stage\n                .as_ref()",
        "let PreparedLaneDecisionBatchV1", "overlay,\n            executions,\n            sources", "= self.prepared",
        "NativeExecutionCustody {\n                seal,\n                sources,\n                executions,\n                context: self.context",
    )),
    (NATIVE_STAGE, "method", "NativeExecutionCustody::retains_state", (
        "Arc::ptr_eq(seal, &self.seal)", "self.context.context().height == state._curr_block.height().get()",
        "self.context.context().network_id == state.network_id", "self.sources.len() == self.seal.batch.groups.len()",
        "self.executions.len() == self.sources.len()", "source.body().payload() == &wire.payload",
        "source.decisions() == wire.decisions", "state.validate_native_lane_stage_membership().is_ok()",
    )),
    (NATIVE_FINALIZED, "struct", "FinalizedNativeLaneBatchV1", (
        "source: NativeLaneBatchRecoveryV1", "carrier: SignedBlock",
    )),
    (NATIVE_FINALIZED, "method", "NativeLaneBatchRecoveryV1::project", (
        "self.finality\n            .validate_for_header(&block.header())", "block.hash() != self.finality.block_hash",
        "canonical_proposal_wire_hash()", "!= self.finality.subject.payload_hash",
        "if block.has_results()", "let executed = block.encode_wire()",
        "let commitment = &self.finality.commit_qc.execution_commitment",
        "u64::try_from(executed.len()).ok() != Some(commitment.executed_block_wire_len)",
        "Hash::new(&executed) != commitment.executed_block_wire_hash",
        "crate::block::native_lane_batch_for_execution(block)?",
        "source: self.clone()", "carrier: block.canonical_resultless_proposal()",
    )),
    (CONTROLS, "struct", "PreparedNativeExecutionControls", (
        "state: &'state State", "generation: u64", "header: BlockHeader",
        "admissions: Vec<Vec<u8>>", "npos: Option<PreparedPristineConsensusEffects>",
        "context: crate::sumeragi::v2::VerifiedHeightContext",
    )),
    (CONTROLS, "method", "ValidBlock::prepare_native_execution_controls", (
        "context: crate::sumeragi::v2::VerifiedHeightContext",
        "ensure_state_access_without_exec_witness()", "native_lane_batch_for_execution(block)",
        "validate_proposal_commitments()", "ensure_da_indexes_hydrated()",
        "let generation = state.state_view_generation()", "let frozen = context.context()",
        "frozen.network_id != *view.network_id()", "frozen.height != block.header().height().get()",
        "height.checked_add(1)", "!= Some(frozen.height)",
        "block.header().prev_block_hash() != view.latest_block_hash()",
        ".map(|qc| qc.subject.block_hash)\n                    != view.latest_block_hash()",
        "active_proof_policy_bundle_at_height(&view.nexus, frozen.height)",
        "block.header().da_proof_policies_hash() != Some(HashOf::new(&expected_da_policy))",
        "committed_nexus_amx_context_hash(state)", "committed_execution_policy_hash(state)",
        "frozen.nexus_amx_context_hash != nexus || frozen.execution_policy_hash != policy",
        "Self::validate_npos_effects_with_state(block, state, Some(frozen.mode), Some(frozen))?",
        "Self::prepare_pristine_consensus_effects(block, state, Some(frozen))?",
        "Ok(PreparedNativeExecutionControls {\n                state,\n                generation,\n                header: block.header(),",
        "admissions: block\n                    .execution_context()", ".queue_plan_admissions\n                    .clone()", "npos,\n                context",
    )),
    (CONTROLS, "method", "PreparedNativeExecutionControls::apply", (
        "self,", "validate_native_pristine_control_owner(self.state, self.generation, &self.header)",
        "stage_queue_plan_admissions_for_carrier(&self.admissions)",
        "if let Some(npos) = self.npos", "npos.apply(overlay)?", "Ok(self.context)",
    )),
    (CONTROLS, "method", "ValidBlock::prepare_pristine_consensus_effects", (
        "active_runtime_abi_hash(", "&state.world_view()", "block.header().height().get()",
        "block.npos_consensus_effects()", "authenticated_height_context.ok_or_else(",
        "v2_committed_evidence_prune_keys_from_state(\n                    state,\n                    height,\n                    effects.v2_evidence_admissions.len(),\n                )",
        "expected_anchor: header.prev_block_hash().map(|block_hash|",
        "height: height.saturating_sub(1)", "roster: context\n                    .roster\n                    .iter()",
        "effects: effects.clone()", "header",
    )),
    (CONTROLS, "method", "PreparedPristineConsensusEffects::apply", (
        "if state_block._curr_block != self.header", "return Err(",
        "state_block.apply_pristine_npos_consensus_effects(",
        "&self.effects, &self.prune_keys, self.expected_anchor, &self.roster,",
        "self.header.height().get(), self.header.view_change_index(), self.header.creation_time_ms",
    )),
    (CONTROLS, "method", "ValidBlock::finalize_native_execution_contexts", (
        "context: &crate::sumeragi::v2::VerifiedHeightContext",
        "Self::validate_staged_execution_controls(block, state)?",
        "validate_axt_envelopes(block, state)?", "state.validate_da_shard_cursors(block)?",
        "Self::validate_sccp_commitment_root(block)?",
        "finalize_lane_consensus_contexts(block, Some(context.context()))",
    )),
    (NATIVE_STAGE, "method", "StateBlock::validate_native_pristine_control_owner", (
        "!std::ptr::eq(self.state_ref, state)",
        "!super::is_stable_state_view_generation(generation, state.state_view_generation())",
        "&self._curr_block != header", "self.start_of_block_effects_applied",
        "self.applied_npos_consensus_effects_hash.is_some()",
        "!self.staged_queue_plan_admissions.is_empty()", "self.staged_merge_entry.is_some()",
        "self.native_lane_stage.is_some()", "!self.world.merge_execution_write_set_bytes().is_empty()",
        "return Err(",
    )),
    (NATIVE_STAGE, "method", "State::record_native_lane_decision_batch", (
        "groups: Vec<VerifiedLaneDecisionGroupV1>", "context: crate::sumeragi::v2::VerifiedHeightContext",
        "ensure_exec_witness_capture_available()", "with_stable_observation(self, ||",
        "if !carrier.is_resultless_proposal()", "native_lane_batch_for_execution(&carrier)",
        "ValidBlock::prepare_native_execution_controls(\n                &carrier, self, context,\n            )",
        "self.prepare_lane_decision_batch(&groups)?", "if &batch != expected",
        "self.with_native_lane_execution_scope(", "begin_exec_witness_capture()",
        "controls\n                        .apply(overlay)", "Ok((recorder, context))",
        "overlay.seal_native_lane_decision_batch(results, batch)",
        "|overlay, executions, (recorder, context)|",
        "ValidBlock::seal_native_execution_outputs(\n                        &mut carrier,\n                        overlay,\n                        &executions,\n                    )",
        "ValidBlock::finalize_native_execution_contexts(\n                        &carrier, overlay, &context,\n                    )",
        "overlay.capture_exec_witness()", "verify_execution_output_seal(&carrier)",
        "drop(recorder)", "Ok((executions, context))", "PreparedLaneDecisionBatchV1::from_stage(overlay, executions, groups)?",
        "Ok(RecordedNativeLaneBatchV1 {\n                prepared,\n                carrier,\n                context,\n            })",
    )),
    (NATIVE_KERNEL, "method", "State::with_native_lane_execution_scope", (
        "enter: impl FnOnce(&mut StateBlock<'state>)",
        "ensure_state_access_without_exec_witness()", "self.block_with_owned_start_stages(",
        "overlay.start_of_block_effects_applied", "overlay.native_lane_stage.is_some()",
        "overlay.staged_merge_entry.is_some()", "!overlay.staged_queue_plan_admissions.is_empty()",
        "!overlay.world.merge_execution_write_set_bytes().is_empty()",
        "preflight_lane_decision_execution_inputs(groups)", "let scope = enter(overlay)?",
        "header: overlay._curr_block.clone()", "groups",
        "overlay.produce_native_execution_outputs(preflight, finish_native)?",
        "finish_scope(overlay, result, scope)",
    )),
    (NATIVE_SOURCE, "struct", "PreparedNativeLaneBatchSourceV1", (
        "state: &'state State", "observed: VerifiedLaneContexts", "generation: u64",
        "input: Arc<SignedBlock>",
        "groups: Vec<VerifiedLaneDecisionGroupV1>",
    )),
    (NATIVE_SOURCE, "method", "State::prepare_native_lane_batch_from_pre_state", (
        "carrier: &SignedBlock", "let header = carrier.header()",
        "header.da_proof_policies_hash() != Some(expected_policy_hash)",
        "self.import_recovered_lane_decision_group(&observed, execution, input)",
        "self.import_lane_decision_group(&observed, execution)",
        "LaneDecisionGroupPreparationV1::Ready(group) => groups.push(group)",
        "!observed.is_current(self)", "state: self,\n                observed,\n                generation,",
        "input: Arc::new(carrier.clone()),\n                groups",
    )),
    (NATIVE_SOURCE, "method", "PreparedNativeLaneBatchSourceV1::record_execution", (
        "self,", "context: crate::sumeragi::v2::VerifiedHeightContext",
        "ensure_exec_witness_capture_available()", "if !self.is_current()",
        "if carrier != *self.input {\n            return Err(", "drop(self.input)",
        "record_native_lane_decision_batch(carrier, self.groups, context)",
        "self.generation,\n            self.state.state_view_generation()", "recorded.map(Some)",
    )),
    (NATIVE_SOURCE, "method", "PreparedNativeLaneBatchSourceV1::stage_with_start_hooks", (
        "ensure_state_access_without_exec_witness()", "native_lane_batch_for_scratch(&self.input)",
        "if !self.is_current()", "self.state.lane_execution_state_hash()",
        "let batch = crate::block::native_lane_batch_for_scratch(&self.input)",
        "actual_base != batch.base_state_hash",
        "replay_lane_decision_batch(&self.input.header(), batch, self.groups)",
    )),
    (NATIVE_CARRIER, "fn", "native_lane_batch_for_scratch", (
        "let batch = native_lane_batch_for_execution(carrier)?",
        ".queue_plan_admissions\n        .is_empty()", "carrier.npos_consensus_effects().is_some()",
        "carrier.header().npos_effects_hash().is_some()", "return Err(", "Ok(batch)",
    )),
    (NATIVE_STAGE, "method", "StateBlock::seal_native_lane_decision_batch", (
        "queue_plan_admissions_hash: HashOf::new(&self.staged_queue_plan_admissions)",
        "npos_effects_hash: self.applied_npos_consensus_effects_hash",
    )),
    (NATIVE_STAGE, "method", "StateBlock::validate_native_lane_stage_membership", (
        "HashOf::new(&self.staged_queue_plan_admissions) != seal.queue_plan_admissions_hash",
        "self.applied_npos_consensus_effects_hash != seal.npos_effects_hash",
        "self.canonical_wsv_merge_commit_authorization.is_some()",
        "self\n                .canonical_carrier_commit_metadata_authorization\n                .is_some()",
        "self._curr_block != seal.carrier", "return Err(",
    )),
    (NATIVE_STAGE, "method", "StateBlock::validate_native_output_carrier", (
        "self.validate_native_lane_execution()", "self.validate_native_output_source(block)",
    )),
    (NATIVE_STAGE, "method", "StateBlock::validate_native_output_source", (
        "self.validate_native_lane_stage_membership()", "seal.completed_write_set_root.is_none()",
        "block.header() != seal.carrier", "block.header().npos_effects_hash() != seal.npos_effects_hash",
        ".map(|bundle| HashOf::new(&bundle.queue_plan_admissions))\n                != Some(seal.queue_plan_admissions_hash)",
        "!block.external_entrypoints_slice().is_empty()", "!= Some(seal.batch.as_ref())", "return Err(",
    )),
)

# The old direct Native manifest, receipt/latest and prune allowance obligations
# now belong to the following owners; ordinary block bytes remain separate.
PREPARATION_OWNER_BINDINGS = (
    *NATIVE_CONTROL_BINDINGS,
    (TAIL, "method", "ValidBlock::finalize_owned_execution_metadata", (
        "Self::finalize_common_execution_metadata(",
        "Self::finalize_lane_settlement_evidence(block, state, &routed, &summaries)?",
        "stage_ordinary_lane_frontiers(block)", "canonical_carrier_membership_hashes(",
        "stage_canonical_carrier_membership(membership, height)",
        "resolve_queue_plan_pending_obligations_from_block(block)",
        "Self::validated_committed_fragment_count(state, advertised_fragments)?",
    )),
    (TAIL, "method", "ValidBlock::finalize_common_execution_metadata", (
        "if routes.len() != block.network_entrypoint_count()", "return Err(",
        "crate::tx::prune_expired_sealed_commitments(state)",
        "u64::try_from(state.committed_fragment_count())",
        "state.finalize_axt_asset_incarnations()", "evaluate_nexus_autoscale(block, fragments)",
        "finalize_axt_policy_transition_ratchets()", "let policy = state.axt_policy_snapshot()",
        "Self::validate_advertised_axt_post_state(advertised_policy, &policy)?",
        "Self::validate_advertised_axt_transitions(", "state.axt_authorization_transitioned()",
    )),
    (NATIVE_METADATA, "method", "ValidBlock::seal_native_execution_outputs", (
        "verify_native_execution_metadata(block, executions)", "block\n                .validate_output_merkle_cache()",
        "advertised_policy\n                .as_ref()\n                .ok_or_else(", ".validate()",
        "seal_execution_outputs(block, |state, source, routes|",
        "verify_native_execution_metadata(source, executions)",
        "Self::finalize_common_execution_metadata(",
        "Self::native_execution_finality_statements(source, state, executions, routes)?",
        "source.header().height().get()", "std::iter::empty::<HashOf<TransactionEntrypoint>>()",
        "Self::validated_committed_fragment_count(state, advertised_fragments)?",
        "Ok(crate::state::ExecutionOutputSealMetadata {", "verify_execution_output_seal(block)",
    )),
    (NATIVE_METADATA, "method", "ValidBlock::native_execution_finality_statements", (
        "executions.len() != routes.len() || executions.len() != block.network_entrypoint_count()",
        "executions.iter().zip(routes)", "source\n                .payload\n                .input\n                .routing_plan()",
        "plan.coordinator_route() != *route", ".find(|(_, slot)| slot.route == *route)",
        "source.decisions.get(slot_index)", "let commitment = &execution.settlement_commitment",
        "iroha_data_model::nexus::compute_settlement_hash(commitment)", "!= execution.settlement_hash",
        "if !Self::native_settlement_requires_relay(commitment)?", "continue;",
        "source\n                .payload\n                .descriptor\n                .canonical_hash()",
        "LaneRelayEnvelope::new(", "commitment.clone()", "decision.manifest.byte_len",
        "with_lane_block_descriptor_hash(Some(descriptor_hash))",
        "entry.dsid == commitment.dataspace_id", "entry.policy.manifest_root",
        "envelope.lane_finality_statement()", "statements.sort_unstable_by_key(",
    )),
    (NATIVE_METADATA, "method", "ValidBlock::native_settlement_requires_relay", (
        "let has_receipts = !commitment.receipts.is_empty()",
        "|| !commitment.nexus_fee_receipts.is_empty()",
        "|| !commitment.native_amx_receipts.is_empty()", "if !has_receipts",
        "!commitment.total_local_amount.is_zero()", "!commitment.total_xor_due.is_zero()",
        "!commitment.total_xor_after_haircut.is_zero()", "!commitment.total_xor_variance.is_zero()",
        "commitment.swap_metadata.is_some()", "return Ok(false)",
        "if commitment.tx_count == 0", "return Err(", "Ok(true)",
    )),
    (NATIVE_STAGE, "method", "StateBlock::verify_native_execution_metadata", (
        "self.validate_native_output_carrier(block)?", "!self.settlement_accumulator.is_empty()",
        "executions.len() != seal.batch.groups.len()", "executions.len() != seal.settlement_hashes.len()",
        ".zip(&seal.batch.groups)", ".zip(&seal.authenticated_aliases)", ".zip(&seal.settlement_hashes)",
        "execution.source != *source", "execution.authenticated_signed_replay_alias != *alias",
        "execution.settlement_hash != *settlement",
        "super::canonical_merge_settlement_hash(&execution.settlement_commitment)", "!= *settlement",
    )),
    (AUTONOMOUS, "method", "Kura::validate_configured_autonomous_mutation_disk_peak_with_reservation_deltas_locked", AUTONOMOUS_TOKENS),
    (BLOCK, "method", "ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block", (
        "validation_context.authenticated_height_context.clone()",
        "context.id() == validation_context.context_id",
        "context.height == block.header().height().get()",
        "context.network_id == *state.network_id_ref()",
        ".eq(context.roster.iter().map(|entry| &entry.validator))",
        "if !context_matches", "Self::validate_sumeragi_v2_candidate_keep_voting_block(",
        "PreparedCarrier::prepare(ValidatedCarrierPreparationInput {",
    )),
    (PREPARED, "method", "PreparedCarrier::prepare", ("execution_prefix::prepare(input)",)),
    (PREFIX, "struct", "ValidatedExecutionPrefix", (
        "sealed: output_capacity::SealedExecutionOutputs", "inventory: Arc<FastpqSourceInventoryV1>",
        "witness: ExecWitness", "fastpq_witness_context: Option<crate::fastpq::FastpqWitnessContext>",
        "parliament_timed_ovn_casting_bindings:",
    )),
    (PREFIX, "struct", "PrefixPreparation", (
        "state: Box<StateBlock<'state>>", "prefix: ValidatedExecutionPrefix",
    )),
    (PREFIX, "method", "PrefixPreparation::capture", (
        "native: Option<lane_decision_batch::NativeExecutionCustody>", "let authority = match native",
        "Some(native)\n                if native.retains_state(&state)\n                    && block\n                        .execution_context()\n                        .is_some_and(|context| context.native_lane_decisions.is_some()) =>",
        "PrefixSourceAuthority::Native(Box::new(native))",
        "None if state.native_lane_stage.is_none()", "state.merge_carrier_entrypoints.is_empty()",
        "context.native_lane_decisions.is_some()", "state.staged_merge_entry.is_some()",
        "state.canonical_wsv_merge_commit_authorization.is_some()",
        "state.verify_execution_output_seal(block)?", "state.verified_fastpq_source_inventory_for_capture()?",
        "state.verify_cached_ordinary_witness_content(&verified_inventory)?", ".exec_witness",
        "from_result_bearing_block_and_merge_entry", "LaneFinalityManifestV1::from_result_bearing_block(block)?",
        "execution_commitment_from_validated_block(witness, &manifest, &lanes, block)",
        ".replace(output_capacity::ExecutionOutputPlanState::Captured)",
        "sealed.sources().is_native()", "sealed.sources().proposal() != block.hash()",
        "Arc::ptr_eq(&inventory, &verified_inventory)", "let prefix = ValidatedExecutionPrefix {",
        "Ok((Self { state, prefix }, manifest, commitment))",
    )),
    (PREFIX, "method", "ValidatedExecutionPrefix::retains_closed_state", (
        "self.sealed.proposal() == state._curr_block.hash()",
        "Some(output_capacity::ExecutionOutputPlanState::Captured)", "state.exec_witness.is_none()",
        "state.fastpq_source_inventory.is_none()", "state.fastpq_witness_context.is_none()",
        "state.parliament_timed_ovn_casting_bindings.is_none()", "state.native_lane_stage.is_none()",
        "PrefixSourceAuthority::Native(native) => native.retains_state(state)",
    )),
    (PREFIX, "method", "PrefixPreparation::prepare_world_effects", (
        "let state = &mut *self.state", "!self.prefix.retains_closed_state(state)",
        "state.block_hashes.pending.as_slice() != [state._curr_block.hash()]",
        "state.validate_canonical_runtime_projection()?", "state.verify_lane_consensus_contexts_publication()?",
        "validate_merge_carrier_entrypoint_binding()", "finalize_axt_asset_incarnations()",
        "finalize_axt_policy_transition_ratchets()", "state.prune_axt_replay_ledger(",
        "validate_owned_runtime_catalog_overlay()", "ensure_pending_autoscale_lifecycle_staking_is_safe(",
        "world_commit::PreparedWorldCommit::prepare_overlay(",
    )),
    (PREFIX, "fn", "prepare", (
        "input.into_parts()", "PrefixPreparation::capture(state, &valid, native)?",
        "prepare_deterministic_carrier_metadata", "preparation.prepare_world_effects()?",
        "prepare_carrier_publication_events(block.header())", "PreparedTieredSnapshot::prepare(",
        "prefix: source_prefix", "Ok(PreparedCarrier {", "Err(error) => Err((Box::new(valid.into()), error))",
    )),
    (JOURNALS, "method", "PreparedCarrier::prepare_journals", (
        "admit_journals: impl FnOnce(CarrierJournalInputs<'_, 'state>)",
        "let admission;", "source_prefix,", "admit_journals(CarrierJournalInputs {",
        "state: &state", "prefix: &source_prefix", "state.prepare_carrier_geometry()?",
        "world.try_detach_journals", "transactions.prepare_commit()?.detach()", "admission,\n        })",
    )),
    (OUTPUT, "struct", "SealedExecutionOutputs", (
        "proposal:", "wire_hash: Hash", "wire_bytes: u64", "world_delta:", "sources: OwnedExecutionSources",
    )),
    (SEAL, "method", "StateBlock::seal_execution_outputs", (
        "let sources = retained", "state.finalize_owned_fastpq_source_inventory_with_pending(&sources, pending)?",
        "sources.network_routes()", "Ok(SealedExecutionOutputs {", "sources,", "world_delta,",
        "proposal: block.hash()", "wire_hash: Hash::new(&wire)", "wire_bytes: u64::try_from(wire.len())",
    )),
    (PREPARED, "method", "PreparedCarrier::native_amx_manifest", ("&self.native_amx_manifest",)),
    (PREPARED, "method", "PreparedCarrier::execution_prefix_commitment", ("self.execution_prefix",)),
    (CAPACITY, "method", "Kura::native_amx_publication_plan_under_prune_and_canonical_guards", (
        "self.native_amx_publication_plan_for_storage_under_prune_and_canonical_guards(",
        "NativeAmxPublicationStorage::Active",
    )),
    (CAPACITY, "method", "Kura::native_amx_publication_plan_for_storage_under_prune_and_canonical_guards", (
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, merge_entry)",
        "self.validate_native_amx_participant_application_evidence_byte_budget(&manifest, None)",
        "native_amx_participant_application_artifacts(",
        "native_amx_participant_application_finality_placeholder_hash()",
        "executed_wire_hash: manifest.executed_block_wire_hash()",
        "native_amx_route_publication_capacity_for_storage_locked(",
        "if routes.insert(route, capacity).is_some()",
    )),
    (CAPACITY, "method", "Kura::native_amx_route_publication_capacity_for_storage_locked", (
        "let descriptor = &receipt.participant_proposal.descriptor",
        "NativeAmxPublicationStorage::Active", "self.lane_storage_entry(descriptor.lane_id)?",
        "native_amx_route_publication_capacity_at_target_locked(",
        "self.native_amx_reservation_physical_target_from_journal(descriptor)?",
        "self.require_native_amx_reservation_physical_target(&target)?",
    )),
    (CAPACITY, "method", "Kura::native_amx_route_publication_capacity_at_target_locked", (
        "self.require_active_lane_artifact(entry, descriptor)?",
        "NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&expected_receipt)",
        "expected_manifest.encode_framed()?.len()", "expected_receipt.encode_framed()?.len()",
        "norito::encode_canonical(&expected_latest)?.len()",
        "let mut component_allocation_bytes = component_bytes.clone()",
        "Self::plan_native_amx_evidence_prune_intent_from_artifacts(",
        "self.native_amx_evidence_prune_intent_max_bytes()",
        "Some(intent) => u64::try_from(norito::encode_canonical(&intent)?.len())?",
        "outstanding_components", "physical_cleanup_pending", "cleanup_complete: false",
    )),
    (CAPACITY, "method", "NativeAmxRoutePublicationCapacity::reserved_bytes", (
        "if self.cleanup_complete", "return Some(0)",
        "self.outstanding_components", ".try_fold(self.prune_journal_bytes, |total, kind|",
        "total.checked_add(*self.component_allocation_bytes.get(kind)?)",
    )),
    (CAPACITY, "method", "NativeAmxPublicationCapacityReservation::reserved_bytes", (
        "self.routes", ".try_fold(self.index_additional_bytes, |total, route|",
        "total.checked_add(route.reserved_bytes()?)",
    )),
    (CAPACITY, "method", "Kura::begin_native_amx_store_capacity_under_prune_and_canonical_guards", (
        "native_amx_publication_plan_under_prune_and_canonical_guards(block, merge_entry)?",
        "self.prepare_native_amx_publication_index(block, merge_entry, replaced)?",
        "self.admit_native_amx_publication_capacity_plan(carrier, plan, replaced, publication)",
    )),
    (CAPACITY, "method", "Kura::admit_native_amx_publication_capacity_plan", (
        "if publication.record.carrier != carrier", "plan.index_record = Some(publication.record.clone())",
        "plan.index_additional_bytes = publication.additional_bytes", "let created = !reservations.contains_key(&carrier)",
        "for (route, old) in &existing.routes", "old.component_bytes != new.component_bytes",
        ".is_subset(&old.outstanding_components)", "new.prune_journal_bytes > old.prune_journal_bytes",
        "for (other_carrier, other) in reservations.iter()", "other.routes.contains_key(route)",
        "reservations.insert(carrier, plan)", "rollback_new_reservation: created",
    )),
    (CAPACITY, "method", "Kura::native_amx_publication_capacity_reserved_bytes", (
        "self.native_amx_publication_capacity_reservations", ".values()",
        ".try_fold(0_u64, |total, reservation|", ".checked_add(reservation.reserved_bytes().ok_or_else(",
    )),
    (CAPACITY, "method", "Kura::lane_publication_budget_reserved_bytes", (
        "let merge = self.post_wsv_lane_artifact_budget_reserved_bytes()?",
        "let native = self.native_amx_publication_capacity_reserved_bytes()?",
        "merge.checked_add(native).ok_or_else(",
    )),
    (KURA, "method", "Kura::check_storage_budget", (
        ".post_wsv_prepend_admission_extra_under_prune_and_canonical_guards(",
        ".block_required_bytes_for_budget(block, merge_entry, limit)?",
        ".checked_add(prepend_extra)",
        "self.lane_publication_budget_reserved_bytes()?",
        ".saturating_add(lane_publication_reservations)",
        "if required > limit", "Error::StorageBudgetExceeded",
    )),
    (DURABLE, "method", "Kura::store_block_durable", (
        "begin_native_amx_store_capacity_under_prune_and_canonical_guards(",
        "self.check_storage_budget(block, merge_entry)?",
        "self.check_native_amx_existing_carrier_capacity_under_prune_and_canonical_guards()?",
        "owner.publish_pending_index()?",
    )),
)
# Private terminal publication is qualified only for identity geometry and
# no outstanding participant artifacts. Production Validate/Apply stays closed.
PHYSICAL_CARRIER = "crates/iroha_core/src/state/carrier_preparation/physical_publication.rs"
TERMINAL_CARRIER = "crates/iroha_core/src/state/carrier_preparation/publication.rs"
ARCHIVE_CARRIER = "crates/iroha_core/src/state/carrier_preparation/archive_publication.rs"
GEOMETRY_CARRIER = "crates/iroha_core/src/state/carrier_geometry_preparation.rs"
WITNESS_CARRIER = "crates/iroha_core/src/state/carrier_preparation/execution_witness_publication.rs"
WITNESS_LEASE = "crates/iroha_core/src/kura/publication_lease.rs"
TERMINAL_OWNER_BINDINGS = (
    (WITNESS_CARRIER, "fn", "publish_execution_witness", (
        "&mut self", "try_publication_lease()", "&self.checkpoint", "self.finality.artifact()",
        "self.journals.checkpoint", "drop(lease)", "stage_kagemusha_finality_sidecar(",
        "self.finality.artifact().height", "self.finality.artifact().block_hash",
        "self.journals.source_prefix.witness()", "self.journals.execution_prefix",
        "self.journals\n                    .source_prefix\n                    .parliament_timed_ovn_casting_bindings()\n                    .unwrap_or(&[])",
        "promote_kagemusha_finality_sidecar(\n                self.finality.artifact(),\n                self.checkpoint.finality_receipt(),\n            )",
    )),
    (WITNESS_LEASE, "method", "KuraPublicationLease::reauthenticate_execution_witness", (
        "self.kura.kagemusha_finality_sidecar_path(finality.height)",
        "let Some((sidecar, read)) = self.kura.decode_kagemusha_finality_sidecar(&path)? else {\n            return Err(",
        "Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)?",
        "self.kura.regular_sidecar_metadata(&path, &directory)?",
        "Kura::stable_sidecar_metadata_unchanged(&read.metadata, current)", "Ok(())",
    )),
    (PREFIX, "method", "ValidatedExecutionPrefix::retains_carrier", (
        "self.sealed.proposal() == block.hash()", "self.sealed.sources().proposal() == block.hash()",
        "PrefixSourceAuthority::Ordinary => {\n                    !self.sealed.sources().is_native()\n                        && !block\n                            .execution_context()\n                            .is_some_and(|bundle| bundle.native_lane_decisions.is_some())\n                }",
        "PrefixSourceAuthority::Native(native) => {\n                    self.sealed.sources().is_native() && native.retains_carrier(block, context)\n                }",
    )),
    (NATIVE_STAGE, "method", "NativeExecutionCustody::retains_carrier", (
        "self.context.context() == context", "self.seal.completed_write_set_root.is_some()",
        "self.seal.carrier == block.header()", "block.header().npos_effects_hash() == self.seal.npos_effects_hash",
        "Some(self.seal.queue_plan_admissions_hash)", "block.external_entrypoints_slice().is_empty()",
        "Some(self.seal.batch.as_ref())", "self.sources.len() == self.seal.batch.groups.len()",
        "self.executions.len() == self.sources.len()", "source.body().payload() == &wire.payload && source.decisions() == wire.decisions",
    )),
    (ARCHIVE_CARRIER, "fn", "publish_archives", (
        "&mut self", "try_publication_lease()", "&self.checkpoint", "self.finality.artifact()",
        "self.journals.checkpoint", "drop(lease)", "self.checkpoint.finality_receipt()",
        "self.journals.provider_capture.as_mut()", "self.journals.reputation_capture.as_mut()",
        "provider\n                .publish(receipt)", "reputation\n                .publish(receipt)",
    )),
    (PHYSICAL_CARRIER, "fn", "try_prepare_physical", (
        "admit: impl FnOnce(&Self, &State)", "admit(&original, target)",
        "target.matches_kura_instance(&original.journals.kura)", "original.publish_execution_witness()", "original.publish_archives()",
        "target.kura.try_publication_lease()", "kura.reauthenticate_checkpoint(",
        "kura.reauthenticate_execution_witness(original.finality.artifact())",
        "StateFences::try_acquire(target)", "journals.try_map_components(",
        "Ok(PhysicallyPreparedCarrier {\n                target,\n                decision: retain!(journals),\n                installation,\n            })",
    )),
    (GEOMETRY_CARRIER, "method", "PreparedCarrierGeometry::is_identity_transition", (
        "self._header == header", "self._pending.is_none()", "self._certified_frontiers.is_empty()",
        "self._previous_runtime_catalog == self._accepted_runtime_catalog",
        "self._predecessor.lanes == self._successor.lanes",
        "self._predecessor.lane_count == self._successor.lane_count",
        "self._predecessor.lane_incarnation_lineage\n                == self._successor.lane_incarnation_lineage",
        "self._predecessor.owner_policy == self._successor.owner_policy",
        "self._predecessor.autoscale_last_transition_height\n                == self._successor.autoscale_last_transition_height",
    )),
    (PHYSICAL_CARRIER, "method", "CarrierFences::release_for_completion", (
        "drop(write)", "drop(lifecycle)", "drop(kura)", "commit",
    )),
    (TERMINAL_CARRIER, "method", "PhysicallyPreparedCarrier::publish", (
        "journals.effects.replay_prevalidation", "journals\n            .source_prefix\n            .retains_carrier(journals.valid.as_ref(), &journals.context)",
        "journals.effects.header != journals.valid.as_ref().header()", "journals.staged_legacy_source()",
        "journals\n            .geometry\n            .is_identity_transition(journals.effects.header)",
        "journals.effects.pending_autoscale_lifecycle.is_some()", "journals.native_amx_manifest.entries().is_empty()",
        "return Err((self.abort(), error))", "target.begin_state_view_write()",
        "transactions.publish()", "runtime.publish()", "world.publish()", "world_effects.publish(target)",
        "target.apply_committed_da_commitment_bundle(pending, false)",
        "target.install_sccp_registry_cache(std::sync::Arc::clone(&effects.sccp_registry))",
        "block_hashes.publish()", "target.update_latest_block_header_cache(effects.header)",
        "drop(generation)", "fences.release_for_completion()", "effects.publish_observability(target)",
        "target.hydrate_verified_lane_relay_records(effects.verified_lane_relay_records)",
        "tiered_snapshot.publish(target, false)", "target.enforce_nexus_storage_budget(height)",
        "target.persist_query_index_status(height, Some(effects.header.hash()))",
        "publication_events.append(&mut extra_events)", "drop(commit)", "Ok(PublishedCarrier {",
        "source: source_prefix", "admission,\n            binding,\n            installation",
    )),
)
PREPARATION_OWNER_BINDINGS += TERMINAL_OWNER_BINDINGS

QUEUE_OWNER = "crates/iroha_core/src/queue.rs"
PUBLICATION_MUTEX = "crates/iroha_core/src/publication_lock.rs"
GEOMETRY_OWNER = "crates/iroha_core/src/kura/lane_geometry.rs"
GUARDED_GEOMETRY = "crates/iroha_core/src/kura/lane_geometry/guarded_publication.rs"
# These existing Queue/Kura owners remain distinct from State publication
# permission. Binding their actual guard transfer does not authorize an arbitrary
# Queue, nor open the terminal publisher's nonidentity-geometry refusal.
QUEUE_GEOMETRY_OWNER_BINDINGS = (
    (QUEUE_OWNER, "struct", "Queue", (
        "lane_reservation_transition_lock: PublicationMutex,",
        "push_remove_lock: PublicationMutex,",
        "lane_reservations: PublicationMutex<LaneQueueReservationStore>,",
    )),
    (QUEUE_OWNER, "method", "Queue::from_config_with_router_limits_and_catalogs", (
        "lane_reservation_transition_lock: PublicationMutex::default(),",
        "push_remove_lock: PublicationMutex::default(),",
        "lane_reservations: PublicationMutex::new(LaneQueueReservationStore::default()),",
    )),
    (QUEUE_OWNER, "struct", "QueueLaneRetirementObserver", (
        "queue: &'queue Queue", "_reservation_transition_guard: PublicationGuard<'queue>",
    )),
    (QUEUE_OWNER, "method", "Queue::lock_lane_retirement_observer", (
        "queue: self", "_reservation_transition_guard: self.lane_reservation_transition_lock.lock()",
    )),
    (QUEUE_OWNER, "method", "Queue::try_lock_lane_retirement_observer", (
        "Result<QueueLaneRetirementObserver<'_>, mv::ReleaseWait>",
        "let guard = self.lane_reservation_transition_lock.try_lock_or_wait()?;",
        "Ok(QueueLaneRetirementObserver {\n            queue: self,\n            _reservation_transition_guard: guard,\n        })",
    )),
    (QUEUE_OWNER, "method", "QueueLaneRetirementObserver::lane_has_pending_work", (
        "self.queue.lane_has_pending_work_under_retirement_observer(\n            lane_id,\n            dataspace_id,\n            lane_incarnation,\n        )",
    )),
    (QUEUE_OWNER, "method", "Queue::lane_has_pending_work_under_retirement_observer", (
        "hash_is_zero(lane_incarnation) || self.transaction_selection_durability_faulted()",
        "let _queue_guard = self.push_remove_lock.lock();",
        "if self.transaction_selection_durability_faulted() {\n            return true;\n        }",
        "let reservations = self.lane_reservations.lock();",
        "let owned = Self::lane_retirement_reservation_snapshot(\n            &reservations,\n            lane_id,\n            dataspace_id,\n            lane_incarnation,\n        );",
        "drop(reservations);", "let Some(owned) = owned else {\n            return true;\n        };",
        "self.lane_has_pending_route_work(&owned, lane_id, dataspace_id)",
    )),
    (QUEUE_OWNER, "method", "Queue::lane_retirement_reservation_snapshot", (
        "reservations: &LaneQueueReservationStore", "Option<HashSet<EntrypointHash>>",
        "key.lane_id == lane_id\n                && key.dataspace_id == dataspace_id\n                && key.lane_incarnation == lane_incarnation",
        "let reservation_owned_hashes = reservations\n            .live_by_entrypoint\n            .keys()\n            .copied()",
        ".chain(\n                reservations\n                    .commit_barriers\n                    .iter()\n                    .map(|key| key.entrypoint_hash),\n            )",
        ".chain(\n                reservations\n                    .completed_releases\n                    .iter()\n                    .flat_map(|completion| {\n                        completion\n                            .ordered_records\n                            .iter()\n                            .map(|record| record.key.entrypoint_hash)\n                    }),\n            )",
        "reservations\n            .live_by_entrypoint\n            .values()\n            .any(|record| exact_reservation(&record.key))",
        "reservations.commit_barriers.iter().any(exact_reservation)",
        "reservations.release_barriers.iter().any(|barrier| {\n                barrier.lane_id == lane_id\n                    && barrier.dataspace_id == dataspace_id\n                    && barrier.lane_incarnation == lane_incarnation\n            })",
        "reservations.completed_releases.iter().any(|completion| {\n                completion.barrier.lane_id == lane_id\n                    && completion.barrier.dataspace_id == dataspace_id\n                    && completion.barrier.lane_incarnation == lane_incarnation\n            })",
        "{\n            return None;\n        }\n        Some(reservation_owned_hashes)",
    )),
    (QUEUE_OWNER, "method", "Queue::lane_has_pending_route_work", (
        "reservation_owned_hashes: &HashSet<EntrypointHash>",
        "self.routing_plans.iter().any(|entry| {\n            !reservation_owned_hashes.contains(entry.key())",
        "self.txs.contains_key(entry.key())", "entry.value().legs().into_iter().any(|leg|",
        "leg.route.lane_id == lane_id && leg.route.dataspace_id == dataspace_id",
    )),
    (QUEUE_OWNER, "method", "QueueLaneRetirementObserver::try_into_cut", (
        "fn try_into_cut(\n        self,\n    ) -> Result<QueueLaneRetirementCut<'queue>, QueueRetirementBusy>",
        "let mutation = self\n            .queue\n            .push_remove_lock\n            .try_lock_or_wait()\n            .map_err(|wait|",
        "field: \"push_remove_lock\",\n                wait",
        "let reservations = self\n            .queue\n            .lane_reservations\n            .try_lock_or_wait()\n            .map_err(|wait|",
        "field: \"lane_reservations\",\n                wait",
        "Ok(QueueLaneRetirementCut {\n            reservations,\n            _mutation: mutation,\n            observer: self,\n        })",
    )),
    (QUEUE_OWNER, "struct", "QueueRetirementBusy", (
        "pub(crate) field: &'static str", "pub(crate) wait: mv::ReleaseWait",
    )),
    (QUEUE_OWNER, "struct", "QueueLaneRetirementCut", (
        "reservations: PublicationGuard<'queue, LaneQueueReservationStore>",
        "_mutation: PublicationGuard<'queue>", "observer: QueueLaneRetirementObserver<'queue>",
    )),
    (QUEUE_OWNER, "method", "QueueLaneRetirementCut::lane_has_pending_work", (
        "let queue = self.observer.queue;",
        "if hash_is_zero(lane_incarnation) || queue.transaction_selection_durability_faulted() {\n            return true;\n        }",
        "let Some(owned) = Queue::lane_retirement_reservation_snapshot(\n            &self.reservations,\n            lane_id,\n            dataspace_id,\n            lane_incarnation,\n        ) else {\n            return true;\n        };",
        "queue.lane_has_pending_route_work(&owned, lane_id, dataspace_id)",
    )),
    (PUBLICATION_MUTEX, "struct", "PublicationMutex", (
        "inner: parking_lot::Mutex<T>", "released: mv::ReleaseNotification",
    )),
    (PUBLICATION_MUTEX, "struct", "PublicationGuard", (
        "inner: mv::ReleaseGuard<'state, PhysicalPublicationGuard<'state, T>>",
    )),
    (PUBLICATION_MUTEX, "method", "PublicationMutex::new", (
        "fn new(value: T) -> Self", "inner: parking_lot::Mutex::new(value)",
        "released: mv::ReleaseNotification::default()",
    )),
    (PUBLICATION_MUTEX, "method", "PublicationMutex::wrap", (
        "guard: parking_lot::MutexGuard<'state, T>",
        "self.released.guard(PhysicalPublicationGuard {\n                guard: Some(guard),\n                fair: false,\n            })",
    )),
    (PUBLICATION_MUTEX, "method", "PublicationMutex::lock", (
        "self.wrap(self.inner.lock())",
    )),
    (PUBLICATION_MUTEX, "method", "PublicationMutex::try_lock", (
        "self.inner.try_lock().map(|guard| self.wrap(guard))",
    )),
    (PUBLICATION_MUTEX, "method", "PublicationMutex::try_lock_or_wait", (
        "let wait = self.released.observe();", "self.try_lock().ok_or(wait)",
    )),
    (PUBLICATION_MUTEX, "method", "PhysicalPublicationGuard<'_, T>::drop", (
        "if let Some(guard) = self.guard.take()", "if self.fair",
        "parking_lot::MutexGuard::unlock_fair(guard);", "else {\n                drop(guard);\n            }",
    )),
    (WITNESS_LEASE, "struct", "KuraPublicationLease", (
        "kura: &'kura Kura", "_sidecar: PublicationGuard<'kura>",
        "_geometry: PublicationGuard<'kura>", "_canonical: PublicationGuard<'kura>",
        "_prune: PublicationGuard<'kura>",
    )),
    (WITNESS_LEASE, "method", "KuraPublicationLease::from_geometry_guards", (
        "pub(super) fn from_geometry_guards(", "kura: &'kura Kura",
        "sidecar: PublicationGuard<'kura>", "geometry: PublicationGuard<'kura>",
        "canonical: PublicationGuard<'kura>", "prune: PublicationGuard<'kura>",
        "Self {\n            kura,\n            _sidecar: sidecar,\n            _geometry: geometry,\n            _canonical: canonical,\n            _prune: prune,\n        }",
    )),
    (WITNESS_LEASE, "method", "KuraPublicationLease::kura_under_publication_guards", (
        "pub(super) fn kura_under_publication_guards(&self) -> &'kura Kura", "self.kura",
    )),
    (GEOMETRY_OWNER, "method", "Kura::apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner", (
        "let _prune_guard = self.prune_lock.lock();", "self.ensure_prune_recovery_not_required()?;",
        "let _canonical_chain_guard = self.canonical_chain_lock.lock();",
        "self.resolve_canonical_storage_before_mutation()?;",
        "self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;",
        "let _geometry_guard = self.lane_geometry_lock.lock();",
        "let mut journal = self.read_lane_geometry_journal()?;",
        "self.finish_pending_lane_geometry_gc_locked(&mut journal)?;",
        "KuraPublicationLease::from_geometry_guards(\n            self,\n            self.sidecar_lock.lock(),\n            _geometry_guard,\n            _canonical_chain_guard,\n            _prune_guard,\n        )",
        "lease.apply_prepared_lane_geometry(guarded_publication::PreparedLaneGeometryTransition {",
        "certified_retirements,\n            transition_height,\n            namespace_receipts,\n            pending_canonical_bytes,",
        "previous_bindings,\n            updated_bindings,\n            previous_catalog,\n            updated_catalog,\n            journal_was_present,\n            journal",
    )),
    (GEOMETRY_OWNER, "method", "Kura::mark_lane_geometry_catalog_published_with_lineage_root", (
        "let _prune_guard = self.prune_lock.lock();", "self.ensure_prune_recovery_not_required()?;",
        "let _canonical_chain_guard = self.canonical_chain_lock.lock();",
        "self.resolve_canonical_storage_before_mutation()?;", "let _geometry_guard = self.lane_geometry_lock.lock();",
        "let mut journal = self.read_lane_geometry_journal()?;",
        "self.finish_pending_lane_geometry_gc_locked(&mut journal)?;",
        "KuraPublicationLease::from_geometry_guards(\n            self,\n            self.sidecar_lock.lock(),\n            _geometry_guard,\n            _canonical_chain_guard,\n            _prune_guard,\n        )",
        "lease.publish_prepared_lane_geometry_catalog(\n            guarded_publication::PreparedLaneGeometryCatalog {\n                bindings,\n                fingerprint,\n                lineage_root,\n                configured_baseline,\n                journal,\n            },\n        )",
    )),
    (GUARDED_GEOMETRY, "method", "KuraPublicationLease::apply_prepared_lane_geometry", (
        "pub(in crate::kura::lane_geometry) fn apply_prepared_lane_geometry(",
        "let kura = self.kura_under_publication_guards();", "PreparedLaneGeometryTransition {",
        "certified_retirements,\n            transition_height,\n            mut namespace_receipts,\n            pending_canonical_bytes,",
        "updated_catalog,\n            journal_was_present,\n            mut journal,\n        } = prepared;",
        "record.previous_catalog == previous_catalog", "record.previous_lineage_root == previous_lineage_root",
        "record.updated_catalog == updated_catalog", "record.updated_lineage_root == updated_lineage_root",
        "existing.previous_bindings != previous_bindings", "existing.updated_bindings != updated_bindings",
        "kura.ensure_lane_retirement_admissible_locked(\n                pending_canonical_bytes,\n                &retiring,\n                &certified_retirements,\n            )?;",
        "PreparedGeometryJournalTransition::prepare(kura, journal, record_index)?",
        "prepared.persist(kura, LaneGeometryPhase::Intent)?;",
        "GeometryEvidencePolicy::FreshJournalIntent", "GeometryEvidencePolicy::AllowJournalIntentProvisioning",
        'kura.poison_canonical_storage("lane geometry apply rollback", &ambiguous);',
        "prepared.persist(kura, LaneGeometryPhase::RolledBack)?;",
        "prepared.persist(kura, LaneGeometryPhase::FilesApplied)?;",
        "kura.ensure_authoritative_lane_markers_with_receipts(",
        "*kura.lane_storage_entries.lock() = kura.lane_storage_entries_from_geometry(",
    )),
    (GUARDED_GEOMETRY, "method", "KuraPublicationLease::publish_prepared_lane_geometry_catalog", (
        "pub(in crate::kura::lane_geometry) fn publish_prepared_lane_geometry_catalog(",
        "let kura = self.kura_under_publication_guards();",
        "PreparedLaneGeometryCatalog {\n            bindings,\n            fingerprint,\n            lineage_root,\n            configured_baseline,\n            mut journal,\n        } = prepared;",
        "let prior_journal_bytes = kura.read_geometry_file_bytes(&journal_path)?;",
        "record.updated_catalog != fingerprint", "record.updated_lineage_root != lineage_root",
        "record.updated_bindings != bindings", "journal.records[index].phase = LaneGeometryPhase::CatalogPublished;",
        "kura.validate_lane_geometry_journal(&journal)?;", "let published_journal_bytes = journal.encode();",
        "kura.atomic_write_geometry_file(\n            &journal_path,\n            &publication_temp,\n            &published_journal_bytes,\n        )",
        "kura.restore_lane_geometry_journal_file(\n                prior_journal_bytes.as_deref(),\n                &published_journal_bytes,\n                publication_temp_preexisted,\n            )",
        "Error::LaneGeometryPublicationRestoreFailed", "return Err(publication_error);",
    )),
)
PREPARATION_OWNER_BINDINGS += QUEUE_GEOMETRY_OWNER_BINDINGS

NATIVE_PREPARATION_SOURCE_RELATIVES = tuple(Path(p) for p in (
    QUEUE_OWNER, PUBLICATION_MUTEX, GEOMETRY_OWNER, GUARDED_GEOMETRY,
    PHYSICAL_CARRIER, TERMINAL_CARRIER, ARCHIVE_CARRIER, GEOMETRY_CARRIER, WITNESS_CARRIER, WITNESS_LEASE,
    APPLY, BLOCK, PREPARED, PREFIX, JOURNALS, OUTPUT, SEAL, TAIL, NATIVE_METADATA, NATIVE_STAGE,
    CONTROLS, NATIVE_SOURCE, NATIVE_KERNEL, NATIVE_CARRIER, NATIVE_FINALIZED, BODY_STORE,
    ORDINARY, CAPACITY, DURABLE, KURA, AUTONOMOUS,
    "scripts/formal/sumeragi_v2_multilane_native_preparation_contract.py",
    "pytests/scripts/sumeragi_v2_multilane_native_preparation_contract_test.py",
))


def validate_native_preparation_contract(
    root: Path, models: Any, errors: list[str], rust_binding_item: Callable,
) -> None:
    """Check executable authority joins and separate accounting without double charge."""
    owners = [m for m in models if isinstance(m, dict) and m.get("module") == MODEL]
    rows = owners[0].get("production_symbols", ()) if len(owners) == 1 else ()
    bindings = (
        (APPLY, "method", "V2ApplyService::validate_candidate", CANDIDATE_TOKENS),
        (ORDINARY, "fn", "lane_artifact_required_bytes_for_block", ORDINARY_TOKENS),
        *PREPARATION_OWNER_BINDINGS,
    )
    items = {}
    raw_items = {}
    for path, kind, symbol, tokens in bindings:
        matches = [r for r in rows if isinstance(r, dict)
                   and (r.get("path"), r.get("kind"), r.get("symbol")) == (path, kind, symbol)]
        if len(matches) != 1:
            errors.append(f"Native preparation ledger owner {symbol} must occur exactly once")
        elif tuple(matches[0].get("required_tokens", ())) != tokens:
            errors.append(f"Native preparation reviewed tokens changed for {symbol}")
        item = rust_binding_item(root, path, kind, symbol, "Native preparation", errors)
        if item is not None:
            raw_items[symbol] = item
            items[symbol] = _code(item)
            for token in tokens:
                if _code(token) not in items[symbol]:
                    errors.append(f"Native preparation {symbol} missing executable relation {token!r}")

    def require(symbol: str, *relations: str) -> None:
        for relation in relations:
            if symbol in items and _code(relation) not in items[symbol]:
                errors.append(f"Native preparation {symbol} missing executable relation {relation!r}")

    def ordered(symbol: str, *relations: str) -> None:
        cursor = 0
        for relation in relations:
            item = items.get(symbol)
            if item is None:
                return
            needle = _code(relation)
            index = item.find(needle, cursor)
            if index < 0:
                errors.append(f"Native preparation {symbol} missing or reorders executable relation {relation!r}")
                return
            cursor = index + len(needle)

    ordered("PublicationMutex::try_lock_or_wait",
            "let wait = self.released.observe();", "self.try_lock().ok_or(wait)")
    ordered("Queue::try_lock_lane_retirement_observer",
            "let guard = self.lane_reservation_transition_lock.try_lock_or_wait()?;",
            "Ok(QueueLaneRetirementObserver { queue: self, _reservation_transition_guard: guard, })")
    ordered("Queue::lane_has_pending_work_under_retirement_observer",
            "hash_is_zero(lane_incarnation) || self.transaction_selection_durability_faulted()",
            "let _queue_guard = self.push_remove_lock.lock();",
            "if self.transaction_selection_durability_faulted() { return true; }",
            "let reservations = self.lane_reservations.lock();",
            "Self::lane_retirement_reservation_snapshot(&reservations, lane_id, dataspace_id, lane_incarnation,)",
            "drop(reservations);", "let Some(owned) = owned else { return true; };",
            "self.lane_has_pending_route_work(&owned, lane_id, dataspace_id)")
    ordered("Queue::lane_retirement_reservation_snapshot",
            "let reservation_owned_hashes =", "if reservations", "return None;",
            "Some(reservation_owned_hashes)")
    ordered("Queue::lane_has_pending_route_work",
            "!reservation_owned_hashes.contains(entry.key())", "self.txs.contains_key(entry.key())",
            "entry.value().legs().into_iter().any(|leg|")
    ordered("QueueLaneRetirementObserver::try_into_cut",
            "let mutation = self.queue.push_remove_lock.try_lock_or_wait()",
            "let reservations = self.queue.lane_reservations.try_lock_or_wait()",
            "Ok(QueueLaneRetirementCut { reservations, _mutation: mutation, observer: self, })")
    require("QueueRetirementBusy", "struct QueueRetirementBusy { pub(crate) field: &'static str, pub(crate) wait: mv::ReleaseWait, }")
    ordered("QueueLaneRetirementCut",
            "reservations: PublicationGuard<'queue, LaneQueueReservationStore>",
            "_mutation: PublicationGuard<'queue>", "observer: QueueLaneRetirementObserver<'queue>")
    ordered("QueueLaneRetirementCut::lane_has_pending_work",
            "hash_is_zero(lane_incarnation) || queue.transaction_selection_durability_faulted()",
            "Queue::lane_retirement_reservation_snapshot(&self.reservations, lane_id, dataspace_id, lane_incarnation,)",
            "queue.lane_has_pending_route_work(&owned, lane_id, dataspace_id)")
    cut_probe = items.get("QueueLaneRetirementObserver::try_into_cut", "")
    # Both fallible probes must return their own pre-probe event with `?`.
    # Rust then drops the original consumed observer and every earlier guard.
    for field in ("push_remove_lock", "lane_reservations"):
        probe = _code(f'self.queue.{field}.try_lock_or_wait().map_err(|wait| QueueRetirementBusy {{ field: "{field}", wait, }})?;')
        if cut_probe and probe not in cut_probe:
            errors.append(f"Native preparation retained Queue cut loses exact {field} refusal/release relation")
    # String masking remains mandatory for ordinary executable relations.
    # For the failed-lock diagnostic identity only, find the actual producer
    # expression in offset-preserving masked code, then read that literal from
    # the original item. A comment or unrelated same-label expression cannot
    # satisfy the failed mutex's exact label relation.
    raw_cut_probe = raw_items.get("QueueLaneRetirementObserver::try_into_cut", "")
    masked_cut_probe = _mask_rust_comments(raw_cut_probe)
    for field in ("push_remove_lock", "lane_reservations"):
        expression = (
            rf"self\s*\.\s*queue\s*\.\s*{field}\s*\.\s*try_lock_or_wait\s*\(\s*\)"
            r"\s*\.\s*map_err\s*\(\s*\|wait\|\s*QueueRetirementBusy\s*\{\s*field\s*:"
            r"(?P<label>\s*),\s*wait\s*,?\s*\}\s*\)\s*\?\s*;"
        )
        matches = list(re.finditer(expression, masked_cut_probe))
        if raw_cut_probe and (len(matches) != 1 or
                raw_cut_probe[matches[0].start("label"):matches[0].end("label")].strip() != f'"{field}"'):
            errors.append(f"Native preparation retained Queue cut loses exact {field} label in refusal/release relation")
    if cut_probe and cut_probe.count(_code(".try_lock_or_wait()")) != 2:
        errors.append("Native preparation retained Queue cut must probe exactly its two inner owners")
    for symbol in ("QueueLaneRetirementObserver::try_into_cut", "QueueLaneRetirementCut::lane_has_pending_work"):
        item = items.get(symbol, "")
        if any(_code(forbidden) in item for forbidden in
               (".lock()", ".await", "mem::forget", "ManuallyDrop", "drop(self", "drop(mutation", "drop(reservations")):
            errors.append(f"Native preparation {symbol} blocks or escapes retained Queue ownership")
    ordered("KuraPublicationLease", "_sidecar: PublicationGuard<'kura>",
            "_geometry: PublicationGuard<'kura>", "_canonical: PublicationGuard<'kura>",
            "_prune: PublicationGuard<'kura>")
    for owner in (
        "Kura::apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner",
        "Kura::mark_lane_geometry_catalog_published_with_lineage_root",
    ):
        ordered(owner, "self.prune_lock.lock()", "self.ensure_prune_recovery_not_required()?;",
                "self.canonical_chain_lock.lock()", "self.resolve_canonical_storage_before_mutation()?;",
                "self.lane_geometry_lock.lock()", "self.read_lane_geometry_journal()?;",
                "self.finish_pending_lane_geometry_gc_locked(&mut journal)?;",
                "KuraPublicationLease::from_geometry_guards(", "self.sidecar_lock.lock()")
    ordered("Kura::apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner",
            "self.resolve_canonical_storage_before_mutation()?;",
            "self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;",
            "self.lane_geometry_lock.lock()", "self.sidecar_lock.lock()",
            "lease.apply_prepared_lane_geometry(")
    ordered("Kura::mark_lane_geometry_catalog_published_with_lineage_root",
            "self.sidecar_lock.lock()", "lease.publish_prepared_lane_geometry_catalog(")
    ordered("KuraPublicationLease::apply_prepared_lane_geometry",
            "PreparedGeometryJournalTransition::prepare(kura, journal, record_index)?;",
            "prepared.persist(kura, LaneGeometryPhase::Intent)?;",
            "if let Err(error) = kura.apply_geometry_operations_forward(",
            "GeometryEvidencePolicy::FreshJournalIntent",
            "if let Err(rollback_error) = kura.apply_geometry_operations_rollback(",
            "GeometryEvidencePolicy::AllowJournalIntentProvisioning",
            'kura.poison_canonical_storage("lane geometry apply rollback", &ambiguous);',
            "prepared.persist(kura, LaneGeometryPhase::RolledBack)?;", "return Err(error);",
            "prepared.persist(kura, LaneGeometryPhase::FilesApplied)?;",
            "kura.ensure_authoritative_lane_markers_with_receipts(",
            "*kura.lane_storage_entries.lock() =")
    ordered("KuraPublicationLease::publish_prepared_lane_geometry_catalog",
            "let prior_journal_bytes = kura.read_geometry_file_bytes(&journal_path)?;",
            "kura.validate_lane_geometry_journal(&journal)?;", "let published_journal_bytes = journal.encode();",
            "kura.atomic_write_geometry_file(", "if let Err(publication_error) = publication_result",
            "kura.restore_lane_geometry_journal_file(", "return Err(publication_error);")
    for owner in ("KuraPublicationLease::apply_prepared_lane_geometry",
                  "KuraPublicationLease::publish_prepared_lane_geometry_catalog",
                  "KuraPublicationLease::from_geometry_guards"):
        held = items.get(owner, "")
        for forbidden in (".prune_lock.lock(", ".canonical_chain_lock.lock(",
                          ".lane_geometry_lock.lock(", ".sidecar_lock.lock(",
                          ".try_publication_lease(", ".pending_canonical_capacity_bytes_under_prune_and_canonical_guards(",
                          ".finish_pending_lane_geometry_gc_locked("):
            if forbidden in held:
                errors.append(f"Native preparation guarded geometry {owner} reenters prelude or locking owner: {forbidden}")
    applied = items.get("KuraPublicationLease::apply_prepared_lane_geometry", "")
    if applied and applied.count(_code("kura.ensure_lane_retirement_admissible_locked(pending_canonical_bytes, &retiring, &certified_retirements,)?;")) != 2:
        errors.append("Native preparation guarded geometry must admit both retained retry and new retirement")

    ordered("publish_execution_witness",
            "try_publication_lease()", "reauthenticate_checkpoint(", "drop(lease)",
            "stage_kagemusha_finality_sidecar(", "promote_kagemusha_finality_sidecar(")
    ordered("KuraPublicationLease::reauthenticate_execution_witness",
            "decode_kagemusha_finality_sidecar(&path)", "Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)",
            "regular_sidecar_metadata(&path, &directory)", "Kura::stable_sidecar_metadata_unchanged(&read.metadata, current)", "Ok(())")
    ordered("publish_archives",
            "try_publication_lease()", "reauthenticate_checkpoint(", "drop(lease)",
            "provider.publish(receipt)", "reputation.publish(receipt)")
    ordered("try_prepare_physical",
            "admit(&original, target)", "target.matches_kura_instance(&original.journals.kura)",
            "original.publish_execution_witness()", "original.publish_archives()", "target.kura.try_publication_lease()",
            "kura.reauthenticate_checkpoint(", "kura.reauthenticate_execution_witness(original.finality.artifact())",
            "StateFences::try_acquire(target)", "journals.try_map_components(")
    ordered("PhysicallyPreparedCarrier::publish",
            "return Err((self.abort(), error))", "target.begin_state_view_write()",
            "transactions.publish()", "runtime.publish()", "world.publish()", "world_effects.publish(target)",
            "block_hashes.publish()", "target.update_latest_block_header_cache(effects.header)",
            "drop(generation)", "fences.release_for_completion()", "effects.publish_observability(target)",
            "tiered_snapshot.publish(target, false)", "drop(commit)", "Ok(PublishedCarrier {")
    terminal = items.get("PhysicallyPreparedCarrier::publish", "")
    if terminal:
        for operation in ("target.begin_state_view_write()", "transactions.publish()", "runtime.publish()",
                          "world.publish()", "block_hashes.publish()", "drop(generation)"):
            if terminal.count(_code(operation)) != 1:
                errors.append(f"Native preparation terminal lifecycle repeats or omits {operation}")
        visible_tail = terminal.split(_code("transactions.publish()"), 1)[-1]
        if "?" in visible_tail or "returnErr(" in visible_tail:
            errors.append("Native preparation terminal publication has a fallible post-write retry")
        for forbidden in (".block(", "commit_inner(", "CheckedCarrierApplications", "validate_and_prepare"):
            if forbidden in terminal:
                errors.append(f"Native preparation terminal publication reconstructs authority: {forbidden}")

    # One verified context and original pristine State owner are carried from
    # pre-writer admission into the same recorder and authenticated suffix join.
    ordered("ValidBlock::prepare_native_execution_controls",
            "ensure_state_access_without_exec_witness()", "native_lane_batch_for_execution(block)",
            "validate_proposal_commitments()", "ensure_da_indexes_hydrated()",
            "let generation = state.state_view_generation();", "let view = state.view();",
            "let expected_da_policy =", "drop(view);", "committed_nexus_amx_context_hash(state)",
            "committed_execution_policy_hash(state)", "Self::validate_npos_effects_with_state(",
            "Self::prepare_pristine_consensus_effects(", "Ok(PreparedNativeExecutionControls {")
    ordered("PreparedNativeExecutionControls::apply",
            "validate_native_pristine_control_owner(self.state, self.generation, &self.header)",
            "stage_queue_plan_admissions_for_carrier(&self.admissions)",
            "npos.apply(overlay)?;", "Ok(self.context)")
    ordered("State::with_native_lane_execution_scope",
            "ensure_state_access_without_exec_witness()", "self.block_with_owned_start_stages(",
            "preflight_lane_decision_execution_inputs(groups)", "let scope = enter(overlay)?;",
            "overlay.produce_native_execution_outputs(preflight, finish_native)?;",
            "finish_scope(overlay, result, scope)")
    ordered("State::record_native_lane_decision_batch",
            "ensure_exec_witness_capture_available()", "with_stable_observation(self, ||",
            "ValidBlock::prepare_native_execution_controls(\n                &carrier, self, context,\n            )",
            "self.prepare_lane_decision_batch(&groups)?;", "if &batch != expected",
            "self.with_native_lane_execution_scope(", "begin_exec_witness_capture()",
            "controls\n                        .apply(overlay)", "Ok((recorder, context))",
            "overlay.seal_native_lane_decision_batch(results, batch)",
            "|overlay, executions, (recorder, context)|",
            "ValidBlock::seal_native_execution_outputs(\n                        &mut carrier,\n                        overlay,\n                        &executions,\n                    )",
            "ValidBlock::finalize_native_execution_contexts(\n                        &carrier, overlay, &context,\n                    )",
            "overlay.capture_exec_witness()", "verify_execution_output_seal(&carrier)",
            "drop(recorder);", "PreparedLaneDecisionBatchV1::from_stage(overlay, executions, groups)?;")
    ordered("ValidBlock::finalize_native_execution_contexts",
            "Self::validate_staged_execution_controls(block, state)?;",
            "validate_axt_envelopes(block, state)?;", "state.validate_da_shard_cursors(block)?;",
            "Self::validate_sccp_commitment_root(block)?;",
            "finalize_lane_consensus_contexts(block, Some(context.context()))")
    ordered("PreparedNativeLaneBatchSourceV1::record_execution",
            "ensure_exec_witness_capture_available()", "if !self.is_current()",
            "if carrier != *self.input {\n            return Err(", "drop(self.input);",
            "record_native_lane_decision_batch(carrier, self.groups, context)",
            "self.generation,\n            self.state.state_view_generation()", "recorded.map(Some)")
    ordered("PreparedNativeLaneBatchSourceV1::stage_with_start_hooks",
            "ensure_state_access_without_exec_witness()", "native_lane_batch_for_scratch(&self.input)",
            "if !self.is_current()", "self.state.lane_execution_state_hash()",
            "replay_lane_decision_batch(&self.input.header(), batch, self.groups)")
    require("native_lane_batch_for_scratch",
            'if !carrier.execution_context().expect("checked Native shape").queue_plan_admissions.is_empty() || carrier.npos_consensus_effects().is_some() || carrier.header().npos_effects_hash().is_some() { return Err(')
    for symbol in ("State::record_native_lane_decision_batch", "PreparedNativeLaneBatchSourceV1::record_execution"):
        body = items.get(symbol, "")
        for forbidden in ("CheckedCarrierApplications", "groups.clone()", "commit_inner("):
            if forbidden in body:
                errors.append(f"Native preparation {symbol} has forbidden executable relation {forbidden}")

    recorded = items.get("State::record_native_lane_decision_batch")
    if recorded is not None:
        for operation in ("begin_exec_witness_capture()", "overlay.capture_exec_witness()", "drop(recorder)"):
            if recorded.count(_code(operation)) != 1:
                errors.append(f"Native preparation recorder lifecycle repeats or omits {operation}")

    require("V2ApplyService::validate_candidate",
            "let prepared = ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block(body.clone(), &topology, &self.genesis_account, &TimeSource::new_system(), self.block_cadence, crate::block::valid::SumeragiV2ValidationContext::from_height_context(context), self.state.as_ref(), &mut voting_block,)",
            "self.kura.validate_native_amx_participant_application_evidence_byte_budget(prepared.native_amx_manifest(), None,).map_err(Self::classify_native_amx_evidence_byte_budget_error)?;")
    require("ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block",
            "let context_matches = context.id() == validation_context.context_id && context.height == block.header().height().get() && context.network_id == *state.network_id_ref() && topology.as_ref().iter().eq(context.roster.iter().map(|entry| &entry.validator));",
            "let (valid, state) = Self::validate_sumeragi_v2_candidate_keep_voting_block(block, topology, genesis_account, time_source, block_cadence, validation_context, state, voting_block,).unpack(|_| {})?;",
            "crate::state::PreparedCarrier::prepare(ValidatedCarrierPreparationInput { valid, state, context, native: None, })")
    # Both finalizers consume the same complete metadata owner. The Native
    # route uses only its already-executed settlements and source-bound stage;
    # no ordinary drain/frontier, recorder reset or live Apply grant enters here.
    ordered("ValidBlock::finalize_owned_execution_metadata",
            "Self::finalize_common_execution_metadata(block, state, routes, advertised_policy, advertised_transitions,)?;",
            "Self::finalize_lane_settlement_evidence(block, state, &routed, &summaries)?;",
            "stage_ordinary_lane_frontiers(block)", "stage_canonical_carrier_membership(membership, height)",
            "resolve_queue_plan_pending_obligations_from_block(block)")
    ordered("ValidBlock::finalize_common_execution_metadata",
            "if routes.len() != block.network_entrypoint_count() { return Err(",
            "crate::tx::prune_expired_sealed_commitments(state)",
            "u64::try_from(state.committed_fragment_count())", "state.finalize_axt_asset_incarnations()",
            "evaluate_nexus_autoscale(block, fragments)", "finalize_axt_policy_transition_ratchets()",
            "let policy = state.axt_policy_snapshot();",
            "Self::validate_advertised_axt_post_state(advertised_policy, &policy)?;",
            "Self::validate_advertised_axt_transitions(advertised_transitions, state.axt_authorization_transitioned(), policy.version,)?;")
    ordered("ValidBlock::seal_native_execution_outputs",
            "verify_native_execution_metadata(block, executions)", "if block.has_results()",
            "seal_execution_outputs(block, |state, source, routes| {",
            "verify_native_execution_metadata(source, executions)",
            "Self::finalize_common_execution_metadata(source, state, routes, advertised_policy.as_ref(), advertised_transitions.as_ref(),)?;",
            "Self::native_execution_finality_statements(source, state, executions, routes)?;",
            "stage_canonical_carrier_membership(std::iter::empty::<HashOf<TransactionEntrypoint>>(), height,)",
            "Self::validated_committed_fragment_count(state, advertised_fragments)?;",
            "Ok(crate::state::ExecutionOutputSealMetadata { committed_fragment_count, lane_finality_statements, })",
            "verify_execution_output_seal(block)")
    require("ValidBlock::native_execution_finality_statements",
            "if executions.len() != routes.len() || executions.len() != block.network_entrypoint_count() { return Err(",
            "if plan.coordinator_route() != *route { return Err(",
            "if (commitment.lane_id, commitment.dataspace_id, commitment.lane_incarnation, commitment.block_height,) != (slot.route.lane_id, slot.route.dataspace_id, slot.lane_incarnation, slot.lane_height,) || iroha_data_model::nexus::compute_settlement_hash(commitment).map_err(|error| Self::execution_context_error(error.to_string()))? != execution.settlement_hash { return Err(",
            "if !Self::native_settlement_requires_relay(commitment)? { continue; }",
            "LaneRelayEnvelope::new(block.header(), block.header().da_commitments_hash(), commitment.clone(), decision.manifest.byte_len,)",
            "statements.sort_unstable_by_key(|statement| { (statement.lane_id, statement.dataspace_id, statement.lane_incarnation, statement.block_height,) });")
    ordered("ValidBlock::native_execution_finality_statements",
            "let commitment = &execution.settlement_commitment;", "!= execution.settlement_hash",
            "Self::native_settlement_requires_relay(commitment)?", "continue;", "let descriptor_hash =",
            "LaneRelayEnvelope::new(", "with_lane_block_descriptor_hash(Some(descriptor_hash))",
            "envelope.manifest_root = policy.entries.iter().find(|entry| entry.dsid == commitment.dataspace_id).map(|entry| entry.policy.manifest_root);",
            "envelope.lane_finality_statement()", "statements.push(statement);")
    require("ValidBlock::native_settlement_requires_relay",
            "let has_receipts = !commitment.receipts.is_empty() || !commitment.nexus_fee_receipts.is_empty() || !commitment.native_amx_receipts.is_empty();",
            "if !has_receipts { if !commitment.total_local_amount.is_zero() || !commitment.total_xor_due.is_zero() || !commitment.total_xor_after_haircut.is_zero() || !commitment.total_xor_variance.is_zero() || commitment.swap_metadata.is_some() { return Err(")
    ordered("ValidBlock::native_settlement_requires_relay", "if !has_receipts",
            "return Err(", "return Ok(false);", "if commitment.tx_count == 0",
            "return Err(", "Ok(true)")
    ordered("StateBlock::validate_native_output_carrier",
            "self.validate_native_lane_execution()", "self.validate_native_output_source(block)")
    require("StateBlock::validate_native_output_source",
            "self.validate_native_lane_stage_membership()",
            "if seal.completed_write_set_root.is_none() || block.header() != seal.carrier || block.header().npos_effects_hash() != seal.npos_effects_hash || block.execution_context().map(|bundle| HashOf::new(&bundle.queue_plan_admissions)) != Some(seal.queue_plan_admissions_hash) || !block.external_entrypoints_slice().is_empty() || block.execution_context().and_then(|context| context.native_lane_decisions.as_deref()) != Some(seal.batch.as_ref()) { return Err(")
    require("StateBlock::verify_native_execution_metadata",
            "self.validate_native_output_carrier(block)?; if !self.settlement_accumulator.is_empty() { return Err(",
            "if executions.len() != seal.batch.groups.len() || executions.len() != seal.settlement_hashes.len() { return Err(",
            "if execution.source != *source || execution.authenticated_signed_replay_alias != *alias || execution.settlement_hash != *settlement || super::canonical_merge_settlement_hash(&execution.settlement_commitment).map_err(|error| error.to_string())? != *settlement { return Err(")
    for symbol in ("ValidBlock::seal_native_execution_outputs", "ValidBlock::native_execution_finality_statements"):
        body = items.get(symbol, "")
        for forbidden in ("stage_ordinary_lane_frontiers", "finalize_lane_settlement_evidence", "drain_lane_execution_settlement", "execute_and_record_canonical_outputs", "exec_witness_guard", "start_block(", "CheckedCarrierApplications"):
            if forbidden in body:
                errors.append(f"Native preparation {symbol} has forbidden executable relation {forbidden}")
    common = items.get("ValidBlock::finalize_common_execution_metadata", "")
    if common and common.find("finalize_axt_policy_transition_ratchets()") < common.find("evaluate_nexus_autoscale(block,fragments)"):
        errors.append("Native preparation common policy metadata before autoscale")
    ordered("ValidBlock::prepare_native_candidate", "ensure_state_access_without_exec_witness()",
            "source.preparation_input()", "verify_origin_block_signature(",
            "Self::validate_static_state_dependent(", "Self::validate_static_with_snapshot(",
            "if generation != state.state_view_generation()", "source.record_execution(body, context)?",
            "recorded.into_preparation_parts()", "PreparedCarrier::prepare(ValidatedCarrierPreparationInput")
    ordered("RecordedNativeLaneBatchV1::into_preparation_parts", "self.prepared\n            .verify_source_binding()",
            "verify_execution_output_seal(&self.carrier)", "validate_native_output_source(&self.carrier)",
            "let seal = Arc::clone(", "= self.prepared", "NativeExecutionCustody")
    require("PreparedCarrier::prepare", "execution_prefix::prepare(input)")
    require("PrefixPreparation::capture",
            "let block = valid.as_ref();",
            "let witness = state.exec_witness.as_ref().ok_or(",
            "let manifest = exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, None,)?;",
            "let commitment = exec::execution_commitment_from_validated_block(witness, &manifest, &lanes, block).map_err(str::to_owned)?;",
            "let inventory = state.fastpq_source_inventory.take().ok_or(",
            "let witness = state.exec_witness.take().ok_or(",
            "let prefix = ValidatedExecutionPrefix { sealed, authority, inventory, witness, fastpq_witness_context: state.fastpq_witness_context.take(), parliament_timed_ovn_casting_bindings: state.parliament_timed_ovn_casting_bindings.take(), };")
    ordered("PrefixPreparation::capture", "let authority = match native",
            "state.verify_execution_output_seal(block)?;",
            "state.verify_cached_ordinary_witness_content(&verified_inventory)?;",
            "let manifest =", "let commitment =", ".replace(output_capacity::ExecutionOutputPlanState::Captured)",
            "let inventory =", "let witness = state.exec_witness.take()", "let prefix =")
    ordered("prepare", "let (valid, state, context, native) = input.into_parts();",
            "PrefixPreparation::capture(state, &valid, native)?;", "prepare_deterministic_carrier_metadata(",
            "preparation.prepare_world_effects()?;", "prepare_carrier_publication_events(block.header())",
            "PreparedTieredSnapshot::prepare(", "let PrefixPreparation { state, prefix: source_prefix, } = preparation;",
            "Ok(PreparedCarrier { valid, state, source_prefix, context, execution_prefix, native_amx_manifest,")
    ordered("PreparedCarrier::prepare_journals", "let admission;", "let Self {",
            "admit_journals(CarrierJournalInputs { state: &state, prefix: &source_prefix, })",
            "state.prepare_carrier_geometry()?;", "world.try_detach_journals(", "Ok(PreparedCarrierJournals {")
    require("StateBlock::seal_execution_outputs", "Ok(SealedExecutionOutputs { witness_hash: None, witness_surface: None, sources, world_delta,")
    if "prepare" in items:
        body = items["prepare"]
        capture = body.find(_code("PrefixPreparation::capture(state, &valid, native)?;"))
        tail = body.find(_code("prepare_deterministic_carrier_metadata("))
        if capture < 0 or tail < capture:
            errors.append("Native preparation stages metadata before prefix capture")

    # Type privacy and consuming field layout are part of the owner boundary;
    # no detached marker, caller-supplied scalar or mutable State accessor suffices.
    for symbol in ("ValidatedExecutionPrefix", "PrefixPreparation", "SealedExecutionOutputs",
                   "PreparedNativeExecutionControls", "PreparedNativeLaneBatchSourceV1",
                   "FinalizedNativeLaneBatchV1", "RecordedNativeLaneBatchV1", "NativeExecutionCustody"):
        if symbol in items and re.search(r"(?:\{|,)pub(?:\([^)]*\))?\w+:", items[symbol]):
            errors.append(f"Native preparation {symbol} exposes mutable/forgeable owner fields")

    require("Kura::native_amx_publication_plan_under_prune_and_canonical_guards",
            "self.native_amx_publication_plan_for_storage_under_prune_and_canonical_guards(block, merge_entry, NativeAmxPublicationStorage::Active,)")
    require("Kura::native_amx_publication_plan_for_storage_under_prune_and_canonical_guards",
            "native_amx_participant_application_artifacts(&manifest, native_amx_participant_application_finality_placeholder_hash(),)",
            "for (manifest, receipt) in &artifacts { if let Some((route, capacity)) = self.native_amx_route_publication_capacity_for_storage_locked(manifest, receipt, storage,)? { if routes.insert(route, capacity).is_some() { return Err(",
            "let carrier = NativeAmxPublicationCarrier { height: block.header().height().get(), block_hash: block.hash(), executed_wire_hash: manifest.executed_block_wire_hash(), };")
    require("Kura::native_amx_route_publication_capacity_for_storage_locked",
            "NativeAmxPublicationStorage::Active => { let entry = self.lane_storage_entry(descriptor.lane_id)?; self.native_amx_route_publication_capacity_at_target_locked(&entry, manifest, receipt,) }",
            "NativeAmxPublicationStorage::JournalPhysical => { let target = self.native_amx_reservation_physical_target_from_journal(descriptor)?; let result = self.native_amx_route_publication_capacity_at_target_locked(&target, manifest, receipt,)?; self.require_native_amx_reservation_physical_target(&target)?; Ok(result) }")
    require("Kura::native_amx_route_publication_capacity_at_target_locked",
            "let route = NativeAmxPublicationRoute { lane_id: descriptor.lane_id, dataspace_id: descriptor.dataspace_id, incarnation: descriptor.lane_incarnation, };",
            "NativeAmxPublicationComponent::Manifest, u64::try_from(expected_manifest.encode_framed()?.len())?",
            "NativeAmxPublicationComponent::Receipt, u64::try_from(expected_receipt.encode_framed()?.len())?",
            "NativeAmxPublicationComponent::Latest, u64::try_from(norito::encode_canonical(&expected_latest)?.len())?",
            "if temporary_manifests.contains_key(&height) { component_allocation_bytes.insert(NativeAmxPublicationComponent::Manifest, 0); }",
            "if temporary_receipts.contains_key(&height) { component_allocation_bytes.insert(NativeAmxPublicationComponent::Receipt, 0); }",
            "if latest_temporary.is_some() { component_allocation_bytes.insert(NativeAmxPublicationComponent::Latest, 0); }",
            "for (kind, bytes) in &mut component_allocation_bytes { if !outstanding_components.contains(kind) { *bytes = 0; } }",
            "if intent.protected_latest.identity != expected_latest { return Err(",
            "Self::plan_native_amx_evidence_prune_intent_from_artifacts(self.native_amx_participant_evidence_retention(), self.native_amx_participant_evidence_file_bytes(), self.native_amx_evidence_prune_intent_max_bytes(), &manifests, &receipts,)?")
    require("Kura::admit_native_amx_publication_capacity_plan",
            "if *other_carrier != carrier && Some(*other_carrier) != replaced && plan.routes.keys().any(|route| other.routes.contains_key(route)) { return Err(",
            "if reservations.len() >= 2 * iroha_data_model::nexus::MAX_ACTIVE_EXECUTION_LANES && created { return Err(")
    require("Kura::check_storage_budget",
            "let mut budget_used = used.saturating_add(pending_bytes).saturating_add(lane_publication_reservations).saturating_add(certified_bundle_reservations).saturating_add(autonomous_terminal_reservations).saturating_add(prune_maintenance_headroom);")
    # Both fresh and exact existing block branches retain capacity before writes.
    ordered("Kura::store_block_durable", "self.ensure_existing_block_wire_matches(block, actual_height, block_hash)?;",
            "begin_native_amx_store_capacity_under_prune_and_canonical_guards(block, merge_entry, None,)?;",
            "self.check_native_amx_existing_carrier_capacity_under_prune_and_canonical_guards()?;",
            "owner.publish_pending_index()?;", "return Ok(());",
            "begin_native_amx_store_capacity_under_prune_and_canonical_guards(block, merge_entry, None,)?;",
            "self.check_storage_budget(block, merge_entry)?;", "owner.publish_pending_index()?;")
    # The terminal owner consumes the sum of separate post-WSV and Native
    # reservations. Keep the original post-WSV obligation at its actual sum owner.
    terminal = "Kura::validate_configured_autonomous_mutation_disk_peak_with_reservation_deltas_locked"
    require(terminal,
            'let lane_publication_reservations = self.lane_publication_budget_reserved_bytes()?;',
            'let certified_bundle_reservations = self.certified_bundle_capacity_reserved_bytes()?;',
            'let required = self.kura_disk_usage_bytes()?.checked_add(pending_canonical_bytes).and_then(|bytes| bytes.checked_add(additional_unreserved_stable_bytes)).and_then(|bytes| bytes.checked_add(physical_and_transient)).and_then(|bytes| bytes.checked_add(stable_terminal_reservations)).and_then(|bytes| bytes.checked_add(lane_publication_reservations)).and_then(|bytes| bytes.checked_add(certified_bundle_reservations)).and_then(|bytes| { bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes()) }).ok_or_else(|| { Self::invalid_lane_artifact_error(path.to_path_buf(), "autonomous mutation configured disk accounting overflowed",) })?;',
            'if required > self.max_disk_usage_bytes { return Err(Self::invalid_lane_artifact_error(path.to_path_buf(), "autonomous mutation would consume globally reserved terminal or carrier capacity",)); } Ok(())',
            'if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() { return Ok(()); }',
    )
    terminal_item = items.get(terminal)
    if terminal_item is not None and (
        terminal_item.count(_code("return Ok(());")) != 1
        or terminal_item.count(_code("let required =")) != 1
    ):
        errors.append("Native preparation terminal capacity has an early success or replaced total")
    ordinary = items.get("lane_artifact_required_bytes_for_block", "")
    for forbidden in ("native_amx", "NativeAmx", "lane_publication_budget_reserved_bytes"):
        if forbidden in ordinary:
            errors.append(f"Native preparation ordinary accounting duplicates Native reservation via {forbidden}")
