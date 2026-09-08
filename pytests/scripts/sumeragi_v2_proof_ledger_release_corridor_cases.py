# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

@pytest.mark.parametrize(
    "mutation_name",
    (
        "missing_boundary",
        "wrong_visibility",
        "extra_boundary",
        "test_include_before_boundary",
    ),
)
def test_kura_production_source_boundary_rejects_hostile_test_suffix_mutations(
    tmp_path: Path,
    mutation_name: str,
) -> None:
    """The production Kura inventory must stop at one exact test boundary."""

    module = load_checker()
    repo_root = tmp_path / mutation_name
    kura_relative = Path("crates/iroha_core/src/kura.rs")
    for relative in (kura_relative, *KURA_PRODUCTION_COMPONENT_FILES):
        destination = repo_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)

    kura_path = repo_root / kura_relative
    canonical = kura_path.read_text(encoding="utf-8")
    boundary = "#[cfg(test)]\npub(crate) mod tests {"
    assert canonical.count(boundary) == 1
    _, _, _, baseline_errors = module._kura_production_source_inventory(repo_root)
    assert baseline_errors == []

    if mutation_name == "missing_boundary":
        replacement = "pub(crate) mod tests {"
        diagnostic = "exactly one terminal cfg(test) module boundary"
    elif mutation_name == "wrong_visibility":
        replacement = "#[cfg(test)]\npub(super) mod tests {"
        diagnostic = "exactly one terminal cfg(test) module boundary"
    elif mutation_name == "extra_boundary":
        replacement = boundary + "\n}\n\n" + boundary
        diagnostic = "exactly one terminal cfg(test) module boundary"
    else:
        replacement = (
            '#[cfg(test)]\ninclude!("kura/tests/hostile.rs");\n\n'
            + boundary
        )
        diagnostic = "production source must end before all test includes"

    mutated = canonical.replace(boundary, replacement, 1)
    assert mutated != canonical
    kura_path.write_text(mutated, encoding="utf-8")
    _, _, _, errors = module._kura_production_source_inventory(repo_root)
    assert any(diagnostic in error for error in errors), errors

def test_release_corridor_prebuilds_and_publishes_source_bound_binaries() -> None:
    release_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    ).read_text(encoding="utf-8")
    seed_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_seed_matrix.sh"
    ).read_text(encoding="utf-8")
    chaos_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_100k_chaos.sh"
    ).read_text(encoding="utf-8")
    prebuilt_shell_source = (
        ROOT_DIR / "scripts" / "sumeragi_v2_prebuilt_bundle.sh"
    ).read_text(encoding="utf-8")
    prebuilt_python_source = (
        ROOT_DIR / "scripts" / "sumeragi_v2_prebuilt_bundle.py"
    ).read_text(encoding="utf-8")
    receipt_source = (
        ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py"
    ).read_text(encoding="utf-8")
    receipt_corridor_source = (
        ROOT_DIR
        / "scripts"
        / "write_sumeragi_v2_release_receipt_corridor_log.py"
    ).read_text(encoding="utf-8")
    receipt_publication_source = (
        ROOT_DIR
        / "scripts"
        / "write_sumeragi_v2_release_receipt_publication.py"
    ).read_text(encoding="utf-8")
    process_policy_source = (
        ROOT_DIR / "scripts" / "sumeragi_v2_release_process_policy.sh"
    ).read_text(encoding="utf-8")
    cargo_proxy_source = (
        ROOT_DIR / "scripts" / "sumeragi_v2_release_cargo_proxy.sh"
    ).read_text(encoding="utf-8")

    for source in (release_source,):
        assert "unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN" in source
        assert "CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami" in source
        assert "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA" in source
        assert "TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO" in source
        assert "CARGO_BIN_EXE_iroha" in source
        assert "export IROHA_TEST_SKIP_BUILD=1" in source
        assert "IROHA_TEST_BUILD_TIMEOUT_MS=3600" in source
        assert "sumeragi-v2-release/${" in source
        assert "ensure_source_bound_localnet_binaries" in source
        assert "export_source_bound_localnet_binaries" in source
    for token in (
        'export TEST_NETWORK_BIN_IROHAD="${IROHA_TEST_TARGET_DIR}/release/iroha3d"',
        'export TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL="${IROHA_TEST_TARGET_DIR}/message-control/release/iroha3d"',
        'export TEST_NETWORK_BIN_IROHA="${IROHA_TEST_TARGET_DIR}/release/iroha"',
        'export KAGAMI_BIN="${IROHA_TEST_TARGET_DIR}/release/kagami"',
    ):
        assert prebuilt_shell_source.count(token) == 1
    assert '${prebuilt_repo_root}/target' not in prebuilt_shell_source
    assert "_workspace_target" not in prebuilt_python_source
    assert prebuilt_shell_source.count(
        '--cargo-target-dir "$CARGO_TARGET_DIR"'
    ) == 3
    assert prebuilt_shell_source.count(
        '--artifact-root "$IROHA_RELEASE_ARTIFACT_ROOT"'
    ) == 2
    assert "def _external_roots(" in prebuilt_python_source
    assert "def _prebuilt_artifact_root(" in receipt_corridor_source
    assert "def _prebuilt_release_roots(" in receipt_corridor_source
    assert "_prebuilt_workspace_target" not in receipt_source
    assert 'fields["artifact_root_path"] != str(artifact_root)' in (
        receipt_corridor_source
    )
    assert 'fields["cargo_target_root_path"] != str(cargo_target_root)' in (
        receipt_corridor_source
    )
    assert receipt_publication_source.count(
        'release_root_path.parent / "output"'
    ) == 2
    assert receipt_publication_source.count(
        'release_root_path.parent / "target"'
    ) == 2
    assert "expected_artifact_root=(" in receipt_publication_source
    assert "expected_cargo_target_root=(" in receipt_publication_source
    assert (
        'prebuilt_artifact_root = release_root_path.parent / "output"'
        in receipt_publication_source
    )
    assert (
        'prebuilt_cargo_target_root = release_root_path.parent / "target"'
        in receipt_publication_source
    )
    assert 'repo_root / "target"' not in receipt_source
    assert 'repo_root / "target"' not in receipt_corridor_source
    assert 'repo_root / "target"' not in receipt_publication_source
    assert 'readonly release_target_root="${release_invocation_root}/target"' in (
        release_source
    )
    assert 'readonly release_host_root="${release_invocation_root}/output"' in (
        release_source
    )
    assert "require_release_artifact_path() {" in process_policy_source
    assert cargo_proxy_source.count('source "${PROCESS_POLICY}"') == 1
    assert (
        cargo_proxy_source.count(
            'require_external_cargo_target_dir "${REPO_ROOT}"'
        )
        == 1
    )
    assert cargo_proxy_source.count('run_cargo "$@"') == 1
    assert "command cargo" not in cargo_proxy_source

    triplet_contract = (
        "CARGO_TARGET_DIR, IROHA_RELEASE_ARTIFACT_ROOT, and "
        "IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied all-or-none"
    )
    for runner_source in (seed_source, chaos_source):
        assert runner_source.count(triplet_contract) == 1
        assert runner_source.count("require_disjoint_release_roots") == 1
        assert '--verify --root "$repo_root" --no-writable-paths' in runner_source
        assert '--writable target' not in runner_source
        assert 'require_release_artifact_path "$evidence_root"' in runner_source
        assert 'require_release_artifact_directory "$evidence_root"' in runner_source
    for token in (
        "seed-matrix:prebuilt-publication:before",
        "seed-matrix:prebuilt-publication:after",
        "seed-matrix:test-harness-${run_index}:before",
        "seed-matrix:test-harness-${run_index}:after",
        "seed-matrix:completion-publication:before",
        "seed-matrix:completion-publication:after",
    ):
        assert seed_source.count(token) == 1
    for token in (
        "chaos-100k:harness:before",
        "chaos-100k:harness:after",
        "chaos-100k:completion-publication:before",
        "chaos-100k:completion-publication:after",
    ):
        assert chaos_source.count(token) == 1
    assert (
        'nexus_cross_completion_path_file="${IROHA_RELEASE_ARTIFACT_ROOT}/'
        'nexus-cross-dataspace-completion-path"'
        in release_source
    )
    assert '${IROHA_RELEASE_HOST_ROOT:-${repo_root}/target}' not in release_source

def test_multilane_inventory_seals_standalone_native_evidence_names() -> None:
    inventory_source = (
        ROOT_DIR / "ci" / "check_sumeragi_v2_multilane_release_inventory.sh"
    ).read_text(encoding="utf-8")
    kura_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "kura.rs"
    ).read_text(encoding="utf-8")

    current_names = (
        "native_amx_manifest_v1_",
        "native_amx_receipt_v1_",
        "native_amx_evidence_prune_intent_v2.norito",
        "native_amx_evidence_prune_intent_v2.norito.tmp",
        "native_amx_participant_receipts.latest_v2.norito",
        "native_amx_participant_receipts.latest_v2.norito.tmp",
    )
    for name in current_names:
        assert name in inventory_source
        assert name in kura_source

    obsolete_dense_names = (
        "native_amx_evidence_prune_intent_v1.norito",
        "native_amx_evidence_prune_intent_v1.norito.tmp",
        "native_amx_participant_receipts.latest_v1.norito",
        "native_amx_participant_receipts.latest_v1.norito.tmp",
        "native_amx_participant_receipts.norito",
        "native_amx_participant_receipts.index",
        "native_amx_application_manifests.norito",
        "native_amx_application_manifests.index",
    )
    for name in obsolete_dense_names:
        assert name in inventory_source
        assert name not in kura_source

def test_multilane_inventory_checker_rejects_weakened_production_count(
    tmp_path: Path,
) -> None:
    """Standalone and aggregate guards reject inventory-seal weakening."""

    module = load_checker()
    checker = ROOT_DIR / "ci" / "check_sumeragi_v2_multilane_release_inventory.sh"
    checker_source = checker.read_text(encoding="utf-8")
    helper_start = checker_source.index("require_exact_token() {")
    helper_end = checker_source.index("\n}\n", helper_start) + 3
    helper = checker_source[helper_start:helper_end]
    canonical_declaration = "readonly canonical_production_test_count=881"
    canonical_module_declaration = "readonly canonical_production_module_count=43"
    canonical_corridor_declaration = "readonly canonical_corridor_leg_count=84"
    count_guard = (
        "require_exact_token \\\n"
        '  "$release_runner" \\\n'
        '  "readonly expected_production_liveness_test_count='
        '${canonical_production_test_count}"'
    )
    assert checker_source.count(canonical_declaration) == 1
    assert checker_source.count(canonical_module_declaration) == 1
    assert checker_source.count(canonical_corridor_declaration) == 1
    assert checker_source.count("def static_corridor_leg_count() -> int:") == 1
    assert (
        checker_source.count(
            "derived_corridor_leg_count = static_corridor_leg_count()"
        )
        == 1
    )
    assert checker_source.count(count_guard) == 1

    probe = "\n".join(
        (
            "set -euo pipefail",
            helper,
            canonical_declaration,
            'readonly release_runner="$1"',
            count_guard,
        )
    )
    bash = shutil.which("bash")
    assert bash is not None
    runner = tmp_path / "run_sumeragi_v2_release_gates.sh"
    canonical = "readonly expected_production_liveness_test_count=881"
    weakened = "readonly expected_production_liveness_test_count=860"
    runner.write_text(f"{canonical}\n", encoding="utf-8")

    baseline = subprocess.run(
        [bash, "-c", probe, "inventory-count-probe", str(runner)],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert baseline.returncode == 0, baseline.stderr

    runner.write_text(f"{weakened}\n", encoding="utf-8")

    mutated = subprocess.run(
        [bash, "-c", probe, "inventory-count-probe", str(runner)],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert mutated.returncode != 0
    assert (
        "required multilane release inventory token is missing or duplicated"
        in mutated.stderr
    )
    assert canonical in mutated.stderr

    assert module._production_liveness_release_inventory_guard_errors(
        ROOT_DIR
    ) == []
    aggregate_root = tmp_path / "aggregate"
    aggregate_checker = (
        aggregate_root
        / "ci"
        / "check_sumeragi_v2_multilane_release_inventory.sh"
    )
    aggregate_runner = (
        aggregate_root / "scripts" / "run_sumeragi_v2_release_gates.sh"
    )
    aggregate_checker.parent.mkdir(parents=True, exist_ok=True)
    aggregate_runner.parent.mkdir(parents=True, exist_ok=True)
    release_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    ).read_text(encoding="utf-8")
    aggregate_runner.write_text(release_source, encoding="utf-8")

    guard_mutations = (
        (
            canonical_declaration,
            "readonly canonical_production_test_count=860",
            "must seal exactly 881 production tests",
        ),
        (
            canonical_module_declaration,
            "readonly canonical_production_module_count=41",
            "must seal the exact production module count",
        ),
        (
            canonical_corridor_declaration,
            "readonly canonical_corridor_leg_count=82",
            "must seal the exact source-derived corridor leg count",
        ),
        (
            '    "sumeragi::v2_effects::tests": 66,',
            '    "sumeragi::v2_effects::tests": 65,',
            "changed-module counts must equal the exact reviewed release inventory",
        ),
        (
            '    "sumeragi::v2_runtime::tests": 65,',
            '    "sumeragi::v2_runtime::tests": 64,',
            "changed-module counts must equal the exact reviewed release inventory",
        ),
        (
            '    "sumeragi::v2_runner::tests": 37,',
            '    "sumeragi::v2_runner::tests": 36,',
            "independent inventory guard source SHA-256 must equal",
        ),
        (
            '    "sumeragi::v2_lane_work::tests": 65,',
            '    "sumeragi::v2_lane_work::tests": 62,',
            "changed-module counts must equal the exact reviewed release inventory",
        ),
        (
            '    "6045ac0993327ed787010c6262275808"',
            '    "00000000000000000000000000000000"',
            "canonical production TSV SHA-256 must equal",
        ),
        (
            "readonly expected_production_liveness_test_count="
            '${canonical_production_test_count}"',
            "readonly expected_production_liveness_test_count=859\"",
            "must bind the release-runner production count exactly once",
        ),
        (
            '_PRODUCTION_TEST_COUNT = ${canonical_production_test_count}"',
            '_PRODUCTION_TEST_COUNT = 859"',
            "must bind the receipt-writer production count exactly once",
        ),
        (
            'if "sumeragi::v2_core::network_simulation" in module_counts:',
            'if "sumeragi::v2_core::network_simulation" not in module_counts:',
            "independent inventory guard source SHA-256 must equal",
        ),
        (
            "set -euo pipefail",
            "set +e",
            "independent inventory guard source SHA-256 must equal",
        ),
        (
            'readonly marker_publisher="scripts/publish_release_marker.py"',
            'readonly marker_publisher="scripts/publish_release_marker_bypass.py"',
            "independent inventory guard source SHA-256 must equal",
        ),
        (
            "seed runner lacks one exact boundary/containment token",
            "seed runner accepts a missing boundary/containment token",
            "independent inventory guard source SHA-256 must equal",
        ),
        (
            'readonly nexus_cross_lane_pr_helper="ci/check_nexus_cross_lane_proofs.sh"',
            'readonly nexus_cross_lane_pr_helper="ci/check_nexus_cross_lane_proofs_bypass.sh"',
            "independent inventory guard source SHA-256 must equal",
        ),
        (
            'if policy.count("build|test|run|clippy|verus)") != 1:',
            'if policy.count("build|test|run|clippy|verus|fetch)") != 1:',
            "independent inventory guard source SHA-256 must equal",
        ),
        (
            "receipt writer may validate Cargo only from the policy-captured transcript",
            "receipt writer may execute Cargo outside the policy-captured transcript",
            "independent inventory guard source SHA-256 must equal",
        ),
        (
            "final proof validation is not cooperatively bracketed",
            "final proof validation may bypass cooperative boundaries",
            "independent inventory guard source SHA-256 must equal",
        ),
    )
    for old, new, expected_error in guard_mutations:
        assert checker_source.count(old) == 1, old
        aggregate_checker.write_text(
            checker_source.replace(old, new, 1),
            encoding="utf-8",
        )
        errors = module._production_liveness_release_inventory_guard_errors(
            aggregate_root
        )
        assert any(expected_error in error for error in errors), errors

    aggregate_checker.write_text(checker_source, encoding="utf-8")
    invocation = "bash ci/check_sumeragi_v2_multilane_release_inventory.sh"
    assert release_source.splitlines().count(invocation) == 1
    aggregate_runner.write_text(
        release_source.replace(invocation, "true # skipped inventory guard", 1),
        encoding="utf-8",
    )
    errors = module._production_liveness_release_inventory_guard_errors(
        aggregate_root
    )
    assert any(
        "must invoke the independent multilane inventory guard exactly once"
        in error
        for error in errors
    ), errors

def test_multilane_inventory_checker_rejects_stale_or_duplicated_sdk_manifest_digest(
    tmp_path: Path,
) -> None:
    """The standalone inventory guard binds SDK hashes to the closure ledger."""

    checker = ROOT_DIR / "ci" / "check_sumeragi_v2_multilane_release_inventory.sh"
    checker_source = checker.read_text(encoding="utf-8")
    helper_start = checker_source.index("require_exact_digest_occurrences() {")
    helper_end = checker_source.index("\n}\n", helper_start) + 3
    helper = checker_source[helper_start:helper_end]
    manifest_guard = (
        "require_exact_digest_occurrences \\\n"
        '  "$closure_ledger" \\\n'
        '  "$grouped_suite_source_manifest_sha256" \\\n'
        "  2 \\\n"
        '  "grouped Native AMX V2 suite-source manifest SHA-256"'
    )
    assert checker_source.count(manifest_guard) == 1

    fixture_digest = "a" * 64
    manifest_digest = "b" * 64
    stale_manifest_digest = "c" * 64
    ledger = tmp_path / "sumeragi_v2_multilane_closure_ledger.md"
    ledger.write_text(
        "\n".join(
            (
                fixture_digest,
                fixture_digest,
                manifest_digest,
                manifest_digest,
            )
        ),
        encoding="utf-8",
    )
    probe = "\n".join(
        (
            "set -euo pipefail",
            helper,
            'readonly closure_ledger="$1"',
            'readonly grouped_fixture_sha256="$2"',
            'readonly grouped_suite_source_manifest_sha256="$3"',
            'require_exact_digest_occurrences "$closure_ledger" "$grouped_fixture_sha256" 2 "grouped Native AMX V2 fixture SHA-256"',
            'require_exact_digest_occurrences "$closure_ledger" "$grouped_suite_source_manifest_sha256" 2 "grouped Native AMX V2 suite-source manifest SHA-256"',
        )
    )
    bash = shutil.which("bash")
    assert bash is not None

    baseline = subprocess.run(
        [
            bash,
            "-c",
            probe,
            "inventory-sdk-digest-probe",
            str(ledger),
            fixture_digest,
            manifest_digest,
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert baseline.returncode == 0, baseline.stderr

    source = ledger.read_text(encoding="utf-8")
    ledger.write_text(
        source.replace(manifest_digest, stale_manifest_digest, 1),
        encoding="utf-8",
    )
    mutated = subprocess.run(
        [
            bash,
            "-c",
            probe,
            "inventory-sdk-digest-probe",
            str(ledger),
            fixture_digest,
            manifest_digest,
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert mutated.returncode != 0
    assert "must publish the current grouped Native AMX V2 suite-source manifest" in (
        mutated.stderr
    )

    ledger.write_text(
        "\n".join(
            (
                fixture_digest,
                fixture_digest,
                manifest_digest + manifest_digest,
                manifest_digest,
            )
        ),
        encoding="utf-8",
    )
    oversupplied = subprocess.run(
        [
            bash,
            "-c",
            probe,
            "inventory-sdk-digest-probe",
            str(ledger),
            fixture_digest,
            manifest_digest,
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert oversupplied.returncode != 0
    assert "must publish the current grouped Native AMX V2 suite-source manifest" in (
        oversupplied.stderr
    )

def test_tlaps_runner_rejects_backend_failure_even_when_tlapm_exits_zero() -> None:
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlaps.sh"
    ).read_text(encoding="utf-8")

    completion_check = source.index('TLAPM_COMPLETION_PATTERN=')
    exact_count = source.index('grep -Ec "$TLAPM_COMPLETION_PATTERN"')
    final_line = source.index('tail -n 1 "${LOG_DIR}/${module}.log"')
    runner_marker = source.index('"SUMERAGI_TLAPS_BACKEND_COMPLETE module=${module}')
    assert completion_check < exact_count < runner_marker
    assert completion_check < final_line < runner_marker
    assert "TLAPM did not report exact strict completion" in source

def test_tla2tools_and_replay_share_the_same_pin() -> None:
    scripts = [
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tla2tools.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh",
        ROOT_DIR / "scripts" / "formal" / "collect_sumeragi_v2_replay_receipt.py",
    ]
    sources = [path.read_text(encoding="utf-8") for path in scripts]

    assert all('1.7.4' in source for source in sources)
    assert all(
        "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
        in source
        for source in sources
    )
    # `-noGenerateSpecTE` was introduced after the immutable v1.7.4 release.
    # Keep both TLC entry points executable with the toolchain pinned above;
    # the shell wrapper delegates and does not duplicate tool identity data.
    assert all("-noGenerateSpecTE" not in source for source in sources[1:])

def test_tlc_entrypoints_use_the_pinned_tlapm_library_closures() -> None:
    direct_source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh"
    ).read_text(encoding="utf-8")
    collector_source = (
        ROOT_DIR / "scripts" / "formal" / "collect_sumeragi_v2_replay_receipt.py"
    ).read_text(encoding="utf-8")

    assert "3ab43c7ff31db4ced850619d4746fa4c841a7681" in collector_source
    for expected_hash in (
        "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063",
        "aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da",
    ):
        assert expected_hash in collector_source
    for expected_hash in (
        "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063",
        "aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da",
        "5cc604533e49792c1c3d050a38d845d08d9c209879ca20c86de04975bc4bc563",
        "484bf0f9ab6a69ef45f7282f7f92dcf1e6ae139e44117b0d5a4427635818e773",
        "08f52420cdaaf11292ed366782b5ce5b596bb7cbe789526a1cfd8806dbf98624",
        "6f2f274c2e987d1edcf004d8e37b053f1f82b912e66d6a51bae0af8012ddcbec",
        "1fdbed9077bba9db329e499535be29f8d2e6fba3a2b338e364c3b0ec56596bf9",
    ):
        assert expected_hash in direct_source
    assert '"-DTLA-Library=${tlapm_compat_dir}"' in direct_source
    assert 'ln -s "${TLAPM_STDLIB}/${module}.tla"' in direct_source
    assert '"${tlapm_compat_dir}/${module}.tla"' in direct_source
    for module in (
        "Functions",
        "Folds",
        "TLAPS",
        "FiniteSetTheorems",
        "NaturalsInduction",
        "WellFoundedInduction",
        "SequenceTheorems",
    ):
        assert module in direct_source
    assert 'readonly TLC_MAX_SET_SIZE="1000000"' in direct_source
    assert '-maxSetSize "$TLC_MAX_SET_SIZE"' in direct_source
    assert "TLAPM_MODULES = {" in collector_source
    assert "_validate_projection(args.tlapm_projection)" in collector_source
    assert "stat.S_IMODE(metadata.st_mode) & 0o222" in collector_source
    assert "symlink" in collector_source

def test_tlapm_corridor_uses_one_pinned_identity() -> None:
    commit = "3ab43c7ff31db4ced850619d4746fa4c841a7681"
    exact_identity_paths = (
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tlapm.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "build_sumeragi_v2_tlapm_from_source.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "sumeragi_v2_tlapm_source_build_lock.json",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlaps.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh",
        ROOT_DIR / "scripts" / "formal" / "collect_sumeragi_v2_replay_receipt.py",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_receipt.py",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py",
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh",
        ROOT_DIR / ".github" / "workflows" / "nightly_sumeragi_formal.yml",
        ROOT_DIR / ".github" / "workflows" / "pr.yml",
        ROOT_DIR / "formal" / "sumeragi_v2" / "README.md",
        ROOT_DIR / "formal" / "sumeragi_v2" / "CROSS_TOOL_EVIDENCE.md",
    )
    for path in exact_identity_paths:
        assert commit in path.read_text(encoding="utf-8"), path

    proof_source = (
        ROOT_DIR / "formal" / "sumeragi_v2" / "PROOF.md"
    ).read_text(encoding="utf-8")
    assert commit[:7] in proof_source

def test_liveness_tlc_ceilings_fit_pinned_evaluator_and_service_budget() -> None:
    source = (
        ROOT_DIR / "formal" / "sumeragi_v2" / "liveness.cfg"
    ).read_text(encoding="utf-8")
    model_source = (
        ROOT_DIR
        / "formal"
        / "sumeragi_v2"
        / "SumeragiV2AsyncNetwork.tla"
    ).read_text(encoding="utf-8")

    def natural(name: str) -> int:
        match = re.search(rf"^  {name} = ([0-9]+)$", source, re.MULTILINE)
        assert match is not None
        return int(match.group(1))

    validator_count = natural("N")
    queue_capacity = natural("AsyncQueueCapacity")
    ingress_capacity = natural("AsyncIngressCapacity")
    progress_reserve = natural("AsyncProgressReserve")
    completion_reserve = natural("AsyncCompletionReserve")
    io_aux_capacity = natural("AsyncIoAuxCapacity")
    io_work_capacity = natural("AsyncIoWorkCapacity")
    deferred_normal_capacity = natural("AsyncDeferredNormalCapacity")
    deferred_progress_capacity = natural("AsyncDeferredProgressCapacity")
    delivery_bound = natural("AsyncDeliveryBound")
    retransmit_period = natural("AsyncRetransmitPeriod")
    chunk_count = natural("AsyncChunkCount")

    runner_cycle_budget = queue_capacity + 2 * ingress_capacity + 3
    runtime_cycle_budget = 3 * queue_capacity * runner_cycle_budget
    io_drain_budget = io_aux_capacity + io_work_capacity + 1
    deferred_drain_budget = (
        2 * deferred_normal_capacity
        + deferred_progress_capacity
        + completion_reserve
    )
    causal_candidate_lifecycle_capacity = 3 * queue_capacity
    candidate_producer_action_episode_budget = 72 * (
        queue_capacity
        + 2 * deferred_normal_capacity
        + deferred_progress_capacity
        + causal_candidate_lifecycle_capacity
        + io_work_capacity
    )
    candidate_physical_service_budget = (
        candidate_producer_action_episode_budget
        + runtime_cycle_budget
        + 4 * deferred_drain_budget
        + 6 * io_drain_budget
    )
    retransmit_emission_budget = (
        7 * validator_count
        + validator_count * chunk_count
        + 2 * validator_count
    )
    one_way_transport_budget = delivery_bound * (
        ingress_capacity
        + runtime_cycle_budget
        + retransmit_emission_budget
        + 1
    )
    proposal_pipeline_budget = (
        4
        * validator_count
        * (chunk_count + 8)
        * (candidate_physical_service_budget + 1)
    )
    certified_recovery_budget = (
        2 * one_way_transport_budget
        + 2 * io_drain_budget * delivery_bound
        + 3 * runtime_cycle_budget * delivery_bound
    )
    worst_case_service_budget = (
        proposal_pipeline_budget * delivery_bound
        + certified_recovery_budget
        + 4 * retransmit_period
        + progress_reserve
        + completion_reserve
    )

    maximum_timeout = natural("AsyncMaximumRoundTimeout")
    maximum_view = natural("AsyncMaximumView")
    assert natural("MaxEpoch") == 0
    assert natural("MaxHeight") == 0
    assert "EpochRosters <- CountRostersOneEpoch" in source
    assert "EpochPowers <- CountPowersOneEpoch" in source
    assert "LeaderStarts <- StartsByzantineFirst" in source
    assert "LaneHashes <- LaneHashesOneHeight" in source
    assert "DaHashes <- DaHashesOneHeight" in source
    assert "AsyncNetworkItems <- FiniteAsyncNetworkItems" in source
    assert "FiniteAsyncPublishableControlItems ==" in model_source
    assert "FiniteAsyncNetworkItems ==" in model_source
    for finite_network_owner in (
        "asyncSentItems",
        "asyncRetainedControl",
        "asyncActiveRequests",
        "{packet.item: packet \\in asyncTransport}",
        "FiniteAsyncPublishableControlItems",
    ):
        assert finite_network_owner in model_source
    assert worst_case_service_budget < maximum_timeout == 999_999
    assert worst_case_service_budget <= maximum_view == 999_999
    assert re.findall(
        r"(?m)^PROPERTY ([A-Za-z][A-Za-z0-9_]*)$", source
    ) == [
        "PostGstEventuallyAsyncDecision",
        "ResponsiveDecisionEventuallyApplied",
        "PostGstEventuallyAsyncApplication",
        "PostGstEventuallyAsyncHeightCompletion",
    ]
    assert (
        "asyncNextServeIngressOrdinal\n"
        "       \\in [ValidatorIds ->\n"
        "             1..(AsyncIngressPhysicalOrdinalMaximum + 1)]"
        not in model_source
    )
    assert (
        "/\\ DOMAIN asyncNextServeIngressOrdinal = ValidatorIds\n"
        "  /\\ \\A node \\in ValidatorIds:\n"
        "       asyncNextServeIngressOrdinal[node]\n"
        "         \\in 1..(AsyncIngressPhysicalOrdinalMaximum + 1)"
        in model_source
    )


def test_workspace_excluded_harness_pins_complete_unit_inventory() -> None:
    source = (
        ROOT_DIR / "scripts/formal/run_sumeragi_v2_harness.sh"
    ).read_text(encoding="utf-8")
    unit_branch = source.index("--unit)")
    unit_inventory = source.index("unit_test_list=", unit_branch)
    ignored_inventory = source.index("unit_ignored_test_list=", unit_inventory)
    unit_run = source.index("--lib -- --test-threads=1", ignored_inventory)

    assert unit_branch < unit_inventory < ignored_inventory < unit_run
    assert "if ((${#listed_unit_tests[@]} != 197)); then" in source
    assert "expected exactly 197 Sumeragi v2 reducer unit tests" in source
    assert "reducer unit gate requires all 197 tests to be runnable" in source


def test_workspace_excluded_harness_names_every_required_fast_simulation() -> None:
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    ).read_text(encoding="utf-8")
    expected = {
        "lossy_offline_leader_simulations_commit_for_4_7_and_10_validators",
        "two_by_two_partition_cannot_advance_but_healing_retransmits_tc_and_commits",
        "historical_prepare_qc_uses_current_consumer_tag_after_timeout_install",
        "responsive_source_redelivers_exact_prepare_qc_after_lagger_installs_tc",
        "asymmetric_partition_stalls_without_dual_quorum_then_heals_and_applies",
        "leader_crash_after_proposal_broadcast_does_not_block_the_remaining_quorum",
        "leader_crash_with_a_locked_body_rotates_and_rebuilds_the_old_commit_quorum",
        "corrupted_chunks_and_withheld_commit_evidence_recover_by_bounded_retransmission",
        "crash_after_proposal_wal_before_signature_replays_exact_intent",
        "divergent_views_converge_and_commit_within_one_rotation",
        "accelerated_chain_chaos_smoke_preserves_prefix",
    }

    required_block = re.search(r"required_tests=\(\n(?P<body>.*?)\n    \)", source, re.S)
    assert required_block is not None
    listed = {
        line.strip()
        for line in required_block.group("body").splitlines()
        if line.strip()
    }
    assert listed == expected
    assert 'ignored_test="accelerated_100_000_block_chaos_preserves_chain_prefix"' in source
    assert "--list --ignored" in source
    assert "expected exactly eleven fast and one ignored" in source
    assert "expected six Sumeragi v2 network simulations" not in source
    assert "--unit" in source
    assert "--model-replay" in source
    assert "--chaos-100k" in source


def test_ledger_validator_enforces_replay_trace_source_fidelity() -> None:
    module = load_checker()
    assert module._replay_trace_source_fidelity_errors() == []

    checker_source = SCRIPT.read_text(encoding="utf-8")
    validate_body = checker_source.split("def validate_ledger(", 1)[1].split(
        "\ndef ",
        1,
    )[0]
    assert (
        validate_body.count(
            "errors.extend(_replay_trace_source_fidelity_errors(ROOT_DIR))"
        )
        == 1
    )


def test_readiness_gate_source_seal_rejects_ci_matrix_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    for relative in module._READINESS_TOOL_SOURCE_SHA256:
        source = ROOT_DIR / relative
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

    ci_gate = tmp_path / "ci/check_sumeragi_formal.sh"
    source = ci_gate.read_text(encoding="utf-8")
    ci_gate.write_text(source + "\nexit 0\n", encoding="utf-8")

    errors = module._readiness_kernel_source_fidelity_errors(
        module.FORMAL_DIR,
        tmp_path,
    )

    assert any(
        str(ci_gate) in error
        and "readiness gate source must match exact reviewed SHA-256" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "semantic_error"),
    (
        (
            Path(
                "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv"
            ),
            "100\tPersistDecision\t2\t-\t1\tCommit\tA\n",
            "",
            "exactly 100 actions",
        ),
        (
            Path("scripts/normalize_sumeragi_v2_tlc_trace.py"),
            "from typing import Union\n",
            "from typing import TypeAlias\n",
            "Python 3.9-compatible union import",
        ),
        (
            Path("scripts/normalize_sumeragi_v2_tlc_trace.py"),
            "Scalar = Union[int, str]\n",
            "Scalar = int | str\n",
            "Python 3.9-compatible scalar alias",
        ),
        (
            Path("scripts/formal/check_sumeragi_v2_replay_trace.sh"),
            "sumeragi_v2_tlc_assert_replay_tool_result \\\n",
            "",
            "the exact process-level replay result check",
        ),
        (
            Path("scripts/formal/check_sumeragi_v2_replay_trace.sh"),
            '"$RESOLVED_PYTHON" -B -I -S "$CHECKER" '
            '"${checker_args[@]}" \\\n',
            "",
            "the independent fail-closed signing-request check",
        ),
        (
            Path("scripts/formal/collect_sumeragi_v2_replay_receipt.py"),
            "start_new_session=True,\n",
            "start_new_session=False,\n",
            "the new-session process-group boundary",
        ),
        (
            Path("scripts/formal/collect_sumeragi_v2_replay_receipt.py"),
            'raise CollectionError("normalized replay TSV differs byte-for-byte from the fixture")\n',
            'raise CollectionError("normalized replay differs")\n',
            "the byte-exact normalized fixture gate",
        ),
        (
            Path("scripts/formal/check_sumeragi_v2_replay_receipt.py"),
            'if receipt["mode"] != "formal-only":\n',
            'if receipt["mode"] not in {"formal-only", "integrated"}:\n',
            "the formal-only mode gate",
        ),
        (
            Path("scripts/formal/sumeragi_v2_replay_receipt_v1.schema.json"),
            '"mode": {"const": "formal-only"}',
            '"mode": {"enum": ["formal-only", "integrated"]}',
            "receipt schema mode must be the exact V1 constant",
        ),
    ),
)
def test_replay_trace_source_fidelity_mutations_fail_closed(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    semantic_error: str | None,
) -> None:
    module = load_checker()
    for sealed_relative in module.REPLAY_TRACE_SOURCE_SHA256:
        source = ROOT_DIR / sealed_relative
        destination = tmp_path / sealed_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

    target = tmp_path / relative
    source = target.read_text(encoding="utf-8")
    assert source.count(old) == 1
    target.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._replay_trace_source_fidelity_errors(tmp_path)
    assert any(
        "replay trace source must match exact reviewed SHA-256" in error
        for error in errors
    ), errors

    if semantic_error is not None:
        module.REPLAY_TRACE_SOURCE_SHA256[str(relative)] = hashlib.sha256(
            target.read_bytes()
        ).hexdigest()
        errors = module._replay_trace_source_fidelity_errors(tmp_path)
        assert any(semantic_error in error for error in errors), errors


@pytest.mark.parametrize(
    "arguments",
    (
        ("/attacker/path/cargo", "test", "--locked", "--offline"),
        ("env", "cargo", "test", "--locked", "--offline"),
        ("bash", "-c", "cargo test --locked --offline"),
    ),
)
def test_workspace_excluded_harness_rejects_indirect_cargo_dispatch(
    tmp_path: Path, arguments: tuple[str, ...]
) -> None:
    result = subprocess.run(
        [
            "bash",
            str(ROOT_DIR / "scripts/formal/run_sumeragi_v2_harness.sh"),
            *arguments,
        ],
        cwd=ROOT_DIR,
        env={**os.environ, "CARGO_TARGET_DIR": str(tmp_path / "target")},
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert result.returncode == 2
    assert (
        "positional harness commands are unsupported; select one fixed mode"
        in result.stderr
    )


def test_formal_workflows_use_fresh_private_external_layouts() -> None:
    def job(source: str, name: str) -> str:
        match = re.search(
            rf"(?ms)^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
            source,
        )
        assert match is not None, name
        return match.group("body")

    nightly = (
        ROOT_DIR / ".github" / "workflows" / "nightly_sumeragi_formal.yml"
    ).read_text(encoding="utf-8")
    pull_request = (ROOT_DIR / ".github" / "workflows" / "pr.yml").read_text(
        encoding="utf-8"
    )
    assert "cancel-in-progress: false" in nightly
    assert "cancel-in-progress: true" not in nightly
    assert "cancel-in-progress: true" in pull_request
    assert "cancel-in-progress: false" not in pull_request
    formal_jobs = (
        job(nightly, "sumeragi-v2-formal"),
        job(pull_request, "sumeragi_formal"),
    )
    for formal_job in formal_jobs:
        assert "timeout-minutes:" not in formal_job
        assert "--fetch" not in formal_job
        assert re.search(
            r"(?m)^\s*(?:run:\s*)?(?:command\s+)?cargo(?:\s|$)", formal_job
        ) is None
        assert (
            'mktemp -d "$invocation_base/iroha-sumeragi-v2-formal.XXXXXX"'
            in formal_job
        )
        assert 'invocation_base="$(cd -- /tmp && pwd -P)"' in formal_job
        for variable in (
            "CARGO_TARGET_DIR",
            "IROHA_RELEASE_ARTIFACT_ROOT",
            "IROHA_RELEASE_CANCEL_REQUEST_PATH",
            "SUMERAGI_V2_FORMAL_EVIDENCE_DIR",
            "TLAPM_INSTALL_ROOT",
            "TLA2TOOLS_INSTALL_ROOT",
            "APALACHE_INSTALL_ROOT",
            "VERUS_INSTALL_ROOT",
        ):
            assert f"printf '{variable}=%s\\n'" in formal_job
        assert formal_job.index('mktemp -d "$invocation_base/') < formal_job.index(
            "bash scripts/formal/install_sumeragi_v2_tlapm.sh"
        )
        assert formal_job.index(
            "bash scripts/formal/install_sumeragi_v2_verus.sh"
        ) < formal_job.index("run: bash ci/check_sumeragi_formal.sh")
        assert "steps.formal_layout.outputs.artifact_root" in formal_job

    chaos_job = job(nightly, "sumeragi-v2-chaos-100k")
    assert "timeout-minutes:" not in chaos_job
    assert "--fetch" not in chaos_job
    assert "cargo generate-lockfile" not in chaos_job
    assert "uses: Swatinem/rust-cache@" not in chaos_job
    assert "uses: actions-rust-lang/setup-rust-toolchain@" not in chaos_job
    assert re.search(
        r"(?m)^\s*(?:run:\s*)?(?:command\s+)?cargo(?:\s|$)", chaos_job
    ) is None
    assert 'invocation_base="$(cd -- /tmp && pwd -P)"' in chaos_job
    assert 'mktemp -d "$invocation_base/iroha-sumeragi-v2-chaos.XXXXXX"' in chaos_job
    for variable in (
        "CARGO_TARGET_DIR",
        "IROHA_RELEASE_ARTIFACT_ROOT",
        "IROHA_RELEASE_CANCEL_REQUEST_PATH",
        "SUMERAGI_V2_CHAOS_EVIDENCE_DIR",
    ):
        assert f"printf '{variable}=%s\\n'" in chaos_job
    assert chaos_job.index('mktemp -d "$invocation_base/') < chaos_job.index(
        "run: bash scripts/run_sumeragi_v2_100k_chaos.sh"
    )
    assert "steps.chaos_layout.outputs.artifact_root" in chaos_job


@pytest.mark.parametrize(
    ("relative", "environment_name", "purpose"),
    (
        (
            "scripts/formal/install_sumeragi_v2_tlapm.sh",
            "TLAPM_INSTALL_ROOT",
            "TLAPM install",
        ),
        (
            "scripts/formal/install_sumeragi_v2_tla2tools.sh",
            "TLA2TOOLS_INSTALL_ROOT",
            "TLA2Tools install",
        ),
        (
            "scripts/formal/install_apalache.sh",
            "APALACHE_INSTALL_ROOT",
            "Apalache install",
        ),
        (
            "scripts/formal/install_sumeragi_v2_verus.sh",
            "VERUS_INSTALL_ROOT",
            "Verus install",
        ),
    ),
)
def test_formal_installers_validate_private_external_roots_before_use(
    relative: str,
    environment_name: str,
    purpose: str,
) -> None:
    source = (ROOT_DIR / relative).read_text(encoding="utf-8")
    normalized = " ".join(source.replace("\\\n", "").split())

    assert (
        f"${{{environment_name}:?{environment_name} must be an explicitly "
        "authorized external directory}"
        in source
    )
    policy_source = (
        'source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"'
    )
    validation = (
        'require_external_private_directory "$REPO_ROOT" "$INSTALL_ROOT" '
        f'"{purpose}" || exit $?'
    )
    assert source.count(policy_source) == 1
    assert validation in normalized
    validation_index = normalized.index(validation)
    for first_effect in ("verify_install", "curl ", 'mkdir -p "$INSTALL_ROOT"'):
        if first_effect in normalized:
            assert validation_index < normalized.index(first_effect)
    assert f"${{{environment_name}:-${{REPO_ROOT}}/target" not in source


def test_installers_use_fixed_urls_and_literal_checksums() -> None:
    installers = [
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tlapm.sh",
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tla2tools.sh",
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_verus.sh",
    ]
    for installer in installers:
        source = installer.read_text(encoding="utf-8")
        assert "latest" not in source.lower()
        assert re.search(r'readonly [A-Z_]*SHA256="[0-9a-f]{64}"', source)
        assert "curl" in source
        assert "checksum mismatch" in source

    tlapm_source = installers[0].read_text(encoding="utf-8")
    assert "releases/download/${TLAPM_VERSION}" not in tlapm_source
    assert (
        'readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"'
        in tlapm_source
    )
    for asset_id, digest in (
        (
            "482292328",
            "a686da5dc31892edcd02f25bb14061427e29e16317002d43c5b5be970d1d5daf",
        ),
        (
            "482297997",
            "3ca4c39613e58b90e46a385ee61e2c7f17375c19854ea1a35e056d6eb902071c",
        ),
    ):
        assert f'RELEASE_ASSET_ID="{asset_id}"' in tlapm_source
        assert f'ARCHIVE_SHA256="{digest}"' in tlapm_source
    assert "GitHub Actions run 29682668751" in tlapm_source
    assert "TLAPM_ARCHIVE_PATH" in tlapm_source


def test_tlapm_immutable_source_build_lock_is_exact_and_self_validating(
    tmp_path: Path,
) -> None:
    tmp_path = tmp_path.resolve(strict=True)
    formal_scripts = ROOT_DIR / "scripts" / "formal"
    lock_path = formal_scripts / "sumeragi_v2_tlapm_source_build_lock.json"
    helper = formal_scripts / "sumeragi_v2_tlapm_source_lock.py"

    def invoke(lock_file: Path, platform: str, *arguments: object):
        return subprocess.run(
            [sys.executable, "-I", "-S", str(helper), "--lock", str(lock_file),
             "--platform", platform, *(str(argument) for argument in arguments)],
            check=False, capture_output=True, text=True, timeout=10,
        )

    lock = json.loads(lock_path.read_text(encoding="utf-8"))

    assert lock["schema_version"] == 1
    assert lock["source"] == {
        "commit": "3ab43c7ff31db4ced850619d4746fa4c841a7681",
        "repository": "https://github.com/tlaplus/tlapm.git",
        "source_date_epoch": 1784455405,
        "tree": "bf173dd38408314652d436f990b2b9edadaaabe9",
        "version": "1.6.0-pre",
    }
    assert lock["opam"] == {
        "repository": {
            "commit": "ba7c59c5aafbef7f549ce2ca2e2b864cbfa0a5f7",
            "repository": "https://github.com/ocaml/opam-repository.git",
            "tree": "afd25878962ffa2ab4788fe71a0d9c726bb02342",
        },
        "version": "2.5.2",
    }
    compiler_packages = lock["compiler_packages"]
    build_packages = lock["build_packages"]
    assert len(compiler_packages) == 9
    assert len(build_packages) == 122
    assert len(
        {
            package["name"]
            for package in compiler_packages + build_packages
        }
    ) == 131
    assert {
        package["name"]: package["version"]
        for package in compiler_packages
    }["ocaml-base-compiler"] == "5.1.0"
    assert {
        package["name"]: package["version"] for package in build_packages
    }["dune"] == "3.24.0"

    assert set(lock["platforms"]) == {"arm64-darwin", "x86_64-linux-gnu"}
    expected_platform_pins = {
        "arm64-darwin": {
            "additional": {},
            "count": 131,
            "opam": "407e53416cfb49b41ce80e6d3c67a3df08df7f5028f407311f457f4e2a19004b",
            "package_set": "2f61af5fd7ef689457f622dfb1b31d9169fcbeacb6d5aed0859b8ca73240f34c",
            "z3": "5fdbec33ca4a2ef8169553b6a4f41d9c05d5e9d5ef56c400c28dafb007f0e768",
            "isabelle": "ea5754c228857f5d9d3ae254ec9814797f2453ea290df20b2f6dcb2ef0e2e7f8",
            "z3_member": "0207d927019e8d90c28acd18c4596796baef36a87583e6e853c81487a9cd0c27",
        },
        "x86_64-linux-gnu": {
            "additional": {"eio_linux": "1.2", "uring": "2.7.0"},
            "count": 133,
            "opam": "edfca2630c373b44b7ee1c2f81cd8dcf67468d0db57d6c02158de553ac63dbd4",
            "package_set": "9e706a61b06be508588ac8be7530ee3b2aea94a5c1ee22204bada0e887e51a95",
            "z3": "42f1644d79596718bf56944365900df8ef261c3150dddbb7687b5d3797d55c2d",
            "isabelle": "3d1d66de371823fe31aa8ae66638f73575bac244f00b31aee1dcb62f38147c56",
            "z3_member": "4321b0c0db1574a1e90881d9e097f12b8753d0c3e78b21a0155c77005a631436",
        },
    }
    for platform, expected in expected_platform_pins.items():
        platform_lock = lock["platforms"][platform]
        additional = {
            package["name"]: package["version"]
            for package in platform_lock["additional_packages"]
        }
        assert additional == expected["additional"]
        assert len(compiler_packages) + len(build_packages) + len(additional) == (
            expected["count"]
        )
        assert platform_lock["package_set_sha256"] == expected["package_set"]
        assert platform_lock["opam_binary"]["sha256"] == expected["opam"]
        backends = {
            backend["name"]: backend
            for backend in platform_lock["backend_downloads"]
        }
        assert list(backends) == [
            "community-modules",
            "isabelle",
            "ls4",
            "z3",
        ]
        community = backends["community-modules"]
        assert community["download_url"] == (
            "https://github.com/tlaplus/CommunityModules/releases/download/"
            "202607181436/CommunityModules.jar"
        )
        assert community["requested_url"] == (
            "https://github.com/tlaplus/CommunityModules/releases/latest/"
            "download/CommunityModules.jar"
        )
        assert community["sha256"] == (
            "c90a5e35c8fbfb656788332c3c532a13d7cef3b71ad9e699afaeb8873bd1ecf6"
        )
        assert community["progress_dot_giga"] is False
        assert backends["ls4"]["sha256"] == (
            "2d3fff1637497971cf00287df1a6cbb572769a61d10a86e0d11d34d39a017b1d"
        )
        assert backends["z3"]["sha256"] == expected["z3"]
        assert backends["z3"]["locked_output_sha256"] == expected["z3_member"]
        assert backends["z3"]["locked_output_architecture"] == "x86_64"
        assert backends["isabelle"]["sha256"] == expected["isabelle"]
        assert backends["isabelle"]["directory_prefix"] == "_build_cache"

        validation = invoke(lock_path, platform, "validate")
        assert validation.returncode == 0, validation.stderr

    changed_package_lock = json.loads(lock_path.read_text(encoding="utf-8"))
    changed_package_lock["build_packages"][20]["version"] = "3.23.0"
    changed_package_lock_path = tmp_path / "changed-package-lock.json"
    changed_package_lock_path.write_text(
        json.dumps(changed_package_lock, indent=2) + "\n",
        encoding="utf-8",
    )
    changed_package = invoke(changed_package_lock_path, "arm64-darwin", "validate")
    assert changed_package.returncode != 0
    assert "package_set_sha256 does not match" in changed_package.stderr

    fixture_bytes = b"bounded source-build fixture\n"
    fixture_sha256 = hashlib.sha256(fixture_bytes).hexdigest()
    fixture_lock = json.loads(lock_path.read_text(encoding="utf-8"))
    for platform_lock in fixture_lock["platforms"].values():
        for backend in platform_lock["backend_downloads"]:
            if backend["name"] in {"community-modules", "z3"}:
                backend["locked_output_sha256"] = fixture_sha256
    fixture_lock_path = tmp_path / "fixture-lock.json"
    fixture_lock_path.write_text(
        json.dumps(fixture_lock, indent=2) + "\n", encoding="utf-8"
    )

    build_tree = tmp_path / "build-tree"
    distribution_tree = tmp_path / "distribution"

    def materialize(root: Path, relative: str, *, executable: bool = False) -> Path:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(fixture_bytes)
        if executable:
            path.chmod(0o500)
        return path

    package_root = distribution_tree / "tlapm"
    for backend in fixture_lock["platforms"]["arm64-darwin"]["backend_downloads"]:
        if backend["derivation_kind"] == "file":
            executable = backend["name"] in {"ls4", "z3"}
            materialize(package_root, backend["package_path"], executable=executable)
            materialize(build_tree, backend["build_path"], executable=executable)
        else:
            for root, relative in ((package_root, backend["package_path"]), (build_tree, backend["build_path"])):
                materialize(root, f"{relative}/bin/isabelle", executable=True)
    for path in (package_root / "lib/tlapm/backends/Isabelle.exec-files", build_tree / "_build/default/deps/isabelle/Isabelle.exec-files"): path.write_text("Isabelle/bin/isabelle\n", encoding="utf-8")
    for build_relative, package_relative in (
        ("_build/default/translate/main.exe", "lib/tlapm/backends/bin/ptl_to_trp"),
        ("_build/default/deps/zenon/zenon", "lib/tlapm/backends/bin/zenon"),
    ):
        materialize(package_root, package_relative, executable=True)
        materialize(build_tree, build_relative, executable=True)

    archive = tmp_path / "source-built.tar.gz"; archive.write_bytes(fixture_bytes)
    attestation = tmp_path / "attestation.json"
    locked_wget = formal_scripts / "sumeragi_v2_tlapm_locked_wget.sh"
    source_builder = formal_scripts / "build_sumeragi_v2_tlapm_from_source.sh"
    common = (fixture_lock_path, "arm64-darwin")
    written = invoke(*common, "write-attestation", "--archive", archive,
        "--build-tree", build_tree, "--distribution-tree", distribution_tree,
        "--locked-wget", locked_wget, "--source-builder", source_builder, "--output", attestation)
    assert written.returncode == 0, written.stderr
    verify_attestation = ("verify-attestation", "--archive", archive,
        "--distribution-tree", distribution_tree, "--locked-wget", locked_wget,
        "--source-builder", source_builder, "--attestation", attestation)
    verified = invoke(*common, *verify_attestation)
    assert verified.returncode == 0, verified.stderr

    install = tmp_path / "install"
    install.mkdir(mode=0o700)
    shutil.copytree(distribution_tree / "tlapm", install / "tlapm")
    shutil.copyfile(fixture_lock_path, install / "source-build-lock.json")
    shutil.copyfile(attestation, install / "source-build-attestation.json")
    archive_sha256 = hashlib.sha256(archive.read_bytes()).hexdigest()
    (install / "archive.sha256").write_text(archive_sha256 + "\n", encoding="utf-8")
    (install / "archive.origin").write_text("immutable-source-build\n", encoding="utf-8")
    state = install / "install-state.json"
    state_written = invoke(*common, "write-install-state", "--directory", install,
        "--origin", "immutable-source-build", "--archive-sha256", archive_sha256,
        "--attestation", install / "source-build-attestation.json",
        "--locked-wget", locked_wget, "--source-builder", source_builder, "--output", state)
    assert state_written.returncode == 0, state_written.stderr
    verify_install_command = [
        "verify-install",
        "--directory", install, "--allowed-origin", "immutable-source-build",
        "--prebuilt-sha256", "0" * 64, "--locked-wget", locked_wget, "--source-builder", source_builder,
    ]
    install_verified = invoke(*common, *verify_install_command)
    assert install_verified.returncode == 0, install_verified.stderr

    installed_community = install / "tlapm/lib/tlapm/stdlib/CommunityModules.jar"
    installed_community.write_bytes(b"forged cache closure\n")
    forged_install = invoke(*common, *verify_install_command)
    assert forged_install.returncode != 0
    assert "locked archive member" in forged_install.stderr
    installed_community.write_bytes(fixture_bytes)

    mutated = json.loads(attestation.read_text(encoding="utf-8"))
    mutated["source_tree"] = "0" * 40
    attestation.chmod(0o600)
    attestation.write_text(
        json.dumps(mutated, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    rejected = invoke(*common, *verify_attestation)
    assert rejected.returncode != 0
    assert "does not match the lock and archive" in rejected.stderr


def test_tlapm_locked_wget_is_exact_consuming_and_fail_closed(
    tmp_path: Path,
) -> None:
    tmp_path = tmp_path.resolve(strict=True)
    tmp_path.chmod(0o700)
    formal_scripts = ROOT_DIR / "scripts" / "formal"
    lock_path = formal_scripts / "sumeragi_v2_tlapm_source_build_lock.json"
    helper = formal_scripts / "sumeragi_v2_tlapm_source_lock.py"
    locked_wget = formal_scripts / "sumeragi_v2_tlapm_locked_wget.sh"
    fixture = b"locked backend fixture\n"
    fixture_digest = hashlib.sha256(fixture).hexdigest()
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    backends = lock["platforms"]["arm64-darwin"]["backend_downloads"]
    for backend in backends:
        backend["sha256"] = fixture_digest
        if backend["name"] in {"community-modules", "z3"}:
            backend["locked_output_sha256"] = fixture_digest
    fixture_lock = tmp_path / "fixture-lock.json"
    fixture_lock.write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")

    cache = tmp_path / "cache"
    receipts = tmp_path / "receipts"
    output_root = tmp_path / "output"
    for directory in (cache, receipts, output_root):
        directory.mkdir(mode=0o700)
    base = [sys.executable, "-I", "-S", str(helper), "--lock", str(fixture_lock),
        "--platform", "arm64-darwin", "serve-wget", "--cache-dir", str(cache),
        "--output-root", str(output_root), "--receipt-dir", str(receipts), "--"]
    def run(command: list[str], cwd: Path):
        return subprocess.run(command, cwd=cwd, check=False, capture_output=True,
            text=True, timeout=10)

    reviewed: dict[str, tuple[list[str], Path]] = {}
    for backend in backends:
        cache_file = cache / backend["destination"]
        cache_file.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
        cache_file.write_bytes(fixture)
        cache_file.chmod(0o400)
        working = output_root / "_build/.sandbox/reviewed" / backend["working_suffix"]
        working.mkdir(parents=True, exist_ok=True)
        arguments = ["--progress=dot:giga"] if backend["progress_dot_giga"] else []
        if backend["directory_prefix"] is not None:
            prefix = output_root / backend["directory_prefix"]
            prefix.mkdir(mode=0o700)
            arguments.append(f"--directory-prefix={prefix}")
        command = [*base, *arguments, backend["requested_url"]]
        accepted = run(command, working)
        assert accepted.returncode == 0, accepted.stderr
        assert (receipts / f"{backend['name']}.json").is_file()
        destination_parent = prefix if backend["directory_prefix"] is not None else working
        assert (destination_parent / Path(backend["destination"]).name).read_bytes() == fixture
        reviewed[backend["name"]] = (command, working)

    command, working = reviewed["community-modules"]
    duplicate = run(command, working)
    assert duplicate.returncode != 0
    assert "destination already exists" in duplicate.stderr

    wrong_url = run([*command[:-1], "https://example.invalid/CommunityModules.jar"], working)
    assert wrong_url.returncode != 0
    assert "rejects unreviewed URL" in wrong_url.stderr

    wrong_working = output_root / "unreviewed"
    wrong_working.mkdir()
    wrong_cwd = run(command, wrong_working)
    assert wrong_cwd.returncode != 0
    assert "rejects the working directory" in wrong_cwd.stderr

    snapshot = tmp_path / "snapshot"
    snapshotted = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(helper),
            "--lock",
            str(lock_path),
            "--platform",
            "arm64-darwin",
            "snapshot-corridor",
            "--helper",
            str(helper),
            "--locked-wget",
            str(locked_wget),
            "--source-builder", str(locked_wget.with_name("build_sumeragi_v2_tlapm_from_source.sh")),
            "--output-dir",
            str(snapshot),
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert snapshotted.returncode == 0, snapshotted.stderr
    assert (snapshot / "source-build-lock.json").read_bytes() == lock_path.read_bytes()
    assert (snapshot / "source-lock.py").read_bytes() == helper.read_bytes()
    assert (snapshot / "locked-wget.sh").read_bytes() == locked_wget.read_bytes()
    assert (snapshot / "source-builder.sh").read_bytes() == locked_wget.with_name("build_sumeragi_v2_tlapm_from_source.sh").read_bytes()

    public_output = tmp_path / "public-output"
    public_output.mkdir(mode=0o755)
    host_platform = "arm64-darwin" if sys.platform == "darwin" else "x86_64-linux-gnu"
    frozen_preflight = subprocess.run(["/bin/bash", str(snapshot / "source-builder.sh"),
        host_platform, str(public_output / "bundle"), str(snapshot), str(ROOT_DIR)],
        check=False, capture_output=True, text=True, timeout=10)
    assert frozen_preflight.returncode != 0
    assert "owner-private mode-0700 directory" in frozen_preflight.stderr


def test_tlapm_source_builder_checks_every_locked_boundary() -> None:
    formal_scripts = ROOT_DIR / "scripts" / "formal"
    builder = (formal_scripts / "build_sumeragi_v2_tlapm_from_source.sh").read_text(encoding="utf-8")
    helper = (formal_scripts / "sumeragi_v2_tlapm_source_lock.py").read_text(encoding="utf-8")
    normalized = " ".join(builder.replace("\\\n", "").split())

    assert (
        'readonly EXPECTED_TLAPM_COMMIT="'
        '3ab43c7ff31db4ced850619d4746fa4c841a7681"'
    ) in builder
    assert (
        'readonly EXPECTED_OCAML_COMPILER_ATOM="'
        'ocaml-base-compiler.5.1.0"'
    ) in builder
    assert 'git -C "$destination" rev-parse --verify HEAD' in builder
    assert "rev-parse --verify 'HEAD^{tree}'" in builder
    assert (
        '[[ "$actual_commit" == "$commit" && "$actual_tree" == "$tree" ]]'
        in builder
    )
    assert (
        'git -C "$destination" status --porcelain=v1 --untracked-files=no'
        in builder
    )
    assert "opam-deps" not in builder
    assert (
        'opam_command install --switch "$OPAM_SWITCH" --yes '
        '"${build_packages[@]}" < /dev/null'
    ) in normalized
    assert builder.count("verify_package_set") >= 4
    assert 'if ! diff -u "$sorted_expected" "$actual_atoms"; then' in builder
    assert builder.count('verify_exact_tree "TLAPM source pin"') >= 2 and builder.count("verify_build_source_checkout") >= 4
    assert builder.count('verify_exact_tree "opam repository"') >= 2
    assert 'verify_checked_file "$backend_name" "$backend_sha256"' in normalized
    assert 'make --jobs=1 -C "$SOURCE_DIR" release' in normalized
    assert 'checkout_exact_tree "TLAPM build source" "$SOURCE_PIN_DIR"' in normalized and 'git -C "$SOURCE_PIN_DIR" archive' not in normalized
    assert 'readonly BACKEND_CACHE="${tmp_dir}/backend-cache"' in builder
    assert 'cp "$LOCKED_WGET" "${CONTROLLED_BIN}/wget"' in builder
    assert "verify-wget-receipts" in builder
    assert "locked Z3 4.8.9 runtime cannot execute" in builder
    assert '"$ENV_BIN" -i' in builder
    assert "OPAMREQUIRECHECKSUMS=true" in builder
    assert '"$OPAM_BINARY" "$@"' in builder
    assert "--require-checksums" not in builder
    opam_subcommands = re.findall(r"\bopam_command\s+([A-Za-z0-9_-]+)", builder)
    assert opam_subcommands == [
        "list", "init", "switch", "var", "install", "install", "exec", "exec"
    ]
    assert (
        'opam_command init --bare --no-setup --disable-sandboxing locked '
        '"$OPAM_REPOSITORY_DIR"'
    ) in normalized
    for scrubbed in (
        "-u MAKEFLAGS",
        "-u MFLAGS",
        "-u GNUMAKEFLAGS",
        "-u DUNE_CACHE_ROOT",
        "-u OPAMFETCH",
        "-u OPAMNOCHECKSUMS",
        "DUNE_CACHE=disabled",
    ):
        assert scrubbed in builder
    assert "snapshot-corridor" in builder
    assert "changed during the long build" in builder
    assert "publish-output-bundle" in builder
    assert "renameatx_np" in helper and "renameat2" in helper and "dir_fd=" in helper
    assert 'rm -f -- "$OUTPUT_ARCHIVE"' not in builder
    assert 'rm -f -- "$OUTPUT_ATTESTATION"' not in builder
    assert "write-attestation" in builder
    assert "verify-attestation" in builder
    assert "byte_reproducibility_claimed" not in builder
    clean_body = " ".join(builder[builder.index("clean_command() {"):builder.index("\n}\n\nfor required_command", builder.index("clean_command() {")) + 2].replace("\\\n", "").split())
    assert clean_body == 'clean_command() { "$ENV_BIN" -i HOME="$BUILD_HOME" PATH="$SANITIZED_HOST_PATH" TMPDIR="$BUILD_TMP" XDG_CACHE_HOME="$BUILD_XDG_CACHE" XDG_CONFIG_HOME="$BUILD_XDG_CONFIG" LANG=C LC_ALL=C TZ=UTC GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 GIT_NO_REPLACE_OBJECTS=1 "$@" }'
    darwin_guard = '\n  [[ "$PLATFORM" == "arm64-darwin" ]] || return 0'
    assert all(f"{name}() {{{darwin_guard}" in builder for name in ("prepare_darwin_conf_boundary", "verify_darwin_depext_capabilities"))
    darwin_probes = ('DARWIN_CXXFLAGS=""', 'DARWIN_CPLUS_INCLUDE_PATH=""', 'clean_command sh -c \'command -v "$1" >/dev/null 2>&1\' sh pkg-config', "conf-*) darwin_conf_packages", "${#darwin_conf_packages[@]} -eq 3", "[[ -x /usr/bin/xcrun && ! -L /usr/bin/xcrun ]]", "clean_command /usr/bin/xcrun --sdk macosx --show-sdk-path", 'darwin_sdk_root="$(cd -P -- "$darwin_sdk_root" && pwd)"', 'darwin_cxx_include="${darwin_sdk_root}/usr/include/c++/v1"', '! -L "${darwin_cxx_include}/numeric"', 'DARWIN_CXXFLAGS="-isystem ${darwin_cxx_include}"', 'DARWIN_CPLUS_INCLUDE_PATH="$darwin_cxx_include"', 'clean_command "$ENV_BIN" CPLUS_INCLUDE_PATH="$DARWIN_CPLUS_INCLUDE_PATH"', "cc -std=c++17 -Wall -Wextra -Werror -pedantic", '-x c++ -c - -o "$DARWIN_CXX_PREFLIGHT_OBJECT"', '! -L "$DARWIN_CXX_PREFLIGHT_OBJECT"', 'clean_command g++ "$DARWIN_CXX_PREFLIGHT_OBJECT" -o "$DARWIN_CXX_PREFLIGHT"', "static_assert(__cplusplus == 201703L", "std::vector<int>", "std::accumulate(", 'clean_command "$DARWIN_CXX_PREFLIGHT"', "clean_command pkg-config --exists zlib", "strcmp(ZLIB_VERSION, zlibVersion())", "compress2(", "clean_command cc -std=c11 -Wall -Wextra -Werror -pedantic", '-x c - -lz -o "$DARWIN_ZLIB_PREFLIGHT"', 'clean_command "$DARWIN_ZLIB_PREFLIGHT"', 'readonly DARWIN_CXXFLAGS', 'readonly DARWIN_CPLUS_INCLUDE_PATH')
    assert all(fragment in builder for fragment in darwin_probes)
    assert builder.count('CXXFLAGS="$DARWIN_CXXFLAGS"') == 3
    assert builder.count('CPLUS_INCLUDE_PATH="$DARWIN_CPLUS_INCLUDE_PATH"') == 4
    assert re.findall(r'darwin_conf_packages\[[0-9]+\]}" == "([^"]+)"', builder) == ["conf-g++.1.0", "conf-pkg-config.5", "conf-zlib.1"]
    assumed_install = 'opam_command install --assume-depexts --switch "$OPAM_SWITCH" --yes "${darwin_conf_packages[@]}" < /dev/null'
    complete_install = 'opam_command install --switch "$OPAM_SWITCH" --yes "${build_packages[@]}" < /dev/null'
    assert normalized.count(assumed_install) == normalized.count(complete_install) == builder.count("--assume-depexts") == 1
    preflight_prefix = builder[:builder.index("\nverify_darwin_depext_capabilities\n")]
    darwin_helper_source = builder[builder.index("prepare_darwin_conf_boundary() {"):builder.index("\ncheckout_exact_tree() {")]
    assert re.search(r"(?m)^(?:(?:clean_command[ \t]+)?curl\b|(?:clean_command[ \t]+)?git\b[^\n]*\bfetch\b|(?:checkout_exact_tree|download_checked|opam_command)[ \t]+)", preflight_prefix) is None and re.search(r"\b(?:curl|checkout_exact_tree|download_checked|opam_command)\b|\bgit\b[^\n]*\bfetch\b", darwin_helper_source) is None and hashlib.sha256(darwin_helper_source.encode()).hexdigest() == "c09ede2f51ae0c2f848b76c63d14b2599ba463c5981f723fc275707d5898d290"
    depext_order = ('prepare_darwin_conf_boundary readonly -a darwin_conf_packages verify_darwin_depext_capabilities readonly DARWIN_CXXFLAGS readonly DARWIN_CPLUS_INCLUDE_PATH echo "[tlapm] fetching immutable source commit', 'if [[ "$PLATFORM" == "arm64-darwin" ]]; then echo "[tlapm] validating the exact Darwin host capability packages" ' + assumed_install, 'verify_package_set darwin-conf "$DARWIN_INTERMEDIATE_ATOMS"', complete_install, 'verify_package_set complete "$EXPECTED_ATOMS"')
    assert [normalized.index(fragment) for fragment in depext_order] == sorted(normalized.index(fragment) for fragment in depext_order)
    assert 'clean_command cp "$COMPILER_ATOMS" "$DARWIN_INTERMEDIATE_ATOMS"' in builder and 'printf \'%s\\n\' "${darwin_conf_packages[@]}" >> "$DARWIN_INTERMEDIATE_ATOMS"' in normalized
    assert all(forbidden not in builder for forbidden in ("OPAMASSUMEDEPEXTS", "OPAMDEPEXTS", "opam option depext=false")) and re.search(r"\b(?:brew|sudo)\b", builder) is None
    clean_projection = (
        'readonly AUTHENTICATED_DISTRIBUTION_PARENT="${tmp_dir}/authenticated-distribution"',
        'dune install --root "$SOURCE_DIR" --relocatable',
        '--prefix "$AUTHENTICATED_DISTRIBUTION"',
        'clean_command make --jobs=1 -C "$AUTHENTICATED_DISTRIBUTION/lib/tlapm"',
        'COPYFILE_DISABLE=1',
        'tar -czf "$BUILT_ARCHIVE" -C "$AUTHENTICATED_DISTRIBUTION_PARENT" tlapm',
        '--distribution-tree "$AUTHENTICATED_DISTRIBUTION_PARENT"',
    )
    assert all(fragment in builder for fragment in clean_projection)
    assert '--distribution-tree "${SOURCE_DIR}/_build"' not in builder


def test_tlapm_publication_is_atomic_no_replace_and_preserves_winner(
    tmp_path: Path,
) -> None:
    tmp_path = tmp_path.resolve(strict=True)
    tmp_path.chmod(0o700)
    formal_scripts = ROOT_DIR / "scripts" / "formal"
    lock = formal_scripts / "sumeragi_v2_tlapm_source_build_lock.json"
    helper = formal_scripts / "sumeragi_v2_tlapm_source_lock.py"
    common = [
        sys.executable,
        "-I",
        "-S",
        str(helper),
        "--lock",
        str(lock),
        "--platform",
        "arm64-darwin",
    ]
    archive = tmp_path / "archive.tar.gz"
    attestation = tmp_path / "attestation.json"
    archive.write_bytes(b"archive winner\n")
    attestation.write_bytes(b"attestation winner\n")
    bundle = tmp_path / "bundle"
    publish_bundle = [
        *common,
        "publish-output-bundle",
        "--archive",
        str(archive),
        "--attestation",
        str(attestation),
        "--output-bundle",
        str(bundle),
    ]
    published = subprocess.run(
        publish_bundle,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert published.returncode == 0, published.stderr
    winner = (bundle / "archive.tar.gz").read_bytes()
    raced = subprocess.run(
        publish_bundle,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert raced.returncode == 3
    assert (bundle / "archive.tar.gz").read_bytes() == winner

    failed_bundle = tmp_path / "failed-bundle"
    failed_command = [str(tmp_path / "missing-attestation") if argument == str(attestation)
        else str(failed_bundle) if argument == str(bundle) else argument for argument in publish_bundle]
    failed = subprocess.run(failed_command, check=False, capture_output=True, text=True, timeout=10)
    assert failed.returncode != 0 and not failed_bundle.exists()
    assert not tuple(tmp_path.glob(".failed-bundle.*.stage"))

    install_stage = tmp_path / "install-stage"
    install_stage.mkdir(mode=0o700)
    (install_stage / "winner").write_bytes(b"first\n")
    install = tmp_path / "installed"
    publish_install = [
        *common,
        "publish-install",
        "--staged",
        str(install_stage),
        "--destination",
        str(install),
    ]
    first_install = subprocess.run(
        publish_install,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert first_install.returncode == 0, first_install.stderr
    second_stage = tmp_path / "second-stage"
    second_stage.mkdir(mode=0o700)
    (second_stage / "winner").write_bytes(b"second\n")
    second_install = subprocess.run(
        [
            *common,
            "publish-install",
            "--staged",
            str(second_stage),
            "--destination",
            str(install),
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert second_install.returncode == 3
    assert (install / "winner").read_bytes() == b"first\n"


def test_prospective_native_wal_serve_release_registrations_are_exact_and_nonignored() -> None:
    """Registration is source inventory, independent of Rust execution evidence."""
    module = load_checker()
    source = (ROOT_DIR / "scripts/run_sumeragi_v2_release_gates.sh").read_text(encoding="utf-8")
    inventory = source.split("required_production_liveness_tests=(\n", 1)[1].split("\n)", 1)[0].split()
    registrations = (
        ('native_amx::participant_application_role_tests::participant_application_role_classifies_exact_routes_and_incarnations', 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs'),
        ('native_amx::participant_application_role_tests::participant_application_role_keeps_each_route_coordinate_distinct', 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs'),
        ('native_amx::participant_application_role_tests::participant_application_role_rejects_independent_prepare_and_commit_identity_drift', 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs'),
        ('native_amx::participant_application_role_tests::participant_application_role_rejects_coherent_same_route_coordinator_drift', 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs'),
        ('native_amx::participant_application_role_tests::participant_application_role_rejects_settlement_identity_and_content_tampering', 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs'),
        ('native_amx::participant_application_role_tests::participant_application_lookup_validates_later_legs_after_an_exact_match', 'crates/iroha_core/src/native_amx/participant_application_role_tests.rs'),
        ('sumeragi::v2::tests::ready_validate_crash_after_wal_append_replays_exact_prepare_and_commit', 'crates/iroha_core/src/sumeragi/tests/v2_adapter_05_direct_lifecycle.rs'),
        ('sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::certified_serve_worker_rejects_corrupt_owned_body_after_receipt_mint', 'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests_durable_recovery_02.rs'),
        ('sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::terminal_owner_faults_on_corrupt_payload_after_worker_readback', 'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests_durable_recovery_02.rs'),
        ('sumeragi::v2_worker::tests::production_exact_output_observes_finality_only_after_state_commit', 'crates/iroha_core/src/sumeragi/tests/v2_worker_backpressure_retirement_cases.rs'),
        ('sumeragi::v2_worker::tests::applied_height_finality_releases_only_ticketless_global_topology_target', 'crates/iroha_core/src/sumeragi/tests/v2_worker_backpressure_retirement_cases.rs'),
    )
    for qualified, relative in registrations:
        assert inventory.count(qualified) == 1
        assert module._PRODUCTION_LIVENESS_NEW_REGRESSIONS.count(qualified) == 1
        rust_source = (ROOT_DIR / relative).read_text(encoding="utf-8")
        tests = module.rust_items(rust_source, qualified.rsplit("::", 1)[1])
        assert len(tests) == 1, qualified
        assert tests[0].brace_context == (), qualified
        assert tuple(module.rust_code_tokens(attribute) for attribute in tests[0].attributes) == (
            module.rust_code_tokens("#[test]"),
        ), qualified
    assert module._PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS.count((
        "production-native-amx-participant-application",
        "native_amx::participant_application_role_tests", 6,
    )) == 1
    native_parent = (ROOT_DIR / "crates/iroha_core/src/native_amx.rs").read_text(encoding="utf-8")
    assert native_parent.count(
        '#[cfg(test)]\n#[path = "native_amx/participant_application_role_tests.rs"]\nmod participant_application_role_tests;'
    ) == 1
    ledger_parent = (ROOT_DIR / "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests.rs").read_text(encoding="utf-8")
    assert '#[cfg(feature = "bls")]' in ledger_parent
    assert 'pub(crate) mod durable_ready_fetch_recovery {' in ledger_parent
    assert 'include!("v2_lifecycle_ledger_tests_durable_recovery_02.rs");' in ledger_parent


def test_prospective_kagemusha_boundary_registrations_are_exact_and_feature_bound() -> None:
    """The nested BLS tests are registered source, not passing execution evidence."""
    module = load_checker()
    runner_source = (
        ROOT_DIR / "scripts/run_sumeragi_v2_release_gates.sh"
    ).read_text(encoding="utf-8")
    inventory = runner_source.split(
        "required_production_liveness_tests=(\n", 1
    )[1].split("\n)", 1)[0].split()
    test_source = (
        ROOT_DIR / "crates/iroha_core/src/sumeragi/tests/v2_adapter_main_04.rs"
    ).read_text(encoding="utf-8")
    module_declaration = (
        '#[cfg(feature = "bls")]\nmod kagemusha_finality_boundary {'
    )
    assert test_source.count(module_declaration) == 1
    expected_context = (
        module.rust_code_tokens(module_declaration.removesuffix(" {")),
    )
    for name in (
        "commit_vote_binds_round_statement_signer_and_both_signatures",
        "commit_qc_binds_round_statement_exact_quorum_and_both_signatures",
    ):
        qualified = f"sumeragi::v2::tests::kagemusha_finality_boundary::{name}"
        assert inventory.count(qualified) == 1
        assert module._PRODUCTION_LIVENESS_NEW_REGRESSIONS.count(qualified) == 1
        items = module.rust_items(test_source, name)
        assert len(items) == 1, qualified
        assert items[0].brace_context == expected_context, qualified
        assert items[0].attributes == ("#[test]",), qualified
    assert module._PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS.count(
        ("production-v2-adapter", "sumeragi::v2::tests", 52)
    ) == 1



def test_prospective_historical_hydration_registrations_and_declaration_controls() -> None:
    """Two current Rust declarations are prospective inventory, not runtime evidence."""
    module = load_checker()
    errors: list[str] = []
    path, source = module._read_reviewed_rust_source(
        ROOT_DIR, "crates/iroha_core/src/sumeragi/v2_lane_work.rs", errors,
        "prospective historical hydration registration",
    )
    assert not errors, errors
    assert module._historical_hydration_registration_source_errors(path, source) == []
    runner = (ROOT_DIR / "scripts/run_sumeragi_v2_release_gates.sh").read_text(encoding="utf-8")
    inventory = runner.split("required_production_liveness_tests=(\n", 1)[1].split("\n)", 1)[0].split()
    for name in (
        "historical_autonomous_hydration_replaces_same_slot_conflict_at_capacity",
        "historical_autonomous_hydration_preserves_conflicting_quorum_at_capacity",
    ):
        qualified = "sumeragi::v2_lane_work::tests::" + name
        assert inventory.count(qualified) == 1
        assert module._PRODUCTION_LIVENESS_NEW_REGRESSIONS.count(qualified) == 1
        items = module.rust_items(source, name)
        assert len(items) == 1
        item = items[0]
        for mutated in (
            source.replace(item.source, "", 1),
            source.replace(item.source, "#[ignore]\n" + item.source, 1),
            source.replace(item.source, '#[cfg(feature = "unqualified")]\n' + item.source, 1),
        ):
            assert module._historical_hydration_registration_source_errors(path, mutated)
    assert module._PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS.count(
        ("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 65)
    ) == 1
