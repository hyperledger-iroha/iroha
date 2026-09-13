"""Semantic composition controls with explicitly synthetic native result inputs.

These tests do not execute Iroha or qualify packet/economic measurements. The
separate owner suites exercise native process refusal and streamed replay.
"""
from __future__ import annotations


import copy
import sys
from pathlib import Path
from unittest import mock
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import private_settlement_session_economics_test as fixture
import private_settlement_session_semantics as semantics

control, economics = fixture.control, fixture.economics


class SemanticControls(unittest.TestCase):
    def setUp(self):
        self.fixture = fixture.EconomicControls(); self.fixture.setUp(); self.addCleanup(self.fixture.tearDown)
        self.addCleanup(self.fixture.doCleanups)
        self.records, self.row, self.prepared = self.fixture.records, self.fixture.row, self.fixture.prepared
        self.request_raw = self.records.read(self.row['request']); self.request = control.decode(self.request_raw)
        self.ready = dict(self.fixture.ready, process_inventory=[])
        self.vector = self.fixture.result()
        self.result = {**{key: self.request[key] for key in ('version', 'protocol', 'request_id',
            'invocation_nonce', 'commit', 'participants')}, 'request_sha256': self.row['request']['sha256'],
            'mandatory_signed_rs16_da_rbc': True, 'authenticated_message_control': True,
            'signed_rs16_da_observations': semantics.runner.minimum_signed_rs16_da_observations(self.request['participants']),
            'process_inventory': [], 'payload': {
                'economic_vector_sha256': self.vector['economic_vector_sha256'],
                'primary_payment_count': self.request['participants'],
                'monetary_movement_count': self.request['participants']+1,
                'successful_leg_applications': self.request['participants'],
                'stages_ms': {stage: (10 if stage == 'end_to_end' else 1.25)
                              for stage in semantics.runner.benchmark_report.REQUIRED_PRIVATE_STAGES},
                'proof_bytes': 100, 'receipt_bytes': 200, 'storage_growth_bytes': 300,
                'finalized_receipt_observed': True, 'each_leg_applied_exactly_once': True,
                'partial_visible_observations': 0, 'partial_spendable_observations': 0}}
        self.owner = semantics.SessionSemantics(self.prepared, records=self.records,
            observations=mock.Mock(images={'worker': mock.Mock()}), packet_utility={'fixture': True},
            cwd=fixture.ROOT, deadline=lambda: 10**18, outer_timeout_ms=60000)

    def terminal(self, outcome=None):
        value = {key: self.result[key] for key in ('version', 'protocol', 'request_id', 'invocation_nonce',
                                                  'request_sha256', 'commit', 'participants')}
        value.update(elapsed_ms=100, outcome=outcome or {'kind': 'succeeded', 'result': self.result})
        raw = semantics.measurement_bytes(value)
        ref = self.records.publish(self.row['output_directory']+'/evidence/benchmark-protocol/rust-result.json', raw)
        return raw, ref

    def synthetic_verification(self):
        ref = self.records.publish(self.fixture.bound['output_path'], control.canonical(self.vector))
        receipt = self.records.publish(self.row['output_directory']+'/fixture-native-execution.json',
                                       control.canonical({'fixture_only': True}))
        return {'reference': ref, 'execution': receipt, 'result': self.vector}

    def drive(self):
        raw, ref = self.terminal()
        window = self.records.publish(self.row['output_directory']+'/measurement-window.json',
                                      control.canonical({'fixture_only': True}))
        verified = self.synthetic_verification()
        self.stack = []
        for patch in (mock.patch.object(economics, 'prepare_verification', return_value=self.fixture.bound),
                      mock.patch.object(economics, 'NativeVectorInvocation'),
                      mock.patch.object(self.owner, '_window', return_value=(
                          {'cpu_time_ns': 1_000_000, 'sampled_peak_rss_bytes': 4096},
                          {'window_ip_bytes': 125, 'counting_unit': 'ipv4_packet_bytes_including_ip_tcp_headers'}))):
            self.stack.append(patch.start()); self.addCleanup(patch.stop)
        self.stack[1].return_value.finish.return_value = verified
        refs = self.owner.materialize_terminal(raw, self.request_raw, native_ref=ref, measurement_window=window,
                                               ready=self.ready, attempt=self.row)
        sample = self.owner.completed[self.row['attempt_id']]['sample']
        sample_ref = self.records.publish(self.row['output_directory']+'/benchmark-sample.json', semantics.measurement_bytes(sample))
        validation = {**self.prepared['identity'], **{key: self.row[key] for key in control.ATTEMPT_FIELDS},
            'passed': True, 'validation_kind': 'accepted', 'response': refs['response'], 'sample': sample_ref}
        bound = {'rust_terminal': raw, 'response': self.records.read(refs['response']),
            'adapter_outcome': self.records.read(refs['adapter_outcome']),
            'sample': semantics.measurement_bytes(sample), 'validation': control.canonical(validation)}
        return bound, window, sample

    def test_native_payload_rejects_changed_economics_partial_state_and_types(self):
        semantics.native_payload(self.result, self.request, self.ready, self.vector)
        for key, value in [('monetary_movement_count', self.request['participants']),
                ('primary_payment_count', True), ('partial_visible_observations', False),
                ('partial_spendable_observations', 1), ('each_leg_applied_exactly_once', 1),
                ('finalized_receipt_observed', False), ('economic_vector_sha256', '9'*64),
                ('proof_bytes', 0), ('receipt_bytes', 0), ('storage_growth_bytes', -1)]:
            with self.subTest(key=key):
                changed = copy.deepcopy(self.result); changed['payload'][key] = value
                with self.assertRaises(control.SessionProtocolError):
                    semantics.native_payload(changed, self.request, self.ready, self.vector)

    def test_native_payload_rejects_stage_and_inventory_substitution(self):
        for mutate in (lambda x: x['payload']['stages_ms'].update(end_to_end=0),
                       lambda x: x['payload']['stages_ms'].update(proof_generation=11),
                       lambda x: x['payload']['stages_ms'].update(proof_generation=True),
                       lambda x: x['payload']['stages_ms'].update(proof_generation=float('nan')),
                       lambda x: x.update(signed_rs16_da_observations=1),
                       lambda x: x.update(mandatory_signed_rs16_da_rbc=False),
                       lambda x: x.update(process_inventory=[{'extra': True}])):
            changed = copy.deepcopy(self.result); mutate(changed)
            with self.assertRaises(control.SessionProtocolError):
                semantics.native_payload(changed, self.request, self.ready, self.vector)

    def test_transparent_profile_requires_zero_proof_and_same_reimbursement(self):
        request = copy.deepcopy(self.request); request['payload']['profile'] = 'transparent_control'
        result = copy.deepcopy(self.result)
        result['payload'].update(proof_bytes=0, stages_ms={'global_finality': 1, 'end_to_end': 2})
        semantics.native_payload(result, request, self.ready, self.vector)
        result['payload']['monetary_movement_count'] -= 1
        with self.assertRaises(control.SessionProtocolError): semantics.native_payload(result, request, self.ready, self.vector)

    def test_failed_attempt_publishes_no_response_or_invented_exit(self):
        raw, ref = self.terminal({'kind': 'failed', 'stage': 'benchmark_worker', 'reason': 'execution_error'})
        with mock.patch.object(economics, 'NativeVectorInvocation') as native:
            result = self.owner.materialize_terminal(raw, self.request_raw, native_ref=ref, measurement_window=None,
                                                     ready=self.ready, attempt=self.row)
            native.assert_not_called()
        outcome = control.decode(self.records.read(result['adapter_outcome']))
        self.assertEqual(outcome['status'], 'failed'); self.assertIsNone(result['response'])
        self.assertNotIn('exit_code', outcome); self.assertFalse(self.owner.completed)

    def test_successful_composition_replays_before_accept_and_has_no_run_coordinate(self):
        bound, window, sample = self.drive()
        self.owner.validate_acceptance(bound, self.row, window)
        self.assertEqual(self.stack[2].call_count, 2)
        self.assertEqual(sample['cpu_seconds'], .001); self.assertEqual(sample['network_bytes'], 125)
        self.assertEqual(sample['throughput_bundles_per_second'], 100)
        self.assertEqual(sample['monetary_movement_count'], self.request['participants']+1)
        self.assertNotIn('run', sample); self.assertNotIn('exit_code', sample)

    def test_forged_sample_or_acceptance_rejects_before_continuation(self):
        bound, window, sample = self.drive()
        for key, value in [('network_bytes', 126), ('cpu_seconds', 0), ('monetary_movement_count', 1),
                           ('session_attempt_index', False), ('run', 0)]:
            with self.subTest(key=key):
                altered = dict(bound, sample=semantics.measurement_bytes(dict(sample, **{key: value})))
                with self.assertRaises(control.SessionProtocolError): self.owner.validate_acceptance(altered, self.row, window)
        validation = control.decode(bound['validation']); validation['passed'] = 1
        with self.assertRaises(control.SessionProtocolError):
            self.owner.validate_acceptance(dict(bound, validation=control.canonical(validation)), self.row, window)

    def test_changed_native_verification_cannot_be_accepted(self):
        bound, window, _ = self.drive()
        ref = self.owner.completed[self.row['attempt_id']]['verification']['execution']
        (self.records.path/ref['path']).write_bytes(b'{"changed":true}')
        with self.assertRaises(control.SessionProtocolError): self.owner.validate_acceptance(bound, self.row, window)


if __name__ == '__main__':
    unittest.main()
