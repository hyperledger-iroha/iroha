"""Bounded public manifest and observed measurements, without release authority.

Original descriptor custody, complete native proof verification and runtime
qualification belong to the fixed experiment owner. These pure projections do
not accept private paths, runtime inputs or caller-supplied qualification flags.
"""
from dataclasses import dataclass, fields
from fractions import Fraction
import hashlib
import json
import re

from resource_bundle import ControlBinding
from resource_evidence_budget import (EvidenceBudget, CapturePolicy, CaptureGeometry,
    RunBudget, FileBudget, StaticFile, RUN_FILE_FIELDS, MAX_FILE_BYTES, select_run_budget, run_budget_inputs)
from resource_experiment import ResourceMaxima
from scaling_experiment_plan import ExperimentPlan, RUN_KEYS, admit_plan
from scaling_measurements import ExperimentMeasurements, RunMeasurements, MAX_REQUESTS
from scaling_public_files import PublicFile, _PATHS
from scaling_trial_captures import CaptureCensus

MANIFEST_SCHEMA = 'iroha.sumeragi_v2.multilane_scaling.manifest.v1'
REPORT_SCHEMA = 'iroha.sumeragi_v2.multilane_scaling.report.v1'
MAX_PROJECTION_BYTES = 1024 * 1024
_RESOURCE_FIELDS = tuple(field.name for field in fields(ResourceMaxima))
_RUN_FIELDS = tuple(field.name for field in fields(RunMeasurements))
_MEASUREMENT_FIELDS = tuple(field.name for field in fields(ExperimentMeasurements))
_BUDGET_RECORDS = (CapturePolicy, CaptureGeometry, RunBudget, FileBudget, StaticFile)
_ENCODER = json.JSONEncoder(sort_keys=True, ensure_ascii=True, allow_nan=False, separators=(',', ':'))


class FinalProjectionError(ValueError):
    """Closed projection failure with no source contents or runtime credentials."""


def _require(condition):
    if not condition:
        raise FinalProjectionError('scaling_final_projection_invalid')


def _integer(value, low=0, high=(1 << 63) - 1):
    _require(type(value) is int and low <= value <= high)
    return value


def _digest(value):
    _require(type(value) is str and len(value) == 64 and re.fullmatch('[a-f0-9]{64}', value))
    return value


def _cap(value):
    return _integer(value, 1, MAX_FILE_BYTES)


def _encode(value, cap):
    # Count the exact encoding before allocating the complete byte result.
    # Every tree supplied here is fixed and bounded, with no caller callbacks.
    maximum = min(cap, MAX_PROJECTION_BYTES)
    size = 0
    for part in _ENCODER.iterencode(value):
        size += len(part)
        _require(size <= maximum)
    return ''.join(_ENCODER.iterencode(value)).encode('ascii')


def _record_size(value):
    """Precharge canonical budget copies using only the closed budget classes."""
    if type(value) is int:
        _integer(value, 0, (1 << 64) - 1)
        return len(str(value))
    if type(value) is str:
        _require(len(value) <= 128)
        return len(_ENCODER.encode(value))
    _require(type(value) in _BUDGET_RECORDS)
    members = fields(type(value))
    return 2 + max(0, len(members) - 1) + sum(
        len(_ENCODER.encode(field.name)) + 1 + _record_size(getattr(value, field.name))
        for field in members)


def _budget_precharge(value, cap):
    # select_run_budget revalidates types/counts/derived totals before this walk.
    # The sole canonical budget projector can then allocate its bounded copies.
    total = _record_size(value.policy)
    for group in (value.runs, value.static_files, value.control_budgets):
        total += 2 + max(0, len(group) - 1) + sum(_record_size(item) for item in group)
    _require(total <= min(cap, MAX_PROJECTION_BYTES))


@dataclass(frozen=True, slots=True)
class RunManifest:
    """Original public bindings and capture census, never a completed-run token."""
    pair_index: int
    variant: str
    original_deadline_ns: int
    files: tuple[PublicFile, ...]
    census: CaptureCensus


def _binding(value, label, path):
    _require(type(value) is ControlBinding and type(value.label) is str
             and type(value.path) is str and value.label == label and value.path == path)
    return {'label': label, 'path': path, 'sha256': _digest(value.sha256)}


def manifest_bytes(plan: ExperimentPlan, budget: EvidenceBudget, original_deadline_ns: int,
                   inputs: tuple[ControlBinding, ...], runs: tuple[RunManifest, ...], cap: int) -> bytes:
    """Describe exactly ten original completed runs in the admitted public layout.

    Deadlines are immutable recorded scopes, not renewed or checked against a
    new clock. The caller performs all live deadline and custody checks.
    """
    try:
        cap = _cap(cap)
        _integer(original_deadline_ns, 1)
        _require(type(plan) is ExperimentPlan and type(budget) is EvidenceBudget
                 and type(inputs) is tuple and len(inputs) == 3
                 and type(runs) is tuple and len(runs) == len(RUN_KEYS))
        selected = select_run_budget(budget, 1, 'one_lane')
        _budget_precharge(selected.experiment, cap)
        plan, budget, raw_plan = admit_plan(plan, selected.experiment)
        _require(cap <= budget.control_budgets[0].max_bytes)
        started = original_deadline_ns - plan.experiment_timeout_ns
        _require(started >= 0)
        plan_sha = hashlib.sha256(raw_plan).hexdigest()
        bindings = []
        for expected, value in zip(('identity', 'plan', 'source_closure'), inputs, strict=True):
            binding = _binding(value, expected, f'inputs/{expected}.json')
            if expected == 'plan':
                _require(binding['sha256'] == plan_sha)
            bindings.append(binding)
        records = []
        previous = started
        for expected, value in zip(RUN_KEYS, runs, strict=True):
            _require(type(value) is RunManifest and type(value.pair_index) is int
                     and type(value.variant) is str and (value.pair_index, value.variant) == expected
                     and type(value.files) is tuple and len(value.files) == len(RUN_FILE_FIELDS)
                     and type(value.census) is CaptureCensus)
            deadline = _integer(value.original_deadline_ns, started + plan.trial_timeout_ns, original_deadline_ns - 1)
            _require(deadline > previous)
            previous = deadline
            allocation = select_run_budget(budget, *expected)
            pair, variant = expected
            prefix = f'runs/pair-{pair:02}/{variant}'
            public = []
            for role, item in zip(RUN_FILE_FIELDS, value.files, strict=True):
                _require(type(item) is PublicFile and type(item.role) is str and item.role == role)
                bound = getattr(allocation.run, role)
                binding = _binding(item.binding, bound.label, f'{prefix}/{_PATHS[role]}')
                _require(_integer(item.max_bytes, 1) == bound.max_bytes)
                public.append({'role': role, **binding, 'bytes': _integer(item.bytes, 1, item.max_bytes),
                               'max_bytes': item.max_bytes})
            census = value.census
            _require(_integer(census.files, 1) == allocation.member_count)
            captures = {'directory': f'resources/pair-{pair:02}/{variant}', 'files': census.files,
                        'bytes': _integer(census.bytes, 1, allocation.resource_bytes),
                        'census_sha256': _digest(census.census_sha256),
                        'metadata_sha256': _digest(census.metadata_sha256)}
            records.append({'pair_index': pair, 'variant': variant, 'original_deadline_ns': deadline,
                            'files': public, 'captures': captures})
        canonical_budget = run_budget_inputs(select_run_budget(budget, 1, 'one_lane'))['experiment']
        return _encode({'schema': MANIFEST_SCHEMA, 'original_deadline_ns': original_deadline_ns,
                        'plan_sha256': plan_sha, 'budget': canonical_budget,
                        'inputs': bindings, 'runs': records}, cap)
    except Exception:
        raise FinalProjectionError('scaling_final_projection_invalid') from None


def _fraction(value, positive=False):
    _require(type(value) is Fraction)
    numerator = _integer(value.numerator, 1 if positive else 0, (1 << 256) - 1)
    denominator = _integer(value.denominator, 1, (1 << 256) - 1)
    # A Fraction can be changed with object.__setattr__; retain canonical exact
    # primitives, and reject invalid/non-reduced internals before any arithmetic.
    normalized = Fraction(numerator, denominator)
    _require(normalized.numerator == numerator and normalized.denominator == denominator)
    return numerator, denominator


def _resources(value):
    _require(type(value) is ResourceMaxima)
    return tuple(_integer(getattr(value, name), 0, 1 << 53) for name in _RESOURCE_FIELDS)


def _run_identity(value, expected):
    _require(type(value) is RunMeasurements and type(value.pair_index) is int
             and type(value.variant) is str and (value.pair_index, value.variant) == expected)
    offered = _fraction(value.offered_load_tps, True)
    warmup = _integer(value.warmup_requests, 0, MAX_REQUESTS)
    warmup_p95 = value.warmup_p95_latency_ns
    if warmup == 0:
        _require(warmup_p95 is None)
    else:
        _integer(warmup_p95, 1)
    count = _integer(value.measurement_requests, 100, MAX_REQUESTS)
    _require(warmup + count <= MAX_REQUESTS)
    committed = _integer(value.measurement_committed, 0, count)
    drain = _integer(value.drain_committed, 0, count)
    _require(committed + drain == count)
    throughput = _fraction(value.committed_throughput_tps)
    _require((committed == 0) == (throughput[0] == 0))
    p95 = _integer(value.p95_latency_ns, 1)
    latencies = value.latencies_ns
    _require(type(latencies) is tuple and len(latencies) == count)
    for latency in latencies:
        _integer(latency, 1)
    resources = _resources(value.observed_resources)
    _require(type(value.observed_resource_limits_met) is bool)
    # Reuse only an exact immutable tuple of exact integers; no large copy and
    # no reference to a mutable record/Fraction remains in the returned pin.
    return (*expected, offered, warmup, warmup_p95, count, committed, drain,
            throughput, p95, latencies, resources, value.observed_resource_limits_met)


def measurement_identity(value: ExperimentMeasurements) -> tuple:
    """Snapshot every measurement field into bounded immutable primitives.

    This validates the closed projection shape, not the provenance or derivation
    of caller-constructible measurements. The outer owner pins the actual result
    of measure_experiment and compares this identity before publication.
    """
    try:
        _require(type(value) is ExperimentMeasurements and type(value.runs) is tuple
                 and len(value.runs) == len(RUN_KEYS))
        runs = tuple(_run_identity(row, expected) for row, expected in zip(value.runs, RUN_KEYS, strict=True))
        _require(all(sum(row[5] for row in runs[offset::2]) <= 5 * MAX_REQUESTS for offset in (0, 1)))
        one = _fraction(value.one_lane_median_throughput_tps, True)
        four = _fraction(value.four_lane_median_throughput_tps)
        throughput_ratio = _fraction(value.median_throughput_ratio)
        one_p95 = _integer(value.one_lane_pooled_p95_latency_ns, 1)
        four_p95 = _integer(value.four_lane_pooled_p95_latency_ns, 1)
        latency_ratio = _fraction(value.pooled_p95_latency_ratio, True)
        flags = (value.throughput_criterion_met, value.latency_criterion_met, value.observed_resource_criterion_met)
        _require(all(type(flag) is bool for flag in flags))
        return (runs, one, four, throughput_ratio, one_p95, four_p95, latency_ratio, *flags,
                _resources(value.one_lane_observed_resources), _resources(value.four_lane_observed_resources))
    except Exception:
        raise FinalProjectionError('scaling_final_projection_invalid') from None


def _rational(value):
    return {'numerator': value[0], 'denominator': value[1]}


def _resource_object(value):
    return dict(zip(_RESOURCE_FIELDS, value, strict=True))


def report_bytes(manifest_sha256: str, measurements: ExperimentMeasurements, cap: int) -> bytes:
    """Publish bounded observations; never publish a combined PASS or runtime claim."""
    try:
        cap = _cap(cap)
        manifest_sha256 = _digest(manifest_sha256)
        pin = measurement_identity(measurements)
        # All latency arrays remain solely in the private immutable pin. Public
        # reports have exactly ten small records regardless of cohort length.
        runs = []
        for row in pin[0]:
            values = dict(zip(_RUN_FIELDS, row, strict=True))
            del values['latencies_ns']
            values['offered_load_tps'] = _rational(values['offered_load_tps'])
            values['committed_throughput_tps'] = _rational(values['committed_throughput_tps'])
            values['observed_resources'] = _resource_object(values['observed_resources'])
            runs.append(values)
        values = dict(zip(_MEASUREMENT_FIELDS, pin, strict=True))
        values['runs'] = runs
        for name in ('one_lane_median_throughput_tps', 'four_lane_median_throughput_tps',
                     'median_throughput_ratio', 'pooled_p95_latency_ratio'):
            values[name] = _rational(values[name])
        for name in ('one_lane_observed_resources', 'four_lane_observed_resources'):
            values[name] = _resource_object(values[name])
        return _encode({'schema': REPORT_SCHEMA, 'scope': 'observed_measurements',
                        'manifest_sha256': manifest_sha256,
                        'criteria': {'minimum_median_throughput_ratio': _rational((3, 2)),
                                     'maximum_pooled_p95_latency_ratio': _rational((5, 4)),
                                     'minimum_latency_samples_per_run': 100}, **values}, cap)
    except Exception:
        raise FinalProjectionError('scaling_final_projection_invalid') from None
