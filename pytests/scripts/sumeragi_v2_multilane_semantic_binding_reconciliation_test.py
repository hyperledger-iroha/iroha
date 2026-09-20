"""Actual-owner controls for moved first-release multilane semantic bindings.

Use the production Rust parser and relation validators on physical source files.
Missing and mutated full providers retain fresh hashes; these scoped checks do
not qualify recursive source admission, native execution, or the complete gate.
"""
from __future__ import annotations

import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
FORMAL = ROOT / "scripts/formal"
if str(FORMAL) not in sys.path:
    sys.path.insert(0, str(FORMAL))
import sumeragi_v2_multilane_authority_recovery_contract as authority
import sumeragi_v2_multilane_native_merge_manifest_contract as native

spec = importlib.util.spec_from_file_location(
    "semantic_reconciliation_actual_checker", FORMAL / "check_sumeragi_v2_multilane_models.py"
)
assert spec is not None and spec.loader is not None
checker = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = checker
spec.loader.exec_module(checker)

# Each moved owner is explicit: deleting its declaration cannot shrink this census.
KEYS = (('crates/iroha_core/src/kura/lane_artifact_budget.rs', 'fn', 'lane_artifact_required_bytes_for_block'), ('crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_validate_apply_cases.rs', 'fn', 'validator_apply_drains_exact_suffix_after_delayed_commit_qc_admission'), ('crates/iroha_core/src/sumeragi/lane_planner.rs', 'fn', 'v2_known_lane_tip_for_route'), ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs', 'method', 'ApplyFixture::new_with_options_and_retention'), ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs', 'method', 'ApplyFixture::new_with_options_and_retention_and_genesis'), ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs', 'method', 'ApplyFixture::new_with_options_and_retention_and_genesis_and_archival_kura'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::lane_publication_budget_reserved_bytes'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::native_amx_publication_capacity_reserved_bytes'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'NativeAmxPublicationCapacityReservation::reserved_bytes'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'NativeAmxRoutePublicationCapacity::reserved_bytes'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::native_amx_publication_plan_under_prune_and_canonical_guards'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::native_amx_publication_plan_for_storage_under_prune_and_canonical_guards'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::native_amx_route_publication_capacity_for_storage_locked'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::native_amx_route_publication_capacity_at_target_locked'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::admit_native_amx_publication_capacity_plan'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::check_native_amx_existing_carrier_capacity_under_prune_and_canonical_guards'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::ensure_native_amx_publication_capacity_under_publication_guard'), ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_01c_historical_recovery.rs', 'fn', 'run_autonomous_merge_frontier_fixture'), ('crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_validate_apply_cases.rs', 'fn', 'ready_validate_apply_actor_global_child_fixture'), ('crates/iroha_core/src/kura/native_amx_publication_capacity.rs', 'method', 'Kura::begin_native_amx_store_capacity_under_prune_and_canonical_guards'), ('crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs', 'method', 'Kura::store_block_durable'), ('crates/iroha_core/src/kura.rs', 'method', 'Kura::replace_top_block'), ('crates/iroha_core/src/kura.rs', 'method', 'Kura::check_storage_budget'), ('crates/iroha_core/src/kura.rs', 'method', 'Kura::check_replace_storage_budget'), ('crates/iroha_core/src/kura/autonomous_terminal_capacity.rs', 'method', 'Kura::validate_configured_autonomous_mutation_disk_peak_with_reservation_deltas_locked'), ('crates/iroha_core/src/kura/lane_artifact_budget.rs', 'method', 'Kura::lane_artifact_required_bytes_for_block'), ('crates/iroha_core/src/queue.rs', 'method', 'Queue::complete_lane_reservation_startup_reconciliation'))


def bindings():
    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    rows = [row for model in document["models"] for row in model["production_symbols"]]
    rows.extend(document["inflight_first_release_layout_contract"]["production_symbols"])
    return rows


def actual_item(root, path, kind, symbol, label, errors):
    provider = root / path
    if not provider.is_file() or provider.is_symlink():
        errors.append(f"{label}: missing physical provider {path}")
        return None
    items = checker._extract_rust_binding_items(provider.read_text(), kind, symbol)
    if len(items) != 1:
        errors.append(f"{label}: {path}!{symbol} owner count is {len(items)}")
        return None
    return items[0]


def check_owner(root, key):
    rows = [r for r in bindings() if (r["path"], r["kind"], r["symbol"]) == key]
    assert rows, f"missing canonical semantic declaration {key}"
    errors = []
    item = actual_item(root, *key, "semantic source owner", errors)
    if item is not None:
        for token in dict.fromkeys(t for row in rows for t in row["required_tokens"]):
            if token not in item:
                errors.append(f"{key[2]} missing semantic token {token!r}")
    return errors


def changed_provider(tmp_path, key, before, after):
    provider = ROOT / key[0]
    original = provider.read_bytes()
    assert before != after and original.count(before.encode()) == 1
    changed = original.replace(before.encode(), after.encode(), 1)
    destination = tmp_path / key[0]
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(changed)
    receipt = {"path": key[0], "before_sha256": hashlib.sha256(original).hexdigest(),
               "after_sha256": hashlib.sha256(changed).hexdigest()}
    (tmp_path / "refreshed-source.json").write_text(json.dumps(receipt, sort_keys=True))
    assert hashlib.sha256(destination.read_bytes()).hexdigest() == receipt["after_sha256"]
    assert receipt["before_sha256"] != receipt["after_sha256"]


@pytest.mark.parametrize("key", KEYS, ids=lambda k: k[2])
def test_moved_semantic_owner_matches_actual_source(key):
    assert check_owner(ROOT, key) == []


@pytest.mark.parametrize("key", KEYS, ids=lambda k: k[2])
def test_moved_semantic_owner_requires_its_physical_provider(tmp_path, key):
    assert any("missing physical provider" in e for e in check_owner(tmp_path, key))


# Mutate behavior-bearing guards/call edges, not comments or fixture labels.
MUTATIONS = (
    ("Kura::store_block_durable", "self.check_storage_budget(block, merge_entry)?;", "let _ = (block, merge_entry);"),
    ("Kura::replace_top_block", "self.check_replace_storage_budget(block.as_ref())?;", "let _ = block.as_ref();"),
    ("Kura::lane_publication_budget_reserved_bytes", "merge.checked_add(native)", "merge.saturating_add(native)"),
    ("Kura::native_amx_publication_capacity_reserved_bytes", "checked_add(reservation.reserved_bytes()", "saturating_add(reservation.reserved_bytes()"),
    ("NativeAmxPublicationCapacityReservation::reserved_bytes", "total.checked_add(route.reserved_bytes()?)", "total.saturating_add(route.reserved_bytes()?)"),
    ("NativeAmxRoutePublicationCapacity::reserved_bytes", ".try_fold(self.prune_journal_bytes,", ".try_fold(0_u64,"),
    ("Kura::native_amx_publication_plan_for_storage_under_prune_and_canonical_guards", "from_result_bearing_block_and_merge_entry(block, merge_entry)", "from_result_bearing_block_and_merge_entry(block, None)"),
    ("Kura::native_amx_route_publication_capacity_at_target_locked", "self.require_active_lane_artifact(entry, descriptor)?;", "let _ = (entry, descriptor);"),
    ("Kura::admit_native_amx_publication_capacity_plan", "old.proposal_hash != new.proposal_hash", "false"),
    ("Kura::check_native_amx_existing_carrier_capacity_under_prune_and_canonical_guards", "if required > self.max_disk_usage_bytes", "if false"),
    ("Kura::ensure_native_amx_publication_capacity_under_publication_guard", "self.check_native_amx_existing_carrier_capacity_under_prune_and_canonical_guards()?;", "let _ = ();"),
    ("ApplyFixture::new_with_options_and_retention", "blocks_in_memory,\n            false,", "blocks_in_memory,\n            true,"),
    ("ApplyFixture::new_with_options_and_retention_and_genesis_and_archival_kura", "(1_u8..=4)", "(1_u8..=1)"),
    ("ApplyFixture::new_with_options_and_retention_and_genesis_and_archival_kura", "install_fixture_validator_authority(&state, &context, &validator_set_pops);", "let _ = &validator_set_pops;"),
    ("v2_known_lane_tip_for_route", "NativeAmxParticipantApplicationObservation::PendingManifestRepair(_)", "NativeAmxParticipantApplicationObservation::PendingReceiptRepair(_)"),
    ("Queue::complete_lane_reservation_startup_reconciliation", "|| !reconciliation_pending", "|| (!receipt.initial_snapshot.is_empty() && !reconciliation_pending)"),
    ("ready_validate_apply_actor_global_child_fixture", "successor_case == ApplySuccessorCase::ValidatorRetained", "false"),
)


@pytest.mark.parametrize("symbol,before_token,after_token", MUTATIONS)
def test_moved_semantic_guard_rejects_full_source_drift(tmp_path, symbol, before_token, after_token):
    key = next(k for k in KEYS if k[2] == symbol)
    errors = []
    item = actual_item(ROOT, *key, "negative preimage", errors)
    assert errors == [] and item is not None and before_token in item
    changed_provider(tmp_path, key, item, item.replace(before_token, after_token, 1))
    assert any("missing semantic token" in e for e in check_owner(tmp_path, key))


def test_native_relations_preserve_all_extracted_historical_assertions():
    errors = []
    native.validate_native_merge_manifest_relations(ROOT, {}, errors, actual_item)
    assert errors == []


def test_historical_macro_requires_its_exact_case(tmp_path):
    relative, _name, _tokens = native.NATIVE_MERGE_MANIFEST_RAW_TEST_CHECKS[0]
    source = (ROOT / relative).read_text()
    before = "run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::HistoricalRecovery);"
    assert source.count(before) == 1
    destination = tmp_path / relative
    destination.parent.mkdir(parents=True)
    destination.write_text(source.replace(before, "run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::SuccessfulApply);", 1))
    errors = []
    native._validate_native_merge_manifest_raw_tests(tmp_path, errors)
    assert any("must dispatch exactly" in e for e in errors)


def test_extracted_historical_helper_requires_actual_assertions(tmp_path):
    key = next(k for k in KEYS if k[2] == "run_autonomous_merge_frontier_fixture")
    errors = []
    item = actual_item(ROOT, *key, "historical helper", errors)
    assert errors == [] and item is not None
    token = "assert_eq!(queue.live_lane_reservations(), reservations);"
    changed_provider(tmp_path, key, item, item.replace(token, 'let _ = reservations;', 1))
    assert any("missing semantic token" in e for e in check_owner(tmp_path, key))


def test_known_lane_tip_keeps_pending_abstention_and_applied_route_order():
    binding = next(r for r in authority.AUTHORITY_RECOVERY_BINDINGS if r[3] == "v2_known_lane_tip_for_route")
    errors = []
    item = actual_item(ROOT, *binding[1:4], "known lane tip", errors)
    assert errors == [] and item is not None
    authority.validate_authority_recovery_item(item, binding, errors)
    assert errors == []


def test_release_component_pin_joins_actual_owner_and_canonical_fixture_checks():
    writer = ROOT / "scripts/write_sumeragi_v2_release_receipt.py"
    tree = ast.parse(writer.read_text())
    pins = ast.literal_eval(next(n.value for n in tree.body if isinstance(n, ast.Assign)
        and any(isinstance(t, ast.Name) and t.id == "_RELEASE_RECEIPT_COMPONENT_SHA256" for t in n.targets)))
    component = "write_sumeragi_v2_release_receipt_gate_evidence.py"
    payload = (ROOT / "scripts" / component).read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    assert pins[component] == digest
    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    mutation = next(m for m in document["closure_mutations"] if m["obligation"] == "MLFixtureHasOneCanonicalOwner")
    writer_check = next(c for c in mutation["source_checks"] if c["path"] == "scripts/write_sumeragi_v2_release_receipt.py")
    assert digest in writer_check["required_tokens"]
    for source_check in mutation["source_checks"]:
        source = (ROOT / source_check["path"]).read_text()
        assert all(token in source for token in source_check["required_tokens"])
    assert not hasattr(checker, "_RELEASE_SOURCE_TOKEN_REBINDINGS")
    assert not hasattr(checker, "_PRODUCTION_TOKEN_REBINDINGS")
    errors = []
    checker.reviewed_source._validate_exact_release_invariant_source_checks(mutation["id"], mutation["source_checks"], errors)
    assert errors == []
    assert hashlib.sha256(payload + b"\n# component drift\n").hexdigest() != pins[component]


def test_inflight_ordered_and_source_declarations_remain_exact():
    import sumeragi_v2_multilane_inflight_contract as inflight

    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    declared = document["inflight_first_release_layout_contract"]
    assert declared["ordered_source_checks"] == [
        {"path": path, "kind": kind, "symbol": symbol, "tokens": list(tokens)}
        for path, kind, symbol, tokens in inflight.INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS
    ]
    assert declared["source_checks"] == [
        {"path": path, "required_tokens": list(tokens)}
        for path, tokens in inflight.INFLIGHT_LAYOUT_SOURCE_CHECKS
    ]


def test_autonomous_terminal_and_authority_contracts_use_current_declarations():
    import sumeragi_v2_multilane_autonomous_terminal_contract as terminal

    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    errors = []
    terminal.validate_autonomous_terminal_recovery_contract(ROOT, document["models"], errors, actual_item)
    authority.validate_authority_recovery_contract(ROOT, document["models"], errors, actual_item)
    assert errors == []


# State wrapper and helper owners are explicit; removing a table row must not
# silently shrink the selected semantic control census.
STATE_MERGE_SYMBOLS = (
    "pending_autoscale_lane_drain_body",
    "pending_autoscale_lane_drain_body_with_frontier",
    "build_merge_execution_candidate_for_consensus",
    "select_merge_execution_source_budget",
    "merge_execution_proposal_gas",
    "build_merge_execution_batch_from_source_prefix",
    "validate_merge_execution_predecessor_against_frontier",
    "validate_lane_frontier_successor",
    "preexecute_merge_execution_sources",
    "preexecute_merge_execution_sources_into",
    "preexecute_merge_execution_sources_into_with_replay",
    "stage_certified_merge_entry",
    "stage_certified_merge_entry_with_replay",
    "validate_merge_execution_batch",
    "validate_merge_execution_batch_with_replay",
)


def state_merge_binding(symbol):
    import sumeragi_v2_multilane_state_merge_contract as contract

    assert tuple(row[3] for row in contract.STATE_MERGE_BINDINGS) == STATE_MERGE_SYMBOLS
    return next(row for row in contract.STATE_MERGE_BINDINGS if row[3] == symbol)


def state_merge_errors(root, symbol):
    import sumeragi_v2_multilane_state_merge_contract as contract

    _module, path, kind, name, tokens = state_merge_binding(symbol)
    errors = []
    item = actual_item(root, path, kind, name, "State merge semantic owner", errors)
    if item is not None:
        contract.validate_state_merge_source_item(item, name, tokens, errors)
    return errors


@pytest.mark.parametrize("symbol", STATE_MERGE_SYMBOLS)
def test_state_merge_wrapper_helper_matches_actual_source(symbol):
    assert state_merge_errors(ROOT, symbol) == []


@pytest.mark.parametrize("symbol", STATE_MERGE_SYMBOLS)
def test_state_merge_wrapper_helper_requires_physical_owner(tmp_path, symbol):
    assert any("missing physical provider" in error for error in state_merge_errors(tmp_path, symbol))


STATE_MERGE_MUTATIONS = (
    ("pending_autoscale_lane_drain_body", "&self.kura,", "&foreign_kura,"),
    ("pending_autoscale_lane_drain_body_with_frontier", "if !autoscale_lane_drain_state_matches_context(", "if autoscale_lane_drain_state_matches_context("),
    ("pending_autoscale_lane_drain_body_with_frontier", "state.intent.min_quorum != min_quorum", "state.intent.min_quorum < min_quorum"),
    ("pending_autoscale_lane_drain_body_with_frontier", "frontier(lane.id, lane.dataspace_id, incarnation)?", "frontier(lane.id, lane.dataspace_id, foreign_incarnation)?"),
    ("build_merge_execution_candidate_for_consensus", "gas_limit_from_parameters(world.parameters())", "u64::MAX"),
    ("build_merge_execution_candidate_for_consensus", "if descriptor.validator_set != authoritative", "if false"),
    ("select_merge_execution_source_budget", "source.origin_proposal.descriptor.proposal_height,", "source.certified.proposal.descriptor.proposal_height,"),
    ("select_merge_execution_source_budget", "selected_entrypoints.checked_add(source.input.entrypoints.len())", "selected_entrypoints.saturating_add(source.input.entrypoints.len())"),
    ("select_merge_execution_source_budget", "!crate::gas::gas_components_fit_block_limit(gas_limit, [selected_gas, gas])", "false"),
    ("select_merge_execution_source_budget", "sources.truncate(selected_count);", "let _ = selected_count;"),
    ("merge_execution_proposal_gas", "total.checked_add(gas)", "total.saturating_add(gas)"),
    ("build_merge_execution_batch_from_source_prefix", "merge_execution_canonical_order_key(&source.certified.proposal)", "source.origin_proposal.descriptor.proposal_height"),
    ("build_merge_execution_batch_from_source_prefix", "validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)", "validate_merge_execution_commit_surface(MergeExecutionCommitSurface::PostBlockPreVote)"),
    ("validate_merge_execution_predecessor_against_frontier", "descriptor.lane_incarnation,", "foreign_incarnation,"),
    ("validate_lane_frontier_successor", "if actual_predecessor != expected_predecessor", "if false"),
    ("validate_lane_frontier_successor", "expected_predecessor.0.checked_add(1)", "expected_predecessor.0.checked_add(2)"),
    ("preexecute_merge_execution_sources", "Self::preexecute_merge_execution_sources_into(&mut state_block, sources)?", "Vec::new()"),
    ("preexecute_merge_execution_sources_into", "state_block, sources, None", "state_block, sources, replay"),
    ("preexecute_merge_execution_sources_into_with_replay", "[state_block.gas_used_in_block, reserved_gas]", "[0, reserved_gas]"),
    ("preexecute_merge_execution_sources_into_with_replay", "source.input.reservation_keys != authenticated_payload.reservation_keys", "false"),
    ("preexecute_merge_execution_sources_into_with_replay", "!seen_reservations.insert(reservation.digest())", "false"),
    ("stage_certified_merge_entry", "entry, frozen_mode, None", "entry, frozen_mode, replay"),
    ("stage_certified_merge_entry_with_replay", "State::preexecute_merge_execution_sources_into_with_replay(self, sources, replay)", "State::preexecute_merge_execution_sources_into_with_replay(self, sources, None)"),
    ("stage_certified_merge_entry_with_replay", "validated_publication_event_bytes: None", "validated_publication_event_bytes: Some(Vec::new())"),
    ("validate_merge_execution_batch", "validation_authority,\n            None,", "validation_authority,\n            replay,"),
    ("validate_merge_execution_batch_with_replay", "authority.entry.execution_batch.as_ref() != Some(batch)", "false"),
    ("validate_merge_execution_batch_with_replay", "order <= previous", "order < previous"),
    ("validate_merge_execution_batch_with_replay", "reservation.routing_plan_digest != routing_plan.digest()", "false"),
)


@pytest.mark.parametrize("symbol,before_token,after_token", STATE_MERGE_MUTATIONS)
def test_state_merge_rejects_semantic_source_drift(tmp_path, symbol, before_token, after_token):
    _module, path, kind, name, _tokens = state_merge_binding(symbol)
    errors = []
    item = actual_item(ROOT, path, kind, name, "State mutation preimage", errors)
    assert errors == [] and item is not None and item.count(before_token) == 1
    changed_provider(tmp_path, (path, kind, name), item, item.replace(before_token, after_token, 1))
    assert state_merge_errors(tmp_path, symbol)


def test_state_merge_commit_surface_must_precede_marker_staging(tmp_path):
    symbol = "stage_certified_merge_entry_with_replay"
    _module, path, kind, name, _tokens = state_merge_binding(symbol)
    errors = []
    item = actual_item(ROOT, path, kind, name, "State order preimage", errors)
    assert errors == [] and item is not None
    first = "self.validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)?;"
    second = "self.stage_merge_execution_markers(entry.epoch_id, batch)?;"
    assert item.count(first) == item.count(second) == 1
    changed = item.replace(first, "STATE_ORDER_SWAP", 1).replace(second, first, 1).replace("STATE_ORDER_SWAP", second, 1)
    changed_provider(tmp_path, (path, kind, name), item, changed)
    assert any("reordered token" in error for error in state_merge_errors(tmp_path, symbol))


def test_state_merge_contract_uses_exact_canonical_declarations():
    import sumeragi_v2_multilane_state_merge_contract as contract

    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    errors = []
    contract.validate_state_merge_source_contract(ROOT, document["models"], errors, actual_item)
    assert errors == []
    selected = next(model for model in document["models"] if model["module"] == "SumeragiV2AutonomousReservationCarrier")
    selected["production_symbols"] = [row for row in selected["production_symbols"] if row["symbol"] != "select_merge_execution_source_budget"]
    errors = []
    contract.validate_state_merge_source_contract(ROOT, document["models"], errors, actual_item)
    assert any("declaration must occur exactly once" in error for error in errors)


def queue_prepare_errors(root):
    import sumeragi_v2_multilane_queue_plan_contract as contract

    key = ("crates/iroha_core/src/sumeragi/v2_lane_work.rs", "method", "&mut V2LaneWorkAdapter::prepare")
    errors = []
    item = actual_item(root, *key, "QueuePlan signed-intent preparation", errors)
    if item is None:
        return errors
    tokens = next(row[3] for row in contract.QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS if row[:3] == key)
    ordered = next(row[3] for row in contract.QUEUE_PLAN_AUTONOMOUS_ONLY_ORDERED_SOURCE_CHECKS if row[:3] == key)
    for token in tokens:
        if token not in item:
            errors.append(f"QueuePlan preparation missing semantic token {token!r}")
    cursor = -1
    for token in ordered:
        position = item.find(token, cursor + 1)
        if position < 0:
            errors.append(f"QueuePlan preparation missing or reordered token {token!r}")
            break
        cursor = position
    return errors


def test_queue_plan_signed_intent_and_reserved_conflicts_match_source():
    assert queue_prepare_errors(ROOT) == []


def test_queue_plan_signed_intent_requires_physical_owner(tmp_path):
    assert any("missing physical provider" in error for error in queue_prepare_errors(tmp_path))


@pytest.mark.parametrize("before_token,after_token", (
    ("== iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced)", "== iroha_data_model::transaction::TransactionAdmissionIntent::Ordinary)"),
    ("reserved_routes.contains(&(leg.route.lane_id, leg.route.dataspace_id))", "false"),
    ("reserved_entrypoints.contains(&entrypoint) || route_conflict", "route_conflict"),
    ("reserved_entrypoints.contains(&entrypoint) || route_conflict", "reserved_entrypoints.contains(&entrypoint) && route_conflict"),
    ("autonomous_lane_payloads.len(),", "0,"),
))
def test_queue_plan_prepare_rejects_corridor_weakening(tmp_path, before_token, after_token):
    key = ("crates/iroha_core/src/sumeragi/v2_lane_work.rs", "method", "&mut V2LaneWorkAdapter::prepare")
    errors = []
    item = actual_item(ROOT, *key, "QueuePlan mutation preimage", errors)
    assert errors == [] and item is not None and item.count(before_token) == 1
    changed_provider(tmp_path, key, item, item.replace(before_token, after_token, 1))
    assert queue_prepare_errors(tmp_path)



def test_state_merge_contract_rejects_malformed_or_duplicate_declarations():
    import copy
    import sumeragi_v2_multilane_state_merge_contract as contract

    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    for mutation in (None, "missing_rows", "duplicate_model"):
        models = copy.deepcopy(document["models"])
        if mutation is None:
            models = None
        elif mutation == "missing_rows":
            next(model for model in models if model["module"] == "SumeragiV2AutonomousReservationCarrier")["production_symbols"] = None
        else:
            models.append(next(model for model in models if model["module"] == "SumeragiV2AutonomousReservationCarrier"))
        errors = []
        contract.validate_state_merge_source_contract(ROOT, models, errors, actual_item)
        assert errors


def test_queue_plan_full_source_contract_preserves_original_and_current_controls():
    import sumeragi_v2_multilane_queue_plan_contract as contract

    document = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    errors = []
    contract.validate_queue_plan_autonomous_only_contract(
        ROOT, ROOT / "formal/sumeragi_v2", document["models"], errors,
        actual_item, checker._regular_file, checker.TLA_DECLARATION_TEMPLATE,
    )
    assert errors == []
