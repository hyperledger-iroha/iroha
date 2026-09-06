#!/usr/bin/env python3
"""Shared SoraFS L1 topology-qualification binding helpers."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Mapping

from sccp_release_common import verify_ed25519
from sorafs_evidence_json import load_evidence_json_with_sha256_or_record_error
from sorafs_evidence_validation import (
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_response_args import non_negative_int_arg, positive_int_arg
import taira_constants


MANIFEST_SCHEMA = "sorafs.l1.deployment_qualification.v1"
SUMMARY_SCHEMA = "sorafs.l1.deployment_qualification.summary.v1"
SIGNED_QUALIFICATION_ENVELOPE_SCHEMA = (
    "sorafs.l1.deployment_qualification.signed_envelope.v1"
)
TOPOLOGY_QUALIFICATION_SIGNATURE_DOMAIN = (
    b"sorafs-l1-topology-qualification-envelope-v1\x00"
)
MAX_QUALIFICATION_SUMMARY_BYTES = 256 * 1024
MAX_QUALIFICATION_ENVELOPE_BYTES = 64 * 1024
DEFAULT_MAX_QUALIFICATION_REVIEW_AGE_SECS = 14 * 24 * 60 * 60
MAX_BOUNDED_INTEGER = (1 << 63) - 1
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
CANONICAL_TAIRA_VALIDATOR_IDS = tuple(taira_constants.SLUGS)
CANONICAL_TAIRA_VALIDATOR_IDS_SHA256 = hashlib.sha256(
    json.dumps(
        list(CANONICAL_TAIRA_VALIDATOR_IDS),
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")
).hexdigest()
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
        "validator_ids",
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
        "network",
        "chain_id",
        "chain_discriminant",
        "validator_ids_sha256",
    }
)
AUTHENTICATED_TOPOLOGY_BINDING_FIELDS = TOPOLOGY_BINDING_FIELDS | frozenset(
    {
        "signer_authentication_kind",
        "signer_backend",
        "signer_service_id",
        "signer_administrator_id",
        "signer_key_revision",
        "signer_policy_revision",
        "signer_policy_digest_sha256",
        "signer_public_key_fingerprint_sha256",
    }
)
SIGNED_QUALIFICATION_ENVELOPE_FIELDS = (
    AUTHENTICATED_TOPOLOGY_BINDING_FIELDS
    | frozenset(
    {
        "schema",
        "reviewed_at_unix",
        "signature_algorithm",
        "signature_hex",
    }
    )
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


def add_signed_topology_qualification_arguments(parser: Any) -> None:
    """Add the mandatory independently authenticated topology trust tuple."""

    add_topology_qualification_argument(parser)
    parser.add_argument(
        "--topology-qualification-envelope",
        required=True,
        type=Path,
        help="Independently signed companion for the exact topology summary.",
    )
    parser.add_argument(
        "--topology-qualification-verification-public-key-hex",
        required=True,
        help="Operator-trusted non-zero raw Ed25519 topology verification key.",
    )
    parser.add_argument(
        "--topology-qualification-signer-service-id",
        required=True,
        help="Operator-trusted external software signer service identity.",
    )
    parser.add_argument(
        "--topology-qualification-signer-administrator-id",
        required=True,
        help="Independently administered topology signer identity.",
    )
    parser.add_argument(
        "--topology-qualification-signer-key-revision",
        required=True,
        type=positive_int_arg,
        help="Operator-trusted positive topology signer key revision.",
    )
    parser.add_argument(
        "--topology-qualification-signer-policy-revision",
        required=True,
        type=positive_int_arg,
        help="Operator-trusted positive topology signer policy revision.",
    )
    parser.add_argument(
        "--topology-qualification-signer-policy-digest-hex",
        required=True,
        help="Operator-trusted topology signer policy SHA-256 digest.",
    )
    parser.add_argument(
        "--max-topology-qualification-review-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_QUALIFICATION_REVIEW_AGE_SECS,
        help="Maximum accepted age of the independently signed topology review.",
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


def _canonical_signature_hex(value: Any) -> str | None:
    if (
        not isinstance(value, str)
        or len(value) != 128
        or value != value.lower()
    ):
        return None
    try:
        decoded = bytes.fromhex(value)
    except ValueError:
        return None
    return value if any(decoded) else None


def _public_key_from_hex(value: Any) -> bytes | None:
    if not isinstance(value, str) or len(value) != 64 or value != value.lower():
        return None
    try:
        public_key = bytes.fromhex(value)
    except ValueError:
        return None
    return public_key if any(public_key) else None


def _positive_bounded_integer(value: Any) -> int | None:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value <= 0
        or value > MAX_BOUNDED_INTEGER
    ):
        return None
    return value


def topology_qualification_envelope_signing_bytes(
    envelope: Mapping[str, Any],
) -> bytes:
    """Return canonical, topology-domain-separated envelope signing bytes."""

    if (
        not isinstance(envelope, Mapping)
        or set(envelope) != SIGNED_QUALIFICATION_ENVELOPE_FIELDS
    ):
        raise ValueError(
            "signed topology qualification envelope has the wrong exact schema"
        )
    unsigned = dict(envelope)
    unsigned.pop("signature_hex")
    try:
        encoded = json.dumps(
            unsigned,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ValueError(
            "signed topology qualification envelope is not canonically encodable"
        ) from error
    return TOPOLOGY_QUALIFICATION_SIGNATURE_DOMAIN + encoded


def _validate_qualification_payload(
    payload: dict[str, Any],
    *,
    expected_deployment_id: str | None,
    expected_environment: str | None,
) -> tuple[dict[str, Any] | None, list[str]]:
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
        "network",
        "chain_id",
        "chain_discriminant",
    }:
        errors.append(
            "topology qualification deployment fields must match the Taira chain contract"
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
        if deployment.get("network") != taira_constants.NETWORK_NAME:
            errors.append(
                "topology qualification network must be exactly `taira`; Minamoto evidence is not accepted"
            )
        if deployment.get("chain_id") != taira_constants.CHAIN_ID:
            errors.append(
                "topology qualification chain_id must match the canonical Taira chain"
            )
        if (
            deployment.get("chain_discriminant")
            != taira_constants.CHAIN_DISCRIMINANT
        ):
            errors.append(
                "topology qualification chain_discriminant must match the canonical Taira discriminator"
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
    if payload.get("validator_ids") != list(CANONICAL_TAIRA_VALIDATOR_IDS):
        errors.append(
            "topology qualification validator_ids must match the canonical "
            "ordered Taira validator identities"
        )
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
        "external_signer",
        "key_custody",
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
        "network": taira_constants.NETWORK_NAME,
        "chain_id": taira_constants.CHAIN_ID,
        "chain_discriminant": taira_constants.CHAIN_DISCRIMINANT,
        "validator_ids_sha256": CANONICAL_TAIRA_VALIDATOR_IDS_SHA256,
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


def load_signed_topology_qualification_binding(
    summary_path: Path,
    envelope_path: Path,
    *,
    trusted_public_key: bytes,
    trusted_signer_service_id: str,
    trusted_signer_administrator_id: str,
    trusted_key_revision: int,
    trusted_policy_revision: int,
    trusted_policy_digest_hex: str,
    now_unix: int,
    max_review_age_secs: int = DEFAULT_MAX_QUALIFICATION_REVIEW_AGE_SECS,
    expected_deployment_id: str | None = None,
    expected_environment: str | None = None,
    independent_public_keys: Mapping[str, bytes | None] | None = None,
    independent_administrator_ids: Mapping[str, str | None] | None = None,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Authenticate one topology binding with an independent Ed25519 envelope.

    The unsigned loader remains limited to lane-local base bindings. Production
    callers receive the authenticated signer provenance in this binding.
    """

    import sorafs_software_signer_evidence as software_signer_evidence

    errors: list[str] = []
    if (
        not isinstance(trusted_public_key, bytes)
        or len(trusted_public_key) != 32
        or not any(trusted_public_key)
    ):
        errors.append("trusted topology public key must be exactly 32 non-zero bytes")
    trusted_signer = software_signer_evidence.validate_foundational_software_signer(
        {
            "backend": "software",
            "service_id": trusted_signer_service_id,
            "administrator_id": trusted_signer_administrator_id,
            "key_revision": trusted_key_revision,
            "policy_revision": trusted_policy_revision,
            "policy_digest_sha256": trusted_policy_digest_hex,
        },
        errors,
    )
    for label, candidate in (independent_public_keys or {}).items():
        if isinstance(candidate, bytes) and candidate == trusted_public_key:
            errors.append(f"trusted topology public key must differ from {label}")
    for label, candidate in (independent_administrator_ids or {}).items():
        if (
            isinstance(candidate, str)
            and candidate == trusted_signer["signer_administrator_id"]
        ):
            errors.append(
                f"trusted topology administrator must differ from {label}"
            )
    validation_clock = _positive_bounded_integer(now_unix)
    if validation_clock is None:
        errors.append(
            "topology qualification validation clock must be a positive bounded integer"
        )
    if (
        not isinstance(max_review_age_secs, int)
        or isinstance(max_review_age_secs, bool)
        or max_review_age_secs < 0
        or max_review_age_secs > MAX_BOUNDED_INTEGER
    ):
        errors.append(
            "topology qualification maximum review age must be a non-negative "
            "bounded integer"
        )
    if errors:
        return None, errors

    binding, binding_errors = load_topology_qualification_binding(
        summary_path,
        expected_deployment_id=expected_deployment_id,
        expected_environment=expected_environment,
    )
    errors.extend(binding_errors)
    if binding is None:
        return None, errors

    loaded = load_evidence_json_with_sha256_or_record_error(
        envelope_path,
        MAX_QUALIFICATION_ENVELOPE_BYTES,
        errors,
    )
    if loaded is None:
        return None, errors
    envelope, _envelope_sha256 = loaded
    if set(envelope) != SIGNED_QUALIFICATION_ENVELOPE_FIELDS:
        errors.append(
            "signed topology qualification envelope fields must match the "
            "schema-closed contract"
        )
    if envelope.get("schema") != SIGNED_QUALIFICATION_ENVELOPE_SCHEMA:
        errors.append(
            "signed topology qualification envelope schema must match the contract"
        )
    for field in (
        "qualification_summary_sha256",
        "manifest_sha256",
        "canonical_manifest_sha256",
        "validator_ids_sha256",
    ):
        digest = _canonical_sha256(envelope.get(field))
        if digest is None:
            errors.append(
                f"signed topology qualification envelope {field} must be "
                "canonical non-zero SHA-256"
            )
        elif digest != binding[field]:
            errors.append(
                f"signed topology qualification envelope {field} must match "
                "the exact qualification binding"
            )
    for field in (
        "deployment_id",
        "environment",
        "network",
        "chain_id",
        "chain_discriminant",
    ):
        if envelope.get(field) != binding[field]:
            errors.append(
                f"signed topology qualification envelope {field} must match "
                "the exact qualification binding"
            )
    if envelope.get("signer_authentication_kind") != "external-ed25519":
        errors.append(
            "signed topology qualification envelope signer_authentication_kind "
            "must be `external-ed25519`"
        )
    software_signer_evidence.validate_aggregate_software_signer(envelope, errors)
    for field in (
        "signer_backend",
        "signer_service_id",
        "signer_administrator_id",
        "signer_key_revision",
        "signer_policy_revision",
        "signer_policy_digest_sha256",
    ):
        if envelope.get(field) != trusted_signer[field]:
            errors.append(
                f"signed topology qualification envelope {field} must match "
                "the trusted external software signer"
            )
    assert isinstance(trusted_public_key, bytes)
    trusted_fingerprint = hashlib.sha256(trusted_public_key).hexdigest()
    fingerprint = _canonical_sha256(
        envelope.get("signer_public_key_fingerprint_sha256")
    )
    if fingerprint is None:
        errors.append(
            "signed topology qualification envelope signer public-key fingerprint "
            "must be canonical non-zero SHA-256"
        )
    elif fingerprint != trusted_fingerprint:
        errors.append(
            "signed topology qualification envelope signer public-key fingerprint "
            "must match the trusted public key"
        )

    reviewed_at = _positive_bounded_integer(envelope.get("reviewed_at_unix"))
    if reviewed_at is None:
        errors.append(
            "signed topology qualification envelope reviewed_at_unix must be "
            "a positive bounded timestamp"
        )
    else:
        assert validation_clock is not None
        if reviewed_at > validation_clock:
            errors.append(
                "signed topology qualification envelope reviewed_at_unix must "
                "not be in the future"
            )
        elif validation_clock - reviewed_at > max_review_age_secs:
            errors.append(
                "signed topology qualification envelope review exceeds the "
                "maximum age"
            )

    signature_algorithm = envelope.get("signature_algorithm")
    if signature_algorithm != "ed25519":
        errors.append(
            "signed topology qualification envelope signature_algorithm must "
            "be `ed25519`"
        )
    signature_hex = _canonical_signature_hex(envelope.get("signature_hex"))
    if signature_hex is None:
        errors.append(
            "signed topology qualification envelope signature_hex must be a "
            "non-zero lowercase Ed25519 signature"
        )
    if signature_algorithm == "ed25519" and signature_hex is not None:
        try:
            signing_bytes = topology_qualification_envelope_signing_bytes(envelope)
            authenticated = verify_ed25519(
                trusted_public_key,
                bytes.fromhex(signature_hex),
                signing_bytes,
            )
        except (TypeError, ValueError):
            errors.append(
                "signed topology qualification envelope signature could not "
                "be authenticated"
            )
        else:
            if not authenticated:
                errors.append(
                    "signed topology qualification envelope signature must "
                    "authenticate with the trusted topology key"
                )
    if errors:
        return None, errors
    return {
        **binding,
        "signer_authentication_kind": envelope["signer_authentication_kind"],
        "signer_backend": envelope["signer_backend"],
        "signer_service_id": envelope["signer_service_id"],
        "signer_administrator_id": envelope["signer_administrator_id"],
        "signer_key_revision": envelope["signer_key_revision"],
        "signer_policy_revision": envelope["signer_policy_revision"],
        "signer_policy_digest_sha256": envelope["signer_policy_digest_sha256"],
        "signer_public_key_fingerprint_sha256": envelope[
            "signer_public_key_fingerprint_sha256"
        ],
    }, []


def load_signed_topology_qualification_from_args(
    args: Any,
    *,
    expected_deployment_id: str | None,
    expected_environment: str | None,
) -> tuple[dict[str, str] | None, list[str]]:
    """Authenticate topology inputs added by the shared signed CLI helper."""

    public_key = _public_key_from_hex(
        args.topology_qualification_verification_public_key_hex
    ) or b""
    independent_public_keys = {}
    for attribute, label in (
        ("resilience_qualification_signer_public_key_hex", "resilience signer key"),
        ("foundational_signer_public_key_hex", "promotion signer key"),
        ("trusted_public_key_hex", "promotion signer key"),
        ("provenance_verification_public_key_hex", "lane signer key"),
    ):
        candidate = _public_key_from_hex(getattr(args, attribute, None))
        if candidate is not None:
            independent_public_keys[label] = candidate
    independent_administrator_ids = {
        label: getattr(args, attribute)
        for attribute, label in (
            ("signer_administrator_id", "promotion signer administrator"),
            ("expected_signer_administrator_id", "promotion signer administrator"),
        )
        if getattr(args, attribute, None) is not None
    }
    return load_signed_topology_qualification_binding(
        args.topology_qualification_summary,
        args.topology_qualification_envelope,
        trusted_public_key=public_key,
        trusted_signer_service_id=args.topology_qualification_signer_service_id,
        trusted_signer_administrator_id=(
            args.topology_qualification_signer_administrator_id
        ),
        trusted_key_revision=args.topology_qualification_signer_key_revision,
        trusted_policy_revision=args.topology_qualification_signer_policy_revision,
        trusted_policy_digest_hex=args.topology_qualification_signer_policy_digest_hex,
        now_unix=args.now_unix,
        max_review_age_secs=args.max_topology_qualification_review_age_secs,
        expected_deployment_id=expected_deployment_id,
        expected_environment=expected_environment,
        independent_public_keys=independent_public_keys,
        independent_administrator_ids=independent_administrator_ids,
    )


def validate_independent_topology_signer_domains(
    topology: Any,
    *signers: tuple[str, Any],
) -> list[str]:
    """Reject administrator or key reuse across qualifying signer domains."""

    if not isinstance(topology, Mapping):
        return []
    errors: list[str] = []
    topology_administrator = topology.get("signer_administrator_id")
    topology_fingerprint = topology.get("signer_public_key_fingerprint_sha256")
    for label, signer in signers:
        if not isinstance(signer, Mapping):
            continue
        if isinstance(topology_administrator, str) and (
            signer.get("signer_administrator_id") == topology_administrator
        ):
            errors.append(f"topology signer administrator must differ from {label}")
        if isinstance(topology_fingerprint, str) and (
            signer.get("signer_public_key_fingerprint_sha256") == topology_fingerprint
        ):
            errors.append(f"topology signer public key must differ from {label}")
    return errors


def _validate_topology_binding_object(
    value: Any,
    *,
    expected: Mapping[str, Any] | None,
    path: str = "topology_qualification",
    authenticated_required: bool,
) -> list[str]:
    """Validate one base or authenticated topology binding."""

    errors: list[str] = []
    observed_fields = frozenset(value) if isinstance(value, dict) else frozenset()
    allowed_fields = (
        {AUTHENTICATED_TOPOLOGY_BINDING_FIELDS}
        if authenticated_required
        else {TOPOLOGY_BINDING_FIELDS, AUTHENTICATED_TOPOLOGY_BINDING_FIELDS}
    )
    if not isinstance(value, dict) or observed_fields not in allowed_fields:
        return [f"{path} fields must match the schema-closed contract"]
    authenticated = observed_fields == AUTHENTICATED_TOPOLOGY_BINDING_FIELDS
    for field in (
        "qualification_summary_sha256",
        "manifest_sha256",
        "canonical_manifest_sha256",
        "validator_ids_sha256",
    ):
        if _canonical_sha256(value.get(field)) is None:
            errors.append(f"{path}.{field} must be canonical non-zero SHA-256")
    deployment_errors: list[str] = []
    require_rollout_deployment_id(value, deployment_errors)
    require_rollout_environment(value, deployment_errors)
    errors.extend(f"{path} {error}" for error in deployment_errors)
    if value.get("environment") not in {"prod", "production"}:
        errors.append(f"{path}.environment must be production")
    if value.get("network") != taira_constants.NETWORK_NAME:
        errors.append(
            f"{path}.network must be exactly `taira`; Minamoto evidence is not accepted"
        )
    if value.get("chain_id") != taira_constants.CHAIN_ID:
        errors.append(f"{path}.chain_id must match the canonical Taira chain")
    if value.get("chain_discriminant") != taira_constants.CHAIN_DISCRIMINANT:
        errors.append(
            f"{path}.chain_discriminant must match the canonical Taira discriminator"
        )
    if value.get("validator_ids_sha256") != CANONICAL_TAIRA_VALIDATOR_IDS_SHA256:
        errors.append(
            f"{path}.validator_ids_sha256 must bind the canonical ordered Taira validators"
        )
    if authenticated:
        import sorafs_software_signer_evidence as software_signer_evidence

        if value.get("signer_authentication_kind") != "external-ed25519":
            errors.append(
                f"{path}.signer_authentication_kind must be `external-ed25519`"
            )
        software_signer_evidence.validate_aggregate_software_signer(value, errors)
        if _canonical_sha256(
            value.get("signer_public_key_fingerprint_sha256")
        ) is None:
            errors.append(
                f"{path}.signer_public_key_fingerprint_sha256 must be canonical "
                "non-zero SHA-256"
            )
    if expected is not None:
        comparison_fields = (
            AUTHENTICATED_TOPOLOGY_BINDING_FIELDS
            if authenticated
            and set(expected) == AUTHENTICATED_TOPOLOGY_BINDING_FIELDS
            else TOPOLOGY_BINDING_FIELDS
        )
        for field in comparison_fields:
            if value.get(field) != expected.get(field):
                errors.append(f"{path}.{field} must match the reviewed topology")
    return errors


def validate_topology_binding_object(
    value: Any,
    *,
    expected: Mapping[str, Any] | None = None,
    path: str = "topology_qualification",
) -> list[str]:
    """Validate a lane-local or authenticated topology binding."""

    return _validate_topology_binding_object(
        value, expected=expected, path=path, authenticated_required=False
    )


def validate_authenticated_topology_binding_object(
    value: Any,
    *,
    expected: Mapping[str, Any] | None = None,
    path: str = "topology_qualification",
) -> list[str]:
    """Require full external software-signer provenance in a binding."""

    return _validate_topology_binding_object(
        value, expected=expected, path=path, authenticated_required=True
    )


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
