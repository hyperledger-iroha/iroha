"""Own fake issuer composition through natural completion before consuming output."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

import run as r


class IssuerCompositionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(dir=r.QUAL)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        (self.root / 'live').mkdir(mode=0o700)
        (self.root / 'live/setup').mkdir(mode=0o700)
        (self.root / 'private').mkdir(mode=0o700)
        self.setup = r.save(self.root / 'live/setup/setup.json', {'setup': 'current DATA'})
        self.registered = r.save(self.root / 'live/registered.json', {'registered': 'current DATA'})
        self.handoff = r.save(self.root / 'issuer-request.json', {'issuer': 'current DATA'})
        self.request_path = self.root / 'private/request.json'
        self.request_data = {'issuer_request': r.ref(self.handoff), 'setup': r.ref(self.setup),
                             'registered': r.ref(self.registered),
                             'private_body': 'OPAQUE-REQUEST-BODY-NOT-A-RECEIPT'}
        self.request = r.save(self.request_path, self.request_data)
        self.input_path = self.root / 'issuer-composer-input.json'
        self.input_value = {'schema': 'iroha.kagemusha.network-controller-issuer-composer-input.v1',
                            'issuer_request': r.ref(self.handoff), 'request': r.ref(self.request)}
        r.save(self.input_path, self.input_value)
        self.configuration = self.root / 'private/issuer.toml'
        self.configuration.write_text('opaque configuration DATA\n')
        self.configuration.chmod(0o400)
        self.selection_path = self.root / 'issuer-selection.json'
        self.owner = Mock(pid=3000)
        self.owner.poll.return_value = None
        self.binary = {'path': '/inert/selected-sumeragi', 'sha256': 'a' * 64}
        self.admitted = {'binaries': {'sumeragi': self.binary}, 'retained': {}}
        self.env = {'TMPDIR': str(self.root)}
        self.calls, self.children, self.events = [], [], []
        self.exit_code = 0
        self.publish = True
        self.uncertain = False
        self.source_drift = False
        self.on_completion = None

    def tearDown(self):
        r._UNFINISHED_CHILDREN.clear()

    def replace_json(self, path, value):
        path.chmod(0o600)
        path.write_text(json.dumps(value))
        path.chmod(0o400)
        return r.observe(path)

    def select_request(self, value):
        selected = self.replace_json(self.request_path, value)
        self.replace_json(self.input_path, self.input_value | {'request': r.ref(selected)})

    def publish_selection(self):
        return r.save(self.selection_path, {
            'schema': 'iroha.kagemusha.network-controller-issuer-selection.v1',
            'setup_sha256': self.setup['sha256'], 'registered_sha256': self.registered['sha256'],
            'peer_index': 1, 'configuration': r.ref(r.observe(self.configuration))})

    def spawn(self, command, **kwargs):
        self.calls.append(command)
        self.assertEqual(command[0], self.binary['path'])
        self.assertEqual(command[1], r.ISSUER_COMPOSER)
        child = Mock(pid=4000 + len(self.calls))
        self.children.append(child)
        if '--list' in command:
            kwargs['stdout'].write(command[1] + ': test\n\n1 test, 0 benchmarks\n')
            child.wait.return_value = 0
            return child
        self.assertEqual(command[2:], ['--exact', '--ignored', '--nocapture', '--test-threads=1'])
        self.assertEqual(kwargs['env'], self.env | {
            'KAGEMUSHA_ENGINEERING_ISSUER_REQUEST': str(self.request_path),
            'KAGEMUSHA_ENGINEERING_ISSUER_REQUEST_SHA256': r.observe(self.request_path)['sha256']})
        self.events.append('composer-started')
        if self.publish:
            self.publish_selection()
            self.events.append('selection-visible')
        outcome = 'ok' if self.exit_code == 0 else 'FAILED'
        count = '1 passed; 0 failed' if self.exit_code == 0 else '0 passed; 1 failed'
        kwargs['stdout'].write(f'running 1 test\ntest {r.ISSUER_COMPOSER} ... {outcome}\n\n'
                              f'test result: {outcome}. {count}; 0 ignored; 0 measured; 10 filtered out; finished in 0.01s\n')

        def complete(**_kwargs):
            if self.on_completion is not None:
                self.on_completion()
            if self.uncertain:
                raise subprocess.TimeoutExpired(command, 1)
            self.events.append('composer-exited')
            return self.exit_code
        child.wait.side_effect = complete
        return child

    def unchanged(self, admitted, selected_tools):
        self.events.append('inputs-checked')
        if self.source_drift and 'composer-exited' in self.events:
            raise ValueError('captured source changed')
        for name, expected in admitted['retained'].items():
            if hashlib.sha256(Path(name).read_bytes()).hexdigest() != expected:
                raise ValueError('retained input changed')

    def compose(self):
        original = r.protocol.issuer_selection
        def consume(*args):
            self.events.append('selection-consumed')
            return original(*args)
        with patch.object(r.subprocess, 'Popen', side_effect=self.spawn), \
             patch.object(r, 'unchanged', side_effect=self.unchanged), \
             patch.object(r.protocol, 'issuer_selection', side_effect=consume):
            return r.compose_issuer(self.root, self.handoff, r.ref(self.setup), self.registered,
                                    self.owner, float('inf'), self.env, self.admitted, {})

    def assert_no_output_consumption(self):
        self.assertNotIn('selection-consumed', self.events)
        self.assertFalse((self.root / 'live/issuer-command.json').exists())
        for child in self.children:
            child.kill.assert_not_called()
            child.terminate.assert_not_called()

    def test_success_retains_current_request_and_natural_completion_before_selection(self):
        inputs, records = self.compose()
        self.assertEqual(inputs, {'peer_index': 1, 'configuration': r.ref(r.observe(self.configuration))})
        self.assertEqual(len(self.calls), 3)
        self.assertLess(self.events.index('composer-exited'), self.events.index('selection-consumed'))
        self.assertEqual(records['issuer-provisioning-request'], self.request)
        self.assertEqual(records['issuer-composer-input'], r.observe(self.input_path))
        self.assertEqual(records['issuer-request'], self.handoff)
        self.assertEqual(self.admitted['retained'][str(self.request_path)], self.request['sha256'])
        directory = self.root / 'issuer-composition'
        for name in ('started.json', 'process.json', 'exit.json', 'result.json'):
            self.assertEqual(records['issuer-composition-' + name], r.observe(directory / name))
        result = r.read_document(directory / 'result.json')
        self.assertTrue(result['accepted_component'])
        for path in directory.iterdir():
            self.assertEqual(self.admitted['retained'][str(path)], r.component.log_pin(path))
            self.assertNotIn('OPAQUE-REQUEST-BODY-NOT-A-RECEIPT', path.read_text())
        self.assertEqual(r._UNFINISHED_CHILDREN, [])

    def test_visible_selection_with_failed_exit_is_retained_but_never_consumed(self):
        self.exit_code = 1
        with self.assertRaisesRegex(ValueError, 'producer failed or source drifted'):
            self.compose()
        self.assertTrue(self.selection_path.is_file())
        self.assert_no_output_consumption()
        self.assertFalse(r.read_document(self.root / 'issuer-composition/result.json')['accepted_component'])
        self.assertEqual(r._UNFINISHED_CHILDREN, [])

    def test_visible_selection_with_uncertain_completion_stops_later_queries(self):
        self.uncertain = True
        with self.assertRaises(subprocess.TimeoutExpired):
            self.compose()
        self.assertTrue(self.selection_path.is_file())
        self.assert_no_output_consumption()
        self.assertEqual(self.events.count('inputs-checked'), 1)
        self.assertEqual(r._UNFINISHED_CHILDREN[-1]['pid'], self.children[-1].pid)
        self.assertFalse((self.root / 'issuer-composition/result.json').exists())

    def test_every_request_original_requires_exact_current_path_and_hash_before_child(self):
        for name in ('issuer_request', 'setup', 'registered'):
            for field, replacement in [('path', str(self.root / 'other-original')), ('sha256', 'b' * 64)]:
                with self.subTest(name=name, field=field):
                    value = copy.deepcopy(self.request_data)
                    value[name][field] = replacement
                    self.select_request(value)
                    with self.assertRaisesRegex(ValueError, 'three exact current controller originals'):
                        self.compose()
        self.assertEqual(self.calls, [])
        self.assert_no_output_consumption()

    def test_closed_input_handoff_refuses_extra_missing_foreign_or_retired_fields(self):
        variants = [self.input_value | {'configuration': {}},
                    {key: value for key, value in self.input_value.items() if key != 'request'},
                    self.input_value | {'schema': 'retired-selection'},
                    self.input_value | {'issuer_request': r.ref(self.registered)},
                    self.input_value | {'request': r.ref(self.request) | {'extra': True}}]
        for value in variants:
            with self.subTest(value=value):
                self.replace_json(self.input_path, value)
                with self.assertRaises(ValueError):
                    self.compose()
        self.assertEqual(self.calls, [])
        self.assert_no_output_consumption()

    def test_request_has_native_size_bound_and_object_shape_before_child(self):
        for value in [self.request_data | {'oversized': 'x' * r.ISSUER_REQUEST_CAP}, ['not an object']]:
            with self.subTest(kind=type(value).__name__):
                self.select_request(value)
                with self.assertRaises(ValueError):
                    self.compose()
        self.assertEqual(self.calls, [])

    def test_stale_request_digest_refuses_before_child(self):
        self.replace_json(self.request_path, self.request_data | {'changed': True})
        with self.assertRaisesRegex(ValueError, 'selected original changed'):
            self.compose()
        self.assertEqual(self.calls, [])

    def test_request_drift_after_publication_refuses_before_output_consumption(self):
        self.on_completion = lambda: self.replace_json(self.request_path, self.request_data | {'changed': True})
        with self.assertRaisesRegex(ValueError, 'producer failed or source drifted'):
            self.compose()
        self.assertTrue(self.selection_path.is_file())
        self.assert_no_output_consumption()

    def test_input_handoff_drift_after_publication_refuses_before_output_consumption(self):
        self.on_completion = lambda: self.replace_json(self.input_path, self.input_value | {'changed': True})
        with self.assertRaisesRegex(ValueError, 'producer failed or source drifted'):
            self.compose()
        self.assert_no_output_consumption()

    def test_source_drift_after_publication_refuses_before_output_consumption(self):
        self.source_drift = True
        with self.assertRaisesRegex(ValueError, 'producer failed or source drifted'):
            self.compose()
        self.assert_no_output_consumption()

    def test_preexisting_and_dangling_selection_refuse_and_preserve_output(self):
        self.selection_path.write_text('retained pre-created DATA')
        with self.assertRaisesRegex(ValueError, 'pre-existing issuer selection'):
            self.compose()
        self.assertEqual(self.selection_path.read_text(), 'retained pre-created DATA')
        self.selection_path.unlink()
        self.selection_path.symlink_to(self.root / 'missing')
        with self.assertRaisesRegex(ValueError, 'pre-existing issuer selection'):
            self.compose()
        self.assertTrue(self.selection_path.is_symlink())
        self.assertEqual(self.calls, [])

    def test_success_without_published_selection_refuses_without_waiting_for_another_writer(self):
        self.publish = False
        with self.assertRaises(FileNotFoundError):
            self.compose()
        self.assertEqual(len(self.calls), 3)
        self.assert_no_output_consumption()

    def test_network_owner_exit_during_composition_refuses_output(self):
        self.on_completion = lambda: setattr(self.owner.poll, 'return_value', 1)
        with self.assertRaisesRegex(ValueError, 'network owner exited during issuer composition'):
            self.compose()
        self.assert_no_output_consumption()

    def test_current_context_drift_refuses_before_native_child(self):
        for item in (self.handoff, self.setup, self.registered):
            path = Path(item['path'])
            original = path.read_bytes()
            with self.subTest(path=path.name):
                self.replace_json(path, {'changed': True})
                with self.assertRaises(ValueError):
                    self.compose()
            path.chmod(0o600)
            path.write_bytes(original)
            path.chmod(0o400)
        self.assertEqual(self.calls, [])


if __name__ == '__main__':
    unittest.main()
