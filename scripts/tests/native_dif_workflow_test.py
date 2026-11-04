"""Exercise native component routing and strict receipts without qualification claims."""

from __future__ import annotations

import json
import os
from pathlib import Path
import re
import subprocess
import sys
import textwrap

import pytest


ROOT = Path(__file__).resolve().parents[2]
OWNER = "privacy_engines::zk_x509::stark::main_aggregate::main_transform::"
SCALAR_OWNER = OWNER + "full_dispatch_tests::full_native_"
SCALAR_NAMES = [SCALAR_OWNER + suffix for suffix in (
    "admission_is_feature_and_architecture_bound",
    "feature_off_measured_batch_executes_scalar_and_clears",
    "gate_requires_every_complete_registered_masked_length",
)]
KERNEL = (
    "privacy_engines::transparent_stark::avx2_test_kernel::dif::"
    "required_avx2_dif_windows_match_independent_modular_arithmetic"
)
NATIVE_NAMES = [KERNEL] + [
    OWNER + "selected_coset::full_tests::avx2_dif::required_avx2_dif_" + suffix
    for suffix in (
        "actual_full_batch_dispatch_keeps_original_outputs",
        "keeps_allocation_erasure_and_original_fallback",
        "owner_matches_scalar_dense_and_refusal_boundaries",
        "registered_common22_outputs_fit_original_funding",
    )
]


def test_native_component_python_tools_preserve_hashed_test_delivery() -> None:
    """Component controls retain all tests and use the existing pure-Python pins."""

    workflow = (ROOT / ".github/workflows/platform_native.yml").read_text()
    preparation = workflow.split("      - name: Prepare retained Python source delivery fixtures\n", 1)[1]
    preparation = preparation.split("      - name:", 1)[0]
    assert "        id: source_delivery_ready\n" in preparation
    assert "        if:" not in preparation
    assert textwrap.dedent(preparation.split("        run: |\n", 1)[1]).splitlines() == [
        "set -euo pipefail",
        "cargo test --locked -p iroha_native_source_delivery --features dev-tools 2>&1 | tee target/platform-native/source-delivery-rust.log",
        "cargo clippy --locked -p iroha_native_source_delivery --features dev-tools --all-targets -- -D warnings 2>&1 | tee target/platform-native/source-delivery-clippy.log",
        "cargo build --locked -p iroha_native_source_delivery --features dev-tools",
        "python -m pip install --require-hashes --only-binary=:all: -r ci/requirements-native-components.lock",
    ]
    step = workflow.split("      - name: Exercise retained Python source delivery\n", 1)[1]
    step = step.split("      - name:", 1)[0]
    assert "        if:" not in step
    assert preparation.count(
        "python -m pip install --require-hashes --only-binary=:all: "
        "-r ci/requirements-native-components.lock"
    ) == 1
    assert "          set -euo pipefail\n" in step
    assert "export IROHA_NATIVE_SOURCE_DELIVERY_TEST_HELPER=" in step
    assert "2>&1 | tee target/platform-native/source-delivery-python.log" in step
    assert '      - "ci/requirements-native-components.*"' in workflow
    pytest_command = step.split("python -m pytest -q", 1)[1]
    assert re.findall(r"(?:scripts/tests|pytests/scripts)/[a-z_]+\.py", pytest_command) == [
        "scripts/tests/python_native_source_delivery_test.py",
        "scripts/tests/python_native_delivery_gate_contract_test.py",
        "scripts/tests/python_wheel_byte_owner_test.py",
        "scripts/tests/python_source_owner_admission_test.py",
        "scripts/tests/sorafs_python_native_platform_test.py",
        "pytests/scripts/workspace_source_manifest_test.py",
        "pytests/scripts/workspace_source_keeper_test.py",
    ]
    component = (ROOT / "ci/requirements-native-components.lock").read_text()
    sdk = (ROOT / "python/iroha_python/requirements-ci.lock").read_text()
    names = ["colorama", "iniconfig", "packaging", "pluggy", "pygments", "pytest"]
    assert re.findall(r"(?m)^([a-z][a-z0-9-]*)==", component) == names
    assert not re.search(r"(?m)^--", component)
    for name in names:
        entries = []
        for owner in (component, sdk):
            match = re.search(
                rf"(?m)^{name}==([^ ;\\\n]+)[^\n]*\n((?:[ \t]+[^\n]*(?:\n|$))*)",
                owner,
            )
            assert match is not None
            hashes = re.findall(r"--hash=sha256:([0-9a-f]{64})", match[2])
            assert hashes
            entries.append((match[1], sorted(hashes)))
        assert entries[0] == entries[1], name


@pytest.mark.parametrize("cancelled", [False, True])
@pytest.mark.parametrize("runner_os", ["Linux", "Windows", "macOS"])
@pytest.mark.parametrize("preparation_outcome", ["success", "failure", "skipped", "cancelled", ""])
@pytest.mark.parametrize("delivery_failed", [False, True])
def test_producer_routing_requires_preparation_and_preserves_cancellation(
    cancelled: bool, runner_os: str, preparation_outcome: str, delivery_failed: bool,
) -> None:
    """Evaluate the maintained condition with actual Actions status-function rules."""
    workflow = (ROOT / ".github/workflows/platform_native.yml").read_text()
    native = workflow.split("  native:\n", 1)[1].split("  workflow-storage:\n", 1)[0]
    assert "continue-on-error:" not in native
    step = native.split("      - name: Exercise producer admission and synchronous original-session supervision\n", 1)[1]
    step = step.split("      - name:", 1)[0]
    condition = re.search(r"(?m)^        if: \$\{\{ (.+) \}\}$", step)[1]
    expression = condition.replace("runner.os", repr(runner_os)).replace(
        "steps.source_delivery_ready.outcome", repr(preparation_outcome),
    )
    status_functions = {
        "cancelled()": cancelled,
        "success()": not cancelled and not delivery_failed,
        "failure()": delivery_failed,
        "always()": True,
    }
    has_status_function = any(name in expression for name in status_functions)
    for name, value in status_functions.items():
        expression = expression.replace(name, repr(value))
    expression = re.sub(r"!(?!=)", "not ", expression).replace("&&", "and").replace("||", "or")
    selected = eval(expression, {"__builtins__": {}}, {})
    if not has_status_function:
        selected = selected and status_functions["success()"]
    assert selected == (not cancelled and runner_os != "macOS" and preparation_outcome == "success")
    upload = native.split("      - name: Retain actual native results\n", 1)[1]
    assert "        if: always()\n" in upload
    assert "          path: target/platform-native\n" in upload
    assert "          if-no-files-found: error\n" in upload


@pytest.mark.parametrize("pytest_status,case,expected_status", [
    (0, "pass", 0), (17, "pass", 17), (0, "skipped", 1),
    (0, "error", 1), (0, "failure", 1), (0, "empty", 1),
])
def test_producer_script_retains_failure_and_nonempty_no_skip_gate(
    tmp_path: Path, pytest_status: int, case: str, expected_status: int,
) -> None:
    """Run the actual shell/JUnit gate while replacing only the costly pytest process."""
    workflow = (ROOT / ".github/workflows/platform_native.yml").read_text()
    step = workflow.split("      - name: Exercise producer admission and synchronous original-session supervision\n", 1)[1]
    step = step.split("      - name:", 1)[0]
    command = textwrap.dedent(step.split("        run: |\n", 1)[1])
    binary_root = tmp_path / "bin"
    binary_root.mkdir()
    python = binary_root / "python"
    python.write_text(f"#!{sys.executable}\n" + textwrap.dedent("""\
        import json, os, sys
        from pathlib import Path

        if sys.argv[1:3] == ["-m", "pytest"]:
            Path(os.environ["PYTEST_CALL"]).write_text(json.dumps(sys.argv[1:]))
            case = os.environ["JUNIT_CASE"]
            child = "" if case == "pass" else f"<{case}/>"
            body = "" if case == "empty" else f"<testcase name='actual-fixture'>{child}</testcase>"
            Path("target/platform-native/release-process-python.xml").write_text(f"<testsuite>{body}</testsuite>")
            print("retained producer pytest output", flush=True)
            sys.exit(int(os.environ["PYTEST_STATUS"]))
        os.execv(sys.executable, [sys.executable, *sys.argv[1:]])
        """))
    python.chmod(0o700)
    logs = tmp_path / "target/platform-native"
    logs.mkdir(parents=True)
    call = tmp_path / "pytest-call.json"
    result = subprocess.run(
        ["bash", "-e", "-o", "pipefail", "-c", command], cwd=tmp_path,
        env={**os.environ, "PATH": str(binary_root) + os.pathsep + os.environ["PATH"],
             "PYTEST_CALL": str(call), "JUNIT_CASE": case, "PYTEST_STATUS": str(pytest_status)},
        text=True, capture_output=True, check=False,
    )
    assert result.returncode == expected_status, result.stdout + result.stderr
    assert json.loads(call.read_text()) == [
        "-m", "pytest", "-q", "--junitxml=target/platform-native/release-process-python.xml",
        *["pytests/scripts/" + name for name in (
            "test_release_entry_owner.py", "test_release_bridge_admission.py",
            "test_release_process_bridge.py", "test_native_supervision.py",
            "test_pipeline_preparation.py", "test_shell_preparation.py",
            "test_bridge_cli_supervision.py",
        )],
    ]
    assert (logs / "release-process-python.log").read_text() == "retained producer pytest output\n"


def test_intel_and_windows_native_jobs_have_the_two_build_timeout_allowance() -> None:
    """Intel's cold variants receive four hours; Windows and Linux retain their limits."""

    workflow = (ROOT / ".github/workflows/platform_native.yml").read_text()
    job = workflow.split("  native:\n", 1)[1].split("    defaults:\n", 1)[0]
    assert job.count("timeout-minutes:") == 1
    assert (
        "    timeout-minutes: ${{ matrix.target == 'x86_64-apple-darwin' && 240 || "
        "matrix.target == 'x86_64-pc-windows-msvc' && 120 || 60 }}\n"
        in job
    )
    assert re.findall(r"- runner: (\S+)\n\s+target: (\S+)", job) == [
        ("ubuntu-latest", "x86_64-unknown-linux-gnu"),
        ("macos-15-intel", "x86_64-apple-darwin"),
        ("windows-latest", "x86_64-pc-windows-msvc"),
    ]
    assert "      fail-fast: false\n" in job


@pytest.mark.parametrize(
    "mutation",
    ["none", "broad-selector", "missing-control", "unexpected-control", "wrong-refusal", "wrong-exit"],
)
def test_native_dif_workflow_selects_its_owner_and_requires_exact_receipts(
    tmp_path: Path, mutation: str,
) -> None:
    """Execute the workflow script with libtest-shaped outputs, including name collisions."""

    workflow = (ROOT / ".github/workflows/platform_native.yml").read_text()
    step = workflow.split("      - name: Require production DIF execution and feature-off refusal\n", 1)[1]
    step = step.split("      - name:", 1)[0]
    command = textwrap.dedent(step.split("        run: |\n", 1)[1])
    if mutation == "broad-selector":
        command = command.replace("--lib " + SCALAR_OWNER + " --", "--lib full_native_ --")

    ordinary = SCALAR_NAMES.copy()
    if mutation == "missing-control":
        ordinary.pop()
    elif mutation == "unexpected-control":
        ordinary.append(SCALAR_OWNER + "unexpected")
    # Real tests outside this owner share the short filter. The generic proof
    # qualification remains ignored here and cannot be counted as a scalar control.
    collisions = [
        ["execution_proofs::proof::tests::execution_queries_match_full_native_race_fixed_lde", False],
        ["execution_proofs::proof::tests::late_finisher_full_native_proof_verifies_and_wrong_profile_is_rejected", True],
    ]
    fixture = tmp_path / "fixture.json"
    fixture.write_text(json.dumps({
        "native": NATIVE_NAMES,
        "scalar": [[name, False] for name in ordinary] + collisions,
        "mutation": mutation,
    }))
    binary_root = tmp_path / "bin"
    binary_root.mkdir()
    cargo = binary_root / "cargo"
    cargo.write_text(f"#!{sys.executable}\n" + textwrap.dedent("""\
        import json, os, sys
        from pathlib import Path

        args = sys.argv[1:]
        with open(os.environ["FIXTURE_CALLS"], "a") as stream:
            stream.write(json.dumps(args) + "\\n")
        fixture = json.loads(Path(os.environ["FIXTURE_INPUT"]).read_text())
        selector = args[args.index("--lib") + 1]
        feature_off = "--no-default-features" in args
        refusal = feature_off and "--ignored" in args
        pool = fixture["scalar"] if feature_off and not refusal else [
            [name, False] for name in fixture["native"]
        ]
        selected = [(name, ignored) for name, ignored in pool if selector in name]
        ignored = sum(skip for _, skip in selected)
        for name, skip in selected:
            print(f"test {name} ... ", end="")
            if refusal:
                message = "simd and actual AVX2 are mandatory" if name == fixture["native"][0] else "simd and actual AVX2 required"
                if fixture["mutation"] == "wrong-refusal":
                    message = "unrelated failure"
                print("\\n" + message + "\\nFAILED")
            else:
                print("ignored, expensive independent qualification" if skip else "ok")
        passed = 0 if refusal else len(selected) - ignored
        failed = len(selected) if refusal else 0
        result = "FAILED" if refusal else "ok"
        print(f"test result: {result}. {passed} passed; {failed} failed; {ignored} ignored;")
        sys.exit((1 if fixture["mutation"] == "wrong-exit" else 101) if refusal else 0)
        """))
    cargo.chmod(0o700)
    (binary_root / "python").symlink_to(sys.executable)
    logs = tmp_path / "target/platform-native"
    logs.mkdir(parents=True)
    calls = tmp_path / "calls.jsonl"
    result = subprocess.run(
        ["bash", "-e", "-o", "pipefail", "-c", command],
        cwd=tmp_path,
        env={
            **os.environ,
            "PATH": str(binary_root) + os.pathsep + os.environ["PATH"],
            "FIXTURE_INPUT": str(fixture),
            "FIXTURE_CALLS": str(calls),
        },
        text=True, capture_output=True, check=False,
    )
    receipt = json.loads((logs / "dif-result.json").read_text())
    assert (result.returncode == 0) == (mutation == "none"), result.stderr
    assert receipt["status"] == ("PASS" if mutation == "none" else "REFUSED")
    assert receipt["expected_native_refusal_admitted"] == (mutation == "none")
    invocations = [json.loads(line) for line in calls.read_text().splitlines()]
    assert len(invocations) == 3
    assert all("--locked" in args for args in invocations)
    assert "--no-default-features" not in invocations[0]
    assert "--no-default-features" in invocations[1]
    assert "--ignored" not in invocations[1]
    assert "--no-default-features" in invocations[2] and "--ignored" in invocations[2]
    if mutation == "none":
        assert receipt["default_native_passed"] == 5
        assert receipt["feature_off_scalar_passed"] == 3
        assert receipt["feature_off_native_failed"] == 5
        assert receipt["feature_off_native_exit_status"] == 101
