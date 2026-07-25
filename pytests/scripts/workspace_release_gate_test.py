"""Static and adversarial tests for the exact-SHA workspace release gate."""

from __future__ import annotations

import re
from collections.abc import Callable
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "workspace_release.yml"
PR_WORKFLOW = ROOT / ".github" / "workflows" / "pr.yml"
PINNED_RUST = "1.93.1"
SETUP_RUST_ACTION = (
    "actions-rust-lang/setup-rust-toolchain@"
    "166cdcfd11aee3cb47222f9ddb555ce30ddb9659"
)
RUST_CACHE_ACTION = "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32"


def _job_block(workflow: str, name: str) -> str:
    """Return one top-level job block from a GitHub Actions workflow."""

    try:
        jobs = workflow.split("\njobs:\n", 1)[1]
    except IndexError:
        return ""
    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)",
        jobs,
    )
    return "" if match is None else match.group(0)


def _normalized(text: str) -> str:
    """Collapse YAML presentation whitespace for command assertions."""

    return " ".join(text.split())


def _replace_once(text: str, old: str, new: str) -> str:
    """Apply one deliberate mutation and fail if the fixture drifted."""

    assert text.count(old) >= 1, f"mutation source is absent: {old!r}"
    return text.replace(old, new, 1)


def _validate_release_workflow(workflow: str) -> list[str]:
    """Return deterministic errors for weakened release-workflow semantics."""

    errors: list[str] = []
    global_requirements = (
        (
            '  push:\n    branches: [main]\n    tags: ["v*"]',
            "release workflow must run on main and v* pushes",
        ),
        ("  workflow_dispatch:\n", "release workflow must support manual runs"),
        ("  workflow_call:\n", "release workflow must support reusable calls"),
        ("permissions:\n  contents: read", "release workflow must be read-only"),
        (
            "group: workspace-release-${{ github.sha }}",
            "release concurrency must be scoped to the exact SHA",
        ),
        (
            "cancel-in-progress: false",
            "release evidence must not be cancelled by ref movement",
        ),
        (
            f"RUSTUP_TOOLCHAIN: {PINNED_RUST}",
            "release workflow must advertise the pinned Rust toolchain",
        ),
    )
    for marker, message in global_requirements:
        if marker not in workflow:
            errors.append(message)

    if re.search(r"(?m)^  pull_request:", workflow):
        errors.append("release workflow must not substitute PR state for release evidence")

    expected_runners = {
        "format": "runs-on: ubuntu-latest",
        "build": "runs-on: [self-hosted, Linux, iroha2]",
        "test": "runs-on: [self-hosted, Linux, iroha2]",
        "clippy": "runs-on: [self-hosted, Linux, iroha2]",
    }
    commands = {
        "format": (
            "python3 -m pytest -q pytests/scripts/workspace_release_gate_test.py",
            "cargo metadata --locked --no-deps --format-version 1 > /dev/null",
            "cargo fmt --all -- --check",
        ),
        "build": ("cargo build --locked --workspace",),
        "test": ("cargo test --locked --workspace --no-fail-fast",),
        "clippy": (
            "cargo clippy --locked --workspace --all-targets --all-features -- -D warnings",
        ),
    }
    exact_source_markers = (
        "persist-credentials: false",
        "ref: ${{ github.sha }}",
        "EXPECTED_SHA: ${{ github.sha }}",
        "WORKFLOW_SHA: ${{ github.workflow_sha }}",
        'source_sha="$(git rev-parse "${EXPECTED_SHA}^{commit}")"',
        'test "$(git rev-parse HEAD)" = "$source_sha"',
        'test "$WORKFLOW_SHA" = "$source_sha"',
    )

    for job_name, runner in expected_runners.items():
        job = _job_block(workflow, job_name)
        if not job:
            errors.append(f"release workflow is missing the {job_name} job")
            continue
        if runner not in job:
            errors.append(f"{job_name} must use its production CI runner class")
        for marker in exact_source_markers:
            if marker not in job:
                errors.append(f"{job_name} must verify the exact workflow SHA: {marker}")
        if f"uses: {SETUP_RUST_ACTION}" not in job:
            errors.append(f"{job_name} must install the managed Rust toolchain")
        if f"toolchain: {PINNED_RUST}" not in job:
            errors.append(f"{job_name} must pin Rust {PINNED_RUST}")

        normalized_job = _normalized(job)
        for command in commands[job_name]:
            if command not in normalized_job:
                errors.append(f"{job_name} is missing required command: {command}")

    return errors


def _validate_pr_parity(workflow: str) -> list[str]:
    """Return errors when PR testing is weaker than the release corridor."""

    errors: list[str] = []
    docs_job = _job_block(workflow, "kotodama_docs")
    if (
        "python3 -m pytest -q pytests/scripts/workspace_release_gate_test.py"
        not in _normalized(docs_job)
    ):
        errors.append("PR workflow must execute the workspace release semantics guard")

    test_job = _job_block(workflow, "test")
    if not test_job:
        errors.append("PR workflow is missing the full test job")
    elif (
        "mold --run cargo llvm-cov nextest --workspace --locked --branch --no-report"
        not in _normalized(test_job)
    ):
        errors.append("PR full test must explicitly select the locked workspace")

    numeric_job = _job_block(workflow, "numeric_v1_architecture_parity")
    if not numeric_job:
        errors.append("PR workflow is missing Numeric V1 architecture parity")
        return errors
    if f"uses: {SETUP_RUST_ACTION}" not in numeric_job:
        errors.append("PR numeric parity must install the managed Rust toolchain")
    if f"toolchain: {PINNED_RUST}" not in numeric_job:
        errors.append(f"PR numeric parity must pin Rust {PINNED_RUST}")

    numeric_commands = [
        line.strip()
        for line in numeric_job.splitlines()
        if line.strip().startswith("cargo test ")
    ]
    if len(numeric_commands) != 4:
        errors.append("PR numeric parity must retain all four consensus test commands")
    for command in numeric_commands:
        if not command.startswith("cargo test --locked "):
            errors.append(f"PR numeric parity command is not locked: {command}")
    return errors


def test_workspace_release_workflow_is_exact_sha_and_complete() -> None:
    """Every full-workspace release phase is pinned, locked, and exact-source."""

    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    assert _validate_release_workflow(workflow) == []


def test_pr_workflow_retains_locked_workspace_and_numeric_parity() -> None:
    """PR coverage explicitly exercises the workspace and pinned numeric tests."""

    workflow = PR_WORKFLOW.read_text(encoding="utf-8")
    assert _validate_pr_parity(workflow) == []


ReleaseMutation = Callable[[str], str]


@pytest.mark.parametrize(
    ("mutation", "expected_error"),
    (
        (
            lambda workflow: _replace_once(
                workflow, '    tags: ["v*"]', '    tags: ["release-*"]'
            ),
            "release workflow must run on main and v* pushes",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "group: workspace-release-${{ github.sha }}",
                "group: workspace-release-${{ github.ref }}",
            ),
            "release concurrency must be scoped to the exact SHA",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "cancel-in-progress: false",
                "cancel-in-progress: true",
            ),
            "release evidence must not be cancelled by ref movement",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "WORKFLOW_SHA: ${{ github.workflow_sha }}",
                "WORKFLOW_SHA: ${{ github.sha }}",
            ),
            "format must verify the exact workflow SHA: WORKFLOW_SHA: ${{ github.workflow_sha }}",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "toolchain: 1.93.1",
                "toolchain: stable",
            ),
            "format must pin Rust 1.93.1",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "python3 -m pytest -q pytests/scripts/workspace_release_gate_test.py",
                "python3 -m pytest -q pytests/scripts/check_kotodama_docs_test.py",
            ),
            "format is missing required command: python3 -m pytest -q pytests/scripts/workspace_release_gate_test.py",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "cargo test --locked --workspace --no-fail-fast",
                "cargo test --workspace --no-fail-fast",
            ),
            "test is missing required command: cargo test --locked --workspace --no-fail-fast",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "cargo clippy --locked --workspace --all-targets --all-features",
                "cargo clippy --locked --workspace --all-targets",
            ),
            "clippy is missing required command: cargo clippy --locked --workspace --all-targets --all-features -- -D warnings",
        ),
    ),
)
def test_release_workflow_guard_rejects_weakening(
    mutation: ReleaseMutation, expected_error: str
) -> None:
    """Representative trigger, source, toolchain, and command drift fails closed."""

    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    assert expected_error in _validate_release_workflow(mutation(workflow))


@pytest.mark.parametrize(
    ("mutation", "expected_error"),
    (
        (
            lambda workflow: _replace_once(
                workflow,
                "python3 -m pytest -q pytests/scripts/workspace_release_gate_test.py",
                "python3 -m pytest -q pytests/scripts/check_kotodama_docs_test.py",
            ),
            "PR workflow must execute the workspace release semantics guard",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "--workspace --locked\n          --branch --no-report",
                "--locked\n          --branch --no-report",
            ),
            "PR full test must explicitly select the locked workspace",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                f"          toolchain: 1.93.1\n      - uses: {RUST_CACHE_ACTION}",
                f"          toolchain: stable\n      - uses: {RUST_CACHE_ACTION}",
            ),
            "PR numeric parity must pin Rust 1.93.1",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "cargo test --locked -p ivm --test ivm_group_06 numeric_",
                "cargo test -p ivm --test ivm_group_06 numeric_",
            ),
            "PR numeric parity command is not locked",
        ),
    ),
)
def test_pr_workflow_guard_rejects_parity_weakening(
    mutation: ReleaseMutation, expected_error: str
) -> None:
    """PR workspace selection, toolchain pinning, and locking fail closed."""

    workflow = PR_WORKFLOW.read_text(encoding="utf-8")
    assert any(
        expected_error in error for error in _validate_pr_parity(mutation(workflow))
    )
