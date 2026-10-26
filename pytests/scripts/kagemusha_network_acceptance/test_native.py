"""Synthetic native-controller refusals; no subprocess, catalog, proof or network."""
from pathlib import Path
import copy
import hashlib
import json
import re
import tempfile
import unittest
from unittest.mock import patch
import run as r

PIN = 'a' * 64
OTHER = 'b' * 64


class NativeMigrationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='native-controller-data-', dir=r.QUAL)
        self.root = Path(self.temp.name)
        (self.root / '.git').mkdir()
        checkout = patch.object(r.admission.capture, 'REPO', self.root)
        checkout.start(); self.addCleanup(checkout.stop)

    def tearDown(self):
        self.temp.cleanup()

    def setup_data(self):
        directory = self.root / 'setup'
        directory.mkdir()
        wire = b'synthetic genesis transport, never native authority'
        capture = {'version': 1, 'chain_id': 'DATA', 'signed_genesis_wire_hex': wire.hex()}
        contents = {name: name.encode() for name in r.component.SETUP_NAMES}
        contents.update({'signed-genesis.wire': wire, 'capture.json': json.dumps(capture).encode()})
        rows = []
        for name, value in contents.items():
            path = directory / name
            path.write_bytes(value)
            item = r.observe(path)
            rows.append({k: item[k] for k in ('bytes', 'sha256')} | {'name': name})
        value = {'schema': 'iroha.kagemusha.executed-ledger-setup.v1', 'chain_id': 'DATA',
                 'registration_height': 1, 'certified_height': 2,
                 'initial_supply_atomic_units': '1000', 'originals': rows}
        item = r.save(directory / 'setup.json', value)
        return r.ref(item), value, r.component.manifest_rows(directory, rows, r.component.SETUP_NAMES)

    def catalog_data(self, output):
        output.mkdir(parents=True)
        bodies = {name: b'synthetic output DATA' for name in r.CATALOG_POLICY.OUTPUT_CAPS}
        bodies['binary.sha256'] = bytes.fromhex(PIN)
        bodies['source.sha256'] = bytes.fromhex(OTHER)
        bodies['producer-inventory.sha256'] = hashlib.sha256(bodies['producer-inventory.norito']).digest()
        bodies['source-policy.norito'] = b'synthetic source policy DATA'
        bodies['qualified-source-membership.json'] = json.dumps({
            'schema': r.CATALOG_POLICY.MEMBERSHIP_SCHEMA,
            'scope': r.CATALOG_POLICY.MEMBERSHIP_SCOPE,
            'wallet_originals': [{'sha256': PIN, 'bytes': 3}, {'sha256': OTHER, 'bytes': 5}],
            'wallet_unique_bytes': 8,
        }).encode()
        for name, value in bodies.items():
            (output / name).write_bytes(value)
        (output.parent / 'run.stderr').write_text(
            'WALLET_SOURCE_CATALOG routes=22 programs=19 terminal_keys=19 originals=90 stored_bytes=1000 '
            'signed=false qualified_wallet=false actual_receipt_proof=false\n'
            'WALLET_SOURCE_ACCEPTED engineering=true deployment_authority=false complete_routes=22 '
            'strict_original_reads=900 active_sigma_imports=11 active_q_imports=27 '
            'cancelled_import_opens=0 unavailable_retry=true active_seal_acquisition_cases=14 '
            'receive_alias_cases=56 signature_reuse_cases=91 '
            f'scheme={PIN} manifest={OTHER} genuine_receipt=false native_wallet_open=false\n')

    def exchange_data(self):
        output = self.root / 'exchange'
        output.mkdir()
        wallet = output / 'wallet-c'
        wallet.mkdir()
        def originals(directory, names):
            rows = []
            for name in sorted(names):
                path = directory / name
                path.write_bytes(('DATA only ' + name).encode())
                item = r.observe(path)
                rows.append({'name': name, 'bytes': item['bytes'], 'sha256': item['sha256']})
            return rows
        archive_rows = originals(output, r.component.ARCHIVE_NAMES)
        c_rows = originals(wallet, {'credential.norito', 'certificates.norito', 'account.norito', 'asset.norito'})
        originals(output, {'a-b-payment.norito', 'b-c-payment.norito', 'b-receive.norito',
                           'c-receive.norito', 'c-unload-claim.norito'})
        env = {key: PIN for key in r.SOURCE_PINS} | {
            'KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256': PIN,
            'KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256': PIN}
        value = {'schema': 'iroha.kagemusha.native-abc-result.v1',
                 'ledger_settlement_executed': False, 'physical_device_qualified': False,
                 'c_unload_claim_sha256': r.observe(output / 'c-unload-claim.norito')['sha256'],
                 'target_sha256': PIN, 'executed_ledger_setup_sha256': PIN,
                 'source_pins': {key: PIN for key in r.SOURCE_PINS},
                 'archive_receive_folded': True, 'archive_status_folded': True,
                 'archive_exact_replay_after_restart': True,
                 'archive_originals': archive_rows, 'c_open_originals': c_rows}
        def check(candidate):
            (output / 'result.json').write_text(json.dumps(candidate))
            return r.component.output_records('exchange', output, env)
        return output, value, check

    def test_exchange_requires_both_archive_folds_and_restart_replay(self):
        output, value, check = self.exchange_data()
        observed = check(value)
        self.assertEqual(len(observed), 14)
        self.assertTrue(r.component.ARCHIVE_NAMES <= set(observed))
        for key in ('archive_receive_folded', 'archive_status_folded', 'archive_exact_replay_after_restart'):
            missing = value.copy(); del missing[key]
            for candidate in [missing, *(value | {key: wrong} for wrong in (False, 1, 'true', None, []))]:
                with self.assertRaisesRegex(ValueError, 'both Archive folds'):
                    check(candidate)

    def test_archive_rows_refuse_missing_duplicate_foreign_and_escaping_names(self):
        output, value, check = self.exchange_data()
        original = value['archive_originals']
        candidates = [None, original[:-1], original + [original[0]], [original[0]] * 4]
        for name in ('foreign.norito', '../a-archive-receive.norito'):
            rows = copy.deepcopy(original); rows[0]['name'] = name; candidates.append(rows)
        for rows in candidates:
            with self.assertRaises(ValueError):
                check(value | {'archive_originals': rows})

    def test_archive_exact_originals_refuse_replacement_absence_and_alias(self):
        output, value, check = self.exchange_data()
        for row in value['archive_originals']:
            path = output / row['name']; raw = path.read_bytes()
            path.write_bytes(raw[:-1] + b'!')
            with self.assertRaisesRegex(ValueError, 'hash/extent differs'): check(value)
            path.unlink()
            with self.assertRaises(FileNotFoundError): check(value)
            alias = output / 'alias-data'; alias.write_bytes(raw); path.symlink_to(alias)
            with self.assertRaisesRegex(ValueError, 'symlink or alias'): check(value)
            path.unlink(); alias.unlink(); path.write_bytes(raw)
        self.assertEqual(len(check(value)), 14)

    def test_archive_length_is_plain_bounded_integer_and_actual_extent(self):
        output, value, check = self.exchange_data()
        for length in (True, 1.0, '1', 0, -1, 10035):
            rows = copy.deepcopy(value['archive_originals']); rows[0]['bytes'] = length
            with self.assertRaisesRegex(ValueError, 'original length bound'):
                check(value | {'archive_originals': rows})
        for length in (10000, 10001, 10034):
            for row in value['archive_originals']:
                path = output / row['name']; raw = b'x' * length; path.write_bytes(raw)
                row.update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
            self.assertTrue(all(check(value)[name]['bytes'] == length for name in r.component.ARCHIVE_NAMES))
        path.write_bytes(raw + b'x')
        with self.assertRaisesRegex(ValueError, 'shape/size'): check(value)

    def test_exchange_payment_and_receive_originals_obey_message_bound(self):
        output, value, check = self.exchange_data()
        for name in ('a-b-payment.norito', 'b-c-payment.norito', 'b-receive.norito', 'c-receive.norito'):
            path = output / name; raw = path.read_bytes()
            for length in (10000, 10001, 10034):
                path.write_bytes(b'x' * length)
                self.assertEqual(check(value)[name]['bytes'], length)
            path.write_bytes(b'x' * 10035)
            with self.assertRaisesRegex(ValueError, 'shape/size'): check(value)
            path.write_bytes(raw)

    def test_exchange_message_limit_matches_current_native_model(self):
        model = (r.ROOT / 'crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1.rs').read_text()
        match = re.search(r'^pub const KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1: usize = ([0-9_]+);$',
                          model, re.MULTILINE)
        self.assertIsNotNone(match)
        self.assertEqual(r.component.MESSAGE_LIMIT, int(match.group(1).replace('_', '')))

    def test_original_manifest_cap_must_be_finite_plain_integer(self):
        for cap in (0, -1, True, 1.0, '10000', r.component.CAP + 1):
            with self.assertRaisesRegex(ValueError, 'finite original bound'):
                r.component.manifest_rows(self.root, [], set(), cap=cap)
        self.assertEqual(r.component.manifest_rows(self.root, [], set(), cap=10000), {})

    def test_live_setup_exact_capture_precedes_single_catalog(self):
        setup, value, originals = self.setup_data()
        compiler = {'path': '/never/execute', 'sha256': PIN, 'source_sha256': OTHER}
        observed = []
        def fake(binary, test, directory, env, admitted, tools):
            observed.append((binary, test, env))
            self.catalog_data(directory / 'output')
        with patch.object(r, 'exact_test', side_effect=fake), patch.object(r.subprocess, 'Popen', side_effect=AssertionError('no process')):
            fixture, catalog, pins, outputs = r.catalog_from_live_setup(self.root, setup,
                {'binaries': {'iroha_core_zk': compiler}}, {}, {})
        self.assertEqual(len(observed), 1)
        self.assertEqual(observed[0][1], r.CATALOG_POLICY.TEST)
        env = observed[0][2]
        self.assertEqual(env['KAGEMUSHA_SIGNED_GENESIS_FIXTURE'], originals['capture.json']['path'])
        self.assertEqual(env['KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256'], originals['capture.json']['sha256'])
        self.assertEqual(set(env), {'KAGEMUSHA_SIGNED_GENESIS_FIXTURE', 'KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256',
                                   'KAGEMUSHA_WALLET_SOURCE_SHA256', 'KAGEMUSHA_WALLET_CATALOG_OUTPUT',
            'KAGEMUSHA_WALLET_NEW_RAW_BYTES'})
        self.assertEqual(set(pins), set(r.SOURCE_PINS))
        self.assertEqual(len(outputs), 10)
        self.assertEqual(fixture, originals['capture.json'])
        self.assertFalse((self.root / 'snapshot').exists())

    def test_foreign_genesis_chain_and_incomplete_setup_refuse_before_catalog(self):
        setup, value, originals = self.setup_data()
        for field, wrong in [('chain_id', 'foreign'), ('registration_height', 2), ('certified_height', 3),
                             ('initial_supply_atomic_units', '999')]:
            changed = copy.deepcopy(value)
            changed[field] = wrong
            with self.assertRaises(ValueError):
                r.native_catalog.setup_fixture(changed, originals, r.component)
        changed = copy.deepcopy(originals)
        changed['signed-genesis.wire']['sha256'] = OTHER
        with self.assertRaisesRegex(ValueError, 'genesis original'):
            r.native_catalog.setup_fixture(value, changed, r.component)
        (Path(originals['capture.json']['path'])).write_bytes(b'changed')
        with patch.object(r, 'exact_test') as child:
            with self.assertRaises(ValueError):
                r.catalog_from_live_setup(self.root, setup, {}, {}, {})
            child.assert_not_called()

    def test_native_catalog_requires_current_marker_and_raw_digest(self):
        output = self.root / 'catalog' / 'output'
        self.catalog_data(output)
        compiler = {'sha256': PIN, 'source_sha256': OTHER}
        call = lambda: r.native_catalog.completed(output, compiler, {'sha256': PIN}, r.component, r.CATALOG_POLICY)
        call()
        stderr = output.parent / 'run.stderr'
        original = stderr.read_text()
        for changed in [original.replace('complete_routes=22', 'complete_routes=21'),
                        original.replace('complete_routes=22', 'complete_routes=52'),
                        original.replace('active_seal_acquisition_cases=14', 'active_seal_acquisition_cases=33'),
                        original.replace('active_seal_acquisition_cases=14', 'active_seal_acquisition_cases=3'),
                        original + original, original.replace('genuine_receipt=false', 'genuine_receipt=true')]:
            stderr.write_text(changed)
            with self.assertRaises(ValueError): call()
        stderr.write_text(original)
        (output / 'binary.sha256').write_bytes(bytes.fromhex(OTHER))
        with self.assertRaisesRegex(ValueError, 'raw original digest'): call()

    def test_native_evidence_cap_flag_and_exact_bindings(self):
        output = self.root / 'evidence'
        output.mkdir()
        receipt = output / 'receipt.norito'; receipt.write_bytes(b'DATA receipt')
        finality = output / 'load-finality.norito'; finality.write_bytes(b'x' * 20000)
        rec, fin = r.observe(receipt), r.observe(finality)
        env = {key: PIN for key in r.SOURCE_PINS} | {
            'KAGEMUSHA_EXECUTED_LOAD_RECEIPT_SHA256': rec['sha256'],
            'KAGEMUSHA_EXECUTED_LOAD_CAPTURE_SHA256': PIN,
            'KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256': PIN,
            'KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256': PIN}
        value = {'schema': 'iroha.kagemusha.generated-native-load-finality.v1',
                 'native_bls_finality': True, 'existing_only_recovery': True, 'receipt_height': 5,
                 'registration_height': 3, 'activation_height': 4,
                 'receipt_sha256': rec['sha256'], 'receipt_bytes': rec['bytes'],
                 'finality_sha256': fin['sha256'], 'finality_bytes': fin['bytes'],
                 'capture_sha256': PIN, 'target_sha256': PIN, 'setup_sha256': PIN,
                 'source_pins': {key: env[key] for key in r.SOURCE_PINS}}
        def check(candidate):
            (output / 'generated.json').write_text(json.dumps(candidate))
            return r.component.output_records('load', output, env, expected_heights={
                'registration_height': 3, 'activation_height': 4, 'certified_height': 5})
        self.assertEqual(check(value)['finality']['bytes'], 20000)
        for key, wrong in [('native_bls_finality', False), ('receipt_height', 6),
                           ('registration_height', 4), ('activation_height', 5), ('finality_bytes', 1),
                           ('finality_sha256', OTHER), ('setup_sha256', OTHER)]:
            with self.assertRaises(ValueError): check(value | {key: wrong})
        old = value.copy(); del old['native_bls_finality']; old['genuine_load_proof'] = True
        with self.assertRaises(ValueError): check(old)
        finality.write_bytes(b'x' * 262145)
        with self.assertRaisesRegex(ValueError, 'shape/size'): check(value)

    def test_selector_and_named_result_refuse_zero_wrong_duplicate_or_ignored(self):
        expected = r.LOAD
        listing = expected + ': test\n\n1 test, 0 benchmarks\n'
        r.exact_inventory(expected, listing, '', 0)
        for stdout, stderr, code in [(listing, '', 1), (listing, 'warning', 0),
                                     ('0 tests, 0 benchmarks\n', '', 0), (listing + listing, '', 0)]:
            with self.assertRaises(ValueError): r.exact_inventory(expected, stdout, stderr, code)
        log = f'running 1 test\ntest {expected} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 1.0s\n'
        self.assertTrue(r.exact_result(expected, log, 0))
        for candidate, code in [(log.replace(expected, r.TARGET), 0), (log, 1),
                                 (log + log, 0), (log.replace('... ok', '... ignored'), 0)]:
            self.assertFalse(r.exact_result(expected, candidate, code))

    def test_native_environment_has_exact_seven_pins_and_no_retired_plumbing(self):
        compiler = {'source_sha256': PIN}
        pins = {key: PIN for key in r.SOURCE_PINS}
        with patch.dict(r.os.environ, {'KAGEMUSHA_WALLET_REUSE_INPUTS': '/never'}, clear=True):
            env = r.native_environment({'binaries': {'iroha_core_zk': compiler}},
                {'path': '/selected/capture.json'}, self.root, {'path': '/setup.json', 'sha256': PIN}, pins)
        self.assertNotIn('KAGEMUSHA_WALLET_REUSE_INPUTS', env)
        self.assertNotIn('KAGEMUSHA_FINALITY_METADATA_SNAPSHOT', env)
        self.assertEqual(env['KAGEMUSHA_SIGNED_GENESIS_FIXTURE'], '/selected/capture.json')
        for key in ('KAGEMUSHA_WALLET_REUSE_INPUTS', 'DYLD_INSERT_LIBRARIES', 'IROHA_TEST_SKIP_BUILD'):
            with self.assertRaises(ValueError): r.CATALOG_POLICY.inherited_environment({key: 'DATA'})
        with self.assertRaises(ValueError):
            r.native_environment({}, {}, self.root, {}, pins | {'KAGEMUSHA_FINALITY_PRODUCER_SHA256': PIN})

    def test_retired_graph_and_native_supplements_refuse_before_capture_reads(self):
        with patch.object(r.compiler_capture, 'admit') as touched:
            with self.assertRaisesRegex(ValueError, 'three exact producer roles'):
                r.compile_selection({'schema': r.SCHEMA, 'captures': dict.fromkeys(('stock', 'owner', 'graph', 'native'), {})})
            native = {'directory': '/DATA', 'result_sha256': PIN, 'review_sha256': PIN}
            for extra in ('source_equivalence', 'historical_runtime_supplement', 'bytes_retirement', 'closure_directory'):
                with self.assertRaisesRegex(ValueError, 'no supplements'):
                    r.native_capture.admit(native | {extra: {}})
            touched.assert_not_called()

    def test_current_native_projection_preserves_all_source_and_retained_custody(self):
        selected = {'directory': '/DATA', 'result_sha256': PIN, 'review_sha256': OTHER}
        source = {'crates/native.rs': PIN}
        original = {'/DATA/current-native-input': OTHER}
        binary = {'path': '/DATA/current-native', 'sha256': PIN}
        custody = {'reviewed_originals': original, 'before_head': '1' * 40, 'after_head': '1' * 40}
        result = {'profile': 'release', 'binaries': {'iroha_core_zk': binary},
                  'classification': 'current_complete_compiler_closure', 'source_before_sha256': OTHER}
        with patch.object(r.compiler_capture, 'admit', return_value=(result, {'actual_packages': ['crates/native']}, source, custody)) as admit:
            owner, digest = r.native_capture.admit(selected)
        admit.assert_called_once_with(selected, {'iroha_core_zk'})
        self.assertEqual(digest, OTHER)
        self.assertEqual(owner['source'], source)
        self.assertEqual(owner['retained'], original)
        self.assertEqual(owner['binary'], binary)
        self.assertEqual(owner['packages'], ['crates/native'])
        self.assertEqual(owner['heads'], ['1' * 40, '1' * 40])

        def captured(item, expected):
            role = 'stock' if 'iroha3d' in expected else 'owner'
            result = {'profile': 'release', 'command': ['cargo', 'test', '--release', '--no-run'],
                      'classification': 'current_complete_compiler_closure', 'source_before_sha256': PIN,
                      'binaries': {name: {'path': '/DATA/' + name, 'sha256': PIN} for name in expected}}
            return result, {}, {'crates/' + role + '.rs': PIN}, dict(custody, reviewed_originals={'/DATA/' + role + '-input': PIN})
        with patch.object(r.native_capture, 'admit', return_value=(owner, digest)), \
             patch.object(r.admission, 'admit_selection', side_effect=captured):
            joined = r.compile_selection({'schema': r.SCHEMA, 'captures': dict.fromkeys(('stock', 'owner', 'native'), selected)})
        self.assertEqual(joined['native_owner'], owner)
        self.assertEqual(joined['binaries']['iroha_core_zk'], dict(binary, source_sha256=OTHER))
        self.assertEqual(joined['source'], source | {'crates/stock.rs': PIN, 'crates/owner.rs': PIN})
        for name, value in original.items(): self.assertEqual(joined['retained'][name], value)
        for name in ('stock', 'owner'): self.assertEqual(joined['retained']['/DATA/' + name + '-input'], PIN)

    def test_package_namespace_drift_is_refused_even_with_unchanged_file_pins(self):
        admitted = {'source': {}, 'retained': {}, 'binaries': {}, 'captures': {
            'owner': {'custody': {'packages': ['crates/DATA'], 'namespace': ['crates/DATA/source.rs']}}}}
        with patch.object(r.admission.capture, 'git', side_effect=lambda *args: 'optimizations' if args[0] == 'branch' else ''), \
             patch.object(r.admission.source_audit, 'package_paths', return_value={'crates/DATA/source.rs', 'crates/DATA/added.rs'}):
            with self.assertRaisesRegex(ValueError, 'namespace changed'):
                r.unchanged(admitted, {})


    def test_externally_selected_manifest_refuses_changed_helpers_before_runtime(self):
        local = self.root / 'controller'; local.mkdir()
        checkout = self.root / 'checkout'; checkout.mkdir()
        manifest = {'schema': 'iroha.kagemusha.network-acceptance-runtime.v1',
                    'runtime': {'local': {}, 'external': {}}}
        for base, role, names in [(local, 'local', r._RUNTIME_LOCAL),
                                  (checkout, 'external', r._RUNTIME_EXTERNAL)]:
            for name in names:
                path = base / name; path.parent.mkdir(parents=True, exist_ok=True)
                raw = ('DATA only: ' + name).encode(); path.write_bytes(raw)
                manifest['runtime'][role][name] = hashlib.sha256(raw).hexdigest()
        selected = local / 'manifest.json'
        raw = json.dumps(manifest).encode(); selected.write_bytes(raw)
        digest = hashlib.sha256(raw).hexdigest()
        with patch.object(r.subprocess, 'Popen', side_effect=AssertionError('no launch')):
            observed = r.authenticate_runtime(selected, digest, checkout)
            self.assertEqual(observed[str(selected)], digest)
            self.assertEqual(len(observed), 1 + len(r._RUNTIME_LOCAL) + len(r._RUNTIME_EXTERNAL))
            with self.assertRaisesRegex(ValueError, 'manifest changed'):
                r.authenticate_runtime(selected, OTHER, checkout)
            helper = local / 'native_data.py'; before = helper.read_bytes()
            helper.write_bytes(before + b'changed')
            with self.assertRaisesRegex(ValueError, 'frozen runtime helper changed'):
                r.authenticate_runtime(selected, digest, checkout)
            helper.write_bytes(before)
            altered = copy.deepcopy(manifest); del altered['runtime']['external'][next(iter(r._RUNTIME_EXTERNAL))]
            value = json.dumps(altered).encode(); selected.write_bytes(value)
            with self.assertRaisesRegex(ValueError, 'runtime inventory'):
                r.authenticate_runtime(selected, hashlib.sha256(value).hexdigest(), checkout)
            selected.write_bytes(raw)
            alias = local / 'alias.json'; alias.symlink_to(selected)
            with self.assertRaisesRegex(ValueError, 'aliased'):
                r.authenticate_runtime(alias, digest, checkout)

    def test_retained_depfile_and_alias_executable_stay_in_runtime_pins(self):
        paths = [self.root / 'actual.d', self.root / 'alias-executable']
        for path in paths: path.write_bytes(('retained DATA ' + path.name).encode())
        pins = {str(path): r.admission.sha(path) for path in paths}
        r.compiler_capture.check_pins(pins)
        admitted = {'source': {}, 'retained': pins, 'binaries': {}}
        with patch.object(r.admission.capture, 'git', side_effect=lambda *args: 'optimizations' if args[0] == 'branch' else ''):
            r.unchanged(admitted, {})
            for path in paths:
                original = path.read_bytes(); path.write_bytes(original + b'change')
                with self.assertRaisesRegex(ValueError, 'retained source review/tool changed'): r.unchanged(admitted, {})
                with self.assertRaisesRegex(ValueError, 'retained compiler original changed'): r.compiler_capture.check_pins(pins)
                path.write_bytes(original)

    def test_generated_namespace_addition_removal_and_alias_refuse(self):
        root = self.root / 'generated'; root.mkdir()
        r.compiler_capture.check_output_namespace(root, {})
        package = root / 'crates/data'; package.mkdir(parents=True)
        original = package / 'generated.rs'; original.write_bytes(b'DATA')
        with self.assertRaisesRegex(ValueError, 'namespace'): r.compiler_capture.check_output_namespace(root, {})
        expected = {str(original): r.admission.sha(original)}
        r.compiler_capture.check_output_namespace(root, expected)
        new = root / 'unknown-package/extra'; new.parent.mkdir(); new.write_bytes(b'DATA')
        with self.assertRaisesRegex(ValueError, 'namespace'): r.compiler_capture.check_output_namespace(root, expected)
        new.unlink(); new.parent.rmdir(); original.unlink()
        with self.assertRaisesRegex(ValueError, 'namespace'): r.compiler_capture.check_output_namespace(root, expected)
        original.symlink_to(root)
        with self.assertRaisesRegex(ValueError, 'aliased'): r.compiler_capture.check_output_namespace(root, expected)

    def test_later_withdrawal_and_dangling_withdrawal_revoke_runtime_custody(self):
        directory = self.root / 'capture'; directory.mkdir()
        for name in ('result.json', 'review.json'): (directory / name).write_text('{}')
        selection = {'directory': str(directory), 'result_sha256': r.admission.sha(directory/'result.json'),
                     'review_sha256': r.admission.sha(directory/'review.json')}
        with patch.object(r.compiler_capture, 'QUAL', self.root):
            r.compiler_capture.selection_paths(selection)
            path = directory / 'withdrawal.json'; path.write_text('{}')
            with self.assertRaisesRegex(ValueError, 'withdrawn'): r.compiler_capture.selection_paths(selection)
            path.unlink(); path.symlink_to(self.root/'missing')
            with self.assertRaisesRegex(ValueError, 'withdrawn'): r.compiler_capture.selection_paths(selection)

    def test_current_compiler_snapshot_must_match_captured_tools(self):
        old = self.root / 'captured-python'; old.write_bytes(b'DATA old Python')
        new = self.root / 'current-python'; new.write_bytes(b'DATA current Python')
        rustc = self.root / 'rustc'; rustc.write_bytes(b'DATA rustc')
        captured = {'files': {}, 'compiler': {'rustc': {'path': str(rustc), 'sha256': r.admission.sha(rustc)},
                    'python_runtime': {'path': str(old), 'sha256': r.admission.sha(old)}}}
        r.compiler_capture.check_tools(captured, copy.deepcopy(captured))
        current = copy.deepcopy(captured)
        current['compiler']['python_runtime'] = {'path': str(new), 'sha256': r.admission.sha(new)}
        with self.assertRaisesRegex(ValueError, 'compiler tools'): r.compiler_capture.check_tools(captured, current)
        changed = copy.deepcopy(captured); changed['compiler']['rustc']['sha256'] = OTHER
        with self.assertRaisesRegex(ValueError, 'compiler tools'): r.compiler_capture.check_tools(captured, changed)
        old.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'retained compiler original changed'): r.compiler_capture.check_tools(captured, captured)


if __name__ == '__main__':
    unittest.main()
