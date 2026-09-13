"""Replay a retained sample through the native, resource and packet owners.

This component is read-only: it neither launches a verifier nor accepts a
claimed verified flag as execution. The enclosing source/evidence collector
admits executable identities and authenticates the immutable record provider.
The registered retained-session collector requires this complete replay.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
import re
import stat
from types import SimpleNamespace

import private_settlement_session_control as control
import private_settlement_session_replay as economic_replay
import private_settlement_session_semantics as semantics
import private_settlement_session_adapter as adapter
import private_settlement_process_observer as process
import private_settlement_network_observer as network


def _same(actual, expected, label):
    control.require(control.canonical(actual) == control.canonical(expected), label)


def _image(value):
    control.exact(value, {'path', 'sha256', 'bytes'}, 'admitted native image')
    control.digest(value['sha256']); control.unsigned(value['bytes'])
    control.require(type(value['path']) is str and Path(value['path']).is_absolute()
                    and value['bytes'] > 0, 'native image has no absolute source binding')
    return control.decode(control.canonical(value))


def _birth(identity):
    """Require the actual supported kernel lifetime shape, not a display name."""
    birth = identity['birth']
    common = {'pid', 'ppid', 'pgid', 'uid', 'birth', 'executable_path', 'executable_sha256'}
    if birth.get('kind') == 'darwin_bsdinfo':
        control.exact(identity, common | {'loaded_image_uuid'}, 'Darwin process identity')
        control.exact(birth, {'kind', 'started_seconds', 'started_microseconds', 'start_abstime'}, 'Darwin birth')
        for key in ('started_seconds', 'start_abstime'):
            control.require(control.unsigned(birth[key]) > 0, 'native birth is unavailable')
        control.require(control.unsigned(birth['started_microseconds']) < 1_000_000
                        and type(identity['loaded_image_uuid']) is str
                        and re.fullmatch('[0-9a-f]{32}', identity['loaded_image_uuid']) is not None
                        and identity['loaded_image_uuid'] != '0'*32, 'native image/birth is invalid')
    elif birth.get('kind') == 'linux_proc':
        control.exact(identity, common, 'Linux process identity')
        control.exact(birth, {'kind', 'boot_id', 'start_ticks'}, 'Linux birth')
        control.require(control.unsigned(birth['start_ticks']) > 0
                        and type(birth['boot_id']) is str
                        and re.fullmatch('[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', birth['boot_id']) is not None,
                        'native boot/birth is invalid')
    else:
        raise control.SessionProtocolError('unsupported retained kernel lifetime')


def _observation(value, declarations, images, uid, *, initial=None):
    """Join every declared process to its exact kernel parent, image and clock."""
    control.exact(value, {'started_monotonic_ns', 'finished_monotonic_ns', 'processes',
                         'cpu_time_ns', 'rss_bytes'}, 'native process observation')
    for key in ('started_monotonic_ns', 'finished_monotonic_ns', 'cpu_time_ns', 'rss_bytes'):
        control.unsigned(value[key])
    control.require(value['started_monotonic_ns'] <= value['finished_monotonic_ns']
                    and type(value['processes']) is list and len(value['processes']) == len(declarations),
                    'native observation interval or complete process count differs')
    for index, (row, declaration) in enumerate(zip(value['processes'], declarations)):
        control.exact(row, {'label', 'identity', 'cpu_time_ns', 'cpu_counter_unit_ns', 'rss_bytes'}, 'native process row')
        identity, image = row['identity'], images[declaration['image']]
        _birth(identity)
        expected = {key: declaration[key] for key in ('pid', 'ppid', 'pgid')}
        expected.update(uid=uid, executable_path=image['path'], executable_sha256=image['sha256'])
        _same({key: identity[key] for key in expected}, expected, 'native process owner/image differs from admitted declaration')
        control.require(row['label'] == declaration['label']
                        and 0 < control.unsigned(row['cpu_counter_unit_ns']) <= 1_000_000_000
                        and 0 < control.unsigned(row['rss_bytes']) <= process.MAX_EXACT_INTEGER,
                        'native label, CPU unit or RSS is invalid')
        control.unsigned(row['cpu_time_ns'])
        if identity['birth']['kind'] == 'darwin_bsdinfo':
            control.require(row['cpu_counter_unit_ns'] == 1, 'Darwin CPU unit changed')
        if initial is not None:
            prior = initial['processes'][index]
            _same({key: row[key] for key in ('label', 'identity', 'cpu_counter_unit_ns')},
                  {key: prior[key] for key in ('label', 'identity', 'cpu_counter_unit_ns')},
                  'native process generation changed since readiness')
            control.require(row['cpu_time_ns'] >= prior['cpu_time_ns'], 'native CPU counter regressed since readiness')
    cpu = sum(row['cpu_time_ns'] for row in value['processes'])
    rss = sum(row['rss_bytes'] for row in value['processes'])
    control.require(cpu < 1 << 64 and 0 < rss <= process.MAX_EXACT_INTEGER
                    and value['cpu_time_ns'] == cpu and value['rss_bytes'] == rss,
                    'native observation aggregate differs from complete process rows')
    if initial is not None:
        control.require(value['started_monotonic_ns'] >= initial['finished_monotonic_ns'],
                        'native observation precedes session readiness')


class _RetainedObservations:
    """Supply only the existing resource reducer to the semantic derivation."""

    def __init__(self, initial, declarations, images, uid):
        self.initial, self.declarations, self.images, self.uid = initial, declarations, images, uid

    def validate_window(self, value, *, records, budget_ms):
        baseline = control.decode(records.read(value['baseline']))
        _observation(baseline, self.declarations, self.images, self.uid, initial=self.initial)
        expected = [{key: row[key] for key in ('label', 'identity', 'cpu_counter_unit_ns')}
                    for row in self.initial['processes']]
        return process.validate_resource_window(value, records=records, outer_timeout_ms=budget_ms,
                                                expected_processes=expected)


class RetainedSampleReplay:
    """Accounting callback configured by independently admitted source identities."""

    def __init__(self, *, worker, validator, owner_uid, group_utility_sha256,
                 listener_utility_sha256, packet_utility):
        self.images = {'worker': _image(worker), 'validator': _image(validator)}
        self.uid = control.unsigned(owner_uid)
        self.group_utility = control.digest(group_utility_sha256)
        self.listener_utility = control.digest(listener_utility_sha256)
        control.exact(packet_utility, {'path', 'sha256'}, 'admitted packet utility image')
        control.digest(packet_utility['sha256'])
        control.require(packet_utility['path'] == str(semantics.packets.TCPDUMP), 'packet utility path differs')
        self.packet_utility = control.decode(control.canonical(packet_utility))

    def _recorded_packet_utility(self, recorded):
        """Bind historical capture identity to admitted bytes, not today's inode."""
        control.exact(recorded, {'path', 'sha256', 'metadata'}, 'recorded packet utility')
        _same({key: recorded[key] for key in ('path', 'sha256')}, self.packet_utility,
              'recorded packet executable differs from source-admitted bytes')
        attributes = control.exact(recorded['metadata'], {
            'st_dev', 'st_ino', 'st_size', 'st_mode', 'st_uid', 'st_mtime_ns', 'st_ctime_ns'},
            'recorded packet executable metadata')
        for value in attributes.values():
            control.unsigned(value)
        control.require(attributes['st_ino'] > 0 and 0 < attributes['st_size'] <= 16*1024**2
                        and attributes['st_mtime_ns'] > 0 and attributes['st_ctime_ns'] > 0
                        and stat.S_ISREG(attributes['st_mode']) and attributes['st_uid'] == 0
                        and stat.S_IMODE(attributes['st_mode']) & 0o022 == 0
                        and stat.S_IMODE(attributes['st_mode']) & 0o111 != 0,
                        'packet utility is not the recorded immutable root-owned executable')
        return control.decode(control.canonical(recorded))

    def _listeners(self, value, expected, observations):
        control.exact(value, {'process_before', 'listener_observation', 'process_after',
                             'expected_endpoints', 'scope'}, 'retained listener attribution')
        _same(value['expected_endpoints'], expected, 'retained listeners use different endpoint owners')
        _observation(value['process_before'], observations.declarations, self.images, self.uid, initial=observations.initial)
        _observation(value['process_after'], observations.declarations, self.images, self.uid, initial=value['process_before'])
        observed = value['listener_observation']
        control.exact(observed, {'started_monotonic_ns', 'finished_monotonic_ns', 'command',
            'utility_sha256', 'raw_utf8', 'raw_sha256', 'listeners'}, 'retained listener utility')
        pids = tuple(sorted({row['pid'] for row in expected}))
        command = [str(network.LSOF), '-nP', '-a', '-p', ','.join(map(str, pids)), '-iTCP', '-sTCP:LISTEN', '-F0pftPnT']
        _same(observed['command'], command, 'listener command is not the exact scoped utility')
        control.require(observed['utility_sha256'] == self.listener_utility
                        and type(observed['raw_utf8']) is str, 'listener utility source differs')
        raw = observed['raw_utf8'].encode('ascii')
        control.require(0 < len(raw) <= network.MAX_OUTPUT_BYTES
                        and hashlib.sha256(raw).hexdigest() == observed['raw_sha256'], 'listener raw output binding differs')
        rows = network.parse_listener_output(raw, pids)
        _same(rows, observed['listeners'], 'listener rows differ from retained raw native output')
        actual = [{key: row[key] for key in ('pid', 'address', 'port', 'transport')} for row in rows]
        order = lambda row: (row['pid'], row['port'])
        _same(sorted(actual, key=order), sorted(expected, key=order), 'native listener endpoints differ')
        times = [value['process_before']['finished_monotonic_ns'],
                 control.unsigned(observed['started_monotonic_ns']), control.unsigned(observed['finished_monotonic_ns']),
                 value['process_after']['started_monotonic_ns']]
        control.require(times == sorted(times), 'kernel observations do not bracket listener collection')

    def __call__(self, identity, attempt, bound, records):
        """Return exact recomputed sample bytes only after every retained join passes."""
        identity = control.identity(identity)
        control.exact(bound, {'rust_terminal', 'adapter_outcome', 'response', 'validation', 'sample'}, 'sample replay inputs')
        prefix = 'sessions/'+identity['session_id']
        request_ref = records.locate(prefix+'/request.json')
        request = control.decode(records.read(request_ref))
        control.require(request_ref['sha256'] == identity['session_request_sha256'], 'session request hash differs')
        prepared = {'identity': identity, 'reference': request_ref, 'request': request}
        start = control.decode(records.read(records.locate(prefix+'/started.json')))
        _same({key: start[key] for key in identity}, identity, 'native session start identity differs')
        worker = self.images['worker']
        command = [worker['path'], adapter.WORKER_TEST, '--exact', '--ignored', '--nocapture', '--test-threads=1']
        _same(start['command'], command, 'session owner used a different native command')
        _same(start['harness'], {key: worker[key] for key in ('sha256', 'bytes')}, 'session image binding differs')
        _same(start['request'], request_ref, 'session start changes its request')
        process_start = control.decode(records.read(records.locate(prefix+'/worker-process-start.json')))
        control.exact(process_start, {'command', 'pid', 'parent_pid', 'process_group', 'started_ns', 'worker_sha256'}, 'worker process start')
        _same(process_start['command'], command, 'worker process command differs')
        control.require(process_start['worker_sha256'] == worker['sha256']
                        and process_start['pid'] == process_start['process_group'], 'worker process image/group differs')
        ready_ref = records.locate(prefix+'/ready.json'); ready = control.decode(records.read(ready_ref))
        observed = control.decode(records.read(records.locate(prefix+'/process-ready.json')))
        control.exact(observed, control.IDENTITY_FIELDS | {'kind', 'ready', 'process_scope', 'listeners', 'network_ports'}, 'retained readiness')
        _same({key: observed[key] for key in identity}, identity, 'kernel readiness identity differs')
        control.require(observed['kind'] == 'benchmark_session_process_ready' and observed['ready'] == ready_ref
                        and observed['network_ports'] == ready['network_ports']
                        and ready['worker_pid'] == process_start['pid'], 'kernel readiness owner differs')
        declarations = process.benchmark_process_declarations(ready['process_inventory'], participants=request['participants'],
            worker_pid=process_start['pid'], adapter_pid=process_start['parent_pid'], process_group=process_start['pid'],
            commit=request['commit'], validator_sha256=self.images['validator']['sha256'], worker_sha256=worker['sha256'])
        initial = observed['process_scope']; _observation(initial, declarations, self.images, self.uid)
        observations = _RetainedObservations(initial, declarations, self.images, self.uid)
        ports = control.decode(records.read(ready['network_ports']))
        endpoints = network.declared_listener_rows(ports, session=identity, network_id=ready['network_id'],
            participants=request['participants'], scope=SimpleNamespace(declarations=declarations))
        self._listeners(observed['listeners'], endpoints, observations)
        output = attempt['output_directory']; raw_request = records.read(attempt['request'])
        refs = {key: records.locate(output+suffix) for key, suffix in {
            'rust_terminal': '/evidence/benchmark-protocol/rust-result.json',
            'adapter_outcome': '/evidence/benchmark-protocol/adapter-outcome.json', 'response': '/response.json',
            'validation': '/validation-outcome.json', 'sample': '/benchmark-sample.json'}.items()}
        for key, ref in refs.items():
            control.require(records.read(ref) == bound[key], 'callback input differs from its canonical retained record')
        outcome = control.decode(bound['adapter_outcome'])
        control.exact(outcome, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
            'request_sha256', 'rust_terminal', 'kind', 'status', 'measurement_window',
            'native_economic_verification', 'response'}, 'native adapter outcome')
        coordinates = {**identity, **{key: attempt[key] for key in control.ATTEMPT_FIELDS}}
        _same({key: outcome[key] for key in coordinates}, coordinates, 'adapter outcome identity differs')
        control.require(outcome['kind'] == 'benchmark_session_attempt_validation' and outcome['status'] == 'succeeded'
                        and outcome['request_sha256'] == attempt['request']['sha256']
                        and outcome['rust_terminal'] == refs['rust_terminal'] and outcome['response'] == refs['response'],
                        'adapter outcome does not bind this successful native result')
        durable = control.decode(records.read(records.locate(output+'/started.json')))
        verification = economic_replay.replay_native_economic_execution(prepared, attempt,
            outcome['native_economic_verification'], records=records, worker_command_path=worker['path'],
            worker_sha256=worker['sha256'], parent_pid=process_start['parent_pid'],
            group_utility_sha256=self.group_utility, outer_timeout_ms=durable['outer_timeout_ms'])
        window = control.decode(records.read(outcome['measurement_window']))
        control.require(control.reference(window['packet_window'])['path']
                        == output+'/packet-capture/packet-window.json',
                        'packet window is outside its canonical attempt namespace')
        resource = control.decode(records.read(window['process_resource_window']))
        control.require(control.reference(resource['baseline'])['path']
                        == output+'/process-observations/baseline.json',
                        'resource journal is outside its canonical attempt namespace')
        listeners = {}
        for key in ('listener_before', 'listener_after'):
            listeners[key] = control.decode(records.read(window[key]))
            self._listeners(listeners[key], endpoints, observations)
        packet = control.decode(records.read(window['packet_window']))
        capture_start = control.decode(records.read(packet['start']))
        packet_utility = self._recorded_packet_utility(capture_start['utility'])
        capture = control.decode(records.read(packet['process']))
        native_prefix = str(Path(verification['prepared']['output_path']).parent)+'/native-vector-process'
        launch = control.decode(records.read(records.locate(native_prefix+'-launch.json')))
        control.require(type(capture['parent_pid']) is int and capture['parent_pid'] == process_start['parent_pid'],
                        'packet collector was spawned by another process owner')
        # Materialization launches the native vector verifier after packet and
        # listener collection completes. Do not extend the retained deadline.
        ended = control.unsigned(packet['terminal']['finished_monotonic_ns'])
        listener_ended = listeners['listener_after']['process_after']['finished_monotonic_ns']
        launched = control.unsigned(launch['started_monotonic_ns'])
        control.require(max(ended, listener_ended) <= launched <= verification['deadline_monotonic_ns'],
                        'measurement collection did not close before native verification')
        owner = semantics.SessionSemantics(prepared, records=records, observations=observations,
            packet_utility=packet_utility, cwd=Path(worker['path']).parent,
            deadline=lambda: verification['deadline_monotonic_ns'], outer_timeout_ms=durable['outer_timeout_ms'])
        owner.validate_ready(ready, request)
        response, sample = owner._derive(bound['rust_terminal'], raw_request, native_ref=refs['rust_terminal'],
            measurement_window=outcome['measurement_window'], ready=ready, attempt=attempt, verification=verification)
        control.require(semantics.measurement_bytes(response) == bound['response']
                        and semantics.measurement_bytes(sample) == bound['sample'],
                        'retained response or sample differs from native/resource/packet replay')
        validation = control.decode(bound['validation'])
        _same(validation, {**coordinates, 'passed': True, 'validation_kind': 'accepted',
                           'response': refs['response'], 'sample': refs['sample']}, 'sample acceptance differs')
        return semantics.measurement_bytes(sample)
