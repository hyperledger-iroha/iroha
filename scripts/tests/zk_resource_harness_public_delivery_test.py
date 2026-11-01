"""Finite normal-publication controls through the real public Linux harness.

This class requires independently configured publication inputs and an explicit
retention root. No native artifact is discovered, substituted or compiled here.
Faults act only on the owned tee fixture or its disposable observer worktree.
"""
from contextlib import redirect_stderr, redirect_stdout
import errno
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import sys
import unittest
from unittest import mock

from zk_resource_harness_test import NativeRunTests, harness, SCOPE, TEE
import linux_observer_delivery as delivery
import linux_owned_process_observer as owned


class PublicDeliveryFiniteTests(NativeRunTests):
    """Actual normal native owners and public acknowledgement, with retained evidence."""
    retention_root = None

    def setUp(self):
        """Require explicit custody inputs before creating any measurement fixture."""
        if not sys.platform.startswith('linux') or self.retention_root is None:
            raise RuntimeError('finite public delivery requires Linux and explicit retention root')
        super().setUp()
        self.owner = None
        self.public_error = None
        self.public_stdout = None
        self.events = []
        self.saved_reports = {}
        self.addCleanup(self._retain_actual_runs)
        original = delivery.DeliveryLifetime.__init__

        def remember(owner, *args, **kwargs):
            """Observe the actual owner before preparation without replacing its behavior."""
            original(owner, *args, **kwargs)
            self.owner = owner

        patcher = mock.patch.object(delivery.DeliveryLifetime, '__init__', remember)
        patcher.start()
        self.addCleanup(patcher.stop)

    def _retain_actual_runs(self):
        """Copy immutable public run evidence before the disposable test root is removed."""
        destination = Path(self.retention_root)/self._testMethodName
        destination.mkdir(mode=0o700)
        if self.root.exists():
            shutil.copytree(self.root, destination/'runs')
        facts = {'test': self._testMethodName, 'events': self.events,
                 'public_stdout': self.public_stdout,
                 'public_exception_type': None if self.public_error is None else type(self.public_error).__name__,
                 'original_reaped': self.owner is not None and self.owner.waited is not None,
                 'native_closed': self.owner is not None and self.owner.native.closed,
                 'delivery_closed': self.owner is not None and self.owner.module_owner.closed,
                 'report_hashes_at_publication': self.saved_reports}
        with (destination/'fixture-facts.json').open('x') as stream:
            json.dump(facts, stream, sort_keys=True)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())

    def run_harness(self, arguments, stdin=None):
        """Retain the actual public exception and stdout without fabricating a summary."""
        return self._invoke(self.publication_fixture.arguments(arguments), stdin)

    def _invoke(self, arguments, stdin=None):
        """Call the actual parser/handler with only explicit caller-supplied arguments."""
        parsed = harness.parser().parse_args(arguments)
        if stdin is not None:
            parsed.stdin = stdin
        out, err = io.StringIO(), io.StringIO()
        self.public_error = None
        with redirect_stdout(out), redirect_stderr(err):
            try:
                status = parsed.handler(parsed)
            except harness.HarnessRefusal as failure:
                self.public_error = failure
                self.public_stdout = out.getvalue()
                return 2, {'refused': str(failure)}, err.getvalue()
        self.public_stdout = out.getvalue()
        lines = [json.loads(line) for line in self.public_stdout.splitlines() if line]
        return status, lines[-1] if lines else None, err.getvalue()

    def _closed_original(self, *, success, delivery_closed=True):
        """Join actual initial sample, original status and genuine native/source close."""
        self.assertIsNotNone(self.owner)
        self.assertIsNotNone(self.owner.waited)
        self.assertEqual(self.owner.waited[0], self.owner.child.pid)
        self.assertEqual(self.owner.child.returncode, 0)
        self.assertTrue(self.owner.finish_attempted and self.owner.joined)
        self.assertTrue(self.owner.closed and self.owner.native.closed)
        self.assertEqual(self.owner.module_owner.closed, delivery_closed)
        self.assertIsNotNone(self.owner.native._initial_raw)
        if success:
            self.assertIsNone(self.owner.first)
            self.assertIsNone(self.owner.module_owner.first)

    def test_public_normal_positive_and_independent_replay(self):
        """Exercise the actual ordinary publisher-backed public positive and pin intake."""
        self.test_accepted_run_merges_the_record_with_kernel_process_observations()
        self._closed_original(success=True)
        self.assertTrue(self.initial_fixture_sample)
        acknowledgement = json.loads(self.public_stdout)
        run = Path(acknowledgement['run_directory'])
        verified = harness.read_pinned_run(run, acknowledgement['manifest_sha256'],
                                          **self.observation_requirement())
        self.assertEqual(verified['report']['observation_contract'], 'linux_owned_root_group_v1')
        self.assertEqual(verified['report']['verdict'], 'accepted')
        status, value, _ = self.run_harness([
            *self.salvage_scope(), '--manifest-sha256', acknowledgement['manifest_sha256'], str(run)])
        self.assertEqual((status, value['reasons']), (0, []))
        with self.assertRaises(harness.HarnessRefusal):
            harness.read_pinned_run(run, acknowledgement['manifest_sha256'],
                                    expected_observation_contract='ordinary_native_v1')
        with self.assertRaises(harness.HarnessRefusal):
            harness.read_pinned_run(run, None, **self.observation_requirement())

    def _direct_terminal(self, site):
        """Force only the reviewed actual Z site after a genuine live admission."""
        original_sample = owned.OwnedLinuxProcessReader.sample
        original_rss = owned._OwnedReads.resident_bytes
        triggered = []

        def wait_original(reader):
            """Wait nonreapingly for this fixture's exact installed original only."""
            self.assertIs(reader, self.owner.reader)
            self.assertIsNotNone(reader._admission_raw)
            terminal = os.waitid(os.P_PID, self.owner.child.pid, os.WEXITED | os.WNOWAIT)
            self.assertEqual(terminal.si_pid, self.owner.child.pid)
            triggered.append(site)

        def sample(reader, pid, image):
            """Rendezvous before the real retained stat operation, not an invented record."""
            if site == 'before' and reader._admission_raw is not None:
                wait_original(reader)
            return original_sample(reader, pid, image)

        def rss(reads):
            """Complete the real RSS read before the original transitions to Z."""
            result = original_rss(reads)
            if site == 'after' and reads.owner._admission_raw is not None:
                wait_original(reads.owner)
            return result

        with mock.patch.object(owned.OwnedLinuxProcessReader, 'sample', sample), \
                mock.patch.object(owned._OwnedReads, 'resident_bytes', rss):
            status, line, _ = self.run_tee()
        self.assertEqual((status, line['verdict'], line['reasons']), (0, 'accepted', []))
        self.assertEqual(triggered, [site])
        self._closed_original(success=True)
        run = Path(line['run_directory'])
        terminal = json.loads((run/'observations/linux-root-terminal.json').read_text())
        self.assertEqual(terminal['operation'], 'stat_'+site)
        self.assertEqual(terminal['stat']['state'], 'Z')
        verified = harness.read_pinned_run(run, line['manifest_sha256'], **self.observation_requirement())
        self.assertEqual(verified['reasons'], [])
        self.events.append({'terminal_site': site, 'actual_trigger_count': len(triggered)})

    def test_public_direct_z_before_stat(self):
        """Require accepted public output and pinned replay for the actual before site."""
        self._direct_terminal('before')

    def test_public_direct_z_after_rss(self):
        """Require accepted public output and pinned replay for the actual after site."""
        self._direct_terminal('after')

    def test_public_missing_and_wrong_pins_precede_workload(self):
        """Missing trust or changed module bytes cannot select an ordinary Linux fallback."""
        base = ['run', '--repository', str(self.repository), *SCOPE,
                '--output-root', str(self.root), '--', str(TEE)]
        complete = self.publication_fixture.arguments(base)
        flags = ['--observer-source-root', '--observer-bridge-root', '--trusted-observer-sha256',
                 '--trusted-observer-provenance-sha256', '--trusted-source-manifest-sha256',
                 '--observer-native-target']
        original = harness.subprocess.Popen
        launches = []

        def watch(*args, **kwargs):
            """Allow real pre-admission Git, but never an unnoticed measured child."""
            if 'IROHA_MEASUREMENT_OUTPUT_DIR' in (kwargs.get('env') or {}):
                launches.append(args)
                raise AssertionError('unadmitted measured workload')
            return original(*args, **kwargs)

        with mock.patch.object(harness.subprocess, 'Popen', watch):
            for flag in flags:
                missing = list(complete)
                index = missing.index(flag)
                del missing[index:index+2]
                status, value, _ = self._invoke(missing)
                self.assertEqual(status, 2)
                self.assertIn('refused', value)
                self.assertIsNotNone(self.public_error.__cause__)
            for flag in flags[2:5]:
                wrong = list(complete)
                index = wrong.index(flag)+1
                wrong[index] = ('f' if wrong[index][0] != 'f' else 'e') + wrong[index][1:]
                self.assertEqual(self._invoke(wrong)[0], 2)
        self.assertEqual(launches, [])
        self.assertFalse(self.root.exists())
        self.events.append({'missing_pins': 6, 'wrong_pins': 3, 'workload_launches': 0})

    def test_public_ordinary_first_survives_outer_unwind(self):
        """A real post-baseline failure remains first through one-shot final unwind."""
        first = OSError(errno.EIO, 'controlled public reader failure')
        original = owned.OwnedLinuxProcessReader.sample
        injected = []
        unwound = []

        def sample(reader, pid, image):
            """Keep the initial actual sample unchanged; then inject the declared error."""
            if reader._admission_raw is not None:
                injected.append(True)
                raise first
            return original(reader, pid, image)

        def trace(frame, event, arg):
            """Raise once at actual delivery completion entry after the earlier failure."""
            if (event == 'call' and frame.f_code is delivery.ObserverDelivery._complete.__code__
                    and injected and not unwound):
                unwound.append(True)
                raise KeyboardInterrupt('one-shot public cleanup fixture')
            return trace

        previous = sys.gettrace()
        try:
            sys.settrace(trace)
            with mock.patch.object(owned.OwnedLinuxProcessReader, 'sample', sample):
                status, line, _ = self.run_tee()
        finally:
            sys.settrace(previous)
        self.assertEqual(status, 2)
        self.assertIn('refused', line)
        self.assertEqual(self.public_stdout, '')
        self.assertTrue(injected)
        self.assertEqual(unwound, [True])
        self._closed_original(success=False)
        self.assertIs(self.owner.first, first)
        self.assertIs(self.owner.module_owner.first, first)
        self.assertIs(self.public_error.__cause__, first)
        self.events.append({'ordinary_injections': len(injected), 'outer_unwinds': len(unwound)})

    def test_public_late_real_census_failure_and_independent_intake(self):
        """Real late source mutation withholds trust while old report bytes stay intact."""
        status, good, _ = self.run_tee()
        self.assertEqual(status, 0)
        old_pin = good['manifest_sha256']
        source = Path(self.publication_fixture.source_root)/'README.md'
        original_bytes = source.read_bytes()
        original_publish = harness.control.RecordDirectory.publish
        mutations = []

        def publish(records, name, raw):
            """Mutate only the owned worktree after actual closed-native publication."""
            result = original_publish(records, name, raw)
            if name in ('report.json', 'manifest.json'):
                self.saved_reports[name] = hashlib.sha256(raw).hexdigest()
            if name == 'manifest.json':
                self._closed_original(success=True, delivery_closed=False)
                source.write_bytes(original_bytes+b'\n<!-- controlled late custody fixture -->\n')
                mutations.append(True)
            return result

        try:
            with mock.patch.object(harness.control.RecordDirectory, 'publish', publish):
                status, value, _ = self.run_tee()
            self.assertEqual(status, 2)
            self.assertIn('refused', value)
            self.assertEqual(mutations, [True])
            self.assertEqual(self.public_stdout, '')
            self._closed_original(success=False)
            self.assertIs(self.public_error.__cause__, self.owner.module_owner.first)
            self.assertIsNotNone(self.owner.module_owner.first)
        finally:
            if mutations:
                self.assertTrue(self.owner.module_owner.closed)
                source.write_bytes(original_bytes)
        runs = sorted(path for path in self.root.iterdir() if str(path) != good['run_directory'])
        self.assertEqual(len(runs), 1)
        run = runs[0]
        for name, expected in self.saved_reports.items():
            self.assertEqual(hashlib.sha256((run/name).read_bytes()).hexdigest(), expected)
        self.assertEqual(json.loads((run/'report.json').read_text())['verdict'], 'accepted')
        import check_state_commitment_candidates as commitments
        import fhe_bgv_cost_model as fhe
        for reader in (harness.read_pinned_run, commitments.load_run, fhe.read_run):
            for pin, reason in ((None, 'manifest_digest_required'), (old_pin, 'manifest_digest_differs')):
                with self.subTest(reader=reader.__name__, reason=reason), self.assertRaises(Exception) as got:
                    reader(run, pin, **self.observation_requirement())
                self.assertIn(reason, str(got.exception))
        self.events.append({'controlled_source_mutations': len(mutations),
                            'source_restored_after_close': source.read_bytes() == original_bytes,
                            'late_report_immutable': True, 'independent_reader_refusals': 6})


FINITE_NAMES = (
    'test_public_normal_positive_and_independent_replay',
    'test_public_direct_z_before_stat',
    'test_public_direct_z_after_rss',
    'test_public_missing_and_wrong_pins_precede_workload',
    'test_public_ordinary_first_survives_outer_unwind',
    'test_public_late_real_census_failure_and_independent_intake',
)
