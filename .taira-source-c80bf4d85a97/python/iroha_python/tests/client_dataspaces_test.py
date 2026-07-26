from __future__ import annotations

import base64
import json
from dataclasses import replace
from pathlib import Path
from urllib.parse import urlsplit

import pytest
import requests
from iroha_torii_client.client import canonical_request_signature_message

from iroha_python import DataspaceSpec, ToriiClient, plan_dataspace, write_dataspace_plan
from iroha_python.crypto import Ed25519KeyPair

FEE_PAYMENT = {
    "payer": "authority",
    "value": {"charge_limits": [], "gas_limit": None},
}


class FakeSession:
    def __init__(self, responses: list[requests.Response]):
        self.responses = responses
        self.calls: list[dict[str, object]] = []

    def request(self, method: str, url: str, **kwargs: object) -> requests.Response:
        self.calls.append(
            {
                "method": method,
                "path": urlsplit(url).path,
                "params": kwargs.get("params"),
                "data": kwargs.get("data"),
                "headers": dict(kwargs.get("headers") or {}),
                "timeout": kwargs.get("timeout"),
            }
        )
        if not self.responses:
            raise AssertionError(f"unexpected request {method} {url}")
        response = self.responses.pop(0)
        response.url = url
        return response


def response(status: int, payload: object | None = None) -> requests.Response:
    result = requests.Response()
    result.status_code = status
    if payload is None:
        result._content = b""
    else:
        result._content = json.dumps(payload).encode("utf-8")
        result.headers["Content-Type"] = "application/json"
    return result


def test_plan_dataspace_generates_manifest_catalog_and_summary(tmp_path: Path) -> None:
    spec = DataspaceSpec(
        dataspace_alias="boi",
        dataspace_id=42,
        lane_alias="boi-payments",
        lane_id=7,
        governance_module="parliament",
        settlement_handle="xor_global",
        validators=["validator01"],
        route_instructions=["TransferAsset"],
    )

    plan = plan_dataspace(spec)

    assert plan.manifest["lane"] == "boi-payments"
    assert plan.manifest["quorum"] == 1
    assert 'alias = "boi"' in plan.catalog_snippet
    assert "dataspace:42" not in plan.catalog_snippet
    assert plan.summary["dataspace_id"] == 42

    outputs = write_dataspace_plan(plan, tmp_path)
    assert outputs["manifest"].read_text(encoding="utf-8").startswith("{\n")
    assert outputs["catalog"].read_text(encoding="utf-8") == plan.catalog_snippet


def test_plan_dataspace_escapes_toml_strings() -> None:
    spec = DataspaceSpec(
        dataspace_alias='boi"retail',
        dataspace_id=42,
        lane_alias='lane\none',
        lane_id=7,
        governance_module="parliament",
        settlement_handle="xor_global",
        validators=["validator01"],
        metadata={'owner"team': "payments\nops"},
        route_accounts=['alice"retail'],
        route_instructions=['Transfer"Asset'],
    )

    plan = plan_dataspace(spec)

    assert 'alias = "lane\\none"' in plan.catalog_snippet
    assert 'dataspace = "boi\\"retail"' in plan.catalog_snippet
    assert '"owner\\"team" = "payments\\nops"' in plan.catalog_snippet


@pytest.mark.parametrize(
    ("patch", "match"),
    [
        ({"dataspace_alias": ""}, "dataspace_alias"),
        ({"lane_alias": ""}, "lane_alias"),
        ({"lane_id": -1}, "lane_id"),
        ({"validators": []}, "validators"),
        ({"validators": ["validator 01"]}, "whitespace"),
        ({"validators": ["validator@is"]}, "encoded account identifier"),
        ({"quorum": 0}, "quorum"),
        ({"quorum": 3}, "quorum"),
        ({"dataspace_id": -1}, "dataspace_id"),
        ({"dataspace_hash": "not-hex"}, "dataspace_hash"),
    ],
)
def test_plan_dataspace_rejects_invalid_inputs(
    patch: dict[str, object],
    match: str,
) -> None:
    fields: dict[str, object] = {
        "dataspace_alias": "boi",
        "lane_alias": "boi-payments",
        "lane_id": 7,
        "governance_module": "parliament",
        "settlement_handle": "xor_global",
        "validators": ["validator01", "validator02"],
        "quorum": 2,
    }
    fields.update(patch)
    spec = DataspaceSpec(**fields)

    with pytest.raises(ValueError, match=match):
        plan_dataspace(spec)


@pytest.mark.parametrize(
    "name_patch",
    [
        {"manifest_name": "../escape.json"},
        {"catalog_name": "/tmp/escape.toml"},
        {"summary_name": "nested/summary.json"},
        {"space_directory_name": r"nested\\manifest.to"},
    ],
)
def test_plan_dataspace_rejects_artifact_path_traversal(
    name_patch: dict[str, str],
) -> None:
    spec = DataspaceSpec(
        dataspace_alias="boi",
        dataspace_id=42,
        lane_alias="boi-payments",
        lane_id=7,
        governance_module="parliament",
        settlement_handle="xor_global",
        validators=["validator01"],
        **name_patch,
    )

    with pytest.raises(ValueError, match="file name"):
        plan_dataspace(spec)


def test_write_dataspace_plan_rejects_tampered_artifact_path(
    tmp_path: Path,
) -> None:
    plan = plan_dataspace(
        DataspaceSpec(
            dataspace_alias="boi",
            dataspace_id=42,
            lane_alias="boi-payments",
            lane_id=7,
            governance_module="parliament",
            settlement_handle="xor_global",
            validators=["validator01"],
        )
    )
    tampered = replace(plan, manifest_name="../escape.json")

    with pytest.raises(ValueError, match="file name"):
        write_dataspace_plan(tampered, tmp_path)

    assert not (tmp_path.parent / "escape.json").exists()


def test_client_dataspace_readiness_accepts_lane_governance_status() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    status = {
        "lane_governance": [
            {
                "lane_id": 3,
                "alias": "boi",
                "dataspace_id": 42,
                "manifest_required": True,
                "manifest_ready": True,
            }
        ],
        "lane_governance_sealed_aliases": [],
    }

    assert client.get_dataspace("boi", status=status)["dataspace_id"] == 42
    assert client.dataspace_ready(42, status=status)
    assert client.smoke_dataspace("boi", status=status).ready


def test_client_dataspace_smoke_rejects_missing_or_sealed_status() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)

    missing = client.dataspace_status("boi", status={"lane_governance": []})
    assert missing.found is False
    assert missing.ready is False

    sealed_status = {
        "lane_governance": [
            {
                "lane_id": 1,
                "alias": "boi",
                "dataspace_id": 9,
                "manifest_required": False,
                "manifest_ready": True,
            }
        ],
        "lane_governance_sealed_aliases": ["boi"],
    }
    sealed = client.dataspace_status("boi", status=sealed_status)
    assert sealed.found is True
    assert sealed.ready is False


def test_client_dataspace_smoke_counts_dataspace_catalog_lanes() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    status = {
        "dataspace_catalog": [
            {
                "alias": "universal",
                "dataspace_id": 0,
                "lane_id": 0,
                "manifest_required": False,
            },
            {
                "alias": "is",
                "dataspace_id": 42,
                "lane_id": 1,
                "manifest_required": False,
            },
        ],
    }

    smoke = client.smoke_dataspace("is", expected_lane_count=2, status=status)

    assert smoke.ready
    assert smoke.dataspace_id == 42


def test_client_dataspace_catalog_metadata_survives_backlog_entries() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    status = {
        "dataspace_catalog": [
            {
                "alias": "is",
                "dataspace_id": 6647857470246403404,
                "lane_alias": "is",
                "lane_id": 5,
                "manifest_required": False,
                "manifest_ready": False,
                "sealed": False,
                "storage_profile": "full_replica",
                "visibility": "restricted",
            }
        ],
        "teu_dataspace_backlog": [
            {"alias": "is", "dataspace_id": 6647857470246403404, "lane_id": lane_id}
            for lane_id in range(6)
        ],
    }

    route = client.get_dataspace("is", status=status)
    smoke = client.smoke_dataspace(
        "is",
        expected_dataspace_id=6647857470246403404,
        status=status,
    )

    assert route is not None
    assert route["visibility"] == "restricted"
    assert route["lane_id"] == 5
    assert smoke.ready is True
    assert smoke.lane["visibility"] == "restricted"
    assert smoke.lane["lane_id"] == 5


def test_client_dataspace_smoke_rejects_wrong_expected_dataspace_id() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    status = {
        "dataspace_catalog": [
            {
                "alias": "is",
                "dataspace_id": 41,
                "lane_id": 5,
                "manifest_required": False,
            }
        ],
    }

    with pytest.raises(AssertionError, match="expected 42"):
        client.smoke_dataspace("is", expected_dataspace_id=42, status=status)


def test_get_status_falls_back_to_public_status_endpoint() -> None:
    session = FakeSession(
        [
            response(404),
            response(200, {"dataspace_catalog": [{"alias": "is", "dataspace_id": 42}]}),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    status = client.get_status()

    assert status["dataspace_catalog"][0]["alias"] == "is"
    assert [call["path"] for call in session.calls] == ["/v1/status", "/status"]
    assert session.calls[0]["headers"]["Accept"] == "application/json"
    assert session.calls[1]["headers"]["Accept"] == "application/json"


def test_get_status_does_not_fallback_on_non_404_failure() -> None:
    session = FakeSession([response(500, {"message": "temporary failure"})])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(RuntimeError, match="temporary failure"):
        client.get_status()

    assert [call["path"] for call in session.calls] == ["/v1/status"]


def test_operator_signature_headers_sign_canonical_request() -> None:
    key_pair = Ed25519KeyPair.from_private_key(bytes([7] * 32))

    headers = ToriiClient.build_operator_signature_headers(
        method="POST",
        path="/v1/configuration?b=2&a=1",
        body=b'{"retire":[]}',
        key_pair=key_pair,
        timestamp_ms=123456,
        nonce="nonce-1",
    )

    message = canonical_request_signature_message(
        "POST",
        "/v1/configuration?b=2&a=1",
        b'{"retire":[]}',
        timestamp_ms=123456,
        nonce="nonce-1",
    )
    assert headers["x-iroha-operator-public-key"] == key_pair.public_key_multihash
    assert headers["x-iroha-operator-timestamp-ms"] == "123456"
    assert headers["x-iroha-operator-nonce"] == "nonce-1"
    assert key_pair.verify(
        message,
        base64.b64decode(headers["x-iroha-operator-signature"]),
    )


def test_operator_signature_headers_rejects_ambiguous_or_bad_signers() -> None:
    key_pair = Ed25519KeyPair.from_private_key(bytes([8] * 32))

    with pytest.raises(ValueError, match="exactly one"):
        ToriiClient.build_operator_signature_headers(
            method="POST",
            path="/v1/configuration",
            key_pair=key_pair,
            private_key_hex="11" * 32,
        )

    with pytest.raises(ValueError, match="nonce"):
        ToriiClient.build_operator_signature_headers(
            method="POST",
            path="/v1/configuration",
            key_pair=key_pair,
            nonce="",
        )

    with pytest.raises(ValueError, match="nonce"):
        ToriiClient.build_operator_signature_headers(
            method="POST",
            path="/v1/configuration",
            key_pair=key_pair,
            nonce="   ",
        )

    with pytest.raises(ValueError, match="timestamp_ms"):
        ToriiClient.build_operator_signature_headers(
            method="POST",
            path="/v1/configuration",
            key_pair=key_pair,
            timestamp_ms=-1,
        )

    class MissingPublicKey:
        def sign(self, message: bytes) -> bytes:
            return b"signature"

    with pytest.raises(TypeError, match="public_key_multihash"):
        ToriiClient.build_operator_signature_headers(
            method="POST",
            path="/v1/configuration",
            key_pair=MissingPublicKey(),
        )

    class BadSignature:
        public_key_multihash = "ed0120abc"

        def sign(self, message: bytes) -> str:
            return "not-bytes"

    with pytest.raises(TypeError, match="return bytes"):
        ToriiClient.build_operator_signature_headers(
            method="POST",
            path="/v1/configuration",
            key_pair=BadSignature(),
        )


def test_operator_signature_headers_accept_raw_private_key_inputs() -> None:
    raw_private_key = bytes([13] * 32)
    key_pair = Ed25519KeyPair.from_private_key(raw_private_key)

    hex_headers = ToriiClient.build_operator_signature_headers(
        method="POST",
        path="/v1/configuration",
        body=b"{}",
        private_key_hex=raw_private_key.hex(),
        timestamp_ms=123,
        nonce="hex-nonce",
    )
    bytes_headers = ToriiClient.build_operator_signature_headers(
        method="POST",
        path="/v1/configuration",
        body=b"{}",
        private_key=raw_private_key,
        timestamp_ms=123,
        nonce="bytes-nonce",
    )

    assert hex_headers["x-iroha-operator-public-key"] == key_pair.public_key_multihash
    assert bytes_headers["x-iroha-operator-public-key"] == key_pair.public_key_multihash
    with pytest.raises(ValueError, match="private-key multihash or raw Ed25519 hex"):
        ToriiClient.build_operator_signature_headers(
            method="POST",
            path="/v1/configuration",
            private_key="not-hex-or-multihash",
        )
    with pytest.raises(ValueError, match="32 bytes"):
        ToriiClient.build_operator_signature_headers(
            method="POST",
            path="/v1/configuration",
            private_key="00" * 31,
        )


def lifecycle_status() -> dict[str, object]:
    return {
        "version": 1,
        "nexus_enabled": True,
        "lane_count": 1,
        "lanes": [
            {
                "id": 0,
                "dataspace_id": 0,
                "alias": "universal",
                "description": None,
                "visibility": "public",
                "lane_type": None,
                "governance": None,
                "settlement": None,
                "storage": "full_replica",
                "proof_scheme": "merkle_sha256",
                "metadata": {},
            }
        ],
        "catalog_hash": "a1" * 32,
        "incarnations": [{"lane_id": 0, "incarnation": "b2" * 32}],
        "incarnation_root": "c3" * 32,
    }


def test_nexus_lane_lifecycle_status_fetches_exact_json_snapshot() -> None:
    status = lifecycle_status()
    session = FakeSession([response(200, status)])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.nexus_lane_lifecycle_status() == status
    assert len(session.calls) == 1
    assert session.calls[0]["method"] == "GET"
    assert session.calls[0]["path"] == "/v1/nexus/lifecycle"
    assert session.calls[0]["headers"]["Accept"] == "application/json"


def test_nexus_lane_lifecycle_submits_native_signed_set_parameter(monkeypatch: pytest.MonkeyPatch) -> None:
    import iroha_python.client as client_module

    status = lifecycle_status()
    session = FakeSession([response(200, status)])
    client = ToriiClient("http://torii.example", session=session, max_retries=3)
    additions = [
        {
            "id": 5,
            "dataspace_id": 42,
            "alias": "is",
            "description": None,
            "visibility": "restricted",
            "lane_type": "cbdc_private",
            "governance": "is_governance",
            "settlement": "boi_rtgs",
            "storage": "full_replica",
            "proof_scheme": "merkle_sha256",
            "metadata": {"owner": "boi-poc"},
        }
    ]
    captured: dict[str, object] = {}

    class FakeInstruction:
        @staticmethod
        def nexus_lane_lifecycle(status_json: str, plan_json: str) -> object:
            captured["status"] = json.loads(status_json)
            captured["plan"] = json.loads(plan_json)
            return "set-parameter-instruction"

    class FakeCrypto:
        Instruction = FakeInstruction

    def fake_submit(
        chain_id: str,
        authority: str,
        private_key: bytes,
        **kwargs: object,
    ) -> tuple[str, dict[str, object]]:
        captured["chain_id"] = chain_id
        captured["authority"] = authority
        captured["private_key"] = private_key
        captured.update(kwargs)
        return "envelope", {"kind": "Applied"}

    monkeypatch.setattr(client_module, "_require_crypto", lambda: FakeCrypto)
    monkeypatch.setattr(client, "build_and_submit_transaction", fake_submit)
    result = client.nexus_lane_lifecycle(
        additions,
        retire=[1],
        chain_id="test-chain",
        authority="alice@wonderland",
        private_key=bytes([9] * 32),
        fee_payment=FEE_PAYMENT,
    )

    assert result == ("envelope", {"kind": "Applied"})
    assert len(session.calls) == 1
    assert session.calls[0]["method"] == "GET"
    assert captured["status"] == status
    assert captured["plan"] == {"additions": additions, "retire": [1]}
    assert captured["chain_id"] == "test-chain"
    assert captured["authority"] == "alice@wonderland"
    assert captured["private_key"] == bytes([9] * 32)
    assert captured["instructions"] == ["set-parameter-instruction"]
    assert captured["wait"] is True
    assert captured["success_statuses"] == ("Applied",)


def test_nexus_lane_lifecycle_rejects_malformed_inputs() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    auth = {
        "chain_id": "test-chain",
        "authority": "alice@wonderland",
        "private_key": bytes([10] * 32),
        "fee_payment": FEE_PAYMENT,
    }

    with pytest.raises(TypeError, match="additions"):
        client.nexus_lane_lifecycle("not-a-list", **auth)  # type: ignore[arg-type]

    with pytest.raises(TypeError, match=r"additions\[0\]"):
        client.nexus_lane_lifecycle([object()], **auth)  # type: ignore[list-item]

    with pytest.raises(ValueError, match=r"retire\[0\]"):
        client.nexus_lane_lifecycle([], retire=[-1], **auth)

    with pytest.raises(TypeError, match="retire"):
        client.nexus_lane_lifecycle([], retire="12", **auth)  # type: ignore[arg-type]

    with pytest.raises(TypeError, match="retire"):
        client.nexus_lane_lifecycle([], retire=object(), **auth)  # type: ignore[arg-type]

    with pytest.raises(ValueError, match=r"retire\[0\]"):
        client.nexus_lane_lifecycle([], retire=[object()], **auth)  # type: ignore[list-item]

    with pytest.raises(ValueError, match=r"retire\[0\]"):
        client.nexus_lane_lifecycle([], retire=[True], **auth)  # type: ignore[list-item]

    for invalid in (1.5, "1", 0x1_0000_0000):
        with pytest.raises(ValueError, match=r"retire\[0\].*u32"):
            client.nexus_lane_lifecycle([], retire=[invalid], **auth)  # type: ignore[list-item]

    with pytest.raises(ValueError, match="duplicates lane"):
        client.nexus_lane_lifecycle([], retire=[1, 1], **auth)

    lane = {"id": 1, "alias": "lane-one"}
    with pytest.raises(ValueError, match=r"additions\[1\].id duplicates"):
        client.nexus_lane_lifecycle([lane, lane], **auth)

    with pytest.raises(ValueError, match=r"additions\[0\].id.*u32"):
        client.nexus_lane_lifecycle([{"id": 1.5, "alias": "bad"}], **auth)

    with pytest.raises(ValueError, match="must add or retire"):
        client.nexus_lane_lifecycle([], **auth)


def test_nexus_lane_lifecycle_rejects_retired_operator_only_shape() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    key_pair = Ed25519KeyPair.from_private_key(bytes([11] * 32))

    with pytest.raises(RuntimeError, match="operator-only Nexus lifecycle calls are deprecated"):
        client.nexus_lane_lifecycle([], key_pair=key_pair, fee_payment=FEE_PAYMENT)
    with pytest.raises(RuntimeError, match="operator-only Nexus lifecycle calls are deprecated"):
        client.nexus_lane_lifecycle(
            [], private_key_hex="11" * 32, fee_payment=FEE_PAYMENT
        )
    with pytest.raises(RuntimeError, match="operator-only Nexus lifecycle calls are deprecated"):
        client.nexus_lane_lifecycle([], private_key="11" * 32, fee_payment=FEE_PAYMENT)


def test_nexus_lane_lifecycle_requires_full_transaction_signing_context() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    with pytest.raises(ValueError, match="chain_id is required"):
        client.nexus_lane_lifecycle(
            [],
            authority="alice@wonderland",
            private_key=bytes([1] * 32),
            fee_payment=FEE_PAYMENT,
        )
    with pytest.raises(ValueError, match="authority is required"):
        client.nexus_lane_lifecycle(
            [],
            chain_id="chain",
            private_key=bytes([1] * 32),
            fee_payment=FEE_PAYMENT,
        )
    with pytest.raises(ValueError, match="private_key bytes are required"):
        client.nexus_lane_lifecycle(
            [],
            chain_id="chain",
            authority="alice@wonderland",
            fee_payment=FEE_PAYMENT,
        )


def test_nexus_lane_lifecycle_rejects_malformed_status() -> None:
    malformed = lifecycle_status()
    malformed["catalog_hash"] = ""
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([response(200, malformed)]),
        max_retries=0,
    )

    with pytest.raises(ValueError, match="catalog_hash"):
        client.nexus_lane_lifecycle_status()

    wrong_incarnation = lifecycle_status()
    wrong_incarnation["incarnations"] = [
        {"lane_id": 1, "incarnation": "b2" * 32}
    ]
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([response(200, wrong_incarnation)]),
        max_retries=0,
    )
    with pytest.raises(ValueError, match="exactly match"):
        client.nexus_lane_lifecycle_status()

    missing_root = lifecycle_status()
    missing_root["incarnation_root"] = ""
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([response(200, missing_root)]),
        max_retries=0,
    )
    with pytest.raises(ValueError, match="incarnation_root"):
        client.nexus_lane_lifecycle_status()


def test_nexus_lane_lifecycle_surfaces_stale_transaction_without_refetch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import iroha_python.client as client_module

    session = FakeSession([response(200, lifecycle_status())])
    client = ToriiClient("http://torii.example", session=session, max_retries=3)

    class FakeInstruction:
        @staticmethod
        def nexus_lane_lifecycle(_status: str, _plan: str) -> object:
            return object()

    class FakeCrypto:
        Instruction = FakeInstruction

    monkeypatch.setattr(client_module, "_require_crypto", lambda: FakeCrypto)

    def stale(*_args: object, **_kwargs: object) -> object:
        raise RuntimeError("stale Nexus lane lifecycle catalog commitment")

    monkeypatch.setattr(client, "build_and_submit_transaction", stale)
    with pytest.raises(RuntimeError, match="stale Nexus"):
        client.nexus_lane_lifecycle(
            [],
            retire=[0],
            chain_id="test-chain",
            authority="alice@wonderland",
            private_key=bytes([12] * 32),
            fee_payment=FEE_PAYMENT,
        )

    assert len(session.calls) == 1
    assert session.calls[0]["path"] == "/v1/nexus/lifecycle"


def test_compose_asset_id_accepts_dataspace_scope_shortcut() -> None:
    assert (
        ToriiClient.compose_asset_id("abc", "alice@boi", scope="42")
        == "abc#alice@boi#dataspace:42"
    )
