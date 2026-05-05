"""Helpers for building Torii JSON query envelopes."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, Iterable, Mapping, Optional

from .query_filter import FilterExpr, ensure_filter


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
    query_name: Optional[str] = None
    select: Optional[Iterable[Mapping[str, Any]]] = None

    def to_dict(self) -> Dict[str, Any]:
        payload: Dict[str, Any] = {
            "pagination": self.pagination.to_dict(),
            "sort": list(self.sort),
        }
        if self.filter is not None:
            payload["filter"] = dict(self.filter)
        if self.fetch_size is not None:
            payload["fetch_size"] = self.fetch_size
        if self.query_name is not None:
            payload["query"] = self.query_name
        if self.select is not None:
            payload["select"] = list(self.select)
        return payload


def account_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    query_name: Optional[str] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/accounts/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        query_name=query_name,
    )
    return envelope.to_dict()


def asset_definitions_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    query_name: Optional[str] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/assets/definitions/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        query_name=query_name,
    )
    return envelope.to_dict()


def domain_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    query_name: Optional[str] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/domains/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        query_name=query_name,
    )
    return envelope.to_dict()


def asset_holders_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    query_name: Optional[str] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/assets/{definition}/holders/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        query_name=query_name,
    )
    return envelope.to_dict()


def rwa_query_envelope(
    *,
    filter: Optional[FilterExpr | Mapping[str, Any]] = None,
    sort: Optional[Iterable[Mapping[str, Any]]] = None,
    limit: Optional[int] = None,
    offset: int = 0,
    fetch_size: Optional[int] = None,
    query_name: Optional[str] = None,
) -> Dict[str, Any]:
    """Build an envelope for POST `/v1/rwas/query`."""

    envelope = QueryEnvelope(
        filter=ensure_filter(filter),
        sort=list(sort) if sort is not None else [],
        pagination=Pagination(limit=limit, offset=offset),
        fetch_size=fetch_size,
        query_name=query_name,
    )
    return envelope.to_dict()
