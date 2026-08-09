# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

def test_chain_liveness_dependencies_separate_handoff_from_recovery_progress() -> None:
    """Genesis handoff stays temporal while height progress composes recovery."""

    module = load_checker()

    handoff_dependencies = module.PROOF_STATUS_DEPENDENCIES[
        "genesis-height-successor-handoff"
    ]
    assert "successor-activation-starvation-freedom" in handoff_dependencies
    assert (
        "successor-activation-exact-recovery-production-refinement"
        not in handoff_dependencies
    )

    height_dependencies = module.PROOF_STATUS_DEPENDENCIES["height-liveness"]
    assert "successor-activation-starvation-freedom" in height_dependencies
    assert (
        "successor-activation-exact-recovery-production-refinement"
        in height_dependencies
    )


def test_retired_v1_corridor_is_absent() -> None:
    module = load_checker()

    assert all(not module._retired_path_present(path) for path in module.RETIRED_PATHS)


def test_release_gate_fails_closed_while_completion_is_false() -> None:
    module = load_checker()
    result = module.validate_ledger(module.load_ledger(), release=True)

    assert "release gate requires machine_checked_completion=true" in result.errors
    assert any(
        "release gate rejects unproved target obligation" in error
        for error in result.errors
    )
    assert "release gate requires fresh TLAPS proof evidence" in result.errors


def test_multilane_trace_dependencies_exclude_generic_liveness_debt() -> None:
    """The multilane theorem has an exact proved/trusted dependency slice."""

    module = load_checker()
    ledger = module.load_ledger()

    assert ledger["machine_checked_completion"] is False
    snapshot = module._production_trace_extraction_ledger_dependency_snapshot(
        ledger
    )
    assert [(entry["id"], entry["status"]) for entry in snapshot] == list(
        module.PRODUCTION_TRACE_EXTRACTION_LEDGER_DEPENDENCIES
    )
    dependency_ids = {entry["id"] for entry in snapshot}
    assert "post-gst-deadlock-freedom" not in dependency_ids
    assert "rotating-leader-liveness" not in dependency_ids
    assert "height-liveness" not in dependency_ids


def test_multilane_trace_dependency_status_drift_fails_closed() -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    dependency_id, _ = module.PRODUCTION_TRACE_EXTRACTION_LEDGER_DEPENDENCIES[0]
    obligation = next(
        entry for entry in ledger["obligations"] if entry["id"] == dependency_id
    )
    obligation["status"] = "specified_unproved"

    with pytest.raises(ValueError, match="dependency status drifted"):
        module._production_trace_extraction_ledger_dependency_snapshot(ledger)
