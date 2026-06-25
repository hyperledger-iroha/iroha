"""Helpers for building Torii JSON query envelopes."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, Iterable, Mapping, Optional, Union

from .query_filter import FilterExpr, ensure_filter

SelectEntry = Union[str, Mapping[str, Any]]


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
        if select is not None:
            payload["select"] = select
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
    )
    return envelope.to_dict()
