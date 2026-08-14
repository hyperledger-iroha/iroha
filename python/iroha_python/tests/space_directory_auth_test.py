"""Exact-network account-authentication tests for Space Directory drafts."""

from __future__ import annotations

import base64
import hashlib
import json
from typing import Any

import pytest
import requests

from iroha_python.address import AccountAddress
from iroha_python.client import (
    LocalSigningContext,
    ToriiCanonicalRequestAuth,
    ToriiClient,
    canonical_network_request_signature_message,
)
from iroha_python.crypto import NetworkId


def _account(seed: int) -> str:
    return AccountAddress.from_account(
        domain="space-directory",
        public_key=bytes([seed]) * 32,
    ).to_i105(0x02F1)


def _draft() -> dict[str, Any]:
    payload = b"\x01\x02\x03"
    signing_message = bytearray(hashlib.blake2b(payload, digest_size=32).digest())
    signing_message[-1] |= 1
    return {
        "submitted": False,
        "transaction_payload_b64": base64.b64encode(payload).decode("ascii"),
        "signing_message_b64": base64.b64encode(signing_message).decode("ascii"),
    }


class _Session(requests.Session):
    def __init__(self) -> None:
        super().__init__()
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str, url: str, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        response = requests.Response()
        response.status_code = 200
        response.headers["Content-Type"] = "application/json"
        response._content = json.dumps(_draft()).encode("utf-8")
        response.encoding = "utf-8"
        return response


NETWORK_ID = NetworkId.from_bytes(bytes([0xA5]) * 32)
FOREIGN_NETWORK_ID = NetworkId.from_bytes(bytes([0xA7]) * 32)
AUTHORITY = _account(0x11)
AUTHORITY_HEADER = AccountAddress.parse_encoded(
    AUTHORITY, expected_discriminant=0x02F1
).canonical_hex()


def _client(
    session: _Session,
    *,
    network_id: NetworkId = NETWORK_ID,
    account_id: str = AUTHORITY,
    captured: list[bytes] | None = None,
) -> ToriiClient:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return bytes([0x44]) * 64

    return ToriiClient(
        "https://torii.example",
        session=session,
        local_signing_context=LocalSigningContext(NETWORK_ID),
        canonical_request_auth=ToriiCanonicalRequestAuth(
            network_id=network_id.literal,
            account_id=account_id,
            signer=signer,
            timestamp_ms=4_102_444_801_000,
            nonce="python-space-directory-auth",
        ),
    )


def test_publish_is_signed_once_over_exact_path_and_body() -> None:
    session = _Session()
    captured: list[bytes] = []
    client = _client(session, captured=captured)

    result = client.publish_space_directory_manifest(
        {
            "authority": AUTHORITY,
            "manifest": {
                "version": "1",
                "uaid": "uaid:" + "11" * 32,
                "dataspace": 7,
                "entries": [{"effect": {"allow": True}}],
            },
        }
    )

    assert result["submitted"] is False
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["allow_redirects"] is False
    assert call["headers"]["X-Iroha-Account"] == AUTHORITY_HEADER
    assert captured == [
        canonical_network_request_signature_message(
            NETWORK_ID.literal,
            "POST",
            "/v1/space-directory/manifests",
            call["data"],
            timestamp_ms=4_102_444_801_000,
            nonce="python-space-directory-auth",
        )
    ]


def test_revoke_binds_the_distinct_path_and_body() -> None:
    session = _Session()
    captured: list[bytes] = []
    client = _client(session, captured=captured)

    client.revoke_space_directory_manifest(
        {
            "authority": AUTHORITY,
            "uaid": "uaid:" + "23" * 32,
            "dataspace": 3,
            "revoked_epoch": 9,
        }
    )

    call = session.calls[0]
    assert captured == [
        canonical_network_request_signature_message(
            NETWORK_ID.literal,
            "POST",
            "/v1/space-directory/manifests/revoke",
            call["data"],
            timestamp_ms=4_102_444_801_000,
            nonce="python-space-directory-auth",
        )
    ]


def test_foreign_genesis_is_rejected_before_signing_or_dispatch() -> None:
    session = _Session()
    captured: list[bytes] = []
    client = _client(session, network_id=FOREIGN_NETWORK_ID, captured=captured)

    with pytest.raises(ValueError, match="must match the immutable local_signing_context"):
        client.revoke_space_directory_manifest(
            {
                "authority": AUTHORITY,
                "uaid": "uaid:" + "23" * 32,
                "dataspace": 3,
                "revoked_epoch": 9,
            }
        )

    assert captured == []
    assert session.calls == []


def test_authority_substitution_and_inline_secret_are_rejected_before_dispatch() -> None:
    session = _Session()
    substituted = _client(session, account_id=_account(0x12))
    request = {
        "authority": AUTHORITY,
        "manifest": {
            "version": "1",
            "uaid": "uaid:" + "11" * 32,
            "dataspace": 7,
            "entries": [{"effect": {"allow": True}}],
        },
    }

    with pytest.raises(ValueError, match="must equal the exact payload authority"):
        substituted.publish_space_directory_manifest(request)
    with pytest.raises(ValueError, match="private_key"):
        _client(session).publish_space_directory_manifest(
            {**request, "private_key": "retired-inline-secret"}
        )

    assert session.calls == []
