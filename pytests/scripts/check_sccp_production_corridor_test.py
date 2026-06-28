"""Tests for the SCCP production corridor runner."""

from __future__ import annotations

import os
import re
import shutil
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


def assert_phase_output_matches_required_fragments(report, phase: str, output: str) -> None:
    """Assert that a dry-run phase would satisfy release transcript matching."""

    commands = report._phase_command_lines(output)
    for fragment in report.PHASE_TRANSCRIPT_REQUIRED_FRAGMENTS[phase]:
        assert any(
            report._phase_command_matches_required_fragment(phase, command, fragment)
            for command in commands
        ), f"{phase} dry-run missing required release fragment: {fragment}"


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
    assert "GRADLE_OPTS=-Dorg.gradle.jvmargs=-Xmx6g" in completed.stdout
    assert "-Dkotlin.daemon.jvmargs=-Xmx6g" in completed.stdout
    assert "-Dkotlin.daemon.jvm.options=-Xmx6g" in completed.stdout
    assert "java -version" in completed.stdout
    assert "./gradlew :core-jvm:test --console=plain --tests" in completed.stdout
    assert "org.hyperledger.iroha.sdk.sccp." in completed.stdout
    assert "org.hyperledger.iroha.sdk.sccp.TonSccpProverTest" in completed.stdout
    assert "SCCP production corridor dry run completed." in completed.stdout


def test_sccp_production_corridor_gradle_opts_override_is_preserved() -> None:
    """Operator-supplied Gradle memory settings must override corridor defaults."""

    env = os.environ.copy()
    env["GRADLE_OPTS"] = "-Dorg.gradle.jvmargs=-Xmx3g -Dcustom.flag=true"
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
        env=env,
    )

    assert "GRADLE_OPTS=-Dorg.gradle.jvmargs=-Xmx3g\\ -Dcustom.flag=true" in (
        completed.stdout
    )
    assert "-Dkotlin.daemon.jvmargs=-Xmx6g" not in completed.stdout


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
    assert "scripts/sccp_bsc_taira_xor_deploy.test.mjs" in completed.stdout
    assert "scripts/sccp_tron_taira_xor_deploy.test.mjs" in completed.stdout
    assert "scripts/sccp_taira_xor_contract.test.mjs" in completed.stdout
    assert "+ node --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js" in (
        completed.stdout
    )
    assert "+ bash scripts/sccp_evm_contract_smoke.sh" in completed.stdout
    assert "cargo test -p iroha_core --test iroha_core_group_01 bridge_proofs:: -- --nocapture" in (
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
        assert_phase_output_matches_required_fragments(report, phase, phase_output)


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
        assert_phase_output_matches_required_fragments(report, phase, phase_output)


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


def test_sccp_production_corridor_evidence_phase_runs_retired_network_scan() -> None:
    """The evidence phase must keep the retired-network surface scan in the runner."""

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

    assert "pytests/scripts/sccp_retired_network_surface_test.py" in completed.stdout


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


@pytest.mark.parametrize(
    "args",
    (
        ("--log-dir", ""),
        ("--log-dir=",),
    ),
)
def test_sccp_production_corridor_rejects_empty_log_dir(args: tuple[str, ...]) -> None:
    """Release evidence collection must not silently drop an empty log directory."""

    completed = subprocess.run(
        ["bash", str(SCRIPT), "--dry-run", *args, "--phase", "js-sdk"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "--log-dir requires a directory." in completed.stderr
    assert "==> SCCP production corridor: js-sdk" not in completed.stdout


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
    assert "GRADLE_OPTS=-Dorg.gradle.jvmargs=-Xmx6g" in completed.stdout
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
    assert "dotnet --version" in completed.stdout
    assert "dotnet --info" in completed.stdout
    assert "CARGO_TARGET_DIR=" in completed.stdout
    assert "cargo build -p connect_norito_bridge" in completed.stdout
    assert "dotnet restore Hyperledger.Iroha.Sdk.sln" in completed.stdout
    assert (
        "dotnet test tests/Hyperledger.Iroha.Sdk.Tests/"
        "Hyperledger.Iroha.Sdk.Tests.csproj"
    ) in completed.stdout
    assert "FullyQualifiedName~Sccp" in completed.stdout
    assert "FullyQualifiedName~SccpEthereumMainnetTests|" not in completed.stdout
    assert "sccp-dotnet-sdk.trx" in completed.stdout
    assert "SCCP production corridor dry run completed." in completed.stdout

    script = SCRIPT.read_text(encoding="utf-8")
    assert "stable canonical .NET 8.0.x SDK version" in script
    assert "exactly one canonical SDK version line" in script
    assert "non-zero patch" in script
    assert r"^8\.0\.[1-9][0-9]*$" in script
    assert r"^8\.[0-9]+\.[0-9]+(-[A-Za-z0-9][A-Za-z0-9_.-]*)?$" not in script
    assert 'dotnet_info_field_count "OS Architecture"' in script
    assert 'dotnet_info_section_field_count "Host" "Architecture"' in script
    assert 'dotnet_info_section_field_value "Host" "Architecture"' in script
    assert "exactly one OS Name and one OS Platform" in script
    assert "exactly one canonical Windows RID" in script
    assert "exactly one OS Architecture" in script
    assert "exactly one Host Architecture" in script
    assert "dotnet_info_field_value" in script
    assert "substr(line, length(label) + 2)" in script
    assert "awk -F:" not in script
    assert "canonical Windows RID" in script
    assert "SCCP .NET SDK Architecture:" in script
    assert "canonical architecture" in script
    assert "^(x64|x86|arm64|arm)$" in script
    assert "tr '[:upper:]' '[:lower:]'" not in script
    assert 'tr "[:upper:]" "[:lower:]"' not in script
    assert "requires the Windows RID architecture to match" in script
    assert "connect_norito_bridge.dll" in script
    assert "connect_norito_bridge native bridge:" in script
    assert "connect_norito_bridge native bridge sha256:" in script
    assert "non-canonical native bridge SHA-256" in script
    assert "requires exactly one .NET TRX result" in script
    assert '[[ ! -s "$dotnet_trx_path" ]]' in script
    assert "empty TRX result" in script
    assert "SCCP .NET SDK TRX:" in script
    assert "SCCP .NET SDK TRX bytes:" in script
    assert "dotnet_test_passed_count" in script
    assert "requires exactly one canonical VSTest summary" in script
    assert "requires VSTest summary to report Failed: 0, Skipped: 0, and Total == Passed" in script
    assert "requires TRX UnitTestResult count to match VSTest passed-test count" in script
    assert 'validate_dotnet_trx_content "$dotnet_trx_path" "$dotnet_passed_count"' in script
    assert "validate_dotnet_trx_content" in script
    assert "SCCP_DOTNET_TRX_MAX_BYTES=16777216" in script
    assert "requires TRX result to be at most" in script
    assert "xml.etree.ElementTree" in script
    assert "Hyperledger.Iroha.Sdk.Tests.dll" in script
    assert 'local_name(element.tag) == "UnitTestResult"' in script
    assert 'result.attrib.get("outcome") != "Passed"' in script
    assert "assembly_sccp_test_ids" in script
    assert "assembly_sccp_execution_ids" in script
    assert "sccp_test_id_to_names" in script
    assert "sccp_execution_id_to_names" in script
    assert "seen_unit_test_ids" in script
    assert "seen_execution_ids" in script
    assert "SCCP_TEST_NAME_RE" in script
    assert r"(^|[.])Sccp[A-Za-z0-9_]*(?:$|[.])" in script
    assert "ASCII_CONTROL_RE" in script
    assert "is_canonical_trx_test_name" in script
    assert "value == value.strip()" in script
    assert "ASCII_CONTROL_RE.search(value) is None" in script
    assert "has_sccp_test_name_token" in script
    assert '"sccp" in value.lower()' not in script
    assert '"adapterTypeName"' not in script
    assert 'result.attrib.get("testId") in assembly_sccp_test_ids' in script
    assert 'result.attrib.get("executionId") in assembly_sccp_execution_ids' in script
    assert "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition" in script
    assert "requires TRX UnitTestResult testName to match its SCCP test definition" in script
    assert "requires unique TRX UnitTest id values" in script
    assert "requires unique TRX Execution id values" in script
    assert "requires TRX result to contain no DTD or entity declarations" in script
    assert "requires TRX result to be well-formed XML" in script
    assert "requires TRX root to be a VSTest TestRun" in script
    assert (
        "requires every TRX UnitTestResult to appear directly under the VSTest Results section"
        in script
    )
    assert (
        "requires every TRX UnitTest definition to appear directly under the VSTest TestDefinitions section"
        in script
    )
    assert 'local_name(root.tag) != "TestRun"' in script
    assert 'local_name(child.tag) == "Results"' in script
    assert 'local_name(child.tag) == "TestDefinitions"' in script
    assert r"^[1-9][0-9]*$" in script
    direct_trx_path = (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/TestResults/"
        "sccp-dotnet-sdk.trx"
    )
    nested_trx_path_pattern = (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/*/TestResults/"
        "sccp-dotnet-sdk.trx"
    )
    assert (
        direct_trx_path in script
    ), "direct .NET TRX TestResults path must remain the only accepted path"
    assert nested_trx_path_pattern not in script, (
        "nested .NET TRX TestResults paths must remain rejected"
    )
    assert "*sccp-dotnet-sdk.trx" not in script


def test_sccp_production_corridor_dotnet_phase_rejects_rid_architecture_mismatch(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must fail before build when RID and arch diverge."""

    dotnet_root = tmp_path / "dotnet-root"
    dotnet_root.mkdir()
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: arm64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: arm64
  RID:         win-x64
EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)

    completed = subprocess.run(
        ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    assert completed.returncode == 1
    assert "SCCP .NET SDK validation requires the Windows RID architecture to match" in (
        completed.stderr
    )
    assert "RID: win-x64" in completed.stderr
    assert "architecture: arm64" in completed.stderr
    assert "cargo build -p connect_norito_bridge" not in completed.stdout
    assert "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
    assert "dotnet test tests/Hyperledger.Iroha.Sdk.Tests" not in completed.stdout


def test_sccp_production_corridor_dotnet_phase_rejects_multiline_version(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must not normalize multi-line version output."""

    dotnet_root = tmp_path / "dotnet-root"
    dotnet_root.mkdir()
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n8.0.102\\n'
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)

    completed = subprocess.run(
        ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    assert completed.returncode == 1
    assert "dotnet --version to emit exactly one canonical SDK version line" in (
        completed.stderr
    )
    assert "SCCP .NET SDK version:" not in completed.stdout
    assert "dotnet --info" not in completed.stdout
    assert "cargo build -p connect_norito_bridge" not in completed.stdout
    assert "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
    assert "dotnet test tests/Hyperledger.Iroha.Sdk.Tests" not in completed.stdout


def test_sccp_production_corridor_dotnet_phase_rejects_contradictory_os_markers(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must require both OS fields to be Windows."""

    dotnet_root = tmp_path / "dotnet-root"
    dotnet_root.mkdir()
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Linux
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)

    completed = subprocess.run(
        ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    assert completed.returncode == 1
    assert "SCCP .NET SDK validation must be captured on Windows" in (
        completed.stderr
    )
    assert "OS Name: Linux" in completed.stderr
    assert "OS Platform: Windows" in completed.stderr
    assert "SCCP .NET SDK OS: Windows" not in completed.stdout
    assert "cargo build -p connect_norito_bridge" not in completed.stdout
    assert "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
    assert "dotnet test tests/Hyperledger.Iroha.Sdk.Tests" not in completed.stdout


def test_sccp_production_corridor_dotnet_phase_rejects_ambiguous_os_metadata(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must reject duplicate or missing OS metadata."""

    cases = {
        "missing-os-name": """Runtime Environment:
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
""",
        "duplicate-os-name": """Runtime Environment:
  OS Name:     Windows
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
""",
        "missing-os-platform": """Runtime Environment:
  OS Name:     Windows
  OS Architecture: x64
  RID:         win-x64
""",
        "duplicate-os-platform": """Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
""",
    }
    for case_name, runtime_environment in cases.items():
        dotnet_root = tmp_path / case_name / "dotnet-root"
        dotnet_root.mkdir(parents=True)
        fake_dotnet = dotnet_root / "dotnet"
        fake_dotnet.write_text(
            f"""#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

{runtime_environment}EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
            encoding="utf-8",
        )
        fake_dotnet.chmod(0o755)
        env = os.environ.copy()
        env["DOTNET_ROOT"] = str(dotnet_root)

        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )

        assert completed.returncode == 1, case_name
        assert "exactly one OS Name and one OS Platform from dotnet --info" in (
            completed.stderr
        ), case_name
        assert "SCCP .NET SDK OS: Windows" not in completed.stdout, case_name
        assert "SCCP .NET SDK RID:" not in completed.stdout, case_name
        assert (
            "cargo build -p connect_norito_bridge" not in completed.stdout
        ), case_name
        assert (
            "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
        ), case_name
        assert (
            "dotnet test tests/Hyperledger.Iroha.Sdk.Tests"
            not in completed.stdout
        ), case_name


def test_sccp_production_corridor_dotnet_phase_rejects_duplicate_rid(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must not choose one RID from duplicate metadata."""

    dotnet_root = tmp_path / "dotnet-root"
    dotnet_root.mkdir()
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
  RID:         linux-x64
EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)

    completed = subprocess.run(
        ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    assert completed.returncode == 1
    assert "exactly one canonical Windows RID from dotnet --info" in (
        completed.stderr
    )
    assert "found: 2" in completed.stderr
    assert "SCCP .NET SDK RID:" not in completed.stdout
    assert "cargo build -p connect_norito_bridge" not in completed.stdout
    assert "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
    assert "dotnet test tests/Hyperledger.Iroha.Sdk.Tests" not in completed.stdout


def test_sccp_production_corridor_dotnet_phase_rejects_ambiguous_rid_or_architecture_metadata(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must reject ambiguous RID and architecture metadata."""

    cases = {
        "missing-rid": (
            """Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
""",
            "exactly one canonical Windows RID from dotnet --info",
            "found: 0",
        ),
        "duplicate-os-architecture": (
            """Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  OS Architecture: x64
  RID:         win-x64
""",
            "exactly one OS Architecture from dotnet --info",
            "found: 2",
        ),
        "duplicate-host-architecture-fallback": (
            """Host:
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  RID:         win-x64
""",
            "exactly one Host Architecture from dotnet --info when OS Architecture is absent",
            "found: 2",
        ),
    }
    for case_name, (runtime_environment, expected_error, expected_count) in (
        cases.items()
    ):
        dotnet_root = tmp_path / case_name / "dotnet-root"
        dotnet_root.mkdir(parents=True)
        fake_dotnet = dotnet_root / "dotnet"
        fake_dotnet.write_text(
            f"""#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

{runtime_environment}EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
            encoding="utf-8",
        )
        fake_dotnet.chmod(0o755)
        env = os.environ.copy()
        env["DOTNET_ROOT"] = str(dotnet_root)

        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )

        assert completed.returncode == 1, case_name
        assert expected_error in completed.stderr, case_name
        assert expected_count in completed.stderr, case_name
        assert "SCCP .NET SDK RID:" not in completed.stdout, case_name
        assert "SCCP .NET SDK Architecture:" not in completed.stdout, case_name
        assert (
            "cargo build -p connect_norito_bridge" not in completed.stdout
        ), case_name
        assert (
            "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
        ), case_name
        assert (
            "dotnet test tests/Hyperledger.Iroha.Sdk.Tests"
            not in completed.stdout
        ), case_name


def test_sccp_production_corridor_dotnet_phase_rejects_noncanonical_rid_values(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must reject noncanonical single RID values."""

    cases = {
        "uppercase-win-rid": "WIN-x64",
        "foreign-linux-rid": "linux-x64",
        "alias-amd64-rid": "win-amd64",
    }
    for case_name, rid in cases.items():
        dotnet_root = tmp_path / case_name / "dotnet-root"
        dotnet_root.mkdir(parents=True)
        fake_dotnet = dotnet_root / "dotnet"
        fake_dotnet.write_text(
            f"""#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         {rid}
EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
            encoding="utf-8",
        )
        fake_dotnet.chmod(0o755)
        env = os.environ.copy()
        env["DOTNET_ROOT"] = str(dotnet_root)

        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )

        assert completed.returncode == 1, case_name
        assert "requires a canonical Windows RID" in completed.stderr, case_name
        assert f"found: {rid}" in completed.stderr, case_name
        assert "SCCP .NET SDK RID:" not in completed.stdout, case_name
        assert "SCCP .NET SDK Architecture:" not in completed.stdout, case_name
        assert (
            "cargo build -p connect_norito_bridge" not in completed.stdout
        ), case_name
        assert (
            "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
        ), case_name
        assert (
            "dotnet test tests/Hyperledger.Iroha.Sdk.Tests"
            not in completed.stdout
        ), case_name


def test_sccp_production_corridor_dotnet_phase_accepts_host_architecture_fallback(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence may use Host Architecture when OS Architecture is absent."""

    tool_dir = tmp_path / "tools"
    dotnet_root = tmp_path / "dotnet-root"
    bridge_target_dir = tmp_path / "bridge-target"
    direct_trx_path = (
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "TestResults"
        / "sccp-dotnet-sdk.trx"
    )
    tool_dir.mkdir()
    dotnet_root.mkdir()
    fake_cargo = tool_dir / "cargo"
    fake_cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" != "build -p connect_norito_bridge" ]]; then
  printf 'unexpected fake cargo invocation: %s\\n' "$*" >&2
  exit 99
fi
mkdir -p "$CARGO_TARGET_DIR/debug"
printf 'fake bridge dll\\n' > "$CARGO_TARGET_DIR/debug/connect_norito_bridge.dll"
""",
        encoding="utf-8",
    )
    fake_cargo.chmod(0o755)
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  RID:         win-x64
EOF
    ;;
  restore)
    exit 0
    ;;
  test)
    mkdir -p tests/Hyperledger.Iroha.Sdk.Tests/TestResults
    cat > tests/Hyperledger.Iroha.Sdk.Tests/TestResults/sccp-dotnet-sdk.trx <<'EOF'
<TestRun xmlns="http://microsoft.com/schemas/VisualStudio/TeamTest/2010">
  <Results>
    <UnitTestResult executionId="exec-sccp" testId="test-sccp" testName="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests.BuildsProof" outcome="Passed" />
  </Results>
  <TestDefinitions>
    <UnitTest id="test-sccp" name="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests.BuildsProof" storage="C:\\repo\\csharp\\tests\\Hyperledger.Iroha.Sdk.Tests\\bin\\Debug\\net8.0\\Hyperledger.Iroha.Sdk.Tests.dll">
      <Execution id="exec-sccp" />
      <TestMethod codeBase="C:\\repo\\csharp\\tests\\Hyperledger.Iroha.Sdk.Tests\\bin\\Debug\\net8.0\\Hyperledger.Iroha.Sdk.Tests.dll" className="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests" name="BuildsProof" />
    </UnitTest>
  </TestDefinitions>
</TestRun>
EOF
    printf 'Passed!  - Failed: 0, Passed: 1, Skipped: 0, Total: 1, Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\\n'
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)
    env["SCCP_DOTNET_BRIDGE_TARGET_DIR"] = str(bridge_target_dir)
    env["PATH"] = f"{tool_dir}{os.pathsep}{env['PATH']}"

    try:
        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )
    finally:
        direct_trx_path.unlink(missing_ok=True)

    assert completed.returncode == 0, completed.stderr
    assert "SCCP .NET SDK Architecture: x64" in completed.stdout
    assert "cargo build -p connect_norito_bridge" in completed.stdout
    assert "dotnet restore Hyperledger.Iroha.Sdk.sln" in completed.stdout
    assert "dotnet test tests/Hyperledger.Iroha.Sdk.Tests" in completed.stdout
    assert (
        "SCCP .NET SDK TRX: "
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/TestResults/sccp-dotnet-sdk.trx"
        in completed.stdout
    )
    assert "SCCP .NET SDK TRX bytes:" in completed.stdout


def test_sccp_production_corridor_dotnet_phase_rejects_noncanonical_architecture_values(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must reject noncanonical architecture aliases."""

    cases = {
        "alias-amd64-architecture": "amd64",
        "alias-x86_64-architecture": "x86_64",
        "alias-aarch64-architecture": "aarch64",
    }
    for case_name, architecture in cases.items():
        dotnet_root = tmp_path / case_name / "dotnet-root"
        dotnet_root.mkdir(parents=True)
        fake_dotnet = dotnet_root / "dotnet"
        fake_dotnet.write_text(
            f"""#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: {architecture}

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: {architecture}
  RID:         win-x64
EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
            encoding="utf-8",
        )
        fake_dotnet.chmod(0o755)
        env = os.environ.copy()
        env["DOTNET_ROOT"] = str(dotnet_root)

        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )

        assert completed.returncode == 1, case_name
        assert "requires a canonical architecture" in completed.stderr, case_name
        assert f"found: {architecture}" in completed.stderr, case_name
        assert "SCCP .NET SDK Architecture:" not in completed.stdout, case_name
        assert (
            "cargo build -p connect_norito_bridge" not in completed.stdout
        ), case_name
        assert (
            "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
        ), case_name
        assert (
            "dotnet test tests/Hyperledger.Iroha.Sdk.Tests"
            not in completed.stdout
        ), case_name


def test_sccp_production_corridor_dotnet_phase_rejects_colon_injected_info_values(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must reject colon-injected dotnet --info fields."""

    cases = {
        "colon-injected-os-name": (
            """Runtime Environment:
  OS Name:     Windows: Linux
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
""",
            "must be captured on Windows",
            "OS Name: Windows: Linux",
        ),
        "colon-injected-os-platform": (
            """Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows: Linux
  OS Architecture: x64
  RID:         win-x64
""",
            "must be captured on Windows",
            "OS Platform: Windows: Linux",
        ),
        "colon-injected-rid": (
            """Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64:linux-x64
""",
            "requires a canonical Windows RID",
            "found: win-x64:linux-x64",
        ),
        "colon-injected-architecture": (
            """Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64:arm64
  RID:         win-x64
""",
            "requires a canonical architecture",
            "found: x64:arm64",
        ),
    }
    for case_name, (runtime_environment, expected_error, expected_value) in (
        cases.items()
    ):
        dotnet_root = tmp_path / case_name / "dotnet-root"
        dotnet_root.mkdir(parents=True)
        fake_dotnet = dotnet_root / "dotnet"
        fake_dotnet.write_text(
            f"""#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

{runtime_environment}EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
            encoding="utf-8",
        )
        fake_dotnet.chmod(0o755)
        env = os.environ.copy()
        env["DOTNET_ROOT"] = str(dotnet_root)

        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )

        assert completed.returncode == 1, case_name
        assert expected_error in completed.stderr, case_name
        assert expected_value in completed.stderr, case_name
        assert "SCCP .NET SDK RID:" not in completed.stdout, case_name
        assert "SCCP .NET SDK Architecture:" not in completed.stdout, case_name
        assert (
            "cargo build -p connect_norito_bridge" not in completed.stdout
        ), case_name
        assert (
            "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
        ), case_name
        assert (
            "dotnet test tests/Hyperledger.Iroha.Sdk.Tests"
            not in completed.stdout
        ), case_name


def test_sccp_production_corridor_dotnet_phase_rejects_uppercase_architecture(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must not normalize noncanonical architecture text."""

    dotnet_root = tmp_path / "dotnet-root"
    dotnet_root.mkdir()
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: X64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: X64
  RID:         win-x64
EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)

    completed = subprocess.run(
        ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    assert completed.returncode == 1
    assert "SCCP .NET SDK validation requires a canonical architecture" in (
        completed.stderr
    )
    assert "found: X64" in completed.stderr
    assert "SCCP .NET SDK Architecture:" not in completed.stdout
    assert "cargo build -p connect_norito_bridge" not in completed.stdout
    assert "dotnet restore Hyperledger.Iroha.Sdk.sln" not in completed.stdout
    assert "dotnet test tests/Hyperledger.Iroha.Sdk.Tests" not in completed.stdout


def test_sccp_production_corridor_dotnet_phase_rejects_nested_trx_path(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must fail if VSTest writes a nested TRX path."""

    tool_dir = tmp_path / "tools"
    dotnet_root = tmp_path / "dotnet-root"
    bridge_target_dir = tmp_path / "bridge-target"
    forged_trx_parent = (
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "codex-forged-nested-trx"
    )
    tool_dir.mkdir()
    dotnet_root.mkdir()
    fake_cargo = tool_dir / "cargo"
    fake_cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" != "build -p connect_norito_bridge" ]]; then
  printf 'unexpected fake cargo invocation: %s\\n' "$*" >&2
  exit 99
fi
mkdir -p "$CARGO_TARGET_DIR/debug"
printf 'fake bridge dll\\n' > "$CARGO_TARGET_DIR/debug/connect_norito_bridge.dll"
""",
        encoding="utf-8",
    )
    fake_cargo.chmod(0o755)
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
EOF
    ;;
  restore)
    exit 0
    ;;
  test)
    mkdir -p tests/Hyperledger.Iroha.Sdk.Tests/codex-forged-nested-trx/TestResults
    printf '<TestRun />\\n' > tests/Hyperledger.Iroha.Sdk.Tests/codex-forged-nested-trx/TestResults/sccp-dotnet-sdk.trx
    printf 'Passed!  - Failed: 0, Passed: 1, Skipped: 0, Total: 1, Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\\n'
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)
    env["SCCP_DOTNET_BRIDGE_TARGET_DIR"] = str(bridge_target_dir)
    env["PATH"] = f"{tool_dir}{os.pathsep}{env['PATH']}"

    try:
        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )
    finally:
        shutil.rmtree(forged_trx_parent, ignore_errors=True)

    assert completed.returncode == 1
    assert (
        "SCCP .NET SDK validation produced an unexpected TRX path"
        in completed.stderr
    )
    assert "codex-forged-nested-trx/TestResults/sccp-dotnet-sdk.trx" in (
        completed.stderr
    )
    assert "SCCP .NET SDK TRX:" not in completed.stdout
    assert "SCCP .NET SDK TRX bytes:" not in completed.stdout


def test_sccp_production_corridor_dotnet_phase_accepts_structured_trx(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must accept a real VSTest-shaped SCCP TRX."""

    tool_dir = tmp_path / "tools"
    dotnet_root = tmp_path / "dotnet-root"
    bridge_target_dir = tmp_path / "bridge-target"
    direct_trx_path = (
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "TestResults"
        / "sccp-dotnet-sdk.trx"
    )
    tool_dir.mkdir(parents=True)
    dotnet_root.mkdir()
    fake_cargo = tool_dir / "cargo"
    fake_cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" != "build -p connect_norito_bridge" ]]; then
  printf 'unexpected fake cargo invocation: %s\\n' "$*" >&2
  exit 99
fi
mkdir -p "$CARGO_TARGET_DIR/debug"
printf 'fake bridge dll\\n' > "$CARGO_TARGET_DIR/debug/connect_norito_bridge.dll"
""",
        encoding="utf-8",
    )
    fake_cargo.chmod(0o755)
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
EOF
    ;;
  restore)
    exit 0
    ;;
  test)
    mkdir -p tests/Hyperledger.Iroha.Sdk.Tests/TestResults
    cat > tests/Hyperledger.Iroha.Sdk.Tests/TestResults/sccp-dotnet-sdk.trx <<'EOF'
<TestRun xmlns="http://microsoft.com/schemas/VisualStudio/TeamTest/2010">
  <Results>
    <UnitTestResult executionId="exec-sccp" testId="test-sccp" testName="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests.BuildsProof" outcome="Passed" />
  </Results>
  <TestDefinitions>
    <UnitTest id="test-sccp" name="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests.BuildsProof" storage="C:\\repo\\csharp\\tests\\Hyperledger.Iroha.Sdk.Tests\\bin\\Debug\\net8.0\\Hyperledger.Iroha.Sdk.Tests.dll">
      <Execution id="exec-sccp" />
      <TestMethod codeBase="C:\\repo\\csharp\\tests\\Hyperledger.Iroha.Sdk.Tests\\bin\\Debug\\net8.0\\Hyperledger.Iroha.Sdk.Tests.dll" className="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests" name="BuildsProof" />
    </UnitTest>
  </TestDefinitions>
</TestRun>
EOF
    printf 'Passed!  - Failed: 0, Passed: 1, Skipped: 0, Total: 1, Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\\n'
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)
    env["SCCP_DOTNET_BRIDGE_TARGET_DIR"] = str(bridge_target_dir)
    env["PATH"] = f"{tool_dir}{os.pathsep}{env['PATH']}"

    try:
        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )
    finally:
        direct_trx_path.unlink(missing_ok=True)

    assert completed.returncode == 0, completed.stderr
    assert (
        "SCCP .NET SDK TRX: "
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/TestResults/sccp-dotnet-sdk.trx"
        in completed.stdout
    )
    assert "SCCP .NET SDK TRX bytes:" in completed.stdout


@pytest.mark.parametrize(
    ("case_name", "summary_output", "expected_error"),
    (
        (
            "missing-summary",
            "No VSTest summary was emitted.\n",
            "requires exactly one canonical VSTest summary",
        ),
        (
            "duplicate-summary",
            (
                "Passed! - Failed: 0, Passed: 1, Skipped: 0, Total: 1, "
                "Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\n"
                "Passed! - Failed: 0, Passed: 1, Skipped: 0, Total: 1, "
                "Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\n"
            ),
            "requires exactly one canonical VSTest summary",
        ),
        (
            "failed-summary",
            (
                "Passed! - Failed: 1, Passed: 1, Skipped: 0, Total: 2, "
                "Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\n"
            ),
            "requires exactly one canonical VSTest summary",
        ),
        (
            "skipped-summary",
            (
                "Passed! - Failed: 0, Passed: 1, Skipped: 1, Total: 2, "
                "Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\n"
            ),
            "requires exactly one canonical VSTest summary",
        ),
        (
            "zero-passed-summary",
            (
                "Passed! - Failed: 0, Passed: 0, Skipped: 0, Total: 0, "
                "Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\n"
            ),
            "requires exactly one canonical VSTest summary",
        ),
        (
            "wrong-total-summary",
            (
                "Passed! - Failed: 0, Passed: 1, Skipped: 0, Total: 2, "
                "Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\n"
            ),
            "requires VSTest summary to report Failed: 0, Skipped: 0, and Total == Passed",
        ),
        (
            "wrong-assembly-summary",
            (
                "Passed! - Failed: 0, Passed: 1, Skipped: 0, Total: 1, "
                "Duration: 1 ms - Other.Tests.dll (net8.0)\n"
            ),
            "requires exactly one canonical VSTest summary",
        ),
        (
            "tabbed-summary",
            (
                "Passed!\t- Failed: 0, Passed: 1, Skipped: 0, Total: 1, "
                "Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\n"
            ),
            "requires exactly one canonical VSTest summary",
        ),
        (
            "trx-count-mismatch",
            (
                "Passed! - Failed: 0, Passed: 2, Skipped: 0, Total: 2, "
                "Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\n"
            ),
            "requires TRX UnitTestResult count to match VSTest passed-test count",
        ),
    ),
)
def test_sccp_production_corridor_dotnet_phase_rejects_forged_vstest_summary(
    tmp_path: Path,
    case_name: str,
    summary_output: str,
    expected_error: str,
) -> None:
    """Windows .NET evidence must bind the VSTest summary to the TRX XML."""

    tool_dir = tmp_path / case_name / "tools"
    dotnet_root = tmp_path / case_name / "dotnet-root"
    bridge_target_dir = tmp_path / case_name / "bridge-target"
    direct_trx_path = (
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "TestResults"
        / "sccp-dotnet-sdk.trx"
    )
    tool_dir.mkdir(parents=True)
    dotnet_root.mkdir()
    fake_cargo = tool_dir / "cargo"
    fake_cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" != "build -p connect_norito_bridge" ]]; then
  printf 'unexpected fake cargo invocation: %s\\n' "$*" >&2
  exit 99
fi
mkdir -p "$CARGO_TARGET_DIR/debug"
printf 'fake bridge dll\\n' > "$CARGO_TARGET_DIR/debug/connect_norito_bridge.dll"
""",
        encoding="utf-8",
    )
    fake_cargo.chmod(0o755)
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
EOF
    ;;
  restore)
    exit 0
    ;;
  test)
    mkdir -p tests/Hyperledger.Iroha.Sdk.Tests/TestResults
    cat > tests/Hyperledger.Iroha.Sdk.Tests/TestResults/sccp-dotnet-sdk.trx <<'EOF'
<TestRun xmlns="http://microsoft.com/schemas/VisualStudio/TeamTest/2010">
  <Results>
    <UnitTestResult executionId="exec-sccp" testId="test-sccp" testName="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests.BuildsProof" outcome="Passed" />
  </Results>
  <TestDefinitions>
    <UnitTest id="test-sccp" name="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests.BuildsProof" storage="C:\\repo\\csharp\\tests\\Hyperledger.Iroha.Sdk.Tests\\bin\\Debug\\net8.0\\Hyperledger.Iroha.Sdk.Tests.dll">
      <Execution id="exec-sccp" />
      <TestMethod codeBase="C:\\repo\\csharp\\tests\\Hyperledger.Iroha.Sdk.Tests\\bin\\Debug\\net8.0\\Hyperledger.Iroha.Sdk.Tests.dll" className="Hyperledger.Iroha.Sdk.Tests.SccpEthereumMainnetTests" name="BuildsProof" />
    </UnitTest>
  </TestDefinitions>
</TestRun>
EOF
    cat <<'EOF'
{summary_output}EOF
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)
    env["SCCP_DOTNET_BRIDGE_TARGET_DIR"] = str(bridge_target_dir)
    env["PATH"] = f"{tool_dir}{os.pathsep}{env['PATH']}"

    try:
        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )
    finally:
        direct_trx_path.unlink(missing_ok=True)

    assert completed.returncode == 1, case_name
    assert expected_error in completed.stderr, case_name
    assert "SCCP .NET SDK TRX:" not in completed.stdout, case_name
    assert "SCCP .NET SDK TRX bytes:" not in completed.stdout, case_name


@pytest.mark.parametrize(
    ("case_name", "trx_payload", "expected_error"),
    (
        (
            "placeholder",
            "<TestRun />\n",
            "requires TRX result to name Hyperledger.Iroha.Sdk.Tests.dll",
        ),
        (
            "wrong-assembly",
            (
                '<TestRun><Results><UnitTestResult testName="SccpFake" outcome="Passed" />'
                '</Results><TestDefinitions><UnitTest><TestMethod '
                'codeBase="Other.Tests.dll" /></UnitTest></TestDefinitions></TestRun>\n'
            ),
            "requires TRX result to name Hyperledger.Iroha.Sdk.Tests.dll",
        ),
        (
            "non-vstest-root",
            (
                '<Envelope><Results><UnitTestResult testId="sccp-test" outcome="Passed" />'
                '</Results><TestDefinitions><UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest></TestDefinitions></Envelope>\n"
            ),
            "requires TRX root to be a VSTest TestRun",
        ),
        (
            "unit-result-outside-results",
            (
                '<TestRun><UnitTestResult testId="sccp-test" outcome="Passed" />'
                '<TestDefinitions><UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest></TestDefinitions></TestRun>\n"
            ),
            "requires every TRX UnitTestResult to appear directly under the VSTest Results section",
        ),
        (
            "unit-definition-outside-testdefinitions",
            (
                '<TestRun><Results><UnitTestResult testId="sccp-test" outcome="Passed" />'
                '</Results><UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest></TestRun>\n"
            ),
            "requires every TRX UnitTest definition to appear directly under the VSTest TestDefinitions section",
        ),
        (
            "no-passed-result",
            (
                '<TestRun><TestDefinitions><UnitTest><TestMethod '
                'codeBase="Hyperledger.Iroha.Sdk.Tests.dll" /></UnitTest>'
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX result to contain at least one passed SCCP test result",
        ),
        (
            "skipped-result",
            (
                '<TestRun><Results><UnitTestResult testName="SccpFake" outcome="NotExecuted" />'
                '</Results><TestDefinitions><UnitTest><TestMethod '
                'codeBase="Hyperledger.Iroha.Sdk.Tests.dll" /></UnitTest>'
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX result to contain no failed, skipped, timed-out, or aborted SCCP test results",
        ),
        (
            "failed-result",
            (
                '<TestRun><Results><UnitTestResult testName="SccpFake" outcome="Failed" />'
                '</Results><TestDefinitions><UnitTest><TestMethod '
                'codeBase="Hyperledger.Iroha.Sdk.Tests.dll" /></UnitTest>'
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX result to contain no failed, skipped, timed-out, or aborted SCCP test results",
        ),
        (
            "single-quoted-failed-result",
            (
                "<TestRun><Results><UnitTestResult testName='SccpFake' outcome='Failed' />"
                "</Results><TestDefinitions><UnitTest><TestMethod "
                "codeBase='Hyperledger.Iroha.Sdk.Tests.dll' /></UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX result to contain no failed, skipped, timed-out, or aborted SCCP test results",
        ),
        (
            "comment-spoofed-assembly",
            (
                '<TestRun><!-- <TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" /> -->'
                '<Results><UnitTestResult testName="SccpFake" outcome="Passed" />'
                "</Results></TestRun>\n"
            ),
            "requires TRX result to name Hyperledger.Iroha.Sdk.Tests.dll",
        ),
        (
            "arbitrary-attribute-spoofed-assembly",
            (
                '<TestRun><Fake codeBase="Hyperledger.Iroha.Sdk.Tests.dll" />'
                '<Results><UnitTestResult testName="SccpFake" outcome="Passed" />'
                "</Results></TestRun>\n"
            ),
            "requires TRX result to name Hyperledger.Iroha.Sdk.Tests.dll",
        ),
        (
            "non-sccp-passed-result",
            (
                '<TestRun><Results><UnitTestResult testName="OtherTests.Passes" outcome="Passed" />'
                '</Results><TestDefinitions><UnitTest><TestMethod '
                'codeBase="Hyperledger.Iroha.Sdk.Tests.dll" /></UnitTest>'
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "unbound-sccp-result",
            (
                '<TestRun><Results><UnitTestResult testName="SccpFake" outcome="Passed" />'
                '</Results><TestDefinitions><UnitTest id="other-test" name="OtherTests.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest></TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "wrong-assembly-sccp-definition",
            (
                '<TestRun><Results><UnitTestResult testId="sccp-test" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Other.Tests.dll" name="Passes" />'
                "</UnitTest>"
                '<UnitTest id="other-test" name="OtherTests.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "duplicate-unit-test-id",
            (
                '<TestRun><Results><UnitTestResult testId="dup-test" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="dup-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                '<UnitTest id="dup-test" name="SccpOther.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires unique TRX UnitTest id values",
        ),
        (
            "duplicate-execution-id",
            (
                '<TestRun><Results><UnitTestResult executionId="dup-exec" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-one" name="SccpFake.Passes">'
                '<Execution id="dup-exec" />'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                '<UnitTest id="sccp-two" name="SccpOther.Passes">'
                '<Execution id="dup-exec" />'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires unique TRX Execution id values",
        ),
        (
            "embedded-sccp-substring",
            (
                '<TestRun><Results><UnitTestResult testId="not-sccp-test" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="not-sccp-test" name="NotSccpEvidence.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" '
                'className="Hyperledger.Iroha.Sdk.Tests.NotSccpEvidence" '
                'name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "lowercase-sccp-token",
            (
                '<TestRun><Results><UnitTestResult testId="lowercase-sccp-test" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="lowercase-sccp-test" name="sccpEvidence.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" '
                'className="Hyperledger.Iroha.Sdk.Tests.sccpEvidence" '
                'name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "adapter-type-name-sccp-spoof",
            (
                '<TestRun><Results><UnitTestResult testId="adapter-spoof" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="adapter-spoof" name="OtherTests.Passes" adapterTypeName="SccpFake">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" '
                'className="Hyperledger.Iroha.Sdk.Tests.OtherTests" '
                'adapterTypeName="SccpFake" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "execution-id-drift",
            (
                '<TestRun><Results><UnitTestResult executionId="wrong-exec" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<Execution id="right-exec" />'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "test-id-valid-execution-id-forged",
            (
                '<TestRun><Results><UnitTestResult testId="sccp-test" executionId="wrong-exec" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<Execution id="right-exec" />'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "test-id-execution-id-cross-binding",
            (
                '<TestRun><Results><UnitTestResult testId="sccp-one" executionId="exec-two" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-one" name="SccpOne.Passes">'
                '<Execution id="exec-one" />'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                '<UnitTest id="sccp-two" name="SccpTwo.Passes">'
                '<Execution id="exec-two" />'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX UnitTestResult testId and executionId to bind the same SCCP test definition",
        ),
        (
            "test-id-result-name-mismatch",
            (
                '<TestRun><Results><UnitTestResult testId="sccp-test" '
                'testName="OtherTests.Passes" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" '
                'className="Hyperledger.Iroha.Sdk.Tests.SccpFake" '
                'name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX UnitTestResult testName to match its SCCP test definition",
        ),
        (
            "execution-id-result-name-mismatch",
            (
                '<TestRun><Results><UnitTestResult executionId="exec-sccp" '
                'testName="SccpOther.Passes" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<Execution id="exec-sccp" />'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" '
                'className="Hyperledger.Iroha.Sdk.Tests.SccpFake" '
                'name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX UnitTestResult testName to match its SCCP test definition",
        ),
        (
            "method-only-result-name",
            (
                '<TestRun><Results><UnitTestResult testId="sccp-test" '
                'testName="Passes" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" '
                'className="Hyperledger.Iroha.Sdk.Tests.SccpFake" '
                'name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX UnitTestResult testName to match its SCCP test definition",
        ),
        (
            "padded-result-name",
            (
                '<TestRun><Results><UnitTestResult testId="sccp-test" '
                'testName="SccpFake.Passes " outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes ">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" '
                'className="Hyperledger.Iroha.Sdk.Tests.SccpFake" '
                'name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires TRX UnitTestResult testName to match its SCCP test definition",
        ),
        (
            "definition-trailing-space-token",
            (
                '<TestRun><Results><UnitTestResult testId="sccp-test" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes ">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" '
                'className="Hyperledger.Iroha.Sdk.Tests.OtherTests" '
                'name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "mixed-sccp-and-non-sccp-results",
            (
                '<TestRun><Results>'
                '<UnitTestResult testId="sccp-test" outcome="Passed" />'
                '<UnitTestResult testId="other-test" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                '<UnitTest id="other-test" name="OtherTests.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "mixed-sccp-and-unmapped-results",
            (
                '<TestRun><Results>'
                '<UnitTestResult executionId="exec-sccp" outcome="Passed" />'
                '<UnitTestResult executionId="exec-forged" outcome="Passed" />'
                '</Results><TestDefinitions>'
                '<UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<Execution id="exec-sccp" />'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest>"
                "</TestDefinitions></TestRun>\n"
            ),
            "requires every TRX test result to bind to a Hyperledger.Iroha.Sdk.Tests.dll SCCP test definition",
        ),
        (
            "doctype-declaration",
            (
                '<!DOCTYPE TestRun [<!ELEMENT TestRun ANY>]>'
                '<TestRun><Results><UnitTestResult testId="sccp-test" outcome="Passed" />'
                '</Results><TestDefinitions><UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="Hyperledger.Iroha.Sdk.Tests.dll" name="Passes" />'
                "</UnitTest></TestDefinitions></TestRun>\n"
            ),
            "requires TRX result to contain no DTD or entity declarations",
        ),
        (
            "entity-declaration",
            (
                '<!DOCTYPE TestRun [<!ENTITY forged "Hyperledger.Iroha.Sdk.Tests.dll">]>'
                '<TestRun><Results><UnitTestResult testId="sccp-test" outcome="Passed" />'
                '</Results><TestDefinitions><UnitTest id="sccp-test" name="SccpFake.Passes">'
                '<TestMethod codeBase="&forged;" name="Passes" />'
                "</UnitTest></TestDefinitions></TestRun>\n"
            ),
            "requires TRX result to contain no DTD or entity declarations",
        ),
        (
            "malformed-xml",
            '<TestRun><Results><UnitTestResult testName="SccpFake" outcome="Passed" /></Results>\n',
            "requires TRX result to be well-formed XML",
        ),
    ),
)
def test_sccp_production_corridor_dotnet_phase_rejects_malformed_trx_content(
    tmp_path: Path,
    case_name: str,
    trx_payload: str,
    expected_error: str,
) -> None:
    """Windows .NET evidence must inspect direct TRX content before publishing."""

    tool_dir = tmp_path / case_name / "tools"
    dotnet_root = tmp_path / case_name / "dotnet-root"
    bridge_target_dir = tmp_path / case_name / "bridge-target"
    direct_trx_path = (
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "TestResults"
        / "sccp-dotnet-sdk.trx"
    )
    tool_dir.mkdir(parents=True)
    dotnet_root.mkdir()
    fake_cargo = tool_dir / "cargo"
    fake_cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" != "build -p connect_norito_bridge" ]]; then
  printf 'unexpected fake cargo invocation: %s\\n' "$*" >&2
  exit 99
fi
mkdir -p "$CARGO_TARGET_DIR/debug"
printf 'fake bridge dll\\n' > "$CARGO_TARGET_DIR/debug/connect_norito_bridge.dll"
""",
        encoding="utf-8",
    )
    fake_cargo.chmod(0o755)
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
EOF
    ;;
  restore)
    exit 0
    ;;
  test)
    mkdir -p tests/Hyperledger.Iroha.Sdk.Tests/TestResults
    cat > tests/Hyperledger.Iroha.Sdk.Tests/TestResults/sccp-dotnet-sdk.trx <<'EOF'
{trx_payload}EOF
    printf 'Passed!  - Failed: 0, Passed: 1, Skipped: 0, Total: 1, Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\\n'
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)
    env["SCCP_DOTNET_BRIDGE_TARGET_DIR"] = str(bridge_target_dir)
    env["PATH"] = f"{tool_dir}{os.pathsep}{env['PATH']}"

    try:
        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )
    finally:
        direct_trx_path.unlink(missing_ok=True)

    assert completed.returncode == 1, case_name
    assert expected_error in completed.stderr, case_name
    assert str(direct_trx_path) in completed.stderr, case_name
    assert "SCCP .NET SDK TRX:" not in completed.stdout, case_name
    assert "SCCP .NET SDK TRX bytes:" not in completed.stdout, case_name


def test_sccp_production_corridor_dotnet_phase_rejects_oversized_trx_before_xml_parse(
    tmp_path: Path,
) -> None:
    """Windows .NET evidence must cap TRX size before XML parsing."""

    tool_dir = tmp_path / "oversized-trx" / "tools"
    dotnet_root = tmp_path / "oversized-trx" / "dotnet-root"
    bridge_target_dir = tmp_path / "oversized-trx" / "bridge-target"
    direct_trx_path = (
        ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "TestResults"
        / "sccp-dotnet-sdk.trx"
    )
    tool_dir.mkdir(parents=True)
    dotnet_root.mkdir()
    fake_cargo = tool_dir / "cargo"
    fake_cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" != "build -p connect_norito_bridge" ]]; then
  printf 'unexpected fake cargo invocation: %s\\n' "$*" >&2
  exit 99
fi
mkdir -p "$CARGO_TARGET_DIR/debug"
printf 'fake bridge dll\\n' > "$CARGO_TARGET_DIR/debug/connect_norito_bridge.dll"
""",
        encoding="utf-8",
    )
    fake_cargo.chmod(0o755)
    fake_dotnet = dotnet_root / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  --version)
    printf '8.0.101\\n'
    ;;
  --info)
    cat <<'EOF'
.NET SDK:
 Version:           8.0.101

Host:
  Version:      8.0.1
  Architecture: x64

Runtime Environment:
  OS Name:     Windows
  OS Platform: Windows
  OS Architecture: x64
  RID:         win-x64
EOF
    ;;
  restore)
    exit 0
    ;;
  test)
    mkdir -p tests/Hyperledger.Iroha.Sdk.Tests/TestResults
    python3 - <<'PY'
from pathlib import Path
Path("tests/Hyperledger.Iroha.Sdk.Tests/TestResults/sccp-dotnet-sdk.trx").write_bytes(
    b"A" * 16777217
)
PY
    printf 'Passed!  - Failed: 0, Passed: 1, Skipped: 0, Total: 1, Duration: 1 ms - Hyperledger.Iroha.Sdk.Tests.dll (net8.0)\\n'
    ;;
  *)
    printf 'unexpected fake dotnet invocation: %s\\n' "$*" >&2
    exit 99
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)
    env = os.environ.copy()
    env["DOTNET_ROOT"] = str(dotnet_root)
    env["SCCP_DOTNET_BRIDGE_TARGET_DIR"] = str(bridge_target_dir)
    env["PATH"] = f"{tool_dir}{os.pathsep}{env['PATH']}"

    try:
        completed = subprocess.run(
            ["bash", str(SCRIPT), "--phase", "dotnet-sdk"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )
    finally:
        direct_trx_path.unlink(missing_ok=True)

    assert completed.returncode == 1
    assert "requires TRX result to be at most 16777216 bytes before XML parsing" in (
        completed.stderr
    )
    assert "requires TRX result to be well-formed XML" not in completed.stderr
    assert "SCCP .NET SDK TRX:" not in completed.stdout
    assert "SCCP .NET SDK TRX bytes:" not in completed.stdout


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
