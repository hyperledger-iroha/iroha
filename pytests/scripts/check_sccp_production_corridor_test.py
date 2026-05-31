"""Tests for the SCCP production corridor runner."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_sccp_production_corridor.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "sccp_production_corridor.yml"
EXPECTED_PHASES = {
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "contract-smoke",
    "core-admission",
}


def test_sccp_production_corridor_script_is_listable() -> None:
    """The runner must stay syntactically valid and expose every release phase."""

    subprocess.run(["bash", "-n", str(SCRIPT)], check=True)
    completed = subprocess.run(
        ["bash", str(SCRIPT), "--list"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    listed_phases = {
        line.strip()
        for line in completed.stdout.splitlines()
        if line.startswith("  ")
    }

    assert listed_phases == EXPECTED_PHASES


def test_sccp_production_corridor_workflow_tracks_runner_phases() -> None:
    """The GitHub Actions attachment must keep every runner phase reachable."""

    workflow = WORKFLOW.read_text(encoding="utf-8")

    assert "workflow_dispatch:" in workflow
    assert "pull_request:" in workflow
    assert "schedule:" in workflow
    assert "scripts/check_sccp_production_corridor.sh" in workflow
    assert workflow.count("uses: actions/upload-artifact@v4") >= len(EXPECTED_PHASES)

    for phase in EXPECTED_PHASES:
        assert f"- {phase}" in workflow
        assert f"--phase {phase}" in workflow
        assert f"tee dist/sccp-production-corridor/{phase}.log" in workflow
        assert f"name: sccp-production-corridor-{phase}" in workflow
        assert f"path: dist/sccp-production-corridor/{phase}.log" in workflow


def test_sccp_production_corridor_java_android_phase_matches_test_surfaces() -> None:
    """The Java Android phase must keep JUnit-only Solana outside the main harness."""

    script = SCRIPT.read_text(encoding="utf-8")
    harness_line = next(
        line for line in script.splitlines() if "android_harness_mains=" in line
    )

    assert "org.hyperledger.iroha.android.GradleHarnessTests" in script
    assert "org.hyperledger.iroha.android.sccp.SolanaSccpProverTests" not in harness_line
    assert (
        "./gradlew :core:test --console=plain --tests "
        "org.hyperledger.iroha.android.sccp.SolanaSccpProverTests"
    ) in script


def test_sccp_production_corridor_swift_phase_covers_submit_payloads() -> None:
    """The Swift phase must cover user-prover submission packaging."""

    script = SCRIPT.read_text(encoding="utf-8")

    assert (
        "swift test --filter SccpSolanaProverTests --disable-swift-testing"
        in script
    )
    assert (
        "ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions"
        in script
    )
    assert (
        "--disable-swift-testing"
    ) in script


def test_sccp_production_corridor_swift_phase_materializes_bridge_first() -> None:
    """The Swift phase must provide NoritoBridge before SwiftPM resolution."""

    script = SCRIPT.read_text(encoding="utf-8")
    phase_start = script.index("phase_swift_sdk()")
    phase_end = script.index("phase_kotlin_sdk()")
    phase_body = script[phase_start:phase_end]

    assert 'local bridge_dir="$ROOT/dist/NoritoBridge.xcframework"' in script
    assert 'local bridge_zip="$ROOT/dist/NoritoBridge.xcframework.zip"' in script
    assert 'run_cmd unzip -q -o "$bridge_zip" -d "$ROOT/dist"' in script
    assert 'run_cmd rustup target add "${rust_targets[@]}"' in script
    assert 'run_cmd bash "$ROOT/scripts/build_norito_xcframework.sh"' in script
    assert phase_body.index("ensure_swift_bridge_artifact") < phase_body.index(
        "swift test --filter SccpSolanaProverTests"
    )


def test_sccp_production_corridor_swift_dry_run_prints_bridge_materialization() -> None:
    """Dry-run mode must show bridge materialization before Swift tests."""

    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--phase",
            "swift-sdk",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    materialization_positions = [
        completed.stdout.find(marker)
        for marker in ("+ rustup target add aarch64-apple-ios", "+ unzip -q -o ")
        if completed.stdout.find(marker) >= 0
    ]

    assert materialization_positions
    assert min(materialization_positions) < completed.stdout.index(
        "swift test --filter SccpSolanaProverTests"
    )


def test_sccp_production_corridor_dry_run_prints_selected_phase_commands() -> None:
    """The release runner can print selected heavyweight phases without running them."""

    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--phase",
            "contract-smoke,core-admission",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert "==> SCCP production corridor: contract-smoke" in completed.stdout
    assert "+ node --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js" in (
        completed.stdout
    )
    assert "+ bash scripts/sccp_evm_contract_smoke.sh" in completed.stdout
    assert "cargo test -p iroha_core --test bridge_proofs -- --nocapture" in (
        completed.stdout
    )
    assert "SCCP production corridor dry run completed." in completed.stdout


def test_sccp_production_corridor_log_dir_dry_run_is_explicit(tmp_path: Path) -> None:
    """Local release evidence collection must be visible before heavyweight runs."""

    log_dir = tmp_path / "logs"
    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--log-dir",
            str(log_dir),
            "--phase",
            "js-sdk,python-sdk",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    script = SCRIPT.read_text(encoding="utf-8")
    assert "SCCP production corridor logs would be written to" in completed.stdout
    assert str(log_dir) in completed.stdout
    assert "==> SCCP production corridor: js-sdk" in completed.stdout
    assert "==> SCCP production corridor: python-sdk" in completed.stdout
    assert "tee \"$LOG_DIR/$phase.log\"" in script


def test_sccp_production_corridor_dry_run_skips_mobile_toolchain_resolution() -> None:
    """Dry-run mode must not fail just because Java or Android SDKs are unavailable."""

    env = os.environ.copy()
    env.pop("JAVA_HOME", None)
    env.pop("ANDROID_HOME", None)
    env.pop("ANDROID_SDK_ROOT", None)
    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--phase",
            "kotlin-sdk,java-android",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    assert "./gradlew :core-jvm:test --console=plain --tests" in completed.stdout
    assert "./gradlew :core:test --console=plain --tests" in completed.stdout
    assert "ANDROID_HOME=" in completed.stdout
    assert "SCCP production corridor dry run completed." in completed.stdout
