"""Release-sink negative controls for the multilane model source contract."""

from pathlib import Path

from sumeragi_v2_multilane_models_test import (
    canonical_contract,
    copy_layout_fixture,
    load_checker,
    replace_once,
    swap_ordered_once,
    validate_fixture,
)

def test_inflight_layout_contract_rejects_disconnected_queue_to_kura_release_proof(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    replace_once(
        path,
        "consume_for_claim_transition(queue_barrier)",
        "disconnect_from_claim_transition(queue_barrier)",
    )

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "ordered in-flight item Kura::finalize_autonomous_lane_slot_release_inner" in error
        and "missing or reorders token" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_queue_release_sink_reordering(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue.rs"
    swap_ordered_once(
        path,
        "journal.complete_release(completion.clone())",
        "journal.forget_release(completion.barrier.clone())",
    )

    errors = validate_fixture(tmp_path, module, contract)

    assert any(
        "ordered in-flight item Queue::finalize_lane_reservation_release_barrier_inner" in error
        and "missing or reorders token" in error
        for error in errors
    ), errors
