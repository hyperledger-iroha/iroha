"""Configuration-once Iroha API for application code.

The lower SDK layers remain available for custom transaction construction.
This module is the small, typed surface used by the Bank of Israel walkthroughs.
"""

from __future__ import annotations

from collections.abc import Iterator, Mapping, Sequence
from dataclasses import dataclass
from enum import Enum
from typing import Any, Optional, Union

from .client import ToriiClient
from .tx import authority_fee_payment

__all__ = [
    "AggregateScope",
    "BatchMode",
    "DeadlineCondition",
    "EscrowRelease",
    "FundingMode",
    "IrohaClient",
    "OracleCondition",
    "Payment",
    "Signer",
    "TransactionReceipt",
]


class AggregateScope(str, Enum):
    """Privacy-preserving aggregate view exposed by Torii."""

    OPERATOR = "operator"


class BatchMode(str, Enum):
    """Commit behavior for one native multi-payment instruction."""

    ATOMIC = "Atomic"
    INDEPENDENT = "Independent"


class EscrowRelease(str, Enum):
    """Native conditional-escrow release policy."""

    ALL_CONDITIONS = "AllConditions"


class FundingMode(str, Enum):
    """Funding policy for a transfer.

    ``DIRECT`` is the native balance transfer. ``WATERFALL`` is reserved for
    deployments with the BOI wallet-policy contract and is deliberately not
    emulated by the SDK.
    """

    DIRECT = "Direct"
    WATERFALL = "Waterfall"


@dataclass(frozen=True)
class Payment:
    """One correlated leg in a native batch transfer."""

    id: str
    to: str
    amount: Any

    def to_payload(self) -> dict[str, Any]:
        """Return the exact native SDK payload."""

        return {"id": self.id, "to": self.to, "amount": self.amount}


@dataclass(frozen=True)
class OracleCondition:
    """One ordered, typed oracle predicate."""

    id: str
    predicate_kind: str
    predicate_value: Any
    attestor: str
    order: int

    @classmethod
    def equals(
        cls,
        claim: str,
        expected: str,
        attestor: str,
        *,
        order: int,
    ) -> "OracleCondition":
        """Require an oracle to attest exact text equality."""

        if not isinstance(expected, str):
            raise TypeError("text equality expected value must be a string")
        return cls(
            id=claim,
            predicate_kind="text_equals",
            predicate_value=expected,
            attestor=attestor,
            order=order,
        )

    @classmethod
    def at_most(
        cls,
        claim: str,
        maximum: Any,
        attestor: str,
        *,
        order: int,
    ) -> "OracleCondition":
        """Require an oracle quantity no greater than ``maximum``."""

        return cls(
            id=claim,
            predicate_kind="quantity_at_most",
            predicate_value=maximum,
            attestor=attestor,
            order=order,
        )

    def to_payload(self) -> dict[str, Any]:
        """Return the exact native SDK payload."""

        return {
            "kind": "oracle",
            "id": self.id,
            "attestor": self.attestor,
            "sequence": self.order,
            "predicate_kind": self.predicate_kind,
            "predicate_value": str(self.predicate_value),
        }


@dataclass(frozen=True)
class DeadlineCondition:
    """Ledger-time deadline relative to escrow creation."""

    id: str
    duration_ms: int

    @classmethod
    def within(
        cls,
        *,
        days: int = 0,
        hours: int = 0,
        minutes: int = 0,
        seconds: int = 0,
        milliseconds: int = 0,
        id: str = "deadline",
    ) -> "DeadlineCondition":
        """Build a positive relative deadline without timestamp arithmetic."""

        parts = {
            "days": days,
            "hours": hours,
            "minutes": minutes,
            "seconds": seconds,
            "milliseconds": milliseconds,
        }
        for name, value in parts.items():
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise ValueError(f"{name} must be a non-negative integer")
        duration_ms = (
            days * 86_400_000
            + hours * 3_600_000
            + minutes * 60_000
            + seconds * 1_000
            + milliseconds
        )
        if duration_ms <= 0:
            raise ValueError("deadline duration must be positive")
        return cls(id=id, duration_ms=duration_ms)

    def to_payload(self) -> dict[str, Any]:
        """Return the exact native SDK payload."""

        return {"kind": "within", "id": self.id, "duration_ms": self.duration_ms}


@dataclass(frozen=True)
class Signer:
    """Configured account identity and its local Ed25519 private key."""

    account_id: str
    private_key: Optional[bytes] = None
    private_key_hex: Optional[str] = None

    def __post_init__(self) -> None:
        if not isinstance(self.account_id, str) or not self.account_id.strip():
            raise ValueError("account_id must be a non-empty string")
        if self.account_id != self.account_id.strip():
            raise ValueError("account_id must not contain surrounding whitespace")
        if (self.private_key is None) == (self.private_key_hex is None):
            raise ValueError("provide exactly one private key representation")

    @classmethod
    def ed25519(
        cls,
        account_id: str,
        private_key: Union[str, bytes, bytearray, memoryview],
    ) -> "Signer":
        """Configure an Ed25519 signer from raw bytes or a hex string."""

        if isinstance(private_key, str):
            return cls(account_id=account_id, private_key_hex=private_key)
        if isinstance(private_key, (bytes, bytearray, memoryview)):
            return cls(account_id=account_id, private_key=bytes(private_key))
        raise TypeError("private_key must be bytes or a hex string")

    def signing_arguments(self) -> dict[str, Any]:
        """Return arguments expected by the lower Torii client."""

        if self.private_key is not None:
            return {"private_key": self.private_key}
        return {"private_key_hex": self.private_key_hex}


@dataclass(frozen=True)
class TransactionReceipt(Mapping[str, Any]):
    """Typed view over the complete lower-level submission result."""

    raw: Mapping[str, Any]

    @property
    def hash(self) -> str:
        """Return the committed transaction hash."""

        direct = self.raw.get("hash") or self.raw.get("tx_hash_hex")
        if direct:
            return str(direct)
        hashes = self.raw.get("tx_hashes")
        if isinstance(hashes, Sequence) and not isinstance(hashes, (str, bytes, bytearray)):
            return str(hashes[0]) if hashes else ""
        return ""

    @property
    def terminal(self) -> Any:
        """Return the terminal status payload, if waiting was requested."""

        return self.raw.get("terminal") or self.raw.get("r#final") or self.raw.get("terminal_kind")

    def __getitem__(self, key: str) -> Any:
        return self.raw[key]

    def __iter__(self) -> Iterator[str]:
        return iter(self.raw)

    def __len__(self) -> int:
        return len(self.raw)


Condition = Union[OracleCondition, DeadlineCondition, Mapping[str, Any]]


def _condition_payload(condition: Condition) -> Mapping[str, Any]:
    if isinstance(condition, (OracleCondition, DeadlineCondition)):
        return condition.to_payload()
    if isinstance(condition, Mapping):
        return dict(condition)
    raise TypeError("conditions must contain condition objects or mappings")


class _AccountsApi:
    def __init__(self, client: "IrohaClient") -> None:
        self._client = client

    def register_many(
        self,
        *,
        accounts: Sequence[str],
        metadata: Optional[Mapping[str, Mapping[str, Any]]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
    ) -> TransactionReceipt:
        """Register several accounts atomically in one native transaction."""

        result = self._client._torii.register_accounts_and_wait(
            **self._client._transaction_arguments(),
            accounts=accounts,
            account_metadata=metadata,
            wait=wait,
            timeout=timeout,
        )
        return TransactionReceipt(result)


class _AssetsApi:
    def __init__(self, client: "IrohaClient") -> None:
        self._client = client

    def transfer_batch(
        self,
        asset_definition_id: str,
        payments: Sequence[Union[Payment, Mapping[str, Any]]],
        *,
        source_account: Optional[str] = None,
        mode: BatchMode = BatchMode.INDEPENDENT,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
    ) -> TransactionReceipt:
        """Submit one native batch transaction with independently evaluated legs."""

        payment_payloads = [
            payment.to_payload() if isinstance(payment, Payment) else dict(payment)
            for payment in payments
        ]
        mode_value = mode.value if isinstance(mode, BatchMode) else str(mode)
        result = self._client._torii.transfer_asset_batch_and_wait(
            **self._client._transaction_arguments(),
            asset_definition_id=asset_definition_id,
            source_account=source_account or self._client.signer.account_id,
            payments=payment_payloads,
            mode=mode_value,
            wait=wait,
            timeout=timeout,
        )
        return TransactionReceipt(result)

    def transfer(
        self,
        *,
        asset_definition_id: str,
        destination: str,
        amount: Any,
        source_account: Optional[str] = None,
        funding: FundingMode = FundingMode.DIRECT,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
    ) -> TransactionReceipt:
        """Transfer one native asset balance.

        Waterfall funding is a BOI application-contract policy, not a native
        transfer flag. It is rejected here instead of silently degrading to a
        direct transfer.
        """

        funding_value = funding.value if isinstance(funding, FundingMode) else str(funding)
        if funding_value != FundingMode.DIRECT.value:
            raise NotImplementedError(
                "waterfall funding requires the configured BOI wallet-policy "
                "contract API; it cannot be emulated by a native transfer"
            )
        source = source_account or self._client.signer.account_id
        canonical_definition = self._client._torii.resolve_asset_definition_id(asset_definition_id)
        result = self._client._torii.transfer_asset_quantity_and_wait(
            **self._client._transaction_arguments(),
            asset_id=self._client._torii.compose_asset_id(
                canonical_definition,
                source,
            ),
            quantity=amount,
            destination=destination,
            wait=wait,
            timeout=timeout,
        )
        return TransactionReceipt(result)

    def balance(self, account_id: str, asset_definition_id: str) -> str:
        """Return one account's canonical quantity, or ``"0"`` if absent."""

        quantity = self._client._torii.asset_balance(
            account_id,
            asset_definition_id,
        )
        return str(quantity)

    def set_holding_limit(
        self,
        account_id: str,
        asset_definition_id: str,
        holding_limit: Optional[Any],
        *,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
    ) -> TransactionReceipt:
        """Set or clear a ledger-enforced account balance ceiling."""

        result = self._client._torii.set_asset_holding_limit_and_wait(
            **self._client._transaction_arguments(),
            account_id=account_id,
            asset_definition_id=asset_definition_id,
            holding_limit=holding_limit,
            wait=wait,
            timeout=timeout,
        )
        return TransactionReceipt(result)


class _QueriesApi:
    def __init__(self, client: "IrohaClient") -> None:
        self._client = client

    def aggregate_statistics(
        self,
        *,
        asset_definition_id: str,
        scope: AggregateScope = AggregateScope.OPERATOR,
    ) -> Mapping[str, Any]:
        """Return exact holder count and quantity without account-level rows."""

        scope_value = scope.value if isinstance(scope, AggregateScope) else str(scope)
        if scope_value != AggregateScope.OPERATOR.value:
            raise ValueError("scope must be AggregateScope.OPERATOR")
        response = self._client._torii.query_asset_holders(
            asset_definition_id,
            aggregate={
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
            },
            limit=1,
            count_mode="exact",
            query_name="boi_operator_asset_statistics_v1",
        )
        items = response.get("items")
        if not isinstance(items, list) or len(items) != 1:
            raise RuntimeError("Torii aggregate query must return exactly one row")
        totals = items[0]
        if not isinstance(totals, Mapping):
            raise RuntimeError("Torii aggregate row must be an object")
        return {
            "asset_definition_id": asset_definition_id,
            "scope": scope_value,
            "totals": dict(totals),
            "indexed_height": response.get("indexed_height"),
            "indexed_block_hash": response.get("indexed_block_hash"),
            "query_source": response.get("query_source"),
        }


class _ContractsApi:
    def __init__(self, client: "IrohaClient") -> None:
        self._client = client

    def call(
        self,
        *,
        alias: str,
        entrypoint: str,
        arguments: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 120.0,
        gas_limit: int = 10_000_000,
    ) -> TransactionReceipt:
        """Call one deployed contract entrypoint using the configured actor.

        ``arguments`` is passed through as the contract payload. This generic
        surface intentionally exposes real deployed-contract semantics instead
        of inventing native instructions for BOI application policies.
        """

        if arguments is not None and not isinstance(arguments, Mapping):
            raise TypeError("arguments must be a mapping when provided")
        if timeout is not None and timeout < 0:
            raise ValueError("timeout must be non-negative or None")
        private_key = (
            self._client.signer.private_key_hex
            if self._client.signer.private_key_hex is not None
            else self._client.signer.private_key.hex()
        )
        result = self._client._torii.call_contract_and_wait(
            authority=self._client.signer.account_id,
            private_key=private_key,
            fee_payment=self._client._contract_fee_payment(gas_limit),
            contract_alias=alias,
            entrypoint=entrypoint,
            payload=dict(arguments) if arguments is not None else None,
            wait=wait,
            timeout_ms=None if timeout is None else int(timeout * 1_000),
        )
        if not isinstance(result, Mapping):
            raise RuntimeError("contract call returned a non-object response")
        return TransactionReceipt(dict(result))


class _EscrowsApi:
    def __init__(self, client: "IrohaClient") -> None:
        self._client = client

    def open(
        self,
        escrow_id: str,
        asset_definition_id: str,
        amount: Any,
        beneficiary: str,
        conditions: Sequence[Condition],
        *,
        release: EscrowRelease = EscrowRelease.ALL_CONDITIONS,
        expires_at_ms: Optional[int] = None,
        evidence_hashes: Optional[Sequence[Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
    ) -> TransactionReceipt:
        """Open one native escrow; all conditions are stored and queryable."""

        release_value = release.value if isinstance(release, EscrowRelease) else str(release)
        result = self._client._torii.open_conditional_escrow_and_wait(
            **self._client._transaction_arguments(),
            escrow_id=escrow_id,
            asset_definition_id=asset_definition_id,
            amount=amount,
            beneficiary=beneficiary,
            conditions=[_condition_payload(condition) for condition in conditions],
            release_policy=release_value,
            expires_at_ms=expires_at_ms,
            evidence_hashes=evidence_hashes,
            wait=wait,
            timeout=timeout,
        )
        return TransactionReceipt(result)

    def attest(
        self,
        escrow_id: str,
        claim: str,
        value: Any,
        *,
        evidence_digest: Optional[Any] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
    ) -> TransactionReceipt:
        """Attest one condition; the last passing condition releases automatically."""

        result = self._client._torii.attest_escrow_condition_and_wait(
            **self._client._transaction_arguments(),
            escrow_id=escrow_id,
            claim=claim,
            value=value,
            evidence_digest=evidence_digest,
            wait=wait,
            timeout=timeout,
        )
        return TransactionReceipt(result)

    def get(self, escrow_id: str) -> Mapping[str, Any]:
        """Query the complete native escrow record and condition state."""

        return self._client._torii.get_asset_escrow(
            escrow_id=escrow_id,
            authority=self._client.signer.account_id,
            **self._client.signer.signing_arguments(),
        )

    def expire(
        self,
        escrow_id: str,
        *,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
    ) -> TransactionReceipt:
        """Expire a timed-out escrow and return custody to its opener."""

        result = self._client._torii.expire_conditional_escrow_and_wait(
            **self._client._transaction_arguments(),
            escrow_id=escrow_id,
            wait=wait,
            timeout=timeout,
        )
        return TransactionReceipt(result)


class IrohaClient:
    """Configure Torii, chain identity, signer, and fees once."""

    def __init__(
        self,
        torii_url: str,
        chain_id: str,
        signer: Signer,
        fees: Union[str, Mapping[str, Any]] = "auto",
        *,
        torii_client: Optional[ToriiClient] = None,
    ) -> None:
        if not isinstance(chain_id, str) or not chain_id.strip():
            raise ValueError("chain_id must be a non-empty string")
        if not isinstance(signer, Signer):
            raise TypeError("signer must be Signer.ed25519(...)")
        if fees == "auto":
            fee_payment: Mapping[str, Any] = authority_fee_payment(charge_limits=[])
            self._auto_fee_quote = True
        elif isinstance(fees, Mapping):
            fee_payment = dict(fees)
            self._auto_fee_quote = False
        else:
            raise ValueError("fees must be 'auto' or an explicit fee-payment mapping")
        self.chain_id = chain_id
        self.signer = signer
        self.fee_payment = fee_payment
        self._torii = torii_client or ToriiClient(torii_url)
        self.accounts = _AccountsApi(self)
        self.assets = _AssetsApi(self)
        self.contracts = _ContractsApi(self)
        self.escrows = _EscrowsApi(self)
        self.queries = _QueriesApi(self)

    def _transaction_arguments(self) -> dict[str, Any]:
        return {
            "chain_id": self.chain_id,
            "authority": self.signer.account_id,
            "fee_payment": self.fee_payment,
            "auto_fee_quote": self._auto_fee_quote,
            **self.signer.signing_arguments(),
        }

    def _contract_fee_payment(self, gas_limit: int) -> Mapping[str, Any]:
        """Return a contract-call intent with the mandatory gas limit."""

        if self._auto_fee_quote:
            return authority_fee_payment(charge_limits=[], gas_limit=gas_limit)
        return self.fee_payment
