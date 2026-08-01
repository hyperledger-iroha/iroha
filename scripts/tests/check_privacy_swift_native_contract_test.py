#!/usr/bin/env python3
"""Freeze the authenticated, no-skip ABI-21 Swift privacy lane."""

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
            "validateBridgeArtifact(at: bridgeAbsolutePath)",
        ):
            self.assertIn(marker, source)

    def test_workflow_builds_authenticates_and_tests_exact_apple_artifact(self) -> None:
        source = read(".github/workflows/pr_privacy_sdk_guard.yml")
        job = workflow_job(source, "privacy_swift_sdk_parse")
        self.assertIn(
            '      - "scripts/tests/check_privacy_swift_native_contract_test.py"',
            source,
        )
        for marker in (
            "runs-on: macos-14",
            "actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065",
            'python-version: "3.12"',
            "update-environment: false",
            "MOBILE_SDK_PYTHON_BINARY",
            '"${HOME}/.cargo/bin/rustup" toolchain install',
            '"1.93.1"',
            "aarch64-apple-ios-sim",
            "x86_64-apple-darwin",
            "cargo fetch --locked",
            "MOBILE_SDK_APPLE_ARTIFACT_DIR",
            "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1",
            "MOBILE_SDK_SWIFT_SCRATCH_DIR",
            "NORITO_BRIDGE_OUT_DIR",
            "NORITO_BRIDGE_BUILD_DIR",
            'chmod -R a-w "$GITHUB_WORKSPACE"',
            "NORITO_BRIDGE_PRESERVE_CARGO_TARGETS=1",
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
        ):
            self.assertNotIn(forbidden, job)


if __name__ == "__main__":
    unittest.main()
