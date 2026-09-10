"""Ten-run resource replay under one retained physical bundle admission.

The trusted launcher supplies expected process lifetimes, executable identity,
timing, allocations and control hashes. This owner never learns those expected
values from captures. Keep the context open through transaction, canonical lane
and effect validation, then verify its final census before accepting the bundle.
"""
from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction
import math
import os
from pathlib import Path
import re

from resource_bundle import (
    MAX_ROOT_BYTES, MAX_ROOT_COMPONENTS, BudgetedBundle, ControlBinding,
)
from resource_evidence_budget import EvidenceBudget, select_run_budget
from resource_replay import (
    CaptureReduction, ExpectedPeer, ReplayGeometry, ReplayResult, replay,
    validate_replay_scope,
)

_RUNS = tuple((pair, variant) for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))
_DIGEST = re.compile(r'[0-9a-f]{64}')


class ExperimentError(ValueError):
    """A bounded resource experiment has incomplete or inconsistent authority."""


def _require(condition, code):
    if not condition:
        raise ExperimentError(code)


@dataclass(frozen=True, slots=True)
class RunReplayInput:
    """One launcher-authenticated run scope, never derived from captured rows."""

    pair_index: int
    variant: str
    peers: tuple[ExpectedPeer, ...]
    geometry: ReplayGeometry


@dataclass(frozen=True, slots=True)
class ResourceMaxima:
    """All-peer observed maxima; this is not a continuously measured peak."""

    queue_depth_max: int
    index_entries_max: int
    memory_bytes_max: int
    disk_bytes_max: int


@dataclass(frozen=True, slots=True)
class RunResourceResult:
    """Independently reduced raw observations for one exact pair and variant."""

    pair_index: int
    variant: str
    replay: ReplayResult
    maxima: ResourceMaxima


def _maxima(captures: tuple[CaptureReduction, ...]) -> ResourceMaxima:
    _require(bool(captures), 'resource_window_has_no_observation')
    return ResourceMaxima(
        max(row.queue_size_sum for row in captures),
        max(row.represented_entries for row in captures),
        max(max(row.rss_before_bytes, row.rss_after_bytes) for row in captures),
        max(row.storage_bytes for row in captures),
    )


def interval_maxima(result: RunResourceResult, start_ns: int, end_ns: int) -> ResourceMaxima:
    """Reduce observations whose actual collection brackets overlap a time window.

    Both boundaries are inclusive so a boundary capture is not lost between
    adjacent windows. Empty windows fail; no interpolation or invented sample is
    allowed. Whole-run maxima additionally include the preflight observation.
    """
    _require(type(result) is RunResourceResult and type(start_ns) is int
             and type(end_ns) is int and 0 <= start_ns < end_ns < 1 << 63,
             'resource_window_invalid')
    captures = tuple(row.capture for row in result.replay.samples
                     if row.start_offset_ns <= end_ns and row.end_offset_ns >= start_ns)
    return _maxima(captures)


class ResourceExperiment:
    """Bracket all ten resource replays and the caller's other semantic checks.

    Successful resource replay establishes neither useful lane execution nor
    throughput. Callers must still verify those authorities before ``verify``.
    Failed or repeated replay cannot be promoted by finishing the physical scan.
    """

    def __init__(self, root: Path, budget: EvidenceBudget,
                 controls: tuple[ControlBinding, ...], runs: tuple[RunReplayInput, ...], *,
                 expected_executable_sha256: str, reported: bool):
        self._root = root
        self._budget = budget
        self._controls = controls
        self._runs = runs
        self._expected_executable_sha256 = expected_executable_sha256
        self._reported = reported
        self._attempted = False
        self._failed = False
        self._results = ()
        self._closed = False
        self._bundle = None
        self._scope_pin = self._scope_identity()
        self._bundle = BudgetedBundle(root, budget, controls, reported=reported)

    def _scope_identity(self):
        _require(type(self._root) is type(Path('/')) and self._root.is_absolute()
                 and str(self._root) == os.path.abspath(self._root)
                 and len(os.fsencode(self._root)) <= MAX_ROOT_BYTES
                 and len(self._root.parts) - 1 <= MAX_ROOT_COMPONENTS, 'root_scope_invalid')
        _require(type(self._reported) is bool and type(self._controls) is tuple
                 and len(self._controls) <= 256
                 and all(type(item) is ControlBinding for item in self._controls), 'control_scope_invalid')
        for item in self._controls:
            item.__post_init__()
        _require(type(self._expected_executable_sha256) is str
                 and _DIGEST.fullmatch(self._expected_executable_sha256), 'executable_digest_invalid')
        _require(type(self._runs) is tuple and len(self._runs) == 10
                 and all(type(row) is RunReplayInput for row in self._runs), 'exact_ten_run_scopes_required')
        _require(tuple((row.pair_index, row.variant) for row in self._runs) == _RUNS,
                 'run_scope_order_invalid')
        first = None
        peer_labels = None
        for row in self._runs:
            allocation = select_run_budget(self._budget, row.pair_index, row.variant)
            validate_replay_scope(row.peers, row.geometry,
                                  expected_policy=allocation.policy, allocation=allocation)
            labels = tuple(peer.peer_id for peer in row.peers)
            _require(all(peer.identity.executable_sha256 == self._expected_executable_sha256
                         for peer in row.peers), 'run_executable_scope_mismatch')
            if first is None:
                first, peer_labels = row.geometry, labels
            else:
                _require(row.geometry == first and labels == peer_labels, 'experiment_scope_mismatch')
        # Scope objects are type/size checked above before their immutable pin.
        # Budget/control/root pins are independently retained by BudgetedBundle.
        return repr((self._runs, self._expected_executable_sha256, allocation.experiment,
                     self._controls, self._reported, str(self._root)))

    def _validate_scope(self):
        _require(not self._closed, 'experiment_closed')
        _require(not self._failed, 'experiment_failed')
        _require(self._scope_identity() == self._scope_pin, 'experiment_scope_changed')

    def collect_replay(self) -> tuple[RunResourceResult, ...]:
        """Recompute every raw resource observation under its own expected lifetime."""
        try:
            self._validate_scope()
            _require(not self._attempted, 'resource_replay_already_attempted')
            self._attempted = True
            by_label = {item.label: item for item in self._controls}
            results = []
            for row in self._runs:
                self._validate_scope()
                allocation = select_run_budget(self._budget, row.pair_index, row.variant)
                journal = by_label[allocation.journal.label]
                captures = self._root / 'resources' / f'pair-{row.pair_index:02}' / row.variant
                reduced = replay(captures, self._root / journal.path, journal.sha256,
                                 row.peers, row.geometry, expected_policy=allocation.policy,
                                 allocation=allocation)
                maxima = _maxima((reduced.preflight, *(item.capture for item in reduced.samples)))
                results.append(RunResourceResult(row.pair_index, row.variant, reduced, maxima))
            self._validate_scope()
            self._results = tuple(results)
            return self._results
        except BaseException:
            # A caught error or interrupted replay cannot later publish a prefix
            # or previously successful result through a fresh physical scan.
            self._failed = True
            self._results = ()
            raise

    def verify(self) -> tuple[RunResourceResult, ...]:
        """Require ten successful replays and reject physical or scope changes.

        Any failed check permanently invalidates this context. A caller must
        open a new context and repeat all semantic checks after repairing input.
        """
        try:
            self._validate_scope()
            _require(len(self._results) == 10, 'resource_replay_incomplete')
            self._bundle.verify()
            self._validate_scope()
            return self._results
        except BaseException:
            self._failed = True
            self._results = ()
            raise

    def reconcile_run(self, pair_index: int, variant: str, raw: dict,
                      budgets: dict) -> ResourceMaxima:
        """Reconcile one report inside its originating retained experiment scope.

        Geometry and replay results come only from this context. Any rejection
        permanently poisons the context, including a caught or interrupted check.
        Reconciliation must precede final verification and context closure.
        """
        try:
            self._validate_scope()
            _require(len(self._results) == 10, 'resource_replay_incomplete')
            select_run_budget(self._budget, pair_index, variant)
            index = _RUNS.index((pair_index, variant))
            value = _reconcile_run_resources(self._results[index], self._runs[index].geometry,
                                             raw, budgets)
            self._validate_scope()
            return value
        except BaseException:
            self._failed = True
            self._results = ()
            raise

    def read_control(self, binding: ControlBinding, *, max_bytes: int) -> bytes:
        """Read one admitted control while retaining the full experiment scope.

        The semantic parser supplies its independent byte cap. The exact
        original binding and physical identities are enforced by the retained
        bundle reader; failures invalidate every prior replay result.
        """
        try:
            self._validate_scope()
            value = self._bundle.read_control(binding, max_bytes=max_bytes)
            self._validate_scope()
            return value
        except BaseException:
            self._failed = True
            self._results = ()
            raise


    def close(self):
        """Release only retained reader descriptors; preserve every evidence file."""
        if not self._closed:
            self._closed = True
            if self._bundle is not None:
                self._bundle.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


def _report_offset_ns(value):
    """Decode an exact, bounded report interval without rounding nanoseconds."""
    _require(type(value) in (int, float) and (type(value) is int or math.isfinite(value)),
             'report_offset_invalid')
    # Bound integer text conversion before parsing the decimal representation.
    _require(0 <= value < 1 << 63, 'report_offset_invalid')
    scaled = Fraction(str(value)) * 1_000_000_000
    _require(scaled.denominator == 1 and 0 <= scaled.numerator < 1 << 63,
             'report_offset_invalid')
    return scaled.numerator


def _reported_maxima(value):
    """Read the four exact resource integers; other fields belong to the trace owner."""
    _require(type(value) is dict, 'reported_resources_invalid')
    fields = ('queue_depth_max', 'index_entries_max', 'memory_bytes_max', 'disk_bytes_max')
    _require(all(name in value and type(value[name]) is int
                 and 0 <= value[name] <= 1 << 53 for name in fields), 'reported_resources_invalid')
    return ResourceMaxima(*(value[name] for name in fields))


def _reconcile_run_resources(result: RunResourceResult, geometry: ReplayGeometry,
                             raw: dict, budgets: dict) -> ResourceMaxima:
    """Require report values to equal actual all-peer capture reductions.

    Internal reduction for ``ResourceExperiment.reconcile_run``, which owns the
    exact geometry and result. It does not replace canonical routing, transaction
    or effect validation. A summary never substitutes for raw observations.

    Measurement and drain intervals use inclusive capture-bracket overlap. Both
    endpoints must be represented and every interval must cover at most one
    declared sampling period. Whole-run budgets additionally include preflight
    and the last collection tail; this is an observed maximum, not a continuous
    process or disk peak. Caller-owned report dictionaries are never modified.
    """
    _require(type(result) is RunResourceResult and type(geometry) is ReplayGeometry,
             'resource_reconciliation_scope_invalid')
    geometry.validate()
    _require(type(raw) is dict and type(raw.get('pair_index')) is int
             and raw['pair_index'] == result.pair_index and raw.get('variant') == result.variant,
             'resource_report_run_mismatch')
    limits = _reported_maxima(budgets)
    _require(all(value > 0 for value in (limits.queue_depth_max, limits.index_entries_max,
                                        limits.memory_bytes_max, limits.disk_bytes_max)),
             'resource_budget_invalid')
    _require(type(result.replay) is ReplayResult and type(result.replay.samples) is tuple
             and len(result.replay.samples) == geometry.samples, 'resource_replay_geometry_mismatch')
    _require(tuple(row.scheduled_offset_ns for row in result.replay.samples)
             == tuple(index * geometry.interval_ns for index in range(geometry.samples)),
             'resource_replay_geometry_mismatch')
    # Replay already enforces these bounds. Recheck the geometry used by the
    # cursor so no malformed internal bracket can cause overlapping full scans.
    for row in result.replay.samples:
        _require(row.scheduled_offset_ns <= row.start_offset_ns
                 <= row.scheduled_offset_ns + geometry.max_start_lag_ns
                 and row.start_offset_ns <= row.end_offset_ns
                 < row.start_offset_ns + geometry.response_deadline_ns,
                 'resource_replay_bracket_invalid')
    cursor = 0
    samples = result.replay.samples
    for phase, begin, finish in (
            (raw, 0, geometry.measurement_ns),
            (raw.get('drain'), geometry.measurement_ns, geometry.final)):
        _require(type(phase) is dict and type(phase.get('samples')) is list,
                 'resource_report_phase_invalid')
        rows = phase['samples']
        _require(0 < len(rows) <= 100_000, 'resource_report_sample_count_invalid')
        previous = begin
        summary = ResourceMaxima(0, 0, 0, 0)
        for index, row in enumerate(rows, 1):
            _require(type(row) is dict and type(row.get('sequence')) is int
                     and row['sequence'] == index, 'resource_report_sequence_invalid')
            start = _report_offset_ns(row.get('start_offset_seconds'))
            end = _report_offset_ns(row.get('end_offset_seconds'))
            _require(start == previous and 0 < end - start <= geometry.interval_ns
                     and end <= finish, 'resource_report_interval_invalid')
            previous = end
            while cursor < len(samples) and samples[cursor].end_offset_ns < start:
                cursor += 1
            overlaps = []
            position = cursor
            while position < len(samples) and samples[position].start_offset_ns <= end:
                overlaps.append(samples[position].capture)
                position += 1
            # Ordered disjoint capture brackets and an interval no wider than
            # one sampling period admit at most two overlaps. Cursor advances
            # once per capture; total work is linear in captures plus report rows.
            _require(len(overlaps) <= 2, 'resource_replay_overlap_invalid')
            observed = _maxima(tuple(overlaps))
            declared = _reported_maxima({name + '_max': row.get(name) for name in
                ('queue_depth', 'index_entries', 'memory_bytes', 'disk_bytes')})
            _require(declared == observed, 'resource_report_observation_mismatch')
            summary = ResourceMaxima(*(max(getattr(summary, field), getattr(observed, field))
                for field in ('queue_depth_max', 'index_entries_max', 'memory_bytes_max', 'disk_bytes_max')))
        _require(previous == finish, 'resource_report_phase_incomplete')
        _require(_reported_maxima(phase.get('summary')) == summary,
                 'resource_report_summary_mismatch')
    observed = _maxima((result.replay.preflight, *(row.capture for row in result.replay.samples)))
    _require(observed == result.maxima, 'resource_replay_maxima_mismatch')
    _require(all(getattr(observed, name) <= getattr(limits, name) for name in
                 ('queue_depth_max', 'index_entries_max', 'memory_bytes_max', 'disk_bytes_max')),
             'resource_observation_exceeds_budget')
    return observed
