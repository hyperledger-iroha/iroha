"""Norito-negotiating routes surface Torii's canonical error envelope.

A route that sends ``Accept: application/x-norito`` receives every Torii failure
as one canonical binary ``iroha_torii_shared::ErrorEnvelope`` frame. Each route
family must expose the envelope's code and exact message after one dispatch and
keep its previous error for a body that is not such an envelope. The builders
below are bound to the Rust-authored frames in the bootle-lantern client fixture.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path
from types import SimpleNamespace

import pytest
import requests
from norito.crc64 import crc64

import iroha_python.client as client_module
import iroha_python.crypto as crypto_module
from iroha_python import (
    AccountAddress,
    Ed25519KeyPair,
    NetworkId,
    NoritoRpcClient,
    NoritoRpcConfig,
    NoritoRpcError,
    ToriiCanonicalRequestAuth,
)
from iroha_python.client import LocalSigningContext, ToriiClient
from iroha_torii_client.errors import (
    ToriiBadRequestError,
    ToriiError,
    ToriiForbiddenError,
    ToriiNotFoundError,
)

from .helpers import RecordingSession

NORITO = "application/x-norito"


def error_envelope_contract():
    """Rust-authored ``iroha_torii_shared::ErrorEnvelope`` frames binding the builder."""

    path = (
        Path(__file__).resolve().parents[3]
        / "fixtures/privacy/bootle_lantern_issuance_client_v1.json"
    )
    return json.loads(path.read_text(encoding="utf-8"))["errors"]


def _leb128(value):
    out = bytearray()
    while True:
        byte, value = value & 0x7F, value >> 7
        out.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(out)


def _field(data):
    return _leb128(len(data)) + data


def _text(value):
    encoded = value.encode("utf-8")
    return _field(_leb128(len(encoded)) + encoded)


def error_envelope_frame(code, message):
    """``norito::encode_canonical(&ErrorEnvelope::new(code, message))``."""

    framing = error_envelope_contract()["norito_envelope"]
    payload = _text(code) + _text(message) + _field(b"\x00")
    return (
        b"NRT0\x00\x00"
        + bytes.fromhex(framing["schema_hash_hex"])
        + b"\x00"
        + struct.pack("<QQ", len(payload), crc64(payload))
        + bytes.fromhex(framing["flags_hex"])
        + payload
    )


def json_error_envelope(code, message):
    """``norito::json::to_json(&ErrorEnvelope::new(code, message))`` for escape-free text."""

    return f'{{"code":"{code}","message":"{message}"}}'.encode()


def corrupted(frame):
    """Flip one checksum byte: the bytes are no longer a canonical envelope."""

    altered = bytearray(frame)
    altered[31] ^= 1
    return bytes(altered)


def norito_response(body, status):
    response = requests.Response()
    response.status_code = status
    response.headers["Content-Type"] = NORITO
    response._content = body
    response._content_consumed = True
    return response


def client_for(response):
    session = RecordingSession(response)
    return ToriiClient("http://torii.example", session=session, max_retries=0, chain_discriminant=753), session


def assert_typed(error, error_type, status, code, message):
    assert type(error) is error_type
    assert (error.status, error.code, error.message) == (status, code, message)
    assert error.details == {}
    assert error.reject_code is None
    assert f"({code})" in str(error)


def test_envelope_builder_reproduces_every_rust_authored_error_envelope():
    responses = error_envelope_contract()["responses"]
    frames = [row for row in responses if row["media_type"] == NORITO]
    assert len(frames) == 7
    for row in frames:
        assert error_envelope_frame(row["code"], row["code"]) == bytes.fromhex(row["body_hex"])
    json_row = next(row for row in responses if row["media_type"] == "application/json")
    assert json_error_envelope(json_row["code"], json_row["code"]) == json_row["body_utf8"].encode()


@pytest.mark.parametrize(
    "read",
    [
        lambda client: client.get_sorafs_por_status(),
        lambda client: client.export_sorafs_por_status(),
        lambda client: client.get_sorafs_por_weekly_report("2026-W05"),
    ],
    ids=["status", "export", "weekly-report"],
)
def test_sorafs_por_reads_surface_the_norito_error_envelope_once(read):
    code, message = "sorafs_por_report_not_found", "no PoR report is published for 2026-W05"
    client, session = client_for(norito_response(error_envelope_frame(code, message), 404))
    with pytest.raises(ToriiError) as raised:
        read(client)
    assert_typed(raised.value, ToriiNotFoundError, 404, code, message)
    assert len(session.calls) == 1
    assert session.calls[0]["headers"]["Accept"] == NORITO
    # A body that is not a canonical envelope keeps the untyped classification.
    client, session = client_for(norito_response(corrupted(error_envelope_frame(code, message)), 404))
    with pytest.raises(ToriiNotFoundError) as raised:
        read(client)
    assert raised.value.code is None
    assert len(session.calls) == 1


def test_transaction_submission_surfaces_the_norito_error_envelope_once():
    code = "invalid_transaction_payload"
    message = "signed transaction bytes are not one canonical VersionedSignedTransaction"
    for body, typed in (
        (error_envelope_frame(code, message), True),
        (corrupted(error_envelope_frame(code, message)), False),
    ):
        client, session = client_for(norito_response(body, 400))
        client._ensure_data_model_validation = lambda: None  # type: ignore[method-assign]
        with pytest.raises(ToriiBadRequestError) as raised:
            client.submit_transaction(b"\x01signed-transaction")
        if typed:
            assert_typed(raised.value, ToriiBadRequestError, 400, code, message)
        else:
            assert raised.value.code is None
        assert len(session.calls) == 1
        assert session.calls[0]["headers"]["Accept"] == "application/x-norito, application/json"


def test_committed_transaction_query_surfaces_the_norito_error_envelope_once(monkeypatch):
    code, message = "query_transaction_not_found", "no committed transaction has this hash"
    monkeypatch.setattr(
        crypto_module, "build_find_committed_transaction_query",
        lambda *_args: b"unit-native-query",
    )
    monkeypatch.setattr(client_module, "_normalize_network_id", lambda value, _context: value)
    monkeypatch.setattr(ToriiClient, "_native_transaction_account_id", lambda self, value, _context: value)
    client, session = client_for(norito_response(error_envelope_frame(code, message), 404))
    with pytest.raises(ToriiError) as raised:
        client.get_verified_committed_transaction(
            transaction_hash="11" * 32, authority="reader", network_id="unit-network",
            native_finality_proof_chain_json="[unit-proof]", expected_chain="unit-chain",
            trusted_checkpoint=b"unit-checkpoint", private_key=b"\x44" * 32,
        )
    assert_typed(raised.value, ToriiNotFoundError, 404, code, message)
    assert len(session.calls) == 1
    assert session.calls[0]["data"] == b"unit-native-query"


def test_exact12_capability_transport_surfaces_the_norito_error_envelope_once(monkeypatch):
    network_id = NetworkId.from_bytes(bytes([0xA5]) * 32)
    public_key = Ed25519KeyPair.from_private_key(bytes([0x11]) * 32).public_key
    canonical_auth = ToriiCanonicalRequestAuth(
        network_id=network_id.literal,
        account_id=AccountAddress.from_account(public_key=public_key).to_i105(0x02F1),
        signer=lambda _message: bytes([0x44]) * 64,
        timestamp_ms=4_102_444_801_000,
        nonce="privacy-capability-envelope-test",
    )
    code, message = "privacy_capabilities_forbidden", "the signing account may not read privacy capabilities"
    for body, typed in (
        (error_envelope_frame(code, message), True),
        (corrupted(error_envelope_frame(code, message)), False),
    ):
        client = ToriiClient(
            "https://torii.invalid",
            local_signing_context=LocalSigningContext(network_id),
            chain_discriminant=753,
        )
        calls = []
        response = SimpleNamespace(
            status_code=403,
            headers={"Content-Type": NORITO},
            url="https://torii.invalid/v1/privacy/capabilities",
            history=[],
            close=lambda: None,
            iter_content=lambda **_kwargs: [body],
        )
        monkeypatch.setattr(client, "_request", lambda *args, **_kwargs: calls.append(args) or response)
        if typed:
            with pytest.raises(ToriiForbiddenError) as raised:
                client_module._fetch_authenticated_privacy_capabilities_archive_v1(client, canonical_auth)
            assert_typed(raised.value, ToriiForbiddenError, 403, code, message)
        else:
            with pytest.raises(ValueError, match="status must be 200"):
                client_module._fetch_authenticated_privacy_capabilities_archive_v1(client, canonical_auth)
        assert calls == [("GET", "/v1/privacy/capabilities")]


class _RpcSession(requests.Session):
    """Return one in-memory response and count dispatches."""

    def __init__(self, response: requests.Response) -> None:
        super().__init__()
        self.response = response
        self.calls = 0

    def request(self, method, url, **kwargs):  # type: ignore[override]
        self.calls += 1
        self.response.url = url
        return self.response


def test_norito_rpc_errors_expose_a_complete_envelope_within_the_detail_bound():
    code, message = "norito_rpc_unknown_route", "no Norito RPC handler is registered for this path"
    session = _RpcSession(norito_response(error_envelope_frame(code, message), 404))
    rpc = NoritoRpcClient(NoritoRpcConfig("https://torii.example"), session=session)
    with pytest.raises(NoritoRpcError) as raised:
        rpc.call("/v1/norito-rpc", b"request")
    error = raised.value
    assert (error.status, error.code, error.error_message) == (404, code, message)
    assert str(error) == f"Norito RPC request failed with status 404: {code}: {message}"
    assert session.calls == 1
    # Other bodies, and an envelope beyond the 4 KiB detail bound, keep the
    # lossy status detail without any typed fields.
    for body in (
        corrupted(error_envelope_frame(code, message)),
        error_envelope_frame(code, "x" * 5_000),
    ):
        session = _RpcSession(norito_response(body, 404))
        rpc = NoritoRpcClient(NoritoRpcConfig("https://torii.example"), session=session)
        with pytest.raises(NoritoRpcError) as raised:
            rpc.call("/v1/norito-rpc", b"request")
        error = raised.value
        assert (error.status, error.code, error.error_message) == (404, None, None)
        assert str(error).startswith("Norito RPC request failed with status 404: ")
        assert session.calls == 1
