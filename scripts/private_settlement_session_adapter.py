"""Native worker and ordered measurement owner for a retained benchmark session.

Requires the canonical prepared request/start, exact admitted native executable
images, owner-only records, and mandatory semantic/packet owners. No packet-byte
estimate or successful-result validator is supplied as a default. This module
never signals a process. A failed or expired observation retains incomplete
facts; the caller must not continue the registered campaign.

The session runtime supplies the admitted native worker and mandatory complete
packet-window owner; the joined closure reducer replays their exact evidence.
Python protocol fixtures cannot qualify native Iroha benchmark execution.
"""
from __future__ import annotations

import ctypes
import errno
import hashlib
import os
from pathlib import Path
import re
import select
import stat
import subprocess
import sys
import tempfile
import time
from typing import Any, BinaryIO, Callable, Mapping

import private_settlement_attempt_accounting as accounting
import private_settlement_session_control as control
import private_settlement_process_observer as process_observer
import private_settlement_network_observer as network_observer
from private_settlement_session_bridge import _BeforeWrite

WORKER_TEST = ('nexus::atomic_private_settlement_localnet::'
               'atomic_private_settlement_real_process_benchmark_session_harness')


def remaining_seconds(deadline: int) -> float:
    """Use an existing monotonic deadline without extending or replacing it."""
    control.require(type(deadline) is int, 'deadline is not an exact monotonic timestamp')
    remaining = (deadline-time.monotonic_ns())/1_000_000_000
    if remaining <= 0:
        raise TimeoutError('registered session or attempt deadline exhausted')
    return remaining


def start_deadline(start: Mapping[str, Any], budget_ms: int) -> int:
    """Subtract time already spent since the durable start from its sealed budget."""
    control.unsigned(budget_ms)
    began = control.unsigned(start['started_ns'])
    anchor = time.monotonic_ns()
    now = time.time_ns()
    control.require(budget_ms > 0 and began <= now and now-began < budget_ms*1_000_000,
                    'durable start is future-dated or its budget has elapsed')
    return anchor + budget_ms*1_000_000 - (now-began)


class DeadlinePipe:
    """Bound each actual read/write by the currently active sealed deadline."""

    def __init__(self, stream: BinaryIO, deadline: Callable[[], int]):
        self.stream, self.deadline = stream, deadline

    def read(self, length: int) -> bytes:
        ready, _, _ = select.select([self.stream.fileno()], [], [], remaining_seconds(self.deadline()))
        if not ready:
            raise TimeoutError('control read exceeded its registered deadline')
        return os.read(self.stream.fileno(), length)

    def write(self, raw: bytes | memoryview) -> int:
        _, ready, _ = select.select([], [self.stream.fileno()], [], remaining_seconds(self.deadline()))
        if not ready:
            raise TimeoutError('control write exceeded its registered deadline')
        # A bounded nonblocking-sized write avoids blocking after select says
        # that only part of a large retained frame fits in the pipe.
        return os.write(self.stream.fileno(), raw[:4096])

    def flush(self) -> None:
        self.stream.flush()


def native_group_snapshot(group: int) -> dict[str, Any]:
    """Read bounded numeric kernel process-group membership without signals."""
    control.require(type(group) is int and 1 < group < 1 << 31, 'invalid owned process group')
    command = ['/bin/ps', '-axo', 'pid=,pgid=']
    # The utility is a fixed native binary; preserve its actual image binding.
    path = Path(command[0]).resolve(strict=True)
    before = process_observer.metadata(path.stat())
    with path.open('rb') as stream:
        utility = hashlib.file_digest(stream, 'sha256').hexdigest()
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        completed = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=output, stderr=errors, timeout=10)
        control.require(completed.returncode == 0 and errors.tell() == 0
                        and output.tell() <= 4*1024*1024,
                        'process-group inventory was unavailable or exceeded its bound')
        output.seek(0)
        raw = output.read(4*1024*1024+1)
    rows = []
    for line in raw.splitlines():
        match = re.fullmatch(rb'\s*([1-9][0-9]*)\s+([0-9]+)\s*', line)
        control.require(match is not None, 'process-group row is malformed')
        pid, pgid = map(int, match.groups())
        if pgid == group:
            rows.append(pid)
    control.require(len(rows) == len(set(rows))
                    and process_observer.metadata(path.stat()) == before,
                    'process-group owner or utility changed')
    return {'process_group': group, 'members': sorted(rows), 'utility_sha256': utility,
            'observed_monotonic_ns': time.monotonic_ns()}


def native_absence(pid: int, reader: Any) -> dict[str, Any]:
    """Require explicit kernel absence; unreadable, live or reused PIDs reject."""
    control.require(type(pid) is int and 1 < pid < 1 << 31, 'invalid observed process PID')
    if sys.platform == 'darwin':
        info = reader.native._BsdInfo()
        ctypes.set_errno(0)
        count = reader.reader.lib.proc_pidinfo(pid, 3, 0, ctypes.byref(info), ctypes.sizeof(info))
        control.require(count == 0 and ctypes.get_errno() == errno.ESRCH,
                        'declared process is present, reused or its absence is unavailable')
    elif sys.platform.startswith('linux'):
        try:
            os.stat(str(pid), dir_fd=reader.proc, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise control.SessionProtocolError('declared process is present or reused')
    else:
        raise control.SessionProtocolError('no native process-absence owner for this platform')
    return {'pid': pid, 'kernel_absence_observed': True, 'observed_monotonic_ns': time.monotonic_ns()}


class NativeWorkerStartFailure(RuntimeError):
    """Preserve a spawned child's actual owner when start publication fails."""

    def __init__(self, worker):
        self.worker = worker
        super().__init__('native worker spawned but its start receipt could not be published')


class NativeWorker:
    """Own one exact child, its dedicated descriptors and natural terminal.

Spawn/timeout failure does not kill a child or infer cleanup. close() closes
only this owner's descriptors; any still-running child is retained explicitly.
"""

    def __init__(self, command: list[str], environment: dict[str, str], *, cwd: Path,
                 image: process_observer.ExecutableImage, records: control.RecordDirectory,
                 prefix: str, deadline: Callable[[], int]):
        control.require(type(command) is list and command and Path(command[0]).resolve() == image.path,
                        'worker command does not use its admitted executable')
        control.require(all(type(part) is str and part and '\x00' not in part for part in command),
                        'worker command is malformed')
        self.image, self.records, self.prefix = image, records, prefix
        self.deadline, self.process, self.reaped = deadline, None, False
        self.physical_exit = None
        self.pipes = control.ChildControlPipes()
        self.stdout = self.stderr = None
        try:
            image.validate()
            read_fd, write_fd = self.pipes.child_fds
            env = dict(environment, APS_BENCHMARK_CONTROL_READ_FD=str(read_fd),
                       APS_BENCHMARK_CONTROL_WRITE_FD=str(write_fd))
            self.stdout = (records.path / prefix / 'worker.stdout.log').open('xb')
            self.stderr = (records.path / prefix / 'worker.stderr.log').open('xb')
            self.process = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                stdout=self.stdout, stderr=self.stderr, pass_fds=self.pipes.child_fds, start_new_session=True)
            self.pipes.child_spawn_finished()
            self.reader = DeadlinePipe(self.pipes.reader, lambda: self.deadline())
            self.writer = DeadlinePipe(self.pipes.writer, lambda: self.deadline())
            self.started = records.publish(prefix+'/worker-process-start.json', control.canonical({
                'command': command, 'pid': self.process.pid, 'parent_pid': os.getpid(),
                'process_group': self.process.pid, 'started_ns': time.time_ns(),
                'worker_sha256': image.sha256}))
        except BaseException as error:
            self.pipes.child_spawn_finished()
            self.close()
            if self.process is not None:
                raise NativeWorkerStartFailure(self) from error
            raise

    def finish(self, declarations: list[dict[str, Any]], reader: Any) -> dict[str, Any]:
        """Join natural wait to explicit kernel absence and empty owned group."""
        control.require(self.process is not None and not self.reaped, 'worker has no pending natural wait')
        code = self.process.wait(timeout=remaining_seconds(self.deadline()))
        self.reaped = True
        self.image.validate()
        for stream in (self.stdout, self.stderr):
            stream.flush(); os.fsync(stream.fileno())
        before = native_group_snapshot(self.process.pid)
        control.require(before['members'] == [], 'worker group has unreaped live descendants')
        pids = {self.process.pid, *(row['pid'] for row in declarations)}
        absent = [native_absence(pid, reader) for pid in sorted(pids)]
        after = native_group_snapshot(self.process.pid)
        control.require(after['members'] == [], 'worker group changed during cleanup observation')
        self.physical_exit = {'worker_process_start': self.started, 'worker_pid': self.process.pid,
                'worker_exit_code': code, 'worker_wait_completed': True,
                'group_before': before, 'kernel_absences': absent, 'group_after': after,
                'worker_sha256': self.image.sha256, 'worker_image_unchanged': True}
        return self.physical_exit

    def close(self) -> None:
        """Release pipe/log handles; never signal or claim unobserved exit."""
        self.pipes.close()
        for stream in (self.stdout, self.stderr):
            if stream is not None and not stream.closed:
                stream.close()


def spawn_native_session(prepared, started, *, records, cwd, images, runtime_root, outer_timeout_ms):
    """Construct the actual native session entrypoint and mandatory environment.

The compiled worker owns one complete network; no old one-shot request/result
environment is installed and no unsupported-entrypoint fallback is attempted.
"""
    import private_settlement_smoke_campaign as campaign
    session = control.identity(prepared['identity'])
    raw = records.read(prepared['reference'])
    start_raw = records.read(started)
    start = control.decode(start_raw)
    control.require(raw == control.canonical(prepared['request'])
                    and all(start.get(key) == value for key, value in session.items())
                    and start.get('request') == prepared['reference'], 'native spawn request/start differs')
    runtime_root = Path(runtime_root)
    info = runtime_root.lstat()
    control.require(runtime_root.resolve(strict=True) == runtime_root and stat.S_ISDIR(info.st_mode)
                    and stat.S_IMODE(info.st_mode) == 0o700 and info.st_uid == os.geteuid(),
                    'session runtime root is not canonical owner-only storage')
    for image in images.values(): image.validate()
    deadline = start_deadline(start, outer_timeout_ms)
    environment = campaign.sanitized_environment()
    environment.update(RAYON_NUM_THREADS=str(campaign.RAYON_WORKER_THREADS),
        IROHA_TEST_REQUIRE_NETWORK='1', IROHA_TEST_NETWORK_START_ATTEMPTS='1',
        IROHA_TEST_SKIP_BUILD='1', IROHA_TEST_BUILD_PROFILE='release',
        TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL=str(images['validator'].path),
        APS_REAL_PROCESS_VALIDATOR_SHA256=images['validator'].sha256,
        APS_BENCHMARK_SESSION_ROOT=str(records.path),
        APS_BENCHMARK_SESSION_REQUEST=prepared['reference']['path'],
        APS_BENCHMARK_SESSION_REQUEST_SHA256=prepared['reference']['sha256'],
        APS_BENCHMARK_SESSION_STARTED=started['path'],
        APS_BENCHMARK_SESSION_STARTED_SHA256=started['sha256'], TMPDIR=str(runtime_root))
    command = [str(images['worker'].path), WORKER_TEST, '--exact', '--ignored', '--nocapture', '--test-threads=1']
    return NativeWorker(command, environment, cwd=Path(cwd), image=images['worker'], records=records,
                        prefix=f"sessions/{session['session_id']}", deadline=lambda: deadline)


def terminal_envelope(raw: bytes, request: dict[str, Any], request_sha256: str) -> dict[str, Any]:
    """Validate the real native wrapper without fabricating per-attempt exit.

Use canonical shared closed fields/vocabularies. Success payload semantics and
economic-vector verification remain mandatory at the materialization owner.
"""
    from private_settlement_release_runner import strict_json_loads
    terminal = strict_json_loads(raw.decode(), 'session Rust terminal')
    accounting.exact_fields(terminal, accounting.RUST_TERMINAL_FIELDS, 'session Rust terminal')
    control.require(type(terminal['version']) is int and terminal['version'] == 1
                    and terminal['protocol'] == control.PROTOCOL
                    and terminal['request_sha256'] == request_sha256
                    and request['kind'] == 'benchmark'
                    and type(terminal['participants']) is int
                    and all(terminal[key] == request[key] for key in
                            ('request_id', 'invocation_nonce', 'commit', 'participants')),
                    'Rust terminal differs from its exact session request')
    elapsed = accounting.unsigned_milliseconds(terminal['elapsed_ms'], 'native elapsed')
    outcome = terminal['outcome']
    control.require(type(outcome) is dict, 'native outcome is not an object')
    kind = outcome.get('kind')
    if kind == 'succeeded':
        control.exact(outcome, {'kind', 'result'}, 'native success')
        value = accounting.exact_fields(outcome['result'], accounting.SUCCESS_RESULT_FIELDS, 'native result')
        control.require(type(value['version']) is int and type(value['participants']) is int
                        and all(value[key] == terminal[key] for key in
            ('version', 'protocol', 'request_id', 'invocation_nonce', 'request_sha256', 'commit', 'participants')),
            'native result header differs')
    elif kind == 'failed':
        control.exact(outcome, {'kind', 'stage', 'reason'}, 'native failure')
        control.require(outcome['stage'] == 'benchmark_worker'
                        and outcome['reason'] in accounting.WORKER_FAILURE_REASONS, 'unknown native failure')
    elif kind == 'timed_out':
        control.exact(outcome, {'kind', 'stage', 'budget_ms', 'elapsed_ms'}, 'native timeout')
        budget = accounting.unsigned_milliseconds(outcome['budget_ms'], 'native timeout budget', positive=True)
        duration = accounting.unsigned_milliseconds(outcome['elapsed_ms'], 'native timeout elapsed')
        control.require(outcome['stage'] in accounting.DEADLINE_STAGES and budget <= duration <= elapsed,
                        'native timeout is not within the declared invocation')
    else:
        raise control.SessionProtocolError('unknown native terminal outcome')
    return terminal


class NativeObservations:
    """Authenticate real ready endpoints and sample the complete retained scope."""

    def __init__(self, *, process_reader: Any, listener_reader: Any,
                 images: dict[str, process_observer.ExecutableImage]):
        self.reader, self.listeners, self.images = process_reader, listener_reader, images
        self.scope = None
        self.expected_endpoints = None

    def ready(self, ready: dict[str, Any], session: dict[str, Any], request: dict[str, Any],
              worker: NativeWorker, records: control.RecordDirectory) -> dict[str, Any]:
        fields = control.IDENTITY_FIELDS | {'network_id', 'genesis_sha256', 'configuration_sha256',
            'workload_manifest_sha256', 'activated_height', 'process_inventory', 'worker_pid', 'network_ports'}
        control.exact(ready, fields, 'native session ready')
        control.require(all(ready[key] == session[key] for key in control.IDENTITY_FIELDS)
                        and ready['worker_pid'] == worker.process.pid and type(ready['worker_pid']) is int
                        and ready['configuration_sha256'] == request['configuration_sha256']
                        and ready['workload_manifest_sha256'] == request['workload_manifest_sha256'],
                        'native readiness identity differs')
        control.digest(ready['genesis_sha256']); control.unsigned(ready['activated_height'])
        declarations = process_observer.benchmark_process_declarations(ready['process_inventory'],
            participants=request['participants'], worker_pid=worker.process.pid, adapter_pid=os.getpid(),
            process_group=worker.process.pid, commit=request['commit'],
            validator_sha256=self.images['validator'].sha256, worker_sha256=self.images['worker'].sha256)
        self.scope = process_observer.ProcessScope(declarations, self.reader, self.images)
        ref = control.reference(ready['network_ports'])
        control.require(ref['path'] == f"sessions/{session['session_id']}/network-ports.json",
                        'port document has an unexpected session locator')
        raw = records.read(ref)
        self.expected_endpoints = network_observer.declared_listener_rows(control.decode(raw),
            session=session, network_id=ready['network_id'], participants=request['participants'], scope=self.scope)
        listener = self.observe_listeners()
        control.require(records.read(ref) == raw, 'network ports changed during kernel authentication')
        return {'process_scope': self.scope.initial, 'listeners': listener, 'network_ports': ref}

    def observe_listeners(self) -> dict[str, Any]:
        return network_observer.observe_declared_listeners(self.scope, self.expected_endpoints, self.listeners)

    def window(self, *, records, prefix, budget_ms, deadline_ns):
        return process_observer.ProcessResourceWindow(self.scope, records=records, prefix=prefix,
            outer_timeout_ms=budget_ms, deadline_monotonic_ns=deadline_ns)

    def validate_window(self, value, *, records, budget_ms):
        expected = [{key: row[key] for key in ('label', 'identity', 'cpu_counter_unit_ns')}
                    for row in self.scope.initial['processes']]
        return process_observer.validate_resource_window(value, records=records,
            outer_timeout_ms=budget_ms, expected_processes=expected)


class SessionAdapter:
    """Pump unchanged runner/worker chains and local measurement handshakes.

``semantics`` must implement validate_ready, materialize_terminal and
validate_acceptance using actual native economic-vector/result validators.
``packets`` supplies PacketCaptureOwner and validate_packet_window with retained
complete capture evidence. Neither dependency has a permissive default.
"""

    def __init__(self, prepared, started, *, records, runner_reader, runner_writer,
                 worker: NativeWorker, observations: NativeObservations, packets: Any, semantics: Any,
                 outer_timeout_ms: int):
        self.records, self.worker, self.observations = records, worker, observations
        self.packets, self.semantics = packets, semantics
        self.prepared = control.decode(control.canonical(prepared))
        self.session = control.identity(self.prepared['identity'])
        self.request_raw = records.read(self.prepared['reference'])
        control.require(self.request_raw == control.canonical(self.prepared['request']), 'session request changed')
        self.started_ref, self.started_raw = dict(started), records.read(started)
        start = control.decode(self.started_raw)
        control.require(all(start.get(key) == value for key, value in self.session.items())
                        and start.get('request') == self.prepared['reference'], 'session start identity differs')
        self.request = self.prepared['request']
        self.rows = self.request['attempts']
        self.prefix = f"sessions/{self.session['session_id']}"
        self.outer_timeout_ms = outer_timeout_ms
        self.current_deadline = start_deadline(start, outer_timeout_ms)
        worker.deadline = lambda: self.current_deadline
        self.rr = DeadlinePipe(runner_reader, lambda: self.current_deadline)
        self.rw = DeadlinePipe(runner_writer, lambda: self.current_deadline)
        self.chains = {}
        for channel, directions in (('runner_adapter', ('owner_to_child', 'child_to_owner')),
                                    ('adapter_worker', ('child_to_owner', 'owner_to_child'))):
            for direction in directions:
                self.chains[channel, direction] = control.ControlChain(self.session, started['sha256'],
                    channel, direction, records, observer='adapter', journal_prefix=self.prefix+'/control')
        self.gate = control.AttemptGate(self.session, len(self.rows))
        self.active = self.measurement = self.ready = None
        self.windows = {}
        self.packet_owners = {}
        self.resource_owners = {}
        self.packet_start_error = None
        self.preceding_acceptance = None
        self.used = False

    def _stable(self):
        control.require(self.records.read(self.prepared['reference']) == self.request_raw
                        and self.records.read(self.started_ref) == self.started_raw, 'session bindings changed')

    def _publish(self, path, value):
        return self.records.publish(path, control.canonical(value))

    def _receive(self, channel):
        direction = 'owner_to_child' if channel == 'runner_adapter' else 'child_to_owner'
        stream = self.rr if channel == 'runner_adapter' else self.worker.reader
        return self.chains[channel, direction].receive(stream)

    def _source_message(self, message):
        """Join the sender's exact journal to this observer's consumed bytes."""
        value = message.decoded()
        owner, child = control.CHANNEL_ENDPOINTS[value['channel']]
        sender = owner if value['direction'] == 'owner_to_child' else child
        source = dict(message.binding, path=(f"{self.prefix}/control/{sender}.{value['channel']}."
                     f"{value['direction']}.{value['sequence']:020d}.frame"))
        control.require(self.records.read(source) == message.raw, 'sender journal differs from consumed control bytes')
        return control.RetainedMessage(message.raw, source)

    def _forward(self, message, additions=None, before=None):
        source = self._source_message(message)
        value = message.decoded()
        channel = 'adapter_worker' if value['channel'] == 'runner_adapter' else 'runner_adapter'
        stream = self.worker.writer if channel == 'adapter_worker' else self.rw
        chain = self.chains[channel, value['direction']]
        if before is not None:
            stream = _BeforeWrite(stream, before)
        forwarded = chain.send(stream, value['kind'], {**value['payload'], **(additions or {})},
                               forwarded_from=source.binding)
        control.verify_forwarded(forwarded, source)
        return forwarded

    def _local(self, kind, payload):
        return self.chains['adapter_worker', 'owner_to_child'].send(self.worker.writer, kind, payload)

    def _attempt_fields(self):
        return {key: self.active[key] for key in control.ATTEMPT_FIELDS}

    def _marker(self, message, boundary):
        payload = message.decoded()['payload']
        control.require(all(payload[key] == self.active[key] for key in control.ATTEMPT_FIELDS),
                        'measurement belongs to another attempt')
        reference = payload['marker']
        control.require(reference['path'] == self.active['output_directory']+
                        f'/evidence/benchmark-protocol/measurement-{boundary}.json', 'measurement marker locator differs')
        raw = self.records.read(reference)
        marker = control.decode(raw)
        control.exact(marker, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {'boundary'}, 'measurement marker')
        control.require(marker == {**self.session, **self._attempt_fields(), 'boundary': boundary},
                        'measurement marker fields differ')
        return dict(reference)

    def _begin(self, message, started):
        control.require(self.measurement is None and self.active['attempt_id'] not in self.windows,
                        'measurement began twice')
        received = time.monotonic_ns()
        marker = self._marker(message, 'ready')
        before = self._publish(self.active['output_directory']+'/listeners-before.json',
                               self.observations.observe_listeners())
        prefix = self.active['output_directory']+'/process-observations'
        directory = self.records.path/prefix
        directory.mkdir(mode=0o700)
        resources = self.observations.window(records=self.records, prefix=prefix,
            budget_ms=started['outer_timeout_ms'], deadline_ns=self.current_deadline)
        self.resource_owners[self.active['attempt_id']] = resources
        self.measurement = {'ready_marker': marker, 'listener_before': before, 'resources': resources,
                            'packet': None, 'ready_received_ns': received,
                            'baseline_published_ns': time.monotonic_ns(), 'budget_ms': started['outer_timeout_ms']}
        (self.records.path/self.active['output_directory']/'packet-capture').mkdir(mode=0o700)
        try:
            packet = self.packets.PacketCaptureOwner.begin(records=self.records, prefix=self.active['output_directory']+'/packet-capture',
                session=self.session, attempt=self._attempt_fields(),
                ready_marker=marker, ports_reference=self.ready['network_ports'], listener_before=before,
                resource_baseline=resources.baseline_reference, deadline_ns=self.current_deadline)
        except self.packets.PacketCaptureIncomplete as error:
            self.packet_owners[self.active['attempt_id']] = error.owner
            self.packet_start_error = error
            self.measurement['packet_ref'] = error.owner.incomplete_reference
            # begin already attempted its established cleanup. Keep packet=None
            # so outer measurement cleanup cannot repeat it or signal again.
            raise
        self.packet_owners[self.active['attempt_id']] = packet
        self.measurement['packet'] = packet
        self._stable()
        self._local('measurement_begin', {**self._attempt_fields(), 'marker': marker,
                                          'process_observation': resources.baseline_reference})
        self.measurement['begin_written_ns'] = time.monotonic_ns()

    def _finish(self, message):
        control.require(self.measurement is not None, 'measurement ended before beginning')
        finished_received = time.monotonic_ns()
        marker = self._marker(message, 'finished')
        measurement = self.measurement
        measurement['packet'].end(marker)
        packet_end_returned = time.monotonic_ns()
        resources = measurement['resources'].finish()
        measurement['resource_result'] = resources
        if not measurement['resources']._thread.is_alive():
            del self.resource_owners[self.active['attempt_id']]
        resource_ref = self._publish(self.active['output_directory']+'/process-window.json', resources)
        measurement['resource_ref'] = resource_ref
        resources_finished = time.monotonic_ns()
        # Listener queries and expensive independent replay follow both ends.
        after = self._publish(self.active['output_directory']+'/listeners-after.json',
                              self.observations.observe_listeners())
        packet_ref = measurement['packet'].finish(listener_after=after, resource_window=resources)
        measurement['packet_ref'] = packet_ref
        control.reference(packet_ref); self.records.read(packet_ref)
        self.observations.validate_window(resources, records=self.records, budget_ms=measurement['budget_ms'])
        self.packets.validate_packet_window(packet_ref, records=self.records, deadline_ns=self.current_deadline, session=self.session, attempt=self._attempt_fields(),
            ready_marker=measurement['ready_marker'], finished_marker=marker,
            ports_reference=self.ready['network_ports'], listener_before=measurement['listener_before'],
            listener_after=after, resource_window=resources,
            expected_utility=measurement['packet'].utility)
        value = {**self.session, **self._attempt_fields(), 'kind': 'benchmark_measurement_window',
            'ready_marker': measurement['ready_marker'], 'finished_marker': marker,
            'process_resource_window': resource_ref, 'packet_window': packet_ref,
            'listener_before': measurement['listener_before'], 'listener_after': after,
            'boundaries': {key: measurement[key] for key in
                           ('ready_received_ns', 'baseline_published_ns', 'begin_written_ns')}}
        value['boundaries'].update(finished_received_ns=finished_received,
            packet_end_returned_ns=packet_end_returned, resources_finished_ns=resources_finished)
        ref = self._publish(self.active['output_directory']+'/measurement-window.json', value)
        self._stable()
        self.windows[self.active['attempt_id']] = ref
        self.measurement = None
        self._local('measurement_recorded', {**self._attempt_fields(), 'marker': marker, 'measurement_window': ref})

    def _abort_measurement(self, reason):
        if self.measurement is None:
            return
        value = self.measurement
        self.measurement = None
        resources = value.get('resource_result')
        if resources is None:
            resources = value['resources'].finish()
        if not value['resources']._thread.is_alive():
            self.resource_owners.pop(self.active['attempt_id'], None)
        ref = value.get('resource_ref') or self._publish(
            self.active['output_directory']+'/process-window-incomplete.json', resources)
        packet = value.get('packet_ref')
        if value['packet'] is not None and not value['packet'].closed:
            packet = value['packet'].abort(reason)
        self._publish(self.active['output_directory']+'/measurement-incomplete.json', {
            **self.session, **self._attempt_fields(), 'kind': 'benchmark_measurement_incomplete',
            'reason': reason, 'ready_marker': value['ready_marker'],
            'process_resource_window': ref, 'packet_capture_incomplete': packet})

    def _terminal(self, message):
        payload = message.decoded()['payload']
        raw = self.records.read(payload['worker_terminal'])
        terminal = control.decode(raw)
        fields = control.IDENTITY_FIELDS | {'kind', 'reason', 'accepted_request_ids', 'active_attempt_id',
            'last_owner_message_sha256', 'last_worker_message_sha256',
            'network_shutdown_observed', 'coordinator_reaped_observed'}
        control.exact(terminal, fields, 'worker terminal')
        control.require(all(terminal[key] == self.session[key] for key in control.IDENTITY_FIELDS)
                        and terminal['accepted_request_ids'] == self.gate.accepted
                        and terminal['kind'] in {'completed', 'failed', 'timed_out', 'incomplete'}
                        and terminal['last_owner_message_sha256'] == self.chains['adapter_worker', 'owner_to_child'].previous
                        and terminal['last_worker_message_sha256'] == message.decoded()['previous_message_sha256'],
                        'worker terminal changed identity, accepted prefix or control predecessor')
        for key in ('network_shutdown_observed', 'coordinator_reaped_observed'):
            control.require(type(terminal[key]) is bool, 'worker cleanup observation has an invalid type')
        complete = terminal['kind'] == 'completed'
        control.require(terminal['active_attempt_id'] == (None if complete or self.active is None
                                                        else self.active['attempt_id']),
                        'worker terminal changed the active attempt')
        control.require(not complete or (len(self.gate.accepted) == len(self.rows)
            and terminal['active_attempt_id'] is None and terminal['reason'] is None
            and terminal['network_shutdown_observed'] and terminal['coordinator_reaped_observed']),
            'worker completed before all accepted attempts and network cleanup')
        control.require(complete or terminal['reason'] in control.STOP_REASONS, 'unknown worker failure reason')
        self._abort_measurement('worker_terminal_before_measurement_finished')
        declarations = [] if self.observations.scope is None else list(self.observations.scope.declarations)
        lifetime = self.worker.finish(declarations, self.observations.reader)
        control.require((lifetime['worker_exit_code'] == 0) == complete,
                        'worker process exit contradicts its typed terminal')
        self._stable()
        control.require(self.records.read(payload['worker_terminal']) == raw, 'worker terminal changed during cleanup')
        lifecycle = self._publish(self.prefix+'/adapter-lifecycle.json', {
            **self.session, 'kind': 'benchmark_session_worker_lifecycle',
            'worker_terminal': payload['worker_terminal'], **lifetime})
        self._forward(message, {'adapter_lifecycle': lifecycle})
        return lifecycle

    def run(self):
        """Run one session exchange; any malformed evidence ends continuation."""
        control.require(not self.used, 'adapter cannot be reused')
        self.used = True
        try:
            ready_message = self._receive('adapter_worker')
            if ready_message.decoded()['kind'] == 'session_completed':
                return self._terminal(ready_message)
            control.require(ready_message.decoded()['kind'] == 'ready', 'worker did not start with readiness')
            ref = ready_message.decoded()['payload']['ready']
            raw = self.records.read(ref); self.ready = control.decode(raw)
            self.semantics.validate_ready(self.ready, self.request)
            observed = self.observations.ready(self.ready, self.session, self.request, self.worker, self.records)
            process_ref = self._publish(self.prefix+'/process-ready.json', {
                **self.session, 'kind': 'benchmark_session_process_ready', 'ready': ref, **observed})
            control.require(self.records.read(ref) == raw, 'ready changed during native authentication')
            self._forward(ready_message, {'process_observation': process_ref})
            self.gate.ready()
            for row in self.rows:
                incoming = self._receive('runner_adapter')
                if incoming.decoded()['kind'] == 'stop':
                    self.gate.stop(); self._forward(incoming)
                    return self._terminal(self._receive('adapter_worker'))
                self.active = row
                start_ref = incoming.decoded()['payload']['attempt_started']
                start_raw = self.records.read(start_ref); started = control.decode(start_raw)
                request_raw = self.records.read(row['request']); request = control.decode(request_raw)
                self.gate.dispatch(incoming, expected_request=request_raw, expected_start=start_raw, records=self.records)
                control.exact(started, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
                    'ordinal', 'session_started', 'request', 'outer_timeout_ms', 'started_ns',
                    'preceding_acceptance'}, 'durable attempt start')
                control.require(type(started['outer_timeout_ms']) is int
                                and started['outer_timeout_ms'] == self.outer_timeout_ms
                                and started['session_started'] == self.started_ref
                                and started['request'] == row['request']
                                and type(started['ordinal']) is int and started['ordinal'] > 0
                                and row['output_directory'] == f"attempts/{started['ordinal']:05}-{row['request_id']}"
                                and started['preceding_acceptance'] == self.preceding_acceptance,
                                'attempt changed its registered start, predecessor or outer timeout')
                self.current_deadline = start_deadline(started, started['outer_timeout_ms'])
                self._stable(); self._forward(incoming)
                while True:
                    message = self._receive('adapter_worker')
                    kind = message.decoded()['kind']
                    if kind == 'measurement_ready':
                        self._begin(message, started)
                    elif kind == 'measurement_finished':
                        self._finish(message)
                    elif kind == 'session_completed':
                        return self._terminal(message)
                    else:
                        control.require(kind == 'attempt_completed', 'unexpected worker attempt message')
                        native_ref = message.decoded()['payload']['rust_terminal']
                        native_raw = self.records.read(native_ref)
                        native = terminal_envelope(native_raw, request, row['request']['sha256'])
                        if self.measurement is not None:
                            control.require(native['outcome']['kind'] != 'succeeded', 'successful native result lacks end boundary')
                            self._abort_measurement('unsuccessful_attempt_before_end_boundary')
                        window = self.windows.get(row['attempt_id'])
                        control.require(native['outcome']['kind'] != 'succeeded' or window is not None,
                                        'successful native result has no complete measurement window')
                        additions = self.semantics.materialize_terminal(native_raw, request_raw,
                            native_ref=native_ref, measurement_window=window, ready=self.ready, attempt=row)
                        control.exact(additions, {'adapter_outcome', 'response'}, 'adapter result bindings')
                        control.require(additions['adapter_outcome'] is not None
                            and (additions['response'] is not None) == (native['outcome']['kind'] == 'succeeded'),
                            'adapter outcome or successful response inventory differs')
                        for reference in additions.values():
                            if reference is not None: self.records.read(reference)
                        control.require(self.records.read(native_ref) == native_raw, 'native result changed during validation')
                        forwarded = self._forward(message, additions)
                        self.gate.complete(forwarded)
                        break
                response = self._receive('runner_adapter')
                if response.decoded()['kind'] == 'stop':
                    self.gate.stop(); self._forward(response)
                    return self._terminal(self._receive('adapter_worker'))
                control.require(native['outcome']['kind'] == 'succeeded', 'unsuccessful attempt cannot be accepted')
                def validate_before_forward(_):
                    self.gate.accept(response, records=self.records,
                        validate=lambda bound: self.semantics.validate_acceptance(bound, row, window))
                    self._stable()
                self._forward(response, before=validate_before_forward)
                self.preceding_acceptance = self._source_message(response).binding
                self.active = None
            return self._terminal(self._receive('adapter_worker'))
        except BaseException as error:
            self.gate.phase = 'stopped'
            for chain in self.chains.values(): chain.poisoned = True
            try:
                self._abort_measurement('adapter_interrupted')
            finally:
                self._publish(self.prefix+'/adapter-incomplete.json', {
                    **self.session, 'kind': 'benchmark_session_adapter_incomplete',
                    'reason': type(error).__name__, 'accepted_request_ids': self.gate.accepted,
                    'active_attempt_id': None if self.active is None else self.active['attempt_id'],
                    'worker_exit_observed': self.worker.process.poll(),
                    'worker_may_still_be_running': self.worker.process.poll() is None})
            raise
        finally:
            self.worker.close()
