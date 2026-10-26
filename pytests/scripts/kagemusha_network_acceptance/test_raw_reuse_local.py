"""Pure local DATA helper admission controls; no native command or catalog proof."""
from pathlib import Path
from paths import QUAL
import hashlib
import importlib.util
import os
import py_compile
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

import native_catalog as catalog
import paths


class LocalRawReuseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='raw-local-', dir=QUAL)
        self.assertTrue(Path(self.temp.name).is_relative_to(QUAL))
        self.assertEqual(Path(self.temp.name).stat().st_mode & 0o777, 0o700)
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name).resolve()
        for name in ('raw_reuse.py', 'raw_reuse_policy.py'):
            (self.directory / name).write_bytes((catalog.REUSE_ROOT / name).read_bytes())

    def load(self):
        with patch.object(catalog, 'REUSE_ROOT', self.directory), patch.dict(sys.modules):
            for name in ('raw_reuse', 'raw_reuse_policy'):
                sys.modules.pop(name, None)
            return catalog.load_reuse()

    def test_local_exact_sources_supply_only_data_authority(self):
        loaded = self.load()
        self.assertEqual(Path(loaded.__file__), self.directory / 'raw_reuse.py')
        self.assertEqual(loaded.MAX_FILES, 12288)
        self.assertEqual(loaded.MAX_FILE_BYTES, 1 << 30)
        self.assertEqual(loaded.MAX_POOL_BYTES, 128 << 30)
        self.assertNotIn('accepted_marker', loaded.__dict__)
        self.assertNotIn('completed_outputs', loaded.__dict__)

    def test_changed_either_helper_is_refused_before_execution(self):
        for name in ('raw_reuse.py', 'raw_reuse_policy.py'):
            with self.subTest(name=name):
                path = self.directory / name
                original = path.read_bytes()
                path.write_bytes(original + b'\nraise AssertionError("unreviewed source executed")\n')
                try:
                    with self.assertRaisesRegex(ValueError, 'helper changed'):
                        self.load()
                finally:
                    path.write_bytes(original)

    def test_helper_symlink_is_refused(self):
        selected = self.directory / 'raw_reuse.py'
        selected.unlink()
        selected.symlink_to(catalog.REUSE_ROOT / 'raw_reuse.py')
        with self.assertRaisesRegex(ValueError, 'helper alias'):
            self.load()

    def test_empty_helper_is_refused(self):
        (self.directory / 'raw_reuse_policy.py').write_bytes(b'')
        with self.assertRaisesRegex(ValueError, 'helper extent'):
            self.load()

    def test_foreign_preloaded_both_helpers_are_refused(self):
        for name in ('raw_reuse', 'raw_reuse_policy'):
            with self.subTest(name=name), patch.dict(sys.modules, {
                    name: types.SimpleNamespace(__file__=__file__)}):
                with self.assertRaisesRegex(ValueError, 'foreign preloaded raw DATA intake helper'):
                    catalog.load_reuse()

    def test_preloaded_helper_without_original_is_refused(self):
        with patch.dict(sys.modules, {'raw_reuse': types.SimpleNamespace()}):
            with self.assertRaisesRegex(ValueError, 'foreign preloaded raw DATA intake helper'):
                catalog.load_reuse()

    def test_same_path_cached_module_is_recompiled(self):
        poison = types.SimpleNamespace(__file__=str(catalog.REUSE_ROOT / 'raw_reuse.py'),
                                       SCHEMA='unreviewed cached code')
        with patch.dict(sys.modules, {'raw_reuse': poison}):
            loaded = catalog.load_reuse()
            self.assertIsNot(loaded, poison)
            self.assertEqual(loaded.SCHEMA, 'kagemusha.native-finality.raw-original-reuse.v1')

    def test_legacy_policy_module_is_not_imported(self):
        poison = types.SimpleNamespace(__file__=__file__)
        with patch.dict(sys.modules, {'policy': poison}):
            loaded = catalog.load_reuse()
            self.assertIs(sys.modules['policy'], poison)
            self.assertEqual(loaded.doc.__module__, 'raw_reuse_policy')

    def test_cached_bytecode_is_never_executed(self):
        for name in ('raw_reuse', 'raw_reuse_policy'):
            source = self.directory / (name + '.py')
            original = source.read_bytes()
            poison = b'raise AssertionError("unreviewed cached bytecode executed")\n'
            source.write_bytes(poison + b'#' * (len(original) - len(poison)))
            py_compile.compile(str(source), doraise=True)
            stamp = source.stat()
            source.write_bytes(original)
            os.utime(source, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
            # Establish the stale cache is actually timestamp/size-valid and harmful.
            spec = importlib.util.spec_from_file_location('poison_control_' + name, source)
            with self.assertRaisesRegex(AssertionError, 'unreviewed cached bytecode executed'):
                spec.loader.exec_module(types.ModuleType(spec.name))
        loaded = self.load()
        self.assertEqual(loaded.SCHEMA, catalog.REUSE.SCHEMA)

    def test_package_relocation_uses_its_selected_checkout(self):
        checkout = self.directory / 'checkout'
        runtime = checkout / 'scripts' / 'acceptance'
        runtime.mkdir(parents=True)
        (checkout / 'Cargo.toml').write_text('[workspace]\n')
        (checkout / '.git').mkdir()
        path_source = runtime / 'paths.py'
        path_source.write_bytes(Path(paths.__file__).read_bytes())
        relocated_paths = types.ModuleType('paths')
        relocated_paths.__file__ = str(path_source)
        exec(compile(path_source.read_bytes(), str(path_source), 'exec'), relocated_paths.__dict__)
        for name in ('raw_reuse.py', 'raw_reuse_policy.py'):
            (runtime / name).write_bytes((self.directory / name).read_bytes())
        with patch.object(catalog, 'REUSE_ROOT', runtime), patch.dict(sys.modules):
            sys.modules['paths'] = relocated_paths
            for name in ('raw_reuse', 'raw_reuse_policy'):
                sys.modules.pop(name, None)
            loaded = catalog.load_reuse()
            self.assertEqual(loaded.QUAL, checkout / 'target' / 'qualification')
            self.assertEqual(sys.modules['raw_reuse_policy'].ROOT, checkout)

    def test_bounded_original_hash_and_document_refusals_preserved(self):
        loaded = self.load()
        original = self.directory / 'inventory.json'
        original.write_bytes(b'{"only":"DATA"}')
        expected = hashlib.sha256(original.read_bytes()).hexdigest()
        self.assertEqual(loaded.doc(original, expected), {'only': 'DATA'})
        self.assertEqual(loaded.sha(original), expected)
        with self.assertRaisesRegex(ValueError, 'selected document hash differs'):
            loaded.doc(original, '1' * 64)
        with self.assertRaisesRegex(ValueError, 'nonempty bounded regular original'):
            loaded.bounded(original, 1)
        alias = self.directory / 'alias.json'
        alias.symlink_to(original)
        with self.assertRaisesRegex(ValueError, 'unaliased absolute original'):
            loaded.doc(alias)
        with self.assertRaisesRegex(ValueError, 'unaliased hash original'):
            loaded.sha(alias)

    def test_transport_exclusive_publication_is_preserved(self):
        loaded = self.load()
        raw = self.directory / 'originals'
        raw.mkdir()
        out = self.directory / 'output'
        out.mkdir()
        value = {'root': str(raw)}
        selected = loaded.transport(value, out)
        self.assertEqual(loaded.doc(Path(selected['path']), selected['sha256']), [str(raw)])
        with self.assertRaises(FileExistsError):
            loaded.transport(value, out)


if __name__ == '__main__':
    unittest.main(verbosity=2)
