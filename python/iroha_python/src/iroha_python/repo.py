"""Repo agreement helpers and dataclasses."""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal
from typing import TYPE_CHECKING, Any, Dict, Mapping, Optional, Union

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .tx import NumericLike
else:
    NumericLike = Union[str, int, float, Decimal]


def _normalize_quantity(value: NumericLike) -> str:
    from .tx import _normalize_quantity as _tx_normalize_quantity

    return _tx_normalize_quantity(value)


def _normalize_metadata(metadata: Optional[Mapping[str, Any]]) -> Optional[Mapping[str, Any]]:
    from .tx import _normalize_metadata as _tx_normalize_metadata

    return _tx_normalize_metadata(metadata)


def _page_metadata(payload: Mapping[str, Any], item_count: int) -> Dict[str, Any]:
    total_literal = payload.get("total")
    if total_literal is None:
        total = None
    else:
        try:
            total = int(total_literal)
        except (TypeError, ValueError) as exc:
            raise TypeError("repo agreement page `total` must be numeric") from exc
        if total < 0:
            raise ValueError("repo agreement page `total` must be non-negative")

    has_metadata = (
        "has_more" in payload
        or "count_mode" in payload
        or "indexed_height" in payload
        or "indexed_block_hash" in payload
        or "query_source" in payload
    )
    has_more_literal = payload.get("has_more", False)
    if not isinstance(has_more_literal, bool):
        raise TypeError("repo agreement page `has_more` must be a boolean")

    count_mode_literal = payload.get("count_mode")
    if count_mode_literal is None:
        count_mode = "exact" if total is not None and not has_metadata else "bounded"
    elif isinstance(count_mode_literal, str) and count_mode_literal in {"bounded", "exact"}:
        count_mode = count_mode_literal
    else:
        raise TypeError("repo agreement page `count_mode` must be 'bounded' or 'exact'")

    if total is None and count_mode == "exact":
        total = item_count

    indexed_height_literal = payload.get("indexed_height")
    if indexed_height_literal is None:
        indexed_height = None
    else:
        try:
            indexed_height = int(indexed_height_literal)
        except (TypeError, ValueError) as exc:
            raise TypeError("repo agreement page `indexed_height` must be numeric") from exc
        if indexed_height < 0:
            raise ValueError("repo agreement page `indexed_height` must be non-negative")

    indexed_block_hash = payload.get("indexed_block_hash")
    if indexed_block_hash is not None and not isinstance(indexed_block_hash, str):
        raise TypeError("repo agreement page `indexed_block_hash` must be a string when provided")

    query_source = payload.get("query_source")
    if query_source is not None and not isinstance(query_source, str):
        raise TypeError("repo agreement page `query_source` must be a string when provided")

    return {
        "total": total,
        "has_more": has_more_literal,
        "count_mode": count_mode,
        "indexed_height": indexed_height,
        "indexed_block_hash": indexed_block_hash,
        "query_source": query_source,
    }

__all__ = [
    "RepoCashLeg",
    "RepoCollateralLeg",
    "RepoGovernance",
    "RepoAgreementRecord",
    "RepoAgreementListPage",
]


@dataclass(frozen=True)
class RepoCashLeg:
    """Cash consideration exchanged during repo initiation and unwind."""

    asset_definition_id: str
    quantity: NumericLike
    metadata: Optional[Mapping[str, Any]] = None

    def to_payload(self) -> Mapping[str, Any]:
        payload: Dict[str, Any] = {
            "asset_definition_id": self.asset_definition_id,
            "quantity": _normalize_quantity(self.quantity),
        }
        if self.metadata:
            payload["metadata"] = _normalize_metadata(self.metadata)
        return payload


@dataclass(frozen=True)
class RepoCollateralLeg:
    """Collateral pledged for the repo lifecycle."""

    asset_definition_id: str
    quantity: NumericLike
    metadata: Optional[Mapping[str, Any]] = None

    def to_payload(self) -> Mapping[str, Any]:
        payload: Dict[str, Any] = {
            "asset_definition_id": self.asset_definition_id,
            "quantity": _normalize_quantity(self.quantity),
        }
        if self.metadata:
            payload["metadata"] = _normalize_metadata(self.metadata)
        return payload


@dataclass(frozen=True)
class RepoGovernance:
    """Governance parameters controlling haircuts and margin cadence."""

    haircut_bps: int
    margin_frequency_secs: int

    def to_payload(self) -> Mapping[str, Any]:
        return {
            "haircut_bps": int(self.haircut_bps),
            "margin_frequency_secs": int(self.margin_frequency_secs),
        }

    def margin_frequency_millis(self) -> Optional[int]:
        """Return the margin frequency in milliseconds, or ``None`` when disabled."""

        if self.margin_frequency_secs <= 0:
            return None
        return int(self.margin_frequency_secs) * 1_000


@dataclass(frozen=True)
class RepoAgreementRecord:
    """Snapshot of a repo agreement stored on-chain."""

    agreement_id: str
    initiator: str
    counterparty: str
    custodian: Optional[str]
    cash_leg: RepoCashLeg
    collateral_leg: RepoCollateralLeg
    rate_bps: int
    maturity_timestamp_ms: int
    initiated_timestamp_ms: int
    last_margin_check_timestamp_ms: int
    governance: RepoGovernance

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RepoAgreementRecord":
        """Construct a record from the Torii JSON payload."""

        def require(name: str) -> Any:
            if name not in payload:
                raise KeyError(f"repo agreement payload missing `{name}` field")
            return payload[name]

        cash_payload = require("cash_leg")
        collateral_payload = require("collateral_leg")
        governance_payload = require("governance")
        cash_leg = RepoCashLeg(
            asset_definition_id=_require_non_empty_string(
                cash_payload.get("asset_definition_id"), "repo cash_leg.asset_definition_id"
            ),
            quantity=_normalize_quantity(cash_payload.get("quantity", 0)),
            metadata=_normalize_metadata(cash_payload.get("metadata")),
        )
        collateral_meta = collateral_payload.get("metadata")
        collateral_leg = RepoCollateralLeg(
            asset_definition_id=_require_non_empty_string(
                collateral_payload.get("asset_definition_id"), "repo collateral_leg.asset_definition_id"
            ),
            quantity=_normalize_quantity(collateral_payload.get("quantity", 0)),
            metadata=_normalize_metadata(collateral_meta),
        )
        governance = RepoGovernance(
            haircut_bps=_coerce_int_field(governance_payload.get("haircut_bps"), "repo governance.haircut_bps"),
            margin_frequency_secs=_coerce_int_field(
                governance_payload.get("margin_frequency_secs"), "repo governance.margin_frequency_secs", allow_zero=True
            ),
        )
        custodian = payload.get("custodian")
        if custodian is not None:
            custodian = _require_non_empty_string(custodian, "repo custodian")

        initiated_timestamp_ms = _coerce_int_field(
            payload.get("initiated_timestamp_ms"),
            "repo initiated_timestamp_ms",
            allow_zero=True,
        )
        maturity_timestamp_ms = _coerce_int_field(
            payload.get("maturity_timestamp_ms"),
            "repo maturity_timestamp_ms",
            allow_zero=True,
        )
        last_margin_literal = payload.get("last_margin_check_timestamp_ms")
        if last_margin_literal is None:
            last_margin_check_timestamp_ms = initiated_timestamp_ms
        else:
            last_margin_check_timestamp_ms = _coerce_int_field(
                last_margin_literal,
                "repo last_margin_check_timestamp_ms",
                allow_zero=True,
            )
        if last_margin_check_timestamp_ms < initiated_timestamp_ms:
            last_margin_check_timestamp_ms = initiated_timestamp_ms
        return cls(
            agreement_id=_require_non_empty_string(require("id"), "repo agreement id"),
            initiator=_require_non_empty_string(require("initiator"), "repo agreement initiator"),
            counterparty=_require_non_empty_string(require("counterparty"), "repo agreement counterparty"),
            custodian=custodian,
            cash_leg=cash_leg,
            collateral_leg=collateral_leg,
            rate_bps=_coerce_int_field(require("rate_bps"), "repo rate_bps", allow_zero=True),
            maturity_timestamp_ms=maturity_timestamp_ms,
            initiated_timestamp_ms=initiated_timestamp_ms,
            last_margin_check_timestamp_ms=last_margin_check_timestamp_ms,
            governance=governance,
        )

    def next_margin_check_after(self, after_timestamp_ms: int) -> Optional[int]:
        """Compute the next margin checkpoint strictly after the provided timestamp."""

        frequency_ms = self.governance.margin_frequency_millis()
        if frequency_ms is None:
            return None
        start = int(self.last_margin_check_timestamp_ms)
        if after_timestamp_ms < start:
            return start
        elapsed = after_timestamp_ms - start
        periods_elapsed = elapsed // frequency_ms
        next_period = periods_elapsed + 1
        return start + frequency_ms * next_period

    def is_margin_check_due(self, at_timestamp_ms: int) -> bool:
        """Return ``True`` when a margin call is due at the provided timestamp."""

        next_due = self.next_margin_check_after(int(self.last_margin_check_timestamp_ms))
        if next_due is None:
            return False
        return at_timestamp_ms >= next_due

    def to_payload(self) -> Mapping[str, Any]:
        """Render a JSON-serialisable view of the agreement."""

        payload = {
            "id": self.agreement_id,
            "initiator": self.initiator,
            "counterparty": self.counterparty,
            "custodian": self.custodian,
            "cash_leg": self.cash_leg.to_payload(),
            "collateral_leg": self.collateral_leg.to_payload(),
            "rate_bps": int(self.rate_bps),
            "maturity_timestamp_ms": int(self.maturity_timestamp_ms),
            "initiated_timestamp_ms": int(self.initiated_timestamp_ms),
            "last_margin_check_timestamp_ms": int(self.last_margin_check_timestamp_ms),
            "governance": self.governance.to_payload(),
        }
        return payload


@dataclass(frozen=True)
class RepoAgreementListPage:
    """Paginated repo agreement listing."""

    items: list[RepoAgreementRecord]
    total: Optional[int]
    has_more: bool = False
    count_mode: str = "exact"
    indexed_height: Optional[int] = None
    indexed_block_hash: Optional[str] = None
    query_source: Optional[str] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RepoAgreementListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("repo agreement page payload must be an object")
        items_literal = payload.get("items", [])
        if not isinstance(items_literal, list):
            raise TypeError("repo agreement page `items` must be a list")
        items = [
            RepoAgreementRecord.from_payload(entry) for entry in items_literal if isinstance(entry, Mapping)
        ]
        if len(items) != len(items_literal):
            raise TypeError("repo agreement page `items` entries must be objects")
        return cls(items=items, **_page_metadata(payload, len(items)))


def _require_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed:
        raise ValueError(f"{context} must be non-empty")
    return trimmed


def _coerce_int_field(value: Any, context: str, *, allow_zero: bool = False) -> int:
    try:
        number = int(value)
    except (TypeError, ValueError) as exc:
        raise TypeError(f"{context} must be numeric") from exc
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{context} must be positive")
    return number
