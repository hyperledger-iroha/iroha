#!/usr/bin/env python3
"""Build payload-free SoraFS PoP credential rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_pop_credentials_rollout_evidence import (  # noqa: E402
    COMMIT_REVEAL_PROBE_LABEL_ERROR,
    COMMIT_REVEAL_PROBE_LABEL_PATTERN,
    CREDENTIAL_LABEL_ERROR,
    CREDENTIAL_LABEL_PATTERN,
    DEFAULT_MAX_REVOCATION_AGE_SECS,
    DEFAULT_MAX_ROOT_AGE_SECS,
    DEFAULT_MAX_SERVICE_LAG_SECS,
    DEFAULT_MAX_VERIFY_LATENCY_MS,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    FORBIDDEN_ISSUER_ID_MARKERS,
    INVALID_PROOF_PROBE_LABEL_ERROR,
    INVALID_PROOF_PROBE_LABEL_PATTERN,
    ISSUER_ID_ERROR,
    ISSUER_ID_PATTERN,
    KIND_BY_NAME,
    REQUIRED_ENROLLMENT_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_VERIFIER_ROUTES,
    SORTITION_PROBE_LABEL_ERROR,
    SORTITION_PROBE_LABEL_PATTERN,
    VALID_PROOF_PROBE_LABEL_ERROR,
    VALID_PROOF_PROBE_LABEL_PATTERN,
    ValidationOptions,
    validate_evidence_payload,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    fsync_checker_output_parent,
    write_all_checker_summary_bytes,
    validate_checker_output_parent,
)
from sorafs_path_identity import path_diagnostic_label  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


CANARY_KINDS = tuple(KIND_BY_NAME)
HEX64_LEN = 64
ROOT_DIGEST_KINDS = (
    "issuer_bundle",
    "commitment_root",
    "juror_client",
    "verifier_service",
    "moderation_integration",
    "metrics_alerts",
    "governance_approval",
)
REVOCATION_DIGEST_KINDS = (
    "issuer_bundle",
    "revocation_registry",
    "juror_client",
    "verifier_service",
    "moderation_integration",
    "metrics_alerts",
    "governance_approval",
)
TRUE_CLAIMS: dict[str, tuple[str, ...]] = {
    "issuer_bundle": (
        "canonical_norito_verified",
        "issuer_signature_verified",
        "issuer_key_policy_verified",
    ),
    "commitment_root": (
        "publisher_signature_verified",
        "monotonic_tree_version",
        "anchor_published",
    ),
    "revocation_registry": (
        "publisher_signature_verified",
        "test_revocation_probe_passed",
    ),
    "enrollment_portal": (
        "issuer_approval_required",
        "renewal_flow_verified",
        "rate_limit_configured",
    ),
    "juror_client": (
        "credential_store_encrypted",
        "revocation_sync_success",
        "proof_generation_success",
        "credential_rotation_dry_run_success",
        "offline_export_encrypted",
    ),
    "verifier_service": (
        "expired_proof_rejected",
        "revoked_proof_rejected",
        "replay_nullifier_rejected",
        "root_binding_verified",
    ),
    "moderation_integration": (
        "juror_pool_bound",
        "moderation_case_binding_verified",
        "duplicate_nullifier_rejected",
        "observer_credentials_excluded",
    ),
    "metrics_alerts": (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
    ),
    "governance_approval": (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "issuer_key_policy_present",
        "revocation_policy_present",
        "retention_policy_present",
        "manual_override_policy_present",
        "zk_verifier_audit_passed",
    ),
}
FORCED_FALSE_FIELDS: dict[str, tuple[str, ...]] = {
    "issuer_bundle": ("credential_payloads_included", "holder_identities_included"),
    "commitment_root": ("credential_leaves_included",),
    "revocation_registry": ("rollback_detected", "revoked_nonces_included"),
    "enrollment_portal": ("pii_fields_included", "attestations_included"),
    "juror_client": ("holder_identity_included", "proof_payloads_included"),
    "verifier_service": ("raw_proofs_included", "holder_identity_disclosed"),
    "moderation_integration": (
        "identity_payloads_included",
        "credential_payloads_included",
    ),
    "metrics_alerts": ("critical_alerts_firing", "response_bodies_included"),
    "governance_approval": (),
}


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into canonical strings."""

    items: list[str] = []
    for value in values:
        for item in value.split(","):
            stripped = item.strip()
            if stripped:
                items.append(stripped)
    return items


def validate_name_set(
    values: Iterable[str],
    *,
    allowed: Sequence[str],
    option: str,
    errors: list[str],
) -> list[str]:
    """Return allowed-order values, requiring complete known non-duplicate coverage."""

    values = tuple(values)
    allowed_set = frozenset(allowed)
    value_set = frozenset(values)
    if len(value_set) != len(values):
        errors.append(f"{option} must not contain duplicates")
    if any(name not in allowed_set for name in value_set):
        errors.append(f"{option} contains an unknown value")
    missing = [name for name in allowed if name not in value_set]
    if missing:
        errors.append(f"{option} must include every required value")
    return [name for name in allowed if name in value_set]


def validate_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    kind: str,
    count_option: str,
    errors: list[str],
    pattern: re.Pattern[str] | None = None,
    label_error: str | None = None,
) -> list[str]:
    """Return reviewed unique inventory labels whose count matches a CLI count."""

    items = list(values)
    if not items:
        errors.append(f"{option} is required for {kind}")
    for index, item in enumerate(items):
        validate_canonical_string(item, label=f"{option}[{index}]", errors=errors)
        if pattern is not None and pattern.fullmatch(item) is None:
            if label_error is None:
                errors.append(f"{option} has malformed inventory label")
            else:
                errors.append(render_inventory_label_error(label_error, option))
            continue
        forbidden = sorted(
            marker
            for marker in FORBIDDEN_INVENTORY_LABEL_MARKERS
            if marker in item.split("-")
        )
        if forbidden:
            errors.append(f"{option} must not contain non-production markers {forbidden}")
    unique_items = set(items)
    if len(unique_items) != len(items):
        errors.append(f"{option} must not contain duplicates")
    if len(unique_items) != expected_count:
        errors.append(f"{option} unique values must match {count_option}")
    return items


def render_inventory_label_error(label_error: str, option: str) -> str:
    """Render checker inventory-label diagnostics against a CLI option."""

    return (
        label_error.replace("credentials[].name", option)
        .replace("probes[].name", option)
        .replace("sortition_probes[].name", option)
        .replace("commit_reveal_probes[].name", option)
    )


def validate_output_path(path: Path, errors: list[str]) -> None:
    """Reject unsafe output targets before writing a canary artifact."""

    if not isinstance(path, Path):
        errors.append(f"--out `{path_diagnostic_label(path)}` must be a path")
        return
    try:
        if path.is_symlink():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a symlink")
            return
        if path.exists() and path.is_dir():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a directory")
            return
    except (OSError, RuntimeError) as error:
        del error
        errors.append(f"--out `{path_diagnostic_label(path)}` cannot be inspected")
        return
    validate_checker_output_parent(path, errors, label="--out")


def validate_hex64(value: str | None, *, option: str, errors: list[str]) -> None:
    """Validate an exact lowercase 32-byte digest hex string."""

    if (
        not isinstance(value, str)
        or len(value) != HEX64_LEN
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{option} must be exact lowercase 32-byte hex")


def validate_canonical_string(value: str | None, *, label: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control characters."""

    if (
        not isinstance(value, str)
        or not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{label} must be a non-empty canonical string")


def validate_issuer_id_arg(value: str | None, *, errors: list[str]) -> None:
    """Require a reviewed lowercase PoP issuer identifier."""

    validate_canonical_string(value, label="--issuer-id", errors=errors)
    if not isinstance(value, str):
        return
    if ISSUER_ID_PATTERN.fullmatch(value) is None:
        errors.append(ISSUER_ID_ERROR.replace("issuer_id", "--issuer-id"))
        return
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_ISSUER_ID_MARKERS
        if marker in value.split("-")
    )
    if forbidden:
        errors.append(f"--issuer-id must not contain non-production markers {forbidden}")


def require_kind_options(
    args: argparse.Namespace,
    errors: list[str],
    required: Sequence[tuple[str, Any]],
) -> None:
    """Require kind-specific options by stable CLI flag."""

    for option, value in required:
        if value is None:
            errors.append(f"{option} is required for {args.kind}")


def build_common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by PoP credential canary payloads."""

    payload: dict[str, Any] = {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
    }
    if args.kind in ROOT_DIGEST_KINDS:
        field = "synced_root_digest_hex" if args.kind == "juror_client" else "root_digest_hex"
        payload[field] = args.root_digest_hex
    if args.kind in REVOCATION_DIGEST_KINDS:
        field = (
            "synced_revocation_list_digest_hex"
            if args.kind == "juror_client"
            else "revocation_list_digest_hex"
        )
        payload[field] = args.revocation_list_digest_hex
    return payload


def apply_verified_claims(payload: dict[str, Any], args: argparse.Namespace) -> None:
    """Populate explicitly verified true claims and forced payload-free false flags."""

    for claim in TRUE_CLAIMS[args.kind]:
        payload[claim] = claim in args.verified_claims
    for field in FORCED_FALSE_FIELDS[args.kind]:
        payload[field] = False


def build_route_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free authenticated route probe records."""

    return [
        {
            "name": route,
            "passed": True,
            "status_code": args.route_status_code,
            "authz_enforced": True,
            "signature_verified": True,
        }
        for route in args.routes
    ]


def build_inventory_records(names: Sequence[str]) -> list[dict[str, str]]:
    """Build reviewed payload-free inventory records."""

    return [{"name": name} for name in names]


def build_proof_probe_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build reviewed payload-free verifier proof probe records."""

    return [
        {"name": name, "accepted": True}
        for name in args.accepted_proof_probes
    ] + [
        {"name": name, "accepted": False}
        for name in args.rejected_proof_probes
    ]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free PoP credential rollout canary payload."""

    payload = build_common_payload(args)
    apply_verified_claims(payload, args)
    if args.kind == "issuer_bundle":
        payload.update(
            {
                "issuer_id": args.issuer_id,
                "bundle_id_hex": args.bundle_id_hex,
                "credential_count": args.credential_count,
                "credentials": build_inventory_records(args.credentials),
                "signed_credential_count": args.credential_count,
            }
        )
    elif args.kind == "commitment_root":
        payload.update(
            {
                "tree_version": args.tree_version,
                "published_at_unix": args.published_at_unix,
            }
        )
    elif args.kind == "revocation_registry":
        payload.update(
            {
                "revocation_list_version": args.revocation_list_version,
                "published_at_unix": args.published_at_unix,
                "revoked_nonce_count": args.revoked_nonce_count,
            }
        )
    elif args.kind == "enrollment_portal":
        routes = build_route_records(args)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
            }
        )
    elif args.kind == "verifier_service":
        routes = build_route_records(args)
        probes = build_proof_probe_records(args)
        payload.update(
            {
                "policy_digest_hex": args.policy_digest_hex,
                "proof_probe_count": len(probes),
                "accepted_valid_proof_count": args.accepted_valid_proof_count,
                "rejected_invalid_proof_count": args.rejected_invalid_proof_count,
                "probes": probes,
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "max_verify_latency_ms": args.max_verify_latency_ms,
                "max_service_lag_seconds": args.max_service_lag_seconds,
                "routes": routes,
            }
        )
    elif args.kind == "moderation_integration":
        payload.update(
            {
                "pop_snapshot_digest_hex": args.pop_snapshot_digest_hex,
                "sortition_probe_count": args.sortition_probe_count,
                "sortition_probes": build_inventory_records(args.sortition_probes),
                "commit_reveal_probe_count": args.commit_reveal_probe_count,
                "commit_reveal_probes": build_inventory_records(
                    args.commit_reveal_probes
                ),
            }
        )
    elif args.kind == "metrics_alerts":
        payload.update(
            {
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "config_source": "iroha_config",
                "privacy_proof_system": args.privacy_proof_system,
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    return payload


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    if args.kind in ROOT_DIGEST_KINDS:
        validate_hex64(args.root_digest_hex, option="--root-digest-hex", errors=errors)
    if args.kind in REVOCATION_DIGEST_KINDS:
        validate_hex64(
            args.revocation_list_digest_hex,
            option="--revocation-list-digest-hex",
            errors=errors,
        )
    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=TRUE_CLAIMS[args.kind],
        option="--verified-claim",
        errors=errors,
    )
    if args.kind == "issuer_bundle":
        require_kind_options(
            args,
            errors,
            (
                ("--issuer-id", args.issuer_id),
                ("--bundle-id-hex", args.bundle_id_hex),
                ("--credential-count", args.credential_count),
            ),
        )
        validate_issuer_id_arg(args.issuer_id, errors=errors)
        validate_hex64(args.bundle_id_hex, option="--bundle-id-hex", errors=errors)
        args.credentials = validate_reviewed_inventory(
            split_csv_values(args.credential),
            expected_count=args.credential_count or 0,
            option="--credential",
            kind="issuer_bundle",
            count_option="--credential-count",
            pattern=CREDENTIAL_LABEL_PATTERN,
            label_error=CREDENTIAL_LABEL_ERROR,
            errors=errors,
        )
    elif args.kind == "commitment_root":
        require_kind_options(
            args,
            errors,
            (
                ("--tree-version", args.tree_version),
                ("--published-at-unix", args.published_at_unix),
            ),
        )
    elif args.kind == "revocation_registry":
        require_kind_options(
            args,
            errors,
            (
                ("--revocation-list-version", args.revocation_list_version),
                ("--published-at-unix", args.published_at_unix),
                ("--revoked-nonce-count", args.revoked_nonce_count),
            ),
        )
    elif args.kind == "enrollment_portal":
        args.routes = validate_name_set(
            split_csv_values(args.route),
            allowed=REQUIRED_ENROLLMENT_ROUTES,
            option="--route",
            errors=errors,
        )
    elif args.kind == "verifier_service":
        args.routes = validate_name_set(
            split_csv_values(args.route),
            allowed=REQUIRED_VERIFIER_ROUTES,
            option="--route",
            errors=errors,
        )
        require_kind_options(
            args,
            errors,
            (
                ("--policy-digest-hex", args.policy_digest_hex),
                ("--accepted-valid-proof-count", args.accepted_valid_proof_count),
                ("--rejected-invalid-proof-count", args.rejected_invalid_proof_count),
                ("--max-verify-latency-ms", args.max_verify_latency_ms),
                ("--max-service-lag-seconds", args.max_service_lag_seconds),
            ),
        )
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)
        args.accepted_proof_probes = validate_reviewed_inventory(
            split_csv_values(args.accepted_proof_probe),
            expected_count=args.accepted_valid_proof_count or 0,
            option="--accepted-proof-probe",
            kind="verifier_service",
            count_option="--accepted-valid-proof-count",
            pattern=VALID_PROOF_PROBE_LABEL_PATTERN,
            label_error=VALID_PROOF_PROBE_LABEL_ERROR,
            errors=errors,
        )
        args.rejected_proof_probes = validate_reviewed_inventory(
            split_csv_values(args.rejected_proof_probe),
            expected_count=args.rejected_invalid_proof_count or 0,
            option="--rejected-proof-probe",
            kind="verifier_service",
            count_option="--rejected-invalid-proof-count",
            pattern=INVALID_PROOF_PROBE_LABEL_PATTERN,
            label_error=INVALID_PROOF_PROBE_LABEL_ERROR,
            errors=errors,
        )
        proof_probe_names = args.accepted_proof_probes + args.rejected_proof_probes
        if len(set(proof_probe_names)) != len(proof_probe_names):
            errors.append(
                "--accepted-proof-probe and --rejected-proof-probe must not overlap"
            )
    elif args.kind == "moderation_integration":
        require_kind_options(
            args,
            errors,
            (
                ("--pop-snapshot-digest-hex", args.pop_snapshot_digest_hex),
                ("--sortition-probe-count", args.sortition_probe_count),
                ("--commit-reveal-probe-count", args.commit_reveal_probe_count),
            ),
        )
        validate_hex64(
            args.pop_snapshot_digest_hex,
            option="--pop-snapshot-digest-hex",
            errors=errors,
        )
        args.sortition_probes = validate_reviewed_inventory(
            split_csv_values(args.sortition_probe),
            expected_count=args.sortition_probe_count or 0,
            option="--sortition-probe",
            kind="moderation_integration",
            count_option="--sortition-probe-count",
            pattern=SORTITION_PROBE_LABEL_PATTERN,
            label_error=SORTITION_PROBE_LABEL_ERROR,
            errors=errors,
        )
        args.commit_reveal_probes = validate_reviewed_inventory(
            split_csv_values(args.commit_reveal_probe),
            expected_count=args.commit_reveal_probe_count or 0,
            option="--commit-reveal-probe",
            kind="moderation_integration",
            count_option="--commit-reveal-probe-count",
            pattern=COMMIT_REVEAL_PROBE_LABEL_PATTERN,
            label_error=COMMIT_REVEAL_PROBE_LABEL_ERROR,
            errors=errors,
        )
    elif args.kind == "metrics_alerts":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    elif args.kind == "governance_approval":
        require_kind_options(
            args,
            errors,
            (
                ("--privacy-proof-system", args.privacy_proof_system),
                ("--policy-digest-hex", args.policy_digest_hex),
            ),
        )
        validate_canonical_string(
            args.privacy_proof_system,
            label="--privacy-proof-system",
            errors=errors,
        )
        if args.privacy_proof_system == "transcript_digest_v1":
            errors.append(
                "--privacy-proof-system must be a production privacy-preserving proof backend"
            )
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    if args.max_verify_latency_ms is not None and (
        args.max_verify_latency_ms > DEFAULT_MAX_VERIFY_LATENCY_MS
    ):
        errors.append(f"--max-verify-latency-ms must be <= {DEFAULT_MAX_VERIFY_LATENCY_MS}")
    if args.max_service_lag_seconds is not None and (
        args.max_service_lag_seconds > DEFAULT_MAX_SERVICE_LAG_SECS
    ):
        errors.append(f"--max-service-lag-seconds must be <= {DEFAULT_MAX_SERVICE_LAG_SECS}")
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_root_age_secs=DEFAULT_MAX_ROOT_AGE_SECS,
        max_revocation_age_secs=DEFAULT_MAX_REVOCATION_AGE_SECS,
        max_service_lag_secs=DEFAULT_MAX_SERVICE_LAG_SECS,
        max_verify_latency_ms=DEFAULT_MAX_VERIFY_LATENCY_MS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the PoP credential gate contract."""

    kind, errors = validate_evidence_payload(payload, validation_options(args))
    if kind != args.kind:
        errors.append(f"generated canary must validate as {args.kind}")
    return errors


def write_payload_atomic(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write the canary JSON atomically without following output symlinks."""

    text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    parent = path.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        del error
        return [f"--out parent `{path_diagnostic_label(parent)}` cannot be created"]
    tmp_name = f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    tmp_path = parent / tmp_name
    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        if nofollow:
            flags |= nofollow
        fd = os.open(tmp_path, flags, 0o600)
        write_all_checker_summary_bytes(fd, text.encode("utf-8"))
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.replace(tmp_path, path)
        parent_sync_errors = fsync_checker_output_parent(path, label="--out")
        if parent_sync_errors:
            return parent_sync_errors
    except (OSError, RuntimeError) as error:
        del error
        try:
            if fd >= 0:
                os.close(fd)
        finally:
            try:
                tmp_path.unlink()
            except FileNotFoundError:
                pass
            except (OSError, RuntimeError):
                pass
        return [f"--out `{path_diagnostic_label(path)}` cannot be written"]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SFM-4b1 PoP credential canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--root-digest-hex")
    parser.add_argument("--revocation-list-digest-hex")
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--issuer-id")
    parser.add_argument("--bundle-id-hex")
    parser.add_argument("--credential-count", type=positive_int_arg)
    parser.add_argument("--credential", action="append", default=[])
    parser.add_argument("--tree-version", type=positive_int_arg)
    parser.add_argument("--revocation-list-version", type=positive_int_arg)
    parser.add_argument("--published-at-unix", type=positive_int_arg)
    parser.add_argument("--revoked-nonce-count", type=non_negative_int_arg)
    parser.add_argument("--route", action="append", default=[])
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--accepted-valid-proof-count", type=positive_int_arg)
    parser.add_argument("--rejected-invalid-proof-count", type=positive_int_arg)
    parser.add_argument("--accepted-proof-probe", action="append", default=[])
    parser.add_argument("--rejected-proof-probe", action="append", default=[])
    parser.add_argument("--max-verify-latency-ms", type=positive_int_arg)
    parser.add_argument("--max-service-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--pop-snapshot-digest-hex")
    parser.add_argument("--sortition-probe-count", type=positive_int_arg)
    parser.add_argument("--sortition-probe", action="append", default=[])
    parser.add_argument("--commit-reveal-probe-count", type=positive_int_arg)
    parser.add_argument("--commit-reveal-probe", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--privacy-proof-system")
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
        return parser.parse_args(expanded_args)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    errors = validate_inputs(args)
    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS PoP credential canary inputs are incomplete:",
            errors,
        )
        return 2

    payload = build_payload(args)
    payload_errors = validate_generated_payload(payload, args)
    if payload_errors:
        emit_checker_error_lines(payload_errors)
        return 2

    write_errors = write_payload_atomic(args.out, payload)
    if write_errors:
        emit_checker_error_lines(write_errors)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
