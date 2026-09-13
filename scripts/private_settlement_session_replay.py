"""Reconstruct native economic execution from exact retained evidence.

This read-only reducer does not execute a new native verification. It joins
the source-admitted command to its retained launch, actual terminal, logs and
result, then invokes the canonical vector validator. The enclosing collector
authenticates the complete source/evidence inventory; arbitrary self-authored
receipts cannot independently establish execution provenance.
The sample replay owner joins these records to process and packet evidence.
"""
from __future__ import annotations

from pathlib import Path
import re

import private_settlement_session_control as control
import private_settlement_session_economics as economics


def retained_verification_inputs(prepared, attempt, *, records):
    """Reconstruct the exact native input packet without republishing any file."""
    identity = control.identity(prepared['identity'])
    control.require(records.read(prepared['reference']) == control.canonical(prepared['request'])
                    and prepared['reference']['sha256'] == identity['session_request_sha256']
                    and sum(control.canonical(row) == control.canonical(attempt)
                            for row in prepared['request']['attempts']) == 1,
                    'retained economic verification substitutes its registered session or attempt')
    prefix = attempt['output_directory']+'/evidence/benchmark-protocol'
    reference = records.locate(prefix+'/economic-vector-verification-request.json')
    raw = records.read(reference); document = control.decode(raw)
    expected = {'version': 1, 'protocol': control.PROTOCOL,
        'kind': 'benchmark_economic_vector_verification', 'session_request': prepared['reference'],
        'benchmark_request': attempt['request'],
        'ready': records.locate('sessions/'+identity['session_id']+'/ready.json'),
        'workload_record': records.locate(attempt['output_directory']+'/evidence/matched-workload.json')}
    control.require(raw == control.canonical(expected), 'native verification input inventory differs')
    bound = {name: records.read(document[name]) for name in economics.INPUT_NAMES}
    request = control.decode(bound['benchmark_request']); ready = control.decode(bound['ready'])
    control.attempt({key: attempt[key] for key in control.ATTEMPT_FIELDS})
    control.require(request['kind'] == 'benchmark'
                    and control.canonical({key: request[key] for key in control.ATTEMPT_FIELDS-{'attempt_id'}})
                        == control.canonical({key: attempt[key] for key in control.ATTEMPT_FIELDS-{'attempt_id'}})
                    and request['session_id'] == identity['session_id']
                    and request['session_invocation_nonce'] == identity['session_invocation_nonce']
                    and request['workload_manifest_sha256'] == prepared['request']['workload_manifest_sha256']
                    and request['participants'] == prepared['request']['participants']
                    and control.canonical({key: ready[key] for key in identity}) == control.canonical(identity),
                    'retained verifier changes its benchmark request or readiness')
    return {'identity': identity, 'attempt': attempt, 'reference': reference,
            'document': document, 'input_bytes': bound,
            'output_path': prefix+'/economic-vector-verification.json'}


def replay_native_economic_execution(prepared, attempt, execution_reference, *, records,
                                    worker_command_path, worker_sha256, parent_pid,
                                    group_utility_sha256, outer_timeout_ms):
    """Require the complete named-test execution, natural exit, and input joins."""
    control.digest(worker_sha256); control.digest(group_utility_sha256)
    control.unsigned(parent_pid); control.unsigned(outer_timeout_ms)
    control.require(parent_pid > 1 and outer_timeout_ms > 0
                    and type(worker_command_path) is str and Path(worker_command_path).is_absolute(),
                    'native economic replay lacks admitted owner or deadline')
    inputs = retained_verification_inputs(prepared, attempt, records=records)
    prefix = str(Path(inputs['output_path']).parent)+'/native-vector-process'
    captured = {}

    def read(name, fields):
        reference = records.locate(prefix+name)
        raw = records.read(reference); captured[reference['path']] = (reference, raw)
        return reference, control.exact(control.decode(raw), set(fields), 'native execution '+name)

    receipt_ref, receipt = read('-verified.json', {'kind', 'verification_request', 'native_result',
        'process_terminal', 'executable_sha256', 'executed_native_verifier'})
    control.require(control.canonical(receipt_ref) == control.canonical(execution_reference)
                    and receipt['kind'] == 'benchmark_native_economic_verification_execution'
                    and receipt['verification_request'] == inputs['reference']
                    and receipt['executable_sha256'] == worker_sha256
                    and receipt['executed_native_verifier'] is True,
                    'native economic execution receipt differs from its admitted source')
    launch_ref, launch = read('-launch.json', {'verification_request', 'command', 'executable_sha256',
        'started_ns', 'started_monotonic_ns', 'deadline_monotonic_ns'})
    command = [worker_command_path, economics.VERIFIER_TEST, '--exact', '--ignored', '--nocapture', '--test-threads=1']
    control.require(control.canonical(launch['command']) == control.canonical(command)
                    and launch['executable_sha256'] == worker_sha256
                    and launch['verification_request'] == inputs['reference'],
                    'economic verifier did not use the exact admitted native entrypoint')
    start_ref, start = read('-start.json', {'launch', 'pid', 'parent_pid', 'process_group', 'spawned_monotonic_ns'})
    pid = control.unsigned(start['pid']); control.unsigned(start['parent_pid']); control.unsigned(start['process_group'])
    control.require(pid > 1 and start['launch'] == launch_ref and start['parent_pid'] == parent_pid
                    and start['process_group'] == pid, 'native verifier process owner differs')
    exit_ref, exit_record = read('-exit.json', {'launch', 'pid', 'exit_code', 'natural_wait_observed', 'observed_monotonic_ns'})
    terminal_ref, terminal = read('-terminal.json', {'launch', 'process_start', 'exit_observation', 'pid',
        'exit_code', 'natural_wait_observed', 'finished_monotonic_ns', 'group_before', 'kernel_absence',
        'group_after', 'logs'})
    for record in (exit_record, terminal):
        control.require(record['launch'] == launch_ref and type(record['pid']) is int and record['pid'] == pid
                        and type(record['exit_code']) is int and record['exit_code'] == 0
                        and record['natural_wait_observed'] is True,
                        'native verifier has no exact successful natural terminal')
    control.require(terminal['process_start'] == start_ref and terminal['exit_observation'] == exit_ref
                    and receipt['process_terminal'] == terminal_ref, 'native verifier terminal links differ')
    groups = []
    for key in ('group_before', 'group_after'):
        group = control.exact(terminal[key], {'process_group', 'members', 'utility_sha256', 'observed_monotonic_ns'}, 'native group absence')
        control.require(type(group['process_group']) is int and group['process_group'] == pid
                        and type(group['members']) is list and group['members'] == []
                        and group['utility_sha256'] == group_utility_sha256,
                        'native verifier group absence is incomplete or uses another utility')
        groups.append(control.unsigned(group['observed_monotonic_ns']))
    absence = control.exact(terminal['kernel_absence'], {'pid', 'kernel_absence_observed', 'observed_monotonic_ns'}, 'native kernel absence')
    control.require(type(absence['pid']) is int and absence['pid'] == pid
                    and absence['kernel_absence_observed'] is True, 'native verifier kernel absence differs')
    deadline = control.unsigned(launch['deadline_monotonic_ns'])
    ordered = [control.unsigned(launch['started_monotonic_ns']), control.unsigned(start['spawned_monotonic_ns']),
               control.unsigned(exit_record['observed_monotonic_ns']), groups[0],
               control.unsigned(absence['observed_monotonic_ns']), groups[1],
               control.unsigned(terminal['finished_monotonic_ns']), deadline]
    durable = control.decode(records.read(records.locate(attempt['output_directory']+'/started.json')))
    expected = {**prepared['identity'], **{key: attempt[key] for key in control.ATTEMPT_FIELDS},
                'request': attempt['request'], 'outer_timeout_ms': outer_timeout_ms}
    control.require(control.canonical({key: durable[key] for key in expected}) == control.canonical(expected),
                    'native verification is not bound to its actual durable attempt start')
    began = control.unsigned(durable['started_ns']); launched = control.unsigned(launch['started_ns'])
    control.require(ordered == sorted(ordered) and 0 < deadline-ordered[0] <= outer_timeout_ms*1_000_000
                    and began <= launched <= began+outer_timeout_ms*1_000_000,
                    'native verifier exceeded or reordered its registered execution boundary')
    logs_ref, logs = read('-logs.json', {'stdout_hex', 'stderr_hex'})
    control.require(terminal['logs'] == logs_ref, 'native verifier log binding differs')
    raw_logs = []
    for name in ('stdout_hex', 'stderr_hex'):
        value = logs[name]
        control.require(type(value) is str and len(value) <= economics.MAX_LOG_BYTES*2
                        and len(value) % 2 == 0 and re.fullmatch('[0-9a-f]*', value) is not None,
                        'native verifier retained log encoding is invalid')
        raw_logs.append(bytes.fromhex(value))
    economics.native_test_passed(*raw_logs)
    result_ref = records.locate(inputs['output_path'])
    result_raw = records.read(result_ref)
    control.require(receipt['native_result'] == result_ref, 'native verification substitutes its result')
    result = economics.validate_vector_result(control.decode(result_raw), inputs, records=records)
    control.require(all(records.read(ref) == raw for ref, raw in captured.values())
                    and records.read(result_ref) == result_raw, 'native execution changed during replay')
    return {'reference': result_ref, 'result': result, 'execution': receipt_ref,
            'execution_raw': records.read(receipt_ref), 'prepared': inputs,
            'deadline_monotonic_ns': deadline}
