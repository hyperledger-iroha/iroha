#!/usr/bin/env python3
"""Tests for the paired Kotlin/JVM and Java/Android JNI source guard."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts/check_jni_sdk_android_pairs.py"
SPEC = importlib.util.spec_from_file_location("check_jni_sdk_android_pairs", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("failed to load JNI pair guard")
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)
SOURCE = (
    REPO_ROOT / "crates/connect_norito_bridge/src/platform_jni/part_3.rs"
).read_text(encoding="utf-8")


class JniSdkAndroidPairGuardTests(unittest.TestCase):
    """Keep pair expansion, symbols, signatures, bodies, and attributes exact."""

    def test_repository_inventory_is_exact(self) -> None:
        result = GUARD.audit_source(SOURCE)
        self.assertEqual(29, result.pair_count)
        self.assertEqual(9, result.sdk_only_count)
        self.assertEqual(GUARD.EXPECTED_ABI_DIGEST, result.abi_digest)
        self.assertEqual(GUARD.EXPECTED_ATTRIBUTE_DIGEST, result.attribute_digest)

    def test_rejects_android_symbol_drift(self) -> None:
        mutated = SOURCE.replace(
            "Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignDetached();",
            "Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignDetachedV2();",
            1,
        )
        self.assertNotEqual(SOURCE, mutated, "mutation must alter the guarded source")
        with self.assertRaisesRegex(GUARD.AuditError, "suffix mismatch"):
            GUARD.audit_source(mutated)

    def test_rejects_helper_argument_reordering(self) -> None:
        mutated = SOURCE.replace(
            "java_native_public_key_from_private(&mut env, algorithm_code, private_key)",
            "java_native_public_key_from_private(&mut env, private_key, algorithm_code)",
            1,
        )
        self.assertNotEqual(SOURCE, mutated, "mutation must alter the guarded source")
        with self.assertRaisesRegex(GUARD.AuditError, "signature/body contract changed"):
            GUARD.audit_source(mutated)

    def test_rejects_platform_documentation_drift(self) -> None:
        mutated = SOURCE.replace(
            "Validate a Torii Exact12 capability manifest for the Kotlin/JVM SDK.",
            "Validate a Torii Exact12 capability manifest for SDK.",
            1,
        )
        self.assertNotEqual(SOURCE, mutated, "mutation must alter the guarded source")
        with self.assertRaisesRegex(GUARD.AuditError, "documentation/attribute contract changed"):
            GUARD.audit_source(mutated)

    def test_rejects_macro_expansion_drift(self) -> None:
        mutated = SOURCE.replace(
            ") $(-> $return_type)? $body\n            $(#[$android_attribute])*",
            ") $(-> $return_type)? { $body }\n            $(#[$android_attribute])*",
            1,
        )
        self.assertNotEqual(SOURCE, mutated, "mutation must alter the guarded source")
        with self.assertRaisesRegex(GUARD.AuditError, "macro expansion contract changed"):
            GUARD.audit_source(mutated)

    def test_rejects_retired_privacy_methods_without_network_binding(self) -> None:
        for method in (
            "nativeValidateExact12CapabilityManifest",
            "nativeRequireExact12CapabilityTuple",
            "nativeValidateExact12SubmitProofConstruction",
        ):
            with self.subTest(method=method):
                mutated = SOURCE.replace(method + "ForNetworkV1", method)
                self.assertNotEqual(SOURCE, mutated)
                with self.assertRaisesRegex(GUARD.AuditError, "inventory changed"):
                    GUARD.audit_source(mutated)

    def test_rejects_dropped_expected_network_parameter(self) -> None:
        mutated = SOURCE.replace(
            "    expected_network: jni::objects::JByteArray<'_>,\n", "", 1,
        )
        self.assertNotEqual(SOURCE, mutated)
        with self.assertRaisesRegex(GUARD.AuditError, "signature/body contract changed"):
            GUARD.audit_source(mutated)

    def test_rejects_every_retired_android_privacy_export(self) -> None:
        for suffix in GUARD.EXPECTED_SDK_ONLY_SUFFIXES:
            with self.subTest(suffix=suffix):
                with self.assertRaisesRegex(GUARD.AuditError, "retired Android"):
                    GUARD.audit_source(SOURCE + "\n" + GUARD.ANDROID_PREFIX + suffix)

    def test_confidential_exports_have_one_kotlin_owner(self) -> None:
        source = (REPO_ROOT / "crates/connect_norito_bridge/src/confidential_note_ffi.rs").read_text()
        GUARD.audit_confidential_source(source)
        for method in GUARD.CONFIDENTIAL_PRIVACY_METHODS:
            with self.subTest(method=method):
                symbol = "Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_" + method
                with self.assertRaisesRegex(GUARD.AuditError, "inventory changed"):
                    GUARD.audit_confidential_source(source.replace(symbol, "removed", 1))
                with self.assertRaisesRegex(GUARD.AuditError, "retired Android"):
                    GUARD.audit_confidential_source(source + "\n" + symbol.replace("iroha_sdk_", "iroha_android_"))


if __name__ == "__main__":
    unittest.main()
