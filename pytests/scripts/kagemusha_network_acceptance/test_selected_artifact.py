"""Exact selected Cargo artifact joins using only inert metadata and file bytes."""
from pathlib import Path
import copy
import os
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import compiler_capture as c
from paths import QUAL


class SelectedArtifactTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=QUAL)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        root_patch = patch.object(c, 'ROOT', self.root)
        root_patch.start(); self.addCleanup(root_patch.stop)
        for owner, names in (
            (os, ('fork', 'forkpty', 'posix_spawn', 'posix_spawnp', 'execve', 'execv', 'execvp',
                  'execvpe', 'execl', 'execle', 'execlp', 'execlpe', 'system', 'kill', 'killpg')),
            (subprocess, ('Popen', 'run', 'call', 'check_call', 'check_output', '_fork_exec')),
        ):
            for name in names:
                if hasattr(owner, name):
                    guard = patch.object(owner, name, side_effect=AssertionError('actual child or signal forbidden'))
                    guard.start(); self.addCleanup(guard.stop)
        self.meta = {'version': 1, 'workspace_root': str(self.root), 'packages': [],
                     'workspace_members': [], 'resolve': {'nodes': []}}
        self.events = {}
        contracts = {
            'iroha_core_zk': ('native', 'iroha_core_zk', 'crates/iroha_core_zk', ['lib'], ['lib'], 'src/lib.rs'),
            'sumeragi': ('network', 'integration_tests', 'integration_tests', ['test'], ['bin'], 'tests/sumeragi.rs'),
            'iroha3d': ('nodes', 'irohad', 'crates/irohad/bins', ['bin'], ['bin'], 'src/bin/iroha3d.rs'),
            'iroha': ('nodes', 'iroha_cli', 'crates/iroha_cli/bins', ['bin'], ['bin'], 'src/bin/iroha.rs'),
        }
        for name, (role, package_name, relative, kind, crate_types, source) in contracts.items():
            package_root = self.root / relative
            manifest = self.file(relative + '/Cargo.toml')
            source_path = self.file(relative + '/' + source)
            binary = self.file('target/fixture/release/' + name)
            identity = 'path+file://' + str(package_root) + '#' + package_name + '@1.0.0'
            target = {'name': name, 'kind': kind, 'crate_types': crate_types, 'src_path': str(source_path),
                      'edition': '2024', 'doc': True, 'doctest': False, 'test': True}
            package = {'id': identity, 'name': package_name, 'version': '1.0.0', 'source': None,
                       'manifest_path': str(manifest), 'targets': [target],
                       'features': {'default': ['enabled', 'dep:external', 'external?/feature'],
                                    'enabled': ['nested'], 'nested': ['enabled'], 'unselected': []}}
            self.meta['packages'].append(package)
            self.meta['workspace_members'].append(identity)
            self.meta['resolve']['nodes'].append({'id': identity, 'dependencies': [], 'deps': [],
                                                  'features': ['default', 'enabled', 'nested']})
            self.events[name] = {
                'reason': 'compiler-artifact', 'package_id': identity, 'manifest_path': str(manifest),
                'target': copy.deepcopy(target), 'profile': {'test': role != 'nodes', 'opt_level': '3',
                                                           'debug_assertions': False},
                'features': ['nested', 'default', 'enabled'], 'filenames': [str(binary)], 'executable': str(binary),
            }

    def file(self, name, raw=b'inert DATA'):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return path

    def config(self, name, profile='release'):
        role = 'native' if name == 'iroha_core_zk' else 'network' if name == 'sumeragi' else 'nodes'
        return {'role': role, 'profile': profile, 'slot': 'fixture', 'jobs': 1}

    def check(self, name, event=None, meta=None, profile='release'):
        return c.selected_artifact(self.events[name] if event is None else event,
                                   self.meta if meta is None else meta, self.config(name, profile))

    def test_all_four_selected_executable_contracts_pass_and_replay_exactly(self):
        for name in self.events:
            self.assertEqual(self.check(name), Path(self.events[name]['executable']))
        for names in (['iroha_core_zk'], ['sumeragi'], ['iroha3d', 'iroha']):
            destination = self.root / ('copy-' + names[0]); destination.mkdir()
            events = [self.events[name] for name in names]
            actual = c.selected_binaries(events, self.meta, self.config(names[0]), destination, create=True)
            self.assertEqual(set(actual), set(names))
            self.assertEqual(actual, c.selected_binaries(events, self.meta, self.config(names[0]), destination, create=False))
            for name, row in actual.items():
                self.assertEqual(Path(row['path']).read_bytes(), Path(row['original']).read_bytes())
                self.assertEqual(Path(row['path']).stat().st_mode & 0o777, 0o555)
                self.assertEqual(row['original'], self.events[name]['executable'])

    def test_executable_must_be_in_typed_unique_emitted_filenames(self):
        for name in self.events:
            for filenames in (None, [], ['foreign'], [self.events[name]['executable'], 1],
                              [self.events[name]['executable']] * 2):
                changed = copy.deepcopy(self.events[name]); changed['filenames'] = filenames
                with self.subTest(name=name, filenames=filenames), self.assertRaises(ValueError):
                    self.check(name, changed)

    def test_release_requires_actual_optimized_profile_for_every_executable(self):
        for name in self.events:
            for field, value in (('opt_level', '0'), ('opt_level', '2'), ('opt_level', 3),
                                 ('debug_assertions', True), ('debug_assertions', 0),
                                 ('test', not self.events[name]['profile']['test']), ('test', 1)):
                changed = copy.deepcopy(self.events[name]); changed['profile'][field] = value
                with self.subTest(name=name, field=field, value=value), self.assertRaises(ValueError):
                    self.check(name, changed)

    def test_nodes_debug_preserves_actual_unoptimized_profile_without_release_claim(self):
        events = []
        for name in ('iroha3d', 'iroha'):
            changed = copy.deepcopy(self.events[name])
            changed['profile'].update(opt_level='0', debug_assertions=True)
            self.assertEqual(self.check(name, changed, profile='debug'), Path(changed['executable']))
            events.append(changed)
        destination = self.root / 'debug-nodes'; destination.mkdir()
        config = self.config('iroha3d', 'debug')
        result = c.selected_binaries(events, self.meta, config, destination, create=True)
        self.assertEqual(result, c.selected_binaries(events, self.meta, config, destination, create=False))
        for name in ('sumeragi', 'iroha_core_zk'):
            with self.assertRaises(ValueError): self.check(name, profile='debug')

    def test_features_require_exact_typed_unique_transitive_defaults(self):
        for name in self.events:
            for features in (None, ['default', 'enabled'], ['nested', 'enabled'],
                             ['default', 'enabled', 'nested', 'unselected'],
                             ['default', 'enabled', 'nested', 'nested'], ['default', 'enabled', 1]):
                changed = copy.deepcopy(self.events[name]); changed['features'] = features
                with self.subTest(name=name, features=features), self.assertRaises(ValueError):
                    self.check(name, changed)

    def test_default_feature_expansion_follows_local_cycles_not_dependency_edges(self):
        package = self.meta['packages'][0]
        self.assertEqual(c.default_features(package), {'default', 'enabled', 'nested'})
        for features in (None, {}, {'default': None}, {'default': [1]}, {'default': ['unknown']},
                         {'default': [], 1: []}):
            changed = copy.deepcopy(package); changed['features'] = features
            with self.subTest(features=features), self.assertRaises(ValueError): c.default_features(changed)

    def test_nonshipping_defaults_still_refuse_before_copy(self):
        for feature in ('sim', 'test-network-extra', 'fault-extra', 'dev-tools', 'cli_integration_harness'):
            meta = copy.deepcopy(self.meta)
            meta['packages'][0]['features']['default'].append(feature)
            meta['packages'][0]['features'][feature] = []
            changed = copy.deepcopy(self.events['iroha_core_zk']); changed['features'].append(feature)
            with self.subTest(feature=feature), self.assertRaisesRegex(ValueError, 'nonshipping'):
                self.check('iroha_core_zk', changed, meta)

    def test_exact_package_id_manifest_source_and_role_are_joined(self):
        for name in self.events:
            for field, value in (('package_id', self.events[name]['package_id'] + '-foreign'),
                                 ('manifest_path', str(self.root / 'Cargo.toml'))):
                changed = copy.deepcopy(self.events[name]); changed[field] = value
                with self.subTest(name=name, field=field), self.assertRaises(ValueError): self.check(name, changed)
        with self.assertRaises(ValueError):
            c.selected_artifact(self.events['sumeragi'], self.meta, self.config('iroha_core_zk'))
        for field, value in (('name', 'foreign'), ('source', 'registry+foreign'), ('id', 'foreign')):
            meta = copy.deepcopy(self.meta); meta['packages'][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): self.check('iroha_core_zk', meta=meta)

    def test_fixed_target_source_kind_crate_types_and_metadata_join_refuse(self):
        for name in self.events:
            for field, value in (('src_path', str(self.root / 'foreign.rs')), ('kind', ['example']),
                                 ('crate_types', ['cdylib']), ('test', False), ('edition', '2015')):
                changed = copy.deepcopy(self.events[name]); changed['target'][field] = value
                with self.subTest(name=name, field=field), self.assertRaises(ValueError): self.check(name, changed)
            for field, value in (('src_path', str(self.root / 'foreign.rs')), ('kind', ['example']),
                                 ('crate_types', ['cdylib']), ('test', False)):
                meta = copy.deepcopy(self.meta)
                package = next(row for row in meta['packages'] if row['id'] == self.events[name]['package_id'])
                package['targets'][0][field] = value
                changed = copy.deepcopy(self.events[name]); changed['target'] = copy.deepcopy(package['targets'][0])
                with self.subTest(name=name, coherent_foreign=field), self.assertRaises(ValueError):
                    self.check(name, changed, meta)

    def test_missing_duplicate_or_incomplete_prospective_metadata_refuses(self):
        for field in ('version', 'workspace_root', 'packages', 'workspace_members', 'resolve'):
            meta = copy.deepcopy(self.meta); del meta[field]
            with self.subTest(missing=field), self.assertRaises(ValueError): self.check('iroha_core_zk', meta=meta)
        for change in ('duplicate-package', 'duplicate-manifest', 'missing-package', 'duplicate-target',
                       'missing-target', 'missing-source', 'missing-resolve', 'duplicate-resolve', 'foreign-workspace'):
            meta = copy.deepcopy(self.meta)
            if change == 'duplicate-package': meta['packages'].append(copy.deepcopy(meta['packages'][0]))
            if change == 'duplicate-manifest':
                other = copy.deepcopy(meta['packages'][0]); other['id'] += '-other'; meta['packages'].append(other)
            if change == 'missing-package': meta['packages'].pop(0)
            if change == 'duplicate-target': meta['packages'][0]['targets'] *= 2
            if change == 'missing-target': meta['packages'][0]['targets'] = []
            if change == 'missing-source': del meta['packages'][0]['source']
            if change == 'missing-resolve': meta['resolve']['nodes'].pop(0)
            if change == 'duplicate-resolve': meta['resolve']['nodes'].append(copy.deepcopy(meta['resolve']['nodes'][0]))
            if change == 'foreign-workspace': meta['workspace_root'] += '-foreign'
            with self.subTest(change=change), self.assertRaises(ValueError): self.check('iroha_core_zk', meta=meta)

    def test_executable_path_must_be_unaliased_and_under_current_target(self):
        outside = self.file('outside-executable')
        alias = self.root / 'target/alias'; alias.symlink_to(Path(self.events['iroha_core_zk']['executable']))
        for path in (str(outside), str(alias), 'relative', str(self.root / 'target/missing')):
            changed = copy.deepcopy(self.events['iroha_core_zk'])
            changed.update(executable=path, filenames=[path])
            with self.subTest(path=path), self.assertRaises((ValueError, OSError)): self.check('iroha_core_zk', changed)

    def test_all_selected_artifacts_are_guarded_before_any_copy(self):
        destination = self.root / 'refused-copy'; destination.mkdir()
        for events in (
            [self.events['iroha3d']],
            [self.events['iroha3d'], self.events['iroha3d'], self.events['iroha']],
            [self.events['iroha3d'], self.events['iroha'] | {'features': ['default']}],
        ):
            with self.assertRaises(ValueError):
                c.selected_binaries(events, self.meta, self.config('iroha3d'), destination, create=True)
            self.assertEqual(list(destination.iterdir()), [])

    def test_capture_and_replay_use_same_guard_and_explicit_metadata(self):
        destination = self.root / 'both-phases'; destination.mkdir()
        event = self.events['iroha_core_zk']
        for create in (True, False):
            with patch.object(c, 'selected_artifact', side_effect=ValueError('inert guard refusal')) as guard:
                with self.assertRaisesRegex(ValueError, 'inert guard refusal'):
                    c.selected_binaries([event], self.meta, self.config('iroha_core_zk'), destination, create=create)
                guard.assert_called_once_with(event, self.meta, self.config('iroha_core_zk'))
            self.assertEqual(list(destination.iterdir()), [])
        with self.assertRaises(TypeError):
            c.selected_binaries([event], self.config('iroha_core_zk'), destination, create=True)

    def test_replay_refuses_changed_retained_binary_and_mutated_cargo_facts(self):
        destination = self.root / 'replay'; destination.mkdir()
        event = self.events['iroha_core_zk']; config = self.config('iroha_core_zk')
        result = c.selected_binaries([event], self.meta, config, destination, create=True)
        changed = copy.deepcopy(event); changed['profile']['debug_assertions'] = True
        with self.assertRaises(ValueError): c.selected_binaries([changed], self.meta, config, destination, create=False)
        copied = Path(result['iroha_core_zk']['path']); copied.chmod(0o600); copied.write_bytes(b'substitution')
        with self.assertRaises(ValueError): c.selected_binaries([event], self.meta, config, destination, create=False)

    def test_malformed_actual_artifact_facts_refuse_explicitly(self):
        destination = self.root / 'malformed'; destination.mkdir()
        for events in ([None], [self.events['iroha_core_zk'] | {'target': None}]):
            with self.assertRaises(ValueError):
                c.selected_binaries(events, self.meta, self.config('iroha_core_zk'), destination, create=True)
        for field in ('target', 'profile', 'features', 'filenames'):
            changed = copy.deepcopy(self.events['iroha_core_zk']); changed[field] = None
            with self.subTest(field=field), self.assertRaises(ValueError): self.check('iroha_core_zk', changed)


if __name__ == '__main__':
    unittest.main()
