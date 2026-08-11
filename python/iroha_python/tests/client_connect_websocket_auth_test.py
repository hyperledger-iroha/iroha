"""Connect WebSocket role-token transport regressions."""

from __future__ import annotations

import base64
from urllib.parse import parse_qs, urlparse

import pytest

from iroha_python.connect_transport import prepare_connect_websocket_request


def _canonical(value: int) -> str:
    return base64.urlsafe_b64encode(bytes([value]) * 32).rstrip(b"=").decode("ascii")


def test_connect_websocket_keeps_role_token_out_of_url_and_fallback_headers() -> None:
    sid = _canonical(0x41)
    token = _canonical(0x51)

    target, headers, protocols = prepare_connect_websocket_request(
        "https://torii.example:8443/base",
        sid,
        "app",
        token,
        headers={"X-Trace-Id": "trace-1"},
    )

    parsed = urlparse(target)
    assert parsed.scheme == "wss"
    assert parsed.netloc == "torii.example:8443"
    assert parsed.path == "/v1/connect/ws"
    assert parse_qs(parsed.query, strict_parsing=True) == {
        "sid": [sid],
        "role": ["app"],
    }
    assert token not in target
    assert headers == [
        "X-Trace-Id: trace-1",
        f"Authorization: Bearer {token}",
    ]
    assert protocols is None


@pytest.mark.parametrize(
    "headers",
    [
        {"Authorization": "Bearer substituted"},
        {"X-API-Token": "retired"},
        {"X-Iroha-Operator-Signature": "precomputed"},
        {"X-Iroha-Signature": "account-domain"},
    ],
)
def test_connect_websocket_rejects_fallback_auth_headers(headers: dict[str, str]) -> None:
    with pytest.raises(ValueError, match="rejects fallback or precomputed auth"):
        prepare_connect_websocket_request(
            "https://torii.example",
            _canonical(0x41),
            "wallet",
            _canonical(0x52),
            headers=headers,
        )


def test_connect_websocket_rejects_noncanonical_identity_and_token_protocol_substitution() -> None:
    sid = _canonical(0x41)
    token = _canonical(0x51)
    with pytest.raises(ValueError, match="role must be exactly"):
        prepare_connect_websocket_request(
            "https://torii.example",
            sid,
            "Wallet",
            token,
        )
    with pytest.raises(ValueError, match="padding|canonical base64url"):
        prepare_connect_websocket_request(
            "https://torii.example",
            sid + "=",
            "wallet",
            token,
        )
    with pytest.raises(ValueError, match="must not conflict"):
        prepare_connect_websocket_request(
            "https://torii.example",
            sid,
            "wallet",
            token,
            subprotocols=[f"iroha-connect.token.v1.{_canonical(0x53)}"],
        )
