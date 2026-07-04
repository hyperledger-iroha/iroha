#!/usr/bin/env python3
"""Build payload-free SoraFS gateway compliance rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import secrets
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_gateway_compliance_rollout_evidence import (  # noqa: E402
    CONTROLLER_INSTANCE_ID_ERROR,
    CONTROLLER_INSTANCE_ID_PATTERN,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_RELOAD_LATENCY_MS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_DENYLIST_ENTRIES,
    DEFAULT_MIN_GATEWAYS,
    DEFAULT_MIN_HONEY_PROBES,
    DENYLIST_ENTRY_LABEL_ERROR,
    DENYLIST_ENTRY_LABEL_PATTERN,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    FORBIDDEN_CONTROLLER_INSTANCE_ID_MARKERS,
    GATEWAY_LABEL_ERROR,
    GATEWAY_LABEL_PATTERN,
    HONEY_PROBE_LABEL_ERROR,
    HONEY_PROBE_LABEL_PATTERN,
    KIND_BY_NAME,
    REQUIRED_CONTROLLER_FEEDS,
    REQUIRED_DENIAL_REASONS,
    REQUIRED_ENFORCEMENT_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_MODERATION_TOGGLES,
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
    positive_int_arg,
)
from sorafs_runner_preflight import runner_url_arg_is_plan_safe  # noqa: E402


CANARY_KINDS = tuple(KIND_BY_NAME)
HEX64_LEN = 64
DEFAULT_GATEWAYS = (
    "gateway-compliance-gateway-a",
    "gateway-compliance-gateway-b",
    "gateway-compliance-gateway-c",
)
DEFAULT_DENYLIST_ENTRIES = (
    "gateway-denylist-entry-ofac",
    "gateway-denylist-entry-eu-sanctions",
    "gateway-denylist-entry-malware",
    "gateway-denylist-entry-csam-hash",
    "gateway-denylist-entry-legal-hold",
)
DEFAULT_ENFORCEMENT_ROUTES = REQUIRED_ENFORCEMENT_ROUTES
DEFAULT_HONEY_PROBES = tuple(
    f"gateway-honey-probe-{index:02d}" for index in range(DEFAULT_MIN_HONEY_PROBES)
)
CONTROLLER_TRUE_CLAIMS = (
    "iroha_config_bound",
    "controller_service_enabled",
    "scheduler_config_bound",
    "external_feeds_fetched",
    "feed_signature_verified",
    "normalization_deterministic",
    "bundle_pack_verified",
    "update_history_persisted",
    "gateway_reload_requested",
    "failure_backoff_configured",
    "rollback_plan_verified",
)
MODERATION_TRUE_CLAIMS = (
    "iroha_config_bound",
    "operator_role_enforced",
    "approval_workflow_verified",
    "expiry_enforced",
    "cache_invalidation_verified",
    "operator_audit_trail_persisted",
    "rollback_verified",
)
FORBIDDEN_PAYLOAD_CLAIMS = {
    "feed_promotion": (
        "raw_feeds_included",
        "feed_payloads_included",
    ),
    "controller_runtime": (
        "raw_feeds_included",
        "feed_payloads_included",
        "response_bodies_included",
    ),
    "moderation_toggle": (
        "raw_toggle_payloads_included",
        "response_bodies_included",
    ),
    "gateway_reload": ("raw_catalog_included",),
    "enforcement_probe": ("response_bodies_included",),
    "honey_audit": ("raw_probe_responses_included",),
    "appeal_override": ("raw_appeal_payload_included",),
    "transparency_publication": ("raw_receipts_included",),
    "observability": ("response_bodies_included",),
    "governance_approval": (),
}
CANARY_URL_ARG_ERROR = (
    "SoraFS gateway compliance canary URL arguments must not contain userinfo, "
    "query strings, fragments, control characters, encoded traversal, separators, "
    "drive prefixes, URI-scheme-like host/path tokens, or secret-looking host/path "
    "components"
)


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


def validate_canonical_string(value: str | None, *, option: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control characters."""

    if (
        not isinstance(value, str)
        or not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{option} must be a non-empty canonical string")


def validate_canary_url(value: str | None, *, option: str, errors: list[str]) -> None:
    """Validate a URL argument before it can enter canary evidence."""

    before = len(errors)
    validate_canonical_string(value, option=option, errors=errors)
    if len(errors) != before:
        return
    if not runner_url_arg_is_plan_safe(value):
        if CANARY_URL_ARG_ERROR not in errors:
            errors.append(CANARY_URL_ARG_ERROR)


def validate_controller_instance_id_arg(
    value: str | None, *, errors: list[str]
) -> None:
    """Require a reviewed lowercase compliance controller instance identifier."""

    validate_canonical_string(value, option="--controller-instance-id", errors=errors)
    if not isinstance(value, str):
        return
    if CONTROLLER_INSTANCE_ID_PATTERN.fullmatch(value) is None:
        errors.append(
            CONTROLLER_INSTANCE_ID_ERROR.replace(
                "controller_instance_id", "--controller-instance-id"
            )
        )
        return
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_CONTROLLER_INSTANCE_ID_MARKERS
        if marker in value.split("-")
    )
    if forbidden:
        errors.append(
            "--controller-instance-id must not contain non-production markers "
            f"{forbidden}"
        )


def render_inventory_label_error(label_error: str, option: str) -> str:
    """Render checker inventory-label diagnostics against builder constants."""

    return (
        label_error.replace("gateways[].name", option)
        .replace("denylist_entries[].name", option)
        .replace("probes[].name", option)
    )


def validate_static_inventory_labels(
    values: Iterable[str],
    *,
    option: str,
    pattern,
    label_error: str,
    errors: list[str],
) -> None:
    """Validate fixed builder inventory labels before generating evidence."""

    for value in values:
        validate_canonical_string(value, option=option, errors=errors)
        if not isinstance(value, str):
            continue
        if pattern.fullmatch(value) is None:
            errors.append(render_inventory_label_error(label_error, option))
            continue
        forbidden = sorted(
            marker
            for marker in FORBIDDEN_INVENTORY_LABEL_MARKERS
            if marker in value.split("-")
        )
        if forbidden:
            errors.append(f"{option} must not contain non-production markers {forbidden}")


def validate_default_inventories(errors: list[str]) -> None:
    """Validate fixed inventories that are not operator-provided CLI args."""

    validate_static_inventory_labels(
        DEFAULT_GATEWAYS,
        option="DEFAULT_GATEWAYS",
        pattern=GATEWAY_LABEL_PATTERN,
        label_error=GATEWAY_LABEL_ERROR,
        errors=errors,
    )
    validate_static_inventory_labels(
        DEFAULT_DENYLIST_ENTRIES,
        option="DEFAULT_DENYLIST_ENTRIES",
        pattern=DENYLIST_ENTRY_LABEL_PATTERN,
        label_error=DENYLIST_ENTRY_LABEL_ERROR,
        errors=errors,
    )
    validate_static_inventory_labels(
        DEFAULT_HONEY_PROBES,
        option="DEFAULT_HONEY_PROBES",
        pattern=HONEY_PROBE_LABEL_PATTERN,
        label_error=HONEY_PROBE_LABEL_ERROR,
        errors=errors,
    )


def validate_feed_names(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate reviewed feed names and bind the optional count cross-check."""

    feed_names = validate_name_set(
        split_csv_values(args.feed),
        allowed=REQUIRED_CONTROLLER_FEEDS,
        option="--feed",
        errors=errors,
    )
    unique_feed_count = len(feed_names)
    if args.feed_count is None:
        errors.append("--feed-count is required for controller_runtime")
    elif unique_feed_count != args.feed_count:
        errors.append(
            "--feed-count must match the number of required unique --feed values"
        )
    args.feeds = feed_names


def validate_toggle_names(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate reviewed moderation toggle names and bind the count cross-check."""

    toggle_names = validate_name_set(
        split_csv_values(args.toggle),
        allowed=REQUIRED_MODERATION_TOGGLES,
        option="--toggle",
        errors=errors,
    )
    unique_toggle_count = len(toggle_names)
    if args.toggle_count is None:
        errors.append("--toggle-count is required for moderation_toggle")
    elif unique_toggle_count != args.toggle_count:
        errors.append(
            "--toggle-count must match the number of required unique --toggle values"
        )
    args.toggles = toggle_names


def validate_denial_reasons(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate reviewed denial reason labels for enforcement probes."""

    args.denial_reasons = validate_name_set(
        split_csv_values(args.denial_reason),
        allowed=REQUIRED_DENIAL_REASONS,
        option="--denial-reason",
        errors=errors,
    )


def validate_metric_names(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate reviewed observability metrics for canary evidence."""

    args.metrics = validate_name_set(
        split_csv_values(args.metric),
        allowed=REQUIRED_METRICS,
        option="--metric",
        errors=errors,
    )


def named_records(names: Iterable[str]) -> list[dict[str, str]]:
    """Build stable `{name}` records for inventory-backed evidence."""

    return [{"name": name} for name in names]


def route_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free enforcement route probe records."""

    return [
        {
            "name": name,
            "passed": True,
            "status_code": 200,
            "body_blake3_hex": args.route_body_blake3_hex,
            "latency_ms": args.route_latency_ms,
            "authz_enforced": True,
        }
        for name in DEFAULT_ENFORCEMENT_ROUTES
    ]


def build_common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by gateway compliance canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
        "bundle_digest_hex": args.bundle_digest_hex,
    }


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free gateway compliance canary payload."""

    payload = build_common_payload(args)
    if args.kind == "feed_promotion":
        payload.update(
            {
                "external_feeds_normalized": True,
                "feed_signature_verified": True,
                "bundle_pack_verified": True,
                "bundle_diff_reviewed": True,
                "merkle_root_bound": True,
                "update_history_persisted": True,
                "gateway_ack_count": len(DEFAULT_GATEWAYS),
                "gateways": named_records(DEFAULT_GATEWAYS),
                "denylist_entry_count": len(DEFAULT_DENYLIST_ENTRIES),
                "denylist_entries": named_records(DEFAULT_DENYLIST_ENTRIES),
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    elif args.kind == "controller_runtime":
        payload.update(
            {
                "controller_instance_id": args.controller_instance_id,
                "iroha_config_bound": "iroha_config_bound" in args.verified_claims,
                "config_source": "iroha_config",
                "external_feed_count": len(args.feeds),
                "fetched_feed_count": len(args.feeds),
                "normalized_feed_count": len(args.feeds),
                "signed_feed_count": len(args.feeds),
                "feeds": named_records(args.feeds),
            }
        )
        for claim in CONTROLLER_TRUE_CLAIMS:
            payload[claim] = claim in args.verified_claims
    elif args.kind == "moderation_toggle":
        payload.update(
            {
                "toggle_api_url": args.toggle_api_url,
                "toggle_count": len(args.toggles),
                "approved_toggle_count": len(args.toggles),
                "toggles": named_records(args.toggles),
                "toggle_digest_hex": args.toggle_digest_hex,
                "iroha_config_bound": "iroha_config_bound" in args.verified_claims,
                "config_source": "iroha_config",
            }
        )
        for claim in MODERATION_TRUE_CLAIMS:
            payload[claim] = claim in args.verified_claims
    elif args.kind == "gateway_reload":
        payload.update(
            {
                "reload_ack_count": len(DEFAULT_GATEWAYS),
                "gateways": named_records(DEFAULT_GATEWAYS),
                "max_reload_latency_ms": args.reload_latency_ms,
                "hot_reload_verified": True,
                "cache_version_bound": True,
                "denylist_catalog_readback_verified": True,
                "persistence_path_configured": True,
                "stale_bundle_rejected": True,
                "rollback_plan_verified": True,
            }
        )
    elif args.kind == "enforcement_probe":
        routes = route_records(args)
        payload.update(
            {
                "denial_reasons_observed": list(args.denial_reasons),
                "denial_reason_count": len(args.denial_reasons),
                "structured_error_labels_verified": True,
                "telemetry_labels_stable": True,
                "fail_closed_missing_envelope": True,
                "fail_closed_unadmitted_provider": True,
                "rate_limit_verified": True,
                "geofence_verified": True,
                "proof_token_required": True,
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
            }
        )
    elif args.kind == "honey_audit":
        payload.update(
            {
                "honey_probe_count": len(DEFAULT_HONEY_PROBES),
                "probes": named_records(DEFAULT_HONEY_PROBES),
                "denied_response_verified": True,
                "cache_version_binding_verified": True,
                "proof_token_verified": True,
                "json_report_generated": True,
                "markdown_report_generated": True,
                "audit_digest_hex": args.audit_digest_hex,
            }
        )
    elif args.kind == "appeal_override":
        payload.update(
            {
                "appeal_outcome_consumed": True,
                "policy_override_signed": True,
                "cache_invalidation_verified": True,
                "override_expiry_enforced": True,
                "operator_audit_trail_persisted": True,
                "denylist_override_scoped": True,
                "override_digest_hex": args.override_digest_hex,
            }
        )
    elif args.kind == "transparency_publication":
        payload.update(
            {
                "gar_receipts_published": True,
                "proof_token_index_published": True,
                "moderation_events_published": True,
                "legal_hold_redaction_summaries_published": True,
                "governance_dag_bound": True,
                "transparency_cycle_verified": True,
                "publication_digest_hex": args.publication_digest_hex,
            }
        )
    elif args.kind == "observability":
        payload.update(
            {
                "metrics_scrape_success": True,
                "dashboard_provisioned": True,
                "alert_rules_installed": True,
                "critical_alerts_firing": False,
                "metrics": list(args.metrics),
                "metric_count": len(args.metrics),
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "approved": True,
                "governance_vote_recorded": True,
                "iroha_config_bound": True,
                "config_source": "iroha_config",
                "compliance_policy_bound": True,
                "denylist_feed_roster_bound": True,
                "transparency_policy_bound": True,
                "operator_roles_bound": True,
                "retention_policy_bound": True,
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    for claim in FORBIDDEN_PAYLOAD_CLAIMS[args.kind]:
        payload[claim] = False
    return payload


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    if args.kind == "controller_runtime":
        validate_controller_instance_id_arg(args.controller_instance_id, errors=errors)
        validate_feed_names(args, errors)
        args.verified_claims = validate_name_set(
            split_csv_values(args.verified_claim),
            allowed=CONTROLLER_TRUE_CLAIMS,
            option="--verified-claim",
            errors=errors,
        )
        return

    if args.kind == "moderation_toggle":
        validate_canary_url(
            args.toggle_api_url,
            option="--toggle-api-url",
            errors=errors,
        )
        validate_toggle_names(args, errors)
        validate_hex64(args.toggle_digest_hex, option="--toggle-digest-hex", errors=errors)
        args.verified_claims = validate_name_set(
            split_csv_values(args.verified_claim),
            allowed=MODERATION_TRUE_CLAIMS,
            option="--verified-claim",
            errors=errors,
        )
        return

    if args.kind in {"feed_promotion", "governance_approval"}:
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)
        return

    if args.kind == "enforcement_probe":
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )
        validate_denial_reasons(args, errors)
        return

    if args.kind == "honey_audit":
        validate_hex64(args.audit_digest_hex, option="--audit-digest-hex", errors=errors)
        return

    if args.kind == "appeal_override":
        validate_hex64(args.override_digest_hex, option="--override-digest-hex", errors=errors)
        return

    if args.kind == "transparency_publication":
        validate_hex64(
            args.publication_digest_hex,
            option="--publication-digest-hex",
            errors=errors,
        )
        return

    if args.kind == "observability":
        validate_metric_names(args, errors)


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, option="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, option="--environment", errors=errors)
    validate_hex64(args.bundle_digest_hex, option="--bundle-digest-hex", errors=errors)
    validate_default_inventories(errors)
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_reload_latency_ms=DEFAULT_MAX_RELOAD_LATENCY_MS,
        min_gateways=DEFAULT_MIN_GATEWAYS,
        min_denylist_entries=DEFAULT_MIN_DENYLIST_ENTRIES,
        min_honey_probes=DEFAULT_MIN_HONEY_PROBES,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the gateway compliance gate contract."""

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
        description=(
            "Build payload-free SoraFS SFM-4 gateway compliance canary JSON."
        ),
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--bundle-digest-hex", required=True)
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--controller-instance-id")
    parser.add_argument("--feed-count", type=positive_int_arg)
    parser.add_argument("--feed", action="append", default=[])
    parser.add_argument("--toggle-api-url")
    parser.add_argument("--toggle-count", type=positive_int_arg)
    parser.add_argument("--toggle", action="append", default=[])
    parser.add_argument("--toggle-digest-hex")
    parser.add_argument(
        "--reload-latency-ms",
        type=positive_int_arg,
        default=1_000,
    )
    parser.add_argument(
        "--route-latency-ms",
        type=positive_int_arg,
        default=120,
    )
    parser.add_argument("--route-body-blake3-hex")
    parser.add_argument("--denial-reason", action="append", default=[])
    parser.add_argument("--audit-digest-hex")
    parser.add_argument("--override-digest-hex")
    parser.add_argument("--publication-digest-hex")
    parser.add_argument("--metric", action="append", default=[])
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
            "ERROR: SoraFS gateway compliance canary inputs are incomplete:",
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
