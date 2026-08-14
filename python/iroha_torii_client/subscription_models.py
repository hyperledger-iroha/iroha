"""Secret-free subscription mutation draft response models."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, List, Mapping, Optional


def _record(payload: Mapping[str, Any], context: str) -> Mapping[str, Any]:
    if not isinstance(payload, Mapping):
        raise RuntimeError(f"{context} must be an object")
    return payload


def _string(record: Mapping[str, Any], field: str, context: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value or value.strip() != value:
        raise RuntimeError(f"{context}.{field} must be an exact non-empty string")
    return value


def _uint(record: Mapping[str, Any], field: str, context: str) -> int:
    value = record.get(field)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise RuntimeError(f"{context}.{field} must be a non-negative integer")
    return value


def _instructions(record: Mapping[str, Any], context: str) -> List[Dict[str, str]]:
    value = record.get("tx_instructions")
    if not isinstance(value, list) or not value:
        raise RuntimeError(f"{context}.tx_instructions must be a non-empty list")
    instructions: List[Dict[str, str]] = []
    for index, raw in enumerate(value):
        item_context = f"{context}.tx_instructions[{index}]"
        item = _record(raw, item_context)
        if set(item) != {"wire_id", "payload_hex"}:
            raise RuntimeError(f"{item_context} must use the exact draft fields")
        wire_id = _string(item, "wire_id", item_context)
        payload_hex = _string(item, "payload_hex", item_context)
        if (
            payload_hex.lower() != payload_hex
            or len(payload_hex) % 2 != 0
            or any(character not in "0123456789abcdef" for character in payload_hex)
        ):
            raise RuntimeError(f"{item_context}.payload_hex must be lowercase hexadecimal")
        instructions.append({"wire_id": wire_id, "payload_hex": payload_hex})
    return instructions


@dataclass(frozen=True)
class SubscriptionCreateResult:
    """Exact unsigned subscription creation draft."""

    version: int
    authority: str
    action: str
    subscription_id: str
    plan_id: str
    billing_trigger_id: str
    usage_trigger_id: Optional[str]
    first_charge_ms: int
    provider_usage_grant_included: bool
    resulting_subscription: Dict[str, Any]
    tx_instructions: List[Dict[str, str]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionCreateResult":
        context = "subscription create response"
        record = _record(payload, context)
        action = _string(record, "action", context)
        if action != "create" or _uint(record, "version", context) != 1:
            raise RuntimeError(f"{context} must use the V1 create layout")
        usage = record.get("usage_trigger_id")
        if usage is not None and (not isinstance(usage, str) or not usage):
            raise RuntimeError(f"{context}.usage_trigger_id must be null or non-empty")
        grant = record.get("provider_usage_grant_included")
        state = record.get("resulting_subscription")
        if not isinstance(grant, bool) or not isinstance(state, Mapping):
            raise RuntimeError(f"{context} has invalid projected state fields")
        return cls(
            version=1,
            authority=_string(record, "authority", context),
            action=action,
            subscription_id=_string(record, "subscription_id", context),
            plan_id=_string(record, "plan_id", context),
            billing_trigger_id=_string(record, "billing_trigger_id", context),
            usage_trigger_id=usage,
            first_charge_ms=_uint(record, "first_charge_ms", context),
            provider_usage_grant_included=grant,
            resulting_subscription=dict(state),
            tx_instructions=_instructions(record, context),
        )


@dataclass(frozen=True)
class SubscriptionActionResult:
    """Exact unsigned pause/resume/cancel/keep/charge-now draft."""

    version: int
    authority: str
    action: str
    subscription_id: str
    details: Dict[str, Any]
    tx_instructions: List[Dict[str, str]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SubscriptionActionResult":
        context = "subscription action response"
        record = _record(payload, context)
        action = _string(record, "action", context)
        if action not in {"pause", "resume", "cancel", "keep", "charge_now"}:
            raise RuntimeError(f"{context}.action is unsupported")
        if _uint(record, "version", context) != 1:
            raise RuntimeError(f"{context}.version must be 1")
        details = record.get("details")
        if not isinstance(details, Mapping):
            raise RuntimeError(f"{context}.details must be an object")
        return cls(
            version=1,
            authority=_string(record, "authority", context),
            action=action,
            subscription_id=_string(record, "subscription_id", context),
            details=dict(details),
            tx_instructions=_instructions(record, context),
        )
