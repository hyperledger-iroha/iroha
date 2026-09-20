"""Replay exact global and peer-local Applied slots from a bounded CLI journal.

Callers supply the independent measurement-plus-drain deadline and feed every
decoded row in order from their bounded, retained journal owner. This library
performs no I/O and proves journal agreement only. Canonical authentication and
the peer's actual state remain the compiled verifier's responsibility.
"""
from __future__ import annotations

from dataclasses import dataclass, field
import re

MAX_REQUESTS = 1_000_000
MAX_I64 = (1 << 63) - 1
MAX_U64 = (1 << 64) - 1
FINAL_FIELDS = frozenset(('event', 'plan', 'hash', 'offer_offset_ns',
    'acknowledgment_offset_ns', 'applied_offset_ns', 'block_height', 'status_attempts',
    'local_applied_offset_ns', 'local_block_height', 'local_status_attempts',
    'submission_finished', 'failure'))
OBSERVATION_EVENTS = frozenset(('status', 'status_missing', 'local_status', 'local_status_missing'))
_DIGEST = re.compile(r'[0-9a-f]{64}')


class AppliedJournalError(ValueError):
    """Static fail-closed reason without remote or signed-request content."""


def _require(value, code):
    if not value:
        raise AppliedJournalError(code)


def _integer(value, minimum=0, maximum=MAX_I64):
    _require(type(value) is int and minimum <= value <= maximum, 'applied_integer_invalid')
    return value


def _fields(row, names):
    _require(type(row) is dict and row.keys() == set(names), 'applied_fields_invalid')


def _digest(value, *, transaction=False):
    _require(type(value) is str and _DIGEST.fullmatch(value) is not None, 'applied_hash_invalid')
    if transaction:
        _require(int(value[-1], 16) & 1 == 1, 'applied_hash_invalid')
    return value


def _plan(row):
    _fields(row, ('cohort', 'sequence', 'logical_id', 'scheduled_offset_ns', 'account_index'))
    cohort = row['cohort']
    _require(type(cohort) is str and cohort in ('warmup', 'measurement'), 'applied_cohort_invalid')
    return (cohort, _integer(row['sequence'], 1, MAX_REQUESTS), _digest(row['logical_id']),
            _integer(row['scheduled_offset_ns'], -(1 << 63)), _integer(row['account_index'], 0, 63))


@dataclass(slots=True)
class _Scope:
    attempts: int = 0
    last_offset: int | None = None
    applied: tuple[int, int] | None = None


@dataclass(slots=True)
class _Request:
    hash: str
    offer: int
    acknowledgment: int | None = None
    global_scope: _Scope = field(default_factory=_Scope)
    local_scope: _Scope = field(default_factory=_Scope)


@dataclass(frozen=True, slots=True)
class RetainedApplication:
    """One complete journal observation, awaiting compiled proof authentication.

    The index identifies the original scheduled request. Both heights and all
    offsets come from independent observed events checked against its final row;
    the final row cannot supply an absent observation.
    """
    index: int
    hash: str
    offer_offset_ns: int
    acknowledgment_offset_ns: int
    applied_offset_ns: int
    block_height: int
    status_attempts: int
    local_applied_offset_ns: int
    local_block_height: int
    local_status_attempts: int


class AppliedRequestReader:
    """One bounded pass with two independent terminal slots per original hash.

Only per-request identities, counters and terminal observations are retained;
status history is not accumulated. Global, local and acknowledgment completion
may occur in any order. Each status scope must stop after its first StateApplied.
Missing and cache/queue observations count as pending attempts, never success.
Only a successful single-use finish exposes the complete immutable observations.
"""

    def __init__(self, measurement_deadline_ns: int):
        self._deadline = _integer(measurement_deadline_ns, 1)
        self._count = None
        self._poll_ns = None
        self._plans = []
        self._indexes = {}
        self._requests = {}
        self._hashes = set()
        self._clock = self._resource_finished = self._finished = self._poisoned = False
        self._postconditions = False
        self._final = 0
        self._done = False

    def consume(self, row):
        """Validate one row; a caught failure still makes this reader unusable."""
        available = not self._poisoned and not self._finished
        self._poisoned = True
        _require(available, 'applied_reader_unavailable')
        self._consume(row)
        self._poisoned = False

    def _consume(self, row):
        _require(type(row) is dict and type(row.get('event')) is str, 'applied_event_invalid')
        event = row['event']
        if event == 'plan':
            _require(self._count is None and row.get('local_applied_required') is True,
                     'applied_local_requirement_missing')
            self._count = _integer(row.get('scheduled_requests'), 1, MAX_REQUESTS)
            self._poll_ns = _integer(row.get('poll_interval_ns'), 1)
            return
        _require(self._count is not None, 'applied_plan_missing')
        if event == 'scheduled':
            _fields(row, ('event', 'index', 'plan'))
            index = _integer(row['index'], 0, self._count - 1)
            plan = _plan(row['plan'])
            _require(not self._clock and index == len(self._plans) and plan not in self._indexes,
                     'applied_schedule_invalid')
            self._indexes[plan] = index
            self._plans.append(plan)
        elif event == 'clock_started':
            _require(not self._clock and len(self._plans) == self._count, 'applied_clock_invalid')
            self._clock = True
        elif event == 'resource_collection_finished':
            _require(self._clock and not self._resource_finished, 'applied_resource_finish_invalid')
            self._resource_finished = True
        elif event == 'workload_postconditions_started':
            _fields(row, ('event',))
            _require(self._resource_finished and not self._postconditions, 'applied_postconditions_order')
            self._postconditions = True
        elif event in ('offer', 'accepted') or event in OBSERVATION_EVENTS:
            # Sampling and transactions are concurrent. A completion at exact
            # T+D may be recorded after resource finish; its own timestamp
            # remains decisive. Postconditions start only after both drain.
            _require(self._clock and not self._postconditions and self._final == 0, 'applied_observation_order')
            index = _integer(row.get('index'), 0, self._count - 1)
            offset = _integer(row.get('offset_ns'), -(1 << 63))
            if event == 'offer':
                _require(not self._resource_finished, 'applied_offer_after_resource_finish')
                _fields(row, ('event', 'index', 'hash', 'offset_ns'))
                tx_hash = _digest(row['hash'], transaction=True)
                _require(index not in self._requests and tx_hash not in self._hashes
                         and offset >= self._plans[index][3], 'applied_offer_invalid')
                self._requests[index] = _Request(tx_hash, offset)
                self._hashes.add(tx_hash)
                return
            _require(index in self._requests, 'applied_offer_missing')
            request = self._requests[index]
            _require(offset >= request.offer, 'applied_before_offer')
            if event == 'accepted':
                _fields(row, ('event', 'index', 'hash', 'offset_ns'))
                _require(_digest(row['hash'], transaction=True) == request.hash
                         and request.acknowledgment is None, 'applied_acknowledgment_invalid')
                self._window(index, offset)
                request.acknowledgment = offset
            else:
                self._observe(index, request, event, row, offset)
        elif event == 'request_final':
            self._finalize(row)
        elif event == 'collection_finished':
            _fields(row, ('event', 'passed', 'failure'))
            _require(self._resource_finished and self._final == self._count
                     and row['passed'] is True and row['failure'] is None, 'applied_coverage_incomplete')
            self._finished = True
        elif event.startswith('local_status'):
            raise AppliedJournalError('applied_event_unknown')

    def _window(self, index, offset):
        _require(offset < 0 if self._plans[index][0] == 'warmup' else 0 <= offset <= self._deadline,
                 'applied_transaction_drain_extended')

    def _observe(self, index, request, event, row, offset):
        local = event.startswith('local_')
        scope = request.local_scope if local else request.global_scope
        _require(scope.applied is None and (scope.last_offset is None or offset >= scope.last_offset + self._poll_ns),
                 'applied_scope_order_invalid')
        scope.attempts += 1
        _integer(scope.attempts, 1, MAX_U64)
        scope.last_offset = offset
        if event.endswith('_missing'):
            _fields(row, ('event', 'index', 'offset_ns', 'hash'))
            _require(_digest(row['hash'], transaction=True) == request.hash, 'applied_observation_hash_mismatch')
            return
        scope_field = 'local_scope_matches' if local else 'global_scope_matches'
        _fields(row, ('event', 'index', 'offset_ns', 'expected_hash', 'hash_matches',
                      scope_field, 'resolved_from', 'status', 'block_height'))
        _require(_digest(row['expected_hash'], transaction=True) == request.hash
                 and row['hash_matches'] is True and row[scope_field] is True,
                 'applied_observation_identity_mismatch')
        source, status, height = row['resolved_from'], row['status'], row['block_height']
        _require(type(source) is str and source in ('state', 'cache', 'queue')
                 and type(status) is str and status in ('Queued', 'Approved', 'Committed', 'Applied', 'Rejected', 'Expired'),
                 'applied_status_unknown')
        if height is not None:
            _integer(height, 0, MAX_U64)
        if source == 'state' and status == 'Applied':
            height = _integer(height, 1, MAX_U64)
            _require(offset > request.offer, 'applied_not_after_offer')
            self._window(index, offset)
            scope.applied = (offset, height)
            other = request.global_scope if local else request.local_scope
            _require(other.applied is None or other.applied[1] == height, 'applied_scope_height_mismatch')
        else:
            _require(source != 'state' or status not in ('Rejected', 'Expired'), 'applied_state_terminal_failure')

    def _finalize(self, row):
        _fields(row, FINAL_FIELDS)
        index = self._indexes.get(_plan(row['plan']))
        _require(self._resource_finished and index == self._final and index in self._requests,
                 'applied_final_order_invalid')
        request = self._requests[index]
        _require(request.acknowledgment is not None and request.global_scope.applied is not None
                 and request.local_scope.applied is not None, 'applied_local_or_global_missing')
        _require(_digest(row['hash'], transaction=True) == request.hash
                 and row['submission_finished'] is True and row['failure'] is None, 'applied_final_invalid')
        expected = {'offer_offset_ns': request.offer, 'acknowledgment_offset_ns': request.acknowledgment,
                    'applied_offset_ns': request.global_scope.applied[0], 'block_height': request.global_scope.applied[1],
                    'status_attempts': request.global_scope.attempts,
                    'local_applied_offset_ns': request.local_scope.applied[0],
                    'local_block_height': request.local_scope.applied[1], 'local_status_attempts': request.local_scope.attempts}
        _require(all(type(row[name]) is int and row[name] == value for name, value in expected.items()),
                 'applied_final_summary_mismatch')
        self._final += 1

    def finish(self) -> tuple[RetainedApplication, ...]:
        """Consume the complete pass and return observations in scheduled order.

        Failure, including an early or repeated finish, permanently invalidates
        the reader. No successful prefix is available before collection finishes.
        """
        available = not self._poisoned and not self._done and self._finished
        self._poisoned = True
        _require(available, 'applied_reader_incomplete')
        observations = tuple(
            RetainedApplication(
                index, request.hash, request.offer, request.acknowledgment,
                request.global_scope.applied[0], request.global_scope.applied[1],
                request.global_scope.attempts, request.local_scope.applied[0],
                request.local_scope.applied[1], request.local_scope.attempts,
            )
            for index in range(self._count)
            for request in (self._requests[index],)
        )
        self._done = True
        self._poisoned = False
        return observations
