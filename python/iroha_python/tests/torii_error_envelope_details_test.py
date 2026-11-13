"""Torii error envelopes whose ``details`` carry queue, retry and fee metadata.

Torii attaches ``details`` to queue rejections and to every HTTP 429 and 503.
The shared decoders must read the Rust-authored frames and JSON renderings in
``fixtures/torii/error_envelope_details_v1.json`` exactly, leave an envelope
whose details carry a structured observation to its caller, and refuse every
other encoding of an otherwise valid envelope. Norito-negotiating routes then
expose the decoded code, message and details on their errors.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path
from types import SimpleNamespace

import pytest
import requests
from norito.crc64 import crc64

from iroha_python import NoritoRpcClient, NoritoRpcConfig, NoritoRpcError
from iroha_python._torii_error_envelope import (
    decode_json_error_envelope_v1,
    decode_norito_error_envelope_v1,
    decode_typed_error_envelope_v1,
)
from iroha_python.client import ToriiClient, _StakingPreparationHTTPError
from iroha_torii_client.errors import (
    ToriiConflictError,
    ToriiError,
    ToriiNotFoundError,
    ToriiRateLimitedError,
    ToriiUnavailableError,
)

from .helpers import RecordingSession

NORITO = "application/x-norito"
JSON_TYPE = "application/json; charset=utf-8"
CONTRACT = json.loads(
    (Path(__file__).resolve().parents[3] / "fixtures/torii/error_envelope_details_v1.json").read_text(
        encoding="utf-8"
    )
)


def _row(name):
    return next(entry for entry in CONTRACT["decoded"] if entry["name"] == name)


def _frame(entry):
    return bytes.fromhex(entry["norito_hex"])


def _json(entry):
    return entry["json_utf8"].encode("utf-8")


def _leb128(value):
    out = bytearray()
    while True:
        byte, value = value & 0x7F, value >> 7
        out.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(out)


def _field(data):
    return _leb128(len(data)) + data


def _string(value):
    encoded = value.encode("utf-8")
    return _leb128(len(encoded)) + encoded


def _some(data):
    return b"\x01" + _leb128(len(data)) + data


_NONE = b"\x00"


def _uint(value, width):
    return value.to_bytes(width, "little")


_TEXT = _string


def _record(*fields):
    """Encode fields in declaration order; ``raw`` replaces a field's bytes."""

    def encode(value, raw=None):
        raw = raw or {}
        out = b""
        for name, encoder, optional in fields:
            if name in raw:
                out += _field(raw[name])
            elif not optional:
                out += _field(encoder(value[name]))
            else:
                out += _field(_NONE if name not in value else _some(encoder(value[name])))
        return out

    return encode


def _undecoded(_value):
    raise AssertionError("pass this observation's field bytes through raw")


_QUEUE = _record(
    ("state", _TEXT, False),
    ("queued", lambda v: _uint(v, 8), False),
    ("capacity", lambda v: _uint(v, 8), False),
    ("saturated", lambda v: bytes([v]), False),
)
_AXT = _record(
    ("code", _TEXT, True),
    ("reason", _TEXT, True),
    ("snapshot_version", lambda v: _uint(v, 8), True),
    ("dataspace", lambda v: _uint(v, 8), True),
    ("lane", lambda v: _uint(v, 4), True),
    ("active_handle_era", lambda v: _uint(v, 8), True),
    ("next_handle_counter", lambda v: _uint(v, 8), True),
)
_FEE = _record(
    ("code", _TEXT, False),
    ("retryable", lambda v: bytes([v]), False),
    ("program_id", _TEXT, True),
    ("program_revision", lambda v: _uint(v, 8), True),
    ("asset_definition_id", _TEXT, True),
    ("required", _TEXT, True),
    ("available", _TEXT, True),
    ("rule_id", _TEXT, True),
    ("observation_height", lambda v: _uint(v, 8), True),
    ("remediation", _TEXT, True),
)
_DETAIL_ENCODERS = {
    "layer": _TEXT, "reject_code": _TEXT, "queue": _QUEUE, "retry_after_seconds": lambda v: _uint(v, 8),
    "endpoint": _TEXT, "field": _TEXT, "expected": _TEXT, "actual": _TEXT, "profile": _TEXT,
    "chain_discriminant": lambda v: _uint(v, 2), "entrypoint_hash": _TEXT, "tx_hash": _TEXT,
    "last_status": _TEXT, "hint": _TEXT, "axt": _AXT, "fee": _FEE,
}
_DETAILS_FIELDS = (
    "ivm_fault", "layer", "reject_code", "queue", "retry_after_seconds", "endpoint", "field", "expected",
    "actual", "profile", "chain_discriminant", "entrypoint_hash", "tx_hash",
    "pipeline_transaction_status_not_found", "finality_attestation_failure", "amx_approval_read_failure",
    "last_status", "hint", "axt", "fee", "alias_setup_report", "account_alias_not_found",
    "account_aliases_by_account_not_found", "sns_registration_not_found", "query_asset_not_found",
    "query_settlement_receipt_not_found",
)
_DETAILS = _record(*((name, _DETAIL_ENCODERS.get(name, _undecoded), True) for name in _DETAILS_FIELDS))


def envelope_payload(code, message, details=None, raw=None):
    present = details is not None or raw is not None
    details_field = _some(_DETAILS(details or {}, raw)) if present else _NONE
    return _field(_string(code)) + _field(_string(message)) + _field(details_field)


def envelope_frame(payload, flags=0x02):
    """Frame ``payload`` under the envelope schema with a valid checksum."""

    framing = CONTRACT["norito_envelope"]
    return (
        b"NRT0\x00\x00"
        + bytes.fromhex(framing["schema_hash_hex"])
        + b"\x00"
        + struct.pack("<QQ", len(payload), crc64(payload))
        + bytes([flags])
        + payload
    )


def test_details_frames_and_json_renderings_decode_exactly_as_rust_authored_them():
    assert CONTRACT["norito_envelope"]["schema_type_name"] == "iroha_torii_shared::ErrorEnvelope"
    assert len(CONTRACT["decoded"]) == 8
    for entry in CONTRACT["decoded"]:
        expected = (entry["code"], entry["message"], entry["details"])
        assert envelope_frame(envelope_payload(*expected)) == _frame(entry), entry["name"]
        assert decode_norito_error_envelope_v1(_frame(entry)) == expected, entry["name"]
        assert decode_json_error_envelope_v1(_json(entry)) == expected, entry["name"]
        assert decode_typed_error_envelope_v1(NORITO, _frame(entry)) == expected
        assert decode_typed_error_envelope_v1(JSON_TYPE, _json(entry)) == expected
    _code, _message, details = decode_norito_error_envelope_v1(_frame(_row("every_decoded_field")))
    assert details["queue"]["queued"] == 2**64 - 1
    assert details["axt"]["lane"] == 2**32 - 1
    assert details["chain_discriminant"] == 65_535
    assert details["hint"] == ""
    assert decode_norito_error_envelope_v1(_frame(_row("empty_details")))[2] == {}


def test_an_envelope_whose_details_carry_a_structured_observation_is_left_to_the_caller():
    assert len(CONTRACT["undecoded"]) == 2
    for entry in CONTRACT["undecoded"]:
        assert decode_typed_error_envelope_v1(NORITO, _frame(entry)) is None
        assert decode_typed_error_envelope_v1(JSON_TYPE, _json(entry)) is None
        with pytest.raises(ValueError, match="observation"):
            decode_norito_error_envelope_v1(_frame(entry))
        with pytest.raises(ValueError, match="observation"):
            decode_json_error_envelope_v1(_json(entry))


def test_norito_details_refuse_every_non_canonical_encoding():
    code, message = "queue_full", "transaction queue is at capacity"
    details = _row("queue_full")["details"]
    details26 = _DETAILS(details)

    def decodes(payload, flags=0x02):
        return decode_typed_error_envelope_v1(NORITO, envelope_frame(payload, flags)) is not None

    def with_raw(**raw):
        return envelope_payload(code, message, details, raw)

    def queue(saturated_byte):
        return _some(
            _field(_string("saturated")) + _field(_uint(65_536, 8)) + _field(_uint(65_536, 8))
            + _field(bytes([saturated_byte]))
        )

    head = _field(_string(code)) + _field(_string(message))
    assert decodes(with_raw(queue=queue(1)))
    cases = {
        "header flags without COMPACT_LEN": (envelope_payload(code, message, details), 0x00),
        "details option tag 2": (head + _field(b"\x02" + _leb128(len(details26)) + details26),),
        "absent details with a trailing byte": (head + _field(b"\x00\x00"),),
        "an empty option field": (with_raw(layer=b""),),
        "absent option with a trailing byte": (with_raw(layer=b"\x00\x00"),),
        "a bool byte of 2": (with_raw(queue=queue(2)),),
        "a seven-byte u64": (with_raw(retry_after_seconds=_some(_uint(1, 7))),),
        "a nine-byte u64": (with_raw(retry_after_seconds=_some(_uint(1, 9))),),
        "a four-byte u16": (with_raw(chain_discriminant=_some(_uint(1, 4))),),
        "an overlong field length": (head + _field(_some(b"\x81\x00" + _NONE + details26[2:])),),
        "trailing bytes after a string": (with_raw(layer=_some(_string("torii") + b"\x00")),),
        "an invalid UTF-8 string": (with_raw(reject_code=_some(b"\x01\xff")),),
        "a present observation": (with_raw(ivm_fault=_some(_string("fault"))),),
        "a missing details field": (head + _field(_some(details26[:-2])),),
        "an extra details field": (head + _field(_some(details26 + _field(_NONE))),),
        "an empty code": (envelope_payload("", message, details),),
    }
    for label, case in cases.items():
        assert not decodes(*case), label
    # A leading U+FEFF is text, not a byte-order mark.
    assert decode_norito_error_envelope_v1(envelope_frame(envelope_payload(code, "﻿m")))[1] == "﻿m"


def test_json_details_refuse_every_non_canonical_rendering():
    def decodes(text):
        return decode_typed_error_envelope_v1(JSON_TYPE, text.encode("utf-8"))

    def envelope(details):
        return '{"code":"c","message":"m","details":' + details + "}"

    assert decodes('{"code":"c","message":"m"}') == ("c", "m", None)
    assert decodes(envelope("{}")) == ("c", "m", {})
    assert decodes(envelope('{"reject_code":"R","retry_after_seconds":1}'))[2] == {
        "reject_code": "R",
        "retry_after_seconds": 1,
    }
    assert decodes(envelope('{"retry_after_seconds":18446744073709551615}'))[2] == {
        "retry_after_seconds": 2**64 - 1
    }
    assert decodes('{"code":"c","message":"﻿m"}')[1] == "﻿m"
    queue_full = _row("queue_full")["json_utf8"]
    non_canonical = [
        queue_full.replace('"code":', '"code": '),
        queue_full + "\n",
        "﻿" + queue_full,
        '{"message":"m","code":"c"}',
        '{"code":"c","message":"m",}',
        '{"code":"","message":"m"}',
        '{"code":"c","message":"m","details":null}',
        envelope('{"retry_after_seconds":1,"reject_code":"R"}'),
        envelope('{"retry_after_secs":1}'),
        envelope('{"layer":null}'),
        envelope('{"layer":"a","layer":"b"}'),
        envelope('{"retry_after_seconds":01}'),
        envelope('{"retry_after_seconds":1.0}'),
        envelope('{"retry_after_seconds":1e0}'),
        envelope('{"retry_after_seconds":-1}'),
        envelope('{"retry_after_seconds":"1"}'),
        envelope('{"retry_after_seconds":true}'),
        envelope('{"retry_after_seconds":18446744073709551616}'),
        envelope('{"chain_discriminant":65536}'),
        envelope('{"axt":{"lane":4294967296}}'),
        envelope('{"queue":{"state":"healthy","queued":1,"capacity":2,"saturated":1}}'),
        envelope('{"queue":{"state":"healthy","queued":1,"capacity":2}}'),
        envelope('{"fee":{"code":"vault_insufficient"}}'),
        envelope('{"ivm_fault":{}}'),
        envelope('{"account_alias_not_found":{"alias":"a@b"}}'),
        '{"code":"c","message":"\\u001F"}',
        '{"code":"c","message":"\\/"}',
        '{"code":"c","message":"\\u0061"}',
        '{"code":"c","message":"\\ud83c\\udf38"}',
        '{"code":"c","message":"\\ud800"}',
        '{"code":"c","message":"m"',
    ]
    for text in non_canonical:
        assert decodes(text) is None, text
    assert decode_typed_error_envelope_v1(JSON_TYPE, b"{\xff}") is None


def _response(body, status, content_type, headers):
    response = requests.Response()
    response.status_code = status
    response.headers["Content-Type"] = content_type
    response.headers.update(headers)
    response._content = body
    response._content_consumed = True
    return response


def _torii_headers(entry):
    """Torii's reject-code and Retry-After headers for a details row."""

    headers = {}
    details = entry["details"]
    if "reject_code" in details:
        headers["x-iroha-reject-code"] = details["reject_code"]
    if "retry_after_seconds" in details:
        headers["Retry-After"] = str(details["retry_after_seconds"])
    return headers


_SUBMISSION_ERRORS = {
    429: ToriiRateLimitedError,
    409: ToriiConflictError,
    422: ToriiError,
    503: ToriiUnavailableError,
}


@pytest.mark.parametrize(
    "name",
    [
        "queue_full",
        "admission_deferred",
        "already_committed",
        "queue_nexus_fee_rejected",
        "transaction_admission_deferred",
    ],
)
@pytest.mark.parametrize("media_type", [NORITO, JSON_TYPE])
@pytest.mark.parametrize("with_headers", [True, False], ids=["headers", "details-only"])
def test_transaction_submission_exposes_queue_rejection_and_retry_details(name, media_type, with_headers):
    entry = _row(name)
    body = _frame(entry) if media_type == NORITO else _json(entry)
    headers = _torii_headers(entry) if with_headers else {}
    response = _response(body, entry["status"], media_type, headers)
    session = RecordingSession(response)
    client = ToriiClient("http://torii.example", session=session, max_retries=0, chain_discriminant=753)
    client._ensure_data_model_validation = lambda: None  # type: ignore[method-assign]
    with pytest.raises(ToriiError) as raised:
        client.submit_transaction(b"\x01signed-transaction")
    error = raised.value
    assert type(error) is _SUBMISSION_ERRORS[entry["status"]]
    assert (error.status, error.code, error.message) == (entry["status"], entry["code"], entry["message"])
    assert error.details == entry["details"]
    # Without Torii's headers the details alone carry the reject code and delay.
    assert error.reject_code == entry["details"].get("reject_code")
    assert error.retry_after == entry["details"].get("retry_after_seconds")
    assert len(session.calls) == 1


def test_transaction_submission_keeps_an_observation_envelope_opaque():
    entry = CONTRACT["undecoded"][0]
    response = _response(_frame(entry), entry["status"], NORITO, {})
    session = RecordingSession(response)
    client = ToriiClient("http://torii.example", session=session, max_retries=0, chain_discriminant=753)
    client._ensure_data_model_validation = lambda: None  # type: ignore[method-assign]
    with pytest.raises(ToriiNotFoundError) as raised:
        client.submit_transaction(b"\x01signed-transaction")
    assert (raised.value.code, raised.value.details) == (None, {})


@pytest.mark.parametrize("media_type", [NORITO, JSON_TYPE])
def test_staking_preparation_errors_expose_envelope_details(media_type):
    entry = _row("transaction_admission_deferred")
    body = _frame(entry) if media_type == NORITO else _json(entry)
    response = SimpleNamespace(status_code=503, headers={"Content-Type": media_type, "Retry-After": "1"})
    error = _StakingPreparationHTTPError.from_response(response, body)
    assert error.staking_preparation_code == entry["code"]
    assert error.staking_preparation_message == entry["message"]
    assert error.staking_preparation_details == {"retry_after_seconds": 1}
    assert error.staking_preparation_reject_code is None
    queue_full = _row("queue_full")
    response = SimpleNamespace(status_code=429, headers={"Content-Type": NORITO})
    error = _StakingPreparationHTTPError.from_response(response, _frame(queue_full))
    assert error.staking_preparation_details == queue_full["details"]
    assert error.staking_preparation_reject_code == "PRTRY:QUEUE_FULL"
    opaque = CONTRACT["undecoded"][0]
    error = _StakingPreparationHTTPError.from_response(
        SimpleNamespace(status_code=404, headers={"Content-Type": NORITO}), _frame(opaque)
    )
    assert error.staking_preparation_details is None
    assert error.staking_preparation_code is None


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


def test_norito_rpc_errors_expose_envelope_details():
    entry = _row("proof_rate_limited")
    session = _RpcSession(_response(_frame(entry), 429, NORITO, {"Retry-After": "3"}))
    rpc = NoritoRpcClient(NoritoRpcConfig("https://torii.example"), session=session)
    with pytest.raises(NoritoRpcError) as raised:
        rpc.call("/v1/norito-rpc", b"request")
    error = raised.value
    assert (error.status, error.code, error.error_message) == (429, entry["code"], entry["message"])
    assert error.details == {"retry_after_seconds": 3, "endpoint": "/v1/zk/proofs"}
    assert session.calls == 1
