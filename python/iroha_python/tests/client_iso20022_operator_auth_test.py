"""Focused exact-operator-auth tests for the existing ISO 20022 SDK methods."""

from __future__ import annotations

import base64
import json
from typing import Any

import pytest
import requests
from iroha_torii_client.client import canonical_request_message
from requests.adapters import HTTPAdapter

from iroha_python import NetworkId, OperatorSigningContext, ToriiClient
from iroha_python.crypto import Ed25519KeyPair

NETWORK_BYTES = bytes([0xA5]) * 32
NETWORK_ID = NetworkId.from_bytes(NETWORK_BYTES)
KEY_PAIR = Ed25519KeyPair.from_private_key(bytes([0x0B]) * 32)


def response(status: int, payload: object | None = None) -> requests.Response:
    result = requests.Response()
    result.status_code = status
    result._content = b"" if payload is None else json.dumps(payload).encode("utf-8")
    if payload is not None:
        result.headers["Content-Type"] = "application/json"
    return result


class RecordingSession(requests.Session):
    """Requests session with real adapter policy and deterministic responses."""

    def __init__(self, responses: list[requests.Response]) -> None:
        super().__init__()
        self.responses = list(responses)
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str | bytes, url: str | bytes, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        if not self.responses:
            raise AssertionError("unexpected ISO HTTP request")
        return self.responses.pop(0)


def context() -> OperatorSigningContext:
    return OperatorSigningContext(NETWORK_ID, KEY_PAIR)


def test_iso_submission_signs_exact_network_query_and_body_once() -> None:
    session = RecordingSession([response(202, {"message_id": "signed-1", "status": "Accepted"})])
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=context(),
        max_retries=5,
        retry_on_methods=["POST"],
        retry_on_status=[503],
    )
    body = b"<Document><MsgId>signed-1</MsgId></Document>"

    client.submit_iso_pacs008(body, profile="swift-cbpr-plus")

    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"] == ("https://torii.example/v1/iso20022/pacs008?profile=swift-cbpr-plus")
    assert call["params"] is None
    assert call["data"] == body
    assert call["allow_redirects"] is False
    headers = call["headers"]
    assert "X-Iroha-Iso-Profile" not in headers
    timestamp = headers["x-iroha-operator-timestamp-ms"]
    nonce = headers["x-iroha-operator-nonce"]
    signature = base64.b64decode(headers["x-iroha-operator-signature"], validate=True)
    request = canonical_request_message(
        "POST",
        "/v1/iso20022/pacs008?profile=swift-cbpr-plus",
        body,
    )
    message = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            NETWORK_BYTES,
            request,
            f"\n{timestamp}\n{nonce}".encode("ascii"),
        )
    )
    assert KEY_PAIR.verify(message, signature)
    foreign_query = message.replace(b"swift-cbpr-plus", b"foreign-profile")
    assert not KEY_PAIR.verify(foreign_query, signature)


def test_iso_operator_auth_is_mandatory_one_shot_and_rejects_retired_shapes() -> None:
    unsigned_session = RecordingSession([])
    unsigned = ToriiClient("https://torii.example", session=unsigned_session)
    with pytest.raises(ValueError, match="operator_signing_context"):
        unsigned.submit_iso_pacs008(b"<xml/>")
    assert unsigned_session.calls == []

    retry_session = RecordingSession([response(503, {"error": "unavailable"})])
    retrying = ToriiClient(
        "https://torii.example",
        session=retry_session,
        operator_signing_context=context(),
        max_retries=5,
        retry_on_methods=["POST"],
        retry_on_status=[503],
    )
    with pytest.raises(RuntimeError):
        retrying.submit_iso_pacs009(b"<xml/>")
    assert len(retry_session.calls) == 1

    retired_options = (
        {"auth_token": "retired-bearer"},
        {"api_token": "retired-api-token"},
        {"default_headers": {"X-Iroha-Account": "retired-app-auth"}},
        {"default_headers": {"X-Iroha-Iso-Profile": "legacy-profile"}},
        {"default_headers": {"X-Iroha-Operator-Nonce": "precomputed"}},
    )
    for options in retired_options:
        session = RecordingSession([])
        client = ToriiClient(
            "https://torii.example",
            session=session,
            operator_signing_context=context(),
            **options,
        )
        with pytest.raises(ValueError, match="generated operator signing"):
            client.get_iso_message_status("signed-1")
        assert session.calls == []


def test_iso_rejects_retrying_adapter_before_signing_or_dispatch() -> None:
    session = RecordingSession([])
    session.mount("https://", HTTPAdapter(max_retries=1))
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=context(),
    )

    with pytest.raises(ValueError, match="transport retries to be disabled"):
        client.get_iso_message_status("signed-1")
    assert session.calls == []


def test_iso_status_polls_use_fresh_operator_nonces() -> None:
    session = RecordingSession(
        [
            response(200, {"message_id": "poll-1", "status": "Pending"}),
            response(200, {"message_id": "poll-1", "status": "Committed"}),
        ]
    )
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=context(),
    )

    result = client.wait_for_iso_message_status(
        "poll-1",
        poll_interval=0.0,
        max_attempts=2,
    )

    assert result.status == "Committed"
    assert len(session.calls) == 2
    nonces = [call["headers"]["x-iroha-operator-nonce"] for call in session.calls]
    assert nonces[0] != nonces[1]
    assert all(call["allow_redirects"] is False for call in session.calls)


@pytest.mark.parametrize(
    "profile",
    [" Swift-CBPR-Plus", "swift_cbpr_plus", "swift-"],
)
def test_iso_profiles_require_exact_catalog_identifiers(profile: str) -> None:
    session = RecordingSession([])
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=context(),
    )

    with pytest.raises(ValueError, match="canonical lowercase profile id"):
        client.submit_iso_pacs008(b"<xml/>", profile=profile)
    assert session.calls == []
