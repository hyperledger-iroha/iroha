from __future__ import annotations

import json
from typing import Any, Dict, Optional


class StubResponse:
    """Minimal requests.Response stub that captures the provided payload."""

    def __init__(self, status_code: int = 200, payload: Optional[Dict[str, Any]] = None) -> None:
        self.status_code = status_code
        self._payload = payload
        self.headers: Dict[str, str] = {"Content-Type": "application/json"}
        self.content = json.dumps(payload).encode("utf-8") if payload is not None else b""
        self.text = self.content.decode("utf-8")

    def json(self) -> Dict[str, Any]:
        if self._payload is None:
            raise ValueError("no payload")
        return json.loads(self.content.decode("utf-8"))

    def close(self) -> None:
        return None

    def __enter__(self) -> "StubResponse":
        return self

    def __exit__(self, exc_type, exc, tb) -> bool:
        self.close()
        return False


class RecordingSession:
    """Capture outgoing requests and return a configured stub response."""

    def __init__(self, response: StubResponse) -> None:
        self._response = response
        self.calls: list[Dict[str, Any]] = []

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

    def get(
        self,
        url: str,
        *,
        params: Optional[Dict[str, Any]] = None,
        stream: bool = False,
        timeout: Optional[float] = None,
        headers: Optional[Dict[str, str]] = None,
    ) -> StubResponse:
        return self.request(
            "GET",
            url,
            params=params,
            headers=headers,
            stream=stream,
            timeout=timeout,
        )
