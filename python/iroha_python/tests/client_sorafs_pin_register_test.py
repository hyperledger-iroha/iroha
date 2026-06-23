from __future__ import annotations

import base64
import copy
import json
from collections.abc import Callable
from typing import Any

import pytest

from iroha_python import SorafsPinAlias, SorafsPinRegisterResponse, ToriiClient

from .helpers import RecordingSession, StubResponse


def _pin_register_request() -> dict[str, Any]:
    return {
        "authority": "alice@boi",
        "privateKey": "ed25519:deadbeef",
        "chunker": {
            "profileId": 1,
            "namespace": "sorafs",
            "name": "sf1",
            "semver": "1.0.0",
            "multihashCode": 0,
        },
        "pinPolicy": {
            "minReplicas": 3,
            "storageClass": "hot",
            "retentionEpoch": 72,
        },
        "manifestDigestHex": "A" * 64,
        "manifestBytes": b"manifest-norito",
        "chunkDigestSha3_256Hex": "0x" + "b" * 64,
        "contentLength": 4096,
        "submittedEpoch": 42,
        "alias": {
            "namespace": "docs",
            "name": "main",
            "proof": b"alias-proof",
        },
        "successorOfHex": "C" * 64,
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
    assert body["chunker_profile_id"] == 1
    assert body["chunker_namespace"] == "sorafs"
    assert body["chunker_name"] == "sf1"
    assert body["chunker_semver"] == "1.0.0"
    assert body["chunker_multihash_code"] == 0
    assert body["pin_policy"] == {
        "min_replicas": 3,
        "storage_class": {"type": "Hot"},
        "retention_epoch": 72,
    }
    assert body["manifest_digest_hex"] == "a" * 64
    assert body["manifest_b64"] == base64.b64encode(b"manifest-norito").decode("ascii")
    assert body["chunk_digest_sha3_256_hex"] == "b" * 64
    assert body["content_length"] == 4096
    assert body["submitted_epoch"] == 42
    assert body["alias"] == {
        "namespace": "docs",
        "name": "main",
        "proof_base64": base64.b64encode(b"alias-proof").decode("ascii"),
    }
    assert body["successor_of_hex"] == "c" * 64


def test_register_sorafs_pin_manifest_accepts_snake_case_policy_alias_and_successor() -> None:
    successor_hex = "d" * 64
    request = copy.deepcopy(_pin_register_request())
    request.pop("pinPolicy")
    request.pop("alias")
    request.pop("successorOfHex")
    request["pin_policy"] = {
        "min_replicas": "3",
        "storage_class": {"type": "warm"},
        "retention_epoch": "72",
    }
    request["alias_namespace"] = "docs"
    request["alias_name"] = "main"
    request["alias_proof_base64"] = base64.b64encode(b"alias-proof").decode("ascii")
    request["successor_of_hex"] = successor_hex.upper()
    request["manifestBytes"] = None
    request["manifest_b64"] = base64.b64encode(b"explicit-manifest").decode("ascii")
    session = RecordingSession(StubResponse(200, {"status": "queued"}))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    client.register_sorafs_pin_manifest(request)

    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert body["pin_policy"]["storage_class"] == {"type": "Warm"}
    assert body["manifest_b64"] == base64.b64encode(b"explicit-manifest").decode("ascii")
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
    ("mutate", "match"),
    [
        (
            lambda request: request.update({"manifest_digest_hex": "a" * 64}),
            "ambiguous aliases: manifest_digest_hex, manifestDigestHex",
        ),
        (
            lambda request: request.update({"chunk_digest_sha3_256_hex": "b" * 64}),
            "ambiguous aliases: chunk_digest_sha3_256_hex, chunkDigestSha3_256Hex",
        ),
        (
            lambda request: request.update({"content_length": 4096}),
            "ambiguous aliases: content_length, contentLength",
        ),
        (
            lambda request: request.update({"submitted_epoch": 42}),
            "ambiguous aliases: submitted_epoch, submittedEpoch",
        ),
        (
            lambda request: request.update({"successor_of_hex": "d" * 64}),
            "ambiguous aliases: successor_of_hex, successorOfHex",
        ),
        (
            lambda request: request.update(
                {"manifest_b64": base64.b64encode(b"other").decode("ascii")}
            ),
            "accepts only one of manifest_b64 or manifest_bytes",
        ),
        (
            lambda request: request.update(
                {"pin_policy": {"min_replicas": 3, "storage_class": "hot"}}
            ),
            "ambiguous aliases: pin_policy, pinPolicy",
        ),
        (
            lambda request: request["chunker"].update({"profile_id": 1}),
            "ambiguous aliases: profile_id, profileId",
        ),
        (
            lambda request: request["pinPolicy"].update({"min_replicas": 3}),
            "ambiguous aliases: min_replicas, minReplicas",
        ),
        (
            lambda request: request["alias"].update(
                {"proofB64": base64.b64encode(b"alias-proof").decode("ascii")}
            ),
            "ambiguous aliases: proof, proofB64",
        ),
    ],
)
def test_register_sorafs_pin_manifest_rejects_duplicate_aliases_before_request(
    mutate: Callable[[dict[str, Any]], object],
    match: str,
) -> None:
    request = copy.deepcopy(_pin_register_request())
    mutate(request)
    session = RecordingSession(StubResponse(200, {"status": "queued"}))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(TypeError, match=match):
        client.register_sorafs_pin_manifest(request)

    assert session.calls == []


def test_register_sorafs_pin_manifest_rejects_alias_object_with_flat_alias_fields() -> None:
    request = copy.deepcopy(_pin_register_request())
    request["alias_namespace"] = "docs"
    request["alias_name"] = "main"
    request["alias_proof"] = b"alias-proof"
    session = RecordingSession(StubResponse(200, {"status": "queued"}))
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(TypeError, match="alias must not be combined with flat alias fields"):
        client.register_sorafs_pin_manifest(request)

    assert session.calls == []


@pytest.mark.parametrize(
    ("mutate", "match"),
    [
        (lambda request: request.update({"manifestDigestHex": "abc123"}), "manifest_digest_hex"),
        (lambda request: request.update({"manifestBytes": b""}), "manifest_bytes"),
        (lambda request: request.update({"manifestBytes": "not base64!"}), "manifest_bytes"),
        (
            lambda request: request.update({"chunkDigestSha3_256Hex": "z" * 64}),
            "chunk_digest_sha3_256_hex",
        ),
        (lambda request: request.update({"successorOfHex": "c" * 63}), "successor_of_hex"),
        (lambda request: request.update({"contentLength": -1}), "content_length"),
        (lambda request: request.update({"submittedEpoch": -1}), "submitted_epoch"),
        (lambda request: request["chunker"].update({"profileId": 0}), "chunker.profile_id"),
        (lambda request: request["chunker"].update({"namespace": " "}), "chunker.namespace"),
        (lambda request: request["chunker"].update({"multihashCode": True}), "multihash_code"),
        (lambda request: request["pinPolicy"].update({"minReplicas": 0}), "min_replicas"),
        (lambda request: request["pinPolicy"].update({"minReplicas": True}), "min_replicas"),
        (lambda request: request["pinPolicy"].update({"storageClass": "lava"}), "storage_class"),
        (lambda request: request["pinPolicy"].update({"retentionEpoch": -1}), "retention_epoch"),
        (lambda request: request["alias"].pop("proof"), "alias.proof"),
        (lambda request: request["alias"].update({"proof": "not base64!"}), "alias.proof"),
        (lambda request: request["alias"].update({"namespace": ""}), "alias.namespace"),
        (
            lambda request: request.update({"private_key": "ed25519:abc"}),
            "private_key or privateKey",
        ),
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
