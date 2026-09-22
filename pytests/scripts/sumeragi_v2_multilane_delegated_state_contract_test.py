"""Actual-source and mutation controls for State execution/frontier delegation."""

from __future__ import annotations

import ast
import importlib.util
import sys
from pathlib import Path

import pytest


def support():
    path = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")
    spec = importlib.util.spec_from_file_location("delegated_state_test_support", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def validate(fixture):
    root, _, checker, models = fixture
    errors = []
    with checker._reviewed_rust_source_cache():
        checker.delegated_state_contract.validate_delegated_state_contract(
            root, models, errors, checker._rust_binding_item,
        )
    return tuple(errors)


@pytest.fixture
def fixture(tmp_path):
    helper = support()
    checker = helper.load_checker()
    helper.copy_reviewed_source_fixture_with_includes(
        tmp_path, checker, {Path(checker.delegated_state_contract.STATE)},
    )
    result = tmp_path, helper, checker, helper.canonical_models()
    assert validate(result) == ()
    return result


def test_delegated_state_accepts_actual_owners(fixture):
    assert validate(fixture) == ()


def test_delegated_state_is_connected_to_release_gate():
    checker = support().load_checker()
    tree = ast.parse(Path(checker.__file__).read_text(encoding="utf-8"))
    owners = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
    assert sum(
        isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
        and isinstance(n.func.value, ast.Name)
        and n.func.value.id == "delegated_state_contract"
        and n.func.attr == "validate_delegated_state_contract"
        for n in ast.walk(owners["_validate"])
    ) == 1
    assert any(
        isinstance(n, ast.Starred) and isinstance(n.value, ast.Attribute)
        and isinstance(n.value.value, ast.Name)
        and n.value.value.id == "delegated_state_contract"
        and n.value.attr == "DELEGATED_STATE_SOURCE_RELATIVES"
        for n in ast.walk(owners["source_manifest_sha256"])
    )


@pytest.mark.parametrize("symbol", [
    "validate_merge_execution_batch",
    "validate_merge_execution_batch_with_replay",
    "canonical_merged_lane_frontier_from_world",
    "canonical_merged_lane_frontier_with_anchor_from_world",
    "validate_merge_execution_predecessor_against_frontier",
    "validate_lane_frontier_successor",
    "preexecute_merge_execution_sources_into",
    "preexecute_merge_execution_sources_into_with_replay",
    "stage_certified_merge_entry",
    "stage_certified_merge_entry_with_replay",
    "stage_certified_merge_reference_for_verified_replay",
    "select_merge_execution_candidate_for_consensus",
    "build_merge_execution_candidate_for_consensus",
    "select_merge_execution_candidate_prefix",
    "select_merge_execution_source_budget",
    "build_merge_execution_batch_from_source_prefix",
    "apply_without_execution_inner",
    "prepare_carrier_publication_events",
])
def test_delegated_state_rejects_missing_forwarder_or_owner(fixture, symbol):
    _, _, checker, models = fixture
    model = next(m for m in models if m["module"] == checker.delegated_state_contract.MODEL)
    model["production_symbols"] = [b for b in model["production_symbols"] if b["symbol"] != symbol]
    assert any(f"ledger owner {symbol}" in e for e in validate(fixture))


def test_delegated_state_rejects_weakened_execution_ledger(fixture):
    _, _, checker, models = fixture
    model = next(m for m in models if m["module"] == checker.delegated_state_contract.MODEL)
    row = next(b for b in model["production_symbols"] if b["symbol"] == "stage_certified_merge_entry_with_replay")
    row["required_tokens"].remove("application_write_set_root")
    assert any("reviewed tokens changed" in e for e in validate(fixture))


@pytest.mark.parametrize("symbol,old,new", [
    ("canonical_merged_lane_frontier_from_world", "            lane_incarnation,", "            replacement_incarnation,"),
    ("canonical_merged_lane_frontier_with_anchor_from_world", "Some(marker.lane_block_descriptor_hash)", "None"),
    ("canonical_merged_lane_frontier_with_anchor_from_world", "marker.applied_global_height", "0"),
    ("validate_merge_execution_predecessor_against_frontier", "descriptor.previous_lane_block_descriptor_hash", "None"),
    ("validate_merge_execution_predecessor_against_frontier", "descriptor.dataspace_id", "DataSpaceId::UNIVERSAL"),
    ("validate_lane_frontier_successor", "actual_predecessor != expected_predecessor", "actual_predecessor == expected_predecessor"),
    ("validate_lane_frontier_successor", "checked_add(1)", "checked_add(0)"),
    ("validate_lane_frontier_successor", "lane_block_height != expected_height", "lane_block_height < expected_height"),
    ("preexecute_merge_execution_sources_into", "state_block, sources, None", "state_block, sources, Some(unverified)"),
    ("preexecute_merge_execution_sources_into_with_replay", "token.native_amx_authority(&*state_block)", "token.native_amx_authority(other_state)"),
    ("preexecute_merge_execution_sources_into_with_replay", "Some(authority) => authority", "Some(authority) => &*state_block"),
    ("stage_certified_merge_entry", "entry, frozen_mode, None", "entry, frozen_mode, Some(unverified)"),
    ("stage_certified_merge_entry_with_replay", "self.ensure_pristine_execution_control_stage()?;", "// self.ensure_pristine_execution_control_stage()?;"),
    ("stage_certified_merge_entry_with_replay", "validate_merge_stage(&self._curr_block, &*self, entry)", "validate_merge_stage(&other_block, &*self, entry)"),
    ("stage_certified_merge_entry_with_replay", "preexecute_merge_execution_sources_into_with_replay(self, sources, replay)", "preexecute_merge_execution_sources_into_with_replay(self, sources, None)"),
    ("stage_certified_merge_reference_for_verified_replay", ".merge_entry(reference)", ".merge_entry(other_reference)"),
])
def test_delegated_state_rejects_semantic_mutation(fixture, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.delegated_state_contract.STATE,
                              f"fn {symbol}(", old, new)
    errors = validate(fixture)
    assert any("missing executable relation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("symbol,old,new", [
    ("validate_merge_execution_batch", "            active_lanes,", "            foreign_lanes,"),
    ("validate_merge_execution_batch", "            batch,", "            foreign_batch,"),
    ("validate_merge_execution_batch", "            validation_authority,", "            foreign_authority,"),
    ("validate_merge_execution_batch", "            None,", "            Some(unverified),"),
    ("validate_merge_execution_batch", "        self.validate_merge_execution_batch_with_replay(", "        return Ok(()); self.validate_merge_execution_batch_with_replay("),
    ("validate_merge_execution_batch_with_replay", "Live(mode) => Some(*mode)", "Live(mode) => None"),
    ("validate_merge_execution_batch_with_replay", "authority.entry.active_lanes != active_lanes", "authority.entry.active_lanes == active_lanes"),
    ("validate_merge_execution_batch_with_replay", "authority.entry.execution_batch.as_ref() != Some(batch)", "authority.entry.execution_batch.as_ref() == Some(batch)"),
    ("validate_merge_execution_batch_with_replay", "let validate_live_authority = frozen_mode.is_some();", "let validate_live_authority = false;"),
    ("validate_merge_execution_batch_with_replay", "if !crate::merge::merge_execution_batch_commitments_match(batch)", "if crate::merge::merge_execution_batch_commitments_match(batch)"),
    ("validate_merge_execution_batch_with_replay", "order <= previous", "order < previous"),
    ("validate_merge_execution_batch_with_replay", "previous_order = Some(order);", "previous_order = None;"),
    ("validate_merge_execution_batch_with_replay", "binding.incarnation != descriptor.lane_incarnation", "binding.incarnation == descriptor.lane_incarnation"),
    ("validate_merge_execution_batch_with_replay", "Self::validate_merge_execution_predecessor_against_frontier(world, descriptor)?;", "let _ = Self::validate_merge_execution_predecessor_against_frontier(world, descriptor);"),
    ("validate_merge_execution_batch_with_replay", "Hash::from(entrypoint.hash()) != expected_hash", "Hash::from(entrypoint.hash()) == expected_hash"),
    ("validate_merge_execution_batch_with_replay", "if !validate_live_authority {", "if validate_live_authority {"),
    ("validate_merge_execution_batch_with_replay", "                        active_lanes,", "                        foreign_lanes,"),
    ("validate_merge_execution_batch_with_replay", "token.native_amx_authority(&authority)", "token.native_amx_authority(&foreign_state)"),
    ("validate_merge_execution_batch_with_replay", "Some(authority) => authority", "Some(authority) => &authority"),
    ("validate_merge_execution_batch_with_replay", "                    receipt_authority,", "                    &authority,"),
    ("validate_merge_execution_batch_with_replay", "Some(expected_v2_context)", "None"),
])
def test_merge_batch_delegation_rejects_semantic_mutation(fixture, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.delegated_state_contract.STATE,
                              f"fn {symbol}(", old, new)
    errors = validate(fixture)
    assert any("missing executable relation" in e or "exact no-replay owner result" in e
               for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


def test_merge_batch_delegation_preserves_original_owner_obligations(fixture):
    _, _, checker, models = fixture
    expected = {
        "reservation_keys", "routing_plans", "merge_execution_batch_commitments_match",
        "entrypoint_hash", "LaneIncarnationMismatch",
        "validate_merge_execution_predecessor_against_frontier", "if !validate_live_authority",
        "validate_historical_native_amx_source_bundle",
        "HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes",
        "merge_execution_canonical_order_key", "previous_order", "strict canonical total order",
    }
    model = next(m for m in models if m["module"] == checker.delegated_state_contract.MODEL)
    row = next(b for b in model["production_symbols"] if b["symbol"] == "validate_merge_execution_batch_with_replay")
    assert expected <= set(row["required_tokens"])
    row["required_tokens"].remove("validate_merge_execution_predecessor_against_frontier")
    assert any("reviewed tokens changed" in e for e in validate(fixture))


@pytest.mark.parametrize("symbol,old,new", [
    ("build_merge_execution_candidate_for_consensus", "deterministic_start_work_pending(&application_block_header)?", "deterministic_start_work_pending(&application_block_header).ok().flatten()"),
    ("build_merge_execution_candidate_for_consensus", ".map_err(StateBlockStartError::History)", ".or_else(|_| Ok(None))"),
    ("select_merge_execution_candidate_for_consensus", "        )?;", "        ).unwrap_or(None);"),
    ("select_merge_execution_candidate_prefix", "build_batch(midpoint)?", "build_batch(midpoint).unwrap_or(None)"),
    ("build_merge_execution_batch_from_source_prefix", "Err(MergeLedgerCommitError::BlockHashAdmission(error)) => return Err(error),", "Err(MergeLedgerCommitError::BlockHashAdmission(_)) => return Ok(None),"),
    ("select_merge_execution_candidate_for_consensus", "gas_limit_from_parameters(world.parameters())", "u64::MAX"),
    ("select_merge_execution_candidate_for_consensus", "sources[..prefix_len].to_vec()", "sources.clone()"),
    ("select_merge_execution_source_budget", "source.origin_proposal.descriptor.proposal_height", "source.certified.proposal.descriptor.proposal_height"),
    ("select_merge_execution_source_budget", "selected_entrypoints.checked_add(source.input.entrypoints.len())", "Some(0)"),
    ("select_merge_execution_source_budget", "next_entrypoints > MAX_MERGE_EXECUTION_ENTRYPOINTS", "false"),
    ("select_merge_execution_source_budget", "[selected_gas, gas]", "[gas]"),
    ("select_merge_execution_source_budget", "selected_gas.checked_add(gas)", "Some(gas)"),
    ("select_merge_execution_source_budget", "sources.truncate(selected_count);", "// sources.truncate(selected_count);"),
    ("build_merge_execution_batch_from_source_prefix", "merge_execution_canonical_order_key(&source.certified.proposal)", "source.origin_proposal.descriptor.proposal_height"),
    ("build_merge_execution_batch_from_source_prefix", "validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)", "validate_merge_execution_commit_surface(MergeExecutionCommitSurface::FinalizedCarrier)"),
    ("apply_without_execution_inner", "block.as_ref().header()", "foreign_header"),
    ("apply_without_execution_inner", "Err(error) => return (Vec::new(), Err(error))", "Err(error) => Vec::new()"),
    ("apply_without_execution_inner", "mint_canonical_carrier_commit_metadata_authorization(block)", "mint_canonical_carrier_commit_metadata_authorization(other_block)"),
    ("prepare_carrier_publication_events", "header != self._curr_block", "header == self._curr_block"),
    ("prepare_carrier_publication_events", "self.validate_merge_execution_external_event_publication_surface()?;", "// self.validate_merge_execution_external_event_publication_surface()?;"),
    ("prepare_carrier_publication_events", "status: BlockStatus::Applied", "status: BlockStatus::Rejected"),
    ("prepare_carrier_publication_events", "Ok(self.world.take_external_events())", "Ok(Vec::new())"),
])
def test_delegated_carrier_budget_and_publication_reject_semantic_mutation(fixture, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.delegated_state_contract.STATE,
                              f"fn {symbol}(", old, new)
    errors = validate(fixture)
    assert any("missing executable relation" in e for e in errors), errors
    assert not any("digest" in e or "must have one" in e for e in errors), errors


@pytest.mark.parametrize("symbol,old,new", [('composed_external_events', 'self.external_event_count,', '0,'),
 ('composed_external_events', 'seal.external_event_count,', '0,'),
 ('composed_external_events',
  'seal.external_event_bytes.as_deref()',
  'self.external_event_bytes.as_deref()'),
 ('composed_write_set_root', '|seal| seal.write_set_root', '|_seal| self.write_set_root'),
 ('apply_verified_merge_beacon_pulse',
  'actual_events.as_deref() != authorization.external_event_bytes.as_deref()',
  'false'),
 ('apply_verified_merge_beacon_pulse',
  'merge_beacon_parent_surface(&self.world) != parent_surface',
  'false'),
 ('apply_verified_merge_beacon_pulse',
  'capability: crate::block::valid::VerifiedMergeBeaconPulse',
  'capability: crate::block::valid::UnverifiedMergeBeaconPulse'),
 ('apply_verified_merge_beacon_pulse',
  'external_event_count: self.world.external_event_buf.len()',
  'external_event_count: 0'),
 ('validate_staged_merge_execution_authorization',
  '.get(..composed_event_count)',
  '.get(..authorization.external_event_count)'),
 ('validate_staged_merge_execution_authorization',
  'autonomous_event_prefix_bytes.as_deref() != composed_event_bytes',
  'false'),
 ('mint_canonical_carrier_commit_metadata_authorization',
  'authorization.composed_external_events().0',
  'authorization.external_event_bytes.as_deref()'),
 ('commit_inner',
  'authorization.composed_external_events().0',
  'authorization.external_event_bytes.as_deref()')])
def test_delegated_composed_events_reject_semantic_mutation(fixture, symbol, old, new):
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.delegated_state_contract.STATE,
                              f"fn {symbol}(", old, new)
    errors = validate(fixture)
    assert any("missing executable relation" in e for e in errors), errors
    assert not any("digest" in e for e in errors), errors


@pytest.mark.parametrize("old,new", [
    pytest.param("let mut this = self;", "let mut this = other;", id="commit-original-state"),
    pytest.param("this.fields.as_mut()", "this.into_fields()", id="commit-retains-field-owner"),
    pytest.param("canonical_wsv_merge_commit_authorization.as_ref()", "other_authorization.as_ref()", id="commit-retains-economic-authorization"),
    pytest.param("canonical_carrier_commit_metadata_authorization.as_ref()", "other_metadata.as_ref()", id="commit-retains-finalized-metadata"),
    pytest.param("transactions.try_prepare_publication()", "Ok(())", id="commit-prepares-original-membership"),
    pytest.param("tx_validate_result?;", "let _ = tx_validate_result;", id="commit-rejects-failed-membership"),
    pytest.param("transactions.publish_prepared()", "transactions.publish()", id="commit-publishes-prepared-membership"),
])
def test_delegated_commit_retains_original_fields_and_authority(fixture, old, new):
    """Economic and finality checks stay attached to the same consumed State."""
    root, helper, checker, _ = fixture
    helper.replace_once_after(root / checker.delegated_state_contract.STATE, "fn commit_inner(", old, new)
    errors = validate(fixture)
    assert any("missing executable relation" in error for error in errors), errors
    assert not any("digest" in error for error in errors), errors
