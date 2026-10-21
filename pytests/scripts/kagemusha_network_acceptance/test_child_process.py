"""Natural tool supervision with inert children; never launches real programs."""
from pathlib import Path
import hashlib
import os
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import child_process as owner
import capture
import source_audit


class FakeChild:
    """Explicit terminal or interruption behavior with observable forbidden calls."""
    def __init__(self, stdout=b'output', stderr=b'error', code=0, error=None, pid=4312):
        self.stdout_data, self.stderr_data = stdout, stderr
        self.code, self.error, self.pid = code, error, pid
        self.returncode = None
        self.communications, self.signals, self.context_entries = 0, [], 0

    def communicate(self):
        self.communications += 1
        if self.error is not None:
            raise self.error
        self.returncode = self.code
        return self.stdout_data, self.stderr_data

    def kill(self):
        self.signals.append('kill')
        raise AssertionError('child must not be killed')

    def terminate(self):
        self.signals.append('terminate')
        raise AssertionError('child must not be terminated')

    def send_signal(self, value):
        self.signals.append(value)
        raise AssertionError('child must not be signaled')

    def __enter__(self):
        self.context_entries += 1
        raise AssertionError('Popen context cleanup is not the natural owner')


class ChildProcessTests(unittest.TestCase):
    def setUp(self):
        for module, name in ((subprocess, 'Popen'), (subprocess, 'run'),
                             (subprocess, 'check_output'), (subprocess, 'call'),
                             (subprocess, 'check_call'), (os, 'fork'), (os, 'forkpty'),
                             (os, 'posix_spawn'), (os, 'posix_spawnp'), (os, 'system'),
                             (os, 'kill'), (os, 'killpg'), (os, 'execve'), (os, 'execv')):
            guard = patch.object(module, name, side_effect=AssertionError('actual process/exec/signal forbidden'))
            guard.start()
            self.addCleanup(guard.stop)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def test_natural_terminal_preserves_byte_streams_and_requested_options(self):
        child = FakeChild()
        argv = ['inert-tool', 'argument']
        with patch.object(owner.subprocess, 'Popen', return_value=child) as spawn:
            result = owner.natural_run(argv, cwd=self.root, env={'EXACT': 'DATA'},
                                       check=True, stdout=subprocess.PIPE)
        spawn.assert_called_once_with(argv, cwd=self.root, env={'EXACT': 'DATA'}, stdout=subprocess.PIPE)
        self.assertEqual((result.args, result.returncode, result.stdout, result.stderr),
                         (argv, 0, b'output', b'error'))
        self.assertEqual(child.communications, 1)
        self.assertEqual(child.signals, [])
        self.assertEqual(child.context_entries, 0)

    def test_capture_output_and_text_are_explicit(self):
        child = FakeChild(stdout='text output', stderr='text diagnostic')
        with patch.object(owner.subprocess, 'Popen', return_value=child) as spawn:
            result = owner.natural_run(['inert'], capture_output=True, text=True, check=True)
        spawn.assert_called_once_with(['inert'], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(result.stdout, 'text output')
        self.assertEqual(result.stderr, 'text diagnostic')

    def test_known_nonzero_terminal_is_not_uncertain_and_keeps_streams(self):
        for checked in (False, True):
            child = FakeChild(code=9)
            with self.subTest(checked=checked), patch.object(owner.subprocess, 'Popen', return_value=child):
                if checked:
                    with self.assertRaises(subprocess.CalledProcessError) as caught:
                        owner.natural_run(['inert'], check=True)
                    self.assertEqual((caught.exception.returncode, caught.exception.output, caught.exception.stderr),
                                     (9, b'output', b'error'))
                else:
                    self.assertEqual(owner.natural_run(['inert']).returncode, 9)
            self.assertEqual(child.signals, [])

    def test_unreviewed_options_refuse_before_spawn(self):
        for options in ({'timeout': None}, {'input': None}, {'capture_output': True, 'stdout': None},
                        {'capture_output': True, 'stderr': None}):
            with self.subTest(options=options), patch.object(owner.subprocess, 'Popen') as spawn:
                with self.assertRaises(ValueError):
                    owner.natural_run(['inert'], **options)
                spawn.assert_not_called()

    def test_interrupted_communicate_retains_identity_and_never_signals(self):
        for failure in (KeyboardInterrupt('DATA'), SystemExit(2), OSError('DATA')):
            child = FakeChild(error=failure)
            argv = ['inert', '--query']
            with self.subTest(failure=type(failure).__name__), \
                    patch.object(owner.subprocess, 'Popen', return_value=child):
                with self.assertRaises(owner.UnfinishedChild) as caught:
                    owner.natural_run(argv, stdout=subprocess.PIPE)
            self.assertIs(caught.exception.__cause__, failure)
            self.assertEqual((caught.exception.argv, caught.exception.pid), (argv, child.pid))
            self.assertIn(str(child.pid), str(caught.exception))
            self.assertIn('--query', str(caught.exception))
            self.assertEqual(child.signals, [])
            self.assertEqual(child.context_entries, 0)

    def test_unknown_spawn_retains_command_without_inventing_pid(self):
        for failure in (OSError('unknown spawn outcome'), KeyboardInterrupt('spawn interrupted')):
            with self.subTest(failure=type(failure).__name__), \
                    patch.object(owner.subprocess, 'Popen', side_effect=failure):
                with self.assertRaises(owner.UnfinishedChild) as caught:
                    owner.natural_run(['inert', 'spawn'])
            self.assertIs(caught.exception.__cause__, failure)
            self.assertIsNone(caught.exception.pid)
            self.assertEqual(caught.exception.argv, ['inert', 'spawn'])
            self.assertIn('pid=None', str(caught.exception))

    def test_capture_git_text_and_snapshot_nul_bytes_use_one_owner(self):
        source = self.root / 'file with space.rs'
        source.write_bytes(b'inert source DATA')
        outputs = {
            ('git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'): b'file with space.rs\0',
            ('git', 'rev-parse', 'HEAD'): 'a' * 40 + '\n',
            ('git', 'branch', '--show-current'): 'optimizations\n',
            ('git', 'diff', '--name-only', '--diff-filter=U'): '',
        }
        calls = []
        def spawn(argv, **kwargs):
            calls.append((argv, kwargs))
            return FakeChild(stdout=outputs[tuple(argv)], stderr=None)
        with patch.object(capture, 'REPO', self.root), \
                patch.object(source_audit, 'package_paths', return_value=set()), \
                patch.object(owner.subprocess, 'Popen', side_effect=spawn):
            result = capture.snapshot()
        self.assertEqual(result['files'], {source.name: hashlib.sha256(source.read_bytes()).hexdigest()})
        self.assertEqual(result['head'], 'a' * 40)
        self.assertEqual(result['branch'], 'optimizations')
        self.assertEqual(result['unmerged'], [])
        self.assertEqual([tuple(argv) for argv, _ in calls], list(outputs))
        self.assertEqual(calls[0][1], {'cwd': self.root, 'stdout': subprocess.PIPE})
        for _, kwargs in calls[1:]:
            self.assertEqual(kwargs, {'cwd': self.root, 'text': True, 'stdout': subprocess.PIPE})

    def tool_outputs(self, platform):
        outputs = {('rustup', 'which', name): str(self.root / name) + '\n' for name in ('rustc', 'cargo')}
        if platform == 'darwin':
            outputs.update({('xcrun', '--find', name): str(self.root / name) + '\n' for name in ('clang', 'ld', 'ar')})
            outputs.update({('xcrun', '--show-sdk-path'): str(self.root / 'sdk') + '\n',
                            ('xcrun', '--show-sdk-version'): '26.0\n'})
        return outputs

    def tools(self, platform, spawn):
        with patch.object(source_audit.sys, 'platform', platform), \
                patch.object(source_audit.shutil, 'which', return_value=None), \
                patch.dict(os.environ, {}, clear=True), \
                patch.object(owner.subprocess, 'Popen', side_effect=spawn):
            return source_audit.tools_snapshot(self.root, lambda path: ('DATA:' + str(path)).encode())

    def test_every_linux_and_darwin_tool_query_preserves_exact_argv_and_text(self):
        for platform in ('linux', 'darwin'):
            outputs, calls = self.tool_outputs(platform), []
            def spawn(argv, **kwargs):
                calls.append((argv, kwargs))
                return FakeChild(stdout=outputs[tuple(argv)], stderr=None)
            with self.subTest(platform=platform):
                result = self.tools(platform, spawn)
                self.assertEqual([tuple(argv) for argv, _ in calls], list(outputs))
                for argv, kwargs in calls:
                    expected = {'text': True, 'stdout': subprocess.PIPE}
                    if argv[:2] != ['xcrun', '--show-sdk-path'] and argv[:2] != ['xcrun', '--show-sdk-version']:
                        expected['cwd'] = self.root
                    self.assertEqual(kwargs, expected)
                self.assertEqual(result['rustc']['path'], str(self.root / 'rustc'))
                self.assertEqual(result['cargo']['path'], str(self.root / 'cargo'))
                self.assertIn('child_process.py', result)
                if platform == 'darwin':
                    self.assertEqual(result['sdk'], {'path': str(self.root / 'sdk'), 'version': '26.0'})
                else:
                    self.assertNotIn('sdk', result)

    def test_uncertain_git_query_propagates_identity_without_followup(self):
        for caller, expected in (
                (lambda: capture.git('rev-parse', 'HEAD'), ['git', 'rev-parse', 'HEAD']),
                (capture.snapshot, ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'])):
            child = FakeChild(error=KeyboardInterrupt('DATA'))
            with self.subTest(expected=expected), patch.object(owner.subprocess, 'Popen', return_value=child) as spawn:
                with self.assertRaises(owner.UnfinishedChild) as caught:
                    caller()
                self.assertEqual((caught.exception.pid, caught.exception.argv), (child.pid, expected))
                self.assertEqual(spawn.call_count, 1)
                self.assertEqual(child.signals, [])

    def test_every_uncertain_tool_query_propagates_actual_command_and_pid(self):
        outputs = self.tool_outputs('darwin')
        for failed in outputs:
            calls, children = [], []
            def spawn(argv, **kwargs):
                calls.append(tuple(argv))
                child = FakeChild(stdout=outputs[tuple(argv)], stderr=None,
                                  error=KeyboardInterrupt('DATA') if tuple(argv) == failed else None,
                                  pid=5000 + len(calls))
                children.append(child)
                return child
            with self.subTest(failed=failed), self.assertRaises(owner.UnfinishedChild) as caught:
                self.tools('darwin', spawn)
            self.assertEqual(caught.exception.argv, list(failed))
            self.assertEqual(caught.exception.pid, children[-1].pid)
            self.assertEqual(calls[-1], failed)
            self.assertEqual(calls, list(outputs)[:list(outputs).index(failed) + 1])
            self.assertTrue(all(not child.signals for child in children))

    def test_query_nonzero_exit_propagates_checked_failure(self):
        child = FakeChild(stdout='DATA', stderr='diagnostic', code=7)
        with patch.object(owner.subprocess, 'Popen', return_value=child):
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                capture.git('rev-parse', 'HEAD')
        self.assertEqual(caught.exception.returncode, 7)
        self.assertEqual(caught.exception.output, 'DATA')
        self.assertEqual(caught.exception.cmd, ['git', 'rev-parse', 'HEAD'])
        self.assertEqual(child.signals, [])


if __name__ == '__main__':
    unittest.main()
