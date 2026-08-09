#!/usr/bin/env python3
"""Build payload-free SoraFS Governance DAG rollout canary artifacts."""

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

from check_sorafs_governance_dag_rollout_evidence import (  # noqa: E402
    BLOCK_REF_LABEL_ERROR,
    BLOCK_REF_LABEL_PATTERN,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_HEAD_AGE_SECS,
    DEFAULT_MAX_PIN_LAG_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_BLOCKS,
    DEFAULT_MIN_PAYLOAD_KINDS,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    INGRESS_ENFORCEMENT,
    KIND_BY_NAME,
    KUBO_CID_MULTIHASH,
    KUBO_CID_VERSION,
    KUBO_UNIXFS_CHUNK_SIZE_BYTES,
    KUBO_UNIXFS_MAX_LINKS_PER_NODE,
    KUBO_UNIXFS_PROFILE,
    MIRROR_RETENTION_MAX_BYTES,
    MIRROR_RETENTION_MAX_ENTRIES,
    REPLAY_POSTURE,
    REQUIRED_DASHBOARD_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_PAYLOAD_KINDS,
    STEADY_AUDIT_MAX_BYTES_PER_POLL,
    STEADY_AUDIT_MAX_ENTRIES_PER_POLL,
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
from sorafs_path_identity import (  # noqa: E402
    diagnostic_text_is_canonical,
    error_diagnostic_label,
    path_diagnostic_label,
)
from sorafs_evidence_validation import (  # noqa: E402
    forbidden_non_production_markers,
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


CANARY_KINDS = tuple(KIND_BY_NAME)
HEX64_LEN = 64
PUBLIC_HEAD_KINDS = tuple(kind for kind in CANARY_KINDS if kind != "ingest_service")
POLICY_DIGEST_KINDS = ("publisher_service", "governance_approval")
INGRESS_QUALIFICATION_DIGEST_KINDS = (
    "publisher_service",
    "governance_approval",
)
INGRESS_BINDING_DIGEST_KINDS = INGRESS_QUALIFICATION_DIGEST_KINDS
TRUE_CLAIMS: dict[str, tuple[str, ...]] = {
    "ingest_service": (
        "daemonized",
        "payload_validation_enabled",
        "publisher_signature_verified",
        "dedupe_by_digest_enabled",
        "quarantine_invalid_blocks",
    ),
    "publisher_service": (
        "dag_builder_daemonized",
        "unixfs_raw_leaves",
        "unixfs_balanced_layout",
        "locally_derived_cids_verified",
        "signed_http_head_cas_enabled",
        "strong_single_etag_verified",
        "conditional_cas_readback_verified",
        "signed_head_verified",
        "parent_chain_verified",
        "objects_pinned",
        "authenticated_ingress_qualified",
        "ingress_scope_binding_verified",
    ),
    "mirror_datastore": (
        "sealed_typed_store_enabled",
        "query_service_enabled",
        "mirror_index_verified",
        "head_lookup_verified",
        "block_lookup_verified",
        "node_lookup_verified",
        "digest_lookup_verified",
        "exact_retained_source_suffix_verified",
        "fresh_checkpoint_coherent_reads_verified",
        "liveness_bound_reader_verified",
    ),
    "operator_recovery": (
        "live_head_fetch_verified",
        "public_checkpoint_published",
        "checkpoint_recovery_verified",
        "derived_mirror_recovery_verified",
        "recovered_head_matches_public_head",
        "post_loss_repair_verified",
        "head_object_repaired_with_same_cid",
        "block_object_repaired_with_same_cid",
        "public_head_unchanged_during_repair",
    ),
    "dashboard_api": (
        "service_mirror_capability_installed",
        "fresh_checkpoint_coherent_reads_verified",
        "liveness_bound_reader_verified",
        "unready_reader_rejected",
        "reader_withdrawal_verified",
    ),
    "observability": (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "publication_metrics_present",
        "first_full_audit_verified",
        "readiness_withheld_until_full_audit",
        "bounded_rotating_audit_verified",
    ),
    "publication_e2e": (
        "local_kubo_tests_passed",
        "deterministic_unixfs_profile_verified",
        "signed_http_head_resolved",
        "strong_single_etag_cas_verified",
        "authenticated_ingress_qualification_verified",
        "replay_attack_rejected",
        "block_replay_verified",
        "duplicate_payload_rejected",
        "invalid_parent_quarantined",
        "post_loss_same_cid_repair_verified",
        "bounded_rotating_audit_verified",
        "fresh_torii_reads_verified",
        "stopped_service_reads_rejected",
        "publisher_key_failure_tested",
    ),
    "governance_approval": (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "publisher_keys_governed",
        "signed_http_head_endpoint_governed",
        "ingress_receiver_policy_governed",
        "replay_namespace_governed",
        "fixed_retention_contract_bound",
    ),
}
FORCED_FALSE_FIELDS: dict[str, tuple[str, ...]] = {
    "ingest_service": ("payload_bytes_included",),
    "publisher_service": ("raw_head_included",),
    "mirror_datastore": ("mirror_drift_detected", "raw_blocks_included"),
    "operator_recovery": ("raw_checkpoint_included",),
    "dashboard_api": ("response_bodies_included",),
    "observability": ("critical_alerts_firing", "response_bodies_included"),
    "publication_e2e": ("raw_blocks_included",),
    "governance_approval": (),
}


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into exact strings."""

    items: list[str] = []
    for value in values:
        items.extend(value.split(","))
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
        validate_canonical_string(item, option=f"{option}[{index}]", errors=errors)
        if pattern is None:
            continue
        if pattern.fullmatch(item) is None:
            if label_error is None:
                errors.append(f"{option} has malformed inventory label")
            else:
                errors.append(render_inventory_label_error(label_error, option))
            continue
        forbidden = forbidden_non_production_markers(item, FORBIDDEN_INVENTORY_LABEL_MARKERS)
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

    return label_error.replace("block_refs entries", option)


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
        or value == "0" * HEX64_LEN
    ):
        errors.append(f"{option} must be exact lowercase 32-byte hex")


def validate_canonical_string(value: str | None, *, option: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control/format text."""

    if not diagnostic_text_is_canonical(value):
        errors.append(f"{option} must be a non-empty canonical string")


def required_positive(value: int | None, *, option: str, errors: list[str]) -> int:
    """Return a required positive integer value, recording a stable error otherwise."""

    if value is None:
        errors.append(f"{option} is required for {option_kind(errors)}")
        return 0
    return value


def option_kind(errors: list[str]) -> str:
    """Return a generic kind label for required-option diagnostics."""

    del errors
    return "this canary kind"


def build_common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by Governance DAG canary payloads."""

    payload: dict[str, Any] = {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
    }
    if args.kind in PUBLIC_HEAD_KINDS:
        payload["public_head_cid_hex"] = args.public_head_cid_hex
    return payload


def apply_verified_claims(payload: dict[str, Any], args: argparse.Namespace) -> None:
    """Populate explicitly verified true claims and forced payload-free false flags."""

    for claim in TRUE_CLAIMS[args.kind]:
        payload[claim] = claim in args.verified_claims
    for field in FORCED_FALSE_FIELDS[args.kind]:
        payload[field] = False


def build_routes(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free dashboard route probe records."""

    return [
        {
            "name": route,
            "passed": True,
            "status_code": args.route_status_code,
            "body_blake3_hex": args.route_body_blake3_hex,
            "latency_ms": args.route_latency_ms,
            "publisher_identity_present": True,
            "verification_valid": True,
        }
        for route in args.routes
    ]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free Governance DAG canary payload."""

    payload = build_common_payload(args)
    apply_verified_claims(payload, args)
    if args.kind == "ingest_service":
        payload.update(
            {
                "source_count": args.source_count,
                "payload_kinds": args.payload_kinds,
            }
        )
    elif args.kind == "publisher_service":
        payload.update(
            {
                "kubo_unixfs_profile": KUBO_UNIXFS_PROFILE,
                "unixfs_chunk_size_bytes": KUBO_UNIXFS_CHUNK_SIZE_BYTES,
                "unixfs_max_links_per_node": KUBO_UNIXFS_MAX_LINKS_PER_NODE,
                "cid_version": KUBO_CID_VERSION,
                "cid_multihash": KUBO_CID_MULTIHASH,
                "ingress_enforcement": INGRESS_ENFORCEMENT,
                "replay_posture": REPLAY_POSTURE,
                "receiver_policy_digest_hex": args.receiver_policy_digest_hex,
                "replay_namespace_digest_hex": args.replay_namespace_digest_hex,
                "replica_set_digest_hex": args.replica_set_digest_hex,
                "kubo_ingress_binding_digest_hex": args.kubo_ingress_binding_digest_hex,
                "signed_head_ingress_binding_digest_hex": (
                    args.signed_head_ingress_binding_digest_hex
                ),
                "policy_digest_hex": args.policy_digest_hex,
                "pin_lag_seconds": args.pin_lag_seconds,
                "head_age_seconds": args.head_age_seconds,
                "block_count": args.block_count,
                "block_refs": args.block_refs,
                "payload_kind_count": len(args.payload_kinds),
                "payload_kinds": args.payload_kinds,
            }
        )
    elif args.kind == "mirror_datastore":
        payload.update(
            {
                "retention_max_entries": MIRROR_RETENTION_MAX_ENTRIES,
                "retention_max_bytes": MIRROR_RETENTION_MAX_BYTES,
                "missing_block_count": 0,
            }
        )
    elif args.kind == "operator_recovery":
        payload["checkpoint_digest_hex"] = args.checkpoint_digest_hex
    elif args.kind == "dashboard_api":
        routes = build_routes(args)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
            }
        )
    elif args.kind == "observability":
        payload.update(
            {
                "audit_max_entries_per_poll": STEADY_AUDIT_MAX_ENTRIES_PER_POLL,
                "audit_max_bytes_per_poll": STEADY_AUDIT_MAX_BYTES_PER_POLL,
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
            }
        )
    elif args.kind == "publication_e2e":
        payload.update(
            {
                "block_count": args.block_count,
                "block_refs": args.block_refs,
                "payload_kind_count": len(args.payload_kinds),
                "payload_kinds": args.payload_kinds,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
                "receiver_policy_digest_hex": args.receiver_policy_digest_hex,
                "replay_namespace_digest_hex": args.replay_namespace_digest_hex,
                "replica_set_digest_hex": args.replica_set_digest_hex,
                "kubo_ingress_binding_digest_hex": args.kubo_ingress_binding_digest_hex,
                "signed_head_ingress_binding_digest_hex": (
                    args.signed_head_ingress_binding_digest_hex
                ),
                "retention_max_entries": MIRROR_RETENTION_MAX_ENTRIES,
                "retention_max_bytes": MIRROR_RETENTION_MAX_BYTES,
            }
        )
    return payload


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    if args.kind in PUBLIC_HEAD_KINDS:
        validate_hex64(
            args.public_head_cid_hex,
            option="--public-head-cid-hex",
            errors=errors,
        )
    if args.kind in POLICY_DIGEST_KINDS:
        validate_hex64(
            args.policy_digest_hex,
            option="--policy-digest-hex",
            errors=errors,
        )
    if args.kind in INGRESS_QUALIFICATION_DIGEST_KINDS:
        for value, option in (
            (args.receiver_policy_digest_hex, "--receiver-policy-digest-hex"),
            (args.replay_namespace_digest_hex, "--replay-namespace-digest-hex"),
            (args.replica_set_digest_hex, "--replica-set-digest-hex"),
        ):
            validate_hex64(value, option=option, errors=errors)
    if args.kind in INGRESS_BINDING_DIGEST_KINDS:
        for value, option in (
            (args.kubo_ingress_binding_digest_hex, "--kubo-ingress-binding-digest-hex"),
            (
                args.signed_head_ingress_binding_digest_hex,
                "--signed-head-ingress-binding-digest-hex",
            ),
        ):
            validate_hex64(value, option=option, errors=errors)
    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=TRUE_CLAIMS[args.kind],
        option="--verified-claim",
        errors=errors,
    )
    if args.kind == "ingest_service":
        source_count = required_positive(
            args.source_count,
            option="--source-count",
            errors=errors,
        )
        args.payload_kinds = validate_name_set(
            split_csv_values(args.payload_kind),
            allowed=REQUIRED_PAYLOAD_KINDS,
            option="--payload-kind",
            errors=errors,
        )
        if source_count and source_count != len(args.payload_kinds):
            errors.append("--source-count must match unique --payload-kind count")
    elif args.kind == "publisher_service":
        required_positive(args.pin_lag_seconds, option="--pin-lag-seconds", errors=errors)
        required_positive(args.head_age_seconds, option="--head-age-seconds", errors=errors)
        block_count = required_positive(args.block_count, option="--block-count", errors=errors)
        args.block_refs = validate_reviewed_inventory(
            split_csv_values(args.block_ref),
            expected_count=block_count,
            option="--block-ref",
            kind="publisher_service",
            count_option="--block-count",
            pattern=BLOCK_REF_LABEL_PATTERN,
            label_error=BLOCK_REF_LABEL_ERROR,
            errors=errors,
        )
        args.payload_kinds = validate_name_set(
            split_csv_values(args.payload_kind),
            allowed=REQUIRED_PAYLOAD_KINDS,
            option="--payload-kind",
            errors=errors,
        )
    elif args.kind == "operator_recovery":
        validate_hex64(
            args.checkpoint_digest_hex,
            option="--checkpoint-digest-hex",
            errors=errors,
        )
    elif args.kind == "dashboard_api":
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )
        args.routes = validate_name_set(
            split_csv_values(args.route),
            allowed=REQUIRED_DASHBOARD_ROUTES,
            option="--route",
            errors=errors,
        )
    elif args.kind == "observability":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    elif args.kind == "publication_e2e":
        block_count = required_positive(args.block_count, option="--block-count", errors=errors)
        args.block_refs = validate_reviewed_inventory(
            split_csv_values(args.block_ref),
            expected_count=block_count,
            option="--block-ref",
            kind="publication_e2e",
            count_option="--block-count",
            pattern=BLOCK_REF_LABEL_PATTERN,
            label_error=BLOCK_REF_LABEL_ERROR,
            errors=errors,
        )
        args.payload_kinds = validate_name_set(
            split_csv_values(args.payload_kind),
            allowed=REQUIRED_PAYLOAD_KINDS,
            option="--payload-kind",
            errors=errors,
        )


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    require_rollout_deployment_id(
        {"--deployment-id": args.deployment_id},
        errors,
        field="--deployment-id",
    )
    require_rollout_environment(
        {"--environment": args.environment},
        errors,
        field="--environment",
    )
    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if args.pin_lag_seconds is not None and args.pin_lag_seconds > DEFAULT_MAX_PIN_LAG_SECS:
        errors.append(f"--pin-lag-seconds must be <= {DEFAULT_MAX_PIN_LAG_SECS}")
    if args.head_age_seconds is not None and args.head_age_seconds > DEFAULT_MAX_HEAD_AGE_SECS:
        errors.append(f"--head-age-seconds must be <= {DEFAULT_MAX_HEAD_AGE_SECS}")
    if args.block_count is not None and args.block_count < DEFAULT_MIN_BLOCKS:
        errors.append(f"--block-count must be >= {DEFAULT_MIN_BLOCKS}")
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_pin_lag_secs=DEFAULT_MAX_PIN_LAG_SECS,
        max_head_age_secs=DEFAULT_MAX_HEAD_AGE_SECS,
        min_blocks=DEFAULT_MIN_BLOCKS,
        min_payload_kinds=DEFAULT_MIN_PAYLOAD_KINDS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the Governance DAG gate contract."""

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
        parent_label = path_diagnostic_label(parent)
        return [
            f"--out parent `{parent_label}` cannot be created: "
            f"{error_diagnostic_label(error, path_label=parent_label)}"
        ]
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
        path_label = path_diagnostic_label(path)
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
        return [
            f"--out `{path_label}` cannot be written: "
            f"{error_diagnostic_label(error, path_label=path_label)}"
        ]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SF-12 Governance DAG canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
    parser.add_argument("--public-head-cid-hex")
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--source-count", type=positive_int_arg)
    parser.add_argument("--payload-kind", action="append", default=[])
    parser.add_argument("--pin-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--head-age-seconds", type=non_negative_int_arg)
    parser.add_argument("--block-count", type=positive_int_arg)
    parser.add_argument("--block-ref", action="append", default=[])
    parser.add_argument("--checkpoint-digest-hex")
    parser.add_argument("--route", action="append", default=[])
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=200)
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-body-blake3-hex")
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--receiver-policy-digest-hex")
    parser.add_argument("--replay-namespace-digest-hex")
    parser.add_argument("--replica-set-digest-hex")
    parser.add_argument("--kubo-ingress-binding-digest-hex")
    parser.add_argument("--signed-head-ingress-binding-digest-hex")
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
            "ERROR: SoraFS Governance DAG canary inputs are incomplete:",
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
