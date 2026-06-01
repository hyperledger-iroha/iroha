"""Tests for the SCCP production corridor runner."""

from __future__ import annotations

import os
import subprocess
import sys
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_sccp_production_corridor.sh"
REPORT_SCRIPT = ROOT / "scripts" / "sccp_release_readiness_report.py"
WORKFLOW = ROOT / ".github" / "workflows" / "sccp_production_corridor.yml"
EXPECTED_PHASES = {
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "core-admission",
}


def load_report_module():
    """Load release-readiness helpers without running the CLI."""

    spec = spec_from_file_location(
        "sccp_release_readiness_report_corridor_helpers",
        REPORT_SCRIPT,
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


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


def test_sccp_production_corridor_dry_run_matches_release_phase_fragments() -> None:
    """Release evidence command fragments must stay synced to the runner output."""

    report = load_report_module()
    assert set(report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS) == EXPECTED_PHASES
    assert set(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS) == EXPECTED_PHASES

    completed = subprocess.run(
        ["bash", str(SCRIPT), "--dry-run"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    output = completed.stdout

    phase_markers = [
        (phase, f"==> SCCP production corridor: {phase}")
        for phase in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS
    ]
    for index, (phase, marker) in enumerate(phase_markers):
        assert marker in output
        start = output.index(marker)
        if index + 1 < len(phase_markers):
            next_marker = phase_markers[index + 1][1]
            end = output.index(next_marker, start + len(marker))
        else:
            end = len(output)
        phase_output = output[start:end]
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]:
            assert fragment in phase_output


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


def test_sccp_production_corridor_dotnet_phase_covers_native_bsc_facades() -> None:
    """The native .NET phase must compile and run the ETH/BSC SCCP facade tests."""

    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--phase",
            "dotnet-sdk",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert "==> SCCP production corridor: dotnet-sdk" in completed.stdout
    assert "DOTNET_CLI_TELEMETRY_OPTOUT=1" in completed.stdout
    assert "DOTNET_CLI_UI_LANGUAGE=en" in completed.stdout
    assert (
        "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/"
        "Hyperledger.Iroha.Sdk.Tests.csproj"
    ) in completed.stdout
    assert (
        "FullyQualifiedName~SccpEthereumMainnetTests\\|"
        "FullyQualifiedName~SccpBscMainnetTests"
    ) in completed.stdout
    assert "SCCP production corridor dry run completed." in completed.stdout


def test_sccp_production_corridor_java_home_resolver_handles_homebrew_jdk() -> None:
    """Gradle phases must fall back to Homebrew JDK 21 on macOS workstations."""

    script = SCRIPT.read_text(encoding="utf-8")

    assert 'macos_java_home="$(/usr/libexec/java_home -v 21 2>/dev/null)"' in script
    assert '[[ -x "$macos_java_home/bin/java" ]]' in script
    assert "/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home" in script
    assert "/usr/local/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home" in script
    assert "/opt/homebrew/Cellar/openjdk@21/*/libexec/openjdk.jdk/Contents/Home" in (
        script
    )
    assert "install Homebrew openjdk@21" in script
