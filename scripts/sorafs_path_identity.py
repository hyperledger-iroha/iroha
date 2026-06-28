"""Shared path identity helpers for SoraFS operator scripts."""

from __future__ import annotations

from pathlib import Path
from string import Formatter
from typing import Any

ALLOWED_FAILURE_TEMPLATE_FIELDS = frozenset({"label", "path", "error"})


class _PathIdentityFailureTemplateError(ValueError):
    """Internal marker for intentional failure-template validation errors."""


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("path identity errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("path identity errors must be a list of strings")
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "path identity errors must contain non-empty canonical strings"
            )
    return errors


def _require_label(label: Any) -> str:
    if (
        not isinstance(label, str)
        or not label.strip()
        or label != label.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in label)
    ):
        raise ValueError("path identity label must be a non-empty canonical string")
    return label


def _require_failure_template(failure_template: Any) -> str:
    if (
        not isinstance(failure_template, str)
        or not failure_template.strip()
        or failure_template != failure_template.strip()
        or any(
            ord(character) < 32 or ord(character) == 127
            for character in failure_template
        )
    ):
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


def _canonical_diagnostic_text(value: str) -> bool:
    return (
        bool(value.strip())
        and value == value.strip()
        and not any(ord(character) < 32 or ord(character) == 127 for character in value)
    )


def path_diagnostic_label(path: Any) -> str:
    """Return a sanitized path label for operator diagnostics."""

    if isinstance(path, Path):
        path_text = str(path)
    elif isinstance(path, str):
        path_text = path
    else:
        return "<non-path>"
    if _canonical_diagnostic_text(path_text):
        return path_text
    return "<non-canonical-path>"


def error_diagnostic_label(
    error: BaseException,
    *,
    path_label: str | None = None,
) -> str:
    """Return a sanitized exception label for operator diagnostics."""

    if path_label == "<non-canonical-path>":
        return "<non-canonical-error>"
    error_text = str(error)
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
