#!/usr/bin/env python3
"""Sumeragi multilane scaling gate: one-lane versus four-lane pairs on a four-validator localnet.

Release requirement F13 (``specs/first_release_completion_goals.md``, "Scaling"): five
fixed-workload one-lane/four-lane pairs, at least 3/2 the committed throughput and at most 5/4
the p95 commit latency with four lanes, with complete resource maxima through drain. Lanes are
those of ``specs/sumeragi_lanes.md``: the ``one_lane`` run is a chain with lane 0 only; the
``four_lane`` run sets a governed ``sumeragi_lane_policy`` with fixed lanes 1, 2 and 3 (every
validator in each pinned committee, dataspace 0) and routes load account ``i`` to lane
``i mod 4``, so each lane carries a quarter of the same workload.

Every run is a fresh ``kagami localnet`` of four validators (the network launcher of
``scripts/sumeragi_soak.py``). The gate registers and funds the load accounts, applies the lane
policy (``four_lane``) and waits until every validator runs every lane instance, then collects
the load with ``iroha tx load``: an open-loop schedule at a fixed offered rate
(warmup, drain, measurement, drain), every request StateApplied globally and on a local
observer, with fixed-cadence resource captures of every validator (RSS, queue, Kura inventory,
``/status`` and ``/metrics`` bodies) by ``scripts/nexus/resource_probe_worker.py`` under the
ten-run evidence ledger of ``scripts/nexus/resource_evidence_budget.py``.

Measurements (all exact rationals):

* committed throughput of a run: measurement requests StateApplied before the end of the
  measurement window, per second of the window (requests applied during the drain count
  against throughput and are kept in the latency sample);
* commit latency of a request: global StateApplied observation minus its offer, both on the
  collector's monotonic clock (resolution: the status poll interval);
* per variant: the median run throughput and the pooled nearest-rank p95 latency;
* the verdict: four-lane median / one-lane median >= 3/2 and four-lane p95 / one-lane p95 <=
  5/4; every run complete (at least 100 measurement requests, all applied, every resource
  sample complete through drain); every four-lane run's lanes carried the load (merged lane
  frontiers advanced and lane instances applied blocks); nodes' own counters are reported
  (``txs_approved`` of ``/status`` between the captures at the start and end of the window).

The verdict is written as ``verdict.json``; the exit status is 0 when every criterion holds, 1
when a criterion fails or a run fails, 2 on a harness error. A run with fewer than five pairs
is a partial verdict that can never qualify the release (``release_qualifying``). The
offered rate must exceed the one-lane capacity for the throughput criterion to be meaningful,
and the drain window must let the one-lane backlog settle (``iroha tx load`` fails
the run otherwise).

Typical runs::

    cargo build --release -p irohad --bin iroha3d -p iroha_kagami --bin kagami -p iroha_cli --bin iroha
    python3 scripts/sumeragi_scaling_gate.py --bin-dir target/release \\
        --offered-load-tps 200 --out artifacts/sumeragi-scaling/gate
    # A short partial run (one pair) on a development machine.
    python3 scripts/sumeragi_scaling_gate.py --bin-dir target/debug --pairs 1 \\
        --offered-load-tps 20 --warmup 8 --measurement 40 --drain 40 \\
        --out artifacts/sumeragi-scaling/smoke
    # Recompute the verdict of a finished run from its files.
    python3 scripts/sumeragi_scaling_gate.py --analyze artifacts/sumeragi-scaling/gate

The resource probe samples processes through Darwin ``libproc``; other hosts fail closed at the
probe admission (``scripts/nexus/resource_process.py``).
"""

from __future__ import annotations

import argparse
import base64
import dataclasses
import hashlib
import json
import os
import re
import secrets
import shutil
import stat
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from fractions import Fraction
from pathlib import Path
from typing import Any, Iterable, Mapping, Optional, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
NEXUS_DIR = SCRIPT_DIR / "nexus"
sys.path.insert(0, str(SCRIPT_DIR))
sys.path.insert(0, str(NEXUS_DIR))

import resource_evidence_budget as budget  # noqa: E402
import sumeragi_soak as soak  # noqa: E402
import sumeragi_soak_logs as soak_logs  # noqa: E402

PLAN_SCHEMA = "iroha.sumeragi.scaling.plan.v1"
RUN_SCHEMA = "iroha.sumeragi.scaling.run.v1"
VERDICT_SCHEMA = "iroha.sumeragi.scaling.verdict.v1"
# Written by `iroha tx load` (crates/iroha_cli/src/transaction_load.rs).
TRACE_SCHEMA = "iroha.sumeragi.scaling.trace.v1"
LOAD_RECEIPT_OPERATION = "transaction_load"
# Resource probe protocol (scripts/nexus/resource_probe_worker.py).
PROBE_CONFIG_SCHEMA = "iroha.sumeragi.resource_probe.config.v1"
CAPTURE_SCHEMA = "iroha.sumeragi.resource_probe.capture.v1"
WORKER_SOURCES = (
    "resource_probe_worker.py",
    "resource_probe.py",
    "resource_process.py",
    "resource_evidence_budget.py",
    "kura_resource_metrics.py",
)

NS = 1_000_000_000
MIB = 1024 * 1024
VALIDATORS = 4
PAIRS = budget.PAIR_COUNT
VARIANTS = ("one_lane", "four_lane")
LANES = {"one_lane": 1, "four_lane": 4}
MIN_THROUGHPUT_RATIO = Fraction(3, 2)
MAX_P95_LATENCY_RATIO = Fraction(5, 4)
MIN_LATENCY_SAMPLES = 100
MAX_ACCOUNTS = 64
MAX_EFFECTS_PER_ACCOUNT = 1024
MAX_REQUESTS = 1_000_000
LANE_POLICY_PARAMETER = "sumeragi_lane_policy"
# The model-owned `SumeragiLanePolicy::for_chain` bounds of a chain's first lane policy.
LANE_ANCHOR_FRESHNESS = 16
LANE_MAX_MERGE_BLOCKS = 16
LANE_STALL_WINDOW = 64
LANE_DATASPACE = 0

EXIT_OK = 0
EXIT_FAIL = 1
EXIT_HARNESS = 2

_DECIMAL_RE = re.compile(r"^(0|[1-9][0-9]{0,11})(\.[0-9]{1,9})?$")
_HEX64_RE = re.compile(r"^[0-9a-f]{64}$")


class GateError(RuntimeError):
    """A harness failure: the gate could not produce a measurement."""


class TraceError(ValueError):
    """A transaction trace that is not a complete, well-formed scaling trace."""


# ---------------------------------------------------------------------------------------------
# Exact arithmetic
# ---------------------------------------------------------------------------------------------


def exact_decimal(text: str, what: str) -> Fraction:
    """A non-negative decimal (at most nine fractional digits) as an exact fraction."""
    if not isinstance(text, str) or _DECIMAL_RE.match(text) is None:
        raise ValueError(f"{what} must be a plain non-negative decimal, got {text!r}")
    return Fraction(text)


def decimal_text(value: Fraction, places: int = 9) -> str:
    """Render ``value`` (a multiple of ``10^-places``) as a canonical decimal string."""
    scaled = value * 10**places
    if scaled.denominator != 1 or scaled < 0:
        raise ValueError(f"{value} is not a non-negative multiple of 1e-{places}")
    whole, fraction = divmod(scaled.numerator, 10**places)
    if fraction == 0:
        return str(whole)
    return f"{whole}.{fraction:0{places}d}".rstrip("0")


def ns_of_seconds(text: str, what: str, unit: int = NS) -> int:
    """Exact integer nanoseconds of a decimal number of seconds (or of another ``unit``)."""
    value = exact_decimal(text, what) * unit
    if value.denominator != 1:
        raise ValueError(f"{what} must be a whole number of nanoseconds")
    return int(value)


def percentile_nearest_rank(values: Sequence[int], percent: int) -> int:
    """The nearest-rank percentile: the ``ceil(percent / 100 * n)``-th smallest value."""
    if not values or not 0 < percent <= 100:
        raise ValueError("percentile of an empty sample or outside (0, 100]")
    ordered = sorted(values)
    return ordered[(percent * len(ordered) + 99) // 100 - 1]


def p95_nearest_rank(values: Sequence[int]) -> int:
    """The nearest-rank 95th percentile, the gate's latency statistic."""
    return percentile_nearest_rank(values, 95)


def median(values: Sequence[Fraction]) -> Fraction:
    """The median; the mean of the two middle values of an even-sized sample."""
    if not values:
        raise ValueError("median of an empty sample")
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) / 2


def fraction_json(value: Optional[Fraction]) -> Optional[dict[str, Any]]:
    """An exact rational with a rounded decimal view for readers."""
    if value is None:
        return None
    return {"exact": f"{value.numerator}/{value.denominator}", "decimal": round(float(value), 6)}


def cohort_count(duration_ns: int, rate: Fraction) -> int:
    """Scheduled requests of one cohort: ``ceil(duration * rate)`` (the collector's rule)."""
    return -(-duration_ns * rate.numerator // (NS * rate.denominator))


def submission_lag_bound_ns(rate: Fraction) -> int:
    """The largest lag the collector admits: one quarter of the arrival period, floored."""
    return (NS * rate.denominator) // (4 * rate.numerator)


# ---------------------------------------------------------------------------------------------
# Options and the evidence ledger
# ---------------------------------------------------------------------------------------------


@dataclass(frozen=True)
class Options:
    """Resolved, validated options of one gate run (everything the plan records)."""

    pairs: int
    accounts: int
    offered_load_tps: str
    warmup_s: str
    measurement_s: str
    drain_s: str
    max_submission_lag_ms: str
    resource_interval_ms: int
    resource_timeout_ms: int
    resource_max_start_lag_ms: int
    status_body_bytes: int
    metrics_body_bytes: int
    journal_bytes: int
    trace_bytes: int
    receipt_bytes: int
    manifest_bytes: int
    report_bytes: int
    min_latency_samples: int
    seed_namespace: str
    fund: str
    base_api_port: int
    base_p2p_port: int
    storage_budget_mb: int
    preparation_lookahead: int
    preparation_concurrency: int
    preparation_ahead_ms: int
    max_submissions: int
    max_in_flight: int
    max_status_requests: int
    poll_interval_ms: int
    journal_capacity: int
    keep_state: bool = False

    @property
    def rate(self) -> Fraction:
        return exact_decimal(self.offered_load_tps, "--offered-load-tps")

    @property
    def warmup_ns(self) -> int:
        return ns_of_seconds(self.warmup_s, "--warmup")

    @property
    def measurement_ns(self) -> int:
        return ns_of_seconds(self.measurement_s, "--measurement")

    @property
    def drain_ns(self) -> int:
        return ns_of_seconds(self.drain_s, "--drain")

    @property
    def geometry(self) -> budget.CaptureGeometry:
        return budget.CaptureGeometry(
            VALIDATORS,
            self.resource_interval_ms * 1_000_000,
            self.measurement_ns,
            self.drain_ns,
        )


def pair_seed(namespace: str, pair: int) -> str:
    """The exact workload seed of a pair: both variants of a pair share it."""
    return hashlib.sha256(f"{namespace}:{pair}".encode()).hexdigest()


def run_order(pairs: int) -> list[tuple[int, str]]:
    """Run order: odd pairs start with ``one_lane``, even pairs with ``four_lane``."""
    order = []
    for pair in range(1, pairs + 1):
        variants = VARIANTS if pair % 2 else tuple(reversed(VARIANTS))
        order.extend((pair, variant) for variant in variants)
    return order


def validate_options(options: Options) -> None:
    """Reject a plan the collector, the probe or the evidence ledger would refuse."""
    if not 1 <= options.pairs <= PAIRS:
        raise ValueError(f"--pairs must be in 1..={PAIRS}")
    if options.accounts % 4 or not 4 <= options.accounts <= MAX_ACCOUNTS:
        raise ValueError("--accounts must be a multiple of four in 4..=64 (one route per lane)")
    rate = options.rate
    if rate <= 0 or rate > NS:
        raise ValueError("--offered-load-tps must be positive and at most one per nanosecond")
    warmup, measurement, drain = options.warmup_ns, options.measurement_ns, options.drain_ns
    if measurement <= 0 or not 0 < drain <= 300 * NS:
        raise ValueError("--measurement must be positive and --drain in (0, 300] seconds")
    lag_ns = ns_of_seconds(options.max_submission_lag_ms, "--max-submission-lag-ms", 1_000_000)
    if lag_ns > submission_lag_bound_ns(rate):
        raise ValueError("--max-submission-lag-ms exceeds one quarter of the arrival period")
    counts = (cohort_count(warmup, rate), cohort_count(measurement, rate))
    if counts[1] == 0 or any(count % options.accounts for count in counts):
        raise ValueError(
            f"each cohort must hold whole account rounds: {counts[0]} warmup and {counts[1]} "
            f"measurement requests over {options.accounts} accounts"
        )
    if sum(counts) > MAX_REQUESTS or sum(counts) // options.accounts > MAX_EFFECTS_PER_ACCOUNT:
        raise ValueError(
            f"{sum(counts)} requests exceed {MAX_EFFECTS_PER_ACCOUNT} effects per account; "
            "add accounts or shorten the schedule"
        )
    if counts[1] < options.min_latency_samples:
        raise ValueError(
            f"the measurement cohort ({counts[1]} requests) is below the "
            f"{options.min_latency_samples}-sample latency minimum"
        )
    interval = options.resource_interval_ms
    if not (
        2 <= interval <= 60_000
        and 1 <= options.resource_timeout_ms <= interval // 2
        and 0 <= options.resource_max_start_lag_ms <= interval // 4
    ):
        raise ValueError("resource timing needs 2 <= interval <= 60000 ms, timeout <= interval/2, lag <= interval/4")
    try:
        options.geometry  # noqa: B018 - constructor validation
    except budget.BudgetError as error:
        raise ValueError(f"resource sampling geometry rejected: {error}") from None
    if not re.fullmatch(r"\d+(\.\d+)?", options.fund):
        raise ValueError("--fund must be a non-negative decimal quantity")


def worker_static_files() -> tuple[budget.StaticFile, ...]:
    """The five pinned probe-worker sources, charged at their actual sizes."""
    return tuple(
        budget.StaticFile(f"source.{name}", (NEXUS_DIR / name).stat().st_size) for name in WORKER_SOURCES
    )


def evidence_budget(options: Options, static_files: tuple[budget.StaticFile, ...]) -> budget.EvidenceBudget:
    """Admit the complete ten-run ledger (all five pairs, even for a partial run)."""
    runs = []
    for pair in range(1, PAIRS + 1):
        for variant in VARIANTS:
            prefix = f"pair{pair}.{variant}"
            runs.append(
                budget.RunBudget(
                    pair,
                    variant,
                    options.geometry,
                    budget.FileBudget(f"{prefix}.journal", options.journal_bytes),
                    budget.FileBudget(f"{prefix}.trace", options.trace_bytes),
                    budget.FileBudget(f"{prefix}.receipt", options.receipt_bytes),
                )
            )
    return budget.admit_experiment(
        policy=budget.CapturePolicy(options.status_body_bytes, options.metrics_body_bytes),
        runs=tuple(runs),
        static_files=static_files,
        manifest=budget.FileBudget("plan", options.manifest_bytes),
        report=budget.FileBudget("verdict", options.report_bytes),
        other_control=(),
    )


def write_bounded_json(path: Path, value: Any, cap: int) -> int:
    """Write ``value`` as JSON unless it exceeds its admitted allocation; never truncate."""
    raw = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    if len(raw) > cap:
        raise GateError(f"{path.name} is {len(raw)} bytes, above its {cap}-byte allocation")
    staging = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    staging.write_bytes(raw)
    os.replace(staging, path)
    return len(raw)


# ---------------------------------------------------------------------------------------------
# Measurements
# ---------------------------------------------------------------------------------------------


@dataclass(frozen=True)
class RunMeasurement:
    """Exact measurements of one run's transaction trace."""

    pair_index: int
    variant: str
    warmup_requests: int
    measurement_requests: int
    committed_in_window: int
    drain_committed: int
    throughput_tps: Fraction
    p95_latency_ns: int
    max_latency_ns: int
    latencies_ns: tuple[int, ...]
    first_height: int
    last_height: int

    def to_json(self) -> dict[str, Any]:
        """The measurement without the raw latency sample (the trace keeps it)."""
        return {
            "warmup_requests": self.warmup_requests,
            "measurement_requests": self.measurement_requests,
            "committed_in_window": self.committed_in_window,
            "drain_committed": self.drain_committed,
            "throughput_tps": fraction_json(self.throughput_tps),
            "p50_latency_ms": round(percentile_nearest_rank(self.latencies_ns, 50) / 1e6, 3),
            "p95_latency_ms": round(self.p95_latency_ns / 1e6, 3),
            "p99_latency_ms": round(percentile_nearest_rank(self.latencies_ns, 99) / 1e6, 3),
            "max_latency_ms": round(self.max_latency_ns / 1e6, 3),
            "first_height": self.first_height,
            "last_height": self.last_height,
        }


def _int(value: Any, what: str, low: int = 0) -> int:
    if type(value) is not int or value < low:
        raise TraceError(f"{what} must be an integer >= {low}")
    return value


def measure_trace(
    trace: Mapping[str, Any],
    *,
    pair_index: int,
    variant: str,
    seed: str,
    measurement_ns: int,
    expected_warmup: int,
    expected_measurement: int,
) -> RunMeasurement:
    """Measure one run: committed-in-window throughput and offer-to-Applied latency.

    Every scheduled request must be present exactly once in its cohort order and settled; a
    trace from another pair, variant or seed, or with a missing, duplicated, reordered or
    inconsistent row, is not a measurement.
    """
    if not isinstance(trace, Mapping) or trace.get("schema") != TRACE_SCHEMA:
        raise TraceError("not a scaling transaction trace")
    if (trace.get("pair_index"), trace.get("variant"), trace.get("seed")) != (pair_index, variant, seed):
        raise TraceError("trace belongs to another pair, variant or seed")
    rows = trace.get("transactions")
    if not isinstance(rows, list):
        raise TraceError("trace has no transaction list")
    expected = [("warmup", index) for index in range(1, expected_warmup + 1)]
    expected += [("measurement", index) for index in range(1, expected_measurement + 1)]
    if len(rows) != len(expected):
        raise TraceError(f"trace has {len(rows)} rows, the schedule has {len(expected)}")
    latencies: list[int] = []
    committed = 0
    heights: list[int] = []
    hashes: set[str] = set()
    for row, (cohort, sequence) in zip(rows, expected, strict=True):
        if not isinstance(row, Mapping) or (row.get("cohort"), row.get("sequence")) != (cohort, sequence):
            raise TraceError(f"trace row out of schedule order at {cohort} {sequence}")
        transaction_hash = row.get("hash")
        if not isinstance(transaction_hash, str) or transaction_hash in hashes:
            raise TraceError("trace repeats or omits a transaction hash")
        hashes.add(transaction_hash)
        offer = _int(row.get("offer_offset_ns"), "offer_offset_ns", -(1 << 62))
        scheduled = _int(row.get("scheduled_offset_ns"), "scheduled_offset_ns", -(1 << 62))
        acknowledgment = row.get("acknowledgment")
        applied = row.get("applied")
        if not isinstance(acknowledgment, Mapping) or not isinstance(applied, Mapping):
            raise TraceError("trace row has no acknowledgment or applied observation")
        if (applied.get("status"), applied.get("scope"), applied.get("resolved_from")) != ("Applied", "global", "state"):
            raise TraceError("trace row is not a global state-resolved Applied observation")
        if acknowledgment.get("status") != "Accepted" or applied.get("hash") != transaction_hash:
            raise TraceError("trace row does not bind its accepted transaction")
        applied_ns = _int(applied.get("offset_ns"), "applied offset", -(1 << 62))
        height = _int(applied.get("block_height"), "block_height", 1)
        if offer < scheduled or applied_ns <= offer:
            raise TraceError("trace row applies before its offer or offers before its schedule")
        if cohort == "measurement":
            if applied_ns > measurement_ns + (1 << 62):
                raise TraceError("applied offset out of range")
            latencies.append(applied_ns - offer)
            heights.append(height)
            if applied_ns < measurement_ns:
                committed += 1
    throughput = Fraction(committed * NS, measurement_ns)
    return RunMeasurement(
        pair_index=pair_index,
        variant=variant,
        warmup_requests=expected_warmup,
        measurement_requests=expected_measurement,
        committed_in_window=committed,
        drain_committed=expected_measurement - committed,
        throughput_tps=throughput,
        p95_latency_ns=p95_nearest_rank(latencies),
        max_latency_ns=max(latencies),
        latencies_ns=tuple(latencies),
        first_height=min(heights),
        last_height=max(heights),
    )


def capture_manifests(capture_dir: Path) -> list[dict[str, Any]]:
    """Every published capture manifest of a run, preflight first, samples in order."""
    manifests = []
    for path in sorted(capture_dir.glob("*.json")):
        if re.fullmatch(r"(preflight|sample)-[0-9]{10}\.json", path.name) is None:
            continue
        manifests.append(json.loads(path.read_text()))
    manifests.sort(key=lambda value: (value.get("kind") != "preflight", value.get("sequence", 0)))
    return manifests


def resource_maxima(manifests: Sequence[Mapping[str, Any]], expected_samples: int) -> dict[str, Any]:
    """Resource maxima of a run through drain, and whether the capture series is complete.

    Complete means the preflight and every scheduled sample (measurement start through the
    end of the drain) were published and available (every validator's process, ``/status``
    queue and Kura inventory observed).
    """
    samples = [value for value in manifests if value.get("kind") == "sample"]
    preflight = [value for value in manifests if value.get("kind") == "preflight"]
    sequences = [value.get("sequence") for value in samples]
    complete = (
        len(preflight) == 1
        and sequences == list(range(1, expected_samples + 1))
        and all(value.get("schema") == CAPTURE_SCHEMA and value.get("available") is True for value in manifests)
    )
    maxima: dict[str, Optional[int]] = {
        "rss_bytes_sum": None,
        "rss_bytes_peer": None,
        "queue_size_sum": None,
        "queue_size_peer": None,
        "kura_storage_bytes_sum": None,
        "kura_represented_entries_sum": None,
    }

    def raise_to(key: str, value: Any) -> None:
        if type(value) is int and (maxima[key] is None or value > maxima[key]):
            maxima[key] = value

    for value in manifests:
        aggregates = value.get("aggregates") or {}
        raise_to("rss_bytes_sum", aggregates.get("rss_before_bytes"))
        raise_to("rss_bytes_sum", aggregates.get("rss_after_bytes"))
        raise_to("queue_size_sum", aggregates.get("queue_size_sum"))
        raise_to("queue_size_peer", aggregates.get("queue_size_max"))
        inventory = aggregates.get("inventory") or {}
        raise_to("kura_storage_bytes_sum", inventory.get("storage_bytes"))
        raise_to("kura_represented_entries_sum", inventory.get("represented_entries"))
        for peer in value.get("peers") or []:
            for sample in ("process_before", "process_after"):
                raise_to("rss_bytes_peer", (peer.get(sample) or {}).get("rss_bytes"))
    return {"complete": complete, "captures": len(manifests), "expected_samples": expected_samples, "maxima": maxima}


def node_counter_window(
    capture_dir: Path, manifests: Sequence[Mapping[str, Any]], window_samples: tuple[int, int]
) -> Optional[dict[str, Any]]:
    """Nodes' own committed-transaction counters across the measurement window.

    ``txs_approved`` of each validator's ``/status`` in the captures at the start (sample 1,
    offset 0) and end (the sample at the end of the measurement window) of the window.
    """
    by_sequence = {value.get("sequence"): value for value in manifests if value.get("kind") == "sample"}
    first, last = (by_sequence.get(sequence) for sequence in window_samples)
    if first is None or last is None:
        return None
    peers = []
    for start, end in zip(first.get("peers") or [], last.get("peers") or [], strict=False):
        values = []
        for peer in (start, end):
            body = ((peer.get("status") or {}).get("body") or {}).get("name")
            status = json.loads((capture_dir / body).read_text()) if body else {}
            values.append((status.get("txs_approved"), status.get("blocks")))
        (tx_start, block_start), (tx_end, block_end) = values
        if not all(type(item) is int for item in (tx_start, tx_end, block_start, block_end)):
            return None
        peers.append(
            {
                "peer": start.get("peer_id"),
                "txs_approved": tx_end - tx_start,
                "blocks": block_end - block_start,
            }
        )
    if not peers:
        return None
    return {"window_samples": list(window_samples), "peers": peers}


def committed_blocks(before: Mapping[str, Any], after: Mapping[str, Any]) -> Optional[int]:
    """Global blocks every validator committed between two ``/v1/sumeragi/status`` snapshots."""
    def lowest(statuses: Mapping[str, Any]) -> Optional[int]:
        heights = [
            status.get("committed_height")
            for status in statuses.values()
            if isinstance(status, Mapping) and type(status.get("committed_height")) is int
        ]
        return min(heights) if len(heights) == len(statuses) and heights else None

    start, end = lowest(before), lowest(after)
    return None if start is None or end is None else end - start


def lane_frontiers(lanes_by_peer: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    """Per lane: the lowest merged height and total rescued count over validators, halts."""
    frontier: dict[str, dict[str, Any]] = {}
    for peer, statuses in lanes_by_peer.items():
        if not isinstance(statuses, list):
            continue
        for status in statuses:
            record = status.get("record") or {}
            lane = str(record.get("lane"))
            merged = (record.get("merged") or {}).get("height")
            instance = status.get("instance")
            entry = frontier.setdefault(lane, {"merged_height": None, "rescued": 0, "halted": [], "peers": []})
            if type(merged) is int and (entry["merged_height"] is None or merged < entry["merged_height"]):
                entry["merged_height"] = merged
            if type(record.get("rescued")) is int:
                entry["rescued"] += record["rescued"]
            if instance is None or instance.get("halted") is not None:
                entry["halted"].append(peer)
            entry["peers"].append(peer)
    return frontier


def lanes_used(
    variant: str,
    before: Mapping[str, Any],
    after: Mapping[str, Any],
    applied_blocks: Mapping[str, int],
) -> tuple[bool, list[str]]:
    """Whether the run used exactly its lanes: every fixed lane merged new blocks everywhere.

    ``applied_blocks`` counts, per lane, the lane-instance blocks the validators' audit logs
    applied while the load ran. A one-lane run has no lane records.
    """
    problems = []
    frontier_before, frontier_after = lane_frontiers(before), lane_frontiers(after)
    expected = [str(lane) for lane in range(1, LANES[variant])]
    if sorted(frontier_after) != sorted(expected):
        problems.append(f"lane records {sorted(frontier_after)} differ from the expected {expected}")
    for lane in expected:
        start = (frontier_before.get(lane) or {}).get("merged_height")
        end = (frontier_after.get(lane) or {}).get("merged_height")
        if not (type(start) is int and type(end) is int and end > start):
            problems.append(f"lane {lane} merged no new block on every validator ({start} -> {end})")
        halted = (frontier_after.get(lane) or {}).get("halted") or []
        if halted:
            problems.append(f"lane {lane} instance missing or halted on {halted}")
        if applied_blocks.get(lane, 0) <= 0:
            problems.append(f"no validator logged an applied block of lane {lane}")
    return not problems, problems


def merge_maxima(values: Iterable[Mapping[str, Any]]) -> dict[str, Optional[int]]:
    """Element-wise maxima of several runs' resource maxima (``None`` where never observed)."""
    merged: dict[str, Optional[int]] = {}
    for maxima in values:
        for name, value in maxima.items():
            current = merged.setdefault(name, None)
            if type(value) is int and (current is None or value > current):
                merged[name] = value
    return merged


def run_failures(
    runs: Sequence[Mapping[str, Any]],
    measurements: Mapping[tuple[int, str], RunMeasurement],
    pairs: int,
    min_latency_samples: int,
) -> list[str]:
    """Why the runs do not form a complete experiment of ``pairs`` pairs (empty: complete)."""
    failures: list[str] = []
    for run in runs:
        key = (run.get("pair_index"), run.get("variant"))
        label = f"pair {key[0]} {key[1]}"
        if run.get("status") != "complete":
            failures.append(f"{label}: {run.get('failure') or 'did not complete'}")
            continue
        measurement = measurements.get(key)
        if measurement is None:
            failures.append(f"{label}: no trace measurement")
            continue
        if measurement.measurement_requests < min_latency_samples:
            failures.append(
                f"{label}: {measurement.measurement_requests} latency samples, below the minimum {min_latency_samples}"
            )
        if not (run.get("resources") or {}).get("complete"):
            failures.append(f"{label}: resource captures incomplete through drain")
        lanes = run.get("lanes") or {}
        if not lanes.get("used"):
            failures.append(f"{label}: " + "; ".join(lanes.get("problems") or ["lane usage not established"]))
    expected = {(pair, variant) for pair in range(1, pairs + 1) for variant in VARIANTS}
    present = {(run.get("pair_index"), run.get("variant")) for run in runs}
    failures.extend(f"pair {pair} {variant}: not run" for pair, variant in sorted(expected - present))
    return failures


def evaluate(
    runs: Sequence[Mapping[str, Any]],
    measurements: Mapping[tuple[int, str], RunMeasurement],
    pairs: int,
    min_latency_samples: int,
) -> dict[str, Any]:
    """The verdict of the finished runs (pure: records in, verdict out).

    The ratios need a measurement of every run of both variants; the verdict holds when the
    runs are complete and both ratios meet their F13 thresholds.
    """
    failures = run_failures(runs, measurements, pairs, min_latency_samples)
    complete = not failures
    per_variant: dict[str, dict[str, Any]] = {}
    for variant in VARIANTS:
        rows = [measurements[(pair, variant)] for pair in range(1, pairs + 1) if (pair, variant) in measurements]
        summary: dict[str, Any] = {"runs": len(rows)}
        if len(rows) == pairs:
            summary["median_throughput_tps"] = median([row.throughput_tps for row in rows])
            summary["pooled_p95_latency_ns"] = p95_nearest_rank([value for row in rows for value in row.latencies_ns])
            summary["resource_maxima"] = merge_maxima(
                (run.get("resources") or {}).get("maxima") or {} for run in runs if run.get("variant") == variant
            )
        per_variant[variant] = summary
    one, four = per_variant["one_lane"], per_variant["four_lane"]
    throughput_ratio = latency_ratio = None
    if "median_throughput_tps" in one and "median_throughput_tps" in four:
        if one["median_throughput_tps"] > 0:
            throughput_ratio = four["median_throughput_tps"] / one["median_throughput_tps"]
        latency_ratio = Fraction(four["pooled_p95_latency_ns"], one["pooled_p95_latency_ns"])
    throughput_ok = throughput_ratio is not None and throughput_ratio >= MIN_THROUGHPUT_RATIO
    latency_ok = latency_ratio is not None and latency_ratio <= MAX_P95_LATENCY_RATIO
    if throughput_ratio is not None and not throughput_ok:
        failures.append(
            f"throughput ratio {float(throughput_ratio):.3f} is below {MIN_THROUGHPUT_RATIO} "
            "(four-lane median / one-lane median)"
        )
    if latency_ratio is not None and not latency_ok:
        failures.append(
            f"p95 latency ratio {float(latency_ratio):.3f} is above {MAX_P95_LATENCY_RATIO} "
            "(four-lane pooled p95 / one-lane pooled p95)"
        )
    ok = complete and throughput_ok and latency_ok

    def variant_json(values: Mapping[str, Any]) -> dict[str, Any]:
        result: dict[str, Any] = {"runs": values["runs"]}
        if "median_throughput_tps" in values:
            result["median_throughput_tps"] = fraction_json(values["median_throughput_tps"])
            result["pooled_p95_latency_ms"] = round(values["pooled_p95_latency_ns"] / 1e6, 3)
            result["resource_maxima"] = values["resource_maxima"]
        return result

    def run_json(run: Mapping[str, Any]) -> dict[str, Any]:
        key = (run.get("pair_index"), run.get("variant"))
        return {
            "pair_index": key[0],
            "variant": key[1],
            "status": run.get("status"),
            "failure": run.get("failure"),
            "measurement": measurements[key].to_json() if key in measurements else None,
            "node_counters": run.get("node_counters"),
            "global_blocks_committed": run.get("global_blocks_committed"),
            "lanes": run.get("lanes"),
            "resources": run.get("resources"),
        }

    return {
        "schema": VERDICT_SCHEMA,
        "ok": ok,
        "release_qualifying": ok and pairs == PAIRS and min_latency_samples >= MIN_LATENCY_SAMPLES,
        "pairs": pairs,
        "thresholds": {
            "min_throughput_ratio": str(MIN_THROUGHPUT_RATIO),
            "max_p95_latency_ratio": str(MAX_P95_LATENCY_RATIO),
            "min_latency_samples": min_latency_samples,
        },
        "throughput_ratio": fraction_json(throughput_ratio),
        "p95_latency_ratio": fraction_json(latency_ratio),
        "criteria": {"runs_complete": complete, "throughput": throughput_ok, "latency": latency_ok},
        "one_lane": variant_json(one),
        "four_lane": variant_json(four),
        "runs": [run_json(run) for run in sorted(runs, key=lambda item: (item.get("pair_index", 0), item.get("variant", "")))],
        "failures": failures,
    }


# ---------------------------------------------------------------------------------------------
# Lane policy
# ---------------------------------------------------------------------------------------------


def lane_of_account(index: int, lanes: int) -> int:
    """Account ``index`` routes to lane ``index mod lanes`` (lane 0 is the default route)."""
    return index % lanes


def lane_policy(
    lane_params: Mapping[str, Any],
    da_layout: Mapping[str, Any],
    members: Sequence[Mapping[str, str]],
    account_ids: Sequence[str],
    lanes: int,
) -> dict[str, Any]:
    """The ``SumeragiLanePolicy`` of a ``lanes``-lane run (``lanes - 1`` fixed lanes).

    Every fixed lane pins the whole validator set (``members``: BLS-normal ``peer`` keys with
    base64 ``pop``) in dataspace 0; explicit account routes send each load account to its lane
    (accounts of lane 0 need no route). The bounds are those ``for_chain`` gives a chain's
    first policy; ``lane_params`` are the chain's own consensus parameters.
    """
    if lanes < 2 or len(account_ids) % lanes:
        raise ValueError("a lane policy needs at least two lanes and whole account rounds")
    if not members:
        raise ValueError("a fixed lane needs a committee")
    committee = [{"peer": member["peer"], "pop": member["pop"]} for member in members]
    return {
        "da_layout": dict(da_layout),
        "anchor_freshness": LANE_ANCHOR_FRESHNESS,
        "max_merge_blocks": LANE_MAX_MERGE_BLOCKS,
        "stall_window": LANE_STALL_WINDOW,
        "lane_params": dict(lane_params),
        "fixed": [
            {"lane": lane, "dataspace": LANE_DATASPACE, "committee": committee} for lane in range(1, lanes)
        ],
        "routes": [
            {"lane": lane_of_account(index, lanes), "account": account, "instruction": None}
            for index, account in enumerate(account_ids)
            if lane_of_account(index, lanes) != 0
        ],
        "autoscale": None,
    }


def lane_policy_parameter(policy: Mapping[str, Any]) -> dict[str, Any]:
    """The ``SetParameter`` payload (``Parameter::Custom``) that installs ``policy``."""
    return {"Custom": {"id": LANE_POLICY_PARAMETER, "payload": policy}}


# ---------------------------------------------------------------------------------------------
# Harness: network, accounts, lanes, load
# ---------------------------------------------------------------------------------------------


def sha256_file(path: Path) -> str:
    """SHA-256 of a file's contents."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def pin_executable(source: Path, target: Path) -> str:
    """Copy ``source`` to a private single-link executable; return its SHA-256.

    The probe admits only an owned, single-link executable image (Cargo hard-links its
    outputs), and pins the running validators to its exact path and digest.
    """
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    os.chmod(target, 0o755)
    return sha256_file(target)


def write_private(path: Path, text: str) -> None:
    """Write an owner-only (0600) file atomically."""
    path.parent.mkdir(parents=True, exist_ok=True)
    staging = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    descriptor = os.open(staging, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w") as handle:
        handle.write(text)
    os.replace(staging, path)


def toml_value(text: str, section: Optional[str], key: str) -> Optional[str]:
    """A basic-string value of ``key`` at the top level (``section=None``) or in ``[section]``."""
    if section is not None:
        return soak.toml_string_value(text, section, key)
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            return None
        match = re.match(rf'^{re.escape(key)}\s*=\s*"((?:[^"\\]|\\.)*)"', stripped)
        if match:
            return json.loads(f'"{match.group(1)}"')
    return None


class GateNetwork(soak.Localnet):
    """A soak localnet (fresh four-validator ``kagami localnet``) with gate helpers."""

    def cli(self, config: Path, arguments: list[str], timeout: float = 120.0, stdin: Optional[str] = None) -> str:
        """Run ``iroha --config <config> <arguments>``; its stdout, or a GateError."""
        result = subprocess.run(
            [str(self.bins["iroha"]), "--config", str(config), *arguments],
            input=stdin,
            capture_output=True,
            text=True,
            check=False,
            timeout=timeout,
        )
        if result.returncode != 0:
            raise GateError(
                f"iroha {' '.join(arguments[:4])} failed ({result.returncode}): "
                + (result.stdout + result.stderr).strip()[-800:]
            )
        return result.stdout

    def funder_config(self) -> Path:
        """An owner-only client config signing with the genesis key (it minted the fee asset)."""
        path = self.run_dir / "clients" / "funder.toml"
        if not path.exists():
            text = self.client_config(0).read_text()
            for key, source in (("public_key", "genesis.public_key"), ("private_key", "genesis.private_key")):
                value = (self.net_dir / source).read_text().strip()
                text = soak.set_toml_keys(text, "account", {key: soak.toml_string(value)})
            write_private(path, text)
        return path

    def account_literal(self, public_key: str) -> str:
        """The canonical account literal of an Ed25519 public key."""
        output = self.cli(self.client_config(0), ["tools", "address", "convert", public_key], 60)
        return output.strip().splitlines()[-1].strip()

    def load_accounts(self, count: int, seed: str) -> list[dict[str, Any]]:
        """``count`` load accounts: deterministic keys per pair, one client config each.

        Account ``i`` submits to validator ``i mod 4`` and routes to lane ``i mod 4``.
        """
        clients = self.run_dir / "accounts"
        base = self.run_dir / "accounts-base.toml"
        client_text = self.client_config(0).read_text()
        network_id_file = toml_value(client_text, None, "network_id_file")
        if not network_id_file:
            raise GateError("the localnet client config names no network_id_file")
        network_id = Path(network_id_file).read_text().strip()
        chain = toml_value(client_text, None, "chain")
        domain = soak.toml_string_value(client_text, "account", "domain")
        write_private(
            base,
            f"chain = {json.dumps(chain)}\nnetwork_id = {json.dumps(network_id)}\n"
            f'torii_url = "http://127.0.0.1:{self.api_port(0)}/"\n',
        )
        names = [f"load{index}" for index in range(count)]
        command = [
            str(self.bins["kagami"]),
            "advanced",
            "client-configs",
            "--base-config",
            str(base),
            "--out-dir",
            str(clients),
            "--domain",
            domain or "universal",
            "--seed-hex",
            hashlib.sha256(f"scaling-accounts:{seed}".encode()).hexdigest(),
            "--names",
            ",".join(names),
        ]
        result = subprocess.run(command, capture_output=True, text=True, check=False, timeout=120)
        if result.returncode != 0:
            raise GateError(f"kagami client-configs failed: {(result.stdout + result.stderr).strip()[-600:]}")
        accounts = []
        for index, name in enumerate(names):
            path = clients / f"{name}.toml"
            text = path.read_text()
            text = re.sub(
                r'^torii_url\s*=\s*"[^"]*"',
                f'torii_url = "http://127.0.0.1:{self.api_port(index % VALIDATORS)}/"',
                text,
                count=1,
                flags=re.MULTILINE,
            )
            write_private(path, text)
            public_key = soak.toml_string_value(text, "account", "public_key")
            if not public_key:
                raise GateError(f"{path} has no [account] public_key")
            accounts.append(
                {"index": index, "config": path, "public_key": public_key, "account_id": self.account_literal(public_key)}
            )
        return accounts

    def register_and_fund(self, accounts: Sequence[Mapping[str, Any]], quantity: str) -> dict[str, str]:
        """Register every load account (universal identity only) and mint it fee asset."""
        operator = self.client_config(0)
        funder = self.funder_config()
        asset = self.fee_asset_id()
        for account in accounts:
            self.cli(
                operator,
                ["--fee-payer", "authority", "account", "register", "--id", account["account_id"]],
                180,
            )
            if float(quantity) > 0:
                self.cli(
                    funder,
                    [
                        "--fee-payer",
                        "authority",
                        "ledger",
                        "asset",
                        "mint",
                        "--definition",
                        asset,
                        "--account",
                        account["account_id"],
                        "--quantity",
                        quantity,
                    ],
                    180,
                )
        return {"asset": asset, "quantity": quantity}

    def validator_members(self) -> list[dict[str, str]]:
        """Every validator's BLS-normal consensus key and base64 proof of possession."""
        genesis = json.loads((self.net_dir / "genesis.json").read_text())
        topology = genesis.get("topology") or []
        members = []
        for entry in topology:
            peer = entry.get("peer") if isinstance(entry, dict) else None
            pop_hex = entry.get("pop_hex") if isinstance(entry, dict) else None
            if not peer or not pop_hex:
                raise GateError("genesis topology entry without a peer key and proof of possession")
            members.append({"peer": peer, "pop": base64.b64encode(bytes.fromhex(pop_hex.removeprefix("0x"))).decode()})
        if len(members) != VALIDATORS:
            raise GateError(f"genesis names {len(members)} validators, expected {VALIDATORS}")
        return members

    def chain_parameters(self) -> Mapping[str, Any]:
        """The chain's committed parameters (``iroha ledger parameter list all``)."""
        return json.loads(self.cli(self.client_config(0), ["ledger", "parameter", "list", "all"], 60))

    def da_layout(self) -> Mapping[str, Any]:
        """The chain's signed data-availability layout (genesis ``sumeragi_context``)."""
        genesis = json.loads((self.net_dir / "genesis.json").read_text())
        layout = (genesis.get("sumeragi_context") or {}).get("da_layout")
        if not isinstance(layout, dict):
            raise GateError("genesis.json has no sumeragi_context.da_layout")
        return layout

    def apply_lane_policy(self, policy: Mapping[str, Any]) -> None:
        """Submit the policy as a governed ``SetParameter`` by the operator account."""
        self.cli(
            self.client_config(0),
            ["--fee-payer", "authority", "ledger", "parameter", "set"],
            180,
            stdin=json.dumps(lane_policy_parameter(policy)),
        )

    def lanes(self) -> dict[str, Any]:
        """``/v1/sumeragi/lanes`` of every validator (``None`` where it did not answer)."""
        return {
            self.node(index): http_json_any(f"http://127.0.0.1:{self.api_port(index)}/v1/sumeragi/lanes")
            for index in range(self.validators)
        }

    def statuses(self) -> dict[str, Any]:
        """``/v1/sumeragi/status`` of every validator."""
        return {
            self.node(index): soak.http_json(f"http://127.0.0.1:{self.api_port(index)}/v1/sumeragi/status")
            for index in range(self.validators)
        }

    def wait_for_lane_instances(self, lanes: int, timeout_s: float) -> dict[str, Any]:
        """Wait until every validator runs an unhalted instance of every fixed lane."""
        deadline = time.monotonic() + timeout_s
        expected = {str(lane) for lane in range(1, lanes)}
        snapshot: dict[str, Any] = {}
        while time.monotonic() < deadline:
            snapshot = self.lanes()
            ready = True
            for statuses in snapshot.values():
                running = {
                    str((status.get("record") or {}).get("lane"))
                    for status in statuses or []
                    if status.get("instance") is not None and status["instance"].get("halted") is None
                }
                ready &= expected <= running
            if ready:
                return snapshot
            time.sleep(1.0)
        raise GateError(f"validators did not all run lanes {sorted(expected)} within {timeout_s:.0f} s: {snapshot}")


def http_json_any(url: str, timeout: float = 3.0) -> Any:
    """GET ``url`` as JSON of any shape, ``None`` on any failure."""
    request = urllib.request.Request(url, headers={"Accept": "application/json"})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return json.loads(response.read().decode("utf-8"))
    except (urllib.error.URLError, OSError, ValueError, TimeoutError):
        return None


def applied_lane_blocks(log_root: Path, lanes_snapshot: Mapping[str, Any], since_ms: float, until_ms: float) -> dict[str, int]:
    """Per lane, blocks the validators' audit logs applied for that lane's instance in a window."""
    instances: dict[str, str] = {}
    for statuses in lanes_snapshot.values():
        for status in statuses or []:
            instance = (status.get("instance") or {}).get("instance")
            lane = (status.get("record") or {}).get("lane")
            if isinstance(instance, str) and lane is not None:
                instances[instance.lower().removeprefix("0x")] = str(lane)
    counts: dict[str, int] = {}
    if not log_root.is_dir():
        return counts
    for log in soak_logs.load_node_logs(log_root).values():
        for event in log.applied:
            lane = instances.get(event.instance.lower().removeprefix("0x"))
            if lane is not None and since_ms <= event.ts_ms <= until_ms:
                counts[lane] = counts.get(lane, 0) + 1
    return counts


def probe_config(net: GateNetwork, executable: Path, executable_sha256: str, selected: budget.PerRunResourceBudget) -> dict[str, Any]:
    """The owner-only runtime configuration of the resource probe worker."""
    peers = []
    for index in range(net.validators):
        pid = net.peers[index].pid
        if pid is None:
            raise GateError(f"peer{index} is not running")
        peers.append(
            {
                "peer_id": net.node(index),
                "pid": pid,
                "executable_path": str(executable),
                "executable_sha256": executable_sha256,
                "endpoint": f"http://127.0.0.1:{net.api_port(index)}",
                "headers": {},
            }
        )
    return {"schema": PROBE_CONFIG_SCHEMA, "peers": peers, "resource_budget": budget.run_budget_inputs(selected)}


def load_command(
    iroha: Path,
    operator_config: Path,
    options: Options,
    *,
    invocation_id: str,
    pair: int,
    variant: str,
    seed: str,
    accounts: Sequence[Path],
    observer_config: Path,
    trace: Path,
    journal: Path,
    probe_config_path: Path,
    budget_sha256: str,
    capture_dir: Path,
    python: Path,
) -> list[str]:
    """The exact ``iroha tx load`` invocation of one run."""
    command = [
        str(iroha),
        "--config",
        str(operator_config),
        "--fee-payer",
        "authority",
        "tx",
        "load",
        "--invocation-id",
        invocation_id,
        "--pair-index",
        str(pair),
        "--variant",
        variant,
        "--seed",
        seed,
        "--offered-load-tps",
        options.offered_load_tps,
        "--warmup-seconds",
        options.warmup_s,
        "--measurement-seconds",
        options.measurement_s,
        "--drain-seconds",
        options.drain_s,
        "--max-submission-lag-ms",
        options.max_submission_lag_ms,
        "--local-observer-config",
        str(observer_config),
        "--trace-out",
        str(trace),
        "--diagnostic-out",
        str(journal),
        "--resource-program",
        str(python),
        "--resource-worker",
        str(NEXUS_DIR / "resource_probe_worker.py"),
        "--resource-config",
        str(probe_config_path),
        "--resource-budget-sha256",
        budget_sha256,
        "--resource-capture-dir",
        str(capture_dir),
        "--resource-interval-ms",
        str(options.resource_interval_ms),
        "--resource-timeout-ms",
        str(options.resource_timeout_ms),
        "--resource-max-start-lag-ms",
        str(options.resource_max_start_lag_ms),
        "--preparation-lookahead",
        str(options.preparation_lookahead),
        "--preparation-concurrency",
        str(options.preparation_concurrency),
        "--preparation-ahead-ms",
        str(options.preparation_ahead_ms),
        "--max-submissions",
        str(options.max_submissions),
        "--max-in-flight",
        str(options.max_in_flight),
        "--max-status-requests",
        str(options.max_status_requests),
        "--poll-interval-ms",
        str(options.poll_interval_ms),
        "--journal-capacity",
        str(options.journal_capacity),
    ]
    for account in accounts:
        command += ["--account-config", str(account)]
    return command


def parse_load_receipt(stdout: str) -> dict[str, Any]:
    """The terminal receipt ``iroha tx load`` prints after publishing its outputs."""
    lines = [line for line in stdout.splitlines() if line.strip()]
    if not lines:
        raise GateError("transaction load printed no terminal receipt")
    receipt = json.loads(lines[-1])
    if receipt.get("operation") != LOAD_RECEIPT_OPERATION or receipt.get("version") != 1:
        raise GateError("transaction load terminal receipt has an unexpected shape")
    return receipt


def run_directory(out: Path, pair: int, variant: str) -> Path:
    """Evidence directory of one run."""
    return out / "runs" / f"pair{pair}-{variant}"


def run_one(
    out: Path,
    options: Options,
    bins: Mapping[str, Path],
    executable: Path,
    executable_sha256: str,
    ledger: budget.EvidenceBudget,
    run_index: int,
    pair: int,
    variant: str,
) -> dict[str, Any]:
    """Launch one fresh network, collect its load and return its run record."""
    directory = run_directory(out, pair, variant)
    runtime = directory / "runtime"
    runtime.mkdir(parents=True, exist_ok=False)
    seed = pair_seed(options.seed_namespace, pair)
    lanes = LANES[variant]
    base_api = options.base_api_port + 10 * run_index
    base_p2p = options.base_p2p_port + 10 * run_index
    selected = budget.select_run_budget(ledger, pair, variant)
    record: dict[str, Any] = {
        "schema": RUN_SCHEMA,
        "pair_index": pair,
        "variant": variant,
        "lanes_configured": lanes,
        "seed": seed,
        "status": "failed",
        "failure": None,
        "network": {"validators": VALIDATORS, "base_api_port": base_api, "base_p2p_port": base_p2p},
        "started_at_ms": soak.now_ms(),
    }
    run_bins = dict(bins)
    run_bins["iroha3d"] = executable
    net = GateNetwork(runtime, run_bins, VALIDATORS, f"scaling-{options.seed_namespace}-p{pair}", base_api, base_p2p)
    load_window = (0.0, 0.0)
    try:
        ports = (*range(base_api, base_api + VALIDATORS), *range(base_p2p, base_p2p + VALIDATORS))
        busy = [port for port in ports if not soak.port_free(port)]
        if busy:
            raise GateError(f"ports already in use: {busy}")
        net.generate()
        net.patch_configs(None, options.storage_budget_mb * MIB)
        for index in range(VALIDATORS):
            net.start(index)
        soak.wait_for_torii(net, timeout_s=300)
        accounts = net.load_accounts(options.accounts, seed)
        record["funding"] = net.register_and_fund(accounts, options.fund)
        record["accounts"] = [
            {"index": item["index"], "account_id": item["account_id"], "lane": lane_of_account(item["index"], lanes)}
            for item in accounts
        ]
        if lanes > 1:
            parameters = net.chain_parameters()
            lane_params = parameters.get("sumeragi") if isinstance(parameters, dict) else None
            if not isinstance(lane_params, dict):
                raise GateError("the chain's parameters have no sumeragi section")
            policy = lane_policy(
                lane_params, net.da_layout(), net.validator_members(), [item["account_id"] for item in accounts], lanes
            )
            (runtime / "lane-policy.json").write_text(json.dumps(policy, indent=2))
            record["lane_policy_sha256"] = hashlib.sha256(
                json.dumps(policy, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest()
            net.apply_lane_policy(policy)
            net.wait_for_lane_instances(lanes, timeout_s=180)
        record["status_before"] = net.statuses()
        lanes_before = net.lanes()
        record["lanes_before"] = lanes_before
        config_path = runtime / "probe-config.json"
        write_private(config_path, json.dumps(probe_config(net, executable, executable_sha256, selected)))
        invocation_id = secrets.token_hex(32)
        command = load_command(
            net.bins["iroha"],
            net.client_config(0),
            options,
            invocation_id=invocation_id,
            pair=pair,
            variant=variant,
            seed=seed,
            accounts=[item["config"] for item in accounts],
            observer_config=net.client_config(VALIDATORS - 1),
            trace=directory / "trace.json",
            journal=directory / "journal.jsonl",
            probe_config_path=config_path,
            budget_sha256=budget.run_budget_sha256(selected),
            capture_dir=directory / "captures",
            python=Path(sys.executable).resolve(),
        )
        schedule_s = float(
            (options.warmup_ns + options.measurement_ns + 2 * options.drain_ns) / NS
        )
        started = soak.now_ms()
        result = subprocess.run(command, capture_output=True, text=True, check=False, timeout=schedule_s + 600)
        load_window = (started, soak.now_ms())
        (runtime / "load.stderr").write_text(result.stderr)
        record["load"] = {
            "invocation_id": invocation_id,
            "exit_code": result.returncode,
            "stderr_tail": result.stderr.strip()[-2000:],
            "started_at_ms": load_window[0],
            "finished_at_ms": load_window[1],
        }
        if result.returncode == 0:
            record["load"]["receipt"] = parse_load_receipt(result.stdout)
        record["status_after"] = net.statuses()
        lanes_after = net.lanes()
        record["lanes_after"] = lanes_after
        if result.returncode != 0:
            raise GateError(f"transaction load failed ({result.returncode}): {result.stderr.strip()[-600:]}")
        record["status"] = "complete"
    except (GateError, OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        record["failure"] = str(error)[-2000:]
    finally:
        try:
            net.stop_all()
        finally:
            net.kill_everything()
        if not options.keep_state:
            for durable in ("state", "storage"):
                shutil.rmtree(net.net_dir / durable, ignore_errors=True)
    record["finished_at_ms"] = soak.now_ms()
    finish_record(directory, record, options, load_window)
    write_bounded_json(directory / "run.json", record, selected.run.run_receipt.max_bytes)
    return record


def finish_record(directory: Path, record: dict[str, Any], options: Options, load_window: tuple[float, float]) -> None:
    """Add resource, node-counter and lane observations of a finished run to its record."""
    geometry = options.geometry
    captures = directory / "captures"
    manifests = capture_manifests(captures) if captures.is_dir() else []
    record["resources"] = resource_maxima(manifests, geometry.sample_count)
    window_end = geometry.measurement_ns // geometry.interval_ns + 1
    record["node_counters"] = node_counter_window(captures, manifests, (1, window_end)) if manifests else None
    record["global_blocks_committed"] = committed_blocks(record.get("status_before") or {}, record.get("status_after") or {})
    variant = record["variant"]
    applied = (
        applied_lane_blocks(directory / "runtime" / "logs", record.get("lanes_after") or {}, *load_window)
        if LANES[variant] > 1
        else {}
    )
    used, problems = lanes_used(variant, record.get("lanes_before") or {}, record.get("lanes_after") or {}, applied)
    record["lanes"] = {
        "used": used,
        "problems": problems,
        "applied_lane_blocks": applied,
        "frontier_before": lane_frontiers(record.get("lanes_before") or {}),
        "frontier_after": lane_frontiers(record.get("lanes_after") or {}),
    }


# ---------------------------------------------------------------------------------------------
# Plan, verdict and CLI
# ---------------------------------------------------------------------------------------------


def plan_record(options: Options, bins: Mapping[str, Path], digests: Mapping[str, str], ledger: budget.EvidenceBudget) -> dict[str, Any]:
    """The experiment manifest: options, pinned binaries and sources, and the admitted ledger."""
    rate = options.rate
    return {
        "schema": PLAN_SCHEMA,
        "options": dataclasses.asdict(options),
        "schedule": {
            "warmup_requests": cohort_count(options.warmup_ns, rate),
            "measurement_requests": cohort_count(options.measurement_ns, rate),
            "resource_samples": options.geometry.sample_count,
        },
        "run_order": [{"pair_index": pair, "variant": variant} for pair, variant in run_order(options.pairs)],
        "binaries": {name: {"path": str(path), "sha256": digests[name]} for name, path in bins.items()},
        "worker_sources": {name: sha256_file(NEXUS_DIR / name) for name in WORKER_SOURCES},
        "ledger": {
            "total_bytes": ledger.total_bytes,
            "remaining_bytes": ledger.remaining_bytes,
            "resource_bytes_per_run": ledger.resource_bytes_per_run,
            "captures_per_run": ledger.geometry.captures_per_run,
        },
        "thresholds": {
            "min_throughput_ratio": str(MIN_THROUGHPUT_RATIO),
            "max_p95_latency_ratio": str(MAX_P95_LATENCY_RATIO),
            "min_latency_samples": options.min_latency_samples,
        },
        "platform": sys.platform,
    }


def options_from_plan(plan: Mapping[str, Any]) -> Options:
    """The options a finished run recorded in its plan."""
    if plan.get("schema") != PLAN_SCHEMA:
        raise GateError("not a scaling gate plan")
    return Options(**plan["options"])


def judge(out: Path) -> int:
    """Compute the verdict of a finished run directory; write ``verdict.json``."""
    plan = json.loads((out / "plan.json").read_text())
    options = options_from_plan(plan)
    rate = options.rate
    runs: list[dict[str, Any]] = []
    measurements: dict[tuple[int, str], RunMeasurement] = {}
    for pair, variant in run_order(options.pairs):
        directory = run_directory(out, pair, variant)
        path = directory / "run.json"
        if not path.exists():
            continue
        run = json.loads(path.read_text())
        runs.append(run)
        trace_path = directory / "trace.json"
        if run.get("status") != "complete" or not trace_path.exists():
            continue
        try:
            measurements[(pair, variant)] = measure_trace(
                json.loads(trace_path.read_text()),
                pair_index=pair,
                variant=variant,
                seed=pair_seed(options.seed_namespace, pair),
                measurement_ns=options.measurement_ns,
                expected_warmup=cohort_count(options.warmup_ns, rate),
                expected_measurement=cohort_count(options.measurement_ns, rate),
            )
        except (TraceError, ValueError) as error:
            run["status"] = "failed"
            run["failure"] = f"trace rejected: {error}"
    verdict = evaluate(runs, measurements, options.pairs, options.min_latency_samples)
    verdict["plan"] = {"offered_load_tps": options.offered_load_tps, "accounts": options.accounts,
                       "warmup_s": options.warmup_s, "measurement_s": options.measurement_s,
                       "drain_s": options.drain_s, "seed_namespace": options.seed_namespace}
    write_bounded_json(out / "verdict.json", verdict, options.report_bytes)
    summary = {
        "ok": verdict["ok"],
        "release_qualifying": verdict["release_qualifying"],
        "throughput_ratio": (verdict["throughput_ratio"] or {}).get("decimal"),
        "p95_latency_ratio": (verdict["p95_latency_ratio"] or {}).get("decimal"),
        "failures": verdict["failures"],
        "verdict": str(out / "verdict.json"),
    }
    print(json.dumps(summary, indent=2))
    return EXIT_OK if verdict["ok"] else EXIT_FAIL


def build_parser() -> argparse.ArgumentParser:
    """The command line."""
    parser = argparse.ArgumentParser(
        description=__doc__.split("\n\n")[0], formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--analyze", type=Path, metavar="OUT_DIR", help="recompute the verdict of a finished run")
    parser.add_argument("--out", type=Path, help="new run directory (plan, runs, verdict)")
    parser.add_argument("--bin-dir", type=Path, help="directory with iroha3d, kagami and iroha")
    parser.add_argument("--pairs", type=int, default=PAIRS, help="one-lane/four-lane pairs (release: 5)")
    parser.add_argument("--accounts", type=int, default=8, help="load accounts, a multiple of four (default 8)")
    parser.add_argument("--offered-load-tps", default="20", help="fixed offered rate, exact decimal (default 20)")
    parser.add_argument("--warmup", default="10", help="warmup seconds (default 10)")
    parser.add_argument("--measurement", default="40", help="measurement seconds (default 40)")
    parser.add_argument("--drain", default="40", help="drain seconds after warmup and measurement (default 40)")
    parser.add_argument("--max-submission-lag-ms", help="submission lag bound (default: a quarter of the arrival period)")
    parser.add_argument("--resource-interval-ms", type=int, default=2000, help="resource sampling period (default 2000)")
    parser.add_argument("--resource-timeout-ms", type=int, default=1000, help="resource response deadline (default 1000)")
    parser.add_argument("--resource-max-start-lag-ms", type=int, default=500, help="resource start lag bound (default 500)")
    parser.add_argument("--status-body-bytes", type=int, default=64 * 1024, help="per-peer /status capture cap")
    parser.add_argument("--metrics-body-bytes", type=int, default=2 * MIB, help="per-peer /metrics capture cap")
    parser.add_argument("--journal-bytes", type=int, default=64 * MIB, help="collector journal allocation per run")
    parser.add_argument("--trace-bytes", type=int, default=16 * MIB, help="transaction trace allocation per run")
    parser.add_argument("--receipt-bytes", type=int, default=MIB, help="run receipt allocation per run")
    parser.add_argument("--min-latency-samples", type=int, default=MIN_LATENCY_SAMPLES)
    parser.add_argument("--seed-namespace", help="workload seed namespace (default: random, recorded)")
    parser.add_argument("--fund", default="1000000", help="fee asset minted to every load account")
    parser.add_argument("--base-api-port", type=int, default=19080, help="first Torii port (runs use distinct ranges)")
    parser.add_argument("--base-p2p-port", type=int, default=12337, help="first P2P port (runs use distinct ranges)")
    parser.add_argument("--storage-budget-mb", type=int, default=4096, help="nexus.storage.local_budget_bytes per peer")
    parser.add_argument("--preparation-lookahead", type=int, default=256)
    parser.add_argument("--preparation-concurrency", type=int, default=8)
    parser.add_argument("--preparation-ahead-ms", type=int, default=2000)
    parser.add_argument("--max-submissions", type=int, default=256)
    parser.add_argument("--max-in-flight", type=int, default=4096)
    parser.add_argument("--max-status-requests", type=int, default=64)
    parser.add_argument("--poll-interval-ms", type=int, default=100)
    parser.add_argument("--journal-capacity", type=int, default=4096)
    parser.add_argument("--keep-state", action="store_true", help="keep the validators' state directories")
    return parser


def resolve_options(args: argparse.Namespace) -> Options:
    """Options from the command line, with derived defaults, validated."""
    rate = exact_decimal(args.offered_load_tps, "--offered-load-tps")
    if rate <= 0:
        raise ValueError("--offered-load-tps must be positive")
    lag = args.max_submission_lag_ms
    if lag is None:
        lag = decimal_text(Fraction(submission_lag_bound_ns(rate), 1_000_000), 6)
    options = Options(
        pairs=args.pairs,
        accounts=args.accounts,
        offered_load_tps=args.offered_load_tps,
        warmup_s=args.warmup,
        measurement_s=args.measurement,
        drain_s=args.drain,
        max_submission_lag_ms=lag,
        resource_interval_ms=args.resource_interval_ms,
        resource_timeout_ms=args.resource_timeout_ms,
        resource_max_start_lag_ms=args.resource_max_start_lag_ms,
        status_body_bytes=args.status_body_bytes,
        metrics_body_bytes=args.metrics_body_bytes,
        journal_bytes=args.journal_bytes,
        trace_bytes=args.trace_bytes,
        receipt_bytes=args.receipt_bytes,
        manifest_bytes=MIB,
        report_bytes=4 * MIB,
        min_latency_samples=args.min_latency_samples,
        seed_namespace=args.seed_namespace or secrets.token_hex(8),
        fund=args.fund,
        base_api_port=args.base_api_port,
        base_p2p_port=args.base_p2p_port,
        storage_budget_mb=args.storage_budget_mb,
        preparation_lookahead=args.preparation_lookahead,
        preparation_concurrency=args.preparation_concurrency,
        preparation_ahead_ms=args.preparation_ahead_ms,
        max_submissions=args.max_submissions,
        max_in_flight=args.max_in_flight,
        max_status_requests=args.max_status_requests,
        poll_interval_ms=args.poll_interval_ms,
        journal_capacity=args.journal_capacity,
        keep_state=args.keep_state,
    )
    validate_options(options)
    return options


def run_gate(args: argparse.Namespace) -> int:
    """Plan, run every pair and judge."""
    try:
        options = resolve_options(args)
    except ValueError as error:
        print(f"sumeragi scaling gate: {error}", file=sys.stderr)
        return EXIT_HARNESS
    if args.out is None:
        print("sumeragi scaling gate: --out is required", file=sys.stderr)
        return EXIT_HARNESS
    out = args.out.resolve()
    if out.exists() and any(out.iterdir()):
        print(f"sumeragi scaling gate: {out} is not empty", file=sys.stderr)
        return EXIT_HARNESS
    out.mkdir(parents=True, exist_ok=True)
    bins = soak.resolve_bins(args.bin_dir)
    try:
        ledger = evidence_budget(options, worker_static_files())
    except budget.BudgetError as error:
        print(f"sumeragi scaling gate: evidence ledger rejected: {error}", file=sys.stderr)
        return EXIT_HARNESS
    executable = out / "bin" / "iroha3d"
    digests = {"iroha3d": pin_executable(bins["iroha3d"], executable)}
    digests.update({name: sha256_file(path) for name, path in bins.items() if name != "iroha3d"})
    pinned = dict(bins)
    pinned["iroha3d"] = executable
    write_bounded_json(out / "plan.json", plan_record(options, pinned, digests, ledger), options.manifest_bytes)
    print(
        f"sumeragi scaling gate: {options.pairs} pair(s), {options.offered_load_tps} tps offered, "
        f"{options.accounts} accounts, seed namespace {options.seed_namespace}, out {out}"
    )
    for run_index, (pair, variant) in enumerate(run_order(options.pairs)):
        print(f"[{time.strftime('%H:%M:%S')}] pair {pair} {variant}: starting")
        try:
            record = run_one(out, options, bins, executable, digests["iroha3d"], ledger, run_index, pair, variant)
        except GateError as error:
            # The run record itself could not be published; the verdict reports the run as missing.
            print(f"[{time.strftime('%H:%M:%S')}] pair {pair} {variant}: harness error: {error}", file=sys.stderr)
            continue
        print(f"[{time.strftime('%H:%M:%S')}] pair {pair} {variant}: {record['status']} {record.get('failure') or ''}".rstrip())
    return judge(out)


def main(argv: Optional[Sequence[str]] = None) -> int:
    """Entry point."""
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(line_buffering=True)
    args = build_parser().parse_args(list(sys.argv[1:] if argv is None else argv))
    if args.analyze is not None:
        try:
            return judge(args.analyze.resolve())
        except (GateError, OSError, ValueError) as error:
            print(f"sumeragi scaling gate: {error}", file=sys.stderr)
            return EXIT_HARNESS
    return run_gate(args)


if __name__ == "__main__":
    sys.exit(main())
