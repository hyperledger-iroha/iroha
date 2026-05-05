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


def test_faucet_registration_fallback_record_preserves_errors() -> None:
    record = MODULE.faucet_registration_fallback_record(
        RuntimeError("onboard failed"),
        RuntimeError("alias missing"),
    )
    assert record["status"] == "faucet_registration_fallback"
    assert record["response_status"] == 400
    assert "onboard failed" in record["response"]
    assert "alias missing" in record["alias_resolve_error"]
