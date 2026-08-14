"""Exact operator-auth regressions for Kaigi relay diagnostic reads."""

from __future__ import annotations

import base64
import json
from typing import Any
from urllib.parse import quote

import pytest
import requests
from iroha_torii_client.client import canonical_request_message

from iroha_python import NetworkId, OperatorSigningContext, ToriiClient
from iroha_python.crypto import Ed25519KeyPair


NETWORK_BYTES = bytes([0xB6]) * 32
NETWORK_ID = NetworkId.from_bytes(NETWORK_BYTES)
FOREIGN_NETWORK_BYTES = bytes([0xB7]) * 32
KEY_PAIR = Ed25519KeyPair.from_private_key(bytes([0x2D]) * 32)
RELAY_ID = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"


class RecordingSession(requests.Session):
    """Record requests while retaining Requests' default no-retry adapters."""

    def __init__(self, responses: list[requests.Response]) -> None:
        super().__init__()
        self.responses = list(responses)
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str | bytes, url: str | bytes, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        if not self.responses:
            raise AssertionError("unexpected Kaigi HTTP request")
        return self.responses.pop(0)


def response(status: int, payload: object | None = None) -> requests.Response:
    result = requests.Response()
    result.status_code = status
    result._content = b"" if payload is None else json.dumps(payload).encode("utf-8")
    if payload is not None:
        result.headers["Content-Type"] = "application/json"
    return result


def context() -> OperatorSigningContext:
    return OperatorSigningContext(NETWORK_ID, KEY_PAIR)


def assert_exact_signature(call: dict[str, Any], target: str) -> None:
    headers = call["headers"]
    timestamp = headers["x-iroha-operator-timestamp-ms"]
    nonce = headers["x-iroha-operator-nonce"]
    signature = base64.b64decode(headers["x-iroha-operator-signature"], validate=True)
    canonical = canonical_request_message("GET", target, b"")
    suffix = f"\n{timestamp}\n{nonce}".encode("ascii")
    message = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            NETWORK_BYTES,
            canonical,
            suffix,
        )
    )
    assert KEY_PAIR.verify(message, signature)
    assert not KEY_PAIR.verify(message.replace(NETWORK_BYTES, FOREIGN_NETWORK_BYTES, 1), signature)

    wrong_path = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            NETWORK_BYTES,
            canonical_request_message("GET", "/v1/kaigi/relays/foreign", b""),
            suffix,
        )
    )
    wrong_query = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            NETWORK_BYTES,
            canonical_request_message("GET", f"{target}?format=json", b""),
            suffix,
        )
    )
    assert not KEY_PAIR.verify(wrong_path, signature)
    assert not KEY_PAIR.verify(wrong_query, signature)


def test_kaigi_diagnostics_require_operator_context_and_reject_precomputed_auth() -> None:
    missing_session = RecordingSession([])
    missing = ToriiClient("https://torii.example", session=missing_session)
    for call in (
        missing.list_kaigi_relays,
        lambda: missing.get_kaigi_relay(RELAY_ID),
        missing.get_kaigi_relays_health,
    ):
        with pytest.raises(ValueError, match="operator_signing_context"):
            call()
    assert missing_session.calls == []

    precomputed_session = RecordingSession([])
    precomputed = ToriiClient(
        "https://torii.example",
        session=precomputed_session,
        operator_signing_context=context(),
        default_headers={"X-Iroha-Operator-Nonce": "precomputed"},
    )
    with pytest.raises(ValueError, match="generated operator signing"):
        precomputed.list_kaigi_relays()
    assert precomputed_session.calls == []


def test_kaigi_diagnostics_sign_exact_network_targets_once() -> None:
    session = RecordingSession(
        [
            response(200, {"total": 0, "items": []}),
            response(404),
            response(
                200,
                {
                    "healthy_total": 0,
                    "degraded_total": 0,
                    "unavailable_total": 0,
                    "reports_total": 0,
                    "registrations_total": 0,
                    "failovers_total": 0,
                    "domains": [],
                },
            ),
        ]
    )
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=context(),
        max_retries=7,
        retry_on_methods=["GET"],
        retry_on_status=[503],
    )

    assert client.list_kaigi_relays_typed().total == 0
    assert client.get_kaigi_relay_typed(RELAY_ID) is None
    assert client.get_kaigi_relays_health_typed().healthy_total == 0

    targets = (
        "/v1/kaigi/relays",
        f"/v1/kaigi/relays/{quote(RELAY_ID, safe='')}",
        "/v1/kaigi/relays/health",
    )
    assert len(session.calls) == len(targets)
    for call, target in zip(session.calls, targets):
        assert call["method"] == "GET"
        assert call["url"] == f"https://torii.example{target}"
        assert call["data"] == b""
        assert call["allow_redirects"] is False
        assert_exact_signature(call, target)


def test_kaigi_operator_read_does_not_retry_after_dispatch() -> None:
    session = RecordingSession([response(503, {"error": "unavailable"})])
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=context(),
        max_retries=7,
        retry_on_methods=["GET"],
        retry_on_status=[503],
    )

    with pytest.raises(RuntimeError):
        client.list_kaigi_relays()
    assert len(session.calls) == 1
    assert session.calls[0]["allow_redirects"] is False
