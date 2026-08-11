"""Exact-network authentication tests for tenant-scoped ZK attachments."""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path
from typing import Any, Dict, List

import pytest
import requests

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import (  # noqa: E402
    ToriiCanonicalRequestAuth,
    ToriiClient,
    canonical_network_request_signature_message,
)


def _network_id(seed: int) -> str:
    body = bytearray([seed] * 32)
    body[-1] |= 1
    literal = f"hash:{body.hex().upper()}"
    crc = 0xFFFF
    for byte in literal.encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return f"{literal}#{crc:04X}"


NETWORK_ID = _network_id(0xA7)
FOREIGN_NETWORK_ID = _network_id(0xB9)
ACCOUNT_ID = "attachment-owner@wonderland"
TIMESTAMP_MS = 4_102_444_801_000
NONCE = "python-zk-attachment-auth"


class RecordingSession:
    """Minimal requests session recording exact transport inputs."""

    def __init__(self) -> None:
        self.calls: List[Dict[str, Any]] = []

    def request(self, method: str, url: str, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        response = requests.Response()
        response.url = url
        if method == "POST":
            response.status_code = 201
            response._content = json.dumps(
                {"id": "att/1", "content_type": "text/plain", "size": 7, "created_ms": 1}
            ).encode("utf-8")
            response.headers["Content-Type"] = "application/json"
        elif method == "GET" and url.endswith("/v1/zk/attachments"):
            response.status_code = 200
            response._content = b"[]"
            response.headers["Content-Type"] = "application/json"
        elif method == "GET":
            response.status_code = 200
            response._content = b"payload"
            response.headers["Content-Type"] = "text/plain"
        else:
            response.status_code = 204
            response._content = b""
        return response


def _auth(messages: List[bytes]) -> ToriiCanonicalRequestAuth:
    def signer(message: bytes) -> bytes:
        messages.append(message)
        return b"\x55" * 64

    return ToriiCanonicalRequestAuth(
        network_id=NETWORK_ID,
        account_id=ACCOUNT_ID,
        signer=signer,
        timestamp_ms=TIMESTAMP_MS,
        nonce=NONCE,
    )


def test_attachment_lifecycle_signs_exact_network_path_and_body_one_shot() -> None:
    session = RecordingSession()
    messages: List[bytes] = []
    auth = _auth(messages)
    client = ToriiClient("https://torii.example", session=session)  # type: ignore[arg-type]

    meta = client.upload_attachment(b"payload", content_type="text/plain", canonical_auth=auth)
    assert meta["id"] == "att/1"
    assert client.list_attachments(canonical_auth=auth) == []
    assert client.get_attachment("att/1", canonical_auth=auth) == (b"payload", "text/plain")
    client.delete_attachment("att/1", canonical_auth=auth)

    expected = [
        ("POST", "/v1/zk/attachments", b"payload"),
        ("GET", "/v1/zk/attachments", b""),
        ("GET", "/v1/zk/attachments/att%2F1", b""),
        ("DELETE", "/v1/zk/attachments/att%2F1", b""),
    ]
    assert len(session.calls) == len(messages) == len(expected)
    for call, message, (method, path, body) in zip(session.calls, messages, expected):
        assert call["method"] == method
        assert call["url"] == f"https://torii.example{path}"
        assert call["allow_redirects"] is False
        headers = call["headers"]
        assert headers["X-Iroha-Account"] == ACCOUNT_ID
        assert headers["X-Iroha-Signature"] == base64.b64encode(b"\x55" * 64).decode("ascii")
        canonical = canonical_network_request_signature_message(
            NETWORK_ID, method, path, body, timestamp_ms=TIMESTAMP_MS, nonce=NONCE
        )
        foreign = canonical_network_request_signature_message(
            FOREIGN_NETWORK_ID, method, path, body, timestamp_ms=TIMESTAMP_MS, nonce=NONCE
        )
        assert message == canonical
        assert message != foreign


def test_attachment_methods_fail_before_dispatch_without_auth() -> None:
    session = RecordingSession()
    client = ToriiClient("https://torii.example", session=session)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        client.upload_attachment(b"", content_type="application/octet-stream")  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        client.list_attachments()  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        client.get_attachment("att-1")  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        client.delete_attachment("att-1")  # type: ignore[call-arg]
    with pytest.raises(ValueError, match="canonical_auth is required"):
        client.list_attachments(canonical_auth=None)  # type: ignore[arg-type]
    with pytest.raises(TypeError, match="must be ToriiCanonicalRequestAuth"):
        client.list_attachments(canonical_auth=object())  # type: ignore[arg-type]
    assert session.calls == []
