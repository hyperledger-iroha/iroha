"""Canonical-auth hard-cut tests for protected runtime/governance routes."""

from __future__ import annotations

from typing import Any

import pytest
import requests
from iroha_torii_client.client import ToriiCanonicalRequestAuth, ToriiClient

ACCOUNT = "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"


def _network(seed: int) -> str:
    body_bytes = bytearray([seed] * 32)
    body_bytes[-1] |= 1
    body = body_bytes.hex().upper()
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return f"hash:{body}#{crc:04X}"


def _auth(network_id: str, messages: list[bytes]) -> ToriiCanonicalRequestAuth:
    def sign(message: bytes) -> bytes:
        messages.append(message)
        return bytes([0x44]) * 64

    return ToriiCanonicalRequestAuth(
        network_id=network_id,
        account_id=ACCOUNT,
        signer=sign,
        timestamp_ms=4_102_444_801_000,
        nonce="runtime-governance-auth-test",
    )


class _Response:
    def __init__(self, status_code: int, payload: Any = None) -> None:
        self.status_code = status_code
        self._payload = payload
        self.content = b"" if payload is None else b"{}"
        self.text = ""

    def json(self) -> Any:
        return self._payload


class _Session(requests.Session):
    def __init__(self, response: _Response) -> None:
        super().__init__()
        self.response = response
        self.calls: list[dict[str, Any]] = []

    @staticmethod
    def get_adapter(_url: str) -> requests.adapters.HTTPAdapter:
        return requests.adapters.HTTPAdapter(max_retries=0)

    def request(self, method: str, url: str, **kwargs: Any) -> _Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        return self.response

    def send(self, request: requests.PreparedRequest, **kwargs: Any) -> _Response:
        return self.request(
            request.method or "",
            request.url or "",
            headers=dict(request.headers),
            data=request.body,
            **kwargs,
        )


def test_protected_surface_has_no_networkless_unsigned_overload() -> None:
    session = _Session(_Response(200, {}))
    client = ToriiClient("https://node.test", session=session)

    with pytest.raises(TypeError, match="canonical_auth"):
        client.get_runtime_metrics()

    assert session.calls == []


@pytest.mark.parametrize("status", [307, 308, 503])
def test_nonce_bearing_request_never_redirects_or_retries(status: int) -> None:
    session = _Session(_Response(status))
    client = ToriiClient("https://node.test", session=session)

    with pytest.raises(RuntimeError, match=f"unexpected status {status}"):
        client.get_runtime_metrics(canonical_auth=_auth(_network(0xA5), []))

    assert len(session.calls) == 1
    assert session.calls[0]["allow_redirects"] is False


def test_same_account_and_route_bind_distinct_genesis_networks() -> None:
    primary_messages: list[bytes] = []
    foreign_messages: list[bytes] = []
    payload = {
        "abi_version": 1,
        "upgrade_events_total": {"proposed": 0, "activated": 0, "canceled": 0},
    }
    primary = _Session(_Response(200, payload))
    foreign = _Session(_Response(200, payload))

    ToriiClient("https://node.test", session=primary).get_runtime_metrics(
        canonical_auth=_auth(_network(0xA5), primary_messages)
    )
    ToriiClient("https://node.test", session=foreign).get_runtime_metrics(
        canonical_auth=_auth(_network(0xA7), foreign_messages)
    )

    assert len(primary.calls) == len(foreign.calls) == 1
    assert primary_messages and foreign_messages
    assert primary_messages[0] != foreign_messages[0]
    assert bytes.fromhex(_network(0xA5)[5:69]) in primary_messages[0]
    assert bytes.fromhex(_network(0xA7)[5:69]) in foreign_messages[0]
