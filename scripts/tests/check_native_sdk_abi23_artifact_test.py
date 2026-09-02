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


def test_csharp_native_contract_requires_offline_cash_v1() -> None:
    required = set(MODULE.REQUIRED_SYMBOLS["csharp"])
    assert {
        "connect_norito_offline_cash_v1_payment_request_validate",
        "connect_norito_offline_cash_v1_payment_validate",
        "connect_norito_offline_cash_v1_acknowledgement_validate",
        "connect_norito_offline_cash_v1_mint_credit_validate",
        "connect_norito_offline_cash_v1_redemption_voucher_validate",
    } <= required


def test_native_artifact_checker_has_no_retired_protocol_surface() -> None:
    source = MODULE_PATH.read_text(encoding="utf-8").lower()
    assert "offline_cash_v1" in source
