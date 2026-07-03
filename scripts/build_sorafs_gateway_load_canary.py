#!/usr/bin/env python3
"""Build payload-free SoraFS gateway load rollout canary artifacts."""

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

from check_sorafs_gateway_load_rollout_evidence import (  # noqa: E402
    ALLOWED_GATEWAY_CONFORMANCE_CARGO_COMMANDS,
    DEFAULT_MAX_ERROR_RATE_BPS,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_P95_LATENCY_MS,
    DEFAULT_MAX_P99_LATENCY_MS,
    DEFAULT_GATEWAY_CONFORMANCE_CARGO_COMMAND,
    DEFAULT_MIN_STAGING_DURATION_SECS,
    DEFAULT_MIN_STREAMS,
    DEFAULT_MIN_SUCCESS_RATE_BPS,
    CACHE_STATE_ERROR,
    FORBIDDEN_STAGING_METADATA_MARKERS,
    GATEWAY_VERSION_ERROR,
    GATEWAY_VERSION_PATTERN,
    HARDWARE_PROFILE_ERROR,
    KIND_BY_NAME,
    PROVIDER_NAME_ERROR,
    PROVIDER_NAME_PATTERN,
    REQUIRED_METRICS,
    REQUIRED_SCENARIOS,
    REQUIRED_CACHE_STATES,
    STAGING_METADATA_LABEL_PATTERN,
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
SUITE_DIGEST_KINDS = ("local_conformance", "staging_load", "governance_approval")
STAGING_DIGEST_KINDS = (
    "staging_load",
    "telemetry_slo",
    "transport_scope",
    "governance_approval",
)
POLICY_DIGEST_KINDS = ("staging_load", "governance_approval")
HEX64_LEN = 64


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


def validate_gateway_version_arg(value: str | None, *, errors: list[str]) -> None:
    """Require a concrete gateway release or release-candidate version label."""

    validate_canonical_string(value, label="--gateway-version", errors=errors)
    if value and GATEWAY_VERSION_PATTERN.fullmatch(value) is None:
        errors.append(GATEWAY_VERSION_ERROR.replace("gateway_version", "--gateway-version"))


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


def validate_staging_metadata_label(
    value: str | None,
    *,
    option: str,
    pattern: Any,
    pattern_error: str,
    errors: list[str],
) -> None:
    """Validate a reviewed staging metadata label before writing a canary."""

    validate_canonical_string(value, label=option, errors=errors)
    if not value:
        return
    if pattern.fullmatch(value) is None:
        errors.append(
            pattern_error.replace("hardware_profile.name", option).replace(
                "providers[].name",
                option,
            )
        )
        return
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_STAGING_METADATA_MARKERS
        if marker in value.split("-")
    )
    if forbidden:
        errors.append(f"{option} must not contain non-production markers {forbidden}")


def validate_cache_state_arg(value: str | None, *, errors: list[str]) -> None:
    """Validate a reviewed cache-state mode before writing a canary."""

    validate_canonical_string(value, label="--cache-state", errors=errors)
    if value and value not in REQUIRED_CACHE_STATES:
        errors.append(CACHE_STATE_ERROR.replace("cache_state.mode", "--cache-state"))


def require_kind_options(
    args: argparse.Namespace,
    errors: list[str],
    required: Sequence[tuple[str, Any]],
) -> None:
    """Require kind-specific options by stable CLI flag."""

    for option, value in required:
        if value is None:
            errors.append(f"{option} is required for {args.kind}")


def generated_stream_inventory(stream_count: int) -> list[dict[str, str]]:
    """Build stable per-stream labels for payload-free staging-load evidence."""

    return [{"name": f"stream-{index:04d}"} for index in range(stream_count)]


def validate_provider_names(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate reviewed staging provider names and bind the count cross-check."""

    provider_names = split_csv_values(args.provider)
    if not provider_names:
        errors.append("--provider is required for staging_load")
    for name in provider_names:
        validate_staging_metadata_label(
            name,
            option="--provider",
            pattern=PROVIDER_NAME_PATTERN,
            pattern_error=PROVIDER_NAME_ERROR,
            errors=errors,
        )
    if len(set(provider_names)) != len(provider_names):
        errors.append("--provider must not contain duplicates")
    if args.provider_count != len(set(provider_names)):
        errors.append("--provider-count must match the number of unique --provider values")
    args.providers = provider_names


def common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by gateway load canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "generated_at_unix": args.generated_at_unix,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free gateway load rollout canary payload."""

    payload = common_payload(args)
    if args.kind == "local_conformance":
        payload.update(
            {
                "ci_script": "ci/check_sorafs_gateway_conformance.sh",
                "cargo_command": args.cargo_command,
                "deterministic_harness_passed": True,
                "attestation_verified": True,
                "suite_report_digest_hex": args.suite_report_digest_hex,
                "scenario_count": len(args.scenarios),
                "load_profile_streams": args.stream_count,
                "load_profile_window_seconds": args.load_profile_window_seconds,
                "scenarios": args.scenarios,
                "raw_report_included": False,
                "private_keys_included": False,
            }
        )
    elif args.kind == "staging_load":
        payload.update(
            {
                "suite_report_digest_hex": args.suite_report_digest_hex,
                "staging_report_digest_hex": args.staging_report_digest_hex,
                "fixture_bundle_digest_hex": args.fixture_bundle_digest_hex,
                "policy_digest_hex": args.policy_digest_hex,
                "gateway_version": args.gateway_version,
                "hardware_profile": {"name": args.hardware_profile},
                "cache_state": {"mode": args.cache_state},
                "duration_seconds": args.duration_seconds,
                "stream_count": args.stream_count,
                "streams": generated_stream_inventory(args.stream_count),
                "provider_count": args.provider_count,
                "providers": [{"name": name} for name in args.providers],
                "success_rate_bps": args.success_rate_bps,
                "error_rate_bps": args.error_rate_bps,
                "p95_latency_ms": args.p95_latency_ms,
                "p99_latency_ms": args.p99_latency_ms,
                "response_bodies_included": False,
                "raw_payloads_included": False,
            }
        )
    elif args.kind == "telemetry_slo":
        payload.update(
            {
                "staging_report_digest_hex": args.staging_report_digest_hex,
                "metrics_scrape_success": True,
                "dashboard_archived": True,
                "slo_baseline_recorded": True,
                "cold_cache_baseline_recorded": True,
                "critical_alerts_firing": False,
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
                "response_bodies_included": False,
            }
        )
    elif args.kind == "transport_scope":
        payload.update(
            {
                "staging_report_digest_hex": args.staging_report_digest_hex,
                "http3_endpoint_committed": args.http3_endpoint_committed,
                "http3_scenarios_deferred": not args.http3_endpoint_committed,
                "http3_config_surface_documented": args.http3_endpoint_committed,
                "http3_scenarios_passed": args.http3_endpoint_committed,
                "transport_scope_reviewed": True,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "approved": True,
                "governance_vote_recorded": True,
                "iroha_config_bound": True,
                "gateway_release_bound": True,
                "local_conformance_bound": True,
                "staging_load_bound": True,
                "telemetry_bound": True,
                "transport_scope_bound": True,
                "suite_report_digest_hex": args.suite_report_digest_hex,
                "staging_report_digest_hex": args.staging_report_digest_hex,
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    return payload


def validate_thresholds(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate threshold-bound facts before payload construction."""

    if args.duration_seconds < DEFAULT_MIN_STAGING_DURATION_SECS:
        errors.append(
            f"--duration-seconds must be >= {DEFAULT_MIN_STAGING_DURATION_SECS}"
        )
    if args.stream_count < DEFAULT_MIN_STREAMS:
        errors.append(f"--stream-count must be >= {DEFAULT_MIN_STREAMS}")
    if args.success_rate_bps < DEFAULT_MIN_SUCCESS_RATE_BPS:
        errors.append(f"--success-rate-bps must be >= {DEFAULT_MIN_SUCCESS_RATE_BPS}")
    if args.error_rate_bps > DEFAULT_MAX_ERROR_RATE_BPS:
        errors.append(f"--error-rate-bps must be <= {DEFAULT_MAX_ERROR_RATE_BPS}")
    if args.p95_latency_ms > DEFAULT_MAX_P95_LATENCY_MS:
        errors.append(f"--p95-latency-ms must be <= {DEFAULT_MAX_P95_LATENCY_MS}")
    if args.p99_latency_ms > DEFAULT_MAX_P99_LATENCY_MS:
        errors.append(f"--p99-latency-ms must be <= {DEFAULT_MAX_P99_LATENCY_MS}")


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    if args.kind in SUITE_DIGEST_KINDS:
        validate_hex64(
            args.suite_report_digest_hex,
            option="--suite-report-digest-hex",
            errors=errors,
        )
    if args.kind in STAGING_DIGEST_KINDS:
        validate_hex64(
            args.staging_report_digest_hex,
            option="--staging-report-digest-hex",
            errors=errors,
        )
    if args.kind in POLICY_DIGEST_KINDS:
        validate_hex64(
            args.policy_digest_hex,
            option="--policy-digest-hex",
            errors=errors,
        )
    if args.kind == "local_conformance":
        args.scenarios = validate_name_set(
            split_csv_values(args.scenario),
            allowed=REQUIRED_SCENARIOS,
            option="--scenario",
            errors=errors,
        )
        validate_canonical_string(
            args.cargo_command,
            label="--cargo-command",
            errors=errors,
        )
        if args.cargo_command not in ALLOWED_GATEWAY_CONFORMANCE_CARGO_COMMANDS:
            errors.append("--cargo-command must be a reviewed gateway conformance command")
        if args.load_profile_window_seconds <= 0:
            errors.append("--load-profile-window-seconds must be positive")
        if args.stream_count < DEFAULT_MIN_STREAMS:
            errors.append(f"--stream-count must be >= {DEFAULT_MIN_STREAMS}")
    elif args.kind == "staging_load":
        require_kind_options(
            args,
            errors,
            (
                ("--fixture-bundle-digest-hex", args.fixture_bundle_digest_hex),
                ("--gateway-version", args.gateway_version),
                ("--hardware-profile", args.hardware_profile),
                ("--cache-state", args.cache_state),
            ),
        )
        validate_hex64(
            args.fixture_bundle_digest_hex,
            option="--fixture-bundle-digest-hex",
            errors=errors,
        )
        validate_gateway_version_arg(args.gateway_version, errors=errors)
        validate_staging_metadata_label(
            args.hardware_profile,
            option="--hardware-profile",
            pattern=STAGING_METADATA_LABEL_PATTERN,
            pattern_error=HARDWARE_PROFILE_ERROR,
            errors=errors,
        )
        validate_cache_state_arg(args.cache_state, errors=errors)
        validate_provider_names(args, errors)
        validate_thresholds(args, errors)
    elif args.kind == "telemetry_slo":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        min_staging_duration_secs=DEFAULT_MIN_STAGING_DURATION_SECS,
        min_streams=DEFAULT_MIN_STREAMS,
        min_success_rate_bps=DEFAULT_MIN_SUCCESS_RATE_BPS,
        max_error_rate_bps=DEFAULT_MAX_ERROR_RATE_BPS,
        max_p95_latency_ms=DEFAULT_MAX_P95_LATENCY_MS,
        max_p99_latency_ms=DEFAULT_MAX_P99_LATENCY_MS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the gateway load gate contract."""

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
        description="Build payload-free SoraFS SF-5a gateway load canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--suite-report-digest-hex")
    parser.add_argument("--staging-report-digest-hex")
    parser.add_argument("--fixture-bundle-digest-hex")
    parser.add_argument("--policy-digest-hex")
    parser.add_argument(
        "--cargo-command",
        default=DEFAULT_GATEWAY_CONFORMANCE_CARGO_COMMAND,
    )
    parser.add_argument("--scenario", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--load-profile-window-seconds", type=positive_int_arg, default=60)
    parser.add_argument("--duration-seconds", type=positive_int_arg, default=3_600)
    parser.add_argument("--stream-count", type=positive_int_arg, default=1_200)
    parser.add_argument("--provider-count", type=positive_int_arg, default=4)
    parser.add_argument("--provider", action="append", default=[])
    parser.add_argument("--success-rate-bps", type=positive_int_arg, default=9_950)
    parser.add_argument("--error-rate-bps", type=non_negative_int_arg, default=50)
    parser.add_argument("--p95-latency-ms", type=positive_int_arg, default=1_200)
    parser.add_argument("--p99-latency-ms", type=positive_int_arg, default=2_200)
    parser.add_argument("--gateway-version")
    parser.add_argument("--hardware-profile")
    parser.add_argument("--cache-state")
    parser.add_argument("--http3-endpoint-committed", action="store_true")
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
            "ERROR: SoraFS gateway load canary inputs are incomplete:",
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
