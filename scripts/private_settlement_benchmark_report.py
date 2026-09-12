#!/usr/bin/env python3
"""Summarize registered AtomicPrivateSettlementV1 benchmark evidence.

Python 3.11+ and the sibling canonical accounting/runner modules are required.
CLI callers must provide a closed retained scope and its successful JSONL rows;
no source execution, network operation, or missing-attempt inference is performed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import re
import statistics
import sys
from collections import defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import private_settlement_attempt_accounting as attempt_accounting

REPORT_VERSION = 1
PROTOCOL = "AtomicPrivateSettlementV1"
_GIT_COMMIT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
REQUIRED_PARTICIPANTS = (2, 3, 4, 8, 16)
REQUIRED_PRIVATE_STAGES = (
    "proof_generation",
    "restricted_upload_availability",
    "auditor_response",
    "committee_verification",
    "prepare",
    "prepare_registration",
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
class Measurement:
    """Validated benchmark metrics before registered attempt identity is attached."""

    commit: str
    hardware_sha256: str
    hardware_profile_sha256: str
    configuration_sha256: str
    profile: str
    participants: int
    seed: int
    run: int
    warmup: bool
    stages_ms: Mapping[str, float]
    resources: Mapping[str, float]


@dataclass(frozen=True)
class Sample(Measurement):
    """One measured success joined to its immutable registered attempt."""

    attempt_id: str


def parse_sample(record: Any, source: str) -> Sample:
    """Require registered attempt identity on every retained statistical sample."""

    if not isinstance(record, dict) or not isinstance(record.get("attempt_id"), str) or re.fullmatch(
            r"[0-9a-f]{64}", record["attempt_id"]) is None:
        raise EvidenceError(f"{source}: sample must bind its registered attempt")
    measurement = parse_measurement({key: value for key, value in record.items() if key != "attempt_id"}, source)
    return Sample(**vars(measurement), attempt_id=record["attempt_id"])


def _finite_nonnegative(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise EvidenceError(f"{label} must be numeric")
    rendered = float(value)
    if not math.isfinite(rendered) or rendered < 0:
        raise EvidenceError(f"{label} must be finite and non-negative")
    return rendered


def parse_measurement(record: Any, source: str) -> Measurement:
    """Validate metrics independently of the outer attempt-registration transport."""

    if not isinstance(record, dict) or type(record.get("version")) is not int or record["version"] != REPORT_VERSION:
        raise EvidenceError(f"{source}: sample version must be 1")
    expected = {
        "version",
        "protocol",
        "commit",
        "hardware_sha256",
        "hardware_profile_sha256",
        "configuration_sha256",
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
    if record["protocol"] != PROTOCOL:
        raise EvidenceError(f"{source}: sample protocol must be {PROTOCOL}")
    commit = record["commit"]
    if not isinstance(commit, str) or _GIT_COMMIT.fullmatch(commit) is None:
        raise EvidenceError(f"{source}: sample commit must be a full Git object id")
    hardware_sha256 = record["hardware_sha256"]
    hardware_profile_sha256 = record["hardware_profile_sha256"]
    configuration_sha256 = record["configuration_sha256"]
    if (
        not isinstance(hardware_sha256, str)
        or re.fullmatch(r"[0-9a-f]{64}", hardware_sha256) is None
        or not isinstance(hardware_profile_sha256, str)
        or re.fullmatch(r"[0-9a-f]{64}", hardware_profile_sha256) is None
        or not isinstance(configuration_sha256, str)
        or re.fullmatch(r"[0-9a-f]{64}", configuration_sha256) is None
    ):
        raise EvidenceError(f"{source}: sample environment digests must be SHA-256")
    profile = record["profile"]
    if profile not in PROFILES:
        raise EvidenceError(f"{source}: profile must be one of {PROFILES}")
    participants = record["participants"]
    seed = record["seed"]
    run = record["run"]
    warmup = record["warmup"]
    if type(participants) is not int or participants not in REQUIRED_PARTICIPANTS:
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
    required_stages = (
        REQUIRED_PRIVATE_STAGES
        if profile == "private"
        else ("global_finality", "end_to_end")
    )
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
    return Measurement(
        commit=commit,
        hardware_sha256=hardware_sha256,
        hardware_profile_sha256=hardware_profile_sha256,
        configuration_sha256=configuration_sha256,
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
    identities: set[str] = set()
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
                record = attempt_accounting._document(line.encode("utf-8"), source)
            except attempt_accounting.AccountingError as error:
                raise EvidenceError(f"{source}: invalid JSON: {error}") from error
            sample = parse_sample(record, source)
            identity = sample.attempt_id
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



# TODO: Connect these arithmetic helpers only after the canonical verifier joins
# real session processes, warmups and attempt terminals. Replace the individual
# resampling callers then; never infer persistent network sessions from seeds.
def _session_statistics_inputs(groups, binding, iterations, *, paired):
    """Canonicalize explicit session/attempt identities for arithmetic only.

    The caller must authenticate the session process and warmup/measurement
    joins. A seed number is not evidence that observations share one network.
    Empty started-session groups are retained; no failure is converted to a
    numeric observation. These helpers do not authenticate evidence.
    """
    if type(binding) is not bytes or len(binding) != 32:
        raise EvidenceError("session statistics require a 32-byte input binding")
    if type(iterations) is not int or iterations < 100:
        raise EvidenceError("bootstrap iterations must be an integer of at least 100")
    if not isinstance(groups, Mapping) or len(groups) < MIN_SEEDS:
        raise EvidenceError("session statistics require multiple explicit session groups")
    digest = re.compile(r"[0-9a-f]{64}")
    if any(type(key) is not str or digest.fullmatch(key) is None for key in groups):
        raise EvidenceError("session group identities must be SHA-256 digests")
    seen = set()
    canonical = []
    for session_id in sorted(groups):
        observations = groups[session_id]
        if not isinstance(observations, Mapping):
            raise EvidenceError("session observations must bind individual attempt identities")
        if any(type(key) is not str or digest.fullmatch(key) is None for key in observations):
            raise EvidenceError("observation identities must be SHA-256 digests")
        values = []
        for observation_id in sorted(observations):
            if observation_id in seen:
                raise EvidenceError("one observation occurs in multiple session groups")
            seen.add(observation_id)
            value = observations[observation_id]
            if paired:
                if not isinstance(value, (tuple, list)) or len(value) != 2:
                    raise EvidenceError("paired observations require private and control values")
                parts = value
            else:
                parts = (value,)
            try:
                converted = tuple(_finite_nonnegative(part, "session observation") for part in parts)
            except OverflowError as error:
                raise EvidenceError("session observation exceeds finite numeric range") from error
            values.append(converted if paired else converted[0])
        canonical.append((session_id, tuple(values)))
    return tuple(canonical)


def _bootstrap_session_draws(session_count, binding, iterations):
    """Draw complete sessions with replacement using the pinned Python runtime.

    All observations in a selected session are retained, including when the
    same session is selected twice. One draw schedule serves all quantiles.
    The full caller binding and a method domain determine the random stream.
    """
    seed = hashlib.sha256(b"iroha:benchmark:session-bootstrap:v1\0" + binding).digest()
    rng = random.Random(int.from_bytes(seed, "big"))
    for _ in range(iterations):
        yield tuple(rng.randrange(session_count) for _ in range(session_count))


def _session_quantile_summary(values, estimates_by_quantile, undefined):
    """Retain undefined replicates instead of dropping or redrawing them."""
    summary = {}
    for label, quantile in (("p50", 0.50), ("p95", 0.95), ("p99", 0.99)):
        summary[label] = percentile(values, quantile) if values else None
        estimates = estimates_by_quantile[label]
        summary[f"{label}_undefined_replicates"] = undefined
        summary[f"{label}_ci95"] = None if undefined else [
            percentile(estimates, 0.025), percentile(estimates, 0.975)]
    return summary


def summarize_session_values(groups, *, binding: bytes, bootstrap_iterations: int) -> dict[str, Any]:
    """Summarize measured successes by resampling their complete network sessions.

    ``groups`` maps authenticated session digests to attempt-digest/value maps.
    Every started measurement session belongs in the input, including an empty
    group when it has no accepted measurements. The caller separately retains
    failures, deadlines, warmup failures and not-started jobs in full accounting.
    The estimand is the pooled measured-success distribution, so a larger group
    contributes more observations; it is not an equal-session quantile average.
    """
    canonical = _session_statistics_inputs(groups, binding, bootstrap_iterations, paired=False)
    clusters = [values for _, values in canonical]
    values = [value for cluster in clusters for value in cluster]
    estimates = {label: [] for label in ("p50", "p95", "p99")}
    undefined = 0
    for draw in _bootstrap_session_draws(len(clusters), binding, bootstrap_iterations):
        # Keep only this draw's observations and its three scalar estimates.
        sample = [value for index in draw for value in clusters[index]]
        if not sample:
            undefined += 1
            continue
        for label, quantile in (("p50", 0.50), ("p95", 0.95), ("p99", 0.99)):
            estimates[label].append(percentile(sample, quantile))
    median = percentile(values, 0.5) if values else None
    return {
        "resampling_unit": "network_session",
        "estimand": "pooled_measured_success_quantiles",
        "session_count": len(clusters),
        "nonempty_session_count": sum(bool(cluster) for cluster in clusters),
        "observations_per_session": {key: len(values) for key, values in canonical},
        "count": len(values),
        "bootstrap_iterations": bootstrap_iterations,
        "mad": percentile([abs(value - median) for value in values], 0.5) if values else None,
        "unconditional_quantiles": "not_estimated",
        **_session_quantile_summary(values, estimates, undefined),
    }


def _paired_session_quantiles(sample):
    """Calculate paired marginal quantile differences and finite ratios."""
    result = {}
    for label, quantile in (("p50", 0.50), ("p95", 0.95), ("p99", 0.99)):
        if not sample:
            result[label] = (None, None)
            continue
        private = percentile([pair[0] for pair in sample], quantile)
        control = percentile([pair[1] for pair in sample], quantile)
        ratio = private / control if control else None
        result[label] = (private - control, ratio if ratio is not None and math.isfinite(ratio) else None)
    return result


def summarize_paired_session_values(groups, *, binding: bytes, bootstrap_iterations: int) -> dict[str, Any]:
    """Resample matched session pairs together for marginal quantile comparisons.

    Each group binds one independent private/control network-session pair with
    the same seed and profile-independent workload. Observation keys bind exact
    matched measurement attempts; each value is (private, transparent_control).
    The caller must prove these joins and retain all unmatched/failed attempts
    outside this conditional paired-success statistic. Empty pairs stay in the
    bootstrap frame. Ratios with zero denominators or overflow are undefined.
    """
    canonical = _session_statistics_inputs(groups, binding, bootstrap_iterations, paired=True)
    clusters = [values for _, values in canonical]
    values = [value for cluster in clusters for value in cluster]
    points = _paired_session_quantiles(values)
    estimates = {metric: {label: [] for label in points} for metric in ("difference", "ratio")}
    undefined = {metric: {label: 0 for label in points} for metric in estimates}
    for draw in _bootstrap_session_draws(len(clusters), binding, bootstrap_iterations):
        sample = [value for index in draw for value in clusters[index]]
        for label, comparison in _paired_session_quantiles(sample).items():
            for index, metric in enumerate(("difference", "ratio")):
                value = comparison[index]
                if value is None:
                    undefined[metric][label] += 1
                else:
                    estimates[metric][label].append(value)
    result = {
        "resampling_unit": "matched_network_session_pair",
        "estimand": "marginal_quantile_comparison_of_matched_measured_successes",
        "session_pair_count": len(clusters),
        "nonempty_session_pair_count": sum(bool(cluster) for cluster in clusters),
        "observations_per_session_pair": {key: len(values) for key, values in canonical},
        "paired_observation_count": len(values),
        "bootstrap_iterations": bootstrap_iterations,
        "difference": {}, "ratio": {},
        "unconditional_quantiles": "not_estimated",
    }
    for label, point in points.items():
        for index, metric in enumerate(("difference", "ratio")):
            samples = estimates[metric][label]
            result[metric][label] = point[index]
            result[metric][f"{label}_undefined_replicates"] = undefined[metric][label]
            result[metric][f"{label}_ci95"] = None if undefined[metric][label] else [
                percentile(samples, 0.025), percentile(samples, 0.975)]
    return result


def validate_matrix(samples: Sequence[Sample]) -> None:
    """Require every real N, both profiles, warmups, measured runs, and seeds."""

    buckets: dict[tuple[str, int], list[Sample]] = defaultdict(list)
    seen = set()
    for sample in samples:
        if not isinstance(sample.attempt_id, str) or re.fullmatch(r"[0-9a-f]{64}", sample.attempt_id) is None or sample.attempt_id in seen:
            raise EvidenceError("sample attempt identity is malformed or duplicated")
        seen.add(sample.attempt_id)
        buckets[(sample.profile, sample.participants)].append(sample)
    required = {
        (profile, participants)
        for profile in PROFILES
        for participants in REQUIRED_PARTICIPANTS
    }
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
            raise EvidenceError(
                f"{key}: requires at least {MIN_MEASURED} measured runs"
            )
        if len(seeds) < MIN_SEEDS:
            raise EvidenceError(f"{key}: requires measured runs across multiple seeds")


def build_report(
    samples: Sequence[Sample], bootstrap_iterations: int, *, scope_raw: bytes,
    campaigns: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Recompute registered counts and statistics from exactly joined successes."""

    if type(bootstrap_iterations) is not int or bootstrap_iterations < 100:
        raise EvidenceError("bootstrap iterations must be an integer of at least 100")
    validate_matrix(samples)
    commits = {sample.commit for sample in samples}
    if len(commits) != 1:
        raise EvidenceError("benchmark evidence must use one exact source commit")
    hardware_digests = {sample.hardware_sha256 for sample in samples}
    if len(hardware_digests) != 1:
        raise EvidenceError(
            "benchmark evidence must use one pinned hardware description"
        )
    hardware_profile_digests = {
        sample.hardware_profile_sha256 for sample in samples
    }
    if len(hardware_profile_digests) != 1:
        raise EvidenceError("benchmark evidence must use one pinned hardware profile")
    configuration_digests: dict[int, str] = {}
    for participants in REQUIRED_PARTICIPANTS:
        digests = {
            sample.configuration_sha256
            for sample in samples
            if sample.participants == participants
        }
        if len(digests) != 1:
            raise EvidenceError(
                f"benchmark N={participants} must use one pinned configuration"
            )
        configuration_digests[participants] = next(iter(digests))
    try:
        successful_rows = []
        for sample in samples:
            document = {
                "version": REPORT_VERSION, "protocol": PROTOCOL,
                **{key: getattr(sample, key) for key in (
                    "attempt_id", "commit", "hardware_sha256", "hardware_profile_sha256",
                    "configuration_sha256", "profile", "participants", "seed", "run", "warmup")},
                "stages_ms": dict(sample.stages_ms), **dict(sample.resources),
            }
            normalized = parse_sample(document, "registered statistical sample")
            document["stages_ms"] = dict(normalized.stages_ms)
            document.update(normalized.resources)
            successful_rows.append(attempt_accounting.accounting_canonical_bytes(document))
        accounting = attempt_accounting.reduce_registered_scope(scope_raw, campaigns, successful_rows)
        if accounting["accounting_complete"] is not True:
            raise EvidenceError("registered benchmark accounting is incomplete")
        for campaign in campaigns:
            plan = attempt_accounting._document(campaign["plan"], "report campaign plan")
            if (type(plan["requirements"].get("bootstrap_iterations")) is not int
                    or plan["requirements"]["bootstrap_iterations"] != bootstrap_iterations):
                raise EvidenceError("report bootstrap policy differs from its registered plan")
            for job in plan["jobs"]:
                if job["kind"] == "benchmark" and job["configuration_sha256"] != configuration_digests[job["participants"]]:
                    raise EvidenceError("registered campaign used a different pinned configuration")
    except attempt_accounting.AccountingError as error:
        raise EvidenceError(f"registered benchmark accounting is invalid: {error}") from error
    report: dict[str, Any] = {
        "version": REPORT_VERSION,
        "protocol": PROTOCOL,
        "commit": next(iter(commits)),
        "environment": {
            "hardware_sha256": next(iter(hardware_digests)),
            "hardware_profile_sha256": next(iter(hardware_profile_digests)),
            "configuration_sha256_by_participants": {
                str(participants): configuration_digests[participants]
                for participants in REQUIRED_PARTICIPANTS
            },
        },
        "requirements": {
            "participants": list(REQUIRED_PARTICIPANTS),
            "minimum_warmups": MIN_WARMUPS,
            "minimum_measured": MIN_MEASURED,
            "minimum_seeds": MIN_SEEDS,
            "bootstrap_iterations": bootstrap_iterations,
        },
        "profiles": {},
        "accounting": accounting,
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
            stage_names = (
                REQUIRED_PRIVATE_STAGES
                if profile == "private"
                else ("global_finality", "end_to_end")
            )
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
    validate_report_accounting(report)
    return report


def validate_report_accounting(report: Mapping[str, Any], label: str = "benchmark report") -> None:
    """Check a public projection's exact shape and partitions, without qualification.

    Original records are required by ``build_report`` and the release verifier.
    This structural check alone never authenticates a baseline or its evidence.
    """

    try:
        if (type(report.get("version")) is not int or report["version"] != REPORT_VERSION
                or report.get("protocol") != PROTOCOL or not isinstance(report.get("commit"), str)
                or _GIT_COMMIT.fullmatch(report["commit"]) is None):
            raise EvidenceError(f"{label} has an invalid report header")
        accounting = attempt_accounting.exact_fields(report.get("accounting"), frozenset({
            "version", "protocol", "scope_id", "scope", "previous_scope_sha256", "timeout_scope",
            "deadline_policy", "counts", "cohorts", "campaigns", "rows", "accounting_complete",
        }), label + ".accounting")
        attempt_accounting._header(accounting, label + ".accounting")
        attempt_accounting._digest(accounting["scope_id"], "scope identity")
        scope = attempt_accounting._binding(accounting["scope"], "scope binding")
        attempt_accounting.validate_deadline_policy(accounting["deadline_policy"])
        if (accounting["previous_scope_sha256"] is not None
                or accounting["timeout_scope"] != attempt_accounting.TIMEOUT_SCOPE
                or accounting["accounting_complete"] is not True):
            raise EvidenceError(f"{label} accounting is not complete first-release accounting")
        rows, campaigns = accounting["rows"], accounting["campaigns"]
        if not isinstance(rows, list) or not rows or not isinstance(campaigns, list) or not campaigns:
            raise EvidenceError(f"{label} accounting inventory is empty or malformed")
        ids, campaign_rows, seen = [], {}, set()
        for campaign in campaigns:
            attempt_accounting.exact_fields(campaign, frozenset({"campaign_id", "plan_sha256", "counts"}), "campaign projection")
            name = attempt_accounting._campaign_id(campaign["campaign_id"])
            attempt_accounting._digest(campaign["plan_sha256"], "campaign plan")
            ids.append(name)
            campaign_rows[name] = []
        if ids != sorted(set(ids)):
            raise EvidenceError(f"{label} campaign projection is not unique and ordered")
        reasons = {
            "not_started": {"closed_without_durable_start"},
            "succeeded": {"validated_measurement"},
            "failed": {"harness_spawn_failed", "rust_failed", "validator_build_failed",
                       "validator_build_spawn_failed", "benchmark_process_spawn_failed", "publication_failed"},
            "timed_out": {"outer_deadline", "rust_timed_out"},
        }
        plans = {campaign["campaign_id"]: campaign["plan_sha256"] for campaign in campaigns}
        for row in rows:
            attempt_accounting.exact_fields(row, frozenset({
                "scope_sha256", "campaign_id", "plan_sha256", "attempt_id", "request_id",
                "profile", "participants", "warmup", "state", "reason",
            }), "accounting row")
            name = row["campaign_id"]
            if (not isinstance(name, str) or name not in plans or row["scope_sha256"] != scope["sha256"]
                    or row["plan_sha256"] != plans[name] or row["profile"] not in PROFILES
                    or type(row["participants"]) is not int or row["participants"] not in REQUIRED_PARTICIPANTS
                    or type(row["warmup"]) is not bool or not isinstance(row["state"], str)
                    or row["state"] not in reasons or row["reason"] not in reasons[row["state"]]):
                raise EvidenceError(f"{label} accounting row is invalid or incomplete")
            identity = attempt_accounting.registered_attempt_id(scope["sha256"], name, plans[name], row["request_id"])
            if row["attempt_id"] != identity or identity in seen:
                raise EvidenceError(f"{label} accounting attempt is substituted or duplicated")
            seen.add(identity)
            campaign_rows[name].append(row)
        if [row["campaign_id"] for row in rows] != sorted(row["campaign_id"] for row in rows):
            raise EvidenceError(f"{label} accounting campaign rows are reordered")
        def check_counts(actual, subset):
            attempt_accounting.exact_fields(actual, frozenset(attempt_accounting.COUNT_FIELDS), "accounting counts")
            for value in actual.values():
                attempt_accounting.unsigned_milliseconds(value, "accounting count")
            attempt_accounting._same(actual, attempt_accounting._counter(subset), "accounting counts")
        check_counts(accounting["counts"], rows)
        for campaign in campaigns:
            check_counts(campaign["counts"], campaign_rows[campaign["campaign_id"]])
        expected_cohorts = []
        for profile, participants, warmup in sorted({(row["profile"], row["participants"], row["warmup"]) for row in rows}):
            subset = [row for row in rows if (row["profile"], row["participants"], row["warmup"]) == (profile, participants, warmup)]
            expected_cohorts.append({"profile": profile, "participants": participants, "warmup": warmup,
                                     "counts": attempt_accounting._counter(subset)})
        attempt_accounting._same(accounting["cohorts"], expected_cohorts, "accounting cohort projection")
        cohorts = {(item["profile"], item["participants"], item["warmup"]): item["counts"] for item in expected_cohorts}
        for profile in PROFILES:
            for participants in REQUIRED_PARTICIPANTS:
                measured = cohorts[(profile, participants, False)]["succeeded"]
                warmups = cohorts[(profile, participants, True)]["succeeded"]
                bucket = report["profiles"][profile][str(participants)]
                if (type(bucket["measured_runs"]) is not int or bucket["measured_runs"] != measured
                        or measured < MIN_MEASURED or warmups < MIN_WARMUPS):
                    raise EvidenceError(f"{label} statistics differ from its successful measured/warmup cohorts")
                for collection in (bucket["stages_ms"], bucket["resources"]):
                    if not isinstance(collection, dict) or not collection:
                        raise EvidenceError(f"{label} statistical summaries are missing")
                    for summary in collection.values():
                        if not isinstance(summary, dict) or type(summary.get("count")) is not int or summary["count"] != measured:
                            raise EvidenceError(f"{label} statistical denominator differs from accounting")
    except (attempt_accounting.AccountingError, KeyError, TypeError) as error:
        raise EvidenceError(f"{label} accounting projection is invalid: {error}") from error


def compare_baseline(
    candidate: Mapping[str, Any], baseline: Mapping[str, Any]
) -> list[dict[str, Any]]:
    """Apply the post-initial-release p95/p99 regression policy."""

    if (
        candidate.get("version") != REPORT_VERSION
        or baseline.get("version") != REPORT_VERSION
        or candidate.get("protocol") != PROTOCOL
        or baseline.get("protocol") != PROTOCOL
    ):
        raise EvidenceError("candidate and baseline must use the V1 settlement profile")
    environments: list[dict[str, Any]] = []
    for label, report in (("candidate", candidate), ("baseline", baseline)):
        validate_report_accounting(report, label)
        environment = report.get("environment")
        expected = {
            "hardware_sha256",
            "hardware_profile_sha256",
            "configuration_sha256_by_participants",
        }
        if not isinstance(environment, dict) or set(environment) != expected:
            raise EvidenceError(f"{label} benchmark environment is malformed")
        configurations = environment["configuration_sha256_by_participants"]
        expected_participants = {str(value) for value in REQUIRED_PARTICIPANTS}
        if any(
            not isinstance(environment[field], str)
            or re.fullmatch(r"[0-9a-f]{64}", environment[field]) is None
            for field in ("hardware_sha256", "hardware_profile_sha256")
        ) or not isinstance(configurations, dict):
            raise EvidenceError(f"{label} benchmark environment is malformed")
        if set(configurations) != expected_participants or any(
            not isinstance(value, str)
            or re.fullmatch(r"[0-9a-f]{64}", value) is None
            for value in configurations.values()
        ):
            raise EvidenceError(f"{label} benchmark environment is malformed")
        environments.append(environment)
    candidate_environment, baseline_environment = environments
    if (
        candidate_environment["hardware_profile_sha256"]
        != baseline_environment["hardware_profile_sha256"]
        or candidate_environment["configuration_sha256_by_participants"]
        != baseline_environment["configuration_sha256_by_participants"]
    ):
        raise EvidenceError(
            "candidate and baseline must use identical hardware profiles and configurations"
        )
    if candidate.get("requirements") != baseline.get("requirements"):
        raise EvidenceError(
            "candidate and baseline must use identical benchmark requirements"
        )
    if candidate["accounting"]["deadline_policy"] != baseline["accounting"]["deadline_policy"]:
        raise EvidenceError("candidate and baseline must use identical declared deadline policies")

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
    parser.add_argument("--scope", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--bootstrap-iterations", type=int, default=2_000)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        # Imported only for CLI collection; the runner itself imports this reporter.
        from private_settlement_release_runner import collect_benchmark_scope
        scope_raw, campaigns, _ = collect_benchmark_scope(args.scope.parent.resolve(strict=True) / args.scope.name)
        report = build_report(load_jsonl(args.input), args.bootstrap_iterations,
                              scope_raw=scope_raw, campaigns=campaigns)
        regressions: list[dict[str, Any]] = []
        if args.baseline is not None:
            try:
                baseline = attempt_accounting._document(args.baseline.read_bytes(), "benchmark baseline")
            except (OSError, attempt_accounting.AccountingError) as error:
                raise EvidenceError(f"cannot read baseline: {error}") from error
            regressions = compare_baseline(report, baseline)
        report["regressions"] = regressions
        report["passed"] = not regressions
        args.output.write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(f"private-settlement benchmark evidence error: {error}", file=sys.stderr)
        return 2
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
