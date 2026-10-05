from __future__ import annotations

import pytest

from iroha_python import F
from iroha_python.connect import ConnectControlPing, _ConnectControlBase


def test_connect_control_base_is_abstract() -> None:
    with pytest.raises(TypeError, match="abstract"):
        _ConnectControlBase("Open")


def test_concrete_helpers_still_render_payloads() -> None:
    assert F.metadata.status.eq("active").to_json() == {
        "op": "eq",
        "args": ["metadata.status", "active"],
    }
    assert ConnectControlPing(nonce=42).to_dict() == {"nonce": 42}
