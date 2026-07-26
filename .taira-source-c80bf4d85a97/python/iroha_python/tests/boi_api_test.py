from __future__ import annotations

from decimal import Decimal
from typing import Any

import pytest

from iroha_python import (
    AggregateScope,
    BatchMode,
    DeadlineCondition,
    EscrowRelease,
    IrohaClient,
    OracleCondition,
    Payment,
    Signer,
)


class _FakeTorii:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, Any]]] = []

    def _record(self, name: str, kwargs: dict[str, Any]) -> dict[str, Any]:
        self.calls.append((name, kwargs))
        return {"hash": f"{name}-hash", "terminal": {"status": "Committed"}}

    def transfer_asset_batch_and_wait(self, **kwargs: Any) -> dict[str, Any]:
        return self._record("batch", kwargs)

    def register_accounts_and_wait(self, **kwargs: Any) -> dict[str, Any]:
        return self._record("register-many", kwargs)

    def transfer_asset_quantity_and_wait(self, **kwargs: Any) -> dict[str, Any]:
        return self._record("transfer", kwargs)

    def set_asset_holding_limit_and_wait(self, **kwargs: Any) -> dict[str, Any]:
        return self._record("holding-limit", kwargs)

    def open_conditional_escrow_and_wait(self, **kwargs: Any) -> dict[str, Any]:
        return self._record("open-escrow", kwargs)

    def attest_escrow_condition_and_wait(self, **kwargs: Any) -> dict[str, Any]:
        return self._record("attest", kwargs)

    def expire_conditional_escrow_and_wait(self, **kwargs: Any) -> dict[str, Any]:
        return self._record("expire", kwargs)

    def get_asset_escrow(self, **kwargs: Any) -> dict[str, Any]:
        self.calls.append(("get", kwargs))
        return {"id": kwargs["escrow_id"], "conditions": []}

    def compose_asset_id(
        self,
        asset_definition_id: str,
        account_id: str,
    ) -> str:
        return f"{asset_definition_id}::{account_id}"

    def resolve_asset_definition_id(self, selector: str) -> str:
        assert selector == "ds#is"
        return "6pEP9RjNoZ7beWkT3pLfKoM1dyfi"

    def asset_balance(
        self,
        account_id: str,
        asset_definition_id: str,
    ) -> Decimal:
        self.calls.append(
            (
                "balance",
                {
                    "account_id": account_id,
                    "asset_definition_id": asset_definition_id,
                },
            )
        )
        return Decimal("42")

    def query_asset_holders(
        self,
        asset_definition_id: str,
        **kwargs: Any,
    ) -> dict[str, Any]:
        self.calls.append(
            (
                "aggregate",
                {"asset_definition_id": asset_definition_id, **kwargs},
            )
        )
        return {
            "items": [{"wallet_count": 3, "total_quantity": "123"}],
            "indexed_height": 77,
            "indexed_block_hash": "aa" * 32,
            "query_source": "indexed",
        }

    def call_contract_and_wait(self, **kwargs: Any) -> dict[str, Any]:
        self.calls.append(("contract", kwargs))
        return {
            "tx_hashes": ["cc" * 32],
            "terminal_kind": "Committed",
            "r#final": {"status": {"kind": "Committed"}},
        }


def _client() -> tuple[IrohaClient, _FakeTorii]:
    torii = _FakeTorii()
    signer = Signer.ed25519("authority@test", bytes([7]) * 32)
    return (
        IrohaClient(
            "http://localhost:8080",
            "boi-poc",
            signer,
            fees="auto",
            torii_client=torii,
        ),
        torii,
    )


def test_exact_walkthrough_batch_surface_builds_one_independent_instruction() -> None:
    client, torii = _client()

    receipt = client.assets.transfer_batch(
        "ds#is",
        [
            Payment(id="israel", to="israel@test", amount="20"),
            Payment(id="roman", to="roman@test", amount="60"),
        ],
        mode=BatchMode.INDEPENDENT,
    )

    assert receipt.hash == "batch-hash"
    name, kwargs = torii.calls[-1]
    assert name == "batch"
    assert kwargs["asset_definition_id"] == "ds#is"
    assert kwargs["source_account"] == "authority@test"
    assert kwargs["mode"] == "Independent"
    assert kwargs["payments"] == [
        {"id": "israel", "to": "israel@test", "amount": "20"},
        {"id": "roman", "to": "roman@test", "amount": "60"},
    ]


def test_register_many_is_one_real_native_transaction() -> None:
    client, torii = _client()

    receipt = client.accounts.register_many(
        accounts=["israel@test", "roman@test"],
        metadata={"israel@test": {"role": "merchant"}},
    )

    assert receipt.hash == "register-many-hash"
    name, kwargs = torii.calls[-1]
    assert name == "register-many"
    assert kwargs["accounts"] == ["israel@test", "roman@test"]
    assert kwargs["account_metadata"] == {"israel@test": {"role": "merchant"}}


def test_native_transfer_and_balance_use_exact_asset_identifier() -> None:
    client, torii = _client()

    receipt = client.assets.transfer(
        asset_definition_id="ds#is",
        destination="israel@test",
        amount="20",
    )

    assert receipt.hash == "transfer-hash"
    name, kwargs = torii.calls[-1]
    assert name == "transfer"
    assert kwargs["asset_id"] == "6pEP9RjNoZ7beWkT3pLfKoM1dyfi::authority@test"
    assert kwargs["destination"] == "israel@test"
    assert kwargs["quantity"] == "20"
    assert client.assets.balance("israel@test", "ds#is") == "42"


def test_operator_statistics_use_one_privacy_preserving_aggregate_query() -> None:
    client, torii = _client()

    statistics = client.queries.aggregate_statistics(
        asset_definition_id="ds#is",
        scope=AggregateScope.OPERATOR,
    )

    assert statistics["totals"] == {
        "wallet_count": 3,
        "total_quantity": "123",
    }
    name, kwargs = torii.calls[-1]
    assert name == "aggregate"
    assert kwargs["asset_definition_id"] == "ds#is"
    assert kwargs["aggregate"] == {
        "group_by": [],
        "metrics": [
            {
                "alias": "wallet_count",
                "fn": "distinct_count",
                "field": "account_id",
            },
            {
                "alias": "total_quantity",
                "fn": "sum",
                "field": "quantity",
            },
        ],
    }
    assert "select" not in kwargs


def test_contract_call_exposes_real_actor_entrypoint_and_arguments() -> None:
    client, torii = _client()

    receipt = client.contracts.call(
        alias="wallet_registry::is",
        entrypoint="record_wallet_consent",
        arguments={
            "consent_id": "consent-7",
            "subject": "customer@is",
            "primary_psp": "psp@is",
            "consent_digest": "ab" * 32,
            "expires_at_ms": 1_800_000_000_000,
        },
    )

    assert receipt.hash == "cc" * 32
    assert receipt.terminal == {"status": {"kind": "Committed"}}
    name, kwargs = torii.calls[-1]
    assert name == "contract"
    assert kwargs["authority"] == "authority@test"
    assert kwargs["private_key"] == (bytes([7]) * 32).hex()
    assert kwargs["contract_alias"] == "wallet_registry::is"
    assert kwargs["entrypoint"] == "record_wallet_consent"
    assert kwargs["payload"]["subject"] == "customer@is"
    assert kwargs["fee_payment"] == {
        "payer": "authority",
        "value": {
            "charge_limits": [],
            "gas_limit": 10_000_000,
        },
    }


def test_oracle_text_equality_rejects_boolean_values() -> None:
    with pytest.raises(TypeError, match="must be a string"):
        OracleCondition.equals("approved", True, "oracle@test", order=1)


def test_exact_walkthrough_conditional_escrow_surface_serializes_three_conditions() -> None:
    client, torii = _client()
    conditions = [
        OracleCondition.equals("processor", "i7", "product@test", order=1),
        OracleCondition.at_most("delivery_days", 3, "delivery@test", order=2),
        DeadlineCondition.within(days=7),
    ]

    receipt = client.escrows.open(
        "11" * 32,
        "ds#is",
        "100",
        "beneficiary@test",
        conditions,
        release=EscrowRelease.ALL_CONDITIONS,
    )

    assert receipt.hash == "open-escrow-hash"
    name, kwargs = torii.calls[-1]
    assert name == "open-escrow"
    assert kwargs["release_policy"] == "AllConditions"
    assert kwargs["conditions"] == [
        {
            "kind": "oracle",
            "id": "processor",
            "attestor": "product@test",
            "sequence": 1,
            "predicate_kind": "text_equals",
            "predicate_value": "i7",
        },
        {
            "kind": "oracle",
            "id": "delivery_days",
            "attestor": "delivery@test",
            "sequence": 2,
            "predicate_kind": "quantity_at_most",
            "predicate_value": "3",
        },
        {"kind": "within", "id": "deadline", "duration_ms": 604_800_000},
    ]

    client.escrows.attest("11" * 32, "processor", "i7")
    assert torii.calls[-1][0] == "attest"
    assert client.escrows.get("11" * 32)["conditions"] == []
    client.escrows.expire("11" * 32)
    assert torii.calls[-1][0] == "expire"
