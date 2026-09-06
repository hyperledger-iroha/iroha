"""Bounded Norito RPC calls over the SDK's canonical Torii transport."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from types import MappingProxyType
from typing import Any, Mapping, Optional

import requests

from .client import (
    ToriiClient,
    _copy_http_headers,
    _normalize_torii_base_url,
    _reject_reserved_default_headers,
    _require_positive_finite_float,
    _require_route_token,
)

__all__ = [
    "NoritoRpcClient",
    "NoritoRpcConfig",
    "NoritoRpcError",
]

_NORITO_MEDIA_TYPE = "application/x-norito"
_DEFAULT_MAX_RESPONSE_BYTES = 64 * 1024 * 1024
_READ_CHUNK_BYTES = 64 * 1024
_ERROR_DETAIL_MAX_BYTES = 4 * 1024
_MEDIA_TYPE_RE = re.compile(r"^[!#$%&'*+.^_`|~0-9A-Za-z-]+/[!#$%&'*+.^_`|~0-9A-Za-z-]+$")


class NoritoRpcError(RuntimeError):
    """Raised when a Norito RPC response violates the transport contract."""


@dataclass(frozen=True, slots=True)
class NoritoRpcConfig:
    """Configuration shared with the canonical Torii HTTP transport."""

    base_url: str
    timeout: float = 30.0
    default_headers: Mapping[str, str] = field(default_factory=dict)
    auth_token: Optional[str] = field(default=None, repr=False, compare=False)
    api_token: Optional[str] = field(default=None, repr=False, compare=False)
    max_response_bytes: int = _DEFAULT_MAX_RESPONSE_BYTES

    def __post_init__(self) -> None:
        if isinstance(self.max_response_bytes, bool) or not isinstance(
            self.max_response_bytes,
            int,
        ):
            raise TypeError("max_response_bytes must be a positive integer")
        if self.max_response_bytes <= 0:
            raise ValueError("max_response_bytes must be a positive integer")
        object.__setattr__(self, "base_url", _normalize_torii_base_url(self.base_url))
        object.__setattr__(
            self,
            "timeout",
            _require_positive_finite_float(self.timeout, "timeout"),
        )
        default_headers = _copy_http_headers(self.default_headers, "default_headers")
        _reject_reserved_default_headers(default_headers, "default_headers")
        object.__setattr__(self, "default_headers", MappingProxyType(default_headers))
        if self.auth_token is not None:
            object.__setattr__(
                self,
                "auth_token",
                _require_route_token(self.auth_token, "auth_token"),
            )
        if self.api_token is not None:
            object.__setattr__(
                self,
                "api_token",
                _require_route_token(self.api_token, "api_token"),
            )


class NoritoRpcClient:
    """Small binary facade over :class:`iroha_python.ToriiClient`."""

    def __init__(
        self,
        config: NoritoRpcConfig,
        session: Optional[requests.Session] = None,
    ) -> None:
        if not isinstance(config, NoritoRpcConfig):
            raise TypeError("config must be a NoritoRpcConfig")
        self._config = config
        self._transport = ToriiClient(
            config.base_url,
            session=session,
            timeout=config.timeout,
            auth_token=config.auth_token,
            api_token=config.api_token,
            default_headers=config.default_headers,
        )

    def __enter__(self) -> "NoritoRpcClient":
        return self

    def __exit__(self, _exc_type: Any, _exc: Any, _traceback: Any) -> None:
        self.close()

    @property
    def base_url(self) -> str:
        """Return the normalized Torii origin."""

        return self._transport._base_url

    def close(self) -> None:
        """Close the transport when it owns the underlying session."""

        self._transport.close()

    def call(
        self,
        path: str,
        payload: bytes,
        *,
        timeout: Optional[float] = None,
        headers: Optional[Mapping[str, str]] = None,
        method: str = "POST",
        params: Optional[Mapping[str, Any]] = None,
        accept: str = _NORITO_MEDIA_TYPE,
    ) -> bytes:
        """Invoke one origin-relative RPC route and return a bounded byte body."""

        if type(payload) is not bytes:
            raise TypeError("payload must be exact immutable bytes")
        expected_media_type = _require_media_type(accept, "accept")
        request_headers = {"Content-Type": _NORITO_MEDIA_TYPE, "Accept": accept}
        if headers is not None:
            copied_headers = _copy_http_headers(headers, "headers")
            _reject_reserved_default_headers(copied_headers, "headers")
            for name, value in copied_headers.items():
                if name.lower() in {"accept", "content-type"}:
                    raise ValueError(
                        f"headers must not override {name}; use accept for response negotiation"
                    )
                request_headers[name] = value

        response = self._transport._request(
            method,
            path,
            data=payload,
            params=params,
            headers=request_headers,
            timeout=timeout,
            allow_retry=False,
            allow_redirects=False,
            stream=True,
        )
        try:
            if not 200 <= response.status_code < 300:
                detail_bytes, truncated = _read_body_prefix(
                    response,
                    maximum_bytes=_ERROR_DETAIL_MAX_BYTES,
                )
                detail = detail_bytes.decode("utf-8", errors="replace")
                if truncated:
                    detail += "… <truncated>"
                raise NoritoRpcError(
                    f"Norito RPC request failed with status {response.status_code}: {detail}"
                )
            response_media_type = response.headers.get("Content-Type")
            if (
                response_media_type is None
                or response_media_type.lower() != expected_media_type.lower()
            ):
                raise NoritoRpcError(
                    f"Norito RPC response Content-Type must exactly match {expected_media_type}"
                )
            return _read_bounded_body(
                response,
                maximum_bytes=self._config.max_response_bytes,
            )
        finally:
            response.close()


def _require_media_type(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be an exact media type")
    if value != value.strip() or not _MEDIA_TYPE_RE.fullmatch(value) or "*" in value:
        raise ValueError(f"{context} must be one concrete media type without parameters")
    return value


def _read_bounded_body(response: requests.Response, *, maximum_bytes: int) -> bytes:
    content_length = response.headers.get("Content-Length")
    if content_length is not None and content_length.isascii() and content_length.isdecimal():
        if int(content_length, 10) > maximum_bytes:
            raise NoritoRpcError(f"Norito RPC response exceeds the {maximum_bytes}-byte limit")
    body = bytearray()
    for chunk in response.iter_content(chunk_size=_READ_CHUNK_BYTES):
        if not isinstance(chunk, bytes):
            raise NoritoRpcError("Norito RPC response yielded a non-bytes body chunk")
        if len(body) + len(chunk) > maximum_bytes:
            raise NoritoRpcError(f"Norito RPC response exceeds the {maximum_bytes}-byte limit")
        body.extend(chunk)
    return bytes(body)


def _read_body_prefix(
    response: requests.Response,
    *,
    maximum_bytes: int,
) -> tuple[bytes, bool]:
    body = bytearray()
    for chunk in response.iter_content(chunk_size=_READ_CHUNK_BYTES):
        if not isinstance(chunk, bytes):
            raise NoritoRpcError("Norito RPC response yielded a non-bytes body chunk")
        remaining = maximum_bytes - len(body)
        if len(chunk) > remaining:
            body.extend(chunk[:remaining])
            return bytes(body), True
        body.extend(chunk)
    return bytes(body), False
