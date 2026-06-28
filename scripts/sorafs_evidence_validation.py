"""Shared validation helpers for SoraFS evidence gates."""

from __future__ import annotations

import math
import re
from collections.abc import Callable, Collection, Hashable, Mapping, Sequence
from pathlib import Path
from string import Formatter
from typing import Any, TypeVar

from sorafs_checker_preflight import record_artifact_error
from sorafs_evidence_fingerprint import artifact_fingerprint
from sorafs_evidence_paths import is_explicit_evidence_path
from sorafs_evidence_sensitivity import visit_sensitive_fields


_T = TypeVar("_T")

SHA256_HEX_PATTERN = re.compile(r"^[0-9a-f]{64}$")
UNKNOWN_REQUIRED_EVIDENCE_KIND = "<unknown>"


def _require_error_list(errors: Any) -> list[str]:
    """Return a mutable summary error list or reject malformed sinks."""

    if not isinstance(errors, list):
        raise ValueError("evidence validation summary errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError(
                "evidence validation summary errors must be a list of strings"
            )
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "evidence validation summary errors must contain non-empty canonical strings"
            )
    return errors


def _evidence_validation_path_label(path: Any, errors: list[str]) -> str | None:
    """Return a canonical validation path label or record a closed failure."""

    if not isinstance(path, Path):
        errors.append("evidence validation path must be a path")
        return None
    label = str(path)
    if (
        not label
        or label != label.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in label)
    ):
        errors.append("evidence validation path must be a canonical path")
        return None
    return label


def _validation_error_messages(
    validation_errors: Any,
) -> tuple[str, ...] | None:
    """Return canonical validation messages or reject malformed containers."""

    if isinstance(validation_errors, (str, bytes, bytearray)) or not isinstance(
        validation_errors, Sequence
    ):
        return None
    messages = tuple(validation_errors)
    if not all(isinstance(error, str) for error in messages):
        return None
    if any(
        not error
        or error != error.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in error)
        for error in messages
    ):
        return ()
    return messages


def _require_validation_label(
    label: Any,
    errors: list[str],
    *,
    label_name: str,
) -> str | None:
    """Return a canonical validation label or record a closed failure."""

    if (
        not isinstance(label, str)
        or not label.strip()
        or label != label.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in label)
    ):
        errors.append(f"{label_name} must be a non-empty canonical string")
        return None
    return label


def _validate_kind_error_template(
    template: Any,
    errors: list[str],
    *,
    template_name: str,
) -> str | None:
    """Return a canonical kind-name formatter template or record a closed failure."""

    template_label = _require_validation_label(
        template,
        errors,
        label_name=template_name,
    )
    if template_label is None:
        return None
    try:
        fields = tuple(Formatter().parse(template_label))
    except ValueError:
        errors.append(f"{template_name} must be a valid formatter template")
        return None
    for _literal, field_name, format_spec, conversion in fields:
        if field_name is None:
            continue
        if field_name != "kind_name" or format_spec or conversion:
            errors.append(
                f"{template_name} must use only plain kind_name formatter fields"
            )
            return None
    return template_label


def _format_kind_error_template(
    template: str,
    kind_name: Any,
    errors: list[str],
    *,
    template_name: str,
) -> str | None:
    """Render a validated kind-name formatter template."""

    kind_label = _require_validation_label(
        kind_name,
        errors,
        label_name="validation evidence kind",
    )
    if kind_label is None:
        return None
    try:
        return template.format(kind_name=kind_label)
    except (KeyError, IndexError, ValueError):
        errors.append(f"{template_name} must be a valid formatter template")
        return None


def require_object(value: Any, path: str, errors: list[str]) -> dict[str, Any]:
    """Return an object value or append a path-qualified validation error."""

    if isinstance(value, dict):
        return value
    diagnostic_label = _require_validation_label(
        path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return {}
    errors.append(f"{diagnostic_label} must be an object")
    return {}


def require_object_array(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
) -> list[tuple[int, dict[str, Any]]]:
    """Return indexed object records from a required non-empty array field."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return []
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return []
    items = payload.get(field_name)
    if not isinstance(items, list) or not items:
        errors.append(f"{field_name} must be a non-empty array")
        return []
    records: list[tuple[int, dict[str, Any]]] = []
    has_malformed_item = False
    for index, item in enumerate(items):
        record = require_object(item, f"{field_name}[{index}]", errors)
        if not isinstance(item, dict):
            has_malformed_item = True
            continue
        records.append((index, record))
    if has_malformed_item:
        return []
    return records


def require_string(payload: Mapping[str, Any], field: str, errors: list[str]) -> str:
    """Return a canonical non-empty string field or append a validation error."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return ""
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return ""
    value = payload.get(field_name)
    value_label = _require_validation_label(
        value,
        errors,
        label_name=field_name,
    )
    if value_label is not None:
        return value_label
    return ""


def require_string_type(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str | None = None,
) -> str | None:
    """Return a canonical string field or append a validation error."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return None
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return None
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return None
    value = payload.get(field_name)
    if not isinstance(value, str):
        errors.append(f"{diagnostic_label} must be a string")
        return None
    value_label = _require_validation_label(
        value,
        errors,
        label_name=diagnostic_label,
    )
    if value_label is None:
        return None
    return value_label


def require_known_schema(
    payload: Any,
    schema_to_kind: Any,
    artifact_label: str,
    errors: list[str],
) -> _T | None:
    """Return the schema kind for a string schema or append validation errors."""

    schema = require_string_type(payload, "schema", errors)
    if schema is None:
        return None
    if not isinstance(schema_to_kind, Mapping):
        errors.append(f"{artifact_label} schema registry must be a mapping")
        return None
    kind = schema_to_kind.get(schema)
    if kind is None:
        errors.append(f"schema `{schema}` is not a recognized {artifact_label}")
        return None
    return kind


def validate_standard_evidence_payload(
    payload: Any,
    schema_to_kind: Any,
    artifact_label: str,
    sensitive_keys: Collection[str],
    evidence_label: str,
    validate_kind: Callable[[_T, dict[str, Any], list[str]], None],
    *,
    require_reviewed_deployment_context: bool = False,
) -> tuple[str | None, list[str]]:
    """Validate the standard rollout/release evidence payload wrapper."""

    errors: list[str] = []
    if not isinstance(require_reviewed_deployment_context, bool):
        errors.append(
            "validation require_reviewed_deployment_context must be a boolean"
        )
        return None, errors
    kind = require_known_schema(payload, schema_to_kind, artifact_label, errors)
    if kind is None:
        return None, errors
    if not isinstance(payload, dict):
        errors.append("payload must be an object")
        return None, errors
    kind_name = getattr(kind, "name", None)
    if not isinstance(kind_name, str) or not kind_name:
        errors.append(f"{artifact_label} schema kind must have a non-empty name")
        return None, errors
    if require_reviewed_deployment_context:
        require_rollout_deployment_id(payload, errors)
        require_rollout_environment(payload, errors)
        require_rollout_deployment_context_review(payload, errors)
    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys=sensitive_keys,
        evidence_label=evidence_label,
    )
    validate_kind(kind, payload, errors)
    return kind_name, errors


def _required_evidence_kind_list(required_kinds: Any) -> list[str] | None:
    """Return required evidence kind names or reject malformed containers."""

    if isinstance(
        required_kinds, (str, bytes, bytearray, Mapping)
    ) or not isinstance(required_kinds, Sequence):
        return None
    names: list[str] = []
    for name in required_kinds:
        if (
            _require_validation_label(
                name,
                [],
                label_name="required evidence kind",
            )
            is None
        ):
            return None
        names.append(name)
    return names


def _duplicate_required_evidence_kind_names(names: Sequence[str]) -> list[str]:
    """Return duplicate required evidence kind names in stable sorted order."""

    return sorted({name for name in names if names.count(name) > 1})


def _evidence_artifact_rows(artifacts: Any) -> list[Mapping[str, Any]] | None:
    """Return artifact rows, rejecting malformed containers and rows."""

    if isinstance(artifacts, (str, bytes, bytearray, Mapping)) or not isinstance(
        artifacts, Sequence
    ):
        return None
    rows: list[Mapping[str, Any]] = []
    for artifact in artifacts:
        if not isinstance(artifact, Mapping):
            return None
        rows.append(artifact)
    return rows


def _evidence_kind_artifact_pairs(
    artifacts: Any,
    errors: list[str],
    *,
    label: str,
) -> list[tuple[str, dict[str, Any]]] | None:
    """Return ``(kind, artifact)`` rows, rejecting malformed containers."""

    error = f"{label} must be a sequence of (kind, artifact) pairs"
    if isinstance(artifacts, (str, bytes, bytearray, Mapping)) or not isinstance(
        artifacts, Sequence
    ):
        errors.append(error)
        return None
    pairs: list[tuple[str, dict[str, Any]]] = []
    for item in artifacts:
        if isinstance(item, (str, bytes, bytearray, Mapping)) or not isinstance(
            item, Sequence
        ):
            errors.append(error)
            return None
        if len(item) != 2:
            errors.append(error)
            return None
        kind_name, artifact = item
        if not isinstance(kind_name, str) or not kind_name or not isinstance(
            artifact, dict
        ):
            errors.append(error)
            return None
        kind_label = _require_validation_label(
            kind_name,
            errors,
            label_name="validation evidence kind",
        )
        if kind_label is None:
            return None
        pairs.append((kind_label, artifact))
    return pairs


def build_required_evidence_summary(
    required_kinds: Any,
    artifacts_by_kind: Mapping[str, Sequence[dict[str, Any]]],
    schema_by_kind: Mapping[str, str],
    errors: list[str],
    *,
    evidence_label: str,
) -> dict[str, dict[str, Any]]:
    """Build required-kind summary rows and append missing/invalid errors."""

    required: dict[str, dict[str, Any]] = {}
    required_kind_names = _required_evidence_kind_list(required_kinds)
    if required_kind_names is None:
        errors.append(
            f"{evidence_label} required evidence kinds must be a sequence of strings"
        )
        return required
    if not required_kind_names:
        errors.append(f"{evidence_label} required evidence kinds must not be empty")
        return required
    duplicate_kind_names = _duplicate_required_evidence_kind_names(
        required_kind_names
    )
    if duplicate_kind_names:
        errors.append(
            f"{evidence_label} required evidence kinds must not contain duplicates "
            f"{duplicate_kind_names}"
        )
        return required
    if isinstance(artifacts_by_kind, Mapping):
        artifact_buckets = artifacts_by_kind
    else:
        errors.append(f"{evidence_label} artifacts by kind must be a mapping")
        artifact_buckets = {}
    if isinstance(schema_by_kind, Mapping):
        schema_map = schema_by_kind
    else:
        errors.append(f"{evidence_label} schema by kind must be a mapping")
        schema_map = {}
    for name in required_kind_names:
        raw_artifacts = artifact_buckets.get(name, [])
        row_errors: list[str] = []
        malformed_artifact_bucket = name in artifact_buckets and (
            isinstance(raw_artifacts, (str, bytes, bytearray, Mapping))
            or not isinstance(raw_artifacts, Sequence)
        )
        if malformed_artifact_bucket:
            row_errors.append(f"required `{name}` artifacts must be a sequence")
            errors.append(
                f"required {name} {evidence_label} artifacts must be a sequence"
            )
            artifacts = []
        else:
            artifacts = _evidence_artifact_rows(raw_artifacts)
            if artifacts is None:
                malformed_artifact_bucket = True
                row_errors.append(
                    f"required `{name}` artifacts must be a sequence of artifact objects"
                )
                errors.append(
                    f"required {name} {evidence_label} artifacts must be a sequence "
                    "of artifact objects"
                )
                artifacts = []
        schema = schema_map.get(name)
        schema_errors: list[str] = []
        schema_label = _require_validation_label(
            schema,
            schema_errors,
            label_name=f"required `{name}` schema",
        )
        if schema_label is None:
            row_errors.append(f"required `{name}` schema must be configured")
            errors.append(f"required {name} {evidence_label} schema must be configured")
            schema = None
        else:
            schema = schema_label
        present = bool(artifacts)
        artifacts_valid = present and all(
            evidence_artifact_is_valid(artifact) for artifact in artifacts
        )
        valid = not row_errors and artifacts_valid
        required[name] = {
            "schema": schema,
            "present": present,
            "valid": valid,
            "artifact_count": len(artifacts),
            "artifacts": artifacts,
            "errors": row_errors,
        }
        if not present and not malformed_artifact_bucket:
            errors.append(f"missing required {name} {evidence_label} evidence")
        elif not artifacts_valid and not malformed_artifact_bucket:
            errors.append(f"{name} {evidence_label} evidence has invalid artifact(s)")

    deployment_error_count = len(errors)
    deployment_context: dict[str, str] = {}
    for kind_name, artifacts in artifact_buckets.items():
        artifact_rows = _evidence_artifact_rows(artifacts)
        if artifact_rows is None:
            continue
        for artifact in artifact_rows:
            if not evidence_artifact_is_valid(artifact):
                continue
            fingerprint = evidence_artifact_fingerprint(artifact)
            if "deployment_id" not in fingerprint and "environment" not in fingerprint:
                continue
            record_consistent_deployment_context(
                deployment_context,
                artifact,
                kind_name,
                errors,
            )
    if len(errors) > deployment_error_count:
        mark_required_evidence_summary_invalid(
            required,
            f"{evidence_label} evidence deployment context must match across artifacts",
        )
    return required


def mark_required_evidence_invalid(
    required: dict[str, dict[str, Any]],
    kind_name: Any,
) -> list[str]:
    """Mark a required evidence summary row invalid and return its errors list."""

    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="validation required evidence kind",
    )
    malformed_kind = kind_label is None
    if kind_label is None:
        kind_label = UNKNOWN_REQUIRED_EVIDENCE_KIND
    if malformed_kind and isinstance(kind_name, Hashable) and kind_name in required:
        row = required.pop(kind_name)
        if kind_label not in required:
            required[kind_label] = row
    elif kind_label not in required:
        required[kind_label] = {"valid": False, "errors": [], "artifacts": []}
    row = required[kind_label]
    if not isinstance(row, dict):
        row = {
            "valid": False,
            "errors": [f"required `{kind_label}` row must be an object"],
            "artifacts": [],
        }
        required[kind_label] = row

    row["valid"] = False
    errors = row.get("errors")
    if not isinstance(errors, list):
        errors = [f"required `{kind_label}` errors must be a list"]
        row["errors"] = errors
    for error in label_errors:
        if error not in errors:
            errors.append(error)
    return errors


def mark_required_evidence_summary_invalid(
    required: dict[str, dict[str, Any]],
    error: str | None = None,
) -> None:
    """Mark every required evidence summary row invalid."""

    summary_error = None
    if error is not None:
        label_errors: list[str] = []
        summary_error = _require_validation_label(
            error,
            label_errors,
            label_name="validation required summary error",
        )
        if summary_error is None:
            summary_error = label_errors[0]
    for kind_name in list(required):
        errors = mark_required_evidence_invalid(required, kind_name)
        if summary_error is not None and summary_error not in errors:
            errors.append(summary_error)


def mark_required_evidence_invalid_if_present(
    required: dict[str, dict[str, Any]],
    kind_name: Any | None,
) -> list[str]:
    """Mark an existing required evidence summary row invalid."""

    if kind_name is None:
        return []
    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="validation required evidence kind",
    )
    if kind_label is None or kind_label not in required:
        return []
    return mark_required_evidence_invalid(required, kind_label)


def required_evidence_summary_is_valid(required: Any) -> bool:
    """Return whether every required evidence summary row is explicitly valid."""

    if not isinstance(required, Mapping):
        return False
    for row in required.values():
        if not isinstance(row, Mapping) or row.get("valid") is not True:
            return False
        row_errors = row.get("errors")
        if row_errors != []:
            return False
        artifacts = _evidence_artifact_rows(row.get("artifacts"))
        if artifacts is None:
            return False
        if not artifacts:
            return False
        if not recognized_evidence_artifacts_are_valid(artifacts):
            return False
    return True


def _evidence_kind_name_set(kinds: Any) -> set[str] | None:
    """Return normalized evidence kind names or reject malformed containers."""

    if isinstance(kinds, (str, bytes, bytearray, Mapping)) or not isinstance(
        kinds, Collection
    ):
        return None
    names: set[str] = set()
    for kind in kinds:
        if (
            _require_validation_label(
                kind,
                [],
                label_name="validation evidence kind",
            )
            is None
        ):
            return None
        names.add(kind)
    return names


def required_evidence_has_any_kind(
    required_kinds: Any,
    candidate_kinds: Any,
) -> bool:
    """Return whether required evidence includes any candidate kind."""

    required_kind_names = _evidence_kind_name_set(required_kinds)
    candidate_kind_names = _evidence_kind_name_set(candidate_kinds)
    if required_kind_names is None or candidate_kind_names is None:
        return False
    return any(kind in required_kind_names for kind in candidate_kind_names)


def required_evidence_has_all_kinds(
    required_kinds: Any,
    candidate_kinds: Any,
) -> bool:
    """Return whether required evidence includes every candidate kind."""

    required_kind_names = _evidence_kind_name_set(required_kinds)
    candidate_kind_names = _evidence_kind_name_set(candidate_kinds)
    if required_kind_names is None or candidate_kind_names is None:
        return False
    return all(kind in required_kind_names for kind in candidate_kind_names)


def _evidence_kind_name_set_with_errors(
    kinds: Any,
    errors: list[str],
    *,
    container_label: str,
    item_label: str,
) -> set[str] | None:
    """Return normalized evidence kind names or record malformed inputs."""

    if isinstance(kinds, (str, bytes, bytearray, Mapping)) or not isinstance(
        kinds, Collection
    ):
        errors.append(f"{container_label} must be a collection of canonical strings")
        return None
    names: set[str] = set()
    has_malformed_kind = False
    for kind in kinds:
        kind_label = _require_validation_label(
            kind,
            errors,
            label_name=item_label,
        )
        if kind_label is None:
            has_malformed_kind = True
            continue
        names.add(kind_label)
    if has_malformed_kind:
        return None
    return names


def _required_evidence_has_any_kind_or_error(
    required_kinds: Any,
    candidate_kinds: Any,
    errors: list[str],
) -> bool | None:
    """Return whether required evidence intersects candidates or record errors."""

    required_kind_names = _evidence_kind_name_set_with_errors(
        required_kinds,
        errors,
        container_label="validation required evidence kinds",
        item_label="validation required evidence kind",
    )
    candidate_kind_names = _evidence_kind_name_set_with_errors(
        candidate_kinds,
        errors,
        container_label="validation missing-anchor required evidence kinds",
        item_label="validation missing-anchor required evidence kind",
    )
    if required_kind_names is None or candidate_kind_names is None:
        return None
    return any(kind in required_kind_names for kind in candidate_kind_names)


def _evidence_value_items(values: Any) -> tuple[Any, ...]:
    """Return evidence value items, rejecting scalar and object containers."""

    if isinstance(values, (str, bytes, bytearray, Mapping)) or not isinstance(
        values, Collection
    ):
        return ()
    return tuple(values)


def hashable_evidence_values(values: Any) -> set[Hashable]:
    """Return truthy hashable evidence values from an observed-value collection."""

    return {
        normalized
        for value in _evidence_value_items(values)
        if (normalized := _hashable_evidence_value(value)) is not None
    }


def _hashable_evidence_value(value: Any) -> Hashable | None:
    """Return a canonical hashable evidence value or reject malformed values."""

    if not value or isinstance(value, bool) or not isinstance(value, Hashable):
        return None
    if (
        isinstance(value, str)
        and _require_validation_label(
            value,
            [],
            label_name="validation evidence value",
        )
        is None
    ):
        return None
    return value


def missing_required_evidence_values(
    required_values: Any,
    observed_values: Any,
) -> list[Any]:
    """Return required evidence values that are absent from observed values."""

    if isinstance(
        required_values, (str, bytes, bytearray, Mapping)
    ) or not isinstance(required_values, Sequence):
        return [required_values]
    observed = hashable_evidence_values(observed_values)
    return [
        value
        for value in required_values
        if not isinstance(value, Hashable) or value not in observed
    ]


def record_missing_required_evidence_value_errors(
    required: dict[str, dict[str, Any]],
    kind_name: str,
    required_values: Any,
    observed_values: Any,
    message_for_value: Callable[[Any], str],
) -> list[Any]:
    """Record row errors for required values absent from observed evidence."""

    missing_values = missing_required_evidence_values(required_values, observed_values)
    if not missing_values:
        return []
    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="validation evidence kind",
    )
    if kind_label is None:
        mark_required_evidence_summary_invalid(required, label_errors[0])
        return missing_values
    errors = mark_required_evidence_invalid(required, kind_label)
    for value in missing_values:
        try:
            message = message_for_value(value)
        except Exception:
            message = None
        message_errors: list[str] = []
        message_label = _require_validation_label(
            message,
            message_errors,
            label_name="validation missing required evidence message",
        )
        errors.append(message_label or message_errors[0])
    return missing_values


def required_or_observed_evidence_values_are_present(
    required_values: Any,
    observed_values: Any,
) -> bool:
    """Return whether evidence values are either required or observed."""

    return _evidence_values_are_clean_and_present(
        required_values
    ) or _evidence_values_are_clean_and_present(observed_values)


def _evidence_values_are_clean_and_present(values: Any) -> bool:
    """Return whether a value collection has only canonical present values."""

    items = _evidence_value_items(values)
    if not items:
        return False
    has_present_value = False
    for value in items:
        if not value:
            continue
        if _hashable_evidence_value(value) is None:
            return False
        has_present_value = True
    return has_present_value


def record_missing_required_or_observed_evidence_error(
    required: dict[str, dict[str, Any]],
    kind_name: str,
    required_values: Any,
    observed_values: Any,
    error: str,
) -> bool:
    """Record a row error when neither required nor observed values exist."""

    if required_or_observed_evidence_values_are_present(
        required_values,
        observed_values,
    ):
        return False
    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="validation evidence kind",
    )
    if kind_label is None:
        mark_required_evidence_summary_invalid(required, label_errors[0])
        return True
    error_label = _require_validation_label(
        error,
        label_errors,
        label_name="validation missing evidence error",
    )
    mark_required_evidence_invalid(required, kind_label).append(
        error_label or label_errors[-1]
    )
    return True


def distinct_evidence_values_are_consistent(values: Any) -> bool:
    """Return whether evidence values contain at most one distinct value."""

    if isinstance(values, (str, bytes, bytearray, Mapping)) or not isinstance(
        values, Collection
    ):
        return False
    normalized_values: list[Hashable] = []
    for value in values:
        normalized = _hashable_evidence_value(value)
        if normalized is None:
            return False
        normalized_values.append(normalized)
    return len(set(normalized_values)) <= 1


def record_inconsistent_evidence_values_error(
    required: dict[str, dict[str, Any]],
    values: Any,
    kind_name: str,
    error: str,
) -> bool:
    """Record a summary-wide error when evidence values disagree."""

    if distinct_evidence_values_are_consistent(values):
        return False
    mark_required_evidence_summary_invalid(required)
    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="validation evidence kind",
    )
    if kind_label is None:
        mark_required_evidence_summary_invalid(required, label_errors[0])
        return True
    error_label = _require_validation_label(
        error,
        label_errors,
        label_name="validation inconsistent evidence error",
    )
    mark_required_evidence_invalid(required, kind_label).append(
        error_label or label_errors[-1]
    )
    return True


def record_consistent_evidence_value(
    values: dict[str, str],
    key: str,
    value: Any,
    context: str,
    errors: list[str],
) -> None:
    """Record a non-empty evidence value and report cross-artifact mismatches."""

    if value == "":
        return
    context_label = _require_validation_label(
        context,
        errors,
        label_name="validation evidence context",
    )
    key_label = _require_validation_label(
        key,
        errors,
        label_name="validation evidence key",
    )
    if context_label is None or key_label is None:
        return
    if not isinstance(value, str):
        errors.append(f"{context_label}.{key_label} must be a string")
        return
    value_label = _require_validation_label(
        value,
        errors,
        label_name="validation evidence value",
    )
    if value_label is None:
        return
    previous = values.get(key_label)
    if previous is None:
        values[key_label] = value_label
    elif previous != value_label:
        error = f"{context_label}.{key_label} `{value_label}` does not match `{previous}`"
        if error not in errors:
            errors.append(error)


def record_consistent_deployment_context(
    values: dict[str, str],
    artifact: Any,
    context: str,
    errors: list[str],
) -> None:
    """Record an artifact deployment context and report bundle mismatches."""

    context_errors: list[str] = []
    context_label = _require_validation_label(
        context,
        context_errors,
        label_name="validation evidence context",
    )
    if context_label is None:
        if isinstance(artifact, dict):
            record_artifact_error(artifact, context_errors[0], errors)
        elif context_errors[0] not in errors:
            errors.append(context_errors[0])
        return
    fingerprint = evidence_artifact_fingerprint(artifact)
    for key in ("deployment_id", "environment"):
        value = fingerprint.get(key)
        if value == "":
            continue
        if not isinstance(value, str):
            error = f"{context_label}.{key} must be a string"
        else:
            value_errors: list[str] = []
            value_label = _require_validation_label(
                value,
                value_errors,
                label_name="validation evidence value",
            )
            if value_label is None:
                error = value_errors[0]
                if isinstance(artifact, dict):
                    record_artifact_error(artifact, error, errors)
                elif error not in errors:
                    errors.append(error)
                continue
            previous = values.get(key)
            if previous is None:
                values[key] = value_label
                continue
            if previous == value_label:
                continue
            error = f"{context_label}.{key} `{value_label}` does not match `{previous}`"
        if isinstance(artifact, dict):
            record_artifact_error(artifact, error, errors)
        elif error not in errors:
            errors.append(error)


def deployment_context_summary(values: Any) -> dict[str, str]:
    """Return the stable deployment-context summary fields."""

    if not isinstance(values, Mapping):
        return {}
    summary: dict[str, str] = {}
    for key in ("deployment_id", "environment"):
        value = values.get(key)
        if value is None:
            continue
        value_label = _require_validation_label(
            value,
            [],
            label_name=f"deployment context {key}",
        )
        if value_label is not None:
            summary[key] = value_label
    return summary


def record_observed_evidence_value(values: set[Any], value: Any) -> None:
    """Record a truthy observed evidence value."""

    values.update(hashable_evidence_values((value,)))


def record_snapshot_bound_evidence_artifact(
    *,
    kind_name: str,
    artifact: dict[str, Any],
    snapshot_id: Any,
    merkle_root: Any,
    valid: bool,
    anchor_kinds: Collection[str],
    bound_kinds: Collection[str],
    valid_snapshot_bindings: set[tuple[str, str]],
    snapshot_bound_artifacts: list[dict[str, Any]],
) -> None:
    """Record valid snapshot anchors or downstream snapshot-bound artifacts."""

    if not isinstance(valid, bool):
        record_artifact_error(artifact, "artifact valid flag must be a boolean", [])
        return
    if not valid:
        return
    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="snapshot binding evidence kind",
    )
    if kind_label is None:
        record_artifact_error(artifact, label_errors[0], [])
        return
    anchor_kind_names = _evidence_kind_name_set(anchor_kinds)
    bound_kind_names = _evidence_kind_name_set(bound_kinds)
    if anchor_kind_names is None or bound_kind_names is None:
        record_artifact_error(
            artifact,
            "snapshot binding kind containers must be sequences of canonical strings",
            [],
        )
        return
    if kind_label in anchor_kind_names:
        anchor_errors: list[str] = []
        snapshot_label = _require_validation_label(
            snapshot_id,
            anchor_errors,
            label_name="snapshot_id_hex",
        )
        merkle_label = _require_validation_label(
            merkle_root,
            anchor_errors,
            label_name="merkle_root_hex",
        )
        if snapshot_label is None or merkle_label is None:
            for error in anchor_errors:
                record_artifact_error(artifact, error, [])
            return
        valid_snapshot_bindings.add((snapshot_label.lower(), merkle_label.lower()))
        return
    if kind_label in bound_kind_names:
        snapshot_bound_artifacts.append(artifact)


def _snapshot_binding_pair_set(bindings: Any) -> set[tuple[str, str]] | None:
    """Return normalized snapshot binding pairs or reject malformed containers."""

    if isinstance(bindings, (str, bytes, bytearray, Mapping)) or not isinstance(
        bindings, Collection
    ):
        return None
    normalized_bindings: set[tuple[str, str]] = set()
    for binding in bindings:
        if isinstance(binding, (str, bytes, bytearray)) or not isinstance(
            binding, Sequence
        ):
            return None
        raw_binding = tuple(binding)
        if len(raw_binding) != 2:
            return None
        label_errors: list[str] = []
        snapshot_label = _require_validation_label(
            raw_binding[0],
            label_errors,
            label_name="snapshot_id_hex",
        )
        merkle_label = _require_validation_label(
            raw_binding[1],
            label_errors,
            label_name="merkle_root_hex",
        )
        if snapshot_label is None or merkle_label is None:
            return None
        normalized_bindings.add((snapshot_label.lower(), merkle_label.lower()))
    return normalized_bindings


def _snapshot_bound_artifact_rows(artifacts: Any) -> tuple[dict[str, Any], ...] | None:
    """Return snapshot-bound artifact rows or reject malformed containers."""

    if isinstance(artifacts, (str, bytes, bytearray, Mapping)) or not isinstance(
        artifacts, Sequence
    ):
        return None
    artifact_rows: list[dict[str, Any]] = []
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            return None
        artifact_rows.append(artifact)
    return tuple(artifact_rows)


def validate_snapshot_bound_evidence_artifacts(
    *,
    required: dict[str, dict[str, Any]],
    required_kinds: Collection[str],
    bound_kinds: Collection[str],
    valid_snapshot_bindings: Collection[tuple[str, str]],
    snapshot_bound_artifacts: Sequence[dict[str, Any]],
    required_anchor_kind: str,
    binding_error: str,
    binding_summary_error: str,
    missing_anchor_error: str,
    missing_anchor_summary_error: str,
    missing_required_anchor_error: str,
) -> None:
    """Validate downstream snapshot-bound artifacts against valid anchors."""

    label_errors: list[str] = []
    required_anchor_label = _require_validation_label(
        required_anchor_kind,
        label_errors,
        label_name="validation required anchor evidence kind",
    )
    binding_error_label = _require_validation_label(
        binding_error,
        label_errors,
        label_name="validation binding error",
    )
    binding_summary_error_label = _require_validation_label(
        binding_summary_error,
        label_errors,
        label_name="validation binding summary error",
    )
    missing_anchor_error_label = _require_validation_label(
        missing_anchor_error,
        label_errors,
        label_name="validation missing anchor error",
    )
    missing_anchor_summary_error_label = _require_validation_label(
        missing_anchor_summary_error,
        label_errors,
        label_name="validation missing anchor summary error",
    )
    missing_required_anchor_error_label = _require_validation_label(
        missing_required_anchor_error,
        label_errors,
        label_name="validation missing required anchor error",
    )
    if (
        required_anchor_label is None
        or binding_error_label is None
        or binding_summary_error_label is None
        or missing_anchor_error_label is None
        or missing_anchor_summary_error_label is None
        or missing_required_anchor_error_label is None
    ):
        mark_required_evidence_summary_invalid(required, label_errors[0])
        return

    required_kind_names = _evidence_kind_name_set(required_kinds)
    bound_kind_names = _evidence_kind_name_set(bound_kinds)
    if required_kind_names is None or bound_kind_names is None:
        mark_required_evidence_summary_invalid(
            required,
            "snapshot binding kind containers must be sequences of canonical strings",
        )
        return

    snapshot_binding_pairs = _snapshot_binding_pair_set(valid_snapshot_bindings)
    if snapshot_binding_pairs is None:
        mark_required_evidence_summary_invalid(
            required,
            "snapshot binding pairs must be a sequence of canonical string pairs",
        )
        return

    snapshot_bound_rows = _snapshot_bound_artifact_rows(snapshot_bound_artifacts)
    if snapshot_bound_rows is None:
        mark_required_evidence_summary_invalid(
            required,
            "snapshot bound artifacts must be a sequence of artifact objects",
        )
        return

    if snapshot_binding_pairs:
        for artifact in snapshot_bound_rows:
            fingerprint = evidence_artifact_fingerprint(artifact)
            snapshot_id = fingerprint.get("snapshot_id_hex")
            merkle_root = fingerprint.get("merkle_root_hex")
            binding_errors: list[str] = []
            require_string_tuple_in(
                (snapshot_id, merkle_root),
                snapshot_binding_pairs,
                binding_errors,
                message=binding_error_label,
            )
            for error in binding_errors:
                summary_errors = mark_required_evidence_invalid_if_present(
                    required,
                    evidence_artifact_kind(artifact),
                )
                record_artifact_error(
                    artifact,
                    error,
                    summary_errors,
                    summary_error=binding_summary_error_label,
                )
    elif snapshot_bound_rows:
        for artifact in snapshot_bound_rows:
            summary_errors = mark_required_evidence_invalid_if_present(
                required,
                evidence_artifact_kind(artifact),
            )
            record_artifact_error(
                artifact,
                missing_anchor_error_label,
                summary_errors,
                summary_error=missing_anchor_summary_error_label,
            )

    if not snapshot_binding_pairs and required_evidence_has_any_kind(
        required_kind_names,
        bound_kind_names,
    ):
        mark_required_evidence_invalid(required, required_anchor_label).append(
            missing_required_anchor_error_label
        )


def finalize_custom_required_evidence_rows(
    required: dict[str, dict[str, Any]],
    *,
    evidence_label: str,
) -> None:
    """Finalize custom required evidence rows with fail-closed validity."""

    label_errors: list[str] = []
    evidence_name = _require_validation_label(
        evidence_label,
        label_errors,
        label_name="validation evidence label",
    )
    if evidence_name is None:
        mark_required_evidence_summary_invalid(required, label_errors[0])
        return

    for kind, row in list(required.items()):
        kind_errors: list[str] = []
        kind_label = _require_validation_label(
            kind,
            kind_errors,
            label_name="validation required evidence kind",
        )
        if kind_label is None:
            mark_required_evidence_invalid(required, kind)
            continue
        if not isinstance(row, dict):
            required[kind_label] = {
                "valid": False,
                "errors": [f"required `{kind_label}` row must be an object"],
                "artifacts": [],
            }
            continue

        row_errors = row.get("errors")
        if not isinstance(row_errors, list):
            row_errors = [f"required `{kind_label}` errors must be a list"]
            row["errors"] = row_errors

        artifacts = row.get("artifacts")
        if not isinstance(artifacts, Sequence) or isinstance(
            artifacts, (str, bytes, bytearray)
        ):
            row["artifacts"] = []
            mark_required_evidence_invalid(required, kind_label).append(
                f"required `{kind_label}` artifacts must be a sequence"
            )
            continue
        artifact_rows = _evidence_artifact_rows(artifacts)
        if artifact_rows is None:
            row["artifacts"] = []
            mark_required_evidence_invalid(required, kind_label).append(
                f"required `{kind_label}` artifacts must be a sequence of artifact objects"
            )
            continue
        row["artifacts"] = list(artifact_rows)
        if not artifact_rows:
            mark_required_evidence_invalid(required, kind_label).append(
                f"missing required `{kind_label}` {evidence_name}"
            )
        else:
            row["valid"] = not row_errors and recognized_evidence_artifacts_are_valid(
                artifact_rows
            )


def record_custom_required_evidence_artifact(
    required: dict[str, dict[str, Any]],
    kind_name: str,
    artifact: dict[str, Any],
    errors: Any,
) -> bool:
    """Record an artifact into a custom required evidence row if present."""

    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="validation required evidence kind",
    )
    if kind_label is None:
        mark_required_evidence_summary_invalid(required, label_errors[0])
        return False
    row = required.get(kind_label)
    if row is None:
        return False
    if not isinstance(row, dict):
        row = {
            "valid": False,
            "errors": [f"required `{kind_label}` row must be an object"],
            "artifacts": [],
        }
        required[kind_label] = row
        return True

    row_errors = row.get("errors")
    if not isinstance(row_errors, list):
        row_errors = [f"required `{kind_label}` errors must be a list"]
        row["errors"] = row_errors

    if not isinstance(artifact, dict):
        row["valid"] = False
        row_errors.append(f"required `{kind_label}` artifact must be an object")
        return True

    artifacts = row.get("artifacts")
    if not isinstance(artifacts, list):
        row["valid"] = False
        row_errors.append(f"required `{kind_label}` artifacts must be a sequence")
        return True
    if not all(
        isinstance(existing_artifact, Mapping) for existing_artifact in artifacts
    ):
        row["valid"] = False
        row_errors.append(
            f"required `{kind_label}` artifacts must be a sequence of artifact objects"
        )
        return True
    artifacts.append(artifact)

    malformed_errors_message = (
        f"required `{kind_label}` artifact errors must be a sequence of canonical strings"
    )
    if isinstance(errors, (str, bytes, bytearray)) or not isinstance(errors, Sequence):
        row_errors.append(malformed_errors_message)
    else:
        artifact_errors = tuple(errors)
        if not all(
            isinstance(error, str)
            and error
            and error == error.strip()
            and not any(
                ord(character) < 32 or ord(character) == 127 for character in error
            )
            for error in artifact_errors
        ):
            row_errors.append(malformed_errors_message)
        else:
            row_errors.extend(artifact_errors)
    return True


def required_evidence_kind_names(required_kinds: Any) -> list[str]:
    """Return the standard non-empty unique required-kind summary names."""

    names = _required_evidence_kind_list(required_kinds)
    if names is None or not names:
        return []
    if _duplicate_required_evidence_kind_names(names):
        return []
    return names


def evidence_schema_by_kind(kind_by_name: Any) -> dict[str, str]:
    """Return the standard evidence schema lookup keyed by kind name."""

    if not isinstance(kind_by_name, Mapping):
        return {}
    schema_lookup: dict[str, str] = {}
    for name, kind in kind_by_name.items():
        schema = getattr(kind, "schema", None)
        if (
            _require_validation_label(
                name,
                [],
                label_name="evidence kind",
            )
            is None
        ):
            return {}
        if (
            _require_validation_label(
                schema,
                [],
                label_name="evidence schema",
            )
            is None
        ):
            return {}
        schema_lookup[name] = schema
    return schema_lookup


def init_evidence_artifact_buckets(
    evidence_kind_names: Any,
) -> dict[str, list[dict[str, Any]]]:
    """Return empty artifact buckets keyed by evidence kind name."""

    names = _required_evidence_kind_list(evidence_kind_names)
    if names is None or not names:
        return {}
    if _duplicate_required_evidence_kind_names(names):
        return {}
    return {name: [] for name in names}


def _artifact_validation_error_list(validation_errors: Any) -> list[str]:
    """Return a stable artifact validation-error list."""

    def has_canonical_errors(errors: Sequence[str]) -> bool:
        return all(
            error
            and error == error.strip()
            and not any(
                ord(character) < 32 or ord(character) == 127 for character in error
            )
            for error in errors
        )

    if isinstance(validation_errors, list) and all(
        isinstance(error, str) for error in validation_errors
    ):
        if has_canonical_errors(validation_errors):
            return validation_errors
        return ["artifact validation errors must be non-empty canonical strings"]
    if (
        not isinstance(validation_errors, (str, bytes, bytearray))
        and isinstance(validation_errors, Sequence)
        and all(isinstance(error, str) for error in validation_errors)
    ):
        errors = list(validation_errors)
        if has_canonical_errors(errors):
            return errors
        return ["artifact validation errors must be non-empty canonical strings"]
    return ["artifact validation errors must be a sequence of strings"]


def _record_artifact_builder_error(errors: list[str], error: str) -> None:
    """Append an artifact-builder error once."""

    if error not in errors:
        errors.append(error)


def _artifact_summary_field_or_error(
    payload: Any,
    field: str,
    errors: list[str],
    *,
    record_error: bool,
) -> str | None:
    """Return a canonical artifact summary field or record a builder error."""

    if not isinstance(payload, Mapping):
        return None
    label_errors: list[str] = []
    field_label = _require_validation_label(
        field,
        label_errors,
        label_name="artifact summary field",
    )
    if field_label is None:
        if record_error:
            _record_artifact_builder_error(errors, label_errors[0])
        return None
    value_label = _require_validation_label(
        payload.get(field_label),
        label_errors,
        label_name=f"artifact {field_label}",
    )
    if value_label is None:
        if record_error:
            _record_artifact_builder_error(errors, label_errors[-1])
        return None
    return value_label


def _artifact_sha256_or_error(digest: Any, errors: list[str]) -> str:
    """Return a canonical artifact SHA-256 digest or record a builder error."""

    if isinstance(digest, str) and SHA256_HEX_PATTERN.fullmatch(digest):
        return digest
    _record_artifact_builder_error(
        errors,
        "artifact sha256 must be a 64-character lowercase hex string",
    )
    return ""


def _artifact_path_or_error(path: Any, errors: list[str]) -> str:
    """Return a canonical artifact path label or record a builder error."""

    if not isinstance(path, Path):
        _record_artifact_builder_error(errors, "artifact path must be a path")
        return "<unknown>"
    label = str(path)
    if (
        not label
        or label != label.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in label)
    ):
        _record_artifact_builder_error(errors, "artifact path must be a canonical path")
        return "<unknown>"
    return label


def _artifact_kind_name_or_error(kind_name: Any, errors: list[str]) -> str:
    """Return a non-empty artifact kind name or record a builder error."""

    if not isinstance(kind_name, str) or not kind_name.strip():
        _record_artifact_builder_error(
            errors,
            "artifact kind must be a non-empty string",
        )
        return "<unknown>"
    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="artifact kind",
    )
    if kind_label is None:
        _record_artifact_builder_error(errors, label_errors[0])
        return "<unknown>"
    return kind_label


def _artifact_fingerprint_or_error(
    payload: Any,
    fingerprint_fields: Any,
    errors: list[str],
) -> dict[str, Any]:
    """Return artifact fingerprint fields or record a stable builder error."""

    try:
        return artifact_fingerprint(payload, fingerprint_fields)
    except ValueError as exc:
        _record_artifact_builder_error(errors, str(exc))
        return {}


def _merge_artifact_fingerprint_values(
    fingerprint: dict[str, Any],
    fingerprint_values: Any,
    errors: list[str],
) -> None:
    """Merge explicit fingerprint values or record a stable builder error."""

    if fingerprint_values is None:
        return
    if not isinstance(fingerprint_values, Mapping):
        _record_artifact_builder_error(
            errors,
            "artifact fingerprint values must be a mapping",
        )
        return
    values_to_merge: list[tuple[str, Any]] = []
    for key, value in fingerprint_values.items():
        if not isinstance(key, str) or not key.strip():
            _record_artifact_builder_error(
                errors,
                "artifact fingerprint value keys must be non-empty strings",
            )
            return
        label_errors: list[str] = []
        key_label = _require_validation_label(
            key,
            label_errors,
            label_name="artifact fingerprint value key",
        )
        if key_label is None:
            _record_artifact_builder_error(errors, label_errors[0])
            return
        values_to_merge.append((key_label, value))
    for key, value in values_to_merge:
        fingerprint[key] = value


def build_evidence_artifact(
    path: Any,
    digest: Any,
    payload: Any,
    validation_errors: Any,
    fingerprint_fields: Any,
) -> dict[str, Any]:
    """Build the standard payload-free artifact row for evidence summaries."""

    errors = _artifact_validation_error_list(validation_errors)
    record_summary_field_errors = not errors
    schema = _artifact_summary_field_or_error(
        payload,
        "schema",
        errors,
        record_error=record_summary_field_errors,
    )
    status = _artifact_summary_field_or_error(
        payload,
        "status",
        errors,
        record_error=record_summary_field_errors,
    )
    fingerprint = _artifact_fingerprint_or_error(payload, fingerprint_fields, errors)
    sha256 = _artifact_sha256_or_error(digest, errors)
    path_label = _artifact_path_or_error(path, errors)
    return {
        "path": path_label,
        "sha256": sha256,
        "schema": schema,
        "status": status,
        "fingerprint": fingerprint,
        "valid": not errors,
        "errors": errors,
    }


def build_kinded_evidence_artifact(
    *,
    kind_name: Any,
    path: Any,
    digest: Any,
    payload: Any,
    validation_errors: Any,
    fingerprint_fields: Any,
    fingerprint_values: Any = None,
) -> dict[str, Any]:
    """Build a payload-free artifact row keyed by evidence kind."""

    errors = _artifact_validation_error_list(validation_errors)
    kind = _artifact_kind_name_or_error(kind_name, errors)
    sha256 = _artifact_sha256_or_error(digest, errors)
    path_label = _artifact_path_or_error(path, errors)
    fingerprint = _artifact_fingerprint_or_error(payload, fingerprint_fields, errors)
    _merge_artifact_fingerprint_values(fingerprint, fingerprint_values, errors)
    return {
        "kind": kind,
        "path": path_label,
        "sha256": sha256,
        "fingerprint": fingerprint,
        "valid": not errors,
        "errors": errors,
    }


def record_evidence_artifact(
    artifacts_by_kind: Any,
    kind_name: Any,
    artifact: Any,
    errors: list[str] | None = None,
) -> bool:
    """Append a recognized evidence artifact to its kind bucket."""

    if not isinstance(artifacts_by_kind, Mapping):
        if errors is not None:
            errors.append("recognized evidence artifacts by kind must be a mapping")
        return False
    if not isinstance(kind_name, str) or not kind_name:
        if errors is not None:
            errors.append("recognized evidence kind must be a non-empty string")
        return False
    label_errors: list[str] = []
    kind_label = _require_validation_label(
        kind_name,
        label_errors,
        label_name="recognized evidence kind",
    )
    if kind_label is None:
        if errors is not None:
            errors.append(label_errors[0])
        return False
    if not isinstance(artifact, dict):
        if errors is not None:
            errors.append(
                f"recognized `{kind_label}` evidence artifact must be an object"
            )
        return False
    artifacts = artifacts_by_kind.get(kind_label)
    if not isinstance(artifacts, list):
        if errors is not None:
            errors.append(
                f"recognized evidence kind `{kind_label}` has no artifact bucket"
            )
        return False
    if not all(isinstance(existing_artifact, Mapping) for existing_artifact in artifacts):
        if errors is not None:
            errors.append(
                f"recognized evidence kind `{kind_label}` artifact bucket must be a sequence of artifact objects"
            )
        return False
    artifacts.append(artifact)
    return True


def evidence_artifact_is_valid(artifact: Any) -> bool:
    """Return whether an evidence artifact is explicitly marked valid."""

    if not isinstance(artifact, Mapping):
        return False
    return artifact.get("valid") is True


def evidence_artifact_kind(artifact: Any) -> str | None:
    """Return a canonical artifact kind name when present."""

    if not isinstance(artifact, Mapping):
        return None
    kind = artifact.get("kind")
    return _require_validation_label(
        kind,
        [],
        label_name="artifact kind",
    )


def evidence_artifact_fingerprint(artifact: Any) -> Mapping[str, Any]:
    """Return an artifact fingerprint mapping or an empty mapping."""

    if not isinstance(artifact, Mapping):
        return {}
    fingerprint = artifact.get("fingerprint")
    if isinstance(fingerprint, Mapping):
        return fingerprint
    return {}


def evidence_artifact_detail(
    artifact: Any,
    field: Any,
) -> Mapping[str, Any]:
    """Return an artifact detail mapping or an empty mapping."""

    if not isinstance(artifact, Mapping):
        return {}
    field_label = _require_validation_label(
        field,
        [],
        label_name="artifact detail field",
    )
    if field_label is None:
        return {}
    detail = artifact.get(field_label)
    if isinstance(detail, Mapping):
        return detail
    return {}


def evidence_artifact_schema(artifact: Any) -> str:
    """Return an artifact schema label for diagnostics."""

    if not isinstance(artifact, Mapping):
        return "<unknown>"
    schema = artifact.get("schema")
    schema_label = _require_validation_label(
        schema,
        [],
        label_name="artifact schema",
    )
    if schema_label is not None:
        return schema_label
    return "<unknown>"


def _evidence_sequence_count(value: Any) -> int:
    """Return the item count for well-formed evidence path sequences."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        return 0
    if not all(isinstance(item, Path) for item in value):
        return 0
    return len(value)


def _evidence_artifact_row_count(value: Any) -> int:
    """Return the item count for well-formed artifact-row sequences."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        return 0
    if not all(isinstance(artifact, Mapping) for artifact in value):
        return 0
    return len(value)


def count_evidence_artifacts(
    artifacts_by_kind: Any,
) -> int:
    """Return the total number of recognized evidence artifacts."""

    if not isinstance(artifacts_by_kind, Mapping):
        return 0
    artifact_count = 0
    for kind_name, artifacts in artifacts_by_kind.items():
        if (
            _require_validation_label(
                kind_name,
                [],
                label_name="recognized evidence kind",
            )
            is None
        ):
            return 0
        artifact_rows = _evidence_artifact_rows(artifacts)
        if artifact_rows is None:
            return 0
        artifact_count += len(artifact_rows)
    return artifact_count


def count_recognized_evidence_artifacts(
    recognized: Any,
) -> int:
    """Return the total number of recognized evidence artifact rows."""

    return _evidence_artifact_row_count(recognized)


def recognized_evidence_artifacts_are_valid(
    recognized: Any,
) -> bool:
    """Return whether every recognized evidence artifact row is valid."""

    if not isinstance(recognized, Sequence) or isinstance(
        recognized, (str, bytes, bytearray)
    ):
        return False
    return all(
        isinstance(artifact, Mapping) and evidence_artifact_is_valid(artifact)
        for artifact in recognized
    )


def count_evidence_files(files: Any) -> int:
    """Return the total number of discovered evidence files."""

    return _evidence_sequence_count(files)


def evidence_gate_status(errors: Any) -> str:
    """Return the standard gate status for a summary error list."""

    if not isinstance(errors, list):
        return "blocked"
    if not all(
        isinstance(error, str)
        and error
        and error == error.strip()
        and not any(ord(character) < 32 or ord(character) == 127 for character in error)
        for error in errors
    ):
        return "blocked"
    return "blocked" if errors else "ready"


def record_evidence_validation_errors(
    path: Any,
    validation_errors: Any,
    errors: list[str],
) -> None:
    """Append path-qualified payload validation errors to the summary errors."""

    summary_errors = _require_error_list(errors)
    path_label = _evidence_validation_path_label(path, summary_errors)
    if path_label is None:
        return
    messages = _validation_error_messages(validation_errors)
    if messages is None:
        summary_errors.append(
            f"{path_label}: validation errors must be a sequence of strings"
        )
        return
    if not messages and validation_errors:
        summary_errors.append(
            f"{path_label}: validation errors must be non-empty canonical strings"
        )
        return
    summary_errors.extend(f"{path_label}: {error}" for error in messages)


def record_explicit_evidence_validation_errors(
    path: Path,
    explicit_identities: set[Path],
    validation_errors: Sequence[str],
    errors: list[str],
) -> None:
    """Record validation errors only for explicit evidence path inputs."""

    if is_explicit_evidence_path(path, explicit_identities, errors):
        record_evidence_validation_errors(path, validation_errors, errors)


def require_string_equal(
    payload: Mapping[str, Any],
    field: str,
    expected: str,
    errors: list[str],
    *,
    path: str | None = None,
    quote_expected: bool = True,
) -> None:
    """Require a field to match an expected string exactly."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return
    if not isinstance(quote_expected, bool):
        errors.append("validation quote_expected must be a boolean")
        return
    expected_value = _require_validation_label(
        expected,
        errors,
        label_name="validation expected value",
    )
    if expected_value is None:
        return
    value = _require_validation_label(
        payload.get(field_name),
        errors,
        label_name=diagnostic_label,
    )
    if value is None:
        return
    if value != expected_value:
        expected_label = f"`{expected_value}`" if quote_expected else expected_value
        errors.append(f"{diagnostic_label} must be {expected_label}")


def require_string_not_equal(
    payload: Mapping[str, Any],
    field: str,
    disallowed: str,
    errors: list[str],
    *,
    message: str | None = None,
) -> str:
    """Return a string field and require it to differ from a disallowed value."""

    value = require_string(payload, field, errors)
    if not value:
        return ""
    disallowed_value = _require_validation_label(
        disallowed,
        errors,
        label_name="validation disallowed value",
    )
    if disallowed_value is None:
        return ""
    message_label = (
        None
        if message is None
        else _require_validation_label(
            message,
            errors,
            label_name="validation message",
        )
    )
    if message is not None and message_label is None:
        return ""
    if value == disallowed_value:
        errors.append(message_label or f"{field} must not be `{disallowed_value}`")
    return value


def require_string_value_equal(
    value: Any,
    label: str,
    expected: Any,
    expected_label: str,
    errors: list[str],
    *,
    message: str | None = None,
) -> str:
    """Return a string value and require it to match another string value."""

    value_label = _require_validation_label(
        label,
        errors,
        label_name="validation value label",
    )
    expected_value_label = _require_validation_label(
        expected_label,
        errors,
        label_name="validation expected label",
    )
    if value_label is None or expected_value_label is None:
        return ""
    value_value = _require_validation_label(
        value,
        errors,
        label_name=value_label,
    )
    if value_value is None:
        return ""
    expected_value = _require_validation_label(
        expected,
        errors,
        label_name=expected_value_label,
    )
    if expected_value is None:
        return ""
    if value_value != expected_value:
        message_label = (
            None
            if message is None
            else _require_validation_label(
                message,
                errors,
                label_name="validation message",
            )
        )
        if message is not None and message_label is None:
            return ""
        errors.append(message_label or f"{value_label} must match {expected_value_label}")
    return value_value


def require_string_value_in(
    value: Any,
    allowed: Any,
    errors: list[str],
    *,
    message: str,
) -> str | None:
    """Return a normalized string value that belongs to an allowed set."""

    message_label = _require_validation_label(
        message,
        errors,
        label_name="validation message",
    )
    if message_label is None:
        return None
    if not isinstance(value, str) or not value:
        errors.append(message_label)
        return None
    value_label = _require_validation_label(
        value,
        errors,
        label_name="validation value",
    )
    if value_label is None:
        return None
    normalized = value_label.lower()
    if isinstance(allowed, (str, bytes, bytearray, Mapping)) or not isinstance(
        allowed, Collection
    ):
        errors.append(message_label)
        return None
    allowed_values: set[str] = set()
    for allowed_value in allowed:
        if not isinstance(allowed_value, str) or not allowed_value:
            errors.append(message_label)
            return None
        allowed_label = _require_validation_label(
            allowed_value,
            errors,
            label_name="validation allowed value",
        )
        if allowed_label is None:
            return None
        allowed_values.add(allowed_label.lower())
    if normalized not in allowed_values:
        errors.append(message_label)
        return None
    return normalized


def require_string_tuple_in(
    values: Any,
    allowed: Any,
    errors: list[str],
    *,
    message: str,
) -> tuple[str, ...] | None:
    """Return a normalized string tuple that belongs to an allowed set."""

    message_label = _require_validation_label(
        message,
        errors,
        label_name="validation message",
    )
    if message_label is None:
        return None
    if isinstance(values, (str, bytes, bytearray)) or not isinstance(
        values, Sequence
    ):
        errors.append(message_label)
        return None
    if not values:
        errors.append(message_label)
        return None
    normalized_values: list[str] = []
    for value in values:
        if not isinstance(value, str) or not value:
            errors.append(message_label)
            return None
        if (
            _require_validation_label(
                value,
                errors,
                label_name="validation tuple value",
            )
            is None
        ):
            return None
        normalized_values.append(value.lower())
    binding = tuple(normalized_values)
    if isinstance(allowed, (str, bytes, bytearray, Mapping)) or not isinstance(
        allowed, Collection
    ):
        errors.append(message_label)
        return None
    allowed_bindings: set[tuple[str, ...]] = set()
    for allowed_binding in allowed:
        if isinstance(allowed_binding, (str, bytes, bytearray)) or not isinstance(
            allowed_binding, Sequence
        ):
            errors.append(message_label)
            return None
        raw_allowed = tuple(allowed_binding)
        if not raw_allowed:
            errors.append(message_label)
            return None
        normalized_allowed: list[str] = []
        for allowed_value in raw_allowed:
            if not isinstance(allowed_value, str) or not allowed_value:
                errors.append(message_label)
                return None
            allowed_label = _require_validation_label(
                allowed_value,
                errors,
                label_name="validation allowed tuple value",
            )
            if allowed_label is None:
                return None
            normalized_allowed.append(allowed_label.lower())
        allowed_bindings.add(tuple(normalized_allowed))
    if binding not in allowed_bindings:
        errors.append(message_label)
        return None
    return binding


def record_string_value_binding_errors(
    artifact: dict[str, Any],
    value: Any,
    allowed: Any,
    errors: list[str],
    *,
    message: str,
) -> str | None:
    """Validate a scalar string binding and mirror failures to artifact errors."""

    binding_errors: list[str] = []
    binding = require_string_value_in(
        value,
        allowed,
        binding_errors,
        message=message,
    )
    for error in binding_errors:
        record_artifact_error(artifact, error, errors)
    return binding


def _fingerprint_field_name(
    field: Any,
    errors: list[str],
    *,
    label_name: str = "validation digest field",
) -> str | None:
    """Return a canonical fingerprint field name or reject malformed labels."""

    return _require_validation_label(field, errors, label_name=label_name)


def _fingerprint_field_names(
    fields: Any,
    errors: list[str],
) -> tuple[str, ...] | None:
    """Return canonical fingerprint field names or reject malformed containers."""

    if isinstance(fields, (str, bytes, bytearray, Mapping)) or not isinstance(
        fields, Sequence
    ):
        errors.append("validation binding fields must be a sequence of strings")
        return None
    if not fields:
        errors.append("validation binding fields must not be empty")
        return None
    field_names: list[str] = []
    for field in fields:
        field_name = _fingerprint_field_name(
            field,
            errors,
            label_name="validation binding field",
        )
        if field_name is None:
            return None
        field_names.append(field_name)
    return tuple(field_names)


def _digest_field_by_kind_labels(
    digest_field_by_kind: Any,
    errors: list[str],
) -> dict[str, str] | None:
    """Return canonical per-kind digest field selectors."""

    if digest_field_by_kind is None:
        return {}
    if not isinstance(digest_field_by_kind, Mapping):
        errors.append("validation digest field map must be a mapping")
        return None
    fields_by_kind: dict[str, str] = {}
    for kind_name, field in digest_field_by_kind.items():
        kind_label = _require_validation_label(
            kind_name,
            errors,
            label_name="validation evidence kind",
        )
        field_label = _fingerprint_field_name(field, errors)
        if kind_label is None or field_label is None:
            return None
        fields_by_kind[kind_label] = field_label
    return fields_by_kind


def evidence_artifact_digest_set(
    artifacts: Sequence[dict[str, Any]],
    digest_field: Any,
) -> set[str]:
    """Return normalized digest values from valid evidence artifacts."""

    field_name = _fingerprint_field_name(digest_field, [])
    if field_name is None:
        return set()
    if isinstance(artifacts, (str, bytes, bytearray, Mapping)) or not isinstance(
        artifacts, Sequence
    ):
        return set()
    values: set[str] = set()
    for artifact in artifacts:
        if not isinstance(artifact, Mapping):
            return set()
        if not evidence_artifact_is_valid(artifact):
            continue
        digest = evidence_artifact_fingerprint(artifact).get(field_name)
        digest_label = _require_validation_label(
            digest,
            [],
            label_name="validation digest value",
        )
        if digest_label is None:
            return set()
        values.add(digest_label.lower())
    return values


def _canonical_digest_value_set(
    digests: Any,
    errors: list[str],
) -> set[str] | None:
    """Return normalized digest values or reject malformed containers."""

    if isinstance(digests, (str, bytes, bytearray, Mapping)) or not isinstance(
        digests, Collection
    ):
        errors.append(
            "validation anchor digests must be a collection of canonical strings"
        )
        return None
    values: set[str] = set()
    has_malformed_value = False
    for digest in digests:
        digest_label = _require_validation_label(
            digest,
            errors,
            label_name="validation anchor digest",
        )
        if digest_label is None:
            has_malformed_value = True
            continue
        values.add(digest_label.lower())
    if has_malformed_value:
        return None
    return values


def _canonical_tuple_binding_set(
    bindings: Any,
    errors: list[str],
) -> set[tuple[str, ...]] | None:
    """Return normalized tuple bindings or reject malformed containers."""

    if isinstance(bindings, (str, bytes, bytearray, Mapping)) or not isinstance(
        bindings, Collection
    ):
        errors.append(
            "validation anchor bindings must be a collection of canonical string sequences"
        )
        return None
    values: set[tuple[str, ...]] = set()
    has_malformed_binding = False
    for binding in bindings:
        if isinstance(binding, (str, bytes, bytearray, Mapping)) or not isinstance(
            binding, Sequence
        ):
            errors.append(
                "validation anchor binding must be a non-empty sequence of canonical strings"
            )
            has_malformed_binding = True
            continue
        if not binding:
            errors.append(
                "validation anchor binding must be a non-empty sequence of canonical strings"
            )
            has_malformed_binding = True
            continue
        binding_values: list[str] = []
        binding_has_malformed_value = False
        for binding_value in binding:
            binding_label = _require_validation_label(
                binding_value,
                errors,
                label_name="validation anchor binding value",
            )
            if binding_label is None:
                has_malformed_binding = True
                binding_has_malformed_value = True
                continue
            binding_values.append(binding_label.lower())
        if not binding_has_malformed_value:
            values.add(tuple(binding_values))
    if has_malformed_binding:
        return None
    return values


def record_evidence_digest_mismatch_errors(
    *,
    artifacts: Any,
    digest_field: Any,
    allowed_digests: Collection[str],
    errors: list[str],
    error: str,
) -> None:
    """Record artifact errors for digest values outside the allowed set."""

    field_name = _fingerprint_field_name(digest_field, errors)
    if field_name is None:
        return
    allowed_digest_values = _canonical_digest_value_set(allowed_digests, errors)
    if allowed_digest_values is None:
        return
    artifact_input_error = (
        "evidence digest mismatch artifacts must be a sequence of artifact objects"
    )
    if isinstance(artifacts, (str, bytes, bytearray, Mapping)) or not isinstance(
        artifacts, Sequence
    ):
        errors.append(artifact_input_error)
        return
    artifact_rows: list[dict[str, Any]] = []
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            errors.append(artifact_input_error)
            return
        artifact_rows.append(artifact)
    for artifact in artifact_rows:
        if not evidence_artifact_is_valid(artifact):
            continue
        digest = evidence_artifact_fingerprint(artifact).get(field_name)
        record_string_value_binding_errors(
            artifact,
            digest,
            allowed_digest_values,
            errors,
            message=error,
        )


def validate_bound_evidence_digest_references(
    *,
    required_kinds: Collection[str],
    missing_anchor_required_kinds: Collection[str],
    bound_artifacts: Sequence[tuple[str, dict[str, Any]]],
    valid_anchor_digests: Collection[str],
    errors: list[str],
    binding_error_template: str,
    missing_anchor_error_template: str,
    digest_field: str | None = None,
    digest_field_by_kind: Mapping[str, str] | None = None,
    missing_anchor_artifacts: Sequence[tuple[str, dict[str, Any]]] | None = None,
    missing_anchor_summary_error: str | None = None,
) -> None:
    """Validate downstream scalar digest references against valid anchors."""

    valid_anchor_digest_values = _canonical_digest_value_set(
        valid_anchor_digests,
        errors,
    )
    if valid_anchor_digest_values is None:
        return

    if valid_anchor_digest_values:
        binding_template = _validate_kind_error_template(
            binding_error_template,
            errors,
            template_name="validation binding error template",
        )
        if binding_template is None:
            return
        default_digest_field = None
        if digest_field is not None:
            default_digest_field = _fingerprint_field_name(digest_field, errors)
            if default_digest_field is None:
                return
        digest_fields_by_kind = _digest_field_by_kind_labels(
            digest_field_by_kind,
            errors,
        )
        if digest_fields_by_kind is None:
            return
        artifact_pairs = _evidence_kind_artifact_pairs(
            bound_artifacts,
            errors,
            label="bound evidence artifacts",
        )
        if artifact_pairs is None:
            return
        for kind_name, artifact in artifact_pairs:
            field = digest_fields_by_kind.get(kind_name, default_digest_field)
            if field is None:
                errors.append(
                    "validation digest field must be a non-empty canonical string"
                )
                return
            digest = (
                evidence_artifact_fingerprint(artifact).get(field)
                if field is not None
                else None
            )
            message = _format_kind_error_template(
                binding_template,
                kind_name,
                errors,
                template_name="validation binding error template",
            )
            if message is None:
                return
            record_string_value_binding_errors(
                artifact,
                digest,
                valid_anchor_digest_values,
                errors,
                message=message,
            )
        return
    required_missing_anchor = _required_evidence_has_any_kind_or_error(
        required_kinds,
        missing_anchor_required_kinds,
        errors,
    )
    if required_missing_anchor is None or not required_missing_anchor:
        return
    missing_anchor_template = _validate_kind_error_template(
        missing_anchor_error_template,
        errors,
        template_name="validation missing-anchor error template",
    )
    if missing_anchor_template is None:
        return
    summary_error = None
    if missing_anchor_summary_error is not None:
        summary_error = _require_validation_label(
            missing_anchor_summary_error,
            errors,
            label_name="validation missing-anchor summary error",
        )
        if summary_error is None:
            return
    artifact_pairs = _evidence_kind_artifact_pairs(
        missing_anchor_artifacts or bound_artifacts,
        errors,
        label="missing-anchor evidence artifacts"
        if missing_anchor_artifacts
        else "bound evidence artifacts",
    )
    if artifact_pairs is None:
        return
    for kind_name, artifact in artifact_pairs:
        error = _format_kind_error_template(
            missing_anchor_template,
            kind_name,
            errors,
            template_name="validation missing-anchor error template",
        )
        if error is None:
            return
        record_artifact_error(
            artifact,
            error,
            errors,
        )
    if summary_error is not None:
        errors.append(summary_error)


def validate_bound_evidence_tuple_references(
    *,
    required_kinds: Collection[str],
    missing_anchor_required_kinds: Collection[str],
    bound_artifacts: Sequence[tuple[str, dict[str, Any]]],
    valid_anchor_bindings: Collection[tuple[str, ...]],
    binding_fields: Sequence[str],
    errors: list[str],
    binding_error_template: str,
    missing_anchor_error_template: str,
    missing_anchor_artifacts: Sequence[tuple[str, dict[str, Any]]] | None = None,
    missing_anchor_summary_error: str | None = None,
) -> None:
    """Validate downstream tuple digest references against valid anchors."""

    valid_anchor_binding_values = _canonical_tuple_binding_set(
        valid_anchor_bindings,
        errors,
    )
    if valid_anchor_binding_values is None:
        return

    if valid_anchor_binding_values:
        binding_template = _validate_kind_error_template(
            binding_error_template,
            errors,
            template_name="validation binding error template",
        )
        if binding_template is None:
            return
        binding_field_names = _fingerprint_field_names(binding_fields, errors)
        if binding_field_names is None:
            return
        artifact_pairs = _evidence_kind_artifact_pairs(
            bound_artifacts,
            errors,
            label="bound evidence artifacts",
        )
        if artifact_pairs is None:
            return
        for kind_name, artifact in artifact_pairs:
            fingerprint = evidence_artifact_fingerprint(artifact)
            message = _format_kind_error_template(
                binding_template,
                kind_name,
                errors,
                template_name="validation binding error template",
            )
            if message is None:
                return
            record_string_tuple_binding_errors(
                artifact,
                tuple(fingerprint.get(field) for field in binding_field_names),
                valid_anchor_binding_values,
                errors,
                message=message,
            )
        return
    required_missing_anchor = _required_evidence_has_any_kind_or_error(
        required_kinds,
        missing_anchor_required_kinds,
        errors,
    )
    if required_missing_anchor is None or not required_missing_anchor:
        return
    missing_anchor_template = _validate_kind_error_template(
        missing_anchor_error_template,
        errors,
        template_name="validation missing-anchor error template",
    )
    if missing_anchor_template is None:
        return
    summary_error = None
    if missing_anchor_summary_error is not None:
        summary_error = _require_validation_label(
            missing_anchor_summary_error,
            errors,
            label_name="validation missing-anchor summary error",
        )
        if summary_error is None:
            return
    artifact_pairs = _evidence_kind_artifact_pairs(
        missing_anchor_artifacts or bound_artifacts,
        errors,
        label="missing-anchor evidence artifacts"
        if missing_anchor_artifacts
        else "bound evidence artifacts",
    )
    if artifact_pairs is None:
        return
    for kind_name, artifact in artifact_pairs:
        error = _format_kind_error_template(
            missing_anchor_template,
            kind_name,
            errors,
            template_name="validation missing-anchor error template",
        )
        if error is None:
            return
        record_artifact_error(
            artifact,
            error,
            errors,
        )
    if summary_error is not None:
        errors.append(summary_error)


def record_string_tuple_binding_errors(
    artifact: dict[str, Any],
    values: Sequence[Any],
    allowed: Collection[tuple[str, ...]],
    errors: list[str],
    *,
    message: str,
) -> tuple[str, ...] | None:
    """Validate a tuple binding and mirror failures to artifact errors."""

    binding_errors: list[str] = []
    binding = require_string_tuple_in(
        values,
        allowed,
        binding_errors,
        message=message,
    )
    for error in binding_errors:
        record_artifact_error(artifact, error, errors)
    return binding


def require_string_in(
    payload: Mapping[str, Any],
    field: str,
    allowed: Any,
    errors: list[str],
    *,
    path: str | None = None,
    quote_values: bool = True,
) -> str:
    """Return a string field that belongs to an allowed set or append an error."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return ""
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return ""
    allowed_path = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if allowed_path is None:
        return ""
    if not isinstance(quote_values, bool):
        errors.append("validation quote_values must be a boolean")
        return ""
    value = payload.get(field_name)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{allowed_path} must be a non-empty string")
        return ""
    value_label = _require_validation_label(
        value,
        errors,
        label_name="validation value",
    )
    if value_label is None:
        return ""
    if isinstance(allowed, (str, bytes, bytearray, Mapping)) or not isinstance(
        allowed, Sequence
    ):
        errors.append(
            f"{allowed_path} allowed values must be a sequence of strings"
        )
        return ""
    if not allowed:
        errors.append(
            f"{allowed_path} allowed values must be a sequence of strings"
        )
        return ""
    allowed_values: list[str] = []
    for allowed_value in allowed:
        if not isinstance(allowed_value, str) or not allowed_value:
            errors.append(
                f"{allowed_path} allowed values must be a sequence of strings"
            )
            return ""
        allowed_value_label = _require_validation_label(
            allowed_value,
            errors,
            label_name="validation allowed value",
        )
        if allowed_value_label is None:
            return ""
        allowed_values.append(allowed_value_label)
    if value_label in allowed_values:
        return value_label
    labels = [
        f"`{allowed_value}`" if quote_values else allowed_value
        for allowed_value in allowed_values
    ]
    if len(labels) == 1:
        allowed_label = labels[0]
    else:
        allowed_label = ", ".join(labels[:-1]) + f" or {labels[-1]}"
    errors.append(f"{allowed_path} must be {allowed_label}")
    return ""


ALLOWED_ROLLOUT_ENVIRONMENTS = {"prod", "production", "release", "staging"}
FORBIDDEN_ROLLOUT_DEPLOYMENT_MARKERS = {
    "changeme",
    "demo",
    "dev",
    "example",
    "local",
    "localnet",
    "mock",
    "placeholder",
    "preprod",
    "preview",
    "qa",
    "sample",
    "sandbox",
    "test",
    "testnet",
    "uat",
    "zero",
}
FORBIDDEN_ROLLOUT_DEPLOYMENT_COMPACT_MARKERS = {
    "changeme",
    "development",
    "dummy",
    "example",
    "fake",
    "localnet",
    "mock",
    "nonprod",
    "nonproduction",
    "notforproduction",
    "notforprod",
    "notprod",
    "notproductionready",
    "placeholder",
    "preprod",
    "replacebeforedeploy",
    "replacebeforeproduction",
    "replacebeforeprod",
    "replacebeforerelease",
    "replaceme",
    "sample",
    "sandbox",
    "testnet",
    "testing",
    "todo",
}
ROLLOUT_DEPLOYMENT_ID_PATTERN = re.compile(
    r"^[A-Za-z0-9](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9])?$"
)


def _require_canonical_payload_string(
    payload: Any,
    field: str,
    errors: list[str],
) -> str:
    """Return a canonical string payload field or append a validation error."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return ""
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return ""
    value = payload.get(field_name)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{field_name} must be a non-empty string")
        return ""
    value_label = _require_validation_label(
        value,
        errors,
        label_name=field_name,
    )
    return value_label or ""


def require_rollout_deployment_id(
    payload: Mapping[str, Any],
    errors: list[str],
    *,
    field: str = "deployment_id",
) -> str:
    """Return a reviewed deployment id or append a validation error."""

    value = _require_canonical_payload_string(payload, field, errors)
    if not value:
        return ""
    if ROLLOUT_DEPLOYMENT_ID_PATTERN.fullmatch(value) is None:
        errors.append(
            f"{field} must be 1-128 ASCII letters, digits, '.', '_' or '-' "
            "and start/end with a letter or digit"
        )
        return ""
    tokens = [token for token in re.split(r"[._-]+", value.lower()) if token]
    compact = "".join(tokens)
    compact_forbidden = {
        marker
        for marker in FORBIDDEN_ROLLOUT_DEPLOYMENT_COMPACT_MARKERS
        if marker in compact
    }
    compact_forbidden = {
        marker
        for marker in compact_forbidden
        if not any(
            marker != other and marker in other for other in compact_forbidden
        )
    }
    forbidden = sorted(
        {token for token in tokens if token in FORBIDDEN_ROLLOUT_DEPLOYMENT_MARKERS}
        | compact_forbidden
    )
    if forbidden:
        errors.append(
            f"{field} must not contain non-reviewed deployment markers {forbidden}"
        )
        return ""
    return value


def require_rollout_environment(
    payload: Mapping[str, Any],
    errors: list[str],
    *,
    field: str = "environment",
) -> str:
    """Return a reviewed rollout environment or append a validation error."""

    value = _require_canonical_payload_string(payload, field, errors)
    if not value:
        return ""
    normalized = value.lower()
    if normalized not in ALLOWED_ROLLOUT_ENVIRONMENTS:
        errors.append(
            f"{field} must be one of {sorted(ALLOWED_ROLLOUT_ENVIRONMENTS)}"
        )
        return ""
    return normalized


def require_rollout_deployment_context_review(
    payload: Mapping[str, Any],
    errors: list[str],
    *,
    field: str = "deployment_context_reviewed",
) -> None:
    """Require explicit operator review of the rollout deployment context."""

    require_bool_true(payload, field, errors)


def require_passed_status(
    payload: Mapping[str, Any],
    errors: list[str],
    *,
    path: str = "status",
) -> None:
    """Require a payload status field to be exactly passed."""

    require_status_in(payload, ("passed",), errors, path=path)


def require_status_in(
    payload: Mapping[str, Any],
    allowed: Any,
    errors: list[str],
    *,
    field: str = "status",
    path: str = "status",
    allow_absent: bool = False,
) -> str:
    """Require a status field to match one of the allowed status labels."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return ""
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return ""
    diagnostic_label = _require_validation_label(
        path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return ""
    if isinstance(allowed, (str, bytes, bytearray, Mapping)) or not isinstance(
        allowed, Sequence
    ):
        errors.append(
            f"{diagnostic_label} allowed statuses must be a sequence of strings"
        )
        return ""
    if not allowed:
        errors.append(
            f"{diagnostic_label} allowed statuses must be a sequence of strings"
        )
        return ""
    allowed_values: list[str] = []
    for allowed_status in allowed:
        if not isinstance(allowed_status, str) or not allowed_status:
            errors.append(
                f"{diagnostic_label} allowed statuses must be a sequence of strings"
            )
            return ""
        allowed_status_label = _require_validation_label(
            allowed_status,
            errors,
            label_name="validation allowed status",
        )
        if allowed_status_label is None:
            return ""
        allowed_values.append(allowed_status_label)
    if not isinstance(allow_absent, bool):
        errors.append(f"{diagnostic_label} allow_absent must be a boolean")
        return ""
    value = payload.get(field_name)
    allowed_label = "/".join(allowed_values)
    if value is None and allow_absent:
        return ""
    if value is None:
        suffix = f"{allowed_label} when present" if allow_absent else allowed_label
        errors.append(f"{diagnostic_label} must be {suffix}")
        return ""
    value_label = _require_validation_label(
        value,
        errors,
        label_name=diagnostic_label,
    )
    if value_label is None:
        return ""
    if value_label not in allowed_values:
        suffix = f"{allowed_label} when present" if allow_absent else allowed_label
        errors.append(f"{diagnostic_label} must be {suffix}")
        return ""
    return value_label


def is_hex(value: str, length: int) -> bool:
    """Return whether a string is exactly the requested hexadecimal length."""

    if (
        not isinstance(value, str)
        or not isinstance(length, int)
        or isinstance(length, bool)
        or length <= 0
    ):
        return False
    return len(value) == length and all(
        char in "0123456789abcdefABCDEF" for char in value
    )


def _require_hex_length(length: Any, errors: list[str]) -> int | None:
    """Return a validated positive hex length or record a closed failure."""

    if not isinstance(length, int) or isinstance(length, bool) or length <= 0:
        errors.append("validation hex length must be a positive integer")
        return None
    return length


def require_hex(
    payload: Mapping[str, Any],
    field: str,
    length: int,
    errors: list[str],
    *,
    path: str | None = None,
) -> str:
    """Return a lowercase hex string field or append a validation error."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return ""
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return ""
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return ""
    hex_length = _require_hex_length(length, errors)
    if hex_length is None:
        return ""
    raw_value = payload.get(field_name)
    if isinstance(raw_value, str) and raw_value:
        value = raw_value
    else:
        errors.append(f"{diagnostic_label} must be a non-empty string")
        return ""
    if value and not is_hex(value, hex_length):
        errors.append(f"{diagnostic_label} must be {hex_length} hex characters")
        return ""
    return value.lower()


POLICY_DIGEST_HEX_LEN = 64


def require_policy_digest(
    payload: Mapping[str, Any],
    errors: list[str],
    *,
    field: str = "policy_digest_hex",
    length: int = POLICY_DIGEST_HEX_LEN,
) -> str:
    """Return a normalized governance policy digest or append an error."""

    return require_hex(payload, field, length, errors)


def require_optional_hex(
    payload: Mapping[str, Any],
    field: str,
    length: int,
    errors: list[str],
) -> None:
    """Require a field to be absent/null or exact-length hexadecimal."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return
    hex_length = _require_hex_length(length, errors)
    if hex_length is None:
        return
    value = payload.get(field_name)
    if value is None:
        return
    if not isinstance(value, str) or not is_hex(value, hex_length):
        errors.append(f"{field_name} must be null or {hex_length} hex characters")


def require_hex_string_array(
    payload: Mapping[str, Any],
    field: str,
    length: int,
    errors: list[str],
    *,
    non_empty: bool = False,
    expected_length: int | None = None,
    expected_length_label: str | None = None,
    unique: bool = False,
    path: str | None = None,
) -> list[str]:
    """Return normalized hex strings from an array field or append errors."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return []
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return []
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return []
    hex_length = _require_hex_length(length, errors)
    if hex_length is None:
        return []
    if not isinstance(non_empty, bool):
        errors.append(f"{diagnostic_label} non_empty must be a boolean")
        return []
    if not isinstance(unique, bool):
        errors.append(f"{diagnostic_label} unique must be a boolean")
        return []
    values = payload.get(field_name)
    if not isinstance(values, list) or (non_empty and not values):
        expected = "a non-empty array" if non_empty else "an array"
        errors.append(f"{diagnostic_label} must be {expected}")
        return []
    if expected_length is not None:
        if (
            not isinstance(expected_length, int)
            or isinstance(expected_length, bool)
            or expected_length < 0
        ):
            errors.append(
                f"{diagnostic_label} expected length must be a non-negative integer"
            )
            return []
    has_array_error = False
    if expected_length is not None and len(values) != expected_length:
        expected_label = (
            str(expected_length)
            if expected_length_label is None
            else _require_validation_label(
                expected_length_label,
                errors,
                label_name="validation count label",
            )
        )
        if expected_label is None:
            return []
        errors.append(f"{diagnostic_label} length must equal {expected_label}")
        has_array_error = True

    normalized_values: list[str] = []
    seen: set[str] = set()
    for index, value in enumerate(values):
        if not isinstance(value, str) or not is_hex(value, hex_length):
            errors.append(
                f"{diagnostic_label}[{index}] must be {hex_length} hex characters"
            )
            has_array_error = True
            continue
        normalized = value.lower()
        if unique and normalized in seen:
            errors.append(f"{diagnostic_label}[{index}] must be unique")
            has_array_error = True
        seen.add(normalized)
        normalized_values.append(normalized)
    if has_array_error:
        return []
    return normalized_values


def require_bool_true(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str | None = None,
) -> None:
    """Require a field to be exactly true."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return
    if payload.get(field_name) is not True:
        errors.append(f"{diagnostic_label} must be true")


def require_governance_approval(
    payload: Mapping[str, Any],
    errors: list[str],
) -> None:
    """Require rollout governance approval to be accepted and recorded."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    require_bool_true(payload, "approved", errors)
    require_bool_true(payload, "governance_vote_recorded", errors)


def require_iroha_config_binding(
    payload: Mapping[str, Any],
    errors: list[str],
    *,
    bound_field: str | None = "iroha_config_bound",
    source_field: str | None = "config_source",
) -> None:
    """Require rollout evidence to bind runtime behavior to iroha_config."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    if bound_field is not None:
        require_bool_true(payload, bound_field, errors)
    if source_field is not None:
        require_string_equal(payload, source_field, "iroha_config", errors)


def require_config_backed_governance_approval(
    payload: Mapping[str, Any],
    errors: list[str],
) -> None:
    """Require governance approval evidence to be accepted and config-backed."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    require_governance_approval(payload, errors)
    require_iroha_config_binding(payload, errors)


def require_false(payload: Mapping[str, Any], field: str, errors: list[str]) -> None:
    """Require a field to be exactly false."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return
    if payload.get(field_name) is not False:
        errors.append(f"{field_name} must be false")


def require_false_or_absent(
    payload: Mapping[str, Any], field: str, errors: list[str]
) -> None:
    """Require a field to be absent/null or exactly false."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return
    value = payload.get(field_name)
    if value is not None and value is not False:
        errors.append(f"{field_name} must be false when present")


def require_false_or_governed(
    payload: Mapping[str, Any],
    field: str,
    governed_field: str,
    errors: list[str],
) -> None:
    """Require a field to be exactly false unless exact true is governed."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return
    value = payload.get(field_name)
    if value is False:
        return
    if value is True:
        require_bool_true(payload, governed_field, errors)
        return
    errors.append(f"{field_name} must be false or explicitly governed")


def require_positive_int(
    payload: Mapping[str, Any], field: str, errors: list[str]
) -> int:
    """Return a positive integer field or append a validation error."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return 0
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return 0
    value = payload.get(field_name)
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        errors.append(f"{field_name} must be a positive integer")
        return 0
    return value


def require_minimum_int(
    payload: Mapping[str, Any],
    field: str,
    minimum: int,
    errors: list[str],
) -> int:
    """Return a positive integer field and require it to meet a minimum."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return 0
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return 0
    value = require_positive_int(payload, field_name, errors)
    require_minimum_value(value, field_name, minimum, errors)
    return value


def require_minimum_value(
    value: int,
    label: str,
    minimum: int,
    errors: list[str],
    *,
    message: str | None = None,
) -> int:
    """Return a computed integer value and require it to meet a minimum."""

    threshold_label = _require_validation_label(
        label,
        errors,
        label_name="validation threshold label",
    )
    if threshold_label is None:
        return 0
    if not isinstance(value, int) or isinstance(value, bool):
        errors.append(f"{threshold_label} must be an integer")
        return 0
    if not isinstance(minimum, int) or isinstance(minimum, bool):
        errors.append(f"{threshold_label} minimum threshold must be an integer")
        return 0
    if value < minimum:
        message_label = (
            None
            if message is None
            else _require_validation_label(
                message,
                errors,
                label_name="validation message",
            )
        )
        if message is not None and message_label is None:
            return 0
        errors.append(message_label or f"{threshold_label} must be at least {minimum}")
    return value


def require_maximum_value(
    value: int,
    label: str,
    maximum: int,
    errors: list[str],
    *,
    message: str | None = None,
) -> int:
    """Return a computed integer value and require it to meet a maximum."""

    threshold_label = _require_validation_label(
        label,
        errors,
        label_name="validation threshold label",
    )
    if threshold_label is None:
        return 0
    if not isinstance(value, int) or isinstance(value, bool):
        errors.append(f"{threshold_label} must be an integer")
        return 0
    if not isinstance(maximum, int) or isinstance(maximum, bool):
        errors.append(f"{threshold_label} maximum threshold must be an integer")
        return 0
    if value > maximum:
        message_label = (
            None
            if message is None
            else _require_validation_label(
                message,
                errors,
                label_name="validation message",
            )
        )
        if message is not None and message_label is None:
            return 0
        errors.append(message_label or f"{threshold_label} must be <= {maximum}")
    return value


def require_non_negative_int(
    payload: Mapping[str, Any], field: str, errors: list[str]
) -> int:
    """Return a non-negative integer field or append a validation error."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return 0
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return 0
    value = payload.get(field_name)
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        errors.append(f"{field_name} must be a non-negative integer")
        return 0
    return value


def require_zero_count(payload: Mapping[str, Any], field: str, errors: list[str]) -> None:
    """Require a non-negative integer count field to be exactly zero."""

    value = require_non_negative_int(payload, field, errors)
    if value != 0:
        errors.append(f"{field} must be 0")


def require_int_range(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    min_value: int,
    max_value: int,
    path: str | None = None,
    message: str | None = None,
) -> int:
    """Return an integer within an inclusive range or append a validation error."""

    if not isinstance(min_value, int) or isinstance(min_value, bool):
        errors.append("validation minimum threshold must be an integer")
        return 0
    if not isinstance(max_value, int) or isinstance(max_value, bool):
        errors.append("validation maximum threshold must be an integer")
        return min_value
    if max_value < min_value:
        errors.append("validation maximum threshold must be >= minimum threshold")
        return min_value
    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return min_value
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return min_value
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return min_value
    value = payload.get(field_name)
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or not (min_value <= value <= max_value)
    ):
        message_label = (
            None
            if message is None
            else _require_validation_label(
                message,
                errors,
                label_name="validation message",
            )
        )
        if message is not None and message_label is None:
            return min_value
        errors.append(
            message_label
            or f"{diagnostic_label} must be an integer in {min_value}..={max_value}"
        )
        return min_value
    return value


def require_advancing_int_pair(
    payload: Mapping[str, Any],
    current_field: str,
    next_field: str,
    errors: list[str],
) -> tuple[int, int]:
    """Return non-negative/current and positive/next ints that strictly advance."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return 0, 0
    current_field_name = _require_validation_label(
        current_field,
        errors,
        label_name="validation field",
    )
    next_field_name = _require_validation_label(
        next_field,
        errors,
        label_name="validation field",
    )
    if current_field_name is None or next_field_name is None:
        return 0, 0
    current_value = require_non_negative_int(payload, current_field_name, errors)
    next_value = require_positive_int(payload, next_field_name, errors)
    current_raw = payload.get(current_field_name)
    next_raw = payload.get(next_field_name)
    current_valid = (
        isinstance(current_raw, int)
        and not isinstance(current_raw, bool)
        and current_raw >= 0
    )
    next_valid = (
        isinstance(next_raw, int) and not isinstance(next_raw, bool) and next_raw > 0
    )
    if current_valid and next_valid and next_value <= current_value:
        errors.append(f"{next_field_name} must advance past {current_field_name}")
    return current_value, next_value


def require_score_bps(payload: Mapping[str, Any], field: str, errors: list[str]) -> None:
    """Require an integer basis-point score in the inclusive 0..=10000 range."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return
    value = require_non_negative_int(payload, field_name, errors)
    if value > 10_000:
        errors.append(f"{field_name} must be <= 10000")


def require_2xx_status(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str | None = None,
) -> None:
    """Require an integer HTTP status in the inclusive 200..299 range."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return
    value = payload.get(field_name)
    if not isinstance(value, int) or isinstance(value, bool) or not (200 <= value < 300):
        errors.append(f"{diagnostic_label} must be a 2xx status")


def require_non_negative_number(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str | None = None,
) -> float:
    """Return a non-negative number field or append a validation error."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return 0.0
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return 0.0
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return 0.0
    value = payload.get(field_name)
    if (
        not isinstance(value, (int, float))
        or isinstance(value, bool)
        or not math.isfinite(value)
        or value < 0
    ):
        errors.append(f"{diagnostic_label} must be a non-negative number")
        return 0.0
    return float(value)


def require_maximum_number(
    payload: Mapping[str, Any],
    field: str,
    maximum: float,
    errors: list[str],
    *,
    path: str | None = None,
) -> float:
    """Return a non-negative number field and require it to meet a maximum."""

    error_count = len(errors)
    value = require_non_negative_number(payload, field, errors, path=path)
    if len(errors) != error_count:
        return value
    diagnostic_label = _require_validation_label(
        field if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return value
    if (
        not isinstance(maximum, (int, float))
        or isinstance(maximum, bool)
        or not math.isfinite(maximum)
        or maximum < 0
    ):
        errors.append(
            f"{diagnostic_label} maximum threshold must be a non-negative number"
        )
        return value
    if value > maximum:
        errors.append(f"{diagnostic_label} must be <= {maximum}")
    return value


def require_maximum_int(
    payload: Mapping[str, Any],
    field: str,
    maximum: int,
    errors: list[str],
    *,
    minimum: int = 0,
    path: str | None = None,
) -> int:
    """Return an integer field and require it to meet a maximum."""

    if not isinstance(minimum, int) or isinstance(minimum, bool):
        errors.append("validation minimum threshold must be an integer")
        return 0
    if not isinstance(maximum, int) or isinstance(maximum, bool):
        errors.append("validation maximum threshold must be an integer")
        return minimum
    if maximum < minimum:
        errors.append("validation maximum threshold must be >= minimum threshold")
        return minimum
    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return minimum
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    if field_name is None:
        return minimum
    diagnostic_label = _require_validation_label(
        field_name if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return minimum
    value = payload.get(field_name)
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        if minimum == 0:
            errors.append(f"{diagnostic_label} must be a non-negative integer")
        elif minimum == 1:
            errors.append(f"{diagnostic_label} must be a positive integer")
        else:
            errors.append(f"{diagnostic_label} must be an integer >= {minimum}")
        return minimum
    if value > maximum:
        errors.append(f"{diagnostic_label} must be <= {maximum}")
    return value


def require_count_equal(
    payload: Mapping[str, Any],
    total_field: str,
    passed_field: str,
    errors: list[str],
) -> int:
    """Require a positive total count and an equal passed count."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return 0
    total_field_name = _require_validation_label(
        total_field,
        errors,
        label_name="validation field",
    )
    if total_field_name is None:
        return 0
    passed_field_name = _require_validation_label(
        passed_field,
        errors,
        label_name="validation field",
    )
    if passed_field_name is None:
        return 0
    total = require_positive_int(payload, total_field_name, errors)
    passed = payload.get(passed_field_name)
    if not isinstance(passed, int) or isinstance(passed, bool) or passed != total:
        errors.append(f"{passed_field_name} must equal {total_field_name}")
    return total


def require_count_value_equal(
    payload: Mapping[str, Any],
    field: str,
    expected_count: int,
    expected_label: str,
    errors: list[str],
) -> None:
    """Require a count field to match an already validated positive count."""

    if (
        isinstance(expected_count, int)
        and not isinstance(expected_count, bool)
        and expected_count == 0
    ):
        return
    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    field_name = _require_validation_label(
        field,
        errors,
        label_name="validation field",
    )
    expected_count_label = _require_validation_label(
        expected_label,
        errors,
        label_name="validation count label",
    )
    if field_name is None or expected_count_label is None:
        return
    if (
        not isinstance(expected_count, int)
        or isinstance(expected_count, bool)
        or expected_count <= 0
    ):
        errors.append(f"{expected_count_label} must be a positive integer")
        return
    value = payload.get(field_name)
    if not isinstance(value, int) or isinstance(value, bool) or value != expected_count:
        errors.append(f"{field_name} must equal {expected_count_label}")


def require_count_match(
    payload: Mapping[str, Any],
    total_field: str,
    passed_field: str,
    errors: list[str],
) -> None:
    """Require a positive total count and matching passed count."""

    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    total_field_name = _require_validation_label(
        total_field,
        errors,
        label_name="validation field",
    )
    passed_field_name = _require_validation_label(
        passed_field,
        errors,
        label_name="validation field",
    )
    if total_field_name is None or passed_field_name is None:
        return
    total = require_positive_int(payload, total_field_name, errors)
    if total == 0:
        return
    passed = payload.get(passed_field_name)
    if not isinstance(passed, int) or isinstance(passed, bool) or passed != total:
        errors.append(f"{passed_field_name} must equal {total_field_name}")


def require_count_length_match(
    count: Any,
    records: Any,
    count_label: str,
    collection_label: str,
    errors: list[str],
) -> None:
    """Require a count value to match an already validated collection length."""

    count_name = _require_validation_label(
        count_label,
        errors,
        label_name="validation count label",
    )
    collection_name = _require_validation_label(
        collection_label,
        errors,
        label_name="validation collection label",
    )
    if count_name is None or collection_name is None:
        return
    if not isinstance(count, int) or isinstance(count, bool) or count < 0:
        errors.append(f"{count_name} must be a non-negative integer count")
        return
    if isinstance(records, (str, bytes, bytearray, Mapping)) or not isinstance(
        records, Sequence
    ):
        errors.append(f"{collection_name} must be a sequence")
        return
    if count != len(records):
        errors.append(f"{count_name} must equal {collection_name} length")


def require_sum_equal(
    total: int,
    parts: Sequence[tuple[str, int]],
    total_label: str,
    errors: list[str],
    *,
    skip_zero_total: bool = False,
) -> None:
    """Require two or more named part counts to sum to a total count."""

    if not isinstance(skip_zero_total, bool):
        errors.append("validation skip_zero_total must be a boolean")
        return
    if (
        skip_zero_total
        and isinstance(total, int)
        and not isinstance(total, bool)
        and total == 0
    ):
        return
    total_count_label = _require_validation_label(
        total_label,
        errors,
        label_name="validation total label",
    )
    if total_count_label is None:
        return
    if not isinstance(total, int) or isinstance(total, bool) or total < 0:
        errors.append(f"{total_count_label} must be a non-negative integer count")
        return
    if isinstance(parts, (str, bytes, bytearray, Mapping)) or not isinstance(
        parts, Sequence
    ):
        errors.append("validation sum parts must be a sequence of (label, count) pairs")
        return
    validated_parts: list[tuple[str, int]] = []
    for part in parts:
        if isinstance(part, (str, bytes, bytearray, Mapping)) or not isinstance(
            part, Sequence
        ):
            errors.append(
                "validation sum parts must be a sequence of (label, count) pairs"
            )
            return
        pair = tuple(part)
        if len(pair) != 2:
            errors.append(
                "validation sum parts must be a sequence of (label, count) pairs"
            )
            return
        label, value = pair
        count_name = _require_validation_label(
            label,
            errors,
            label_name="validation count label",
        )
        if count_name is None:
            return
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(
                f"{count_name} must be paired with a non-negative integer count"
            )
            return
        validated_parts.append((count_name, value))
    if sum(value for _label, value in validated_parts) != total:
        labels = " plus ".join(label for label, _value in validated_parts)
        errors.append(f"{labels} must equal {total_count_label}")


def require_recent_timestamp(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    now_unix: int,
    max_age_secs: int,
    path: str | None = None,
) -> int:
    """Require a positive timestamp that is not future-dated or stale."""

    generated_at = require_positive_int(payload, field, errors)
    if generated_at == 0:
        return 0
    diagnostic_label = _require_validation_label(
        field if path is None else path,
        errors,
        label_name="validation path",
    )
    if diagnostic_label is None:
        return 0
    if not isinstance(now_unix, int) or isinstance(now_unix, bool) or now_unix < 0:
        errors.append(
            f"{diagnostic_label} current time must be a non-negative integer timestamp"
        )
        return 0
    if (
        not isinstance(max_age_secs, int)
        or isinstance(max_age_secs, bool)
        or max_age_secs < 0
    ):
        errors.append(f"{diagnostic_label} maximum age must be a non-negative integer")
        return 0
    if generated_at > now_unix:
        errors.append(f"{diagnostic_label} must not be in the future")
    elif now_unix - generated_at > max_age_secs:
        errors.append(f"{diagnostic_label} is older than {max_age_secs} seconds")
    return generated_at


def collect_string_values(
    payload: Mapping[str, Any],
    array_field: str,
    field: str,
    *,
    allow_scalar_items: bool = True,
    trim_values: bool = True,
) -> set[str]:
    """Collect non-empty string values from an evidence array field."""

    values: set[str] = set()
    if not isinstance(payload, Mapping):
        return values
    items = payload.get(array_field)
    if not isinstance(items, list):
        return values
    for item in items:
        value = ""
        if isinstance(item, str) and allow_scalar_items and not field:
            if trim_values:
                value = item.strip()
            elif item.strip():
                value = item
        elif isinstance(item, Mapping):
            raw = item.get(field)
            if isinstance(raw, str):
                if trim_values:
                    value = raw.strip()
                elif raw.strip():
                    value = raw
        if value:
            values.add(value)
    return values


def _collect_canonical_string_values(
    payload: Mapping[str, Any],
    array_field: str,
    field: str,
    errors: list[str],
    *,
    allow_scalar_items: bool = True,
) -> set[str]:
    """Collect canonical string values from an evidence array field."""

    values: set[str] = set()
    items = payload.get(array_field)
    if not isinstance(items, list):
        return values
    value_label = field or "value"
    for index, item in enumerate(items):
        raw: Any = None
        if isinstance(item, str) and allow_scalar_items and not field:
            raw = item
        elif isinstance(item, Mapping):
            if not field:
                errors.append(f"{array_field}[{index}] must be a string")
                continue
            raw = item.get(field)
        else:
            if field:
                errors.append(f"{array_field}[{index}] must be an object with `{field}`")
            else:
                errors.append(f"{array_field}[{index}] must be a string")
            continue
        value = _require_validation_label(
            raw,
            errors,
            label_name=f"validation {value_label}",
        )
        if value is not None:
            values.add(value)
    return values


def _required_string_values(required_values: Any) -> tuple[str, ...] | None:
    """Return required string coverage labels or reject malformed containers."""

    if isinstance(
        required_values, (str, bytes, bytearray, Mapping)
    ) or not isinstance(required_values, Sequence):
        return None
    if not all(isinstance(required, str) and required for required in required_values):
        return None
    return tuple(required_values)


def require_string_coverage(
    payload: Mapping[str, Any],
    array_field: str,
    field: str,
    required_values: Any,
    errors: list[str],
    *,
    allow_scalar_items: bool = True,
    trim_values: bool = True,
) -> None:
    """Append an error for each required string value missing from evidence."""

    if not isinstance(allow_scalar_items, bool):
        errors.append("validation allow_scalar_items must be a boolean")
        return
    if not isinstance(trim_values, bool):
        errors.append("validation trim_values must be a boolean")
        return
    if not isinstance(payload, Mapping):
        errors.append("payload must be an object")
        return
    array_name = _require_validation_label(
        array_field,
        errors,
        label_name="validation array field",
    )
    if array_name is None:
        return
    if field:
        field_name = _require_validation_label(
            field,
            errors,
            label_name="validation field",
        )
        if field_name is None:
            return
    else:
        field_name = ""
    present = _collect_canonical_string_values(
        payload,
        array_name,
        field_name,
        errors,
        allow_scalar_items=allow_scalar_items,
    )
    _ = trim_values
    required_strings = _required_string_values(required_values)
    if required_strings is None:
        errors.append(f"{array_name} required values must be a sequence of strings")
        return
    required_labels: list[str] = []
    for required in required_strings:
        required_label = _require_validation_label(
            required,
            errors,
            label_name="validation required value",
        )
        if required_label is None:
            return
        required_labels.append(required_label)
    value_label = field_name or "value"
    for required in required_labels:
        if required not in present:
            errors.append(f"{array_name} must include {value_label} `{required}`")
