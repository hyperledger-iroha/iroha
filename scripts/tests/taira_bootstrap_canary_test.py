"""Tests for scripts/taira_bootstrap_canary.py."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "taira_bootstrap_canary.py"
SPEC = importlib.util.spec_from_file_location("taira_bootstrap_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_default_chain_id_targets_public_sumeragi_v2_taira() -> None:
    assert MODULE.DEFAULT_CHAIN_ID == "fc56984b-2be7-431d-840e-21514d1883f0"


def test_faucet_claim_requires_applied_status() -> None:
    assert MODULE.faucet_claim_status_kind({"status": "Applied"}) == "Applied"
    assert MODULE.faucet_claim_status_kind({"status": {"kind": "Rejected"}}) == "Rejected"
    assert MODULE.faucet_claim_status_kind({}) is None
