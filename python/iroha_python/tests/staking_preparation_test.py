"""Native preparation-frame parity and bounded one-dispatch observation transport."""

import json
import logging
import struct
from dataclasses import replace
from pathlib import Path

import pytest
import requests
from norito.crc64 import crc64
from norito.errors import (
    ChecksumMismatchError,
    LengthMismatchError,
    SchemaMismatchError,
    UnsupportedVersionError,
)

from iroha_python import KotodamaQuantity, LocalSigningContext, ToriiClient
from iroha_python.crypto import NetworkId
from iroha_python.validator_staking import (
    StakingAssetScopeV1,
    StakingMonetaryRegistrationV1,
    StakingPreparationRequestV1,
    StakingPreparationV1,
)
from iroha_python.validator_staking import (
    decode_staking_preparation_frame_v1 as decode,
)
from iroha_python.validator_staking import (
    encode_staking_preparation_frame_v1 as encode,
)
from iroha_python.validator_staking import (
    validate_staking_preparation_v1 as validate,
)
from staking_transport_server import observation_server


def fixture(name="registration"):
    lines = (
        (Path(__file__).resolve().parents[3] / "fixtures/validator_staking/preparation_v1.tsv")
        .read_text()
        .splitlines()
    )
    rows = dict(row.split("\t") for row in lines if row and not row.startswith("#"))
    request_bytes = bytes.fromhex(rows[f"prepare_{name}_request"])
    response_bytes = bytes.fromhex(rows[f"prepare_{name}_response"])
    return (
        decode(StakingPreparationRequestV1, request_bytes),
        decode(StakingPreparationV1, response_bytes),
        request_bytes,
        response_bytes,
    )


REFUSED = "staking_preparation_refused"
REASON = "Iroha invariant violation: no funded reward entitlement is available to claim"


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


def client(url, network, *, timeout=3):
    session = requests.Session()
    session.trust_env = False
    logger = logging.Logger("staking.preparation.tests")
    logger.propagate = False
    logger.addHandler(logging.NullHandler())
    return ToriiClient(
        url,
        session=session,
        timeout=timeout,
        sorafs_alias_logger=logger,
        local_signing_context=LocalSigningContext(network),
        max_retries=3,
        retry_on_methods=["POST"],
        chain_discriminant=753,
    )


@pytest.mark.parametrize("name", ["registration", "bond", "unbond", "claim"])
def test_native_preparation_frames_keep_exact_intent_global_xor_and_additive_reserves(name):
    request, prepared, request_bytes, response_bytes = fixture(name)
    assert encode(request) == request_bytes
    assert encode(prepared) == response_bytes
    assert (
        validate(prepared, request, prepared.network_id, prepared.xor_asset_definition_id)
        is prepared
    )
    assert prepared.observed_height == 200 and prepared.assumed_execution_height == 201
    assert len(prepared.balances) == 2
    for row in prepared.balances:
        assert row.asset.scope.dataspace is None
        assert row.asset.definition == "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        assert str(row.stake_reserved) == "1000" and str(row.rewards_reserved) == "22"


def test_preparation_frames_reject_bad_identity_layout_padding_crc_truncation_and_oversize():
    request, _, request_bytes, response_bytes = fixture()
    for end in range(len(response_bytes)):
        with pytest.raises(LengthMismatchError):
            decode(StakingPreparationV1, response_bytes[:end])
    for index, error in (
        (4, UnsupportedVersionError),
        (5, UnsupportedVersionError),
        (6, SchemaMismatchError),
        (22, ValueError),
        (23, LengthMismatchError),
        (31, ChecksumMismatchError),
        (39, SchemaMismatchError),
    ):
        changed = bytearray(response_bytes)
        changed[index] ^= 1
        with pytest.raises(error):
            decode(StakingPreparationV1, bytes(changed))
    for bad, error in (
        (request_bytes, SchemaMismatchError),
        (response_bytes + b"\0", LengthMismatchError),
        (response_bytes[:40] + b"\0" + response_bytes[40:], ValueError),
        (bytes(256 * 1024 + 1), ValueError),
    ):
        with pytest.raises(error):
            decode(StakingPreparationV1, bad)
    with pytest.raises(ValueError, match="positive"):
        replace(request, valid_for_blocks=0)


def test_preparation_binding_rejects_changed_request_network_xor_effects_and_balance_set():
    request, prepared, _, _ = fixture()

    def reject(candidate):
        with pytest.raises(ValueError):
            validate(candidate, request, prepared.network_id, prepared.xor_asset_definition_id)

    reject(replace(prepared, request=replace(request, lane_id=1)))
    other = bytearray(prepared.network_id.to_bytes())
    other[0] ^= 1
    reject(replace(prepared, network_id=NetworkId.from_bytes(bytes(other))))
    reject(replace(prepared, xor_asset_definition_id="62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))
    reject(replace(prepared, observed_height=0))
    reject(replace(prepared, assumed_execution_height=202))
    reject(replace(prepared, balances=tuple(reversed(prepared.balances))))
    reject(replace(prepared, balances=prepared.balances[:1]))
    reject(replace(prepared, plan=replace(prepared.plan, amount=KotodamaQuantity("999"))))
    reject(replace(prepared, plan=replace(prepared.plan, valid_until_height=211)))
    reject(
        replace(
            prepared, plan=replace(prepared.plan, precondition=StakingMonetaryRegistrationV1(0))
        )
    )
    scope = StakingAssetScopeV1(7)
    scoped_plan = replace(
        prepared.plan,
        source_asset=replace(prepared.plan.source_asset, scope=scope),
        destination_asset=replace(prepared.plan.destination_asset, scope=scope),
    )
    reject(
        replace(
            prepared,
            plan=scoped_plan,
            balances=tuple(
                replace(row, asset=replace(row.asset, scope=scope)) for row in prepared.balances
            ),
        )
    )
    with pytest.raises((ValueError, TypeError)):
        validate(prepared, request, prepared.network_id, "fake-xor")


def test_reward_preparation_binds_current_beneficiary_and_mandatory_entitlement():
    request, prepared, _, _ = fixture("claim")
    plan = prepared.plan
    changed = replace(
        plan, fee_claim=replace(plan.fee_claim, destination_asset=plan.fee_claim.source_asset)
    )
    with pytest.raises(ValueError):
        validate(
            replace(prepared, plan=changed),
            request,
            prepared.network_id,
            prepared.xor_asset_definition_id,
        )
    with pytest.raises(TypeError):
        replace(plan, fee_claim=None)
    for field in ("max_records", "upto_epoch", "accrued_sources"):
        with pytest.raises(TypeError):
            replace(request.operation, **{field: None})


def test_transport_sends_one_unsigned_exact_canonical_request_and_closes_stream():
    request, prepared, request_bytes, response_bytes = fixture()
    with observation_server(response_bytes) as server:
        actual = client(server["url"], prepared.network_id).prepare_public_lane_plan(
            request, prepared.xor_asset_definition_id
        )
        assert actual.to_norito() == prepared.to_norito()
        assert len(server["calls"]) == 1 and server["finished"].wait(1)
        method, path, headers, body = server["calls"][0]
        assert method == "POST" and path == "/v1/nexus/staking/prepare"
        assert body == request_bytes
        assert headers["Content-Type"] == "application/x-norito"
        assert not any(key.lower() == "x-iroha-signature" for key in headers)


@pytest.mark.parametrize(
    "kind",
    ["media", "declared_oversize", "stream_oversize", "error", "redirect", "length_mismatch"],
)
def test_transport_rejections_are_bounded_closed_and_never_replayed(kind):
    request, prepared, _, body = fixture()
    headers = {}
    status = 200
    if kind == "media":
        headers["Content-Type"] = "application/json"
    if kind == "declared_oversize":
        headers["Content-Length"] = str(256 * 1024 + 1)
    if kind == "stream_oversize":
        body = bytes(256 * 1024 + 1)
        status = 503
    if kind == "error":
        body = b"unavailable"
        status = 503
    if kind == "redirect":
        status = 302
        headers["Location"] = "/must-not-follow"
    if kind == "length_mismatch":
        headers["Content-Length"] = str(len(body) + 1)
    early = kind in ("media", "declared_oversize")
    with observation_server(
        body, status=status, headers=headers, probe_before_body=early
    ) as server:
        expected = (
            requests.ConnectionError
            if kind == "length_mismatch"
            else (ValueError, requests.HTTPError)
        )
        with pytest.raises(expected) as failure:
            client(server["url"], prepared.network_id).prepare_public_lane_plan(
                request, prepared.xor_asset_definition_id
            )
        assert len(server["calls"]) == 1 and server["finished"].wait(1)
        if kind in ("error", "redirect"):
            assert failure.value.staking_preparation_body == body
        if early:
            assert server["peer_closed"] and server["body_writes"] == 0


def test_refusal_envelope_builder_reproduces_rust_authored_error_envelopes():
    responses = error_envelope_contract()["responses"]
    frames = [row for row in responses if row["media_type"] == "application/x-norito"]
    assert len(frames) == 7
    for row in frames:
        assert error_envelope_frame(row["code"], row["code"]) == bytes.fromhex(row["body_hex"])
    (json_row,) = [row for row in responses if row["media_type"] == "application/json"]
    assert json_error_envelope(json_row["code"], json_row["code"]) == json_row["body_utf8"].encode()


# Envelope classification is not a deadline control; a generous ceiling keeps
# these cases independent of isolated-worker start-up time on a loaded host.
ENVELOPE_CASE_TIMEOUT = 120


@pytest.mark.parametrize("media_type", ["application/x-norito", "application/json"])
def test_refusal_surfaces_typed_code_and_exact_reason_without_replay(media_type):
    request, prepared, _, _ = fixture("claim")
    body = (
        error_envelope_frame(REFUSED, REASON)
        if media_type == "application/x-norito"
        else json_error_envelope(REFUSED, REASON)
    )
    headers = {"Content-Type": media_type, "x-iroha-reject-code": REFUSED}
    with observation_server(body, status=409, headers=headers) as server:
        torii = client(server["url"], prepared.network_id, timeout=ENVELOPE_CASE_TIMEOUT)
        with pytest.raises(requests.HTTPError) as failure:
            torii.prepare_public_lane_plan(request, prepared.xor_asset_definition_id)
        assert len(server["calls"]) == 1 and server["finished"].wait(1)
    error = failure.value
    assert error.response.status_code == 409
    assert error.staking_preparation_code == REFUSED
    assert error.staking_preparation_message == REASON
    assert error.staking_preparation_reject_code == REFUSED
    assert error.staking_preparation_body == body
    assert str(error) == f"staking preparation HTTP 409: {REFUSED}: {REASON}"


@pytest.mark.parametrize("kind", ["corrupted_norito", "noncanonical_json", "foreign_media"])
def test_error_bodies_that_are_not_canonical_envelopes_keep_the_generic_failure(kind):
    request, prepared, _, _ = fixture()
    body = error_envelope_frame(REFUSED, REASON)
    media_type = "application/x-norito"
    if kind == "corrupted_norito":
        changed = bytearray(body)
        changed[31] ^= 1
        body = bytes(changed)
    if kind == "noncanonical_json":
        body = json_error_envelope(REFUSED, REASON) + b" "
        media_type = "application/json"
    if kind == "foreign_media":
        media_type = "application/octet-stream"
    headers = {"Content-Type": media_type, "x-iroha-reject-code": REFUSED}
    with observation_server(body, status=409, headers=headers) as server:
        torii = client(server["url"], prepared.network_id, timeout=ENVELOPE_CASE_TIMEOUT)
        with pytest.raises(requests.HTTPError) as failure:
            torii.prepare_public_lane_plan(request, prepared.xor_asset_definition_id)
        assert len(server["calls"]) == 1 and server["finished"].wait(1)
    error = failure.value
    assert str(error) == "staking preparation HTTP 409"
    assert error.staking_preparation_body == body
    assert error.staking_preparation_code is None
    assert error.staking_preparation_message is None
    assert error.staking_preparation_reject_code == REFUSED


def test_transport_refuses_missing_pin_before_dispatch_and_does_not_retry_transport_error():
    request, prepared, _, body = fixture()
    with observation_server(body) as server:
        with pytest.raises((ValueError, TypeError)):
            ToriiClient(server["url"], chain_discriminant=753).prepare_public_lane_plan(
                request, prepared.xor_asset_definition_id
            )
        assert server["calls"] == []
    with observation_server(disconnect=True) as server:
        with pytest.raises(requests.ConnectionError):
            client(server["url"], prepared.network_id).prepare_public_lane_plan(
                request, prepared.xor_asset_definition_id
            )
        assert len(server["calls"]) == 1 and server["finished"].wait(1)


def test_transport_original_operation_deadline_covers_late_headers_and_slow_body():
    request, prepared, _, body = fixture()
    with observation_server(body, header_delay=0.3, chunk_delay=0.03, chunk_size=1) as server:
        with pytest.raises(requests.Timeout):
            client(server["url"], prepared.network_id, timeout=0.7).prepare_public_lane_plan(
                request, prepared.xor_asset_definition_id
            )
        assert len(server["calls"]) == 1 and server["finished"].wait(1)
        assert server["peer_closed"] and 0 < server["body_writes"] < len(body)
