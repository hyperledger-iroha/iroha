"""Adversarial tests for the closed first-release SCCP Python surface."""

from __future__ import annotations

import base64
import copy
import hashlib
import inspect
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Mapping, Optional, Union

import pytest
import requests
from requests.structures import CaseInsensitiveDict

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

import iroha_torii_client as package  # noqa: E402
import iroha_torii_client.sccp as sccp  # noqa: E402
from iroha_torii_client import (  # noqa: E402
    SCCP_CODEC_CANONICAL_TEXT,
    SCCP_CODEC_EVM_ADDRESS20,
    SCCP_CODEC_KEYS,
    SCCP_CODEC_TRON_ADDRESS21,
    SCCP_NETWORK_PROFILES,
    SCCP_PAYLOAD_KINDS,
    ToriiClient,
    normalize_bridge_message_submit_payload,
    normalize_bridge_proof_submit_payload,
    normalize_sccp_bridge_submit_response,
    normalize_sccp_capabilities,
    normalize_sccp_codec_value,
    normalize_sccp_message_bundle,
    normalize_sccp_proof_request,
    normalize_sccp_recent_messages,
    normalize_sccp_registry,
    parse_sccp_bridge_submit_response_json,
    parse_sccp_json_object,
    sccp_source_event_digest,
)
from iroha_torii_client.mock import ToriiMockServer  # noqa: E402

HASH = lambda byte: f"{byte:02x}" * 32
PREFIX_HASH = lambda byte: "0x" + HASH(byte)
UPPER = lambda byte, length: f"{byte:02x}".upper() * length
AUTHORITY = "sorauﾛ1Nヱﾐﾚﾗﾗﾁ9SHyｾｼF2ﾚbヱAｦiﾇｺﾂpﾆWyｿﾛWﾍ7ｾA7ﾋヰｿUJEKNX"
MESSAGE_ID = HASH(0x11)


def _b64(value: bytes) -> str:
    return base64.b64encode(value).decode("ascii")


def _network(profile: str) -> Dict[str, Any]:
    return {"network": profile.replace("-", "_"), "profile": None}


def _lane(source: str = "bsc-mainnet") -> Dict[str, Any]:
    return {"source": _network(source), "target": _network("sora-taira")}


def _g1(x: int = 1, y: int = 2) -> Dict[str, str]:
    return {"x": UPPER(x, 32), "y": UPPER(y, 32)}


def _g2(seed: int = 3) -> Dict[str, str]:
    return {
        "x_c0": UPPER(seed, 32),
        "x_c1": UPPER(seed + 1, 32),
        "y_c0": UPPER(seed + 2, 32),
        "y_c1": UPPER(seed + 3, 32),
    }


def _verifying_key() -> Dict[str, Any]:
    ic = {"constant": _g1()}
    ic.update({f"signal_{index}": _g1() for index in range(10)})
    return {
        "version": 1,
        "alpha1": _g1(),
        "beta2": _g2(),
        "gamma2": _g2(),
        "delta2": _g2(),
        "ic": ic,
    }


def _key_bytes(key: Mapping[str, Any]) -> bytes:
    words: List[str] = []

    def add_g1(point: Mapping[str, str]) -> None:
        words.extend((point["x"], point["y"]))

    def add_g2(point: Mapping[str, str]) -> None:
        words.extend((point["x_c0"], point["x_c1"], point["y_c0"], point["y_c1"]))

    add_g1(key["alpha1"])
    add_g2(key["beta2"])
    add_g2(key["gamma2"])
    add_g2(key["delta2"])
    add_g1(key["ic"]["constant"])
    for index in range(10):
        add_g1(key["ic"][f"signal_{index}"])
    return bytes.fromhex("".join(words))


def _key_hash(key: Mapping[str, Any]) -> str:
    return sccp._keccak_256(_key_bytes(key)).hex()  # noqa: SLF001 - parity oracle


def _capabilities() -> Dict[str, Any]:
    return {
        "version": 1,
        "registry_revision": PREFIX_HASH(0x10),
        "registry_path": "/v1/sccp/registry",
        "message_bundle_path": "/v1/sccp/proofs/message/{message_id}",
        "proof_request_path": "/v1/sccp/proof-requests/{message_id}",
        "recent_messages_path": "/v1/sccp/messages/recent",
        "proof_submit_path": "/v1/bridge/proofs/submit",
        "native_message_submit_path": "/v1/bridge/messages",
    }


def _route(*, revision: int = 1, activation: str = "staged") -> Dict[str, Any]:
    key = _verifying_key()
    route_address = UPPER(0x31, 20)
    route_code_hash = UPPER(0x41, 32)
    return {
        "lane_id": _lane(),
        "route_id": "taira_bsc_xor",
        "asset_key": "xor",
        "revision": revision,
        "activation": {"activation": activation, "direction": None},
        "source_identity": {
            "lane": _lane(),
            "emitter": {
                "emitter": "evm",
                "identity": {
                    "address": route_address,
                    "runtime_code_hash": route_code_hash,
                    "route_config_hash": UPPER(0x42, 32),
                },
            },
        },
        "destination": {
            "family": "evm",
            "deployment": {
                "token_address": UPPER(0x11, 20),
                "token_code_hash": UPPER(0x21, 32),
                "verifier_address": UPPER(0x12, 20),
                "verifier_code_hash": UPPER(0x22, 32),
                "verifying_key": key,
                "verifier_key_hash": _key_hash(key).upper(),
                "route_address": route_address,
                "route_code_hash": route_code_hash,
                "taira_to_token_multiplier": 1_000_000_000,
            },
        },
        "settlement": {
            "asset_definition_id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
            "custody_account_id": AUTHORITY,
            "payload_amount_scale": 9,
        },
    }


def _registry(routes: Optional[List[Dict[str, Any]]] = None) -> Dict[str, Any]:
    return {
        "version": 1,
        "lanes": [
            {"lane_id": _lane(), "native_trust_anchor": None, "routes": routes or [_route()]}
        ],
    }


def _bundle() -> Dict[str, Any]:
    return {
        "version": 1,
        "commitment_root": PREFIX_HASH(0x51),
        "commitment": {"version": 1},
        "merkle_proof": {"steps": []},
        "payload": {"Transfer": {"amount": "1"}},
        "finality_proof": "0x0102",
    }


def _proof_request() -> Dict[str, Any]:
    key = _verifying_key()
    return {
        "version": 1,
        "backend": {"backend": "evm_groth16_bn254_v1", "family": None},
        "source_network": _network("sora-taira"),
        "target_network": _network("bsc-mainnet"),
        "public_inputs": {
            "version": 1,
            "message_id": PREFIX_HASH(0x11),
            "payload_hash": PREFIX_HASH(0x12),
            "target_domain": 2,
            "commitment_root": PREFIX_HASH(0x13),
            "finality_height": "9",
            "finality_block_hash": PREFIX_HASH(0x14),
        },
        "verifying_key": key,
        "verifier_key_hash": "0x" + _key_hash(key),
        "bundle_bytes": "0x0102",
        "statement_hash": PREFIX_HASH(0x61),
        "destination_binding_hash": PREFIX_HASH(0x62),
        "route_configuration_hash": PREFIX_HASH(0x63),
        "request_hash": PREFIX_HASH(0x64),
    }


def _recent(height: int = 9, message_id: str = MESSAGE_ID) -> Dict[str, Any]:
    return {
        "height": height,
        "message_id_hex": message_id,
        "kind": "transfer",
        "source_profile": "sora-taira",
        "target_profile": "bsc-mainnet",
        "destination_binding_hash": PREFIX_HASH(0x71),
        "route_configuration_hash": PREFIX_HASH(0x72),
        "target_domain": 2,
        "asset_id": "xor",
        "route_id": "taira_bsc_xor",
        "recipient": None,
        "amount": "1000",
        "payload_projection": None,
        "links": {
            "bundle_path": f"/v1/sccp/proofs/message/{message_id}",
            "proof_request_path": f"/v1/sccp/proof-requests/{message_id}",
        },
    }


def _prehash(value: bytes) -> bytes:
    digest = bytearray(hashlib.blake2b(value, digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


def _prepared_response(**overrides: Any) -> Dict[str, Any]:
    transaction = b"\x01\x02\x03\x04"
    response: Dict[str, Any] = {
        "submitted": False,
        "payload_kind": "transfer",
        "message_id_hex": MESSAGE_ID,
        "backend": "bridge/sccp/native/bsc-parlia-v1",
        "counterparty_domain": 2,
        "counterparty_chain": "bsc-mainnet",
        "manifest_hash_hex": HASH(0x31),
        "range_start_height": 7,
        "range_end_height": 9,
        "creation_time_ms": 10,
        "tx_hash_hex": None,
        "transaction_payload_b64": _b64(transaction),
        "signing_message_b64": _b64(_prehash(transaction)),
    }
    response.update(overrides)
    return response


class StubResponse(requests.Response):
    """Minimal requests response preserving raw JSON and binary bodies."""

    def __init__(
        self,
        payload: Optional[Any] = None,
        *,
        status_code: int = 200,
        content_type: str = "application/json",
        raw: Optional[bytes] = None,
    ) -> None:
        super().__init__()
        self.status_code = status_code
        self.headers = CaseInsensitiveDict({"Content-Type": content_type})
        self._content = raw if raw is not None else json.dumps(payload).encode("utf-8")
        self.encoding = "utf-8"


class RecordingSession(requests.Session):
    """Queue-backed HTTP session for exact request assertions."""

    def __init__(self, responses: List[StubResponse]) -> None:
        super().__init__()
        self.responses = list(responses)
        self.calls: List[Mapping[str, Any]] = []

    def request(
        self,
        method: Union[str, bytes],
        url: Union[str, bytes],
        *args: Any,
        **kwargs: Any,
    ) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        return self.responses.pop(0)


def test_closed_inventory_removes_solana_ton_and_nontransfer_payloads() -> None:
    assert tuple(SCCP_NETWORK_PROFILES) == (
        "sora-nexus",
        "sora-taira",
        "ethereum-mainnet",
        "ethereum-sepolia",
        "bsc-mainnet",
        "bsc-testnet",
        "tron-mainnet",
        "tron-nile",
        "tron-shasta",
    )
    assert tuple(SCCP_CODEC_KEYS) == (1, 2, 5)
    assert SCCP_PAYLOAD_KINDS == ("transfer",)
    for retired in (
        "SCCP_DOMAIN_SOL",
        "SCCP_DOMAIN_TON",
        "SCCP_CODEC_SOLANA_PUBKEY32",
        "SCCP_CODEC_TON_ACCOUNT36",
        "SCCP_CODEC_SORA_ASSET_ID",
        "normalize_sccp_proof_manifests",
        "normalize_sccp_source_adapter_engine_deployment",
    ):
        assert not hasattr(sccp, retired)
        assert retired not in sccp.__all__
        assert retired not in package.__all__


def test_closed_codecs_accept_exact_bytes_and_reject_retired_or_textual_aliases() -> None:
    assert normalize_sccp_codec_value(SCCP_CODEC_CANONICAL_TEXT, "merchant@taira") == b"merchant@taira"
    assert normalize_sccp_codec_value(SCCP_CODEC_EVM_ADDRESS20, b"\x01" * 20) == b"\x01" * 20
    assert normalize_sccp_codec_value(SCCP_CODEC_TRON_ADDRESS21, b"\x41" + b"\x02" * 20)
    for codec, value in (
        (3, b"\x01" * 32),
        (4, b"\x01" * 36),
        (6, b"\x01"),
        (2, "0x" + "11" * 20),
        (2, b"\x00" * 20),
        (5, b"\x42" + b"\x01" * 20),
        (1, " padded"),
    ):
        with pytest.raises((TypeError, ValueError)):
            normalize_sccp_codec_value(codec, value)


def test_source_event_digest_matches_all_shared_vectors_and_rejects_aliases() -> None:
    fixture = json.loads(
        (Path(__file__).resolve().parents[3] / "fixtures/sccp/native_transfer_event_v1.json").read_text()
    )
    for vector in fixture["vectors"]:
        assert sccp_source_event_digest(
            vector["lane_hash_hex"], vector["message_id_hex"], vector["payload_hash_hex"]
        ) == vector["source_event_digest_hex"]
    for roles in (
        ("00" * 32, HASH(2), HASH(3)),
        (HASH(1), HASH(1), HASH(3)),
        ("0x" + HASH(1), HASH(2), HASH(3)),
        ("AB" * 32, HASH(2), HASH(3)),
    ):
        with pytest.raises(ValueError):
            sccp_source_event_digest(*roles)


def test_capabilities_require_exact_paths_and_reject_retired_fields_and_queries() -> None:
    assert normalize_sccp_capabilities(_capabilities()).registry_path == "/v1/sccp/registry"
    mutations = (
        lambda value: value.update(registry_path="/v1/sccp/manifests"),
        lambda value: value.update(proof_request_path="/v1/sccp/proof-requests/{message_id}?x=1"),
        lambda value: value.update(proof_artifact_path="/v1/sccp/artifacts/message/{message_id}"),
        lambda value: value.update(proof_job_path="/v1/sccp/jobs/message/{message_id}"),
        lambda value: value.update(allow_unready=True),
        lambda value: value.update(registry_revision=PREFIX_HASH(0)),
    )
    for mutate in mutations:
        value = _capabilities()
        mutate(value)
        with pytest.raises((TypeError, ValueError)):
            normalize_sccp_capabilities(value)


def test_registry_validates_full_key_and_rejects_retired_or_aliased_routes() -> None:
    assert len(normalize_sccp_registry(_registry()).lanes) == 1
    wrong_key = _registry()
    wrong_key["lanes"][0]["routes"][0]["destination"]["deployment"]["verifier_key_hash"] = UPPER(0x99, 32)
    with pytest.raises(ValueError, match="verifier_key_hash"):
        normalize_sccp_registry(wrong_key)
    retired = _registry()
    retired["lanes"][0]["lane_id"]["source"] = {"network": "ton_mainnet", "profile": None}
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_registry(retired)
    browser = _registry()
    browser["lanes"][0]["routes"][0]["destination_browser_prover"] = {}
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_registry(browser)
    alias = _registry()
    deployment = alias["lanes"][0]["routes"][0]["destination"]["deployment"]
    deployment["verifier_address"] = deployment["token_address"]
    with pytest.raises(ValueError, match="reuses"):
        normalize_sccp_registry(alias)


def test_registry_rejects_duplicate_lanes_revision_gaps_and_two_live_revisions() -> None:
    duplicate = _registry()
    duplicate["lanes"].append(copy.deepcopy(duplicate["lanes"][0]))
    with pytest.raises(ValueError, match="duplicate lane"):
        normalize_sccp_registry(duplicate)
    with pytest.raises(ValueError, match="start at one"):
        normalize_sccp_registry(_registry([_route(revision=2)]))
    with pytest.raises(ValueError, match="multiple revisions"):
        normalize_sccp_registry(
            _registry(
                [
                    _route(revision=1, activation="bidirectional"),
                    _route(revision=2, activation="bidirectional"),
                ]
            )
        )


def test_recent_links_are_exact_and_route_configuration_is_independent() -> None:
    parsed = normalize_sccp_recent_messages({"items": [_recent(9), _recent(8, HASH(0x12))]})
    assert [item["height"] for item in parsed.items] == [9, 8]
    retired = _recent()
    retired["links"]["job_path"] = f"/v1/sccp/jobs/message/{MESSAGE_ID}"
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_recent_messages({"items": [retired]})
    mismatch = _recent()
    mismatch["links"]["proof_request_path"] = f"/v1/sccp/proof-requests/{HASH(0x12)}"
    with pytest.raises(ValueError, match="exact message"):
        normalize_sccp_recent_messages({"items": [mismatch]})
    alias = _recent()
    alias["route_configuration_hash"] = alias["destination_binding_hash"]
    with pytest.raises(ValueError, match="distinct"):
        normalize_sccp_recent_messages({"items": [alias]})
    with pytest.raises(ValueError, match="newest-first"):
        normalize_sccp_recent_messages({"items": [_recent(8), _recent(9)]})


def test_bundle_and_proof_request_are_closed_and_query_free() -> None:
    assert normalize_sccp_message_bundle(_bundle())["version"] == 1
    assert normalize_sccp_proof_request(_proof_request())["public_inputs"]["target_domain"] == 2
    burn = _bundle()
    burn["payload"] = {"Burn": {}}
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_message_bundle(burn)
    retired = _proof_request()
    retired["backend"]["backend"] = "solana_recursive_v1"
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_proof_request(retired)
    alias = _proof_request()
    alias["route_configuration_hash"] = alias["destination_binding_hash"]
    with pytest.raises(ValueError, match="role-separated"):
        normalize_sccp_proof_request(alias)
    selector = _proof_request()
    selector["allow_unready"] = True
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_proof_request(selector)


def test_submit_dtos_have_no_redundant_public_key_or_caller_selected_route() -> None:
    proof = normalize_bridge_proof_submit_payload(
        {
            "authority": AUTHORITY,
            "signature_b64": "AQ==",
            "destination_proof_b64": "Ag==",
            "creation_time_ms": 10,
        }
    )
    assert list(proof) == ["authority", "destination_proof_b64", "signature_b64", "creation_time_ms"]
    assert list(
        normalize_bridge_message_submit_payload(
            {"authority": AUTHORITY, "native_proof_b64": "Aw=="}
        )
    ) == ["authority", "native_proof_b64"]
    parameters = inspect.signature(ToriiClient.submit_bridge_proof).parameters
    assert "public_key_hex" not in parameters
    assert "message_bundle_b64" not in parameters
    assert "destination_proof_b64" in parameters


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("public_key_hex", HASH(1)),
        ("message_bundle_b64", "AQ=="),
        ("proof_bytes_hex", "01"),
        ("network_id_hex", HASH(2)),
        ("manifest_hash", HASH(3)),
        ("deployment", {}),
        ("allow_unready", True),
        ("signature", "AQ=="),
    ],
)
def test_proof_submit_rejects_retired_fields(field: str, value: Any) -> None:
    with pytest.raises(ValueError, match="retired"):
        normalize_bridge_proof_submit_payload(
            {"authority": AUTHORITY, "destination_proof_b64": "AQ==", field: value}
        )


@pytest.mark.parametrize("artifact", ["AQ", " AQ==", "AQ==\n", "", "====", "A==="])
def test_proof_submit_rejects_noncanonical_base64(artifact: str) -> None:
    with pytest.raises(ValueError, match="base64"):
        normalize_bridge_proof_submit_payload(
            {"authority": AUTHORITY, "destination_proof_b64": artifact}
        )


@pytest.mark.parametrize("timestamp", [0, -1, 1.5, True, "1"])
def test_proof_submit_rejects_nonpositive_or_ambiguous_time(timestamp: Any) -> None:
    with pytest.raises(ValueError, match="integer"):
        normalize_bridge_proof_submit_payload(
            {
                "authority": AUTHORITY,
                "destination_proof_b64": "AQ==",
                "creation_time_ms": timestamp,
            }
        )


def test_bridge_response_and_strict_json_reject_contradictions_and_duplicates() -> None:
    assert normalize_sccp_bridge_submit_response(_prepared_response()).submitted is False
    submitted = _prepared_response(
        submitted=True,
        tx_hash_hex=HASH(0x55),
        transaction_payload_b64=None,
        signing_message_b64=None,
    )
    assert normalize_sccp_bridge_submit_response(submitted).submitted is True
    for value in (
        _prepared_response(payload_kind="burn"),
        _prepared_response(counterparty_chain="solana-mainnet-beta"),
        _prepared_response(proof_artifact_hash=HASH(3)),
        _prepared_response(creation_time_ms=0),
        _prepared_response(tx_hash_hex=HASH(4)),
        _prepared_response(signing_message_b64=_b64(b"\x09" * 32)),
    ):
        with pytest.raises((TypeError, ValueError)):
            normalize_sccp_bridge_submit_response(value)
    canonical = json.dumps(_prepared_response())
    assert parse_sccp_bridge_submit_response_json(canonical).submitted is False
    with pytest.raises(ValueError, match="duplicate"):
        parse_sccp_bridge_submit_response_json(canonical.replace("{", '{"submitted":false,', 1))
    with pytest.raises(ValueError):
        parse_sccp_json_object(canonical + "{}")


def test_torii_exact_endpoints_and_content_negotiation() -> None:
    session = RecordingSession(
        [
            StubResponse(_capabilities()),
            StubResponse({"version": 1, "lanes": []}),
            StubResponse(_bundle()),
            StubResponse(raw=b"\x07\x08", content_type="application/x-norito"),
            StubResponse({"items": []}),
        ]
    )
    client = ToriiClient("https://example.invalid", session=session)
    assert client.get_sccp_capabilities().version == 1
    assert client.get_sccp_registry().version == 1
    assert client.get_sccp_message_bundle(MESSAGE_ID)["version"] == 1
    assert client.get_sccp_proof_request(MESSAGE_ID, format="norito") == b"\x07\x08"
    assert client.get_sccp_recent_messages(from_height=9, limit=0).items == ()
    assert [call["url"] for call in session.calls] == [
        "https://example.invalid/v1/sccp/capabilities",
        "https://example.invalid/v1/sccp/registry",
        f"https://example.invalid/v1/sccp/proofs/message/{MESSAGE_ID}",
        f"https://example.invalid/v1/sccp/proof-requests/{MESSAGE_ID}",
        "https://example.invalid/v1/sccp/messages/recent",
    ]
    assert session.calls[3]["headers"] == {"Accept": "application/x-norito"}
    assert session.calls[4]["params"] == {"from": "9", "limit": "0"}


@pytest.mark.parametrize(
    "message_id",
    [
        "0x" + MESSAGE_ID,
        "AB" * 32,
        MESSAGE_ID + "?network=bsc",
        MESSAGE_ID + "/../registry",
        "00" * 32,
    ],
)
def test_torii_rejects_message_id_path_injection_without_io(message_id: str) -> None:
    session = RecordingSession([])
    with pytest.raises(ValueError, match="message id"):
        ToriiClient("https://example.invalid", session=session).get_sccp_proof_request(message_id)
    assert session.calls == []


@pytest.mark.parametrize("format", ["JSON", "artifact", "", True, None])
def test_torii_rejects_ambiguous_response_formats_without_io(format: Any) -> None:
    session = RecordingSession([])
    with pytest.raises(ValueError, match="format"):
        ToriiClient("https://example.invalid", session=session).get_sccp_proof_request(
            MESSAGE_ID, format=format
        )
    assert session.calls == []


@pytest.mark.parametrize(
    ("from_height", "limit"),
    [(-1, None), (True, None), ("1", None), (None, -1), (None, 51), (None, True)],
)
def test_torii_rejects_invalid_recent_queries_without_io(
    from_height: Any, limit: Any
) -> None:
    session = RecordingSession([])
    with pytest.raises(ValueError):
        ToriiClient("https://example.invalid", session=session).get_sccp_recent_messages(
            from_height=from_height, limit=limit
        )
    assert session.calls == []


def test_torii_proof_submit_sends_only_closed_artifact_fields() -> None:
    session = RecordingSession([StubResponse(_prepared_response(creation_time_ms=42))])
    client = ToriiClient("https://example.invalid", session=session)
    assert client.submit_bridge_proof(
        authority=AUTHORITY, destination_proof_b64="AQ==", creation_time_ms=42
    ).submitted is False
    call = session.calls[0]
    assert call["url"] == "https://example.invalid/v1/bridge/proofs/submit"
    assert json.loads(call["data"]) == {
        "authority": AUTHORITY,
        "destination_proof_b64": "AQ==",
        "creation_time_ms": 42,
    }


def test_torii_rejects_wrong_content_type_and_duplicate_submit_response() -> None:
    plain = RecordingSession([StubResponse(_prepared_response(), content_type="text/plain")])
    with pytest.raises(TypeError, match="application/json"):
        ToriiClient("https://example.invalid", session=plain).submit_bridge_proof(
            authority=AUTHORITY, destination_proof_b64="AQ=="
        )
    canonical = json.dumps(_prepared_response())
    duplicate = canonical.replace("{", '{"submitted":false,', 1).encode()
    session = RecordingSession([StubResponse(raw=duplicate)])
    with pytest.raises(ValueError, match="duplicate"):
        ToriiClient("https://example.invalid", session=session).submit_bridge_proof(
            authority=AUTHORITY, destination_proof_b64="AQ=="
        )


def test_embedded_mock_serves_only_exact_registry_bundle_and_request_routes() -> None:
    server = ToriiMockServer().start()
    try:
        config = {
            "registry": {"version": 1, "lanes": []},
            "message_bundles": {MESSAGE_ID: _bundle()},
            "proof_requests": {MESSAGE_ID: _proof_request()},
            "message_bundle_norito_b64": {MESSAGE_ID: _b64(b"bundle")},
            "proof_request_norito_b64": {MESSAGE_ID: _b64(b"request")},
            "recent_messages": {"items": [_recent()]},
        }
        response = requests.post(server.base_url + "__mock__/sccp/config", json=config, timeout=5)
        assert response.status_code == 200
        client = ToriiClient(server.base_url)
        assert client.get_sccp_registry().version == 1
        assert client.get_sccp_message_bundle(MESSAGE_ID)["version"] == 1
        assert client.get_sccp_message_bundle(MESSAGE_ID, format="norito") == b"bundle"
        assert client.get_sccp_proof_request(MESSAGE_ID)["request_hash"] == PREFIX_HASH(0x64)
        assert client.get_sccp_proof_request(MESSAGE_ID, format="norito") == b"request"
        assert len(client.get_sccp_recent_messages(from_height=9, limit=1).items) == 1
        assert requests.get(
            server.base_url + "v1/sccp/manifests", timeout=5
        ).status_code == 404
        assert requests.get(
            server.base_url + f"v1/sccp/proof-requests/{MESSAGE_ID}?allow_unready=true",
            timeout=5,
        ).status_code == 200
    finally:
        server.stop()
