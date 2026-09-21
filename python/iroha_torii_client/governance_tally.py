"""One exact standalone tally model and response owner for both Python clients."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any, Mapping, Optional

import requests

from ._strict_json_response import (
    decode_exact_json_bytes,
    expect_status_without_body,
    read_bounded_identity_response,
)

__all__ = ["GovernanceTally"]

# The six-field response has at most a 128-byte ASCII selector, a u64 height,
# a 64-byte hex hash and three u128 integers, well below this actual-byte cap.
GOVERNANCE_TALLY_RESPONSE_MAX_BYTES = 4 * 1024
_FIELDS = frozenset(
    {
        "referendum_id",
        "evaluated_block_height",
        "evaluated_block_hash",
        "approve",
        "reject",
        "abstain",
    }
)


def require_tally_selector(value: Any, context: str) -> str:
    """Require the exact Core referendum selector before a request or projection."""

    if type(value) is not str:
        raise TypeError(f"{context} must be a string")
    if re.fullmatch(r"[A-Za-z0-9_~-][A-Za-z0-9._~-]{0,127}", value) is None:
        raise ValueError(f"{context} must be a canonical governance selector V1")
    return value


def _require_uint(value: Any, bits: int, context: str) -> int:
    if type(value) is not int:
        raise TypeError(f"{context} must be an unquoted u{bits} integer")
    if not 0 <= value < 1 << bits:
        raise ValueError(f"{context} must fit in a u{bits}")
    return value


@dataclass(frozen=True)
class GovernanceTally:
    """Exact referendum tally and the committed block at which it was evaluated."""

    referendum_id: str
    evaluated_block_height: int
    evaluated_block_hash: str
    approve: int
    reject: int
    abstain: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceTally":
        """Reject missing fields, coercion, overflow and inconsistent anchors."""

        if not isinstance(payload, Mapping):
            raise TypeError("tally payload must be a mapping")
        unknown = set(payload) - _FIELDS
        if unknown:
            raise ValueError("tally payload contains unknown fields")
        missing = _FIELDS - set(payload)
        if missing:
            raise TypeError(f"tally payload is missing required `{sorted(missing)[0]}` field")
        referendum_id = require_tally_selector(
            payload["referendum_id"], "tally payload.referendum_id"
        )
        evaluated_block_height = _require_uint(
            payload["evaluated_block_height"], 64, "tally payload.evaluated_block_height"
        )
        evaluated_block_hash = payload["evaluated_block_hash"]
        if type(evaluated_block_hash) is not str:
            raise TypeError("tally payload.evaluated_block_hash must be a lowercase 32-byte hash")
        if re.fullmatch(r"[0-9a-f]{64}", evaluated_block_hash) is None:
            raise ValueError("tally payload.evaluated_block_hash must be a lowercase 32-byte hash")
        if (evaluated_block_height == 0) != (evaluated_block_hash == "0" * 64):
            raise ValueError("tally payload evaluated block height and hash are inconsistent")
        approve, reject, abstain = (
            _require_uint(payload[field], 128, f"tally payload.{field}")
            for field in ("approve", "reject", "abstain")
        )
        if approve + reject + abstain >= 1 << 128:
            raise ValueError("tally payload aggregate votes must fit in a u128")
        return cls(
            referendum_id=referendum_id,
            evaluated_block_height=evaluated_block_height,
            evaluated_block_hash=evaluated_block_hash,
            approve=approve,
            reject=reject,
            abstain=abstain,
        )


def read_tally_response(
    response: requests.Response, expected_referendum_id: str
) -> Optional[Mapping[str, Any]]:
    """Consume original response bytes once, with absence distinct from zero votes."""

    try:
        selector = require_tally_selector(expected_referendum_id, "referendum_id")
    except (TypeError, ValueError):
        response.close()
        raise
    context = "governance tally"
    expect_status_without_body(response, (200, 404), context)
    if response.status_code == 404:
        response.close()
        return None
    body = read_bounded_identity_response(
        response,
        GOVERNANCE_TALLY_RESPONSE_MAX_BYTES,
        context,
        expected_content_type="application/json",
    )
    # Apply the actual-byte limit before removing ordinary JSON whitespace.
    payload = decode_exact_json_bytes(
        body.strip(b" \t\r\n"), context, maximum_bytes=GOVERNANCE_TALLY_RESPONSE_MAX_BYTES
    )
    tally = GovernanceTally.from_payload(payload)
    if tally.referendum_id != selector:
        raise ValueError("governance tally referendum_id does not match the requested selector")
    return payload
