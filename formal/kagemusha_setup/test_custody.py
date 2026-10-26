"""Cheap source-inventory and pre-I/O optimized-mode refusal controls."""
import copy
from contextlib import ExitStack
import json
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from . import custody


class CustodyTests(unittest.TestCase):
    def test_generated_registry_is_observed_data_not_source_authority(self):
        manifest = custody.checked_sources()
        self.assertNotIn(custody.GENERATED_DATA, manifest['sources'])
        before = custody.observed_generated_data()
        read_bytes = custody.Path.read_bytes
        def substituted(path):
            raw = read_bytes(path)
            return raw + b'\nchanged DATA' if path == custody.ROOT/custody.GENERATED_DATA else raw
        with patch.object(custody.Path, 'read_bytes', substituted):
            self.assertEqual(custody.checked_sources(), manifest)
            after = custody.observed_generated_data()
        self.assertNotEqual(before['sha256'], after['sha256'])
        self.assertEqual(before['role'], 'unqualified generated DATA')
        with patch.object(custody.Path, 'is_symlink', return_value=True):
            with self.assertRaisesRegex(ValueError, 'regular file'):
                custody.observed_generated_data()
        changed = copy.deepcopy(manifest)
        changed['sources'][custody.GENERATED_DATA] = before['sha256']
        with self.assertRaisesRegex(ValueError, 'dependency inventory'):
            custody.validate_manifest(changed)
        for name in ('crates/iroha_plonk/build_support/source_identity.rs',
                     'crates/iroha_plonk/src/keys/keygen/builtin.rs'):
            def changed_owner(path, selected=custody.ROOT/name):
                raw = read_bytes(path)
                return raw + b'\nchanged owner' if path == selected else raw
            with self.subTest(source=name):
                with patch.object(custody.Path, 'read_bytes', changed_owner):
                    with self.assertRaisesRegex(ValueError, 'changed source '+name):
                        custody.checked_sources()

    def test_exact_inventory_rejects_missing_extra_or_noncanonical_pins(self):
        manifest = custody.checked_sources()
        for role in ('files', 'sources'):
            changed = copy.deepcopy(manifest)
            del changed[role][next(iter(changed[role]))]
            with self.assertRaisesRegex(ValueError, 'inventory'):
                custody.validate_manifest(changed)
            changed = copy.deepcopy(manifest)
            changed[role]['outside.py'] = '0'*64
            with self.assertRaisesRegex(ValueError, 'inventory'):
                custody.validate_manifest(changed)
        # Newly exported exact-source reconstruction is part of the key authority.
        # Missing roles or changed bodies must not pass by refreshing unrelated pins.
        reconstruction = (
            'crates/iroha_plonk/src/keys/keygen/rebuild.rs',
            'crates/iroha_plonk/src/keys/mod.rs',
            'crates/iroha_plonk/src/keys/source_fingerprint.rs',
        )
        read_bytes = custody.Path.read_bytes
        for name in (*reconstruction, *sorted(custody.DELEGATED_SOURCES)):
            self.assertIn(name, manifest['sources'])
            changed = copy.deepcopy(manifest)
            del changed['sources'][name]
            with self.subTest(source=name, mutation='missing'):
                with self.assertRaisesRegex(ValueError, 'dependency inventory'):
                    custody.validate_manifest(changed)
            def substituted(path, selected=custody.ROOT/name):
                raw = read_bytes(path)
                return raw + b'\nchanged' if path == selected else raw
            with self.subTest(source=name, mutation='body'):
                with patch.object(custody.Path, 'read_bytes', substituted):
                    with self.assertRaisesRegex(ValueError, 'changed source '+name):
                        custody.checked_sources()
        changed = copy.deepcopy(manifest)
        changed['files'][next(iter(changed['files']))] = 'F'*64
        with self.assertRaisesRegex(ValueError, 'SHA256'):
            custody.validate_manifest(changed)

    def test_optimized_execution_refuses_before_manifest_read(self):
        with patch.object(custody.sys, 'flags', SimpleNamespace(optimize=1)):
            with patch.object(custody.Path, 'read_text', side_effect=RuntimeError('unexpected read')):
                with self.assertRaisesRegex(ValueError, 'unoptimized'):
                    custody.checked_sources()

        # The maintained runner must also reject bytecode-writing invocation
        # before it opens the manifest or imports arithmetic/proof owners.
        from .check import main
        with patch.object(custody.sys,'dont_write_bytecode',False):
            with patch.object(custody.Path,'read_bytes',side_effect=RuntimeError('unexpected source read')):
                with self.assertRaisesRegex(ValueError,'bytecode-free'):main()

    def _synthetic_runner(self, final_observation, primary_error=None):
        """Run main's control flow only; no model/proof work or retained fake proof claim."""
        from . import check
        before = {'path': 'synthetic-generated', 'sha256': '0'*64, 'bytes': 0,
                  'role': 'synthetic runner fixture only'}
        class_names = ('CustodyTests', 'FreshTarget', 'PreimageTests', 'AdapterTests',
                       'SeamTests', 'ProofControls', 'ProducerTests', 'OwnerTests',
                       'GenericKnownLog', 'CompactSourceControls', 'FileCustodyTests',
                       'ReferenceCustodyTests', 'RequestShapeTests')
        classes = {name: type(name, (), {
            'verified_toy_proofs': 14 if name == 'ProofControls' else 0,
            'verified_adapted_toy_proofs': 6 if name == 'ProofControls' else 0,
        }) for name in class_names}
        module = SimpleNamespace(**classes)
        # These labels and counters exercise runner bookkeeping, never proof validity.
        names = ['test_synthetic_'+str(i) for i in range(116)]
        fake_result = SimpleNamespace(testsRun=116, failures=[], errors=[], skipped=[],
                                      wasSuccessful=lambda: True)
        runner = SimpleNamespace(run=lambda suite: fake_result)
        observations = [before, final_observation] if primary_error else [before, before, final_observation]
        with TemporaryDirectory(prefix='kagemusha-check-synthetic-') as directory:
            here = Path(directory)
            (here/'source_manifest.json').write_text('synthetic runner manifest\n')
            output = here/'output'
            with ExitStack() as patches:
                patches.enter_context(patch.object(custody, 'HERE', here))
                patches.enter_context(patch.object(custody, 'checked_sources', return_value={}))
                patches.enter_context(patch.object(custody, 'observed_generated_data', side_effect=observations))
                patches.enter_context(patch.object(custody, 'validate_algebra',
                                                   side_effect=primary_error, return_value={}))
                patches.enter_context(patch.object(check.sys, 'argv', ['synthetic-check', '--output', str(output)]))
                imports = patches.enter_context(patch.object(check.importlib, 'import_module', return_value=module))
                patches.enter_context(patch.object(check.unittest.defaultTestLoader, 'getTestCaseNames',
                    side_effect=lambda klass: names if klass is classes['CustodyTests'] else []))
                patches.enter_context(patch.object(check.unittest.defaultTestLoader, 'loadTestsFromTestCase',
                                                   return_value=unittest.TestSuite()))
                patches.enter_context(patch.object(check.unittest, 'TextTestRunner', return_value=runner))
                result, caught = None, None
                try:
                    result = check.main()
                except BaseException as error:
                    caught = error
                record = json.loads((output/'result.json').read_text())
                if primary_error is not None:
                    imports.assert_not_called()
            # The temporary synthetic receipt is removed here; it is not proof evidence.
        return result, caught, record, before

    def test_runner_final_observation_controls_success_and_exit(self):
        """A final read refusal or changed observation cannot retain success/exit zero."""
        before = {'path': 'synthetic-generated', 'sha256': '0'*64, 'bytes': 0,
                  'role': 'synthetic runner fixture only'}
        for observation in (before, dict(before, sha256='1'*64),
                            FileNotFoundError('synthetic generated file vanished'),
                            ValueError('synthetic generated path became aliased'),
                            KeyboardInterrupt('synthetic final observation interrupt')):
            with self.subTest(observation=repr(observation)):
                code, error, record, initial = self._synthetic_runner(observation)
                unchanged = observation == before
                self.assertEqual(record['success'], unchanged)
                self.assertEqual(record['generated_data_unchanged'], unchanged)
                self.assertEqual(record['generated_data_before'], initial)
                self.assertFalse(record['generated_data_qualified'])
                self.assertFalse(record['C12_closed'])
                self.assertFalse(record['current_setup_qualified'])
                if isinstance(observation, BaseException):
                    self.assertIs(error, observation)
                    self.assertIsNone(code)
                    self.assertIsNone(record['generated_data_after'])
                    self.assertEqual(record['generated_data_after_error'],
                                     type(observation).__name__+': '+str(observation))
                else:
                    self.assertIsNone(error)
                    self.assertEqual(code, 0 if unchanged else 1)
                    self.assertEqual(record['generated_data_after'], observation)
                    self.assertNotIn('generated_data_after_error', record)

    def test_runner_retains_primary_exception_when_final_observation_fails(self):
        """Post-observation diagnostics cannot replace an earlier error or interrupt."""
        for primary in (ValueError('synthetic primary failure'),
                        KeyboardInterrupt('synthetic primary interrupt')):
            with self.subTest(primary=type(primary).__name__):
                secondary = FileNotFoundError('synthetic final observation failure')
                code, error, record, before = self._synthetic_runner(secondary, primary)
                self.assertIs(error, primary)
                self.assertIsNone(code)
                self.assertEqual(record['exception'], type(primary).__name__+': '+str(primary))
                self.assertEqual(record['generated_data_after_error'],
                                 type(secondary).__name__+': '+str(secondary))
                self.assertEqual(record['generated_data_before'], before)
                self.assertIsNone(record['generated_data_after'])
                self.assertFalse(record['generated_data_unchanged'])
                self.assertFalse(record['success'])
                self.assertEqual(record['verified_toy_k6_proofs'], 0)
                self.assertEqual(record['verified_adapted_toy_k6_proofs'], 0)
