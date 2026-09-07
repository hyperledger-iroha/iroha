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
        "            RESERVATION_JOURNAL_OPERATION_SCHEMA_V1,\n",
        "",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "bootstrap_frame" in error
        and "RESERVATION_JOURNAL_OPERATION_SCHEMA_V1" in error
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
        "    ForgetCommit(LaneQueueReservationKeyV1),\n"
        "    /// Durably claim an exact FIFO-ordered live reservation set for release.",
        "    ForgetCommit(LaneQueueReservationKeyV1),\n"
        "    Prune { lane_id: LaneId, lane_incarnation: Hash },\n"
        "    /// Durably claim an exact FIFO-ordered live reservation set for release.",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "LaneQueueReservationJournalFrameV1" in error
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


# These controls refresh the item seals after every mutation. The independent
# semantic contract must still reject a shipping raw-key release or an unchecked
# transition before the one durable append.
_DIRECT_RELEASE_AUTHORITY_MUTATIONS = (
    ("raw_single_cfg", "queue", "release_lane_reservation", "#[cfg(test)]\n", ""),
    ("raw_batch_cfg", "queue", "release_lane_reservations_in_order", "#[cfg(test)]\n", ""),
    ("raw_journal_cfg", "journal", "release", "#[cfg(test)]\n", ""),
    ("raw_single_public", "queue", "release_lane_reservation", "pub(crate) fn", "pub fn"),
    ("raw_batch_public", "queue", "release_lane_reservations_in_order", "pub(crate) fn", "pub fn"),
    ("fixture_variant_cfg", "queue", "enum", "#[cfg(test)]", ""),
    ("fixture_arm_cfg", "queue", "release_lane_reservations_in_order_inner", "#[cfg(test)]", ""),
    ("empty_authority", "queue", "release_strictly_absent_lane_reservations_in_order", "StrictAbsence(authorizations)", "StrictAbsence(Vec::new())"),
    ("early_return", "queue", "release_lane_reservations_in_order_inner", "if self.transaction_selection_durability_faulted()", "return Ok(0);\n        if self.transaction_selection_durability_faulted()"),
    ("skip_union", "queue", "release_lane_reservations_in_order_inner", "if authorized_hashes != entrypoint_hashes", "if false"),
    ("skip_live_revalidation", "queue", "release_lane_reservations_in_order_inner", "self.revalidate_complete_live_pre_kura_group_locked(group, group_keys)?;", ""),
    ("skip_complete_keys", "queue", "release_lane_reservations_in_order_inner", "if records.len() != keys.len()", "if false"),
    ("skip_consume", "queue", "release_lane_reservations_in_order_inner", "for authorization in authorizations {\n                        let projection = authorization.consume_for_queue()", "for authorization in authorizations.into_iter().take(0) {\n                        let projection = authorization.consume_for_queue()"),
    ("skip_fifo_terminal", "queue", "release_lane_reservations_in_order_inner", "if !terminal.ordinary_fifo_owner", "if false"),
)


def _direct_release_authority_fixture(tmp_path: Path):
    from sumeragi_v2_multilane_models_test import copy_reviewed_rust_source_fixture

    module = load_checker()
    import check_sumeragi_v2_proof_ledger as ledger
    import sumeragi_v2_multilane_queue_plan_contract as contract

    copy_reviewed_rust_source_fixture(tmp_path, module, "crates/iroha_core/src/queue.rs")
    copy_reviewed_rust_source_fixture(tmp_path, module, "crates/iroha_core/src/queue/reservation_journal.rs")
    return module, ledger, contract


def test_direct_release_authority_canonical_source_has_one_shipping_path(tmp_path: Path) -> None:
    module, _ledger, contract = _direct_release_authority_fixture(tmp_path)
    errors: list[str] = []
    contract.validate_direct_release_authority_contract(tmp_path, errors, module._rust_binding_item)
    assert errors == []


def test_direct_release_authority_mutations_survive_digest_refresh(tmp_path: Path, monkeypatch) -> None:
    import hashlib
    import json

    module, ledger, contract = _direct_release_authority_fixture(tmp_path)
    queue_path = tmp_path / "crates/iroha_core/src/queue.rs"
    journal_path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    originals = {"queue": queue_path.read_text(), "journal": journal_path.read_text()}
    baseline: list[str] = []
    contract.validate_direct_release_authority_contract(tmp_path, baseline, module._rust_binding_item)
    assert baseline == []
    for name, owner, symbol, before, after in _DIRECT_RELEASE_AUTHORITY_MUTATIONS:
        queue_path.write_text(originals["queue"])
        journal_path.write_text(originals["journal"])
        path = queue_path if owner == "queue" else journal_path
        source = originals[owner]
        if symbol == "enum":
            item_source = module._rust_binding_item(
                tmp_path, "crates/iroha_core/src/queue.rs", "enum", "LaneQueueDirectReleaseGate", "direct-release gate mutation", [],
            )
            assert item_source is not None
            changed = item_source.replace(before, after, 1)
            assert changed != item_source
            source = source.replace(item_source, changed, 1)
        else:
            item, = ledger.rust_items(source, symbol)
            if before == "#[cfg(test)]\n":
                prefix = "#[cfg(test)]\n" + item.source
                assert source.count(prefix) == 1, name
                source = source.replace(prefix, item.source, 1)
            else:
                assert before in item.source, name
                changed = item.source.replace(before, after, 1)
                source = source.replace(item.source, changed, 1)
        path.write_text(source)
        current = queue_path.read_text()
        refreshed = {
            symbol: ledger._rust_item_token_sha256(ledger.rust_items(current, symbol)[0])
            for symbol in contract._DIRECT_RELEASE_PRODUCTION_ITEM_SHA256
        }
        with monkeypatch.context() as local_patch:
            local_patch.setattr(contract, "_DIRECT_RELEASE_PRODUCTION_ITEM_SHA256", refreshed)
            errors: list[str] = []
            contract.validate_direct_release_authority_contract(tmp_path, errors, module._rust_binding_item)
        retained = tmp_path / "mutants" / name
        retained.mkdir(parents=True, exist_ok=False)
        (retained / path.name).write_text(source)
        (retained / "result.json").write_text(json.dumps({
            "mutation": name,
            "source_sha256": hashlib.sha256(source.encode()).hexdigest(),
            "refreshed_item_seals": refreshed,
            "errors": errors,
        }, indent=2) + "\n")
        assert errors, (name, hashlib.sha256(source.encode()).hexdigest())
        assert not any("source seal" in error for error in errors), (name, errors)
        assert any("direct-release authority" in error or "raw-key direct release" in error for error in errors), (name, errors)
