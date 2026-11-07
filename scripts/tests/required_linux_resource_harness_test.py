#!/usr/bin/env python3
"""Offline refusal controls for the required Linux native consumer.

Synthetic hashes and receipts below are test inputs only. They cannot authorize a
production artifact; no bridge is loaded and no native test result is fabricated.
Run with Python 3.12: python -B -m unittest discover -s scripts/tests
-p required_linux_resource_harness_test.py -v
"""
from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import textwrap
import unittest
import warnings
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('required_native', ROOT / 'scripts/run_required_linux_resource_harness.py')
consumer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(consumer)


def synthetic_approval():
    """Construct a structurally complete synthetic record, never a real approval."""
    return {
        'schema': 'iroha.linux-observer-approval.v1', 'status': 'approved',
        'repository': 'test-owner/test-repository', 'producer_commit': '1' * 40,
        'producer_head_commit': '8' * 40,
        'producer_run_id': 11, 'producer_run_attempt': 2, 'artifact_id': 13,
        'artifact_name': 'linux-observer-candidate-11-2', 'artifact_sha256': '2' * 64,
        'candidate_sha256': '3' * 64, 'source_commit': '4' * 40,
        'source_manifest_sha256': '5' * 64, 'native_target': 'x86_64-unknown-linux-gnu',
        'module_sha256': '6' * 64, 'provenance_sha256': '7' * 64,
        'review': {'reviewer': 'test-reviewer',
                   'evidence_url': 'https://github.com/test-owner/test-repository/issues/1',
                   'reviewed_at_utc': '2026-11-07T00:00:00Z'},
    }


class ApprovalTests(unittest.TestCase):
    def test_pending_approval_cannot_run(self):
        pending = synthetic_approval()
        pending['status'] = 'pending-independent-review'
        with self.assertRaisesRegex(ValueError, 'approval'):
            consumer.approval(pending)

    def test_complete_synthetic_shape_is_accepted(self):
        self.assertEqual(consumer.approval(synthetic_approval()), synthetic_approval())

    def test_every_missing_or_placeholder_pin_is_rejected(self):
        config = synthetic_approval()
        for field in consumer.SHA_FIELDS + ('source_commit', 'producer_commit', 'producer_head_commit'):
            for value in (None, '', 'HEAD', '0' * len(config[field]), config[field].upper() + 'A'):
                with self.subTest(field=field, value=value):
                    changed = copy.deepcopy(config)
                    changed[field] = value
                    with self.assertRaises(ValueError):
                        consumer.approval(changed)

    def test_ids_cannot_be_boolean_or_names(self):
        for field in ('artifact_id', 'producer_run_id', 'producer_run_attempt'):
            for value in (True, 0, -1, 'latest'):
                with self.subTest(field=field, value=value):
                    changed = synthetic_approval()
                    changed[field] = value
                    with self.assertRaises(ValueError):
                        consumer.approval(changed)

    def test_review_and_name_cannot_be_omitted_or_substituted(self):
        for field, value in [('review', None), ('artifact_name', 'latest'), ('native_target', 'aarch64-unknown-linux-gnu')]:
            changed = synthetic_approval()
            changed[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                consumer.approval(changed)

    def test_duplicate_json_fields_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'duplicate.json'
            path.write_text('{"status":"pending","status":"approved"}')
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                consumer.read_json(path)

    def test_oversized_control_document_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'large.json'
            path.write_bytes(b' ' * (1024 * 1024 + 1))
            with self.assertRaisesRegex(ValueError, '1 MiB'):
                consumer.read_json(path)


class ProducerMetadataTests(unittest.TestCase):
    def setUp(self):
        self.config = synthetic_approval()
        self.artifact = {'id': 13, 'name': self.config['artifact_name'], 'expired': False,
                         'digest': 'sha256:' + self.config['artifact_sha256'], 'workflow_run': {'id': 11}}
        self.run = {'id': 11, 'run_attempt': 2, 'head_sha': self.config['producer_head_commit'],
                    'path': consumer.WORKFLOW, 'repository': {'full_name': self.config['repository']},
                    'status': 'completed', 'conclusion': 'success'}

    def test_exact_successful_attempt_is_accepted(self):
        consumer.metadata(self.config, self.artifact, self.run)

    def test_api_head_and_actual_merge_checkout_have_independent_pins(self):
        self.assertNotEqual(self.config['producer_head_commit'], self.config['producer_commit'])
        consumer.metadata(self.config, self.artifact, self.run)
        self.run['head_sha'] = self.config['producer_commit']
        with self.assertRaises(ValueError):
            consumer.metadata(self.config, self.artifact, self.run)

    def test_artifact_substitution_and_expiry_are_rejected(self):
        for field, value in [('id', 14), ('name', 'latest'), ('expired', True), ('digest', 'sha256:' + 'a' * 64), ('workflow_run', {'id': 12})]:
            artifact = copy.deepcopy(self.artifact)
            artifact[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                consumer.metadata(self.config, artifact, self.run)

    def test_producer_commit_attempt_and_outcome_are_bound(self):
        for field, value in [('id', 12), ('run_attempt', 3), ('head_sha', 'a' * 40),
                             ('path', '.github/workflows/other.yml'), ('repository', {'full_name': 'other/repo'}),
                             ('status', 'in_progress'), ('conclusion', 'failure')]:
            run = copy.deepcopy(self.run)
            run[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                consumer.metadata(self.config, self.artifact, run)


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.config = synthetic_approval()
        self.module = b'synthetic non-executable module; unit tests never load it'
        self.provenance = b'{"synthetic_fixture_only":true}\n'
        self.config['module_sha256'] = hashlib.sha256(self.module).hexdigest()
        self.config['provenance_sha256'] = hashlib.sha256(self.provenance).hexdigest()
        self.candidate = {
            'schema': 'iroha.linux-observer-candidate.v1', 'status': 'requires_independent_approval',
            'bridge_executed': False, 'observer_commit': self.config['source_commit'],
            'driver_revision': self.config['producer_commit'],
            **{name: self.config[name] for name in ('native_target', 'module_sha256', 'provenance_sha256', 'source_manifest_sha256')},
        }

    def make_archive(self, extra=(), omit=None):
        raw = json.dumps(self.candidate).encode()
        self.config['candidate_sha256'] = hashlib.sha256(raw).hexdigest()
        entries = [('review/candidate.json', raw), ('review/bridge/' + consumer.LIBRARY, self.module),
                   ('review/bridge/publisher-source.json', self.provenance)]
        archive = self.root / 'artifact.zip'
        with warnings.catch_warnings(), zipfile.ZipFile(archive, 'w') as packet:
            warnings.simplefilter('ignore', UserWarning)
            for name, content in entries + list(extra):
                if name != omit:
                    packet.writestr(name, content)
        self.config['artifact_sha256'] = consumer.digest(archive)
        output = self.root / 'output'
        output.mkdir()
        return archive, output

    def test_only_original_bridge_files_are_extracted(self):
        archive, output = self.make_archive(extra=[('logs/build.log', b'review only')])
        bridge = consumer.unpack(self.config, archive, output)
        self.assertEqual(sorted(path.name for path in bridge.iterdir()), [consumer.LIBRARY, 'publisher-source.json'])
        self.assertEqual((bridge / consumer.LIBRARY).read_bytes(), self.module)
        self.assertEqual((bridge / 'publisher-source.json').read_bytes(), self.provenance)
        self.assertFalse((output / 'logs').exists())

    def test_changed_archive_is_rejected_before_extraction(self):
        archive, output = self.make_archive()
        with archive.open('ab') as handle:
            handle.write(b'substitution')
        with self.assertRaisesRegex(ValueError, 'independent digest'):
            consumer.unpack(self.config, archive, output)
        self.assertFalse((output / 'bridge').exists())

    def test_missing_original_is_rejected(self):
        archive, output = self.make_archive(omit='review/bridge/' + consumer.LIBRARY)
        with self.assertRaisesRegex(ValueError, 'missing'):
            consumer.unpack(self.config, archive, output)

    def test_duplicate_archive_members_are_rejected(self):
        archive, output = self.make_archive(extra=[('review/candidate.json', b'{}')])
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            consumer.unpack(self.config, archive, output)

    def test_traversal_member_is_rejected(self):
        archive, output = self.make_archive(extra=[('../escape', b'x')])
        with self.assertRaisesRegex(ValueError, 'unsafe'):
            consumer.unpack(self.config, archive, output)
        self.assertFalse((self.root / 'escape').exists())

    def test_symlink_member_is_rejected(self):
        entry = zipfile.ZipInfo('review/shortcut')
        entry.create_system = 3
        entry.external_attr = (stat.S_IFLNK | 0o777) << 16
        archive, output = self.make_archive(extra=[(entry, b'bridge/' + consumer.LIBRARY.encode())])
        with self.assertRaisesRegex(ValueError, 'unsafe'):
            consumer.unpack(self.config, archive, output)

    def test_mismatched_original_digest_is_rejected(self):
        archive, output = self.make_archive()
        self.config['module_sha256'] = 'a' * 64
        with self.assertRaisesRegex(ValueError, 'digest mismatch'):
            consumer.unpack(self.config, archive, output)

    def test_candidate_cannot_substitute_approved_source(self):
        self.candidate['observer_commit'] = 'a' * 40
        archive, output = self.make_archive()
        with self.assertRaisesRegex(ValueError, 'receipt differs'):
            consumer.unpack(self.config, archive, output)

    def test_candidate_cannot_have_executed_before_review(self):
        self.candidate['bridge_executed'] = True
        archive, output = self.make_archive()
        with self.assertRaisesRegex(ValueError, 'unexecuted'):
            consumer.unpack(self.config, archive, output)


class CompleteSuiteTests(unittest.TestCase):
    def setUp(self):
        self.names = consumer.read_json(ROOT / 'ci/linux_native_harness_tests.json')
        self.text = ''.join(f'{name} (zk_resource_harness_test.NativeRunTests.{name}) ... ok\n' for name in self.names)
        self.text += '\n----------------------------------------------------------------------\nRan 20 tests in 1.000s\n\nOK\n'

    def test_all_twenty_explicit_results_are_required(self):
        consumer.successful_suite(0, self.text, self.names)
        for text in [self.text.replace('Ran 20', 'Ran 19'), self.text.replace('... ok', "... skipped 'unavailable'", 1),
                     self.text.replace('\nOK\n', '\nOK (skipped=1)\n'), self.text.split('\n', 1)[1]]:
            with self.subTest(text=text[-100:]), self.assertRaises(ValueError):
                consumer.successful_suite(0, text, self.names)

    def test_nonzero_runner_exit_cannot_pass(self):
        with self.assertRaises(ValueError):
            consumer.successful_suite(1, self.text, self.names)

    def test_inventory_refuses_missing_or_new_methods(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'ci').mkdir()
            (root / 'scripts/tests').mkdir(parents=True)
            (root / 'ci/linux_native_harness_tests.json').write_text(json.dumps(self.names))
            script = root / 'scripts/tests/zk_resource_harness_test.py'
            for names in [self.names, self.names[:-1], self.names + ['test_new_unreviewed_method']]:
                script.write_text('class NativeRunTests:\n' + ''.join(f'    def {name}(self): pass\n' for name in names))
                if names == self.names:
                    self.assertEqual(consumer.test_inventory(root), self.names)
                else:
                    with self.assertRaises(ValueError):
                        consumer.test_inventory(root)


class RequiredWorkflowTests(unittest.TestCase):
    def test_perf_setup_is_hosted_nonroot_and_requires_successful_exact_readback(self):
        workflow = (ROOT / '.github/workflows/pr.yml').read_text()
        step = workflow.split('      - name: Permit required user-space perf events on the disposable hosted VM\n', 1)[1]
        shell = textwrap.dedent(step.split('        run: |\n', 1)[1].split('      - name:', 1)[0])
        job = workflow.split('  linux_resource_harness_shards:\n', 1)[1].split('\n  linux_resource_harness:\n', 1)[0]
        self.assertEqual([line.strip() for line in job.splitlines() if 'sudo ' in line],
                         ['sudo -n sysctl -w kernel.perf_event_paranoid=2 \\'])
        controls = r'''
        id() {
          if [[ "${1-}" == -u ]]; then printf '%s\n' "$FAKE_UID";
          else printf 'uid=%s(control)\n' "$FAKE_UID"; fi
        }
        uname() { printf 'Linux offline-control\n'; }
        sysctl() {
          [[ "$*" == '-n kernel.perf_event_paranoid' ]] || return 91
          cat "$RUNNER_TEMP/fake-policy"
        }
        sudo() {
          [[ "$*" == '-n sysctl -w kernel.perf_event_paranoid=2' ]] || return 92
          printf '%s\n' "$*" >> "$RUNNER_TEMP/fake-sudo-calls"
          if [[ "$CONTROL" == denied ]]; then printf 'fixture denied\n' >&2; return 93; fi
          if [[ "$CONTROL" == readback ]]; then printf '3\n' > "$RUNNER_TEMP/fake-policy";
          else printf '2\n' > "$RUNNER_TEMP/fake-policy"; fi
          printf 'kernel.perf_event_paranoid = 2\n'
        }
        '''
        for control in ('success', 'denied', 'readback', 'self-hosted', 'root'):
            with self.subTest(control=control), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                evidence = root / 'native-observer-metadata'
                evidence.mkdir()
                (root / 'fake-policy').write_text('4\n')
                environment = {'PATH': os.environ['PATH'], 'RUNNER_TEMP': directory, 'CONTROL': control,
                               'FAKE_UID': '0' if control == 'root' else '1001',
                               'RUNNER_ENVIRONMENT': 'self-hosted' if control == 'self-hosted' else 'github-hosted'}
                result = subprocess.run(['bash', '-c', textwrap.dedent(controls) + shell],
                                        env=environment, capture_output=True)
                self.assertEqual(result.returncode == 0, control == 'success')
                if control in ('root', 'self-hosted'):
                    self.assertFalse((root / 'fake-sudo-calls').exists())
                    self.assertEqual((root / 'fake-policy').read_text(), '4\n')
                else:
                    self.assertEqual((evidence / 'perf-policy-before.txt').read_text(), '4\n')
                    self.assertEqual((evidence / 'kernel.txt').read_text(), 'Linux offline-control\n')
                    self.assertEqual((root / 'fake-sudo-calls').read_text(),
                                     '-n sysctl -w kernel.perf_event_paranoid=2\n')
                    if control == 'denied':
                        self.assertIn('fixture denied', (evidence / 'perf-policy-setup.stderr').read_text())
                        self.assertFalse((evidence / 'perf-policy-after.txt').exists())
                    else:
                        self.assertEqual((evidence / 'perf-policy-after.txt').read_text(),
                                         '3\n' if control == 'readback' else '2\n')

    def test_native_job_has_no_skip_gate_and_deselection_is_scoped(self):
        workflow = (ROOT / '.github/workflows/pr.yml').read_text()
        job = workflow.split('  linux_resource_harness_shards:\n', 1)[1].split('\n  linux_resource_harness:\n', 1)[0]
        self.assertNotRegex(job, r'(?m)^    (if|continue-on-error):')
        self.assertIn('Run assigned actual native methods with zero skips', job)
        self.assertIn('shard: [native-1, native-2, native-3, native-4]', job)
        aggregate = workflow.split('  linux_resource_harness:\n', 1)[1].split('\n  rust_required:\n', 1)[0]
        self.assertIn('if: always()', aggregate)
        self.assertIn('Require all four same-attempt shards and exact 20 passing native methods', aggregate)
        self.assertEqual(workflow.count('--deselect=scripts/tests/zk_resource_harness_test.py::NativeRunTests'), 1)
        self.assertIn('skip-decompress: true', job)
        self.assertIn('digest-mismatch: error', job)
        self.assertIn('path: observer-source', job)
        self.assertIn('path: harness', job)
        self.assertIn('path: observer-approval', job)

    def test_aggregate_rejects_every_non_success_native_outcome(self):
        workflow = (ROOT / '.github/workflows/pr.yml').read_text()
        job = workflow.split('  rust_required:\n', 1)[1].split('\n  pytests:\n', 1)[0]
        self.assertRegex(job, r'needs: \[[^\n]*linux_resource_harness\]')
        shell = textwrap.dedent(job.split('        run: |\n', 1)[1])
        base = {'PATH': os.environ['PATH'], 'CLASSIFIER_RESULT': 'success',
                'HELPER_CONTROLS_RESULT': 'success', 'SOURCE_GUARDS_RESULT': 'success'}
        for name in ('BINARY_FREE', 'NETWORK', 'BUILD', 'CONSISTENCY', 'KOTODAMA', 'PYTESTS', 'PARLIAMENT'):
            base[name + '_SELECTED'] = 'false'
            base[name + '_RESULT'] = 'skipped'
        for outcome in ('success', 'failure', 'cancelled', 'skipped', ''):
            with self.subTest(outcome=outcome):
                completed = subprocess.run(['bash', '-c', shell], env={**base, 'NATIVE_HARNESS_RESULT': outcome}, capture_output=True)
                self.assertEqual(completed.returncode == 0, outcome == 'success')


if __name__ == '__main__':
    unittest.main()
