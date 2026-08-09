#!/usr/bin/env python3
"""Freeze the authenticated, no-skip ABI-22 Swift privacy lane."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def read(relative: str) -> str:
    """Read one UTF-8 repository file."""

    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def workflow_job(source: str, name: str) -> str:
    """Return one top-level workflow job block."""

    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n.*?(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        source,
    )
    if match is None:
        raise AssertionError(f"missing workflow job: {name}")
    return match.group(0)


class PrivacySwiftNativeContractTests(unittest.TestCase):
    """Guard the release Swift tests against native capability skips."""

    def test_swift_release_test_inventory_has_no_runtime_skip(self) -> None:
        test_roots = (
            REPO_ROOT / "IrohaSwift" / "Tests",
            REPO_ROOT / "IrohaSwift" / "KagemushaCandidateEvidenceLab" / "Tests",
            REPO_ROOT / "examples" / "ios" / "NoritoDemo" / "Tests",
        )
        test_sources = [
            path
            for root in test_roots
            for path in sorted(root.rglob("*.swift"))
        ]
        self.assertTrue(test_sources, "Swift release test inventory is empty")
        for path in test_sources:
            relative = path.relative_to(REPO_ROOT)
            self.assertNotIn("XCTSkip", path.read_text(encoding="utf-8"), relative)
        parity = read(
            "IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift"
        )
        self.assertIn(
            'throw ParityHarnessError.unzipFailed(\n'
            '            "unzip-based bridge materialization is unavailable outside macOS"',
            parity,
        )

    def test_swift_runner_reauthenticates_external_apple_artifact(self) -> None:
        source = read("ci/check_privacy_swift_sdk.sh")
        for marker in (
            '"$(uname -s)" != "Darwin"',
            '"${MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT:-}" != "1"',
            "MOBILE_SDK_APPLE_ARTIFACT_DIR",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "MOBILE_SDK_PYTHON_BINARY",
            "scripts/check_mobile_sdk_artifacts.sh",
            '--apple-only',
            "SorafsOrchestratorParityTests.swift",
            "--disable-automatic-resolution",
            '--scratch-path "${SWIFT_SCRATCH_DIRECTORY}"',
        ):
            self.assertIn(marker, source)
        self.assertLess(
            source.index('bash "${APPLE_ARTIFACT_CHECKER}" --apple-only'),
            source.index('"${SWIFT_BIN}" test'),
        )

    def test_package_manifest_requires_the_external_artifact(self) -> None:
        source = read("IrohaSwift/Package.swift")
        for marker in (
            '"MOBILE_SDK_APPLE_ARTIFACT_DIR"',
            '"MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"',
            "configuredArtifactDirectory == nil",
            "must be outside the reviewed Iroha source tree",
            "requiredBridgeAbiVersion = 22",
            '"NoritoBridge.artifacts.json"',
            'manifest["native_bridge_abi_version"]',
            "validateBridgeArtifact(at: bridgeAbsolutePath)",
        ):
            self.assertIn(marker, source)

    def test_cocoapods_bridge_lint_cannot_capability_skip(self) -> None:
        source = read("scripts/check_swift_pod_bridge.sh")
        self.assertIn(
            'write_summary "failed" "cocoapods CLI not available"',
            source,
        )
        self.assertIn(
            "cocoapods (pod) is required; refusing to skip lint",
            source,
        )
        self.assertNotIn('write_summary "skipped"', source)
        self.assertNotIn("skipping lint", source)
        self.assertIn("MOBILE_SDK_APPLE_ARTIFACT_DIR", source)
        self.assertIn("scripts/check_mobile_sdk_artifacts.sh", source)
        self.assertIn("NoritoBridge artifact authentication failed", source)
        self.assertIn('"--configuration=Release"', source)
        self.assertNotIn('"--allow-warnings"', source)
        self.assertNotIn('"--skip-tests"', source)
        self.assertLess(
            source.index('bash "${ARTIFACT_CHECKER}"'),
            source.index('pod "${LINT_ARGS[@]}"'),
        )

        workflow = read(".github/workflows/mobile_sdk_artifacts.yml")
        apple_job = workflow_job(workflow, "apple-mobile-sdk")
        for trigger in (
            "ci/check_swift_pod_bridge.sh",
            "scripts/check_swift_pod_bridge.sh",
        ):
            self.assertIn(f'      - "{trigger}"', workflow)
        self.assertIn("CocoaPods structural lint (no capability skip)", apple_job)
        self.assertIn(
            "SWIFT_POD_REPORT_DIR: ${{ runner.temp }}/iroha-swift-pod-report",
            apple_job,
        )
        self.assertIn("run: ci/check_swift_pod_bridge.sh", apple_job)

    def test_release_guidance_keeps_cocoapods_delivery_open(self) -> None:
        guide = read("docs/norito_bridge_release.md")
        readme = read("IrohaSwift/README.md")
        plan = read("specs/sorafs_reference_sdk_plan.md")
        for source in (guide, readme, plan):
            self.assertIn("CocoaPods", source)
            self.assertIn("vendored-XCFramework", source)
        self.assertIn("Native CocoaPods publication remains blocked", guide)
        self.assertIn("Generated `dist/*`", guide)
        self.assertIn("only `dist/.gitkeep` belongs in Git", guide)
        self.assertNotIn("swift package compute-checksum", guide)
        self.assertNotIn("Commit the generated artifacts", guide)

    def test_workflow_builds_authenticates_and_tests_exact_apple_artifact(self) -> None:
        source = read(".github/workflows/pr_privacy_sdk_guard.yml")
        job = workflow_job(source, "privacy_swift_sdk_parse")
        for trigger in (
            "IrohaSwift/Package.swift",
            "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
            "IrohaSwift/Tests/IrohaSwiftTests/NativeBridgeLoaderTests.swift",
            "scripts/tests/check_privacy_swift_native_contract_test.py",
        ):
            self.assertIn(f'      - "{trigger}"', source)
        for marker in (
            "runs-on: macos-14",
            "actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065",
            'python-version: "3.12"',
            "update-environment: false",
            "MOBILE_SDK_PYTHON_BINARY",
            '"${HOME}/.cargo/bin/rustup" toolchain install',
            '"1.93.1-aarch64-apple-darwin"',
            "aarch64-apple-ios-sim",
            "x86_64-apple-darwin",
            "cargo fetch --locked",
            "MOBILE_SDK_APPLE_ARTIFACT_DIR",
            "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "NORITO_BRIDGE_OUT_DIR",
            "NORITO_BRIDGE_BUILD_DIR",
            'chmod -R a-w "$GITHUB_WORKSPACE"',
            "scripts/build_norito_xcframework.sh",
            "scripts/check_mobile_sdk_artifacts.sh --apple-only",
            "python3 -I -B scripts/tests/check_privacy_swift_native_contract_test.py",
            "run: ci/check_privacy_swift_sdk.sh",
        ):
            self.assertIn(marker, job)
        for forbidden in (
            "--allow-dirty-source",
            "NORITO_BRIDGE_TEST_PREBUILT_SLICES",
            "MOBILE_SDK_SKIP_BINARY_INSPECTION",
            "NORITO_BRIDGE_PRESERVE_CARGO_TARGETS",
        ):
            self.assertNotIn(forbidden, job)


if __name__ == "__main__":
    unittest.main()
