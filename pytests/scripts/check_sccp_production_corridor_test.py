"""Adversarial checks for the SCCP production-corridor attachment."""

from __future__ import annotations

import os
import re
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "scripts" / "check_sccp_production_corridor.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "sccp_production_corridor.yml"
PHASES = (
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "tvm-contract-smoke",
    "core-admission",
    "runtime-api",
)
AUTOMATIC_PHASES = tuple(phase for phase in PHASES if phase != "tvm-contract-smoke")
RETIRED_STEMS = (
    "source_bridge_evidence",
    "destination_evidence",
    "live_evidence",
    "source_state_evidence",
    "receipt_proof_evidence",
    "source_template_hashes",
    "groth16_material",
    "taira_xor_deploy",
    "client_loader",
)


def dry_run(phases: str, *, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    """Return one bounded dry-run trace."""

    return subprocess.run(
        ["bash", str(RUNNER), "--dry-run", "--phase", phases],
        cwd=ROOT,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def workflow_job(text: str, name: str) -> str:
    """Extract one top-level workflow job without parsing untrusted YAML tags."""

    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [a-z0-9-]+:\n|\Z)",
        text,
    )
    assert match is not None
    return match.group(0)


def test_runner_is_valid_bash_and_lists_exact_phase_set() -> None:
    subprocess.run(["bash", "-n", str(RUNNER)], cwd=ROOT, check=True)
    result = subprocess.run(
        ["bash", str(RUNNER), "--list"],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert tuple(line.strip() for line in result.stdout.splitlines()[1:]) == PHASES


def test_sccp_bls_verification_is_mandatory_in_every_crate_build() -> None:
    manifest_path = ROOT / "crates" / "iroha_sccp" / "Cargo.toml"
    manifest = manifest_path.read_text(encoding="utf-8")
    feature_block = manifest.split("[features]\n", 1)[1].split("\n[", 1)[0]
    assert re.search(r"(?m)^bls\s*=", feature_block) is None
    assert re.search(r"(?m)^default\s*=", feature_block) is None
    assert re.search(
        r'(?m)^iroha_crypto\s*=\s*\{[^\n]*features\s*=\s*\["node-crypto"\][^\n]*\}$',
        manifest,
    )
    crypto_manifest = (ROOT / "crates" / "iroha_crypto" / "Cargo.toml").read_text(
        encoding="utf-8"
    )
    assert re.search(
        r'(?m)^node-crypto\s*=\s*\["application",\s*"consensus"\]$',
        crypto_manifest,
    )
    assert re.search(r'(?m)^consensus\s*=\s*\[[^\n]*"bls"[^\n]*\]$', crypto_manifest)

    for relative in (
        "crates/iroha_sccp/src/bsc_native.rs",
        "crates/iroha_sccp/src/ethereum_source.rs",
        "crates/iroha_sccp/src/lib.rs",
    ):
        source = (ROOT / relative).read_text(encoding="utf-8")
        assert 'feature = "bls"' not in source
        assert "BlsUnavailable" not in source

    core_manifest = (ROOT / "crates" / "iroha_core" / "Cargo.toml").read_text(
        encoding="utf-8"
    )
    assert "iroha_sccp/bls" not in core_manifest


def test_every_production_release_path_requires_audited_pairing_valid_semantic_proofs() -> None:
    common = (ROOT / "scripts" / "sccp_release_common.py").read_text(encoding="utf-8")
    rust = (
        ROOT / "crates" / "iroha_sccp" / "src" / "bin" / "sccp_release_evidence.rs"
    ).read_text(encoding="utf-8")
    for required in (
        "verify_production_semantic_artifacts",
        "verify_rust_semantic_proofs",
        '"validate-semantic-proof"',
        "decode_canonical_sccp_groth16_bn254_proof_artifact_v1",
        "pairing_verified: true",
    ):
        assert required in common or required in rust
    for script in (
        "sccp_all_lanes_evidence.py",
        "sccp_release_bundle.py",
        "sccp_release_readiness_report.py",
        "sccp_verify_release_bundle.py",
    ):
        source = (ROOT / "scripts" / script).read_text(encoding="utf-8")
        assert "verify_production_semantic_artifacts(" in source
        assert "verify_rust_semantic_proofs(" in source
    assert "MAX_GROTH16_PROOF_ARTIFACT_BYTES" in common
    assert "public_signal_words_hex" in common
    assert "public_signal_words_hex" in rust


@pytest.mark.parametrize("bad", ("", "unknown", "rust-sccp,,js-sdk", "../evidence"))
def test_runner_rejects_noncanonical_phase_selection(bad: str) -> None:
    result = subprocess.run(
        ["bash", str(RUNNER), "--dry-run", "--phase", bad],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert result.returncode == 2
    assert len(result.stderr) < 4096


def test_evidence_phase_builds_and_uses_production_validator() -> None:
    trace = dry_run("evidence-scripts").stdout
    assert (
        "cargo build --locked -p iroha_sccp --features dev-tools "
        "--bin sccp_release_evidence"
    ) in trace
    assert "pytests/scripts/sccp_release_tooling_test.py" in trace
    assert "scripts/sccp_release_fixture.py" in trace
    assert "sccp_release_fixture.py reject" in trace
    assert " build --output-dir" not in trace
    assert " verify " not in trace
    assert "--features test-fixtures" not in trace


def test_rust_phase_tests_production_validator_without_fixture_feature() -> None:
    trace = dry_run("rust-sccp").stdout
    assert "cargo test --locked -p iroha_sccp -- --nocapture" in trace
    assert (
        "cargo test --locked -p iroha_sccp --features dev-tools "
        "--bin sccp_release_evidence -- --nocapture"
        in trace
    )
    assert "test-fixtures" not in trace


def test_core_phase_runs_exact_governance_and_four_peer_admission() -> None:
    trace = dry_run("core-admission").stdout
    for expected in (
        "cargo test --locked -p iroha_core --lib sccp_ -- --nocapture",
        "cargo test --locked -p iroha_core --test iroha_core_group_01 bridge_proofs:: -- --nocapture",
        "cargo test --locked -p iroha_core --lib parliament_due_certificate_ -- --nocapture",
        "cargo test --locked -p integration_tests --test network_functional",
        "sccp_route_governance::exact_sccp_route_governance_converges_and_rejects_adversarial_updates",
    ):
        assert expected in trace
    assert "sccp_route_manifest" not in trace


def test_runtime_api_phase_covers_durable_finality_router_cli_and_generated_spec() -> None:
    trace = dry_run("runtime-api").stdout
    for expected in (
        "cargo test --locked -p iroha_core --lib kura::tests::v2_finality -- --nocapture",
        "kura::tests::finalized_top_block_rejects_replacement_without_mutation",
        "kura::tests::pruning_across_durable_v2_finality_is_atomic_and_rejected",
        "kura::tests::startup_corruption_recovery_cannot_prune_finalized_block_bytes",
        "kura::tests::finalized_remote_only_block_retains_header_across_restart",
        "kura::tests::v2_finality_durably_archives_sccp_before_body_eviction_and_restart",
        "kura::tests::retained_sccp_archive_rejects_gap_omission_swap_overflow_and_rootless_extra",
        "cargo test --locked -p iroha_core --test bridge_finality_proof -- --nocapture",
        "cargo test --locked -p iroha_core --lib sumeragi::v2_apply::tests:: -- --nocapture",
        "cargo test --locked -p iroha_core --lib sumeragi::v2_effects::tests:: -- --nocapture",
        "cargo test --locked -p iroha_torii --lib sccp_ -- --nocapture",
        "cargo test --locked -p iroha_torii --lib bridge_finality_ -- --nocapture",
        "generated_openapi_has_only_resolvable_component_schema_refs",
        "cargo test --locked -p iroha_torii --test bridge_finality_endpoint -- --nocapture",
        "cargo test --locked -p iroha_cli sccp_ -- --nocapture",
        "bash ci/check_openapi_spec.sh",
    ):
        assert expected in trace
    for bypass in ("--ignored", "--no-run", "test-fixtures", "|| true"):
        assert bypass not in trace


def test_sdk_phases_use_only_exact_first_release_v1_suites() -> None:
    trace = dry_run("js-sdk,swift-sdk,kotlin-sdk,java-android").stdout
    for expected in (
        "sccpExact.test.js",
        "scripts/build_norito_xcframework.sh",
        "--disable-automatic-resolution",
        "SccpV1Tests",
        "org.hyperledger.iroha.sdk.sccp.",
        "SccpClientExactTest",
        "org.hyperledger.iroha.android.sccp.SccpV1Tests",
        "org.hyperledger.iroha.android.client.SccpClientExactTests",
    ):
        assert expected in trace
    for retired in ("SolanaSccp", "TonSccp", "sccpSolana", "sccpEthereumMainnet"):
        assert retired not in trace


def test_swift_phase_always_builds_fresh_and_rejects_relative_cargo_target() -> None:
    trace = dry_run("swift-sdk").stdout
    assert "rustup target list --toolchain 1.93.1 --installed" in trace
    assert "scripts/build_norito_xcframework.sh" in trace
    assert "--disable-automatic-resolution" in trace

    override_environment = os.environ.copy()
    override_environment["MOBILE_SDK_RUSTUP_BINARY"] = "/absolute/test/rustup"
    override_trace = dry_run("swift-sdk", env=override_environment).stdout
    assert (
        "/absolute/test/rustup target list --toolchain 1.93.1 --installed"
        in override_trace
    )

    source = RUNNER.read_text(encoding="utf-8")
    swift_builder = source.split("ensure_swift_bridge_artifact() {", 1)[1].split(
        "\nresolve_java_home() {", 1
    )[0]
    for required in (
        'artifact_dir="${MOBILE_SDK_APPLE_ARTIFACT_DIR:-}"',
        'rustup_binary="${MOBILE_SDK_RUSTUP_BINARY:-rustup}"',
        '"$rustup_binary" target list --toolchain 1.93.1 --installed',
        '"${NORITO_BRIDGE_OUT_DIR:-}" != "$canonical_artifact"',
    ):
        assert required in swift_builder
    for retired in ("bridge_zip", "unzip", "target add", "return 0\n  fi\n\n  if [[ -f"):
        assert retired not in swift_builder

    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = "target/relative-is-forbidden"
    result = subprocess.run(
        ["bash", str(RUNNER), "--phase", "swift-sdk"],
        cwd=ROOT,
        env=environment,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert result.returncode != 0
    assert "requires CARGO_TARGET_DIR" in result.stderr


def test_contract_phase_contains_only_direct_contract_smoke() -> None:
    trace = dry_run("contract-smoke").stdout
    assert "sccp_taira_xor_contract.test.mjs" not in trace
    assert "node --test scripts/tests/contract_tvm_receipts_test.mjs" in trace
    assert "node --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js" in trace
    assert "bash scripts/sccp_evm_contract_smoke.sh" in trace
    assert "bash -n scripts/sccp_ton_contract_build.sh" in trace
    assert "scripts/tests/ton_sccp_builder_test.py" in trace
    assert "python3 scripts/ton_sccp_builder.py --help" in trace
    assert (
        "bash scripts/sccp_ton_contract_build.sh development-local "
        "--acton /usr/local/bin/acton"
    ) in trace
    for retired in RETIRED_STEMS:
        assert retired not in trace


def test_runtime_overrides_apply_only_to_script_runtimes() -> None:
    environment = os.environ.copy()
    environment["SCCP_CORRIDOR_NODE_BIN"] = "/opt/pinned/node"
    environment["SCCP_CORRIDOR_PYTHON_BIN"] = "/opt/pinned/python"
    trace = dry_run("evidence-scripts,contract-smoke", env=environment).stdout
    assert "/opt/pinned/python -m pytest" in trace
    assert "/opt/pinned/python scripts/sccp_release_fixture.py" in trace
    assert "/opt/pinned/node --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js" in trace


def test_dry_run_never_creates_fixture_bundle(tmp_path: Path) -> None:
    environment = os.environ.copy()
    environment["TMPDIR"] = str(tmp_path)
    trace = dry_run("evidence-scripts", env=environment).stdout
    assert "iroha-sccp-release-fixture" not in trace
    assert list(tmp_path.iterdir()) == []


def test_retired_operator_surface_is_physically_absent() -> None:
    allowed = {
        "sccp_all_lanes_evidence.py",
        "sccp_evm_contract_smoke.sh",
        "sccp_ton_contract_build.sh",
        "sccp_release_bundle.py",
        "sccp_release_common.py",
        "sccp_release_fixture.py",
        "sccp_phase_log_runner.py",
        "sccp_release_readiness_report.py",
        "sccp_validator_builder.py",
        "sccp_validator_builder_driver.py",
        "sccp_verify_release_bundle.py",
    }
    actual = {path.name for path in (ROOT / "scripts").glob("sccp*")}
    assert actual == allowed
    assert {path.name for path in (ROOT / "pytests" / "scripts").glob("sccp*_test.py")} == {
        "sccp_release_tooling_test.py",
    }


def test_production_attachments_never_enable_fixture_feature() -> None:
    paths = [RUNNER, WORKFLOW, *ROOT.glob("Makefile*")]
    for path in paths:
        assert "test-fixtures" not in path.read_text(encoding="utf-8")


def test_workflow_exposes_every_phase_and_strict_automatic_aggregate() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    for phase in PHASES:
        assert f"          - {phase}" in workflow
        job = workflow_job(workflow, phase)
        if phase == "tvm-contract-smoke":
            assert "needs: [runner-self-check, contract-smoke]" in job
            assert (
                "if: ${{ github.event_name == 'workflow_dispatch' && "
                "github.event.inputs.phase == 'tvm-contract-smoke' }}" in job
            )
            assert "TODO: Add this live lane to the automatic aggregate" in job
            assert "bash scripts/contract_tvm_runner.sh" in job
            assert "tronbox/tre@sha256:" in job
        else:
            assert "needs: runner-self-check" in job
            assert f"--phase {phase}" in job
            if phase == "contract-smoke":
                assert "github.event.inputs.phase == 'tvm-contract-smoke'" in job
        assert "| tee" not in job
        assert "sccp_phase_log_runner.py" in job or (
            "--log-dir dist/sccp-production-corridor" in job
        )
        assert f"dist/sccp-production-corridor/{phase}.log" in job
        assert f"dist/sccp-production-corridor/{phase}.manifest.json" in job
        assert "if-no-files-found: error" in job
    aggregate = workflow_job(workflow, "sccp-production-corridor")
    assert f"needs: [runner-self-check, {', '.join(AUTOMATIC_PHASES)}]" in aggregate
    assert "tvm-contract-smoke" not in aggregate
    for state in ("failure", "cancelled", "skipped"):
        assert f"contains(needs.*.result, '{state}')" in aggregate
    assert "exit 1" in aggregate
    assert "| tee" not in workflow


def test_evidence_workflow_installs_rust_and_runs_real_corridor() -> None:
    job = workflow_job(WORKFLOW.read_text(encoding="utf-8"), "evidence-scripts")
    assert "actions/setup-python@" in job
    assert "actions-rust-lang/setup-rust-toolchain@" in job
    assert "Swatinem/rust-cache@" in job
    assert "bash scripts/check_sccp_production_corridor.sh --phase evidence-scripts" in job


def test_python_workflow_installs_hash_locked_sdk_dependencies() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    job = workflow_job(workflow, "python-sdk")
    for required in (
        'python-version: "3.12"',
        "cache-dependency-path: python/iroha_python/requirements-ci.lock",
        "--require-hashes",
        "--only-binary=:all:",
        "-r python/iroha_python/requirements-ci.lock",
    ):
        assert required in job
    assert '"python/iroha_python/requirements-ci.lock"' in workflow
    assert "pip install --upgrade pip pytest" not in job


def test_contract_workflow_installs_pinned_test_dependencies_and_acton_first() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    job = workflow_job(workflow, "contract-smoke")
    install = "python3 -m pip install -r scripts/requirements.txt"
    corridor = "bash scripts/check_sccp_production_corridor.sh --phase contract-smoke"
    acton_url = (
        "https://github.com/ton-blockchain/acton/releases/download/"
        "v${ACTON_VERSION}/acton-x86_64-unknown-linux-gnu.tar.gz"
    )
    for required in (
        "ACTON_VERSION: 1.1.0",
        "c2e640eacbb5b6ece1c343cab2ab6d2db74643d0706777aad181ed7e6e1bfc16",
        acton_url,
        "sha256sum --check --strict",
        "acton 1.1.0 (9cf4d1f 2026-05-22)",
    ):
        assert required in job
    assert install in job
    assert job.index(acton_url) < job.index(corridor)
    assert job.index(install) < job.index(corridor)
    assert '"scripts/requirements.txt"' in workflow


def test_swift_workflow_binds_the_exact_first_release_bridge_envelope() -> None:
    job = workflow_job(WORKFLOW.read_text(encoding="utf-8"), "swift-sdk")
    for required in (
        "actions/setup-python@",
        'python-version: "3.12"',
        "dtolnay/rust-toolchain@",
        "toolchain: 1.93.1",
        '"$rustup_path" target add --toolchain 1.93.1',
        "x86_64-apple-darwin",
        "cargo fetch --locked",
        "CARGO_BUILD_JOBS=1",
        "CARGO_INCREMENTAL=0",
        "CARGO_NET_OFFLINE=true",
        "CARGO_TARGET_DIR=$cargo_target",
        "MOBILE_SDK_PYTHON_BINARY=$mobile_python",
        "MOBILE_SDK_RUSTUP_BINARY=$rustup_path",
        'rustup_source="$(command -v rustup)"',
        'rustup_path="$rustup_tools/rustup"',
        '/bin/cp "$rustup_source" "$rustup_path"',
        '"${rustup_path##*/}" == rustup',
        "MOBILE_SDK_APPLE_ARTIFACT_DIR=$bridge_artifacts",
        "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1",
        "NORITO_BRIDGE_OUT_DIR=$bridge_artifacts",
        "NORITO_BRIDGE_BUILD_DIR=$bridge_build",
        "RUSTC=$rustc_path",
        "RUSTC_BOOTSTRAP=1",
        "RUSTDOC=$rustdoc_path",
        "$RUNNER_TEMP/iroha-sccp-apple-cargo",
        "$RUNNER_TEMP/iroha-sccp-apple-artifacts",
        "bash scripts/check_sccp_production_corridor.sh --phase swift-sdk",
    ):
        assert required in job
    assert "target/sccp-production-corridor" not in job


@pytest.mark.parametrize(
    "mutation",
    (
        lambda value: value.replace("contains(needs.*.result, 'skipped')", "false", 1),
        lambda value: value.replace("if-no-files-found: error", "if-no-files-found: ignore", 1),
        lambda value: value.replace("needs: runner-self-check", "needs: []", 1),
    ),
)
def test_workflow_guard_detects_weakened_attachment(mutation) -> None:
    original = WORKFLOW.read_text(encoding="utf-8")
    changed = mutation(original)
    assert changed != original
    aggregate = workflow_job(changed, "sccp-production-corridor")
    strict_aggregate = all(
        f"contains(needs.*.result, '{state}')" in aggregate
        for state in ("failure", "cancelled", "skipped")
    )
    strict_uploads = all(
        "if-no-files-found: error" in workflow_job(changed, phase) for phase in PHASES
    )
    strict_dependencies = all(
        (
            "needs: [runner-self-check, contract-smoke]" in workflow_job(changed, phase)
            if phase == "tvm-contract-smoke"
            else "needs: runner-self-check" in workflow_job(changed, phase)
        )
        for phase in PHASES
    )
    assert not (strict_aggregate and strict_uploads and strict_dependencies)


def test_workflow_path_filters_cover_release_trust_and_fixture_inputs() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    for path in (
        '"crates/iroha_sccp/**"',
        '"crates/iroha_data_model/**"',
        '"crates/iroha_js_host/**"',
        '"crates/connect_norito_bridge/**"',
        '"contracts/bsc/sccp/**"',
        '"contracts/ethereum/sccp/**"',
        '"fixtures/sccp/**"',
        '"integration_tests/**"',
        '"artifacts/openapi/**"',
        '"scripts/sccp_*"',
        '"scripts/build_norito_xcframework.sh"',
        '"scripts/archive_norito_xcframework.py"',
        '"scripts/check_mobile_sdk_artifacts.sh"',
        '"scripts/check_mobile_sdk_artifact_pin_commit.py"',
        '"scripts/exec_with_file_lock.py"',
        '"scripts/norito_bridge_apple_slice_handoff.py"',
        '"scripts/norito_bridge_source_seal.py"',
        '"scripts/run_mobile_hermetic_command.py"',
        '"scripts/update_norito_bridge_swift_pins.py"',
        '"scripts/validate_norito_bridge_xcframework.py"',
        '"scripts/ci/run_xcframework_smoke.sh"',
        '"scripts/contract_tooling/**"',
        '"scripts/tests/contract_artifact_corridor_test.py"',
        '"scripts/tests/contract_tvm_receipts_test.mjs"',
        '"pytests/scripts/sccp_*"',
    ):
        assert path in workflow
