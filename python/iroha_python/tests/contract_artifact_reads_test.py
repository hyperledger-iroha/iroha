"""Exact network/dataspace binding and one-shot authenticated artifact reads."""

from __future__ import annotations

import base64
import hashlib
import json

import pytest

from client_contract_manifest_test import _full_manifest_payload
from expensive_query_auth_test import ACCOUNT_ID, NETWORK_ID, _Session, _client, _response
from iroha_python import ContractArtifactId
from iroha_python.client import ToriiCanonicalRequestAuth, canonical_network_request_signature_message


def _auth(captured: list[bytes]) -> ToriiCanonicalRequestAuth:
    def sign(message: bytes) -> bytes:
        captured.append(message)
        return bytes([0x44]) * 64

    return ToriiCanonicalRequestAuth(
        network_id=NETWORK_ID.literal,
        account_id=ACCOUNT_ID,
        signer=sign,
        timestamp_ms=4_102_444_801_000,
        nonce="artifact-read",
    )


def _install_response(session: _Session, payload: dict) -> None:
    response = _response()
    response._content = json.dumps(payload).encode()
    session.responses.append(response)


def test_manifest_read_signs_the_exact_full_width_artifact_path() -> None:
    artifact = ContractArtifactId((1 << 64) - 1, "b" * 64)
    session = _Session([])
    payload = {
        "network_id": NETWORK_ID.literal,
        "artifact_id": artifact.to_payload(),
        "manifest": _full_manifest_payload(),
        "code_hash": artifact.code_hash,
        "abi_hash": "d" * 64,
    }
    _install_response(session, payload)
    captured: list[bytes] = []
    client = _client(session, api_token="owner-token")
    record = client.get_contract_manifest_typed(artifact, canonical_auth=_auth(captured))
    assert record.artifact_id == artifact
    assert len(session.calls) == len(captured) == 1
    call = session.calls[0]
    assert call["path"] == artifact.path
    assert call["headers"]["X-API-Token"] == "owner-token"
    assert call["allow_redirects"] is False
    assert captured[0] == canonical_network_request_signature_message(
        NETWORK_ID.literal, "GET", artifact.path, b"",
        timestamp_ms=4_102_444_801_000, nonce="artifact-read",
    )


@pytest.mark.parametrize("mutation", [None, "network", "dataspace", "digest", "base64", "unknown"])
def test_artifact_bytes_verify_scope_and_complete_domain_digest(mutation: str | None) -> None:
    code = b"complete contract header, interface, literals, and instructions"
    digest = bytearray(hashlib.blake2b(b"iroha:ivm:contract-artifact:v1\0" + code, digest_size=32).digest())
    digest[-1] |= 1
    artifact = ContractArtifactId((1 << 64) - 1, digest.hex())
    payload = {
        "network_id": NETWORK_ID.literal,
        "artifact_id": artifact.to_payload(),
        "code_b64": base64.b64encode(code).decode(),
    }
    if mutation == "network":
        payload["network_id"] = ContractArtifactId(0, "b" * 64).to_payload()["code_hash"]
    elif mutation == "dataspace":
        payload["artifact_id"] = ContractArtifactId(0, artifact.code_hash).to_payload()
    elif mutation == "digest":
        payload["code_b64"] = base64.b64encode(code + b"tampered").decode()
    elif mutation == "base64":
        payload["code_b64"] += "\n"
    elif mutation == "unknown":
        payload["unbound"] = True
    session = _Session([])
    _install_response(session, payload)
    captured: list[bytes] = []
    client = _client(session)
    if mutation is None:
        assert client.get_contract_code_bytes(artifact, canonical_auth=_auth(captured)) == payload
    else:
        with pytest.raises((RuntimeError, TypeError, ValueError)):
            client.get_contract_code_bytes(artifact, canonical_auth=_auth(captured))
    assert len(session.calls) == len(captured) == 1
    assert session.calls[0]["path"] == artifact.path + "/bytes"


def test_artifact_read_rejects_hash_only_input_before_any_request() -> None:
    session = _Session([])
    client = _client(session)
    for read in [client.get_contract_manifest, client.get_contract_manifest_typed, client.get_contract_code_bytes]:
        with pytest.raises(TypeError, match="ContractArtifactId"):
            read("b" * 64, canonical_auth=_auth([]))
    assert session.calls == []
    assert not hasattr(client, "register_contract_code")
