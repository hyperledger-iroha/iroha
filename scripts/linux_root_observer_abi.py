"""Adapter for the admitted Cargo-emitted root observer.

This owns observation resources and borrows an admitted module, never a process. The
caller retains its original Popen through native finish, then actual wait4.
Unclosed native tokens retain their module pin; there is no destructor cleanup,
PID attachment, signal operation, or successful-command inference here.
"""
from __future__ import annotations

import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import threading
import time

import private_settlement_process_observer as observer

OK, PENDING, INVALID, UNKNOWN, IO, PANIC, UNSUPPORTED = 0, 2, -1, -2, -3, -4, -5
STATUSES = {OK, PENDING, INVALID, UNKNOWN, IO, PANIC, UNSUPPORTED}
TAG = 1 << 63
MAX_REPORT = 64 * 1024
_RETAINED_MODULES = []
FIELDS = {'schema', 'scope', 'supervisor_pid', 'root_pid', 'prepared_ns',
          'observed_through_ns', 'initial_exec_ns', 'possible_cpus', 'online_cpus',
          'records', 'target_tasks', 'counters', 'first_error', 'terminal',
          'finalized', 'lifecycle_certified', 'image_authenticated', 'descendant_cleanup'}


class Refusal(RuntimeError):
    """A fixed public adapter-contract refusal, without command payloads."""


def require(value, message):
    """Reject any unproved ABI, source, schema, or original-process join."""
    if not value:
        raise Refusal(message)


def unsigned(value, maximum=(1 << 64) - 1):
    """Reject booleans, negatives and out-of-range native public integers."""
    return type(value) is int and 0 <= value <= maximum


def _pairs(rows):
    """Decode an exact JSON object without duplicate-key normalization."""
    result = {}
    for key, value in rows:
        require(key not in result, 'duplicate native report key')
        result[key] = value
    return result


def report(raw, supervisor, root):
    """Validate the closed bounded Norito JSON projection before using its facts."""
    require(type(raw) is bytes and 0 < len(raw) <= MAX_REPORT, 'native report extent')
    value = json.loads(raw.decode('utf-8'), object_pairs_hook=_pairs,
                       parse_constant=lambda _: (_ for _ in ()).throw(Refusal('nonfinite report number')))
    require(type(value) is dict and set(value) == FIELDS, 'native report field inventory')
    require(value['schema'] == 'iroha.root-thread-group-observation.v1'
            and value['scope'] == 'original_root_thread_group_lifecycle', 'native report schema')
    require(value['supervisor_pid'] == supervisor and type(value['supervisor_pid']) is int,
            'native supervisor identity')
    require(value['root_pid'] is None or unsigned(value['root_pid'], (1 << 32)-1), 'native root extent')
    require(value['root_pid'] in (None, root) if root is not None else value['root_pid'] is None,
            'native returned-child binding differs')
    for key in ('prepared_ns', 'observed_through_ns'):
        require(unsigned(value[key]), 'native clock extent')
    require(value['observed_through_ns'] >= value['prepared_ns'], 'native clock order')
    require(value['initial_exec_ns'] is None or unsigned(value['initial_exec_ns']), 'initial exec extent')
    for key in ('possible_cpus', 'online_cpus'):
        cpus = value[key]
        require(type(cpus) is list and len(cpus) <= 256
                and all(unsigned(cpu, 255) for cpu in cpus)
                and cpus == sorted(set(cpus)), 'native CPU inventory')
    require(set(value['online_cpus']) <= set(value['possible_cpus']), 'online CPU coverage')
    require(unsigned(value['records'], 4096) and unsigned(value['target_tasks'], 256),
            'native lifecycle extent')
    counters = value['counters']
    require(type(counters) is list and len(counters) <= 256, 'native counter inventory')
    cpus = []
    for counter in counters:
        require(type(counter) is dict and set(counter) == {'cpu', 'lost'}
                and unsigned(counter['cpu'], 255) and unsigned(counter['lost']),
                'native counter schema')
        cpus.append(counter['cpu'])
    require(cpus == sorted(set(cpus)) and set(cpus) <= set(value['possible_cpus']),
            'native counter CPU coverage')
    first = value['first_error']
    require(first is None or (type(first) is dict and set(first) == {'reason', 'errno'}
            and type(first['reason']) is str and first['reason'].isascii()
            and 0 < len(first['reason']) <= 64
            and (first['errno'] is None or type(first['errno']) is int
                 and -(1 << 31) <= first['errno'] < (1 << 31))), 'native first-error schema')
    terminal = value['terminal']
    require(terminal is None or (type(terminal) is dict and set(terminal) == {'pid', 'code', 'status'}
            and terminal['pid'] == root and type(terminal['pid']) is int
            and terminal['code'] in (os.CLD_EXITED, os.CLD_KILLED, os.CLD_DUMPED)
            and type(terminal['code']) is int and unsigned(terminal['status'], 255)),
            'native terminal schema')
    for key in ('finalized', 'lifecycle_certified', 'image_authenticated'):
        require(type(value[key]) is bool, 'native boolean schema')
    require(value['image_authenticated'] is False, 'native layer claimed executable authority')
    require(value['descendant_cleanup'] in ('root_thread_group_only', 'unsupported_or_unknown'),
            'native cleanup scope')
    if value['lifecycle_certified']:
        require(value['finalized'] and first is None and terminal is not None and root is not None
                and value['initial_exec_ns'] is not None and value['prepared_ns'] > 0
                and value['prepared_ns'] < value['initial_exec_ns'] <= value['observed_through_ns']
                and bool(value['possible_cpus']) and cpus == value['possible_cpus']
                and all(counter['lost'] == 0 for counter in counters)
                and value['target_tasks'] > 0
                and value['descendant_cleanup'] == 'root_thread_group_only',
                'incomplete native lifecycle certificate')
    return value


class NativeObservation:
    """Retain one emitted library and native observer token through explicit close."""

    def __init__(self, module_owner, workload, reader):
        """Borrow an independently admitted module/workload through the caller's owner.

        ``module_owner.library`` is already loaded; ``validate`` rechecks its full
        admitted source/artifact stack. This facade never loads an ambient path.
        The caller keeps all three owners alive until explicit observation close.
        """
        self.image = module_owner
        self.workload, self.reader = workload, reader
        self.library = module_owner.library
        self.token = ctypes.c_uint64(0)
        self.buffer = (ctypes.c_uint8 * MAX_REPORT)()
        self.written = ctypes.c_size_t(0)
        self.supervisor = os.getpid()
        self.original = self.original_pid = None
        self.prepared = self.closed = False
        self.final = self.final_raw = None
        self._initial_raw = self._initial_sha256 = None
        prefix = 'iroha_process_root_observer_'
        self.prepare_fn = getattr(self.library, prefix+'prepare_v1')
        self.prepare_fn.argtypes = [ctypes.POINTER(ctypes.c_uint64)]
        self.bind_fn = getattr(self.library, prefix+'bind_v1')
        self.bind_fn.argtypes = [ctypes.c_uint64, ctypes.c_uint32]
        self.poll_fn = getattr(self.library, prefix+'poll_v1')
        self.finish_fn = getattr(self.library, prefix+'finish_v1')
        for function in (self.poll_fn, self.finish_fn):
            function.argtypes = [ctypes.c_uint64, ctypes.POINTER(ctypes.c_uint8),
                                 ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
        self.close_fn = getattr(self.library, prefix+'close_v1')
        self.close_fn.argtypes = [ctypes.c_uint64]
        for function in (self.prepare_fn, self.bind_fn, self.poll_fn, self.finish_fn, self.close_fn):
            function.restype = ctypes.c_int32
        self.image.validate()
        self.workload.validate()
        _RETAINED_MODULES.append(self)

    def _caller(self):
        """Keep all native ownership operations on the preparing main thread."""
        require(os.getpid() == self.supervisor == threading.get_native_id(), 'observer caller changed')

    def prepare(self):
        """Install the token before acquisition; nonzero errors never authorize spawn."""
        self._caller()
        require(self.token.value == 0 and not self.closed, 'observer already prepared')
        status = self.prepare_fn(ctypes.byref(self.token))
        require(status in STATUSES, 'unknown prepare status')
        require(not self.token.value or self.token.value & TAG, 'observer token kind')
        self.prepared = status == OK
        require(not self.prepared or self.token.value != 0, 'missing successful observer token')
        return status

    def bind_original(self, child: subprocess.Popen):
        """Join only the returned Popen to its already observed birth, without lookup."""
        self._caller()
        require(self.prepared and not self.closed and isinstance(child, subprocess.Popen),
                'bind requires original returned Popen')
        require(self.original is None or self.original is child, 'Popen owner substituted')
        require(unsigned(child.pid, (1 << 32)-1) and child.pid > 1, 'returned child PID extent')
        require(self.original_pid in (None, child.pid), 'original returned PID changed')
        self.original = child
        self.original_pid = child.pid
        status = self.bind_fn(self.token.value, child.pid)
        require(status in STATUSES, 'unknown bind status')
        return status

    def capture_initial(self):
        """Own the actual reader call, both clock endpoints, and complete initial facts."""
        self._caller()
        require(self.original is not None and self.original.pid == self.original_pid
                and self._initial_raw is None and not self.closed, 'invalid initial capture state')
        self.image.validate()
        self.workload.validate()
        start = time.monotonic_ns()
        sample = self.reader.sample(self.original_pid, self.workload)
        end = time.monotonic_ns()
        value = {'sample': sample, 'started_ns': start, 'finished_ns': end}
        self._validate_initial(value)
        raw = json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
        require(len(raw) <= MAX_REPORT, 'initial sample extent')
        self._initial_raw = raw
        self._initial_sha256 = hashlib.sha256(raw).hexdigest()
        return json.loads(raw)

    def _validate_initial(self, value):
        """Rejoin every complete original lifetime/image field and both sample times."""
        require(type(value) is dict and set(value) == {'sample', 'started_ns', 'finished_ns'},
                'initial capture schema')
        require(unsigned(value['started_ns']) and unsigned(value['finished_ns'])
                and 0 < value['started_ns'] <= value['finished_ns'], 'initial capture clock order')
        sample = value['sample']
        require(type(sample) is dict and set(sample) == {'identity', 'cpu_time_ns', 'cpu_counter_unit_ns', 'rss_bytes'},
                'initial sample schema')
        identity = sample['identity']
        require(type(identity) is dict and set(identity) == {'pid', 'ppid', 'pgid', 'uid', 'birth',
                'executable_path', 'executable_sha256'}, 'initial identity inventory')
        require(type(identity['pid']) is int and identity['pid'] == self.original_pid
                and type(identity['ppid']) is int and identity['ppid'] == self.supervisor
                and unsigned(identity['pgid'], observer.PID_MAX) and identity['pgid'] > 0
                and type(identity['uid']) is int and identity['uid'] == os.geteuid(),
                'initial original process identity')
        birth = identity['birth']
        require(type(birth) is dict and set(birth) == {'kind', 'boot_id', 'start_ticks'}
                and birth['kind'] == 'linux_proc' and birth['boot_id'] == self.reader.boot_id
                and unsigned(birth['start_ticks']) and birth['start_ticks'] > 0,
                'initial original birth identity')
        require(identity['executable_path'] == str(self.workload.path)
                and identity['executable_sha256'] == self.workload.sha256, 'initial original executable identity')
        require(unsigned(sample['cpu_time_ns'])
                and sample['cpu_counter_unit_ns'] == 1_000_000_000 // self.reader.clock_ticks
                and type(sample['cpu_counter_unit_ns']) is int
                and unsigned(sample['rss_bytes'], observer.MAX_EXACT_INTEGER) and sample['rss_bytes'] > 0,
                'initial resource counters')

    def initial(self):
        """Return a fresh projection of the adapter-owned, byte-frozen live sample."""
        require(self._initial_raw is not None and hashlib.sha256(self._initial_raw).hexdigest()
                == self._initial_sha256, 'initial captured bytes changed or missing')
        value = json.loads(self._initial_raw, object_pairs_hook=_pairs)
        self._validate_initial(value)
        return value

    def join_wait(self, waited_pid, wait_status):
        """Join the stored genuine sample and frozen native report to original wait4."""
        self._caller()
        require(self.original is not None and self.original.pid == self.original_pid
                and waited_pid == self.original_pid, 'wait4 differs from original Popen')
        require(self.final_raw is not None, 'native certificate not finalized before wait4')
        value = report(self.final_raw, self.supervisor, self.original_pid)
        require(value['lifecycle_certified'] and value['finalized'], 'lifecycle was not certified')
        initial = self.initial()
        self.image.validate()
        self.workload.validate()
        require(value['initial_exec_ns'] <= initial['started_ns'] <= initial['finished_ns']
                <= value['observed_through_ns'], 'live sample is outside certified image interval')
        terminal = value['terminal']
        require(type(wait_status) is int, 'invalid wait status')
        if terminal['code'] == os.CLD_EXITED:
            require(os.WIFEXITED(wait_status) and os.WEXITSTATUS(wait_status) == terminal['status'],
                    'wait4 exit differs from WNOWAIT')
        else:
            require(os.WIFSIGNALED(wait_status) and os.WTERMSIG(wait_status) == terminal['status']
                    and bool(os.WCOREDUMP(wait_status)) == (terminal['code'] == os.CLD_DUMPED),
                    'wait4 signal differs from WNOWAIT')
        return {'lifecycle_and_initial_image_joined': True,
                'command_succeeded': os.WIFEXITED(wait_status) and os.WEXITSTATUS(wait_status) == 0}

    def read(self, *, finish=False):
        """Return native status plus facts; finish never reaps or makes exit101 succeed."""
        self._caller()
        require(self.token.value != 0 and not self.closed, 'closed observer token')
        self.written.value = 0
        function = self.finish_fn if finish else self.poll_fn
        status = function(self.token.value, self.buffer, MAX_REPORT, ctypes.byref(self.written))
        require(status in STATUSES and 0 <= self.written.value <= MAX_REPORT, 'native result extent')
        if status < 0:
            require(self.written.value == 0, 'error returned unadmitted report bytes')
            return status, None, None
        raw = bytes(self.buffer[:self.written.value])
        parsed = report(raw, self.supervisor, self.original.pid if self.original is not None else None)
        if self.final_raw is not None:
            require(raw == self.final_raw, 'frozen native certificate changed')
        if finish and status == OK:
            require(parsed['finalized'], 'finish returned incomplete lifecycle')
            self.final_raw, self.final = raw, parsed
        return status, parsed, raw

    def close(self):
        """Explicitly close observer resources; failure retains the original module."""
        self._caller()
        require(not self.closed, 'observer already closed')
        status = self.close_fn(self.token.value) if self.token.value else OK
        require(status in STATUSES, 'unknown close status')
        if status == OK:
            self.closed = True
            # Library remains a normal Python object until the caller drops this
            # adapter. An unknown native close never reaches this release path.
            # These are borrowed from the enclosing original-child owner.
            # It closes reader/workload/module only after this explicit boundary.
            _RETAINED_MODULES.remove(self)
        return status
