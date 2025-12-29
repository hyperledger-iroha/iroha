#!/usr/bin/env python3
"""Gate NX-18 slot-duration metrics captured from Prometheus exports.

The helper consumes a Prometheus text exposition that contains the
``iroha_slot_duration_ms`` histogram and latest-sample gauge emitted by
`iroha_telemetry`. It aggregates every bucket (across all label sets),
computes the p50/p95/p99 quantiles, emits a short summary, and exits with a
non-zero status whenever the configured thresholds are violated.

By default the script enforces the NX-18 acceptance criteria:

* histogram p95 ≤ 1000 ms
* histogram p99 ≤ 1100 ms

Examples::

    # Parse an extracted metrics snapshot and use the default thresholds.
    scripts/telemetry/check_slot_duration.py \
        artifacts/nx18/prometheus/metrics.prom

    # Tighten the thresholds and require at least 500 samples.
    scripts/telemetry/check_slot_duration.py \
        --max-p95-ms 900 \
        --max-p99-ms 1050 \
        --min-samples 500 \
        artifacts/nx18/prometheus/metrics.prom
"""

from __future__ import annotations

import argparse
import math
import re
from pathlib import Path
import json
from typing import Dict, Iterable, List, Mapping, MutableMapping, Optional, Tuple

BUCKET_METRIC = "iroha_slot_duration_ms_bucket"
LATEST_METRIC = "iroha_slot_duration_ms_latest"
METRIC_RE = re.compile(
    r"""
    ^
    (?P<name>[a-zA-Z_:][a-zA-Z0-9_:]*)
    (?P<labels>\{[^}]*\})?
    \s+
    (?P<value>[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?)
    (?:\s+(?P<timestamp>\d+))?
    $
    """,
    re.VERBOSE,
)


class SlotStats:
    """Aggregated slot-duration statistics."""

    __slots__ = ("count", "p50_ms", "p95_ms", "p99_ms", "latest_ms")

    def __init__(
        self,
        *,
        count: float,
        p50_ms: float,
        p95_ms: float,
        p99_ms: float,
        latest_ms: Optional[float],
    ) -> None:
        self.count = count
        self.p50_ms = p50_ms
        self.p95_ms = p95_ms
        self.p99_ms = p99_ms
        self.latest_ms = latest_ms


class SlotDurationError(RuntimeError):
    """Raised when the metrics snapshot is missing required series."""


def parse_labels(payload: str) -> Mapping[str, str]:
    """Parse a Prometheus label payload (``{key="value",...}``)."""

    if not payload:
        return {}
    if payload[0] != "{" or payload[-1] != "}":
        raise SlotDurationError(f"invalid label payload: {payload!r}")

    labels: Dict[str, str] = {}
    i = 1
    length = len(payload)
    while i < length - 1:
        while i < length - 1 and payload[i] in " ,":
            i += 1
        if i >= length - 1:
            break
        start = i
        while i < length - 1 and payload[i] not in "=\n":
            if payload[i] == ",":
                break
            i += 1
        key = payload[start:i].strip()
        if not key:
            raise SlotDurationError(f"empty label key in {payload!r}")
        if i >= length - 1 or payload[i] != "=":
            raise SlotDurationError(f"malformed label assignment in {payload!r}")
        i += 1
        if i >= length - 1 or payload[i] != '"':
            raise SlotDurationError(f'label "{key}" missing opening quote in {payload!r}')
        i += 1
        value_chars: List[str] = []
        while i < length:
            char = payload[i]
            if char == "\\":
                i += 1
                if i >= length - 1:
                    raise SlotDurationError(f"unterminated escape in {payload!r}")
                value_chars.append(payload[i])
            elif char == '"':
                break
            else:
                value_chars.append(char)
            i += 1
        else:
            raise SlotDurationError(f'label "{key}" missing closing quote in {payload!r}')
        value = "".join(value_chars)
        labels[key] = value
        i += 1  # skip closing quote
        while i < length and payload[i] in ", ":
            i += 1
    return labels


def parse_metrics(lines: Iterable[str]) -> Iterable[Tuple[str, Mapping[str, str], float]]:
    """Yield ``(name, labels, value)`` tuples for every metric sample."""

    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        match = METRIC_RE.match(stripped)
        if not match:
            continue
        name = match.group("name")
        labels = parse_labels(match.group("labels") or "")
        value = float(match.group("value"))
        yield name, labels, value


def _aggregate_histogram_buckets(
    metrics: Iterable[Tuple[str, Mapping[str, str], float]]
) -> Tuple[MutableMapping[str, float], List[float]]:
    buckets: MutableMapping[str, float] = {}
    latest_samples: List[float] = []
    for name, labels, value in metrics:
        if name == BUCKET_METRIC:
            le = labels.get("le")
            if not le:
                raise SlotDurationError("slot histogram bucket is missing the `le` label")
            buckets[le] = buckets.get(le, 0.0) + value
        elif name == LATEST_METRIC:
            latest_samples.append(value)
    return buckets, latest_samples


def _normalise_bounds(buckets: Mapping[str, float]) -> List[Tuple[float, float]]:
    if not buckets:
        raise SlotDurationError("no slot histogram buckets were found in the metrics snapshot")
    normalised: List[Tuple[float, float]] = []
    for bound_str, cumulative in buckets.items():
        if bound_str == "+Inf":
            bound = math.inf
        else:
            try:
                bound = float(bound_str)
            except ValueError as exc:
                raise SlotDurationError(f"invalid bucket bound: {bound_str!r}") from exc
        normalised.append((bound, cumulative))
    normalised.sort(key=lambda item: item[0])
    return normalised


def _compute_quantile(buckets: List[Tuple[float, float]], quantile: float) -> float:
    if not buckets:
        raise SlotDurationError("slot histogram is empty")
    total = buckets[-1][1]
    if total <= 0:
        raise SlotDurationError("slot histogram total count must be positive")
    target = total * quantile
    previous_bound = 0.0
    previous_count = 0.0
    for bound, cumulative in buckets:
        if cumulative >= target:
            if math.isinf(bound):
                return previous_bound
            bucket_count = cumulative - previous_count
            if bucket_count <= 0 or math.isinf(bound):
                return bound
            fraction = (target - previous_count) / bucket_count
            span = bound - previous_bound
            if span <= 0:
                return bound
            return previous_bound + fraction * span
        previous_bound = bound
        previous_count = cumulative
    return buckets[-1][0]


def load_slot_stats(path: Path) -> SlotStats:
    """Return aggregated slot statistics extracted from *path*."""

    with path.open("r", encoding="utf-8") as handle:
        buckets, latest_samples = _aggregate_histogram_buckets(parse_metrics(handle))
    normalised = _normalise_bounds(buckets)
    p50 = _compute_quantile(normalised, 0.50)
    p95 = _compute_quantile(normalised, 0.95)
    p99 = _compute_quantile(normalised, 0.99)
    latest = max(latest_samples) if latest_samples else None
    return SlotStats(count=normalised[-1][1], p50_ms=p50, p95_ms=p95, p99_ms=p99, latest_ms=latest)


def parse_args(argv: Optional[Iterable[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check NX-18 slot-duration metrics from a Prometheus export."
    )
    parser.add_argument(
        "metrics_path",
        type=Path,
        help="Path to a Prometheus metrics file containing iroha_slot_duration_ms buckets.",
    )
    parser.add_argument(
        "--max-p95-ms",
        type=float,
        default=1000.0,
        help="Maximum allowed p95 slot duration (default: 1000 ms).",
    )
    parser.add_argument(
        "--max-p99-ms",
        type=float,
        default=1100.0,
        help="Maximum allowed p99 slot duration (default: 1100 ms).",
    )
    parser.add_argument(
        "--min-samples",
        type=float,
        default=1.0,
        help="Minimum slot-sample count required before gating (default: 1).",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress human-readable output (exit codes still apply).",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write the computed summary as JSON to the given path.",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def _emit_summary(stats: SlotStats) -> None:
    print("Nexus slot-duration summary:")
    print(f"  samples: {stats.count:.0f}")
    print(f"  p50: {stats.p50_ms:.2f} ms")
    print(f"  p95: {stats.p95_ms:.2f} ms")
    print(f"  p99: {stats.p99_ms:.2f} ms")
    if stats.latest_ms is not None:
        print(f"  latest sample: {stats.latest_ms:.2f} ms")


def check_thresholds(stats: SlotStats, *, max_p95: float, max_p99: float, min_samples: float) -> int:
    if stats.count < min_samples:
        print(
            f"[error] insufficient slot samples: required {min_samples}, observed {stats.count}",
            flush=True,
        )
        return 3
    violations: List[str] = []
    if stats.p95_ms > max_p95:
        violations.append(f"p95 {stats.p95_ms:.2f} ms exceeds {max_p95} ms")
    if stats.p99_ms > max_p99:
        violations.append(f"p99 {stats.p99_ms:.2f} ms exceeds {max_p99} ms")
    if violations:
        for violation in violations:
            print(f"[error] {violation}", flush=True)
        return 2
    return 0


def main(argv: Optional[Iterable[str]] = None) -> int:
    args = parse_args(argv)
    metrics_path = args.metrics_path.resolve()
    if not metrics_path.exists():
        raise SystemExit(f"[error] metrics file '{metrics_path}' does not exist")
    try:
        stats = load_slot_stats(metrics_path)
    except SlotDurationError as exc:
        raise SystemExit(f"[error] {exc}") from exc
    if not args.quiet:
        _emit_summary(stats)
    exit_code = check_thresholds(
        stats,
        max_p95=args.max_p95_ms,
        max_p99=args.max_p99_ms,
        min_samples=args.min_samples,
    )
    if args.json_out is not None:
        report = {
            "metrics_path": str(metrics_path),
            "samples": stats.count,
            "p50_ms": stats.p50_ms,
            "p95_ms": stats.p95_ms,
            "p99_ms": stats.p99_ms,
            "latest_ms": stats.latest_ms,
            "thresholds": {
                "max_p95_ms": args.max_p95_ms,
                "max_p99_ms": args.max_p99_ms,
                "min_samples": args.min_samples,
            },
            "exit_code": exit_code,
        }
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
