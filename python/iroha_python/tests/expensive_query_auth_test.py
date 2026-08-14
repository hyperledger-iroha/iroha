"""Exact-network authentication tests for expensive application queries."""

from __future__ import annotations

import json
from typing import Any
from urllib.parse import quote, urlsplit

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


NETWORK_ID = NetworkId.from_bytes(bytes([0xA5]) * 32)
FOREIGN_NETWORK_ID = NetworkId.from_bytes(bytes([0xA7]) * 32)
ACCOUNT_ID = AccountAddress.from_account(
    domain="query-auth",
    public_key=bytes([0x31]) * 32,
).to_i105(0x02F1)
ACCOUNT_HEADER = AccountAddress.parse_encoded(
    ACCOUNT_ID, expected_discriminant=0x02F1
).canonical_hex()


def _response(status: int = 200) -> requests.Response:
    response = requests.Response()
    response.status_code = status
    response.headers["Content-Type"] = "application/json"
    response._content = json.dumps({"items": [], "total": 0}).encode()
    response.encoding = "utf-8"
    return response


class _Session(requests.Session):
    def __init__(self, statuses: list[int]) -> None:
        super().__init__()
        self.responses = [_response(status) for status in statuses]
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str, url: str, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "path": urlsplit(url).path, **kwargs})
        if not self.responses:
            raise AssertionError(f"unexpected request {method} {url}")
        response = self.responses.pop(0)
        response.url = url
        return response


def _client(
    session: _Session,
    *,
    auth_network: NetworkId = NETWORK_ID,
    captured: list[bytes] | None = None,
    default_headers: dict[str, str] | None = None,
) -> ToriiClient:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return bytes([0x44]) * 64

    return ToriiClient(
        "https://torii.example",
        session=session,
        max_retries=4,
        default_headers=default_headers,
        local_signing_context=LocalSigningContext(NETWORK_ID),
        canonical_request_auth=ToriiCanonicalRequestAuth(
            network_id=auth_network.literal,
            account_id=ACCOUNT_ID,
            signer=signer,
            timestamp_ms=4_102_444_801_000,
            nonce="python-expensive-query-auth",
        ),
    )


def test_all_existing_query_callers_sign_the_exact_one_shot_target() -> None:
    session = _Session([200] * 10)
    captured: list[bytes] = []
    client = _client(session, captured=captured)

    client.query_account_transactions(ACCOUNT_ID, limit=1)
    client.query_account_assets(ACCOUNT_ID, limit=1)
    client.query_domains(limit=1)
    client.query_accounts(limit=1)
    client.query_transactions(limit=1)
    client.query_transactions(limit=1, visible=True)
    client.query_repo_agreements({"pagination": {"limit": 1}})
    client.query_asset_holders("rose#wonderland", limit=1)
    client.query_asset_definitions(limit=1)
    client.query_rwas(limit=1)

    assert [call["path"] for call in session.calls] == [
        f"/v1/accounts/{quote(ACCOUNT_ID, safe='')}/transactions/query",
        f"/v1/accounts/{quote(ACCOUNT_ID, safe='')}/assets/query",
        "/v1/domains/query",
        "/v1/accounts/query",
        "/v1/transactions/query",
        "/v1/transactions/visible/query",
        "/v1/repo/agreements/query",
        "/v1/assets/rose%23wonderland/holders/query",
        "/v1/assets/definitions/query",
        "/v1/rwas/query",
    ]
    assert len(captured) == len(session.calls)
    for call, message in zip(session.calls, captured, strict=True):
        assert call["method"] == "POST"
        assert call["allow_redirects"] is False
        assert call["headers"]["X-Iroha-Account"] == ACCOUNT_HEADER
        assert message == canonical_network_request_signature_message(
            NETWORK_ID.literal,
            "POST",
            call["path"],
            call["data"],
            timestamp_ms=4_102_444_801_000,
            nonce="python-expensive-query-auth",
        )

    assert captured[0] != captured[1], "the substituted account route must be signed"
    assert captured[3] != captured[4], "the exact final route and body must be signed"


def test_foreign_genesis_and_legacy_auth_shapes_fail_before_dispatch() -> None:
    foreign_session = _Session([])
    captured: list[bytes] = []
    foreign = _client(
        foreign_session,
        auth_network=FOREIGN_NETWORK_ID,
        captured=captured,
    )
    with pytest.raises(ValueError, match="immutable local_signing_context"):
        foreign.query_accounts(limit=1)
    assert captured == []
    assert foreign_session.calls == []

    missing_session = _Session([])
    missing = ToriiClient(
        "https://torii.example",
        session=missing_session,
        local_signing_context=LocalSigningContext(NETWORK_ID),
    )
    with pytest.raises(ValueError, match="canonical_auth is required"):
        missing.query_accounts(limit=1)
    with pytest.raises(TypeError, match="private_key"):
        missing.query_accounts(limit=1, private_key="inline-secret")  # type: ignore[call-arg]
    assert missing_session.calls == []

    precomputed_session = _Session([])
    with pytest.raises(ValueError, match="canonical authentication headers"):
        _client(precomputed_session, default_headers={"X-Iroha-Signature": "precomputed"})
    assert precomputed_session.calls == []


def test_query_dispatch_is_not_retried_after_a_503() -> None:
    session = _Session([503])
    client = _client(session)
    with pytest.raises(RuntimeError, match="unexpected status 503"):
        client.query_accounts(limit=1)
    assert len(session.calls) == 1
