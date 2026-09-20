"""Offline safety and continuity checks; never signal a real nginx process."""
import argparse
import errno
import importlib.util
import io
import json
import os
from pathlib import Path
import plistlib
import stat
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('taira_nginx_logrotate', ROOT / 'scripts/taira_nginx_logrotate.py')
rotation = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rotation)


class RotationTests(unittest.TestCase):
    def setUp(self):
        # Use the owned repository target, not a world-writable /tmp ancestor.
        (ROOT / 'target').mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='nginx-rotation-test-', dir=ROOT / 'target')
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.logs = [self.base / 'access.log', self.base / 'error.log']
        for path in self.logs:
            path.touch(mode=0o600)
        self.pid_file = self.base / 'nginx.pid'
        self.pid_file.write_text('16392\n')
        self.pid_file.chmod(0o600)
        self.state = self.base / 'state'
        self.state.mkdir(mode=0o700)
        self.settings = {'logs': list(map(str, self.logs)), 'pid_file': str(self.pid_file), 'count': 4,
                         'size_kib': 32768, 'hours': 24, 'interval_seconds': 300}
        for name, value in [('ALLOWED_LOGS', frozenset(self.logs)), ('PID_FILE', self.pid_file)]:
            patcher = mock.patch.object(rotation, name, value)
            patcher.start()
            self.addCleanup(patcher.stop)
        patcher = mock.patch.object(rotation.signal, 'SIGUSR1', 30)
        patcher.start()
        self.addCleanup(patcher.stop)
        rotation.write_private(self.state / 'settings.json', json.dumps(self.settings).encode())
        rotation.write_private(self.state / 'newsyslog.conf', rotation.config_bytes(self.logs, self.pid_file, 4, 32768, 24))

    def test_rejects_outside_selected_logs_before_commands(self):
        with mock.patch.object(rotation, 'command') as command:
            for paths in ([], self.logs * 2, [self.base / 'postgres.data']):
                with self.assertRaises(rotation.RotationError):
                    rotation.selected_logs(paths)
            command.assert_not_called()

    def test_rejects_symlink_and_hardlink_logs(self):
        log = self.logs[0]
        other = self.base / 'other'
        log.rename(other)
        log.symlink_to(other)
        with self.assertRaises(rotation.RotationError):
            rotation.selected_logs(self.logs)
        log.unlink()
        os.link(other, log)
        with self.assertRaises(rotation.RotationError):
            rotation.selected_logs(self.logs)

    def test_config_has_exact_paths_private_mode_usr1_and_bounded_compression(self):
        data = rotation.config_bytes(self.logs, self.pid_file, 4, 32768, 24).decode().splitlines()
        self.assertEqual(len(data), 3)
        for row, log in zip(data[1:], self.logs):
            self.assertEqual(row.split(), [str(log), f'{os.getuid()}:{log.stat().st_gid}', '600', '3',
                                           '32768', '24', 'BZ', str(self.pid_file), '30'])
        for count, size, hours in ((0, 32768, 24), (1, 32768, 24), (11, 32768, 24), (4, 1, 24), (4, 32768, 0)):
            with self.assertRaises(rotation.RotationError):
                rotation.config_bytes(self.logs, self.pid_file, count, size, hours)

    @unittest.skipUnless(sys.platform == 'darwin', 'macOS newsyslog archive semantics')
    def test_macos_newsyslog_enforces_exact_retained_archive_count(self):
        # Real system tool, tiny private fixtures, N explicitly disables signals.
        # The generated count is tested beyond saturation, not just the first run.
        for count, log in zip((2, 4), self.logs):
            with self.subTest(count=count):
                rows = rotation.config_bytes([log], self.pid_file, count, 1024, 24).decode().splitlines()
                fields = rows[1].split()
                self.assertEqual(fields[-3:], ['BZ', str(self.pid_file), '30'])
                config = self.base / f'offline-{count}.conf'
                rotation.write_private(config, (' '.join([*fields[:-3], 'BZN']) + '\n').encode())
                for generation in range(count + 2):
                    log.write_bytes(b'owned offline fixture\n')
                    child = subprocess.run([rotation.NEWSYSLOG, '-r', '-F', '-f', str(config), str(log)],
                                           stdin=subprocess.DEVNULL, capture_output=True, timeout=15)
                    self.assertEqual(child.returncode, 0, child.stderr.decode())
                    archives = rotation.archive_metadata(log, count, compressed_only=True)
                    self.assertEqual({path.name for path in archives},
                                     {f'{log.name}.{index}.gz' for index in range(min(generation + 1, count))})

    def test_pid_file_and_master_identity(self):
        output = f'{os.getuid()} Mon Sep 7 12:34:56 2026 nginx: master process /opt/homebrew/opt/nginx/bin/nginx\n'.encode()
        with mock.patch.object(rotation, 'command', return_value=output):
            self.assertEqual(rotation.master_identity(self.pid_file, 16392), (16392, 'Mon Sep 7 12:34:56 2026'))
            with self.assertRaises(rotation.RotationError):
                rotation.master_identity(self.pid_file, 999)
        for value in ('0\n', '16392\n123\n', '16392; echo secret\n', '1\n'):
            self.pid_file.write_text(value)
            with mock.patch.object(rotation, 'command') as command:
                with self.assertRaises(rotation.RotationError):
                    rotation.master_identity(self.pid_file)
                command.assert_not_called()

    def test_foreign_process_is_never_signalled(self):
        for identity in (f'{os.getuid() + 1} Mon Sep 7 12:34:56 2026 nginx: master process /nginx',
                         f'{os.getuid()} Mon Sep 7 12:34:56 2026 postgres'):
            with mock.patch.object(rotation, 'command', return_value=identity.encode()):
                with self.assertRaises(rotation.RotationError):
                    rotation.master_identity(self.pid_file)

    def test_lsof_parses_both_device_inode_orders_and_requires_master(self):
        fields = b'p16392\nf4\ni123\nD0xa\nf5\nD0xb\ni456\np16393\nf4\ni789\nD0xc\n'
        with mock.patch.object(rotation, 'command', return_value=fields):
            self.assertEqual(rotation.open_inodes({16392, 16393}, 16392), {(10, 123), (11, 456), (12, 789)})
        with mock.patch.object(rotation, 'command', return_value=b'p16393\n'):
            with self.assertRaises(rotation.RotationError):
                rotation.open_inodes({16392, 16393}, 16392)

    def emulate_rotation(self, argv, **_kwargs):
        self.assertEqual(argv, [rotation.NEWSYSLOG, '-r', '-f', str(self.state / 'newsyslog.conf'), '-F',
                                *map(str, self.logs)])
        for log in self.logs:
            # Move the inode and create a new one without reading any contents.
            log.rename(log.with_name(log.name + '.0.gz'))
            log.touch(mode=0o600)
        return b''

    def run_rotation(self, *, effect=None, descriptors=None, identity=None):
        with mock.patch.object(rotation, 'master_identity', side_effect=identity or [(16392, 'start'), (16392, 'start')]), \
             mock.patch.object(rotation, 'nginx_processes', return_value={16392, 16393}), \
             mock.patch.object(rotation, 'open_inodes', return_value=descriptors or set()), \
             mock.patch.object(rotation, 'command', side_effect=effect or self.emulate_rotation):
            return rotation.rotate(self.state, force=True, expected_pid=16392)

    def test_forced_rotation_preserves_master_and_closes_old_descriptors(self):
        report = self.run_rotation()
        self.assertEqual(report['rotated_logs'], list(map(str, self.logs)))
        self.assertTrue(report['nginx_master_unchanged'])
        self.assertTrue(report['old_log_descriptors_closed'])
        self.assertFalse(report['log_contents_read'])

    def test_old_descriptor_retention_is_reported_as_failure(self):
        info = self.logs[0].stat()
        with self.assertRaisesRegex(rotation.RotationError, 'still holds'):
            self.run_rotation(descriptors={(info.st_dev, info.st_ino)})

    def test_master_change_is_reported_as_failure(self):
        with self.assertRaisesRegex(rotation.RotationError, 'master changed'):
            self.run_rotation(identity=[(16392, 'start'), (16392, 'different-start')])

    def test_force_noop_cannot_report_success(self):
        with self.assertRaisesRegex(rotation.RotationError, 'did not replace'):
            self.run_rotation(effect=lambda *_args, **_kwargs: b'')

    def test_tampered_config_rejects_before_newsyslog(self):
        (self.state / 'newsyslog.conf').write_text('/some/other/log\n')
        with mock.patch.object(rotation, 'master_identity', return_value=(16392, 'start')), \
             mock.patch.object(rotation, 'nginx_processes', return_value={16392}), \
             mock.patch.object(rotation, 'command') as command:
            with self.assertRaisesRegex(rotation.RotationError, 'config differs'):
                rotation.rotate(self.state)
            command.assert_not_called()

    def test_archives_reject_unknown_generations_duplicates_and_uncompressed_final(self):
        log = self.logs[0]
        for suffix in ('.4.gz', '.backup'):
            archive = log.with_name(log.name + suffix)
            archive.touch(mode=0o600)
            with self.assertRaises(rotation.RotationError):
                rotation.archive_metadata(log, 4)
            archive.unlink()
        raw = log.with_name(log.name + '.0')
        raw.touch(mode=0o600)
        with self.assertRaises(rotation.RotationError):
            rotation.archive_metadata(log, 4, compressed_only=True)
        raw.with_name(raw.name + '.gz').touch(mode=0o600)
        with self.assertRaisesRegex(rotation.RotationError, 'Duplicate'):
            rotation.archive_metadata(log, 4)

    def test_rotation_lock_rejects_concurrent_operation(self):
        with rotation.rotation_lock(self.state):
            with self.assertRaisesRegex(rotation.RotationError, 'already running'):
                with rotation.rotation_lock(self.state):
                    self.fail('Second operation acquired lock')

    def test_finished_archives_must_be_owner_only(self):
        log = self.logs[0]
        archive = log.with_name(log.name + '.0.gz')
        archive.touch(mode=0o644)
        archive.chmod(0o644)
        with self.assertRaisesRegex(rotation.RotationError, 'archive is not owner-only'):
            rotation.archive_metadata(log, 4, compressed_only=True)

    def test_private_settings_and_types_required(self):
        path = self.state / 'settings.json'
        path.chmod(0o644)
        with self.assertRaises(rotation.RotationError):
            rotation.load_settings(self.state)
        path.chmod(0o600)
        self.settings['count'] = True
        path.write_text(json.dumps(self.settings))
        with self.assertRaisesRegex(rotation.RotationError, 'settings types'):
            rotation.load_settings(self.state)

    def test_install_writes_private_config_and_direct_launch_agent(self):
        args = argparse.Namespace(log=self.logs, pid_file=self.pid_file, expected_pid=16392,
                                  interval_seconds=300, count=4, size_kib=32768, hours=24, state_dir=self.state)
        with mock.patch.object(rotation, 'master_identity', return_value=(16392, 'start')), \
             mock.patch.object(rotation, 'nginx_processes', return_value={16392}), \
             mock.patch.object(rotation.Path, 'home', return_value=self.base), \
             mock.patch.object(rotation.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1)) as launchctl, \
             mock.patch.object(rotation, 'command') as command:
            result = rotation.install(args)
            plist = Path(result['launch_agent'])
            self.assertEqual(command.call_args_list, [
                mock.call([rotation.NEWSYSLOG, '-r', '-f', str(self.state / 'newsyslog.conf'), *map(str, self.logs)]),
                mock.call(['/bin/launchctl', 'bootstrap', f'user/{os.getuid()}', str(plist)])])
            self.assertEqual(launchctl.call_args.args[0], ['/bin/launchctl', 'print', f'user/{os.getuid()}/{rotation.LABEL}'])
        self.assertTrue(result['rotation_verified'])
        self.assertEqual(rotation.load_health(self.state)['last_attempt']['outcome'], 'succeeded')
        for path in [plist, *(self.state / name for name in ('rotate.py', 'settings.json', 'newsyslog.conf'))]:
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        agent = plistlib.loads(plist.read_bytes())
        self.assertEqual(agent['ProgramArguments'], ['/usr/bin/python3', '-I', '-B', str(self.state / 'rotate.py'),
                                                     'run', '--state-dir', str(self.state)])
        self.assertEqual(agent['StartInterval'], 300)
        self.assertFalse(agent['RunAtLoad'])
        self.assertEqual(agent['LimitLoadToSessionType'], 'Background')
        self.assertEqual(agent['StandardOutPath'], '/dev/null')
        self.assertEqual(agent['StandardErrorPath'], '/dev/null')

    def test_command_never_discloses_subprocess_output(self):
        child = subprocess.CompletedProcess([], 1, b'PRIVATE LOG CONTENT', b'PRIVATE LOG CONTENT')
        with mock.patch.object(rotation.subprocess, 'run', return_value=child):
            with self.assertRaisesRegex(rotation.RotationError, '^newsyslog failed with exit 1$'):
                rotation.command([rotation.NEWSYSLOG])

    def test_command_reports_safe_actionable_disk_failure(self):
        child = subprocess.CompletedProcess([], 1, b'', b'PRIVATE LOG CONTENT: No space left on device')
        with mock.patch.object(rotation.subprocess, 'run', return_value=child):
            with self.assertRaisesRegex(rotation.RotationError, '^newsyslog failed with exit 1: No space left on device$'):
                rotation.command([rotation.NEWSYSLOG])

    def healthy_scheduler(self):
        return mock.patch.object(rotation, 'scheduler_status', return_value={
            'loaded': True, 'state': 'not running', 'last_exit_status': 0})

    def test_success_records_private_bounded_health_and_status_is_read_only(self):
        self.run_rotation()
        path = self.state / 'health.json'
        health = rotation.load_health(self.state)
        self.assertEqual(health['last_attempt'], health['last_success'])
        self.assertEqual(health['last_attempt']['phase'], 'complete')
        self.assertIsNone(health['last_attempt']['reason'])
        self.assertEqual(health['last_attempt']['settings_sha256'], rotation.settings_digest(self.settings))
        self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        self.assertLess(path.stat().st_size, rotation.STATE_LIMIT)
        before = {p.name: (p.read_bytes(), p.stat().st_mtime_ns) for p in self.state.iterdir()}
        with self.healthy_scheduler(), mock.patch.object(rotation, 'write_private') as write, \
             mock.patch.object(rotation, 'rotation_lock') as lock:
            report = rotation.status(self.state)
            self.assertTrue(report['healthy'])
            self.assertTrue(report['rotation_verified'])
            write.assert_not_called()
            lock.assert_not_called()
        self.assertEqual(before, {p.name: (p.read_bytes(), p.stat().st_mtime_ns) for p in self.state.iterdir()})
        missing = self.base / 'absent'
        with self.assertRaises(FileNotFoundError):
            rotation.status(missing)
        self.assertFalse(missing.exists())

    def test_failure_retains_last_success_and_records_only_typed_reason(self):
        self.run_rotation()
        success = rotation.load_health(self.state)['last_success']
        with self.assertRaises(OSError):
            self.run_rotation(effect=OSError(errno.ENOSPC, 'PRIVATE LOG CONTENT'))
        health = rotation.load_health(self.state)
        self.assertEqual(health['last_success'], success)
        self.assertEqual(health['last_attempt']['outcome'], 'failed')
        self.assertEqual(health['last_attempt']['phase'], 'rotation')
        self.assertEqual(health['last_attempt']['reason'], 'disk_full')
        self.assertNotIn('PRIVATE', (self.state / 'health.json').read_text())
        with self.healthy_scheduler():
            self.assertFalse(rotation.status(self.state)['healthy'])
        # The original retention guard is observable before invoking newsyslog.
        self.logs[0].with_name(self.logs[0].name + '.4.gz').touch(mode=0o600)
        with self.assertRaisesRegex(rotation.RotationError, 'Unexpected archive'):
            self.run_rotation()
        self.assertEqual(rotation.load_health(self.state)['last_attempt']['phase'], 'admission')

    def test_enospc_persisting_health_fails_before_rotation_and_scheduler_exit_exposes_failure(self):
        self.run_rotation()
        previous = (self.state / 'health.json').read_bytes()
        with mock.patch.object(rotation, 'save_health', side_effect=OSError(errno.ENOSPC, 'PRIVATE')), \
             mock.patch.object(rotation, 'command') as command:
            with self.assertRaisesRegex(rotation.RotationError, 'health record persistence failed \\(disk_full\\)'):
                rotation.rotate(self.state)
            command.assert_not_called()
        self.assertEqual((self.state / 'health.json').read_bytes(), previous)
        child = subprocess.CompletedProcess([], 0, b'state = not running\nlast exit code = 1\nPRIVATE\n')
        with mock.patch.object(rotation.subprocess, 'run', return_value=child):
            report = rotation.status(self.state)
        self.assertTrue(report['rotation_verified'])  # The earlier success remains evidence.
        self.assertFalse(report['healthy'])  # It cannot mask the failed scheduled run.
        self.assertEqual(report['scheduler']['last_exit_status'], 1)
        self.assertNotIn('PRIVATE', json.dumps(report))

    def test_health_write_failure_after_rotation_remains_incomplete(self):
        original = rotation.save_health
        count = 0

        def disk_fills(state, health):
            nonlocal count
            count += 1
            if count >= 3:
                raise OSError(errno.ENOSPC, 'full')
            original(state, health)

        with mock.patch.object(rotation, 'save_health', side_effect=disk_fills):
            with self.assertRaisesRegex(rotation.RotationError, 'health record persistence failed'):
                self.run_rotation()
        health = rotation.load_health(self.state)
        self.assertEqual(health['last_attempt']['outcome'], 'running')
        self.assertEqual(health['last_attempt']['phase'], 'rotation')
        self.assertIsNone(health['last_success'])
        with self.healthy_scheduler():
            self.assertFalse(rotation.status(self.state)['healthy'])

    def test_failed_success_publication_preserves_previously_persisted_success(self):
        self.run_rotation()
        previous = rotation.load_health(self.state)['last_success']
        original = rotation.save_health
        publications = 0

        def transient_full_disk(state, health):
            nonlocal publications
            publications += 1
            if publications == 4:
                self.assertEqual(health['last_attempt']['outcome'], 'succeeded')
                raise OSError(errno.ENOSPC, 'full during success publication')
            original(state, health)

        with mock.patch.object(rotation, 'save_health', side_effect=transient_full_disk):
            with self.assertRaises(OSError):
                self.run_rotation()
        self.assertEqual(publications, 5)
        health = rotation.load_health(self.state)
        self.assertEqual(health['last_success'], previous)
        self.assertEqual(health['last_attempt']['outcome'], 'failed')
        self.assertEqual(health['last_attempt']['reason'], 'disk_full')
        with self.healthy_scheduler():
            self.assertFalse(rotation.status(self.state)['healthy'])

    def test_status_rejects_missing_stale_future_running_or_changed_policy_evidence(self):
        with self.healthy_scheduler():
            self.assertFalse(rotation.status(self.state)['healthy'])
        self.run_rotation()
        health = rotation.load_health(self.state)
        finished = health['last_attempt']['finished_at_ns']
        for now in (finished - 1, finished + 601 * 1_000_000_000):
            with self.subTest(now=now), self.healthy_scheduler(), mock.patch.object(rotation.time, 'time_ns', return_value=now):
                self.assertFalse(rotation.status(self.state)['healthy'])
        changed = dict(self.settings, interval_seconds=600)
        rotation.write_private(self.state / 'settings.json', json.dumps(changed).encode())
        with self.healthy_scheduler():
            self.assertFalse(rotation.status(self.state)['healthy'])
        rotation.write_private(self.state / 'settings.json', json.dumps(self.settings).encode())
        health['last_attempt'] = dict(health['last_attempt'], finished_at_ns=None, outcome='running', phase='admission')
        rotation.save_health(self.state, health)
        with self.healthy_scheduler():
            self.assertFalse(rotation.status(self.state)['healthy'])

    def test_health_closed_schema_bounds_and_private_custody(self):
        self.run_rotation()
        path = self.state / 'health.json'
        good = path.read_bytes()
        for content in (b'x' * (rotation.STATE_LIMIT + 1), b'{}',
                        good.replace(b'"complete"', b'"PRIVATE"'),
                        good.replace(b'"succeeded"', b'"running"')):
            with self.subTest(content=content[:20]):
                rotation.write_private(path, content)
                with self.assertRaises((rotation.RotationError, ValueError)), mock.patch.object(rotation, 'command') as command:
                    rotation.rotate(self.state)
                command.assert_not_called()
        rotation.write_private(path, good)
        path.chmod(0o644)
        with self.assertRaisesRegex(rotation.RotationError, 'owner-only'):
            rotation.load_health(self.state)
        path.chmod(0o600)
        other = self.state / 'other.json'
        path.rename(other)
        path.symlink_to(other)
        with self.assertRaisesRegex(rotation.RotationError, 'symlinks'):
            rotation.load_health(self.state)

    def test_private_read_ignores_atime_but_rejects_integrity_changes(self):
        path = self.state / 'settings.json'
        before = path.stat()
        fields = ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid', 'st_nlink', 'st_size',
                  'st_mtime_ns', 'st_ctime_ns', 'st_atime_ns')
        changed = SimpleNamespace(**{field: getattr(before, field) for field in fields})
        changed.st_atime_ns += 1
        with mock.patch.object(rotation.os, 'fstat', side_effect=[before, changed]):
            self.assertEqual(rotation.load_settings(self.state), self.settings)
        changed.st_size += 1
        with mock.patch.object(rotation.os, 'fstat', side_effect=[before, changed]):
            with self.assertRaisesRegex(rotation.RotationError, 'changed during reading'):
                rotation.load_settings(self.state)

    def test_scheduler_status_is_separate_and_has_closed_output(self):
        fixtures = ((0, b'state = not running\nlast exit code = 0\n', 0),
                    (0, b'state = running\nlast terminating signal = 9\n', 137),
                    (0, b'state = waiting\n', None), (1, b'PRIVATE', None))
        for code, output, exit_status in fixtures:
            with self.subTest(code=code, output=output), mock.patch.object(rotation.subprocess, 'run',
                    return_value=subprocess.CompletedProcess([], code, output)) as child:
                report = rotation.scheduler_status()
            self.assertEqual(report['loaded'], code == 0)
            self.assertEqual(report['last_exit_status'], exit_status)
            self.assertNotIn('PRIVATE', json.dumps(report))
            self.assertEqual(child.call_args.kwargs['timeout'], 10)
            self.assertEqual(child.call_args.args[0], ['/bin/launchctl', 'print', f'user/{os.getuid()}/{rotation.LABEL}'])

    def test_install_cannot_bootstrap_or_claim_success_when_rotation_fails(self):
        args = argparse.Namespace(log=self.logs, pid_file=self.pid_file, expected_pid=16392,
                                  interval_seconds=300, count=4, size_kib=32768, hours=24, state_dir=self.state)
        with mock.patch.object(rotation, 'master_identity', return_value=(16392, 'start')), \
             mock.patch.object(rotation.Path, 'home', return_value=self.base), \
             mock.patch.object(rotation, 'rotate_locked', side_effect=rotation.RotationError('rotation failed')) as verify, \
             mock.patch.object(rotation, 'command') as command, mock.patch.object(rotation.subprocess, 'run') as scheduler:
            with self.assertRaisesRegex(rotation.RotationError, 'rotation failed'):
                rotation.install(args)
            verify.assert_called_once_with(self.state, expected_pid=16392)
            command.assert_not_called()
            scheduler.assert_not_called()

    def test_cli_status_is_read_only_and_failed_health_write_exits_nonzero(self):
        self.run_rotation()
        with mock.patch.object(rotation.sys, 'platform', 'darwin'), \
             mock.patch.object(rotation.Path, 'home', return_value=self.base), \
             mock.patch.object(rotation.sys, 'argv', ['rotate.py', 'status', '--state-dir', str(self.state)]), \
             self.healthy_scheduler(), mock.patch('sys.stdout', new_callable=io.StringIO) as output:
            self.assertEqual(rotation.main(), 0)
            self.assertTrue(json.loads(output.getvalue())['healthy'])
        with mock.patch.object(rotation.sys, 'platform', 'darwin'), \
             mock.patch.object(rotation.Path, 'home', return_value=self.base), \
             mock.patch.object(rotation.sys, 'argv', ['rotate.py', 'run', '--state-dir', str(self.state)]), \
             mock.patch.object(rotation, 'save_health', side_effect=OSError(errno.ENOSPC, 'PRIVATE')), \
             mock.patch('sys.stderr', new_callable=io.StringIO) as error:
            self.assertEqual(rotation.main(), 1)
            self.assertIn('health record persistence failed (disk_full)', error.getvalue())
            self.assertNotIn('PRIVATE', error.getvalue())


if __name__ == '__main__':
    unittest.main()
