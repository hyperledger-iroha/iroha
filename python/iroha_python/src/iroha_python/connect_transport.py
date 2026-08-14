"""Fail-closed transport preparation for Iroha Connect WebSockets."""

from __future__ import annotations

from typing import Mapping, Optional, Sequence, Tuple
from urllib.parse import urlencode, urlparse, urlunparse

from .connect import _canonical_connect_token, _decode_canonical_base64url

_CONNECT_TOKEN_PROTOCOL_PREFIX = "iroha-connect.token.v1."
_FORBIDDEN_AUTH_HEADERS = frozenset(
    {
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce",
        "x-iroha-witness",
        "x-iroha-operator-public-key",
        "x-iroha-operator-timestamp-ms",
        "x-iroha-operator-nonce",
        "x-iroha-operator-signature",
    }
)


def prepare_connect_websocket_request(
    base_url: str,
    sid: str,
    role: str,
    token: str,
    *,
    headers: Optional[Mapping[str, str]] = None,
    subprotocols: Optional[Sequence[str]] = None,
) -> Tuple[str, list[str], Optional[list[str]]]:
    """Build one canonical WebSocket target with header-only role authentication."""

    _decode_canonical_base64url(sid, 32, "sid")
    if role not in {"app", "wallet"}:
        raise ValueError("role must be exactly 'app' or 'wallet'")
    role_token = _canonical_connect_token(token, "token")

    prepared_headers: list[str] = []
    for raw_name, raw_value in (headers or {}).items():
        name = str(raw_name)
        value = str(raw_value)
        if name.lower() in _FORBIDDEN_AUTH_HEADERS:
            raise ValueError(
                f"Connect WebSocket rejects fallback or precomputed auth header {name}"
            )
        if not name or "\r" in name or "\n" in name or "\r" in value or "\n" in value:
            raise ValueError("Connect WebSocket headers must not contain line breaks")
        prepared_headers.append(f"{name}: {value}")
    prepared_headers.append(f"Authorization: Bearer {role_token}")

    prepared_subprotocols = None if subprotocols is None else list(subprotocols)
    if prepared_subprotocols is not None:
        for protocol in prepared_subprotocols:
            if not isinstance(protocol, str) or not protocol:
                raise TypeError("Connect WebSocket subprotocols must be non-empty strings")
            if protocol.startswith(_CONNECT_TOKEN_PROTOCOL_PREFIX):
                raise ValueError(
                    "Connect WebSocket token subprotocol must not conflict with generated Authorization"
                )

    parsed = urlparse(base_url)
    scheme = "wss" if parsed.scheme == "https" else "ws"
    query = urlencode({"sid": sid, "role": role})
    target = urlunparse((scheme, parsed.netloc, "/v1/connect/ws", "", query, ""))
    return target, prepared_headers, prepared_subprotocols
