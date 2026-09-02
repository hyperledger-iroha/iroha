"""Focused first-release contract tests for native SDK artifact inspection."""

from __future__ import annotations

import importlib.util
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts/check_native_sdk_abi23_artifact.py"
SPEC = importlib.util.spec_from_file_location("check_native_sdk_abi23_artifact", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

OFFLINE_CASH_V1_C_SYMBOLS = {
    "connect_norito_offline_cash_v1_payment_request_validate",
    "connect_norito_offline_cash_v1_acceptance_intent_authorization_validate",
    "connect_norito_offline_cash_v1_acceptance_ticket_validate",
    "connect_norito_offline_cash_v1_no_commit_closure_validate",
    "connect_norito_offline_cash_v1_payment_validate",
    "connect_norito_offline_cash_v1_acknowledgement_validate",
    "connect_norito_offline_cash_v1_complete_exchange_validate",
    "connect_norito_offline_cash_v1_mint_authorization_validate",
    "connect_norito_offline_cash_v1_mint_credit_validate",
    "connect_norito_offline_cash_v1_mint_credit_against_authorization_validate",
    "connect_norito_offline_cash_v1_redemption_voucher_validate",
    "connect_norito_offline_cash_v1_payment_request_text_validate",
    "connect_norito_offline_cash_v1_acceptance_intent_authorization_text_validate",
    "connect_norito_offline_cash_v1_acceptance_ticket_text_validate",
    "connect_norito_offline_cash_v1_no_commit_closure_text_validate",
    "connect_norito_offline_cash_v1_payment_text_validate",
    "connect_norito_offline_cash_v1_acknowledgement_text_validate",
    "connect_norito_offline_cash_v1_complete_exchange_text_validate",
    "connect_norito_offline_cash_v1_mint_authorization_text_validate",
    "connect_norito_offline_cash_v1_mint_credit_text_validate",
    "connect_norito_offline_cash_v1_mint_credit_against_authorization_text_validate",
    "connect_norito_offline_cash_v1_redemption_voucher_text_validate",
    "connect_norito_offline_cash_device_capabilities_v1",
    "connect_norito_offline_cash_device_execute_v1",
}


def test_native_c_contracts_require_complete_offline_cash_v1() -> None:
    for sdk in ("c-jni", "csharp"):
        assert OFFLINE_CASH_V1_C_SYMBOLS <= set(MODULE.REQUIRED_SYMBOLS[sdk])


def test_native_artifact_checker_has_no_retired_protocol_surface() -> None:
    retired = {
        "c-jni": {
            "connect_norito_private_settlement_auditor_capsule_response_verify_v1",
            "Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditorCapsuleResponseV1",
            "Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditorCapsuleResponseV1",
        },
        "csharp": {
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
        ["connect_norito_kagemusha_recursive_spend_init_v4"],
    ):
        try:
            MODULE.validate_retired_protocol_symbols(symbols, sdk="csharp")
        except MODULE.ArtifactContractError as error:
            assert "retired protocol symbols" in str(error)
        else:
            raise AssertionError(f"retired symbols were accepted: {symbols}")
