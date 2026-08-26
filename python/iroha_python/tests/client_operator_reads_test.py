"""Exact-network operator authentication tests for node-local GET helpers."""

from __future__ import annotations

import base64
from typing import Any, Callable

import pytest
import requests
from iroha_torii_client.client import canonical_request_message
from requests.adapters import HTTPAdapter

from iroha_python import NetworkId, OperatorSigningContext, ToriiClient, ToriiPipelinePreflight
from iroha_python.crypto import Ed25519KeyPair

NETWORK_BYTES = bytes([0xA5]) * 32
NETWORK_ID = NetworkId.from_bytes(NETWORK_BYTES)
FOREIGN_NETWORK_ID = NetworkId.from_bytes(bytes([0xA7]) * 32)
KEY_PAIR = Ed25519KeyPair.from_private_key(bytes([0x0B]) * 32)
ACCOUNT_ID = KEY_PAIR.account_id()


def pipeline_preflight_payload() -> dict[str, Any]:
    """Return one exact current pipeline-preflight payload."""

    return {
        "schema_version": 1,
        "chain_height": 42,
        "sumeragi": {
            "block_time_ms": 1_000,
            "commit_time_ms": 2_000,
            "stall_threshold_ms": 6_000,
        },
        "admission": {
            "max_signatures": 32,
            "max_instructions": 4_096,
            "max_tx_bytes": 1_048_576,
            "max_decompressed_bytes": 1_048_576,
            "max_metadata_depth": 16,
        },
        "block": {"max_transactions": 512},
        "pipeline": {
            "signature_batch_max_ed25519": 64,
            "signature_batch_max_secp256k1": 16,
            "signature_batch_max_pqc": 8,
            "signature_batch_max_bls": 16,
            "overlay_max_instructions": 0,
            "ivm_max_cycles_upper_bound": 2_000_000,
            "ivm_admission_cycle_limit": 1_000_000,
            "ivm_max_decoded_instructions": 1_048_576,
        },
        "queue": {"size": 2, "queued": 1, "inflight": 1},
        "fees": {
            "fee_asset_id": "xor#sora",
            "fee_sink_account_id": ACCOUNT_ID,
            "base_fee": "0",
            "per_byte_fee": "0",
            "per_instruction_fee": "0",
            "per_gas_unit_fee": "0",
            "sponsor_vault_custody_account_id": ACCOUNT_ID,
            "settlement_mode": "direct",
            "successful_claim_fee_exempt_authorities": [ACCOUNT_ID],
        },
    }


def test_pipeline_preflight_requires_current_cycle_limits_and_domainless_accounts() -> None:
    preflight = ToriiPipelinePreflight.from_payload(pipeline_preflight_payload())

    assert preflight.pipeline["ivm_max_cycles_upper_bound"] == 2_000_000
    assert preflight.pipeline["ivm_admission_cycle_limit"] == 1_000_000
    assert preflight.fees["fee_sink_account_id"] == ACCOUNT_ID
    assert preflight.fees["successful_claim_fee_exempt_authorities"] == [ACCOUNT_ID]


def test_pipeline_preflight_rejects_missing_current_cycle_limit() -> None:
    payload = pipeline_preflight_payload()
    del payload["pipeline"]["ivm_admission_cycle_limit"]

    with pytest.raises(ValueError, match="missing ivm_admission_cycle_limit"):
        ToriiPipelinePreflight.from_payload(payload)


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("fee_sink_account_id", "fees@system"),
        ("sponsor_vault_custody_account_id", "vault@system"),
        ("successful_claim_fee_exempt_authorities", ["authority@system"]),
    ],
)
def test_pipeline_preflight_rejects_alias_shaped_fee_accounts(
    field: str,
    value: Any,
) -> None:
    payload = pipeline_preflight_payload()
    payload["fees"][field] = value

    with pytest.raises(ValueError, match="exact canonical I105 account id"):
        ToriiPipelinePreflight.from_payload(payload)


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
    with pytest.raises(ValueError, match="session.headers.*Authorization"):
        ToriiClient(
            "https://torii.example",
            session=header_session,
            operator_signing_context=signing_context(),
        )
    assert header_session.calls == []

    auth_session = RecordingSession()
    auth_session.auth = ("retired-user", "retired-password")
    with pytest.raises(ValueError, match="session.auth"):
        ToriiClient(
            "https://torii.example",
            session=auth_session,
            operator_signing_context=signing_context(),
        )
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
