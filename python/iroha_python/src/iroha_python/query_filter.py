"""Lightweight Norito filter builder for Torii JSON envelopes."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Iterable, Mapping, Sequence

__all__ = [
    "FilterExpr",
    "Eq",
    "Ne",
    "Gt",
    "Lt",
    "Ge",
    "Le",
    "Between",
    "In",
    "Nin",
    "Exists",
    "IsNull",
    "And",
    "Or",
    "Not",
    "metadata_eq",
    "metadata_exists",
    "field_in",
    "field_not_in",
    "ensure_filter",
]


class FilterExpr:
    """Base class for filter expressions."""

    def to_dict(self) -> Dict[str, Any]:
        raise NotImplementedError


@dataclass
class Eq(FilterExpr):
    field: str
    value: Any

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "eq", "args": [self.field, self.value]}


@dataclass
class Ne(FilterExpr):
    field: str
    value: Any

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "ne", "args": [self.field, self.value]}


@dataclass
class Gt(FilterExpr):
    field: str
    value: Any

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "gt", "args": [self.field, self.value]}


@dataclass
class Lt(FilterExpr):
    field: str
    value: Any

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "lt", "args": [self.field, self.value]}


@dataclass
class Ge(FilterExpr):
    field: str
    value: Any

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "gte", "args": [self.field, self.value]}


@dataclass
class Le(FilterExpr):
    field: str

    value: Any

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "lte", "args": [self.field, self.value]}


@dataclass
class Between(FilterExpr):
    field: str
    low: Any
    high: Any

    def to_dict(self) -> Dict[str, Any]:
        return {
            "op": "and",
            "args": [
                {"op": "gte", "args": [self.field, self.low]},
                {"op": "lte", "args": [self.field, self.high]},
            ],
        }


@dataclass
class In(FilterExpr):
    field: str
    values: Sequence[Any]

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "in", "args": [self.field, list(self.values)]}


@dataclass
class Nin(FilterExpr):
    field: str
    values: Sequence[Any]

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "nin", "args": [self.field, list(self.values)]}


@dataclass
class Exists(FilterExpr):
    field: str

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "exists", "args": [self.field]}


@dataclass
class IsNull(FilterExpr):
    field: str

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "is_null", "args": [self.field]}


@dataclass
class And(FilterExpr):
    operands: Iterable[FilterExpr]

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "and", "args": [expr.to_dict() for expr in self.operands]}


@dataclass
class Or(FilterExpr):
    operands: Iterable[FilterExpr]

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "or", "args": [expr.to_dict() for expr in self.operands]}


@dataclass
class Not(FilterExpr):
    operand: FilterExpr

    def to_dict(self) -> Dict[str, Any]:
        return {"op": "not", "args": [self.operand.to_dict()]}


def ensure_filter(value: FilterExpr | Mapping[str, Any] | None) -> Mapping[str, Any] | None:
    if value is None:
        return None
    if isinstance(value, FilterExpr):
        return value.to_dict()
    return value


def metadata_eq(key: str, value: Any) -> Eq:
    """Convenience helper to match ``metadata.<key>`` against ``value``."""

    if not key:
        raise ValueError("metadata key must be non-empty")
    return Eq(f"metadata.{key}", value)


def metadata_exists(key: str) -> Exists:
    """Return an ``exists`` predicate for ``metadata.<key>``."""

    if not key:
        raise ValueError("metadata key must be non-empty")
    return Exists(f"metadata.{key}")


def field_in(field: str, values: Sequence[Any]) -> In:
    if not field:
        raise ValueError("field must be non-empty")
    return In(field, values)


def field_not_in(field: str, values: Sequence[Any]) -> Nin:
    if not field:
        raise ValueError("field must be non-empty")
    return Nin(field, values)
