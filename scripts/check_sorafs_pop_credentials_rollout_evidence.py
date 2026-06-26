#!/usr/bin/env python3
"""Validate SoraFS proof-of-personhood rollout evidence artifacts."""

from __future__ import annotations

import argparse
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_notice,
    render_and_write_checker_summary,
    validate_checker_preflight,
)
from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    evidence_path_identities,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_validation import (  # noqa: E402
    build_evidence_artifact,
    count_evidence_artifacts,
    count_evidence_files,
    evidence_gate_status,
    evidence_artifact_digest_set,
    evidence_artifact_is_valid,
    evidence_schema_by_kind,
    init_evidence_artifact_buckets,
    build_required_evidence_summary,
    record_explicit_evidence_validation_errors,
    record_evidence_artifact,
    record_evidence_validation_errors,
    require_2xx_status,
    require_bool_true,
    require_count_equal,
    require_false,
    require_false_or_absent,
    require_hex,
    require_config_backed_governance_approval,
    validate_standard_evidence_payload,
    require_maximum_int,
    require_minimum_value,
    require_non_negative_int,
    require_object,
    require_object_array,
    required_evidence_kind_names,
    require_passed_status,
    require_policy_digest,
    require_positive_int,
    require_recent_timestamp,
    require_string,
    require_string_coverage,
    require_string_equal,
    require_string_not_equal,
    record_evidence_digest_mismatch_errors,
    require_sum_equal,
    validate_bound_evidence_digest_references,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_evidence_kinds,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


SUMMARY_SCHEMA = "sorafs.pop_credentials.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_ROOT_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_REVOCATION_AGE_SECS = 24 * 60 * 60
DEFAULT_MAX_SERVICE_LAG_SECS = 15 * 60
DEFAULT_MAX_VERIFY_LATENCY_MS = 1_000
HEX64_LEN = 64

REQUIRED_ENROLLMENT_ROUTES = (
    "application_submit",
    "application_status",
    "issuer_approval",
    "renewal_request",
)
REQUIRED_VERIFIER_ROUTES = (
    "proof_verify",
    "proof_status",
    "health",
)
REQUIRED_METRICS = (
    "pop_credential_issuance_total",
    "pop_revocation_publication_total",
    "pop_membership_verify_success_total",
    "pop_membership_verify_failure_total",
    "pop_verifier_latency_ms",
    "pop_revocation_lag_seconds",
)
ROOT_BOUND_KINDS = (
    "juror_client",
    "verifier_service",
    "moderation_integration",
    "metrics_alerts",
    "governance_approval",
)
REVOCATION_BOUND_KINDS = ROOT_BOUND_KINDS

SENSITIVE_KEYS = {
    "account_id",
    "attestation_body",
    "authorization",
    "bearer_token",
    "body",
    "canonical_account",
    "credential",
    "credential_body",
    "credential_bytes",
    "credential_payload",
    "holder_identity",
    "identity_document",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "proof",
    "proof_b64",
    "proof_body",
    "proof_bytes",
    "raw_attestation",
    "raw_credential",
    "raw_proof",
    "response_body",
    "secret",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SFM-4b1 rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("issuer_bundle", "sorafs.pop.issuer_bundle_canary.v1"),
    EvidenceKind("commitment_root", "sorafs.pop.commitment_root_publication_canary.v1"),
    EvidenceKind("revocation_registry", "sorafs.pop.revocation_registry_canary.v1"),
    EvidenceKind("enrollment_portal", "sorafs.pop.enrollment_portal_canary.v1"),
    EvidenceKind("juror_client", "sorafs.pop.juror_client_canary.v1"),
    EvidenceKind("verifier_service", "sorafs.pop.verifier_service_canary.v1"),
    EvidenceKind("moderation_integration", "sorafs.pop.moderation_integration_canary.v1"),
    EvidenceKind("metrics_alerts", "sorafs.pop.metrics_alert_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.pop.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SFM-4b1 rollout gate."""

    now_unix: int
    max_root_age_secs: int
    max_revocation_age_secs: int
    max_service_lag_secs: int
    max_verify_latency_ms: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "root_digest_hex",
    "revocation_list_digest_hex",
    "synced_root_digest_hex",
    "synced_revocation_list_digest_hex",
    "pop_snapshot_digest_hex",
    "policy_digest_hex",
)


def validate_routes(
    payload: dict[str, Any],
    errors: list[str],
    *,
    required_routes: tuple[str, ...],
) -> None:
    for index, record in require_object_array(payload, "routes", errors):
        require_bool_true(record, "passed", errors, path=f"routes[{index}].passed")
        require_2xx_status(
            record,
            "status_code",
            errors,
            path=f"routes[{index}].status_code",
        )
        for field in ("authz_enforced", "signature_verified"):
            require_bool_true(record, field, errors, path=f"routes[{index}].{field}")
    require_string_coverage(payload, "routes", "name", required_routes, errors)


def validate_issuer_bundle(payload: dict[str, Any], errors: list[str]) -> None:
    require_string(payload, "issuer_id", errors)
    require_hex(payload, "bundle_id_hex", HEX64_LEN, errors)
    require_hex(payload, "root_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "revocation_list_digest_hex", HEX64_LEN, errors)
    credential_count = require_count_equal(
        payload, "credential_count", "signed_credential_count", errors
    )
    require_minimum_value(credential_count, "credential_count", 1, errors)
    require_bool_true(payload, "canonical_norito_verified", errors)
    require_bool_true(payload, "issuer_signature_verified", errors)
    require_bool_true(payload, "issuer_key_policy_verified", errors)
    require_false(payload, "credential_payloads_included", errors)
    require_false(payload, "holder_identities_included", errors)


def validate_commitment_root(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "root_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "tree_version", errors)
    require_recent_timestamp(
        payload,
        "published_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_root_age_secs,
    )
    require_bool_true(payload, "publisher_signature_verified", errors)
    require_bool_true(payload, "monotonic_tree_version", errors)
    require_bool_true(payload, "anchor_published", errors)
    require_false(payload, "credential_leaves_included", errors)


def validate_revocation_registry(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "revocation_list_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "revocation_list_version", errors)
    require_recent_timestamp(
        payload,
        "published_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_revocation_age_secs,
    )
    require_bool_true(payload, "publisher_signature_verified", errors)
    require_bool_true(payload, "test_revocation_probe_passed", errors)
    require_false(payload, "rollback_detected", errors)
    require_false(payload, "revoked_nonces_included", errors)
    require_non_negative_int(payload, "revoked_nonce_count", errors)


def validate_enrollment_portal(payload: dict[str, Any], errors: list[str]) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_bool_true(payload, "issuer_approval_required", errors)
    require_bool_true(payload, "renewal_flow_verified", errors)
    require_bool_true(payload, "rate_limit_configured", errors)
    require_false(payload, "pii_fields_included", errors)
    require_false(payload, "attestations_included", errors)
    validate_routes(payload, errors, required_routes=REQUIRED_ENROLLMENT_ROUTES)


def validate_juror_client(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "synced_root_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "synced_revocation_list_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "credential_store_encrypted", errors)
    require_bool_true(payload, "revocation_sync_success", errors)
    require_bool_true(payload, "proof_generation_success", errors)
    require_bool_true(payload, "credential_rotation_dry_run_success", errors)
    require_bool_true(payload, "offline_export_encrypted", errors)
    require_false(payload, "holder_identity_included", errors)
    require_false(payload, "proof_payloads_included", errors)


def validate_verifier_service(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "root_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "revocation_list_digest_hex", HEX64_LEN, errors)
    probe_count = require_positive_int(payload, "proof_probe_count", errors)
    accepted = require_positive_int(payload, "accepted_valid_proof_count", errors)
    rejected = require_positive_int(payload, "rejected_invalid_proof_count", errors)
    require_sum_equal(
        probe_count,
        (
            ("accepted_valid_proof_count", accepted),
            ("rejected_invalid_proof_count", rejected),
        ),
        "proof_probe_count",
        errors,
        skip_zero_total=True,
    )
    require_bool_true(payload, "expired_proof_rejected", errors)
    require_bool_true(payload, "revoked_proof_rejected", errors)
    require_bool_true(payload, "replay_nullifier_rejected", errors)
    require_bool_true(payload, "root_binding_verified", errors)
    require_maximum_int(
        payload,
        "max_verify_latency_ms",
        options.max_verify_latency_ms,
        errors,
        minimum=1,
    )
    require_maximum_int(
        payload,
        "max_service_lag_seconds",
        options.max_service_lag_secs,
        errors,
    )
    require_false(payload, "raw_proofs_included", errors)
    require_false(payload, "holder_identity_disclosed", errors)
    validate_routes(payload, errors, required_routes=REQUIRED_VERIFIER_ROUTES)


def validate_moderation_integration(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "root_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "revocation_list_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "pop_snapshot_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "sortition_probe_count", errors)
    require_positive_int(payload, "commit_reveal_probe_count", errors)
    require_bool_true(payload, "juror_pool_bound", errors)
    require_bool_true(payload, "moderation_case_binding_verified", errors)
    require_bool_true(payload, "duplicate_nullifier_rejected", errors)
    require_bool_true(payload, "observer_credentials_excluded", errors)
    require_false(payload, "identity_payloads_included", errors)
    require_false(payload, "credential_payloads_included", errors)


def validate_metrics_alerts(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "root_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "revocation_list_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_false(payload, "response_bodies_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "root_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "revocation_list_digest_hex", HEX64_LEN, errors)
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "issuer_key_policy_present", errors)
    require_bool_true(payload, "revocation_policy_present", errors)
    require_bool_true(payload, "retention_policy_present", errors)
    require_bool_true(payload, "manual_override_policy_present", errors)
    require_bool_true(payload, "zk_verifier_audit_passed", errors)
    require_string_not_equal(
        payload,
        "privacy_proof_system",
        "transcript_digest_v1",
        errors,
        message="privacy_proof_system must be production privacy-preserving proof backend",
    )
    require_policy_digest(payload, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_passed_status(payload, errors)

    if kind.name == "issuer_bundle":
        validate_issuer_bundle(payload, errors)
    elif kind.name == "commitment_root":
        validate_commitment_root(payload, errors, options)
    elif kind.name == "revocation_registry":
        validate_revocation_registry(payload, errors, options)
    elif kind.name == "enrollment_portal":
        validate_enrollment_portal(payload, errors)
    elif kind.name == "juror_client":
        validate_juror_client(payload, errors)
    elif kind.name == "verifier_service":
        validate_verifier_service(payload, errors, options)
    elif kind.name == "moderation_integration":
        validate_moderation_integration(payload, errors)
    elif kind.name == "metrics_alerts":
        validate_metrics_alerts(payload, errors)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS PoP rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        lambda kind, checked_payload, errors: validate_kind_specific(
            kind, checked_payload, errors, options
        ),
    )



def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    issuer_bundle_artifacts: list[dict[str, Any]] = []
    commitment_root_artifacts: list[dict[str, Any]] = []
    revocation_registry_artifacts: list[dict[str, Any]] = []
    root_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    revocation_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    files = discover_evidence_files(
        evidence_dirs,
        evidence_files,
        errors,
        reserved_output_paths=() if summary_out is None else (summary_out,),
    )
    explicit = evidence_path_identities(evidence_files, errors)

    for path in files:
        loaded = load_evidence_json_with_sha256_or_record_error(
            path, MAX_EVIDENCE_BYTES, errors
        )
        if loaded is None:
            continue
        payload, digest = loaded
        kind_name, validation_errors = validate_evidence_payload(payload, options)
        if kind_name is None:
            record_explicit_evidence_validation_errors(
                path, explicit, validation_errors, errors
            )
            continue
        artifact = build_evidence_artifact(
            path,
            digest,
            payload,
            validation_errors,
            FINGERPRINT_FIELDS,
        )
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact)
        if evidence_artifact_is_valid(artifact):
            if kind_name == "issuer_bundle":
                issuer_bundle_artifacts.append(artifact)
            if kind_name == "commitment_root":
                commitment_root_artifacts.append(artifact)
            if kind_name == "revocation_registry":
                revocation_registry_artifacts.append(artifact)
            if kind_name in ROOT_BOUND_KINDS:
                root_bound_artifacts.append((kind_name, artifact))
            if kind_name in REVOCATION_BOUND_KINDS:
                revocation_bound_artifacts.append((kind_name, artifact))
        record_evidence_validation_errors(path, validation_errors, errors)

    issuer_root_digests = evidence_artifact_digest_set(
        issuer_bundle_artifacts,
        "root_digest_hex",
    )
    commitment_root_digests = evidence_artifact_digest_set(
        commitment_root_artifacts,
        "root_digest_hex",
    )
    if issuer_root_digests and commitment_root_digests and (
        issuer_root_digests != commitment_root_digests
    ):
        shared_roots = issuer_root_digests & commitment_root_digests
        error = "issuer_bundle root_digest_hex must match commitment_root root_digest_hex"
        record_evidence_digest_mismatch_errors(
            artifacts=issuer_bundle_artifacts,
            digest_field="root_digest_hex",
            allowed_digests=shared_roots,
            errors=errors,
            error=error,
        )
        record_evidence_digest_mismatch_errors(
            artifacts=commitment_root_artifacts,
            digest_field="root_digest_hex",
            allowed_digests=shared_roots,
            errors=errors,
            error=error,
        )

    issuer_revocation_digests = evidence_artifact_digest_set(
        issuer_bundle_artifacts,
        "revocation_list_digest_hex",
    )
    registry_revocation_digests = evidence_artifact_digest_set(
        revocation_registry_artifacts,
        "revocation_list_digest_hex",
    )
    if issuer_revocation_digests and registry_revocation_digests and (
        issuer_revocation_digests != registry_revocation_digests
    ):
        shared_revocations = issuer_revocation_digests & registry_revocation_digests
        error = (
            "issuer_bundle revocation_list_digest_hex must match "
            "revocation_registry revocation_list_digest_hex"
        )
        record_evidence_digest_mismatch_errors(
            artifacts=issuer_bundle_artifacts,
            digest_field="revocation_list_digest_hex",
            allowed_digests=shared_revocations,
            errors=errors,
            error=error,
        )
        record_evidence_digest_mismatch_errors(
            artifacts=revocation_registry_artifacts,
            digest_field="revocation_list_digest_hex",
            allowed_digests=shared_revocations,
            errors=errors,
            error=error,
        )

    valid_root_digests: set[str] = set()
    issuer_root_digests = evidence_artifact_digest_set(
        issuer_bundle_artifacts,
        "root_digest_hex",
    )
    commitment_root_digests = evidence_artifact_digest_set(
        commitment_root_artifacts,
        "root_digest_hex",
    )
    if issuer_root_digests and issuer_root_digests == commitment_root_digests:
        valid_root_digests = set(issuer_root_digests)

    valid_revocation_digests: set[str] = set()
    issuer_revocation_digests = evidence_artifact_digest_set(
        issuer_bundle_artifacts,
        "revocation_list_digest_hex",
    )
    registry_revocation_digests = evidence_artifact_digest_set(
        revocation_registry_artifacts,
        "revocation_list_digest_hex",
    )
    if (
        issuer_revocation_digests
        and issuer_revocation_digests == registry_revocation_digests
    ):
        valid_revocation_digests = set(issuer_revocation_digests)

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=root_bound_artifacts,
        valid_anchor_digests=valid_root_digests,
        digest_field="root_digest_hex",
        digest_field_by_kind={"juror_client": "synced_root_digest_hex"},
        errors=errors,
        binding_error_template=(
            "{kind_name} root binding must match a valid "
            "issuer_bundle/commitment_root root_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} root binding must match a valid "
            "issuer_bundle/commitment_root root_digest_hex"
        ),
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=revocation_bound_artifacts,
        valid_anchor_digests=valid_revocation_digests,
        digest_field="revocation_list_digest_hex",
        digest_field_by_kind={
            "juror_client": "synced_revocation_list_digest_hex"
        },
        errors=errors,
        binding_error_template=(
            "{kind_name} revocation binding must match a valid "
            "issuer_bundle/revocation_registry revocation_list_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} revocation binding must match a valid "
            "issuer_bundle/revocation_registry revocation_list_digest_hex"
        ),
    )

    required = build_required_evidence_summary(
        required_kinds,
        artifacts_by_kind,
        evidence_schema_by_kind(KIND_BY_NAME),
        errors,
        evidence_label="rollout",
    )

    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": evidence_gate_status(errors),
        "required_kinds": required_evidence_kind_names(required_kinds),
        "thresholds": {
            "max_root_age_secs": options.max_root_age_secs,
            "max_revocation_age_secs": options.max_revocation_age_secs,
            "max_service_lag_secs": options.max_service_lag_secs,
            "max_verify_latency_ms": options.max_verify_latency_ms,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_root_digests": sorted(valid_root_digests),
        "valid_revocation_list_digests": sorted(valid_revocation_digests),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SFM-4b1 PoP credential rollout evidence artifacts."
    )
    parser.add_argument(
        "--evidence-dir",
        action="append",
        type=Path,
        default=[],
        help="Directory containing rollout evidence JSON artifacts.",
    )
    parser.add_argument(
        "--evidence",
        action="append",
        type=Path,
        default=[],
        help="Explicit rollout evidence JSON artifact.",
    )
    parser.add_argument(
        "--require-kind",
        action="append",
        default=[],
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-4b1 kinds.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        default=int(time.time()),
        help="Validator clock used for age checks. Defaults to current Unix time.",
    )
    parser.add_argument(
        "--max-root-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_ROOT_AGE_SECS,
    )
    parser.add_argument(
        "--max-revocation-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_REVOCATION_AGE_SECS,
    )
    parser.add_argument(
        "--max-service-lag-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SERVICE_LAG_SECS,
    )
    parser.add_argument(
        "--max-verify-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_VERIFY_LATENCY_MS,
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_error_lines((str(error),))
        return 2
    try:
        args = parser.parse_args(expanded_args)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    try:
        required_kinds = parse_required_evidence_kinds(
            args.require_kind,
            allowed_kinds=KIND_BY_NAME,
            default_required=DEFAULT_REQUIRED_KINDS,
        )
    except ValueError as error:
        emit_checker_error_lines((str(error),))
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_root_age_secs=args.max_root_age_secs,
        max_revocation_age_secs=args.max_revocation_age_secs,
        max_service_lag_secs=args.max_service_lag_secs,
        max_verify_latency_ms=args.max_verify_latency_ms,
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir, args.evidence, required_kinds, options, args.summary_out
    )
    rendered_summary, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2

    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS PoP credential rollout evidence is incomplete:",
            errors,
        )
        return 1

    emit_checker_notice(
        "SoraFS PoP credential rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
