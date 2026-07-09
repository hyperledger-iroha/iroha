#!/usr/bin/env python3
"""Build payload-free SoraFS PDP rollout canary artifacts."""

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

from check_sorafs_pdp_rollout_evidence import (  # noqa: E402
    CHALLENGE_LABEL_ERROR,
    CHALLENGE_LABEL_PATTERN,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_PROOF_LATENCY_MS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_CHALLENGES,
    DEFAULT_MIN_PROOFS,
    DEFAULT_MIN_PROVIDERS,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    FORBIDDEN_PROVIDER_LABEL_MARKERS,
    KIND_BY_NAME,
    PROVIDER_LABEL_ERROR,
    PROVIDER_LABEL_PATTERN,
    PROOF_LABEL_ERROR,
    PROOF_LABEL_PATTERN,
    PROOF_SUMMARY_BOUND_KINDS,
    REQUIRED_METRICS,
    REQUIRED_ROUTES,
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
PROOF_SUMMARY_DIGEST_KINDS = ("proof_generation",) + PROOF_SUMMARY_BOUND_KINDS
POLICY_DIGEST_KINDS = ("proof_generation", "governance_approval")
PROVIDER_ROSTER_DIGEST_KINDS = ("proof_generation", "governance_approval")
HEX64_LEN = 64


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


def render_inventory_label_error(label_error: str, *, option: str) -> str:
    """Render checker inventory diagnostics as CLI option diagnostics."""

    return (
        label_error.replace("challenges[].name", option)
        .replace("proofs[].name", option)
    )


def validate_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    count_option: str,
    errors: list[str],
    pattern: re.Pattern[str] | None = None,
    label_error: str | None = None,
) -> list[str]:
    """Return reviewed unique inventory labels whose count matches a CLI count."""

    items = list(values)
    if not items:
        errors.append(f"{option} is required for proof_generation")
    for index, item in enumerate(items):
        validate_canonical_string(item, label=f"{option}[{index}]", errors=errors)
        if option == "--provider":
            validate_provider_label_arg(item, option=option, errors=errors)
        if pattern is None or not isinstance(item, str):
            continue
        if pattern.fullmatch(item) is None:
            errors.append(
                render_inventory_label_error(
                    label_error or f"{option} must use the expected label family",
                    option=option,
                )
            )
            continue
        forbidden = forbidden_non_production_markers(item, FORBIDDEN_INVENTORY_LABEL_MARKERS)
        if forbidden:
            errors.append(
                f"{option} must not contain non-production markers {forbidden}"
            )
    unique_items = set(items)
    if len(unique_items) != len(items):
        errors.append(f"{option} must not contain duplicates")
    if len(unique_items) != expected_count:
        errors.append(f"{option} unique values must match {count_option}")
    return items


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
    """Require a non-empty canonical string without control/format text."""

    if not diagnostic_text_is_canonical(value):
        errors.append(f"{label} must be a non-empty canonical string")


def validate_provider_label_arg(
    value: str | None,
    *,
    option: str,
    errors: list[str],
) -> None:
    """Require a reviewed lowercase production provider inventory label."""

    if not isinstance(value, str):
        return
    if PROVIDER_LABEL_PATTERN.fullmatch(value) is None:
        errors.append(PROVIDER_LABEL_ERROR.replace("providers[].name", option))
        return
    forbidden = forbidden_non_production_markers(value, FORBIDDEN_PROVIDER_LABEL_MARKERS)
    if forbidden:
        errors.append(f"{option} must not contain non-production markers {forbidden}")


def require_kind_options(
    args: argparse.Namespace,
    errors: list[str],
    required: Sequence[tuple[str, Any]],
) -> None:
    """Require kind-specific options by stable CLI flag."""

    for option, value in required:
        if value is None:
            errors.append(f"{option} is required for {args.kind}")


def common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by PDP canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "generated_at_unix": args.generated_at_unix,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def build_route_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build PDP provider transport route probe records."""

    return [
        {
            "name": name,
            "passed": True,
            "status_code": args.route_status_code,
            "body_blake3_hex": args.route_body_blake3_hex,
            "latency_ms": args.route_latency_ms,
            "authz_enforced": True,
            "norito_verified": True,
        }
        for name in args.routes
    ]


def build_inventory_records(names: Sequence[str]) -> list[dict[str, str]]:
    """Build reviewed payload-free inventory records."""

    return [{"name": name} for name in names]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free PDP rollout canary payload."""

    payload = common_payload(args)
    if args.kind == "provider_transport":
        routes = build_route_records(args)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "provider_protocol_enabled": True,
                "torii_pdp_fail_closed_guard_removed": True,
                "challenge_fetch_verified": True,
                "proof_submit_verified": True,
                "deadline_headers_verified": True,
                "provider_authz_enforced": True,
                "proof_stream_pdp_enabled": True,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "proof_generation":
        payload.update(
            {
                "provider_count": args.provider_count,
                "providers": build_inventory_records(args.providers),
                "challenge_count": args.challenge_count,
                "challenges": build_inventory_records(args.challenges),
                "proof_count": args.proof_count,
                "proofs": build_inventory_records(args.proofs),
                "provider_signatures_verified": True,
                "manifest_binding_verified": True,
                "commitment_binding_verified": True,
                "segment_merkle_paths_verified": True,
                "hot_leaf_merkle_paths_verified": True,
                "deadline_policy_verified": True,
                "hardware_determinism_reviewed": True,
                "max_proof_latency_ms": args.proof_latency_ms,
                "proof_summary_digest_hex": args.proof_summary_digest_hex,
                "policy_digest_hex": args.policy_digest_hex,
                "provider_roster_digest_hex": args.provider_roster_digest_hex,
                "raw_challenge_bytes_included": False,
                "raw_proof_bytes_included": False,
            }
        )
    elif args.kind == "validator_replay":
        payload.update(
            {
                "sorafs_validate_pdp_passed": True,
                "commitment_challenge_binding_verified": True,
                "challenge_proof_binding_verified": True,
                "segment_coverage_verified": True,
                "hot_leaf_coverage_verified": True,
                "deadline_policy_verified": True,
                "missing_merkle_path_negative_verified": True,
                "expanded_negative_fixtures_committed": True,
                "validation_outcome_schema_verified": True,
                "pairs_replayed": args.pairs_replayed,
                "proof_summary_digest_hex": args.proof_summary_digest_hex,
                "validation_bundle_digest_hex": args.validation_bundle_digest_hex,
                "raw_challenge_bytes_included": False,
                "raw_proof_bytes_included": False,
            }
        )
    elif args.kind == "governance_repair":
        payload.update(
            {
                "governance_dag_challenge_published": True,
                "governance_dag_verdict_published": True,
                "repair_handoff_verified": True,
                "repair_handoff_digest_hex": args.repair_handoff_digest_hex,
                "archive_retention_bound": True,
                "slash_policy_bound": True,
                "operator_export_verified": True,
                "proof_summary_digest_hex": args.proof_summary_digest_hex,
                "archive_summary_digest_hex": args.archive_summary_digest_hex,
                "raw_export_included": False,
                "raw_report_included": False,
            }
        )
    elif args.kind == "observability":
        payload.update(
            {
                "metrics_scrape_success": True,
                "dashboard_provisioned": True,
                "alert_rules_installed": True,
                "deadline_breach_alert_tested": True,
                "proof_failure_alert_tested": True,
                "repair_handoff_alert_tested": True,
                "critical_alerts_firing": False,
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
                "proof_summary_digest_hex": args.proof_summary_digest_hex,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "approved": True,
                "governance_vote_recorded": True,
                "iroha_config_bound": True,
                "pdp_policy_bound": True,
                "provider_roster_bound": True,
                "repair_policy_bound": True,
                "governance_dag_bound": True,
                "config_source": "iroha_config",
                "proof_summary_digest_hex": args.proof_summary_digest_hex,
                "policy_digest_hex": args.policy_digest_hex,
                "provider_roster_digest_hex": args.provider_roster_digest_hex,
            }
        )
    return payload


def validate_thresholds(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate threshold-bound facts before payload construction."""

    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if args.proof_latency_ms > DEFAULT_MAX_PROOF_LATENCY_MS:
        errors.append(f"--proof-latency-ms must be <= {DEFAULT_MAX_PROOF_LATENCY_MS}")
    if args.provider_count < DEFAULT_MIN_PROVIDERS:
        errors.append(f"--provider-count must be >= {DEFAULT_MIN_PROVIDERS}")
    if args.challenge_count < DEFAULT_MIN_CHALLENGES:
        errors.append(f"--challenge-count must be >= {DEFAULT_MIN_CHALLENGES}")
    if args.proof_count < DEFAULT_MIN_PROOFS:
        errors.append(f"--proof-count must be >= {DEFAULT_MIN_PROOFS}")


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
    validate_thresholds(args, errors)
    if args.kind in PROOF_SUMMARY_DIGEST_KINDS:
        validate_hex64(
            args.proof_summary_digest_hex,
            option="--proof-summary-digest-hex",
            errors=errors,
        )
    if args.kind in PROVIDER_ROSTER_DIGEST_KINDS:
        require_kind_options(
            args,
            errors,
            (("--provider-roster-digest-hex", args.provider_roster_digest_hex),),
        )
        validate_hex64(
            args.provider_roster_digest_hex,
            option="--provider-roster-digest-hex",
            errors=errors,
        )
    if args.kind == "provider_transport":
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )
        args.routes = validate_name_set(
            split_csv_values(args.route),
            allowed=REQUIRED_ROUTES,
            option="--route",
            errors=errors,
        )
        if args.route_status_code < 200 or args.route_status_code > 299:
            errors.append("--route-status-code must be a 2xx HTTP status code")
    elif args.kind == "proof_generation":
        args.providers = validate_reviewed_inventory(
            split_csv_values(args.provider),
            expected_count=args.provider_count,
            option="--provider",
            count_option="--provider-count",
            errors=errors,
        )
        args.challenges = validate_reviewed_inventory(
            split_csv_values(args.challenge),
            expected_count=args.challenge_count,
            option="--challenge",
            count_option="--challenge-count",
            errors=errors,
            pattern=CHALLENGE_LABEL_PATTERN,
            label_error=CHALLENGE_LABEL_ERROR,
        )
        args.proofs = validate_reviewed_inventory(
            split_csv_values(args.proof),
            expected_count=args.proof_count,
            option="--proof",
            count_option="--proof-count",
            errors=errors,
            pattern=PROOF_LABEL_PATTERN,
            label_error=PROOF_LABEL_ERROR,
        )
    elif args.kind == "validator_replay":
        require_kind_options(
            args,
            errors,
            (("--validation-bundle-digest-hex", args.validation_bundle_digest_hex),),
        )
        validate_hex64(
            args.validation_bundle_digest_hex,
            option="--validation-bundle-digest-hex",
            errors=errors,
        )
    elif args.kind == "governance_repair":
        require_kind_options(
            args,
            errors,
            (
                ("--archive-summary-digest-hex", args.archive_summary_digest_hex),
                ("--repair-handoff-digest-hex", args.repair_handoff_digest_hex),
            ),
        )
        validate_hex64(
            args.archive_summary_digest_hex,
            option="--archive-summary-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.repair_handoff_digest_hex,
            option="--repair-handoff-digest-hex",
            errors=errors,
        )
    elif args.kind == "observability":
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
            (("--policy-digest-hex", args.policy_digest_hex),),
        )
        validate_hex64(
            args.policy_digest_hex,
            option="--policy-digest-hex",
            errors=errors,
        )
    if args.kind in POLICY_DIGEST_KINDS and args.kind != "governance_approval":
        require_kind_options(
            args,
            errors,
            (("--policy-digest-hex", args.policy_digest_hex),),
        )
        validate_hex64(
            args.policy_digest_hex,
            option="--policy-digest-hex",
            errors=errors,
        )
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_proof_latency_ms=DEFAULT_MAX_PROOF_LATENCY_MS,
        min_providers=DEFAULT_MIN_PROVIDERS,
        min_challenges=DEFAULT_MIN_CHALLENGES,
        min_proofs=DEFAULT_MIN_PROOFS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the PDP gate contract."""

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
        description="Build payload-free SoraFS SF-13 PDP canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
    parser.add_argument("--proof-summary-digest-hex")
    parser.add_argument("--validation-bundle-digest-hex")
    parser.add_argument("--archive-summary-digest-hex")
    parser.add_argument("--repair-handoff-digest-hex")
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--provider-roster-digest-hex")
    parser.add_argument("--provider", action="append", default=[])
    parser.add_argument("--challenge", action="append", default=[])
    parser.add_argument("--proof", action="append", default=[])
    parser.add_argument("--route", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=200)
    parser.add_argument("--route-body-blake3-hex")
    parser.add_argument("--provider-count", type=positive_int_arg, default=3)
    parser.add_argument("--challenge-count", type=positive_int_arg, default=3)
    parser.add_argument("--proof-count", type=positive_int_arg, default=3)
    parser.add_argument("--proof-latency-ms", type=positive_int_arg, default=1_000)
    parser.add_argument("--pairs-replayed", type=positive_int_arg, default=3)
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
            "ERROR: SoraFS PDP canary inputs are incomplete:",
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
