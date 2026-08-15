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
    ledger = copy.deepcopy(module.load_ledger())
    ledger["machine_checked_completion"] = False
    next(
        obligation
        for obligation in ledger["obligations"]
        if obligation["id"] == "height-liveness"
    )["status"] = "specified_unproved"
    result = module.validate_ledger(ledger, release=True)

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

    assert ledger["machine_checked_completion"] is True
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


def test_temporal_proof_promotions_require_prerequisites_and_ledger_order() -> None:
    module = load_checker()
    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    by_id = {obligation["id"]: obligation for obligation in obligations}

    for dependent_id, prerequisite_id in (
        ("async-type-invariant", "async-runner-scheduler-preservation"),
        ("post-gst-deadlock-freedom", "async-type-invariant"),
        ("post-gst-deadlock-freedom", "progress-witness-preservation"),
        ("post-gst-deadlock-freedom", "protected-service-rank"),
        ("timeout-view-liveness", "post-gst-deadlock-freedom"),
        ("successor-activation-starvation-freedom", "epoch-boundary"),
        ("successor-activation-starvation-freedom", "async-type-invariant"),
    ):
        original_dependent_status = by_id[dependent_id]["status"]
        original_prerequisite_status = by_id[prerequisite_id]["status"]
        by_id[dependent_id]["status"] = "tlaps_proved"
        by_id[prerequisite_id]["status"] = "specified_unproved"
        errors = module._proof_status_dependency_errors(obligations)
        assert (
            f"proof obligation {dependent_id} cannot be tlaps_proved before "
            f"prerequisite {prerequisite_id} is tlaps_proved"
        ) in errors
        by_id[dependent_id]["status"] = original_dependent_status
        by_id[prerequisite_id]["status"] = original_prerequisite_status

    original_progress_refinement_status = by_id[
        "progress-witness-production-refinement"
    ]["status"]
    by_id["progress-witness-production-refinement"]["status"] = "cross_tool_proved"
    original_progress_status = by_id["progress-witness-preservation"]["status"]
    by_id["progress-witness-preservation"]["status"] = "specified_unproved"
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation progress-witness-production-refinement cannot be "
        "cross_tool_proved before prerequisite progress-witness-preservation "
        "is proved"
    ) in errors
    by_id["progress-witness-production-refinement"][
        "status"
    ] = original_progress_refinement_status
    by_id["progress-witness-preservation"]["status"] = original_progress_status

    original_rank_status = by_id["protected-service-rank"]["status"]
    original_starvation_status = by_id["post-gst-starvation-freedom"]["status"]
    by_id["post-gst-starvation-freedom"]["status"] = "tlaps_proved"
    by_id["protected-service-rank"]["status"] = "specified_unproved"
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation post-gst-starvation-freedom cannot be tlaps_proved "
        "before prerequisite protected-service-rank is tlaps_proved"
    ) in errors

    by_id["post-gst-starvation-freedom"]["status"] = original_starvation_status
    by_id["protected-service-rank"]["status"] = original_rank_status
    for dependent_id in (
        "genesis-height-successor-handoff",
        "height-liveness",
    ):
        for prerequisite_id in (
            "rotating-leader-liveness",
            "application-liveness",
            "successor-activation-starvation-freedom",
        ):
            weakened = copy.deepcopy(module.load_ledger()["obligations"])
            weakened_by_id = {
                obligation["id"]: obligation for obligation in weakened
            }
            weakened_by_id[dependent_id]["status"] = "tlaps_proved"
            weakened_by_id[prerequisite_id]["status"] = "specified_unproved"
            errors = module._proof_status_dependency_errors(weakened)
            assert (
                f"proof obligation {dependent_id} cannot be tlaps_proved before "
                f"prerequisite {prerequisite_id} is tlaps_proved"
            ) in errors

    rank_index = next(
        index
        for index, obligation in enumerate(obligations)
        if obligation["id"] == "protected-service-rank"
    )
    starvation_index = next(
        index
        for index, obligation in enumerate(obligations)
        if obligation["id"] == "post-gst-starvation-freedom"
    )
    obligations[rank_index], obligations[starvation_index] = (
        obligations[starvation_index],
        obligations[rank_index],
    )
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation post-gst-starvation-freedom must appear after "
        "prerequisite protected-service-rank"
    ) in errors

    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    fair = next(
        obligation
        for obligation in obligations
        if obligation["id"] == "async-fair-action-refinement"
    )
    obligations.remove(fair)
    protected_rank_index = next(
        index
        for index, obligation in enumerate(obligations)
        if obligation["id"] == "protected-service-rank"
    )
    obligations.insert(protected_rank_index + 1, fair)
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation protected-service-rank must appear after "
        "prerequisite async-fair-action-refinement"
    ) in errors

    for dependent_id, prerequisite_id in (
        ("async-type-invariant", "async-runner-scheduler-preservation"),
        ("post-gst-deadlock-freedom", "async-type-invariant"),
        (
            "progress-witness-production-refinement",
            "progress-witness-preservation",
        ),
        ("post-gst-deadlock-freedom", "protected-service-rank"),
        ("timeout-view-liveness", "post-gst-deadlock-freedom"),
        ("successor-activation-starvation-freedom", "epoch-boundary"),
        ("successor-activation-starvation-freedom", "async-type-invariant"),
    ):
        obligations = copy.deepcopy(module.load_ledger()["obligations"])
        dependent = next(
            obligation
            for obligation in obligations
            if obligation["id"] == dependent_id
        )
        obligations.remove(dependent)
        prerequisite_index = next(
            index
            for index, obligation in enumerate(obligations)
            if obligation["id"] == prerequisite_id
        )
        obligations.insert(prerequisite_index, dependent)
        errors = module._proof_status_dependency_errors(obligations)
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            f"{prerequisite_id}"
        ) in errors

    for dependent_id in (
        "genesis-height-successor-handoff",
        "height-liveness",
    ):
        obligations = copy.deepcopy(module.load_ledger()["obligations"])
        dependent = next(
            obligation
            for obligation in obligations
            if obligation["id"] == dependent_id
        )
        obligations.remove(dependent)
        rotating_index = next(
            index
            for index, obligation in enumerate(obligations)
            if obligation["id"] == "rotating-leader-liveness"
        )
        obligations.insert(rotating_index, dependent)
        errors = module._proof_status_dependency_errors(obligations)
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            "rotating-leader-liveness"
        ) in errors
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            "application-liveness"
        ) in errors
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            "successor-activation-starvation-freedom"
        ) in errors

def test_promotion_target_evidence_rejects_every_range_log_and_digest_mutation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    assert module.EVIDENCE_SCHEMA_VERSION == 3
    ledger = complete_ledger(module)
    formal_dir, _, evidence = build_test_evidence(module, tmp_path)

    def errors_for(mutant):
        return module._release_evidence_errors(
            ledger,
            mutant,
            formal_dir=formal_dir,
            root_dir=tmp_path,
        )

    assert errors_for(evidence) == []

    omitted = copy.deepcopy(evidence)
    omitted["promotion_targets"].pop()
    assert any("canonical 9 + 3 order" in error for error in errors_for(omitted))

    duplicated = copy.deepcopy(evidence)
    duplicated["promotion_targets"][-1] = copy.deepcopy(
        duplicated["promotion_targets"][0]
    )
    duplicate_errors = errors_for(duplicated)
    assert any("must not repeat" in error for error in duplicate_errors)

    reordered = copy.deepcopy(evidence)
    reordered["promotion_targets"][0], reordered["promotion_targets"][1] = (
        reordered["promotion_targets"][1],
        reordered["promotion_targets"][0],
    )
    assert any("canonical 9 + 3 order" in error for error in errors_for(reordered))

    field_mutations = {
        "kind": "cross_tool",
        "ledger_module": "MutatedLedgerModule",
        "provider_module": "SumeragiV2AsyncTemporalClosureProofs",
        "theorem": "EffectiveLockBodyAcquisitionProductionRefinementObligation",
        "start_line": evidence["promotion_targets"][0]["start_line"] + 1,
        "end_line": evidence["promotion_targets"][0]["end_line"] + 1,
        "source": "formal/sumeragi_v2/MutatedProvider.tla",
        "source_sha256": "0" * 64,
        "proof_span_sha256": "1" * 64,
        "invocation_sha256": "2" * 64,
        "expected_obligations": (
            1
            if evidence["promotion_targets"][0]["expected_obligations"] is None
            else evidence["promotion_targets"][0]["expected_obligations"] + 1
        ),
    }
    for field, value in field_mutations.items():
        mutant = copy.deepcopy(evidence)
        mutant["promotion_targets"][0][field] = value
        assert any(
            f"wrong {field}" in error for error in errors_for(mutant)
        )

    zero = copy.deepcopy(evidence)
    zero["promotion_targets"][0]["obligations_proved"] = 0
    assert any("no positive proved count" in error for error in errors_for(zero))

    forged_count = copy.deepcopy(evidence)
    forged_count["promotion_targets"][0]["obligations_proved"] = 999
    assert any("does not match log" in error for error in errors_for(forged_count))

    weakened_invocation = copy.deepcopy(evidence)
    weakened_invocation["promotion_targets"][0]["invocation"].remove("--nofp")
    assert any(
        "wrong invocation" in error for error in errors_for(weakened_invocation)
    )

    wrong_schema = copy.deepcopy(evidence)
    wrong_schema["schema_version"] = 2
    assert any(
        "proof evidence schema_version must equal 3" in error
        for error in errors_for(wrong_schema)
    )

    swapped_log = copy.deepcopy(evidence)
    first, second = swapped_log["promotion_targets"][:2]
    first["log"] = second["log"]
    assert any("must use log" in error for error in errors_for(swapped_log))

    stale_target_log_digest = copy.deepcopy(evidence)
    stale_target_log_digest["promotion_targets"][0]["log_sha256"] = "4" * 64
    assert any(
        "target log digest mismatch" in error
        for error in errors_for(stale_target_log_digest)
    )

    stale_target_ledger = copy.deepcopy(evidence)
    stale_target_ledger["promotion_targets"][0]["ledger_sha256"] = "5" * 64
    assert any(
        "not bound to the current proof ledger" in error
        for error in errors_for(stale_target_ledger)
    )

    stale_ledger = copy.deepcopy(evidence)
    stale_ledger["ledger_sha256"] = "2" * 64
    for entry in stale_ledger["modules"]:
        entry["ledger_sha256"] = "2" * 64
    for entry in stale_ledger["promotion_targets"]:
        entry["ledger_sha256"] = "2" * 64
    stale_errors = errors_for(stale_ledger)
    assert any("byte-exact proof ledger" in error for error in stale_errors)

    stale_source = copy.deepcopy(evidence)
    stale_source["promotion_targets"][0]["source_manifest_sha256"] = "3" * 64
    assert any(
        "current source manifest" in error for error in errors_for(stale_source)
    )

    # Even a semantically identical ledger rewrite invalidates every first-pass
    # transcript because promotion is a byte-level source change.
    (formal_dir / "proof_coverage.json").write_text(
        json.dumps(ledger, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    assert any(
        "byte-exact proof ledger" in error for error in errors_for(evidence)
    )
