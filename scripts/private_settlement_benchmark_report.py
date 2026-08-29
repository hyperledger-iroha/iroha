#!/usr/bin/env python3
"""Validate and summarize AtomicPrivateSettlementV1 benchmark JSONL evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import statistics
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


REPORT_VERSION = 1
REQUIRED_PARTICIPANTS = (2, 3, 4, 8, 16)
REQUIRED_PRIVATE_STAGES = (
    "proof_generation",
    "restricted_upload_availability",
    "auditor_response",
    "committee_verification",
    "prepare",
    "commit",
    "global_finality",
    "end_to_end",
)
PROFILES = ("private", "transparent_control")
RESOURCE_FIELDS = (
    "throughput_bundles_per_second",
    "cpu_seconds",
    "peak_rss_bytes",
    "network_bytes",
    "proof_bytes",
    "receipt_bytes",
    "storage_growth_bytes",
)
MIN_WARMUPS = 5
MIN_MEASURED = 30
MIN_SEEDS = 2


class EvidenceError(ValueError):
    """Raised when raw benchmark evidence is incomplete or malformed."""


@dataclass(frozen=True)
class Sample:
    """One validated benchmark run."""

    profile: str
    participants: int
    seed: int
    run: int
    warmup: bool
    stages_ms: Mapping[str, float]
    resources: Mapping[str, float]


def _finite_nonnegative(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise EvidenceError(f"{label} must be numeric")
    rendered = float(value)
    if not math.isfinite(rendered) or rendered < 0:
        raise EvidenceError(f"{label} must be finite and non-negative")
    return rendered


def parse_sample(record: Any, source: str) -> Sample:
    """Parse one strict versioned raw sample."""

    if not isinstance(record, dict) or record.get("version") != REPORT_VERSION:
        raise EvidenceError(f"{source}: sample version must be 1")
    expected = {
        "version",
        "profile",
        "participants",
        "seed",
        "run",
        "warmup",
        "stages_ms",
        *RESOURCE_FIELDS,
    }
    unknown = set(record) - expected
    missing = expected - set(record)
    if unknown or missing:
        raise EvidenceError(
            f"{source}: sample fields mismatch; missing={sorted(missing)} unknown={sorted(unknown)}"
        )
    profile = record["profile"]
    if profile not in PROFILES:
        raise EvidenceError(f"{source}: profile must be one of {PROFILES}")
    participants = record["participants"]
    seed = record["seed"]
    run = record["run"]
    warmup = record["warmup"]
    if participants not in REQUIRED_PARTICIPANTS:
        raise EvidenceError(f"{source}: unsupported real-network participant count")
    if isinstance(seed, bool) or not isinstance(seed, int) or seed < 0:
        raise EvidenceError(f"{source}: seed must be a non-negative integer")
    if isinstance(run, bool) or not isinstance(run, int) or run < 0:
        raise EvidenceError(f"{source}: run must be a non-negative integer")
    if not isinstance(warmup, bool):
        raise EvidenceError(f"{source}: warmup must be boolean")
    stages = record["stages_ms"]
    if not isinstance(stages, dict):
        raise EvidenceError(f"{source}: stages_ms must be an object")
    required_stages = REQUIRED_PRIVATE_STAGES if profile == "private" else ("global_finality", "end_to_end")
    if set(stages) != set(required_stages):
        raise EvidenceError(
            f"{source}: {profile} stages must be exactly {required_stages}"
        )
    normalized_stages = {
        stage: _finite_nonnegative(value, f"{source}: stages_ms.{stage}")
        for stage, value in stages.items()
    }
    resources = {
        field: _finite_nonnegative(record[field], f"{source}: {field}")
        for field in RESOURCE_FIELDS
    }
    return Sample(
        profile=profile,
        participants=participants,
        seed=seed,
        run=run,
        warmup=warmup,
        stages_ms=normalized_stages,
        resources=resources,
    )


def load_jsonl(paths: Sequence[Path]) -> list[Sample]:
    """Load raw JSONL files and reject duplicate run identities."""

    samples: list[Sample] = []
    identities: set[tuple[str, int, int, int, bool]] = set()
    for path in paths:
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            raise EvidenceError(f"cannot read {path}: {error}") from error
        for line_number, line in enumerate(lines, 1):
            if not line.strip():
                continue
            source = f"{path}:{line_number}"
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise EvidenceError(f"{source}: invalid JSON: {error}") from error
            sample = parse_sample(record, source)
            identity = (
                sample.profile,
                sample.participants,
                sample.seed,
                sample.run,
                sample.warmup,
            )
            if identity in identities:
                raise EvidenceError(f"{source}: duplicate sample identity {identity}")
            identities.add(identity)
            samples.append(sample)
    if not samples:
        raise EvidenceError("benchmark input is empty")
    return samples


def percentile(values: Sequence[float], quantile: float) -> float:
    """Return a deterministic linearly interpolated quantile."""

    if not values:
        raise EvidenceError("cannot summarize an empty sample")
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    position = (len(ordered) - 1) * quantile
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    weight = position - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def _bootstrap_interval(
    values: Sequence[float], quantile: float, *, seed: int, iterations: int
) -> tuple[float, float]:
    rng = random.Random(seed)
    estimates = []
    for _ in range(iterations):
        resample = [values[rng.randrange(len(values))] for _ in values]
        estimates.append(percentile(resample, quantile))
    return percentile(estimates, 0.025), percentile(estimates, 0.975)


def summarize_values(
    values: Sequence[float], *, binding: bytes, bootstrap_iterations: int
) -> dict[str, Any]:
    """Compute quantiles, deterministic bootstrap CIs, and MAD."""

    if bootstrap_iterations < 100:
        raise EvidenceError("bootstrap iterations must be at least 100")
    median = statistics.median(values)
    mad = statistics.median(abs(value - median) for value in values)
    summary: dict[str, Any] = {"count": len(values), "mad": mad}
    for label, quantile in (("p50", 0.50), ("p95", 0.95), ("p99", 0.99)):
        seed_bytes = hashlib.sha256(binding + label.encode("ascii")).digest()[:8]
        low, high = _bootstrap_interval(
            values,
            quantile,
            seed=int.from_bytes(seed_bytes, "big"),
            iterations=bootstrap_iterations,
        )
        summary[label] = percentile(values, quantile)
        summary[f"{label}_ci95"] = [low, high]
    return summary


def validate_matrix(samples: Sequence[Sample]) -> None:
    """Require every real N, both profiles, warmups, measured runs, and seeds."""

    buckets: dict[tuple[str, int], list[Sample]] = defaultdict(list)
    for sample in samples:
        buckets[(sample.profile, sample.participants)].append(sample)
    required = {(profile, participants) for profile in PROFILES for participants in REQUIRED_PARTICIPANTS}
    missing = required - set(buckets)
    if missing:
        raise EvidenceError(f"benchmark matrix is incomplete: {sorted(missing)}")
    for key in sorted(required):
        bucket = buckets[key]
        warmups = [sample for sample in bucket if sample.warmup]
        measured = [sample for sample in bucket if not sample.warmup]
        seeds = {sample.seed for sample in measured}
        if len(warmups) < MIN_WARMUPS:
            raise EvidenceError(f"{key}: requires at least {MIN_WARMUPS} warmups")
        if len(measured) < MIN_MEASURED:
            raise EvidenceError(f"{key}: requires at least {MIN_MEASURED} measured runs")
        if len(seeds) < MIN_SEEDS:
            raise EvidenceError(f"{key}: requires measured runs across multiple seeds")


def build_report(samples: Sequence[Sample], bootstrap_iterations: int) -> dict[str, Any]:
    """Build the signed-baseline-compatible statistical report."""

    validate_matrix(samples)
    report: dict[str, Any] = {
        "version": REPORT_VERSION,
        "requirements": {
            "participants": list(REQUIRED_PARTICIPANTS),
            "minimum_warmups": MIN_WARMUPS,
            "minimum_measured": MIN_MEASURED,
            "minimum_seeds": MIN_SEEDS,
            "bootstrap_iterations": bootstrap_iterations,
        },
        "profiles": {},
    }
    for profile in PROFILES:
        profile_report: dict[str, Any] = {}
        for participants in REQUIRED_PARTICIPANTS:
            bucket = [
                sample
                for sample in samples
                if sample.profile == profile
                and sample.participants == participants
                and not sample.warmup
            ]
            binding_prefix = f"{profile}:{participants}:".encode("ascii")
            stage_names = REQUIRED_PRIVATE_STAGES if profile == "private" else ("global_finality", "end_to_end")
            stages = {
                stage: summarize_values(
                    [sample.stages_ms[stage] for sample in bucket],
                    binding=binding_prefix + b"stage:" + stage.encode("ascii"),
                    bootstrap_iterations=bootstrap_iterations,
                )
                for stage in stage_names
            }
            resources = {
                field: summarize_values(
                    [sample.resources[field] for sample in bucket],
                    binding=binding_prefix + b"resource:" + field.encode("ascii"),
                    bootstrap_iterations=bootstrap_iterations,
                )
                for field in RESOURCE_FIELDS
            }
            profile_report[str(participants)] = {
                "measured_runs": len(bucket),
                "seeds": sorted({sample.seed for sample in bucket}),
                "stages_ms": stages,
                "resources": resources,
            }
        report["profiles"][profile] = profile_report
    return report


def compare_baseline(candidate: Mapping[str, Any], baseline: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Apply the post-initial-release p95/p99 regression policy."""

    regressions: list[dict[str, Any]] = []
    for profile in PROFILES:
        for participants in REQUIRED_PARTICIPANTS:
            participant = str(participants)
            candidate_stages = candidate["profiles"][profile][participant]["stages_ms"]
            baseline_stages = baseline["profiles"][profile][participant]["stages_ms"]
            if set(candidate_stages) != set(baseline_stages):
                raise EvidenceError(
                    f"baseline stage set differs for {profile}/{participant}"
                )
            for stage in sorted(candidate_stages):
                current = candidate_stages[stage]
                previous = baseline_stages[stage]
                p95_limit = previous["p95"] + max(
                    previous["p95"] * 0.10, previous["mad"] * 3.0
                )
                p99_limit = previous["p99"] * 1.20
                if current["p95"] > p95_limit:
                    regressions.append(
                        {
                            "profile": profile,
                            "participants": participants,
                            "stage": stage,
                            "quantile": "p95",
                            "actual": current["p95"],
                            "limit": p95_limit,
                        }
                    )
                if current["p99"] > p99_limit:
                    regressions.append(
                        {
                            "profile": profile,
                            "participants": participants,
                            "stage": stage,
                            "quantile": "p99",
                            "actual": current["p99"],
                            "limit": p99_limit,
                        }
                    )
    return regressions


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", action="append", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--bootstrap-iterations", type=int, default=2_000)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        report = build_report(load_jsonl(args.input), args.bootstrap_iterations)
        regressions: list[dict[str, Any]] = []
        if args.baseline is not None:
            try:
                baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
            except (OSError, UnicodeError, json.JSONDecodeError) as error:
                raise EvidenceError(f"cannot read baseline: {error}") from error
            regressions = compare_baseline(report, baseline)
        report["regressions"] = regressions
        report["passed"] = not regressions
        args.output.write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    except (EvidenceError, OSError, KeyError, TypeError) as error:
        print(f"private-settlement benchmark evidence error: {error}", file=sys.stderr)
        return 2
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
