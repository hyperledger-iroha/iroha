"""Exact standalone-election tally model and bounded Torii response owner."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any, Mapping, Optional, Tuple

import requests

from ._strict_json_response import (
    decode_exact_json_bytes,
    expect_status_without_body,
    read_bounded_identity_response,
)

__all__ = ["ElectionTally"]

ELECTION_TALLY_RESPONSE_MAX_BYTES = 8 * 1024
_FIELDS = frozenset(
    {"evaluated_block_height", "evaluated_block_hash", "finalized", "tally"}
)


def _require_uint(value: Any, bits: int, context: str) -> int:
    if type(value) is not int:
        raise TypeError(f"{context} must be an unquoted u{bits} integer")
    if not 0 <= value < 1 << bits:
        raise ValueError(f"{context} must fit in a u{bits}")
    return value


@dataclass(frozen=True)
class ElectionTally:
    """Exact public weights and the committed block that supplied them."""

    evaluated_block_height: int
    evaluated_block_hash: str
    finalized: bool
    tally: Tuple[int, ...]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ElectionTally":
        """Reject any response outside the one four-field election V1 layout."""

        if not isinstance(payload, Mapping):
            raise TypeError("election tally payload must be a mapping")
        unknown = set(payload) - _FIELDS
        if unknown:
            raise ValueError("election tally payload contains unknown fields")
        missing = _FIELDS - set(payload)
        if missing:
            raise TypeError(
                f"election tally payload is missing required `{sorted(missing)[0]}` field"
            )
        height = _require_uint(
            payload["evaluated_block_height"],
            64,
            "election tally.evaluated_block_height",
        )
        block_hash = payload["evaluated_block_hash"]
        if type(block_hash) is not str or re.fullmatch(r"[0-9a-f]{64}", block_hash) is None:
            raise ValueError("election tally.evaluated_block_hash must be a lowercase 32-byte hash")
        if (height == 0) != (block_hash == "0" * 64):
            raise ValueError("election tally evaluated block height and hash are inconsistent")
        finalized = payload["finalized"]
        if type(finalized) is not bool:
            raise TypeError("election tally.finalized must be a boolean")
        weights = payload["tally"]
        if type(weights) is not list or not 2 <= len(weights) <= 64:
            raise ValueError("election tally.tally must contain 2-64 weights")
        tally = tuple(
            _require_uint(weight, 128, f"election tally.tally[{index}]")
            for index, weight in enumerate(weights)
        )
        if sum(tally) >= 1 << 128:
            raise ValueError("election tally aggregate must fit in a u128")
        return cls(
            evaluated_block_height=height,
            evaluated_block_hash=block_hash,
            finalized=finalized,
            tally=tally,
        )


def read_election_tally_response(response: requests.Response) -> Optional[ElectionTally]:
    """Consume one identity-encoded bounded response without reparsing rounded JSON."""

    context = "election tally"
    expect_status_without_body(response, (200, 404), context)
    if response.status_code == 404:
        response.close()
        return None
    body = read_bounded_identity_response(
        response,
        ELECTION_TALLY_RESPONSE_MAX_BYTES,
        context,
        expected_content_type="application/json",
    )
    payload = decode_exact_json_bytes(
        body, context, maximum_bytes=ELECTION_TALLY_RESPONSE_MAX_BYTES
    )
    return ElectionTally.from_payload(payload)
