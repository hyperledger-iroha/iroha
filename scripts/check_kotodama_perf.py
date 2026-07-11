#!/usr/bin/env python3
"""Enforce the Kotodama V1 Criterion regression budget.

The gate compares workloads present before the reset against Criterion's real
``base`` samples or an explicit checked-in baseline. New V1 List, Amount, and
typed-query workloads remain mandatory current evidence and enforce their
cross-workload invariants without fabricating a comparison-base sample. A
comparable benchmark fails when its median is more than five percent slower;
missing or malformed samples fail closed.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence


SCHEMA = "kotodama-perf-baseline-v1"
MAX_REGRESSION = 0.05
LIST_SUGAR_MAX_SLOWDOWN = 0.0
LIST_SUGAR_BENCHMARK = "kotodama_list_comprehension_runtime_64"
LIST_MANUAL_BENCHMARK = "kotodama_list_manual_runtime_64"
REPRESENTATIVE_BENCHMARKS = (
    "kotodama_phase_parse",
    "kotodama_phase_semantic",
    "kotodama_phase_ir_lower",
    "kotodama_phase_codegen_end_to_end",
    "kotodama_list_semantic_64",
    "kotodama_list_lower_64",
    "kotodama_list_get_64",
    "kotodama_list_try_set_64",
    "kotodama_list_try_push_pop_64",
    "kotodama_list_contains_64",
    LIST_SUGAR_BENCHMARK,
    LIST_MANUAL_BENCHMARK,
    "kotodama_amount_add",
    "kotodama_amount_sub",
    "kotodama_amount_mul",
    "kotodama_amount_div_exact",
    "kotodama_amount_div_round_floor",
    "kotodama_amount_div_round_ceil",
    "kotodama_amount_div_round_nearest_even",
    "typed_core_query_accounts_page_64",
    "kotodama_runtime_cold_add",
    "kotodama_runtime_warm_add",
    "kotodama_core_runtime_warm_add",
)

# These workloads predate the V1 data-processing reset and therefore have a
# real comparison sample on the pull-request base revision. Newly introduced
# List, Amount, and typed-query benchmarks are still mandatory current
# evidence, but must never manufacture a "base" by comparing the candidate
# against itself.
REGRESSION_BENCHMARKS = (
    "kotodama_phase_parse",
    "kotodama_phase_semantic",
    "kotodama_phase_ir_lower",
    "kotodama_phase_codegen_end_to_end",
    "kotodama_runtime_cold_add",
    "kotodama_runtime_warm_add",
    "kotodama_core_runtime_warm_add",
)


class GateError(RuntimeError):
    """Raised when performance evidence is absent or invalid."""


@dataclass(frozen=True)
class Comparison:
    """One benchmark comparison, expressed in Criterion nanoseconds."""

    name: str
    baseline_ns: float
    measured_ns: float

    @property
    def change(self) -> float:
        return self.measured_ns / self.baseline_ns - 1.0


def _positive_finite(value: object, context: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise GateError(f"{context} must be a number")
    number = float(value)
    if not math.isfinite(number) or number <= 0.0:
        raise GateError(f"{context} must be finite and positive")
    return number


def read_criterion_median(path: Path) -> float:
    """Read ``median.point_estimate`` from one Criterion estimates file."""

    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise GateError(f"failed to read Criterion sample {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise GateError(f"invalid Criterion JSON {path}: {error}") from error
    try:
        value = payload["median"]["point_estimate"]
    except (KeyError, TypeError) as error:
        raise GateError(
            f"Criterion sample {path} is missing median.point_estimate"
        ) from error
    return _positive_finite(value, f"Criterion median in {path}")


def read_current_samples(
    criterion_dir: Path, benchmarks: Sequence[str]
) -> dict[str, float]:
    """Read the current ``new`` medians for the selected benchmarks."""

    return {
        name: read_criterion_median(
            criterion_dir / name / "new" / "estimates.json"
        )
        for name in benchmarks
    }


def read_criterion_base(
    criterion_dir: Path, benchmarks: Sequence[str]
) -> dict[str, float]:
    """Read Criterion's previous-run ``base`` medians."""

    return {
        name: read_criterion_median(
            criterion_dir / name / "base" / "estimates.json"
        )
        for name in benchmarks
    }


def read_baseline(path: Path, benchmarks: Sequence[str]) -> dict[str, float]:
    """Read and strictly validate a portable Kotodama baseline document."""

    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise GateError(f"failed to read baseline {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise GateError(f"invalid baseline JSON {path}: {error}") from error
    if not isinstance(payload, dict) or payload.get("schema") != SCHEMA:
        raise GateError(f"baseline {path} must declare schema {SCHEMA!r}")
    if payload.get("unit") != "ns":
        raise GateError(f"baseline {path} must declare nanosecond units")
    values = payload.get("benchmarks")
    if not isinstance(values, dict):
        raise GateError(f"baseline {path} is missing its benchmarks object")
    expected = set(benchmarks)
    actual = set(values)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        details = []
        if missing:
            details.append("missing: " + ", ".join(missing))
        if extra:
            details.append("unexpected: " + ", ".join(extra))
        raise GateError(
            f"baseline {path} benchmark coverage mismatch ("
            + "; ".join(details)
            + ")"
        )
    return {
        name: _positive_finite(values[name], f"baseline value for {name}")
        for name in benchmarks
    }


def write_baseline(path: Path, samples: Mapping[str, float]) -> None:
    """Atomically write a canonical baseline captured from current samples."""

    payload = {
        "schema": SCHEMA,
        "unit": "ns",
        "benchmarks": {name: samples[name] for name in sorted(samples)},
    }
    encoded = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(encoded, encoding="utf-8")
    temporary.replace(path)


def compare_samples(
    baseline: Mapping[str, float], measured: Mapping[str, float]
) -> list[Comparison]:
    """Build deterministic comparisons and reject mismatched coverage."""

    if set(baseline) != set(measured):
        missing = sorted(set(baseline) - set(measured))
        extra = sorted(set(measured) - set(baseline))
        details = []
        if missing:
            details.append("missing measured: " + ", ".join(missing))
        if extra:
            details.append("unexpected measured: " + ", ".join(extra))
        raise GateError("benchmark coverage mismatch (" + "; ".join(details) + ")")
    return [
        Comparison(name, baseline[name], measured[name]) for name in sorted(baseline)
    ]


def enforce(comparisons: Sequence[Comparison], threshold: float) -> None:
    """Raise when any benchmark exceeds the configured slowdown budget."""

    if not math.isfinite(threshold) or threshold < 0.0 or threshold > MAX_REGRESSION:
        raise GateError(
            f"threshold must be between 0 and {MAX_REGRESSION:.2f}; "
            "the V1 gate cannot be loosened above five percent"
        )
    # Decimal Criterion values and binary floating point can represent an
    # exact 5% boundary as 5.000000000000004%. Keep the policy inclusive while
    # allowing only machine-rounding noise, not a measurable relaxation.
    failures = [row for row in comparisons if row.change - threshold > 1e-12]
    if failures:
        details = "\n".join(
            f"  {row.name}: {row.change * 100.0:+.2f}% "
            f"({row.baseline_ns:.0f} ns -> {row.measured_ns:.0f} ns)"
            for row in failures
        )
        raise GateError(
            f"Kotodama performance regression exceeds {threshold * 100.0:.2f}%:\n"
            + details
        )


def enforce_list_sugar(samples: Mapping[str, float]) -> None:
    """Require comprehension sugar to be no slower than the manual loop."""
    try:
        sugar = _positive_finite(
            samples[LIST_SUGAR_BENCHMARK], "List comprehension runtime median"
        )
        manual = _positive_finite(
            samples[LIST_MANUAL_BENCHMARK], "manual List runtime median"
        )
    except KeyError as error:
        raise GateError(f"missing List runtime comparison sample {error.args[0]}") from error
    change = sugar / manual - 1.0
    if change - LIST_SUGAR_MAX_SLOWDOWN > 1e-12:
        raise GateError(
            "Kotodama List comprehension sugar exceeds the manual-loop "
            f"baseline by {change * 100.0:+.2f}% "
            f"({manual:.0f} ns -> {sugar:.0f} ns; "
            "the V1 sugar path must be no slower)"
        )


def render(comparisons: Sequence[Comparison]) -> str:
    """Render a compact, deterministic comparison table."""

    rows = ["benchmark | baseline ns | measured ns | change", "---|---:|---:|---:"]
    rows.extend(
        f"{row.name} | {row.baseline_ns:.0f} | {row.measured_ns:.0f} | "
        f"{row.change * 100.0:+.2f}%"
        for row in comparisons
    )
    return "\n".join(rows)


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--criterion-dir", type=Path, default=Path("target/criterion")
    )
    baseline_mode = parser.add_mutually_exclusive_group()
    baseline_mode.add_argument(
        "--baseline",
        type=Path,
        help="checked-in baseline JSON; defaults to Criterion base samples",
    )
    baseline_mode.add_argument(
        "--write-baseline",
        type=Path,
        help="capture current samples instead of comparing them",
    )
    parser.add_argument("--threshold", type=float, default=MAX_REGRESSION)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        current = read_current_samples(
            args.criterion_dir, REPRESENTATIVE_BENCHMARKS
        )
        enforce_list_sugar(current)
        if args.write_baseline is not None:
            write_baseline(
                args.write_baseline,
                {name: current[name] for name in REGRESSION_BENCHMARKS},
            )
            print(f"wrote Kotodama performance baseline to {args.write_baseline}")
            return 0
        baseline = (
            read_baseline(args.baseline, REGRESSION_BENCHMARKS)
            if args.baseline is not None
            else read_criterion_base(args.criterion_dir, REGRESSION_BENCHMARKS)
        )
        comparisons = compare_samples(
            baseline,
            {name: current[name] for name in REGRESSION_BENCHMARKS},
        )
        print(render(comparisons))
        enforce(comparisons, args.threshold)
    except GateError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        "Comparable Kotodama medians are within the 5% V1 budget, List sugar is "
        "no slower than its manual-loop baseline, and all required V1 current "
        "samples are present."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
