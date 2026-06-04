"""Tests for the SCCP production corridor runner."""

from __future__ import annotations

import os
import re
import subprocess
import sys
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path

import pytest


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
AGGREGATE_JOB = "sccp-production-corridor"
AGGREGATE_NEEDS = [
    "runner-self-check",
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
]
PHASE_TEST_PATH_PATTERNS = {
    "evidence-scripts": (r"pytests/scripts/[A-Za-z0-9_]+_test\.py",),
    "js-sdk": (r"javascript/iroha_js/test/[A-Za-z0-9_.-]+\.js",),
    "python-sdk": (r"python/iroha_torii_client/tests/[A-Za-z0-9_]+_test\.py",),
    "contract-smoke": (
        r"contracts/evm/sccp/test/[A-Za-z0-9_.-]+\.js",
        r"scripts/sccp_evm_contract_smoke\.sh",
    ),
}


def workflow_job(workflow: str, job: str) -> str:
    """Return a top-level job block from the workflow text."""

    marker = f"  {job}:\n"
    start = workflow.find(marker)
    assert start >= 0, f"workflow missing job {job}"
    rest = workflow[start + len(marker) :]
    end = len(workflow)
    for candidate in EXPECTED_PHASES | {AGGREGATE_JOB}:
        next_marker = f"\n  {candidate}:\n"
        index = rest.find(next_marker)
        if index >= 0:
            end = min(end, start + len(marker) + index)
    return workflow[start:end]


def assert_sccp_aggregate_gate(workflow: str) -> None:
    """Assert that the aggregate job requires every phase result."""

    aggregate = workflow_job(workflow, AGGREGATE_JOB)
    expected_needs = f"    needs: [{', '.join(AGGREGATE_NEEDS)}]"
    assert expected_needs in aggregate
    assert "if: ${{ always() &&" in aggregate
    assert "github.event.inputs.phase == 'all'" in aggregate
    assert "contains(needs.*.result, 'failure')" in aggregate
    assert "contains(needs.*.result, 'cancelled')" in aggregate
    assert "contains(needs.*.result, 'skipped')" in aggregate
    assert "exit 1" in aggregate
    assert "All SCCP production corridor phases completed successfully." in aggregate


def assert_phase_artifact_uploads_are_strict(workflow: str) -> None:
    """Assert every phase uploads a phase-local transcript as required evidence."""

    for phase in EXPECTED_PHASES:
        job = workflow_job(workflow, phase)
        assert "    needs: runner-self-check" in job
        assert f"bash scripts/check_sccp_production_corridor.sh --phase {phase}" in job
        assert f"tee dist/sccp-production-corridor/{phase}.log" in job
        assert "      - uses: actions/upload-artifact@v4" in job
        assert "        if: always()" in job
        assert f"          name: sccp-production-corridor-{phase}" in job
        assert f"          path: dist/sccp-production-corridor/{phase}.log" in job
        assert "          if-no-files-found: error" in job


def assert_mobile_workflow_uses_jdk21(workflow: str) -> None:
    """Assert mobile SCCP jobs install JDK 21 before running Gradle phases."""

    for phase in ("kotlin-sdk", "java-android"):
        job = workflow_job(workflow, phase)
        assert "      - uses: actions/setup-java@v4" in job
        assert "          distribution: temurin" in job
        assert '          java-version: "21"' in job
        assert f"bash scripts/check_sccp_production_corridor.sh --phase {phase}" in job


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


def test_sccp_production_corridor_workflow_has_aggregate_phase_gate() -> None:
    """The PR/scheduled corridor must not pass unless every phase job passed."""

    workflow = WORKFLOW.read_text(encoding="utf-8")

    assert_sccp_aggregate_gate(workflow)
    assert_phase_artifact_uploads_are_strict(workflow)
    assert_mobile_workflow_uses_jdk21(workflow)


def test_sccp_production_corridor_aggregate_rejects_missing_phase_need() -> None:
    """A dropped phase dependency must fail the workflow self-check."""

    workflow = WORKFLOW.read_text(encoding="utf-8")
    mutated = workflow.replace(", dotnet-sdk", "", 1)

    with pytest.raises(AssertionError):
        assert_sccp_aggregate_gate(mutated)


def test_sccp_production_corridor_aggregate_rejects_skipped_phase_tolerance() -> None:
    """The aggregate job must treat skipped phase jobs as a failure."""

    workflow = WORKFLOW.read_text(encoding="utf-8")
    mutated = workflow.replace(
        " || contains(needs.*.result, 'skipped')",
        "",
        1,
    )

    with pytest.raises(AssertionError):
        assert_sccp_aggregate_gate(mutated)


def test_sccp_production_corridor_aggregate_rejects_single_phase_dispatch_gate() -> None:
    """The aggregate job must not require all phases during a single-phase dispatch."""

    workflow = WORKFLOW.read_text(encoding="utf-8")
    mutated = workflow.replace(
        " && (github.event_name != 'workflow_dispatch' || github.event.inputs.phase == 'all')",
        "",
        1,
    )

    with pytest.raises(AssertionError):
        assert_sccp_aggregate_gate(mutated)


def test_sccp_production_corridor_artifact_guard_rejects_optional_transcripts() -> None:
    """Phase transcripts must remain required release evidence artifacts."""

    workflow = WORKFLOW.read_text(encoding="utf-8")
    mutated = workflow.replace("          if-no-files-found: error", "", 1)

    with pytest.raises(AssertionError):
        assert_phase_artifact_uploads_are_strict(mutated)


def test_sccp_production_corridor_mobile_jobs_reject_missing_jdk21_pin() -> None:
    """Mobile workflow jobs must stay pinned to JDK 21."""

    workflow = WORKFLOW.read_text(encoding="utf-8")
    mutated = workflow.replace('          java-version: "21"\n', "", 1)

    with pytest.raises(AssertionError):
        assert_mobile_workflow_uses_jdk21(mutated)


def test_sccp_production_corridor_mobile_jobs_reject_wrong_jdk_major() -> None:
    """Mobile workflow jobs must not drift to another Java major version."""

    workflow = WORKFLOW.read_text(encoding="utf-8")
    mutated = workflow.replace('          java-version: "21"', '          java-version: "25"', 1)

    with pytest.raises(AssertionError):
        assert_mobile_workflow_uses_jdk21(mutated)


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


def test_sccp_production_corridor_kotlin_phase_covers_sccp_package() -> None:
    """The Kotlin phase must keep the JVM SCCP package test selector."""

    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--phase",
            "kotlin-sdk",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert "==> SCCP production corridor: kotlin-sdk" in completed.stdout
    assert "JAVA_HOME=" in completed.stdout
    assert "java -version" in completed.stdout
    assert "./gradlew :core-jvm:test --console=plain --tests" in completed.stdout
    assert "org.hyperledger.iroha.sdk.sccp." in completed.stdout
    assert "SCCP production corridor dry run completed." in completed.stdout


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


def test_sccp_production_corridor_dry_run_honors_script_runtime_overrides() -> None:
    """The runner must honor explicit Node/Python binaries for reproducible local reruns."""

    env = os.environ.copy()
    env["SCCP_CORRIDOR_NODE_BIN"] = "/tmp/iroha-node20"
    env["SCCP_CORRIDOR_PYTHON_BIN"] = "/tmp/iroha-python-pytest"
    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--phase",
            "evidence-scripts,js-sdk,python-sdk,contract-smoke",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    assert "+ /tmp/iroha-python-pytest -m pytest -q pytests/scripts/" in (
        completed.stdout
    )
    assert "+ /tmp/iroha-node20 --test javascript/iroha_js/test/" in completed.stdout
    assert (
        "+ /tmp/iroha-python-pytest -m pytest -q "
        "python/iroha_torii_client/tests/sccp_test.py"
    ) in completed.stdout
    assert (
        "+ /tmp/iroha-node20 --check "
        "contracts/evm/sccp/test/sccp_message_bridge_smoke.js"
    ) in completed.stdout


def test_sccp_production_corridor_override_dry_run_matches_release_fragments() -> None:
    """Runtime-overridden command traces must still satisfy release evidence."""

    report = load_report_module()
    phases = ("evidence-scripts", "js-sdk", "python-sdk", "contract-smoke")
    env = os.environ.copy()
    env["SCCP_CORRIDOR_NODE_BIN"] = "/tmp/iroha-node20"
    env["SCCP_CORRIDOR_PYTHON_BIN"] = "/tmp/iroha-python-pytest"
    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--phase",
            ",".join(phases),
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )
    output = completed.stdout

    for index, phase in enumerate(phases):
        marker = f"==> SCCP production corridor: {phase}"
        assert marker in output
        start = output.index(marker)
        if index + 1 < len(phases):
            end = output.index(
                f"==> SCCP production corridor: {phases[index + 1]}",
                start + len(marker),
            )
        else:
            end = len(output)
        phase_output = output[start:end]
        for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]:
            assert fragment in phase_output


def test_sccp_production_corridor_dry_run_matches_release_phase_fragments() -> None:
    """Release evidence command fragments must stay synced to the runner output."""

    report = load_report_module()
    assert set(report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS) == EXPECTED_PHASES
    assert set(report.PHASE_TRANSCRIPT_SUCCESS_FRAGMENTS) == EXPECTED_PHASES

    env = os.environ.copy()
    env.pop("SCCP_CORRIDOR_NODE_BIN", None)
    env.pop("SCCP_CORRIDOR_PYTHON_BIN", None)
    completed = subprocess.run(
        ["bash", str(SCRIPT), "--dry-run"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
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


def test_sccp_production_corridor_evidence_phase_has_no_untracked_pytests() -> None:
    """Every evidence-script pytest path must be present in release evidence."""

    report = load_report_module()
    completed = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--dry-run",
            "--phase",
            "evidence-scripts",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    required_fragments = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS["evidence-scripts"]
    pytest_paths = sorted(
        set(
            re.findall(
                r"pytests/scripts/[A-Za-z0-9_]+_test\.py",
                completed.stdout,
            )
        )
    )
    missing = [
        path
        for path in pytest_paths
        if not any(path in fragment for fragment in required_fragments)
    ]

    assert missing == [], f"untracked evidence-scripts pytest paths: {missing}"


def test_sccp_production_corridor_test_paths_are_release_inventory_tracked() -> None:
    """Path-based corridor test commands must be present in release evidence."""

    report = load_report_module()

    for phase, patterns in PHASE_TEST_PATH_PATTERNS.items():
        completed = subprocess.run(
            [
                "bash",
                str(SCRIPT),
                "--dry-run",
                "--phase",
                phase,
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        required_fragments = report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]
        command_paths = sorted(
            {
                match
                for pattern in patterns
                for match in re.findall(pattern, completed.stdout)
            }
        )
        assert command_paths, f"{phase} dry-run exposed no tracked test paths"

        missing = [
            path
            for path in command_paths
            if not any(path in fragment for fragment in required_fragments)
        ]
        assert missing == [], f"{phase} has untracked release test paths: {missing}"


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
    assert "java -version" in completed.stdout
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

    assert 'if [[ -n "${JAVA_HOME:-}" ]] && is_java_21_home "$JAVA_HOME"; then' in script
    assert 'version[[:space:]]+\\"21(\\.|\\")' in script
    assert "run_java_version_check \"$java_home\"" in script
    assert 'macos_java_home="$(/usr/libexec/java_home -v 21 2>/dev/null)"' in script
    assert '&& is_java_21_home "$macos_java_home"; then' in script
    assert "/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home" in script
    assert "/usr/local/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home" in script
    assert "/opt/homebrew/Cellar/openjdk@21/*/libexec/openjdk.jdk/Contents/Home" in (
        script
    )
    assert "install Homebrew openjdk@21" in script
