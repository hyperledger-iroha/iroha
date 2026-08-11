"""Exact operator-auth transport for the existing ISO 20022 client APIs."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple
from urllib.parse import urlencode

_RETIRED_ISO_AUTH_HEADERS = frozenset(
    {
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce",
        "x-iroha-witness",
        "x-iroha-iso-profile",
        "x-iroha-operator-public-key",
        "x-iroha-operator-timestamp-ms",
        "x-iroha-operator-nonce",
        "x-iroha-operator-signature",
    }
)
_PROFILE_ID = re.compile(r"[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\Z")
_ISO_STATUS_VALUES = {
    "pending": "Pending",
    "accepted": "Accepted",
    "committed": "Committed",
    "rejected": "Rejected",
}
_ISO_NON_TERMINAL_STATUSES = frozenset({"Pending", "Accepted"})
_PACS002_STATUS_CODES = frozenset({"ACTC", "ACSP", "ACSC", "ACWC", "PDNG", "RJCT"})


def normalize_iso_optional_string(
    value: Any,
    context: str,
    *,
    allow_empty: bool = False,
) -> Optional[str]:
    """Normalize one optional ISO string field."""

    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed and not allow_empty:
        return None
    return trimmed


def normalize_iso_string_array(value: Any, context: str) -> Tuple[str, ...]:
    """Normalize an optional ISO string array."""

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


def normalize_iso_status(value: Any, context: str) -> str:
    """Normalize an ISO submission status."""

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


def normalize_pacs002_code(value: Any, context: str) -> Optional[str]:
    """Normalize an optional pacs.002 status code."""

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


def is_iso_status_terminal(status: Optional[Any], resolve_on_accepted: bool) -> bool:
    """Return whether a parsed ISO status satisfies the caller's wait policy."""

    if status is None:
        return False
    if status.status in _ISO_NON_TERMINAL_STATUSES:
        return status.status == "Accepted" and resolve_on_accepted
    return True


def normalize_iso_wait_kwargs(
    options: Optional[Mapping[str, Any]],
    *,
    context: str,
) -> Dict[str, Any]:
    """Validate the bounded options accepted by ISO submit-and-wait helpers."""

    if options is None:
        return {}
    if not isinstance(options, Mapping):
        raise TypeError(f"{context} must be a mapping when provided")
    allowed = ("poll_interval", "max_attempts", "resolve_on_accepted", "timeout", "on_poll")
    extras = [key for key in options.keys() if key not in allowed]
    if extras:
        extras_str = ", ".join(sorted(extras))
        raise ValueError(f"{context} contains unsupported fields: {extras_str}")
    normalized: Dict[str, Any] = {}
    for key in allowed:
        if key in options:
            normalized[key] = options[key]
    return normalized


@dataclass(frozen=True)
class OperatorSigningContext:
    """Immutable exact-network key pair for generated Torii operator auth."""

    network_id: Any
    key_pair: Any

    def __post_init__(self) -> None:
        from .crypto import _require_network_id

        network_id = _require_network_id(
            self.network_id,
            "OperatorSigningContext.network_id",
        )
        signer = getattr(self.key_pair, "sign", None)
        public_key = getattr(self.key_pair, "public_key_multihash", None)
        if not callable(signer):
            raise TypeError("OperatorSigningContext.key_pair must expose sign(message)")
        if not isinstance(public_key, str):
            raise TypeError("OperatorSigningContext.key_pair must expose public_key_multihash")
        if (
            not public_key
            or public_key.strip() != public_key
            or not public_key.isascii()
            or any(ord(character) < 0x21 or ord(character) > 0x7E for character in public_key)
        ):
            raise ValueError("operator public key must be exact non-empty printable ASCII")
        object.__setattr__(self, "network_id", network_id)


class ToriiClientIsoOperatorContextMixin:
    """Store the operator context without exposing a mutable public setter."""

    __operator_signing_context: Optional[OperatorSigningContext] = None

    def _install_operator_signing_context(
        self,
        value: Optional[OperatorSigningContext],
    ) -> None:
        if value is not None and not isinstance(value, OperatorSigningContext):
            raise TypeError("operator_signing_context must be an OperatorSigningContext")
        self.__operator_signing_context = value

    @property
    def operator_signing_context(self) -> Optional[OperatorSigningContext]:
        """Immutable exact-network context used only by operator APIs."""

        return self.__operator_signing_context

    def _submit_iso_message(
        self,
        path: str,
        message: Any,
        *,
        content_type: Optional[str],
        profile: Optional[str],
        timeout: Optional[float],
        context: str,
    ) -> Optional[Any]:
        payload = _normalize_payload(message, f"{context}.message")
        return submit_iso_message(
            self,
            path,
            payload,
            content_type=content_type,
            profile=profile,
            timeout=timeout,
            context=context,
        )

    def _operator_get(
        self,
        path: str,
        *,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
        context: str = "operator GET",
    ) -> Any:
        """Sign and dispatch one exact-network operator GET without redirect or retry."""

        return operator_get(
            self,
            path,
            headers=headers,
            timeout=timeout,
            stream=stream,
            context=context,
        )


def _require_profile(value: Optional[str], context: str) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    if _PROFILE_ID.fullmatch(value) is None:
        raise ValueError(f"{context} must be a canonical lowercase profile id")
    return value


def _normalize_payload(message: Any, context: str) -> bytes:
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


def _reject_retired_auth(
    client: Any,
    context: str,
    headers: Optional[Mapping[str, Any]] = None,
) -> None:
    session = getattr(client, "_session", None)
    for source in (
        getattr(client, "_default_headers", {}),
        getattr(session, "headers", {}),
        headers or {},
    ):
        for name in source:
            if str(name).lower() in _RETIRED_ISO_AUTH_HEADERS:
                raise ValueError(
                    f"{context} requires generated operator signing; header {name} is not accepted"
                )
    if getattr(session, "auth", None) is not None:
        raise ValueError(
            f"{context} requires generated operator signing; Session.auth is not accepted"
        )


def _require_one_shot_transport(client: Any, target: str, context: str) -> None:
    try:
        retry_total = client._session.get_adapter(f"{client._base_url}{target}").max_retries.total
    except (AttributeError, LookupError, ValueError) as exc:
        raise ValueError(f"{context} requires a verifiable one-shot HTTP transport") from exc
    if retry_total is not False and retry_total != 0:
        raise ValueError(f"{context} requires transport retries to be disabled")


def _signed_headers(
    client: Any,
    method: str,
    target: str,
    body: bytes,
    context: str,
) -> dict[str, str]:
    signing_context = client.operator_signing_context
    if signing_context is None:
        raise ValueError(f"{context} requires immutable operator_signing_context")
    _reject_retired_auth(client, context)
    _require_one_shot_transport(client, target, context)
    return client.build_operator_signature_headers(
        network_id=signing_context.network_id,
        method=method,
        path=target,
        body=body,
        key_pair=signing_context.key_pair,
    )


def operator_get(
    client: Any,
    path: str,
    *,
    headers: Optional[Mapping[str, str]],
    timeout: Optional[float],
    stream: bool,
    context: str,
) -> Any:
    """Sign and dispatch one exact-path, empty-body operator GET."""

    if not isinstance(path, str) or not path.startswith("/") or "#" in path:
        raise ValueError(f"{context} path must be an absolute-path reference without a fragment")
    _reject_retired_auth(client, context, headers)
    final_headers = _signed_headers(client, "GET", path, b"", context)
    if headers is not None:
        for name, value in headers.items():
            final_headers[str(name)] = str(value)
    response = client._request(
        "GET",
        path,
        data=b"",
        headers=final_headers,
        timeout=timeout,
        stream=stream,
        allow_retry=False,
        allow_redirects=False,
    )
    return response


def submit_iso_message(
    client: Any,
    path: str,
    payload: bytes,
    *,
    content_type: Optional[str],
    profile: Optional[str],
    timeout: Optional[float],
    context: str,
) -> Optional[Any]:
    """Sign and dispatch one existing pacs submission exactly once."""

    profile_id = _require_profile(profile, f"{context}.profile")
    target = path if profile_id is None else f"{path}?{urlencode({'profile': profile_id})}"
    headers = _signed_headers(client, "POST", target, payload, context)
    headers.update(
        {
            "Content-Type": content_type.strip()
            if isinstance(content_type, str) and content_type.strip()
            else "application/xml",
            "Accept": "application/json",
        }
    )
    response = client._request(
        "POST",
        target,
        data=payload,
        headers=headers,
        timeout=timeout,
        allow_retry=False,
        allow_redirects=False,
    )
    client._expect_status(response, (202,))
    return client._maybe_json(response)


def get_iso_message_status(
    client: Any,
    path: str,
    *,
    timeout: Optional[float],
    context: str,
) -> Optional[Any]:
    """Sign and dispatch one existing ISO status read exactly once."""

    headers = _signed_headers(client, "GET", path, b"", context)
    headers["Accept"] = "application/json"
    response = client._request(
        "GET",
        path,
        headers=headers,
        timeout=timeout,
        allow_retry=False,
        allow_redirects=False,
    )
    client._expect_status(response, (200,))
    return client._maybe_json(response)
