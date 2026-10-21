"""Tiny DATA-only reuse integration controls; no Cargo, proof or network launch."""
from pathlib import Path
from paths import QUAL
import copy
import hashlib
import json
import os
import tempfile
import unittest
from unittest.mock import patch

import run as r
import test_native as native_tests

PIN = 'a' * 64
OTHER = 'b' * 64


class ReuseControllerTests(unittest.TestCase):
    setup_data = native_tests.NativeMigrationTests.setup_data
    catalog_data = native_tests.NativeMigrationTests.catalog_data

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='controller7-data-', dir=QUAL)
        self.assertTrue(Path(self.temp.name).is_relative_to(QUAL))
        self.assertEqual(Path(self.temp.name).stat().st_mode & 0o777, 0o700)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        (self.root / '.git').mkdir()
        checkout = patch.object(r.admission.capture, 'REPO', self.root)
        checkout.start(); self.addCleanup(checkout.stop)
        self.raw = self.root / 'raw' / 'originals'
        self.raw.mkdir(parents=True)
        self.content = b'raw DATA, no receipt or signing authority'
        self.name = hashlib.sha256(self.content).hexdigest()
        self.file = self.raw / self.name
        self.file.write_bytes(self.content)
        self.inventory = {
            'schema': r.native_catalog.REUSE.SCHEMA, 'root': str(self.raw),
            'directory_identity': r.native_catalog.REUSE.directory(self.raw),
            'files': {self.name: {'sha256': self.name,
                                'identity': r.native_catalog.REUSE.identity(self.file.stat())}},
            'scope': 'Synthetic content-addressed DATA only; no genuine source.',
        }
        self.selected = r.ref(r.save(self.root / 'inventory.json', self.inventory))
        self.setup, _, self.originals = self.setup_data()
        self.compiler = {'path': '/never/execute', 'sha256': PIN, 'source_sha256': OTHER}
        self.admitted = {'binaries': {'iroha_core_zk': self.compiler}, 'source': {}, 'retained': {},
                         'raw_reuse': r.native_catalog.REUSE.admit(self.selected)}
        self.calls = []

    def fake_catalog(self, binary, test, directory, env, admitted, tools):
        self.calls.append((binary, test, dict(env)))
        self.catalog_data(directory / 'output')
        output = directory / 'output'
        (output / 'originals').mkdir()
        os.link(self.file, output / 'originals' / self.name)
        (output / 'reuse-inputs.json').write_bytes(Path(env['KAGEMUSHA_WALLET_REUSE_INPUTS']).read_bytes())

    def run_catalog(self, fake=None):
        with patch.object(r, 'exact_test', side_effect=fake or self.fake_catalog), \
                patch.object(r.subprocess, 'Popen', side_effect=AssertionError('no child process permitted')):
            return r.catalog_from_live_setup(self.root, self.setup, self.admitted, {}, {})

    def test_exact_live_genesis_and_current_authority_survive_raw_link_intake(self):
        fixture, catalog, pins, outputs = self.run_catalog()
        self.assertEqual(len(self.calls), 1)
        self.assertEqual(self.calls[0][:2], (self.compiler, r.CATALOG))
        env = self.calls[0][2]
        self.assertEqual(set(env), {'KAGEMUSHA_SIGNED_GENESIS_FIXTURE',
            'KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256', 'KAGEMUSHA_WALLET_SOURCE_SHA256',
            'KAGEMUSHA_WALLET_CATALOG_OUTPUT', 'KAGEMUSHA_WALLET_REUSE_INPUTS',
            'KAGEMUSHA_WALLET_NEW_RAW_BYTES'})
        self.assertEqual(int(env['KAGEMUSHA_WALLET_NEW_RAW_BYTES']),
            r.read_document(self.root / 'catalog-storage-budget.json')['maximum_new_raw_bytes'])
        self.assertEqual(fixture, self.originals['capture.json'])
        self.assertEqual(env['KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256'], fixture['sha256'])
        self.assertEqual(json.loads(Path(env['KAGEMUSHA_WALLET_REUSE_INPUTS']).read_text()), [str(self.raw)])
        self.assertEqual(self.admitted['retained'][env['KAGEMUSHA_WALLET_REUSE_INPUTS']],
                         r.observe(Path(env['KAGEMUSHA_WALLET_REUSE_INPUTS']))['sha256'])
        self.assertEqual(set(pins), set(r.SOURCE_PINS))
        self.assertEqual(len(outputs), 10)
        self.assertEqual(self.file.stat().st_ino, (catalog / 'originals' / self.name).stat().st_ino)
        after = r.read_document(self.root / 'raw-reuse-after.json')
        self.assertTrue(after['accepted'])
        self.assertTrue(after['custody']['content_hashes_verified'])
        self.assertEqual(after['custody']['controlled_subject_hard_links'], 1)
        self.assertEqual(after['custody']['authority'], 'untrusted raw DATA; current Rust descriptor/VK/PK import required')

    def test_reuse_cannot_substitute_genesis_or_historical_grants(self):
        for key in ('genesis', 'signed_catalog', 'source_equivalence', 'historical_runtime_supplement'):
            with self.subTest(key=key), self.assertRaises(ValueError):
                r.compile_selection({'schema': r.SCHEMA, 'captures': {}, 'reuse': self.selected, key: {}})
        with patch.object(r.native_capture, 'admit', side_effect=AssertionError('no source admission')):
            for value in ({}, {'path': str(self.raw)}, self.selected | {'signed_catalog': PIN},
                          self.selected | {'sha256': OTHER}):
                with self.subTest(value=value), self.assertRaises(ValueError):
                    r.compile_selection({'schema': r.SCHEMA,
                        'captures': dict.fromkeys(('stock', 'owner', 'native'), {}), 'reuse': value})

    def test_insufficient_capacity_refuses_before_catalog_and_raw_link_transport(self):
        with patch.object(r.sealed_data, 'capacity', side_effect=ValueError('headroom')):
            with self.assertRaisesRegex(ValueError, 'headroom'):
                self.run_catalog()
        self.assertEqual(self.calls, [])
        self.assertFalse((self.root / 'raw-reuse').exists())
        self.assertFalse((self.root / 'catalog').exists())

    def test_changed_raw_namespace_refuses_before_any_catalog(self):
        (self.raw / ('1' * 64)).write_bytes(b'extra')
        with self.assertRaisesRegex(ValueError, 'raw namespace changed'):
            self.run_catalog()
        self.assertEqual(self.calls, [])
        self.assertFalse((self.root / 'catalog').exists())

    def test_foreign_live_setup_is_not_waived_by_valid_raw_inventory(self):
        Path(self.originals['capture.json']['path']).write_bytes(b'foreign')
        with self.assertRaises(ValueError):
            self.run_catalog()
        self.assertEqual(self.calls, [])
        self.assertFalse((self.root / 'raw-reuse').exists())

    def test_wrong_published_transport_refuses_and_preserves_postcheck(self):
        def wrong(*args):
            self.fake_catalog(*args)
            (args[2] / 'output/reuse-inputs.json').write_text('[]')
        with self.assertRaisesRegex(ValueError, 'native reuse transport differs'):
            self.run_catalog(wrong)
        self.assertTrue(r.read_document(self.root / 'raw-reuse-after.json')['accepted'])
        self.assertFalse((self.root / 'new-anchor-catalog.json').exists())

    def test_failed_subject_still_checks_originals_without_retry(self):
        def fail(*args):
            self.fake_catalog(*args)
            raise ValueError('retained natural test exit101')
        with self.assertRaisesRegex(ValueError, 'retained natural test exit101'):
            self.run_catalog(fail)
        self.assertEqual(len(self.calls), 1)
        self.assertTrue(r.read_document(self.root / 'raw-reuse-after.json')['accepted'])
        self.assertFalse((self.root / 'new-anchor-catalog.json').exists())

    def test_content_hash_is_not_replaced_by_matching_metadata(self):
        self.file.write_bytes(b'wrong bytes')
        self.admitted['raw_reuse']['files'][self.name]['identity'] = r.native_catalog.REUSE.identity(self.file.stat())
        with self.assertRaisesRegex(ValueError, 'raw content-address hash differs'):
            self.run_catalog()
        self.assertFalse(r.read_document(self.root / 'raw-reuse-after.json')['accepted'])
        self.assertFalse((self.root / 'new-anchor-catalog.json').exists())

    def test_extra_unexplained_link_prevents_acceptance(self):
        def changed(*args):
            self.fake_catalog(*args)
            os.link(self.file, self.root / 'unexplained')
        with self.assertRaisesRegex(ValueError, 'unexpected raw hard-link mutation'):
            self.run_catalog(changed)
        self.assertFalse(r.read_document(self.root / 'raw-reuse-after.json')['accepted'])
        self.assertFalse((self.root / 'new-anchor-catalog.json').exists())

    def test_remaining_phase_rechecks_only_the_controlled_link_change(self):
        self.run_catalog()
        self.admitted['binaries'] = {}
        responses = {('branch', '--show-current'): 'optimizations',
                     ('diff', '--name-only', '--diff-filter=U'): ''}
        with patch.object(r.admission.capture, 'git', side_effect=lambda *args: responses[args]):
            r.unchanged(self.admitted, {})
            os.link(self.file, self.root / 'late-extra-link')
            with self.assertRaisesRegex(ValueError, 'unexpected raw hard-link mutation'):
                r.unchanged(self.admitted, {})

    def test_reviewed_external_loader_rejects_foreign_preloaded_module(self):
        import types
        with patch.dict(r.native_catalog.sys.modules, {'raw_reuse': types.SimpleNamespace(__file__=__file__)}):
            with self.assertRaisesRegex(ValueError, 'foreign preloaded raw DATA intake helper'):
                r.native_catalog.load_reuse()


if __name__ == '__main__':
    unittest.main(verbosity=2)
