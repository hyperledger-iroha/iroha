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
# The old direct Native manifest, receipt/latest and prune allowance obligations
# now belong to the following owners; ordinary block bytes remain separate.
PREPARATION_OWNER_BINDINGS = (
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
        "verify_native_execution_metadata(block, executions)", "block.validate_output_merkle_cache()",
        "advertised_policy.as_ref().ok_or_else(", ".validate()",
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
        "executions.iter().zip(routes)", "source.payload.input.routing_plan()",
        "plan.coordinator_route() != *route", ".find(|(_, slot)| slot.route == *route)",
        "source.decisions.get(slot_index)", "let commitment = &execution.settlement_commitment",
        "iroha_data_model::nexus::compute_settlement_hash(commitment)", "!= execution.settlement_hash",
        "commitment.receipts.is_empty() && commitment.nexus_fee_receipts.is_empty() && commitment.native_amx_receipts.is_empty()",
        "commitment.total_local_amount.is_zero()", "commitment.total_xor_due.is_zero()",
        "commitment.total_xor_after_haircut.is_zero()", "commitment.total_xor_variance.is_zero()",
        "commitment.swap_metadata.is_some()", "source.payload.descriptor.canonical_hash()",
        "LaneRelayEnvelope::new(", "commitment.clone()", "decision.manifest.byte_len",
        "with_lane_block_descriptor_hash(Some(descriptor_hash))",
        "entry.dsid == commitment.dataspace_id", "entry.policy.manifest_root",
        "envelope.lane_finality_statement()", "statements.sort_unstable_by_key(",
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
        "state.native_lane_stage.is_some()", "context.native_lane_decisions.is_some()",
        "state.staged_merge_entry.is_some()", "!state.merge_carrier_entrypoints.is_empty()",
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
        "input.into_parts()", "PrefixPreparation::capture(state, &valid)?",
        "prepare_deterministic_carrier_metadata", "preparation.prepare_world_effects()?",
        "prepare_carrier_publication_events(block.header())", "PreparedTieredSnapshot::prepare(",
        "prefix: source_prefix", "Ok(PreparedCarrier {", "Err(error) => Err((Box::new(valid.into()), error))",
    )),
    (JOURNALS, "method", "PreparedCarrier::prepare_journals", (
        "admit_journals: impl FnOnce(CarrierJournalInputs<'_, 'state>)",
        "let admission;", "source_prefix,", "admit_journals(CarrierJournalInputs {",
        "state: &state", "prefix: &source_prefix", "state.prepare_carrier_geometry()?",
        "world.try_detach_journals", "transactions.prepare_commit()?.detach()", "admission, })",
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
NATIVE_PREPARATION_SOURCE_RELATIVES = tuple(Path(p) for p in (
    APPLY, BLOCK, PREPARED, PREFIX, JOURNALS, OUTPUT, SEAL, TAIL, NATIVE_METADATA, NATIVE_STAGE,
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
    for path, kind, symbol, tokens in bindings:
        matches = [r for r in rows if isinstance(r, dict)
                   and (r.get("path"), r.get("kind"), r.get("symbol")) == (path, kind, symbol)]
        if len(matches) != 1:
            errors.append(f"Native preparation ledger owner {symbol} must occur exactly once")
        elif tuple(matches[0].get("required_tokens", ())) != tokens:
            errors.append(f"Native preparation reviewed tokens changed for {symbol}")
        item = rust_binding_item(root, path, kind, symbol, "Native preparation", errors)
        if item is not None:
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

    require("V2ApplyService::validate_candidate",
            "let prepared = ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block(body.clone(), &topology, &self.genesis_account, &TimeSource::new_system(), self.block_cadence, crate::block::valid::SumeragiV2ValidationContext::from_height_context(context), self.state.as_ref(), &mut voting_block,)",
            "self.kura.validate_native_amx_participant_application_evidence_byte_budget(prepared.native_amx_manifest(), None,).map_err(Self::classify_native_amx_evidence_byte_budget_error)?;")
    require("ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block",
            "let context_matches = context.id() == validation_context.context_id && context.height == block.header().height().get() && context.network_id == *state.network_id_ref() && topology.as_ref().iter().eq(context.roster.iter().map(|entry| &entry.validator));",
            "let (valid, state) = Self::validate_sumeragi_v2_candidate_keep_voting_block(block, topology, genesis_account, time_source, block_cadence, validation_context, state, voting_block,).unpack(|_| {})?;",
            "crate::state::PreparedCarrier::prepare(ValidatedCarrierPreparationInput { valid, state, context, })")
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
            "if commitment.receipts.is_empty() && commitment.nexus_fee_receipts.is_empty() && commitment.native_amx_receipts.is_empty() { if !commitment.total_local_amount.is_zero() || !commitment.total_xor_due.is_zero() || !commitment.total_xor_after_haircut.is_zero() || !commitment.total_xor_variance.is_zero() || commitment.swap_metadata.is_some() { return Err(",
            "LaneRelayEnvelope::new(block.header(), block.header().da_commitments_hash(), commitment.clone(), decision.manifest.byte_len,)",
            "statements.sort_unstable_by_key(|statement| { (statement.lane_id, statement.dataspace_id, statement.lane_incarnation, statement.block_height,) });")
    ordered("ValidBlock::native_execution_finality_statements",
            "let commitment = &execution.settlement_commitment;", "!= execution.settlement_hash",
            "commitment.receipts.is_empty()", "continue;", "let descriptor_hash =",
            "LaneRelayEnvelope::new(", "with_lane_block_descriptor_hash(Some(descriptor_hash))",
            "envelope.manifest_root = policy.entries.iter().find(|entry| entry.dsid == commitment.dataspace_id).map(|entry| entry.policy.manifest_root);",
            "envelope.lane_finality_statement()", "statements.push(statement);")
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
    require("PreparedCarrier::prepare", "execution_prefix::prepare(input)")
    require("PrefixPreparation::capture",
            "let block = valid.as_ref();",
            "let witness = state.exec_witness.as_ref().ok_or(",
            "let manifest = exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, None,)?;",
            "let commitment = exec::execution_commitment_from_validated_block(witness, &manifest, &lanes, block).map_err(str::to_owned)?;",
            "let inventory = state.fastpq_source_inventory.take().ok_or(",
            "let witness = state.exec_witness.take().ok_or(",
            "let prefix = ValidatedExecutionPrefix { sealed, inventory, witness, fastpq_witness_context: state.fastpq_witness_context.take(), parliament_timed_ovn_casting_bindings: state.parliament_timed_ovn_casting_bindings.take(), };")
    ordered("PrefixPreparation::capture", "if state.native_lane_stage.is_some()",
            "state.verify_execution_output_seal(block)?;",
            "state.verify_cached_ordinary_witness_content(&verified_inventory)?;",
            "let manifest =", "let commitment =", ".replace(output_capacity::ExecutionOutputPlanState::Captured)",
            "let inventory =", "let witness = state.exec_witness.take()", "let prefix =")
    ordered("prepare", "let (valid, state, context) = input.into_parts();",
            "PrefixPreparation::capture(state, &valid)?;", "prepare_deterministic_carrier_metadata(",
            "preparation.prepare_world_effects()?;", "prepare_carrier_publication_events(block.header())",
            "PreparedTieredSnapshot::prepare(", "let PrefixPreparation { state, prefix: source_prefix, } = preparation;",
            "Ok(PreparedCarrier { valid, state, source_prefix, context, execution_prefix, native_amx_manifest,")
    ordered("PreparedCarrier::prepare_journals", "let admission;", "let Self {",
            "admit_journals(CarrierJournalInputs { state: &state, prefix: &source_prefix, })",
            "state.prepare_carrier_geometry()?;", "world.try_detach_journals(", "Ok(PreparedCarrierJournals {")
    require("StateBlock::seal_execution_outputs", "Ok(SealedExecutionOutputs { sources, world_delta,")
    if "prepare" in items:
        body = items["prepare"]
        capture = body.find(_code("PrefixPreparation::capture(state, &valid)?;"))
        tail = body.find(_code("prepare_deterministic_carrier_metadata("))
        if capture < 0 or tail < capture:
            errors.append("Native preparation stages metadata before prefix capture")

    # Type privacy and consuming field layout are part of the owner boundary;
    # no detached marker, caller-supplied scalar or mutable State accessor suffices.
    for symbol in ("ValidatedExecutionPrefix", "PrefixPreparation", "SealedExecutionOutputs"):
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
