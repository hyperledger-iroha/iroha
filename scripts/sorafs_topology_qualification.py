#!/usr/bin/env python3
"""Shared SoraFS L1 topology-qualification binding helpers."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Mapping

from sorafs_evidence_json import load_evidence_json_with_sha256_or_record_error
from sorafs_evidence_validation import (
    require_rollout_deployment_id,
    require_rollout_environment,
)


MANIFEST_SCHEMA = "sorafs.l1.deployment_qualification.v1"
SUMMARY_SCHEMA = "sorafs.l1.deployment_qualification.summary.v1"
MAX_QUALIFICATION_SUMMARY_BYTES = 256 * 1024
CANONICAL_READINESS_LANES = (
    "ai_prescreen",
    "appeal_finance",
    "gateway_compliance",
    "gateway_load",
    "governance_dag",
    "hedging_billing",
    "moderation_panel",
    "orderbook",
    "pdp",
    "pop_credentials",
    "por",
    "potr",
    "reference_sdk_release",
    "repair",
    "reputation",
    "reserve_rent",
    "transparency",
)
QUALIFICATION_SUMMARY_FIELDS = frozenset(
    {
        "schema",
        "status",
        "qualification_scope",
        "live_evidence_recognized",
        "promotion_eligible",
        "manifest_sha256",
        "canonical_manifest_sha256",
        "deployment",
        "validator_count",
        "storage_provider_count",
        "gateway_count",
        "governance_dag_instance_count",
        "runtime_handle_kinds",
        "runtime_material_policy_valid",
        "signed_model_artifact_count",
        "required_lane_slots",
        "recognized_lane_slot_count",
        "errors",
    }
)
TOPOLOGY_BINDING_FIELDS = frozenset(
    {
        "qualification_summary_sha256",
        "manifest_sha256",
        "canonical_manifest_sha256",
        "deployment_id",
        "environment",
    }
)


def add_topology_qualification_argument(
    parser: Any,
    *,
    required: bool = True,
) -> None:
    """Add the mandatory first-release L1 qualification input to a parser."""

    parser.add_argument(
        "--topology-qualification-summary",
        required=required,
        type=Path,
        help=(
            "Exact schema-qualified, non-promotable L1 topology summary whose "
            "digest must bind this lane."
        ),
    )


def canonical_manifest_sha256(payload: Mapping[str, Any]) -> str:
    """Hash the canonical JSON encoding of one schema-closed topology manifest."""

    rendered = json.dumps(
        payload,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(rendered).hexdigest()


def _canonical_sha256(value: Any) -> str | None:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or value != value.lower()
    ):
        return None
    try:
        decoded = bytes.fromhex(value)
    except ValueError:
        return None
    return value if any(decoded) else None


def _validate_qualification_payload(
    payload: dict[str, Any],
    *,
    expected_deployment_id: str | None,
    expected_environment: str | None,
) -> tuple[dict[str, str] | None, list[str]]:
    errors: list[str] = []
    if set(payload) != QUALIFICATION_SUMMARY_FIELDS:
        errors.append(
            "topology qualification summary fields must match the schema-closed contract"
        )
    if payload.get("schema") != SUMMARY_SCHEMA:
        errors.append("topology qualification summary schema must match the contract")
    if payload.get("status") != "configuration-qualified":
        errors.append("topology qualification status must be configuration-qualified")
    if payload.get("qualification_scope") != "pre-deployment-configuration":
        errors.append(
            "topology qualification scope must be pre-deployment-configuration"
        )
    if payload.get("live_evidence_recognized") is not False:
        errors.append("topology qualification must not claim live evidence")
    if payload.get("promotion_eligible") is not False:
        errors.append("topology qualification must remain non-promotable")
    manifest_sha256 = _canonical_sha256(payload.get("manifest_sha256"))
    if manifest_sha256 is None:
        errors.append(
            "topology qualification manifest_sha256 must be canonical non-zero SHA-256"
        )
    canonical_sha256 = _canonical_sha256(payload.get("canonical_manifest_sha256"))
    if canonical_sha256 is None:
        errors.append(
            "topology qualification canonical_manifest_sha256 must be canonical non-zero SHA-256"
        )

    deployment = payload.get("deployment")
    deployment_id = None
    environment = None
    if not isinstance(deployment, dict) or set(deployment) != {
        "deployment_id",
        "environment",
    }:
        errors.append(
            "topology qualification deployment fields must be deployment_id and environment"
        )
    else:
        deployment_errors: list[str] = []
        deployment_id = require_rollout_deployment_id(deployment, deployment_errors)
        environment = require_rollout_environment(deployment, deployment_errors)
        errors.extend(f"topology qualification {error}" for error in deployment_errors)
        if environment not in {"prod", "production"}:
            errors.append("topology qualification environment must be production")
        if expected_deployment_id is not None and deployment_id != expected_deployment_id:
            errors.append(
                "topology qualification deployment_id must match the reviewed lane context"
            )
        if expected_environment is not None and environment != expected_environment:
            errors.append(
                "topology qualification environment must match the reviewed lane context"
            )

    expected_scalars = {
        "validator_count": 4,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "recognized_lane_slot_count": len(CANONICAL_READINESS_LANES),
    }
    for field, expected in expected_scalars.items():
        if payload.get(field) != expected:
            errors.append(f"topology qualification {field} must be {expected}")
    provider_count = payload.get("storage_provider_count")
    if (
        not isinstance(provider_count, int)
        or isinstance(provider_count, bool)
        or not 2 <= provider_count <= 64
    ):
        errors.append(
            "topology qualification storage_provider_count must be in 2..64"
        )
    if payload.get("runtime_handle_kinds") != [
        "monitoring",
        "hsm",
        "kms",
        "webauthn",
    ]:
        errors.append(
            "topology qualification runtime_handle_kinds must match the production contract"
        )
    if payload.get("runtime_material_policy_valid") is not True:
        errors.append("topology qualification runtime material policy must be valid")
    model_artifact_count = payload.get("signed_model_artifact_count")
    if (
        not isinstance(model_artifact_count, int)
        or isinstance(model_artifact_count, bool)
        or not 1 <= model_artifact_count <= 64
    ):
        errors.append(
            "topology qualification signed_model_artifact_count must be in 1..64"
        )
    if payload.get("required_lane_slots") != list(CANONICAL_READINESS_LANES):
        errors.append(
            "topology qualification lane inventory must match the canonical 17-lane order"
        )
    if payload.get("errors") != []:
        errors.append("topology qualification errors must be empty")
    if errors:
        return None, errors
    assert manifest_sha256 is not None
    assert canonical_sha256 is not None
    assert deployment_id
    assert environment
    return {
        "manifest_sha256": manifest_sha256,
        "canonical_manifest_sha256": canonical_sha256,
        "deployment_id": deployment_id,
        "environment": environment,
    }, errors


def load_topology_qualification_binding(
    path: Path,
    *,
    expected_deployment_id: str | None = None,
    expected_environment: str | None = None,
) -> tuple[dict[str, str] | None, list[str]]:
    """Load one exact non-promotable L1 summary and return its release binding."""

    errors: list[str] = []
    loaded = load_evidence_json_with_sha256_or_record_error(
        path,
        MAX_QUALIFICATION_SUMMARY_BYTES,
        errors,
    )
    if loaded is None:
        return None, errors
    payload, summary_sha256 = loaded
    validated, validation_errors = _validate_qualification_payload(
        payload,
        expected_deployment_id=expected_deployment_id,
        expected_environment=expected_environment,
    )
    errors.extend(validation_errors)
    if validated is None:
        return None, errors
    return {
        "qualification_summary_sha256": summary_sha256,
        **validated,
    }, errors


def validate_topology_binding_object(
    value: Any,
    *,
    expected: Mapping[str, str] | None = None,
    path: str = "topology_qualification",
) -> list[str]:
    """Validate one schema-closed topology binding and optional exact match."""

    errors: list[str] = []
    if not isinstance(value, dict) or set(value) != TOPOLOGY_BINDING_FIELDS:
        return [f"{path} fields must match the schema-closed contract"]
    for field in (
        "qualification_summary_sha256",
        "manifest_sha256",
        "canonical_manifest_sha256",
    ):
        if _canonical_sha256(value.get(field)) is None:
            errors.append(f"{path}.{field} must be canonical non-zero SHA-256")
    deployment_errors: list[str] = []
    require_rollout_deployment_id(value, deployment_errors)
    require_rollout_environment(value, deployment_errors)
    errors.extend(f"{path} {error}" for error in deployment_errors)
    if value.get("environment") not in {"prod", "production"}:
        errors.append(f"{path}.environment must be production")
    if expected is not None:
        for field in TOPOLOGY_BINDING_FIELDS:
            if value.get(field) != expected.get(field):
                errors.append(f"{path}.{field} must match the reviewed topology")
    return errors


def lane_summary_deployment_context(
    summary: Mapping[str, Any],
) -> tuple[str | None, str | None, list[str]]:
    """Extract the one deployment context represented by lane artifacts."""

    errors: list[str] = []
    contexts: set[tuple[str, str]] = set()
    artifacts = summary.get("recognized_artifacts")
    if isinstance(artifacts, list):
        for artifact in artifacts:
            if not isinstance(artifact, Mapping):
                continue
            fingerprint = artifact.get("fingerprint")
            if not isinstance(fingerprint, Mapping):
                continue
            deployment_id = fingerprint.get("deployment_id")
            environment = fingerprint.get("environment")
            if isinstance(deployment_id, str) and isinstance(environment, str):
                contexts.add((deployment_id, environment))
    if len(contexts) != 1:
        errors.append(
            "lane summary must contain exactly one reviewed deployment context"
        )
        return None, None, errors
    deployment_id, environment = next(iter(contexts))
    return deployment_id, environment, errors


def bind_lane_summary_to_topology(
    summary: dict[str, Any],
    qualification_path: Path,
) -> list[str]:
    """Attach the mandatory exact L1 binding to one generated lane summary."""

    deployment_id, environment, errors = lane_summary_deployment_context(summary)
    binding, binding_errors = load_topology_qualification_binding(
        qualification_path,
        expected_deployment_id=deployment_id,
        expected_environment=environment,
    )
    errors.extend(binding_errors)
    summary["topology_qualification"] = binding
    return errors
