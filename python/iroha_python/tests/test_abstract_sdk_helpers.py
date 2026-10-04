from __future__ import annotations

import pytest

from iroha_python.connect import ConnectControlPing, _ConnectControlBase
from iroha_python.event_filter import DataEventFilter, EventFilter
from iroha_python.query_filter import Eq, FilterExpr


def test_python_sdk_helper_bases_are_abstract() -> None:
    with pytest.raises(TypeError, match="abstract"):
        EventFilter()
    with pytest.raises(TypeError, match="abstract"):
        FilterExpr()
    with pytest.raises(TypeError, match="abstract"):
        _ConnectControlBase("Open")


def test_concrete_helpers_still_render_payloads() -> None:
    assert Eq("metadata.status", "active").to_dict() == {
        "op": "eq",
        "args": ["metadata.status", "active"],
    }
    assert DataEventFilter.pipeline_block(height=7, status="Committed").to_dict() == {
        "Pipeline": {"Block": {"height": 7, "status": "Committed"}}
    }
    assert ConnectControlPing(nonce=42).to_dict() == {"nonce": 42}
