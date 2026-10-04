"""Native preparation-frame parity and bounded one-dispatch observation transport."""
from dataclasses import replace
from pathlib import Path

import pytest
import requests
from requests.structures import CaseInsensitiveDict
from norito.errors import DecodeError

from iroha_python import KotodamaQuantity, LocalSigningContext, ToriiClient
from iroha_python.crypto import NetworkId
from iroha_python.validator_staking import (
    StakingAssetScopeV1, StakingPreparationRequestV1, StakingPreparationV1,
    StakingMonetaryRegistrationV1,
    encode_staking_preparation_frame_v1 as encode,
    decode_staking_preparation_frame_v1 as decode,
    validate_staking_preparation_v1 as validate,
)


def fixture(name="registration"):
    lines = (Path(__file__).resolve().parents[3] / "fixtures/validator_staking/preparation_v1.tsv").read_text().splitlines()
    rows = dict(row.split("\t") for row in lines if row and not row.startswith("#"))
    request_bytes = bytes.fromhex(rows[f"prepare_{name}_request"])
    response_bytes = bytes.fromhex(rows[f"prepare_{name}_response"])
    return decode(StakingPreparationRequestV1, request_bytes), decode(StakingPreparationV1, response_bytes), request_bytes, response_bytes


class Response(requests.Response):
    """A real Requests surface with controlled streamed chunks and close evidence."""
    def __init__(self, body, status=200, headers=None):
        super().__init__()
        self.status_code = status
        self.headers = CaseInsensitiveDict({"Content-Type": "application/x-norito", **(headers or {})})
        self.body = body
        self.closed = False
        self.reads = 0

    def iter_content(self, chunk_size=8192, decode_unicode=False):
        assert decode_unicode is False
        for offset in range(0, len(self.body), chunk_size):
            self.reads += 1
            yield self.body[offset:offset + chunk_size]

    def close(self):
        self.closed = True


class Session(requests.Session):
    """No socket access; record exactly one outgoing native request."""
    def __init__(self, response):
        super().__init__()
        self.response = response
        self.calls = []

    def request(self, method, url, **kwargs):
        self.calls.append((method, url, kwargs))
        if isinstance(self.response, Exception): raise self.response
        return self.response


def client(session, network):
    return ToriiClient("https://staking.invalid", session=session,
        local_signing_context=LocalSigningContext(network), max_retries=3, retry_on_methods=["POST"])


@pytest.mark.parametrize("name", ["registration", "bond", "unbond", "claim"])
def test_native_preparation_frames_keep_exact_intent_global_xor_and_additive_reserves(name):
    request, prepared, request_bytes, response_bytes = fixture(name)
    assert encode(request) == request_bytes
    assert encode(prepared) == response_bytes
    assert validate(prepared, request, prepared.network_id, prepared.xor_asset_definition_id) is prepared
    assert prepared.observed_height == 200 and prepared.assumed_execution_height == 201
    assert len(prepared.balances) == 2
    for row in prepared.balances:
        assert row.asset.scope.dataspace is None
        assert row.asset.definition == "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        assert str(row.stake_reserved) == "1000" and str(row.rewards_reserved) == "22"


def test_preparation_frames_reject_bad_identity_layout_padding_crc_truncation_and_oversize():
    request, _, request_bytes, response_bytes = fixture()
    for end in range(len(response_bytes)):
        with pytest.raises((ValueError, TypeError, DecodeError)): decode(StakingPreparationV1, response_bytes[:end])
    for index in (4, 5, 6, 22, 23, 31, 39):
        changed = bytearray(response_bytes); changed[index] ^= 1
        with pytest.raises((ValueError, TypeError, DecodeError)): decode(StakingPreparationV1, bytes(changed))
    for bad in (request_bytes, response_bytes + b"\0", response_bytes[:40] + b"\0" + response_bytes[40:], bytes(256 * 1024 + 1)):
        with pytest.raises((ValueError, TypeError, DecodeError)): decode(StakingPreparationV1, bad)
    with pytest.raises(ValueError, match="positive"): replace(request, valid_for_blocks=0)


def test_preparation_binding_rejects_changed_request_network_xor_effects_and_balance_set():
    request, prepared, _, _ = fixture()
    def reject(candidate):
        with pytest.raises(ValueError): validate(candidate, request, prepared.network_id, prepared.xor_asset_definition_id)
    reject(replace(prepared, request=replace(request, lane_id=1)))
    other = bytearray(prepared.network_id.to_bytes()); other[0] ^= 1
    reject(replace(prepared, network_id=NetworkId.from_bytes(bytes(other))))
    reject(replace(prepared, xor_asset_definition_id="62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))
    reject(replace(prepared, observed_height=0))
    reject(replace(prepared, assumed_execution_height=202))
    reject(replace(prepared, balances=tuple(reversed(prepared.balances))))
    reject(replace(prepared, balances=prepared.balances[:1]))
    reject(replace(prepared, plan=replace(prepared.plan, amount=KotodamaQuantity("999"))))
    reject(replace(prepared, plan=replace(prepared.plan, valid_until_height=211)))
    reject(replace(prepared, plan=replace(prepared.plan, precondition=StakingMonetaryRegistrationV1(0))))
    scope = StakingAssetScopeV1(7)
    scoped_plan = replace(prepared.plan, source_asset=replace(prepared.plan.source_asset, scope=scope), destination_asset=replace(prepared.plan.destination_asset, scope=scope))
    reject(replace(prepared, plan=scoped_plan, balances=tuple(replace(row, asset=replace(row.asset, scope=scope)) for row in prepared.balances)))
    with pytest.raises((ValueError, TypeError)): validate(prepared, request, prepared.network_id, "fake-xor")


def test_reward_preparation_retains_selected_accruals_epoch_cut_and_recipient():
    request, prepared, _, _ = fixture("claim")
    plan = prepared.plan
    for changed in (
        replace(plan, records=(replace(plan.records[0], epoch=202),)),
        replace(plan, expected_state=replace(plan.expected_state, through_epoch=200), records=()),
        replace(plan, sources=()),
        replace(plan, sources=(replace(plan.sources[0], expected_accrued=None),)),
    ):
        # The cursor-only plan is valid when its only source was explicitly selected.
        if not changed.records and changed.sources:
            assert validate(replace(prepared, plan=changed), request, prepared.network_id, prepared.xor_asset_definition_id)
        else:
            with pytest.raises(ValueError): validate(replace(prepared, plan=changed), request, prepared.network_id, prepared.xor_asset_definition_id)
    with pytest.raises(ValueError): replace(request.operation, max_records=65)
    with pytest.raises(ValueError): replace(request.operation, accrued_sources=request.operation.accrued_sources * 2)


def test_transport_sends_one_unsigned_exact_canonical_request_and_closes_stream():
    request, prepared, request_bytes, response_bytes = fixture()
    response = Response(response_bytes); session = Session(response)
    actual = client(session, prepared.network_id).prepare_public_lane_plan(request, prepared.xor_asset_definition_id)
    assert actual.to_norito() == prepared.to_norito()
    assert len(session.calls) == 1 and response.closed
    method, url, args = session.calls[0]
    assert method == "POST" and url == "https://staking.invalid/v1/nexus/staking/prepare"
    assert args["data"] == request_bytes and args["stream"] is True and args["allow_redirects"] is False
    assert args["headers"]["Content-Type"] == "application/x-norito"
    assert not any(key.lower() == "x-iroha-signature" for key in args["headers"])


@pytest.mark.parametrize("kind", ["media", "declared_oversize", "stream_oversize", "error", "redirect", "length_mismatch"])
def test_transport_rejections_are_bounded_closed_and_never_replayed(kind):
    request, prepared, _, body = fixture()
    response = Response(body)
    if kind == "media": response.headers["Content-Type"] = "application/json"
    if kind == "declared_oversize": response.headers["Content-Length"] = str(256 * 1024 + 1)
    if kind == "stream_oversize": response.body = bytes(256 * 1024 + 1); response.status_code = 503
    if kind == "error": response.body = b"unavailable"; response.status_code = 503
    if kind == "redirect": response.status_code = 302
    if kind == "length_mismatch": response.headers["Content-Length"] = str(len(body) + 1)
    session = Session(response)
    with pytest.raises((ValueError, requests.HTTPError)):
        client(session, prepared.network_id).prepare_public_lane_plan(request, prepared.xor_asset_definition_id)
    assert len(session.calls) == 1 and response.closed
    if kind in ("media", "declared_oversize"): assert response.reads == 0


def test_transport_refuses_missing_pin_before_dispatch_and_does_not_retry_transport_error():
    request, prepared, _, body = fixture()
    session = Session(Response(body))
    with pytest.raises((ValueError, TypeError)):
        ToriiClient("https://staking.invalid", session=session).prepare_public_lane_plan(request, prepared.xor_asset_definition_id)
    assert session.calls == []
    session = Session(requests.ConnectionError("unavailable"))
    with pytest.raises(requests.ConnectionError): client(session, prepared.network_id).prepare_public_lane_plan(request, prepared.xor_asset_definition_id)
    assert len(session.calls) == 1
