"""Current controller protocol/recovery checks using only fake native owners.

These checks never build, prove, start validators, sign or move funds.
"""
import copy
import hashlib
import importlib
import json
import os
from pathlib import Path
import py_compile
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch
import protocol as p
import run as r
import sealed_data

A, B, C, D = ('a' * 64, 'b' * 64, 'c' * 64, 'd' * 64)
PINS = {key: A for key in r.SOURCE_PINS}
REFERENCE = {'path': '/selected/original', 'sha256': A}


def ref(value):
    return {'path': '/fake/' + value.get('schema', 'data'),
            'sha256': hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()}


class FakeOwner:
    """Closed phase-order fake with no native or monetary authority."""
    def __init__(self):
        self.order = []
        self.previous = B
        self.setup = A
        self.wire = {'path': '/fake/wire', 'bytes': 29, 'sha256': D}
        self.registration = {'certified_height': 9}
        self.registration_ref = {'path': '/fake/registered', 'sha256': C}
        self.retained = False
        self.target = {'path': '/fake/target', 'sha256': B}
        self.retention = {'path': '/fake/retention', 'sha256': A}
        self.prepared = None
        self.funding = None

    def publish(self, phase, previous, **inputs):
        # Emulates current native closed grammar plus wait_command previous join.
        command = p.command(phase, self.setup, previous, inputs, lambda item: r.require(
            type(item) is dict and set(item) == {'path', 'sha256'}, 'fake exact reference'))
        expected = ['registration', 'issuer', 'funding', 'activation-retained', 'settlement', 'confirmed'][len(self.order)]
        if phase != expected or previous != self.previous:
            raise ValueError('native owner waits for ' + expected + ' and exact previous receipt')
        self.order.append(phase)
        selected = ref(command)
        if phase == 'registration':
            self.previous = self.registration_ref['sha256']
        elif phase == 'issuer':
            self.issuer_ref = {'path': '/fake/issuer', 'sha256': D}
            self.previous = D
        elif phase == 'funding':
            value = {'schema': 'iroha.kagemusha.network-activation-prepared.v1', 'setup_sha256': A,
                     'registration_sha256': C, 'funding_command_sha256': selected['sha256'],
                     'target_sha256': B, 'transaction_hash_hex': C,
                     'signed_transaction': {'name': 'funding/activate-signed.wire', 'bytes': 29, 'sha256': D}}
            self.prepared = (ref(value), value)
            self.previous = self.prepared[0]['sha256']
        elif phase == 'activation-retained':
            if not self.retained or inputs['retention'] != self.retention:
                raise ValueError('activation not retained before submit')
            value = {'schema': 'iroha.kagemusha.network-funded-load.v1', 'setup_sha256': A,
                     'registration_sha256': C, 'target_sha256': B, 'source_pins': PINS,
                     'registration_height': 9, 'activation_height': 10, 'certified_height': 11,
                     'activation_retention': {'prepared_sha256': self.prepared[0]['sha256'],
                        'acknowledgment_sha256': selected['sha256'], 'retention_sha256': A, 'signed_wire_sha256': D}}
            self.funding = (ref(value), value)
            self.previous = self.funding[0]['sha256']
        elif phase == 'settlement':
            self.previous = C
        return selected

    def wait(self, name):
        if name == 'activation-prepared.json' and self.order[-1] == 'funding':
            return self.prepared
        if name == 'funded.json' and self.order[-1] == 'activation-retained':
            return self.funding
        raise ValueError('fake owner has no completed phase ' + name)

    def retain(self, prepared, wire):
        if prepared != self.prepared[0] or wire != self.wire:
            raise ValueError('different native custody original')
        self.retained = True
        return self.retention

    def register_and_select_issuer(self):
        self.publish('registration', B, scheme_id=A, manifest_digest=B, verifier_pack=REFERENCE)
        self.publish('issuer', C, peer_index=1, configuration=REFERENCE)

    def run_funding(self, **override):
        args = dict(setup=A, registration_ref=self.registration_ref,
            registration_value=self.registration, issuer_ref=self.issuer_ref, target=self.target,
            pins=PINS, publish=self.publish, wait=self.wait, observe_wire=lambda: self.wire, retain=self.retain)
        args.update(override)
        return p.funding_handshake(**args)


class CurrentProtocolTests(unittest.TestCase):
    def tearDown(self):
        r._UNFINISHED_CHILDREN.clear()

    def test_current_six_stage_sequence_with_nested_retention(self):
        owner = FakeOwner(); owner.register_and_select_issuer()
        result = owner.run_funding()
        self.assertEqual(result[1]['certified_height'], 11)
        owner.publish('settlement', result[0]['sha256'], exchange=REFERENCE, target=REFERENCE,
                      verifier_pack=REFERENCE, adversarial={'kind': 'software_bad_sigma', 'claim': REFERENCE})
        owner.publish('confirmed', C, confirmation=REFERENCE)
        self.assertEqual(owner.order, list(p.PHASE_FIELDS))
        self.assertTrue(owner.retained)

    def test_retired_direct_funding_protocol_refused(self):
        owner = FakeOwner()
        with self.assertRaisesRegex(ValueError, 'waits for registration'):
            owner.publish('funding', B, target=REFERENCE)
        self.assertEqual(owner.order, [])
        with self.assertRaisesRegex(ValueError, 'exact funding'):
            p.command('funding', A, B, {'target': REFERENCE, 'verifier_pack': REFERENCE}, lambda _item: None)

    def test_every_closed_phase_rejects_missing_extra_fields(self):
        cases = {
            'registration': dict(scheme_id=A, manifest_digest=B, verifier_pack=REFERENCE),
            'issuer': dict(peer_index=0, configuration=REFERENCE), 'funding': dict(target=REFERENCE),
            'activation-retained': dict(retention=REFERENCE),
            'settlement': dict(exchange=REFERENCE, target=REFERENCE, verifier_pack=REFERENCE,
                               adversarial={'kind': 'software_bad_sigma', 'claim': REFERENCE}),
            'confirmed': dict(confirmation=REFERENCE)}
        for phase, fields in cases.items():
            with self.subTest(phase=phase):
                p.command(phase, A, B, fields, lambda _item: None)
                for mutation in [dict(fields, unknown=True), dict(list(fields.items())[1:])]:
                    with self.assertRaises(ValueError):
                        p.command(phase, A, B, mutation, lambda _item: None)

    def test_retention_failure_never_acknowledges_or_advances(self):
        owner = FakeOwner(); owner.register_and_select_issuer()
        with self.assertRaisesRegex(OSError, 'simulated storage failure'):
            owner.run_funding(retain=Mock(side_effect=OSError('simulated storage failure')))
        self.assertEqual(owner.order, ['registration', 'issuer', 'funding'])
        self.assertIsNone(owner.funding)
        self.assertFalse(owner.retained)

    def test_foreign_preparation_or_wire_never_calls_native_retention(self):
        for field in ['setup_sha256', 'registration_sha256', 'funding_command_sha256', 'target_sha256']:
            with self.subTest(field=field):
                owner = FakeOwner(); owner.register_and_select_issuer()
                def altered(name):
                    selected, value = owner.wait(name)
                    value = copy.deepcopy(value); value[field] = 'e' * 64
                    return selected, value
                retain = Mock()
                with self.assertRaisesRegex(ValueError, 'another occurrence'):
                    owner.run_funding(wait=altered, retain=retain)
                retain.assert_not_called()
        owner = FakeOwner(); owner.register_and_select_issuer(); retain = Mock()
        with self.assertRaisesRegex(ValueError, 'wire differs'):
            owner.run_funding(observe_wire=lambda: dict(owner.wire, sha256=A), retain=retain)
        retain.assert_not_called()

    def test_foreign_funded_receipt_and_skipped_height_refused(self):
        for field in ['registration_sha256', 'target_sha256', 'source_pins', 'activation_height', 'certified_height']:
            with self.subTest(field=field):
                owner = FakeOwner(); owner.register_and_select_issuer()
                def altered(name):
                    selected, value = owner.wait(name)
                    if name == 'funded.json':
                        value = copy.deepcopy(value); value[field] = None
                    return selected, value
                with self.assertRaises(ValueError):
                    owner.run_funding(wait=altered)
                self.assertEqual(owner.order[-1], 'activation-retained')

    def test_issuer_handoff_requires_exact_current_registration_and_private_reference(self):
        value = dict(schema='iroha.kagemusha.network-controller-issuer-selection.v1', setup_sha256=A,
                     registered_sha256=B, peer_index=2, configuration=REFERENCE)
        selected = Mock()
        self.assertEqual(p.issuer_selection(value, A, B, selected), dict(peer_index=2, configuration=REFERENCE))
        selected.assert_called_once_with(REFERENCE)
        for mutation in [dict(value, setup_sha256=C), dict(value, registered_sha256=C),
                         dict(value, peer_index=4), dict(value, peer_index=True), dict(value, secret='never accepted')]:
            with self.assertRaises(ValueError):
                p.issuer_selection(mutation, A, B, selected)

    def test_historical_retained_input_selection_refused_before_capture(self):
        with patch.object(r.native_capture, 'admit', side_effect=AssertionError('no capture reads')):
            with self.assertRaisesRegex(ValueError, 'campaign selection grammar'):
                r.compile_selection({'schema': r.SCHEMA, 'captures': {}, 'retained_network_inputs': REFERENCE})

    def test_exact_issuer_restart_multiset(self):
        rows = {'a': [1,2,3], 'b': [1,2,3,4], 'c': [1,2,3], 'd': [1,2,3]}
        self.assertTrue(p.peer_restarts(rows))
        for mutation in [dict(rows, b=[1,2,3]), dict(rows, a=[1,2,3,4]), dict(rows, b=[1,3,4,5]),
                         dict(rows, extra=[1,2]), {key:value for key,value in rows.items() if key != 'a'}]:
            self.assertFalse(p.peer_restarts(mutation))

    def test_current_native_namespace_and_retention_selector(self):
        self.assertEqual(r.NATIVE, 'kagemusha_wallet_advance_v1::enrollment_owner_tests::native_exchange::')
        self.assertEqual(r.RETENTION, r.NATIVE + 'load::activation_retention::retain_actual_a_activation_before_network_submission')
        self.assertTrue(all(selector.startswith(r.NATIVE) for selector in [r.TARGET,r.RETENTION,r.LOAD,r.EXCHANGE,r.CONFIRM]))

    def test_nocapture_exact_test_count_and_selector_remain_required(self):
        summary = '\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1s\n'
        for body in ['test selected ... ok\n', 'test selected ... NATIVE_RECORD data\nmore output\nok\n']:
            text = 'running 1 test\n' + body + summary
            self.assertTrue(r.exact_result('selected', text, 0))
            for bad in [text.replace('running 1 test','running 0 tests'), text + summary,
                        text.replace('test selected ...','test foreign ...'), text.replace('ok\n','FAILED\n')]:
                self.assertFalse(r.exact_result('selected', bad, 0))
            self.assertFalse(r.exact_result('selected', text, 1))

    def test_timeout_preserves_pid_without_any_signal_or_retry(self):
        child = Mock(pid=431); child.wait.side_effect = subprocess.TimeoutExpired('fake', 0)
        with patch.object(r.time, 'monotonic', return_value=2), patch.object(r, 'save') as save:
            with self.assertRaises(subprocess.TimeoutExpired):
                r.await_natural_exit(child, Path('/fake'), 1)
            record = save.call_args.args[1]
            self.assertEqual(record['pid'], 431)
            self.assertFalse(record['signals_permitted'])
            self.assertFalse(record['automatic_retry_permitted'])
        self.assertEqual(child.method_calls, [unittest.mock.call.wait(timeout=0)])

    def test_timeout_survives_failed_receipt_write(self):
        child = Mock(pid=432); child.wait.side_effect = subprocess.TimeoutExpired('fake', 0)
        with patch.object(r.time, 'monotonic', return_value=2), patch.object(r, 'save', side_effect=OSError('full')):
            with self.assertRaises(subprocess.TimeoutExpired):
                r.await_natural_exit(child, Path('/fake'), 1)
        child.terminate.assert_not_called(); child.kill.assert_not_called()

    def test_linux_clone_uses_one_ficlone_no_fallback(self):
        for failure in [None, OSError('unsupported')]:
            with self.subTest(failure=failure), patch.object(sealed_data.sys, 'platform', 'linux'), \
                    patch.object(sealed_data.os, 'open', return_value=17) as opened, \
                    patch.object(sealed_data.os, 'close') as closed, \
                    patch.object(sealed_data.fcntl, 'ioctl', side_effect=failure) as ioctl:
                clone = sealed_data.clone_function()
                if failure:
                    with self.assertRaises(OSError): clone(5, 6, A)
                else:
                    clone(5, 6, A)
                ioctl.assert_called_once_with(17, 0x40049409, 5)
                closed.assert_called_once_with(17)
                self.assertEqual(opened.call_count, 1)
                flags = opened.call_args.args[1]
                self.assertTrue(flags & os.O_EXCL); self.assertTrue(flags & os.O_NOFOLLOW)

    def test_private_runtime_selection_rejects_readable_alias_and_links(self):
        with tempfile.TemporaryDirectory(dir=r.QUAL, prefix='controller-private-test-') as tmp:
            root=Path(tmp); root.chmod(0o700); original=root/'private'; original.write_bytes(b'opaque'); original.chmod(0o600)
            selected=r.ref(r.observe(original)); self.assertEqual(r.private_selected(selected), original)
            for mode in [0o644, 0o640]:
                original.chmod(mode)
                with self.assertRaisesRegex(ValueError, 'owner-only'): r.private_selected(selected)
            original.chmod(0o600); os.link(original, root/'hardlink')
            with self.assertRaisesRegex(ValueError, 'owner-only'): r.private_selected(selected)
            (root/'hardlink').unlink(); alias=root/'alias'; alias.symlink_to(original)
            with self.assertRaises(ValueError): r.private_selected(dict(selected, path=str(alias)))

    def test_authenticated_import_ignores_stale_valid_bytecode(self):
        with tempfile.TemporaryDirectory(dir=r.QUAL, prefix='controller-import-test-') as tmp:
            root=Path(tmp); source=root/'selected_controller_fake.py'
            source.write_text('VALUE = "bad"\n'); stamp=source.stat().st_mtime
            py_compile.compile(str(source), doraise=True)
            source.write_text('VALUE = "yes"\n'); os.utime(source, (stamp, stamp))
            digest=hashlib.sha256(source.read_bytes()).hexdigest()
            finder=r.install_checked_source_imports({str(source):digest}); sys.path.insert(0,str(root))
            try:
                loaded=importlib.import_module('selected_controller_fake')
                self.assertEqual(loaded.VALUE, 'yes')
            finally:
                sys.meta_path.remove(finder); sys.path.remove(str(root)); sys.modules.pop('selected_controller_fake', None)


if __name__ == '__main__':
    unittest.main()
