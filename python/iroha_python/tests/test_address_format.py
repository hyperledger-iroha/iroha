from __future__ import annotations

import json
from typing import Any, Dict, Optional, Union

import pytest
import requests
from requests.structures import CaseInsensitiveDict

from iroha_python import ToriiClient, account_query_envelope, asset_holders_query_envelope


class StubResponse(requests.Response):
    def __init__(self, payload: Optional[Dict[str, Any]] = None) -> None:
        super().__init__()
        self.status_code = 200
        self._payload = payload or {"items": [], "total": 0}
        self.headers = CaseInsensitiveDict({"Content-Type": "application/json"})
        self._content = json.dumps(self._payload).encode("utf-8")
        self.encoding = "utf-8"

    def json(self, **kwargs: Any) -> Any:
        return json.loads(self.content.decode("utf-8"))

    def close(self) -> None:
        return None

    def __enter__(self) -> "StubResponse":
        return self

    def __exit__(self, *args: object) -> None:
        self.close()
        return None


class RecordingSession(requests.Session):
    def __init__(self) -> None:
        super().__init__()
        self.calls: list[Dict[str, Any]] = []
        self._response = StubResponse()

    def request(
        self,
        method: Union[str, bytes],
        url: Union[str, bytes],
        *args: Any,
        **kwargs: Any,
    ) -> requests.Response:
        params = kwargs.get("params") or {}
        headers = kwargs.get("headers") or {}
        data = kwargs.get("data")
        self.calls.append(
            {
                "method": method,
                "url": url,
                "params": params,
                "headers": headers,
                "data": data,
            }
        )
        return self._response


def _client_with_session() -> tuple[ToriiClient, RecordingSession]:
    session = RecordingSession()
    client = ToriiClient("http://localhost:8080", session=session)
    return client, session


def test_account_query_envelope_omits_canonical_i105() -> None:
    payload = account_query_envelope()
    assert "canonical_i105" not in payload


def test_asset_holders_envelope_omits_canonical_i105() -> None:
    payload = asset_holders_query_envelope()
    assert "canonical_i105" not in payload


def test_list_accounts_omits_canonical_i105_param() -> None:
    client, session = _client_with_session()

    client.list_accounts()

    params = session.calls[0]["params"]
    assert "canonical_i105" not in params


def test_list_accounts_rejects_removed_canonical_i105_arg() -> None:
    client, _ = _client_with_session()

    with pytest.raises(TypeError):
        client.list_accounts(canonical_i105="i105")


def test_query_accounts_omits_canonical_i105() -> None:
    client, session = _client_with_session()

    client.query_accounts()

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert "canonical_i105" not in body


def test_list_asset_holders_omits_canonical_i105() -> None:
    client, session = _client_with_session()

    client.list_asset_holders("xor#wonderland")

    assert "canonical_i105" not in session.calls[0]["params"]


def test_query_asset_holders_omits_canonical_i105() -> None:
    client, session = _client_with_session()

    client.query_asset_holders("xor#wonderland")

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert "canonical_i105" not in body


def test_query_asset_holders_rejects_removed_canonical_i105_arg() -> None:
    client, _ = _client_with_session()

    with pytest.raises(TypeError):
        client.query_asset_holders("xor#wonderland", canonical_i105="i105")
