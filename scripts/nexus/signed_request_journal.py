"""Bounded signed-request retention replay; canonical authentication belongs to Core.

Only a successfully finished reader exposes immutable requests. Digest agreement
proves which bytes the journal retained, not transaction validity, signature
validity, successful fsync, or association with an authenticated block. The
compiled verifier must decode these exact bytes and check those claims.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import re

MAX_REQUEST_BYTES = 1024 * 1024
CHUNK_BYTES = 4096
MAX_REQUESTS = 1_000_000
MAX_ACCOUNTS = 64
MAX_JOURNAL_BYTES = 256 * 1024 * 1024
ENCODING = 'norito.canonical.signed_transaction.v1'
EVENTS = frozenset(('signed_request_begin', 'signed_request_chunk', 'signed_request_retained'))
PLAN_FIELDS = frozenset(('cohort', 'sequence', 'logical_id', 'scheduled_offset_ns', 'account_index'))
_DIGEST = re.compile(r'[0-9a-f]{64}')
_HEX = re.compile(r'[0-9a-f]+')


class SignedRequestError(ValueError):
    """Static fail-closed journal reason without transaction contents."""


def _require(value, code):
    if not value:
        raise SignedRequestError(code)


def _integer(value, minimum=0, maximum=MAX_REQUESTS):
    _require(type(value) is int and minimum <= value <= maximum, 'signed_integer_invalid')
    return value


def _fields(row, names):
    _require(type(row) is dict and row.keys() == set(names), 'signed_fields_invalid')


def _digest(value):
    _require(type(value) is str and _DIGEST.fullmatch(value) is not None, 'signed_digest_invalid')
    return value


def _hash(value):
    value = _digest(value)
    # The SDK's canonical hash text owner requires this Hash::prehashed marker.
    # This is lexical admission only; Core recomputes the signed transaction hash.
    _require(int(value[-1], 16) & 1 == 1, 'signed_hash_invalid')
    return value


@dataclass(frozen=True, slots=True)
class RequestPlan:
    """Exact scheduled request identity, copied without retaining mutable JSON."""
    cohort: str
    sequence: int
    logical_id: str
    scheduled_offset_ns: int
    account_index: int


def _plan(value):
    _fields(value, PLAN_FIELDS)
    _require(type(value['cohort']) is str and value['cohort'] in ('warmup', 'measurement'), 'signed_cohort_invalid')
    return RequestPlan(value['cohort'], _integer(value['sequence'], 1), _digest(value['logical_id']),
                       _integer(value['scheduled_offset_ns'], -(1 << 63), (1 << 63) - 1),
                       _integer(value['account_index'], 0, MAX_ACCOUNTS - 1))


@dataclass(frozen=True, slots=True)
class RetainedRequest:
    """Exact retained bytes for a subsequent compiled canonical/signature join."""
    index: int
    plan: RequestPlan
    hash: str
    canonical_sha256: str
    canonical_bytes: bytes


class SignedRequestReader:
    """One bounded, poison-on-error journal pass; no partial result API.

    Feed every decoded journal row in physical order. ResourceReplay owns strict
    JSON framing, source descriptor identity and the admitted journal byte cap.
    This owner independently caps decoded canonical storage at half that cap.
    A reader is consumed by finish; any failed consume or finish permanently
    invalidates it, including caught exceptions. Concurrent preparations may
    finish out of index order, but a retention command cannot interleave rows.
    """
    def __init__(self, journal_byte_limit: int):
        self._limit = _integer(journal_byte_limit, 1, MAX_JOURNAL_BYTES) // 2
        self._poisoned = False
        self._done = False
        self._started = False
        self._clock = False
        self._finished = False
        self._count = None
        self._plans = []
        self._next_sequence = {'warmup': 1, 'measurement': 1}
        self._measurement_started = False
        self._plan_indexes = {}
        self._logical_ids = set()
        self._identities = set()
        self._requests = {}
        self._hashes = set()
        self._prepared = {}
        self._offers = {}
        self._final = set()
        self._active = None
        self._total = 0
        self._origin = None

    def abort(self):
        """Invalidate the pass when its enclosing source or framing owner fails."""
        self._poisoned = True
        self._active = None
        self._requests.clear()

    def check_interleaving(self, row):
        """Reject another owner's row inside a command before external work."""
        try:
            _require(not self._poisoned and not self._done, 'signed_reader_unavailable')
            _require(type(row) is dict and type(row.get('event')) is str, 'signed_event_invalid')
            if self._active is not None:
                expected = 'signed_request_chunk' if self._active['chunks'] < self._active['count'] else 'signed_request_retained'
                _require(row['event'] == expected, 'signed_command_interleaved')
        except BaseException:
            self.abort()
            raise

    def consume(self, row):
        """Consume one actual journal row without returning retained content."""
        self.check_interleaving(row)
        self._poisoned = True
        try:
            self._consume(row)
        except BaseException:
            self.abort()
            raise
        self._poisoned = False

    def _consume(self, row):
        event = row['event']
        _require(not self._finished, 'signed_after_collection')
        if event == 'plan':
            _require(not self._started, 'signed_plan_order')
            self._count = _integer(row.get('scheduled_requests'), 1)
            self._started = True
            return
        _require(self._started, 'signed_plan_missing')
        if event == 'scheduled':
            _fields(row, ('event', 'index', 'plan'))
            index = _integer(row['index'], 0, self._count - 1)
            _require(not self._clock and index == len(self._plans), 'signed_schedule_order')
            plan = _plan(row['plan'])
            _require(plan.sequence == self._next_sequence[plan.cohort]
                     and (plan.cohort != 'warmup' or not self._measurement_started), 'signed_schedule_cohort_order')
            identity = (plan.cohort, plan.sequence)
            _require(identity not in self._identities and plan.logical_id not in self._logical_ids, 'signed_schedule_duplicate')
            self._next_sequence[plan.cohort] += 1
            self._measurement_started |= plan.cohort == 'measurement'
            self._identities.add(identity)
            self._logical_ids.add(plan.logical_id)
            self._plan_indexes[plan] = index
            self._plans.append(plan)
        elif event == 'clock_started':
            _fields(row, ('event', 'initial_offset_ns'))
            _require(not self._clock and len(self._plans) == self._count, 'signed_clock_before_schedule')
            self._origin = _integer(row['initial_offset_ns'], -(1 << 63), (1 << 63) - 1)
            self._clock = True
        elif event in EVENTS or event in ('prepared', 'offer', 'request_final'):
            _require(self._clock, 'signed_before_clock')
            self._request_event(event, row)
        elif event.startswith('signed_request_'):
            raise SignedRequestError('signed_event_unknown')
        elif event == 'collection_finished':
            _require(self._clock and self._active is None and len(self._requests) == self._count
                     and len(self._prepared) == self._count and len(self._offers) == self._count
                     and len(self._final) == self._count, 'signed_coverage_incomplete')
            _fields(row, ('event', 'passed', 'failure'))
            _require(row['passed'] is True and row['failure'] is None, 'signed_collection_failed')
            self._finished = True

    def _request_event(self, event, row):
        if event == 'request_final':
            # ResourceReplay validates the complete final row and Applied window.
            plan = _plan(row.get('plan'))
            index = self._plan_indexes.get(plan)
            _require(index is not None and index in self._offers and index not in self._final, 'signed_final_order')
            _require(_hash(row.get('hash')) == self._requests[index].hash, 'signed_final_hash_mismatch')
            _require(type(row.get('offer_offset_ns')) is int and row['offer_offset_ns'] == self._offers[index], 'signed_final_offer_mismatch')
            self._final.add(index)
            return
        index = _integer(row.get('index'), 0, self._count - 1)
        if event == 'signed_request_begin':
            _fields(row, ('event', 'index', 'plan', 'hash', 'encoding', 'byte_length', 'canonical_sha256', 'chunk_count'))
            _require(self._active is None and index not in self._requests, 'signed_begin_duplicate')
            _require(_plan(row['plan']) == self._plans[index], 'signed_plan_mismatch')
            _require(type(row['encoding']) is str and row['encoding'] == ENCODING, 'signed_encoding_invalid')
            length = _integer(row['byte_length'], 1, MAX_REQUEST_BYTES)
            count = _integer(row['chunk_count'], 1, MAX_REQUEST_BYTES // CHUNK_BYTES)
            _require(count == (length + CHUNK_BYTES - 1) // CHUNK_BYTES, 'signed_chunk_count_invalid')
            _require(length <= self._limit - self._total, 'signed_cumulative_limit')
            tx_hash, digest = _hash(row['hash']), _digest(row['canonical_sha256'])
            _require(tx_hash not in self._hashes, 'signed_hash_duplicate')
            self._active = dict(index=index, length=length, count=count, chunks=0,
                                hash=tx_hash, digest=digest, body=bytearray())
        elif event == 'signed_request_chunk':
            _fields(row, ('event', 'index', 'chunk_index', 'offset', 'bytes_hex'))
            a = self._active
            _require(a is not None and index == a['index'], 'signed_chunk_owner')
            _require(_integer(row['chunk_index'], 0, 255) == a['chunks']
                     and _integer(row['offset'], 0, MAX_REQUEST_BYTES - 1) == len(a['body']), 'signed_chunk_order')
            length = min(CHUNK_BYTES, a['length'] - len(a['body']))
            value = row['bytes_hex']
            _require(type(value) is str and len(value) == length * 2 and _HEX.fullmatch(value) is not None, 'signed_chunk_bytes_invalid')
            a['body'].extend(bytes.fromhex(value))
            a['chunks'] += 1
        elif event == 'signed_request_retained':
            _fields(row, ('event', 'index', 'hash', 'byte_length', 'canonical_sha256', 'chunk_count'))
            a = self._active
            _require(a is not None and index == a['index'] and a['chunks'] == a['count'], 'signed_terminal_order')
            _require(_integer(row['byte_length'], 1, MAX_REQUEST_BYTES) == a['length']
                     and _integer(row['chunk_count'], 1, 256) == a['count']
                     and _hash(row['hash']) == a['hash'] and _digest(row['canonical_sha256']) == a['digest'], 'signed_terminal_mismatch')
            _require(len(a['body']) == a['length'] and hashlib.sha256(a['body']).hexdigest() == a['digest'], 'signed_content_digest_mismatch')
            request = RetainedRequest(index, self._plans[index], a['hash'], a['digest'], bytes(a['body']))
            self._requests[index] = request
            self._hashes.add(a['hash'])
            self._total += a['length']
            self._active = None
        else:
            _fields(row, ('event', 'index', 'hash', 'offset_ns'))
            _require(index in self._requests and _hash(row['hash']) == self._requests[index].hash, 'signed_ready_hash_mismatch')
            offset = _integer(row['offset_ns'], self._origin, (1 << 63) - 1)
            if event == 'prepared':
                _require(index not in self._prepared, 'signed_prepared_duplicate')
                self._prepared[index] = offset
            else:
                _require(index in self._prepared and index not in self._offers
                         and offset >= self._prepared[index] and offset >= self._plans[index].scheduled_offset_ns, 'signed_offer_order')
                self._offers[index] = offset

    def finish(self) -> tuple[RetainedRequest, ...]:
        """Consume a complete pass and expose requests in exact schedule order."""
        try:
            _require(not self._poisoned and not self._done and self._finished, 'signed_reader_incomplete')
            result = tuple(self._requests[index] for index in range(self._count))
            self._done = True
            self._requests.clear()
            return result
        except BaseException:
            self.abort()
            raise
