"""Issuer preflight uses fake native listings, never creates custody or a network."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

import run as r


class IssuerPreflightTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=r.QUAL)
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()
        self.binaries = {name: {'path': '/inert/' + name, 'sha256': 'a' * 64}
                         for name in ('sumeragi', 'iroha_core_zk')}
        self.calls = []
        self.children = []

    def tearDown(self):
        r._UNFINISHED_CHILDREN.clear()

    def spawn(self, command, **kwargs):
        self.assertIn('--list', command)
        self.assertIn('--exact', command)
        self.assertNotIn('--nocapture', command)
        self.assertEqual(command[0], self.binaries[
            'sumeragi' if command[1].startswith('kagemusha_network_setup::') else 'iroha_core_zk']['path'])
        self.calls.append(command)
        kwargs['stdout'].write(command[1] + ': test\n\n1 test, 0 benchmarks\n')
        child = Mock(pid=3000 + len(self.calls))
        child.wait.return_value = 0
        self.children.append(child)
        return child

    def test_composer_is_only_listed_with_current_owner_and_every_phase_is_retained(self):
        with patch.object(r.subprocess, 'Popen', side_effect=self.spawn), \
             patch.object(r, 'unchanged') as unchanged, \
             patch.object(r, 'exact_test', side_effect=AssertionError('no test execution')):
            r.preflight(self.binaries, self.directory, {}, {}, {})
        self.assertEqual(unchanged.call_count, 2)
        self.assertEqual(len(self.calls), 16)
        self.assertEqual([command[1] for command in self.calls],
                         [test for _, test in r.PREFLIGHT_TESTS for _ in range(2)])
        composer = [command for command in self.calls if command[1] == r.ISSUER_COMPOSER]
        self.assertEqual(composer, [
            ['/inert/sumeragi', r.ISSUER_COMPOSER, '--exact', '--list'],
            ['/inert/sumeragi', r.ISSUER_COMPOSER, '--exact', '--list', '--ignored']])
        for number, (role, test) in enumerate(r.PREFLIGHT_TESTS):
            for stem in ('list', 'ignored-list'):
                receipt = r.read_document(self.directory / f'preflight-{number}-{stem}-process.json')
                self.assertEqual(receipt['binary'], self.binaries[role])
                self.assertEqual(receipt['command'][1], test)
                self.assertFalse(receipt['signals_permitted'])
                self.assertEqual(r.read_document(self.directory / f'preflight-{number}-{stem}-exit.json'), {'exit': 0})
        for child in self.children:
            child.kill.assert_not_called(); child.terminate.assert_not_called()

    def test_missing_ignored_composer_stops_preflight_before_later_phases(self):
        def absent(command, **kwargs):
            if command[1] == r.ISSUER_COMPOSER and '--ignored' in command:
                self.calls.append(command)
                kwargs['stdout'].write('0 tests, 0 benchmarks\n')
                child = Mock(pid=4000); child.wait.return_value = 0
                return child
            return self.spawn(command, **kwargs)
        with patch.object(r.subprocess, 'Popen', side_effect=absent), \
             patch.object(r, 'unchanged') as unchanged:
            with self.assertRaisesRegex(ValueError, 'exact qualified test absent'):
                r.preflight(self.binaries, self.directory, {}, {}, {})
        self.assertEqual(len(self.calls), 4)
        self.assertEqual(self.calls[-1][1], r.ISSUER_COMPOSER)
        self.assertEqual(unchanged.call_count, 1)

    def test_uncertain_composer_listing_stops_source_queries_and_later_children(self):
        def uncertain(command, **kwargs):
            child = self.spawn(command, **kwargs)
            if command[1] == r.ISSUER_COMPOSER:
                child.wait.side_effect = subprocess.TimeoutExpired(command, 60)
            return child
        with patch.object(r.subprocess, 'Popen', side_effect=uncertain), \
             patch.object(r, 'unchanged') as unchanged:
            with self.assertRaises(subprocess.TimeoutExpired):
                r.preflight(self.binaries, self.directory, {}, {}, {})
        self.assertEqual(len(self.calls), 3)
        self.assertEqual(unchanged.call_count, 1)
        self.assertEqual(r._UNFINISHED_CHILDREN[-1]['pid'], self.children[-1].pid)
        for child in self.children:
            child.kill.assert_not_called(); child.terminate.assert_not_called()

    def test_post_listing_source_drift_refuses_preflight_success(self):
        with patch.object(r.subprocess, 'Popen', side_effect=self.spawn), \
             patch.object(r, 'unchanged', side_effect=[None, ValueError('source drift')]):
            with self.assertRaisesRegex(ValueError, 'source drift'):
                r.preflight(self.binaries, self.directory, {}, {}, {})
        self.assertEqual(len(self.calls), 16)

    def test_controller_entry_refuses_pending_merge_before_selection_or_custody(self):
        git_dir = self.directory / '.git'; git_dir.mkdir()
        (git_dir / 'MERGE_HEAD').write_text('b' * 40 + '\n')
        args = ['run.py', 'a' * 64, '/inert/selection', 'b' * 64, '/inert/campaign']
        with patch.object(r.sys, 'argv', args), patch.object(r.sys, 'platform', 'linux'), \
             patch.object(r, '_AUTHENTICATED_RUNTIME', {}), \
             patch.object(r, 'authenticate_runtime', return_value={}), \
             patch.object(r.Path, 'cwd', return_value=r.ROOT), \
             patch.object(r.os, 'umask'), \
             patch.object(r.admission.capture, 'REPO', self.directory), \
             patch.object(r, 'document') as document, patch.object(r, 'compile_selection') as selection, \
             patch.object(r, 'tools') as tools, patch.object(r, 'private') as private, \
             patch.object(r.subprocess, 'Popen') as child:
            with self.assertRaisesRegex(ValueError, 'MERGE_HEAD'):
                r.main()
            for touched in (document, selection, tools, private, child): touched.assert_not_called()


if __name__ == '__main__':
    unittest.main()
