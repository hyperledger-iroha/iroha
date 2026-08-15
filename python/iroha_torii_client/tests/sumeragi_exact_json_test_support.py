"""Shared HTTP fixtures for selected Sumeragi exact-JSON client tests."""

from __future__ import annotations

import json
from typing import Any, Dict, List, Mapping, Optional, Tuple, Type, Union

import requests
from requests.structures import CaseInsensitiveDict


class StubResponse(requests.Response):
    """Queueable response that preserves raw bytes and records closure."""

    def __init__(
        self,
        status_code: int = 200,
        payload: Optional[Any] = None,
        *,
        headers: Optional[Dict[str, str]] = None,
        raw: Optional[bytes] = None,
        text: Optional[str] = None,
    ) -> None:
        super().__init__()
        self.status_code = status_code
        self._payload = payload
        self.headers = CaseInsensitiveDict(headers or {})
        if raw is not None:
            content = raw
        elif payload is None:
            content = text.encode("utf-8") if text is not None else b""
        else:
            content = json.dumps(payload).encode("utf-8")
            if "Content-Type" not in self.headers:
                self.headers["Content-Type"] = "application/json"
        self._content, self._content_consumed = content, True
        self.encoding = "utf-8"
        self.was_closed = False

    def close(self) -> None:
        self.was_closed = True
        super().close()

    def json(self, **kwargs: Any) -> Any:
        if self._payload is None:
            raise ValueError("no payload available")
        return json.loads(self.text)


class RecordingSession(requests.Session):
    """Queue-backed session retaining the exact transport arguments."""

    def __init__(self) -> None:
        super().__init__()
        self.calls: List[Dict[str, Any]] = []
        self._responses: List[StubResponse] = []

    def queue(self, response: StubResponse) -> None:
        self._responses.append(response)

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
                "allow_redirects": kwargs.get("allow_redirects"),
                "stream": kwargs.get("stream"),
            }
        )
        if not self._responses:
            raise AssertionError("no queued responses")
        return self._responses.pop(0)

    def send(
        self,
        request: requests.PreparedRequest,
        **kwargs: Any,
    ) -> requests.Response:
        """Record a canonical request after Requests has fixed its wire target."""

        self.calls.append(
            {
                "method": request.method,
                "url": request.url,
                "params": {},
                "headers": dict(request.headers),
                "data": request.body,
                "allow_redirects": kwargs.get("allow_redirects"),
                "stream": kwargs.get("stream"),
            }
        )
        if not self._responses:
            raise AssertionError("no queued responses")
        response = self._responses.pop(0)
        response.request = request
        response.url = request.url
        return response


AdversarialResponseCase = Tuple[str, StubResponse, Type[Exception], str]


def sumeragi_exact_json_response_cases() -> Tuple[AdversarialResponseCase, ...]:
    """Return duplicate, encoding, media-type, and byte-bound mutations."""

    return (
        (
            "diagnostics",
            StubResponse(
                raw=b'{"receipt":{"version":1,"version":2}}',
                headers={"Content-Type": "application/json"},
            ),
            ValueError,
            "duplicate field `version`",
        ),
        (
            "status",
            StubResponse(
                raw=b'{"value":"\xff"}',
                headers={"Content-Type": "application/json"},
            ),
            ValueError,
            "UTF-8 JSON",
        ),
        (
            "status",
            StubResponse(raw=b"{}", headers={"Content-Type": "text/plain"}),
            TypeError,
            "content type",
        ),
        (
            "diagnostics",
            StubResponse(
                raw=b"{}",
                headers={
                    "Content-Type": "application/json",
                    "Content-Length": str(16 * 1024 * 1024 + 1),
                },
            ),
            ValueError,
            "16777216-byte size bound",
        ),
        (
            "status",
            StubResponse(
                raw=b" " * (1024 * 1024 + 1),
                headers={"Content-Type": "application/json"},
            ),
            ValueError,
            "1048576-byte size bound",
        ),
    )
