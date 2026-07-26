from __future__ import annotations

from typing import Optional

import pytest

from iroha_python.client import ToriiClient

from .helpers import RecordingSession, StubResponse


@pytest.mark.parametrize(
    ("asset_id", "expected_params"),
    [
        (None, {}),
        ("xor#wonderland", {"asset_id": "xor#wonderland"}),
    ],
)
def test_get_uaid_portfolio_cleans_optional_query_params(
    asset_id: Optional[str], expected_params: dict[str, str]
) -> None:
    payload = {"uaid": "uaid:" + "ab" * 32, "totals": {}, "dataspaces": []}
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    result = client.get_uaid_portfolio(payload["uaid"], asset_id=asset_id)

    assert result == payload
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"] == "http://torii.example/v1/accounts/" + payload["uaid"] + "/portfolio"
    assert call["params"] == expected_params
    assert call["headers"] == {"Accept": "application/json"}
