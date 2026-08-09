"""Terminal tail negative controls for the multilane model/source-binding contract."""

from pathlib import Path

from sumeragi_v2_multilane_models_test import (
    canonical_contract,
    copy_layout_fixture,
    load_checker,
    replace_once,
    validate_fixture,
)


def test_inflight_contract_rejects_reservation_bootstrap_without_operation_schema(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "            RESERVATION_JOURNAL_OPERATION_SCHEMA_V6,\n",
        "",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "bootstrap_frame" in error
        and "RESERVATION_JOURNAL_OPERATION_SCHEMA_V6" in error
        for error in errors
    ), errors


def test_inflight_contract_rejects_primitive_prune_action_reintroduction(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(
        path,
        "        let projection = $projection;\n"
        "        canonical_identity_is_typed_body!(",
        "        let projection = $projection;\n"
        "        let _retired = IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED;\n"
        "        canonical_identity_is_typed_body!(",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_reservation_transition_body" in error
        and "IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_snapshot_nonstutter_mapping(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(
        path,
        "// exact stutter, never a new reservation acquisition.\n"
        "                projection.actor == 0u128\n"
        "                    && projection.target == 0u128\n"
        "                    && in_flight_first_release_state_equal_body!(before, after)",
        "// exact stutter, never a new reservation acquisition.\n"
        "                projection.actor == 0u128\n"
        "                    && projection.target == 0u128\n"
        "                    && before.queue.reservation_state "
        "== after.queue.reservation_state",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_transition_body" in error
        and "in_flight_first_release_state_equal_body" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_missing_direct_release_action(
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
        "ReleaseReservationDirect ==",
        "ReleaseReservationDirectRemoved ==",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "current-semantics action ReleaseReservationDirect" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_rehydrate_without_kura_ownership(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(
        path,
        "                    && (before.session.crashed & projection.actor) == 0u128\n"
        "                    && (before.carrier.kura_active & projection.actor) != 0u128\n"
        "                    && (before.session.bodies & projection.actor) == 0u128\n"
        "                    && !before.release.kura_retired\n",
        "                    && (before.session.crashed & projection.actor) == 0u128\n"
        "                    && (before.session.bodies & projection.actor) == 0u128\n"
        "                    && !before.release.kura_retired\n",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_transition_body" in error
        and "kura_active" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_rehydrate_action_tag_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(
        path,
        "pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY: u8 = 27;",
        "pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY: u8 = 28;",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "current-layout token" in error
        and "REHYDRATE_LOCAL_KURA_CUSTODY" in error
        and "27" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_rehydrate_ready_tampering(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(
        path,
        "                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED\n"
        "                        )\n"
        "                    && after.session.bodies == (before.session.bodies | projection.actor)\n"
        "                    && after.session.ready_authorized == before.session.ready_authorized\n"
        "                    && after.session.crashed == before.session.crashed\n",
        "                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED\n"
        "                        )\n"
        "                    && after.session.bodies == (before.session.bodies | projection.actor)\n"
        "                    && after.session.ready_authorized\n"
        "                        == (before.session.ready_authorized | projection.actor)\n"
        "                    && after.session.crashed == before.session.crashed\n",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_transition_body" in error
        and "ready_authorized" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_terminal_rehydrate_resurrection(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(path, "                    && !before.release.kura_retired\n", "")
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_transition_body" in error
        and "kura_retired" in error
        for error in errors
    ), errors


def test_inflight_contract_rejects_reservation_journal_prune_variant_reintroduction(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "    ForgetCommit(LaneQueueReservationKeyV2),\n"
        "    /// Durably claim an exact FIFO-ordered live reservation set for release.",
        "    ForgetCommit(LaneQueueReservationKeyV2),\n"
        "    Prune { lane_id: LaneId, lane_incarnation: Hash },\n"
        "    /// Durably claim an exact FIFO-ordered live reservation set for release.",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "LaneQueueReservationJournalFrameV6" in error
        and "forbidden source-bound token 'Prune'" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_unreachable_prune_reintroduction(
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
        '   "DirectReleased"}',
        '   "DirectReleased", "PrunedRetired"}',
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "stale first-release layout token 'PrunedRetired' is forbidden" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_terminal_wsv_before_full_forget_prefix(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    replace_once(
        path,
        "        && projection.history.reservation_commit_forgotten_prefix == projection.queue.selected_count\n",
        "",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "production_in_flight_first_release_terminal_owner" in error
        and "reservation_commit_forgotten_prefix" in error
        for error in errors
    ), errors


def test_inflight_composed_contract_rejects_tla_noncanonical_key_prefix(
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
        "  /\\ keys = PrefixThrough(Cardinality(keys))",
        "  /\\ Cardinality(keys) <= bound",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "composed Rust/TLA action-alignment token" in error
        and "CanonicalKeyPrefix" in error
        for error in errors
    ), errors
