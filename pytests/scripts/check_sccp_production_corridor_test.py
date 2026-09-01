"""Adversarial checks for the SCCP production-corridor attachment."""

from __future__ import annotations

import os
import re
import shutil
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
APPLE_SLICES = (
    "ios-arm64",
    "ios-sim-arm64",
    "ios-sim-x64",
    "macos-arm64",
    "macos-x64",
)
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


def has_background_command(text: str) -> bool:
    """Return whether a shell body contains a standalone background operator."""

    return re.search(r"(?<![>&])&(?![>&])", text) is not None


def has_native_compile(text: str) -> bool:
    """Return whether an Apple assembler job directly invokes a compiler."""

    return (
        re.search(
            r"(?m)(?:^|[ \t])cargo[ \t]+(?:build|rustc)(?:[ \t]|$)", text
        )
        is not None
        or re.search(r"(?m)(?:^|[ \t])rustc(?:[ \t]|$)", text) is not None
    )


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
    assert "pytests/scripts/sccp_release_fixture_reseal_test.py" in trace
    assert "scripts/sccp_release_fixture.py" in trace
    assert " validate" in trace
    assert " build --output-dir" in trace
    assert " verify " in trace
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
        "cargo test --locked -p iroha_core --test sccp_route_governance_isi -- --nocapture",
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

    source = RUNNER.read_text(encoding="utf-8")
    swift_builder = source.split("ensure_swift_bridge_artifact() {", 1)[1].split(
        "\nresolve_java_home() {", 1
    )[0]
    for retired in ("bridge_zip", "unzip", "target add", "return 0\n  fi\n\n  if [[ -f"):
        assert retired not in swift_builder

    assembled_environment = os.environ.copy()
    assembled_environment.update(
        {
            "MOBILE_SDK_APPLE_ARTIFACT_DIR": "/authenticated/apple-artifacts",
            "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT": "1",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR": "/authenticated/swift-scratch",
            "SCCP_SWIFT_BRIDGE_MODE": "assembled",
        }
    )
    assembled_trace = dry_run("swift-sdk", env=assembled_environment).stdout
    assert "scripts/check_mobile_sdk_artifacts.sh" in assembled_trace
    assert "--apple-only" in assembled_trace
    assert "scripts/build_norito_xcframework.sh" not in assembled_trace
    assert "--scratch-path /authenticated/swift-scratch" in assembled_trace

    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = "target/relative-is-forbidden"
    environment["SCCP_SWIFT_BRIDGE_MODE"] = "source-build"
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


def test_assembled_swift_mode_rejects_untrusted_artifact_roots(
    tmp_path: Path,
) -> None:
    real_artifact = tmp_path / "real-artifact"
    real_artifact.mkdir()
    symlink_artifact = tmp_path / "symlink-artifact"
    symlink_artifact.symlink_to(real_artifact, target_is_directory=True)
    incomplete_artifact = tmp_path / "incomplete-artifact"
    incomplete_artifact.mkdir()
    cases = (
        ({"MOBILE_SDK_APPLE_ARTIFACT_DIR": str(incomplete_artifact)}, "requires MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1"),
        (
            {
                "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT": "1",
                "MOBILE_SDK_APPLE_ARTIFACT_DIR": "relative-artifact",
            },
            "requires a canonical external MOBILE_SDK_APPLE_ARTIFACT_DIR",
        ),
        (
            {
                "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT": "1",
                "MOBILE_SDK_APPLE_ARTIFACT_DIR": str(symlink_artifact),
            },
            "requires a canonical external MOBILE_SDK_APPLE_ARTIFACT_DIR",
        ),
        (
            {
                "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT": "1",
                "MOBILE_SDK_APPLE_ARTIFACT_DIR": str(ROOT / "scripts"),
            },
            "must be outside the Iroha source tree",
        ),
        (
            {
                "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT": "1",
                "MOBILE_SDK_APPLE_ARTIFACT_DIR": str(incomplete_artifact),
            },
            "assembled NoritoBridge.xcframework is incomplete",
        ),
    )
    for overrides, expected in cases:
        environment = os.environ.copy()
        environment.update(overrides)
        environment["SCCP_SWIFT_BRIDGE_MODE"] = "assembled"
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
        assert expected in result.stderr

    invalid_environment = os.environ.copy()
    invalid_environment["SCCP_SWIFT_BRIDGE_MODE"] = "invalid"
    invalid = subprocess.run(
        ["bash", str(RUNNER), "--phase", "swift-sdk"],
        cwd=ROOT,
        env=invalid_environment,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert invalid.returncode != 0
    assert "must be exactly source-build or assembled" in invalid.stderr


@pytest.mark.parametrize("scratch_kind", ("relative", "symlink", "in-tree"))
def test_assembled_swift_mode_rejects_untrusted_scratch_roots(
    tmp_path: Path,
    scratch_kind: str,
) -> None:
    fixture_root = tmp_path / "fixture-repo"
    fixture_scripts = fixture_root / "scripts"
    fixture_scripts.mkdir(parents=True)
    (fixture_root / "IrohaSwift").mkdir()
    (fixture_root / "IrohaSwift" / "Package.resolved").write_text(
        "{}\n", encoding="utf-8"
    )
    fixture_runner = fixture_scripts / RUNNER.name
    shutil.copy2(RUNNER, fixture_runner)
    (fixture_scripts / "check_mobile_sdk_artifacts.sh").write_text(
        "#!/usr/bin/env bash\nexit 0\n", encoding="utf-8"
    )

    artifact_root = tmp_path / "authenticated-artifact"
    bridge = artifact_root / "NoritoBridge.xcframework"
    bridge.mkdir(parents=True)
    (bridge / "Info.plist").write_text("fixture\n", encoding="utf-8")
    (bridge / "NoritoBridge.artifacts.json").write_text(
        "{}\n", encoding="utf-8"
    )
    real_scratch = tmp_path / "real-scratch"
    real_scratch.mkdir()
    if scratch_kind == "relative":
        scratch = "relative-scratch"
        expected = "requires an existing writable external SwiftPM scratch directory"
    elif scratch_kind == "symlink":
        scratch_link = tmp_path / "scratch-link"
        scratch_link.symlink_to(real_scratch, target_is_directory=True)
        scratch = str(scratch_link)
        expected = "requires an existing writable external SwiftPM scratch directory"
    else:
        scratch = str(fixture_scripts)
        expected = "scratch directory must be outside the Iroha source tree"

    environment = os.environ.copy()
    environment.update(
        {
            "MOBILE_SDK_APPLE_ARTIFACT_DIR": str(artifact_root),
            "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT": "1",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR": scratch,
            "SCCP_SWIFT_BRIDGE_MODE": "assembled",
        }
    )
    result = subprocess.run(
        ["bash", str(fixture_runner), "--phase", "swift-sdk"],
        cwd=fixture_root,
        env=environment,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert result.returncode != 0
    assert expected in result.stderr


def test_contract_phase_contains_only_direct_contract_smoke() -> None:
    trace = dry_run("contract-smoke").stdout
    assert "sccp_taira_xor_contract.test.mjs" not in trace
    assert "node --test scripts/tests/contract_tvm_receipts_test.mjs" in trace
    assert "node --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js" in trace
    assert "bash scripts/sccp_evm_contract_smoke.sh" in trace
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
    assert "iroha-sccp-release-fixture.dry-run/bundle" in trace
    assert list(tmp_path.iterdir()) == []


def test_retired_operator_surface_is_physically_absent() -> None:
    allowed = {
        "sccp_all_lanes_evidence.py",
        "sccp_evm_contract_smoke.sh",
        "sccp_release_bundle.py",
        "sccp_release_common.py",
        "sccp_release_fixture.py",
        "sccp_release_fixture_reseal.py",
        "sccp_release_readiness_report.py",
        "sccp_verify_release_bundle.py",
    }
    actual = {path.name for path in (ROOT / "scripts").glob("sccp*")}
    assert actual == allowed
    assert {path.name for path in (ROOT / "pytests" / "scripts").glob("sccp*_test.py")} == {
        "sccp_release_fixture_reseal_test.py",
        "sccp_release_tooling_test.py",
    }


def test_production_attachments_never_enable_fixture_feature() -> None:
    paths = [RUNNER, WORKFLOW, *ROOT.glob("Makefile*")]
    for path in paths:
        assert "test-fixtures" not in path.read_text(encoding="utf-8")


def test_workflow_exposes_every_phase_and_strict_aggregate() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    assert "permissions:\n  contents: read\n" in workflow
    assert workflow.count("actions/checkout@") == 14
    assert workflow.count("persist-credentials: false") == 14
    for phase in PHASES:
        assert f"          - {phase}" in workflow
        job = workflow_job(workflow, phase)
        if phase == "tvm-contract-smoke":
            assert "needs: [runner-self-check, contract-smoke]" in job
            assert "bash scripts/contract_tvm_runner.sh" in job
            assert "tronbox/tre@sha256:" in job
        elif phase == "swift-sdk":
            assert "needs: [runner-self-check, swift-apple-slice]" in job
        else:
            assert "needs: runner-self-check" in job
            assert f"--phase {phase}" in job
        assert f"tee dist/sccp-production-corridor/{phase}.log" in job
        assert "if-no-files-found: error" in job
    aggregate = workflow_job(workflow, "sccp-production-corridor")
    assert f"needs: [runner-self-check, {', '.join(PHASES)}]" in aggregate
    for state in ("failure", "cancelled", "skipped"):
        assert f"contains(needs.*.result, '{state}')" in aggregate
    assert "exit 1" in aggregate


def test_evidence_workflow_installs_rust_and_runs_real_corridor() -> None:
    job = workflow_job(WORKFLOW.read_text(encoding="utf-8"), "evidence-scripts")
    assert "actions/setup-python@" in job
    assert "actions-rust-lang/setup-rust-toolchain@" in job
    assert "Swatinem/rust-cache@" in job
    assert "bash scripts/check_sccp_production_corridor.sh --phase evidence-scripts" in job


def test_swift_workflow_splits_five_authenticated_apple_slices() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    producer = workflow_job(workflow, "swift-apple-slice")
    manual_guard = (
        "if: ${{ github.event_name != 'workflow_dispatch' || "
        "github.event.inputs.phase == 'all' || "
        "github.event.inputs.phase == 'swift-sdk' }}"
    )
    for required in (
        manual_guard,
        "needs: runner-self-check",
        "runs-on: macos-26",
        "timeout-minutes: 180",
        "fail-fast: false",
        "max-parallel: 5",
        "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
        "persist-credentials: false",
        "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
        'python-version: "3.12"',
        "DEVELOPER_DIR: /Applications/Xcode_26.6.app/Contents/Developer",
        "NORITO_BRIDGE_DEVELOPER_DIR: /Applications/Xcode_26.6.app/Contents/Developer",
        "NORITO_BRIDGE_SLICE_BUILD_ID: ${{ github.run_id }}.${{ github.run_attempt }}",
        "Require the exact Xcode 26.6 release toolchain",
        "Xcode 26.6\\nBuild version 17F113",
        "unexpected DEVELOPER_DIR",
        "bridge and job Xcode identities differ",
        "unable to query Xcode identity",
        "unexpected Xcode identity",
        "rustup target add --toolchain 1.93.1",
        "x86_64-apple-darwin",
        "cargo fetch --locked",
        "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32",
        'cache-bin: "false"',
        'cache-on-failure: "false"',
        'cache-targets: "false"',
        'key: "sccp-apple-slice-registry-v1"',
        'artifact_dir="$RUNNER_TEMP/iroha-sccp-apple-slice-artifacts"',
        'build_dir="$RUNNER_TEMP/iroha-sccp-apple-slice-build"',
        'cargo_target="$RUNNER_TEMP/iroha-sccp-apple-slice-cargo"',
        'slice_root="$RUNNER_TEMP/iroha-sccp-apple-slice-output"',
        'chmod 0700 "$artifact_dir" "$build_dir" "$cargo_target" "$slice_root"',
        "CARGO_BUILD_JOBS=1",
        "CARGO_INCREMENTAL=0",
        "CARGO_NET_OFFLINE=true",
        "MOBILE_SDK_PYTHON_BINARY=$mobile_python",
        "RUSTC=$rustc_path",
        "RUSTC_BOOTSTRAP=1",
        "RUSTDOC=$rustdoc_path",
        'chmod -R a-w "$GITHUB_WORKSPACE"',
        '--produce-slice "${{ matrix.slice }}"',
        '--slice-output-root "$NORITO_BRIDGE_SLICE_OUTPUT_ROOT"',
        "sccp-norito-bridge-apple-slice-${{ github.run_id }}-${{ github.run_attempt }}-${{ matrix.slice }}",
        "iroha-sccp-apple-slice-output/${{ matrix.slice }}/*",
        "if-no-files-found: error",
    ):
        assert required in producer
    matrix = producer.split("      matrix:\n", 1)[1].split("    env:\n", 1)[0]
    assert tuple(
        line.removeprefix("          - ")
        for line in matrix.splitlines()
        if line.startswith("          - ")
    ) == APPLE_SLICES
    assert producer.count("persist-credentials: false") == 1
    assert producer.count("exit 1; }") == 4
    assert producer.count("scripts/build_norito_xcframework.sh") == 1
    assert "workspaces:" not in producer
    assert 'cache-targets: "true"' not in producer
    assert "--assemble-slices" not in producer
    assert "nohup" not in producer
    assert not has_background_command(producer)


def test_swift_workflow_assembles_and_consumes_all_five_slices_without_building() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    job = workflow_job(workflow, "swift-sdk")
    for required in (
        "needs: [runner-self-check, swift-apple-slice]",
        "runs-on: macos-26",
        "timeout-minutes: 180",
        "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
        "persist-credentials: false",
        "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
        'python-version: "3.12"',
        "DEVELOPER_DIR: /Applications/Xcode_26.6.app/Contents/Developer",
        "NORITO_BRIDGE_DEVELOPER_DIR: /Applications/Xcode_26.6.app/Contents/Developer",
        "NORITO_BRIDGE_SLICE_BUILD_ID: ${{ github.run_id }}.${{ github.run_attempt }}",
        "Require the exact Xcode 26.6 release toolchain",
        "Xcode 26.6\\nBuild version 17F113",
        "unexpected DEVELOPER_DIR",
        "bridge and job Xcode identities differ",
        "unable to query Xcode identity",
        "unexpected Xcode identity",
        "toolchain: 1.93.1",
        "rustup target add --toolchain 1.93.1",
        "x86_64-apple-darwin",
        "cargo fetch --locked",
        "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32",
        'cache-bin: "false"',
        'cache-on-failure: "false"',
        'cache-targets: "false"',
        'key: "sccp-apple-assembly-registry-v1"',
        'artifact_dir="$RUNNER_TEMP/iroha-sccp-apple-artifacts"',
        'build_dir="$RUNNER_TEMP/iroha-sccp-apple-build"',
        'cargo_target="$RUNNER_TEMP/iroha-sccp-apple-assembly-cargo"',
        'slice_root="$RUNNER_TEMP/iroha-sccp-apple-slices"',
        'swift_scratch_dir="$RUNNER_TEMP/iroha-sccp-swift-build"',
        'chmod 0700 "$artifact_dir" "$build_dir" "$cargo_target" "$slice_root" "$swift_scratch_dir"',
        "CARGO_BUILD_JOBS=1",
        "CARGO_INCREMENTAL=0",
        "CARGO_NET_OFFLINE=true",
        "MOBILE_SDK_APPLE_ARTIFACT_DIR=$artifact_dir",
        "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1",
        "MOBILE_SDK_SWIFT_SCRATCH_DIR=$swift_scratch_dir",
        "MOBILE_SDK_PYTHON_BINARY=$mobile_python",
        "NORITO_BRIDGE_BUILD_DIR=$build_dir",
        "NORITO_BRIDGE_SLICE_INPUT_ROOT=$slice_root",
        "SCCP_SWIFT_BRIDGE_MODE=assembled",
        "RUSTC=$rustc_path",
        "RUSTC_BOOTSTRAP=1",
        "RUSTDOC=$rustdoc_path",
        'chmod -R a-w "$GITHUB_WORKSPACE"',
        '--assemble-slices "$NORITO_BRIDGE_SLICE_INPUT_ROOT"',
        "bash scripts/check_mobile_sdk_artifacts.sh --apple-only",
        "bash scripts/check_sccp_production_corridor.sh --phase swift-sdk",
        "tee dist/sccp-production-corridor/swift-sdk.log",
    ):
        assert required in job
    for slice_id in APPLE_SLICES:
        artifact_name = (
            "sccp-norito-bridge-apple-slice-${{ github.run_id }}-"
            f"${{{{ github.run_attempt }}}}-{slice_id}"
        )
        assert artifact_name in job
        assert f"iroha-sccp-apple-slices/{slice_id}" in job
    assert job.count("actions/download-artifact@") == len(APPLE_SLICES)
    assert job.count("persist-credentials: false") == 1
    assert job.count("exit 1; }") == 4
    assert job.count("scripts/build_norito_xcframework.sh") == 1
    assert "workspaces:" not in job
    assert 'cache-targets: "true"' not in job
    assert "--produce-slice" not in job
    assert not has_native_compile(job)
    assert "nohup" not in job
    assert not has_background_command(job)
    assert job.index("Require the exact Xcode 26.6 release toolchain") < job.index(
        '--assemble-slices "$NORITO_BRIDGE_SLICE_INPUT_ROOT"'
    ) < job.index("bash scripts/check_sccp_production_corridor.sh --phase swift-sdk")
    assert "target/sccp-production-corridor" not in job


@pytest.mark.parametrize(
    ("job_name", "marker"),
    (
        (
            "swift-apple-slice",
            '            --slice-output-root "$NORITO_BRIDGE_SLICE_OUTPUT_ROOT"\n',
        ),
        (
            "swift-sdk",
            '            --assemble-slices "$NORITO_BRIDGE_SLICE_INPUT_ROOT"\n',
        ),
    ),
)
def test_swift_workflow_contract_detects_multiline_background_builds(
    job_name: str,
    marker: str,
) -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    assert workflow.count(marker) == 1
    for suffix in (" &\n", " & wait\n"):
        changed = workflow.replace(marker, marker.rstrip("\n") + suffix, 1)
        assert has_background_command(workflow_job(changed, job_name))


@pytest.mark.parametrize(
    "command",
    (
        "cargo build -p connect_norito_bridge",
        "cargo   rustc -p connect_norito_bridge",
        "rustc forged.rs",
    ),
)
def test_swift_workflow_contract_detects_direct_consumer_compilation(
    command: str,
) -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    marker = "      - name: Assemble authenticated SCCP NoritoBridge XCFramework\n"
    assert workflow.count(marker) == 1
    changed = workflow.replace(marker, f"      - run: {command}\n{marker}", 1)
    assert has_native_compile(workflow_job(changed, "swift-sdk"))


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
            else "needs: [runner-self-check, swift-apple-slice]"
            in workflow_job(changed, phase)
            if phase == "swift-sdk"
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
