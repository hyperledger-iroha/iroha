#!/usr/bin/env python3
"""Verify Criterion evidence for the Kotodama Numeric V1 gas weights."""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Sequence


ADD_REPETITIONS = 50_000
SAFETY_MARGIN = 1.25
MAX_WEIGHT = 4.0
MIN_NUMERIC_SAMPLES = 30
REQUIRED_NUMERIC_BENCHMARKS = frozenset(
    {
        "checked_add",
        "checked_multiply",
        "divide_remainder",
        "entry_control_pipeline",
        "wrapping_multiply",
        "decimal_div_round",
        "decimal_compare",
        "input_envelope_pipeline",
        "output_envelope_pipeline",
    }
)
_DENOMINATOR = re.compile(r"(?:work|gas)=(?P<value>[0-9]+)")


class CalibrationError(RuntimeError):
    """Raised when Criterion evidence is missing, malformed, or underpriced."""


@dataclass(frozen=True)
class CalibrationSample:
    """One numeric benchmark normalized to scalar-ADD work."""

    benchmark: str
    denominator: int
    denominator_kind: str
    median_ns: float
    raw_ratio: float
    safety_adjusted_ratio: float
    allowed_ratio: float
    safety_utilization: float


def _median_point(path: Path) -> float:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
        value = float(payload["median"]["point_estimate"])
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as error:
        raise CalibrationError(f"invalid Criterion estimate {path}: {error}") from error
    if not math.isfinite(value) or value <= 0:
        raise CalibrationError(f"Criterion median must be finite and positive: {path}")
    return value


def _new_estimates(root: Path) -> dict[str, float]:
    if not root.is_dir():
        raise CalibrationError(f"Criterion root does not exist: {root}")
    estimates: dict[str, float] = {}
    for path in sorted(root.rglob("estimates.json")):
        if path.parent.name != "new":
            continue
        relative = path.relative_to(root)
        benchmark = "/".join(relative.parts[:-2])
        if benchmark in estimates:
            raise CalibrationError(f"duplicate Criterion benchmark {benchmark}")
        estimates[benchmark] = _median_point(path)
    return estimates


def evaluate_calibration(root: Path) -> tuple[float, list[CalibrationSample]]:
    """Load Criterion output and return scalar-ADD latency plus numeric ratios."""

    estimates = _new_estimates(root)
    try:
        add_median = estimates["ivm-gas-cal/ADD"]
        empty_median = estimates["ivm-gas-cal/EMPTY_HARNESS"]
    except KeyError as error:
        raise CalibrationError(
            "Criterion evidence must contain ivm-gas-cal/ADD and EMPTY_HARNESS"
        ) from error
    adjusted_add = add_median - empty_median
    if adjusted_add <= 0:
        raise CalibrationError("ADD median must exceed EMPTY_HARNESS median")
    add_ns = adjusted_add / ADD_REPETITIONS

    samples: list[CalibrationSample] = []
    for benchmark, median_ns in estimates.items():
        if not benchmark.startswith("ivm-numeric-limb-cal/"):
            continue
        matched = _DENOMINATOR.search(benchmark)
        if matched is None:
            raise CalibrationError(
                f"numeric benchmark does not declare work or gas: {benchmark}"
            )
        denominator = int(matched.group("value"))
        if denominator <= 0:
            raise CalibrationError(f"numeric denominator must be positive: {benchmark}")
        denominator_kind = "work" if "work=" in matched.group(0) else "gas"
        if "/entry_control_pipeline/" in benchmark:
            median_ns -= empty_median
            if median_ns <= 0:
                raise CalibrationError(
                    "numeric entry median must exceed EMPTY_HARNESS median"
                )
        raw_ratio = (median_ns / denominator) / add_ns
        allowed_ratio = MAX_WEIGHT if denominator_kind == "work" else 1.0
        safety_adjusted_ratio = raw_ratio * SAFETY_MARGIN
        samples.append(
            CalibrationSample(
                benchmark=benchmark,
                denominator=denominator,
                denominator_kind=denominator_kind,
                median_ns=median_ns,
                raw_ratio=raw_ratio,
                safety_adjusted_ratio=safety_adjusted_ratio,
                allowed_ratio=allowed_ratio,
                safety_utilization=safety_adjusted_ratio / allowed_ratio,
            )
        )

    if len(samples) < MIN_NUMERIC_SAMPLES:
        raise CalibrationError(
            f"expected at least {MIN_NUMERIC_SAMPLES} numeric samples, "
            f"found {len(samples)}"
        )
    missing_benchmarks = sorted(
        label
        for label in REQUIRED_NUMERIC_BENCHMARKS
        if not any(f"/{label}/" in sample.benchmark for sample in samples)
    )
    if missing_benchmarks:
        raise CalibrationError(
            "Criterion evidence is missing required numeric families: "
            + ", ".join(missing_benchmarks)
        )
    if {sample.denominator_kind for sample in samples} != {"gas", "work"}:
        raise CalibrationError(
            "numeric evidence must contain both work and gas denominators"
        )
    samples.sort(key=lambda sample: sample.safety_utilization, reverse=True)
    overpriced = [
        sample
        for sample in samples
        if sample.safety_adjusted_ratio > sample.allowed_ratio
    ]
    if overpriced:
        worst = overpriced[0]
        raise CalibrationError(
            "a numeric consensus weight is insufficient after the 25% safety margin: "
            f"{worst.benchmark} requires {worst.safety_adjusted_ratio:.6f}, "
            f"allowed {worst.allowed_ratio:.6f}"
        )
    return add_ns, samples


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "criterion_root",
        nargs="?",
        type=Path,
        default=Path("target/criterion"),
        help="Criterion output root (default: target/criterion)",
    )
    parser.add_argument(
        "--json-output",
        type=Path,
        help="optional path for the machine-readable verification report",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Validate one calibration directory and print its worst-case ratio."""

    args = _parser().parse_args(argv)
    try:
        add_ns, samples = evaluate_calibration(args.criterion_root)
    except CalibrationError as error:
        print(f"numeric V1 calibration rejected: {error}", file=sys.stderr)
        return 1
    report = {
        "format": "iroha.numeric-v1.calibration.v1",
        "add_repetitions": ADD_REPETITIONS,
        "empty_harness_subtracted": True,
        "scalar_add_ns": add_ns,
        "safety_margin": SAFETY_MARGIN,
        "maximum_weight": MAX_WEIGHT,
        "worst": asdict(samples[0]),
        "samples": [asdict(sample) for sample in samples],
    }
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.json_output is not None:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(encoded, encoding="utf-8")
    print(
        "numeric V1 calibration accepted: "
        f"ADD={add_ns:.6f} ns, worst utilization={samples[0].safety_utilization:.6f} "
        f"({samples[0].benchmark})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
