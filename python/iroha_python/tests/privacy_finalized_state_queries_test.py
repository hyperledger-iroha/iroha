"""Authenticated finalized privacy-state query coverage for stable IDs 97–104."""

from __future__ import annotations

import json
from pathlib import Path
from types import SimpleNamespace

import pytest
import requests

import iroha_python
import iroha_python.client as client_module
import iroha_python.crypto as crypto_module
from iroha_python import (
    AccountAddress,
    NetworkId,
    SorafsAliasPolicy,
    ToriiCanonicalRequestAuth,
)
from iroha_python.client import (
    LocalSigningContext,
    PrivacyFinalizedStateViewV1,
    ToriiClient,
)

ROOT = Path(__file__).resolve().parents[3]
NETWORK_ID = NetworkId.from_bytes(bytes([0x91]) * 32)
AUTH = ToriiCanonicalRequestAuth(
    network_id=NETWORK_ID.literal,
    account_id=AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x11]) * 32,
    ).to_i105(0x02F1),
    signer=lambda _message: bytes([0x44]) * 64,
)
CHUNKS = tuple(bytes([index]) * 32 for index in range(1, 6))


def _client(monkeypatch: pytest.MonkeyPatch) -> ToriiClient:
    monkeypatch.setattr(client_module, "_CRYPTO_MODULE", object())
    return ToriiClient(
        "http://torii.invalid",
        sorafs_alias_policy=SorafsAliasPolicy(
            positive_ttl_secs=1,
            refresh_window_secs=1,
            hard_expiry_secs=1,
            negative_ttl_secs=1,
            revocation_ttl_secs=1,
            rotation_max_age_secs=1,
            successor_grace_secs=0,
            governance_grace_secs=0,
        ),
        local_signing_context=LocalSigningContext(NETWORK_ID),
    )


def _ok_response() -> requests.Response:
    response = requests.Response()
    response.status_code = 200
    response.headers["Content-Type"] = "application/x-norito"
    response._content = b"canonical-finalized-state-response"
    return response


def test_all_stable_state_queries_use_native_signer_and_exact_binding(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client = _client(monkeypatch)
    build_calls: list[tuple[object, ...]] = []
    inspect_calls: list[tuple[object, ...]] = []
    request_calls: list[tuple[str, str, dict[str, object]]] = []

    def build(*args: object) -> bytes:
        build_calls.append(args)
        return b"signed-finalized-state-query"

    def inspect(*args: object) -> dict[str, object]:
        inspect_calls.append(args)
        return {
            "network_id": "native-network-id",
            "finalized_height": 501,
            "finalized_block_hash": "native-finalized-block-hash",
            "nested": {"items": [1, 2]},
        }

    def request(method: str, path: str, **kwargs: object) -> requests.Response:
        request_calls.append((method, path, kwargs))
        return _ok_response()

    monkeypatch.setattr(
        crypto_module,
        "build_privacy_finalized_state_query_with_signer",
        build,
    )
    monkeypatch.setattr(
        crypto_module,
        "inspect_privacy_finalized_state_query_response",
        inspect,
    )
    monkeypatch.setattr(client, "_request", request)

    calls = (
        (
            97,
            0,
            CHUNKS[0] + CHUNKS[1],
            lambda: client.get_privacy_zk_ace_replay_nullifier_v1(
                CHUNKS[0], CHUNKS[1], canonical_auth=AUTH
            ),
        ),
        (
            98,
            2,
            CHUNKS[0],
            lambda: client.get_privacy_proof_managed_pool_state_v1(
                "pq-masp-stark-v0", CHUNKS[0], canonical_auth=AUTH
            ),
        ),
        (
            99,
            0,
            CHUNKS[0],
            lambda: client.get_privacy_orchard_pool_state_v1(
                CHUNKS[0], canonical_auth=AUTH
            ),
        ),
        (
            100,
            0,
            CHUNKS[0] + CHUNKS[1],
            lambda: client.get_privacy_orchard_nullifier_v1(
                CHUNKS[0], CHUNKS[1], canonical_auth=AUTH
            ),
        ),
        (
            101,
            0,
            CHUNKS[0],
            lambda: client.get_privacy_anonymous_pgc_pool_state_v1(
                CHUNKS[0], canonical_auth=AUTH
            ),
        ),
        (
            102,
            0,
            b"".join(CHUNKS[:4]),
            lambda: client.get_privacy_zk_ams_admission_v1(
                *CHUNKS[:4], canonical_auth=AUTH
            ),
        ),
        (
            103,
            0,
            b"".join(CHUNKS[:4]),
            lambda: client.get_privacy_zk_ams_provision_v1(
                *CHUNKS[:4], canonical_auth=AUTH
            ),
        ),
        (
            104,
            0,
            b"".join(CHUNKS[:3]),
            lambda: client.get_privacy_zk_x509_certificate_nullifier_v1(
                *CHUNKS[:3], canonical_auth=AUTH
            ),
        ),
    )

    for query_id, protocol_index, binding, invoke in calls:
        view = invoke()
        assert isinstance(view, PrivacyFinalizedStateViewV1)
        assert view.query_id == query_id
        assert view.finalized_height == 501
        assert build_calls[-1] == (
            AUTH.account_id,
            AUTH.signer,
            NETWORK_ID,
            query_id,
            protocol_index,
            binding,
        )
        assert inspect_calls[-1] == (
            NETWORK_ID,
            query_id,
            protocol_index,
            binding,
            b"canonical-finalized-state-response",
        )

    assert len(request_calls) == 8
    for method, path, kwargs in request_calls:
        assert (method, path) == ("POST", "/v1/query")
        assert kwargs["data"] == b"signed-finalized-state-query"
        assert kwargs["allow_retry"] is False
        assert kwargs["allow_redirects"] is False
        assert kwargs["headers"] == {
            "Content-Type": "application/x-norito",
            "Accept": "application/x-norito",
        }


def test_finalized_state_view_deep_freezes_native_projection() -> None:
    view = PrivacyFinalizedStateViewV1(
        query_id=101,
        query_schema="anonymous_pgc_pool_state_v1",
        projection={
            "network_id": "network",
            "finalized_height": 7,
            "finalized_block_hash": "block",
            "latest_transition": {"path": [1, 2]},
        },
    )

    assert view["latest_transition"]["path"] == (1, 2)
    with pytest.raises(TypeError):
        view.projection["finalized_height"] = 8  # type: ignore[index]
    with pytest.raises(TypeError):
        view["latest_transition"]["path"] = ()  # type: ignore[index]


def test_crypto_wrapper_rejects_any_native_projection_shape_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    projection = {
        "network_id": "network",
        "policy_id": "policy",
        "replay_nullifier": "nullifier",
        "policy_record_digest": "policy-record",
        "statement_digest": "statement",
        "admitted_at_height": 4,
        "action_index": 0,
        "finalized_height": 7,
        "finalized_block_hash": "block",
    }
    native_calls: list[tuple[object, ...]] = []

    def inspect(*args: object) -> str:
        native_calls.append(args)
        return json.dumps(projection)

    monkeypatch.setattr(
        crypto_module,
        "_crypto",
        SimpleNamespace(inspect_privacy_finalized_state_query_response=inspect),
    )
    observed = crypto_module.inspect_privacy_finalized_state_query_response(
        NETWORK_ID,
        97,
        0,
        CHUNKS[0] + CHUNKS[1],
        b"canonical-response",
    )
    assert observed == projection
    assert native_calls == [
        (
            NETWORK_ID,
            97,
            0,
            CHUNKS[0] + CHUNKS[1],
            b"canonical-response",
        )
    ]

    projection["forged"] = True
    with pytest.raises(RuntimeError, match="invalid fields"):
        crypto_module.inspect_privacy_finalized_state_query_response(
            NETWORK_ID,
            97,
            0,
            CHUNKS[0] + CHUNKS[1],
            b"canonical-response",
        )


def test_only_404_is_a_not_found_result(monkeypatch: pytest.MonkeyPatch) -> None:
    client = _client(monkeypatch)
    monkeypatch.setattr(
        crypto_module,
        "build_privacy_finalized_state_query_with_signer",
        lambda *_args: b"signed-finalized-state-query",
    )
    monkeypatch.setattr(
        crypto_module,
        "inspect_privacy_finalized_state_query_response",
        lambda *_args: pytest.fail("absent response reached native inspection"),
    )
    missing = requests.Response()
    missing.status_code = 404
    missing._content = b""
    monkeypatch.setattr(client, "_request", lambda *_args, **_kwargs: missing)
    assert (
        client.get_privacy_anonymous_pgc_pool_state_v1(
            CHUNKS[0], canonical_auth=AUTH
        )
        is None
    )

    denied = requests.Response()
    denied.status_code = 403
    denied._content = b"denied"
    monkeypatch.setattr(client, "_request", lambda *_args, **_kwargs: denied)
    monkeypatch.setattr(
        client,
        "_expect_status",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(
            RuntimeError("permission denied")
        ),
    )
    with pytest.raises(RuntimeError, match="permission denied"):
        client.get_privacy_anonymous_pgc_pool_state_v1(
            CHUNKS[0], canonical_auth=AUTH
        )


def test_request_union_rejects_zero_trailing_and_open_protocols_before_torii(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client = _client(monkeypatch)
    monkeypatch.setattr(
        client,
        "_request",
        lambda *_args, **_kwargs: pytest.fail("invalid state query reached Torii"),
    )

    with pytest.raises(ValueError, match="32 non-zero"):
        client.get_privacy_orchard_pool_state_v1(bytes(32), canonical_auth=AUTH)
    with pytest.raises(ValueError, match="32 non-zero"):
        client.get_privacy_orchard_pool_state_v1(bytes(33), canonical_auth=AUTH)
    with pytest.raises(ValueError, match=r"FCMP\+\+"):
        client.get_privacy_proof_managed_pool_state_v1(
            "orchard-note-action-v1", CHUNKS[0], canonical_auth=AUTH
        )


def test_python_native_state_query_boundary_is_sealed_to_ids_97_through_104() -> None:
    native = (
        ROOT
        / "python/iroha_python/iroha_python_rs/src/privacy_finalized_state.rs"
    ).read_text(encoding="utf-8")
    bridge = (
        ROOT / "python/iroha_python/iroha_python_rs/src/lib.rs"
    ).read_text(encoding="utf-8")
    client = (
        ROOT / "python/iroha_python/src/iroha_python/client.py"
    ).read_text(encoding="utf-8")

    for query_id, query in (
        (97, "FindPrivacyZkAceReplayNullifierV1"),
        (98, "FindPrivacyProofManagedPoolStateV1"),
        (99, "FindPrivacyOrchardPoolStateV1"),
        (100, "FindPrivacyOrchardNullifierV1"),
        (101, "FindPrivacyAnonymousPgcPoolStateV1"),
        (102, "FindPrivacyZkAmsAdmissionV1"),
        (103, "FindPrivacyZkAmsProvisionV1"),
        (104, "FindPrivacyZkX509CertificateNullifierV1"),
    ):
        assert f"{query_id} =>" in native
        assert query in native
    for marker in (
        "sign_query_request_with_signer(",
        "decode_canonical_with_limits(",
        "if canonical != response",
        "view.validate()",
        "view.network_id != *expected_network_id",
    ):
        assert marker in native
    assert "mod privacy_finalized_state;" in bridge
    assert "privacy_finalized_state::build_query_with_signer" in bridge
    assert "privacy_finalized_state::inspect_response" in bridge
    assert '"/v1/query"' in client
    assert "canonical_auth.signer" in client
    assert iroha_python.PrivacyFinalizedStateViewV1 is PrivacyFinalizedStateViewV1
