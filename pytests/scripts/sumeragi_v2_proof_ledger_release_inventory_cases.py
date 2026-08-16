# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

def complete_ledger(module):
    ledger = copy.deepcopy(module.load_ledger())
    ledger["machine_checked_completion"] = True
    for obligation in ledger["obligations"]:
        expected_status = (
            module.MACHINE_CHECKED_COMPLETION_EXPECTED_STATUS_BY_ID.get(
                obligation["id"]
            )
        )
        if expected_status is not None:
            obligation["status"] = expected_status
    return ledger


def write_tlaps_fixture_logs(
    module, formal_dir: Path, root_dir: Path, log_dir: Path
):
    """Write canonical positive module and exact-target logs for unit fixtures."""

    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / "targets").mkdir(parents=True, exist_ok=True)
    source_manifest_sha256 = module._formal_source_manifest(
        formal_dir, root_dir
    )["sha256"]
    ledger_sha256 = module._proof_ledger_sha256(formal_dir)
    for name in module.RELEASE_PROOF_MODULES:
        (log_dir / f"{name}.preflight.log").write_text(
            "frontend summary passed\n"
            f"{module._tlapm_preflight_marker(name, source_manifest_sha256, ledger_sha256)}\n",
            encoding="utf-8",
        )
        (log_dir / f"{name}.log").write_text(
            "[INFO]: All 1 obligation proved.\n"
            f"{module._tlapm_runner_marker(name, source_manifest_sha256, ledger_sha256)}\n",
            encoding="utf-8",
        )
    for target in module._promotion_target_entries(formal_dir, root_dir):
        (log_dir / "targets" / f"{target['obligation_id']}.log").write_text(
            "[INFO]: All 1 obligation proved.\n"
            + module._tlapm_target_marker(
                target,
                obligations_proved=1,
                source_manifest_sha256=source_manifest_sha256,
                ledger_sha256=ledger_sha256,
            )
            + "\n",
            encoding="utf-8",
        )
    return source_manifest_sha256, ledger_sha256


def build_test_evidence(module, tmp_path: Path):
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    (formal_dir / "proof_coverage.json").write_text(
        json.dumps(complete_ledger(module), indent=2) + "\n",
        encoding="utf-8",
    )
    log_dir = tmp_path / module.FORMAL_EVIDENCE_LOGICAL_ROOT / "tlaps"
    write_tlaps_fixture_logs(module, formal_dir, tmp_path, log_dir)
    evidence = module.build_release_evidence(
        tlapm_version=module.TLAPM_COMMIT[:7],
        log_dir=log_dir,
        formal_dir=formal_dir,
        root_dir=tmp_path,
    )
    return formal_dir, log_dir, evidence


def complete_cross_tool_ledger(module):
    """Return a synthetic complete ledger using the reviewed cross-tool status."""

    return complete_ledger(module)


def build_cross_tool_fixture(module, tmp_path: Path):
    """Build canonical synthetic component logs for checker-only negative tests."""

    # Materialize compact exact non-vacuous synthetic contracts so the
    # promotion validator and every mutation below run through the full
    # signature/kernel/call-site path without duplicating production sources.
    hardened_contracts = []
    shared_kernel_source = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        claims = []
        for claim in contract.claims:
            if claim.proof_mode == "total_checked_gate":
                claims.append(claim)
                continue
            kernel = f"synthetic_{claim.verus_theorem}_kernel"
            projection_builder = f"synthetic_{claim.verus_theorem}_projection"
            projection_builder_source = (
                f"pub closed spec fn {projection_builder}(projection: u64) "
                "-> u64 { projection }"
            )
            projection_builder_sha256 = hashlib.sha256(
                "\0".join(
                    module.rust_code_tokens(projection_builder_source)
                ).encode("utf-8")
            ).hexdigest()
            call_source = claim.production_sources[0]
            call_item = f"enforce_{claim.verus_theorem}"
            call_expression = f"assert!({kernel}(projection));"
            synthetic_call_source = (
                f"fn {call_item}(projection: u64) {{\n"
                f"    {call_expression}\n"
                "}\n"
            )
            extracted_call_items = module.rust_items(
                synthetic_call_source, call_item
            )
            assert len(extracted_call_items) == 1
            call_item_sha256 = module._rust_sealed_item_token_sha256(
                extracted_call_items[0]
            )
            claims.append(
                module.CrossToolClaimContract(
                    constant=claim.constant,
                    verus_theorem=claim.verus_theorem,
                    verus_source=claim.verus_source,
                    production_sources=claim.production_sources,
                    verus_parameters="projection: u64",
                    verus_requires="projection > 0",
                    verus_ensures=(
                        f"{kernel}({projection_builder}(projection)), "
                        f"{projection_builder}(projection) >= 1"
                    ),
                    verified_kernel=kernel,
                    verified_kernel_source=shared_kernel_source,
                    verified_kernel_parameters="projection: u64",
                    verified_kernel_body="projection > 0",
                    theorem_kernel_projection=(
                        f"{projection_builder}(projection)"
                    ),
                    theorem_projection_builder=projection_builder,
                    theorem_projection_builder_parameters="projection: u64",
                    theorem_projection_builder_return="u64",
                    theorem_projection_builder_item_sha256=(
                        projection_builder_sha256
                    ),
                    production_call_sites=(
                        module.CrossToolProductionCallContract(
                            source=call_source,
                            item=call_item,
                            projection="projection",
                            required_expression=call_expression,
                            item_token_sha256=call_item_sha256,
                        ),
                    ),
                )
            )
        hardened_contracts.append(
            module.CrossToolObligationContract(
                obligation_id=contract.obligation_id,
                module=contract.module,
                ledger_symbol=contract.ledger_symbol,
                tla_theorem=contract.tla_theorem,
                tla_statement=contract.tla_statement,
                claims=tuple(claims),
                ledger_declaration_kind=contract.ledger_declaration_kind,
                ledger_statement=contract.ledger_statement,
                tla_proof=contract.tla_proof,
            )
        )
    module.CROSS_TOOL_REFINEMENT_CONTRACTS = tuple(hardened_contracts)
    module.CROSS_TOOL_REFINEMENT_BY_ID = {
        contract.obligation_id: contract
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
    }

    ledger = complete_cross_tool_ledger(module)
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    shutil.copytree(
        module.FORMAL_DIR,
        formal_dir,
        ignore=shutil.ignore_patterns(".tlacache"),
    )

    contracts_by_module = {}
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        contracts_by_module.setdefault(contract.module, []).append(contract)
    for module_name, contracts in contracts_by_module.items():
        path = formal_dir / f"{module_name}.tla"
        source = path.read_text(encoding="utf-8")
        original_source = source
        inherited_source = "\n".join(
            provider_source
            for _, _, provider_source in module._cross_tool_tla_module_closure(
                formal_dir, module_name
            )
        )
        model_side_declarations = ""
        for contract in contracts:
            premise = contract.tla_statement.split(" => ", maxsplit=1)[0]
            if module._expanded_tla_alias(
                inherited_source, premise
            ) == module._expanded_tla_alias(
                inherited_source, contract.ledger_symbol
            ):
                synthetic = f"{contract.tla_theorem}SyntheticModelSide"
                old = f"THEOREM {contract.ledger_symbol} ==\n  {premise}"
                assert source.count(old) == 1
                source = source.replace(
                    old,
                    f"THEOREM {contract.ledger_symbol} ==\n"
                    f"  /\\ {premise}\n"
                    f"  /\\ {synthetic}",
                    1,
                )
                model_side_declarations += f"\n{synthetic} == FALSE\n"
        end = source.rfind("====")
        assert end >= 0
        declarations = model_side_declarations + "".join(
            "\nTHEOREM "
            f"{contract.tla_theorem} ==\n"
            f"  {contract.tla_statement}\n"
            "PROOF\n"
            "  OBVIOUS\n"
            for contract in contracts
            if module._top_level_theorem_body(
                inherited_source, contract.tla_theorem
            )
            is None
        )
        if source != original_source or declarations:
            path.write_text(
                source[:end] + declarations + "\n====\n",
                encoding="utf-8",
            )

    verus_contract = module._verus_evidence_contract_module()
    production_sources = {
        relative
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        for relative in claim.production_sources
    }
    copied_sources = (
        set(verus_contract.REQUIRED_SOURCE_PATHS)
        | production_sources
        | {"crates/iroha_core/src/sumeragi/v2_core.rs"}
    )
    for relative in sorted(copied_sources):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        source = ROOT_DIR / relative
        if source.is_file():
            shutil.copyfile(source, destination)
        else:
            # The fixture exercises the evidence schema independently of
            # unrelated source-inventory migrations in the shared worktree.
            destination.write_text("// synthetic fixture source\n", encoding="utf-8")

    theorem_claims_by_source = {}
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        for claim in contract.claims:
            theorem_claims_by_source.setdefault(claim.verus_source, []).append(
                claim
            )
    for relative, claims in theorem_claims_by_source.items():
        path = tmp_path / relative
        legacy_claims = [
            claim
            for claim in claims
            if claim.proof_mode == "legacy_requires_builder"
        ]
        if not legacy_claims:
            continue
        assert len(legacy_claims) == len(claims)
        source = ""
        synthetic_proofs = "\nverus! {\n"
        for claim in legacy_claims:
            expected_call = (
                f"{claim.verified_kernel}({claim.theorem_kernel_projection})"
            )
            synthetic_proofs += (
                f"pub closed spec fn {claim.theorem_projection_builder}("
                f"{claim.theorem_projection_builder_parameters}) -> "
                f"{claim.theorem_projection_builder_return} {{\n"
                "    projection\n"
                "}\n"
                f"pub closed spec fn {claim.verified_kernel}("
                f"{claim.verified_kernel_parameters}) -> bool {{\n"
                f"    {claim.verified_kernel_body}\n"
                "}\n"
                f"pub proof fn {claim.verus_theorem}({claim.verus_parameters})\n"
                f"    requires {claim.verus_requires},\n"
                f"    ensures {claim.verus_ensures},\n"
                "{\n"
                f"    assert({expected_call});\n"
                "}\n"
            )
        synthetic_proofs += "}\n"
        path.write_text(source + synthetic_proofs, encoding="utf-8")

    kernel_path = tmp_path / shared_kernel_source
    kernel_source = kernel_path.read_text(encoding="utf-8")
    kernel_source += "\n" + "".join(
        f"pub(crate) const fn {claim.verified_kernel}"
        f"({claim.verified_kernel_parameters}) -> bool {{\n"
        f"    {claim.verified_kernel_body}\n"
        "}\n"
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.proof_mode == "legacy_requires_builder"
    )
    kernel_path.write_text(kernel_source, encoding="utf-8")

    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        for claim in contract.claims:
            if claim.proof_mode != "legacy_requires_builder":
                continue
            for call_site in claim.production_call_sites:
                path = tmp_path / call_site.source
                source = path.read_text(encoding="utf-8")
                source += (
                    "\n"
                    f"fn {call_site.item}(projection: u64) {{\n"
                    f"    {call_site.required_expression}\n"
                    "}\n"
                )
                path.write_text(source, encoding="utf-8")

    # Cross-tool release evidence must describe the exact ledger that is part
    # of the source-bound checkout, not a separately supplied archive mutant.
    (formal_dir / "proof_coverage.json").write_text(
        json.dumps(ledger, indent=2) + "\n",
        encoding="utf-8",
    )

    log_dir = tmp_path / module.FORMAL_EVIDENCE_LOGICAL_ROOT / "tlaps"
    write_tlaps_fixture_logs(module, formal_dir, tmp_path, log_dir)
    tlaps_evidence = module.build_release_evidence(
        tlapm_version=module.TLAPM_COMMIT[:7],
        log_dir=log_dir,
        formal_dir=formal_dir,
        root_dir=tmp_path,
    )

    host = verus_contract._host_key()
    if host not in verus_contract.EXPECTED_TOOL_SHA256:
        pytest.skip(f"cross-tool evidence fixture has no pinned Verus host {host}")
    pinned_tool = verus_contract.EXPECTED_TOOL_SHA256[host]
    workspace_manifest_sha256 = "a" * 64
    nonce = "b" * 64
    verus_log = tmp_path / verus_contract.EXPECTED_LOG_PATH
    verus_log.parent.mkdir(parents=True, exist_ok=True)
    verus_log.write_text(
        verus_contract.begin_marker(nonce, workspace_manifest_sha256)
        + "\n"
        + "verification results:: "
        + f"{verus_contract.EXPECTED_DEPENDENCY_VERIFIED} verified, 0 errors\n"
        + "verification results:: "
        + f"{verus_contract.EXPECTED_ROOT_VERIFIED} verified, 0 errors\n"
        + verus_contract.success_marker(nonce, workspace_manifest_sha256)
        + "\n",
        encoding="utf-8",
    )
    verus_evidence = {
        "schema_version": verus_contract.SCHEMA_VERSION,
        "verification_contract_sha256": verus_contract.verification_contract_sha256(),
        "source_manifest_sha256": workspace_manifest_sha256,
        "sources": verus_contract._source_entries(tmp_path),
        "tool": {
            "version": verus_contract.EXPECTED_VERUS_VERSION,
            "platform": pinned_tool["platform"],
            "verus_sha256": pinned_tool["verus"],
            "cargo_verus_sha256": pinned_tool["cargo_verus"],
        },
        "invocation": list(verus_contract.EXPECTED_INVOCATION),
        "log": verus_contract.EXPECTED_LOG_PATH,
        "log_sha256": module._sha256_file(verus_log),
        "nonce": nonce,
        "results": {
            "dependency_verified": verus_contract.EXPECTED_DEPENDENCY_VERIFIED,
            "root_verified": verus_contract.EXPECTED_ROOT_VERIFIED,
            "errors": 0,
        },
        "backend_verification": True,
    }
    cross_tool_evidence = module.build_cross_tool_evidence(
        ledger,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest_sha256,
    )
    return (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest_sha256,
    )

RELEASE_RECEIPT_COMPONENT_FILES = (
    Path("scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py"),
    Path("scripts/write_sumeragi_v2_release_receipt_corridor_log.py"),
    Path("scripts/write_sumeragi_v2_release_receipt_gate_evidence.py"),
    Path("scripts/write_sumeragi_v2_release_receipt_publication.py"),
)
RELEASE_BOOTSTRAP_COMPONENT_FILES = (
    Path("scripts/bootstrap_sumeragi_v2_release_receipt_replay.py"),
)


def _release_inventory_fixture_paths(module, paths: tuple[Path, ...]) -> tuple[Path, ...]:
    """Expand reviewed Rust parents to their exact include-component closure."""

    reviewed_paths = [
        Path("ci/run_native_amx_v2_grouped_sdk_parity.sh"),
        Path("ci/run_sumeragi_v2_sdk_diagnostics.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
        Path("javascript/iroha_js/test/sumeragiDiagnosticsContract.test.js"),
        Path("javascript/iroha_js/test/toriiClient.test.js"),
        Path("crates/iroha_core/src/kura/autonomous_retired_attempt.rs"),
        Path(
            "crates/iroha_core/src/sumeragi/v2_worker/"
            "autonomous_lane_output_reconstruction.rs"
        ),
        Path("crates/iroha_core/src/sumeragi/v2_runner_tests.rs"),
        Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
        *paths,
    ]
    expanded: list[Path] = []

    def append_closure(relative: Path) -> None:
        if relative in expanded:
            return
        expanded.append(relative)
        for component in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(
            relative.as_posix(), ()
        ):
            append_closure(relative.parent / component)
        if relative == Path("scripts/write_sumeragi_v2_release_receipt.py"):
            for component in RELEASE_RECEIPT_COMPONENT_FILES:
                append_closure(component)
        if relative == Path("scripts/bootstrap_sumeragi_v2_release.py"):
            for component in RELEASE_BOOTSTRAP_COMPONENT_FILES:
                append_closure(component)

    for relative in reviewed_paths:
        append_closure(relative)
    return tuple(expanded)


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
            "",
            "must contain exactly 860 tests",
        ),
        (
            "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
            "  peer::shared_byte_budget_tests::authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift\n",
            "production liveness inventory repeats tests",
        ),
        *(
            (
                f"  {test_name}\n",
                "",
                f"production ownership regression {test_name} must be pinned exactly once; found 0",
            )
            for test_name in (
                "sumeragi::v2_effects::tests::exact_candidate_retry_coalesces_under_the_incumbent_owner",
                "sumeragi::v2_effects::tests::fetch_owner_replacement_is_rejected_before_upgrade_refinement_or_request_work",
                "sumeragi::v2_effects::tests::adapter_effect_retry_policy_is_closed_over_all_eleven_effect_classes",
                "sumeragi::v2_effects::tests::late_passive_fetch_completion_opens_one_serve_predecessor_admission_and_steps",
                "sumeragi::v2_lane_work::tests::native_amx_manifest_projects_finality_bound_merge_batch_in_canonical_order",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_multiple_participant_heights_in_one_carrier",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_height_participant_identity_conflict",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_excludes_coordinator_only_receipts",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_route_identity_conflict",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_duplicate_group_source",
                "sumeragi::v2_lane_work::tests::native_amx_merge_projection_matches_decoded_replay_entry",
                "sumeragi::v2_runtime::tests::adapter_effect_binding_is_exact_route_neutral_and_three_bounded",
                "sumeragi::v2_runtime::tests::certified_body_pipeline_retains_statement_and_owner_across_stage_kinds",
                "sumeragi::v2_runtime::tests::body_pipeline_acquires_commit_authority_monotonically_under_one_owner",
                "sumeragi::v2_runtime::tests::applied_validation_failure_suppresses_retry_and_rejects_opposite_outcome",
                "sumeragi::v2_runtime::tests::applied_local_proposal_handoff_suppresses_retry_before_ordinal_allocation",
                "sumeragi::v2_runtime::tests::drained_internal_ignore_uses_exact_durable_tombstone_before_readmission",
                "sumeragi::v2_runtime::tests::queued_body_completion_coalesces_only_its_incumbent_owner",
                "sumeragi::v2_runtime::tests::stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal",
                "sumeragi::v2_runtime::tests::restored_serve_high_watermark_precedes_startup_runtime_owner",
                "sumeragi::v2_runtime::tests::full_runtime_churn_cannot_cross_an_exact_serve_ordinal",
                "sumeragi::v2_worker::tests::exact_serve_predecessor_admission_is_transient_and_barrier_bound",
                "state::tests::block_leaves_governance_unlock_audit_clean_when_no_locks_are_expired",
            )
        ),
        *(
            (
                "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
                f"  sumeragi::v2_core::network_simulation::{test_name}\n",
                "must map to exactly one reviewed module",
            )
            for test_name in (
                "lossy_offline_leader_simulations_commit_for_4_7_and_10_validators",
                "two_by_two_partition_cannot_advance_but_healing_retransmits_tc_and_commits",
                "historical_prepare_qc_uses_current_consumer_tag_after_timeout_install",
                "responsive_source_redelivers_exact_prepare_qc_after_lagger_installs_tc",
                "asymmetric_partition_stalls_without_dual_quorum_then_heals_and_applies",
                "leader_crash_after_proposal_broadcast_does_not_block_the_remaining_quorum",
                "leader_crash_with_a_locked_body_rotates_and_rebuilds_the_old_commit_quorum",
                "corrupted_chunks_and_withheld_commit_evidence_recover_by_bounded_retransmission",
                "crash_after_proposal_wal_before_signature_replays_exact_intent",
                "taira_divergent_views_converge_and_commit_within_one_rotation",
            )
        ),
        (
            "  sumeragi::v2_runner::tests::"
            "terminal_sweep_source_partitions_whole_units_before_any_mutation\n",
            "  sumeragi::v2_runner::tests::"
            "terminal_sweep_source_partitions_whole_units_before_any_mutation_mutant\n",
            "canonical module/test inventory SHA-256",
        ),
        (
            "readonly expected_production_liveness_test_count=860",
            "readonly expected_production_liveness_test_count=859",
            "production liveness source count must be sealed as 860",
        ),
        (
            "  sumeragi::v2_core::tests\n"
            "  sumeragi::v2_core::refinement::tests\n",
            "  sumeragi::v2_core::tests\n"
            "  sumeragi::v2_core::network_simulation\n"
            "  sumeragi::v2_core::refinement::tests\n",
            "production liveness modules must equal the reviewed ordered",
        ),
        (
            "  production-v2-core\n"
            "  production-v2-core-refinement\n",
            "  production-v2-core\n"
            "  production-v2-core-network-simulation\n"
            "  production-v2-core-refinement\n",
            "production module leg IDs must equal the reviewed",
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
            "readonly expected_multilane_focus_test_count=527",
            "readonly expected_multilane_focus_test_count=526",
            "multilane G-UNIT source count must be sealed as 527",
        ),
        (
            '  if [[ "$(wc -l <"$corridor_g_unit_inventory" | tr -d '
                """'[:space:]')" != 528 ]]; then""",
            '  if [[ "$(wc -l <"$corridor_g_unit_inventory" | tr -d '
                """'[:space:]')" != 527 ]]; then""",
            "G-UNIT TSV guard must require one header plus exactly 527 focus rows",
        ),
        (
            "The canonical 527-row TSV is",
            "The canonical 526-row TSV is",
            "G-UNIT inventory comment must seal 527 rows",
        ),
        (
            "including exact 527/527 G-UNIT,",
            "including exact 526/527 G-UNIT,",
            "terminal success text must seal exact 527/527 G-UNIT",
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
            "    63\n"
            "    61\n",
            "  native_amx_grouped_parity_test_counts=(\n"
            "    7\n"
            "    62\n"
            "    61\n",
            "grouped Native AMX SDK runner suite inventory must equal",
        ),
        (
            Path("ci/run_native_amx_v2_grouped_sdk_parity.sh"),
            "  python)\n    observed_test_count=63\n",
            "  python)\n    observed_test_count=62\n",
            "grouped Native AMX SDK harness suite inventory must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    ("python", 63),\n',
            '    ("python", 62),\n',
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
            "    129\n"
            "    88\n",
            "  sumeragi_v2_sdk_diagnostics_test_counts=(\n"
            "    125\n"
            "    88\n",
            "Sumeragi SDK diagnostics runner suite inventory must equal",
        ),
        (
            Path("ci/run_sumeragi_v2_sdk_diagnostics.sh"),
            "  python)\n    observed_test_count=129\n",
            "  python)\n    observed_test_count=125\n",
            "Sumeragi SDK diagnostics harness suite inventory must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    ("python", 129),\n',
            '    ("python", 125),\n',
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
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
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
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
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
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
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

    mutations = (
        (
            Path("crates/iroha_core/src/sumeragi/tests/v2_adapter_activation_context.rs"),
            "successor_core_context_preserves_the_parent_certificate_binding",
            "assert_ne!(core_parent.context_id(), context_id(successor_id));",
            "assert_eq!(core_parent.context_id(), context_id(successor_id));",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_00.rs"
            ),
            "successor_context_requires_the_durable_cryptographic_parent",
            "let admitted = adapter\n        .receive_authenticated(authenticated)",
            "let admitted = adapter\n        .receive_authenticated(proposal)",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_04.rs"
            ),
            "authentication_rejects_valid_commitment_conflicts_without_mutating_adapter",
            "adapter.authenticate(conflicting_proposal_message),\n"
            "        Err(AdapterError::ConflictingExecutionCommitment)",
            "adapter.authenticate(conflicting_proposal_message),\n"
            "        Err(AdapterError::MissingExecutionCommitment)",
        ),
    )
    for relative, test_name, old, new in mutations:
        source_path = tmp_path / relative
        canonical_source = source_path.read_text(encoding="utf-8")
        assert canonical_source.count(old) == 1, old
        source_path.write_text(
            canonical_source.replace(old, new, 1),
            encoding="utf-8",
        )
        errors = module._production_liveness_release_inventory_errors(tmp_path)
        assert any(
            "successor parent-binding release regression "
            f"{test_name} must match exact reviewed token digest" in error
            for error in errors
        ), errors
        source_path.write_text(canonical_source, encoding="utf-8")

    semantic_mutations = (
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_00.rs"
            ),
            "Hash::new(b\"substituted successor execution policy\")",
            "successor.execution_policy_hash",
            "successor authentication must reject execution-policy substitution "
            "against the durable parent context",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_00.rs"
            ),
            "proposal_subject.payload_hash = Hash::new(&proposal_body);",
            "proposal_subject.payload_hash = Hash::new(b\"unbound parent body\");",
            "successor parent-certificate authentication must use a canonical "
            "payload-bound proposal fixture",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_04.rs"
            ),
            "&locally_validated_payload,",
            "&[0x88, 2],",
            "execution-commitment conflict authentication must bind the locally "
            "validated canonical payload fixture",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_adapter_main_04.rs"
            ),
            "encode_payload(&context, proposal_round, proposal_subject, &proposal_body)\n"
            "            .expect(\"encode later-view proposal payload\")",
            "encode_payload(&context, proposal_round, proposal_subject, &[0x83, 3])\n"
            "            .expect(\"encode later-view proposal payload\")",
            "embedded-certificate conflict authentication must bind the "
            "later-view canonical payload fixture",
        ),
    )
    for relative, old, new, expected_error in semantic_mutations:
        source_path = tmp_path / relative
        canonical_source = source_path.read_text(encoding="utf-8")
        assert canonical_source.count(old) == 1, old
        source_path.write_text(
            canonical_source.replace(old, new, 1),
            encoding="utf-8",
        )
        errors = module._production_liveness_release_inventory_errors(tmp_path)
        assert any(expected_error in error for error in errors), errors
        source_path.write_text(canonical_source, encoding="utf-8")

    helper_path = (
        tmp_path
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_worker"
        / "autonomous_lane_output_reconstruction.rs"
    )
    canonical_helper = helper_path.read_text(encoding="utf-8")
    exact_retirement_gate = "bound_supersession_source.is_none()"
    assert canonical_helper.count(exact_retirement_gate) == 1
    helper_path.write_text(
        canonical_helper.replace(
            exact_retirement_gate,
            "bound_supersession_source.is_some()",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "production liveness helper "
        "autonomous_lane_output_has_exact_retirement_source declaration and "
        "complete control flow must match the exact reviewed token digest"
        in error
        for error in errors
    ), errors


def test_production_release_inventory_seals_closed_prefix_suffix_retry(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
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
    exact_retry_split = "        if calls == 2 {"
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
            "current\ninventory to 860 tests across 40 modules.\n"
            "Together with the source-sealed command and tooling legs, the pre-network\n"
            "corridor contains 88 legs.",
            "current\ninventory to 860 tests across 40 modules.\n"
            "Together with the source-sealed command and tooling legs, the pre-network\n"
            "corridor contains 87 legs.",
        ),
        (
            Path("formal/sumeragi_v2/PROOF.md"),
            "current 860-test, 40-module inventory. The complete source-sealed\n"
            "pre-network corridor\n"
            "contains 88 legs",
            "current 860-test, 40-module inventory. The complete source-sealed\n"
            "pre-network corridor\n"
            "contains 87 legs",
        ),
        (
            Path("specs/sumeragi_v2_liveness.md"),
            "current\nsource-bound inventory to 860 exact tests across 40 modules and 88 pre-network\n"
            "legs.",
            "current\nsource-bound inventory to 860 exact tests across 40 modules and 87 pre-network\n"
            "legs.",
        ),
        (
            Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
            "terminal_sweep_source_partitions_whole_units_before_any_mutation",
            "terminal_sweep_source_binds_chain_route_and_empty_post_readback",
        ),
        (
            Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
            "contain exactly 527 unique required",
            "contain exactly 526 unique required",
        ),
        (
            Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
            "tests: 321 core, 143 queue-journal",
            "tests: 320 core, 143 queue-journal",
        ),
        (
            Path("specs/sumeragi_v2_multilane_closure_ledger.md"),
            "exact `527/527` source consistency",
            "exact `526/527` source consistency",
        ),
    ),
    ids=(
        "readme-corridor-count",
        "proof-corridor-count",
        "liveness-corridor-count",
        "closure-ledger-terminal-test-name",
        "closure-ledger-g-unit-total",
        "closure-ledger-g-unit-core-count",
        "closure-ledger-g-unit-ratio",
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
            "_PRODUCTION_TEST_COUNT = 860",
            "_PRODUCTION_TEST_COUNT = 859",
            "production test count must equal the exact shell inventory count 860",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    "write_sumeragi_v2_release_receipt_publication.py",\n',
            "",
            "release receipt component manifest must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt_publication.py"),
            "    return 0\n",
            "    return 0\n\n\ndef _owned_unlink_name(*_args):\n    return True\n",
            "release receipt component symbols must equal",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-core", "sumeragi::v2_core::tests", 38),',
            '("production-v2-core", "sumeragi::v2_core::tests", 39),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '        "sumeragi::authoritative_runtime_gate_tests",\n'
            "        43,\n"
            "    ),",
            '        "sumeragi::authoritative_runtime_gate_tests",\n'
            "        42,\n"
            "    ),",
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-adapter", "sumeragi::v2::tests", 47),',
            '("production-v2-adapter", "sumeragi::v2::tests", 46),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-effects", "sumeragi::v2_effects::tests", 72),',
            '("production-v2-effects", "sumeragi::v2_effects::tests", 71),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-runtime", "sumeragi::v2_runtime::tests", 68),',
            '("production-v2-runtime", "sumeragi::v2_runtime::tests", 67),',
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
            '("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 63),',
            '("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 62),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-worker", "sumeragi::v2_worker::tests", 135),',
            '("production-v2-worker", "sumeragi::v2_worker::tests", 134),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-runner", "sumeragi::v2_runner::tests", 37),',
            '("production-v2-runner", "sumeragi::v2_runner::tests", 36),',
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
            "  taira_public_localnet::strict_restart::taira_localnet_restart_catchup_behavior",
            "  taira_public_localnet::strict_restart::taira_localnet_restart_catchup_warning",
            "Taira release contract inventory must equal the reviewed six-test tuple",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '    "taira_public_localnet::strict_restart::taira_localnet_restart_catchup_behavior",',
            '    "taira_public_localnet::strict_restart::taira_localnet_restart_catchup_warning",',
            "Taira receipt tuple must equal the exact six-test runner inventory",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "  readonly expected_corridor_leg_count=88",
            "  readonly expected_corridor_leg_count=87",
            "sealed at 88 legs",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            '    source-sealed-workspace-tests command 0 \\\n'
            '    "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --offline --workspace" \\\n'
            "    run_cargo test --locked --offline --workspace",
            '    source-sealed-workspace-tests command 0 \\\n'
            '    "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --workspace" \\\n'
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
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
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
