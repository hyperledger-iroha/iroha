from __future__ import annotations

import base64
import copy
import hashlib
import json
from decimal import Decimal
from typing import Callable
from urllib.parse import quote, urlsplit

import pytest
import requests

import iroha_python.client as client_module

from iroha_python import (
    AccountAsset,
    AccountAssetsPage,
    AssetHolderRecord,
    ContractCallIntent,
    DataEventFilter,
    Ed25519KeyPair,
    ExplorerRwaRecord,
    Instruction,
    KotodamaQuantity,
    LocalSigningContext,
    RwaListItem,
    ToriiClient,
    TransactionConfig,
    TransactionDraft,
    UaidPortfolioAsset,
    authority_fee_payment,
)
from iroha_python._privacy_backends import (
    _VERIFIER_BACKEND_REGISTRY_LABELS_V1,
    _is_verifier_backend_registry_label_v1,
    _require_verifier_backend_registry_label_v1,
    _verifier_backend_registry_tag_v1,
)
from iroha_python.client import ACCOUNT_ONBOARDING_TOKEN_HEADER, DATA_MODEL_VERSION
from iroha_python.repo import (
    RepoAgreementListPage,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
)
from iroha_python.settlement import SettlementLeg
from iroha_python.tx import (
    _normalize_quantity,
    _normalize_rwa_quantity_fields,
    _normalize_u128_quantity,
    _require_canonical_positive_u128_literal,
)

FEE_PAYMENT = authority_fee_payment(charge_limits=[])
VK_LOCAL_SIGNING_CONTEXT = LocalSigningContext("vk-test")


def canonical_proof_attachment(
    *,
    backend: str = "halo2/ipa",
    proof_bytes: bytes = b"proof-bytes",
    vk_backend: str | None = None,
    vk_name: str = "vk_transfer",
) -> dict[str, object]:
    return {
        "backend": backend,
        "proof": {"backend": backend, "bytes": proof_bytes},
        "vk_ref": {"backend": vk_backend or backend, "name": vk_name},
    }


def iroha_hash_bytes(payload: bytes) -> bytes:
    digest = bytearray(hashlib.blake2b(payload, digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


def test_data_model_version_matches_current_wire_contract() -> None:
    assert DATA_MODEL_VERSION == 4


def test_confidential_gas_schedule_has_no_runtime_setter() -> None:
    assert not hasattr(ToriiClient, "set_confidential_gas_schedule")


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


class OnboardingSession:
    def __init__(self, responses: list[requests.Response]):
        self.responses = responses
        self.calls: list[dict[str, object]] = []

    def request(self, method: str, url: str, **kwargs: object) -> requests.Response:
        self.calls.append(
            {
                "method": method,
                "path": urlsplit(url).path,
                "headers": dict(kwargs.get("headers") or {}),
                "data": kwargs.get("data"),
                "allow_redirects": kwargs.get("allow_redirects"),
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


ONBOARDING_TOKEN = "0123456789abcdef0123456789ABCDEF"
ONBOARDING_UAID = "uaid:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"


def test_onboard_account_sends_exact_route_token_and_current_json_contract() -> None:
    session = OnboardingSession([response(202, {"status": "QUEUED"})])
    client = ToriiClient(
        "https://torii.example",
        session=session,
        api_token="global-api-token",
    )

    result = client.onboard_account(
        onboarding_token=ONBOARDING_TOKEN,
        alias="merchant@universal",
        uaid=ONBOARDING_UAID.upper(),
        public_key_hex="AB" * 32,
        identity_commitment_hex="CD" * 32,
        permissions=["CanFoo", "CanFoo", "CanBar"],
    )

    assert result.status_code == 202
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["path"] == "/v1/accounts/onboard"
    assert call["allow_redirects"] is False
    headers = call["headers"]
    assert isinstance(headers, dict)
    onboarding_headers = [
        (name, value)
        for name, value in headers.items()
        if name.lower() == ACCOUNT_ONBOARDING_TOKEN_HEADER.lower()
    ]
    assert onboarding_headers == [(ACCOUNT_ONBOARDING_TOKEN_HEADER, ONBOARDING_TOKEN)]
    assert headers["X-API-Token"] == "global-api-token"
    assert headers["Accept"] == "application/json"
    assert headers["Content-Type"] == "application/json"
    data = call["data"]
    assert isinstance(data, bytes)
    assert ONBOARDING_TOKEN.encode() not in data
    assert json.loads(data) == {
        "alias": "merchant@universal",
        "uaid": ONBOARDING_UAID,
        "public_key_hex": "ab" * 32,
        "identity_commitment_hex": "cd" * 32,
        "permissions": ["CanFoo", "CanBar"],
    }


@pytest.mark.parametrize(
    "token",
    [
        None,
        "",
        "T" * 31,
        "T" * 257,
        "T" * 31 + " ",
        "T" * 31 + "é",
    ],
)
def test_onboard_account_rejects_malformed_route_token_before_dispatch(
    token: object,
) -> None:
    session = OnboardingSession([])
    client = ToriiClient("https://torii.example", session=session)

    with pytest.raises((TypeError, ValueError)) as error:
        client.onboard_account(
            onboarding_token=token,  # type: ignore[arg-type]
            alias="merchant@universal",
            uaid=ONBOARDING_UAID,
            public_key_hex="ab" * 32,
        )

    if token:
        assert str(token) not in str(error.value)
    assert session.calls == []


def test_onboard_account_requires_explicit_token_and_rejects_global_default() -> None:
    session = OnboardingSession([])
    client = ToriiClient("https://torii.example", session=session)

    with pytest.raises(TypeError, match="onboarding_token"):
        client.onboard_account(  # type: ignore[call-arg]
            alias="merchant@universal",
            uaid=ONBOARDING_UAID,
            public_key_hex="ab" * 32,
        )
    with pytest.raises(ValueError, match="pass onboarding_token explicitly"):
        ToriiClient(
            "https://torii.example",
            session=session,
            default_headers={ACCOUNT_ONBOARDING_TOKEN_HEADER.lower(): ONBOARDING_TOKEN},
        )
    assert session.calls == []


def test_onboard_account_does_not_follow_redirect_or_accept_retired_fields() -> None:
    session = OnboardingSession([response(307, text="redirect")])
    client = ToriiClient("https://torii.example", session=session)

    result = client.onboard_account(
        onboarding_token=ONBOARDING_TOKEN,
        alias="merchant@universal",
        uaid=ONBOARDING_UAID,
        public_key_hex="ab" * 32,
    )

    assert result.status_code == 307
    assert len(session.calls) == 1
    assert session.calls[0]["allow_redirects"] is False
    with pytest.raises(TypeError, match="gas_asset_id"):
        client.onboard_account(  # type: ignore[call-arg]
            onboarding_token=ONBOARDING_TOKEN,
            alias="merchant@universal",
            uaid=ONBOARDING_UAID,
            public_key_hex="ab" * 32,
            gas_asset_id="retired",
        )


def test_privacy_verifier_registry_is_closed_exact_and_engine_typed() -> None:
    expected = frozenset(
        {
            "halo2/ipa",
            "halo2/pasta/kaigi-roster-v1",
            "halo2/pasta/kaigi-usage-v1",
            "halo2/pasta/ivm-execution-v1",
            "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
            "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
            "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
            "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1",
        }
    )
    assert len(expected) == 12
    assert _VERIFIER_BACKEND_REGISTRY_LABELS_V1 == expected
    for backend in expected:
        expected_tag = "halo2-ipa-pasta" if backend.startswith("halo2/") else "stark"
        assert _verifier_backend_registry_tag_v1(backend) == expected_tag
        assert _is_verifier_backend_registry_label_v1(backend)
        assert (
            _require_verifier_backend_registry_label_v1(backend, "backend")
            == backend
        )


def test_privacy_verifier_registry_rejects_aliases_retired_and_hostile_labels() -> None:
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
        "HALO2/IPA",
        "stark/FRI",
        "halo2/ipa::ivm-execution-v1",
        "halo2//ipa",
        "halo2/ipa:",
        "halo2/ipa.",
        "halo2/ipa/.ivm-execution-v1",
        "halo2/ipa:ivm..execution-v1",
        "halo2/pasta/ipa-pasta-cycle-v1",
        "halo2/ipa-pasta-cycle-v1",
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
        "stark/fri/miden",
        "stark/fri/miden/claimed-production",
        "stark/fri/latest",
        "stark/fri/random-profile",
        "stark/fri/sha512-goldilocks",
        "stark/fri/audit-proof-v1",
        "stark/fri/sha256 goldilocks",
        "stark/fri/sha256+goldilocks",
        "fcmp++",
        "halo2/ipa+mock",
        "halo2/ipa:production-ready",
        "halo2/ipa:claimed-production",
        "halo2/ipa:mainnet-ready",
        "halo2/ipa:release-ready",
        "halo2/ipa:certified-mainnet",
        "halo2/ipa:third-party-audited",
        "halo2/ipa/orchard:production-ready",
        "orchard:mainnet-ready",
        "penumbra-masp:external-security-review",
        "jindo-lattice-pcs-zk:release-ready",
        "miden-stark:dev-fixture",
        "sis-hints-anoncred-pq-v0",
        "sis-with-hints",
        "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
        "halo2/ipa/orchard:kzg",
        "orchard:universal-srs",
        "penumbra-masp:kzg",
        "jindo-lattice-pcs-zk:trusted-setup",
        "miden-stark:ptau",
        "sis-with-hints:groth16",
        "pq-masp-stark-fri:kzg",
        "stark/fri/audit-signoff",
        "stark/fri/externally-audited",
        "stark/fri/boi-audited",
        "stark/fri/external-security-review",
        "stark/fri/security-review-passed",
        "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
        "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
        "stark/fri/a-u-d-i-t-c-l-a-i-m",
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/todo",
        "stark/fri/t-o-d-o",
        "stark/fri/draft-only",
        "stark/fri/d-r-a-f-t",
        "stark/fri/pending-audit",
        "stark/fri/replace-before-mainnet",
        "stark/fri/not-production-ready",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:todo-proof",
        "halo2/ipa:t-o-d-o-proof",
        "halo2/ipa:draft-proof",
        "halo2/ipa:d-r-a-f-t-proof",
        "halo2/ipa:pending-audit",
        "halo2/ipa:replace-before-production",
        "halo2/ipa:not-for-production",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/kzg",
        "halo2/pasta/mock",
        "kzg/powersoftau",
    )
    for backend in unsupported:
        assert _verifier_backend_registry_tag_v1(backend) is None, backend
        assert not _is_verifier_backend_registry_label_v1(backend), backend
        with pytest.raises(ValueError, match="unsupported verifier-registry label"):
            _require_verifier_backend_registry_label_v1(backend, "backend")
    for backend in (None, b"halo2/ipa", 1, object()):
        assert _verifier_backend_registry_tag_v1(backend) is None
        assert not _is_verifier_backend_registry_label_v1(backend)
        with pytest.raises(TypeError, match="must be a string"):
            _require_verifier_backend_registry_label_v1(backend, "backend")


def test_each_privacy_verifier_registry_label_rejects_structural_mutations() -> None:
    for label in _VERIFIER_BACKEND_REGISTRY_LABELS_V1:
        replacement = "y" if label.endswith("x") else "x"
        mutations = {
            f" {label}",
            f"{label} ",
            label.upper(),
            f"{label}/",
            f"{label}\0",
            f"{label}\u200b",
            label.replace("/", "//", 1),
            f"{label[:-1]}{replacement}",
        }
        mutations.discard(label)
        for mutation in mutations:
            assert not _is_verifier_backend_registry_label_v1(mutation), (
                mutation,
                label,
            )
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


def zk_verifying_key_transaction_draft(
    **overrides: object,
) -> dict[str, object]:
    transaction_payload = b"vk transaction payload"
    signing_message = bytearray(
        hashlib.blake2b(transaction_payload, digest_size=32).digest()
    )
    signing_message[-1] |= 1
    draft: dict[str, object] = {
        "submitted": False,
        "transaction_payload_b64": base64.b64encode(transaction_payload).decode("ascii"),
        "signing_message_b64": base64.b64encode(signing_message).decode("ascii"),
    }
    draft.update(overrides)
    return draft


def zk_verifying_key_draft_for_payload(
    transaction_payload: bytes,
) -> dict[str, object]:
    signing_message = bytearray(
        hashlib.blake2b(transaction_payload, digest_size=32).digest()
    )
    signing_message[-1] |= 1
    return zk_verifying_key_transaction_draft(
        transaction_payload_b64=base64.b64encode(transaction_payload).decode("ascii"),
        signing_message_b64=base64.b64encode(signing_message).decode("ascii"),
    )


class FakeVkDraftCrypto:
    def __init__(
        self,
        decoded: dict[str, object] | None = None,
        error: Exception | None = None,
    ) -> None:
        self.decoded = decoded
        self.error = error
        self.calls: list[tuple[bytes, str, str, str]] = []

    def decode_zk_vk_transaction_payload(
        self,
        payload: bytes,
        expected_chain_id: str,
        expected_authority: str,
        operation: str,
    ) -> dict[str, object]:
        self.calls.append(
            (payload, expected_chain_id, expected_authority, operation)
        )
        if self.error is not None:
            raise self.error
        if self.decoded is None:
            raise AssertionError("fake VK decoder has no result")
        return self.decoded


def expected_zk_verifying_key_instruction(
    payload: dict[str, object],
    authority: str,
    *,
    update: bool,
) -> dict[str, object]:
    normalized = (
        client_module._normalize_zk_verifying_key_update_payload(payload)
        if update
        else client_module._normalize_zk_verifying_key_registration_payload(payload)
    )
    normalized["authority"] = authority
    return client_module._expected_zk_verifying_key_instruction(normalized)


def account_address(seed: int, discriminant: int = 0x02F1) -> str:
    return Ed25519KeyPair.from_private_key(bytes([seed] * 32)).default_account_id(
        "wonderland",
        discriminant,
    )


def test_submit_transaction_draft_result_wait_false_preserves_hash_after_submit_timeout() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)

    class FakeEnvelope:
        hash = bytes.fromhex("ab" * 32)

    envelope = FakeEnvelope()
    client._sign_transaction_draft = (  # type: ignore[method-assign]
        lambda *_args, **_kwargs: envelope
    )

    def fake_submit(_envelope: object) -> object:
        raise requests.Timeout("submit timed out")

    client.submit_transaction_envelope = fake_submit  # type: ignore[method-assign]
    client.wait_for_transaction_status = (  # type: ignore[method-assign]
        lambda *_args, **_kwargs: pytest.fail("wait=False must not poll status")
    )

    result = client._submit_transaction_draft_result(
        object(),  # type: ignore[arg-type]
        private_key_hex="11" * 32,
        wait=False,
    )

    assert result["hash"] == "ab" * 32
    assert result["envelope"] is envelope
    assert result["submission"] == {
        "ok": False,
        "status": "submission_timeout_pending_status",
        "error": "submit timed out",
    }
    assert "terminal" not in result


def test_submit_transaction_draft_result_wait_true_raises_submit_timeout() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)

    class FakeEnvelope:
        hash = bytes.fromhex("cd" * 32)

    client._sign_transaction_draft = (  # type: ignore[method-assign]
        lambda *_args, **_kwargs: FakeEnvelope()
    )

    def fake_submit(_envelope: object) -> object:
        raise requests.Timeout("submit timed out")

    client.submit_transaction_envelope = fake_submit  # type: ignore[method-assign]

    with pytest.raises(requests.Timeout, match="submit timed out"):
        client._submit_transaction_draft_result(
            object(),  # type: ignore[arg-type]
            private_key_hex="11" * 32,
            wait=True,
        )


def test_submit_transaction_draft_result_wait_false_does_not_hide_rejection() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)

    class FakeEnvelope:
        hash = bytes.fromhex("ef" * 32)

    client._sign_transaction_draft = (  # type: ignore[method-assign]
        lambda *_args, **_kwargs: FakeEnvelope()
    )

    def fake_submit(_envelope: object) -> object:
        raise RuntimeError("signature rejected")

    client.submit_transaction_envelope = fake_submit  # type: ignore[method-assign]

    with pytest.raises(RuntimeError, match="signature rejected"):
        client._submit_transaction_draft_result(
            object(),  # type: ignore[arg-type]
            private_key_hex="11" * 32,
            wait=False,
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


def test_asset_quantity_builder_boundary_is_exact_and_canonical() -> None:
    assert _normalize_quantity(KotodamaQuantity("12.5")) == "12.5"
    assert _normalize_quantity("12.5") == "12.5"
    assert _normalize_quantity(12) == "12"
    assert _normalize_quantity(Decimal("12.500")) == "12.5"
    assert _normalize_quantity((1 << 511) - 1) == str((1 << 511) - 1)

    for alternate in ("+1", "01", "1.0", "1.2300", "1e0", "-0", "-1", " 1"):
        with pytest.raises(ValueError):
            _normalize_quantity(alternate)
    for lossy in (1.0, True, None, object()):
        with pytest.raises(TypeError):
            _normalize_quantity(lossy)  # type: ignore[arg-type]
    for out_of_domain in (Decimal("1e1000000"), Decimal("1e-1000000")):
        with pytest.raises(ValueError):
            _normalize_quantity(out_of_domain)
    with pytest.raises(ValueError, match="canonical V1 bound"):
        _normalize_quantity("1." + "0" * 10_000)
    with pytest.raises(ValueError):
        _normalize_quantity(1 << 511)


def test_u128_quantity_boundary_rejects_out_of_range_values() -> None:
    assert _normalize_u128_quantity((1 << 128) - 1, "amount") == str((1 << 128) - 1)
    with pytest.raises(ValueError, match="within u128"):
        _normalize_u128_quantity(1 << 128, "amount")


@pytest.mark.parametrize(
    "quantity",
    [1.5, True, "+1", "01", "1.0", "1.500", "1e0", "-1", " 1"],
)
def test_repo_and_settlement_quantity_builders_reject_alternate_inputs(
    quantity: object,
) -> None:
    builders = (
        RepoCashLeg("cash#is", quantity),  # type: ignore[arg-type]
        RepoCollateralLeg("bond#is", quantity),  # type: ignore[arg-type]
        SettlementLeg(
            "cash#is", quantity, "alice@is", "bob@is"  # type: ignore[arg-type]
        ),
    )
    for builder in builders:
        with pytest.raises((TypeError, ValueError)):
            builder.to_payload()


def test_rwa_nested_quantity_fields_are_normalized_without_float_coercion() -> None:
    normalized = _normalize_rwa_quantity_fields(
        {
            "quantity": Decimal("10.500"),
            "parents": [{"rwa": "parent", "quantity": Decimal("1.2500")}],
        },
        "rwa",
        top_level_quantity=True,
    )
    assert normalized["quantity"] == "10.5"
    assert normalized["parents"][0]["quantity"] == "1.25"

    for quantity in (1.5, "1.0", "01", "-1"):
        with pytest.raises((TypeError, ValueError)):
            _normalize_rwa_quantity_fields(
                {"quantity": quantity},
                "rwa",
                top_level_quantity=True,
            )
        with pytest.raises((TypeError, ValueError)):
            _normalize_rwa_quantity_fields(
                {"parents": [{"rwa": "parent", "quantity": quantity}]},
                "merge",
                top_level_quantity=False,
            )


def _quantity_readback_factories(
    quantity: object,
) -> tuple[Callable[[], object], ...]:
    return (
        lambda: AccountAsset.from_payload({"asset_id": "asset#account", "quantity": quantity}),
        lambda: AssetHolderRecord.from_payload(
            {"account_id": "account", "quantity": quantity}
        ),
        lambda: UaidPortfolioAsset.from_payload(
            {
                "asset_id": "asset#account",
                "asset_definition_id": "asset",
                "quantity": quantity,
            }
        ),
        lambda: ExplorerRwaRecord.from_payload(
            {
                "id": "rwa",
                "owned_by": "account",
                "quantity": quantity,
                "held_quantity": "0",
                "primary_reference": "reference",
                "is_frozen": False,
            }
        ),
        lambda: RwaListItem.from_payload({"id": "rwa", "quantity": quantity}),
    )


@pytest.mark.parametrize("quantity", ["1.0", "01", "+1", "-1", 1, 1.0, None])
def test_typed_quantity_readbacks_reject_noncanonical_or_untyped_values(
    quantity: object,
) -> None:
    for factory in _quantity_readback_factories(quantity):
        with pytest.raises((TypeError, ValueError)):
            factory()


def test_rwa_readback_rejects_noncanonical_nested_parent_quantity() -> None:
    with pytest.raises(ValueError):
        RwaListItem.from_payload(
            {
                "id": "rwa",
                "quantity": "2",
                "parents": [{"rwa": "parent", "quantity": "1.0"}],
            }
        )


def test_typed_quantity_readback_rejects_oversized_alternate_before_bigint_parsing() -> None:
    with pytest.raises(ValueError, match="canonical V1 text bound"):
        AccountAsset.from_payload(
            {
                "asset_id": "asset#account",
                "quantity": "1." + "0" * 10_000,
            }
        )


@pytest.mark.parametrize("quantity", ["1.0", "01", "+1", "-1", 1, None])
def test_asset_balance_rejects_noncanonical_or_untyped_quantities(quantity: object) -> None:
    session = FakeSession(
        [
            response(
                200,
                {
                    "items": [
                        {
                            "asset_id": "canonical-ds-id#adult@is",
                            "asset_alias": "ds#wonderland.is",
                            "quantity": quantity,
                        }
                    ],
                    "total": 1,
                },
            )
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    with pytest.raises((TypeError, ValueError)):
        client.asset_balance("adult@is", "ds#wonderland.is")


def test_get_asset_definition_returns_none_for_missing_definition() -> None:
    session = FakeSession([response(404, text="missing")])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.get_asset_definition("ds#wonderland.is") is None


def test_data_model_validation_uses_typed_node_capabilities() -> None:
    session = FakeSession(
        [response(200, {"abi_version": 1, "data_model_version": DATA_MODEL_VERSION})]
    )
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

    page = client.query_accounts_typed(
        limit=1,
        count_mode="bounded",
        select=[" id ", {"metadata": {"tier": True}}],
    )

    assert page.total is None
    assert page.has_more is True
    assert page.count_mode == "bounded"
    assert page.indexed_height == 7
    assert page.indexed_block_hash == "ab" * 32
    assert page.query_source == "live"
    body = json.loads(session.calls[0]["data"])
    assert body["count_mode"] == "bounded"
    assert body["pagination"] == {"offset": 0, "limit": 1}
    assert body["select"] == ["id", {"metadata": {"tier": True}}]


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


def test_query_account_transactions_posts_count_mode_and_select_projection() -> None:
    account = account_address(0x31)
    session = FakeSession([response(200, {"items": [], "total": 0})])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    payload = client.query_account_transactions(
        account,
        count_mode="bounded",
        select=[" authority ", {"metadata": {"amount": True}}],
        query_name=" VisibleTransactions ",
        limit=25,
        offset=5,
    )

    assert payload == {"items": [], "total": 0}
    assert session.calls[0]["path"] == f"/v1/accounts/{quote(account, safe='')}/transactions/query"
    body = json.loads(session.calls[0]["data"])
    assert body["pagination"] == {"limit": 25, "offset": 5}
    assert "limit" not in body
    assert "offset" not in body
    assert body["count_mode"] == "bounded"
    assert body["query"] == "VisibleTransactions"
    assert "query_name" not in body
    assert body["select"] == ["authority", {"metadata": {"amount": True}}]


def test_query_triggers_posts_query_wire_name_and_count_mode() -> None:
    session = FakeSession([response(200, {"items": [], "total": 0, "count_mode": "bounded"})])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    payload = client.query_triggers(
        fetch_size=3,
        count_mode="bounded",
        select=[" id ", {"authority": True}],
        query_name=" recent-triggers ",
        limit=10,
        offset=2,
    )

    assert payload == {"items": [], "total": 0, "count_mode": "bounded"}
    assert session.calls[0]["path"] == "/v1/triggers/query"
    body = json.loads(session.calls[0]["data"])
    assert body["pagination"] == {"limit": 10, "offset": 2}
    assert "limit" not in body
    assert "offset" not in body
    assert body["fetch_size"] == 3
    assert body["count_mode"] == "bounded"
    assert body["query"] == "recent-triggers"
    assert "query_name" not in body
    assert body["select"] == ["id", {"authority": True}]


def test_query_account_transactions_rejects_bad_select_before_request() -> None:
    account = account_address(0x32)
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)

    with pytest.raises(TypeError, match="select must be a sequence"):
        client.query_account_transactions(account, select="authority")
    with pytest.raises(ValueError, match=r"select\[1].*non-empty"):
        client.query_account_transactions(account, select=["authority", " "])
    with pytest.raises(TypeError, match=r"select\[1].*field-path string or mapping"):
        client.query_account_transactions(account, select=["authority", 7])
    with pytest.raises(ValueError, match="filter/select/sort"):
        client.query_account_transactions(
            account,
            envelope={"select": ["authority"]},
            select=["authority"],
        )


def test_repo_agreement_page_preserves_bounded_metadata_and_rejects_bad_flags() -> None:
    payload = {
        "items": [
            {
                "id": "repo-1",
                "initiator": "alice@is",
                "counterparty": "bob@is",
                "custodian": None,
                "cash_leg": {"asset_definition_id": "cash#is", "quantity": "100"},
                "cash_source": "cash#is::bob@is",
                "collateral_leg": {"asset_definition_id": "bond#is", "quantity": "120"},
                "collateral_custody_asset": "bond#is::bob@is",
                "rate_bps": 250,
                "maturity_timestamp_ms": 2_000,
                "initiated_timestamp_ms": 1_000,
                "last_margin_check_timestamp_ms": 1_000,
                "governance": {"haircut_bps": 500, "margin_frequency_secs": 3600},
                "settlement_timestamp_ms": None,
                "status": "active",
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
    assert page.items[0].cash_source == "cash#is::bob@is"
    assert page.items[0].collateral_custody_asset == "bond#is::bob@is"
    assert page.items[0].settlement_timestamp_ms is None
    assert page.items[0].status == "active"

    bad = dict(payload)
    bad["has_more"] = "true"
    with pytest.raises(TypeError, match="has_more"):
        RepoAgreementListPage.from_payload(bad)

    inconsistent = dict(payload["items"][0])
    inconsistent["status"] = "settled"
    with pytest.raises(ValueError, match="status must agree"):
        RepoAgreementRecord.from_payload(inconsistent)


@pytest.mark.parametrize("quantity", ["1.0", "01", "+1", "-1", 1, 1.0, None])
def test_repo_agreement_readback_rejects_noncanonical_quantities(quantity: object) -> None:
    payload = {
        "items": [
            {
                "id": "repo-1",
                "initiator": "alice@is",
                "counterparty": "bob@is",
                "custodian": None,
                "cash_leg": {"asset_definition_id": "cash#is", "quantity": quantity},
                "cash_source": "cash#is::bob@is",
                "collateral_leg": {
                    "asset_definition_id": "bond#is",
                    "quantity": "120",
                },
                "collateral_custody_asset": "bond#is::bob@is",
                "rate_bps": 250,
                "maturity_timestamp_ms": 2_000,
                "initiated_timestamp_ms": 1_000,
                "last_margin_check_timestamp_ms": 1_000,
                "governance": {"haircut_bps": 500, "margin_frequency_secs": 3600},
                "settlement_timestamp_ms": None,
                "status": "active",
            }
        ]
    }

    with pytest.raises((TypeError, ValueError)):
        RepoAgreementListPage.from_payload(payload)


def test_repo_agreement_readback_requires_quantity_fields() -> None:
    payload = {
        "id": "repo-1",
        "initiator": "alice@is",
        "counterparty": "bob@is",
        "custodian": None,
        "cash_leg": {"asset_definition_id": "cash#is"},
        "cash_source": "cash#is::bob@is",
        "collateral_leg": {"asset_definition_id": "bond#is", "quantity": "120"},
        "collateral_custody_asset": "bond#is::bob@is",
        "rate_bps": 250,
        "maturity_timestamp_ms": 2_000,
        "initiated_timestamp_ms": 1_000,
        "last_margin_check_timestamp_ms": 1_000,
        "governance": {"haircut_bps": 500, "margin_frequency_secs": 3600},
        "settlement_timestamp_ms": None,
        "status": "active",
    }

    with pytest.raises(KeyError, match="quantity"):
        RepoAgreementListPage.from_payload({"items": [payload]})


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


def test_sns_helpers_read_policy_and_name() -> None:
    session = FakeSession(
        [
            response(200, {"payment_asset_id": "fee", "pricing": []}),
            response(200, {"literal": "merchant@paynet"}),
        ]
    )
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    assert client.get_sns_policy(2)["payment_asset_id"] == "fee"
    registration = client.get_sns_name("account-alias", "merchant@paynet")

    assert registration == {"literal": "merchant@paynet"}
    assert [call["path"] for call in session.calls] == [
        "/v1/sns/policies/2",
        "/v1/sns/names/account-alias/merchant%40paynet",
    ]


def test_zk_verifying_key_helpers_detect_active_status_and_return_registration_draft(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    expected_draft = zk_verifying_key_transaction_draft()
    authority = account_address(31)
    registration_payload: dict[str, object] = {
        "authority": authority,
        "backend": "halo2/ipa",
        "name": "vk_transfer",
        "version": 1,
        "circuit_id": "vk-transfer-v1",
        "public_inputs_schema_hash_hex": "11" * 32,
        "gas_schedule_id": "zk.verify.default",
        "vk_bytes": "dms=",
    }
    native = FakeVkDraftCrypto(
        expected_zk_verifying_key_instruction(
            registration_payload,
            authority,
            update=False,
        )
    )
    monkeypatch.setattr(client_module, "_require_crypto", lambda: native)
    session = FakeSession(
        [
            response(200, {"record": {"status": "Active"}}),
            response(200, expected_draft),
        ]
    )
    client = ToriiClient(
        "http://torii.example",
        session=session,
        local_signing_context=VK_LOCAL_SIGNING_CONTEXT,
        max_retries=0,
    )
    assert client.local_signing_context is VK_LOCAL_SIGNING_CONTEXT

    assert client.zk_verifying_key_active("halo2/ipa", "vk_transfer")
    draft = client.register_zk_verifying_key(registration_payload)

    assert draft == expected_draft
    assert [call["path"] for call in session.calls] == [
        "/v1/zk/vk/halo2%2Fipa/vk_transfer",
        "/v1/zk/vk/register",
    ]
    body = json.loads(session.calls[1]["data"])
    assert "private_key" not in body
    assert native.calls == [
        (
            b"vk transaction payload",
            "vk-test",
            authority,
            "register",
        )
    ]


def test_zk_verifying_key_helpers_reject_labels_outside_exact_registry() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    for backend in ("stark/fri/latest", "stark/fri/attestation", "stark/fri/contest"):
        with pytest.raises(
            ValueError,
            match=(
                "must be a non-empty string|surrounding whitespace|"
                "unsupported production verifier backend"
            ),
        ):
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
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/dev",
        "stark/fri/d-e-v",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/todo",
        "stark/fri/t-o-d-o",
        "stark/fri/draft-only",
        "stark/fri/d-r-a-f-t",
        "stark/fri/pending-audit",
        "stark/fri/replace-before-mainnet",
        "stark/fri/not-production-ready",
        "stark/fri/placeholder",
        " stark/fri/sha256-goldilocks",
        "stark/fri/sha256-goldilocks ",
        "halo2/ipa/orchard",
        "halo2/kzg",
        "halo2/ipa\0",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dev",
        "halo2/ipa:d-e-v",
        "halo2/ipa:todo-proof",
        "halo2/ipa:t-o-d-o-proof",
        "halo2/ipa:draft-proof",
        "halo2/ipa:d-r-a-f-t-proof",
        "halo2/ipa:pending-audit",
        "halo2/ipa:replace-before-production",
        "halo2/ipa:not-for-production",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/ipa:s-a-m-p-l-e",
        "mock/dev",
    ):
        with pytest.raises(
            ValueError,
            match=(
                "must be a non-empty string|surrounding whitespace|"
                "unsupported production verifier backend"
            ),
        ):
            client.submit_zk_verifying_key_registration(
                {"backend": backend, "name": "vk_transfer"}
            )
    assert session.calls == []


def test_zk_verifying_key_registration_rejects_bad_names_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    for bad_name in ("", "   ", "\t", " vk_transfer", "vk_transfer ", None, 7):
        with pytest.raises((TypeError, ValueError), match="register_zk_verifying_key.name"):
            client.submit_zk_verifying_key_registration(
                {"backend": "halo2/ipa", "name": bad_name}
            )

    assert session.calls == []


def test_zk_verifying_key_registration_rejects_padded_selector_metadata_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    base = {
        "authority": "alice",
        "backend": "halo2/ipa",
        "name": "vk_transfer",
        "version": 1,
        "circuit_id": "halo2/ipa::transfer_v1",
        "public_inputs_schema_hash_hex": "aa" * 32,
        "gas_schedule_id": "halo2_default",
        "vk_bytes": "dms=",
    }

    for field, value in (
        ("name", " vk_transfer"),
        ("name", "vk_transfer "),
        ("circuit_id", " halo2/ipa::transfer_v1"),
        ("circuit_id", "halo2/ipa::transfer_v1 "),
        ("gas_schedule_id", " halo2_default"),
        ("gas_schedule_id", "halo2_default "),
    ):
        with pytest.raises(
            ValueError,
            match=rf"register_zk_verifying_key\.{field}.*surrounding whitespace",
        ):
            client.submit_zk_verifying_key_registration({**base, field: value})

    assert session.calls == []


def test_zk_verifying_key_registration_rejects_missing_authority_and_private_keys() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    for authority in (None, "", "   ", 7):
        with pytest.raises((TypeError, ValueError), match="register_zk_verifying_key.authority"):
            client.submit_zk_verifying_key_registration(
                {
                    "authority": authority,
                    "backend": "halo2/ipa",
                    "name": "vk_transfer",
                }
            )

    for field, private_key in (
        ("private_key", None),
        ("private_key", "ed25519:deadbeef"),
        ("privateKey", "ed25519:deadbeef"),
        ("private_key_hex", "11" * 32),
        ("privateKeyBytes", bytes([0x11]) * 32),
    ):
        with pytest.raises(
            ValueError,
            match="does not accept private-key fields.*sign the returned transaction draft locally",
        ):
            client.submit_zk_verifying_key_registration(
                {
                    "authority": "alice",
                    "backend": "halo2/ipa",
                    "name": "vk_transfer",
                    field: private_key,
                }
            )

    assert session.calls == []


def test_zk_verifying_key_registration_rejects_mismatched_inline_commitment() -> None:
    session = FakeSession([response(200, zk_verifying_key_transaction_draft())])
    client = ToriiClient(
        "http://torii.example",
        session=session,
        local_signing_context=VK_LOCAL_SIGNING_CONTEXT,
        max_retries=0,
    )
    vk_bytes = b"abc"
    matching_commitment = zk_verifying_key_commitment("halo2/ipa", vk_bytes)

    with pytest.raises(ValueError, match="commitment_hex must match domain-separated SHA-256"):
        client.submit_zk_verifying_key_registration(
            {
                "authority": "alice",
                "backend": "halo2/ipa",
                "name": "vk_transfer",
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
            "version": 1,
            "circuit_id": "halo2/ipa::transfer_v1",
            "public_inputs_schema_hash_hex": "aa" * 32,
            "gas_schedule_id": "halo2_default",
            "vk_bytes": vk_bytes,
            "commitment_hex": matching_commitment,
        }
    )

    assert response_obj.status_code == 200
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
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "public_inputs_schema_hash_hex": "aa" * 32,
                "gas_schedule_id": "halo2_default",
                "activation_height": -1,
            }
        )
    assert session.calls == []


def test_zk_verifying_key_update_helper_returns_unsigned_draft(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    expected_draft = zk_verifying_key_transaction_draft()
    session = FakeSession([response(200, expected_draft)])
    vk_bytes = b"abc"
    matching_commitment = zk_verifying_key_commitment("halo2/ipa", vk_bytes)
    authority = account_address(32)
    update_payload: dict[str, object] = {
        "authority": authority,
        "backend": "halo2/ipa",
        "name": "vk_transfer",
        "version": 2,
        "circuit_id": "halo2/ipa::transfer_v2",
        "public_inputs_schema_hash_hex": "aa" * 32,
        "gas_schedule_id": "halo2_default",
        "vk_bytes": vk_bytes,
        "commitment_hex": matching_commitment,
        "activation_height": "10",
        "withdraw_height": "10",
    }
    native = FakeVkDraftCrypto(
        expected_zk_verifying_key_instruction(
            update_payload,
            authority,
            update=True,
        )
    )
    monkeypatch.setattr(client_module, "_require_crypto", lambda: native)
    client = ToriiClient(
        "http://torii.example",
        session=session,
        local_signing_context=VK_LOCAL_SIGNING_CONTEXT,
        max_retries=0,
    )

    decoded = client.update_zk_verifying_key(update_payload)

    assert decoded == expected_draft
    assert [call["path"] for call in session.calls] == ["/v1/zk/vk/update"]
    call = session.calls[0]
    assert call["method"] == "POST"
    body = json.loads(call["data"])
    assert body["authority"] == authority
    assert "private_key" not in body
    assert body["backend"] == "halo2/ipa"
    assert body["name"] == "vk_transfer"
    assert body["vk_bytes"] == base64.b64encode(vk_bytes).decode("ascii")
    assert body["commitment_hex"] == matching_commitment
    assert body["activation_height"] == 10
    assert body["withdraw_height"] == 10


def test_zk_verifying_key_mutation_helpers_enforce_unsigned_draft_contract() -> None:
    payload = {
        "authority": account_address(33),
        "backend": "halo2/ipa",
        "name": "vk_transfer",
        "version": 1,
        "circuit_id": "halo2/ipa::transfer_v1",
        "public_inputs_schema_hash_hex": "aa" * 32,
        "gas_schedule_id": "halo2_default",
        "vk_bytes": "dms=",
    }

    for malformed, pattern in (
        (
            zk_verifying_key_transaction_draft(submitted=True),
            r"submitted must be false",
        ),
        (
            zk_verifying_key_transaction_draft(transaction_payload_b64="AQ"),
            r"transaction_payload_b64 must be canonical padded base64",
        ),
        (
            zk_verifying_key_transaction_draft(signing_message_b64=None),
            r"signing_message_b64 must be canonical padded base64",
        ),
        (
            zk_verifying_key_transaction_draft(
                transaction_payload_b64=base64.b64encode(
                    bytes(16 * 1024 * 1024 + 1)
                ).decode("ascii")
            ),
            r"transaction_payload_b64 exceeds the 16777216-byte transaction payload limit",
        ),
        (
            zk_verifying_key_transaction_draft(
                signing_message_b64=base64.b64encode(bytes(31)).decode("ascii")
            ),
            r"signing_message_b64 must decode to exactly 32 bytes",
        ),
        (
            zk_verifying_key_transaction_draft(
                signing_message_b64=base64.b64encode(bytes(32)).decode("ascii")
            ),
            r"signing_message_b64 must equal the canonical Iroha HashOf",
        ),
        (
            zk_verifying_key_transaction_draft(accepted=True),
            r"contains unsupported fields: accepted",
        ),
    ):
        client = ToriiClient(
            "http://torii.example",
            session=FakeSession([response(200, malformed)]),
            local_signing_context=VK_LOCAL_SIGNING_CONTEXT,
            max_retries=0,
        )
        with pytest.raises((TypeError, ValueError), match=pattern):
            client.register_zk_verifying_key(payload)

    legacy_status_client = ToriiClient(
        "http://torii.example",
        session=FakeSession([response(202, zk_verifying_key_transaction_draft())]),
        local_signing_context=VK_LOCAL_SIGNING_CONTEXT,
        max_retries=0,
    )
    with pytest.raises(RuntimeError, match=r"unexpected status 202; expected \[200\]"):
        legacy_status_client.update_zk_verifying_key(payload)


def test_zk_verifying_key_drafts_reject_substitution_extra_wrong_context_and_noncanonical(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    authority = account_address(34)
    payload: dict[str, object] = {
        "authority": authority,
        "backend": "halo2/ipa",
        "name": "vk_transfer",
        "version": 1,
        "circuit_id": "halo2/ipa::transfer_v1",
        "public_inputs_schema_hash_hex": "aa" * 32,
        "gas_schedule_id": "halo2_default",
        "vk_bytes": "dms=",
    }
    for label, error, pattern in (
        (
            "operation substitution",
            ValueError(
                "verifying-key transaction payload must contain RegisterVerifyingKey"
            ),
            r"must contain RegisterVerifyingKey",
        ),
        (
            "extra instruction",
            ValueError(
                "verifying-key transaction payload must contain exactly one instruction"
            ),
            r"exactly one instruction",
        ),
        (
            "wrong chain",
            ValueError("transaction payload changed the configured chain ID"),
            r"changed the configured chain ID",
        ),
        (
            "wrong authority",
            ValueError("transaction payload changed the requested authority"),
            r"changed the requested authority",
        ),
        (
            "noncanonical",
            ValueError("invalid canonical transaction payload"),
            r"invalid canonical transaction payload",
        ),
    ):
        native = FakeVkDraftCrypto(error=error)
        monkeypatch.setattr(client_module, "_require_crypto", lambda native=native: native)
        transaction_payload = f"vk-{label}".encode()
        client = ToriiClient(
            "http://torii.example",
            session=FakeSession(
                [response(200, zk_verifying_key_draft_for_payload(transaction_payload))]
            ),
            local_signing_context=VK_LOCAL_SIGNING_CONTEXT,
            max_retries=0,
        )
        with pytest.raises(ValueError, match=pattern):
            client.register_zk_verifying_key(payload)
        assert native.calls[0][1:] == (
            "vk-test",
            authority,
            "register",
        ), label


def test_zk_verifying_key_draft_rejects_any_record_field_mismatch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    authority = account_address(35)
    payload: dict[str, object] = {
        "authority": authority,
        "backend": "halo2/ipa",
        "name": "vk_transfer",
        "version": 1,
        "circuit_id": "halo2/ipa::transfer_v1",
        "public_inputs_schema_hash_hex": "aa" * 32,
        "gas_schedule_id": "halo2_default",
        "vk_bytes": "dms=",
    }
    decoded = copy.deepcopy(
        expected_zk_verifying_key_instruction(payload, authority, update=False)
    )
    record = decoded["record"]
    assert isinstance(record, dict)
    record["max_proof_bytes"] = 1
    native = FakeVkDraftCrypto(decoded)
    monkeypatch.setattr(client_module, "_require_crypto", lambda: native)
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([response(200, zk_verifying_key_transaction_draft())]),
        local_signing_context=VK_LOCAL_SIGNING_CONTEXT,
        max_retries=0,
    )
    with pytest.raises(
        ValueError,
        match="does not contain the exact requested verifying-key registry record",
    ):
        client.register_zk_verifying_key(payload)


def test_zk_verifying_key_local_signing_fails_closed_without_chain_context() -> None:
    session = FakeSession([response(200, zk_verifying_key_transaction_draft())])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    payload = {
        "authority": account_address(36),
        "backend": "halo2/ipa",
        "name": "vk_transfer",
        "version": 1,
        "circuit_id": "halo2/ipa::transfer_v1",
        "public_inputs_schema_hash_hex": "aa" * 32,
        "gas_schedule_id": "halo2_default",
        "vk_bytes": "dms=",
    }
    with pytest.raises(
        ValueError,
        match="requires immutable ToriiClient local_signing_context",
    ):
        client.register_zk_verifying_key(payload)
    with pytest.raises(
        ValueError,
        match="requires immutable ToriiClient local_signing_context",
    ):
        client.update_zk_verifying_key(payload)
    with pytest.raises(
        ValueError,
        match="requires immutable ToriiClient local_signing_context",
    ):
        client.submit_zk_verifying_key_registration(payload)
    with pytest.raises(
        ValueError,
        match="requires immutable ToriiClient local_signing_context",
    ):
        client.submit_zk_verifying_key_update(payload)
    assert session.calls == []
    with pytest.raises(AttributeError):
        client.local_signing_context = LocalSigningContext(  # type: ignore[misc]
            "other-chain"
        )
    with pytest.raises(AttributeError):
        VK_LOCAL_SIGNING_CONTEXT.chain_id = "other-chain"  # type: ignore[misc]
    with pytest.raises(ValueError, match="must not contain surrounding whitespace"):
        LocalSigningContext(" vk-test")
    with pytest.raises(TypeError, match="unexpected keyword argument 'signing_chain_id'"):
        ToriiClient(
            "http://torii.example",
            **{"signing_chain_id": "vk-test"},  # type: ignore[arg-type]
        )

    signing_client = ToriiClient(
        "http://torii.example",
        session=session,
        local_signing_context=VK_LOCAL_SIGNING_CONTEXT,
        max_retries=0,
    )
    with pytest.raises(ValueError, match="must be an exact canonical I105 account id"):
        signing_client.register_zk_verifying_key(
            {**payload, "authority": "merchant@paynet"}
        )
    assert session.calls == []


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

    for bad_name in ("", "   ", "\t", " vk_transfer", "vk_transfer ", None, 7):
        with pytest.raises((TypeError, ValueError), match="update_zk_verifying_key.name"):
            client.submit_zk_verifying_key_update(payload(name=bad_name))

    for field, value in (
        ("circuit_id", " halo2/ipa::transfer_v2"),
        ("circuit_id", "halo2/ipa::transfer_v2 "),
        ("gas_schedule_id", " halo2_default"),
        ("gas_schedule_id", "halo2_default "),
    ):
        with pytest.raises(
            ValueError,
            match=rf"update_zk_verifying_key\.{field}.*surrounding whitespace",
        ):
            client.submit_zk_verifying_key_update(payload(**{field: value}))

    for authority in (None, "", "   ", 7):
        with pytest.raises((TypeError, ValueError), match="update_zk_verifying_key.authority"):
            client.submit_zk_verifying_key_update(payload(authority=authority))

    for field, private_key in (
        ("private_key", None),
        ("private_key", "ed25519:deadbeef"),
        ("privateKey", "ed25519:deadbeef"),
        ("private_key_hex", "11" * 32),
        ("privateKeyBytes", bytes([0x11]) * 32),
    ):
        with pytest.raises(
            ValueError,
            match="does not accept private-key fields.*sign the returned transaction draft locally",
        ):
            client.submit_zk_verifying_key_update(payload(**{field: private_key}))

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
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/dev",
        "stark/fri/d-e-v",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/todo",
        "stark/fri/t-o-d-o",
        "stark/fri/draft-only",
        "stark/fri/d-r-a-f-t",
        "stark/fri/pending-audit",
        "stark/fri/replace-before-mainnet",
        "stark/fri/not-production-ready",
        "stark/fri/placeholder",
        " stark/fri/sha256-goldilocks",
        "stark/fri/sha256-goldilocks ",
        "halo2/ipa/orchard",
        "halo2/kzg",
        "halo2/ipa\0",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dev",
        "halo2/ipa:d-e-v",
        "halo2/ipa:todo-proof",
        "halo2/ipa:t-o-d-o-proof",
        "halo2/ipa:draft-proof",
        "halo2/ipa:d-r-a-f-t-proof",
        "halo2/ipa:pending-audit",
        "halo2/ipa:replace-before-production",
        "halo2/ipa:not-for-production",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/ipa:s-a-m-p-l-e",
        "mock/dev",
    ):
        with pytest.raises(
            ValueError,
            match=(
                "must be a non-empty string|surrounding whitespace|"
                "unsupported production verifier backend"
            ),
        ):
            client.request_zk_verifying_key(backend, "vk_transfer")
        with pytest.raises(
            ValueError,
            match=(
                "must be a non-empty string|surrounding whitespace|"
                "unsupported production verifier backend"
            ),
        ):
            client.zk_verifying_key_active(backend, "vk_transfer")
    assert session.calls == []


def test_zk_verifying_key_read_helpers_reject_padded_names_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)

    for name in (" vk_transfer", "vk_transfer "):
        with pytest.raises(ValueError, match="name.*surrounding whitespace"):
            client.request_zk_verifying_key("halo2/ipa", name)
        with pytest.raises(ValueError, match="name.*surrounding whitespace"):
            client.zk_verifying_key_active("halo2/ipa", name)

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
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/dev",
        "stark/fri/d-e-v",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/todo",
        "stark/fri/t-o-d-o",
        "stark/fri/draft-only",
        "stark/fri/d-r-a-f-t",
        "stark/fri/pending-audit",
        "stark/fri/replace-before-mainnet",
        "stark/fri/not-production-ready",
        "stark/fri/placeholder",
        " stark/fri/sha256-goldilocks",
        "stark/fri/sha256-goldilocks ",
        "halo2/ipa/orchard",
        "halo2/kzg",
        "halo2/ipa\0",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dev",
        "halo2/ipa:d-e-v",
        "halo2/ipa:todo-proof",
        "halo2/ipa:t-o-d-o-proof",
        "halo2/ipa:draft-proof",
        "halo2/ipa:d-r-a-f-t-proof",
        "halo2/ipa:pending-audit",
        "halo2/ipa:replace-before-production",
        "halo2/ipa:not-for-production",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:stub",
        "halo2/ipa:s-a-m-p-l-e",
        "mock/dev",
    ):
        with pytest.raises(
            ValueError,
            match="unsupported verifier-registry label",
        ):
            DataEventFilter.verifying_key(backend=backend, name="vk_transfer")
        with pytest.raises(
            ValueError,
            match="unsupported verifier-registry label",
        ):
            DataEventFilter.proof(backend=backend, proof_hash_hex="a" * 64)
        with pytest.raises(
            ValueError,
            match="unsupported verifier-registry label",
        ):
            client.stream_verifying_key_events(backend=backend, name="vk_transfer")
        with pytest.raises(
            ValueError,
            match="unsupported verifier-registry label",
        ):
            client.stream_proof_events(backend=backend, proof_hash_hex="a" * 64)
    assert session.calls == []


def test_zk_verifying_key_event_filters_reject_malformed_names_before_request() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    for name in ("", "   ", "\t", "\n", " vk_transfer", "vk_transfer ", "vk:transfer", 42):
        with pytest.raises((TypeError, ValueError), match="verifying_key_filter.name"):
            DataEventFilter.verifying_key(backend="halo2/ipa", name=name)
        with pytest.raises((TypeError, ValueError), match="verifying_key_filter.name"):
            client.stream_verifying_key_events(backend="halo2/ipa", name=name)

    payload = DataEventFilter.verifying_key(backend="halo2/ipa", name="vk_transfer").to_dict()
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
                "id_matcher": {"backend": "halo2/ipa", "name": " vk_transfer"},
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
                "id_matcher": {"backend": "halo2/ipa", "name": "vk_transfer"},
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



def test_call_contract_and_wait_delegates_to_caller_signed_batch() -> None:
    tx_hash = "d" * 64
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    captured: dict[str, object] = {}

    def call_batch(**kwargs: object) -> dict[str, object]:
        captured.update(kwargs)
        return {
            "hash": tx_hash,
            "submission": {"accepted": True},
        }

    client.call_contract_batch_and_wait = call_batch  # type: ignore[method-assign]

    result = client.call_contract_and_wait(
        chain_id="chain",
        authority="authority@is",
        private_key_hex="11" * 32,
        contract_alias="contract::is",
        entrypoint="main",
        payload={"amount": 7},
        fee_payment=authority_fee_payment(charge_limits=[], gas_limit=5000),
        wait=True,
        timeout_ms=1000,
        interval=0,
    )

    assert captured["chain_id"] == "chain"
    assert captured["authority"] == "authority@is"
    assert captured["private_key"] is None
    assert captured["private_key_hex"] == "11" * 32
    entries = captured["entries"]
    assert isinstance(entries, list) and len(entries) == 1
    intent = entries[0]
    assert isinstance(intent, ContractCallIntent)
    assert intent.to_payload() == {
        "entrypoint": "main",
        "contract_alias": "contract::is",
        "payload": {"amount": 7},
    }
    assert result["submit"] == {"accepted": True}
    assert result["tx_hashes"] == [tx_hash]
    assert session.calls == []


def test_call_contract_and_wait_requires_explicit_chain_id_before_dispatch() -> None:
    session = FakeSession([])
    client = ToriiClient("http://torii.example", session=session, max_retries=0)
    client.call_contract_batch_and_wait = (  # type: ignore[method-assign]
        lambda **_kwargs: pytest.fail("missing chain_id must fail before dispatch")
    )

    with pytest.raises(TypeError, match="chain_id"):
        client.call_contract_and_wait(  # type: ignore[call-arg]
            authority="authority@is",
            private_key_hex="11" * 32,
            contract_alias="contract::is",
            entrypoint="main",
            payload={"amount": 7},
            fee_payment=authority_fee_payment(charge_limits=[], gas_limit=5000),
        )
    assert session.calls == []


def test_mint_assets_and_wait_batches_records_in_one_transaction() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    captured: dict[str, object] = {}
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    adult = account_address(0x11)
    business = account_address(0x22)

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "mint-batch"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    result = client.mint_assets_and_wait(
        chain_id="chain",
        authority="authority@is",
        fee_payment=FEE_PAYMENT,
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


def test_transaction_draft_rejects_padded_chain_and_authority_before_signing() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)

    with pytest.raises(
        ValueError,
        match="chain_id must not contain surrounding whitespace",
    ):
        client._transaction_draft(
            chain_id=" chain",
            authority="authority@is",
            fee_payment=FEE_PAYMENT,
        )

    with pytest.raises(
        ValueError,
        match="authority must not contain surrounding whitespace",
    ):
        client._transaction_draft(
            chain_id="chain",
            authority=" authority@is ",
            fee_payment=FEE_PAYMENT,
        )


def test_transfer_assets_and_wait_batches_records_in_one_transaction() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    captured: dict[str, object] = {}
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    source = account_address(0x11)
    dest = account_address(0x22)
    fees = account_address(0x33)

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "transfer-batch"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    result = client.transfer_assets_and_wait(
        chain_id="chain",
        authority="source@is",
        fee_payment=FEE_PAYMENT,
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
        fee_payment=FEE_PAYMENT,
        private_key_hex="11" * 32,
        account_id=account,
        permission_name="CanEnrollFeeSponsorProgram",
        permission_payload={"program_id": "sponsor@is/retail"},
        transaction_metadata={"purpose": "fee-sponsor-program"},
        wait=False,
    )
    revoke = client.revoke_account_permission_and_wait(
        chain_id="chain",
        authority="authority@is",
        fee_payment=FEE_PAYMENT,
        private_key_hex="22" * 32,
        account_id=account,
        permission_name="CanEnrollFeeSponsorProgram",
        permission_payload={"program_id": "sponsor@is/retail"},
        wait=True,
    )

    assert grant == {"hash": "permission-1"}
    assert revoke == {"hash": "permission-2"}
    grant_draft, grant_kwargs = captured[0]
    revoke_draft, revoke_kwargs = captured[1]
    assert len(grant_draft) == 1
    assert len(revoke_draft) == 1
    assert grant_draft.config.metadata == {"purpose": "fee-sponsor-program"}
    assert grant_kwargs["private_key_hex"] == "11" * 32
    assert grant_kwargs["wait"] is False
    assert revoke_kwargs["private_key_hex"] == "22" * 32
    assert revoke_kwargs["wait"] is True


def test_permission_grant_normalizes_configured_chain_discriminant_for_transaction_draft() -> None:
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([]),
        max_retries=0,
        chain_discriminant=0x0171,
    )
    captured: dict[str, object] = {}
    account = account_address(0x45, 0x0171)
    fixed_account = account_address(0x45)

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "permission-taira"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    assert client.grant_account_permission_and_wait(
        chain_id="chain",
        authority=account,
        fee_payment=FEE_PAYMENT,
        private_key_hex="11" * 32,
        account_id=account,
        permission_name="CanEnrollFeeSponsorProgram",
        permission_payload={"program_id": "sponsor@is/retail"},
        wait=False,
    ) == {"hash": "permission-taira"}

    draft = captured["draft"]
    assert draft.config.authority == fixed_account
    assert len(draft) == 1
    assert captured["kwargs"]["private_key_hex"] == "11" * 32


def test_transfer_helper_normalizes_configured_chain_discriminant_for_transaction_draft() -> None:
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([]),
        max_retries=0,
        chain_discriminant=0x0171,
    )
    captured: dict[str, object] = {}
    source = account_address(0x46, 0x0171)
    destination = account_address(0x47, 0x0171)
    fixed_source = account_address(0x46)
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "transfer-taira"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    assert client.transfer_asset_and_wait(
        chain_id="chain",
        authority=source,
        fee_payment=FEE_PAYMENT,
        private_key_hex="22" * 32,
        asset_id=f"{asset_definition_id}#{source}",
        destination=destination,
        quantity=Decimal("3"),
        wait=False,
    ) == {"hash": "transfer-taira"}

    draft = captured["draft"]
    assert draft.config.authority == fixed_source
    assert len(draft) == 1
    assert captured["kwargs"]["private_key_hex"] == "22" * 32


def test_transfer_helper_normalizes_scoped_asset_id_account_segment() -> None:
    client = ToriiClient(
        "http://torii.example",
        session=FakeSession([]),
        max_retries=0,
        chain_discriminant=0x0171,
    )
    captured: dict[str, object] = {}
    source = account_address(0x48, 0x0171)
    destination = account_address(0x49, 0x0171)
    fixed_source = account_address(0x48)
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    scope = "dataspace:6647857470246403404"

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured["draft"] = draft
        captured["kwargs"] = kwargs
        return {"hash": "transfer-taira-scoped"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    assert client.transfer_asset_and_wait(
        chain_id="chain",
        authority=source,
        fee_payment=FEE_PAYMENT,
        private_key_hex="23" * 32,
        asset_id=f"{asset_definition_id}#{source}#{scope}",
        destination=destination,
        quantity=Decimal("3"),
        wait=False,
    ) == {"hash": "transfer-taira-scoped"}

    draft = captured["draft"]
    assert draft.config.authority == fixed_source
    assert len(draft) == 1
    assert captured["kwargs"]["private_key_hex"] == "23" * 32


def test_zk_instruction_helpers_serialize_full_surface() -> None:
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    source = account_address(0x61)
    destination = account_address(0x62)
    proof = canonical_proof_attachment()
    proof["vk_commitment"] = b"\x44" * 32
    proof["envelope_hash"] = iroha_hash_bytes(b"proof-bytes")

    instructions = [
        Instruction.register_zk_asset(
            asset_definition_id,
            vk_transfer="halo2/ipa:vk_transfer",
            vk_unshield={"backend": "halo2/ipa", "name": "vk_unshield"},
        ),
        Instruction.shield_asset(
            asset_definition_id,
            source,
            "340282366920938463463374607431768211456.25",
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
            "18446744073709551616.25",
            ["dd" * 32],
            proof,
            outputs=["ee" * 32],
            root_hint="ff" * 32,
        ),
        Instruction.verify_proof(proof),
    ]

    encoded = [instruction.to_json() for instruction in instructions]
    assert all(payload for payload in encoded)
    json_roundtrip_indexes = [0, 1]
    assert [
        Instruction.from_json(encoded[index]).to_json()
        for index in json_roundtrip_indexes
    ] == [encoded[index] for index in json_roundtrip_indexes]


def test_legacy_zk_ace_instruction_and_client_surfaces_are_absent() -> None:
    for name in (
        "register_zk_ace_identity_commitment",
        "rotate_zk_ace_identity_commitment",
        "revoke_zk_ace_identity_commitment",
        "zk_ace_authorized_transfer",
    ):
        assert not hasattr(Instruction, name)
        assert not hasattr(TransactionDraft, name)

    for name in (
        "register_zk_ace_identity_commitment_and_wait",
        "rotate_zk_ace_identity_commitment_and_wait",
        "revoke_zk_ace_identity_commitment_and_wait",
        "zk_ace_authorized_transfer_and_wait",
    ):
        assert not hasattr(ToriiClient, name)

    for name in (
        "sign_privacy_zk_ace_transfer_action_v1",
        "sign_privacy_anonymous_pgc_payment_action_v1",
        "sign_privacy_orchard_note_action_v1",
        "sign_privacy_fcmp_membership_payment_action_v1",
        "sign_privacy_ivm_private_note_action_v1",
        "sign_privacy_pq_masp_note_action_v1",
    ):
        assert hasattr(TransactionDraft, name)


@pytest.mark.parametrize(
    "method_name",
    (
        "sign_privacy_anonymous_pgc_payment_action_v1",
        "sign_privacy_orchard_note_action_v1",
        "sign_privacy_fcmp_membership_payment_action_v1",
        "sign_privacy_ivm_private_note_action_v1",
        "sign_privacy_pq_masp_note_action_v1",
    ),
)
def test_native_privacy_bundle_actions_reject_nonempty_drafts(method_name: str) -> None:
    draft = TransactionDraft(
        TransactionConfig(
            chain_id="chain",
            authority=account_address(0x73),
            fee_payment=FEE_PAYMENT,
        )
    ).use_executable_batch()

    with pytest.raises(ValueError, match="otherwise empty"):
        getattr(draft, method_name)(
            b"untrusted-bundle",
            public_action_json=b"{}",
            canonical_genesis_hash=b"\x01" * 32,
        )


def test_zk_instruction_helpers_accept_tuple_inputs() -> None:
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    proof = canonical_proof_attachment()

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
            fee_payment=authority_fee_payment(charge_limits=[]),
        )
    )

    draft.shield_asset(
        "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
        account_address(0x65),
        Decimal("340282366920938463463374607431768211456.25"),
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
            canonical_proof_attachment(vk_backend="other"),
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
                "1.0",
                "11" * 32,
                "22" * 32,
                "33" * 24,
                b"ciphertext",
            ),
            ValueError,
            "amount",
            id="shield-noncanonical-amount",
        ),
        pytest.param(
            lambda asset, source, _destination, _proof: Instruction.shield_asset(
                asset,
                source,
                str(2**511),
                "11" * 32,
                "22" * 32,
                "33" * 24,
                b"ciphertext",
            ),
            ValueError,
            "amount",
            id="shield-numeric-v1-overflow",
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
                {**proof, "proof": {"backend": "halo2/ipa"}},
            ),
            ValueError,
            "proof.bytes",
            id="transfer-missing-proof-bytes",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                ["bb" * 32],
                {**proof, "proof": {"backend": "halo2/ipa", "bytes": b""}},
            ),
            ValueError,
            "proof.bytes",
            id="transfer-empty-proof",
        ),
        pytest.param(
            lambda asset, _source, _destination, proof: Instruction.zk_transfer_prepared(
                asset,
                ["aa" * 32],
                ["bb" * 32],
                {**proof, "proof_b64": "cHJvb2Y="},
            ),
            ValueError,
            "unknown first-release field",
            id="transfer-retired-flat-base64-proof",
        ),
        pytest.param(
            lambda asset, _source, destination, proof: Instruction.unshield_prepared(
                asset,
                destination,
                "-1",
                ["aa" * 32],
                canonical_proof_attachment(vk_name="vk_unshield"),
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
                canonical_proof_attachment(vk_name="vk_unshield"),
                outputs=["bb" * 32, "bb" * 32],
            ),
            ValueError,
            "duplicates",
            id="unshield-duplicate-output",
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
    proof = canonical_proof_attachment()

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
                "01",
                note_commitment="11" * 32,
                ephemeral_public_key="22" * 32,
                nonce="33" * 24,
                ciphertext=b"raw",
            ),
            ValueError,
            "canonical",
            id="shield-noncanonical-amount",
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
                proof=canonical_proof_attachment(vk_name="vk_unshield"),
            ),
            ValueError,
            "public_amount",
            id="unshield-negative-public-amount",
        ),
    ],
)
def test_zk_transaction_draft_rejects_invalid_inputs(
    call,
    error_type: type[Exception],
    match: str,
) -> None:
    account = account_address(0x68)
    draft = TransactionDraft(
        TransactionConfig(
            chain_id="chain",
            authority=account,
            fee_payment=authority_fee_payment(charge_limits=[]),
        )
    )
    proof = canonical_proof_attachment()

    with pytest.raises(error_type, match=match):
        call(draft, "7MBRDd8cGFBZkFGdDMwV7S6FPwbw", account, proof)


def test_zk_client_helpers_build_transaction_drafts() -> None:
    client = ToriiClient("http://torii.example", session=FakeSession([]), max_retries=0)
    captured: list[tuple[object, dict[str, object]]] = []
    asset_definition_id = "7MBRDd8cGFBZkFGdDMwV7S6FPwbw"
    source = account_address(0x63)
    destination = account_address(0x64)
    proof = canonical_proof_attachment()

    def fake_submit(draft: object, **kwargs: object) -> dict[str, object]:
        captured.append((draft, kwargs))
        return {"hash": f"zk-{len(captured)}"}

    client._submit_transaction_draft_result = fake_submit  # type: ignore[method-assign]

    assert client.register_zk_asset_and_wait(
        chain_id="chain",
        authority="authority@is",
        fee_payment=FEE_PAYMENT,
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
        fee_payment=FEE_PAYMENT,
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
        fee_payment=FEE_PAYMENT,
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
        fee_payment=FEE_PAYMENT,
        private_key_hex="44" * 32,
        asset_definition_id=asset_definition_id,
        to_account_id=destination,
        public_amount="3",
        inputs=["dd" * 32],
        outputs=["ee" * 32],
        proof=proof,
        root_hint="ff" * 32,
    ) == {"hash": "zk-4"}
    assert client.verify_proof_and_wait(
        chain_id="chain",
        authority=source,
        fee_payment=FEE_PAYMENT,
        private_key_hex="bb" * 32,
        proof=proof,
        wait=False,
    ) == {"hash": "zk-5"}

    assert [len(draft) for draft, _kwargs in captured] == [1] * 5
    assert captured[0][0].config.metadata == {"purpose": "zk-register"}
    assert captured[0][1]["wait"] is False
    assert captured[1][1]["private_key_hex"] == "22" * 32
    assert captured[4][1]["private_key_hex"] == "bb" * 32


def test_zk_ace_transaction_amount_boundary_is_canonical_and_exact() -> None:
    u128_max = str((1 << 128) - 1)
    assert _require_canonical_positive_u128_literal("17", "amount") == "17"
    assert _require_canonical_positive_u128_literal(u128_max, "amount") == u128_max

    for amount in [
        None,
        True,
        False,
        23,
        0,
        -1,
        1.5,
        Decimal("1"),
        "",
        " ",
        "0",
        "00",
        "01",
        "00017",
        "-1",
        "+1",
        "1.0",
        "1e3",
        1 << 128,
        str(1 << 128),
        [],
        object(),
    ]:
        with pytest.raises(
            (TypeError, ValueError),
            match="amount must be a canonical positive decimal u128 string",
        ):
            _require_canonical_positive_u128_literal(amount, "amount")


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
                    "proof": {"backend": "halo2/ipa", "bytes": b"proof-bytes"},
                    "vk_ref": {"backend": "halo2/ipa", "name": "vk_transfer"},
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
            "verify_proof_and_wait",
            {"proof": object()},
            TypeError,
            "proof must be a mapping",
            id="verify-proof-not-mapping",
        ),
        pytest.param(
            "unshield_prepared_and_wait",
            {
                "asset_definition_id": "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "to_account_id": account_address(0x6A),
                "public_amount": 1,
                "inputs": ["aa" * 32],
                "proof": {
                    "proof": {"backend": "halo2/ipa", "bytes": b"proof-bytes"},
                    "vk_ref": {"backend": "halo2/ipa", "name": "vk_unshield"},
                },
            },
            ValueError,
            "backend",
            id="unshield-missing-proof-backend",
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
            fee_payment=FEE_PAYMENT,
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
            fee_payment=FEE_PAYMENT,
            private_key_hex="11" * 32,
            **kwargs,
        )


@pytest.mark.parametrize(
    ("kwargs", "match"),
    [
        (
            {
                "account_id": "adult@is",
                "permission_name": "CanEnrollFeeSponsorProgram",
            },
            "invalid account id",
        ),
        (
            {"account_id": account_address(0x49), "permission_name": ""},
            "permission name",
        ),
        (
            {
                "account_id": account_address(0x4A),
                "permission_name": "CanEnrollFeeSponsorProgram",
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
            fee_payment=FEE_PAYMENT,
            private_key_hex="11" * 32,
            **kwargs,
        )
