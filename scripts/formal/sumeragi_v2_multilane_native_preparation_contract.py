"""Exact prepared execution and separate Native capacity ownership.

These bindings describe existing execution-prefix preparation and disk accounting.
They do not claim a complete prepared State publisher, aggregate pre-vote resource admission,
local-resource deferral, or historical Native target authority.
"""
from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable

from sumeragi_v2_multilane_geometry_evidence_contract import _code
from sumeragi_v2_multilane_reviewed_rust_source import (
    _mask_rust_comments, _read_reviewed_rust_source,
)

MODEL = "SumeragiV2NativeApplicationEvidence"
STATE = "crates/iroha_core/src/state.rs"
HASH_RESTORE = "crates/iroha_core/src/state/deserialize_core.rs"
RUNNER_HISTORY = "crates/iroha_core/src/sumeragi/v2_runner.rs"
LANE_WORK_HISTORY = "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
HASH_ADMISSION = "crates/iroha_core/src/state/block_hashes_admission.rs"
RUNTIME_ACQUISITION = "crates/iroha_core/src/state/canonical_runtime.rs"
HASH_PUBLICATION = "crates/iroha_core/src/state/block_hashes_publication.rs"
HASH_SURFACE = "crates/iroha_core/src/state/output_publication.rs"
APPLY = "crates/iroha_core/src/sumeragi/v2_apply.rs"
BLOCK = "crates/iroha_core/src/block/carrier_preparation.rs"
PREPARED = "crates/iroha_core/src/state/carrier_preparation.rs"
PREFIX = "crates/iroha_core/src/state/carrier_preparation/execution_prefix.rs"
JOURNALS = "crates/iroha_core/src/state/carrier_preparation/journals.rs"
WORLD_COMMIT = "crates/iroha_core/src/state/world_commit.rs"
DECISION_CARRIER = "crates/iroha_core/src/state/carrier_preparation/decision_binding.rs"
VALIDATION_CUSTODY = "crates/iroha_core/src/sumeragi/v2_apply/validation_custody.rs"
RETAINED_VALIDATION = "crates/iroha_core/src/sumeragi/v2_body_store/retained_validation.rs"
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
        "sealed: output_capacity::SealedExecutionOutputs", "_inventory: Arc<FastpqSourceInventoryV1>",
        "witness: ExecWitness", "_fastpq_witness_context: Option<crate::fastpq::FastpqWitnessContext>",
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
        "!state\n                .block_hashes\n                .pending()\n                .iter()\n                .copied()\n                .eq([state._curr_block.hash()])",
        "state.validate_canonical_runtime_projection()?", "state.verify_lane_consensus_contexts_publication()?",
        "validate_merge_carrier_entrypoint_binding()", "finalize_axt_asset_incarnations()",
        "finalize_axt_policy_transition_ratchets()", "state.prune_axt_replay_ledger(",
        "validate_owned_runtime_catalog_overlay()", "ensure_pending_autoscale_lifecycle_staking_is_safe(",
        "world_commit::PreparedWorldCommit::prepare_overlay(",
    )),
    (PREFIX, "fn", "prepare", (
        "input.into_parts()", "PrefixPreparation::capture(state, &valid, native)?",
        "prepare_deterministic_carrier_metadata", "preparation.prepare_world_effects()?",
        "prepare_carrier_publication_events(block.header())",
        "prefix: source_prefix", "Ok(PreparedCarrier {", "Err(error) => Err((Box::new(valid.into()), error))",
    )),
    (JOURNALS, "struct", "CarrierJournalInputs", (
        "pub(crate) valid:", "pub(crate) state:", "pub(crate) prefix:",
        "pub(crate) context:", "pub(crate) execution_prefix:", "pub(crate) native_amx_manifest:",
        "pub(crate) da_pins:", "pub(crate) publication_events:",
        "pub(crate) provider:", "pub(crate) reputation:", "pub(crate) retained_effects_layout:",
    )),
    (WORLD_COMMIT, "struct", "PreparedWorldEffects", ("da_pins: Vec<DaPinIntentWithLocation>",)),
    (WORLD_COMMIT, "method", "PreparedWorldEffects::admission_pins", (
        "&self", "-> &Vec<DaPinIntentWithLocation>", "let Self { da_pins } = self;", "da_pins",
    )),
    (JOURNALS, "method", "PreparedCarrier::prepare_journals", (
        "admit_journals: impl FnOnce(CarrierJournalInputs<'_, 'state>)",
        "} = &self;",
        "let admission = match admit_journals(CarrierJournalInputs {",
        "prefix: source_prefix", "da_pins: _world_effects.admission_pins()",
        "publication_events: _publication_events",
        "provider: provider_capture.as_ref()", "reputation: reputation_capture.as_ref()",
        "retained_effects_layout: std::alloc::Layout::new::<RetainedCarrierEffects>()",
        "Box::new(RetainedCarrierEffects {",
        'CarrierJournalPreparationError::JournalAdmission {\n                    carrier: self,\n                    provider: provider_capture,\n                    reputation: reputation_capture,\n                    error,\n                }',
        "let mut provider_capture = provider_capture;", "let mut reputation_capture = reputation_capture;",
        'PreparedTieredSnapshot::prepare(\n            &state.world,\n            &state.state_ref.tiered_snapshot_worker,\n        )',
        "state.prepare_carrier_geometry()?", "owner.capture_original(state.as_ref())",
        "world.try_detach_journals", "transactions.prepare_commit()?.detach()",
        "let journals = PreparedCarrierJournals {", "admission,", "StagedCarrierCapture {",
        "carrier.try_prepare_archives()", "carrier: Box::new(carrier)", "Ok(carrier.into_journals())",
    )),
    (JOURNALS, "method", "StagedCarrierCapture::try_complete", (
        "mut self: Box<Self>", "if let Err(error) = self.try_prepare_archives()",
        "return Err((self, error));", "Ok((*self).into_journals())",
    )),
    (JOURNALS, "method", "StagedCarrierCapture::try_prepare_archives", (
        "if let Some(error) = &self.capture_refusal", "return Err(error.clone());",
        'provider\n                .try_prepare()', 'reputation\n                .try_prepare()',
        'if let Some(provider) = &mut self.provider {\n            provider\n                .try_prepare()\n                .map_err(|error| CarrierArchivePreparationError::Provider(Arc::new(error)))?;\n        }',
        'if let Some(reputation) = &mut self.reputation {\n            reputation\n                .try_prepare()\n                .map_err(|error| CarrierArchivePreparationError::Reputation(Arc::new(error)))?;\n        }',
    )),
    (JOURNALS, "method", "StagedCarrierCapture::into_journals", (
        "self.journals.provider_capture = self.provider.take()", "self.journals.reputation_capture = self.reputation.take()",
        'owner\n                .into_prepared()', "self.journals",
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
        "old.participant_height != new.participant_height", "old.proposal_hash != new.proposal_hash",
        "old.settlement_hash != new.settlement_hash",
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
# Private terminal publication consumes the original geometry and lifecycle.
# Retirement additionally retains the original service Queue proof through visibility;
# independent Kura and participant authority remain mandatory. This does not
# activate production Validate/Apply or establish aggregate resource admission.
PHYSICAL_CARRIER = "crates/iroha_core/src/state/carrier_preparation/physical_publication.rs"
TERMINAL_CARRIER = "crates/iroha_core/src/state/carrier_preparation/publication.rs"
ARCHIVE_CARRIER = "crates/iroha_core/src/state/carrier_preparation/archive_publication.rs"
GEOMETRY_CARRIER = "crates/iroha_core/src/state/carrier_geometry_preparation.rs"
WITNESS_CARRIER = "crates/iroha_core/src/state/carrier_preparation/execution_witness_publication.rs"
WITNESS_LEASE = "crates/iroha_core/src/kura/publication_lease.rs"
SERVICE_QUEUE = "crates/iroha_core/src/sumeragi/v2_apply/carrier_queue_retirement.rs"
CARRIER_QUEUE = "crates/iroha_core/src/state/carrier_preparation/queue_retirement.rs"
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
        "provider\n                .publish_under_publication_lease(&lease, receipt)",
        "reputation\n                .publish_under_publication_lease(&lease, receipt)",
    )),
    (PHYSICAL_CARRIER, "method", "SourceAuthenticatedCarrier::try_new", (
        "let owner = Self { decision, kura };", "let original = &owner.decision;",
        'owner\n                .kura\n                .reauthenticate_checkpoint(\n                    &original.checkpoint,\n                    original.finality.artifact(),\n                    original.journals.checkpoint,\n                )',
        'original\n                .journals\n                .source_prefix\n                .authenticate_durable_carrier(\n                    original.block(),\n                    &original.journals.context,\n                    &original.journals.execution_prefix,\n                    &owner.kura,\n                )',
        "original.journals.provider_capture.as_ref()", "original.journals.reputation_capture.as_ref()",
        'capture\n                    .reauthenticate_under_publication_lease(\n                        &owner.kura,\n                        original.checkpoint.finality_receipt(),\n                    )',
        "Err(error) => Err((owner.release(), error))",
        'if let Some(capture) = original.journals.provider_capture.as_ref() {\n                capture\n                    .reauthenticate_under_publication_lease(\n                        &owner.kura,\n                        original.checkpoint.finality_receipt(),\n                    )\n                    .map_err(CarrierPhysicalPreparationError::Provider)?;\n            }',
        'if let Some(capture) = original.journals.reputation_capture.as_ref() {\n                capture\n                    .reauthenticate_under_publication_lease(\n                        &owner.kura,\n                        original.checkpoint.finality_receipt(),\n                    )\n                    .map_err(CarrierPhysicalPreparationError::Reputation)?;\n            }',
    )),
    (PHYSICAL_CARRIER, "fn", "try_prepare_physical", ('Err((runtime, error, runtime_retirement)) => {\n                        let (transactions, transactions_retirement) = transactions.abort();\n                        let (block_hashes, block_hashes_retirement) = block_hashes.abort();\n                        drop(fences.release_for_completion());\n                        drop((\n                            runtime_retirement,\n                            transactions_retirement,\n                            block_hashes_retirement,\n                        ));', 'Err((world, error, world_retirement)) => {\n                    let (runtime, runtime_retirement) = runtime.abort();\n                    let (transactions, transactions_retirement) = transactions.abort();\n                    let (block_hashes, block_hashes_retirement) = block_hashes.abort();\n                    drop(fences.release_for_completion());\n                    drop((\n                        world_retirement,\n                        runtime_retirement,\n                        transactions_retirement,\n                        block_hashes_retirement,\n                    ));', 'admit: impl FnOnce(&Self, &State)', 'admit(&original, target)', 'if !original\n            .journals\n            .geometry\n            .matches_publication_target(target, original.block().header())\n        {\n            drop(installation);\n            return Err((original, CarrierPhysicalPreparationError::ForeignTarget));\n        }', 'target.matches_kura_instance(&original.journals.kura)', 'original.publish_execution_witness()', 'original.publish_archives()', 'target.kura.try_publication_lease()', 'SourceAuthenticatedCarrier::try_new(original, kura)', 'reauthenticate_execution_witness(authenticated.decision.finality.artifact())', 'if original.journals.geometry.requires_queue_custody()', 'None => Some(CarrierQueueRetirementError::Missing)', 'Some(source) if !source.belongs_to(target)', 'Some(CarrierQueueRetirementError::ForeignState)', 'match source.try_observe()', 'StateFences::try_acquire(target)', '.try_into_cut()', 'CarrierQueueRetirement::try_new(\n                            target,\n                            &authenticated.decision.journals.geometry,\n                            authenticated.decision.block().header(),\n                            source,\n                            cut,\n                        )', '_queue: queue', 'journals.try_map_components(', 'Ok(PhysicallyPreparedCarrier {\n                target,\n                decision: retain!(journals),\n                installation,\n            })', 'Err((error, state_retirement)) => {\n                let queue_retirement = queue_observer.map(|observer| observer.release_deferred());\n                let SourceAuthenticatedCarrier {\n                    decision: original,\n                    kura,\n                } = authenticated;\n                let kura_retirement = kura.release_deferred();\n                drop((state_retirement, queue_retirement, kura_retirement));\n                drop(installation);\n                return Err((original, error));', 'Err((error, queue_retirement)) => {\n                        let state_retirement = state.release_deferred();\n                        let SourceAuthenticatedCarrier {\n                            decision: original,\n                            kura,\n                        } = authenticated;\n                        let kura_retirement = kura.release_deferred();\n                        drop((queue_retirement, state_retirement, kura_retirement));\n                        drop(installation);\n                        return Err((original, CarrierPhysicalPreparationError::Queue(error)));')),
    (GEOMETRY_CARRIER, "method", "PreparedCarrierGeometry::is_identity_transition", (
        "self._header == header", "self._pending.is_none()", "self._certified_frontiers.is_empty()",
        "self._previous_runtime_catalog == self._accepted_runtime_catalog",
        "self._predecessor.lanes == self._successor.lanes",
        "self._predecessor.lane_count == self._successor.lane_count",
        "self._predecessor.lane_incarnation_lineage\n                == self._successor.lane_incarnation_lineage",
        "self._predecessor.owner_policy == self._successor.owner_policy",
        "self._predecessor.autoscale_last_transition_height\n                == self._successor.autoscale_last_transition_height",
    )),
    (GEOMETRY_CARRIER, "struct", "PreparedCarrierGeometry", (
        "_pending: Option<PendingAutoscaleLaneLifecycle>",
        "raw: Option<crate::kura::RawGeometryAttempt>",
        "tiered: Option<tiered::TieredGeometryAttempt>",
        "state_owner: NativeLaneStateOwner", "kura: Arc<Kura>",
    )),
    (GEOMETRY_CARRIER, "method", "StateBlock::prepare_carrier_geometry", (
        "self.validate_canonical_runtime_projection()", "self.canonical_runtime.get_before_block()",
        "self.canonical_runtime.get()", "_header: self._curr_block",
        "_pending: self.pending_autoscale_lifecycle.clone()", "raw: None", "tiered: None",
        "state_owner: self.state_ref.native_lane_state_owner().ok_or_else(|| {\n                LaneLifecycleError::RuntimeCatalog(\"read-only State cannot publish geometry\".into())\n            })?",
        "kura: Arc::clone(&self.state_ref.kura)",
    )),
    (GEOMETRY_CARRIER, "method", "PreparedCarrierGeometry::matches_publication_target", (
        "self._header == header && self.state_owner.matches_state(target)",
    )),
    (GEOMETRY_CARRIER, "method", "PreparedCarrierGeometry::has_pending_lifecycle", (
        "self._pending.is_some()",
    )),
    (GEOMETRY_CARRIER, "method", "PreparedCarrierGeometry::requires_storage_transition", (
        "self._pending", ".as_ref()", ".is_some_and(|pending| pending.transition.requires_geometry())",
    )),
    (GEOMETRY_CARRIER, "method", "PreparedCarrierGeometry::requires_queue_custody", (
        "self._pending.as_ref().is_some_and(|pending|",
        "!pending.plan.retire.is_empty() || !pending.catalog_update.replaced_lane_ids.is_empty()",
    )),
    (GEOMETRY_CARRIER, "method", "PreparedCarrierGeometry::complete_under", (
        "target: &State", "header: BlockHeader",
        "lease: &'geometry crate::kura::KuraPublicationLease<'kura>",
        "queue: Option<&carrier_preparation::queue_retirement::CarrierQueueRetirement<'_>>",
        "if !self.matches_publication_target(target, header)", "if !self.has_queue_custody(target, header, queue)",
        "return Err(LaneLifecycleError::Storage(", "self.resume_under(backend, lease)?;",
        "if let Some(raw) = &mut self.raw", "raw.phase() != crate::kura::RawGeometryPhase::CatalogPublished",
        "raw.publish_catalog_under(lease, None)", "raw.reauthenticate_catalog_under(lease)",
        "Ok(CompletedCarrierGeometry {\n            geometry: self,\n            _lease: lease,\n        })",
    )),
    (GEOMETRY_CARRIER, "struct", "CompletedCarrierGeometry", (
        "geometry: &'geometry PreparedCarrierGeometry",
        "_lease: &'geometry crate::kura::KuraPublicationLease<'kura>",
    )),
    (GEOMETRY_CARRIER, "method", "CompletedCarrierGeometry::updated_da_mapping", (
        "self.geometry", "._pending", ".as_ref()",
        ".filter(|pending| pending.transition.requires_geometry())",
        ".map(|pending| &pending.catalog_update.updated_lane_config)",
    )),
    (PHYSICAL_CARRIER, "method", "PhysicallyPreparedCarrier::try_complete_geometry", (
        "let journals = &mut self.decision.journals;",
        "if !journals.geometry.requires_storage_transition() {\n            return Ok(false);\n        }",
        "self\n            .target\n            .tiered_backend\n            .try_lock_or_wait()",
        '.map_err(|wait| crate::state::LaneLifecycleError::PublicationBusy {\n                field: "tiered_backend",\n                wait,\n            })?;',
        "journals\n            .geometry\n            .prepare_under(&backend, &journals.components._fences._kura)?;",
        "journals.geometry.complete_under(\n            self.target,\n            journals.effects.header,\n            &mut backend,\n            &journals.components._fences._kura,\n            journals.components._fences._queue.as_ref(),\n        )?",
        "Ok(completed.updated_da_mapping().is_some())",
    )),
    (PHYSICAL_CARRIER, "struct", "CarrierFences", (
        "_state: StateFences<'target>", "_queue: Option<CarrierQueueRetirement<'target>>",
        "_kura: KuraPublicationLease<'target>",
    )),
    (PHYSICAL_CARRIER, "method", "CarrierFences::release_for_completion", (
        "write.release_deferred()", "lifecycle.release_deferred()",
        "queue.map(CarrierQueueRetirement::release_deferred)", "kura.release_deferred()",
        "CompletionFences {", "_commit: commit", "_state: state", "_queue: queue", "_kura: kura",
    )),
    (TERMINAL_CARRIER, "method", "PhysicallyPreparedCarrier::publish", (
        "queue.ensure_available().err()", "Some(CarrierPublicationError::QueueRetirement(error))",
        "journals.effects.replay_prevalidation", "journals\n            .source_prefix\n            .retains_carrier(journals.valid.as_ref(), &journals.context)",
        "journals.effects.header != journals.valid.as_ref().header()", "journals.staged_legacy_source()",
        "journals\n            .geometry\n            .matches_publication_target(self.target, journals.effects.header)",
        "journals.geometry.has_pending_lifecycle() != journals.effects.lifecycle.is_some()",
        "!journals.geometry.has_queue_custody(\n            self.target,\n            journals.effects.header,\n            journals.components._fences._queue.as_ref(),\n        )", "Some(CarrierPublicationError::QueueRetirementRequired)",
        'journals\n            .components\n            ._fences\n            ._queue\n            .as_ref()\n            .and_then(|queue| queue.ensure_available().err())',
        'self\n            .decision\n            .journals\n            .components\n            ._fences\n            ._queue\n            .as_ref()\n            .and_then(|queue| queue.ensure_available().err())',
        "CarrierPublicationError::QueueRetirement(error)",
        "journals.native_amx_manifest.entries().is_empty()", "return Err((self.abort(), error))",
        "let update_da_mapping = match self.try_complete_geometry()",
        "return Err((\n                    self.abort(),\n                    CarrierPublicationError::GeometryStorage(error),\n                ))",
        "target.begin_state_view_write()",
        "transactions.publish()", "runtime.publish()", "world.publish()", "world_effects.publish(target)",
        "if update_da_mapping", "target\n                .da_shard_cursors\n                .write()\n                .sync_mapping(&effects.nexus.lane_config)",
        "let lifecycle_post_publication = effects\n            .lifecycle\n            .take()\n            .map(|effects| effects.publish(target, &generation, true))",
        "let da_post_publication = effects\n            .da_commitments\n            .take()\n            .map(|effects| effects.publish(target, &generation, true))",
        "target.install_sccp_registry_cache(std::sync::Arc::clone(&effects.sccp_registry))",
        "hash_retirement = block_hashes.publish()", "target.update_latest_block_header_cache(effects.header)",
        "drop(generation)", "if let Some(post) = da_post_publication {\n            post.publish(target);\n        }",
        "if let Some(post) = lifecycle_post_publication {\n            post.publish(target);\n        }",
        "fences.release_for_completion()", "effects.publish_observability(target)",
        "target.hydrate_verified_lane_relay_records(effects.verified_lane_relay_records)",
        "tiered_snapshot.publish(target, false)", "target.enforce_nexus_storage_budget(height)",
        "target.persist_query_index_status(height, Some(effects.header.hash()))",
        "publication_events.append(&mut extra_events)", "drop(commit)", "drop(membership_retirement)", "drop(hash_retirement)", "Ok(PublishedCarrier {",
        "source: source_prefix", "_admission: admission,\n            _binding: binding,\n            _installation: installation",
    )),
)
PREPARATION_OWNER_BINDINGS += TERMINAL_OWNER_BINDINGS

QUEUE_OWNER = "crates/iroha_core/src/queue.rs"
PUBLICATION_MUTEX = "crates/iroha_core/src/publication_lock.rs"
GEOMETRY_OWNER = "crates/iroha_core/src/kura/lane_geometry.rs"
RAW_GEOMETRY = "crates/iroha_core/src/kura/lane_geometry/raw_attempt.rs"
# Queue custody observes local readiness only. Original service identity and the
# retained exact route proof join this owner to publication; Kura and lifecycle
# authority independently authorize retirement, never local Queue emptiness.
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
        "Result<QueueLaneRetirementObserver<'_>, concread::release::ReleaseWait>",
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
    (QUEUE_OWNER, "method", "QueueLaneRetirementObserver::try_into_cut", ('fn try_into_cut(\n        self,\n    ) -> Result<QueueLaneRetirementCut<\'queue>, (QueueRetirementBusy, QueueRetirementCleanup)> {\n        let mutation = match self.queue.push_remove_lock.try_lock_or_wait() {\n            Ok(guard) => guard,\n            Err(wait) => {\n                return Err((\n                    QueueRetirementBusy {\n                        field: "push_remove_lock",\n                        wait,\n                    },\n                    QueueRetirementCleanup {\n                        released: [None, None, Some(self.release_deferred())],\n                    },\n                ));\n            }\n        };\n        let reservations = match self.queue.lane_reservations.try_lock_or_wait() {\n            Ok(guard) => guard,\n            Err(wait) => {\n                let mutation = mutation.release_deferred();\n                let transition = self.release_deferred();\n                return Err((\n                    QueueRetirementBusy {\n                        field: "lane_reservations",\n                        wait,\n                    },\n                    QueueRetirementCleanup {\n                        released: [None, Some(mutation), Some(transition)],\n                    },\n                ));\n            }\n        };\n        Ok(QueueLaneRetirementCut {\n            reservations,\n            _mutation: mutation,\n            observer: self,\n        })\n    }',)),
    (QUEUE_OWNER, "struct", "QueueRetirementBusy", (
        "pub(crate) field: &'static str", "pub(crate) wait: concread::release::ReleaseWait",
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
        "inner: parking_lot::Mutex<T>", "released: concread::release::ReleaseNotification",
    )),
    (PUBLICATION_MUTEX, "struct", "PublicationGuard", (
        "inner: concread::release::ReleaseGuard<'state, PhysicalPublicationGuard<'state, T>>",
    )),
    (PUBLICATION_MUTEX, "method", "PublicationMutex::new", (
        "fn new(value: T) -> Self", "inner: parking_lot::Mutex::new(value)",
        "released: concread::release::ReleaseNotification::default()",
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
    (WITNESS_LEASE, "struct", "KuraPublicationLease", ("kura: &'kura Kura", 'pending_canonical_bytes: u64', "fences: AcquiredKuraPublicationFences<'kura>")),
    (WITNESS_LEASE, "method", "Kura::try_publication_lease", ('fn try_publication_lease(\n        &self,\n    ) -> Result<KuraPublicationLease<\'_>, KuraPublicationPreparationError> {\n        fn acquire<\'kura>(\n            field: &\'static str,\n            lock: &\'kura PublicationMutex,\n        ) -> Result<PublicationGuard<\'kura>, KuraPublicationPreparationError> {\n            lock.try_lock_or_wait()\n                .map_err(|wait| KuraPublicationPreparationError::Busy { field, wait })\n        }\n        // Canonical poisoning is permanent for this Kura. It cannot become a\n        // lock-release dependency, even when another physical owner is busy.\n        self.ensure_canonical_storage_not_poisoned()\n            .map_err(KuraPublicationPreparationError::Storage)?;\n        let mut fences = AcquiredKuraPublicationFences::new(self);\n        fences.prune = Some(acquire("prune_lock", &self.prune_lock)?);\n        // Active pruning also sets this flag while it owns prune_lock. Only\n        // classify it as restart-required after acquiring that actual owner.\n        self.ensure_prune_recovery_not_required()\n            .map_err(KuraPublicationPreparationError::Storage)?;\n        fences.canonical = Some(acquire("canonical_chain_lock", &self.canonical_chain_lock)?);\n        let pending_canonical_bytes = self\n            .try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards(&mut fences)?;\n        fences.geometry = Some(acquire("lane_geometry_lock", &self.lane_geometry_lock)?);\n        fences.sidecar = Some(acquire("sidecar_lock", &self.sidecar_lock)?);\n        self.ensure_prune_recovery_not_required()\n            .map_err(KuraPublicationPreparationError::Storage)?;\n        self.ensure_canonical_storage_not_poisoned()\n            .map_err(KuraPublicationPreparationError::Storage)?;\n        Ok(KuraPublicationLease {\n            kura: self,\n            pending_canonical_bytes,\n            fences,\n        })\n    }',)),
    (WITNESS_LEASE, "method", "KuraPublicationLease::pending_canonical_bytes", (
        "pub(super) fn pending_canonical_bytes(&self) -> u64", "self.pending_canonical_bytes",
    )),
    (WITNESS_LEASE, "method", "Kura::try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards", ('fn try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards<\'kura>(\n        &\'kura self,\n        fences: &mut AcquiredKuraPublicationFences<\'kura>,\n    ) -> Result<u64, KuraPublicationPreparationError> {\n        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {\n            return Ok(0);\n        }\n        let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes()?;\n        self.pending_block_bytes_with_merge_resolver(persisted_count, unindexed_bytes, |hash| {\n            fences.sidecar = Some(self.sidecar_lock.try_lock_or_wait().map_err(|wait| {\n                KuraPublicationPreparationError::Busy {\n                    field: "sidecar_lock",\n                    wait,\n                }\n            })?);\n            let pending = self.pending_merge_entry_by_hash_under_sidecar_guard(hash)?;\n            fences.release_cold_sidecar()?;\n            self.merge_entry_by_hash_after_sidecar(hash, pending)\n                .map_err(KuraPublicationPreparationError::Storage)\n        })\n    }',)),
    (WITNESS_LEASE, "method", "KuraPublicationLease::original_kura", (
        "pub(super) fn original_kura(&self) -> &Kura", "self.kura",
    )),
    (RAW_GEOMETRY, "struct", "RawGeometryAttempt", (
        "kura: KuraInstanceIdentity", "request: OwnedRequest",
        "previous_entries: Option<BTreeMap<LaneId, LaneStorageEntry>>",
        "updated_entries: Option<BTreeMap<LaneId, LaneStorageEntry>>",
        "maintenance: RawGeometryMaintenance", "target: Option<PreparedGeometryJournalTransition>",
        "pending_phase: Option<LaneGeometryPhase>", "intent_complete: bool", "operation_cursor: usize",
        "provisioning_failure: Option<RawGeometryProvisioningFailure>", "claim: RawGeometryClaim",
    )),
    (RAW_GEOMETRY, "method", "RawGeometryClaim::authorizes", (
        "Arc::ptr_eq(&self.state, &gate.state)", "!state.abandoned",
        "state.active.ptr_eq(&Arc::downgrade(&self.signal))", "!self.complete",
    )),
    (RAW_GEOMETRY, "method", "RawGeometryClaim::drop", (
        "state.active.ptr_eq(&Arc::downgrade(&self.signal))",
        "state.abandoned |= self.effects_started && !self.complete;",
        "drop(state);", "self.signal.released.send_replace(true);",
    )),
    (RAW_GEOMETRY, "method", "KuraPublicationLease::begin_raw_geometry_attempt", (
        "let kura = self.original_kura();", "kura.durable_mutation_authorized()?;",
        "kura.require_raw_geometry_canonical_recovery_complete()?;",
        "let claim = kura.raw_geometry_claim.claim()?;",
        "kura.validate_certified_lane_drain_frontier_under_publication_lease(",
        "kura.read_lane_geometry_journal_structure()?",
        "retained_journal::RetainedGeometryJournal::capture(kura, journal.encode().len())?",
        "if observed != journal", "kura: kura.instance_identity()",
        "request: OwnedRequest::capture(request, replaced, certified_frontiers)",
        "previous_entries: Some(previous_entries)", "updated_entries: Some(updated_entries)",
        "phase: RawGeometryPhase::Captured", 'namespace_receipts: Vec::new(),\n            claim',
    )),
    (RAW_GEOMETRY, "method", "RawGeometryAttempt::authenticate", (
        "let kura = lease.original_kura();",
        "!self.kura.matches(kura) || !self.claim.authorizes(&kura.raw_geometry_claim)",
        "if let Some(failure) = &self.provisioning_failure", "return Err(failure.error());",
        "kura.ensure_prune_recovery_not_required()?;", "kura.durable_mutation_authorized()?;",
        "kura.require_raw_geometry_canonical_recovery_complete()?;",
    )),
    (RAW_GEOMETRY, "method", "RawGeometryAttempt::persist_target", (
        "self.pending_phase.is_some_and(|pending| pending != phase)",
        "self.pending_phase = Some(phase);", 'self.target\n            .as_mut()',
        ".persist(kura, phase)?;", "self.pending_phase = None;",
    )),
    (RAW_GEOMETRY, "method", "RawGeometryAttempt::prepare_target", (
        "PreparedGeometryJournalTransition::prepare_with_retained_writer(",
        'kura,\n            self.journal.clone(),\n            index,\n            &mut self.maintenance.writer',
    )),
    (RAW_GEOMETRY, "method", "RawGeometryAttempt::select_plan", (
        "record.transition_height == self.request.transition_height",
        "record.previous_catalog == previous_catalog",
        "record.previous_lineage_root == self.request.previous_lineage_root",
        "record.updated_catalog == updated_catalog",
        "record.updated_lineage_root == self.request.updated_lineage_root",
        "record.previous_bindings != self.previous_bindings",
        "record.updated_bindings != self.updated_bindings",
        "TargetKind::Published", "TargetKind::Retained", "TargetKind::Fresh",
    )),
    (RAW_GEOMETRY, "method", "RawGeometryAttempt::resume_under", (
        "let kura = self.authenticate(lease)?;", "RawGeometryPhase::FilesApplied => return Ok(())",
        "self.maintenance.flush(kura)?;", "self.claim.effects_started = true;",
        'kura.finish_pending_lane_geometry_gc_with_custody(\n                &mut self.journal,\n                Some(&mut mutation),\n            )?;',
        "self.plan = Some(self.select_plan(kura)?);",
        "kura.reconcile_lane_geometry_history_to_count_with_custody(",
        "if self.target.is_none()", 'if !matches!(kind, TargetKind::Published) {\n                    let retiring = kura.geometry_retirement_identities(',
        "let pending = lease.pending_canonical_bytes();",
        "kura.ensure_lane_retirement_admissible_locked(pending, &retiring, &certified)?;",
        "self.target = Some(self.prepare_target(kura, index)?);",
        "self.persist_target(kura, LaneGeometryPhase::Intent)?;", "self.intent_complete = true;",
        "GeometryEvidencePolicy::FreshJournalIntent", "GeometryEvidencePolicy::RequireDurableEvidence",
        "while self.operation_cursor < target.operations().len()",
        "kura.apply_geometry_operations_forward_with_progress(",
        "if matches!(kind, TargetKind::Fresh) && provisioning_started",
        "self.provisioning_failure = Some(failure);", "self.phase = RawGeometryPhase::RecoveryRequired;",
        "self.operation_cursor += 1;", "self.persist_target(kura, LaneGeometryPhase::FilesApplied)?;",
        '&self.request.updated,\n            &self.request.updated_incarnations,\n            &self.request.updated_activation_heights,',
        "self.updated_entries.take()", "*kura.lane_storage_entries.lock() = entries;",
        "self.phase = RawGeometryPhase::FilesApplied;",
    )),
    (RAW_GEOMETRY, "method", "RawGeometryAttempt::publish_catalog_under", (
        "let kura = self.authenticate(lease)?;",
        "RawGeometryPhase::FilesApplied | RawGeometryPhase::PublishingCatalog",
        'self\n            .catalog_baseline\n            .is_some_and(|original| original != configured_baseline)',
        "self.journal.configured_catalog_hash != Some(expected)",
        "self.journal.configured_primary_binding.as_ref() != self.updated_bindings.first()",
        "kura.require_lane_marker(primary)?;", "self.catalog_baseline = Some(configured_baseline);",
        "self.phase = RawGeometryPhase::PublishingCatalog;",
        "self.persist_target(kura, LaneGeometryPhase::CatalogPublished)?;",
        "self.phase = RawGeometryPhase::CatalogPublished;", "self.claim.finish();",
    )),
    (RAW_GEOMETRY, "method", "RawGeometryAttempt::reauthenticate_catalog_under", (
        "let kura = lease.original_kura();", "!self.kura.matches(kura)",
        "self.phase != RawGeometryPhase::CatalogPublished", "!self.claim.complete",
        "self.has_pending_journal_write()", "kura.raw_geometry_claim.ensure_unclaimed()?;",
        "target.reauthenticate_completed(kura, LaneGeometryPhase::CatalogPublished)?;",
        "writer.reauthenticate_current(kura)?;", "installed.len() != self.updated_bindings.len()",
        'installed.get(&binding.lane_id).map(|entry| entry.identity)\n                    != Some(binding.identity())',
    )),
    (RAW_GEOMETRY, "method", "RawGeometryAttempt::rollback_under", (
        "let kura = self.authenticate(lease)?;", "self.maintenance.pending.is_some()",
        'self.phase != RawGeometryPhase::RollingBack\n                    || phase != LaneGeometryPhase::RolledBack',
        'RawGeometryPhase::PublishingCatalog\n                | RawGeometryPhase::CatalogPublished\n                | RawGeometryPhase::RolledBack',
        "GeometryEvidencePolicy::AllowJournalIntentProvisioning", "GeometryEvidencePolicy::RequireDurableEvidence",
        "let index = target.operations().len() - self.operation_cursor - 1;",
        'kura.apply_geometry_operations_rollback(\n                    &target.operations()[index..index + 1],\n                    policy,\n                )?;',
        "self.persist_target(kura, LaneGeometryPhase::RolledBack)?;",
        '&self.request.previous,\n                &self.request.previous_incarnations,\n                &self.request.previous_activation_heights,',
        "self.previous_entries.take()", "*kura.lane_storage_entries.lock() = entries;", "self.claim.finish();",
    )),
)
PREPARATION_OWNER_BINDINGS += QUEUE_GEOMETRY_OWNER_BINDINGS

# Phase custody is a private integration boundary, not production activation or
# aggregate execution admission. The original payload and admissions stay in
# the existing descriptor while its block type advances irreversibly.
RETAINED_CARRIER_BINDINGS = (
    (DECISION_CARRIER, "enum", "RetainedCarrier", (
        "Capturing(Box<super::StagedCarrierCapture<Admission>>)",
        "Validated(PreparedCarrierJournals<Admission>)",
        "Decided(DecisionBoundCarrierJournals<Admission, BindingAdmission>)",
        "super::DetachedCarrierComponents", "crate::kura::KuraWsvCheckpointReceipt",
    )),
    (DECISION_CARRIER, "method", "RetainedCarrier::matches_validation_candidate", (
        "Self::Capturing(carrier) => carrier.matches_candidate(context, proposal)",
        "Self::Validated(journals) => journals.matches_validation_candidate(context, proposal)",
        "Self::Decided(carrier) => carrier\n                .journals\n                .matches_validation_candidate(context, proposal)",
        "Self::Checkpointed(carrier) => carrier\n                .journals\n                .matches_validation_candidate(context, proposal)",
    )),
    (DECISION_CARRIER, "method", "RetainedCarrier::ready_commitment", (
        "Self::Capturing(_) => None",
        "Self::Validated(journals) => Some(journals.execution_prefix_commitment())",
        "Self::Decided(carrier) => Some(carrier.journals.execution_prefix_commitment())",
        "Self::Checkpointed(carrier) => Some(carrier.journals.execution_prefix_commitment())",
    )),
    (DECISION_CARRIER, "method", "RetainedCarrier::resume_capture", (
        "Self::Capturing(carrier) => carrier\n                .try_complete()\n                .map(Self::Validated)",
        ".map_err(|(carrier, error)| (Self::Capturing(carrier), error))", "ready => Ok(ready)",
    )),
    (JOURNALS, "method", "StagedCarrierCapture::matches_candidate", (
        "self.journals\n            .matches_validation_candidate(context, proposal)",
    )),
    (JOURNALS, "fn", "matches_validation_candidate", (
        "if self.context.as_ref() != context", "return false;",
        "self.valid.as_ref().canonical_proposal_wire_hash()",
        "proposal.canonical_proposal_wire_hash()",
        "(Ok(original), Ok(candidate)) => original == candidate", "_ => false",
    )),
    (JOURNALS, "fn", "execution_prefix_commitment", ("self.execution_prefix",)),
    (VALIDATION_CUSTODY, "struct", "Candidate", (
        "subject: wire::BlockSubject", "owner: Option<O>",
    )),
    (VALIDATION_CUSTODY, "enum", "CarrierMarkerPreparation", (
        "Ready(wire::ExecutionCommitment)", "Deferred(LocalValidationRefusal)", "ValidationError(E)",
    )),
    (VALIDATION_CUSTODY, "struct", "RetainedBodyValidationService", (
        "validator: P", "identity: V2BodyStoreInstanceIdentity",
        "candidates: Vec<Candidate<P::Owner>>", "markers: Vec<Marker>", "limit: usize",
        "_descriptor_admission: [AllocationCharge; 2]",
    )),
    (VALIDATION_CUSTODY, "method", "RetainedBodyValidationService::descriptor_layouts", (
        'Layout::array::<Candidate<P::Owner>>(limit)\n                .map_err(|_| AllocationRefusal::DemandOverflow)?',
        "Layout::array::<Marker>(limit).map_err(|_| AllocationRefusal::DemandOverflow)?",
    )),
    (VALIDATION_CUSTODY, "method", "RetainedBodyValidationService::descriptor_bytes", (
        "Self::descriptor_layouts(limit)?", ".try_fold(0, |total: usize, layout|",
        'total\n                    .checked_add(layout.size())\n                    .ok_or(AllocationRefusal::DemandOverflow)',
    )),
    (VALIDATION_CUSTODY, "method", "RetainedBodyValidationService::new", (
        "budget: &AllocationBudget", "let layouts = Self::descriptor_layouts(limit)?",
        "let mut reservation = budget.try_reserve_layouts(layouts)?",
        'let descriptor_admission = [\n            reservation.try_split(layouts[0])?,\n            reservation.try_split(layouts[1])?,\n        ]',
        "drop(reservation)", "candidates.try_reserve_exact(limit)?", "markers.try_reserve_exact(limit)?",
        'Ok(Self {\n            validator,\n            identity,\n            candidates,\n            markers,\n            limit,\n            _descriptor_admission: descriptor_admission,\n        })',
    )),
    (VALIDATION_CUSTODY, "method", "RetainedBodyValidationService::preflight_marker", (
        "&self", "let candidate = self", ".find(|row| row.subject == durable.subject())",
        "if candidate.is_some_and(|row| row.owner.is_none())",
        "return Err(CarrierCustodyError::MissingOwner)",
        "!self.markers.iter().any(|row| row.durable == *durable)",
        "self.markers.len() == self.limit", "candidate.is_none() && self.candidates.len() == self.limit",
        "return Err(CarrierCustodyError::Capacity)", "Ok(())",
    )),
    (VALIDATION_CUSTODY, "method", "RetainedBodyValidationService::prepare_marker", (
        "if marker.is_none() && self.markers.len() == self.limit",
        "if requires_existing_owner", "return Err(CarrierCustodyError::MissingOwner)",
        "if self.candidates.len() == self.limit", "return Err(CarrierCustodyError::Capacity)",
        "let index = self.candidates.len()", "self.candidates.push(Candidate {",
        "subject: durable.subject()", "owner: None",
        "let owner = match self.validator.prepare(context, body)",
        'let vacant = self\n                            .candidates\n                            .pop()\n                            .expect("reserved candidate descriptor")',
        "debug_assert_eq!(vacant.subject, durable.subject())", "debug_assert!(vacant.owner.is_none())",
        "return Ok(CarrierMarkerPreparation::ValidationError(error))",
        "self.candidates[index].owner = Some(owner)", "if !owner.matches_candidate(context, body)",
        "return Err(CarrierCustodyError::Identity)", "let commitment = match owner.ready_commitment()",
        "Some(commitment) => commitment", "self.resume_candidate(index, context, body)?",
        "return Ok(CarrierMarkerPreparation::Deferred(refusal))",
        ".ready_commitment()\n                    .ok_or(CarrierCustodyError::IncompleteCapture)?",
        "confirmed: None", "Ok(CarrierMarkerPreparation::Ready(commitment))",
    )),
    (VALIDATION_CUSTODY, "method", "RetainedBodyValidationService::resume_candidate", (
        "match self.validator.resume(owner)",
        "Ok(owner) => {\n                self.candidates[index].owner = Some(owner);\n                None\n            }",
        "Err((owner, refusal)) => {\n                self.candidates[index].owner = Some(owner);\n                Some(refusal)\n            }",
        "if !owner.matches_candidate(context, body)", "return Err(CarrierCustodyError::Identity)", "Ok(refusal)",
    )),
    (VALIDATION_CUSTODY, "method", "RetainedBodyValidationService::confirm", (
        "if owner.ready_commitment() != Some(receipt.execution_commitment())",
        "return Err(CarrierCustodyError::Identity)", "marker.confirmed = Some(receipt.clone())",
    )),
    (VALIDATION_CUSTODY, "struct", "SelectedValidationCarrier", (
        "service: &'a mut RetainedBodyValidationService<P>", "index: usize", "owner: Option<P::Owner>",
    )),
    (VALIDATION_CUSTODY, "method", "SelectedValidationCarrier::try_consume", (
        "mut self", "publish: impl FnOnce(&P, P::Owner) -> Result<R, (P::Owner, E)>",
        "match publish(&self.service.validator, owner)", "Err((owner, error)) => {\n                self.owner = Some(owner);\n                Err(error)\n            }",
        "let subject = self.service.candidates[self.index].subject",
        ".retain(|row| row.durable.subject() != subject)",
    )),
    (VALIDATION_CUSTODY, "method", "SelectedValidationCarrier::drop", (
        "if let Some(owner) = self.owner.take()",
        "debug_assert!(self.service.candidates[self.index].owner.is_none())",
        "self.service.candidates[self.index].owner = Some(owner)",
    )),
    (RETAINED_VALIDATION, "method", "V2BodyStore::retained_validation_descriptor_bytes", (
        "RetainedBodyValidationService::<P>::descriptor_bytes(self.capacity.max_body_entries)",
        ".map_err(super::super::v2_apply::validation_custody::CarrierCustodyError::from)",
        ".map_err(V2BodyStoreError::from)",
    )),
    (RETAINED_VALIDATION, "method", "V2BodyStore::retained_validation_service", (
        "budget: &mv::allocation::AllocationBudget",
        'RetainedBodyValidationService::new(\n            validator,\n            self.instance_identity(),\n            self.capacity.max_body_entries,\n            budget,\n        )',
    )),
    (RETAINED_VALIDATION, "method", "V2BodyStore::execute_retained_durable_validation", (
        "if !service.matches_store(&self.instance_identity())",
        "if !self.rejected.contains_key(&key)", "service.preflight_marker(&durable)?",
        "self.load_validation_envelope(&durable, expected_manifest_hash)?", "service.prepare_marker(",
        "already_validated.is_some() || reused.is_some()",
        "let validated = self.persist_validated_receipt(&durable, commitment)?",
        "service.confirm(&validated)?",
        "CarrierMarkerPreparation::Deferred(refusal) => {\n                Err(V2BodyStoreError::LocalValidation(refusal))\n            }",
        "CarrierMarkerPreparation::ValidationError(error) =>",
    )),
)
PREPARATION_OWNER_BINDINGS += RETAINED_CARRIER_BINDINGS


# The original service State/Queue pair and exact route cut now discharge the
# formerly unconditional retirement refusal; emptiness alone grants no authority.
CARRIER_QUEUE_BINDINGS = (
    ('crates/iroha_core/src/state/world_publication.rs', 'method', 'DetachedWorld::try_prepare_publication', ('Err((field, error)) => {', 'prepared.push(field);', 'prepared.release_all();', 'fields.extend(prepared.iter_mut().rev().map(|field| field.abort()));', 'let retirement = AbortedWorld {', '_fields: prepared,', '_installation: Some(installation)', 'WorldPublicationError::Field(error),', 'retirement')),
    ('crates/iroha_core/src/state/world_publication.rs', 'struct', 'AbortedWorld', ("_fields: PreparedWorldFields<'target>", '_installation: Option<Installation>')),
    (APPLY, 'method', 'V2ApplyService::carrier_queue_source', (
        'carrier_queue_retirement::OriginalCarrierQueue::new(&self.state, &self.queue)',
    )),
    (SERVICE_QUEUE, 'struct', 'OriginalCarrierQueue', (
        "state: &'service State",
        "queue: &'service Queue",
    )),
    (SERVICE_QUEUE, 'method', 'OriginalCarrierQueue::new', (
        "pub(super) fn new(state: &'service State, queue: &'service Queue)",
        'Self { state, queue }',
        "pub(super) fn new(state: &'service State, queue: &'service Queue) -> Self",
    )),
    (SERVICE_QUEUE, 'method', 'OriginalCarrierQueue::belongs_to', (
        'core::ptr::eq(self.state, state)',
    )),
    (SERVICE_QUEUE, 'method', 'OriginalCarrierQueue::try_observe', (
        'self.queue.try_lock_lane_retirement_observer()',
        "Result<QueueLaneRetirementObserver<'service>, concread::release::ReleaseWait>",
    )),
    (SERVICE_QUEUE, 'method', 'OriginalCarrierQueue::owns_cut', (
        'cut.belongs_to(self.queue)',
    )),
    (QUEUE_OWNER, 'method', 'QueueLaneRetirementCut::belongs_to', (
        'core::ptr::eq(self.observer.queue, queue)',
    )),
    (QUEUE_OWNER, 'method', 'QueueLaneRetirementCut::durability_faulted', (
        'self.observer.durability_faulted()',
    )),
    (QUEUE_OWNER, 'method', 'QueueLaneRetirementCut::lane_pending_work_release', (
        'self.observer.queue.lane_pending_work_release_locked(',
        '&self.reservations,',
        'lane_id,',
        'dataspace_id,',
        'lane_incarnation',
        'self.observer.queue.lane_pending_work_release_locked(\n            &self.reservations,\n            lane_id,\n            dataspace_id,\n            lane_incarnation,\n        )',
    )),
    (CARRIER_QUEUE, 'struct', 'CarrierQueueRetirement', (
        'state_owner: NativeLaneStateOwner',
        'header: BlockHeader',
        'routes: Vec<(LaneId, DataSpaceId, Hash)>',
        "_cut: QueueLaneRetirementCut<'queue>",
    )),
    (CARRIER_QUEUE, 'method', 'CarrierQueueRetirement::try_new', ("fn try_new(\n        target: &State,\n        geometry: &PreparedCarrierGeometry,\n        header: BlockHeader,\n        source: &crate::sumeragi::v2_apply::carrier_queue_retirement::OriginalCarrierQueue<'queue>,\n        cut: QueueLaneRetirementCut<'queue>,\n    ) -> Result<Self, (CarrierQueueRetirementError, QueueRetirementCleanup)> {\n        let checked = (|| {\n            if !source.belongs_to(target) || !geometry.matches_publication_target(target, header) {\n                return Err(CarrierQueueRetirementError::ForeignState);\n            }\n            if !source.owns_cut(&cut) {\n                return Err(CarrierQueueRetirementError::ForeignQueue);\n            }\n            let mut routes = Vec::new();\n            geometry\n                .for_each_retirement_route(|lane, dataspace, incarnation| {\n                    routes.push((lane, dataspace, incarnation));\n                    Ok(())\n                })\n                .map_err(CarrierQueueRetirementError::Geometry)?;\n            for &(lane, dataspace, incarnation) in &routes {\n                match cut.lane_pending_work_release(lane, dataspace, incarnation) {\n                    Ok(None) => {}\n                    Ok(Some(wait)) => {\n                        return Err(CarrierQueueRetirementError::Pending {\n                            lane,\n                            dataspace,\n                            incarnation,\n                            wait,\n                        });\n                    }\n                    Err(error) => return Err(CarrierQueueRetirementError::Unavailable(error)),\n                }\n            }\n            let state_owner = target\n                .native_lane_state_owner()\n                .ok_or(CarrierQueueRetirementError::ForeignState)?;\n            Ok((state_owner, routes))\n        })();\n        match checked {\n            Ok((state_owner, routes)) => Ok(Self {\n                state_owner,\n                header,\n                routes,\n                _cut: cut,\n            }),\n            Err(error) => Err((\n                error,\n                QueueRetirementCleanup {\n                    released: cut.release_deferred().map(Some),\n                },\n            )),\n        }\n    }",)),
    (CARRIER_QUEUE, 'method', 'CarrierQueueRetirement::ensure_available', (
        'if self._cut.durability_faulted()',
        'QueueLaneRetirementUnavailable::DurabilityFault',
        'Err(CarrierQueueRetirementError::Unavailable(\n                QueueLaneRetirementUnavailable::DurabilityFault,\n            ))',
        'else {\n            Ok(())\n        }',
    )),
    (CARRIER_QUEUE, 'method', 'CarrierQueueRetirement::authenticates', (
        'self.ensure_available().is_err()',
        '!self.state_owner.matches_state(target)',
        'self.header != header',
        '!geometry.matches_publication_target(target, header)',
        'return false;',
        'geometry.for_each_retirement_route(|lane, dataspace, incarnation|',
        'self.routes.get(index) != Some(&(lane, dataspace, incarnation))',
        'index += 1;',
        'result.is_ok() && index == self.routes.len()',
        'if self.ensure_available().is_err()\n            || !self.state_owner.matches_state(target)\n            || self.header != header\n            || !geometry.matches_publication_target(target, header)',
        'return false',
        'let mut index = 0',
        'let result = geometry.for_each_retirement_route(|lane, dataspace, incarnation|',
        'if self.routes.get(index) != Some(&(lane, dataspace, incarnation))',
        'return Err(LaneLifecycleError::Storage(',
        'index += 1',
        'Ok(())',
    )),
    (GEOMETRY_CARRIER, 'method', 'PreparedCarrierGeometry::for_each_retirement_route', (
        'pending.plan.retire.iter().chain(',
        '.replaced_lane_ids',
        '.filter(|lane| !pending.plan.retire.contains(lane))',
        '.previous_catalog',
        '.find(|entry| entry.id == *lane)',
        '.previous_lane_incarnations',
        '.filter(|incarnation| !lane_incarnation_is_zero(*incarnation))',
        'visit(*lane, previous.dataspace_id, incarnation)?',
        'let Some(pending) = &self._pending else {\n            return Ok(());\n        }',
        'let update = &pending.catalog_update',
        'pending.plan.retire.iter().chain(\n            update\n                .replaced_lane_ids\n                .iter()\n                .filter(|lane| !pending.plan.retire.contains(lane)),\n        )',
        'update\n                .previous_catalog\n                .lanes()\n                .iter()\n                .find(|entry| entry.id == *lane)\n                .ok_or_else(||',
        'update\n                .previous_lane_incarnations\n                .get(lane)\n                .copied()\n                .filter(|incarnation| !lane_incarnation_is_zero(*incarnation))\n                .ok_or_else(||',
    )),
    (GEOMETRY_CARRIER, 'method', 'PreparedCarrierGeometry::has_queue_custody', (
        '!self.requires_queue_custody()',
        'queue.is_some_and(|queue| queue.authenticates(target, self, header))',
        '!self.requires_queue_custody()\n            || queue.is_some_and(|queue| queue.authenticates(target, self, header))',
    )),
    (QUEUE_OWNER, 'method', 'QueueLaneRetirementObserver::durability_faulted', (
        'self.queue.transaction_selection_durability_faulted()',
    )),
    (QUEUE_OWNER, 'method', 'QueueLaneRetirementObserver::lane_pending_work_release', (
        'let _mutation = self.queue.push_remove_lock.lock()',
        'let reservations = self.queue.lane_reservations.lock()',
        'self.queue.lane_pending_work_release_locked(\n            &reservations,\n            lane_id,\n            dataspace_id,\n            lane_incarnation,\n        )',
    )),
    (QUEUE_OWNER, 'method', 'Queue::lane_pending_work_release_locked', (
        'if hash_is_zero(lane_incarnation)',
        'return Err(QueueLaneRetirementUnavailable::InvalidIncarnation)',
        'let scope = (lane_id, dataspace_id, lane_incarnation)',
        'self\n            .lane_retirement_releases\n            .lock()\n            .entry(scope)\n            .or_default()\n            .observe()',
        'if self.transaction_selection_durability_faulted()',
        'Err(QueueLaneRetirementUnavailable::DurabilityFault)',
        'Self::lane_retirement_reservation_snapshot(\n            reservations,\n            lane_id,\n            dataspace_id,\n            lane_incarnation,\n        )',
        '.is_none_or(|owned| self.lane_has_pending_route_work(&owned, lane_id, dataspace_id))',
        'Ok(Some(wait))',
        'Ok(None)',
        'if !matches!(&result, Ok(Some(_)))',
        'self.lane_retirement_releases.lock().remove(&scope)',
        'drop(source.guard(scope))',
        'result',
    )),
    (CARRIER_QUEUE, 'enum', 'CarrierQueueRetirementError', (
        'Missing',
        'ForeignState',
        'ForeignQueue',
        "Busy {\n        field: &'static str,\n        wait: concread::release::ReleaseWait,\n    }",
        'Pending {\n        lane: LaneId,\n        dataspace: DataSpaceId,\n        incarnation: Hash,\n        wait: concread::release::ReleaseWait,\n    }',
        'Unavailable(QueueLaneRetirementUnavailable)',
        'Geometry(LaneLifecycleError)',
    )),
    (PHYSICAL_CARRIER, 'struct', 'AcquiredCarrierComponents', (
        "original: Option<AcquiredCarrierParticipants<'target>>",
    )),
    (PHYSICAL_CARRIER, 'struct', 'AcquiredCarrierParticipants', (
        "world: PreparedWorld<'target, (), ()>", "runtime: PreparedRuntimeJournals<'target, (), ()>",
        "transactions: PreparedDetachedTransactionsBlock<'target, ()>",
        "block_hashes: PreparedBlockHashes<'target, ()>", "_fences: CarrierFences<'target>",
    )),
    (PHYSICAL_CARRIER, 'method', 'AcquiredCarrierComponents::into_original', (
        'self.original.take().expect("original carrier participants")',
    )),
    (PHYSICAL_CARRIER, 'method', 'AcquiredCarrierComponents::abort', (
        "self.into_original().abort()",
    )),
    (PHYSICAL_CARRIER, 'method', 'AcquiredCarrierComponents::drop', (
        "if let Some(original) = self.original.take()", "drop(original.abort())",
    )),
    (PHYSICAL_CARRIER, 'method', 'AcquiredCarrierParticipants::abort', (
        "let (world, world_retirement) = world.abort()",
        "let (runtime, runtime_retirement) = runtime.abort()",
        "let (transactions, transactions_retirement) = transactions.abort()",
        "let (block_hashes, block_hashes_retirement) = block_hashes.abort()",
        "drop(fences.release_for_completion())", "world_retirement", "runtime_retirement",
        "transactions_retirement", "block_hashes_retirement", "DetachedCarrierComponents {",
    )),
    (PHYSICAL_CARRIER, 'method', 'StateFences::try_acquire', ('fn try_acquire<E>(\n        target: &\'target State,\n    ) -> Result<\n        Self,\n        (\n            CarrierPhysicalPreparationError<E>,\n            [Option<concread::release::DeferredRelease>; 3],\n        ),\n    > {\n        let acquire = |field, lock: &\'target crate::publication_lock::PublicationMutex| {\n            lock.try_lock_or_wait()\n                .map_err(|wait| CarrierPhysicalPreparationError::Fence { field, wait })\n        };\n        let commit = acquire("state_commit_lock", &target.state_commit_lock)\n            .map_err(|error| (error, [None, None, None]))?;\n        let lifecycle = match acquire("lane_lifecycle_lock", &target.lane_lifecycle_lock) {\n            Ok(guard) => guard,\n            Err(error) => return Err((error, [None, None, Some(commit.release_deferred())])),\n        };\n        let write = match acquire("state_write_lock", &target.state_write_lock) {\n            Ok(guard) => guard,\n            Err(error) => {\n                let lifecycle = lifecycle.release_deferred();\n                let commit = commit.release_deferred();\n                return Err((error, [None, Some(lifecycle), Some(commit)]));\n            }\n        };\n        Ok(Self {\n            _write: write,\n            _lifecycle: lifecycle,\n            _commit: commit,\n        })\n    }',)),
)
PREPARATION_OWNER_BINDINGS += CARRIER_QUEUE_BINDINGS

# State identity is now custody of the actual shared history root, not a second
# synthetic allocation. The exact original predecessor remains publication authority.
SHARED_HISTORY_BINDINGS = (
    (STATE, "method", "NativeLaneStateOwner::matches_state", (
        "state\n            .block_hashes\n            .map()\n            .is_some_and(|map| self.0.matches(map))",
    )),
    (STATE, "method", "NativeLaneStateOwner::same_family", ("self.0.same_family(&other.0)",)),
    (STATE, "method", "State::native_lane_state_owner", (
        "-> Option<NativeLaneStateOwner>",
        "self.block_hashes\n            .map()\n            .map(|map| NativeLaneStateOwner(map.family()))",
    )),
    (STATE, "method", "BlockHashes::map", (
        "BlockHashStorage::Owned(map) => Some(map)",
        "BlockHashStorage::EmergencyFastMapped(_) | BlockHashStorage::EmergencyFastEmpty => None",
    )),
    (STATE, "method", "BlockHashesBlock::pending", (
        "BlockHashRange {\n            source: self,\n            start: self.visible_len,\n            end: self.len(),\n        }",
    )),
    (STATE, "method", "DetachedBlockHashes::observe_current", (
        "let Some(map) = target.map() else {\n            return Ok(false);\n        };",
        "self.work.try_matches_current(map)",
    )),
    (STATE, "method", "DetachedBlockHashes::matches_current", ("self.observe_current(target) == Ok(true)",)),
    (STATE, "method", "DetachedBlockHashes::matches_block_predecessor", (
        "self.mode == block.mode", "self\n                .work\n                .predecessor()\n                .same_predecessor(&block.work.predecessor())",
    )),
    (HASH_SURFACE, "struct", "BlockHashSurface", (
        "predecessor:\n        concread::bptree::BptreeMapRetainedPredecessor<usize, HashOf<BlockHeader>, BlockHashMode>",
        "mode: mv::BlockMode", "visible_len: usize", "pending: Vec<HashOf<BlockHeader>>",
    )),
    (HASH_SURFACE, "method", "BlockHashSurface::capture", (
        "!std::ptr::eq(block.inner, expected) || block.visible_len > block.len()",
        "predecessor: block.work.predecessor().retain()", "mode: block.mode",
        "visible_len: block.visible_len", "pending: block.pending().iter().copied().collect()",
    )),
    (HASH_PUBLICATION, "struct", "PreparedBlockHashes", ("struct PreparedBlockHashes<'target, Installation> {\n    owner: NativeLaneStateOwner,\n    prepared: concread::release::ReleaseGuard<\n        'target,\n        BptreeMapPreparedCommit<'target, usize, HashOf<BlockHeader>, BlockHashMode>,\n    >,\n    mode: mv::BlockMode,\n    visible_len: usize,\n    height: usize,\n    committed_height: &'target AtomicUsize,\n    installation: Installation,\n}",)),
    (HASH_PUBLICATION, "method", "DetachedBlockHashes::try_prepare_publication", ('fn try_prepare_publication<\'target, Installation, E>(\n        self,\n        target: &\'target BlockHashes,\n        admit: impl FnOnce(&Self, &BlockHashes) -> Result<Installation, E>,\n    ) -> Result<\n        PreparedBlockHashes<\'target, Installation>,\n        (\n            Self,\n            mv::PublicationPreparationError<E>,\n            RefusedBlockHashes<Installation>,\n        ),\n    > {\n        let mut cleanup = RefusedBlockHashes {\n            _release: None,\n            _installation: None,\n        };\n        if self.reserved_tip.is_some() {\n            return Err((self, mv::PublicationPreparationError::Changed, cleanup));\n        }\n        let Some(map) = target.map() else {\n            return Err((self, mv::PublicationPreparationError::Changed, cleanup));\n        };\n        let wait = map.observe_reader_release();\n        match self.observe_current(target) {\n            Ok(true) => {}\n            Ok(false) => return Err((self, mv::PublicationPreparationError::Changed, cleanup)),\n            Err(error) => return Err((self, refusal(error, wait), cleanup)),\n        }\n        let installation = match admit(&self, target) {\n            Ok(value) => value,\n            Err(error) => {\n                return Err((\n                    self,\n                    mv::PublicationPreparationError::Admission(error),\n                    cleanup,\n                ));\n            }\n        };\n        cleanup._installation = Some(installation);\n        let height = self.len();\n        let Self {\n            work,\n            mode,\n            visible_len,\n            reserved_tip,\n        } = self;\n        let wait = target.released.observe();\n        let acquired = match map.try_acquire_owned(work) {\n            Ok(acquired) => target.released.poisoning_guard(acquired),\n            Err((work, error)) => {\n                return Err((\n                    Self {\n                        work,\n                        mode,\n                        visible_len,\n                        reserved_tip,\n                    },\n                    refusal(error, wait),\n                    cleanup,\n                ));\n            }\n        };\n        let writer = match acquired.try_map_preserving_release(|acquired| acquired.validate()) {\n            Ok(writer) => writer,\n            Err((acquired, error)) => {\n                let (work, released) = acquired.release_deferred(|acquired| acquired.abort());\n                cleanup._release = Some(released);\n                return Err((\n                    Self {\n                        work,\n                        mode,\n                        visible_len,\n                        reserved_tip,\n                    },\n                    refusal(error, wait),\n                    cleanup,\n                ));\n            }\n        };\n        let wait = map.observe_reader_release();\n        let prepared = match writer.try_map_preserving_release(|writer| writer.try_prepare_commit())\n        {\n            Ok(prepared) => prepared,\n            Err((writer, error)) => {\n                let (work, released) = writer.release_deferred(|writer| writer.detach());\n                cleanup._release = Some(released);\n                return Err((\n                    Self {\n                        work,\n                        mode,\n                        visible_len,\n                        reserved_tip,\n                    },\n                    refusal(error, wait),\n                    cleanup,\n                ));\n            }\n        };\n        Ok(PreparedBlockHashes {\n            owner: NativeLaneStateOwner(map.family()),\n            prepared,\n            mode,\n            visible_len,\n            height,\n            committed_height: &target.committed_height,\n            installation: cleanup\n                ._installation\n                .take()\n                .expect("original hash installation"),\n        })\n    }',)),
    (HASH_PUBLICATION, "method", "PreparedBlockHashes::state_owner", ("self.owner.clone()",)),
    (HASH_PUBLICATION, "method", "PreparedBlockHashes::abort", ('fn abort(self) -> (DetachedBlockHashes, AbortedBlockHashes<Installation>) {\n        let Self {\n            owner,\n            prepared,\n            mode,\n            visible_len,\n            installation,\n            ..\n        } = self;\n        let ((work, reader), writer) = prepared.release_deferred(|prepared| {\n            let (writer, reader) = prepared.abort_retaining();\n            (writer.detach(), reader)\n        });\n        let retirement = AbortedBlockHashes {\n            _owner: owner,\n            _release: [reader, writer],\n            _installation: installation,\n        };\n        (\n            DetachedBlockHashes {\n                work,\n                mode,\n                visible_len,\n                reserved_tip: None,\n            },\n            retirement,\n        )\n    }',)),
    (HASH_PUBLICATION, "method", "PreparedBlockHashes::publish", ("fn publish(self) -> PublishedBlockHashes<'target, Installation> {\n        let Self {\n            prepared,\n            height,\n            committed_height,\n            installation,\n            ..\n        } = self;\n        let published = prepared.map_preserving_release(|prepared| prepared.publish());\n        committed_height.store(height, Ordering::Release);\n        let retirement = published.release_retaining(|published| published.release());\n        PublishedBlockHashes {\n            _retirement: retirement,\n            _installation: installation,\n        }\n    }",)),
    (HASH_PUBLICATION, "struct", "PublishedBlockHashes", ("struct PublishedBlockHashes<'a, Installation> {\n    _retirement: concread::release::ReleaseGuard<\n        'a,\n        concread::bptree::BptreeMapCommitRetirement<usize, HashOf<BlockHeader>, BlockHashMode>,\n    >,\n    _installation: Installation,\n}",)),
)
PREPARATION_OWNER_BINDINGS += SHARED_HISTORY_BINDINGS


# A concrete fixed-width hash journal prepays the original successor before World.
# This does not assert aggregate World/VM/native-runtime funding or activate the
# retained Validate-to-Apply path.
PREPAID_HISTORY_BINDINGS = (
    (HASH_ADMISSION, "method", "BlockHashPolicy::take_node_charge", (
        "-> Self::Charge", "self.0\n            .try_split(layout)\n            .expect(\"complete concrete hash edit plan\")",
    )),
    (HASH_ADMISSION, "method", "BlockHashes::admit_successor", (
        "existing: AllocationDemand", "additional: AllocationDemand",
        "existing\n            .bytes()\n            .checked_add(additional.bytes())", "mv::allocation::AllocationRefusal::DemandOverflow",
        "if required > self.budget.limit_bytes()", "AllocationRefusal::ExceedsLimit",
        "requested_bytes: required", "limit_bytes: self.budget.limit_bytes()", "self.admit(additional)",
    )),
    (HASH_ADMISSION, "method", "BlockHashes::admit", (
        "self.budget\n            .try_reserve_bytes(demand.bytes())\n            .map(BlockHashPolicy)",
    )),
    (HASH_ADMISSION, "method", "BlockHashes::admission_error", (
        "MapAdmissionError::Busy => BlockHashAdmissionError::Busy(wait)",
        "MapAdmissionError::Poisoned => BlockHashAdmissionError::Poisoned",
        "MapAdmissionError::Changed => BlockHashAdmissionError::Changed(wait)",
        "MapAdmissionError::Planning(error) => BlockHashAdmissionError::Planning(error)",
        "MapAdmissionError::Refused(error) => BlockHashAdmissionError::Capacity(error)",
    )),
    (HASH_ADMISSION, "method", "BlockHashes::try_new", (
        "budget.with_deferred_refund_notifications(|_|", "BlockHashMap::try_new_with_node_custody",
        "budget\n                    .try_reserve_bytes(demand.bytes())\n                    .map(BlockHashPolicy)", "budget: budget.clone()",
        "initial.into_iter().enumerate()", ".try_insert_admitted_with_footprint(index, hash, |existing, additional|",
        "owner.admit_successor(existing, additional)", "map\n                    .try_write_owned(work)",
        "writer.prepare_commit().publish().release()", "owner.committed_height.store(index + 1, Ordering::Release)",
    )),
    (HASH_ADMISSION, "method", "BlockHashes::try_next_block", ('fn try_next_block(\n        &self,\n        replacement: bool,\n    ) -> Result<BlockHashesBlock<\'_>, BlockHashAdmissionError> {\n        self.budget.with_deferred_refund_notifications(|_| {\n            let map = self.map().ok_or(BlockHashAdmissionError::ReadOnly)?;\n            let wait = map.observe_reader_release();\n            let view = self.try_view().map_err(|error| match error {\n                concread::bptree::OwnedWriteError::Busy => {\n                    BlockHashAdmissionError::Busy(wait.clone())\n                }\n                concread::bptree::OwnedWriteError::Poisoned => BlockHashAdmissionError::Poisoned,\n                concread::bptree::OwnedWriteError::Changed => {\n                    BlockHashAdmissionError::Changed(wait.clone())\n                }\n            })?;\n            let prefix = if replacement {\n                view.len().saturating_sub(1)\n            } else {\n                view.len()\n            };\n            let predecessor = match &view.inner {\n                BlockHashesViewInner::Owned(view) => view.predecessor().retain(),\n                BlockHashesViewInner::Mapped(_) => unreachable!("mutable map checked above"),\n            };\n            drop(view);\n            let wait = self.released.observe();\n            let acquired = map\n                .try_acquire_writer()\n                .ok_or_else(|| BlockHashAdmissionError::Busy(wait.clone()))?;\n            let writer = match self\n                .released\n                .poisoning_guard(acquired)\n                .try_map_preserving_release(|acquired| {\n                    acquired\n                        .try_insert_admitted_with_footprint(\n                            prefix,\n                            HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])),\n                            |existing, additional| self.admit_successor(existing, additional),\n                        )\n                        .map_err(|(acquired, input, error)| (acquired, (input, error)))\n                }) {\n                Ok(writer) => writer,\n                Err((acquired, (_, error))) => {\n                    drop(acquired);\n                    return Err(self.admission_error(error, wait));\n                }\n            };\n            let (work, _) = writer.release_with(|(writer, previous)| (writer.detach(), previous));\n            if !predecessor.matches(&work.predecessor()) {\n                return Err(BlockHashAdmissionError::Changed(wait));\n            }\n            Ok(BlockHashesBlock {\n                inner: self,\n                work,\n                visible_len: prefix,\n                reserved_tip: Some(prefix),\n                fixture_edits: false,\n                mode: if replacement {\n                    mv::BlockMode::Replace\n                } else {\n                    mv::BlockMode::Ordinary\n                },\n            })\n        })\n    }',)),
    (STATE, "method", "BlockHashesBlock::push", (
        "if let Some(index) = self.reserved_tip.take()", "self.work\n                .try_update_private(&index, hash)",
        "assert!(\n                self.fixture_edits", "self.inner.budget.with_deferred_refund_notifications(|_|",
    )),
    (STATE, "method", "BlockHashesBlock::detach", (
        "work: self.work", "mode: self.mode", "visible_len: self.visible_len", "reserved_tip: self.reserved_tip",
    )),
    (STATE, "macro", "work_hash_read", (
        "self.work.len() - usize::from(self.reserved_tip.is_some())",
        "(index < self.hash_count())\n                    .then(|| self.work.get(&index))\n                    .flatten()",
        "assert!(start <= end && end <= self.len())", "self.work.range(start..end)",
    )),
    (APPLY, "method", "V2ApplyService::classify_validation_failure", (
        "BlockValidationError::BlockHashAdmission(refusal)",
        "BlockHashAdmissionError::Busy(wait) | BlockHashAdmissionError::Changed(wait) => {\n                    Some(wait)\n                }",
        "AllocationRefusal::Capacity { release, .. }", "=> Some(release)", "_ => None",
        "V2ApplyError::LocalValidation(match wait", "Some(wait) =>",
        "LocalValidationRefusal::PhysicalBusy", "wait.clone()", "self.queue.sumeragi_waker()",
        "None =>", "LocalValidationRefusal::RecoveryRequired(\n                    refusal.to_string(),\n                )",
    )),
    (CONTROLS, "fn", "map_block_err_to_reason", (
        "BlockValidationError::LocalStorageRecoveryRequired { .. }\n            | BlockValidationError::BlockHashAdmission(_) => return None",
    )),
)
PREPARATION_OWNER_BINDINGS += PREPAID_HISTORY_BINDINGS
PREPAID_HISTORY_ACQUISITION_BINDINGS = (
    (HASH_ADMISSION, "method", "BlockHashAdmissionError::release_wait", (
        "Self::Busy(wait) | Self::Changed(wait) => Some(wait)",
        "Self::Capacity(mv::allocation::AllocationRefusal::Capacity { release, .. }) => {\n                Some(release)\n            }",
        "_ => None",
    )),
    (HASH_ADMISSION, "method", "StateBlockStartError::release_wait", (
        "Self::History(error) => error.release_wait()", "Self::Stage(_) => None",
    )),
    (STATE, "method", "BlockHashes::new_emergency_fast_empty", (
        "inner: BlockHashStorage::EmergencyFastEmpty", "budget: mv::allocation::AllocationBudget::new(0)",
        "committed_height: AtomicUsize::new(0)",
    )),
    (HASH_RESTORE, "fn", "emergency_fast_block_hashes", (
        "kura\n        .emergency_fast_snapshot_boundary(snapshot_height)",
        "durable_height != snapshot_height || durable_tip != snapshot_tip",
        "Some(mapping) => BlockHashes::new_emergency_fast_mapped(mapping, snapshot_height)",
        "None if snapshot_height == 0 => BlockHashes::new_emergency_fast_empty()", "None => {\n                return Err(json::Error::InvalidField",
    )),
    (RUNTIME_ACQUISITION, "method", "State::acquire_canonical_runtime_block", (
        "let generation = self.state_view_generation()", "if generation % 2 != 0",
        "let block_hashes = self.block_hashes.try_next_block(replacement)?",
        "self.world.block_and_revert()", "self.world.block()",
        "if !is_stable_state_view_generation(generation, self.state_view_generation())",
        "drop(canonical_runtime)", "drop(lane_consensus_contexts)", "drop(prev_commit_topology)",
        "drop(commit_topology)", "drop(transactions)", "drop(world)", "drop(block_hashes)",
        "return Ok(AcquiredRuntimeBlock {", "block_hashes,",
    )),
    (STATE, "method", "State::block_and_revert_with_pristine_stage", (
        "self.acquire_canonical_runtime_block(true)?", "self.rewind_da_indexes_to_height(target_height)",
        "let mut state_block = StateBlock {", "block_hashes,",
    )),
    (BODY_STORE, "method", "HistoryAdmissionWait::new", (
        "let mut pending = Self(wait.wait_for_release())", "if pending.is_ready(wake) {\n            wake.wake_by_ref();\n        }", "pending",
    )),
    (BODY_STORE, "method", "HistoryAdmissionWait::is_ready", (
        "std::future::Future::poll(", "std::pin::Pin::new(&mut self.0)", "&mut std::task::Context::from_waker(wake)", ".is_ready()",
    )),
    (RUNNER_HISTORY, "method", "LocalProposalState::history_admission_pending", (
        "if let Some((pending_owner, pending)) = &mut self.history_wait", "*pending_owner == owner",
        "!pending.is_ready(wake)", "return true", "self.history_wait = None", "false",
    )),
    (RUNNER_HISTORY, "method", "LocalProposalState::defer_history_admission", (
        "let Some(wait) = error.release_wait() else {\n            return false;\n        }",
        "self.history_wait = Some((\n            owner,\n            super::v2_body_store::HistoryAdmissionWait::new(wait.clone(), wake),\n        ))", "true",
    )),
)
PREPAID_HISTORY_ACQUISITION_BINDINGS += (
    (RUNNER_HISTORY, "fn", "schedule_local_proposal", (
        "let owner = proposal_state.reconcile(LocalProposalOwner::from(directive))",
        "if proposal_state.history_admission_pending(owner, &queue.sumeragi_waker()) {\n        return Ok(());\n    }",
        "Err(error)\n                if proposal_state.defer_history_admission(\n                    LocalProposalOwner::from(current),\n                    &error,\n                    &queue.sumeragi_waker(),\n                ) =>\n            {\n                services\n                    .rearm_loaded_candidate_delivery(current.tag(), loaded_round, loaded_subject)\n                    .map_err(V2RunnerError::Service)?;\n                return Ok(());\n            }",
        "Err(error) => {\n                return Err(super::v2_candidate::CandidateError::LocalStateAdmission(error).into());\n            }",
        "Err(super::v2_candidate::CandidateError::LocalStateAdmission(error))\n                if proposal_state.defer_history_admission(\n                    owner,\n                    &error,\n                    &queue.sumeragi_waker(),\n                ) =>\n            {\n                return Ok(());\n            }",
        "Err(V2RunnerError::CandidateBuild(\n                super::v2_candidate::CandidateError::LocalStateAdmission(error),\n            )) if proposal_state.defer_history_admission(\n                owner,\n                &error,\n                &queue.sumeragi_waker(),\n            ) =>\n            {\n                return Ok(());\n            }",
    )),
    (LANE_WORK_HISTORY, "fn", "build_and_memoize_merge_execution_candidate", (
        "crate::state::StateBlockStartError<iroha_data_model::executor::IvmAdmissionError>",
        ".build_merge_execution_candidate(application_block_header, self.context.mode)?",
        "self.state.state_view_generation() == state_view_generation", "Ok(Some(candidate))",
    )),
    (LANE_WORK_HISTORY, "fn", "refresh_merge_candidates", (
        "if let Some((view, pending)) = &mut self.merge_history_wait",
        "if *view == active_view && !pending.is_ready(&wake) {\n                return Ok(self.defer_merge_candidate_work());\n            }",
        "self.merge_history_wait = None",
        "let Some(wait) = error.release_wait() else {\n                                return Err(V2LaneWorkError::StateAdmission(error));\n                            }",
        "self\n                .lane_drain_queue\n                .as_ref()", ".sumeragi_waker()",
        "self.merge_history_wait = Some((\n                                active_view,\n                                super::v2_body_store::HistoryAdmissionWait::new(\n                                    wait.clone(),\n                                    &wake,\n                                ),\n                            ))",
        "return Ok(self.defer_merge_candidate_work())",
    )),
)
PREPAID_HISTORY_ACQUISITION_BINDINGS += (
    (RUNNER_HISTORY, "fn", "candidate_attachments", (
        ".derive_npos_consensus_effects(round_header)",
        "if let Some(refusal) = error.downcast_ref::<crate::state::BlockHashAdmissionError>()",
        "V2RunnerError::CandidateBuild(\n                    super::v2_candidate::CandidateError::LocalStateAdmission(\n                        crate::state::StateBlockStartError::History(refusal.clone()),\n                    ),\n                )",
        "V2RunnerError::Candidate(error.to_string())",
    )),
    (LANE_WORK_HISTORY, "fn", "classify_merge_state_validation", (
        "Ok(()) => Ok(MergeCandidateValidation::Ready)",
        "Err(crate::state::MergeLedgerCommitError::BlockHashAdmission(error)) =>",
        "self.validated_merge_execution_candidate = None",
        "let Some(wait) = error.release_wait() else {\n                    return Err(MergeCandidateValidationError::Frontier(error.to_string()));\n                }",
        "self\n                    .lane_drain_queue\n                    .as_ref()", ".sumeragi_waker()",
        "self.merge_history_wait = Some((\n                    active_view,\n                    super::v2_body_store::HistoryAdmissionWait::new(wait.clone(), &wake),\n                ))",
        "Ok(MergeCandidateValidation::Deferred)",
        "Err(error) => Err(MergeCandidateValidationError::Invalid(error.to_string()))",
    )),
    (LANE_WORK_HISTORY, "fn", "validate_merge_candidate_for_active_round", (
        "return self.classify_merge_state_validation(active_view, validation)",
        "if self.classify_merge_state_validation(active_view, validation)?\n            == MergeCandidateValidation::Deferred\n        {\n            return Ok(MergeCandidateValidation::Deferred);\n        }",
        "self.validated_merge_execution_candidate = Some(validated)",
    )),
)
PREPARATION_OWNER_BINDINGS += PREPAID_HISTORY_ACQUISITION_BINDINGS

# Deferred notifications retain the original owners through the final commit unlock.
DEFERRED_COMPLETION_BINDINGS = (
    (PUBLICATION_MUTEX, "method", "PublicationGuard::release_deferred", (
        "self.inner.release_deferred(drop).1",
    )),
    ("crates/iroha_core/src/kura/publication_lease.rs", "method", "KuraPublicationLease::release_deferred", ('self.fences.release_deferred()',)),
    (QUEUE_OWNER, "method", "QueueLaneRetirementCut::release_deferred", (
        "reservations.release_deferred()", "_mutation.release_deferred()",
        "_reservation_transition_guard.release_deferred()",
    )),
    (CARRIER_QUEUE, "struct", "ReleasedCarrierQueue", (
        "_routes: Vec<(LaneId, DataSpaceId, Hash)>", "_state_owner: NativeLaneStateOwner",
        "_released: [concread::release::DeferredRelease; 3]",
    )),
    (CARRIER_QUEUE, "method", "CarrierQueueRetirement::release_deferred", (
        "ReleasedCarrierQueue {", "_released: _cut.release_deferred()",
        "_routes: routes", "_state_owner: state_owner",
    )),
    (PHYSICAL_CARRIER, "struct", "CompletionFences", ("_commit: PublicationGuard<'target>", '_state: [concread::release::DeferredRelease; 2]', '_queue: Option<ReleasedCarrierQueue>', '_kura: KuraPublicationCleanup')),
)
PREPARATION_OWNER_BINDINGS += DEFERRED_COMPLETION_BINDINGS

# A fully acquired aggregate owns joint release on ordinary Drop and unwind.
AGGREGATE_ABANDONMENT_BINDINGS = (
    ('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'struct', 'PreparedRuntimeJournals', ("original: Option<AcquiredRuntimeJournals<'target, Admission, Installation>>",)),
    ('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'PreparedRuntimeJournals::abort', ('self.original\n            .take()\n            .expect("original prepared runtime")\n            .abort()',)),
    ('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'PreparedRuntimeJournals::publish', ('self.original\n            .take()\n            .expect("original prepared runtime")\n            .publish()',)),
    ('crates/iroha_core/src/state/carrier_preparation/runtime_publication.rs', 'method', 'PreparedRuntimeJournals::drop', ('let Some(original) = self.original.take() else {\n            return;\n        };', 'let admission;\n        let installation;', 'let AcquiredRuntimeJournals {', 'admission = retained_admission;\n        installation = retained_installation;\n        let canonical_runtime = canonical_runtime.abort();\n        let commit_topology = commit_topology.abort();\n        let prev_commit_topology = prev_commit_topology.abort();\n        let lane_consensus_contexts = lane_consensus_contexts.abort();\n        drop((\n            canonical_runtime,\n            commit_topology,\n            prev_commit_topology,\n            lane_consensus_contexts,\n        ));\n        drop((admission, installation));')),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'struct', 'PreparedSet', ("original: Option<AcquiredSet<'target, Admission, Installation>>",)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'PreparedSet::abort', ('self.original\n            .take()\n            .expect("original prepared triggers")\n            .abort()',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'PreparedSet::publish', ('self.original\n            .take()\n            .expect("original prepared triggers")\n            .publish()',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_publication.rs', 'method', 'PreparedSet::drop', ('let Some(original) = self.original.take() else {\n            return;\n        };', 'let admission;\n        let installation;', 'let AcquiredSet {', 'admission = retained_admission;\n        installation = retained_installation;\n        let data_triggers = data_triggers.abort();\n        let pipeline_triggers = pipeline_triggers.abort();\n        let time_triggers = time_triggers.abort();\n        let by_call_triggers = by_call_triggers.abort();\n        let ids = ids.abort();\n        let active_data_trigger_ids = active_data_trigger_ids.abort();\n        let active_pipeline_trigger_ids = active_pipeline_trigger_ids.abort();\n        let active_time_trigger_ids = active_time_trigger_ids.abort();\n        let active_by_call_trigger_ids = active_by_call_trigger_ids.abort();\n        let contracts = contracts.abort();\n        drop((\n            data_triggers,\n            pipeline_triggers,\n            time_triggers,\n            by_call_triggers,\n            ids,\n            active_data_trigger_ids,\n            active_pipeline_trigger_ids,\n            active_time_trigger_ids,\n            active_by_call_trigger_ids,\n            contracts,\n        ));\n        drop((admission, installation));')),
)
PREPARATION_OWNER_BINDINGS += AGGREGATE_ABANDONMENT_BINDINGS

PARTIAL_FENCE_REFUSAL_BINDINGS = (
    ('crates/iroha_core/src/state/carrier_preparation/physical_publication.rs', 'method', 'SourceAuthenticatedCarrier::release', ('drop(kura.release_deferred())',)),
    ('crates/iroha_core/src/state/carrier_preparation/physical_publication.rs', 'method', 'StateFences::release_deferred', ('self._write.release_deferred()', 'self._lifecycle.release_deferred()', 'self._commit.release_deferred()')),
    ('crates/iroha_core/src/queue.rs', 'struct', 'QueueRetirementCleanup', ('released: [Option<concread::release::DeferredRelease>; 3]',)),
    ('crates/iroha_core/src/queue.rs', 'method', 'QueueLaneRetirementObserver::release_deferred', ('self._reservation_transition_guard.release_deferred()',)),
    ('crates/iroha_core/src/sumeragi/v2_apply.rs', 'method', 'V2ApplyService::try_validate_autoscale_retirement_queue_binding', ('Err((error, cleanup)) => {\n                drop(lifecycle_guard);\n                drop(cleanup);\n                return Err(busy(error.field, error.wait));\n            }', 'let queue_cleanup = queue_retirement_cut.release_deferred();\n        drop(lifecycle_guard);\n        drop(queue_cleanup);\n        result')),
)
PREPARATION_OWNER_BINDINGS += PARTIAL_FENCE_REFUSAL_BINDINGS

KURA_JOINT_RELEASE_BINDINGS = (
    ('crates/iroha_core/src/kura/publication_lease.rs', 'struct', 'AcquiredKuraPublicationFences', ("struct AcquiredKuraPublicationFences<'kura> {\n    sidecar: Option<PublicationGuard<'kura>>,\n    geometry: Option<PublicationGuard<'kura>>,\n    canonical: Option<PublicationGuard<'kura>>,\n    prune: Option<PublicationGuard<'kura>>,\n    cold_sidecar: Option<concread::release::DeferredReleaseBatch>,\n}",)),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'struct', 'KuraPublicationCleanup', ('struct KuraPublicationCleanup {\n    _fences: [Option<concread::release::DeferredRelease>; 4],\n    _cold_sidecar: Option<concread::release::DeferredReleaseBatch>,\n}',)),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'method', 'AcquiredKuraPublicationFences::new', ("fn new(kura: &'kura Kura) -> Self {\n        Self {\n            sidecar: None,\n            geometry: None,\n            canonical: None,\n            prune: None,\n            cold_sidecar: Some(kura.sidecar_lock.deferred_releases()),\n        }\n    }",)),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'method', 'AcquiredKuraPublicationFences::release_cold_sidecar', ('fn release_cold_sidecar(&mut self) -> Result<(), KuraPublicationPreparationError> {\n        let sidecar = self.sidecar.take().expect("original cold sidecar guard");\n        match sidecar.try_release_into(\n            self.cold_sidecar\n                .as_mut()\n                .expect("original sidecar release batch"),\n        ) {\n            Ok(()) => Ok(()),\n            Err(sidecar) => {\n                // Preserve even an invalid source substitution for joint cleanup.\n                // Nothing was unlocked or notified by the refused transfer.\n                self.sidecar = Some(sidecar);\n                Err(KuraPublicationPreparationError::Storage(\n                    Error::PruneIntentConflict(\n                        "publication sidecar release belongs to a foreign physical owner"\n                            .to_owned(),\n                    ),\n                ))\n            }\n        }\n    }',)),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'method', 'AcquiredKuraPublicationFences::take_cleanup', ('fn take_cleanup(&mut self) -> KuraPublicationCleanup {\n        KuraPublicationCleanup {\n            _fences: [\n                self.sidecar.take().map(PublicationGuard::release_deferred),\n                self.geometry.take().map(PublicationGuard::release_deferred),\n                self.canonical\n                    .take()\n                    .map(PublicationGuard::release_deferred),\n                self.prune.take().map(PublicationGuard::release_deferred),\n            ],\n            _cold_sidecar: self.cold_sidecar.take(),\n        }\n    }',)),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'method', 'AcquiredKuraPublicationFences::release_deferred', ('fn release_deferred(mut self) -> KuraPublicationCleanup {\n        self.take_cleanup()\n    }',)),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'method', 'AcquiredKuraPublicationFences::drop', ('fn drop(&mut self) {\n        // The fixed cleanup owner is built only after all four physical unlocks.\n        // Empty slots and an unused cold batch cannot signal an unacquired lock.\n        drop(self.take_cleanup());\n    }',)),
    ('crates/iroha_core/src/kura/publication_lease.rs', 'method', 'Kura::authenticate_archive_capture', ('fn authenticate_archive_capture(\n        &self,\n        network_id: NetworkId,\n        height: u64,\n        block_hash: [u8; 32],\n        finalized_at_unix_ms: u64,\n        receipt: &super::KuraV2CommitReceipt,\n    ) -> Result<(), KuraArchiveCaptureAuthenticationError> {\n        let mut fences = AcquiredKuraPublicationFences::new(self);\n        fences.prune = Some(self.prune_lock.lock());\n        fences.canonical = Some(self.canonical_chain_lock.lock());\n        fences.sidecar = Some(self.sidecar_lock.lock());\n        self.authenticate_archive_capture_under_publication_guards(\n            network_id,\n            height,\n            block_hash,\n            finalized_at_unix_ms,\n            receipt,\n        )\n    }',)),
    ('vendor/concread/src/release.rs', 'struct', 'DeferredReleaseBatch', ('struct DeferredReleaseBatch {\n    notification: ReleaseNotification,\n    released: bool,\n    poisoned: bool,\n}',)),
    ('vendor/concread/src/release.rs', 'method', 'ReleaseNotification::deferred_batch', ('fn deferred_batch(&self) -> DeferredReleaseBatch {\n        DeferredReleaseBatch {\n            notification: ReleaseNotification {\n                state: Arc::clone(&self.state),\n            },\n            released: false,\n            poisoned: false,\n        }\n    }',)),
    ('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_release_into', ('fn try_release_into<R>(\n        mut self,\n        batch: &mut DeferredReleaseBatch,\n        release: impl FnOnce(T) -> R,\n    ) -> Result<R, Self> {\n        if !Arc::ptr_eq(&self.notification.state, &batch.notification.state) {\n            return Err(self);\n        }\n        struct Record<\'a> {\n            batch: &\'a mut DeferredReleaseBatch,\n            poison_on_unwind: bool,\n        }\n        impl Drop for Record<\'_> {\n            fn drop(&mut self) {\n                self.batch.released = true;\n                self.batch.poisoned |= self.poison_on_unwind && std::thread::panicking();\n            }\n        }\n        // On callback unwind the original physical owner drops before this\n        // record. The batch remains in its caller\'s aggregate throughout.\n        let record = Record {\n            batch,\n            poison_on_unwind: self.poison_on_unwind,\n        };\n        let inner = self.inner.take().expect("owned release guard");\n        let _transferred = std::mem::ManuallyDrop::new(self);\n        let result = release(inner);\n        drop(record);\n        Ok(result)\n    }',)),
    ('vendor/concread/src/release.rs', 'method', 'DeferredReleaseBatch::drop', ('fn drop(&mut self) {\n        if self.released {\n            self.notification.released(self.poisoned);\n        }\n    }',)),
    ('crates/iroha_core/src/publication_lock.rs', 'method', 'PublicationMutex::deferred_releases', ('fn deferred_releases(&self) -> concread::release::DeferredReleaseBatch {\n        self.released.deferred_batch()\n    }',)),
    ('crates/iroha_core/src/publication_lock.rs', 'method', 'PublicationGuard::try_release_into', ('fn try_release_into(\n        self,\n        batch: &mut concread::release::DeferredReleaseBatch,\n    ) -> Result<(), Self> {\n        self.inner\n            .try_release_into(batch, drop)\n            .map_err(|inner| Self { inner })\n    }',)),
    ('crates/iroha_core/src/kura.rs', 'method', 'Kura::merge_entry_by_hash_with_sidecar_guard', ("fn merge_entry_by_hash_with_sidecar_guard(\n        &self,\n        hash: HashOf<MergeLedgerEntry>,\n        sidecar: PublicationGuard<'_>,\n    ) -> Result<Option<MergeLedgerEntry>> {\n        let pending = self.pending_merge_entry_by_hash_under_sidecar_guard(hash)?;\n        drop(sidecar);\n        self.merge_entry_by_hash_after_sidecar(hash, pending)\n    }",)),
    ('crates/iroha_core/src/kura.rs', 'method', 'Kura::pending_merge_entry_by_hash_under_sidecar_guard', ('fn pending_merge_entry_by_hash_under_sidecar_guard(\n        &self,\n        hash: HashOf<MergeLedgerEntry>,\n    ) -> Result<Option<MergeLedgerEntry>> {\n        self.ensure_prune_recovery_not_required()?;\n        self.ensure_canonical_storage_not_poisoned()?;\n        self.read_pending_merge_entry_path(&self.pending_merge_entry_path(hash), Some(hash))\n    }',)),
    ('crates/iroha_core/src/kura.rs', 'method', 'Kura::merge_entry_by_hash_after_sidecar', ('fn merge_entry_by_hash_after_sidecar(\n        &self,\n        hash: HashOf<MergeLedgerEntry>,\n        pending: Option<MergeLedgerEntry>,\n    ) -> Result<Option<MergeLedgerEntry>> {\n        self.ensure_prune_recovery_not_required()?;\n        if pending.is_some() {\n            return Ok(pending);\n        }\n        let mut merge_log = self.merge_log.lock();\n        self.ensure_prune_recovery_not_required()?;\n        let entry = merge_log.entry_by_hash(hash)?;\n        self.ensure_prune_recovery_not_required()?;\n        Ok(entry)\n    }',)),
)
PREPARATION_OWNER_BINDINGS += KURA_JOINT_RELEASE_BINDINGS

ACQUIRED_WRITER_BINDINGS = (
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'struct', 'LinCowCellOwnedAcquisition', ("struct LinCowCellOwnedAcquisition<'a, T, R, U, Charge = Untracked> {\n    guard: MutexGuard<'a, WriteState<T, R, Charge>>,\n    owned: LinCowCellOwned<T, R, U, Charge>,\n    caller: &'a LinCowCell<T, R, U, Charge>,\n    poisoned: bool,\n}",)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellOwnedAcquisition::validate', ("fn validate(\n        self,\n    ) -> Result<LinCowCellWriteTxn<'a, T, R, U, Charge>, (Self, OwnedWriteError)> {\n        if self.poisoned {\n            return Err((self, OwnedWriteError::Poisoned));\n        }\n        if !Shared::ptr_eq(&self.guard.current, &self.owned.base) {\n            return Err((self, OwnedWriteError::Changed));\n        }\n        let Self {\n            guard,\n            owned,\n            caller,\n            ..\n        } = self;\n        let LinCowCellOwned {\n            work,\n            next,\n            base,\n            root,\n        } = owned;\n        // The same root remains borrowed through caller throughout this handoff.\n        drop(root);\n        Ok(LinCowCellWriteTxn {\n            caller,\n            guard,\n            work,\n            next,\n            base,\n        })\n    }",)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellOwnedAcquisition::abort', ('fn abort(self) -> LinCowCellOwned<T, R, U, Charge> {\n        let Self { guard, owned, .. } = self;\n        drop(guard);\n        owned\n    }',)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCell::try_acquire_owned', ("fn try_acquire_owned(\n        &self,\n        owned: LinCowCellOwned<T, R, U, Charge>,\n    ) -> Result<\n        LinCowCellOwnedAcquisition<'_, T, R, U, Charge>,\n        (LinCowCellOwned<T, R, U, Charge>, OwnedWriteError),\n    > {\n        if !Shared::ptr_eq(&self.write, &owned.root) {\n            return Err((owned, OwnedWriteError::Changed));\n        }\n        let (guard, poisoned) = match self.write.try_lock() {\n            Ok(guard) => (guard, false),\n            Err(TryLockError::WouldBlock) => return Err((owned, OwnedWriteError::Busy)),\n            Err(TryLockError::Poisoned(error)) => (error.into_inner(), true),\n        };\n        Ok(LinCowCellOwnedAcquisition {\n            guard,\n            owned,\n            caller: self,\n            poisoned,\n        })\n    }",)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCell::try_write_owned', ("fn try_write_owned(\n        &self,\n        owned: LinCowCellOwned<T, R, U, Charge>,\n    ) -> Result<\n        LinCowCellWriteTxn<'_, T, R, U, Charge>,\n        (LinCowCellOwned<T, R, U, Charge>, OwnedWriteError),\n    > {\n        self.try_acquire_owned(owned)?\n            .validate()\n            .map_err(|(acquired, error)| (acquired.abort(), error))\n    }",)),
    ('vendor/concread/src/bptree/mod.rs', 'struct', 'BptreeMapOwnedAcquisition', ("struct BptreeMapOwnedAcquisition<'a, K, V, M = Untracked>\nwhere\n    K: Ord + Clone + Debug + Sync + Send + 'static,\n    V: Clone + Sync + Send + 'static,\n    M: MapMode + NodeCloning<K, V>,\n{\n    inner: LinCowCellOwnedAcquisition<\n        'a,\n        SuperBlock<K, V, M>,\n        CursorRead<K, V, M>,\n        CursorWrite<K, V, M>,\n        M::Charge,\n    >,\n}",)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapOwnedAcquisition::validate', ("fn validate(self) -> Result<BptreeMapWriteTxn<'a, K, V, M>, (Self, OwnedWriteError)> {\n        self.inner\n            .validate()\n            .map(|inner| BptreeMapWriteTxn { inner })\n            .map_err(|(inner, error)| (Self { inner }, error))\n    }",)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapOwnedAcquisition::abort', ('fn abort(self) -> BptreeMapOwned<K, V, M> {\n        BptreeMapOwned {\n            inner: self.inner.abort(),\n        }\n    }',)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMap::try_acquire_owned', ("fn try_acquire_owned(\n        &self,\n        owned: BptreeMapOwned<K, V, M>,\n    ) -> Result<BptreeMapOwnedAcquisition<'_, K, V, M>, (BptreeMapOwned<K, V, M>, OwnedWriteError)>\n    {\n        owned.inner.as_ref().assert_operable();\n        self.inner\n            .try_acquire_owned(owned.inner)\n            .map(|inner| BptreeMapOwnedAcquisition { inner })\n            .map_err(|(inner, error)| (BptreeMapOwned { inner }, error))\n    }",)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMap::try_write_owned', ("fn try_write_owned(\n        &self,\n        owned: BptreeMapOwned<K, V, M>,\n    ) -> Result<BptreeMapWriteTxn<'_, K, V, M>, (BptreeMapOwned<K, V, M>, OwnedWriteError)> {\n        self.try_acquire_owned(owned)?\n            .validate()\n            .map_err(|(acquired, error)| (acquired.abort(), error))\n    }",)),
    ('crates/mv/src/storage/physical.rs', 'fn', 'acquire_owned_writer', ("fn acquire_owned_writer<'a, K: Key, V: Value, M: MapMode + NodeCloning<K, V>>(\n    map: &'a BptreeMap<K, V, M>,\n    released: &'a ReleaseNotification,\n    owned: BptreeMapOwned<K, V, M>,\n) -> Result<\n    ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V, M>>,\n    (\n        BptreeMapOwned<K, V, M>,\n        OwnedWriteError,\n        Option<DeferredRelease>,\n    ),\n> {\n    let acquired = map\n        .try_acquire_owned(owned)\n        .map_err(|(owned, error)| (owned, error, None))?;\n    released\n        .poisoning_guard(acquired)\n        .try_map_preserving_release(|acquired| acquired.validate())\n        .map_err(|(acquired, error)| {\n            let (owned, notification) = acquired.release_deferred(|acquired| acquired.abort());\n            (owned, error, Some(notification))\n        })\n}",)),
    ('crates/iroha_core/src/state/block_hashes_publication.rs', 'struct', 'RefusedBlockHashes', ('struct RefusedBlockHashes<Installation> {\n    _release: Option<concread::release::DeferredRelease>,\n    _installation: Option<Installation>,\n}',)),
    ('crates/iroha_core/src/state.rs', 'method', 'StateBlock::commit_inner', ('let mut hash_refusal_cleanup = None;', 'let Self {', '.map_err(|(_, _, cleanup)| {\n                    hash_refusal_cleanup = Some(cleanup);\n                    TransactionsBlockError::SnapshotObservationChanged\n                })?;', 'drop(hash_refusal_cleanup);')),
)
PREPARATION_OWNER_BINDINGS += ACQUIRED_WRITER_BINDINGS

SUCCESSOR_ACQUISITION_BINDINGS = (
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'struct', 'LinCowCellWriterAcquisition', ("struct LinCowCellWriterAcquisition<'a, T, R, U, Charge = Untracked> {\n    guard: MutexGuard<'a, WriteState<T, R, Charge>>,\n    caller: &'a LinCowCell<T, R, U, Charge>,\n    poisoned: bool,\n}",)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'enum', 'WriterAdmissionError', ('enum WriterAdmissionError<E> {\n    /// An earlier unwind poisoned the acquired writer; admission was not called.\n    Poisoned,\n    /// Planning or admission refused before successor construction.\n    Refused(E),\n}',)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellWriterAcquisition::try_write_charged', ("fn try_write_charged<E>(\n        self,\n        admit: impl FnOnce(&T, WriterLayouts) -> Result<WriterAdmission<Charge, T::WriterInput>, E>,\n    ) -> Result<LinCowCellWriteTxn<'a, T, R, U, Charge>, (Self, WriterAdmissionError<E>)> {\n        if self.poisoned {\n            return Err((self, WriterAdmissionError::Poisoned));\n        }\n        let admission = match admit(\n            &self.guard.data,\n            LinCowCell::<T, R, U, Charge>::writer_allocation_layouts(),\n        ) {\n            Ok(admission) => admission,\n            Err(error) => return Err((self, WriterAdmissionError::Refused(error))),\n        };\n        let Self { guard, caller, .. } = self;\n        Ok(caller.create_writer(guard, admission))\n    }",)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCell::try_write_charged', ("fn try_write_charged<E>(\n        &self,\n        admit: impl FnOnce(&T, WriterLayouts) -> Result<WriterAdmission<Charge, T::WriterInput>, E>,\n    ) -> Result<Option<LinCowCellWriteTxn<'_, T, R, U, Charge>>, E> {\n        let Some(acquired) = self.try_acquire_writer() else {\n            return Ok(None);\n        };\n        match acquired.try_write_charged(admit) {\n            Ok(writer) => Ok(Some(writer)),\n            Err((acquired, error)) => {\n                drop(acquired);\n                match error {\n                    WriterAdmissionError::Poisoned => Ok(None),\n                    WriterAdmissionError::Refused(error) => Err(error),\n                }\n            }\n        }\n    }",)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCell::try_acquire_writer', ("fn try_acquire_writer(&self) -> Option<LinCowCellWriterAcquisition<'_, T, R, U, Charge>> {\n        let (guard, poisoned) = match self.write.try_lock() {\n            Ok(guard) => (guard, false),\n            Err(TryLockError::WouldBlock) => return None,\n            Err(TryLockError::Poisoned(error)) => (error.into_inner(), true),\n        };\n        Some(LinCowCellWriterAcquisition {\n            guard,\n            caller: self,\n            poisoned,\n        })\n    }",)),
    ('vendor/concread/src/bptree/mod.rs', 'struct', 'BptreeMapWriterAcquisition', ("struct BptreeMapWriterAcquisition<'a, K, V, M = Untracked>\nwhere\n    K: Ord + Clone + Debug + Sync + Send + 'static,\n    V: Clone + Sync + Send + 'static,\n    M: MapMode + NodeCloning<K, V>,\n{\n    inner: LinCowCellWriterAcquisition<\n        'a,\n        SuperBlock<K, V, M>,\n        CursorRead<K, V, M>,\n        CursorWrite<K, V, M>,\n        M::Charge,\n    >,\n}",)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMap::try_acquire_writer', ("fn try_acquire_writer(&self) -> Option<BptreeMapWriterAcquisition<'_, K, V, M>> {\n        self.inner\n            .try_acquire_writer()\n            .map(|inner| BptreeMapWriterAcquisition { inner })\n    }",)),
    ('vendor/concread/src/bptree/admission.rs', 'method', 'BptreeMap::insert_with_source', ('fn insert_with_source<E>(\n        &self,\n        key: K,\n        value: V,\n        admit: impl FnOnce(\n            &SuperBlock<K, V, Prepaid<P>>,\n            AllocationDemand,\n        ) -> Result<P, MapAdmissionError<E>>,\n    ) -> Result<(BptreeMapOwned<K, V, Prepaid<P>>, Option<V>), ((K, V), MapAdmissionError<E>)> {\n        let Some(acquired) = self.try_acquire_writer() else {\n            return Err(((key, value), MapAdmissionError::Busy));\n        };\n        match acquired.insert_with_source(key, value, admit) {\n            Ok((writer, previous)) => Ok((writer.detach(), previous)),\n            Err((acquired, input, error)) => {\n                drop(acquired);\n                Err((input, error))\n            }\n        }\n    }',)),
    ('vendor/concread/src/bptree/admission.rs', 'method', 'BptreeMapWriterAcquisition::insert_with_source', ('fn insert_with_source<E>(\n        self,\n        key: K,\n        value: V,\n        admit: impl FnOnce(\n            &SuperBlock<K, V, Prepaid<P>>,\n            AllocationDemand,\n        ) -> Result<P, MapAdmissionError<E>>,\n    ) -> Result<\n        (BptreeMapWriteTxn<\'a, K, V, Prepaid<P>>, Option<V>),\n        (Self, (K, V), MapAdmissionError<E>),\n    > {\n        let mut input = Some((key, value));\n        let acquired = self.inner.try_write_charged(|source, shells| {\n            let plan =\n                plan_insert::<K, V, P>(source, &input.as_ref().expect("original input").0, shells)\n                    .map_err(MapAdmissionError::Planning)?;\n            let mut provider = Prepaid(Some(admit(source, plan.demand)?));\n            let first_charge = provider.take_node_charge(plan.first_layout);\n            let first = FixedTrackingBuffer::try_new(plan.first, first_charge)\n                .unwrap_or_else(|_| unreachable!("planned first buffer layout"));\n            let last_charge = provider.take_node_charge(plan.last_layout);\n            let last = FixedTrackingBuffer::try_new(plan.last, last_charge)\n                .unwrap_or_else(|_| unreachable!("planned retirement buffer layout"));\n            let charges = WriterCharges {\n                cursor: provider.take_node_charge(shells.cursor),\n                reader: provider.take_node_charge(shells.reader),\n            };\n            Ok(WriterAdmission {\n                charges,\n                input: (provider, first, last),\n            })\n        });\n        let mut writer = match acquired {\n            Ok(writer) => writer,\n            Err((inner, error)) => {\n                let error = match error {\n                    WriterAdmissionError::Poisoned => MapAdmissionError::Poisoned,\n                    WriterAdmissionError::Refused(error) => error,\n                };\n                return Err((\n                    Self { inner },\n                    input.take().expect("original refused input"),\n                    error,\n                ));\n            }\n        };\n        let (key, value) = input.take().expect("original admitted input");\n        writer.as_mut().begin_admitted_edit();\n        let previous = writer.as_mut().try_insert(key, value).unwrap_or_else(|_| {\n            unreachable!("complete fixed tracking bound planned under original writer")\n        });\n        // A further closed edit requires its own complete admission. Release the\n        // unused remainder now, before potentially long handoff waits;\n        // actual allocation charges remain attached to their original owners.\n        writer.as_mut().finish_admitted_funding();\n        Ok((BptreeMapWriteTxn { inner: writer }, previous))\n    }',)),
    ('vendor/concread/src/bptree/admission.rs', 'method', 'BptreeMap::try_insert_admitted_with_footprint', ('fn try_insert_admitted_with_footprint<E>(\n        &self,\n        key: K,\n        value: V,\n        admit: impl FnOnce(AllocationDemand, AllocationDemand) -> Result<P, E>,\n    ) -> Result<(BptreeMapOwned<K, V, Prepaid<P>>, Option<V>), ((K, V), MapAdmissionError<E>)> {\n        self.insert_with_source(key, value, |source, additional| {\n            let existing =\n                current_footprint::<K, V, P>(source).map_err(MapAdmissionError::Planning)?;\n            admit(existing, additional).map_err(MapAdmissionError::Refused)\n        })\n    }',)),
    ('vendor/concread/src/bptree/admission.rs', 'method', 'BptreeMapWriterAcquisition::try_insert_admitted_with_footprint', ("fn try_insert_admitted_with_footprint<E>(\n        self,\n        key: K,\n        value: V,\n        admit: impl FnOnce(AllocationDemand, AllocationDemand) -> Result<P, E>,\n    ) -> Result<\n        (BptreeMapWriteTxn<'a, K, V, Prepaid<P>>, Option<V>),\n        (Self, (K, V), MapAdmissionError<E>),\n    > {\n        self.insert_with_source(key, value, |source, additional| {\n            let existing =\n                current_footprint::<K, V, P>(source).map_err(MapAdmissionError::Planning)?;\n            admit(existing, additional).map_err(MapAdmissionError::Refused)\n        })\n    }",)),
    ('vendor/concread/src/bptree/admission.rs', 'fn', 'current_footprint', ("fn current_footprint<K, V, P>(\n    source: &SuperBlock<K, V, Prepaid<P>>,\n) -> Result<AllocationDemand, PlanningError>\nwhere\n    K: Copy + Ord + Debug + Send + Sync + 'static,\n    V: Copy + Send + Sync + 'static,\n    P: ClonePlanning<K, V>,\n{\n    let mut existing = AllocationDemand::new();\n    existing.add_layout(MapCell::<K, V, Prepaid<P>>::initial_allocation_layouts().root)?;\n    existing.add_layout(MapCell::<K, V, Prepaid<P>>::reader_allocation_layout())?;\n    let (leaves, branches) = source.node_counts();\n    let mut leaf = AllocationDemand::new();\n    leaf.add_layout(Layout::new::<CachePadded<Leaf<K, V, P::Charge>>>())?;\n    let mut branch = AllocationDemand::new();\n    branch.add_layout(Layout::new::<CachePadded<Branch<K, V, P::Charge>>>())?;\n    existing.add(leaf, leaves)?;\n    existing.add(branch, branches)?;\n    Ok(existing)\n}",)),
)
PREPARATION_OWNER_BINDINGS += SUCCESSOR_ACQUISITION_BINDINGS

# One pair owns both actual acquisitions before either policy or cursor can
# unwind. These exact bodies preserve allocation/refund and notification owners;
# they do not claim complete carrier admission or enable retained execution.
FRESH_PAIR_ACQUISITION_BINDINGS = (
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCell::acquire_writer', ("pub fn acquire_writer(&self) -> LinCowCellWriterAcquisition<'_, T, R, U, Charge> {\n        let (guard, poisoned) = match self.write.lock() {\n            Ok(guard) => (guard, false),\n            Err(error) => (error.into_inner(), true),\n        };\n        LinCowCellWriterAcquisition {\n            guard,\n            caller: self,\n            poisoned,\n        }\n    }",)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellWriterAcquisition::is_poisoned', ('pub fn is_poisoned(&self) -> bool {\n        self.poisoned\n    }',)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCellWriterAcquisition::write_with', ('pub fn write_with(\n        self,\n        input: impl FnOnce(&T) -> T::WriterInput,\n    ) -> LinCowCellWriteTxn<\'a, T, R, U> {\n        match self.try_write_charged(|data, _| {\n            Ok::<_, std::convert::Infallible>(WriterAdmission {\n                charges: WriterCharges {\n                    cursor: Untracked,\n                    reader: Untracked,\n                },\n                input: input(data),\n            })\n        }) {\n            Ok(writer) => writer,\n            Err((_acquired, WriterAdmissionError::Poisoned)) => {\n                panic!("original writer is poisoned")\n            }\n            Err((_, WriterAdmissionError::Refused(never))) => match never {},\n        }\n    }',)),
    ('vendor/concread/src/internals/lincowcell/mod.rs', 'method', 'LinCowCell::write_with', ("pub fn write_with(\n        &self,\n        input: impl FnOnce(&T) -> T::WriterInput,\n    ) -> LinCowCellWriteTxn<'_, T, R, U> {\n        self.acquire_writer().write_with(input)\n    }",)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMap::acquire_writer', ("pub fn acquire_writer(&self) -> BptreeMapWriterAcquisition<'_, K, V, M> {\n        BptreeMapWriterAcquisition {\n            inner: self.inner.acquire_writer(),\n        }\n    }",)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapWriterAcquisition::is_poisoned', ('pub fn is_poisoned(&self) -> bool {\n        self.inner.is_poisoned()\n    }',)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapWriterAcquisition::write', ("pub fn write(self) -> BptreeMapWriteTxn<'a, K, V> {\n        BptreeMapWriteTxn {\n            inner: self.inner.write_with(|_| ()),\n        }\n    }",)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMap::write', ("pub fn write(&self) -> BptreeMapWriteTxn<'_, K, V> {\n        self.acquire_writer().write()\n    }",)),
    ('vendor/concread/src/bptree/admission.rs', 'method', 'BptreeMap::try_write_admitted', ("pub fn try_write_admitted<E>(\n        &self,\n        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,\n    ) -> Result<BptreeMapWriteTxn<'_, K, V, Prepaid<P>>, MapAdmissionError<E>> {\n        let Some(acquired) = self.try_acquire_writer() else {\n            return Err(MapAdmissionError::Busy);\n        };\n        match acquired.try_write_admitted(admit) {\n            Ok(writer) => Ok(writer),\n            Err((acquired, error)) => {\n                drop(acquired);\n                Err(error)\n            }\n        }\n    }",)),
    ('vendor/concread/src/bptree/admission.rs', 'method', 'BptreeMap::try_clear_admitted', ("pub fn try_clear_admitted<E>(\n        &self,\n        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,\n    ) -> Result<BptreeMapWriteTxn<'_, K, V, Prepaid<P>>, MapAdmissionError<E>> {\n        let Some(acquired) = self.try_acquire_writer() else {\n            return Err(MapAdmissionError::Busy);\n        };\n        match acquired.try_clear_admitted(admit) {\n            Ok(writer) => Ok(writer),\n            Err((acquired, error)) => {\n                drop(acquired);\n                Err(error)\n            }\n        }\n    }",)),
    ('vendor/concread/src/bptree/admission.rs', 'method', 'BptreeMapWriterAcquisition::try_write_admitted', ('pub fn try_write_admitted<E>(\n        self,\n        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,\n    ) -> Result<BptreeMapWriteTxn<\'a, K, V, Prepaid<P>>, (Self, MapAdmissionError<E>)> {\n        let acquired = self.inner.try_write_charged(|source, shells| {\n            let plan = plan_writer_start::<K, V, P>(source, shells)\n                .map_err(MapAdmissionError::Planning)?;\n            let mut provider = Prepaid(Some(\n                admit(plan.demand).map_err(MapAdmissionError::Refused)?,\n            ));\n            let first_charge = provider.take_node_charge(plan.tracking_layout);\n            let first = FixedTrackingBuffer::try_new(0, first_charge)\n                .unwrap_or_else(|_| unreachable!("planned empty first buffer layout"));\n            let last_charge = provider.take_node_charge(plan.tracking_layout);\n            let last = FixedTrackingBuffer::try_new(0, last_charge)\n                .unwrap_or_else(|_| unreachable!("planned empty retirement buffer layout"));\n            let charges = WriterCharges {\n                cursor: provider.take_node_charge(shells.cursor),\n                reader: provider.take_node_charge(shells.reader),\n            };\n            Ok(WriterAdmission {\n                charges,\n                input: (provider, first, last),\n            })\n        });\n        let mut writer = match acquired {\n            Ok(writer) => writer,\n            Err((inner, error)) => {\n                let error = match error {\n                    WriterAdmissionError::Poisoned => MapAdmissionError::Poisoned,\n                    WriterAdmissionError::Refused(error) => error,\n                };\n                return Err((Self { inner }, error));\n            }\n        };\n        // Seal this no-edit operation under the same panic discipline as an\n        // insertion: cleanup must succeed before the cursor becomes operable.\n        writer.as_mut().begin_admitted_edit();\n        writer.as_mut().finish_admitted_funding();\n        Ok(BptreeMapWriteTxn { inner: writer })\n    }',)),
    ('vendor/concread/src/bptree/admission.rs', 'method', 'BptreeMapWriterAcquisition::try_clear_admitted', ('pub fn try_clear_admitted<E>(\n        self,\n        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,\n    ) -> Result<BptreeMapWriteTxn<\'a, K, V, Prepaid<P>>, (Self, MapAdmissionError<E>)> {\n        let acquired = self.inner.try_write_charged(|source, shells| {\n            checked_next_generation(source.txid)\n                .ok_or(MapAdmissionError::Planning(PlanningError::Overflow))?;\n            // SAFETY: this exact source is retained under the original writer.\n            let mut plan = unsafe { plan_tree_clear::<K, V, P>(source.root, [(0, 0); 2]) }\n                .map_err(MapAdmissionError::Planning)?;\n            for layout in [shells.cursor, shells.reader] {\n                plan.demand\n                    .add_layout(layout)\n                    .map_err(MapAdmissionError::Planning)?;\n            }\n            let mut provider = admit(plan.demand).map_err(MapAdmissionError::Refused)?;\n            let first = allocate_tracking::<K, V, P>(plan.first, &mut provider)\n                .expect("one empty leaf requires original first-seen storage");\n            let last = allocate_tracking::<K, V, P>(plan.last, &mut provider)\n                .expect("every tree has an original root to retire");\n            let charges = WriterCharges {\n                cursor: provider.take_node_charge(shells.cursor),\n                reader: provider.take_node_charge(shells.reader),\n            };\n            Ok(WriterAdmission {\n                charges,\n                input: (Prepaid(Some(provider)), first, last),\n            })\n        });\n        let mut inner = match acquired {\n            Ok(writer) => writer,\n            Err((inner, error)) => {\n                let error = match error {\n                    WriterAdmissionError::Poisoned => MapAdmissionError::Poisoned,\n                    WriterAdmissionError::Refused(error) => error,\n                };\n                return Err((Self { inner }, error));\n            }\n        };\n        inner.as_mut().begin_admitted_edit();\n        inner\n            .as_mut()\n            .try_clear()\n            .expect("complete original clear plan");\n        inner.as_mut().finish_admitted_funding();\n        Ok(BptreeMapWriteTxn { inner })\n    }',)),
    ('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_map_pair_preserving_release', ('pub fn try_map_pair_preserving_release<\'other, S, A, B, E>(\n        mut self,\n        mut other: ReleaseGuard<\'other, S>,\n        consume: impl FnOnce(T, S) -> Result<(A, B), E>,\n        observe_poison: impl Fn() -> (bool, bool),\n    ) -> Result<(ReleaseGuard<\'owner, A>, ReleaseGuard<\'other, B>), E> {\n        struct Signal<\'a> {\n            notification: &\'a ReleaseNotification,\n            poisoned: bool,\n        }\n        impl Drop for Signal<\'_> {\n            fn drop(&mut self) {\n                self.notification.released(self.poisoned);\n            }\n        }\n        struct PairSignals<\'a, \'b, F: Fn() -> (bool, bool)> {\n            first: &\'a ReleaseNotification,\n            second: &\'b ReleaseNotification,\n            observe_poison: F,\n            armed: bool,\n        }\n        impl<F: Fn() -> (bool, bool)> Drop for PairSignals<\'_, \'_, F> {\n            fn drop(&mut self) {\n                if !self.armed {\n                    return;\n                }\n                let (first, second) = (self.observe_poison)();\n                let first = Signal {\n                    notification: self.first,\n                    poisoned: first,\n                };\n                let second = Signal {\n                    notification: self.second,\n                    poisoned: second,\n                };\n                // Both verdicts are frozen before the first callback. Unwind\n                // must still deliver the other original release with its own\n                // physical verdict, not the callback\'s panic state.\n                drop(first);\n                drop(second);\n            }\n        }\n        let mut signals = PairSignals {\n            first: self.notification,\n            second: other.notification,\n            observe_poison,\n            armed: true,\n        };\n        let first = self.inner.take().expect("owned first release guard");\n        let second = other.inner.take().expect("owned second release guard");\n        // Only empty wrappers remain; the pair owns both original signals.\n        let _first = std::mem::ManuallyDrop::new(self);\n        let _second = std::mem::ManuallyDrop::new(other);\n        match consume(first, second) {\n            Ok((first, second)) => {\n                signals.armed = false;\n                Ok((\n                    ReleaseGuard {\n                        inner: Some(first),\n                        notification: _first.notification,\n                        poison_on_unwind: _first.poison_on_unwind,\n                    },\n                    ReleaseGuard {\n                        inner: Some(second),\n                        notification: _second.notification,\n                        poison_on_unwind: _second.poison_on_unwind,\n                    },\n                ))\n            }\n            Err(error) => {\n                drop(signals);\n                Err(error)\n            }\n        }\n    }',)),
    ('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::release_pair_with', ('pub fn release_pair_with<S, R>(\n        self,\n        other: ReleaseGuard<\'_, S>,\n        consume: impl FnOnce(T, S) -> R,\n        observe_poison: impl Fn() -> (bool, bool),\n    ) -> R {\n        match self.try_map_pair_preserving_release::<_, (), (), R>(\n            other,\n            |first, second| Err(consume(first, second)),\n            observe_poison,\n        ) {\n            Err(result) => result,\n            Ok(_) => unreachable!("release never transfers physical owners"),\n        }\n    }',)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::initialize', ('    fn initialize(&mut self, mode: BlockMode) {\n        assert!(!self.started, "original storage acquisition is one-shot");\n        self.started = true;\n        let target = self.target;\n        let AcquisitionPhase::Pending { undo, current } = &mut self.phase else {\n            panic!("original pending acquisition");\n        };\n        *undo = WriterPhase::Raw(\n            target\n                .revert_released\n                .poisoning_guard(target.revert.acquire_writer()),\n        );\n        assert!(!undo.is_poisoned(), "original undo writer is poisoned");\n        *current = WriterPhase::Raw(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );\n        assert!(\n            !current.is_poisoned(),\n            "original storage writer is poisoned"\n        );\n        let raw = undo.take_raw();\n        *undo = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.undo_release,\n                    |undo| Ok::<_, (_, std::convert::Infallible)>(undo.write()),\n                    || target.revert.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original undo release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        let raw = current.take_raw();\n        *current = WriterPhase::Writer(\n            match raw\n                .try_map_preserving_release_into(\n                    &mut self.current_release,\n                    |current| Ok::<_, (_, std::convert::Infallible)>(current.write()),\n                    || target.blocks.is_poisoned(),\n                )\n                .unwrap_or_else(|_| unreachable!("original current release source"))\n            {\n                Ok(writer) => writer,\n                Err((_, never)) => match never {},\n            },\n        );\n        // The caller slot owns completed writers before identity lookup and all\n        // reset/replacement operations that may invoke arbitrary payload code.\n        let predecessor = target.publication.capture();\n        let writers = StorageWriters::new(target, undo.take_writer(), current.take_writer());\n        self.phase = AcquisitionPhase::Block(Block::new(\n            writers,\n            mode == BlockMode::Replace,\n            predecessor,\n            mode,\n        ));\n        let AcquisitionPhase::Block(block) = &mut self.phase else {\n            unreachable!("original storage block");\n        };\n        block.failed = true;\n        let OriginalWriters { revert, blocks } = block.writers.as_mut();\n        if mode == BlockMode::Replace {\n            for (key, value) in revert.iter() {\n                match value {\n                    None => blocks.remove(key),\n                    Some(value) => blocks.insert(key.clone(), value.clone()),\n                };\n            }\n        }\n        revert.clear();\n        block.failed = false;\n        self.complete = true;\n    }',)),
    ('crates/mv/src/storage.rs', 'method', 'Storage::block', ("    pub fn block(&self) -> Block<'_, K, V> {\n        let mut slot = self.block_acquisition();\n        crate::BlockAcquisition::initialize(&mut slot, BlockMode::Ordinary);\n        crate::BlockAcquisition::into_block(slot)\n    }",)),
    ('crates/mv/src/storage.rs', 'method', 'Storage::block_and_revert', ("    pub fn block_and_revert(&self) -> Block<'_, K, V> {\n        let mut slot = self.block_acquisition();\n        crate::BlockAcquisition::initialize(&mut slot, BlockMode::Replace);\n        crate::BlockAcquisition::into_block(slot)\n    }",)),
    ('crates/mv/src/storage/admitted.rs', 'method', 'Storage::open_admitted_writers', ('fn open_admitted_writers(&self) -> Result<AdmittedWriters<\'_, K, V, P>, AdmittedStorageError> {\n        let budget = self\n            .allocation\n            .as_ref()\n            .expect("admitted Storage original pool");\n        let current = BptreeMap::<K, V, Prepaid<P>>::writer_start_allocation_demand()\n            .map_err(AdmittedStorageError::Planning)?;\n        let undo = BptreeMap::<K, Option<V>, Prepaid<P>>::writer_start_allocation_demand()\n            .map_err(AdmittedStorageError::Planning)?;\n        let identity =\n            NextPublication::allocation_demand().map_err(AdmittedStorageError::Planning)?;\n        let (current, undo, identity) = reserve_owners(budget, current, undo, identity)?;\n        let undo_wait = self.revert_released.observe();\n        let revert = self.revert.try_acquire_writer().ok_or_else(|| {\n            writer_error(\n                MapAdmissionError::Busy,\n                StorageRole::Undo,\n                undo_wait.clone(),\n            )\n        })?;\n        let revert = self.revert_released.poisoning_guard(revert);\n        if revert.is_poisoned() {\n            revert.release_with_observed_poison(drop, || self.revert.is_poisoned());\n            return Err(AdmittedStorageError::Poisoned {\n                role: StorageRole::Undo,\n            });\n        }\n        let current_wait = self.blocks_released.observe();\n        let blocks = self.blocks.try_acquire_writer().ok_or_else(|| {\n            writer_error(\n                MapAdmissionError::Busy,\n                StorageRole::Current,\n                current_wait.clone(),\n            )\n        })?;\n        let blocks = self.blocks_released.poisoning_guard(blocks);\n        let (revert, blocks) = revert.try_map_pair_preserving_release(\n            blocks,\n            |revert, blocks| {\n                // Both actual poison checks precede either cursor allocation.\n                if blocks.is_poisoned() {\n                    return Err(AdmittedStorageError::Poisoned {\n                        role: StorageRole::Current,\n                    });\n                }\n                let revert = revert\n                    .try_write_admitted(|demand| policy::<P>(budget, undo, demand))\n                    .map_err(|(acquired, error)| {\n                        drop(acquired);\n                        writer_error(error, StorageRole::Undo, undo_wait)\n                    })?;\n                let blocks = blocks\n                    .try_write_admitted(|demand| policy::<P>(budget, current, demand))\n                    .map_err(|(acquired, error)| {\n                        drop(acquired);\n                        writer_error(error, StorageRole::Current, current_wait)\n                    })?;\n                Ok((revert, blocks))\n            },\n            || (self.revert.is_poisoned(), self.blocks.is_poisoned()),\n        )?;\n        // Refused/poisoned acquisition must not allocate an unused identity.\n        // Its original reservation already exists; both writers now belong to\n        // this opening, before reset, replacement copying or user execution.\n        let writers = StorageWriters::new(self, revert, blocks);\n        let next = NextPublication::from_admission(identity);\n        Ok(AdmittedWriters { writers, next })\n    }',)),
    ('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::release_with_observed_poison', ('pub fn release_with_observed_poison<R>(\n        mut self,\n        consume: impl FnOnce(T) -> R,\n        observe_poison: impl Fn() -> bool,\n    ) -> R {\n        struct Signal<\'a, F: Fn() -> bool> {\n            notification: &\'a ReleaseNotification,\n            observe_poison: F,\n        }\n        impl<F: Fn() -> bool> Drop for Signal<\'_, F> {\n            fn drop(&mut self) {\n                self.notification.released((self.observe_poison)());\n            }\n        }\n        let signal = Signal {\n            notification: self.notification,\n            observe_poison,\n        };\n        let inner = self.inner.take().expect("owned release guard");\n        let _transferred = std::mem::ManuallyDrop::new(self);\n        let result = consume(inner);\n        drop(signal);\n        result\n    }',)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::release', ('    fn release(&mut self) {\n        self.complete = false;\n        self.started = true;\n        match &mut self.phase {\n            AcquisitionPhase::Empty => {}\n            AcquisitionPhase::Block(block) => crate::BlockRetirement::release_writers(block),\n            AcquisitionPhase::Pending { undo, current } => {\n                current.release(&self.target.blocks, &mut self.current_release);\n                undo.release(&self.target.revert, &mut self.undo_release);\n            }\n        }\n    }',)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'WriterPhase::release', ('    fn release(&mut self, target: &BptreeMap<K, V>, releases: &mut DeferredReleaseBatch) {\n        match std::mem::replace(self, Self::Empty) {\n            Self::Empty => {}\n            Self::Raw(raw) => {\n                raw.try_release_into_observed(releases, drop, || target.is_poisoned())\n                    .unwrap_or_else(|_| unreachable!("original raw release source"));\n            }\n            Self::Writer(writer) => {\n                let retirement = writer\n                    .try_release_into_observed(\n                        releases,\n                        |writer| writer.abort_retaining(),\n                        || target.is_poisoned(),\n                    )\n                    .unwrap_or_else(|_| unreachable!("original converted release source"));\n                *self = Self::Retired(retirement);\n            }\n            Self::Retired(retirement) => *self = Self::Retired(retirement),\n        }\n    }',)),
)
PREPARATION_OWNER_BINDINGS += FRESH_PAIR_ACQUISITION_BINDINGS

# The original EBR raw guard and private allocation remain separate during
# construction; the persistent MV pair releases both writers before retirement.
# These exact owner bodies bind cleanup/order, not full payload/runtime funding.
EBR_PAIR_ACQUISITION_BINDINGS = (
    ('vendor/concread/src/ebrcell/mod.rs', 'struct', 'EbrCellWriterAcquisition', ("pub struct EbrCellWriterAcquisition<\n    'a,\n    T: Clone + Send + Sync + 'static,\n    Charge: Send + Sync + 'static = Untracked,\n> {\n    caller: &'a EbrCell<T, Charge>,\n    guard: MutexGuard<'a, ()>,\n}",)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCell::acquire_writer', ("    pub fn acquire_writer(&self) -> EbrCellWriterAcquisition<'_, T, Charge> {\n        EbrCellWriterAcquisition {\n            caller: self,\n            guard: self\n                .write\n                .lock()\n                .unwrap_or_else(|poison| poison.into_inner()),\n        }\n    }",)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCell::try_acquire_writer', ("    pub fn try_acquire_writer(&self) -> Option<EbrCellWriterAcquisition<'_, T, Charge>> {\n        let guard = match self.write.try_lock() {\n            Ok(guard) => guard,\n            Err(TryLockError::Poisoned(poison)) => poison.into_inner(),\n            Err(TryLockError::WouldBlock) => return None,\n        };\n        Some(EbrCellWriterAcquisition {\n            caller: self,\n            guard,\n        })\n    }",)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellWriterAcquisition::is_poisoned', ('    pub fn is_poisoned(&self) -> bool {\n        self.caller.is_poisoned()\n    }',)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellWriterAcquisition::try_clone_charged', ('    pub fn try_clone_charged<E>(\n        self,\n        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,\n    ) -> Result<(Self, EbrCellOwned<T, Charge>), (Self, EbrCellWriterAdmissionError<E>)> {\n        if self.is_poisoned() {\n            return Err((self, EbrCellWriterAdmissionError::Poisoned));\n        }\n        // SAFETY: this original writer excludes replacement, and its borrowed\n        // cell excludes destruction. The active allocation therefore cannot be\n        // unlinked while admission and cloning run; no collector pin is needed.\n        let current = self\n            .caller\n            .active\n            .load(Acquire, unsafe { epoch::unprotected() });\n        let current = unsafe { current.deref() };\n        let charge = match admit(&current.value, EbrCell::<T, Charge>::allocation_layout()) {\n            Ok(charge) => ManuallyDrop::new(charge),\n            Err(error) => return Err((self, EbrCellWriterAdmissionError::Refused(error))),\n        };\n        let allocation = Owned::new(Allocation {\n            value: current.value.clone(),\n            charge,\n        });\n        Ok((\n            self,\n            EbrCellOwned {\n                data: Some(allocation),\n            },\n        ))\n    }',)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellWriterAcquisition::try_write_owned', ("    pub fn try_write_owned(\n        self,\n        owned: EbrCellOwned<T, Charge>,\n    ) -> Result<EbrCellWriteTxn<'a, T, Charge>, (Self, EbrCellOwned<T, Charge>)> {\n        if self.is_poisoned() {\n            return Err((self, owned));\n        }\n        Ok(self.install_owned(owned))\n    }",)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellWriterAcquisition::install_owned', ("    fn install_owned(self, mut owned: EbrCellOwned<T, Charge>) -> EbrCellWriteTxn<'a, T, Charge> {\n        EbrCellWriteTxn {\n            data: owned.data.take(),\n            caller: self.caller,\n            _guard: Some(self.guard),\n        }\n    }",)),
    ('vendor/concread/src/ebrcell/mod.rs', 'struct', 'EbrCellWriteTxn', ("pub struct EbrCellWriteTxn<\n    'a,\n    T: 'static + Clone + Send + Sync,\n    Charge: Send + Sync + 'static = Untracked,\n> {\n    data: Option<Owned<Allocation<T, Charge>>>,\n    // This way we know who to contact for updating our data ....\n    caller: &'a EbrCell<T, Charge>,\n    _guard: Option<MutexGuard<'a, ()>>,\n}",)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellWriteTxn::drop', ('    fn drop(&mut self) {\n        // No payload destructor or capacity refund may run under this writer.\n        // Aggregates additionally retain the private allocation until every\n        // sibling guard releases, using detach rather than sequential Drop.\n        drop(self._guard.take());\n        if let Some(allocation) = self.data.take() {\n            reclaim(allocation);\n        }\n    }',)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCellWriteTxn::detach', ('    pub fn detach(mut self) -> EbrCellOwned<T, Charge> {\n        EbrCellOwned {\n            data: self.data.take(),\n        }\n    }',)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCell::write_from_guard', ('    fn write_from_guard<\'a, E>(\n        &\'a self,\n        mguard: MutexGuard<\'a, ()>,\n        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,\n    ) -> Result<EbrCellWriteTxn<\'a, T, Charge>, E> {\n        let acquired = EbrCellWriterAcquisition {\n            caller: self,\n            guard: mguard,\n        };\n        match acquired.try_clone_charged(admit) {\n            Ok((acquired, owned)) => Ok(acquired.install_owned(owned)),\n            Err((_acquired, EbrCellWriterAdmissionError::Poisoned)) => {\n                panic!("original writer is poisoned")\n            }\n            Err((_acquired, EbrCellWriterAdmissionError::Refused(error))) => Err(error),\n        }\n    }',)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCell::write_charged', ("    pub fn write_charged<E>(\n        &self,\n        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,\n    ) -> Result<EbrCellWriteTxn<'_, T, Charge>, E> {\n        self.write_from_guard(self.write.lock().unwrap(), admit)\n    }",)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCell::try_write_charged', ("    pub fn try_write_charged<E>(\n        &self,\n        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,\n    ) -> Result<Option<EbrCellWriteTxn<'_, T, Charge>>, E> {\n        let Ok(mguard) = self.write.try_lock() else {\n            return Ok(None);\n        };\n        self.write_from_guard(mguard, admit).map(Some)\n    }",)),
    ('vendor/concread/src/ebrcell/mod.rs', 'method', 'EbrCell::try_write_owned', ("    pub fn try_write_owned(\n        &self,\n        owner: EbrCellOwned<T, Charge>,\n    ) -> Result<EbrCellWriteTxn<'_, T, Charge>, EbrCellOwned<T, Charge>> {\n        let Some(acquired) = self.try_acquire_writer() else {\n            return Err(owner);\n        };\n        acquired\n            .try_write_owned(owner)\n            .map_err(|(acquired, owner)| {\n                drop(acquired);\n                owner\n            })\n    }",)),
    ('crates/mv/src/cell/acquisition.rs', 'struct', 'BlockAcquisitionSlot', ("pub struct BlockAcquisitionSlot<'a, V: Value, C: Send + Sync + 'static = Untracked> {\n    target: &'a Cell<V, C>,\n    phase: AcquisitionPhase<'a, V, C>,\n    started: bool,\n    complete: bool,\n    // Last: original payload/charge cleanup precedes original notification.\n    undo_release: DeferredReleaseBatch,\n    current_release: DeferredReleaseBatch,\n}",)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::drop', ('    fn drop(&mut self) {\n        crate::BlockAcquisition::release(self);\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'struct', 'OriginalCellWriters', ("pub(super) struct OriginalCellWriters<'a, V: Value, C: Send + Sync + 'static> {\n    pub(super) revert: CellWriter<'a, Option<V>, C>,\n    pub(super) blocks: CellWriter<'a, V, C>,\n}",)),
    ('crates/mv/src/cell/acquisition.rs', 'struct', 'CellWriters', ("pub(super) struct CellWriters<'a, V: Value, C: Send + Sync + 'static> {\n    state: Option<CellWriterState<'a, V, C>>,\n}",)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::acquire', ("    pub(super) fn acquire(target: &'a Cell<V, C>, charges: CellAllocationCharges<C>) -> Self {\n        let mut slot = BlockAcquisitionSlot::new(target, charges);\n        slot.initialize_writers();\n        slot.take_writers()\n    }",)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::into_original', ('    pub(super) fn into_original(mut self) -> OriginalCellWriters<\'a, V, C> {\n        match self.state.take() {\n            Some(CellWriterState::Attached(original)) => original,\n            other => {\n                self.state = other;\n                panic!("original cell pair was released")\n            }\n        }\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::detach_retaining', ('    pub(super) fn detach_retaining(\n        self,\n    ) -> (\n        EbrCellOwned<Option<V>, C>,\n        EbrCellOwned<V, C>,\n        crate::CaptureCleanup,\n    ) {\n        let OriginalCellWriters { revert, blocks } = self.into_original();\n        // The capture slot checked attachment while still owning its Block.\n        // Native detachment only moves these exact allocations and unlocks.\n        let (revert, undo_release) = revert.release_deferred(|writer| writer.detach());\n        let (blocks, current_release) = blocks.release_deferred(|writer| writer.detach());\n        (\n            revert,\n            blocks,\n            crate::CaptureCleanup::new(current_release, undo_release),\n        )\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::drop', ('    fn drop(&mut self) {\n        self.release();\n    }',)),
    ('crates/mv/src/cell.rs', 'method', 'Cell::acquire_charged_writers', ("    fn acquire_charged_writers(\n        &self,\n        charges: CellAllocationCharges<Charge>,\n    ) -> CellWriters<'_, V, Charge> {\n        CellWriters::acquire(self, charges)\n    }",)),
    ('crates/mv/src/cell.rs', 'method', 'Cell::block_charged', ("    pub fn block_charged(&self, charges: CellAllocationCharges<Charge>) -> Block<'_, V, Charge> {\n        let mut slot = self.block_acquisition_charged(charges);\n        crate::BlockAcquisition::initialize(&mut slot, BlockMode::Ordinary);\n        crate::BlockAcquisition::into_block(slot)\n    }",)),
    ('crates/mv/src/cell.rs', 'method', 'Cell::block_and_revert_charged', ("    pub fn block_and_revert_charged(\n        &self,\n        charges: CellAllocationCharges<Charge>,\n    ) -> Block<'_, V, Charge> {\n        let mut slot = self.block_acquisition_charged(charges);\n        crate::BlockAcquisition::initialize(&mut slot, BlockMode::Replace);\n        crate::BlockAcquisition::into_block(slot)\n    }",)),
    ('crates/mv/src/cell.rs', 'method', 'Cell::current_replacement_charged', ("    pub fn current_replacement_charged(\n        &self,\n        charges: CellAllocationCharges<Charge>,\n    ) -> CurrentReplacement<'_, V, Charge> {\n        CurrentReplacement {\n            writers: self.acquire_charged_writers(charges),\n            publication: &self.publication,\n        }\n    }",)),
    ('crates/mv/src/cell.rs', 'struct', 'CurrentReplacement', ("pub struct CurrentReplacement<'storage, V: Value, Charge: Send + Sync + 'static = Untracked> {\n    writers: CellWriters<'storage, V, Charge>,\n    publication: &'storage Publication,\n}",)),
    ('crates/mv/src/cell.rs', 'method', 'CurrentReplacement::publish', ('    pub fn publish(self, value: V) {\n        let Self {\n            mut writers,\n            publication,\n        } = self;\n        *writers.as_mut().blocks.get_mut() = value;\n        publish_pair(writers, publication, NextPublication::new(), true, false);\n    }',)),
    ('crates/mv/src/cell.rs', 'struct', 'Block', ("    pub struct Block<'storage, V: Value, Charge: Send + Sync + 'static = Untracked> {\n        pub(super) writers: CellWriters<'storage, V, Charge>,\n        pub(super) dirty: bool,\n        pub(super) publication: &'storage Publication,\n        pub(super) predecessor: CapturedPublication,\n        pub(super) mode: BlockMode,\n    }",)),
    ('crates/mv/src/cell.rs', 'method', 'Block::commit', ('        pub fn commit(self) {\n            let Self {\n                writers,\n                dirty,\n                publication,\n                predecessor: _,\n                mode: _,\n            } = self;\n            // Even an untouched block publishes its clear-undo transition and\n            // rotates pair identity before either writer can notify a waiter.\n            publish_pair(writers, publication, NextPublication::new(), dirty, true);\n        }',)),
    ('crates/mv/src/cell.rs', 'method', 'Block::try_detach', ('        pub fn try_detach<Admission, E>(\n            self,\n            admit: impl FnOnce(&Self) -> Result<Admission, E>,\n        ) -> Result<Detached<V, Admission, Charge>, E> {\n            let mut slot = self.capture_slot();\n            crate::BlockCapture::try_capture(&mut slot, admit)?;\n            let (journal, cleanup) = crate::BlockCapture::into_detached(slot);\n            drop(cleanup);\n            Ok(journal)\n        }',)),
    ('crates/mv/src/cell.rs', 'fn', 'publish_pair', ("fn publish_pair<'a, V: Value, Charge: Send + Sync + 'static>(\n    writers: CellWriters<'a, V, Charge>,\n    publication: &Publication,\n    next: NextPublication,\n    publish_current: bool,\n    publish_undo: bool,\n) {\n    let retirement = publication.publish_retaining(\n        next,\n        || {\n            // Retain the complete joint owner until the fallible identity-lock\n            // acquisition above succeeds. Native preparation and publication\n            // below neither allocate nor execute payload or collector callbacks.\n            let OriginalCellWriters { revert, blocks } = writers.into_original();\n            let (blocks, unchanged_blocks) = if publish_current {\n                (\n                    Some(blocks.map_preserving_release(|writer| writer.prepare_commit())),\n                    None,\n                )\n            } else {\n                (None, Some(blocks))\n            };\n            let (revert, unchanged_revert) = if publish_undo {\n                (\n                    Some(revert.map_preserving_release(|writer| writer.prepare_commit())),\n                    None,\n                )\n            } else {\n                (None, Some(revert))\n            };\n            let blocks =\n                blocks.map(|writer| writer.map_preserving_release(|prepared| prepared.publish()));\n            let revert =\n                revert.map(|writer| writer.map_preserving_release(|prepared| prepared.publish()));\n            (blocks, revert, unchanged_blocks, unchanged_revert)\n        },\n        |(blocks, revert, unchanged_blocks, unchanged_revert)| {\n            let blocks =\n                blocks.map(|writer| writer.release_retaining(|published| published.release()));\n            let revert =\n                revert.map(|writer| writer.release_retaining(|published| published.release()));\n            let unchanged_blocks =\n                unchanged_blocks.map(|writer| writer.release_retaining(|writer| writer.detach()));\n            let unchanged_revert =\n                unchanged_revert.map(|writer| writer.release_retaining(|writer| writer.detach()));\n            (blocks, revert, unchanged_blocks, unchanged_revert)\n        },\n    );\n    drop(retirement);\n}",)),
    ('crates/mv/src/publication.rs', 'method', 'Publication::publish_retaining', ('    pub(crate) fn publish_retaining<Published, Retirement>(\n        &self,\n        next: NextPublication,\n        publish: impl FnOnce() -> Published,\n        release: impl FnOnce(Published) -> Retirement,\n    ) -> Retirement {\n        // Declare retirement first so unwind also releases the visibility lock\n        // before the old identity can refund its original allocation credits.\n        let retired_version;\n        let mut version = self.lock_version();\n        let published = publish();\n        retired_version = std::mem::replace(&mut **version, next.0);\n        let retirement = release(published);\n        drop(version);\n        drop(retired_version);\n        retirement\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::initialize_writers', ('    fn initialize_writers(&mut self) {\n        assert!(!self.started, "original cell acquisition is one-shot");\n        self.started = true;\n        let target = self.target;\n        let AcquisitionPhase::Pending(pending) = &mut self.phase else {\n            panic!("original pending acquisition");\n        };\n        pending.revert = Some(\n            target\n                .revert_released\n                .poisoning_guard(target.revert.acquire_writer()),\n        );\n        assert!(\n            !pending\n                .revert\n                .as_ref()\n                .expect("original undo")\n                .is_poisoned(),\n            "original undo writer is poisoned",\n        );\n        pending.blocks = Some(\n            target\n                .blocks_released\n                .poisoning_guard(target.blocks.acquire_writer()),\n        );\n        assert!(\n            !pending\n                .blocks\n                .as_ref()\n                .expect("original current")\n                .is_poisoned(),\n            "original current writer is poisoned",\n        );\n        // The original slot owns both guards and both charges before either clone.\n        let undo = pending.revert.take().expect("original undo");\n        let undo_value = &mut pending.undo_value;\n        let undo_charge = &mut pending.undo_charge;\n        let result = undo\n            .try_map_preserving_release_into(\n                &mut self.undo_release,\n                |undo| match undo.try_clone_charged(|_, _| {\n                    Ok::<_, std::convert::Infallible>(\n                        undo_charge.take().expect("original undo charge"),\n                    )\n                }) {\n                    Ok((undo, value)) => {\n                        *undo_value = Some(value);\n                        Ok(undo)\n                    }\n                    Err((undo, error)) => Err((undo, error)),\n                },\n                || target.revert.is_poisoned(),\n            )\n            .unwrap_or_else(|_| unreachable!("original undo release source"));\n        match result {\n            Ok(undo) => pending.revert = Some(undo),\n            Err((undo, error)) => {\n                pending.revert = Some(undo);\n                match error {\n                    EbrCellWriterAdmissionError::Poisoned => {\n                        panic!("original undo writer is poisoned")\n                    }\n                    EbrCellWriterAdmissionError::Refused(never) => match never {},\n                }\n            }\n        }\n        let current = pending.blocks.take().expect("original current");\n        let current_value = &mut pending.current_value;\n        let current_charge = &mut pending.current_charge;\n        let result = current\n            .try_map_preserving_release_into(\n                &mut self.current_release,\n                |current| match current.try_clone_charged(|_, _| {\n                    Ok::<_, std::convert::Infallible>(\n                        current_charge.take().expect("original current charge"),\n                    )\n                }) {\n                    Ok((current, value)) => {\n                        *current_value = Some(value);\n                        Ok(current)\n                    }\n                    Err((current, error)) => Err((current, error)),\n                },\n                || target.blocks.is_poisoned(),\n            )\n            .unwrap_or_else(|_| unreachable!("original current release source"));\n        match result {\n            Ok(current) => pending.blocks = Some(current),\n            Err((current, error)) => {\n                pending.blocks = Some(current);\n                match error {\n                    EbrCellWriterAdmissionError::Poisoned => {\n                        panic!("original current writer is poisoned")\n                    }\n                    EbrCellWriterAdmissionError::Refused(never) => match never {},\n                }\n            }\n        }\n        // No new lock or user operation occurs during either attachment. Keep the\n        // original values in the caller slot if the native attachment refuses.\n        let undo = pending.revert.take().expect("original undo");\n        let undo_value = &mut pending.undo_value;\n        let undo = match undo\n            .try_map_preserving_release_into(\n                &mut self.undo_release,\n                |undo| match undo.try_write_owned(undo_value.take().expect("original undo value")) {\n                    Ok(writer) => Ok(writer),\n                    Err((undo, value)) => {\n                        *undo_value = Some(value);\n                        Err((undo, ()))\n                    }\n                },\n                || target.revert.is_poisoned(),\n            )\n            .unwrap_or_else(|_| unreachable!("original undo release source"))\n        {\n            Ok(writer) => writer,\n            Err((undo, ())) => {\n                pending.revert = Some(undo);\n                panic!("original healthy undo stays acquired");\n            }\n        };\n        // Attachment cannot panic or refuse after the same exclusive healthy\n        // acquisitions above; no payload code runs between these two transfers.\n        let current = pending.blocks.take().expect("original current");\n        let current_value = pending\n            .current_value\n            .take()\n            .expect("original current value");\n        let current = current.map_preserving_release(|current| {\n            match current.try_write_owned(current_value) {\n                Ok(writer) => writer,\n                Err(_) => unreachable!("original healthy current stays acquired"),\n            }\n        });\n        self.phase = AcquisitionPhase::Writers(CellWriters::new(undo, current));\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::initialize', ('    fn initialize(&mut self, mode: BlockMode) {\n        self.initialize_writers();\n        let predecessor = self.target.publication.capture();\n        let writers = self.take_writers();\n        self.phase = AcquisitionPhase::Block(Block::new(\n            writers,\n            mode == BlockMode::Replace,\n            &self.target.publication,\n            predecessor,\n            mode,\n        ));\n        let AcquisitionPhase::Block(block) = &mut self.phase else {\n            unreachable!("original block");\n        };\n        let OriginalCellWriters { revert, blocks } = block.writers.as_mut();\n        match mode {\n            BlockMode::Ordinary => *revert.get_mut() = None,\n            BlockMode::Replace => {\n                if let Some(value) = core::mem::take(revert.get_mut()) {\n                    *blocks.get_mut() = value;\n                }\n            }\n        }\n        self.complete = true;\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::release', ('    fn release(&mut self) {\n        self.complete = false;\n        self.started = true;\n        match &mut self.phase {\n            AcquisitionPhase::Empty => {}\n            AcquisitionPhase::Block(block) => crate::BlockRetirement::release_writers(block),\n            AcquisitionPhase::Writers(writers) => writers.release(),\n            AcquisitionPhase::Pending(pending) => {\n                let target = self.target;\n                if let Some(current) = pending.blocks.take() {\n                    current\n                        .try_release_into_observed(&mut self.current_release, drop, || {\n                            target.blocks.is_poisoned()\n                        })\n                        .unwrap_or_else(|_| unreachable!("original current release source"));\n                }\n                if let Some(undo) = pending.revert.take() {\n                    undo.try_release_into_observed(&mut self.undo_release, drop, || {\n                        target.revert.is_poisoned()\n                    })\n                    .unwrap_or_else(|_| unreachable!("original undo release source"));\n                }\n            }\n        }\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::release', ('    pub(super) fn release(&mut self) {\n        let Some(CellWriterState::Attached(original)) = self.state.as_ref() else {\n            return;\n        };\n        let _ = original;\n        let Some(CellWriterState::Attached(OriginalCellWriters { revert, blocks })) =\n            self.state.take()\n        else {\n            unreachable!()\n        };\n        let (undo, undo_release) = revert.release_deferred(|writer| writer.detach());\n        let (current, current_release) = blocks.release_deferred(|writer| writer.detach());\n        self.state = Some(CellWriterState::Released {\n            _undo: undo,\n            _current: current,\n            _undo_release: undo_release,\n            _current_release: current_release,\n        });\n    }',)),
    ('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::release_deferred', ('    pub fn release_deferred<R>(self, release: impl FnOnce(T) -> R) -> (R, DeferredRelease) {\n        let poisoned = self.poison_on_unwind && std::thread::panicking();\n        let mut retirement = self.release_retaining(release);\n        let notification = DeferredRelease {\n            notification: ReleaseNotification {\n                state: Arc::clone(&retirement.notification.state),\n            },\n            poisoned,\n        };\n        let retained = retirement.inner.take().expect("owned release retirement");\n        let _transferred = std::mem::ManuallyDrop::new(retirement);\n        (retained, notification)\n    }',)),
)
PREPARATION_OWNER_BINDINGS += EBR_PAIR_ACQUISITION_BINDINGS

# Original caller-owned slots survive callee unwind. Complete owners release
# all physical writers in place before payloads, charges and native wakes drop.
# Capture/commit into_fields transfers remain separate, explicitly open boundaries.
AGGREGATE_ACQUISITION_BINDINGS = (
    ('crates/mv/src/cell/acquisition.rs', 'enum', 'AcquisitionPhase', ("enum AcquisitionPhase<'a, V: Value, C: Send + Sync + 'static> {\n    Empty,\n    Pending(PendingPair<'a, V, C>),\n    Writers(CellWriters<'a, V, C>),\n    Block(Block<'a, V, C>),\n}",)),
    ('crates/mv/src/cell/acquisition.rs', 'struct', 'PendingPair', ("struct PendingPair<'a, V: Value, C: Send + Sync + 'static> {\n    revert: Option<ReleaseGuard<'a, EbrCellWriterAcquisition<'a, Option<V>, C>>>,\n    blocks: Option<ReleaseGuard<'a, EbrCellWriterAcquisition<'a, V, C>>>,\n    undo_value: Option<EbrCellOwned<Option<V>, C>>,\n    current_value: Option<EbrCellOwned<V, C>>,\n    undo_charge: Option<C>,\n    current_charge: Option<C>,\n}",)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::take_writers', ('    fn take_writers(&mut self) -> CellWriters<\'a, V, C> {\n        match std::mem::replace(&mut self.phase, AcquisitionPhase::Empty) {\n            AcquisitionPhase::Writers(writers) => writers,\n            other => {\n                self.phase = other;\n                panic!("original initialized pair")\n            }\n        }\n    }',)),
    ('crates/mv/src/storage/acquisition.rs', 'enum', 'AcquisitionPhase', ("enum AcquisitionPhase<'a, K: Key, V: Value> {\n    Empty,\n    Pending {\n        undo: WriterPhase<'a, K, Option<V>>,\n        current: WriterPhase<'a, K, V>,\n    },\n    Block(Block<'a, K, V>),\n}",)),
    ('crates/mv/src/storage/acquisition.rs', 'enum', 'WriterPhase', ("enum WriterPhase<'a, K: Key, V: Value> {\n    Empty,\n    Raw(ReleaseGuard<'a, BptreeMapWriterAcquisition<'a, K, V>>),\n    Writer(ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V>>),\n    Retired(BptreeMapAbandonment<K, V>),\n}",)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'WriterPhase::is_poisoned', ('    fn is_poisoned(&self) -> bool {\n        match self {\n            Self::Raw(raw) => raw.is_poisoned(),\n            _ => panic!("original raw writer"),\n        }\n    }',)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'WriterPhase::take_raw', ('    fn take_raw(&mut self) -> ReleaseGuard<\'a, BptreeMapWriterAcquisition<\'a, K, V>> {\n        match std::mem::replace(self, Self::Empty) {\n            Self::Raw(raw) => raw,\n            other => {\n                *self = other;\n                panic!("original raw writer")\n            }\n        }\n    }',)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'WriterPhase::take_writer', ('    fn take_writer(&mut self) -> ReleaseGuard<\'a, BptreeMapWriteTxn<\'a, K, V>> {\n        match std::mem::replace(self, Self::Empty) {\n            Self::Writer(writer) => writer,\n            other => {\n                *self = other;\n                panic!("original converted writer")\n            }\n        }\n    }',)),
    ('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_map_preserving_release_into', ('    pub fn try_map_preserving_release_into<R, E>(\n        mut self,\n        batch: &mut DeferredReleaseBatch,\n        consume: impl FnOnce(T) -> Result<R, (T, E)>,\n        observe_poison: impl Fn() -> bool,\n    ) -> Result<Result<ReleaseGuard<\'owner, R>, (Self, E)>, Self> {\n        if !Arc::ptr_eq(&self.notification.state, &batch.notification.state) {\n            return Err(self);\n        }\n        struct Record<\'a, F: Fn() -> bool> {\n            batch: &\'a mut DeferredReleaseBatch,\n            observe_poison: F,\n            armed: bool,\n        }\n        impl<F: Fn() -> bool> Drop for Record<\'_, F> {\n            fn drop(&mut self) {\n                if self.armed {\n                    self.batch.released = true;\n                    self.batch.poisoned |= (self.observe_poison)();\n                }\n            }\n        }\n        let mut record = Record {\n            batch,\n            observe_poison,\n            armed: true,\n        };\n        let inner = self.inner.take().expect("original physical guard");\n        let transferred = std::mem::ManuallyDrop::new(self);\n        let result = consume(inner);\n        record.armed = false;\n        drop(record);\n        Ok(match result {\n            Ok(inner) => Ok(ReleaseGuard {\n                inner: Some(inner),\n                notification: transferred.notification,\n                poison_on_unwind: transferred.poison_on_unwind,\n            }),\n            Err((inner, error)) => Err((\n                Self {\n                    inner: Some(inner),\n                    notification: transferred.notification,\n                    poison_on_unwind: transferred.poison_on_unwind,\n                },\n                error,\n            )),\n        })\n    }',)),
    ('vendor/concread/src/release.rs', 'method', 'ReleaseGuard::try_release_into_observed', ('    pub fn try_release_into_observed<R>(\n        mut self,\n        batch: &mut DeferredReleaseBatch,\n        release: impl FnOnce(T) -> R,\n        observe_poison: impl Fn() -> bool,\n    ) -> Result<R, Self> {\n        if !Arc::ptr_eq(&self.notification.state, &batch.notification.state) {\n            return Err(self);\n        }\n        struct Record<\'a, F: Fn() -> bool> {\n            batch: &\'a mut DeferredReleaseBatch,\n            observe_poison: F,\n        }\n        impl<F: Fn() -> bool> Drop for Record<\'_, F> {\n            fn drop(&mut self) {\n                self.batch.released = true;\n                self.batch.poisoned |= (self.observe_poison)();\n            }\n        }\n        let record = Record {\n            batch,\n            observe_poison,\n        };\n        let inner = self.inner.take().expect("original physical guard");\n        let _transferred = std::mem::ManuallyDrop::new(self);\n        let result = release(inner);\n        drop(record);\n        Ok(result)\n    }',)),
    ('vendor/concread/src/bptree/mod.rs', 'struct', 'BptreeMapAbandonment', ("pub struct BptreeMapAbandonment<K, V, M = Untracked>\nwhere\n    K: Clone + Ord + Debug + Send + Sync + 'static,\n    V: Clone + Send + Sync + 'static,\n    M: MapMode + NodeCloning<K, V>,\n{\n    _inner:\n        LinCowCellOwned<SuperBlock<K, V, M>, CursorRead<K, V, M>, CursorWrite<K, V, M>, M::Charge>,\n}",)),
    ('vendor/concread/src/bptree/mod.rs', 'method', 'BptreeMapWriteTxn::abort_retaining', ('    pub fn abort_retaining(self) -> BptreeMapAbandonment<K, V, M> {\n        BptreeMapAbandonment {\n            _inner: self.inner.detach(),\n        }\n    }',)),
    ('crates/mv/src/cell.rs', 'method', 'Cell::block_acquisition', ("    pub fn block_acquisition(&self) -> BlockAcquisitionSlot<'_, V> {\n        self.block_acquisition_charged(CellAllocationCharges::untracked())\n    }",)),
    ('crates/mv/src/cell.rs', 'method', 'Cell::block_acquisition_charged', ("    pub fn block_acquisition_charged(\n        &self,\n        charges: CellAllocationCharges<Charge>,\n    ) -> BlockAcquisitionSlot<'_, V, Charge> {\n        BlockAcquisitionSlot::new(self, charges)\n    }",)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::new', ("    pub(super) fn new(target: &'a Cell<V, C>, charges: CellAllocationCharges<C>) -> Self {\n        let CellAllocationCharges { current, undo } = charges;\n        Self {\n            target,\n            phase: AcquisitionPhase::Pending(PendingPair {\n                revert: None,\n                blocks: None,\n                undo_value: None,\n                current_value: None,\n                undo_charge: Some(undo),\n                current_charge: Some(current),\n            }),\n            started: false,\n            complete: false,\n            undo_release: target.revert_released.deferred_batch(),\n            current_release: target.blocks_released.deferred_batch(),\n        }\n    }",)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'BlockAcquisitionSlot::into_block', ('    fn into_block(mut self) -> Self::Block {\n        assert!(\n            self.complete,\n            "original cell initialization did not complete"\n        );\n        match std::mem::replace(&mut self.phase, AcquisitionPhase::Empty) {\n            AcquisitionPhase::Block(block) => block,\n            other => {\n                self.phase = other;\n                panic!("original completed cell block")\n            }\n        }\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'enum', 'CellWriterState', ("enum CellWriterState<'a, V: Value, C: Send + Sync + 'static> {\n    Attached(OriginalCellWriters<'a, V, C>),\n    Released {\n        _undo: EbrCellOwned<Option<V>, C>,\n        _current: EbrCellOwned<V, C>,\n        _undo_release: DeferredRelease,\n        _current_release: DeferredRelease,\n    },\n}",)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::new', ("    fn new(revert: CellWriter<'a, Option<V>, C>, blocks: CellWriter<'a, V, C>) -> Self {\n        Self {\n            state: Some(CellWriterState::Attached(OriginalCellWriters {\n                revert,\n                blocks,\n            })),\n        }\n    }",)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::as_ref', ('    pub(super) fn as_ref(&self) -> &OriginalCellWriters<\'a, V, C> {\n        match self.state.as_ref() {\n            Some(CellWriterState::Attached(original)) => original,\n            _ => panic!("original cell pair was released"),\n        }\n    }',)),
    ('crates/mv/src/cell/acquisition.rs', 'method', 'CellWriters::as_mut', ('    pub(super) fn as_mut(&mut self) -> &mut OriginalCellWriters<\'a, V, C> {\n        match self.state.as_mut() {\n            Some(CellWriterState::Attached(original)) => original,\n            _ => panic!("original cell pair was released"),\n        }\n    }',)),
    ('crates/mv/src/cell.rs', 'method', 'Block::release_writers', ('    fn release_writers(&mut self) {\n        self.writers.release();\n    }',)),
    ('crates/mv/src/storage.rs', 'method', 'Storage::block_acquisition', ("    pub fn block_acquisition(&self) -> BlockAcquisitionSlot<'_, K, V> {\n        BlockAcquisitionSlot::new(self)\n    }",)),
    ('crates/mv/src/storage/acquisition.rs', 'struct', 'BlockAcquisitionSlot', ("pub struct BlockAcquisitionSlot<'a, K: Key, V: Value> {\n    target: &'a Storage<K, V>,\n    phase: AcquisitionPhase<'a, K, V>,\n    started: bool,\n    complete: bool,\n    // Last: recorded native notifications survive payload/charge destruction.\n    undo_release: DeferredReleaseBatch,\n    current_release: DeferredReleaseBatch,\n}",)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::new', ("    pub(super) fn new(target: &'a Storage<K, V>) -> Self {\n        Self {\n            target,\n            phase: AcquisitionPhase::Pending {\n                undo: WriterPhase::Empty,\n                current: WriterPhase::Empty,\n            },\n            started: false,\n            complete: false,\n            undo_release: target.revert_released.deferred_batch(),\n            current_release: target.blocks_released.deferred_batch(),\n        }\n    }",)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::into_block', ('    fn into_block(mut self) -> Self::Block {\n        assert!(\n            self.complete,\n            "original storage initialization did not complete"\n        );\n        match std::mem::replace(&mut self.phase, AcquisitionPhase::Empty) {\n            AcquisitionPhase::Block(block) => block,\n            other => {\n                self.phase = other;\n                panic!("original completed storage block")\n            }\n        }\n    }',)),
    ('crates/mv/src/storage/acquisition.rs', 'method', 'BlockAcquisitionSlot::drop', ('    fn drop(&mut self) {\n        crate::BlockAcquisition::release(self);\n    }',)),
    ('crates/mv/src/storage.rs', 'enum', 'StorageWriterState', ("enum StorageWriterState<'a, K: Key, V: Value, M: StorageMode<K, V>> {\n    Attached(OriginalWriters<'a, K, V, M>),\n    Released {\n        _blocks: BptreeMapAbandonment<K, V, M>,\n        _revert: BptreeMapAbandonment<K, Option<V>, M>,\n        _blocks_release: concread::release::DeferredRelease,\n        _revert_release: concread::release::DeferredRelease,\n    },\n}",)),
    ('crates/mv/src/storage.rs', 'struct', 'StorageWriters', ("struct StorageWriters<'target, K: Key, V: Value, M: StorageMode<K, V>> {\n    state: Option<StorageWriterState<'target, K, V, M>>,\n    target: &'target Storage<K, V, M>,\n}",)),
    ('crates/mv/src/storage.rs', 'method', 'StorageWriters::new', ("    fn new(\n        target: &'target Storage<K, V, M>,\n        revert: ReleaseGuard<'target, BptreeMapWriteTxn<'target, K, Option<V>, M>>,\n        blocks: ReleaseGuard<'target, BptreeMapWriteTxn<'target, K, V, M>>,\n    ) -> Self {\n        Self {\n            state: Some(StorageWriterState::Attached(OriginalWriters {\n                revert,\n                blocks,\n            })),\n            target,\n        }\n    }",)),
    ('crates/mv/src/storage.rs', 'method', 'StorageWriters::as_ref', ('    fn as_ref(&self) -> &OriginalWriters<\'target, K, V, M> {\n        match self.state.as_ref() {\n            Some(StorageWriterState::Attached(original)) => original,\n            _ => panic!("original storage pair was released"),\n        }\n    }',)),
    ('crates/mv/src/storage.rs', 'method', 'StorageWriters::as_mut', ('    fn as_mut(&mut self) -> &mut OriginalWriters<\'target, K, V, M> {\n        match self.state.as_mut() {\n            Some(StorageWriterState::Attached(original)) => original,\n            _ => panic!("original storage pair was released"),\n        }\n    }',)),
    ('crates/mv/src/storage.rs', 'method', 'StorageWriters::into_original', ('    fn into_original(mut self) -> OriginalWriters<\'target, K, V, M> {\n        match self.state.take() {\n            Some(StorageWriterState::Attached(original)) => original,\n            other => {\n                self.state = other;\n                panic!("original storage pair was released")\n            }\n        }\n    }',)),
    ('crates/mv/src/storage.rs', 'method', 'StorageWriters::release', ('    fn release(&mut self) {\n        if !matches!(&self.state, Some(StorageWriterState::Attached(_))) {\n            return;\n        }\n        let Some(StorageWriterState::Attached(OriginalWriters { revert, blocks })) =\n            self.state.take()\n        else {\n            unreachable!()\n        };\n        // Unlike detach, cleanup-only native retirement also accepts an edit-failed\n        // private cursor. It grants no read or publication authority afterward.\n        let (blocks, blocks_release) = blocks.release_deferred(|writer| writer.abort_retaining());\n        let (revert, revert_release) = revert.release_deferred(|writer| writer.abort_retaining());\n        self.state = Some(StorageWriterState::Released {\n            _blocks: blocks,\n            _revert: revert,\n            _blocks_release: blocks_release,\n            _revert_release: revert_release,\n        });\n    }',)),
    ('crates/mv/src/storage.rs', 'method', 'StorageWriters::drop', ('    fn drop(&mut self) {\n        self.release();\n    }',)),
    ('crates/mv/src/storage.rs', 'method', 'Block::release_writers', ('    fn release_writers(&mut self) {\n        self.writers.release();\n    }',)),
)
PREPARATION_OWNER_BINDINGS += AGGREGATE_ACQUISITION_BINDINGS

WORLD_ACQUISITION_BINDINGS = (
    ('crates/iroha_core/src/state/world_acquisition.rs', 'macro', 'declare_world_acquisition', ('macro_rules! declare_world_acquisition {\n    (; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {\n        // Generic parameter names deliberately follow the single field census.\n        #[allow(non_camel_case_types)]\n        pub(super) struct WorldAcquisition<\n            $($prefix: BlockAcquisition,)*\n            $($privacy: BlockAcquisition,)*\n            $($suffix: BlockAcquisition,)*\n        > {\n            $(pub(super) $prefix: Option<$prefix>,)*\n            $(pub(super) $privacy: Option<$privacy>,)*\n            $(pub(super) $suffix: Option<$suffix>,)*\n        }\n\n        #[allow(non_camel_case_types)]\n        impl<$($prefix: BlockAcquisition,)* $($privacy: BlockAcquisition,)* $($suffix: BlockAcquisition,)*>\n            WorldAcquisition<$($prefix,)* $($privacy,)* $($suffix,)*>\n        {\n            pub(super) fn initialize(&mut self, mode: BlockMode) {\n                $(self.$prefix.as_mut().expect("original field acquisition").initialize(mode);)*\n                $(self.$privacy.as_mut().expect("original field acquisition").initialize(mode);)*\n                $(self.$suffix.as_mut().expect("original field acquisition").initialize(mode);)*\n            }\n        }\n\n        #[allow(non_camel_case_types)]\n        impl<$($prefix: BlockAcquisition,)* $($privacy: BlockAcquisition,)* $($suffix: BlockAcquisition,)*>\n            Drop for WorldAcquisition<$($prefix,)* $($privacy,)* $($suffix,)*>\n        {\n            fn drop(&mut self) {\n                $(if let Some(field) = self.$prefix.as_mut() { field.release(); })*\n                $(if let Some(field) = self.$privacy.as_mut() { field.release(); })*\n                $(if let Some(field) = self.$suffix.as_mut() { field.release(); })*\n                // Automatic field drop now reclaims payloads/charges and wakes\n                // original waiters only after all acquired writers are free.\n            }\n        }\n\n        impl BlockRetirement for WorldBlock<\'_> {\n            fn release_writers(&mut self) {\n                if let Some(fields) = self.fields.as_mut() {\n                    $(fields.$prefix.release_writers();)*\n                    $(fields.$privacy.release_writers();)*\n                    $(fields.$suffix.release_writers();)*\n                }\n            }\n        }\n    };\n}',)),
    ('crates/iroha_core/src/state/world_acquisition.rs', 'macro', 'build_world_block_from_fields', ('macro_rules! build_world_block_from_fields {\n    ($state:expr, $mode:expr;\n        [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {{\n        use mv::BlockAcquisition as _;\n        let mut pending = world_acquisition::WorldAcquisition {\n            $($prefix: None,)*\n            $($privacy: None,)*\n            $($suffix: None,)*\n        };\n        // Fill the same caller-owned slots one at a time before acquiring any\n        // writer, without retaining a full composite initializer temporary.\n        world_acquisition::fill_world_acquisition(|| {\n            $(pending.$prefix = Some($state.$prefix.block_acquisition());)*\n            $(pending.$privacy = Some($state.$privacy.block_acquisition());)*\n            $(pending.$suffix = Some($state.$suffix.block_acquisition());)*\n        });\n        pending.initialize($mode);\n        world_acquisition::finish_world_acquisition(|| WorldBlock {\n            fields: Some(WorldBlockFields {\n                dataspace_catalog: iroha_data_model::nexus::DataSpaceCatalog::default(),\n                $($prefix: pending.$prefix.take().expect("original field acquisition").into_block(),)*\n                $($privacy: pending.$privacy.take().expect("original field acquisition").into_block(),)*\n                $($suffix: pending.$suffix.take().expect("original field acquisition").into_block(),)*\n                external_event_buf: Vec::new(),\n            }),\n        })\n    }};\n}',)),
    ('crates/iroha_core/src/state/world_acquisition.rs', 'macro', 'build_world_block', ('macro_rules! build_world_block {\n    ($state:expr, $mode:expr) => {\n        with_world_overlay_fields!(build_world_block_from_fields, $state, ($mode))\n    };\n}',)),
    ('crates/iroha_core/src/state/world_acquisition.rs', 'method', 'WorldBlock::drop', ('    fn drop(&mut self) {\n        self.release_writers();\n    }',)),
    ('crates/iroha_core/src/state/world_acquisition.rs', 'method', 'WorldBlock::into_fields', ('    pub(super) fn into_fields(mut self) -> WorldBlockFields<\'world> {\n        // TODO: retain joint retirement through consuming commit. Capture now\n        // installs caller-owned slots before invoking any native operation.\n        self.fields.take().expect("original World block fields")\n    }',)),
    ('crates/iroha_core/src/state.rs', 'struct', 'WorldBlock', ("pub struct WorldBlock<'world> {\n    fields: Option<WorldBlockFields<'world>>,\n}",)),
    ('crates/iroha_core/src/state.rs', 'method', 'World::block', ("    pub fn block(&self) -> WorldBlock<'_> {\n        build_world_block!(self, mv::BlockMode::Ordinary)\n    }",)),
    ('crates/iroha_core/src/state.rs', 'method', 'World::block_and_revert', ("    pub fn block_and_revert(&self) -> WorldBlock<'_> {\n        build_world_block!(self, mv::BlockMode::Replace)\n    }",)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_acquisition.rs', 'macro', 'trigger_acquisition', ('macro_rules! trigger_acquisition {\n    ($($field:ident: ($key:ty, $value:ty)),+ $(,)?) => {\n        /// Inert original field slots retained across all fallible initialization.\n        pub(crate) struct SetBlockAcquisition<\'set> {\n            $($field: Option<mv::storage::BlockAcquisitionSlot<\'set, $key, $value>>,)+\n        }\n\n        impl<\'set> SetBlockAcquisition<\'set> {\n            pub(super) fn new(target: &\'set Set) -> Self {\n                Self { $($field: Some(target.$field.block_acquisition()),)+ }\n            }\n        }\n\n        impl<\'set> BlockAcquisition for SetBlockAcquisition<\'set> {\n            type Block = SetBlock<\'set>;\n\n            fn initialize(&mut self, mode: BlockMode) {\n                $(self.$field.as_mut().expect("original trigger slot").initialize(mode);)+\n            }\n\n            fn release(&mut self) {\n                $(if let Some(field) = self.$field.as_mut() { field.release(); })+\n            }\n\n            fn into_block(mut self) -> Self::Block {\n                SetBlock { fields: Some(SetBlockFields {\n                    $($field: self.$field.take().expect("original trigger slot").into_block(),)+\n                }) }\n            }\n        }\n\n        impl Drop for SetBlockAcquisition<\'_> {\n            fn drop(&mut self) {\n                self.release();\n            }\n        }\n\n        impl BlockRetirement for SetBlock<\'_> {\n            fn release_writers(&mut self) {\n                if let Some(fields) = self.fields.as_mut() {\n                    $(fields.$field.release_writers();)+\n                }\n            }\n        }\n    };\n}',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_acquisition.rs', 'method', 'SetBlock::drop', ('    fn drop(&mut self) {\n        self.release_writers();\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_acquisition.rs', 'method', 'SetBlock::into_fields', ('    pub(super) fn into_fields(mut self) -> SetBlockFields<\'set> {\n        // Capture only performs inert moves before its caller owns every slot.\n        // TODO: retain aggregate retirement through consuming commit too.\n        self.fields.take().expect("original trigger block fields")\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set.rs', 'struct', 'SetBlock', ("pub struct SetBlock<'set> {\n    fields: Option<SetBlockFields<'set>>,\n}",)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set.rs', 'method', 'Set::block_acquisition', ("    pub(crate) fn block_acquisition(&self) -> SetBlockAcquisition<'_> {\n        SetBlockAcquisition::new(self)\n    }",)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set.rs', 'method', 'Set::block', ("    pub fn block(&self) -> SetBlock<'_> {\n        use mv::BlockAcquisition as _;\n        let mut pending = self.block_acquisition();\n        pending.initialize(mv::BlockMode::Ordinary);\n        pending.into_block()\n    }",)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set.rs', 'method', 'Set::block_and_revert', ("    pub fn block_and_revert(&self) -> SetBlock<'_> {\n        use mv::BlockAcquisition as _;\n        let mut pending = self.block_acquisition();\n        pending.initialize(mv::BlockMode::Replace);\n        pending.into_block()\n    }",)),
    ('crates/iroha_core/src/state/world_acquisition.rs', 'fn', 'finish_world_acquisition', ("pub(super) fn finish_world_acquisition<'world>(\n    finish: impl FnOnce() -> WorldBlock<'world>,\n) -> WorldBlock<'world> {\n    finish()\n}",)),
    ('crates/iroha_core/src/state/world_acquisition.rs', 'fn', 'fill_world_acquisition', ('pub(super) fn fill_world_acquisition(fill: impl FnOnce()) {\n    fill();\n}',)),
)
PREPARATION_OWNER_BINDINGS += WORLD_ACQUISITION_BINDINGS

# Capture slots retain original attached owners through validation/admission and
# every sibling release. These exact delegates do not authorize publication or
# turn terminal abandonment into a reusable journal. Commit is a separate owner.
CAPTURE_OWNER_BINDINGS = (
    ('crates/mv/src/capture.rs', 'struct', 'CaptureCleanup', ('pub struct CaptureCleanup {\n    current: Option<DeferredRelease>,\n    undo: Option<DeferredRelease>,\n}',)),
    ('crates/mv/src/capture.rs', 'method', 'CaptureCleanup::new', ('    pub(crate) fn new(current: DeferredRelease, undo: DeferredRelease) -> Self {\n        Self {\n            current: Some(current),\n            undo: Some(undo),\n        }\n    }',)),
    ('crates/mv/src/capture.rs', 'method', 'CaptureCleanup::drop', ('    fn drop(&mut self) {\n        // Taking each field keeps the remaining actual notification armed if\n        // the first callback unwinds. No physical guard remains in this owner.\n        drop(self.current.take());\n        drop(self.undo.take());\n    }',)),
    ('crates/mv/src/cell/capture.rs', 'struct', 'BlockCaptureSlot', ("pub struct BlockCaptureSlot<'a, V: Value, Admission, C: Send + Sync + 'static = Untracked> {\n    phase: CapturePhase<'a, V, Admission, C>,\n    started: bool,\n    // Last: captured payloads and admission outlive all physical writers and\n    // are destroyed before these original notifications on abandonment.\n    cleanup: CaptureCleanup,\n}",)),
    ('crates/mv/src/cell/capture.rs', 'enum', 'CapturePhase', ("enum CapturePhase<'a, V: Value, Admission, C: Send + Sync + 'static> {\n    Empty,\n    Attached {\n        block: Block<'a, V, C>,\n        admission: Option<Admission>,\n    },\n    Captured(Detached<V, Admission, C>),\n}",)),
    ('crates/mv/src/cell/capture.rs', 'method', 'Block::capture_slot', ("    pub fn capture_slot<Admission>(self) -> BlockCaptureSlot<'a, V, Admission, C> {\n        BlockCaptureSlot {\n            phase: CapturePhase::Attached {\n                block: self,\n                admission: None,\n            },\n            started: false,\n            cleanup: CaptureCleanup::default(),\n        }\n    }",)),
    ('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::try_capture', ('    fn try_capture<E>(\n        &mut self,\n        admit: impl FnOnce(&Self::Block) -> Result<Admission, E>,\n    ) -> Result<(), E> {\n        assert!(!self.started, "original cell capture is one-shot");\n        self.started = true;\n        let CapturePhase::Attached {\n            block,\n            admission: retained,\n        } = &mut self.phase\n        else {\n            panic!("original attached cell capture");\n        };\n        // Released cleanup-only blocks must never regain journal authority.\n        block.writers.as_ref();\n        *retained = Some(admit(block)?);\n        let next = NextPublication::new();\n        // From here only infallible original-owner moves/native unlocks occur.\n        // Any future fallible/user operation belongs above this extraction.\n        let CapturePhase::Attached {\n            block,\n            admission: Some(admission),\n        } = std::mem::replace(&mut self.phase, CapturePhase::Empty)\n        else {\n            unreachable!("original checked cell block");\n        };\n        let Block {\n            writers,\n            dirty,\n            predecessor,\n            mode,\n            publication: _,\n        } = block;\n        let (revert, blocks, cleanup) = writers.detach_retaining();\n        self.phase = CapturePhase::Captured(Detached {\n            revert,\n            blocks,\n            metadata: DetachedMetadata {\n                predecessor,\n                mode,\n                dirty,\n                next,\n                admission,\n            },\n        });\n        self.cleanup = cleanup;\n        Ok(())\n    }',)),
    ('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::release', ('    fn release(&mut self) {\n        self.started = true;\n        if let CapturePhase::Attached { block, .. } = &mut self.phase {\n            block.release_writers();\n        }\n    }',)),
    ('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::into_detached', ('    fn into_detached(mut self) -> (Self::Detached, CaptureCleanup) {\n        match std::mem::replace(&mut self.phase, CapturePhase::Empty) {\n            CapturePhase::Captured(journal) => (journal, std::mem::take(&mut self.cleanup)),\n            original => {\n                self.phase = original;\n                panic!("original cell capture did not complete");\n            }\n        }\n    }',)),
    ('crates/mv/src/cell/capture.rs', 'method', 'BlockCaptureSlot::drop', ('    fn drop(&mut self) {\n        self.release();\n    }',)),
    ('crates/mv/src/storage/capture.rs', 'struct', 'BlockCaptureSlot', ("pub struct BlockCaptureSlot<'a, K: Key, V: Value, Admission, M: StorageMode<K, V> = Untracked> {\n    phase: CapturePhase<'a, K, V, Admission, M>,\n    started: bool,\n    cleanup: CaptureCleanup,\n}",)),
    ('crates/mv/src/storage/capture.rs', 'enum', 'CapturePhase', ("enum CapturePhase<'a, K: Key, V: Value, Admission, M: StorageMode<K, V>> {\n    Empty,\n    Attached {\n        block: Block<'a, K, V, M>,\n        admission: Option<Admission>,\n    },\n    Captured(Detached<K, V, Admission, M>),\n}",)),
    ('crates/mv/src/storage/capture.rs', 'method', 'Block::capture_slot', ("    pub fn capture_slot<Admission>(self) -> BlockCaptureSlot<'a, K, V, Admission, M> {\n        BlockCaptureSlot {\n            phase: CapturePhase::Attached {\n                block: self,\n                admission: None,\n            },\n            started: false,\n            cleanup: CaptureCleanup::default(),\n        }\n    }",)),
    ('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::try_capture', ('    fn try_capture<E>(\n        &mut self,\n        admit: impl FnOnce(&Self::Block) -> Result<Admission, E>,\n    ) -> Result<(), E> {\n        assert!(!self.started, "original map capture is one-shot");\n        self.started = true;\n        let CapturePhase::Attached {\n            block,\n            admission: retained,\n        } = &mut self.phase\n        else {\n            panic!("original attached map capture");\n        };\n        // Both logical cursor verdicts run before taking the original Block.\n        // A caught panic leaves failed private roots owned here for abandonment.\n        block.assert_operable();\n        *retained = Some(admit(block)?);\n        self.finish_capture();\n        Ok(())\n    }',)),
    ('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::release', ('    fn release(&mut self) {\n        self.started = true;\n        if let CapturePhase::Attached { block, .. } = &mut self.phase {\n            block.release_writers();\n        }\n    }',)),
    ('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::into_detached', ('    fn into_detached(mut self) -> (Self::Detached, CaptureCleanup) {\n        match std::mem::replace(&mut self.phase, CapturePhase::Empty) {\n            CapturePhase::Captured(journal) => (journal, std::mem::take(&mut self.cleanup)),\n            original => {\n                self.phase = original;\n                panic!("original map capture did not complete");\n            }\n        }\n    }',)),
    ('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::drop', ('    fn drop(&mut self) {\n        self.release();\n    }',)),
    ('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::capture_admitted', ('    pub(super) fn capture_admitted(&mut self, admission: Admission) {\n        assert!(!self.started, "original map capture is one-shot");\n        self.started = true;\n        let CapturePhase::Attached {\n            block,\n            admission: retained,\n        } = &mut self.phase\n        else {\n            panic!("original attached map capture");\n        };\n        *retained = Some(admission);\n        block.assert_operable();\n        self.finish_capture();\n    }',)),
    ('crates/mv/src/storage/capture.rs', 'method', 'BlockCaptureSlot::finish_capture', ('    fn finish_capture(&mut self) {\n        let CapturePhase::Attached {\n            block,\n            admission: Some(admission),\n        } = std::mem::replace(&mut self.phase, CapturePhase::Empty)\n        else {\n            unreachable!("original checked map block");\n        };\n        let Block {\n            writers,\n            dirty,\n            failed: _,\n            predecessor,\n            next,\n            mode,\n        } = block;\n        let OriginalWriters { revert, blocks } = writers.into_original();\n        let (blocks, revert, cleanup) = detach_pair_retaining(blocks, revert);\n        self.phase = CapturePhase::Captured(Detached {\n            revert,\n            blocks,\n            metadata: DetachedMetadata {\n                predecessor,\n                mode,\n                dirty,\n                next,\n                admission,\n            },\n        });\n        self.cleanup = cleanup;\n    }',)),
    ('crates/mv/src/storage.rs', 'fn', 'detach_pair_retaining', ("fn detach_pair_retaining<K: Key, V: Value, M: StorageMode<K, V>>(\n    blocks: ReleaseGuard<'_, BptreeMapWriteTxn<'_, K, V, M>>,\n    revert: ReleaseGuard<'_, BptreeMapWriteTxn<'_, K, Option<V>, M>>,\n) -> (\n    BptreeMapOwned<K, V, M>,\n    BptreeMapOwned<K, Option<V>, M>,\n    crate::CaptureCleanup,\n) {\n    // Both cursor flags were checked while the caller still owned its Block.\n    // Exclusive ownership prevents a new edit between that check and detach.\n    let (blocks, current_release) = blocks.release_deferred(|writer| writer.detach());\n    let (revert, undo_release) = revert.release_deferred(|writer| writer.detach());\n    (\n        blocks,\n        revert,\n        crate::CaptureCleanup::new(current_release, undo_release),\n    )\n}",)),
    ('crates/mv/src/storage.rs', 'method', 'Block::detach_owned', ('        pub(super) fn detach_owned<Admission>(\n            self,\n            admission: Admission,\n        ) -> Detached<K, V, Admission, M> {\n            let mut slot = self.capture_slot();\n            slot.capture_admitted(admission);\n            let (journal, cleanup) = crate::BlockCapture::into_detached(slot);\n            drop(cleanup);\n            journal\n        }',)),
    ('crates/mv/src/storage.rs', 'method', 'Block::try_detach', ('        pub fn try_detach<Admission, E>(\n            self,\n            admit: impl FnOnce(&Self) -> Result<Admission, E>,\n        ) -> Result<Detached<K, V, Admission>, E> {\n            let mut slot = self.capture_slot();\n            crate::BlockCapture::try_capture(&mut slot, admit)?;\n            let (journal, cleanup) = crate::BlockCapture::into_detached(slot);\n            drop(cleanup);\n            Ok(journal)\n        }',)),
)
PREPARATION_OWNER_BINDINGS += CAPTURE_OWNER_BINDINGS

# World/Trigger capture uses original caller slots before retaining wrappers.
# Admission/extras outlive pending cleanup; commit remains a separate boundary.
CORE_CAPTURE_BINDINGS = (
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'StorageBlock::into_capture', ('    fn into_capture(self) -> Self::Capture {\n        self.capture_slot()\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'CellBlock::into_capture', ('    fn into_capture(self) -> Self::Capture {\n        self.capture_slot()\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'TriggerSetBlock::into_capture', ('    fn into_capture(self) -> Self::Capture {\n        self.capture_slot()\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'StorageCaptureSlot::capture', ('    fn capture(&mut self) -> Result<(), CaptureError<Infallible>> {\n        match self.try_capture(|_| Ok::<(), Infallible>(())) {\n            Ok(()) => Ok(()),\n            Err(impossible) => match impossible {},\n        }\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'StorageCaptureSlot::release', ('    fn release(&mut self) {\n        BlockCapture::release(self);\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'StorageCaptureSlot::retain', ("    fn retain(self, name: &'static str, target: fn(&World) -> &Self::Target) -> Self::Retained {\n        let (journal, cleanup) = self.into_detached();\n        let retained = RetainedStorage {\n            name,\n            journal: Some(journal),\n            target,\n        };\n        drop(cleanup);\n        retained\n    }",)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'CellCaptureSlot::capture', ('    fn capture(&mut self) -> Result<(), CaptureError<Infallible>> {\n        match self.try_capture(|_| Ok::<(), Infallible>(())) {\n            Ok(()) => Ok(()),\n            Err(impossible) => match impossible {},\n        }\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'CellCaptureSlot::release', ('    fn release(&mut self) {\n        BlockCapture::release(self);\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'CellCaptureSlot::retain', ("    fn retain(self, name: &'static str, target: fn(&World) -> &Self::Target) -> Self::Retained {\n        let (journal, cleanup) = self.into_detached();\n        let retained = RetainedCell {\n            name,\n            journal: Some(journal),\n            target,\n        };\n        drop(cleanup);\n        retained\n    }",)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'SetBlockCapture::capture', ('    fn capture(&mut self) -> Result<(), CaptureError<Infallible>> {\n        self.try_capture(|_| Ok::<(), Infallible>(()))\n            .map_err(trigger_error)\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'SetBlockCapture::release', ('    fn release(&mut self) {\n        Self::release(self);\n    }',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'SetBlockCapture::retain', ("    fn retain(self, name: &'static str, target: fn(&World) -> &Self::Target) -> Self::Retained {\n        let (journal, cleanup) = self.into_detached();\n        let retained = RetainedTriggers {\n            name,\n            journal: Some(journal),\n            target,\n        };\n        drop(cleanup);\n        retained\n    }",)),
    ('crates/iroha_core/src/state/world_journals.rs', 'macro', 'declare_world_capture', ('macro_rules! declare_world_capture {\n    (; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {\n        #[allow(non_camel_case_types)]\n        struct WorldCapture<$($prefix: WorldCaptureSlot,)* $($privacy: WorldCaptureSlot,)* $($suffix: WorldCaptureSlot,)*> {\n            $($prefix: Option<$prefix>,)* $($privacy: Option<$privacy>,)* $($suffix: Option<$suffix>,)*\n        }\n        #[allow(non_camel_case_types)]\n        impl<$($prefix: WorldCaptureSlot,)* $($privacy: WorldCaptureSlot,)* $($suffix: WorldCaptureSlot,)*>\n            WorldCapture<$($prefix,)* $($privacy,)* $($suffix,)*>\n        {\n            fn capture(&mut self) -> Result<(), CaptureError<Infallible>> {\n                $(self.$prefix.as_mut().expect("original World capture slot").capture()?;)*\n                $(self.$privacy.as_mut().expect("original World capture slot").capture()?;)*\n                $(self.$suffix.as_mut().expect("original World capture slot").capture()?;)*\n                Ok(())\n            }\n        }\n        #[allow(non_camel_case_types)]\n        impl<$($prefix: WorldCaptureSlot,)* $($privacy: WorldCaptureSlot,)* $($suffix: WorldCaptureSlot,)*>\n            Drop for WorldCapture<$($prefix,)* $($privacy,)* $($suffix,)*>\n        {\n            fn drop(&mut self) {\n                $(if let Some(field) = self.$prefix.as_mut() { field.release(); })*\n                $(if let Some(field) = self.$privacy.as_mut() { field.release(); })*\n                $(if let Some(field) = self.$suffix.as_mut() { field.release(); })*\n            }\n        }\n    };\n}',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'macro', 'capture_world_fields', ('macro_rules! capture_world_fields {\n    ($original:ident, $admit:ident;\n        [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {{\n        let mode = $original.parameters.mode();\n        $(check_mode!($original, mode, $prefix);)*\n        $(check_mode!($original, mode, $privacy);)*\n        $(check_mode!($original, mode, $suffix);)*\n        // No wrapper/vector/delta allocation or value copy precedes this call.\n        let admission = $admit(&$original).map_err(CaptureError::Admission)?;\n        // These payloads and their admission outlive the capture aggregate on\n        // unwind: release every original writer before either can be destroyed.\n        let mut extras = None;\n        let mut pending = WorldCapture {\n            $($prefix: None,)* $($privacy: None,)* $($suffix: None,)*\n        };\n        fill_world_capture(|| {\n            // Inert moves only after extraction. The closure borrows both\n            // original owners; its transfer temporaries leave before capture.\n            let WorldBlockFields {\n                dataspace_catalog,\n                $($prefix,)* $($privacy,)* $($suffix,)*\n                external_event_buf,\n            } = $original.fields.take().expect("original World block fields");\n            $(pending.$prefix = Some($prefix.into_capture());)*\n            $(pending.$privacy = Some($privacy.into_capture());)*\n            $(pending.$suffix = Some($suffix.into_capture());)*\n            extras = Some((dataspace_catalog, external_event_buf));\n        });\n        pending.capture().map_err(widen_error)?;\n        // All sibling writers are now free. Original notifications can be\n        // retired while materializing the admitted journal wrappers.\n        const FIELD_COUNT: usize = [\n            $(stringify!($prefix),)* $(stringify!($privacy),)* $(stringify!($suffix),)*\n        ].len();\n        let fields = finish_world_capture(|| {\n            let mut fields: Vec<Box<dyn RetainedWorldField>> = Vec::with_capacity(FIELD_COUNT);\n            $(retain_field!(fields, pending, $prefix);)*\n            $(retain_field!(fields, pending, $privacy);)*\n            $(retain_field!(fields, pending, $suffix);)*\n            fields\n        });\n        let (dataspace_catalog, external_event_buf) = extras.take().expect("original World extras");\n        Ok(DetachedWorld { mode, fields, dataspace_catalog, external_event_buf, admission })\n    }};\n}',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'macro', 'retain_field', ('macro_rules! retain_field {\n    ($fields:ident, $pending:ident, $field:ident) => {\n        $fields.push(Box::new(\n            $pending\n                .$field\n                .take()\n                .expect("original World capture slot")\n                .retain(stringify!($field), |target: &World| &target.$field),\n        ));\n    };\n}',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'fn', 'fill_world_capture', ('fn fill_world_capture(fill: impl FnOnce()) {\n    fill()\n}',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'fn', 'finish_world_capture', ('fn finish_world_capture<R>(finish: impl FnOnce() -> R) -> R {\n    finish()\n}',)),
    ('crates/iroha_core/src/state/world_journals.rs', 'method', 'WorldBlock::try_detach_journals', ('    pub(in crate::state) fn try_detach_journals<Admission, E>(\n        mut self,\n        admit: impl FnOnce(&Self) -> Result<Admission, E>,\n    ) -> Result<DetachedWorld<Admission>, CaptureError<E>> {\n        with_world_overlay_fields!(capture_world_fields, self, admit)\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'struct', 'SetBlockCapture', ("pub(crate) struct SetBlockCapture<'set, Admission> {\n    phase: CapturePhase<'set, Admission>,\n    started: bool,\n    // Last: retained payloads precede the original release notifications.\n    cleanup: SetCaptureCleanup,\n}",)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'method', 'SetCaptureCleanup::default', ('    fn default() -> Self {\n        Self(std::array::from_fn(|_| None))\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'method', 'SetCaptureCleanup::drop', ('    fn drop(&mut self) {\n        for cleanup in &mut self.0 {\n            drop(cleanup.take());\n        }\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'enum', 'CapturePhase', ("enum CapturePhase<'set, Admission> {\n    Empty,\n    Attached(SetBlock<'set>),\n    Capturing(CapturingSet<'set, Admission>),\n    Captured(DetachedSet<Admission>),\n}",)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'macro', 'capture_fields', ('macro_rules! capture_fields {\n    ($($field:ident: ($key:ty, $value:ty)),+ $(,)?) => {\n        struct CapturingSet<\'set, Admission> {\n            $($field: Option<mv::storage::BlockCaptureSlot<\'set, $key, $value, ()>>,)+\n            admission: Option<Admission>,\n        }\n\n        impl<\'set, Admission> CapturingSet<\'set, Admission> {\n            fn new(original: SetBlock<\'set>, admission: Admission) -> Self {\n                // Only infallible, inert owner moves occur across this extraction.\n                let SetBlockFields { $($field,)+ } = original.into_fields();\n                Self { $($field: Some($field.capture_slot()),)+ admission: Some(admission) }\n            }\n\n            fn capture(&mut self) {\n                $(match self.$field.as_mut().expect("original trigger capture slot")\n                    .try_capture(|_| Ok::<(), core::convert::Infallible>(() )) {\n                    Ok(()) => {},\n                    Err(impossible) => match impossible {},\n                })+\n            }\n\n            fn release(&mut self) {\n                $(if let Some(field) = self.$field.as_mut() { field.release(); })+\n            }\n\n            fn finish(mut self, mode: mv::BlockMode) -> (DetachedSet<Admission>, SetCaptureCleanup) {\n                // All native capture calls completed before any journal transfer.\n                $(let $field = self.$field.take().expect("original trigger capture slot").into_detached();)+\n                let cleanup = SetCaptureCleanup([$(Some($field.1),)+]);\n                let journal = DetachedSet {\n                    mode,\n                    $($field: $field.0,)+\n                    admission: self.admission.take().expect("original trigger admission"),\n                };\n                (journal, cleanup)\n            }\n        }\n\n        impl<Admission> Drop for CapturingSet<\'_, Admission> {\n            fn drop(&mut self) { self.release(); }\n        }\n    };\n}',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'method', 'SetBlock::capture_slot', ("    pub(crate) fn capture_slot<Admission>(self) -> SetBlockCapture<'set, Admission> {\n        SetBlockCapture {\n            phase: CapturePhase::Attached(self),\n            started: false,\n            cleanup: SetCaptureCleanup::default(),\n        }\n    }",)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'method', 'SetBlockCapture::try_capture', ('    pub(crate) fn try_capture<E>(\n        &mut self,\n        admit: impl FnOnce(&SetBlock<\'set>) -> Result<Admission, E>,\n    ) -> Result<(), DetachError<E>> {\n        assert!(!self.started, "original trigger capture is one-shot");\n        self.started = true;\n        let CapturePhase::Attached(original) = &self.phase else {\n            panic!("original attached trigger capture");\n        };\n        let mode = original.capture_mode().map_err(|error| match error {\n            DetachError::InconsistentMode {\n                field,\n                expected,\n                actual,\n            } => DetachError::InconsistentMode {\n                field,\n                expected,\n                actual,\n            },\n            DetachError::Admission(impossible) => match impossible {},\n        })?;\n        let admission = admit(original).map_err(DetachError::Admission)?;\n        let CapturePhase::Attached(original) =\n            std::mem::replace(&mut self.phase, CapturePhase::Empty)\n        else {\n            unreachable!("original checked trigger block");\n        };\n        self.phase = CapturePhase::Capturing(CapturingSet::new(original, admission));\n        let CapturePhase::Capturing(pending) = &mut self.phase else {\n            unreachable!()\n        };\n        pending.capture();\n        let CapturePhase::Capturing(pending) =\n            std::mem::replace(&mut self.phase, CapturePhase::Empty)\n        else {\n            unreachable!("original completed trigger capture");\n        };\n        let (journal, cleanup) = pending.finish(mode);\n        self.phase = CapturePhase::Captured(journal);\n        self.cleanup = cleanup;\n        Ok(())\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'method', 'SetBlockCapture::release', ('    pub(crate) fn release(&mut self) {\n        self.started = true;\n        match &mut self.phase {\n            CapturePhase::Attached(original) => original.release_writers(),\n            CapturePhase::Capturing(pending) => pending.release(),\n            CapturePhase::Captured(_) | CapturePhase::Empty => {}\n        }\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'method', 'SetBlockCapture::into_detached', ('    pub(crate) fn into_detached(mut self) -> (DetachedSet<Admission>, SetCaptureCleanup) {\n        match std::mem::replace(&mut self.phase, CapturePhase::Empty) {\n            CapturePhase::Captured(journal) => (journal, std::mem::take(&mut self.cleanup)),\n            original => {\n                self.phase = original;\n                panic!("original trigger capture did not complete");\n            }\n        }\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_capture.rs', 'method', 'SetBlockCapture::drop', ('    fn drop(&mut self) {\n        self.release();\n    }',)),
    ('crates/iroha_core/src/smartcontracts/isi/triggers/set_detachment.rs', 'method', 'SetBlock::try_detach', ('    pub(crate) fn try_detach<Admission, E>(\n        self,\n        admit: impl FnOnce(&Self) -> Result<Admission, E>,\n    ) -> Result<DetachedSet<Admission>, DetachError<E>> {\n        let mut pending = self.capture_slot();\n        pending.try_capture(admit)?;\n        let (journal, cleanup) = pending.into_detached();\n        drop(cleanup);\n        Ok(journal)\n    }',)),
)
PREPARATION_OWNER_BINDINGS += CORE_CAPTURE_BINDINGS

PREPARATION_OWNER_BINDINGS = tuple((path, kind, name, tokens + ('Err((block_hashes, cause, hash_retirement)) => {\n                    drop(fences.release_for_completion());\n                    drop(hash_retirement);',) if name == "try_prepare_physical" else tokens) for path, kind, name, tokens in PREPARATION_OWNER_BINDINGS)

_NATIVE_EXPLICIT_SOURCE_RELATIVES = tuple(Path(p) for p in (
    STATE, HASH_RESTORE, RUNNER_HISTORY, LANE_WORK_HISTORY, HASH_ADMISSION, RUNTIME_ACQUISITION, HASH_PUBLICATION, HASH_SURFACE,
    QUEUE_OWNER, PUBLICATION_MUTEX, GEOMETRY_OWNER, RAW_GEOMETRY, SERVICE_QUEUE, CARRIER_QUEUE,
    "crates/iroha_core/src/kura/publication_lease.rs",
    PHYSICAL_CARRIER, TERMINAL_CARRIER, ARCHIVE_CARRIER, GEOMETRY_CARRIER, WITNESS_CARRIER, WITNESS_LEASE,
    APPLY, BLOCK, PREPARED, PREFIX, JOURNALS, WORLD_COMMIT, DECISION_CARRIER, VALIDATION_CUSTODY, RETAINED_VALIDATION,
    OUTPUT, SEAL, TAIL, NATIVE_METADATA, NATIVE_STAGE,
    CONTROLS, NATIVE_SOURCE, NATIVE_KERNEL, NATIVE_CARRIER, NATIVE_FINALIZED, BODY_STORE,
    ORDINARY, CAPACITY, DURABLE, KURA, AUTONOMOUS,
    "scripts/formal/sumeragi_v2_multilane_native_preparation_contract.py",
    "pytests/scripts/sumeragi_v2_multilane_native_preparation_contract_test.py",
))

# Binding owners are authoritative inputs; mutation fixtures must copy every
# referenced owner without a second manually synchronized Rust path inventory.
NATIVE_PREPARATION_SOURCE_RELATIVES = tuple(dict.fromkeys((
    *_NATIVE_EXPLICIT_SOURCE_RELATIVES,
    *(Path(path) for path, _, _, _ in PREPARATION_OWNER_BINDINGS),
)))


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

    # Exact storage shape prevents a second identity/owner registry and keeps
    # the producer alive through payload release and descriptor-charge refunds.
    # Matching method bodies delegate rather than caching a scalar result.
    retained_bodies = {
        "RetainedCarrier": """{
            Capturing(Box<super::StagedCarrierCapture<Admission>>),
            Validated(PreparedCarrierJournals<Admission>),
            Decided(DecisionBoundCarrierJournals<Admission, BindingAdmission>),
            Checkpointed(DecisionBoundCarrierJournals<Admission, BindingAdmission,
                super::DetachedCarrierComponents, crate::kura::KuraWsvCheckpointReceipt>),
        }""",
        "Candidate": "{ subject: wire::BlockSubject, owner: Option<O> }",
        "CarrierMarkerPreparation": """{
            Ready(wire::ExecutionCommitment), Deferred(LocalValidationRefusal), ValidationError(E),
        }""",
        "RetainedBodyValidationService": """{
            candidates: Vec<Candidate<P::Owner>>, markers: Vec<Marker>,
            identity: V2BodyStoreInstanceIdentity, limit: usize,
            _descriptor_admission: [AllocationCharge; 2], validator: P,
        }""",
        "RetainedBodyValidationService::preflight_marker": """{
            let candidate = self.candidates.iter().find(|row| row.subject == durable.subject());
            if candidate.is_some_and(|row| row.owner.is_none()) {
                return Err(CarrierCustodyError::MissingOwner);
            }
            if (!self.markers.iter().any(|row| row.durable == *durable)
                && self.markers.len() == self.limit)
                || (candidate.is_none() && self.candidates.len() == self.limit) {
                return Err(CarrierCustodyError::Capacity);
            }
            Ok(())
        }""",
        "SelectedValidationCarrier": """{
            service: &'a mut RetainedBodyValidationService<P>, index: usize, owner: Option<P::Owner>,
        }""",
        "SelectedValidationCarrier::try_consume": """{
            let owner = self.owner.take().expect("live selection retains its original owner");
            match publish(&self.service.validator, owner) {
                Ok(value) => {
                    let subject = self.service.candidates[self.index].subject;
                    self.service.markers.retain(|row| row.durable.subject() != subject);
                    Ok(value)
                }
                Err((owner, error)) => { self.owner = Some(owner); Err(error) }
            }
        }""",
        "CarrierJournalInputs": """{
            pub(crate) valid: &'owner crate::block::ValidBlock,
            pub(crate) state: &'owner StateBlock<'state>,
            pub(crate) prefix: &'owner ValidatedExecutionPrefix,
            pub(crate) context: &'owner Arc<iroha_data_model::block::consensus_v2::HeightContext>,
            pub(crate) execution_prefix: &'owner iroha_data_model::block::consensus_v2::ExecutionCommitment,
            pub(crate) native_amx_manifest: &'owner crate::sumeragi::exec::NativeAmxApplicationManifestV1,
            pub(crate) da_pins: &'owner Vec<DaPinIntentWithLocation>,
            pub(crate) publication_events: &'owner Vec<EventBox>,
            pub(crate) provider: Option<&'owner ProviderCandidateCapture>,
            pub(crate) reputation: Option<&'owner ReputationCandidateCapture>,
            pub(crate) retained_effects_layout: std::alloc::Layout,
        }""",
        "PreparedWorldEffects": "{ da_pins: Vec<DaPinIntentWithLocation> }",
        "PreparedWorldEffects::admission_pins": "{ let Self { da_pins } = self; da_pins }",
        "RetainedCarrier::matches_validation_candidate": """{ match self {
            Self::Capturing(carrier) => carrier.matches_candidate(context, proposal),
            Self::Validated(journals) => journals.matches_validation_candidate(context, proposal),
            Self::Decided(carrier) => carrier.journals.matches_validation_candidate(context, proposal),
            Self::Checkpointed(carrier) => carrier.journals.matches_validation_candidate(context, proposal),
        } }""",
        "RetainedCarrier::ready_commitment": """{ match self {
            Self::Capturing(_) => None,
            Self::Validated(journals) => Some(journals.execution_prefix_commitment()),
            Self::Decided(carrier) => Some(carrier.journals.execution_prefix_commitment()),
            Self::Checkpointed(carrier) => Some(carrier.journals.execution_prefix_commitment()),
        } }""",
        "RetainedCarrier::resume_capture": """{ match self {
            Self::Capturing(carrier) => carrier.try_complete().map(Self::Validated)
                .map_err(|(carrier, error)| (Self::Capturing(carrier), error)),
            ready => Ok(ready),
        } }""",
        "StagedCarrierCapture::matches_candidate": "{ self.journals.matches_validation_candidate(context, proposal) }",
        "StagedCarrierCapture::try_complete": """{
            if let Err(error) = self.try_prepare_archives() { return Err((self, error)); }
            Ok((*self).into_journals())
        }""",
        "execution_prefix_commitment": "{ self.execution_prefix }",
    }
    _, state_source = _read_reviewed_rust_source(root, STATE, "Native shared history owner", errors)
    if state_source is not None:
        state_code = _code(state_source)
        for relation in (
            "type BlockHashMode = concread::bptree::Prepaid<BlockHashPolicy>;",
            "type BlockHashMap = concread::bptree::BptreeMap<usize, HashOf<BlockHeader>, BlockHashMode>;",
            "type BlockHashWork = concread::bptree::BptreeMapOwned<usize, HashOf<BlockHeader>, BlockHashMode>;",
            "type BlockHashFamily = concread::bptree::BptreeMapFamily<usize, HashOf<BlockHeader>, BlockHashMode>;",
            "pub(crate) struct NativeLaneStateOwner(BlockHashFamily);",
        ):
            if _code(relation) not in state_code:
                errors.append(f"Native preparation shared history loses original physical owner: {relation}")

    _, hash_admission_source = _read_reviewed_rust_source(root, HASH_ADMISSION, "Native prepaid history", errors)
    if hash_admission_source is not None:
        for relation in (
            "pub(super) struct BlockHashPolicy(mv::allocation::AllocationReservation);",
            "type Charge = mv::allocation::AllocationCharge;",
        ):
            if _code(relation) not in _code(hash_admission_source):
                errors.append(f"Native preparation prepaid history loses original charge: {relation}")

    retained_bodies.update({
        "BlockHashAdmissionError::release_wait": "{ match self { Self::Busy(wait) | Self::Changed(wait) => Some(wait), Self::Capacity(mv::allocation::AllocationRefusal::Capacity { release, .. }) => { Some(release) } _ => None, } }",
        "StateBlockStartError::release_wait": "{ match self { Self::History(error) => error.release_wait(), Self::Stage(_) => None, } }",
        "HistoryAdmissionWait::new": "{ let mut pending = Self(wait.wait_for_release()); if pending.is_ready(wake) { wake.wake_by_ref(); } pending }",
        "HistoryAdmissionWait::is_ready": "{ std::future::Future::poll(std::pin::Pin::new(&mut self.0), &mut std::task::Context::from_waker(wake),).is_ready() }",
        "DetachedBlockHashes::observe_current": "{ let Some(map) = target.map() else { return Ok(false); }; self.work.try_matches_current(map) }",
        "DetachedBlockHashes::matches_current": "{ self.observe_current(target) == Ok(true) }",
        "NativeLaneStateOwner::matches_state": "{ state.block_hashes.map().is_some_and(|map| self.0.matches(map)) }",
        "NativeLaneStateOwner::same_family": "{ self.0.same_family(&other.0) }",
        "State::native_lane_state_owner": "{ self.block_hashes.map().map(|map| NativeLaneStateOwner(map.family())) }",
        "BlockHashes::map": "{ match &self.inner { BlockHashStorage::Owned(map) => Some(map), BlockHashStorage::EmergencyFastMapped(_) | BlockHashStorage::EmergencyFastEmpty => None, } }",
        "BlockHashesBlock::pending": "{ BlockHashRange { source: self, start: self.visible_len, end: self.len(), } }",
        "PreparedCarrierGeometry::has_queue_custody": """{
            !self.requires_queue_custody()
                || queue.is_some_and(|queue| queue.authenticates(target, self, header))
        }""",
        "CarrierFences": "{ _state: StateFences<'target>, _queue: Option<CarrierQueueRetirement<'target>>, _kura: KuraPublicationLease<'target>, }",
        "CarrierQueueRetirement": "{ state_owner: NativeLaneStateOwner, header: BlockHeader, routes: Vec<(LaneId, DataSpaceId, Hash)>, _cut: QueueLaneRetirementCut<'queue>, }",
        "OriginalCarrierQueue": "{ state: &'service State, queue: &'service Queue }",
        "OriginalCarrierQueue::new": "{ Self { state, queue } }",
        "OriginalCarrierQueue::belongs_to": "{ core::ptr::eq(self.state, state) }",
        "OriginalCarrierQueue::owns_cut": "{ cut.belongs_to(self.queue) }",
        "CarrierQueueRetirement::ensure_available": """{
            if self._cut.durability_faulted() {
                Err(CarrierQueueRetirementError::Unavailable(QueueLaneRetirementUnavailable::DurabilityFault,))
            } else { Ok(()) }
        }""",
        "RetainedBodyValidationService::descriptor_layouts": """{ Ok([
            Layout::array::<Candidate<P::Owner>>(limit).map_err(|_| AllocationRefusal::DemandOverflow)?,
            Layout::array::<Marker>(limit).map_err(|_| AllocationRefusal::DemandOverflow)?,
        ]) }""",
        "RetainedBodyValidationService::new": """{
            let layouts = Self::descriptor_layouts(limit)?;
            let mut reservation = budget.try_reserve_layouts(layouts)?;
            let descriptor_admission = [reservation.try_split(layouts[0])?, reservation.try_split(layouts[1])?,];
            drop(reservation);
            let mut candidates = Vec::new(); candidates.try_reserve_exact(limit)?;
            let mut markers = Vec::new(); markers.try_reserve_exact(limit)?;
            Ok(Self { validator, identity, candidates, markers, limit, _descriptor_admission: descriptor_admission, })
        }""",
    })
    for symbol, body in retained_bodies.items():
        if symbol in items and items[symbol].partition("{")[2] != _code(body)[1:]:
            errors.append(f"Native preparation retained carrier {symbol} replaces or duplicates original custody")
    ordered("RetainedBodyValidationService::new",
            "let layouts = Self::descriptor_layouts(limit)?", "budget.try_reserve_layouts(layouts)?",
            "reservation.try_split(layouts[0])?", "reservation.try_split(layouts[1])?",
            "drop(reservation)", "candidates.try_reserve_exact(limit)?", "markers.try_reserve_exact(limit)?",
            "Ok(Self {", "_descriptor_admission: descriptor_admission")
    descriptor_new = items.get("RetainedBodyValidationService::new", "")
    for operation in ("budget.try_reserve_layouts(layouts)?", "candidates.try_reserve_exact(limit)?", "markers.try_reserve_exact(limit)?"):
        if descriptor_new and descriptor_new.count(_code(operation)) != 1:
            errors.append(f"Native preparation descriptor admission repeats or omits executable relation {operation}")
    for forbidden in ("drop(descriptor_admission)", "mem::forget", "ManuallyDrop", "AllocationBudget::"):
        if _code(forbidden) in descriptor_new:
            errors.append(f"Native preparation descriptor admission loses executable relation: {forbidden}")
    ordered("RetainedBodyValidationService::prepare_marker",
            "if marker.is_none() && self.markers.len() == self.limit", "return Err(CarrierCustodyError::Capacity)",
            "if requires_existing_owner", "return Err(CarrierCustodyError::MissingOwner)",
            "if self.candidates.len() == self.limit", "return Err(CarrierCustodyError::Capacity)",
            "let index = self.candidates.len()", "self.candidates.push(Candidate {",
            "subject: durable.subject()", "owner: None",
            "self.validator.prepare(context, body)", "Err(error) => {",
            "self.candidates.pop()", "return Ok(CarrierMarkerPreparation::ValidationError(error))",
            "self.candidates[index].owner = Some(owner)",
            "if !owner.matches_candidate(context, body)",
            "let commitment = match owner.ready_commitment()", "Some(commitment) => commitment", "None => {",
            "self.resume_candidate(index, context, body)?",
            "return Ok(CarrierMarkerPreparation::Deferred(refusal))",
            ".ready_commitment().ok_or(CarrierCustodyError::IncompleteCapture)?",
            "self.markers.push(Marker {", "Ok(CarrierMarkerPreparation::Ready(commitment))")
    ordered("RetainedBodyValidationService::resume_candidate", "self.candidates[index].owner.take()",
            "match self.validator.resume(owner)",
            "Ok(owner) => { self.candidates[index].owner = Some(owner); None }",
            "Err((owner, refusal)) => { self.candidates[index].owner = Some(owner); Some(refusal) }",
            "if !owner.matches_candidate(context, body)",
            "Ok(refusal)")
    prepare = items.get("RetainedBodyValidationService::prepare_marker", "")
    if prepare.count(_code("self.validator.prepare(")) != 1:
        errors.append("Native preparation retained carrier repeats execution before or after descriptor admission")
    if prepare.count(_code("self.candidates.push(")) != 1 or prepare.count(_code("self.candidates.pop(")) != 1:
        errors.append("Native preparation retained carrier loses reserved descriptor custody")
    resume = items.get("RetainedBodyValidationService::resume_candidate", "")
    if prepare.count(_code("self.resume_candidate(")) != 1 or resume.count(_code("self.validator.resume(")) != 1:
        errors.append("Native preparation retained carrier does not resume exactly its installed owner")
    if _code("Some(commitment) => commitment, None => {") not in prepare or _code("self.validator.resume(") in prepare:
        errors.append("Native preparation retained carrier moves a ready owner through capture resumption")
    ordered("RetainedBodyValidationService::confirm",
            "owner.ready_commitment() != Some(receipt.execution_commitment())",
            "return Err(CarrierCustodyError::Identity)", "marker.confirmed = Some(receipt.clone())")
    ordered("SelectedValidationCarrier::try_consume", "self.owner.take()", "match publish(&self.service.validator, owner)",
            "Err((owner, error)) =>", "self.owner = Some(owner)", "Err(error)")
    ordered("SelectedValidationCarrier::drop", "self.owner.take()",
            "self.service.candidates[self.index].owner = Some(owner)")
    consume = items.get("SelectedValidationCarrier::try_consume", "")
    for forbidden in (".candidates.remove(", ".candidates.swap_remove(", ".candidates.clear(",
                      ".candidates.retain(", ".validator.prepare(", "ValidBlock::", ".clone()"):
        if _code(forbidden) in consume:
            errors.append(f"Native preparation retained carrier loses its current owner or tombstone: {forbidden}")
    ordered("V2BodyStore::execute_retained_durable_validation",
            "service.matches_store(&self.instance_identity())", "if !self.rejected.contains_key(&key)",
            "service.preflight_marker(&durable)?",
            "self.load_validation_envelope(&durable, expected_manifest_hash)?", "service.prepare_marker(",
            "self.persist_validated_receipt(&durable, commitment)?", "service.confirm(&validated)?")

    # The owner trait's path-qualified impl is outside the generic item parser.
    # Read through the same reviewed-source resolver and permit only its one
    # production phase owner plus the explicitly test-gated custody fixture.
    _, custody_source = _read_reviewed_rust_source(root, VALIDATION_CUSTODY, "Native preparation", errors)
    if custody_source is not None:
        custody = _code(custody_source)
        phase_impl = _code("""
            impl<A: Send + 'static, B: Send + 'static> RetainedValidationOwner
                for crate::state::RetainedCarrier<A, B> {
                fn matches_candidate(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool {
                    self.matches_validation_candidate(context, body)
                }
                fn ready_commitment(&self) -> Option<wire::ExecutionCommitment> { self.ready_commitment() }
            }
        """)
        if phase_impl not in custody or _code("RetainedValidationOwner: sealed::Owner + Send + 'static") not in custody:
            errors.append("Native preparation retained carrier loses its sealed phase delegation")
        resume_api = _code("""fn resume(&mut self, owner: Self::Owner,)
            -> Result<Self::Owner, (Self::Owner, LocalValidationRefusal)>;""")
        if resume_api not in custody:
            errors.append("Native preparation retained carrier loses its original-owner local capture refusal")
        fixture_start = custody.find(_code("#[cfg(test)] pub(in crate::sumeragi) mod test_support {"))
        fixture_end = fixture_start
        if fixture_start >= 0:
            opening = custody.index("{", fixture_start)
            depth = 1
            fixture_end = opening + 1
            while fixture_end < len(custody) and depth:
                depth += (custody[fixture_end] == "{") - (custody[fixture_end] == "}")
                fixture_end += 1
        else:
            errors.append("Native preparation retained carrier exposes the fixture owner in production")
        for trait in ("sealed::Owner", "RetainedValidationOwner"):
            implementations = list(re.finditer(r"impl(?:<[^{}]*?>)?" + re.escape(trait) + r"for([^{}]+)\{", custody))
            if sorted(m[1] for m in implementations) != ["TrackedOwner", "crate::state::RetainedCarrier<A,B>"]:
                errors.append(f"Native preparation retained carrier allows another {trait} implementation")
            if any(m[1] == "TrackedOwner" and not fixture_start < m.start() < fixture_end
                   for m in implementations):
                errors.append("Native preparation retained carrier fixture implementation escapes its test gate")

    # Close the production constructor surface, including the real fixture's gate.
    # Private fields alone do not prevent an added public constructor in this module.
    _, queue_source = _read_reviewed_rust_source(root, SERVICE_QUEUE, "Native preparation", errors)
    if queue_source is not None:
        masked = _mask_rust_comments(queue_source)
        methods = re.findall(r"\bfn\s+(\w+)\s*(?:<[^{}]*>)?\s*\(", masked)
        if sorted(methods) != sorted(("new", "belongs_to", "try_observe", "owns_cut", "for_test")):
            errors.append("Native preparation original Queue source changes closed constructor executable relation")
        fixture_constructor = _code("""#[cfg(test)]
            pub(crate) fn for_test(state: &'service State, queue: &'service Queue) -> Self {
                Self::new(state, queue)
            }""")
        if fixture_constructor not in _code(queue_source):
            errors.append("Native preparation original Queue fixture constructor escapes its test-gated executable relation")

    # The service capability and move-only proof cannot expose a public arbitrary
    # Queue constructor, retain only a scalar observation, or reorder release.
    for symbol, body in {
        "OriginalCarrierQueue": "{ state: &'service State, queue: &'service Queue, }",
        "OriginalCarrierQueue::new": "{ Self { state, queue } }",
        "OriginalCarrierQueue::belongs_to": "{ core::ptr::eq(self.state, state) }",
        "OriginalCarrierQueue::owns_cut": "{ cut.belongs_to(self.queue) }",
        "OriginalCarrierQueue::try_observe": "{ self.queue.try_lock_lane_retirement_observer() }",
        "V2ApplyService::carrier_queue_source": "{ carrier_queue_retirement::OriginalCarrierQueue::new(&self.state, &self.queue) }",
        "QueueLaneRetirementCut::belongs_to": "{ core::ptr::eq(self.observer.queue, queue) }",
        "QueueLaneRetirementCut::durability_faulted": "{ self.observer.durability_faulted() }",
        "QueueLaneRetirementObserver::durability_faulted": "{ self.queue.transaction_selection_durability_faulted() }",
        "QueueLaneRetirementCut::lane_pending_work_release": "{ self.observer.queue.lane_pending_work_release_locked(&self.reservations, lane_id, dataspace_id, lane_incarnation,) }",
        "PreparedCarrierGeometry::has_queue_custody": "{ !self.requires_queue_custody() || queue.is_some_and(|queue| queue.authenticates(target, self, header)) }",
        "CarrierQueueRetirement": "{ state_owner: NativeLaneStateOwner, header: BlockHeader, routes: Vec<(LaneId, DataSpaceId, Hash)>, _cut: QueueLaneRetirementCut<'queue>, }",
        "CarrierFences": "{ _state: StateFences<'target>, _queue: Option<CarrierQueueRetirement<'target>>, _kura: KuraPublicationLease<'target>, }",
    }.items():
        if symbol in items and items[symbol].partition("{")[2] != _code(body)[1:]:
            errors.append(f"Native preparation retained Queue {symbol} changes exact executable relation")
    ordered("AcquiredCarrierParticipants", "world:", "runtime:", "transactions:", "block_hashes:", "_fences:")
    ordered("CarrierFences::release_for_completion", "write.release_deferred()", "lifecycle.release_deferred()",
            "queue.map(CarrierQueueRetirement::release_deferred)", "kura.release_deferred()", "CompletionFences {")
    ordered("StateFences::try_acquire", "lock.try_lock_or_wait()",
            'acquire("state_commit_lock", &target.state_commit_lock)',
            'acquire("lane_lifecycle_lock", &target.lane_lifecycle_lock)',
            'acquire("state_write_lock", &target.state_write_lock)', "Ok(Self {")
    ordered("CarrierQueueRetirement::try_new", "if !source.belongs_to(target)",
            "return Err(CarrierQueueRetirementError::ForeignState)",
            "if !source.owns_cut(&cut)", "return Err(CarrierQueueRetirementError::ForeignQueue)",
            "geometry.for_each_retirement_route(", "for &(lane, dataspace, incarnation) in &routes",
            "cut.lane_pending_work_release(lane, dataspace, incarnation)",
            "return Err(CarrierQueueRetirementError::Pending {", "Ok(Self {")
    ordered("Queue::lane_pending_work_release_locked", "if hash_is_zero(lane_incarnation)",
            "let scope = (lane_id, dataspace_id, lane_incarnation)",
            "self.lane_retirement_releases.lock().entry(scope).or_default().observe()",
            "self.transaction_selection_durability_faulted()", "Self::lane_retirement_reservation_snapshot(",
            "self.lane_has_pending_route_work(", "Ok(Some(wait))", "Ok(None)")
    ordered("QueueLaneRetirementObserver::lane_pending_work_release",
            "self.queue.push_remove_lock.lock()", "self.queue.lane_reservations.lock()",
            "self.queue.lane_pending_work_release_locked(")
    for symbol in ("CarrierQueueRetirement::try_new", "CarrierQueueRetirement::authenticates",
                   "CarrierQueueRetirement::ensure_available", "OriginalCarrierQueue::try_observe",
                   "QueueLaneRetirementCut::lane_pending_work_release", "StateFences::try_acquire",
                   "try_prepare_physical"):
        body = items.get(symbol, "")
        for forbidden in (".await", ".lock()", "mem::forget", "ManuallyDrop", "block_on(",
                          "Queue::new", "State::new"):
            if _code(forbidden) in body:
                errors.append(f"Native preparation retained Queue {symbol} blocks or replaces its executable relation: {forbidden}")

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
            "let mutation = match self.queue.push_remove_lock.try_lock_or_wait()",
            "let reservations = match self.queue.lane_reservations.try_lock_or_wait()",
            "Ok(QueueLaneRetirementCut { reservations, _mutation: mutation, observer: self, })")
    require("QueueRetirementBusy", "struct QueueRetirementBusy { pub(crate) field: &'static str, pub(crate) wait: concread::release::ReleaseWait, }")
    ordered("QueueLaneRetirementCut",
            "reservations: PublicationGuard<'queue, LaneQueueReservationStore>",
            "_mutation: PublicationGuard<'queue>", "observer: QueueLaneRetirementObserver<'queue>")
    ordered("QueueLaneRetirementCut::lane_has_pending_work",
            "hash_is_zero(lane_incarnation) || queue.transaction_selection_durability_faulted()",
            "Queue::lane_retirement_reservation_snapshot(&self.reservations, lane_id, dataspace_id, lane_incarnation,)",
            "queue.lane_has_pending_route_work(&owned, lane_id, dataspace_id)")
    cut_probe = items.get("QueueLaneRetirementObserver::try_into_cut", "")
    # Both failed probes return their original event and actual released owners.
    # Notifications remain retained through the enclosing State/Kura fences.
    require("QueueLaneRetirementObserver::try_into_cut",
            "QueueRetirementCleanup { released: [None, None, Some(self.release_deferred())], }",
            "let mutation = mutation.release_deferred(); let transition = self.release_deferred();",
            "QueueRetirementCleanup { released: [None, Some(mutation), Some(transition)], }")
    # String masking remains mandatory for ordinary executable relations.
    # For the failed-lock diagnostic identity only, find the actual producer
    # expression in offset-preserving masked code, then read that literal from
    # the original item. A comment or unrelated same-label expression cannot
    # satisfy the failed mutex's exact label relation.
    raw_cut_probe = raw_items.get("QueueLaneRetirementObserver::try_into_cut", "")
    masked_cut_probe = _mask_rust_comments(raw_cut_probe)
    for field in ("push_remove_lock", "lane_reservations"):
        expression = (
            rf"match\s+self\s*\.\s*queue\s*\.\s*{field}\s*\.\s*try_lock_or_wait\s*\(\s*\)"
            r"\s*\{\s*Ok\(guard\)\s*=>\s*guard,\s*Err\(wait\)\s*=>\s*\{[^{}]*?"
            r"return\s+Err\(\(\s*QueueRetirementBusy\s*\{\s*field\s*:"
            r"(?P<label>\s*),\s*wait\s*,?\s*\}"
        )
        matches = list(re.finditer(expression, masked_cut_probe, re.S))
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
    ordered("AcquiredKuraPublicationFences", "sidecar: Option<PublicationGuard<'kura>>",
            "geometry: Option<PublicationGuard<'kura>>", "canonical: Option<PublicationGuard<'kura>>",
            "prune: Option<PublicationGuard<'kura>>")
    ordered("Kura::try_publication_lease",
            'acquire("prune_lock", &self.prune_lock)',
            "self.ensure_prune_recovery_not_required()",
            'acquire("canonical_chain_lock", &self.canonical_chain_lock)',
            "self.try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards(&mut fences)?;",
            'acquire("lane_geometry_lock", &self.lane_geometry_lock)',
            'acquire("sidecar_lock", &self.sidecar_lock)', "Ok(KuraPublicationLease {")
    ordered("KuraPublicationLease::begin_raw_geometry_attempt",
            "kura.durable_mutation_authorized()?;", "kura.require_raw_geometry_canonical_recovery_complete()?;",
            "let claim = kura.raw_geometry_claim.claim()?;",
            "kura.validate_certified_lane_drain_frontier_under_publication_lease(",
            "kura.read_lane_geometry_journal_structure()?",
            "retained_journal::RetainedGeometryJournal::capture(", "if observed != journal", "Ok(RawGeometryAttempt {")
    ordered("RawGeometryAttempt::persist_target", "self.pending_phase = Some(phase);",
            ".persist(kura, phase)?;", "self.pending_phase = None;")
    ordered("RawGeometryAttempt::resume_under", "self.authenticate(lease)?;",
            "self.maintenance.flush(kura)?;", "kura.finish_pending_lane_geometry_gc_with_custody(",
            "self.plan = Some(self.select_plan(kura)?);",
            "kura.reconcile_lane_geometry_history_to_count_with_custody(",
            "if self.target.is_none()", "let pending = lease.pending_canonical_bytes();",
            "kura.ensure_lane_retirement_admissible_locked(pending, &retiring, &certified)?;",
            "self.target = Some(self.prepare_target(kura, index)?);",
            "self.persist_target(kura, LaneGeometryPhase::Intent)?;", "self.intent_complete = true;",
            "kura.apply_geometry_operations_forward_with_progress(", "self.operation_cursor += 1;",
            "self.persist_target(kura, LaneGeometryPhase::FilesApplied)?;",
            "kura.ensure_authoritative_lane_markers_with_receipts(",
            "self.updated_entries.take()", "*kura.lane_storage_entries.lock() = entries;")
    ordered("RawGeometryAttempt::publish_catalog_under", "self.authenticate(lease)?;",
            'self\n            .catalog_baseline\n            .is_some_and(|original| original != configured_baseline)',
            "self.catalog_baseline = Some(configured_baseline);",
            "self.phase = RawGeometryPhase::PublishingCatalog;",
            "self.persist_target(kura, LaneGeometryPhase::CatalogPublished)?;",
            "self.phase = RawGeometryPhase::CatalogPublished;", "self.claim.finish();")
    ordered("RawGeometryAttempt::reauthenticate_catalog_under", "!self.kura.matches(kura)",
            "kura.raw_geometry_claim.ensure_unclaimed()?;", "target.reauthenticate_completed(",
            "let installed = kura.lane_storage_entries.lock();", "installed.len() != self.updated_bindings.len()")
    ordered("RawGeometryAttempt::rollback_under", "self.authenticate(lease)?;",
            "self.maintenance.pending.is_some()", "kura.apply_geometry_operations_rollback(",
            "self.persist_target(kura, LaneGeometryPhase::RolledBack)?;",
            "kura.ensure_authoritative_lane_markers_with_receipts(",
            "self.previous_entries.take()", "*kura.lane_storage_entries.lock() = entries;",
            "self.phase = RawGeometryPhase::RolledBack;", "self.claim.finish();")
    for owner in ("KuraPublicationLease::begin_raw_geometry_attempt", "RawGeometryAttempt::authenticate",
                  "RawGeometryAttempt::resume_under", "RawGeometryAttempt::publish_catalog_under",
                  "RawGeometryAttempt::reauthenticate_catalog_under", "RawGeometryAttempt::rollback_under"):
        held = items.get(owner, "")
        for forbidden in (".prune_lock.lock(", ".canonical_chain_lock.lock(",
                          ".lane_geometry_lock.lock(", ".sidecar_lock.lock(",
                          ".try_publication_lease(", ".finish_pending_lane_geometry_gc_locked(",
                          ".pending_canonical_capacity_bytes_under_prune_and_canonical_guards(",
                          ".try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards("):
            if forbidden in held:
                errors.append(f"Native preparation retained geometry {owner} reenters prelude or locking owner: {forbidden}")
    resumed = items.get("RawGeometryAttempt::resume_under", "")
    if resumed and resumed.count(_code("kura.ensure_lane_retirement_admissible_locked(pending, &retiring, &certified)?;")) != 1:
        errors.append("Native preparation retained geometry requires one shared new/retained retirement admission")
    for owner in ("RawGeometryAttempt::resume_under", "RawGeometryAttempt::publish_catalog_under",
                  "RawGeometryAttempt::rollback_under"):
        body = items.get(owner, "")
        for forbidden in ("begin_raw_geometry_attempt(", "RetainedGeometryJournal::capture(",
                          "read_lane_geometry_journal(", "read_lane_geometry_journal_structure("):
            if forbidden in body:
                errors.append(f"Native preparation retained geometry {owner} reconstructs original custody: {forbidden}")

    ordered("publish_execution_witness",
            "try_publication_lease()", "reauthenticate_checkpoint(", "drop(lease)",
            "stage_kagemusha_finality_sidecar(", "promote_kagemusha_finality_sidecar(")
    ordered("KuraPublicationLease::reauthenticate_execution_witness",
            "decode_kagemusha_finality_sidecar(&path)", "Kura::validate_kagemusha_finality_sidecar(&sidecar, finality)",
            "regular_sidecar_metadata(&path, &directory)", "Kura::stable_sidecar_metadata_unchanged(&read.metadata, current)", "Ok(())")
    ordered("publish_archives",
            "try_publication_lease()", "reauthenticate_checkpoint(",
            "provider.publish_under_publication_lease(&lease, receipt)",
            "reputation.publish_under_publication_lease(&lease, receipt)", "drop(lease)")
    archive_publication = items.get("publish_archives", "")
    if archive_publication and archive_publication.count(_code("drop(lease)")) != 1:
        errors.append("Native preparation publish_archives loses its single final lease-release executable relation")
    ordered("SourceAuthenticatedCarrier::try_new", "owner.kura.reauthenticate_checkpoint(",
            "original.journals.source_prefix.authenticate_durable_carrier(",
            "original.journals.provider_capture.as_ref()",
            "capture.reauthenticate_under_publication_lease(&owner.kura, original.checkpoint.finality_receipt(),)",
            ".map_err(CarrierPhysicalPreparationError::Provider)?;",
            "original.journals.reputation_capture.as_ref()",
            "capture.reauthenticate_under_publication_lease(&owner.kura, original.checkpoint.finality_receipt(),)",
            ".map_err(CarrierPhysicalPreparationError::Reputation)?;")
    ordered("try_prepare_physical",
            "admit(&original, target)", "target.matches_kura_instance(&original.journals.kura)",
            "if !original.journals.geometry.matches_publication_target(target, original.block().header())",
            "drop(installation);", "return Err((original, CarrierPhysicalPreparationError::ForeignTarget));",
            "if original.journals.geometry.requires_queue_custody()",
            "None => Some(CarrierQueueRetirementError::Missing)",
            "Some(source) if !source.belongs_to(target)",
            "Some(CarrierQueueRetirementError::ForeignState)",
            "return Err((original, CarrierPhysicalPreparationError::Queue(error)))",
            "target.kura.try_publication_lease()", "SourceAuthenticatedCarrier::try_new(original, kura)",
            "original.publish_execution_witness()", "original.publish_archives()", "target.kura.try_publication_lease()",
            "SourceAuthenticatedCarrier::try_new(original, kura)",
            "reauthenticate_execution_witness(authenticated.decision.finality.artifact())",
            "let queue_observer = if authenticated.decision.journals.geometry.requires_queue_custody()",
            "source.try_observe()", "StateFences::try_acquire(target)",
            "observer.try_into_cut()", "CarrierQueueRetirement::try_new(target, &authenticated.decision.journals.geometry, authenticated.decision.block().header(), source, cut,)",
            "let fences = CarrierFences { _state: state, _queue: queue, _kura: kura, }",
            "journals.try_map_components(")
    physical = items.get("try_prepare_physical", "")
    for operation in ("source.try_observe()", "StateFences::try_acquire(target)", "observer.try_into_cut()", "journals.try_map_components("):
        if physical and physical.count(_code(operation)) != 1:
            errors.append(f"Native preparation Queue acquisition repeats or omits executable relation {operation}")
    ordered("CarrierFences::release_for_completion", "write.release_deferred()", "lifecycle.release_deferred()",
            "queue.map(CarrierQueueRetirement::release_deferred)", "kura.release_deferred()", "CompletionFences {")
    ordered("CarrierQueueRetirement::try_new", "!source.belongs_to(target)",
            "!geometry.matches_publication_target(target, header)", "return Err(CarrierQueueRetirementError::ForeignState)",
            "!source.owns_cut(&cut)", "return Err(CarrierQueueRetirementError::ForeignQueue)",
            "geometry.for_each_retirement_route(", "cut.lane_pending_work_release(",
            "Ok(Some(wait)) =>", "return Err(CarrierQueueRetirementError::Pending {", "Ok(Self {")
    ordered("CarrierQueueRetirement::authenticates", "self.ensure_available().is_err()",
            "!self.state_owner.matches_state(target)", "self.header != header",
            "!geometry.matches_publication_target(target, header)", "return false;",
            "geometry.for_each_retirement_route(", "self.routes.get(index) != Some(&(lane, dataspace, incarnation))",
            "result.is_ok() && index == self.routes.len()")
    ordered("PreparedCarrierGeometry::complete_under",
            "if !self.matches_publication_target(target, header)", "return Err(LaneLifecycleError::Storage(",
            "if !self.has_queue_custody(target, header, queue)", "return Err(LaneLifecycleError::Storage(",
            "self.resume_under(backend, lease)?;", "raw.publish_catalog_under(lease, None)",
            "raw.reauthenticate_catalog_under(lease)", "Ok(CompletedCarrierGeometry {")
    ordered("PhysicallyPreparedCarrier::try_complete_geometry",
            "if !journals.geometry.requires_storage_transition()", "return Ok(false);",
            "self.target.tiered_backend.try_lock_or_wait()",
            "journals.geometry.prepare_under(&backend, &journals.components._fences._kura)?;",
            "journals.geometry.complete_under(self.target, journals.effects.header, &mut backend, &journals.components._fences._kura, journals.components._fences._queue.as_ref(),)?",
            "Ok(completed.updated_da_mapping().is_some())")
    for symbol in ("PreparedCarrierGeometry::complete_under", "PhysicallyPreparedCarrier::try_complete_geometry"):
        item = items.get(symbol, "")
        for forbidden in (".try_publication_lease(", ".lock(", ".await", "begin_raw_geometry_attempt(",
                          "resume_lane_geometry_publication(", "finish_lane_geometry_publication("):
            if _code(forbidden) in item:
                errors.append(f"Native preparation geometry completion {symbol} reacquires or reconstructs ownership: {forbidden}")
    ordered("PhysicallyPreparedCarrier::publish",
            "queue.ensure_available().err()", "Some(CarrierPublicationError::QueueRetirement(error))",
            "journals.effects.replay_prevalidation",
            "journals.geometry.matches_publication_target(self.target, journals.effects.header)",
            "journals.geometry.has_pending_lifecycle() != journals.effects.lifecycle.is_some()",
            "!journals.geometry.has_queue_custody(\n            self.target,\n            journals.effects.header,\n            journals.components._fences._queue.as_ref(),\n        )", "Some(CarrierPublicationError::QueueRetirementRequired)",
            "journals.native_amx_manifest.entries().is_empty()", "return Err((self.abort(), error))",
            "let update_da_mapping = match self.try_complete_geometry()",
            "return Err((self.abort(), CarrierPublicationError::GeometryStorage(error)))",
            "self.decision.journals.components._fences._queue.as_ref().and_then(|queue| queue.ensure_available().err())",
            "return Err((self.abort(), CarrierPublicationError::QueueRetirement(error)))",
            "target.begin_state_view_write()", "transactions.publish()", "runtime.publish()",
            "if update_da_mapping { target.da_shard_cursors.write().sync_mapping(&effects.nexus.lane_config); }",
            "let lifecycle_post_publication = effects.lifecycle.take().map(|effects| effects.publish(target, &generation, true));",
            "world.publish()", "world_effects.publish(target)",
            "let da_post_publication = effects.da_commitments.take().map(|effects| effects.publish(target, &generation, true));",
            "hash_retirement = block_hashes.publish()", "target.update_latest_block_header_cache(effects.header)",
            "drop(generation)", "if let Some(post) = da_post_publication { post.publish(target); }",
            "if let Some(post) = lifecycle_post_publication { post.publish(target); }",
            "fences.release_for_completion()", "effects.publish_observability(target)",
            "tiered_snapshot.publish(target, false)", "drop(commit)", "drop(hash_retirement)", "Ok(PublishedCarrier {")
    terminal = items.get("PhysicallyPreparedCarrier::publish", "")
    if terminal:
        if terminal.count(_code("queue.ensure_available().err()")) != 2:
            errors.append("Native preparation terminal lifecycle must recheck sticky Queue fault before storage and visibility")
        for operation in ("target.begin_state_view_write()", "transactions.publish()", "runtime.publish()",
                          "world.publish()", "block_hashes.publish()", "drop(generation)",
                          "self.try_complete_geometry()", "effects.lifecycle.take()",
                          "effects.da_commitments.take()", "sync_mapping(&effects.nexus.lane_config)"):
            if terminal.count(_code(operation)) != 1:
                errors.append(f"Native preparation terminal lifecycle repeats or omits {operation}")
        visible_tail = terminal.split(_code("transactions.publish()"), 1)[-1]
        if "?" in visible_tail or "returnErr(" in visible_tail:
            errors.append("Native preparation terminal publication has a fallible post-write retry")
        for forbidden in (".block(", "commit_inner(", "CheckedCarrierApplications", "validate_and_prepare"):
            if forbidden in terminal:
                errors.append(f"Native preparation terminal publication reconstructs authority: {forbidden}")

    ordered("CompletionFences", "_commit:", "_state:", "_queue:", "_kura:")
    ordered("ReleasedCarrierQueue", "_routes:", "_state_owner:", "_released:")
    ordered("PhysicallyPreparedCarrier::publish", "let membership_retirement;", "let AcquiredCarrierParticipants {",
            "membership_retirement = transactions.publish()", "fences.release_for_completion()",
            "drop(commit)", "drop(membership_retirement)")
    ordered("PhysicallyPreparedCarrier::publish", "let hash_retirement;", "let AcquiredCarrierParticipants {",
            "hash_retirement = block_hashes.publish()", "drop(generation)", "fences.release_for_completion()",
            "drop(commit)", "drop(hash_retirement)")
    ordered("PreparedBlockHashes", "prepared: concread::release::ReleaseGuard", "installation: Installation")
    ordered("PublishedBlockHashes", "_retirement: concread::release::ReleaseGuard", "_installation: Installation")
    ordered("DetachedBlockHashes::try_prepare_publication", "if self.reserved_tip.is_some()",
            "return Err((self, mv::PublicationPreparationError::Changed, cleanup))", "match self.observe_current(target)",
            "let installation = match admit(&self, target)", "match map.try_acquire_owned(work)",
            "target.released.poisoning_guard(acquired)", "acquired.try_map_preserving_release(|acquired| acquired.validate())",
            "writer.try_map_preserving_release(|writer| writer.try_prepare_commit())", "Ok(PreparedBlockHashes {")
    ordered("PreparedBlockHashes::abort", "prepared.release_deferred", "prepared.abort_retaining()", "writer.detach()", "AbortedBlockHashes {", "DetachedBlockHashes {")
    ordered("StateBlock::commit_inner", "let mut hash_refusal_cleanup = None;", "let Self {",
            "hash_refusal_cleanup = Some(cleanup);", "drop(hash_refusal_cleanup);")
    ordered("AcquiredCarrierParticipants::abort", "world.abort()", "runtime.abort()", "transactions.abort()", "block_hashes.abort()", "drop(fences.release_for_completion())", "drop((world_retirement, runtime_retirement, transactions_retirement, block_hashes_retirement,))")
    ordered("PreparedBlockHashes::publish", "prepared.publish()", "committed_height.store(height, Ordering::Release)",
            "published.release()", "PublishedBlockHashes {")
    for symbol in ("DetachedBlockHashes::try_prepare_publication", "PreparedBlockHashes::publish", "PreparedBlockHashes::abort"):
        item = items.get(symbol, "")
        for forbidden in (".lock()", ".write()", ".read()", ".prepare_commit()", "Arc::new(", "Vec::new(", ".to_vec()", "mem::forget", "ManuallyDrop"):
            if _code(forbidden) in item:
                errors.append(f"Native preparation shared history {symbol} reconstructs or blocks original custody: {forbidden}")

    ordered("BlockHashes::admit_successor", "checked_add(additional.bytes())",
            "if required > self.budget.limit_bytes()", "AllocationRefusal::ExceedsLimit", "self.admit(additional)")
    ordered("BlockHashes::try_next_block", "self.map().ok_or(BlockHashAdmissionError::ReadOnly)?",
            "let wait = map.observe_reader_release()", "self.try_view()", "view.predecessor().retain()", "drop(view)",
            "let wait = self.released.observe()", ".try_acquire_writer()", ".poisoning_guard(acquired)",
            ".try_map_preserving_release(|acquired|", ".try_insert_admitted_with_footprint(",
            "self.admit_successor(existing, additional)", "writer.release_with(|(writer, previous)| (writer.detach(), previous))",
            "if !predecessor.matches(&work.predecessor())", "Ok(BlockHashesBlock {")
    ordered("BlockHashes::try_new", "BlockHashMap::try_new_with_node_custody", "budget.try_reserve_bytes(demand.bytes())",
            "initial.into_iter().enumerate()", "try_insert_admitted_with_footprint", "owner.admit_successor(existing, additional)",
            "map\n                    .try_write_owned(work)", "writer.prepare_commit().publish().release()", "owner.committed_height.store(index + 1, Ordering::Release)")
    for symbol in ("BlockHashes::admit", "BlockHashes::admit_successor", "BlockHashes::try_next_block"):
        item = items.get(symbol, "")
        for forbidden in ("AllocationBudget::new(", "Untracked", "Vec::", ".collect(", ".to_vec()", ".try_remove_admitted("):
            if _code(forbidden) in item:
                errors.append(f"Native preparation prepaid history {symbol} bypasses its original demand owner: {forbidden}")

    ordered("State::acquire_canonical_runtime_block", "self.block_hashes.try_next_block(replacement)?",
            "self.world.block_and_revert()", "self.world.block()", "is_stable_state_view_generation(",
            "drop(canonical_runtime)", "drop(world)", "drop(block_hashes)", "return Ok(AcquiredRuntimeBlock {")
    ordered("State::block_and_revert_with_pristine_stage", "self.acquire_canonical_runtime_block(true)?",
            "self.rewind_da_indexes_to_height(target_height)", "let mut state_block = StateBlock {")
    ordered("schedule_local_proposal", "proposal_state.reconcile(LocalProposalOwner::from(directive))",
            "proposal_state.history_admission_pending(owner, &queue.sumeragi_waker())", "lane_work.schedule_autonomous_lane_production(")
    ordered("refresh_merge_candidates", "if let Some((view, pending)) = &mut self.merge_history_wait",
            "if *view == active_view && !pending.is_ready(&wake)", "self.merge_history_wait = None",
            "let refresh_generation = self.state.state_view_generation()", "self.build_and_memoize_merge_execution_candidate(",
            "let Some(wait) = error.release_wait()", "self.merge_history_wait = Some((")
    ordered("HistoryAdmissionWait::new", "wait.wait_for_release()", "pending.is_ready(wake)", "wake.wake_by_ref()", "pending")

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
            "let prefix = ValidatedExecutionPrefix { sealed, authority, _inventory: inventory, witness, _fastpq_witness_context: state.fastpq_witness_context.take(), parliament_timed_ovn_casting_bindings: state.parliament_timed_ovn_casting_bindings.take(), };")
    ordered("PrefixPreparation::capture", "let authority = match native",
            "state.verify_execution_output_seal(block)?;",
            "state.verify_cached_ordinary_witness_content(&verified_inventory)?;",
            "let manifest =", "let commitment =", ".replace(output_capacity::ExecutionOutputPlanState::Captured)",
            "let inventory =", "let witness = state.exec_witness.take()", "let prefix =")
    ordered("prepare", "let (valid, state, context, native) = input.into_parts();",
            "PrefixPreparation::capture(state, &valid, native)?;", "prepare_deterministic_carrier_metadata(",
            "preparation.prepare_world_effects()?;", "prepare_carrier_publication_events(block.header())",
            "let PrefixPreparation { state, prefix: source_prefix, } = preparation;",
            "Ok(PreparedCarrier { valid, state, source_prefix, context, execution_prefix, native_amx_manifest,")
    admission_borrow = """let Self {
        valid, state, source_prefix, context, execution_prefix,
        native_amx_manifest, _world_effects, _publication_events,
    } = &self;"""
    admission_inputs = """let admission = match admit_journals(CarrierJournalInputs {
        valid, state, prefix: source_prefix, context, execution_prefix, native_amx_manifest,
        da_pins: _world_effects.admission_pins(), publication_events: _publication_events,
        provider: provider_capture.as_ref(), reputation: reputation_capture.as_ref(),
        retained_effects_layout: std::alloc::Layout::new::<RetainedCarrierEffects>(),
    }) {"""
    ordered("PreparedCarrier::prepare_journals", admission_borrow, admission_inputs,
            "let mut provider_capture = provider_capture;", "let Self {",
            "PreparedTieredSnapshot::prepare(", "state.prepare_carrier_geometry()?;",
            "owner.capture_original(state.as_ref())", "world.try_detach_journals(",
            "let journals = PreparedCarrierJournals {", "Box::new(RetainedCarrierEffects {",
            "StagedCarrierCapture {", "carrier.try_prepare_archives()",
            "return Err(CarrierJournalPreparationError::ArchivePreparation {", "carrier: Box::new(carrier)",
            "Ok(carrier.into_journals())")
    journal_prepare = items.get("PreparedCarrier::prepare_journals", "")
    admission_start = journal_prepare.find(_code(admission_inputs))
    for projection in ("PreparedTieredSnapshot::prepare(", "state.prepare_carrier_geometry(",
                       "owner.capture_original(", "world.try_detach_journals(",
                       "Box::new(RetainedCarrierEffects {"):
        if _code(projection) in journal_prepare[:max(0, admission_start)]:
            errors.append(f"Native preparation journal admission follows projection: {projection}")
    ordered("StagedCarrierCapture::try_prepare_archives", "self.capture_refusal", "provider.try_prepare()",
            "reputation.try_prepare()", "Ok(())")
    ordered("StagedCarrierCapture::into_journals", "self.provider.take()", "owner.into_prepared()",
            "self.reputation.take()", "owner.into_prepared()")
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
