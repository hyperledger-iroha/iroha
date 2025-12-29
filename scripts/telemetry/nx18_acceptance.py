#!/usr/bin/env python3
"""NX-18 acceptance harness for slot, DA, oracle, and settlement telemetry.

This helper ingests a Prometheus text exposition (typically scraped during a
chaos or load run), enforces the NX-18 thresholds, prints a short report, and
optionally writes a JSON summary for evidence bundles.

Default thresholds match the NX-18 runbook:
- slot p95 ≤ 1000 ms, p99 ≤ 1100 ms (with a minimum sample count)
- DA quorum ratio ≥ 0.95
- oracle staleness ≤ 90 s, TWAP window 60 s ± 5 s, haircut ≤ 100 bps
- settlement buffer ≥ 25 %
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, Optional, Tuple

# Reuse the slot-duration parser/quantile helpers from check_slot_duration.py
_SLOT_SPEC = importlib.util.spec_from_file_location(
    "check_slot_duration",
    Path(__file__).resolve().parent / "check_slot_duration.py",
)
assert _SLOT_SPEC and _SLOT_SPEC.loader  # pragma: no cover - import guard
slot_mod = importlib.util.module_from_spec(_SLOT_SPEC)
_SLOT_SPEC.loader.exec_module(slot_mod)  # type: ignore[misc]


class AcceptanceError(RuntimeError):
    """Raised when the metrics snapshot is invalid or incomplete."""


@dataclass
class Thresholds:
    max_slot_p95: float
    max_slot_p99: float
    min_slot_samples: float
    min_da_quorum: float
    max_oracle_staleness: float
    expected_oracle_twap: float
    oracle_twap_tolerance: float
    max_oracle_haircut_bps: float
    min_settlement_buffer: float


def parse_args(argv: Optional[Iterable[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "metrics_path",
        type=Path,
        help="Path to a Prometheus text exposition containing NX-18 metrics.",
    )
    parser.add_argument(
        "--max-slot-p95",
        type=float,
        default=1000.0,
        help="Maximum allowed p95 slot duration (default: 1000 ms).",
    )
    parser.add_argument(
        "--max-slot-p99",
        type=float,
        default=1100.0,
        help="Maximum allowed p99 slot duration (default: 1100 ms).",
    )
    parser.add_argument(
        "--min-slot-samples",
        type=float,
        default=10.0,
        help="Minimum slot-sample count required before gating (default: 10).",
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
        default=90.0,
        help="Maximum allowable `iroha_oracle_staleness_seconds` (default: 90).",
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
        "--json-out",
        type=Path,
        help="Write the computed summary as JSON to the given path.",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress human-readable output (exit codes still apply).",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def load_metrics(path: Path) -> Dict[str, List[Tuple[Mapping[str, str], float]]]:
    try:
        contents = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise AcceptanceError(f"unable to read metrics from {path}: {exc}") from exc

    metrics: Dict[str, List[Tuple[Mapping[str, str], float]]] = {}
    for name, labels, value in slot_mod.parse_metrics(contents.splitlines()):
        metrics.setdefault(name, []).append((labels, value))
    return metrics


def require_single_metric(
    metrics: Dict[str, List[Tuple[Mapping[str, str], float]]],
    name: str,
) -> float:
    samples = metrics.get(name, [])
    if not samples:
        raise AcceptanceError(f"metrics snapshot missing required series `{name}`")
    if len(samples) > 1:
        raise AcceptanceError(
            f"expected a single sample for `{name}`, found {len(samples)} entries",
        )
    return samples[0][1]


def evaluate_acceptance(
    metrics_path: Path, thresholds: Thresholds
) -> Tuple[Dict[str, object], List[str]]:
    slot_stats = slot_mod.load_slot_stats(metrics_path)
    metrics = load_metrics(metrics_path)
    summary: Dict[str, object] = {
        "metrics_path": str(metrics_path),
        "thresholds": {
            "max_slot_p95": thresholds.max_slot_p95,
            "max_slot_p99": thresholds.max_slot_p99,
            "min_slot_samples": thresholds.min_slot_samples,
            "min_da_quorum": thresholds.min_da_quorum,
            "max_oracle_staleness": thresholds.max_oracle_staleness,
            "expected_oracle_twap": thresholds.expected_oracle_twap,
            "oracle_twap_tolerance": thresholds.oracle_twap_tolerance,
            "max_oracle_haircut_bps": thresholds.max_oracle_haircut_bps,
            "min_settlement_buffer": thresholds.min_settlement_buffer,
        },
    }

    failures: List[str] = []

    slot_ok = (
        slot_stats.count >= thresholds.min_slot_samples
        and slot_stats.p95_ms <= thresholds.max_slot_p95
        and slot_stats.p99_ms <= thresholds.max_slot_p99
    )
    summary["slot"] = {
        "ok": slot_ok,
        "samples": slot_stats.count,
        "p50_ms": slot_stats.p50_ms,
        "p95_ms": slot_stats.p95_ms,
        "p99_ms": slot_stats.p99_ms,
        "latest_ms": slot_stats.latest_ms,
    }
    if not slot_ok:
        failures.append(
            "slot-duration thresholds breached "
            f"(samples={slot_stats.count:.0f}, p95={slot_stats.p95_ms:.1f} ms, "
            f"p99={slot_stats.p99_ms:.1f} ms)",
        )

    da_ratio = require_single_metric(metrics, "iroha_da_quorum_ratio")
    da_ok = da_ratio >= thresholds.min_da_quorum
    summary["da_quorum"] = {"ok": da_ok, "ratio": da_ratio}
    if not da_ok:
        failures.append(
            f"DA quorum ratio {da_ratio:.3f} below minimum {thresholds.min_da_quorum}",
        )

    staleness = require_single_metric(metrics, "iroha_oracle_staleness_seconds")
    twap = require_single_metric(metrics, "iroha_oracle_twap_window_seconds")
    haircut = require_single_metric(metrics, "iroha_oracle_haircut_basis_points")
    oracle_ok = (
        staleness <= thresholds.max_oracle_staleness
        and abs(twap - thresholds.expected_oracle_twap)
        <= thresholds.oracle_twap_tolerance
        and haircut <= thresholds.max_oracle_haircut_bps
    )
    summary["oracle"] = {
        "ok": oracle_ok,
        "staleness_seconds": staleness,
        "twap_window_seconds": twap,
        "haircut_bps": haircut,
    }
    if not oracle_ok:
        if staleness > thresholds.max_oracle_staleness:
            failures.append(
                f"oracle staleness {staleness:.1f}s exceeds "
                f"{thresholds.max_oracle_staleness:.1f}s",
            )
        if abs(twap - thresholds.expected_oracle_twap) > thresholds.oracle_twap_tolerance:
            failures.append(
                "oracle TWAP window "
                f"{twap:.1f}s deviates from {thresholds.expected_oracle_twap:.1f}s "
                f"by more than {thresholds.oracle_twap_tolerance:.1f}s",
            )
        if haircut > thresholds.max_oracle_haircut_bps:
            failures.append(
                f"oracle haircut {haircut:.1f} bps exceeds "
                f"{thresholds.max_oracle_haircut_bps:.1f} bps",
            )

    settlement_buffer = require_single_metric(metrics, "iroha_settlement_buffer_xor")
    buffer_ok = settlement_buffer >= thresholds.min_settlement_buffer
    summary["settlement_buffer"] = {
        "ok": buffer_ok,
        "value": settlement_buffer,
    }
    if not buffer_ok:
        failures.append(
            f"settlement buffer {settlement_buffer:.2f} below "
            f"minimum {thresholds.min_settlement_buffer:.2f}",
        )

    overall_ok = slot_ok and da_ok and oracle_ok and buffer_ok
    summary["ok"] = overall_ok
    summary["failures"] = failures
    summary["exit_code"] = 0 if overall_ok else 1
    return summary, failures


def emit_summary(summary: Dict[str, object], *, quiet: bool) -> None:
    if quiet:
        return
    print("NX-18 acceptance summary:")
    slot = summary["slot"]  # type: ignore[assignment]
    print(
        f"  slot: {'ok' if slot['ok'] else 'fail'} "
        f"(samples={slot['samples']:.0f}, "
        f"p95={slot['p95_ms']:.1f} ms, p99={slot['p99_ms']:.1f} ms)"
    )
    da = summary["da_quorum"]  # type: ignore[assignment]
    print(f"  DA quorum: {'ok' if da['ok'] else 'fail'} (ratio={da['ratio']:.3f})")
    oracle = summary["oracle"]  # type: ignore[assignment]
    print(
        "  oracle: "
        f"{'ok' if oracle['ok'] else 'fail'} "
        f"(staleness={oracle['staleness_seconds']:.1f}s, "
        f"twap={oracle['twap_window_seconds']:.1f}s, "
        f"haircut={oracle['haircut_bps']:.1f} bps)"
    )
    buffer = summary["settlement_buffer"]  # type: ignore[assignment]
    print(
        f"  settlement buffer: {'ok' if buffer['ok'] else 'fail'} "
        f"(value={buffer['value']:.2f})"
    )
    if summary["failures"]:  # type: ignore[truthy-bool]
        print("Failures:")
        for failure in summary["failures"]:  # type: ignore[assignment]
            print(f"  - {failure}")


def main(argv: Optional[Iterable[str]] = None) -> int:
    args = parse_args(argv)
    thresholds = Thresholds(
        max_slot_p95=args.max_slot_p95,
        max_slot_p99=args.max_slot_p99,
        min_slot_samples=args.min_slot_samples,
        min_da_quorum=args.min_da_quorum,
        max_oracle_staleness=args.max_oracle_staleness,
        expected_oracle_twap=args.expected_oracle_twap,
        oracle_twap_tolerance=args.oracle_twap_tolerance,
        max_oracle_haircut_bps=args.max_oracle_haircut_bps,
        min_settlement_buffer=args.min_settlement_buffer,
    )

    try:
        summary, failures = evaluate_acceptance(args.metrics_path, thresholds)
    except (AcceptanceError, slot_mod.SlotDurationError) as exc:
        if args.json_out:
            args.json_out.write_text(
                json.dumps(
                    {
                        "ok": False,
                        "exit_code": 2,
                        "metrics_path": str(args.metrics_path),
                        "error": str(exc),
                    },
                    indent=2,
                ),
                encoding="utf-8",
            )
        print(f"[error] {exc}", file=sys.stderr)
        return 2

    emit_summary(summary, quiet=args.quiet)
    if args.json_out:
        args.json_out.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    if failures:
        return 1
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    sys.exit(main())
