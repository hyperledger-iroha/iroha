"""Exact measurements from public snapshots, without authority or release verdicts.

Accepts explicit measurement data or completed public run projections.
Both use the same arithmetic; neither input form establishes provenance.
This pure module performs no I/O, native execution, cryptographic verification
or quota enforcement. Its criterion booleans describe supplied observations;
the original custody/native/resource owners must establish their provenance.
"""
from dataclasses import dataclass, fields
from fractions import Fraction
import hashlib
import re

from resource_experiment import ResourceMaxima, _maxima
from resource_process import ProcessIdentity
from resource_replay import Bracket, CaptureReduction, ReplayGeometry
from scaling_completed_authority import PublicRequest, PublicRunProjection, ResourceSnapshot, _PUBLIC_RECORDS
from scaling_experiment_plan import ExperimentPlan, ResourceLimits, RUN_KEYS, _RECORDS, plan_bytes
from scaling_fixed_trial import TrialPlan
from scaling_generator import GenerationReceipt, GeneratorPlan
from scaling_native_facts_inputs import FactsJournalPlan, journal_snapshot
from scaling_native_load import NativeLoadPlan, NativeLoadReceipt
from scaling_readiness_inputs import GeneratedAccount
from signed_request_journal import MAX_REQUEST_BYTES

NS = 1_000_000_000
MAX_REQUESTS = 64 * 1024
MAX_SAMPLES = 100_000
MIN_LATENCY_SAMPLES = 100
MIN_THROUGHPUT_RATIO = Fraction(3, 2)
MAX_P95_LATENCY_RATIO = Fraction(5, 4)
_RESOURCE_NAMES = tuple(field.name for field in fields(ResourceMaxima))


class MeasurementError(ValueError):
    """Closed measurement-input error, without input contents or authority claims."""


def _require(condition):
    if not condition:
        raise MeasurementError('scaling_measurements_invalid')


def _integer(value, low=0, high=(1 << 63) - 1):
    _require(type(value) is int and low <= value <= high)
    return value


def _digest(value, marked=False):
    _require(type(value) is str and re.fullmatch('[a-f0-9]{64}', value))
    _require(not marked or int(value[-1], 16) & 1 == 1)


def _policy(value, record, snapshot=True):
    _require(type(value) is (_PUBLIC_RECORDS[record] if snapshot else record))
    values = tuple(getattr(value, field.name) for field in fields(record))
    for item in values:
        _require((type(item) is int and -(1 << 127) < item < 1 << 128)
                 or (type(item) is str and 0 < len(item) <= 4096))
    return values


def _trial(value, snapshot=True):
    _require(type(value) is (_PUBLIC_RECORDS[TrialPlan] if snapshot else TrialPlan))
    policies = tuple(_policy(getattr(value, name), record, snapshot) for name, record in _RECORDS)
    return (*policies, _integer(value.replay_reply_max_bytes, 1, 256 * 1024 * 1024),
            _integer(value.stop_timeout_ns, 1, 300 * NS))


def _accounts(value):
    _require(type(value) is _PUBLIC_RECORDS[GenerationReceipt]
             and type(value.accounts) is tuple and 4 <= len(value.accounts) <= 64)
    result = []
    for index, item in enumerate(value.accounts):
        _require(type(item) is _PUBLIC_RECORDS[GeneratedAccount]
                 and type(item.index) is int and item.index == index
                 and type(item.account_id) is str and 0 < len(item.account_id) <= 2048
                 and all(33 <= ord(char) <= 126 for char in item.account_id))
        result.append(item.account_id)
    _require(len(result) % 4 == 0 and len(set(result)) == len(result))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class MeasurementRun:
    """Only the data consumed by arithmetic; no custody or execution capability.

    Historical PIDs are distinct cohort labels here. No kernel, current clock,
    private account configuration, original descriptor or receipt is required.
    Native proof verification and execution attribution are separate obligations.
    """
    pair_index: int
    variant: str
    plan: TrialPlan
    accounts: tuple[str, ...]
    peer_pids: tuple[int, ...]
    geometry: ReplayGeometry
    preflight: CaptureReduction
    samples: tuple[Bracket, ...]
    requests: tuple[PublicRequest, ...]


def _account_ids(value):
    _require(type(value) is tuple and 4 <= len(value) <= 64 and len(value) % 4 == 0)
    _require(all(type(item) is str and 0 < len(item) <= 2048
                 and all(33 <= ord(char) <= 126 for char in item) for item in value))
    _require(len(set(value)) == len(value))
    return value


def measurement_data(value: PublicRunProjection) -> MeasurementRun:
    """Project existing public snapshots after retaining every prior data join.

    The live custody owner still establishes provenance before calling its
    measurement API. This projection cannot mint or reconstruct that owner.
    """
    try:
        _require(type(value) is PublicRunProjection)
        pin = _trial(value.plan)
        accounts = _accounts(value.generation)
        _require(_policy(value.generation.plan, GeneratorPlan) == pin[0]
                 and type(value.load) is _PUBLIC_RECORDS[NativeLoadReceipt]
                 and _policy(value.load.plan, NativeLoadPlan) == pin[1])
        declared = TrialPlan(*(record(*part) for (_, record), part in zip(_RECORDS, pin[:6], strict=True)), *pin[6:])
        _require(type(value.peers) is tuple and len(value.peers) == 4
                 and all(type(peer) is _PUBLIC_RECORDS[ProcessIdentity] for peer in value.peers))
        _require(type(value.geometry) is _PUBLIC_RECORDS[ReplayGeometry]
                 and type(value.resources) is ResourceSnapshot)
        _require(type(value.resources.samples) is tuple and len(value.resources.samples) <= MAX_SAMPLES)
        def capture(item):
            _require(type(item) is _PUBLIC_RECORDS[CaptureReduction])
            return CaptureReduction(*item)
        rows = []
        for item in value.resources.samples:
            _require(type(item) is _PUBLIC_RECORDS[Bracket])
            rows.append(Bracket(item.scheduled_offset_ns, item.start_offset_ns,
                                item.end_offset_ns, capture(item.capture)))
        return MeasurementRun(value.pair_index, value.variant, declared, accounts,
            tuple(peer.pid for peer in value.peers), ReplayGeometry(*value.geometry),
            capture(value.resources.preflight), tuple(rows), value.requests)
    except Exception:
        raise MeasurementError('scaling_measurements_invalid') from None


def _schedule(value):
    _require(type(value) is MeasurementRun and type(value.requests) is tuple
             and 0 < len(value.requests) <= MAX_REQUESTS)
    pin = _trial(value.plan, False)
    generator, load = GeneratorPlan(*pin[0]), NativeLoadPlan(*pin[1])
    generator.validate()
    _require(type(value.pair_index) is int and type(value.variant) is str
             and (value.pair_index, value.variant) in RUN_KEYS
             and (load.pair_index, load.variant) == (value.pair_index, value.variant)
             and generator.lane_count == (1 if value.variant == 'one_lane' else 4))
    accounts = _account_ids(value.accounts)
    _require(len(accounts) == generator.account_count)
    _require(len(load.offered_load_tps) <= 64
             and re.fullmatch(r'(?:0|[1-9][0-9]*)(?:\.[0-9]+)?', load.offered_load_tps))
    rate = Fraction(load.offered_load_tps)
    _integer(load.journal_capacity, 1, 16384)
    declared = FactsJournalPlan(load.seed, load.pair_index, rate.numerator, rate.denominator,
        load.warmup_ns, load.measurement_ns, load.drain_ns, load.submission_lag_ns,
        load.preparation_lookahead, load.preparation_concurrency, load.preparation_ahead_ms * 1_000_000,
        load.max_submissions, load.max_in_flight, load.max_status_requests, load.poll_interval_ms * 1_000_000,
        len(value.requests), load.resource_interval_ms * 1_000_000, load.resource_timeout_ms * 1_000_000,
        load.resource_max_start_lag_ms * 1_000_000)
    _, counts = journal_snapshot(declared, accounts)
    _require(sum(counts) == len(value.requests)
             and (max(counts) - 1) * NS * rate.denominator <= (1 << 128) - 1)
    return pin, load, accounts, counts, rate


def _capture(value):
    _require(type(value) is CaptureReduction)
    for field in fields(CaptureReduction):
        _integer(getattr(value, field.name), 0, 1 << 53)
    _require(value.rss_before_bytes > 0 and value.rss_after_bytes > 0
             and value.queue_size_max <= value.queue_size_sum <= 4 * value.queue_size_max)
    return value


def _resources(value, load):
    _require(type(value.peer_pids) is tuple and len(value.peer_pids) == 4)
    _require(len({_integer(pid, 1) for pid in value.peer_pids}) == 4)
    _require(type(value.geometry) is ReplayGeometry)
    numbers = tuple(_integer(getattr(value.geometry, field.name)) for field in fields(ReplayGeometry))
    geometry = ReplayGeometry(*numbers)
    geometry.validate()
    _require(numbers == (load.warmup_ns, load.measurement_ns, load.drain_ns,
        load.preparation_ahead_ms * 1_000_000, load.resource_interval_ms * 1_000_000,
        load.resource_timeout_ms * 1_000_000, load.resource_max_start_lag_ms * 1_000_000))
    resources = value
    _require(type(resources.samples) is tuple and len(resources.samples) == geometry.samples <= MAX_SAMPLES)
    captures = [_capture(resources.preflight)]
    _require(captures[0].sequence == 0)
    for index, row in enumerate(resources.samples):
        _require(type(row) is Bracket)
        scheduled = _integer(row.scheduled_offset_ns)
        start, end = _integer(row.start_offset_ns), _integer(row.end_offset_ns)
        deadline = min(start + geometry.response_deadline_ns, geometry.final + geometry.response_deadline_ns)
        _require(scheduled == index * geometry.interval_ns
                 and scheduled <= start <= scheduled + geometry.max_start_lag_ns
                 and start <= end < deadline)
        capture = _capture(row.capture)
        _require(capture.sequence == index + 1)
        captures.append(capture)
    # This is the canonical raw-resource reduction, including preflight and the
    # final sampling tail. These values are observed maxima, not live quotas.
    return _maxima(tuple(captures))


def _p95(values):
    _require(type(values) is tuple and 0 < len(values) <= 5 * MAX_REQUESTS)
    _require(all(type(value) is int and 0 < value < 1 << 63 for value in values))
    return sorted(values)[(95 * len(values) + 99) // 100 - 1]


@dataclass(frozen=True, slots=True)
class RunMeasurements:
    """Complete cohort measurements; booleans compare observed samples only."""
    pair_index: int
    variant: str
    offered_load_tps: Fraction
    warmup_requests: int
    warmup_p95_latency_ns: int | None
    measurement_requests: int
    measurement_committed: int
    drain_committed: int
    committed_throughput_tps: Fraction
    p95_latency_ns: int
    latencies_ns: tuple[int, ...]
    observed_resources: ResourceMaxima
    observed_resource_limits_met: bool


def measure_data_run(value: MeasurementRun, limits: ResourceLimits) -> RunMeasurements:
    """Measure every scheduled request; reject missing, reordered or late cohorts."""
    try:
        _require(type(limits) is ResourceLimits)
        limits.validate()
        _, load, accounts, counts, rate = _schedule(value)
        _require(counts[1] >= limits.min_latency_samples >= MIN_LATENCY_SAMPLES)
        period = NS * rate.denominator
        rotation = int.from_bytes(hashlib.sha256(
            f'gscale-account-offset-v1:{load.seed}'.encode('ascii')).digest()[:8], 'little') % len(accounts)
        latencies, warmup_latencies, seen, committed = [], [], set(), 0
        for index, row in enumerate(value.requests):
            _require(type(row) is PublicRequest and type(row.index) is int and row.index == index)
            warmup = index < counts[0]
            ordinal = index if warmup else index - counts[0]
            cohort = 'warmup' if warmup else 'measurement'
            start = -(load.warmup_ns + load.drain_ns) if warmup else 0
            scheduled = start + ordinal * period // rate.numerator
            expected = (cohort, ordinal + 1, hashlib.sha256(
                f'{load.seed}:{cohort}:{ordinal + 1}'.encode('ascii')).hexdigest(), scheduled,
                (ordinal + rotation) % len(accounts))
            actual = (row.cohort, row.sequence, row.logical_id, row.scheduled_offset_ns, row.account_index)
            _require(all(type(left) is type(right) and left == right
                         for left, right in zip(actual, expected, strict=True)))
            _digest(row.transaction_hash, marked=True); _digest(row.canonical_sha256)
            _require(row.transaction_hash not in seen); seen.add(row.transaction_hash)
            _integer(row.canonical_size_bytes, 1, MAX_REQUEST_BYTES)
            phase_end = -load.drain_ns if warmup else load.measurement_ns
            offer = _integer(row.offer_offset_ns, scheduled, min(scheduled + load.submission_lag_ns, phase_end - 1))
            deadline = -1 if warmup else load.measurement_ns + load.drain_ns
            _integer(row.acknowledgment_offset_ns, offer, deadline)
            applied = _integer(row.applied_offset_ns, offer + 1, deadline)
            _integer(row.local_applied_offset_ns, offer + 1, deadline)
            height = _integer(row.block_height, 1, (1 << 64) - 1)
            _require(_integer(row.local_block_height, 1, (1 << 64) - 1) == height)
            _integer(row.status_attempts, 1, (1 << 64) - 1)
            _integer(row.local_status_attempts, 1, (1 << 64) - 1)
            (warmup_latencies if warmup else latencies).append(applied - offer)
            if not warmup and applied < load.measurement_ns:
                committed += 1
        observed = _resources(value, load)
        within_limits = all(getattr(observed, name) <= getattr(limits, name) for name in _RESOURCE_NAMES)
        latency_values = tuple(latencies)
        return RunMeasurements(value.pair_index, value.variant, rate, counts[0],
            _p95(tuple(warmup_latencies)) if warmup_latencies else None, counts[1], committed,
            counts[1] - committed, Fraction(committed * NS, load.measurement_ns),
            _p95(latency_values), latency_values, observed, within_limits)
    except Exception:
        raise MeasurementError('scaling_measurements_invalid') from None


@dataclass(frozen=True, slots=True)
class ExperimentMeasurements:
    """Exact five-pair comparisons, with no combined PASS or authority token."""
    runs: tuple[RunMeasurements, ...]
    one_lane_median_throughput_tps: Fraction
    four_lane_median_throughput_tps: Fraction
    median_throughput_ratio: Fraction
    one_lane_pooled_p95_latency_ns: int
    four_lane_pooled_p95_latency_ns: int
    pooled_p95_latency_ratio: Fraction
    throughput_criterion_met: bool
    latency_criterion_met: bool
    observed_resource_criterion_met: bool
    one_lane_observed_resources: ResourceMaxima
    four_lane_observed_resources: ResourceMaxima


def measure_data_experiment(plan: ExperimentPlan, values: tuple[MeasurementRun, ...]) -> ExperimentMeasurements:
    """Compare exactly five matched pairs using medians and complete pooled p95."""
    try:
        plan_bytes(plan)  # Bound and type-check every original declared scalar first.
        _require(type(values) is tuple and len(values) == len(RUN_KEYS))
        normalized, measurements, hashes, previous_accounts = None, [], set(), None
        for key, declared, value in zip(RUN_KEYS, plan.trials, values, strict=True):
            _require(type(value) is MeasurementRun and (value.pair_index, value.variant) == key)
            actual, expected = _trial(value.plan, False), _trial(declared, False)
            _require(actual == expected)
            pair, variant = key
            _require(declared.load.seed == hashlib.sha256(f'{plan.seed_namespace}:{pair}'.encode('ascii')).hexdigest())
            gen = tuple(1 if field.name == 'lane_count' else item for field, item in zip(fields(GeneratorPlan), actual[0], strict=True))
            common = {'pair_index': 1, 'variant': 'one_lane', 'seed': '0' * 64}
            load = tuple(common.get(field.name, item) for field, item in zip(fields(NativeLoadPlan), actual[1], strict=True))
            comparable = (gen, load, *actual[2:])
            if normalized is None:
                normalized = comparable
            _require(comparable == normalized)
            accounts = _account_ids(value.accounts)
            if variant == 'one_lane':
                previous_accounts = accounts
            else:
                _require(accounts == previous_accounts)
            measured = measure_data_run(value, plan.resource_limits)
            for row in value.requests:
                _require(row.transaction_hash not in hashes); hashes.add(row.transaction_hash)
            measurements.append(measured)
        one, four = tuple(measurements[::2]), tuple(measurements[1::2])
        one_median = sorted(row.committed_throughput_tps for row in one)[2]
        four_median = sorted(row.committed_throughput_tps for row in four)[2]
        _require(one_median > 0)
        throughput_ratio = four_median / one_median
        one_p95 = _p95(tuple(value for row in one for value in row.latencies_ns))
        four_p95 = _p95(tuple(value for row in four for value in row.latencies_ns))
        latency_ratio = Fraction(four_p95, one_p95)
        def resources(rows):
            return ResourceMaxima(*(max(getattr(row.observed_resources, name) for row in rows) for name in _RESOURCE_NAMES))
        return ExperimentMeasurements(tuple(measurements), one_median, four_median, throughput_ratio,
            one_p95, four_p95, latency_ratio, throughput_ratio >= MIN_THROUGHPUT_RATIO,
            latency_ratio <= MAX_P95_LATENCY_RATIO,
            all(row.observed_resource_limits_met for row in measurements), resources(one), resources(four))
    except Exception:
        raise MeasurementError('scaling_measurements_invalid') from None


def measure_run(value: PublicRunProjection, limits: ResourceLimits) -> RunMeasurements:
    """Measure one original public projection using the canonical data reduction."""
    return measure_data_run(measurement_data(value), limits)


def measure_experiment(plan: ExperimentPlan, values: tuple[PublicRunProjection, ...]) -> ExperimentMeasurements:
    """Measure original public projections; preserve the existing live API joins."""
    try:
        _require(type(values) is tuple and len(values) == len(RUN_KEYS))
        return measure_data_experiment(plan, tuple(measurement_data(value) for value in values))
    except Exception:
        raise MeasurementError('scaling_measurements_invalid') from None
