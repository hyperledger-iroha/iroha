"""Synthetic retained-session accounting controls using the real control codec.

These tests launch no process or network and make no measurement qualification.
The semantic callback recomputes an explicit fixture sample, never echoes it.
"""
from __future__ import annotations

import copy
import hashlib
import io
import json
from pathlib import Path
import sys
import unittest
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import private_settlement_attempt_accounting as accounting
import private_settlement_session_control as control


def digest(value):
    return hashlib.sha256(accounting.accounting_canonical_bytes(value)).hexdigest()


class MemoryRecords:
    def __init__(self): self.values = {}
    def inventory(self): return {name: self.locate(name) for name in self.values}
    def publish(self, name, raw):
        assert name not in self.values
        self.values[name] = raw
        return self.locate(name)
    def put(self, name, value): return self.publish(name, control.canonical(value))
    def locate(self, name): return {'path': name, 'sha256': hashlib.sha256(self.values[name]).hexdigest(), 'bytes': len(self.values[name])}
    def read(self, reference):
        assert self.locate(reference['path']) == reference
        return self.values[reference['path']]


class Fixture:
    """Build one real-codec session among a complete 800-job registered plan."""
    def __init__(self, kinds=('succeeded',)*6+('failed',), *, ack_consumed=True,
                 pending_successor=False, setup_failure=False, fault_prefix=False,
                 session_index=0, worker_birth=10, ack_published=True, activated_height=4):
        self.store = MemoryRecords(); self.replays = 0
        self.image = {'sha256': 'f'*64, 'bytes': 100}
        self.command = ['/source-admitted/worker', 'retained_session', '--exact', '--ignored']
        configs = {n: digest(['configuration', n]) for n in (2, 3, 4, 8, 16)}
        workloads = {n: digest(accounting.build_benchmark_workload_policy(n)) for n in configs}
        planned = accounting.build_benchmark_session_plan(configs, list(range(10)), workloads,
                                                         warmups_per_session=5, measured_per_profile=30)
        policy = {'timeout_scope': accounting.TIMEOUT_SCOPE, 'outer_timeout_ms': 600000,
                  'rust_deadline_budgets_ms': {key: 300000 for key in accounting.DEADLINE_STAGES}}
        self.policy = policy
        plan = {key: {} for key in accounting.PLAN_FIELDS}
        plan.update(version=1, protocol=control.PROTOCOL, commit='a'*40, worktree_clean=True,
                    publication_evidence=False, execution_required=True,
                    harness={'sha256': 'e'*64, 'bytes': 200},
                    hardware={'sha256': 'b'*64, 'profile_sha256': 'c'*64}, benchmark_accounting=policy,
                    benchmark_sessions=planned['sessions'], jobs=planned['jobs'], workload_manifests=[
                        {'participants':n,'path':f'workloads/n{n}.json','sha256':workloads[n],
                         'bytes':len(accounting.accounting_canonical_bytes(accounting.build_benchmark_workload_policy(n)))} for n in configs])
        tail = {'kind': 'leakage', 'case': 'unstarted_tail'}
        tail = {'request_id': digest(tail), **tail}
        plan['jobs'] = plan['jobs'] + [tail]
        if fault_prefix:
            fault = {'kind': 'fault', 'case': 'prefix'}
            plan['jobs'].insert(0, {'request_id': digest(fault), **fault})
        self.plan = plan; self.plan_raw = accounting.accounting_canonical_bytes(plan)
        self.scope = {'version': 1, 'protocol': control.PROTOCOL, 'scope_id': 'd'*64,
            'previous_scope_sha256': None, 'registered_ns': 1, 'stopping_policy': 'fail_fast',
            'deadline_policy': policy, 'campaigns': [{'campaign_id': 'campaign-a',
                                                 'plan': accounting.accounting_file_binding(self.plan_raw)}]}
        self.scope_raw = accounting.accounting_canonical_bytes(self.scope)
        self.base = {'scope_sha256': hashlib.sha256(self.scope_raw).hexdigest(),
                     'campaign_id': 'campaign-a', 'plan_sha256': hashlib.sha256(self.plan_raw).hexdigest()}
        self.descriptor = planned['sessions'][session_index]; sid = self.descriptor['session_id']
        session_nonce = digest(['session', sid])
        offset = session_index * 10**15
        self.prefix = f'sessions/{sid}'
        self.jobs = [(i, job) for i, job in enumerate(plan['jobs'], 1) if job.get('session_id') == sid]
        self.rows = []
        for index, (ordinal, job) in enumerate(self.jobs):
            aid = accounting.registered_attempt_id(**self.base, request_id=job['request_id'])
            nonce = digest(['attempt', sid, index]); path = f"attempts/{ordinal:05}-{job['request_id']}"
            request = {'version': 1, 'protocol': control.PROTOCOL, 'kind': 'benchmark',
                **{key: job[key] for key in ('request_id', 'participants', 'seed', 'session_attempt_index',
                                           'configuration_sha256', 'workload_manifest_sha256')},
                'session_id': sid, 'session_invocation_nonce': session_nonce,
                'invocation_nonce': nonce, 'commit': plan['commit'],
                'payload': {'profile': job['profile'], 'warmup': job['warmup']}}
            ref = self.store.put(path+'/request.json', request)
            self.rows.append({'attempt_id': aid, 'request_id': job['request_id'], 'invocation_nonce': nonce,
                              'session_attempt_index': index, 'request': ref, 'output_directory': path})
        request = {'version': 1, 'protocol': control.PROTOCOL, 'kind': 'benchmark_session', **self.base,
            'session_id': sid, 'session_invocation_nonce': session_nonce,
            **{key: self.descriptor[key] for key in ('profile', 'participants', 'seed',
                'configuration_sha256', 'workload_manifest_sha256')},
            'warmups': 5, 'attempts': self.rows,
            'workload_manifest': accounting.build_benchmark_workload_policy(2)}
        session_ref = self.store.put(self.prefix+'/request.json', request)
        self.identity = {'version': 1, 'protocol': control.PROTOCOL, **self.base, 'session_id': sid,
                        'session_invocation_nonce': session_nonce, 'session_request_sha256': session_ref['sha256']}
        started = {**self.identity, 'request': session_ref, 'command': self.command,
                   'harness': self.image, 'started_ns': offset+10}
        self.started = self.store.put(self.prefix+'/started.json', started)
        self.worker_start = self.store.put(self.prefix+'/worker-process-start.json', {
            'command': self.command, 'pid': 777, 'parent_pid': 700, 'process_group': 777,
            'started_ns': offset+11, 'worker_sha256': self.image['sha256']})
        self.chains = {}
        for channel, endpoints in control.CHANNEL_ENDPOINTS.items():
            for direction in control.DIRECTIONS:
                for observer in endpoints:
                    self.chains[observer, channel, direction] = control.ControlChain(
                        self.identity, self.started['sha256'], channel, direction, self.store,
                        observer=observer, journal_prefix=self.prefix+'/control')
        self.ready = self.process_ready = None
        pids = list(range(778, 791))  # 3 exact four-validator committees and coordinator.
        if not setup_failure:
            inventory = [{'pid': pid} for pid in pids]
            ports = self.store.put(self.prefix+'/network-ports.json', {'fixture': 'ports'})
            self.ready = self.store.put(self.prefix+'/ready.json', {**self.identity,
                'worker_pid': 777, 'process_inventory': inventory, 'activated_height': activated_height,
                'network_id': '1'*64, 'genesis_sha256': '2'*64,
                'configuration_sha256': request['configuration_sha256'],
                'workload_manifest_sha256': request['workload_manifest_sha256'], 'network_ports': ports})
            identities = [{'label': str(pid), 'identity': {'pid': pid, 'ppid': 700 if pid == 777 else 777,
                'pgid': 777, 'executable_sha256': self.image['sha256'],
                'loaded_image_uuid': '7'*32, 'birth': {'kind': 'darwin_bsdinfo',
                    'started_seconds': worker_birth, 'started_microseconds': pid, 'start_abstime': pid}},
                'cpu_counter_unit_ns': 1} for pid in [777] + pids]
            self.process_ready = self.store.put(self.prefix+'/process-ready.json', {**self.identity,
                'kind': 'benchmark_session_process_ready', 'ready': self.ready,
                'process_scope': {'processes': identities}, 'listeners': {'fixture': 'listeners'}, 'network_ports': ports})
            message = self.send('adapter_worker', 'child_to_owner', 'ready', {'ready': self.ready})
            self.send('runner_adapter', 'child_to_owner', 'ready',
                      {'ready': self.ready, 'process_observation': self.process_ready}, upstream=message)
        self.samples = []; self.starts = []; self.accepted = []; predecessor = None
        for index, kind in enumerate(() if setup_failure else kinds):
            row = self.rows[index]; aid = {key: row[key] for key in control.ATTEMPT_FIELDS}
            start = self.attempt_start(index, predecessor)
            self.starts.append(row['request_id'])
            payload = {**aid, 'request': row['request'], 'attempt_started': start}
            dispatch = self.send('runner_adapter', 'owner_to_child', 'dispatch', payload)
            self.send('adapter_worker', 'owner_to_child', 'dispatch', payload, upstream=dispatch)
            if kind == 'incomplete': break
            request = json.loads(self.store.read(row['request']))
            native = {'version': 1, 'protocol': control.PROTOCOL,
                **{key: request[key] for key in ('request_id', 'invocation_nonce', 'commit', 'participants')},
                'request_sha256': row['request']['sha256'], 'elapsed_ms': 300000}
            if kind == 'succeeded':
                result = {**{key: native[key] for key in native if key != 'elapsed_ms'},
                    'mandatory_signed_rs16_da_rbc': True, 'signed_rs16_da_observations': [],
                    'authenticated_message_control': {}, 'process_inventory': [],
                    'payload': {'fixture_value': index + 1.25}}
                native['outcome'] = {'kind': kind, 'result': result}
            elif kind == 'failed':
                native['outcome'] = {'kind': kind, 'stage': 'benchmark_worker', 'reason': 'execution_error'}
            else:
                native['outcome'] = {'kind': kind, 'stage': 'private_receipt', 'budget_ms': 300000, 'elapsed_ms': 300000}
            terminal_ref = self.store.publish(row['output_directory']+'/evidence/benchmark-protocol/rust-result.json',
                                              accounting.accounting_canonical_bytes(native))
            response = (self.store.put(row['output_directory']+'/response.json', {**self.identity, **aid,
                        'kind': 'benchmark', 'passed': True}) if kind == 'succeeded' else None)
            adapter_ref = self.store.put(row['output_directory']+'/evidence/benchmark-protocol/adapter-outcome.json', {
                **self.identity, **aid, 'request_sha256': row['request']['sha256'], 'rust_terminal': terminal_ref,
                'kind': 'benchmark_session_attempt_validation', 'status': kind,
                'measurement_window': None, 'native_economic_verification': None, 'response': response})
            completed = self.send('adapter_worker', 'child_to_owner', 'attempt_completed', {**aid, 'rust_terminal': terminal_ref})
            payload = {**aid, 'rust_terminal': terminal_ref, 'adapter_outcome': adapter_ref, 'response': response}
            self.send('runner_adapter', 'child_to_owner', 'attempt_completed', payload, upstream=completed)
            if kind != 'succeeded': break
            sample = self.recompute(self.identity, row, {'rust_terminal': self.store.read(terminal_ref)}, None)
            sample_ref = self.store.publish(row['output_directory']+'/benchmark-sample.json', sample)
            validation = self.store.put(row['output_directory']+'/validation-outcome.json', {
                **self.identity, **aid, 'passed': True, 'validation_kind': 'accepted', 'response': response, 'sample': sample_ref})
            payload.update(validation=validation, sample=sample_ref)
            if not ack_published and index == len(kinds)-1:
                self.samples.append(sample)
                break
            ack = self.send('runner_adapter', 'owner_to_child', 'accept', payload,
                            received=ack_consumed or index != len(kinds)-1)
            for phase in ('proposed', 'validated', 'pipe-written'):
                self.store.put(self.prefix+f'/control/ack-{index:06d}-{phase}.json', {
                    **self.identity, **aid, 'acknowledgement': ack.binding, 'phase': phase})
            if ack_consumed or index != len(kinds)-1:
                self.send('adapter_worker', 'owner_to_child', 'accept', payload, upstream=ack)
                self.accepted.append(row['request_id'])
            predecessor = ack.binding
            self.samples.append(sample)
        if pending_successor:
            index = len(kinds)
            self.attempt_start(index, predecessor)
            self.starts.append(self.rows[index]['request_id'])
        self.replays = 0
        complete = len(self.accepted) == len(self.rows)
        kind = 'completed' if complete else 'failed' if setup_failure else (
            kinds[-1] if kinds and kinds[-1] in {'failed', 'timed_out'} else 'incomplete')
        self.terminal_value = {**self.identity, 'kind': kind,
            'reason': None if complete else 'setup_failed' if setup_failure else 'attempt_'+kind,
            'accepted_request_ids': self.accepted,
            'active_attempt_id': None if setup_failure or len(self.accepted) == len(kinds) else self.rows[len(kinds)-1]['attempt_id'],
            'last_owner_message_sha256': self.chains['worker', 'adapter_worker', 'owner_to_child'].previous,
            'last_worker_message_sha256': self.chains['worker', 'adapter_worker', 'child_to_owner'].previous,
            'network_shutdown_observed': True, 'coordinator_reaped_observed': True}
        terminal_ref = self.store.put(self.prefix+'/worker-terminal.json', self.terminal_value)
        message = self.send('adapter_worker', 'child_to_owner', 'session_completed', {'worker_terminal': terminal_ref})
        observed_pids = [777] if setup_failure else [777] + pids
        lifecycle = {**self.identity, 'kind': 'benchmark_session_worker_lifecycle',
            'worker_terminal': terminal_ref, 'worker_process_start': self.worker_start, 'worker_pid': 777,
            'worker_exit_code': 0 if complete else 101, 'worker_wait_completed': True,
            'worker_sha256': self.image['sha256'], 'worker_image_unchanged': True,
            'group_before': {'process_group': 777, 'members': [], 'utility_sha256': '6'*64, 'observed_monotonic_ns': 100},
            'kernel_absences': [{'pid': pid, 'kernel_absence_observed': True,
                                'observed_monotonic_ns': 101+i} for i, pid in enumerate(observed_pids)],
            'group_after': {'process_group': 777, 'members': [], 'utility_sha256': '6'*64, 'observed_monotonic_ns': 200}}
        life_ref = self.store.put(self.prefix+'/adapter-lifecycle.json', lifecycle)
        self.send('runner_adapter', 'child_to_owner', 'session_completed',
                  {'worker_terminal': terminal_ref, 'adapter_lifecycle': life_ref}, upstream=message)
        closure = {**self.identity, 'session_started': self.started, 'worker_terminal': terminal_ref,
            'adapter_lifecycle': life_ref, 'ready': self.ready, 'process_observation': self.process_ready,
            'closed_ns': offset+10**15, 'bindings_unchanged': True, 'adapter_thread_joined': True}
        self.closure_ref = self.store.put(self.prefix+'/session-closure.json', closure)
        campaign_closure = {'version': 1, 'protocol': control.PROTOCOL, **self.base,
            'closed_ns': offset+10**15+1, 'quiescent': True, 'started_request_ids': self.starts,
            'reason': 'fail_fast', 'started_session_ids': [sid], 'session_closures': [self.closure_ref]}
        others = [{'request_id': job['request_id'], 'started': None, 'request': None, 'process': None}
                  for job in plan['jobs'] if job['kind'] != 'benchmark']
        self.packet = {'campaign_id': 'campaign-a', 'plan': self.plan_raw,
            'closure': accounting.accounting_canonical_bytes(campaign_closure),
            'sessions': [None]*session_index + [{'session_id': sid, 'closure': self.closure_ref}]
                        + [None]*(len(planned['sessions'])-session_index-1),
            'records': self.store, 'nonbenchmark': others}

    def attempt_start(self, index, predecessor):
        row = self.rows[index]; ordinal = self.jobs[index][0]
        return self.store.put(row['output_directory']+'/started.json', {
            **self.identity, **{key: row[key] for key in control.ATTEMPT_FIELDS},
            'ordinal': ordinal, 'session_started': self.started, 'request': row['request'],
            'outer_timeout_ms': self.policy['outer_timeout_ms'], 'started_ns': self.plan['benchmark_sessions'].index(self.descriptor)*10**15+20+index,
            'preceding_acceptance': predecessor})

    def send(self, channel, direction, kind, payload, *, upstream=None, received=True):
        owner, child = control.CHANNEL_ENDPOINTS[channel]
        sender, receiver = (owner, child) if direction == 'owner_to_child' else (child, owner)
        stream = io.BytesIO()
        wire = self.chains[sender, channel, direction].send(stream, kind, payload,
            forwarded_from=None if upstream is None else upstream.binding)
        if received:
            stream.seek(0); self.chains[receiver, channel, direction].receive(stream)
        return wire

    def recompute(self, identity, attempt, bound, records):
        self.replays += 1
        native = json.loads(bound['rust_terminal'])
        job = self.jobs[attempt['session_attempt_index']][1]
        value = {**identity, **{key: attempt[key] for key in control.ATTEMPT_FIELDS},
                 **{key: job[key] for key in ('profile', 'participants', 'seed', 'warmup',
                                            'configuration_sha256', 'workload_manifest_sha256')},
                 'fixture_value': native['outcome']['result']['payload']['fixture_value']}
        return accounting.accounting_canonical_bytes(value)

    def reduce(self, **kwargs):
        return accounting.reduce_registered_scope(self.scope_raw, [self.packet], self.samples,
            worker_command=self.command, worker_image=self.image,
            validate_success=kwargs.pop('validate_success', self.recompute), **kwargs)


class RetainedAccountingTests(unittest.TestCase):
    def reject(self, fixture):
        with self.assertRaises((accounting.AccountingError, control.SessionProtocolError)):
            fixture.reduce()

    def test_measured_success_precedes_failed_campaign_and_unstarted_leakage_tail(self):
        f = Fixture(); result = f.reduce()
        self.assertEqual(result['counts'], dict(planned=800, attempted=7, not_started=793,
                                               succeeded=6, failed=1, timed_out=0, incomplete=0))
        self.assertEqual(f.replays, 6)
        measured = next(r for r in result['cohorts'] if r['profile'] == 'private' and r['participants'] == 2 and not r['warmup'])
        self.assertEqual(measured['counts']['succeeded'], 1)
        self.assertEqual(measured['counts']['failed'], 1)

    def test_failed_warmup_is_attempted_and_stops_all_later_sessions(self):
        result = Fixture(('failed',)).reduce()
        self.assertEqual(result['counts']['attempted'], 1)
        self.assertEqual(result['counts']['failed'], 1)
        self.assertTrue(result['rows'][0]['warmup'])

    def test_typed_deadline_is_not_a_censored_latency_sample(self):
        f = Fixture(('timed_out',)); result = f.reduce()
        self.assertEqual(result['counts']['timed_out'], 1)
        self.assertEqual(f.samples, [])

    def test_missing_attempt_completion_is_incomplete_after_real_session_closure(self):
        result = Fixture(('succeeded', 'incomplete')).reduce()
        self.assertEqual(result['counts']['succeeded'], 1)
        self.assertEqual(result['counts']['incomplete'], 1)
        self.assertFalse(result['accounting_complete'])

    def test_native_success_survives_later_ack_delivery_failure(self):
        result = Fixture(('succeeded',), ack_consumed=False).reduce()
        self.assertEqual(result['counts']['succeeded'], 1)
        self.assertEqual(result['campaigns'][0]['sessions'][0]['terminal_kind'], 'incomplete')

    def test_replayed_published_success_survives_ack_frame_publication_failure(self):
        result = Fixture(('succeeded',), ack_consumed=False, ack_published=False).reduce()
        self.assertEqual(result['counts']['succeeded'], 1)
        self.assertEqual(result['counts']['attempted'], 1)

    def test_consumed_ack_does_not_fabricate_a_missing_pipe_written_receipt(self):
        f = Fixture(('succeeded',))
        del f.store.values[f.prefix+'/control/ack-000000-pipe-written.json']
        self.assertEqual(f.reduce()['counts']['succeeded'], 1)

    def test_successor_durable_start_before_delivery_counts_attempted_incomplete(self):
        result = Fixture(('succeeded',), ack_consumed=False, pending_successor=True).reduce()
        self.assertEqual(result['counts']['attempted'], 2)
        self.assertEqual(result['counts']['succeeded'], 1)
        self.assertEqual(result['counts']['incomplete'], 1)

    def test_complete_session_uses_one_worker_exit_for_eight_attempts(self):
        result = Fixture(('succeeded',)*8).reduce()
        self.assertEqual(result['counts']['succeeded'], 8)
        self.assertEqual(result['campaigns'][0]['sessions'][0]['terminal_kind'], 'completed')

    def test_ready_observed_heights_have_no_scheduled_activation_floor(self):
        for height in (1, 4, 300, 301, accounting.MAX_U64):
            with self.subTest(height=height):
                f = Fixture(('succeeded',)*8, activated_height=height)
                result = f.reduce()
                self.assertEqual(result['counts']['succeeded'], 8)
                self.assertEqual(f.replays, 8)
                self.assertEqual(result['campaigns'][0]['sessions'][0]['terminal_kind'], 'completed')

    def test_ready_rejects_zero_and_noninteger_observed_heights(self):
        # These values survive the control JSON codec and reach accounting.
        for height in (0, True, False, '4', None):
            with self.subTest(height=height), self.assertRaises(accounting.AccountingError):
                Fixture(activated_height=height).reduce()
        # The bounded codec itself refuses values outside the wire u64 domain.
        for height in (-1, 4.0, accounting.MAX_U64 + 1):
            with self.subTest(height=height), self.assertRaises(control.SessionProtocolError):
                Fixture(activated_height=height)

    def test_rehashed_immediate_ready_cannot_replace_worker_or_session_workload(self):
        for field, replacement in (('worker_pid', 778), ('configuration_sha256', '8'*64),
                ('workload_manifest_sha256', '8'*64), ('session_invocation_nonce', '8'*64)):
            f = Fixture(activated_height=1)
            closure = json.loads(f.store.values[f.closure_ref['path']])
            ready = json.loads(f.store.values[f.ready['path']]); ready[field] = replacement
            f.store.values[f.ready['path']] = control.canonical(ready)
            closure['ready'] = f.store.locate(f.ready['path'])
            observed = json.loads(f.store.values[f.process_ready['path']])
            observed['ready'] = closure['ready']
            f.store.values[f.process_ready['path']] = control.canonical(observed)
            closure['process_observation'] = f.store.locate(f.process_ready['path'])
            with self.subTest(field=field), self.assertRaises(accounting.AccountingError):
                accounting._session_lifetime(accounting._SessionRecords(f.store), closure,
                    f.identity, json.loads(f.store.read(f.started)), f.accepted,
                    f.rows[6]['attempt_id'], f.image, unready_attempts_absent=False)

    def test_setup_failure_creates_no_attempt_denominator(self):
        result = Fixture(setup_failure=True).reduce()
        self.assertEqual(result['counts']['attempted'], 0)
        self.assertEqual(len(result['campaigns'][0]['sessions']), 1)

    def test_scope_requires_explicit_semantic_replay(self):
        with self.assertRaises(accounting.AccountingError): Fixture().reduce(validate_success=None)

    def test_fabricated_metric_is_rejected_by_replay(self):
        f = Fixture()
        with self.assertRaises(accounting.AccountingError):
            f.reduce(validate_success=lambda *args: b'{"fabricated":true}')

    def test_missing_and_extra_published_success_rejected(self):
        for operation in ('missing', 'extra'):
            f = Fixture()
            if operation == 'missing': f.samples.pop()
            else: f.samples.append(f.samples[0])
            self.reject(f)

    def test_missing_lifecycle_or_unclosed_campaign_cannot_establish_tail(self):
        for name in ('adapter-lifecycle.json', 'worker-terminal.json', 'worker-process-start.json'):
            f = Fixture(); del f.store.values[f.prefix+'/'+name]; self.reject(f)
        f = Fixture(); closure = json.loads(f.packet['closure']); closure['quiescent'] = False
        f.packet['closure'] = accounting.accounting_canonical_bytes(closure); self.reject(f)

    def test_rehashed_session_closure_owner_and_cut_mutations_rejected(self):
        for field, value in (('session_invocation_nonce', '8'*64), ('session_id', '4'*64),
                             ('session_request_sha256', '5'*64), ('bindings_unchanged', False),
                             ('closed_ns', True), ('closed_ns', 10**16)):
            f = Fixture(); self.rebind_closure(f, field, value); self.reject(f)

    def test_native_owner_image_and_command_must_match_source_admission(self):
        f = Fixture(); f.image = {'sha256': '3'*64, 'bytes': 100}; self.reject(f)
        f = Fixture(); f.command = ['/different/worker']; self.reject(f)

    def test_rebound_lifecycle_quiescence_and_owner_rejected(self):
        for field, value in (('worker_pid', 778), ('worker_wait_completed', False),
                             ('worker_exit_code', 0), ('worker_exit_code', False),
                             ('worker_image_unchanged', False), ('worker_sha256', '3'*64)):
            f = Fixture(); self.rebind_record(f, 'adapter-lifecycle.json', field, value); self.reject(f)

    def test_kernel_absence_cannot_be_incomplete_or_live(self):
        for mutation in ('omit', 'live', 'foreign_group', 'time'):
            f = Fixture(); value = json.loads(f.store.values[f.prefix+'/adapter-lifecycle.json'])
            if mutation == 'omit': value['kernel_absences'].pop()
            elif mutation == 'live': value['group_after']['members'] = [999]
            elif mutation == 'foreign_group': value['group_after']['process_group'] = 778
            else: value['group_after']['observed_monotonic_ns'] = 99
            self.rebind_record(f, 'adapter-lifecycle.json', None, value); self.reject(f)

    def test_missing_and_reordered_control_journal_rejected(self):
        for mutation in ('missing', 'order', 'receiver'):
            f = Fixture(); names = [p for p in f.store.values if p.endswith('.frame') and 'runner.runner_adapter.owner_to_child' in p]
            if mutation == 'missing': del f.store.values[names[0]]
            elif mutation == 'order': f.store.values[names[0]], f.store.values[names[1]] = f.store.values[names[1]], f.store.values[names[0]]
            else:
                received = names[0].replace('/runner.', '/adapter.')
                f.store.values[received] = f.store.values[names[1]]
            self.reject(f)

    def test_proposed_ack_is_not_validated_or_written_delivery(self):
        for phase in ('proposed', 'validated', 'pipe-written'):
            f = Fixture(); del f.store.values[f.prefix+f'/control/ack-000000-{phase}.json']; self.reject(f)

    def test_unstarted_session_cannot_hide_durable_start(self):
        f = Fixture(); f.packet['sessions'][0] = None; self.reject(f)

    def test_first_release_rejects_old_one_shot_packet(self):
        f = Fixture(); f.packet['attempts'] = []; self.reject(f)

    def test_unstarted_fault_prefix_blocks_a_later_benchmark_session(self):
        self.reject(Fixture(fault_prefix=True))
        self.reject(Fixture(fault_prefix=True, setup_failure=True))

    def test_unjoined_adapter_pump_cannot_close_the_session(self):
        f = Fixture(); self.rebind_closure(f, 'adapter_thread_joined', False); self.reject(f)

    def test_same_pid_requires_a_new_native_generation_in_the_next_session(self):
        for birth, accepted in ((10, False), (11, True)):
            first = Fixture(('succeeded',)*8)
            second = Fixture(('succeeded',)*8, session_index=1, worker_birth=birth)
            first.store.values.update(second.store.values)
            first.packet['sessions'][1] = second.packet['sessions'][1]
            first.samples.extend(second.samples)
            closure = json.loads(first.packet['closure'])
            closure.update(closed_ns=3*10**15, started_request_ids=first.starts+second.starts,
                started_session_ids=[first.descriptor['session_id'], second.descriptor['session_id']],
                session_closures=[first.closure_ref, second.closure_ref])
            first.packet['closure'] = accounting.accounting_canonical_bytes(closure)
            def replay(identity, attempt, bound, records):
                owner = first if identity['session_id'] == first.descriptor['session_id'] else second
                return owner.recompute(identity, attempt, bound, records)
            if accepted:
                self.assertEqual(first.reduce(validate_success=replay)['counts']['succeeded'], 16)
            else:
                with self.assertRaisesRegex(accounting.AccountingError, 'same native worker generation'):
                    first.reduce(validate_success=replay)

    def test_plan_rejects_wrong_warmup_counts_and_legacy_coordinates(self):
        for field, value in (('warmup_attempts', True), ('warmup_attempts', 0),
                             ('workload_manifest_sha256', '7'*64), ('measured_attempts', 0)):
            f = Fixture(); plan = copy.deepcopy(f.plan); plan['benchmark_sessions'][0][field] = value
            with self.assertRaises(accounting.AccountingError): accounting._retained_plan(plan)

    def test_owner_chain_rejects_overlapping_attempts_and_post_stop_work(self):
        def frames(kinds): return [SimpleNamespace(decoded=lambda kind=kind: {'kind': kind}) for kind in kinds]
        for kinds in (['accept'], ['dispatch', 'dispatch'], ['dispatch', 'stop', 'accept'],
                      ['dispatch', 'accept', 'accept']):
            with self.assertRaises(accounting.AccountingError):
                accounting._ordered_session_attempt_controls(frames(kinds))
        accounting._ordered_session_attempt_controls(frames(['dispatch', 'measurement_begin',
            'measurement_recorded', 'accept', 'dispatch', 'stop']))

    def test_exact_kernel_owner_predicates_survive_rebound_content(self):
        # Exercise the actual lifecycle owner beyond outer frame/hash rejection.
        for mutation in ('birth', 'parent', 'image', 'group', 'missing_pid', 'live_pid'):
            f = Fixture()
            closure = json.loads(f.store.values[f.closure_ref['path']])
            observed = json.loads(f.store.values[f.process_ready['path']])
            worker = observed['process_scope']['processes'][0]['identity']
            if mutation == 'birth': worker['birth'] = {'kind': 'pid_only'}
            elif mutation == 'parent': worker['ppid'] = 701
            elif mutation == 'image': worker['executable_sha256'] = '2'*64
            elif mutation == 'group': worker['pgid'] = 778
            elif mutation == 'missing_pid': observed['process_scope']['processes'].pop()
            else:
                lifecycle = json.loads(f.store.values[closure['adapter_lifecycle']['path']])
                lifecycle['kernel_absences'][0]['kernel_absence_observed'] = False
                path = closure['adapter_lifecycle']['path']; f.store.values[path] = control.canonical(lifecycle)
                closure['adapter_lifecycle'] = f.store.locate(path)
            f.store.values[f.process_ready['path']] = control.canonical(observed)
            closure['process_observation'] = f.store.locate(f.process_ready['path'])
            with self.subTest(mutation=mutation), self.assertRaises(accounting.AccountingError):
                accounting._session_lifetime(accounting._SessionRecords(f.store), closure,
                    f.identity, json.loads(f.store.read(f.started)), f.accepted,
                    f.rows[6]['attempt_id'], f.image, unready_attempts_absent=False)

    def test_lazy_provider_never_reads_opaque_logs_and_retains_empty_stderr(self):
        f = Fixture(); original = f.store.read
        f.store.values[f.prefix+'/worker.stderr.log'] = b''
        f.store.values[f.prefix+'/opaque-packet-chunk.pcap'] = b'x'*(17*1024*1024)
        def read(ref):
            self.assertNotIn(ref['path'], (f.prefix+'/worker.stderr.log', f.prefix+'/opaque-packet-chunk.pcap'))
            return original(ref)
        f.store.read = read
        self.assertEqual(f.reduce()['counts']['succeeded'], 6)

    def test_provider_rejects_wrong_content_new_files_and_raw_dictionary_fallback(self):
        f = Fixture(); f.packet['records'] = f.store.values; self.reject(f)
        f = Fixture(); original = f.store.read
        def wrong(ref): return original(ref) + b' '
        f.store.read = wrong; self.reject(f)
        f = Fixture(); original = f.recompute
        def appeared(*args):
            f.store.values[f.prefix+'/late.stderr.log'] = b''
            return original(*args)
        with self.assertRaisesRegex(accounting.AccountingError, 'inventory changed'):
            f.reduce(validate_success=appeared)

    def test_later_session_cannot_follow_failed_or_incomplete_session(self):
        f = Fixture(); f.packet['sessions'][1] = copy.deepcopy(f.packet['sessions'][0]); self.reject(f)

    def test_full_plan_failure_start_inventory_cannot_be_omitted(self):
        f = Fixture(); value = json.loads(f.packet['closure']); value['started_request_ids'].pop()
        f.packet['closure'] = accounting.accounting_canonical_bytes(value); self.reject(f)

    def test_sample_byte_format_is_bound_without_numeric_coercion(self):
        f = Fixture(); f.samples[0] = f.samples[0] + b'\n'; self.reject(f)

    @staticmethod
    def rebind_closure(f, field, value):
        path = f.closure_ref['path']; closure = json.loads(f.store.values[path]); closure[field] = value
        f.store.values[path] = control.canonical(closure); new = f.store.locate(path)
        f.packet['sessions'][0]['closure'] = new
        campaign = json.loads(f.packet['closure']); campaign['session_closures'][0] = new
        f.packet['closure'] = accounting.accounting_canonical_bytes(campaign)

    @staticmethod
    def rebind_record(f, name, field, replacement):
        path = f.prefix+'/'+name; value = json.loads(f.store.values[path])
        if field is None: value = replacement
        else: value[field] = replacement
        f.store.values[path] = control.canonical(value)
        key = {'adapter-lifecycle.json': 'adapter_lifecycle', 'worker-terminal.json': 'worker_terminal'}[name]
        RetainedAccountingTests.rebind_closure(f, key, f.store.locate(path))



class RetainedAttemptTerminalTests(unittest.TestCase):
    """Exact attempt payloads remain distinct from the single session exit."""
    def setUp(self):
        self.fixture=Fixture(('succeeded',))
        row=self.fixture.rows[0]
        self.request=json.loads(self.fixture.store.read(row['request']))
        self.digest=row['request']['sha256']
        self.good=json.loads(self.fixture.store.values[row['output_directory']+'/evidence/benchmark-protocol/rust-result.json'])

    def value(self,kind):
        result=copy.deepcopy(self.good)
        if kind=='failed':result['outcome']={'kind':kind,'stage':'benchmark_worker','reason':'execution_error'}
        if kind=='timed_out':result['outcome']={'kind':kind,'stage':'private_receipt','budget_ms':300000,'elapsed_ms':300000}
        return result

    def validate(self,value):
        accounting._terminal_without_exit(value,self.request,self.digest)

    def test_all_declared_outcomes_and_deadline_stages(self):
        self.validate(self.value('succeeded'))
        for reason in accounting.WORKER_FAILURE_REASONS:
            value=self.value('failed');value['outcome']['reason']=reason;self.validate(value)
        for stage in accounting.DEADLINE_STAGES:
            value=self.value('timed_out');value['outcome']['stage']=stage;self.validate(value)

    def test_identity_and_full_field_inventory_are_mandatory(self):
        for key in self.good:
            value=copy.deepcopy(self.good);del value[key]
            with self.subTest(missing=key),self.assertRaises(accounting.AccountingError):self.validate(value)
        for key in ('request_id','invocation_nonce','request_sha256','commit','participants'):
            value=copy.deepcopy(self.good);value[key]=3 if key=='participants' else 'b'*len(value[key])
            with self.subTest(substituted=key),self.assertRaises(accounting.AccountingError):self.validate(value)
        for path in ((),('outcome',),('outcome','result')):
            value=copy.deepcopy(self.good);owner=value
            for key in path:owner=owner[key]
            owner['unlisted']=True
            with self.assertRaises(accounting.AccountingError):self.validate(value)
        for key,replacement in (('version',True),('version',1.0),('participants',3.0),('request_sha256','2'*64)):
            value=copy.deepcopy(self.good);value['outcome']['result'][key]=replacement
            with self.assertRaises(accounting.AccountingError):self.validate(value)

    def test_deadline_requires_a_typed_stage_and_actual_exhausted_duration(self):
        for key,replacement in (('stage','text says timed out'),('budget_ms',0),('budget_ms',300001),
                ('elapsed_ms',299999),('elapsed_ms',300001),('elapsed_ms',True),('budget_ms',1.0),('elapsed_ms',1<<64)):
            value=self.value('timed_out');value['outcome'][key]=replacement
            with self.subTest(key=key,replacement=replacement),self.assertRaises(accounting.AccountingError):self.validate(value)
        value=self.value('failed');value['outcome']['reason']='the request timed out'
        with self.assertRaises(accounting.AccountingError):self.validate(value)

    def test_non_success_cannot_include_measurements(self):
        for kind in ('failed','timed_out'):
            value=self.value(kind);value['outcome']['result']=self.good['outcome']['result']
            with self.assertRaises(accounting.AccountingError):self.validate(value)

    def test_attempt_terminal_cannot_claim_its_own_process_exit(self):
        for code in (0,101,-9,False):
            value=copy.deepcopy(self.good);value['exit_code']=code
            with self.assertRaises(accounting.AccountingError):self.validate(value)

if __name__ == '__main__': unittest.main()
