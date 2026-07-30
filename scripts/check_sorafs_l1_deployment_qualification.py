#!/usr/bin/env python3
"""Validate the pre-deployment SoraFS L1 topology contract.

This checker qualifies only a non-secret deployment plan. It never accepts or
emits rollout evidence and its success cannot promote a deployment.
"""

from __future__ import annotations

import argparse
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_production_readiness import (  # noqa: E402
    DEFAULT_REQUIRED_GATES,
    canonical_string,
    is_production_ready_environment,
    require_production_deployment_id_value,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    emit_checker_notice,
    render_and_write_checker_summary,
    validate_checker_preflight,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_sensitivity import visit_sensitive_fields  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
)
from sorafs_topology_qualification import (  # noqa: E402
    MANIFEST_SCHEMA,
    SUMMARY_SCHEMA,
    canonical_manifest_sha256,
)


MAX_MANIFEST_BYTES = 256 * 1024
EXPECTED_VALIDATOR_COUNT = 4
MIN_PROVIDER_COUNT = 2
MAX_PROVIDER_COUNT = 64
EXPECTED_GATEWAY_COUNT = 2
EXPECTED_GOVERNANCE_DAG_COUNT = 2
EXPECTED_LANE_SLOT_COUNT = 17
MIN_SIGNED_MODEL_ARTIFACT_COUNT = 1
MAX_SIGNED_MODEL_ARTIFACT_COUNT = 64
MAX_SIGNED_MODEL_REVISION = (1 << 63) - 1
RUNTIME_HANDLE_KINDS = ("monitoring", "hsm", "kms", "webauthn")
SIGNED_MODEL_SIGNATURE_ALGORITHMS = ("ed25519", "ml-dsa-87")
IDENTIFIER_PATTERN = re.compile(r"^[a-z0-9]+(?:[.-][a-z0-9]+)*\Z")
NON_PRODUCTION_HANDLE_COMPONENTS = frozenset(
    {"null", "mock", "test", "dev", "fake", "placeholder"}
)
RUNTIME_HANDLE_COMPACT_ROLES = frozenset(
    {
        "adapter",
        "hsm",
        "kms",
        "kubo",
        "monitoring",
        "provider",
        "runtime",
        "service",
        "signer",
        "webauthn",
    }
)
MODEL_ARTIFACT_COMPACT_ROLES = frozenset(
    {
        "ai",
        "artifact",
        "classifier",
        "model",
        "moderation",
        "prescreen",
        "signer",
    }
)

MANIFEST_FIELDS = frozenset(
    {
        "schema",
        "deployment",
        "validators",
        "storage_providers",
        "gateways",
        "governance_dag_instances",
        "runtime_handles",
        "runtime_material_policy",
        "signed_model_artifacts",
        "lane_slots",
    }
)
DEPLOYMENT_FIELDS = frozenset({"deployment_id", "environment"})
VALIDATOR_FIELDS = frozenset(
    {"validator_id", "voting", "da_enabled", "rbc_enabled"}
)
PROVIDER_FIELDS = frozenset({"provider_id", "operator_id"})
GATEWAY_FIELDS = frozenset({"gateway_id", "region", "administrator_id"})
GOVERNANCE_DAG_FIELDS = frozenset(
    {"instance_id", "kubo_handle", "administrator_id"}
)
RUNTIME_HANDLE_FIELDS = frozenset(RUNTIME_HANDLE_KINDS)
RUNTIME_MATERIAL_POLICY_FIELDS = frozenset(
    {
        "configuration_contains_credentials",
        "configuration_contains_private_material",
        "external_injection_required",
    }
)
SIGNED_MODEL_ARTIFACT_FIELDS = frozenset(
    {
        "artifact_id",
        "revision",
        "artifact_sha256",
        "signature_algorithm",
        "signature_sha256",
        "signer_public_key_fingerprint_sha256",
        "signature_verified",
    }
)
LANE_SLOT_FIELDS = frozenset({"gate", "deployment_id", "environment"})


def _closed_object(
    value: Any,
    fields: frozenset[str],
    label: str,
    errors: list[str],
) -> Mapping[str, Any] | None:
    """Return one schema-closed object or record a deterministic diagnostic."""

    if not isinstance(value, Mapping):
        errors.append(f"{label} must be an object")
        return None
    if set(value) != fields:
        errors.append(f"{label} fields must match the schema-closed contract")
    return value


def _row_sequence(
    value: Any,
    label: str,
    errors: list[str],
) -> Sequence[Any] | None:
    """Return a non-string row sequence or record a deterministic diagnostic."""

    if (
        isinstance(value, (str, bytes, bytearray, Mapping))
        or not isinstance(value, Sequence)
    ):
        errors.append(f"{label} must be an array")
        return None
    return value


def _canonical_identifier(value: Any, label: str, errors: list[str]) -> str | None:
    """Return one canonical lowercase deployment identifier."""

    if (
        not isinstance(value, str)
        or canonical_string(value) is None
        or IDENTIFIER_PATTERN.fullmatch(value) is None
        or len(value) > 128
    ):
        errors.append(
            f"{label} must be a canonical lowercase identifier of at most 128 bytes"
        )
        return None
    return value


def _production_runtime_handle(
    value: Any,
    label: str,
    errors: list[str],
) -> str | None:
    """Return one opaque, non-test production runtime handle."""

    if (
        not isinstance(value, str)
        or not value
        or len(value) > 256
        or not value.isascii()
        or any(character.isspace() or ord(character) < 32 for character in value)
    ):
        errors.append(f"{label} must be a canonical production runtime handle")
        return None
    detected = _non_production_label_markers(
        value,
        compact_roles=RUNTIME_HANDLE_COMPACT_ROLES,
    )
    if detected:
        errors.append(f"{label} must be a canonical production runtime handle")
        return None
    return value


def _non_production_label_markers(
    value: str,
    *,
    compact_roles: frozenset[str],
) -> set[str]:
    """Detect explicit or unambiguous compact test-role label components."""

    tokens = tuple(
        component
        for component in re.split(r"[^a-z0-9]+", value.lower())
        if component
    )
    detected = {
        component
        for component in tokens
        if component in NON_PRODUCTION_HANDLE_COMPONENTS
    }
    for token in tokens:
        for marker in NON_PRODUCTION_HANDLE_COMPONENTS:
            for role in compact_roles:
                if re.fullmatch(
                    rf"(?:{re.escape(marker)}[0-9]*{re.escape(role)}|"
                    rf"{re.escape(role)}[0-9]*{re.escape(marker)})[0-9]*",
                    token,
                ):
                    detected.add(marker)
    return detected


def _require_true(value: Any, label: str, errors: list[str]) -> None:
    if value is not True:
        errors.append(f"{label} must be true")


def _require_false(value: Any, label: str, errors: list[str]) -> None:
    if value is not False:
        errors.append(f"{label} must be false")


def _require_unique(
    values: Sequence[str],
    expected_count: int,
    label: str,
    errors: list[str],
) -> None:
    if len(values) == expected_count and len(set(values)) != expected_count:
        errors.append(f"{label} must be unique")


def _validate_validators(value: Any, errors: list[str]) -> int:
    rows = _row_sequence(value, "validators", errors)
    if rows is None:
        return 0
    if len(rows) != EXPECTED_VALIDATOR_COUNT:
        errors.append(
            "validators must contain exactly "
            f"{EXPECTED_VALIDATOR_COUNT} voting validators"
        )
    validator_ids: list[str] = []
    for index, item in enumerate(rows):
        label = f"validators[{index}]"
        row = _closed_object(item, VALIDATOR_FIELDS, label, errors)
        if row is None:
            continue
        validator_id = _canonical_identifier(
            row.get("validator_id"), f"{label}.validator_id", errors
        )
        if validator_id is not None:
            validator_ids.append(validator_id)
        _require_true(row.get("voting"), f"{label}.voting", errors)
        _require_true(row.get("da_enabled"), f"{label}.da_enabled", errors)
        _require_true(row.get("rbc_enabled"), f"{label}.rbc_enabled", errors)
    _require_unique(
        validator_ids,
        EXPECTED_VALIDATOR_COUNT,
        "validator identities",
        errors,
    )
    return len(rows)


def _validate_storage_providers(value: Any, errors: list[str]) -> int:
    rows = _row_sequence(value, "storage_providers", errors)
    if rows is None:
        return 0
    if not MIN_PROVIDER_COUNT <= len(rows) <= MAX_PROVIDER_COUNT:
        errors.append(
            "storage_providers must contain between "
            f"{MIN_PROVIDER_COUNT} and {MAX_PROVIDER_COUNT} providers"
        )
    provider_ids: list[str] = []
    operator_ids: list[str] = []
    for index, item in enumerate(rows):
        label = f"storage_providers[{index}]"
        row = _closed_object(item, PROVIDER_FIELDS, label, errors)
        if row is None:
            continue
        provider_id = _canonical_identifier(
            row.get("provider_id"), f"{label}.provider_id", errors
        )
        operator_id = _canonical_identifier(
            row.get("operator_id"), f"{label}.operator_id", errors
        )
        if provider_id is not None:
            provider_ids.append(provider_id)
        if operator_id is not None:
            operator_ids.append(operator_id)
    if len(provider_ids) == len(rows) and len(set(provider_ids)) != len(rows):
        errors.append("storage provider identities must be unique")
    if len(operator_ids) == len(rows) and len(set(operator_ids)) < MIN_PROVIDER_COUNT:
        errors.append("storage providers must have at least two distinct operators")
    return len(rows)


def _validate_gateways(value: Any, errors: list[str]) -> int:
    rows = _row_sequence(value, "gateways", errors)
    if rows is None:
        return 0
    if len(rows) != EXPECTED_GATEWAY_COUNT:
        errors.append(f"gateways must contain exactly {EXPECTED_GATEWAY_COUNT} entries")
    fields: dict[str, list[str]] = {
        "gateway_id": [],
        "region": [],
        "administrator_id": [],
    }
    for index, item in enumerate(rows):
        label = f"gateways[{index}]"
        row = _closed_object(item, GATEWAY_FIELDS, label, errors)
        if row is None:
            continue
        for field in fields:
            parsed = _canonical_identifier(
                row.get(field), f"{label}.{field}", errors
            )
            if parsed is not None:
                fields[field].append(parsed)
    for field, values in fields.items():
        _require_unique(
            values,
            EXPECTED_GATEWAY_COUNT,
            f"gateway {field.replace('_', ' ')} values",
            errors,
        )
    return len(rows)


def _validate_governance_dag(value: Any, errors: list[str]) -> int:
    rows = _row_sequence(value, "governance_dag_instances", errors)
    if rows is None:
        return 0
    if len(rows) != EXPECTED_GOVERNANCE_DAG_COUNT:
        errors.append(
            "governance_dag_instances must contain exactly "
            f"{EXPECTED_GOVERNANCE_DAG_COUNT} entries"
        )
    instance_ids: list[str] = []
    kubo_handles: list[str] = []
    administrator_ids: list[str] = []
    for index, item in enumerate(rows):
        label = f"governance_dag_instances[{index}]"
        row = _closed_object(item, GOVERNANCE_DAG_FIELDS, label, errors)
        if row is None:
            continue
        instance_id = _canonical_identifier(
            row.get("instance_id"), f"{label}.instance_id", errors
        )
        kubo_handle = _production_runtime_handle(
            row.get("kubo_handle"), f"{label}.kubo_handle", errors
        )
        administrator_id = _canonical_identifier(
            row.get("administrator_id"), f"{label}.administrator_id", errors
        )
        if instance_id is not None:
            instance_ids.append(instance_id)
        if kubo_handle is not None:
            kubo_handles.append(kubo_handle)
        if administrator_id is not None:
            administrator_ids.append(administrator_id)
    for label, values in (
        ("Governance DAG instance identities", instance_ids),
        ("Kubo runtime handles", kubo_handles),
        ("Governance DAG administrator identities", administrator_ids),
    ):
        _require_unique(values, EXPECTED_GOVERNANCE_DAG_COUNT, label, errors)
    return len(rows)


def _validate_runtime_handles(value: Any, errors: list[str]) -> list[str]:
    handles = _closed_object(
        value,
        RUNTIME_HANDLE_FIELDS,
        "runtime_handles",
        errors,
    )
    if handles is None:
        return []
    valid_kinds: list[str] = []
    distinct_handles: list[str] = []
    for kind in RUNTIME_HANDLE_KINDS:
        handle = _production_runtime_handle(
            handles.get(kind), f"runtime_handles.{kind}", errors
        )
        if handle is not None:
            valid_kinds.append(kind)
            distinct_handles.append(handle)
    if len(distinct_handles) == len(RUNTIME_HANDLE_KINDS) and len(
        set(distinct_handles)
    ) != len(RUNTIME_HANDLE_KINDS):
        errors.append("runtime handles must be distinct")
    return valid_kinds


def _validate_runtime_material_policy(value: Any, errors: list[str]) -> bool:
    before = len(errors)
    policy = _closed_object(
        value,
        RUNTIME_MATERIAL_POLICY_FIELDS,
        "runtime_material_policy",
        errors,
    )
    if policy is None:
        return False
    _require_false(
        policy.get("configuration_contains_credentials"),
        "runtime_material_policy.configuration_contains_credentials",
        errors,
    )
    _require_false(
        policy.get("configuration_contains_private_material"),
        "runtime_material_policy.configuration_contains_private_material",
        errors,
    )
    _require_true(
        policy.get("external_injection_required"),
        "runtime_material_policy.external_injection_required",
        errors,
    )
    return len(errors) == before


def _canonical_nonzero_sha256(
    value: Any,
    label: str,
    errors: list[str],
) -> str | None:
    """Return one canonical non-zero SHA-256 digest."""

    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{label} must be canonical lowercase SHA-256")
        return None
    if not any(bytes.fromhex(value)):
        errors.append(f"{label} must not be zero")
        return None
    return value


def _validate_signed_model_artifacts(value: Any, errors: list[str]) -> int:
    """Validate the digest-only signed model inventory bound to the topology."""

    rows = _row_sequence(value, "signed_model_artifacts", errors)
    if rows is None:
        return 0
    if not MIN_SIGNED_MODEL_ARTIFACT_COUNT <= len(rows) <= MAX_SIGNED_MODEL_ARTIFACT_COUNT:
        errors.append(
            "signed_model_artifacts must contain between "
            f"{MIN_SIGNED_MODEL_ARTIFACT_COUNT} and "
            f"{MAX_SIGNED_MODEL_ARTIFACT_COUNT} entries"
        )
    artifact_ids: list[str] = []
    artifact_digests: list[str] = []
    signature_digests: list[str] = []
    for index, item in enumerate(rows):
        label = f"signed_model_artifacts[{index}]"
        row = _closed_object(item, SIGNED_MODEL_ARTIFACT_FIELDS, label, errors)
        if row is None:
            continue
        artifact_id = _canonical_identifier(
            row.get("artifact_id"),
            f"{label}.artifact_id",
            errors,
        )
        if artifact_id is not None:
            forbidden = _non_production_label_markers(
                artifact_id,
                compact_roles=MODEL_ARTIFACT_COMPACT_ROLES,
            )
            if forbidden:
                errors.append(f"{label}.artifact_id must identify a production model")
            else:
                artifact_ids.append(artifact_id)
        revision = row.get("revision")
        if (
            not isinstance(revision, int)
            or isinstance(revision, bool)
            or not 1 <= revision <= MAX_SIGNED_MODEL_REVISION
        ):
            errors.append(
                f"{label}.revision must be an integer in "
                f"1..{MAX_SIGNED_MODEL_REVISION}"
            )
        artifact_digest = _canonical_nonzero_sha256(
            row.get("artifact_sha256"),
            f"{label}.artifact_sha256",
            errors,
        )
        if artifact_digest is not None:
            artifact_digests.append(artifact_digest)
        signature_digest = _canonical_nonzero_sha256(
            row.get("signature_sha256"),
            f"{label}.signature_sha256",
            errors,
        )
        if signature_digest is not None:
            signature_digests.append(signature_digest)
        _canonical_nonzero_sha256(
            row.get("signer_public_key_fingerprint_sha256"),
            f"{label}.signer_public_key_fingerprint_sha256",
            errors,
        )
        if row.get("signature_algorithm") not in SIGNED_MODEL_SIGNATURE_ALGORITHMS:
            errors.append(
                f"{label}.signature_algorithm must be one of "
                f"{SIGNED_MODEL_SIGNATURE_ALGORITHMS}"
            )
        _require_true(
            row.get("signature_verified"),
            f"{label}.signature_verified",
            errors,
        )
    for label, values in (
        ("signed model artifact identities", artifact_ids),
        ("signed model artifact digests", artifact_digests),
        ("signed model signature digests", signature_digests),
    ):
        if len(values) == len(rows) and len(set(values)) != len(values):
            errors.append(f"{label} must be unique")
    return len(rows)


def _validate_lane_slots(
    value: Any,
    deployment_id: str | None,
    environment: str | None,
    errors: list[str],
) -> bool:
    before = len(errors)
    rows = _row_sequence(value, "lane_slots", errors)
    if rows is None:
        return False
    valid = True
    if len(DEFAULT_REQUIRED_GATES) != EXPECTED_LANE_SLOT_COUNT:
        errors.append(
            "canonical readiness gate inventory must contain exactly "
            f"{EXPECTED_LANE_SLOT_COUNT} lanes"
        )
        valid = False
    if len(rows) != EXPECTED_LANE_SLOT_COUNT:
        errors.append(
            f"lane_slots must contain exactly {EXPECTED_LANE_SLOT_COUNT} entries"
        )
        valid = False
    observed_gates: list[str] = []
    for index, item in enumerate(rows):
        label = f"lane_slots[{index}]"
        row = _closed_object(item, LANE_SLOT_FIELDS, label, errors)
        if row is None:
            valid = False
            continue
        gate = canonical_string(row.get("gate"))
        if gate is None:
            errors.append(f"{label}.gate must be a canonical string")
            valid = False
        else:
            observed_gates.append(gate)
        if row.get("deployment_id") != deployment_id:
            errors.append(f"{label}.deployment_id must match deployment context")
            valid = False
        if row.get("environment") != environment:
            errors.append(f"{label}.environment must match deployment context")
            valid = False
    expected_gates = list(DEFAULT_REQUIRED_GATES)
    if observed_gates != expected_gates:
        errors.append(
            "lane_slots must match all 17 readiness lanes in canonical order"
        )
        valid = False
    if len(set(observed_gates)) != len(observed_gates):
        errors.append("lane_slots must not contain duplicate gates")
        valid = False
    if set(observed_gates) - set(expected_gates):
        errors.append("lane_slots must not contain unknown gates")
        valid = False
    return valid and len(errors) == before


def validate_manifest(
    payload: dict[str, Any],
    digest: str,
    *,
    expected_deployment_id: str,
    expected_environment: str,
) -> tuple[dict[str, Any], list[str]]:
    """Validate one non-secret deployment plan and build a payload-free summary."""

    errors: list[str] = []
    visit_sensitive_fields(
        payload,
        "",
        errors,
        sensitive_keys=(),
        evidence_label="SoraFS L1 deployment qualification manifest",
    )
    manifest = _closed_object(
        payload,
        MANIFEST_FIELDS,
        "deployment qualification manifest",
        errors,
    )

    deployment_id: str | None = None
    environment: str | None = None
    if manifest is not None:
        if manifest.get("schema") != MANIFEST_SCHEMA:
            errors.append("deployment qualification schema must match the contract")
        deployment = _closed_object(
            manifest.get("deployment"),
            DEPLOYMENT_FIELDS,
            "deployment",
            errors,
        )
        if deployment is not None:
            deployment_errors: list[str] = []
            parsed_deployment_id = require_production_deployment_id_value(
                deployment.get("deployment_id"),
                deployment_errors,
                "deployment.deployment_id",
            )
            errors.extend(deployment_errors)
            if parsed_deployment_id:
                deployment_id = parsed_deployment_id
            if not is_production_ready_environment(deployment.get("environment")):
                errors.append("deployment.environment must be production")
            else:
                environment = str(deployment.get("environment"))
            if deployment_id != expected_deployment_id:
                errors.append(
                    "deployment.deployment_id must match the operator-reviewed value"
                )
            if environment != expected_environment:
                errors.append(
                    "deployment.environment must match the operator-reviewed value"
                )

    validator_count = _validate_validators(
        None if manifest is None else manifest.get("validators"), errors
    )
    provider_count = _validate_storage_providers(
        None if manifest is None else manifest.get("storage_providers"), errors
    )
    gateway_count = _validate_gateways(
        None if manifest is None else manifest.get("gateways"), errors
    )
    governance_dag_count = _validate_governance_dag(
        None if manifest is None else manifest.get("governance_dag_instances"),
        errors,
    )
    runtime_handle_kinds = _validate_runtime_handles(
        None if manifest is None else manifest.get("runtime_handles"), errors
    )
    runtime_material_policy_valid = _validate_runtime_material_policy(
        None if manifest is None else manifest.get("runtime_material_policy"),
        errors,
    )
    signed_model_artifact_count = _validate_signed_model_artifacts(
        None if manifest is None else manifest.get("signed_model_artifacts"),
        errors,
    )
    lane_slots_valid = _validate_lane_slots(
        None if manifest is None else manifest.get("lane_slots"),
        deployment_id,
        environment,
        errors,
    )

    summary: dict[str, Any] = {
        "schema": SUMMARY_SCHEMA,
        "status": "configuration-qualified" if not errors else "blocked",
        "qualification_scope": "pre-deployment-configuration",
        "live_evidence_recognized": False,
        "promotion_eligible": False,
        "manifest_sha256": digest,
        "canonical_manifest_sha256": canonical_manifest_sha256(payload),
        "deployment": {
            "deployment_id": deployment_id,
            "environment": environment,
        },
        "validator_count": validator_count,
        "storage_provider_count": provider_count,
        "gateway_count": gateway_count,
        "governance_dag_instance_count": governance_dag_count,
        "runtime_handle_kinds": runtime_handle_kinds,
        "runtime_material_policy_valid": runtime_material_policy_valid,
        "signed_model_artifact_count": signed_model_artifact_count,
        "required_lane_slots": list(DEFAULT_REQUIRED_GATES),
        "recognized_lane_slot_count": (
            EXPECTED_LANE_SLOT_COUNT if lane_slots_valid else 0
        ),
        "errors": errors,
    }
    return summary, errors


def _blocked_summary(errors: list[str]) -> dict[str, Any]:
    """Build a stable summary when the manifest cannot be loaded."""

    return {
        "schema": SUMMARY_SCHEMA,
        "status": "blocked",
        "qualification_scope": "pre-deployment-configuration",
        "live_evidence_recognized": False,
        "promotion_eligible": False,
        "manifest_sha256": None,
        "canonical_manifest_sha256": None,
        "deployment": {"deployment_id": None, "environment": None},
        "validator_count": 0,
        "storage_provider_count": 0,
        "gateway_count": 0,
        "governance_dag_instance_count": 0,
        "runtime_handle_kinds": [],
        "runtime_material_policy_valid": False,
        "signed_model_artifact_count": 0,
        "required_lane_slots": list(DEFAULT_REQUIRED_GATES),
        "recognized_lane_slot_count": 0,
        "errors": errors,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse L1 deployment-qualification checker arguments."""

    parser = EvidenceArgumentParser(
        description="Validate a non-secret SoraFS L1 deployment topology plan.",
    )
    parser.add_argument(
        "--manifest",
        dest="evidence",
        action="append",
        type=Path,
        default=[],
        help=(
            "Exactly one schema-closed, non-secret deployment "
            "qualification manifest."
        ),
    )
    parser.add_argument(
        "--deployment-id",
        required=True,
        help="Operator-reviewed final production deployment id.",
    )
    parser.add_argument(
        "--environment",
        required=True,
        help="Operator-reviewed prod/production environment.",
    )
    parser.add_argument("--summary-out", type=Path)
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error
    return parser.parse_args(expanded_args)


def main(argv: list[str] | None = None) -> int:
    """Run the pre-deployment topology qualification checker."""

    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    preflight_errors = validate_checker_preflight(args)
    if len(args.evidence) != 1:
        preflight_errors.append(
            "provide exactly one --manifest deployment qualification input"
        )
    expected_deployment_errors: list[str] = []
    expected_deployment_id = require_production_deployment_id_value(
        args.deployment_id,
        expected_deployment_errors,
        "--deployment-id",
    )
    preflight_errors.extend(expected_deployment_errors)
    if not is_production_ready_environment(args.environment):
        preflight_errors.append("--environment must be production")
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    load_errors: list[str] = []
    loaded = load_evidence_json_with_sha256_or_record_error(
        args.evidence[0],
        MAX_MANIFEST_BYTES,
        load_errors,
    )
    if loaded is None:
        summary = _blocked_summary(load_errors)
        _, render_errors = render_and_write_checker_summary(args.summary_out, summary)
        if render_errors:
            emit_checker_error_lines(render_errors)
            return 2
        emit_checker_error_block(
            "SoraFS L1 deployment configuration qualification is blocked:",
            load_errors,
        )
        return 1

    payload, digest = loaded
    summary, errors = validate_manifest(
        payload,
        digest,
        expected_deployment_id=expected_deployment_id,
        expected_environment=args.environment,
    )
    _, render_errors = render_and_write_checker_summary(args.summary_out, summary)
    if render_errors:
        emit_checker_error_lines(render_errors)
        return 2
    if errors:
        emit_checker_error_block(
            "SoraFS L1 deployment configuration qualification is blocked:",
            errors,
        )
        return 1
    emit_checker_notice(
        "SoraFS L1 deployment configuration is schema-qualified; "
        "no live evidence was recognized and promotion remains ineligible."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
