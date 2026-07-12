#!/usr/bin/env python3
"""Verify Criterion evidence for the Kotodama Numeric V1 gas weights."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
from dataclasses import asdict, dataclass, fields
from pathlib import Path
from typing import Sequence


ADD_REPETITIONS = 50_000
SAFETY_MARGIN = 1.25
MAX_WEIGHT = 4.0
MIN_NUMERIC_SAMPLES = 30
REFERENCE_HOST_FORMAT = "iroha.numeric-v1.reference-host.v1"
REPORT_FORMAT = "iroha.numeric-v1.calibration.v2"
EXPECTED_HARDWARE_MODEL = "Mac13,2"
EXPECTED_CHIP = "Apple M1 Ultra"
EXPECTED_ARCHITECTURE = "arm64"
EXPECTED_RUNNER_OS = "macOS"
EXPECTED_RUNNER_ARCH = "ARM64"
EXPECTED_RUSTC_RELEASE = "1.93.1"
EXPECTED_RUSTC_HOST = "aarch64-apple-darwin"
EXPECTED_RUSTC_COMMIT_HASH = "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf"
EXPECTED_RUSTC_COMMIT_DATE = "2026-02-11"
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
_COMMIT = re.compile(r"[0-9a-f]{40}")
_RELEASE_TAG = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}")
_REPOSITORY = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+")


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


@dataclass(frozen=True)
class ReferenceHostMetadata:
    """Authenticated identity fields required for a release calibration."""

    format: str
    hardware_model: str
    chip: str
    architecture: str
    runner_os: str
    runner_arch: str
    runner_name: str
    rustc_release: str
    rustc_host: str
    rustc_commit_hash: str
    rustc_commit_date: str
    source_commit: str
    release_tag: str
    repository: str
    workflow_ref: str
    workflow_repository: str
    workflow_sha: str
    workflow_run_id: str
    workflow_run_attempt: str


def _load_json_object(path: Path, label: str) -> dict[str, object]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CalibrationError(f"invalid {label} {path}: {error}") from error
    if not isinstance(payload, dict):
        raise CalibrationError(f"{label} must be a JSON object: {path}")
    return payload


def _required_string(payload: dict[str, object], field: str) -> str:
    value = payload.get(field)
    if not isinstance(value, str) or not value:
        raise CalibrationError(
            f"reference-host metadata field {field!r} must be a non-empty string"
        )
    return value


def load_reference_host_metadata(
    path: Path,
    *,
    expected_commit: str,
    expected_release_tag: str,
    expected_repository: str,
) -> ReferenceHostMetadata:
    """Load and strictly validate the normative release-calibration host record."""

    if _COMMIT.fullmatch(expected_commit) is None:
        raise CalibrationError("expected commit must be a lowercase full Git SHA")
    if _RELEASE_TAG.fullmatch(expected_release_tag) is None:
        raise CalibrationError("expected release tag contains unsupported characters")
    if _REPOSITORY.fullmatch(expected_repository) is None:
        raise CalibrationError("expected repository must have owner/name form")

    payload = _load_json_object(path, "reference-host metadata")
    field_names = {field.name for field in fields(ReferenceHostMetadata)}
    unknown = sorted(set(payload) - field_names)
    missing = sorted(field_names - set(payload))
    if unknown or missing:
        details: list[str] = []
        if missing:
            details.append("missing " + ", ".join(missing))
        if unknown:
            details.append("unknown " + ", ".join(unknown))
        raise CalibrationError(
            "reference-host metadata has an invalid schema: " + "; ".join(details)
        )
    metadata = ReferenceHostMetadata(
        **{field: _required_string(payload, field) for field in sorted(field_names)}
    )

    required_values = {
        "format": REFERENCE_HOST_FORMAT,
        "hardware_model": EXPECTED_HARDWARE_MODEL,
        "chip": EXPECTED_CHIP,
        "architecture": EXPECTED_ARCHITECTURE,
        "runner_os": EXPECTED_RUNNER_OS,
        "runner_arch": EXPECTED_RUNNER_ARCH,
        "rustc_release": EXPECTED_RUSTC_RELEASE,
        "rustc_host": EXPECTED_RUSTC_HOST,
        "rustc_commit_hash": EXPECTED_RUSTC_COMMIT_HASH,
        "rustc_commit_date": EXPECTED_RUSTC_COMMIT_DATE,
        "source_commit": expected_commit,
        "release_tag": expected_release_tag,
        "repository": expected_repository,
        "workflow_repository": expected_repository,
        "workflow_sha": expected_commit,
    }
    for field, expected in required_values.items():
        actual = getattr(metadata, field)
        if actual != expected:
            raise CalibrationError(
                f"reference-host metadata {field} mismatch: "
                f"expected {expected!r}, found {actual!r}"
            )
    expected_workflow_prefix = (
        f"{expected_repository}/.github/workflows/numeric_v1_calibration.yml@"
    )
    if not metadata.workflow_ref.startswith(expected_workflow_prefix):
        raise CalibrationError(
            "reference-host metadata workflow_ref mismatch: expected prefix "
            f"{expected_workflow_prefix!r}, found {metadata.workflow_ref!r}"
        )
    for field in ("workflow_run_id", "workflow_run_attempt"):
        value = getattr(metadata, field)
        if not value.isascii() or not value.isdigit() or int(value) <= 0:
            raise CalibrationError(
                f"reference-host metadata {field} must be a positive decimal integer"
            )
    return metadata


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


def criterion_estimates_sha256(root: Path) -> str:
    """Hash every Criterion estimate consumed by the verifier, including its path."""

    if not root.is_dir():
        raise CalibrationError(f"Criterion root does not exist: {root}")
    paths = sorted(
        path
        for path in root.rglob("estimates.json")
        if path.parent.name == "new"
    )
    if not paths:
        raise CalibrationError("Criterion evidence contains no new estimates")
    digest = hashlib.sha256()
    digest.update(b"iroha.numeric-v1.criterion-estimates.v1\0")
    for path in paths:
        relative = path.relative_to(root).as_posix().encode("utf-8")
        try:
            encoded = path.read_bytes()
        except OSError as error:
            raise CalibrationError(
                f"cannot hash Criterion estimate {path}: {error}"
            ) from error
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
    return digest.hexdigest()


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
        # Only the entry benchmark executes a VM program. The direct bigint,
        # decimal, and codec cases contain no EMPTY_HARNESS work to subtract.
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
        "--host-metadata",
        required=True,
        type=Path,
        help="reference-host JSON captured by the release workflow",
    )
    parser.add_argument(
        "--expected-commit",
        required=True,
        help="lowercase full Git SHA being calibrated",
    )
    parser.add_argument(
        "--expected-release-tag",
        required=True,
        help="future release tag associated with the calibration evidence",
    )
    parser.add_argument(
        "--expected-repository",
        required=True,
        help="GitHub repository in owner/name form",
    )
    parser.add_argument(
        "--validate-host-only",
        action="store_true",
        help="validate reference-host metadata without reading Criterion evidence",
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
        metadata = load_reference_host_metadata(
            args.host_metadata,
            expected_commit=args.expected_commit,
            expected_release_tag=args.expected_release_tag,
            expected_repository=args.expected_repository,
        )
        if args.validate_host_only:
            print(
                "numeric V1 reference host accepted: "
                f"{metadata.chip} ({metadata.hardware_model}), "
                f"rustc {metadata.rustc_release}"
            )
            return 0
        estimates_sha256 = criterion_estimates_sha256(args.criterion_root)
        add_ns, samples = evaluate_calibration(args.criterion_root)
    except CalibrationError as error:
        if args.json_output is not None:
            args.json_output.parent.mkdir(parents=True, exist_ok=True)
            args.json_output.write_text(
                json.dumps(
                    {
                        "accepted": False,
                        "error": str(error),
                        "format": REPORT_FORMAT,
                    },
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
        print(f"numeric V1 calibration rejected: {error}", file=sys.stderr)
        return 1
    report = {
        "accepted": True,
        "format": REPORT_FORMAT,
        "add_repetitions": ADD_REPETITIONS,
        "criterion_estimates_sha256": estimates_sha256,
        "empty_harness_subtracted_from": [
            "scalar_add",
            "entry_control_pipeline",
        ],
        "scalar_add_ns": add_ns,
        "reference_host": asdict(metadata),
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
