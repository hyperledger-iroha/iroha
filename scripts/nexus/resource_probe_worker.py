"""Persistent, sequence-bound resource probe with immutable owner-only captures.

The CLI owns measurement timestamps, cadence and the hard child deadline. This
worker never emits URL, runtime authentication or configuration contents. Public
HTTP bodies remain separately replayable through a bounded capture manifest.
"""
from __future__ import annotations

from contextlib import ExitStack
from dataclasses import asdict, dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys

from resource_probe import Endpoint, PeerTarget, Probe, ProbeError, ProbeObservation, _Deadline, _policy_values
from resource_process import DarwinProcessReader, ExecutableImage, PinnedProcess
from resource_evidence_budget import (
    BudgetError, CapturePolicy, PerRunResourceBudget, parse_run_budget, run_budget_inputs, canonical_run_budget_bytes, run_budget_sha256,
    CAPTURE_MANIFEST_BYTES as MAX_MANIFEST_BYTES, MAX_SAMPLES, MAX_WIRE_BYTES,
)

REQUEST_SCHEMA = 'iroha.sumeragi_v2.resource_probe.request.v1'
RESPONSE_SCHEMA = 'iroha.sumeragi_v2.resource_probe.response.v1'
CONFIG_SCHEMA = 'iroha.sumeragi_v2.resource_probe.config.v1'
CAPTURE_SCHEMA = 'iroha.sumeragi_v2.resource_probe.capture.v1'
ADMISSION_SCHEMA = 'iroha.sumeragi_v2.resource_probe.admission.v1'
MAX_FRAME_BYTES = 16 * 1024
MAX_CONFIG_BYTES = 1024 * 1024
MAX_JSON_DEPTH = 32


def _require(condition: bool, code: str) -> None:
    if not condition:
        raise ProbeError(code)


def _json(raw: bytes, cap: int):
    """Strict bounded protocol/configuration JSON, with no float or duplicate keys."""
    _require(0 < len(raw) <= cap, 'json_size_invalid')
    depth, quoted, escaped = 0, False, False
    for byte in raw:
        if quoted:
            if escaped:
                escaped = False
            elif byte == 92:
                escaped = True
            elif byte == 34:
                quoted = False
        elif byte == 34:
            quoted = True
        elif byte in (91, 123):
            depth += 1
            _require(depth <= MAX_JSON_DEPTH, 'json_depth_invalid')
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
        _require(len(token) <= 32, 'json_number_invalid')
        return int(token)

    def reject(_):
        raise ProbeError('json_number_invalid')

    try:
        return json.loads(raw.decode('utf-8'), object_pairs_hook=pairs,
                          parse_int=integer, parse_float=reject, parse_constant=reject)
    except (ValueError, UnicodeError, RecursionError) as error:
        if isinstance(error, ProbeError):
            raise
        raise ProbeError('json_invalid') from None


def _encoded(value: dict, cap: int) -> bytes:
    raw = json.dumps(value, ensure_ascii=True, separators=(',', ':'), allow_nan=False).encode('ascii')
    _require(len(raw) <= cap, 'encoded_size_invalid')
    return raw


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_nlink, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


def read_config(path: Path) -> dict:
    """Admit only a bounded current-user regular mode0600 prelaunch configuration."""
    _require(path.is_absolute(), 'config_path_invalid')
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        _require(stat.S_ISREG(before.st_mode) and stat.S_IMODE(before.st_mode) == 0o600
                 and before.st_uid == os.geteuid() and before.st_nlink == 1 and 0 < before.st_size <= MAX_CONFIG_BYTES,
                 'config_file_invalid')
        raw = os.pread(fd, before.st_size + 1, 0)
        _require(len(raw) == before.st_size and _identity(os.fstat(fd)) == _identity(before)
                 and _identity(os.stat(path, follow_symlinks=False)) == _identity(before),
                 'config_file_changed')
        value = _json(raw, MAX_CONFIG_BYTES)
    finally:
        os.close(fd)
    _require(type(value) is dict and set(value) == {'schema', 'peers', 'resource_budget'}
             and value['schema'] == CONFIG_SCHEMA and type(value['peers']) is list
             and 4 <= len(value['peers']) <= 64, 'config_invalid')
    _config_allocation(value)
    ids, pids, endpoints = set(), set(), set()
    for row in value['peers']:
        _require(type(row) is dict and set(row) == {'peer_id', 'pid', 'executable_path',
                    'executable_sha256', 'endpoint', 'headers'}, 'config_peer_invalid')
        peer_id, pid = row['peer_id'], row['pid']
        _require(type(peer_id) is str and re.fullmatch(r'[A-Za-z0-9_.-]{1,128}', peer_id) is not None
                 and type(pid) is int and 1 < pid <= 2**31 - 1
                 and peer_id not in ids and pid not in pids, 'config_peer_invalid')
        ids.add(peer_id)
        pids.add(pid)
        _require(type(row['executable_path']) is str and 1 <= len(row['executable_path']) <= 4096
                 and Path(row['executable_path']).is_absolute()
                 and type(row['executable_sha256']) is str
                 and re.fullmatch(r'[0-9a-f]{64}', row['executable_sha256']) is not None
                 and type(row['headers']) is dict, 'config_peer_invalid')
        endpoint = Endpoint.parse(row['endpoint'], tuple(row['headers'].items()))
        endpoint_identity = endpoint.scheme, endpoint.host, endpoint.port, endpoint.base_path
        _require(endpoint_identity not in endpoints, 'config_peer_invalid')
        endpoints.add(endpoint_identity)
    return value


def _config_allocation(config: dict) -> PerRunResourceBudget:
    """Re-admit all ten runs before image, process or capture ownership begins."""
    try:
        allocation = parse_run_budget(config['resource_budget'])
    except BudgetError:
        raise ProbeError('resource_budget_invalid') from None
    _require(allocation.geometry.peers == len(config['peers']), 'config_peer_count_mismatch')
    return allocation


@dataclass(frozen=True)
class ConfiguredProbe:
    """A configured collector and its fully admitted immutable run allocation."""
    collector: Probe
    allocation: PerRunResourceBudget


def build_probe(config_path: Path, owners: ExitStack) -> ConfiguredProbe:
    """Keep exact prelaunch-hashed images open; never discover or replace peer PIDs."""
    config = read_config(config_path)
    allocation = _config_allocation(config)
    reader = DarwinProcessReader()
    images, targets = {}, []
    for row in config['peers']:
        key = row['executable_path'], row['executable_sha256']
        if key not in images:
            images[key] = owners.enter_context(ExecutableImage(Path(key[0]), key[1]))
        pinned = PinnedProcess(row['peer_id'], row['pid'], images[key], reader)
        targets.append(PeerTarget(pinned, Endpoint.parse(row['endpoint'], tuple(row['headers'].items()))))
    return ConfiguredProbe(Probe(tuple(targets), allocation.policy), allocation)


@dataclass(frozen=True)
class _Admission:
    """One retained config/collector and deadline, before capture ownership exists."""
    configured: ConfiguredProbe
    collector: Probe
    allocation: PerRunResourceBudget
    allocation_inputs: bytes
    timeout_ms: int
    deadline: _Deadline

    def validate(self) -> None:
        """Reject original collector/allocation replacement after the admission reply."""
        _require(self.configured.collector is self.collector, 'resource_collector_changed')
        _require(self.configured.allocation is self.allocation, 'resource_budget_changed')
        try:
            current = canonical_run_budget_bytes(self.allocation)
        except BudgetError:
            raise ProbeError('resource_budget_invalid') from None
        _require(current == self.allocation_inputs, 'resource_budget_changed')
        self.deadline.remaining()


def _admit(config_path: Path, owners: ExitStack, timeout_ms: int, probe_builder) -> _Admission:
    """Admit exactly once without reading or creating any capture-directory path."""
    deadline = _Deadline(timeout_ms / 1000)
    configured = probe_builder(config_path, owners)
    _require(type(configured) is ConfiguredProbe, 'configured_probe_required')
    try:
        inputs = canonical_run_budget_bytes(configured.allocation)
        _require(len(inputs) <= MAX_CONFIG_BYTES, 'resource_budget_size_invalid')
    except BudgetError:
        raise ProbeError('resource_budget_invalid') from None
    admission = _Admission(configured, configured.collector, configured.allocation, inputs, timeout_ms, deadline)
    admission.validate()
    return admission


@dataclass(frozen=True)
class BodyReference:
    """A no-clobber capture relative to the single retained capture-directory owner."""
    name: str
    sha256: str
    bytes: int


class CaptureDirectory:
    """Pinned, empty, mode0700 directory; writes are bounded and never replace files."""
    def __init__(self, path: Path, allocation: PerRunResourceBudget):
        self.allocation = self._admitted_allocation = allocation
        try:
            self._allocation_inputs = _encoded(run_budget_inputs(allocation), MAX_CONFIG_BYTES)
        except BudgetError:
            raise ProbeError('resource_budget_invalid') from None
        self._captures = self._members = self._bytes = 0
        self._failed = False
        self._pending = None
        self._pending_index = 0
        self._peer_scope = None
        _require(path.is_absolute(), 'capture_path_invalid')
        self.path, self.fd = path, -1
        fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            info = os.fstat(fd)
            _require(stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700
                     and info.st_uid == os.geteuid(), 'capture_directory_invalid')
            self.owner = info.st_dev, info.st_ino, info.st_uid
            # One bounded startup emptiness check; never rescan a populated capture.
            with os.scandir(fd) as entries:
                _require(next(entries, None) is None, 'capture_directory_not_empty')
            self.fd = fd
            self.validate()
        except BaseException:
            os.close(fd)
            self.fd = -1
            raise

    def _check_allocation(self) -> None:
        """Keep the admitted public reservation fixed for the whole directory."""
        _require(self.allocation is self._admitted_allocation, 'resource_budget_changed')
        try:
            current = _encoded(run_budget_inputs(self.allocation), MAX_CONFIG_BYTES)
        except BudgetError:
            raise ProbeError('resource_budget_invalid') from None
        _require(current == self._allocation_inputs, 'resource_budget_changed')

    def validate(self) -> None:
        """Reject replacement/retirement of the actual directory owner."""
        _require(self.fd >= 0, 'capture_directory_closed')
        current, named = os.fstat(self.fd), os.stat(self.path, follow_symlinks=False)
        _require(all(stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700
                     and (info.st_dev, info.st_ino, info.st_uid) == self.owner
                     for info in (current, named)), 'capture_directory_changed')

    def close(self) -> None:
        if self.fd >= 0:
            os.close(self.fd)
            self.fd = -1

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def _write_reserved(self, name: str, raw: bytes, cap: int, deadline: _Deadline) -> BodyReference:
        """Create, flush and independently read back one exact immutable body."""
        _require(self._pending is not None and self._pending_index < len(self._pending),
                 'capture_reservation_required')
        reserved = self._pending[self._pending_index]
        _require(name == reserved[0] and raw is reserved[1] and cap == reserved[2],
                 'capture_reservation_mismatch')
        _require(re.fullmatch(r'(?:preflight|sample)-[0-9]{10}(?:-peer-[0-9]{4}-(?:status|metrics)\.body|\.json)', name) is not None
                 and type(raw) is bytes and 0 < len(raw) <= cap, 'capture_body_invalid')
        self.validate()
        deadline.remaining()
        fd = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK,
                     0o600, dir_fd=self.fd)
        try:
            os.fchmod(fd, 0o600)
            position = 0
            while position < len(raw):
                deadline.remaining()
                count = os.write(fd, raw[position:position + 64 * 1024])
                _require(count > 0, 'capture_write_incomplete')
                position += count
            os.fsync(fd)
            completed = os.fstat(fd)
            _require(stat.S_ISREG(completed.st_mode) and stat.S_IMODE(completed.st_mode) == 0o600
                     and completed.st_uid == self.owner[2] and completed.st_nlink == 1
                     and completed.st_size == len(raw),
                     'capture_file_invalid')
            digest = hashlib.sha256()
            for offset in range(0, len(raw), 64 * 1024):
                deadline.remaining()
                chunk = os.pread(fd, min(64 * 1024, len(raw) - offset), offset)
                _require(len(chunk) == min(64 * 1024, len(raw) - offset), 'capture_readback_incomplete')
                digest.update(chunk)
            expected = hashlib.sha256(raw).hexdigest()
            _require(digest.hexdigest() == expected and _identity(os.fstat(fd)) == _identity(completed)
                     and _identity(os.stat(name, dir_fd=self.fd, follow_symlinks=False)) == _identity(completed),
                     'capture_file_changed')
            self.validate()
            os.fsync(self.fd)
            deadline.remaining()
            self._pending_index += 1
            return BodyReference(name, expected, len(raw))
        finally:
            os.close(fd)

    def publish(self, kind: str, sequence: int, observation: ProbeObservation,
                deadline: _Deadline) -> BodyReference:
        """Publish the bounded manifest only after every exact body is durable."""
        _require(not self._failed and self._pending is None, 'capture_owner_failed')
        self._failed = True
        self._check_allocation()
        self.validate()
        _require(kind in ('preflight', 'sample') and type(sequence) is int
                 and ((kind == 'preflight' and sequence == 0)
                      or (kind == 'sample' and 1 <= sequence <= MAX_SAMPLES))
                 and sequence == self._captures and sequence < self.allocation.capture_count,
                 'capture_sequence_invalid')
        _require(type(observation) is ProbeObservation and type(observation.peers) is tuple
                 and len(observation.peers) == self.allocation.geometry.peers,
                 'capture_peer_scope_invalid')
        policy = CapturePolicy(*_policy_values(self.allocation.policy))
        _require(_policy_values(observation.capture_policy) == _policy_values(policy),
                 'capture_policy_mismatch')
        scope = tuple((peer.peer_id, peer.process_before.identity, peer.process_after.identity)
                      for peer in observation.peers)
        _require(all(type(peer_id) is str and re.fullmatch(r'[A-Za-z0-9_.-]{1,128}', peer_id)
                     and before == after for peer_id, before, after in scope)
                 and len({row[0] for row in scope}) == len(scope)
                 and len({row[1].pid for row in scope}) == len(scope)
                 and (self._peer_scope is None or self._peer_scope == scope),
                 'capture_peer_scope_invalid')
        prefix = f'{kind}-{sequence:010}'
        peers, planned, body_bytes = [], [], 0
        for ordinal, peer in enumerate(observation.peers):
            captured = {}
            for role, response in (('status', peer.status), ('metrics', peer.metrics)):
                cap = policy.status_body_bytes if role == 'status' else policy.metrics_body_bytes
                _require(type(response.raw_body) is bytes and 0 < len(response.raw_body) <= cap,
                         'capture_body_policy_exceeded')
                _require(response.route == '/' + role and type(response.body_bytes) is int
                         and hashlib.sha256(response.raw_body).hexdigest() == response.body_sha256
                         and len(response.raw_body) == response.body_bytes, 'capture_body_changed')
                name = f'{prefix}-peer-{ordinal:04}-{role}.body'
                reference = BodyReference(name, response.body_sha256, response.body_bytes)
                planned.append((name, response.raw_body, cap))
                body_bytes += len(response.raw_body)
                captured[role] = {'body': asdict(reference), 'content_type': response.content_type}
            captured['status']['queue_size'] = peer.queue_size
            captured['metrics']['kura'] = asdict(peer.kura)
            peers.append({'peer_id': peer.peer_id, **captured,
                          'process_before': asdict(peer.process_before),
                          'process_after': asdict(peer.process_after)})
        _require(type(observation.wire_bytes) is int and body_bytes <= observation.wire_bytes <= MAX_WIRE_BYTES,
                 'capture_wire_budget_exceeded')
        value = {'schema': CAPTURE_SCHEMA, 'kind': kind, 'sequence': sequence,
                 'capture_policy': asdict(policy),
                 'available': observation.available, 'probe_local_elapsed_ns': observation.local_elapsed_ns,
                 'wire_bytes': observation.wire_bytes, 'peers': peers,
                 'aggregates': {'rss_before_bytes': observation.rss_before_bytes,
                                'rss_after_bytes': observation.rss_after_bytes,
                                'queue_size_sum': observation.queue_size_sum,
                                'queue_size_max': observation.queue_size_max,
                                'inventory': asdict(observation.inventory) if observation.inventory is not None else None}}
        raw = _encoded(value, MAX_MANIFEST_BYTES)
        planned.append((f'{prefix}.json', raw, MAX_MANIFEST_BYTES))
        used_bytes = body_bytes + len(raw)
        _require(len(planned) == self.allocation.members_per_capture
                 and used_bytes <= self.allocation.bytes_per_capture,
                 'capture_reservation_exceeded')
        for used, addition, ceiling in ((self._captures, 1, self.allocation.capture_count),
                                        (self._members, len(planned), self.allocation.member_count),
                                        (self._bytes, used_bytes, self.allocation.resource_bytes)):
            _require(type(used) is int and 0 <= used <= ceiling and addition <= ceiling - used,
                     'capture_run_reservation_exceeded')
        self._check_allocation()
        # Reserve the complete exact image before the first create. A partial
        # failure consumes its reservation and poisons this directory for reuse.
        self._captures += 1
        self._members += len(planned)
        self._bytes += used_bytes
        self._peer_scope = scope
        self._pending, self._pending_index = tuple(planned), 0
        for name, body, cap in self._pending:
            if self._pending_index == len(self._pending) - 1:
                self._check_allocation()
            reference = self._write_reserved(name, body, cap, deadline)
        _require(self._pending_index == len(self._pending), 'capture_write_incomplete')
        self._check_allocation()
        self._pending = None
        self._failed = False
        return reference


def _request(raw: bytes) -> dict:
    _require(len(raw) <= MAX_FRAME_BYTES and raw.endswith(b'\n')
             and b'\n' not in raw[:-1], 'request_frame_invalid')
    value = _json(raw, MAX_FRAME_BYTES)
    _require(type(value) is dict and set(value) == {'schema', 'kind', 'sequence', 'timeout_ms'}
             and value['schema'] == REQUEST_SCHEMA
             and value['kind'] in ('admit', 'preflight', 'sample', 'finish')
             and type(value['sequence']) is int and 0 <= value['sequence'] <= MAX_SAMPLES + 1
             and type(value['timeout_ms']) is int and 1 <= value['timeout_ms'] <= 60000,
             'request_invalid')
    return value


def _admission_receipt(admission: _Admission) -> dict:
    """Return only the re-admitted public identity and selected writer allocations."""
    admission.validate()
    allocation = admission.allocation
    receipt = {'schema': ADMISSION_SCHEMA, 'budget_sha256': run_budget_sha256(allocation),
               'pair_index': allocation.run.pair_index, 'variant': allocation.run.variant,
               'geometry': asdict(allocation.geometry),
               'journal': asdict(allocation.journal),
               'trace': asdict(allocation.run.transaction_trace)}
    admission.validate()
    return receipt


def _admission_response(admission: _Admission | None) -> bytes:
    value = {'schema': RESPONSE_SCHEMA, 'kind': 'admit', 'sequence': 0,
             'outcome': 'complete' if admission is not None else 'failed',
             'admission': _admission_receipt(admission) if admission is not None else None}
    return _encoded(value, MAX_FRAME_BYTES - 1) + b'\n'


def _response(kind: str, sequence: int, outcome: str, reference: BodyReference | None) -> bytes:
    if kind == 'admit':
        _require(outcome == 'failed' and reference is None, 'admission_response_invalid')
        return _encoded({'schema': RESPONSE_SCHEMA, 'kind': kind, 'sequence': sequence,
                         'outcome': outcome, 'admission': None}, MAX_FRAME_BYTES - 1) + b'\n'
    value = {'schema': RESPONSE_SCHEMA, 'kind': kind, 'sequence': sequence, 'outcome': outcome,
             'manifest': asdict(reference) if reference is not None else None}
    return _encoded(value, MAX_FRAME_BYTES - 1) + b'\n'


def run_worker(config_path: Path, capture_path: Path, input_stream, output_stream,
               *, probe_builder=build_probe) -> int:
    """Serve one strict session; injected builder is a unit-test seam, never a flag."""
    expected, attempted, healthy, collector, directory = 0, False, False, None, None
    admission_attempted, admission = False, None
    with ExitStack() as owners:
        while True:
            raw = input_stream.readline(MAX_FRAME_BYTES + 1)
            if not raw:
                return 1  # An explicit finish acknowledgement is mandatory.
            try:
                request = _request(raw)
            except ProbeError:
                return 1  # Invalid fields are not reflected into output.
            kind, sequence = request['kind'], request['sequence']
            if not admission_attempted:
                if kind != 'admit' or sequence != 0:
                    output_stream.write(_response(kind, sequence, 'failed', None))
                    output_stream.flush()
                    return 1
                admission_attempted = True
                try:
                    admission = _admit(config_path, owners, request['timeout_ms'], probe_builder)
                    message = _admission_response(admission)
                except Exception:
                    # Admission failure is terminal for observation, but retains
                    # the explicit finish0 cleanup path without reflecting secrets.
                    admission = None
                    message = _admission_response(None)
                output_stream.write(message)
                output_stream.flush()
                continue
            if admission is None:
                finished = kind == 'finish' and sequence == 0
                output_stream.write(_response(kind, sequence, 'complete' if finished else 'failed', None))
                output_stream.flush()
                return 0 if finished else 1
            if kind == 'admit' or sequence != expected or (not attempted and kind != 'preflight') or (
                attempted and kind == 'preflight') or (kind == 'sample' and not healthy) or (
                kind == 'sample' and sequence > MAX_SAMPLES):
                output_stream.write(_response(kind, sequence, 'failed', None))
                output_stream.flush()
                return 1
            if kind == 'finish':
                if directory is not None:
                    directory.validate()
                    directory._check_allocation()
                output_stream.write(_response(kind, sequence, 'complete', None))
                output_stream.flush()
                return 0
            attempted = True
            expected += 1
            try:
                if collector is None:
                    _require(request['timeout_ms'] == admission.timeout_ms, 'admission_timeout_changed')
                    admission.validate()
                    # Includes admission work and the parent's directory setup;
                    # the preflight frame never starts another deadline/config.
                    deadline = admission.deadline
                    directory = owners.enter_context(CaptureDirectory(capture_path, admission.allocation))
                    collector = admission.collector
                else:
                    deadline = _Deadline(request['timeout_ms'] / 1000)
                deadline.remaining()
                observation = collector.collect(deadline.remaining())
                deadline.remaining()
                reference = directory.publish(kind, sequence, observation, deadline)
                deadline.remaining()
                healthy = observation.available
                message = _response(kind, sequence, 'complete' if healthy else 'unavailable', reference)
            except Exception:
                # Runtime config, URLs, native paths and response bodies must never
                # escape through an exception string or traceback. Partial files
                # stay retained without a qualifying manifest reference.
                healthy = False
                message = _response(kind, sequence, 'failed', None)
            output_stream.write(message)
            output_stream.flush()


def main(argv: list[str] | None = None) -> int:
    """Fixed arguments only; secrets stay in a separately owner-only runtime file."""
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 4 or args[0] != '--config' or args[2] != '--capture-dir':
        return 2
    try:
        return run_worker(Path(args[1]), Path(args[3]), sys.stdin.buffer, sys.stdout.buffer)
    except Exception:
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
