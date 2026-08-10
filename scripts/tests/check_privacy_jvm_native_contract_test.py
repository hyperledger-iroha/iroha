#!/usr/bin/env python3
"""Freeze the fail-closed ABI-22 Kotlin/JVM and Java privacy contract."""

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


class PrivacyJvmNativeContractTests(unittest.TestCase):
    """Guard both JVM SDKs against native capability skips."""

    def test_kotlin_native_tests_require_the_bridge_unconditionally(self) -> None:
        source = read(
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/"
            "privacy/PrivacyNativeBridgeTest.kt"
        )
        for method, native_call, message in (
            (
                "compiledProfileCatalogRoundTripsAndRejectsAdversarialBytesThroughNativeAbi22()",
                "PrivacyNativeBridge.compiledProfileCatalogV1()",
                "compiled-profile catalog JNI exports is required",
            ),
            (
                "exact12FixtureBundleRoundTripsAndRejectsAdversarialBytesThroughNativeAbi22()",
                "PrivacyNativeBridge.exact12FixtureBundleV1()",
                "exact-12 fixture JNI exports is required",
            ),
        ):
            start = source.index(method)
            preflight = source[start : source.index(native_call, start)]
            self.assertIn(
                "val available = PrivacyNativeBridge.isNativeAvailable()",
                preflight,
            )
            self.assertIn(
                "assertTrue(\n            available,",
                preflight,
            )
            self.assertIn(message, preflight)
        for forbidden in (
            "WhenAvailable",
            "IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE",
            "if (!available) return",
        ):
            self.assertNotIn(forbidden, source)

    def test_java_native_tests_require_the_bridge_unconditionally(self) -> None:
        source = read(
            "java/iroha_android/src/test/java/org/hyperledger/iroha/android/"
            "privacy/PrivacyNativeBridgeTest.java"
        )
        for method, native_call, message in (
            (
                "compiledProfileCatalogRoundTripsAndRejectsAdversarialBytesThroughNativeAbi22()",
                "PrivacyNativeBridge.compiledProfileCatalogV1()",
                "compiled-profile catalog JNI exports is required",
            ),
            (
                "exact12FixtureBundleRoundTripsAndRejectsAdversarialBytesThroughNativeAbi22()",
                "PrivacyNativeBridge.exact12FixtureBundleV1()",
                "exact-12 fixture JNI exports is required",
            ),
        ):
            self.assertIn(f"    {method};", source)
            start = source.index(f"private static void {method}")
            preflight = source[start : source.index(native_call, start)]
            self.assertIn(
                "final boolean available = PrivacyNativeBridge.isNativeAvailable();",
                preflight,
            )
            self.assertIn(
                "if (!available) {",
                preflight,
            )
            self.assertIn("throw new AssertionError(", preflight)
            self.assertIn(message, preflight)
        for forbidden in (
            "WhenAvailable",
            "IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE",
            "if (!available) return",
        ):
            self.assertNotIn(forbidden, source)

    def test_rust_bridge_exports_both_privacy_jni_namespaces(self) -> None:
        source = read("crates/connect_norito_bridge/src/lib.rs")
        for namespace in ("sdk", "android"):
            prefix = (
                "Java_org_hyperledger_iroha_"
                f"{namespace}_privacy_PrivacyNativeBridge_"
            )
            for method in (
                "nativeBridgeAbiVersion",
                "nativeCompiledProfileCatalog",
                "nativeValidateCompiledProfileCatalog",
                "nativeExact12FixtureBundle",
                "nativeValidateExact12FixtureBundle",
            ):
                self.assertEqual(1, source.count(prefix + method))

    def test_jvm_runner_authenticates_the_only_loadable_bridge(self) -> None:
        source = read("ci/check_privacy_jvm_sdk.sh")
        for marker in (
            'ABI22_CHECKER="${ROOT_DIR}/scripts/check_native_sdk_abi22_artifact.py"',
            '"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify',
            'export IROHA_NATIVE_LIBRARY_PATH="${NATIVE_LIBRARY_DIR}"',
            'export LD_LIBRARY_PATH="${NATIVE_LIBRARY_DIR}"',
            '-Djava.library.path="${NATIVE_LIBRARY_DIR}"',
            "org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridgeTest",
            "org.hyperledger.iroha.android.privacy.PrivacyNativeBridgeTest",
        ):
            self.assertIn(marker, source)

    def test_workflow_builds_and_exports_authenticated_bridge(self) -> None:
        source = read(".github/workflows/pr_privacy_sdk_guard.yml")
        job = workflow_job(source, "privacy_jvm_sdk_tests")
        self.assertIn(
            '      - "scripts/tests/check_privacy_jvm_native_contract_test.py"',
            source,
        )
        for marker in (
            "PRIVACY_JVM_SDK_PYTHON_BIN: ${{ steps.privacy-jvm-python.outputs.python-path }}",
            "PRIVACY_JVM_NATIVE_EXPORT_DIR: ${{ runner.temp }}/privacy-jvm-native-abi22",
            "run: ci/check_privacy_jvm_sdk.sh",
            "Upload source-bound privacy JVM native ABI22 input",
            "privacy-jvm-native-abi22-${{ github.sha }}",
        ):
            self.assertIn(marker, job)
        self.assertNotIn("IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE", job)


if __name__ == "__main__":
    unittest.main()
