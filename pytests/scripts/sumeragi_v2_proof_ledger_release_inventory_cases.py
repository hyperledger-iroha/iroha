# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

RELEASE_RECEIPT_COMPONENT_FILES = (
    Path("scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py"),
)


def _release_inventory_fixture_paths(module, paths: tuple[Path, ...]) -> tuple[Path, ...]:
    """Expand reviewed Rust parents to their exact include-component closure."""

    expanded: list[Path] = [
        Path("ci/run_native_amx_v2_grouped_sdk_parity.sh"),
        Path("ci/run_sumeragi_v2_sdk_diagnostics.sh"),
        Path("javascript/iroha_js/test/sumeragiDiagnosticsContract.test.js"),
        Path("javascript/iroha_js/test/toriiClient.test.js"),
    ]
    for relative in paths:
        expanded.append(relative)
        expanded.extend(
            relative.parent / component
            for component in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(
                relative.as_posix(), ()
            )
        )
        if relative == Path("scripts/write_sumeragi_v2_release_receipt.py"):
            expanded.extend(RELEASE_RECEIPT_COMPONENT_FILES)
    return tuple(dict.fromkeys(expanded))


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
            "",
            "must contain exactly 813 tests",
        ),
        (
            "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
            "  peer::shared_byte_budget_tests::authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift\n",
            "production liveness inventory repeats tests",
        ),
        (
            "readonly expected_production_liveness_test_count=813",
            "readonly expected_production_liveness_test_count=812",
            "production liveness source count must be sealed as 813",
        ),
        (
            "readonly expected_typed_rollover_formal_mutation_count=45",
            "readonly expected_typed_rollover_formal_mutation_count=44",
            "45-mutation typed rollover contract fragment",
        ),
        (
            "(INVARIANT|TEMPORAL)_MARKER",
            "INVARIANT_MARKER",
            "45-mutation typed rollover contract fragment",
        ),
        (
            'echo "[tlc] typed rollover-handoff repaired models and 45-mutant '
            'root-anchored V3 matrix passed"',
            'echo "[tlc] typed rollover-handoff matrix passed"',
            "45-mutation typed rollover contract fragment",
        ),
        (
            "readonly expected_multilane_focus_test_count=472",
            "readonly expected_multilane_focus_test_count=467",
            "multilane G-UNIT source count must be sealed as 472",
        ),
        (
            '  if [[ "$(wc -l <"$corridor_g_unit_inventory" | tr -d '
                """'[:space:]')" != 473 ]]; then""",
            '  if [[ "$(wc -l <"$corridor_g_unit_inventory" | tr -d '
                """'[:space:]')" != 472 ]]; then""",
            "G-UNIT TSV guard must require one header plus exactly 472 focus rows",
        ),
        (
            "The canonical 472-row TSV is",
            "The canonical 467-row TSV is",
            "G-UNIT inventory comment must seal 472 rows",
        ),
        (
            "including exact 472/472 G-UNIT,",
            "including exact 471/472 G-UNIT,",
            "terminal success text must seal exact 472/472 G-UNIT",
        ),
        (
            "  kura::tests::native_amx_prevote_byte_budget_is_exact_per_route_and_finality_width_stable\n",
            "  kura::tests::native_amx_prevote_byte_budget_is_exact_per_route_and_finality_width_stable_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  kura::tests::native_amx_prevote_pair_geometry_rejects_empty_hard_cap_and_overflow\n",
            "  kura::tests::native_amx_prevote_pair_geometry_rejects_empty_hard_cap_and_overflow_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  sumeragi::v2_apply::tests::native_amx_prevote_byte_failures_have_precommit_error_classification\n",
            "  sumeragi::v2_apply::tests::native_amx_prevote_byte_failures_have_precommit_error_classification_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  sumeragi::v2_core::refinement::tests::"
            "in_flight_reservation_kernel_accepts_only_identity_bound_local_owner_steps\n",
            "  sumeragi::v2_core::refinement::tests::"
            "in_flight_reservation_kernel_accepts_only_identity_bound_local_owner_steps_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "post_sync_append_publication_failure_is_poisoned_and_replayed_on_reopen\n",
            "  queue::reservation_journal::tests::"
            "post_sync_append_publication_failure_is_poisoned_and_replayed_on_reopen_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "post_sync_compaction_publication_failure_is_poisoned_and_replayed_on_reopen\n",
            "  queue::reservation_journal::tests::"
            "post_sync_compaction_publication_failure_is_poisoned_and_replayed_on_reopen_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "runtime_commit_requires_live_owner_but_snapshot_recovery_may_restore_commit_barrier\n",
            "  queue::reservation_journal::tests::"
            "runtime_commit_requires_live_owner_but_snapshot_recovery_may_restore_commit_barrier_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_is_bound_to_frame_and_state_generation\n",
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_is_bound_to_frame_and_state_generation_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_rejects_same_generation_cross_state_substitution\n",
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_rejects_same_generation_cross_state_substitution_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_binds_exact_ordered_owner_token_coverage\n",
            "  queue::reservation_journal::tests::"
            "prepared_checked_transition_binds_exact_ordered_owner_token_coverage_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "checked_transition_result_identity_and_candidate_application_are_atomic\n",
            "  queue::reservation_journal::tests::"
            "checked_transition_result_identity_and_candidate_application_are_atomic_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "checked_transition_generation_overflow_is_rejected_without_mutation\n",
            "  queue::reservation_journal::tests::"
            "checked_transition_generation_overflow_is_rejected_without_mutation_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "snapshot_replay_seal_covers_empty_and_live_owner_replays\n",
            "  queue::reservation_journal::tests::"
            "snapshot_replay_seal_covers_empty_and_live_owner_replays_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "snapshot_replay_seal_rejects_changed_journal_before_publication\n",
            "  queue::reservation_journal::tests::"
            "snapshot_replay_seal_rejects_changed_journal_before_publication_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  queue::reservation_journal::tests::"
            "snapshot_replay_receipt_rejects_same_count_owner_identity_drift\n",
            "  queue::reservation_journal::tests::"
            "snapshot_replay_receipt_rejects_same_count_owner_identity_drift_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  native_amx::tests::signing_guard_durably_binds_full_source_session_and_participant_incarnation\n"
            "  native_amx::tests::signing_guard_is_restart_safe_idempotent_and_rejects_body_equivocation\n",
            "  native_amx::tests::signing_guard_is_restart_safe_idempotent_and_rejects_body_equivocation\n"
            "  native_amx::tests::signing_guard_durably_binds_full_source_session_and_participant_incarnation\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  block::tests::historical_native_amx_source_bundle_"
            "authenticates_every_evidence_layer\n",
            "  block::tests::historical_native_amx_source_bundle_"
            "authenticates_every_evidence_layer_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  kura::tests::native_amx_all_manifest_barrier_"
            "does_not_promote_another_routes_receipt_temp\n",
            "  kura::tests::native_amx_all_manifest_barrier_"
            "does_not_promote_another_routes_receipt_temp_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  sumeragi::v2_apply::tests::historical_autonomous_recovery_"
            "reaches_exactly_once_canonical_merge_application\n",
            "  sumeragi::v2_apply::tests::historical_autonomous_recovery_"
            "reaches_exactly_once_canonical_merge_application_mutant\n",
            "canonical G-UNIT leg/crate/test inventory SHA-256",
        ),
        (
            "  append_g_unit_inventory \\\n"
            '    g-unit-iroha-core iroha_core "${required_multilane_core_focus_tests[@]}"',
            "  append_g_unit_inventory \\\n"
            '    g-unit-iroha-core iroha_p2p "${required_multilane_core_focus_tests[@]}"',
            "G-UNIT leg g-unit-iroha-core must append the exact",
        ),
        (
            "  zk::kagemusha_finality::tests::aggregate_signature_authenticates_proposal_origin\n"
            "  block::consensus_v2::finality::tests::header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round\n",
            "  block::consensus_v2::finality::tests::header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round\n"
            "  zk::kagemusha_finality::tests::aggregate_signature_authenticates_proposal_origin\n",
            "canonical module/test inventory SHA-256",
        ),
        (
            'production_p2p_unit_list="$(run_cargo test --locked --offline -p iroha_p2p --lib -- --list)"',
            'production_p2p_unit_list="$(run_cargo test --locked --offline -p iroha_p2p --all-features --lib -- --list)"',
            "reviewed P2P corridor must use exact default-feature test discovery",
        ),
        (
            'production_config_unit_list="$(run_cargo test --locked --offline -p iroha_config --lib -- --list)"',
            'production_config_unit_list="$(run_cargo test --locked --offline -p iroha_config --all-features --lib -- --list)"',
            "exact-output configuration discovery must use the exact iroha_config library test surface",
        ),
        (
            'elif [[ "$required_test" == parameters::* ]]; then',
            'elif [[ "$required_test" == configuration::* ]]; then',
            "exact-output configuration tests must route through the iroha_config library corridor",
        ),
        (
            'elif [[ "$module" == parameters::* ]]; then\n'
            '    module_command="cargo test --locked --offline -p iroha_config --lib '
            '${module} -- --test-threads=1"',
            'elif [[ "$module" == parameters::* ]]; then\n'
            '    module_command="cargo test --locked --offline -p iroha_core --lib '
            '${module} -- --test-threads=1"',
            "exact-output configuration tests must route through the iroha_config library corridor",
        ),
    ),
)
def test_production_release_inventory_rejects_name_count_and_feature_mutants(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    for relative in _release_inventory_fixture_paths(
        module,
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("formal/sumeragi_v2/README.md"),
            Path("formal/sumeragi_v2/PROOF.md"),
            Path("specs/sumeragi_v2_liveness.md"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
            Path("integration_tests/tests/sumeragi_v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        ),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    release_path = tmp_path / "scripts" / "run_sumeragi_v2_release_gates.sh"
    source = release_path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    release_path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "  native_amx_grouped_parity_test_counts=(\n"
            "    7\n"
            "    60\n"
            "    58\n",
            "  native_amx_grouped_parity_test_counts=(\n"
            "    7\n"
            "    59\n"
            "    58\n",
            "grouped Native AMX SDK runner suite inventory must equal",
        ),
        (
            Path("ci/run_native_amx_v2_grouped_sdk_parity.sh"),
            "  python)\n    observed_test_count=60\n",
            "  python)\n    observed_test_count=59\n",
            "grouped Native AMX SDK harness suite inventory must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    ("python", 60),\n',
            '    ("python", 59),\n',
            "grouped Native AMX SDK receipt suite inventory must equal",
        ),
    ),
)
def test_production_release_inventory_rejects_grouped_sdk_count_drift(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    fixture_paths = _release_inventory_fixture_paths(
        module,
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("formal/sumeragi_v2/README.md"),
            Path("formal/sumeragi_v2/PROOF.md"),
            Path("specs/sumeragi_v2_liveness.md"),
            Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
            Path("integration_tests/tests/sumeragi_v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
            Path("ci/run_native_amx_v2_grouped_sdk_parity.sh"),
        ),
    )
    for fixture_relative in fixture_paths:
        destination = tmp_path / fixture_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / fixture_relative, destination)

    baseline_errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert baseline_errors == [], baseline_errors
    target = tmp_path / relative
    source = target.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    target.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "  sumeragi_v2_sdk_diagnostics_test_counts=(\n"
            "    114\n"
            "    88\n",
            "  sumeragi_v2_sdk_diagnostics_test_counts=(\n"
            "    113\n"
            "    88\n",
            "Sumeragi SDK diagnostics runner suite inventory must equal",
        ),
        (
            Path("ci/run_sumeragi_v2_sdk_diagnostics.sh"),
            "  python)\n    observed_test_count=114\n",
            "  python)\n    observed_test_count=113\n",
            "Sumeragi SDK diagnostics harness suite inventory must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    ("python", 114),\n',
            '    ("python", 113),\n',
            "Sumeragi SDK diagnostics receipt suite inventory must equal",
        ),
        (
            Path("javascript/iroha_js/test/sumeragiDiagnosticsContract.test.js"),
            '  "typed Sumeragi endpoints reject swapped status and diagnostics payloads",\n',
            "",
            "dedicated JavaScript Sumeragi diagnostics inventory must contain exactly 44",
        ),
        (
            Path("ci/run_sumeragi_v2_sdk_diagnostics.sh"),
            '    "# skipped": 0,\n',
            '    "# skipped": 1,\n',
            "no-skip selector lacks exact fragment",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "# Execute every maintained consumer of the Rust-owned grouped Native AMX V2\n",
            "# --test-name-pattern is retired; execute every maintained consumer.\n",
            "retains retired ordinal/partial selector '--test-name-pattern'",
        ),
    ),
)
def test_production_release_inventory_rejects_sdk_diagnostics_drift(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    fixture_paths = _release_inventory_fixture_paths(
        module,
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("formal/sumeragi_v2/README.md"),
            Path("formal/sumeragi_v2/PROOF.md"),
            Path("specs/sumeragi_v2_liveness.md"),
            Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
            Path("integration_tests/tests/sumeragi_v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        ),
    )
    for fixture_relative in fixture_paths:
        destination = tmp_path / fixture_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / fixture_relative, destination)

    baseline_errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert baseline_errors == [], baseline_errors
    target = tmp_path / relative
    source = target.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    target.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


def test_production_release_inventory_seals_later_genesis_proposal_origin(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert errors == [], errors

    finality_path = (
        tmp_path
        / "crates"
        / "iroha_data_model"
        / "src"
        / "block"
        / "consensus_v2"
        / "finality.rs"
    )
    source = finality_path.read_text(encoding="utf-8")
    exact_call = "artifact_bound_to_header(3, 5)"
    assert source.count(exact_call) == 1
    finality_path.write_text(
        source.replace(exact_call, "artifact_bound_to_header(4, 5)", 1),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "genesis header-binding release regression must match exact reviewed "
        "token digest" in error
        for error in errors
    ), errors


def test_production_release_inventory_seals_contention_tolerant_restart_deadline(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    assert module._production_liveness_release_inventory_errors(tmp_path) == []

    runner_path = (
        tmp_path
        / "integration_tests"
        / "tests"
        / "sumeragi_v2_runner"
        / "restart_timing_test.rs"
    )
    source = runner_path.read_text(encoding="utf-8")
    exact_assertion = "assert_eq!(base_round_timeout_ms, 20_000);"
    assert source.count(exact_assertion) == 1
    runner_path.write_text(
        source.replace(
            exact_assertion,
            "assert_eq!(base_round_timeout_ms, 19_999);",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "contention-tolerant restart release regression must match exact "
        "reviewed token digest" in error
        for error in errors
    ), errors


def test_production_release_inventory_seals_successor_parent_binding(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert errors == [], errors

    adapter_path = tmp_path / "crates" / "iroha_core" / "src" / "sumeragi" / "v2.rs"
    canonical_source = adapter_path.read_text(encoding="utf-8")
    mutations = (
        (
            "successor_core_context_preserves_the_parent_certificate_binding",
            "assert_ne!(core_parent.context_id(), context_id(successor_id));",
            "assert_eq!(core_parent.context_id(), context_id(successor_id));",
        ),
        (
            "successor_context_requires_the_durable_cryptographic_parent",
            "let admitted = adapter\n            .receive_authenticated(authenticated)",
            "let admitted = adapter\n            .receive_authenticated(proposal)",
        ),
        (
            "authentication_rejects_valid_commitment_conflicts_without_mutating_adapter",
            "adapter.authenticate(conflicting_proposal_message),\n"
            "            Err(AdapterError::ConflictingExecutionCommitment)",
            "adapter.authenticate(conflicting_proposal_message),\n"
            "            Err(AdapterError::MissingExecutionCommitment)",
        ),
    )
    for test_name, old, new in mutations:
        assert canonical_source.count(old) == 1, old
        adapter_path.write_text(
            canonical_source.replace(old, new, 1),
            encoding="utf-8",
        )
        errors = module._production_liveness_release_inventory_errors(tmp_path)
        assert any(
            "successor parent-binding release regression "
            f"{test_name} must match exact reviewed token digest" in error
            for error in errors
        ), errors
        adapter_path.write_text(canonical_source, encoding="utf-8")


def test_production_release_inventory_seals_closed_prefix_suffix_retry(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    assert module._production_liveness_release_inventory_errors(tmp_path) == []

    runner_path = (
        tmp_path
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "tests"
        / "v2_runner_unsealed_01.rs"
    )
    source = runner_path.read_text(encoding="utf-8")
    exact_retry_split = (
        "        let error = apply_certified_merge_sidecar_closed_prefixes_with"
        "(&mut adapter, |prefix| {\n"
        "            calls = calls.saturating_add(1);\n"
        "            if calls == 2 {\n"
    )
    assert source.count(exact_retry_split) == 1
    runner_path.write_text(
        source.replace(
            exact_retry_split,
            exact_retry_split.replace("if calls == 2", "if calls == 1"),
            1,
        ),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "closed-prefix suffix-retry release regression must match exact "
        "reviewed token digest" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "old", "new"),
    (
        (
            Path("formal/sumeragi_v2/README.md"),
            "current inventory to 813 tests across 39 modules.\n"
            "Together with the source-sealed command and tooling legs, the pre-network\n"
            "corridor contains 86 legs.",
            "current inventory to 813 tests across 39 modules.\n"
            "Together with the source-sealed command and tooling legs, the pre-network\n"
            "corridor contains 85 legs.",
        ),
        (
            Path("formal/sumeragi_v2/PROOF.md"),
            "current 813-test,\n39-module inventory. The complete source-sealed\n"
            "pre-network corridor\n"
            "contains 86 legs",
            "current 813-test,\n39-module inventory. The complete source-sealed\n"
            "pre-network corridor\n"
            "contains 85 legs",
        ),
        (
            Path("specs/sumeragi_v2_liveness.md"),
            "current source-bound inventory to 813 exact tests "
            "across\n39 modules and 86 pre-network legs.",
            "current source-bound inventory to 813 exact tests "
            "across\n39 modules and 85 pre-network legs.",
        ),
    ),
)
def test_production_release_inventory_rejects_stale_liveness_corridor_claim(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    for fixture_relative in _release_inventory_fixture_paths(
        module,
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            Path("formal/sumeragi_v2/README.md"),
            Path("formal/sumeragi_v2/PROOF.md"),
            Path("specs/sumeragi_v2_liveness.md"),
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
            Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
            Path("integration_tests/tests/sumeragi_v2_runner.rs"),
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        ),
    ):
        destination = tmp_path / fixture_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / fixture_relative, destination)

    document_path = tmp_path / relative
    source = document_path.read_text(encoding="utf-8")
    assert source.count(old) == 1
    document_path.write_text(
        source.replace(old, new, 1),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "release inventory documentation must contain exact claim" in error
        and relative.name in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            "_PRODUCTION_TEST_COUNT = 813",
            "_PRODUCTION_TEST_COUNT = 812",
            "production test count must equal the exact shell inventory count 813",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '        "sumeragi::authoritative_runtime_gate_tests",\n'
            "        41,\n"
            "    ),",
            '        "sumeragi::authoritative_runtime_gate_tests",\n'
            "        40,\n"
            "    ),",
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-adapter", "sumeragi::v2::tests", 46),',
            '("production-v2-adapter", "sumeragi::v2::tests", 45),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-runtime", "sumeragi::v2_runtime::tests", 57),',
            '("production-v2-runtime", "sumeragi::v2_runtime::tests", 56),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-merge-sidecar", "merge_sidecar::tests", 118),',
            '("production-merge-sidecar", "merge_sidecar::tests", 117),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 53),',
            '("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 52),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-worker", "sumeragi::v2_worker::tests", 129),',
            '("production-v2-worker", "sumeragi::v2_worker::tests", 128),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-runner", "sumeragi::v2_runner::tests", 34),',
            '("production-v2-runner", "sumeragi::v2_runner::tests", 33),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '        "production-irohad-network-relay",\n'
            '        "network_relay_tests",\n'
            "        4,\n"
            "    ),",
            '        "production-irohad-network-relay",\n'
            '        "network_relay_tests",\n'
            "        3,\n"
            "    ),",
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "  readonly expected_corridor_leg_count=86",
            "  readonly expected_corridor_leg_count=85",
            "sealed at 86 legs",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            '    source-sealed-workspace-tests command 0 \\\n'
            '    "cargo test --locked --offline --workspace" \\\n'
            "    run_cargo test --locked --offline --workspace",
            '    source-sealed-workspace-tests command 0 \\\n'
            '    "cargo test --locked --workspace" \\\n'
            "    run_cargo test --locked --workspace",
            "source-sealed command-success leg source-sealed-workspace-tests",
        ),
    ),
)
def test_production_release_inventory_rejects_receipt_and_command_drift(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
    )
    required_paths = _release_inventory_fixture_paths(module, required_paths)
    for required in required_paths:
        destination = tmp_path / required
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / required, destination)

    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors
