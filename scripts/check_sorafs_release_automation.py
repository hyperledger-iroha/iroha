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
        '- "scripts/requirements.txt"',
        "python3 -m pip install -r scripts/requirements.txt",
        "python3 scripts/check_workflow_action_pins.py",
        "scripts/tests/check_sorafs_reference_sdk_release_evidence_test.py",
        "run: bash ci/check_sorafs_cli_release.sh",
        "scripts/package_sorafs_validate_release.sh",
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
        "name: Finalize platform checksums",
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
                    verify_checksums
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
            binary_sbom = source.index("name: Generate platform binary SBOM")
            binary_scan = source.index("name: Scan platform binary SBOM")
            checksums = source.index("name: Finalize platform checksums")
            upload = source.index("name: Upload unsigned release candidate")
        except ValueError:
            pass
        else:
            if not package_reference < binary_sbom < binary_scan < checksums < upload:
                errors.append(
                    f"{relative}: reference packaging, platform SBOM, scan, checksum, and upload steps are out of order"
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
