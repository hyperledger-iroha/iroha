#!/usr/bin/env python3
"""NX-7 validator smoke tests for newly provisioned Nexus lanes."""

import argparse
import json
import math
import ssl
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple


class SmokeError(RuntimeError):
    """Raised when the smoke test detects a failure."""


@dataclass
class LaneCheck:
    alias: str
    manifest_required: bool
    manifest_ready: bool
    manifest_path: Optional[str]
    validators: int
    quorum: Optional[int]


def main(argv: Optional[Iterable[str]] = None) -> int:
    try:
        args = parse_args(argv)
        status, status_source = load_status_source(
            args.status_url,
            args.status_file,
            timeout=args.timeout,
            insecure=args.insecure,
        )
        lane_results = [verify_lane_ready(status, alias) for alias in args.lane_alias]

        telemetry_events = None
        metrics = None
        metrics_source = None
        metrics_payload = load_metrics_source(
            args.metrics_url,
            args.metrics_file,
            timeout=args.timeout,
            insecure=args.insecure,
        )
        if metrics_payload is not None:
            metrics_text, metrics_source = metrics_payload
            metrics = parse_prometheus_metrics(metrics_text)
            if args.expected_lane_count is not None:
                actual_count = get_metric_value(metrics, "nexus_lane_configured_total")
                if actual_count is None:
                    raise SmokeError(
                        "metrics data did not include `nexus_lane_configured_total` "
                        "(is telemetry enabled?)"
                    )
                if int(actual_count) != args.expected_lane_count:
                    raise SmokeError(
                        f"expected nexus_lane_configured_total={args.expected_lane_count}, "
                        f"found {actual_count}"
                    )
            verify_lane_governance_sealed(
                metrics,
                args.lane_alias,
                allow_missing=args.allow_missing_lane_metrics,
            )
        if args.telemetry_file:
            telemetry_events = load_telemetry_events(args.telemetry_file)
            verify_telemetry_aliases(args.lane_alias, telemetry_events)
            if args.require_alias_migration:
                verify_alias_migrations(args.require_alias_migration, telemetry_events)

        print(f"[ok] Torii status reachable: {status_source}")
        for lane in lane_results:
            if lane.manifest_path:
                detail = lane.manifest_path
            elif lane.manifest_required:
                detail = "manifest loaded"
            else:
                detail = "manifest optional"
            print(
                f"[ok] lane `{lane.alias}` ready "
                f"(quorum={lane.quorum or 'n/a'}, validators={lane.validators}, {detail})"
            )
        if metrics is not None and metrics_source is not None:
            print(f"[ok] Metrics reachable: {metrics_source}")
            if args.expected_lane_count is not None:
                print(f"[ok] nexus_lane_configured_total={args.expected_lane_count}")
            da_ratio = get_metric_value(metrics, "iroha_da_quorum_ratio")
            if da_ratio is None:
                raise SmokeError("metrics missing `iroha_da_quorum_ratio` gauge")
            if da_ratio < args.min_da_quorum:
                raise SmokeError(
                    f"iroha_da_quorum_ratio {da_ratio:.3f} below minimum {args.min_da_quorum}"
                )
            print(f"[ok] DA quorum ratio={da_ratio:.3f}")
            staleness = get_metric_value(metrics, "iroha_oracle_staleness_seconds")
            if staleness is None:
                raise SmokeError("metrics missing `iroha_oracle_staleness_seconds` gauge")
            if staleness > args.max_oracle_staleness:
                raise SmokeError(
                    f"oracle staleness {staleness:.1f}s exceeds {args.max_oracle_staleness:.1f}s"
                )
            print(f"[ok] oracle staleness={staleness:.1f}s")
            twap = get_metric_value(metrics, "iroha_oracle_twap_window_seconds")
            if twap is None:
                raise SmokeError("metrics missing `iroha_oracle_twap_window_seconds` gauge")
            if abs(twap - args.expected_oracle_twap) > args.oracle_twap_tolerance:
                raise SmokeError(
                    "oracle TWAP window deviates from "
                    f"{args.expected_oracle_twap:.1f}s by more than "
                    f"{args.oracle_twap_tolerance:.1f}s (observed {twap:.1f}s)"
                )
            print(f"[ok] oracle TWAP={twap:.1f}s")
            haircut = get_metric_value(metrics, "iroha_oracle_haircut_basis_points")
            if haircut is None:
                raise SmokeError("metrics missing `iroha_oracle_haircut_basis_points` gauge")
            if haircut > args.max_oracle_haircut_bps:
                raise SmokeError(
                    f"oracle haircut {haircut:.1f} bps exceeds {args.max_oracle_haircut_bps:.1f} bps"
                )
            print(f"[ok] oracle haircut={haircut:.1f} bps")
            settlement_buffer = get_metric_value(metrics, "iroha_settlement_buffer_xor")
            if settlement_buffer is None:
                raise SmokeError("metrics missing `iroha_settlement_buffer_xor` gauge")
            if settlement_buffer < args.min_settlement_buffer:
                raise SmokeError(
                    f"settlement buffer {settlement_buffer:.2f} below minimum {args.min_settlement_buffer:.2f}"
                )
            print(f"[ok] settlement buffer={settlement_buffer:.2f}")
            verify_lane_metrics(
                metrics,
                args.lane_alias,
                min_block_height=args.min_block_height,
                max_finality_lag=args.max_finality_lag,
                max_settlement_backlog=args.max_settlement_backlog,
                allow_missing=args.allow_missing_lane_metrics,
            )
            verify_slot_duration_histogram(
                metrics,
                max_p95=args.max_slot_p95,
                max_p99=args.max_slot_p99,
                min_samples=args.min_slot_samples,
            )
            if args.max_headroom_events is not None:
                verify_lane_headroom_events(
                    metrics,
                    args.lane_alias,
                    max_events=args.max_headroom_events,
                    allow_missing=args.allow_missing_lane_metrics,
                )
            verify_lane_teu_metrics(
                metrics,
                args.lane_alias,
                min_capacity=args.min_teu_capacity,
                max_slot_commit_ratio=args.max_teu_slot_commit_ratio,
                max_deferrals=args.max_teu_deferrals,
                max_truncations=args.max_must_serve_truncations,
                allow_missing=args.allow_missing_lane_metrics,
            )
        if telemetry_events is not None:
            print(f"[ok] Telemetry log validated: {args.telemetry_file}")
            if args.require_alias_migration:
                for prev_alias, new_alias in args.require_alias_migration:
                    print(f"[ok] alias migration recorded: {prev_alias} -> {new_alias}")
        print("Nexus lane smoke test passed.")
        return 0
    except SmokeError as exc:
        print(f"smoke test failed: {exc}", file=sys.stderr)
        return 1
    except urllib.error.URLError as exc:  # pragma: no cover - network failures are environment specific
        print(f"smoke test failed: unable to fetch endpoint: {exc}", file=sys.stderr)
        return 1


def parse_args(argv: Optional[Iterable[str]]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    status_group = parser.add_mutually_exclusive_group(required=True)
    status_group.add_argument(
        "--status-url",
        help="Full Torii status endpoint (e.g., https://host/v1/sumeragi/status)",
    )
    status_group.add_argument(
        "--status-file",
        help="Path to a recorded Torii status JSON payload",
    )
    metrics_group = parser.add_mutually_exclusive_group()
    metrics_group.add_argument(
        "--metrics-url",
        help="Prometheus metrics endpoint (e.g., https://host/metrics)",
    )
    metrics_group.add_argument(
        "--metrics-file",
        help="Path to a recorded Prometheus metrics text dump",
    )
    parser.add_argument(
        "--telemetry-file",
        "--from-telemetry",
        dest="telemetry_file",
        help=(
            "Path to a newline-delimited telemetry log containing "
            "`nexus.lane.topology` entries (for example, captured via "
            "`journalctl -u irohad -o json`)."
        ),
    )
    parser.add_argument(
        "--require-alias-migration",
        action="append",
        default=[],
        metavar="OLD:NEW",
        help=(
            "Ensure the telemetry log recorded an alias migration from OLD to "
            "NEW (requires --telemetry-file); repeat for multiple migrations."
        ),
    )
    parser.add_argument(
        "--lane-alias",
        action="append",
        default=[],
        help="Lane alias expected to report manifest_ready=true (repeatable)",
    )
    parser.add_argument(
        "--expected-lane-count",
        type=int,
        help="Expected value for `nexus_lane_configured_total` (requires --metrics-url)",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=15,
        help="HTTP timeout in seconds",
    )
    parser.add_argument(
        "--insecure",
        action="store_true",
        help="Disable TLS certificate verification (useful for staging)",
    )
    parser.add_argument(
        "--min-da-quorum",
        type=float,
        default=0.95,
        help="Minimum allowable value for `iroha_da_quorum_ratio` (default: 0.95).",
    )
    parser.add_argument(
        "--max-oracle-staleness",
        type=float,
        default=75.0,
        help="Maximum allowable `iroha_oracle_staleness_seconds` (default: 75).",
    )
    parser.add_argument(
        "--expected-oracle-twap",
        type=float,
        default=60.0,
        help="Expected `iroha_oracle_twap_window_seconds` value (default: 60).",
    )
    parser.add_argument(
        "--oracle-twap-tolerance",
        type=float,
        default=5.0,
        help="Allowed +/- drift for the TWAP window (default: 5).",
    )
    parser.add_argument(
        "--max-oracle-haircut-bps",
        type=float,
        default=100.0,
        help="Maximum allowable `iroha_oracle_haircut_basis_points` (default: 100).",
    )
    parser.add_argument(
        "--min-settlement-buffer",
        type=float,
        default=0.25,
        help="Minimum allowable `iroha_settlement_buffer_xor` (default: 0.25).",
    )
    parser.add_argument(
        "--min-block-height",
        type=int,
        default=1,
        help=(
            "Minimum acceptable `nexus_lane_block_height` per alias "
            "(set <=0 to disable this check)."
        ),
    )
    parser.add_argument(
        "--max-finality-lag",
        type=float,
        default=4.0,
        help=(
            "Maximum acceptable `nexus_lane_finality_lag_slots` per alias "
            "(set <0 to disable this check)."
        ),
    )
    parser.add_argument(
        "--max-settlement-backlog",
        type=float,
        default=1.0,
        help=(
            "Maximum acceptable `nexus_lane_settlement_backlog_xor` per alias "
            "(set <0 to disable this check)."
        ),
    )
    parser.add_argument(
        "--max-headroom-events",
        type=float,
        help=(
            "Maximum allowable `nexus_scheduler_lane_headroom_events_total` per alias "
            "(set <0 to disable this check)."
        ),
    )
    parser.add_argument(
        "--min-teu-capacity",
        type=float,
        default=1.0,
        help=(
            "Minimum allowable `nexus_scheduler_lane_teu_capacity` per alias "
            "(set <=0 to disable)."
        ),
    )
    parser.add_argument(
        "--max-teu-slot-commit-ratio",
        type=float,
        default=0.9,
        help=(
            "Maximum slot_committed/capacity ratio enforced per lane "
            "(set <=0 to disable)."
        ),
    )
    parser.add_argument(
        "--max-teu-deferrals",
        type=float,
        default=0.0,
        help=(
            "Maximum allowable cumulative `nexus_scheduler_lane_teu_deferral_total` per lane "
            "(set <0 to disable)."
        ),
    )
    parser.add_argument(
        "--max-must-serve-truncations",
        type=float,
        default=0.0,
        help=(
            "Maximum allowable `nexus_scheduler_must_serve_truncations_total` per lane "
            "(set <0 to disable)."
        ),
    )
    parser.add_argument(
        "--allow-missing-lane-metrics",
        action="store_true",
        help=(
            "Downgrade missing per-lane metrics to warnings instead of failing the run."
        ),
    )
    parser.add_argument(
        "--max-slot-p95",
        type=float,
        default=None,
        help=(
            "Maximum allowable slot-duration p95 in milliseconds "
            "(requires metrics histogram; omit to skip)."
        ),
    )
    parser.add_argument(
        "--max-slot-p99",
        type=float,
        default=None,
        help=(
            "Maximum allowable slot-duration p99 in milliseconds "
            "(requires metrics histogram; omit to skip)."
        ),
    )
    parser.add_argument(
        "--min-slot-samples",
        type=int,
        default=10,
        help=(
            "Minimum number of slot-duration samples required before enforcing "
            "p95/p99 limits (default: 10)."
        ),
    )
    args = parser.parse_args(list(argv) if argv is not None else None)
    if args.expected_lane_count is not None and not (args.metrics_url or args.metrics_file):
        parser.error("--expected-lane-count requires --metrics-url or --metrics-file")
    if not args.lane_alias:
        parser.error("provide at least one --lane-alias to verify")
    alias_migrations: List[Tuple[str, str]] = []
    for spec in args.require_alias_migration:
        alias_migrations.append(_parse_alias_migration_spec(spec))
    if alias_migrations and not args.telemetry_file:
        parser.error("--require-alias-migration requires --telemetry-file/--from-telemetry")
    args.require_alias_migration = alias_migrations
    return args


def fetch_text(url: str, timeout: int, insecure: bool) -> str:
    headers = {"User-Agent": "nexus-lane-smoke/1.0"}
    request = urllib.request.Request(url, headers=headers)
    context = ssl._create_unverified_context() if insecure else None
    with urllib.request.urlopen(request, timeout=timeout, context=context) as response:
        charset = response.headers.get_content_charset() or "utf-8"
        return response.read().decode(charset)


def fetch_json(url: str, timeout: int, insecure: bool) -> Dict:
    text = fetch_text(url, timeout=timeout, insecure=insecure)
    try:
        return json.loads(text)
    except json.JSONDecodeError as exc:
        raise SmokeError(f"status endpoint returned invalid JSON: {exc}") from exc


def load_status_source(
    status_url: Optional[str],
    status_file: Optional[str],
    timeout: int,
    insecure: bool,
) -> Tuple[Dict, str]:
    if status_file:
        return read_json_file(status_file, label="status"), status_file
    if not status_url:
        raise SmokeError("missing status source (--status-url or --status-file)")
    return fetch_json(status_url, timeout=timeout, insecure=insecure), status_url


def load_metrics_source(
    metrics_url: Optional[str],
    metrics_file: Optional[str],
    timeout: int,
    insecure: bool,
) -> Optional[Tuple[str, str]]:
    if metrics_file:
        return read_text_file(metrics_file, label="metrics"), metrics_file
    if metrics_url:
        return (
            fetch_text(metrics_url, timeout=timeout, insecure=insecure),
            metrics_url,
        )
    return None


def read_json_file(path: str, *, label: str) -> Dict:
    try:
        contents = Path(path).read_text(encoding="utf-8")
    except OSError as exc:
        raise SmokeError(f"unable to read {label} file `{path}`: {exc}") from exc
    try:
        return json.loads(contents)
    except json.JSONDecodeError as exc:
        raise SmokeError(f"{label} file `{path}` contained invalid JSON: {exc}") from exc


def read_text_file(path: str, *, label: str) -> str:
    try:
        return Path(path).read_text(encoding="utf-8")
    except OSError as exc:
        raise SmokeError(f"unable to read {label} file `{path}`: {exc}") from exc


def verify_lane_ready(status: Dict, alias: str) -> LaneCheck:
    lanes = status.get("lane_governance")
    if not isinstance(lanes, list):
        raise SmokeError("status payload missing `lane_governance` array")
    for entry in lanes:
        if entry.get("alias") != alias:
            continue
        manifest_ready = bool(entry.get("manifest_ready"))
        if not manifest_ready:
            raise SmokeError(f"lane `{alias}` is not ready (manifest_ready=false)")
        manifest_required = bool(entry.get("manifest_required", True))
        validators = entry.get("validator_ids")
        validator_count = len(validators) if isinstance(validators, list) else 0
        quorum_value = entry.get("quorum")
        manifest_path = entry.get("manifest_path")
        if manifest_required:
            if not isinstance(manifest_path, str) or not manifest_path.strip():
                raise SmokeError(
                    f"lane `{alias}` requires a manifest but `manifest_path` is missing"
                )
            manifest_path = manifest_path.strip()
        elif isinstance(manifest_path, str):
            manifest_path = manifest_path.strip() or None
        else:
            manifest_path = None
        return LaneCheck(
            alias=alias,
            manifest_required=manifest_required,
            manifest_ready=manifest_ready,
            manifest_path=manifest_path,
            validators=validator_count,
            quorum=int(quorum_value) if isinstance(quorum_value, (int, float)) else None,
        )
    raise SmokeError(f"lane `{alias}` not found in Torii status payload")


def parse_prometheus_metrics(text: str) -> Dict[str, List[Tuple[Dict[str, str], float]]]:
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]] = {}
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            continue
        descriptor, value_text = parts
        name, labels = _split_descriptor(descriptor)
        try:
            value = float(value_text.split()[0])
        except ValueError:
            continue
        metrics.setdefault(name, []).append((labels, value))
    return metrics


def _split_descriptor(descriptor: str) -> Tuple[str, Dict[str, str]]:
    if "{" not in descriptor:
        return descriptor, {}
    name, rest = descriptor.split("{", 1)
    rest = rest.rstrip("}")
    labels: Dict[str, str] = {}
    if rest:
        for chunk in rest.split(","):
            if "=" not in chunk:
                continue
            key, value = chunk.split("=", 1)
            labels[key.strip()] = value.strip().strip('"')
    return name, labels


def get_metric_value(
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]],
    name: str,
    labels: Optional[Dict[str, str]] = None,
) -> Optional[float]:
    entries = metrics.get(name)
    if not entries:
        return None
    if not labels:
        return entries[0][1]
    for entry_labels, value in entries:
        if all(entry_labels.get(k) == v for k, v in labels.items()):
            return value
    return None


def verify_lane_metrics(
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]],
    lane_aliases: Sequence[str],
    *,
    min_block_height: int,
    max_finality_lag: float,
    max_settlement_backlog: float,
    allow_missing: bool,
) -> None:
    enforce_height = min_block_height > 0
    enforce_finality = max_finality_lag >= 0
    enforce_backlog = max_settlement_backlog >= 0
    for alias in lane_aliases:
        height = _lane_metric(metrics, "nexus_lane_block_height", alias, allow_missing)
        finality = _lane_metric(
            metrics, "nexus_lane_finality_lag_slots", alias, allow_missing
        )
        backlog = _lane_metric(
            metrics, "nexus_lane_settlement_backlog_xor", alias, allow_missing
        )
        if height is not None and enforce_height and height < min_block_height:
            raise SmokeError(
                f"lane `{alias}` block height {height:.0f} below minimum {min_block_height}"
            )
        if finality is not None and enforce_finality and finality > max_finality_lag:
            raise SmokeError(
                f"lane `{alias}` finality lag {finality:.2f} exceeds {max_finality_lag:.2f}"
            )
        if backlog is not None and enforce_backlog and backlog > max_settlement_backlog:
            raise SmokeError(
                f"lane `{alias}` settlement backlog {backlog:.3f} exceeds "
                f"{max_settlement_backlog:.3f}"
            )
        parts = []
        if height is not None:
            parts.append(f"height={int(height)}")
        if finality is not None:
            parts.append(f"finality_lag={finality:.2f}")
        if backlog is not None:
            parts.append(f"settlement_backlog={backlog:.3f}")
        if parts:
            print(f"[ok] lane `{alias}` metrics: {', '.join(parts)}")


def _lane_metric(
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]],
    name: str,
    alias: str,
    allow_missing: bool,
    *,
    label_key: str = "alias",
) -> Optional[float]:
    value = get_metric_value(metrics, name, labels={label_key: alias})
    if value is None and not allow_missing:
        raise SmokeError(
            f"metrics missing `{name}` for lane `{alias}`; "
            "rerun with --allow-missing-lane-metrics to skip this check"
        )
    if value is None:
        print(
            f"[warn] lane `{alias}` metric `{name}` missing; "
            "skipping due to --allow-missing-lane-metrics"
        )
    return value


def _lane_counter_total(
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]],
    name: str,
    alias: str,
) -> float:
    entries = metrics.get(name)
    if not entries:
        return 0.0
    total = 0.0
    for labels, value in entries:
        if labels.get("lane") == alias:
            total += value
    return total


def verify_lane_governance_sealed(
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]],
    lane_aliases: Sequence[str],
    *,
    allow_missing: bool,
) -> None:
    for alias in lane_aliases:
        value = get_metric_value(
            metrics,
            "nexus_lane_governance_sealed",
            labels={"alias": alias},
        )
        if value is None:
            if allow_missing:
                print(
                    "[warn] lane "
                    f"`{alias}` metric `nexus_lane_governance_sealed` missing; "
                    "skipping due to --allow-missing-lane-metrics"
                )
                continue
            raise SmokeError(
                "metrics missing `nexus_lane_governance_sealed` "
                f"for lane `{alias}`; rerun with --allow-missing-lane-metrics to skip"
            )
        if value > 0:
            raise SmokeError(
                f"metrics report alias `{alias}` as sealed "
                "(nexus_lane_governance_sealed > 0)"
            )
        print(f"[ok] lane `{alias}` governance sealed={value:.0f}")


def verify_lane_headroom_events(
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]],
    lane_aliases: Sequence[str],
    *,
    max_events: float,
    allow_missing: bool,
) -> None:
    if max_events < 0:
        return
    for alias in lane_aliases:
        value = get_metric_value(
            metrics,
            "nexus_scheduler_lane_headroom_events_total",
            labels={"lane": alias},
        )
        if value is None and not allow_missing:
            raise SmokeError(
                "metrics missing `nexus_scheduler_lane_headroom_events_total` "
                f"for lane `{alias}`; rerun with --allow-missing-lane-metrics to skip"
            )
        if value is None:
            print(
                "[warn] lane "
                f"`{alias}` metric `nexus_scheduler_lane_headroom_events_total` missing; "
                "skipping due to --allow-missing-lane-metrics"
            )
            continue
        if value > max_events:
            raise SmokeError(
                f"lane `{alias}` headroom events {value:.0f} exceed {max_events:.0f}"
            )
        print(f"[ok] lane `{alias}` headroom events={value:.0f}")


def verify_lane_teu_metrics(
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]],
    lane_aliases: Sequence[str],
    *,
    min_capacity: float,
    max_slot_commit_ratio: float,
    max_deferrals: float,
    max_truncations: float,
    allow_missing: bool,
) -> None:
    enforce_capacity = min_capacity > 0
    enforce_ratio = max_slot_commit_ratio > 0
    enforce_deferrals = max_deferrals >= 0
    enforce_truncations = max_truncations >= 0
    if not any([enforce_capacity, enforce_ratio, enforce_deferrals, enforce_truncations]):
        return
    for alias in lane_aliases:
        summary_parts = []
        capacity = _lane_metric(
            metrics,
            "nexus_scheduler_lane_teu_capacity",
            alias,
            allow_missing,
            label_key="lane",
        )
        if capacity is not None:
            summary_parts.append(f"capacity={capacity:.1f}")
            if enforce_capacity and capacity < min_capacity:
                raise SmokeError(
                    f"lane `{alias}` TEU capacity {capacity:.2f} below minimum {min_capacity:.2f}"
                )
        slot_committed = _lane_metric(
            metrics,
            "nexus_scheduler_lane_teu_slot_committed",
            alias,
            allow_missing,
            label_key="lane",
        )
        if (
            enforce_ratio
            and capacity is not None
            and slot_committed is not None
            and capacity > 0
        ):
            ratio = slot_committed / capacity
            summary_parts.append(f"slot_committed={slot_committed:.1f}")
            summary_parts.append(f"commit_ratio={ratio:.2f}")
            if ratio > max_slot_commit_ratio:
                raise SmokeError(
                    f"lane `{alias}` TEU slot commitment ratio {ratio:.3f} exceeds "
                    f"{max_slot_commit_ratio:.3f}"
                )
        elif slot_committed is not None:
            summary_parts.append(f"slot_committed={slot_committed:.1f}")
        deferrals = _lane_counter_total(
            metrics,
            "nexus_scheduler_lane_teu_deferral_total",
            alias,
        )
        if enforce_deferrals and deferrals > max_deferrals:
            raise SmokeError(
                f"lane `{alias}` TEU deferrals {deferrals:.0f} exceed {max_deferrals:.0f}"
            )
        if deferrals:
            summary_parts.append(f"deferrals={deferrals:.0f}")
        truncations = _lane_counter_total(
            metrics,
            "nexus_scheduler_must_serve_truncations_total",
            alias,
        )
        if enforce_truncations and truncations > max_truncations:
            raise SmokeError(
                f"lane `{alias}` must-serve truncations {truncations:.0f} exceed "
                f"{max_truncations:.0f}"
            )
        if truncations:
            summary_parts.append(f"must_serve={truncations:.0f}")
        if summary_parts:
            print(f"[ok] lane `{alias}` TEU scheduler: {', '.join(summary_parts)}")


def verify_slot_duration_histogram(
    metrics: Dict[str, List[Tuple[Dict[str, str], float]]],
    *,
    max_p95: Optional[float],
    max_p99: Optional[float],
    min_samples: int,
) -> None:
    if max_p95 is None and max_p99 is None:
        return
    buckets = metrics.get("iroha_slot_duration_ms_bucket")
    if not buckets:
        raise SmokeError(
            "metrics missing `iroha_slot_duration_ms_bucket`; capture a Prometheus dump "
            "that includes the slot-duration histogram or omit --max-slot-* thresholds"
        )
    count = get_metric_value(metrics, "iroha_slot_duration_ms_count")
    if count is None:
        raise SmokeError(
            "metrics missing `iroha_slot_duration_ms_count`; capture a Prometheus dump "
            "that includes the histogram count or omit --max-slot-* thresholds"
        )
    total_samples = int(count)
    if total_samples < min_samples:
        raise SmokeError(
            "slot-duration histogram does not have enough samples "
            f"(observed {total_samples}, require >= {min_samples})"
        )
    summaries = [f"samples={total_samples}"]
    if max_p95 is not None:
        p95 = _histogram_quantile(buckets, total_samples, 0.95)
        if not math.isfinite(p95):
            raise SmokeError("unable to compute slot-duration p95 from histogram buckets")
        if p95 > max_p95:
            raise SmokeError(
                f"slot-duration p95 {p95:.1f}ms exceeds limit {max_p95:.1f}ms"
            )
        summaries.append(f"p95={p95:.1f}ms")
    if max_p99 is not None:
        p99 = _histogram_quantile(buckets, total_samples, 0.99)
        if not math.isfinite(p99):
            raise SmokeError("unable to compute slot-duration p99 from histogram buckets")
        if p99 > max_p99:
            raise SmokeError(
                f"slot-duration p99 {p99:.1f}ms exceeds limit {max_p99:.1f}ms"
            )
        summaries.append(f"p99={p99:.1f}ms")
    print(f"[ok] slot duration {' '.join(summaries)}")


def _histogram_quantile(
    buckets: Sequence[Tuple[Dict[str, str], float]],
    total_samples: int,
    quantile: float,
) -> float:
    if total_samples <= 0:
        return float("inf")
    target = max(1, math.ceil(total_samples * quantile))
    samples: List[Tuple[float, float]] = []
    for labels, value in buckets:
        raw_bound = labels.get("le")
        if raw_bound is None:
            continue
        bound = float("inf") if raw_bound == "+Inf" else float(raw_bound)
        samples.append((bound, value))
    samples.sort(key=lambda item: item[0])
    for upper_bound, cumulative in samples:
        if cumulative >= target:
            return upper_bound
    return float("inf")


def load_telemetry_events(path: str) -> List[Dict]:
    """Load nexus.lane.topology events from a newline-delimited log."""

    try:
        contents = Path(path).read_text(encoding="utf-8")
    except OSError as exc:
        raise SmokeError(f"unable to read telemetry file `{path}`: {exc}") from exc

    events: List[Dict] = []
    for line_number, raw_line in enumerate(contents.splitlines(), start=1):
        line = raw_line.strip()
        if not line:
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            raise SmokeError(
                f"telemetry line {line_number} is not valid JSON: {exc}"
            ) from exc
        msg_value = record.get("msg") or record.get("MESSAGE")
        if msg_value != "nexus.lane.topology":
            continue
        payload_raw = record.get("event") or record.get("payload") or record
        if isinstance(payload_raw, str):
            try:
                payload = json.loads(payload_raw)
            except json.JSONDecodeError as exc:
                raise SmokeError(
                    f"telemetry line {line_number} has invalid event payload: {exc}"
                ) from exc
        elif isinstance(payload_raw, dict):
            payload = payload_raw
        else:
            raise SmokeError(
                f"telemetry line {line_number} missing JSON payload (event/payload keys)"
            )
        events.append(payload)

    if not events:
        raise SmokeError(
            f"telemetry file `{path}` did not contain any `nexus.lane.topology` events"
        )
    return events


def verify_telemetry_aliases(aliases: Sequence[str], events: Sequence[Dict]) -> None:
    """Ensure telemetry captured an event for each alias."""

    for alias in aliases:
        if not _telemetry_has_alias(alias, events):
            raise SmokeError(
                f"telemetry log missing `nexus.lane.topology` event for alias `{alias}`"
            )


def verify_alias_migrations(
    expected: Sequence[Tuple[str, str]],
    events: Sequence[Dict],
) -> None:
    """Ensure telemetry logged each alias migration."""

    for old_alias, new_alias in expected:
        found = any(
            event.get("action") == "alias_migrated"
            and event.get("alias_before") == old_alias
            and event.get("alias_after") == new_alias
            for event in events
        )
        if not found:
            raise SmokeError(
                "telemetry log missing alias_migrated event "
                f"from `{old_alias}` to `{new_alias}`"
            )


def _telemetry_has_alias(alias: str, events: Sequence[Dict]) -> bool:
    for event in events:
        for key in ("alias", "alias_before", "alias_after"):
            value = event.get(key)
            if isinstance(value, str) and value == alias:
                return True
    return False


def _parse_alias_migration_spec(spec: str) -> Tuple[str, str]:
    if ":" not in spec:
        raise SmokeError(f"alias migration spec `{spec}` must be formatted as OLD:NEW")
    old_alias, new_alias = spec.split(":", 1)
    old_alias = old_alias.strip()
    new_alias = new_alias.strip()
    if not old_alias or not new_alias:
        raise SmokeError(f"alias migration spec `{spec}` cannot be empty")
    return old_alias, new_alias


if __name__ == "__main__":
    sys.exit(main())
