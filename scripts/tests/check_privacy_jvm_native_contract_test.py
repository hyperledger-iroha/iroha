#!/usr/bin/env python3
"""Freeze the fail-closed ABI-22 Kotlin/JVM and Java privacy contract."""

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

    def test_offline_device_registration_result_is_typed_through_sdk_jni(self) -> None:
        jni = read_rust_bridge_source()
        bridge = read(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
            "AuthenticatedTransactionDetailsNativeBridge.kt"
        )
        model = read(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
            "AuthenticatedOfflineDeviceRegistrationResultV1.kt"
        )
        android_model = read(
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
            "AuthenticatedOfflineDeviceRegistrationResultV1.java"
        )
        transport = read(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
            "HttpClientTransport.kt"
        )
        runner = read("ci/check_privacy_jvm_sdk.sh")
        artifact_checker = read("scripts/check_mobile_sdk_artifacts.sh")
        artifact_checker_test = read("scripts/check_mobile_sdk_artifacts_test.sh")
        symbol = (
            "Java_org_hyperledger_iroha_sdk_client_"
            "AuthenticatedTransactionDetailsNativeBridge_"
            "nativeProjectExactOfflineDeviceRegistrationResultV1"
        )
        self.assertEqual(1, jni.count(symbol))
        self.assertIn(symbol, artifact_checker)
        self.assertIn(
            symbol.replace("_sdk_", "_android_"), artifact_checker
        )
        self.assertIn(symbol, artifact_checker_test)
        self.assertIn(
            symbol.replace("_sdk_", "_android_"), artifact_checker_test
        )
        self.assertIn(
            "fun projectCommittedOfflineDeviceRegistrationResultV1(", bridge
        )
        self.assertIn(
            "private external fun nativeProjectExactOfflineDeviceRegistrationResultV1(",
            bridge,
        )
        self.assertIn(
            "class AuthenticatedOfflineDeviceRegistrationResultV1", model
        )
        self.assertIn("JSON_MAX_BYTES = 128 * 1024", model)
        self.assertIn("JSON_MAX_BYTES = 128 * 1024", android_model)
        for field in (
            '"terminal_state"',
            '"eligibility_outcome"',
            '"eligibility_reason"',
            '"matched_rule_ids"',
            '"rejection_code"',
            '"rejection_message"',
        ):
            self.assertIn(field, model)
        self.assertIn(
            "fun getAuthenticatedOfflineDeviceRegistrationResultV1(", transport
        )
        self.assertIn(
            "AuthenticatedOfflineDeviceRegistrationResultV1Test", runner
        )
        self.assertIn(
            "AuthenticatedOfflineDeviceRegistrationResultV1Tests", runner
        )

    def test_exact12_inspector_binds_canonical_auth_authority_across_jni(self) -> None:
        action = read("crates/connect_norito_bridge/src/privacy_exact12_action.rs")
        jni = read_rust_bridge_source()
        kotlin_bridge = read(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/"
            "PrivacyNativeBridge.kt"
        )
        kotlin_transport = read(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
            "HttpClientTransport.kt"
        )
        java_bridge = read(
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/"
            "privacy/PrivacyNativeBridge.java"
        )
        java_transport = read(
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/"
            "client/HttpClientTransport.java"
        )

        for marker in (
            "canonical_authority(authority_literal)?",
            "signed.authority() != &expected_authority",
            "authority differs from canonicalAuth account",
        ):
            self.assertIn(marker, action)
        authority_parser = read(
            "crates/connect_norito_bridge/src/authenticated_transaction_details.rs"
        )
        self.assertIn("AccountId::parse_encoded(authority_literal)", authority_parser)
        self.assertIn("parsed.canonical() != authority_literal", authority_parser)
        self.assertIn("shared_authority_parser_accepts_exact_unicode_i105_only", authority_parser)
        self.assertNotIn("!authority_literal.is_ascii()", authority_parser)
        for marker in (
            '"authorityAccountId"',
            "AUTHENTICATED_TRANSACTION_DETAILS_AUTHORITY_MAX_BYTES_V1",
            "std::str::from_utf8(&authority)",
            "authority: jni::objects::JByteArray<'_>",
        ):
            self.assertIn(marker, jni)
        self.assertIn("authorityAccountId.toByteArray(Charsets.UTF_8)", kotlin_bridge)
        self.assertIn("canonicalAuth.accountId,", kotlin_transport)
        self.assertIn(
            "authorityAccountId.getBytes(StandardCharsets.UTF_8)", java_bridge
        )
        self.assertIn("canonicalAuth.accountId());", java_transport)

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

    def test_exact12_applied_requires_native_finalized_receipt_on_both_jvms(self) -> None:
        jni = read_rust_bridge_source()
        kotlin_bridge = read(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
            "AuthenticatedPrivacyActionReceiptNativeBridge.kt"
        )
        java_bridge = read(
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
            "AuthenticatedPrivacyActionReceiptNativeBridge.java"
        )
        kotlin_model = read(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/"
            "PrivacyExact12ActionModelsV1.kt"
        )
        kotlin_transport = read(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
            "HttpClientTransport.kt"
        )
        java_transport = read(
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
            "HttpClientTransport.java"
        )

        for namespace in ("sdk", "android"):
            prefix = (
                "Java_org_hyperledger_iroha_"
                f"{namespace}_client_AuthenticatedPrivacyActionReceiptNativeBridge_"
            )
            for method in (
                "nativeBridgeAbiVersion",
                "nativePreparePrivacyActionReceiptQueryV1",
                "nativeFinalizePrivacyActionReceiptQueryV1",
                "nativeProjectPrivacyActionReceiptV1",
            ):
                self.assertEqual(1, jni.count(prefix + method))

        for source in (kotlin_bridge, java_bridge):
            for marker in (
                "buildSignedPrivacyActionReceiptQueryV1",
                "nativePreparePrivacyActionReceiptQueryV1",
                "nativeFinalizePrivacyActionReceiptQueryV1",
                "nativeProjectPrivacyActionReceiptV1",
                "requestedActionBinding",
                "capabilityManifestDigest",
                "admittedAtHeight",
                "finalizedBlockHash",
            ):
                self.assertIn(marker, source)

        for marker in (
            "executionCapabilityManifestDigest",
            "executionCapabilityCommittedHeight",
            "executionReceiptFinalizedHeight",
            "executionReceiptFinalizedBlockHash",
            "Applied Exact12 action requires complete authenticated execution-receipt evidence",
        ):
            self.assertIn(marker, kotlin_model)

        for source in (kotlin_transport, java_transport):
            for marker in (
                '"/v1/query"',
                "fetchExactNoritoBytesAllowingNotFound",
                "getOptionalAuthenticatedCommittedTransactionResultV1",
                "getAuthenticatedPrivacyActionExecutionReceiptV1",
                "rejected Exact12 action contradicts an authenticated execution receipt",
                "authenticated Exact12 receipt differs from the committed transaction height",
            ):
                self.assertIn(marker, source)
            self.assertIn("Committed", source)
            self.assertIn("cache", source)

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
        for path_filter in (
            '      - "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/**"',
            '      - "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/**"',
            '      - "IrohaSwift/Sources/IrohaSwift/**"',
            '      - "csharp/src/Hyperledger.Iroha.Sdk/**"',
        ):
            self.assertIn(path_filter, source)
        self.assertNotIn("IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE", job)


if __name__ == "__main__":
    unittest.main()
