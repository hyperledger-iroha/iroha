"""Norito RPC helpers for interacting with Torii endpoints."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Mapping, MutableMapping, Optional
from urllib.parse import urljoin

import requests

__all__ = [
    "NoritoRpcClient",
    "NoritoRpcConfig",
    "NoritoRpcError",
]


class NoritoRpcError(RuntimeError):
    """Raised when a Norito RPC request fails."""


@dataclass(frozen=True)
class NoritoRpcConfig:
    """Configuration for the Norito RPC client."""

    base_url: str
    timeout: Optional[float] = None
    default_headers: Mapping[str, str] = field(default_factory=dict)


class NoritoRpcClient:
    """Minimal HTTP client that speaks the Norito RPC surface exposed by Torii."""

    def __init__(
        self,
        config: NoritoRpcConfig,
        session: Optional[requests.Session] = None,
    ) -> None:
        self._config = config
        self._session = session or requests.Session()
        self._owns_session = session is None

    def __enter__(self) -> "NoritoRpcClient":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    @property
    def base_url(self) -> str:
        """Return the configured base URL."""

        return self._config.base_url

    def close(self) -> None:
        """Close the underlying HTTP session if owned by the client."""

        if self._owns_session:
            self._session.close()

    def call(
        self,
        path: str,
        payload: bytes,
        *,
        timeout: Optional[float] = None,
        headers: Optional[Mapping[str, str]] = None,
        method: str = "POST",
        params: Optional[Mapping[str, Any]] = None,
        accept: Optional[str] = "application/x-norito",
    ) -> bytes:
        """Invoke a Norito RPC endpoint relative to the configured base URL."""

        url = _join_url(self._config.base_url, path)
        request_headers: MutableMapping[str, str] = dict(self._config.default_headers)
        request_headers.setdefault("Content-Type", "application/x-norito")
        if accept:
            request_headers.setdefault("Accept", accept)
        if headers:
            request_headers.update(headers)
        response = self._session.request(
            method.upper(),
            url,
            data=payload,
            params=params,
            headers=request_headers,
            timeout=self._pick_timeout(timeout),
        )
        _ensure_success(response)
        return response.content

    def _pick_timeout(self, timeout: Optional[float]) -> Optional[float]:
        if timeout is not None:
            return max(0.0, timeout)
        return self._config.timeout


def _ensure_success(response: requests.Response) -> None:
    if 200 <= response.status_code < 300:
        return
    raise NoritoRpcError(
        f"Norito RPC request failed with status {response.status_code}: {response.text}"
    )


def _join_url(base: str, path: str) -> str:
    if not path:
        return base
    if path.startswith("http://") or path.startswith("https://"):
        return path
    base_with_slash = base if base.endswith("/") else f"{base}/"
    return urljoin(base_with_slash, path.lstrip("/"))
