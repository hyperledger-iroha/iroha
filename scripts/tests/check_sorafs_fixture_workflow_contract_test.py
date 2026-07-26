"""Contract tests for SoraFS fixture and native-SDK workflow coverage."""

from __future__ import annotations

import re
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_ROOT = REPO_ROOT / ".github" / "workflows"

NATIVE_ESCROW_TRIGGER_PATHS = {
    "crates/connect_norito_bridge/**",
    "crates/iroha_core/src/smartcontracts/isi/escrow.rs",
    "crates/iroha_data_model/src/bin/cancel_asset_lock_fixtures.rs",
    "crates/iroha_data_model/src/escrow.rs",
    "crates/iroha_data_model/src/events/data/escrow.rs",
    "crates/iroha_data_model/src/isi/escrow.rs",
    "crates/iroha_data_model/src/testing/cancel_asset_lock.rs",
    "crates/kotodama_lang/src/samples/native_escrow.ko",
    "crates/kotodama_lang/src/samples/native_escrow.to",
    "crates/sorafs_manifest/**",
    "fixtures/sorafs_manifest/appeal_finance/**",
    "fixtures/sorafs_manifest/reference_sdk/**",
    "fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json",
    "integration_tests/tests/native_escrow.rs",
}


def read(relative: str) -> str:
    """Read one repository file as UTF-8."""

    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def pull_request_paths(workflow_name: str) -> set[str]:
    """Return the literal pull-request path filter from one workflow."""

    source = (WORKFLOW_ROOT / workflow_name).read_text(encoding="utf-8")
    match = re.search(
        r"(?ms)^  pull_request:\n"
        r"(?P<body>(?:^    .*\n)*)",
        source,
    )
    assert match is not None, f"{workflow_name} must define pull_request"
    body = match.group("body")
    paths = re.search(
        r"(?ms)^    paths:\n(?P<paths>(?:^      - .*\n)+)",
        body,
    )
    assert paths is not None, f"{workflow_name} must define pull_request.paths"
    return {
        line.removeprefix("      - ").strip().strip('"')
        for line in paths.group("paths").splitlines()
    }


@pytest.mark.parametrize(
    "workflow_name",
    [
        "mobile_sdk_artifacts.yml",
        "pr_csharp.yml",
        "sorafs-orchestrator-sdk.yml",
    ],
)
def test_native_sdk_workflows_cover_appeal_finance_and_escrow_sources(
    workflow_name: str,
) -> None:
    """Every relevant SDK job reruns for all shared native-escrow inputs."""

    paths = pull_request_paths(workflow_name)
    assert NATIVE_ESCROW_TRIGGER_PATHS <= paths
    assert "scripts/check_sorafs_reference_sdk_fixtures.py" in paths
    assert "scripts/tests/check_sorafs_fixture_workflow_contract_test.py" in paths


def test_fixture_workflow_covers_closed_reference_sdk_inputs() -> None:
    """The nightly fixture job cannot miss wire, fixture, or checker changes."""

    paths = pull_request_paths("sorafs-fixtures-nightly.yml")
    expected = NATIVE_ESCROW_TRIGGER_PATHS - {"crates/connect_norito_bridge/**"}
    assert expected <= paths
    assert {
        "ci/check_sorafs_fixtures.sh",
        "scripts/check_sorafs_reference_sdk_fixtures.py",
        "scripts/tests/check_sorafs_fixture_workflow_contract_test.py",
        "scripts/tests/check_sorafs_reference_sdk_fixtures_test.py",
    } <= paths


def test_fixture_workflow_requires_its_installed_toolchains() -> None:
    """A broken Node or Go setup is a gate failure, not a capability skip."""

    workflow = read(".github/workflows/sorafs-fixtures-nightly.yml")
    fixture_gate = read("ci/check_sorafs_fixtures.sh")
    assert 'SORAFS_FIXTURE_REQUIRE_TOOLCHAIN: "1"' in workflow
    assert 'if [[ "${require_fixture_toolchain}" == "1" ]]' in fixture_gate
    assert "fixture_tool_available node" in fixture_gate
    assert "fixture_tool_available go" in fixture_gate
    assert "error: ${check_label} requires ${tool_name}" in fixture_gate


def test_reference_sdk_regeneration_is_closed_and_double_run_stable() -> None:
    """Both generators, the signed inventory, and all bytes are checked twice."""

    fixture_gate = read("ci/check_sorafs_fixtures.sh")
    assert (
        fixture_gate.count(
            "python3 scripts/check_sorafs_reference_sdk_fixtures.py"
        )
        == 2
    )
    assert "for fixture_regeneration_pass in 1 2; do" in fixture_gate
    assert "--bin cancel_asset_lock_fixtures" in fixture_gate
    assert "--bin generate_por_fixtures" in fixture_gate
    assert '"fixtures/sorafs_manifest"' in fixture_gate
    assert '"byte_length": path_stat.st_size' in fixture_gate
    assert '"sha256": digest.hexdigest()' in fixture_gate
    assert "manifest-pass-1.json" in fixture_gate
    assert "manifest-pass-2.json" in fixture_gate
    assert "cmp -s" in fixture_gate
    assert (
        "git status --short --untracked-files=all -- fixtures/sorafs_manifest"
        in fixture_gate
    )


def test_parity_fixture_snapshot_rejects_missing_inputs() -> None:
    """The SDK parity artifact collector cannot silently omit a fixture."""

    parity_gate = read("ci/sdk_sorafs_orchestrator.sh")
    assert 'sys.exit(f"[sorafs-sdk] {label} missing: {path}")' in parity_gate
    assert "warning: fixture file missing" not in parity_gate
    assert "if source is None:" not in parity_gate


def test_native_release_jobs_build_and_require_the_bridge() -> None:
    """C# and both mobile jobs fail closed when the ABI-21 bridge is absent."""

    csharp = read(".github/workflows/pr_csharp.yml")
    mobile = read(".github/workflows/mobile_sdk_artifacts.yml")
    parity = read(".github/workflows/sorafs-orchestrator-sdk.yml")

    assert 'IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"' in csharp
    assert "cargo build --locked -p connect_norito_bridge" in csharp
    assert mobile.count('IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"') == 2
    assert "name: Build NoritoBridge XCFramework" in mobile
    assert "name: Build host SoraFS reference native bridge" in mobile
    assert "npm run build:native" in read("ci/sdk_sorafs_orchestrator.sh")
    assert "bash ci/sdk_sorafs_orchestrator.sh" in parity
