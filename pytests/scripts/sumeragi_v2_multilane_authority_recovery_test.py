"""Actual source-owner controls for authenticated State and canonical recovery.

These scoped tests use the production Rust item extractor against real provider
files. They do not admit a recursive include manifest, execute Rust, or qualify
the complete model gate. Mutated full files and refreshed SHA receipts survive
in pytest's retained basetemp for independent review.
"""
from __future__ import annotations

import copy
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
import sumeragi_v2_multilane_authority_recovery_contract as contract
import sumeragi_v2_multilane_queue_plan_contract as queue_contract

spec = importlib.util.spec_from_file_location("authority_recovery_real_checker", FORMAL / "check_sumeragi_v2_multilane_models.py")
assert spec is not None and spec.loader is not None
checker = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = checker
spec.loader.exec_module(checker)
BINDINGS = contract.AUTHORITY_RECOVERY_BINDINGS


def models():
    return json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())["models"]


def actual_item(root, path, kind, symbol, label, errors):
    """Extract the actual physical provider, without bypassing any gate in production."""
    provider = root / path
    if not provider.is_file() or provider.is_symlink():
        errors.append(f"{label}: missing physical provider {path}")
        return None
    items = checker._extract_rust_binding_items(provider.read_text(), kind, symbol)
    if len(items) != 1:
        errors.append(f"{label}: {path}!{symbol} owner count is {len(items)}")
        return None
    return items[0]


def source_item(binding):
    errors = []
    _module, path, kind, symbol, _required, _ordered = binding
    item = actual_item(ROOT, path, kind, symbol, "actual source23", errors)
    assert errors == [] and item is not None
    return item


def changed_provider(tmp_path, binding, old_item, new_item):
    _module, path, _kind, _symbol, _required, _ordered = binding
    source = (ROOT / path).read_bytes()
    assert source.count(old_item.encode()) == 1
    changed = source.replace(old_item.encode(), new_item.encode(), 1)
    assert changed != source
    destination = tmp_path / path
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(changed)
    receipt = {"path": path, "before_sha256": hashlib.sha256(source).hexdigest(),
               "after_sha256": hashlib.sha256(changed).hexdigest(), "bytes": len(changed),
               "owner_before_sha256": hashlib.sha256(old_item.encode()).hexdigest(),
               "owner_after_sha256": hashlib.sha256(new_item.encode()).hexdigest()}
    (tmp_path / "refreshed-source.json").write_text(json.dumps(receipt, sort_keys=True))
    retained = json.loads((tmp_path / "refreshed-source.json").read_text())
    assert hashlib.sha256(destination.read_bytes()).hexdigest() == retained["after_sha256"]
    assert retained["before_sha256"] != retained["after_sha256"]
    return destination


@pytest.mark.parametrize("binding", BINDINGS, ids=lambda b:b[3])
def test_actual_source23_owner_satisfies_current_contract(binding):
    errors = []
    contract.validate_authority_recovery_item(source_item(binding), binding, errors)
    assert errors == []


def test_actual_source23_declarations_are_explicit_and_complete():
    errors = []
    contract.validate_authority_recovery_contract(ROOT, models(), errors, actual_item)
    assert errors == []


@pytest.mark.parametrize("binding", BINDINGS, ids=lambda b:b[3])
def test_deleting_actual_owner_is_rejected_after_full_provider_rehash(tmp_path, binding):
    item = source_item(binding)
    changed_provider(tmp_path, binding, item, "")
    _module, path, kind, symbol, _required, _ordered = binding
    errors = []
    assert actual_item(tmp_path, path, kind, symbol, "deleted source owner", errors) is None
    assert any("owner count is 0" in error for error in errors)


@pytest.mark.parametrize("binding", BINDINGS, ids=lambda b:b[3])
def test_deleting_json_owner_cannot_remove_its_obligation(binding):
    candidate = models()
    module, path, kind, symbol, _required, _ordered = binding
    model = next(m for m in candidate if m["module"] == module)
    before = len(model["production_symbols"])
    model["production_symbols"] = [r for r in model["production_symbols"]
        if (r["path"], r["kind"], r["symbol"]) != (path, kind, symbol)]
    assert len(model["production_symbols"]) == before - 1
    errors = []
    contract.validate_authority_recovery_contract(ROOT, candidate, errors, actual_item)
    assert errors == [f"{module}: authority/recovery binding changed for {path}!{symbol}"]


@pytest.mark.parametrize("binding", BINDINGS, ids=lambda b:b[3])
def test_weakening_declared_required_tokens_is_rejected(binding):
    candidate = models()
    module, path, kind, symbol, _required, _ordered = binding
    model = next(m for m in candidate if m["module"] == module)
    row = next(r for r in model["production_symbols"] if (r["path"],r["kind"],r["symbol"]) == (path,kind,symbol))
    row["required_tokens"].pop()
    errors = []
    contract.validate_authority_recovery_contract(ROOT, candidate, errors, actual_item)
    assert errors == [f"{module}: authority/recovery binding changed for {path}!{symbol}"]


def test_queue_locked_body_current_tokens_and_order_match_actual_owner():
    binding = next(b for b in BINDINGS if b[3] == "V2LaneWorkAdapter::bind_locked_global_body_from_origin")
    _module, path, kind, symbol, required, ordered = binding
    row = next(b for b in queue_contract.QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS if b[:3] == (path,kind,symbol))
    assert row[3] == required
    order = next(b for b in queue_contract.QUEUE_PLAN_AUTONOMOUS_ONLY_ORDERED_SOURCE_CHECKS if b[:3] == (path,kind,symbol))
    assert order[3] == ordered
    errors = []
    contract.validate_authority_recovery_item(source_item(binding), binding, errors)
    assert errors == []


@pytest.fixture(scope="module")
def actual_queue_startup_items():
    """Cache actual Rust owners used by the complete startup contract."""
    rows = (
        queue_contract.QUEUE_PLAN_STARTUP_REPLAY_BINDINGS
        + queue_contract.QUEUE_PLAN_STARTUP_REPLAY_ORDERED_SOURCE_CHECKS
        + queue_contract.QUEUE_PLAN_STARTUP_REPLAY_FORBIDDEN_SOURCE_CHECKS
        + tuple((path, "fn", symbol, tokens) for path, symbol, tokens in
                queue_contract.QUEUE_PLAN_STARTUP_REPLAY_TEST_BINDINGS)
    )
    items = {}
    for path, kind, symbol, _tokens in rows:
        key = (path, kind, symbol)
        if key not in items:
            errors = []
            items[key] = actual_item(ROOT, *key, "actual Queue startup owner", errors)
            assert errors == [] and items[key] is not None
    return items


def test_empty_queue_startup_publication_actual_source_contract(
    monkeypatch, actual_queue_startup_items
):
    monkeypatch.setattr(checker, "_rust_binding_item", lambda root, path, kind,
                        symbol, label, errors: actual_queue_startup_items[path, kind, symbol])
    errors = []
    checker._validate_queue_plan_startup_replay_contract(ROOT, models(), errors)
    assert errors == []


@pytest.mark.parametrize("symbol,old,new", [
    ("Queue::install_lane_reservation_journal",
     ".store(true, Ordering::Release);", ".store(false, Ordering::Release);"),
    ("Queue::install_lane_reservation_journal",
     ".store(true, Ordering::Release);",
     ".store(!store.live_by_entrypoint.is_empty(), Ordering::Release);"),
    ("Queue::bind_lane_reservation_startup_reconciliation_receipt",
     "if !self\n            .lane_reservation_reconciliation_pending\n            .load(Ordering::Acquire)",
     "if !expected_snapshot.is_empty() && !self.lane_reservation_startup_reconciliation_pending()"),
    ("Queue::revalidate_lane_reservation_startup_reconciliation_receipt",
     "|| !self.lane_reservation_startup_reconciliation_pending()", "|| false"),
    ("Queue::revalidate_lane_reservation_startup_reconciliation_receipt_locked",
     "|| !self.lane_reservation_startup_reconciliation_pending()", "|| false"),
    ("Queue::complete_lane_reservation_startup_reconciliation",
     "|| !reconciliation_pending", "|| (!receipt.initial_snapshot.is_empty() && !reconciliation_pending)"),
    ("Queue::complete_lane_reservation_startup_reconciliation",
     "*self.lane_reservation_startup_completion.lock() =",
     "self.lane_reservation_reconciliation_pending.store(false, Ordering::Release);\n        "
     "*self.lane_reservation_startup_completion.lock() ="),
])
def test_empty_queue_startup_publication_rejects_early_open(
    tmp_path, monkeypatch, actual_queue_startup_items, symbol, old, new
):
    key = ("crates/iroha_core/src/queue.rs", "method", symbol)
    item = actual_queue_startup_items[key]
    assert item.count(old) == 1
    mutated = item.replace(old, new, 1)
    # Move the opening store, rather than duplicating a harmless later store.
    if new.startswith("self.lane_reservation_reconciliation_pending.store(false"):
        later_store = ("        self.lane_reservation_reconciliation_pending\n"
                       "            .store(false, Ordering::Release);\n")
        assert mutated.count(later_store) == 1
        mutated = mutated.replace(later_store, "", 1)
    binding = (queue_contract.QUEUE_PLAN_STARTUP_REPLAY_MODULE, *key, (), ())
    changed_provider(tmp_path, binding, item, mutated)

    def provider(root, path, kind, requested, label, errors):
        if (path, kind, requested) == key:
            return actual_item(tmp_path, path, kind, requested, label, errors)
        return actual_queue_startup_items[path, kind, requested]

    monkeypatch.setattr(checker, "_rust_binding_item", provider)
    errors = []
    checker._validate_queue_plan_startup_replay_contract(ROOT, models(), errors)
    assert any(f"ordered QueuePlan startup replay item {symbol}" in error
               for error in errors)


@pytest.mark.parametrize("bad", [None, {}, "models"])
def test_malformed_model_container_cannot_skip_contract(bad):
    errors = []
    contract.validate_authority_recovery_contract(ROOT, bad, errors, actual_item)
    assert errors == ["authority/recovery models must be an array"]


def test_duplicate_or_missing_model_and_owner_are_rejected():
    candidate = models()
    errors = []
    candidate.append(copy.deepcopy(candidate[1]))
    contract.validate_authority_recovery_contract(ROOT, candidate, errors, actual_item)
    assert errors and all("exactly one model" in error for error in errors)
    candidate = models()
    candidate[1]["production_symbols"] = None
    errors = []
    contract.validate_authority_recovery_contract(ROOT, candidate, errors, actual_item)
    assert errors and all("must be an array" in error for error in errors)
    candidate = models()
    row = BINDINGS[0]
    owner = next(m for m in candidate if m["module"] == row[0])
    original = next(r for r in owner["production_symbols"] if r["path"] == row[1] and r["symbol"] == row[3])
    owner["production_symbols"].append(copy.deepcopy(original))
    errors = []
    contract.validate_authority_recovery_contract(ROOT, candidate, errors, actual_item)
    assert len(errors) == 1 and "binding changed" in errors[0]

MUTATIONS = ((0,
  'N01',
  '.native_coordinator_height_is_current(body)\n                .unwrap_or(false)',
  '.native_coordinator_height_is_current(body)\n                .unwrap_or(true)',
  'Coordinator storage failure cannot admit a body.',
  False),
 (1,
  'N02',
  '.native_coordinator_predecessor_is_current(request)\n                .unwrap_or(false)',
  '.native_coordinator_predecessor_is_current(request)\n                .unwrap_or(true)',
  'Coordinator error cannot admit request.',
  False),
 (1,
  'N03',
  '&& self\n'
  '                .native_coordinator_predecessor_is_current(request)\n'
  '                .unwrap_or(false)',
  '&& true',
  'Coordinator predicate cannot be removed.',
  False),
 (1,
  'N04',
  '&& self.native_control_predecessor_is_current(request)',
  '&& true',
  'Control and coordinator predecessor authorities are distinct.',
  False),
 (2,
  'N06',
  'body.coordinator_lane_incarnation,\n            body.authority_context_height,',
  'body.participant_lane_incarnation,\n            body.authority_context_height,',
  'Route incarnation cannot be substituted.',
  False),
 (2,
  'N07',
  'if pending.contains_key(&(body.coordinator_lane_id, body.coordinator_dataspace_id)) {\n'
  '            return Ok(None);\n'
  '        }',
  'if pending.contains_key(&(body.coordinator_lane_id, body.coordinator_dataspace_id)) {}',
  'Occupied pending route cannot continue to tip discovery.',
  False),
 (2,
  'N08',
  'let pending = self.consensus_storage_read(',
  'let pending = std::convert::identity(',
  'Pending read must use the latching storage error owner.',
  False),
 (3,
  'N09',
  '(height == 0) == descriptor_hash.is_none()',
  'true',
  'Zero/hash geometry cannot disappear.',
  False),
 (3,
  'N10',
  'height.checked_add(1)',
  'Some(height.saturating_add(1))',
  'Height overflow cannot authorize successor.',
  False),
 (4,
  'N11',
  '|| request.body.planned_coordinator_block_height\n'
  '                != request.coordinator_proposal.descriptor.lane_block_height',
  '|| false',
  'Body and descriptor lane heights must agree.',
  False),
 (4,
  'N12',
  'crate::state::LanePredecessorApplicationMode::CurrentTip',
  'crate::state::LanePredecessorApplicationMode::OrdinaryBodyStatePrefix',
  'Producer cannot use replay repair mode.',
  False),
 (4,
  'N13',
  'self.consensus_storage_read(State::lane_block_predecessor_is_applied_for_snapshot(',
  'std::convert::identity(State::lane_block_predecessor_is_applied_for_snapshot(',
  'Delegated corruption must reach restart latch.',
  False),
 (5,
  'N05',
  'self.output_guard.close_admission_for_restart();',
  'let _ = &self.output_guard;',
  'Persistence error must irreversibly close current output admission.',
  False),
 (6,
  'S01',
  '.read_native_amx_participant_application_history(descriptor.lane_id)',
  '.read_native_amx_participant_application_history_unchecked(descriptor.lane_id)',
  'The exact complete occupied-history authentication owner cannot be bypassed; this is a '
  'source-only removed-owner control, not a compilable implementation proposal.',
  False),
 (6,
  'S02',
  'if mode == LanePredecessorApplicationMode::CurrentTip {\n'
  '                    return Ok(false);\n'
  '                }',
  'if false {\n                    return Ok(false);\n                }',
  'CurrentTip cannot skip future occupancy.',
  False),
 (6,
  'S03',
  'if !prefix_contains(application_height, application_hash)',
  'if false',
  'Application height alone is not canonical prefix authority.',
  False),
 (6,
  'S04',
  'if pending || !route_matches',
  'if false',
  'Pending and wrong-route history must reject.',
  False),
 (6,
  'S05',
  '(Some(_), None) | (None, Some(_)) => return Ok(false),',
  '(Some(_), None) | (None, Some(_)) => {},',
  'Missing publication half is not empty.',
  False),
 (6,
  'S06',
  'if !Self::native_amx_participant_receipt_matches_frontier(&receipt, marker)',
  'if false',
  'Exact Native receipt and marker must agree.',
  False),
 (6,
  'S07',
  'mode == LanePredecessorApplicationMode::CurrentTip\n'
  '                        || artifact.ownership.proposal_height <= prefix_height',
  'artifact.ownership.proposal_height <= prefix_height',
  'CurrentTip cannot hide a future ordinary artifact.',
  False),
 (6,
  'S08',
  'Self::lane_block_artifact_matches_certified_proposal(&artifact, &receipt.proposal)\n'
  '                    && prefix_contains(receipt.application_block_height, '
  'receipt.application_block_hash)',
  'prefix_contains(receipt.application_block_height, receipt.application_block_hash)',
  'Exact ordinary artifact identity remains mandatory.',
  False),
 (6,
  'S09',
  'if !state.kura().is_audited_snapshot_import_height(height)',
  'if false',
  'Absent body needs exact audited import authority.',
  False),
 (6,
  'S10',
  'if mode != LanePredecessorApplicationMode::OrdinaryBodyStatePrefix',
  'if false',
  'CurrentTip cannot use ordinary receipt-repair fallback.',
  False),
 (6,
  'S11',
  '        if pending || !route_matches {\n'
  '            return Ok(false);\n'
  '        }\n'
  '        match (native_marker, native_receipt) {\n'
  '            (Some(marker), Some(receipt)) => {\n'
  '                if receipt.participant_proposal.descriptor.lane_block_height\n'
  '                    != marker.lane_block_height\n'
  '                {\n'
  '                    // Durable application and its replicated marker cross\n'
  '                    // separate publication boundaries. Wait for this exact\n'
  '                    // snapshot to catch up instead of inventing an empty tip.\n'
  '                    return Ok(false);\n'
  '                }\n'
  '                if !Self::native_amx_participant_receipt_matches_frontier(&receipt, marker) {\n'
  '                    return Err(MergeLedgerCommitError::ExecutionMarkerConflict(\n'
  '                        "Native AMX snapshot marker conflicts with its authenticated '
  'application receipt".to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '                if receipt.application_block_height >= descriptor.proposal_height {\n'
  '                    return Ok(false);\n'
  '                }\n'
  '                tips.push((marker.lane_block_height, marker.lane_block_descriptor_hash));\n'
  '            }\n'
  '            (None, None) => {}\n'
  '            (Some(_), None) | (None, Some(_)) => return Ok(false),\n'
  '        }\n'
  '        let latest = state\n'
  '            .kura()\n'
  '            .latest_lane_block_artifact_matching(descriptor.lane_id, |artifact| {\n'
  '                artifact.ownership.dataspace_id == descriptor.dataspace_id\n'
  '                    && artifact.ownership.lane_incarnation == descriptor.lane_incarnation\n'
  '                    && (mode == LanePredecessorApplicationMode::CurrentTip\n'
  '                        || artifact.ownership.proposal_height <= prefix_height)\n'
  '            })\n'
  '            .map_err(MergeLedgerCommitError::Persistence)?;',
  '        let latest = state\n'
  '            .kura()\n'
  '            .latest_lane_block_artifact_matching(descriptor.lane_id, |artifact| {\n'
  '                artifact.ownership.dataspace_id == descriptor.dataspace_id\n'
  '                    && artifact.ownership.lane_incarnation == descriptor.lane_incarnation\n'
  '                    && (mode == LanePredecessorApplicationMode::CurrentTip\n'
  '                        || artifact.ownership.proposal_height <= prefix_height)\n'
  '            })\n'
  '            .map_err(MergeLedgerCommitError::Persistence)?;\n'
  '        if pending || !route_matches {\n'
  '            return Ok(false);\n'
  '        }\n'
  '        match (native_marker, native_receipt) {\n'
  '            (Some(marker), Some(receipt)) => {\n'
  '                if receipt.participant_proposal.descriptor.lane_block_height\n'
  '                    != marker.lane_block_height\n'
  '                {\n'
  '                    // Durable application and its replicated marker cross\n'
  '                    // separate publication boundaries. Wait for this exact\n'
  '                    // snapshot to catch up instead of inventing an empty tip.\n'
  '                    return Ok(false);\n'
  '                }\n'
  '                if !Self::native_amx_participant_receipt_matches_frontier(&receipt, marker) {\n'
  '                    return Err(MergeLedgerCommitError::ExecutionMarkerConflict(\n'
  '                        "Native AMX snapshot marker conflicts with its authenticated '
  'application receipt".to_owned(),\n'
  '                    ));\n'
  '                }\n'
  '                if receipt.application_block_height >= descriptor.proposal_height {\n'
  '                    return Ok(false);\n'
  '                }\n'
  '                tips.push((marker.lane_block_height, marker.lane_block_descriptor_hash));\n'
  '            }\n'
  '            (None, None) => {}\n'
  '            (Some(_), None) | (None, Some(_)) => return Ok(false),\n'
  '        }\n',
  'Ordinary Kura selection must follow Native pending and marker/receipt validation; moving the '
  'read ahead violates the source authority order while preserving all individual required tokens.',
  True),
 (6,
  'S12',
  'if height == latest_height && hash != latest_hash =>',
  'if false =>',
  'Conflicting same-height tips must error.',
  False),
 (6,
  'S13',
  'descriptor.previous_lane_block_descriptor_hash == Some(hash)',
  'true',
  'Exact predecessor hash cannot be dropped.',
  False),
 (7,
  'P01',
  'crate::kura::NativeAmxLatestReceiptObservation::PendingTipMetadata(_) => {\n'
  '                // The exact durable frontier is occupied. Owned Apply recovery\n'
  '                // must complete before a new participant slot can be planned.\n'
  '                return Ok(None);',
  'crate::kura::NativeAmxLatestReceiptObservation::PendingTipMetadata(_) => {\n'
  '                // The exact durable frontier is occupied. Owned Apply recovery\n'
  '                // must complete before a new participant slot can be planned.\n'
  '                return Ok(Some((0, None)));',
  'Pending Native index occupancy cannot become empty.',
  True),
 (7,
  'P02',
  '|| latest_receipt.application_block_height >= proposal_height',
  '|| false',
  'Receipt from same/future global height cannot authorize proposal.',
  False),
 (7,
  'P03',
  '} else if matching.is_empty() {\n'
  '        // The skipped latest-receipt index means an apparently empty route is unknown. '
  'Abstain\n'
  '        // instead of synthesizing a height-zero predecessor until Strict rebuilds the index.\n'
  '        return Ok(None);',
  '} else if matching.is_empty() {\n'
  '        // The skipped latest-receipt index means an apparently empty route is unknown. '
  'Abstain\n'
  '        // instead of synthesizing a height-zero predecessor until Strict rebuilds the index.\n'
  '        return Ok(Some((0, None)));',
  'Emergency unknown empty history cannot synthesize genesis.',
  True),
 (8,
  'N06-route',
  'lane_incarnation,\n            proposal_height,',
  'lane_incarnation,\n            self.context.height,',
  'Helper cannot silently replace caller authority height.',
  False),
 (9,
  'A01',
  '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()',
  '.native_amx_participant_frontiers_pending_durable_evidence_snapshot_cached()',
  'The authenticated Native reader cannot be replaced with the retired cached projection.',
  False),
 (9,
  'A02',
  '.map_or(true, |markers|',
  '.map_or(false, |markers|',
  'Storage failure must block drain.',
  False),
 (10,
  'A03',
  '.map_err(MergeLedgerCommitError::Persistence)?;',
  '.unwrap_or_default();',
  'Complete history errors cannot become empty history.',
  False),
 (10,
  'A04',
  'observed_incarnation != incarnation',
  'false',
  'ABA incarnation mismatch cannot pass.',
  False),
 (10,
  'A05',
  'if !marker_matches {',
  'if false {',
  'A conflicting replicated marker cannot authorize Applied.',
  False),
 (10,
  'A06',
  ') || marker.is_none_or(|marker| height > marker.lane_block_height)',
  ') || false',
  'Unmarked or ahead-of-marker durable occupancy must block producers.',
  False),
 (10,
  'A07',
  'snapshot.repair.push(marker);',
  'let _ = marker;',
  'Missing durable marker evidence must retain a repair obligation.',
  False),
 (11,
  'A08',
  'Ok(self.native_amx_participant_application_snapshot()?.repair)',
  'Ok(Vec::new())',
  'Repair projection cannot substitute an empty list.',
  False),
 (12,
  'A09-plan_lane_application_evidence_repair',
  '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
  '        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;',
  '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
  '        .unwrap_or_default();',
  'Startup planning or readback must not turn corrupt history into completed repair.',
  False),
 (13,
  'A09-apply_lane_application_evidence_repair',
  '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
  '        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;',
  '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
  '        .unwrap_or_default();',
  'Startup planning or readback must not turn corrupt history into completed repair.',
  False),
 (14,
  'A10',
  'self.certified_lane_block_predecessor_is_applied_or_snapshot_anchored(\n'
  '                    &session.proposal,\n'
  '                )?',
  'self.certified_lane_block_predecessor_is_applied_or_snapshot_anchored(\n'
  '                    &session.proposal,\n'
  '                ).unwrap_or(true)',
  'Corrupt predecessor evidence cannot terminate the repair walk.',
  False),
 (15,
  'A11',
  'if session.prepare_qc.payload_availability_qc.is_some() {',
  'if false {',
  'Autonomous evidence cannot enter the ordinary hash-only anchor branch.',
  False),
 (16,
  'A12',
  'let native_snapshot = self.native_amx_participant_application_snapshot()?;',
  'let native_snapshot = self.native_amx_participant_application_snapshot().unwrap_or_default();',
  'Read error cannot authorize a genesis predecessor.',
  False),
 (17,
  'A13',
  'receipt.proposal == *proposal',
  'true',
  'A different receipt cannot authenticate a candidate.',
  False),
 (18,
  'A14',
  'if !Self::lane_block_artifact_matches_certified_proposal(&artifact, proposal)',
  'if false',
  'An unrelated ownership record is not an authenticated anchor.',
  False),
 (19,
  'A15',
  'receipt.application_block_height < descriptor.proposal_height',
  'true',
  'Same or future Native application cannot authenticate a predecessor.',
  False),
 (20,
  'A16',
  'receipt.proposal == *proposal',
  'true',
  'Autonomous global Applied authority requires exact proposal.',
  False),
 (22,
  'A17',
  '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
  '            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;',
  '.native_amx_participant_frontiers_pending_durable_evidence_snapshot()\n'
  '            .unwrap_or_default();',
  'Admission cannot treat failed Native readback as no obligations.',
  False),
 (23,
  'C01',
  'let Some(effect) = recovery.next_effect() else {',
  'let Some(effect) = recovery.drain_effects(1).pop() else {',
  'Dispatch cannot dequeue before retention or service acceptance.',
  False),
 (23,
  'C02',
  'LaneWorkEffectDispatch::SourceRetained(_) => break,',
  'LaneWorkEffectDispatch::SourceRetained(_) => { recovery.drain_effects(1); break; },',
  'Backpressure must retain its sole source owner.',
  False),
 (24,
  'C03',
  'self.effects.front().cloned()',
  'self.effects.pop_front()',
  'Peek must not consume the source.',
  False),
 (25,
  'C04',
  'peer == &outstanding.responder.peer',
  'true',
  'Current request must bind the responder.',
  False),
 (26,
  'C05',
  'self.available_proposal_for_vote_body(&qc.body)?',
  'self.available_proposal_for_vote_body(&qc.body).unwrap_or(None)',
  'Unreadable historical evidence cannot become absent work.',
  False),
 (27,
  'C06',
  'self.canonical_proposal_for_vote_body(body)',
  'Ok(None)',
  'Canonical recovery cannot disappear behind missing session state.',
  False),
 (28,
  'C07',
  '.read_current_autonomous_lane_block_artifact(',
  '.read_autonomous_lane_block_artifact(',
  'Current-pointer authentication cannot be replaced by the obsolete convenience reader.',
  False),
 (28,
  'C08',
  '.read_lane_completion_certificate(',
  '.read_certified_lane_block_artifact(',
  'Completion requires the live durability-attesting reader.',
  False),
 (28,
  'C08-pending',
  'return Err(slot_error("awaits exact Native AMX application metadata"));',
  'return Ok(exact_current_slot);',
  'Pending Native metadata cannot authorize payload admission.',
  False),
 (29,
  'C09',
  'self.consensus_storage_read(canonical_recovery)',
  'Ok::<bool, V2LaneWorkError>(canonical_recovery.unwrap_or(true))',
  'A failed canonical read cannot authorize or mutate locked-body ownership.',
  False),
 (30,
  'K01',
  'lane_block_height,\n            true,',
  'lane_block_height,\n            false,',
  'Completion cannot skip durability attestation.',
  False),
 (31,
  'K02',
  'self.ensure_prune_recovery_not_required()?;',
  'let _ = ();',
  'Read must reject pending prune recovery.',
  False),
 (32,
  'K03',
  'pointer.lane_incarnation != marker.0',
  'false',
  'Current reader cannot accept stale incarnation.',
  False),
 (32,
  'K04',
  'AutonomousLaneBlockViewStateReadMode::LatestReadOnly',
  'AutonomousLaneBlockViewStateReadMode::Latest',
  'A consensus read must not invoke write-recovery mode.',
  False),
 (32,
  'K05',
  'record.retirement.is_none().then_some(record.artifact)',
  'Some(record.artifact)',
  'Retired attempt cannot become a live current artifact.',
  False),
 (33,
  'K06',
  'let mut history = self.read_native_amx_participant_application_history(lane_id)?;',
  'let mut history = '
  'self.read_native_amx_participant_application_history(lane_id).unwrap_or_default();',
  'Corrupt occupied history cannot become empty latest.',
  False),
 (34,
  'K07',
  '|| !Self::progress_mutation_namespace_unchanged(&namespace)',
  '|| false',
  'History must reject namespace replacement across the body-read gap.',
  False),
 (34,
  'K08',
  'if retained_receipts.get(height) != Some(&confirmed)',
  'if false',
  'Retained receipt bytes must be compared after lock reacquisition.',
  False),
 (34,
  'K09',
  'if metadata.get(height) != Some(&confirmed)',
  'if false',
  'Canonical metadata changes cannot retain Applied authority.',
  False))

@pytest.mark.parametrize("case", MUTATIONS, ids=lambda c:c[1])
def test_semantic_mutations_rejected_after_full_provider_rehash(tmp_path, case):
    index, name, old, new, reason, order_only = case
    binding = BINDINGS[index]
    item = source_item(binding)
    assert item.count(old) == 1, reason
    post = item.replace(old, new, 1)
    changed_provider(tmp_path, binding, item, post)
    _module, path, kind, symbol, required, _ordered = binding
    extraction_errors = []
    mutated = actual_item(tmp_path, path, kind, symbol, name, extraction_errors)
    assert extraction_errors == [] and mutated is not None
    assert hashlib.sha256(mutated.encode()).hexdigest() == hashlib.sha256(post.encode()).hexdigest()
    if order_only:
        assert all(token in mutated for token in required), reason
    errors = []
    contract.validate_authority_recovery_item(mutated, binding, errors)
    assert errors, reason
    assert all("authority/recovery item" in error for error in errors)
    assert not any("sha256" in error or "digest" in error for error in errors)
    if order_only:
        assert any("violates order" in error for error in errors), reason
    (tmp_path / "semantic-rejection.json").write_text(json.dumps({"id":name,"reason":reason,"errors":errors}))


@pytest.mark.parametrize("remove_owner", [False, True])
def test_checker_dispatch_reaches_actual_authority_contract(monkeypatch, tmp_path, remove_owner):
    """Exercise this main-checker dispatch seam, not unrelated formal gates."""
    class ReachedAuthorityContract(Exception):
        pass

    ledger = json.loads((ROOT / "formal/sumeragi_v2/multilane_source_bindings.json").read_text())
    binding = BINDINGS[0]
    if remove_owner:
        owner = next(model for model in ledger["models"] if model["module"] == binding[0])
        owner["production_symbols"] = [row for row in owner["production_symbols"]
            if (row["path"], row["kind"], row["symbol"]) != binding[1:4]]
    destination = tmp_path / "formal/sumeragi_v2/multilane_source_bindings.json"
    destination.parent.mkdir(parents=True)
    destination.write_text(json.dumps(ledger))
    for name in ("_validate_reviewed_rust_include_manifest", "_validate_model",
                 "_validate_kura_replica_retention_contract",
                 "validate_autonomous_terminal_recovery_contract"):
        monkeypatch.setattr(checker, name, lambda *args: None)
    monkeypatch.setattr(checker, "_rust_binding_item",
        lambda _root, *args: actual_item(ROOT, *args))
    real_validator = contract.validate_authority_recovery_contract
    observed = []

    def capture_actual_validator(root, declared, errors, extractor):
        real_validator(root, declared, errors, extractor)
        observed.extend(errors)
        raise ReachedAuthorityContract

    monkeypatch.setattr(checker.authority_recovery_contract,
        "validate_authority_recovery_contract", capture_actual_validator)
    with pytest.raises(ReachedAuthorityContract):
        checker._validate(tmp_path)
    if remove_owner:
        assert observed == [f"{binding[0]}: authority/recovery binding changed for {binding[1]}!{binding[3]}"]
    else:
        assert observed == []


def test_source_manifest_registers_authority_contract_and_controls(monkeypatch):
    """Inspect the real manifest's assembled paths before its unrelated expansion."""
    class CapturedManifestInventory(Exception):
        pass

    observed = []

    def capture(paths):
        observed.extend(paths)
        raise CapturedManifestInventory

    monkeypatch.setattr(checker, "_expanded_source_manifest_paths", capture)
    with pytest.raises(CapturedManifestInventory):
        checker.source_manifest_sha256(ROOT)
    assert Path("scripts/formal/sumeragi_v2_multilane_authority_recovery_contract.py") in observed
    assert Path("pytests/scripts/sumeragi_v2_multilane_authority_recovery_test.py") in observed


ACTION_REGRESSIONS = ({'id': 'A18-blocked-insertion',
  'symbol': 'native_amx_participant_application_snapshot',
  'old': 'snapshot\n'
         '                        .blocked\n'
         '                        .entry(route)\n'
         '                        .and_modify(|blocked| *blocked = (*blocked).max(height))\n'
         '                        .or_insert(height);',
  'new': 'let _ = (route, height);'},
 {'id': 'A19-applied-projection',
  'symbol': 'native_amx_participant_application_snapshot',
  'old': 'snapshot.applied.push(marker);',
  'new': 'let _ = marker;'},
 {'id': 'A20-blocked-predecessor',
  'symbol': 'State::certified_lane_block_predecessor_is_applied_or_snapshot_anchored',
  'old': 'if native_snapshot\n'
         '            .blocked\n'
         '            .contains_key(&(descriptor.lane_id, descriptor.dataspace_id))\n'
         '        {\n'
         '            return Ok(false);\n'
         '        }',
  'new': 'if native_snapshot\n'
         '            .blocked\n'
         '            .contains_key(&(descriptor.lane_id, descriptor.dataspace_id))\n'
         '        {\n'
         '            return Ok(true);\n'
         '        }'})


@pytest.mark.parametrize("control", ACTION_REGRESSIONS, ids=lambda row:row["id"])
def test_exact_authority_projection_actions_reject_reviewed_full_provider_mutants(tmp_path, control):
    """Keep the reviewed pending, Applied and predecessor action failures closed."""
    binding = next(row for row in BINDINGS if row[3] == control["symbol"])
    original = source_item(binding)
    assert original.count(control["old"]) == 1
    changed = original.replace(control["old"], control["new"], 1)
    provider = changed_provider(tmp_path, binding, original, changed)
    full_provider_sha256 = hashlib.sha256(provider.read_bytes()).hexdigest()
    assert full_provider_sha256 != hashlib.sha256((ROOT / binding[1]).read_bytes()).hexdigest()
    _module, path, kind, symbol, _required, _ordered = binding
    extraction_errors = []
    mutated = actual_item(tmp_path, path, kind, symbol, control["id"], extraction_errors)
    assert extraction_errors == [] and mutated == changed
    errors = []
    contract.validate_authority_recovery_item(mutated, binding, errors)
    assert errors and all("authority/recovery item" in error for error in errors)
    assert any(symbol in error for error in errors)
    (tmp_path / "semantic-rejection.json").write_text(json.dumps(
        {"id": control["id"], "errors": errors, "full_provider_sha256": full_provider_sha256}))
