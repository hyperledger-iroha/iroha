from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from scripts import taira_privacy_action_driver_ipc as ipc


ROOT = Path(__file__).resolve().parents[2]


def _request() -> bytes:
    return ipc.build_verange_request(
        asset_definition_id="verange_value#privacy",
        candidate_binding_sha256="11" * 32,
        chain_id="taira-qualification-v1",
        creation_time_millis=1_900_000_000_000,
        genesis_hash_hex="22" * 32,
        nonce=17,
        ttl_millis=7_200_000,
        values=[0, 1, 17, 2**32 - 1],
    )


def _canonical(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def _response(request: dict[str, object]) -> bytes:
    transaction = b"norito-proof-bearing-transaction"
    return _canonical(
        {
            "candidate_binding_sha256": request["candidate_binding_sha256"],
            "operation": ipc.OPERATION,
            "protocol": ipc.PROTOCOL,
            "request_id": request["request_id"],
            "schema": ipc.RESPONSE_SCHEMA,
            "schema_version": ipc.SCHEMA_VERSION,
            "transaction_hash_hex": "33" * 32,
            "transaction_norito_hex": transaction.hex(),
            "transaction_sha256": hashlib.sha256(transaction).hexdigest(),
        }
    )


def test_canonical_request_and_typed_response_round_trip() -> None:
    request = ipc.validate_request(_request())
    response = ipc.validate_response(_response(request), expected_request=request)
    assert response["transaction_norito"] == b"norito-proof-bearing-transaction"
    assert response["transaction_hash_hex"] == "33" * 32


def test_python_and_rust_share_one_request_id_golden() -> None:
    path = ROOT / "fixtures/privacy_exact12_action_driver_request_id_v1.json"
    golden = json.loads(path.read_bytes())
    assert set(golden) == {
        "canonical_request",
        "canonical_request_id_body",
        "request",
        "request_id",
        "schema",
        "schema_version",
    }
    assert golden["schema"] == "iroha.taira.privacy_action_driver_request_id_golden"
    assert golden["schema_version"] == 1
    request = dict(golden["request"])
    request_id = request.pop("request_id")
    body = ipc._canonical(request)[:-1]
    assert body.decode("ascii") == golden["canonical_request_id_body"]
    assert request_id == golden["request_id"]
    assert request_id == hashlib.sha256(ipc.REQUEST_ID_DOMAIN + body).hexdigest()
    rebuilt = ipc.build_verange_request(
        asset_definition_id=request["asset_definition_id"],
        candidate_binding_sha256=request["candidate_binding_sha256"],
        chain_id=request["chain_id"],
        creation_time_millis=request["creation_time_millis"],
        genesis_hash_hex=request["genesis_hash_hex"],
        nonce=request["nonce"],
        ttl_millis=request["ttl_millis"],
        values=request["values"],
    )
    assert rebuilt.decode("ascii") == golden["canonical_request"]
    assert ipc.validate_request(rebuilt)["request_id"] == request_id


@pytest.mark.parametrize("mutation", ["suffix", "truncated", "unknown", "request-id"])
def test_request_framing_fails_closed(mutation: str) -> None:
    payload = _request()
    if mutation == "suffix":
        payload += b"\n"
    elif mutation == "truncated":
        payload = payload[:-1]
    else:
        value = json.loads(payload)
        if mutation == "unknown":
            value["endpoint"] = "http://peer.invalid"
        else:
            value["request_id"] = "ff" * 32
        payload = _canonical(value)
    with pytest.raises(ipc.PrivacyActionDriverIpcError):
        ipc.validate_request(payload)


def test_response_context_and_payload_digests_fail_closed() -> None:
    request = ipc.validate_request(_request())
    value = json.loads(_response(request))
    value["transaction_norito_hex"] = "00" + value["transaction_norito_hex"][2:]
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="digest differs"):
        ipc.validate_response(_canonical(value), expected_request=request)


def test_response_rejects_duplicate_fields_and_driver_outcome_claims() -> None:
    request = ipc.validate_request(_request())
    response = _response(request)
    duplicate = response.replace(
        b'{"candidate_binding_sha256":',
        b'{"operation":"build-verange-action-v1","candidate_binding_sha256":',
        1,
    )
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="not JSON"):
        ipc.validate_response(duplicate, expected_request=request)
    value = json.loads(response)
    value["status"] = "passed"
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="fields are not exact"):
        ipc.validate_response(_canonical(value), expected_request=request)


def test_response_rejects_an_incomplete_caller_claimed_request_context() -> None:
    request = ipc.validate_request(_request())
    with pytest.raises(ipc.PrivacyActionDriverIpcError, match="fields are not exact"):
        ipc.validate_response(
            _response(request),
            expected_request={
                "candidate_binding_sha256": request["candidate_binding_sha256"],
                "request_id": request["request_id"],
            },
        )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("asset_definition_id", "privacy_☃#asset"),
        ("chain_id", "taira_☃"),
        ("creation_time_millis", 2**63),
    ],
)
def test_cross_language_input_bounds_fail_closed(field: str, value: object) -> None:
    arguments = {
        "asset_definition_id": "verange_value#privacy",
        "candidate_binding_sha256": "11" * 32,
        "chain_id": "taira-qualification-v1",
        "creation_time_millis": 1_900_000_000_000,
        "genesis_hash_hex": "22" * 32,
        "nonce": 17,
        "ttl_millis": 7_200_000,
        "values": [1],
    }
    arguments[field] = value
    with pytest.raises(ipc.PrivacyActionDriverIpcError):
        ipc.build_verange_request(**arguments)


def test_rust_driver_is_narrow_non_networked_and_builds_a_native_proof_action() -> None:
    source = (
        ROOT / "crates/iroha_core/src/bin/privacy_exact12_action_driver.rs"
    ).read_text(encoding="utf-8")
    manifest = (ROOT / "crates/iroha_core/Cargo.toml").read_text(encoding="utf-8")
    assert "build_privacy_release_verange_network_action_v1" in source
    assert "norito::to_bytes(&action.transaction)" in source
    assert "Zeroizing::new" in source
    assert 'const MAX_ASSET_DEFINITION_ID_BYTES: usize = 1024;' in source
    assert 'const MAX_CHAIN_ID_BYTES: usize = 128;' in source
    assert (
        "const MAX_CREATION_TIME_MILLIS: u64 = 9_223_372_036_854_775_807;"
        in source
    )
    assert "reqwest" not in source
    assert "iroha::client" not in source
    assert "privacy_exact12_action_driver" in manifest
    assert 'required-features = ["privacy-release-evidence"]' in manifest
    assert "python_and_rust_share_one_request_id_golden" in source
    assert "privacy_exact12_action_driver_request_id_v1.json" in source
