#!/usr/bin/env python3
"""Build payload-free SoraFS hedging/billing rollout canary artifacts."""

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

from check_sorafs_hedging_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_CYCLE_AGE_SECS,
    DEFAULT_MAX_DIVERGENCE_BPS,
    DEFAULT_MAX_FEED_LAG_SECS,
    DEFAULT_MIN_BILLING_CYCLES,
    KIND_BY_NAME,
    REQUIRED_METRICS,
    REQUIRED_PUBLICATION_ROUTES,
    REQUIRED_RECONCILIATION_SOURCES,
    ValidationOptions,
    validate_evidence_payload,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
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
CYCLE_BINDING_KINDS = (
    "billing_cycle",
    "statement_publication",
    "reconciliation",
    "metrics_alerts",
    "governance_approval",
)
POLICY_DIGEST_KINDS = ("billing_cycle", "governance_approval")
HEX64_LEN = 64
TRUE_CLAIMS: dict[str, tuple[str, ...]] = {
    "feed_collector": (
        "primary_feed_present",
        "secondary_feed_present",
    ),
    "reference_price": (
        "feed_quorum_met",
        "signed_payload_verified",
    ),
    "billing_cycle": (
        "staged_cycle",
        "reference_price_bound",
        "acknowledgement_required",
    ),
    "statement_publication": (),
    "reconciliation": (),
    "metrics_alerts": (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
    ),
    "native_bridge_release": (
        "artifact_hashes_verified",
        "sdk_wrappers_verified",
    ),
    "governance_approval": (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "manual_override_policy_present",
        "treasury_limits_present",
    ),
}
FORCED_FALSE_FIELDS: dict[str, tuple[str, ...]] = {
    "feed_collector": ("payload_bytes_included", "response_bodies_included"),
    "reference_price": ("degraded", "payload_bytes_included"),
    "billing_cycle": ("statement_bodies_included", "raw_financial_records_included"),
    "statement_publication": ("response_bodies_included",),
    "reconciliation": ("raw_financial_records_included",),
    "metrics_alerts": ("critical_alerts_firing", "response_bodies_included"),
    "native_bridge_release": ("debug_artifacts",),
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
) -> list[str]:
    """Return reviewed unique inventory labels whose count matches a CLI count."""

    items = list(values)
    if not items:
        errors.append(f"{option} is required for {kind}")
    for index, item in enumerate(items):
        validate_canonical_string(item, label=f"{option}[{index}]", errors=errors)
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
    """Require a non-empty canonical string without control characters."""

    if (
        not isinstance(value, str)
        or not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{label} must be a non-empty canonical string")


def parse_artifacts(values: Sequence[str], errors: list[str]) -> list[dict[str, str]]:
    """Parse repeated id:sha256 artifact descriptors."""

    artifacts: list[dict[str, str]] = []
    seen_artifact_ids: set[str] = set()
    for index, value in enumerate(values):
        if ":" not in value:
            errors.append("--artifact must use id:sha256")
            continue
        artifact_id, sha256 = value.split(":", 1)
        validate_canonical_string(artifact_id, label=f"--artifact[{index}].id", errors=errors)
        validate_hex64(sha256, option=f"--artifact[{index}].sha256", errors=errors)
        if artifact_id in seen_artifact_ids:
            errors.append("duplicate --artifact id")
            continue
        seen_artifact_ids.add(artifact_id)
        artifacts.append({"id": artifact_id, "sha256": sha256})
    if not artifacts:
        errors.append("--artifact must include at least one artifact")
    return artifacts


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
    """Build fields shared by hedging/billing canary payloads."""

    payload: dict[str, Any] = {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
    }
    if args.kind in CYCLE_BINDING_KINDS:
        payload["statement_bundle_digest_hex"] = args.statement_bundle_digest_hex
        payload["reconciliation_digest_hex"] = args.reconciliation_digest_hex
    return payload


def apply_verified_claims(payload: dict[str, Any], args: argparse.Namespace) -> None:
    """Populate explicitly verified true claims and forced payload-free false flags."""

    for claim in TRUE_CLAIMS[args.kind]:
        payload[claim] = claim in args.verified_claims
    for field in FORCED_FALSE_FIELDS[args.kind]:
        payload[field] = False


def build_route_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free statement publication route probe records."""

    return [
        {
            "name": route,
            "passed": True,
            "status_code": args.route_status_code,
            "publisher_identity_present": True,
            "signature_verified": True,
        }
        for route in args.routes
    ]


def build_inventory_records(names: Sequence[str]) -> list[dict[str, str]]:
    """Build reviewed payload-free inventory records."""

    return [{"name": name} for name in names]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free hedging/billing rollout canary payload."""

    payload = build_common_payload(args)
    apply_verified_claims(payload, args)
    if args.kind == "feed_collector":
        payload.update(
            {
                "feed_count": args.feed_count,
                "accepted_feed_count": args.feed_count,
                "feeds": build_inventory_records(args.feeds),
                "rejected_feed_count": 0,
                "stale_feed_count": 0,
                "feed_lag_seconds": args.feed_lag_seconds,
            }
        )
    elif args.kind == "reference_price":
        payload.update(
            {
                "decision_id_hex": args.decision_id_hex,
                "reference_price_micro_usd": args.reference_price_micro_usd,
                "feed_count": args.feed_count,
                "accepted_feed_count": args.feed_count,
                "feeds": build_inventory_records(args.feeds),
                "rejected_feed_count": 0,
                "stale_feed_count": 0,
                "divergence_bps": args.divergence_bps,
                "decision_lag_seconds": args.decision_lag_seconds,
            }
        )
    elif args.kind == "billing_cycle":
        payload.update(
            {
                "cycle_id": args.cycle_id,
                "cycle_index": args.cycle_index,
                "statement_count": len(args.statement_digests_hex),
                "signed_statement_count": len(args.statement_digests_hex),
                "statements": build_inventory_records(args.statements),
                "line_item_count": args.line_item_count,
                "line_items": build_inventory_records(args.line_items),
                "total_micro_xor": args.total_micro_xor,
                "total_usd_micro": args.total_usd_micro,
                "reference_decision_id_hex": args.reference_decision_id_hex,
                "policy_digest_hex": args.policy_digest_hex,
                "line_item_root_hex": args.line_item_root_hex,
                "statement_digests_hex": args.statement_digests_hex,
            }
        )
    elif args.kind == "statement_publication":
        routes = build_route_records(args)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "acknowledgement_probe_count": args.acknowledgement_probe_count,
                "routes": routes,
            }
        )
    elif args.kind == "reconciliation":
        payload.update(
            {
                "source_count": len(args.sources),
                "sources": [{"name": source} for source in args.sources],
                "line_item_count": args.line_item_count,
                "line_items": build_inventory_records(args.line_items),
                "reconciled_line_item_count": args.line_item_count,
                "mismatch_count": 0,
                "unmatched_event_count": 0,
            }
        )
    elif args.kind == "metrics_alerts":
        payload["metrics"] = args.metrics
    elif args.kind == "native_bridge_release":
        payload.update(
            {
                "bridge_abi_version": args.bridge_abi_version,
                "artifact_count": len(args.artifacts),
                "artifacts": args.artifacts,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
                "hedge_execution_enabled": args.hedge_execution_enabled,
            }
        )
        if args.hedge_execution_governed:
            payload["hedge_execution_governed"] = True
    return payload


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=TRUE_CLAIMS[args.kind],
        option="--verified-claim",
        errors=errors,
    )
    if args.kind in CYCLE_BINDING_KINDS:
        validate_hex64(
            args.statement_bundle_digest_hex,
            option="--statement-bundle-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.reconciliation_digest_hex,
            option="--reconciliation-digest-hex",
            errors=errors,
        )
    if args.kind in POLICY_DIGEST_KINDS:
        require_kind_options(
            args,
            errors,
            (("--policy-digest-hex", args.policy_digest_hex),),
        )
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)
    if args.kind == "feed_collector":
        require_kind_options(
            args,
            errors,
            (
                ("--feed-count", args.feed_count),
                ("--feed-lag-seconds", args.feed_lag_seconds),
            ),
        )
        args.feeds = validate_reviewed_inventory(
            split_csv_values(args.feed),
            expected_count=args.feed_count or 0,
            option="--feed",
            kind="feed_collector",
            count_option="--feed-count",
            errors=errors,
        )
    elif args.kind == "reference_price":
        require_kind_options(
            args,
            errors,
            (
                ("--decision-id-hex", args.decision_id_hex),
                ("--reference-price-micro-usd", args.reference_price_micro_usd),
                ("--feed-count", args.feed_count),
                ("--divergence-bps", args.divergence_bps),
                ("--decision-lag-seconds", args.decision_lag_seconds),
            ),
        )
        validate_hex64(args.decision_id_hex, option="--decision-id-hex", errors=errors)
        args.feeds = validate_reviewed_inventory(
            split_csv_values(args.feed),
            expected_count=args.feed_count or 0,
            option="--feed",
            kind="reference_price",
            count_option="--feed-count",
            errors=errors,
        )
    elif args.kind == "billing_cycle":
        require_kind_options(
            args,
            errors,
            (
                ("--cycle-id", args.cycle_id),
                ("--cycle-index", args.cycle_index),
                ("--line-item-count", args.line_item_count),
                ("--total-micro-xor", args.total_micro_xor),
                ("--total-usd-micro", args.total_usd_micro),
                ("--reference-decision-id-hex", args.reference_decision_id_hex),
                ("--line-item-root-hex", args.line_item_root_hex),
            ),
        )
        validate_canonical_string(args.cycle_id, label="--cycle-id", errors=errors)
        validate_hex64(
            args.reference_decision_id_hex,
            option="--reference-decision-id-hex",
            errors=errors,
        )
        validate_hex64(args.line_item_root_hex, option="--line-item-root-hex", errors=errors)
        args.statement_digests_hex = split_csv_values(args.statement_digest_hex)
        if not args.statement_digests_hex:
            errors.append("--statement-digest-hex must include at least one value")
        for index, digest in enumerate(args.statement_digests_hex):
            validate_hex64(digest, option=f"--statement-digest-hex[{index}]", errors=errors)
        args.statements = validate_reviewed_inventory(
            split_csv_values(args.statement),
            expected_count=len(args.statement_digests_hex),
            option="--statement",
            kind="billing_cycle",
            count_option="--statement-digest-hex",
            errors=errors,
        )
        args.line_items = validate_reviewed_inventory(
            split_csv_values(args.line_item),
            expected_count=args.line_item_count or 0,
            option="--line-item",
            kind="billing_cycle",
            count_option="--line-item-count",
            errors=errors,
        )
    elif args.kind == "statement_publication":
        require_kind_options(
            args,
            errors,
            (("--acknowledgement-probe-count", args.acknowledgement_probe_count),),
        )
        args.routes = validate_name_set(
            split_csv_values(args.route),
            allowed=REQUIRED_PUBLICATION_ROUTES,
            option="--route",
            errors=errors,
        )
    elif args.kind == "reconciliation":
        require_kind_options(
            args,
            errors,
            (("--line-item-count", args.line_item_count),),
        )
        args.sources = validate_name_set(
            split_csv_values(args.source),
            allowed=REQUIRED_RECONCILIATION_SOURCES,
            option="--source",
            errors=errors,
        )
        args.line_items = validate_reviewed_inventory(
            split_csv_values(args.line_item),
            expected_count=args.line_item_count or 0,
            option="--line-item",
            kind="reconciliation",
            count_option="--line-item-count",
            errors=errors,
        )
    elif args.kind == "metrics_alerts":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    elif args.kind == "native_bridge_release":
        require_kind_options(
            args,
            errors,
            (("--bridge-abi-version", args.bridge_abi_version),),
        )
        args.artifacts = parse_artifacts(args.artifact, errors)
    elif args.kind == "governance_approval":
        if args.hedge_execution_enabled and not args.hedge_execution_governed:
            errors.append("--hedge-execution-governed is required when hedge execution is enabled")


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    if args.feed_lag_seconds is not None and args.feed_lag_seconds > DEFAULT_MAX_FEED_LAG_SECS:
        errors.append(f"--feed-lag-seconds must be <= {DEFAULT_MAX_FEED_LAG_SECS}")
    if (
        args.decision_lag_seconds is not None
        and args.decision_lag_seconds > DEFAULT_MAX_FEED_LAG_SECS
    ):
        errors.append(f"--decision-lag-seconds must be <= {DEFAULT_MAX_FEED_LAG_SECS}")
    if args.divergence_bps is not None and args.divergence_bps > DEFAULT_MAX_DIVERGENCE_BPS:
        errors.append(f"--divergence-bps must be <= {DEFAULT_MAX_DIVERGENCE_BPS}")
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_feed_lag_secs=DEFAULT_MAX_FEED_LAG_SECS,
        max_cycle_age_secs=DEFAULT_MAX_CYCLE_AGE_SECS,
        max_divergence_bps=DEFAULT_MAX_DIVERGENCE_BPS,
        min_billing_cycles=DEFAULT_MIN_BILLING_CYCLES,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the hedging/billing gate contract."""

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
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            fd = -1
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp_path, path)
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
        description="Build payload-free SoraFS SFM-5 hedging/billing canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--feed-count", type=positive_int_arg)
    parser.add_argument("--feed", action="append", default=[])
    parser.add_argument("--feed-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--decision-id-hex")
    parser.add_argument("--reference-price-micro-usd", type=positive_int_arg)
    parser.add_argument("--divergence-bps", type=non_negative_int_arg)
    parser.add_argument("--decision-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--cycle-id")
    parser.add_argument("--cycle-index", type=positive_int_arg)
    parser.add_argument("--line-item-count", type=positive_int_arg)
    parser.add_argument("--line-item", action="append", default=[])
    parser.add_argument("--total-micro-xor", type=positive_int_arg)
    parser.add_argument("--total-usd-micro", type=positive_int_arg)
    parser.add_argument("--reference-decision-id-hex")
    parser.add_argument("--line-item-root-hex")
    parser.add_argument("--statement-bundle-digest-hex")
    parser.add_argument("--reconciliation-digest-hex")
    parser.add_argument("--statement-digest-hex", action="append", default=[])
    parser.add_argument("--statement", action="append", default=[])
    parser.add_argument("--acknowledgement-probe-count", type=positive_int_arg)
    parser.add_argument("--route", action="append", default=[])
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--source", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--bridge-abi-version", type=positive_int_arg)
    parser.add_argument("--artifact", action="append", default=[])
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--hedge-execution-enabled", action="store_true")
    parser.add_argument("--hedge-execution-governed", action="store_true")
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
            "ERROR: SoraFS hedging/billing canary inputs are incomplete:",
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
