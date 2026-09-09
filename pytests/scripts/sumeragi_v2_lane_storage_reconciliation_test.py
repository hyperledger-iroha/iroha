"""Independent rehashed-source controls for durable lane and retry ownership."""
from __future__ import annotations

import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sys

import pytest

REPO = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "lane_storage_reconciliation_checker",
    REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py",
)
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
assert SPEC.loader is not None
SPEC.loader.exec_module(CHECKER)
RULES = CHECKER._LANE_STORAGE_RECONCILIATION_RULES
EXPECTED_KEYS = ('lock_error_order', 'construction_exact_applied_tip', 'retained_handoff_error', 'hydrate_exact_receipt', 'hydrate_published_finality', 'hydrate_no_current_unpublished', 'hydrate_strict_chain', 'predecessor_authority', 'effect_preflight_authority', 'retransmission_error_order', 'historical_exact_completion', 'rollover_pending_error', 'rollover_anchor_error', 'certificate_reconstruction', 'anchored_hydrate_collect', 'anchored_role_observer', 'finalized_preflight', 'finalized_rollover', 'historical_tick', 'dispatch_exact_owner', 'dispatch_restart', 'storage_fail_stop', 'certificate_transport_fallback', 'rotation_preserves_owner', 'catalog_exact_family', 'application_exact_receipt', 'application_receipt_slot', 'finalized_exact_proposal', 'autonomous_applied_authority', 'kura_read_completion', 'kura_read_application', 'kura_recovery_publication')
MUTATIONS = (
    {'key': 'lock_error_order', 'label': 'lock ignores purge error', 'old': 'self.purge_queued_global_body_effects_except_committed_outputs()?;', 'new': 'let _ = self.purge_queued_global_body_effects_except_committed_outputs();', 'count': 1},
    {'key': 'lock_error_order', 'label': 'lock ignores scheduling error', 'old': 'self.schedule_committed_lane_outputs()?;', 'new': 'let _ = self.schedule_committed_lane_outputs();', 'count': 1},
    {'key': 'construction_exact_applied_tip', 'label': 'post Apply rechecks successor policy', 'old': 'if is_pre_apply\n            && !is_fresh_genesis_pre_apply', 'new': 'if !is_fresh_genesis_pre_apply', 'count': 1},
    {'key': 'construction_exact_applied_tip', 'label': 'post Apply loses exact WAL authority', 'old': 'if (is_post_apply || recovered_applied_height.is_some()) && !recovered_applied_tip_matches {', 'new': 'if false {', 'count': 1},
    {'key': 'construction_exact_applied_tip', 'label': 'pre Apply bypasses execution policy', 'old': 'if is_pre_apply\n            && !is_fresh_genesis_pre_apply', 'new': 'if false\n            && !is_fresh_genesis_pre_apply', 'count': 1},
    {'key': 'retained_handoff_error', 'label': 'handoff treats unreadable as empty', 'old': 'self.has_pending_committed_output_handoff()?', 'new': 'self.has_pending_committed_output_handoff().unwrap_or(false)', 'count': 1},
    {'key': 'hydrate_exact_receipt', 'label': 'hydrate swallows receipt failure', 'old': 'self.lane_application_receipt_available(proposal)?', 'new': 'self.lane_application_receipt_available(proposal).unwrap_or(false)', 'count': 2},
    {'key': 'hydrate_published_finality', 'label': 'hydrate repairs before finality', 'old': 'if current_finality_published {', 'new': 'if true {', 'count': 1},
    {'key': 'hydrate_no_current_unpublished', 'label': 'hydrate consumes unpublished current ownership', 'old': 'if ownership.proposal_height == self.context.height && !current_finality_published {', 'new': 'if false {', 'count': 1},
    {'key': 'hydrate_strict_chain', 'label': 'hydrate ignores exact ownership multiplicity', 'old': 'if canonical.as_slice() != [artifact.clone()] {', 'new': 'if false {', 'count': 1},
    {'key': 'predecessor_authority', 'label': 'autonomous predecessor borrows ordinary proof', 'old': 'certified_autonomous_lane_block_predecessor_is_globally_applied(proposal)', 'new': 'certified_lane_block_predecessor_is_applied_or_snapshot_anchored(proposal)', 'count': 1},
    {'key': 'predecessor_authority', 'label': 'predecessor error leaves output open', 'old': 'self.output_guard.close_admission_for_restart();', 'new': '', 'count': 1},
    {'key': 'effect_preflight_authority', 'label': 'fresh vote borrows certificate recovery fallback', 'old': 'self.outbound_lane_message_predecessor_is_ready(message)', 'new': 'Ok(true)', 'count': 1},
    {'key': 'effect_preflight_authority', 'label': 'certificate response wrongly requires fresh predecessor', 'old': 'self.durable_lane_certificate_source_is_ready(&certificate.proposal)', 'new': 'self.proposal_predecessor_is_ready_for_progress(&certificate.proposal)', 'count': 1},
    {'key': 'effect_preflight_authority', 'label': 'preflight error leaves output open', 'old': 'self.output_guard.close_admission_for_restart();', 'new': '', 'count': 1},
    {'key': 'effect_preflight_authority', 'label': 'preflight ignores restart latch', 'old': 'if self.output_guard.restart_required() {', 'new': 'if false {', 'count': 1},
    {'key': 'retransmission_error_order', 'label': 'retransmission swallows collection error', 'old': 'self.collect_committed_lane_sessions()?;', 'new': 'let _ = self.collect_committed_lane_sessions();', 'count': 1},
    {'key': 'historical_exact_completion', 'label': 'historical source loses exact proposal', 'old': 'if durable.proposal.proposal_hash != proposal_hash {', 'new': 'if false {', 'count': 1},
    {'key': 'historical_exact_completion', 'label': 'historical source loses exact receipt', 'old': 'if receipt.proposal != durable.proposal {', 'new': 'if false {', 'count': 1},
    {'key': 'rollover_pending_error', 'label': 'rollover treats unreadable as no recovery', 'old': 'self.has_pending_historical_recovery()?', 'new': 'self.has_pending_historical_recovery().unwrap_or(false)', 'count': 1},
    {'key': 'rollover_anchor_error', 'label': 'rollover hides autonomous anchor failure', 'old': 'self.canonical_autonomous_anchor_matches_kura(proposal)?', 'new': 'self.canonical_autonomous_anchor_matches_kura(proposal).unwrap_or(false)', 'count': 1},
    {'key': 'certificate_reconstruction', 'label': 'reconstruction bypasses source authority', 'old': 'if !self\n            .durable_lane_certificate_source_is_ready(proposal)', 'new': 'if false && !self\n            .durable_lane_certificate_source_is_ready(proposal)', 'count': 1},
    {'key': 'certificate_reconstruction', 'label': 'reconstruction borrows another proposal', 'old': 'if artifact.proposal != *proposal {', 'new': 'if false {', 'count': 1},
    {'key': 'anchored_hydrate_collect', 'label': 'anchored collection precedes hydration', 'old': 'self.hydrate_canonical_lane_artifacts()?;\n        self.collect_committed_lane_sessions()?;', 'new': 'self.collect_committed_lane_sessions()?;\n        self.hydrate_canonical_lane_artifacts()?;', 'count': 1},
    {'key': 'anchored_role_observer', 'label': 'observer uses committee lifecycle', 'old': 'if autonomous_certificate && !self.local_can_own_autonomous_payload(&session.proposal) {', 'new': 'if false {', 'count': 1},
    {'key': 'anchored_role_observer', 'label': 'observer borrows another proposal', 'old': 'replica.bundle.executable_payload().origin_proposal != session.proposal', 'new': 'false', 'count': 1},
    {'key': 'finalized_preflight', 'label': 'finalized preflight hides recovery failure', 'old': 'lane_work.has_pending_historical_recovery()?', 'new': 'lane_work.has_pending_historical_recovery().unwrap_or(false)', 'count': 1},
    {'key': 'finalized_rollover', 'label': 'finalized rollover hides recovery failure', 'old': 'lane_work.has_pending_historical_recovery()?', 'new': 'lane_work.has_pending_historical_recovery().unwrap_or(false)', 'count': 1},
    {'key': 'finalized_rollover', 'label': 'finalized rollover hides handoff failure', 'old': 'lane_work.has_pending_committed_output_handoff()?', 'new': 'lane_work.has_pending_committed_output_handoff().unwrap_or(false)', 'count': 1},
    {'key': 'historical_tick', 'label': 'archive tick hides recovery failure', 'old': '.has_pending_historical_recovery()?', 'new': '.has_pending_historical_recovery().unwrap_or(false)', 'count': 1},
    {'key': 'dispatch_exact_owner', 'label': 'dispatch loses original before fallible transfer', 'old': 'match dispatch_lane_work_effect_from_snapshot(', 'new': 'let _ = require_peeked_lane_work_effect(lane_work.drain_effects(1).pop())?;\n        match dispatch_lane_work_effect_from_snapshot(', 'count': 1},
    {'key': 'dispatch_exact_owner', 'label': 'dispatch retained owner repeats fresh admission', 'old': 'lane_work.rotate_next_effect()', 'new': 'lane_work.drain_effects(1).pop().is_some()', 'count': 2},
    {'key': 'dispatch_exact_owner', 'label': 'dispatch success retains duplicate original', 'old': 'LaneWorkEffectDispatch::Complete => {\n                let _ = require_peeked_lane_work_effect(lane_work.drain_effects(1).pop())?;', 'new': 'LaneWorkEffectDispatch::Complete => {', 'count': 1},
    {'key': 'dispatch_restart', 'label': 'dispatch ignores service restart', 'old': 'if services.lifecycle_output_guard().restart_required() {', 'new': 'if false {', 'count': 1},
    {'key': 'storage_fail_stop', 'label': 'storage error does not close admission', 'old': 'self.output_guard.close_admission_for_restart();', 'new': '', 'count': 1},
    {'key': 'certificate_transport_fallback', 'label': 'recovery bypasses exact global application', 'old': 'if !self.consensus_storage_read(', 'new': 'if false && !self.consensus_storage_read(', 'count': 1},
    {'key': 'certificate_transport_fallback', 'label': 'recovery treats absent carrier as authorized', 'old': '.is_some())', 'new': '.is_none())', 'count': 1},
    {'key': 'rotation_preserves_owner', 'label': 'rotation drops original ingress owner', 'old': 'self.effects.push_back(effect);', 'new': 'drop(effect);', 'count': 1},
    {'key': 'catalog_exact_family', 'label': 'catalog treats reply source as recreatable', 'old': 'reply_routes: None,', 'new': 'reply_routes: Some(_),', 'count': 1},
    {'key': 'application_exact_receipt', 'label': 'receipt check accepts another proposal', 'old': 'receipt.proposal == *proposal', 'new': 'true', 'count': 1},
    {'key': 'finalized_exact_proposal', 'label': 'public carrier accepts another proposal', 'old': '.filter(|payload| payload.origin_proposal == *proposal)', 'new': '', 'count': 1},
    {'key': 'autonomous_applied_authority', 'label': 'ordinary receipt authorizes autonomous execution', 'old': 'crate::kura::LaneBlockApplicationReceiptArtifactFormat::MergeExecution', 'new': 'crate::kura::LaneBlockApplicationReceiptArtifactFormat::Current', 'count': 1},
    {'key': 'kura_read_completion', 'label': 'completion lookup skips durability reattestation', 'old': '            true,', 'new': '            false,', 'count': 1},
    {'key': 'kura_read_application', 'label': 'receipt lookup skips durability reattestation', 'old': 'self.read_lane_application_receipt_under_guards(lane_id, lane_block_height, true)', 'new': 'self.read_lane_application_receipt_under_guards(lane_id, lane_block_height, false)', 'count': 1},
    {'key': 'kura_recovery_publication', 'label': 'recovery suppresses exact publication error', 'old': 'self.recover_exact_canonical_lane_artifact(artifact)?;', 'new': 'let _ = self.recover_exact_canonical_lane_artifact(artifact);', 'count': 1},
 )


@pytest.fixture(scope="module")
def sources():
    return {row["path"]: (REPO / row["path"]).read_text() for row in RULES.values()}


def test_complete_owner_inventory_and_checker_hook():
    assert tuple(RULES) == EXPECTED_KEYS
    source = (REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py").read_text()
    functions = [node for node in ast.parse(source).body if isinstance(node, ast.FunctionDef)
                 and node.name == "_exact_output_production_source_fidelity_errors"]
    assert len(functions) == 1
    calls = [node for node in ast.walk(functions[0]) if isinstance(node, ast.Call)
             and isinstance(node.func, ast.Name)
             and node.func.id == "_lane_storage_reconciled_source_fidelity_errors"]
    assert len(calls) == 1
    assert len(calls[0].args) == 1 and isinstance(calls[0].args[0], ast.Name)
    assert calls[0].args[0].id == "repo_root"


@pytest.mark.parametrize("key", EXPECTED_KEYS)
def test_actual_owner_baseline(key, sources):
    row = RULES[key]
    assert CHECKER._lane_storage_reconciled_owner_errors(
        key, REPO / row["path"], sources[row["path"]]) == []


@pytest.mark.parametrize("mutation", MUTATIONS, ids=[row["label"] for row in MUTATIONS])
def test_owner_mutation_is_rejected_after_its_digest_refresh(mutation, sources, tmp_path):
    key = mutation["key"]
    row = RULES[key]
    path = tmp_path / row["path"]
    path.parent.mkdir(parents=True, exist_ok=True)
    source = sources[row["path"]]
    path.write_text(source)
    assert CHECKER._lane_storage_reconciled_owner_errors(key, path, source) == []
    items = CHECKER.rust_items(source, row["name"])
    assert len(items) == 1
    item = items[0]
    assert item.source.count(mutation["old"]) == mutation["count"]
    modified = item.source.replace(mutation["old"], mutation["new"])
    assert modified != item.source and source.count(item.source) == 1
    path.write_text(source.replace(item.source, modified, 1))
    after = path.read_text()
    changed = CHECKER.rust_items(after, row["name"])
    assert len(changed) == 1
    digest = CHECKER._rust_item_token_sha256(changed[0])
    assert digest != CHECKER._rust_item_token_sha256(item)
    seal_errors = []
    CHECKER._require_rust_item_token_sha256(path, changed[0], digest, "refreshed mutation", seal_errors)
    assert seal_errors == []
    failures = CHECKER._lane_storage_reconciled_owner_errors(key, path, after)
    (tmp_path / "rehashed-control.json").write_text(json.dumps({
        "key": key, "mutation": mutation,
        "original_file_sha256": hashlib.sha256(source.encode()).hexdigest(),
        "mutated_file_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "refreshed_owner_sha256": digest, "seal_errors": seal_errors, "errors": failures,
    }, indent=2) + "\n")
    assert failures and any(f"lane storage reconciliation {key}" in error for error in failures)
    assert not any("item SHA-256" in error for error in failures)


@pytest.mark.parametrize("mode", ("missing", "duplicate", "disabled"))
def test_real_owner_cannot_be_missing_duplicated_or_disabled(mode, sources, tmp_path):
    key = "certificate_transport_fallback"
    row = RULES[key]
    source = sources[row["path"]]
    item = CHECKER.rust_items(source, row["name"])[0]
    replacement = {"missing": "", "duplicate": item.source + "\n" + item.source,
                   "disabled": "#[cfg(any())]\n" + item.source}[mode]
    source = source.replace(item.source, replacement, 1)
    path = tmp_path / row["path"]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(source)
    assert CHECKER._lane_storage_reconciled_owner_errors(key, path, source)


RECONCILED_SOURCE_SEALS = (('_PRODUCTION_EXACT_OUTPUT_ITEM_SHA256', 'autonomous_lane_output_has_durable_reconstruction_source', 'crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs', 'autonomous_lane_output_has_durable_reconstruction_source'), ('_APPLIED_HEIGHT_PREDECESSOR_DURABILITY_HANDOFF_TEST_SHA256', 'applied_height_handoff_accepts_kura_applied_ordinary_historical_lane_output', 'crates/iroha_core/src/sumeragi/v2_worker/applied_height_handoff_tests.rs', 'applied_height_handoff_accepts_kura_applied_ordinary_historical_lane_output'), ('_APPLIED_HEIGHT_PREDECESSOR_DURABILITY_HANDOFF_TEST_SHA256', 'applied_height_handoff_accepts_record_backed_autonomous_historical_lane_certificate', 'crates/iroha_core/src/sumeragi/v2_worker/applied_height_handoff_tests.rs', 'applied_height_handoff_accepts_record_backed_autonomous_historical_lane_certificate'), ('_PRODUCTION_MERGE_EXECUTION_CACHE_ITEM_SHA256', 'V2LaneWorkAdapter::mark_global_body_locked', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'mark_global_body_locked'), ('_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256', 'V2LaneWorkAdapter::new_with_output_guard_and_transport_inner', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'new_with_output_guard_and_transport_inner'), ('_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256', 'V2LaneWorkAdapter::into_retained_merge_sidecars', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'into_retained_merge_sidecars'), ('_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256', 'V2LaneWorkAdapter::hydrate_canonical_lane_artifacts', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'hydrate_canonical_lane_artifacts'), ('_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256', 'V2LaneWorkAdapter::proposal_predecessor_is_ready_for_progress', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'proposal_predecessor_is_ready_for_progress'), ('_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256', 'V2LaneWorkAdapter::preflight_effect_insertion', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'preflight_effect_insertion'), ('_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256', 'V2LaneWorkAdapter::schedule_retransmission_at', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'schedule_retransmission_at'), ('_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256', 'preflight_finalized_lane_rollover', 'crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs', 'preflight_finalized_lane_rollover'), ('_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256', 'rollover_finalized_height_outputs', 'crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs', 'rollover_finalized_height_outputs'), ('_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256', 'service_historical_recovery_tick', 'crates/iroha_core/src/sumeragi/v2_runner/canonical_recovery_ingress.rs', 'service_historical_recovery_tick'), ('_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256', 'dispatch_lane_work_effects_with_progress', 'crates/iroha_core/src/sumeragi/v2_runner.rs', 'dispatch_lane_work_effects_with_progress'), ('_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256', 'dispatch_lane_work_effect_from_snapshot', 'crates/iroha_core/src/sumeragi/v2_runner.rs', 'dispatch_lane_work_effect_from_snapshot'), ('_PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256', 'durable_historical_lane_output_source_hash', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'durable_historical_lane_output_source_hash'), ('_PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256', 'durable_lane_rollover_authority', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'durable_lane_rollover_authority'), ('_PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256', 'reconstruct_durable_lane_certificate', 'crates/iroha_core/src/sumeragi/v2_lane_work.rs', 'reconstruct_durable_lane_certificate'), ('_PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256', 'preflight_finalized_lane_rollover', 'crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs', 'preflight_finalized_lane_rollover'), ('_PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256', 'rollover_finalized_height_outputs', 'crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs', 'rollover_finalized_height_outputs'), ('_PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256', 'dispatch_lane_work_effects_with_progress', 'crates/iroha_core/src/sumeragi/v2_runner.rs', 'dispatch_lane_work_effects_with_progress'), ('_PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256', 'dispatch_lane_work_effect_from_snapshot', 'crates/iroha_core/src/sumeragi/v2_runner.rs', 'dispatch_lane_work_effect_from_snapshot'), ('_PRODUCTION_EXACT_OUTPUT_INGRESS_SEAM_ITEM_SHA256', 'worker::sweep_buffered_payload_chunk_lifecycles', 'crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs', 'sweep_buffered_payload_chunk_lifecycles'))


def test_all_reviewed_source_seals_and_duplicate_registrations_match_real_owners():
    assert len(RECONCILED_SOURCE_SEALS) == 23
    assert len({row[3] for row in RECONCILED_SOURCE_SEALS}) == 19
    for mapping, key, relative, name in RECONCILED_SOURCE_SEALS:
        items = CHECKER.rust_items((REPO / relative).read_text(), name)
        assert len(items) == 1
        independent = hashlib.sha256("\0".join(CHECKER.rust_code_tokens(items[0].source)).encode()).hexdigest()
        assert getattr(CHECKER, mapping)[key] == independent
