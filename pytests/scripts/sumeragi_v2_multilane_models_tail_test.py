"""Tail negative controls for the multilane model/source-binding contract."""

from pathlib import Path

from sumeragi_v2_multilane_models_test import (
    canonical_contract,
    copy_layout_fixture,
    load_checker,
    replace_once,
    swap_ordered_once,
    validate_fixture,
)


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
    path = tmp_path / "crates/iroha_sumeragi_core/src/verus_proofs.rs"
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
    path = tmp_path / "crates/iroha_sumeragi_core/src/verus_proofs.rs"
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


def test_inflight_layout_contract_rejects_kura_atomic_replace_order_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    swap_ordered_once(path, ".write_all(bytes)", ".flush()")

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "ordered in-flight item Kura::write_atomic_synced_impl" in error
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
    replace_once(
        path,
        "for key in &barrier.ordered_keys {\n"
        "            let hash = key.signed_transaction_hash;\n"
        "            if !self.txs.contains_key(&hash)",
        "for key in &barrier.ordered_keys {\n"
        "            let hash = key.signed_transaction_hash;\n"
        "            if false",
    )

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "Queue::release_barrier_has_exact_fifo_ownership_locked" in error
        and "self.txs.contains_key(&hash)" in error
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
