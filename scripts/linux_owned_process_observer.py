"""Retained Linux terminal facts for an original Popen lifecycle.

The general reader remains strict. This owner admits only an actual retained
stat Z after a full live sample. Kernel absence classifications remain disabled
until their exact sites are exercised. No raw comm bytes, digest or length enter
evidence; replay checks the trusted reader's closed public numeric projection.
"""
from __future__ import annotations

import copy
import json
import os
import re
import subprocess
import time
from typing import Any

import private_settlement_process_observer as observer

MAX_STAT_BYTES = 4096
LIVE_STATES = (b'R', b'S', b'D', b'T', b't', b'I')
STAT_KEYS = frozenset({'pid', 'state', 'ppid', 'pgid', 'start_ticks', 'utime', 'stime'})
TERMINAL_KEYS = frozenset({'version', 'kind', 'operation', 'kernel_absence', 'identity',
                           'directory', 'stat', 'started_ns', 'finished_ns'})


def _canonical(value: Any) -> bytes:
    """Freeze only already-validated public observations in an independent value."""
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def _unsigned(value: Any, maximum: int = (1 << 64) - 1) -> bool:
    """Reject booleans and nonintegral or out-of-range public counters."""
    return type(value) is int and 0 <= value <= maximum


def _directory(info: os.stat_result) -> dict[str, int]:
    """Project only stable original proc-directory identity and owner fields."""
    return {'device': info.st_dev, 'inode': info.st_ino, 'uid': info.st_uid}


class LinuxOwnedTerminalObservation(Exception):
    """A typed pending terminal fact; it is never a successful sample by itself."""

    def __init__(self, value: dict[str, Any]):
        """Keep the closed numeric projection and a fixed payload-free message."""
        super().__init__('owned Linux terminal stat observed')
        self._raw = _canonical(value)

    def projection(self) -> dict[str, Any]:
        """Return fresh public facts without mutable authority over retained bytes."""
        return json.loads(self._raw)


class _OwnedReads(observer._LinuxSampleReads):
    """Classify only directly observed Z at the canonical before/after stat sites."""

    def __init__(self, owner: OwnedLinuxProcessReader):
        """Borrow the exact owner directory, never reopen a selected PID path."""
        super().__init__(owner.directory, owner.pid)
        self.owner = owner

    def close_failed(self, initiating: BaseException, cleanup: BaseException) -> BaseException:
        """A failed original close cannot authenticate a pending terminal fact."""
        self.cleanup_error = cleanup
        if self.owner.cleanup_error is None:
            self.owner.cleanup_error = cleanup
        self.owner.failed = True
        if isinstance(initiating, LinuxOwnedTerminalObservation):
            # Only an ordinary initiating error has first-error precedence.
            # A pending terminal fact is never a waiver for failed cleanup.
            initiating.__traceback__ = None
            return cleanup
        return initiating

    def stat(self, stage: str) -> dict[str, int]:
        """Strictly parse a transient kernel record before discarding comm material."""
        observer.require(stage in ('before', 'after'), 'unknown owned stat site')
        raw = None
        try:
            raw = observer.kernel_record(self.directory, 'stat')
            observer.require(0 < len(raw) <= MAX_STAT_BYTES, 'owned proc stat exceeds bound')
            state, fields = observer._parse_linux_stat_states(raw, self.pid, (*LIVE_STATES, b'Z'))
        except BaseException as error:
            # Preserve the actual exception and its cause/context identities, but
            # not read/parser traceback locals containing workload-controlled comm.
            raw = None
            pending, seen = [error], set()
            while pending:
                current = pending.pop()
                if id(current) in seen:
                    continue
                seen.add(id(current))
                current.__traceback__ = None
                for related in (current.__cause__, current.__context__):
                    if related is not None:
                        pending.append(related)
            raise error
        raw = None
        if state == b'Z':
            # The raw record, including the workload-controlled comm, never enters
            # the typed fact, a hash, an error payload or any persisted diagnostic.
            self.owner.terminal_stat(stage, fields)
        return fields


class OwnedLinuxProcessReader:
    """Retain one already-bound original proc directory until terminal and reap."""

    def __init__(self, native: Any, reader: observer.LinuxProcessReader,
                 image: observer.ExecutableImage):
        """Acquire only the Popen already bound by actual direct-FORK evidence."""
        observer.require(isinstance(native.original, subprocess.Popen)
                         and native.original_pid == native.original.pid,
                         'owned reader requires original Popen binding')
        self.native, self.base, self.image = native, reader, image
        self.pid = native.original.pid
        self.boot_id, self.clock_ticks = reader.boot_id, reader.clock_ticks
        self.directory = -1
        self._admission_raw = self._last_live_raw = None
        self.failed = self.terminated = False
        self.cleanup_error = None
        self.started_ns = 0
        self.directory = os.open(str(self.pid), os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC
                                 | os.O_NOFOLLOW, dir_fd=reader.proc)
        try:
            self.directory_identity = _directory(os.fstat(self.directory))
            observer.require(self.directory_identity['uid'] == os.geteuid(),
                             'owned proc directory has another owner')
            self._join_directory()
        except BaseException:
            try:
                os.close(self.directory)
            except BaseException:
                pass
            self.directory = -1
            raise

    def _join_directory(self) -> None:
        """Compare the retained and still-named proc directory before any admission."""
        observer.require(self.directory >= 0 and _directory(os.fstat(self.directory))
                         == self.directory_identity
                         == _directory(os.stat(str(self.pid), dir_fd=self.base.proc,
                                               follow_symlinks=False)),
                         'owned proc directory changed')

    def bind_initial(self, capture: dict[str, Any]) -> None:
        """Join the native adapter's actual complete capture to this reader's result."""
        observer.require(self._admission_raw is None and self._last_live_raw is not None
                         and type(capture) is dict
                         and set(capture) == {'sample', 'started_ns', 'finished_ns'}
                         and _canonical(capture['sample']) == self._last_live_raw
                         and _unsigned(capture['started_ns'])
                         and _unsigned(capture['finished_ns'])
                         and capture['started_ns'] <= self.started_ns <= capture['finished_ns'],
                         'owned initial capture differs from actual reader result')
        self._join_directory()
        self._admission_raw = _canonical({'capture': capture,
                                          'directory': self.directory_identity})

    def admission(self) -> dict[str, Any]:
        """Project the independently frozen original live sample and directory join."""
        observer.require(self._admission_raw is not None, 'owned terminal lacks live admission')
        return json.loads(self._admission_raw)

    def sample(self, pid: int, image: observer.ExecutableImage) -> dict[str, Any]:
        """Run the same canonical live algorithm on the retained original directory."""
        observer.require(pid == self.pid and type(pid) is int and image is self.image
                         and not self.failed and not self.terminated,
                         'owned reader selection or state differs')
        self.started_ns = time.monotonic_ns()
        try:
            image.validate()
            self._join_directory()
            value = self.base._sample_directory(pid, image, self.directory, _OwnedReads(self))
            self._last_live_raw = _canonical(value)
            return value
        except LinuxOwnedTerminalObservation:
            self.terminated = True
            raise
        except BaseException:
            self.failed = True
            raise

    def terminal_stat(self, stage: str, fields: dict[str, int]) -> None:
        """Raise only an original Z projection after checking prior live identity."""
        admitted = self.admission()
        identity = admitted['capture']['sample']['identity']
        self.image.validate()
        self._join_directory()
        observer.require(self.boot_id == identity['birth']['boot_id']
                         and identity['birth']['kind'] == 'linux_proc'
                         and identity['pid'] == self.pid
                         and identity['uid'] == self.directory_identity['uid']
                         and all(fields[key] == identity[key] for key in ('ppid', 'pgid'))
                         and fields['start_ticks'] == identity['birth']['start_ticks'],
                         'terminal proc identity differs from original live admission')
        last = json.loads(self._last_live_raw)
        cpu = (fields['utime'] + fields['stime']) * (1_000_000_000 // self.clock_ticks)
        observer.require(cpu >= last['cpu_time_ns'], 'terminal proc CPU counter regressed')
        finished = time.monotonic_ns()
        observer.require(self.started_ns <= finished, 'terminal observation clock reversed')
        value = {'version': 1, 'kind': 'linux_owned_terminal_stat',
                 'operation': 'stat_' + stage, 'kernel_absence': None,
                 'identity': copy.deepcopy(identity), 'directory': dict(self.directory_identity),
                 'stat': {'pid': self.pid, 'state': 'Z', **fields},
                 'started_ns': self.started_ns, 'finished_ns': finished}
        raise LinuxOwnedTerminalObservation(value)

    def close(self) -> None:
        """Close the exact proc directory and base reader, preserving the first failure."""
        first = None
        if self.directory >= 0:
            try:
                os.close(self.directory)
                self.directory = -1
            except BaseException as error:
                first = error
        try:
            self.base.close()
        except BaseException as error:
            if first is None:
                first = error
        if first is not None:
            raise first


def validate_terminal(value: dict[str, Any], admission: dict[str, Any],
                      lifecycle: dict[str, Any], last_cpu_ns: int,
                      total_cpu_ns: int, slack_ns: int) -> None:
    """Replay exact trusted-reader projections, not raw kernel event reconstruction."""
    observer.require(type(value) is dict and set(value) == TERMINAL_KEYS
                     and type(value['version']) is int and value['version'] == 1
                     and value['kind'] == 'linux_owned_terminal_stat'
                     and value['operation'] in ('stat_before', 'stat_after')
                     and value['kernel_absence'] is None,
                     'invalid owned terminal schema')
    observer.require(type(admission) is dict and set(admission) == {'capture', 'directory'},
                     'invalid owned admission schema')
    validate_admission(admission)
    capture = admission['capture']
    identity = capture['sample']['identity']
    observer.require(_canonical(value['identity']) == _canonical(identity)
                     and _canonical(value['directory']) == _canonical(admission['directory']),
                     'owned terminal original joins differ')
    fields = value['stat']
    observer.require(type(fields) is dict and set(fields) == STAT_KEYS
                     and fields['state'] == 'Z'
                     and all(_unsigned(fields[key]) for key in STAT_KEYS - {'state'})
                     and fields['pid'] == identity['pid']
                     and fields['ppid'] == identity['ppid']
                     and fields['pgid'] == identity['pgid']
                     and fields['start_ticks'] == identity['birth']['start_ticks'],
                     'owned terminal numeric identity differs')
    cpu = (fields['utime'] + fields['stime']) * capture['sample']['cpu_counter_unit_ns']
    observer.require(_unsigned(last_cpu_ns) and _unsigned(total_cpu_ns)
                     and _unsigned(slack_ns) and _unsigned(cpu)
                     and last_cpu_ns <= cpu <= total_cpu_ns + slack_ns,
                     'owned terminal numeric CPU differs from live sample or wait4')
    observer.require(lifecycle['finalized'] is True and lifecycle['lifecycle_certified'] is True
                     and lifecycle['first_error'] is None
                     and lifecycle['root_pid'] == fields['pid']
                     and lifecycle['terminal']['pid'] == fields['pid']
                     and all(counter['lost'] == 0 for counter in lifecycle['counters'])
                     and _unsigned(value['started_ns']) and _unsigned(value['finished_ns'])
                     and lifecycle['initial_exec_ns'] <= capture['started_ns']
                     <= capture['finished_ns'] <= value['started_ns']
                     <= value['finished_ns'] <= lifecycle['observed_through_ns'],
                     'owned terminal lifecycle or time join differs')


def validate_admission(value: dict[str, Any]) -> None:
    """Validate the full frozen public initial observation without reading a PID."""
    observer.require(type(value) is dict and set(value) == {'capture', 'directory'},
                     'invalid owned admission schema')
    directory = value['directory']
    observer.require(type(directory) is dict and set(directory) == {'device', 'inode', 'uid'}
                     and all(_unsigned(item) for item in directory.values())
                     and directory['inode'] > 0, 'invalid owned original directory')
    capture = value['capture']
    observer.require(type(capture) is dict and set(capture) == {'sample', 'started_ns', 'finished_ns'}
                     and _unsigned(capture['started_ns']) and _unsigned(capture['finished_ns'])
                     and 0 < capture['started_ns'] <= capture['finished_ns'],
                     'invalid owned initial capture times')
    sample = capture['sample']
    observer.require(type(sample) is dict and set(sample) == {
        'identity', 'cpu_time_ns', 'cpu_counter_unit_ns', 'rss_bytes'}
        and _unsigned(sample['cpu_time_ns'])
        and _unsigned(sample['cpu_counter_unit_ns'], 1_000_000_000)
        and sample['cpu_counter_unit_ns'] >= 1_000
        and 1_000_000_000 % sample['cpu_counter_unit_ns'] == 0
        and _unsigned(sample['rss_bytes'], observer.MAX_EXACT_INTEGER)
        and sample['rss_bytes'] > 0, 'invalid owned initial resource sample')
    identity = sample['identity']
    observer.require(type(identity) is dict and set(identity) == {
        'pid', 'ppid', 'pgid', 'uid', 'birth', 'executable_path', 'executable_sha256'}
        and all(_unsigned(identity[key], observer.PID_MAX) and identity[key] > 1
                for key in ('pid', 'ppid', 'pgid'))
        and _unsigned(identity['uid']) and identity['uid'] == directory['uid'],
        'invalid owned initial identity')
    birth = identity['birth']
    observer.require(type(birth) is dict and set(birth) == {'kind', 'boot_id', 'start_ticks'}
        and birth['kind'] == 'linux_proc' and type(birth['boot_id']) is str
        and re.fullmatch(r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', birth['boot_id']) is not None
        and birth['boot_id'].replace('-', '') != '0' * 32
        and _unsigned(birth['start_ticks']) and birth['start_ticks'] > 0,
        'invalid owned initial birth identity')
    observer.require(type(identity['executable_path']) is str
        and identity['executable_path'].startswith('/')
        and '\0' not in identity['executable_path']
        and type(identity['executable_sha256']) is str
        and re.fullmatch(r'[0-9a-f]{64}', identity['executable_sha256']) is not None
        and identity['executable_sha256'] != '0' * 64,
        'invalid owned initial executable identity')
