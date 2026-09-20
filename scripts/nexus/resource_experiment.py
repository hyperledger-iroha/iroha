"""Borrow ten resource replays from the original completed fixed experiment.

Only the originating FixedExperimentCustody creates and registers this reader.
Original physical files, controls, deadlines and native proof authority remain
with that owner. This borrower never creates a replacement bundle admission and
retains no signed request body after a run joins its original native authority.
"""
from __future__ import annotations

from dataclasses import dataclass, fields
from fractions import Fraction
import math
import os
from pathlib import Path
import re

from resource_bundle import MAX_ROOT_BYTES, MAX_ROOT_COMPONENTS
from resource_evidence_budget import (
    PerRunResourceBudget, canonical_run_budget_bytes, parse_run_budget,
    run_budget_inputs, select_run_budget,
)
from resource_process import ProcessIdentity
from resource_replay import (
    CaptureReduction, ExpectedPeer, ReplayGeometry, ReplayResult, replay,
    validate_replay_scope,
)
from scaling_completed_authority import (
    PublicRunProjection, ResourceSnapshot, _Freeze, _PUBLIC_RECORDS, _RESOURCE_FIELDS,
)

_RUNS = tuple((pair, variant) for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))
_DIGEST = re.compile(r'[0-9a-f]{64}')


class ExperimentError(ValueError):
    """An original resource borrower failed; a fresh reader cannot rescue it."""


def _require(condition, code):
    if not condition:
        raise ExperimentError(code)


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise ExperimentError('resource_experiment_failed') from None


@dataclass(frozen=True, slots=True)
class RunReplayScope:
    """One exact original completed run, supplied only by its fixed owner."""
    pair_index: int
    variant: str
    capture_directory: Path
    journal_path: Path
    journal_sha256: str
    peers: tuple[ExpectedPeer, ...]
    geometry: ReplayGeometry
    allocation: PerRunResourceBudget


@dataclass(frozen=True, slots=True)
class RunResourceResult:
    """Immutable resource reductions joined to the original native proof."""
    pair_index: int
    variant: str
    resources: ResourceSnapshot
    maxima: ResourceMaxima


def _path(value):
    _require(type(value) is type(Path('/')) and value.is_absolute()
             and str(value) == os.path.abspath(value)
             and len(os.fsencode(value)) <= MAX_ROOT_BYTES
             and len(value.parts) - 1 <= MAX_ROOT_COMPONENTS, 'resource_path_invalid')
    return str(value)


def _same_public(value, expected):
    """Compare only the exact bounded primitive tree of a trusted projection."""
    if type(value) is not type(expected): return False
    if type(expected) in (int, str, bytes): return value == expected
    if isinstance(expected, tuple):
        return len(value) == len(expected) and all(_same_public(a, b) for a, b in zip(value, expected, strict=True))
    return False


def _resource_snapshot(reduced):
    _require(type(reduced) is ReplayResult and type(reduced.samples) is tuple
             and len(reduced.samples) <= 100_000, 'resource_result_invalid')
    freeze = _Freeze(0, len(reduced.samples), 1)
    return ResourceSnapshot(*(freeze(getattr(reduced, name)) for name in _RESOURCE_FIELDS))


class ResourceExperiment:
    """Single-use borrower; completed native authority stays with its origin."""
    def __init__(self, *args, **kwargs):
        raise ExperimentError('completed_experiment_required')

    @classmethod
    def from_completed(cls, owner):
        """Ask only the exact fixed owner to create and register its borrower."""
        from scaling_experiment_custody import FixedExperimentCustody
        value = None
        try:
            _require(cls is ResourceExperiment and type(owner) is FixedExperimentCustody,
                     'completed_experiment_required')
            value = owner._create_replay()
            _require(type(value) is ResourceExperiment, 'original_borrower_required')
            value._validate(('admitted',))
            _require(value._owner is owner, 'original_borrower_required')
            return value
        except BaseException as error:
            if type(owner) is FixedExperimentCustody:
                try: owner._reject_replay(value, getattr(value, '_token', None))
                except BaseException: pass
            _failure(error)

    def _initialize(self, owner, token, scopes):
        """Pure internal initialization; only the owner can register this instance."""
        from scaling_experiment_custody import FixedExperimentCustody
        try:
            _require(type(self) is ResourceExperiment and not hasattr(self, '_origin')
                     and type(owner) is FixedExperimentCustody and type(token) is object,
                     'original_borrower_required')
            self._origin = (owner, token)
            self._owner, self._token = owner, token
            self._scopes = scopes
            initial = self._scope_identity()
            self._scopes = tuple(RunReplayScope(row.pair_index, row.variant,
                Path(str(row.capture_directory)), Path(str(row.journal_path)), row.journal_sha256,
                tuple(ExpectedPeer(peer.peer_id, ProcessIdentity(**{field.name: getattr(peer.identity, field.name)
                    for field in fields(ProcessIdentity)})) for peer in row.peers),
                ReplayGeometry(**{field.name: getattr(row.geometry, field.name) for field in fields(ReplayGeometry)}),
                parse_run_budget(run_budget_inputs(row.allocation))) for row in scopes)
            self._scope_pin = self._scope_identity()
            _require(initial == self._scope_pin, 'resource_scope_changed')
            self._phase, self._busy = 'admitted', False
            self._results, self._results_pin = (), ()
        except BaseException as error:
            self._reject(error)

    def _scope_identity(self):
        """Pure bounded current scope values, for both borrower and origin guards."""
        _require(type(self._scopes) is tuple and len(self._scopes) == 10
                 and all(type(row) is RunReplayScope for row in self._scopes), 'exact_ten_run_scopes_required')
        values = []
        common = None
        for index, row in enumerate(self._scopes):
            pair, variant = _RUNS[index]
            _require(type(row.pair_index) is int and type(row.variant) is str
                     and (row.pair_index, row.variant) == (pair, variant), 'run_scope_order_invalid')
            capture, journal = _path(row.capture_directory), _path(row.journal_path)
            _require(type(row.journal_sha256) is str and _DIGEST.fullmatch(row.journal_sha256), 'journal_digest_invalid')
            _require(type(row.allocation) is PerRunResourceBudget, 'resource_allocation_invalid')
            validate_replay_scope(row.peers, row.geometry, expected_policy=row.allocation.policy, allocation=row.allocation)
            _require((row.allocation.run.pair_index, row.allocation.run.variant) == (pair, variant), 'resource_allocation_invalid')
            _require(row.capture_directory.parts[-3:] == ('resources', f'pair-{pair:02}', variant)
                     and row.journal_path.parts[-4:] == ('runs', f'pair-{pair:02}', variant, 'collector.jsonl')
                     and row.capture_directory.parents[2] == row.journal_path.parents[3], 'resource_fixed_paths_required')
            peers = tuple((peer.peer_id, tuple(getattr(peer.identity, field.name) for field in fields(ProcessIdentity)))
                          for peer in row.peers)
            geometry = tuple(getattr(row.geometry, field.name) for field in fields(ReplayGeometry))
            allocation = canonical_run_budget_bytes(row.allocation)
            shared = (str(row.capture_directory.parents[2]), geometry, tuple(peer.peer_id for peer in row.peers),
                      tuple(peer.identity.executable_sha256 for peer in row.peers),
                      canonical_run_budget_bytes(select_run_budget(row.allocation.experiment, 1, 'one_lane')))
            if common is None: common = shared
            _require(shared == common and len(set(shared[3])) == 1, 'experiment_scope_mismatch')
            values.append((pair, variant, capture, journal, row.journal_sha256, peers, geometry, allocation))
        return tuple(values)

    def _validate(self, phases):
        _require(type(self) is ResourceExperiment and self._owner is self._origin[0]
                 and self._token is self._origin[1] and type(self._token) is object
                 and type(self._phase) is str and self._phase in phases
                 and self._scope_identity() == self._scope_pin,
                 'resource_borrower_invalid')
        if self._phase in ('finishing', 'complete'):
            _require(type(self._results_pin) is tuple and len(self._results_pin) == 10
                     and self._result_identity() == self._results_pin, 'resource_results_changed')
        else:
            _require(type(self._results) is tuple and self._results == ()
                     and type(self._results_pin) is tuple and self._results_pin == (),
                     'resource_results_changed')

    def _result_identity(self):
        _require(type(self._results) is tuple and len(self._results) == 10, 'resource_replay_incomplete')
        values = []
        for index, row in enumerate(self._results):
            _require(type(row) is RunResourceResult and type(row.pair_index) is int
                     and type(row.variant) is str and (row.pair_index, row.variant) == _RUNS[index]
                     and type(row.resources) is ResourceSnapshot and type(row.maxima) is ResourceMaxima,
                     'resource_result_invalid')
            maxima = tuple(getattr(row.maxima, field.name) for field in fields(ResourceMaxima))
            _require(all(type(value) is int and 0 <= value < 1 << 128 for value in maxima), 'resource_result_invalid')
            if self._results_pin:
                _require(_same_public(row.resources, self._results_pin[index][2]), 'resource_results_changed')
            values.append((row.pair_index, row.variant, row.resources, maxima))
        return tuple(values)

    def _reject(self, error):
        self._phase, self._busy = 'failed', False
        self._results, self._results_pin = (), ()
        origin = getattr(self, '_origin', None)
        if origin is not None and not getattr(self, '_rejecting', False):
            self._rejecting = True
            try: origin[0]._reject_replay(self, origin[1])
            except BaseException: pass
            finally: self._rejecting = False
        self._phase, self._busy = 'failed', False
        self._results, self._results_pin = (), ()
        _failure(error)

    def collect_replay(self) -> tuple[RunResourceResult, ...]:
        """Replay each original scope and discard bodies after its native join."""
        reduced = projected = None
        try:
            self._validate(('admitted',)); _require(not self._busy, 'resource_replay_reentrant')
            self._phase, self._busy = 'replaying', True
            self._owner._replay_before(self, self._token)
            results = []
            for index, row in enumerate(self._scopes):
                self._validate(('replaying',)); _require(self._busy, 'resource_replay_reentrant')
                reduced = replay(row.capture_directory, row.journal_path, row.journal_sha256,
                                 row.peers, row.geometry, expected_policy=row.allocation.policy, allocation=row.allocation)
                resources = _resource_snapshot(reduced)
                maxima = _maxima((reduced.preflight, *(item.capture for item in reduced.samples)))
                projected = self._owner._replay_accept(self, self._token, index, reduced)
                _require(type(projected) is PublicRunProjection and type(projected.pair_index) is int
                         and type(projected.variant) is str and (projected.pair_index, projected.variant) == (row.pair_index, row.variant)
                         and type(projected.budget) is bytes and projected.budget == self._scope_pin[index][7]
                         and _same_public(projected.geometry, _PUBLIC_RECORDS[ReplayGeometry](*self._scope_pin[index][6]))
                         and _same_public(projected.resources, resources), 'resource_authority_join_invalid')
                self._validate(('replaying',)); _require(self._busy, 'resource_replay_reentrant')
                results.append(RunResourceResult(row.pair_index, row.variant, projected.resources, maxima))
                reduced = projected = None
            self._results = tuple(results)
            self._results_pin = self._result_identity()
            self._phase = 'finishing'
            self._owner._replay_finish(self, self._token, self._results)
            self._validate(('finishing',)); _require(self._busy, 'resource_replay_reentrant')
            self._phase, self._busy = 'complete', False
            return self._results
        except BaseException as error:
            reduced = projected = None
            self._reject(error)

    def verify(self) -> tuple[RunResourceResult, ...]:
        """Verify through the same original experiment; never reopen a bundle."""
        try:
            self._validate(('complete',)); _require(not self._busy, 'resource_replay_reentrant')
            self._busy = True
            self._owner._verify_replay(self, self._token)
            self._validate(('complete',)); _require(self._busy, 'resource_replay_reentrant')
            self._busy = False
            return self._results
        except BaseException as error: self._reject(error)

    def close(self):
        """Release this borrower once; close none of the original FD owners."""
        if getattr(self, '_phase', None) == 'closed': return
        try:
            self._validate(('admitted', 'complete')); _require(not self._busy, 'resource_replay_reentrant')
            self._busy = True
            self._owner._release_replay(self, self._token)
            self._validate(('admitted', 'complete')); _require(self._busy, 'resource_replay_reentrant')
            self._phase, self._busy = 'closed', False
        except BaseException as error: self._reject(error)

    def __enter__(self):
        try: self._validate(('admitted', 'complete')); return self
        except BaseException as error: self._reject(error)

    def __exit__(self, kind, error, traceback):
        if error is not None: self._reject(error)
        self.close()


@dataclass(frozen=True, slots=True)
class ResourceMaxima:
    """All-peer observed maxima; this is not a continuously measured peak."""

    queue_depth_max: int
    index_entries_max: int
    memory_bytes_max: int
    disk_bytes_max: int


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
    captures = tuple(row.capture for row in result.resources.samples
                     if row.start_offset_ns <= end_ns and row.end_offset_ns >= start_ns)
    return _maxima(captures)


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

    Pure reduction for the originating experiment report owner, which supplies
    the exact retained geometry and joined immutable resource result. It does not replace canonical routing, transaction
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
    _require(type(result.resources) is ResourceSnapshot and type(result.resources.samples) is tuple
             and len(result.resources.samples) == geometry.samples, 'resource_replay_geometry_mismatch')
    _require(tuple(row.scheduled_offset_ns for row in result.resources.samples)
             == tuple(index * geometry.interval_ns for index in range(geometry.samples)),
             'resource_replay_geometry_mismatch')
    # Replay already enforces these bounds. Recheck the geometry used by the
    # cursor so no malformed internal bracket can cause overlapping full scans.
    for row in result.resources.samples:
        _require(row.scheduled_offset_ns <= row.start_offset_ns
                 <= row.scheduled_offset_ns + geometry.max_start_lag_ns
                 and row.start_offset_ns <= row.end_offset_ns
                 < row.start_offset_ns + geometry.response_deadline_ns,
                 'resource_replay_bracket_invalid')
    cursor = 0
    samples = result.resources.samples
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
    observed = _maxima((result.resources.preflight, *(row.capture for row in result.resources.samples)))
    _require(observed == result.maxima, 'resource_replay_maxima_mismatch')
    _require(all(getattr(observed, name) <= getattr(limits, name) for name in
                 ('queue_depth_max', 'index_entries_max', 'memory_bytes_max', 'disk_bytes_max')),
             'resource_observation_exceeds_budget')
    return observed
