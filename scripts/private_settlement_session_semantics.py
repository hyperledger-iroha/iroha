"""Validate retained-session results before allowing another benchmark attempt.

Native Norito economics, actual packet replay and native CPU/RSS observations
are mandatory. A successful attempt is not a worker process exit. Final session
cleanup and registered-scope denominator accounting remain separate owners.
The session execution owner invokes these checks before publishing an ACK;
registered scope replay independently authenticates the retained result.
"""
from __future__ import annotations

import math
import json
from pathlib import Path

import private_settlement_release_runner as runner
import private_settlement_session_adapter as adapter
import private_settlement_session_control as control
import private_settlement_session_economics as economics
import private_settlement_packet_window as packets

NATIVE_PAYLOAD_FIELDS = frozenset({
    'economic_vector_sha256', 'primary_payment_count', 'monetary_movement_count',
    'stages_ms', 'proof_bytes', 'receipt_bytes', 'storage_growth_bytes',
    'finalized_receipt_observed', 'successful_leg_applications',
    'each_leg_applied_exactly_once', 'partial_visible_observations', 'partial_spendable_observations',
})


def measurement_bytes(value):
    """Encode measured durations/resources without admitting floats to control frames."""
    def inspect(item, depth):
        control.require(depth <= 32, 'measurement JSON exceeds its nesting bound')
        if item is None or type(item) in (str, bool):
            return
        if type(item) is int:
            control.unsigned(item)
        elif type(item) is float:
            control.require(math.isfinite(item) and item >= 0, 'measurement number is invalid')
        elif type(item) is list:
            for child in item: inspect(child, depth+1)
        elif type(item) is dict:
            control.require(all(type(key) is str for key in item), 'measurement key is not text')
            for child in item.values(): inspect(child, depth+1)
        else:
            raise control.SessionProtocolError('measurement JSON type is unsupported')
    inspect(value, 0)
    raw = json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode()
    control.require(0 < len(raw) <= control.MAX_FRAME_BYTES, 'measurement JSON exceeds its byte bound')
    return raw


def native_payload(result, request, ready, vector):
    """Require exact completed economics, atomicity, topology and stage inventory."""
    payload = control.exact(result['payload'], NATIVE_PAYLOAD_FIELDS, 'native benchmark payload')
    participants = request['participants']
    control.require(type(participants) is int and participants in (2, 3, 4, 8, 16),
                    'native benchmark participant count is invalid')
    for key in ('mandatory_signed_rs16_da_rbc', 'authenticated_message_control'):
        control.require(result[key] is True, 'native benchmark disabled required consensus evidence')
    control.unsigned(result['signed_rs16_da_observations'])
    control.require(result['signed_rs16_da_observations'] >= runner.minimum_signed_rs16_da_observations(participants)
                    and control.canonical(result['process_inventory']) == control.canonical(ready['process_inventory']),
                    'native benchmark omits DA evidence or substitutes its retained process inventory')
    control.require(payload['economic_vector_sha256'] == vector['economic_vector_sha256'],
                    'native result differs from the independently rederived payment vector')
    for key, expected in (('primary_payment_count', participants), ('monetary_movement_count', participants+1),
                          ('successful_leg_applications', participants), ('partial_visible_observations', 0),
                          ('partial_spendable_observations', 0)):
        control.require(type(payload[key]) is int and payload[key] == expected,
                        'native result has incomplete, partial or altered monetary movements')
    for key in ('finalized_receipt_observed', 'each_leg_applied_exactly_once'):
        control.require(payload[key] is True, 'native result does not prove exactly-once financial finality')
    profile = request['payload']['profile']
    control.require(profile in ('private', 'transparent_control'), 'unknown benchmark profile')
    stages = payload['stages_ms']
    expected_stages = (runner.benchmark_report.REQUIRED_PRIVATE_STAGES if profile == 'private'
                       else ('global_finality', 'end_to_end'))
    control.exact(stages, set(expected_stages), 'native benchmark stages')
    for duration in stages.values():
        control.require(type(duration) in (int, float) and math.isfinite(duration) and 0 <= duration <= 2**53-1,
                        'native benchmark stage is not a finite nonnegative duration')
    control.require(stages['end_to_end'] > 0
                    and all(duration <= stages['end_to_end'] for duration in stages.values()),
                    'native benchmark stage exceeds its end-to-end interval')
    for key in ('proof_bytes', 'receipt_bytes', 'storage_growth_bytes'):
        control.unsigned(payload[key])
    control.require(payload['receipt_bytes'] > 0 and (payload['proof_bytes'] > 0 if profile == 'private'
                                                     else payload['proof_bytes'] == 0),
                    'native benchmark proof/receipt sizes differ from its profile')
    return payload


class SessionSemantics:
    """Compose the actual economic verifier and independently replayed windows."""

    def __init__(self, prepared, *, records, observations, packet_utility,
                 cwd: Path, deadline, outer_timeout_ms: int):
        self.prepared, self.records, self.observations = prepared, records, observations
        self.session = control.identity(prepared['identity'])
        self.packet_utility = control.decode(control.canonical(packet_utility))
        self.cwd, self.deadline, self.outer_timeout_ms = cwd, deadline, outer_timeout_ms
        self.completed = {}
        self.native_invocations = {}

    def validate_ready(self, ready, request):
        """Require the frozen ready record and the unmodified activation policy."""
        control.require(request == self.prepared['request']
                        and self.records.read(self.prepared['reference']) == control.canonical(request),
                        'semantic owner received another session request')
        control.exact(ready, control.IDENTITY_FIELDS | {'network_id', 'genesis_sha256',
            'configuration_sha256', 'workload_manifest_sha256', 'activated_height',
            'process_inventory', 'worker_pid', 'network_ports'}, 'session readiness')
        control.identity({key: ready[key] for key in control.IDENTITY_FIELDS})
        control.require(all(ready[key] == value for key, value in self.session.items())
                        and ready['configuration_sha256'] == request['configuration_sha256']
                        and ready['workload_manifest_sha256'] == request['workload_manifest_sha256']
                        and type(ready['activated_height']) is int and ready['activated_height'] >= 301,
                        'session readiness does not bind completed governed activation')
        control.digest(ready['genesis_sha256'])
        runner.validate_process_inventory(ready['process_inventory'], participants=request['participants'],
                                          commit=request['commit'], label='session readiness inventory')
        reference = self.records.locate(f"sessions/{self.session['session_id']}/ready.json")
        control.require(self.records.read(reference) == control.canonical(ready), 'readiness record differs')
        self.ready_ref = reference

    def _window(self, reference, attempt, ready):
        control.reference(reference)
        control.require(reference['path'] == attempt['output_directory']+'/measurement-window.json',
                        'measurement window has a different attempt locator')
        raw = self.records.read(reference)
        window = control.decode(raw)
        control.exact(window, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
            'kind', 'ready_marker', 'finished_marker', 'process_resource_window', 'packet_window',
            'listener_before', 'listener_after', 'boundaries'}, 'benchmark measurement window')
        control.identity({key: window[key] for key in control.IDENTITY_FIELDS})
        control.attempt({key: window[key] for key in control.ATTEMPT_FIELDS})
        control.require(all(window[key] == value for key, value in self.session.items())
                        and all(window[key] == attempt[key] for key in control.ATTEMPT_FIELDS)
                        and window['kind'] == 'benchmark_measurement_window', 'measurement window identity differs')
        prefix = attempt['output_directory']
        expected_paths = {'ready_marker': prefix+'/evidence/benchmark-protocol/measurement-ready.json',
            'finished_marker': prefix+'/evidence/benchmark-protocol/measurement-finished.json',
            'process_resource_window': prefix+'/process-window.json',
            'listener_before': prefix+'/listeners-before.json', 'listener_after': prefix+'/listeners-after.json'}
        for key, path in expected_paths.items():
            control.require(control.reference(window[key])['path'] == path, 'measurement input locator differs')
        for boundary in ('ready', 'finished'):
            marker = control.decode(self.records.read(window[boundary+'_marker']))
            control.require(marker == {**self.session, **{key: attempt[key] for key in control.ATTEMPT_FIELDS},
                                       'boundary': boundary}, 'measurement marker differs')
        resource = control.decode(self.records.read(window['process_resource_window']))
        measured = self.observations.validate_window(resource, records=self.records, budget_ms=self.outer_timeout_ms)
        packet = packets.validate_packet_window(window['packet_window'], records=self.records,
            session=self.session, attempt={key: attempt[key] for key in control.ATTEMPT_FIELDS},
            ready_marker=window['ready_marker'], finished_marker=window['finished_marker'],
            ports_reference=ready['network_ports'], listener_before=window['listener_before'],
            listener_after=window['listener_after'], resource_window=resource,
            deadline_ns=self.deadline(), expected_utility=self.packet_utility)
        names = ('ready_received_ns', 'baseline_published_ns', 'begin_written_ns',
                 'finished_received_ns', 'packet_end_returned_ns', 'resources_finished_ns')
        control.exact(window['boundaries'], set(names), 'measurement handshake boundaries')
        boundaries = [control.unsigned(window['boundaries'][name]) for name in names]
        control.require(boundaries == sorted(boundaries) and boundaries[-1] <= self.deadline()
                        and measured['baseline_finished_monotonic_ns'] <= boundaries[1]
                        and boundaries[4] <= measured['final_started_monotonic_ns']
                        and measured['final_finished_monotonic_ns'] <= boundaries[5],
                        'native counter interval does not contain the measured handshake')
        control.require(measured['cpu_time_ns'] > 0 and measured['sampled_peak_rss_bytes'] > 0
                        and packet['window_ip_bytes'] > 0, 'benchmark has no measured CPU/RSS/network work')
        control.require(self.records.read(reference) == raw, 'measurement window changed during replay')
        return measured, packet

    def _derive(self, native_raw, request_raw, *, native_ref, measurement_window, ready, attempt, verification):
        request = control.decode(request_raw)
        terminal = adapter.terminal_envelope(native_raw, request, attempt['request']['sha256'])
        control.require(terminal['outcome']['kind'] == 'succeeded', 'unsuccessful attempt has no sample')
        vector = economics.validate_vector_result(verification['result'], verification['prepared'], records=self.records)
        control.require(self.records.read(verification['reference']) == control.canonical(vector)
                        and self.records.read(verification['execution']) == verification['execution_raw'],
                        'native vector execution evidence changed')
        result = terminal['outcome']['result']
        payload = native_payload(result, request, ready, vector)
        resource, packet = self._window(measurement_window, attempt, ready)
        metrics = {'stages_ms': payload['stages_ms'],
            'throughput_bundles_per_second': 1000/payload['stages_ms']['end_to_end'],
            'cpu_seconds': resource['cpu_time_ns']/1e9, 'peak_rss_bytes': resource['sampled_peak_rss_bytes'],
            'network_bytes': packet['window_ip_bytes'],
            **{key: payload[key] for key in ('proof_bytes', 'receipt_bytes', 'storage_growth_bytes')}}
        identity = {**self.session, **{key: attempt[key] for key in control.ATTEMPT_FIELDS}}
        environment = {key: request[key] for key in ('commit', 'hardware_sha256', 'hardware_profile_sha256',
            'configuration_sha256', 'participants', 'seed', 'workload_manifest_sha256')}
        provenance = {'rust_terminal': native_ref, 'measurement_window': measurement_window,
            'native_economic_verification': verification['execution'], 'request_sha256': attempt['request']['sha256']}
        sample = {**identity, **environment, **metrics, 'profile': request['payload']['profile'],
            'warmup': request['payload']['warmup'], 'economic_vector_sha256': vector['economic_vector_sha256'],
            'primary_payment_count': vector['primary_payment_count'],
            'monetary_movement_count': vector['monetary_movement_count'],
            'throughput_basis': 'serial_completed_bundle_elapsed',
            'network_counting_unit': packet['counting_unit'],
            'rss_observation': 'sampled_aggregate_peak', **provenance}
        response = {**identity, **environment, **provenance, 'kind': 'benchmark', 'passed': True,
            **{key: result[key] for key in ('mandatory_signed_rs16_da_rbc', 'signed_rs16_da_observations',
                                          'authenticated_message_control', 'process_inventory')},
            'payload': {**payload, **metrics}}
        return response, sample

    def materialize_terminal(self, native_raw, request_raw, *, native_ref, measurement_window, ready, attempt):
        """Publish a response only after actual native economics and window replay."""
        control.require(attempt['attempt_id'] not in self.completed, 'attempt materialized twice')
        request = control.decode(request_raw)
        control.require(self.records.read(native_ref) == native_raw
                        and self.records.read(attempt['request']) == request_raw,
                        'native terminal/request bytes differ from the retained attempt')
        terminal = adapter.terminal_envelope(native_raw, request, attempt['request']['sha256'])
        status = terminal['outcome']['kind']
        fields = {**self.session, **{key: attempt[key] for key in control.ATTEMPT_FIELDS},
            'request_sha256': attempt['request']['sha256'], 'rust_terminal': native_ref,
            'kind': 'benchmark_session_attempt_validation', 'status': status,
            'measurement_window': measurement_window, 'native_economic_verification': None, 'response': None}
        response_ref = None
        if status == 'succeeded':
            verification_inputs = economics.prepare_verification(self.prepared, attempt, records=self.records)
            try:
                invocation = economics.NativeVectorInvocation(verification_inputs, records=self.records,
                    image=self.observations.images['worker'], cwd=self.cwd, deadline_ns=self.deadline())
            except economics.NativeVectorIncomplete as error:
                self.native_invocations[attempt['attempt_id']] = error.invocation
                raise
            self.native_invocations[attempt['attempt_id']] = invocation
            verified = invocation.finish(process_reader=self.observations.reader)
            verified.update(prepared=verification_inputs, execution_raw=self.records.read(verified['execution']))
            response, sample = self._derive(native_raw, request_raw, native_ref=native_ref,
                measurement_window=measurement_window, ready=ready, attempt=attempt, verification=verified)
            response_ref = self.records.publish(attempt['output_directory']+'/response.json', measurement_bytes(response))
            fields.update(native_economic_verification=verified['execution'], response=response_ref)
            self.completed[attempt['attempt_id']] = {'verification': verified, 'sample': sample,
                'native_raw': native_raw, 'request_raw': request_raw, 'native_ref': native_ref,
                'response': response_ref, 'ready': control.decode(control.canonical(ready)), 'window': measurement_window}
        outcome = self.records.publish(attempt['output_directory']+'/evidence/benchmark-protocol/adapter-outcome.json',
                                       control.canonical(fields))
        if status == 'succeeded': self.completed[attempt['attempt_id']]['adapter_outcome'] = outcome
        return {'adapter_outcome': outcome, 'response': response_ref}

    def validate_acceptance(self, bound, attempt, window):
        """Recompute the sample and reject altered fields before the next dispatch."""
        control.exact(bound, {'rust_terminal', 'adapter_outcome', 'response', 'validation', 'sample'}, 'acceptance inputs')
        completed = self.completed[attempt['attempt_id']]
        control.require(window == completed['window'] and bound['rust_terminal'] == completed['native_raw']
                        and bound['adapter_outcome'] == self.records.read(completed['adapter_outcome'])
                        and bound['response'] == self.records.read(completed['response']),
                        'acceptance substitutes its completed result')
        response, sample = self._derive(completed['native_raw'], completed['request_raw'],
            native_ref=completed['native_ref'], measurement_window=window, ready=completed['ready'],
            attempt=attempt, verification=completed['verification'])
        control.require(bound['response'] == measurement_bytes(response) and bound['sample'] == measurement_bytes(sample),
                        'accepted response/sample differs from replayed native measurement')
        validation = control.decode(bound['validation'])
        control.exact(validation, control.IDENTITY_FIELDS | control.ATTEMPT_FIELDS | {
            'passed', 'validation_kind', 'response', 'sample'}, 'attempt acceptance')
        control.identity({key: validation[key] for key in control.IDENTITY_FIELDS})
        control.attempt({key: validation[key] for key in control.ATTEMPT_FIELDS})
        control.require(all(validation[key] == value for key, value in self.session.items())
                        and all(validation[key] == attempt[key] for key in control.ATTEMPT_FIELDS)
                        and validation['passed'] is True and validation['validation_kind'] == 'accepted'
                        and validation['response'] == completed['response']
                        and validation['sample'] == self.records.locate(attempt['output_directory']+'/benchmark-sample.json'),
                        'accepted validation differs from the exact native response/sample')
