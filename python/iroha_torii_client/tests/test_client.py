from __future__ import annotations

import base64
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

import pytest
import requests
from requests.structures import CaseInsensitiveDict

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import (  # noqa: E402  (import depends on sys.path mutation)
    ExplorerAccountQr,
    NetworkTimeSnapshot,
    NetworkTimeStatus,
    ToriiClient,
    decode_pdp_commitment_header,
)

CANONICAL_OWNER = "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"
CANONICAL_ASSET_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9"


class StubResponse(requests.Response):
    def __init__(
        self,
        status_code: int = 200,
        payload: Optional[Any] = None,
        *,
        headers: Optional[Dict[str, str]] = None,
        text: Optional[str] = None,
    ) -> None:
        super().__init__()
        self.status_code = status_code
        self._payload = payload
        self.headers = CaseInsensitiveDict(headers or {})
        if payload is None:
            content = text.encode("utf-8") if text is not None else b""
        else:
            content = json.dumps(payload).encode("utf-8")
            if "Content-Type" not in self.headers:
                self.headers["Content-Type"] = "application/json"
        self._content = content
        self.encoding = "utf-8"

    def json(self, **kwargs: Any) -> Any:
        if self._payload is None:
            raise ValueError("no payload available")
        return json.loads(self.text)


class RecordingSession(requests.Session):
    def __init__(self) -> None:
        super().__init__()
        self.calls: List[Dict[str, Any]] = []
        self._responses: List[StubResponse] = []

    def queue(self, response: StubResponse) -> None:
        self._responses.append(response)

    def request(
        self,
        method: Union[str, bytes],
        url: Union[str, bytes],
        *args: Any,
        **kwargs: Any,
    ) -> requests.Response:
        params = kwargs.get("params") or {}
        headers = kwargs.get("headers") or {}
        data = kwargs.get("data")
        self.calls.append(
            {
                "method": method,
                "url": url,
                "params": params,
                "headers": headers,
                "data": data,
            }
        )
        if not self._responses:
            raise AssertionError("no queued responses")
        return self._responses.pop(0)


def test_list_peers_returns_typed_records() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=[
                {"address": "127.0.0.1:1337", "id": {"public_key": "ed01"}},
                {"address": "[::1]:1337", "id": {"public_key": "ed02"}},
            ]
        )
    )
    client = ToriiClient("http://node.test", session=session)

    peers = client.list_peers()

    assert len(peers) == 2
    assert peers[0].address == "127.0.0.1:1337"
    assert peers[0].public_key_hex == "ed01"
    assert session.calls == [
        {
            "method": "GET",
            "url": "http://node.test/v1/peers",
            "params": {},
            "headers": {},
            "data": None,
        }
    ]


def test_list_telemetry_peers_info_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=[
                {
                    "url": "https://peer-1.example",
                    "connected": True,
                    "telemetry_unsupported": False,
                    "config": {
                        "public_key": "ed011122",
                        "queue_capacity": 8,
                        "network_block_gossip_size": 32,
                        "network_block_gossip_period": {"ms": 150},
                        "network_tx_gossip_size": 16,
                        "network_tx_gossip_period": {"ms": 50},
                    },
                    "location": {"lat": 35.0, "lon": 139.7, "country": "JP", "city": "Tokyo"},
                    "connected_peers": ["peer-A", "peer-B"],
                }
            ]
        )
    )
    client = ToriiClient("http://node.test", session=session)

    peers = client.list_telemetry_peers_info()

    assert len(peers) == 1
    peer = peers[0]
    assert peer.url == "https://peer-1.example"
    assert peer.connected is True
    assert peer.telemetry_unsupported is False
    assert peer.config is not None
    assert peer.config.queue_capacity == 8
    assert peer.config.network_block_gossip_period_ms == 150
    assert peer.location is not None
    assert peer.location.country == "JP"
    assert peer.connected_peers == ["peer-A", "peer-B"]
    assert session.calls[0]["headers"] == {"Accept": "application/json"}


def test_list_telemetry_peers_info_rejects_non_list_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"not": "a list"}))
    client = ToriiClient("http://node.test", session=session)

    try:
        client.list_telemetry_peers_info()
    except RuntimeError as exc:
        assert "/v1/telemetry/peers-info response must be a list" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for invalid telemetry response")


def test_list_telemetry_peers_info_rejects_camelcase_config_fields() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload=[
                {
                    "url": "https://peer-2.example",
                    "connected": True,
                    "telemetry_unsupported": False,
                    "config": {
                        "publicKey": "ed011122",
                    },
                }
            ]
        )
    )
    client = ToriiClient("http://node.test", session=session)

    try:
        client.list_telemetry_peers_info()
    except RuntimeError as exc:
        assert "missing `public_key`" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for camelCase telemetry config")


def test_get_health_status_returns_plain_text() -> None:
    session = RecordingSession()
    session.queue(StubResponse(text="Healthy"))
    client = ToriiClient("http://node.test", session=session)

    assert client.get_health_status() == "Healthy"
    assert session.calls[0]["url"].endswith("/v1/health")
    assert session.calls[0]["method"] == "GET"


def test_runtime_manifest_rejects_alias_fields() -> None:
    try:
        ToriiClient._normalize_runtime_manifest_payload(
            {
                "name": "upgrade-1",
                "description": "First upgrade",
                "abiVersion": 1,
                "abi_hash": "0" * 64,
                "start_height": 1,
                "end_height": 2,
            },
            context="runtime upgrade manifest",
        )
    except RuntimeError as exc:
        assert "abi_version is required" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for alias manifest fields")


def test_runtime_manifest_rejects_non_v1_abi_version() -> None:
    with pytest.raises(RuntimeError, match="abi_version must be 1"):
        ToriiClient._normalize_runtime_manifest_payload(
            {
                "name": "upgrade-1",
                "description": "First upgrade",
                "abi_version": 2,
                "abi_hash": "0" * 64,
                "start_height": 1,
                "end_height": 2,
            },
            context="runtime upgrade manifest",
        )


def test_runtime_manifest_rejects_non_empty_added_surfaces() -> None:
    with pytest.raises(RuntimeError, match="added_syscalls must be empty"):
        ToriiClient._normalize_runtime_manifest_payload(
            {
                "name": "upgrade-1",
                "description": "First upgrade",
                "abi_version": 1,
                "abi_hash": "0" * 64,
                "added_syscalls": [512],
                "start_height": 1,
                "end_height": 2,
            },
            context="runtime upgrade manifest",
        )


def test_get_node_version_returns_string() -> None:
    session = RecordingSession()
    session.queue(StubResponse(text="2.1.0-dev"))
    client = ToriiClient("http://node.test", session=session)

    assert client.get_node_version() == "2.1.0-dev"
    assert session.calls[0]["url"].endswith("/v1/version")
    assert session.calls[0]["method"] == "GET"


def test_get_time_now_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "now": 1_700_000,
                "offset_ms": -4,
                "confidence_ms": 9,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_time_now()

    assert isinstance(snapshot, NetworkTimeSnapshot)
    assert snapshot.now_ms == 1_700_000
    assert snapshot.offset_ms == -4
    assert snapshot.confidence_ms == 9
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/time/now")
    assert call["headers"]["Accept"] == "application/json"


def test_get_time_status_parses_histogram_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "peers": 3,
                "samples": [
                    {"peer": "peer-1", "last_offset_ms": -2, "last_rtt_ms": 7, "count": 5}
                ],
                "rtt": {
                    "buckets": [
                        {"le": 5, "count": 10},
                        {"count": 2},
                    ],
                    "sum_ms": 42,
                    "count": 12,
                },
                "note": "ok",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    status = client.get_time_status()

    assert isinstance(status, NetworkTimeStatus)
    assert status.peers == 3
    assert len(status.samples) == 1
    sample = status.samples[0]
    assert sample.peer == "peer-1"
    assert sample.last_offset_ms == -2
    assert sample.last_rtt_ms == 7
    assert sample.count == 5
    assert len(status.rtt_buckets) == 2
    first_bucket, second_bucket = status.rtt_buckets
    assert first_bucket.upper_bound_ms == 5
    assert first_bucket.count == 10
    assert second_bucket.upper_bound_ms is None
    assert second_bucket.count == 2
    assert status.rtt_sum_ms == 42
    assert status.rtt_count == 12
    assert status.note == "ok"
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/time/status")
    assert call["headers"]["Accept"] == "application/json"


def test_get_explorer_account_qr_parses_payload_and_params() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "canonical_id": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                "literal": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                "network_prefix": 26,
                "error_correction": "quartile",
                "modules": 33,
                "qr_version": 5,
                "svg": "<svg></svg>",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    qr = client.get_explorer_account_qr("6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL")

    assert qr == ExplorerAccountQr(
        canonical_id="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        literal="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        network_prefix=26,
        error_correction="quartile",
        modules=33,
        qr_version=5,
        svg="<svg></svg>",
    )
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith(
        "/v1/explorer/accounts/6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL/qr"
    )
    assert call["params"] == {}
    assert call["headers"]["Accept"] == "application/json"


def test_get_explorer_account_qr_normalizes_payload_variants() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "canonicalId": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
                "literal": "sorabobacct",
                "networkPrefix": 27,
                "errorCorrection": "medium",
                "modules": 41,
                "qrVersion": 7,
                "svg": "<svg/>",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    qr = client.get_explorer_account_qr("34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r")

    assert qr == ExplorerAccountQr(
        canonical_id="34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
        literal="sorabobacct",
        network_prefix=27,
        error_correction="medium",
        modules=41,
        qr_version=7,
        svg="<svg/>",
    )
    call = session.calls[0]
    assert call["params"] == {}
    assert call["headers"]["Accept"] == "application/json"


def test_get_node_capabilities_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "abi_version": 1,
                "data_model_version": 1,
                "crypto": {
                    "sm": {
                        "enabled": True,
                        "default_hash": "sm3",
                        "allowed_signing": ["sm2"],
                        "sm2_distid_default": "soranet",
                        "openssl_preview": False,
                        "acceleration": {
                            "scalar": True,
                            "neon_sm3": True,
                            "neon_sm4": False,
                            "policy": "scalar",
                        },
                    },
                    "curves": {
                        "registry_version": 2,
                        "allowed_curve_ids": [1, 15],
                        "allowed_curve_bitmap": [32770],
                    },
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    capabilities = client.get_node_capabilities()

    assert capabilities.abi_version == 1
    assert capabilities.data_model_version == 1
    assert capabilities.crypto.sm.allowed_signing == ["sm2"]
    assert capabilities.crypto.sm.acceleration.neon_sm3 is True
    assert capabilities.crypto.curves.registry_version == 2
    assert capabilities.crypto.curves.allowed_curve_bitmap == [32770]


def test_get_runtime_abi_active_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "abi_version": 1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_runtime_abi_active()

    assert snapshot.abi_version == 1


def test_get_runtime_abi_hash_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "policy": "V1",
                "abi_hash_hex": "aa" * 32,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.get_runtime_abi_hash()

    assert result.policy == "V1"
    assert result.abi_hash_hex == "aa" * 32


def test_get_runtime_metrics_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "abi_version": 1,
                "upgrade_events_total": {"proposed": 5, "activated": 3, "canceled": 1},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    metrics = client.get_runtime_metrics()

    assert metrics.abi_version == 1
    assert metrics.upgrade_events_total.proposed == 5
    assert metrics.upgrade_events_total.activated == 3
    assert metrics.upgrade_events_total.canceled == 1


def test_list_runtime_upgrades_parses_records() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "id_hex": "aa" * 32,
                        "record": {
                            "manifest": {
                                "name": "ABI v1 refresh",
                                "description": "scheduled rollout",
                                "abi_version": 1,
                                "abi_hash": "11" * 32,
                                "added_syscalls": [],
                                "added_pointer_types": [],
                                "start_height": 10,
                                "end_height": 20,
                            },
                            "status": {"ActivatedAt": 12},
                            "proposer": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                            "created_height": 8,
                        },
                    },
                    {
                        "id_hex": "bb" * 32,
                        "record": {
                            "manifest": {
                                "name": "ABI v1 maintenance",
                                "description": "next window",
                                "abi_version": 1,
                                "abi_hash": "22" * 32,
                                "added_syscalls": [],
                                "added_pointer_types": [],
                                "start_height": 30,
                                "end_height": 40,
                            },
                            "status": {"Proposed": None},
                            "proposer": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
                            "created_height": 25,
                        },
                    },
                ]
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    upgrades = client.list_runtime_upgrades()

    assert len(upgrades) == 2
    assert upgrades[0].record.status.kind == "ActivatedAt"
    assert upgrades[0].record.status.activated_height == 12
    assert upgrades[1].record.status.kind == "Proposed"


def test_propose_runtime_upgrade_posts_manifest() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [{"wire_id": "ProposeRuntimeUpgrade", "payload_hex": "aa" * 32}],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.propose_runtime_upgrade(
        {
            "name": "ABI v1 maintenance",
            "description": "roll out refreshed binaries",
            "abi_version": 1,
            "abi_hash": "ff" * 32,
            "start_height": 50,
            "end_height": 60,
            "added_syscalls": [],
            "added_pointer_types": [],
        }
    )

    assert result.ok is True
    assert result.tx_instructions[0].wire_id == "ProposeRuntimeUpgrade"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades/propose")
    body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert body["abi_version"] == 1
    assert body["abi_hash"] == "ff" * 32


def test_activate_runtime_upgrade_posts_identifier() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [{"wire_id": "ActivateRuntimeUpgrade", "payload_hex": "cc" * 32}],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    response = client.activate_runtime_upgrade("0x" + "bb" * 32)

    assert response.tx_instructions[0].wire_id == "ActivateRuntimeUpgrade"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades/activate/0x" + "bb" * 32)


def test_cancel_runtime_upgrade_posts_identifier() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [{"wire_id": "CancelRuntimeUpgrade", "payload_hex": "dd" * 32}],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    response = client.cancel_runtime_upgrade("aa" * 32)

    assert response.tx_instructions[0].wire_id == "CancelRuntimeUpgrade"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades/cancel/0x" + "aa" * 32)


def test_get_uaid_portfolio_parses_payload() -> None:
    uaid_literal = "UAID:" + "AB" * 32
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "uaid": uaid_literal,
                "totals": {"accounts": 2, "positions": 3},
                "dataspaces": [
                    {
                        "dataspace_id": 7,
                        "dataspace_alias": "treasury",
                        "accounts": [
                            {
                                "account_id": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                                "label": "primary",
                                "assets": [
                                    {
                                        "asset_id": CANONICAL_ASSET_ID,
                                        "asset_definition_id": "xor#wonderland",
                                        "quantity": "42",
                                    }
                                ],
                            }
                        ],
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    response = client.get_uaid_portfolio(uaid_literal)

    assert response.uaid == "uaid:" + "ab" * 32
    assert response.totals.accounts == 2
    assert response.dataspaces[0].accounts[0].assets[0].quantity == "42"
    expected_suffix = "/v1/accounts/uaid%3A" + "ab" * 32 + "/portfolio"
    assert session.calls[0]["url"].endswith(expected_suffix)


def test_get_uaid_portfolio_encodes_asset_id_filter() -> None:
    uaid_literal = "uaid:" + "ab" * 32
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "uaid": uaid_literal,
                "totals": {"accounts": 0, "positions": 0},
                "dataspaces": [],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.get_uaid_portfolio(uaid_literal, asset_id=CANONICAL_ASSET_ID)

    assert session.calls[0]["params"]["asset_id"] == CANONICAL_ASSET_ID


def test_get_uaid_portfolio_rejects_invalid_lsb() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())
    invalid = "uaid:" + "10" * 32
    with pytest.raises(RuntimeError, match="least significant bit"):
        client.get_uaid_portfolio(invalid)


def test_get_uaid_bindings_fetches_dataspace_accounts() -> None:
    uaid_literal = "uaid:" + "bb" * 32
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "uaid": uaid_literal,
                "dataspaces": [
                    {
                        "dataspace_id": 9,
                        "dataspace_alias": "alpha",
                        "accounts": ["6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL", " 34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r "],
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    bindings = client.get_uaid_bindings(uaid_literal)

    assert bindings.dataspaces[0].accounts == ["6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL", "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r"]
    assert session.calls[0]["params"] == {}


def test_get_uaid_manifests_parses_payload_and_filters() -> None:
    uaid_literal = "uaid:" + "cd" * 32
    manifest_hash = "0x" + "dd" * 32
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "uaid": uaid_literal,
                "manifests": [
                    {
                        "dataspace_id": 5,
                        "dataspace_alias": "lane-5",
                        "manifest_hash": manifest_hash,
                        "status": "Active",
                        "lifecycle": {
                            "activated_epoch": 12,
                            "revocation": {"epoch": 44, "reason": "duplicate"},
                        },
                        "accounts": ["6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"],
                        "manifest": {
                            "version": "1.0",
                            "uaid": uaid_literal,
                            "dataspace": 5,
                            "issued_ms": 123,
                            "activation_epoch": 12,
                            "entries": [
                                {
                                    "scope": {"accounts": ["6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"]},
                                    "effect": {"action": "allow"},
                                    "notes": "demo",
                                }
                            ],
                        },
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    manifests = client.get_uaid_manifests(
        uaid_literal,
        dataspace_id=9,
    )

    assert len(manifests.manifests) == 1
    record = manifests.manifests[0]
    assert record.manifest_hash == manifest_hash.lower()
    assert record.lifecycle.revocation is not None
    assert record.manifest.entries[0].notes == "demo"
    assert session.calls[0]["params"] == {"dataspace": 9}


def test_publish_space_directory_manifest_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=202, payload={"queued": True}))
    client = ToriiClient("http://node.test", session=session)

    manifest: Dict[str, Any] = {
        "version": "V1",
        "uaid": "uaid:" + "11" * 32,
        "dataspace": 7,
        "entries": [{"scope": {"program": "cbdc.transfer"}, "effect": {"Allow": {"max_amount": "10"}}}],
    }
    response = client.publish_space_directory_manifest(
        authority=CANONICAL_OWNER,
        private_key="ed25519:AAAA",
        manifest=manifest,
        reason="demo",
    )

    assert response == {"queued": True}
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"].endswith("/v1/space-directory/manifests")
    assert session.calls[0]["headers"]["Content-Type"] == "application/json"
    body = json.loads(session.calls[0]["data"])
    assert body["authority"] == CANONICAL_OWNER
    assert body["reason"] == "demo"
    assert body["manifest"]["entries"][0]["scope"]["program"] == "cbdc.transfer"

    manifest["entries"][0]["scope"]["program"] = "mutated"
    assert body["manifest"]["entries"][0]["scope"]["program"] == "cbdc.transfer"


def test_revoke_space_directory_manifest_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=202))
    client = ToriiClient("http://node.test", session=session)

    result = client.revoke_space_directory_manifest(
        authority=CANONICAL_OWNER,
        private_key="ed25519:BBBB",
        uaid="UAID:" + "23" * 32,
        dataspace=3,
        revoked_epoch=4096,
        reason="audit",
    )

    assert result is None
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/space-directory/manifests/revoke")
    payload = json.loads(call["data"])
    assert payload["uaid"] == "uaid:" + "23" * 32
    assert payload["dataspace"] == 3
    assert payload["revoked_epoch"] == 4096
    assert payload["reason"] == "audit"


def test_get_configuration_returns_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "public_key": "ed0123",
                "logger": {"level": "Info", "filter": None},
                "network": {
                    "block_gossip_size": 32,
                    "block_gossip_period_ms": 150,
                    "transaction_gossip_size": 16,
                    "transaction_gossip_period_ms": 75,
                },
                "queue": {"capacity": 1024},
                "confidential_gas": {
                    "proof_base": 10,
                    "per_public_input": 2,
                    "per_proof_byte": 3,
                    "per_nullifier": 4,
                    "per_commitment": 5,
                },
                "transport": {
                    "norito_rpc": {
                        "enabled": True,
                        "stage": "ga",
                        "require_mtls": True,
                        "canary_allowlist_size": 3,
                    },
                    "streaming": {
                        "soranet": {
                            "enabled": True,
                            "stream_tag": "norito",
                            "exit_multiaddr": "/dns/torii/udp/9443/quic",
                            "padding_budget_ms": 25,
                            "access_kind": "authenticated",
                            "gar_category": "soranet-auth",
                            "channel_salt": "salt-123",
                            "provision_spool_dir": "./storage/streaming/soranet_routes",
                            "provision_window_segments": 4,
                            "provision_queue_capacity": 256,
                        }
                    },
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_configuration()

    assert snapshot.public_key_hex == "ed0123"
    assert snapshot.logger.level == "Info"
    assert snapshot.logger.filter is None
    assert snapshot.queue is not None and snapshot.queue.capacity == 1024
    assert snapshot.confidential_gas is not None
    assert snapshot.confidential_gas.per_nullifier == 4
    transport = snapshot.transport
    assert transport is not None
    assert transport.norito_rpc is not None
    assert transport.norito_rpc.stage == "ga"
    assert transport.norito_rpc.canary_allowlist_size == 3
    assert transport.streaming is not None
    assert transport.streaming.soranet is not None
    assert transport.streaming.soranet.padding_budget_ms == 25
    assert transport.streaming.soranet.provision_queue_capacity == 256


def test_update_configuration_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=202))
    client = ToriiClient("http://node.test", session=session)

    result = client.update_configuration({"logger": {"level": "Info", "filter": "net=debug"}})

    assert result == {}
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"].endswith("/v1/configuration")
    assert json.loads(session.calls[0]["data"]) == {"logger": {"level": "Info", "filter": "net=debug"}}


def test_get_sumeragi_qc_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "highest_qc": {"height": 10, "view": 2, "subject_block_hash": "aa11"},
                "locked_qc": {"height": 9, "view": 1, "subject_block_hash": None},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_sumeragi_qc()

    assert snapshot.highest_qc.height == 10
    assert snapshot.locked_qc.subject_block_hash is None
    assert session.calls[0]["url"].endswith("/v1/sumeragi/qc")


def test_get_status_snapshot_parses_payload_and_computes_metrics() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=_status_payload(queue_size=4, da_total=1, approved=3, rejected=1, views=2)))
    session.queue(StubResponse(payload=_status_payload(queue_size=9, da_total=4, approved=5, rejected=2, views=5)))
    client = ToriiClient("http://node.test", session=session)

    first = client.get_status_snapshot()
    second = client.get_status_snapshot()

    assert first.status.queue_size == 4
    assert first.metrics.queue_delta == 0
    assert first.metrics.has_activity is False
    assert first.status.lane_commitments[0].lane_id == 7
    assert first.status.dataspace_commitments[0].dataspace_id == 9

    assert second.status.queue_size == 9
    assert second.metrics.queue_delta == 5
    assert second.metrics.da_reschedule_delta == 3
    assert second.metrics.tx_approved_delta == 2
    assert second.metrics.tx_rejected_delta == 1
    assert second.metrics.view_change_delta == 3
    assert second.metrics.has_activity is True
    lane_gov = second.status.lane_governance[0]
    assert lane_gov.alias == "lane-alpha"
    assert lane_gov.runtime_upgrade is not None
    assert lane_gov.runtime_upgrade.allowed_ids == ["alpha"]
    assert second.status.lane_governance_sealed_aliases == ["sealed-one"]
    assert "peers" in second.status.raw


def _status_payload(
    *,
    queue_size: int,
    da_total: int,
    approved: int,
    rejected: int,
    views: int,
) -> Dict[str, Any]:
    governance = {
        "proposals": {
            "proposed": 1,
            "approved": 2,
            "rejected": 3,
            "enacted": 4,
        },
        "protected_namespace": {
            "total_checks": 4,
            "allowed": 3,
            "rejected": 1,
        },
        "manifest_admission": {
            "total_checks": 5,
            "allowed": 4,
            "missing_manifest": 1,
            "non_validator_authority": 0,
            "quorum_rejected": 0,
            "protected_namespace_rejected": 0,
            "runtime_hook_rejected": 0,
        },
        "manifest_quorum": {
            "total_checks": 3,
            "satisfied": 2,
            "rejected": 1,
        },
        "recent_manifest_activations": [
            {
                "namespace": "demo",
                "contract_id": "contract.demo",
                "code_hash_hex": "deadbeef",
                "abi_hash_hex": "cafebabe",
                "height": 42,
                "activated_at_ms": 1_111,
            }
        ],
    }
    lane_commitments = [
        {
            "block_height": 10,
            "lane_id": 7,
            "tx_count": 2,
            "total_chunks": 4,
            "rbc_bytes_total": 64,
            "teu_total": 128,
            "block_hash": "hash-lane",
        }
    ]
    dataspace_commitments = [
        {
            "block_height": 10,
            "lane_id": 7,
            "dataspace_id": 9,
            "tx_count": 2,
            "total_chunks": 4,
            "rbc_bytes_total": 64,
            "teu_total": 128,
            "block_hash": "hash-dataspace",
        }
    ]
    lane_governance = [
        {
            "lane_id": 7,
            "alias": "lane-alpha",
            "dataspace_id": 9,
            "visibility": "public",
            "storage_profile": "balanced",
            "governance": None,
            "manifest_required": True,
            "manifest_ready": False,
            "manifest_path": None,
            "validator_ids": ["val#1"],
            "quorum": 1,
            "protected_namespaces": ["alpha"],
            "runtime_upgrade": {
                "allow": True,
                "require_metadata": True,
                "metadata_key": "manifest",
                "allowed_ids": ["alpha"],
            },
        }
    ]
    return {
        "peers": 5,
        "queue_size": queue_size,
        "commit_time_ms": 250,
        "da_reschedule_total": da_total,
        "txs_approved": approved,
        "txs_rejected": rejected,
        "view_changes": views,
        "governance": governance,
        "lane_commitments": lane_commitments,
        "dataspace_commitments": dataspace_commitments,
        "lane_governance": lane_governance,
        "lane_governance_sealed_total": 1,
        "lane_governance_sealed_aliases": ["sealed-one"],
    }


def test_get_sumeragi_pacemaker_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "backoff_ms": 50,
                "rtt_floor_ms": 10,
                "jitter_ms": 5,
                "backoff_multiplier": 2,
                "rtt_floor_multiplier": 3,
                "max_backoff_ms": 120,
                "jitter_frac_permille": 25,
                "round_elapsed_ms": 40,
                "view_timeout_target_ms": 90,
                "view_timeout_remaining_ms": 12,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    pacemaker = client.get_sumeragi_pacemaker()

    assert pacemaker.backoff_ms == 50
    assert pacemaker.view_timeout_remaining_ms == 12
    assert session.calls[0]["url"].endswith("/v1/sumeragi/pacemaker")


def test_get_sumeragi_phases_parses_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "propose_ms": 1,
                "collect_da_ms": 2,
                "collect_prevote_ms": 3,
                "collect_precommit_ms": 4,
                "collect_aggregator_ms": 5,
                "commit_ms": 8,
                "pipeline_total_ms": 9,
                "collect_aggregator_gossip_total": 10,
                "block_created_dropped_by_lock_total": 11,
                "block_created_hint_mismatch_total": 12,
                "block_created_proposal_mismatch_total": 13,
                "ema_ms": {
                    "propose_ms": 14,
                    "collect_da_ms": 15,
                    "collect_prevote_ms": 16,
                    "collect_precommit_ms": 17,
                    "collect_aggregator_ms": 18,
                    "commit_ms": 21,
                    "pipeline_total_ms": 22,
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    phases = client.get_sumeragi_phases()

    assert phases.collect_aggregator_ms == 5
    assert phases.ema_ms.commit_ms == 21
    assert session.calls[0]["url"].endswith("/v1/sumeragi/phases")


def test_get_sumeragi_leader_parses_prf() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "leader_index": 3,
                "prf": {"height": 100, "view": 4, "epoch_seed": "ff00"},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    leader = client.get_sumeragi_leader()

    assert leader.leader_index == 3
    assert leader.prf.epoch_seed == "ff00"
    assert session.calls[0]["url"].endswith("/v1/sumeragi/leader")


def test_get_sumeragi_collectors_parses_entries() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "consensus_mode": "Permissioned",
                "mode": "Permissioned",
                "topology_len": 7,
                "min_votes_for_commit": 5,
                "proxy_tail_index": 2,
                "height": 11,
                "view": 3,
                "collectors_k": 4,
                "redundant_send_r": 1,
                "epoch_seed": "abcd",
                "collectors": [{"index": 0, "peer_id": "peer#0"}, {"index": 1, "peer_id": "peer#1"}],
                "prf": {"height": 11, "view": 3, "epoch_seed": None},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    collectors = client.get_sumeragi_collectors()

    assert collectors.collectors_k == 4
    assert collectors.collectors[1].peer_id == "peer#1"
    assert session.calls[0]["url"].endswith("/v1/sumeragi/collectors")


def test_get_sumeragi_params_parses_flags() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "block_time_ms": 2000,
                "commit_time_ms": 500,
                "max_clock_drift_ms": 20,
                "collectors_k": 3,
                "redundant_send_r": 1,
                "da_enabled": True,
                "next_mode": None,
                "mode_activation_height": 1200,
                "chain_height": 777,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    params = client.get_sumeragi_params()

    assert params.da_enabled is True
    assert params.mode_activation_height == 1200
    assert session.calls[0]["url"].endswith("/v1/sumeragi/params")


def test_get_sumeragi_bls_keys_parses_map() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ed01": "ff00",
                "ed02": None,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    mapping = client.get_sumeragi_bls_keys()

    assert mapping["ed01"] == "ff00"
    assert mapping["ed02"] is None
    assert session.calls[0]["url"].endswith("/v1/sumeragi/bls_keys")


def test_get_sumeragi_evidence_count_returns_int() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"count": 42}))
    client = ToriiClient("http://node.test", session=session)

    count = client.get_sumeragi_evidence_count()

    assert count == 42
    assert session.calls[0]["url"].endswith("/v1/sumeragi/evidence/count")


def test_list_sumeragi_evidence_parses_records() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 3,
                "items": [
                    {
                        "kind": "DoublePrepare",
                        "recorded_height": 1,
                        "recorded_view": 2,
                        "recorded_ms": 3,
                        "phase": "Prepare",
                        "height": 4,
                        "view": 5,
                        "epoch": 6,
                        "signer": "ed011122",
                        "block_hash_1": "aa11",
                        "block_hash_2": "bb22",
                    },
                    {
                        "kind": "InvalidProposal",
                        "recorded_height": 7,
                        "recorded_view": 8,
                        "recorded_ms": 9,
                        "height": 10,
                        "view": 11,
                        "epoch": 12,
                        "subject_block_hash": "cc33",
                        "payload_hash": "dd44",
                        "reason": "payload mismatch",
                    },
                    {
                        "kind": "UnknownEvidence",
                        "recorded_height": 13,
                        "recorded_view": 14,
                        "recorded_ms": 15,
                        "detail": "unknown entry",
                    },
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_sumeragi_evidence(limit=5, offset=1, kind="DoublePrepare")

    assert page.total == 3
    assert len(page.items) == 3
    prevote = page.items[0]
    assert prevote.kind == "DoublePrepare"
    assert prevote.phase == "Prepare"
    assert prevote.block_hash_1 == "aa11"
    assert prevote.block_hash_2 == "bb22"
    invalid_proposal = page.items[1]
    assert invalid_proposal.payload_hash == "dd44"
    assert invalid_proposal.reason == "payload mismatch"
    fallback = page.items[2]
    assert fallback.kind == "UnknownEvidence"
    assert fallback.detail == "unknown entry"
    call = session.calls[0]
    assert call["url"].endswith("/v1/sumeragi/evidence")
    assert call["params"] == {"limit": 5, "offset": 1, "kind": "DoublePrepare"}


def test_list_sumeragi_evidence_validates_limit() -> None:
    client = ToriiClient("http://node.test")

    try:
        client.list_sumeragi_evidence(limit=2000)
    except RuntimeError as exc:
        assert "limit must be <= 1000" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for oversized limit")


def test_set_confidential_gas_schedule_reuses_logger() -> None:
    session = RecordingSession()
    config_payload = {
        "public_key": "ed0123",
        "logger": {"level": "Info", "filter": "mod=warn"},
        "network": {
            "block_gossip_size": 32,
            "block_gossip_period_ms": 100,
            "transaction_gossip_size": 16,
            "transaction_gossip_period_ms": 50,
        },
        "queue": {"capacity": 2048},
        "confidential_gas": {
            "proof_base": 1,
            "per_public_input": 1,
            "per_proof_byte": 1,
            "per_nullifier": 1,
            "per_commitment": 1,
        },
    }
    session.queue(StubResponse(payload=config_payload))
    session.queue(StubResponse(status_code=202))
    client = ToriiClient("http://node.test", session=session)

    client.set_confidential_gas_schedule(
        proof_base=9,
        per_public_input=8,
        per_proof_byte=7,
        per_nullifier=6,
        per_commitment=5,
    )

    assert len(session.calls) == 2
    assert session.calls[1]["method"] == "POST"
    body = json.loads(session.calls[1]["data"])
    assert body["logger"] == {"level": "Info", "filter": "mod=warn"}
    assert body["confidential_gas"]["per_nullifier"] == 6


def test_get_time_now_parses_snapshot_alt_values() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "now": 123456789,
                "offset_ms": -5,
                "confidence_ms": 42,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_time_now()

    assert snapshot.now_ms == 123456789
    assert snapshot.offset_ms == -5
    assert snapshot.confidence_ms == 42


def test_get_time_status_parses_diagnostics() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "peers": 2,
                "samples": [
                    {"peer": "peer-a", "last_offset_ms": 1, "last_rtt_ms": 10, "count": 5},
                    {"peer": "peer-b", "last_offset_ms": -2, "last_rtt_ms": 15, "count": 7},
                ],
                "rtt": {
                    "buckets": [{"le": 25, "count": 3}, {"le": 50, "count": 4}],
                    "sum_ms": 28,
                    "count": 9,
                },
                "note": "NTS running",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    status = client.get_time_status()

    assert status.peers == 2
    assert len(status.samples) == 2
    assert status.samples[0].peer == "peer-a"
    assert status.rtt_buckets[1].upper_bound_ms == 50
    assert status.rtt_sum_ms == 28
    assert status.note == "NTS running"


def test_get_sumeragi_rbc_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "sessions_active": 2,
                "sessions_pruned_total": 7,
                "ready_broadcasts_total": 11,
                "deliver_broadcasts_total": 13,
                "payload_bytes_delivered_total": 1024,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_sumeragi_rbc()

    assert snapshot.sessions_active == 2
    assert snapshot.payload_bytes_delivered_total == 1024
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"].endswith("/v1/sumeragi/rbc")


def test_get_sumeragi_rbc_sessions_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "sessions_active": 1,
                "items": [
                    {
                        "block_hash": "AA55",
                        "height": 42,
                        "view": 3,
                        "total_chunks": 8,
                        "received_chunks": 4,
                        "ready_count": 2,
                        "delivered": True,
                        "invalid": False,
                        "payload_hash": "FF",
                        "recovered": False,
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    sessions = client.get_sumeragi_rbc_sessions()

    assert sessions.sessions_active == 1
    assert len(sessions.items) == 1
    assert sessions.items[0].block_hash == "AA55"
    assert sessions.items[0].delivered is True
    assert session.calls[0]["url"].endswith("/v1/sumeragi/rbc/sessions")


def test_get_sumeragi_rbc_delivered_flow() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=404))
    session.queue(
        StubResponse(
            payload={
                "height": 5,
                "view": 1,
                "delivered": True,
                "present": True,
                "block_hash": "DEADBEEF",
                "ready_count": 7,
                "received_chunks": 8,
                "total_chunks": 10,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    assert client.get_sumeragi_rbc_delivered(5, 1) is None
    status = client.get_sumeragi_rbc_delivered(height="5", view="1")

    assert status is not None
    assert status.block_hash == "DEADBEEF"
    assert status.ready_count == 7
    assert session.calls[1]["url"].endswith("/v1/sumeragi/rbc/delivered/5/1")


def test_sample_rbc_chunks_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "block_hash": "AA",
                "height": 9,
                "view": 0,
                "total_chunks": 16,
                "chunk_root": "BB",
                "payload_hash": None,
                "samples": [
                    {
                        "index": 0,
                        "chunk_hex": "CC",
                        "digest_hex": "DD",
                        "proof": {
                            "leaf_index": 0,
                            "depth": 2,
                            "audit_path": ["11", None],
                        },
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    sample = client.sample_rbc_chunks(
        block_hash="AA",
        height=9,
        view=0,
        count=2,
        seed="10",
        api_token="secret-token",
    )

    assert sample is not None
    assert sample.block_hash == "AA"
    assert sample.samples[0].proof.audit_path[0] == "11"

    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/sumeragi/rbc/sample")
    assert call["headers"]["X-API-Token"] == "secret-token"
    assert json.loads(call["data"]) == {
        "block_hash": "AA",
        "height": 9,
        "view": 0,
        "count": 2,
        "seed": 10,
    }


def test_sample_rbc_chunks_requires_token() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=401))
    client = ToriiClient("http://node.test", session=session)

    try:
        client.sample_rbc_chunks(block_hash="AA", height=1, view=0)
    except RuntimeError as exc:
        assert "requires a valid X-API-Token" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for missing RBC token")


def test_list_kaigi_relays_parses_summary() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 1,
                "items": [
                    {
                        "relay_id": "relay-alpha",
                        "domain": "kaigi.core",
                        "bandwidth_class": 3,
                        "hpke_fingerprint_hex": "ab" * 32,
                        "status": "healthy",
                        "reported_at_ms": 123,
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    summary = client.list_kaigi_relays()

    assert summary.total == 1
    assert len(summary.items) == 1
    relay = summary.items[0]
    assert relay.relay_id == "relay-alpha"
    assert relay.status == "healthy"
    assert session.calls[0]["url"].endswith("/v1/kaigi/relays")
    assert session.calls[0]["headers"]["Accept"] == "application/json"


def test_get_kaigi_relay_returns_detail_and_none_on_404() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=404))
    session.queue(
        StubResponse(
            payload={
                "relay": {
                    "relay_id": "relay-alpha",
                    "domain": "kaigi.core",
                    "bandwidth_class": 3,
                    "hpke_fingerprint_hex": "cd" * 32,
                },
                "hpke_public_key_b64": "QUJDRA==",
                "reported_call": {"domain_id": "kaigi.core", "call_name": "register"},
                "reported_by": "ops@example",
                "notes": "Primary relay",
                "metrics": {
                    "domain": "kaigi.core",
                    "registrations_total": 5,
                    "manifest_updates_total": 7,
                    "failovers_total": 1,
                    "health_reports_total": 9,
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    assert client.get_kaigi_relay("relay-alpha") is None
    detail = client.get_kaigi_relay("relay-alpha")

    assert detail is not None
    assert detail.relay.domain == "kaigi.core"
    assert detail.metrics is not None and detail.metrics.failovers_total == 1
    assert detail.reported_call is not None
    assert detail.reported_call.call_name == "register"
    assert session.calls[1]["url"].endswith("/v1/kaigi/relays/relay-alpha")


def test_get_kaigi_relays_health_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "healthy_total": 2,
                "degraded_total": 1,
                "unavailable_total": 0,
                "reports_total": 5,
                "registrations_total": 7,
                "failovers_total": 1,
                "domains": [
                    {
                        "domain": "kaigi.core",
                        "registrations_total": 5,
                        "manifest_updates_total": 3,
                        "failovers_total": 1,
                        "health_reports_total": 4,
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_kaigi_relays_health()

    assert snapshot.healthy_total == 2
    assert snapshot.domains[0].domain == "kaigi.core"
    assert session.calls[0]["url"].endswith("/v1/kaigi/relays/health")


def test_finalize_referendum_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [
                    {"wire_id": "FinalizeReferendum", "payload_hex": "AA"}
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    draft = client.finalize_referendum(referendum_id="ref-1", proposal_id="a" * 64)

    assert draft.ok is True
    assert len(draft.tx_instructions) == 1
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/gov/finalize")
    assert json.loads(call["data"]) == {
        "referendum_id": "ref-1",
        "proposal_id": "a" * 64,
    }


def test_enact_proposal_supports_preimage_and_window() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "tx_instructions": [{"wire_id": "EnactReferendum", "payload_hex": "BB"}],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    draft = client.enact_proposal(
        proposal_id="b" * 64,
        preimage_hash="c" * 64,
        window=(10, 20),
    )

    assert draft.ok is True
    assert draft.tx_instructions[0].wire_id == "EnactReferendum"
    call = session.calls[0]
    assert call["url"].endswith("/v1/gov/enact")
    assert json.loads(call["data"]) == {
        "proposal_id": "b" * 64,
        "preimage_hash": "c" * 64,
        "window": {"lower": 10, "upper": 20},
    }


def test_get_connect_status_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "enabled": True,
                "sessions_total": 5,
                "sessions_active": 3,
                "per_ip_sessions": [{"ip": "192.0.2.1", "sessions": 2}],
                "buffered_sessions": 1,
                "total_buffer_bytes": 42,
                "dedupe_size": 7,
                "frames_in_total": 10,
                "frames_out_total": 11,
                "ciphertext_total": 12,
                "dedupe_drops_total": 0,
                "buffer_drops_total": 0,
                "plaintext_control_drops_total": 0,
                "monotonic_drops_total": 0,
                "ping_miss_total": 0,
                "policy": {
                    "relay_enabled": True,
                    "ws_max_sessions": 32,
                    "session_ttl_ms": 10000,
                    "heartbeat_interval_ms": 5000,
                    "heartbeat_miss_tolerance": 3,
                    "heartbeat_min_interval_ms": 1000,
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_connect_status()

    assert snapshot.enabled is True
    assert snapshot.sessions_total == 5
    assert snapshot.per_ip_sessions[0].ip == "192.0.2.1"
    assert snapshot.policy is not None
    assert snapshot.policy.ws_max_sessions == 32
    assert snapshot.policy.heartbeat_interval_ms == 5000
    assert session.calls[0]["url"].endswith("/v1/connect/status")


def test_create_and_delete_connect_session() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "sid": "abc",
                "wallet_uri": "iroha://wallet",
                "app_uri": "iroha://app",
                "token_app": "app-token",
                "token_wallet": "wallet-token",
                "ttl": 30,
            }
        )
    )
    session.queue(StubResponse(status_code=204))
    client = ToriiClient("http://node.test", session=session)

    session_info = client.create_connect_session({"scope": "demo"})
    deleted = client.delete_connect_session("abc")

    assert session_info.sid == "abc"
    assert session_info.extra["ttl"] == 30
    assert deleted is True
    post_call = session.calls[0]
    assert post_call["method"] == "POST"
    assert post_call["url"].endswith("/v1/connect/session")
    assert json.loads(post_call["data"]) == {"scope": "demo"}


def test_connect_app_registry_and_policy_helpers() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "app_id": "demo.wallet",
                        "display_name": "Demo Wallet",
                        "namespaces": ["wallets"],
                        "metadata": {"category": "wallet"},
                        "policy": {"relay_enabled": True},
                    }
                ],
                "total": 1,
                "next_cursor": "cursor-1",
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "policy": {
                    "relay_enabled": False,
                    "ws_max_sessions": 16,
                    "heartbeat_interval_ms": 15000,
                }
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "policy": {
                    "relay_enabled": True,
                    "heartbeat_interval_ms": 12000,
                }
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_connect_apps(limit=5, cursor="start")
    assert page.total == 1
    assert page.items[0].app_id == "demo.wallet"
    assert page.next_cursor == "cursor-1"

    policy = client.get_connect_app_policy()
    assert policy.relay_enabled is False
    assert policy.heartbeat_interval_ms == 15000

    updated = client.update_connect_app_policy({"relay_enabled": True, "heartbeat_interval_ms": 12000})
    assert updated.relay_enabled is True
    assert updated.heartbeat_interval_ms == 12000
    assert json.loads(session.calls[2]["data"]) == {
        "relay_enabled": True,
        "heartbeat_interval_ms": 12000,
    }


def test_iterate_connect_apps_pages_and_limit() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {"app_id": "demo.wallet", "namespaces": [], "metadata": {}, "policy": {}},
                    {"app_id": "demo.market", "namespaces": [], "metadata": {}, "policy": {}},
                ],
                "next_cursor": "c2",
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {"app_id": "demo.bridge", "namespaces": [], "metadata": {}, "policy": {}},
                ],
                "next_cursor": None,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    apps = list(client.iterate_connect_apps(limit=2))

    assert [app.app_id for app in apps] == ["demo.wallet", "demo.market"]
    # Only the first page is fetched because limit was satisfied.
    assert len(session.calls) == 1
    assert session.calls[0]["params"]["limit"] == 2


def test_iterate_connect_apps_consumes_all_pages_when_unbounded() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {"app_id": "app-1", "namespaces": [], "metadata": {}, "policy": {}},
                ],
                "next_cursor": "c2",
            }
        )
    )
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {"app_id": "app-2", "namespaces": [], "metadata": {}, "policy": {}},
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    apps = list(client.iterate_connect_apps(page_size=1))

    assert [app.app_id for app in apps] == ["app-1", "app-2"]
    assert len(session.calls) == 2
    assert session.calls[0]["params"]["limit"] == 1
    assert session.calls[1]["params"]["cursor"] == "c2"
def test_connect_admission_manifest_helpers() -> None:
    session = RecordingSession()
    manifest_payload = {
        "version": 2,
        "manifest_hash": "abcd",
        "entries": [
            {
                "app_id": "demo.wallet",
                "namespaces": ["wallets"],
                "metadata": {"region": "global"},
                "policy": {"relay_enabled": True},
            }
        ],
    }
    session.queue(StubResponse(payload=manifest_payload))
    session.queue(StubResponse(payload=manifest_payload))
    client = ToriiClient("http://node.test", session=session)

    manifest = client.get_connect_admission_manifest()
    assert manifest.version == 2
    assert manifest.entries[0].namespaces == ["wallets"]

    updated = client.set_connect_admission_manifest(manifest_payload)
    assert updated.manifest_hash == "abcd"
    put_call = session.calls[1]
    assert put_call["method"] == "PUT"
    assert json.loads(put_call["data"]) == manifest_payload


def test_trigger_listing_and_lookup_roundtrip() -> None:
    session = RecordingSession()
    list_payload: Dict[str, Any] = {
        "items": [
            {
                "id": "daily-airdrop",
                "action": {"Mint": {"params": {"asset_id": CANONICAL_ASSET_ID}}},
                "metadata": {"cron": "0 0 * * *"},
            }
        ],
        "total": 1,
    }
    session.queue(StubResponse(payload=list_payload))
    session.queue(StubResponse(payload=list_payload["items"][0]))
    session.queue(StubResponse(status_code=404))
    client = ToriiClient("http://node.test", session=session)

    page = client.list_triggers(namespace="core", authority=CANONICAL_OWNER, limit=5, offset=10)
    trigger = client.get_trigger("daily-airdrop")
    missing = client.get_trigger("unknown-trigger")

    assert page.total == 1
    assert page.items[0].id == "daily-airdrop"
    assert trigger is not None and trigger.metadata["cron"] == "0 0 * * *"
    assert missing is None

    assert session.calls[0]["params"] == {
        "namespace": "core",
        "authority": CANONICAL_OWNER,
        "limit": 5,
        "offset": 10,
    }
    assert session.calls[0]["url"].endswith("/v1/triggers")
    assert session.calls[1]["url"].endswith("/v1/triggers/daily-airdrop")
    assert session.calls[2]["url"].endswith("/v1/triggers/unknown-trigger")


def test_trigger_registration_deletion_and_query() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=201, payload={"ok": True}))
    session.queue(StubResponse(status_code=204))
    session.queue(StubResponse(status_code=404))
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "id": "hook",
                        "action": {"Grant": {"params": {}}},
                        "metadata": {},
                    }
                ],
                "total": 1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.register_trigger({"id": "hook", "action": {"Grant": {}}})
    deleted = client.delete_trigger("hook")
    deleted_missing = client.delete_trigger("missing")
    page = client.query_triggers(filter={"id": {"$eq": "hook"}}, fetch_size=1, query_name="named_query")

    assert result["ok"] is True
    assert deleted is True
    assert deleted_missing is False
    assert page.total == 1
    assert page.items[0].id == "hook"

    post_call = session.calls[0]
    assert post_call["method"] == "POST"
    assert post_call["url"].endswith("/v1/triggers")
    assert json.loads(post_call["data"].decode("utf-8")) == {"id": "hook", "action": {"Grant": {}}}

    query_call = session.calls[-1]
    assert query_call["url"].endswith("/v1/triggers/query")
    assert json.loads(query_call["data"].decode("utf-8")) == {
        "filter": {"id": {"$eq": "hook"}},
        "fetch_size": 1,
        "query_name": "named_query",
    }


def test_list_offline_allowances_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 1,
                "items": [
                    {
                        "certificate_id_hex": "cafebabe",
                        "controller_id": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                        "controller_display": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                        "asset_id": CANONICAL_ASSET_ID,
                        "asset_definition_id": "usd#wonderland",
                        "asset_definition_name": "USD",
                        "asset_definition_alias": None,
                        "registered_at_ms": 10,
                        "expires_at_ms": 20,
                        "policy_expires_at_ms": 30,
                        "refresh_at_ms": 40,
                        "verdict_id_hex": "deadbeef",
                        "attestation_nonce_hex": "feedface",
                        "remaining_amount": "13",
                        "deadline_kind": "policy",
                        "deadline_state": "warning",
                        "deadline_ms": 99,
                        "deadline_ms_remaining": -5,
                        "record": {"certificate": {"controller": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"}},
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_offline_allowances(controller_id="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL", include_expired=True)

    assert page.total == 1
    assert len(page.items) == 1
    item = page.items[0]
    assert item.certificate_id_hex == "cafebabe"
    assert item.asset_definition_id == "usd#wonderland"
    assert item.asset_definition_name == "USD"
    assert item.asset_definition_alias is None
    assert item.deadline is not None
    assert item.deadline.kind == "policy"
    assert item.deadline.ms_remaining == -5
    assert item.record["certificate"]["controller"] == "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"
    assert session.calls[0]["params"] == {
        "controller_id": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        "include_expired": True,
    }


def test_query_offline_allowances_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"items": [], "total": 0}))
    client = ToriiClient("http://node.test", session=session)

    page = client.query_offline_allowances({"filter": {"controller_id": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"}})

    assert page.total == 0
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/offline/allowances/query")
    assert call["headers"]["Content-Type"] == "application/json"
    assert json.loads(call["data"].decode("utf-8")) == {
        "filter": {"controller_id": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"}
    }


def test_issue_offline_certificate_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "certificate_id_hex": "deadbeef",
                "certificate": {
                    "controller": CANONICAL_OWNER,
                    "allowance": {"asset": "rose#wonderland", "amount": "10", "commitment": [1, 2]},
                    "spend_public_key": "ed0120deadbeef",
                    "attestation_report": [3, 4],
                    "issued_at_ms": 100,
                    "expires_at_ms": 200,
                    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
                    "operator_signature": "AA",
                    "metadata": {},
                    "verdict_id": None,
                    "attestation_nonce": None,
                    "refresh_at_ms": None,
                },
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)
    draft = {
        "controller": CANONICAL_OWNER,
        "allowance": {"asset": "rose#wonderland", "amount": "10", "commitment": [1, 2]},
        "spend_public_key": "ed0120deadbeef",
        "attestation_report": [3, 4],
        "issued_at_ms": 100,
        "expires_at_ms": 200,
        "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
        "metadata": {},
        "verdict_id": None,
        "attestation_nonce": None,
        "refresh_at_ms": None,
    }

    response = client.issue_offline_certificate(draft)

    assert response.certificate_id_hex == "deadbeef"
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/offline/certificates/issue")
    assert json.loads(call["data"].decode("utf-8")) == {"certificate": draft}


def test_register_offline_allowance_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"certificate_id_hex": "deadbeef"}))
    client = ToriiClient("http://node.test", session=session)
    certificate = {
        "controller": CANONICAL_OWNER,
        "allowance": {"asset": "rose#wonderland", "amount": "10", "commitment": [1, 2]},
        "spend_public_key": "ed0120deadbeef",
        "attestation_report": [3, 4],
        "issued_at_ms": 100,
        "expires_at_ms": 200,
        "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
        "operator_signature": "AA",
        "metadata": {},
        "verdict_id": None,
        "attestation_nonce": None,
        "refresh_at_ms": None,
    }

    response = client.register_offline_allowance(
        certificate=certificate,
        authority=CANONICAL_OWNER,
        private_key="deadbeef",
    )

    assert response.certificate_id_hex == "deadbeef"
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/offline/allowances")
    assert json.loads(call["data"].decode("utf-8")) == {
        "authority": CANONICAL_OWNER,
        "private_key": "deadbeef",
        "certificate": certificate,
    }


def test_renew_offline_allowance_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"certificate_id_hex": "deadbeef"}))
    client = ToriiClient("http://node.test", session=session)
    certificate = {
        "controller": CANONICAL_OWNER,
        "allowance": {"asset": "rose#wonderland", "amount": "10", "commitment": [1, 2]},
        "spend_public_key": "ed0120deadbeef",
        "attestation_report": [3, 4],
        "issued_at_ms": 100,
        "expires_at_ms": 200,
        "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
        "operator_signature": "AA",
        "metadata": {},
        "verdict_id": None,
        "attestation_nonce": None,
        "refresh_at_ms": None,
    }

    client.renew_offline_allowance(
        certificate_id_hex="deadbeef",
        certificate=certificate,
        authority=CANONICAL_OWNER,
        private_key="deadbeef",
    )

    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/offline/allowances/deadbeef/renew")
    assert json.loads(call["data"].decode("utf-8")) == {
        "authority": CANONICAL_OWNER,
        "private_key": "deadbeef",
        "certificate": certificate,
    }


def test_top_up_offline_allowance_chains_issue_and_register() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "certificate_id_hex": "deadbeef",
                "certificate": {
                    "controller": CANONICAL_OWNER,
                    "allowance": {"asset": "rose#wonderland", "amount": "10", "commitment": [1, 2]},
                    "spend_public_key": "ed0120deadbeef",
                    "attestation_report": [3, 4],
                    "issued_at_ms": 100,
                    "expires_at_ms": 200,
                    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
                    "operator_signature": "AA",
                    "metadata": {},
                    "verdict_id": None,
                    "attestation_nonce": None,
                    "refresh_at_ms": None,
                },
            }
        )
    )
    session.queue(StubResponse(payload={"certificate_id_hex": "deadbeef"}))
    client = ToriiClient("http://node.test", session=session)
    draft = {
        "controller": CANONICAL_OWNER,
        "allowance": {"asset": "rose#wonderland", "amount": "10", "commitment": [1, 2]},
        "spend_public_key": "ed0120deadbeef",
        "attestation_report": [3, 4],
        "issued_at_ms": 100,
        "expires_at_ms": 200,
        "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
        "metadata": {},
        "verdict_id": None,
        "attestation_nonce": None,
        "refresh_at_ms": None,
    }

    response = client.top_up_offline_allowance(
        certificate=draft,
        authority=CANONICAL_OWNER,
        private_key="deadbeef",
    )

    assert response.certificate.certificate_id_hex == "deadbeef"
    assert response.registration.certificate_id_hex == "deadbeef"
    assert len(session.calls) == 2
    assert session.calls[0]["url"].endswith("/v1/offline/certificates/issue")
    assert session.calls[1]["url"].endswith("/v1/offline/allowances")
    register_body = json.loads(session.calls[1]["data"].decode("utf-8"))
    assert register_body["certificate"]["operator_signature"] == "AA"


def test_top_up_offline_allowance_renewal_chains_issue_and_register() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "certificate_id_hex": "deadbeef",
                "certificate": {
                    "controller": CANONICAL_OWNER,
                    "allowance": {"asset": "rose#wonderland", "amount": "10", "commitment": [1, 2]},
                    "spend_public_key": "ed0120deadbeef",
                    "attestation_report": [3, 4],
                    "issued_at_ms": 100,
                    "expires_at_ms": 200,
                    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
                    "operator_signature": "AA",
                    "metadata": {},
                    "verdict_id": None,
                    "attestation_nonce": None,
                    "refresh_at_ms": None,
                },
            }
        )
    )
    session.queue(StubResponse(payload={"certificate_id_hex": "deadbeef"}))
    client = ToriiClient("http://node.test", session=session)
    draft = {
        "controller": CANONICAL_OWNER,
        "allowance": {"asset": "rose#wonderland", "amount": "10", "commitment": [1, 2]},
        "spend_public_key": "ed0120deadbeef",
        "attestation_report": [3, 4],
        "issued_at_ms": 100,
        "expires_at_ms": 200,
        "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
        "metadata": {},
        "verdict_id": None,
        "attestation_nonce": None,
        "refresh_at_ms": None,
    }

    client.top_up_offline_allowance_renewal(
        certificate_id_hex="deadbeef",
        certificate=draft,
        authority=CANONICAL_OWNER,
        private_key="deadbeef",
    )

    assert len(session.calls) == 2
    assert session.calls[0]["url"].endswith("/v1/offline/certificates/deadbeef/renew/issue")
    assert session.calls[1]["url"].endswith("/v1/offline/allowances/deadbeef/renew")


def test_list_offline_transfers_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 1,
                "items": [
                    {
                        "bundle_id_hex": "bead",
                        "controller_id": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                        "controller_display": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                        "receiver_id": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
                        "receiver_display": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
                        "deposit_account_id": "3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF",
                        "deposit_account_display": "3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF",
                        "asset_id": CANONICAL_ASSET_ID,
                        "certificate_id_hex": "cafebabe",
                        "certificate_expires_at_ms": 100,
                        "policy_expires_at_ms": 200,
                        "refresh_at_ms": 300,
                        "verdict_id_hex": "deadbeef",
                        "attestation_nonce_hex": "feedface",
                        "receipt_count": 2,
                        "total_amount": "11",
                        "status": "settled",
                        "recorded_at_ms": 400,
                        "recorded_at_height": 500,
                        "archived_at_height": 600,
                        "status_transitions": [
                            {
                                "status": "settled",
                                "transitioned_at_ms": 410,
                                "verdict_snapshot": {"certificate_id": "0x01"},
                            }
                        ],
                        "claimed_delta": "5",
                        "platform_policy": "play_integrity",
                        "platform_token_snapshot": {
                            "policy": "play_integrity",
                            "attestation_jws_b64": "token",
                        },
                        "verdict_snapshot": {"certificate_id": "0x01"},
                        "transfer": {"bundle_id": "bead"},
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_offline_transfers(limit=1)

    assert page.total == 1
    item = page.items[0]
    assert item.bundle_id_hex == "bead"
    assert item.status_transitions[0].status == "settled"
    assert item.platform_policy == "play_integrity"
    assert item.transfer["bundle_id"] == "bead"
    assert session.calls[0]["params"] == {"limit": 1}


def test_query_offline_transfers_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"items": [], "total": 0}))
    client = ToriiClient("http://node.test", session=session)

    page = client.query_offline_transfers({"filter": {"status": "settled"}})

    assert page.total == 0
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/offline/transfers/query")
    assert call["headers"]["Content-Type"] == "application/json"
    assert json.loads(call["data"].decode("utf-8")) == {"filter": {"status": "settled"}}


def test_list_offline_summaries_parses_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 1,
                "items": [
                    {
                        "certificate_id_hex": "c0de",
                        "controller_id": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                        "controller_display": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                        "summary_hash_hex": "f00d",
                        "apple_key_counters": {"k1": 1},
                        "android_series_counters": {"p0": 2},
                    }
                ],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_offline_summaries()

    assert page.total == 1
    assert page.items[0].apple_key_counters == {"k1": 1}
    assert page.items[0].android_series_counters["p0"] == 2


def test_query_offline_summaries_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"items": [], "total": 0}))
    client = ToriiClient("http://node.test", session=session)

    page = client.query_offline_summaries({"filter": {"certificate_id_hex": "c0de"}})

    assert page.total == 0
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/offline/summaries/query")
    assert json.loads(call["data"].decode("utf-8")) == {
        "filter": {"certificate_id_hex": "c0de"}
    }


def test_status_snapshot_parses_mode_and_consensus_caps() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "mode_tag": "iroha2-consensus::permissioned-sumeragi@v1",
                "staged_mode_tag": "iroha2-consensus::npos-sumeragi@v1",
                "staged_mode_activation_height": 10,
                "mode_activation_lag_blocks": 2,
                "consensus_caps": {
                    "collectors_k": 2,
                    "redundant_send_r": 1,
                    "da_enabled": True,
                    "rbc_chunk_max_bytes": 1024,
                    "rbc_session_ttl_ms": 5000,
                    "rbc_store_max_sessions": 64,
                    "rbc_store_soft_sessions": 32,
                    "rbc_store_max_bytes": 4096,
                    "rbc_store_soft_bytes": 2048,
                },
                "peers": 1,
                "queue_size": 2,
                "commit_time_ms": 3,
                "da_reschedule_total": 4,
                "txs_approved": 5,
                "txs_rejected": 6,
                "view_changes": 7,
                "lane_commitments": [],
                "dataspace_commitments": [],
                "lane_governance": [],
                "lane_governance_sealed_total": 0,
                "lane_governance_sealed_aliases": [],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    snapshot = client.get_status_snapshot()

    assert snapshot.status.mode_tag == "iroha2-consensus::permissioned-sumeragi@v1"
    assert snapshot.status.staged_mode_tag == "iroha2-consensus::npos-sumeragi@v1"
    assert snapshot.status.staged_mode_activation_height == 10
    assert snapshot.status.mode_activation_lag_blocks == 2
    assert snapshot.status.consensus_caps is not None
    assert snapshot.status.consensus_caps.collectors_k == 2
    assert snapshot.status.consensus_caps.rbc_chunk_max_bytes == 1024


def test_decode_pdp_commitment_header_handles_mapping() -> None:
    payload = b"\x01\x02\x03"
    header_value = base64.b64encode(payload).decode("ascii")

    decoded = decode_pdp_commitment_header({"sora-pdp-commitment": header_value})

    assert decoded == payload


def test_decode_pdp_commitment_header_is_case_insensitive() -> None:
    payload = b"\xAA\xBB"
    header_value = base64.b64encode(payload).decode("ascii")

    decoded = decode_pdp_commitment_header({"Sora-PDP-Commitment": header_value})

    assert decoded == payload


def test_decode_pdp_commitment_header_rejects_invalid_payload() -> None:
    try:
        decode_pdp_commitment_header({"sora-pdp-commitment": "###"})
    except RuntimeError as exc:
        assert "Failed to decode" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for invalid header")


def test_decode_pdp_commitment_header_returns_none_when_missing() -> None:
    assert decode_pdp_commitment_header({}) is None
    assert decode_pdp_commitment_header(None) is None


def test_submit_zk_ballot_rejects_deprecated_public_inputs() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "accepted": True,
                "reason": None,
                "tx_instructions": [],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="durationBlocks"):
        client.submit_zk_ballot(
            authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
            chain_id="chain",
            election_id="election-1",
            proof_b64="AAAA",
            public={
                "owner": CANONICAL_OWNER,
                "amount": "100",
                "durationBlocks": 5,
            },
        )


def test_submit_zk_ballot_normalizes_public_inputs() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "accepted": True,
                "reason": None,
                "tx_instructions": [],
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    client.submit_zk_ballot(
        authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        chain_id="chain",
        election_id="election-1",
        proof_b64="AAAA",
        public={
            "owner": CANONICAL_OWNER,
            "amount": "100",
            "duration_blocks": 5,
            "root_hint": f"0x{'Cc' * 32}",
            "nullifier": bytes.fromhex("DD" * 32),
        },
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    public = payload["public"]
    assert public["root_hint"] == "cc" * 32
    assert public["nullifier"] == "dd" * 32


def test_submit_zk_ballot_rejects_invalid_hex_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="root_hint"):
        client.submit_zk_ballot(
            authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
            chain_id="chain",
            election_id="election-1",
            proof_b64="AAAA",
            public={
                "owner": CANONICAL_OWNER,
                "amount": "100",
                "duration_blocks": 5,
                "root_hint": "not-hex",
            },
        )


def test_submit_zk_ballot_rejects_incomplete_lock_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="owner, amount, duration_blocks"):
        client.submit_zk_ballot(
            authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
            chain_id="chain",
            election_id="election-1",
            proof_b64="AAAA",
            public={"owner": CANONICAL_OWNER},
        )


def test_submit_zk_ballot_rejects_noncanonical_owner() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="canonical account id"):
        client.submit_zk_ballot(
            authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
            chain_id="chain",
            election_id="election-1",
            proof_b64="AAAA",
            public={
                "owner": "soradead",
                "amount": "100",
                "duration_blocks": 5,
            },
        )


def test_submit_zk_ballot_v1_rejects_incomplete_lock_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="owner, amount, duration_blocks"):
        client.submit_zk_ballot_v1(
            authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
            chain_id="chain",
            election_id="election-1",
            backend="halo2/ipa",
            envelope_b64="AAAA",
            owner=CANONICAL_OWNER,
        )


def test_submit_zk_ballot_v1_rejects_noncanonical_owner() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="canonical account id"):
        client.submit_zk_ballot_v1(
            authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
            chain_id="chain",
            election_id="election-1",
            backend="halo2/ipa",
            envelope_b64="AAAA",
            owner="soradead",
            amount="100",
            duration_blocks=5,
        )


def test_submit_zk_ballot_v1_normalizes_hex_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    client.submit_zk_ballot_v1(
        authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        chain_id="chain",
        election_id="election-1",
        backend="halo2/ipa",
        envelope_b64="AAAA",
        root_hint=f"0x{'Aa' * 32}",
        nullifier=f"blake2b32:{'BB' * 32}",
    )

    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["root_hint"] == "aa" * 32
    assert payload["nullifier"] == "bb" * 32


def test_submit_zk_ballot_v1_rejects_invalid_hex_hints() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True}))
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(RuntimeError, match="root_hint"):
        client.submit_zk_ballot_v1(
            authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
            chain_id="chain",
            election_id="election-1",
            backend="halo2/ipa",
            envelope_b64="AAAA",
            root_hint="not-hex",
        )


def test_list_subscription_plans_encodes_params() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "plan_id": "plan#subs",
                        "plan": {"provider": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL", "pricing": {"kind": "fixed"}},
                    }
                ],
                "total": 1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_subscription_plans(provider="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL", limit=10, offset=5)

    assert page.total == 1
    assert page.items[0].plan_id == "plan#subs"
    assert page.items[0].plan["provider"] == "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"
    assert session.calls[0]["params"] == {
        "provider": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        "limit": 10,
        "offset": 5,
    }


def test_create_subscription_plan_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "plan_id": "plan#subs",
                "tx_hash_hex": "deadbeef",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.create_subscription_plan(
        authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        private_key="ed25519:priv",
        plan_id="plan#subs",
        plan={"provider": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"},
    )

    assert result.ok is True
    assert result.plan_id == "plan#subs"
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["authority"] == "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"
    assert payload["private_key"] == "ed25519:priv"
    assert payload["plan_id"] == "plan#subs"
    assert payload["plan"]["provider"] == "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"


def test_list_subscriptions_encodes_params() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "items": [
                    {
                        "subscription_id": "sub-1$subscriptions",
                        "subscription": {"status": "active"},
                        "invoice": {"amount": "120"},
                        "plan": {"provider": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"},
                    }
                ],
                "total": 1,
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    page = client.list_subscriptions(
        owned_by="34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
        provider="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        status="ACTIVE",
        limit=25,
        offset=0,
    )

    assert page.total == 1
    assert page.items[0].subscription_id == "sub-1$subscriptions"
    assert page.items[0].subscription["status"] == "active"
    assert session.calls[0]["params"] == {
        "owned_by": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
        "provider": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        "status": "active",
        "limit": 25,
        "offset": 0,
    }


def test_list_subscriptions_rejects_invalid_status() -> None:
    client = ToriiClient("http://node.test", session=RecordingSession())

    with pytest.raises(ValueError, match="subscriptions.status"):
        client.list_subscriptions(status="unknown")


def test_create_subscription_posts_payload() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "ok": True,
                "subscription_id": "sub-1$subscriptions",
                "billing_trigger_id": "sub-bill",
                "usage_trigger_id": "sub-usage",
                "first_charge_ms": 1_704_067_200_000,
                "tx_hash_hex": "deadbeef",
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    result = client.create_subscription(
        authority="34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
        private_key="ed25519:priv",
        subscription_id="sub-1$subscriptions",
        plan_id="plan#subs",
        billing_trigger_id="sub-bill",
        usage_trigger_id="sub-usage",
        first_charge_ms=1_704_067_200_000,
        grant_usage_to_provider=True,
    )

    assert result.subscription_id == "sub-1$subscriptions"
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["authority"] == "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r"
    assert payload["private_key"] == "ed25519:priv"
    assert payload["billing_trigger_id"] == "sub-bill"
    assert payload["usage_trigger_id"] == "sub-usage"
    assert payload["first_charge_ms"] == 1_704_067_200_000
    assert payload["grant_usage_to_provider"] is True


def test_get_subscription_encodes_path_and_parses_response() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "subscription_id": "sub-1$subscriptions",
                "subscription": {"status": "active"},
                "plan": {"provider": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL"},
            }
        )
    )
    client = ToriiClient("http://node.test", session=session)

    record = client.get_subscription("sub-1$subscriptions")

    assert record is not None
    assert record.subscription_id == "sub-1$subscriptions"
    assert session.calls[0]["url"].endswith("/v1/subscriptions/sub-1%24subscriptions")


def test_get_subscription_returns_none_on_404() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=404, payload=None))
    client = ToriiClient("http://node.test", session=session)

    assert client.get_subscription("sub-404$subscriptions") is None


def test_subscription_actions_post_payloads() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "a"}))
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "b"}))
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "c"}))
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "d"}))
    client = ToriiClient("http://node.test", session=session)

    client.pause_subscription("sub-1", authority="34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r", private_key="ed25519:priv")
    client.resume_subscription(
        "sub-1",
        authority="34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
        private_key="ed25519:priv",
        charge_at_ms=1_704_067_200_000,
    )
    client.cancel_subscription("sub-1", authority="34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r", private_key="ed25519:priv")
    client.charge_subscription_now(
        "sub-1",
        authority="34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
        private_key="ed25519:priv",
        charge_at_ms=1_704_067_200_000,
    )

    pause_body = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert pause_body["authority"] == "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r"
    resume_body = json.loads(session.calls[1]["data"].decode("utf-8"))
    assert resume_body["charge_at_ms"] == 1_704_067_200_000
    cancel_body = json.loads(session.calls[2]["data"].decode("utf-8"))
    assert cancel_body["private_key"] == "ed25519:priv"
    charge_body = json.loads(session.calls[3]["data"].decode("utf-8"))
    assert charge_body["charge_at_ms"] == 1_704_067_200_000


def test_record_subscription_usage_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload={"ok": True, "subscription_id": "sub-1", "tx_hash_hex": "e"}))
    client = ToriiClient("http://node.test", session=session)

    result = client.record_subscription_usage(
        "sub-1",
        authority="6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
        private_key="ed25519:priv",
        unit_key="compute_ms",
        delta=3600,
        usage_trigger_id="sub-usage",
    )

    assert result.ok is True
    payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert payload["unit_key"] == "compute_ms"
    assert payload["delta"] == "3600"
    assert payload["usage_trigger_id"] == "sub-usage"
