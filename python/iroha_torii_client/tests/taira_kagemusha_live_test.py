"""Opt-in read-only Kagemusha capability probe against public Taira."""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import ToriiClient


@pytest.mark.skipif(
    os.environ.get("IROHA_TAIRA_KAGEMUSHA_READ_ONLY") != "1",
    reason="set IROHA_TAIRA_KAGEMUSHA_READ_ONLY=1 for the public Taira probe",
)
def test_public_taira_exposes_exact_kagemusha_capability() -> None:
    """Exercise the real Python SDK against Taira without mutating chain state."""

    public_root = os.environ.get("IROHA_TAIRA_PUBLIC_ROOT", "https://taira.sora.org")
    capability = ToriiClient(public_root).get_offline_capability(timeout=20.0)

    assert capability.cash_handoff_capability == "cash_handoff_v1"
    assert capability.required_bridge_abi_version == 23
    assert capability.max_hops == 8
    assert capability.ready is True
