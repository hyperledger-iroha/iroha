from __future__ import annotations

import json
from typing import Any, Dict, Optional

import pytest

from iroha_python import (
    ToriiClient,
    account_query_envelope,
    asset_holders_query_envelope,
)


class StubResponse:
    def __init__(self, payload: Optional[Dict[str, Any]] = None) -> None:
        self.status_code = 200
        self._payload = payload or {"items": [], "total": 0}
        self.headers: Dict[str, str] = {"Content-Type": "application/json"}
        self.content = json.dumps(self._payload).encode("utf-8")

    def json(self) -> Dict[str, Any]:
        return json.loads(self.content.decode("utf-8"))

    def close(self) -> None:
        return None

    def __enter__(self) -> "StubResponse":
        return self

    def __exit__(self, exc_type, exc, tb) -> bool:
        self.close()
        return False


class RecordingSession:
    def __init__(self) -> None:
        self.calls: list[Dict[str, Any]] = []
        self._response = StubResponse()

    def request(
        self,
        method: str,
        url: str,
        params: Optional[Dict[str, Any]] = None,
        headers: Optional[Dict[str, str]] = None,
        data: Optional[bytes] = None,
        stream: bool = False,
        timeout: Optional[float] = None,
    ) -> StubResponse:
        self.calls.append(
            {
                "method": method,
                "url": url,
                "params": params or {},
                "headers": headers or {},
                "data": data,
            }
        )
        return self._response


def _client_with_session() -> tuple[ToriiClient, RecordingSession]:
    session = RecordingSession()
    client = ToriiClient("http://localhost:8080", session=session)  # type: ignore[arg-type]
    return client, session


def test_account_query_envelope_includes_address_format() -> None:
    payload = account_query_envelope(address_format="compressed")
    assert payload["address_format"] == "compressed"


def test_asset_holders_envelope_includes_address_format() -> None:
    payload = asset_holders_query_envelope(address_format="ih58")
    assert payload["address_format"] == "ih58"


def test_list_accounts_sends_address_format_param() -> None:
    client, session = _client_with_session()

    client.list_accounts(address_format="compressed")

    params = session.calls[0]["params"]
    assert params["address_format"] == "compressed"


def test_list_accounts_rejects_alias() -> None:
    client, session = _client_with_session()

    with pytest.raises(ValueError):
        client.list_accounts(address_format="IH-b32")


def test_list_accounts_rejects_invalid_address_format() -> None:
    client, _ = _client_with_session()

    with pytest.raises(ValueError):
        client.list_accounts(address_format="base64")


def test_query_accounts_includes_address_format() -> None:
    client, session = _client_with_session()

    client.query_accounts(address_format="compressed")

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert body["address_format"] == "compressed"


def test_list_asset_holders_sends_address_format() -> None:
    client, session = _client_with_session()

    client.list_asset_holders("xor#wonderland", address_format="compressed")

    assert session.calls[0]["params"]["address_format"] == "compressed"


def test_query_asset_holders_includes_address_format() -> None:
    client, session = _client_with_session()

    client.query_asset_holders("xor#wonderland", address_format="compressed")

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert body["address_format"] == "compressed"


def test_query_asset_holders_rejects_alias() -> None:
    client, _ = _client_with_session()

    with pytest.raises(ValueError):
        client.query_asset_holders("xor#wonderland", address_format="snx1")
