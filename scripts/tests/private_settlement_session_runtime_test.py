"""Exercise the runtime with one real protocol fixture process and native reads.

The worker is Python, packet/economic inputs are explicit fixtures, and these
controls do not qualify Iroha, packet capture, or benchmark results.
"""
from __future__ import annotations


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

import private_settlement_session_adapter_test as fixture
import private_settlement_session_runtime as runtime

adapter, bridge, control, runner = fixture.adapter, fixture.bridge, fixture.control, fixture.runner


class RuntimeControls(unittest.TestCase):
    def setUp(self):
        self.fixture = fixture.AdapterControls()
        self.fixture.setUp()
        self.addCleanup(self.fixture.tearDown)
        self.addCleanup(self.fixture.doCleanups)
        for name in ('root', 'records', 'prepared', 'start', 'image', 'observations', 'events'):
            setattr(self, name, getattr(self.fixture, name))

    def drive(self, *, reject_at=None, inspect_deadline=False, **options):
        config = self.root/'runtime-native-fixture-input.json'
        config.write_bytes(control.canonical({'prepared': self.prepared, 'start': self.start,
                                             'root': str(self.root), **options}))
        deadline = time.monotonic_ns()+60_000_000_000
        self.worker = adapter.NativeWorker([str(self.image.path), '-B', str(Path(fixture.__file__).resolve()),
            '--worker-fixture', str(config)], dict(os.environ, PYTHONDONTWRITEBYTECODE='1'),
            cwd=ROOT, image=self.image, records=self.records,
            prefix='sessions/'+self.prepared['identity']['session_id'], deadline=lambda: deadline)
        semantic = fixture.FixtureSemantics(self.records)
        self.runtime = runtime.SessionRuntime(self.prepared, self.start, records=self.records,
            worker=self.worker, observations=self.observations,
            packet_owner=fixture.FixturePackets(self.events, options.get('reject_capture', False)),
            semantic_owner=semantic, outer_timeout_ms=60000)
        semantic.gate = self.runtime.owner.gate
        ack_entered, successor_started, ack_checked = (threading.Event() for _ in range(3))
        if inspect_deadline:
            validate = semantic.validate_acceptance
            def delayed_acceptance(bound, attempt, window):
                if attempt['session_attempt_index'] == 0:
                    previous_deadline = self.runtime.owner.current_deadline
                    ack_entered.set()
                    control.require(successor_started.wait(timeout=5), 'fixture successor was not durably started')
                    self.deadline_unchanged = self.runtime.owner.current_deadline == previous_deadline
                    ack_checked.set()
                    control.require(self.deadline_unchanged, 'successor start extended prior adapter deadline')
                return validate(bound, attempt, window)
            semantic.validate_acceptance = delayed_acceptance
            send = self.runtime.controller.outgoing.send
            def dispatch_after_deadline_check(writer, kind, payload, **kwargs):
                if kind == 'dispatch' and payload['session_attempt_index'] == 1:
                    successor_started.set()
                    control.require(ack_checked.wait(timeout=5), 'fixture prior ACK deadline was not checked')
                return send(writer, kind, payload, **kwargs)
            self.runtime.controller.outgoing.send = dispatch_after_deadline_check

        def start(index, previous):
            if inspect_deadline and index == 1:
                control.require(ack_entered.wait(timeout=5), 'fixture prior ACK validation did not start')
            return runner.publish_benchmark_attempt_start(self.prepared, index, records=self.records,
                session_started=self.start, ordinal=51+index, outer_timeout_ms=60000,
                preceding_acceptance=previous)

        def ready(bound):
            scope = control.decode(bound['process_observation'])['process_scope']
            self.assertEqual(scope['processes'][0]['identity']['pid'], self.worker.process.pid)
            observed = self.runtime.observation()
            self.assertTrue(observed['adapter_thread_alive'])
            self.assertIsNone(observed['worker_poll_exit_code'])
            self.assertIsNone(observed['lifecycle'])
            self.assertFalse(observed['exchange_complete'])

        def completion(index, bound):
            prefix = self.prepared['request']['attempts'][index]['output_directory']
            self.assertIn('rust_terminal', bound)
            return bridge.CompletionDecision('succeeded', self.records.publish(prefix+'/validation.json',
                control.canonical({'passed': True, 'validation_kind': 'accepted', 'fixture_only': True})),
                self.records.publish(prefix+'/sample.json', b'{"fixture_only":true,"end_to_end":1.25}'))

        def acceptance(index, bound):
            if reject_at == index:
                raise ValueError('controlled runner acceptance rejection')
            self.assertIn(b'fixture_only', bound['sample'])

        def terminal(bound, accepted, complete):
            self.assertFalse(self.runtime.thread.is_alive(), 'session closure callback precedes actual pump join')
            self.assertIsNotNone(self.runtime.lifecycle)
            value = control.decode(bound['adapter_lifecycle'])
            self.assertEqual(value['worker_pid'], self.worker.process.pid)
            self.assertTrue(value['worker_wait_completed'])
            self.assertEqual(value['worker_exit_code'], 0)
            self.assertEqual(value['group_after']['members'], [])
            self.assertTrue(all(row['kernel_absence_observed'] for row in value['kernel_absences']))
            self.assertEqual(len(accepted), 8)
            self.assertTrue(complete)

        try:
            return self.runtime.run(publish_start=start, validate_ready=ready,
                validate_completion=completion, validate_acceptance=acceptance, validate_terminal=terminal)
        finally:
            # Only the fixture is executed. It exits naturally on protocol/EOF;
            # no cleanup helper sends a signal or terminates a process.
            self.runtime.thread.join(timeout=15)
            self.assertFalse(self.runtime.thread.is_alive())
            self.worker.process.wait(timeout=15)
            self.runtime.close_inactive_descriptors()

    def test_eight_attempts_share_one_actual_worker_and_join_natural_lifecycle(self):
        result = self.drive()
        self.assertTrue(result.all_attempts_accepted)
        self.assertEqual(len(result.started), 8)
        self.assertEqual(len(result.acknowledgements), 8)
        observed = self.runtime.observation()
        self.assertEqual(observed['worker_poll_exit_code'], 0)
        self.assertFalse(observed['adapter_thread_alive'])
        self.assertIsNone(observed['adapter_error_type'])
        self.assertIsNotNone(observed['lifecycle'])
        self.assertEqual(self.events, ['packet_begin', 'packet_end', 'packet_finish', 'packet_validate']*8)

    def test_runner_rejection_preserves_real_owner_prior_ack_and_no_successor_start(self):
        with self.assertRaises(runtime.SessionRuntimeIncomplete) as caught:
            self.drive(reject_at=5)
        self.assertIs(caught.exception.runtime, self.runtime)
        self.assertIs(self.runtime.worker, self.worker)
        self.assertEqual(len(self.runtime.controller.started), 6)
        self.assertEqual(len(self.runtime.controller.written), 5)
        self.assertFalse((self.root/self.prepared['request']['attempts'][6]['output_directory']/'started.json').exists())
        self.assertIsNotNone(self.runtime.adapter_error)
        self.assertIsNone(self.runtime.lifecycle)
        self.assertIsNone(self.runtime.exchange)

    def test_adapter_packet_failure_retains_owner_without_sample_or_lifecycle(self):
        with self.assertRaises(runtime.SessionRuntimeIncomplete):
            self.drive(reject_capture=True)
        self.assertEqual(len(self.runtime.controller.started), 1)
        self.assertEqual(self.runtime.controller.written, [])
        self.assertIsNone(self.runtime.lifecycle)
        self.assertIsNotNone(self.runtime.adapter_error)
        self.assertEqual(self.events, ['packet_begin', 'packet_end', 'packet_finish', 'packet_validate'])

    def test_completed_runtime_cannot_be_reused(self):
        self.drive()
        with self.assertRaises(control.SessionProtocolError):
            self.runtime.run(publish_start=None, validate_ready=None, validate_completion=None,
                             validate_acceptance=None, validate_terminal=None)

    def test_next_durable_start_does_not_extend_previous_adapter_deadline(self):
        self.drive(inspect_deadline=True)
        self.assertTrue(self.deadline_unchanged)

    def test_substituted_owner_command_and_image_reject_before_native_spawn(self):
        with mock.patch.object(adapter, 'spawn_native_session') as spawn:
            with self.assertRaisesRegex(control.SessionProtocolError, 'admitted native executable'):
                runtime.spawn_bound_worker(self.prepared, self.start, records=self.records,
                    cwd=ROOT, images={'worker': self.image}, runtime_root=self.root, outer_timeout_ms=60000)
            spawn.assert_not_called()


class RunnerCallbackControls(unittest.TestCase):
    def setUp(self):
        import private_settlement_session_semantics_test as semantic_fixture
        self.fixture = semantic_fixture.SemanticControls()
        self.fixture.setUp()
        self.addCleanup(self.fixture.tearDown)
        self.addCleanup(self.fixture.doCleanups)
        self.semantic_fixture = semantic_fixture
        self.records, self.prepared, self.row = self.fixture.records, self.fixture.prepared, self.fixture.row
        self.semantic = self.fixture.owner
        self.start = runner.publish_benchmark_session_start(self.prepared, records=self.records,
            command=['fixture-only'], harness={'sha256': 'f'*64, 'bytes': 1})
        self.lifecycle_validator = mock.Mock()
        self.callbacks = runtime.RunnerSemanticCallbacks(self.prepared, self.start, records=self.records,
            semantic_owner=self.semantic, first_ordinal=51, outer_timeout_ms=60000,
            validate_lifecycle=self.lifecycle_validator)

    def completed(self):
        raw, ref = self.fixture.terminal()
        window = self.records.publish(self.row['output_directory']+'/measurement-window.json',
                                      control.canonical({'fixture_only': True}))
        verified = self.fixture.synthetic_verification()
        economic = self.semantic_fixture.economics
        for patch in (mock.patch.object(economic, 'prepare_verification', return_value=self.fixture.fixture.bound),
                      mock.patch.object(economic, 'NativeVectorInvocation'),
                      mock.patch.object(self.semantic, '_window', return_value=(
                          {'cpu_time_ns': 1_000_000, 'sampled_peak_rss_bytes': 4096},
                          {'window_ip_bytes': 125, 'counting_unit': 'ipv4_packet_bytes_including_ip_tcp_headers'}))):
            value = patch.start(); self.addCleanup(patch.stop)
            if patch.attribute == 'NativeVectorInvocation':
                value.return_value.finish.return_value = verified
        refs = self.semantic.materialize_terminal(raw, self.fixture.request_raw, native_ref=ref,
            measurement_window=window, ready=self.fixture.ready, attempt=self.row)
        return {'rust_terminal': raw, **{key: self.records.read(value) for key, value in refs.items()}}

    def test_completion_publishes_exact_sample_then_replays_before_acceptance(self):
        bound = self.completed()
        decision = self.callbacks.validate_completion(0, bound)
        self.assertEqual(decision.kind, 'succeeded')
        self.assertEqual(decision.sample['path'], self.row['output_directory']+'/benchmark-sample.json')
        self.assertEqual(self.records.read(decision.sample),
                         runtime.semantics.measurement_bytes(self.semantic.completed[self.row['attempt_id']]['sample']))
        accepted = dict(bound, validation=self.records.read(decision.validation), sample=self.records.read(decision.sample))
        self.callbacks.validate_acceptance(0, accepted)
        with self.assertRaises(control.SessionProtocolError):
            self.callbacks.validate_acceptance(0, dict(accepted, sample=b'{"forged":true}'))

    def test_failed_native_terminal_has_typed_validation_and_no_sample(self):
        raw, ref = self.fixture.terminal({'kind': 'failed', 'stage': 'benchmark_worker', 'reason': 'execution_error'})
        refs = self.semantic.materialize_terminal(raw, self.fixture.request_raw, native_ref=ref,
            measurement_window=None, ready=self.fixture.ready, attempt=self.row)
        decision = self.callbacks.validate_completion(0, {'rust_terminal': raw,
            'adapter_outcome': self.records.read(refs['adapter_outcome'])})
        self.assertEqual(decision.kind, 'failed'); self.assertIsNone(decision.sample)
        validation = control.decode(self.records.read(decision.validation))
        self.assertIs(validation['passed'], False)
        self.assertNotIn('exit_code', validation)
        self.assertFalse((self.records.path/self.row['output_directory']/'benchmark-sample.json').exists())

    def test_callbacks_preserve_actual_ordinal_and_require_terminal_validator(self):
        start = control.decode(self.records.read(self.callbacks.publish_start(0, None)))
        self.assertEqual(start['ordinal'], 51); self.assertEqual(start['outer_timeout_ms'], 60000)
        self.callbacks.validate_terminal({'worker_terminal': b'fixture'}, ('request',), False)
        self.lifecycle_validator.assert_called_once_with({'worker_terminal': b'fixture'}, ('request',), False)
        with self.assertRaises(control.SessionProtocolError):
            runtime.RunnerSemanticCallbacks(self.prepared, self.start, records=self.records,
                semantic_owner=self.semantic, first_ordinal=51, outer_timeout_ms=60000, validate_lifecycle=None)

    def test_readiness_must_match_exact_retained_process_observation(self):
        prefix = 'sessions/'+self.prepared['identity']['session_id']
        observed = {'ready': self.records.locate(prefix+'/ready.json'), 'process_scope': {'fixture_only': True}}
        ref = self.records.publish(prefix+'/process-ready.json', control.canonical(observed))
        self.semantic.observations.scope.initial = observed['process_scope']
        with mock.patch.object(self.semantic, 'validate_ready') as validate:
            bound = {'ready': self.records.read(observed['ready']), 'process_observation': self.records.read(ref)}
            self.callbacks.validate_ready(bound)
            validate.assert_called_once()
            with self.assertRaises(control.SessionProtocolError):
                self.callbacks.validate_ready(dict(bound, process_observation=control.canonical(dict(observed, process_scope={}))))


if __name__ == '__main__':
    unittest.main()
