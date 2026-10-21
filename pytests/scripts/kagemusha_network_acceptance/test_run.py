"""Controller refusal and handoff tests; no network, proof, wallet or cargo process."""
from pathlib import Path
import copy
import hashlib
import json
import os
import tempfile
import unittest
from unittest.mock import patch, Mock
import run as r

PIN = 'a' * 64
OTHER = 'b' * 64
OK = 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 0.01s\n'


class CampaignTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='network-controller-test-', dir=r.QUAL)
        self.root = Path(self.temp.name)
        (self.root / '.git').mkdir()
        checkout = patch.object(r.admission.capture, 'REPO', self.root)
        checkout.start(); self.addCleanup(checkout.stop)

    def tearDown(self):
        self.temp.cleanup()

    def test_command_is_durable_exact_and_cannot_overwrite(self):
        target = r.save(self.root / 'target.json', {'selected': True})
        receipt = r.phase_command(self.root, 'funding', PIN, OTHER, target=r.ref(target))
        value = r.document(r.ref(receipt))
        self.assertEqual(value['previous_sha256'], OTHER)
        self.assertEqual(value['setup_sha256'], PIN)
        self.assertEqual(value['target'], r.ref(target))
        self.assertEqual((self.root / 'funding-command.json').stat().st_mode & 0o777, 0o400)
        self.assertFalse((self.root / '.funding-command.json.pending').exists())
        with self.assertRaisesRegex(ValueError, 'already exists'):
            r.phase_command(self.root, 'funding', PIN, OTHER, target=r.ref(target))
        self.assertEqual(r.observe(self.root / 'funding-command.json'), receipt)

    def test_changed_selected_input_never_publishes_command(self):
        target = r.save(self.root / 'target.json', {'selected': True})
        with self.assertRaisesRegex(ValueError, 'selected original changed'):
            r.phase_command(self.root, 'funding', PIN, OTHER, target=dict(path=target['path'], sha256=OTHER))
        self.assertFalse((self.root / 'settlement-command.json').exists())

    def test_missing_unknown_duplicate_and_linked_input_refused(self):
        with self.assertRaisesRegex(ValueError, 'unknown monetary phase'):
            r.phase_command(self.root, 'recreate-wallet', PIN, OTHER)
        p = self.root / 'duplicate.json'; p.write_text('{"x":1,"x":2}')
        with self.assertRaisesRegex(ValueError, 'duplicate JSON field'):
            r.read_document(p)
        link = self.root / 'linked.json'; link.symlink_to(p)
        with self.assertRaisesRegex(ValueError, 'without aliases'):
            r.observe(link)
        sparse = self.root / 'oversized'; sparse.touch()
        with sparse.open('r+b') as stream:
            stream.truncate(1 << 30)
        with self.assertRaisesRegex(ValueError, 'shape/size'):
            r.observe(sparse, 16384)

    def test_incomplete_adversarial_case_does_not_pass_campaign(self):
        result = r.final_verdict(0, OK, {'adversarial_qualified': False}, {'adversarial_qualified': False}, True, True, [])
        self.assertTrue(result['positive_network_campaign_completed'])
        self.assertFalse(result['accepted_engineering_campaign'])
        self.assertFalse(result['release_qualified'])
        self.assertFalse(result['physical_device_qualified'])

    def test_every_runtime_qualification_condition_is_required(self):
        complete = {'adversarial_qualified': True, 'native_proof_rejection_qualified': True, 'account_binding_model_refusal': True, 'account_specific_network_rejection_qualified': False}
        self.assertTrue(r.final_verdict(0, OK, complete, complete, True, True, [])['accepted_engineering_campaign'])
        for code, log, source, peers, live in [
            (1, OK, True, True, []), (0, OK + OK, True, True, []),
            (0, OK, False, True, []), (0, OK, True, False, []), (0, OK, True, True, ['still live']),
        ]:
            self.assertFalse(r.final_verdict(code, log, complete, complete, source, peers, live)['accepted_engineering_campaign'])

    def test_wait_refuses_dead_owner_without_restarting(self):
        child = Mock(); child.poll.return_value = 1
        with self.assertRaisesRegex(ValueError, 'same live network owner exited'):
            r.wait_document(self.root / 'funded.json', child, float('inf'))
        self.assertEqual(list(self.root.iterdir()), [self.root / '.git'])
        self.assertEqual(child.method_calls, [unittest.mock.call.poll()])

    def test_current_source_drift_and_binary_changes_refused(self):
        binary = self.root / 'binary'; binary.write_bytes(b'executable')
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        admitted = {'source': {}, 'retained': {}, 'binaries': {'selected': {'path': str(binary), 'sha256': digest}}}
        responses = {('branch', '--show-current'): 'optimizations',
                     ('diff', '--name-only', '--diff-filter=U'): ''}
        with patch.object(r.admission.capture, 'git', side_effect=lambda *args: responses[args]):
            r.unchanged(admitted, {})
            admitted['source'] = {'crates/example.rs': OTHER}
            with patch.object(r.admission, 'sha', return_value=PIN):
                with self.assertRaisesRegex(ValueError, 'current consumed source differs'):
                    r.unchanged(admitted, {})
            admitted['source'] = {}; binary.write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'captured executable changed'):
                r.unchanged(admitted, {})

    def test_branch_or_conflicts_refused_before_source_and_binary_checks(self):
        admitted = {'source': {'crates/unread.rs': PIN}, 'retained': {},
                    'binaries': {'selected': {'path': '/never/read', 'sha256': PIN}}}
        for branch, conflicts in [('other-branch', ''), ('optimizations', 'conflicted.rs')]:
            with self.subTest(branch=branch, conflicts=conflicts):
                responses = {('branch', '--show-current'): branch,
                             ('diff', '--name-only', '--diff-filter=U'): conflicts}
                with patch.object(r.admission.capture, 'git', side_effect=lambda *args: responses[args]), \
                     patch.object(r.admission, 'sha') as sha:
                    with self.assertRaisesRegex(ValueError, 'original resolved branch required'):
                        r.unchanged(admitted, {})
                    sha.assert_not_called()

    def write_case(self, value, field, case):
        path = self.root / (case['name'] + '-observation.json')
        path.write_text(json.dumps(case))
        item = r.observe(path)
        value[field] = {'name': case['name'], 'transaction_hash_hex': case['transaction_hash_hex'],
                        'qualified': case['adversarial_input_rejection_qualified'],
                        'observation_original': {'name': path.name, 'bytes': item['bytes'], 'sha256': item['sha256']}}

    def read_case(self, value, field):
        return json.loads((self.root / value[field]['observation_original']['name']).read_text())

    def negative_fixture(self, complete=True, preparation=False):
        value = {'last_certified_height': 9, 'adversarial_qualified': complete,
                 'native_proof_rejection_qualified': complete, 'account_binding_model_refusal': True,
                 'account_specific_network_rejection_qualified': complete and not preparation}
        for name, field in [('foreign-account', 'foreign_account_observation'), ('bad-sigma', 'altered_proof_observation')]:
            early = preparation and name == 'foreign-account'; qualified = complete and not early
            names = {f'{name}-signed.norito', f'{name}-expected-reason.norito', f'{name}-submission-error.txt'} | {f'{name}-status-{i}.norito' for i in range(4)}
            if qualified:
                names |= {f'{name}-committed.norito', f'{name}-reason.norito'}
            if early:
                names = {f'{name}-instruction.norito', f'{name}-expected-reason.norito', f'{name}-preparation-error.txt'}
            rows = []
            for name_file in sorted(names):
                path = self.root / name_file; path.write_bytes(name_file.encode()); item = r.observe(path)
                rows.append({'name': name_file, 'bytes': item['bytes'], 'sha256': item['sha256']})
            case = {'name': name, 'phase': 'preparation' if early else 'submitted',
                'transaction_hash_hex': None if early else PIN, 'committed_failure_height': 8 if qualified else None,
                'adversarial_input_rejection_qualified': qualified, 'authoritative_rejection_observed': qualified, 'originals': rows,
                'peer_statuses': [] if early else [{'peer_id': str(i), 'state_rejection_height': 8 if qualified else None,
                    'response': {'hash': PIN, 'scope': 'global', 'resolved_from': 'state' if qualified else 'cache',
                                 'status': {'kind': 'Rejected', 'block_height': 8}}} for i in range(4)]}
            self.write_case(value, field, case)
        return value

    def test_negative_originals_require_exact_inventory_and_rehash(self):
        value = self.negative_fixture()
        original = r.negative_originals(self.root, value)
        self.assertEqual(len(original), 20)
        pristine = self.read_case(value, 'foreign_account_observation')
        for mode in ['missing', 'duplicate', 'extra', 'foreign', 'oversized']:
            changed = copy.deepcopy(value); case = copy.deepcopy(pristine); rows = case['originals']
            if mode == 'missing': rows.pop()
            elif mode == 'duplicate': rows[1] = rows[0]
            elif mode == 'extra': rows.append(rows[0])
            elif mode == 'foreign': rows[0]['name'] = '../foreign'
            else: rows[0]['bytes'] = (16 << 20) + 1
            self.write_case(changed, 'foreign_account_observation', case)
            with self.assertRaises(ValueError): r.negative_originals(self.root, changed)
        self.write_case(value, 'foreign_account_observation', pristine)
        (self.root / 'foreign-account-reason.norito').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'hash/extent'):
            r.negative_originals(self.root, value)

    def test_negative_authority_rejects_cache_upgrade_and_selector_mutations(self):
        value = self.negative_fixture(False)
        self.assertEqual(len(r.negative_originals(self.root, value)), 16)
        changed = copy.deepcopy(value); changed['adversarial_qualified'] = True
        with self.assertRaises(ValueError): r.negative_originals(self.root, changed)
        pristine = self.read_case(value, 'foreign_account_observation')
        for field, replacement in [('hash', OTHER), ('scope', 'local'), ('resolved_from', 'publisher')]:
            changed = copy.deepcopy(value); case = copy.deepcopy(pristine)
            case['peer_statuses'][0]['response'][field] = replacement
            self.write_case(changed, 'foreign_account_observation', case)
            with self.assertRaises(ValueError): r.negative_originals(self.root, changed)
        changed = copy.deepcopy(value); case = copy.deepcopy(pristine)
        case['peer_statuses'][0]['state_rejection_height'] = 8
        self.write_case(changed, 'foreign_account_observation', case)
        with self.assertRaises(ValueError): r.negative_originals(self.root, changed)
        changed = self.negative_fixture(); case = self.read_case(changed, 'foreign_account_observation')
        case['peer_statuses'][0]['response']['status']['block_height'] = 7
        self.write_case(changed, 'foreign_account_observation', case)
        with self.assertRaises(ValueError): r.negative_originals(self.root, changed)

    def test_preparation_is_inconclusive_without_blocking_certified_proof_gate(self):
        value = self.negative_fixture(preparation=True)
        self.assertEqual(len(r.negative_originals(self.root, value)), 14)
        verdict = r.final_verdict(0, OK, {'adversarial_qualified': True}, value, True, True, [])
        self.assertTrue(verdict['accepted_engineering_campaign'])
        self.assertTrue(verdict['native_proof_rejection_qualified'])
        self.assertTrue(verdict['account_binding_model_refusal'])
        self.assertFalse(verdict['account_specific_network_rejection_qualified'])
        self.assertFalse(verdict['all_named_network_rejections_qualified'])
        pristine = self.read_case(value, 'foreign_account_observation')
        for field, replacement in [('transaction_hash_hex', PIN), ('committed_failure_height', 8),
                                   ('peer_statuses', [{'invented': True}]), ('authoritative_rejection_observed', True)]:
            changed = copy.deepcopy(value); case = copy.deepcopy(pristine); case[field] = replacement
            self.write_case(changed, 'foreign_account_observation', case)
            with self.assertRaises(ValueError): r.negative_originals(self.root, changed)
        changed = copy.deepcopy(value); case = self.read_case(value, 'altered_proof_observation'); case['phase'] = 'preparation'
        self.write_case(changed, 'altered_proof_observation', case)
        with self.assertRaises(ValueError): r.negative_originals(self.root, changed)

    def test_observation_json_and_expected_reason_are_independently_pinned_and_bounded(self):
        value = self.negative_fixture()
        (self.root / 'bad-sigma-observation.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'observation hash/extent'):
            r.negative_originals(self.root, value)
        value = self.negative_fixture()
        (self.root / 'bad-sigma-expected-reason.norito').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'hash/extent'):
            r.negative_originals(self.root, value)
        value = self.negative_fixture(); value['foreign_account_observation']['observation_original']['bytes'] = (16 << 10) + 1
        with self.assertRaisesRegex(ValueError, 'bound/grammar'):
            r.negative_originals(self.root, value)

    def test_native_bad_sigma_original_requires_exact_grammar_and_hash(self):
        path = self.root / 'c-unload-bad-sigma-claim.norito'; path.write_bytes(b'proof-fixture')
        item = r.observe(path); value = {'c_unload_bad_sigma_claim': {'name': path.name, 'bytes': item['bytes'],
            'sha256': item['sha256'], 'mutation': 'sigma_ipa_f_plus_one', 'model_verified': True,
            'native_proof_rejected': True, 'adversarial_fixture_signatures': 1}}
        def check(candidate):
            selected = self.root / 'exchange.json'; selected.write_text(json.dumps(candidate))
            return r.native_negative_records(self.root, r.observe(selected))
        self.assertEqual(check(value), {path.name: item})
        for field, replacement in [('name', '../elsewhere'), ('bytes', (16 << 10) + 1), ('sha256', OTHER),
            ('mutation', 'unbound-byte-flip'), ('model_verified', False), ('native_proof_rejected', False),
            ('adversarial_fixture_signatures', True), ('adversarial_fixture_signatures', 2)]:
            changed = copy.deepcopy(value); changed['c_unload_bad_sigma_claim'][field] = replacement
            with self.assertRaises(ValueError): check(changed)
        path.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'hash/extent'):
            check(value)

    def test_network_settlement_requires_exact_observed_prefix_and_bindings(self):
        env = {key: PIN for key in r.SOURCE_PINS} | {'KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256': PIN, 'KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256': PIN}
        exchange = {'exchange': {'sha256': PIN}, 'claim': {'sha256': PIN}}
        value = {'schema': 'iroha.kagemusha.network-unload-settlement.v1', 'setup_sha256': PIN,
                 'source_pins': {key: PIN for key in r.SOURCE_PINS}, 'exchange_sha256': PIN, 'claim_sha256': PIN,
                 'target_sha256': PIN, 'receipt_sha256': PIN, 'settlement_height': 6, 'last_certified_height': 7,
                 'exact_transaction_retry_returned_original': True, 'originals': []}
        names = {f'native-proof-{i}.norito' for i in range(1, 8)} | {'unload-claim.norito', 'unload-instruction.norito', 'unload-block.norito', 'unload-signed-transaction.norito'}
        for name in sorted(names):
            path = self.root / name; path.write_bytes(name.encode()); item = r.observe(path)
            value['originals'].append({'name': name, 'bytes': item['bytes'], 'sha256': item['sha256']})
        value.update(self.negative_fixture(False))
        value['last_certified_height'] = 7
        path = self.root / 'settlement.json'
        def check(candidate):
            path.write_text(json.dumps(candidate))
            return r.settlement_records(path, env, exchange, {'receipt_sha256': PIN, 'certified_height': 5})
        check(value)
        for key, replacement in [('claim_sha256', OTHER), ('last_certified_height', 8), ('settlement_height', 5)]:
            changed = copy.deepcopy(value); changed[key] = replacement
            with self.assertRaises(ValueError):
                check(changed)
        changed = copy.deepcopy(value); changed['originals'][-1] = changed['originals'][0]
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            check(changed)
        changed = copy.deepcopy(value); changed['originals'].append(changed['originals'][0])
        with self.assertRaisesRegex(ValueError, 'count differs'):
            check(changed)


    def test_owner_and_native_build_commands_must_be_release(self):
        commands = [['cargo', 'test', '--release', '--no-run', '--test', name] for name in ('sumeragi', 'iroha_core_zk')]
        for command in commands:
            self.assertEqual(r.release_command(command), command)
            for flag in ('--release', 'test', '--no-run'):
                changed = command.copy(); changed.remove(flag)
                with self.assertRaises(ValueError): r.release_command(changed)
        for value in [[], commands, {'commands': commands}, {'command': commands[0]},
                      {'commands': commands, 'command': commands[0]}, commands[0] + [None]]:
            with self.assertRaises(ValueError):
                r.release_command(value)

    def test_readiness_failure_can_request_normal_abort_from_original_setup(self):
        live = self.root / 'live'; live.mkdir(); (live / 'setup').mkdir()
        setup = r.save(live / 'setup/setup.json', {'original': 'only a shutdown selector'})
        (live / 'ready.json').write_text('malformed')
        failure = r.save(self.root / 'failure.json', {'accepted': False})
        child = Mock(); child.poll.return_value = None
        abort = r.request_owned_abort(live, None, failure, child)
        self.assertEqual(r.document(r.ref(abort))['setup_sha256'], setup['sha256'])
        self.assertEqual(r.document(r.ref(abort))['failure_sha256'], failure['sha256'])
        self.assertEqual(child.method_calls, [unittest.mock.call.poll()])



    def restart_fixture(self):
        name = 'post-restart-unload-retry-signed.norito'
        path = self.root / name
        path.write_bytes(b'exact retained signed transaction DATA')
        original = r.observe(path)
        completed = {'post_restart_exact_transaction_retry_returned_original': True,
                     'restarted_height': 9, 'post_restart_instruction_retry_height': 10,
                     'post_restart_instruction_retry_hash_hex': OTHER,
                     'post_restart_instruction_retry_original_sha256': original['sha256']}
        return path, original, completed, {'settlement_transaction_hash_hex': PIN}

    def test_post_restart_signed_original_is_joined_for_final_recheck(self):
        path, original, completed, settled = self.restart_fixture()
        retained = r.post_restart_records(self.root, completed, settled)
        self.assertEqual(retained, {path.name: original})
        # Same record shape enters main's existing remember/recheck_artifacts map.
        artifacts = {item['path']: item for item in retained.values()}
        self.assertEqual([r.observe(item['path']) for item in artifacts.values()], [original])
        path.write_bytes(b'late changed transaction')
        self.assertNotEqual(r.observe(original['path']), original)
        with self.assertRaisesRegex(ValueError, 'signed original hash'):
            r.post_restart_records(self.root, completed, settled)

    def test_post_restart_status_height_and_transaction_identity_refusals(self):
        _, _, completed, settled = self.restart_fixture()
        mutants = [('post_restart_exact_transaction_retry_returned_original', x)
                   for x in (False, 1, None)]
        mutants += [('restarted_height', x) for x in (False, 0, -1, 10, '9')]
        mutants += [('post_restart_instruction_retry_height', x)
                    for x in (True, 9, 8, 1 << 64, '10')]
        mutants += [('post_restart_instruction_retry_hash_hex', x)
                    for x in (PIN, '0' * 64, OTHER.upper(), OTHER[:-1], None)]
        mutants += [('post_restart_instruction_retry_original_sha256', x)
                    for x in (PIN, '0' * 64, OTHER.upper(), None)]
        for field, value in mutants:
            with self.subTest(field=field, value=value):
                changed = dict(completed); changed[field] = value
                with self.assertRaises(ValueError):
                    r.post_restart_records(self.root, changed, settled)
        for field in completed:
            changed = dict(completed); del changed[field]
            with self.subTest(missing=field), self.assertRaises(ValueError):
                r.post_restart_records(self.root, changed, settled)
        for value in (None, '0' * 64, PIN.upper(), PIN[:-1]):
            with self.subTest(settlement=value), self.assertRaises(ValueError):
                r.post_restart_records(self.root, completed, {'settlement_transaction_hash_hex': value})

    def test_post_restart_missing_changed_oversized_and_aliased_originals_refuse(self):
        path, _, completed, settled = self.restart_fixture()
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            r.post_restart_records(self.root, completed, settled)
        path.write_bytes(b'changed transaction')
        with self.assertRaisesRegex(ValueError, 'signed original hash'):
            r.post_restart_records(self.root, completed, settled)
        with path.open('r+b') as stream:
            stream.truncate((16 << 20) + 1)
        with self.assertRaisesRegex(ValueError, 'shape/size'):
            r.post_restart_records(self.root, completed, settled)
        path.unlink()
        other = self.root / 'other-original'; other.write_bytes(b'original')
        path.symlink_to(other)
        with self.assertRaisesRegex(ValueError, 'without aliases'):
            r.post_restart_records(self.root, completed, settled)


if __name__ == '__main__':
    unittest.main()
