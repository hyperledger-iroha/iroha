"""Focused controls for passive diagnostics and bounded historical retry."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


SUPPORT_PATH = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")


def load_support():
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_multilane_models_test_support", SUPPORT_PATH
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def copy_fixture(tmp_path: Path, support, module) -> list[dict]:
    models = support.canonical_models()
    relatives = {
        Path(relative)
        for _, relative, _, _, _ in (
            module.passive_recovery_contract.PASSIVE_RECOVERY_MODEL_BINDINGS
        )
    }
    relatives.update(
        Path(relative)
        for relative, _ in (
            module.passive_recovery_contract.PASSIVE_RECOVERY_INCLUDE_RELATIONS
        )
    )
    relatives.update(
        Path(relative)
        for relative, _, _ in (
            module.passive_recovery_contract.PASSIVE_RECOVERY_RAW_TEST_CHECKS
        )
    )
    support.copy_reviewed_source_fixture_with_includes(
        tmp_path, module, relatives
    )
    return models


def validate_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    with module._reviewed_rust_source_cache():
        module.passive_recovery_contract.validate_passive_recovery_contract(
            tmp_path, models, errors, module._rust_binding_item
        )
    return tuple(errors)


def test_passive_recovery_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    assert validate_fixture(tmp_path, module, models) == ()


def test_passive_recovery_contract_rejects_unbound_nested_kura_provider(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once(
        tmp_path / "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
        'include!("passive_diagnostic_reads.rs");',
        'include!("unreviewed_passive_diagnostic_reads.rs");',
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any("passive provider include" in error for error in errors), errors


def test_passive_recovery_contract_rejects_repairing_state_projection(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once(
        tmp_path
        / "crates/iroha_core/src/state/passive_lane_diagnostic_methods.rs",
        "self.kura.lane_block_payload_is_recoverable(proposal)",
        "self.kura.recover_lane_block_payload(proposal).is_ok()",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "durable_lane_diagnostic_execution_status" in error
        and ("repair-capable" in error or "lane_block_payload_is_recoverable" in error)
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_duplicate_externalized_state_provider(
    tmp_path: Path,
) -> None:
    """A duplicated reviewed child declaration fails without any digest seal."""

    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/state/passive_lane_diagnostic_methods.rs"
    )
    source = path.read_text(encoding="utf-8")
    symbol = "durable_lane_diagnostic_execution_status"
    items = module._extract_rust_binding_items(source, "fn", symbol)
    assert len(items) == 1
    duplicate = items[0] + "\n" + items[0]
    path.write_text(source.replace(items[0], duplicate, 1), encoding="utf-8")

    errors = validate_fixture(tmp_path, module, models)

    assert any(
        symbol in error and "must have one fn declaration, found 2" in error
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_repairing_torii_projection(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path / "crates/iroha_torii/src/routing.rs",
        "pub async fn handle_v1_sumeragi_diagnostics(",
        ".durable_lane_diagnostics()",
        ".recover_lane_block_payload()",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "handle_v1_sumeragi_diagnostics" in error
        and ("repair-capable" in error or "durable_lane_diagnostics" in error)
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_deadline_before_local_check(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.swap_ordered_once_after(
        tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "fn service_next_historical_recovery_at_with_archive_targets(",
        "self.persist_historical_recovery_session(&session)",
        "self.schedule_historical_recovery_request(",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "service_next_historical_recovery_at_with_archive_targets" in error
        and "missing or reorders" in error
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_reason_or_request_reset_drift(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "fn schedule_historical_recovery_request(",
        "existing.cadence.reason == observation.reason",
        "existing.cadence.reason == existing.cadence.reason",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "schedule_historical_recovery_request" in error
        and "observation.reason" in error
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_prior_deadline_anchoring(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "fn after_retained_attempt(",
        "now.checked_add(delay)",
        "self.next_retry_at.checked_add(delay)",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "after_retained_attempt" in error and "now.checked_add(delay)" in error
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_unsigned_retry_bounds(
    tmp_path: Path,
) -> None:
    cases = (
        (
            "ordinary",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "run_non_pending_lifecycle_loop",
        ),
        (
            "pending-kura",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "run_pending_kura_lifecycle_height",
        ),
    )
    for fixture_name, relative, symbol in cases:
        fixture = tmp_path / fixture_name
        support = load_support()
        module = support.load_checker()
        models = copy_fixture(fixture, support, module)
        support.swap_ordered_once_after(
            fixture / relative,
            "let lane_work_limits = lane_work_limits(",
            "retransmit_interval",
            "round_timeout",
        )
        errors = validate_fixture(fixture, module, models)
        assert any(
            symbol in error
            and ("missing or reorders" in error or "source-bound token" in error)
            for error in errors
        ), errors


@pytest.mark.parametrize("pending,occurrence", [(False, 0), (False, 1), (True, 0), (True, 1)])
def test_passive_recovery_contract_rejects_missing_quiet_tick_branch(
    tmp_path: Path, pending: bool, occurrence: int,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    assert validate_fixture(tmp_path, module, models) == ()
    relative, symbol, now = (
        ("lifecycle_pending_kura.rs", "run_pending_active_height", "Instant::now()")
        if pending else ("lifecycle_run_inner.rs", "run_lifecycle_active_height", "now")
    )
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner" / relative
    source = path.read_text(encoding="utf-8")
    item, = module._extract_rust_binding_items(source, "fn", symbol)
    token = f"native.poll(native_global, native_network, {now}, receiver)?;"
    assert item.count(token) == 2
    offset = item.index(token) if occurrence == 0 else item.rindex(token)
    mutated = item[:offset] + item[offset:].replace(token, "skip_native_poll();", 1)
    path.write_text(source.replace(item, mutated, 1), encoding="utf-8")
    errors = validate_fixture(tmp_path, module, models)
    assert any(symbol in error and "Native quiet-loop prefix" in error for error in errors), errors


def test_passive_recovery_contract_rejects_state_control_without_explicit_repair(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path
        / "crates/iroha_core/src/state/"
        "autonomous_merge_and_queue_plan_native_diagnostic_tests.rs",
        "fn assert_passive_state_diagnostics(",
        "kura.recover_lane_block_payload(&session.proposal)",
        "kura.skip_lane_block_payload_recovery(&session.proposal)",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "assert_passive_state_diagnostics" in error
        and "recover_lane_block_payload" in error
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_torii_control_without_explicit_repair(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path / "crates/iroha_torii/src/tests/routing.rs",
        "async fn permissioned_sumeragi_diagnostics_omit_npos_and_canonical_state()",
        "kura.recover_lane_block_payload(&proposal)",
        "kura.skip_lane_block_payload_recovery(&proposal)",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "permissioned_sumeragi_diagnostics_omit_npos_and_canonical_state" in error
        and "recover_lane_block_payload" in error
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_lost_local_completion_control(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "historical_recovery_and_carrier_tests.rs",
        "fn historical_missing_canonical_block_schedules_authenticated_retry_then_completes()",
        "local completion is never gated by the network deadline",
        "local completion may wait for the network deadline",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "historical_missing_canonical_block_schedules_authenticated_retry_then_completes"
        in error
        and "local completion is never gated" in error
        for error in errors
    ), errors


def test_passive_recovery_contract_rejects_externalized_quiet_tick_regression_drift(
    tmp_path: Path,
) -> None:
    """The reviewed runner-test facade must expose the exact quiet-tick fixture."""

    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path
        / "crates/iroha_core/src/sumeragi/tests/v2_runner_upstream_recovery.rs",
        "fn quiet_retransmission_tick_services_one_retained_historical_session()",
        "CanonicalBlockPending",
        "CanonicalBlockComplete",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(
        "quiet_retransmission_tick_services_one_retained_historical_session" in error
        and "CanonicalBlockPending" in error
        for error in errors
    ), errors


@pytest.mark.parametrize("symbol,old,new", [('latest_autonomous_lane_block_artifacts_snapshot',
  '                    expected_epoch,\n                    None,',
  '                    expected_epoch,\n                    Some(0),'),
 ('latest_autonomous_lane_block_artifacts_snapshot',
  'recovered.push((artifact, current))',
  'recovered.push((artifact, other_current))'),
 ('read_autonomous_lane_block_attempt_record_with_current_locked',
  'pointer.proposal_height != proposal_height',
  'pointer.proposal_height == proposal_height'),
 ('read_autonomous_lane_block_attempt_record_with_current_locked',
  'AutonomousLaneBlockViewStateReadMode::MainOnly',
  'AutonomousLaneBlockViewStateReadMode::Unchecked'),
 ('read_autonomous_lane_block_attempt_artifact_with_current_locked',
  'pointer.network_id != expected_network_id || pointer.epoch != expected_epoch',
  'pointer.network_id != expected_network_id && pointer.epoch != expected_epoch'),
 ('read_autonomous_lane_block_attempt_artifact_with_current_locked',
  '!pointer.matches_payload(&artifact.executable_payload)',
  'false'),
 ('read_autonomous_lane_block_attempt_artifact_with_current_locked',
  '(state.retirement, current)',
  '(state.retirement, other_current)')])
def test_passive_latest_snapshot_rejects_pair_custody_substitution(tmp_path, symbol, old, new):
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    assert validate_fixture(tmp_path, module, models) == ()
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    support.replace_once_after(path, f"fn {symbol}", old, new)
    errors = validate_fixture(tmp_path, module, models)
    assert any(symbol in error and "missing source-bound token" in error for error in errors), errors


@pytest.mark.parametrize(
    "relative,anchor,old,new,symbol",
    (
        ("evidence.rs", "fn recover_finalized_lifecycle_equivocations(",
         ".v2_finality_artifact(height)", ".structural_context_record(height)",
         "recover_finalized_lifecycle_equivocations"),
        ("evidence.rs", "fn recover_finalized_lifecycle_equivocations(",
         "for height in first_height..=height", "for height in first_height..height",
         "recover_finalized_lifecycle_equivocations"),
        ("evidence.rs", "fn recover_finalized_lifecycle_equivocations(",
         "proposal_height.saturating_sub(horizon).max(1)", "height.saturating_sub(horizon).max(1)",
         "recover_finalized_lifecycle_equivocations"),
        ("evidence.rs", "fn recover_context_lifecycle_equivocations(",
         "if &context.network_id != state.network_id_ref()", "if &context.network_id == state.network_id_ref()",
         "recover_context_lifecycle_equivocations"),
        ("evidence.rs", "fn recover_context_lifecycle_equivocations(",
         "retain_sumeragi_v2_equivocation(state, context, proofs_of_possession, proof)",
         "retain_sumeragi_v2_equivocation(state, context, &[], proof)",
         "recover_context_lifecycle_equivocations"),
        ("evidence.rs", "fn retain_sumeragi_v2_equivocation(",
         "validate_v2_equivocation(&payload)?;", "let _ = validate_v2_equivocation(&payload);",
         "retain_sumeragi_v2_equivocation"),
        ("evidence.rs", "fn retain_validated_local_evidence(",
         "!evidence_within_configured_horizon(earliest_admission_height, horizon, Some(subject_height))",
         "evidence_within_configured_horizon(earliest_admission_height, horizon, Some(subject_height))",
         "retain_validated_local_evidence"),
        ("evidence.rs", "fn retain_validated_local_evidence(",
         "committed_key == &key", "committed_key != &key",
         "retain_validated_local_evidence"),
        ("v2_lifecycle_ledger.rs", "fn read_completed_equivocations(",
         "record.terminal() != Some(Some(TerminalOutcome::Advanced))",
         "record.terminal() != Some(None)", "read_completed_equivocations"),
        ("v2_lifecycle_ledger.rs", "fn read_completed_equivocations(",
         "record.terminal() != Some(Some(TerminalOutcome::Advanced))",
         "record.terminal() != Some(Some(TerminalOutcome::Cancelled))", "read_completed_equivocations"),
        ("v2_lifecycle_ledger.rs", "fn read_completed_equivocations(",
         "record.owner().first_admission_ordinal() != record.ordinal()",
         "record.owner().first_admission_ordinal() == record.ordinal()", "read_completed_equivocations"),
        ("v2_lifecycle_ledger.rs", "fn read_completed_equivocations(",
         "record.reconstruction_source() != record.owner().causal_root().digest()",
         "record.reconstruction_source() == record.owner().causal_root().digest()", "read_completed_equivocations"),
        ("v2_lifecycle_ledger_store.rs", "fn read_existing(",
         "BoundLifecycleLedgerDirectory::bind(root, false)?",
         "BoundLifecycleLedgerDirectory::bind(root, true)?", "read_existing"),
        ("v2_lifecycle_ledger_store.rs", "fn read_existing(",
         "let guard = directory.lock()?;",
         "let guard = directory.lock()?; guard.directory.remove_stale_temporary_locked(LEDGER_TEMPORARY_FILE, MAX_LEDGER_FRAME_BYTES)?;",
         "read_existing"),
        ("v2_lifecycle_ledger_store.rs", "fn read_existing(",
         "if ledger.context() != context", "if ledger.context() == context", "read_existing"),
        ("v2_lifecycle_ledger_store.rs", "fn read_existing(",
         "ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;",
         "let _ = ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT);", "read_existing"),
        ("v2_lifecycle_ledger_store.rs", "fn read_bounded_locked(",
         "self.verify_open_leaf(&file, name, leaf)?;",
         "let _ = self.verify_open_leaf(&file, name, leaf);", "read_bounded_locked"),
        ("v2_worker_services_impl.rs", "fn start_with_apply_service(",
         "!state.matches_kura_instance(&kura)", "state.matches_kura_instance(&kura)",
         "start_with_apply_service"),
    ),
    ids=(
        "no-structural-context-authority", "include-applied-tip", "current-horizon",
        "same-network", "original-pops", "reject-invalid-signature", "expiry",
        "committed-key", "never-reopen-ready", "never-revive-cancelled",
        "exact-independent-owner", "exact-causal-root", "never-create-ledger",
        "never-clean-temporary", "exact-frame-context", "reject-malformed-frame",
        "retain-exact-open-file", "same-kura-service",
    ),
)
def test_completed_equivocation_recovery_rejects_semantic_mutation(
    tmp_path: Path, relative: str, anchor: str, old: str, new: str, symbol: str,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path / "crates/iroha_core/src/sumeragi" / relative, anchor, old, new,
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(symbol in error for error in errors), errors


@pytest.mark.parametrize(
    "relative,anchor,first,second,symbol",
    (
        ("mod.rs", "impl SumeragiStartArgs {",
         "evidence::recover_finalized_lifecycle_equivocations(state.as_ref())",
         "FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(",
         "SumeragiStartArgs::start"),
        ("v2_worker_services_impl.rs", "fn start_inner(",
         "super::evidence::recover_context_lifecycle_equivocations(",
         "let io = V2IoHandle::spawn(", "ProductionV2Services::start_inner"),
    ),
    ids=("recover-before-ingress", "recover-before-worker-spawn"),
)
def test_completed_equivocation_recovery_rejects_reordered_startup(
    tmp_path: Path, relative: str, anchor: str, first: str, second: str, symbol: str,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.swap_ordered_once_after(
        tmp_path / "crates/iroha_core/src/sumeragi" / relative, anchor, first, second,
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any(symbol in error and "missing or reorders" in error for error in errors), errors


@pytest.mark.parametrize(
    "relative,token",
    (
        (
            'crates/iroha_core/src/sumeragi/mod.rs',
            'pub(crate) mod evidence;',
        ),
        (
            'crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs',
            '#[path = "v2_lifecycle_ledger.rs"]\nmod ledger;',
        ),
        (
            'crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs',
            '#[path = "v2_lifecycle_replay_authority.rs"]\n#[cfg_attr(not(test), allow(dead_code))]\nmod replay_authority;',
        ),
        (
            'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs',
            'include!("v2_lifecycle_ledger_store.rs");',
        ),
        (
            'crates/iroha_core/src/sumeragi/v2_worker.rs',
            'include!("v2_worker_services_impl.rs");',
        ),
    ),
)
def test_completed_equivocation_recovery_rejects_unbound_provider(
    tmp_path: Path, relative: str, token: str,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once(tmp_path / relative, token, token.replace(";", "_unreviewed;"))
    errors = validate_fixture(tmp_path, module, models)
    assert any("passive provider include" in error for error in errors), errors


def test_completed_equivocation_recovery_requires_real_cold_control(tmp_path: Path) -> None:
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    support.replace_once_after(
        tmp_path / "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests_durable_recovery_02.rs",
        "fn completed_equivocation_recovers_into_new_state_without_reopening_lifecycle()",
        "drop(original)", "retain(original)",
    )
    errors = validate_fixture(tmp_path, module, models)
    assert any("focused control" in error and "drop(original)" in error for error in errors), errors


@pytest.mark.parametrize("pending,index", [(False, i) for i in range(8)] + [(True, i) for i in range(4)])
def test_native_quiet_recovery_rejects_each_unbounded_wait(tmp_path, pending, index):
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    assert validate_fixture(tmp_path, module, models) == ()
    relative, symbol, argument = (
        ("lifecycle_pending_kura.rs", "run_pending_active_height", "IDLE_POLL")
        if pending else ("lifecycle_run_inner.rs", "run_lifecycle_active_height", "native_wait")
    )
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner" / relative
    source = path.read_text(encoding="utf-8")
    item, = module._extract_rust_binding_items(source, "fn", symbol)
    token = f"wake_rx.recv_timeout({argument})"
    parts = item.split(token)
    assert len(parts) == (5 if pending else 9)
    mutated = token.join(parts[:index + 1]) + "wake_rx.recv()" + token.join(parts[index + 1:])
    path.write_text(source.replace(item, mutated, 1), encoding="utf-8")
    errors = validate_fixture(tmp_path, module, models)
    assert any(symbol in error and "bounded Native quiet waits" in error for error in errors), errors


@pytest.mark.parametrize("symbol,old,new", [
    ("NativeSourceRequest::new", "&artifact.validator_set_pops", "&[]"),
    ("NativeSourceRequest::new", "certificate: artifact.commit_qc.clone()", "certificate: other_certificate"),
    ("NativeSourceRequest::new", ".filter(|peer| peer != local)", ".filter(|_| true)"),
    ("NativeSourceRequest::accept", "response.request_hash != request.request_hash()", "false"),
    ("NativeSourceRequest::accept", "&self.source.finality().height_context", "&another_height_context"),
    ("NativeSourceRequest::poll", "now < self.next_retry", "false"),
    ("NativeSourceRequest::poll", "deadline_after(now, retransmit)", "deadline_after(self.next_retry, retransmit)"),
    ("NativeSourceRequest::poll", "self.ticket.take()", "None"),
    ("NativeSourceRequest::poll", "Arc::ptr_eq(message, &self.message)", "true"),
    ("NativeSourceRequest::poll", "self.returned = Some(post)", "self.returned = None"),
    ("NativeSourceRequest::poll", "self.ticket = ticket", "self.ticket = None"),
    ("NativeSourceRequest::next_deadline", "self.response.is_none()", "self.response.is_some()"),
])
def test_native_source_recovery_rejects_custody_or_deadline_mutation(tmp_path, symbol, old, new):
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    assert validate_fixture(tmp_path, module, models) == ()
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner/native_source.rs"
    source = path.read_text(encoding="utf-8")
    item, = module._extract_rust_binding_items(source, "method", symbol)
    assert item.count(old) == 1
    path.write_text(source.replace(item, item.replace(old, new, 1), 1), encoding="utf-8")
    errors = validate_fixture(tmp_path, module, models)
    assert any(symbol in error for error in errors), errors


def test_native_source_retry_requires_deadline_inside_accepted_actor_branch(tmp_path):
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    assert validate_fixture(tmp_path, module, models) == ()
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner/native_source.rs"
    source = path.read_text(encoding="utf-8")
    item, = module._extract_rust_binding_items(source, "method", "NativeSourceRequest::poll")
    deadline = """self.next_retry = if self.cursor == 0 {
                    deadline_after(now, retransmit)
                } else {
                    now
                };"""
    assert item.count(deadline) == 1
    mutated = item.replace(deadline, "", 1).replace(
        "let result = match network.post_recoverable", deadline + "\n        let result = match network.post_recoverable", 1
    )
    path.write_text(source.replace(item, mutated, 1), encoding="utf-8")
    errors = validate_fixture(tmp_path, module, models)
    assert any("exact retained Native retry transition" in error for error in errors), errors


@pytest.mark.parametrize("mutate_source", [False, True])
def test_native_recovery_generic_bindings_use_actual_sources(tmp_path, mutate_source):
    """Exercise the unmodified generic model consumer on the new Native owners."""
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    model = next(m for m in models if m["module"] == module.passive_recovery_contract.AUTONOMOUS_MODULE)
    keys = {
        (relative, kind, symbol)
        for _, relative, kind, symbol, _ in module.passive_recovery_contract.NATIVE_RECOVERY_BINDINGS
    }
    # Other Autonomous source owners are checked by their own contracts. Keep
    # the real TLA/config/invariant metadata and isolate these Native bindings.
    model["production_symbols"] = [
        binding for binding in model["production_symbols"]
        if (binding["path"], binding["kind"], binding["symbol"]) in keys
    ]
    assert len(model["production_symbols"]) == len(keys)
    def validate():
        errors = []
        with module._reviewed_rust_source_cache():
            module._validate_model(tmp_path, support.ROOT_DIR / "formal/sumeragi_v2", model, errors)
        return errors
    assert validate() == []
    if mutate_source:
        support.replace_once_after(
            tmp_path / "crates/iroha_core/src/sumeragi/v2_runner/native_source.rs",
            "pub(super) fn poll(",
            "network.post_recoverable(post, self.ticket.take())",
            "network.post_recoverable(post, None)",
        )
        errors = validate()
        assert len(errors) == 1, errors
        assert "NativeSourceRequest::poll" in errors[0] and "self.ticket.take()" in errors[0], errors


@pytest.mark.parametrize("relative,symbol,old,new", [
    ("v2_lane_process.rs", "LaneProcessOwner::source_recovery_target", "let Owner::Active(owner)", "let Owner::Closing(owner)"),
    ("v2_lane_process.rs", "LaneProcessOwner::source_recovery_target", "owner.current_gate(&self.state, observed) != LaneCurrentGate::Current", "false"),
    ("v2_lane_process.rs", "LaneProcessOwner::source_recovery_target_gate", "!target.state_owner.matches_state(&self.state)", "false"),
    ("v2_lane_process.rs", "LaneProcessOwner::source_recovery_target_gate", "LaneInstance::gate_for(&target.verified, &self.state, observed)", "LaneCurrentGate::InstanceClosed"),
    ("v2_runner/native_source.rs", "NativeSourceRequest::retire_closed_instance", "return LaneCurrentGate::ObservationChanged", "return LaneCurrentGate::InstanceClosed"),
    ("v2_runner/native_source.rs", "NativeSourceRequest::retire_closed_instance", "if gate == LaneCurrentGate::InstanceClosed", "if gate != LaneCurrentGate::ObservationChanged"),
    ("v2_runner/native_process.rs", "NativeRunnerProcess::service_sources", ".source_recovery_target(id, observed.as_ref()?)?", ".unchecked_source_target(id)?"),
    ("v2_runner/native_process.rs", "NativeRunnerProcess::poll", "source_gate != LaneCurrentGate::ObservationChanged", "true"),
    ("v2_runner/native_process.rs", "NativeRunnerProcess::next_deadline", "if self.awaiting_current_observation", "if false"),
    ("v2_runner/native_process.rs", "NativeRunnerProcess::note_current_observation", "observed.is_none_or(|observed| !observed.is_current(&self.state))", "observed.is_none()"),
])
def test_native_source_retirement_requires_original_authenticated_closure(tmp_path, relative, symbol, old, new):
    support = load_support()
    module = support.load_checker()
    models = copy_fixture(tmp_path, support, module)
    assert validate_fixture(tmp_path, module, models) == ()
    path = tmp_path / "crates/iroha_core/src/sumeragi" / relative
    source = path.read_text(encoding="utf-8")
    item, = module._extract_rust_binding_items(source, "method", symbol)
    assert item.count(old) == 1
    path.write_text(source.replace(item, item.replace(old, new, 1), 1), encoding="utf-8")
    errors = validate_fixture(tmp_path, module, models)
    assert any(symbol in error for error in errors), errors
