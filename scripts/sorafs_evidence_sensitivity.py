"""Shared sensitive-field checks for SoraFS evidence gates."""

from __future__ import annotations

from typing import Any, Iterable


COMMON_SENSITIVE_KEYS = frozenset(
    {
        "access_key",
        "access_token",
        "api_key",
        "authorization",
        "authorization_header",
        "bearer_token",
        "client_secret",
        "private_key",
        "response_body",
        "secret",
        "seed",
        "seed_phrase",
        "signing_key",
        "token",
    }
)
HIGH_RISK_SENSITIVE_KEY_FRAGMENTS = frozenset(
    {
        "accesskey",
        "accesstoken",
        "apikey",
        "authorizationheader",
        "bearertoken",
        "clientsecret",
        "privatekey",
        "requestbody",
        "responsebody",
        "seedphrase",
        "signingkey",
    }
)
PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES = frozenset(
    {
        "absent",
        "blake3",
        "digest",
        "digesthex",
        "hash",
        "hashhex",
        "included",
        "present",
        "sha256",
    }
)
MAX_SENSITIVE_FIELD_DEPTH = 128


def normalize_sensitive_key(key: str) -> str:
    """Return a punctuation-insensitive key form for secret-field checks."""

    return "".join(char.lower() for char in key if char.isalnum())


def _key_forms(sensitive_keys: Iterable[str]) -> tuple[frozenset[str], frozenset[str]]:
    exact_keys = frozenset(key.lower() for key in sensitive_keys) | COMMON_SENSITIVE_KEYS
    normalized_keys = frozenset(normalize_sensitive_key(key) for key in exact_keys)
    return exact_keys, normalized_keys


def _is_inclusion_marker(normalized_key: str) -> bool:
    return normalized_key.endswith("included")


def _is_allowed_inclusion_marker_value(value: Any) -> bool:
    return value is False


def _is_payload_free_sensitive_reference(normalized_key: str) -> bool:
    return any(
        normalized_key.endswith(suffix)
        for suffix in PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES
    )


def _is_sensitive_key(
    key_lower: str,
    normalized_key: str,
    *,
    exact_keys: frozenset[str],
    normalized_keys: frozenset[str],
) -> bool:
    return (
        key_lower in exact_keys
        or normalized_key in normalized_keys
        or any(
            fragment in normalized_key
            and not _is_payload_free_sensitive_reference(normalized_key)
            for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
        )
    )


def _visit_sensitive_fields(
    value: Any,
    path: str,
    errors: list[str],
    *,
    exact_keys: frozenset[str],
    normalized_keys: frozenset[str],
    evidence_label: str,
    depth: int = 0,
) -> None:
    if depth > MAX_SENSITIVE_FIELD_DEPTH:
        errors.append(
            "{} nesting exceeds {} levels".format(
                path or "<root>",
                MAX_SENSITIVE_FIELD_DEPTH,
            )
        )
        return
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}.{key}" if path else key
            key_lower = key.lower()
            normalized_key = normalize_sensitive_key(key)
            if _is_sensitive_key(
                key_lower,
                normalized_key,
                exact_keys=exact_keys,
                normalized_keys=normalized_keys,
            ):
                errors.append(f"{child_path} must not be present in {evidence_label}")
            if _is_inclusion_marker(
                normalized_key
            ) and not _is_allowed_inclusion_marker_value(child):
                errors.append(f"{child_path} must be false")
            _visit_sensitive_fields(
                child,
                child_path,
                errors,
                exact_keys=exact_keys,
                normalized_keys=normalized_keys,
                evidence_label=evidence_label,
                depth=depth + 1,
            )
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _visit_sensitive_fields(
                child,
                f"{path}[{index}]",
                errors,
                exact_keys=exact_keys,
                normalized_keys=normalized_keys,
                evidence_label=evidence_label,
                depth=depth + 1,
            )


def visit_sensitive_fields(
    value: Any,
    path: str,
    errors: list[str],
    *,
    sensitive_keys: Iterable[str],
    evidence_label: str = "rollout evidence",
) -> None:
    """Reject sensitive fields and non-false payload-inclusion markers."""

    exact_keys, normalized_keys = _key_forms(sensitive_keys)
    _visit_sensitive_fields(
        value,
        path,
        errors,
        exact_keys=exact_keys,
        normalized_keys=normalized_keys,
        evidence_label=evidence_label,
    )
