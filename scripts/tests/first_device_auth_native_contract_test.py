"""First-device auth JNI shipping contract; no device or Native authority qualification."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import types
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "scripts.first_device_auth_contract_checker", REPO_ROOT / "scripts/check_native_sdk_artifact.py"
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def test_both_auth_endpoints_are_required_and_not_wallet_exports() -> None:
    expected = tuple(
        "Java_org_hyperledger_iroha_sdk_crypto_keystore_NativeFirstDeviceAuthKeyJniV1_" + method
        for method in ("reserve", "restore")
    )
    assert MODULE.FIRST_DEVICE_AUTH_JNI_EXPORTS == expected
    assert set(expected).isdisjoint(MODULE.KAGEMUSHA_WALLET_JNI_EXPORTS)
    required = MODULE.REQUIRED_SYMBOLS["c-jni"]
    assert all(required.count(symbol) == 1 for symbol in expected)
    # Independent auth-key ownership grants no new monetary C surface.
    assert set(expected).isdisjoint(MODULE.REQUIRED_SYMBOLS["csharp"])


def test_old_bridge_missing_either_auth_endpoint_is_refused() -> None:
    required = MODULE.REQUIRED_SYMBOLS["c-jni"]
    for missing in MODULE.FIRST_DEVICE_AUTH_JNI_EXPORTS:
        library = types.SimpleNamespace(**{symbol: object() for symbol in required if symbol != missing})
        with mock.patch.object(MODULE.ctypes, "CDLL", return_value=library):
            try:
                MODULE.probe_c_abi(Path("source-contract-only-library"), required)
            except MODULE.ArtifactContractError as error:
                assert str(error) == "native C ABI artifact is missing required symbols: " + missing
            else:
                raise AssertionError("Old bridge accepted without auth JNI endpoint: " + missing)


def test_android_release_inventory_and_native_definitions_match_sdk() -> None:
    consumer = (REPO_ROOT / "kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/NativeFirstDeviceAuthKeyOwnerV1.kt").read_text()
    declared = set(re.findall(r"external\s+fun\s+(\w+)\s*\(", consumer))
    assert declared == {"reserve", "restore"}
    owner = "Java_org_hyperledger_iroha_sdk_crypto_keystore_NativeFirstDeviceAuthKeyJniV1_"
    expected = MODULE.FIRST_DEVICE_AUTH_JNI_EXPORTS
    assert set(expected) == {owner + method for method in declared}
    source = (REPO_ROOT / "crates/connect_norito_bridge/src/platform_jni/first_device_auth_key_v1.rs").read_text()
    defined = re.findall(r'pub\s+extern\s+"system"\s+fn\s+(Java_\w+)\s*\(', source)
    assert len(defined) == len(set(defined)) == 2
    assert set(defined) == set(expected)
    checker = (REPO_ROOT / "scripts/check_mobile_sdk_artifacts.sh").read_text()
    inventory = checker.split("REQUIRED_AUTH_JNI_SYMBOLS=(\n", 1)[1].split("\n)", 1)[0]
    assert tuple(inventory.split()) == expected
    assert 'for symbol in "${REQUIRED_WALLET_JNI_SYMBOLS[@]}" "${REQUIRED_AUTH_JNI_SYMBOLS[@]}"; do' in checker


def test_private_upcalls_and_reply_constructors_have_shipping_r8_rules() -> None:
    rules = (REPO_ROOT / "kotlin/client-android/consumer-rules.pro").read_text()
    for name in ("FirstDeviceAuthNativePlatformV1", "NativeFirstDeviceAuthKeyJniV1",
                 "NativeFirstDeviceAuthPlatformReplyV1", "NativeFirstDeviceAuthKeyReplyV1"):
        assert rules.count("-keep class org.hyperledger.iroha.sdk.crypto.keystore." + name + " { *; }") == 1
    consumer = (REPO_ROOT / "kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/NativeFirstDeviceAuthKeyOwnerV1.kt").read_text()
    for name in ("requireOriginalFromNative", "transcriptFromNative", "apiLevelFromNative",
                 "noBackupRootFromNative", "probeFromNative", "generateFromConsumedNativeIntent"):
        assert re.search(r"private\s+fun\s+" + name + r"\s*\(", consumer)


def test_both_native_modules_are_registered_in_shipping_crate() -> None:
    library = (REPO_ROOT / "crates/connect_norito_bridge/src/lib.rs").read_text()
    jni = (REPO_ROOT / "crates/connect_norito_bridge/src/platform_jni.rs").read_text()
    for source in (library, jni):
        assert source.count('#[cfg(unix)]\nmod first_device_auth_key_v1;') == 1
