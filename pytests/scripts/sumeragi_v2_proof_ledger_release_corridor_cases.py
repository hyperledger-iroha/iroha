# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

def test_release_inventory_constants_match_current_source_seal(
    tmp_path: Path,
) -> None:
    """Every release consumer binds the current production and focus seals."""

    module = load_checker()
    assert module._PRODUCTION_LIVENESS_RELEASE_COUNT == 864
    assert module._PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256 == (
        "23325cb037bc930c7503986845dbb25891ef80af6f08092533b1e0e1d8233fad"
    )
    assert module._PRODUCTION_LIVENESS_INVENTORY_GUARD_SHA256 == (
        "355564c335110edc2811b8dd3542305ebf1dac3f269e2bb22ac758c0fea93cbd"
    )
    assert module._SUMERAGI_V2_PACKAGE_LAYOUT_GUARD_SHA256 == (
        "e99da2c824b86930b76c741d2f7aa47ab16092c2f84e43550fb6362a36133268"
    )
    assert module._SUMERAGI_V2_PACKAGE_LAYOUT_VERIFIER_SHA256 == (
        "42fc1fb789e115df9f54c230ee6bfc1e1c20504a904aa20f945b6369df6d7679"
    )
    assert module._PRODUCTION_MULTILANE_FOCUS_TEST_COUNT == 530
    assert module._PRODUCTION_MULTILANE_G_UNIT_TSV_LINE_COUNT == 531
    assert module._PRODUCTION_MULTILANE_FOCUS_INVENTORY_SHA256 == (
        "e7eb7d609b110a421297d740f0a69cc2b01f2083731d29ca604826849bb36474"
    )
    assert module._PRODUCTION_LIFECYCLE_INGRESS_PUBLICATION_FENCE_ITEM_SHA256 == {
        "PreparedFairIngressQueueWitness::lock_exact_dequeue_retaining": (
            "66d33b07c062bd6dc4a1b879b0b3624bc0403e59305cbc44763d409f97d109fc"
        ),
        "LockedPreparedFairIngressExactDequeue::commit": (
            "2df7516317611dcc3fc0f959cca1e80a7b6aa3670a90d2add798f744cfebbd4c"
        ),
        "locked_publication_fence_serializes_same_wire_and_reenqueues_after_commit": (
            "da01b212e5b3db1163c100f1088e943c074aa124c2f225a4c068052d79e499f9"
        ),
        "locked_publication_fence_serializes_unrelated_append_and_preserves_it": (
            "3f863e16e284ffd980a06b45cd16d3ee4cfd4559a8f7ff23687a55d32ff7481b"
        ),
        "dropping_locked_publication_fence_releases_producer_without_dequeue": (
            "4375aa5205367018773fec586aefb2c44940f502e6de1ae94b8e6e221a350d6f"
        ),
    }
    assert (
        "_production_liveness_release_inventory_guard_errors"
        in module._production_liveness_release_inventory_errors.__code__.co_names
    )
    assert module._sumeragi_v2_package_layout_guard_errors(ROOT_DIR) == []

    package_root = tmp_path / "package-layout"
    package_guard = package_root / "scripts" / "check_sumeragi_v2_package_layout.sh"
    package_verifier = package_root / "scripts" / "verify_sumeragi_v2.sh"
    package_core_root = (
        package_root / "crates" / "iroha_core" / "src" / "sumeragi"
    )
    package_guard.parent.mkdir(parents=True)
    package_core_root.mkdir(parents=True)
    shutil.copy2(
        ROOT_DIR / "scripts" / "check_sumeragi_v2_package_layout.sh",
        package_guard,
    )
    shutil.copy2(ROOT_DIR / "scripts" / "verify_sumeragi_v2.sh", package_verifier)
    shutil.copy2(
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_core.rs",
        package_core_root / "v2_core.rs",
    )
    shutil.copytree(
        ROOT_DIR
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_core",
        package_core_root / "v2_core",
    )
    bash = shutil.which("bash")
    assert bash is not None
    baseline = subprocess.run(
        [bash, str(package_guard)],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert baseline.returncode == 0, baseline.stderr

    refinement = package_core_root / "v2_core" / "refinement.rs"
    refinement_source = refinement.read_text(encoding="utf-8")
    layout_mutations = (
        (
            refinement_source + '\n#[path = "shadow.rs"]\nmod shadow;\n',
            "second path attribute",
        ),
        (
            refinement_source.replace(
                '#[path = "refinement_cases.rs"]',
                '#[path = "../refinement_cases.rs"]',
                1,
            ),
            "parent-relative path attribute",
        ),
        (
            refinement_source.replace(
                '#[cfg(test)]\n#[path = "refinement_cases.rs"]',
                '#[path = "refinement_cases.rs"]',
                1,
            ),
            "non-test path attribute",
        ),
    )
    for mutation, description in layout_mutations:
        refinement.write_text(mutation, encoding="utf-8")
        result = subprocess.run(
            [bash, str(package_guard)],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode != 0, description
        assert (
            "only the reviewed package-local refinement test split and "
            "identity-preserving nested include"
            in result.stderr
        )
    refinement.write_text(refinement_source, encoding="utf-8")

    refinement_cases = package_core_root / "v2_core" / "refinement_cases.rs"
    refinement_cases_source = refinement_cases.read_text(encoding="utf-8")
    nested_include = 'include!("refinement_cases/terminal_body_pipeline.rs");'
    assert refinement_cases_source.count(nested_include) == 1
    refinement_cases.write_text(
        refinement_cases_source.replace(
            nested_include,
            '#[path = "refinement_cases/terminal_body_pipeline.rs"]\n'
            "mod terminal_body_pipeline;",
            1,
        ),
        encoding="utf-8",
    )
    nested_result = subprocess.run(
        [bash, str(package_guard)],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert nested_result.returncode != 0, "nested module identity drift"
    assert (
        "only the reviewed package-local refinement test split and "
        "identity-preserving nested include"
        in nested_result.stderr
    )
    refinement_cases.write_text(refinement_cases_source, encoding="utf-8")

    package_guard_source = package_guard.read_text(encoding="utf-8")
    package_guard.write_text(
        package_guard_source.replace("set -euo pipefail", "set +e", 1),
        encoding="utf-8",
    )
    errors = module._sumeragi_v2_package_layout_guard_errors(package_root)
    assert any(
        "package-layout guard source SHA-256 must equal" in error
        for error in errors
    ), errors
    package_guard.write_text(package_guard_source, encoding="utf-8")

    invocation = 'bash "$REPO_ROOT/scripts/check_sumeragi_v2_package_layout.sh"'
    verifier_source = package_verifier.read_text(encoding="utf-8")
    assert verifier_source.splitlines().count(invocation) == 1
    package_verifier.write_text(
        verifier_source.replace(invocation, "true # skipped package-layout guard", 1),
        encoding="utf-8",
    )
    errors = module._sumeragi_v2_package_layout_guard_errors(package_root)
    assert any(
        "must invoke the package-layout guard exactly once" in error
        for error in errors
    ), errors

    checker_source = SCRIPT.read_text(encoding="utf-8")
    validate_body = checker_source.split("def validate_ledger(", 1)[1].split(
        "\ndef ",
        1,
    )[0]
    assert (
        validate_body.count(
            "errors.extend(_sumeragi_v2_package_layout_guard_errors(ROOT_DIR))"
        )
        == 1
    )

    receipt_spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_release_receipt_current_inventory",
        ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py",
    )
    assert receipt_spec is not None
    assert receipt_spec.loader is not None
    receipt_module = importlib.util.module_from_spec(receipt_spec)
    sys.modules[receipt_spec.name] = receipt_module
    receipt_spec.loader.exec_module(receipt_module)
    assert receipt_module._PRODUCTION_TEST_COUNT == 864
    assert receipt_module._G_UNIT_TEST_COUNT == 530
    assert sum(count for _, _, count in receipt_module._PRODUCTION_MODULES) == 864
    receipt_module_counts = {
        module_name: count
        for _leg_id, module_name, count in receipt_module._PRODUCTION_MODULES
    }
    assert receipt_module_counts["kura::tests"] == 18
    assert receipt_module_counts["sumeragi::authoritative_runtime_gate_tests"] == 42
    assert receipt_module_counts["sumeragi::v2::tests"] == 48
    assert receipt_module_counts["sumeragi::v2_effects::tests"] == 71
    assert receipt_module_counts["sumeragi::v2_lane_work::tests"] == 63
    assert receipt_module_counts["sumeragi::v2_runtime::tests"] == 65
    assert receipt_module_counts["sumeragi::v2_certified_serve_payload_store::tests"] == 11
    assert receipt_module_counts["sumeragi::v2_lifecycle_coordinator"] == 39
    assert receipt_module_counts["sumeragi::v2_runner::tests"] == 37
    assert receipt_module_counts["sumeragi::v2_runner::lifecycle_height_driver::tests"] == 1
    assert receipt_module_counts["sumeragi::v2_worker::tests"] == 88
    assert "sumeragi::v2_core::network_simulation" not in receipt_module_counts
    assert (
        sum(count for _, _, _, count, _ in receipt_module._G_UNIT_GROUPS)
        == 530
    )

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

def test_release_corridor_rejects_network_skips_and_zero_test_filters(
    tmp_path: Path,
) -> None:
    module = load_checker()

    def read_source_bundle(*relative_paths: str) -> str:
        """Read one Rust module together with its lexically included test files."""
        return "\n".join(
            (ROOT_DIR / relative_path).read_text(encoding="utf-8")
            for relative_path in relative_paths
        )

    def read_reviewed_source_bundle(relative_path: str) -> str:
        """Mirror the complete source-sealed include manifest for one Rust parent."""
        errors: list[str] = []
        _path, source = module._read_reviewed_rust_source(
            ROOT_DIR,
            relative_path,
            errors,
            f"release-corridor reviewed Rust source {relative_path}",
        )
        assert errors == []
        assert source
        return source

    seed_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_seed_matrix.sh"
    ).read_text(encoding="utf-8")
    release_parent_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    ).read_text(encoding="utf-8")
    release_support_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates_support.sh"
    ).read_text(encoding="utf-8")
    release_support_loader = (
        'source "${repo_root}/scripts/run_sumeragi_v2_release_gates_support.sh"'
    )
    assert release_parent_source.count(release_support_loader) == 1
    release_source = release_parent_source.replace(
        release_support_loader,
        f"{release_support_loader}\n{release_support_source}",
    )
    harness_source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    ).read_text(encoding="utf-8")
    formal_launcher_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_formal_release.sh"
    ).read_text(encoding="utf-8")
    receipt_source = read_source_bundle(
        "scripts/write_sumeragi_v2_release_receipt.py",
        "scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py",
        "scripts/write_sumeragi_v2_release_receipt_corridor_log.py",
        "scripts/write_sumeragi_v2_release_receipt_gate_evidence.py",
        "scripts/write_sumeragi_v2_release_receipt_publication.py",
    )
    taira_source = read_reviewed_source_bundle(
        "integration_tests/tests/taira_public_localnet.rs"
    )
    taira_strict_restart_source = (
        ROOT_DIR / "integration_tests/tests/taira_public_localnet/strict_restart.rs"
    ).read_text(encoding="utf-8")
    integration_runner_source = read_reviewed_source_bundle(
        "integration_tests/tests/sumeragi_v2_runner.rs"
    )
    sumeragi_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/mod.rs"
    )
    lane_work_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    )
    lane_relay_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "nexus" / "lane_relay.rs"
    ).read_text(encoding="utf-8")
    merge_sidecar_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/merge_sidecar.rs"
    )
    state_tests_source = read_source_bundle(
        "crates/iroha_core/src/state/tests.rs"
    )
    # The production runner owns the split cfg(test) module and its reviewed
    # include closure; seal both the production parent and live test owner.
    runner_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/v2_runner.rs"
    )
    ordinary_lifecycle_runner_source = (
        ROOT_DIR
        / "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs"
    ).read_text(encoding="utf-8")
    pending_lifecycle_runner_source = (
        ROOT_DIR
        / "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"
    ).read_text(encoding="utf-8")
    lifecycle_launch_source = (
        ROOT_DIR / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"
    ).read_text(encoding="utf-8")
    runner_tests_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/v2_runner_tests.rs"
    )
    adapter_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/v2.rs"
    )
    core_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/v2_core/tests.rs"
    )
    refinement_source = "\n".join(
        (
            read_reviewed_source_bundle(
                "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
            ),
            read_reviewed_source_bundle(
                "crates/iroha_core/src/sumeragi/v2_core/refinement_cases.rs"
            ),
        )
    )
    effects_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/v2_effects.rs"
    )
    runtime_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    )
    lifecycle_coordinator_source = read_reviewed_source_bundle("crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs")
    worker_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/sumeragi/v2_worker.rs"
    )
    p2p_network_source = read_reviewed_source_bundle(
        "crates/iroha_p2p/src/network.rs"
    )
    p2p_peer_source = read_reviewed_source_bundle(
        "crates/iroha_p2p/src/peer.rs"
    )
    config_actual_source = read_reviewed_source_bundle(
        "crates/iroha_config/src/parameters/actual.rs"
    )
    config_user_source = read_reviewed_source_bundle(
        "crates/iroha_config/src/parameters/user.rs"
    )
    irohad_control_source = (
        ROOT_DIR / "crates" / "irohad" / "src" / "consensus_message_control.rs"
    ).read_text(encoding="utf-8")
    irohad_main_source = read_reviewed_source_bundle(
        "crates/irohad/src/main.rs"
    )
    kura_source = (ROOT_DIR / "crates" / "iroha_core" / "src" / "kura.rs").read_text(
        encoding="utf-8"
    )
    lifecycle_recovery_source = (
        ROOT_DIR
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_lifecycle_recovery.rs"
    ).read_text(encoding="utf-8")
    lane_geometry_source = read_reviewed_source_bundle(
        "crates/iroha_core/src/kura/lane_geometry.rs"
    )
    liveness_doc = (
        ROOT_DIR / "specs" / "sumeragi_v2_liveness.md"
    ).read_text(encoding="utf-8")

    fidelity_root = tmp_path / "kura-application-receipt-source-fidelity"
    kura_relative = Path("crates/iroha_core/src/kura.rs")
    lane_geometry_relative = Path(
        "crates/iroha_core/src/kura/lane_geometry.rs"
    )
    release_relative = Path("scripts/run_sumeragi_v2_release_gates.sh")
    for relative in (
        kura_relative,
        *KURA_PRODUCTION_COMPONENT_FILES,
        lane_geometry_relative,
        release_relative,
    ):
        destination = fidelity_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    assert (
        module._kura_application_receipt_production_source_fidelity_errors(
            fidelity_root
        )
        == []
    )

    kura_fidelity_path = fidelity_root / kura_relative
    canonical_kura = kura_fidelity_path.read_text(encoding="utf-8")

    def mutate_kura_item(item_name: str, old: str, new: str) -> None:
        items = module.rust_items(canonical_kura, item_name)
        assert len(items) == 1
        item = items[0]
        assert item.source.count(old) == 1, (item_name, old)
        start = canonical_kura.index(item.source)
        end = start + len(item.source)
        kura_fidelity_path.write_text(
            canonical_kura[:start]
            + item.source.replace(old, new, 1)
            + canonical_kura[end:],
            encoding="utf-8",
        )

    observation_prune = """        if self.prune_recovery_is_required() {
            return None;
        }
"""
    observation_recovery = """        if self.prune_recovery_is_required()
            || !self.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "lane block application receipt",
            )
        {
            return None;
        }
"""
    kura_mutations = (
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            ".open_bound_progress_sidecar(&data_path, &index_path)",
            ".open_bound_progress_pair(&data_path, &index_path)",
            "must use the structural bound open/read path",
        ),
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            observation_prune,
            observation_recovery,
            "writer observation may not execute sidecar recovery",
        ),
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            observation_prune,
            """        let _ = self
            .read_active_lane_block_application_receipt_durability_attested(
                lane_id,
                lane_block_height,
            );
"""
            + observation_prune,
            "writer observation may not use an attesting reader",
        ),
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            observation_prune,
            """        let _ = self.read_lane_block_application_receipt(
            lane_id,
            lane_block_height,
        );
"""
            + observation_prune,
            "application-receipt writer observation control flow must match the exact reviewed",
        ),
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            """        let artifact = self.read_lane_block_application_receipt_from_bound_locked(
""",
            """        let _ = self.sync_bound_progress_sidecar(
            &bound,
            "lane block application receipt",
        );
        let artifact = self.read_lane_block_application_receipt_from_bound_locked(
""",
            "writer observation may not sync a sidecar",
        ),
        (
            "write_lane_block_application_receipt_artifact",
            "if !self.recover_bound_progress_sidecar_artifacts(",
            "if !self.bound_progress_namespace_unchanged(",
            "sidecar lock and recovery",
        ),
        (
            "write_lane_block_application_receipt_artifact",
            ".sync_bound_progress_sidecar(",
            ".bound_progress_sidecar_unchanged(",
            "exact-existing strict barrier reissue",
        ),
    )
    for item_name, old, new, diagnostic in kura_mutations:
        mutate_kura_item(item_name, old, new)
        errors = module._kura_application_receipt_production_source_fidelity_errors(
            fidelity_root
        )
        assert any(diagnostic in error for error in errors), errors
        kura_fidelity_path.write_text(canonical_kura, encoding="utf-8")

    mutate_kura_item(
        "write_lane_block_application_receipt_artifact",
        """            if existing == *artifact {
""",
        """            if existing == *artifact {
                return Ok(());
            }
            if existing == *artifact {
""",
    )
    errors = module._kura_application_receipt_production_source_fidelity_errors(
        fidelity_root
    )
    for diagnostic in (
        "exact-existing condition must occur exactly 1 time(s)",
        "writer success return must occur exactly 1 time(s)",
    ):
        assert any(diagnostic in error for error in errors), (diagnostic, errors)
    kura_fidelity_path.write_text(canonical_kura, encoding="utf-8")

    release_fidelity_path = fidelity_root / release_relative
    canonical_release = release_fidelity_path.read_text(encoding="utf-8")
    strict_receipt_regression = (
        "kura::tests::progress_witness_durability::"
        "lane_block_application_receipt_strict_retry_reissues_every_barrier"
    )
    assert canonical_release.count(strict_receipt_regression) == 1
    release_fidelity_path.write_text(
        canonical_release.replace(
            strict_receipt_regression,
            "kura::tests::progress_witness_durability::"
            "lane_block_application_receipt_retry_is_not_release_bound",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._kura_application_receipt_production_source_fidelity_errors(
        fidelity_root
    )
    assert any(
        "strict application-receipt retry regression must be pinned exactly once"
        in error
        for error in errors
    ), errors
    release_fidelity_path.write_text(canonical_release, encoding="utf-8")

    runner_path = ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    bash = Path(shutil.which("bash") or "").resolve(strict=True)
    for sealed_value in ("0", "1"):
        direct_environment = {
            "HOME": os.environ.get("HOME", str(ROOT_DIR)),
            "IROHA_RELEASE_SEALED_WORKTREE": sealed_value,
            "PATH": os.defpath,
        }
        direct = subprocess.run(
            [str(bash), str(runner_path), "--release"],
            cwd=ROOT_DIR,
            env=direct_environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=10,
            check=False,
        )
        assert direct.returncode != 0
        expected_diagnostic = (
            "production release requires matching bootstrap path aliases"
            if sealed_value == "0"
            else "production release sealed child requires its authenticated Python"
        )
        assert expected_diagnostic in direct.stderr

    assert "export IROHA_TEST_REQUIRE_NETWORK=1" in seed_source
    assert "export IROHA_TEST_NETWORK_START_ATTEMPTS=1" in seed_source
    assert "-- --list --ignored" in seed_source
    assert "expected exactly one release test named" in seed_source
    assert 'compute_workspace_source_manifest.py --root "$repo_root"' in seed_source
    assert ".seed-matrix.lock" in seed_source
    assert "COMPLETED.tsv" in seed_source

    platform_case = release_source[
        release_source.index('case "$(uname -s)-$(uname -m)" in') :
        release_source.index("  esac", release_source.index('case "$(uname -s)-$(uname -m)" in'))
    ]
    assert platform_case.count("Darwin-arm64)") == 1
    assert platform_case.count("Linux-x86_64)") == 1
    assert "Windows" not in platform_case
    assert "UnsupportedValidatorStoragePlatform" in lane_work_source
    assert "require_validator_storage_platform(" in lane_work_source
    assert "sumeragi_v2_validator_storage_supported()" in lane_work_source
    assert 'cfg!(any(target_os = "linux", target_os = "macos"))' in kura_source
    run_inner_items = module.rust_items(runner_source, "run_inner")
    assert len(run_inner_items) == 1
    run_inner = run_inner_items[0].source
    ordinary_loop_items = module.rust_items(
        ordinary_lifecycle_runner_source, "run_non_pending_lifecycle_loop"
    )
    ordinary_launch_items = module.rust_items(
        ordinary_lifecycle_runner_source, "launch_non_pending_lifecycle_height"
    )
    pending_loop_items = module.rust_items(
        pending_lifecycle_runner_source, "run_pending_kura_lifecycle_height"
    )
    pending_reconcile_items = module.rust_items(
        pending_lifecycle_runner_source, "reconcile_pending_lane_startup"
    )
    assert len(ordinary_loop_items) == len(ordinary_launch_items) == 1
    assert len(pending_loop_items) == 1
    assert len(pending_reconcile_items) == 1
    ordinary_loop = ordinary_loop_items[0].source
    ordinary_launch = ordinary_launch_items[0].source
    pending_loop = pending_loop_items[0].source
    pending_reconcile = pending_reconcile_items[0].source
    runner_platform_gate = run_inner.index("    require_validator_storage_platform(")
    runner_gate_end = run_inner.index("    )?;", runner_platform_gate)
    runner_gate = run_inner[runner_platform_gate:runner_gate_end]
    assert "config.role == NodeRole::Validator" in runner_gate
    for side_effect in (
        "output_guard\n        .begin_fail_stop_operation()",
        "recover_active_height_with_plan(",
    ):
        assert runner_platform_gate < run_inner.index(side_effect)
    ordinary_handoff = run_inner.index(
        "lifecycle_run_inner::run_non_pending_lifecycle_loop("
    )
    pending_handoff = run_inner.index(
        "lifecycle_pending_kura::run_pending_kura_lifecycle_height("
    )
    assert runner_platform_gate < ordinary_handoff < pending_handoff
    ordinary_adapter_open = ordinary_loop.index(
        "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry("
    )
    ordinary_launch_call = ordinary_loop.index(
        "launch_non_pending_lifecycle_height(", ordinary_adapter_open
    )
    assert ordinary_adapter_open < ordinary_launch_call
    assert "owner.launch(inputs)?" in ordinary_launch
    pending_adapter_open = pending_loop.index(
        "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry("
    )
    pending_launch_call = pending_loop.index("owner.launch(", pending_adapter_open)
    assert pending_adapter_open < pending_launch_call
    assert "ProductionV2Services::start_with_apply_service(" in lifecycle_launch_source
    recovered_context = run_inner.index("let recovered = recover_active_height_with_plan(")
    recovered_parts = run_inner.index("    ) = recovered.into_parts();", recovered_context)
    lifecycle_generation_claim = run_inner.index(
        "claim_runner_lifecycle_process_generation("
    )
    assert recovered_parts < lifecycle_generation_claim < ordinary_handoff
    assert lifecycle_generation_claim < pending_handoff
    membership_gate = run_inner.index(
        "local_validator_index(verified_context.context(), &local_peer, config.role)?;"
    )
    assert recovered_parts < membership_gate < lifecycle_generation_claim
    lifecycle_claim = run_inner[
        run_inner.rfind(
            "let _lifecycle_process_generation", 0, lifecycle_generation_claim
        ) : run_inner.index("    )?;", lifecycle_generation_claim) + len("    )?;")
    ]
    assert "config.role" in lifecycle_claim
    assert "kura.as_ref()" in lifecycle_claim
    assert "verified_context.context()" in lifecycle_claim
    assert "&local_peer" in lifecycle_claim
    claim_helper_start = runner_source.index(
        "fn claim_runner_lifecycle_process_generation("
    )
    claim_helper_end = runner_source.index("\nfn round_for_tag(", claim_helper_start)
    claim_helper = runner_source[claim_helper_start:claim_helper_end]
    assert "match role" in claim_helper
    assert "NodeRole::Observer => Ok(None)" in claim_helper
    assert "NodeRole::Validator =>" in claim_helper
    claim_helper_tokens = module.rust_code_tokens(claim_helper)
    assert (
        module._token_sequence_count(
            claim_helper_tokens,
            module.rust_code_tokens("context: &wire::HeightContext"),
        )
        == 1
    )
    assert (
        module._token_sequence_count(
            claim_helper_tokens,
            module.rust_code_tokens(
                """
kura.claim_autonomous_lifecycle_process_generation(
    context.network_id,
    local_peer
)
"""
            ),
        )
        == 1
    )
    assert ".map(Some)" in claim_helper
    for forbidden_claim_identity in (
        "context.roster",
        "local_validator_index",
        "context.chain_id",
        "lifecycle_chain_id",
        "Hash::new",
        "NetworkId::default",
        "Default::default",
        "synthetic_network_id",
    ):
        assert (
            module._token_sequence_count(
                claim_helper_tokens,
                module.rust_code_tokens(forbidden_claim_identity),
            )
            == 0
        ), forbidden_claim_identity
    kura_claim_start = kura_source.index(
        "    pub(crate) fn claim_autonomous_lifecycle_process_generation("
    )
    kura_claim_end = kura_source.index(
        "\n    fn decode_autonomous_lifecycle_bootstrap(", kura_claim_start
    )
    kura_claim_tokens = module.rust_code_tokens(
        kura_source[kura_claim_start:kura_claim_end]
    )
    assert (
        module._production_trace_ordered_token_sequence_error(
            kura_claim_tokens,
            (
                "network_id: iroha_data_model::NetworkId",
                "Result<AutonomousLifecycleProcessGenerationClaim>",
                "if claim.network_id != network_id",
                "AutonomousLifecycleProcessGenerationRecordV1::new(network_id, local_peer_id.clone(), generation,)",
                "let claim = AutonomousLifecycleProcessGenerationClaim { store_root: self.store_root.clone(), network_id, local_peer_id: local_peer_id.clone(), generation, record_hash: next.record_hash, }",
                "self.validate_autonomous_lifecycle_process_generation_claim(&claim)",
                "Ok(claim)",
            ),
        )
        is None
    )
    lifecycle_reconcile_start = lifecycle_recovery_source.index(
        "pub(crate) fn reconcile_autonomous_lifecycle_startup("
    )
    lifecycle_reconcile_end = lifecycle_recovery_source.index(
        "\n#[cfg(test)]", lifecycle_reconcile_start
    )
    lifecycle_reconcile_tokens = module.rust_code_tokens(
        lifecycle_recovery_source[
            lifecycle_reconcile_start:lifecycle_reconcile_end
        ]
    )
    assert (
        module._production_trace_ordered_token_sequence_error(
            lifecycle_reconcile_tokens,
            (
                "context: &wire::HeightContext",
                "let network_id = context.network_id",
                "let Some(process_generation) = process_generation else",
                "process_generation.local_peer_id() != local_peer",
                "process_generation.network_id() != network_id",
            ),
        )
        is None
    )
    ordinary_startup_reconcile = ordinary_loop.index(
        "reconcile_autonomous_lifecycle_startup("
    )
    ordinary_startup_claim_handoff = ordinary_loop.index(
        "lifecycle_process_generation.as_ref(),", ordinary_startup_reconcile
    )
    ordinary_lane_service = ordinary_loop.index(
        "V2LaneWorkAdapter::new_with_output_guard_and_transport("
    )
    ordinary_adapter_claim_handoff = ordinary_loop.index(
        "lifecycle_process_generation.clone(),", ordinary_lane_service
    )
    assert ordinary_startup_reconcile < ordinary_startup_claim_handoff
    assert ordinary_startup_claim_handoff < ordinary_lane_service
    assert ordinary_lane_service < ordinary_adapter_claim_handoff
    pending_startup_reconcile = pending_reconcile.index(
        "reconcile_autonomous_lifecycle_startup("
    )
    pending_startup_claim_handoff = pending_reconcile.index(
        "lifecycle_process_generation,", pending_startup_reconcile
    )
    pending_reconcile_call = pending_loop.index("reconcile_pending_lane_startup(")
    pending_reconcile_claim_handoff = pending_loop.index(
        "lifecycle_process_generation.as_ref(),", pending_reconcile_call
    )
    pending_lane_service = pending_loop.index(
        "V2LaneWorkAdapter::new_with_output_guard_and_transport("
    )
    pending_adapter_claim_handoff = pending_loop.index(
        "lifecycle_process_generation.clone(),", pending_lane_service
    )
    assert pending_startup_reconcile < pending_startup_claim_handoff
    assert pending_reconcile_call < pending_reconcile_claim_handoff
    assert pending_reconcile_claim_handoff < pending_lane_service
    assert pending_lane_service < pending_adapter_claim_handoff
    lane_constructor_start = lane_work_source.index(
        "    fn new_with_output_guard_and_transport_inner("
    )
    lane_constructor = lane_work_source[lane_constructor_start:]
    lane_platform_gate = lane_constructor.index(
        "        require_validator_storage_platform("
    )
    for side_effect in (
        "begin_fail_stop_operation()",
        "context\n            .validate()",
        "MergeSigningGuard::open_with_committed_frontier",
        "NativeAmxSigningGuard::open",
    ):
        assert lane_platform_gate < lane_constructor.index(side_effect)
    lane_constructor_end = lane_constructor.index(
        "    pub(crate) fn activate_after_lane_drain_queue_install("
    )
    carrier_silent_constructor = lane_constructor[:lane_constructor_end]
    assert "adapter.hydrate_canonical_lane_artifacts()?;" not in carrier_silent_constructor
    assert "adapter.drive_lane_sessions();" not in carrier_silent_constructor
    activation = lane_constructor[lane_constructor_end:]
    hydration = activation.index("self.hydrate_canonical_lane_artifacts()?;")
    queue_revalidation = activation.index(
        "self.revalidate_hydrated_autonomous_queue_owners(installed_queue.as_ref())?;"
    )
    drive = activation.index("self.drive_lane_sessions();")
    assert hydration < queue_revalidation < drive
    queue_install = ordinary_loop.index(
        "lane_work.install_lane_drain_queue(Arc::clone(&queue))?;"
    )
    lane_activation = ordinary_loop.index(
        "lane_work.activate_after_lane_drain_queue_install(&queue)?;"
    )
    assert queue_install < lane_activation
    for lifecycle_source in (ordinary_loop, pending_loop):
        lane_constructor = lifecycle_source[
            lifecycle_source.index(
                "V2LaneWorkAdapter::new_with_output_guard_and_transport("
            ) :
        ]
        lane_constructor = lane_constructor[: lane_constructor.index(".map_err(")]
        assert "config.role == NodeRole::Validator," in lane_constructor
        assert "local_validator.is_some()," not in lane_constructor
    assert "(NodeRole::Observer, _) => Ok(None)" in runner_source
    assert (
        module._kura_retirement_progress_production_source_fidelity_errors(
            fidelity_root
        )
        == []
    )
    assert (
        module._kura_native_amx_standalone_evidence_production_source_fidelity_errors(
            fidelity_root
        )
        == []
    )
    lane_geometry_fidelity_path = fidelity_root / lane_geometry_relative
    canonical_lane_geometry = lane_geometry_fidelity_path.read_text(
        encoding="utf-8"
    )

    def mutate_lane_geometry_item(
        item_name: str, old: str, new: str
    ) -> None:
        items = module.rust_items(canonical_lane_geometry, item_name)
        assert len(items) == 1
        item = items[0]
        assert item.source.count(old) == 1, (item_name, old)
        start = canonical_lane_geometry.index(item.source)
        end = start + len(item.source)
        lane_geometry_fidelity_path.write_text(
            canonical_lane_geometry[:start]
            + item.source.replace(old, new, 1)
            + canonical_lane_geometry[end:],
            encoding="utf-8",
        )

    retirement_item = module.rust_items(
        canonical_lane_geometry,
        "ensure_first_release_lane_retirement_admissible_with_certified_locked",
    )
    assert len(retirement_item) == 1
    retirement_item_source = retirement_item[0].source
    fixed_pairs_start = retirement_item_source.index(
        "let fixed_progress_pairs:"
    )
    fixed_pairs_end = retirement_item_source.index(
        "];", fixed_pairs_start
    ) + len("];")
    canonical_fixed_pairs = retirement_item_source[
        fixed_pairs_start:fixed_pairs_end
    ]

    def mutate_fixed_progress_pairs(old: str, new: str) -> None:
        assert canonical_fixed_pairs.count(old) == 1, old
        mutate_lane_geometry_item(
            "ensure_first_release_lane_retirement_admissible_with_certified_locked",
            canonical_fixed_pairs,
            canonical_fixed_pairs.replace(old, new, 1),
        )

    mutate_fixed_progress_pairs("; 6]", "; 5]")
    errors = module._kura_retirement_progress_production_source_fidelity_errors(
        fidelity_root
    )
    assert any(
        "fixed retirement progress-pair declaration and bound" in error
        for error in errors
    ), errors
    lane_geometry_fidelity_path.write_text(
        canonical_lane_geometry, encoding="utf-8"
    )

    for data_name, _index_name, _path_builder, _kind in (
        module._KURA_RETIREMENT_FIXED_PROGRESS_PAIR_CONTRACTS
    ):
        mutate_fixed_progress_pairs(
            f"&{data_name}", f"&mutated_{data_name}"
        )
        errors = (
            module._kura_retirement_progress_production_source_fidelity_errors(
                fidelity_root
            )
        )
        assert any(
            "must preserve exact six-member artifact membership and order"
            in error
            for error in errors
        ), (data_name, errors)
        lane_geometry_fidelity_path.write_text(
            canonical_lane_geometry, encoding="utf-8"
        )

    lane_geometry_mutations = (
        (
            "ensure_first_release_lane_retirement_admissible_with_certified_locked",
            "    &fixed_progress_pairs,\n",
            "    &fixed_progress_pairs[..5],\n",
            "all fixed retirement progress pairs must recover before the "
            "immutable snapshot",
        ),
        (
            "recover_geometry_progress_pairs_before_snapshot",
            "for &(data_path, index_path, kind) in pairs {",
            "for &(data_path, index_path, kind) in pairs.iter().take(5) {",
            "retirement recovery must visit every pair inside the "
            "authenticated directory",
        ),
        (
            "recover_geometry_progress_pairs_before_snapshot",
            "BoundProgressRecoveryFailure::RetryableIo => ErrorKind::WouldBlock,",
            "BoundProgressRecoveryFailure::RetryableIo => ErrorKind::InvalidData,",
            "retirement recovery failure classification",
        ),
        (
            "recover_geometry_progress_pairs_before_snapshot",
            "recovery_directory = refreshed_directory;",
            "let _discarded_directory = refreshed_directory;",
            "retirement recovery must rebind the authenticated directory "
            "after every pair",
        ),
    )
    for item_name, old, new, diagnostic in lane_geometry_mutations:
        mutate_lane_geometry_item(item_name, old, new)
        errors = (
            module._kura_retirement_progress_production_source_fidelity_errors(
                fidelity_root
            )
        )
        assert any(diagnostic in error for error in errors), (
            diagnostic,
            errors,
        )
        lane_geometry_fidelity_path.write_text(
            canonical_lane_geometry, encoding="utf-8"
        )

    standalone_native_mutations = (
        (
            "read_geometry_native_amx_per_height_evidence",
            "Self::parse_native_amx_evidence_path(&path)?",
            "None",
            "standalone Native AMX scanner must classify only canonical "
            "per-height names",
        ),
        (
            "read_geometry_native_amx_per_height_evidence",
            "evidence_bytes = evidence_bytes.checked_add(encoded_len)",
            "evidence_bytes = encoded_len",
            "standalone Native AMX shared aggregate byte total must be "
            "overflow checked",
        ),
        (
            "read_geometry_native_amx_per_height_evidence",
            "if evidence_bytes > self.native_amx_participant_evidence_file_bytes() {",
            "if false {",
            "standalone Native AMX shared aggregate byte bound must use the "
            "configured source of truth",
        ),
        (
            "ensure_first_release_lane_retirement_admissible_with_certified_locked",
            """            if !native_amx_retained_windows_are_complete(
                &native_manifest_heights,
                &native_receipt_heights,
            ) {
""",
            """            if native_manifest_heights != native_receipt_heights {
""",
            "live Native AMX evidence must form a complete retained suffix",
        ),
        (
            "ensure_first_release_lane_retirement_admissible_with_certified_locked",
            "match native_receipt_heights.last().copied() {",
            "match native_receipt_heights.first().copied() {",
            "live Native AMX latest lookup must select the highest retained "
            "receipt",
        ),
        (
            "ensure_first_release_lane_retirement_admissible_with_certified_locked",
            "manifest.leaf.lane_id != entry.lane_id",
            "manifest.leaf.lane_id == entry.lane_id",
            "live Native AMX manifests must join the active route, incarnation, "
            "and application height",
        ),
        (
            "ensure_archived_lane_work_released",
            "manifest.leaf.lane_incarnation != binding.incarnation",
            "manifest.leaf.lane_incarnation == binding.incarnation",
            "archived Native AMX manifests must join the archived incarnation "
            "and canonical finality",
        ),
        (
            "ensure_first_release_lane_retirement_admissible_with_certified_locked",
            """                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane retirement scan encountered an unknown artifact filename",
                    ),
                    path,
                ));
""",
            """                continue;
""",
            "live Native AMX retirement must reject every unexpected or legacy "
            "artifact after the complete allowlist",
        ),
        (
            "ensure_first_release_lane_retirement_admissible_with_certified_locked",
            """                if Self::parse_native_amx_evidence_path(&path)?.is_some() {
                    continue;
                }
""",
            """                if Self::parse_native_amx_evidence_path(&path)?.is_some() {
                    continue;
                }
                if name == "native_amx_application_manifests.norito" {
                    continue;
                }
""",
            "must reject obsolete dense Native AMX evidence acceptance",
        ),
    )
    for item_name, old, new, diagnostic in standalone_native_mutations:
        mutate_lane_geometry_item(item_name, old, new)
        errors = (
            module._kura_native_amx_standalone_evidence_production_source_fidelity_errors(
                fidelity_root
            )
        )
        assert any(diagnostic in error for error in errors), (
            diagnostic,
            errors,
        )
        lane_geometry_fidelity_path.write_text(
            canonical_lane_geometry, encoding="utf-8"
        )

    mutate_kura_item(
        "native_amx_participant_evidence_file_bytes",
        "u64::try_from(self.pending_control_sidecar_limits.aggregate_bytes)",
        "u64::try_from(STRICT_INIT_MAX_BLOCK_BYTES)",
    )
    native_amx_source_errors = (
        module._kura_native_amx_standalone_evidence_production_source_fidelity_errors(
            fidelity_root
        )
    )
    assert any(
        "standalone Native AMX configured aggregate byte source must be the "
        "pending-control sidecar geometry"
        in error
        for error in native_amx_source_errors
    ), native_amx_source_errors
    kura_fidelity_path.write_text(canonical_kura, encoding="utf-8")

    mutate_kura_item(
        "native_amx_participant_evidence_file_bytes",
        '.expect("configured pending-control sidecar bytes fit u64")',
        '.expect("configured pending-control sidecar bytes fit u64")\n'
        "            .max(STRICT_INIT_MAX_BLOCK_BYTES)",
    )
    native_amx_source_errors = (
        module._kura_native_amx_standalone_evidence_production_source_fidelity_errors(
            fidelity_root
        )
    )
    assert any(
        "standalone Native AMX configured aggregate byte source must be the "
        "pending-control sidecar geometry"
        in error
        for error in native_amx_source_errors
    ), native_amx_source_errors
    kura_fidelity_path.write_text(canonical_kura, encoding="utf-8")

    new_production_inventory_additions = (
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_classifies_recovery_sync_failure_as_retryable",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_discards_unpublished_temp_for_every_fixed_pair",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_rejects_obsolete_autonomous_rewrite_without_promotion",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_recovers_complete_certified_rewrite_before_snapshot",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_recovery_rejects_temp_symlink_without_external_writes",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_rejects_directory_substitution_at_pair_refresh",
            lane_geometry_source,
        ),
        (
            "sumeragi::v2_lane_work::tests::",
            "validator_storage_platform_gate_rejects_voters_and_allows_observers",
            lane_work_source,
        ),
        (
            "sumeragi::v2_runner::tests::",
            "unsupported_storage_platform_rejects_runner_voter_and_admits_observer",
            runner_tests_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "distinct_prepare_qc_view_zero_wait_covers_deadline_without_masking_view_one",
            integration_runner_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
            integration_runner_source,
        ),
        (
            "sumeragi::v2::tests::",
            "successor_core_context_preserves_the_parent_certificate_binding",
            adapter_source,
        ),
        (
            "state::tests::",
            "block_leaves_governance_unlock_audit_clean_when_no_locks_are_expired",
            state_tests_source,
        ),
    )
    macro_step_production_inventory_additions = (
        (
            "sumeragi::v2::tests::",
            "persistence_macro_step_budgets_have_exact_four_effect_maximum",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "drive_effects_rejects_oversized_non_persisting_batch",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "drive_effects_rejects_record_specific_overbudget_before_wal_append",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "drive_effects_rejects_multiple_persist_owners_before_wal_append",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "post_wal_oversized_continuation_fails_closed_and_replays_exact_record",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "deferred_dispatch_decreases_rank_by_exactly_one_macro_step_per_turn",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "deferred_service_contract_violation_is_terminal",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "busy_deferred_input_blocks_terminal_readiness_until_serviced",
            adapter_source,
        ),
        (
            "sumeragi::v2_core::tests::",
            "commit_qc_preempts_hung_timeout_signature_but_not_pending_wal",
            core_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "certified_request_pressure_retains_higher_authority_upgrade_under_one_owner",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "reconstructible_new_certified_fetch_acquires_ownership_from_retained_admission",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "production_capacity_saturation_admits_response_and_reconstructible_fetch",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "retained_producer_suffix_allows_exact_payload_chunk_to_release_fetch_capacity",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "retained_producer_suffix_allows_exact_certified_response_to_release_fetch_capacity",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "reconciled_decision_rejects_same_round_subject_commitment_drift",
            effects_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "absolute_timeout_preempts_serviceable_adapter_debt_then_debt_drains",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "serviceable_adapter_debt_runs_without_runtime_ingress",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "authenticated_remote_proposal_retains_exact_fetch_store_validate_replay_origin",
            runtime_source,
        ),
    )
    assert len(macro_step_production_inventory_additions) == 18
    latest_production_inventory_additions = (
        (
            "nexus::lane_relay::tests::",
            "actor_backpressure_retains_exact_relay_and_fifo_ticket",
            lane_relay_source,
        ),
        (
            "nexus::lane_relay::tests::",
            "blocked_relay_does_not_starve_a_responsive_relay",
            lane_relay_source,
        ),
        (
            "nexus::lane_relay::tests::",
            "terminal_actor_failures_return_exact_relay_ownership",
            lane_relay_source,
        ),
        (
            "nexus::lane_relay::tests::",
            "saturated_relay_owner_returns_sixty_fifth_without_actor_ticket",
            lane_relay_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "certified_tc_crosses_full_fence_blocked_prepare_prefix",
            runtime_source,
        ),
        (
            "sumeragi::v2_lane_work::tests::",
            "applied_lane_certificate_retires_alternative_qc_replays_without_weakening_conflicts",
            lane_work_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_completion_bound_overflow_fails_closed",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_completion_corridor_survives_ordinary_progress_and_timeout_saturation",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_completion_owner_is_source_isolated_and_queue_scoped",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_exact_max_chunk_bound_matches_canonical_wire",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_exact_response_bound_accepts_required_and_rejects_required_minus_one",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_recommended_context_fits_default_disjoint_byte_partitions",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "v2_ingress_rejects_capacity_without_per_validator_progress_reservations",
            sumeragi_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "exact_prepare_qc_requires_both_count_and_power_quorum",
            integration_runner_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "locked_commit_progress_witness_rejects_inexact_or_empty_ownership",
            integration_runner_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "locked_commit_progress_witness_accepts_each_exact_owner",
            integration_runner_source,
        ),
        (
            "sumeragi::v2_worker::tests::",
            "applied_height_handoff_accepts_historical_kura_global_responses_atomically",
            worker_source,
        ),
        (
            "sumeragi::v2_worker::tests::",
            "applied_height_handoff_accepts_only_exact_historical_kura_lane_certificate",
            worker_source,
        ),
        (
            "sumeragi::v2_worker::tests::",
            "ownership_units_reject_reservation_spill_and_release_exact_target",
            worker_source,
        ),
        (
            "network::tests::",
            "reliable_progress_class_matches_actor_reservations_exactly",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "reply_route_survives_peer_message_clone_mapping_and_split",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "reply_source_key_groups_relay_origins_and_orders_actor_instances",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "reply_route_source_updates_are_ordinal_monotonic_and_target_scoped",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "cancelled_newer_hub_cannot_erase_older_independent_route_attempt",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "dependent_fixture_models_bounded_actor_global_multi_hub_ownership",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "route_cancelled_between_preflight_and_admission_retires_without_queue_ownership",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets",
            p2p_network_source,
        ),
        (
            "network::handle_update_tests::",
            "targetized_broadcast_coalesces_only_the_same_digest_and_membership",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "distinct_broadcast_residual_is_target_isolated_and_its_rank_decreases",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "exact_broadcast_retry_coalesces_but_distinct_and_direct_requests_do_not",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "removed_membership_cancels_only_old_broadcast_debt_across_readd",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "cancelled_target_child_with_pending_flush_ack_releases_exactly_once",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "requested_topology_is_not_authority_and_closed_fanout_returns_all_targets",
            p2p_network_source,
        ),
    )
    assert len(latest_production_inventory_additions) == 34
    route_completion_inventory_additions = (
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_coalesces_semantic_request_and_attaches_independent_routes",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "transport_reply_route_construction_is_fallible_and_target_bound",
            sumeragi_source,
        ),
        *(
            ("merge_sidecar::tests::", test_name, merge_sidecar_source)
            for test_name in (
                "exact_active_delivery_retry_preserves_decreasing_chunk_rank",
                "alternate_source_progress_and_reconnect_preserve_independent_cursors",
                "reused_actor_ordinals_under_different_tenures_are_rejected_atomically",
                "reply_unwritable_route_parks_inflight_materialization_without_bytes",
                "later_delivery_preserves_the_current_source_cursor",
                "later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit",
                "late_old_exact_item_receipt_completes_reconnected_attempt_once",
                "later_delivery_during_materialization_keeps_exact_authorized_route",
                "writable_reconnect_during_materialization_keeps_exact_authorized_tenure",
                "equal_sequence_with_different_semantic_identity_is_rejected_before_materialization",
                "transient_materialization_release_keeps_exact_retry",
                "response_materialization_requires_and_consumes_its_exact_admission_gate",
                "inactive_reply_route_is_rejected_before_server_gate_admission",
                "completed_source_later_and_reconnect_stay_terminal_while_sibling_progresses",
                "exact_delivery_retry_stays_terminal_beyond_retired_ttl_horizon",
                "request_stream_close_floor_advances_only_over_a_contiguous_terminal_prefix",
                "authenticated_close_floor_retires_covered_output_and_rejects_replay_or_regression",
                "authenticated_source_quota_rejects_origin_churn_and_preserves_other_source",
                "rejected_request_does_not_consume_server_stream_state",
                "completed_source_does_not_block_a_new_alternate_source",
                "configured_route_source_capacity_bounds_semantic_attempts",
                "configured_source_geometry_reserves_more_than_eight_independent_attempts",
                "durable_responder_restart_preserves_same_hub_gate_budget",
                "fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses",
                "quiescent_multi_source_pressure_never_rolls_or_bypasses_source_caps",
                "legacy_lifecycle_v1_snapshot_is_rejected_without_migration",
                "third_session_from_one_hub_is_rejected_while_another_hub_progresses",
                "source_byte_overflow_is_rejected_while_another_hub_progresses",
                "completed_short_session_replacement_cannot_starve_an_older_long_session",
                "unsent_request_restores_holder_and_backoff_state",
                "idle_request_retry_starts_strictly_after_the_fairness_cursor",
                "route_retirement_between_admission_and_enqueue_releases_all_response_reservations",
                "saturated_materializer_does_not_erase_same_request_alternate_session",
                "saturated_materializer_does_not_erase_same_request_alternate_bytes",
                "sidecar_flush_refinement_advances_only_exact_source_chunk",
            )
        ),
        *(
            ("sumeragi::v2::tests::", test_name, adapter_source)
            for test_name in (
                "deferred_actor_source_never_aliases_across_adapter_instances",
                "deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step",
                "deferred_authenticated_retry_retains_exact_original_and_effective_tags",
                "deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap",
                "deferred_service_debt_overflow_is_typed_and_fail_closed",
                "deferred_service_evidence_rejects_every_owner_and_rank_mutation",
                "deferred_zero_ordinal_is_exact_single_use_and_never_reminted",
            )
        ),
        *(
            ("sumeragi::v2_effects::tests::", test_name, effects_source)
            for test_name in (
                "live_runtime_step_rejects_missing_scheduler_ownership_before_callbacks",
                "recovery_runtime_step_rejects_invalid_scheduler_ownership_before_callbacks",
            )
        ),
        *(
            ("sumeragi::v2_lane_work::tests::", test_name, lane_work_source)
            for test_name in (
                "native_amx_request_rejects_inactive_reply_route_before_signing",
                "duplicate_reply_effect_preserves_exact_source_delivery",
                "reply_effect_rejects_missing_or_retargeted_route_set",
                "duplicate_reply_effect_updates_only_later_delivery_from_same_source",
                "duplicate_reply_effect_retains_alternate_sources_across_source_update",
                "temporarily_unserviceable_effect_requeues_behind_later_reserved_work",
                "retired_sidecar_route_between_drain_and_lane_queue_preserves_live_sibling",
                "durable_lane_certificate_coalescing_preserves_alternate_ingress_owners",
            )
        ),
        *(
            ("sumeragi::v2_runtime::tests::", test_name, runtime_source)
            for test_name in (
                "adapter_command_identity_is_derived_from_exact_immutable_payload",
                "admission_ordinal_exhaustion_fails_runtime_closed",
                "runtime_rejects_replayed_foreign_and_mutated_deferred_tokens",
                "scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches",
                "scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields",
                "scheduler_owner_must_be_taken_before_a_later_step_can_enter",
                "selected_owner_without_a_runtime_minted_ordinal_fails_closed",
            )
        ),
        *(
            ("sumeragi::v2_runner::tests::", test_name, runner_tests_source)
            for test_name in (
                "reserved_lane_output_bypasses_unserviceable_head_without_losing_owner",
                "runner_dispatch_preserves_durable_lane_certificate_reply_routes",
                "runner_dispatch_preserves_certified_sidecar_chunk_reply_routes",
                "bounded_sidecar_admission_turn_applies_only_its_budget",
                "runner_dispatch_prunes_retired_sidecar_source_without_losing_live_sibling",
                "runner_dispatch_advances_certified_sidecar_only_after_writer_flush",
                "runner_dispatch_retired_admission_race_emits_no_sidecar_receipt",
                "runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once",
                "runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route",
                "runner_dispatch_rejects_durable_response_without_reply_routes",
            )
        ),
        *(
            ("sumeragi::v2_worker::tests::", test_name, worker_source)
            for test_name in (
                "actor_backpressure_retains_exact_final_lane_commit_qc_post",
                "actor_backpressure_retains_complete_merge_share_fanout",
                "same_tenure_updates_and_reconnect_preserve_current_item",
                "closed_sidecar_source_reconnect_retries_current_item_while_sibling_backpressures",
                "completed_sidecar_reconnect_preserves_terminal_cursor_without_capacity_charge",
                "later_delivery_cannot_requeue_pending_or_unapplied_sidecar_flush_but_other_attempts_progress",
                    "mixed_source_retry_retains_pending_flush_target_without_resetting_live_siblings",
                "inactive_reply_target_tombstone_rejects_cross_source_equal_ordinal_collision",
                "owned_reply_history_merge_retries_candidate_retirement_after_prune",
                "newly_observed_alternate_hub_starts_at_zero_without_resetting_parked_source",
                "a_b_a_hub_reconnect_preserves_each_source_cursor",
                "owned_reply_transfer_retirement_after_validation_is_atomic",
                "bulk_backpressure_does_not_block_reserved_lane_or_safety_output",
                "non_roster_targets_cannot_consume_frozen_validator_reservations",
                "partial_fanout_progress_releases_only_the_completed_target_unit",
                "ownership_units_reject_reservation_spill_and_release_exact_target",
                "backpressured_source_does_not_block_other_sources_or_consume_their_reserve",
                "response_outputs_without_exact_routes_fail_stop",
                "exact_output_coalescing_preserves_distinct_fair_ingress_admissions",
                "orphan_chunk_coalescing_preserves_alternate_fair_ingress_routes",
                "sidecar_flush_ack_identity_mismatch_fails_closed",
                "sidecar_receipts_use_a_separate_bounded_control_queue",
                "exact_output_retry_rejects_a_different_message_identity",
                "full_exact_output_corridor_does_not_disguise_non_progress_routes_as_backpressure",
                "applied_height_handoff_retires_all_sidecar_flush_states_without_blocking_successor",
                "applied_height_handoff_counts_and_clears_parked_reply_cursor_atomically",
            )
        ),
        *(
            ("network::tests::", test_name, p2p_network_source)
            for test_name in (
                "reliable_progress_class_matches_actor_reservations_exactly",
                "reply_route_survives_peer_message_clone_mapping_and_split",
                "reply_source_key_groups_relay_origins_and_orders_actor_instances",
                "reply_route_source_updates_are_ordinal_monotonic_and_target_scoped",
                "dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals",
                "cancelled_newer_hub_cannot_erase_older_independent_route_attempt",
                "dependent_fixture_models_bounded_actor_global_multi_hub_ownership",
                "reply_route_pruning_retains_equal_ordinal_tenure_tombstone",
                "reply_route_set_isolates_sources_preserves_cursors_and_prunes_retired_capacity",
                "route_cancelled_between_preflight_and_admission_retires_without_queue_ownership",
                "reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets",
                "reply_actor_admission_does_not_complete_writer_flush_ack",
                "reply_flush_identity_binds_ticket_tenure_source_payload_and_delivery_occurrence",
                "reply_flush_test_fixture_binds_exact_canonical_post_and_opaque_actor",
                "reply_flush_ack_cancellation_between_precheck_and_budget_lock_returns_none",
                "retired_reply_tenure_closes_flush_ack_without_false_completion",
                "reply_flush_test_fixture_distinguishes_success_timeout_and_close",
                "reply_flush_ack_completes_only_after_peer_writer_flush",
            )
        ),
        (
            "consensus_message_control::tests::",
            "controlled_v2_admission_preserves_distinct_relay_identity",
            irohad_control_source,
        ),
        *(
            ("consensus_message_control::tests::", test_name, irohad_control_source)
            for test_name in (
                "failed_release_clears_in_flight_ownership_and_latches_fatal",
                "fatal_controller_rejects_an_unchanged_command_poll",
                "retired_release_finishes_drain_without_claiming_delivery",
            )
        ),
        (
            "network_relay_tests::",
            "obsolete_sumeragi_relay_message_fails_closed",
            irohad_main_source,
        ),
        (
            "network_relay_tests::",
            "test_control_hold_release_preserves_live_route_and_retires_canceled_reentry",
            irohad_main_source,
        ),
        (
            "network_relay_tests::",
            "certified_merge_sidecar_close_is_limited_but_responder_controls_are_critical",
            irohad_main_source,
        ),
    )
    assert len(route_completion_inventory_additions) == 123
    source_geometry_inventory_additions = (
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins",
            sumeragi_source,
        ),
        (
            "merge_sidecar::tests::",
            "authenticated_source_limits_are_fixed_at_four_gates_two_sessions_and_sixteen_mibibytes",
            merge_sidecar_source,
        ),
        (
            "sumeragi::v2::tests::",
            "authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer",
            adapter_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "owned_payload_chunk_rejects_source_swap_before_service_and_keeps_unknown_work_nonfatal",
            effects_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "runtime_merges_alternate_sources_for_one_semantic_request",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "busy_deferred_request_merges_alternate_source_and_services_exact_carrier",
            runtime_source,
        ),
        (
            "sumeragi::v2_worker::tests::",
            "owned_orphan_chunk_replay_preserves_alternate_source_routes_and_cursors",
            worker_source,
        ),
        (
            "network::tests::",
            "peer_message_mints_actor_global_delivery_ordinals_across_connection_tenures",
            p2p_network_source,
        ),
        (
            "parameters::actual::tests::",
            "sumeragi_v2_exact_output_geometry_checks_every_arithmetic_boundary",
            config_actual_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_v2_exact_output_geometry_accepts_network_source_boundary",
            config_user_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary",
            config_user_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources",
            config_user_source,
        ),
    )
    assert len(source_geometry_inventory_additions) == 13
    route_lifecycle_inventory_additions = tuple(
        ("network::tests::", test_name, p2p_network_source)
        for test_name in (
            "reply_route_binding_rejects_evicted_tombstone_collision",
            "network_actor_drop_retires_routes_and_only_its_waiters",
            "peer_message_rehydration_rejects_second_reply_route_without_retargeting",
        )
    )
    assert len(route_lifecycle_inventory_additions) == 3
    latest_h_geometry_and_daemon_inventory_additions = (
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "authenticated_non_validator_source_cap_retries_third_source_until_one_lane_drains",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "alternate_reply_route_attaches_before_authenticated_source_lane_cap",
            sumeragi_source,
        ),
        (
            "consensus_message_control::tests::",
            "stale_duplicate_reordered_and_unknown_releases_are_atomic",
            irohad_control_source,
        ),
        (
            "consensus_message_control::tests::",
            "hold_capacity_is_bounded_by_count_bytes_and_checked_arithmetic",
            irohad_control_source,
        ),
        (
            "consensus_message_control::tests::",
            "drain_fence_holds_racing_chunks_fifo_until_atomic_cutover",
            irohad_control_source,
        ),
        (
            "tests::relay_fairness::",
            "hold_release_preserves_exact_layered_ownership_until_recorded_terminal",
            irohad_main_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_authenticated_non_validator_sources_must_fit_network_geometry",
            config_user_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_authenticated_non_validator_sources_use_effective_lane_profile_geometry",
            config_user_source,
        ),
        (
            "parameters::actual::tests::",
            "sumeragi_v2_config_format_changes_the_handshake_fingerprint",
            config_actual_source,
        ),
        (
            "sumeragi::v2_core::refinement::tests::",
            "historical_body_pipeline_kernel_rejects_request_subject_and_owner_substitution",
            refinement_source,
        ),
        (
            "sumeragi::v2_core::refinement::tests::",
            "historical_certificate_kernel_rejects_foreign_admission_and_unretired_request",
            refinement_source,
        ),
        (
            "peer::run::tests::",
            "consensus_lane_and_v2_topics_share_authenticated_high_source_credit",
            p2p_peer_source,
        ),
    )
    assert len(latest_h_geometry_and_daemon_inventory_additions) == 12
    apply_authority_inventory_additions = (
        (
            "sumeragi::v2_effects::tests::",
            "apply_rejects_matching_commit_qc_from_foreign_context_without_scheduling_work",
            effects_source,
        ),
    )
    deterministic_ownership_inventory_additions = (
        (
            "sumeragi::v2_effects::tests::",
            "exact_candidate_retry_coalesces_under_the_incumbent_owner",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "fetch_owner_replacement_is_rejected_before_upgrade_refinement_or_request_work",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "adapter_effect_retry_policy_is_closed_over_all_eleven_effect_classes",
            effects_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "adapter_effect_binding_is_exact_route_neutral_and_three_bounded",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "certified_body_pipeline_retains_statement_and_owner_across_stage_kinds",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "body_pipeline_acquires_commit_authority_monotonically_under_one_owner",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "pending_validate_projects_exact_prepare_commit_and_report_successors",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "pending_validate_projects_only_the_exact_commit_authorized_apply_successor",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "drained_internal_ignore_uses_exact_durable_tombstone_before_readmission",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "queued_body_completion_coalesces_only_its_incumbent_owner",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal",
            runtime_source,
        ),
        ("sumeragi::v2_lifecycle_coordinator::tests::", "restart_seeds_high_water_and_rollover_preserves_it", lifecycle_coordinator_source),
        ("sumeragi::v2_lifecycle_coordinator::tests::", "producer_handoff_blocks_later_work_without_making_serve_a_global_barrier", lifecycle_coordinator_source),
    )
    assert len(deterministic_ownership_inventory_additions) == 13
    production_inventory_additions = tuple(
        item
        for item in (
            new_production_inventory_additions
            + macro_step_production_inventory_additions
            + latest_production_inventory_additions
            + route_completion_inventory_additions
            + source_geometry_inventory_additions
            + route_lifecycle_inventory_additions
            + latest_h_geometry_and_daemon_inventory_additions
            + apply_authority_inventory_additions
            + deterministic_ownership_inventory_additions
        )
        if f"{item[0]}{item[1]}"
        not in module._PRODUCTION_LIVENESS_RETIRED_REGRESSIONS
        )
    for module_name, test_name, source in production_inventory_additions:
        declaration_count = sum(
            source.count(marker)
            for marker in (f"fn {test_name}(", f"state_test! {{ sync {test_name}")
        )
        assert declaration_count == 1, (
            module_name,
            test_name,
            declaration_count,
        )
    normalized_liveness_doc = re.sub(r"\s+", " ", liveness_doc.lower())
    assert (
        "other platforms are restricted to non-voting observer or development use"
        in normalized_liveness_doc
    )
    assert (
        "complete observer application and lane-retirement behavior there is not release-certified"
        in normalized_liveness_doc
    )
    assert (
        "no unsupported-platform validator release receipt may be emitted"
        in normalized_liveness_doc
    )

    bootstrap_validation = release_source.index("for bootstrap_suffix in")
    required_network = release_source.index("export IROHA_TEST_REQUIRE_NETWORK=1")
    production_units = release_source.index("required_production_liveness_tests")
    taira_rust_contracts = release_source.index(
        "required_taira_release_contract_tests=("
    )
    source_contract_preflight = release_source.index(
        "source_manifest_contract_tests=("
    )
    seed_launcher_preflight = release_source.index("seed_launcher_contract_tests=(")
    chaos_launcher_preflight = release_source.index(
        "chaos_launcher_contract_files=("
    )
    receipt_contract_preflight = release_source.index(
        "release_receipt_contract_files=("
    )
    proof_fidelity_preflight = release_source.index(
        "proof_fidelity_contract_files=("
    )
    formal_launcher_preflight = release_source.index(
        "formal_launcher_contract_files=("
    )
    taira_soak_preflight = release_source.index("taira_soak_contract_files=(")
    seed_matrix = release_source.index("run_sumeragi_v2_seed_matrix.sh")
    pr_branch = release_source.index('if [[ "$profile" == "--pr" ]]; then')
    pr_fast_formal = release_source.index(
        "run_sumeragi_v2_harness.sh --unit", pr_branch
    )
    formal_definition = release_source.index("run_release_formal_gate() {")
    formal_gate = release_source.index("run_sumeragi_v2_formal_release.sh")
    formal_call = release_source.index("\n  run_release_formal_gate\n", formal_gate)
    scaling_definition = release_source.index("run_release_scaling_gate() {")
    scaling_gate = release_source.index(
        "validate_multilane_scaling_evidence.py", scaling_definition
    )
    g12_soak = release_source.index(
        'verify_release_identity "after G-12P two-hour rotating-validator fault soak"'
    )
    scaling_call = release_source.index("\n  run_release_scaling_gate\n", g12_soak)
    chaos_gate = release_source.index("run_sumeragi_v2_100k_chaos.sh")
    pre_soak_manifest = release_source.index("pre_soak_source_manifest_sha256")
    taira_run = release_source.index("run_taira_v2_24h_soak.sh")
    final_manifest = release_source.index("final_release_source_manifest_sha256")
    final_proof_check = release_source.index("final_proof_evidence_args=(")
    final_proof_invocation = release_source.index(
        '"${final_proof_evidence_args[@]}"', final_proof_check
    )
    sealed_child_call = release_source.index(
        '"$release_child_bin/bash" '
        '"$sealed_repo_root/scripts/run_sumeragi_v2_release_gates.sh" --release'
    )
    outer_child_status = release_source.index("  sealed_status=$?", sealed_child_call)
    aggregate_receipt = release_source.index(
        "write_sumeragi_v2_release_receipt.py", outer_child_status
    )
    protected_receipt_validation = release_source.index(
        '"$release_bootstrap_evidence_dir/validate-receipt.py"',
        aggregate_receipt,
    )
    assert (
        bootstrap_validation
        < required_network
        < production_units
        < taira_rust_contracts
        < source_contract_preflight
        < seed_launcher_preflight
        < chaos_launcher_preflight
        < receipt_contract_preflight
        < proof_fidelity_preflight
        < formal_launcher_preflight
        < taira_soak_preflight
        < formal_definition
        < formal_gate
        < formal_call
        < seed_matrix
        < chaos_gate
        < pre_soak_manifest
        < taira_run
        < final_manifest
        < final_proof_check
    )
    assert (
        sealed_child_call
        < outer_child_status
        < aggregate_receipt
        < protected_receipt_validation
    )
    assert scaling_definition < scaling_gate < g12_soak < scaling_call < pr_branch
    assert "run_release_scaling_and_formal_gates" not in release_source
    assert release_source.count("\n  run_release_formal_gate\n") == 1
    assert release_source.count("\n  run_release_scaling_gate\n") == 1
    assert seed_matrix < pr_branch < pr_fast_formal
    final_proof_region = release_source[
        final_proof_check : final_proof_invocation
        + len('"${final_proof_evidence_args[@]}"')
    ]
    assert "--release" in final_proof_region
    assert (
        '--evidence "${SUMERAGI_V2_FORMAL_EVIDENCE_DIR}/proof_evidence.json"'
        in final_proof_region
    )
    assert (
        '--verus-evidence "${SUMERAGI_V2_FORMAL_EVIDENCE_DIR}/verus_evidence.json"'
        in final_proof_region
    )
    assert '--print-cross-tool-obligations' in release_source[
        final_manifest:final_proof_check
    ]
    assert '--cross-tool-evidence "$cross_tool_evidence_path"' in final_proof_region
    assert '"${final_proof_evidence_args[@]}"' in final_proof_region
    assert "/tmp/iroha-sumeragi-v2-release-host-" not in release_source
    assert "IROHA_RELEASE_AGGREGATE_RECEIPT_PATH_FILE" not in release_source
    assert "release_invocation_base=/private/tmp" in release_source
    assert "release_invocation_base=/tmp" in release_source
    assert (
        'tempfile.mkdtemp(prefix="iroha-sumeragi-v2-release.", dir=base)'
        in release_source
    )
    assert (
        '"$release_invocation_base" "$repo_root" '
        '"$release_bootstrap_evidence_dir" \\\n'
        '    "$inherited_cargo_cache_home"'
        in release_source
    )
    assert 'readonly sealed_repo_root="${release_invocation_root}/source"' in release_source
    assert '--bootstrap-completion "$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION"' in release_source
    assert '--expected-bootstrap-completion-sha256' in release_source
    assert '--bootstrap-candidate-root "$repo_root"' in release_source
    assert '--bootstrap-runner "$repo_root/scripts/run_sumeragi_v2_release_gates.sh"' in release_source
    assert 'mv -- "$release_receipt_partial"' not in release_source
    production_inventory_end = release_source.index("\n)", production_units)
    production_inventory = tuple(
        line.strip()
        for line in release_source[
            production_units:production_inventory_end
        ].splitlines()
        if line.strip().startswith(
            (
                "sumeragi::",
                "sumeragi_v2_runner::",
                "kura::",
                "nexus::",
                "merge_sidecar::",
                "state::",
                "zk::",
                "block::",
                "offline::",
                "peer::",
                "network::",
                "consensus_message_control::tests::",
                "network_relay_tests::",
                "tests::relay_fairness::",
                "parameters::",
            )
        )
    )
    assert len(production_inventory) == 864
    assert len(set(production_inventory)) == 864
    native_merge_projection_regressions = {
        "sumeragi::v2_lane_work::tests::native_amx_manifest_projects_finality_bound_merge_batch_in_canonical_order",
        "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_multiple_participant_heights_in_one_carrier",
        "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_height_participant_identity_conflict",
        "sumeragi::v2_lane_work::tests::native_amx_merge_projection_excludes_coordinator_only_receipts",
        "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_same_route_identity_conflict",
        "sumeragi::v2_lane_work::tests::native_amx_merge_projection_rejects_duplicate_group_source",
        "sumeragi::v2_lane_work::tests::native_amx_merge_projection_matches_decoded_replay_entry",
    }
    assert native_merge_projection_regressions <= set(production_inventory)
    assert native_merge_projection_regressions <= set(
        module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    )
    deferred_terminal_regressions = {
        "sumeragi::v2_apply::tests::deferred_canonical_carrier_owned_and_absent_groups_complete_before_gate_publication",
        "sumeragi::v2_apply::tests::deferred_canonical_carrier_missing_after_queue_cleanup_keeps_startup_gate_closed",
    }
    assert deferred_terminal_regressions <= set(production_inventory)
    assert deferred_terminal_regressions <= set(
        module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    )
    terminal_sweep_regression = (
        "sumeragi::v2_runner::tests::"
        "terminal_sweep_source_partitions_whole_units_before_any_mutation"
    )
    assert terminal_sweep_regression in production_inventory
    assert terminal_sweep_regression in module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    assert not any(
        test.startswith("sumeragi::v2_core::network_simulation::")
        for test in production_inventory
    )
    leader_wire_slot_product_regression = (
        "sumeragi::serviced_candidate_store::tests::"
        "leader_wire_gate_retains_independent_cross_origin_phase_and_chunk_slots"
    )
    assert leader_wire_slot_product_regression in production_inventory
    assert (
        leader_wire_slot_product_regression
        in module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    )
    exact_certified_serve_regressions = {
        "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::turn_cut_dequeues_exact_winner_once_and_preserves_ready_rotation",
        "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_same_source_serve",
        "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_churn",
        "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_occurrence_ordinal_coalesces_and_overflow_closes",
        "sumeragi::v2_certified_serve_payload_store::tests::authenticated_cut_rejects_a_later_valid_payload_from_a_second_store_owner",
        "sumeragi::v2_certified_serve_payload_store::tests::authenticated_cut_rejects_store_directory_symlink_replacement",
        "sumeragi::v2_certified_serve_payload_store::tests::capacity_is_checked_before_a_second_file_is_published",
        "sumeragi::v2_certified_serve_payload_store::tests::completed_payload_requires_exact_certified_responder_authority",
        "sumeragi::v2_certified_serve_payload_store::tests::completed_payload_requires_exact_durable_body_receipt_and_bytes",
        "sumeragi::v2_certified_serve_payload_store::tests::negative_terminal_is_idempotent_and_cannot_be_replaced",
        "sumeragi::v2_certified_serve_payload_store::tests::only_the_call_that_created_pending_owns_preledger_abort_authority",
        "sumeragi::v2_certified_serve_payload_store::tests::pending_receipt_requires_verified_qc_and_local_retention_authority",
        "sumeragi::v2_certified_serve_payload_store::tests::recovery_cut_reauthenticates_request_qc_and_typed_negative",
        "sumeragi::v2_certified_serve_payload_store::tests::recovery_cut_reconstructs_and_authenticates_completed_response",
        "sumeragi::v2_lifecycle_coordinator::ledger::tests::frame_roundtrip_is_canonical_and_preserves_high_water",
        "sumeragi::v2_lifecycle_coordinator::ledger::tests::one_signed_serve_request_cannot_own_two_lifecycle_pairs",
        "sumeragi::v2_lifecycle_coordinator::ledger::tests::orphan_serve_or_producer_records_are_rejected",
        "sumeragi::v2_lifecycle_coordinator::projection::tests::cancelled_certified_serve_tombstone_replays_with_its_terminal_producer_pair",
        "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_serve_completion_settles_from_the_post_fsync_response_receipt",
        "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_serve_negative_settlement_requires_the_exact_post_fsync_receipt",
        "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_serve_rejects_a_receipt_for_another_signed_request",
        "sumeragi::v2_lifecycle_coordinator::projection::tests::certified_serve_terminal_family_mismatch_fails_without_state_mutation",
        "sumeragi::v2_lifecycle_coordinator::projection::tests::pending_certified_serve_admits_one_ready_serve_and_adjacent_dormant_producer",
        "sumeragi::v2_lifecycle_coordinator::replay_authority::tests::certified_serve_pending_replay_pair_binds_exact_fsync_origin_and_records",
        "sumeragi::v2_lifecycle_coordinator::replay_authority::tests::recovered_serve_states_reconstruct_one_common_source_per_replay_pair",
        "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::post_cut_append_preserves_geometry_but_pre_cut_mutation_fails_cas",
        "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::prepared_commit_preserves_unrelated_post_cut_append",
        "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::locked_publication_fence_serializes_same_wire_and_reenqueues_after_commit",
        "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::locked_publication_fence_serializes_unrelated_append_and_preserves_it",
        "sumeragi::v2_lifecycle_coordinator::ingress_position::tests::dropping_locked_publication_fence_releases_producer_without_dequeue",
        "sumeragi::v2_lifecycle_coordinator::launch::turn_driver::ordinary_ingress_token_tests::armed_token_closes_output_before_releasing_dequeued_carrier_and_serve_result",
        "sumeragi::v2_lifecycle_coordinator::open::recovery_tests::complete_tip_serve_reconciliation_binds_the_exact_source_frame",
        "sumeragi::v2_lifecycle_coordinator::open::recovery_tests::complete_tip_serve_reconciliation_rejects_missing_final_cut_coverage",
        "sumeragi::v2_lifecycle_coordinator::scheduler_inputs::certified_serve_scheduler_tests::certified_serve_claim_rolls_back_when_its_exact_carrier_drifted",
        "sumeragi::v2_lifecycle_coordinator::scheduler_inputs::certified_serve_scheduler_tests::certified_serve_scheduler_creates_exactly_one_live_claim",
        "sumeragi::v2_lifecycle_coordinator::tests::capacity_fence_freezes_the_complete_serve_companion",
        "sumeragi::v2_lifecycle_coordinator::tests::durable_rollover_rejects_live_serve_without_payload_cancellation_receipt",
        "sumeragi::v2_lifecycle_coordinator::tests::recovery_requires_a_bijective_atomic_serve_producer_pair",
        "sumeragi::v2_lifecycle_coordinator::tests::restart_derives_ready_producer_debt_from_terminal_serve",
        "sumeragi::v2_lifecycle_coordinator::tests::serve_and_producer_share_one_reconstruction_source",
        "sumeragi::v2_lifecycle_coordinator::tests::serve_and_producer_terminalization_fail_closed_without_the_atomic_debt",
    }
    assert len(exact_certified_serve_regressions) == 41
    assert exact_certified_serve_regressions <= set(production_inventory)
    assert exact_certified_serve_regressions <= set(
        module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    )
    final_replenishment_lasso_regressions = {
        "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence",
        "sumeragi::v2::tests::deferred_occurrence_capability_binds_direct_authenticated_provenance",
        "sumeragi::v2_runtime::tests::runtime_rejects_driver_selection_outside_eligible_deferred_owner_set",
        "sumeragi::v2_runtime::tests::runtime_physical_cut_is_monotone_and_regression_fails_closed",
        "sumeragi::v2_runtime::tests::deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences",
        "sumeragi::v2_runtime::tests::post_cut_old_logical_replay_cannot_overtake_fenced_busy_deferred_target",
        "sumeragi::v2_runtime::tests::pre_dequeue_probe_validates_unfrozen_leader_wire_identity",
        "sumeragi::v2_runtime::tests::busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation",
        "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::fresh_certified_serve_rejects_foreign_target_and_rolls_back_capacity_wait",
    }
    assert final_replenishment_lasso_regressions <= set(production_inventory)
    assert final_replenishment_lasso_regressions <= set(
        module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    )
    deterministic_ownership_regressions = {
        f"{module_name}{test_name}"
        for module_name, test_name, _ in deterministic_ownership_inventory_additions
    }
    assert len(deterministic_ownership_regressions) == 13
    assert deterministic_ownership_regressions <= set(production_inventory)
    assert deterministic_ownership_regressions <= set(
        module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    )
    recovered_decision_fetch_regression = "sumeragi::v2_lifecycle_coordinator::launch::tests::recovered_decision_fetch_composite_dispatch_reserves_capacity_before_claim_and_commit"
    assert recovered_decision_fetch_regression in production_inventory
    assert recovered_decision_fetch_regression in module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    autonomous_retirement_regression = (
        "sumeragi::v2_worker::tests::"
        "applied_height_handoff_retires_only_exact_same_finality_nonwinning_autonomous_outputs_atomically"
    )
    assert autonomous_retirement_regression in production_inventory
    assert (
        autonomous_retirement_regression
        in module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
    )
    for predecessor_durability_regression in (
        "sumeragi::v2_worker::tests::"
        "applied_height_handoff_accepts_kura_applied_ordinary_historical_lane_output",
        "sumeragi::v2_worker::tests::"
        "applied_height_handoff_accepts_record_backed_autonomous_historical_lane_certificate",
    ):
        assert predecessor_durability_regression in production_inventory
        assert (
            predecessor_durability_regression
            in module._PRODUCTION_LIVENESS_NEW_REGRESSIONS
        )
    assert len(module._PRODUCTION_LIVENESS_NEW_REGRESSIONS) == 445
    assert "readonly expected_production_liveness_test_count=864" in release_source
    assert (
        "readonly expected_typed_rollover_formal_mutation_count=45"
        in release_source
    )
    assert (
        'echo "[tlc] typed rollover-handoff repaired models and 45-mutant '
        'root-anchored V3 matrix passed"'
        in release_source
    )
    assert "_PRODUCTION_TEST_COUNT = 864" in receipt_source
    receipt_spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_release_receipt_inventory",
        ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py",
    )
    assert receipt_spec is not None
    assert receipt_spec.loader is not None
    receipt_module = importlib.util.module_from_spec(receipt_spec)
    sys.modules[receipt_spec.name] = receipt_module
    receipt_spec.loader.exec_module(receipt_module)
    assert sum(count for _, _, count in receipt_module._PRODUCTION_MODULES) == 864
    assert (
        receipt_module._PRODUCTION_MODULES
        == module._PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
    )
    assert all(
        module_name != "sumeragi::v2_core::network_simulation"
        for _leg_id, module_name, _count in receipt_module._PRODUCTION_MODULES
    )
    assert (
        len(receipt_module._corridor_legs())
        == module._PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT
        == 91
    )
    assert len(receipt_module._TAIRA_CONTRACT_TESTS) == 6
    assert receipt_module._TAIRA_CONTRACT_TESTS[-1] == (
        "taira_public_localnet::strict_restart::taira_localnet_restart_catchup_behavior"
    )
    assert receipt_module._production_module_command(
        "parameters::actual::tests"
    ) == (
        "cargo test --locked --offline -p iroha_config --lib "
        "parameters::actual::tests "
        "-- --test-threads=1"
    )
    assert receipt_module._production_module_command(
        "parameters::user::duration_clamp_tests"
    ) == (
        "cargo test --locked --offline -p iroha_config --lib "
        "parameters::user::duration_clamp_tests -- --test-threads=1"
    )
    assert receipt_module._production_module_command(
        "block::consensus_v2::finality::tests"
    ) == (
        "cargo test --locked --offline -p iroha_data_model --lib "
        "block::consensus_v2::finality::tests -- --test-threads=1"
    )
    for _, production_module, expected_count in receipt_module._PRODUCTION_MODULES:
        assert (
            sum(
                test.startswith(f"{production_module}::")
                for test in production_inventory
            )
            == expected_count
        )
    for production_module, test_name, _ in production_inventory_additions:
        assert f"{production_module}{test_name}" in production_inventory
    for required_test in (
        "kura::tests::progress_witness_durability::"
        "absent_progress_namespace_requires_every_directory_barrier",
        "kura::tests::progress_witness_durability::"
        "bound_progress_recovery_handles_crash_phases_without_path_escape",
        "kura::tests::progress_witness_durability::"
        "direct_receipt_snapshot_preserves_sparse_and_mixed_format_entries",
        "kura::tests::progress_witness_durability::"
        "initial_preindex_data_sync_failure_rolls_back_payload_before_retry",
        "kura::tests::progress_witness_durability::"
        "lane_block_application_receipt_strict_retry_reissues_every_barrier",
        "kura::tests::progress_witness_durability::"
        "progress_sidecar_mutation_rejects_symlinks_without_external_writes",
        "kura::tests::progress_witness_durability::"
        "progress_prepend_directory_failure_retries_without_corruption",
        "kura::tests::progress_witness_durability::"
        "unindexed_crash_suffix_is_repaired_before_retry_or_append",
        "kura::lane_geometry::tests::"
        "first_release_retirement_requires_bound_progress_sidecar_durability",
        "kura::lane_geometry::tests::"
        "geometry_gc_requires_bound_merge_receipt_durability_before_deletion",
        "sumeragi::v2_core::tests::"
        "timeout_elapsed_cannot_start_durable_timeout_after_decision",
        "sumeragi::v2_core::tests::"
        "quorum_completing_timeout_vote_cannot_form_tc_after_decision",
        "sumeragi::status::v2_liveness_watchdog_tests::"
        "active_watchdog_is_deadline_driven_edge_triggered_and_recovers_on_progress",
        "sumeragi::status::v2_liveness_watchdog_tests::"
        "active_watchdog_resets_on_successor_owner_and_status_clear",
        "peer::shared_byte_budget_tests::"
        "frame_retention_coalesces_each_distinct_source_owner_without_reaccounting",
        "peer::shared_byte_budget_tests::"
        "authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift",
        "network::tests::reconnecting_peer_cannot_multiply_retained_source_credits",
        "sumeragi::v2_core::refinement::tests::"
        "two_stage_relay_retry_kernel_rejects_source_rotation_eligibility_and_fifo_mutations",
        "tests::relay_fairness::"
        "daemon_source_credit_layers_over_upstream_and_preserves_the_ninth_exact_owner",
        "tests::relay_fairness::"
        "saturated_sumeragi_dispatch_does_not_hold_normal_worker_permits",
        "tests::relay_fairness::"
        "real_inner_ingress_retry_preserves_a_copies_and_bounds_b_service_rank",
        "block::consensus_v2::finality::tests::"
        "header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round",
        "sumeragi::v2_core::tests::"
        "future_prepare_qc_is_transactionally_ignored_without_retransmit_ownership",
        "sumeragi::v2::tests::"
        "successor_context_requires_the_durable_cryptographic_parent",
        "sumeragi::v2_lane_work::tests::"
        "late_old_sidecar_flush_removes_only_reconnected_source_retry",
        "sumeragi::v2_recovery::tests::"
        "finalized_tip_derives_one_idempotent_successor_context",
        "sumeragi::v2_runner::tests::"
        "closed_sidecar_prefix_handoff_requeues_only_failed_suffix",
        "sumeragi::v2_runner::tests::"
        "exact_locked_body_is_reencoded_at_the_reproposal_round_without_byte_drift",
    ):
        assert required_test in production_inventory
    assert (
        'required_data_model_lane_certificate_test="block::consensus::tests::'
        'lane_block_certificate_decodes_atomically_from_slice"'
        in release_source
    )
    assert '"lane-certificate-rust"' in receipt_source
    assert "_DATA_LANE_CERTIFICATE_TEST" in receipt_source
    assert (
        "sumeragi::v2_effects::tests::"
        "runtime_step_dispatches_entire_effect_batch_before_returning"
        in production_inventory
    )
    assert (
        "sumeragi::v2_apply::tests::"
        "committed_merge_reservation_rejects_bare_norito"
        in production_inventory
    )
    assert (
        "sumeragi::v2_worker::tests::"
        "locked_candidate_future_completion_is_rejected_without_replacing_owner"
        in production_inventory
    )
    assert (
        "sumeragi::v2_worker::tests::"
        "unavailable_locked_candidate_rebinds_latest_consumer_before_retry"
        in production_inventory
    )
    assert (
        "sumeragi::v2_effects::tests::"
        "decision_installed_by_same_runtime_step_retires_stale_terminal_effects"
        in production_inventory
    )
    assert (
        "sumeragi::v2_effects::tests::"
        "decision_installed_by_same_runtime_step_keeps_exact_commit_and_body_work"
        in production_inventory
    )
    production_modules_start = release_source.index("production_liveness_modules=(")
    production_modules_end = release_source.index("\n)", production_modules_start)
    production_modules = tuple(
        line.strip()
        for line in release_source[
            production_modules_start:production_modules_end
        ].splitlines()
        if line.strip().startswith(
            (
                "sumeragi::",
                "sumeragi_v2_runner",
                "kura::",
                "nexus::",
                "merge_sidecar::",
                "state::",
                "zk::",
                "block::",
                "offline::",
                "peer::",
                "network::",
                "consensus_message_control::tests",
                "network_relay_tests",
                "tests::relay_fairness",
                "parameters::",
            )
        )
    )
    assert len(production_modules) == 43
    assert len(set(production_modules)) == 43
    assert "kura::tests" in production_modules
    assert "kura::lane_geometry::tests" in production_modules
    assert "sumeragi::authoritative_runtime_gate_tests" in production_modules
    assert "sumeragi::v2_core::network_simulation" not in production_modules
    assert "sumeragi::v2_block_sync::tests" in production_modules
    assert "sumeragi::v2_apply::tests" in production_modules
    assert "sumeragi::v2_certified_serve_payload_store::tests" in production_modules
    assert "sumeragi::v2_lifecycle_coordinator" in production_modules
    assert "sumeragi::v2_runner::lifecycle_height_driver::tests" in production_modules
    assert "sumeragi_v2_runner" in production_modules
    assert "peer::run::tests" in production_modules
    assert "network::tests" in production_modules
    assert "merge_sidecar::tests" in production_modules
    assert "consensus_message_control::tests" in production_modules
    assert "network_relay_tests" in production_modules
    assert "tests::relay_fairness" in production_modules
    assert "parameters::actual::tests" in production_modules
    assert "parameters::user::duration_clamp_tests" in production_modules
    assert (
        'for module_index in "${!production_liveness_modules[@]}"; do'
        in release_source
    )
    assert (
        'cargo test --locked --offline -p iroha_core --lib "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        'cargo test --locked --offline -p iroha_p2p --lib "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        "cargo test --locked --offline -p irohad --bin iroha3d --features "
        'test-network-message-control \\\n        "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        'cargo test --locked --offline -p iroha_config --lib "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        'production_config_unit_list="$(run_cargo test --locked --offline -p iroha_config '
        '--lib -- --list)"'
        in release_source
    )
    assert (
        'production_config_ignored_unit_list="$(\n'
        '  run_cargo test --locked --offline -p iroha_config --lib -- --list --ignored\n'
        ')"'
        in release_source
    )
    assert 'elif [[ "$required_test" == parameters::* ]]; then' in release_source
    assert (
        "cargo test --locked --offline -p integration_tests --test "
        "sumeragi_v2_runner_isolated "
        "sumeragi_v2_runner::prepare_qc_split_tests "
        "-- --test-threads=1"
        in release_source
    )
    assert (
        '_PRODUCTION_INTEGRATION_MODULE = "sumeragi_v2_runner::prepare_qc_split_tests"'
        in receipt_source
    )
    assert "production_integration_ignored_unit_list=" in release_source
    assert (
        "serialized_runtime_rebinds_busy_deferred_body_completion_before_service"
        in release_source
    )
    assert (
        "tc_body_rebind_preserves_certified_request_ownership_through_signed_response"
        in release_source
    )
    assert "replay_does_not_resign_commit_superseded_by_higher_tc_lock" in release_source
    assert "fetch_consumer_rebind_preserves_live_or_queued_reconstruction_owner" in release_source
    assert "blocker_classifier_has_stable_specific_precedence" in release_source
    assert (
        "current_view_timeout_path_yields_only_to_an_exact_locked_commit_owner"
        in release_source
    )
    assert "missing required production Sumeragi v2 liveness test" in release_source
    assert "production_ignored_unit_list=" in release_source
    assert "required production Sumeragi v2 liveness test is ignored" in release_source
    assert "compute_workspace_source_manifest.py" in release_source
    assert 'compute_workspace_source_manifest.py --root "$repo_root"' in release_source
    assert "IROHA_RELEASE_SOURCE_MANIFEST_SHA256" in release_source
    assert "source_manifest_contract_tests=(" in release_source
    assert "pytests/scripts/workspace_source_manifest_test.py" in release_source
    assert "pytests/scripts/seal_workspace_source_test.py" in release_source
    assert "did not run exactly 78 passing tests" in release_source
    assert "preflight-source-seal pytest 78" in release_source
    assert (
        '"preflight-source-seal",\n                "pytest",\n                78,'
        in receipt_source
    )
    assert "seed_launcher_contract_tests=(" in release_source
    assert "did not run exactly 14 passing tests" in release_source
    for seed_contract in (
        "test_mocked_seed_matrix_rejects_bundle_tampering_before_completion",
        "test_mocked_seed_matrix_rejects_symlinked_marker_temp_without_completion",
        "test_mocked_seed_matrix_marker_durability_failure_is_not_terminal",
    ):
        assert seed_contract in release_source
    assert (
        '"preflight-seed-launcher",\n                "pytest",\n                14,'
        in receipt_source
    )
    assert "did not run exactly five passing tests" in release_source
    assert "preflight-chaos-launcher pytest 5" in release_source
    assert "did not run exactly 75 passing tests" in release_source
    assert "preflight-release-identity pytest 75" in release_source
    assert "did not run exactly 258 passing tests" in release_source
    assert "preflight-release-bootstrap pytest 258" in release_source
    assert "did not run exactly 44 passing tests" in release_source
    assert "preflight-release-bootstrap-validator pytest 44" in release_source
    assert "did not run exactly 368 passing tests" in release_source
    assert "preflight-release-receipt pytest 368" in release_source
    assert (
        "pytests/scripts/sumeragi_v2_release_receipt_components_test.py"
        in release_source
    )
    assert "pytests/scripts/sumeragi_v2_prebuilt_bundle_test.py" in release_source
    assert (
        "pytests/scripts/sumeragi_v2_prebuilt_bundle_shell_test.py"
        in release_source
    )
    assert (
        "pytests/scripts/sumeragi_v2_release_process_policy_test.py"
        in release_source
    )
    assert (
        '"preflight-chaos-launcher",\n                "pytest",\n                5,'
        in receipt_source
    )
    assert (
        '"preflight-release-identity",\n                "pytest",\n                75,'
        in receipt_source
    )
    assert (
        '"preflight-release-bootstrap",\n                "pytest",\n                258,'
        in receipt_source
    )
    assert (
        '"preflight-release-bootstrap-validator",\n                "pytest",\n                44,'
        in receipt_source
    )
    assert (
        '"preflight-release-receipt",\n                "pytest",\n                368,'
        in receipt_source
    )
    assert "did not run exactly 5513 passing tests" in release_source
    assert "preflight-proof-fidelity pytest 5513" in release_source
    assert (
        "^5513 passed in [0-9]+([.][0-9]+)?s( "
        r"\([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$"
        in release_source
    )
    assert (
        r'r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s"' in release_source
    )
    assert (
        r'r"(?: \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?"'
        in release_source
    )
    for contract_file in (
        "pytests/scripts/sumeragi_v2_proof_ledger_test.py",
        "pytests/scripts/sumeragi_v2_verus_evidence_test.py",
        "pytests/scripts/sumeragi_v2_tlc_trace_normalizer_test.py",
        "pytests/scripts/sumeragi_v2_reviewed_rust_source_test.py",
        "pytests/scripts/sumeragi_v2_multilane_native_merge_manifest_test.py",
        "pytests/scripts/sumeragi_v2_multilane_passive_recovery_contract_test.py",
    ):
        assert contract_file in release_source
        assert contract_file in receipt_source
    proof_fidelity_receipt_commands = [
        command
        for leg_id, _kind, _count, command in receipt_module._corridor_legs()
        if leg_id == "preflight-proof-fidelity"
    ]
    assert len(proof_fidelity_receipt_commands) == 1
    proof_fidelity_receipt_command = proof_fidelity_receipt_commands[0]
    proof_fidelity_array_start = release_source.index(
        "proof_fidelity_contract_files=(\n"
    ) + len("proof_fidelity_contract_files=(\n")
    proof_fidelity_array_end = release_source.index(
        "\n)", proof_fidelity_array_start
    )
    proof_fidelity_runner_nodes = tuple(
        line.strip()
        for line in release_source[
            proof_fidelity_array_start:proof_fidelity_array_end
        ].splitlines()
        if line.strip()
    )
    proof_fidelity_command_prefix = (
        "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
        "-q -p no:cacheprovider "
    )
    assert proof_fidelity_receipt_command.startswith(proof_fidelity_command_prefix)
    proof_fidelity_receipt_nodes = tuple(
        proof_fidelity_receipt_command[len(proof_fidelity_command_prefix) :].split()
    )
    assert len(proof_fidelity_runner_nodes) == 18
    assert len(set(proof_fidelity_runner_nodes)) == 18
    assert proof_fidelity_runner_nodes == proof_fidelity_receipt_nodes
    assert "Collection is source-bound as 5,410 ledger/checker cases" in release_source
    for selector in (
        "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
        "test_inflight_composed_contract_rejects_legacy_layout_only_claim",
        "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
        "test_inflight_composed_contract_rejects_state_order_weakening",
        "pytests/scripts/sumeragi_v2_multilane_models_terminal_tail_test.py::"
        "test_inflight_composed_contract_rejects_snapshot_nonstutter_mapping",
        "pytests/scripts/sumeragi_v2_multilane_models_terminal_tail_test.py::"
        "test_inflight_composed_contract_rejects_missing_direct_release_action",
        "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
        "test_inflight_layout_contract_rejects_action_inventory_weakening",
        "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
        "test_inflight_composed_contract_rejects_per_key_prefix_skip_weakening",
        "pytests/scripts/sumeragi_v2_multilane_models_tail_test.py::"
        "test_inflight_composed_contract_rejects_tla_snapshot_nonstutter_mapping",
        "pytests/scripts/sumeragi_v2_multilane_models_tail_test.py::"
        "test_inflight_composed_contract_rejects_verus_snapshot_stutter_proof_removal",
        "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
        "test_inflight_layout_contract_rejects_membership_only_lane_authorship",
        "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
        "test_wire_release_invariant_binds_current_semantic_sources",
        "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
        "test_wire_release_invariant_rejects_ledger_weakening",
        "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
        "test_wire_release_invariant_rejects_semantic_source_mutation",
    ):
        assert selector in release_source
        assert selector in proof_fidelity_receipt_command
    assert (
        '"preflight-proof-fidelity",\n                "pytest",\n                5513,'
        in receipt_source
    )
    assert "did not run exactly 27 passing tests" in release_source
    assert "preflight-formal-launcher pytest 27" in release_source
    assert (
        "^27 passed in [0-9]+([.][0-9]+)?s( "
        r"\([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$"
        in release_source
    )
    assert (
        '"preflight-formal-launcher",\n                "pytest",\n                27,'
        in receipt_source
    )
    assert "taira_release_ignored_contract_list=" in release_source
    assert "required Taira release-evidence contract test is ignored" in release_source
    for test_name in (
        "release_execution_profile_accepts_only_the_exact_positive_profile",
        "release_execution_profile_rejects_wrong_or_blank_build_profiles",
        "release_execution_profile_rejects_cargo_profile_mismatch",
        "release_execution_profile_rejects_non_exact_offline_values",
        "simulation_summary_json_records_release_profile_and_status_evidence",
        "strict_restart::taira_localnet_restart_catchup_behavior",
    ):
        assert test_name in release_source
    strict_restart_declaration = "async fn taira_localnet_restart_catchup_behavior()"
    assert taira_strict_restart_source.count(strict_restart_declaration) == 1
    strict_restart_offset = taira_strict_restart_source.index(strict_restart_declaration)
    strict_restart_attribute = taira_strict_restart_source.rfind(
        "#[tokio::test", 0, strict_restart_offset
    )
    assert strict_restart_attribute >= 0
    strict_restart_test = taira_strict_restart_source[strict_restart_attribute:]
    for forbidden_success_path in (
        "#[ignore",
        "finalize_result(",
        "sandbox_reason",
        "enforce_network_start_requirement",
        "skipping",
    ):
        assert forbidden_success_path not in taira_strict_restart_source
    assert "setup_taira_harness::<true>" in strict_restart_test
    assert strict_restart_test.count("strict_process_restart_catchup(") == 1
    assert "\n    result\n}" in strict_restart_test
    assert "const TAIRA_VALIDATORS: u16 = 4;" in taira_source
    for exact_tip_token in (
        ".query(FindBlocks)",
        "height: tip.header().height().get(),",
        "hash: tip.hash(),",
        "parent_hash: tip.header().prev_block_hash(),",
        "tips.iter().all(|(_, tip)| *tip == expected)",
        "observed.block == expected && observed.contains_transaction",
    ):
        assert exact_tip_token in taira_strict_restart_source
    strict_restart_helper_offset = taira_strict_restart_source.index(
        "async fn strict_process_restart_catchup("
    )
    strict_restart_helper = taira_strict_restart_source[
        strict_restart_helper_offset:strict_restart_attribute
    ]
    assert strict_restart_helper.count("signed_probe(") == 2
    assert strict_restart_helper.count("submit_signed(") == 2
    for restart_contract_token in (
        "harness.validator_clients.len() == usize::from(TAIRA_VALIDATORS)",
        "let baseline = wait_for_all_common_tip(",
        '"strict four-validator baseline convergence"',
        "harness.localnet.stop_validator(restart_index)?;",
        "let sentinel = signed_probe(",
        "let sentinel_block = wait_for_all_signed_tip(",
        "sentinel_block.height",
        "&& sentinel_block.parent_hash == Some(baseline.hash)",
        "harness.localnet.start_validator(restart_index)?;",
        "Some(sentinel_block),",
        '"restarted validator must reach the exact sentinel height/hash"',
        "ensure!(recovered == sentinel_block);",
        "let successor = signed_probe(",
        "let successor_block = wait_for_all_signed_tip(",
        '"all four validators must finalize the exact successor"',
        "successor_block.height",
        "&& successor_block.parent_hash == Some(sentinel_block.hash)",
        "&& successor_block.hash != sentinel_block.hash",
    ):
        assert restart_contract_token in strict_restart_helper
    assert strict_restart_helper.count(".checked_add(1)") == 2
    strict_setup = taira_source.index(
        "async fn setup_taira_harness<const STRICT_ALL_VALIDATORS: bool>"
    )
    strict_initial_failure = taira_source.index("if STRICT_ALL_VALIDATORS", strict_setup)
    assert taira_source.index("return Err(err);", strict_initial_failure) > strict_initial_failure
    process_churn_start = taira_source.index("async fn process_churn_cycle(")
    process_churn_end = taira_source.index(
        "fn validator_restart_catchup_target(", process_churn_start
    )
    all_validator_lag_branch = taira_source.index(
        "} else if let Err(err) = wait_for_cluster_convergence(",
        process_churn_start,
        process_churn_end,
    )
    all_validator_lag_diagnostic = taira_source.index(
        '"validator restart all-validator convergence lagged;',
        all_validator_lag_branch,
        process_churn_end,
    )
    assert (
        taira_source.index(
            "lagged = true;",
            all_validator_lag_branch,
            all_validator_lag_diagnostic,
        )
        > all_validator_lag_branch
    )
    assert "taira_soak_contract_files=(" in release_source
    assert "did not run exactly 43 passing tests" in release_source
    for soak_contract in (
        "test_launcher_rejects_bundle_tampering_before_completion",
        "test_launcher_rejects_symlinked_marker_temp_without_completion",
        "test_launcher_marker_durability_failure_is_not_terminal",
        "test_launcher_does_not_promote_provisional_evidence_when_validation_fails",
    ):
        assert soak_contract in release_source
    assert (
        '"preflight-taira-soak",\n                "pytest",\n                43,'
        in receipt_source
    )
    assert (
        "expected_corridor_leg_count="
        f"{module._PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT}"
        in release_source
    )
    assert 'export IROHA_RELEASE_CARGO_BIN="$release_cargo_bin"' in release_source
    assert (
        'corridor_cargo_path="$(canonical_path "$IROHA_RELEASE_CARGO_BIN")"'
        in release_source
    )
    assert (
        'expected_legs = _corridor_legs(fields["cargo_path"])'
        in receipt_source
    )
    for leg_id, command in (
        (
            "source-sealed-workspace-build",
            "${IROHA_RELEASE_CARGO_BIN} build -j1 --locked --offline --workspace",
        ),
        (
            "source-sealed-workspace-tests",
            "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --offline --workspace",
        ),
        (
            "source-sealed-irohad-tests",
            "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --offline -p irohad --bin irohad "
            "--features test-network-message-control",
        ),
        (
            "source-sealed-workspace-clippy",
            "${IROHA_RELEASE_CARGO_BIN} clippy -j1 --locked --offline --workspace --all-targets "
            "-- -D warnings",
        ),
        (
            "source-sealed-workspace-format",
            "${IROHA_RELEASE_CARGO_BIN} fmt --all -- --check",
        ),
        (
            "source-sealed-legacy-codec-guard",
            "bash scripts/check_no_legacy_codec.sh",
        ),
    ):
        assert f"  {leg_id} command 0" in release_source
        assert command in release_source
        assert any(
            receipt_leg_id == leg_id
            and kind == "command"
            and expected_count == 0
            and receipt_command == command
            for receipt_leg_id, kind, expected_count, receipt_command in receipt_module._corridor_legs()
        )
    assert (
        'scripts/nexus/validate_multilane_scaling_evidence.py \\\n'
        '    "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"'
        in release_source
    )
    assert '--expected-source-revision "$release_head_commit"' in release_source
    assert (
        '--expected-workspace-source-sha256 "$release_source_manifest_sha256"'
        in release_source
    )
    assert "--expected-validator-sha256" in release_source
    for expected_flag in (
        "--expected-trial-harness-sha256",
        "--expected-configuration-sha256",
        "--expected-irohad-sha256",
        "--expected-iroha-cli-sha256",
        "--expected-repository-root",
    ):
        assert expected_flag in release_source
    assert (
        '--g4p-completion "$multilane_four_peer_completion_path" \\\n'
        '      --g12-seed-completion "$nexus_cross_completion_path" \\\n'
        '      --g12-fault-soak-completion "$nexus_cross_soak_completion_path" \\\n'
        '      --scaling-evidence-manifest "$release_scaling_evidence_manifest"'
        in release_source
    )
    for expected_flag in (
        "--expected-scaling-trial-harness-sha256",
        "--expected-scaling-configuration-sha256",
        "--expected-scaling-irohad-sha256",
        "--expected-scaling-iroha-cli-sha256",
    ):
        assert expected_flag in release_source

    g4p_fidelity_root = tmp_path / "g4p-validator-argument-source-fidelity"
    for relative in _release_inventory_fixture_paths(module, (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("ci/check_sumeragi_v2_multilane_release_inventory.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py"),
        Path("scripts/write_sumeragi_v2_release_receipt_corridor_log.py"),
        Path("scripts/write_sumeragi_v2_release_receipt_gate_evidence.py"),
        Path("scripts/write_sumeragi_v2_release_receipt_publication.py"),
        Path("scripts/bootstrap_sumeragi_v2_release.py"),
        Path("scripts/bootstrap_sumeragi_v2_release_receipt_replay.py"),
        Path("scripts/validate_sumeragi_v2_release_bootstrap.py"),
        Path("formal/sumeragi_v2/README.md"),
        Path("formal/sumeragi_v2/PROOF.md"),
        Path("specs/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
    )):
        destination = g4p_fidelity_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    g4p_runner_path = (
        g4p_fidelity_root / "scripts" / "run_sumeragi_v2_release_gates.sh"
    )
    canonical_g4p_runner = g4p_runner_path.read_text(encoding="utf-8")
    g4p_argument = (
        '      --g4p-completion "$multilane_four_peer_completion_path" \\\n'
    )
    assert canonical_g4p_runner.count(g4p_argument) == 1
    baseline_errors = module._production_liveness_release_inventory_errors(
        g4p_fidelity_root
    )
    assert not any(
        "source-bound G-4P/G-12P/G-SCALE receipt corridor" in error
        for error in baseline_errors
    ), baseline_errors
    assert not any(
        "canonical G-UNIT leg/crate/test inventory SHA-256" in error
        or "canonical module/test inventory SHA-256" in error
        for error in baseline_errors
    ), baseline_errors
    for component_relative, expected_error in (
        (
            Path("scripts/write_sumeragi_v2_release_receipt_gate_evidence.py"),
            "release receipt component SHA-256 must equal",
        ),
        (
            Path("scripts/bootstrap_sumeragi_v2_release_receipt_replay.py"),
            "release bootstrap component SHA-256 must equal",
        ),
    ):
        component_path = g4p_fidelity_root / component_relative
        canonical_component = component_path.read_text(encoding="utf-8")
        component_path.write_text(
            canonical_component + "\n# source-binding mutation\n",
            encoding="utf-8",
        )
        component_errors = module._production_liveness_release_inventory_errors(
            g4p_fidelity_root
        )
        assert any(expected_error in error for error in component_errors), (
            expected_error,
            component_errors,
        )
        component_path.write_text(canonical_component, encoding="utf-8")

    for parent_relative, manifest_name, component_name, expected_error in (
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            "_RELEASE_RECEIPT_COMPONENT_FILES",
            "write_sumeragi_v2_release_receipt_publication.py",
            "release receipt component manifest must equal",
        ),
        (
            Path("scripts/bootstrap_sumeragi_v2_release.py"),
            "_BOOTSTRAP_COMPONENT_FILES",
            "bootstrap_sumeragi_v2_release_receipt_replay.py",
            "release bootstrap component manifest must equal",
        ),
    ):
        parent_path = g4p_fidelity_root / parent_relative
        canonical_parent = parent_path.read_text(encoding="utf-8")
        assert canonical_parent.count(f'    "{component_name}",') == 1
        parent_path.write_text(
            canonical_parent.replace(f'    "{component_name}",\n', "", 1),
            encoding="utf-8",
        )
        manifest_errors = module._production_liveness_release_inventory_errors(
            g4p_fidelity_root
        )
        assert any(expected_error in error for error in manifest_errors), (
            manifest_name,
            manifest_errors,
        )
        parent_path.write_text(canonical_parent, encoding="utf-8")
    g4p_runner_path.write_text(
        canonical_g4p_runner.replace(
            g4p_argument,
            '      --g4p-completion "$nexus_cross_completion_path" \\\n',
            1,
        ),
        encoding="utf-8",
    )
    mutated_errors = module._production_liveness_release_inventory_errors(
        g4p_fidelity_root
    )
    assert any(
        "source-bound G-4P/G-12P/G-SCALE receipt corridor" in error
        for error in mutated_errors
    ), mutated_errors
    g4p_runner_path.write_text(canonical_g4p_runner, encoding="utf-8")

    for reviewed_focus_test in (
        "sumeragi::v2_lane_work::tests::"
        "repeated_non_empty_retries_never_make_autonomous_routes_ordinary_eligible",
        "sumeragi::v2_runner::tests::"
        "deferred_autonomous_work_timeout_arms_only_a_non_empty_retry",
    ):
        assert canonical_g4p_runner.count(reviewed_focus_test) == 1
        g4p_runner_path.write_text(
            canonical_g4p_runner.replace(
                reviewed_focus_test,
                f"{reviewed_focus_test}_mutant",
                1,
            ),
            encoding="utf-8",
        )
        focus_mutation_errors = (
            module._production_liveness_release_inventory_errors(
                g4p_fidelity_root
            )
        )
        assert any(
            "canonical G-UNIT leg/crate/test inventory SHA-256" in error
            for error in focus_mutation_errors
        ), (reviewed_focus_test, focus_mutation_errors)
        g4p_runner_path.write_text(canonical_g4p_runner, encoding="utf-8")

    reviewed_persistence_budget_test = (
        "sumeragi::v2::tests::"
        "persistence_macro_step_budgets_have_exact_four_effect_maximum"
    )
    assert canonical_g4p_runner.count(reviewed_persistence_budget_test) == 1
    g4p_runner_path.write_text(
        canonical_g4p_runner.replace(
            reviewed_persistence_budget_test,
            reviewed_persistence_budget_test.replace("four", "five"),
            1,
        ),
        encoding="utf-8",
    )
    persistence_inventory_errors = (
        module._production_liveness_release_inventory_errors(
            g4p_fidelity_root
        )
    )
    assert any(
        "canonical module/test inventory SHA-256" in error
        for error in persistence_inventory_errors
    ), persistence_inventory_errors
    g4p_runner_path.write_text(canonical_g4p_runner, encoding="utf-8")

    reviewed_timeout_preemption_test = (
        "sumeragi::v2_core::tests::"
        "commit_qc_preempts_hung_timeout_signature_but_not_pending_wal"
    )
    assert canonical_g4p_runner.count(reviewed_timeout_preemption_test) == 1
    g4p_runner_path.write_text(
        canonical_g4p_runner.replace(
            reviewed_timeout_preemption_test,
            "sumeragi::v2_core::tests::"
            "commit_qc_cannot_overtake_timeout_frontier",
            1,
        ),
        encoding="utf-8",
    )
    timeout_preemption_inventory_errors = (
        module._production_liveness_release_inventory_errors(
            g4p_fidelity_root
        )
    )
    assert any(
        "canonical module/test inventory SHA-256" in error
        for error in timeout_preemption_inventory_errors
    ), timeout_preemption_inventory_errors
    g4p_runner_path.write_text(canonical_g4p_runner, encoding="utf-8")

    reviewed_inventory_name_mutations = (
        (
            "sumeragi::authoritative_runtime_gate_tests::"
            "sidecar_allocations_defer_historical_roster_proof_to_bounded_lane_owner",
            "sumeragi::authoritative_runtime_gate_tests::"
            "sidecar_allocations_require_roster_requester_before_lane_queue_admission",
        ),
        (
            "sumeragi::v2::tests::"
            "unowned_busy_prepare_certificate_rolls_back_staged_registry_and_active_subject",
            "sumeragi::v2::tests::"
            "unowned_busy_certificates_roll_back_staged_registry_and_active_subject",
        ),
        (
            "sumeragi::v2_block_sync::tests::"
            "historical_body_uses_self_contained_kura_finality_without_context_store",
            "sumeragi::v2_block_sync::tests::"
            "historical_body_comes_from_kura_and_a_non_signer_archive_can_serve",
        ),
        (
            "sumeragi::v2_effects::tests::"
            "exact_candidate_retry_coalesces_under_the_incumbent_owner",
            "sumeragi::v2_effects::tests::"
            "exact_candidate_retry_coalesces_and_owner_replacement_fails_closed",
        ),
        (
            "sumeragi::v2_lane_work::tests::"
            "lane_work_stays_quiescent_until_the_exact_global_prepare_lock",
            "sumeragi::v2_lane_work::tests::"
            "lane_work_stays_quiescent_until_the_exact_global_decision",
        ),
        (
            "sumeragi::v2_runtime::tests::"
            "exact_authenticated_timeout_certificate_coalesces_then_applies_through_signer",
            "sumeragi::v2_runtime::tests::"
            "exact_authenticated_timeout_certificate_from_distinct_sources_coalesces_in_one_runtime_slot",
        ),
        (
            "sumeragi::v2_runtime::tests::"
            "certified_tc_crosses_full_fence_blocked_prepare_prefix",
            "sumeragi::v2_runtime::tests::"
            "commit_certificate_response_coalesces_with_exact_busy_deferred_qc",
        ),
        (
            "sumeragi::v2_runtime::tests::"
            "absolute_timeout_preempts_serviceable_adapter_debt_then_debt_drains",
            "sumeragi::v2_runtime::tests::"
            "serviceable_adapter_debt_drains_one_macro_step_before_new_work",
        ),
        (
            "sumeragi::v2_runner::tests::"
            "drain_decided_lane_recovery_ingress_retains_current_serve_for_lifecycle",
            "sumeragi::v2_runner::tests::"
            "drain_decided_lane_recovery_ingress_retains_current_serve_for_lifecycle_mutant",
        ),
        (
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::completed_certified_serve_tombstone_replays_without_a_serve_carrier",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::completed_certified_serve_tombstone_replays_without_a_serve_carrier_mutant",
        ),
        (
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::fresh_certified_serve_publishes_exact_ledger_beside_fetch_and_broadcast",
            "sumeragi::v2_lifecycle_coordinator::ledger::tests::durable_ready_fetch_recovery::fresh_certified_serve_publishes_exact_ledger_beside_fetch_and_broadcast_mutant",
        ),
        (
            "network::inbound_source_memory_bound_tests::"
            "reliable_actor_waiter_geometry_rejects_source_overflow",
            "network::inbound_source_memory_bound_tests::"
            "reliable_actor_waiter_geometry_rejects_zero_and_combined_overflow",
        ),
        (
            "sumeragi::v2_lifecycle_coordinator::projection::tests::"
            "certified_serve_negative_settlement_requires_the_exact_post_fsync_receipt",
            "sumeragi::v2_lifecycle_coordinator::projection::tests::"
            "certified_serve_negative_settlement_requires_the_exact_post_fsync_receipt_mutant",
        ),
        (
            "sumeragi::v2_lifecycle_coordinator::projection::tests::"
            "certified_serve_rejects_a_receipt_for_another_signed_request",
            "sumeragi::v2_lifecycle_coordinator::projection::tests::"
            "certified_serve_rejects_a_receipt_for_another_signed_request_mutant",
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::"
            "fair_v2_ingress_certified_escape_survives_exact_same_source_saturation",
            "sumeragi::authoritative_runtime_gate_tests::"
            "fair_v2_ingress_certified_escape_survives_exact_same_source_saturation_omitted",
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::"
            "fair_v2_ingress_serializes_distinct_timeout_certificates_per_source",
            "sumeragi::authoritative_runtime_gate_tests::"
            "fair_v2_ingress_serializes_distinct_timeout_certificates_per_source_omitted",
        ),
    )
    for reviewed_name, mutation in reviewed_inventory_name_mutations:
        assert canonical_g4p_runner.count(reviewed_name) == 1, reviewed_name
        g4p_runner_path.write_text(
            canonical_g4p_runner.replace(reviewed_name, mutation, 1),
            encoding="utf-8",
        )
        reviewed_name_errors = (
            module._production_liveness_release_inventory_errors(
                g4p_fidelity_root
            )
        )
        assert any(
            "canonical module/test inventory SHA-256" in error
            for error in reviewed_name_errors
        ), (reviewed_name, reviewed_name_errors)
        g4p_runner_path.write_text(canonical_g4p_runner, encoding="utf-8")

    late_lane_recovery_path = (
        g4p_fidelity_root
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_lane_work.rs"
    )
    canonical_late_lane_recovery = late_lane_recovery_path.read_text(
        encoding="utf-8"
    )
    late_lane_recovery_items = module.rust_items(
        canonical_late_lane_recovery,
        "globally_applied_lane_body_without_certificate_remains_recoverable",
    )
    assert len(late_lane_recovery_items) == 1
    late_lane_recovery_item = late_lane_recovery_items[0]
    late_lane_recovery_item_start = canonical_late_lane_recovery.index(
        late_lane_recovery_item.source
    )
    late_lane_recovery_item_end = (
        late_lane_recovery_item_start + len(late_lane_recovery_item.source)
    )
    late_lane_baseline_errors = (
        module._production_liveness_release_inventory_errors(
            g4p_fidelity_root
        )
    )
    assert not any(
        "late canonical lane" in error
        for error in late_lane_baseline_errors
    ), late_lane_baseline_errors
    late_lane_recovery_mutations = (
        (
            "adapter.proposal_body_available(&proposal)",
            "!adapter.proposal_body_available(&proposal)",
            "late canonical lane recovery must distinguish global body application from lane-certificate durability",
        ),
        (
            "0,\n            \"no certificate exists yet to persist\"",
            "1,\n            \"no certificate exists yet to persist\"",
            "late canonical lane recovery must retain incomplete certificate progress in the active predecessor",
        ),
        (
            "retained_prepare_qc = lane_qc_for_phase("
            "&proposal, &keys[..3], CertPhase::Prepare);",
            "retained_prepare_qc = lane_qc_for_phase("
            "&proposal, &keys[..3], CertPhase::Commit);",
            "late canonical lane recovery must retain incomplete certificate progress in the active predecessor",
        ),
        (
            "inspect incomplete decided-lane authority\")\n                .is_none()",
            "inspect incomplete decided-lane authority\")\n                .is_some()",
            "late canonical lane recovery must retain incomplete certificate progress in the active predecessor",
        ),
        (
            ".schedule_retransmission()\n"
            "            .expect(\"schedule exact missing-certificate discovery\");",
            "let _ = &adapter;",
            "late canonical lane recovery must expose one bounded exact certificate-discovery source",
        ),
        (
            "message: BlockMessage::LaneBlockProposal(pending),",
            "message: BlockMessage::LaneBlockVote(pending),",
            "late canonical lane recovery must expose one bounded exact certificate-discovery source",
        ),
        (
            "V2LaneIngressOutcome::Inserted\n        );",
            "V2LaneIngressOutcome::Rejected\n        );",
            "late canonical lane recovery must release successor activation only after the exact certificate",
        ),
        (
            ".expect(\"persist recovered certificate and application receipt\"),\n"
            "            1\n        );",
            ".expect(\"persist recovered certificate and application receipt\"),\n"
            "            0\n        );",
            "late canonical lane recovery must release successor activation only after the exact certificate",
        ),
        (
            ".lane_block_application_receipt_available(&proposal)",
            ".lane_block_application_receipt_available(&proposal) && false",
            "late canonical lane recovery must release successor activation only after the exact certificate",
        ),
        (
            "build recovered decided-lane rollover authority\")\n                .is_some()",
            "build recovered decided-lane rollover authority\")\n                .is_none()",
            "late canonical lane recovery must release successor activation only after the exact certificate",
        ),
    )
    for reviewed_source, mutation, expected_error in late_lane_recovery_mutations:
        assert late_lane_recovery_item.source.count(reviewed_source) == 1
        late_lane_recovery_path.write_text(
            canonical_late_lane_recovery[:late_lane_recovery_item_start]
            + late_lane_recovery_item.source.replace(
                reviewed_source, mutation, 1
            )
            + canonical_late_lane_recovery[late_lane_recovery_item_end:],
            encoding="utf-8",
        )
        late_lane_recovery_errors = (
            module._production_liveness_release_inventory_errors(
                g4p_fidelity_root
            )
        )
        assert any(
            expected_error in error for error in late_lane_recovery_errors
        ), (expected_error, late_lane_recovery_errors)
        late_lane_recovery_path.write_text(
            canonical_late_lane_recovery,
            encoding="utf-8",
        )

    scaling_environment = {
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST",
        "IROHA_RELEASE_SCALING_IROHAD_SHA256",
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
    }
    for environment_contract, assignment_name in (
        (
            ROOT_DIR / "scripts" / "bootstrap_sumeragi_v2_release.py",
            "_RUNNER_ENV_ALLOWLIST",
        ),
        (
            ROOT_DIR / "scripts" / "validate_sumeragi_v2_release_bootstrap.py",
            "_RUNNER_EXTRA_ENV",
        ),
    ):
        tree = ast.parse(environment_contract.read_text(encoding="utf-8"))
        assignments = [
            statement
            for statement in tree.body
            if isinstance(statement, ast.Assign)
            and len(statement.targets) == 1
            and isinstance(statement.targets[0], ast.Name)
            and statement.targets[0].id == assignment_name
        ]
        assert len(assignments) == 1
        allowlist = ast.literal_eval(assignments[0].value)
        assert {
            value
            for value in allowlist
            if value.startswith("IROHA_RELEASE_SCALING_")
        } == scaling_environment
    assert "resolve_java.sh" in formal_launcher_source
    assert '"preflight-formal-launcher"' in receipt_source
    assert 'if [[ "$profile" == "--release" ]]; then' in release_source
    assert "  seed_count=32" in seed_source
    scenario_marker = "readonly scenarios=(\n"
    assert seed_source.count(scenario_marker) == 1
    scenario_inventory = seed_source.split(scenario_marker, 1)[1].split("\n)", 1)[0]
    scenario_count = sum(
        1 for line in scenario_inventory.splitlines() if line.strip()
    )
    assert scenario_count == 5
    assert 32 * scenario_count == 160
    assert (
        'readonly expected_runs="$((seed_count * ${#scenarios[@]}))"'
        in seed_source
    )
    assert "workspace sources changed during the PR release corridor" in release_source
    assert "workspace sources changed before the Taira production soak" in release_source
    assert "workspace sources changed during the production release corridor" in release_source
    soak_source = (
        ROOT_DIR / "scripts" / "run_taira_v2_24h_soak.sh"
    ).read_text(encoding="utf-8")
    assert "expected exactly one ignored Taira soak" in soak_source
    assert "check_taira_v2_soak_evidence.py" in soak_source
    pinned_taira_profile = {
        "IROHA_TAIRA_SIM_DURATION_SECS": "86400",
        "IROHA_TAIRA_SIM_SEED": "taira-public-sim",
        "IROHA_TAIRA_LOAD_TPS": "5",
        "IROHA_TAIRA_PACKET_LOSS_PERCENT": "10",
        "IROHA_TAIRA_CHURN_INTERVAL_SECS": "300",
        "IROHA_TAIRA_MAX_HEIGHT_SKEW": "2",
        "IROHA_TAIRA_MAX_HEIGHT_SKEW_GRACE_SECS": "30",
        "IROHA_TAIRA_MAX_TRANSIENT_HEIGHT_SKEW": "32",
        "IROHA_TAIRA_STALL_TIMEOUT_SECS": "300",
        "IROHA_TAIRA_MAX_VIEW_CHANGE_RATE": "0.2",
        "IROHA_TAIRA_MAX_LAGGED_CYCLE_RATIO": "0.35",
        "IROHA_TAIRA_MIN_COMMITTED_TPS_RATIO": "0.6",
        "IROHA_TAIRA_KEEP_LOCALNET": "1",
    }
    for name, value in pinned_taira_profile.items():
        assert f"export {name}={value}" in soak_source

    chaos_branch = harness_source.index("--chaos-100k)")
    chaos_inventory = harness_source.index("ignored_test_list=", chaos_branch)
    chaos_run = harness_source.index('"$ignored_test" \\\n', chaos_inventory)
    assert chaos_branch < chaos_inventory < chaos_run
    assert "expected exactly one ignored chaos test" in harness_source

    unit_branch = harness_source.index("--unit)")
    unit_inventory = harness_source.index("unit_test_list=", unit_branch)
    unit_ignored_inventory = harness_source.index(
        "unit_ignored_test_list=", unit_inventory
    )
    unit_run = harness_source.index(
        "--lib -- --test-threads=1", unit_ignored_inventory
    )
    assert unit_branch < unit_inventory < unit_ignored_inventory < unit_run
    assert "expected exactly 140 Sumeragi v2 reducer unit tests" in harness_source
    assert "reducer unit gate requires all 140 tests to be runnable" in harness_source

    replay_branch = harness_source.index("--model-replay)")
    replay_inventory = harness_source.index("model_replay_test_list=", replay_branch)
    replay_ignored_inventory = harness_source.index(
        "replay_ignored_test_list=", replay_inventory
    )
    replay_run = harness_source.index(
        "--test model_trace_replay -- --test-threads=1", replay_ignored_inventory
    )
    assert replay_branch < replay_inventory < replay_ignored_inventory < replay_run
    assert "expected exactly eight Sumeragi v2 model-replay tests" in harness_source
    assert "model-replay gate requires all eight tests to be runnable" in harness_source

    finalizer = taira_source.index("fn finalize_result")
    fail_closed = taira_source.index(
        "sandbox::enforce_network_start_requirement::<()>(None, context)?", finalizer
    )
    successful_skip = taira_source.index("return Ok(());", fail_closed)
    assert finalizer < fail_closed < successful_skip

def test_release_corridor_prebuilds_and_publishes_source_bound_binaries() -> None:
    release_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    ).read_text(encoding="utf-8")
    soak_source = (
        ROOT_DIR / "scripts" / "run_taira_v2_24h_soak.sh"
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

    for source in (release_source, soak_source):
        assert "unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN" in source
        assert "CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami" in source
        assert "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA" in source
        assert "TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO" in source
        assert "CARGO_BIN_EXE_iroha" in source
        assert "export IROHA_TEST_SKIP_BUILD=1" in source
        assert "export IROHA_TEST_ALLOW_REENTRANT_BUILD=0" in source
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
    assert (
        'readonly source_bound_root="${IROHA_RELEASE_ARTIFACT_ROOT}/'
        'sumeragi-v2-release/${source_manifest_sha256}"'
        in soak_source
    )
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
    canonical_declaration = "readonly canonical_production_test_count=864"
    count_guard = (
        "require_exact_token \\\n"
        '  "$release_runner" \\\n'
        '  "readonly expected_production_liveness_test_count='
        '${canonical_production_test_count}"'
    )
    assert checker_source.count(canonical_declaration) == 1
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
    canonical = "readonly expected_production_liveness_test_count=864"
    weakened = "readonly expected_production_liveness_test_count=859"
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
            "readonly canonical_production_test_count=859",
            "must seal exactly 864 production tests",
        ),
        (
            '    "sumeragi::v2_effects::tests": 70,',
            '    "sumeragi::v2_effects::tests": 69,',
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
            '    "sumeragi::v2_lane_work::tests": 63,',
            '    "sumeragi::v2_lane_work::tests": 62,',
            "changed-module counts must equal the exact reviewed release inventory",
        ),
        (
            '    "baf080f72349404d56b6a4eff0c6ad1b"',
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
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_trace.sh",
    ]
    sources = [path.read_text(encoding="utf-8") for path in scripts]

    assert all('1.7.4' in source for source in sources)
    assert all(
        "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
        in source
        for source in sources
    )
    # `-noGenerateSpecTE` was introduced after the immutable v1.7.4 release.
    # Keep both TLC entry points executable with the toolchain pinned above.
    assert all("-noGenerateSpecTE" not in source for source in sources[1:])

def test_tlc_entrypoints_use_the_pinned_tlapm_function_library() -> None:
    scripts = [
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_trace.sh",
    ]
    sources = [path.read_text(encoding="utf-8") for path in scripts]

    assert all(
        "3ab43c7ff31db4ced850619d4746fa4c841a7681" in source
        for source in sources
    )
    for expected_hash in (
        "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063",
        "aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da",
    ):
        assert all(expected_hash in source for source in sources)
    assert all("standard-library checksum mismatch" in source for source in sources)
    assert all('"-DTLA-Library=${tlapm_compat_dir}"' in source for source in sources)
    assert all(
        'ln -s "${TLAPM_STDLIB}/Functions.tla"' in source
        and 'ln -s "${TLAPM_STDLIB}/Folds.tla"' in source
        for source in sources
    )
    assert all('readonly TLC_MAX_SET_SIZE="1000000"' in source for source in sources)
    assert all('-maxSetSize "$TLC_MAX_SET_SIZE"' in source for source in sources)

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
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_trace.sh",
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
    assert worst_case_service_budget < maximum_timeout <= 2_147_483_647
    assert worst_case_service_budget <= maximum_view <= 2_147_483_647


def test_workspace_excluded_harness_pins_complete_unit_inventory() -> None:
    source = (
        ROOT_DIR / "scripts/formal/run_sumeragi_v2_harness.sh"
    ).read_text(encoding="utf-8")
    unit_branch = source.index("--unit)")
    unit_inventory = source.index("unit_test_list=", unit_branch)
    ignored_inventory = source.index("unit_ignored_test_list=", unit_inventory)
    unit_run = source.index("--lib -- --test-threads=1", ignored_inventory)

    assert unit_branch < unit_inventory < ignored_inventory < unit_run
    assert "if ((${#listed_unit_tests[@]} != 140)); then" in source
    assert "expected exactly 140 Sumeragi v2 reducer unit tests" in source
    assert "reducer unit gate requires all 140 tests to be runnable" in source


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
        "taira_divergent_views_converge_and_commit_within_one_rotation",
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
            Path("crates/iroha_sumeragi_core/tests/model_trace_replay.rs"),
            "if action == ModelAction::DeliverQc {",
            "if false {",
            None,
        ),
        (
            Path("scripts/formal/run_sumeragi_v2_harness.sh"),
            "if ((${#listed_unit_tests[@]} != 140)); then",
            "if false; then",
            None,
        ),
        (
            Path("crates/iroha_sumeragi_core/tests/model_trace_replay.rs"),
            "assert_eq!(steps.len(), 100);",
            "assert_eq!(steps.len(), 99);",
            "the exact 100-action assertion",
        ),
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
            'sumeragi_v2_tlc_assert_regular_log '
            '"replay-decision-witness" "$tlc_log"\n',
            "",
            "fresh regular witness-log assertion",
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
    assert "cancel-in-progress: false" in pull_request
    assert "cancel-in-progress: true" not in pull_request
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
            "mktemp -d /private/tmp/iroha-sumeragi-v2-formal.XXXXXX"
            in formal_job
        )
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
        assert formal_job.index("mktemp -d /private/tmp") < formal_job.index(
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
    assert "mktemp -d /private/tmp/iroha-sumeragi-v2-chaos.XXXXXX" in chaos_job
    for variable in (
        "CARGO_TARGET_DIR",
        "IROHA_RELEASE_ARTIFACT_ROOT",
        "IROHA_RELEASE_CANCEL_REQUEST_PATH",
        "SUMERAGI_V2_CHAOS_EVIDENCE_DIR",
    ):
        assert f"printf '{variable}=%s\\n'" in chaos_job
    assert chaos_job.index("mktemp -d /private/tmp") < chaos_job.index(
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
    assert opam_subcommands == ["list", "init", "switch", "var", "install", "install", "exec"]
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
    darwin_probes = ('clean_command sh -c \'command -v "$1" >/dev/null 2>&1\' sh pkg-config', "conf-*) darwin_conf_packages", "${#darwin_conf_packages[@]} -eq 3", "clean_command g++ -std=c++17 -Wall -Wextra -Werror -pedantic", "static_assert(__cplusplus == 201703L", "std::vector<int>", "std::accumulate(", 'clean_command "$DARWIN_CXX_PREFLIGHT"', "clean_command pkg-config --exists zlib", "strcmp(ZLIB_VERSION, zlibVersion())", "compress2(", "clean_command cc -std=c11 -Wall -Wextra -Werror -pedantic", '-x c - -lz -o "$DARWIN_ZLIB_PREFLIGHT"', 'clean_command "$DARWIN_ZLIB_PREFLIGHT"')
    assert all(fragment in builder for fragment in darwin_probes)
    assert re.findall(r'darwin_conf_packages\[[0-9]+\]}" == "([^"]+)"', builder) == ["conf-g++.1.0", "conf-pkg-config.5", "conf-zlib.1"]
    assumed_install = 'opam_command install --assume-depexts --switch "$OPAM_SWITCH" --yes "${darwin_conf_packages[@]}" < /dev/null'
    complete_install = 'opam_command install --switch "$OPAM_SWITCH" --yes "${build_packages[@]}" < /dev/null'
    assert normalized.count(assumed_install) == normalized.count(complete_install) == builder.count("--assume-depexts") == 1
    preflight_prefix = builder[:builder.index("\nverify_darwin_depext_capabilities\n")]
    darwin_helper_source = builder[builder.index("prepare_darwin_conf_boundary() {"):builder.index("\ncheckout_exact_tree() {")]
    assert re.search(r"(?m)^(?:(?:clean_command[ \t]+)?curl\b|(?:clean_command[ \t]+)?git\b[^\n]*\bfetch\b|(?:checkout_exact_tree|download_checked|opam_command)[ \t]+)", preflight_prefix) is None and re.search(r"\b(?:curl|checkout_exact_tree|download_checked|opam_command)\b|\bgit\b[^\n]*\bfetch\b", darwin_helper_source) is None and hashlib.sha256(darwin_helper_source.encode()).hexdigest() == "5674f471244306d795323a6b5497966a596b0dcebfaa368365238c1a93b6da77"
    depext_order = ('prepare_darwin_conf_boundary readonly -a darwin_conf_packages verify_darwin_depext_capabilities echo "[tlapm] fetching immutable source commit', 'if [[ "$PLATFORM" == "arm64-darwin" ]]; then echo "[tlapm] validating the exact Darwin host capability packages" ' + assumed_install, 'verify_package_set darwin-conf "$DARWIN_INTERMEDIATE_ATOMS"', complete_install, 'verify_package_set complete "$EXPECTED_ATOMS"')
    assert [normalized.index(fragment) for fragment in depext_order] == sorted(normalized.index(fragment) for fragment in depext_order)
    assert 'clean_command cp "$COMPILER_ATOMS" "$DARWIN_INTERMEDIATE_ATOMS"' in builder and 'printf \'%s\\n\' "${darwin_conf_packages[@]}" >> "$DARWIN_INTERMEDIATE_ATOMS"' in normalized
    assert all(forbidden not in builder for forbidden in ("OPAMASSUMEDEPEXTS", "OPAMDEPEXTS", "opam option depext=false")) and re.search(r"\b(?:brew|sudo)\b", builder) is None


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
