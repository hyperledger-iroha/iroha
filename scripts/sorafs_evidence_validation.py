"""Shared validation helpers for SoraFS evidence gates."""

from __future__ import annotations

import math
import re
from collections.abc import Callable, Collection, Hashable, Mapping, Sequence
from pathlib import Path
from typing import Any, TypeVar

from sorafs_checker_preflight import record_artifact_error
from sorafs_evidence_fingerprint import artifact_fingerprint
from sorafs_evidence_paths import is_explicit_evidence_path
from sorafs_evidence_sensitivity import visit_sensitive_fields


_T = TypeVar("_T")


def require_object(value: Any, path: str, errors: list[str]) -> dict[str, Any]:
    """Return an object value or append a path-qualified validation error."""

    if isinstance(value, dict):
        return value
    errors.append(f"{path} must be an object")
    return {}


def require_object_array(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
) -> list[tuple[int, dict[str, Any]]]:
    """Return indexed object records from a required non-empty array field."""

    items = payload.get(field)
    if not isinstance(items, list) or not items:
        errors.append(f"{field} must be a non-empty array")
        return []
    return [
        (index, require_object(item, f"{field}[{index}]", errors))
        for index, item in enumerate(items)
    ]


def require_string(payload: Mapping[str, Any], field: str, errors: list[str]) -> str:
    """Return a stripped non-empty string field or append a validation error."""

    value = payload.get(field)
    if isinstance(value, str) and value.strip():
        return value.strip()
    errors.append(f"{field} must be a non-empty string")
    return ""


def require_string_type(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str | None = None,
) -> str | None:
    """Return a string field without trimming or append a validation error."""

    value = payload.get(field)
    if not isinstance(value, str):
        errors.append(f"{path or field} must be a string")
        return None
    return value


def require_known_schema(
    payload: Mapping[str, Any],
    schema_to_kind: Mapping[str, _T],
    artifact_label: str,
    errors: list[str],
) -> _T | None:
    """Return the schema kind for a string schema or append validation errors."""

    schema = require_string_type(payload, "schema", errors)
    if schema is None:
        return None
    kind = schema_to_kind.get(schema)
    if kind is None:
        errors.append(f"schema `{schema}` is not a recognized {artifact_label}")
        return None
    return kind


def validate_standard_evidence_payload(
    payload: dict[str, Any],
    schema_to_kind: Mapping[str, _T],
    artifact_label: str,
    sensitive_keys: Collection[str],
    evidence_label: str,
    validate_kind: Callable[[_T, dict[str, Any], list[str]], None],
    *,
    require_reviewed_deployment_context: bool = False,
) -> tuple[str | None, list[str]]:
    """Validate the standard rollout/release evidence payload wrapper."""

    errors: list[str] = []
    kind = require_known_schema(payload, schema_to_kind, artifact_label, errors)
    if kind is None:
        return None, errors
    if require_reviewed_deployment_context:
        require_rollout_deployment_id(payload, errors)
        require_rollout_environment(payload, errors)
    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys=sensitive_keys,
        evidence_label=evidence_label,
    )
    validate_kind(kind, payload, errors)
    return kind.name, errors


def build_required_evidence_summary(
    required_kinds: Sequence[str],
    artifacts_by_kind: Mapping[str, Sequence[dict[str, Any]]],
    schema_by_kind: Mapping[str, str],
    errors: list[str],
    *,
    evidence_label: str,
) -> dict[str, dict[str, Any]]:
    """Build required-kind summary rows and append missing/invalid errors."""

    required: dict[str, dict[str, Any]] = {}
    for name in required_kinds:
        artifacts = list(artifacts_by_kind.get(name, []))
        present = bool(artifacts)
        valid = present and all(
            evidence_artifact_is_valid(artifact) for artifact in artifacts
        )
        required[name] = {
            "schema": schema_by_kind[name],
            "present": present,
            "valid": valid,
            "artifact_count": len(artifacts),
            "artifacts": artifacts,
        }
        if not present:
            errors.append(f"missing required {name} {evidence_label} evidence")
        elif not valid:
            errors.append(f"{name} {evidence_label} evidence has invalid artifact(s)")
    return required


def mark_required_evidence_invalid(
    required: dict[str, dict[str, Any]],
    kind_name: str,
) -> list[str]:
    """Mark a required evidence summary row invalid and return its errors list."""

    row = required.setdefault(
        kind_name,
        {"valid": False, "errors": [], "artifacts": []},
    )
    row["valid"] = False
    errors = row.get("errors")
    if not isinstance(errors, list):
        errors = []
        row["errors"] = errors
    return errors


def mark_required_evidence_summary_invalid(
    required: dict[str, dict[str, Any]],
) -> None:
    """Mark every required evidence summary row invalid."""

    for kind_name in list(required):
        mark_required_evidence_invalid(required, kind_name)


def mark_required_evidence_invalid_if_present(
    required: dict[str, dict[str, Any]],
    kind_name: str | None,
) -> list[str]:
    """Mark an existing required evidence summary row invalid."""

    if kind_name is None or kind_name not in required:
        return []
    return mark_required_evidence_invalid(required, kind_name)


def required_evidence_summary_is_valid(
    required: Mapping[str, Mapping[str, Any]],
) -> bool:
    """Return whether every required evidence summary row is explicitly valid."""

    return all(row.get("valid") is True for row in required.values())


def required_evidence_has_any_kind(
    required_kinds: Collection[str],
    candidate_kinds: Collection[str],
) -> bool:
    """Return whether required evidence includes any candidate kind."""

    required_kind_names = set(required_kinds)
    return any(kind in required_kind_names for kind in candidate_kinds)


def required_evidence_has_all_kinds(
    required_kinds: Collection[str],
    candidate_kinds: Collection[str],
) -> bool:
    """Return whether required evidence includes every candidate kind."""

    required_kind_names = set(required_kinds)
    return all(kind in required_kind_names for kind in candidate_kinds)


def hashable_evidence_values(values: Collection[Any]) -> set[Hashable]:
    """Return truthy hashable evidence values from an observed-value collection."""

    return {value for value in values if value and isinstance(value, Hashable)}


def missing_required_evidence_values(
    required_values: Sequence[Any],
    observed_values: Collection[Any],
) -> list[Any]:
    """Return required evidence values that are absent from observed values."""

    observed = hashable_evidence_values(observed_values)
    return [
        value
        for value in required_values
        if not isinstance(value, Hashable) or value not in observed
    ]


def record_missing_required_evidence_value_errors(
    required: dict[str, dict[str, Any]],
    kind_name: str,
    required_values: Sequence[Any],
    observed_values: Collection[Any],
    message_for_value: Callable[[Any], str],
) -> list[Any]:
    """Record row errors for required values absent from observed evidence."""

    missing_values = missing_required_evidence_values(required_values, observed_values)
    if not missing_values:
        return []
    errors = mark_required_evidence_invalid(required, kind_name)
    for value in missing_values:
        errors.append(message_for_value(value))
    return missing_values


def required_or_observed_evidence_values_are_present(
    required_values: Collection[Any],
    observed_values: Collection[Any],
) -> bool:
    """Return whether evidence values are either required or observed."""

    return bool(hashable_evidence_values(required_values)) or bool(
        hashable_evidence_values(observed_values)
    )


def record_missing_required_or_observed_evidence_error(
    required: dict[str, dict[str, Any]],
    kind_name: str,
    required_values: Collection[Any],
    observed_values: Collection[Any],
    error: str,
) -> bool:
    """Record a row error when neither required nor observed values exist."""

    if required_or_observed_evidence_values_are_present(
        required_values,
        observed_values,
    ):
        return False
    mark_required_evidence_invalid(required, kind_name).append(error)
    return True


def distinct_evidence_values_are_consistent(values: Collection[Any]) -> bool:
    """Return whether evidence values contain at most one distinct value."""

    return len(values) <= 1


def record_inconsistent_evidence_values_error(
    required: dict[str, dict[str, Any]],
    values: Collection[Any],
    kind_name: str,
    error: str,
) -> bool:
    """Record a summary-wide error when evidence values disagree."""

    if distinct_evidence_values_are_consistent(values):
        return False
    mark_required_evidence_summary_invalid(required)
    mark_required_evidence_invalid(required, kind_name).append(error)
    return True


def record_consistent_evidence_value(
    values: dict[str, str],
    key: str,
    value: Any,
    context: str,
    errors: list[str],
) -> None:
    """Record a non-empty evidence value and report cross-artifact mismatches."""

    if not isinstance(value, str) or not value:
        return
    previous = values.get(key)
    if previous is None:
        values[key] = value
    elif previous != value:
        errors.append(f"{context}.{key} `{value}` does not match `{previous}`")


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

    if not valid:
        return
    if kind_name in anchor_kinds:
        if (
            isinstance(snapshot_id, str)
            and snapshot_id
            and isinstance(merkle_root, str)
            and merkle_root
        ):
            valid_snapshot_bindings.add((snapshot_id.lower(), merkle_root.lower()))
        return
    if kind_name in bound_kinds:
        snapshot_bound_artifacts.append(artifact)


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

    if valid_snapshot_bindings:
        for artifact in snapshot_bound_artifacts:
            fingerprint = evidence_artifact_fingerprint(artifact)
            snapshot_id = fingerprint.get("snapshot_id_hex")
            merkle_root = fingerprint.get("merkle_root_hex")
            binding_errors: list[str] = []
            require_string_tuple_in(
                (snapshot_id, merkle_root),
                valid_snapshot_bindings,
                binding_errors,
                message=binding_error,
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
                    summary_error=binding_summary_error,
                )
    elif snapshot_bound_artifacts:
        for artifact in snapshot_bound_artifacts:
            summary_errors = mark_required_evidence_invalid_if_present(
                required,
                evidence_artifact_kind(artifact),
            )
            record_artifact_error(
                artifact,
                missing_anchor_error,
                summary_errors,
                summary_error=missing_anchor_summary_error,
            )

    if not valid_snapshot_bindings and required_evidence_has_any_kind(
        required_kinds,
        bound_kinds,
    ):
        mark_required_evidence_invalid(required, required_anchor_kind).append(
            missing_required_anchor_error
        )


def finalize_custom_required_evidence_rows(
    required: dict[str, dict[str, Any]],
    *,
    evidence_label: str,
) -> None:
    """Finalize custom required evidence rows with fail-closed validity."""

    for kind, row in required.items():
        artifacts = row.get("artifacts")
        if not isinstance(artifacts, Sequence) or isinstance(
            artifacts, (str, bytes, bytearray)
        ):
            row["artifacts"] = []
            mark_required_evidence_invalid(required, kind).append(
                f"missing required `{kind}` {evidence_label}"
            )
            continue
        if not artifacts:
            mark_required_evidence_invalid(required, kind).append(
                f"missing required `{kind}` {evidence_label}"
            )
        else:
            row["valid"] = recognized_evidence_artifacts_are_valid(artifacts)


def record_custom_required_evidence_artifact(
    required: dict[str, dict[str, Any]],
    kind_name: str,
    artifact: dict[str, Any],
    errors: Sequence[str],
) -> None:
    """Record an artifact into a custom required evidence row if present."""

    row = required.get(kind_name)
    if row is None:
        return

    artifacts = row.get("artifacts")
    if not isinstance(artifacts, list):
        artifacts = []
        row["artifacts"] = artifacts
    artifacts.append(artifact)

    row_errors = row.get("errors")
    if not isinstance(row_errors, list):
        row_errors = []
        row["errors"] = row_errors
    row_errors.extend(errors)


def required_evidence_kind_names(required_kinds: Sequence[str]) -> list[str]:
    """Return the standard summary list of required evidence kind names."""

    return list(required_kinds)


def evidence_schema_by_kind(kind_by_name: Mapping[str, Any]) -> dict[str, str]:
    """Return the standard evidence schema lookup keyed by kind name."""

    return {name: kind.schema for name, kind in kind_by_name.items()}


def init_evidence_artifact_buckets(
    evidence_kind_names: Sequence[str],
) -> dict[str, list[dict[str, Any]]]:
    """Return empty artifact buckets keyed by evidence kind name."""

    return {name: [] for name in evidence_kind_names}


def build_evidence_artifact(
    path: Any,
    digest: str,
    payload: Mapping[str, Any],
    validation_errors: list[str],
    fingerprint_fields: Sequence[str],
) -> dict[str, Any]:
    """Build the standard payload-free artifact row for evidence summaries."""

    return {
        "path": str(path),
        "sha256": digest,
        "schema": payload.get("schema"),
        "status": payload.get("status"),
        "fingerprint": artifact_fingerprint(payload, fingerprint_fields),
        "valid": not validation_errors,
        "errors": validation_errors,
    }


def build_kinded_evidence_artifact(
    *,
    kind_name: str,
    path: Any,
    digest: str,
    payload: Mapping[str, Any],
    validation_errors: list[str],
    fingerprint_fields: Sequence[str],
    fingerprint_values: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a payload-free artifact row keyed by evidence kind."""

    fingerprint = artifact_fingerprint(payload, fingerprint_fields)
    if fingerprint_values:
        fingerprint.update(fingerprint_values)
    return {
        "kind": kind_name,
        "path": str(path),
        "sha256": digest,
        "fingerprint": fingerprint,
        "valid": not validation_errors,
        "errors": validation_errors,
    }


def record_evidence_artifact(
    artifacts_by_kind: dict[str, list[dict[str, Any]]],
    kind_name: str,
    artifact: dict[str, Any],
) -> None:
    """Append a recognized evidence artifact to its kind bucket."""

    artifacts_by_kind[kind_name].append(artifact)


def evidence_artifact_is_valid(artifact: Mapping[str, Any]) -> bool:
    """Return whether an evidence artifact is explicitly marked valid."""

    return artifact.get("valid") is True


def evidence_artifact_kind(artifact: Mapping[str, Any]) -> str | None:
    """Return an artifact kind name when it is a string."""

    kind = artifact.get("kind")
    if isinstance(kind, str):
        return kind
    return None


def evidence_artifact_fingerprint(artifact: Mapping[str, Any]) -> Mapping[str, Any]:
    """Return an artifact fingerprint mapping or an empty mapping."""

    fingerprint = artifact.get("fingerprint")
    if isinstance(fingerprint, Mapping):
        return fingerprint
    return {}


def evidence_artifact_detail(
    artifact: Mapping[str, Any],
    field: str,
) -> Mapping[str, Any]:
    """Return an artifact detail mapping or an empty mapping."""

    detail = artifact.get(field)
    if isinstance(detail, Mapping):
        return detail
    return {}


def evidence_artifact_schema(artifact: Mapping[str, Any]) -> str:
    """Return an artifact schema label for diagnostics."""

    schema = artifact.get("schema")
    if isinstance(schema, str) and schema:
        return schema
    return "<unknown>"


def count_evidence_artifacts(
    artifacts_by_kind: Mapping[str, Sequence[dict[str, Any]]],
) -> int:
    """Return the total number of recognized evidence artifacts."""

    return sum(len(artifacts) for artifacts in artifacts_by_kind.values())


def count_recognized_evidence_artifacts(
    recognized: Sequence[Mapping[str, Any]],
) -> int:
    """Return the total number of recognized evidence artifact rows."""

    return len(recognized)


def recognized_evidence_artifacts_are_valid(
    recognized: Sequence[Any],
) -> bool:
    """Return whether every recognized evidence artifact row is valid."""

    return all(
        isinstance(artifact, Mapping) and evidence_artifact_is_valid(artifact)
        for artifact in recognized
    )


def count_evidence_files(files: Sequence[Any]) -> int:
    """Return the total number of discovered evidence files."""

    return len(files)


def evidence_gate_status(errors: Sequence[str]) -> str:
    """Return the standard gate status for a summary error list."""

    return "blocked" if errors else "ready"


def record_evidence_validation_errors(
    path: Any,
    validation_errors: Sequence[str],
    errors: list[str],
) -> None:
    """Append path-qualified payload validation errors to the summary errors."""

    errors.extend(f"{path}: {error}" for error in validation_errors)


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

    if payload.get(field) != expected:
        expected_label = f"`{expected}`" if quote_expected else expected
        errors.append(f"{path or field} must be {expected_label}")


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
    if value == disallowed:
        errors.append(message or f"{field} must not be `{disallowed}`")
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

    if not isinstance(value, str) or not value:
        errors.append(f"{label} must be a non-empty string")
        return ""
    if not isinstance(expected, str) or not expected:
        errors.append(f"{expected_label} must be a non-empty string")
        return value
    if value and expected and value != expected:
        errors.append(message or f"{label} must match {expected_label}")
    return value


def require_string_value_in(
    value: Any,
    allowed: Collection[str],
    errors: list[str],
    *,
    message: str,
) -> str | None:
    """Return a normalized string value that belongs to an allowed set."""

    if not isinstance(value, str) or not value:
        errors.append(message)
        return None
    normalized = value.lower()
    if normalized not in allowed:
        errors.append(message)
        return None
    return normalized


def require_string_tuple_in(
    values: Sequence[Any],
    allowed: Collection[tuple[str, ...]],
    errors: list[str],
    *,
    message: str,
) -> tuple[str, ...] | None:
    """Return a normalized string tuple that belongs to an allowed set."""

    if not all(isinstance(value, str) and value for value in values):
        errors.append(message)
        return None
    binding = tuple(value.lower() for value in values)
    if binding not in allowed:
        errors.append(message)
        return None
    return binding


def record_string_value_binding_errors(
    artifact: dict[str, Any],
    value: Any,
    allowed: Collection[str],
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


def evidence_artifact_digest_set(
    artifacts: Sequence[dict[str, Any]],
    digest_field: str,
) -> set[str]:
    """Return normalized digest values from valid evidence artifacts."""

    values: set[str] = set()
    for artifact in artifacts:
        if not evidence_artifact_is_valid(artifact):
            continue
        digest = evidence_artifact_fingerprint(artifact).get(digest_field)
        if isinstance(digest, str) and digest:
            values.add(digest.lower())
    return values


def record_evidence_digest_mismatch_errors(
    *,
    artifacts: Sequence[dict[str, Any]],
    digest_field: str,
    allowed_digests: Collection[str],
    errors: list[str],
    error: str,
) -> None:
    """Record artifact errors for digest values outside the allowed set."""

    for artifact in artifacts:
        if not evidence_artifact_is_valid(artifact):
            continue
        digest = evidence_artifact_fingerprint(artifact).get(digest_field)
        record_string_value_binding_errors(
            artifact,
            digest,
            allowed_digests,
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

    if valid_anchor_digests:
        for kind_name, artifact in bound_artifacts:
            field = (digest_field_by_kind or {}).get(kind_name, digest_field)
            digest = (
                evidence_artifact_fingerprint(artifact).get(field)
                if field is not None
                else None
            )
            record_string_value_binding_errors(
                artifact,
                digest,
                valid_anchor_digests,
                errors,
                message=binding_error_template.format(kind_name=kind_name),
            )
        return
    if not required_evidence_has_any_kind(
        required_kinds, missing_anchor_required_kinds
    ):
        return
    for kind_name, artifact in missing_anchor_artifacts or bound_artifacts:
        record_artifact_error(
            artifact,
            missing_anchor_error_template.format(kind_name=kind_name),
            errors,
        )
    if missing_anchor_summary_error is not None:
        errors.append(missing_anchor_summary_error)


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

    if valid_anchor_bindings:
        for kind_name, artifact in bound_artifacts:
            fingerprint = evidence_artifact_fingerprint(artifact)
            record_string_tuple_binding_errors(
                artifact,
                tuple(fingerprint.get(field) for field in binding_fields),
                valid_anchor_bindings,
                errors,
                message=binding_error_template.format(kind_name=kind_name),
            )
        return
    if not required_evidence_has_any_kind(
        required_kinds, missing_anchor_required_kinds
    ):
        return
    for kind_name, artifact in missing_anchor_artifacts or bound_artifacts:
        record_artifact_error(
            artifact,
            missing_anchor_error_template.format(kind_name=kind_name),
            errors,
        )
    if missing_anchor_summary_error is not None:
        errors.append(missing_anchor_summary_error)


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
    allowed: Sequence[str],
    errors: list[str],
    *,
    path: str | None = None,
    quote_values: bool = True,
) -> str:
    """Return a string field that belongs to an allowed set or append an error."""

    value = require_string(payload, field, errors)
    if not value:
        return ""
    if value in allowed:
        return value
    labels = [
        f"`{allowed_value}`" if quote_values else allowed_value
        for allowed_value in allowed
    ]
    if len(labels) == 1:
        allowed_label = labels[0]
    else:
        allowed_label = ", ".join(labels[:-1]) + f" or {labels[-1]}"
    errors.append(f"{path or field} must be {allowed_label}")
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


def require_rollout_deployment_id(
    payload: Mapping[str, Any],
    errors: list[str],
    *,
    field: str = "deployment_id",
) -> str:
    """Return a reviewed deployment id or append a validation error."""

    value = require_string(payload, field, errors)
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

    value = require_string(payload, field, errors)
    if not value:
        return ""
    normalized = value.lower()
    if normalized not in ALLOWED_ROLLOUT_ENVIRONMENTS:
        errors.append(
            f"{field} must be one of {sorted(ALLOWED_ROLLOUT_ENVIRONMENTS)}"
        )
        return ""
    return normalized


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
    allowed: tuple[str, ...],
    errors: list[str],
    *,
    field: str = "status",
    path: str = "status",
    allow_absent: bool = False,
) -> str:
    """Require a status field to match one of the allowed status labels."""

    value = payload.get(field)
    if value is None and allow_absent:
        return ""
    allowed_label = "/".join(allowed)
    if value not in allowed:
        suffix = f"{allowed_label} when present" if allow_absent else allowed_label
        errors.append(f"{path} must be {suffix}")
        return ""
    return str(value)


def is_hex(value: str, length: int) -> bool:
    """Return whether a string is exactly the requested hexadecimal length."""

    return len(value) == length and all(
        char in "0123456789abcdefABCDEF" for char in value
    )


def require_hex(
    payload: Mapping[str, Any],
    field: str,
    length: int,
    errors: list[str],
) -> str:
    """Return a lowercase hex string field or append a validation error."""

    value = require_string(payload, field, errors)
    if value and not is_hex(value, length):
        errors.append(f"{field} must be {length} hex characters")
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

    value = payload.get(field)
    if value is None:
        return
    if not isinstance(value, str) or not is_hex(value, length):
        errors.append(f"{field} must be null or {length} hex characters")


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

    label = path or field
    values = payload.get(field)
    if not isinstance(values, list) or (non_empty and not values):
        expected = "a non-empty array" if non_empty else "an array"
        errors.append(f"{label} must be {expected}")
        return []
    if expected_length is not None and len(values) != expected_length:
        expected_label = expected_length_label or str(expected_length)
        errors.append(f"{label} length must equal {expected_label}")

    normalized_values: list[str] = []
    seen: set[str] = set()
    for index, value in enumerate(values):
        if not isinstance(value, str) or not is_hex(value, length):
            errors.append(f"{label}[{index}] must be {length} hex characters")
            continue
        normalized = value.lower()
        if unique and normalized in seen:
            errors.append(f"{label}[{index}] must be unique")
        seen.add(normalized)
        normalized_values.append(normalized)
    return normalized_values


def require_bool_true(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str | None = None,
) -> None:
    """Require a field to be exactly true."""

    if payload.get(field) is not True:
        errors.append(f"{path or field} must be true")


def require_governance_approval(
    payload: Mapping[str, Any],
    errors: list[str],
) -> None:
    """Require rollout governance approval to be accepted and recorded."""

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

    if bound_field is not None:
        require_bool_true(payload, bound_field, errors)
    if source_field is not None:
        require_string_equal(payload, source_field, "iroha_config", errors)


def require_config_backed_governance_approval(
    payload: Mapping[str, Any],
    errors: list[str],
) -> None:
    """Require governance approval evidence to be accepted and config-backed."""

    require_governance_approval(payload, errors)
    require_iroha_config_binding(payload, errors)


def require_false(payload: Mapping[str, Any], field: str, errors: list[str]) -> None:
    """Require a field to be exactly false."""

    if payload.get(field) is not False:
        errors.append(f"{field} must be false")


def require_false_or_absent(
    payload: Mapping[str, Any], field: str, errors: list[str]
) -> None:
    """Require a field to be absent/null or exactly false."""

    value = payload.get(field)
    if value is not None and value is not False:
        errors.append(f"{field} must be false when present")


def require_false_or_governed(
    payload: Mapping[str, Any],
    field: str,
    governed_field: str,
    errors: list[str],
) -> None:
    """Require a field to be exactly false unless exact true is governed."""

    value = payload.get(field)
    if value is False:
        return
    if value is True:
        require_bool_true(payload, governed_field, errors)
        return
    errors.append(f"{field} must be false or explicitly governed")


def require_positive_int(
    payload: Mapping[str, Any], field: str, errors: list[str]
) -> int:
    """Return a positive integer field or append a validation error."""

    value = payload.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        errors.append(f"{field} must be a positive integer")
        return 0
    return value


def require_minimum_int(
    payload: Mapping[str, Any],
    field: str,
    minimum: int,
    errors: list[str],
) -> int:
    """Return a positive integer field and require it to meet a minimum."""

    value = require_positive_int(payload, field, errors)
    require_minimum_value(value, field, minimum, errors)
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

    if value < minimum:
        errors.append(message or f"{label} must be at least {minimum}")
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

    if value > maximum:
        errors.append(message or f"{label} must be <= {maximum}")
    return value


def require_non_negative_int(
    payload: Mapping[str, Any], field: str, errors: list[str]
) -> int:
    """Return a non-negative integer field or append a validation error."""

    value = payload.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        errors.append(f"{field} must be a non-negative integer")
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

    value = payload.get(field)
    label = path or field
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or not (min_value <= value <= max_value)
    ):
        errors.append(
            message or f"{label} must be an integer in {min_value}..={max_value}"
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

    current_value = require_non_negative_int(payload, current_field, errors)
    next_value = require_positive_int(payload, next_field, errors)
    current_raw = payload.get(current_field)
    next_raw = payload.get(next_field)
    current_valid = (
        isinstance(current_raw, int)
        and not isinstance(current_raw, bool)
        and current_raw >= 0
    )
    next_valid = (
        isinstance(next_raw, int) and not isinstance(next_raw, bool) and next_raw > 0
    )
    if current_valid and next_valid and next_value <= current_value:
        errors.append(f"{next_field} must advance past {current_field}")
    return current_value, next_value


def require_score_bps(payload: Mapping[str, Any], field: str, errors: list[str]) -> None:
    """Require an integer basis-point score in the inclusive 0..=10000 range."""

    value = require_non_negative_int(payload, field, errors)
    if value > 10_000:
        errors.append(f"{field} must be <= 10000")


def require_2xx_status(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str | None = None,
) -> None:
    """Require an integer HTTP status in the inclusive 200..299 range."""

    value = payload.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or not (200 <= value < 300):
        errors.append(f"{path or field} must be a 2xx status")


def require_non_negative_number(
    payload: Mapping[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str | None = None,
) -> float:
    """Return a non-negative number field or append a validation error."""

    value = payload.get(field)
    if (
        not isinstance(value, (int, float))
        or isinstance(value, bool)
        or not math.isfinite(value)
        or value < 0
    ):
        errors.append(f"{path or field} must be a non-negative number")
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

    value = require_non_negative_number(payload, field, errors, path=path)
    if value > maximum:
        errors.append(f"{path or field} must be <= {maximum}")
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

    value = payload.get(field)
    label = path or field
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        if minimum == 0:
            errors.append(f"{label} must be a non-negative integer")
        elif minimum == 1:
            errors.append(f"{label} must be a positive integer")
        else:
            errors.append(f"{label} must be an integer >= {minimum}")
        return minimum
    if value > maximum:
        errors.append(f"{label} must be <= {maximum}")
    return value


def require_count_equal(
    payload: Mapping[str, Any],
    total_field: str,
    passed_field: str,
    errors: list[str],
) -> int:
    """Require a positive total count and an equal passed count."""

    total = require_positive_int(payload, total_field, errors)
    if payload.get(passed_field) != total:
        errors.append(f"{passed_field} must equal {total_field}")
    return total


def require_count_value_equal(
    payload: Mapping[str, Any],
    field: str,
    expected_count: int,
    expected_label: str,
    errors: list[str],
) -> None:
    """Require a count field to match an already validated positive count."""

    if expected_count == 0:
        return
    if payload.get(field) != expected_count:
        errors.append(f"{field} must equal {expected_label}")


def require_count_match(
    payload: Mapping[str, Any],
    total_field: str,
    passed_field: str,
    errors: list[str],
) -> None:
    """Require a positive total count and matching passed count."""

    total = require_positive_int(payload, total_field, errors)
    if total == 0:
        return
    if payload.get(passed_field) != total:
        errors.append(f"{passed_field} must equal {total_field}")


def require_count_length_match(
    count: Any,
    records: Sequence[Any],
    count_label: str,
    collection_label: str,
    errors: list[str],
) -> None:
    """Require a count value to match an already validated collection length."""

    if count != len(records):
        errors.append(f"{count_label} must equal {collection_label} length")


def require_sum_equal(
    total: int,
    parts: Sequence[tuple[str, int]],
    total_label: str,
    errors: list[str],
    *,
    skip_zero_total: bool = False,
) -> None:
    """Require two or more named part counts to sum to a total count."""

    if skip_zero_total and total == 0:
        return
    if sum(value for _label, value in parts) != total:
        labels = " plus ".join(label for label, _value in parts)
        errors.append(f"{labels} must equal {total_label}")


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
    label = path or field
    if generated_at > now_unix:
        errors.append(f"{label} must not be in the future")
    elif now_unix - generated_at > max_age_secs:
        errors.append(f"{label} is older than {max_age_secs} seconds")
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


def require_string_coverage(
    payload: Mapping[str, Any],
    array_field: str,
    field: str,
    required_values: Sequence[str],
    errors: list[str],
    *,
    allow_scalar_items: bool = True,
    trim_values: bool = True,
) -> None:
    """Append an error for each required string value missing from evidence."""

    present = collect_string_values(
        payload,
        array_field,
        field,
        allow_scalar_items=allow_scalar_items,
        trim_values=trim_values,
    )
    value_label = field or "value"
    for required in required_values:
        if required not in present:
            errors.append(f"{array_field} must include {value_label} `{required}`")
