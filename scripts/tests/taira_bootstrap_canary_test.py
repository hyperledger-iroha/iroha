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


def test_faucet_claim_requires_applied_status() -> None:
    assert MODULE.faucet_claim_status_kind({"status": "Applied"}) == "Applied"
    assert MODULE.faucet_claim_status_kind({"status": {"kind": "Rejected"}}) == "Rejected"
    assert MODULE.faucet_claim_status_kind({}) is None
