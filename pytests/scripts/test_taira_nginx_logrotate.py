"""Offline safety and continuity checks; never signal a real nginx process."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import stat
import subprocess
import tempfile
import unittest
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
            self.assertEqual(row.split(), [str(log), f'{os.getuid()}:{log.stat().st_gid}', '600', '4',
                                           '32768', '24', 'BZ', str(self.pid_file), '30'])
        for count, size, hours in ((0, 32768, 24), (11, 32768, 24), (4, 1, 24), (4, 32768, 0)):
            with self.assertRaises(rotation.RotationError):
                rotation.config_bytes(self.logs, self.pid_file, count, size, hours)

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
             mock.patch.object(rotation.Path, 'home', return_value=self.base), \
             mock.patch.object(rotation.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1)) as launchctl, \
             mock.patch.object(rotation, 'command') as command:
            result = rotation.install(args)
            plist = Path(result['launch_agent'])
            command.assert_called_once_with(['/bin/launchctl', 'bootstrap', f'user/{os.getuid()}', str(plist)])
            self.assertEqual(launchctl.call_args.args[0], ['/bin/launchctl', 'print', f'user/{os.getuid()}/{rotation.LABEL}'])
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


if __name__ == '__main__':
    unittest.main()
