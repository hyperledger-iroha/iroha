"""Public-only unit rendering and publication checks; no service is started."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import shlex
import stat
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'taira_epoch_supervisor_unit.py'
SPEC = importlib.util.spec_from_file_location('taira_epoch_supervisor_unit', SCRIPT)
unit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(unit)


def fixture():
    generation = unit.STATE_ROOT + '/generations/' + 'a' * 64
    return {
        'schema_version': 1,
        'cli': '/srv/taira/runtime/release-' + 'b' * 40 + '-update-' + 'c' * 32 + '/bin/iroha',
        'admin_config': generation + '/administrator.toml',
        'operator_key': generation + '/http-operator.key',
        'policy': generation + '/policy.json',
        'trust': generation + '/trust.json',
        'custody': generation + '/custody.json',
        'journal_dir': unit.JOURNAL_DIR,
        'timeout_ms': 3600000,
    }


class FixedUnitTests(unittest.TestCase):
    def test_direct_argv_binds_every_explicit_native_input(self):
        spec = fixture()
        expected = [spec['cli'], '--config', spec['admin_config'],
                    '--operator-private-key-file', spec['operator_key'], '--fee-payer', 'authority',
                    'taira', 'epoch-maintenance', 'supervise', '--policy', spec['policy'],
                    '--trust', spec['trust'], '--custody', spec['custody'],
                    '--journal-dir', spec['journal_dir'], '--timeout-ms', '3600000']
        body = unit.render(spec).decode()
        executable = [line.removeprefix('ExecStart=') for line in body.splitlines()
                      if line.startswith('ExecStart=')]
        self.assertEqual(len(executable), 1)
        self.assertEqual(shlex.split(executable[0]), expected)
        self.assertNotIn('ExecStartPre=', body)
        self.assertNotIn('ExecStop=', body)
        self.assertNotIn('Environment', body)
        self.assertNotIn('Requires=iroha3d', body)

    def test_renderer_does_not_open_private_or_public_input_paths(self):
        with patch('builtins.open', side_effect=AssertionError('unexpected file read')), \
             patch.object(os, 'open', side_effect=AssertionError('unexpected descriptor read')):
            self.assertIn(b'Type=exec\n', unit.render(fixture()))

    def test_restart_policy_never_declares_exit_one_success_or_transient(self):
        body = unit.render(fixture()).decode()
        for line in ('Restart=on-failure', 'RestartPreventExitStatus=3 4 7',
                     'StartLimitIntervalSec=300s', 'StartLimitBurst=3', 'RestartSec=5s',
                     'KillMode=control-group', 'TimeoutStopSec=30s'):
            self.assertEqual(body.splitlines().count(line), 1)
        self.assertNotIn('SuccessExitStatus', body)
        self.assertNotIn('RestartForceExitStatus', body)
        self.assertNotIn('reset-failed', body)
        self.assertNotIn('RuntimeDirectory=', body)

    def test_persistent_root_is_the_only_writable_system_path(self):
        body = unit.render(fixture()).decode()
        self.assertIn('ReadWritePaths=' + unit.STATE_ROOT + '\n', body)
        self.assertIn('ProtectSystem=strict\n', body)
        self.assertIn('User=root\nGroup=root\nUMask=0077\n', body)
        self.assertIn('WorkingDirectory=' + unit.STATE_ROOT + '\n', body)

    def test_closed_fields_reject_additions_and_omissions(self):
        for field in unit.SPEC_KEYS:
            with self.subTest(field=field):
                value = fixture(); del value[field]
                with self.assertRaises(unit.UnitPolicyError):
                    unit.render(value)
        for field in ('environment', 'unit_name', 'restart', 'command', 'seed'):
            with self.subTest(field=field):
                value = fixture(); value[field] = 'unadmitted'
                with self.assertRaises(unit.UnitPolicyError):
                    unit.render(value)

    def test_timeout_and_schema_reject_boolean_zero_negative_and_unbounded(self):
        for value in (False, True, 0, -1, 2 ** 64, '3600000', 1.5, None):
            spec = fixture(); spec['timeout_ms'] = value
            with self.subTest(timeout=value), self.assertRaises(unit.UnitPolicyError):
                unit.render(spec)
        for value in (True, 0, 2, '1', None):
            spec = fixture(); spec['schema_version'] = value
            with self.subTest(schema=value), self.assertRaises(unit.UnitPolicyError):
                unit.render(spec)

    def test_path_expansion_and_normalization_reject_before_render(self):
        for value in ('relative/bin/iroha', '/srv//release/bin/iroha', '/srv/./bin/iroha',
                      '/srv/../bin/iroha', '/srv/a b/bin/iroha', '/srv/%n/bin/iroha',
                      '/srv/$HOME/bin/iroha', '/srv/\\x/bin/iroha', '/srv/a\nbin/iroha',
                      '/srv/a\t/bin/iroha', '/srv/a\x00/bin/iroha', '/bin/other', None):
            spec = fixture(); spec['cli'] = value
            with self.subTest(path=value), self.assertRaises(unit.UnitPolicyError):
                unit.render(spec)

    def test_generation_rejects_mixed_identity_and_noncanonical_digest(self):
        for field in unit.GENERATION_FILES:
            spec = fixture(); spec[field] = spec[field].replace('a' * 64, 'd' * 64)
            with self.subTest(field=field), self.assertRaises(unit.UnitPolicyError):
                unit.render(spec)
        for digest in ('A' * 64, 'a' * 63, 'latest', '0' * 65):
            spec = fixture()
            for field in unit.GENERATION_FILES:
                spec[field] = spec[field].replace('a' * 64, digest)
            with self.subTest(digest=digest), self.assertRaises(unit.UnitPolicyError):
                unit.render(spec)

    def test_generation_filenames_and_journal_root_are_fixed(self):
        for field in unit.GENERATION_FILES:
            spec = fixture(); spec[field] += '.old'
            with self.subTest(field=field), self.assertRaises(unit.UnitPolicyError):
                unit.render(spec)
        for path in ('/tmp/journals', unit.JOURNAL_DIR + '/', unit.STATE_ROOT + '/generations/journals'):
            spec = fixture(); spec['journal_dir'] = path
            with self.subTest(path=path), self.assertRaises(unit.UnitPolicyError):
                unit.render(spec)

    def test_renderer_does_not_mutate_input(self):
        value = fixture(); original = copy.deepcopy(value)
        self.assertEqual(unit.render(value), unit.render(value))
        self.assertEqual(value, original)

    def test_argument_quoting_keeps_literal_systemd_expansions(self):
        self.assertEqual(unit.systemd_argument('a%nb$c'), '"a%%nb$$c"')
        for value in ('a\nb', 'a\rb', 'a\x00b', 'a\x7fb'):
            with self.subTest(value=value), self.assertRaises(unit.UnitPolicyError):
                unit.systemd_argument(value)

    def test_bounded_public_spec_and_duplicate_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory).resolve() / 'public.json'
            path.write_text(json.dumps(fixture()))
            self.assertEqual(unit.load_public_spec(path), fixture())
            path.write_text(json.dumps(fixture()).replace('"schema_version": 1', '"schema_version": 1, "schema_version": 1'))
            with self.assertRaisesRegex(unit.UnitPolicyError, 'duplicate'):
                unit.load_public_spec(path)
            for body in ('', 'x' * (16 * 1024 + 1)):
                path.write_text(body)
                with self.assertRaises(unit.UnitPolicyError):
                    unit.load_public_spec(path)

    def test_public_spec_rejects_symlink_and_hardlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); source = root / 'public.json'
            source.write_text(json.dumps(fixture()))
            link = root / 'link.json'; link.symlink_to(source)
            with self.assertRaises(OSError):
                unit.load_public_spec(link)
            hardlink = root / 'hard.json'; os.link(source, hardlink)
            with self.assertRaises(unit.UnitPolicyError):
                unit.load_public_spec(hardlink)

    def test_publish_complete_readable_unit_without_replacing_prior_output(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory).resolve() / unit.UNIT_NAME
            unit.publish(fixture(), target)
            original = target.read_bytes()
            self.assertEqual(original, unit.render(fixture()))
            self.assertEqual(stat.S_IMODE(target.stat().st_mode), 0o644)
            with self.assertRaises(FileExistsError):
                unit.publish(fixture(), target)
            self.assertEqual(target.read_bytes(), original)

    def test_bad_spec_or_output_cannot_publish_a_partial_unit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); target = root / unit.UNIT_NAME
            spec = fixture(); spec['timeout_ms'] = 0
            with self.assertRaises(unit.UnitPolicyError):
                unit.publish(spec, target)
            self.assertFalse(target.exists())
            with self.assertRaises(unit.UnitPolicyError):
                unit.publish(fixture(), root / 'other.service')
            self.assertEqual(list(root.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
