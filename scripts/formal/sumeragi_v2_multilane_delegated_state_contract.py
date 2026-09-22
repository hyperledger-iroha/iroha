"""Bind live State forwarders and their exact shared execution/frontier owners.

Invoked by the multilane release checker, with no environment configuration.
Retains the prior obligations after extraction and checks authority forwarding.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

from sumeragi_v2_multilane_geometry_evidence_contract import _code


STATE = "crates/iroha_core/src/state.rs"
MODEL = "SumeragiV2AutonomousReservationCarrier"
DELEGATED_STATE_BINDINGS = (
    ("validate_merge_execution_batch", (
        "self.validate_merge_execution_batch_with_replay(",
        "active_lanes", "batch", "validation_authority", "None",
    )),
    ("validate_merge_execution_batch_with_replay", (
        "reservation_keys", "routing_plans", "merge_execution_batch_commitments_match",
        "entrypoint_hash", "LaneIncarnationMismatch",
        "validate_merge_execution_predecessor_against_frontier", "if !validate_live_authority",
        "validate_historical_native_amx_source_bundle",
        "HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes",
        "merge_execution_canonical_order_key", "previous_order", "strict canonical total order",
        "replay: Option<&crate::block::VerifiedReplayProposal>",
        "MergeExecutionValidationAuthority::Live(mode)",
        "MergeExecutionValidationAuthority::Historical(authority)",
        "token.native_amx_authority(&authority)", "validate_native_amx_receipt_against_plan",
    )),
    ("canonical_merged_lane_frontier_from_world", (
        "canonical_merged_lane_frontier_with_anchor_from_world",
        "world", "lane_id", "dataspace_id", "lane_incarnation",
        ".map(|(height, descriptor_hash, _)| (height, descriptor_hash))",
    )),
    ("canonical_merged_lane_frontier_with_anchor_from_world", (
        "merge_lane_frontier_marker_key", "decode_exact_merge_lane_frontier_marker",
        "lane_block_height", "lane_block_descriptor_hash", "applied_global_height",
    )),
    ("validate_merge_execution_predecessor_against_frontier", (
        "validate_lane_frontier_successor", "descriptor.lane_id",
        "descriptor.dataspace_id", "descriptor.lane_incarnation",
        "descriptor.lane_block_height", "descriptor.previous_lane_block_height",
        "descriptor.previous_lane_block_descriptor_hash",
    )),
    ("validate_lane_frontier_successor", (
        "canonical_merged_lane_frontier_from_world", "previous_height", "previous_hash",
        "checked_add(1)", "NonContiguousLaneSnapshot",
        "actual_predecessor != expected_predecessor", "lane_block_height != expected_height",
    )),
    ("preexecute_merge_execution_sources_into", (
        "Self::preexecute_merge_execution_sources_into_with_replay(state_block, sources, None)",
    )),
    ("preexecute_merge_execution_sources_into_with_replay", (
        "decode_autonomous_lane_merge_bundle", "validate_lane_block_execution_input_with_routing_context",
        "stage_merge_carrier_entrypoints", "drain_merge_lane_settlement_commitment",
        "seen_entrypoints", "seen_reservations", "replay: Option<&crate::block::VerifiedReplayProposal>",
        "token.native_amx_authority(&*state_block)", "validate_native_amx_receipt_against_plan",
    )),
    ("stage_certified_merge_entry", (
        "self.stage_certified_merge_entry_with_replay(entry, frozen_mode, None)",
    )),
    ("stage_certified_merge_entry_with_replay", (
        "merge_execution_already_applied", "base_state_hash", "preexecute_merge_execution_sources_into",
        "validate_merge_execution_commit_surface", "MergeExecutionCommitSurface::Pristine",
        "application_write_set_root", "stage_merge_execution_markers", "expected_post_state_hash",
        "let external_event_count = self.world.external_event_buf.len()",
        "let external_event_bytes = Self::merge_execution_external_event_bytes(&self.world)",
        "validated_publication_event_bytes: None", "ensure_pristine_execution_control_stage",
        "replay: Option<&crate::block::VerifiedReplayProposal>",
        "validate_merge_stage(&self._curr_block, &*self, entry)",
        "validate_certified_merge_entry_for_global_order_with_replay",
    )),
    ("stage_certified_merge_reference_for_verified_replay", (
        "replay: &crate::block::VerifiedReplayProposal", ".merge_entry(reference)",
        "self.stage_certified_merge_entry_with_replay(entry, frozen_mode, Some(replay))",
    )),
)
DELEGATED_STATE_BINDINGS += (('select_merge_execution_candidate_for_consensus',
  ('canonical_merged_lane_frontier_from_world',
   'validate_merge_execution_predecessor_against_frontier',
   'durable_autonomous_merge_source_for_lane_slot',
   'MergeExecutionSource::from_durable',
   'build_merge_execution_batch_from_source_prefix',
   '!consensus.is_current(self)',
   'epoch_id != admission.expected_epoch()',
   'consensus.committed_height.checked_add(1)?',
   'application_block_header.prev_block_hash() != consensus.latest_block_hash',
   'nexus.lane_catalog.lanes().len() > MAX_ACTIVE_EXECUTION_LANES',
   'lane_authority::authenticated_committee_for_descriptor(',
   'if descriptor.validator_set != authoritative',
   '.merge_active_lane_authority_snapshot(application_block_header.height().get())',
   'let candidate_template = crate::merge::MergeLedgerCandidate {',
   'lane_authority_catalog,',
   'Self::select_merge_execution_candidate_prefix(',
   'MAX_MERGE_LEDGER_ENTRY_BYTES.saturating_sub(MAX_MERGE_QC_BYTES)',
   'sources[..prefix_len].to_vec()',
   'consensus.is_current(self).then_some(selected).flatten()',
   'select_merge_execution_source_budget(',
   'gas_limit_from_parameters(world.parameters())')),
 ('select_merge_execution_source_budget',
  ('source.origin_proposal.descriptor.proposal_height',
   'merge_execution_canonical_order_key(&source.certified.proposal)',
   'selected_entrypoints.checked_add(source.input.entrypoints.len())',
   'merge_execution_proposal_gas(&source.input.entrypoints)?',
   'next_entrypoints > MAX_MERGE_EXECUTION_ENTRYPOINTS',
   '!crate::gas::gas_components_fit_block_limit(gas_limit, [selected_gas, gas])',
   'selected_gas.checked_add(gas)',
   'sources.truncate(selected_count)',
   'Ok(sources)')),
 ('build_merge_execution_batch_from_source_prefix',
  ('base_state_height',
   'base_state_hash',
   'preexecute_merge_execution_sources',
   'application_write_set_root',
   'merge_execution_write_set_root',
   'expected_post_state_hash',
   'merge_execution_batch_hash',
   'total_entrypoints > MAX_MERGE_EXECUTION_ENTRYPOINTS',
   'merge_execution_canonical_order_key(&source.certified.proposal)',
   'validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)')),
 ('apply_without_execution_inner',
  ('prepare_deterministic_carrier_metadata(',
   'prepare_carrier_publication_events(block.as_ref().header())',
   'return (Vec::new(), Err(error))',
   'self.mint_canonical_carrier_commit_metadata_authorization(block)',
   '(events, carrier_authorization)')),
 ('prepare_carrier_publication_events',
  ('if header != self._curr_block',
   'return Err(MergeLedgerCommitError::ExecutionBatchInvalid(',
   'self.validate_merge_execution_external_event_publication_surface()?',
   'status: BlockStatus::Applied',
   'Ok(self.world.take_external_events())')))

# Original merge roots remain exact; only the separately verified pulse seal
# selects composed events/root for all three publication checks.
DELEGATED_STATE_BINDINGS += (('composed_external_events',
  ('self.beacon_composition.as_ref().map_or(',
   'self.external_event_bytes.as_deref(),\n                self.external_event_count',
   'seal.external_event_bytes.as_deref(),\n                    seal.external_event_count')),
 ('composed_write_set_root',
  ('self.beacon_composition',
   '.as_ref()',
   '.map_or(self.write_set_root, |seal| seal.write_set_root)')),
 ('apply_verified_merge_beacon_pulse',
  ('capability: crate::block::valid::VerifiedMergeBeaconPulse',
   'capability.into_parts()',
   'self._curr_block != header',
   'self.network_id != network_id',
   'self.start_of_block_effects_applied',
   'self.applied_npos_consensus_effects_hash.is_some()',
   'merge_beacon_parent_surface(&self.world) != parent_surface',
   'self.validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)?;',
   'authorization.beacon_composition.is_some()',
   'authorization.validated_publication_event_bytes.is_some()',
   'actual_events.as_deref() != authorization.external_event_bytes.as_deref()',
   '!Self::canonical_wsv_merge_commit_authorization_matches(',
   'original_root,\n            )',
   'self.apply_pristine_npos_consensus_effects(',
   'let external_event_bytes = (!self.world.external_event_buf.is_empty())',
   'publication_events.push(self.create_time_event(&header).into());',
   'let seal = CanonicalMergeBeaconCompositionAuthorization {',
   'effects_hash: HashOf::new(&effects)',
   'external_event_count: self.world.external_event_buf.len()',
   'publication_event_bytes: publication_events.encode()',
   '.beacon_composition = Some(seal)')),
 ('validate_staged_merge_execution_authorization',
  ('canonical_carrier_commit_metadata_authorization',
   'autonomous merge carrier metadata was authorized before finality',
   'MergeExecutionCommitSurface::PostBlockPreVote',
   '.get(..composed_event_count)',
   'autonomous_event_prefix.to_vec().encode()',
   'merge_execution_write_set_root_from_overlay_with_external_events',
   'autonomous_event_prefix_bytes.as_deref() != composed_event_bytes',
   'canonical_wsv_merge_commit_authorization_matches',
   'canonical WSV merge commit authorization drifted before block admission',
   'validated_publication_event_bytes = Some(publication_event_bytes)',
   'let (composed_event_bytes, composed_event_count) = authorization.composed_external_events();',
   'self.applied_npos_consensus_effects_hash != expected_effects_hash',
   'current_write_set_root != authorization.composed_write_set_root()',
   'seal.publication_event_bytes != publication_event_bytes')),
 ('mint_canonical_carrier_commit_metadata_authorization',
  ('finalized carrier metadata authorization was already minted',
   'reference.matches_entry(entry)',
   'canonical_wsv_merge_commit_authorization_matches',
   '!self\n            .block_hashes\n            .pending()\n            .iter()\n            .copied()\n            .eq([carrier_hash])',
   'has_exact_staged_block(carrier_storage_height, &self.merge_carrier_entrypoints)',
   'authorization.validated_publication_event_bytes.is_none()',
   'finalized autonomous carrier lacks a validated publication event surface',
   '!self.world.external_event_buf.is_empty()',
   'finalized autonomous carrier retained events after publication',
   'merge_execution_write_set_root_from_overlay_with_external_events',
   'authorization.composed_external_events().0',
   'authorization.write_set_root',
   'finalized carrier economic authorization is stale or mismatched',
   'CanonicalCarrierCommitMetadataAuthorization',
   'post_finality_write_set_root,',
   'previous_commit_topology',
   'autoscale_sample',
   'authorization.composed_write_set_root()')),
 ('commit_inner',
  ('MergeExecutionCommitSurface::FinalizedCarrier', 'merge_execution_commit_surface_result', 'let Some(authorization) = canonical_wsv_merge_commit_authorization.as_ref() else {', 'let Some(carrier_authorization) =\n                    canonical_carrier_commit_metadata_authorization.as_ref()\n                else {', 'merge_execution_write_set_root_from_overlay_with_external_events', 'authorization.composed_external_events().0', 'authorization.write_set_root', 'carrier_authorization.post_finality_write_set_root != current_write_set_root', 'certified merge entry has no exact durable carrier before state commit', 'transactions.try_prepare_publication()', 'tx_validate_result?;', 'transactions.publish_prepared()', 'authorization.composed_write_set_root()', 'let mut this = self;', 'this.fields.as_mut()')))

DELEGATED_STATE_BINDINGS += (("build_merge_execution_candidate_for_consensus", ()), ("select_merge_execution_candidate_prefix", ()))

# Exact existing diagnostic literals, separate from executable relations.
DELEGATED_STATE_DIAGNOSTIC_TOKENS = (
    "strict canonical total order",
    "autonomous merge carrier metadata was authorized before finality",
    "canonical WSV merge commit authorization drifted before block admission",
    "finalized carrier metadata authorization was already minted",
    "finalized autonomous carrier lacks a validated publication event surface",
    "finalized autonomous carrier retained events after publication",
    "finalized carrier economic authorization is stale or mismatched",
    "certified merge entry has no exact durable carrier before state commit",
)

DELEGATED_STATE_SOURCE_RELATIVES = (
    Path(STATE),
    Path("scripts/formal/sumeragi_v2_multilane_delegated_state_contract.py"),
    Path("pytests/scripts/sumeragi_v2_multilane_delegated_state_contract_test.py"),
)


# Shared wrappers and helpers use the same complete semantic inventory as the
# State merge contract; each validator retains its own operation-order checks.
from sumeragi_v2_multilane_state_merge_contract import STATE_MERGE_BINDINGS

_STATE_MERGE_TOKENS = {symbol: tokens for _, _, _, symbol, tokens in STATE_MERGE_BINDINGS}
DELEGATED_STATE_BINDINGS = tuple(
    (symbol, _STATE_MERGE_TOKENS.get(symbol, tokens))
    for symbol, tokens in DELEGATED_STATE_BINDINGS
)


def validate_delegated_state_contract(
    root: Path, models: Any, errors: list[str], rust_binding_item: Callable,
) -> None:
    """Require both ends of delegation and exact route/replay capability flow."""

    owners = [m for m in models if isinstance(m, dict) and m.get("module") == MODEL]
    bindings = owners[0].get("production_symbols", ()) if len(owners) == 1 else ()
    items = {}
    for symbol, tokens in DELEGATED_STATE_BINDINGS:
        matches = [b for b in bindings if isinstance(b, dict)
                   and (b.get("path"), b.get("kind"), b.get("symbol")) == (STATE, "fn", symbol)]
        if len(matches) != 1:
            errors.append(f"delegated State ledger owner {symbol} must occur exactly once")
        elif tuple(matches[0].get("required_tokens", ())) != tokens:
            errors.append(f"delegated State reviewed tokens changed for {symbol}")
        item = rust_binding_item(root, STATE, "fn", symbol, "delegated State", errors)
        if item is not None:
            items[symbol] = _code(item)
            for token in tokens:
                # Preserve the original diagnostic-string binding; the actual
                # strict comparator and refusal are checked independently below.
                if token in DELEGATED_STATE_DIAGNOSTIC_TOKENS:
                    if token not in item:
                        errors.append(f"delegated State {symbol} missing reviewed diagnostic {token!r}")
                elif _code(token).rstrip(",") not in items[symbol]:
                    errors.append(f"delegated State {symbol} missing executable relation {token!r}")

    def require(symbol: str, *relations: str) -> None:
        for relation in relations:
            if symbol in items and _code(relation) not in items[symbol]:
                errors.append(f"delegated State {symbol} missing executable relation {relation!r}")

    require("canonical_merged_lane_frontier_from_world",
            "Self::canonical_merged_lane_frontier_with_anchor_from_world(world, lane_id, dataspace_id, lane_incarnation,).map(|(height, descriptor_hash, _)| (height, descriptor_hash))")
    require("canonical_merged_lane_frontier_with_anchor_from_world",
            "Self::merge_lane_frontier_marker_key(lane_id, dataspace_id, lane_incarnation)?",
            "let Some(payload) = world.smart_contract_state().get(&key) else { return Ok((0, None, 0)); };",
            "Self::decode_exact_merge_lane_frontier_marker(&key, payload)?",
            "Ok((marker.lane_block_height, Some(marker.lane_block_descriptor_hash), marker.applied_global_height,))")
    require("validate_merge_execution_predecessor_against_frontier",
            "Self::validate_lane_frontier_successor(world, (descriptor.lane_id, descriptor.dataspace_id, descriptor.lane_incarnation,), descriptor.lane_block_height, descriptor.previous_lane_block_height, descriptor.previous_lane_block_descriptor_hash,)")
    require("validate_lane_frontier_successor",
            "Self::canonical_merged_lane_frontier_from_world(world, lane_id, dataspace_id, lane_incarnation,)?",
            "let actual_predecessor = (previous_height, previous_hash);",
            "if actual_predecessor != expected_predecessor { return Err(",
            "expected_predecessor.0.checked_add(1).ok_or_else(",
            "if lane_block_height != expected_height { return Err(MergeLedgerCommitError::NonContiguousLaneSnapshot {")
    require("preexecute_merge_execution_sources_into_with_replay",
            "let replay_authority = replay.map(|token| token.native_amx_authority(&*state_block)).transpose().map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;",
            "match replay_authority.as_ref() { Some(authority) => authority, None => &*state_block, }")
    require("stage_certified_merge_entry_with_replay",
            "self.ensure_pristine_execution_control_stage()?; if let Some(authority) = replay { authority.validate_merge_stage(&self._curr_block, &*self, entry).map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?; }",
            ".validate_certified_merge_entry_for_global_order_with_replay(entry, frozen_mode, replay,)?;",
            "State::preexecute_merge_execution_sources_into_with_replay(self, sources, replay)")

    wrapper = items.get("validate_merge_execution_batch")
    if wrapper is not None:
        body = wrapper.partition("{")[2].rsplit("}", 1)[0]
        expected = _code("self.validate_merge_execution_batch_with_replay(active_lanes, batch, validation_authority, None,)")
        if body != expected:
            errors.append("delegated State validate_merge_execution_batch must return the exact no-replay owner result")
    require("validate_merge_execution_batch_with_replay",
            "MergeExecutionValidationAuthority::Live(mode) => Some(*mode)",
            "if authority.entry.active_lanes != active_lanes || authority.entry.execution_batch.as_ref() != Some(batch) { return Err(invalid_batch(",
            "let validate_live_authority = frozen_mode.is_some();",
            "if !crate::merge::merge_execution_batch_commitments_match(batch) { return Err(",
            "let order = merge_execution_canonical_order_key(&execution.proposal); if previous_order.is_some_and(|previous| order <= previous) { return Err(",
            "previous_order = Some(order);",
            "if binding.incarnation != descriptor.lane_incarnation { return Err(MergeLedgerCommitError::LaneIncarnationMismatch",
            "if validate_live_authority { Self::validate_merge_execution_predecessor_against_frontier(world, descriptor)?; }",
            "if Hash::from(entrypoint.hash()) != expected_hash { return Err(",
            "if !validate_live_authority { crate::block::validate_historical_native_amx_source_bundle(&execution.source_bundle, execution.autonomous_network_id, execution.autonomous_epoch, crate::block::HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes(active_lanes,),)",
            "let replay_authority = replay.map(|token| token.native_amx_authority(&authority)).transpose().map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;",
            "match replay_authority.as_ref() { Some(authority) => authority, None => &authority, }",
            "crate::block::validate_native_amx_receipt_against_plan(receipt, &source.origin_proposal, entrypoint.hash(), routing_plan, source_id, execution.autonomous_network_id, &authority.nexus.dataspace_catalog, receipt_authority, Some(expected_v2_context),)")

    require("select_merge_execution_source_budget",
            "sources.sort_by_key(|source| { (source.origin_proposal.descriptor.proposal_height, merge_execution_canonical_order_key(&source.certified.proposal),) });",
            "let Some(next_entrypoints) = selected_entrypoints.checked_add(source.input.entrypoints.len()) else { break; };",
            "if next_entrypoints > MAX_MERGE_EXECUTION_ENTRYPOINTS || !crate::gas::gas_components_fit_block_limit(gas_limit, [selected_gas, gas]) { break; }",
            "let Some(next_gas) = selected_gas.checked_add(gas) else { break; };",
            "selected_gas = next_gas; selected_entrypoints = next_entrypoints; selected_count += 1;",
            "sources.truncate(selected_count); Ok(sources)")
    require("select_merge_execution_candidate_for_consensus",
            "let sources = match select_merge_execution_source_budget(sources, gas_limit_from_parameters(world.parameters()),)",
            "sources[..prefix_len].to_vec()")
    require("build_merge_execution_batch_from_source_prefix",
            "sources.sort_by_key(|source| { merge_execution_canonical_order_key(&source.certified.proposal) });",
            "if let Err(err) = state_block.validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)")
    require("apply_without_execution_inner",
            "if let Err(error) = self.prepare_deterministic_carrier_metadata(block.as_ref(), topology, topology_authority,) { return (Vec::new(), Err(error)); }",
            "let events = match self.prepare_carrier_publication_events(block.as_ref().header()) { Ok(events) => events, Err(error) => return (Vec::new(), Err(error)), };",
            "let carrier_authorization = if self.staged_merge_entry.as_ref().is_some_and(|entry| entry.execution_batch.is_some()) { self.mint_canonical_carrier_commit_metadata_authorization(block) } else { Ok(()) };",
            "(events, carrier_authorization)")
    require("prepare_carrier_publication_events",
            "if header != self._curr_block { return Err(",
            "if self.staged_merge_entry.as_ref().is_some_and(|entry| entry.execution_batch.is_some()) { self.validate_merge_execution_external_event_publication_surface()?; }",
            "self.world.external_event_buf.push(BlockEvent { header, status: BlockStatus::Applied, }.into(),);",
            "Ok(self.world.take_external_events())")
    require("composed_external_events", "self.beacon_composition.as_ref().map_or((self.external_event_bytes.as_deref(), self.external_event_count,), |seal| { (seal.external_event_bytes.as_deref(), seal.external_event_count,) },)")
    require("validate_staged_merge_execution_authorization",
            "let (composed_event_bytes, composed_event_count) = authorization.composed_external_events();",
            "if autonomous_event_prefix_bytes.as_deref() != composed_event_bytes { return Err(",
            "Self::merge_execution_write_set_root_from_overlay_with_external_events(&self.world, &self.merge_carrier_entrypoints, composed_event_bytes, self.merge_execution_runtime_effects().as_ref(),)")
    # Presence alone cannot permit minting before metadata/event validation succeeds.
    for symbol, ordered in (
        ("apply_verified_merge_beacon_pulse", ("capability.into_parts()", "self.validate_merge_execution_commit_surface(", "let original_root =", "!Self::canonical_wsv_merge_commit_authorization_matches(", "self.apply_pristine_npos_consensus_effects(", "let external_event_bytes =", "let seal =", ".beacon_composition = Some(seal)")),
        ("apply_without_execution_inner", ("self.prepare_deterministic_carrier_metadata(", "self.prepare_carrier_publication_events(", "self.mint_canonical_carrier_commit_metadata_authorization(")),
        ("prepare_carrier_publication_events", ("if header != self._curr_block", "self.validate_merge_execution_external_event_publication_surface()?", "self.world.external_event_buf.push(", "self.world.take_external_events()")),
    ):
        item = items.get(symbol)
        if item is not None:
            positions = [item.find(_code(token)) for token in ordered]
            if -1 in positions or positions != sorted(set(positions)):
                errors.append(f"delegated State {symbol} reorders carrier publication authorization")
