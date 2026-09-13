"""Focused first-release contract tests for native SDK artifact inspection."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import types
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts/check_native_sdk_abi23_artifact.py"
SPEC = importlib.util.spec_from_file_location("check_native_sdk_abi23_artifact", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

KAGEMUSHA_V1_C_SYMBOLS = {
    "connect_norito_kagemusha_v1_payment_request_validate",
    "connect_norito_kagemusha_v1_payment_validate",
    "connect_norito_kagemusha_v1_acknowledgement_validate",
    "connect_norito_kagemusha_v1_complete_exchange_validate",
    "connect_norito_kagemusha_v1_mint_authorization_validate",
    "connect_norito_kagemusha_v1_mint_credit_validate",
    "connect_norito_kagemusha_v1_mint_credit_against_authorization_validate",
    "connect_norito_kagemusha_v1_redemption_voucher_validate",
    "connect_norito_kagemusha_v1_payment_request_text_validate",
    "connect_norito_kagemusha_v1_payment_text_validate",
    "connect_norito_kagemusha_v1_acknowledgement_text_validate",
    "connect_norito_kagemusha_v1_complete_exchange_text_validate",
    "connect_norito_kagemusha_v1_mint_authorization_text_validate",
    "connect_norito_kagemusha_v1_mint_credit_text_validate",
    "connect_norito_kagemusha_v1_mint_credit_against_authorization_text_validate",
    "connect_norito_kagemusha_v1_redemption_voucher_text_validate",
    "connect_norito_kagemusha_device_mint_stage_command_v1_validate",
    "connect_norito_kagemusha_device_mint_stage_result_v1_validate",
    "connect_norito_kagemusha_contract_vector_v1",
    "connect_norito_kagemusha_core_coordinator_contract_v1",
    "connect_norito_kagemusha_core_coordinator_open_v1",
    "connect_norito_kagemusha_core_coordinator_invoke_v1",
    "connect_norito_kagemusha_device_capabilities_v1",
    "connect_norito_kagemusha_device_execute_v1",
    "connect_norito_kagemusha_device_command_response_v1_verify",
    "connect_norito_kagemusha_reserve_finality_hint_v1",
    "connect_norito_kagemusha_reserve_finality_verify_v1",
    "connect_norito_kagemusha_top_up_signed_request_validate_v1",
}
RETIRED_KAGEMUSHA_C_PREFIX = (
    "connect_norito_" + "_".join(reversed(("cash", "offline"))) + "_"
)


def test_native_c_contracts_require_complete_kagemusha_v1() -> None:
    assert len(KAGEMUSHA_V1_C_SYMBOLS) == 28
    for sdk in ("c-jni", "csharp"):
        required = [
            symbol for symbol in MODULE.REQUIRED_SYMBOLS[sdk]
            if symbol.startswith("connect_norito_kagemusha_")
        ]
        assert len(required) == len(KAGEMUSHA_V1_C_SYMBOLS)
        assert set(required) == KAGEMUSHA_V1_C_SYMBOLS


def test_native_privacy_inventory_requires_authoritative_capability_validator() -> None:
    missing = "iroha_privacy_validate_exact12_capability_manifest_v1"
    assert len(MODULE.APPROVED_PRIVACY_C_EXPORTS) == 6
    assert missing in MODULE.REQUIRED_SYMBOLS["csharp"]
    symbols = [symbol for symbol in MODULE.APPROVED_PRIVACY_C_EXPORTS if symbol != missing]
    try:
        MODULE.validate_privacy_c_exports(symbols, require_exact=True)
    except MODULE.ArtifactContractError as error:
        assert str(error) == (
            "native bridge artifact is missing approved privacy C symbols: " + missing
        )
    else:
        raise AssertionError("native privacy artifact without capability validator was accepted")


def test_native_c_probe_rejects_missing_privacy_capability_validator() -> None:
    missing = "iroha_privacy_validate_exact12_capability_manifest_v1"
    library = types.SimpleNamespace(**{
        symbol: object() for symbol in MODULE.REQUIRED_SYMBOLS["csharp"]
        if symbol != missing
    })
    with mock.patch.object(MODULE.ctypes, "CDLL", return_value=library):
        try:
            MODULE.probe_c_abi(Path("test-only-library"), MODULE.REQUIRED_SYMBOLS["csharp"])
        except MODULE.ArtifactContractError as error:
            assert str(error) == (
                "native C ABI artifact is missing required symbols: " + missing
            )
        else:
            raise AssertionError("C# native probe accepted missing capability validator")


def test_coordinator_jni_requires_the_kotlin_sdk_owner() -> None:
    required = set(MODULE.REQUIRED_SYMBOLS["c-jni"])
    assert {
        "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeContractV1",
        "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeOpenV1",
        "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeInvokeV1",
    } <= required
    assert not any(symbol.startswith("Java_pg_") for symbol in required)
    retired = "".join(reversed(("NativeCore", "Offline")))
    assert not any(retired in symbol for symbol in required)


def test_reserve_finality_rejects_each_missing_kotlin_jni_endpoint() -> None:
    required = MODULE.REQUIRED_SYMBOLS["c-jni"]
    for method in ("nativeBridgeAbiVersion", "nativeHint", "nativeVerify"):
        missing = "Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaReserveFinalityJniV1_" + method
        assert missing in required
        library = types.SimpleNamespace(**{
            symbol: object() for symbol in required if symbol != missing
        })
        with mock.patch.object(MODULE.ctypes, "CDLL", return_value=library):
            try:
                MODULE.probe_c_abi(Path("test-only-library"), required)
            except MODULE.ArtifactContractError as error:
                assert str(error) == "native C ABI artifact is missing required symbols: " + missing
            else:
                raise AssertionError("native probe accepted missing reserve finality endpoint")


def test_kagami_consumes_the_same_native_artifact_inventory() -> None:
    source = (REPO_ROOT / "crates/iroha_kagami/src/kagemusha.rs").read_text()
    declaration = re.search(
        r"const REQUIRED_C_JNI_SYMBOLS_V1: \[&str; (\d+)\] = \[(.*?)\n\];",
        source,
        re.DOTALL,
    )
    assert declaration is not None
    symbols = re.findall(r'"([A-Za-z0-9_]+)"', declaration.group(2))
    assert len(symbols) == int(declaration.group(1))
    assert tuple(symbols) == MODULE.REQUIRED_SYMBOLS["c-jni"]


def test_native_c_probe_rejects_required_kagemusha_export() -> None:
    for sdk in ("c-jni", "csharp"):
        for missing in (
            "connect_norito_kagemusha_device_mint_stage_command_v1_validate",
            "connect_norito_kagemusha_device_mint_stage_result_v1_validate",
            "connect_norito_kagemusha_contract_vector_v1",
            "connect_norito_kagemusha_core_coordinator_contract_v1",
            "connect_norito_kagemusha_core_coordinator_open_v1",
            "connect_norito_kagemusha_core_coordinator_invoke_v1",
            "connect_norito_kagemusha_device_command_response_v1_verify",
            "connect_norito_kagemusha_reserve_finality_hint_v1",
            "connect_norito_kagemusha_reserve_finality_verify_v1",
        ):
            library = types.SimpleNamespace(**{
                symbol: object() for symbol in MODULE.REQUIRED_SYMBOLS[sdk]
                if symbol != missing
            })
            with mock.patch.object(MODULE.ctypes, "CDLL", return_value=library):
                try:
                    MODULE.probe_c_abi(Path("test-only-library"), MODULE.REQUIRED_SYMBOLS[sdk])
                except MODULE.ArtifactContractError as error:
                    assert str(error) == (
                        "native C ABI artifact is missing required symbols: " + missing
                    )
                else:
                    raise AssertionError(f"missing KAGEMUSHA export was accepted: {sdk}: {missing}")


def test_kagemusha_required_symbols_are_declared_by_the_native_header() -> None:
    header = (
        REPO_ROOT / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
    ).read_text(encoding="utf-8")
    for symbol in KAGEMUSHA_V1_C_SYMBOLS:
        assert re.search(rf"\b{re.escape(symbol)}\s*\(", header), symbol


def test_current_kagemusha_symbols_are_not_blanket_retired() -> None:
    MODULE.validate_retired_protocol_symbols(
        sorted(KAGEMUSHA_V1_C_SYMBOLS), sdk="csharp"
    )


def test_native_artifact_checker_has_no_retired_protocol_surface() -> None:
    retired = {
        "c-jni": {
            "connect_norito_kagemusha_device_response_authenticator_v1_verify",
            "Java_org_hyperledger_iroha_sdk_offline_KagemushaDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeVerifyResponseAuthenticatorV1",
            "connect_norito_private_settlement_auditor_capsule_response_verify_v1",
            "Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditorCapsuleResponseV1",
            "Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditorCapsuleResponseV1",
        },
        "csharp": {
            "connect_norito_kagemusha_device_response_authenticator_v1_verify",
            "connect_norito_private_settlement_auditor_capsule_response_verify_v1",
        },
        "node": {"privateSettlementVerifyAuditorCapsuleResponseV1"},
        "python": {"private_settlement_verify_auditor_capsule_response_v1"},
    }
    for sdk, forbidden in retired.items():
        assert forbidden == set(MODULE.RETIRED_PROTOCOL_SYMBOLS[sdk])
        assert forbidden.isdisjoint(MODULE.REQUIRED_SYMBOLS[sdk])


def test_retired_protocol_symbol_inventory_is_rejected() -> None:
    for symbols in (
        ["connect_norito_private_settlement_auditor_capsule_response_verify_v1"],
        [RETIRED_KAGEMUSHA_C_PREFIX + "v1_payment_validate"],
        ["connect_norito_kagemusha_unrecognized_v1"],
    ):
        try:
            MODULE.validate_retired_protocol_symbols(symbols, sdk="csharp")
        except MODULE.ArtifactContractError as error:
            assert "retired protocol symbols" in str(error)
        else:
            raise AssertionError(f"retired symbols were accepted: {symbols}")


def test_command_verifier_never_accepts_previous_c_or_jni_alias() -> None:
    old_c = "connect_norito_kagemusha_device_response_authenticator_v1_verify"
    old_jni = (
        "Java_org_hyperledger_iroha_sdk_offline_"
        "KagemushaDeviceLifecycleBridgeV1_00024NativeEndpoint_"
        "nativeVerifyResponseAuthenticatorV1"
    )
    for sdk, aliases in (("c-jni", (old_c, old_jni)), ("csharp", (old_c,))):
        for alias in aliases:
            symbols = [*MODULE.REQUIRED_SYMBOLS[sdk], alias]
            try:
                MODULE.validate_retired_protocol_symbols(symbols, sdk=sdk)
            except MODULE.ArtifactContractError as error:
                assert alias in str(error)
            else:
                raise AssertionError(f"retired verifier alias was accepted: {sdk}: {alias}")


def test_top_up_binding_requires_all_c_and_kotlin_endpoints() -> None:
    expected = ["connect_norito_kagemusha_top_up_signed_request_validate_v1",
        "Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaTopUpSubmissionJniV1_nativeBridgeAbiVersion",
        "Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaTopUpSubmissionJniV1_nativeValidate"]
    required = MODULE.REQUIRED_SYMBOLS["c-jni"]
    for missing in expected:
        assert missing in required
        library = types.SimpleNamespace(**{symbol: object() for symbol in required if symbol != missing})
        with mock.patch.object(MODULE.ctypes, "CDLL", return_value=library):
            try:
                MODULE.probe_c_abi(Path("test-only-library"), required)
            except MODULE.ArtifactContractError as error:
                assert str(error) == "native C ABI artifact is missing required symbols: " + missing
            else:
                raise AssertionError("missing top-up binding endpoint accepted: " + missing)
