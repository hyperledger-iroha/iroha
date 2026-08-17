"""Static and adversarial tests for the exact-SHA workspace release gate."""

from __future__ import annotations

import json
import re
from collections.abc import Callable
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "workspace_release.yml"
PR_WORKFLOW = ROOT / ".github" / "workflows" / "pr.yml"
PINNED_RUST = "1.93.1"
SETUP_RUST_TOOLCHAIN_COMMIT = "166cdcfd11aee3cb47222f9ddb555ce30ddb9659"
RUST_CACHE_COMMIT = "e18b497796c12c097a38f9edb9d0641fb99eee32"
COMPILE_UNIT_BASELINE = ROOT / "ci" / "compile_unit_baselines.json"
COMPILE_UNIT_GUARD_COMMAND = (
    "python3 scripts/check_compile_unit_budget.py --locked --lib "
    "-p iroha_data_model --artifact-scope workspace "
    "--baseline ci/compile_unit_baselines.json "
    "--baseline-key iroha_data_model_lib "
    "--budget-percent 2 --budget-min-growth 3 "
    "--json-out target/ci/iroha-data-model-compile-units.json"
)
COMPILE_UNIT_REPORT = "target/ci/iroha-data-model-compile-units.json"
COMPILE_UNIT_ARTIFACT_IDENTITY = "cargo-package-target-features-profile-v2"
BUILD_EFFICIENCY_PROVENANCE_COMMAND = (
    "python3 -I -S scripts/check_build_efficiency_provenance.py"
)
BUILD_EFFICIENCY_PROVENANCE_TEST = (
    "scripts/tests/check_build_efficiency_provenance_test.py"
)
REQUIRED_NUMERIC_TEST_COMMANDS = (
    "cargo test --locked -p ivm --test ivm_group_06 numeric_",
    "cargo test --locked -p ivm --test ivm_group_01 abi_hash_versions::",
    "cargo test --locked -p ivm --test ivm_group_03 gas_schedule_hash",
    "cargo test --locked -p ivm --test ivm_group_05 "
    "kotodama_checked_arithmetic::",
    "cargo test --locked -p iroha_primitives "
    "randomized_decimal_arithmetic_matches_independent_rational_reference",
)


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


def _replace_once_in_job(
    workflow: str, job_name: str, old: str, new: str
) -> str:
    """Apply one deliberate mutation inside exactly one named workflow job."""

    job = _job_block(workflow, job_name)
    assert job, f"workflow job is absent: {job_name!r}"
    mutated_job = _replace_once(job, old, new)
    assert workflow.count(job) == 1, f"workflow job block is not unique: {job_name!r}"
    return workflow.replace(job, mutated_job, 1)


def _validate_release_workflow(workflow: str) -> list[str]:
    """Return deterministic errors for weakened release-workflow semantics."""

    errors: list[str] = []
    global_requirements = (
        (
            '  schedule:\n    - cron: "17 2 * * *"',
            "release workflow must run nightly",
        ),
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
        "doc": "runs-on: [self-hosted, Linux, iroha2]",
        "test": "runs-on: [self-hosted, Linux, iroha2]",
        "coverage": "runs-on: [self-hosted, Linux, iroha2]",
        "clippy": "runs-on: [self-hosted, Linux, iroha2]",
    }
    commands = {
        "format": (
            "python3 -m pytest -q pytests/scripts/workspace_release_gate_test.py",
            "cargo metadata --locked --no-deps --format-version 1 > /dev/null",
            "cargo fmt --all -- --check",
        ),
        "build": ("cargo build --locked --workspace",),
        "doc": ("cargo doc --locked --workspace --no-deps --all-features",),
        "test": (
            COMPILE_UNIT_GUARD_COMMAND,
            "name: workspace-release-compile-units",
            f"path: {COMPILE_UNIT_REPORT}",
            "if-no-files-found: error",
            "cargo test --locked --workspace --no-fail-fast",
        ),
        "coverage": (
            "mold --run cargo llvm-cov nextest --workspace --locked --branch --no-report",
            "mold --run cargo llvm-cov --doc --branch --no-report",
            "cargo llvm-cov report --doctests --ignore-filename-regex "
            "'iroha_cli|iroha_torii' --lcov --output-path lcov.info",
            "uses: coverallsapp/github-action@648a8eb78e6d50909eff900e4ec85cab4524a45b",
        ),
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
        if (
            "uses: actions-rust-lang/setup-rust-toolchain@"
            f"{SETUP_RUST_TOOLCHAIN_COMMIT}"
        ) not in job:
            errors.append(
                f"{job_name} must install the managed Rust toolchain from the reviewed commit"
            )
        if f"toolchain: {PINNED_RUST}" not in job:
            errors.append(f"{job_name} must pin Rust {PINNED_RUST}")

        normalized_job = _normalized(job)
        for command in commands[job_name]:
            if command not in normalized_job:
                errors.append(f"{job_name} is missing required command: {command}")

    return errors


def _validate_pr_parity(workflow: str) -> list[str]:
    """Return errors when affected PR routing can silently weaken validation."""

    errors: list[str] = []
    if "paths-ignore:" not in workflow or "paths_ignore:" in workflow:
        errors.append("PR workflow must use the valid paths-ignore trigger key")
    docs_job = _job_block(workflow, "kotodama_docs")
    if (
        "python3 -m pytest -q pytests/scripts/workspace_release_gate_test.py"
        not in _normalized(docs_job)
    ):
        errors.append("PR workflow must execute the workspace release semantics guard")

    classifier_job = _job_block(workflow, "rust_changes")
    if not classifier_job:
        errors.append("PR workflow is missing the Rust lane classifier")
    else:
        normalized_classifier = _normalized(classifier_job)
        classifier_requirements = (
            "fetch-depth: 0",
            "python3 scripts/rust_ci.py validate",
            "python3 -m pytest -q pytests/scripts/rust_ci_test.py",
            "pytests/scripts/check_cargo_feature_hygiene_test.py",
            "pytests/scripts/check_workspace_target_inventory_test.py",
            BUILD_EFFICIENCY_PROVENANCE_TEST,
            "scripts/tests/check_source_file_budget_test.py",
            "scripts/tests/check_compile_unit_budget_test.py",
            "scripts/tests/check_generated_artifacts_test.py",
            "python3 scripts/check_cargo_feature_hygiene.py",
            "python3 scripts/check_workspace_target_inventory.py",
            BUILD_EFFICIENCY_PROVENANCE_COMMAND,
            "python3 scripts/check_source_file_budget.py --require-objective",
            "python3 scripts/check_generated_artifacts.py",
            'FULL_REQUESTED: ${{ contains(github.event.pull_request.labels.*.name, '
            "'ci/full') }}",
            'scope_args=(--base "$BASE_SHA")',
            "scope_args=(--all)",
            'python3 scripts/rust_ci.py classify \\ "${scope_args[@]}" \\ '
            "--json-out target/ci/rust-classification.json \\ "
            '--github-output "$GITHUB_OUTPUT"',
        )
        for requirement in classifier_requirements:
            if requirement not in normalized_classifier:
                errors.append(
                    f"PR Rust classifier is missing required behavior: {requirement}"
                )
        provenance_position = normalized_classifier.find(
            BUILD_EFFICIENCY_PROVENANCE_COMMAND
        )
        provenance_followers = (
            "python3 scripts/rust_ci.py validate",
            "python3 -m pytest -q pytests/scripts/rust_ci_test.py",
            "python3 scripts/check_cargo_feature_hygiene.py",
            "python3 scripts/check_workspace_target_inventory.py",
            "python3 scripts/check_compile_time_table_assets.py",
            "python3 scripts/check_dependency_budget.py",
            "python3 scripts/check_source_file_budget.py --require-objective",
        )
        if provenance_position >= 0 and any(
            normalized_classifier.find(command) < provenance_position
            for command in provenance_followers
            if normalized_classifier.find(command) >= 0
        ):
            errors.append(
                "PR build-efficiency provenance guard must run before dependency, "
                "source-budget, and Cargo-facing checks"
            )

    affected_job = _job_block(workflow, "rust_affected")
    if not affected_job:
        errors.append("PR workflow is missing affected Rust validation")
    else:
        normalized_affected = _normalized(affected_job)
        affected_requirements = (
            "matrix: ${{ fromJSON(needs.rust_changes.outputs.matrix) }}",
            "uses: actions-rust-lang/setup-rust-toolchain@"
            f"{SETUP_RUST_TOOLCHAIN_COMMIT}",
            'cache: "false"',
            f"toolchain: {PINNED_RUST}",
            f"shared-key: rust-lane-{PINNED_RUST}-${{{{ matrix.lane }}}}",
            "if: matrix.lane == 'execution'",
            COMPILE_UNIT_GUARD_COMMAND,
            "if: always() && matrix.lane == 'execution'",
            "name: compile-units-${{ matrix.lane }}",
            f"path: {COMPILE_UNIT_REPORT}",
            "if-no-files-found: error",
            'python3 scripts/rust_ci.py run --packages "${{ matrix.packages }}" '
            "--checks clippy,build,test,doc",
        )
        for requirement in affected_requirements:
            if requirement not in normalized_affected:
                errors.append(
                    f"PR affected Rust job is missing required behavior: {requirement}"
                )

    required_job = _job_block(workflow, "rust_required")
    if not required_job:
        errors.append("PR workflow is missing the single Rust result aggregator")
    else:
        normalized_required = _normalized(required_job)
        for requirement in (
            "if: always()",
            "needs: [rust_changes, rust_affected]",
            'test "$CLASSIFIER_RESULT" = success',
            'test "$AFFECTED_RESULT" = success',
            'test "$AFFECTED_RESULT" = skipped',
        ):
            if requirement not in normalized_required:
                errors.append(
                    f"PR Rust result aggregator is missing required behavior: {requirement}"
                )

    numeric_job = _job_block(workflow, "numeric_v1_architecture_parity")
    if not numeric_job:
        errors.append("PR workflow is missing Numeric V1 architecture parity")
        return errors
    if (
        "uses: actions-rust-lang/setup-rust-toolchain@"
        f"{SETUP_RUST_TOOLCHAIN_COMMIT}"
    ) not in numeric_job:
        errors.append(
            "PR numeric parity must install the managed Rust toolchain from the reviewed commit"
        )
    if f"toolchain: {PINNED_RUST}" not in numeric_job:
        errors.append(f"PR numeric parity must pin Rust {PINNED_RUST}")

    numeric_commands = [
        line.strip()
        for line in numeric_job.splitlines()
        if line.strip().startswith("cargo test ")
    ]
    for required_command in REQUIRED_NUMERIC_TEST_COMMANDS:
        if required_command not in numeric_commands:
            errors.append(
                "PR numeric parity is missing required command: "
                f"{required_command}"
            )
    for command in numeric_commands:
        if not command.startswith("cargo test --locked "):
            errors.append(f"PR numeric parity command is not locked: {command}")
    return errors


def test_workspace_release_workflow_is_exact_sha_and_complete() -> None:
    """Every full-workspace release phase is pinned, locked, and exact-source."""

    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    assert _validate_release_workflow(workflow) == []


def test_pr_workflow_retains_locked_workspace_and_numeric_parity() -> None:
    """PR routing fails closed while retaining pinned numeric tests."""

    workflow = PR_WORKFLOW.read_text(encoding="utf-8")
    assert _validate_pr_parity(workflow) == []


def test_compile_unit_baseline_pins_the_cross_platform_measurement_scope() -> None:
    """The checked-in ratchet describes exactly the graph enforced in CI."""

    payload = json.loads(COMPILE_UNIT_BASELINE.read_text(encoding="utf-8"))
    assert payload["schema_version"] == 2
    baseline = payload["iroha_data_model_lib"]
    assert baseline["compile_units"] > 0
    assert baseline["artifact_identity"] == COMPILE_UNIT_ARTIFACT_IDENTITY
    assert baseline["manifest_path"] == "Cargo.toml"
    assert baseline["packages"] == ["iroha_data_model"]
    assert baseline["target"] == "lib"
    assert baseline["artifact_scope"] == "workspace"
    assert baseline["cargo_locked"] is True
    assert baseline["toolchain"] == PINNED_RUST
    assert baseline["workspace"] is False
    assert baseline["budget_percent"] == 2
    assert baseline["budget_min_growth"] == 3


ReleaseMutation = Callable[[str], str]


@pytest.mark.parametrize(
    ("mutation", "expected_error"),
    (
        (
            lambda workflow: _replace_once(
                workflow,
                '  schedule:\n    - cron: "17 2 * * *"',
                "",
            ),
            "release workflow must run nightly",
        ),
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
                f"actions-rust-lang/setup-rust-toolchain@{SETUP_RUST_TOOLCHAIN_COMMIT}",
                "actions-rust-lang/setup-rust-toolchain@v1",
            ),
            "format must install the managed Rust toolchain from the reviewed commit",
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
                "cargo doc --locked --workspace --no-deps --all-features",
                "cargo doc --locked --workspace --no-deps",
            ),
            "doc is missing required command: cargo doc --locked --workspace --no-deps --all-features",
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
            lambda workflow: _replace_once_in_job(
                workflow,
                "test",
                "python3 scripts/check_compile_unit_budget.py",
                "true # compile-unit guard removed",
            ),
            f"test is missing required command: {COMPILE_UNIT_GUARD_COMMAND}",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "test",
                "--budget-percent 2",
                "--budget-percent 20",
            ),
            f"test is missing required command: {COMPILE_UNIT_GUARD_COMMAND}",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "coverallsapp/github-action@648a8eb78e6d50909eff900e4ec85cab4524a45b",
                "coverallsapp/github-action@main",
            ),
            "coverage is missing required command: uses: coverallsapp/github-action@648a8eb78e6d50909eff900e4ec85cab4524a45b",
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
                "    paths-ignore:\n",
                "    paths_ignore:\n",
            ),
            "PR workflow must use the valid paths-ignore trigger key",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                '          scope_args=(--base "$BASE_SHA")\n',
                "          scope_args=(--paths specs/index.md)\n",
            ),
            "PR Rust classifier is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_changes",
                "python3 scripts/check_cargo_feature_hygiene.py",
                "true # Cargo feature guard removed",
            ),
            "PR Rust classifier is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_changes",
                "python3 scripts/check_workspace_target_inventory.py",
                "true # Cargo target guard removed",
            ),
            "PR Rust classifier is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_changes",
                BUILD_EFFICIENCY_PROVENANCE_TEST,
                "scripts/tests/check_source_file_budget_test.py",
            ),
            "PR Rust classifier is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_changes",
                BUILD_EFFICIENCY_PROVENANCE_COMMAND,
                "true # build-efficiency provenance removed",
            ),
            "PR Rust classifier is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_changes",
                f"{BUILD_EFFICIENCY_PROVENANCE_COMMAND}\n"
                "          python3 scripts/rust_ci.py validate",
                "python3 scripts/rust_ci.py validate\n"
                f"          {BUILD_EFFICIENCY_PROVENANCE_COMMAND}",
            ),
            "PR build-efficiency provenance guard must run before",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_affected",
                "--checks clippy,build,test,doc",
                "--checks build",
            ),
            "PR affected Rust job is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_affected",
                "--artifact-scope workspace",
                "--artifact-scope all",
            ),
            "PR affected Rust job is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_affected",
                f"toolchain: {PINNED_RUST}",
                "toolchain: stable",
            ),
            "PR affected Rust job is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_affected",
                "if: matrix.lane == 'execution'",
                "if: false",
            ),
            "PR affected Rust job is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "rust_required",
                '          test "$CLASSIFIER_RESULT" = success\n',
                "          true\n",
            ),
            "PR Rust result aggregator is missing required behavior",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "numeric_v1_architecture_parity",
                "          toolchain: 1.93.1\n"
                f"      - uses: Swatinem/rust-cache@{RUST_CACHE_COMMIT}",
                "          toolchain: stable\n"
                f"      - uses: Swatinem/rust-cache@{RUST_CACHE_COMMIT}",
            ),
            "PR numeric parity must pin Rust 1.93.1",
        ),
        (
            lambda workflow: _replace_once_in_job(
                workflow,
                "numeric_v1_architecture_parity",
                f"actions-rust-lang/setup-rust-toolchain@{SETUP_RUST_TOOLCHAIN_COMMIT}",
                "actions-rust-lang/setup-rust-toolchain@v1",
            ),
            "PR numeric parity must install the managed Rust toolchain from the reviewed commit",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "cargo test --locked -p ivm --test ivm_group_06 numeric_",
                "cargo test -p ivm --test ivm_group_06 numeric_",
            ),
            "PR numeric parity command is not locked",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                "cargo test --locked -p ivm --test ivm_group_03 gas_schedule_hash",
                "true # numeric gas schedule coverage removed",
            ),
            "PR numeric parity is missing required command",
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
