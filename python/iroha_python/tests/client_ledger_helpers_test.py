from __future__ import annotations

import json
from decimal import Decimal
from urllib.parse import urlsplit

import pytest
import requests

from iroha_python import AccountAddress, Ed25519KeyPair, ToriiClient


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


def account_address(seed: int) -> str:
    return Ed25519KeyPair.from_private_key(bytes([seed] * 32)).default_account_id(
        "wonderland",
        0x02F1,
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
        {"backend": "halo2/ipa", "name": "vk_transfer"}
    )

    assert response_obj.status_code == 202
    assert [call["path"] for call in session.calls] == [
        "/v1/zk/vk/halo2%2Fipa/vk_transfer",
        "/v1/zk/vk/register",
    ]


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
