"""Fail-closed Python SDK coverage for native Hijiri validation-fee quotes."""

from __future__ import annotations

import json
from dataclasses import FrozenInstanceError
from typing import Any

import pytest
import requests
from requests.structures import CaseInsensitiveDict

import iroha_python.client as client_module
import iroha_python.validation_fee_hijiri_quote as quote_module
from iroha_python import (
    AccountAddress,
    Ed25519KeyPair,
    NetworkId,
    ToriiCanonicalRequestAuth,
    ToriiClient,
    ValidationFeeHijiriQuoteV1,
    encode_validation_fee_hijiri_quote_request_v1,
    verify_validation_fee_hijiri_quote_response_v1,
)

NETWORK_ID = NetworkId.from_bytes(bytes([0xA5]) * 32)
ACCOUNT_ID = AccountAddress.from_account(
    public_key=Ed25519KeyPair.from_private_key(bytes([0x51]) * 32).public_key,
).to_i105(0x02F1)
REQUEST_NORITO = b"\x01\x02\x03"
RESPONSE_NORITO = b"\x04\x05\x06"


def _projection() -> dict[str, object]:
    return {
        "schema": "iroha.torii.v1.validation_fee.hijiri_quote.response",
        "version": 1,
        "assurance": "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED",
        "evaluatedStateHeight": "42",
        "quotedExecutionHeight": "43",
        "accountId": ACCOUNT_ID,
        "activePolicyVersion": "1",
        "activePolicyHash": "03" * 32,
        "feeAssetDefinitionId": "fee-asset",
        "treasuryAccountId": ACCOUNT_ID,
        "feeScale": 2,
        "hijiriParametersVersion": 1,
        "hijiriParametersRevision": "1",
        "hijiriParametersDigest": "05" * 32,
        "defaultAccountRiskQ16": 0,
        "effectiveAccountRiskQ16": 0,
        "accountRiskRevision": None,
        "accountRiskDigest": None,
        "feeMultiplierQ16": 81_920,
        "hijiriFeeQuoteHash": "07" * 32,
        "basePerTransferFeeMinorUnits": "10",
        "adjustedPerTransferFeeMinorUnits": "13",
        "qualifyingTransferCount": 2,
        "aggregateBaseFeeMinorUnits": "20",
        "aggregateAdjustedFeeMinorUnits": "25",
    }


class _NativeQuoteBridge:
    @staticmethod
    def connect_norito_bridge_abi_version() -> int:
        return 23

    @staticmethod
    def validation_fee_hijiri_quote_request_v1(account_id: str, count: int) -> bytes:
        assert account_id == ACCOUNT_ID
        assert count == 2
        return REQUEST_NORITO

    @staticmethod
    def validation_fee_verify_hijiri_quote_response_v1(
        response: bytes,
        request: bytes,
    ) -> str:
        assert response == RESPONSE_NORITO
        assert request == REQUEST_NORITO
        return json.dumps(_projection(), separators=(",", ":"))


class _ChunkedResponse(requests.Response):
    def __init__(
        self,
        body: bytes,
        headers: dict[str, str],
        *,
        status: int = 200,
        url: str = "https://torii.example/v1/validation-fee/hijiri/quote",
        redirected: bool = False,
    ) -> None:
        super().__init__()
        self.status_code = status
        self.headers = CaseInsensitiveDict(headers)
        self.url = url
        if redirected:
            self.history = [requests.Response()]
        self._body = body
        self._content = False
        self.closed = False

    def iter_content(self, chunk_size: int = 1, decode_unicode: bool = False):
        assert chunk_size == 8_192
        assert decode_unicode is False
        yield self._body

    def close(self) -> None:
        self.closed = True


class _Session(requests.Session):
    def __init__(self, response: requests.Response) -> None:
        super().__init__()
        self.response = response
        self.calls: list[dict[str, Any]] = []

    def send(self, request: requests.PreparedRequest, **kwargs: Any) -> requests.Response:
        self.calls.append({"request": request, **kwargs})
        return self.response


def _canonical_auth() -> ToriiCanonicalRequestAuth:
    return ToriiCanonicalRequestAuth(
        network_id=NETWORK_ID.literal,
        account_id=ACCOUNT_ID,
        signer=lambda _message: bytes([0x44]) * 64,
        timestamp_ms=4_102_444_801_000,
        nonce="hijiri-quote-test",
    )


def _success_response(**headers: str) -> _ChunkedResponse:
    return _ChunkedResponse(
        RESPONSE_NORITO,
        {
            "Cache-Control": "private, no-store",
            "Content-Length": str(len(RESPONSE_NORITO)),
            "Content-Type": "application/x-norito",
            **headers,
        },
    )


def test_quote_codec_uses_only_the_abi_23_native_bridge(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(quote_module, "load_crypto_extension", lambda: _NativeQuoteBridge)

    request = encode_validation_fee_hijiri_quote_request_v1(ACCOUNT_ID, 2)
    assert request == REQUEST_NORITO
    quote = verify_validation_fee_hijiri_quote_response_v1(
        RESPONSE_NORITO,
        REQUEST_NORITO,
    )
    assert isinstance(quote, ValidationFeeHijiriQuoteV1)
    assert quote.aggregate_adjusted_fee_minor_units == "25"
    with pytest.raises(FrozenInstanceError):
        quote.qualifying_transfer_count = 3  # type: ignore[misc]

    monkeypatch.setattr(
        quote_module,
        "load_crypto_extension",
        lambda: type(
            "IncompleteBridge",
            (),
            {"connect_norito_bridge_abi_version": staticmethod(lambda: 23)},
        ),
    )
    with pytest.raises(RuntimeError, match="lacks the ABI 23"):
        encode_validation_fee_hijiri_quote_request_v1(ACCOUNT_ID, 2)


def test_quote_codec_enforces_wire_and_transfer_bounds(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(quote_module, "load_crypto_extension", lambda: _NativeQuoteBridge)

    for count in (0, 100_001, -1, True, 1.5):
        with pytest.raises((TypeError, ValueError), match="qualifying_transfer_count"):
            encode_validation_fee_hijiri_quote_request_v1(ACCOUNT_ID, count)  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="response_norito"):
        verify_validation_fee_hijiri_quote_response_v1(
            bytes(quote_module.VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES + 1),
            REQUEST_NORITO,
        )
    with pytest.raises(ValueError, match="request_norito"):
        verify_validation_fee_hijiri_quote_response_v1(
            RESPONSE_NORITO,
            bytes(quote_module.VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES + 1),
        )

    class MaterializationGuard(bytearray):
        def __bytes__(self) -> bytes:
            pytest.fail("oversized caller buffer was materialized before its bound check")

        def __len__(self) -> int:
            return 1

    with pytest.raises(ValueError, match="response_norito"):
        verify_validation_fee_hijiri_quote_response_v1(
            MaterializationGuard(quote_module.VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES + 1),
            REQUEST_NORITO,
        )
    with pytest.raises(ValueError, match="request_norito"):
        verify_validation_fee_hijiri_quote_response_v1(
            MaterializationGuard(RESPONSE_NORITO),
            MaterializationGuard(quote_module.VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES + 1),
        )


@pytest.mark.parametrize(
    "updates, message",
    [
        ({"hijiriParametersVersion": 2}, "markers"),
        (
            {"accountRiskRevision": "02", "accountRiskDigest": "09" * 32},
            "accountRiskRevision",
        ),
        (
            {"accountRiskRevision": "2", "accountRiskDigest": "AA" * 32},
            "accountRiskDigest",
        ),
    ],
)
def test_quote_codec_closes_native_projection_shape(
    updates: dict[str, object],
    message: str,
) -> None:
    valid_risk_projection = {
        **_projection(),
        "accountRiskRevision": "2",
        "accountRiskDigest": "09" * 32,
    }
    assert (
        ValidationFeeHijiriQuoteV1.from_native_projection(
            valid_risk_projection
        ).account_risk_revision
        == "2"
    )
    invalid_projection = {**_projection(), **updates}
    with pytest.raises(ValueError, match=message):
        ValidationFeeHijiriQuoteV1.from_native_projection(invalid_projection)


def test_torii_quote_is_exact_authenticated_private_native_norito(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    verified = ValidationFeeHijiriQuoteV1.from_native_projection(_projection())
    monkeypatch.setattr(
        client_module,
        "encode_validation_fee_hijiri_quote_request_v1",
        lambda account_id, count: (
            REQUEST_NORITO
            if (account_id, count) == (ACCOUNT_ID, 2)
            else pytest.fail("wrong request inputs")
        ),
    )
    monkeypatch.setattr(
        client_module,
        "verify_validation_fee_hijiri_quote_response_v1",
        lambda response, request: (
            verified
            if (response, request) == (RESPONSE_NORITO, REQUEST_NORITO)
            else pytest.fail("wrong verification inputs")
        ),
    )
    response = _success_response()
    session = _Session(response)
    client = ToriiClient(
        "https://torii.example",
        session=session,
        default_headers={"Content-Encoding": "gzip"},
        max_retries=0,
    )

    assert (
        client.quote_validation_fee_hijiri(
            ACCOUNT_ID,
            2,
            canonical_auth=_canonical_auth(),
        )
        is verified
    )
    assert response.closed
    assert len(session.calls) == 1
    call = session.calls[0]
    request = call["request"]
    assert request.method == "POST"
    assert request.url == "https://torii.example/v1/validation-fee/hijiri/quote"
    assert request.path_url == "/v1/validation-fee/hijiri/quote"
    assert request.body == REQUEST_NORITO
    assert request.headers["Accept"] == "application/x-norito"
    assert request.headers["Accept-Encoding"] == "identity"
    assert request.headers["Cache-Control"] == "no-store"
    assert request.headers["Content-Encoding"] == "identity"
    assert request.headers["Content-Type"] == "application/x-norito"
    assert "X-Iroha-Signature" in request.headers
    assert call["stream"] is True
    assert call["allow_redirects"] is False


def test_torii_quote_treats_every_valid_i105_discriminator_as_presentation(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    presentation_account_id = AccountAddress.parse_encoded(ACCOUNT_ID).to_i105(4_242)
    projection = {
        **_projection(),
        "accountId": presentation_account_id,
    }
    verified = ValidationFeeHijiriQuoteV1.from_native_projection(projection)
    encoded_inputs: list[tuple[str, int]] = []

    def encode(account_id: str, count: int) -> bytes:
        encoded_inputs.append((account_id, count))
        return REQUEST_NORITO

    monkeypatch.setattr(
        client_module,
        "encode_validation_fee_hijiri_quote_request_v1",
        encode,
    )
    monkeypatch.setattr(
        client_module,
        "verify_validation_fee_hijiri_quote_response_v1",
        lambda response, request: (
            verified
            if (response, request) == (RESPONSE_NORITO, REQUEST_NORITO)
            else pytest.fail("wrong verification inputs")
        ),
    )
    response = _success_response()
    client = ToriiClient(
        "https://torii.example",
        session=_Session(response),
        max_retries=0,
    )

    assert (
        client.quote_validation_fee_hijiri(
            presentation_account_id,
            2,
            canonical_auth=_canonical_auth(),
        )
        is verified
    )
    assert encoded_inputs == [(presentation_account_id, 2)]
    assert response.closed


@pytest.mark.parametrize(
    "failure, expected_error, message",
    [
        (
            client_module.SorafsAliasError("rejected"),
            RuntimeError,
            "failed to validate",
        ),
        (ValueError("malformed proof"), ValueError, "malformed proof"),
    ],
)
def test_torii_quote_closes_stream_when_alias_policy_enforcement_fails(
    monkeypatch: pytest.MonkeyPatch,
    failure: Exception,
    expected_error: type[Exception],
    message: str,
) -> None:
    response = _success_response(**{"Sora-Proof": "invalid"})
    session = _Session(response)
    monkeypatch.setattr(
        client_module,
        "encode_validation_fee_hijiri_quote_request_v1",
        lambda _account_id, _count: REQUEST_NORITO,
    )

    def fail_alias_policy(*_args: object, **_kwargs: object) -> None:
        raise failure

    monkeypatch.setattr(client_module, "enforce_sorafs_alias_policy", fail_alias_policy)
    client = ToriiClient("https://torii.example", session=session, max_retries=0)

    with pytest.raises(expected_error, match=message):
        client.quote_validation_fee_hijiri(
            ACCOUNT_ID,
            2,
            canonical_auth=_canonical_auth(),
        )
    assert response.closed


@pytest.mark.parametrize(
    "headers, message",
    [
        ({"Cache-Control": "no-store"}, "private and no-store"),
        ({"Content-Type": "application/json"}, "application/x-norito"),
        (
            {"Content-Type": "application/x-norito; charset=binary"},
            "application/x-norito",
        ),
        ({"Content-Encoding": "gzip"}, "Content-Encoding"),
        ({"X-Iroha-Reject-Code": "rejected"}, "rejection code"),
        ({"X-Iroha-Reject-Code": ""}, "rejection code"),
        ({"Cache-Control": "private, no-store, public"}, "must not be public"),
        ({"Cache-Control": 'private="field", no-store'}, "private and no-store"),
        ({"Cache-Control": "private, no-store=foo"}, "private and no-store"),
        (
            {"Cache-Control": "private, no-store, public=max-age"},
            "must not be public",
        ),
        (
            {"Cache-Control": 'extension="x, private, no-store, y"'},
            "private and no-store",
        ),
        (
            {"Cache-Control": 'private, no-store, extension="unterminated'},
            "private and no-store",
        ),
        (
            {"Cache-Control": 'private, no-store, extension="dangling\\'},
            "private and no-store",
        ),
        ({"Content-Length": str(len(RESPONSE_NORITO) + 1)}, "Content-Length"),
    ],
)
def test_torii_quote_rejects_unsafe_success_metadata(
    monkeypatch: pytest.MonkeyPatch,
    headers: dict[str, str],
    message: str,
) -> None:
    response = _success_response(**headers)
    session = _Session(response)
    monkeypatch.setattr(
        client_module,
        "encode_validation_fee_hijiri_quote_request_v1",
        lambda _account_id, _count: REQUEST_NORITO,
    )
    monkeypatch.setattr(
        client_module,
        "verify_validation_fee_hijiri_quote_response_v1",
        lambda _response, _request: pytest.fail("unsafe response reached native verifier"),
    )
    client = ToriiClient("https://torii.example", session=session, max_retries=0)

    with pytest.raises(RuntimeError, match=message):
        client.quote_validation_fee_hijiri(
            ACCOUNT_ID,
            2,
            canonical_auth=_canonical_auth(),
        )
    assert response.closed


def test_torii_quote_accepts_valid_quoted_cache_extension(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    verified = ValidationFeeHijiriQuoteV1.from_native_projection(_projection())
    response = _success_response(
        **{"Cache-Control": 'private, extension="public, no-store", no-store'}
    )
    monkeypatch.setattr(
        client_module,
        "encode_validation_fee_hijiri_quote_request_v1",
        lambda _account_id, _count: REQUEST_NORITO,
    )
    monkeypatch.setattr(
        client_module,
        "verify_validation_fee_hijiri_quote_response_v1",
        lambda response_norito, request_norito: (
            verified
            if (response_norito, request_norito)
            == (RESPONSE_NORITO, REQUEST_NORITO)
            else pytest.fail("wrong verification inputs")
        ),
    )
    client = ToriiClient(
        "https://torii.example",
        session=_Session(response),
        max_retries=0,
    )

    assert (
        client.quote_validation_fee_hijiri(
            ACCOUNT_ID,
            2,
            canonical_auth=_canonical_auth(),
        )
        is verified
    )
    assert response.closed


@pytest.mark.parametrize(
    "response",
    [
        _ChunkedResponse(
            RESPONSE_NORITO,
            {
                "Cache-Control": "private, no-store",
                "Content-Type": "application/x-norito",
            },
            url="https://torii.example/redirected",
        ),
        _ChunkedResponse(
            RESPONSE_NORITO,
            {
                "Cache-Control": "private, no-store",
                "Content-Type": "application/x-norito",
            },
            redirected=True,
        ),
    ],
)
def test_torii_quote_rejects_redirected_final_target(
    monkeypatch: pytest.MonkeyPatch,
    response: _ChunkedResponse,
) -> None:
    session = _Session(response)
    monkeypatch.setattr(
        client_module,
        "encode_validation_fee_hijiri_quote_request_v1",
        lambda _account_id, _count: REQUEST_NORITO,
    )
    monkeypatch.setattr(
        client_module,
        "verify_validation_fee_hijiri_quote_response_v1",
        lambda _response, _request: pytest.fail("redirected response reached native verifier"),
    )
    client = ToriiClient("https://torii.example", session=session, max_retries=0)

    with pytest.raises(RuntimeError, match="exact signed URL without redirects"):
        client.quote_validation_fee_hijiri(
            ACCOUNT_ID,
            2,
            canonical_auth=_canonical_auth(),
        )
    assert response.closed


@pytest.mark.parametrize(
    "response, message",
    [
        (
            _ChunkedResponse(
                RESPONSE_NORITO,
                {
                    "Cache-Control": "private, no-store",
                    "Content-Type": "application/x-norito",
                },
                status=503,
            ),
            "unexpected status 503",
        ),
        (
            _ChunkedResponse(
                RESPONSE_NORITO,
                {"Content-Type": "application/x-norito"},
                status=503,
            ),
            "private and no-store",
        ),
        (
            _ChunkedResponse(
                bytes(quote_module.VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES + 1),
                {
                    "Cache-Control": "private, no-store",
                    "Content-Type": "application/x-norito",
                },
                status=503,
            ),
            "exceeds its 65536-byte size bound",
        ),
        (
            _ChunkedResponse(
                RESPONSE_NORITO,
                {
                    "Cache-Control": "private, no-store",
                    "Content-Length": str(len(RESPONSE_NORITO) + 1),
                    "Content-Type": "application/x-norito",
                },
                status=503,
            ),
            "Content-Length does not match",
        ),
    ],
)
def test_torii_quote_validates_error_response_before_status_failure(
    monkeypatch: pytest.MonkeyPatch,
    response: _ChunkedResponse,
    message: str,
) -> None:
    session = _Session(response)
    monkeypatch.setattr(
        client_module,
        "encode_validation_fee_hijiri_quote_request_v1",
        lambda _account_id, _count: REQUEST_NORITO,
    )
    monkeypatch.setattr(
        client_module,
        "verify_validation_fee_hijiri_quote_response_v1",
        lambda _response, _request: pytest.fail("non-200 response reached native verifier"),
    )
    client = ToriiClient("https://torii.example", session=session, max_retries=0)

    with pytest.raises((RuntimeError, ValueError), match=message):
        client.quote_validation_fee_hijiri(
            ACCOUNT_ID,
            2,
            canonical_auth=_canonical_auth(),
        )
    assert response.closed


def test_torii_quote_requires_https_before_native_encoding(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        client_module,
        "encode_validation_fee_hijiri_quote_request_v1",
        lambda _account_id, _count: pytest.fail("HTTP request reached native encoder"),
    )
    client = ToriiClient("http://torii.example", max_retries=0)
    with pytest.raises(RuntimeError, match="HTTPS"):
        client.quote_validation_fee_hijiri(
            ACCOUNT_ID,
            2,
            canonical_auth=_canonical_auth(),
        )
