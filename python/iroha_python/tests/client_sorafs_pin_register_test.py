from __future__ import annotations

import base64
import copy
import json
from collections.abc import Callable
from typing import Any

import pytest

from iroha_python import (
    SorafsPinAlias,
    SorafsPinRegisterResponse,
    ToriiClient,
    authority_fee_payment,
)

from .helpers import RecordingSession, StubResponse


def _pin_register_request() -> dict[str, Any]:
    return {
        "authority": "alice@boi",
        "private_key": "ed25519:deadbeef",
        "manifest_payload": b"manifest-norito",
        "submitted_epoch": 42,
        "fee_payment": authority_fee_payment(charge_limits=[]),
        "alias": {
            "namespace": "docs",
            "name": "main",
            "proof_base64": base64.b64encode(b"alias-proof").decode("ascii"),
        },
        "successor_of_hex": "C" * 64,
    }


def test_register_sorafs_pin_manifest_posts_validated_payload() -> None:
    manifest_hex = "a" * 64
    session = RecordingSession(
        StubResponse(200, {"status": "queued", "manifest_digest_hex": manifest_hex})
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    result = client.register_sorafs_pin_manifest(_pin_register_request())

    assert result == {"status": "queued", "manifest_digest_hex": manifest_hex}
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "POST"
    assert str(call["url"]).endswith("/v1/sorafs/pin/register")
    assert call["headers"]["Content-Type"] == "application/json"
    assert call["headers"]["Accept"] == "application/json"
    body = json.loads(call["data"].decode("utf-8"))
    assert body["authority"] == "alice@boi"
    assert body["private_key"] == "ed25519:deadbeef"
    assert body["manifest_payload"] == base64.b64encode(b"manifest-norito").decode("ascii")
    assert body["submitted_epoch"] == 42
    assert body["fee_payment"] == authority_fee_payment(charge_limits=[])
    assert body["alias"] == {
        "namespace": "docs",
        "name": "main",
        "proof_base64": base64.b64encode(b"alias-proof").decode("ascii"),
    }
    assert body["successor_of_hex"] == "c" * 64


def test_register_sorafs_pin_manifest_accepts_canonical_payload_and_alias() -> None:
    successor_hex = "d" * 64
    request = copy.deepcopy(_pin_register_request())
    request.pop("successor_of_hex")
    request["successor_of_hex"] = successor_hex.upper()
    request["manifest_payload"] = base64.b64encode(b"explicit-manifest").decode("ascii")
    session = RecordingSession(StubResponse(200, {"status": "queued"}))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    client.register_sorafs_pin_manifest(request)

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert body["manifest_payload"] == base64.b64encode(b"explicit-manifest").decode("ascii")
    assert body["alias"]["proof_base64"] == base64.b64encode(b"alias-proof").decode("ascii")
    assert body["successor_of_hex"] == successor_hex


def test_register_sorafs_pin_manifest_typed_normalizes_response() -> None:
    manifest_hex = "d" * 64
    successor_hex = "e" * 64
    alias_b64 = base64.b64encode(b"alias-proof").decode("ascii")
    session = RecordingSession(
        StubResponse(
            200,
            {
                "manifestDigestHex": manifest_hex.upper(),
                "chunkerHandle": "sorafs.sf1@1.0.0",
                "submittedEpoch": "42",
                "contentLength": "4096",
                "pinFeeNano": "500000000",
                "pinFeeAssetId": "xor#universal",
                "pinFeeTreasuryAccountId": "treasury@boi",
                "alias": {
                    "namespace": "docs",
                    "name": "main",
                    "proof_base64": alias_b64,
                },
                "successorOfHex": "0x" + successor_hex.upper(),
            },
        )
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    result = client.register_sorafs_pin_manifest_typed(_pin_register_request())

    assert result == SorafsPinRegisterResponse(
        manifest_digest_hex=manifest_hex,
        chunker_handle="sorafs.sf1@1.0.0",
        submitted_epoch=42,
        content_length=4096,
        pin_fee_nano=500000000,
        pin_fee_asset_id="xor#universal",
        pin_fee_treasury_account_id="treasury@boi",
        alias=SorafsPinAlias(
            namespace="docs",
            name="main",
            proof_base64=alias_b64,
        ),
        successor_of_hex=successor_hex,
    )


@pytest.mark.parametrize(
    "retired_field",
    [
        "account",
        "privateKey",
        "private_key_bytes",
        "manifestBytes",
        "manifest_bytes",
        "manifestPayload",
        "submittedEpoch",
        "gas_asset_id",
        "gasAssetId",
        "successorOfHex",
        "alias_namespace",
        "alias_name",
        "alias_proof_base64",
        "chunker",
        "pinPolicy",
        "manifestDigestHex",
        "manifest_b64",
        "chunkDigestSha3_256Hex",
        "contentLength",
        "unexpected",
    ],
)
def test_register_sorafs_pin_manifest_rejects_retired_and_unknown_fields(
    retired_field: str,
) -> None:
    request = copy.deepcopy(_pin_register_request())
    request[retired_field] = "retired"
    session = RecordingSession(StubResponse(200, {"status": "queued"}))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(TypeError, match=f"unsupported fields: {retired_field}"):
        client.register_sorafs_pin_manifest(request)

    assert session.calls == []


def test_register_sorafs_pin_manifest_rejects_alias_object_with_flat_alias_fields() -> None:
    request = copy.deepcopy(_pin_register_request())
    request["alias_namespace"] = "docs"
    request["alias_name"] = "main"
    request["alias_proof"] = b"alias-proof"
    session = RecordingSession(StubResponse(200, {"status": "queued"}))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(TypeError, match="unsupported fields: alias_name, alias_namespace, alias_proof"):
        client.register_sorafs_pin_manifest(request)

    assert session.calls == []


@pytest.mark.parametrize(
    ("mutate", "match"),
    [
        (lambda request: request.update({"manifest_payload": b""}), "manifest_payload"),
        (lambda request: request.update({"manifest_payload": "not base64!"}), "manifest_payload"),
        (
            lambda request: request.update({"manifest_payload": b"x" * (512 * 1024 + 1)}),
            "at most 524288 bytes",
        ),
        (lambda request: request.update({"successor_of_hex": "c" * 63}), "successor_of_hex"),
        (lambda request: request.update({"successor_of_hex": "0" * 64}), "must not be zero"),
        (lambda request: request.update({"submitted_epoch": -1}), "submitted_epoch"),
        (lambda request: request.update({"submitted_epoch": True}), "submitted_epoch"),
        (lambda request: request.update({"submitted_epoch": "42"}), "submitted_epoch"),
        (lambda request: request.update({"authority": " alice@boi"}), "authority"),
        (lambda request: request.update({"private_key": "ed25519:deadbeef "}), "private_key"),
        (lambda request: request.pop("fee_payment"), "fee_payment"),
        (lambda request: request.update({"fee_payment": {}}), "fee_payment"),
        (lambda request: request["alias"].pop("proof_base64"), "alias.proof_base64"),
        (
            lambda request: request["alias"].update({"proof_base64": "not base64!"}),
            "alias.proof_base64",
        ),
        (
            lambda request: request["alias"].update(
                {
                    "proof_base64": base64.b64encode(b"x" * (1024 * 1024 + 1)).decode(
                        "ascii"
                    )
                }
            ),
            "at most 1048576 bytes",
        ),
        (lambda request: request["alias"].update({"namespace": ""}), "alias.namespace"),
        (lambda request: request["alias"].update({"namespace": " docs"}), "alias.namespace"),
        (lambda request: request["alias"].update({"namespace": "Docs"}), "lowercase ASCII"),
        (lambda request: request["alias"].update({"name": "main site"}), "lowercase ASCII"),
        (lambda request: request["alias"].update({"name": "máin"}), "lowercase ASCII"),
        (lambda request: request["alias"].update({"name": "a" * 129}), "1..128"),
        (lambda request: request["alias"].update({"name": "docs\u0000"}), "lowercase ASCII"),
        (lambda request: request["alias"].update({"extra": True}), "unsupported fields: extra"),
        (lambda request: request.update({"private_key": ""}), "private_key"),
    ],
)
def test_register_sorafs_pin_manifest_rejects_invalid_inputs_before_request(
    mutate: Callable[[dict[str, Any]], object],
    match: str,
) -> None:
    request = copy.deepcopy(_pin_register_request())
    mutate(request)
    session = RecordingSession(StubResponse(200, {"status": "queued"}))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises((TypeError, ValueError), match=match):
        client.register_sorafs_pin_manifest(request)

    assert session.calls == []


def test_register_sorafs_pin_manifest_rejects_empty_response_payload() -> None:
    session = RecordingSession(StubResponse(200, None))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(RuntimeError, match="no payload"):
        client.register_sorafs_pin_manifest(_pin_register_request())


@pytest.mark.parametrize(
    ("patch", "match"),
    [
        ({"pinFeeNano": "-1"}, "pin_fee_nano"),
        ({"pinFeeAssetId": ""}, "pin_fee_asset_id"),
        ({"manifestDigestHex": "f" * 63}, "manifest_digest_hex"),
    ],
)
def test_register_sorafs_pin_manifest_typed_rejects_bad_response(
    patch: dict[str, Any],
    match: str,
) -> None:
    payload = {
        "manifestDigestHex": "f" * 64,
        "chunkerHandle": "sorafs.sf1@1.0.0",
        "submittedEpoch": 42,
        "contentLength": 4096,
        "pinFeeNano": 500000000,
        "pinFeeAssetId": "xor#universal",
        "pinFeeTreasuryAccountId": "treasury@boi",
    }
    payload.update(patch)
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises((TypeError, ValueError), match=match):
        client.register_sorafs_pin_manifest_typed(_pin_register_request())


def test_register_sorafs_pin_manifest_typed_rejects_duplicate_response_aliases() -> None:
    manifest_hex = "f" * 64
    payload = {
        "manifest_digest_hex": manifest_hex,
        "manifestDigestHex": manifest_hex,
        "chunkerHandle": "sorafs.sf1@1.0.0",
        "submittedEpoch": 42,
        "contentLength": 4096,
        "pinFeeNano": 500000000,
        "pinFeeAssetId": "xor#universal",
        "pinFeeTreasuryAccountId": "treasury@boi",
    }
    session = RecordingSession(StubResponse(200, payload))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(TypeError, match="ambiguous aliases: manifest_digest_hex, manifestDigestHex"):
        client.register_sorafs_pin_manifest_typed(_pin_register_request())
