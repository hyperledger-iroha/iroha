"""Synthetic current catalog receipts; no Cargo, native binary, proof or network."""
from pathlib import Path
import copy
import hashlib
import json
import tempfile
import types
import unittest
from unittest.mock import patch

import native_catalog as catalog

policy = catalog.CATALOG_POLICY
PIN = 'a' * 64
OTHER = 'b' * 64


def generated(programs=19, terminals=19, originals=90, stored=1000):
    """The generated count is deliberately not the registry's 18 DATA indices."""
    return (
        f'WALLET_SOURCE_CATALOG routes=22 programs={programs} terminal_keys={terminals} '
        f'originals={originals} stored_bytes={stored} signed=false '
        'qualified_wallet=false actual_receipt_proof=false'
    )


def accepted(q=27):
    return (
        'WALLET_SOURCE_ACCEPTED engineering=true deployment_authority=false complete_routes=22 '
        f'strict_original_reads=900 active_sigma_imports=11 active_q_imports={q} '
        'cancelled_import_opens=0 unavailable_retry=true active_seal_acquisition_cases=14 '
        'receive_alias_cases=56 signature_reuse_cases=91 '
        f'scheme={PIN} manifest={OTHER} genuine_receipt=false native_wallet_open=false'
    )


def members():
    return {
        'schema': policy.MEMBERSHIP_SCHEMA,
        'scope': policy.MEMBERSHIP_SCOPE,
        'wallet_originals': [{'sha256': PIN, 'bytes': 3}, {'sha256': OTHER, 'bytes': 5}],
        'wallet_unique_bytes': 8,
    }


class CurrentCatalogTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='catalog-synthetic-', dir=policy.QUAL)
        self.root = Path(self.temp.name)
        self.output = self.root / 'catalog'
        self.output.mkdir()
        bodies = {name: b'synthetic DATA, not a native artifact' for name in policy.OUTPUT_CAPS}
        bodies['qualified-source-membership.json'] = json.dumps(members()).encode()
        bodies['binary.sha256'] = bytes.fromhex(PIN)
        bodies['source.sha256'] = bytes.fromhex(OTHER)
        bodies['producer-inventory.sha256'] = hashlib.sha256(bodies['producer-inventory.norito']).digest()
        bodies['source-policy.norito'] = b'synthetic source policy DATA'
        for name, raw in bodies.items():
            (self.output / name).write_bytes(raw)
        self.stderr = generated() + '\n' + accepted() + '\n'
        (self.root / 'run.stderr').write_text(self.stderr)

    def tearDown(self):
        self.temp.cleanup()

    def declared(self):
        return policy.completed_outputs(self.output, PIN, OTHER)

    def details(self, stderr=None):
        return policy.completion_details(
            self.output, self.declared(), self.stderr if stderr is None else stderr)

    def test_current_counts_and_retained_membership_are_reported(self):
        value = self.details()
        self.assertEqual(value['generated']['programs'], 19)
        self.assertEqual(value['generated']['routes'], 22)
        self.assertEqual(value['accepted']['q_imports'], 27)
        self.assertEqual(value['accepted']['sigma_imports'], 11)
        self.assertEqual(value['accepted']['active_seal_acquisition_cases'], 14)
        self.assertEqual(value['accepted']['receive_alias_cases'], 56)
        self.assertEqual(value['accepted']['signature_reuse_cases'], 91)
        self.assertEqual(value['wallet_originals'], 2)
        self.assertEqual(value['wallet_unique_bytes'], 8)
        self.assertEqual(self.declared()['binary.sha256']['bytes'], 32)

    def test_route_domain_cannot_be_old52_registry18_or_subset(self):
        for count in (18, 21, 23, 52):
            for key in ('routes=22', 'complete_routes=22'):
                with self.subTest(count=count, key=key), self.assertRaises(ValueError):
                    self.details(self.stderr.replace(key, key.split('=')[0] + '=' + str(count)))

    def test_both_unique_ordered_markers_are_mandatory(self):
        for text in (
            '', generated(), accepted(), accepted() + '\n' + generated(),
            self.stderr + generated(), self.stderr + accepted(),
            self.stderr + 'WALLET_SOURCE_ACCEPTED_OTHER DATA',
            self.stderr + 'WALLET_SOURCE_CATALOG_OTHER DATA',
        ):
            with self.subTest(text=text), self.assertRaises(ValueError):
                self.details(text)
        self.assertEqual(self.details('diagnostic DATA\n' + self.stderr)['generated']['programs'], 19)

    def test_current_control_and_scope_fields_cannot_be_omitted_or_weakened(self):
        changes = {
            'active_sigma_imports=11': ('active_sigma_imports=1', 'active_sigma_imports=10'),
            'active_seal_acquisition_cases=14': ('active_seal_acquisition_cases=3',),
            'cancelled_import_opens=0': ('cancelled_import_opens=1',),
            'unavailable_retry=true': ('unavailable_retry=false',),
            'receive_alias_cases=56': ('receive_alias_cases=0', ''),
            'signature_reuse_cases=91': ('signature_reuse_cases=0', ''),
            'engineering=true': ('engineering=false',),
            'deployment_authority=false': ('deployment_authority=true',),
            'signed=false': ('signed=true',),
            'qualified_wallet=false': ('qualified_wallet=true',),
            'actual_receipt_proof=false': ('actual_receipt_proof=true',),
            'genuine_receipt=false': ('genuine_receipt=true',),
            'native_wallet_open=false': ('native_wallet_open=true',),
        }
        for field, candidates in changes.items():
            for replacement in candidates:
                with self.subTest(field=field, replacement=replacement), self.assertRaises(ValueError):
                    self.details(self.stderr.replace(field, replacement))

    def test_counts_are_canonical_bounded_and_cross_checked(self):
        for field in ('programs=19', 'terminal_keys=19', 'originals=90',
                      'stored_bytes=1000', 'strict_original_reads=900',
                      'active_q_imports=27', 'receive_alias_cases=56', 'signature_reuse_cases=91'):
            for number in ('0', '-1', '01', '1.0', 'true', str(1 << 64)):
                with self.subTest(field=field, number=number), self.assertRaises(ValueError):
                    self.details(self.stderr.replace(field, field.split('=')[0] + '=' + number))
        for text in (
            generated(programs=23) + '\n' + accepted(),
            generated(terminals=20) + '\n' + accepted(),
            generated(originals=4097) + '\n' + accepted(),
            generated(stored=(128 << 30) + 1) + '\n' + accepted(),
            generated() + '\n' + accepted(q=18),
            generated() + '\n' + accepted(q=58),
            generated(stored=7) + '\n' + accepted(),
        ):
            with self.subTest(text=text), self.assertRaises(ValueError):
                self.details(text)
        # Retained unselected reuse DATA is allowed; stored bytes need not equal membership.
        self.assertEqual(self.details()['generated']['stored_bytes'], 1000)

    def test_marker_pins_are_nonzero_lowercase(self):
        for value in ('0' * 64, 'A' * 64, 'a' * 63, 'g' * 64):
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.details(self.stderr.replace('scheme=' + PIN, 'scheme=' + value))

    def test_membership_closed_schema_and_plain_lengths(self):
        value = members()
        bad = [[], {}, value | {'foreign': 1},
               value | {'schema': 'legacy-membership'},
               value | {'scope': 'deployment authority'},
               value | {'wallet_originals': []},
               value | {'wallet_originals': {}},
               value | {'wallet_unique_bytes': 9}]
        for size in (True, False, 0, -1, '3', 3.0, (1 << 30) + 1):
            modified = copy.deepcopy(value)
            modified['wallet_originals'][0]['bytes'] = size
            bad.append(modified)
        for size in (True, 0, '8', 8.0, (128 << 30) + 1):
            bad.append(value | {'wallet_unique_bytes': size})
        for candidate in bad:
            with self.subTest(candidate=candidate), self.assertRaises(ValueError):
                policy.membership(json.dumps(candidate))
        self.assertEqual(policy.membership(json.dumps(value)), value)

    def test_membership_refuses_duplicate_unsorted_foreign_and_zero_originals(self):
        for rows in (
            members()['wallet_originals'][::-1],
            [members()['wallet_originals'][0]] * 2,
            [{'sha256': PIN, 'bytes': 3, 'foreign': 1}],
            [{'sha256': '0' * 64, 'bytes': 3}],
            [{'sha256': PIN.upper(), 'bytes': 3}],
        ):
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                policy.membership(json.dumps(members() | {'wallet_originals': rows}))
        for raw in ('{"x":1,"x":2}', '{"x":NaN}', '{"x":Infinity}', '{"x":-Infinity}'):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                policy.strict_json(raw)

    def test_membership_must_fit_generated_originals(self):
        value = members()
        value['wallet_originals'] = [{'sha256': digit * 64, 'bytes': 1} for digit in 'abcd']
        value['wallet_unique_bytes'] = 4
        (self.output / 'qualified-source-membership.json').write_text(json.dumps(value))
        with self.assertRaisesRegex(ValueError, 'exceeds inventory'):
            self.details(generated(originals=1) + '\n' + accepted())

    def test_changed_membership_is_not_joined_to_prior_declared_hash(self):
        declared = self.declared()
        path = self.output / 'qualified-source-membership.json'
        path.write_bytes(path.read_bytes() + b'\n')
        with self.assertRaisesRegex(ValueError, 'changed between reads'):
            policy.completion_details(self.output, declared, self.stderr)

    def test_digest_files_bind_selected_compiler_source_and_inventory(self):
        for name in ('binary.sha256', 'source.sha256', 'producer-inventory.sha256'):
            path = self.output / name
            raw = path.read_bytes()
            for replacement in (b'x' * 32, raw[:-1], raw + b'\0'):
                path.write_bytes(replacement)
                with self.subTest(name=name), self.assertRaises(ValueError):
                    self.declared()
            path.write_bytes(raw)
        for binary, source in ((OTHER, OTHER), (PIN, PIN)):
            with self.assertRaises(ValueError):
                policy.completed_outputs(self.output, binary, source)

    def test_output_missing_empty_and_symlink_are_refused(self):
        path = self.output / 'input-pins.norito'
        raw = path.read_bytes()
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            self.declared()
        path.write_bytes(b'')
        with self.assertRaises(ValueError):
            self.declared()
        path.unlink()
        target = self.root / 'target'
        target.write_bytes(raw)
        path.symlink_to(target)
        with self.assertRaises(ValueError):
            self.declared()

    def test_completed_preserves_exact_source_pins_and_observation_consistency(self):
        def observe(path, cap):
            raw = policy.bounded(path, cap)
            return {'path': str(path), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
        keys = {
            'KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256', 'KAGEMUSHA_WALLET_CATALOG_PRODUCER_SHA256',
            'KAGEMUSHA_WALLET_CATALOG_SOURCE_SHA256', 'KAGEMUSHA_WALLET_CATALOG_INVENTORY_SHA256',
            'KAGEMUSHA_WALLET_CATALOG_PACK_SHA256', 'KAGEMUSHA_WALLET_SCHEME_ID',
            'KAGEMUSHA_WALLET_MANIFEST_DIGEST',
        }
        data = types.SimpleNamespace(
            require=policy.require, SOURCE_PINS=keys, observe=observe,
            retained_text=lambda path: policy.bounded(path, 1 << 20).decode())
        compiler = {'sha256': PIN, 'source_sha256': OTHER}
        outputs, pins = catalog.completed(self.output, compiler, {'sha256': PIN}, data, policy)
        self.assertEqual(set(pins), keys)
        self.assertEqual(pins['KAGEMUSHA_WALLET_CATALOG_SOURCE_SHA256'], OTHER)
        self.assertEqual(pins['KAGEMUSHA_WALLET_MANIFEST_DIGEST'], OTHER)
        self.assertEqual(set(outputs), set(policy.OUTPUT_CAPS) | {'source-policy.norito'})
        data.observe = lambda path, cap: observe(path, cap) | {'sha256': OTHER}
        with self.assertRaisesRegex(ValueError, 'output changed between reads'):
            catalog.completed(self.output, compiler, {'sha256': PIN}, data, policy)

    def test_policy_loader_compiles_exact_bytes_and_refuses_wrong_identity(self):
        pin = hashlib.sha256(catalog.POLICY.read_bytes()).hexdigest()
        self.assertEqual(catalog.load_policy(pin).ROUTES, 22)
        with self.assertRaisesRegex(ValueError, 'original differs'):
            catalog.load_policy(PIN)
        alias = self.root / 'policy-alias.py'
        alias.symlink_to(catalog.POLICY)
        with patch.object(catalog, 'POLICY', alias), self.assertRaisesRegex(ValueError, 'alias'):
            catalog.load_policy(pin)

    def test_existing_exact_test_inventory_and_exit_checks_still_apply(self):
        stdout = (f'running 1 test\ntest {policy.TEST} ... ok\n'
                  'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; '
                  '2 filtered out; finished in 1.0s\n')
        self.assertEqual(policy.test_result(stdout, 0)['passed'], 1)
        policy.compiled_inventory(policy.TEST + ': test\n1 test, 0 benchmarks\n', '', 0)
        for code in (True, 1, -1, None):
            with self.assertRaises(ValueError):
                policy.test_result(stdout, code)
            with self.assertRaises(ValueError):
                policy.compiled_inventory(policy.TEST + ': test\n1 test, 0 benchmarks\n', '', code)
        with self.assertRaises(ValueError):
            policy.test_result(stdout.replace('... ok', '... ignored'), 0)
        with self.assertRaises(ValueError):
            policy.compiled_inventory(policy.TEST + ': test\n2 tests, 0 benchmarks\n', '', 0)

    def test_environment_and_bounded_json_retention_still_apply(self):
        self.assertEqual(policy.inherited_environment({'PATH': '/usr/bin'}), {'PATH': '/usr/bin'})
        for key in ('KAGEMUSHA_DATA', 'IROHA_TEST_DATA', 'TEST_NETWORK_DATA',
                    'RUST_TEST_THREADS', 'LD_PRELOAD', 'DYLD_LIBRARY_PATH', 'PYTHONOPTIMIZE'):
            with self.subTest(key=key), self.assertRaises(ValueError):
                policy.inherited_environment({key: '1'})
        path = self.root / 'saved.json'
        row = policy.save(path, {'data': 1})
        self.assertEqual(policy.doc(path, row['sha256']), {'data': 1})
        with self.assertRaises(FileExistsError):
            policy.save(path, {})
        path.write_text('{"data":1,"data":2}')
        with self.assertRaises(ValueError):
            policy.doc(path)


if __name__ == '__main__':
    unittest.main()
