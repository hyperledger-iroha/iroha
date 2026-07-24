#!/usr/bin/env python3
"""Validate the committed SoraFS release and conformance workflow contracts."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_release_version_map import (  # noqa: E402
    _read_bytes_no_follow,
    _require_regular_repo_file,
)


SCHEMA = "sorafs.release.automation.v1"
RELEASE_TARGET_RUNNERS: tuple[tuple[str, str], ...] = (
    ("ubuntu-24.04", "x86_64-unknown-linux-gnu"),
    ("ubuntu-24.04-arm", "aarch64-unknown-linux-gnu"),
    ("macos-15-intel", "x86_64-apple-darwin"),
    ("macos-14", "aarch64-apple-darwin"),
    ("windows-latest", "x86_64-pc-windows-msvc"),
)
RELEASE_DOCUMENTS: dict[str, tuple[str, ...]] = {
    "docs/portal/sidebars.js": ("'sorafs/release-rollback-yank'",),
    "docs/portal/docs/sorafs/runbooks-index.md": (
        "[`sorafs/release-rollback-yank`](./release-rollback-yank.md)",
    ),
    "docs/source/sorafs_release_pipeline_plan.md": (
        "builds native Linux x86_64/aarch64",
        "executes all three binaries from each clean extraction",
        "`scripts/package_sorafs_cli_candidate.py` assembles the whole platform",
        "exactly the five expected target-triple checksum manifests",
        "The five-target CLI archive path is source-complete",
        "build, publish, and clean-install all six",
        "`docs/portal/docs/sorafs/release-rollback-yank.md`",
        "`sorafs-release-authentication` environment",
        "`scripts/release_manifest_signing.py verify`",
        "never receives a private key or invokes a signer",
    ),
    "docs/portal/docs/sorafs/release-rollback-yank.md": (
        "# SoraFS Release Rollback and Yank",
        "`cargo yank --vers <version> <crate>`",
        "npm deprecate <package>@<version>",
        "Python/PyPI",
        "C#/NuGet",
        "JVM/Android",
        "Swift Package Manager",
        "GitHub CLI artifacts",
        "`withdrawn`, `not_published`, or `failed`",
        "Never reuse a withdrawn version",
    ),
    "docs/portal/docs/sorafs/developer-releases.md": (
        "sorafs-cli-vX.Y.Z",
        "all five native candidate archives",
        "[SoraFS Release Rollback and Yank](./release-rollback-yank.md)",
        "`SORAFS_RELEASE_MANIFEST_VERIFIER_PATH`",
        "No private key or HSM signing operation",
    ),
    "docs/examples/sorafs_release_notes.md": (
        "## Rollback / Yank Record",
        "every package row in `release/version-map.toml`",
        "`<withdrawn | not_published | failed>`",
    ),
}
FORBIDDEN_RELEASE_DOCUMENT_CLAIMS: dict[str, tuple[str, ...]] = {
    "docs/source/sorafs_release_pipeline_plan.md": (
        "via cross",
        "exactly the three expected platform checksum manifests",
    ),
    "docs/portal/docs/sorafs/developer-releases.md": (
        "git tag -s sorafs-v",
        "invokes the script above",
    ),
}
WORKFLOWS: dict[str, tuple[str, ...]] = {
    ".github/workflows/sorafs-cli-release.yml": (
        '"sorafs-cli-v*"',
        "actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10",
        "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
        "scripts/check_sorafs_release_version_map.py",
        "scripts/check_sorafs_reference_sdk_release_evidence.py",
        "scripts/build_sorafs_reference_sdk_release_canary.py",
        "scripts/run_sorafs_reference_sdk_release_evidence.py",
        "scripts/check_workflow_action_pins.py",
        '- "scripts/build_release_bundle.sh"',
        '- "scripts/build_release_image.sh"',
        '- "scripts/generate_release_manifest.py"',
        '- "scripts/generate_sorafs_cli_release_manifest.py"',
        '- "scripts/release_manifest_signing.py"',
        '- "scripts/publish_plan.py"',
        '- "scripts/run_release_pipeline.py"',
        '- "scripts/requirements.txt"',
        "python3 -m pip install -r scripts/requirements.txt",
        "python3 scripts/check_workflow_action_pins.py",
        "scripts/tests/check_sorafs_reference_sdk_release_evidence_test.py",
        "run: bash ci/check_sorafs_cli_release.sh",
        "scripts/package_sorafs_validate_release.sh",
        "scripts/package_sorafs_cli_candidate.py",
        "scripts/tests/package_sorafs_cli_candidate_test.py",
        '- "scripts/tests/release_profile_validation_test.py"',
        '- "scripts/tests/release_manifest_signing_test.py"',
        '- "scripts/tests/release_manifest_signing_test.sh"',
        '- "scripts/tests/generate_release_manifest_test.py"',
        '- "scripts/tests/generate_sorafs_cli_release_manifest_test.py"',
        '- "scripts/tests/publish_plan_test.py"',
        '- "docs/source/sorafs_release_pipeline_plan*.md"',
        '- "docs/source/release_dual_track_automation_plan*.md"',
        '- "docs/source/release_dual_track_runbook*.md"',
        '- "docs/source/release_artifact_selection*.md"',
        '- "docs/source/sora_nexus_operator_onboarding*.md"',
        '- "docs/portal/docs/nexus/nexus-operator-onboarding*.md"',
        '- "CHANGELOG.md"',
        '- "LICENSE"',
        "anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610",
        "anchore/scan-action@e1165082ffb1fe366ebaf02d8526e7c4989ea9d2",
        "syft-version: v1.44.0",
        "grype-version: v0.112.0",
        "severity-cutoff: high",
        "fail-build: true",
        'cargo build --locked --release -p sorafs_orchestrator --bin sorafs_cli --target "$target"',
        'cargo build --locked --release -p sorafs_car --features cli --bin sorafs_fetch --target "$target"',
        'cargo build --locked --release -p sorafs_manifest --bin sorafs-validate --target "$target"',
        'if [[ "$host_target" != "$target" ]]; then',
        "target: ${{ matrix.target }}",
        '--target "${{ matrix.target }}"',
        'binary_suffix: ".exe"',
        "find . -type f ! -name SHA256SUMS -print",
        "done > SHA256SUMS",
        "name: sorafs-cli-release-gate-${{ github.run_id }}",
        "path: artifacts/release-gate",
        "artifacts/release-gate/artifacts/sorafs-release/sorafs-release.spdx.json",
        "name: Generate platform binary SBOM",
        "name: Package reference validator and FFI header",
        "artifacts/sorafs-cli/reference-validator",
        "output-file: artifacts/sorafs-cli/sorafs-cli-${{ matrix.target }}.spdx.json",
        "name: Scan platform binary SBOM",
        "output-file: artifacts/sorafs-cli/sorafs-cli-${{ matrix.target }}-vulnerabilities.sarif",
        "docs/portal/docs/sorafs/release-rollback-yank.md artifacts/sorafs-cli/ROLLBACK-YANK.md",
        "cp CHANGELOG.md LICENSE artifacts/sorafs-cli/",
        "name: Rebuild deterministic platform archive and run clean-consumer smoke",
        "candidate-package-first.json",
        "candidate-package-replay.json",
        "artifacts/sorafs-cli/platform-archive",
        "name: Stage source release scan evidence",
        "name: Finalize platform checksums",
        "  prepare-release-manifest:",
        "name: Build the canonical foundational manifest twice",
        "python3 scripts/generate_sorafs_cli_release_manifest.py create",
        "name: Upload unsigned foundational manifest for external HSM signing",
        "  verify-release-auth:",
        "environment: sorafs-release-authentication",
        "runs-on: [self-hosted, linux, x64, sorafs-release-auth]",
        "SORAFS_RELEASE_SIGNATURE_PATH: ${{ vars.SORAFS_RELEASE_SIGNATURE_PATH }}",
        "SORAFS_RELEASE_PUBLIC_KEY_PATH: ${{ vars.SORAFS_RELEASE_PUBLIC_KEY_PATH }}",
        "SORAFS_RELEASE_MANIFEST_VERIFIER_PATH: ${{ vars.SORAFS_RELEASE_MANIFEST_VERIFIER_PATH }}",
        "SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT: ${{ vars.SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT }}",
        "SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256: ${{ vars.SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256 }}",
        "name: Reconcile the foundational manifest with the immutable candidates",
        "name: Verify the protected external Ed25519 manifest tuple",
        "python3 scripts/release_manifest_signing.py verify",
        "name: Upload authenticated foundational manifest tuple",
        "name: Verify authenticated release-manifest candidate binding before provenance",
        "needs: [release-gate, package, verify-release-auth]",
        "sigstore/cosign-installer@ba7bc0a3fef59531c69a25acd34668d6d3fe6f22",
        "name: Verify platform package checksums before signing",
        '[[ "${#checksum_files[@]}" -ne 5 ]]',
        "sha256sum --check SHA256SUMS",
        "SHA256SUMS contains duplicate file entries",
        "SHA256SUMS does not cover the exact platform candidate file set",
        "actions/attest@a1948c3f048ba23858d222213b7c278aabede763",
        "name: Stage offline provenance bundles",
        '[[ "$attestation_path" != "${RUNNER_TEMP}/"*',
        '[[ "$count" -gt 16 ]]',
        "cosign sign-blob --yes --bundle",
        "cosign verify-blob",
        'certificate-identity "https://github.com/${GITHUB_REPOSITORY}/.github/workflows/sorafs-cli-release.yml@${GITHUB_REF}"',
        'certificate-oidc-issuer "https://token.actions.githubusercontent.com"',
        "id-token: write",
    ),
    ".github/workflows/sorafs-fixtures-nightly.yml": (
        'cron: "17 2 * * *"',
        "actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10",
        "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
        'python-version: "3.11"',
        "bash ci/check_sorafs_fixtures.sh",
        "actions/setup-go@924ae3a1cded613372ab5595356fb5720e22ba16",
        'go-version: "1.26.x"',
        "actions/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e",
        'node-version: "24"',
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
    ),
    ".github/workflows/sorafs-orchestrator-sdk.yml": (
        'cron: "41 3 * * *"',
        "actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10",
        "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
        'python-version: "3.11"',
        "actions/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e",
        'node-version: "24"',
        "runs-on: macos-14",
        "bash ci/sdk_sorafs_orchestrator.sh",
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
    ),
}


def _workflow_job(source: str, name: str) -> str | None:
    """Return one top-level workflow job, including its header."""

    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n.*?(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        source,
    )
    return match.group(0) if match is not None else None


def _validate_workflow_source(relative: str, source: str) -> list[str]:
    """Return deterministic contract errors for one workflow source."""

    errors: list[str] = []
    if "pull_request_target:" in source:
        errors.append(f"{relative}: pull_request_target is forbidden")
    if re.search(r"\b(?:curl|wget)\b", source):
        errors.append(f"{relative}: network bootstrap commands are forbidden")
    action_refs = re.findall(r"uses:\s*[^\s@]+@([^\s#]+)", source)
    if any(re.fullmatch(r"[0-9a-f]{40}", reference) is None for reference in action_refs):
        errors.append(
            f"{relative}: floating action references are forbidden; pin full commits"
        )
    if "cancel-in-progress: false" not in source:
        errors.append(f"{relative}: release evidence jobs must not be cancelled")
    for marker in WORKFLOWS[relative]:
        if marker not in source:
            errors.append(f"{relative}: missing contract marker `{marker}`")

    if relative.endswith("sorafs-cli-release.yml"):
        jobs_source = source[source.index("jobs:\n") + len("jobs:\n") :]
        job_inventory = tuple(
            re.findall(r"(?m)^  ([A-Za-z0-9_-]+):\n", jobs_source)
        )
        expected_job_inventory = (
            "release-gate",
            "package",
            "prepare-release-manifest",
            "verify-release-auth",
            "sign",
        )
        if job_inventory != expected_job_inventory:
            errors.append(
                f"{relative}: release workflow job inventory must be exactly "
                f"{expected_job_inventory}"
            )
        if "${{ secrets." in source:
            errors.append(
                f"{relative}: release workflow must not consume GitHub secrets"
            )
        lowered_source = source.lower()
        if any(
            marker in lowered_source
            for marker in (
                "release_manifest_signing.py sign",
                "--external-signer",
                "--signing-seed",
                "signing_seed",
                "private-key",
                "private_key",
                "development-local-signing",
            )
        ):
            errors.append(
                f"{relative}: GitHub release jobs must not receive private signing "
                "material or invoke the foundational Ed25519 signer"
            )
        if source.count("merge-multiple: false") != 3:
            errors.append(
                f"{relative}: candidate downloads must preserve exactly five "
                "artifact-name directories"
            )
        prepare_job = _workflow_job(source, "prepare-release-manifest")
        auth_job = _workflow_job(source, "verify-release-auth")
        promotion_job = _workflow_job(source, "sign")
        promotion_guard = (
            "if: ${{ startsWith(github.ref, 'refs/tags/sorafs-cli-v') "
            "|| inputs.sign_artifacts }}"
        )
        if prepare_job is None:
            errors.append(f"{relative}: missing foundational-manifest preparation job")
        else:
            if promotion_guard not in prepare_job:
                errors.append(
                    f"{relative}: foundational-manifest preparation lacks the "
                    "reviewed promotion trigger guard"
                )
            if "needs: [release-gate, package]" not in prepare_job:
                errors.append(
                    f"{relative}: foundational manifest must depend on the release "
                    "gate and all platform packages"
                )
        if auth_job is None:
            errors.append(f"{relative}: missing protected release-authentication job")
        else:
            required_auth_bindings = (
                "SORAFS_RELEASE_SIGNATURE_PATH: ${{ vars.SORAFS_RELEASE_SIGNATURE_PATH }}",
                "SORAFS_RELEASE_PUBLIC_KEY_PATH: ${{ vars.SORAFS_RELEASE_PUBLIC_KEY_PATH }}",
                "SORAFS_RELEASE_MANIFEST_VERIFIER_PATH: ${{ vars.SORAFS_RELEASE_MANIFEST_VERIFIER_PATH }}",
                "SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT: ${{ vars.SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT }}",
                "SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256: ${{ vars.SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256 }}",
            )
            if any(binding not in auth_job for binding in required_auth_bindings):
                errors.append(
                    f"{relative}: release authentication is missing an explicit "
                    "protected public tuple or trust-anchor binding"
                )
            if "needs: [release-gate, prepare-release-manifest]" not in auth_job:
                errors.append(
                    f"{relative}: release authentication must consume the gated "
                    "foundational manifest"
                )
            if promotion_guard not in auth_job:
                errors.append(
                    f"{relative}: release authentication lacks the reviewed "
                    "promotion trigger guard"
                )
            if "environment: sorafs-release-authentication" not in auth_job:
                errors.append(
                    f"{relative}: release authentication must use the protected "
                    "release-authentication environment"
                )
            if (
                "runs-on: [self-hosted, linux, x64, sorafs-release-auth]"
                not in auth_job
            ):
                errors.append(
                    f"{relative}: release authentication must use the protected "
                    "self-hosted release-auth runner"
                )
            if "${{ secrets." in auth_job:
                errors.append(
                    f"{relative}: release authentication must not receive GitHub secrets"
                )
            if any(
                marker in auth_job
                for marker in (
                    "id-token:",
                    "attestations:",
                    "artifact-metadata:",
                    "actions/attest@",
                    "cosign",
                )
            ):
                errors.append(
                    f"{relative}: OIDC and provenance authority must not enter the "
                    "release-authentication job"
                )
            lowered_auth = auth_job.lower()
            forbidden_auth_markers = (
                "release_manifest_signing.py sign",
                "--external-signer",
                "--signing-seed",
                "signing_seed",
                "private-key",
                "private_key",
                "development-local-signing",
            )
            if any(marker in lowered_auth for marker in forbidden_auth_markers):
                errors.append(
                    f"{relative}: release authentication must be verification-only "
                    "and must not receive private signing material"
                )
            if (
                auth_job.count(
                    "python3 scripts/release_manifest_signing.py verify"
                )
                != 2
            ):
                errors.append(
                    f"{relative}: release authentication must verify both the "
                    "protected tuple and its staged public snapshot"
                )
            if (
                auth_job.count(
                    "python3 scripts/generate_sorafs_cli_release_manifest.py check"
                )
                != 1
            ):
                errors.append(
                    f"{relative}: release authentication must reconcile the signed "
                    "manifest with the downloaded candidates exactly once"
                )
            try:
                reconcile = auth_job.index(
                    "name: Reconcile the foundational manifest with the immutable candidates"
                )
                external_verify = auth_job.index(
                    "name: Verify the protected external Ed25519 manifest tuple"
                )
                first_native_verify = auth_job.index(
                    "python3 scripts/release_manifest_signing.py verify"
                )
                stage_signature = auth_job.index(
                    '"$evidence_dir/release_manifest.json.sig"'
                )
                second_native_verify = auth_job.index(
                    "python3 scripts/release_manifest_signing.py verify",
                    first_native_verify + 1,
                )
                upload_auth = auth_job.index(
                    "name: Upload authenticated foundational manifest tuple"
                )
            except ValueError:
                pass
            else:
                if not (
                    reconcile
                    < external_verify
                    <= first_native_verify
                    < stage_signature
                    < second_native_verify
                    < upload_auth
                ):
                    errors.append(
                        f"{relative}: candidate reconciliation, external "
                        "verification, public snapshot, replay verification, and "
                        "authentication upload are out of order"
                    )
        if promotion_job is None:
            errors.append(f"{relative}: missing release promotion job")
        else:
            if promotion_guard not in promotion_job:
                errors.append(
                    f"{relative}: release promotion lacks the reviewed trigger guard"
                )
            if (
                "needs: [release-gate, package, verify-release-auth]"
                not in promotion_job
            ):
                errors.append(
                    f"{relative}: release promotion must depend on protected "
                    "Ed25519 manifest authentication"
                )
            if (
                promotion_job.count(
                    "python3 scripts/generate_sorafs_cli_release_manifest.py check"
                )
                != 1
            ):
                errors.append(
                    f"{relative}: release promotion must reconcile the downloaded "
                    "authenticated manifest with the exact candidate inventory"
                )

        package_matrix = re.search(
            r"(?ms)^  package:\n.*?^      matrix:\n"
            r"(?P<matrix>.*?)^    runs-on: \$\{\{ matrix\.os \}\}",
            source,
        )
        if package_matrix is None:
            errors.append(f"{relative}: malformed native release matrix")
        else:
            target_runners = tuple(
                re.findall(
                    r"(?m)^          - os: ([^\s#]+)\n"
                    r"            target: ([^\s#]+)$",
                    package_matrix.group("matrix"),
                )
            )
            if target_runners != RELEASE_TARGET_RUNNERS:
                errors.append(
                    f"{relative}: native release matrix must contain exactly the "
                    "reviewed Linux, macOS, and Windows target/runner pairs"
                )
        try:
            global_permissions = source[
                source.index("permissions:") : source.index("concurrency:")
            ]
            sign_job = source[source.index("  sign:") :]
        except ValueError:
            errors.append(f"{relative}: malformed permissions or signing job")
        else:
            global_elevated = [
                permission
                for permission in ("id-token:", "attestations:", "artifact-metadata:")
                if permission in global_permissions
            ]
            if global_elevated:
                errors.append(
                    f"{relative}: OIDC and attestation permissions must be signing-job scoped"
                )
            if "permissions:" not in sign_job or "id-token: write" not in sign_job:
                errors.append(f"{relative}: signing job must request OIDC explicitly")
            for permission in ("attestations: write", "artifact-metadata: write"):
                if permission not in sign_job:
                    errors.append(
                        f"{relative}: signing job must request `{permission}` explicitly"
                    )
            if "if: ${{ startsWith(github.ref, 'refs/tags/sorafs-cli-v') || inputs.sign_artifacts }}" not in sign_job:
                errors.append(f"{relative}: signing job lacks the reviewed trigger guard")
            required_targets = re.search(
                r"(?ms)^\s+required_targets=\(\n"
                r"(?P<targets>.*?)^\s+\)$",
                sign_job,
            )
            expected_targets = tuple(
                target for _runner, target in RELEASE_TARGET_RUNNERS
            )
            if required_targets is None:
                errors.append(
                    f"{relative}: signing job lacks the mandatory target inventory"
                )
            else:
                signed_targets = tuple(
                    line.strip()
                    for line in required_targets.group("targets").splitlines()
                    if line.strip()
                )
                if signed_targets != expected_targets:
                    errors.append(
                        f"{relative}: signing job target inventory must exactly "
                        "match the native release matrix"
                    )
            try:
                verify_manifest_binding = sign_job.index(
                    "name: Verify authenticated release-manifest candidate binding before provenance"
                )
                verify_checksums = sign_job.index(
                    "name: Verify platform package checksums before signing"
                )
                attest = sign_job.index("name: Attest release-candidate build provenance")
                stage_attestations = sign_job.index("name: Stage offline provenance bundles")
                sign_blobs = sign_job.index("name: Keyless-sign every release-candidate file")
                upload_signed = sign_job.index("name: Upload signed release candidate")
            except ValueError:
                pass
            else:
                if not (
                    verify_manifest_binding
                    < verify_checksums
                    < attest
                    < stage_attestations
                    < sign_blobs
                    < upload_signed
                ):
                    errors.append(
                        f"{relative}: checksum verification, provenance, signing, and upload are out of order"
                    )
        sbom_action = "anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610"
        scan_action = "anchore/scan-action@e1165082ffb1fe366ebaf02d8526e7c4989ea9d2"
        if source.count(sbom_action) != 2 or source.count(scan_action) != 2:
            errors.append(
                f"{relative}: source and platform packages require exactly one SBOM and vulnerability scan each"
            )
        if source.count("grype-version: v0.112.0") != 2:
            errors.append(
                f"{relative}: source and platform scans must both pin Grype v0.112.0"
            )
        try:
            package_reference = source.index("name: Package reference validator and FFI header")
            reproducible_archive = source.index(
                "name: Rebuild deterministic platform archive and run clean-consumer smoke"
            )
            stage_source_scan = source.index("name: Stage source release scan evidence")
            binary_sbom = source.index("name: Generate platform binary SBOM")
            binary_scan = source.index("name: Scan platform binary SBOM")
            checksums = source.index("name: Finalize platform checksums")
            upload = source.index("name: Upload unsigned release candidate")
        except ValueError:
            pass
        else:
            if not (
                package_reference
                < reproducible_archive
                < stage_source_scan
                < binary_sbom
                < binary_scan
                < checksums
                < upload
            ):
                errors.append(
                    f"{relative}: reference packaging, reproducible archive, "
                    "source evidence, platform SBOM, scan, checksum, and upload "
                    "steps are out of order"
                )
        if source.count("python3 scripts/package_sorafs_cli_candidate.py") != 2:
            errors.append(
                f"{relative}: deterministic platform candidate must be built "
                "exactly twice for byte-identical replay"
            )
        if source.count('cmp \\\n            "${first_out}/${package_name}') != 2:
            errors.append(
                f"{relative}: platform archive and manifest replay comparisons "
                "must both be enforced"
            )
    return errors


def validate_release_automation(root: Path) -> dict[str, Any]:
    """Validate every committed SoraFS workflow and return a closed summary."""

    errors: list[str] = []
    validated: list[str] = []
    for relative in sorted(WORKFLOWS):
        path = _require_regular_repo_file(root, relative)
        try:
            source = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(f"{relative}: workflow must be UTF-8") from error
        errors.extend(_validate_workflow_source(relative, source))
        validated.append(relative)
    for relative, markers in sorted(RELEASE_DOCUMENTS.items()):
        path = _require_regular_repo_file(root, relative)
        try:
            source = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(f"{relative}: release document must be UTF-8") from error
        for marker in markers:
            if marker not in source:
                errors.append(
                    f"{relative}: missing release-document contract marker `{marker}`"
                )
        for stale_claim in FORBIDDEN_RELEASE_DOCUMENT_CLAIMS.get(relative, ()):
            if stale_claim in source:
                errors.append(
                    f"{relative}: stale release-document claim `{stale_claim}`"
                )
    if errors:
        raise ValueError("; ".join(errors))
    return {
        "schema": SCHEMA,
        "workflow_count": len(validated),
        "workflows": validated,
    }


def main() -> int:
    """Run the release-automation validator from the repository root."""

    try:
        summary = validate_release_automation(SCRIPT_DIR.parent)
    except (OSError, ValueError) as error:
        print(f"error: invalid SoraFS release automation: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
