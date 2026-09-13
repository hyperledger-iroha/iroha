"""Control tests; synthetic vector records are never settlement measurements."""
from __future__ import annotations


import copy
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
for path in (Path(__file__).resolve().parents[1], Path(__file__).resolve().parent):
    sys.path.insert(0, str(path))
import private_settlement_session_control as control
import private_settlement_session_economics as economics
import private_settlement_process_observer as process


class EconomicControls(unittest.TestCase):
    def setUp(self):
        from private_settlement_session_records_test import SessionRecordTests
        self.fixture = SessionRecordTests(); self.fixture.setUp(); self.addCleanup(self.fixture.tearDown)
        self.root, self.prepared = self.fixture.campaign, self.fixture.prepared()
        self.records = control.RecordDirectory(self.root); self.addCleanup(self.records.close)
        self.row = self.prepared['request']['attempts'][0]
        self.ready = {**self.prepared['identity'], 'network_id': 'synthetic-network'}
        self.records.publish(f"sessions/{self.prepared['identity']['session_id']}/ready.json", control.canonical(self.ready))
        self.workload = {'economic_vector_sha256': '7'*64, 'canonical_economic_vector_hex': '00010203'}
        self.records.publish(self.row['output_directory']+'/evidence/matched-workload.json', control.canonical(self.workload))
        self.bound = economics.prepare_verification(self.prepared, self.row, records=self.records)

    def result(self):
        request = control.decode(self.bound['input_bytes']['benchmark_request'])
        return {**self.bound['identity'], **{key: self.row[key] for key in control.ATTEMPT_FIELDS},
            **{key: self.bound['document'][key] for key in economics.INPUT_NAMES},
            'kind': 'benchmark_economic_vector_verified', 'verified': True,
            'request_sha256': self.row['request']['sha256'], 'network_id': self.ready['network_id'],
            'workload_manifest_sha256': request['workload_manifest_sha256'],
            'economic_vector_sha256': self.workload['economic_vector_sha256'],
            'canonical_economic_vector_sha256': hashlib.sha256(bytes.fromhex(self.workload['canonical_economic_vector_hex'])).hexdigest(),
            'primary_payment_count': request['participants'],
            'monetary_movement_count': request['participants']+1,
            'verification_request': self.bound['reference']}

    def test_exact_native_result_joins_complete_inputs(self):
        result = self.result()
        self.assertEqual(economics.validate_vector_result(result, self.bound, records=self.records), result)
        self.assertNotIn('executed_native_verifier', result)

    def test_substituted_identity_network_and_payment_count_reject(self):
        for key, value in [('network_id', 'another-network'), ('request_sha256', '8'*64),
                ('session_id', '9'*64), ('primary_payment_count', True),
                ('monetary_movement_count', self.result()['primary_payment_count']),
                ('economic_vector_sha256', '8'*64), ('canonical_economic_vector_sha256', '8'*64),
                ('verified', 1), ('kind', 'verified'), ('extra', True)]:
            with self.subTest(key=key):
                result = self.result(); result[key] = value
                with self.assertRaises(control.SessionProtocolError):
                    economics.validate_vector_result(result, self.bound, records=self.records)

    def test_rehashed_input_reference_cannot_replace_frozen_input(self):
        result = self.result()
        ref = self.bound['document']['workload_record']
        path = self.root/ref['path']
        changed = dict(self.workload, economic_vector_sha256='9'*64)
        path.write_bytes(control.canonical(changed))
        result['workload_record'] = self.records.locate(ref['path'])
        result['economic_vector_sha256'] = changed['economic_vector_sha256']
        with self.assertRaises(control.SessionProtocolError):
            economics.validate_vector_result(result, self.bound, records=self.records)

    def test_native_result_rejects_boolean_identity_coordinates(self):
        for key, value in (('version', True), ('session_attempt_index', False)):
            with self.subTest(key=key):
                result = self.result(); result[key] = value
                # Round-trip through the actual canonical decoder; JSON boolean
                # values must not qualify by Python's bool/int equality.
                result = control.decode(control.canonical(result))
                with self.assertRaises(control.SessionProtocolError):
                    economics.validate_vector_result(result, self.bound, records=self.records)

    def exit_owner(self):
        """Construct only a wait-owner fixture; it cannot qualify a verifier."""
        owner = economics.NativeVectorInvocation.__new__(economics.NativeVectorInvocation)
        owner.process, owner.reaped = mock.Mock(pid=123456), False
        owner.exit_observation, owner.exit_reference = None, None
        owner.deadline_ns = time.monotonic_ns()-1
        owner.records = self.records
        owner.prefix = str(Path(self.bound['output_path']).parent) + '/native-vector-process'
        owner.launch = self.records.publish(owner.prefix+'-launch.json', control.canonical({
            'fixture_only': True, 'deadline_monotonic_ns': owner.deadline_ns}))
        return owner

    def test_late_natural_exit_is_retained_after_an_expired_pending_wait(self):
        owner = self.exit_owner()
        owner.process.wait.side_effect = [subprocess.TimeoutExpired('fixture', 0), 0]
        with self.assertRaises(economics.NativeVectorIncomplete) as caught:
            owner.observe_exit()
        self.assertIs(caught.exception.invocation, owner)
        self.assertFalse(owner.reaped)
        self.assertFalse((self.root/(owner.prefix+'-exit.json')).exists())
        reference = owner.observe_exit()
        observed = control.decode(self.records.read(reference))
        self.assertTrue(owner.reaped)
        self.assertEqual(observed['exit_code'], 0)
        self.assertTrue(observed['natural_wait_observed'])
        self.assertGreater(observed['observed_monotonic_ns'], owner.deadline_ns)
        self.assertEqual(owner.observe_exit(), reference)
        self.assertEqual(owner.process.wait.call_args_list, [mock.call(timeout=0)]*2)
        owner.process.kill.assert_not_called(); owner.process.terminate.assert_not_called()
        self.assertFalse((self.root/(owner.prefix+'-verified.json')).exists())

    def test_exit_publication_failure_retains_actual_wait_for_later_publication(self):
        owner = self.exit_owner(); owner.process.wait.return_value = 1
        publish = self.records.publish
        with mock.patch.object(self.records, 'publish', side_effect=OSError('fixture destination failure')):
            with self.assertRaises(OSError): owner.observe_exit()
        observed = copy.deepcopy(owner.exit_observation)
        self.assertTrue(owner.reaped); self.assertIsNone(owner.exit_reference)
        self.assertEqual(owner.observe_exit(), owner.exit_reference)
        self.assertEqual(control.decode(self.records.read(owner.exit_reference)), observed)
        owner.process.wait.assert_called_once_with(timeout=0)
        self.assertFalse((self.root/(owner.prefix+'-verified.json')).exists())

    def test_post_wait_log_publication_failure_preserves_real_native_exit(self):
        path = Path('/usr/bin/false').resolve(strict=True)
        with process.ExecutableImage(path, hashlib.sha256(path.read_bytes()).hexdigest()) as image:
            owner = economics.NativeVectorInvocation(self.bound, records=self.records,
                image=image, cwd=ROOT, deadline_ns=time.monotonic_ns()+30_000_000_000)
            self.addCleanup(owner.close)
            publish = self.records.publish
            def fail_logs(name, raw):
                if name == owner.prefix+'-logs.json':
                    raise OSError('fixture log-publication failure')
                return publish(name, raw)
            with mock.patch.object(self.records, 'publish', side_effect=fail_logs):
                with self.assertRaisesRegex(OSError, 'fixture log-publication failure'):
                    owner.finish(process_reader=None)
            reference = owner.observe_exit()
            observed = control.decode(self.records.read(reference))
            self.assertEqual(observed['exit_code'], 1)
            self.assertTrue(observed['natural_wait_observed'])
            self.assertTrue(owner.reaped)
            self.assertTrue(owner.stdout.stream.closed); self.assertTrue(owner.stderr.stream.closed)
            self.assertEqual(owner.stdout.parents, []); self.assertEqual(owner.stderr.parents, [])
            self.assertFalse((self.root/(owner.prefix+'-verified.json')).exists())

    def test_successful_nonverifier_exit_cannot_qualify_after_named_test_rejection(self):
        # An actual zero exit is insufficient without the exact native test.
        path = Path('/usr/bin/true').resolve(strict=True)
        with process.ExecutableImage(path, hashlib.sha256(path.read_bytes()).hexdigest()) as image:
            reader = process.native_reader()
            self.addCleanup(reader.close)
            owner = economics.NativeVectorInvocation(self.bound, records=self.records,
                image=image, cwd=ROOT, deadline_ns=time.monotonic_ns()+30_000_000_000)
            self.addCleanup(owner.close)
            with self.assertRaisesRegex(control.SessionProtocolError, 'exactly one successful named test'):
                owner.finish(process_reader=reader)
            reference = owner.observe_exit()
            terminal = control.decode(self.records.read(self.records.locate(owner.prefix+'-terminal.json')))
            self.assertEqual(terminal['exit_code'], 0)
            self.assertEqual(terminal['exit_observation'], reference)
            self.assertEqual(terminal['group_before']['members'], [])
            self.assertEqual(terminal['group_after']['members'], [])
            self.assertFalse((self.root/(owner.prefix+'-verified.json')).exists())

    def test_different_attempt_is_not_prepared(self):
        row = copy.deepcopy(self.row); row['attempt_id'] = '9'*64
        with self.assertRaises(control.SessionProtocolError):
            economics.prepare_verification(self.prepared, row, records=self.records)

    def test_unsafe_norito_encoding_rejects(self):
        for raw in ('', 'ABCDEF', '001', 'gg', True):
            with self.subTest(raw=raw):
                bound = copy.deepcopy(self.bound)
                changed = dict(self.workload, canonical_economic_vector_hex=raw)
                bound['input_bytes']['workload_record'] = control.canonical(changed)
                fake = mock.Mock(wraps=self.records)
                fake.read.side_effect = lambda ref: (bound['input_bytes']['workload_record']
                    if ref == bound['document']['workload_record'] else self.records.read(ref))
                with self.assertRaises(control.SessionProtocolError):
                    economics.validate_vector_result(self.result(), bound, records=fake)

    def test_locate_and_bound_read_share_exact_identity_checks(self):
        ref = self.records.locate(self.row['request']['path'])
        self.assertEqual(ref, self.row['request'])
        self.assertEqual(self.records.read(ref), self.bound['input_bytes']['benchmark_request'])
        path = self.root/ref['path']; before = path.read_bytes()
        path.unlink(); path.symlink_to(self.root/self.bound['document']['ready']['path'])
        with self.assertRaises(OSError): self.records.locate(ref['path'])
        path.unlink(); path.write_bytes(before); path.chmod(0o600)

    def test_locate_rejects_hardlink_and_public_file(self):
        path = self.root/self.row['request']['path']
        extra = path.with_name('extra-link.json'); os.link(path, extra)
        with self.assertRaises(control.SessionProtocolError): self.records.locate(self.row['request']['path'])
        extra.unlink(); path.chmod(0o644)
        with self.assertRaises(control.SessionProtocolError): self.records.locate(self.row['request']['path'])

    def test_locate_rejects_replacement_during_descriptor_read(self):
        path = self.root/self.row['request']['path']; original = os.read
        replaced = False
        def substitute(fd, length):
            nonlocal replaced
            raw = original(fd, length)
            if not replaced:
                replaced = True
                other = path.with_name('substituted.json'); other.write_bytes(path.read_bytes()); other.chmod(0o600)
                os.replace(other, path)
            return raw
        with mock.patch.object(control.os, 'read', side_effect=substitute):
            with self.assertRaises(control.SessionProtocolError): self.records.locate(self.row['request']['path'])

    def test_native_nonverifier_failure_has_real_natural_terminal(self):
        # /usr/bin/false is an actual negative native process fixture. It is not
        # an admitted Iroha verifier and cannot create a verified execution.
        path = Path('/usr/bin/false').resolve(strict=True)
        with process.ExecutableImage(path, hashlib.sha256(path.read_bytes()).hexdigest()) as image:
            reader = process.native_reader()
            try:
                invocation = economics.NativeVectorInvocation(self.bound, records=self.records,
                    image=image, cwd=ROOT, deadline_ns=time.monotonic_ns()+30_000_000_000)
                with self.assertRaises(control.SessionProtocolError): invocation.finish(process_reader=reader)
                self.assertTrue(invocation.reaped)
                terminal = control.decode(self.records.read(self.records.locate(invocation.prefix+'-terminal.json')))
                self.assertEqual(terminal['exit_code'], 1)
                self.assertTrue(terminal['natural_wait_observed'])
                self.assertEqual(terminal['group_after']['members'], [])
                self.assertFalse((self.root/(invocation.prefix+'-verified.json')).exists())
            finally:
                reader.close()

    def test_owned_log_rejects_redirected_parent_before_creation(self):
        directory = self.root/self.row['output_directory']/'evidence'/'benchmark-protocol'
        moved = directory.with_name('protocol-original')
        outside = self.root/'unrelated-output'; outside.mkdir(mode=0o700)
        directory.rename(moved); directory.symlink_to(outside, target_is_directory=True)
        try:
            with self.assertRaises(OSError):
                economics.OwnedNativeLog(self.records,
                    self.row['output_directory']+'/evidence/benchmark-protocol/new.log')
            self.assertEqual(list(outside.iterdir()), [])
        finally:
            directory.unlink(); moved.rename(directory)

    def test_owned_log_reads_exact_fd_bytes_and_detects_parent_replacement(self):
        name = self.row['output_directory']+'/evidence/benchmark-protocol/owned.log'
        log = economics.OwnedNativeLog(self.records, name); self.addCleanup(log.close)
        os.write(log.fileno(), b'native bytes')
        self.assertEqual(log.completed_bytes(), b'native bytes')
        directory = (self.root/name).parent; moved = directory.with_name('protocol-original')
        directory.rename(moved); directory.mkdir(mode=0o700)
        try:
            with self.assertRaises(control.SessionProtocolError): log.completed_bytes()
        finally:
            directory.rmdir(); moved.rename(directory)

    def test_failed_spawn_preserves_launch_without_invented_pid(self):
        image = mock.Mock(path=Path('/not-invoked-native'), sha256='8'*64)
        with mock.patch.object(economics.subprocess, 'Popen', side_effect=OSError('fixture')):
            with self.assertRaises(economics.NativeVectorIncomplete) as caught:
                economics.NativeVectorInvocation(self.bound, records=self.records,
                    image=image, cwd=ROOT, deadline_ns=time.monotonic_ns()+30_000_000_000)
        owner = caught.exception.invocation
        self.assertIsNone(owner.process)
        self.assertTrue((self.root/(owner.prefix+'-launch.json')).exists())
        self.assertFalse((self.root/(owner.prefix+'-start.json')).exists())

    def test_expired_wait_retains_actual_owner_without_signal(self):
        owner = economics.NativeVectorInvocation.__new__(economics.NativeVectorInvocation)
        owner.process, owner.reaped = mock.Mock(), False
        owner.process.wait.side_effect = subprocess.TimeoutExpired('fixture', 0)
        owner.deadline_ns = time.monotonic_ns()-1
        with self.assertRaises(economics.NativeVectorIncomplete) as caught:
            owner.finish(process_reader=None)
        self.assertIs(caught.exception.invocation, owner)
        owner.process.wait.assert_called_once_with(timeout=0); owner.process.kill.assert_not_called()
        owner.process.terminate.assert_not_called(); self.assertFalse(owner.reaped)


class NativeTerminalControls(unittest.TestCase):
    def good(self):
        return ('running 1 test\ntest '+economics.VERIFIER_TEST+' ... ok\n'
                'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.01s\n').encode()

    def test_named_native_test_is_required(self):
        economics.native_test_passed(self.good(), b'')
        for raw in (self.good().replace(b'1 passed', b'0 passed'),
                    self.good().replace(b'0 ignored', b'1 ignored'),
                    self.good().replace(economics.VERIFIER_TEST.encode(), b'other_test'),
                    self.good()+self.good(), b'', self.good().replace(b'running 1 test', b'running 0 tests')):
            with self.subTest(raw=raw[:45]):
                with self.assertRaises(control.SessionProtocolError): economics.native_test_passed(raw, b'')

    def test_logs_cannot_exceed_bound(self):
        with self.assertRaises(control.SessionProtocolError):
            economics.native_test_passed(self.good(), b'x'*(economics.MAX_LOG_BYTES+1))


if __name__ == '__main__':
    unittest.main()
