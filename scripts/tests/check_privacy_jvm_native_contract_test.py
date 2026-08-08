#!/usr/bin/env python3
"""Freeze the fail-closed ABI-21 Kotlin/JVM and Java privacy contract."""

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
                "compiledProfileCatalogRoundTripsAndRejectsAdversarialBytes()",
                "PrivacyNativeBridge.compiledProfileCatalogV1()",
                "compiled-profile catalog JNI exports is required",
            ),
            (
                "exact12FixtureBundleRoundTripsAndRejectsAdversarialBytes()",
                "PrivacyNativeBridge.exact12FixtureBundleV1()",
                "exact-12 fixture JNI exports is required",
            ),
        ):
            start = source.index(method)
            preflight = source[start : source.index(native_call, start)]
            self.assertIn(
                "assertTrue(\n            PrivacyNativeBridge.isNativeAvailable(),",
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
                "compiledProfileCatalogRoundTripsAndRejectsAdversarialBytes()",
                "PrivacyNativeBridge.compiledProfileCatalogV1()",
                "compiled-profile catalog JNI exports is required",
            ),
            (
                "exact12FixtureBundleRoundTripsAndRejectsAdversarialBytes()",
                "PrivacyNativeBridge.exact12FixtureBundleV1()",
                "exact-12 fixture JNI exports is required",
            ),
        ):
            self.assertIn(f"    {method};", source)
            start = source.index(f"private static void {method}")
            preflight = source[start : source.index(native_call, start)]
            self.assertIn(
                "if (!PrivacyNativeBridge.isNativeAvailable()) {",
                preflight,
            )
            self.assertIn("throw new AssertionError(", preflight)
            self.assertIn(message, preflight)
        for forbidden in (
            "WhenAvailable",
            "IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE",
            "if (!available)",
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
            "PRIVACY_JVM_NATIVE_ARTIFACT",
            "PRIVACY_JVM_NATIVE_MANIFEST",
            "scripts/check_native_sdk_abi21_artifact.py",
            '"${PYTHON_BIN}" -I -B "${ABI21_ARTIFACT_CHECKER}" verify',
            '"${IROHA_NATIVE_LIBRARY_PATH:-}" != "${NATIVE_DIRECTORY}"',
            '"${LD_LIBRARY_PATH:-}" != "${NATIVE_DIRECTORY}"',
            '-Djava.library.path="${NATIVE_DIRECTORY}"',
            "org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridgeTest",
            "org.hyperledger.iroha.android.privacy.PrivacyNativeBridgeTest",
        ):
            self.assertIn(marker, source)

    def test_workflow_downloads_and_threads_authenticated_bridge(self) -> None:
        source = read(".github/workflows/pr_privacy_sdk_guard.yml")
        job = workflow_job(source, "privacy_jvm_sdk_tests")
        self.assertIn(
            '      - "scripts/tests/check_privacy_jvm_native_contract_test.py"',
            source,
        )
        for marker in (
            "needs: privacy_native_bridge_tests",
            "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093",
            "privacy-csharp-native-${{ github.sha }}",
            "python3 -I -B scripts/tests/check_privacy_jvm_native_contract_test.py",
            "IROHA_NATIVE_LIBRARY_PATH:",
            "LD_LIBRARY_PATH:",
            "PRIVACY_JVM_NATIVE_ARTIFACT:",
            "PRIVACY_JVM_NATIVE_MANIFEST:",
            "PRIVACY_JVM_PYTHON_BIN: python3",
            "run: ci/check_privacy_jvm_sdk.sh",
        ):
            self.assertIn(marker, job)
        self.assertNotIn("IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE", job)


if __name__ == "__main__":
    unittest.main()
