"""Exact-network operator authentication tests for node-local GET helpers."""

from __future__ import annotations

import base64
from typing import Any, Callable

import pytest
import requests
from iroha_torii_client.client import canonical_request_message
from requests.adapters import HTTPAdapter

from iroha_python import NetworkId, OperatorSigningContext, ToriiClient
from iroha_python.crypto import Ed25519KeyPair

NETWORK_BYTES = bytes([0xA5]) * 32
NETWORK_ID = NetworkId.from_bytes(NETWORK_BYTES)
FOREIGN_NETWORK_ID = NetworkId.from_bytes(bytes([0xA7]) * 32)
KEY_PAIR = Ed25519KeyPair.from_private_key(bytes([0x0B]) * 32)


class RecordingSession(requests.Session):
    """Record exactly one request and return a fixed unavailable response."""

    def __init__(self) -> None:
        super().__init__()
        self.calls: list[dict[str, Any]] = []

    def request(self, method: str | bytes, url: str | bytes, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        response = requests.Response()
        response.status_code = 503
        response._content = b""
        return response


def signing_context(network_id: NetworkId = NETWORK_ID) -> OperatorSigningContext:
    return OperatorSigningContext(network_id, KEY_PAIR)


OPERATOR_READS: tuple[tuple[str, Callable[[ToriiClient], object]], ...] = (
    ("/v1/configuration", lambda client: client.get_configuration()),
    ("/v1/peers", lambda client: client.list_peers()),
    ("/v1/time/status", lambda client: client.get_time_status()),
    ("/v1/pipeline/preflight", lambda client: client.get_pipeline_preflight()),
    ("/v1/pipeline/recovery/42", lambda client: client.get_pipeline_recovery(42)),
    ("/v1/sumeragi/status", lambda client: client.get_sumeragi_status()),
    (
        "/v1/sumeragi/diagnostics",
        lambda client: client.get_sumeragi_diagnostics(),
    ),
    ("/v1/sumeragi/qc", lambda client: client.get_sumeragi_qc()),
    (
        f"/v1/sumeragi/commit-qcs/{'ab' * 32}",
        lambda client: client.get_sumeragi_commit_qc("ab" * 32),
    ),
    ("/v1/sumeragi/leader", lambda client: client.get_sumeragi_leader()),
    (
        "/v1/sumeragi/evidence/count",
        lambda client: client.get_sumeragi_evidence_count(),
    ),
    (
        "/v1/sumeragi/evidence?kind=Equivocation&limit=2&offset=1",
        lambda client: client.list_sumeragi_evidence(
            limit=2,
            offset=1,
            kind="Equivocation",
        ),
    ),
    ("/v1/sumeragi/params", lambda client: client.get_sumeragi_params()),
)


@pytest.mark.parametrize(("path", "invoke"), OPERATOR_READS)
def test_operator_reads_sign_exact_path_network_and_empty_body_once(
    path: str,
    invoke: Callable[[ToriiClient], object],
) -> None:
    session = RecordingSession()
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=signing_context(),
        max_retries=5,
        retry_on_methods=["GET"],
        retry_on_status=[503],
    )

    with pytest.raises(RuntimeError):
        invoke(client)

    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"] == f"https://torii.example{path}"
    assert call["params"] is None
    assert call["data"] == b""
    assert call["allow_redirects"] is False
    headers = call["headers"]
    assert "Authorization" not in headers
    assert "X-API-Token" not in headers

    timestamp = headers["x-iroha-operator-timestamp-ms"]
    nonce = headers["x-iroha-operator-nonce"]
    signature = base64.b64decode(headers["x-iroha-operator-signature"], validate=True)
    canonical = canonical_request_message("GET", path, b"")
    local_message = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            NETWORK_BYTES,
            canonical,
            f"\n{timestamp}\n{nonce}".encode("ascii"),
        )
    )
    foreign_message = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            bytes(FOREIGN_NETWORK_ID.to_bytes()),
            canonical,
            f"\n{timestamp}\n{nonce}".encode("ascii"),
        )
    )
    assert KEY_PAIR.verify(local_message, signature)
    assert not KEY_PAIR.verify(foreign_message, signature)


@pytest.mark.parametrize(("path", "invoke"), OPERATOR_READS)
def test_operator_reads_fail_before_dispatch_without_context(
    path: str,
    invoke: Callable[[ToriiClient], object],
) -> None:
    del path
    session = RecordingSession()
    client = ToriiClient("https://torii.example", session=session)

    with pytest.raises(ValueError, match="operator_signing_context"):
        invoke(client)

    assert session.calls == []


def test_operator_reads_reject_session_auth_and_adapter_retries_before_dispatch() -> None:
    header_session = RecordingSession()
    header_session.headers["Authorization"] = "Bearer retired"
    header_client = ToriiClient(
        "https://torii.example",
        session=header_session,
        operator_signing_context=signing_context(),
    )
    with pytest.raises(ValueError, match="Authorization"):
        header_client.list_peers()
    assert header_session.calls == []

    auth_session = RecordingSession()
    auth_session.auth = ("retired-user", "retired-password")
    auth_client = ToriiClient(
        "https://torii.example",
        session=auth_session,
        operator_signing_context=signing_context(),
    )
    with pytest.raises(ValueError, match="Session.auth"):
        auth_client.list_peers()
    assert auth_session.calls == []

    retry_session = RecordingSession()
    retry_session.mount("https://", HTTPAdapter(max_retries=1))
    retry_client = ToriiClient(
        "https://torii.example",
        session=retry_session,
        operator_signing_context=signing_context(),
    )
    with pytest.raises(ValueError, match="retries to be disabled"):
        retry_client.get_time_status()
    assert retry_session.calls == []


def test_operator_reads_generate_a_fresh_nonce_for_each_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=signing_context(),
    )

    for _ in range(2):
        with pytest.raises(RuntimeError):
            client.list_peers()

    assert len(session.calls) == 2
    assert (
        session.calls[0]["headers"]["x-iroha-operator-nonce"]
        != session.calls[1]["headers"]["x-iroha-operator-nonce"]
    )
