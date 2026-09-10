"""Independently replay raw resource captures against CLI clock and peer owners.

This library validates resource evidence only. It never samples processes, makes
HTTP requests, repairs captures, or extends the transaction Applied drain. The
caller supplies trusted prelaunch identities and exact expected timing geometry.
"""
from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat

from kura_resource_metrics import AvailableObservation, ProjectionError, parse_kura_resource_metrics
from resource_process import ProcessIdentity
from signed_request_journal import SignedRequestError, SignedRequestReader, RetainedRequest, EVENTS as SIGNED_EVENTS
from resource_evidence_budget import (
    BudgetError, CaptureGeometry, CapturePolicy, PerRunResourceBudget, validate_run_budget,
    MAX_CONTROL_FILES, MAX_TOTAL_BYTES, MAX_WIRE_BYTES,
    MAX_FILE_BYTES as MAX_JOURNAL_BYTES,
    CAPTURE_MANIFEST_BYTES as MAX_MANIFEST_BYTES,
    MAX_STATUS_BODY_BYTES as MAX_STATUS_BYTES,
    MAX_METRICS_BODY_BYTES as MAX_METRICS_BYTES,
)

NS = 1_000_000_000
MAX_EXACT = 1 << 53
MAX_U64 = (1 << 64) - 1
MAX_EVENT_BYTES = 16 * 1024
CAPTURE_SCHEMA = 'iroha.sumeragi_v2.resource_probe.capture.v1'
JOURNAL_SCHEMA = 'iroha.sumeragi_v2.multilane_scaling.collector_journal.v1'
DIGEST = re.compile(r'[0-9a-f]{64}')
PLAN_FIELDS = {'event', 'schema', 'pair_index', 'variant', 'seed', 'accounts', 'account_selection',
               'workload', 'max_effects_per_account', 'scheduled_requests', 'warmup_ns', 'measurement_ns',
               'drain_ns', 'submission_lag_bound_ns', 'preparation_lookahead', 'preparation_concurrency',
               'preparation_ahead_ns', 'max_submissions', 'max_in_flight', 'max_status_requests', 'poll_interval_ns'}
OTHER_EVENTS = SIGNED_EVENTS | {'scheduled', 'workload_account_preflight', 'prepared', 'offer', 'accepted', 'status',
                'status_missing', 'workload_postconditions_started', 'workload_account_postcondition'}


class ReplayError(ValueError):
    """Closed failure reason without untrusted path, URL or response text."""


def _require(condition, code):
    if not condition:
        raise ReplayError(code)


def _integer(value, minimum=0, maximum=MAX_EXACT):
    _require(type(value) is int and minimum <= value <= maximum, 'integer_outside_bounds')
    return value


def _fields(value, names):
    _require(type(value) is dict and set(value) == set(names), 'object_fields_invalid')
    return value


def _same(actual, expected):
    if type(expected) is dict:
        return type(actual) is dict and actual.keys() == expected.keys() and all(_same(actual[k], v) for k, v in expected.items())
    if type(expected) in (list, tuple):
        return type(actual) is list and len(actual) == len(expected) and all(_same(a, b) for a, b in zip(actual, expected, strict=True))
    return type(actual) is type(expected) and actual == expected


def _json(raw, cap, *, status=False):
    _require(type(raw) is bytes and 0 < len(raw) <= cap, 'json_size_invalid')
    depth, quoted, escaped = 0, False, False
    for byte in raw:
        if quoted:
            if escaped: escaped = False
            elif byte == 92: escaped = True
            elif byte == 34: quoted = False
        elif byte == 34: quoted = True
        elif byte in (91, 123):
            depth += 1
            _require(depth <= 64, 'json_depth_exceeded')
        elif byte in (93, 125):
            depth -= 1
            _require(depth >= 0, 'json_invalid')
    _require(depth == 0 and not quoted, 'json_invalid')
    def pairs(rows):
        result = {}
        for key, value in rows:
            _require(key not in result, 'json_duplicate_key')
            result[key] = value
        return result
    def integer(token):
        _require(len(token) <= 128, 'json_numeric_token_exceeded')
        return int(token)
    def real(token):
        _require(status and len(token) <= 128, 'json_number_invalid')
        value = float(token)
        _require(math.isfinite(value), 'json_number_invalid')
        return value
    def constant(_): raise ReplayError('json_number_invalid')
    try:
        return json.loads(raw.decode('utf-8'), object_pairs_hook=pairs, parse_int=integer,
                          parse_float=real, parse_constant=constant)
    except (ValueError, UnicodeError, RecursionError) as error:
        if isinstance(error, ReplayError): raise
        raise ReplayError('json_invalid') from None


def _sum(values):
    total = 0
    for value in values:
        total += _integer(value)
        _require(total <= MAX_EXACT, 'aggregate_overflow')
    return total


@dataclass(frozen=True)
class ExpectedPeer:
    """One explicit trusted prelaunch process lifetime; never inferred from captures."""
    peer_id: str
    identity: ProcessIdentity


@dataclass(frozen=True)
class ReplayGeometry:
    """Independent declared transaction and resource timing, in integer nanoseconds."""
    warmup_ns: int
    measurement_ns: int
    drain_ns: int
    preparation_ahead_ns: int
    interval_ns: int
    response_deadline_ns: int
    max_start_lag_ns: int

    def validate(self):
        for value in asdict(self).values(): _integer(value, 0, (1 << 63) - 1)
        _require(2_000_000 <= self.interval_ns <= 60 * NS
                 and 1_000_000 <= self.response_deadline_ns <= 30 * NS
                 and self.response_deadline_ns <= self.interval_ns // 2
                 and self.max_start_lag_ns <= self.interval_ns // 4
                 and all(value % 1_000_000 == 0 for value in
                         (self.interval_ns, self.response_deadline_ns, self.max_start_lag_ns))
                 and self.measurement_ns >= 20 * self.interval_ns
                 and 0 < self.drain_ns <= 300 * NS
                 and self.measurement_ns % self.interval_ns == 0
                 and self.drain_ns % self.interval_ns == 0
                 and 2 <= self.samples <= 100000, 'geometry_invalid')
        _require(self.final + self.warmup_ns + self.drain_ns + self.preparation_ahead_ns
                 + 3 * self.response_deadline_ns + self.max_start_lag_ns < 1 << 63, 'geometry_overflow')

    @property
    def final(self): return self.measurement_ns + self.drain_ns
    @property
    def samples(self): return self.final // self.interval_ns + 1 if self.interval_ns else 0
    def sampling(self):
        return {'interval_ns': self.interval_ns, 'response_deadline_ns': self.response_deadline_ns,
                'max_start_lag_ns': self.max_start_lag_ns, 'first_offset_ns': 0,
                'final_offset_ns': self.final, 'sample_count': self.samples}


@dataclass(frozen=True)
class CaptureReduction:
    """One independently reduced capture and its actual filesystem cost."""
    sequence: int
    rss_before_bytes: int
    rss_after_bytes: int
    queue_size_sum: int
    queue_size_max: int
    storage_bytes: int
    represented_entries: int
    raw_body_bytes: int
    manifest_bytes: int
    reported_wire_bytes: int


@dataclass(frozen=True)
class Bracket:
    """Collector-owned sample times; these are not Python probe offsets."""
    scheduled_offset_ns: int
    start_offset_ns: int
    end_offset_ns: int
    capture: CaptureReduction


@dataclass(frozen=True)
class ReplayResult:
    """Scoped replay evidence, not throughput or release qualification."""
    preflight: CaptureReduction
    samples: tuple[Bracket, ...]
    finish_start_ns: int
    finish_end_ns: int
    journal_sha256: str
    journal_bytes: int
    capture_file_count: int
    capture_bytes: int
    raw_body_bytes: int
    manifest_bytes: int
    reported_http_framing_bytes: int
    admitted_resource_byte_limit: int
    admitted_journal_byte_limit: int
    admitted_resource_file_count: int
    admitted_experiment_resource_bytes: int
    admitted_experiment_resource_file_count: int
    admitted_experiment_total_bytes: int
    global_byte_limit: int
    control_file_limit: int
    signed_requests: tuple[RetainedRequest, ...]


def _stat_identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _open_directory(path):
    _require(path.is_absolute() and all(part not in ('.', '..') for part in path.parts), 'directory_path_invalid')
    fd = os.open('/', os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY)
    try:
        for component in path.parts[1:]:
            next_fd = os.open(component, os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY | os.O_NOFOLLOW,
                              dir_fd=fd)
            os.close(fd)
            fd = next_fd
        return fd
    except BaseException:
        os.close(fd)
        raise


class _Captures:
    def __init__(self, path, expected_files, maximum_bytes):
        self.path, self.fd = path, _open_directory(path)
        try:
            info = os.fstat(self.fd)
            _require(stat.S_IMODE(info.st_mode) == 0o700 and info.st_uid == os.geteuid(), 'capture_owner_invalid')
            self.identity = _stat_identity(info)
            self.expected_files, self.maximum_bytes = expected_files, maximum_bytes
            self.files, self.total_bytes = {}, 0
        except BaseException:
            os.close(self.fd)
            raise

    def read(self, reference, expected_name, cap):
        _fields(reference, ('name', 'sha256', 'bytes'))
        _require(type(reference['name']) is str and reference['name'] == expected_name
                 and type(reference['sha256']) is str and DIGEST.fullmatch(reference['sha256']) is not None,
                 'capture_reference_invalid')
        count = _integer(reference['bytes'], 1, cap)
        _require(expected_name not in self.files and len(self.files) < self.expected_files
                 and count <= self.maximum_bytes - self.total_bytes, 'capture_geometry_exceeded')
        fd = os.open(expected_name, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=self.fd)
        try:
            before = os.fstat(fd)
            _file_admitted(before, count)
            raw = bytearray()
            while len(raw) < count:
                chunk = os.pread(fd, min(65536, count - len(raw)), len(raw))
                _require(bool(chunk), 'capture_truncated')
                raw.extend(chunk)
            _require(_stat_identity(os.fstat(fd)) == _stat_identity(before)
                     and _stat_identity(os.stat(expected_name, dir_fd=self.fd, follow_symlinks=False)) == _stat_identity(before),
                     'capture_changed')
            _require(hashlib.sha256(raw).hexdigest() == reference['sha256'], 'capture_digest_mismatch')
            self.files[expected_name] = _stat_identity(before)
            self.total_bytes += count
            return bytes(raw)
        finally:
            os.close(fd)

    def finish(self):
        _require(len(self.files) == self.expected_files, 'capture_set_incomplete')
        count = 0
        with os.scandir(self.fd) as entries:
            for entry in entries:
                count += 1
                _require(count <= self.expected_files and entry.name in self.files, 'capture_set_extra')
                _require(_stat_identity(entry.stat(follow_symlinks=False)) == self.files[entry.name], 'capture_changed')
        _require(count == self.expected_files and _stat_identity(os.fstat(self.fd)) == self.identity, 'capture_directory_changed')
        check = _open_directory(self.path)
        try: _require(_stat_identity(os.fstat(check)) == self.identity, 'capture_directory_changed')
        finally: os.close(check)

    def close(self): os.close(self.fd)


def _file_admitted(info, count):
    _require(stat.S_ISREG(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o600
             and info.st_uid == os.geteuid() and info.st_nlink == 1 and info.st_size == count,
             'capture_file_invalid')


def _peer_identity(value):
    _fields(value, ('pid', 'uid', 'start_seconds', 'start_microseconds', 'start_abstime', 'image_uuid', 'executable_sha256'))
    _integer(value['pid'], 2, (1 << 31) - 1)
    _integer(value['uid'], 0, (1 << 32) - 1)
    _integer(value['start_seconds'], 1, MAX_U64)
    _integer(value['start_microseconds'], 0, 999999)
    _integer(value['start_abstime'], 1, MAX_U64)
    _require(type(value['image_uuid']) is str and re.fullmatch(r'[0-9a-f]{32}', value['image_uuid']) is not None
             and value['image_uuid'] != '0' * 32 and type(value['executable_sha256']) is str
             and DIGEST.fullmatch(value['executable_sha256']) is not None, 'process_identity_invalid')
    return value


def _expected_peers(peers):
    _require(type(peers) is tuple and 4 <= len(peers) <= 64, 'peer_scope_invalid')
    ids, pids = set(), set()
    for peer in peers:
        _require(type(peer) is ExpectedPeer and type(peer.identity) is ProcessIdentity
                 and type(peer.peer_id) is str and re.fullmatch(r'[A-Za-z0-9_.-]{1,128}', peer.peer_id) is not None,
                 'peer_scope_invalid')
        _peer_identity(asdict(peer.identity))
        _require(peer.peer_id not in ids and peer.identity.pid not in pids, 'peer_scope_duplicate')
        ids.add(peer.peer_id)
        pids.add(peer.identity.pid)


def _capture(store, reference, kind, sequence, peers, timeout_ns, policy, allocation):
    raw = store.read(reference, f'{kind}-{sequence:010}.json', MAX_MANIFEST_BYTES)
    manifest = _json(raw, MAX_MANIFEST_BYTES)
    _fields(manifest, ('schema', 'kind', 'sequence', 'available', 'probe_local_elapsed_ns', 'wire_bytes',
                      'peers', 'aggregates', 'capture_policy'))
    _require(manifest['schema'] == CAPTURE_SCHEMA and manifest['kind'] == kind
             and type(manifest['sequence']) is int and manifest['sequence'] == sequence
             and manifest['available'] is True, 'manifest_identity_or_availability_invalid')
    _fields(manifest['capture_policy'], ('status_body_bytes', 'metrics_body_bytes'))
    _require(_same(manifest['capture_policy'], asdict(policy)), 'capture_policy_mismatch')
    _integer(manifest['probe_local_elapsed_ns'], 1, timeout_ns - 1)
    wire = _integer(manifest['wire_bytes'], 1, MAX_WIRE_BYTES)
    _require(type(manifest['peers']) is list and len(manifest['peers']) == len(peers), 'capture_peer_set_invalid')
    before, after, queues, storage, represented = [], [], [], [], []
    raw_bytes = 0
    for ordinal, (row, peer) in enumerate(zip(manifest['peers'], peers, strict=True)):
        _fields(row, ('peer_id', 'status', 'metrics', 'process_before', 'process_after'))
        _require(row['peer_id'] == peer.peer_id, 'capture_peer_identity_invalid')
        for field, values in (('process_before', before), ('process_after', after)):
            sample = _fields(row[field], ('identity', 'rss_bytes'))
            identity = _peer_identity(sample['identity'])
            _require(_same(identity, asdict(peer.identity)), 'process_lifetime_mismatch')
            values.append(_integer(sample['rss_bytes'], 1))
        status = _fields(row['status'], ('body', 'content_type', 'queue_size'))
        _require(status['content_type'] in ('application/json', 'application/json; charset=utf-8'), 'content_type_invalid')
        body = store.read(status['body'], f'{kind}-{sequence:010}-peer-{ordinal:04}-status.body', min(policy.status_body_bytes, wire - raw_bytes))
        raw_bytes += len(body)
        decoded = _json(body, policy.status_body_bytes, status=True)
        _require(type(decoded) is dict and 'queue_size' in decoded, 'status_queue_missing')
        queue = _integer(decoded['queue_size'])
        _require(_same(status['queue_size'], queue), 'status_queue_mismatch')
        queues.append(queue)
        metric = _fields(row['metrics'], ('body', 'content_type', 'kura'))
        _require(metric['content_type'] in ('text/plain', 'text/plain; version=0.0.4',
                 'text/plain; version=0.0.4; charset=utf-8', 'text/plain; charset=utf-8'), 'content_type_invalid')
        body = store.read(metric['body'], f'{kind}-{sequence:010}-peer-{ordinal:04}-metrics.body', min(policy.metrics_body_bytes, wire - raw_bytes))
        raw_bytes += len(body)
        try: decoded = parse_kura_resource_metrics(body)
        except ProjectionError: raise ReplayError('kura_projection_invalid') from None
        _require(type(decoded) is AvailableObservation, 'kura_unavailable')
        _require(_same(metric['kura'], asdict(decoded)), 'kura_projection_mismatch')
        storage.append(decoded.total.storage_bytes)
        represented.append(decoded.represented_entries)
    reductions = {'rss_before_bytes': _sum(before), 'rss_after_bytes': _sum(after),
                  'queue_size_sum': _sum(queues), 'queue_size_max': max(queues),
                  'inventory': {'storage_bytes': _sum(storage), 'represented_entries': _sum(represented)}}
    _require(_same(manifest['aggregates'], reductions), 'capture_aggregate_mismatch')
    _require(raw_bytes <= wire <= MAX_WIRE_BYTES, 'wire_geometry_invalid')
    _require(raw_bytes + len(raw) <= allocation.bytes_per_capture, 'capture_allocation_exceeded')
    return CaptureReduction(sequence, reductions['rss_before_bytes'], reductions['rss_after_bytes'],
                            reductions['queue_size_sum'], reductions['queue_size_max'],
                            reductions['inventory']['storage_bytes'], reductions['inventory']['represented_entries'],
                            raw_bytes, len(raw), wire)


class _JournalState:
    def __init__(self, geometry, peers, captures, policy, allocation):
        self.geometry, self.peers, self.captures = geometry, peers, captures
        self.policy, self.allocation = policy, allocation
        self.stage, self.preflight, self.pending = 'plan', None, None
        self.samples, self.last_end = [], 0
        self.finish_start, self.finish_end, self.final_rows = None, None, set()
        self.scheduled_requests = None
        self.signed = SignedRequestReader(allocation.journal.max_bytes)

    def consume(self, row):
        try:
            self.signed.check_interleaving(row)
            self._consume_resource(row)
            self.signed.consume(row)
        except SignedRequestError as error:
            self.signed.abort()
            raise ReplayError(str(error)) from None
        except BaseException:
            self.signed.abort()
            raise

    def _consume_resource(self, row):
        _require(type(row) is dict and type(row.get('event')) is str, 'journal_event_invalid')
        event, geometry = row['event'], self.geometry
        if event == 'plan':
            _require(self.stage == 'plan', 'journal_plan_order')
            _fields(row, PLAN_FIELDS)
            _require(row['schema'] == JOURNAL_SCHEMA, 'journal_schema_invalid')
            _require(_same(row['pair_index'], self.allocation.run.pair_index)
                     and _same(row['variant'], self.allocation.run.variant), 'journal_run_budget_mismatch')
            for key in ('warmup_ns', 'measurement_ns', 'drain_ns', 'preparation_ahead_ns'):
                _require(_same(row[key], getattr(geometry, key)), 'journal_geometry_mismatch')
            self.scheduled_requests = _integer(row['scheduled_requests'], 1, 1_000_000)
            self.stage = 'preflight'
        elif event == 'resource_preflight':
            _require(self.stage == 'preflight', 'preflight_order_invalid')
            _fields(row, ('event', 'sequence', 'outcome', 'manifest', 'sampling'))
            _require(_same(row['sequence'], 0) and row['outcome'] == 'complete'
                     and _same(row['sampling'], geometry.sampling()), 'preflight_invalid')
            self.preflight = _capture(self.captures, row['manifest'], 'preflight', 0, self.peers,
                                     geometry.response_deadline_ns, self.policy, self.allocation)
            self.stage = 'clock'
        elif event == 'clock_started':
            _require(self.stage == 'clock', 'clock_order_invalid')
            _fields(row, ('event', 'initial_offset_ns'))
            _require(_same(row['initial_offset_ns'], -(geometry.warmup_ns + geometry.drain_ns + geometry.preparation_ahead_ns)),
                     'clock_origin_invalid')
            self.stage = 'sample'
        elif event == 'resource_request':
            _require(self.stage == 'sample' and self.pending is None, 'request_order_invalid')
            finish = len(self.samples) == geometry.samples
            _fields(row, ('event', 'kind', 'sequence', 'start_offset_ns') if finish else
                    ('event', 'kind', 'sequence', 'scheduled_offset_ns', 'start_offset_ns'))
            _require(row['kind'] == ('finish' if finish else 'sample')
                     and _same(row['sequence'], len(self.samples) + 1), 'request_identity_invalid')
            start = _integer(row['start_offset_ns'], 0, (1 << 63) - 1)
            _require(start >= self.last_end, 'clock_reordered')
            if not finish:
                scheduled = len(self.samples) * geometry.interval_ns
                _require(_same(row['scheduled_offset_ns'], scheduled)
                         and scheduled <= start <= scheduled + geometry.max_start_lag_ns, 'sample_schedule_invalid')
            self.pending = row
        elif event == 'resource_observation':
            _require(self.stage == 'sample' and self.pending is not None and self.pending['kind'] == 'sample', 'observation_order_invalid')
            _fields(row, ('event', 'sequence', 'scheduled_offset_ns', 'start_offset_ns', 'end_offset_ns', 'outcome', 'manifest'))
            for name in ('sequence', 'scheduled_offset_ns', 'start_offset_ns'):
                _require(_same(row[name], self.pending[name]), 'bracket_identity_mismatch')
            _require(row['outcome'] == 'complete', 'observation_incomplete')
            end = _integer(row['end_offset_ns'], row['start_offset_ns'], (1 << 63) - 1)
            deadline = row['start_offset_ns'] + geometry.response_deadline_ns
            if row['scheduled_offset_ns'] == geometry.final:
                deadline = min(deadline, geometry.final + geometry.response_deadline_ns)
            _require(end < deadline, 'sample_deadline_exceeded')
            capture = _capture(self.captures, row['manifest'], 'sample', row['sequence'], self.peers,
                               geometry.response_deadline_ns, self.policy, self.allocation)
            self.samples.append(Bracket(row['scheduled_offset_ns'], row['start_offset_ns'], end, capture))
            self.pending, self.last_end = None, end
        elif event == 'resource_collection_finished':
            _require(self.stage == 'sample' and self.pending is not None and self.pending['kind'] == 'finish'
                     and len(self.samples) == geometry.samples, 'finish_order_invalid')
            _fields(row, ('event', 'sequence', 'start_offset_ns', 'end_offset_ns', 'sampling'))
            _require(_same(row['sequence'], self.pending['sequence'])
                     and _same(row['start_offset_ns'], self.pending['start_offset_ns'])
                     and _same(row['sampling'], geometry.sampling()), 'finish_identity_mismatch')
            end = _integer(row['end_offset_ns'], row['start_offset_ns'], (1 << 63) - 1)
            _require(end < row['start_offset_ns'] + geometry.response_deadline_ns, 'finish_deadline_exceeded')
            self.finish_start, self.finish_end, self.pending, self.stage = row['start_offset_ns'], end, None, 'final'
        elif event == 'request_final':
            _require(self.stage == 'final', 'transaction_final_order_invalid')
            _fields(row, ('event', 'plan', 'hash', 'offer_offset_ns', 'acknowledgment_offset_ns', 'applied_offset_ns',
                          'block_height', 'status_attempts', 'submission_finished', 'failure'))
            plan = _fields(row['plan'], ('cohort', 'sequence', 'logical_id', 'scheduled_offset_ns', 'account_index'))
            _require(plan['cohort'] in ('warmup', 'measurement'), 'transaction_cohort_invalid')
            sequence = _integer(plan['sequence'], 0, 1_000_000)
            identity = plan['cohort'], sequence
            _require(identity not in self.final_rows and len(self.final_rows) < self.scheduled_requests
                     and row['failure'] is None and row['submission_finished'] is True, 'transaction_final_invalid')
            offer = _integer(row['offer_offset_ns'], -(1 << 63), (1 << 63) - 1)
            ack = _integer(row['acknowledgment_offset_ns'], offer, (1 << 63) - 1)
            applied = _integer(row['applied_offset_ns'], offer + 1, (1 << 63) - 1)
            _require((ack < 0 and applied < 0) if plan['cohort'] == 'warmup' else
                     (0 <= offer and ack <= geometry.final and applied <= geometry.final), 'transaction_drain_extended')
            self.final_rows.add(identity)
        elif event == 'collection_finished':
            _require(self.stage == 'final' and len(self.final_rows) == self.scheduled_requests, 'collection_finish_order_invalid')
            _fields(row, ('event', 'passed', 'failure'))
            _require(row['passed'] is True and row['failure'] is None, 'collection_failed')
            self.stage = 'done'
        else:
            _require(event in OTHER_EVENTS and self.stage not in ('plan', 'done'), 'unknown_or_failed_journal_event')


def validate_replay_scope(expected_peers: tuple[ExpectedPeer, ...], geometry: ReplayGeometry, *,
                          expected_policy: CapturePolicy,
                          allocation: PerRunResourceBudget) -> PerRunResourceBudget:
    """Validate trusted peer/Clock/policy/allocation scope without evidence I/O.

    Orchestrators use this same owner before scanning; replay calls it directly.
    The caller supplies independently pinned parent inputs and process identities.
    This validates their shape and consistency, not their external provenance.
    """
    _require(type(geometry) is ReplayGeometry, 'geometry_invalid')
    geometry.validate()
    _expected_peers(expected_peers)
    _require(type(expected_policy) is CapturePolicy, 'resource_policy_invalid')
    try:
        expected_policy.__post_init__()
        allocation = validate_run_budget(allocation)
        capture_geometry = CaptureGeometry(len(expected_peers), geometry.interval_ns,
                                           geometry.measurement_ns, geometry.drain_ns)
    except BudgetError:
        raise ReplayError('resource_budget_invalid') from None
    _require(_same(asdict(expected_policy), asdict(allocation.policy)), 'resource_policy_mismatch')
    _require(_same(asdict(capture_geometry), asdict(allocation.geometry)), 'resource_budget_geometry_mismatch')
    return allocation


def replay(capture_directory: Path, journal_path: Path, expected_journal_sha256: str,
           expected_peers: tuple[ExpectedPeer, ...], geometry: ReplayGeometry, *,
           expected_policy: CapturePolicy, allocation: PerRunResourceBudget) -> ReplayResult:
    """Replay raw evidence within independently supplied policy and admitted limits.

    The required allocation is re-admitted before filesystem access. The caller
    independently pins its full public experiment inputs and run/file identities;
    neither the capture manifest nor the returned reduction supplies that trust.
    """
    allocation = validate_replay_scope(expected_peers, geometry,
                                       expected_policy=expected_policy, allocation=allocation)
    _require(type(expected_journal_sha256) is str and DIGEST.fullmatch(expected_journal_sha256) is not None,
             'journal_digest_invalid')
    # One preflight and all inclusive measurement/drain samples; finish has no capture.
    count, maximum_bytes = allocation.member_count, allocation.resource_bytes
    captures, parent, fd = None, None, None
    try:
        captures = _Captures(capture_directory, count, maximum_bytes)
        parent = _open_directory(journal_path.parent)
        fd = os.open(journal_path.name, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
        info = os.fstat(fd)
        _require(0 < info.st_size <= MAX_JOURNAL_BYTES, 'journal_size_invalid')
        _require(info.st_size <= allocation.journal.max_bytes, 'journal_allocation_exceeded')
        _file_admitted(info, info.st_size)
        state = _JournalState(geometry, expected_peers, captures, expected_policy, allocation)
        digest, position, buffer = hashlib.sha256(), 0, bytearray()
        while position < info.st_size:
            chunk = os.pread(fd, min(65536, info.st_size - position), position)
            _require(bool(chunk), 'journal_truncated')
            position += len(chunk)
            digest.update(chunk)
            buffer.extend(chunk)
            while True:
                boundary = buffer.find(b'\n')
                if boundary < 0: break
                _require(boundary <= MAX_EVENT_BYTES and boundary > 0, 'journal_event_size_invalid')
                state.consume(_json(bytes(buffer[:boundary]), MAX_EVENT_BYTES))
                del buffer[:boundary + 1]
            _require(len(buffer) <= MAX_EVENT_BYTES, 'journal_event_size_invalid')
        _require(not buffer and state.stage == 'done', 'journal_incomplete')
        _require(digest.hexdigest() == expected_journal_sha256, 'journal_digest_mismatch')
        _require(_stat_identity(os.fstat(fd)) == _stat_identity(info)
                 and _stat_identity(os.stat(journal_path.name, dir_fd=parent, follow_symlinks=False)) == _stat_identity(info),
                 'journal_changed')
        check = _open_directory(journal_path.parent)
        try:
            _require((os.fstat(check).st_dev, os.fstat(check).st_ino) == (os.fstat(parent).st_dev, os.fstat(parent).st_ino),
                     'journal_parent_changed')
        finally: os.close(check)
        captures.finish()
        reductions = [state.preflight, *(row.capture for row in state.samples)]
        raw_bytes = sum(row.raw_body_bytes for row in reductions)
        manifest_bytes = sum(row.manifest_bytes for row in reductions)
        return ReplayResult(state.preflight, tuple(state.samples), state.finish_start, state.finish_end,
                            digest.hexdigest(), info.st_size, count, captures.total_bytes, raw_bytes, manifest_bytes,
                            sum(row.reported_wire_bytes - row.raw_body_bytes for row in reductions), maximum_bytes,
                            allocation.journal.max_bytes, allocation.member_count,
                            allocation.experiment.resource_bytes, allocation.experiment.resource_member_count,
                            allocation.experiment.total_bytes, MAX_TOTAL_BYTES, MAX_CONTROL_FILES,
                            state.signed.finish())
    except SignedRequestError as error:
        raise ReplayError(str(error)) from None
    except OSError:
        raise ReplayError('evidence_filesystem_unavailable') from None
    finally:
        if fd is not None: os.close(fd)
        if parent is not None: os.close(parent)
        if captures is not None: captures.close()
