"""Adversarial tests for the closed first-release SCCP Python surface."""

from __future__ import annotations

import base64
import copy
import hashlib
import inspect
import json
import sys
from pathlib import Path
from typing import Any, Callable, Dict, List, Mapping, Optional, Union

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
MESSAGE_BUNDLE_NORITO_TYPE = "iroha_sccp::TairaSccpMessageProofV1"
PROOF_REQUEST_NORITO_TYPE = "iroha_sccp::SccpGroth16Bn254ProofRequestV1"
DESTINATION_ARTIFACT_NORITO_TYPE = "iroha_sccp::SccpGroth16Bn254ProofArtifactV1"
NATIVE_INBOUND_PROOF_NORITO_TYPE = (
    "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1"
)


def _b64(value: bytes) -> str:
    return base64.b64encode(value).decode("ascii")


def _norito_crc64_xz(payload: bytes) -> int:
    polynomial = 0xC96C_5795_D787_0F42
    mask = 0xFFFF_FFFF_FFFF_FFFF
    table: List[int] = []
    for value in range(256):
        crc = value
        for _ in range(8):
            crc = (crc >> 1) ^ polynomial if crc & 1 else crc >> 1
        table.append(crc)
    crc = mask
    for byte in payload:
        crc = table[(crc ^ byte) & 0xFF] ^ (crc >> 8)
    return (crc ^ mask) & mask


def _sccp_norito_frame(
    type_name: str, *, payload: bytes = b"\x01\x02\x03\x04", padding: int = 0
) -> bytes:
    schema_hash = hashlib.sha256(
        b"norito:v1:type-name\0" + type_name.encode("utf-8")
    ).digest()[:16]
    return b"".join(
        (
            b"NRT0",
            b"\0\0",
            schema_hash,
            b"\0",
            len(payload).to_bytes(8, "little"),
            _norito_crc64_xz(payload).to_bytes(8, "little"),
            b"\x02",
            b"\0" * padding,
            payload,
        )
    )


def _destination_artifact_b64(*, padding: int = 0) -> str:
    return _b64(_sccp_norito_frame(DESTINATION_ARTIFACT_NORITO_TYPE, padding=padding))


def _native_inbound_proof_b64(*, padding: int = 0) -> str:
    return _b64(_sccp_norito_frame(NATIVE_INBOUND_PROOF_NORITO_TYPE, padding=padding))


def _network(profile: str) -> Dict[str, Any]:
    return {"network": profile.replace("-", "_"), "profile": None}


def _lane(source: str = "bsc-mainnet") -> Dict[str, Any]:
    return {"source": _network(source), "target": _network("sora-taira")}


def _native_trust_anchor(source: str = "bsc-mainnet") -> Dict[str, Any]:
    if source.startswith("ethereum-"):
        backend = "ethereum_beacon_v1"
    elif source.startswith("bsc-"):
        backend = "bsc_parlia_v1"
    else:
        backend = "tron_dpos_v1"
    return {
        "backend": {"backend": backend, "protocol": None},
        "anchor_hash": UPPER(0x91, 32),
        "checkpoint_height": 1,
    }


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
    ic.update({f"signal_{index}": _g1() for index in range(11)})
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
    for index in range(11):
        add_g1(key["ic"][f"signal_{index}"])
    return bytes.fromhex("".join(words))


def _key_hash(key: Mapping[str, Any]) -> str:
    return sccp._keccak_256(_key_bytes(key)).hex()  # noqa: SLF001 - parity oracle


def _semantic_profile() -> Dict[str, Any]:
    return {
        "profile": "sora_taira_finality_inclusion_groth16_bn254",
        "commitments": {
            "version": 1,
            "circuit_commitment": UPPER(0xC1, 32),
            "witness_generator_commitment": UPPER(0xC2, 32),
            "public_signal_schema_hash": sccp._PUBLIC_SIGNAL_SCHEMA_HASH.hex().upper(),  # noqa: SLF001
        },
    }


def _finality_anchor() -> Dict[str, Any]:
    return {
        "version": 1,
        "source_network": _network("sora-taira"),
        "protocol_version": 2,
        "chain_id_hash": sccp._SORA_TAIRA_CHAIN_ID_HASH.hex().upper(),  # noqa: SLF001
        "checkpoint_height": 7,
        "checkpoint_block_hash": UPPER(0xA1, 32),
        "checkpoint_context_id": UPPER(0xA2, 32),
        "checkpoint_finality_artifact_hash": UPPER(0xA3, 32),
    }


def _outbound_policy() -> Dict[str, Any]:
    return {
        "version": 1,
        "semantic_profile": _semantic_profile(),
        "sora_finality_anchor": _finality_anchor(),
    }


def _policy_hashes(policy: Mapping[str, Any]) -> tuple[str, str]:
    semantic, anchor = sccp._outbound_proof_policy(policy, "test policy")  # noqa: SLF001
    return semantic.hex(), anchor.hex()


def _capabilities() -> Dict[str, Any]:
    return {
        "version": 1,
        "registry_revision": PREFIX_HASH(0x10),
        "registry_path": "/v1/sccp/registry",
        "message_bundle_path": "/v1/sccp/proofs/message/{message_id}",
        "proof_request_path": "/v1/sccp/proof-requests/{message_id}",
        "recent_messages_path": "/v1/sccp/messages/recent",
        "registry_limits": {
            "max_governed_lanes": 16,
            "max_live_governed_routes": 64,
            "max_live_routes_per_lane": 8,
            "max_retained_routes_per_lane": 64,
            "max_retained_native_trust_anchors_per_lane": 4_096,
        },
        "resource_limits": {
            "max_outbound_messages_per_block": 512,
            "max_outbound_message_payload_bytes": 4_096,
            "max_pending_outbound_messages": 65_536,
            "max_pending_outbound_payload_bytes": 256 * 1024 * 1024,
            "max_proofs_per_transaction": 1,
            "max_proofs_per_block": 4,
            "max_proof_bytes_per_proof": 8 * 1024 * 1024,
            "max_proof_bytes_per_transaction": 8 * 1024 * 1024,
            "max_proof_bytes_per_block": 32 * 1024 * 1024,
            "max_native_headers_per_transaction": 1_004,
            "max_native_headers_per_block": 4_016,
            "max_ethereum_light_client_updates_per_transaction": 128,
            "max_ethereum_light_client_updates_per_block": 512,
            "max_native_header_bytes_per_transaction": 8 * 1024 * 1024,
            "max_native_header_bytes_per_block": 32 * 1024 * 1024,
            "max_secp256k1_recoveries_per_transaction": 1_005,
            "max_secp256k1_recoveries_per_block": 4_020,
            "max_bls_aggregate_checks_per_transaction": 1_004,
            "max_bls_aggregate_checks_per_block": 4_016,
            "max_bls_signer_contributions_per_transaction": 131_713,
            "max_bls_signer_contributions_per_block": 526_852,
            "max_bn254_pairing_checks_per_transaction": 1,
            "max_bn254_pairing_checks_per_block": 4,
        },
        "proof_submit_path": "/v1/bridge/proofs/submit",
        "native_message_submit_path": "/v1/bridge/messages",
    }


def _parsed_destination_and_route_hash(
    route: Mapping[str, Any],
) -> tuple[Any, bytes]:
    lane = sccp._lane(route["lane_id"], "test route lane")  # noqa: SLF001
    destination = sccp._destination(  # noqa: SLF001
        route["destination"], lane, "test route destination"
    )
    route_hash = sccp._route_configuration_hash(  # noqa: SLF001
        lane,
        route["route_id"],
        route["asset_key"],
        route["revision"],
        destination,
    )
    return destination, route_hash


def _refresh_route_config_hash(route: Dict[str, Any]) -> None:
    _, route_hash = _parsed_destination_and_route_hash(route)
    route["source_identity"]["emitter"]["identity"]["route_config_hash"] = (
        route_hash.hex().upper()
    )


def _route(
    *,
    source: str = "bsc-mainnet",
    revision: int = 1,
    activation: str = "staged",
    inbound_finality_cutoff: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    key = _verifying_key()
    route_address = UPPER(0x31, 20)
    route_code_hash = UPPER(0x41, 32)
    family = "tron" if source.startswith("tron-") else "evm"
    if source.startswith("ethereum-"):
        route_id = "taira_eth_xor"
    elif source.startswith("bsc-"):
        route_id = "taira_bsc_xor"
    else:
        route_id = "taira_tron_xor"
    route = {
        "lane_id": _lane(source),
        "route_id": route_id,
        "asset_key": "xor",
        "revision": revision,
        "activation": {"activation": activation, "direction": None},
        "inbound_finality_cutoff": inbound_finality_cutoff,
        "source_identity": {
            "lane": _lane(source),
            "emitter": {
                "emitter": family,
                "identity": {
                    "address": route_address,
                    "runtime_code_hash": route_code_hash,
                    "route_config_hash": UPPER(0x42, 32),
                },
            },
        },
        "destination": {
            "family": family,
            "deployment": {
                "token_address": UPPER(0x11, 20),
                "token_code_hash": UPPER(0x21, 32),
                "verifier_address": UPPER(0x12, 20),
                "verifier_code_hash": UPPER(0x22, 32),
                "verifying_key": key,
                "verifier_key_hash": _key_hash(key).upper(),
                "outbound_proof_policy": _outbound_policy(),
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
    _refresh_route_config_hash(route)
    return route


def _registry(
    routes: Optional[List[Dict[str, Any]]] = None,
    *,
    source: str = "bsc-mainnet",
    native_trust_anchor: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    native_trust_anchors = (
        [] if native_trust_anchor is None else [native_trust_anchor]
    )
    return {
        "version": 1,
        "lanes": [
            {
                "lane_id": _lane(source),
                "native_trust_anchors": native_trust_anchors,
                "current_native_trust_anchor_hash": (
                    native_trust_anchors[-1]["anchor_hash"]
                    if native_trust_anchors
                    else None
                ),
                "routes": routes or [_route(source=source)],
            }
        ],
    }


def _bundle() -> Dict[str, Any]:
    return {
        "version": 1,
        "commitment_root": PREFIX_HASH(0x51),
        "commitment": {
            "version": 1,
            "kind": "Transfer",
            "context": {
                "lane": {
                    "source": _network("sora-taira"),
                    "target": _network("bsc-mainnet"),
                },
                "destination_binding_hash": PREFIX_HASH(0x52),
                "route_configuration_hash": PREFIX_HASH(0x53),
            },
            "message_id": PREFIX_HASH(0x54),
            "payload_hash": PREFIX_HASH(0x55),
        },
        "merkle_proof": {"steps": []},
        "payload": {
            "Transfer": {
                "version": 1,
                "source_domain": 0,
                "dest_domain": 2,
                "nonce": "7",
                "route_revision": 1,
                "asset_home_domain": 0,
                "asset_id_codec": 1,
                "asset_id": "0x786f72",
                "amount": "1",
                "sender_codec": 1,
                "sender": "0x616c696365",
                "recipient_codec": 2,
                "recipient": "0x" + HASH(0x21)[:40],
                "route_id_codec": 1,
                "route_id": "0x74616972615f6273635f786f72",
            }
        },
        "finality_proof": "0x0102",
    }


def _proof_request() -> Dict[str, Any]:
    key = _verifying_key()
    policy = _outbound_policy()
    semantic_hash, anchor_hash = _policy_hashes(policy)
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
        "semantic_proof_profile": policy["semantic_profile"],
        "semantic_proof_profile_hash": "0x" + semantic_hash,
        "sora_finality_anchor": policy["sora_finality_anchor"],
        "sora_finality_anchor_hash": "0x" + anchor_hash,
        "bundle_bytes": "0x0102",
        "statement_hash": PREFIX_HASH(0x61),
        "destination_binding_hash": PREFIX_HASH(0x62),
        "route_configuration_hash": PREFIX_HASH(0x63),
        "request_hash": PREFIX_HASH(0x64),
    }


def _recent(
    height: int = 9, message_id: str = MESSAGE_ID, commitment_index: int = 0
) -> Dict[str, Any]:
    return {
        "height": height,
        "commitment_index": commitment_index,
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
        "payload_projection": {
            "Transfer": {
                "version": 1,
                "source_domain": 0,
                "dest_domain": 2,
                "nonce": 7,
                "route_revision": 1,
                "asset_home_domain": 0,
                "asset_id": {"CanonicalText": {"value": "xor"}},
                "amount": 1000,
                "sender": {"CanonicalText": {"value": "alice@taira"}},
                "recipient": {"EvmAddress20": {"bytes": "0x" + "11" * 20}},
                "route_id": {"CanonicalText": {"value": "taira_bsc_xor"}},
            }
        },
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
        "route_configuration_hash_hex": HASH(0x31),
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
        content_length: Optional[str] = None,
        raw: Optional[bytes] = None,
    ) -> None:
        super().__init__()
        self.status_code = status_code
        self.headers = CaseInsensitiveDict({"Content-Type": content_type})
        if content_length is not None:
            self.headers["Content-Length"] = content_length
        self._content = raw if raw is not None else json.dumps(payload).encode("utf-8")
        self._content_consumed = True
        self.encoding = "utf-8"
        self.was_closed = False

    def close(self) -> None:
        """Record closure so bounded-response tests can assert connection cleanup."""

        self.was_closed = True
        super().close()


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
        "sora-taira",
        "ethereum-mainnet",
        "ethereum-sepolia",
        "bsc-mainnet",
        "bsc-testnet",
        "tron-mainnet",
        "tron-nile",
        "tron-shasta",
    )
    assert all(profile["tag"] != 0 for profile in SCCP_NETWORK_PROFILES.values())
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
    assert not AUTHORITY.isascii(), "fixture must exercise non-ASCII I105 digits"
    assert normalize_sccp_codec_value(SCCP_CODEC_CANONICAL_TEXT, AUTHORITY) == AUTHORITY.encode()
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
        (1, "contains space"),
        (1, "line\nbreak"),
        (1, "merchant🙂"),
        (1, AUTHORITY[:-1] + ("2" if AUTHORITY.endswith("1") else "1")),
        (1, "n753" + AUTHORITY.removeprefix("sora")),
        (1, AUTHORITY + "ｲ" * 100),
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
    parsed = normalize_sccp_capabilities(_capabilities())
    assert parsed.registry_path == "/v1/sccp/registry"
    assert parsed.registry_limits.max_retained_routes_per_lane == 64
    assert parsed.registry_limits.max_retained_native_trust_anchors_per_lane == 4_096
    assert parsed.resource_limits.max_outbound_messages_per_block == 512
    assert parsed.resource_limits.max_outbound_message_payload_bytes == 4_096
    assert parsed.resource_limits.max_pending_outbound_messages == 65_536
    assert parsed.resource_limits.max_pending_outbound_payload_bytes == 256 * 1024 * 1024
    assert parsed.resource_limits.max_bls_signer_contributions_per_transaction == 131_713
    read_only = _capabilities()
    del read_only["proof_submit_path"]
    del read_only["native_message_submit_path"]
    assert normalize_sccp_capabilities(read_only).proof_submit_path is None
    mutations = (
        lambda value: value.update(registry_path="/v1/sccp/manifests"),
        lambda value: value.update(proof_request_path="/v1/sccp/proof-requests/{message_id}?x=1"),
        lambda value: value.update(proof_artifact_path="/v1/sccp/artifacts/message/{message_id}"),
        lambda value: value.update(proof_job_path="/v1/sccp/jobs/message/{message_id}"),
        lambda value: value.update(allow_unready=True),
        lambda value: value.update(registry_revision=PREFIX_HASH(0)),
        lambda value: value.pop("proof_submit_path"),
        lambda value: value.pop("native_message_submit_path"),
    )
    for mutate in mutations:
        value = _capabilities()
        mutate(value)
        with pytest.raises((TypeError, ValueError)):
            normalize_sccp_capabilities(value)

    for field in _capabilities()["resource_limits"]:
        value = _capabilities()
        value["resource_limits"][field] = 0
        with pytest.raises(ValueError, match=field):
            normalize_sccp_capabilities(value)
    for field, invalid in (
        ("max_outbound_messages_per_block", 511),
        ("max_outbound_messages_per_block", 513),
        ("max_outbound_message_payload_bytes", 4_095),
        ("max_outbound_message_payload_bytes", 4_097),
    ):
        value = _capabilities()
        value["resource_limits"][field] = invalid
        with pytest.raises(ValueError, match="fixed outbound"):
            normalize_sccp_capabilities(value)
    for field in (
        "max_outbound_messages_per_block",
        "max_outbound_message_payload_bytes",
        "max_pending_outbound_messages",
        "max_pending_outbound_payload_bytes",
    ):
        value = _capabilities()
        del value["resource_limits"][field]
        with pytest.raises(ValueError, match="missing required"):
            normalize_sccp_capabilities(value)
    drifted_registry_limits = _capabilities()
    drifted_registry_limits["registry_limits"]["max_retained_routes_per_lane"] = 65
    with pytest.raises(ValueError, match="fixed V1 capacities"):
        normalize_sccp_capabilities(drifted_registry_limits)


@pytest.mark.parametrize(
    ("lower_field", "upper_field", "expected"),
    (
        (
            "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction",
            "per-proof byte limit",
        ),
        ("max_proofs_per_transaction", "max_proofs_per_block", "transaction resource"),
        (
            "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block",
            "transaction resource",
        ),
        (
            "max_native_headers_per_transaction",
            "max_native_headers_per_block",
            "transaction resource",
        ),
        (
            "max_ethereum_light_client_updates_per_transaction",
            "max_ethereum_light_client_updates_per_block",
            "transaction resource",
        ),
        (
            "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block",
            "transaction resource",
        ),
        (
            "max_secp256k1_recoveries_per_transaction",
            "max_secp256k1_recoveries_per_block",
            "transaction resource",
        ),
        (
            "max_bls_aggregate_checks_per_transaction",
            "max_bls_aggregate_checks_per_block",
            "transaction resource",
        ),
        (
            "max_bls_signer_contributions_per_transaction",
            "max_bls_signer_contributions_per_block",
            "transaction resource",
        ),
        (
            "max_bn254_pairing_checks_per_transaction",
            "max_bn254_pairing_checks_per_block",
            "transaction resource",
        ),
    ),
)
def test_capabilities_reject_every_reversed_resource_limit_relation(
    lower_field: str, upper_field: str, expected: str
) -> None:
    reversed_limits = _capabilities()
    reversed_limits["resource_limits"][lower_field] = (
        reversed_limits["resource_limits"][upper_field] + 1
    )
    with pytest.raises(ValueError, match=expected):
        normalize_sccp_capabilities(reversed_limits)


def test_capability_integers_preserve_canonical_json_tokens_and_shared_range() -> None:
    canonical = json.dumps(_capabilities(), separators=(",", ":"))
    needle = '"max_proofs_per_transaction":1'
    assert needle in canonical
    for token in ("1.0", "1e0", "-0", "9007199254740992.5", "1e999"):
        hostile = canonical.replace(needle, f'"max_proofs_per_transaction":{token}')
        with pytest.raises(ValueError):
            parse_sccp_json_object(hostile, "SCCP capabilities")

    boundary = _capabilities()
    byte_fields = (
        "max_proof_bytes_per_proof",
        "max_proof_bytes_per_transaction",
        "max_proof_bytes_per_block",
        "max_native_header_bytes_per_transaction",
        "max_native_header_bytes_per_block",
        "max_pending_outbound_messages",
        "max_pending_outbound_payload_bytes",
    )
    for field in byte_fields:
        boundary["resource_limits"][field] = (1 << 53) - 1
    assert normalize_sccp_capabilities(boundary).resource_limits.max_proof_bytes_per_block == (
        1 << 53
    ) - 1
    boundary["resource_limits"]["max_proof_bytes_per_block"] = 1 << 53
    with pytest.raises(ValueError, match="max_proof_bytes_per_block"):
        normalize_sccp_capabilities(boundary)
    for field in ("max_pending_outbound_messages", "max_pending_outbound_payload_bytes"):
        overflow = _capabilities()
        overflow["resource_limits"][field] = 1 << 53
        with pytest.raises(ValueError, match=field):
            normalize_sccp_capabilities(overflow)


def test_registry_checks_retained_history_caps_before_traversal() -> None:
    exact_anchors = _registry()
    exact_anchors["lanes"][0]["native_trust_anchors"] = [None] * 4_096
    with pytest.raises(ValueError) as exact_anchor_error:
        normalize_sccp_registry(exact_anchors)
    assert "more than 4,096" not in str(exact_anchor_error.value)

    over_anchors = _registry()
    over_anchors["lanes"][0]["native_trust_anchors"] = [None] * 4_097
    with pytest.raises(ValueError, match="more than 4,096"):
        normalize_sccp_registry(over_anchors)

    exact_routes = _registry()
    exact_routes["lanes"][0]["routes"] = [{}] * 64
    with pytest.raises((TypeError, ValueError)) as exact_route_error:
        normalize_sccp_registry(exact_routes)
    assert "more than 64 retained" not in str(exact_route_error.value)

    over_routes = _registry()
    over_routes["lanes"][0]["routes"] = [{}] * 65
    with pytest.raises(ValueError, match="more than 64 retained"):
        normalize_sccp_registry(over_routes)


def test_registry_validates_full_key_and_rejects_retired_or_aliased_routes() -> None:
    assert len(normalize_sccp_registry(_registry()).lanes) == 1
    for removed in ("sora-nexus", "sora_nexus"):
        retired_sora = _registry()
        retired_sora["lanes"][0]["lane_id"]["target"] = {
            "network": removed,
            "profile": None,
        }
        with pytest.raises(ValueError, match="retired"):
            normalize_sccp_registry(retired_sora)
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
    ten_signal = _registry()
    del ten_signal["lanes"][0]["routes"][0]["destination"]["deployment"]["verifying_key"]["ic"][
        "signal_10"
    ]
    with pytest.raises(ValueError, match="signal_10"):
        normalize_sccp_registry(ten_signal)
    policyless = _registry()
    del policyless["lanes"][0]["routes"][0]["destination"]["deployment"][
        "outbound_proof_policy"
    ]
    with pytest.raises(ValueError, match="outbound_proof_policy"):
        normalize_sccp_registry(policyless)
    wrong_asset = _registry()
    wrong_asset["lanes"][0]["routes"][0]["settlement"]["asset_definition_id"] = "xor"
    with pytest.raises(ValueError, match="canonical Taira XOR"):
        normalize_sccp_registry(wrong_asset)
    noncanonical_custody = _registry()
    noncanonical_custody["lanes"][0]["routes"][0]["settlement"][
        "custody_account_id"
    ] = "n753" + AUTHORITY.removeprefix("sora")
    with pytest.raises(ValueError, match="exact canonical rendering"):
        normalize_sccp_registry(noncanonical_custody)


@pytest.mark.parametrize(
    ("mutation", "expected"),
    (
        (lambda anchor: anchor.update(protocol_version=1), "protocol_version"),
        (lambda anchor: anchor.update(protocol_version=True), "integer"),
        (lambda anchor: anchor.update(checkpoint_context_id=UPPER(0, 32)), "nonzero"),
        (
            lambda anchor: anchor.update(
                checkpoint_finality_artifact_hash=anchor["checkpoint_context_id"]
            ),
            "consensus hash role",
        ),
        (lambda anchor: anchor.update(validator_set_epoch=2), "field"),
    ),
)
def test_registry_rejects_legacy_or_ambiguous_v2_finality_anchor(
    mutation, expected: str
) -> None:
    value = _registry()
    anchor = value["lanes"][0]["routes"][0]["destination"]["deployment"][
        "outbound_proof_policy"
    ]["sora_finality_anchor"]
    mutation(anchor)
    with pytest.raises((TypeError, ValueError), match=expected):
        normalize_sccp_registry(value)


@pytest.mark.parametrize(
    ("source", "binding", "deployment", "configuration"),
    (
        (
            "bsc-mainnet",
            "e2ce4a5df24ee62891f0f856b3e418f5bd3e2705baefd80a5fbf5e8cc2d3de1e",
            "2958dc4b874a166fbca91d1d1342c57c5150264c96c8d65fd64df8d57b46ab24",
            "0fc9aacab4fda553fff88ac434294fa879b4205e723c377a82754bdc2db152c6",
        ),
        (
            "tron-nile",
            "f24976e50078da09188c4a2101facfba7b905cfd33cc895d3e12fc64e52a654a",
            "5d9d742ae3e48271dc66edd579f23ce7dbe29c92fb3bfa1956da2ac97272fec3",
            "a806a759ea6104c7202276811a0ac8dd8e6f40ac37d6050f93f7d0106c921f9d",
        ),
    ),
)
def test_registry_destination_and_route_hashes_match_canonical_vectors(
    source: str, binding: str, deployment: str, configuration: str
) -> None:
    route = _route(source=source)
    destination, route_hash = _parsed_destination_and_route_hash(route)
    assert destination.destination_binding_hash.hex() == binding
    assert destination.deployment_config_hash.hex() == deployment
    assert route_hash.hex() == configuration
    assert (
        route["source_identity"]["emitter"]["identity"]["route_config_hash"]
        == configuration.upper()
    )
    assert len(normalize_sccp_registry(_registry([route], source=source)).lanes) == 1


@pytest.mark.parametrize("source", ("bsc-mainnet", "tron-nile"))
def test_registry_policy_mutations_recompute_every_hash_and_reject_stale_emitter(
    source: str,
) -> None:
    baseline = _route(source=source)
    baseline_destination, baseline_route_hash = _parsed_destination_and_route_hash(baseline)
    baseline_policy = baseline["destination"]["deployment"]["outbound_proof_policy"]
    baseline_policy_hashes = _policy_hashes(baseline_policy)

    for policy_role in ("semantic_profile", "sora_finality_anchor"):
        candidate = copy.deepcopy(baseline)
        policy = candidate["destination"]["deployment"]["outbound_proof_policy"]
        if policy_role == "semantic_profile":
            policy["semantic_profile"]["commitments"]["circuit_commitment"] = UPPER(
                0xD1, 32
            )
        else:
            policy["sora_finality_anchor"]["checkpoint_height"] += 1

        changed_policy_hashes = _policy_hashes(policy)
        assert changed_policy_hashes != baseline_policy_hashes
        changed_destination, changed_route_hash = _parsed_destination_and_route_hash(candidate)
        assert (
            changed_destination.destination_binding_hash
            != baseline_destination.destination_binding_hash
        )
        assert (
            changed_destination.deployment_config_hash
            != baseline_destination.deployment_config_hash
        )
        assert changed_route_hash != baseline_route_hash

        with pytest.raises(ValueError, match="source route_config_hash"):
            normalize_sccp_registry(_registry([candidate], source=source))

        _refresh_route_config_hash(candidate)
        assert len(normalize_sccp_registry(_registry([candidate], source=source)).lanes) == 1


@pytest.mark.parametrize("source", ("bsc-mainnet", "tron-nile"))
def test_registry_recomputes_binding_and_deployment_intermediaries(source: str) -> None:
    baseline = _route(source=source)
    baseline_destination, baseline_route_hash = _parsed_destination_and_route_hash(baseline)

    changed_code = copy.deepcopy(baseline)
    changed_code["destination"]["deployment"]["token_code_hash"] = UPPER(0x25, 32)
    code_destination, code_route_hash = _parsed_destination_and_route_hash(changed_code)
    assert (
        code_destination.destination_binding_hash
        == baseline_destination.destination_binding_hash
    )
    assert code_destination.deployment_config_hash != baseline_destination.deployment_config_hash
    assert code_route_hash != baseline_route_hash
    with pytest.raises(ValueError, match="source route_config_hash"):
        normalize_sccp_registry(_registry([changed_code], source=source))

    changed_verifier = copy.deepcopy(baseline)
    changed_verifier["destination"]["deployment"]["verifier_address"] = UPPER(0x13, 20)
    verifier_destination, verifier_route_hash = _parsed_destination_and_route_hash(
        changed_verifier
    )
    assert (
        verifier_destination.destination_binding_hash
        != baseline_destination.destination_binding_hash
    )
    assert (
        verifier_destination.deployment_config_hash
        != baseline_destination.deployment_config_hash
    )
    assert verifier_route_hash != baseline_route_hash
    with pytest.raises(ValueError, match="source route_config_hash"):
        normalize_sccp_registry(_registry([changed_verifier], source=source))


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
                ],
                native_trust_anchor=_native_trust_anchor(),
            )
        )


@pytest.mark.parametrize("activation", ("bidirectional", "inbound_only"))
def test_registry_inbound_activation_requires_native_trust_anchor(activation: str) -> None:
    with pytest.raises(ValueError, match="without a native trust anchor"):
        normalize_sccp_registry(_registry([_route(activation=activation)]))
    assert len(
        normalize_sccp_registry(
            _registry(
                [_route(activation=activation)],
                native_trust_anchor=_native_trust_anchor(),
            )
        ).lanes
    ) == 1


def test_registry_requires_append_only_trust_anchor_history_and_current_pointer() -> None:
    first = _native_trust_anchor()
    second = copy.deepcopy(first)
    second["anchor_hash"] = UPPER(0x92, 32)
    second["checkpoint_height"] = 2
    canonical = _registry(
        [_route(activation="inbound_only")], native_trust_anchor=first
    )
    canonical["lanes"][0]["native_trust_anchors"].append(second)
    canonical["lanes"][0]["current_native_trust_anchor_hash"] = second["anchor_hash"]
    assert len(normalize_sccp_registry(canonical).lanes) == 1

    stale_pointer = copy.deepcopy(canonical)
    stale_pointer["lanes"][0]["current_native_trust_anchor_hash"] = first["anchor_hash"]
    with pytest.raises(ValueError, match="last retained anchor"):
        normalize_sccp_registry(stale_pointer)

    duplicate = copy.deepcopy(canonical)
    duplicate["lanes"][0]["native_trust_anchors"][1]["anchor_hash"] = first[
        "anchor_hash"
    ]
    duplicate["lanes"][0]["current_native_trust_anchor_hash"] = first["anchor_hash"]
    with pytest.raises(ValueError, match="duplicate native trust-anchor"):
        normalize_sccp_registry(duplicate)

    rollback = copy.deepcopy(canonical)
    rollback["lanes"][0]["native_trust_anchors"][1]["checkpoint_height"] = 1
    with pytest.raises(ValueError, match="advance monotonically"):
        normalize_sccp_registry(rollback)

    legacy = copy.deepcopy(canonical)
    legacy["lanes"][0]["native_trust_anchor"] = first
    del legacy["lanes"][0]["native_trust_anchors"]
    del legacy["lanes"][0]["current_native_trust_anchor_hash"]
    with pytest.raises(ValueError, match="field set|unknown or retired"):
        normalize_sccp_registry(legacy)


def test_retired_routes_require_one_complete_retained_anchor_interval() -> None:
    first = _native_trust_anchor()
    second = copy.deepcopy(first)
    second["anchor_hash"] = UPPER(0x92, 32)
    second["checkpoint_height"] = 2
    cutoff = {
        "trust_anchor_hash": first["anchor_hash"],
        "max_anchor_interval_height": second["checkpoint_height"],
    }
    canonical = _registry(
        [_route(activation="retired", inbound_finality_cutoff=cutoff)],
        native_trust_anchor=first,
    )
    canonical["lanes"][0]["native_trust_anchors"].append(second)
    canonical["lanes"][0]["current_native_trust_anchor_hash"] = second["anchor_hash"]
    assert len(normalize_sccp_registry(canonical).lanes) == 1

    missing = copy.deepcopy(canonical)
    missing["lanes"][0]["routes"][0]["inbound_finality_cutoff"] = None
    with pytest.raises(ValueError, match="required for a retired"):
        normalize_sccp_registry(missing)

    nonterminal = copy.deepcopy(canonical)
    nonterminal["lanes"][0]["routes"][0]["activation"]["activation"] = "paused"
    with pytest.raises(ValueError, match="allowed only for a retired"):
        normalize_sccp_registry(nonterminal)

    for mutate in (
        lambda value: value.update(trust_anchor_hash=UPPER(0xFF, 32)),
        lambda value: value.update(max_anchor_interval_height=1),
        lambda value: value.update(trust_anchor_hash=second["anchor_hash"]),
    ):
        incomplete = copy.deepcopy(canonical)
        mutate(incomplete["lanes"][0]["routes"][0]["inbound_finality_cutoff"])
        with pytest.raises(ValueError, match="complete retained anchor interval"):
            normalize_sccp_registry(incomplete)

    omitted = _registry()
    del omitted["lanes"][0]["routes"][0]["inbound_finality_cutoff"]
    with pytest.raises(ValueError, match="field set|missing required"):
        normalize_sccp_registry(omitted)


def test_registry_accepts_zero_bn254_limbs_but_rejects_all_zero_point() -> None:
    route = _route()
    key = route["destination"]["deployment"]["verifying_key"]
    key["alpha1"]["x"] = UPPER(0, 32)
    route["destination"]["deployment"]["verifier_key_hash"] = _key_hash(key).upper()
    _refresh_route_config_hash(route)
    assert len(normalize_sccp_registry(_registry([route])).lanes) == 1

    key["alpha1"]["y"] = UPPER(0, 32)
    with pytest.raises(ValueError, match="point at infinity"):
        normalize_sccp_registry(_registry([route]))


def test_recent_links_are_exact_and_route_configuration_is_independent() -> None:
    parsed = normalize_sccp_recent_messages({"items": [_recent(9), _recent(8, HASH(0x12))]})
    assert [item["height"] for item in parsed.items] == [9, 8]
    assert parsed.next is None
    continued = normalize_sccp_recent_messages(
        {
            "items": [
                _recent((1 << 64) - 1, HASH(0x13), 510),
                _recent((1 << 64) - 1, HASH(0x14), 511),
            ],
            "next": {"from": (1 << 64) - 1, "after_index": 511},
        }
    )
    assert [item["commitment_index"] for item in continued.items] == [510, 511]
    assert continued.next is not None
    assert continued.next.from_height == (1 << 64) - 1
    assert continued.next.after_index == 511
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
        normalize_sccp_recent_messages({"items": [_recent(8), _recent(9, HASH(0x12))]})
    with pytest.raises(ValueError, match="duplicate"):
        normalize_sccp_recent_messages({"items": [_recent(), _recent()]})
    for indices in ((1, 1), (1, 3), (2, 1)):
        with pytest.raises(ValueError, match="contiguous"):
            normalize_sccp_recent_messages(
                {
                    "items": [
                        _recent(9, HASH(0x15), indices[0]),
                        _recent(9, HASH(0x16), indices[1]),
                    ]
                }
            )
    with pytest.raises(ValueError, match="begin at commitment index zero"):
        normalize_sccp_recent_messages(
            {"items": [_recent(9), _recent(8, HASH(0x17), 1)]}
        )
    for mutation in (
        lambda value: value.pop("commitment_index"),
        lambda value: value.update(commitment_index=512),
        lambda value: value.update(commitment_index=True),
    ):
        invalid_index = _recent()
        mutation(invalid_index)
        with pytest.raises((TypeError, ValueError)):
            normalize_sccp_recent_messages({"items": [invalid_index]})
    unknown_item = _recent()
    unknown_item["commitment_position"] = 0
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_recent_messages({"items": [unknown_item]})
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_recent_messages(
            {"items": [_recent()], "cursor": {"from": 9, "after_index": 0}}
        )
    for next_cursor in (
        None,
        {"from": 9, "after_index": 1},
        {"from": 9, "after_index": 0, "extra": 0},
        {"from": 9, "after_index": 512},
        {"from": True, "after_index": 0},
    ):
        with pytest.raises((TypeError, ValueError)):
            normalize_sccp_recent_messages({"items": [_recent()], "next": next_cursor})
    with pytest.raises(ValueError, match="empty"):
        normalize_sccp_recent_messages(
            {"items": [], "next": {"from": 9, "after_index": 0}}
        )
    oversized = _recent()
    oversized["amount"] = str(1 << 128)
    with pytest.raises(ValueError, match="u128"):
        normalize_sccp_recent_messages({"items": [oversized]})
    with pytest.raises(ValueError, match="50"):
        normalize_sccp_recent_messages(
            {"items": [_recent(51 - index, HASH(index + 1)) for index in range(51)]}
        )
    mutations = (
        lambda value: value.pop("payload_projection"),
        lambda value: value.update(payload_projection=None),
        lambda value: value["payload_projection"]["Transfer"].update(dest_domain=5),
        lambda value: value["payload_projection"]["Transfer"].update(
            recipient={"CanonicalText": {"value": "not-an-address"}}
        ),
        lambda value: value["payload_projection"]["Transfer"]["route_id"][
            "CanonicalText"
        ].update(value="taira_tron_xor"),
        lambda value: value["payload_projection"]["Transfer"].update(amount=0),
        lambda value: value.update(amount="1001"),
    )
    for mutate in mutations:
        invalid_projection = _recent()
        mutate(invalid_projection)
        with pytest.raises((TypeError, ValueError)):
            normalize_sccp_recent_messages({"items": [invalid_projection]})


def test_bundle_and_proof_request_are_closed_and_query_free() -> None:
    assert normalize_sccp_message_bundle(_bundle())["version"] == 1
    assert normalize_sccp_proof_request(_proof_request())["public_inputs"]["target_domain"] == 2
    burn = _bundle()
    burn["payload"] = {"Burn": {}}
    with pytest.raises(ValueError, match="retired"):
        normalize_sccp_message_bundle(burn)
    aliased_commitment = _bundle()
    aliased_commitment["commitment"]["context"]["route_configuration_hash"] = (
        aliased_commitment["commitment"]["context"]["destination_binding_hash"]
    )
    with pytest.raises(ValueError, match="role-separated"):
        normalize_sccp_message_bundle(aliased_commitment)
    reserved_domain = _bundle()
    reserved_domain["payload"]["Transfer"]["dest_domain"] = 3
    with pytest.raises(ValueError, match="reserved"):
        normalize_sccp_message_bundle(reserved_domain)
    oversized_nonce = _bundle()
    oversized_nonce["payload"]["Transfer"]["nonce"] = str(1 << 64)
    with pytest.raises(ValueError, match="u64"):
        normalize_sccp_message_bundle(oversized_nonce)
    wrong_recipient_codec = _bundle()
    wrong_recipient_codec["payload"]["Transfer"]["recipient_codec"] = 5
    with pytest.raises(ValueError, match="protocol domain"):
        normalize_sccp_message_bundle(wrong_recipient_codec)
    long_merkle_path = _bundle()
    long_merkle_path["merkle_proof"]["steps"] = [
        {"sibling_hash": PREFIX_HASH(0x70), "sibling_is_left": False} for _ in range(65)
    ]
    with pytest.raises(ValueError, match="64"):
        normalize_sccp_message_bundle(long_merkle_path)
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
    wrong_semantic = _proof_request()
    wrong_semantic["semantic_proof_profile_hash"] = PREFIX_HASH(0x99)
    with pytest.raises(ValueError, match="semantic_proof_profile_hash"):
        normalize_sccp_proof_request(wrong_semantic)
    wrong_anchor = _proof_request()
    wrong_anchor["sora_finality_anchor_hash"] = PREFIX_HASH(0x99)
    with pytest.raises(ValueError, match="sora_finality_anchor_hash"):
        normalize_sccp_proof_request(wrong_anchor)
    cross_policy_alias = _proof_request()
    cross_policy_alias["semantic_proof_profile"]["commitments"]["circuit_commitment"] = (
        cross_policy_alias["sora_finality_anchor"]["checkpoint_block_hash"]
    )
    semantic_hash, _ = sccp._semantic_proof_profile(  # noqa: SLF001
        cross_policy_alias["semantic_proof_profile"], "test semantic profile"
    )
    cross_policy_alias["semantic_proof_profile_hash"] = "0x" + semantic_hash.hex()
    with pytest.raises(ValueError, match="proof-policy hash role"):
        normalize_sccp_proof_request(cross_policy_alias)
    archived_identity = _proof_request()
    archived_identity["sora_finality_anchor"]["chain_id_hash"] = sccp._keccak_256(  # noqa: SLF001
        bytes.fromhex("809574f5fee75e69bfcf52451e42d50f")
    ).hex().upper()
    with pytest.raises(ValueError, match="Taira chain commitment"):
        normalize_sccp_proof_request(archived_identity)


def test_submit_dtos_have_no_redundant_public_key_or_caller_selected_route() -> None:
    transaction_payload_b64 = _b64(b"\x01\x02\x03\x04")
    proof = normalize_bridge_proof_submit_payload(
        {
            "authority": AUTHORITY,
            "signature_b64": base64.b64encode(bytes([1]) * 64).decode("ascii"),
            "transaction_payload_b64": transaction_payload_b64,
            "destination_proof_b64": _destination_artifact_b64(),
            "creation_time_ms": 10,
        }
    )
    assert list(proof) == [
        "authority",
        "signature_b64",
        "transaction_payload_b64",
        "destination_proof_b64",
        "creation_time_ms",
    ]
    assert proof["transaction_payload_b64"] == transaction_payload_b64
    assert list(
        normalize_bridge_message_submit_payload(
            {"authority": AUTHORITY, "native_proof_b64": _native_inbound_proof_b64()}
        )
    ) == ["authority", "native_proof_b64"]
    native = normalize_bridge_message_submit_payload(
        {
            "authority": AUTHORITY,
            "signature_b64": "AQ==",
            "transaction_payload_b64": transaction_payload_b64,
            "native_proof_b64": _native_inbound_proof_b64(),
            "creation_time_ms": 10,
        }
    )
    assert native["transaction_payload_b64"] == transaction_payload_b64
    parameters = inspect.signature(ToriiClient.submit_bridge_proof).parameters
    assert "public_key_hex" not in parameters
    assert "message_bundle_b64" not in parameters
    assert "destination_proof_b64" in parameters
    assert "transaction_payload_b64" in parameters


def test_submit_authorities_require_exact_canonical_i105() -> None:
    noncanonical = "n753" + AUTHORITY.removeprefix("sora")
    with pytest.raises(ValueError, match="exact canonical rendering"):
        normalize_bridge_proof_submit_payload(
            {"authority": noncanonical, "destination_proof_b64": _destination_artifact_b64()}
        )
    with pytest.raises(ValueError, match="exact canonical rendering"):
        normalize_bridge_message_submit_payload(
            {"authority": noncanonical, "native_proof_b64": _native_inbound_proof_b64()}
        )


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
        ("client_signature_b64", "AQ=="),
    ],
)
def test_proof_submit_rejects_retired_fields(field: str, value: Any) -> None:
    with pytest.raises(ValueError, match="retired"):
        normalize_bridge_proof_submit_payload(
            {
                "authority": AUTHORITY,
                "destination_proof_b64": _destination_artifact_b64(),
                field: value,
            }
        )


@pytest.mark.parametrize("artifact", ["AQ", " AQ==", "AQ==\n", "", "====", "A==="])
def test_proof_submit_rejects_noncanonical_base64(artifact: str) -> None:
    with pytest.raises(ValueError, match="base64"):
        normalize_bridge_proof_submit_payload(
            {"authority": AUTHORITY, "destination_proof_b64": artifact}
        )


def test_submit_artifacts_require_exact_schema_and_zero_alignment_padding() -> None:
    normalize_bridge_proof_submit_payload(
        {"authority": AUTHORITY, "destination_proof_b64": _destination_artifact_b64()}
    )
    normalize_bridge_message_submit_payload(
        {"authority": AUTHORITY, "native_proof_b64": _native_inbound_proof_b64()}
    )
    with pytest.raises(ValueError, match="schema hash"):
        normalize_bridge_proof_submit_payload(
            {"authority": AUTHORITY, "destination_proof_b64": _native_inbound_proof_b64()}
        )
    with pytest.raises(ValueError, match="schema hash"):
        normalize_bridge_message_submit_payload(
            {"authority": AUTHORITY, "native_proof_b64": _destination_artifact_b64()}
        )
    for padding in (1, 8, 64):
        with pytest.raises(ValueError, match="alignment padding"):
            normalize_bridge_proof_submit_payload(
                {
                    "authority": AUTHORITY,
                    "destination_proof_b64": _destination_artifact_b64(padding=padding),
                }
            )
        with pytest.raises(ValueError, match="alignment padding"):
            normalize_bridge_message_submit_payload(
                {
                    "authority": AUTHORITY,
                    "native_proof_b64": _native_inbound_proof_b64(padding=padding),
                }
            )


@pytest.mark.parametrize(
    "signing_state",
    [
        {"signature_b64": "AQ==", "creation_time_ms": 1},
        {"transaction_payload_b64": "AQ==", "creation_time_ms": 1},
        {"signature_b64": "AQ==", "transaction_payload_b64": "Ag=="},
        {
            "signature_b64": "AQ",
            "transaction_payload_b64": "Ag==",
            "creation_time_ms": 1,
        },
        {
            "signature_b64": "AQ==",
            "transaction_payload_b64": "Ag",
            "creation_time_ms": 1,
        },
    ],
)
def test_proof_submit_rejects_mixed_or_malformed_signing_state(
    signing_state: Mapping[str, Any],
) -> None:
    with pytest.raises(ValueError, match="provide both|required|base64"):
        normalize_bridge_proof_submit_payload(
            {
                "authority": AUTHORITY,
                "destination_proof_b64": _destination_artifact_b64(),
                **signing_state,
            }
        )


@pytest.mark.parametrize("timestamp", [0, -1, 1.5, True, "1"])
def test_proof_submit_rejects_nonpositive_or_ambiguous_time(timestamp: Any) -> None:
    with pytest.raises(ValueError, match="integer"):
        normalize_bridge_proof_submit_payload(
            {
                "authority": AUTHORITY,
                "destination_proof_b64": _destination_artifact_b64(),
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
        _prepared_response(manifest_hash_hex=HASH(3)),
        _prepared_response(route_configuration_hash_hex=HASH(0xAB).upper()),
        _prepared_response(creation_time_ms=0),
        _prepared_response(tx_hash_hex=HASH(4)),
        _prepared_response(transaction_payload_b64=_b64(b"\x01\x02\x03\x05")),
        _prepared_response(signing_message_b64=_b64(b"\x09" * 32)),
    ):
        with pytest.raises((TypeError, ValueError)):
            normalize_sccp_bridge_submit_response(value)
    missing_route_hash = _prepared_response()
    del missing_route_hash["route_configuration_hash_hex"]
    with pytest.raises(ValueError, match="missing required"):
        normalize_sccp_bridge_submit_response(missing_route_hash)
    with pytest.raises(ValueError, match="signing state"):
        normalize_sccp_bridge_submit_response(_prepared_response(), {"submitted": True})
    canonical = json.dumps(_prepared_response())
    assert parse_sccp_bridge_submit_response_json(canonical).submitted is False
    with pytest.raises(ValueError, match="duplicate"):
        parse_sccp_bridge_submit_response_json(canonical.replace("{", '{"submitted":false,', 1))
    duplicated_hash = canonical.replace(
        f'"route_configuration_hash_hex": "{HASH(0x31)}"',
        f'"route_configuration_hash_hex": "{HASH(0x31)}", '
        f'"route_configuration_hash_hex": "{HASH(0x32)}"',
    )
    with pytest.raises(ValueError, match="duplicate"):
        parse_sccp_bridge_submit_response_json(duplicated_hash)
    with pytest.raises(ValueError):
        parse_sccp_json_object(canonical + "{}")


def test_torii_exact_endpoints_and_content_negotiation() -> None:
    proof_request_frame = _sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE)
    session = RecordingSession(
        [
            StubResponse(_capabilities()),
            StubResponse({"version": 1, "lanes": []}),
            StubResponse(_bundle()),
            StubResponse(raw=proof_request_frame, content_type="application/x-norito"),
            StubResponse({"items": []}),
        ]
    )
    client = ToriiClient("https://example.invalid", session=session)
    assert client.get_sccp_capabilities().version == 1
    assert client.get_sccp_registry().version == 1
    assert client.get_sccp_message_bundle(MESSAGE_ID)["version"] == 1
    assert client.get_sccp_proof_request(MESSAGE_ID, format="norito") == proof_request_frame
    assert client.get_sccp_recent_messages(
        from_height=(1 << 64) - 1, after_index=511, limit=1
    ).items == ()
    assert [call["url"] for call in session.calls] == [
        "https://example.invalid/v1/sccp/capabilities",
        "https://example.invalid/v1/sccp/registry",
        f"https://example.invalid/v1/sccp/proofs/message/{MESSAGE_ID}",
        f"https://example.invalid/v1/sccp/proof-requests/{MESSAGE_ID}",
        "https://example.invalid/v1/sccp/messages/recent",
    ]
    assert session.calls[3]["headers"] == {"Accept": "application/x-norito"}
    assert session.calls[4]["params"] == {
        "from": str((1 << 64) - 1),
        "after_index": "511",
        "limit": "1",
    }
    assert all(call["stream"] is True for call in session.calls)


def test_torii_sccp_norito_preflight_accepts_exact_type_padding() -> None:
    frame = _sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE)
    response = StubResponse(raw=frame, content_type="application/x-norito")
    client = ToriiClient(
        "https://example.invalid", session=RecordingSession([response])
    )
    assert client.get_sccp_proof_request(MESSAGE_ID, format="norito") == frame
    assert response.was_closed is True


def test_torii_sccp_norito_preflight_rejects_malformed_and_cross_type_frames() -> None:
    canonical = _sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE)

    def mutate(offset: int, value: int) -> bytes:
        frame = bytearray(canonical)
        frame[offset] = value
        return bytes(frame)

    declared_long = bytearray(canonical)
    declared_long[23:31] = (5).to_bytes(8, "little")
    declared_short = bytearray(canonical)
    declared_short[23:31] = (3).to_bytes(8, "little")
    cases = [
        ("empty body", b""),
        ("short header", canonical[:39]),
        ("magic", mutate(0, 0)),
        ("major version", mutate(4, 1)),
        ("minor version", mutate(5, 1)),
        ("zero schema", canonical[:6] + b"\0" * 16 + canonical[22:]),
        ("wrong response type", _sccp_norito_frame(MESSAGE_BUNDLE_NORITO_TYPE)),
        ("compressed payload", mutate(22, 1)),
        ("reserved flag", mutate(39, 0x08)),
        ("invalid bitset flags", mutate(39, 0x20)),
        ("declared payload too long", bytes(declared_long)),
        ("declared payload too short", bytes(declared_short)),
        ("checksum", mutate(31, canonical[31] ^ 0x01)),
        ("one-byte noncanonical padding", _sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE, padding=1)),
        ("eight-byte noncanonical padding", _sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE, padding=8)),
        ("64-byte noncanonical padding", _sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE, padding=64)),
        (
            "65-byte padding",
            _sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE, padding=65),
        ),
        ("trailing byte", canonical + b"\0"),
    ]
    for label, body in cases:
        response = StubResponse(raw=body, content_type="application/x-norito")
        client = ToriiClient(
            "https://example.invalid", session=RecordingSession([response])
        )
        with pytest.raises(ValueError):
            client.get_sccp_proof_request(MESSAGE_ID, format="norito")
        assert response.was_closed is True, label


def _padded_json_bytes(value: Mapping[str, Any], byte_length: int) -> bytes:
    canonical = json.dumps(value).encode("utf-8")
    assert len(canonical) <= byte_length
    return canonical + b" " * (byte_length - len(canonical))


def test_torii_sccp_streaming_accepts_exact_capability_size_and_closes() -> None:
    maximum_bytes = 64 * 1024
    response = StubResponse(
        raw=_padded_json_bytes(_capabilities(), maximum_bytes),
        content_length=str(maximum_bytes),
    )
    client = ToriiClient(
        "https://example.invalid", session=RecordingSession([response])
    )
    assert client.get_sccp_capabilities().version == 1
    assert response.was_closed is True


@pytest.mark.parametrize(
    ("label", "content_length"),
    [
        ("negative", "-1"),
        ("explicit plus", "+1"),
        ("leading zero", "01"),
        ("fractional", "1.0"),
        ("coalesced duplicate", "1, 1"),
        ("trailing whitespace", "1 "),
        ("leading whitespace", " 1"),
        ("empty", ""),
    ],
)
def test_torii_sccp_streaming_rejects_noncanonical_content_length(
    label: str, content_length: str
) -> None:
    response = StubResponse(_capabilities())
    response.headers["Content-Length"] = content_length
    client = ToriiClient(
        "https://example.invalid", session=RecordingSession([response])
    )
    with pytest.raises(ValueError, match="canonical unsigned decimal"):
        client.get_sccp_capabilities()
    assert response.was_closed is True, label


@pytest.mark.parametrize(
    ("label", "content_length"),
    [
        ("missing Content-Length", None),
        ("understated Content-Length", "1"),
    ],
)
def test_torii_sccp_streaming_rejects_actual_capability_overflow(
    label: str, content_length: Optional[str]
) -> None:
    response = StubResponse(
        raw=b" " * (64 * 1024 + 1), content_length=content_length
    )
    client = ToriiClient(
        "https://example.invalid", session=RecordingSession([response])
    )
    with pytest.raises(ValueError, match="65536-byte size bound"):
        client.get_sccp_capabilities()
    assert response.was_closed is True, label


def test_torii_sccp_streaming_rejects_declared_overflow_before_body_read() -> None:
    response = StubResponse(
        _capabilities(), content_length=str(64 * 1024 + 1)
    )
    client = ToriiClient(
        "https://example.invalid", session=RecordingSession([response])
    )
    with pytest.raises(ValueError, match="65536-byte size bound"):
        client.get_sccp_capabilities()
    assert response.was_closed is True


def test_torii_sccp_streaming_rejects_non_utf8_json_and_closes() -> None:
    response = StubResponse(raw=b'{"\xff":1}')
    client = ToriiClient(
        "https://example.invalid", session=RecordingSession([response])
    )
    with pytest.raises(ValueError, match="UTF-8 JSON"):
        client.get_sccp_capabilities()
    assert response.was_closed is True


def test_torii_sccp_error_response_uses_the_same_actual_byte_bound() -> None:
    response = StubResponse(status_code=400, raw=b" " * (64 * 1024 + 1))
    client = ToriiClient(
        "https://example.invalid", session=RecordingSession([response])
    )
    with pytest.raises(ValueError, match="65536-byte size bound"):
        client.get_sccp_capabilities()
    assert response.was_closed is True


@pytest.mark.parametrize(
    ("maximum_bytes", "content_type", "invoke"),
    [
        (
            8 * 1024 * 1024,
            "application/json",
            lambda client: client.get_sccp_recent_messages(),
        ),
        (
            16 * 1024 * 1024,
            "application/x-norito",
            lambda client: client.get_sccp_message_bundle(MESSAGE_ID, format="norito"),
        ),
        (
            16 * 1024 * 1024 + 64 * 1024,
            "application/x-norito",
            lambda client: client.get_sccp_proof_request(MESSAGE_ID, format="norito"),
        ),
        (
            64 * 1024 * 1024,
            "application/json",
            lambda client: client.submit_bridge_proof(
                authority=AUTHORITY, destination_proof_b64=_destination_artifact_b64()
            ),
        ),
    ],
)
def test_torii_sccp_routes_apply_endpoint_specific_declared_limits(
    maximum_bytes: int,
    content_type: str,
    invoke: Callable[[ToriiClient], Any],
) -> None:
    response = StubResponse(
        {},
        content_type=content_type,
        content_length=str(maximum_bytes + 1),
    )
    client = ToriiClient(
        "https://example.invalid", session=RecordingSession([response])
    )
    with pytest.raises(ValueError, match=rf"{maximum_bytes}-byte size bound"):
        invoke(client)
    assert response.was_closed is True


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
    ("from_height", "after_index", "limit"),
    [
        (0, None, None),
        (-1, None, None),
        (True, None, None),
        ("1", None, None),
        ((1 << 64), None, None),
        (None, 0, None),
        (1, -1, None),
        (1, 512, None),
        (1, True, None),
        (None, None, 0),
        (None, None, -1),
        (None, None, 51),
        (None, None, True),
    ],
)
def test_torii_rejects_invalid_recent_queries_without_io(
    from_height: Any, after_index: Any, limit: Any
) -> None:
    session = RecordingSession([])
    with pytest.raises(ValueError):
        ToriiClient("https://example.invalid", session=session).get_sccp_recent_messages(
            from_height=from_height, after_index=after_index, limit=limit
        )
    assert session.calls == []


def test_torii_proof_submit_sends_only_closed_artifact_fields() -> None:
    session = RecordingSession([StubResponse(_prepared_response(creation_time_ms=42))])
    client = ToriiClient("https://example.invalid", session=session)
    destination_artifact = _destination_artifact_b64()
    assert client.submit_bridge_proof(
        authority=AUTHORITY,
        destination_proof_b64=destination_artifact,
        creation_time_ms=42,
    ).submitted is False
    call = session.calls[0]
    assert call["url"] == "https://example.invalid/v1/bridge/proofs/submit"
    assert json.loads(call["data"]) == {
        "authority": AUTHORITY,
        "destination_proof_b64": destination_artifact,
        "creation_time_ms": 42,
    }


def test_torii_prepare_then_submit_resends_byte_identical_transaction_payload() -> None:
    prepared = _prepared_response(creation_time_ms=42)
    submitted = _prepared_response(
        submitted=True,
        creation_time_ms=42,
        tx_hash_hex=HASH(0x55),
        transaction_payload_b64=None,
        signing_message_b64=None,
    )
    session = RecordingSession([StubResponse(prepared), StubResponse(submitted)])
    client = ToriiClient("https://example.invalid", session=session)
    destination_artifact = _destination_artifact_b64()
    preparation = client.submit_bridge_proof(
        authority=AUTHORITY,
        destination_proof_b64=destination_artifact,
        creation_time_ms=42,
    )
    submission = client.submit_bridge_proof(
        authority=AUTHORITY,
        destination_proof_b64=destination_artifact,
        signature_b64=_b64(bytes([7]) * 64),
        transaction_payload_b64=preparation.transaction_payload_b64,
        creation_time_ms=preparation.creation_time_ms,
    )
    assert submission.submitted is True
    submitted_body = json.loads(session.calls[1]["data"])
    assert submitted_body["transaction_payload_b64"] == prepared["transaction_payload_b64"]
    assert base64.b64decode(submitted_body["transaction_payload_b64"], validate=True) == bytes(
        [1, 2, 3, 4]
    )


def test_torii_rejects_response_state_that_contradicts_request() -> None:
    submitted = _prepared_response(
        submitted=True,
        tx_hash_hex=HASH(0x55),
        transaction_payload_b64=None,
        signing_message_b64=None,
    )
    prepare_session = RecordingSession([StubResponse(submitted)])
    with pytest.raises(ValueError, match="signing state"):
        ToriiClient("https://example.invalid", session=prepare_session).submit_bridge_proof(
            authority=AUTHORITY, destination_proof_b64=_destination_artifact_b64()
        )
    signed_session = RecordingSession([StubResponse(_prepared_response(creation_time_ms=42))])
    with pytest.raises(ValueError, match="signing state"):
        ToriiClient("https://example.invalid", session=signed_session).submit_bridge_proof(
            authority=AUTHORITY,
            destination_proof_b64=_destination_artifact_b64(),
            signature_b64="AQ==",
            transaction_payload_b64="Ag==",
            creation_time_ms=42,
        )


def test_torii_rejects_wrong_content_type_and_duplicate_submit_response() -> None:
    plain = RecordingSession([StubResponse(_prepared_response(), content_type="text/plain")])
    with pytest.raises(TypeError, match="application/json"):
        ToriiClient("https://example.invalid", session=plain).submit_bridge_proof(
            authority=AUTHORITY, destination_proof_b64=_destination_artifact_b64()
        )
    canonical = json.dumps(_prepared_response())
    duplicate = canonical.replace("{", '{"submitted":false,', 1).encode()
    session = RecordingSession([StubResponse(raw=duplicate)])
    with pytest.raises(ValueError, match="duplicate"):
        ToriiClient("https://example.invalid", session=session).submit_bridge_proof(
            authority=AUTHORITY, destination_proof_b64=_destination_artifact_b64()
        )


def test_embedded_mock_serves_only_exact_registry_bundle_and_request_routes() -> None:
    server = ToriiMockServer().start()
    try:
        config = {
            "registry": {"version": 1, "lanes": []},
            "message_bundles": {MESSAGE_ID: _bundle()},
            "proof_requests": {MESSAGE_ID: _proof_request()},
            "message_bundle_norito_b64": {
                MESSAGE_ID: _b64(_sccp_norito_frame(MESSAGE_BUNDLE_NORITO_TYPE))
            },
            "proof_request_norito_b64": {
                MESSAGE_ID: _b64(_sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE))
            },
            "recent_messages": {"items": [_recent()]},
        }
        response = requests.post(server.base_url + "__mock__/sccp/config", json=config, timeout=5)
        assert response.status_code == 200
        client = ToriiClient(server.base_url)
        assert client.get_sccp_registry().version == 1
        assert client.get_sccp_message_bundle(MESSAGE_ID)["version"] == 1
        assert client.get_sccp_message_bundle(
            MESSAGE_ID, format="norito"
        ) == _sccp_norito_frame(MESSAGE_BUNDLE_NORITO_TYPE)
        assert client.get_sccp_proof_request(MESSAGE_ID)["request_hash"] == PREFIX_HASH(0x64)
        assert client.get_sccp_proof_request(
            MESSAGE_ID, format="norito"
        ) == _sccp_norito_frame(PROOF_REQUEST_NORITO_TYPE)
        assert len(client.get_sccp_recent_messages(from_height=9, limit=1).items) == 1
        assert requests.get(
            server.base_url + "v1/sccp/manifests", timeout=5
        ).status_code == 404
        assert requests.get(
            server.base_url + f"v1/sccp/proof-requests/{MESSAGE_ID}?allow_unready=true",
            timeout=5,
        ).status_code == 400
    finally:
        server.stop()
