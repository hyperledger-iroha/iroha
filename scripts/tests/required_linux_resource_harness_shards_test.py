#!/usr/bin/env python3
"""Offline, synthetic-only controls for the four-VM native aggregate.

Python 3.12 and Git required. No native module, Cargo, network, or real receipt is
executed. Tiny temporary repositories test actual clean-checkout admission; the
synthetic ZIP bytes below are deliberately non-executable test data, not evidence.
Run: python -B -m unittest discover -s scripts/tests
     -p 'required_linux_resource_harness*_test.py' -v
"""
from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock
import warnings
import zipfile

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('required_native_controls', ROOT / 'scripts/tests/required_linux_resource_harness_test.py')
controls = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(controls)
consumer = controls.consumer


def encoded(value):
    return (json.dumps(value, sort_keys=True) + '\n').encode()


def successful_log(selected):
    return (''.join(f'{name} (zk_resource_harness_test.NativeRunTests.{name}) ... ok\n' for name in selected)
            + f'\n----------------------------------------------------------------------\nRan {len(selected)} tests in 1.000s\n\nOK\n').encode()


class ShardAggregateTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.config = controls.synthetic_approval()
        self.originals = {'candidate.json': b'{"synthetic_fixture_only":true}\n',
                          'bridge/' + consumer.LIBRARY: b'not a shared library; synthetic offline control only',
                          'bridge/publisher-source.json': b'{"synthetic_source_fixture_only":true}\n'}
        for name, key in [('candidate.json', 'candidate_sha256'), ('bridge/' + consumer.LIBRARY, 'module_sha256'),
                          ('bridge/publisher-source.json', 'provenance_sha256')]:
            self.config[key] = hashlib.sha256(self.originals[name]).hexdigest()
        self.context = {'schema': 'iroha.linux-resource-harness-inputs.v1', 'repository': self.config['repository'],
                        'run_id': 51, 'run_attempt': 2, 'head_commit': 'a' * 40, 'harness_commit': 'b' * 40,
                        'approval_commit': 'c' * 40, 'approval_sha256': hashlib.sha256(encoded(self.config)).hexdigest(),
                        'approved_pins': self.config, **{key: 'd' * 64 for key in consumer.CONTEXT_FILES}}
        self.shards = consumer.partitions(ROOT)
        self.jobs = {'total_count': 4, 'jobs': [
            {'id': 200 + index, 'name': f'Linux native resource harness shard ({shard})',
             'run_id': 51, 'run_attempt': 2, 'head_sha': 'a' * 40, 'status': 'completed', 'conclusion': 'success',
             'runner_id': 300 + index, 'runner_name': f'synthetic-host-{index}',
             'started_at': '2026-11-07T00:00:00Z', 'completed_at': '2026-11-07T00:01:00Z'}
            for index, shard in enumerate(consumer.SHARDS)]}
        self.run = {'id': 51, 'run_attempt': 2, 'head_sha': 'a' * 40, 'path': '.github/workflows/pr.yml',
                    'repository': {'full_name': self.config['repository']}, 'status': 'in_progress', 'conclusion': None}
        self.producer_artifact = {'id': self.config['artifact_id'], 'name': self.config['artifact_name'], 'expired': False,
                                  'digest': 'sha256:' + self.config['artifact_sha256'], 'workflow_run': {'id': 11}}
        self.producer_run = {'id': 11, 'run_attempt': 2, 'head_sha': self.config['producer_head_commit'],
                            'path': consumer.WORKFLOW, 'repository': {'full_name': self.config['repository']},
                            'status': 'completed', 'conclusion': 'success'}
        self.artifacts = {'total_count': 4, 'artifacts': []}
        self.members = {}
        for index, shard in enumerate(consumer.SHARDS):
            selected = self.shards[shard]
            log = successful_log(selected)
            result = {'schema': 'iroha.linux-resource-harness-shard.v1', 'status': 'pass', 'qualification': 'shard_only',
                      'published': False, 'shard': shard, 'context': self.context, 'approved_pins': self.config,
                      'tests': selected, 'harness_commit': self.context['harness_commit'],
                      'runner_sha256': self.context['runner_sha256'], 'tests_run': len(selected),
                      'failures': 0, 'errors': 0, 'skipped': 0, 'returncode': 0,
                      'log_sha256': hashlib.sha256(log).hexdigest()}
            members = {'native-observer-required/result.json': encoded(result), 'native-observer-required/native-tests.log': log,
                       **{'native-observer-required/' + name: raw for name, raw in self.originals.items()},
                       'native-observer-metadata/approval.json': encoded(self.config),
                       'native-observer-metadata/approval-commit.txt': (self.context['approval_commit'] + '\n').encode(),
                       'native-observer-metadata/artifact.json': encoded(self.producer_artifact),
                       'native-observer-metadata/run.json': encoded(self.producer_run),
                       'native-observer-metadata/perf-policy-after.txt': b'2\n'}
            self.members[shard] = members
            self.artifacts['artifacts'].append({'id': 100 + index, 'name': f'linux-resource-harness-51-2-{shard}',
                                                'expired': False, 'workflow_run': {'id': 51, 'head_sha': 'a' * 40}})
            self.rewrite(shard)

    def rewrite(self, shard, extra=()):
        path = self.root / (shard + '.zip')
        with warnings.catch_warnings(), zipfile.ZipFile(path, 'w') as archive:
            warnings.simplefilter('ignore', UserWarning)
            for name, raw in [*self.members[shard].items(), *extra]:
                archive.writestr(name, raw)
        row = self.artifacts['artifacts'][consumer.SHARDS.index(shard)]
        row.update(digest='sha256:' + consumer.digest(path), size_in_bytes=path.stat().st_size)

    def aggregate(self, artifacts=None, jobs=None, run=None):
        return consumer.aggregate_packets(self.context, self.config, self.shards,
                                          self.artifacts if artifacts is None else artifacts,
                                          self.jobs if jobs is None else jobs, self.run if run is None else run, self.root)

    def test_exact_four_original_synthetic_packets_are_accepted_as_control_only(self):
        with (mock.patch.object(consumer.subprocess, 'run', side_effect=AssertionError('must not execute')),
              mock.patch.object(consumer.subprocess, 'check_output', side_effect=AssertionError('must not execute'))):
            result = self.aggregate()
        self.assertEqual(result['tests_run'], 20)
        self.assertEqual(result['tests'], consumer.test_inventory(ROOT))
        self.assertEqual(len(result['shards']), 4)
        self.assertFalse(result['published'])
        self.assertEqual(result['qualification'], 'all_20_native_methods_across_four_hosts')

    def test_paginated_metadata_is_complete_and_unique(self):
        pages = [{'total_count': 4, 'artifacts': self.artifacts['artifacts'][:2]},
                 {'total_count': 4, 'artifacts': self.artifacts['artifacts'][2:]}]
        self.assertEqual(len(consumer.selected_artifacts(self.context, pages)), 4)
        for bad in [pages[:1], pages + pages, [], {'total_count': 3, 'artifacts': self.artifacts['artifacts']}]:
            with self.subTest(bad=str(bad)[:80]), self.assertRaises(ValueError):
                consumer.selected_artifacts(self.context, bad)

    def test_every_non_success_or_wrong_attempt_job_is_rejected(self):
        for field, value in [('status', 'queued'), ('status', 'in_progress'), ('conclusion', 'failure'),
                             ('conclusion', 'cancelled'), ('conclusion', 'skipped'), ('run_attempt', 1),
                             ('run_id', 52), ('head_sha', 'e' * 40), ('name', 'unknown shard'),
                             ('runner_id', None), ('completed_at', None)]:
            jobs = copy.deepcopy(self.jobs)
            jobs['jobs'][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.aggregate(jobs=jobs)
        for rows in [self.jobs['jobs'][:-1], self.jobs['jobs'] + [self.jobs['jobs'][0]]]:
            with self.subTest(count=len(rows)), self.assertRaises(ValueError):
                self.aggregate(jobs={'total_count': len(rows), 'jobs': rows})

    def test_attempt_one_booleans_never_alias_integer_identifiers(self):
        context, jobs, run = copy.deepcopy(self.context), copy.deepcopy(self.jobs), copy.deepcopy(self.run)
        context['run_attempt'] = run['run_attempt'] = 1
        for row in jobs['jobs']:
            row['run_attempt'] = 1
        consumer.shard_jobs(context, jobs, run)
        for owner in ('run', 'job'):
            bad_run, bad_jobs = copy.deepcopy(run), copy.deepcopy(jobs)
            (bad_run if owner == 'run' else bad_jobs['jobs'][0])['run_attempt'] = True
            with self.subTest(owner=owner), self.assertRaises(ValueError):
                consumer.shard_jobs(context, bad_jobs, bad_run)
        producer = copy.deepcopy(self.producer_run)
        config = copy.deepcopy(self.config)
        config['producer_run_attempt'] = 1
        producer['run_attempt'] = True
        with self.assertRaises(ValueError):
            consumer.metadata(config, self.producer_artifact, producer)
        self.assertFalse(consumer.same_json({'run_attempt': True}, {'run_attempt': 1}))
        self.assertFalse(consumer.same_json({'pins': {'producer_run_attempt': True}},
                                            {'pins': {'producer_run_attempt': 1}}))

    def test_wrong_consumer_run_head_attempt_workflow_and_outcome_are_rejected(self):
        for field, value in [('id', 52), ('run_attempt', 1), ('head_sha', 'e' * 40), ('path', 'other.yml'),
                             ('repository', {'full_name': 'other/repo'}), ('status', 'queued'), ('conclusion', 'failure')]:
            run = copy.deepcopy(self.run)
            run[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.aggregate(run=run)

    def test_wrong_expired_duplicate_missing_or_extra_api_artifact_is_rejected(self):
        for field, value in [('name', 'linux-resource-harness-51-1-native-1'), ('expired', True),
                             ('digest', 'sha256:' + '0' * 64), ('workflow_run', {'id': 52, 'head_sha': 'a' * 40}),
                             ('workflow_run', {'id': 51, 'head_sha': 'e' * 40}), ('size_in_bytes', 1)]:
            artifacts = copy.deepcopy(self.artifacts)
            artifacts['artifacts'][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.aggregate(artifacts=artifacts)
        for rows in [self.artifacts['artifacts'][:-1], self.artifacts['artifacts'] + [self.artifacts['artifacts'][0]]]:
            with self.subTest(count=len(rows)), self.assertRaises(ValueError):
                self.aggregate(artifacts={'total_count': len(rows), 'artifacts': rows})

    def test_zip_bytes_cannot_be_substituted_without_matching_api_digest(self):
        path = self.root / 'native-1.zip'
        with path.open('ab') as handle:
            handle.write(b'substituted')
        with self.assertRaises(ValueError):
            self.aggregate()

    def test_shard_receipt_must_match_selected_context_pins_and_counts(self):
        name = 'native-observer-required/result.json'
        original = self.members['native-1'][name]
        changes = [('schema', 'full_suite'), ('status', 'fail'), ('qualification', 'all20'), ('published', True),
                   ('shard', 'native-2'), ('tests', self.shards['native-2']), ('harness_commit', 'e' * 40),
                   ('runner_sha256', 'e' * 64), ('tests_run', 20), ('failures', 1), ('errors', 1),
                   ('skipped', 1), ('returncode', 1), ('failures', False), ('log_sha256', 'e' * 64),
                   ('context', {**self.context, 'approval_commit': 'e' * 40}),
                   ('approved_pins', {**self.config, 'source_commit': 'e' * 40})]
        for field, value in changes:
            result = json.loads(original)
            result[field] = value
            self.members['native-1'][name] = encoded(result)
            self.rewrite('native-1')
            with self.subTest(field=field, value=str(value)[:30]), self.assertRaises(ValueError):
                self.aggregate()
        self.members['native-1'][name] = original

    def test_raw_log_rejects_skips_missing_duplicate_extra_and_wrong_owner(self):
        logname = 'native-observer-required/native-tests.log'
        resultname = 'native-observer-required/result.json'
        original = self.members['native-1'][logname].decode()
        cases = [original.replace('... ok', "... skipped 'unavailable'", 1),
                 original.replace('\nOK\n', '\nOK (skipped=1)\n'),
                 original.split('\n', 1)[1], original.splitlines(True)[0] + original,
                 'test_extra (zk_resource_harness_test.NativeRunTests.test_extra) ... ok\n' + original,
                 original.replace('NativeRunTests.', 'OtherClass.', 1),
                 original.replace('Ran 3', 'Ran 20'), original + 'Ran 3 tests in 1.0s\n\nOK\n']
        for log in cases:
            self.members['native-1'][logname] = log.encode()
            result = json.loads(self.members['native-1'][resultname])
            result['log_sha256'] = hashlib.sha256(log.encode()).hexdigest()
            self.members['native-1'][resultname] = encoded(result)
            self.rewrite('native-1')
            with self.subTest(log=log[-100:]), self.assertRaises(ValueError):
                self.aggregate()

    def test_original_approval_producer_module_source_candidate_and_perf_are_bound(self):
        names = ['native-observer-metadata/approval.json', 'native-observer-metadata/approval-commit.txt',
                 'native-observer-metadata/artifact.json', 'native-observer-metadata/run.json',
                 'native-observer-required/candidate.json', 'native-observer-required/bridge/publisher-source.json',
                 'native-observer-required/bridge/' + consumer.LIBRARY, 'native-observer-metadata/perf-policy-after.txt']
        for name in names:
            original = self.members['native-1'][name]
            self.members['native-1'][name] = b'{}' if name.endswith('.json') else b'changed'
            self.rewrite('native-1')
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.aggregate()
            self.members['native-1'][name] = original
        self.rewrite('native-1')

    def test_unsafe_duplicate_missing_and_symlink_packet_members_are_rejected(self):
        link = zipfile.ZipInfo('shortcut')
        link.create_system = 3
        link.external_attr = (stat.S_IFLNK | 0o777) << 16
        for extra in [[('../escape', b'x')], [('/absolute', b'x')], [('bad\\path', b'x')], [(link, b'target')],
                      [('native-observer-required/result.json', b'{}')]]:
            self.rewrite('native-1', extra)
            with self.subTest(extra=str(extra)[:70]), self.assertRaises(ValueError):
                self.aggregate()
        del self.members['native-1']['native-observer-required/result.json']
        self.rewrite('native-1')
        with self.assertRaises(ValueError):
            self.aggregate()


class PartitionAndContextTests(unittest.TestCase):
    def test_closed_partition_rejects_unknown_missing_duplicate_or_unreviewed_methods(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ['ci/linux_native_harness_tests.json', 'ci/linux_native_harness_shards.json',
                         'scripts/tests/zk_resource_harness_test.py']:
                (root / name).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, root / name)
            path = root / 'ci/linux_native_harness_shards.json'
            original = json.loads(path.read_text())
            self.assertEqual(sum(map(len, consumer.partitions(root).values())), 20)
            cases = []
            for kind in ('unknown', 'missing', 'duplicate', 'unreviewed', 'empty'):
                value = copy.deepcopy(original)
                if kind == 'unknown': value['shards']['native-5'] = value['shards'].pop('native-4')
                if kind == 'missing': value['shards']['native-1'].pop()
                if kind == 'duplicate': value['shards']['native-1'].append(value['shards']['native-2'][0])
                if kind == 'unreviewed': value['shards']['native-1'][0] = 'test_unreviewed'
                if kind == 'empty': value['shards']['native-1'] = []
                cases.append((kind, value))
            for kind, value in cases:
                path.write_bytes(encoded(value))
                with self.subTest(kind=kind), self.assertRaises(ValueError):
                    consumer.partitions(root)

    def test_actual_git_context_is_immutable_and_bound_to_current_github_attempt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            harness, owner = root / 'harness', root / 'approval'
            config = controls.synthetic_approval()
            for name in consumer.CONTEXT_FILES.values():
                (harness / name).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, harness / name)
            config_path = owner / 'ci/linux_observer_approval.json'
            config_path.parent.mkdir(parents=True)
            config_path.write_bytes(encoded(config))
            def commit(path):
                def git(*args):
                    return subprocess.check_output(['git', '-c', 'core.hooksPath=/dev/null', '-C', str(path), *args], stderr=subprocess.DEVNULL).decode().strip()
                git('init', '-q')
                git('add', '.')
                tree = git('write-tree')
                raw = (f'tree {tree}\n'
                       'author Synthetic Control <synthetic@example.invalid> 1 +0000\n'
                       'committer Synthetic Control <synthetic@example.invalid> 1 +0000\n'
                       '\nsynthetic offline fixture\n').encode()
                object_id = subprocess.check_output(
                    ['git', '-C', str(path), 'hash-object', '-t', 'commit', '-w', '--stdin'], input=raw).decode().strip()
                git('update-ref', 'HEAD', object_id)
                return git('rev-parse', 'HEAD')
            harness_head, approval_head = commit(harness), commit(owner)
            args = SimpleNamespace(harness_root=str(harness), approval_root=str(owner), config=str(config_path),
                                   harness_commit=harness_head, approval_commit=approval_head,
                                   head_commit='e' * 40, run_id='51', run_attempt='2')
            env = {'GITHUB_REPOSITORY': config['repository'], 'GITHUB_RUN_ID': '51',
                   'GITHUB_RUN_ATTEMPT': '2', 'GITHUB_SHA': harness_head}
            with mock.patch.dict(os.environ, env):
                context = consumer.make_context(args, config)
                self.assertEqual(consumer.validate_context(context, config, harness, config_path), consumer.partitions(harness))
                output = root / 'github-output'
                script = harness / 'scripts/run_required_linux_resource_harness.py'
                command = [sys.executable, '-B', str(script), 'inputs', '--config', str(config_path),
                           '--harness-root', str(harness), '--approval-root', str(owner),
                           '--approval-commit', approval_head, '--harness-commit', harness_head,
                           '--head-commit', args.head_commit, '--run-id', args.run_id,
                           '--run-attempt', args.run_attempt, '--github-output', str(output)]
                completed = subprocess.run(command, capture_output=True, text=True)
                self.assertEqual(completed.returncode, 0, completed.stderr)
                outputs = dict(line.split('=', 1) for line in output.read_text().splitlines())
                self.assertEqual(json.loads(outputs['inputs_json']), context)
                context_path = root / 'context.json'
                context_path.write_text(outputs['inputs_json'])
                metadata = root / 'artifacts.json'
                rows = [{'id': 100 + index, 'name': f'linux-resource-harness-51-2-{shard}',
                         'expired': False, 'size_in_bytes': 123, 'digest': 'sha256:' + 'd' * 64,
                         'workflow_run': {'id': 51, 'head_sha': args.head_commit}}
                        for index, shard in enumerate(consumer.SHARDS)]
                metadata.write_bytes(encoded([{'total_count': 4, 'artifacts': rows}]))
                selected_output = root / 'selected-output'
                completed = subprocess.run([sys.executable, '-B', str(script), 'select', '--config', str(config_path),
                                            '--harness-root', str(harness), '--context', str(context_path),
                                            '--artifacts-metadata', str(metadata), '--github-output', str(selected_output)],
                                           capture_output=True, text=True)
                self.assertEqual(completed.returncode, 0, completed.stderr)
                self.assertEqual(selected_output.read_text(), 'artifact_ids=100,101,102,103\n')
                for field, value in [('run_id', 52), ('run_attempt', 1), ('harness_commit', 'e' * 40),
                                     ('approval_commit', 'e' * 40), ('approval_sha256', 'e' * 64),
                                     ('runner_sha256', 'e' * 64), ('tests_sha256', 'e' * 64),
                                     ('partition_sha256', 'e' * 64), ('inventory_sha256', 'e' * 64),
                                     ('consumer_sha256', 'e' * 64), ('head_commit', 'main'), ('run_attempt', True)]:
                    changed = copy.deepcopy(context)
                    changed[field] = value
                    with self.subTest(field=field), self.assertRaises(ValueError):
                        consumer.validate_context(changed, config, harness, config_path)
                (harness / 'untracked').write_text('changes')
                with self.assertRaisesRegex(ValueError, 'dirty'):
                    consumer.validate_context(context, config, harness, config_path)
                (harness / 'untracked').unlink()
                config_path.write_bytes(encoded({**config, 'source_commit': 'e' * 40}))
                with self.assertRaisesRegex(ValueError, 'dirty'):
                    consumer.validate_context(context, config, harness, config_path)


if __name__ == '__main__':
    unittest.main()
