"""Shared path identity helpers for SoraFS operator scripts."""

from __future__ import annotations

import unicodedata
from html import unescape
from pathlib import Path
from string import Formatter
from typing import Any
from urllib.parse import unquote

from sorafs_evidence_sensitivity import (
    COMMON_SENSITIVE_KEYS,
    COMMON_SENSITIVE_KEY_NORMALIZED,
    HIGH_RISK_SENSITIVE_KEY_FRAGMENTS,
    MAX_SENSITIVE_KEY_DECODE_PASSES,
    normalize_sensitive_key,
)

ALLOWED_FAILURE_TEMPLATE_FIELDS = frozenset({"label", "path", "error"})


class _PathIdentityFailureTemplateError(ValueError):
    """Internal marker for intentional failure-template validation errors."""


def _canonical_diagnostic_text(value: str) -> bool:
    return (
        bool(value.strip())
        and value == value.strip()
        and not any(
            ord(character) < 32
            or ord(character) == 127
            or unicodedata.category(character).startswith("C")
            for character in value
        )
    )


def diagnostic_text_is_canonical(value: Any) -> bool:
    """Return whether text is safe to render in operator diagnostics."""

    return isinstance(value, str) and _canonical_diagnostic_text(value)


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("path identity errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("path identity errors must be a list of strings")
        if not diagnostic_text_is_canonical(error):
            raise ValueError(
                "path identity errors must contain non-empty canonical strings"
            )
    return errors


def _require_label(label: Any) -> str:
    if not diagnostic_text_is_canonical(label):
        raise ValueError("path identity label must be a non-empty canonical string")
    return label


def _require_failure_template(failure_template: Any) -> str:
    if not diagnostic_text_is_canonical(failure_template):
        raise _PathIdentityFailureTemplateError(
            "path identity failure template must be a non-empty string"
        )
    fields: set[str] = set()
    try:
        parsed = Formatter().parse(failure_template)
        for _, field_name, format_spec, conversion in parsed:
            if field_name is None:
                continue
            if field_name not in ALLOWED_FAILURE_TEMPLATE_FIELDS:
                raise _PathIdentityFailureTemplateError(
                    "path identity failure template fields must be label, path, or error"
                )
            if format_spec or conversion:
                raise _PathIdentityFailureTemplateError(
                    "path identity failure template fields must not use format specifiers"
                )
            fields.add(field_name)
    except _PathIdentityFailureTemplateError:
        raise
    except ValueError as error:
        raise ValueError(
            "path identity failure template must be valid format text"
        ) from error
    if not {"path", "error"}.issubset(fields):
        raise _PathIdentityFailureTemplateError(
            "path identity failure template must include {path} and {error}"
        )
    return failure_template


def _decoded_label_variants(value: str) -> tuple[str, ...]:
    variants = [value]
    seen = {value}
    current = value
    for _ in range(MAX_SENSITIVE_KEY_DECODE_PASSES):
        decoded = unescape(unquote(current))
        if decoded == current or decoded in seen:
            break
        variants.append(decoded)
        seen.add(decoded)
        current = decoded
    return tuple(variants)


def _label_scan_variants(value: str) -> tuple[str, ...]:
    variants: list[str] = []
    seen: set[str] = set()
    for decoded in _decoded_label_variants(value):
        without_format_controls = "".join(
            character
            for character in decoded
            if unicodedata.category(character) != "Cf"
        )
        for candidate in (
            decoded,
            unicodedata.normalize("NFKC", decoded),
            without_format_controls,
            unicodedata.normalize("NFKC", without_format_controls),
        ):
            if candidate not in seen:
                variants.append(candidate)
                seen.add(candidate)
    return tuple(variants)


def _path_component_is_secret_looking(component: str) -> bool:
    for variant in _label_scan_variants(component):
        stem = variant.rsplit(".", 1)[0]
        normalized_values = {
            normalize_sensitive_key(variant),
            normalize_sensitive_key(stem),
        }
        if any(value in COMMON_SENSITIVE_KEY_NORMALIZED for value in normalized_values):
            return True
        if (
            variant.lower() in COMMON_SENSITIVE_KEYS
            or stem.lower() in COMMON_SENSITIVE_KEYS
        ):
            return True
        if any(
            fragment in value
            for value in normalized_values
            for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
        ):
            return True
    return False


def _path_text_is_secret_looking(path_text: str) -> bool:
    return any(
        _path_component_is_secret_looking(component)
        for component in Path(path_text).parts
    )


def path_diagnostic_label(path: Any) -> str:
    """Return a sanitized path label for operator diagnostics."""

    if isinstance(path, Path):
        path_text = str(path)
    elif isinstance(path, str):
        path_text = path
    else:
        return "<non-path>"
    if _path_text_is_secret_looking(path_text):
        return "<secret-looking-path>"
    if _canonical_diagnostic_text(path_text):
        return path_text
    return "<non-canonical-path>"


def error_diagnostic_label(
    error: BaseException,
    *,
    path_label: str | None = None,
) -> str:
    """Return a sanitized exception label for operator diagnostics."""

    if path_label in {"<non-canonical-path>", "<secret-looking-path>"}:
        return "<non-canonical-error>"
    error_text = str(error)
    if _path_text_is_secret_looking(error_text):
        return "<non-canonical-error>"
    if _canonical_diagnostic_text(error_text):
        return error_text
    return "<non-canonical-error>"


def _path_label(path: Any) -> str:
    return path_diagnostic_label(path)


def _error_label(error: BaseException, *, path_label: str | None = None) -> str:
    return error_diagnostic_label(error, path_label=path_label)


def resolve_path_identity(
    path: Path,
    errors: list[str],
    *,
    label: str = "path",
    failure_template: str = "{label} `{path}` cannot be resolved: {error}",
) -> Path | None:
    """Return the canonical filesystem identity for a path, recording failures."""

    error_list = _require_error_list(errors)
    identity_label = _require_label(label)
    identity_failure_template = _require_failure_template(failure_template)
    if not isinstance(path, Path):
        error_list.append(f"{identity_label} `{_path_label(path)}` must be a path")
        return None
    try:
        return path.resolve()
    except (OSError, RuntimeError) as error:
        path_label = _path_label(path)
        error_list.append(
            identity_failure_template.format(
                label=identity_label,
                path=path_label,
                error=_error_label(error, path_label=path_label),
            )
        )
        return None
