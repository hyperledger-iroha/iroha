"""Client helpers for interacting with Iroha Torii endpoints."""

from __future__ import annotations

import base64
import binascii
import json
import logging
import math
import os
import re
import time
from dataclasses import dataclass, field, asdict
from enum import Enum
from types import ModuleType
from typing import (
    Any,
    Callable,
    Dict,
    Iterable,
    Iterator,
    List,
    Mapping,
    Optional,
    Sequence,
    Tuple,
    TYPE_CHECKING,
    Union,
)

from urllib.parse import urlencode, urlparse, urlunparse, quote

import requests

from iroha_torii_client.client import (
    ToriiClient as _BaseToriiClient,
    ConfigurationSnapshot,
    ConfidentialGasSchedule,
    NetworkTimeSnapshot,
    NetworkTimeStatus,
    NetworkTimeSample,
    NetworkTimeRttBucket,
    OfflineAllowanceDeadline,
    OfflineAllowanceListItem,
    OfflineAllowanceListPage,
    OfflineTransferHistoryEntry,
    OfflineTransferListItem,
    OfflineTransferListPage,
    OfflineSummaryListItem,
    OfflineSummaryListPage,
)
from .repo import RepoAgreementListPage

from .query import (
    account_query_envelope,
    asset_definitions_query_envelope,
    asset_holders_query_envelope,
    domain_query_envelope,
)
from .event_filter import DataEventFilter, ensure_event_filter
from .connect import ConnectSessionInfo
from .sorafs import (
    SorafsAliasPolicy,
    SorafsAliasEvaluation,
    SorafsAliasWarning,
    SorafsAliasError,
    enforce_alias_policy as enforce_sorafs_alias_policy,
)

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .connect import _ConnectControlBase as ConnectControlBase  # noqa: F401
    from .crypto import Instruction, SignedTransactionEnvelope  # noqa: F401
    from .tx import TransactionDraft
else:  # pragma: no cover - runtime fallback when extension is absent
    Instruction = Any  # type: ignore[assignment]
    SignedTransactionEnvelope = Any  # type: ignore[assignment]
    ConnectControlBase = Any  # type: ignore[assignment]
    TransactionDraft = Any  # type: ignore[assignment]


def _json_safe_value(value: Any) -> Any:
    if isinstance(value, (bytes, bytearray, memoryview)):
        return base64.b64encode(bytes(value)).decode("ascii")
    if isinstance(value, Mapping):
        return {key: _json_safe_value(val) for key, val in value.items()}
    if isinstance(value, list):
        return [_json_safe_value(item) for item in value]
    if isinstance(value, tuple):
        return [_json_safe_value(item) for item in value]
    return value


def _encode_filter_arg(filter_value: Optional[Any]) -> Optional[str]:
    if filter_value is None:
        return None
    if isinstance(filter_value, str):
        return filter_value
    return json.dumps(filter_value)


def _encode_sort_arg(sort_value: Optional[Any]) -> Optional[str]:
    if sort_value is None:
        return None
    if isinstance(sort_value, str):
        return sort_value
    if isinstance(sort_value, Sequence):
        return ",".join(str(entry).strip() for entry in sort_value if entry is not None)
    return str(sort_value)


def _reject_alias_keys(source: Mapping[str, Any],
                       aliases: Mapping[str, str],
                       *,
                       context: str) -> None:
    for alias_key, canonical_key in aliases.items():
        if alias_key in source:
            raise TypeError(
                f"{context} does not accept {alias_key}; use {canonical_key}"
            )


_ISO_STATUS_VALUES = {
    "pending": "Pending",
    "accepted": "Accepted",
    "committed": "Committed",
    "rejected": "Rejected",
}
_ISO_NON_TERMINAL_STATUSES = frozenset({"Pending", "Accepted"})
_PACS002_STATUS_CODES = frozenset({"ACTC", "ACSP", "ACSC", "ACWC", "PDNG", "RJCT"})
_DEFAULT_ISO_POLL_INTERVAL_SECONDS = 2.0
_DEFAULT_ISO_WAIT_ATTEMPTS = 12
_MIN_ISO_POLL_INTERVAL_SECONDS = 0.01


def _normalize_address_format(value: Optional[str]) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError("address_format must be a string when provided")
    trimmed = value.strip().lower()
    if not trimmed:
        return None
    if trimmed in ("ih58", "compressed"):
        return trimmed
    allowed = ", ".join(["ih58", "compressed"])
    raise ValueError(f"address_format must be one of {allowed}")


def _apply_address_format_alias(params: Dict[str, Any], key: str = "address_format") -> None:
    """Normalize ``address_format`` values in parameter dictionaries."""

    if key not in params:
        return
    normalized = _normalize_address_format(params.pop(key))
    if normalized is not None:
        params[key] = normalized


def _require_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed:
        raise ValueError(f"{context} must be a non-empty string")
    return trimmed


def _normalize_optional_int_field(value: Any, context: str) -> Optional[int]:
    parsed = _coerce_int(value, context, allow_zero=True)
    return parsed if parsed is not None else None


def _normalize_optional_string(value: Any, context: str) -> Optional[str]:
    if value is None:
        return None
    return _require_non_empty_string(value, context)


def _normalize_string_list(value: Any, context: str) -> List[str]:
    if not isinstance(value, (list, tuple)):
        raise TypeError(f"{context} must be an array of strings")
    normalized: List[str] = []
    for index, item in enumerate(value):
        if not isinstance(item, str):
            raise TypeError(f"{context}[{index}] must be a string")
        normalized.append(item)
    return normalized


def _normalize_iso_optional_string(
    value: Any,
    context: str,
    *,
    allow_empty: bool = False,
) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed and not allow_empty:
        return None
    return trimmed


def _normalize_iso_string_array(value: Any, context: str) -> Tuple[str, ...]:
    if value is None:
        return ()
    if not isinstance(value, Sequence):
        raise TypeError(f"{context} must be an array of strings")
    entries: List[str] = []
    for index, entry in enumerate(value):
        if not isinstance(entry, str):
            raise TypeError(f"{context}[{index}] must be a string")
        trimmed = entry.strip()
        if not trimmed:
            raise ValueError(f"{context}[{index}] must be a non-empty string")
        entries.append(trimmed)
    return tuple(entries)


def _normalize_iso_status(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip().lower()
    if not trimmed:
        raise ValueError(f"{context} must be non-empty")
    normalized = _ISO_STATUS_VALUES.get(trimmed)
    if normalized is None:
        allowed = ", ".join(sorted(_ISO_STATUS_VALUES.values()))
        raise ValueError(f"{context} must be one of {allowed}")
    return normalized


def _normalize_pacs002_code(value: Any, context: str) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string or null")
    trimmed = value.strip().upper()
    if not trimmed:
        return None
    if trimmed not in _PACS002_STATUS_CODES:
        allowed = ", ".join(sorted(_PACS002_STATUS_CODES))
        raise ValueError(f"{context} must be one of {allowed}")
    return trimmed


def _normalize_iso_payload(message: Any, context: str) -> bytes:
    if isinstance(message, (bytes, bytearray, memoryview)):
        payload = bytes(message)
        if not payload:
            raise ValueError(f"{context} must be non-empty")
        return payload
    if isinstance(message, str):
        trimmed = message.strip()
        if not trimmed:
            raise ValueError(f"{context} must be a non-empty string")
        return trimmed.encode("utf-8")
    raise TypeError(f"{context} must be bytes or a UTF-8 string")


def _is_iso_status_terminal(
    status: Optional["IsoSubmissionRecord"],
    resolve_on_accepted: bool,
) -> bool:
    if status is None:
        return False
    if status.status in _ISO_NON_TERMINAL_STATUSES:
        return status.status == "Accepted" and resolve_on_accepted
    return True


def _normalize_iso_wait_kwargs(
    options: Optional[Mapping[str, Any]],
    *,
    context: str,
) -> Dict[str, Any]:
    if options is None:
        return {}
    if not isinstance(options, Mapping):
        raise TypeError(f"{context} must be a mapping when provided")
    allowed = {"poll_interval", "max_attempts", "resolve_on_accepted", "timeout", "on_poll"}
    extras = [key for key in options.keys() if key not in allowed]
    if extras:
        extras_str = ", ".join(sorted(extras))
        raise ValueError(f"{context} contains unsupported fields: {extras_str}")
    normalized: Dict[str, Any] = {}
    if "poll_interval" in options:
        normalized["poll_interval"] = options["poll_interval"]
    if "max_attempts" in options:
        normalized["max_attempts"] = options["max_attempts"]
    if "resolve_on_accepted" in options:
        normalized["resolve_on_accepted"] = options["resolve_on_accepted"]
    if "timeout" in options:
        normalized["timeout"] = options["timeout"]
    if "on_poll" in options:
        normalized["on_poll"] = options["on_poll"]
    return normalized


def _bytes_like_to_hex(value: Any, context: str) -> str:
    if isinstance(value, (bytes, bytearray, memoryview)):
        return bytes(value).hex()
    if isinstance(value, (list, tuple)):
        try:
            return bytes(value).hex()
        except (TypeError, ValueError) as exc:
            raise TypeError(f"{context} must contain byte values") from exc
    raise TypeError(f"{context} must be bytes-like")


def _normalize_hex_string(
    value: Any,
    context: str,
    *,
    expected_length: Optional[int] = None,
) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a hex string")
    trimmed = value.strip().lower()
    if not trimmed:
        raise ValueError(f"{context} must be a non-empty hex string")
    if expected_length is not None and len(trimmed) != expected_length:
        raise ValueError(f"{context} must contain {expected_length} hex characters")
    try:
        bytes.fromhex(trimmed)
    except ValueError as exc:
        raise ValueError(f"{context} must contain valid hexadecimal characters") from exc
    return trimmed


def _normalize_uaid_literal(value: Any, *, context: str = "uaid") -> str:
    """Normalise raw UAID inputs to the canonical ``uaid:<hex>`` form."""

    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed:
        raise ValueError(f"{context} must be a non-empty string")
    if trimmed.lower().startswith("uaid:"):
        _, hex_value = trimmed.split(":", 1)
    else:
        hex_value = trimmed
    normalized = _normalize_hex_string(
        hex_value,
        context,
        expected_length=64,
    )
    return f"uaid:{normalized}"


def _normalize_positive_int(value: Any, context: str, *, allow_zero: bool) -> int:
    try:
        integer = int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"{context} must be numeric") from exc
    if integer < 0 or (integer == 0 and not allow_zero):
        comparator = "non-negative" if allow_zero else "greater than zero"
        raise ValueError(f"{context} must be {comparator}")
    return integer


def _normalize_space_directory_manifest_payload(
    manifest: Any,
    *,
    context: str,
) -> Dict[str, Any]:
    if not isinstance(manifest, Mapping):
        raise TypeError(f"{context} must be an object")
    _reject_alias_keys(
        manifest,
        {
            "Version": "version",
            "uaid_literal": "uaid",
            "uaidLiteral": "uaid",
            "dataspace_id": "dataspace",
            "dataspaceId": "dataspace",
            "issuedMs": "issued_ms",
            "activationEpoch": "activation_epoch",
            "expiryEpoch": "expiry_epoch",
            "Entries": "entries",
        },
        context=context,
    )
    result: Dict[str, Any] = {}
    version_raw = manifest.get("version")
    result["version"] = _require_non_empty_string(version_raw, f"{context}.version")
    uaid_literal = manifest.get("uaid")
    result["uaid"] = _normalize_uaid_literal(uaid_literal, context=f"{context}.uaid")
    dataspace_raw = manifest.get("dataspace")
    if dataspace_raw is None:
        raise TypeError(f"{context}.dataspace is required")
    result["dataspace"] = _normalize_positive_int(
        dataspace_raw,
        f"{context}.dataspace",
        allow_zero=True,
    )
    issued_ms = _normalize_optional_int_field(
        manifest.get("issued_ms"),
        f"{context}.issued_ms",
    )
    if issued_ms is not None:
        result["issued_ms"] = issued_ms
    activation_epoch = _normalize_optional_int_field(
        manifest.get("activation_epoch"),
        f"{context}.activation_epoch",
    )
    if activation_epoch is not None:
        result["activation_epoch"] = activation_epoch
    expiry_epoch = _normalize_optional_int_field(
        manifest.get("expiry_epoch"),
        f"{context}.expiry_epoch",
    )
    if expiry_epoch is not None:
        result["expiry_epoch"] = expiry_epoch
    accounts = manifest.get("accounts")
    if accounts is not None:
        result["accounts"] = _normalize_string_list(accounts, f"{context}.accounts")
    entries_raw = manifest.get("entries")
    if not isinstance(entries_raw, Sequence) or not entries_raw:
        raise TypeError(f"{context}.entries must be a non-empty array")
    normalized_entries: List[Dict[str, Any]] = []
    for index, entry in enumerate(entries_raw):
        if not isinstance(entry, Mapping):
            raise TypeError(f"{context}.entries[{index}] must be an object")
        effect = entry.get("effect")
        if not isinstance(effect, Mapping):
            raise TypeError(f"{context}.entries[{index}].effect must be an object")
        normalized_entry: Dict[str, Any] = {"effect": _json_safe_value(effect)}
        scope = entry.get("scope")
        if scope is not None:
            if not isinstance(scope, Mapping):
                raise TypeError(f"{context}.entries[{index}].scope must be an object")
            normalized_entry["scope"] = _json_safe_value(scope)
        notes = entry.get("notes")
        if notes is not None:
            if not isinstance(notes, str):
                raise TypeError(f"{context}.entries[{index}].notes must be a string")
            normalized_entry["notes"] = notes
        normalized_entries.append(normalized_entry)
    result["entries"] = normalized_entries
    return result


def _normalize_authority_credentials(
    payload: Mapping[str, Any],
    *,
    context: str,
) -> Dict[str, str]:
    if not isinstance(payload, Mapping):
        raise TypeError(f"{context} must be an object")
    _reject_alias_keys(
        payload,
        {
            "account": "authority",
            "privateKey": "private_key",
            "privateKeyMultihash": "private_key_multihash",
            "privateKeyHex": "private_key_hex",
            "privateKeyBytes": "private_key_bytes",
            "privateKeySeed": "private_key_seed",
            "privateKeyAlgorithm": "private_key_algorithm",
        },
        context=context,
    )
    authority_raw = payload.get("authority")
    authority = _require_non_empty_string(authority_raw, f"{context}.authority")
    private_key_literal = payload.get("private_key")
    if private_key_literal is not None:
        private_key = _require_non_empty_string(private_key_literal, f"{context}.private_key")
    else:
        multihash = payload.get("private_key_multihash")
        if multihash is not None:
            private_key = _require_non_empty_string(multihash, f"{context}.private_key_multihash")
        else:
            hex_literal = payload.get("private_key_hex")
            bytes_literal = payload.get("private_key_bytes") or payload.get("private_key_seed")
            if hex_literal is None and bytes_literal is None:
                raise TypeError(f"{context}.private_key is required")
            if hex_literal is not None:
                hex_value = _normalize_hex_string(
                    hex_literal,
                    f"{context}.private_key_hex",
                    expected_length=64,
                )
            else:
                hex_value = _bytes_like_to_hex(
                    bytes_literal,
                    f"{context}.private_key_bytes",
                )
                if len(hex_value) != 64:
                    raise ValueError(f"{context}.private_key_bytes must contain 32 bytes")
            algorithm = payload.get("private_key_algorithm") or "ed25519"
            algorithm_literal = _require_non_empty_string(
                algorithm,
                f"{context}.private_key_algorithm",
            )
            private_key = f"{algorithm_literal}:{hex_value.lower()}"
    return {"authority": authority, "private_key": private_key}


def _normalize_publish_space_directory_manifest_request(
    request: Mapping[str, Any],
) -> Dict[str, Any]:
    credentials = _normalize_authority_credentials(
        request,
        context="publish_space_directory_manifest",
    )
    manifest_payload = request.get("manifest")
    if manifest_payload is None:
        raise TypeError("publish_space_directory_manifest.manifest is required")
    manifest = _normalize_space_directory_manifest_payload(
        manifest_payload,
        context="publish_space_directory_manifest.manifest",
    )
    payload: Dict[str, Any] = {**credentials, "manifest": manifest}
    reason = request.get("reason")
    if reason is not None:
        if not isinstance(reason, str):
            raise TypeError("publish_space_directory_manifest.reason must be a string")
        payload["reason"] = reason
    return payload


def _normalize_revoke_space_directory_manifest_request(
    request: Mapping[str, Any],
) -> Dict[str, Any]:
    credentials = _normalize_authority_credentials(
        request,
        context="revoke_space_directory_manifest",
    )
    _reject_alias_keys(
        request,
        {
            "uaid_literal": "uaid",
            "uaidLiteral": "uaid",
            "dataspace_id": "dataspace",
            "dataspaceId": "dataspace",
            "revokedEpoch": "revoked_epoch",
        },
        context="revoke_space_directory_manifest",
    )
    uaid_literal = request.get("uaid")
    uaid = _normalize_uaid_literal(
        uaid_literal,
        context="revoke_space_directory_manifest.uaid",
    )
    dataspace_raw = request.get("dataspace")
    if dataspace_raw is None:
        raise TypeError("revoke_space_directory_manifest.dataspace is required")
    dataspace = _normalize_positive_int(
        dataspace_raw,
        "revoke_space_directory_manifest.dataspace",
        allow_zero=True,
    )
    revoked_epoch_raw = request.get("revoked_epoch")
    if revoked_epoch_raw is None:
        raise TypeError("revoke_space_directory_manifest.revoked_epoch is required")
    revoked_epoch = _normalize_positive_int(
        revoked_epoch_raw,
        "revoke_space_directory_manifest.revoked_epoch",
        allow_zero=True,
    )
    payload: Dict[str, Any] = {
        **credentials,
        "uaid": uaid,
        "dataspace": dataspace,
        "revoked_epoch": revoked_epoch,
    }
    reason = request.get("reason")
    if reason is not None:
        if not isinstance(reason, str):
            raise TypeError("revoke_space_directory_manifest.reason must be a string")
        payload["reason"] = reason
    return payload


def _normalize_iso_week_label(value: Any, context: str) -> str:
    if isinstance(value, str):
        label = value.strip().upper()
        if not _ISO_WEEK_RE.match(label):
            raise ValueError(f"{context} must match YYYY-Www (e.g., 2026-W05)")
        return label
    if isinstance(value, (tuple, list)):
        if len(value) != 2:
            raise ValueError(f"{context} tuple must contain (year, week)")
        year = _normalize_positive_int(value[0], f"{context}.year", allow_zero=False)
        week = _normalize_positive_int(value[1], f"{context}.week", allow_zero=False)
        if week > 53:
            raise ValueError(f"{context}.week must be between 1 and 53")
        return f"{year:04d}-W{week:02d}"
    raise TypeError(f"{context} must be a string or (year, week) tuple")


def _coerce_bool_flag(value: Any, context: str) -> bool:
    if isinstance(value, bool):
        return value
    raise TypeError(f"{context} must be a boolean")


def _coerce_finite_float(value: Any, context: str) -> float:
    try:
        number = float(value)
    except (TypeError, ValueError):
        raise TypeError(f"{context} must be a finite number") from None
    if not math.isfinite(number):
        raise ValueError(f"{context} must be finite")
    return number


def _parse_optional_duration_ms_field(value: Any, context: str) -> Optional[int]:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be an object")
    return _coerce_int(value.get("ms"), f"{context}.ms", allow_zero=True)


def _normalize_base64_payload(
    explicit_b64: Optional[Any],
    fallback_payload: Optional[Any],
    context: str,
) -> str:
    source = explicit_b64 if explicit_b64 is not None else fallback_payload
    if source is None:
        raise ValueError(f"{context} must be provided")
    if isinstance(source, str):
        trimmed = source.strip()
        if not trimmed:
            raise ValueError(f"{context} must be a non-empty base64 string")
        try:
            base64.b64decode(trimmed, validate=True)
        except binascii.Error as exc:
            raise ValueError(f"{context} must be base64 encoded") from exc
        return trimmed
    if isinstance(source, (bytes, bytearray, memoryview)):
        return base64.b64encode(bytes(source)).decode("ascii")
    raise TypeError(f"{context} must be bytes or a base64 string")


def _build_sorafs_por_status_params(
    manifest_hex: Optional[str],
    provider_hex: Optional[str],
    epoch: Optional[int],
    status: Optional[str],
    limit: Optional[int],
    page_token_hex: Optional[str],
) -> Optional[Dict[str, Any]]:
    params: Dict[str, Any] = {}
    if manifest_hex is not None:
        params["manifest"] = _normalize_hex_string(
            manifest_hex, "sorafs_por_status.manifest_hex", expected_length=64
        )
    if provider_hex is not None:
        params["provider"] = _normalize_hex_string(
            provider_hex, "sorafs_por_status.provider_hex", expected_length=64
        )
    if epoch is not None:
        params["epoch"] = _normalize_positive_int(epoch, "sorafs_por_status.epoch", allow_zero=False)
    if status is not None:
        trimmed = status.strip()
        if not trimmed:
            raise ValueError("sorafs_por_status.status must be non-empty")
        params["status"] = trimmed
    if limit is not None:
        params["limit"] = _normalize_positive_int(limit, "sorafs_por_status.limit", allow_zero=False)
    if page_token_hex is not None:
        params["page_token"] = _normalize_hex_string(
            page_token_hex, "sorafs_por_status.page_token_hex", expected_length=64
        )
    return params or None


def _build_sorafs_por_export_params(
    start_epoch: Optional[int],
    end_epoch: Optional[int],
) -> Optional[Dict[str, Any]]:
    params: Dict[str, Any] = {}
    if start_epoch is not None:
        params["start_epoch"] = _normalize_positive_int(
            start_epoch, "sorafs_por_export.start_epoch", allow_zero=False
        )
    if end_epoch is not None:
        params["end_epoch"] = _normalize_positive_int(
            end_epoch, "sorafs_por_export.end_epoch", allow_zero=False
        )
    return params or None

_CRYPTO_MODULE: Optional[ModuleType] = None
_ISO_WEEK_RE = re.compile(r"^\d{4}-W(0[1-9]|[1-4][0-9]|5[0-3])$")


@dataclass(frozen=True)
class ResolvedToriiClientConfig:
    """Fully merged Torii client configuration."""

    timeout: float
    max_retries: int
    backoff_initial: float
    backoff_multiplier: float
    max_backoff: float
    retry_statuses: frozenset[int]
    retry_methods: frozenset[str]
    default_headers: Dict[str, str]
    auth_token: Optional[str]
    api_token: Optional[str]
    sorafs_alias_policy: SorafsAliasPolicy


_DEFAULT_RESOLVED_CONFIG = ResolvedToriiClientConfig(
    timeout=30.0,
    max_retries=3,
    backoff_initial=0.5,
    backoff_multiplier=2.0,
    max_backoff=5.0,
    retry_statuses=frozenset({429, 502, 503, 504}),
    retry_methods=frozenset({"GET", "HEAD", "OPTIONS"}),
    default_headers={"Accept": "application/json"},
    auth_token=None,
    api_token=None,
    sorafs_alias_policy=SorafsAliasPolicy.defaults(),
)


@dataclass(frozen=True)
class SorafsPorSubmissionResponse:
    """Response wrapper for PoR challenge/proof submissions."""

    status: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any], context: str) -> "SorafsPorSubmissionResponse":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be a JSON object")
        status = payload.get("status")
        if not isinstance(status, str) or not status.strip():
            raise TypeError(f"{context} missing string `status` field")
        return cls(status=status.strip())


@dataclass(frozen=True)
class SorafsPorObservationResponse:
    """Response wrapper for PoR observation submissions."""

    status: str
    success: bool

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any], context: str) -> "SorafsPorObservationResponse":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be a JSON object")
        status = payload.get("status")
        if not isinstance(status, str) or not status.strip():
            raise TypeError(f"{context} missing string `status` field")
        success = payload.get("success")
        if not isinstance(success, bool):
            raise TypeError(f"{context} missing boolean `success` field")
        return cls(status=status.strip(), success=success)


@dataclass(frozen=True)
class SorafsPorVerdictResponse:
    """Response wrapper for PoR verdict submissions."""

    status: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any], context: str) -> "SorafsPorVerdictResponse":
        base = SorafsPorSubmissionResponse.from_payload(payload, context)
        return cls(status=base.status)


@dataclass(frozen=True)
class SorafsPorIngestionProviderStatus:
    """Provider-level PoR ingestion snapshot returned by `/v1/sorafs/por/ingestion/{manifest}`."""

    provider_id_hex: str
    pending_challenges: int
    oldest_epoch_id: Optional[int]
    oldest_response_deadline_unix: Optional[int]
    last_success_unix: Optional[int]
    last_failure_unix: Optional[int]
    failures_total: int
    consecutive_failures: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SorafsPorIngestionProviderStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("por ingestion provider entry must be an object")
        provider_literal = payload.get("provider_id_hex")
        if not isinstance(provider_literal, str) or not provider_literal:
            raise TypeError("por ingestion provider entry missing string `provider_id_hex` field")
        provider_id_hex = _normalize_hex_string(
            provider_literal,
            "por_ingestion.provider_id_hex",
            expected_length=64,
        )
        pending = _coerce_int(
            payload.get("pending_challenges"),
            "por ingestion provider pending_challenges",
            allow_zero=True,
        )
        if pending is None:
            raise TypeError("por ingestion provider entry missing numeric `pending_challenges` field")
        oldest_epoch = _coerce_int(
            payload.get("oldest_epoch_id"),
            "por ingestion provider oldest_epoch_id",
            allow_zero=True,
        )
        oldest_deadline = _coerce_int(
            payload.get("oldest_response_deadline_unix"),
            "por ingestion provider oldest_response_deadline_unix",
            allow_zero=True,
        )
        last_success = _coerce_int(
            payload.get("last_success_unix"),
            "por ingestion provider last_success_unix",
            allow_zero=True,
        )
        last_failure = _coerce_int(
            payload.get("last_failure_unix"),
            "por ingestion provider last_failure_unix",
            allow_zero=True,
        )
        failures_total = _coerce_int(
            payload.get("failures_total"),
            "por ingestion provider failures_total",
            allow_zero=True,
        )
        if failures_total is None:
            raise TypeError("por ingestion provider entry missing numeric `failures_total` field")
        consecutive_failures = _coerce_int(
            payload.get("consecutive_failures"),
            "por ingestion provider consecutive_failures",
            allow_zero=True,
        )
        if consecutive_failures is None:
            raise TypeError("por ingestion provider entry missing numeric `consecutive_failures` field")
        return cls(
            provider_id_hex=provider_id_hex,
            pending_challenges=pending,
            oldest_epoch_id=oldest_epoch,
            oldest_response_deadline_unix=oldest_deadline,
            last_success_unix=last_success,
            last_failure_unix=last_failure,
            failures_total=failures_total,
            consecutive_failures=consecutive_failures,
        )


@dataclass(frozen=True)
class SorafsPorIngestionStatus:
    """Manifest-level PoR ingestion snapshot."""

    manifest_digest_hex: str
    providers: List[SorafsPorIngestionProviderStatus]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SorafsPorIngestionStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("por ingestion response must be an object")
        manifest_literal = payload.get("manifest_digest_hex")
        if not isinstance(manifest_literal, str) or not manifest_literal:
            raise TypeError("por ingestion response missing string `manifest_digest_hex` field")
        manifest_digest_hex = _normalize_hex_string(
            manifest_literal,
            "por_ingestion.manifest_digest_hex",
            expected_length=64,
        )
        providers_payload = payload.get("providers")
        if not isinstance(providers_payload, list):
            raise TypeError("por ingestion response `providers` must be a list")
        providers: List[SorafsPorIngestionProviderStatus] = []
        for index, entry in enumerate(providers_payload):
            if not isinstance(entry, Mapping):
                raise TypeError(f"por ingestion providers[{index}] must be an object")
            providers.append(SorafsPorIngestionProviderStatus.from_payload(entry))
        return cls(manifest_digest_hex=manifest_digest_hex, providers=providers)


@dataclass(frozen=True)
class ExplorerMetricsSnapshot:
    """Network metrics exposed via `/v1/explorer/metrics`."""

    peers: int
    domains: int
    accounts: int
    assets: int
    transactions_accepted: int
    transactions_rejected: int
    block_height: int
    block_created_at: Optional[str]
    finalized_block_height: int
    average_commit_time_ms: Optional[int]
    average_block_time_ms: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ExplorerMetricsSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("explorer metrics payload must be an object")

        def _resolve_int(key: str, label: str) -> int:
            raw = payload.get(key)
            parsed = _coerce_int(raw, label, allow_zero=True)
            return 0 if parsed is None else parsed

        def _resolve_optional_string(key: str, label: str) -> Optional[str]:
            raw = payload.get(key)
            if raw is None:
                return None
            if not isinstance(raw, str):
                raise TypeError(f"{label} must be a string")
            trimmed = raw.strip()
            return trimmed or None

        def _resolve_optional_duration(
            key: str,
            label: str,
        ) -> Optional[int]:
            raw = payload.get(key)
            return _parse_optional_duration_ms_field(raw, label)

        return cls(
            peers=_resolve_int("peers", "explorer_metrics.peers"),
            domains=_resolve_int("domains", "explorer_metrics.domains"),
            accounts=_resolve_int("accounts", "explorer_metrics.accounts"),
            assets=_resolve_int("assets", "explorer_metrics.assets"),
            transactions_accepted=_resolve_int(
                "transactions_accepted",
                "explorer_metrics.transactions_accepted",
            ),
            transactions_rejected=_resolve_int(
                "transactions_rejected",
                "explorer_metrics.transactions_rejected",
            ),
            block_height=_resolve_int(
                "block",
                "explorer_metrics.block",
            ),
            block_created_at=_resolve_optional_string(
                "block_created_at",
                "explorer_metrics.block_created_at",
            ),
            finalized_block_height=_resolve_int(
                "finalized_block",
                "explorer_metrics.finalized_block",
            ),
            average_commit_time_ms=_resolve_optional_duration(
                "avg_commit_time",
                "explorer_metrics.avg_commit_time",
            ),
            average_block_time_ms=_resolve_optional_duration(
                "avg_block_time",
                "explorer_metrics.avg_block_time",
            ),
        )


@dataclass(frozen=True)
class ExplorerAccountQrSnapshot:
    """Account QR metadata exposed via `/v1/explorer/accounts/{account_id}/qr`."""

    canonical_id: str
    literal: str
    address_format: str
    network_prefix: int
    error_correction: str
    modules: int
    qr_version: int
    svg: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ExplorerAccountQrSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("explorer account qr payload must be an object")

        def _require_string(key: str, label: str) -> str:
            raw = payload.get(key)
            if not isinstance(raw, str) or not raw.strip():
                raise TypeError(f"{label} must be a non-empty string")
            return raw.strip()

        def _require_positive_int(key: str, label: str) -> int:
            value = _coerce_int(payload.get(key), label, allow_zero=False)
            if value is None:
                raise TypeError(f"{label} must be provided")
            return value

        canonical_id = _require_string(
            "canonical_id",
            "explorer_account_qr.canonical_id",
        )
        literal = _require_string("literal", "explorer_account_qr.literal")
        error_correction = _require_string(
            "error_correction",
            "explorer_account_qr.error_correction",
        )
        svg = _require_string("svg", "explorer_account_qr.svg")

        network_prefix = _require_positive_int(
            "network_prefix",
            "explorer_account_qr.network_prefix",
        )
        modules = _require_positive_int("modules", "explorer_account_qr.modules")
        qr_version = _require_positive_int(
            "qr_version",
            "explorer_account_qr.qr_version",
        )

        address_raw = payload.get("address_format")
        if address_raw is None:
            address_format = "ih58"
        else:
            address_format = _normalize_address_format(str(address_raw)) or "ih58"

        return cls(
            canonical_id=canonical_id,
            literal=literal,
            address_format=address_format,
            network_prefix=network_prefix,
            error_correction=error_correction,
            modules=modules,
            qr_version=qr_version,
            svg=svg,
        )


@dataclass(frozen=True)
class IsoSubmissionRecord:
    """Normalized ISO 20022 bridge status payload."""

    message_id: str
    status: str
    pacs002_code: Optional[str]
    transaction_hash: Optional[str]
    hold_reason_code: Optional[str]
    change_reason_codes: Tuple[str, ...]
    rejection_reason_code: Optional[str]
    ledger_id: Optional[str]
    source_account_id: Optional[str]
    source_account_address: Optional[str]
    target_account_id: Optional[str]
    target_account_address: Optional[str]
    asset_definition_id: Optional[str]
    asset_id: Optional[str]
    detail: Optional[str]
    updated_at_ms: Optional[int]

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> "IsoSubmissionRecord":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be a JSON object")
        record = dict(payload)
        message_id = _require_non_empty_string(record.get("message_id"), f"{context}.message_id")
        status = _normalize_iso_status(record.get("status"), f"{context}.status")
        pacs002_code = _normalize_pacs002_code(record.get("pacs002_code"), f"{context}.pacs002_code")
        transaction_hash = _normalize_iso_optional_string(
            record.get("transaction_hash"),
            f"{context}.transaction_hash",
        )
        hold_reason_code = _normalize_iso_optional_string(
            record.get("hold_reason_code"),
            f"{context}.hold_reason_code",
        )
        change_reason_codes = _normalize_iso_string_array(
            record.get("change_reason_codes"),
            f"{context}.change_reason_codes",
        )
        rejection_reason_code = _normalize_iso_optional_string(
            record.get("rejection_reason_code"),
            f"{context}.rejection_reason_code",
        )
        ledger_id = _normalize_iso_optional_string(record.get("ledger_id"), f"{context}.ledger_id")
        source_account_id = _normalize_iso_optional_string(
            record.get("source_account_id"),
            f"{context}.source_account_id",
        )
        source_account_address = _normalize_iso_optional_string(
            record.get("source_account_address"),
            f"{context}.source_account_address",
        )
        target_account_id = _normalize_iso_optional_string(
            record.get("target_account_id"),
            f"{context}.target_account_id",
        )
        target_account_address = _normalize_iso_optional_string(
            record.get("target_account_address"),
            f"{context}.target_account_address",
        )
        asset_definition_id = _normalize_iso_optional_string(
            record.get("asset_definition_id"),
            f"{context}.asset_definition_id",
        )
        asset_id = _normalize_iso_optional_string(record.get("asset_id"), f"{context}.asset_id")
        detail = _normalize_iso_optional_string(
            record.get("detail"),
            f"{context}.detail",
            allow_empty=True,
        )
        updated_at_field = record.get("updated_at_ms")
        if updated_at_field is None:
            updated_at_ms = None
        else:
            updated_at_ms = _normalize_positive_int(
                updated_at_field,
                f"{context}.updated_at_ms",
                allow_zero=True,
            )
        return cls(
            message_id=message_id,
            status=status,
            pacs002_code=pacs002_code,
            transaction_hash=transaction_hash,
            hold_reason_code=hold_reason_code,
            change_reason_codes=change_reason_codes,
            rejection_reason_code=rejection_reason_code,
            ledger_id=ledger_id,
            source_account_id=source_account_id,
            source_account_address=source_account_address,
            target_account_id=target_account_id,
            target_account_address=target_account_address,
            asset_definition_id=asset_definition_id,
            asset_id=asset_id,
            detail=detail,
            updated_at_ms=updated_at_ms,
        )


class IsoMessageTimeoutError(RuntimeError):
    """Raised when ISO bridge messages fail to reach a terminal state."""

    def __init__(
        self,
        message_id: str,
        attempts: int,
        last_status: Optional[IsoSubmissionRecord],
    ) -> None:
        detail = f"Timed out waiting for ISO message {message_id} after {attempts} attempts"
        if last_status is not None:
            detail = f"{detail} (last status: {last_status.status})"
        super().__init__(detail)
        self.message_id = message_id
        self.attempts = attempts
        self.last_status = last_status


_KAIGI_HEALTH_STATUSES = frozenset({"healthy", "degraded", "unavailable"})


@dataclass(frozen=True)
class KaigiRelaySummary:
    """Summary entry returned by `/v1/kaigi/relays`."""

    relay_id: str
    domain: str
    bandwidth_class: int
    hpke_fingerprint_hex: str
    status: Optional[str]
    reported_at_ms: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelaySummary":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay summary payload must be an object")
        relay_literal = payload.get("relay_id")
        domain_literal = payload.get("domain")
        bandwidth_literal = payload.get("bandwidth_class")
        fingerprint_literal = payload.get("hpke_fingerprint_hex")
        status_literal = payload.get("status")
        reported_literal = payload.get("reported_at_ms")

        relay_id = _require_non_empty_string(relay_literal, "kaigi_relay_summary.relay_id")
        domain = _require_non_empty_string(domain_literal, "kaigi_relay_summary.domain")
        bandwidth_value = _coerce_int(
            bandwidth_literal,
            "kaigi_relay_summary.bandwidth_class",
            allow_zero=True,
        )
        if bandwidth_value is None:
            bandwidth_value = 0
        fingerprint = _require_non_empty_string(
            fingerprint_literal,
            "kaigi_relay_summary.hpke_fingerprint_hex",
        )
        status: Optional[str] = None
        if status_literal is not None:
            status_value = _require_non_empty_string(
                status_literal,
                "kaigi_relay_summary.status",
            ).lower()
            if status_value not in _KAIGI_HEALTH_STATUSES:
                raise ValueError(
                    f"kaigi_relay_summary.status must be one of {sorted(_KAIGI_HEALTH_STATUSES)}"
                )
            status = status_value
        reported_at_ms = (
            _coerce_int(
                reported_literal,
                "kaigi_relay_summary.reported_at_ms",
                allow_zero=True,
            )
            if reported_literal is not None
            else None
        )

        return cls(
            relay_id=relay_id,
            domain=domain,
            bandwidth_class=bandwidth_value,
            hpke_fingerprint_hex=_normalize_hex_string(
                fingerprint,
                "kaigi_relay_summary.hpke_fingerprint_hex",
            ),
            status=status,
            reported_at_ms=reported_at_ms,
        )


@dataclass(frozen=True)
class KaigiRelaySummaryList:
    """Payload envelope returned by `/v1/kaigi/relays`."""

    items: List[KaigiRelaySummary]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelaySummaryList":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay summary response must be an object")
        raw_items = payload.get("items") or []
        if not isinstance(raw_items, Sequence) or isinstance(raw_items, (bytes, bytearray, str)):
            raise TypeError("kaigi relay summary response `items` must be an array")
        items = []
        for index, entry in enumerate(raw_items):
            if not isinstance(entry, Mapping):
                raise TypeError(f"kaigi relay summary response items[{index}] must be an object")
            items.append(KaigiRelaySummary.from_payload(entry))
        total_literal = payload.get("total")
        total_value = _coerce_int(
            total_literal,
            "kaigi_relay_summary.total",
            allow_zero=True,
        )
        if total_value is None:
            total_value = len(items)
        return cls(items=items, total=total_value)


@dataclass(frozen=True)
class KaigiRelayReportedCall:
    """Call metadata referenced by `/v1/kaigi/relays/{relay_id}`."""

    domain_id: str
    call_name: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelayReportedCall":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay call payload must be an object")
        domain_literal = payload.get("domain_id")
        name_literal = payload.get("call_name")
        return cls(
            domain_id=_require_non_empty_string(
                domain_literal,
                "kaigi_relay_reported_call.domain_id",
            ),
            call_name=_require_non_empty_string(
                name_literal,
                "kaigi_relay_reported_call.call_name",
            ),
        )


@dataclass(frozen=True)
class KaigiRelayDomainMetrics:
    """Per-domain metrics included in Kaigi relay responses."""

    domain: str
    registrations_total: int
    manifest_updates_total: int
    failovers_total: int
    health_reports_total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelayDomainMetrics":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay domain metrics payload must be an object")
        domain_literal = payload.get("domain")

        def _resolve_counter(name: str) -> int:
            value = _coerce_int(payload.get(name), f"kaigi_relay_domain_metrics.{name}", allow_zero=True)
            return value or 0

        return cls(
            domain=_require_non_empty_string(domain_literal, "kaigi_relay_domain_metrics.domain"),
            registrations_total=_resolve_counter("registrations_total"),
            manifest_updates_total=_resolve_counter("manifest_updates_total"),
            failovers_total=_resolve_counter("failovers_total"),
            health_reports_total=_resolve_counter("health_reports_total"),
        )


@dataclass(frozen=True)
class KaigiRelayDetail:
    """Detailed relay metadata returned by `/v1/kaigi/relays/{relay_id}`."""

    relay: KaigiRelaySummary
    hpke_public_key_b64: str
    reported_call: Optional[KaigiRelayReportedCall]
    reported_by: Optional[str]
    notes: Optional[str]
    metrics: Optional[KaigiRelayDomainMetrics]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelayDetail":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay detail payload must be an object")
        relay_payload = payload.get("relay")
        if not isinstance(relay_payload, Mapping):
            raise TypeError("kaigi relay detail payload missing object `relay` field")
        hpke_literal = payload.get("hpke_public_key_b64")
        reported_call_payload = payload.get("reported_call")
        metrics_payload = payload.get("metrics")
        reported_by_literal = payload.get("reported_by")
        notes_literal = payload.get("notes")

        reported_by: Optional[str] = None
        if reported_by_literal is not None:
            reported_by = _require_non_empty_string(
                reported_by_literal,
                "kaigi_relay_detail.reported_by",
            )
        notes: Optional[str] = None
        if notes_literal is not None:
            if not isinstance(notes_literal, str):
                notes = str(notes_literal)
            else:
                trimmed = notes_literal.strip()
                notes = trimmed or None

        return cls(
            relay=KaigiRelaySummary.from_payload(relay_payload),
            hpke_public_key_b64=_require_non_empty_string(
                hpke_literal,
                "kaigi_relay_detail.hpke_public_key_b64",
            ),
            reported_call=KaigiRelayReportedCall.from_payload(reported_call_payload)
            if isinstance(reported_call_payload, Mapping)
            else None,
            reported_by=reported_by,
            notes=notes,
            metrics=KaigiRelayDomainMetrics.from_payload(metrics_payload)
            if isinstance(metrics_payload, Mapping)
            else None,
        )


@dataclass(frozen=True)
class KaigiRelayHealthSnapshot:
    """Aggregated relay health counters returned by `/v1/kaigi/relays/health`."""

    healthy_total: int
    degraded_total: int
    unavailable_total: int
    reports_total: int
    registrations_total: int
    failovers_total: int
    domains: List[KaigiRelayDomainMetrics]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelayHealthSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay health payload must be an object")
        domains_value = payload.get("domains") or []
        if not isinstance(domains_value, Sequence) or isinstance(domains_value, (bytes, bytearray, str)):
            raise TypeError("kaigi relay health payload `domains` must be an array")
        domains: List[KaigiRelayDomainMetrics] = []
        for index, entry in enumerate(domains_value):
            if not isinstance(entry, Mapping):
                raise TypeError(f"kaigi relay health payload domains[{index}] must be an object")
            domains.append(KaigiRelayDomainMetrics.from_payload(entry))

        def _resolve_counter(name: str) -> int:
            value = _coerce_int(payload.get(name), f"kaigi_relay_health.{name}", allow_zero=True)
            return value or 0

        return cls(
            healthy_total=_resolve_counter("healthy_total"),
            degraded_total=_resolve_counter("degraded_total"),
            unavailable_total=_resolve_counter("unavailable_total"),
            reports_total=_resolve_counter("reports_total"),
            registrations_total=_resolve_counter("registrations_total"),
            failovers_total=_resolve_counter("failovers_total"),
            domains=domains,
        )

def _configuration_snapshot_to_dict(snapshot: ConfigurationSnapshot) -> Dict[str, Any]:
    result: Dict[str, Any] = {
        "public_key": snapshot.public_key_hex,
        "logger": snapshot.logger.to_payload(),
        "network": {
            "block_gossip_size": snapshot.network.block_gossip_size,
            "block_gossip_period_ms": snapshot.network.block_gossip_period_ms,
            "transaction_gossip_size": snapshot.network.transaction_gossip_size,
            "transaction_gossip_period_ms": snapshot.network.transaction_gossip_period_ms,
        },
    }
    if snapshot.queue is not None:
        result["queue"] = {"capacity": snapshot.queue.capacity}
    if snapshot.confidential_gas is not None:
        result["confidential_gas"] = snapshot.confidential_gas.to_payload()
    return result


def _configuration_update_payload(snapshot: ConfigurationSnapshot) -> Dict[str, Any]:
    payload = _configuration_snapshot_to_dict(snapshot)
    payload.pop("public_key", None)
    return payload


def _network_time_snapshot_to_dict(snapshot: NetworkTimeSnapshot) -> Dict[str, int]:
    return {
        "now": snapshot.now_ms,
        "offset_ms": snapshot.offset_ms,
        "confidence_ms": snapshot.confidence_ms,
    }


def _network_time_status_to_dict(status: NetworkTimeStatus) -> Dict[str, Any]:
    samples = [
        {
            "peer": sample.peer,
            "last_offset_ms": sample.last_offset_ms,
            "last_rtt_ms": sample.last_rtt_ms,
            "count": sample.count,
        }
        for sample in status.samples
    ]
    rtt: Dict[str, Any] = {
        "buckets": [
            {"le": bucket.upper_bound_ms, "count": bucket.count}
            for bucket in status.rtt_buckets
        ],
        "sum_ms": status.rtt_sum_ms,
        "count": status.rtt_count,
    }
    payload: Dict[str, Any] = {
        "peers": status.peers,
        "samples": samples,
        "rtt": rtt,
    }
    if status.note is not None:
        payload["note"] = status.note
    return payload

@dataclass(frozen=True)
class GovernanceReferendumResult:
    """Wrapper for `/v1/gov/referenda/{id}` responses."""

    found: bool
    referendum: Optional[Dict[str, Any]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceReferendumResult":
        if not isinstance(payload, Mapping):
            raise TypeError("referendum payload must be a mapping")
        found = payload.get("found")
        if not isinstance(found, bool):
            raise TypeError("referendum payload missing bool `found` field")
        referendum = payload.get("referendum")
        if referendum is not None and not isinstance(referendum, Mapping):
            raise TypeError("referendum payload `referendum` must be an object when present")
        copied = dict(referendum) if isinstance(referendum, Mapping) else None
        return cls(found=found, referendum=copied)


@dataclass(frozen=True)
class GovernanceTally:
    """Referendum tally summary returned by `/v1/gov/tally/{id}`."""

    referendum_id: str
    approve: int
    reject: int
    abstain: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceTally":
        if not isinstance(payload, Mapping):
            raise TypeError("tally payload must be a mapping")
        referendum_id = payload.get("referendum_id")
        if not isinstance(referendum_id, str):
            raise TypeError("tally payload missing string `referendum_id`")
        try:
            approve = int(payload.get("approve", 0))
            reject = int(payload.get("reject", 0))
            abstain = int(payload.get("abstain", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("tally payload must contain numeric approve/reject/abstain values") from exc
        return cls(
            referendum_id=referendum_id,
            approve=approve,
            reject=reject,
            abstain=abstain,
        )


class GovernanceProposalStatus(str, Enum):
    """Governance proposal lifecycle status."""

    PROPOSED = "Proposed"
    APPROVED = "Approved"
    REJECTED = "Rejected"
    ENACTED = "Enacted"

    @classmethod
    def from_value(cls, value: str) -> "GovernanceProposalStatus":
        try:
            return cls(value)
        except ValueError as exc:
            raise TypeError("proposal status must be one of Proposed, Approved, Rejected, Enacted") from exc


@dataclass(frozen=True)
class GovernanceProposalDeployContract:
    """`DeployContract` payload embedded in governance proposals."""

    namespace: str
    contract_id: str
    code_hash_hex: str
    abi_hash_hex: str
    abi_version: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceProposalDeployContract":
        if not isinstance(payload, Mapping):
            raise TypeError("DeployContract payload must be an object")
        namespace = payload.get("namespace")
        contract_id = payload.get("contract_id")
        code_hash_hex = payload.get("code_hash_hex")
        abi_hash_hex = payload.get("abi_hash_hex")
        abi_version = payload.get("abi_version")
        for field_name, field_value in [
            ("namespace", namespace),
            ("contract_id", contract_id),
            ("code_hash_hex", code_hash_hex),
            ("abi_hash_hex", abi_hash_hex),
            ("abi_version", abi_version),
        ]:
            if not isinstance(field_value, str):
                raise TypeError(f"DeployContract payload missing string `{field_name}` field")
        return cls(
            namespace=namespace,
            contract_id=contract_id,
            code_hash_hex=code_hash_hex,
            abi_hash_hex=abi_hash_hex,
            abi_version=abi_version,
        )


@dataclass(frozen=True)
class GovernanceProposalKind:
    """Normalized governance proposal kind."""

    variant: str
    deploy_contract: Optional[GovernanceProposalDeployContract]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceProposalKind":
        if not isinstance(payload, Mapping):
            raise TypeError("proposal kind must be an object")
        if len(payload) != 1:
            raise TypeError("proposal kind must contain exactly one variant entry")
        (variant, details), = payload.items()
        if not isinstance(variant, str):
            raise TypeError("proposal kind variant key must be a string")
        raw: Dict[str, Any]
        deploy_contract: Optional[GovernanceProposalDeployContract] = None
        if isinstance(details, Mapping):
            raw = dict(details)
        else:
            raw = {"value": details}
        if variant == "DeployContract":
            if not isinstance(details, Mapping):
                raise TypeError("DeployContract proposal kind expects an object payload")
            deploy_contract = GovernanceProposalDeployContract.from_payload(details)
        return cls(variant=variant, deploy_contract=deploy_contract, raw=raw)


@dataclass(frozen=True)
class GovernanceProposalRecord:
    """Structured governance proposal record."""

    proposer: str
    kind: GovernanceProposalKind
    created_height: int
    status: GovernanceProposalStatus

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceProposalRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("proposal record must be an object")
        proposer = payload.get("proposer")
        if not isinstance(proposer, str):
            raise TypeError("proposal record missing string `proposer` field")
        kind_payload = payload.get("kind")
        if not isinstance(kind_payload, Mapping):
            raise TypeError("proposal record missing object `kind` field")
        created_height_raw = payload.get("created_height")
        status_raw = payload.get("status")
        if not isinstance(status_raw, str):
            raise TypeError("proposal record missing string `status` field")
        try:
            created_height = int(created_height_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("proposal record `created_height` must be numeric") from exc
        status = GovernanceProposalStatus.from_value(status_raw)
        kind = GovernanceProposalKind.from_payload(kind_payload)
        return cls(
            proposer=proposer,
            kind=kind,
            created_height=created_height,
            status=status,
        )


@dataclass(frozen=True)
class GovernanceProposalResult:
    """Wrapper for `/v1/gov/proposals/{id}` responses."""

    found: bool
    proposal: Optional[GovernanceProposalRecord]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceProposalResult":
        if not isinstance(payload, Mapping):
            raise TypeError("proposal response must be an object")
        found = payload.get("found")
        if not isinstance(found, bool):
            raise TypeError("proposal response missing bool `found` field")
        proposal_payload = payload.get("proposal")
        if proposal_payload is None:
            return cls(found=found, proposal=None)
        if not isinstance(proposal_payload, Mapping):
            raise TypeError("proposal response `proposal` must be an object when present")
        proposal = GovernanceProposalRecord.from_payload(proposal_payload)
        return cls(found=found, proposal=proposal)


@dataclass(frozen=True)
class GovernanceLockRecord:
    """Governance lock record stored for a referendum."""

    owner: str
    amount: int
    expiry_height: int
    direction: int
    duration_blocks: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceLockRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("governance lock record must be an object")
        owner = payload.get("owner")
        if not isinstance(owner, str):
            raise TypeError("governance lock record missing string `owner` field")
        amount_raw = payload.get("amount")
        try:
            amount = int(amount_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("governance lock record `amount` must be numeric") from exc
        expiry_raw = payload.get("expiry_height")
        try:
            expiry_height = int(expiry_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("governance lock record `expiry_height` must be numeric") from exc
        direction_raw = payload.get("direction")
        try:
            direction = int(direction_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("governance lock record `direction` must be numeric") from exc
        if direction < 0 or direction > 255:
            raise ValueError("governance lock record `direction` must be within 0-255")
        duration_raw = payload.get("duration_blocks", 0)
        try:
            duration_blocks = int(duration_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("governance lock record `duration_blocks` must be numeric") from exc
        return cls(
            owner=owner,
            amount=amount,
            expiry_height=expiry_height,
            direction=direction,
            duration_blocks=duration_blocks,
        )


@dataclass(frozen=True)
class GovernanceLocksResult:
    """Wrapper for `/v1/gov/locks/{id}` responses."""

    found: bool
    referendum_id: str
    locks: Dict[str, GovernanceLockRecord]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceLocksResult":
        if not isinstance(payload, Mapping):
            raise TypeError("locks response must be an object")
        found = payload.get("found")
        if not isinstance(found, bool):
            raise TypeError("locks response missing bool `found` field")
        referendum_id = payload.get("referendum_id")
        if not isinstance(referendum_id, str):
            raise TypeError("locks response missing string `referendum_id` field")
        locks_payload = payload.get("locks")
        parsed: Dict[str, GovernanceLockRecord] = {}
        if locks_payload is not None:
            if not isinstance(locks_payload, Mapping):
                raise TypeError("locks response `locks` must be an object when present")
            for account, record_payload in locks_payload.items():
                if not isinstance(account, str):
                    raise TypeError("locks response keys must be account-id strings")
                if not isinstance(record_payload, Mapping):
                    raise TypeError("locks response values must be objects")
                parsed[account] = GovernanceLockRecord.from_payload(record_payload)
        return cls(found=found, referendum_id=referendum_id, locks=parsed)


@dataclass(frozen=True)
class GovernanceUnlockStats:
    """Aggregate unlock sweep statistics returned by `/v1/gov/unlocks/stats`."""

    height_current: int
    expired_locks_now: int
    referenda_with_expired: int
    last_sweep_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceUnlockStats":
        if not isinstance(payload, Mapping):
            raise TypeError("unlock stats payload must be an object")
        try:
            height_current = int(payload.get("height_current"))
            expired_locks_now = int(payload.get("expired_locks_now"))
            referenda_with_expired = int(payload.get("referenda_with_expired"))
            last_sweep_height = int(payload.get("last_sweep_height"))
        except (TypeError, ValueError) as exc:
            raise TypeError("unlock stats fields must be numeric") from exc
        return cls(
            height_current=height_current,
            expired_locks_now=expired_locks_now,
            referenda_with_expired=referenda_with_expired,
            last_sweep_height=last_sweep_height,
        )


@dataclass(frozen=True)
class ContractInstance:
    """Contract instance metadata returned by Torii listings."""

    contract_id: str
    code_hash_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractInstance":
        if not isinstance(payload, Mapping):
            raise TypeError("instance payload must be an object")
        contract_id = payload.get("contract_id")
        code_hash_hex = payload.get("code_hash_hex")
        if not isinstance(contract_id, str):
            raise TypeError("instance payload missing string `contract_id` field")
        if not isinstance(code_hash_hex, str):
            raise TypeError("instance payload missing string `code_hash_hex` field")
        return cls(contract_id=contract_id, code_hash_hex=code_hash_hex)


@dataclass(frozen=True)
class ContractInstancesPage:
    """Paginated response for contract instance listings."""

    namespace: str
    instances: List[ContractInstance]
    total: int
    offset: int
    limit: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractInstancesPage":
        if not isinstance(payload, Mapping):
            raise TypeError("instances response must be an object")
        namespace = payload.get("namespace")
        if not isinstance(namespace, str):
            raise TypeError("instances response missing string `namespace` field")
        instances_payload = payload.get("instances")
        if not isinstance(instances_payload, list):
            raise TypeError("instances response missing list `instances` field")
        instances: List[ContractInstance] = []
        for item in instances_payload:
            if not isinstance(item, Mapping):
                raise TypeError("instance entry must be an object")
            instances.append(ContractInstance.from_payload(item))
        try:
            total = int(payload.get("total", 0))
            offset = int(payload.get("offset", 0))
            limit = int(payload.get("limit", len(instances)))
        except (TypeError, ValueError) as exc:
            raise TypeError("instances response pagination fields must be numeric") from exc
        return cls(
            namespace=namespace,
            instances=instances,
            total=total,
            offset=offset,
            limit=limit,
        )


@dataclass(frozen=True)
class ContractManifest:
    """On-chain contract manifest metadata."""

    code_hash: Optional[str]
    abi_hash: Optional[str]
    compiler_fingerprint: Optional[str]
    features_bitmap: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractManifest":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest payload must be an object")
        code_hash = payload.get("code_hash")
        if code_hash is not None and not isinstance(code_hash, str):
            raise TypeError("manifest `code_hash` must be a string when provided")
        abi_hash = payload.get("abi_hash")
        if abi_hash is not None and not isinstance(abi_hash, str):
            raise TypeError("manifest `abi_hash` must be a string when provided")
        compiler_fingerprint = payload.get("compiler_fingerprint")
        if compiler_fingerprint is not None and not isinstance(compiler_fingerprint, str):
            raise TypeError("manifest `compiler_fingerprint` must be a string when provided")
        features_raw = payload.get("features_bitmap")
        if features_raw is None:
            features_bitmap: Optional[int] = None
        else:
            try:
                features_bitmap = int(features_raw)
            except (TypeError, ValueError) as exc:
                raise TypeError("manifest `features_bitmap` must be numeric") from exc
        return cls(
            code_hash=code_hash,
            abi_hash=abi_hash,
            compiler_fingerprint=compiler_fingerprint,
            features_bitmap=features_bitmap,
        )


@dataclass(frozen=True)
class ContractManifestRecord:
    """Contract manifest record returned by Torii (`/v1/contracts/code/{hash}`)."""

    manifest: ContractManifest
    code_bytes: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractManifestRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest response must be an object")
        manifest_payload = payload.get("manifest")
        if not isinstance(manifest_payload, Mapping):
            raise TypeError("manifest response missing object `manifest` field")
        manifest = ContractManifest.from_payload(manifest_payload)
        code_bytes = payload.get("code_bytes")
        if code_bytes is not None and not isinstance(code_bytes, str):
            raise TypeError("manifest response `code_bytes` must be a string when provided")
        return cls(manifest=manifest, code_bytes=code_bytes)


@dataclass(frozen=True)
class PeerInfo:
    """Metadata describing an online peer returned by `GET /v1/peers`."""

    address: str
    public_key_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerInfo":
        if not isinstance(payload, Mapping):
            raise TypeError("peer payload must be a mapping")
        address = payload.get("address")
        if not isinstance(address, str):
            raise TypeError("peer payload missing string `address` field")
        id_section = payload.get("id")
        if not isinstance(id_section, Mapping):
            raise TypeError("peer payload missing `id` object")
        public_key = id_section.get("public_key")
        if not isinstance(public_key, str):
            raise TypeError("peer id missing string `public_key` field")
        return cls(address=address, public_key_hex=public_key)


@dataclass(frozen=True)
class PeerTelemetryConfig:
    """Configuration snapshot returned by `/v1/telemetry/peers-info`."""

    public_key_hex: str
    queue_capacity: Optional[int]
    network_block_gossip_size: Optional[int]
    network_block_gossip_period_ms: Optional[int]
    network_tx_gossip_size: Optional[int]
    network_tx_gossip_period_ms: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerTelemetryConfig":
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry peer config must be an object")
        public_key = payload.get("public_key")
        if not isinstance(public_key, str) or not public_key:
            raise TypeError("telemetry peer config missing string `public_key` field")
        queue_capacity = _coerce_int(
            payload.get("queue_capacity"),
            "telemetry peer config queue_capacity",
            allow_zero=True,
        )
        block_size = _coerce_int(
            payload.get("network_block_gossip_size"),
            "telemetry peer config network_block_gossip_size",
            allow_zero=True,
        )
        tx_size = _coerce_int(
            payload.get("network_tx_gossip_size"),
            "telemetry peer config network_tx_gossip_size",
            allow_zero=True,
        )
        block_period = _parse_optional_duration_ms_field(
            payload.get("network_block_gossip_period"),
            "telemetry peer config network_block_gossip_period",
        )
        tx_period = _parse_optional_duration_ms_field(
            payload.get("network_tx_gossip_period"),
            "telemetry peer config network_tx_gossip_period",
        )
        return cls(
            public_key_hex=public_key,
            queue_capacity=queue_capacity,
            network_block_gossip_size=block_size,
            network_block_gossip_period_ms=block_period,
            network_tx_gossip_size=tx_size,
            network_tx_gossip_period_ms=tx_period,
        )


@dataclass(frozen=True)
class PeerTelemetryLocation:
    """Geolocation metadata for `/v1/telemetry/peers-info` entries."""

    lat: float
    lon: float
    country: str
    city: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerTelemetryLocation":
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry peer location must be an object")
        lat = _coerce_finite_float(payload.get("lat"), "telemetry peer location lat")
        lon = _coerce_finite_float(payload.get("lon"), "telemetry peer location lon")
        country = payload.get("country")
        city = payload.get("city")
        if not isinstance(country, str) or not country:
            raise TypeError("telemetry peer location missing string `country` field")
        if not isinstance(city, str) or not city:
            raise TypeError("telemetry peer location missing string `city` field")
        return cls(lat=lat, lon=lon, country=country, city=city)


@dataclass(frozen=True)
class PeerTelemetryInfo:
    """Entry returned by `GET /v1/telemetry/peers-info`."""

    url: str
    connected: bool
    telemetry_unsupported: bool
    config: Optional[PeerTelemetryConfig]
    location: Optional[PeerTelemetryLocation]
    connected_peers: Optional[List[str]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerTelemetryInfo":
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry peer payload must be an object")
        url = payload.get("url")
        if not isinstance(url, str) or not url:
            raise TypeError("telemetry peer payload missing string `url` field")
        connected = _coerce_bool_flag(payload.get("connected"), "telemetry peer connected")
        telemetry_flag = payload.get("telemetry_unsupported")
        telemetry_unsupported = _coerce_bool_flag(
            telemetry_flag if telemetry_flag is not None else False,
            "telemetry peer telemetry_unsupported",
        )
        config_payload = payload.get("config")
        if config_payload is not None and not isinstance(config_payload, Mapping):
            raise TypeError("telemetry peer config must be an object when provided")
        location_payload = payload.get("location")
        if location_payload is not None and not isinstance(location_payload, Mapping):
            raise TypeError("telemetry peer location must be an object when provided")
        peers_value = payload.get("connected_peers")
        connected_peers = None
        if peers_value is not None:
            if not isinstance(peers_value, list):
                raise TypeError("telemetry peer `connected_peers` must be a list when provided")
            peer_list: List[str] = []
            for index, peer in enumerate(peers_value):
                if not isinstance(peer, str) or not peer:
                    raise TypeError(f"telemetry peer connected_peers[{index}] must be a non-empty string")
                peer_list.append(peer)
            connected_peers = peer_list
        return cls(
            url=url,
            connected=connected,
            telemetry_unsupported=telemetry_unsupported,
            config=PeerTelemetryConfig.from_payload(config_payload) if config_payload else None,
            location=PeerTelemetryLocation.from_payload(location_payload) if location_payload else None,
            connected_peers=connected_peers,
        )


@dataclass(frozen=True)
class SseEvent:
    """Structured Server-Sent Event returned by Torii SSE endpoints."""

    event: Optional[str]
    data: Any
    id: Optional[str]
    retry: Optional[int]
    raw: str


@dataclass
class EventCursor:
    """Track the last observed SSE event id so streams can resume after reconnects."""

    last_event_id: Optional[str] = None

    def advance(self, event: SseEvent) -> None:
        """Record the latest event id if present."""

        if event.id is not None:
            self.last_event_id = event.id


@dataclass(frozen=True)
class NodeSmAcceleration:
    """Acceleration advert nested within :class:`NodeCapabilities`."""

    scalar: bool
    neon_sm3: bool
    neon_sm4: bool
    policy: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeSmAcceleration":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities acceleration payload must be an object")
        try:
            scalar = bool(payload.get("scalar", False))
            neon_sm3 = bool(payload.get("neon_sm3", False))
            neon_sm4 = bool(payload.get("neon_sm4", False))
        except (TypeError, ValueError) as exc:
            raise TypeError("node capabilities acceleration booleans must be bool") from exc
        policy_value = payload.get("policy", "unknown")
        if policy_value is None:
            policy = "unknown"
        else:
            policy = str(policy_value)
        return cls(scalar=scalar, neon_sm3=neon_sm3, neon_sm4=neon_sm4, policy=policy)


@dataclass(frozen=True)
class NodeSmCapabilities:
    """SM manifest nested within :class:`NodeCapabilities`."""

    enabled: bool
    default_hash: str
    allowed_signing: List[str]
    sm2_distid_default: str
    openssl_preview: bool
    acceleration: NodeSmAcceleration

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeSmCapabilities":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities `crypto.sm` payload must be an object")
        enabled = bool(payload.get("enabled", False))
        default_hash_value = payload.get("default_hash", "")
        default_hash = str(default_hash_value)
        allowed_payload = payload.get("allowed_signing", [])
        if not isinstance(allowed_payload, list):
            raise TypeError("node capabilities `allowed_signing` must be a list")
        allowed_signing = [str(item) for item in allowed_payload]
        sm2_distid_default = str(payload.get("sm2_distid_default", ""))
        openssl_preview = bool(payload.get("openssl_preview", False))
        accel_payload = payload.get("acceleration", {})
        acceleration = NodeSmAcceleration.from_payload(accel_payload)
        return cls(
            enabled=enabled,
            default_hash=default_hash,
            allowed_signing=allowed_signing,
            sm2_distid_default=sm2_distid_default,
            openssl_preview=openssl_preview,
            acceleration=acceleration,
        )


@dataclass(frozen=True)
class NodeCurveCapabilities:
    """Curve manifest nested within :class:`NodeCapabilities`."""

    registry_version: int
    allowed_curve_ids: List[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeCurveCapabilities":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities `crypto.curves` payload must be an object")
        raw_version = payload.get("registry_version", 1)
        try:
            registry_version = int(raw_version)
        except (TypeError, ValueError) as exc:
            raise TypeError("node curve capability `registry_version` must be numeric") from exc
        if registry_version <= 0:
            raise TypeError("node curve capability `registry_version` must be positive")
        allowed_payload = payload.get("allowed_curve_ids", [])
        if not isinstance(allowed_payload, list):
            raise TypeError("node curve capability `allowed_curve_ids` must be a list")
        allowed_curve_ids: List[int] = []
        for entry in allowed_payload:
            try:
                allowed_curve_ids.append(int(entry))
            except (TypeError, ValueError) as exc:
                raise TypeError("node curve capability `allowed_curve_ids` entries must be numeric") from exc
        return cls(registry_version=registry_version, allowed_curve_ids=allowed_curve_ids)


@dataclass(frozen=True)
class NodeCryptoCapabilities:
    """Crypto manifest nested within :class:`NodeCapabilities`."""

    sm: NodeSmCapabilities
    curves: NodeCurveCapabilities

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeCryptoCapabilities":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities `crypto` payload must be an object")
        sm_payload = payload.get("sm")
        if not isinstance(sm_payload, Mapping):
            raise TypeError("node capabilities `crypto.sm` payload must be an object")
        sm_caps = NodeSmCapabilities.from_payload(sm_payload)
        curves_payload = payload.get("curves", {})
        if not isinstance(curves_payload, Mapping):
            raise TypeError("node capabilities `crypto.curves` payload must be an object when present")
        curves_caps = NodeCurveCapabilities.from_payload(curves_payload)
        return cls(sm=sm_caps, curves=curves_caps)


@dataclass(frozen=True)
class NodeCapabilities:
    """Typed advert covering `/v1/node/capabilities`."""

    supported_abi_versions: List[int]
    default_compile_target: int
    crypto: Optional[NodeCryptoCapabilities]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeCapabilities":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities payload must be an object")
        versions = payload.get("supported_abi_versions")
        if not isinstance(versions, list):
            raise TypeError("node capabilities missing list `supported_abi_versions` field")
        supported = []
        for item in versions:
            try:
                supported.append(int(item))
            except (TypeError, ValueError) as exc:
                raise TypeError("supported_abi_versions entries must be numeric") from exc
        try:
            default = int(payload["default_compile_target"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("node capabilities missing numeric `default_compile_target` field") from exc
        crypto_payload = payload.get("crypto")
        crypto_caps: Optional[NodeCryptoCapabilities]
        if crypto_payload is None:
            crypto_caps = None
        else:
            if not isinstance(crypto_payload, Mapping):
                raise TypeError("node capabilities `crypto` field must be an object when present")
            crypto_caps = NodeCryptoCapabilities.from_payload(crypto_payload)
        return cls(
            supported_abi_versions=supported,
            default_compile_target=default,
            crypto=crypto_caps,
        )


@dataclass(frozen=True)
class NodeAdminSnapshot:
    """Aggregated evidence captured from `/v1/configuration`, `/v1/peers`, `/v1/time/*`, `/v1/telemetry/peers-info`, and `/v1/node/capabilities`."""

    configuration: ConfigurationSnapshot
    peers: List[PeerInfo]
    time_now: NetworkTimeSnapshot
    time_status: NetworkTimeStatus
    node_capabilities: NodeCapabilities
    telemetry_peers: Optional[List[PeerTelemetryInfo]] = None


@dataclass(frozen=True)
class PipelineDagSnapshot:
    """Deterministic DAG fingerprint snapshot in pipeline recovery payloads."""

    fingerprint_hex: str
    key_count: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PipelineDagSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline DAG snapshot must be an object")
        fingerprint = payload.get("fingerprint")
        if not isinstance(fingerprint, str):
            raise TypeError("pipeline DAG snapshot missing string `fingerprint`")
        try:
            key_count = int(payload.get("key_count", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("pipeline DAG snapshot `key_count` must be numeric") from exc
        return cls(fingerprint_hex=fingerprint, key_count=key_count)


@dataclass(frozen=True)
class PipelineTxSnapshot:
    """Access summary for a transaction in a pipeline recovery sidecar."""

    hash_hex: str
    reads: List[str]
    writes: List[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PipelineTxSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline transaction snapshot must be an object")
        hash_hex = payload.get("hash")
        if not isinstance(hash_hex, str):
            raise TypeError("pipeline transaction snapshot missing string `hash`")
        reads_value = payload.get("reads", [])
        writes_value = payload.get("writes", [])
        if not isinstance(reads_value, list) or not all(isinstance(item, str) for item in reads_value):
            raise TypeError("pipeline transaction snapshot `reads` must be a list of strings")
        if not isinstance(writes_value, list) or not all(isinstance(item, str) for item in writes_value):
            raise TypeError("pipeline transaction snapshot `writes` must be a list of strings")
        return cls(hash_hex=hash_hex, reads=list(reads_value), writes=list(writes_value))


@dataclass(frozen=True)
class PipelineRecoverySidecar:
    """Typed representation of `/v1/pipeline/recovery/{height}` responses."""

    format: str
    height: int
    dag: PipelineDagSnapshot
    txs: List[PipelineTxSnapshot]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PipelineRecoverySidecar":
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline recovery payload must be an object")
        format_label = payload.get("format")
        if not isinstance(format_label, str):
            raise TypeError("pipeline recovery payload missing string `format`")
        try:
            height = int(payload.get("height", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("pipeline recovery `height` must be numeric") from exc
        dag_payload = payload.get("dag")
        if not isinstance(dag_payload, Mapping):
            raise TypeError("pipeline recovery payload missing object `dag`")
        txs_payload = payload.get("txs", [])
        if not isinstance(txs_payload, list):
            raise TypeError("pipeline recovery payload `txs` must be a list")
        dag = PipelineDagSnapshot.from_payload(dag_payload)
        txs = [PipelineTxSnapshot.from_payload(item) for item in txs_payload]
        return cls(format=format_label, height=height, dag=dag, txs=txs)


@dataclass(frozen=True)
class AccountAsset:
    """Account asset entry returned by account asset listings."""

    asset_id: str
    quantity: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountAsset":
        if not isinstance(payload, Mapping):
            raise TypeError("account asset entry must be an object")
        asset_id = payload.get("asset_id")
        quantity = payload.get("quantity")
        if not isinstance(asset_id, str):
            raise TypeError("account asset entry missing string `asset_id` field")
        if not isinstance(quantity, str):
            raise TypeError("account asset entry missing string `quantity` field")
        return cls(asset_id=asset_id, quantity=quantity)


@dataclass(frozen=True)
class AccountAssetsPage:
    """Paginated account asset list."""

    items: List[AccountAsset]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountAssetsPage":
        if not isinstance(payload, Mapping):
            raise TypeError("account assets response must be an object")
        items_raw = payload.get("items", [])
        if not isinstance(items_raw, list):
            raise TypeError("account assets response `items` must be a list")
        try:
            total = int(payload.get("total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("account assets response `total` must be numeric") from exc
        items = [AccountAsset.from_payload(entry) for entry in items_raw]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class AccountTransaction:
    """Projection of a transaction returned by account transaction listings."""

    entrypoint_hash: str
    result_ok: bool
    authority: Optional[str]
    timestamp_ms: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountTransaction":
        if not isinstance(payload, Mapping):
            raise TypeError("account transaction entry must be an object")
        entrypoint_hash = payload.get("entrypoint_hash")
        if not isinstance(entrypoint_hash, str):
            raise TypeError("account transaction entry missing string `entrypoint_hash` field")
        result_ok = payload.get("result_ok")
        if not isinstance(result_ok, bool):
            raise TypeError("account transaction entry missing bool `result_ok` field")
        authority = payload.get("authority")
        if authority is not None and not isinstance(authority, str):
            raise TypeError("account transaction `authority` must be a string when provided")
        timestamp_value = payload.get("timestamp_ms")
        if timestamp_value is None:
            timestamp_ms: Optional[int] = None
        else:
            try:
                timestamp_ms = int(timestamp_value)
            except (TypeError, ValueError) as exc:
                raise TypeError("account transaction `timestamp_ms` must be numeric") from exc
        return cls(
            entrypoint_hash=entrypoint_hash,
            result_ok=result_ok,
            authority=authority,
            timestamp_ms=timestamp_ms,
        )


@dataclass(frozen=True)
class AccountTransactionsPage:
    """Paginated account transaction list."""

    items: List[AccountTransaction]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountTransactionsPage":
        if not isinstance(payload, Mapping):
            raise TypeError("account transactions response must be an object")
        items_raw = payload.get("items", [])
        if not isinstance(items_raw, list):
            raise TypeError("account transactions response `items` must be a list")
        try:
            total = int(payload.get("total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("account transactions response `total` must be numeric") from exc
        items = [AccountTransaction.from_payload(entry) for entry in items_raw]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class AccountRecord:
    """Account entry returned by account queries."""

    id: str
    signatories: List[str]
    metadata: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("account record must be an object")
        account_id = payload.get("id")
        if not isinstance(account_id, str):
            raise TypeError("account record missing string `id` field")
        signatories_payload = payload.get("signatories", [])
        if signatories_payload is None:
            signatories_list: List[str] = []
        elif isinstance(signatories_payload, list):
            if not all(isinstance(item, str) for item in signatories_payload):
                raise TypeError("account record `signatories` must be a list of strings")
            signatories_list = list(signatories_payload)
        else:
            raise TypeError("account record `signatories` must be a list")
        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata_dict: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata_dict = dict(metadata_payload)
        else:
            raise TypeError("account record `metadata` must be an object when present")
        return cls(
            id=account_id,
            signatories=signatories_list,
            metadata=metadata_dict,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class AccountListPage:
    """Paginated account query result."""

    items: List[AccountRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("account query payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("account query `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("account query `total` must be numeric") from exc
        items = [AccountRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class DomainRecord:
    """Domain projection returned by domain listings and queries."""

    id: str
    owned_by: Optional[str]
    metadata: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "DomainRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("domain record must be an object")
        domain_id = payload.get("id")
        if not isinstance(domain_id, str):
            raise TypeError("domain record missing string `id` field")
        owned_by = payload.get("owned_by")
        if owned_by is not None and not isinstance(owned_by, str):
            raise TypeError("domain record `owned_by` must be a string when provided")
        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata = dict(metadata_payload)
        else:
            raise TypeError("domain record `metadata` must be an object when present")
        return cls(id=domain_id, owned_by=owned_by, metadata=metadata, raw=dict(payload))


@dataclass(frozen=True)
class DomainListPage:
    """Paginated domain query result."""

    items: List[DomainRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "DomainListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("domain query payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("domain query `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("domain query `total` must be numeric") from exc
        items = [DomainRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class AssetDefinitionRecord:
    """Asset definition projection returned by asset definition queries."""

    id: str
    metadata: Dict[str, Any]
    owned_by: Optional[str]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AssetDefinitionRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("asset definition record must be an object")
        definition_id = payload.get("id")
        if not isinstance(definition_id, str):
            raise TypeError("asset definition record missing string `id` field")
        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata = dict(metadata_payload)
        else:
            raise TypeError("asset definition record `metadata` must be an object when present")
        owned_by = payload.get("owned_by")
        if owned_by is not None and not isinstance(owned_by, str):
            raise TypeError("asset definition record `owned_by` must be a string when provided")
        return cls(id=definition_id, metadata=metadata, owned_by=owned_by, raw=dict(payload))


@dataclass(frozen=True)
class AssetDefinitionListPage:
    """Paginated asset definition query result."""

    items: List[AssetDefinitionRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AssetDefinitionListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("asset definition query payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("asset definition query `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("asset definition query `total` must be numeric") from exc
        items = [AssetDefinitionRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class AssetHolderRecord:
    """Asset holder projection returned by asset holder queries."""

    account_id: str
    quantity: str
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AssetHolderRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("asset holder record must be an object")
        account_id = payload.get("account_id")
        quantity = payload.get("quantity")
        if not isinstance(account_id, str):
            raise TypeError("asset holder record missing string `account_id` field")
        if not isinstance(quantity, str):
            raise TypeError("asset holder record missing string `quantity` field")
        return cls(account_id=account_id, quantity=quantity, raw=dict(payload))


@dataclass(frozen=True)
class AssetHolderListPage:
    """Paginated asset holder query result."""

    items: List[AssetHolderRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AssetHolderListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("asset holder query payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("asset holder query `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("asset holder query `total` must be numeric") from exc
        items = [AssetHolderRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class AccountPermissionRecord:
    """Account permission entry returned by `GET /v1/accounts/{account_id}/permissions`."""

    name: str
    payload: Any
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountPermissionRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("account permission record must be an object")
        name = payload.get("name")
        if not isinstance(name, str):
            raise TypeError("account permission record missing string `name` field")
        permission_payload = _json_safe_value(payload.get("payload"))
        return cls(name=name, payload=permission_payload, raw=dict(payload))


@dataclass(frozen=True)
class AccountPermissionListPage:
    """Paginated account permission result."""

    items: List[AccountPermissionRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountPermissionListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("account permission payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("account permission `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("account permission `total` must be numeric") from exc
        items = [AccountPermissionRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


# ---------------------------------------------------------------------------
# UAID portfolio & Space Directory helpers
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class UaidPortfolioAsset:
    asset_id: str
    asset_definition_id: str
    quantity: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidPortfolioAsset":
        if not isinstance(payload, Mapping):
            raise TypeError("portfolio asset must be an object")
        asset_id = payload.get("asset_id")
        definition_id = payload.get("asset_definition_id")
        quantity = payload.get("quantity")
        if not isinstance(asset_id, str) or not asset_id:
            raise TypeError("portfolio asset missing `asset_id` string")
        if not isinstance(definition_id, str) or not definition_id:
            raise TypeError("portfolio asset missing `asset_definition_id` string")
        if not isinstance(quantity, str) or not quantity:
            raise TypeError("portfolio asset missing `quantity` string")
        return cls(
            asset_id=asset_id,
            asset_definition_id=definition_id,
            quantity=quantity,
        )


@dataclass(frozen=True)
class UaidPortfolioAccount:
    account_id: str
    label: Optional[str]
    assets: List[UaidPortfolioAsset]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidPortfolioAccount":
        if not isinstance(payload, Mapping):
            raise TypeError("portfolio account must be an object")
        account_id = payload.get("account_id")
        label = payload.get("label")
        if not isinstance(account_id, str) or not account_id:
            raise TypeError("portfolio account missing `account_id` string")
        if label is not None and not isinstance(label, str):
            raise TypeError("portfolio account `label` must be a string when present")
        assets_payload = payload.get("assets", [])
        if not isinstance(assets_payload, list):
            raise TypeError("portfolio account `assets` must be a list")
        assets = [UaidPortfolioAsset.from_payload(item) for item in assets_payload]
        return cls(account_id=account_id, label=label, assets=assets)


@dataclass(frozen=True)
class UaidPortfolioDataspace:
    dataspace_id: int
    dataspace_alias: Optional[str]
    accounts: List[UaidPortfolioAccount]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidPortfolioDataspace":
        if not isinstance(payload, Mapping):
            raise TypeError("portfolio dataspace must be an object")
        dataspace_id = _coerce_int(payload.get("dataspace_id"), "dataspace_id", allow_zero=True)
        if dataspace_id is None:
            raise TypeError("portfolio dataspace missing `dataspace_id` integer")
        alias = payload.get("dataspace_alias")
        if alias is not None and not isinstance(alias, str):
            raise TypeError("portfolio dataspace `dataspace_alias` must be a string when present")
        accounts_payload = payload.get("accounts", [])
        if not isinstance(accounts_payload, list):
            raise TypeError("portfolio dataspace `accounts` must be a list")
        accounts = [UaidPortfolioAccount.from_payload(item) for item in accounts_payload]
        return cls(dataspace_id=dataspace_id, dataspace_alias=alias, accounts=accounts)


@dataclass(frozen=True)
class UaidPortfolioSnapshot:
    uaid: str
    total_accounts: int
    total_positions: int
    dataspaces: List[UaidPortfolioDataspace]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidPortfolioSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("portfolio payload must be an object")
        uaid = payload.get("uaid")
        if not isinstance(uaid, str) or not uaid:
            raise TypeError("portfolio payload missing `uaid` string")
        totals = payload.get("totals", {})
        if not isinstance(totals, Mapping):
            raise TypeError("portfolio payload `totals` must be an object")
        accounts = _coerce_int(totals.get("accounts"), "totals.accounts", allow_zero=True)
        positions = _coerce_int(totals.get("positions"), "totals.positions", allow_zero=True)
        if accounts is None or positions is None:
            raise TypeError("portfolio payload missing totals")
        dataspaces_payload = payload.get("dataspaces", [])
        if not isinstance(dataspaces_payload, list):
            raise TypeError("portfolio payload `dataspaces` must be a list")
        dataspaces = [UaidPortfolioDataspace.from_payload(item) for item in dataspaces_payload]
        return cls(
            uaid=uaid,
            total_accounts=accounts,
            total_positions=positions,
            dataspaces=dataspaces,
        )


@dataclass(frozen=True)
class UaidBindingsSlice:
    dataspace_id: int
    dataspace_alias: Optional[str]
    accounts: List[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidBindingsSlice":
        if not isinstance(payload, Mapping):
            raise TypeError("bindings slice must be an object")
        dataspace_id = _coerce_int(payload.get("dataspace_id"), "dataspace_id", allow_zero=True)
        if dataspace_id is None:
            raise TypeError("bindings slice missing `dataspace_id` integer")
        alias = payload.get("dataspace_alias")
        if alias is not None and not isinstance(alias, str):
            raise TypeError("bindings slice `dataspace_alias` must be a string when present")
        accounts_value = payload.get("accounts", [])
        if not isinstance(accounts_value, list):
            raise TypeError("bindings slice `accounts` must be a list")
        accounts: List[str] = []
        for index, literal in enumerate(accounts_value):
            if not isinstance(literal, str) or not literal:
                raise TypeError(f"bindings slice account[{index}] must be a non-empty string")
            accounts.append(literal)
        return cls(dataspace_id=dataspace_id, dataspace_alias=alias, accounts=accounts)


@dataclass(frozen=True)
class UaidBindingsSnapshot:
    uaid: str
    dataspaces: List[UaidBindingsSlice]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidBindingsSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("bindings payload must be an object")
        uaid = payload.get("uaid")
        if not isinstance(uaid, str) or not uaid:
            raise TypeError("bindings payload missing `uaid` string")
        dataspaces_payload = payload.get("dataspaces", [])
        if not isinstance(dataspaces_payload, list):
            raise TypeError("bindings payload `dataspaces` must be a list")
        dataspaces = [UaidBindingsSlice.from_payload(item) for item in dataspaces_payload]
        return cls(uaid=uaid, dataspaces=dataspaces)


@dataclass(frozen=True)
class SpaceDirectoryManifestLifecycle:
    activated_epoch: Optional[int]
    expired_epoch: Optional[int]
    revocation_epoch: Optional[int]
    revocation_reason: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SpaceDirectoryManifestLifecycle":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest lifecycle must be an object")
        activated = _coerce_int(payload.get("activated_epoch"), "lifecycle.activated_epoch", allow_zero=True)
        expired = _coerce_int(payload.get("expired_epoch"), "lifecycle.expired_epoch", allow_zero=True)
        revocation = payload.get("revocation")
        revocation_epoch: Optional[int] = None
        revocation_reason: Optional[str] = None
        if revocation is not None:
            if not isinstance(revocation, Mapping):
                raise TypeError("lifecycle.revocation must be an object when present")
            revocation_epoch = _coerce_int(revocation.get("epoch"), "lifecycle.revocation.epoch", allow_zero=True)
            reason_value = revocation.get("reason")
            if reason_value is not None and not isinstance(reason_value, str):
                raise TypeError("lifecycle.revocation.reason must be a string when present")
            revocation_reason = reason_value
        return cls(
            activated_epoch=activated,
            expired_epoch=expired,
            revocation_epoch=revocation_epoch,
            revocation_reason=revocation_reason,
        )


@dataclass(frozen=True)
class SpaceDirectoryManifestRecord:
    dataspace_id: int
    dataspace_alias: Optional[str]
    manifest_hash: str
    status: str
    lifecycle: SpaceDirectoryManifestLifecycle
    accounts: List[str]
    manifest: Mapping[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SpaceDirectoryManifestRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest record must be an object")
        dataspace_id = _coerce_int(payload.get("dataspace_id"), "dataspace_id", allow_zero=True)
        if dataspace_id is None:
            raise TypeError("manifest record missing `dataspace_id` integer")
        alias = payload.get("dataspace_alias")
        if alias is not None and not isinstance(alias, str):
            raise TypeError("manifest record `dataspace_alias` must be a string when present")
        manifest_hash = payload.get("manifest_hash")
        if not isinstance(manifest_hash, str) or not manifest_hash:
            raise TypeError("manifest record missing `manifest_hash` string")
        status = payload.get("status")
        if not isinstance(status, str) or not status:
            raise TypeError("manifest record missing `status` string")
        lifecycle_payload = payload.get("lifecycle", {})
        lifecycle = SpaceDirectoryManifestLifecycle.from_payload(lifecycle_payload)
        accounts_value = payload.get("accounts", [])
        if not isinstance(accounts_value, list):
            raise TypeError("manifest record `accounts` must be a list")
        accounts: List[str] = []
        for index, literal in enumerate(accounts_value):
            if not isinstance(literal, str) or not literal:
                raise TypeError(f"manifest record account[{index}] must be a non-empty string")
            accounts.append(literal)
        manifest_payload = payload.get("manifest")
        if not isinstance(manifest_payload, Mapping):
            raise TypeError("manifest record `manifest` must be an object")
        return cls(
            dataspace_id=dataspace_id,
            dataspace_alias=alias,
            manifest_hash=manifest_hash,
            status=status,
            lifecycle=lifecycle,
            accounts=accounts,
            manifest=dict(manifest_payload),
        )


@dataclass(frozen=True)
class SpaceDirectoryManifestList:
    uaid: str
    total: int
    manifests: List[SpaceDirectoryManifestRecord]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SpaceDirectoryManifestList":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest list payload must be an object")
        uaid = payload.get("uaid")
        if not isinstance(uaid, str) or not uaid:
            raise TypeError("manifest list payload missing `uaid` string")
        total = payload.get("total")
        try:
            total_int = int(total)
        except (TypeError, ValueError) as exc:
            raise TypeError("manifest list `total` must be numeric") from exc
        manifests_payload = payload.get("manifests", [])
        if not isinstance(manifests_payload, list):
            raise TypeError("manifest list `manifests` must be a list")
        manifests = [SpaceDirectoryManifestRecord.from_payload(item) for item in manifests_payload]
        return cls(uaid=uaid, total=total_int, manifests=manifests)


@dataclass(frozen=True)
class TriggerRecord:
    """Trigger definition returned by trigger listing/query endpoints."""

    id: str
    action: Dict[str, Any]
    metadata: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TriggerRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("trigger record must be an object")
        trigger_id = payload.get("id")
        if not isinstance(trigger_id, str):
            raise TypeError("trigger record missing string `id` field")
        action_payload = payload.get("action", {})
        if not isinstance(action_payload, Mapping):
            raise TypeError("trigger record `action` must be an object")
        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata = dict(metadata_payload)
        else:
            raise TypeError("trigger record `metadata` must be an object when present")
        return cls(
            id=trigger_id,
            action=dict(action_payload),
            metadata=metadata,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class TriggerListPage:
    """Paginated trigger listing."""

    items: List[TriggerRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TriggerListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("trigger list payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("trigger list `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("trigger list `total` must be numeric") from exc
        items = [TriggerRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class TriggerMutationResponse:
    """Governance draft emitted when triggers are registered or deleted."""

    ok: bool
    tx_instructions: List[RuntimeInstruction]
    trigger_id: Optional[str]
    proposal_id: Optional[str]
    accepted: Optional[bool]
    message: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TriggerMutationResponse":
        if not isinstance(payload, Mapping):
            raise TypeError("trigger mutation response must be an object")
        ok_value = payload.get("ok")
        if not isinstance(ok_value, bool):
            raise TypeError("trigger mutation response missing boolean `ok` field")
        trigger_id_field = payload.get("trigger_id")
        if trigger_id_field is None:
            trigger_id: Optional[str] = None
        else:
            trigger_id = _require_non_empty_string(
                trigger_id_field, "trigger mutation response `trigger_id`"
            )
        proposal_field = payload.get("proposal_id")
        if proposal_field is None:
            proposal_id: Optional[str] = None
        else:
            proposal_id = _require_non_empty_string(
                proposal_field, "trigger mutation response `proposal_id`"
            )
        instructions_payload = payload.get("tx_instructions")
        if instructions_payload is None:
            instructions_payload = []
        if not isinstance(instructions_payload, list):
            raise TypeError("trigger mutation response `tx_instructions` must be a list")
        instructions = [RuntimeInstruction.from_payload(entry) for entry in instructions_payload]
        accepted_field = payload.get("accepted")
        accepted = None
        if accepted_field is not None:
            if not isinstance(accepted_field, bool):
                raise TypeError("trigger mutation response `accepted` must be boolean when present")
            accepted = accepted_field
        message_field = payload.get("message")
        if message_field is None:
            message_field = payload.get("error")
        if message_field is None:
            message_field = payload.get("reason")
        message = None
        if message_field is not None:
            message = str(message_field).strip()
            if not message:
                message = None
        return cls(
            ok=ok_value,
            tx_instructions=instructions,
            trigger_id=trigger_id,
            proposal_id=proposal_id,
            accepted=accepted,
            message=message,
        )


@dataclass(frozen=True)
class SumeragiAvailabilityCollector:
    """Collector vote ingestion snapshot exposed via `/v1/sumeragi/telemetry`."""

    collector_idx: int
    peer_id: str
    votes_ingested: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiAvailabilityCollector":
        if not isinstance(payload, Mapping):
            raise TypeError("availability collector entry must be an object")
        try:
            collector_idx = int(payload["collector_idx"])
            votes_ingested = int(payload["votes_ingested"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("availability collector entry must expose numeric counters") from exc
        peer_id = payload.get("peer_id")
        if not isinstance(peer_id, str):
            raise TypeError("availability collector entry missing string `peer_id` field")
        return cls(
            collector_idx=collector_idx,
            peer_id=peer_id,
            votes_ingested=votes_ingested,
        )


@dataclass(frozen=True)
class SumeragiAvailabilitySnapshot:
    """Aggregated availability vote ingestion statistics."""

    total_votes_ingested: int
    collectors: List[SumeragiAvailabilityCollector]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiAvailabilitySnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("availability snapshot must be an object")
        try:
            total_votes_ingested = int(payload["total_votes_ingested"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("availability snapshot missing numeric `total_votes_ingested`") from exc
        raw_collectors = payload.get("collectors", [])
        if raw_collectors is None:
            collectors_payload: List[Any] = []
        elif isinstance(raw_collectors, list):
            collectors_payload = raw_collectors
        else:
            raise TypeError("availability snapshot `collectors` must be a list when present")
        collectors = [
            SumeragiAvailabilityCollector.from_payload(entry) for entry in collectors_payload
        ]
        return cls(total_votes_ingested=total_votes_ingested, collectors=collectors)


@dataclass(frozen=True)
class SumeragiQcLatencyEntry:
    """Latency moving average for a consensus stage."""

    kind: str
    last_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiQcLatencyEntry":
        if not isinstance(payload, Mapping):
            raise TypeError("QC latency entry must be an object")
        kind = payload.get("kind")
        if not isinstance(kind, str):
            raise TypeError("QC latency entry missing string `kind` field")
        try:
            last_ms = int(payload["last_ms"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("QC latency entry missing numeric `last_ms` field") from exc
        return cls(kind=kind, last_ms=last_ms)


@dataclass(frozen=True)
class SumeragiRbcBacklog:
    """Backpressure snapshot for RBC chunk ingestion."""

    pending_sessions: int
    total_missing_chunks: int
    max_missing_chunks: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcBacklog":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC backlog snapshot must be an object")
        try:
            pending_sessions = int(payload.get("pending_sessions", 0))
            total_missing_chunks = int(payload.get("total_missing_chunks", 0))
            max_missing_chunks = int(payload.get("max_missing_chunks", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC backlog counters must be numeric") from exc
        return cls(
            pending_sessions=pending_sessions,
            total_missing_chunks=total_missing_chunks,
            max_missing_chunks=max_missing_chunks,
        )


@dataclass(frozen=True)
class SumeragiVrfLateReveal:
    """Late reveal entry surfaced in the telemetry VRF summary."""

    signer: str
    noted_at_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiVrfLateReveal":
        if not isinstance(payload, Mapping):
            raise TypeError("VRF late reveal entry must be an object")
        signer = payload.get("signer")
        if not isinstance(signer, str):
            raise TypeError("VRF late reveal entry missing string `signer` field")
        try:
            noted_at_height = int(payload["noted_at_height"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("VRF late reveal entry missing numeric `noted_at_height`") from exc
        return cls(signer=signer, noted_at_height=noted_at_height)


@dataclass(frozen=True)
class SumeragiVrfSummary:
    """VRF epoch snapshot embedded in `/v1/sumeragi/telemetry`."""

    found: bool
    epoch: int
    finalized: bool
    seed_hex: Optional[str]
    epoch_length: int
    commit_deadline_offset: int
    reveal_deadline_offset: int
    roster_len: int
    updated_at_height: int
    participants_total: int
    commitments_total: int
    reveals_total: int
    late_reveals_total: int
    committed_no_reveal: List[int]
    no_participation: List[int]
    late_reveals: List[SumeragiVrfLateReveal]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiVrfSummary":
        if not isinstance(payload, Mapping):
            raise TypeError("VRF summary must be an object")
        found = payload.get("found")
        if not isinstance(found, bool):
            raise TypeError("VRF summary missing bool `found` field")
        try:
            epoch = int(payload.get("epoch", 0))
            epoch_length = int(payload.get("epoch_length", 0))
            commit_deadline_offset = int(payload.get("commit_deadline_offset", 0))
            reveal_deadline_offset = int(payload.get("reveal_deadline_offset", 0))
            roster_len = int(payload.get("roster_len", 0))
            updated_at_height = int(payload.get("updated_at_height", 0))
            participants_total = int(payload.get("participants_total", 0))
            commitments_total = int(payload.get("commitments_total", 0))
            reveals_total = int(payload.get("reveals_total", 0))
            late_reveals_total = int(payload.get("late_reveals_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("VRF summary numeric fields must contain integers") from exc
        finalized = payload.get("finalized")
        if not isinstance(finalized, bool):
            raise TypeError("VRF summary missing bool `finalized` field")
        seed_value = payload.get("seed_hex")
        if seed_value is not None and seed_value is not False and not isinstance(seed_value, str):
            raise TypeError("VRF summary `seed_hex` must be a string when provided")
        seed_hex: Optional[str]
        if isinstance(seed_value, str):
            seed_hex = seed_value
        else:
            seed_hex = None
        committed_payload = payload.get("committed_no_reveal", [])
        no_participation_payload = payload.get("no_participation", [])
        if committed_payload is None:
            committed_raw: List[Any] = []
        elif isinstance(committed_payload, list):
            committed_raw = committed_payload
        else:
            raise TypeError("VRF summary `committed_no_reveal` must be a list when present")
        if no_participation_payload is None:
            no_participation_raw: List[Any] = []
        elif isinstance(no_participation_payload, list):
            no_participation_raw = no_participation_payload
        else:
            raise TypeError("VRF summary `no_participation` must be a list when present")
        try:
            committed_no_reveal = [int(item) for item in committed_raw]
            no_participation = [int(item) for item in no_participation_raw]
        except (TypeError, ValueError) as exc:
            raise TypeError("VRF summary participant arrays must be numeric") from exc
        late_reveals_payload = payload.get("late_reveals", [])
        if late_reveals_payload is None:
            late_reveals_raw: List[Any] = []
        elif isinstance(late_reveals_payload, list):
            late_reveals_raw = late_reveals_payload
        else:
            raise TypeError("VRF summary `late_reveals` must be a list when present")
        late_reveals = [
            SumeragiVrfLateReveal.from_payload(entry) for entry in late_reveals_raw
        ]
        return cls(
            found=found,
            epoch=epoch,
            finalized=finalized,
            seed_hex=seed_hex,
            epoch_length=epoch_length,
            commit_deadline_offset=commit_deadline_offset,
            reveal_deadline_offset=reveal_deadline_offset,
            roster_len=roster_len,
            updated_at_height=updated_at_height,
            participants_total=participants_total,
            commitments_total=commitments_total,
            reveals_total=reveals_total,
            late_reveals_total=late_reveals_total,
            committed_no_reveal=committed_no_reveal,
            no_participation=no_participation,
            late_reveals=late_reveals,
        )


@dataclass(frozen=True)
class SumeragiTelemetrySnapshot:
    """Typed payload for `/v1/sumeragi/telemetry`."""

    availability: SumeragiAvailabilitySnapshot
    qc_latency_ms: List[SumeragiQcLatencyEntry]
    rbc_backlog: SumeragiRbcBacklog
    vrf: SumeragiVrfSummary

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiTelemetrySnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry payload must be an object")
        availability_payload = payload.get("availability")
        if not isinstance(availability_payload, Mapping):
            raise TypeError("telemetry payload missing object `availability` field")
        rbc_payload = payload.get("rbc_backlog")
        if not isinstance(rbc_payload, Mapping):
            raise TypeError("telemetry payload missing object `rbc_backlog` field")
        qc_payload = payload.get("qc_latency_ms", [])
        if qc_payload is None:
            qc_entries_raw: List[Any] = []
        elif isinstance(qc_payload, list):
            qc_entries_raw = qc_payload
        else:
            raise TypeError("telemetry payload `qc_latency_ms` must be a list when present")
        vrf_payload = payload.get("vrf")
        if not isinstance(vrf_payload, Mapping):
            raise TypeError("telemetry payload missing object `vrf` field")
        availability = SumeragiAvailabilitySnapshot.from_payload(availability_payload)
        qc_entries = [SumeragiQcLatencyEntry.from_payload(entry) for entry in qc_entries_raw]
        rbc_backlog = SumeragiRbcBacklog.from_payload(rbc_payload)
        vrf = SumeragiVrfSummary.from_payload(vrf_payload)
        return cls(
            availability=availability,
            qc_latency_ms=qc_entries,
            rbc_backlog=rbc_backlog,
            vrf=vrf,
        )


@dataclass(frozen=True)
class SumeragiEvidenceRecord:
    """Evidence record returned by `/v1/sumeragi/evidence`."""

    kind: str
    recorded_height: int
    recorded_view: int
    recorded_ms: int
    data: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiEvidenceRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi evidence record must be an object")
        kind = payload.get("kind")
        if not isinstance(kind, str):
            raise TypeError("sumeragi evidence record missing string `kind` field")
        try:
            recorded_height = int(payload.get("recorded_height"))
            recorded_view = int(payload.get("recorded_view"))
            recorded_ms = int(payload.get("recorded_ms"))
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi evidence timing fields must be numeric") from exc
        extras = {
            key: value
            for key, value in payload.items()
            if key not in {"kind", "recorded_height", "recorded_view", "recorded_ms"}
        }
        return cls(
            kind=kind,
            recorded_height=recorded_height,
            recorded_view=recorded_view,
            recorded_ms=recorded_ms,
            data=extras,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class SumeragiEvidenceListPage:
    """Paginated evidence snapshot from `/v1/sumeragi/evidence`."""

    items: List[SumeragiEvidenceRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiEvidenceListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi evidence payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("sumeragi evidence `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi evidence `total` must be numeric") from exc
        items = [SumeragiEvidenceRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class SumeragiQcSummary:
    """Quorum certificate tuple {height, view, subject_block_hash}."""

    height: int
    view: int
    subject_block_hash: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiQcSummary":
        if not isinstance(payload, Mapping):
            raise TypeError("QC summary must be an object")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("QC summary `height` and `view` must be numeric") from exc
        hash_value = payload.get("subject_block_hash")
        if hash_value is None:
            subject_hash: Optional[str] = None
        elif isinstance(hash_value, str):
            subject_hash = hash_value
        else:
            raise TypeError("QC summary `subject_block_hash` must be a string when present")
        return cls(height=height, view=view, subject_block_hash=subject_hash)


@dataclass(frozen=True)
class SumeragiTxQueueStatus:
    """Transaction queue saturation snapshot."""

    depth: int
    capacity: Optional[int]
    saturated: bool

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiTxQueueStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("transaction queue payload must be an object")
        try:
            depth = int(payload.get("depth", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("transaction queue `depth` must be numeric") from exc
        capacity_val = payload.get("capacity")
        capacity: Optional[int]
        if capacity_val is None:
            capacity = None
        else:
            try:
                capacity = int(capacity_val)
            except (TypeError, ValueError) as exc:
                raise TypeError("transaction queue `capacity` must be numeric when present") from exc
        saturated = payload.get("saturated")
        if not isinstance(saturated, bool):
            raise TypeError("transaction queue `saturated` flag must be boolean")
        return cls(depth=depth, capacity=capacity, saturated=saturated)


@dataclass(frozen=True)
class SumeragiEpochSchedule:
    """Current epoch scheduling parameters."""

    length_blocks: int
    commit_deadline_offset: int
    reveal_deadline_offset: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiEpochSchedule":
        if not isinstance(payload, Mapping):
            raise TypeError("epoch schedule must be an object")
        try:
            length_blocks = int(payload.get("length_blocks", 0))
            commit_deadline_offset = int(payload.get("commit_deadline_offset", 0))
            reveal_deadline_offset = int(payload.get("reveal_deadline_offset", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("epoch schedule fields must be numeric") from exc
        return cls(
            length_blocks=length_blocks,
            commit_deadline_offset=commit_deadline_offset,
            reveal_deadline_offset=reveal_deadline_offset,
        )


@dataclass(frozen=True)
class SumeragiRbcEviction:
    """Evicted RBC payload metadata."""

    block_hash: Optional[str]
    height: Optional[int]
    view: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcEviction":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC eviction entry must be an object")
        block_hash = payload.get("block_hash")
        if block_hash is not None and not isinstance(block_hash, str):
            raise TypeError("RBC eviction `block_hash` must be a string when present")
        height_val = payload.get("height")
        view_val = payload.get("view")
        try:
            height = None if height_val is None else int(height_val)
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC eviction `height` must be numeric when present") from exc
        try:
            view = None if view_val is None else int(view_val)
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC eviction `view` must be numeric when present") from exc
        return cls(block_hash=block_hash, height=height, view=view)


@dataclass(frozen=True)
class SumeragiRbcStoreStatus:
    """Reliable broadcast backlog health."""

    sessions: int
    bytes: int
    pressure_level: int
    backpressure_deferrals_total: int
    evictions_total: int
    recent_evictions: List[SumeragiRbcEviction]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcStoreStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC store payload must be an object")
        try:
            sessions = int(payload.get("sessions", 0))
            bytes_used = int(payload.get("bytes", 0))
            pressure_level = int(payload.get("pressure_level", 0))
            backpressure_deferrals_total = int(payload.get("backpressure_deferrals_total", 0))
            evictions_total = int(payload.get("evictions_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC store counters must be numeric") from exc
        raw_evictions = payload.get("recent_evictions", [])
        if raw_evictions is None:
            evictions_payload: List[Any] = []
        elif isinstance(raw_evictions, list):
            evictions_payload = raw_evictions
        else:
            raise TypeError("RBC store `recent_evictions` must be a list when present")
        recent_evictions = [SumeragiRbcEviction.from_payload(entry) for entry in evictions_payload]
        return cls(
            sessions=sessions,
            bytes=bytes_used,
            pressure_level=pressure_level,
            backpressure_deferrals_total=backpressure_deferrals_total,
            evictions_total=evictions_total,
            recent_evictions=recent_evictions,
        )


@dataclass(frozen=True)
class SumeragiPrfStatus:
    """Pending PRF (pseudo-random function) window state."""

    height: int
    view: int
    epoch_seed: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiPrfStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("PRF status must be an object")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("PRF status `height` and `view` must be numeric") from exc
        seed_value = payload.get("epoch_seed")
        if seed_value is None:
            epoch_seed: Optional[str] = None
        elif isinstance(seed_value, str):
            epoch_seed = seed_value
        else:
            raise TypeError("PRF status `epoch_seed` must be a string when present")
        return cls(height=height, view=view, epoch_seed=epoch_seed)


@dataclass(frozen=True)
class SumeragiMembershipStatus:
    """Deterministic membership hash snapshot."""

    height: int
    view: int
    epoch: int
    view_hash: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiMembershipStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("membership status must be an object")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
            epoch = int(payload.get("epoch", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("membership `height`, `view`, and `epoch` must be numeric") from exc
        view_hash_value = payload.get("view_hash")
        if view_hash_value is None:
            view_hash: Optional[str] = None
        elif isinstance(view_hash_value, str):
            view_hash = view_hash_value
        else:
            raise TypeError("membership `view_hash` must be a string when present")
        return cls(height=height, view=view, epoch=epoch, view_hash=view_hash)


@dataclass(frozen=True)
class SumeragiLaneCommitment:
    """Lane-level commitment snapshot surfaced by `/v1/sumeragi/status`."""

    block_height: int
    lane_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiLaneCommitment":
        if not isinstance(payload, Mapping):
            raise TypeError("lane commitment entry must be an object")
        try:
            block_height = int(payload.get("block_height", 0))
            lane_id = int(payload.get("lane_id", 0))
            tx_count = int(payload.get("tx_count", 0))
            total_chunks = int(payload.get("total_chunks", 0))
            rbc_bytes_total = int(payload.get("rbc_bytes_total", 0))
            teu_total = int(payload.get("teu_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("lane commitment numeric fields must be numeric") from exc
        block_hash = payload.get("block_hash")
        if not isinstance(block_hash, str):
            raise TypeError("lane commitment `block_hash` must be a string")
        return cls(
            block_height=block_height,
            lane_id=lane_id,
            tx_count=tx_count,
            total_chunks=total_chunks,
            rbc_bytes_total=rbc_bytes_total,
            teu_total=teu_total,
            block_hash=block_hash,
        )


@dataclass(frozen=True)
class SumeragiDataspaceCommitment:
    """Dataspace-level commitment snapshot."""

    block_height: int
    lane_id: int
    dataspace_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiDataspaceCommitment":
        if not isinstance(payload, Mapping):
            raise TypeError("dataspace commitment entry must be an object")
        try:
            block_height = int(payload.get("block_height", 0))
            lane_id = int(payload.get("lane_id", 0))
            dataspace_id = int(payload.get("dataspace_id", 0))
            tx_count = int(payload.get("tx_count", 0))
            total_chunks = int(payload.get("total_chunks", 0))
            rbc_bytes_total = int(payload.get("rbc_bytes_total", 0))
            teu_total = int(payload.get("teu_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("dataspace commitment numeric fields must be numeric") from exc
        block_hash = payload.get("block_hash")
        if not isinstance(block_hash, str):
            raise TypeError("dataspace commitment `block_hash` must be a string")
        return cls(
            block_height=block_height,
            lane_id=lane_id,
            dataspace_id=dataspace_id,
            tx_count=tx_count,
            total_chunks=total_chunks,
            rbc_bytes_total=rbc_bytes_total,
            teu_total=teu_total,
            block_hash=block_hash,
        )


@dataclass(frozen=True)
class SumeragiLaneSettlementReceipt:
    """Receipt entry bundled in a lane settlement commitment."""

    source_id: str
    local_amount_micro: int
    xor_due_micro: int
    xor_after_haircut_micro: int
    xor_variance_micro: int
    timestamp_ms: int


@dataclass(frozen=True)
class SumeragiLaneSwapMetadata:
    """Swap metadata attached to a lane settlement commitment."""

    epsilon_bps: int
    twap_window_seconds: int
    liquidity_profile: str
    twap_local_per_xor: str
    volatility_class: str


@dataclass(frozen=True)
class SumeragiLaneSettlementCommitment:
    """Lane settlement totals and receipts bundled into sumeragi status."""

    block_height: int
    lane_id: int
    dataspace_id: int
    tx_count: int
    total_local_micro: int
    total_xor_due_micro: int
    total_xor_after_haircut_micro: int
    total_xor_variance_micro: int
    receipts: List[SumeragiLaneSettlementReceipt]
    swap_metadata: Optional[SumeragiLaneSwapMetadata]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiLaneSettlementCommitment":
        if not isinstance(payload, Mapping):
            raise TypeError("lane settlement commitment must be an object")
        try:
            block_height = int(payload.get("block_height", 0))
            lane_id = int(payload.get("lane_id", 0))
            dataspace_id = int(payload.get("dataspace_id", 0))
            tx_count = int(payload.get("tx_count", 0))
            total_local_micro = int(payload.get("total_local_micro", 0))
            total_xor_due_micro = int(payload.get("total_xor_due_micro", 0))
            total_xor_after_haircut_micro = int(payload.get("total_xor_after_haircut_micro", 0))
            total_xor_variance_micro = int(payload.get("total_xor_variance_micro", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("lane settlement numeric fields must be numeric") from exc
        receipts_payload = payload.get("receipts", [])
        if not isinstance(receipts_payload, list):
            raise TypeError("lane settlement `receipts` must be a list")
        receipts: List[SumeragiLaneSettlementReceipt] = []
        for index, receipt in enumerate(receipts_payload):
            if not isinstance(receipt, Mapping):
                raise TypeError("lane settlement receipts must be objects")
            source_id = receipt.get("source_id")
            if not isinstance(source_id, str):
                raise TypeError("lane settlement receipt `source_id` must be a string")
            try:
                receipt_local = int(receipt.get("local_amount_micro", 0))
                receipt_due = int(receipt.get("xor_due_micro", 0))
                receipt_after = int(receipt.get("xor_after_haircut_micro", 0))
                receipt_variance = int(receipt.get("xor_variance_micro", 0))
                receipt_timestamp = int(receipt.get("timestamp_ms", 0))
            except (TypeError, ValueError) as exc:
                raise TypeError(
                    f"lane settlement receipt numeric fields must be numeric (index {index})"
                ) from exc
            receipts.append(
                SumeragiLaneSettlementReceipt(
                    source_id=source_id,
                    local_amount_micro=receipt_local,
                    xor_due_micro=receipt_due,
                    xor_after_haircut_micro=receipt_after,
                    xor_variance_micro=receipt_variance,
                    timestamp_ms=receipt_timestamp,
                )
            )
        swap_metadata_payload = payload.get("swap_metadata")
        swap_metadata: Optional[SumeragiLaneSwapMetadata]
        if swap_metadata_payload is None:
            swap_metadata = None
        elif isinstance(swap_metadata_payload, Mapping):
            try:
                swap_metadata = SumeragiLaneSwapMetadata(
                    epsilon_bps=int(swap_metadata_payload.get("epsilon_bps", 0)),
                    twap_window_seconds=int(swap_metadata_payload.get("twap_window_seconds", 0)),
                    liquidity_profile=str(swap_metadata_payload.get("liquidity_profile", "")),
                    twap_local_per_xor=str(swap_metadata_payload.get("twap_local_per_xor", "")),
                    volatility_class=str(swap_metadata_payload.get("volatility_class", "")),
                )
            except (TypeError, ValueError) as exc:
                raise TypeError("lane settlement `swap_metadata` numeric fields must be numeric") from exc
        else:
            raise TypeError("lane settlement `swap_metadata` must be an object when present")
        return cls(
            block_height=block_height,
            lane_id=lane_id,
            dataspace_id=dataspace_id,
            tx_count=tx_count,
            total_local_micro=total_local_micro,
            total_xor_due_micro=total_xor_due_micro,
            total_xor_after_haircut_micro=total_xor_after_haircut_micro,
            total_xor_variance_micro=total_xor_variance_micro,
            receipts=receipts,
            swap_metadata=swap_metadata,
        )


@dataclass(frozen=True)
class SumeragiLaneRelayEnvelope:
    """Relay envelope capturing block header, DA hash, and settlement commitment."""

    lane_id: int
    dataspace_id: int
    block_height: int
    block_header: Mapping[str, Any]
    execution_qc: Optional[Mapping[str, Any]]
    da_commitment_hash: Optional[str]
    settlement_commitment: SumeragiLaneSettlementCommitment
    settlement_hash: str
    rbc_bytes_total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiLaneRelayEnvelope":
        if not isinstance(payload, Mapping):
            raise TypeError("lane relay envelope must be an object")
        try:
            lane_id = int(payload.get("lane_id", 0))
            dataspace_id = int(payload.get("dataspace_id", 0))
            block_height = int(payload.get("block_height", 0))
            rbc_bytes_total = int(payload.get("rbc_bytes_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("lane relay numeric fields must be numeric") from exc
        block_header = payload.get("block_header")
        if not isinstance(block_header, Mapping):
            raise TypeError("lane relay `block_header` must be an object")
        execution_qc = payload.get("execution_qc")
        if execution_qc is not None and not isinstance(execution_qc, Mapping):
            raise TypeError("lane relay `execution_qc` must be an object when present")
        da_commitment_hash = payload.get("da_commitment_hash")
        if da_commitment_hash is not None and not isinstance(da_commitment_hash, str):
            raise TypeError("lane relay `da_commitment_hash` must be a string when present")
        settlement_hash = payload.get("settlement_hash")
        if not isinstance(settlement_hash, str) or settlement_hash == "":
            raise TypeError("lane relay `settlement_hash` must be a non-empty string")
        settlement_payload = payload.get("settlement_commitment")
        if not isinstance(settlement_payload, Mapping):
            raise TypeError("lane relay `settlement_commitment` must be an object")
        settlement_commitment = SumeragiLaneSettlementCommitment.from_payload(settlement_payload)
        return cls(
            lane_id=lane_id,
            dataspace_id=dataspace_id,
            block_height=block_height,
            block_header=block_header,
            execution_qc=execution_qc,
            da_commitment_hash=da_commitment_hash,
            settlement_commitment=settlement_commitment,
            settlement_hash=settlement_hash,
            rbc_bytes_total=rbc_bytes_total,
        )


@dataclass(frozen=True)
class SumeragiLaneGovernanceSnapshot:
    """Governance snapshot for a lane as exposed by sumeragi status."""

    lane_id: int
    alias: str
    governance: Optional[str]
    manifest_required: bool
    manifest_ready: bool
    manifest_path: Optional[str]
    validator_ids: List[str]
    quorum: Optional[int]
    protected_namespaces: List[str]
    runtime_upgrade: Optional["ToriiLaneRuntimeUpgradeHookSnapshot"]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiLaneGovernanceSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("lane governance entry must be an object")
        alias = payload.get("alias")
        if not isinstance(alias, str):
            raise TypeError("lane governance `alias` must be a string")
        governance = payload.get("governance")
        if governance is not None and not isinstance(governance, str):
            raise TypeError("lane governance `governance` must be a string when present")
        try:
            lane_id = int(payload.get("lane_id", 0))
            manifest_required = bool(payload.get("manifest_required", False))
            manifest_ready = bool(payload.get("manifest_ready", False))
            quorum_value = payload.get("quorum")
            quorum = None if quorum_value is None else int(quorum_value)
        except (TypeError, ValueError) as exc:
            raise TypeError("lane governance numeric fields must be numeric when present") from exc
        manifest_path = payload.get("manifest_path")
        if manifest_path is not None and not isinstance(manifest_path, str):
            raise TypeError("lane governance `manifest_path` must be a string when present")
        validator_ids_raw = payload.get("validator_ids", [])
        if not isinstance(validator_ids_raw, list):
            raise TypeError("lane governance `validator_ids` must be a list")
        validator_ids: List[str] = []
        for entry in validator_ids_raw:
            if not isinstance(entry, str):
                raise TypeError("lane governance validator ids must be strings")
            validator_ids.append(entry)
        protected_namespaces_raw = payload.get("protected_namespaces", [])
        if not isinstance(protected_namespaces_raw, list):
            raise TypeError("lane governance `protected_namespaces` must be a list")
        protected_namespaces: List[str] = []
        for ns in protected_namespaces_raw:
            if not isinstance(ns, str):
                raise TypeError("lane governance protected namespaces must be strings")
            protected_namespaces.append(ns)
        runtime_upgrade_payload = payload.get("runtime_upgrade")
        runtime_upgrade: Optional["ToriiLaneRuntimeUpgradeHookSnapshot"]
        if runtime_upgrade_payload is None:
            runtime_upgrade = None
        elif isinstance(runtime_upgrade_payload, Mapping):
            runtime_upgrade = ToriiLaneRuntimeUpgradeHookSnapshot(
                allow=bool(runtime_upgrade_payload.get("allow", False)),
                require_metadata=bool(runtime_upgrade_payload.get("require_metadata", False)),
                metadata_key=runtime_upgrade_payload.get("metadata_key"),
                allowed_ids=list(runtime_upgrade_payload.get("allowed_ids", [])),
            )
        else:
            raise TypeError("lane governance `runtime_upgrade` must be an object when present")
        return cls(
            lane_id=lane_id,
            alias=alias,
            governance=governance,
            manifest_required=manifest_required,
            manifest_ready=manifest_ready,
            manifest_path=manifest_path,
            validator_ids=validator_ids,
            quorum=quorum,
            protected_namespaces=protected_namespaces,
            runtime_upgrade=runtime_upgrade,
        )


@dataclass(frozen=True)
class SumeragiStatusSnapshot:
    """Structured snapshot returned by `/v1/sumeragi/status`."""

    leader_index: int
    view_change_index: int
    view_change_proof_accepted_total: int
    view_change_proof_stale_total: int
    view_change_proof_rejected_total: int
    view_change_suggest_total: int
    view_change_install_total: int
    highest_qc: SumeragiQcSummary
    locked_qc: SumeragiQcSummary
    tx_queue: SumeragiTxQueueStatus
    epoch: SumeragiEpochSchedule
    gossip_fallback_total: int
    block_created_dropped_by_lock_total: int
    block_created_hint_mismatch_total: int
    block_created_proposal_mismatch_total: int
    pacemaker_backpressure_deferrals_total: int
    da_reschedule_total: int
    rbc_store: SumeragiRbcStoreStatus
    prf: SumeragiPrfStatus
    membership: SumeragiMembershipStatus
    vrf_penalty_epoch: Optional[int]
    vrf_committed_no_reveal_total: int
    vrf_no_participation_total: int
    vrf_late_reveals_total: int
    collectors_targeted_current: int
    collectors_targeted_last_per_block: Optional[int]
    redundant_sends_total: int
    lane_commitments: List[SumeragiLaneCommitment]
    dataspace_commitments: List[SumeragiDataspaceCommitment]
    lane_settlement_commitments: List[SumeragiLaneSettlementCommitment]
    lane_relay_envelopes: List[SumeragiLaneRelayEnvelope]
    lane_governance_sealed_total: int
    lane_governance_sealed_aliases: List[str]
    lane_governance: List[SumeragiLaneGovernanceSnapshot]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiStatusSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi status payload must be an object")
        try:
            leader_index = int(payload.get("leader_index", 0))
            view_change_index = int(payload.get("view_change_index", 0))
            view_change_proof_accepted_total = int(
                payload.get("view_change_proof_accepted_total", 0)
            )
            view_change_proof_stale_total = int(
                payload.get("view_change_proof_stale_total", 0)
            )
            view_change_proof_rejected_total = int(
                payload.get("view_change_proof_rejected_total", 0)
            )
            view_change_suggest_total = int(payload.get("view_change_suggest_total", 0))
            view_change_install_total = int(payload.get("view_change_install_total", 0))
            gossip_fallback_total = int(payload.get("gossip_fallback_total", 0))
            block_drop_total = int(payload.get("block_created_dropped_by_lock_total", 0))
            block_hint_total = int(payload.get("block_created_hint_mismatch_total", 0))
            block_proposal_total = int(payload.get("block_created_proposal_mismatch_total", 0))
            pacemaker_deferrals = int(payload.get("pacemaker_backpressure_deferrals_total", 0))
            da_reschedule_total = int(payload.get("da_reschedule_total", 0))
            vrf_committed_no_reveal_total = int(payload.get("vrf_committed_no_reveal_total", 0))
            vrf_no_participation_total = int(payload.get("vrf_no_participation_total", 0))
            vrf_late_reveals_total = int(payload.get("vrf_late_reveals_total", 0))
            collectors_targeted_current = int(payload.get("collectors_targeted_current", 0))
            redundant_sends_total = int(payload.get("redundant_sends_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi status counters must be numeric") from exc
        highest_qc_payload = payload.get("highest_qc")
        locked_qc_payload = payload.get("locked_qc")
        tx_queue_payload = payload.get("tx_queue")
        epoch_payload = payload.get("epoch")
        rbc_store_payload = payload.get("rbc_store")
        prf_payload = payload.get("prf")
        membership_payload = payload.get("membership")
        for field_name, field_payload in [
            ("highest_qc", highest_qc_payload),
            ("locked_qc", locked_qc_payload),
            ("tx_queue", tx_queue_payload),
            ("epoch", epoch_payload),
            ("rbc_store", rbc_store_payload),
            ("prf", prf_payload),
            ("membership", membership_payload),
        ]:
            if not isinstance(field_payload, Mapping):
                raise TypeError(f"sumeragi status missing object `{field_name}` field")
        lane_commitments_payload = payload.get("lane_commitments", [])
        if not isinstance(lane_commitments_payload, list):
            raise TypeError("sumeragi status `lane_commitments` must be a list")
        lane_commitments = [
            SumeragiLaneCommitment.from_payload(entry) for entry in lane_commitments_payload
        ]
        dataspace_commitments_payload = payload.get("dataspace_commitments", [])
        if not isinstance(dataspace_commitments_payload, list):
            raise TypeError("sumeragi status `dataspace_commitments` must be a list")
        dataspace_commitments = [
            SumeragiDataspaceCommitment.from_payload(entry)
            for entry in dataspace_commitments_payload
        ]
        lane_settlement_payload = payload.get("lane_settlement_commitments", [])
        if not isinstance(lane_settlement_payload, list):
            raise TypeError("sumeragi status `lane_settlement_commitments` must be a list")
        lane_settlement_commitments = [
            SumeragiLaneSettlementCommitment.from_payload(entry)
            for entry in lane_settlement_payload
        ]
        lane_relay_payload = payload.get("lane_relay_envelopes", [])
        if not isinstance(lane_relay_payload, list):
            raise TypeError("sumeragi status `lane_relay_envelopes` must be a list")
        lane_relay_envelopes = [
            SumeragiLaneRelayEnvelope.from_payload(entry) for entry in lane_relay_payload
        ]
        try:
            lane_governance_sealed_total = int(payload.get("lane_governance_sealed_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi status `lane_governance_sealed_total` must be numeric") from exc
        lane_governance_sealed_aliases_payload = payload.get("lane_governance_sealed_aliases", [])
        if not isinstance(lane_governance_sealed_aliases_payload, list):
            raise TypeError("sumeragi status `lane_governance_sealed_aliases` must be a list")
        lane_governance_sealed_aliases: List[str] = []
        for alias in lane_governance_sealed_aliases_payload:
            if not isinstance(alias, str):
                raise TypeError("lane governance sealed aliases must be strings")
            lane_governance_sealed_aliases.append(alias)
        lane_governance_payload = payload.get("lane_governance", [])
        if not isinstance(lane_governance_payload, list):
            raise TypeError("sumeragi status `lane_governance` must be a list")
        lane_governance = [
            SumeragiLaneGovernanceSnapshot.from_payload(entry) for entry in lane_governance_payload
        ]
        vrf_penalty_epoch_val = payload.get("vrf_penalty_epoch")
        try:
            vrf_penalty_epoch = None if vrf_penalty_epoch_val is None else int(vrf_penalty_epoch_val)
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi status `vrf_penalty_epoch` must be numeric when present") from exc
        collectors_last_val = payload.get("collectors_targeted_last_per_block")
        try:
            collectors_targeted_last_per_block = (
                None if collectors_last_val is None else int(collectors_last_val)
            )
        except (TypeError, ValueError) as exc:
            raise TypeError(
                "sumeragi status `collectors_targeted_last_per_block` must be numeric when present"
            ) from exc
        return cls(
            leader_index=leader_index,
            view_change_index=view_change_index,
            view_change_proof_accepted_total=view_change_proof_accepted_total,
            view_change_proof_stale_total=view_change_proof_stale_total,
            view_change_proof_rejected_total=view_change_proof_rejected_total,
            view_change_suggest_total=view_change_suggest_total,
            view_change_install_total=view_change_install_total,
            highest_qc=SumeragiQcSummary.from_payload(highest_qc_payload),
            locked_qc=SumeragiQcSummary.from_payload(locked_qc_payload),
            tx_queue=SumeragiTxQueueStatus.from_payload(tx_queue_payload),
            epoch=SumeragiEpochSchedule.from_payload(epoch_payload),
            gossip_fallback_total=gossip_fallback_total,
            block_created_dropped_by_lock_total=block_drop_total,
            block_created_hint_mismatch_total=block_hint_total,
            block_created_proposal_mismatch_total=block_proposal_total,
            pacemaker_backpressure_deferrals_total=pacemaker_deferrals,
            da_reschedule_total=da_reschedule_total,
            rbc_store=SumeragiRbcStoreStatus.from_payload(rbc_store_payload),
            prf=SumeragiPrfStatus.from_payload(prf_payload),
            membership=SumeragiMembershipStatus.from_payload(membership_payload),
            vrf_penalty_epoch=vrf_penalty_epoch,
            vrf_committed_no_reveal_total=vrf_committed_no_reveal_total,
            vrf_no_participation_total=vrf_no_participation_total,
            vrf_late_reveals_total=vrf_late_reveals_total,
            collectors_targeted_current=collectors_targeted_current,
            collectors_targeted_last_per_block=collectors_targeted_last_per_block,
            redundant_sends_total=redundant_sends_total,
            lane_commitments=lane_commitments,
            dataspace_commitments=dataspace_commitments,
            lane_settlement_commitments=lane_settlement_commitments,
            lane_relay_envelopes=lane_relay_envelopes,
            lane_governance_sealed_total=lane_governance_sealed_total,
            lane_governance_sealed_aliases=lane_governance_sealed_aliases,
            lane_governance=lane_governance,
        )


@dataclass(frozen=True)
class SumeragiNewViewReceipt:
    """NEW_VIEW receipt counts per (height, view)."""

    height: int
    view: int
    count: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiNewViewReceipt":
        if not isinstance(payload, Mapping):
            raise TypeError("new_view entry must be an object")
        try:
            height = int(payload.get("height"))
            view = int(payload.get("view"))
            count = int(payload.get("count", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("new_view entry fields must be numeric") from exc
        return cls(height=height, view=view, count=count)


@dataclass(frozen=True)
class SumeragiNewViewSnapshot:
    """Snapshot returned by `/v1/sumeragi/new_view`."""

    ts_ms: int
    items: List[SumeragiNewViewReceipt]
    locked_qc: Optional[SumeragiQcSummary]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiNewViewSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("new_view payload must be an object")
        try:
            ts_ms = int(payload.get("ts_ms", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("new_view `ts_ms` must be numeric") from exc
        raw_items = payload.get("items", [])
        if raw_items is None:
            items_payload: List[Any] = []
        elif isinstance(raw_items, list):
            items_payload = raw_items
        else:
            raise TypeError("new_view `items` must be a list when present")
        items = [SumeragiNewViewReceipt.from_payload(entry) for entry in items_payload]
        locked_payload = payload.get("locked_qc")
        locked_qc: Optional[SumeragiQcSummary]
        if locked_payload is None:
            locked_qc = None
        elif isinstance(locked_payload, Mapping):
            locked_qc = SumeragiQcSummary.from_payload(locked_payload)
        else:
            raise TypeError("new_view `locked_qc` must be an object when present")
        return cls(ts_ms=ts_ms, items=items, locked_qc=locked_qc)


@dataclass(frozen=True)
class SumeragiRbcSnapshot:
    """Aggregate RBC metrics from `/v1/sumeragi/rbc`."""

    sessions_active: int
    sessions_pruned_total: int
    ready_broadcasts_total: int
    deliver_broadcasts_total: int
    payload_bytes_delivered_total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("rbc snapshot payload must be an object")
        try:
            sessions_active = int(payload.get("sessions_active", 0))
            sessions_pruned_total = int(payload.get("sessions_pruned_total", 0))
            ready_broadcasts_total = int(payload.get("ready_broadcasts_total", 0))
            deliver_broadcasts_total = int(payload.get("deliver_broadcasts_total", 0))
            payload_bytes_delivered_total = int(payload.get("payload_bytes_delivered_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("rbc snapshot counters must be numeric") from exc
        return cls(
            sessions_active=sessions_active,
            sessions_pruned_total=sessions_pruned_total,
            ready_broadcasts_total=ready_broadcasts_total,
            deliver_broadcasts_total=deliver_broadcasts_total,
            payload_bytes_delivered_total=payload_bytes_delivered_total,
        )


@dataclass(frozen=True)
class SumeragiRbcSession:
    """Per-session RBC state from `/v1/sumeragi/rbc/sessions`."""

    block_hash: Optional[str]
    height: int
    view: int
    total_chunks: int
    received_chunks: int
    ready_count: int
    delivered: bool
    invalid: bool
    payload_hash: Optional[str]
    recovered: bool

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcSession":
        if not isinstance(payload, Mapping):
            raise TypeError("rbc session entry must be an object")
        block_hash = payload.get("block_hash")
        if block_hash is not None and not isinstance(block_hash, str):
            raise TypeError("rbc session `block_hash` must be a string when present")
        payload_hash = payload.get("payload_hash")
        if payload_hash is not None and not isinstance(payload_hash, str):
            raise TypeError("rbc session `payload_hash` must be a string when present")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
            total_chunks = int(payload.get("total_chunks", 0))
            received_chunks = int(payload.get("received_chunks", 0))
            ready_count = int(payload.get("ready_count", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("rbc session counters must be numeric") from exc
        delivered = payload.get("delivered")
        invalid = payload.get("invalid")
        recovered = payload.get("recovered")
        for name, flag in [
            ("delivered", delivered),
            ("invalid", invalid),
            ("recovered", recovered),
        ]:
            if not isinstance(flag, bool):
                raise TypeError(f"rbc session `{name}` field must be boolean")
        return cls(
            block_hash=block_hash,
            height=height,
            view=view,
            total_chunks=total_chunks,
            received_chunks=received_chunks,
            ready_count=ready_count,
            delivered=delivered,
            invalid=invalid,
            payload_hash=payload_hash,
            recovered=recovered,
        )


@dataclass(frozen=True)
class SumeragiRbcSessionsSnapshot:
    """Snapshot of all active RBC sessions."""

    sessions_active: int
    items: List[SumeragiRbcSession]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcSessionsSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("rbc sessions payload must be an object")
        try:
            sessions_active = int(payload.get("sessions_active", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("rbc sessions `sessions_active` must be numeric") from exc
        raw_items = payload.get("items", [])
        if raw_items is None:
            items_payload: List[Any] = []
        elif isinstance(raw_items, list):
            items_payload = raw_items
        else:
            raise TypeError("rbc sessions `items` must be a list when present")
        items = [SumeragiRbcSession.from_payload(entry) for entry in items_payload]
        return cls(sessions_active=sessions_active, items=items)


@dataclass(frozen=True)
class SumeragiRbcDeliveryStatus:
    """Delivery status for a specific RBC session (`/v1/sumeragi/rbc/delivered/{height}/{view}`)."""

    height: int
    view: int
    delivered: bool
    present: bool
    block_hash: Optional[str]
    ready_count: int
    received_chunks: int
    total_chunks: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcDeliveryStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("rbc delivered payload must be an object")
        block_hash = payload.get("block_hash")
        if block_hash is not None and not isinstance(block_hash, str):
            raise TypeError("rbc delivered `block_hash` must be a string when present")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
            ready_count = int(payload.get("ready_count", 0))
            received_chunks = int(payload.get("received_chunks", 0))
            total_chunks = int(payload.get("total_chunks", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("rbc delivered counters must be numeric") from exc
        delivered = payload.get("delivered")
        present = payload.get("present")
        if not isinstance(delivered, bool) or not isinstance(present, bool):
            raise TypeError("rbc delivered flags must be boolean")
        return cls(
            height=height,
            view=view,
            delivered=delivered,
            present=present,
            block_hash=block_hash,
            ready_count=ready_count,
            received_chunks=received_chunks,
            total_chunks=total_chunks,
        )


@dataclass(frozen=True)
class SumeragiCollectorEntry:
    """Collector index/peer pair used in deterministic collector plans."""

    index: int
    peer_id: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiCollectorEntry":
        if not isinstance(payload, Mapping):
            raise TypeError("collector entry must be an object")
        peer_id = payload.get("peer_id")
        if not isinstance(peer_id, str):
            raise TypeError("collector entry missing string `peer_id` field")
        try:
            index = int(payload.get("index", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("collector entry `index` must be numeric") from exc
        return cls(index=index, peer_id=peer_id)


@dataclass(frozen=True)
class SumeragiCollectorPlan:
    """Deterministic collector plan exposed by `/v1/sumeragi/collectors`."""

    consensus_mode: str
    mode: str
    topology_len: int
    min_votes_for_commit: int
    proxy_tail_index: int
    height: int
    view: int
    collectors_k: int
    redundant_send_r: int
    epoch_seed: Optional[str]
    collectors: List[SumeragiCollectorEntry]
    prf: SumeragiPrfStatus

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiCollectorPlan":
        if not isinstance(payload, Mapping):
            raise TypeError("collector plan payload must be an object")
        consensus_mode = payload.get("consensus_mode")
        mode = payload.get("mode")
        if not isinstance(consensus_mode, str) or not isinstance(mode, str):
            raise TypeError("collector plan `consensus_mode` and `mode` must be strings")
        epoch_seed_value = payload.get("epoch_seed")
        if epoch_seed_value is not None and not isinstance(epoch_seed_value, str):
            raise TypeError("collector plan `epoch_seed` must be a string when present")
        collectors_raw = payload.get("collectors", [])
        if collectors_raw is None:
            collectors_payload: List[Any] = []
        elif isinstance(collectors_raw, list):
            collectors_payload = collectors_raw
        else:
            raise TypeError("collector plan `collectors` must be a list when present")
        prf_payload = payload.get("prf")
        if not isinstance(prf_payload, Mapping):
            raise TypeError("collector plan missing object `prf` field")
        try:
            topology_len = int(payload.get("topology_len", 0))
            min_votes_for_commit = int(payload.get("min_votes_for_commit", 0))
            proxy_tail_index = int(payload.get("proxy_tail_index", 0))
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
            collectors_k = int(payload.get("collectors_k", 0))
            redundant_send_r = int(payload.get("redundant_send_r", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("collector plan counters must be numeric") from exc
        collectors = [SumeragiCollectorEntry.from_payload(entry) for entry in collectors_payload]
        return cls(
            consensus_mode=consensus_mode,
            mode=mode,
            topology_len=topology_len,
            min_votes_for_commit=min_votes_for_commit,
            proxy_tail_index=proxy_tail_index,
            height=height,
            view=view,
            collectors_k=collectors_k,
            redundant_send_r=redundant_send_r,
            epoch_seed=epoch_seed_value,
            collectors=collectors,
            prf=SumeragiPrfStatus.from_payload(prf_payload),
        )


@dataclass(frozen=True)
class SumeragiParamsSnapshot:
    """On-chain Sumeragi parameter snapshot from `/v1/sumeragi/params`."""

    block_time_ms: int
    commit_time_ms: int
    max_clock_drift_ms: int
    collectors_k: int
    redundant_send_r: int
    da_enabled: bool
    next_mode: Optional[str]
    mode_activation_height: Optional[int]
    chain_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiParamsSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi params payload must be an object")
        next_mode_value = payload.get("next_mode")
        if next_mode_value is not None and not isinstance(next_mode_value, str):
            raise TypeError("sumeragi params `next_mode` must be a string when present")
        mode_activation_value = payload.get("mode_activation_height")
        try:
            block_time_ms = int(payload.get("block_time_ms", 0))
            commit_time_ms = int(payload.get("commit_time_ms", 0))
            max_clock_drift_ms = int(payload.get("max_clock_drift_ms", 0))
            collectors_k = int(payload.get("collectors_k", 0))
            redundant_send_r = int(payload.get("redundant_send_r", 0))
            chain_height = int(payload.get("chain_height", 0))
            mode_activation_height = (
                None if mode_activation_value is None else int(mode_activation_value)
            )
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi params numeric fields must be integers") from exc
        da_enabled = payload.get("da_enabled")
        if not isinstance(da_enabled, bool):
            raise TypeError("sumeragi params `da_enabled` must be a boolean")
        return cls(
            block_time_ms=block_time_ms,
            commit_time_ms=commit_time_ms,
            max_clock_drift_ms=max_clock_drift_ms,
            collectors_k=collectors_k,
            redundant_send_r=redundant_send_r,
            da_enabled=da_enabled,
            next_mode=next_mode_value,
            mode_activation_height=mode_activation_height,
            chain_height=chain_height,
        )


@dataclass(frozen=True)
class SumeragiPacemakerSnapshot:
    """Pacemaker runtime metrics from `/v1/sumeragi/pacemaker`."""

    backoff_ms: int
    rtt_floor_ms: int
    jitter_ms: int
    backoff_multiplier: int
    rtt_floor_multiplier: int
    max_backoff_ms: int
    jitter_frac_permille: int
    round_elapsed_ms: int
    view_timeout_target_ms: int
    view_timeout_remaining_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiPacemakerSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("pacemaker payload must be an object")
        try:
            backoff_ms = int(payload.get("backoff_ms", 0))
            rtt_floor_ms = int(payload.get("rtt_floor_ms", 0))
            jitter_ms = int(payload.get("jitter_ms", 0))
            backoff_multiplier = int(payload.get("backoff_multiplier", 0))
            rtt_floor_multiplier = int(payload.get("rtt_floor_multiplier", 0))
            max_backoff_ms = int(payload.get("max_backoff_ms", 0))
            jitter_frac_permille = int(payload.get("jitter_frac_permille", 0))
            round_elapsed_ms = int(payload.get("round_elapsed_ms", 0))
            view_timeout_target_ms = int(payload.get("view_timeout_target_ms", 0))
            view_timeout_remaining_ms = int(payload.get("view_timeout_remaining_ms", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("pacemaker metrics must be numeric") from exc
        return cls(
            backoff_ms=backoff_ms,
            rtt_floor_ms=rtt_floor_ms,
            jitter_ms=jitter_ms,
            backoff_multiplier=backoff_multiplier,
            rtt_floor_multiplier=rtt_floor_multiplier,
            max_backoff_ms=max_backoff_ms,
            jitter_frac_permille=jitter_frac_permille,
            round_elapsed_ms=round_elapsed_ms,
            view_timeout_target_ms=view_timeout_target_ms,
            view_timeout_remaining_ms=view_timeout_remaining_ms,
        )


@dataclass(frozen=True)
class SumeragiPhasesEmaSnapshot:
    """Exponential moving average latency per phase."""

    propose_ms: int
    collect_da_ms: int
    collect_prevote_ms: int
    collect_precommit_ms: int
    collect_aggregator_ms: int
    collect_exec_ms: int
    collect_witness_ms: int
    commit_ms: int
    pipeline_total_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiPhasesEmaSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("phases EMA payload must be an object")
        try:
            return cls(
                propose_ms=int(payload.get("propose_ms", 0)),
                collect_da_ms=int(payload.get("collect_da_ms", 0)),
                collect_prevote_ms=int(payload.get("collect_prevote_ms", 0)),
                collect_precommit_ms=int(payload.get("collect_precommit_ms", 0)),
                collect_aggregator_ms=int(payload.get("collect_aggregator_ms", 0)),
                collect_exec_ms=int(payload.get("collect_exec_ms", 0)),
                collect_witness_ms=int(payload.get("collect_witness_ms", 0)),
                commit_ms=int(payload.get("commit_ms", 0)),
                pipeline_total_ms=int(payload.get("pipeline_total_ms", 0)),
            )
        except (TypeError, ValueError) as exc:
            raise TypeError("phases EMA metrics must be numeric") from exc


@dataclass(frozen=True)
class SumeragiPhasesSnapshot:
    """Per-phase latency summary from `/v1/sumeragi/phases`."""

    propose_ms: int
    collect_da_ms: int
    collect_prevote_ms: int
    collect_precommit_ms: int
    collect_aggregator_ms: int
    collect_exec_ms: int
    collect_witness_ms: int
    commit_ms: int
    pipeline_total_ms: int
    collect_aggregator_gossip_total: int
    block_created_dropped_by_lock_total: int
    block_created_hint_mismatch_total: int
    block_created_proposal_mismatch_total: int
    ema_ms: SumeragiPhasesEmaSnapshot

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiPhasesSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("phases payload must be an object")
        ema_payload = payload.get("ema_ms")
        if not isinstance(ema_payload, Mapping):
            raise TypeError("phases payload missing object `ema_ms` field")
        try:
            return cls(
                propose_ms=int(payload.get("propose_ms", 0)),
                collect_da_ms=int(payload.get("collect_da_ms", 0)),
                collect_prevote_ms=int(payload.get("collect_prevote_ms", 0)),
                collect_precommit_ms=int(payload.get("collect_precommit_ms", 0)),
                collect_aggregator_ms=int(payload.get("collect_aggregator_ms", 0)),
                collect_exec_ms=int(payload.get("collect_exec_ms", 0)),
                collect_witness_ms=int(payload.get("collect_witness_ms", 0)),
                commit_ms=int(payload.get("commit_ms", 0)),
                pipeline_total_ms=int(payload.get("pipeline_total_ms", 0)),
                collect_aggregator_gossip_total=int(
                    payload.get("collect_aggregator_gossip_total", 0)
                ),
                block_created_dropped_by_lock_total=int(
                    payload.get("block_created_dropped_by_lock_total", 0)
                ),
                block_created_hint_mismatch_total=int(
                    payload.get("block_created_hint_mismatch_total", 0)
                ),
                block_created_proposal_mismatch_total=int(
                    payload.get("block_created_proposal_mismatch_total", 0)
                ),
                ema_ms=SumeragiPhasesEmaSnapshot.from_payload(ema_payload),
            )
        except (TypeError, ValueError) as exc:
            raise TypeError("phases metrics must be numeric") from exc


@dataclass(frozen=True)
class SumeragiLeaderSnapshot:
    """Leader index snapshot from `/v1/sumeragi/leader`."""

    leader_index: int
    prf: SumeragiPrfStatus

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiLeaderSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("leader payload must be an object")
        prf_payload = payload.get("prf")
        if not isinstance(prf_payload, Mapping):
            raise TypeError("leader payload missing object `prf` field")
        try:
            leader_index = int(payload.get("leader_index", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("leader index must be numeric") from exc
        return cls(leader_index=leader_index, prf=SumeragiPrfStatus.from_payload(prf_payload))


@dataclass(frozen=True)
class SumeragiQcSnapshot:
    """Highest/Locked QC snapshot from `/v1/sumeragi/qc`."""

    highest_qc: SumeragiQcSummary
    locked_qc: SumeragiQcSummary

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiQcSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("qc payload must be an object")
        highest_payload = payload.get("highest_qc")
        locked_payload = payload.get("locked_qc")
        if not isinstance(highest_payload, Mapping) or not isinstance(locked_payload, Mapping):
            raise TypeError("qc payload must contain object `highest_qc` and `locked_qc` fields")
        return cls(
            highest_qc=SumeragiQcSummary.from_payload(highest_payload),
            locked_qc=SumeragiQcSummary.from_payload(locked_payload),
        )


@dataclass(frozen=True)
class SumeragiEvidenceCount:
    """Evidence store size from `/v1/sumeragi/evidence/count`."""

    count: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiEvidenceCount":
        if not isinstance(payload, Mapping):
            raise TypeError("evidence count payload must be an object")
        try:
            count = int(payload.get("count", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("evidence count must be numeric") from exc
        return cls(count=count)


@dataclass(frozen=True)
class RbcMerkleProof:
    """Merkle proof for an RBC chunk sample."""

    leaf_index: int
    depth: Optional[int]
    audit_path: List[Optional[str]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RbcMerkleProof":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC merkle proof must be an object")
        try:
            leaf_index = int(payload.get("leaf_index", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC merkle proof `leaf_index` must be numeric") from exc
        depth_value = payload.get("depth")
        depth = None if depth_value is None else int(depth_value)
        path_value = payload.get("audit_path", [])
        if not isinstance(path_value, list):
            raise TypeError("RBC merkle proof `audit_path` must be a list")
        audit_path: List[Optional[str]] = []
        for item in path_value:
            if item is None:
                audit_path.append(None)
            elif isinstance(item, str):
                audit_path.append(item)
            else:
                raise TypeError("RBC merkle proof `audit_path` entries must be string or null")
        return cls(leaf_index=leaf_index, depth=depth, audit_path=audit_path)


@dataclass(frozen=True)
class RbcChunkProof:
    """Proof object for a sampled RBC chunk."""

    index: int
    chunk_hex: str
    digest_hex: str
    proof: RbcMerkleProof

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RbcChunkProof":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC chunk proof must be an object")
        try:
            index = int(payload.get("index", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC chunk proof `index` must be numeric") from exc
        chunk_hex = payload.get("chunk_hex")
        digest_hex = payload.get("digest_hex")
        if not isinstance(chunk_hex, str) or not isinstance(digest_hex, str):
            raise TypeError("RBC chunk proof `chunk_hex` and `digest_hex` must be strings")
        proof_payload = payload.get("proof")
        if not isinstance(proof_payload, Mapping):
            raise TypeError("RBC chunk proof missing `proof` object")
        proof = RbcMerkleProof.from_payload(proof_payload)
        return cls(index=index, chunk_hex=chunk_hex, digest_hex=digest_hex, proof=proof)


@dataclass(frozen=True)
class RbcSample:
    """Structured response returned by `/v1/sumeragi/rbc/sample`."""

    block_hash: str
    height: int
    view: int
    total_chunks: int
    chunk_root: str
    payload_hash: Optional[str]
    samples: List[RbcChunkProof]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RbcSample":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC sample payload must be an object")
        block_hash = payload.get("block_hash")
        chunk_root = payload.get("chunk_root")
        if not isinstance(block_hash, str) or not isinstance(chunk_root, str):
            raise TypeError("RBC sample requires string `block_hash` and `chunk_root`")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
            total_chunks = int(payload.get("total_chunks", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC sample numeric fields must be integers") from exc
        payload_hash_value = payload.get("payload_hash")
        if payload_hash_value is not None and not isinstance(payload_hash_value, str):
            raise TypeError("RBC sample `payload_hash` must be a string when provided")
        sample_payload = payload.get("samples", [])
        if not isinstance(sample_payload, list):
            raise TypeError("RBC sample `samples` must be a list")
        samples = [RbcChunkProof.from_payload(item) for item in sample_payload]
        return cls(
            block_hash=block_hash,
            height=height,
            view=view,
            total_chunks=total_chunks,
            chunk_root=chunk_root,
            payload_hash=payload_hash_value,
            samples=samples,
        )


@dataclass(frozen=True)
class RuntimeUpgradeCounters:
    """Lifecycle counters returned by `/v1/runtime/metrics`."""

    proposed: int
    activated: int
    canceled: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeCounters":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade counters payload must be an object")
        try:
            proposed = int(payload.get("proposed", 0))
            activated = int(payload.get("activated", 0))
            canceled = int(payload.get("canceled", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime upgrade counter values must be numeric") from exc
        return cls(proposed=proposed, activated=activated, canceled=canceled)


@dataclass(frozen=True)
class RuntimeMetrics:
    """Summary metrics for runtime upgrades."""

    active_abi_versions_count: int
    upgrade_events_total: RuntimeUpgradeCounters

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeMetrics":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime metrics payload must be an object")
        try:
            active_count = int(payload.get("active_abi_versions_count", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime metrics `active_abi_versions_count` must be numeric") from exc
        counters_payload = payload.get("upgrade_events_total", {})
        counters = RuntimeUpgradeCounters.from_payload(
            counters_payload if isinstance(counters_payload, Mapping) else {}
        )
        return cls(active_abi_versions_count=active_count, upgrade_events_total=counters)


@dataclass(frozen=True)
class RuntimeAbiActive:
    """Active ABI versions advertised by `/v1/runtime/abi/active`."""

    active_versions: List[int]
    default_compile_target: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeAbiActive":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime ABI active payload must be an object")
        versions = payload.get("active_versions")
        if not isinstance(versions, list):
            raise TypeError("runtime ABI active payload missing list `active_versions` field")
        active_versions: List[int] = []
        for item in versions:
            try:
                active_versions.append(int(item))
            except (TypeError, ValueError) as exc:
                raise TypeError("active_versions entries must be numeric") from exc
        try:
            default = int(payload["default_compile_target"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("runtime ABI active missing numeric `default_compile_target` field") from exc
        return cls(active_versions=active_versions, default_compile_target=default)


@dataclass(frozen=True)
class RuntimeAbiHash:
    """Canonical ABI hash summary from `/v1/runtime/abi/hash`."""

    policy: str
    abi_hash_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeAbiHash":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime ABI hash payload must be an object")
        policy = payload.get("policy")
        abi_hash_hex = payload.get("abi_hash_hex")
        if not isinstance(policy, str) or not isinstance(abi_hash_hex, str):
            raise TypeError("runtime ABI hash payload missing string `policy`/`abi_hash_hex` fields")
        return cls(policy=policy, abi_hash_hex=abi_hash_hex)


@dataclass(frozen=True)
class RuntimeUpgradeStatus:
    """Lifecycle status for a runtime upgrade record."""

    kind: str
    activated_height: Optional[int] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade status payload must be an object")
        if len(payload) != 1:
            raise TypeError("runtime upgrade status payload must contain exactly one variant")
        variant, value = next(iter(payload.items()))
        if variant == "Proposed":
            return cls(kind="Proposed")
        if variant == "Canceled":
            return cls(kind="Canceled")
        if variant == "ActivatedAt":
            if value is None:
                raise TypeError("runtime upgrade status `ActivatedAt` requires a height value")
            try:
                height = int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError("runtime upgrade status `ActivatedAt` height must be numeric") from exc
            if height < 0:
                raise ValueError("runtime upgrade status `ActivatedAt` height must be non-negative")
            return cls(kind="ActivatedAt", activated_height=height)
        raise TypeError(f"unknown runtime upgrade status variant `{variant}`")


def _coerce_int_list(values: Any, label: str) -> List[int]:
    if values is None:
        return []
    if not isinstance(values, list):
        raise TypeError(f"{label} must be a list")
    result: List[int] = []
    for entry in values:
        try:
            number = int(entry)
        except (TypeError, ValueError) as exc:
            raise TypeError(f"{label} entries must be integers") from exc
        if number < 0:
            raise ValueError(f"{label} entries must be non-negative")
        result.append(number)
    return result


@dataclass(frozen=True)
class RuntimeUpgradeManifest:
    """Runtime upgrade manifest advertised by `/v1/runtime/upgrades`."""

    name: str
    description: str
    abi_version: int
    abi_hash_hex: str
    added_syscalls: List[int]
    added_pointer_types: List[int]
    start_height: int
    end_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeManifest":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade manifest payload must be an object")
        name = payload.get("name")
        description = payload.get("description")
        if not isinstance(name, str) or not isinstance(description, str):
            raise TypeError("runtime upgrade manifest requires string `name` and `description`")
        abi_hash_hex = payload.get("abi_hash")
        if not isinstance(abi_hash_hex, str):
            raise TypeError("runtime upgrade manifest missing string `abi_hash` field")
        try:
            abi_version = int(payload.get("abi_version"))
            start_height = int(payload.get("start_height"))
            end_height = int(payload.get("end_height"))
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime upgrade manifest numeric fields must be integers") from exc
        added_syscalls = _coerce_int_list(
            payload.get("added_syscalls", []), "runtime upgrade manifest `added_syscalls`"
        )
        added_pointer_types = _coerce_int_list(
            payload.get("added_pointer_types", []), "runtime upgrade manifest `added_pointer_types`"
        )
        return cls(
            name=name,
            description=description,
            abi_version=abi_version,
            abi_hash_hex=abi_hash_hex,
            added_syscalls=added_syscalls,
            added_pointer_types=added_pointer_types,
            start_height=start_height,
            end_height=end_height,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Return a JSON-serialisable payload suitable for Torii POST requests."""

        return {
            "name": self.name,
            "description": self.description,
            "abi_version": self.abi_version,
            "abi_hash": self.abi_hash_hex,
            "added_syscalls": list(self.added_syscalls),
            "added_pointer_types": list(self.added_pointer_types),
            "start_height": self.start_height,
            "end_height": self.end_height,
        }


@dataclass(frozen=True)
class RuntimeUpgradeRecord:
    """Individual runtime upgrade record maintained by the node."""

    manifest: RuntimeUpgradeManifest
    status: RuntimeUpgradeStatus
    proposer: str
    created_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade record payload must be an object")
        manifest_payload = payload.get("manifest")
        status_payload = payload.get("status")
        if not isinstance(manifest_payload, Mapping):
            raise TypeError("runtime upgrade record missing object `manifest` field")
        if not isinstance(status_payload, Mapping):
            raise TypeError("runtime upgrade record missing object `status` field")
        proposer = payload.get("proposer")
        if not isinstance(proposer, str):
            raise TypeError("runtime upgrade record missing string `proposer` field")
        try:
            created_height = int(payload.get("created_height"))
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime upgrade record `created_height` must be numeric") from exc
        manifest = RuntimeUpgradeManifest.from_payload(manifest_payload)
        status = RuntimeUpgradeStatus.from_payload(status_payload)
        return cls(manifest=manifest, status=status, proposer=proposer, created_height=created_height)


@dataclass(frozen=True)
class RuntimeUpgradeListItem:
    """Entry returned from `/v1/runtime/upgrades`."""

    id_hex: str
    record: RuntimeUpgradeRecord

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeListItem":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade list entry must be an object")
        id_hex = payload.get("id_hex")
        if not isinstance(id_hex, str):
            raise TypeError("runtime upgrade list entry missing string `id_hex` field")
        record_payload = payload.get("record")
        if not isinstance(record_payload, Mapping):
            raise TypeError("runtime upgrade list entry missing object `record` field")
        record = RuntimeUpgradeRecord.from_payload(record_payload)
        return cls(id_hex=id_hex, record=record)


@dataclass(frozen=True)
class RuntimeUpgradeListPage:
    """Paginated runtime upgrade listing."""

    items: List[RuntimeUpgradeListItem]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrades response must be an object")
        items_raw = payload.get("items", [])
        if items_raw is None:
            items_raw = []
        if not isinstance(items_raw, list):
            raise TypeError("runtime upgrades response `items` must be a list")
        try:
            total = int(payload.get("total", len(items_raw)))
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime upgrades response `total` must be numeric") from exc
        items = [RuntimeUpgradeListItem.from_payload(entry) for entry in items_raw]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class RuntimeInstruction:
    """Instruction emitted by runtime upgrade helpers."""

    wire_id: str
    payload_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeInstruction":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime instruction payload must be an object")
        wire_id = payload.get("wire_id")
        payload_hex = payload.get("payload_hex")
        if not isinstance(wire_id, str) or not isinstance(payload_hex, str):
            raise TypeError("runtime instruction requires string `wire_id` and `payload_hex` fields")
        return cls(wire_id=wire_id, payload_hex=payload_hex)


@dataclass(frozen=True)
class RuntimeUpgradeActionResponse:
    """Response returned by runtime upgrade proposal/activation helpers."""

    ok: bool
    tx_instructions: List[RuntimeInstruction]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeActionResponse":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade action response must be an object")
        ok_value = payload.get("ok")
        if not isinstance(ok_value, bool):
            raise TypeError("runtime upgrade action response missing boolean `ok` field")
        instructions_payload = payload.get("tx_instructions", [])
        if instructions_payload is None:
            instructions_payload = []
        if not isinstance(instructions_payload, list):
            raise TypeError("runtime upgrade action response `tx_instructions` must be a list")
        instructions = [RuntimeInstruction.from_payload(entry) for entry in instructions_payload]
        return cls(ok=ok_value, tx_instructions=instructions)


@dataclass(frozen=True)
class ConnectPerIpSessions:
    """Active session count for a single IP address."""

    ip: str
    sessions: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectPerIpSessions":
        if not isinstance(payload, Mapping):
            raise TypeError("per-ip sessions entry must be an object")
        ip = payload.get("ip")
        if not isinstance(ip, str):
            raise TypeError("per-ip sessions entry missing string `ip` field")
        try:
            sessions = int(payload.get("sessions", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("per-ip sessions entry `sessions` must be numeric") from exc
        return cls(ip=ip, sessions=sessions)


@dataclass(frozen=True)
class ConnectPolicyStatusSnapshot:
    """Policy limits surfaced by `/v1/connect/status`."""

    ws_max_sessions: int
    ws_per_ip_max_sessions: int
    ws_rate_per_ip_per_min: int
    session_ttl_ms: int
    frame_max_bytes: int
    session_buffer_max_bytes: int
    relay_enabled: bool
    heartbeat_interval_ms: int
    heartbeat_miss_tolerance: int
    heartbeat_min_interval_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectPolicyStatusSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("connect policy payload must be an object")
        try:
            ws_max_sessions = int(payload.get("ws_max_sessions", 0))
            ws_per_ip_max_sessions = int(payload.get("ws_per_ip_max_sessions", 0))
            ws_rate = int(payload.get("ws_rate_per_ip_per_min", 0))
            session_ttl_ms = int(payload.get("session_ttl_ms", 0))
            frame_max_bytes = int(payload.get("frame_max_bytes", 0))
            session_buffer_max_bytes = int(payload.get("session_buffer_max_bytes", 0))
            relay_enabled = bool(payload.get("relay_enabled", False))
            heartbeat_interval_ms = int(payload.get("heartbeat_interval_ms", 0))
            heartbeat_miss_tolerance = int(payload.get("heartbeat_miss_tolerance", 0))
            heartbeat_min_interval_ms = int(payload.get("heartbeat_min_interval_ms", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("connect policy fields must be numeric/boolean") from exc
        return cls(
            ws_max_sessions=ws_max_sessions,
            ws_per_ip_max_sessions=ws_per_ip_max_sessions,
            ws_rate_per_ip_per_min=ws_rate,
            session_ttl_ms=session_ttl_ms,
            frame_max_bytes=frame_max_bytes,
            session_buffer_max_bytes=session_buffer_max_bytes,
            relay_enabled=relay_enabled,
            heartbeat_interval_ms=heartbeat_interval_ms,
            heartbeat_miss_tolerance=heartbeat_miss_tolerance,
            heartbeat_min_interval_ms=heartbeat_min_interval_ms,
        )


@dataclass(frozen=True)
class ConnectStatusSnapshot:
    """Runtime status snapshot returned by `/v1/connect/status`."""

    enabled: bool
    sessions_total: int
    sessions_active: int
    per_ip_sessions: List[ConnectPerIpSessions]
    buffered_sessions: int
    total_buffer_bytes: int
    dedupe_size: int
    policy: Optional[ConnectPolicyStatusSnapshot]
    frames_in_total: int
    frames_out_total: int
    ciphertext_total: int
    dedupe_drops_total: int
    buffer_drops_total: int
    plaintext_control_drops_total: int
    monotonic_drops_total: int
    ping_miss_total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectStatusSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("connect status payload must be an object")
        enabled = bool(payload.get("enabled", False))
        def _coerce_int_field(name: str, default: int = 0) -> int:
            try:
                return int(payload.get(name, default))
            except (TypeError, ValueError) as exc:
                raise TypeError(f"connect status field `{name}` must be numeric") from exc

        per_ip_raw = payload.get("per_ip_sessions", [])
        if per_ip_raw is None:
            per_ip_raw = []
        if not isinstance(per_ip_raw, list):
            raise TypeError("connect status `per_ip_sessions` must be a list")
        per_ip = [ConnectPerIpSessions.from_payload(item) for item in per_ip_raw]
        policy_payload = payload.get("policy")
        policy = (
            ConnectPolicyStatusSnapshot.from_payload(policy_payload)
            if isinstance(policy_payload, Mapping)
            else None
        )
        return cls(
            enabled=enabled,
            sessions_total=_coerce_int_field("sessions_total"),
            sessions_active=_coerce_int_field("sessions_active"),
            per_ip_sessions=per_ip,
            buffered_sessions=_coerce_int_field("buffered_sessions"),
            total_buffer_bytes=_coerce_int_field("total_buffer_bytes"),
            dedupe_size=_coerce_int_field("dedupe_size"),
            policy=policy,
            frames_in_total=_coerce_int_field("frames_in_total"),
            frames_out_total=_coerce_int_field("frames_out_total"),
            ciphertext_total=_coerce_int_field("ciphertext_total"),
            dedupe_drops_total=_coerce_int_field("dedupe_drops_total"),
            buffer_drops_total=_coerce_int_field("buffer_drops_total"),
            plaintext_control_drops_total=_coerce_int_field("plaintext_control_drops_total"),
            monotonic_drops_total=_coerce_int_field("monotonic_drops_total"),
            ping_miss_total=_coerce_int_field("ping_miss_total"),
        )


@dataclass(frozen=True)
class ToriiStatusMetrics:
    """Derived metrics computed from consecutive `/v1/status` samples."""

    commit_latency_ms: int
    queue_size: int
    queue_delta: int
    da_reschedule_delta: int
    tx_approved_delta: int
    tx_rejected_delta: int
    view_change_delta: int

    @classmethod
    def from_samples(
        cls,
        previous: Optional["ToriiStatusPayload"],
        current: "ToriiStatusPayload",
    ) -> "ToriiStatusMetrics":
        if previous is None:
            return cls(
                commit_latency_ms=current.commit_time_ms,
                queue_size=current.queue_size,
                queue_delta=0,
                da_reschedule_delta=0,
                tx_approved_delta=0,
                tx_rejected_delta=0,
                view_change_delta=0,
            )
        return cls(
            commit_latency_ms=current.commit_time_ms,
            queue_size=current.queue_size,
            queue_delta=current.queue_size - previous.queue_size,
            da_reschedule_delta=max(
                0, current.da_reschedule_total - previous.da_reschedule_total
            ),
            tx_approved_delta=max(0, current.txs_approved - previous.txs_approved),
            tx_rejected_delta=max(0, current.txs_rejected - previous.txs_rejected),
            view_change_delta=max(0, current.view_changes - previous.view_changes),
        )

    @property
    def has_activity(self) -> bool:
        """Return ``True`` if the snapshot reflects any queue or transaction movement."""

        return any(
            value
            for value in (
                self.queue_delta,
                self.da_reschedule_delta,
                self.tx_approved_delta,
                self.tx_rejected_delta,
                self.view_change_delta,
            )
        )


@dataclass(frozen=True)
class GovernanceProposalSnapshot:
    proposed: int
    approved: int
    rejected: int
    enacted: int


@dataclass(frozen=True)
class GovernanceProtectedNamespaceSnapshot:
    total_checks: int
    allowed: int
    rejected: int


@dataclass(frozen=True)
class GovernanceManifestAdmissionSnapshot:
    total_checks: int
    allowed: int
    missing_manifest: int
    non_validator_authority: int
    quorum_rejected: int
    protected_namespace_rejected: int
    runtime_hook_rejected: int


@dataclass(frozen=True)
class GovernanceManifestQuorumSnapshot:
    total_checks: int
    satisfied: int
    rejected: int


@dataclass(frozen=True)
class GovernanceManifestActivationSnapshot:
    namespace: str
    contract_id: str
    code_hash_hex: str
    abi_hash_hex: Optional[str]
    height: int
    activated_at_ms: int


@dataclass(frozen=True)
class GovernanceStatusSnapshot:
    proposals: GovernanceProposalSnapshot
    protected_namespace: GovernanceProtectedNamespaceSnapshot
    manifest_admission: GovernanceManifestAdmissionSnapshot
    manifest_quorum: GovernanceManifestQuorumSnapshot
    recent_manifest_activations: List[GovernanceManifestActivationSnapshot]


@dataclass(frozen=True)
class ToriiLaneCommitmentSnapshot:
    block_height: int
    lane_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash_hex: str


@dataclass(frozen=True)
class ToriiDataspaceCommitmentSnapshot:
    block_height: int
    lane_id: int
    dataspace_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash_hex: str


@dataclass(frozen=True)
class ToriiLaneRuntimeUpgradeHookSnapshot:
    allow: bool
    require_metadata: bool
    metadata_key: Optional[str]
    allowed_ids: List[str]


@dataclass(frozen=True)
class ToriiLaneMerkleCommitmentSnapshot:
    root: str
    max_depth: int


@dataclass(frozen=True)
class ToriiLaneSnarkCommitmentSnapshot:
    circuit_id: int
    verifying_key_digest: str
    statement_hash: str
    proof_hash: str


@dataclass(frozen=True)
class ToriiLanePrivacyCommitmentSnapshot:
    id: int
    scheme: str
    merkle: Optional[ToriiLaneMerkleCommitmentSnapshot]
    snark: Optional[ToriiLaneSnarkCommitmentSnapshot]


@dataclass(frozen=True)
class ToriiLaneGovernanceSnapshot:
    lane_id: int
    alias: str
    dataspace_id: int
    visibility: str
    storage_profile: str
    governance: Optional[str]
    manifest_required: bool
    manifest_ready: bool
    manifest_path: Optional[str]
    validator_ids: List[str]
    quorum: Optional[int]
    protected_namespaces: List[str]
    runtime_upgrade: Optional[ToriiLaneRuntimeUpgradeHookSnapshot]
    privacy_commitments: List[ToriiLanePrivacyCommitmentSnapshot]


@dataclass(frozen=True)
class ToriiStatusPayload:
    """Decoded `/v1/status` payload with convenient integer accessors and lane summaries."""

    peers: int
    queue_size: int
    commit_time_ms: int
    da_reschedule_total: int
    txs_approved: int
    txs_rejected: int
    view_changes: int
    governance: Optional[GovernanceStatusSnapshot]
    lane_commitments: List[ToriiLaneCommitmentSnapshot]
    dataspace_commitments: List[ToriiDataspaceCommitmentSnapshot]
    lane_governance: List[ToriiLaneGovernanceSnapshot]
    lane_governance_sealed_total: int
    lane_governance_sealed_aliases: List[str]
    raw: Mapping[str, Any] = field(default_factory=dict)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ToriiStatusPayload":
        if not isinstance(payload, Mapping):
            raise TypeError("status payload must be an object")

        def _coerce_int(name: str) -> int:
            value = payload.get(name, 0)
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"status payload field `{name}` must be numeric") from exc

        def _coerce_nested_int(mapping: Mapping[str, Any], key: str, context: str) -> int:
            value = mapping.get(key, 0)
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"{context} `{key}` must be numeric") from exc

        def _coerce_string(value: Any, context: str) -> str:
            if isinstance(value, str):
                return value
            raise TypeError(f"{context} must be a string")

        def _coerce_optional_string(value: Any, context: str) -> Optional[str]:
            if value is None:
                return None
            if isinstance(value, str):
                return value
            raise TypeError(f"{context} must be a string when present")

        def _coerce_string_list(value: Any, context: str) -> List[str]:
            if value is None:
                return []
            if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
                raise TypeError(f"{context} must be an array of strings")
            result: List[str] = []
            for idx, item in enumerate(value):
                if not isinstance(item, str):
                    raise TypeError(f"{context}[{idx}] must be a string")
                result.append(item)
            return result

        def _parse_privacy_commitments(
            value: Any, context: str
        ) -> List[ToriiLanePrivacyCommitmentSnapshot]:
            if value is None:
                return []
            if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
                raise TypeError(f"{context} must be an array")
            commitments: List[ToriiLanePrivacyCommitmentSnapshot] = []
            for idx, item in enumerate(value):
                if not isinstance(item, Mapping):
                    raise TypeError(f"{context}[{idx}] must be an object")
                entry_context = f"{context}[{idx}]"
                commitment_id = _coerce_nested_int(item, "id", entry_context)
                scheme = _coerce_string(item.get("scheme"), f"{entry_context}.scheme")
                merkle_payload = item.get("merkle")
                snark_payload = item.get("snark")
                merkle: Optional[ToriiLaneMerkleCommitmentSnapshot] = None
                snark: Optional[ToriiLaneSnarkCommitmentSnapshot] = None
                if scheme == "merkle":
                    if not isinstance(merkle_payload, Mapping):
                        raise TypeError(f"{entry_context}.merkle must be an object")
                    merkle = ToriiLaneMerkleCommitmentSnapshot(
                        root=_coerce_string(
                            merkle_payload.get("root"),
                            f"{entry_context}.merkle.root",
                        ),
                        max_depth=_coerce_nested_int(
                            merkle_payload,
                            "max_depth",
                            f"{entry_context}.merkle",
                        ),
                    )
                elif scheme == "snark":
                    if not isinstance(snark_payload, Mapping):
                        raise TypeError(f"{entry_context}.snark must be an object")
                    snark = ToriiLaneSnarkCommitmentSnapshot(
                        circuit_id=_coerce_nested_int(
                            snark_payload,
                            "circuit_id",
                            f"{entry_context}.snark",
                        ),
                        verifying_key_digest=_coerce_string(
                            snark_payload.get("verifying_key_digest"),
                            f"{entry_context}.snark.verifying_key_digest",
                        ),
                        statement_hash=_coerce_string(
                            snark_payload.get("statement_hash"),
                            f"{entry_context}.snark.statement_hash",
                        ),
                        proof_hash=_coerce_string(
                            snark_payload.get("proof_hash"),
                            f"{entry_context}.snark.proof_hash",
                        ),
                    )
                else:
                    raise ValueError(f"{entry_context}.scheme must be 'merkle' or 'snark'")
                commitments.append(
                    ToriiLanePrivacyCommitmentSnapshot(
                        id=commitment_id,
                        scheme=scheme,
                        merkle=merkle,
                        snark=snark,
                    )
                )
            return commitments

        def _coerce_bool(value: Any, context: str) -> bool:
            if isinstance(value, bool):
                return value
            raise TypeError(f"{context} must be a boolean")

        def _coerce_optional_int(value: Any, context: str) -> Optional[int]:
            if value is None:
                return None
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"{context} must be numeric when present") from exc

        governance_snapshot: Optional[GovernanceStatusSnapshot] = None
        governance_payload = payload.get("governance")
        if isinstance(governance_payload, Mapping):
            proposals_payload = governance_payload.get("proposals")
            protected_payload = governance_payload.get("protected_namespace")
            admission_payload = governance_payload.get("manifest_admission")
            quorum_payload = governance_payload.get("manifest_quorum")
            activations_payload = governance_payload.get("recent_manifest_activations")

            if not isinstance(proposals_payload, Mapping):
                raise TypeError("governance payload missing object `proposals` field")
            if not isinstance(protected_payload, Mapping):
                raise TypeError("governance payload missing object `protected_namespace` field")
            if not isinstance(admission_payload, Mapping):
                raise TypeError("governance payload missing object `manifest_admission` field")
            if not isinstance(quorum_payload, Mapping):
                raise TypeError("governance payload missing object `manifest_quorum` field")
            if activations_payload is None:
                activations_payload = []
            if not isinstance(activations_payload, Sequence):
                raise TypeError("governance payload `recent_manifest_activations` must be an array")

            proposals = GovernanceProposalSnapshot(
                proposed=_coerce_nested_int(proposals_payload, "proposed", "governance.proposals"),
                approved=_coerce_nested_int(proposals_payload, "approved", "governance.proposals"),
                rejected=_coerce_nested_int(proposals_payload, "rejected", "governance.proposals"),
                enacted=_coerce_nested_int(proposals_payload, "enacted", "governance.proposals"),
            )
            protected = GovernanceProtectedNamespaceSnapshot(
                total_checks=_coerce_nested_int(
                    protected_payload, "total_checks", "governance.protected_namespace"
                ),
                allowed=_coerce_nested_int(
                    protected_payload, "allowed", "governance.protected_namespace"
                ),
                rejected=_coerce_nested_int(
                    protected_payload, "rejected", "governance.protected_namespace"
                ),
            )
            admission = GovernanceManifestAdmissionSnapshot(
                total_checks=_coerce_nested_int(
                    admission_payload, "total_checks", "governance.manifest_admission"
                ),
                allowed=_coerce_nested_int(
                    admission_payload, "allowed", "governance.manifest_admission"
                ),
                missing_manifest=_coerce_nested_int(
                    admission_payload, "missing_manifest", "governance.manifest_admission"
                ),
                non_validator_authority=_coerce_nested_int(
                    admission_payload,
                    "non_validator_authority",
                    "governance.manifest_admission",
                ),
                quorum_rejected=_coerce_nested_int(
                    admission_payload, "quorum_rejected", "governance.manifest_admission"
                ),
                protected_namespace_rejected=_coerce_nested_int(
                    admission_payload,
                    "protected_namespace_rejected",
                    "governance.manifest_admission",
                ),
                runtime_hook_rejected=_coerce_nested_int(
                    admission_payload, "runtime_hook_rejected", "governance.manifest_admission"
                ),
            )
            quorum = GovernanceManifestQuorumSnapshot(
                total_checks=_coerce_nested_int(
                    quorum_payload, "total_checks", "governance.manifest_quorum"
                ),
                satisfied=_coerce_nested_int(
                    quorum_payload, "satisfied", "governance.manifest_quorum"
                ),
                rejected=_coerce_nested_int(
                    quorum_payload, "rejected", "governance.manifest_quorum"
                ),
            )

            recent_activations: List[GovernanceManifestActivationSnapshot] = []
            for idx, item in enumerate(activations_payload):
                if not isinstance(item, Mapping):
                    raise TypeError(
                        f"governance manifest activation at index {idx} must be an object"
                    )
                namespace = item.get("namespace", "")
                contract_id = item.get("contract_id", "")
                code_hash = item.get("code_hash_hex", "")
                abi_hash = item.get("abi_hash_hex")
                try:
                    height = int(item.get("height", 0))
                    activated_at_ms = int(item.get("activated_at_ms", 0))
                except (TypeError, ValueError) as exc:
                    raise TypeError(
                        "governance manifest activation height/activated_at_ms must be numeric"
                    ) from exc
                recent_activations.append(
                    GovernanceManifestActivationSnapshot(
                        namespace=str(namespace),
                        contract_id=str(contract_id),
                        code_hash_hex=str(code_hash),
                        abi_hash_hex=str(abi_hash) if abi_hash is not None else None,
                        height=height,
                        activated_at_ms=activated_at_ms,
                    )
                )

            governance_snapshot = GovernanceStatusSnapshot(
                proposals=proposals,
                protected_namespace=protected,
                manifest_admission=admission,
                manifest_quorum=quorum,
                recent_manifest_activations=recent_activations,
            )

        lane_commitments_payload = payload.get("lane_commitments")
        lane_commitments: List[ToriiLaneCommitmentSnapshot] = []
        if lane_commitments_payload:
            if not isinstance(lane_commitments_payload, Sequence):
                raise TypeError("lane_commitments must be an array")
            for idx, item in enumerate(lane_commitments_payload):
                if not isinstance(item, Mapping):
                    raise TypeError(f"lane_commitments[{idx}] must be an object")
                lane_commitments.append(
                    ToriiLaneCommitmentSnapshot(
                        block_height=_coerce_nested_int(item, "block_height", f"lane_commitments[{idx}]"),
                        lane_id=_coerce_nested_int(item, "lane_id", f"lane_commitments[{idx}]"),
                        tx_count=_coerce_nested_int(item, "tx_count", f"lane_commitments[{idx}]"),
                        total_chunks=_coerce_nested_int(item, "total_chunks", f"lane_commitments[{idx}]"),
                        rbc_bytes_total=_coerce_nested_int(item, "rbc_bytes_total", f"lane_commitments[{idx}]"),
                        teu_total=_coerce_nested_int(item, "teu_total", f"lane_commitments[{idx}]"),
                        block_hash_hex=_coerce_string(item.get("block_hash"), f"lane_commitments[{idx}].block_hash"),
                    )
                )

        dataspace_commitments_payload = payload.get("dataspace_commitments")
        dataspace_commitments: List[ToriiDataspaceCommitmentSnapshot] = []
        if dataspace_commitments_payload:
            if not isinstance(dataspace_commitments_payload, Sequence):
                raise TypeError("dataspace_commitments must be an array")
            for idx, item in enumerate(dataspace_commitments_payload):
                if not isinstance(item, Mapping):
                    raise TypeError(f"dataspace_commitments[{idx}] must be an object")
                dataspace_commitments.append(
                    ToriiDataspaceCommitmentSnapshot(
                        block_height=_coerce_nested_int(item, "block_height", f"dataspace_commitments[{idx}]"),
                        lane_id=_coerce_nested_int(item, "lane_id", f"dataspace_commitments[{idx}]"),
                        dataspace_id=_coerce_nested_int(item, "dataspace_id", f"dataspace_commitments[{idx}]"),
                        tx_count=_coerce_nested_int(item, "tx_count", f"dataspace_commitments[{idx}]"),
                        total_chunks=_coerce_nested_int(item, "total_chunks", f"dataspace_commitments[{idx}]"),
                        rbc_bytes_total=_coerce_nested_int(item, "rbc_bytes_total", f"dataspace_commitments[{idx}]"),
                        teu_total=_coerce_nested_int(item, "teu_total", f"dataspace_commitments[{idx}]"),
                        block_hash_hex=_coerce_string(item.get("block_hash"), f"dataspace_commitments[{idx}].block_hash"),
                    )
                )

        lane_governance_payload = payload.get("lane_governance")
        lane_governance: List[ToriiLaneGovernanceSnapshot] = []
        if lane_governance_payload:
            if not isinstance(lane_governance_payload, Sequence):
                raise TypeError("lane_governance must be an array")
            for idx, item in enumerate(lane_governance_payload):
                if not isinstance(item, Mapping):
                    raise TypeError(f"lane_governance[{idx}] must be an object")
                validator_ids = _coerce_string_list(
                    item.get("validator_ids"),
                    f"lane_governance[{idx}].validator_ids",
                )
                namespaces = _coerce_string_list(
                    item.get("protected_namespaces"),
                    f"lane_governance[{idx}].protected_namespaces",
                )
                runtime_payload = item.get("runtime_upgrade")
                runtime_upgrade = None
                if runtime_payload is not None:
                    if not isinstance(runtime_payload, Mapping):
                        raise TypeError(f"lane_governance[{idx}].runtime_upgrade must be an object")
                    runtime_upgrade = ToriiLaneRuntimeUpgradeHookSnapshot(
                        allow=_coerce_bool(
                            runtime_payload.get("allow"),
                            f"lane_governance[{idx}].runtime_upgrade.allow",
                        ),
                        require_metadata=_coerce_bool(
                            runtime_payload.get("require_metadata"),
                            f"lane_governance[{idx}].runtime_upgrade.require_metadata",
                        ),
                        metadata_key=_coerce_optional_string(
                            runtime_payload.get("metadata_key"),
                            f"lane_governance[{idx}].runtime_upgrade.metadata_key",
                        ),
                        allowed_ids=_coerce_string_list(
                            runtime_payload.get("allowed_ids"),
                            f"lane_governance[{idx}].runtime_upgrade.allowed_ids",
                        ),
                    )
                privacy_commitments = _parse_privacy_commitments(
                    item.get("privacy_commitments"),
                    f"lane_governance[{idx}].privacy_commitments",
                )
                lane_governance.append(
                    ToriiLaneGovernanceSnapshot(
                        lane_id=_coerce_nested_int(item, "lane_id", f"lane_governance[{idx}]"),
                        alias=_coerce_string(item.get("alias"), f"lane_governance[{idx}].alias"),
                        dataspace_id=_coerce_nested_int(
                            item,
                            "dataspace_id",
                            f"lane_governance[{idx}]",
                        ),
                        visibility=_coerce_string(
                            item.get("visibility"),
                            f"lane_governance[{idx}].visibility",
                        ),
                        storage_profile=_coerce_string(
                            item.get("storage_profile"),
                            f"lane_governance[{idx}].storage_profile",
                        ),
                        governance=_coerce_optional_string(
                            item.get("governance"),
                            f"lane_governance[{idx}].governance",
                        ),
                        manifest_required=_coerce_bool(
                            item.get("manifest_required"),
                            f"lane_governance[{idx}].manifest_required",
                        ),
                        manifest_ready=_coerce_bool(
                            item.get("manifest_ready"),
                            f"lane_governance[{idx}].manifest_ready",
                        ),
                        manifest_path=_coerce_optional_string(
                            item.get("manifest_path"),
                            f"lane_governance[{idx}].manifest_path",
                        ),
                        validator_ids=validator_ids,
                        quorum=_coerce_optional_int(
                            item.get("quorum"),
                            f"lane_governance[{idx}].quorum",
                        ),
                        protected_namespaces=namespaces,
                        runtime_upgrade=runtime_upgrade,
                        privacy_commitments=privacy_commitments,
                    )
                )

        lane_governance_sealed_total = _coerce_int("lane_governance_sealed_total")
        lane_governance_sealed_aliases = _coerce_string_list(
            payload.get("lane_governance_sealed_aliases"),
            "lane_governance_sealed_aliases",
        )

        return cls(
            peers=_coerce_int("peers"),
            queue_size=_coerce_int("queue_size"),
            commit_time_ms=_coerce_int("commit_time_ms"),
            da_reschedule_total=_coerce_int("da_reschedule_total"),
            txs_approved=_coerce_int("txs_approved"),
            txs_rejected=_coerce_int("txs_rejected"),
            view_changes=_coerce_int("view_changes"),
            governance=governance_snapshot,
            lane_commitments=lane_commitments,
            dataspace_commitments=dataspace_commitments,
            lane_governance=lane_governance,
            lane_governance_sealed_total=lane_governance_sealed_total,
            lane_governance_sealed_aliases=lane_governance_sealed_aliases,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class ToriiStatusSnapshot:
    """Snapshot captured from `/v1/status` together with derived metrics."""

    timestamp: float
    status: ToriiStatusPayload
    metrics: ToriiStatusMetrics

    @property
    def has_activity(self) -> bool:
        """Return ``True`` when the underlying metrics observed any movement."""

        return self.metrics.has_activity


class _ToriiStatusState:
    """Internal helper tracking the previous status sample per client."""

    def __init__(self) -> None:
        self._previous: Optional[ToriiStatusPayload] = None

    def record(self, payload: ToriiStatusPayload) -> ToriiStatusMetrics:
        metrics = ToriiStatusMetrics.from_samples(self._previous, payload)
        self._previous = payload
        return metrics


_TORII_ENV_KEYS = {
    "timeout_ms": "IROHA_TORII_TIMEOUT_MS",
    "max_retries": "IROHA_TORII_MAX_RETRIES",
    "backoff_initial_ms": "IROHA_TORII_BACKOFF_INITIAL_MS",
    "backoff_multiplier": "IROHA_TORII_BACKOFF_MULTIPLIER",
    "max_backoff_ms": "IROHA_TORII_MAX_BACKOFF_MS",
    "retry_statuses": "IROHA_TORII_RETRY_STATUSES",
    "retry_methods": "IROHA_TORII_RETRY_METHODS",
    "api_token": "IROHA_TORII_API_TOKEN",
    "auth_token": "IROHA_TORII_AUTH_TOKEN",
}


def _coerce_sorafs_policy_value(value: Any, context: str) -> SorafsAliasPolicy:
    if isinstance(value, SorafsAliasPolicy):
        return value
    if isinstance(value, Mapping):
        return SorafsAliasPolicy.from_mapping(value)
    raise TypeError(f"{context} must be provided as a mapping or SorafsAliasPolicy instance")


def _normalize_sorafs_policy_config(
    policy: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]]
) -> SorafsAliasPolicy:
    if policy is None:
        return SorafsAliasPolicy.defaults()
    return _coerce_sorafs_policy_value(policy, "sorafs_alias_policy")


def resolve_torii_client_config(
    *,
    config: Optional[Mapping[str, Any]] = None,
    env: Optional[Mapping[str, str]] = None,
    overrides: Optional[Mapping[str, Any]] = None,
) -> ResolvedToriiClientConfig:
    """Merge Torii client settings from config files, environment variables, and overrides."""

    state: Dict[str, Any] = {
        "timeout": _DEFAULT_RESOLVED_CONFIG.timeout,
        "max_retries": _DEFAULT_RESOLVED_CONFIG.max_retries,
        "backoff_initial": _DEFAULT_RESOLVED_CONFIG.backoff_initial,
        "backoff_multiplier": _DEFAULT_RESOLVED_CONFIG.backoff_multiplier,
        "max_backoff": _DEFAULT_RESOLVED_CONFIG.max_backoff,
        "retry_statuses": set(_DEFAULT_RESOLVED_CONFIG.retry_statuses),
        "retry_methods": set(_DEFAULT_RESOLVED_CONFIG.retry_methods),
        "default_headers": dict(_DEFAULT_RESOLVED_CONFIG.default_headers),
        "auth_token": _DEFAULT_RESOLVED_CONFIG.auth_token,
        "api_token": _DEFAULT_RESOLVED_CONFIG.api_token,
        "sorafs_alias_policy": _DEFAULT_RESOLVED_CONFIG.sorafs_alias_policy,
    }

    def apply_source(source: Optional[Mapping[str, Any]]) -> None:
        if not source:
            return
        _reject_alias_keys(
            source,
            {
                "timeoutMs": "timeout_ms",
                "timeoutSeconds": "timeout",
                "maxRetries": "max_retries",
                "backoffInitialMs": "backoff_initial_ms",
                "backoffInitial": "backoff_initial",
                "backoffMultiplier": "backoff_multiplier",
                "maxBackoffMs": "max_backoff_ms",
                "maxBackoff": "max_backoff",
                "retryStatuses": "retry_statuses",
                "retryMethods": "retry_methods",
                "defaultHeaders": "default_headers",
                "authToken": "auth_token",
                "apiToken": "api_token",
                "sorafsAliasPolicy": "sorafs_alias_policy",
            },
            context="torii_client config",
        )
        timeout = _coerce_timeout_seconds(
            source.get("timeout_ms"),
            fallback=source.get("timeout"),
        )
        if timeout is not None:
            state["timeout"] = timeout
        max_retries = _coerce_int(
            source.get("max_retries"),
            "max_retries",
            allow_zero=True,
        )
        if max_retries is not None:
            state["max_retries"] = max_retries
        backoff_initial = _coerce_duration_seconds(
            source.get("backoff_initial_ms"),
            fallback=source.get("backoff_initial"),
        )
        if backoff_initial is not None:
            state["backoff_initial"] = backoff_initial
        backoff_multiplier = _coerce_float(
            source.get("backoff_multiplier"),
            "backoff_multiplier",
            allow_zero=False,
        )
        if backoff_multiplier is not None:
            state["backoff_multiplier"] = max(backoff_multiplier, 1.0)
        max_backoff = _coerce_duration_seconds(
            source.get("max_backoff_ms"),
            fallback=source.get("max_backoff"),
        )
        if max_backoff is not None:
            state["max_backoff"] = max_backoff
        statuses = _parse_retry_statuses(
            source.get("retry_statuses")
        )
        if statuses is not None:
            state["retry_statuses"] = statuses
        methods = _parse_retry_methods(
            source.get("retry_methods")
        )
        if methods is not None:
            state["retry_methods"] = methods
        headers = _normalize_headers(source.get("default_headers"))
        if headers:
            state["default_headers"].update(headers)
        auth_token = source.get("auth_token")
        if auth_token is not None:
            state["auth_token"] = str(auth_token)
        api_token = source.get("api_token")
        if api_token is not None:
            state["api_token"] = str(api_token)
        policy_override = source.get("sorafs_alias_policy")
        if policy_override is not None:
            state["sorafs_alias_policy"] = _coerce_sorafs_policy_value(
                policy_override, "sorafs_alias_policy"
            )

    apply_source(_extract_torii_client_section(config))

    if isinstance(config, Mapping):
        if "toriiConfig" in config:
            raise TypeError("toriiConfig is not supported; use torii")
        torii_section = config.get("torii")
        token = _pick_api_token(torii_section)
        if token and not state["api_token"]:
            state["api_token"] = token

    env_vars = os.environ if env is None else env
    apply_source(
        {
            "timeout_ms": env_vars.get(_TORII_ENV_KEYS["timeout_ms"]),
            "max_retries": env_vars.get(_TORII_ENV_KEYS["max_retries"]),
            "backoff_initial_ms": env_vars.get(_TORII_ENV_KEYS["backoff_initial_ms"]),
            "backoff_multiplier": env_vars.get(_TORII_ENV_KEYS["backoff_multiplier"]),
            "max_backoff_ms": env_vars.get(_TORII_ENV_KEYS["max_backoff_ms"]),
            "retry_statuses": env_vars.get(_TORII_ENV_KEYS["retry_statuses"]),
            "retry_methods": env_vars.get(_TORII_ENV_KEYS["retry_methods"]),
            "api_token": env_vars.get(_TORII_ENV_KEYS["api_token"]),
            "auth_token": env_vars.get(_TORII_ENV_KEYS["auth_token"]),
        }
    )

    apply_source(overrides)

    headers = dict(state["default_headers"])
    if not any(key.lower() == "accept" for key in headers):
        headers["Accept"] = "application/json"

    max_backoff = state["max_backoff"]
    if max_backoff <= 0:
        max_backoff = math.inf

    return ResolvedToriiClientConfig(
        timeout=max(state["timeout"], 0.0),
        max_retries=max(0, int(state["max_retries"])),
        backoff_initial=max(state["backoff_initial"], 0.0),
        backoff_multiplier=max(state["backoff_multiplier"], 1.0),
        max_backoff=max_backoff,
        retry_statuses=frozenset(int(code) for code in state["retry_statuses"]),
        retry_methods=frozenset(method.upper() for method in state["retry_methods"]),
        default_headers=headers,
        auth_token=state["auth_token"],
        api_token=state["api_token"],
        sorafs_alias_policy=state["sorafs_alias_policy"],
    )


def _extract_torii_client_section(config: Optional[Mapping[str, Any]]) -> Mapping[str, Any]:
    if not config:
        return {}
    if isinstance(config, Mapping):
        if "toriiClient" in config:
            raise TypeError("toriiClient is not supported; use torii_client")
        nested = config.get("torii_client")
        if isinstance(nested, Mapping):
            return nested
    return config


def _extract_pipeline_status_kind(payload: Any) -> Optional[str]:
    """Return the pipeline status `kind` from a Torii response, if present."""

    if not isinstance(payload, Mapping):
        return None

    def coerce_status(status_obj: Any) -> Optional[str]:
        if isinstance(status_obj, Mapping):
            kind = status_obj.get("kind")
            if kind is not None:
                return str(kind)
        elif status_obj is not None:
            return str(status_obj)
        return None

    status = coerce_status(payload.get("status"))
    if status is not None:
        return status

    content = payload.get("content")
    if isinstance(content, Mapping):
        return coerce_status(content.get("status"))

    return None


def _pick_api_token(torii_section: Optional[Mapping[str, Any]]) -> Optional[str]:
    if not isinstance(torii_section, Mapping):
        return None
    if "apiTokens" in torii_section:
        raise TypeError("apiTokens is not supported; use api_tokens")
    tokens = torii_section.get("api_tokens")
    if isinstance(tokens, (list, tuple)) and tokens:
        return str(tokens[0])
    if isinstance(tokens, str):
        return tokens
    return None


def _coerce_int(value: Any, name: str, *, allow_zero: bool = False) -> Optional[int]:
    if value is None or value == "":
        return None
    try:
        number = int(value)
    except (TypeError, ValueError):
        raise TypeError(f"{name} must be an integer") from None
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{name} must be {'non-negative' if allow_zero else 'positive'}")
    return number


def _coerce_float(value: Any, name: str, *, allow_zero: bool = False) -> Optional[float]:
    if value is None or value == "":
        return None
    try:
        number = float(value)
    except (TypeError, ValueError):
        raise TypeError(f"{name} must be numeric") from None
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{name} must be greater than 0")
    return number


def _coerce_duration_seconds(value: Any, *, fallback: Any = None) -> Optional[float]:
    millis = _coerce_float(value, "duration_ms", allow_zero=True)
    if millis is not None:
        return millis / 1000.0
    seconds = _coerce_float(fallback, "duration", allow_zero=True)
    return seconds


def _coerce_timeout_seconds(value: Any, *, fallback: Any = None) -> Optional[float]:
    result = _coerce_duration_seconds(value)
    if result is not None:
        return result
    seconds = _coerce_float(fallback, "timeout", allow_zero=True)
    return seconds


def _parse_retry_statuses(value: Any) -> Optional[set[int]]:
    if value is None or value == "":
        return None
    statuses: set[int] = set()
    if isinstance(value, str):
        parts = [part.strip() for part in value.split(",") if part.strip()]
    elif isinstance(value, (list, tuple, set)):
        parts = value
    else:
        parts = [value]
    for entry in parts:
        statuses.add(int(entry))
    return statuses


def _parse_retry_methods(value: Any) -> Optional[set[str]]:
    if value is None or value == "":
        return None
    methods: set[str] = set()
    if isinstance(value, str):
        entries = [part.strip() for part in value.split(",") if part.strip()]
    elif isinstance(value, (list, tuple, set)):
        entries = value
    else:
        entries = [value]
    for entry in entries:
        methods.add(str(entry).upper())
    return methods


def _normalize_headers(headers: Any) -> Dict[str, str]:
    normalized: Dict[str, str] = {}
    if isinstance(headers, Mapping):
        for key, value in headers.items():
            if value is not None:
                normalized[str(key)] = str(value)
    return normalized


def _require_crypto() -> ModuleType:
    """Return the compiled crypto bindings, raising a helpful error when missing."""

    global _CRYPTO_MODULE
    if _CRYPTO_MODULE is not None:
        return _CRYPTO_MODULE
    try:
        from . import crypto as _crypto  # type: ignore import
    except RuntimeError as exc:  # pragma: no cover - optional runtime dependency
        raise RuntimeError(
            "iroha_python._crypto extension module is required for transaction helpers. "
            "Run `maturin develop --release` inside `python/iroha_python` (or install the wheel) "
            "before using these APIs."
        ) from exc
    _CRYPTO_MODULE = _crypto
    return _crypto


def signed_transaction_envelope_from_json(envelope_json: str) -> "SignedTransactionEnvelope":
    """Parse a signed transaction envelope from a JSON payload."""

    return _require_crypto().signed_transaction_envelope_from_json(envelope_json)


__all__ = [
    "ToriiClient",
    "create_torii_client",
    "TransactionStatusError",
    "signed_transaction_envelope_from_json",
    "resolve_torii_client_config",
    "ResolvedToriiClientConfig",
    "SseEvent",
    "EventCursor",
    "NetworkTimeSnapshot",
    "NetworkTimeStatus",
    "NetworkTimeSample",
    "NetworkTimeRttBucket",
    "NodeCapabilities",
    "NodeAdminSnapshot",
    "SorafsPorSubmissionResponse",
    "SorafsPorObservationResponse",
    "SorafsPorVerdictResponse",
    "SorafsPorIngestionProviderStatus",
    "SorafsPorIngestionStatus",
    "ExplorerMetricsSnapshot",
    "ExplorerAccountQrSnapshot",
    "IsoSubmissionRecord",
    "IsoMessageTimeoutError",
    "AccountAsset",
    "AccountAssetsPage",
    "AccountTransaction",
    "AccountTransactionsPage",
    "AccountRecord",
    "AccountListPage",
    "DomainRecord",
    "DomainListPage",
    "AssetDefinitionRecord",
    "AssetDefinitionListPage",
    "AssetHolderRecord",
    "AssetHolderListPage",
    "AccountPermissionRecord",
    "AccountPermissionListPage",
    "SumeragiEvidenceRecord",
    "SumeragiEvidenceListPage",
    "SumeragiQcSummary",
    "SumeragiTxQueueStatus",
    "SumeragiEpochSchedule",
    "SumeragiRbcEviction",
    "SumeragiRbcStoreStatus",
    "SumeragiPrfStatus",
    "SumeragiStatusSnapshot",
    "SumeragiNewViewReceipt",
    "SumeragiNewViewSnapshot",
    "SumeragiRbcSnapshot",
    "SumeragiRbcSession",
    "SumeragiRbcSessionsSnapshot",
    "SumeragiRbcDeliveryStatus",
    "SumeragiCollectorEntry",
    "SumeragiCollectorPlan",
    "SumeragiParamsSnapshot",
    "SumeragiPacemakerSnapshot",
    "SumeragiPhasesEmaSnapshot",
    "SumeragiPhasesSnapshot",
    "SumeragiLeaderSnapshot",
    "SumeragiQcSnapshot",
    "SumeragiEvidenceCount",
    "TriggerRecord",
    "TriggerListPage",
    "PipelineDagSnapshot",
    "PipelineTxSnapshot",
    "PipelineRecoverySidecar",
    "RbcSample",
    "RbcChunkProof",
    "RbcMerkleProof",
    "RuntimeUpgradeCounters",
    "RuntimeMetrics",
    "RuntimeAbiActive",
    "RuntimeAbiHash",
    "RuntimeUpgradeStatus",
    "RuntimeUpgradeManifest",
    "RuntimeUpgradeRecord",
    "RuntimeUpgradeListItem",
    "RuntimeUpgradeListPage",
    "RuntimeInstruction",
    "RuntimeUpgradeActionResponse",
]


_DEFAULT_SUCCESS_STATUSES = frozenset({"Approved", "Committed", "Applied"})
_DEFAULT_FAILURE_STATUSES = frozenset({"Rejected", "Expired"})
_DEFAULT_RETRY_STATUSES = frozenset({502, 503, 504})
_DEFAULT_RETRY_METHODS = frozenset({"GET", "HEAD", "OPTIONS"})

try:  # pragma: no cover - optional dependency
    import websocket  # type: ignore[import-not-found]
except ImportError:  # pragma: no cover - optional dependency
    websocket = None  # type: ignore


class TransactionStatusError(RuntimeError):
    """Raised when a transaction reaches a terminal failure status."""

    def __init__(self, hash_hex: str, status: Optional[str], payload: Any) -> None:
        self.hash_hex = hash_hex
        self.status = status
        self.payload = payload
        status_repr = repr(status) if status is not None else "unknown"
        super().__init__(f"transaction {hash_hex} reported failure status {status_repr}")


class ToriiClient(_BaseToriiClient):
    """Convenience wrapper that exposes Torii attachment/prover APIs under `iroha_python`.

    The implementation delegates to :class:`iroha_torii_client.client.ToriiClient`
    so existing behaviour stays intact while we grow a richer, higher-level SDK.
    """

    # NOTE: `iroha_torii_client.client.ToriiClient` already implements the full
    # API surface. This subclass only exists to document the export location and
    # to leave room for SDK-specific conveniences (e.g., auth interceptors).
    def __init__(
        self,
        base_url: str,
        session: Optional[requests.Session] = None,
        *,
        auth_token: Optional[str] = None,
        api_token: Optional[str] = None,
        default_headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = 30.0,
        max_retries: int = 3,
        backoff_factor: float = 0.5,
        backoff_initial_ms: Optional[int] = None,
        max_backoff_ms: Optional[int] = None,
        backoff_multiplier: Optional[float] = None,
        retry_on_status: Optional[Sequence[int]] = None,
        retry_on_methods: Optional[Sequence[str]] = None,
        sorafs_alias_policy: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]] = None,
        sorafs_alias_warning: Optional[Callable[[SorafsAliasWarning], None]] = None,
        sorafs_alias_logger: Optional[logging.Logger] = None,
    ) -> None:
        super().__init__(base_url, session=session)
        self._timeout = timeout
        self._max_retries = max(0, int(max_retries))
        self._retry_statuses = (
            set(retry_on_status) if retry_on_status is not None else set(_DEFAULT_RETRY_STATUSES)
        )
        self._retry_methods = {
            method.upper()
            for method in (
                retry_on_methods if retry_on_methods is not None else _DEFAULT_RETRY_METHODS
            )
        }
        self._default_headers: Dict[str, str] = {"Accept": "application/json"}
        if default_headers:
            self._default_headers.update(default_headers)
        self._auth_token: Optional[str] = None
        self._api_token: Optional[str] = None
        self._status_state = _ToriiStatusState()
        if auth_token:
            self.set_auth_token(auth_token)
        if api_token:
            self.set_api_token(api_token)
        if backoff_initial_ms is not None or max_backoff_ms is not None or backoff_multiplier is not None:
            self._backoff_initial = max(0.0, (backoff_initial_ms or 0) / 1000.0)
            self._backoff_multiplier = max(1.0, backoff_multiplier if backoff_multiplier is not None else 2.0)
            if max_backoff_ms is None or max_backoff_ms <= 0:
                self._backoff_cap = math.inf
            else:
                self._backoff_cap = max(0.0, max_backoff_ms / 1000.0)
        else:
            self._backoff_initial = max(0.0, float(backoff_factor))
            self._backoff_multiplier = 2.0
            self._backoff_cap = math.inf
        self._sorafs_alias_policy = _normalize_sorafs_policy_config(sorafs_alias_policy)
        self._sorafs_alias_warning_hook = sorafs_alias_warning
        self._sorafs_alias_logger = sorafs_alias_logger or logging.getLogger(
            "iroha_python.sorafs.client"
        )
        self._sorafs_alias_metrics: Dict[str, int] = {}
        self._last_sorafs_alias_evaluation: Optional[SorafsAliasEvaluation] = None

    @property
    def sorafs_alias_policy(self) -> SorafsAliasPolicy:
        """Return the resolved SoraFS alias cache policy."""

        return self._sorafs_alias_policy

    def set_sorafs_alias_policy(
        self,
        policy: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]],
    ) -> None:
        """Override the SoraFS alias cache policy used for validation."""

        self._sorafs_alias_policy = _normalize_sorafs_policy_config(policy)
        self._sorafs_alias_metrics.clear()
        self._last_sorafs_alias_evaluation = None

    def set_sorafs_alias_warning(
        self, callback: Optional[Callable[[SorafsAliasWarning], None]]
    ) -> None:
        """Install a callback invoked when proofs enter the refresh window or require rotation."""

        self._sorafs_alias_warning_hook = callback

    def get_sorafs_alias_metrics(self) -> Dict[str, int]:
        """Return aggregate counters for alias proof evaluations."""

        return dict(self._sorafs_alias_metrics)

    def get_last_sorafs_alias_evaluation(self) -> Optional[SorafsAliasEvaluation]:
        """Return the most recent alias proof evaluation observed by the client, if any."""

        return self._last_sorafs_alias_evaluation

    def submit_transaction(self, payload: bytes) -> Optional[Any]:
        """Submit a Norito-encoded transaction payload to `/v1/pipeline/transactions`."""

        response = self._request(
            "POST",
            "/v1/pipeline/transactions",
            data=payload,
            headers={"Content-Type": "application/x-norito"},
        )
        self._expect_status(response, {200, 201, 202, 204})
        return type(self)._maybe_json(response)

    def submit_transaction_envelope(
        self, envelope: "SignedTransactionEnvelope"
    ) -> Optional[Any]:
        """Submit a transaction using a :class:`SignedTransactionEnvelope`."""

        payload = envelope.signed_transaction_versioned
        return self.submit_transaction(bytes(payload))

    def submit_transaction_draft(
        self,
        draft: "TransactionDraft",
        *,
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        instructions: Optional[Iterable["Instruction"]] = None,
        **sign_overrides: Any,
    ) -> tuple["SignedTransactionEnvelope", Optional[Any]]:
        """Sign a :class:`TransactionDraft` and submit it to Torii.

        Exactly one of ``private_key`` or ``private_key_hex`` must be provided. Additional
        keyword arguments are forwarded to :meth:`TransactionDraft.sign`, allowing callers to
        override fields such as ``creation_time_ms`` or ``ttl_ms``.
        """

        if (private_key is None) and (private_key_hex is None):
            raise ValueError("provide either `private_key` or `private_key_hex`")
        if private_key is not None and private_key_hex is not None:
            raise ValueError("provide only one of `private_key` or `private_key_hex`")

        envelope = self._sign_transaction_draft(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            instructions=instructions,
            **sign_overrides,
        )
        status = self.submit_transaction_envelope(envelope)
        return envelope, status

    def submit_transaction_json(self, envelope_json: str) -> Optional[Any]:
        """Submit a transaction described by the JSON produced via `to_json`."""

        envelope = signed_transaction_envelope_from_json(envelope_json)
        return self.submit_transaction_envelope(envelope)

    def submit_transaction_draft_and_wait(
        self,
        draft: "TransactionDraft",
        *,
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        instructions: Optional[Iterable["Instruction"]] = None,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
        **sign_overrides: Any,
    ) -> Any:
        """Sign a draft, submit it, and wait for the transaction to reach a terminal status."""

        envelope = self._sign_transaction_draft(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            instructions=instructions,
            **sign_overrides,
        )
        self.submit_transaction_envelope(envelope)
        return self.submit_transaction_envelope_and_wait(
            envelope,
            interval=interval,
            timeout=timeout,
            max_attempts=max_attempts,
            success_statuses=success_statuses,
            failure_statuses=failure_statuses,
            on_status=on_status,
        )

    @staticmethod
    def _sign_transaction_draft(
        draft: "TransactionDraft",
        *,
        private_key: Optional[bytes],
        private_key_hex: Optional[str],
        instructions: Optional[Iterable["Instruction"]],
        **sign_overrides: Any,
    ) -> "SignedTransactionEnvelope":
        if private_key is None and private_key_hex is None:
            raise ValueError("provide either `private_key` or `private_key_hex`")
        if private_key is not None and private_key_hex is not None:
            raise ValueError("provide only one of `private_key` or `private_key_hex`")

        if private_key_hex is not None:
            return draft.sign_hex_private_key(
                private_key_hex,
                instructions=instructions,
                **sign_overrides,
            )
        assert private_key is not None
        return draft.sign(
            private_key,
            instructions=instructions,
            **sign_overrides,
        )

    def submit_transaction_json_and_wait(
        self,
        envelope_json: str,
        *,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
    ) -> Any:
        """Submit a transaction JSON blob and wait for final status."""

        envelope = signed_transaction_envelope_from_json(envelope_json)
        return self.submit_transaction_envelope_and_wait(
            envelope,
            interval=interval,
            timeout=timeout,
            max_attempts=max_attempts,
            success_statuses=success_statuses,
            failure_statuses=failure_statuses,
            on_status=on_status,
        )

    def submit_transaction_envelope_and_wait(
        self,
        envelope: "SignedTransactionEnvelope",
        *,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
    ) -> Any:
        """Submit a signed transaction and wait for its terminal status."""

        self.submit_transaction_envelope(envelope)

        hash_field = getattr(envelope, "hash", None)
        if hash_field is None:
            raise ValueError("SignedTransactionEnvelope.hash is required to poll status")
        if isinstance(hash_field, memoryview):
            hash_field = hash_field.tobytes()
        if isinstance(hash_field, (bytes, bytearray)):
            hash_hex = bytes(hash_field).hex()
        elif isinstance(hash_field, str):
            hash_hex = hash_field
        else:
            raise TypeError(
                "SignedTransactionEnvelope.hash must be bytes or hex string, "
                f"got {type(hash_field)!r}"
            )

        return self.wait_for_transaction_status(
            hash_hex,
            interval=interval,
            timeout=timeout,
            max_attempts=max_attempts,
            success_statuses=success_statuses,
            failure_statuses=failure_statuses,
            on_status=on_status,
        )

    # ------------------------------------------------------------------
    # HTTP helper utilities
    # ------------------------------------------------------------------
    def set_auth_token(self, token: Optional[str]) -> None:
        """Configure (or clear) the Authorization bearer token."""

        if token:
            self._auth_token = token
            self._default_headers["Authorization"] = f"Bearer {token}"
        else:
            self._auth_token = None
            self._default_headers.pop("Authorization", None)

    def set_api_token(self, token: Optional[str]) -> None:
        """Configure (or clear) the Torii `X-API-Token` header."""

        if token:
            self._api_token = token
            self._default_headers["X-API-Token"] = token
        else:
            self._api_token = None
            self._default_headers.pop("X-API-Token", None)

    def update_default_headers(self, headers: Mapping[str, str]) -> None:
        """Merge `headers` into the default header set applied to every request."""

        self._default_headers.update(headers)

    def request_json(
        self,
        method: str,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[Mapping[str, str]] = None,
        json_body: Optional[Mapping[str, Any]] = None,
        data: Optional[bytes] = None,
        expected_status: Sequence[int] = (200,),
        timeout: Optional[float] = None,
        allow_retry: bool = True,
    ) -> Optional[Any]:
        """Issue an HTTP request and decode the JSON payload when present."""

        response = self._request(
            method,
            path,
            params=params,
            headers=headers,
            json_body=json_body,
            data=data,
            timeout=timeout,
            allow_retry=allow_retry,
        )
        self._expect_status(response, expected_status)
        return self._maybe_json(response)

    # -------------------------
    # Explorer APIs
    # -------------------------

    def get_explorer_metrics(self) -> Optional[Any]:
        """Fetch `/v1/explorer/metrics`. Returns `None` when telemetry is gated."""

        response = self._request(
            "GET",
            "/v1/explorer/metrics",
            headers={"Accept": "application/json"},
            allow_retry=True,
        )
        if response.status_code in {403, 404, 503}:
            return None
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    def get_explorer_metrics_typed(self) -> Optional[ExplorerMetricsSnapshot]:
        """Typed wrapper for :meth:`get_explorer_metrics`."""

        payload = self.get_explorer_metrics()
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("explorer metrics endpoint returned non-object payload")
        return ExplorerMetricsSnapshot.from_payload(payload)

    def get_explorer_account_qr(
        self,
        account_id: str,
        *,
        address_format: Optional[str] = None,
    ) -> Mapping[str, Any]:
        """Fetch explorer QR metadata via `GET /v1/explorer/accounts/{account_id}/qr`."""

        params: Dict[str, Any] = {}
        normalized_format = _normalize_address_format(address_format)
        if normalized_format is not None:
            params["address_format"] = normalized_format
        payload = self.request_json(
            "GET",
            f"/v1/explorer/accounts/{account_id}/qr",
            params=params or None,
            headers={"Accept": "application/json"},
            expected_status=(200,),
        )
        if payload is None:
            raise RuntimeError("explorer account qr endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("explorer account qr response must be a JSON object")
        return payload

    def get_explorer_account_qr_typed(
        self,
        account_id: str,
        *,
        address_format: Optional[str] = None,
    ) -> ExplorerAccountQrSnapshot:
        """Typed QR wrapper for :meth:`get_explorer_account_qr`."""

        payload = self.get_explorer_account_qr(
            account_id,
            address_format=address_format,
        )
        return ExplorerAccountQrSnapshot.from_payload(payload)

    # -------------------------
    # ISO 20022 bridge APIs
    # -------------------------

    def _submit_iso_message(
        self,
        path: str,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str],
        timeout: Optional[float],
        context: str,
    ) -> Optional[Any]:
        payload = _normalize_iso_payload(message, f"{context}.message")
        headers = {
            "Content-Type": content_type.strip() if isinstance(content_type, str) and content_type.strip() else "application/xml",
            "Accept": "application/json",
        }
        response = self._request(
            "POST",
            path,
            data=payload,
            headers=headers,
            timeout=timeout,
        )
        self._expect_status(response, (202,))
        return self._maybe_json(response)

    def submit_iso_pacs008(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Submit a pacs.008 payload (`POST /v1/iso20022/pacs008`)."""

        return self._submit_iso_message(
            "/v1/iso20022/pacs008",
            message,
            content_type=content_type,
            timeout=timeout,
            context="submit_iso_pacs008",
        )

    def submit_iso_pacs008_typed(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Optional[IsoSubmissionRecord]:
        """Typed wrapper for :meth:`submit_iso_pacs008`."""

        payload = self.submit_iso_pacs008(
            message,
            content_type=content_type,
            timeout=timeout,
        )
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("ISO pacs.008 submission returned a non-object payload")
        return IsoSubmissionRecord.from_payload(payload, context="iso pacs.008 submission")

    def submit_iso_pacs009(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Submit a pacs.009 payload (`POST /v1/iso20022/pacs009`)."""

        return self._submit_iso_message(
            "/v1/iso20022/pacs009",
            message,
            content_type=content_type,
            timeout=timeout,
            context="submit_iso_pacs009",
        )

    def submit_iso_pacs009_typed(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Optional[IsoSubmissionRecord]:
        """Typed wrapper for :meth:`submit_iso_pacs009`."""

        payload = self.submit_iso_pacs009(
            message,
            content_type=content_type,
            timeout=timeout,
        )
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("ISO pacs.009 submission returned a non-object payload")
        return IsoSubmissionRecord.from_payload(payload, context="iso pacs.009 submission")

    def get_iso_message_status(
        self,
        message_id: str,
        *,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Fetch ISO bridge status via `GET /v1/iso20022/status/{message_id}`."""

        normalized_id = _require_non_empty_string(message_id, "message_id")
        encoded_id = quote(normalized_id, safe="")
        response = self._request(
            "GET",
            f"/v1/iso20022/status/{encoded_id}",
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    def get_iso_message_status_typed(
        self,
        message_id: str,
        *,
        timeout: Optional[float] = None,
    ) -> Optional[IsoSubmissionRecord]:
        """Typed wrapper for :meth:`get_iso_message_status`."""

        payload = self.get_iso_message_status(message_id, timeout=timeout)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("ISO status endpoint returned a non-object payload")
        return IsoSubmissionRecord.from_payload(payload, context="iso status response")

    def wait_for_iso_message_status(
        self,
        message_id: str,
        *,
        poll_interval: float = _DEFAULT_ISO_POLL_INTERVAL_SECONDS,
        max_attempts: int = _DEFAULT_ISO_WAIT_ATTEMPTS,
        resolve_on_accepted: bool = False,
        timeout: Optional[float] = None,
        on_poll: Optional[Callable[[Optional[IsoSubmissionRecord], int], None]] = None,
    ) -> IsoSubmissionRecord:
        """Poll the ISO bridge until the message reaches a terminal state."""

        normalized_id = _require_non_empty_string(message_id, "message_id")
        if poll_interval < 0.0:
            raise ValueError("poll_interval must be non-negative")
        if 0.0 < poll_interval < _MIN_ISO_POLL_INTERVAL_SECONDS:
            poll_interval = _MIN_ISO_POLL_INTERVAL_SECONDS
        if max_attempts <= 0:
            raise ValueError("max_attempts must be positive")
        if on_poll is not None and not callable(on_poll):
            raise TypeError("wait.on_poll must be callable when provided")

        attempts = 0
        last_status: Optional[IsoSubmissionRecord] = None
        while True:
            attempts += 1
            status_payload = self.get_iso_message_status_typed(normalized_id, timeout=timeout)
            last_status = status_payload
            if on_poll is not None:
                on_poll(status_payload, attempts)
            if status_payload and _is_iso_status_terminal(status_payload, resolve_on_accepted):
                return status_payload
            if attempts >= max_attempts:
                raise IsoMessageTimeoutError(normalized_id, attempts, last_status)
            if poll_interval > 0.0:
                time.sleep(poll_interval)

    def submit_iso_pacs008_and_wait(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
        wait: Optional[Mapping[str, Any]] = None,
    ) -> IsoSubmissionRecord:
        """Submit a pacs.008 payload and wait for a terminal status."""

        submission = self.submit_iso_pacs008_typed(
            message,
            content_type=content_type,
            timeout=timeout,
        )
        if submission is None:
            raise RuntimeError("ISO pacs.008 submission did not return a message_id")
        wait_kwargs = _normalize_iso_wait_kwargs(wait, context="submit_iso_pacs008_and_wait.wait")
        return self.wait_for_iso_message_status(submission.message_id, **wait_kwargs)

    def submit_iso_pacs009_and_wait(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
        wait: Optional[Mapping[str, Any]] = None,
    ) -> IsoSubmissionRecord:
        """Submit a pacs.009 payload and wait for a terminal status."""

        submission = self.submit_iso_pacs009_typed(
            message,
            content_type=content_type,
            timeout=timeout,
        )
        if submission is None:
            raise RuntimeError("ISO pacs.009 submission did not return a message_id")
        wait_kwargs = _normalize_iso_wait_kwargs(wait, context="submit_iso_pacs009_and_wait.wait")
        return self.wait_for_iso_message_status(submission.message_id, **wait_kwargs)

    # ------------------------------------------------------------------
    # Repo agreements
    # ------------------------------------------------------------------

    def list_repo_agreements(self, **params: Any) -> RepoAgreementListPage:
        """List repo agreements (`GET /v1/repo/agreements`)."""

        response = self._request(
            "GET",
            "/v1/repo/agreements",
            params=self._clean_params(params),
        )
        self._expect_status(response, (200,))
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("repo agreements endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("repo agreements endpoint returned malformed payload")
        return RepoAgreementListPage.from_payload(payload)

    def query_repo_agreements(self, envelope: Mapping[str, Any]) -> RepoAgreementListPage:
        """Query repo agreements (`POST /v1/repo/agreements/query`)."""

        payload = self._post_json(
            "/v1/repo/agreements/query",
            dict(envelope),
            context="repo agreements query",
        )
        return RepoAgreementListPage.from_payload(payload)

    # ------------------------------------------------------------------
    # Offline allowances, transfers, and summaries
    # ------------------------------------------------------------------

    def list_offline_allowances(self, **params: Any) -> OfflineAllowanceListPage:
        """List offline allowances (`GET /v1/offline/allowances`)."""

        normalized = dict(params)
        _apply_address_format_alias(normalized)
        return super().list_offline_allowances(**normalized)

    def list_offline_transfers(self, **params: Any) -> OfflineTransferListPage:
        """List offline transfers (`GET /v1/offline/transfers`)."""

        normalized = dict(params)
        _apply_address_format_alias(normalized)
        return super().list_offline_transfers(**normalized)

    def list_offline_summaries(self, **params: Any) -> OfflineSummaryListPage:
        """List offline counter summaries (`GET /v1/offline/summaries`)."""

        normalized = dict(params)
        _apply_address_format_alias(normalized)
        return super().list_offline_summaries(**normalized)

    def get_sorafs_pin_manifest(
        self,
        digest_hex: str,
        *,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Optional[Any]:
        """Fetch a SoraFS pin manifest (`GET /v1/sorafs/pin/{digest}`) enforcing alias policy."""

        if not isinstance(digest_hex, str) or not digest_hex.strip():
            raise ValueError("digest_hex must be a non-empty string")
        response = self._request(
            "GET",
            f"/v1/sorafs/pin/{digest_hex}",
            headers=headers,
        )
        self._expect_status(response, (200,))
        return type(self)._maybe_json(response)

    # -------------------------
    # SoraFS Proof-of-Retrievability APIs
    # -------------------------

    def record_sorafs_por_challenge(
        self,
        *,
        challenge: Optional[Union[bytes, bytearray, memoryview]] = None,
        challenge_b64: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> SorafsPorSubmissionResponse:
        """Submit a `PorChallengeV1` record (auditor/governance only)."""

        payload = {
            "challenge_b64": _normalize_base64_payload(
                challenge_b64, challenge, "record_sorafs_por_challenge.challenge"
            )
        }
        response = self._request(
            "POST",
            "/v1/sorafs/capacity/por-challenge",
            json_body=payload,
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        body = self._maybe_json(response)
        if body is None:
            raise RuntimeError("por-challenge endpoint returned an empty payload")
        return SorafsPorSubmissionResponse.from_payload(body, "sorafs_por_challenge")

    def record_sorafs_por_proof(
        self,
        *,
        proof: Optional[Union[bytes, bytearray, memoryview]] = None,
        proof_b64: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> SorafsPorSubmissionResponse:
        """Submit a `PorProofV1` record for a provider."""

        payload = {
            "proof_b64": _normalize_base64_payload(
                proof_b64, proof, "record_sorafs_por_proof.proof"
            )
        }
        response = self._request(
            "POST",
            "/v1/sorafs/capacity/por-proof",
            json_body=payload,
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        body = self._maybe_json(response)
        if body is None:
            raise RuntimeError("por-proof endpoint returned an empty payload")
        return SorafsPorSubmissionResponse.from_payload(body, "sorafs_por_proof")

    def record_sorafs_por_verdict(
        self,
        *,
        verdict: Optional[Union[bytes, bytearray, memoryview]] = None,
        verdict_b64: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> SorafsPorVerdictResponse:
        """Submit an audit verdict for a PoR challenge."""

        payload = {
            "verdict_b64": _normalize_base64_payload(
                verdict_b64, verdict, "record_sorafs_por_verdict.verdict"
            )
        }
        response = self._request(
            "POST",
            "/v1/sorafs/capacity/por-verdict",
            json_body=payload,
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        body = self._maybe_json(response)
        if body is None:
            raise RuntimeError("por-verdict endpoint returned an empty payload")
        return SorafsPorVerdictResponse.from_payload(body, "sorafs_por_verdict")

    def submit_sorafs_por_observation(
        self,
        success: bool,
        *,
        timeout: Optional[float] = None,
    ) -> SorafsPorObservationResponse:
        """Record the outcome of a PoR observation probe."""

        payload = {"success": _coerce_bool_flag(success, "submit_sorafs_por_observation.success")}
        response = self._request(
            "POST",
            "/v1/sorafs/capacity/por",
            json_body=payload,
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        body = self._maybe_json(response)
        if body is None:
            raise RuntimeError("por observation endpoint returned an empty payload")
        return SorafsPorObservationResponse.from_payload(body, "sorafs_por_observation")

    def get_sorafs_por_status(
        self,
        *,
        manifest_hex: Optional[str] = None,
        provider_hex: Optional[str] = None,
        epoch: Optional[int] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        page_token_hex: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> bytes:
        """Return Norito-encoded `PorChallengeStatusV1` records for the given filters."""

        params = _build_sorafs_por_status_params(
            manifest_hex, provider_hex, epoch, status, limit, page_token_hex
        )
        response = self._request(
            "GET",
            "/v1/sorafs/por/status",
            params=params,
            headers={"Accept": "application/x-norito"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        return response.content

    def export_sorafs_por_status(
        self,
        *,
        start_epoch: Optional[int] = None,
        end_epoch: Optional[int] = None,
        timeout: Optional[float] = None,
    ) -> bytes:
        """Return a Norito-exported history for the supplied epoch range."""

        params = _build_sorafs_por_export_params(start_epoch, end_epoch)
        response = self._request(
            "GET",
            "/v1/sorafs/por/export",
            params=params,
            headers={"Accept": "application/x-norito"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        return response.content

    def get_sorafs_por_weekly_report(
        self,
        iso_week: Union[str, Tuple[int, int]],
        *,
        timeout: Optional[float] = None,
    ) -> bytes:
        """Fetch the Norito-encoded weekly PoR report for the provided ISO week."""

        label = _normalize_iso_week_label(iso_week, "get_sorafs_por_weekly_report.iso_week")
        response = self._request(
            "GET",
            f"/v1/sorafs/por/report/{label}",
            headers={"Accept": "application/x-norito"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        return response.content

    def get_sorafs_por_ingestion_status(
        self,
        manifest_hex: str,
        *,
        timeout: Optional[float] = None,
    ) -> SorafsPorIngestionStatus:
        """Return the JSON PoR ingestion snapshot for the provided manifest digest."""

        digest = _normalize_hex_string(
            manifest_hex,
            "get_sorafs_por_ingestion_status.manifest_hex",
            expected_length=64,
        )
        response = self._request(
            "GET",
            f"/v1/sorafs/por/ingestion/{digest}",
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        payload = self._maybe_json(response)
        if payload is None or not isinstance(payload, Mapping):
            raise RuntimeError("por ingestion endpoint returned an invalid payload")
        return SorafsPorIngestionStatus.from_payload(payload)

    @staticmethod
    def _pagination_params(
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        return params

    def get_status(self) -> Optional[Any]:
        """Return Torii node status (`GET /v1/status`)."""

        return self.request_json("GET", "/v1/status", expected_status=(200,))

    def get_status_snapshot_typed(self) -> ToriiStatusSnapshot:
        """Return a typed status snapshot together with derived metrics."""

        payload = self.request_json("GET", "/v1/status", expected_status=(200,))
        if payload is None:
            raise TypeError("status response body was empty")
        if not isinstance(payload, Mapping):
            raise TypeError("status response must be a JSON object")
        status_payload = ToriiStatusPayload.from_payload(payload)
        metrics = self._status_state.record(status_payload)
        return ToriiStatusSnapshot(
            timestamp=time.monotonic(),
            status=status_payload,
            metrics=metrics,
        )

    def get_health(self) -> Optional[Any]:
        """Return Torii health information (`GET /v1/health`)."""

        return self.request_json("GET", "/v1/health", expected_status=(200,))

    def get_configuration(self) -> Mapping[str, Any]:
        """Return the current node configuration as a JSON-compatible mapping."""

        snapshot = self.get_configuration_typed()
        return _configuration_snapshot_to_dict(snapshot)

    def get_configuration_typed(self) -> ConfigurationSnapshot:
        """Typed configuration snapshot (`GET /v1/configuration`)."""

        return super().get_configuration()

    def get_confidential_gas_schedule(self) -> Optional[Mapping[str, int]]:
        """Return the confidential verification gas schedule as a mapping, when available."""

        schedule = self.get_confidential_gas_schedule_typed()
        if schedule is None:
            return None
        return schedule.to_payload()

    def get_confidential_gas_schedule_typed(self) -> Optional[ConfidentialGasSchedule]:
        """Typed confidential verification gas schedule."""

        snapshot = self.get_configuration_typed()
        return snapshot.confidential_gas

    def set_confidential_gas_schedule(
        self,
        *,
        proof_base: int,
        per_public_input: int,
        per_proof_byte: int,
        per_nullifier: int,
        per_commitment: int,
    ) -> Optional[Any]:
        """Update the node's confidential verification gas schedule.

        Torii requires the current logger configuration to be supplied alongside updates.
        This helper fetches the latest configuration, reuses the existing ``logger`` section,
        and posts the new ``confidential_gas`` payload.
        """

        schedule = ConfidentialGasSchedule(
            proof_base=int(proof_base),
            per_public_input=int(per_public_input),
            per_proof_byte=int(per_proof_byte),
            per_nullifier=int(per_nullifier),
            per_commitment=int(per_commitment),
        )
        snapshot = self.get_configuration_typed()
        payload = {
            "logger": snapshot.logger.to_payload(),
            "confidential_gas": schedule.to_payload(),
        }
        return self.update_configuration(payload)

    def set_network_gossip_config(
        self,
        *,
        block_gossip_size: int,
        block_gossip_period_ms: int,
        transaction_gossip_size: int,
        transaction_gossip_period_ms: int,
    ) -> Mapping[str, Any]:
        """Update Torii gossip fan-out and interval parameters.

        The helper fetches the latest configuration, preserves the existing logger/queue/gas sections,
        and posts the updated `network` payload so PY6 admin-surface evidence can remain deterministic.
        """

        snapshot = self.get_configuration_typed()
        payload = _configuration_update_payload(snapshot)
        payload["network"] = {
            "block_gossip_size": _normalize_positive_int(
                block_gossip_size, "network.block_gossip_size", allow_zero=False
            ),
            "block_gossip_period_ms": _normalize_positive_int(
                block_gossip_period_ms, "network.block_gossip_period_ms", allow_zero=False
            ),
            "transaction_gossip_size": _normalize_positive_int(
                transaction_gossip_size, "network.transaction_gossip_size", allow_zero=False
            ),
            "transaction_gossip_period_ms": _normalize_positive_int(
                transaction_gossip_period_ms, "network.transaction_gossip_period_ms", allow_zero=False
            ),
        }
        return self.update_configuration(payload)

    def set_queue_capacity(self, *, capacity: int) -> Mapping[str, Any]:
        """Update the transaction queue capacity exposed by `/v1/configuration`.

        The payload reuses the current logger/network/confidential gas configuration so the queue
        change mirrors the node's existing state.
        """

        snapshot = self.get_configuration_typed()
        payload = _configuration_update_payload(snapshot)
        payload["queue"] = {
            "capacity": _normalize_positive_int(capacity, "queue.capacity", allow_zero=False)
        }
        return self.update_configuration(payload)

    def update_configuration(self, payload: Mapping[str, Any]) -> Mapping[str, Any]:
        """Update node configuration (`POST /v1/configuration`)."""

        return super().update_configuration(payload)

    def get_metrics(self, *, as_text: bool = False) -> Optional[Any]:
        """Fetch Torii metrics (`GET /v1/metrics`)."""

        if as_text:
            response = self._request(
                "GET",
                "/v1/metrics",
                headers={"Accept": "text/plain"},
                allow_retry=False,
            )
            self._expect_status(response, {200})
            return response.text
        return self.request_json("GET", "/v1/metrics", expected_status=(200,))

    def get_block(self, height: int) -> Optional[Any]:
        """Fetch a block by height (`GET /v1/blocks/{height}`)."""

        return self.request_json("GET", f"/v1/blocks/{height}", expected_status=(200, 404))

    def list_blocks(
        self,
        *,
        offset_height: Optional[int] = None,
        limit: Optional[int] = None,
    ) -> Optional[Any]:
        """List blocks via `GET /v1/blocks` with optional pagination."""

        params: Dict[str, Any] = {}
        if offset_height is not None:
            params["offset_height"] = int(offset_height)
        if limit is not None:
            params["limit"] = int(limit)
        return self.request_json("GET", "/v1/blocks", params=params or None, expected_status=(200,))

    def get_pipeline_recovery(self, height: int) -> Optional[Any]:
        """Fetch pipeline recovery sidecar for `height` (`GET /v1/pipeline/recovery/{height}`)."""

        response = self._request(
            "GET",
            f"/v1/pipeline/recovery/{int(height)}",
        )
        self._expect_status(response, {200, 404})
        return self._maybe_json(response)

    def get_pipeline_recovery_typed(self, height: int) -> Optional[PipelineRecoverySidecar]:
        """Typed wrapper for :meth:`get_pipeline_recovery`."""

        payload = self.get_pipeline_recovery(height)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline recovery response must be a JSON object")
        return PipelineRecoverySidecar.from_payload(payload)

    def list_peers(self) -> Optional[Any]:
        """List currently online peers (`GET /v1/peers`)."""

        return self.request_json("GET", "/v1/peers", expected_status=(200,))

    def list_peers_typed(self) -> List[PeerInfo]:
        """Return the online peer set as `PeerInfo` structures (`GET /v1/peers`)."""

        payload = self.request_json("GET", "/v1/peers", expected_status=(200,))
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise TypeError("expected list payload from /v1/peers")
        peers: List[PeerInfo] = []
        for entry in payload:
            peers.append(PeerInfo.from_payload(entry))
        return peers

    def list_telemetry_peers_info(self) -> Optional[Any]:
        """Return telemetry metadata from `GET /v1/telemetry/peers-info`."""

        return self.request_json(
            "GET",
            "/v1/telemetry/peers-info",
            expected_status=(200,),
        )

    def list_telemetry_peers_info_typed(self) -> List[PeerTelemetryInfo]:
        """Typed wrapper for :meth:`list_telemetry_peers_info`."""

        payload = self.list_telemetry_peers_info()
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise TypeError("/v1/telemetry/peers-info response must be a list")
        entries: List[PeerTelemetryInfo] = []
        for index, entry in enumerate(payload):
            if not isinstance(entry, Mapping):
                raise TypeError(f"telemetry peers[{index}] must be an object")
            entries.append(PeerTelemetryInfo.from_payload(entry))
        return entries

    def list_kaigi_relays(self) -> Optional[Any]:
        """List registered Kaigi relays (`GET /v1/kaigi/relays`)."""

        response = self._request(
            "GET",
            "/v1/kaigi/relays",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    def list_kaigi_relays_typed(self) -> KaigiRelaySummaryList:
        """Typed wrapper for :meth:`list_kaigi_relays`."""

        payload = self.list_kaigi_relays()
        if payload is None:
            raise RuntimeError("kaigi relays endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relays response must be a JSON object")
        return KaigiRelaySummaryList.from_payload(payload)

    def get_kaigi_relay(self, relay_id: str) -> Optional[Any]:
        """Fetch metadata for a specific Kaigi relay (`GET /v1/kaigi/relays/{relay_id}`)."""

        relay_literal = _require_non_empty_string(relay_id, "relay_id")
        response = self._request(
            "GET",
            f"/v1/kaigi/relays/{relay_literal}",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, (200, 404))
        if response.status_code == 404:
            return None
        payload = self._maybe_json(response)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay detail response must be an object")
        return payload

    def get_kaigi_relay_typed(self, relay_id: str) -> Optional[KaigiRelayDetail]:
        """Typed wrapper for :meth:`get_kaigi_relay`."""

        payload = self.get_kaigi_relay(relay_id)
        if payload is None:
            return None
        return KaigiRelayDetail.from_payload(payload)

    def get_kaigi_relays_health(self) -> Optional[Any]:
        """Fetch aggregated Kaigi relay health metrics (`GET /v1/kaigi/relays/health`)."""

        response = self._request(
            "GET",
            "/v1/kaigi/relays/health",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    def get_kaigi_relays_health_typed(self) -> KaigiRelayHealthSnapshot:
        """Typed wrapper for :meth:`get_kaigi_relays_health`."""

        payload = self.get_kaigi_relays_health()
        if payload is None:
            raise RuntimeError("kaigi relays health endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relays health response must be an object")
        return KaigiRelayHealthSnapshot.from_payload(payload)

    def get_time_now(self) -> Mapping[str, int]:
        """Return the Network Time Service snapshot as a mapping."""

        snapshot = self.get_time_now_typed()
        return _network_time_snapshot_to_dict(snapshot)

    def get_time_now_typed(self) -> NetworkTimeSnapshot:
        """Typed Network Time Service snapshot."""

        return super().get_time_now()

    def get_time_status(self) -> Mapping[str, Any]:
        """Return Network Time Service diagnostics as a mapping."""

        status = self.get_time_status_typed()
        return _network_time_status_to_dict(status)

    def get_time_status_typed(self) -> NetworkTimeStatus:
        """Typed Network Time Service diagnostics."""

        return super().get_time_status()

    def capture_node_admin_snapshot(
        self,
        *,
        include_peer_telemetry: bool = True,
    ) -> NodeAdminSnapshot:
        """Collect `/v1/configuration`, `/v1/peers`, `/v1/time/*`, and `/v1/node/capabilities` evidence.

        When ``include_peer_telemetry`` is true (default) the helper also fetches
        `/v1/telemetry/peers-info` so roadmap item PY6-P5 can record peer
        instrumentation alongside the core admin surfaces.
        """

        configuration = self.get_configuration_typed()
        peers = self.list_peers_typed()
        time_status = self.get_time_status_typed()
        time_now = self.get_time_now_typed()
        node_capabilities = self.get_node_capabilities_typed()
        telemetry_peers: Optional[List[PeerTelemetryInfo]]
        if include_peer_telemetry:
            telemetry_peers = self.list_telemetry_peers_info_typed()
        else:
            telemetry_peers = None
        return NodeAdminSnapshot(
            configuration=configuration,
            peers=peers,
            time_now=time_now,
            time_status=time_status,
            node_capabilities=node_capabilities,
            telemetry_peers=telemetry_peers,
        )

    # ------------------------------------------------------------------
    # Runtime & admission helpers
    # ------------------------------------------------------------------
    def get_node_capabilities(self) -> Optional[Any]:
        """Fetch node capability advert (`GET /v1/node/capabilities`)."""

        return self.request_json(
            "GET",
            "/v1/node/capabilities",
            expected_status=(200,),
        )

    def get_node_capabilities_typed(self) -> NodeCapabilities:
        """Typed wrapper for :meth:`get_node_capabilities`."""

        payload = self.get_node_capabilities()
        if payload is None:
            raise RuntimeError("node capabilities endpoint returned no payload")
        return NodeCapabilities.from_payload(payload)

    def get_runtime_metrics(self) -> Optional[Any]:
        """Fetch runtime upgrade metrics summary (`GET /v1/runtime/metrics`)."""

        return self.request_json(
            "GET",
            "/v1/runtime/metrics",
            expected_status=(200,),
        )

    def get_runtime_metrics_typed(self) -> RuntimeMetrics:
        """Typed wrapper for :meth:`get_runtime_metrics`."""

        payload = self.get_runtime_metrics()
        if payload is None:
            raise RuntimeError("runtime metrics endpoint returned no payload")
        return RuntimeMetrics.from_payload(payload)

    def get_runtime_abi_active(self) -> Optional[Any]:
        """Fetch the active ABI versions and default compile target (`GET /v1/runtime/abi/active`)."""

        return self.request_json(
            "GET",
            "/v1/runtime/abi/active",
            expected_status=(200,),
        )

    def get_runtime_abi_active_typed(self) -> RuntimeAbiActive:
        """Typed wrapper for :meth:`get_runtime_abi_active`."""

        payload = self.get_runtime_abi_active()
        if payload is None:
            raise RuntimeError("runtime ABI active endpoint returned no payload")
        return RuntimeAbiActive.from_payload(payload)

    def get_runtime_abi_hash(self) -> Optional[Any]:
        """Fetch the canonical ABI hash for the node's active policy (`GET /v1/runtime/abi/hash`)."""

        return self.request_json(
            "GET",
            "/v1/runtime/abi/hash",
            expected_status=(200,),
        )

    def get_runtime_abi_hash_typed(self) -> RuntimeAbiHash:
        """Typed wrapper for :meth:`get_runtime_abi_hash`."""

        payload = self.get_runtime_abi_hash()
        if payload is None:
            raise RuntimeError("runtime ABI hash endpoint returned no payload")
        return RuntimeAbiHash.from_payload(payload)

    def list_runtime_upgrades(self) -> Optional[Any]:
        """List runtime upgrade records (`GET /v1/runtime/upgrades`)."""

        return self.request_json(
            "GET",
            "/v1/runtime/upgrades",
            expected_status=(200,),
        )

    def list_runtime_upgrades_typed(self) -> RuntimeUpgradeListPage:
        """Typed wrapper for :meth:`list_runtime_upgrades`."""

        payload = self.list_runtime_upgrades()
        if payload is None:
            return RuntimeUpgradeListPage(items=[], total=0)
        return RuntimeUpgradeListPage.from_payload(payload)

    def propose_runtime_upgrade(self, manifest: Mapping[str, Any]) -> Dict[str, Any]:
        """Wrap a runtime upgrade manifest into instructions (`POST /v1/runtime/upgrades/propose`)."""

        response = self._request(
            "POST",
            "/v1/runtime/upgrades/propose",
            json_body=dict(manifest),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected runtime upgrade proposal response")
        return payload

    def propose_runtime_upgrade_typed(
        self, manifest: Union[RuntimeUpgradeManifest, Mapping[str, Any]]
    ) -> RuntimeUpgradeActionResponse:
        """Typed wrapper for :meth:`propose_runtime_upgrade`."""

        manifest_payload: Mapping[str, Any]
        if isinstance(manifest, RuntimeUpgradeManifest):
            manifest_payload = manifest.to_payload()
        else:
            manifest_payload = manifest
        payload = self.propose_runtime_upgrade(manifest_payload)
        return RuntimeUpgradeActionResponse.from_payload(payload)

    def activate_runtime_upgrade(self, upgrade_id_hex: str) -> Dict[str, Any]:
        """Generate activation instructions for a runtime upgrade (`POST /v1/runtime/upgrades/activate/{id}`)."""

        path = f"/v1/runtime/upgrades/activate/{upgrade_id_hex.strip()}"
        response = self._request(
            "POST",
            path,
            headers={"Content-Type": "application/json"},
            data=b"",
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected runtime upgrade activation response")
        return payload

    def activate_runtime_upgrade_typed(self, upgrade_id_hex: str) -> RuntimeUpgradeActionResponse:
        """Typed wrapper for :meth:`activate_runtime_upgrade`."""

        payload = self.activate_runtime_upgrade(upgrade_id_hex)
        return RuntimeUpgradeActionResponse.from_payload(payload)

    def cancel_runtime_upgrade(self, upgrade_id_hex: str) -> Dict[str, Any]:
        """Generate cancellation instructions for a runtime upgrade (`POST /v1/runtime/upgrades/cancel/{id}`)."""

        path = f"/v1/runtime/upgrades/cancel/{upgrade_id_hex.strip()}"
        response = self._request(
            "POST",
            path,
            headers={"Content-Type": "application/json"},
            data=b"",
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected runtime upgrade cancellation response")
        return payload

    def cancel_runtime_upgrade_typed(self, upgrade_id_hex: str) -> RuntimeUpgradeActionResponse:
        """Typed wrapper for :meth:`cancel_runtime_upgrade`."""

        payload = self.cancel_runtime_upgrade(upgrade_id_hex)
        return RuntimeUpgradeActionResponse.from_payload(payload)

    def list_account_assets(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Optional[Any]:
        """List account assets via `GET /v1/accounts/{account_id}/assets`."""

        params = self._pagination_params(limit=limit, offset=offset)
        return self.request_json(
            "GET",
            f"/v1/accounts/{account_id}/assets",
            params=params or None,
            expected_status=(200,),
        )

    def list_account_assets_typed(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> AccountAssetsPage:
        """Typed wrapper for :meth:`list_account_assets`."""

        payload = self.list_account_assets(account_id, limit=limit, offset=offset)
        if payload is None:
            raise RuntimeError("account assets endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("account assets response must be a JSON object")
        return AccountAssetsPage.from_payload(payload)

    def list_account_transactions(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Optional[Any]:
        """List account transactions via `GET /v1/accounts/{account_id}/transactions`."""

        params = self._pagination_params(limit=limit, offset=offset)
        return self.request_json(
            "GET",
            f"/v1/accounts/{account_id}/transactions",
            params=params or None,
            expected_status=(200,),
        )

    def list_account_transactions_typed(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> AccountTransactionsPage:
        """Typed wrapper for :meth:`list_account_transactions`."""

        payload = self.list_account_transactions(account_id, limit=limit, offset=offset)
        if payload is None:
            raise RuntimeError("account transactions endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("account transactions response must be a JSON object")
        return AccountTransactionsPage.from_payload(payload)

    def query_account_assets(
        self,
        account_id: str,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
        envelope: Optional[Mapping[str, Any]] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/accounts/{account_id}/assets/query` with a Norito-style envelope."""

        if envelope is not None:
            self._ensure_no_query_args(
                envelope=envelope,
                filter=filter,
                sort=sort,
                limit=limit,
                offset=offset,
                fetch_size=fetch_size,
                query_name=query_name,
            )
            body = dict(envelope)
        else:
            body = self._build_query_envelope(
                filter=filter,
                sort=sort,
                limit=limit,
                offset=offset,
                fetch_size=fetch_size,
                query_name=query_name,
            )
        response = self._request(
            "POST",
            f"/v1/accounts/{account_id}/assets/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected account assets query response")
        return payload

    def query_account_assets_typed(
        self,
        account_id: str,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
        envelope: Optional[Mapping[str, Any]] = None,
    ) -> AccountAssetsPage:
        """Typed wrapper for :meth:`query_account_assets`."""

        payload = self.query_account_assets(
            account_id,
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
            envelope=envelope,
        )
        return AccountAssetsPage.from_payload(payload)

    def query_account_transactions(
        self,
        account_id: str,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
        envelope: Optional[Mapping[str, Any]] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/accounts/{account_id}/transactions/query` with a Norito-style envelope."""

        if envelope is not None:
            self._ensure_no_query_args(
                envelope=envelope,
                filter=filter,
                sort=sort,
                limit=limit,
                offset=offset,
                fetch_size=fetch_size,
                query_name=query_name,
            )
            body = dict(envelope)
        else:
            body = self._build_query_envelope(
                filter=filter,
                sort=sort,
                limit=limit,
                offset=offset,
                fetch_size=fetch_size,
                query_name=query_name,
            )
        response = self._request(
            "POST",
            f"/v1/accounts/{account_id}/transactions/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected account transactions query response")
        return payload

    def query_account_transactions_typed(
        self,
        account_id: str,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
        envelope: Optional[Mapping[str, Any]] = None,
    ) -> AccountTransactionsPage:
        """Typed wrapper for :meth:`query_account_transactions`."""

        payload = self.query_account_transactions(
            account_id,
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
            envelope=envelope,
        )
        return AccountTransactionsPage.from_payload(payload)

    # ------------------------------------------------------------------
    # UAID portfolio & Space Directory surfaces
    # ------------------------------------------------------------------

    def get_uaid_portfolio(self, uaid: str) -> Dict[str, Any]:
        """Fetch the aggregated UAID portfolio (`GET /v1/accounts/{uaid}/portfolio`)."""

        literal = _normalize_uaid_literal(uaid)
        response = self._request(
            "GET",
            f"/v1/accounts/{literal}/portfolio",
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected UAID portfolio response")
        return payload

    def get_uaid_portfolio_typed(self, uaid: str) -> UaidPortfolioSnapshot:
        """Typed wrapper for :meth:`get_uaid_portfolio`."""

        payload = self.get_uaid_portfolio(uaid)
        return UaidPortfolioSnapshot.from_payload(payload)

    def get_uaid_bindings(
        self,
        uaid: str,
        *,
        address_format: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Fetch UAID dataspace bindings (`GET /v1/space-directory/uaids/{uaid}`)."""

        literal = _normalize_uaid_literal(uaid)
        params: Dict[str, Any] = {}
        normalized_format = _normalize_address_format(address_format)
        if normalized_format is not None:
            params["address_format"] = normalized_format
        response = self._request(
            "GET",
            f"/v1/space-directory/uaids/{literal}",
            params=params or None,
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected UAID bindings response")
        return payload

    def get_uaid_bindings_typed(
        self,
        uaid: str,
        *,
        address_format: Optional[str] = None,
    ) -> UaidBindingsSnapshot:
        """Typed wrapper for :meth:`get_uaid_bindings`."""

        payload = self.get_uaid_bindings(uaid, address_format=address_format)
        return UaidBindingsSnapshot.from_payload(payload)

    def list_space_directory_manifests(
        self,
        uaid: str,
        *,
        dataspace: Optional[int] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        address_format: Optional[str] = None,
    ) -> Dict[str, Any]:
        """List Space Directory manifests bound to a UAID (`GET /v1/space-directory/uaids/{uaid}/manifests`)."""

        literal = _normalize_uaid_literal(uaid)
        params: Dict[str, Any] = {}
        if dataspace is not None:
            params["dataspace"] = _normalize_positive_int(
                dataspace,
                "dataspace",
                allow_zero=True,
            )
        if status is not None:
            if not isinstance(status, str):
                raise TypeError("status must be a string when provided")
            normalized_status = status.strip().lower()
            if normalized_status not in {"active", "inactive", "all"}:
                raise ValueError("status must be one of {'active', 'inactive', 'all'}")
            params["status"] = normalized_status
        if limit is not None:
            params["limit"] = _normalize_positive_int(limit, "limit", allow_zero=False)
        if offset is not None:
            params["offset"] = _normalize_positive_int(offset, "offset", allow_zero=True)
        normalized_format = _normalize_address_format(address_format)
        if normalized_format is not None:
            params["address_format"] = normalized_format
        response = self._request(
            "GET",
            f"/v1/space-directory/uaids/{literal}/manifests",
            params=params or None,
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected Space Directory manifests response")
        return payload

    def list_space_directory_manifests_typed(
        self,
        uaid: str,
        *,
        dataspace: Optional[int] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        address_format: Optional[str] = None,
    ) -> SpaceDirectoryManifestList:
        """Typed wrapper for :meth:`list_space_directory_manifests`."""

        payload = self.list_space_directory_manifests(
            uaid,
            dataspace=dataspace,
            status=status,
            limit=limit,
            offset=offset,
            address_format=address_format,
        )
        return SpaceDirectoryManifestList.from_payload(payload)

    def publish_space_directory_manifest(
        self,
        request: Mapping[str, Any],
    ) -> Optional[Any]:
        """Publish or rotate a Space Directory manifest (`POST /v1/space-directory/manifests`)."""

        payload = _normalize_publish_space_directory_manifest_request(request)
        response = self._request(
            "POST",
            "/v1/space-directory/manifests",
            json_body=payload,
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {202})
        return self._maybe_json(response)

    def revoke_space_directory_manifest(
        self,
        request: Mapping[str, Any],
    ) -> Optional[Any]:
        """Revoke a Space Directory manifest (`POST /v1/space-directory/manifests/revoke`)."""

        payload = _normalize_revoke_space_directory_manifest_request(request)
        response = self._request(
            "POST",
            "/v1/space-directory/manifests/revoke",
            json_body=payload,
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {202})
        return self._maybe_json(response)

    def _handle_sorafs_alias_warning(self, warning: SorafsAliasWarning) -> None:
        """Internal hook for alias-proof warnings."""

        self._sorafs_alias_metrics["warnings"] = self._sorafs_alias_metrics.get("warnings", 0) + 1
        if self._sorafs_alias_warning_hook:
            self._sorafs_alias_warning_hook(warning)

    def _enforce_sorafs_alias_policy(
        self,
        response: requests.Response,
    ) -> Optional[SorafsAliasEvaluation]:
        """Validate SoraFS alias proofs stapled on HTTP responses."""

        try:
            evaluation = enforce_sorafs_alias_policy(
                response,
                policy=self._sorafs_alias_policy,
                warning_hook=self._handle_sorafs_alias_warning,
                logger=self._sorafs_alias_logger,
            )
        except SorafsAliasError as exc:
            raise RuntimeError(f"failed to validate SoraFS alias proof: {exc}") from exc
        if evaluation is None:
            self._last_sorafs_alias_evaluation = None
            return None
        self._last_sorafs_alias_evaluation = evaluation
        self._sorafs_alias_metrics["total"] = self._sorafs_alias_metrics.get("total", 0) + 1
        label = evaluation.status_label or evaluation.state
        self._sorafs_alias_metrics[label] = self._sorafs_alias_metrics.get(label, 0) + 1
        return evaluation

    def _request(  # type: ignore[override]
        self,
        method: str,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[Mapping[str, str]] = None,
        data: Optional[bytes] = None,
        json_body: Optional[Mapping[str, Any]] = None,
        timeout: Optional[float] = None,
        allow_retry: bool = True,
    ) -> requests.Response:
        if json_body is not None and data is not None:
            raise ValueError("provide either `json_body` or `data`, not both")

        final_headers: Dict[str, str] = dict(self._default_headers)
        if headers:
            final_headers.update(headers)

        if json_body is not None:
            payload = json.dumps(json_body).encode("utf-8")
            final_headers.setdefault("Content-Type", "application/json")
        else:
            payload = data

        method_upper = method.upper()
        retry_enabled = allow_retry and method_upper in self._retry_methods
        max_attempts = 1 + (self._max_retries if retry_enabled else 0)
        request_timeout = timeout if timeout is not None else self._timeout
        url = f"{self._base_url}{path}"

        delay = self._backoff_initial
        for attempt in range(max_attempts):
            try:
                response = self._session.request(
                    method_upper,
                    url,
                    params=params,
                    headers=final_headers or None,
                    data=payload,
                    timeout=request_timeout,
                )
            except requests.RequestException:
                if attempt == max_attempts - 1:
                    raise
                delay = self._apply_backoff(delay)
                continue

            if (
                retry_enabled
                and response.status_code in self._retry_statuses
                and attempt < max_attempts - 1
            ):
                delay = self._apply_backoff(delay)
                continue

            self._enforce_sorafs_alias_policy(response)
            return response

        raise RuntimeError("exhausted retries without receiving a response")

    def _apply_backoff(self, current_delay: float) -> float:
        delay = current_delay
        if delay <= 0.0 and self._backoff_initial > 0.0:
            delay = self._backoff_initial
        if delay > 0.0:
            time.sleep(delay)
        if delay <= 0.0:
            return 0.0
        next_delay = delay * self._backoff_multiplier
        if self._backoff_cap != math.inf:
            next_delay = min(self._backoff_cap, next_delay)
        return next_delay

    def build_and_submit_transaction(
        self,
        chain_id: str,
        authority: str,
        private_key: bytes,
        *,
        instructions: Iterable["Instruction"] = (),
        creation_time_ms: Optional[int] = None,
        ttl_ms: Optional[int] = None,
        nonce: Optional[int] = None,
        metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
        expect_json: bool = True,
        envelope_format: str = "object",
    ) -> tuple["SignedTransactionEnvelope", Optional[Any]]:
        """Build, submit, and optionally wait for a transaction to finalize.

        When `expect_json` is true, an empty or non-dict response from the
        submission endpoint is normalised to `{}` so callers can assume a mapping.
        """

        crypto = _require_crypto()
        envelope = crypto.build_signed_transaction(
            chain_id,
            authority,
            private_key,
            instructions=instructions,
            creation_time_ms=creation_time_ms,
            ttl_ms=ttl_ms,
            nonce=nonce,
            metadata=metadata,
        )
        if envelope_format == "object":
            envelope_out: Union[SignedTransactionEnvelope, Dict[str, Any], str] = envelope
        elif envelope_format == "dict":
            envelope_out = envelope.as_dict()
        elif envelope_format == "json":
            envelope_out = envelope.to_json()
        else:
            raise ValueError(
                "envelope_format must be one of {'object', 'dict', 'json'}"
            )
        if wait:
            result = self.submit_transaction_envelope_and_wait(
                envelope,
                interval=interval,
                timeout=timeout,
                max_attempts=max_attempts,
                success_statuses=success_statuses,
                failure_statuses=failure_statuses,
                on_status=on_status,
            )
            return envelope_out, result
        response = self.submit_transaction_envelope(envelope)
        if expect_json and response is None:
            response = {}
        return envelope_out, response

    def get_transaction_status(self, hash_hex: str) -> Optional[Any]:
        """Fetch transaction pipeline status for the given hash (hex encoded)."""

        response = self._request(
            "GET",
            "/v1/pipeline/transactions/status",
            params={"hash": hash_hex},
        )
        self._expect_status(response, {200, 202, 204})
        return self._maybe_json(response)

    def wait_for_transaction_status(
        self,
        hash_hex: str,
        *,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
    ) -> Any:
        """Poll pipeline status until the transaction reaches a terminal state.

        Returns the final payload when the transaction reports one of the
        `success_statuses`. Raises :class:`TransactionStatusError` if a failure
        status is encountered, or :class:`TimeoutError` if neither a success nor
        a failure status is observed within the configured bounds.
        """

        success_set = (
            frozenset(str(s) for s in success_statuses)
            if success_statuses is not None
            else _DEFAULT_SUCCESS_STATUSES
        )
        failure_set = (
            frozenset(str(s) for s in failure_statuses)
            if failure_statuses is not None
            else _DEFAULT_FAILURE_STATUSES
        )

        attempts = 0
        deadline = None if timeout is None else (time.monotonic() + max(timeout, 0.0))

        while True:
            attempts += 1
            payload = self.get_transaction_status(hash_hex)
            status = _extract_pipeline_status_kind(payload)

            if on_status is not None:
                on_status(status, payload, attempts)

            if status is not None:
                if status in success_set:
                    return payload
                if status in failure_set:
                    raise TransactionStatusError(hash_hex, status, payload)

            if max_attempts is not None and attempts >= max_attempts:
                raise TimeoutError(
                    f"transaction {hash_hex} did not reach a terminal status "
                    f"after {attempts} attempts"
                )

            if deadline is not None and time.monotonic() >= deadline:
                raise TimeoutError(
                    f"transaction {hash_hex} did not reach a terminal status "
                    f"within {timeout} seconds"
                )

            if interval > 0.0:
                time.sleep(interval)

    # ------------------------------------------------------------------
    # Account queries
    # ------------------------------------------------------------------
    def query_accounts(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
        address_format: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Execute POST `/v1/accounts/query` with a structured envelope."""

        address_pref = _normalize_address_format(address_format)
        body = account_query_envelope(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
            address_format=address_pref,
        )
        response = self._request(
            "POST",
            "/v1/accounts/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected accounts query response")
        return payload

    def query_accounts_typed(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
        address_format: Optional[str] = None,
    ) -> AccountListPage:
        """Typed wrapper for :meth:`query_accounts`."""

        payload = self.query_accounts(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
            address_format=address_format,
        )
        return AccountListPage.from_payload(payload)

    def list_accounts(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        address_format: Optional[str] = None,
    ) -> Optional[Any]:
        """List accounts via `GET /v1/accounts`."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        filter_arg = _encode_filter_arg(filter)
        if filter_arg is not None:
            params["filter"] = filter_arg
        sort_arg = _encode_sort_arg(sort)
        if sort_arg is not None:
            params["sort"] = sort_arg
        address_pref = _normalize_address_format(address_format)
        if address_pref is not None:
            params["address_format"] = address_pref
        return self.request_json(
            "GET",
            "/v1/accounts",
            params=params or None,
            expected_status=(200,),
        )

    def list_accounts_typed(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        address_format: Optional[str] = None,
    ) -> AccountListPage:
        """Typed wrapper for :meth:`list_accounts`."""

        payload = self.list_accounts(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            address_format=address_format,
        )
        if payload is None:
            return AccountListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("accounts endpoint returned non-object payload")
        return AccountListPage.from_payload(payload)

    def list_domains(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Optional[Any]:
        """List domains via `GET /v1/domains`."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        filter_arg = _encode_filter_arg(filter)
        if filter_arg is not None:
            params["filter"] = filter_arg
        sort_arg = _encode_sort_arg(sort)
        if sort_arg is not None:
            params["sort"] = sort_arg
        return self.request_json(
            "GET",
            "/v1/domains",
            params=params or None,
            expected_status=(200,),
        )

    def list_domains_typed(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> DomainListPage:
        """Typed wrapper for :meth:`list_domains`."""

        payload = self.list_domains(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
        )
        if payload is None:
            return DomainListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("domains endpoint returned non-object payload")
        return DomainListPage.from_payload(payload)

    def query_asset_definitions(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/assets/definitions/query` with a structured envelope."""

        body = asset_definitions_query_envelope(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
        )
        response = self._request(
            "POST",
            "/v1/assets/definitions/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected assets definitions query response")
        return payload

    def query_asset_definitions_typed(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
    ) -> AssetDefinitionListPage:
        """Typed wrapper for :meth:`query_asset_definitions`."""

        payload = self.query_asset_definitions(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
        )
        return AssetDefinitionListPage.from_payload(payload)

    def list_asset_definitions(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Optional[Any]:
        """List asset definitions via `GET /v1/assets/definitions`."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        filter_arg = _encode_filter_arg(filter)
        if filter_arg is not None:
            params["filter"] = filter_arg
        sort_arg = _encode_sort_arg(sort)
        if sort_arg is not None:
            params["sort"] = sort_arg
        return self.request_json(
            "GET",
            "/v1/assets/definitions",
            params=params or None,
            expected_status=(200,),
        )

    def list_asset_definitions_typed(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> AssetDefinitionListPage:
        """Typed wrapper for :meth:`list_asset_definitions`."""

        payload = self.list_asset_definitions(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
        )
        if payload is None:
            return AssetDefinitionListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("asset definitions endpoint returned non-object payload")
        return AssetDefinitionListPage.from_payload(payload)

    def query_domains(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/domains/query` with a structured envelope."""

        body = domain_query_envelope(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
        )
        response = self._request(
            "POST",
            "/v1/domains/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected domains query response")
        return payload

    def query_domains_typed(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
    ) -> DomainListPage:
        """Typed wrapper for :meth:`query_domains`."""

        payload = self.query_domains(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
        )
        return DomainListPage.from_payload(payload)

    def query_asset_holders(
        self,
        asset_definition_id: str,
        *,
        filter: Optional[Dict[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
        address_format: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/assets/{definition}/holders/query` with a structured envelope."""

        address_pref = _normalize_address_format(address_format)
        body = asset_holders_query_envelope(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
            address_format=address_pref,
        )
        response = self._request(
            "POST",
            f"/v1/assets/{asset_definition_id}/holders/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected asset holders query response")
        return payload

    def query_asset_holders_typed(
        self,
        asset_definition_id: str,
        *,
        filter: Optional[Dict[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
        address_format: Optional[str] = None,
    ) -> AssetHolderListPage:
        """Typed wrapper for :meth:`query_asset_holders`."""

        payload = self.query_asset_holders(
            asset_definition_id,
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
            address_format=address_format,
        )
        return AssetHolderListPage.from_payload(payload)

    def list_asset_holders(
        self,
        asset_definition_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        address_format: Optional[str] = None,
    ) -> Optional[Any]:
        """List asset holders via `GET /v1/assets/{definition}/holders`."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        address_pref = _normalize_address_format(address_format)
        if address_pref is not None:
            params["address_format"] = address_pref
        return self.request_json(
            "GET",
            f"/v1/assets/{asset_definition_id}/holders",
            params=params or None,
            expected_status=(200,),
        )

    def list_asset_holders_typed(
        self,
        asset_definition_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        address_format: Optional[str] = None,
    ) -> AssetHolderListPage:
        """Typed wrapper for :meth:`list_asset_holders`."""

        payload = self.list_asset_holders(
            asset_definition_id,
            limit=limit,
            offset=offset,
            address_format=address_format,
        )
        if payload is None:
            return AssetHolderListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("asset holders endpoint returned non-object payload")
        return AssetHolderListPage.from_payload(payload)

    def list_account_permissions(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Optional[Any]:
        """List account permissions via `GET /v1/accounts/{account_id}/permissions`."""

        params = self._pagination_params(limit=limit, offset=offset)
        return self.request_json(
            "GET",
            f"/v1/accounts/{account_id}/permissions",
            params=params or None,
            expected_status=(200,),
        )

    def list_account_permissions_typed(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> AccountPermissionListPage:
        """Typed wrapper for :meth:`list_account_permissions`."""

        payload = self.list_account_permissions(account_id, limit=limit, offset=offset)
        if payload is None:
            return AccountPermissionListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("account permissions endpoint returned non-object payload")
        return AccountPermissionListPage.from_payload(payload)

    # ------------------------------------------------------------------
    # Contracts API
    # ------------------------------------------------------------------
    def register_contract_code(self, manifest: Mapping[str, Any]) -> Optional[Any]:
        response = self._request(
            "POST",
            "/v1/contracts/code",
            data=json.dumps(manifest).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200, 202})
        return self._maybe_json(response)

    def deploy_contract(self, request: Mapping[str, Any]) -> Optional[Any]:
        response = self._request(
            "POST",
            "/v1/contracts/deploy",
            data=json.dumps(request).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200, 202})
        return self._maybe_json(response)

    def get_contract_manifest(self, code_hash_hex: str) -> Optional[Any]:
        response = self._request(
            "GET",
            f"/v1/contracts/code/{code_hash_hex}",
        )
        self._expect_status(response, {200, 404})
        return self._maybe_json(response)

    def get_contract_manifest_typed(self, code_hash_hex: str) -> ContractManifestRecord:
        """Typed wrapper for :meth:`get_contract_manifest`."""

        payload = self.get_contract_manifest(code_hash_hex)
        if payload is None:
            raise RuntimeError("contract manifest endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("contract manifest endpoint returned non-object payload")
        return ContractManifestRecord.from_payload(payload)

    def get_contract_code_bytes(self, code_hash_hex: str) -> Optional[Any]:
        response = self._request(
            "GET",
            f"/v1/contracts/code-bytes/{code_hash_hex}",
        )
        self._expect_status(response, {200, 404})
        return self._maybe_json(response)

    def list_contract_instances(
        self,
        namespace: str,
        *,
        params: Optional[Dict[str, Any]] = None,
    ) -> Optional[Any]:
        response = self._request(
            "GET",
            f"/v1/contracts/instances/{namespace}",
            params=params,
        )
        self._expect_status(response, {200})
        return self._maybe_json(response)

    # ------------------------------------------------------------------
    # Connect API
    # ------------------------------------------------------------------
    def create_connect_session(
        self,
        payload: Optional[Mapping[str, Any]] = None,
    ) -> Optional[Any]:
        """POST `/v1/connect/session` and return the session payload."""

        body = dict(payload) if payload is not None else {}
        return self.request_json(
            "POST",
            "/v1/connect/session",
            json_body=body,
            expected_status=(200, 201),
        )

    def create_connect_session_info(
        self,
        payload: Optional[Mapping[str, Any]] = None,
        *,
        include_expiry: bool = True,
    ) -> ConnectSessionInfo:
        """Create a session and parse the response into `ConnectSessionInfo`."""

        response = self.create_connect_session(payload)
        if not isinstance(response, Mapping):
            raise ValueError("connect session response is missing or malformed")
        ttl_ms: Optional[int] = None
        if include_expiry:
            status_snapshot = self.get_connect_status_typed()
            if status_snapshot is not None and status_snapshot.policy is not None:
                ttl_ms = status_snapshot.policy.session_ttl_ms
        return ConnectSessionInfo.from_mapping(response, session_ttl_ms=ttl_ms)

    def send_connect_control(
        self,
        sid: str,
        *,
        kind: str,
        payload: Mapping[str, Any],
    ) -> Optional[Any]:
        """Convenience helper for posting Connect control frames via `/v1/connect/control/{kind}`."""

        return self.request_json(
            "POST",
            f"/v1/connect/control/{kind}",
            params={"sid": sid},
            json_body=dict(payload),
            expected_status=(200, 202),
        )

    def send_connect_control_frame(
        self,
        sid: str,
        control: "ConnectControlBase",
    ) -> Optional[Any]:
        """Send a typed Connect control by inferring the REST endpoint from the variant."""

        payload = _json_safe_value(control.to_dict())
        return self.send_connect_control(
            sid,
            kind=control.endpoint_kind,
            payload=payload,
        )

    def delete_connect_session(self, sid: str) -> bool:
        """DELETE `/v1/connect/session/{sid}` and return True when the session existed."""

        if not isinstance(sid, str) or not sid:
            raise TypeError("sid must be a non-empty string")
        response = self._request("DELETE", f"/v1/connect/session/{sid}")
        self._expect_status(response, (204, 404))
        return response.status_code == 204

    def connect_websocket(
        self,
        sid: str,
        role: str,
        token: str,
        *,
        timeout: Optional[float] = None,
        headers: Optional[Mapping[str, str]] = None,
        subprotocols: Optional[Sequence[str]] = None,
    ):
        """Open a Connect WebSocket (`/v1/connect/ws`). Requires `websocket-client`."""

        if websocket is None:  # pragma: no cover - dependency optional
            raise RuntimeError(
                "websocket-client is not installed. Install iroha-python with the `ws` extra "
                "(`pip install iroha-python[ws]`) or add `websocket-client` to your environment."
            )

        parsed = urlparse(self._base_url)
        scheme = "wss" if parsed.scheme == "https" else "ws"
        ws_path = "/v1/connect/ws"
        query = urlencode({"sid": sid, "role": role, "token": token})
        ws_url = urlunparse((scheme, parsed.netloc, ws_path, "", query, ""))

        header_list: List[str] = []
        combined_headers: Dict[str, str] = dict(self._default_headers)
        combined_headers.pop("Accept", None)
        if headers:
            combined_headers.update(headers)
        for key, value in combined_headers.items():
            header_list.append(f"{key}: {value}")

        return websocket.create_connection(  # type: ignore[call-arg]
            ws_url,
            timeout=timeout,
            header=header_list or None,
            subprotocols=list(subprotocols) if subprotocols else None,
        )

    def get_connect_status(self) -> Optional[Any]:
        """Fetch Connect runtime status (`GET /v1/connect/status`)."""

        return self.request_json(
            "GET",
            "/v1/connect/status",
            expected_status=(200,),
        )

    def get_connect_status_typed(self) -> Optional[ConnectStatusSnapshot]:
        """Typed wrapper for :meth:`get_connect_status`. Returns `None` when Connect is disabled."""

        payload = self.get_connect_status()
        if payload is None:
            return None
        return ConnectStatusSnapshot.from_payload(payload)

    def list_connect_apps(
        self,
        *,
        limit: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> "ConnectAppRegistryPage":
        """List registered Connect applications (`GET /v1/connect/app/apps`)."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if cursor is not None:
            if not isinstance(cursor, str):
                raise TypeError("connect app cursor must be a string")
            params["cursor"] = cursor
        payload = self.request_json(
            "GET",
            "/v1/connect/app/apps",
            params=params or None,
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect app registry response must be a JSON object")
        return ConnectAppRegistryPage.from_payload(payload)

    def iter_connect_apps(
        self,
        *,
        page_size: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> Iterator["ConnectAppRecord"]:
        """Iterate over all Connect applications by following pagination cursors.

        Args:
            page_size: Optional limit applied to each request. Must be positive when set.
            cursor: Optional starting cursor returned by a previous listing.

        Yields:
            :class:`ConnectAppRecord` entries for every registry item.
        """

        if page_size is not None:
            page_limit = int(page_size)
            if page_limit <= 0:
                raise ValueError("page_size must be positive when provided")
        else:
            page_limit = None

        if cursor is not None and not isinstance(cursor, str):
            raise TypeError("cursor must be a string when provided")

        seen_cursors: set[str] = set()
        next_cursor = cursor
        if next_cursor is not None:
            seen_cursors.add(next_cursor)

        while True:
            page = self.list_connect_apps(limit=page_limit, cursor=next_cursor)
            for record in page.items:
                yield record
            next_cursor = page.next_cursor
            if next_cursor is None:
                break
            if next_cursor in seen_cursors:
                raise RuntimeError(
                    f"connect app registry returned duplicate cursor {next_cursor!r}"
                )
            seen_cursors.add(next_cursor)

    def get_connect_app(self, app_id: str) -> "ConnectAppRecord":
        """Fetch a single Connect application (`GET /v1/connect/app/apps/{app_id}`)."""

        if not isinstance(app_id, str) or not app_id:
            raise TypeError("app_id must be a non-empty string")
        payload = self.request_json(
            "GET",
            f"/v1/connect/app/apps/{app_id}",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect app response must be a JSON object")
        return ConnectAppRecord.from_payload(payload)

    def register_connect_app(
        self,
        registration: Union["ConnectAppRecord", Mapping[str, Any]],
    ) -> Optional[Any]:
        """Register or update a Connect application (`POST /v1/connect/app/apps`)."""

        if isinstance(registration, ConnectAppRecord):
            body = registration.to_payload()
        else:
            body = dict(registration)
        payload = self.request_json(
            "POST",
            "/v1/connect/app/apps",
            json_body=_json_safe_value(body),
            expected_status=(200, 201, 202),
        )
        if isinstance(payload, Mapping):
            try:
                return ConnectAppRecord.from_payload(payload)
            except TypeError:
                return payload
        return payload

    def delete_connect_app(self, app_id: str) -> Optional[Any]:
        """Delete a Connect application (`DELETE /v1/connect/app/apps/{app_id}`)."""

        if not isinstance(app_id, str) or not app_id:
            raise TypeError("app_id must be a non-empty string")
        response = self._request("DELETE", f"/v1/connect/app/apps/{app_id}")
        self._expect_status(response, {200, 202, 204, 404})
        return self._maybe_json(response)

    def get_connect_app_policy_controls(self) -> "ConnectAppPolicyControls":
        """Fetch mutable Connect policy toggles (`GET /v1/connect/app/policy`)."""

        payload = self.request_json(
            "GET",
            "/v1/connect/app/policy",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect app policy response must be a JSON object")
        policy_payload = payload.get("policy")
        if isinstance(policy_payload, Mapping):
            return ConnectAppPolicyControls.from_payload(policy_payload)
        return ConnectAppPolicyControls.from_payload(payload)

    def update_connect_app_policy_controls(
        self,
        updates: Union["ConnectAppPolicyControls", Mapping[str, Any]],
    ) -> "ConnectAppPolicyControls":
        """Update Connect policy toggles (`POST /v1/connect/app/policy`)."""

        if isinstance(updates, ConnectAppPolicyControls):
            body = updates.to_payload()
        else:
            body = dict(updates)
        payload = self.request_json(
            "POST",
            "/v1/connect/app/policy",
            json_body=_json_safe_value(body),
            expected_status=(200, 202),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect app policy response must be a JSON object")
        policy_payload = payload.get("policy")
        if isinstance(policy_payload, Mapping):
            return ConnectAppPolicyControls.from_payload(policy_payload)
        return ConnectAppPolicyControls.from_payload(payload)

    def get_connect_admission_manifest(self) -> "ConnectAdmissionManifest":
        """Fetch the Connect admission manifest (`GET /v1/connect/app/manifest`)."""

        payload = self.request_json(
            "GET",
            "/v1/connect/app/manifest",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission manifest response must be a JSON object")
        return ConnectAdmissionManifest.from_payload(payload)

    def set_connect_admission_manifest(
        self,
        manifest: Union["ConnectAdmissionManifest", Mapping[str, Any]],
    ) -> "ConnectAdmissionManifest":
        """Replace the Connect admission manifest (`PUT /v1/connect/app/manifest`)."""

        if isinstance(manifest, ConnectAdmissionManifest):
            body = manifest.to_payload()
        else:
            body = dict(manifest)
        payload = self.request_json(
            "PUT",
            "/v1/connect/app/manifest",
            json_body=_json_safe_value(body),
            expected_status=(200, 202),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission manifest response must be a JSON object")
        return ConnectAdmissionManifest.from_payload(payload)


@dataclass(frozen=True)
class ConnectAppPolicyControls:
    """Runtime-configurable Connect policy toggles."""

    relay_enabled: Optional[bool]
    ws_max_sessions: Optional[int]
    ws_per_ip_max_sessions: Optional[int]
    ws_rate_per_ip_per_min: Optional[int]
    session_ttl_ms: Optional[int]
    frame_max_bytes: Optional[int]
    session_buffer_max_bytes: Optional[int]
    ping_interval_ms: Optional[int]
    ping_miss_tolerance: Optional[int]
    ping_min_interval_ms: Optional[int]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppPolicyControls":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app policy payload must be an object")
        data = dict(payload)

        def _coerce_optional_int(name: str) -> Optional[int]:
            value = data.get(name)
            if value is None:
                return None
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"connect app policy field `{name}` must be numeric") from exc

        relay_enabled_raw = data.get("relay_enabled")
        relay_enabled: Optional[bool]
        if relay_enabled_raw is None:
            relay_enabled = None
        elif isinstance(relay_enabled_raw, bool):
            relay_enabled = relay_enabled_raw
        else:
            raise TypeError("connect app policy `relay_enabled` must be boolean when present")

        recognized = {
            "relay_enabled",
            "ws_max_sessions",
            "ws_per_ip_max_sessions",
            "ws_rate_per_ip_per_min",
            "session_ttl_ms",
            "frame_max_bytes",
            "session_buffer_max_bytes",
            "ping_interval_ms",
            "ping_miss_tolerance",
            "ping_min_interval_ms",
        }

        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(
            relay_enabled=relay_enabled,
            ws_max_sessions=_coerce_optional_int("ws_max_sessions"),
            ws_per_ip_max_sessions=_coerce_optional_int("ws_per_ip_max_sessions"),
            ws_rate_per_ip_per_min=_coerce_optional_int("ws_rate_per_ip_per_min"),
            session_ttl_ms=_coerce_optional_int("session_ttl_ms"),
            frame_max_bytes=_coerce_optional_int("frame_max_bytes"),
            session_buffer_max_bytes=_coerce_optional_int("session_buffer_max_bytes"),
            ping_interval_ms=_coerce_optional_int("ping_interval_ms"),
            ping_miss_tolerance=_coerce_optional_int("ping_miss_tolerance"),
            ping_min_interval_ms=_coerce_optional_int("ping_min_interval_ms"),
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the policy controls back to a JSON-serializable mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        if self.relay_enabled is not None:
            payload["relay_enabled"] = self.relay_enabled
        if self.ws_max_sessions is not None:
            payload["ws_max_sessions"] = self.ws_max_sessions
        if self.ws_per_ip_max_sessions is not None:
            payload["ws_per_ip_max_sessions"] = self.ws_per_ip_max_sessions
        if self.ws_rate_per_ip_per_min is not None:
            payload["ws_rate_per_ip_per_min"] = self.ws_rate_per_ip_per_min
        if self.session_ttl_ms is not None:
            payload["session_ttl_ms"] = self.session_ttl_ms
        if self.frame_max_bytes is not None:
            payload["frame_max_bytes"] = self.frame_max_bytes
        if self.session_buffer_max_bytes is not None:
            payload["session_buffer_max_bytes"] = self.session_buffer_max_bytes
        if self.ping_interval_ms is not None:
            payload["ping_interval_ms"] = self.ping_interval_ms
        if self.ping_miss_tolerance is not None:
            payload["ping_miss_tolerance"] = self.ping_miss_tolerance
        if self.ping_min_interval_ms is not None:
            payload["ping_min_interval_ms"] = self.ping_min_interval_ms
        return payload


@dataclass(frozen=True)
class ConnectAppRecord:
    """Registered Connect application metadata."""

    app_id: str
    display_name: Optional[str]
    description: Optional[str]
    icon_url: Optional[str]
    namespaces: Sequence[str]
    metadata: Mapping[str, Any]
    policy: Mapping[str, Any]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app entry must be an object")
        data = dict(payload)
        app_id = data.get("app_id")
        if not isinstance(app_id, str) or not app_id:
            raise TypeError("connect app entry requires string `app_id` field")

        def _coerce_optional_str(name: str) -> Optional[str]:
            value = data.get(name)
            if value is None:
                return None
            if isinstance(value, str):
                return value
            raise TypeError(f"connect app entry `{name}` must be a string when present")

        namespaces_raw = data.get("namespaces") or []
        if namespaces_raw is None:
            namespaces_raw = []
        if not isinstance(namespaces_raw, list):
            raise TypeError("connect app entry `namespaces` must be a list")
        namespaces: List[str] = []
        for item in namespaces_raw:
            if not isinstance(item, str):
                raise TypeError("connect app entry `namespaces` must contain strings")
            namespaces.append(item)

        metadata_raw = data.get("metadata") or {}
        if metadata_raw is None:
            metadata_raw = {}
        if not isinstance(metadata_raw, Mapping):
            raise TypeError("connect app entry `metadata` must be an object")

        policy_raw = data.get("policy") or {}
        if policy_raw is None:
            policy_raw = {}
        if not isinstance(policy_raw, Mapping):
            raise TypeError("connect app entry `policy` must be an object")

        recognized = {
            "app_id",
            "display_name",
            "description",
            "icon_url",
            "namespaces",
            "metadata",
            "policy",
        }

        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(
            app_id=app_id,
            display_name=_coerce_optional_str("display_name"),
            description=_coerce_optional_str("description"),
            icon_url=_coerce_optional_str("icon_url"),
            namespaces=tuple(namespaces),
            metadata=dict(metadata_raw),
            policy=dict(policy_raw),
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the record back into a JSON-friendly mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        payload["app_id"] = self.app_id
        if self.display_name is not None:
            payload["display_name"] = self.display_name
        if self.description is not None:
            payload["description"] = self.description
        if self.icon_url is not None:
            payload["icon_url"] = self.icon_url
        payload["namespaces"] = list(self.namespaces)
        payload["metadata"] = dict(self.metadata)
        payload["policy"] = dict(self.policy)
        return payload


@dataclass(frozen=True)
class ConnectAppRegistryPage:
    """Paginated Connect application registry results."""

    items: Sequence[ConnectAppRecord]
    total: Optional[int]
    next_cursor: Optional[str]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppRegistryPage":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app registry payload must be an object")
        data = dict(payload)
        items_raw = data.get("items") or []
        if items_raw is None:
            items_raw = []
        if not isinstance(items_raw, list):
            raise TypeError("connect app registry `items` must be a list")
        items = [ConnectAppRecord.from_payload(entry) for entry in items_raw]

        total_raw = data.get("total")
        total: Optional[int]
        if total_raw is None:
            total = None
        else:
            try:
                total = int(total_raw)
            except (TypeError, ValueError) as exc:
                raise TypeError("connect app registry `total` must be numeric when present") from exc

        cursor_raw = data.get("next_cursor")
        if cursor_raw is not None and not isinstance(cursor_raw, str):
            raise TypeError("connect app registry cursor must be a string when present")

        recognized = {"items", "total", "next_cursor"}
        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(items=tuple(items), total=total, next_cursor=cursor_raw, extra=extra)


@dataclass(frozen=True)
class ConnectAdmissionManifestEntry:
    """Admission control record for a Connect application."""

    app_id: str
    namespaces: Sequence[str]
    metadata: Mapping[str, Any]
    policy: Mapping[str, Any]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAdmissionManifestEntry":
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission entry must be an object")
        data = dict(payload)
        app_id = data.get("app_id")
        if not isinstance(app_id, str) or not app_id:
            raise TypeError("connect admission entry requires string `app_id` field")

        namespaces_raw = data.get("namespaces") or []
        if namespaces_raw is None:
            namespaces_raw = []
        if not isinstance(namespaces_raw, list):
            raise TypeError("connect admission entry `namespaces` must be a list")
        namespaces: List[str] = []
        for item in namespaces_raw:
            if not isinstance(item, str):
                raise TypeError("connect admission entry `namespaces` values must be strings")
            namespaces.append(item)

        metadata_raw = data.get("metadata") or {}
        if metadata_raw is None:
            metadata_raw = {}
        if not isinstance(metadata_raw, Mapping):
            raise TypeError("connect admission entry `metadata` must be an object")

        policy_raw = data.get("policy") or {}
        if policy_raw is None:
            policy_raw = {}
        if not isinstance(policy_raw, Mapping):
            raise TypeError("connect admission entry `policy` must be an object")

        recognized = {"app_id", "namespaces", "metadata", "policy"}
        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(
            app_id=app_id,
            namespaces=tuple(namespaces),
            metadata=dict(metadata_raw),
            policy=dict(policy_raw),
            extra=extra,
        )


@dataclass(frozen=True)
class ConnectAdmissionManifest:
    """Connect admission manifest describing allowed applications."""

    version: Optional[int]
    entries: Sequence[ConnectAdmissionManifestEntry]
    manifest_hash: Optional[str]
    updated_at: Optional[str]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAdmissionManifest":
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission manifest payload must be an object")
        data = dict(payload)
        entries_raw = data.get("entries") or []
        if entries_raw is None:
            entries_raw = []
        if not isinstance(entries_raw, list):
            raise TypeError("connect admission manifest `entries` must be a list")
        entries = [ConnectAdmissionManifestEntry.from_payload(item) for item in entries_raw]

        version_raw = data.get("version")
        if version_raw is None:
            version: Optional[int] = None
        else:
            try:
                version = int(version_raw)
            except (TypeError, ValueError) as exc:
                raise TypeError("connect admission manifest `version` must be numeric") from exc

        manifest_hash = data.get("manifest_hash")
        if manifest_hash is not None and not isinstance(manifest_hash, str):
            raise TypeError("connect admission manifest `manifest_hash` must be a string when present")
        updated_at = data.get("updated_at")
        if updated_at is not None and not isinstance(updated_at, str):
            raise TypeError("connect admission manifest `updated_at` must be a string when present")

        recognized = {"entries", "version", "manifest_hash", "updated_at"}
        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(
            version=version,
            entries=tuple(entries),
            manifest_hash=manifest_hash,
            updated_at=updated_at,
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the manifest to a JSON-serializable mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        payload["entries"] = [
            {
                "app_id": entry.app_id,
                "namespaces": list(entry.namespaces),
                "metadata": dict(entry.metadata),
                "policy": dict(entry.policy),
                **dict(entry.extra),
            }
            for entry in self.entries
        ]
        if self.version is not None:
            payload["version"] = self.version
        if self.manifest_hash is not None:
            payload["manifest_hash"] = self.manifest_hash
        if self.updated_at is not None:
            payload["updated_at"] = self.updated_at
        return payload

    # ------------------------------------------------------------------
    # Telemetry & Sumeragi helpers
    # ------------------------------------------------------------------
    def get_sumeragi_telemetry(self) -> Optional[Any]:
        """Fetch aggregated consensus telemetry (`GET /v1/sumeragi/telemetry`)."""

        return self.request_json("GET", "/v1/sumeragi/telemetry", expected_status=(200,))

    def get_sumeragi_telemetry_typed(self) -> SumeragiTelemetrySnapshot:
        """Return `/v1/sumeragi/telemetry` as a structured snapshot."""

        payload = self.request_json("GET", "/v1/sumeragi/telemetry", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry response must be a JSON object")
        return SumeragiTelemetrySnapshot.from_payload(payload)

    def get_sumeragi_status(self) -> Optional[Any]:
        """Fetch consensus status (`GET /v1/sumeragi/status`)."""

        return self.request_json("GET", "/v1/sumeragi/status", expected_status=(200,))

    def get_sumeragi_status_typed(self) -> SumeragiStatusSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_status`."""

        payload = self.request_json("GET", "/v1/sumeragi/status", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi status response must be a JSON object")
        return SumeragiStatusSnapshot.from_payload(payload)

    def get_sumeragi_new_view(self) -> Optional[Any]:
        """Fetch NEW_VIEW receipt counts (`GET /v1/sumeragi/new_view/json`)."""

        return self.request_json("GET", "/v1/sumeragi/new_view/json", expected_status=(200,))

    def get_sumeragi_new_view_typed(self) -> SumeragiNewViewSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_new_view`."""

        payload = self.request_json("GET", "/v1/sumeragi/new_view/json", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("new_view response must be a JSON object")
        return SumeragiNewViewSnapshot.from_payload(payload)

    def get_sumeragi_rbc(self) -> Optional[Any]:
        """Fetch RBC throughput metrics (`GET /v1/sumeragi/rbc`)."""

        return self.request_json("GET", "/v1/sumeragi/rbc", expected_status=(200,))

    def get_sumeragi_rbc_typed(self) -> SumeragiRbcSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_rbc`."""

        payload = self.request_json("GET", "/v1/sumeragi/rbc", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("rbc snapshot response must be a JSON object")
        return SumeragiRbcSnapshot.from_payload(payload)

    def get_sumeragi_rbc_sessions(self) -> Optional[Any]:
        """Fetch active RBC sessions (`GET /v1/sumeragi/rbc/sessions`)."""

        return self.request_json("GET", "/v1/sumeragi/rbc/sessions", expected_status=(200,))

    def get_sumeragi_rbc_sessions_typed(self) -> SumeragiRbcSessionsSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_rbc_sessions`."""

        payload = self.request_json("GET", "/v1/sumeragi/rbc/sessions", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("rbc sessions response must be a JSON object")
        return SumeragiRbcSessionsSnapshot.from_payload(payload)

    def find_sumeragi_rbc_sampling_candidate(self) -> Optional[Any]:
        """Return the first delivered RBC session with a block hash, or ``None``."""

        candidate = self.find_sumeragi_rbc_sampling_candidate_typed()
        if candidate is None:
            return None
        return asdict(candidate)

    def find_sumeragi_rbc_sampling_candidate_typed(self) -> Optional[SumeragiRbcSession]:
        """Typed helper mirroring :meth:`find_sumeragi_rbc_sampling_candidate`."""

        snapshot = self.get_sumeragi_rbc_sessions_typed()
        for session in snapshot.items:
            if session.delivered and session.block_hash:
                return session
        return None

    def get_sumeragi_rbc_delivered(self, height: int, view: int) -> Optional[Any]:
        """Fetch delivery status for a specific RBC session."""

        return self.request_json(
            "GET",
            f"/v1/sumeragi/rbc/delivered/{int(height)}/{int(view)}",
            expected_status=(200,),
        )

    def get_sumeragi_rbc_delivered_typed(self, height: int, view: int) -> SumeragiRbcDeliveryStatus:
        """Typed wrapper for :meth:`get_sumeragi_rbc_delivered`."""

        payload = self.get_sumeragi_rbc_delivered(height, view)
        if not isinstance(payload, Mapping):
            raise TypeError("rbc delivered response must be a JSON object")
        return SumeragiRbcDeliveryStatus.from_payload(payload)

    def request_sumeragi_rbc_sample(
        self,
        block_hash: str,
        height: int,
        view: int,
        *,
        count: Optional[int] = None,
        seed: Optional[int] = None,
        api_token: Optional[str] = None,
    ) -> Optional[Any]:
        """Request RBC chunk samples (`POST /v1/sumeragi/rbc/sample`)."""

        payload: Dict[str, Any] = {
            "block_hash": block_hash,
            "height": int(height),
            "view": int(view),
        }
        if count is not None:
            payload["count"] = int(count)
        if seed is not None:
            payload["seed"] = int(seed)

        headers: Dict[str, str] = {}
        token = api_token or self._api_token
        if token:
            headers["X-API-Token"] = token

        response = self._request(
            "POST",
            "/v1/sumeragi/rbc/sample",
            headers=headers or None,
            json_body=payload,
            allow_retry=False,
        )
        self._expect_status(response, {200, 404})
        if response.status_code == 404:
            return None
        payload_json = self._maybe_json(response)
        if not isinstance(payload_json, Mapping):
            raise RuntimeError("unexpected RBC sample response")
        return payload_json

    def request_sumeragi_rbc_sample_typed(
        self,
        block_hash: str,
        height: int,
        view: int,
        *,
        count: Optional[int] = None,
        seed: Optional[int] = None,
        api_token: Optional[str] = None,
    ) -> Optional[RbcSample]:
        """Typed wrapper for :meth:`request_sumeragi_rbc_sample`."""

        raw = self.request_sumeragi_rbc_sample(
            block_hash,
            height,
            view,
            count=count,
            seed=seed,
            api_token=api_token,
        )
        if raw is None:
            return None
        if not isinstance(raw, Mapping):
            raise TypeError("RBC sample response must be an object")
        return RbcSample.from_payload(raw)

    def get_sumeragi_pacemaker(self) -> Optional[Any]:
        """Fetch pacemaker configuration (`GET /v1/sumeragi/pacemaker`)."""

        return self.request_json("GET", "/v1/sumeragi/pacemaker", expected_status=(200,))

    def get_sumeragi_pacemaker_typed(self) -> SumeragiPacemakerSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_pacemaker`."""

        payload = self.request_json("GET", "/v1/sumeragi/pacemaker", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("pacemaker response must be a JSON object")
        return SumeragiPacemakerSnapshot.from_payload(payload)

    def get_sumeragi_qc(self) -> Optional[Any]:
        """Fetch HighestQC/LockedQC snapshot (`GET /v1/sumeragi/qc`)."""

        return self.request_json("GET", "/v1/sumeragi/qc", expected_status=(200,))

    def get_sumeragi_qc_typed(self) -> SumeragiQcSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_qc`."""

        payload = self.request_json("GET", "/v1/sumeragi/qc", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("qc response must be a JSON object")
        return SumeragiQcSnapshot.from_payload(payload)

    def get_sumeragi_leader(self) -> Optional[Any]:
        """Fetch leader index snapshot (`GET /v1/sumeragi/leader`)."""

        return self.request_json("GET", "/v1/sumeragi/leader", expected_status=(200,))

    def get_sumeragi_leader_typed(self) -> SumeragiLeaderSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_leader`."""

        payload = self.request_json("GET", "/v1/sumeragi/leader", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("leader response must be a JSON object")
        return SumeragiLeaderSnapshot.from_payload(payload)

    def get_sumeragi_evidence_count(self) -> Optional[Any]:
        """Return total persisted evidence records (`GET /v1/sumeragi/evidence/count`)."""

        return self.request_json(
            "GET",
            "/v1/sumeragi/evidence/count",
            expected_status=(200,),
        )

    def get_sumeragi_evidence_count_typed(self) -> SumeragiEvidenceCount:
        """Typed wrapper for :meth:`get_sumeragi_evidence_count`."""

        payload = self.request_json(
            "GET",
            "/v1/sumeragi/evidence/count",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("evidence count response must be a JSON object")
        return SumeragiEvidenceCount.from_payload(payload)

    def list_sumeragi_evidence(
        self,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        kind: Optional[str] = None,
    ) -> Optional[Any]:
        """List evidence records with optional filters (`GET /v1/sumeragi/evidence`)."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        if kind is not None:
            params["kind"] = str(kind)
        return self.request_json(
            "GET",
            "/v1/sumeragi/evidence",
            params=params or None,
            expected_status=(200,),
        )

    def list_sumeragi_evidence_typed(
        self,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        kind: Optional[str] = None,
    ) -> SumeragiEvidenceListPage:
        """Typed wrapper for :meth:`list_sumeragi_evidence`."""

        payload = self.list_sumeragi_evidence(limit=limit, offset=offset, kind=kind)
        if payload is None:
            return SumeragiEvidenceListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("sumeragi evidence endpoint returned non-object payload")
        return SumeragiEvidenceListPage.from_payload(payload)

    def submit_sumeragi_evidence(self, evidence_hex: str) -> Optional[Any]:
        """Submit a Norito-encoded evidence payload (`POST /v1/sumeragi/evidence`)."""

        return self.request_json(
            "POST",
            "/v1/sumeragi/evidence",
            json_body={"evidence_hex": str(evidence_hex)},
            expected_status=(200, 202),
            allow_retry=False,
        )

    def get_sumeragi_phases(self) -> Optional[Any]:
        """Fetch consensus phase durations (`GET /v1/sumeragi/phases`)."""

        return self.request_json("GET", "/v1/sumeragi/phases", expected_status=(200,))

    def get_sumeragi_phases_typed(self) -> SumeragiPhasesSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_phases`."""

        payload = self.request_json("GET", "/v1/sumeragi/phases", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("phases response must be a JSON object")
        return SumeragiPhasesSnapshot.from_payload(payload)

    def get_sumeragi_collectors(self) -> Optional[Any]:
        """Fetch collector plan snapshot (`GET /v1/sumeragi/collectors`)."""

        return self.request_json("GET", "/v1/sumeragi/collectors", expected_status=(200,))

    def get_sumeragi_collectors_typed(self) -> SumeragiCollectorPlan:
        """Typed wrapper for :meth:`get_sumeragi_collectors`."""

        payload = self.request_json("GET", "/v1/sumeragi/collectors", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("collector plan response must be a JSON object")
        return SumeragiCollectorPlan.from_payload(payload)

    def get_sumeragi_params(self) -> Optional[Any]:
        """Fetch on-chain Sumeragi parameters (`GET /v1/sumeragi/params`)."""

        return self.request_json("GET", "/v1/sumeragi/params", expected_status=(200,))

    def get_sumeragi_params_typed(self) -> SumeragiParamsSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_params`."""

        payload = self.request_json("GET", "/v1/sumeragi/params", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi params response must be a JSON object")
        return SumeragiParamsSnapshot.from_payload(payload)

    # ------------------------------------------------------------------
    # Governance API
    # ------------------------------------------------------------------
    def set_protected_namespaces(self, namespaces: Sequence[str]) -> Optional[Any]:
        """Apply the `gov_protected_namespaces` parameter via `POST /v1/gov/protected-namespaces`."""

        if isinstance(namespaces, str):
            values = [str(namespaces)]
        else:
            values = [str(value) for value in namespaces]
        return self.request_json(
            "POST",
            "/v1/gov/protected-namespaces",
            json_body={"namespaces": values},
            expected_status=(200,),
        )

    def get_protected_namespaces(self) -> Optional[Any]:
        """Fetch the current `gov_protected_namespaces` setting (`GET /v1/gov/protected-namespaces`)."""

        return self.request_json(
            "GET",
            "/v1/gov/protected-namespaces",
            expected_status=(200,),
        )

    def get_governance_council_current(self) -> Optional[Any]:
        """GET `/v1/gov/council/current`."""

        return self.request_json(
            "GET",
            "/v1/gov/council/current",
            expected_status=(200,),
        )

    def get_governance_council_audit(self, *, epoch: Optional[int] = None) -> Optional[Any]:
        """GET `/v1/gov/council/audit` (optionally override the audited epoch)."""

        params: Dict[str, Any] = {}
        if epoch is not None:
            params["epoch"] = int(epoch)
        return self.request_json(
            "GET",
            "/v1/gov/council/audit",
            params=params or None,
            expected_status=(200,),
        )

    def derive_governance_council_vrf(
        self,
        payload: Mapping[str, Any],
    ) -> Optional[Any]:
        """POST `/v1/gov/council/derive-vrf` (requires Torii built with `gov_vrf`)."""

        return self.request_json(
            "POST",
            "/v1/gov/council/derive-vrf",
            json_body=dict(payload),
            expected_status=(200,),
        )

    def persist_governance_council(
        self,
        payload: Mapping[str, Any],
    ) -> Optional[Any]:
        """POST `/v1/gov/council/persist` (requires `gov_vrf`)."""

        return self.request_json(
            "POST",
            "/v1/gov/council/persist",
            json_body=dict(payload),
            expected_status=(200,),
        )

    def list_governance_instances(
        self,
        namespace: str,
        *,
        contains: Optional[str] = None,
        hash_prefix: Optional[str] = None,
        offset: Optional[int] = None,
        limit: Optional[int] = None,
        order: Optional[str] = None,
    ) -> Optional[Any]:
        """List contract instances tracked by governance (`GET /v1/gov/instances/{namespace}`)."""

        params: Dict[str, Any] = {}
        if contains is not None:
            params["contains"] = contains
        if hash_prefix is not None:
            params["hash_prefix"] = hash_prefix
        if offset is not None:
            params["offset"] = int(offset)
        if limit is not None:
            params["limit"] = int(limit)
        if order is not None:
            params["order"] = order
        return self.request_json(
            "GET",
            f"/v1/gov/instances/{namespace}",
            params=params or None,
            expected_status=(200,),
        )

    def list_governance_instances_typed(
        self,
        namespace: str,
        *,
        contains: Optional[str] = None,
        hash_prefix: Optional[str] = None,
        offset: Optional[int] = None,
        limit: Optional[int] = None,
        order: Optional[str] = None,
    ) -> ContractInstancesPage:
        """Typed wrapper for :meth:`list_governance_instances`."""

        payload = self.list_governance_instances(
            namespace,
            contains=contains,
            hash_prefix=hash_prefix,
            offset=offset,
            limit=limit,
            order=order,
        )
        if payload is None:
            raise RuntimeError("governance instances endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("governance instances endpoint returned non-object payload")
        return ContractInstancesPage.from_payload(payload)

    def list_contract_instances_typed(
        self,
        namespace: str,
        *,
        contains: Optional[str] = None,
        hash_prefix: Optional[str] = None,
        offset: Optional[int] = None,
        limit: Optional[int] = None,
        order: Optional[str] = None,
    ) -> ContractInstancesPage:
        """Typed wrapper for :meth:`list_contract_instances`."""

        payload = self.list_contract_instances(
            namespace,
            params={
                key: value
                for key, value in {
                    "contains": contains,
                    "hash_prefix": hash_prefix,
                    "offset": None if offset is None else int(offset),
                    "limit": None if limit is None else int(limit),
                    "order": order,
                }.items()
                if value is not None
            }
            or None,
        )
        if payload is None:
            raise RuntimeError("contract instances endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("contract instances endpoint returned non-object payload")
        return ContractInstancesPage.from_payload(payload)

    def governance_deploy_contract_proposal(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/proposals/deploy-contract`."""

        return self.request_json(
            "POST",
            "/v1/gov/proposals/deploy-contract",
            json_body=dict(payload),
        )

    def governance_submit_plain_ballot(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/ballots/plain`."""

        return self.request_json(
            "POST",
            "/v1/gov/ballots/plain",
            json_body=dict(payload),
        )

    def governance_submit_zk_ballot(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/ballots/zk`."""

        return self.request_json(
            "POST",
            "/v1/gov/ballots/zk",
            json_body=dict(payload),
        )

    def governance_submit_zk_ballot_v1(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/ballots/zk-v1`."""

        return self.request_json(
            "POST",
            "/v1/gov/ballots/zk-v1",
            json_body=dict(payload),
        )

    def governance_submit_zk_ballot_proof_v1(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/ballots/zk-v1/ballot-proof`."""

        return self.request_json(
            "POST",
            "/v1/gov/ballots/zk-v1/ballot-proof",
            json_body=dict(payload),
        )

    def governance_finalize_referendum(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/finalize`."""

        return self.request_json(
            "POST",
            "/v1/gov/finalize",
            json_body=dict(payload),
        )

    def governance_enact_proposal(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/enact`."""

        return self.request_json(
            "POST",
            "/v1/gov/enact",
            json_body=dict(payload),
        )

    def get_governance_proposal(self, proposal_id: str) -> Optional[Any]:
        """GET `/v1/gov/proposals/{proposal_id}`."""

        return self.request_json(
            "GET",
            f"/v1/gov/proposals/{proposal_id}",
            expected_status=(200, 404),
        )

    def get_governance_proposal_typed(self, proposal_id: str) -> GovernanceProposalResult:
        """Typed wrapper for :meth:`get_governance_proposal`."""

        payload = self.get_governance_proposal(proposal_id)
        if payload is None:
            return GovernanceProposalResult(found=False, proposal=None)
        return GovernanceProposalResult.from_payload(payload)

    def get_governance_referendum(self, referendum_id: str) -> Optional[Any]:
        """GET `/v1/gov/referenda/{referendum_id}`."""

        return self.request_json(
            "GET",
            f"/v1/gov/referenda/{referendum_id}",
            expected_status=(200, 404),
        )

    def get_governance_referendum_typed(
        self,
        referendum_id: str,
    ) -> GovernanceReferendumResult:
        """Typed wrapper for :meth:`get_governance_referendum`."""

        payload = self.get_governance_referendum(referendum_id)
        if payload is None:
            return GovernanceReferendumResult(found=False, referendum=None)
        return GovernanceReferendumResult.from_payload(payload)

    def get_governance_tally(self, referendum_id: str) -> Optional[Any]:
        """GET `/v1/gov/tally/{referendum_id}`."""

        return self.request_json(
            "GET",
            f"/v1/gov/tally/{referendum_id}",
            expected_status=(200, 404),
        )

    def get_governance_tally_typed(self, referendum_id: str) -> GovernanceTally:
        """Typed wrapper for :meth:`get_governance_tally`."""

        payload = self.get_governance_tally(referendum_id)
        if payload is None:
            return GovernanceTally(
                referendum_id=referendum_id,
                approve=0,
                reject=0,
                abstain=0,
            )
        return GovernanceTally.from_payload(payload)

    def get_governance_locks(self, referendum_id: str) -> Optional[Any]:
        """GET `/v1/gov/locks/{referendum_id}`."""

        return self.request_json(
            "GET",
            f"/v1/gov/locks/{referendum_id}",
            expected_status=(200, 404),
        )

    def get_governance_locks_typed(self, referendum_id: str) -> GovernanceLocksResult:
        """Typed wrapper for :meth:`get_governance_locks`."""

        payload = self.get_governance_locks(referendum_id)
        if payload is None:
            return GovernanceLocksResult(found=False, referendum_id=referendum_id, locks={})
        return GovernanceLocksResult.from_payload(payload)

    def get_governance_unlock_stats(self) -> Optional[Any]:
        """GET `/v1/gov/unlocks/stats`."""

        return self.request_json(
            "GET",
            "/v1/gov/unlocks/stats",
            expected_status=(200,),
        )

    def get_governance_unlock_stats_typed(self) -> GovernanceUnlockStats:
        """Typed wrapper for :meth:`get_governance_unlock_stats`."""

        payload = self.get_governance_unlock_stats()
        if payload is None:
            raise RuntimeError("governance unlock stats endpoint returned no payload")
        return GovernanceUnlockStats.from_payload(payload)

    def stream_events(
        self,
        *,
        filter: Optional[Any] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream JSON events from `/v1/events/sse`. Automatically retries on transient errors.

        The `filter` parameter accepts a JSON string, mapping, or an
        :class:`iroha_python.event_filter.EventFilter` instance.
        """

        filter_payload = ensure_event_filter(filter)
        params = {"filter": filter_payload} if filter_payload else None
        initial_event_id = last_event_id if last_event_id is not None else (
            cursor.last_event_id if cursor is not None else None
        )
        should_resume = resume or cursor is not None or last_event_id is not None

        def _handle(event: SseEvent) -> None:
            if on_event is None:
                return
            if with_metadata:
                on_event(event)
            else:
                on_event(event.data, event.id)

        iterator = self._stream_sse(
            "/v1/events/sse",
            params=params,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=initial_event_id,
            resume=should_resume,
            decode_json=decode_json,
            cursor=cursor,
            on_event=_handle if on_event is not None else None,
        )
        if with_metadata:
            return iterator
        return (event.data for event in iterator)

    def stream_sumeragi_status(
        self,
        *,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream `/v1/sumeragi/status/sse` for live consensus metrics."""

        initial_event_id = last_event_id if last_event_id is not None else (
            cursor.last_event_id if cursor is not None else None
        )
        should_resume = resume or cursor is not None or last_event_id is not None

        def _handle(event: SseEvent) -> None:
            if on_event is None:
                return
            if with_metadata:
                on_event(event)
            else:
                on_event(event.data, event.id)

        iterator = self._stream_sse(
            "/v1/sumeragi/status/sse",
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=initial_event_id,
            resume=should_resume,
            decode_json=decode_json,
            cursor=cursor,
            on_event=_handle if on_event is not None else None,
        )
        if with_metadata:
            return iterator
        return (event.data for event in iterator)

    def stream_sumeragi_new_view(
        self,
        *,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream `/v1/sumeragi/new_view/sse` for live NEW_VIEW receipt counts."""

        initial_event_id = last_event_id if last_event_id is not None else (
            cursor.last_event_id if cursor is not None else None
        )
        should_resume = resume or cursor is not None or last_event_id is not None

        def _handle(event: SseEvent) -> None:
            if on_event is None:
                return
            if with_metadata:
                on_event(event)
            else:
                on_event(event.data, event.id)

        iterator = self._stream_sse(
            "/v1/sumeragi/new_view/sse",
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=initial_event_id,
            resume=should_resume,
            decode_json=decode_json,
            cursor=cursor,
            on_event=_handle if on_event is not None else None,
        )
        if with_metadata:
            return iterator
        return (event.data for event in iterator)

    def stream_verifying_key_events(
        self,
        *,
        backend: Optional[str] = None,
        name: Optional[str] = None,
        registered: bool = True,
        updated: bool = True,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream verifying-key lifecycle events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.verifying_key(
            backend=backend,
            name=name,
            registered=registered,
            updated=updated,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=last_event_id,
            resume=resume,
            on_event=on_event,
            cursor=cursor,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_proof_events(
        self,
        *,
        backend: Optional[str] = None,
        proof_hash_hex: Optional[str] = None,
        verified: bool = True,
        rejected: bool = True,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream proof verification events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.proof(
            backend=backend,
            proof_hash_hex=proof_hash_hex,
            verified=verified,
            rejected=rejected,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=last_event_id,
            resume=resume,
            on_event=on_event,
            cursor=cursor,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_trigger_events(
        self,
        *,
        trigger_id: Optional[str] = None,
        created: bool = True,
        deleted: bool = True,
        extended: bool = True,
        shortened: bool = True,
        metadata_inserted: bool = True,
        metadata_removed: bool = True,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream trigger lifecycle events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.trigger(
            trigger_id=trigger_id,
            created=created,
            deleted=deleted,
            extended=extended,
            shortened=shortened,
            metadata_inserted=metadata_inserted,
            metadata_removed=metadata_removed,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=last_event_id,
            resume=resume,
            on_event=on_event,
            cursor=cursor,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_pipeline_transactions(
        self,
        *,
        hash_hex: Optional[str] = None,
        block_height: Optional[int] = None,
        status: Optional[str] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream pipeline transaction events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.pipeline_transaction(
            hash_hex=hash_hex,
            block_height=block_height,
            status=status,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=last_event_id,
            resume=resume,
            on_event=on_event,
            cursor=cursor,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_pipeline_blocks(
        self,
        *,
        height: Optional[int] = None,
        status: Optional[str] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream pipeline block events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.pipeline_block(
            height=height,
            status=status,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=last_event_id,
            resume=resume,
            on_event=on_event,
            cursor=cursor,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_pipeline_witnesses(
        self,
        *,
        block_hash_hex: Optional[str] = None,
        height: Optional[int] = None,
        view: Optional[int] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream execution witness events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.pipeline_witness(
            block_hash_hex=block_hash_hex,
            height=height,
            view=view,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=last_event_id,
            resume=resume,
            on_event=on_event,
            cursor=cursor,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_pipeline_merges(
        self,
        *,
        epoch_id: Optional[int] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream merge-ledger events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.pipeline_merge(epoch_id=epoch_id)
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=last_event_id,
            resume=resume,
            on_event=on_event,
            cursor=cursor,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    # ------------------------------------------------------------------
    # Triggers API
    # ------------------------------------------------------------------
    def list_triggers(
        self,
        *,
        namespace: Optional[str] = None,
        authority: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Optional[Any]:
        """GET `/v1/triggers` with optional filtering."""

        params: Dict[str, Any] = {}
        namespace_value = _normalize_optional_string(namespace, "list_triggers.namespace")
        if namespace_value is not None:
            params["namespace"] = namespace_value
        authority_value = _normalize_optional_string(authority, "list_triggers.authority")
        if authority_value is not None:
            params["authority"] = authority_value
        limit_value = _coerce_int(limit, "list_triggers.limit") if limit is not None else None
        if limit_value is not None:
            params["limit"] = limit_value
        offset_value = (
            _coerce_int(offset, "list_triggers.offset", allow_zero=True)
            if offset is not None
            else None
        )
        if offset_value is not None:
            params["offset"] = offset_value
        return self.request_json(
            "GET",
            "/v1/triggers",
            params=params or None,
            expected_status=(200,),
        )

    def list_triggers_typed(
        self,
        *,
        namespace: Optional[str] = None,
        authority: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> TriggerListPage:
        """Typed wrapper for :meth:`list_triggers`."""

        payload = self.list_triggers(
            namespace=namespace,
            authority=authority,
            limit=limit,
            offset=offset,
        )
        if payload is None:
            return TriggerListPage(items=[], total=0)
        return TriggerListPage.from_payload(payload)

    def get_trigger(self, trigger_id: str) -> Optional[Any]:
        """GET `/v1/triggers/{trigger_id}` and return the stored trigger or `None` when missing."""

        normalized_id = _require_non_empty_string(trigger_id, "trigger_id")
        return self.request_json(
            "GET",
            f"/v1/triggers/{normalized_id}",
            expected_status=(200, 404),
        )

    def get_trigger_typed(self, trigger_id: str) -> Optional[TriggerRecord]:
        """Typed wrapper for :meth:`get_trigger`."""

        payload = self.get_trigger(trigger_id)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("trigger endpoint returned non-object payload")
        return TriggerRecord.from_payload(payload)

    def register_trigger(self, trigger: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/triggers` with a trigger registration payload."""

        if not isinstance(trigger, Mapping):
            raise TypeError("trigger must be a mapping")
        return self.request_json(
            "POST",
            "/v1/triggers",
            json_body=dict(trigger),
            expected_status=(200, 201, 202),
        )

    def register_trigger_typed(self, trigger: Mapping[str, Any]) -> Optional[TriggerMutationResponse]:
        """Typed wrapper for :meth:`register_trigger`."""

        payload = self.register_trigger(trigger)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("trigger registration returned malformed payload")
        return TriggerMutationResponse.from_payload(payload)

    def delete_trigger(self, trigger_id: str) -> Optional[Any]:
        """DELETE `/v1/triggers/{trigger_id}`."""

        response = self._request("DELETE", f"/v1/triggers/{trigger_id}")
        self._expect_status(response, {200, 202, 204, 404})
        return self._maybe_json(response)

    def delete_trigger_typed(self, trigger_id: str) -> Optional[TriggerMutationResponse]:
        """Typed wrapper for :meth:`delete_trigger`."""

        payload = self.delete_trigger(trigger_id)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("trigger deletion returned malformed payload")
        return TriggerMutationResponse.from_payload(payload)

    def query_triggers(
        self,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/triggers/query` with a structured envelope."""

        body: Dict[str, Any] = {}
        if filter is not None:
            if not isinstance(filter, Mapping):
                raise TypeError("query_triggers.filter must be a mapping")
            body["filter"] = dict(filter)
        if sort is not None:
            body["sort"] = sort
        limit_value = _coerce_int(limit, "query_triggers.limit") if limit is not None else None
        if limit_value is not None:
            body["limit"] = limit_value
        offset_value = (
            _coerce_int(offset, "query_triggers.offset", allow_zero=True)
            if offset is not None
            else None
        )
        if offset_value is not None:
            body["offset"] = offset_value
        fetch_size_value = (
            _coerce_int(fetch_size, "query_triggers.fetch_size") if fetch_size is not None else None
        )
        if fetch_size_value is not None:
            body["fetch_size"] = fetch_size_value
        query_name_value = _normalize_optional_string(query_name, "query_triggers.query_name")
        if query_name_value is not None:
            body["query_name"] = query_name_value
        response = self._request(
            "POST",
            "/v1/triggers/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected triggers query response")
        return payload

    def query_triggers_typed(
        self,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
    ) -> TriggerListPage:
        """Typed wrapper for :meth:`query_triggers`."""

        payload = self.query_triggers(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
        )
        return TriggerListPage.from_payload(payload)

    @staticmethod
    def _maybe_json(response: requests.Response) -> Optional[Any]:
        if not response.content:
            return None
        try:
            return response.json()
        except ValueError:
            return response.text or None

    @staticmethod
    def _parse_sse_event(
        lines: Iterable[str],
        *,
        decode_json: bool = True,
    ) -> Optional[SseEvent]:
        raw_lines = list(lines)
        data_chunks: List[str] = []
        event_name: Optional[str] = None
        event_id: Optional[str] = None
        retry_value: Optional[int] = None
        for entry in raw_lines:
            if entry.startswith(":"):
                continue
            field, sep, value = entry.partition(":")
            value = value.lstrip() if sep else ""
            if field == "data":
                data_chunks.append(value)
            elif field == "id":
                event_id = value or None
            elif field == "event":
                event_name = value or None
            elif field == "retry":
                try:
                    retry_value = int(value)
                except ValueError:
                    retry_value = None
        if not data_chunks and event_name is None and event_id is None and retry_value is None:
            return None
        payload: Any
        if data_chunks:
            joined = "\n".join(data_chunks)
            if decode_json:
                try:
                    payload = json.loads(joined)
                except json.JSONDecodeError:
                    payload = joined
            else:
                payload = joined
        else:
            payload = None
        return SseEvent(
            event=event_name,
            data=payload,
            id=event_id,
            retry=retry_value,
            raw="\n".join(raw_lines),
        )

    def _stream_sse(
        self,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
        max_retries: Optional[int] = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        decode_json: bool = True,
        cursor: Optional[EventCursor] = None,
        on_event: Optional[Callable[[SseEvent], None]] = None,
    ):
        url = f"{self._base_url}{path}"
        active_last_id = last_event_id if last_event_id is not None else (
            cursor.last_event_id if cursor is not None else None
        )
        should_resume = resume or last_event_id is not None or cursor is not None

        def iterator():
            nonlocal active_last_id
            attempt = 0
            backoff = max(backoff_base, 0.0)
            while True:
                try:
                    final_headers: Dict[str, str] = dict(self._default_headers)
                    final_headers.pop("Accept", None)
                    if headers:
                        final_headers.update(headers)
                    final_headers.setdefault("Accept", "text/event-stream")
                    if should_resume and active_last_id:
                        final_headers["Last-Event-ID"] = active_last_id
                    with self._session.get(
                        url,
                        params=params,
                        headers=final_headers or None,
                        stream=True,
                        timeout=timeout,
                    ) as response:
                        self._expect_status(response, {200})
                        attempt = 0
                        backoff = max(backoff_base, 0.0)
                        buffer: list[str] = []
                        for raw_line in response.iter_lines(decode_unicode=True):
                            if raw_line is None:
                                continue
                            line = raw_line.strip()
                            if not line:
                                if buffer:
                                    event = self._parse_sse_event(buffer, decode_json=decode_json)
                                    buffer.clear()
                                    if event is None:
                                        continue
                                    if event.id is not None:
                                        active_last_id = event.id
                                        if cursor is not None:
                                            cursor.advance(event)
                                    if on_event is not None:
                                        on_event(event)
                                    yield event
                                continue
                            buffer.append(line)
                        if buffer:
                            event = self._parse_sse_event(buffer, decode_json=decode_json)
                            buffer.clear()
                            if event is not None:
                                if event.id is not None:
                                    active_last_id = event.id
                                    if cursor is not None:
                                        cursor.advance(event)
                                if on_event is not None:
                                    on_event(event)
                                yield event
                        break
                except requests.RequestException:
                    attempt += 1
                    if max_retries is not None and attempt > max_retries:
                        raise
                    if backoff > 0.0:
                        time.sleep(backoff)
                        backoff *= 2
                    continue

        return iterator()

    @staticmethod
    def _build_query_envelope(
        *,
        filter: Optional[Mapping[str, Any]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        body: Dict[str, Any] = {}
        if filter is not None:
            body["filter"] = dict(filter)
        if sort is not None:
            body["sort"] = sort
        if limit is not None:
            body["limit"] = int(limit)
        if offset is not None:
            body["offset"] = int(offset)
        if fetch_size is not None:
            body["fetch_size"] = int(fetch_size)
        if query_name is not None:
            body["query_name"] = query_name
        return body

    @staticmethod
    def _ensure_no_query_args(
        *,
        envelope: Mapping[str, Any],
        filter: Optional[Mapping[str, Any]],
        sort: Optional[Any],
        limit: Optional[int],
        offset: Optional[int],
        fetch_size: Optional[int],
        query_name: Optional[str],
    ) -> None:
        if any(
            value is not None
            for value in (filter, sort, limit, offset, fetch_size, query_name)
        ):
            raise ValueError(
                "provide either `envelope` or builder arguments (filter/sort/limit/offset/fetch_size/query_name), not both"
            )


def create_torii_client(
    base_url: str,
    *,
    session: Optional[requests.Session] = None,
    auth_token: Optional[str] = None,
    api_token: Optional[str] = None,
    default_headers: Optional[Mapping[str, str]] = None,
    timeout: Optional[float] = 30.0,
    max_retries: int = 3,
    backoff_factor: float = 0.5,
    retry_on_status: Optional[Sequence[int]] = None,
    retry_on_methods: Optional[Sequence[str]] = None,
    config: Optional[Mapping[str, Any]] = None,
    env: Optional[Mapping[str, str]] = None,
    overrides: Optional[Mapping[str, Any]] = None,
    resolved_config: Optional[ResolvedToriiClientConfig] = None,
    sorafs_alias_policy: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]] = None,
    sorafs_alias_warning: Optional[Callable[[SorafsAliasWarning], None]] = None,
    sorafs_alias_logger: Optional[logging.Logger] = None,
) -> ToriiClient:
    """Return a :class:`ToriiClient` instance with the given base URL."""
    resolved = resolved_config
    if resolved is None and (config is not None or overrides is not None or env is not None):
        resolved = resolve_torii_client_config(config=config, env=env, overrides=overrides)

    header_merge: Dict[str, str] = (
        dict(resolved.default_headers) if resolved is not None else {"Accept": "application/json"}
    )
    if default_headers:
        header_merge.update(default_headers)

    auth_value = auth_token if auth_token is not None else (resolved.auth_token if resolved else None)
    api_value = api_token if api_token is not None else (resolved.api_token if resolved else None)
    timeout_value = timeout if timeout is not None else (resolved.timeout if resolved else 30.0)
    max_retries_value = max_retries if max_retries is not None else (
        resolved.max_retries if resolved else 3
    )
    retry_statuses = retry_on_status if retry_on_status is not None else (
        list(resolved.retry_statuses) if resolved else None
    )
    retry_methods = retry_on_methods if retry_on_methods is not None else (
        list(resolved.retry_methods) if resolved else None
    )
    if resolved is not None:
        backoff_initial_ms = int(resolved.backoff_initial * 1000)
        max_backoff_ms = None if math.isinf(resolved.max_backoff) else int(resolved.max_backoff * 1000)
        backoff_mult = resolved.backoff_multiplier
    else:
        backoff_initial_ms = None
        max_backoff_ms = None
        backoff_mult = None

    policy_value: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]] = sorafs_alias_policy
    if policy_value is None and resolved is not None:
        policy_value = resolved.sorafs_alias_policy

    return ToriiClient(
        base_url,
        session=session,
        auth_token=auth_value,
        api_token=api_value,
        default_headers=header_merge,
        timeout=timeout_value,
        max_retries=max_retries_value,
        backoff_factor=backoff_factor,
        backoff_initial_ms=backoff_initial_ms,
        max_backoff_ms=max_backoff_ms,
        backoff_multiplier=backoff_mult,
        retry_on_status=retry_statuses,
        retry_on_methods=retry_methods,
        sorafs_alias_policy=policy_value,
        sorafs_alias_warning=sorafs_alias_warning,
        sorafs_alias_logger=sorafs_alias_logger,
    )


if not hasattr(ToriiClient, "_maybe_json"):
    def _torii_client_maybe_json(response: requests.Response) -> Optional[Any]:
        if not response.content:
            return None
        try:
            return response.json()
        except ValueError:
            return response.text or None

    ToriiClient._maybe_json = staticmethod(_torii_client_maybe_json)
