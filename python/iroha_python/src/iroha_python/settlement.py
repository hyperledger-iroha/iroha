"""Settlement helpers for delivery-versus-payment (DvP) and payment-versus-payment (PvP) flows."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import TYPE_CHECKING, Any, Dict, Mapping, Optional

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .tx import NumericLike
else:  # pragma: no cover - alias for runtime to avoid circular import
    NumericLike = Any


def _normalize_quantity(value: Any) -> str:
    from .tx import _normalize_quantity as _tx_normalize_quantity

    return _tx_normalize_quantity(value)


def _normalize_metadata(metadata: Optional[Mapping[str, Any]]) -> Optional[Mapping[str, Any]]:
    from .tx import _normalize_metadata as _tx_normalize_metadata

    return _tx_normalize_metadata(metadata)

__all__ = [
    "SettlementLeg",
    "SettlementPlan",
    "SettlementExecutionOrder",
    "SettlementAtomicity",
]


class SettlementExecutionOrder(str, Enum):
    """Execution ordering between the two settlement legs."""

    DELIVERY_THEN_PAYMENT = "delivery_then_payment"
    PAYMENT_THEN_DELIVERY = "payment_then_delivery"


class SettlementAtomicity(str, Enum):
    """Atomicity policy controlling how partial failures are handled."""

    ALL_OR_NOTHING = "all_or_nothing"
    COMMIT_FIRST_LEG = "commit_first_leg"
    COMMIT_SECOND_LEG = "commit_second_leg"


@dataclass(frozen=True)
class SettlementPlan:
    """Plan describing execution ordering and atomicity semantics."""

    order: SettlementExecutionOrder = SettlementExecutionOrder.DELIVERY_THEN_PAYMENT
    atomicity: SettlementAtomicity = SettlementAtomicity.ALL_OR_NOTHING

    def to_payload(self) -> Mapping[str, Any]:
        return {
            "order": self.order.value,
            "atomicity": self.atomicity.value,
        }


@dataclass(frozen=True)
class SettlementLeg:
    """One leg of a bilateral settlement."""

    asset_definition_id: str
    quantity: NumericLike
    from_account: str
    to_account: str
    metadata: Optional[Mapping[str, Any]] = None

    def to_payload(self) -> Mapping[str, Any]:
        payload: Dict[str, Any] = {
            "asset_definition_id": self.asset_definition_id,
            "quantity": _normalize_quantity(self.quantity),
            "from": self.from_account,
            "to": self.to_account,
        }
        if self.metadata:
            payload["metadata"] = _normalize_metadata(self.metadata)
        return payload
