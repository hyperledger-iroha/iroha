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
        self.assertEqual(91, result.pair_count)
        self.assertEqual(GUARD.EXPECTED_ABI_DIGEST, result.abi_digest)
        self.assertEqual(GUARD.EXPECTED_ATTRIBUTE_DIGEST, result.attribute_digest)

    def test_rejects_android_symbol_drift(self) -> None:
        mutated = SOURCE.replace(
            "Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignDetached();",
            "Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignDetachedV2();",
            1,
        )
        with self.assertRaisesRegex(GUARD.AuditError, "suffix mismatch"):
            GUARD.audit_source(mutated)

    def test_rejects_helper_argument_reordering(self) -> None:
        mutated = SOURCE.replace(
            "java_native_public_key_from_private(&mut env, algorithm_code, private_key)",
            "java_native_public_key_from_private(&mut env, private_key, algorithm_code)",
            1,
        )
        with self.assertRaisesRegex(GUARD.AuditError, "signature/body contract changed"):
            GUARD.audit_source(mutated)

    def test_rejects_platform_documentation_drift(self) -> None:
        mutated = SOURCE.replace(
            "Validate a Torii Exact12 capability manifest for the Java Android SDK.",
            "Validate a Torii Exact12 capability manifest for Android.",
            1,
        )
        with self.assertRaisesRegex(GUARD.AuditError, "documentation/attribute contract changed"):
            GUARD.audit_source(mutated)

    def test_rejects_macro_expansion_drift(self) -> None:
        mutated = SOURCE.replace(
            ") $(-> $return_type:ty)? $body:block\n        )*",
            ") -> $return_type:ty $body:block\n        )*",
            1,
        )
        with self.assertRaisesRegex(GUARD.AuditError, "macro expansion contract changed"):
            GUARD.audit_source(mutated)

    def test_rejects_typed_kagemusha_forwarder_drift(self) -> None:
        mutated = SOURCE.replace(
            "nativeArtifactWriteV4 { handle long, chunk bytes }",
            "nativeArtifactWriteV4 { chunk bytes, handle long }",
            1,
        )
        with self.assertRaisesRegex(GUARD.AuditError, "forwarder invocation contract changed"):
            GUARD.audit_source(mutated)


if __name__ == "__main__":
    unittest.main()
