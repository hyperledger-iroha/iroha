"""Helpers for building Torii JSON query envelopes."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, Iterable, Mapping, Optional, Union

from .query_filter import FilterExpr, ensure_filter

SelectEntry = Union[str, Mapping[str, Any]]
AggregateEntry = Union["AggregateMetric", Mapping[str, Any]]

_AGGREGATE_ALIAS_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def _normalize_count_mode(count_mode: Optional[str]) -> Optional[str]:
    if count_mode is None:
        return None
    value = str(count_mode).strip().lower()
    if value not in {"bounded", "exact"}:
        raise ValueError("count_mode must be 'bounded' or 'exact'")
    return value


def _normalize_query_name(query_name: Optional[str]) -> Optional[str]:
    if query_name is None:
        return None
    if not isinstance(query_name, str):
        raise TypeError("query_name must be a string")
    value = query_name.strip()
    if not value:
        raise ValueError("query_name must be a non-empty string")
    return value


def _normalize_select(
    select: Optional[Iterable[SelectEntry]],
) -> Optional[list[Union[str, Dict[str, Any]]]]:
    if select is None:
        return None
    if isinstance(select, (str, bytes, bytearray)):
        raise TypeError("select must be a sequence of field paths or objects")
    normalized: list[Union[str, Dict[str, Any]]] = []
    for index, entry in enumerate(select):
        if isinstance(entry, str):
            field_path = entry.strip()
            if not field_path:
                raise ValueError(f"select[{index}] must be a non-empty field path")
            normalized.append(field_path)
        elif isinstance(entry, Mapping):
            normalized.append(dict(entry))
        else:
            raise TypeError(f"select[{index}] must be a field-path string or mapping")
    return normalized


def _normalize_field_path(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    normalized = value.strip()
    if not normalized:
        raise ValueError(f"{context} must be a non-empty field path")
    return normalized


class AggregateFn(str, Enum):
    """Aggregate functions accepted by Torii's generic query engine."""

    COUNT = "count"
    SUM = "sum"
    MIN = "min"
    MAX = "max"
    AVG = "avg"
    DISTINCT_COUNT = "distinct_count"


def _normalize_aggregate_fn(value: AggregateFn | str) -> AggregateFn:
    if isinstance(value, AggregateFn):
        return value
    if not isinstance(value, str):
        raise TypeError("aggregate metric fn must be an AggregateFn or string")
    try:
        return AggregateFn(value.strip().lower())
    except ValueError as error:
        supported = ", ".join(item.value for item in AggregateFn)
        raise ValueError(f"aggregate metric fn must be one of: {supported}") from error


@dataclass(frozen=True)
class AggregateMetric:
    """One named aggregate value returned for each group."""

    alias: str
    fn: AggregateFn | str
    field: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        if not isinstance(self.alias, str):
            raise TypeError("aggregate metric alias must be a string")
        alias = self.alias.strip()
        if not _AGGREGATE_ALIAS_RE.fullmatch(alias):
            raise ValueError(
                "aggregate metric alias must begin with a letter or underscore "
                "and contain only letters, digits, or underscores"
            )
        function = _normalize_aggregate_fn(self.fn)
        field_path = (
            _normalize_field_path(self.field, f"aggregate metric `{alias}` field")
            if self.field is not None
            else None
        )
        if function is AggregateFn.COUNT and field_path is not None:
            raise ValueError("count must not declare a field")
        if function is not AggregateFn.COUNT and field_path is None:
            raise ValueError(f"{function.value} requires a field")
        payload: Dict[str, Any] = {"alias": alias, "fn": function.value}
        if field_path is not None:
            payload["field"] = field_path
        return payload


@dataclass(frozen=True)
class AggregateSpec:
    """Grouping, metrics, and optional post-aggregation filtering."""

    group_by: Iterable[str] = field(default_factory=tuple)
    metrics: Iterable[AggregateEntry] = field(default_factory=tuple)
    having: Optional[FilterExpr | Mapping[str, Any]] = None

    def to_dict(self) -> Dict[str, Any]:
        if isinstance(self.group_by, (str, bytes, bytearray)):
            raise TypeError("aggregate group_by must be a sequence of field paths")
        group_by = [
            _normalize_field_path(value, f"aggregate group_by[{index}]")
            for index, value in enumerate(self.group_by)
        ]
        if len(group_by) > 4:
            raise ValueError("aggregate group_by supports at most four fields")
        if len(set(group_by)) != len(group_by):
            raise ValueError("aggregate group_by fields must be unique")

        if isinstance(self.metrics, (str, bytes, bytearray, Mapping)):
            raise TypeError("aggregate metrics must be a sequence")
        metrics: list[Dict[str, Any]] = []
        for index, metric in enumerate(self.metrics):
            if isinstance(metric, AggregateMetric):
                normalized = metric.to_dict()
            elif isinstance(metric, Mapping):
                unknown = set(metric).difference({"alias", "fn", "field"})
                if unknown:
                    names = ", ".join(sorted(str(name) for name in unknown))
                    raise ValueError(f"aggregate metrics[{index}] contains unknown fields: {names}")
                try:
                    normalized = AggregateMetric(
                        alias=metric["alias"],
                        fn=metric["fn"],
                        field=metric.get("field"),
                    ).to_dict()
                except KeyError as error:
                    raise ValueError(
                        f"aggregate metrics[{index}] is missing `{error.args[0]}`"
                    ) from error
            else:
                raise TypeError(f"aggregate metrics[{index}] must be an AggregateMetric or mapping")
            metrics.append(normalized)
        if not metrics:
            raise ValueError("aggregate metrics must not be empty")
        if len(metrics) > 8:
            raise ValueError("aggregate metrics supports at most eight metrics")

        aliases = [metric["alias"] for metric in metrics]
        if len(set(aliases)) != len(aliases):
            raise ValueError("aggregate metric aliases must be unique")
        collisions = set(group_by).intersection(aliases)
        if collisions:
            raise ValueError(
                "aggregate output fields must be unique; metric alias conflicts "
                f"with group_by field `{sorted(collisions)[0]}`"
            )

        payload: Dict[str, Any] = {"group_by": group_by, "metrics": metrics}
        having = ensure_filter(self.having)
        if having is not None:
            if not isinstance(having, Mapping):
                raise TypeError("aggregate having must be a FilterExpr or mapping")
            payload["having"] = dict(having)
        return payload


def ensure_aggregate(
    value: AggregateSpec | Mapping[str, Any] | None,
) -> Optional[Dict[str, Any]]:
    """Normalize a typed or mapping aggregate specification."""

    if value is None:
        return None
    if isinstance(value, AggregateSpec):
        return value.to_dict()
    if not isinstance(value, Mapping):
        raise TypeError("aggregate must be an AggregateSpec or mapping")
    unknown = set(value).difference({"group_by", "metrics", "having"})
    if unknown:
        names = ", ".join(sorted(str(name) for name in unknown))
        raise ValueError(f"aggregate contains unknown fields: {names}")
    return AggregateSpec(
        group_by=value.get("group_by", ()),
        metrics=value.get("metrics", ()),
        having=value.get("having"),
    ).to_dict()


@dataclass
class Pagination:
    """Pagination controls for streaming endpoints."""

    limit: Optional[int] = None
    offset: int = 0

    def to_dict(self) -> Dict[str, Any]:
        payload: Dict[str, Any] = {"offset": self.offset}
        if self.limit is not None:
            payload["limit"] = self.limit
        return payload


@dataclass
class QueryEnvelope:
    """JSON-friendly representation of Torii `QueryEnvelope`."""

    filter: Optional[Mapping[str, Any]] = None
    sort: Iterable[Mapping[str, Any]] = field(default_factory=list)
    pagination: Pagination = field(default_factory=Pagination)
    fetch_size: Optional[int] = None
    count_mode: Optional[str] = None
    query_name: Optional[str] = None
    select: Optional[Iterable[SelectEntry]] = None
    aggregate: Optional[AggregateSpec | Mapping[str, Any]] = None

    def to_dict(self) -> Dict[str, Any]:
        payload: Dict[str, Any] = {
            "pagination": self.pagination.to_dict(),
            "sort": list(self.sort),
        }
        if self.filter is not None:
            payload["filter"] = dict(self.filter)
        if self.fetch_size is not None:
            payload["fetch_size"] = self.fetch_size
        count_mode = _normalize_count_mode(self.count_mode)
        if count_mode is not None:
            payload["count_mode"] = count_mode
        query_name = _normalize_query_name(self.query_name)
        if query_name is not None:
            payload["query"] = query_name
        select = _normalize_select(self.select)
        aggregate = ensure_aggregate(self.aggregate)
        if select is not None and aggregate is not None:
            raise ValueError("select and aggregate are mutually exclusive")
        if select is not None:
            payload["select"] = select
        if aggregate is not None:
            payload["aggregate"] = aggregate
        return payload


def account_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    count_mode: Optional[str] = None,
    query_name: Optional[str] = None,
    select: Optional[Iterable[SelectEntry]] = None,
    aggregate: Optional[AggregateSpec | Mapping[str, Any]] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/accounts/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        count_mode=count_mode,
        query_name=query_name,
        select=select,
        aggregate=aggregate,
    )
    return envelope.to_dict()


def asset_definitions_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    count_mode: Optional[str] = None,
    query_name: Optional[str] = None,
    select: Optional[Iterable[SelectEntry]] = None,
    aggregate: Optional[AggregateSpec | Mapping[str, Any]] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/assets/definitions/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        count_mode=count_mode,
        query_name=query_name,
        select=select,
        aggregate=aggregate,
    )
    return envelope.to_dict()


def domain_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    count_mode: Optional[str] = None,
    query_name: Optional[str] = None,
    select: Optional[Iterable[SelectEntry]] = None,
    aggregate: Optional[AggregateSpec | Mapping[str, Any]] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/domains/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        count_mode=count_mode,
        query_name=query_name,
        select=select,
        aggregate=aggregate,
    )
    return envelope.to_dict()


def asset_holders_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    count_mode: Optional[str] = None,
    query_name: Optional[str] = None,
    select: Optional[Iterable[SelectEntry]] = None,
    aggregate: Optional[AggregateSpec | Mapping[str, Any]] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/assets/{definition}/holders/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        count_mode=count_mode,
        query_name=query_name,
        select=select,
        aggregate=aggregate,
    )
    return envelope.to_dict()


def rwa_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    count_mode: Optional[str] = None,
    query_name: Optional[str] = None,
    select: Optional[Iterable[SelectEntry]] = None,
    aggregate: Optional[AggregateSpec | Mapping[str, Any]] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/rwas/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        count_mode=count_mode,
        query_name=query_name,
        select=select,
        aggregate=aggregate,
    )
    return envelope.to_dict()
