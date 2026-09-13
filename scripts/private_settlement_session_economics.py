"""Bind session economics to the admitted native Norito verifier.

The native entrypoint rederives the complete payment vector from the frozen
request. Python checks its exact inputs, actual exit, result and executable;
it does not implement an alternative economic codec or infer a worker exit.
The session semantic validator requires this execution receipt, and the
registered collector independently replays the retained native evidence.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re
import stat
import subprocess
import time

import private_settlement_session_control as control
import private_settlement_session_adapter as adapter

VERIFIER_TEST = ('nexus::atomic_private_settlement_localnet::'
                 'atomic_private_settlement_verify_benchmark_economic_vector')
MAX_LOG_BYTES = 1024 * 1024
INPUT_NAMES = ('session_request', 'benchmark_request', 'ready', 'workload_record')


def prepare_verification(prepared, attempt, *, records):
    """Publish exact native-verifier input references for one registered attempt."""
    session = control.identity(prepared['identity'])
    session_raw = records.read(prepared['reference'])
    control.require(session_raw == control.canonical(prepared['request'])
                    and prepared['reference']['sha256'] == session['session_request_sha256']
                    and prepared['reference']['path'] == f"sessions/{session['session_id']}/request.json",
                    'economic verification substitutes its session request')
    rows = prepared['request']['attempts']
    matches = [row for row in rows if row == attempt]
    control.require(len(matches) == 1, 'economic verification has no unique registered attempt')
    request_raw = records.read(attempt['request'])
    request = control.decode(request_raw)
    control.require(request['kind'] == 'benchmark'
                    and all(request.get(key) == attempt[key] for key in
                            control.ATTEMPT_FIELDS - {'attempt_id'})
                    and request['session_id'] == session['session_id']
                    and request['session_invocation_nonce'] == session['session_invocation_nonce']
                    and request['workload_manifest_sha256'] == prepared['request']['workload_manifest_sha256']
                    and request['participants'] == prepared['request']['participants'],
                    'economic verification substitutes its benchmark request')
    prefix = attempt['output_directory'] + '/evidence/benchmark-protocol'
    inputs = {'session_request': dict(prepared['reference']),
              'benchmark_request': dict(attempt['request']),
              'ready': records.locate(f"sessions/{session['session_id']}/ready.json"),
              'workload_record': records.locate(attempt['output_directory'] + '/evidence/matched-workload.json')}
    bound = {key: records.read(ref) for key, ref in inputs.items()}
    ready = control.decode(bound['ready'])
    control.require(all(ready.get(key) == value for key, value in session.items()),
                    'economic verification readiness belongs to another session')
    document = {'version': 1, 'protocol': control.PROTOCOL,
                'kind': 'benchmark_economic_vector_verification', **inputs}
    reference = records.publish(prefix + '/economic-vector-verification-request.json', control.canonical(document))
    control.require(all(records.read(inputs[key]) == value for key, value in bound.items()),
                    'economic verification inputs changed during preparation')
    return {'identity': session, 'attempt': control.decode(control.canonical(attempt)),
            'reference': reference, 'document': document, 'input_bytes': bound,
            'output_path': prefix + '/economic-vector-verification.json'}


def validate_vector_result(value, prepared, *, records):
    """Join the native result to every retained input and its full Norito bytes.

This function checks a result, not process provenance. Only an actual successful
NativeVectorInvocation may qualify it as an executed native verification.
"""
    fields = control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | set(INPUT_NAMES) | {
        'kind', 'verified', 'request_sha256', 'network_id', 'workload_manifest_sha256',
        'economic_vector_sha256', 'canonical_economic_vector_sha256',
        'primary_payment_count', 'monetary_movement_count', 'verification_request'}
    control.exact(value, fields, 'native economic verification')
    control.identity({key: value[key] for key in control.IDENTITY_FIELDS})
    control.attempt({key: value[key] for key in control.ATTEMPT_FIELDS})
    identity, attempt = prepared['identity'], prepared['attempt']
    control.require(all(value[key] == item for key, item in identity.items())
                    and all(value[key] == attempt[key] for key in control.ATTEMPT_FIELDS)
                    and value['kind'] == 'benchmark_economic_vector_verified'
                    and value['verified'] is True and value['verification_request'] == prepared['reference'],
                    'native economic verification identity differs')
    document = prepared['document']
    control.require(records.read(prepared['reference']) == control.canonical(document),
                    'native economic verification request changed')
    for name in INPUT_NAMES:
        control.require(value[name] == document[name]
                        and records.read(document[name]) == prepared['input_bytes'][name],
                        'native economic verification input changed')
    request = control.decode(prepared['input_bytes']['benchmark_request'])
    ready = control.decode(prepared['input_bytes']['ready'])
    workload = control.decode(prepared['input_bytes']['workload_record'])
    participants = request['participants']
    control.require(type(participants) is int and participants in (2, 3, 4, 8, 16)
                    and value['request_sha256'] == document['benchmark_request']['sha256']
                    and value['network_id'] == ready['network_id']
                    and value['workload_manifest_sha256'] == request['workload_manifest_sha256']
                    and type(value['primary_payment_count']) is int
                    and value['primary_payment_count'] == participants
                    and type(value['monetary_movement_count']) is int
                    and value['monetary_movement_count'] == participants + 1,
                    'native economic verification changes its network or payment counts')
    control.digest(value['economic_vector_sha256']); control.digest(value['canonical_economic_vector_sha256'])
    encoded = workload.get('canonical_economic_vector_hex')
    control.require(type(encoded) is str and 0 < len(encoded) <= control.MAX_FRAME_BYTES
                    and len(encoded) % 2 == 0 and re.fullmatch('[0-9a-f]+', encoded) is not None,
                    'native economic vector has invalid bounded Norito bytes')
    control.require(value['economic_vector_sha256'] == workload.get('economic_vector_sha256')
                    and value['canonical_economic_vector_sha256'] == hashlib.sha256(bytes.fromhex(encoded)).hexdigest(),
                    'native economic verification substitutes the complete encoded vector')
    return value


def native_test_passed(stdout: bytes, stderr: bytes) -> None:
    """Require the named verifier to execute once; zero-test success rejects."""
    control.require(len(stdout) <= MAX_LOG_BYTES and len(stderr) <= MAX_LOG_BYTES,
                    'native economic verifier logs exceed their bound')
    output = stdout.decode('utf-8')
    terminals = re.findall(r'^test result: .*$', output, re.MULTILINE)
    control.require(len(terminals) == 1 and re.fullmatch(
        r'test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in .+',
        terminals[0]) is not None
        and len(re.findall(r'^running 1 test\s*$', output, re.MULTILINE)) == 1
        and len(re.findall(r'^test '+re.escape(VERIFIER_TEST)+r' \.\.\. ok\s*$', output, re.MULTILINE)) == 1,
        'native economic verifier did not execute exactly one successful named test')


class NativeVectorIncomplete(RuntimeError):
    """Retain the actual child owner after an incomplete spawn or observation."""

    def __init__(self, invocation):
        self.invocation = invocation
        super().__init__('native economic verification incomplete; actual child owner retained')


class OwnedNativeLog:
    """Hold every output parent so native logs cannot follow redirected ancestors."""

    def __init__(self, records, name):
        control.reference({'path': name, 'sha256': '1'*64, 'bytes': 1})
        records.validate()
        self.records, self.name, self.stream = records, name, None
        self.parents, self.entries = [os.dup(records.fd)], []
        self.leaf = Path(name).name
        fd = -1
        try:
            for part in Path(name).parts[:-1]:
                parent = self.parents[-1]
                child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW,
                                dir_fd=parent)
                self.parents.append(child)
                info = os.fstat(child)
                control.require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0,
                                'native log parent is not owner-only')
                self.entries.append((parent, part, child, control._directory_identity(info)))
            fd = os.open(self.leaf, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW,
                         0o600, dir_fd=self.parents[-1])
            info = os.fstat(fd)
            self.inode = info.st_dev, info.st_ino
            self.stream = os.fdopen(fd, 'w+b'); fd = -1
            self.validate()
        except BaseException:
            if fd >= 0: os.close(fd)
            self.close()
            raise

    def validate(self):
        """Require exact held/named parents and a single private regular output."""
        self.records.validate()
        for parent, name, child, identity in self.entries:
            control.require(control._directory_identity(os.fstat(child)) == identity
                            == control._directory_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)),
                            'native log parent changed')
        held = os.fstat(self.stream.fileno())
        named = os.stat(self.leaf, dir_fd=self.parents[-1], follow_symlinks=False)
        control.require(stat.S_ISREG(held.st_mode) and held.st_nlink == 1
                        and held.st_uid == os.geteuid() and stat.S_IMODE(held.st_mode) & 0o077 == 0
                        and (held.st_dev, held.st_ino) == self.inode == (named.st_dev, named.st_ino)
                        and control._metadata(held) == control._metadata(named), 'native log output changed')

    def completed_bytes(self):
        """Read bounded bytes only after the child has naturally exited."""
        self.stream.flush(); os.fsync(self.stream.fileno()); self.validate()
        before = os.fstat(self.stream.fileno())
        control.require(before.st_size <= MAX_LOG_BYTES, 'native verifier log exceeds its bound')
        self.stream.seek(0)
        raw = self.stream.read(MAX_LOG_BYTES+1)
        control.require(len(raw) == before.st_size
                        and control._metadata(before) == control._metadata(os.fstat(self.stream.fileno())),
                        'native verifier log changed during its owned descriptor read')
        self.validate()
        return raw

    def fileno(self):
        """Pass the actual owned descriptor to the native child."""
        return self.stream.fileno()

    def close(self):
        """Close only this log and held parent descriptors."""
        if self.stream is not None and not self.stream.closed: self.stream.close()
        for fd in reversed(self.parents): os.close(fd)
        self.parents = []


class NativeVectorInvocation:
    """Own an offline native child, natural wait, and immutable execution receipt.

Timeout leaves this exact owner available for a later natural wait. No process
is signalled. The benchmark worker continues to own its original network.
"""

    def __init__(self, prepared, *, records, image, cwd: Path, deadline_ns: int):
        import private_settlement_smoke_campaign as campaign
        self.prepared, self.records, self.image = prepared, records, image
        self.deadline_ns, self.process, self.reaped = deadline_ns, None, False
        self.physical_exit = None
        self.exit_observation, self.exit_reference = None, None
        self.stdout = self.stderr = None
        self.prefix = str(Path(prepared['output_path']).parent) + '/native-vector-process'
        self.command = [str(image.path), VERIFIER_TEST, '--exact', '--ignored', '--nocapture', '--test-threads=1']
        adapter.remaining_seconds(deadline_ns)
        image.validate()
        self._stable_inputs()
        self.launch = records.publish(self.prefix + '-launch.json', control.canonical({
            'verification_request': prepared['reference'], 'command': self.command,
            'executable_sha256': image.sha256, 'started_ns': time.time_ns(),
            'started_monotonic_ns': time.monotonic_ns(), 'deadline_monotonic_ns': deadline_ns}))
        try:
            self.stdout = OwnedNativeLog(records, self.prefix + '.stdout.log')
            self.stderr = OwnedNativeLog(records, self.prefix + '.stderr.log')
            environment = campaign.sanitized_environment()
            environment.update(APS_BENCHMARK_SESSION_ROOT=str(records.path),
                APS_BENCHMARK_VECTOR_REQUEST=prepared['reference']['path'],
                APS_BENCHMARK_VECTOR_REQUEST_SHA256=prepared['reference']['sha256'])
            self.process = subprocess.Popen(self.command, cwd=cwd, env=environment,
                stdin=subprocess.DEVNULL, stdout=self.stdout, stderr=self.stderr, start_new_session=True)
            self.started = records.publish(self.prefix + '-start.json', control.canonical({
                'launch': self.launch, 'pid': self.process.pid, 'parent_pid': os.getpid(),
                'process_group': self.process.pid, 'spawned_monotonic_ns': time.monotonic_ns()}))
        except BaseException as error:
            if self.process is None:
                self.close()
            raise NativeVectorIncomplete(self) from error

    def _stable_inputs(self):
        p = self.prepared
        control.require(self.records.read(p['reference']) == control.canonical(p['document'])
                        and all(self.records.read(p['document'][name]) == raw
                                for name, raw in p['input_bytes'].items()),
                        'native economic verifier input bytes changed')

    def observe_exit(self):
        """Retain natural termination even after the qualification deadline.

        After the deadline this is a zero-wait observation. A still-live process
        remains pending, while a late exit is retained without becoming success.
        This operation also remains available after later validation failures.
        """
        control.require(self.process is not None, 'native economic verifier has no child')
        if not self.reaped:
            try:
                code = self.process.wait(timeout=max(0, (self.deadline_ns-time.monotonic_ns())/1e9))
            except BaseException as error:
                raise NativeVectorIncomplete(self) from error
            self.reaped = True
            self.exit_observation = {'launch': self.launch, 'pid': self.process.pid,
                'exit_code': code, 'natural_wait_observed': True,
                'observed_monotonic_ns': time.monotonic_ns()}
        if self.exit_reference is None:
            self.exit_reference = self.records.publish(self.prefix+'-exit.json', control.canonical(self.exit_observation))
        control.require(self.records.read(self.exit_reference) == control.canonical(self.exit_observation),
                        'retained native exit observation changed')
        return self.exit_reference

    def finish(self, *, process_reader):
        """Accept output only after natural exit, empty group, and native absence."""
        exit_reference = self.observe_exit()
        code = self.exit_observation['exit_code']
        logs = [stream.completed_bytes() for stream in (self.stdout, self.stderr)]
        self.close()
        raw_stdout, raw_stderr = logs
        logs_ref = self.records.publish(self.prefix + '-logs.json', control.canonical({
            'stdout_hex': raw_stdout.hex(), 'stderr_hex': raw_stderr.hex()}))
        before = adapter.native_group_snapshot(self.process.pid)
        absent = adapter.native_absence(self.process.pid, process_reader)
        after = adapter.native_group_snapshot(self.process.pid)
        process_record = {'launch': self.launch, 'process_start': getattr(self, 'started', None),
            'exit_observation': exit_reference,
            'pid': self.process.pid, 'exit_code': code, 'natural_wait_observed': True,
            'finished_monotonic_ns': time.monotonic_ns(), 'group_before': before,
            'kernel_absence': absent, 'group_after': after, 'logs': logs_ref}
        if before['members'] == [] and after['members'] == []:
            # Keep the completed physical identity even if later evidence
            # publication or the original deadline check fails. Never re-probe
            # this PID/PGID during cleanup of a later session attempt.
            self.physical_exit = process_record
        terminal = self.records.publish(self.prefix + '-terminal.json', control.canonical(process_record))
        control.require(getattr(self, 'started', None) is not None
                        and code == 0 and before['members'] == [] and after['members'] == []
                        and process_record['finished_monotonic_ns'] <= self.deadline_ns,
                        'native economic verifier did not complete within its registered boundary')
        self.image.validate(); self._stable_inputs()
        native_test_passed(raw_stdout, raw_stderr)
        reference = self.records.locate(self.prepared['output_path'])
        raw = self.records.read(reference)
        result = validate_vector_result(control.decode(raw), self.prepared, records=self.records)
        self.image.validate(); self._stable_inputs()
        control.require(self.records.read(reference) == raw, 'native economic verifier output changed')
        receipt = self.records.publish(self.prefix + '-verified.json', control.canonical({
            'kind': 'benchmark_native_economic_verification_execution',
            'verification_request': self.prepared['reference'], 'native_result': reference,
            'process_terminal': terminal, 'executable_sha256': self.image.sha256,
            'executed_native_verifier': True}))
        return {'result': result, 'reference': reference, 'execution': receipt}

    def close(self):
        """Close only owned log descriptors, retaining any live child owner."""
        for stream in (self.stdout, self.stderr):
            if stream is not None: stream.close()
