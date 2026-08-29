"""Tail negative controls for the multilane model/source-binding contract."""

from pathlib import Path

from sumeragi_v2_multilane_models_test import (
    canonical_contract,
    copy_layout_fixture,
    copy_stable_generation_diagnostics_fixture,
    load_checker,
    replace_once,
    replace_once_after,
    swap_ordered_once,
    swap_ordered_once_after,
    validate_fixture,
    validate_stable_generation_diagnostics_fixture,
)


def test_stable_generation_diagnostics_rejects_retry_bound_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    state, _helper = copy_stable_generation_diagnostics_fixture(tmp_path, module)
    replace_once(
        state,
        "const DIAGNOSTIC_STABLE_STATE_GENERATION_ATTEMPTS: usize = 4;",
        "const DIAGNOSTIC_STABLE_STATE_GENERATION_ATTEMPTS: usize = 40;",
    )
    errors = validate_stable_generation_diagnostics_fixture(tmp_path, module)
    assert any("four-attempt declaration" in error for error in errors), errors


def test_stable_generation_diagnostics_rejects_missing_fail_closed_sink(
    tmp_path: Path,
) -> None:
    module = load_checker()
    _state, helper = copy_stable_generation_diagnostics_fixture(tmp_path, module)
    replace_once(helper, "Err(generation_drift_error())", "derive()")
    errors = validate_stable_generation_diagnostics_fixture(tmp_path, module)
    assert any(
        "stable-generation diagnostics helper token" in error
        and "generation_drift_error" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_tla_rehydrate_guard_omission(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = (
        tmp_path
        / "formal"
        / "sumeragi_v2"
        / "SumeragiV2InFlightFirstRelease.tla"
    )
    replace_once(
        path,
        "  /\\ p \\notin session.crashed\n"
        "  /\\ p \\in carrier.kuraActive\n"
        "  /\\ p \\notin session.bodies\n",
        "  /\\ p \\notin session.crashed\n"
        "  /\\ p \\notin session.bodies\n",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "composed Rust/TLA action-alignment token" in error
        and "RehydrateLocalKuraCustody" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_verus_rehydrate_tamper_proof_removal(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = (
        tmp_path
        / "crates/iroha_sumeragi_core/src/verus_proofs/in_flight_first_release_proofs.rs"
    )
    replace_once(
        path,
        "pub proof fn production_in_flight_first_release_local_kura_rehydration_rejects_volatile_drift(",
        "pub proof fn production_in_flight_first_release_local_kura_rehydration_allows_volatile_drift(",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_local_kura_rehydration_rejects_volatile_drift"
        in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_tla_snapshot_nonstutter_mapping(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = (
        tmp_path
        / "formal"
        / "sumeragi_v2"
        / "SumeragiV2InFlightFirstRelease.tla"
    )
    replace_once(
        path,
        "RecoverReservationSnapshot ==\n  UNCHANGED vars",
        "RecoverReservationSnapshot ==\n"
        "  /\\ queue' = [queue EXCEPT !.reservation = \"Live\"]\n"
        "  /\\ UNCHANGED <<ownership, payloadBinding, carrier, session, "
        "history, decision, release>>",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "composed Rust/TLA action-alignment token" in error
        and "RecoverReservationSnapshot" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_verus_snapshot_stutter_proof_removal(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = (
        tmp_path
        / "crates/iroha_sumeragi_core/src/verus_proofs/in_flight_first_release_proofs.rs"
    )
    replace_once(
        path,
        "pub proof fn production_in_flight_first_release_snapshot_recovery_is_stutter(",
        "pub proof fn production_in_flight_first_release_snapshot_recovery_is_unbound(",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_snapshot_recovery_is_stutter" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_refinement_claim_inflation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    module_path = (
        tmp_path
        / "formal"
        / "sumeragi_v2"
        / "SumeragiV2InFlightFirstRelease.tla"
    )
    replace_once(
        module_path,
        "InFlightFirstReleaseSpec == Init /\\ [][Next]_vars",
        "InFlightFirstReleaseProductionRefinementObligation == TRUE\n\n"
        "InFlightFirstReleaseSpec == Init /\\ [][Next]_vars",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any("must not declare a production refinement" in error for error in errors)


def test_inflight_layout_contract_rejects_kura_release_prefix_preflight_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    replace_once(path, "if claim.stage > previous_stage", "if false")

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "Kura::transition_autonomous_lane_entrypoint_claims_locked" in error
        and "if claim.stage > previous_stage" in error
        for error in errors
    ), errors

    release_path = (
        tmp_path
        / "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs"
    )
    replace_once(
        release_path,
        "match (release_mode, released_disposition) {",
        "match release_mode {",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "AutonomousLaneReleaseProjectionContext::claim_transition_authorization"
        in error
        and "match (release_mode, released_disposition)" in error
        for error in errors
    ), errors

    replace_once(
        release_path,
        "match release_mode {",
        "match (release_mode, released_disposition) {",
    )
    replace_once_after(
        release_path,
        "AutonomousLaneClaimReleaseAuthorizationMode::ReplicaFifo,",
        "if self.actor == self.producer {",
        "if false {",
    )
    replace_once_after(
        release_path,
        "AutonomousLaneClaimReleaseAuthorizationMode::ReplicaDisposition,",
        "if self.actor == self.producer {",
        "if false {",
    )
    errors = validate_fixture(tmp_path, module, contract)
    for required_message in (
        "producer cannot use non-Queue replica FIFO release authority",
        "producer cannot use replica Queue disposition authority",
    ):
        assert any(
            "AutonomousLaneReleaseProjectionContext::claim_transition_authorization"
            in error
            and required_message in error
            for error in errors
        ), errors


def test_inflight_layout_contract_rejects_kura_atomic_replace_order_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = (
        tmp_path
        / "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs"
    )
    swap_ordered_once_after(
        path,
        "fn write_atomic_synced_impl_with_prefix(",
        ".write_all(bytes)",
        ".flush()",
    )

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "ordered in-flight item write_atomic_synced_impl_with_prefix" in error
        and "missing or reorders token" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_missing_queue_release_pending_guard(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue.rs"
    replace_once(
        path,
        "missing Queue release ownership cannot authorize pending Kura claims",
        "missing Queue release ownership is accepted without Kura evidence",
    )

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "Queue::prepare_lane_reservation_release_barrier_inner" in error
        and "missing Queue release ownership cannot authorize pending Kura claims" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_terminal_fifo_ownership_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue.rs"
    replace_once_after(
        path,
        "fn release_barrier_has_exact_fifo_ownership_locked(",
        "let Some(tx) = self.txs.get(&hash) else",
        "let Some(tx) = self.txs.iter().next() else",
    )

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "Queue::release_barrier_has_exact_fifo_ownership_locked" in error
        and "let Some(tx) = self.txs.get(&hash) else" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_expired_release_owner_filter(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue.rs"
    replace_once(
        path,
        "            drop(tx);\n"
        "            if members.insert(hash)",
        "            if self.is_expired(tx.as_accepted()) {\n"
        "                continue;\n"
        "            }\n"
        "            drop(tx);\n"
        "            if members.insert(hash)",
    )

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "Queue::fifo_with_released_reservations_locked" in error
        and "is_expired(" in error
        for error in errors
    ), errors
