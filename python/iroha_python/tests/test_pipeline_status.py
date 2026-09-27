from __future__ import annotations

from typing import Any

import requests

from iroha_python import ToriiClient

from .helpers import RecordingSession, StubResponse


class TimeoutRecordingSession(requests.Session):
    def __init__(self, response: StubResponse) -> None:
        super().__init__()
        self._response = response
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str, url: str, **kwargs: Any) -> requests.Response:
        self.calls.append(
            {
                "method": method,
                "url": url,
                "timeout": kwargs.get("timeout"),
                "params": kwargs.get("params") or {},
            }
        )
        return self._response


def test_get_transaction_status_returns_none_on_404() -> None:
    response = StubResponse(status_code=404, payload=None)
    session = RecordingSession(response)
    client = ToriiClient("http://localhost:8080", session=session)

    result = client.get_transaction_status("deadbeef")

    assert result is None
    assert session.calls[0]["params"]["hash"] == "deadbeef"


def test_get_transaction_status_forwards_request_timeout() -> None:
    session = TimeoutRecordingSession(StubResponse(payload={"status": "Committed"}))
    client = ToriiClient("http://localhost:8080", session=session)

    result = client.get_transaction_status("deadbeef", timeout=7.5)

    assert result == {"status": "Committed"}
    assert session.calls[0]["timeout"] == 7.5


def test_wait_for_transaction_status_caps_poll_timeout_to_deadline() -> None:
    session = TimeoutRecordingSession(StubResponse(payload={"status": "Committed"}))
    client = ToriiClient("http://localhost:8080", session=session, timeout=30)

    result = client.wait_for_transaction_status(
        "deadbeef",
        interval=0,
        timeout=2,
    )

    assert result == {"status": "Committed"}
    assert session.calls[0]["timeout"] is not None
    assert 0 < session.calls[0]["timeout"] <= 2
