"""Focused controls for passive diagnostics and bounded historical retry."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


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


def test_passive_recovery_contract_rejects_missing_quiet_tick_branch(
    tmp_path: Path,
) -> None:
    cases = (
        (
            "ordinary",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "fn run_lifecycle_active_height(",
            "run_lifecycle_active_height",
            "service_historical_recovery_tick(&mut lane_work, services)?",
            "skip_historical_recovery_tick(&mut lane_work, services)?",
        ),
        (
            "pending-kura",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "fn run_pending_active_height(",
            "run_pending_active_height",
            "service_historical_recovery_tick(lane_work, services)?",
            "skip_historical_recovery_tick(lane_work, services)?",
        ),
    )
    for fixture_name, relative, anchor, symbol, old, new in cases:
        fixture = tmp_path / fixture_name
        support = load_support()
        module = support.load_checker()
        models = copy_fixture(fixture, support, module)
        support.replace_once_after(fixture / relative, anchor, old, new)
        errors = validate_fixture(fixture, module, models)
        assert any(
            symbol in error
            and (
                "source-bound token" in error
                or "must service exactly one retained historical owner" in error
            )
            for error in errors
        ), errors


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
