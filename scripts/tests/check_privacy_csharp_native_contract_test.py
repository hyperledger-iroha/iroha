#!/usr/bin/env python3
"""Freeze the fail-closed ABI-21 C# privacy test and workflow contract."""

from __future__ import annotations

import importlib.util
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


class PrivacyCsharpNativeContractTests(unittest.TestCase):
    """Guard C# exact-12 tests against native capability skips."""

    def test_exact12_test_requires_native_bridge_unconditionally(self) -> None:
        source = read(
            "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"
        )
        method_name = (
            "Exact12FixtureBundleRoundTripsAndRejectsAdversarialBytes()"
        )
        start = source.index(method_name)
        native_assertion = source.index("var bundle =", start)
        preflight = source[start:native_assertion]

        self.assertIn(
            "Assert.True(\n"
            "            PrivacyNative.IsAvailable(),\n"
            "            \"ABI-21 connect_norito_bridge with exact-12 fixture "
            "symbols is required.\");",
            preflight,
        )
        self.assertNotIn("WhenAvailable", source)
        self.assertNotIn("IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE", source)
        self.assertNotIn("if (!PrivacyNative.IsAvailable())", source)

    def test_csharp_artifact_manifest_requires_all_privacy_symbols(self) -> None:
        checker_path = REPO_ROOT / "scripts/check_native_sdk_abi21_artifact.py"
        spec = importlib.util.spec_from_file_location(
            "privacy_csharp_abi21_checker",
            checker_path,
        )
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(checker)

        self.assertEqual(
            checker.REQUIRED_SYMBOLS["csharp"],
            (
                "connect_norito_bridge_abi_version",
                "connect_norito_free",
                "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
                "iroha_privacy_capabilities_v1",
                "iroha_privacy_validate_capabilities_v1",
                "iroha_privacy_exact12_fixture_bundle_v1",
                "iroha_privacy_validate_exact12_fixture_bundle_v1",
                "iroha_privacy_free_buffer",
            ),
        )

    def test_csharp_runner_authenticates_the_only_loadable_bridge(self) -> None:
        source = read("ci/check_privacy_csharp_sdk.sh")
        for marker in (
            '"${IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE:-}" != "1"',
            "PRIVACY_CSHARP_NATIVE_ARTIFACT",
            "PRIVACY_CSHARP_NATIVE_MANIFEST",
            "scripts/check_native_sdk_abi21_artifact.py",
            '"${PYTHON_BIN}" -I -B "${ABI21_ARTIFACT_CHECKER}" verify',
            '--artifact "${PRIVACY_CSHARP_NATIVE_ARTIFACT}"',
            '--manifest "${PRIVACY_CSHARP_NATIVE_MANIFEST}"',
            '--source-root "${ROOT_DIR}"',
            '"${LD_LIBRARY_PATH:-}" != "${NATIVE_DIRECTORY}"',
            "FullyQualifiedName~PrivacyNativeTests",
        ):
            self.assertIn(marker, source)

    def test_workflow_builds_authenticates_and_consumes_bridge(self) -> None:
        source = read(".github/workflows/pr_privacy_sdk_guard.yml")
        native = workflow_job(source, "privacy_native_bridge_tests")
        csharp = workflow_job(source, "privacy_csharp_sdk_tests")

        for path in (
            '      - "scripts/check_native_sdk_abi21_artifact.py"',
            '      - "scripts/tests/check_privacy_csharp_native_contract_test.py"',
        ):
            self.assertIn(path, source)
        for marker in (
            "cargo build --locked --release -p connect_norito_bridge",
            "scripts/check_native_sdk_abi21_artifact.py record",
            "--sdk csharp",
            "scripts/check_native_sdk_abi21_artifact.py verify",
            "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
            "privacy-csharp-native-${{ github.sha }}",
        ):
            self.assertIn(marker, native)
        for marker in (
            "needs: privacy_native_bridge_tests",
            "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093",
            "python3 -I -B scripts/tests/check_privacy_csharp_native_contract_test.py",
            'IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE: "1"',
            "LD_LIBRARY_PATH: ${{ runner.temp }}/iroha-privacy-csharp-native",
            "PRIVACY_CSHARP_NATIVE_ARTIFACT: ${{ runner.temp }}/"
            "iroha-privacy-csharp-native/libconnect_norito_bridge.so",
            "PRIVACY_CSHARP_NATIVE_MANIFEST: ${{ runner.temp }}/"
            "iroha-privacy-csharp-native/native-sdk-abi21.json",
            "run: ci/check_privacy_csharp_sdk.sh",
        ):
            self.assertIn(marker, csharp)

        ordered = (
            csharp.index("actions/setup-dotnet@"),
            csharp.index("actions/download-artifact@"),
            csharp.index("C# privacy native fail-closed contract"),
            csharp.index("Privacy C# SDK tests"),
        )
        self.assertEqual(ordered, tuple(sorted(ordered)))


if __name__ == "__main__":
    unittest.main()
