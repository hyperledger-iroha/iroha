"""Norito RPC uses the canonical bounded, origin-locked SDK transport."""

from __future__ import annotations

from urllib.parse import urlsplit

import pytest
import requests

from iroha_python import NoritoRpcClient, NoritoRpcConfig, NoritoRpcError


class Response(requests.Response):
    """In-memory response with observable cleanup."""

    def __init__(self, status: int, body: bytes) -> None:
        super().__init__()
        self.status_code = status
        self._content = body
        self.headers["Content-Type"] = "application/x-norito"
        self.closed_by_client = False

    def close(self) -> None:
        self.closed_by_client = True
        super().close()


class Session(requests.Session):
    """Deterministic request recorder."""

    def __init__(self, responses: list[requests.Response]) -> None:
        super().__init__()
        self.responses = list(responses)
        self.calls: list[dict[str, object]] = []
        self.close_count = 0

    def request(self, method: str, url: str, **kwargs: object) -> requests.Response:
        self.calls.append(
            {
                "method": method,
                "url": url,
                "path": urlsplit(url).path,
                **kwargs,
            }
        )
        if not self.responses:
            raise AssertionError(f"unexpected request {method} {url}")
        response = self.responses.pop(0)
        response.url = url
        return response

    def close(self) -> None:
        self.close_count += 1
        super().close()


def test_call_uses_canonical_transport_and_returns_bytes() -> None:
    response = Response(200, b"norito-response")
    session = Session([response])
    rpc = NoritoRpcClient(
        NoritoRpcConfig(
            "https://torii.example/",
            default_headers={"X-Client": "iroha-python"},
        ),
        session=session,
    )

    body = rpc.call(
        "/v1/norito-rpc",
        b"request",
        params={"mode": "strict"},
    )

    assert body == b"norito-response"
    assert response.closed_by_client is True
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"] == "https://torii.example/v1/norito-rpc"
    assert call["data"] == b"request"
    assert call["params"] == {"mode": "strict"}
    assert call["allow_redirects"] is False
    assert call["stream"] is True
    assert call["headers"] == {
        "Accept": "application/x-norito",
        "X-Client": "iroha-python",
        "Content-Type": "application/x-norito",
    }


def test_call_rejects_cross_origin_and_mutable_payloads_before_io() -> None:
    session = Session([])
    rpc = NoritoRpcClient(NoritoRpcConfig("https://torii.example"), session=session)

    with pytest.raises(ValueError, match="origin-relative"):
        rpc.call("https://attacker.example/steal", b"request")
    with pytest.raises(ValueError, match="origin-relative"):
        rpc.call("//attacker.example/steal", b"request")
    with pytest.raises(TypeError, match="immutable bytes"):
        rpc.call("/v1/norito-rpc", bytearray(b"request"))  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="must not override"):
        rpc.call(
            "/v1/norito-rpc",
            b"request",
            headers={"Content-Type": "application/json"},
        )
    with pytest.raises(ValueError, match="auth_token explicitly"):
        rpc.call(
            "/v1/norito-rpc",
            b"request",
            headers={"Authorization": "Bearer secret"},
        )
    assert session.calls == []


def test_response_body_is_bounded_and_always_closed() -> None:
    response = Response(200, b"12345")
    rpc = NoritoRpcClient(
        NoritoRpcConfig("https://torii.example", max_response_bytes=4),
        session=Session([response]),
    )

    with pytest.raises(NoritoRpcError, match="4-byte limit"):
        rpc.call("/v1/norito-rpc", b"request")
    assert response.closed_by_client is True


def test_success_requires_the_negotiated_media_type() -> None:
    response = Response(200, b"<html>not norito</html>")
    response.headers["Content-Type"] = "text/html"
    rpc = NoritoRpcClient(
        NoritoRpcConfig("https://torii.example"),
        session=Session([response]),
    )

    with pytest.raises(NoritoRpcError, match="Content-Type must exactly match"):
        rpc.call("/v1/norito-rpc", b"request")
    assert response.closed_by_client is True


def test_call_rejects_hidden_http_adapter_retries_before_io() -> None:
    session = Session([])
    session.mount("https://", requests.adapters.HTTPAdapter(max_retries=1))
    rpc = NoritoRpcClient(NoritoRpcConfig("https://torii.example"), session=session)

    with pytest.raises(ValueError, match="adapter retries to be disabled"):
        rpc.call("/v1/norito-rpc", b"request")
    assert session.calls == []


def test_declared_oversize_response_is_rejected_before_body_iteration() -> None:
    response = Response(200, b"")
    response.headers["Content-Length"] = "5"
    rpc = NoritoRpcClient(
        NoritoRpcConfig("https://torii.example", max_response_bytes=4),
        session=Session([response]),
    )

    with pytest.raises(NoritoRpcError, match="4-byte limit"):
        rpc.call("/v1/norito-rpc", b"request")
    assert response.closed_by_client is True


def test_error_body_is_bounded_and_reported() -> None:
    response = Response(503, b"down")
    rpc = NoritoRpcClient(
        NoritoRpcConfig("https://torii.example", max_response_bytes=4),
        session=Session([response]),
    )

    with pytest.raises(NoritoRpcError, match="status 503: down"):
        rpc.call("/v1/norito-rpc", b"request")
    assert response.closed_by_client is True


def test_error_detail_is_truncated_before_exception_rendering() -> None:
    response = Response(503, b"x" * 5_000)
    rpc = NoritoRpcClient(
        NoritoRpcConfig("https://torii.example", max_response_bytes=6_000),
        session=Session([response]),
    )

    with pytest.raises(NoritoRpcError) as captured:
        rpc.call("/v1/norito-rpc", b"request")
    assert "<truncated>" in str(captured.value)
    assert len(str(captured.value)) < 4_200
    assert response.closed_by_client is True


def test_config_is_strict_immutable_and_secret_redacted() -> None:
    config = NoritoRpcConfig(
        "https://torii.example/",
        auth_token="auth-secret",
        api_token="api-secret",
        default_headers={"X-Client": "python"},
    )

    assert config.base_url == "https://torii.example"
    assert "auth-secret" not in repr(config)
    assert "api-secret" not in repr(config)
    with pytest.raises(TypeError):
        config.default_headers["X-Client"] = "changed"  # type: ignore[index]
    with pytest.raises(ValueError, match="positive"):
        NoritoRpcConfig("https://torii.example", timeout=0)
    with pytest.raises(ValueError, match="positive integer"):
        NoritoRpcConfig("https://torii.example", max_response_bytes=0)
    with pytest.raises(ValueError, match="api_token explicitly"):
        NoritoRpcConfig(
            "https://torii.example",
            default_headers={"X-API-Token": "secret"},
        )


def test_client_preserves_caller_session_ownership() -> None:
    session = Session([])
    rpc = NoritoRpcClient(NoritoRpcConfig("https://torii.example"), session=session)
    rpc.close()
    assert session.close_count == 0
