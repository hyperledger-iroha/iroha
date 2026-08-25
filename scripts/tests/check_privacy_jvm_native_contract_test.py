#!/usr/bin/env python3
"""Freeze the fail-closed ABI-23 Kotlin/JVM and Java privacy contract."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
RUST_BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
RUST_BRIDGE_PLATFORM_JNI = "crates/connect_norito_bridge/src/platform_jni.rs"
RUST_BRIDGE_PLATFORM_JNI_PARTS = (
    "crates/connect_norito_bridge/src/platform_jni/part_1.rs",
    "crates/connect_norito_bridge/src/platform_jni/part_2.rs",
    "crates/connect_norito_bridge/src/platform_jni/part_3.rs",
)
RUST_BRIDGE_PLATFORM_JNI_INCLUDES = (
    "platform_jni/part_1.rs",
    "platform_jni/part_2.rs",
    "platform_jni/part_3.rs",
)


def read(relative: str) -> str:
    """Read one UTF-8 repository file."""

    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def read_rust_bridge_source() -> str:
    """Read the authenticated split JNI source closure."""

    paths = (RUST_BRIDGE, RUST_BRIDGE_PLATFORM_JNI, *RUST_BRIDGE_PLATFORM_JNI_PARTS)
    for relative in paths:
        path = REPO_ROOT / relative
        if path.is_symlink() or not path.is_file():
            raise AssertionError(f"required Rust bridge source is unavailable: {relative}")
    bridge = read(RUST_BRIDGE)
    if len(re.findall(r"^mod platform_jni;$", bridge, flags=re.MULTILINE)) != 1:
        raise AssertionError("Rust bridge must own exactly one platform_jni module")
    platform_jni = read(RUST_BRIDGE_PLATFORM_JNI)
    observed_includes = tuple(
        re.findall(r'^include!\("([^"]+)"\);$', platform_jni, flags=re.MULTILINE)
    )
    if observed_includes != RUST_BRIDGE_PLATFORM_JNI_INCLUDES:
        raise AssertionError(
            "Rust bridge platform_jni include closure differs from the exact "
            f"three-part inventory: found {observed_includes}"
        )
    return "\n".join(read(relative) for relative in paths)


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
                "compiledProfileCatalogRoundTripsAndRejectsAdversarialBytesThroughNativeAbi23()",
                "PrivacyNativeBridge.compiledProfileCatalogV1()",
                "compiled-profile catalog JNI exports is required",
            ),
            (
                "exact12FixtureBundleRoundTripsAndRejectsAdversarialBytesThroughNativeAbi23()",
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
                "compiledProfileCatalogRoundTripsAndRejectsAdversarialBytesThroughNativeAbi23()",
                "PrivacyNativeBridge.compiledProfileCatalogV1()",
                "compiled-profile catalog JNI exports is required",
            ),
            (
                "exact12FixtureBundleRoundTripsAndRejectsAdversarialBytesThroughNativeAbi23()",
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
        source = read_rust_bridge_source()
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
            "Upload source-bound privacy JVM native ABI23 input",
            "privacy-jvm-native-abi22-${{ github.sha }}",
        ):
            self.assertIn(marker, job)
        self.assertNotIn("IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE", job)


if __name__ == "__main__":
    unittest.main()
