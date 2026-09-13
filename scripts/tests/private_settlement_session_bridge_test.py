"""Real pipe/process bridge tests using explicit nonqualifying synthetic records.

The fixture is a protocol peer, not an Iroha validator or measurement producer.
The production planner, durable record owners and control chains run unchanged.
"""

from __future__ import annotations


import io
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
for path in (Path(__file__).resolve().parents[1], Path(__file__).resolve().parent):
    sys.path.insert(0, str(path))
import private_settlement_release_runner as runner
import private_settlement_session_control as control
import private_settlement_session_bridge as bridge


def fixture_main(root: Path, config_path: Path, read_fd: int, write_fd: int) -> None:
    config = json.loads(config_path.read_bytes())
    prepared, start = config['prepared'], config['start']
    session = prepared['identity']
    prefix = f"sessions/{session['session_id']}/control"
    with control.RecordDirectory(root) as records, os.fdopen(read_fd, 'rb', buffering=0) as reader, \
            os.fdopen(write_fd, 'wb', buffering=0) as writer:
        outgoing = control.ControlChain(session, start['sha256'], 'runner_adapter', 'child_to_owner',
                                        records, observer='adapter', journal_prefix=prefix)
        incoming = control.ControlChain(session, start['sha256'], 'runner_adapter', 'owner_to_child',
                                        records, observer='adapter', journal_prefix=prefix)
        worker = control.ControlChain(session, start['sha256'], 'adapter_worker', 'child_to_owner',
                                     records, observer='worker', journal_prefix=prefix)
        def publish(name, value):
            return records.publish(f"sessions/{session['session_id']}/{name}.json",
                json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode())
        def forward(kind, payload, additions=None):
            original = worker.send(io.BytesIO(), kind, payload)
            if kind == 'session_completed':
                additions = {'adapter_lifecycle': publish('fixture-lifecycle', {'fixture_only': True, 'pid': os.getpid()})}
            outgoing.send(writer, kind, {**payload, **(additions or {})}, forwarded_from=original.binding)
        if config.get('setup_failed'):
            terminal = publish('fixture-worker-terminal', {**session, 'kind': 'failed',
                'accepted_request_ids': [], 'fixture_only': True, 'pid': os.getpid()})
            forward('session_completed', {'worker_terminal': terminal})
            return
        ready = publish('fixture-ready', {**session, 'fixture_only': True, 'pid': os.getpid()})
        observation = publish('fixture-process-observation', {'pid': os.getpid(), 'fixture_only': True})
        forward('ready', {'ready': ready}, {'process_observation': observation})
        accepted = []
        kind = 'completed'
        for index, row in enumerate(prepared['request']['attempts']):
            message = incoming.receive(reader).decoded()
            control.require(message['kind'] == 'dispatch', 'fixture expected dispatch')
            control.require(message['payload']['request'] == row['request'], 'fixture request changed')
            if config.get('eof_at') == index:
                return
            failed = config.get('failed_at') == index
            fields = {key: row[key] for key in control.ATTEMPT_FIELDS}
            terminal = publish(f'fixture-rust-{index}', {**fields, 'kind': 'failed' if failed else 'succeeded',
                'fixture_only': True, 'pid': os.getpid()})
            adapter = None if failed else publish(f'fixture-adapter-{index}', {**fields, 'fixture_only': True})
            response = None if failed else publish(f'fixture-response-{index}',
                {**fields, 'fixture_only': True, 'stages_ms': {'end_to_end': 1.25}})
            forward('attempt_completed', {**fields, 'rust_terminal': terminal},
                    {'adapter_outcome': adapter, 'response': response})
            owner = incoming.receive(reader).decoded()
            if owner['kind'] == 'stop':
                kind = 'failed'
                break
            control.require(owner['kind'] == 'accept', 'fixture expected acceptance')
            phase_path = root/prefix/f'ack-{index:06d}-validated.json'
            control.require(phase_path.exists(), 'worker received ACK before validation was durable')
            accepted.append(row['request_id'])
        if config.get('bad_terminal'):
            accepted = []
        terminal = publish('fixture-worker-terminal', {**session, 'kind': kind,
            'accepted_request_ids': accepted, 'fixture_only': True, 'pid': os.getpid()})
        forward('session_completed', {'worker_terminal': terminal})


class SessionBridgeTests(unittest.TestCase):
    def setUp(self):
        # Reuse only the existing fixture construction; inherited test methods
        # are not loaded into this suite.
        from private_settlement_session_records_test import SessionRecordTests
        self.fixture = SessionRecordTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.tearDown)
        self.root = self.fixture.campaign
        self.prepared = self.fixture.prepared()
        self.records = control.RecordDirectory(self.root)
        self.addCleanup(self.records.close)
        self.start = runner.publish_benchmark_session_start(self.prepared, records=self.records,
            command=[sys.executable, '-B', str(Path(__file__).resolve()), '--adapter-fixture'],
            harness={'sha256': 'f'*64, 'bytes': 100})
        self.child_pids = []
        self.validated = []
        self.accept_hook = None

    def publish_start(self, index, predecessor):
        return runner.publish_benchmark_attempt_start(self.prepared, index, records=self.records,
            session_started=self.start, ordinal=51+index, outer_timeout_ms=7200000,
            preceding_acceptance=predecessor)

    def validate_ready(self, bound):
        ready, observation = json.loads(bound['ready']), json.loads(bound['process_observation'])
        self.assertEqual(ready['pid'], observation['pid'])
        self.assertEqual(ready['pid'], self.child.pid)
        self.assertTrue(ready['fixture_only'])
        self.assertIsNone(self.child.poll())

    def validate_completion(self, index, bound):
        terminal = json.loads(bound['rust_terminal'])
        self.assertTrue(terminal['fixture_only'])
        self.assertEqual(terminal['request_id'], self.prepared['request']['attempts'][index]['request_id'])
        self.child_pids.append(terminal['pid'])
        reference = self.records.publish(f"sessions/{self.prepared['identity']['session_id']}/validation-{index}.json",
            control.canonical({'fixture_only': True, 'passed': terminal['kind'] == 'succeeded'}))
        if terminal['kind'] == 'failed':
            return bridge.CompletionDecision('failed', reference, None)
        self.assertEqual(json.loads(bound['response'])['stages_ms']['end_to_end'], 1.25)
        sample = self.records.publish(f"sessions/{self.prepared['identity']['session_id']}/sample-{index}.json",
            json.dumps({'fixture_only': True, 'end_to_end': 1.25, 'warmup': index < 5}).encode())
        return bridge.CompletionDecision('succeeded', reference, sample)

    def validate_acceptance(self, index, bound):
        self.assertTrue(json.loads(bound['validation'])['passed'])
        self.assertEqual(json.loads(bound['sample'])['end_to_end'], 1.25)
        self.assertIsNone(self.child.poll())
        # The next start cannot exist before this ACK is validated and written.
        if index+1 < len(self.prepared['request']['attempts']):
            following = self.prepared['request']['attempts'][index+1]
            self.assertFalse((self.root/following['output_directory']/'started.json').exists())
        self.validated.append(index)
        if self.accept_hook:
            self.accept_hook(index, bound)

    def validate_terminal(self, bound, accepted, complete):
        terminal = json.loads(bound['worker_terminal'])
        lifecycle = json.loads(bound['adapter_lifecycle'])
        self.assertTrue(lifecycle['fixture_only'])
        self.assertEqual(lifecycle['pid'], self.child.pid)
        self.assertEqual(terminal['accepted_request_ids'], list(accepted))
        self.assertEqual(terminal['kind'], 'completed' if complete else 'failed')
        self.assertEqual(terminal['pid'], self.child.pid)

    def drive(self, **options):
        config_path = self.root/'fixture-input.json'
        config_path.write_bytes(control.canonical({'prepared': self.prepared, 'start': self.start, **options}))
        with control.ChildControlPipes() as pipes:
            read_fd, write_fd = pipes.child_fds
            try:
                self.child = subprocess.Popen([sys.executable, '-B', str(Path(__file__).resolve()),
                    '--adapter-fixture', str(self.root), str(config_path), str(read_fd), str(write_fd)],
                    pass_fds=pipes.child_fds, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                    stderr=subprocess.PIPE)
            finally:
                pipes.child_spawn_finished()
            self.owner = bridge.RunnerSessionBridge(self.prepared, self.start, records=self.records,
                                                    reader=pipes.reader, writer=pipes.writer)
            try:
                return self.owner.run(publish_start=self.publish_start, validate_ready=self.validate_ready,
                    validate_completion=self.validate_completion, validate_acceptance=self.validate_acceptance,
                    validate_terminal=self.validate_terminal)
            finally:
                pipes.close()
                # Test peer exits naturally on the complete protocol or EOF.
                self.child.wait(timeout=15)
                self.child_stderr = self.child.stderr.read()
                self.child.stderr.close()

    def test_five_warmups_and_three_measurements_use_one_child_and_ack_chain(self):
        result = self.drive()
        self.assertEqual(self.child.returncode, 0, self.child_stderr.decode())
        self.assertEqual(len(result.started), 8)
        self.assertEqual(len(result.acknowledgements), 8)
        self.assertTrue(result.all_attempts_accepted)
        self.assertEqual(set(self.child_pids), {self.child.pid})
        self.assertEqual(self.validated, list(range(8)))
        for index, ref in enumerate(result.started):
            start = control.decode(self.records.read(ref))
            if index == 0:
                self.assertIsNone(start['preceding_acceptance'])
            else:
                self.assertEqual(control.RetainedMessage(self.records.read(start['preceding_acceptance']),
                    start['preceding_acceptance']).decoded()['payload']['session_attempt_index'], index-1)
            self.assertNotIn('exit_code', start)
            self.assertNotIn('owned_process_group_gone', start)

    def test_failed_warmup_stops_full_suffix_and_preserves_first_success(self):
        result = self.drive(failed_at=1)
        self.assertFalse(result.all_attempts_accepted)
        self.assertEqual(len(result.started), 2)
        self.assertEqual(len(result.acknowledgements), 1)
        self.assertEqual(self.child.returncode, 0)
        self.assertEqual(len(list(self.root.glob('attempts/*/started.json'))), 2)

    def test_measured_success_before_failure_is_retained(self):
        result = self.drive(failed_at=6)
        self.assertFalse(result.all_attempts_accepted)
        self.assertEqual(len(result.started), 7)
        self.assertEqual(len(result.acknowledgements), 6)
        self.assertEqual(self.validated, list(range(6)))

    def test_validation_failure_never_writes_ack_or_starts_successor(self):
        def reject(index, bound):
            if index == 5:
                raise ValueError('controlled semantic rejection')
        self.accept_hook = reject
        with self.assertRaises(bridge.SessionBridgeFailure) as caught:
            self.drive()
        self.assertEqual(len(caught.exception.started), 6)
        self.assertEqual(len(caught.exception.acknowledgements), 5)
        prefix = self.root/self.owner.prefix
        self.assertTrue((prefix/'ack-000005-proposed.json').exists())
        self.assertFalse((prefix/'ack-000005-validated.json').exists())
        self.assertFalse((prefix/'ack-000005-pipe-written.json').exists())

    def test_sample_mutation_during_validation_fails_before_ack_write(self):
        def mutate(index, bound):
            if index == 1:
                path = self.root/f"sessions/{self.prepared['identity']['session_id']}/sample-{index}.json"
                path.write_bytes(bound['sample'].replace(b'1.25', b'2.25'))
        self.accept_hook = mutate
        with self.assertRaises(bridge.SessionBridgeFailure) as caught:
            self.drive()
        self.assertEqual(len(caught.exception.acknowledgements), 1)
        self.assertFalse((self.root/self.owner.prefix/'ack-000001-validated.json').exists())

    def test_eof_is_incomplete_with_real_start_and_no_terminal_or_success(self):
        with self.assertRaises(bridge.SessionBridgeFailure) as caught:
            self.drive(eof_at=0)
        self.assertEqual(len(caught.exception.started), 1)
        self.assertEqual(caught.exception.acknowledgements, ())
        self.assertIsInstance(caught.exception.__cause__, control.SessionInterrupted)
        self.assertEqual(self.child.returncode, 0)

    def test_terminal_cannot_drop_previously_accepted_requests(self):
        with self.assertRaises(bridge.SessionBridgeFailure) as caught:
            self.drive(bad_terminal=True)
        self.assertEqual(len(caught.exception.acknowledgements), 8)
        self.assertEqual(self.child.returncode, 0)

    def test_bridge_cannot_retry_after_terminal(self):
        self.drive()
        with self.assertRaises(control.SessionProtocolError):
            self.owner.run(publish_start=self.publish_start, validate_ready=self.validate_ready,
                validate_completion=self.validate_completion, validate_acceptance=self.validate_acceptance,
                validate_terminal=self.validate_terminal)

    def test_setup_failure_retains_terminal_without_any_attempt_start(self):
        result = self.drive(setup_failed=True)
        self.assertFalse(result.all_attempts_accepted)
        self.assertEqual(result.started, ())
        self.assertEqual(result.acknowledgements, ())
        self.assertEqual(list(self.root.glob('attempts/*/started.json')), [])
        self.assertEqual(self.child.returncode, 0)

    def test_ack_delivery_record_failure_stops_before_any_successor_start(self):
        publish = self.records.publish
        def fail_delivery(name, raw):
            if name.endswith('ack-000001-pipe-written.json'):
                raise OSError('controlled publication failure')
            return publish(name, raw)
        self.records.publish = fail_delivery
        with self.assertRaises(bridge.SessionBridgeFailure) as caught:
            self.drive()
        self.assertEqual(len(caught.exception.started), 2)
        self.assertEqual(len(caught.exception.acknowledgements), 1)
        self.assertTrue((self.root/self.owner.prefix/'ack-000001-validated.json').exists())
        self.assertFalse((self.root/self.owner.prefix/'ack-000001-pipe-written.json').exists())
        self.assertEqual(len(list(self.root.glob('attempts/*/started.json'))), 2)


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == '--adapter-fixture':
        try:
            fixture_main(Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]))
        except (control.SessionInterrupted, BrokenPipeError):
            pass
    else:
        unittest.main()
