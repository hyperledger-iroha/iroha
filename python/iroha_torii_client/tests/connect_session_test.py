"""Exact-network Connect session client tests."""

from __future__ import annotations

import base64
import hashlib
import json
import sys
from pathlib import Path
from typing import Any, Dict
from urllib.parse import quote

import pytest

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

import iroha_torii_client.client as client_module  # noqa: E402
from iroha_torii_client import ToriiClient  # noqa: E402
from iroha_torii_client.connect_session import normalize_connect_session_request  # noqa: E402
from sumeragi_exact_json_test_support import RecordingSession, StubResponse  # noqa: E402


def _canonical_hash(seed: int) -> str:
    body_bytes = bytearray([seed & 0xFF] * 32)
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


def _base64url(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode("ascii")


def _session_fixture(
    *,
    network_seed: int = 0xA5,
    app_seed: int = 0x41,
    nonce_seed: int = 0x51,
) -> tuple[Dict[str, str], Dict[str, Any]]:
    network_id = _canonical_hash(network_seed)
    network_bytes = bytes.fromhex(network_id[5:69])
    public_network_id = network_bytes.hex()
    app_pk_bytes = bytes([app_seed]) * 32
    nonce_bytes = bytes([nonce_seed]) * 16
    sid = _base64url(
        hashlib.blake2b(
            b"iroha-connect|sid|" + network_bytes + app_pk_bytes + nonce_bytes,
            digest_size=32,
        ).digest()
    )
    app_pk = _base64url(app_pk_bytes)
    nonce = _base64url(nonce_bytes)
    node = "node.example:443"
    token_app = _base64url(bytes([0x61]) * 32)
    token_wallet = _base64url(bytes([0x62]) * 32)
    token_management = _base64url(bytes([0x63]) * 32)
    token_relay = _base64url(bytes([0x64]) * 32)

    def role_uri(role: str, token: str) -> str:
        return (
            "iroha://connect"
            f"?sid={sid}&network_id={public_network_id}&app_pk={app_pk}"
            f"&nonce={nonce}&node={quote(node, safe='')}&v=1&role={role}"
            f"&token={token}&relay={token_relay}"
        )

    request = {
        "sid": sid,
        "network_id": network_id,
        "app_pk": app_pk,
        "nonce": nonce,
        "node": node,
    }
    response: Dict[str, Any] = {
        **request,
        "wallet_uri": role_uri("wallet", token_wallet),
        "app_uri": role_uri("app", token_app),
        "token_app": token_app,
        "token_wallet": token_wallet,
        "token_management": token_management,
        "token_relay": token_relay,
    }
    response.pop("node")
    return request, response


def test_request_matches_exact_sid_vector() -> None:
    request = {
        "sid": "zUU9qC43rOABcvuk8riGm7tXx8LsMAYuYmcBQiOmzDc",
        "network_id": _canonical_hash(0xA5),
        "app_pk": _base64url(bytes(range(32))),
        "nonce": _base64url(bytes(range(0xA0, 0xB0))),
    }

    assert normalize_connect_session_request(
        request,
        hash_literal=client_module._offline_hash_literal,
    ) == request


def test_create_and_delete_session() -> None:
    request, response = _session_fixture()
    session = RecordingSession()
    session.queue(StubResponse(payload=response))
    session.queue(StubResponse(status_code=204))
    client = ToriiClient("http://node.test", session=session)

    session_info = client.create_connect_session(request)
    deleted = client.delete_connect_session(request["sid"], session_info.token_management)

    assert session_info.sid == request["sid"]
    assert session_info.network_id == request["network_id"][5:69].lower()
    assert session_info.app_pk == request["app_pk"]
    assert session_info.nonce == request["nonce"]
    assert session_info.token_relay == response["token_relay"]
    assert deleted is True
    assert json.loads(session.calls[0]["data"]) == request
    assert session.calls[1]["headers"] == {
        "Authorization": f"Bearer {response['token_management']}"
    }


def test_session_keeps_marked_json_network_out_of_public_identity_and_deep_links() -> None:
    request, response = _session_fixture()
    session = RecordingSession()
    session.queue(StubResponse(payload=response))

    info = ToriiClient("http://node.test", session=session).create_connect_session(request)

    raw_network_id = request["network_id"][5:69].lower()
    assert info.network_id == raw_network_id
    assert f"network_id={raw_network_id}" in info.wallet_uri
    assert "network_id=hash%3A" not in info.wallet_uri
    assert json.loads(session.calls[0]["data"])["network_id"] == request["network_id"]


def test_session_rejects_marked_network_identity_in_deep_link() -> None:
    request, response = _session_fixture()
    raw_network_id = request["network_id"][5:69].lower()
    response["wallet_uri"] = response["wallet_uri"].replace(
        f"network_id={raw_network_id}",
        f"network_id={quote(request['network_id'], safe='')}",
    )
    session = RecordingSession()
    session.queue(StubResponse(payload=response))

    with pytest.raises(ValueError, match="substituted Connect session identity"):
        ToriiClient("http://node.test", session=session).create_connect_session(request)


@pytest.mark.parametrize("field", ["chain", "chain_id", "chainId", "genesis_hash", "scope"])
def test_rejects_retired_or_unsupported_fields(field: str) -> None:
    request, _ = _session_fixture()
    request[field] = "retired"
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError):
        client.create_connect_session(request)

    assert session.calls == []


@pytest.mark.parametrize("field", ["sid", "network_id", "app_pk", "nonce"])
def test_rejects_request_identity_substitution(field: str) -> None:
    request, _ = _session_fixture()
    alternate, _ = _session_fixture(network_seed=0xB5, app_seed=0x42, nonce_seed=0x52)
    request[field] = alternate[field]
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError):
        client.create_connect_session(request)

    assert session.calls == []


@pytest.mark.parametrize(
    "alternate",
    [{"network_seed": 0xB5}, {"app_seed": 0x42}, {"nonce_seed": 0x52}],
)
def test_rejects_coherent_response_identity_substitution(
    alternate: Dict[str, int],
) -> None:
    request, _ = _session_fixture()
    _, substituted_response = _session_fixture(**alternate)
    session = RecordingSession()
    session.queue(StubResponse(payload=substituted_response))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="substituted request identity"):
        client.create_connect_session(request)


def test_rejects_duplicate_uri_identity_parameter() -> None:
    request, response = _session_fixture()
    response["app_uri"] = f"{response['app_uri']}&sid={request['sid']}"
    session = RecordingSession()
    session.queue(StubResponse(payload=response))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="duplicate parameters"):
        client.create_connect_session(request)


def test_rejects_response_extension_field() -> None:
    request, response = _session_fixture()
    response["ttl"] = 30
    session = RecordingSession()
    session.queue(StubResponse(payload=response))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="inexact field set"):
        client.create_connect_session(request)


@pytest.mark.parametrize(
    ("sid", "token"),
    [
        ("../status", _base64url(bytes([0x63]) * 32)),
        (_base64url(bytes([0x41]) * 32), "management-token"),
    ],
)
def test_delete_rejects_noncanonical_sid_or_token(sid: str, token: str) -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="canonical unpadded base64url"):
        client.delete_connect_session(sid, token)

    assert session.calls == []
