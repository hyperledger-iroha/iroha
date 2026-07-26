from __future__ import annotations

import json
from typing import Any, Dict, Optional, Union

import requests
from requests.structures import CaseInsensitiveDict


class StubResponse(requests.Response):
    """Minimal requests.Response stub that captures the provided payload."""

    def __init__(self, status_code: int = 200, payload: Optional[Dict[str, Any]] = None) -> None:
        super().__init__()
        self.status_code = status_code
        self._payload = payload
        self.headers = CaseInsensitiveDict({"Content-Type": "application/json"})
        content = json.dumps(payload).encode("utf-8") if payload is not None else b""
        self._content = content
        self.encoding = "utf-8"

    def json(self, **kwargs: Any) -> Any:
        if self._payload is None:
            raise ValueError("no payload")
        return json.loads(self.content.decode("utf-8"))

    def close(self) -> None:
        return None

    def __enter__(self) -> "StubResponse":
        return self

    def __exit__(self, *args: object) -> None:
        self.close()
        return None


class RecordingSession(requests.Session):
    """Capture outgoing requests and return a configured stub response."""

    def __init__(self, response: StubResponse) -> None:
        super().__init__()
        self._response = response
        self.calls: list[Dict[str, Any]] = []

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

    def get(
        self,
        url: Union[str, bytes],
        **kwargs: Any,
    ) -> requests.Response:
        return self.request(
            "GET",
            url,
            **kwargs,
        )
