from __future__ import annotations

import base64
import hashlib
import json
from decimal import Decimal
from urllib.parse import quote, urlsplit

import pytest
import requests

from iroha_python import (
    AccountAddress,
    AccountAssetsPage,
    DataEventFilter,
    Ed25519KeyPair,
    Instruction,
    ToriiClient,
    TransactionConfig,
    TransactionDraft,
)
from iroha_python._privacy_backends import (
    _is_pending_production_backend_label,
    _is_production_verify_backend_label,
    _require_production_verify_backend_label,
)
from iroha_python.repo import RepoAgreementListPage


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
            }
        )
        if not self.responses:
            raise AssertionError(f"unexpected request {method} {url}")
        response = self.responses.pop(0)
        response.url = url
        return response


def response(
    status: int,
    payload: object | None = None,
    *,
    text: str = "",
    headers: dict[str, str] | None = None,
) -> requests.Response:
    result = requests.Response()
    result.status_code = status
    result.headers.update(headers or {})
    if payload is None:
        result._content = text.encode("utf-8")
    else:
        result._content = json.dumps(payload).encode("utf-8")
        result.headers["Content-Type"] = "application/json"
    return result


def test_privacy_backend_pending_classifier_rejects_adversarial_splices() -> None:
    for label in (
        "halo2/ipa/penumbra",
        "halo2/ipa/masp",
        "halo2/ipa/monero",
        "halo2/ipa/curve-tree",
    ):
        assert _is_pending_production_backend_label(label)

    for label in (
        "halo2/ipa/orchard/dev-fixture",
        "stark/fri/miden/claimed-production",
        "anonymous-pgc-k-out-of-n-v1-production",
        "sis-hints-anoncred-pq-v0-devfixture",
        "groth16/bls12-377/../../prod",
        "post-quantum-masp/audit-claimed",
    ):
        assert not _is_pending_production_backend_label(label)


def test_privacy_backend_production_verify_classifier_parity() -> None:
    supported = (
        "halo2/ipa",
        "halo2/ipa:ivm-execution-v1",
        "halo2/pasta/ivm-execution-v1",
        "halo2/pasta/kagemusha-folded-v1",
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
        "stark/fri",
        "stark/fri/sha256-goldilocks",
    )
    for backend in supported:
        assert _is_production_verify_backend_label(backend), backend
        assert _require_production_verify_backend_label(backend, "backend") == backend

    unsupported = (
        "",
        "unknown/privacy/backend",
        "halo2/unknown-native-v1",
        "halo2/ipa:unknown-native-v1",
        "stark/unknown-native-v1",
        "halo2/bn254",
        "groth16",
        "groth16/bls12-377",
        " halo2/ipa",
        "halo2/ipa ",
        "\thalo2/ipa",
        "halo2/ipa\n",
        "halo2\uFF0Fipa",
        "halo2/\u200Bipa",
        "h\u0430lo2/ipa",
        "halo2/ipa\0",
        "../halo2/ipa",
        "halo2/ipa/orchard",
        "halo2-ipa-orchard",
        "halo2/ipa/penumbra",
        "halo2/ipa/masp",
        "halo2/ipa/monero",
        "halo2/ipa/curve-tree",
        "halo2/pasta/tiny-add",
        "halo2/ipa/tiny-add",
        "halo2/ipa:tiny-add",
        "halo2/pasta/asset-hidden-transfer-public-test",
        "halo2/ipa/asset-hidden-transfer-public-test",
        "halo2/ipa:asset-hidden-transfer-public-test",
        "stark/fri/miden",
        "stark/fri/miden/claimed-production",
        "stark/fri/latest",
        "stark/fri/random-profile",
        "stark/fri/sha512-goldilocks",
        "stark/fri/audit-proof-v1",
        "stark/fri/sha256 goldilocks",
        "stark/fri/sha256+goldilocks",
        "halo2/ipa+mock",
        "halo2/ipa:production-ready",
        "halo2/ipa:claimed-production",
        "halo2/ipa:mainnet-ready",
        "stark/fri/audit-signoff",
        "stark/fri/externally-audited",
        "stark/fri/security-review-passed",
        "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
        "stark/fri/a-u-d-i-t-c-l-a-i-m",
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/kzg",
        "halo2/pasta/mock",
        "kzg/powersoftau",
    )
    for backend in unsupported:
        assert not _is_production_verify_backend_label(backend), backend
        expected_error = (
            "non-empty string"
            if not isinstance(backend, str) or not backend.strip()
            else "unsupported production verifier backend"
        )
        with pytest.raises(ValueError, match=expected_error):
            _require_production_verify_backend_label(backend, "backend")


def zk_verifying_key_commitment(backend: str, vk_bytes: bytes) -> str:
    backend_bytes = backend.encode("utf-8")
    preimage = (
        b"iroha:zk:v1:vk"
        + len(backend_bytes).to_bytes(8, "big")
        + backend_bytes
        + len(vk_bytes).to_bytes(8, "big")
        + vk_bytes
    )
    return hashlib.sha256(preimage).hexdigest()


def account_address(seed: int, discriminant: int = 0x02F1) -> str:
    return Ed25519KeyPair.from_private_key(bytes([seed] * 32)).default_account_id(
        "wonderland",
        discriminant,
    )


def test_account_exists_falls_back_to_listing_on_route_unavailable() -> None:
    session = FakeSession(
        [
            response(
                503,
                text="route unavailable",
                headers={"x-iroha-reject-code": "route_unavailable"},
            ),
            response(200, {"items": [{"id": "adult@is"}], "total": 1}),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.account_exists("adult@is")
    assert session.calls == [
        {
            "method": "GET",
            "path": "/v1/accounts/adult%40is",
            "params": None,
            "data": None,
        },
        {
            "method": "GET",
            "path": "/v1/accounts",
            "params": {"limit": 200, "offset": 0},
            "data": None,
        },
    ]


def test_asset_balance_tries_taira_prefix_variant_after_prefix_error() -> None:
    session = FakeSession(
        [
            response(400, text="ERR_UNEXPECTED_NETWORK_PREFIX"),
            response(
                200,
                {
                    "items": [
                        {
                            "asset_id": "canonical-ds-id#sorau123",
                            "asset_alias": "ds#wonderland.is",
                            "quantity": "42.5",
                        }
                    ],
                    "total": 1,
                },
            ),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.asset_balance(
        "testu123",
        "ds#wonderland.is",
        include_taira_prefix_variant=True,
    ) == Decimal("42.5")
    assert [call["path"] for call in session.calls] == [
        "/v1/accounts/testu123/assets",
        "/v1/accounts/sorau123/assets",
    ]


def test_asset_balance_returns_zero_when_account_has_no_matching_asset() -> None:
    session = FakeSession(
        [
            response(
                200,
                {
                    "items": [{"asset_id": "other#adult@is", "quantity": "3"}],
                    "total": 1,
                },
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.asset_balance("adult@is", "ds#wonderland.is") == Decimal("0")


def test_get_asset_definition_returns_none_for_missing_definition() -> None:
    session = FakeSession([response(404, text="missing")])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.get_asset_definition("ds#wonderland.is") is None


def test_data_model_validation_uses_typed_node_capabilities() -> None:
    session = FakeSession([response(200, {"abi_version": 1, "data_model_version": 1})])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    client._ensure_data_model_validation()

    assert client._data_model_validation == "matched"
    assert session.calls == [
        {
            "method": "GET",
            "path": "/v1/node/capabilities",
            "params": None,
            "data": None,
        }
    ]


def test_query_accounts_typed_preserves_bounded_page_metadata() -> None:
    session = FakeSession(
        [
            response(
                200,
                {
                    "items": [{"id": "adult@is"}],
                    "has_more": True,
                    "count_mode": "bounded",
                    "indexed_height": 7,
                    "indexed_block_hash": "ab" * 32,
                    "query_source": "live",
                },
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    page = client.query_accounts_typed(limit=1, count_mode="bounded")

    assert page.total is None
    assert page.has_more is True
    assert page.count_mode == "bounded"
    assert page.indexed_height == 7
    assert page.indexed_block_hash == "ab" * 32
    assert page.query_source == "live"
    assert json.loads(session.calls[0]["data"])["count_mode"] == "bounded"


def test_query_accounts_rejects_invalid_count_mode_without_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(ValueError, match="count_mode"):
        client.query_accounts(count_mode="full")
    assert session.calls == []


def test_account_assets_page_rejects_malformed_page_metadata() -> None:
    with pytest.raises(TypeError, match="has_more"):
        AccountAssetsPage.from_payload(
            {
                "items": [{"asset": "rose#wonderland", "quantity": "1"}],
                "has_more": "false",
                "count_mode": "bounded",
            }
        )


def test_list_domains_typed_passes_count_mode_and_preserves_bounded_metadata() -> None:
    session = FakeSession(
        [
            response(
                200,
                {
                    "items": [{"id": "wonderland"}],
                    "has_more": False,
                    "count_mode": "bounded",
                },
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    page = client.list_domains_typed(limit=1, count_mode="bounded")

    assert page.total is None
    assert page.has_more is False
    assert page.count_mode == "bounded"
    assert session.calls[0]["params"] == {"limit": 1, "count_mode": "bounded"}


def test_query_rwas_typed_preserves_bounded_metadata_and_validates_count_mode() -> None:
    session = FakeSession(
        [
            response(
                200,
                {
                    "items": [{"id": "rwa$bond"}],
                    "has_more": True,
                    "count_mode": "bounded",
                },
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    page = client.query_rwas_typed(limit=1, count_mode="bounded")

    assert page.total is None
    assert page.has_more is True
    assert page.count_mode == "bounded"
    assert json.loads(session.calls[0]["data"])["count_mode"] == "bounded"

    rejecting = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    with pytest.raises(ValueError, match="count_mode"):
        rejecting.list_rwas(count_mode="full")


def test_repo_agreement_page_preserves_bounded_metadata_and_rejects_bad_flags() -> None:
    payload = {
        "items": [
            {
                "id": "repo-1",
                "initiator": "alice@is",
                "counterparty": "bob@is",
                "custodian": None,
                "cash_leg": {"asset_definition_id": "cash#is", "quantity": "100"},
                "collateral_leg": {"asset_definition_id": "bond#is", "quantity": "120"},
                "rate_bps": 250,
                "maturity_timestamp_ms": 2_000,
                "initiated_timestamp_ms": 1_000,
                "last_margin_check_timestamp_ms": 1_000,
                "governance": {"haircut_bps": 500, "margin_frequency_secs": 3600},
            }
        ],
        "has_more": True,
        "count_mode": "bounded",
        "indexed_height": 11,
        "indexed_block_hash": "cd" * 32,
        "query_source": "live",
    }

    page = RepoAgreementListPage.from_payload(payload)

    assert page.total is None
    assert page.has_more is True
    assert page.count_mode == "bounded"
    assert page.indexed_height == 11
    assert page.indexed_block_hash == "cd" * 32
    assert page.query_source == "live"

    bad = dict(payload)
    bad["has_more"] = "true"
    with pytest.raises(TypeError, match="has_more"):
        RepoAgreementListPage.from_payload(bad)


def test_repo_agreement_client_normalizes_count_mode_before_request() -> None:
    session = FakeSession([response(200, {"items": [], "has_more": False, "count_mode": "bounded"})])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    page = client.list_repo_agreements(limit=1, count_mode="bounded")

    assert page.count_mode == "bounded"
    assert session.calls[0]["params"] == {"limit": 1, "count_mode": "bounded"}

    rejecting = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    with pytest.raises(ValueError, match="count_mode"):
        rejecting.query_repo_agreements({"count_mode": "full"})


def test_solve_account_faucet_pow_accepts_zero_difficulty_puzzle() -> None:
    anchor_height, nonce_hex = ToriiClient.solve_account_faucet_pow(
        "adult@is",
        {
            "difficulty_bits": 0,
            "anchor_height": 7,
            "anchor_block_hash_hex": "00" * 32,
            "scrypt_log_n": 1,
            "scrypt_r": 1,
            "scrypt_p": 1,
        },
    )

    assert anchor_height == 7
    assert nonce_hex == "0000000000000000"


def test_sns_helpers_read_policy_and_submit_registration() -> None:
    session = FakeSession(
        [
            response(200, {"payment_asset_id": "fee", "pricing": []}),
            response(202, {"ok": True}),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.get_sns_policy(2)["payment_asset_id"] == "fee"
    registration = client.submit_sns_name_registration(
        {
            "selector": {"suffix_id": 2, "label": "is"},
            "owner": "owner@is",
        }
    )

    assert registration.status_code == 202
    assert [call["path"] for call in session.calls] == [
        "/v1/sns/policies/2",
        "/v1/sns/names",
    ]
    assert b'"owner": "owner@is"' in session.calls[-1]["data"]


def test_zk_verifying_key_helpers_detect_active_status() -> None:
    session = FakeSession(
        [
            response(200, {"record": {"status": "Active"}}),
            response(202, {"accepted": True}),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.zk_verifying_key_active("halo2/ipa", "vk_transfer")
    response_obj = client.submit_zk_verifying_key_registration(
        {
            "authority": "alice",
            "backend": "halo2/ipa",
            "name": "vk_transfer",
            "private_key": "ed25519:deadbeef",
        }
    )

    assert response_obj.status_code == 202
    assert [call["path"] for call in session.calls] == [
        "/v1/zk/vk/halo2%2Fipa/vk_transfer",
        "/v1/zk/vk/register",
    ]


def test_zk_verifying_key_helpers_reject_unstable_stark_aliases() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    for backend in ("stark/fri/latest", "stark/fri/attestation", "stark/fri/contest"):
        with pytest.raises(ValueError, match="unsupported production verifier backend"):
            client.submit_zk_verifying_key_registration(
                {"backend": backend, "name": "vk_false_positive_guard"}
            )

    assert session.calls == []


def test_zk_verifying_key_registration_rejects_unsupported_backends_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    for backend in (
        "halo2/unknown-native-v1",
        "halo2/ipa:unknown-native-v1",
        "stark/unknown-native-v1",
        " halo2/ipa",
        "halo2/ipa ",
        "\thalo2/ipa",
        "halo2/ipa\n",
        "halo2\uFF0Fipa",
        "halo2/\u200Bipa",
        "h\u0430lo2/ipa",
        "stark/fri/miden",
        "stark/fri/latest",
        "stark/fri/attestation",
        "stark/fri/contest",
        "stark/fri/random-profile",
        "stark/fri/sha512-goldilocks",
        "stark/fri/audit-proof-v1",
        "halo2/ipa:production-ready",
        "halo2/ipa:claimed-production",
        "halo2/ipa:mainnet-ready",
        "stark/fri/audit-signoff",
        "stark/fri/externally-audited",
        "stark/fri/security-review-passed",
        "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
        "stark/fri/a-u-d-i-t-c-l-a-i-m",
        "halo2/ipa/penumbra",
        "halo2/ipa/masp",
        "halo2/ipa/monero",
        "halo2/ipa/curve-tree",
        "halo2/pasta/tiny-add",
        "halo2/ipa/tiny-add",
        "halo2/ipa:tiny-add",
        "halo2/pasta/tiny-commit-open",
        "halo2/pasta/anon-transfer-2x2",
        "halo2/ipa/anon-transfer-2x2",
        "halo2/ipa:anon-transfer-2x2",
        "halo2/pasta/anon-transfer-2x2-merkle2",
        "halo2/ipa/anon-transfer-2x2-merkle8",
        "halo2/ipa:anon-transfer-2x2-merkle16",
        "halo2/pasta/vote-bool-commit",
        "halo2/ipa/vote-bool-commit",
        "halo2/ipa:vote-bool-commit",
        "halo2/pasta/vote-bool-commit-merkle2",
        "halo2/ipa/vote-bool-commit-merkle8",
        "halo2/ipa:vote-bool-commit-merkle16",
        "halo2/pasta/asset-hidden-transfer-public-test",
        "halo2/ipa/asset-hidden-transfer-public-test",
        "halo2/ipa:asset-hidden-transfer-public-test",
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/dev",
        "stark/fri/d-e-v",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/placeholder",
        " stark/fri/sha256-goldilocks",
        "stark/fri/sha256-goldilocks ",
        "halo2/ipa/orchard",
        "halo2/kzg",
        "halo2/ipa\0",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dev",
        "halo2/ipa:d-e-v",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/ipa:s-a-m-p-l-e",
        "mock/dev",
    ):
        with pytest.raises(ValueError, match="unsupported production verifier backend"):
            client.submit_zk_verifying_key_registration(
                {"backend": backend, "name": "vk_transfer"}
            )
    assert session.calls == []


def test_zk_verifying_key_registration_rejects_bad_names_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    for bad_name in ("", "   ", "\t", None, 7):
        with pytest.raises((TypeError, ValueError), match="register_zk_verifying_key.name"):
            client.submit_zk_verifying_key_registration(
                {"backend": "halo2/ipa", "name": bad_name}
            )

    assert session.calls == []


def test_zk_verifying_key_registration_rejects_missing_signing_fields_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    for authority in (None, "", "   ", 7):
        with pytest.raises((TypeError, ValueError), match="register_zk_verifying_key.authority"):
            client.submit_zk_verifying_key_registration(
                {
                    "authority": authority,
                    "backend": "halo2/ipa",
                    "name": "vk_transfer",
                    "private_key": "ed25519:deadbeef",
                }
            )

    for private_key in (None, "", "   ", 7):
        with pytest.raises((TypeError, ValueError), match="register_zk_verifying_key.private_key"):
            client.submit_zk_verifying_key_registration(
                {
                    "authority": "alice",
                    "backend": "halo2/ipa",
                    "name": "vk_transfer",
                    "private_key": private_key,
                }
            )

    assert session.calls == []


def test_zk_verifying_key_registration_rejects_mismatched_inline_commitment() -> None:
    session = FakeSession([response(202, {"accepted": True})])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    vk_bytes = b"abc"
    matching_commitment = zk_verifying_key_commitment("halo2/ipa", vk_bytes)

    with pytest.raises(ValueError, match="commitment_hex must match domain-separated SHA-256"):
        client.submit_zk_verifying_key_registration(
            {
                "authority": "alice",
                "backend": "halo2/ipa",
                "name": "vk_transfer",
                "private_key": "ed25519:deadbeef",
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "public_inputs_schema_hash_hex": "aa" * 32,
                "gas_schedule_id": "halo2_default",
                "vk_bytes": base64.b64encode(vk_bytes).decode("ascii"),
                "commitment_hex": "00" * 32,
            }
        )

    assert session.calls == []
    response_obj = client.submit_zk_verifying_key_registration(
            {
                "authority": "alice",
                "backend": "halo2/ipa",
                "name": "vk_transfer",
                "private_key": "ed25519:deadbeef",
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "public_inputs_schema_hash_hex": "aa" * 32,
                "gas_schedule_id": "halo2_default",
                "vk_bytes": vk_bytes,
                "commitment_hex": matching_commitment,
            }
    )

    assert response_obj.status_code == 202
    assert [call["path"] for call in session.calls] == ["/v1/zk/vk/register"]


def test_zk_verifying_key_registration_rejects_withdraw_height_before_activation_height() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(ValueError, match="withdraw_height"):
        client.submit_zk_verifying_key_registration(
            {
                "authority": "alice",
                "backend": "halo2/ipa",
                "name": "vk_transfer",
                "private_key": "ed25519:deadbeef",
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "public_inputs_schema_hash_hex": "aa" * 32,
                "gas_schedule_id": "halo2_default",
                "activation_height": 10,
                "withdraw_height": 9,
            }
        )
    with pytest.raises(ValueError, match="activation_height"):
        client.submit_zk_verifying_key_registration(
            {
                "authority": "alice",
                "backend": "halo2/ipa",
                "name": "vk_transfer",
                "private_key": "ed25519:deadbeef",
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "public_inputs_schema_hash_hex": "aa" * 32,
                "gas_schedule_id": "halo2_default",
                "activation_height": -1,
            }
        )
    assert session.calls == []


def test_zk_verifying_key_update_helper_posts_signed_payload() -> None:
    session = FakeSession([response(202, {"accepted": True})])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    vk_bytes = b"abc"
    matching_commitment = zk_verifying_key_commitment("halo2/ipa", vk_bytes)

    decoded = client.update_zk_verifying_key(
        {
            "authority": " alice ",
            "backend": "halo2/ipa",
            "name": "vk_transfer",
            "private_key": " ed25519:deadbeef ",
            "version": 2,
            "circuit_id": "halo2/ipa::transfer_v2",
            "public_inputs_schema_hash_hex": "aa" * 32,
            "gas_schedule_id": "halo2_default",
            "vk_bytes": vk_bytes,
            "commitment_hex": matching_commitment,
            "activation_height": "10",
            "withdraw_height": "10",
        }
    )

    assert decoded == {"accepted": True}
    assert [call["path"] for call in session.calls] == ["/v1/zk/vk/update"]
    call = session.calls[0]
    assert call["method"] == "POST"
    body = json.loads(call["data"])
    assert body["authority"] == "alice"
    assert body["private_key"] == "ed25519:deadbeef"
    assert body["backend"] == "halo2/ipa"
    assert body["name"] == "vk_transfer"
    assert body["vk_bytes"] == base64.b64encode(vk_bytes).decode("ascii")
    assert body["commitment_hex"] == matching_commitment
    assert body["activation_height"] == 10
    assert body["withdraw_height"] == 10


def test_zk_verifying_key_update_rejects_bad_inputs_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    vk_bytes = b"abc"
    matching_commitment = zk_verifying_key_commitment("halo2/ipa", vk_bytes)

    def payload(**overrides: object) -> dict[str, object]:
        base: dict[str, object] = {
            "authority": "alice",
            "backend": "halo2/ipa",
            "name": "vk_transfer",
            "private_key": "ed25519:deadbeef",
            "version": 2,
            "circuit_id": "halo2/ipa::transfer_v2",
            "public_inputs_schema_hash_hex": "aa" * 32,
            "vk_bytes": base64.b64encode(vk_bytes).decode("ascii"),
            "commitment_hex": matching_commitment,
        }
        base.update(overrides)
        return base

    for backend in (
        " halo2/ipa",
        "halo2/ipa ",
        "halo2/ipa\n",
        "halo2\uFF0Fipa",
        "halo2/\u200Bipa",
        "h\u0430lo2/ipa",
        "stark/fri/miden",
        "stark/fri/latest",
        "halo2/kzg",
        "mock/dev",
    ):
        with pytest.raises(ValueError, match="update_zk_verifying_key.backend"):
            client.submit_zk_verifying_key_update(payload(backend=backend))

    for bad_name in ("", "   ", "\t", None, 7):
        with pytest.raises((TypeError, ValueError), match="update_zk_verifying_key.name"):
            client.submit_zk_verifying_key_update(payload(name=bad_name))

    for authority in (None, "", "   ", 7):
        with pytest.raises((TypeError, ValueError), match="update_zk_verifying_key.authority"):
            client.submit_zk_verifying_key_update(payload(authority=authority))

    for private_key in (None, "", "   ", 7):
        with pytest.raises((TypeError, ValueError), match="update_zk_verifying_key.private_key"):
            client.submit_zk_verifying_key_update(payload(private_key=private_key))

    with pytest.raises(TypeError, match="update_zk_verifying_key.vk_bytes"):
        client.submit_zk_verifying_key_update(payload(vk_bytes=7))

    with pytest.raises(ValueError, match="update_zk_verifying_key.vk_bytes"):
        client.submit_zk_verifying_key_update(payload(vk_bytes="not base64!"))

    with pytest.raises(ValueError, match="commitment_hex must match domain-separated SHA-256"):
        client.submit_zk_verifying_key_update(payload(commitment_hex="00" * 32))

    with pytest.raises(ValueError, match="commitment_hex"):
        client.submit_zk_verifying_key_update(
            payload(vk_bytes=None, vk_len=3, commitment_hex=None)
        )

    with pytest.raises(ValueError, match="withdraw_height"):
        client.submit_zk_verifying_key_update(payload(activation_height=10, withdraw_height=9))

    with pytest.raises(ValueError, match="activation_height"):
        client.submit_zk_verifying_key_update(payload(activation_height=-1))

    assert session.calls == []


def test_zk_verifying_key_read_helpers_reject_unsupported_backends_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    for backend in (
        " halo2/ipa",
        "halo2/ipa ",
        "\thalo2/ipa",
        "halo2/ipa\n",
        "halo2\uFF0Fipa",
        "halo2/\u200Bipa",
        "h\u0430lo2/ipa",
        "stark/fri/miden",
        "stark/fri/latest",
        "stark/fri/attestation",
        "stark/fri/contest",
        "stark/fri/random-profile",
        "stark/fri/sha512-goldilocks",
        "stark/fri/audit-proof-v1",
        "halo2/ipa:production-ready",
        "halo2/ipa:claimed-production",
        "halo2/ipa:mainnet-ready",
        "stark/fri/audit-signoff",
        "stark/fri/externally-audited",
        "stark/fri/security-review-passed",
        "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
        "stark/fri/a-u-d-i-t-c-l-a-i-m",
        "halo2/ipa/penumbra",
        "halo2/ipa/masp",
        "halo2/ipa/monero",
        "halo2/ipa/curve-tree",
        "halo2/pasta/tiny-add",
        "halo2/ipa/tiny-add",
        "halo2/ipa:tiny-add",
        "halo2/pasta/tiny-commit-open",
        "halo2/pasta/anon-transfer-2x2",
        "halo2/ipa/anon-transfer-2x2",
        "halo2/ipa:anon-transfer-2x2",
        "halo2/pasta/anon-transfer-2x2-merkle2",
        "halo2/ipa/anon-transfer-2x2-merkle8",
        "halo2/ipa:anon-transfer-2x2-merkle16",
        "halo2/pasta/vote-bool-commit",
        "halo2/ipa/vote-bool-commit",
        "halo2/ipa:vote-bool-commit",
        "halo2/pasta/vote-bool-commit-merkle2",
        "halo2/ipa/vote-bool-commit-merkle8",
        "halo2/ipa:vote-bool-commit-merkle16",
        "halo2/pasta/asset-hidden-transfer-public-test",
        "halo2/ipa/asset-hidden-transfer-public-test",
        "halo2/ipa:asset-hidden-transfer-public-test",
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/dev",
        "stark/fri/d-e-v",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/placeholder",
        " stark/fri/sha256-goldilocks",
        "stark/fri/sha256-goldilocks ",
        "halo2/ipa/orchard",
        "halo2/kzg",
        "halo2/ipa\0",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dev",
        "halo2/ipa:d-e-v",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/ipa:s-a-m-p-l-e",
        "mock/dev",
    ):
        with pytest.raises(ValueError, match="unsupported production verifier backend"):
            client.request_zk_verifying_key(backend, "vk_transfer")
        with pytest.raises(ValueError, match="unsupported production verifier backend"):
            client.zk_verifying_key_active(backend, "vk_transfer")
    assert session.calls == []


def test_zk_event_filters_reject_unsupported_backends_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    for backend in (
        " halo2/ipa",
        "halo2/ipa ",
        "\thalo2/ipa",
        "halo2/ipa\n",
        "halo2\uFF0Fipa",
        "halo2/\u200Bipa",
        "h\u0430lo2/ipa",
        "stark/fri/miden",
        "stark/fri/latest",
        "stark/fri/attestation",
        "stark/fri/contest",
        "stark/fri/random-profile",
        "stark/fri/sha512-goldilocks",
        "stark/fri/audit-proof-v1",
        "halo2/ipa:production-ready",
        "halo2/ipa:claimed-production",
        "halo2/ipa:mainnet-ready",
        "stark/fri/audit-signoff",
        "stark/fri/externally-audited",
        "stark/fri/security-review-passed",
        "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
        "stark/fri/a-u-d-i-t-c-l-a-i-m",
        "halo2/ipa/penumbra",
        "halo2/ipa/masp",
        "halo2/ipa/monero",
        "halo2/ipa/curve-tree",
        "halo2/pasta/tiny-add",
        "halo2/ipa/tiny-add",
        "halo2/ipa:tiny-add",
        "halo2/pasta/tiny-commit-open",
        "halo2/pasta/anon-transfer-2x2",
        "halo2/ipa/anon-transfer-2x2",
        "halo2/ipa:anon-transfer-2x2",
        "halo2/pasta/anon-transfer-2x2-merkle2",
        "halo2/ipa/anon-transfer-2x2-merkle8",
        "halo2/ipa:anon-transfer-2x2-merkle16",
        "halo2/pasta/vote-bool-commit",
        "halo2/ipa/vote-bool-commit",
        "halo2/ipa:vote-bool-commit",
        "halo2/pasta/vote-bool-commit-merkle2",
        "halo2/ipa/vote-bool-commit-merkle8",
        "halo2/ipa:vote-bool-commit-merkle16",
        "halo2/pasta/asset-hidden-transfer-public-test",
        "halo2/ipa/asset-hidden-transfer-public-test",
        "halo2/ipa:asset-hidden-transfer-public-test",
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/dev",
        "stark/fri/d-e-v",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/placeholder",
        " stark/fri/sha256-goldilocks",
        "stark/fri/sha256-goldilocks ",
        "halo2/ipa/orchard",
        "halo2/kzg",
        "halo2/ipa\0",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dev",
        "halo2/ipa:d-e-v",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/ipa:s-a-m-p-l-e",
        "mock/dev",
    ):
        with pytest.raises(ValueError, match="unsupported production verifier backend"):
            DataEventFilter.verifying_key(backend=backend, name="vk_transfer")
        with pytest.raises(ValueError, match="unsupported production verifier backend"):
            DataEventFilter.proof(backend=backend, proof_hash_hex="a" * 64)
        with pytest.raises(ValueError, match="unsupported production verifier backend"):
            client.stream_verifying_key_events(backend=backend, name="vk_transfer")
        with pytest.raises(ValueError, match="unsupported production verifier backend"):
            client.stream_proof_events(backend=backend, proof_hash_hex="a" * 64)
    assert session.calls == []


def test_zk_verifying_key_event_filters_reject_malformed_names_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    for name in ("", "   ", "\t", "\n", "vk:transfer", 42):
        with pytest.raises((TypeError, ValueError), match="verifying_key_filter.name"):
            DataEventFilter.verifying_key(backend="halo2/ipa", name=name)
        with pytest.raises((TypeError, ValueError), match="verifying_key_filter.name"):
            client.stream_verifying_key_events(backend="halo2/ipa", name=name)

    payload = DataEventFilter.verifying_key(
        backend="halo2/ipa",
        name=" vk_transfer ",
    ).to_dict()
    assert payload["VerifyingKey"]["id_matcher"]["name"] == "vk_transfer"
    assert session.calls == []


def test_zk_proof_event_filters_reject_malformed_hashes_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    for proof_hash_hex in (
        "",
        "abc",
        "z" * 64,
        "a" * 63,
        "0x" + "a" * 63,
    ):
        with pytest.raises((TypeError, ValueError), match="32-byte hex string"):
            DataEventFilter.proof(backend="halo2/ipa", proof_hash_hex=proof_hash_hex)
        with pytest.raises((TypeError, ValueError), match="32-byte hex string"):
            client.stream_proof_events(backend="halo2/ipa", proof_hash_hex=proof_hash_hex)

    payload = DataEventFilter.proof(
        backend="halo2/ipa",
        proof_hash_hex="0x" + "A" * 64,
    ).to_dict()
    assert payload["Proof"]["id_matcher"]["hash_hex"] == "a" * 64
    assert session.calls == []


def test_zk_raw_event_filters_reject_malformed_privacy_matchers_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    raw_filters = [
        {
            "VerifyingKey": {
                "id_matcher": {"backend": "halo2/ipa/orchard", "name": "vk_transfer"},
                "event_set": {"Registered": True},
            }
        },
        {
            "VerifyingKey": {
                "id_matcher": {"backend": "halo2/ipa", "name": "vk:transfer"},
                "event_set": {"Registered": True},
            }
        },
        {
            "VerifyingKey": {
                "id_matcher": {"backend": "halo2/ipa", "name": 42},
                "event_set": {"Registered": True},
            }
        },
        {
            "Proof": {
                "id_matcher": {"backend": "mock/dev", "hash_hex": "a" * 64},
                "event_set": {"Verified": True},
            }
        },
        {
            "Proof": {
                "id_matcher": {"backend": "groth16/bls12-377", "hash_hex": "a" * 64},
                "event_set": {"Verified": True},
            }
        },
        {
            "Proof": {
                "id_matcher": {"backend": "halo2/ipa", "hash_hex": "z" * 64},
                "event_set": {"Verified": True},
            }
        },
    ]

    for raw_filter in raw_filters:
        with pytest.raises((TypeError, ValueError)):
            client.stream_events(filter=raw_filter)
        with pytest.raises((TypeError, ValueError)):
            client.stream_events(filter=json.dumps(raw_filter))

    with pytest.raises(ValueError, match="data_event_filter.VerifyingKey.id_matcher.name"):
        DataEventFilter(
            {
                "VerifyingKey": {
                    "id_matcher": {"backend": "halo2/ipa", "name": "vk:transfer"},
                    "event_set": {"Registered": True},
                }
            }
        )
    assert session.calls == []


def test_zk_raw_event_filters_canonicalize_privacy_matchers_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    captured_params = []

    def capture_stream(path, **kwargs):
        captured_params.append(kwargs.get("params"))
        return iter(())

    client._stream_sse = capture_stream

    client.stream_events(
        filter={
            "VerifyingKey": {
                "id_matcher": {"backend": "halo2/ipa", "name": " vk_transfer "},
                "event_set": {"Registered": True},
            }
        }
    )
    encoded_vk_filter = captured_params[-1]["filter"]
    decoded_vk_filter = json.loads(encoded_vk_filter)
    assert decoded_vk_filter["VerifyingKey"]["id_matcher"]["name"] == "vk_transfer"

    client.stream_events(
        filter=json.dumps(
            {
                "Proof": {
                    "id_matcher": {
                        "backend": "halo2/ipa",
                        "hash_hex": "0x" + "A" * 64,
                        "proof_hash_hex": "0x" + "B" * 64,
                    },
                    "event_set": {"Verified": True},
                }
            }
        )
    )
    encoded_proof_filter = captured_params[-1]["filter"]
    decoded_proof_filter = json.loads(encoded_proof_filter)
    proof_matcher = decoded_proof_filter["Proof"]["id_matcher"]
    assert proof_matcher["hash_hex"] == "a" * 64
    assert proof_matcher["proof_hash_hex"] == "b" * 64

    assert session.calls == []


def test_account_has_permission_uses_typed_permission_listing() -> None:
    session = FakeSession(
        [
            response(
                200,
                {
                    "items": [
                        {
                            "name": "register_zk_asset",
                            "payload": {"asset_definition_id": "ds#wonderland.is"},
                        }
                    ],
                    "total": 1,
                },
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.account_has_permission(
        "asset-owner@is",
        "register_zk_asset",
        expected_payload={"asset_definition_id": "ds#wonderland.is"},
    )


def test_account_permission_listing_accepts_configured_chain_discriminant() -> None:
    taira_account = account_address(5, 0x0171)
    session = FakeSession([response(200, {"items": [], "total": 0})])
    client = ToriiClient(
        "http://torii.example",
        session=session,
        max_retries=0,
        chain_discriminant=0x0171,
    )

    assert client.list_account_permissions(taira_account) == {"items": [], "total": 0}
    assert session.calls == [
        {
            "method": "GET",
            "path": f"/v1/accounts/{quote(taira_account, safe='')}/permissions",
            "params": None,
            "data": None,
        }
    ]


def test_account_permission_listing_rejects_foreign_chain_discriminant() -> None:
    taira_account = account_address(6, 0x0171)
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises(
        ValueError,
        match="account_id must be a canonical I105 account id or on-chain account alias",
    ):
        client.list_account_permissions(taira_account)
    assert session.calls == []


def test_deploy_contract_bundle_reads_code_file_and_waits(tmp_path) -> None:
    code_file = tmp_path / "contract.to"
    code_file.write_bytes(b"contract-code")
    tx_hash = "a" * 64
    session = FakeSession(
        [
            response(
                200,
                {
                    "ok": True,
                    "bundle_name": "single",
                    "bundle_digest": "digest",
                    "chain_fingerprint": "chain",
                    "dry_run": False,
                    "completed_stages": [],
                    "contracts": [
                        {
                            "name": "contract",
                            "contract_alias": "contract::is",
                            "contract_address": "addr",
                            "upgraded": False,
                            "tx_hash_hex": tx_hash,
                            "code_hash_hex": "b" * 64,
                            "abi_hash_hex": "c" * 64,
                            "status": "submitted",
                        }
                    ],
                    "init_calls": [],
                    "assertions": [],
                },
            ),
            response(200, {"status": {"kind": "Applied"}, "hash": tx_hash}),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    result = client.deploy_contract_bundle(
        authority="authority@is",
        private_key="priv",
        contract_alias="contract::is",
        code_file=code_file,
        wait=True,
        timeout_ms=1000,
        interval=0,
    )

    assert result["terminal_kind"] == "Applied"
    assert result["tx_hashes"] == [tx_hash]
    deploy_payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert deploy_payload["code_b64"] == "Y29udHJhY3QtY29kZQ=="


def test_call_contract_and_wait_posts_typed_request() -> None:
    tx_hash = "d" * 64
    session = FakeSession(
        [
            response(
                200,
                {
                    "ok": True,
                    "submitted": True,
                    "dataspace": "is",
                    "code_hash_hex": "b" * 64,
                    "abi_hash_hex": "c" * 64,
                    "creation_time_ms": 1,
                    "contract_alias": "contract::is",
                    "tx_hash_hex": tx_hash,
                    "entrypoint": "main",
                },
            ),
            response(200, {"status": {"kind": "Committed"}, "hash": tx_hash}),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    result = client.call_contract_and_wait(
        authority="authority@is",
        private_key="priv",
        contract_alias="contract::is",
        entrypoint="main",
        payload={"amount": 7},
        gas_limit=5000,
        wait=True,
        timeout_ms=1000,
        interval=0,
    )

    assert result["terminal_kind"] == "Committed"
    call_payload = json.loads(session.calls[0]["data"].decode("utf-8"))
    assert call_payload["payload"] == {"amount": 7}


def test_mint_assets_and_wait_batches_records_in_one_transaction() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    captured: dict[str, object] = {}
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    adult = AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x11] * 32),
    ).to_i105(0x02F1)
    business = AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x22] * 32),
    ).to_i105(0x02F1)

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "mint-batch"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    result = client.mint_assets_and_wait(
        chain_id="chain",
        authority="authority@is",
        private_key_hex="11" * 32,
        mints=[
            {"asset_id": f"{asset_definition_id}#{adult}", "quantity": "1.25"},
            {"asset_id": f"{asset_definition_id}#{business}", "quantity": 2},
        ],
        transaction_metadata={"purpose": "batch"},
        wait=False,
    )

    draft = captured["draft"]
    assert result == {"hash": "mint-batch"}
    assert len(draft) == 2
    assert draft.config.metadata == {"purpose": "batch"}
    assert captured["kwargs"]["private_key_hex"] == "11" * 32
    assert captured["kwargs"]["wait"] is False


def test_transfer_assets_and_wait_batches_records_in_one_transaction() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    captured: dict[str, object] = {}
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    source = AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x11] * 32),
    ).to_i105(0x02F1)
    dest = AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x22] * 32),
    ).to_i105(0x02F1)
    fees = AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x33] * 32),
    ).to_i105(0x02F1)

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "transfer-batch"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    result = client.transfer_assets_and_wait(
        chain_id="chain",
        authority="source@is",
        private_key_hex="22" * 32,
        transfers=[
            {
                "asset_id": f"{asset_definition_id}#{source}",
                "quantity": Decimal("3"),
                "destination": dest,
            },
            {
                "asset_id": f"{asset_definition_id}#{source}",
                "quantity": "0.1",
                "destination": fees,
            },
        ],
        wait=True,
    )

    draft = captured["draft"]
    assert result == {"hash": "transfer-batch"}
    assert len(draft) == 2
    assert draft.config.authority == "source@is"
    assert captured["kwargs"]["private_key_hex"] == "22" * 32
    assert captured["kwargs"]["wait"] is True


def test_permission_grant_and_revoke_helpers_build_one_instruction() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    captured: list[tuple[object, dict[str, object]]] = []
    account = account_address(0x44)

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured.append((draft, kwargs))
        return {"hash": f"permission-{len(captured)}"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    grant = client.grant_account_permission_and_wait(
        chain_id="chain",
        authority="authority@is",
        private_key_hex="11" * 32,
        account_id=account,
        permission_name="CanUseFeeSponsor",
        permission_payload={"sponsor": "sponsor@is"},
        transaction_metadata={"purpose": "fee-sponsor"},
        wait=False,
    )
    revoke = client.revoke_account_permission_and_wait(
        chain_id="chain",
        authority="authority@is",
        private_key_hex="22" * 32,
        account_id=account,
        permission_name="CanUseFeeSponsor",
        permission_payload={"sponsor": "sponsor@is"},
        wait=True,
    )

    assert grant == {"hash": "permission-1"}
    assert revoke == {"hash": "permission-2"}
    grant_draft, grant_kwargs = captured[0]
    revoke_draft, revoke_kwargs = captured[1]
    assert len(grant_draft) == 1
    assert len(revoke_draft) == 1
    assert grant_draft.config.metadata == {"purpose": "fee-sponsor"}
    assert grant_kwargs["private_key_hex"] == "11" * 32
    assert grant_kwargs["wait"] is False
    assert revoke_kwargs["private_key_hex"] == "22" * 32
    assert revoke_kwargs["wait"] is True


def test_permission_grant_accepts_configured_chain_discriminant_account_ids() -> None:
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([]),
        max_retries=0,
        chain_discriminant=0x0171,
    )
    captured: dict[str, object] = {}
    account = account_address(0x45, 0x0171)
    native_account = account_address(0x45)

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "permission-taira"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    assert client.grant_account_permission_and_wait(
        chain_id="chain",
        authority=account,
        private_key_hex="11" * 32,
        account_id=account,
        permission_name="CanUseFeeSponsor",
        permission_payload={"sponsor": "sponsor@is"},
        wait=False,
    ) == {"hash": "permission-taira"}

    draft = captured["draft"]
    assert draft.config.authority == native_account
    assert len(draft) == 1
    assert captured["kwargs"]["private_key_hex"] == "11" * 32


def test_transfer_helper_accepts_configured_chain_discriminant_account_ids() -> None:
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([]),
        max_retries=0,
        chain_discriminant=0x0171,
    )
    captured: dict[str, object] = {}
    source = account_address(0x46, 0x0171)
    destination = account_address(0x47, 0x0171)
    native_source = account_address(0x46)
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "transfer-taira"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    assert client.transfer_asset_and_wait(
        chain_id="chain",
        authority=source,
        private_key_hex="22" * 32,
        asset_id=f"{asset_definition_id}#{source}",
        destination=destination,
        quantity=Decimal("3"),
        wait=False,
    ) == {"hash": "transfer-taira"}

    draft = captured["draft"]
    assert draft.config.authority == native_source
    assert len(draft) == 1
    assert captured["kwargs"]["private_key_hex"] == "22" * 32


def test_zk_instruction_helpers_serialize_full_surface() -> None:
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    source = account_address(0x61)
    destination = account_address(0x62)
    zk_ace_verifier = "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0"
    proof = {
        "backend": "halo2/ipa",
        "proof_bytes": b"proof-bytes",
        "verifying_key_ref": "halo2/ipa:vk_transfer",
        "verifying_key_commitment": "44" * 32,
        "envelope_hash": "55" * 32,
    }

    instructions = [
        Instruction.register_zk_asset(
            asset_definition_id,
            vk_transfer="halo2/ipa:vk_transfer",
            vk_unshield={"backend": "halo2/ipa", "name": "vk_unshield"},
        ),
        Instruction.shield_asset(
            asset_definition_id,
            source,
            "7",
            "11" * 32,
            "22" * 32,
            "33" * 24,
            b"ciphertext",
        ),
        Instruction.zk_transfer_prepared(
            asset_definition_id,
            ["aa" * 32],
            ["bb" * 32],
            proof,
            root_hint="cc" * 32,
        ),
        Instruction.unshield_prepared(
            asset_definition_id,
            destination,
            "3",
            ["dd" * 32],
            proof,
            outputs=["ee" * 32],
            root_hint="ff" * 32,
        ),
        Instruction.register_zk_ace_identity_commitment(
            asset_definition_id,
            "11" * 32,
            "22" * 32,
            [source],
            zk_ace_verifier,
        ),
        Instruction.rotate_zk_ace_identity_commitment(
            asset_definition_id,
            "11" * 32,
            "12" * 32,
            "22" * 32,
            [source],
            zk_ace_verifier,
        ),
    ]

    encoded = [instruction.to_json() for instruction in instructions]
    assert all(payload for payload in encoded)
    assert [Instruction.from_json(payload).to_json() for payload in encoded] == encoded

    alternate_source = account_address(0x63)
    alternate_register = Instruction.register_zk_ace_identity_commitment(
        asset_definition_id,
        "11" * 32,
        "22" * 32,
        [alternate_source],
        zk_ace_verifier,
    )
    alternate_rotate = Instruction.rotate_zk_ace_identity_commitment(
        asset_definition_id,
        "11" * 32,
        "12" * 32,
        "22" * 32,
        [alternate_source],
        zk_ace_verifier,
    )
    assert alternate_register.to_json() != instructions[-2].to_json()
    assert alternate_rotate.to_json() != instructions[-1].to_json()


def test_zk_instruction_helpers_accept_tuple_inputs() -> None:
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    proof = {
        "backend": "halo2/ipa",
        "proof_bytes": b"proof-bytes",
        "verifying_key_ref": "halo2/ipa:vk_transfer",
    }

    instruction = Instruction.zk_transfer_prepared(
        asset_definition_id,
        ("aa" * 32,),
        ("bb" * 32,),
        proof,
        root_hint="cc" * 32,
    )

    assert Instruction.from_json(instruction.to_json()).to_json() == instruction.to_json()


def test_transaction_draft_shield_accepts_raw_text_ciphertext() -> None:
    draft = TransactionDraft(
        TransactionConfig(
            chain_id="chain",
            authority=account_address(0x65),
        )
    )

    draft.shield_asset(
        "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
        account_address(0x65),
        7,
        note_commitment="11" * 32,
        ephemeral_public_key="22" * 32,
        nonce="33" * 24,
        ciphertext="raw ciphertext payload",
    )

    assert len(draft) == 1


def test_zk_instruction_helpers_reject_invalid_prepared_proof() -> None:
    with pytest.raises(ValueError, match="vk_ref.backend"):
        Instruction.zk_transfer_prepared(
            "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
            ["aa" * 32],
            ["bb" * 32],
            {
                "backend": "halo2/ipa",
                "proof_bytes": b"proof-bytes",
                "verifying_key_ref": "other:vk_transfer",
            },
        )


@pytest.mark.parametrize(
    ("factory", "error_type", "match"),
    [
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.register_zk_asset(
                asset,
                mode="../../Hybrid",
            ),
            ValueError,
            "invalid ZK asset mode",
            id="register-invalid-mode",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.register_zk_asset(
                asset,
                vk_transfer="halo2/ipa",
            ),
            ValueError,
            "backend:name",
            id="register-invalid-vk-format",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.shield_asset(
                asset,
                source,
                "-1",
                "11" * 32,
                "22" * 32,
                "33" * 24,
                b"ciphertext",
            ),
            ValueError,
            "amount",
            id="shield-negative-amount",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.shield_asset(
                asset,
                source,
                "1.5",
                "11" * 32,
                "22" * 32,
                "33" * 24,
                b"ciphertext",
            ),
            ValueError,
            "amount",
            id="shield-decimal-amount",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.shield_asset(
                asset,
                source,
                str(2**128),
                "11" * 32,
                "22" * 32,
                "33" * 24,
                b"ciphertext",
            ),
            ValueError,
            "amount",
            id="shield-u128-overflow",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.shield_asset(
                asset,
                source,
                "1",
                "11" * 31,
                "22" * 32,
                "33" * 24,
                b"ciphertext",
            ),
            ValueError,
            "note_commitment",
            id="shield-short-commitment",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.shield_asset(
                asset,
                source,
                "1",
                "11" * 32,
                "22" * 32,
                "33" * 23,
                b"ciphertext",
            ),
            ValueError,
            "nonce",
            id="shield-short-nonce",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.shield_asset(
                asset,
                source,
                "1",
                "11" * 32,
                "22" * 32,
                "33" * 24,
                b"",
            ),
            ValueError,
            "ciphertext",
            id="shield-empty-ciphertext",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                [],
                ["bb" * 32],
                proof,
            ),
            ValueError,
            "inputs",
            id="transfer-empty-inputs",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                [],
                proof,
            ),
            ValueError,
            "outputs",
            id="transfer-empty-outputs",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                "not-a-list",
                ["bb" * 32],
                proof,
            ),
            TypeError,
            "list or tuple",
            id="transfer-inputs-not-sequence",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32, "aa" * 32],
                ["bb" * 32],
                proof,
            ),
            ValueError,
            "duplicates",
            id="transfer-duplicate-nullifier",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                ["bb" * 32, "bb" * 32],
                proof,
            ),
            ValueError,
            "duplicates",
            id="transfer-duplicate-commitment",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                ["bb" * 32],
                proof,
                root_hint="cc" * 31,
            ),
            ValueError,
            "root_hint",
            id="transfer-short-root",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                ["bb" * 32],
                {key: value for key, value in proof.items() if key != "backend"},
            ),
            ValueError,
            "backend",
            id="transfer-missing-proof-backend",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                ["bb" * 32],
                {key: value for key, value in proof.items() if key != "proof_bytes"},
            ),
            ValueError,
            "proof_bytes",
            id="transfer-missing-proof-bytes",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                ["bb" * 32],
                {**proof, "proof_bytes": b""},
            ),
            ValueError,
            "proof_bytes",
            id="transfer-empty-proof",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                ["bb" * 32],
                {**proof, "proof_b64": "not base64!", "proof_bytes": None},
            ),
            ValueError,
            "base64",
            id="transfer-invalid-base64-proof",
        ),
        pytest.param(
            lambda asset, _source, destination, proof: Instruction.unshield_prepared(
                asset,
                destination,
                "-1",
                ["aa" * 32],
                {**proof, "verifying_key_ref": "halo2/ipa:vk_unshield"},
            ),
            ValueError,
            "public_amount",
            id="unshield-negative-public-amount",
        ),
        pytest.param(
            lambda asset, _source, destination, proof: Instruction.unshield_prepared(
                asset,
                destination,
                "1",
                ["aa" * 32],
                {**proof, "verifying_key_ref": "halo2/ipa:vk_unshield"},
                outputs=["bb" * 32, "bb" * 32],
            ),
            ValueError,
            "duplicates",
            id="unshield-duplicate-output",
        ),
        pytest.param(
            lambda asset, source, destination, _proof: Instruction.zk_ace_authorized_transfer(
                source,
                destination,
                asset,
                "1",
                "11" * 32,
                "22" * 32,
                "chain",
                "iroha:zk-ace:pq-authorization:v0",
                "transparent_asset_transfer",
                "33" * 32,
                "44" * 32,
                {
                    "backend": "halo2/ipa",
                    "proof_bytes": b"proof-bytes",
                    "verifying_key_ref": "halo2/ipa:vk_wrong",
                },
            ),
            ValueError,
            "stark/fri/sha256-goldilocks",
            id="zk-ace-transfer-wrong-proof-backend",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.register_zk_ace_identity_commitment(
                asset,
                "00" * 32,
                "22" * 32,
                [source],
                "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            ),
            ValueError,
            "identity_commitment",
            id="zk-ace-register-zero-identity",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.rotate_zk_ace_identity_commitment(
                asset,
                "11" * 32,
                "11" * 32,
                "22" * 32,
                [source],
                "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            ),
            ValueError,
            "must differ",
            id="zk-ace-rotate-same-identity",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.register_zk_ace_identity_commitment(
                asset,
                "11" * 32,
                "22" * 32,
                [source],
                "halo2/ipa:zk_ace_pq_authorization_v0",
            ),
            ValueError,
            "stark/fri/sha256-goldilocks",
            id="zk-ace-register-wrong-verifier-backend",
        ),
        pytest.param(
            lambda asset, _source, _destination, _proof: Instruction.register_zk_ace_identity_commitment(
                asset,
                "11" * 32,
                "22" * 32,
                [],
                "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            ),
            ValueError,
            "allowed_accounts.*non-empty",
            id="zk-ace-register-empty-allowlist",
        ),
        pytest.param(
            lambda asset, _source, _destination, _proof: Instruction.register_zk_ace_identity_commitment(
                asset,
                "11" * 32,
                "22" * 32,
                "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            ),
            TypeError,
            "allowed_accounts",
            id="zk-ace-register-requires-allowlist",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.register_zk_ace_identity_commitment(
                asset,
                "11" * 32,
                "22" * 32,
                [source, source],
                "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            ),
            ValueError,
            "duplicates",
            id="zk-ace-register-duplicate-allowlist",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.register_zk_ace_identity_commitment(
                asset,
                "11" * 32,
                "22" * 32,
                [source] * 17,
                "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            ),
            ValueError,
            "at most 16",
            id="zk-ace-register-oversized-allowlist",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.rotate_zk_ace_identity_commitment(
                asset,
                "11" * 32,
                "12" * 32,
                "22" * 32,
                [source] * 17,
                "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            ),
            ValueError,
            "at most 16",
            id="zk-ace-rotate-oversized-allowlist",
        ),
    ],
)
def test_zk_instruction_helpers_reject_adversarial_inputs(
    factory,
    error_type: type[Exception],
    match: str,
) -> None:
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    source = account_address(0x66)
    destination = account_address(0x67)
    proof = {
        "backend": "halo2/ipa",
        "proof_bytes": b"proof-bytes",
        "verifying_key_ref": "halo2/ipa:vk_transfer",
    }

    with pytest.raises(error_type, match=match):
        factory(asset_definition_id, source, destination, proof)


@pytest.mark.parametrize(
    ("call", "error_type", "match"),
    [
        pytest.param(
            lambda draft, asset, account, proof: draft.shield_asset(
                asset,
                account,
                1,
                note_commitment="11" * 32,
                ephemeral_public_key="22" * 32,
                nonce="33" * 24,
            ),
            ValueError,
            "ciphertext",
            id="shield-missing-ciphertext",
        ),
        pytest.param(
            lambda draft, asset, account, proof: draft.shield_asset(
                asset,
                account,
                1,
                note_commitment="11" * 32,
                ephemeral_public_key="22" * 32,
                nonce="33" * 24,
                ciphertext=b"raw",
                ciphertext_b64="cmF3",
            ),
            ValueError,
            "only one",
            id="shield-conflicting-ciphertext",
        ),
        pytest.param(
            lambda draft, asset, account, proof: draft.shield_asset(
                asset,
                account,
                Decimal("1.25"),
                note_commitment="11" * 32,
                ephemeral_public_key="22" * 32,
                nonce="33" * 24,
                ciphertext=b"raw",
            ),
            ValueError,
            "whole number",
            id="shield-decimal-amount",
        ),
        pytest.param(
            lambda draft, asset, account, proof: draft.zk_transfer_prepared(
                asset,
                inputs=["aa" * 32],
                outputs=["bb" * 32],
                proof=["not", "a", "mapping"],
            ),
            TypeError,
            "proof must be a mapping",
            id="transfer-proof-not-mapping",
        ),
        pytest.param(
            lambda draft, asset, account, proof: draft.unshield_prepared(
                asset,
                account,
                -1,
                inputs=["aa" * 32],
                proof={**proof, "verifying_key_ref": "halo2/ipa:vk_unshield"},
            ),
            ValueError,
            "public_amount",
            id="unshield-negative-public-amount",
        ),
        pytest.param(
            lambda draft, asset, account, proof: draft.register_zk_ace_identity_commitment(
                asset,
                identity_commitment="00" * 32,
                policy_hash="22" * 32,
                allowed_accounts=[account],
                verifier_key="stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            ),
            ValueError,
            "identity_commitment",
            id="zk-ace-register-zero-identity",
        ),
        pytest.param(
            lambda draft, asset, account, proof: draft.zk_ace_authorized_transfer(
                from_account_id=account,
                to_account_id=account_address(0x6C),
                asset_definition_id=asset,
                amount=1,
                identity_commitment="11" * 32,
                tx_digest="22" * 32,
                chain_id="chain",
                domain_tag="iroha:zk-ace:pq-authorization:v0",
                action_class="wrong-action",
                replay_nullifier="33" * 32,
                policy_hash="44" * 32,
                proof={
                    "backend": "stark/fri/sha256-goldilocks",
                    "proof_bytes": b"proof-bytes",
                    "verifying_key_ref": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
                },
            ),
            ValueError,
            "transparent_asset_transfer",
            id="zk-ace-transfer-wrong-action",
        ),
    ],
)
def test_zk_transaction_draft_rejects_invalid_inputs(
    call,
    error_type: type[Exception],
    match: str,
) -> None:
    account = account_address(0x68)
    draft = TransactionDraft(TransactionConfig(chain_id="chain", authority=account))
    proof = {
        "backend": "halo2/ipa",
        "proof_bytes": b"proof-bytes",
        "verifying_key_ref": "halo2/ipa:vk_transfer",
    }

    with pytest.raises(error_type, match=match):
        call(draft, "7MBRDd8cGFBZkFGdDMwV7S6FPwbw", account, proof)


def test_zk_client_helpers_build_transaction_drafts() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    captured: list[tuple[object, dict[str, object]]] = []
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    source = account_address(0x63)
    destination = account_address(0x64)
    proof = {
        "backend": "halo2/ipa",
        "proof_bytes": b"proof-bytes",
        "verifying_key_ref": "halo2/ipa:vk_transfer",
    }
    zk_ace_proof = {
        "backend": "stark/fri/sha256-goldilocks",
        "proof_bytes": b"zk-ace-proof",
        "verifying_key_ref": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    }
    zk_ace_verifier = "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0"

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured.append((draft, kwargs))
        return {"hash": f"zk-{len(captured)}"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    assert client.register_zk_asset_and_wait(
        chain_id="chain",
        authority="authority@is",
        private_key_hex="11" * 32,
        asset_definition_id=asset_definition_id,
        vk_transfer="halo2/ipa:vk_transfer",
        vk_unshield="halo2/ipa:vk_unshield",
        transaction_metadata={"purpose": "zk-register"},
        wait=False,
    ) == {"hash": "zk-1"}
    assert client.shield_asset_and_wait(
        chain_id="chain",
        authority=source,
        private_key_hex="22" * 32,
        asset_definition_id=asset_definition_id,
        from_account_id=source,
        amount="7",
        note_commitment="11" * 32,
        ephemeral_public_key="22" * 32,
        nonce="33" * 24,
        ciphertext_b64="Y2lwaGVydGV4dA==",
    ) == {"hash": "zk-2"}
    assert client.zk_transfer_prepared_and_wait(
        chain_id="chain",
        authority=source,
        private_key_hex="33" * 32,
        asset_definition_id=asset_definition_id,
        inputs=["aa" * 32],
        outputs=["bb" * 32],
        proof=proof,
        root_hint="cc" * 32,
    ) == {"hash": "zk-3"}
    assert client.unshield_prepared_and_wait(
        chain_id="chain",
        authority=source,
        private_key_hex="44" * 32,
        asset_definition_id=asset_definition_id,
        to_account_id=destination,
        public_amount="3",
        inputs=["dd" * 32],
        outputs=["ee" * 32],
        proof=proof,
        root_hint="ff" * 32,
    ) == {"hash": "zk-4"}
    assert client.register_zk_ace_identity_commitment_and_wait(
        chain_id="chain",
        authority=source,
        private_key_hex="55" * 32,
        asset_definition_id=asset_definition_id,
        identity_commitment="11" * 32,
        policy_hash="22" * 32,
        allowed_accounts=[source],
        verifier_key=zk_ace_verifier,
        wait=False,
    ) == {"hash": "zk-5"}
    assert client.rotate_zk_ace_identity_commitment_and_wait(
        chain_id="chain",
        authority=source,
        private_key_hex="66" * 32,
        asset_definition_id=asset_definition_id,
        old_identity_commitment="11" * 32,
        new_identity_commitment="12" * 32,
        policy_hash="22" * 32,
        allowed_accounts=[source],
        verifier_key=zk_ace_verifier,
        wait=False,
    ) == {"hash": "zk-6"}
    assert client.revoke_zk_ace_identity_commitment_and_wait(
        chain_id="chain",
        authority=source,
        private_key_hex="77" * 32,
        asset_definition_id=asset_definition_id,
        identity_commitment="12" * 32,
        reason_hash="55" * 32,
        wait=False,
    ) == {"hash": "zk-7"}
    assert client.zk_ace_authorized_transfer_and_wait(
        chain_id="chain",
        authority=source,
        private_key_hex="88" * 32,
        from_account_id=source,
        to_account_id=destination,
        asset_definition_id=asset_definition_id,
        amount="7",
        identity_commitment="11" * 32,
        tx_digest="22" * 32,
        domain_tag="iroha:zk-ace:pq-authorization:v0",
        action_class="transparent_asset_transfer",
        replay_nullifier="33" * 32,
        policy_hash="44" * 32,
        proof=zk_ace_proof,
        wait=False,
    ) == {"hash": "zk-8"}

    assert [len(draft) for draft, _kwargs in captured] == [1, 1, 1, 1, 1, 1, 1, 1]
    assert captured[0][0].config.metadata == {"purpose": "zk-register"}
    assert captured[0][1]["wait"] is False
    assert captured[1][1]["private_key_hex"] == "22" * 32


@pytest.mark.parametrize(
    ("method_name", "kwargs", "error_type", "match"),
    [
        pytest.param(
            "shield_asset_and_wait",
            {
                "asset_definition_id": "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "from_account_id": account_address(0x69),
                "amount": 1,
                "note_commitment": "11" * 32,
                "ephemeral_public_key": "22" * 32,
                "nonce": "33" * 24,
            },
            ValueError,
            "ciphertext",
            id="shield-missing-ciphertext",
        ),
        pytest.param(
            "zk_transfer_prepared_and_wait",
            {
                "asset_definition_id": "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "inputs": ["aa" * 32, "aa" * 32],
                "outputs": ["bb" * 32],
                "proof": {
                    "backend": "halo2/ipa",
                    "proof_bytes": b"proof-bytes",
                    "verifying_key_ref": "halo2/ipa:vk_transfer",
                },
            },
            ValueError,
            "duplicates",
            id="transfer-duplicate-input",
        ),
        pytest.param(
            "zk_transfer_prepared_and_wait",
            {
                "asset_definition_id": "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "inputs": ["aa" * 32],
                "outputs": ["bb" * 32],
                "proof": object(),
            },
            TypeError,
            "proof must be a mapping",
            id="transfer-proof-not-mapping",
        ),
        pytest.param(
            "unshield_prepared_and_wait",
            {
                "asset_definition_id": "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "to_account_id": account_address(0x6A),
                "public_amount": 1,
                "inputs": ["aa" * 32],
                "proof": {
                    "proof_bytes": b"proof-bytes",
                    "verifying_key_ref": "halo2/ipa:vk_unshield",
                },
            },
            ValueError,
            "backend",
            id="unshield-missing-proof-backend",
        ),
        pytest.param(
            "register_zk_ace_identity_commitment_and_wait",
            {
                "asset_definition_id": "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "identity_commitment": "11" * 32,
                "policy_hash": "22" * 32,
                "allowed_accounts": [account_address(0x6D)],
                "verifier_key": "halo2/ipa:zk_ace_pq_authorization_v0",
            },
            ValueError,
            "stark/fri/sha256-goldilocks",
            id="zk-ace-register-wrong-verifier-backend",
        ),
        pytest.param(
            "zk_ace_authorized_transfer_and_wait",
            {
                "from_account_id": account_address(0x6D),
                "to_account_id": account_address(0x6E),
                "asset_definition_id": "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "amount": 1,
                "identity_commitment": "11" * 32,
                "tx_digest": "22" * 32,
                "domain_tag": "iroha:zk-ace:pq-authorization:v0",
                "action_class": "transparent_asset_transfer",
                "replay_nullifier": "33" * 32,
                "policy_hash": "44" * 32,
                "proof": {
                    "backend": "halo2/ipa",
                    "proof_bytes": b"proof-bytes",
                    "verifying_key_ref": "halo2/ipa:vk_wrong",
                },
            },
            ValueError,
            "stark/fri/sha256-goldilocks",
            id="zk-ace-transfer-wrong-proof-backend",
        ),
    ],
)
def test_zk_client_helpers_reject_invalid_inputs_before_submission(
    method_name: str,
    kwargs: dict[str, object],
    error_type: type[Exception],
    match: str,
) -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    client._submit_transaction_draft_result = (  # type: ignore[method-assign]
        lambda *_args, **_kwargs: pytest.fail("invalid ZK helper should not submit")
    )

    with pytest.raises(error_type, match=match):
        getattr(client, method_name)(
            chain_id="chain",
            authority=account_address(0x6B),
            private_key_hex="11" * 32,
            **kwargs,
        )


@pytest.mark.parametrize(
    ("method_name", "kwargs", "error_type", "match"),
    [
        (
            "mint_assets_and_wait",
            {"mints": []},
            ValueError,
            "at least one mint record",
        ),
        (
            "mint_assets_and_wait",
            {"mints": [object()]},
            TypeError,
            r"mints\[0\] must be a mapping",
        ),
        (
            "mint_assets_and_wait",
            {"mints": [{"quantity": "1"}]},
            TypeError,
            r"mints\[0\]\.asset_id",
        ),
        (
            "mint_assets_and_wait",
            {"mints": [{"asset_id": "asset#account"}]},
            TypeError,
            r"mints\[0\]\.quantity is required",
        ),
        (
            "transfer_assets_and_wait",
            {"transfers": []},
            ValueError,
            "at least one transfer record",
        ),
        (
            "transfer_assets_and_wait",
            {"transfers": [object()]},
            TypeError,
            r"transfers\[0\] must be a mapping",
        ),
        (
            "transfer_assets_and_wait",
            {"transfers": [{"quantity": "1", "destination": account_address(0x45)}]},
            TypeError,
            r"transfers\[0\]\.asset_id",
        ),
        (
            "transfer_assets_and_wait",
            {"transfers": [{"asset_id": f'asset#{account_address(0x46)}', "quantity": "1"}]},
            TypeError,
            r"transfers\[0\]\.destination",
        ),
        (
            "transfer_assets_and_wait",
            {
                "transfers": [
                    {
                        "asset_id": f"asset#{account_address(0x47)}",
                        "destination": account_address(0x48),
                    }
                ]
            },
            TypeError,
            r"transfers\[0\]\.quantity is required",
        ),
    ],
)
def test_batch_helpers_reject_invalid_records(
    method_name: str,
    kwargs: dict[str, object],
    error_type: type[Exception],
    match: str,
) -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    client._submit_transaction_draft_result = (  # type: ignore[method-assign]
        lambda *_args, **_kwargs: pytest.fail("invalid batch should not submit")
    )
    method = getattr(client, method_name)

    with pytest.raises(error_type, match=match):
        method(
            chain_id="chain",
            authority="authority@is",
            private_key_hex="11" * 32,
            **kwargs,
        )


@pytest.mark.parametrize(
    ("kwargs", "match"),
    [
        (
            {"account_id": "adult@is", "permission_name": "CanUseFeeSponsor"},
            "invalid account id",
        ),
        (
            {"account_id": account_address(0x49), "permission_name": ""},
            "permission name",
        ),
        (
            {
                "account_id": account_address(0x4A),
                "permission_name": "CanUseFeeSponsor",
                "permission_payload": object(),
            },
            "JSON",
        ),
    ],
)
def test_permission_helpers_reject_invalid_inputs(
    kwargs: dict[str, object],
    match: str,
) -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    client._submit_transaction_draft_result = (  # type: ignore[method-assign]
        lambda *_args, **_kwargs: pytest.fail("invalid permission should not submit")
    )

    with pytest.raises(ValueError, match=match):
        client.grant_account_permission_and_wait(
            chain_id="chain",
            authority="authority@is",
            private_key_hex="11" * 32,
            **kwargs,
        )
