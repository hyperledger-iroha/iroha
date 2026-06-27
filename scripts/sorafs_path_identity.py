"""Shared path identity helpers for SoraFS operator scripts."""

from __future__ import annotations

from pathlib import Path
from string import Formatter
from typing import Any

ALLOWED_FAILURE_TEMPLATE_FIELDS = frozenset({"label", "path", "error"})


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("path identity errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("path identity errors must be a list of strings")
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
    if not isinstance(failure_template, str) or not failure_template.strip():
        raise ValueError("path identity failure template must be a non-empty string")
    fields: set[str] = set()
    try:
        parsed = Formatter().parse(failure_template)
        for _, field_name, format_spec, conversion in parsed:
            if field_name is None:
                continue
            if field_name not in ALLOWED_FAILURE_TEMPLATE_FIELDS:
                raise ValueError(
                    "path identity failure template fields must be label, path, or error"
                )
            if format_spec or conversion:
                raise ValueError(
                    "path identity failure template fields must not use format specifiers"
                )
            fields.add(field_name)
    except ValueError as error:
        if str(error).startswith("path identity failure template"):
            raise
        raise ValueError(
            "path identity failure template must be valid format text"
        ) from error
    if not {"path", "error"}.issubset(fields):
        raise ValueError(
            "path identity failure template must include {path} and {error}"
        )
    return failure_template


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
        error_list.append(f"{identity_label} `{path}` must be a path")
        return None
    try:
        return path.resolve()
    except (OSError, RuntimeError) as error:
        error_list.append(
            identity_failure_template.format(
                label=identity_label,
                path=path,
                error=error,
            )
        )
        return None
