"""Shared sensitive-field checks for SoraFS evidence gates."""

from __future__ import annotations

import re
import unicodedata
from collections.abc import Iterable, Mapping
from html import unescape
from typing import Any
from urllib.parse import urlsplit, unquote


COMMON_SENSITIVE_KEYS = frozenset(
    {
        "access_key",
        "access_token",
        "api_key",
        "api_token",
        "auth_token",
        "authorization",
        "authorization_header",
        "bearer_token",
        "client_secret",
        "cookie",
        "id_token",
        "jwt",
        "oauth_token",
        "password",
        "private_key",
        "refresh_token",
        "response_body",
        "secret",
        "seed",
        "seed_phrase",
        "session_token",
        "signing_key",
        "set_cookie",
        "token",
        "x_api_token",
    }
)
HIGH_RISK_SENSITIVE_KEY_FRAGMENTS = frozenset(
    {
        "accesskey",
        "accesstoken",
        "apikey",
        "apitoken",
        "authtoken",
        "authorizationheader",
        "bearertoken",
        "clientsecret",
        "idtoken",
        "oauth",
        "oauthtoken",
        "password",
        "privatekey",
        "refreshtoken",
        "requestbody",
        "responsebody",
        "seedphrase",
        "setcookie",
        "sessiontoken",
        "signingkey",
        "xapitoken",
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
MAX_SENSITIVE_KEY_DECODE_PASSES = 4
SECRET_VALUE_AUTH_PREFIXES = (
    "authorization: bearer ",
    "authorization: basic ",
    "bearer ",
    "basic ",
)
SECRET_VALUE_COOKIE_PREFIXES = (
    "cookie:",
    "set-cookie:",
)
SECRET_VALUE_PEM_MARKERS = (
    "-----begin private key-----",
    "-----begin rsa private key-----",
    "-----begin ec private key-----",
    "-----begin openssh private key-----",
)
JWT_LIKE_VALUE_RE = re.compile(
    r"(?<![A-Za-z0-9_-])"
    r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"
    r"(?![A-Za-z0-9_-])"
)
URL_SCHEME_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")


def _require_error_list(errors: Any) -> list[str]:
    """Return a mutable sensitivity error list or reject malformed sinks."""

    if not isinstance(errors, list):
        raise ValueError("sensitive field errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("sensitive field errors must be a list of strings")
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "sensitive field errors must contain non-empty canonical strings"
            )
    return errors


def _require_diagnostic_string(
    value: Any,
    errors: list[str],
    *,
    label: str,
    allow_empty: bool = False,
) -> str | None:
    """Return canonical diagnostic text or reject unsafe error fragments."""

    if not isinstance(value, str):
        errors.append(f"{label} must be a non-empty canonical string")
        return None
    if allow_empty and value == "":
        return value
    if (
        not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{label} must be a non-empty canonical string")
        return None
    return value


def normalize_sensitive_key(key: str) -> str:
    """Return a punctuation-insensitive key form for secret-field checks."""

    return "".join(char.lower() for char in key if char.isalnum())


COMMON_SENSITIVE_KEY_NORMALIZED = frozenset(
    normalize_sensitive_key(key) for key in COMMON_SENSITIVE_KEYS
)


def _decoded_key_variants(key: str) -> tuple[str, ...]:
    """Return raw plus bounded percent/HTML-decoded key variants."""

    variants = [key]
    seen = {key}
    current = key
    for _ in range(MAX_SENSITIVE_KEY_DECODE_PASSES):
        decoded = unescape(unquote(current))
        if decoded == current or decoded in seen:
            break
        variants.append(decoded)
        seen.add(decoded)
        current = decoded
    return tuple(variants)


def _value_scan_variants(value: str) -> tuple[str, ...]:
    """Return decoded value variants plus Unicode format-control-free aliases."""

    variants: list[str] = []
    seen: set[str] = set()
    for decoded in _decoded_key_variants(value):
        for candidate in (
            decoded,
            "".join(
                character
                for character in decoded
                if unicodedata.category(character) != "Cf"
            ),
        ):
            if candidate not in seen:
                variants.append(candidate)
                seen.add(candidate)
    return tuple(variants)


def _is_canonical_sensitive_key(key: str) -> bool:
    if not key or not ("a" <= key[0] <= "z"):
        return False
    previous_underscore = False
    for character in key:
        if "a" <= character <= "z" or "0" <= character <= "9":
            previous_underscore = False
        elif character == "_" and not previous_underscore:
            previous_underscore = True
        else:
            return False
    return not previous_underscore


def _key_forms(
    sensitive_keys: Any,
    errors: list[str],
) -> tuple[frozenset[str], frozenset[str]] | None:
    if (
        isinstance(sensitive_keys, (str, bytes, bytearray, Mapping))
        or not isinstance(sensitive_keys, Iterable)
    ):
        errors.append("sensitive keys must be a sequence of strings")
        return None
    provided_keys: set[str] = set()
    provided_normalized_keys: set[str] = set()
    common_normalized_keys = {
        normalize_sensitive_key(key): key for key in COMMON_SENSITIVE_KEYS
    }
    for key in sensitive_keys:
        if (
            not isinstance(key, str)
            or not _is_canonical_sensitive_key(key)
        ):
            errors.append("sensitive keys must be non-empty canonical strings")
            return None
        normalized_key = normalize_sensitive_key(key)
        common_key = common_normalized_keys.get(normalized_key)
        if common_key is not None and key != common_key:
            errors.append("sensitive keys must not alias common sensitive names")
            return None
        if key in provided_keys or normalized_key in provided_normalized_keys:
            errors.append("sensitive keys must not contain duplicate normalized names")
            return None
        provided_keys.add(key)
        provided_normalized_keys.add(normalized_key)
    exact_keys = frozenset(provided_keys) | COMMON_SENSITIVE_KEYS
    normalized_keys = frozenset(normalize_sensitive_key(key) for key in exact_keys)
    return exact_keys, normalized_keys


def _is_inclusion_marker(normalized_key: str) -> bool:
    return normalized_key.endswith("included")


def _is_allowed_inclusion_marker_value(value: Any) -> bool:
    return value is False


def _inclusion_marker_stem_variants(normalized_key: str) -> tuple[str, ...]:
    stem = normalized_key[: -len("included")]
    variants = {stem}
    if stem.endswith("s"):
        variants.add(stem[:-1])
    if "bodies" in stem:
        variants.add(stem.replace("bodies", "body"))
    return tuple(variant for variant in variants if variant)


def _is_sensitive_inclusion_marker(
    normalized_key: str,
    *,
    normalized_keys: frozenset[str],
) -> bool:
    if not _is_inclusion_marker(normalized_key):
        return False
    for stem in _inclusion_marker_stem_variants(normalized_key):
        if any(fragment in stem for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS):
            return True
        if any(sensitive_key in stem for sensitive_key in normalized_keys):
            return True
    return False


def _is_payload_free_sensitive_reference(normalized_key: str) -> bool:
    return any(
        normalized_key.endswith(suffix)
        for suffix in PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES
    )


def _is_sensitive_key(
    key_lower_variants: tuple[str, ...],
    normalized_key_variants: tuple[str, ...],
    *,
    exact_keys: frozenset[str],
    normalized_keys: frozenset[str],
) -> bool:
    return any(key_lower in exact_keys for key_lower in key_lower_variants) or any(
        normalized_key in normalized_keys
        or any(
            fragment in normalized_key
            and not _is_payload_free_sensitive_reference(normalized_key)
            for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
        )
        for normalized_key in normalized_key_variants
    )


def _is_ascii_alphanumeric(character: str) -> bool:
    return (
        "a" <= character <= "z"
        or "A" <= character <= "Z"
        or "0" <= character <= "9"
    )


def _is_canonical_path_segment(value: str) -> bool:
    if (
        not value
        or not _is_ascii_alphanumeric(value[0])
        or not _is_ascii_alphanumeric(value[-1])
    ):
        return False
    for character in value:
        if (
            _is_ascii_alphanumeric(character)
            or character == "_"
            or character == "-"
        ):
            continue
        return False
    return True


def _text_component_is_secret_like(value: str) -> bool:
    for variant in _decoded_key_variants(value):
        normalized = normalize_sensitive_key(variant)
        if not normalized:
            continue
        if (
            variant.lower() in COMMON_SENSITIVE_KEYS
            or normalized in COMMON_SENSITIVE_KEY_NORMALIZED
            or any(
                fragment in normalized
                and not _is_payload_free_sensitive_reference(normalized)
                for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
            )
            or "@" in variant
        ):
            return True
    return False


def _url_query_has_secret_like_name(query: str) -> bool:
    if not query:
        return False
    for part in query.split("&"):
        key, _separator, _value = part.partition("=")
        if _text_component_is_secret_like(key):
            return True
    return False


def _has_sensitive_assignment_key(value: str) -> bool:
    """Return whether a scalar line starts with a sensitive assignment key."""

    for separator in ("=", ":"):
        key, found, _raw_assignment_value = value.partition(separator)
        if found and _text_component_is_secret_like(key.strip().lstrip("-")):
            return True
    return False


def _multiline_value_has_folded_secret(lines: tuple[str, ...]) -> bool:
    """Return whether multiline copied text folds a secret header value."""

    for index, line in enumerate(lines[:-1]):
        if not _has_sensitive_assignment_key(line):
            continue
        for next_line in lines[index + 1 :]:
            if next_line:
                return True
    return False


def _scalar_value_is_secret_like(value: str) -> bool:
    stripped = value.strip()
    if not stripped:
        return False
    lines = tuple(line.strip() for line in stripped.splitlines() if line.strip())
    if len(lines) > 1 and _multiline_value_has_folded_secret(lines):
        return True
    if len(lines) > 1 and any(_scalar_value_is_secret_like(line) for line in lines):
        return True
    for separator in ("=", ":"):
        key, found, raw_assignment_value = stripped.partition(separator)
        if found and _text_component_is_secret_like(key.strip().lstrip("-")):
            if raw_assignment_value.strip():
                return True
    lowered = stripped.lower()
    for prefix in SECRET_VALUE_AUTH_PREFIXES:
        if lowered.startswith(prefix):
            token = stripped[len(prefix) :].strip()
            if len(token) >= 8:
                return True
    for prefix in SECRET_VALUE_COOKIE_PREFIXES:
        if lowered.startswith(prefix) and "=" in stripped[len(prefix) :]:
            return True
    if any(marker in lowered for marker in SECRET_VALUE_PEM_MARKERS):
        return True
    return JWT_LIKE_VALUE_RE.search(stripped) is not None


def _url_query_has_secret_like_value(query: str) -> bool:
    if not query:
        return False
    for part in query.split("&"):
        _key, separator, value = part.partition("=")
        if not separator or not value:
            continue
        for variant in _decoded_key_variants(value):
            if (
                _scalar_value_is_secret_like(variant)
                or _text_component_is_secret_like(variant)
            ):
                return True
    return False


def _url_value_is_secret_like(value: str) -> bool:
    if URL_SCHEME_RE.match(value) is None:
        return False
    try:
        parsed = urlsplit(value)
    except ValueError:
        return False
    if not parsed.scheme or not (parsed.netloc or parsed.path or parsed.query):
        return False
    if (
        parsed.username is not None
        or parsed.password is not None
        or "@" in parsed.netloc
    ):
        return True
    if (
        _url_query_has_secret_like_name(parsed.query)
        or _url_query_has_secret_like_value(parsed.query)
    ):
        return True
    host = parsed.hostname or ""
    for component in host.split("."):
        if component and _text_component_is_secret_like(component):
            return True
    for component in parsed.path.split("/"):
        if component and _text_component_is_secret_like(component):
            return True
    return False


def _is_secret_like_value(value: str) -> bool:
    stripped = value.strip()
    if not stripped:
        return False
    return any(
        _url_value_is_secret_like(variant) or _scalar_value_is_secret_like(variant)
        for variant in _value_scan_variants(stripped)
    )


def _is_redacted_diagnostic_path(path: str) -> bool:
    return any(
        segment in path
        for segment in (
            "<sensitive-key>",
            "<sensitive-inclusion-marker>",
        )
    )


def _diagnostic_path_segment(key: Any) -> str:
    if isinstance(key, str):
        key_variants = _decoded_key_variants(key)
        return (
            key
            if len(key_variants) == 1
            and all(
                _is_canonical_path_segment(variant)
                for variant in key_variants
            )
            else "<non-canonical-key>"
        )
    return "<non-string-key>"


def _is_canonical_diagnostic_path_component(component: str) -> bool:
    if not component:
        return False
    head, separator, rest = component.partition("[")
    if (
        not _is_canonical_path_segment(head)
        or len(_decoded_key_variants(head)) != 1
    ):
        return False
    while separator:
        index, close, tail = rest.partition("]")
        if (
            not close
            or not index.isascii()
            or not index.isdecimal()
            or (len(index) > 1 and index.startswith("0"))
        ):
            return False
        if not tail:
            return True
        if not tail.startswith("["):
            return False
        separator = "["
        rest = tail[1:]
    return True


def _is_canonical_diagnostic_path(value: str) -> bool:
    if value == "":
        return True
    return all(
        _is_canonical_diagnostic_path_component(component)
        for component in value.split(".")
    )


def _is_canonical_evidence_label(value: str) -> bool:
    if (
        len(_decoded_key_variants(value)) != 1
        or "  " in value
        or not _is_ascii_alphanumeric(value[0])
        or not _is_ascii_alphanumeric(value[-1])
    ):
        return False
    normalized_value = normalize_sensitive_key(value)
    if any(
        fragment in normalized_value
        for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
    ):
        return False
    for character in value:
        if (
            _is_ascii_alphanumeric(character)
            or character in {" ", "_", "-"}
        ):
            continue
        return False
    return True


def _join_diagnostic_path(path: str, segment: str) -> str:
    return f"{path}.{segment}" if path else segment


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
    if isinstance(value, Mapping):
        for key, child in value.items():
            key_segment = _diagnostic_path_segment(key)
            child_path = _join_diagnostic_path(path, key_segment)
            if not isinstance(key, str):
                errors.append(f"{child_path} key must be a string")
                _visit_sensitive_fields(
                    child,
                    child_path,
                    errors,
                    exact_keys=exact_keys,
                    normalized_keys=normalized_keys,
                    evidence_label=evidence_label,
                    depth=depth + 1,
                )
                continue
            key_variants = _decoded_key_variants(key)
            key_lower_variants = tuple(variant.lower() for variant in key_variants)
            normalized_key_variants = tuple(
                normalize_sensitive_key(variant) for variant in key_variants
            )
            visit_path = child_path
            if _is_sensitive_key(
                key_lower_variants,
                normalized_key_variants,
                exact_keys=exact_keys,
                normalized_keys=normalized_keys,
            ):
                visit_path = _join_diagnostic_path(path, "<sensitive-key>")
                errors.append(f"{visit_path} must not be present in {evidence_label}")
            if any(
                _is_inclusion_marker(normalized_key)
                for normalized_key in normalized_key_variants
            ) and not _is_allowed_inclusion_marker_value(child):
                marker_path = child_path
                if any(
                    _is_sensitive_inclusion_marker(
                        normalized_key,
                        normalized_keys=normalized_keys,
                    )
                    for normalized_key in normalized_key_variants
                ):
                    marker_path = _join_diagnostic_path(
                        path,
                        "<sensitive-inclusion-marker>",
                    )
                errors.append(f"{marker_path} must be false")
            _visit_sensitive_fields(
                child,
                visit_path,
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
    elif (
        isinstance(value, str)
        and not _is_redacted_diagnostic_path(path)
        and _is_secret_like_value(value)
    ):
        errors.append(
            f"{path or '<root>'} must not contain secret-looking values in "
            f"{evidence_label}"
        )


def visit_sensitive_fields(
    value: Any,
    path: str,
    errors: list[str],
    *,
    sensitive_keys: Any,
    evidence_label: str = "rollout evidence",
) -> None:
    """Reject sensitive fields and non-false payload-inclusion markers."""

    error_list = _require_error_list(errors)
    root_path = _require_diagnostic_string(
        path,
        error_list,
        label="sensitive field path",
        allow_empty=True,
    )
    if root_path is not None and not _is_canonical_diagnostic_path(root_path):
        error_list.append("sensitive field path must be a non-empty canonical string")
        root_path = None
    evidence_label_text = _require_diagnostic_string(
        evidence_label,
        error_list,
        label="sensitive field evidence label",
    )
    if evidence_label_text is not None and not _is_canonical_evidence_label(
        evidence_label_text
    ):
        error_list.append(
            "sensitive field evidence label must be a non-empty canonical string"
        )
        evidence_label_text = None
    if root_path is None or evidence_label_text is None:
        return
    key_forms = _key_forms(sensitive_keys, error_list)
    if key_forms is None:
        return
    exact_keys, normalized_keys = key_forms
    _visit_sensitive_fields(
        value,
        root_path,
        error_list,
        exact_keys=exact_keys,
        normalized_keys=normalized_keys,
        evidence_label=evidence_label_text,
    )
