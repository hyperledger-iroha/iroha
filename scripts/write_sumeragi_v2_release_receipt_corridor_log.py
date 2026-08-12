# Executed lexically in write_sumeragi_v2_release_receipt.py; do not import directly.

_WIRE_RELEASE_INVARIANT_PYTEST_NODES = (
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_binds_current_semantic_sources "
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_rejects_ledger_weakening "
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_rejects_semantic_source_mutation"
)


def _sdk_suite_source_manifest(repo_root: Path, suite: str) -> str:
    """Resolve one reviewed SDK suite through its source-bound closure tool."""

    if suite not in _SDK_SOURCE_CLOSURE_SUITES:
        raise ReceiptError(f"unknown SDK source-closure suite {suite!r}")
    resolver = repo_root / _SDK_SOURCE_CLOSURE_RESOLVER
    manifest = repo_root / _SDK_SOURCE_CLOSURE_MANIFEST
    manifest_snapshot = _bounded_evidence_snapshot(
        manifest,
        "SDK source-closure manifest",
        maximum_bytes=_MAX_HELPER_BYTES,
        allowed_owners={os.geteuid()},
        require_single_link=True,
    )
    environment = _closed_replay_environment(repo_root)
    environment.update(
        {
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONHASHSEED": "0",
        }
    )
    status, stdout, stderr = _run_bounded_python_validator(
        resolver,
        [
            "--root",
            str(repo_root),
            "--manifest",
            str(manifest),
            "--suite",
            suite,
            "--manifest-sha256",
        ],
        cwd=repo_root,
        environment=environment,
        name=f"SDK source-closure resolver for {suite}",
        maximum_output_bytes=4096,
        watched_contracts=(manifest_snapshot,),
    )
    if status != 0:
        diagnostic = stderr.decode("utf-8", errors="replace").strip()
        suffix = f": {diagnostic}" if diagnostic else ""
        raise ReceiptError(
            f"SDK source-closure resolver rejected {suite!r}{suffix}"
        )
    if stderr:
        raise ReceiptError(
            f"SDK source-closure resolver for {suite!r} wrote to stderr"
        )
    if re.fullmatch(rb"[0-9a-f]{64}\n", stdout) is None:
        raise ReceiptError(
            f"SDK source-closure resolver for {suite!r} emitted a "
            "noncanonical digest"
        )
    return stdout[:-1].decode("ascii")


def _test_count_from_log(lines: list[str], kind: str, name: str) -> int:
    if kind == "cargo-focus":
        running = [line for line in lines if line == "running 1 test"]
        results = [
            line
            for line in lines
            if re.fullmatch(
                r"test result: ok\. 1 passed; 0 failed; 0 ignored; "
                r"0 measured; [0-9]+ filtered out; finished in .+",
                line,
            )
            is not None
        ]
        if not running or len(running) != len(results):
            raise ReceiptError(
                f"{name} has an ambiguous Cargo transcript for focused tests"
            )
        return len(results)
    if kind.startswith("cargo-"):
        running = [
            match
            for line in lines
            if (match := re.fullmatch(r"running ([0-9]+) tests?", line))
        ]
        results = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; "
                    r"0 measured; [0-9]+ filtered out; finished in .+",
                    line,
                )
            )
        ]
        if (
            len(running) != 1
            or len(results) != 1
            or running[0].group(1) != results[0].group(1)
        ):
            raise ReceiptError(f"{name} has an ambiguous Cargo transcript")
        return int(results[0].group(1))
    if kind == "pytest":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s", line
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(f"{name} has an ambiguous pytest transcript")
        return int(matches[0].group(1))
    if kind == "node":
        matches = [
            match
            for line in lines
            if (match := re.fullmatch(r"# pass ([0-9]+)", line))
        ]
        if (
            len(matches) != 1
            or lines.count(f"# tests {matches[0].group(1)}") != 1
            or lines.count("# fail 0") != 1
            or lines.count("# cancelled 0") != 1
            or lines.count("# skipped 0") != 1
            or lines.count("# todo 0") != 1
        ):
            raise ReceiptError(f"{name} has an ambiguous Node transcript")
        return int(matches[0].group(1))
    if kind == "native-amx-sdk":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"native-amx-v2-grouped-parity surface=[a-z]+ "
                    r"tests=([0-9]+) fixture_sha256=[0-9a-f]{64} "
                    r"suite_source_manifest_sha256=[0-9a-f]{64}",
                    line,
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(
                f"{name} has an ambiguous grouped Native AMX V2 SDK transcript"
            )
        return int(matches[0].group(1))
    if kind == "sdk-diagnostics":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"sumeragi-v2-sdk-diagnostics surface=[a-z]+ "
                    r"tests=([0-9]+) "
                    r"suite_source_manifest_sha256=[0-9a-f]{64}",
                    line,
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(
                f"{name} has an ambiguous Sumeragi v2 SDK diagnostics transcript"
            )
        return int(matches[0].group(1))
    if kind == "command":
        return 0
    raise ReceiptError(f"{name} has unknown leg kind {kind}")


def _prebuilt_artifact_root(
    repo_root: Path, artifact_root: Path, label: str = "release artifact root"
) -> Path:
    """Authenticate one private external root used by a prebuilt bundle."""

    if artifact_root != Path(os.path.abspath(artifact_root)):
        raise ReceiptError(f"{label} must be absolute and normalized")
    try:
        resolved = artifact_root.resolve(strict=True)
        metadata = resolved.lstat()
    except (OSError, RuntimeError) as error:
        raise ReceiptError(f"{label} is unavailable") from error
    source_root = repo_root.resolve(strict=True)
    try:
        roots_overlap = (
            Path(os.path.commonpath((resolved, source_root))) in {resolved, source_root}
        )
    except ValueError:
        roots_overlap = True
    if (
        resolved != artifact_root
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077
        or roots_overlap
    ):
        raise ReceiptError(
            f"{label} must be one private owner-owned directory "
            "outside the sealed source"
        )
    return resolved


def _prebuilt_release_roots(
    *,
    repo_root: Path,
    fields: dict[str, str],
    expected_artifact_root: Path,
    expected_cargo_target_root: Path,
) -> tuple[Path, Path]:
    """Bind the corridor root fields to the authenticated bootstrap layout."""

    artifact_root = _prebuilt_artifact_root(
        repo_root, expected_artifact_root
    )
    cargo_target_root = _prebuilt_artifact_root(
        repo_root,
        expected_cargo_target_root,
        "release Cargo target root",
    )
    if fields["artifact_root_path"] != str(artifact_root):
        raise ReceiptError(
            "corridor artifact root is not the exact authenticated release "
            "artifact root"
        )
    if fields["cargo_target_root_path"] != str(cargo_target_root):
        raise ReceiptError(
            "corridor Cargo target root is not the exact authenticated "
            "release Cargo target root"
        )
    try:
        roots_overlap = Path(
            os.path.commonpath((artifact_root, cargo_target_root))
        ) in {artifact_root, cargo_target_root}
    except ValueError:
        roots_overlap = True
    if roots_overlap:
        raise ReceiptError("release artifact and Cargo target roots overlap")
    return artifact_root, cargo_target_root


def _prebuilt_directory(path: Path, name: str) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o500
        or metadata.st_uid != os.geteuid()
    ):
        raise ReceiptError(
            f"{name} must be an owner-owned resolved non-symlink directory "
            "with exact mode 0500"
        )
    return path


def _corridor_legs() -> list[tuple[str, str, int, str]]:
    legs = [
        (
            leg_id,
            "cargo-focus",
            count,
            _g_unit_leg_command(array_name, package, cargo_target),
        )
        for array_name, leg_id, package, count, cargo_target in _G_UNIT_GROUPS
    ]
    legs.extend(
        (
            (
                leg_id,
                "cargo-module",
                count,
                _production_module_command(module),
            )
            for leg_id, module, count in _PRODUCTION_MODULES
        )
    )
    legs.append(
        (
            "status-rust",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p iroha_data_model --lib "
            f"{_DATA_STATUS_TEST} -- --test-threads=1",
        )
    )
    legs.append(
        (
            "lane-certificate-rust",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p iroha_data_model --lib "
            f"{_DATA_LANE_CERTIFICATE_TEST} -- --exact --test-threads=1",
        )
    )
    legs.extend(
        (
            (
                "source-sealed-workspace-build",
                "command",
                0,
                "cargo +1.93.1 build -j1 --locked --offline --workspace",
            ),
            (
                "source-sealed-workspace-tests",
                "command",
                0,
                "cargo +1.93.1 test -j1 --locked --offline --workspace",
            ),
            (
                "source-sealed-irohad-tests",
                "command",
                0,
                "cargo +1.93.1 test -j1 --locked --offline -p irohad "
                "--bin irohad --features test-network-message-control",
            ),
            (
                "source-sealed-workspace-clippy",
                "command",
                0,
                "cargo +1.93.1 clippy -j1 --locked --offline --workspace "
                "--all-targets -- -D warnings",
            ),
            (
                "source-sealed-workspace-format",
                "command",
                0,
                "cargo +1.93.1 fmt --all -- --check",
            ),
            (
                "source-sealed-legacy-codec-guard",
                "command",
                0,
                "bash scripts/check_no_legacy_codec.sh",
            ),
        )
    )
    legs.extend(
        (
            f"taira-contract-{index}",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p integration_tests "
            "--test consensus_and_da "
            f"{test} -- --exact --test-threads=1",
        )
        for index, test in enumerate(_TAIRA_CONTRACT_TESTS)
    )
    legs.append(
        (
            "cross-sdk-rust",
            "cargo-exact",
            2,
            "cargo test --locked --offline -p iroha_data_model --test "
            "iroha_data_model_group_02 sumeragi_v2_cross_sdk_fixtures:: "
            "-- --test-threads=1",
        )
    )
    legs.append(
        (
            "native-amx-rust-fixture-check",
            "command",
            0,
            "cargo run --locked --offline -p iroha_data_model --bin "
            "sumeragi_v2_wire_fixtures -- --check",
        )
    )
    legs.extend(
        (
            f"native-amx-grouped-{surface}",
            "native-amx-sdk",
            count,
            f"bash {_NATIVE_AMX_GROUPED_PARITY_HARNESS} {surface}",
        )
        for surface, count in _NATIVE_AMX_GROUPED_PARITY_SUITES
    )
    legs.append(
        (
            "sumeragi-diagnostics-rust",
            "cargo-exact",
            len(_RUST_SDK_DIAGNOSTICS_TESTS),
            "cargo test --locked --offline -p iroha --lib "
            "client::tests::get_sumeragi_ -- --test-threads=1",
        )
    )
    legs.extend(
        (
            f"sumeragi-diagnostics-{surface}",
            "sdk-diagnostics",
            count,
            f"bash {_SUMERAGI_SDK_DIAGNOSTICS_HARNESS} {surface}",
        )
        for surface, count in _SUMERAGI_SDK_DIAGNOSTICS_SUITES
    )
    legs.extend(
        (
            (
                "preflight-source-seal",
                "pytest",
                30,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/workspace_source_manifest_test.py "
                "pytests/scripts/seal_workspace_source_test.py",
            ),
            (
                "preflight-seed-launcher",
                "pytest",
                14,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_runs_every_exact_scenario_with_one_start_attempt "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_preserves_prior_invocation_evidence "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_release_profile_uses_32_seeds_per_scenario "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_zero_test_and_preserves_evidence "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_ambiguous_test_summary "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_preserves_cargo_failure_through_tee "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_bundle_tampering_before_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_symlinked_marker_temp_without_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_marker_durability_failure_is_not_terminal "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_parent_source_manifest_mismatch "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_source_drift_before_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_concurrent_writer_without_clobbering "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_refuses_uninspected_stale_lock "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_unsafe_retained_localnet_entries",
            ),
            (
                "preflight-chaos-launcher",
                "pytest",
                5,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_chaos_release_test.py",
            ),
            (
                "preflight-release-identity",
                "pytest",
                68,
                "SUMERAGI_V2_TEST_RELOCATABLE_SSH_KEYGEN_BIN="
                "$IROHA_RELEASE_SSH_KEYGEN_BIN PYTHONDONTWRITEBYTECODE=1 "
                "PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_identity_signature_test.py",
            ),
            (
                "preflight-release-bootstrap",
                "pytest",
                257,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_bootstrap_test.py "
                "pytests/scripts/sumeragi_v2_release_bootstrap_cancellation_test.py",
            ),
            (
                "preflight-release-bootstrap-validator",
                "pytest",
                37,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_bootstrap_validator_test.py",
            ),
            (
                "preflight-release-receipt",
                "pytest",
                363,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_receipt_test.py "
                "pytests/scripts/sumeragi_v2_release_receipt_components_test.py "
                "pytests/scripts/sumeragi_v2_prebuilt_bundle_test.py "
                "pytests/scripts/sumeragi_v2_prebuilt_bundle_shell_test.py "
                "pytests/scripts/sumeragi_v2_release_process_policy_test.py",
            ),
            (
                "preflight-multilane-scaling",
                "pytest",
                52,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "scripts/tests/validate_multilane_scaling_evidence_test.py "
                "scripts/tests/run_multilane_scaling_gate_test.py",
            ),
            (
                "preflight-proof-fidelity",
                "pytest",
                5291,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_proof_ledger_test.py "
                "pytests/scripts/sumeragi_v2_verus_evidence_test.py "
                "pytests/scripts/sumeragi_v2_tlc_trace_normalizer_test.py "
                "pytests/scripts/sumeragi_v2_reviewed_rust_source_test.py "
                "pytests/scripts/sumeragi_v2_multilane_native_merge_manifest_test.py "
                "pytests/scripts/sumeragi_v2_multilane_passive_recovery_contract_test.py "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_legacy_layout_only_claim "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_state_order_weakening "
                "pytests/scripts/sumeragi_v2_multilane_models_terminal_tail_test.py::"
                "test_inflight_composed_contract_rejects_snapshot_nonstutter_mapping "
                "pytests/scripts/sumeragi_v2_multilane_models_terminal_tail_test.py::"
                "test_inflight_composed_contract_rejects_missing_direct_release_action "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_layout_contract_rejects_action_inventory_weakening "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_per_key_prefix_skip_weakening "
                "pytests/scripts/sumeragi_v2_multilane_models_tail_test.py::"
                "test_inflight_composed_contract_rejects_tla_snapshot_nonstutter_mapping "
                "pytests/scripts/sumeragi_v2_multilane_models_tail_test.py::"
                "test_inflight_composed_contract_rejects_verus_snapshot_stutter_proof_removal "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_layout_contract_rejects_membership_only_lane_authorship "
                + _WIRE_RELEASE_INVARIANT_PYTEST_NODES,
            ),
            (
                "preflight-formal-launcher",
                "pytest",
                26,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_formal_release_test.py",
            ),
            (
                "preflight-taira-soak",
                "pytest",
                43,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_pins_complete_profile_and_runs_exactly_one_test "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_zero_test_inventory "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_zero_test_execution_output "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_bundle_tampering_before_completion "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_symlinked_marker_temp_without_completion "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_marker_durability_failure_is_not_terminal "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_profile_override_arguments_before_cargo "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_a_concurrent_source_bound_soak "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_does_not_promote_provisional_evidence_when_validation_fails "
                "pytests/scripts/taira_v2_soak_evidence_test.py",
            ),
        )
    )
    return legs
