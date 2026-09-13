"""Adapter controls with one real native-observed Python protocol peer.

The peer is not Iroha and has no validator network. Packet records are explicit
synthetic fixtures. Native process lifetime/CPU/RSS and actual pipe ordering are
tested independently of the still-required compiled network and capture owners.
"""
from __future__ import annotations


import hashlib
import json
import os
from pathlib import Path
import sys
import threading
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
for path in (Path(__file__).resolve().parents[1], Path(__file__).resolve().parent):
    sys.path.insert(0, str(path))
import private_settlement_release_runner as runner
import private_settlement_session_control as control
import private_settlement_session_bridge as bridge
import private_settlement_session_adapter as adapter
import private_settlement_process_observer as process


def fixture_worker(path):
    document = json.loads(Path(path).read_bytes())
    prepared, start = document['prepared'], document['start']
    session, attempts = prepared['identity'], prepared['request']['attempts']
    root = Path(document['root'])
    prefix = f"sessions/{session['session_id']}"
    with control.RecordDirectory(root) as records, os.fdopen(int(os.environ['APS_BENCHMARK_CONTROL_READ_FD']),
            'rb', buffering=0) as reader, os.fdopen(int(os.environ['APS_BENCHMARK_CONTROL_WRITE_FD']),
            'wb', buffering=0) as writer:
        incoming = control.ControlChain(session, start['sha256'], 'adapter_worker', 'owner_to_child',
                                        records, observer='worker', journal_prefix=prefix+'/control')
        outgoing = control.ControlChain(session, start['sha256'], 'adapter_worker', 'child_to_owner',
                                        records, observer='worker', journal_prefix=prefix+'/control')
        def publish(name, value):
            return records.publish(name, json.dumps(value, sort_keys=True, separators=(',', ':')).encode())
        ready = publish(prefix+'/ready.json', {**session, 'network_id': 'synthetic-network',
            'genesis_sha256': '1'*64, 'configuration_sha256': prepared['request']['configuration_sha256'],
            'workload_manifest_sha256': prepared['request']['workload_manifest_sha256'],
            'activated_height': 1, 'worker_pid': os.getpid(), 'process_inventory': [],
            'network_ports': publish(prefix+'/network-ports.json', {'fixture_only': True})})
        outgoing.send(writer, 'ready', {'ready': ready})
        accepted, active, kind, reason = [], None, 'completed', None
        for index, row in enumerate(attempts):
            dispatch = incoming.receive(reader)
            upstream_ref = dispatch.decoded()['forwarded_from']
            control.verify_forwarded(dispatch, control.RetainedMessage(records.read(upstream_ref), upstream_ref))
            control.require(dispatch.decoded()['kind'] == 'dispatch', 'fixture expected dispatch')
            active = row['attempt_id']
            fields = {key: row[key] for key in control.ATTEMPT_FIELDS}
            for boundary in ('ready', 'finished'):
                if boundary == 'finished' and document.get('missing_finished'):
                    break
                marker = publish(row['output_directory']+f'/evidence/benchmark-protocol/measurement-{boundary}.json',
                                 {**session, **fields, 'boundary': boundary})
                outgoing.send(writer, 'measurement_'+boundary, {**fields, 'marker': marker})
                response = incoming.receive(reader).decoded()
                expected = 'measurement_begin' if boundary == 'ready' else 'measurement_recorded'
                control.require(response['kind'] == expected and response['payload']['marker'] == marker,
                                'fixture timing acknowledgement differs')
                records.read(response['payload']['process_observation' if boundary == 'ready' else 'measurement_window'])
                if boundary == 'ready':
                    sum(value*value for value in range(20000))
            request = control.decode(records.read(row['request']))
            header = {key: request[key] for key in ('version', 'protocol', 'request_id', 'invocation_nonce',
                                                   'commit', 'participants')}
            header['request_sha256'] = row['request']['sha256']
            native_result = {**header, 'mandatory_signed_rs16_da_rbc': True,
                'signed_rs16_da_observations': 1, 'authenticated_message_control': True,
                'process_inventory': [], 'payload': {'fixture_only': True}}
            terminal_ref = publish(row['output_directory']+'/evidence/benchmark-protocol/rust-result.json',
                {**header, 'elapsed_ms': 1, 'outcome': {'kind': 'succeeded', 'result': native_result}})
            outgoing.send(writer, 'attempt_completed', {**fields, 'rust_terminal': terminal_ref})
            response = incoming.receive(reader)
            ref = response.decoded()['forwarded_from']
            control.verify_forwarded(response, control.RetainedMessage(records.read(ref), ref))
            if response.decoded()['kind'] == 'stop':
                kind, reason = 'failed', 'validation_failed'
                break
            control.require(response.decoded()['kind'] == 'accept', 'fixture expected exact acceptance')
            accepted.append(row['request_id']); active = None
        terminal = publish(prefix+'/worker-terminal.json', {**session, 'kind': kind, 'reason': reason,
            'accepted_request_ids': accepted, 'active_attempt_id': active,
            'last_owner_message_sha256': incoming.previous, 'last_worker_message_sha256': outgoing.previous,
            'network_shutdown_observed': True, 'coordinator_reaped_observed': True})
        outgoing.send(writer, 'session_completed', {'worker_terminal': terminal})
        return 0 if kind == 'completed' else 1


class OneProcessObservations(adapter.NativeObservations):
    """Explicit fixture seam: real kernel scope, no claimed Iroha topology."""
    def ready(self, ready, session, request, worker, records):
        self.scope = process.ProcessScope([{'label': 'proof_worker', 'pid': worker.process.pid,
            'ppid': os.getpid(), 'pgid': worker.process.pid, 'image': 'worker'}], self.reader, self.images)
        return {'process_scope': self.scope.initial, 'listeners': {'fixture_only': True},
                'network_ports': ready['network_ports']}

    def observe_listeners(self):
        self.scope.observe()
        return {'fixture_only': True}


class FixturePackets:
    """No traffic metric: test only ordering and required capture validation."""
    def __init__(self, events, reject=False):
        self.events, self.reject = events, reject
        self.PacketCaptureOwner = self

    def begin(self, **arguments):
        self.events.append('packet_begin')
        arguments['records'].read(arguments['resource_baseline'])
        # Exercise the real fd-relative publication contract at the exact
        # prefix used by PacketCaptureOwner, without launching tcpdump.
        self.start = arguments['records'].publish(arguments['prefix']+'/capture-start.json',
                                                  control.canonical({'fixture_only': True}))
        import stat
        control.require(stat.S_IMODE((arguments['records'].path/arguments['prefix']).stat().st_mode) == 0o700,
                        'packet parent is not exclusively owner accessible')
        class Window:
            def __init__(inner):
                inner.ended = False
                inner.closed = False
                inner.utility = {'fixture_only': True}
            def end(inner, marker):
                self.events.append('packet_end'); inner.ended = True
                arguments['records'].read(marker)
            def finish(inner, *, listener_after, resource_window):
                self.events.append('packet_finish'); inner.closed = True
                control.require(inner.ended, 'fixture capture has no end')
                arguments['records'].read(listener_after)
                control.require(resource_window['sampler_stopped_observed'], 'capture finalized before resource end')
                return arguments['records'].publish(arguments['prefix']+'/packet-window.json',
                    control.canonical({'fixture_only': True}))
            def abort(inner, reason):
                self.events.append('packet_abort')
                return arguments['records'].publish(arguments['prefix']+'-incomplete.json',
                    control.canonical({'fixture_only': True, 'reason': reason}))
        return Window()

    def validate_packet_window(self, reference, **expected):
        self.events.append('packet_validate')
        control.require(expected['expected_utility'] == {'fixture_only': True},
                        'packet utility admission was not forwarded')
        if self.reject:
            raise ValueError('controlled complete-capture rejection')


class FixtureSemantics:
    """Strict fixture records, never a network/result qualification validator."""
    def __init__(self, records): self.records = records
    def validate_ready(self, ready, request):
        control.require(ready['network_id'] == 'synthetic-network', 'fixture ready differs')
    def materialize_terminal(self, native_raw, request_raw, **context):
        native = json.loads(native_raw)
        control.require(native['outcome']['result']['payload'] == {'fixture_only': True}, 'unexpected fixture payload')
        control.require(context['measurement_window'] is not None, 'fixture result lost its completed window')
        self.records.read(context['measurement_window'])
        prefix = context['attempt']['output_directory']
        return {'adapter_outcome': self.records.publish(prefix+'/adapter-outcome.json',
                    control.canonical({'fixture_only': True, 'status': 'succeeded'})),
                'response': self.records.publish(prefix+'/response.json',
                    b'{"fixture_only":true,"end_to_end":1.25}')}
    def validate_acceptance(self, bound, attempt, window):
        control.require(json.loads(bound['sample'])['fixture_only'] is True, 'fixture sample differs')
        control.require({'adapter_outcome', 'response'} <= set(self.gate.completed),
                        'adapter gate did not retain its own completion references')
        self.records.read(window)


class AdapterControls(unittest.TestCase):
    def setUp(self):
        from private_settlement_session_records_test import SessionRecordTests
        self.fixture = SessionRecordTests(); self.fixture.setUp(); self.addCleanup(self.fixture.tearDown)
        self.root, self.prepared = self.fixture.campaign, self.fixture.prepared()
        self.records = control.RecordDirectory(self.root); self.addCleanup(self.records.close)
        self.start = runner.publish_benchmark_session_start(self.prepared, records=self.records,
            command=['/controlled/native-adapter'], harness={'sha256': 'f'*64, 'bytes': 100})
        image_path = Path(sys.executable).resolve()
        if sys.platform == 'darwin':
            import ctypes
            probe = process.native_reader()
            try:
                native_path = ctypes.create_string_buffer(4096)
                count = probe.reader.lib.proc_pidpath(os.getpid(), native_path, len(native_path))
                self.assertGreater(count, 0)
                image_path = Path(os.fsdecode(native_path.raw[:count])).resolve(strict=True)
            finally:
                probe.close()
        self.image = process.ExecutableImage(image_path, hashlib.sha256(image_path.read_bytes()).hexdigest())
        self.addCleanup(self.image.close)
        self.native = process.native_reader(); self.addCleanup(self.native.close)
        self.observations = OneProcessObservations(process_reader=self.native, listener_reader=None,
                                                   images={'worker': self.image})
        self.events = []

    def drive(self, **options):
        config = self.root/'native-fixture-input.json'
        config.write_bytes(control.canonical({'prepared': self.prepared, 'start': self.start,
                                             'root': str(self.root), **options}))
        deadline = time.monotonic_ns()+60_000_000_000
        self.worker = adapter.NativeWorker([str(self.image.path), '-B', str(Path(__file__).resolve()),
            '--worker-fixture', str(config)], dict(os.environ, PYTHONDONTWRITEBYTECODE='1'),
            cwd=ROOT, image=self.image, records=self.records,
            prefix=f"sessions/{self.prepared['identity']['session_id']}", deadline=lambda: deadline)
        pipes = control.ChildControlPipes()
        read_fd, write_fd = pipes.child_fds
        adapter_reader = os.fdopen(os.dup(read_fd), 'rb', buffering=0)
        adapter_writer = os.fdopen(os.dup(write_fd), 'wb', buffering=0)
        pipes.child_spawn_finished()
        self.owner = adapter.SessionAdapter(self.prepared, self.start, records=self.records,
            runner_reader=adapter_reader, runner_writer=adapter_writer, worker=self.worker,
            observations=self.observations, packets=FixturePackets(self.events, options.get('reject_capture', False)),
            semantics=FixtureSemantics(self.records), outer_timeout_ms=60000)
        self.owner.semantics.gate = self.owner.gate
        self.adapter_error = None
        def pump():
            try: self.lifecycle = self.owner.run()
            except BaseException as error: self.adapter_error = error
            finally: adapter_reader.close(); adapter_writer.close()
        thread = threading.Thread(target=pump); thread.start()
        controller = bridge.RunnerSessionBridge(self.prepared, self.start, records=self.records,
                                                reader=pipes.reader, writer=pipes.writer)
        def start(index, previous):
            if index == 1 and options.get('rebound_predecessor'):
                publish = self.records.publish
                def rebound(name, raw):
                    if name != self.prepared['request']['attempts'][index]['output_directory']+'/started.json':
                        return publish(name, raw)
                    value = control.decode(raw)
                    value['preceding_acceptance'] = {**previous, 'sha256': '1'*64}
                    return publish(name, control.canonical(value))
                with mock.patch.object(self.records, 'publish', side_effect=rebound):
                    return runner.publish_benchmark_attempt_start(self.prepared, index, records=self.records,
                        session_started=self.start, ordinal=51+index, outer_timeout_ms=60000, preceding_acceptance=previous)
            return runner.publish_benchmark_attempt_start(self.prepared, index, records=self.records,
                session_started=self.start, ordinal=51+index, outer_timeout_ms=60000, preceding_acceptance=previous)
        def ready(bound):
            value = control.decode(bound['process_observation'])
            identity = value['process_scope']['processes'][0]['identity']
            self.assertEqual(identity['pid'], self.worker.process.pid)
            self.assertEqual(identity['executable_sha256'], self.image.sha256)
        def completion(index, bound):
            prefix = self.prepared['request']['attempts'][index]['output_directory']
            return bridge.CompletionDecision('succeeded', self.records.publish(prefix+'/validation.json',
                control.canonical({'passed': True, 'validation_kind': 'accepted', 'fixture_only': True})),
                self.records.publish(prefix+'/sample.json', b'{"fixture_only":true,"end_to_end":1.25}'))
        def acceptance(index, bound):
            self.assertEqual(json.loads(bound['sample'])['end_to_end'], 1.25)
        def terminal(bound, accepted, complete):
            value = control.decode(bound['adapter_lifecycle'])
            self.assertTrue(value['worker_wait_completed'])
            self.assertEqual(value['worker_exit_code'], 0)
            self.assertEqual(value['group_after']['members'], [])
            self.assertTrue(all(row['kernel_absence_observed'] for row in value['kernel_absences']))
        try:
            return controller.run(publish_start=start, validate_ready=ready, validate_completion=completion,
                                  validate_acceptance=acceptance, validate_terminal=terminal)
        finally:
            pending_error = sys.exception()
            pipes.close(); thread.join(timeout=15)
            self.assertFalse(thread.is_alive(), 'fixture adapter did not stop')
            self.worker.process.wait(timeout=15)
            if pending_error is not None and self.adapter_error is not None:
                import traceback
                pending_error.add_note(''.join(traceback.format_exception(self.adapter_error)))
                pending_error.add_note((self.root/f"sessions/{self.prepared['identity']['session_id']}/worker.stderr.log").read_text())

    def test_one_real_worker_survives_eight_attempts_and_is_reaped_before_final_forward(self):
        result = self.drive()
        self.assertIsNone(self.adapter_error)
        self.assertTrue(result.all_attempts_accepted)
        self.assertEqual(len(result.started), 8)
        self.assertEqual(self.events, ['packet_begin', 'packet_end', 'packet_finish', 'packet_validate']*8)
        self.assertEqual(self.worker.process.returncode, 0)
        for row in self.prepared['request']['attempts']:
            window = json.loads((self.root/row['output_directory']/'measurement-window.json').read_bytes())
            times = window['boundaries']
            self.assertLessEqual(times['baseline_published_ns'], times['begin_written_ns'])
            self.assertLessEqual(times['finished_received_ns'], times['packet_end_returned_ns'])
            self.assertLessEqual(times['packet_end_returned_ns'], times['resources_finished_ns'])
            self.assertNotIn('network_bytes', window)

    def test_missing_finished_boundary_aborts_capture_and_cannot_accept_success(self):
        with self.assertRaises(bridge.SessionBridgeFailure): self.drive(missing_finished=True)
        self.assertIsNotNone(self.adapter_error)
        self.assertEqual(self.events, ['packet_begin', 'packet_abort'])
        self.assertEqual(len(list(self.root.glob('attempts/*/started.json'))), 1)
        self.assertEqual(self.owner.gate.accepted, [])

    def test_capture_rejection_prevents_measurement_ack_and_sample(self):
        with self.assertRaises(bridge.SessionBridgeFailure): self.drive(reject_capture=True)
        self.assertIsNotNone(self.adapter_error)
        self.assertEqual(self.owner.gate.accepted, [])
        self.assertEqual(len(list(self.root.glob('attempts/*/started.json'))), 1)
        self.assertFalse(any(self.root.glob('attempts/*/sample.json')))
        self.assertNotIn('packet_abort', self.events, 'closed capture must not be aborted twice')

    def test_rebound_predecessor_retains_first_success_and_prevents_second_dispatch(self):
        with self.assertRaises(bridge.SessionBridgeFailure): self.drive(rebound_predecessor=True)
        self.assertEqual(self.owner.gate.accepted, [self.prepared['request']['attempts'][0]['request_id']])
        self.assertEqual(len(list(self.root.glob('attempts/*/started.json'))), 2)
        self.assertEqual(self.events, ['packet_begin', 'packet_end', 'packet_finish', 'packet_validate'])

    def test_spawn_publication_failure_preserves_actual_child_owner(self):
        with mock.patch.object(self.records, 'publish', side_effect=OSError('controlled publication failure')):
            with self.assertRaises(adapter.NativeWorkerStartFailure) as caught:
                adapter.NativeWorker([str(self.image.path), '-B', '-c', 'pass'], dict(os.environ),
                    cwd=ROOT, image=self.image, records=self.records,
                    prefix=f"sessions/{self.prepared['identity']['session_id']}",
                    deadline=lambda: time.monotonic_ns()+10_000_000_000)
        worker = caught.exception.worker
        self.assertIsNotNone(worker.process)
        self.assertGreater(worker.process.pid, 1)
        self.assertEqual(worker.process.wait(timeout=10), 0)
        self.assertTrue(adapter.native_absence(worker.process.pid, self.native)['kernel_absence_observed'])

    def test_delayed_wall_sample_cannot_extend_registered_deadline(self):
        calls = []
        def monotonic(): calls.append('monotonic'); return 100_000_000
        def wall(): calls.append('wall'); return 10_500_000_000
        with mock.patch.object(adapter.time, 'monotonic_ns', side_effect=monotonic), \
                mock.patch.object(adapter.time, 'time_ns', side_effect=wall):
            self.assertEqual(adapter.start_deadline({'started_ns':10_000_000_000}, 1000), 600_000_000)
        self.assertEqual(calls, ['monotonic', 'wall'])
        for budget in (0, True):
            with self.assertRaises(ValueError): adapter.start_deadline({'started_ns':time.time_ns()}, budget)

    def test_live_native_child_cannot_be_reported_absent(self):
        # No signal is used: the current test process is a known live lifetime.
        with self.assertRaises(control.SessionProtocolError): adapter.native_absence(os.getpid(), self.native)

    def test_group_output_bound_is_checked_before_reading_subprocess_bytes(self):
        def oversized(command, **arguments):
            self.assertNotIn('capture_output', arguments)
            arguments['stdout'].seek(4*1024*1024)
            arguments['stdout'].write(b'x')
            return type('Completed', (), {'returncode': 0})()
        with mock.patch.object(adapter.subprocess, 'run', side_effect=oversized):
            with self.assertRaises(control.SessionProtocolError): adapter.native_group_snapshot(os.getpid())

    def test_native_terminal_requires_typed_budget_without_invented_exit(self):
        row = self.prepared['request']['attempts'][0]
        request = control.decode(self.records.read(row['request']))
        value = {key: request[key] for key in ('version', 'protocol', 'request_id', 'invocation_nonce',
                                              'commit', 'participants')}
        value.update(request_sha256=row['request']['sha256'], elapsed_ms=300001,
                     outcome={'kind': 'timed_out', 'stage': 'coordinator_ack', 'budget_ms': 300000, 'elapsed_ms': 300001})
        self.assertEqual(adapter.terminal_envelope(control.canonical(value), request, row['request']['sha256']), value)
        value['outcome']['budget_ms'] = 0
        with self.assertRaises(ValueError): adapter.terminal_envelope(control.canonical(value), request, row['request']['sha256'])
        result = {key: value[key] for key in ('version', 'protocol', 'request_id', 'invocation_nonce',
                                             'request_sha256', 'commit', 'participants')}
        result.update(mandatory_signed_rs16_da_rbc=True, signed_rs16_da_observations=1,
                      authenticated_message_control=True, process_inventory=[], payload={})
        value['outcome'] = {'kind': 'succeeded', 'result': result}
        result['version'] = True
        with self.assertRaises(ValueError): adapter.terminal_envelope(control.canonical(value), request, row['request']['sha256'])

    def test_runner_session_completion_requires_lifecycle_but_worker_payload_does_not(self):
        session = self.prepared['identity']
        with control.RecordDirectory(self.root) as records:
            prefix = f"sessions/{session['session_id']}/control"
            chain = control.ControlChain(session, self.start['sha256'], 'runner_adapter', 'child_to_owner',
                                         records, observer='adapter', journal_prefix=prefix)
            reference = {'path': 'synthetic.json', 'sha256': '1'*64, 'bytes': 2}
            import io
            with self.assertRaises(control.SessionProtocolError):
                chain.send(io.BytesIO(), 'session_completed', {'worker_terminal': reference}, forwarded_from=reference)


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == '--worker-fixture':
        try: code = fixture_worker(sys.argv[2])
        except (control.SessionInterrupted, BrokenPipeError): code = 1
        raise SystemExit(code)
    unittest.main()
