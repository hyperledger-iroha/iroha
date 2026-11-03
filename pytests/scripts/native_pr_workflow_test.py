"""Guard native PR validation against deleted paths and silently lost coverage."""

from pathlib import Path
import os
import subprocess
import tomllib

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = ROOT / ".github/workflows/pr.yml"
WORKFLOW = yaml.load(WORKFLOW_PATH.read_text(), Loader=yaml.BaseLoader)
JOBS = WORKFLOW["jobs"]


def commands(job):
    """Return the shell commands a parsed workflow job executes."""
    return "\n".join(step.get("run", "") for step in job["steps"])


def test_native_pr_jobs_have_hosted_runners_and_no_retired_artifacts():
    source = WORKFLOW_PATH.read_text()
    for retired in ("wasm", "iroha2-ci", "self-hosted", "scripts/test_env.py"):
        assert retired not in source
    assert "paths-ignore" in WORKFLOW["on"]["pull_request"]
    assert "paths_ignore" not in WORKFLOW["on"]["pull_request"]


def test_native_build_names_are_real_workspace_targets():
    build = commands(JOBS["pre_build"])
    for crate, binary in (
        ("irohad", "iroha3d"),
        ("iroha_cli", "iroha"),
        ("iroha_kagami", "kagami"),
    ):
        manifest = tomllib.loads((ROOT / "crates" / crate / "Cargo.toml").read_text())
        assert binary in {target["name"] for target in manifest["bin"]}
        assert f"--bin {binary}" in build
        assert f"target/release/{binary}" in build
    assert "--locked" in build


@pytest.mark.parametrize(
    "job",
    [
        name
        for name, config in JOBS.items()
        if any(
            step.get("uses") == "./.github/actions/setup-native-ci"
            for step in config["steps"]
        )
    ],
)
def test_native_builds_fetch_base_history_for_norito_bindings_guard(job):
    """Keep the base ref and merge ancestry available to the build-time guard."""
    steps = JOBS[job]["steps"]
    checkout_index = next(
        index
        for index, step in enumerate(steps)
        if step.get("uses", "").startswith("actions/checkout@")
    )
    setup_index = next(
        index
        for index, step in enumerate(steps)
        if step.get("uses") == "./.github/actions/setup-native-ci"
    )
    assert checkout_index < setup_index
    assert steps[checkout_index].get("with", {}).get("fetch-depth") == "0"


def test_xtask_fixture_verification_does_not_require_an_absent_cargo_alias():
    command = commands(JOBS["norito_rpc_verify"])
    assert "cargo run --locked -p xtask --bin xtask -- norito-rpc-verify" in command
    assert (ROOT / "xtask/src/main.rs").is_file()


def test_workspace_branch_and_documentation_coverage_are_preserved():
    command = commands(JOBS["test"])
    assert "llvm-cov nextest --workspace --locked --branch --no-report" in command
    assert "llvm-cov --workspace --locked --doc --branch --no-report" in command
    assert "llvm-cov report --doctests --branch" in command
    assert command.count("cargo +nightly-2026-10-23 llvm-cov") == 3
    setup = next(
        step
        for step in JOBS["test"]["steps"]
        if step.get("uses") == "./.github/actions/setup-native-ci"
    )
    assert setup["with"]["toolchain"] == "nightly-2026-10-23"


@pytest.mark.parametrize(
    "job", ["test", "nexus_cross_dataspace_localnet", "nexus_cross_lane_proofs"]
)
def test_network_gates_receive_current_native_binaries(job):
    config = JOBS[job]
    assert config["needs"] == "pre_build"
    assert config["env"]["TEST_NETWORK_BIN_IROHAD"].endswith("/bins/iroha3d")
    assert config["env"]["TEST_NETWORK_BIN_IROHA"].endswith("/bins/iroha")
    assert config["env"]["KAGAMI_BIN"].endswith("/bins/kagami")
    assert config["env"]["IROHA_FAIL_ON_SANDBOX_SKIP"] == "1"
    assert any(step.get("with", {}).get("name") == "bins" for step in config["steps"])


def test_python_job_runs_nonempty_existing_suites():
    command = commands(JOBS["pytests"])
    for suite in (
        "python/norito_py/tests",
        "python/iroha_torii_client/tests",
        "pytests/scripts",
    ):
        assert f"python -m pytest -q {suite}" in command
        assert any(
            "def test_" in path.read_text() for path in (ROOT / suite).glob("*.py")
        )


def test_shared_native_setup_has_explicit_toolchains_without_a_registry():
    action = yaml.load(
        (ROOT / ".github/actions/setup-native-ci/action.yml").read_text(),
        Loader=yaml.BaseLoader,
    )
    steps = action["runs"]["steps"]
    for step in steps:
        if "uses" in step:
            assert len(step["uses"].split("@")[1]) == 40
    assert any(step.get("with", {}).get("python-version") == "3.12" for step in steps)
    assert any(step.get("with", {}).get("java-version") == "21" for step in steps)
    assert any(step.get("with", {}).get("node-version") == "22.23.4" for step in steps)
    rust = next(
        step for step in steps if step.get("uses", "").startswith("actions-rust-lang/")
    )
    assert rust["with"]["toolchain"] == "${{ inputs.toolchain }}"
    assert action["inputs"]["toolchain"]["default"] == ""
    assert "container" not in action


def test_apalache_runs_the_model_instead_of_an_unsupported_version_probe(tmp_path):
    record = tmp_path / "arguments.txt"
    executable = tmp_path / "apalache-mc"
    executable.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$APALACHE_TEST_RECORD"\n')
    executable.chmod(0o755)
    env = dict(
        os.environ, APALACHE_BIN=str(executable), APALACHE_TEST_RECORD=str(record)
    )
    subprocess.run(
        ["bash", "scripts/formal/sumeragi_apalache.sh", "fast"],
        cwd=ROOT,
        env=env,
        check=True,
    )
    args = record.read_text().splitlines()
    assert args.count("check") == 1
    assert "--length=10" in args
    assert any(arg.endswith("Sumeragi_fast.cfg") for arg in args)
    assert args[-1].endswith("Sumeragi.tla")
    assert "--version" not in commands(JOBS["sumeragi_formal"])
    assert "bash ci/check_sumeragi_formal.sh" in commands(JOBS["sumeragi_formal"])


@pytest.mark.parametrize(
    ("script", "expected_calls"),
    [
        ("ci/check_nexus_cross_lane_proofs.sh", 4),
        ("scripts/run_nexus_cross_dataspace_atomic_swap.sh", 1),
    ],
)
def test_network_scripts_keep_locked_dependency_resolution(
    tmp_path, script, expected_calls
):
    record = tmp_path / "cargo.txt"
    executable = tmp_path / "cargo"
    executable.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$CARGO_TEST_RECORD"\n')
    executable.chmod(0o755)
    env = dict(
        os.environ,
        PATH=f"{tmp_path}{os.pathsep}{os.environ['PATH']}",
        CARGO_TEST_RECORD=str(record),
        IROHA_TEST_NETWORK_PERMIT_DIR=str(tmp_path / "permits"),
    )
    subprocess.run(["bash", script], cwd=ROOT, env=env, check=True)
    calls = record.read_text().splitlines()
    assert len(calls) == expected_calls
    assert all(call.startswith("test --locked -p ") for call in calls)
