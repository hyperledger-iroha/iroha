"""Offline routine-update contracts; no Cargo, SSH or runtime signing inputs.

Run from a normal checkout with python3 scripts/tests/taira_update_test.py.
"""
import argparse
import ast
import base64
from contextlib import ExitStack, redirect_stdout
import copy
import fcntl
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import taira_update as runner
import taira_validator_unit as renderer

# In a normal checkout these resolve to the same scripts directory. Keeping
# dependencies module-owned also permits an isolated source overlay in review.
ROOT = Path(renderer.__file__).resolve().parents[1]
runner.ROOT = ROOT
GUEST_FILE = SCRIPTS / 'taira_update_guest.py'
OPERATION = 'update-' + '1' * 32


def fresh_guest():
    spec = importlib.util.spec_from_file_location('taira_update_guest_test', GUEST_FILE)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def deployment():
    return {'schema':'taira.runtime-deployment.v1', 'guest_ssh':{'argv':[], 'pins':[]},
        'runtime_root':'/private/runtime/taira', 'state_root':'/var/lib/taira',
        'config_root':'/srv/taira', 'config_release':'a'*40,
        'genesis_manifest':'/private/runtime/taira/genesis/genesis.json',
        'network_id':'hash:fixture', 'public_origin':'https://test.example',
        'roles':[f'taira-validator-{i}' for i in range(1,5)], 'ports':[8080,8081,8082,8083],
        'replay_floor':146, 'renderer_sha256':runner.sha((ROOT/'scripts/taira_validator_unit.py').read_bytes()),
        'current':{'commit':'b'*40, 'daemon':'/private/runtime/taira/selected/bin/iroha3d_taira',
          'attempt_name':'retained', 'plan_schema':'taira.daemon-update.plan.v1',
          'result_schema':'taira.daemon-update.result.v1', 'local_plan':'/owner/prior.json',
          'local_plan_sha256':'c'*64}}


def installed_unit(role):
    raw=renderer.render(role, '/fixture/retained/key', '/fixture/retained/seed').encode()
    old=f'/srv/taira/{role}/current/bin/iroha3d_taira'.encode()
    return raw.replace(old, deployment()['current']['daemon'].encode(), 1)


def fixture():
    value=deployment()
    build={'commit':'a'*40, 'target':'aarch64-unknown-linux-gnu', 'profile':'release',
        'environment_sha256':'e'*64,
        'jobs':6, 'source_unchanged':True, 'toolchain_unchanged':True,
        'deployed':False, 'release_qualified':False, 'native_check_scope':'basic',
        'artifacts':[{'name':name,'package':package,'path':'/public/'+name,
                      'sha256':'b'*64,'size':2_000_000}
            for name,package in [('iroha3d_taira','irohad'),('iroha','iroha_cli'),
                                 ('kagami','iroha_kagami'),('sorafs-node','sorafs_node')]]}
    prior={'schema':value['current']['plan_schema'], 'commit':value['current']['commit'],
        'network_id':value['network_id'], 'renderer_sha256':value['renderer_sha256'],
        'units':[{'role':role, 'after':base64.b64encode(installed_unit(role)).decode(),
                  'after_sha256':runner.sha(installed_unit(role))} for role in value['roles']]}
    return build,prior


def plan_for(build=None, prior=None, value=None, failed_start=None):
    global guest
    if build is None or prior is None:
        default_build,default_prior=fixture()
        build=default_build if build is None else build
        prior=default_prior if prior is None else prior
    guest=fresh_guest()
    return runner.make_plan(build, deployment() if value is None else value, prior, guest,
                            OPERATION, failed_start)


def failed_fixture(value=None, prior=None):
    build, default_prior = fixture()
    value = deployment() if value is None else value
    prior = default_prior if prior is None else prior
    failed = runner.make_plan(build, value, prior, fresh_guest(), 'update-' + '2' * 32)
    before = [{'role': role, 'unit_stamp': [1, 2, 0o100600, 0, 0, 1, 3, 4, 5],
               'config_stamp': [1, 4], 'state_root_identity': [1, 8], 'current_target': 'unchanged',
               'public': {'commit': value['current']['commit'], 'network_id': value['network_id'],
                          'height': 199}, 'systemd': {'InvocationID': 'e' * 32}}
              for role in value['roles']]
    checkpoints = [{'role': role, 'cohort_stopped': True, 'invocation_id': 'e' * 32,
                    'checkpoint_height': 199, 'kura_tip': {'height': 200, 'hash': 'c' * 64},
                    'selection': {'selector': 'b' * 64, 'pointer_stamp': [0] * 7 + [90_000]},
                    'native_events': [{'time_us': 100,
                                       'message': 'Successfully created a snapshot of state at_height=199'}],
                    'proof_invocation_id': 'e' * 32}
                   for role in value['roles']]
    records = {'intent.json': failed, 'before.json': before, 'checkpoint-stopped.json': checkpoints,
               'start-intent.json': {'units': [f'iroha3d-{role}.service' for role in value['roles']],
                                    'automatic_old_binary_rollback_after_start': False},
               'failure.json': {'error': 'native startup failure', 'new_start_attempted': True,
                                'installed_units': value['roles']}}
    return failed, records


def write_failed_reference(directory, records):
    directory = Path(directory).resolve()
    directory.mkdir(exist_ok=True)
    refs = {}
    for name, value in records.items():
        path = directory / name
        raw = (json.dumps(value, sort_keys=True) + '\n').encode()
        path.write_bytes(raw)
        refs[name] = {'path': str(path), 'sha256': runner.sha(raw)}
    return {'schema': 'taira.failed-start-chain.v1', 'attempts': [
        {'operation': records['intent.json']['operation'],
         'plan': refs['intent.json'], 'records': refs}]}


def failed_chain_fixture(directory, commits=('a', 'c'), historical_second=False):
    """Create actual sequential failed plans, retaining completed baseline health."""
    build, prior = fixture()
    _, template = failed_fixture()
    chain = {'schema': 'taira.failed-start-chain.v1', 'attempts': []}
    entries = []
    for index, source in enumerate(commits):
        build['commit'] = source * 40
        failed = runner.make_plan(copy.deepcopy(build), deployment(), prior, fresh_guest(),
            'update-' + f'{index + 2:032x}', copy.deepcopy(chain) if index else None)
        if historical_second and index == 1:
            first = chain['attempts'][0]
            failed['failed_start'] = {'schema': 'taira.failed-start-reference.v1',
                'plan': first['plan'], 'records': first['records'],
                'installed': failed['failed_start']['installed']}
        records = copy.deepcopy(template)
        records['intent.json'] = failed
        for before, checkpoint in zip(records['before.json'], records['checkpoint-stopped.json']):
            before['systemd']['InvocationID'] = f'{index + 10:032x}'
            checkpoint['invocation_id'] = before['systemd']['InvocationID']
            # A failed startup may reuse the same authenticated checkpoint.
            checkpoint['proof_invocation_id'] = 'e' * 32
        ref = write_failed_reference(Path(directory) / failed['operation'], records)['attempts'][0]
        chain['attempts'].append(ref)
        entries.append((failed, records))
    return build, prior, chain, entries


def local_inputs(directory):
    build,prior=fixture();value=deployment()
    root=Path(directory).resolve()
    prior_path=root/'prior.json';prior_raw=json.dumps(prior).encode();prior_path.write_bytes(prior_raw)
    value['current'].update(local_plan=str(prior_path),local_plan_sha256=runner.sha(prior_raw))
    deployment_path=root/'deployment.json';deployment_path.write_text(json.dumps(value))
    build_path=root/'result.json';build_path.write_text(json.dumps(build))
    return value,build,deployment_path,build_path


def cli_argv(deployment_path,build_path,output):
    return ['taira_update.py','--deployment',str(deployment_path),'--prepared-result',str(build_path),
            '--output',str(output),'--plan-only']


class CoordinatorTests(unittest.TestCase):
    def setUp(self):
        plan_for()

    def test_cli_failed_start_uses_installed_units_without_promoting_failed_health(self):
        with tempfile.TemporaryDirectory() as temporary:
            value, build, descriptor, result = local_inputs(temporary)
            prior = json.loads(Path(value['current']['local_plan']).read_bytes())
            failed, records = failed_fixture(value, prior)
            reference = write_failed_reference(descriptor.parent / 'failed', records)
            reference_path = descriptor.parent / 'failed-reference.json'
            reference_path.write_text(json.dumps(reference))
            build['commit'] = 'c' * 40
            result.write_text(json.dumps(build))
            output = descriptor.parent / 'corrective.json'
            with patch.object(sys, 'argv', cli_argv(descriptor, result, output) +
                              ['--failed-start-chain', str(reference_path)]), \
                 patch.object(runner.subprocess, 'check_output', return_value='optimizations\n'), \
                 patch.object(runner.retry, 'validate_ssh', return_value=['approved']), \
                 patch.object(runner.subprocess, 'run') as remote, \
                 patch.object(runner, 'apply_plan') as apply, redirect_stdout(io.StringIO()):
                runner.main()
            remote.assert_not_called(); apply.assert_not_called()
            plan = json.loads(output.read_bytes())
            self.assertEqual(plan['deployment'], value)
            self.assertEqual(plan['retained_predecessor'], failed['retained_predecessor'])
            self.assertEqual(plan['failed_start']['installed']['commit'], failed['commit'])
            self.assertEqual(plan['failed_start']['attempts'][-1]['records'], reference['attempts'][-1]['records'])
            for previous, current in zip(failed['units'], plan['units'], strict=True):
                self.assertEqual(current['before'], previous['after'])
                self.assertEqual(current['before_sha256'], previous['after_sha256'])
            self.assertNotIn('accepted_health', plan['failed_start'])
            self.assertNotEqual(runner.release_name(plan), runner.release_name(failed))
            with patch.object(sys, 'argv', cli_argv(descriptor, result, output) +
                              ['--failed-start-chain', str(reference_path)]), \
                 patch.object(runner.subprocess, 'check_output', return_value='optimizations\n'), \
                 patch.object(runner.retry, 'validate_ssh', return_value=['approved']), \
                 patch.object(runner.subprocess, 'run') as remote:
                with self.assertRaises(FileExistsError): runner.main()
            remote.assert_not_called()
            self.assertEqual(plan, json.loads(output.read_bytes()))

    def test_failed_start_planning_rejects_unproven_or_inconsistent_failure_lineage(self):
        for failure in ('missing', 'digest', 'predecessor', 'unit', 'artifact', 'not_started',
                        'partial_install', 'checkpoint', 'invocation', 'intent', 'same_operation', 'nested'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                build, prior = fixture()
                build['commit'] = 'c' * 40
                failed, records = failed_fixture()
                if failure == 'predecessor': failed['retained_predecessor']['intent_sha256'] = 'd' * 64
                if failure == 'unit': failed['units'][0]['before_sha256'] = 'd' * 64
                if failure == 'artifact': failed['artifacts'][0]['package'] = 'wrong'
                if failure == 'not_started': records['failure.json']['new_start_attempted'] = False
                if failure == 'partial_install': records['failure.json']['installed_units'] = []
                if failure == 'checkpoint': records['checkpoint-stopped.json'][0]['kura_tip']['height'] = 198
                if failure == 'invocation': records['checkpoint-stopped.json'][0]['invocation_id'] = 'd' * 32
                if failure == 'same_operation': failed['operation'] = OPERATION
                if failure == 'nested': failed['failed_start'] = {}
                reference = write_failed_reference(temporary, records)
                if failure == 'missing': Path(reference['attempts'][-1]['records']['failure.json']['path']).unlink()
                if failure == 'digest': reference['attempts'][-1]['records']['failure.json']['sha256'] = 'd' * 64
                if failure == 'intent': reference['attempts'][-1]['plan'] = dict(reference['attempts'][-1]['plan'], sha256='d' * 64)
                with patch.object(runner.subprocess, 'run') as remote:
                    with self.assertRaises((RuntimeError, FileNotFoundError)):
                        plan_for(build, prior, failed_start=reference)
                remote.assert_not_called()

    def test_cli_plan_only_binds_the_completed_build_and_exact_predecessor_without_ssh(self):
        with tempfile.TemporaryDirectory() as temporary:
            value,build,descriptor,result=local_inputs(temporary)
            output=descriptor.parent/'plan.json'
            with patch.object(sys,'argv',cli_argv(descriptor,result,output)), \
                 patch.object(runner.subprocess,'check_output',return_value='optimizations\n'), \
                 patch.object(runner.retry,'validate_ssh',return_value=['approved']), \
                 patch.object(runner.subprocess,'run') as remote, \
                 patch.object(runner,'apply_plan') as apply, redirect_stdout(io.StringIO()) as printed:
                runner.main()
            remote.assert_not_called();apply.assert_not_called()
            self.assertFalse(json.loads(printed.getvalue())['host_contacted'])
            plan=json.loads(output.read_bytes())
            self.assertEqual(plan['commit'],build['commit'])
            self.assertEqual(plan['build_result_path'],str(result))
            self.assertEqual(plan['build_result_sha256'],runner.sha(result.read_bytes()))
            self.assertEqual(plan['deployment'],value)
            self.assertEqual([row['name'] for row in plan['artifacts']],['iroha3d_taira','iroha'])
            self.assertEqual([row['role'] for row in plan['units']],value['roles'])
            self.assertEqual(stat.S_IMODE(output.stat().st_mode),0o600)
            prior=json.loads(Path(value['current']['local_plan']).read_bytes())
            self.assertEqual(plan['retained_predecessor']['intent_sha256'],
                runner.sha(json.dumps(prior,sort_keys=True,separators=(',',':')).encode()))
            self.assertFalse(plan['transaction_submission'])

    def test_cli_rejects_missing_failed_or_incomplete_preparation_without_transfer(self):
        for failure in ('missing','failed','source_changed','artifact_missing'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                _,build,descriptor,result=local_inputs(temporary)
                if failure=='missing':result.unlink()
                else:
                    if failure=='failed':build['exit_code']=1
                    elif failure=='source_changed':build['source_unchanged']=False
                    else:build['artifacts'].pop()
                    result.write_text(json.dumps(build))
                output=descriptor.parent/'plan.json'
                with patch.object(sys,'argv',cli_argv(descriptor,result,output)), \
                     patch.object(runner.subprocess,'check_output',return_value='optimizations\n'), \
                     patch.object(runner.retry,'validate_ssh',return_value=['approved']), \
                     patch.object(runner.subprocess,'run') as remote, \
                     patch.object(runner,'apply_plan') as apply:
                    with self.assertRaises((RuntimeError,FileNotFoundError)):runner.main()
                remote.assert_not_called();apply.assert_not_called()
                self.assertFalse(output.exists())

    def test_cli_rejects_stale_predecessor_or_changed_custody_before_plan_publication(self):
        for failure in ('digest','commit','custody'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                value,_,descriptor,result=local_inputs(temporary)
                prior_path=Path(value['current']['local_plan'])
                prior=json.loads(prior_path.read_bytes())
                if failure=='commit':prior['commit']='e'*40
                elif failure=='custody':
                    raw=base64.b64decode(prior['units'][0]['after']).replace(b'--sora',b'--wrong')
                    prior['units'][0].update(after=base64.b64encode(raw).decode(),after_sha256=runner.sha(raw))
                else:prior['unknown']='changed after binding'
                prior_path.write_text(json.dumps(prior))
                if failure!='digest':
                    value['current']['local_plan_sha256']=runner.sha(prior_path.read_bytes())
                    descriptor.write_text(json.dumps(value))
                output=descriptor.parent/'plan.json'
                with patch.object(sys,'argv',cli_argv(descriptor,result,output)), \
                     patch.object(runner.subprocess,'check_output',return_value='optimizations\n'), \
                     patch.object(runner.retry,'validate_ssh',return_value=['approved']), \
                     patch.object(runner.subprocess,'run') as remote, \
                     patch.object(runner,'apply_plan') as apply:
                    with self.assertRaises(RuntimeError):runner.main()
                remote.assert_not_called();apply.assert_not_called()
                self.assertFalse(output.exists())

    def test_local_lock_contention_prevents_plan_or_transfer(self):
        with tempfile.TemporaryDirectory() as temporary:
            _,_,descriptor,result=local_inputs(temporary)
            output=descriptor.parent/'plan.json'
            lock=os.open(descriptor.parent/'.taira-update.lock',os.O_RDWR|os.O_CREAT,0o600)
            try:
                fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
                with patch.object(sys,'argv',cli_argv(descriptor,result,output)), \
                     patch.object(runner.subprocess,'check_output',return_value='optimizations\n'), \
                     patch.object(runner.retry,'validate_ssh',return_value=['approved']), \
                     patch.object(runner.subprocess,'run') as remote, \
                     patch.object(runner,'apply_plan') as apply:
                    with self.assertRaises(BlockingIOError):runner.main()
                remote.assert_not_called();apply.assert_not_called()
                self.assertFalse(output.exists())
            finally:os.close(lock)

    def test_indirect_or_shared_public_inputs_and_output_paths_are_rejected(self):
        for failure in ('descriptor_symlink','descriptor_hardlink','output_parent_symlink'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                _,_,descriptor,result=local_inputs(temporary)
                output=descriptor.parent/'plan.json'
                if failure=='output_parent_symlink':
                    alias=descriptor.parent/'alias';alias.symlink_to(descriptor.parent,target_is_directory=True)
                    output=alias/'plan.json'
                else:
                    alias=descriptor.with_name('alias.json')
                    if failure=='descriptor_symlink':alias.symlink_to(descriptor)
                    else:os.link(descriptor,alias)
                    descriptor=alias
                with patch.object(sys,'argv',cli_argv(descriptor,result,output)), \
                     patch.object(runner.subprocess,'check_output',return_value='optimizations\n'), \
                     patch.object(runner.retry,'validate_ssh',return_value=['approved']), \
                     patch.object(runner.subprocess,'run') as remote:
                    with self.assertRaises(RuntimeError):runner.main()
                remote.assert_not_called()
                self.assertFalse(output.exists())

    def test_guest_lock_denies_contention_and_linked_lock_before_any_state_operation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();plan={'deployment':{'runtime_root':str(root)}}
            path=root/'.routine-update.lock'
            actual_fstat=os.fstat
            def root_stat(fd):
                value=actual_fstat(fd)
                return SimpleNamespace(st_mode=value.st_mode,st_uid=0,st_nlink=value.st_nlink,
                                       st_dev=value.st_dev,st_ino=value.st_ino)
            def root_stamp(path,directory=False):
                value=Path(path).lstat()
                return [value.st_dev,value.st_ino,value.st_mode,0,value.st_gid,value.st_nlink]
            held=os.open(path,os.O_RDWR|os.O_CREAT,0o600)
            try:
                fcntl.flock(held,fcntl.LOCK_EX|fcntl.LOCK_NB)
                with patch.object(guest,'stamp',side_effect=root_stamp), \
                     patch.object(guest.os,'fstat',side_effect=root_stat),patch.object(guest,'apply') as apply:
                    with self.assertRaises(BlockingIOError):guest.apply_locked(plan)
                    apply.assert_not_called()
            finally:os.close(held)
            alias=root/'shared-lock';os.link(path,alias)
            with patch.object(guest,'stamp',side_effect=root_stamp), \
                 patch.object(guest.os,'fstat',side_effect=root_stat),patch.object(guest,'apply') as apply:
                with self.assertRaisesRegex(RuntimeError,'invalid guest update lock'):guest.apply_locked(plan)
                apply.assert_not_called()
                alias.unlink()
                guest.apply_locked(plan)
                apply.assert_called_once_with(plan)
    def test_unit_update_preserves_complete_custody_and_changes_only_daemon_path(self):
        for role in guest.ROLES:
            before = installed_unit(role)
            after = guest.replace_daemon(before, role)
            old = str(guest.PREVIOUS_DAEMON).encode()
            self.assertEqual(after.replace(str(guest.DAEMON).encode(), old), before)
            self.assertEqual(guest.unit_command(after)[1:], guest.unit_command(before)[1:])
            self.assertIn(renderer.CUSTODY.splitlines()[0].encode(), before)

    def test_unexpected_executable_or_extra_occurrence_is_rejected(self):
        raw = installed_unit(guest.ROLES[0])
        with self.assertRaisesRegex(RuntimeError, 'old unit command differs'):
            guest.replace_daemon(raw.replace(b'--sora', b'--other'), guest.ROLES[0])
        with self.assertRaisesRegex(RuntimeError, 'old unit command differs'):
            guest.replace_daemon(raw + b'\n# ' + str(guest.PREVIOUS_DAEMON).encode() + b'\n', guest.ROLES[0])

    def test_plan_selects_daemon_and_same_revision_cli_from_completed_build_and_exact_four_units(self):
        build, metadata = fixture()
        plan = plan_for(build, metadata)
        self.assertEqual([a['name'] for a in plan['artifacts']], ['iroha3d_taira', 'iroha'])
        self.assertEqual(len(plan['units']), 4)
        for row in plan['units']:
            self.assertEqual(guest.replace_daemon(base64.b64decode(row['before']), row['role']),
                             base64.b64decode(row['after']))
        for field, value in [('commit', deployment()['current']['commit']), ('source_unchanged', False), ('target', 'x86_64-unknown-linux-gnu')]:
            invalid = copy.deepcopy(build)
            invalid[field] = value
            with self.assertRaises(RuntimeError):
                plan_for(invalid, metadata)
        metadata['units'][0]['after_sha256'] = '0' * 64
        with self.assertRaisesRegex(RuntimeError, 'predecessor unit digest differs'):
            plan_for(build, metadata)

    def test_config_diagnostics_are_native_file_descriptors_only(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(guest, 'ATTEMPT', Path(directory)), \
                patch.object(guest, 'record'), patch.object(guest.subprocess, 'run') as run:
            run.return_value = subprocess.CompletedProcess([], 0)
            guest.native_private_command(['/native/daemon', '--config', '/private/config'], timeout=3, name='config')
            kwargs = run.call_args.kwargs
            self.assertIs(type(kwargs['stdout']), int)
            self.assertIs(type(kwargs['stderr']), int)
            self.assertNotIn('capture_output', kwargs)

    def test_transfer_creates_release_once_then_adds_same_revision_cli(self):
        first = runner.transfer_code('iroha3d_taira', True, plan_for())
        second = runner.transfer_code('iroha', False, plan_for())
        self.assertIn('if True:', first)
        self.assertIn('if False:', second)
        self.assertIn("bins/'iroha'", second)
        compile(first, '<native-transfer-daemon>', 'exec')
        compile(second, '<native-transfer-cli>', 'exec')
        with self.assertRaises(RuntimeError):
            runner.transfer_code('../config', False, plan_for())
        self.assertIn("release=base/" + repr(runner.release_name(plan_for())), first)


    def test_artifact_transfer_compression_preserves_the_approved_ssh_route(self):
        approved = ['/usr/bin/ssh', '-F', '/dev/null', '-o', 'UserKnownHostsFile=/approved/hosts',
                    '-o', 'ProxyCommand=/approved/proxy', 'root@192.168.64.3', '/usr/bin/python3 -I -']
        transfer = runner.artifact_transfer_argv(approved, '/reviewed/transfer')
        self.assertEqual(transfer, [approved[0], '-C', *approved[1:-1], '/reviewed/transfer'])
        self.assertNotIn('-C', approved)
        self.assertEqual(approved[-1], '/usr/bin/python3 -I -')

    def test_snapshot_journal_is_ordered_by_timestamp_before_selecting_latest(self):
        invocation = "a" * 32
        records = [
            {"_SYSTEMD_INVOCATION_ID": invocation, "__REALTIME_TIMESTAMP": str(t),
             "MESSAGE": f"Successfully created a snapshot of state at_height={h}"}
            for t, h in [(300, 217), (100, 17), (200, 180)]
        ]
        raw = "\n".join(json.dumps(row) for row in records).encode()
        with patch.object(guest.subprocess, 'run',
                          return_value=subprocess.CompletedProcess([], 0, stdout=raw, stderr=b'')):
            events = guest.snapshot_events("taira-validator-1", invocation)
        self.assertEqual([row["time_us"] for row in events], [100, 200, 300])
        self.assertIn("at_height=217", events[-1]["message"])

    def test_empty_snapshot_journal_accepts_only_clean_no_match(self):
        with patch.object(guest.subprocess, 'run') as query:
            query.return_value = subprocess.CompletedProcess([], 1, stdout=b'', stderr=b'')
            self.assertEqual(guest.snapshot_events(guest.ROLES[0], 'a' * 32), [])
            self.assertIn('_SYSTEMD_INVOCATION_ID=' + 'a' * 32, query.call_args.args[0])
            for status, out, err in [(2, b'', b''), (1, b'partial', b''),
                                     (1, b'', b'journal failure'), (1, b'', b'\n')]:
                query.return_value = subprocess.CompletedProcess([], status, stdout=out, stderr=err)
                with self.assertRaisesRegex(RuntimeError, 'snapshot journal query failed'):
                    guest.snapshot_events(guest.ROLES[0], 'a' * 32)

    def test_checkpoint_retains_authenticated_pre_boundary_proof_and_later_kura_tip(self):
        obs = {'role': guest.ROLES[0], 'systemd': {'InvocationID': 'a' * 32}, 'public': {'height': 221}}
        selected = {'pointer_stamp': [0] * 7 + [90_000], 'selector': 'b' * 64}
        published = {'time_us': 100, 'message': 'Successfully created a snapshot of state at_height=215'}
        tip = {'height': 221, 'hash': 'c' * 64}
        with patch.object(guest, 'snapshot_selection', return_value=selected) as selection, \
             patch.object(guest, 'native_kura_tip', return_value=tip), \
             patch.object(guest, 'native_kura_hash', return_value=tip['hash']) as retained_hash, \
             patch.object(guest, 'snapshot_events', return_value=[published]) as logs:
            prior = guest.checkpoint_barrier(obs)
            self.assertEqual(prior['checkpoint_height'], 215)
            self.assertFalse(prior['standalone_native_manifest_verification'])
            # Failed shutdown publication does not invalidate an intact authenticated generation.
            logs.return_value = [published, {'time_us': 120, 'message': 'Failed to create a snapshot of state'}]
            stopped = guest.checkpoint_barrier(obs, stopped=True, prior=prior)
            self.assertTrue(stopped['cohort_stopped'])
            self.assertFalse(stopped['graceful_stop_observed'])
            self.assertEqual(stopped['kura_tip'], tip)
            # An invocation with no publication uses the retained proof, never invents one.
            obs['systemd']['InvocationID'] = 'd' * 32
            logs.return_value = []
            reused = guest.checkpoint_barrier(obs, stopped=True, prior=prior)
            self.assertTrue(reused['reused_checkpoint_proof'])
            self.assertEqual(reused['proof_invocation_id'], 'a' * 32)
            self.assertEqual(reused['native_events'], [published])
            # Proof remains usable across another attempt, with its original provenance.
            again = guest.checkpoint_barrier(obs, stopped=True, prior=reused)
            self.assertEqual(again['proof_invocation_id'], 'a' * 32)
            retained_hash.return_value = 'e' * 64
            with self.assertRaisesRegex(RuntimeError, 'prefix hash changed'):
                guest.checkpoint_barrier(obs, stopped=True, prior=prior)
            retained_hash.return_value = tip['hash']
            selection.return_value = dict(selected, selector='f' * 64)
            with self.assertRaisesRegex(RuntimeError, 'selection changed'):
                guest.checkpoint_barrier(obs, prior=prior)
            selection.return_value = selected
            logs.return_value = [{'time_us': 100, 'message': 'Successfully created a snapshot of state at_height=145'}]
            with self.assertRaisesRegex(RuntimeError, 'precedes affected history'):
                guest.checkpoint_barrier(obs)
            logs.return_value = []
            with self.assertRaisesRegex(RuntimeError, 'no native authenticated'):
                guest.checkpoint_barrier(obs)

    def test_new_invocation_must_restore_authenticated_checkpoint_without_historical_replay(self):
        obs = {'role': guest.ROLES[0], 'systemd': {'InvocationID': 'd' * 32}, 'public': {'height': 201}}
        cp = {'checkpoint_height': 200, 'kura_tip': {'height': 201, 'hash': 'a' * 64}}
        events = [{'message': 'Validated snapshot block hashes against Kura snapshot_height=200 kura_height=201'},
                  {'message': 'Successfully loaded the state from a snapshot at_height=200'},
                  {'message': 'Replaying authenticated complete Kura prefix start_height=201 generic_replay_height=201'}]
        with patch.object(guest, 'snapshot_events', return_value=events) as logs, \
             patch.object(guest, 'native_kura_hash', return_value='a' * 64) as native_hash:
            result = guest.verify_restored_checkpoint(obs, cp)
            self.assertEqual(result['restored_height'], 200)
            logs.assert_called_with(obs['role'], 'd' * 32)
            native_hash.assert_called_with(obs['role'], 201)
            native_hash.return_value = 'b' * 64
            with self.assertRaisesRegex(RuntimeError, 'prefix hash changed'):
                guest.verify_restored_checkpoint(obs, cp)
            native_hash.return_value = 'a' * 64
            for bad in [[], events[:1], events + [{'message': 'Failed to load state snapshot'}],
                        events[:2] + [{'message': 'Replaying authenticated complete Kura prefix start_height=146'}],
                        [{'message': 'Validated snapshot block hashes against Kura snapshot_height=145'},
                         {'message': 'Successfully loaded the state from a snapshot at_height=145'}]]:
                logs.return_value = bad
                with self.assertRaises(RuntimeError):
                    guest.verify_restored_checkpoint(obs, cp)

    def test_current_invocation_strict_restore_authenticates_a_newer_selected_generation(self):
        observation = {'role': guest.ROLES[0], 'systemd': {'InvocationID': 'f' * 32},
                       'public': {'height': 221}}
        selected = {'pointer_stamp': [0] * 7 + [200_000], 'selector': 'b' * 64}
        prior = {'role': guest.ROLES[0], 'selection': {'selector': 'older-generation'},
                 'kura_tip': {'height': 219, 'hash': 'c' * 64}}
        events = [{'time_us': 300, 'message': 'Validated snapshot block hashes against Kura snapshot_height=221'},
                  {'time_us': 301, 'message': 'Successfully loaded the state from a snapshot at_height=221'}]
        with patch.object(guest, 'snapshot_selection', return_value=selected), \
             patch.object(guest, 'native_kura_tip', return_value={'height': 221, 'hash': 'd' * 64}), \
             patch.object(guest, 'native_kura_hash', return_value='c' * 64), \
             patch.object(guest, 'snapshot_events', return_value=events) as logs:
            result = guest.checkpoint_barrier(observation, stopped=True, prior=prior)
            self.assertEqual(result['proof_kind'], 'native_strict_restore')
            self.assertEqual(result['checkpoint_height'], 221)
            self.assertEqual(result['proof_invocation_id'], 'f' * 32)
            self.assertFalse(result['reused_checkpoint_proof'])
            self.assertNotIn('native_publication_height', result)
            logs.assert_called_with(guest.ROLES[0], 'f' * 32)
            logs.return_value = events[1:]
            with self.assertRaisesRegex(RuntimeError, 'authentication missing'):
                guest.checkpoint_barrier(observation, stopped=True, prior=prior)
            logs.return_value = events
            selected['pointer_stamp'][7] = 302_000
            with self.assertRaisesRegex(RuntimeError, 'replaced after native'):
                guest.checkpoint_barrier(observation, stopped=True, prior=prior)

    def test_stop_accepts_terminated_failed_services_and_records_their_failure(self):
        clean = {'ActiveState': 'inactive', 'SubState': 'dead', 'MainPID': '0',
                 'ControlPID': '0', 'Job': '', 'Result': 'success', 'ExecMainCode': '1', 'ExecMainStatus': '0'}
        failed = dict(clean, ActiveState='failed', SubState='failed', Result='exit-code', ExecMainStatus='1')
        with patch.object(guest, 'command') as command, \
             patch.object(guest, 'systemd', side_effect=[clean, failed, clean, failed]) as states, \
             patch.object(guest, 'record') as record:
            observed = guest.stop_all()
            self.assertEqual([row['clean_exit'] for row in observed], [True, False, True, False])
            self.assertEqual(observed[1]['systemd']['ExecMainStatus'], '1')
            record.assert_called_once_with('stop-observations.json', observed)
            self.assertEqual(command.call_args.args[0], ['/usr/bin/systemctl', 'stop', *guest.UNITS])
            for bad in [dict(failed, MainPID='42'), dict(failed, ControlPID='43'),
                        dict(failed, Job='123'), dict(clean, SubState='stop-sigterm')]:
                states.side_effect = [bad]
                with self.assertRaisesRegex(RuntimeError, 'cohort stop incomplete'):
                    guest.stop_all()

    def test_cohort_retry_waits_for_process_http_and_readiness_under_one_deadline(self):
        props = {'ActiveState': 'active', 'SubState': 'running', 'ControlPID': '0',
                 'MainPID': '42', 'InvocationID': 'a' * 32}
        old = {'role': guest.ROLES[0], 'config_stamp': [1], 'config_sha256': 'same',
               'state_root_identity': [2], 'current_target': 'same', 'systemd': props,
               'public': {'height': 221, 'commit': guest.OLD}}
        tip = {'height': 221, 'hash': 'c' * 64}
        now = [0.0]
        with patch.object(guest.time, 'monotonic', side_effect=lambda: now[0]), \
             patch.object(guest.time, 'sleep', side_effect=lambda seconds: now.__setitem__(0, now[0] + seconds)), \
             patch.object(guest, 'observe', side_effect=[RuntimeError('curl before HTTP start'),
                                                      old, old]) as observe, \
             patch.object(guest, 'systemd', return_value=props), \
             patch.object(guest, 'native_kura_hash', return_value=tip['hash']), \
             patch.object(guest, 'command', side_effect=[RuntimeError('readyz 503'), b'']) as http:
            result = guest.wait_for_cohort([{'role': guest.ROLES[0]}], [old], after=False,
                                          commit=guest.OLD, retained_tip=tip, timeout=5)
            self.assertEqual(result, [old])
            self.assertEqual(observe.call_count, 3)
            self.assertEqual(now[0], 4)
            self.assertIn('http://127.0.0.1:8080/readyz', http.call_args.args[0])
            observe.side_effect = RuntimeError('still not ready')
            with self.assertRaisesRegex(RuntimeError, 'cohort observation deadline'):
                guest.wait_for_cohort([{}], [old], after=True, commit='a' * 40,
                                      retained_tip=tip, timeout=5)
            self.assertEqual(now[0], 9)

    def test_public_probe_errors_identify_role_endpoint_and_exit_without_native_content(self):
        private = b'never expose this response, stderr or argv'
        for route, code in (('/status', 7), ('/readyz', 22),
                            ('/v1/accounts/faucet/puzzle', 28)):
            with self.subTest(route=route, code=code), \
                 patch.object(guest.subprocess, 'run', return_value=SimpleNamespace(
                     returncode=code, stdout=private, stderr=private)):
                with self.assertRaises(RuntimeError) as failure:
                    guest.public_probe(2, route)
                self.assertEqual(str(failure.exception),
                                 f'public probe failed: role={guest.ROLES[2]} endpoint={route} curl_exit={code}')
                self.assertNotIn(private.decode(), str(failure.exception))
        with patch.object(guest.subprocess, 'run', side_effect=subprocess.TimeoutExpired(
                ['curl', private.decode()], 10, output=private, stderr=private)):
            with self.assertRaises(RuntimeError) as failure:
                guest.public_probe(1, '/readyz')
            self.assertEqual(str(failure.exception),
                             f'public probe failed: role={guest.ROLES[1]} endpoint=/readyz timeout')
        for response in (private, b'["never expose this response"]', b'\xffprivate'):
            with self.subTest(response=response), \
                 patch.object(guest, 'public_probe', return_value=response):
                with self.assertRaises(RuntimeError) as failure:
                    guest.public_get(0, '/status')
                self.assertIn(f'role={guest.ROLES[0]} endpoint=/status', str(failure.exception))
                self.assertNotIn('never expose', str(failure.exception))
                self.assertNotIn('private', str(failure.exception))

    def test_failed_http_observation_reports_process_restart_without_config_or_body(self):
        old = {'ActiveState': 'active', 'SubState': 'running', 'ControlPID': '0',
               'MainPID': '42', 'InvocationID': 'a' * 32, 'NRestarts': '0',
               'Result': 'success', 'ExecMainStatus': '0'}
        new = dict(old, MainPID='43', InvocationID='b' * 32, NRestarts='1',
                   Result='exit-code', ExecMainStatus='1')
        row = {'role': guest.ROLES[2], 'after': base64.b64encode(b'public unit').decode()}
        argv = ['/fixed/daemon', '--config', '/owner-private/config.toml', '--sora']
        private = b'never expose this diagnostic'
        with patch.object(guest, 'systemd', side_effect=[old, new]), \
             patch.object(guest, 'retained_identity', return_value={
                 'role': row['role'], 'executable': argv[0]}), \
             patch.object(guest, 'unit_command', return_value=argv), \
             patch.object(Path, 'read_bytes', return_value='\0'.join(argv).encode()), \
             patch.object(guest.os, 'readlink', return_value=argv[0]), \
             patch.object(guest.subprocess, 'run', return_value=SimpleNamespace(
                 returncode=7, stdout=private, stderr=private)):
            with self.assertRaises(RuntimeError) as failure:
                guest.observe(row, after=True)
            message = str(failure.exception)
            self.assertIn(f'role={row["role"]} endpoint=/status curl_exit=7', message)
            self.assertIn('observed=ActiveState=active,SubState=running,MainPID=42', message)
            self.assertIn('current=ActiveState=active,SubState=running,MainPID=43', message)
            self.assertIn('NRestarts=1,Result=exit-code,ExecMainStatus=1', message)
            self.assertNotIn(private.decode(), message)
            self.assertNotIn(argv[2], message)
        with patch.object(guest, 'systemd', return_value=dict(new, ActiveState='failed', SubState='failed')):
            with self.assertRaisesRegex(RuntimeError, 'validator not running: taira-validator-3.*NRestarts=1'):
                guest.observe(row, after=True)
        summary = guest.process_summary(dict(new, Result='arbitrary diagnostic\nprivate token',
                                             UnrequestedField=private.decode()))
        self.assertIn('Result=invalid', summary)
        self.assertNotIn('private', summary)
        self.assertNotIn('UnrequestedField', summary)

    def test_cohort_target_is_highest_stopped_tip_and_rejects_conflicting_maximum(self):
        checkpoints = [{'role': role, 'kura_tip': {'height': height, 'hash': digest * 64}}
                       for role, height, digest in zip(guest.ROLES,
                           [1260, 1260, 1023, 1260], ['c', 'c', 'b', 'c'], strict=True)]
        self.assertEqual(guest.cohort_retained_tip(checkpoints),
                         {'height': 1260, 'hash': 'c' * 64})
        checkpoints[1]['kura_tip']['hash'] = 'd' * 64
        with self.assertRaisesRegex(RuntimeError, 'highest retained Kura tips disagree'):
            guest.cohort_retained_tip(checkpoints)
        checkpoints[1]['kura_tip']['hash'] = 'c' * 64
        checkpoints[2]['kura_tip']['height'] = guest.REPLAY_BARRIER - 1
        with self.assertRaisesRegex(RuntimeError, 'retained cohort Kura tip is invalid'):
            guest.cohort_retained_tip(checkpoints)
        with self.assertRaisesRegex(RuntimeError, 'checkpoint cohort differs'):
            guest.cohort_retained_tip(checkpoints[::-1])

    def test_cohort_requires_common_prefix_without_requiring_empty_blocks(self):
        rows = [{'role': role} for role in guest.ROLES]
        props = {'ActiveState': 'active', 'SubState': 'running', 'ControlPID': '0',
                 'MainPID': '42', 'InvocationID': 'a' * 32}
        before = [{'role': role, 'config_stamp': [1], 'state_root_identity': [2],
                   'current_target': 'same', 'systemd': props,
                   'public': {'height': height, 'commit': guest.OLD}}
                  for role, height in zip(guest.ROLES, [1260, 1260, 1023, 1260], strict=True)]
        current = copy.deepcopy(before)
        tip = {'height': 1260, 'hash': 'c' * 64}
        now = [0.0]
        with patch.object(guest.time, 'monotonic', side_effect=lambda: now[0]), \
             patch.object(guest.time, 'sleep', side_effect=lambda seconds: now.__setitem__(0, now[0] + seconds)), \
             patch.object(guest, 'observe', side_effect=lambda row, **kw: current[guest.ROLES.index(row['role'])]), \
             patch.object(guest, 'systemd', return_value=props) as states, \
             patch.object(guest, 'native_kura_hash', return_value=tip['hash']) as hashes, \
             patch.object(guest, 'command', return_value=b'') as ready:
            with self.assertRaisesRegex(RuntimeError, 'common retained cohort height is not ready'):
                guest.wait_for_cohort(rows, before, after=True, commit=guest.OLD,
                                      retained_tip=tip, timeout=3)
            self.assertGreater(ready.call_count, 0, 'catch-up extensions require readiness')
            self.assertGreater(states.call_count, 0, 'catch-up extensions require stable processes')
            current[2]['public']['height'] = 1260
            hashes.reset_mock()
            self.assertEqual(guest.wait_for_cohort(rows, before, after=True, commit=guest.OLD,
                                                 retained_tip=tip, timeout=3), current)
            self.assertEqual([call.args for call in hashes.call_args_list],
                             [(role, 1260) for role in guest.ROLES])
            self.assertEqual(now[0], 3, 'an idle converged chain needs no delay or new block')
            hashes.return_value = 'd' * 64
            with self.assertRaisesRegex(RuntimeError, 'retained Kura prefix hash changed'):
                guest.observe_cohort(rows, before, after=True, commit=guest.OLD, retained_tip=tip)

    def test_cohort_final_process_sweep_rejects_restart_after_individual_observation(self):
        props = {'ActiveState': 'active', 'SubState': 'running', 'ControlPID': '0',
                 'MainPID': '42', 'InvocationID': 'a' * 32}
        observations = [{'role': role, 'systemd': dict(props)} for role in guest.ROLES]
        for changes in ({'MainPID': '43'}, {'InvocationID': 'b' * 32},
                        {'ActiveState': 'failed', 'SubState': 'failed', 'MainPID': '0'}):
            with self.subTest(changes=changes), \
                 patch.object(guest, 'systemd', return_value=dict(props, **changes)):
                with self.assertRaisesRegex(RuntimeError, 'process changed across cohort verification'):
                    guest.verify_cohort_processes(observations)
        restarted = copy.deepcopy(observations)
        restarted[2]['systemd']['InvocationID'] = 'b' * 32
        with patch.object(guest, 'systemd', return_value=props):
            with self.assertRaisesRegex(RuntimeError, 'process changed across cohort verification'):
                guest.verify_cohort_processes(restarted, observations)

    def test_observation_intent_is_published_after_start_before_completion(self):
        _, records, _, plan = self.simulate()
        intent = records['cohort-observation-intent.json']
        self.assertEqual(intent, {
            'schema': 'taira.cohort-observation-intent.v1',
            'operation': plan['operation'], 'commit': plan['commit'],
            'phase': 'cohort_observation',
            'owner': {'pid': 991, 'start_time_ticks': 1234,
                      'argv': ['/usr/bin/python3', '-I', '-'],
                      'lock': {'device': 1, 'inode': 9}},
            'automatic_restart_or_rollback_after_start': False,
            'remaining_actions': ['observe_cohort', 'verify_strict_restore',
                                  'public_basic_doctor', 'publish_completion_receipts']})
        names = list(records)
        self.assertLess(names.index('start-intent.json'), names.index('cohort-observation-intent.json'))
        self.assertLess(names.index('cohort-observation-intent.json'), names.index('after.json'))
        _, failed, _, _ = self.simulate('start')
        self.assertNotIn('cohort-observation-intent.json', failed)
        self.assertIn('failure.json', failed)

    def test_retained_attempt_binds_exact_completed_predecessor(self):
        build, old_plan = fixture()
        plan = plan_for(build, old_plan)
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(guest, 'BASE', Path(temporary)), \
             patch.object(guest, 'stamp', return_value=[0] * 6 + [100]):
            prior = Path(temporary) / deployment()['current']['attempt_name']
            prior.mkdir()
            records = {'intent.json': old_plan,
                       'result.json': {'schema': 'taira.daemon-update.result.v1',
                                       'runtime_update_complete': True, 'state_preserved': True,
                                       'retained_native_snapshot_verified': True,
                                       'commit': guest.OLD, 'network_id': guest.NETWORK},
                       'after.json': [{'role': role, 'public': {'commit': guest.OLD, 'network_id': guest.NETWORK}} for role in guest.ROLES],
                       'checkpoint-stopped.json': [{'role': role} for role in guest.ROLES],
                       'checkpoint-restored.json': [{'role': role, 'native_strict_checkpoint_verified': True} for role in guest.ROLES]}
            for name, value in records.items():
                (prior / name).write_text(json.dumps(value))
            before_bytes = {p.name: p.read_bytes() for p in prior.iterdir()}
            result = guest.retained_attempt(plan)
            self.assertEqual(result, (records['after.json'], records['checkpoint-stopped.json']))
            self.assertEqual(before_bytes, {p.name: p.read_bytes() for p in prior.iterdir()})
            changed = copy.deepcopy(plan)
            changed['units'][0]['before_sha256'] = 'd' * 64
            with self.assertRaisesRegex(RuntimeError, 'changed installed predecessor unit'):
                guest.retained_attempt(changed)
            changed = dict(plan, commit=old_plan['commit'])
            with self.assertRaisesRegex(RuntimeError, 'installed predecessor source or candidate differs'):
                guest.retained_attempt(changed)
            records['result.json']['runtime_update_complete'] = False
            (prior / 'result.json').write_text(json.dumps(records['result.json']))
            with self.assertRaisesRegex(RuntimeError, 'completion receipt differs'):
                guest.retained_attempt(plan)
            records['result.json']['runtime_update_complete'] = True
            (prior / 'result.json').write_text(json.dumps(records['result.json']))
            records['checkpoint-restored.json'][0]['native_strict_checkpoint_verified'] = False
            (prior / 'checkpoint-restored.json').write_text(json.dumps(records['checkpoint-restored.json']))
            with self.assertRaisesRegex(RuntimeError, 'Strict restoration is not verified'):
                guest.retained_attempt(plan)

    def test_guest_recovery_authenticates_failed_records_and_completed_baseline_independently(self):
        for failure in (None, 'missing', 'digest', 'after.json', 'checkpoint-restored.json',
                        'result.json', 'rollback.json', 'unit', 'artifact', 'health',
                        'completion', 'strict', 'identity'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                build, prior = fixture()
                build['commit'] = 'c' * 40
                failed, records = failed_fixture()
                root = Path(temporary).resolve()
                reference = write_failed_reference(root / failed['operation'], records)
                plan = plan_for(build, prior, failed_start=reference)
                baseline = root / deployment()['current']['attempt_name']
                baseline.mkdir()
                baseline_records = {
                    'intent.json': prior, 'after.json': records['before.json'],
                    'checkpoint-stopped.json': [{'role': role} for role in guest.ROLES],
                    'checkpoint-restored.json': [{'role': role, 'native_strict_checkpoint_verified': True}
                                                 for role in guest.ROLES],
                    'result.json': {'schema': 'taira.daemon-update.result.v1',
                                    'runtime_update_complete': True, 'state_preserved': True,
                                    'retained_native_snapshot_verified': True,
                                    'commit': guest.PREDECESSOR['commit'], 'network_id': guest.NETWORK}}
                if failure == 'completion': baseline_records['result.json']['runtime_update_complete'] = False
                if failure == 'strict': baseline_records['checkpoint-restored.json'][0]['native_strict_checkpoint_verified'] = False
                for name, row in baseline_records.items():
                    (baseline / name).write_text(json.dumps(row))
                failed_directory = root / failed['operation']
                if failure in ('after.json', 'checkpoint-restored.json', 'result.json', 'rollback.json'):
                    (failed_directory / failure).write_text('{}')
                if failure == 'missing': (failed_directory / 'start-intent.json').unlink()
                if failure == 'digest': (failed_directory / 'failure.json').write_text('{}')
                if failure == 'unit': plan['units'][0]['before_sha256'] = 'd' * 64
                if failure in ('health', 'identity'):
                    if failure == 'health': records['before.json'][0]['public']['height'] = 198
                    else: records['before.json'][0]['state_root_identity'] = [8, 8]
                    rebound = write_failed_reference(failed_directory, records)
                    plan['failed_start']['attempts'][-1]['records'] = rebound['attempts'][-1]['records']
                contents = {str(path): path.read_bytes() for path in root.rglob('*.json')}
                with patch.object(guest, 'BASE', root), \
                     patch.object(guest, 'native_kura_hash', return_value='c' * 64), \
                     patch.object(guest, 'stamp', return_value=[0] * 6 + [2_000_000]), \
                     patch.object(guest, 'native_digest', return_value=('d' if failure == 'artifact' else 'b') * 64), \
                     patch.object(guest, 'record') as record, \
                     patch.object(guest, 'stop_all') as stop:
                    if failure not in (None, 'after.json', 'checkpoint-restored.json'):
                        with self.assertRaises((RuntimeError, FileNotFoundError)):
                            guest.retained_attempt(plan)
                    else:
                        retained = guest.retained_attempt(plan)
                        self.assertEqual(retained, (records['before.json'], records['checkpoint-stopped.json']))
                        self.assertEqual(guest.OLD, failed['commit'])
                        self.assertEqual(retained[0][0]['public']['commit'], guest.PREDECESSOR['commit'])
                        observation = copy.deepcopy(retained[0][0])
                        observation['systemd']['InvocationID'] = 'f' * 32
                        checkpoint = retained[1][0]
                        with patch.object(guest, 'snapshot_selection', return_value=checkpoint['selection']) as selection, \
                             patch.object(guest, 'snapshot_events', return_value=[]) as logs, \
                             patch.object(guest, 'native_kura_tip', return_value={'height': 201, 'hash': 'd' * 64}), \
                             patch.object(guest, 'native_kura_hash', return_value='c' * 64) as prefix:
                            recovered = guest.checkpoint_barrier(observation, stopped=True, prior=checkpoint)
                            self.assertTrue(recovered['reused_checkpoint_proof'])
                            self.assertEqual(recovered['proof_invocation_id'], 'e' * 32)
                            self.assertEqual(recovered['invocation_id'], 'f' * 32)
                            self.assertEqual(recovered['kura_tip']['height'], 201)
                            logs.assert_called_with(observation['role'], 'f' * 32)
                            prefix.assert_called_with(observation['role'], 200)
                            prefix.return_value = 'd' * 64
                            with self.assertRaisesRegex(RuntimeError, 'prefix hash changed'):
                                guest.checkpoint_barrier(observation, stopped=True, prior=checkpoint)
                            prefix.return_value = 'c' * 64
                            selection.return_value = dict(checkpoint['selection'], selector='d' * 64)
                            with self.assertRaisesRegex(RuntimeError, 'selection changed'):
                                guest.checkpoint_barrier(observation, stopped=True, prior=checkpoint)
                    record.assert_not_called(); stop.assert_not_called()
                self.assertEqual(contents, {str(path): path.read_bytes() for path in root.rglob('*.json')})

    def simulate(self, failure=None, *, recovery=False, same_artifacts=False, failed_chain_depth=1):
        build, metadata = fixture()
        if recovery:
            with tempfile.TemporaryDirectory() as temporary:
                build, metadata, reference, entries = failed_chain_fixture(
                    temporary, commits=('a',) + ('c',) * (failed_chain_depth - 1))
                failed_records = entries[-1][1]
                if not same_artifacts:
                    build['commit'] = ('c' if failed_chain_depth == 1 else 'd') * 40
                plan = plan_for(build, metadata, failed_start=reference)
        else:
            plan = plan_for(build, metadata)
        events = []
        records = {}
        units = {row['role']: base64.b64decode(row['before']) for row in plan['units']}

        def running(role):
            index = guest.ROLES.index(role) + 1
            props = {'ActiveState': 'active', 'SubState': 'running', 'ControlPID': '0',
                     'MainPID': str(100 + index), 'InvocationID': str(index) * 32, 'Job': ''}
            if 'public-doctor' in events and role == guest.ROLES[2]:
                if failure == 'post-doctor-invocation':
                    props['InvocationID'] = 'b' * 32
                if failure == 'post-doctor-pid':
                    props['MainPID'] = '203'
            return props

        def native(argv, *, timeout=60, name=None):
            events.append(name or str(argv[0]))
            if failure is not None and name == failure:
                raise RuntimeError('injected ' + str(name))
            if name == 'candidate-version':
                return b'iroha3d 3.0.0\n'  # Real --version has no commit.
            if name == 'public-doctor':
                self.assertIn(deployment()['public_origin'], argv)
                self.assertNotIn(deployment()['public_origin']+'/', argv)
                return json.dumps({'command': 'taira_doctor', 'status': 'ok', 'scope': 'basic',
                                   'checks': [{'ok': True}] * 10, 'failures': []}).encode()
            return b''

        def identity(row, *, after=False):
            need_raw = base64.b64decode(row['after' if after else 'before'])
            self.assertEqual(units[row['role']], need_raw)
            return {'role': row['role'], 'unit_stamp': [1, 2, 0o100600, 0, 0, 1, 3, 4, 5],
                    'config_stamp': [1, 4], 'config_sha256': 'same', 'state_root_identity': [1, 8],
                    'current_target': 'unchanged',
                    'systemd': running(row['role']),
                    'executable': str(guest.DAEMON if after else guest.PREVIOUS_DAEMON),
                    'public': {'commit': plan['commit'] if after else guest.PREDECESSOR['commit'],
                               'network_id': guest.NETWORK,
                               'height': 200 if after else 199}}

        def observe(row, *, after=False):
            if not after:
                raise AssertionError('stopped predecessor has no live Torii observation')
            events.append('observe-' + row['role'])
            if (failure == 'post-doctor-http' and 'public-doctor' in events
                    and row['role'] == guest.ROLES[2]):
                raise RuntimeError('validator no longer answers HTTP')
            return identity(row, after=after)

        def install(path, raw, expected, mode):
            role = path.name.removeprefix('iroha3d-').removesuffix('.service')
            events.append('install-' + role)
            self.assertEqual(units[role], expected)
            units[role] = raw
            if failure == 'partial-install' and role == guest.ROLES[1] and raw != base64.b64decode(plan['units'][1]['before']):
                raise RuntimeError('injected installation error after atomic rename')

        original_read = Path.read_bytes

        def read(path):
            if str(path).startswith('/etc/systemd/system/'):
                return units[path.name.removeprefix('iroha3d-').removesuffix('.service')]
            return original_read(path)

        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            stack.enter_context(patch.object(guest, 'configure', side_effect=lambda value: self.assertEqual(value, plan)))
            stack.enter_context(patch.object(guest, 'BASE', Path(directory)))
            stack.enter_context(patch.object(guest, 'ATTEMPT', Path(directory) / 'attempt'))
            if failure == 'attempt-exists':
                guest.ATTEMPT.mkdir()
            stack.enter_context(patch.object(guest.os, 'geteuid', return_value=0))
            stack.enter_context(patch.object(guest, 'native_digest', return_value='b' * 64))
            stack.enter_context(patch.object(guest, 'stamp', return_value=[1, 2, 0o100755, 0, 0, 1, 2_000_000]))
            # A real, empty test file stands in for the already transferred
            # public daemon solely for its fsync; no subprocess executes it.
            fake = Path(directory) / 'daemon'
            fake.touch()
            original_open = guest.os.open
            stack.enter_context(patch.object(guest.os, 'open', side_effect=lambda path, flags, *a:
                                            original_open(fake if path in (guest.DAEMON, guest.CLI) else path, flags, *a)))
            stack.enter_context(patch.object(guest, 'sync'))
            stack.enter_context(patch.object(guest, 'write_new'))
            stack.enter_context(patch.object(guest, 'record', side_effect=lambda k, v: records.update({k: v})))
            stack.enter_context(patch.object(guest, 'command', side_effect=native))
            stack.enter_context(patch.object(guest, 'native_private_command', side_effect=lambda *a, **k: events.append(k['name'])))
            stack.enter_context(patch.object(guest, 'observe', side_effect=observe))
            def observing_owner():
                self.assertIn('start', events)
                self.assertNotIn('after.json', records)
                return {'pid': 991, 'start_time_ticks': 1234,
                        'argv': ['/usr/bin/python3', '-I', '-'],
                        'lock': {'device': 1, 'inode': 9}}
            stack.enter_context(patch.object(guest, 'cohort_observation_owner', side_effect=observing_owner))
            def maintain_owners(operation):
                events.append('stopped-owner-maintenance')
                self.assertEqual(operation, plan['operation'])
                self.assertEqual([row['role'] for row in records['checkpoint-stopped.json']],
                                 list(guest.ROLES))
                self.assertIn('cohort-retained-tip.json', records)
                self.assertFalse(any(event.startswith('install-') for event in events))
                self.assertNotIn('start', events)
                if failure == 'stopped-owner-maintenance':
                    raise RuntimeError('native stopped-owner maintenance failed')
            stack.enter_context(patch.object(guest, 'stopped_owner_maintenance', side_effect=maintain_owners))
            prior = ([identity(row) for row in plan['units']],
                     failed_records['checkpoint-stopped.json'] if recovery else [{} for _ in guest.ROLES])
            stack.enter_context(patch.object(guest, 'retained_attempt', return_value=prior))
            stack.enter_context(patch.object(guest, 'retained_identity', side_effect=identity))
            paused = {'ActiveState': 'inactive', 'SubState': 'dead', 'MainPID': '0',
                      'ControlPID': '0', 'Job': '', 'InvocationID': 'f' * 32}
            def systemd(unit):
                events.append('systemd-' + unit)
                return (running(unit.removeprefix('iroha3d-').removesuffix('.service'))
                        if 'start' in events else paused)
            stack.enter_context(patch.object(guest, 'systemd', side_effect=systemd))
            stack.enter_context(patch.object(guest, 'stop_all', side_effect=lambda: events.append('stop-all') or [{'unit': unit, 'systemd': paused} for unit in guest.UNITS]))
            def checkpoint(row, *, stopped, prior):
                self.assertTrue(stopped)
                self.assertEqual(row['systemd']['InvocationID'], 'f' * 32)
                if recovery:
                    self.assertEqual(prior, failed_records['checkpoint-stopped.json'][guest.ROLES.index(row['role'])])
                return {'role': row['role'], 'selection': 'selected', 'checkpoint_height': 199,
                        'cohort_stopped': True, 'invocation_id': row['systemd']['InvocationID'],
                        'kura_tip': {'height': 200, 'hash': 'c' * 64}}
            stack.enter_context(patch.object(guest, 'checkpoint_barrier', side_effect=checkpoint))
            stack.enter_context(patch.object(guest, 'snapshot_selection', return_value='selected'))
            stack.enter_context(patch.object(guest, 'native_kura_tip', return_value={'height': 200, 'hash': 'c' * 64}))
            def kura_hash(role, height):
                events.append('hash-' + role)
                self.assertEqual(height, 200)
                return ('d' if failure == 'post-doctor-hash' and 'public-doctor' in events
                        and role == guest.ROLES[2] else 'c') * 64
            stack.enter_context(patch.object(guest, 'native_kura_hash', side_effect=kura_hash))
            stack.enter_context(patch.object(guest, 'verify_restored_checkpoint', side_effect=lambda row, cp:
                {'role': row['role'], 'restored_height': 199, 'native_strict_checkpoint_verified': True}))
            stack.enter_context(patch.object(guest, 'install_unit', side_effect=install))
            stack.enter_context(patch.object(Path, 'read_bytes', read))
            with redirect_stdout(io.StringIO()):
                if failure:
                    with self.assertRaises(RuntimeError):
                        guest.apply(plan)
                else:
                    guest.apply(plan)
        return events, records, units, plan

    def test_full_cohort_is_stopped_before_any_unit_replacement_and_readbacks_precede_success(self):
        events, records, _, _ = self.simulate()
        self.assertLess(events.index('verify-units'), events.index('stop-all'))
        self.assertEqual(events.count('stopped-owner-maintenance'), 1)
        self.assertLess(events.index('stop-all'), events.index('stopped-owner-maintenance'))
        self.assertLess(events.index('stopped-owner-maintenance'), events.index('install-taira-validator-1'))
        self.assertLess(events.index('install-taira-validator-4'), events.index('start'))
        self.assertIn('after.json', records)
        self.assertIn('retained-entry.json', records)
        self.assertIn('checkpoint-stopped.json', records)
        self.assertIn('checkpoint-restored.json', records)
        self.assertEqual(records['cohort-retained-tip.json'], {'height': 200, 'hash': 'c' * 64})
        self.assertEqual(records['cohort-ready.json']['retained_tip'], records['cohort-retained-tip.json'])
        self.assertTrue(records['cohort-ready.json']['startup_processes_unchanged'])
        self.assertTrue(records['result.json']['cohort_processes_verified_after_public_doctor'])
        doctor = events.index('public-doctor')
        for role in guest.ROLES:
            for event in ('observe-' + role, 'hash-' + role, 'systemd-iroha3d-' + role + '.service'):
                self.assertIn(event, events[doctor + 1:])
        self.assertFalse(records['result.json']['canary_applied_verified'])
        self.assertFalse(records['result.json']['application_ready'])
        self.assertNotIn('rollback-start', events)

    def test_stopped_owner_maintenance_failure_keeps_all_units_stopped_and_unchanged(self):
        events, records, units, plan = self.simulate('stopped-owner-maintenance')
        self.assertEqual(events.count('stopped-owner-maintenance'), 1)
        self.assertFalse(any(event.startswith('install-') for event in events))
        self.assertNotIn('start', events)
        self.assertNotIn('start-intent.json', records)
        self.assertNotIn('result.json', records)
        self.assertFalse(records['failure.json']['new_start_attempted'])
        self.assertTrue(records['rollback.json']['restored_previous_stopped_cohort'])
        self.assertFalse(records['rollback.json']['old_daemons_restarted'])
        for row in plan['units']:
            self.assertEqual(units[row['role']], base64.b64decode(row['before']))

    def test_partial_install_restores_every_old_unit_before_any_new_daemon_start(self):
        events, records, units, plan = self.simulate('partial-install')
        self.assertNotIn('start', events)
        self.assertNotIn('rollback-start', events)
        self.assertTrue(records['rollback.json']['restored_previous_stopped_cohort'])
        self.assertFalse(records['rollback.json']['old_daemons_restarted'])
        for row in plan['units']:
            self.assertEqual(units[row['role']], base64.b64decode(row['before']))

    def test_update_stops_the_installed_cohort_without_requiring_old_http(self):
        events, records, _, _ = self.simulate()
        self.assertIn('stop-all', events)
        self.assertIn('start', events)
        self.assertTrue(records['result.json']['state_preserved'])
        self.assertEqual(records['intent.json']['schema'], 'taira.daemon-update.plan.v1')
        self.assertEqual(records['before.json'][0]['systemd']['InvocationID'], 'f' * 32)
        self.assertEqual(len(records['stopped.json']['observations']), 4)

    def test_failure_after_start_never_blindly_rolls_back_execution_rules(self):
        events, records, _, _ = self.simulate('public-doctor')
        self.assertIn('start', events)
        self.assertNotIn('rollback-start', events)
        self.assertTrue(records['failure.json']['new_start_attempted'])
        self.assertNotIn('result.json', records)

    def test_post_doctor_cohort_failures_cannot_report_success_or_restart_old_daemons(self):
        for failure in ('post-doctor-invocation', 'post-doctor-pid',
                        'post-doctor-http', 'post-doctor-hash'):
            with self.subTest(failure=failure):
                events, records, _, _ = self.simulate(failure)
                self.assertIn('public-doctor', events)
                self.assertIn('after.json', records, 'retain the earlier startup observations')
                self.assertIn('checkpoint-restored.json', records)
                self.assertTrue(records['failure.json']['new_start_attempted'])
                self.assertNotIn('cohort-ready.json', records)
                self.assertNotIn('result.json', records)
                self.assertNotIn('rollback.json', records)
                self.assertNotIn('rollback-start', events)

    def test_actual_late_failure_records_can_recover_without_promoting_partial_observations(self):
        for failure in ('public-doctor', 'post-doctor-invocation', 'post-doctor-pid',
                        'post-doctor-http', 'post-doctor-hash'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                _, records, _, failed = self.simulate(failure)
                root = Path(temporary).resolve()
                failed_directory = root / failed['operation']
                required = {name: records[name] for name in guest.FAILED_START_RECORDS}
                reference = write_failed_reference(failed_directory, required)
                for name, value in records.items():
                    if name not in required:
                        (failed_directory / name).write_text(json.dumps(value))
                self.assertTrue((failed_directory / 'after.json').exists())
                self.assertTrue((failed_directory / 'checkpoint-restored.json').exists())
                build, prior = fixture()
                build['commit'] = 'c' * 40
                recovery_guest = fresh_guest()
                plan = runner.make_plan(build, deployment(), prior, recovery_guest,
                                        'update-' + '3' * 32, reference)
                baseline = root / deployment()['current']['attempt_name']
                baseline.mkdir()
                baseline_records = {
                    'intent.json': prior, 'after.json': records['before.json'],
                    'checkpoint-stopped.json': records['checkpoint-stopped.json'],
                    'checkpoint-restored.json': records['checkpoint-restored.json'],
                    'result.json': {'schema': 'taira.daemon-update.result.v1',
                                    'runtime_update_complete': True, 'state_preserved': True,
                                    'retained_native_snapshot_verified': True,
                                    'commit': recovery_guest.PREDECESSOR['commit'],
                                    'network_id': recovery_guest.NETWORK}}
                for name, value in baseline_records.items():
                    (baseline / name).write_text(json.dumps(value))
                with patch.object(recovery_guest, 'BASE', root), \
                     patch.object(recovery_guest, 'native_kura_hash', return_value='c' * 64), \
                     patch.object(recovery_guest, 'stamp', return_value=[0] * 6 + [2_000_000]), \
                     patch.object(recovery_guest, 'native_digest', return_value='b' * 64), \
                     patch.object(recovery_guest, 'stop_all') as stop:
                    retained = recovery_guest.retained_attempt(plan)
                    self.assertEqual(retained, (records['before.json'], records['checkpoint-stopped.json']))
                    self.assertEqual(retained[0][0]['public']['commit'], recovery_guest.PREDECESSOR['commit'])
                    self.assertNotEqual(retained[0][0]['public']['commit'], records['after.json'][0]['public']['commit'])
                    for name in ('result.json', 'rollback.json'):
                        marker = failed_directory / name
                        marker.write_text('{}')
                        with self.assertRaisesRegex(RuntimeError, 'success or rollback marker'):
                            recovery_guest.retained_attempt(plan)
                        marker.unlink()
                    stop.assert_not_called()

    def test_failed_start_recovery_keeps_historical_health_and_exact_rollback_boundary(self):
        for failure in (None, 'partial-install', 'start', 'public-doctor'):
            with self.subTest(failure=failure):
                events, records, units, plan = self.simulate(failure, recovery=True)
                for row in records['before.json']:
                    self.assertEqual(row['public']['commit'], deployment()['current']['commit'])
                    self.assertEqual(row['executable'], plan['failed_start']['installed']['daemon'])
                    self.assertEqual(row['systemd']['InvocationID'], 'f' * 32)
                if failure == 'partial-install':
                    self.assertNotIn('start', events)
                    self.assertFalse(records['rollback.json']['old_daemons_restarted'])
                    self.assertFalse(records['failure.json']['new_start_attempted'])
                    for row in plan['units']:
                        self.assertEqual(units[row['role']], base64.b64decode(row['before']))
                        self.assertEqual(guest.unit_command(units[row['role']])[0],
                                         plan['failed_start']['installed']['daemon'])
                else:
                    self.assertIn('start', events)
                    self.assertNotIn('rollback.json', records)
                    for row in plan['units']:
                        self.assertEqual(units[row['role']], base64.b64decode(row['after']))
                if failure:
                    self.assertNotIn('result.json', records)
                else:
                    self.assertTrue(records['result.json']['runtime_update_complete'])

    def test_failed_start_recovery_cannot_repeat_an_existing_attempt(self):
        events, records, units, plan = self.simulate('attempt-exists', recovery=True)
        self.assertEqual(events, [])
        self.assertEqual(records, {})
        for row in plan['units']:
            self.assertEqual(units[row['role']], base64.b64decode(row['before']))

    def test_deployment_metadata_and_route_are_explicit_and_closed(self):
        value=deployment()
        with patch.object(runner.retry,'validate_ssh',return_value=['fixed']) as ssh:
            self.assertIs(runner.validate_deployment(value),value)
            ssh.assert_called_once_with(value['guest_ssh'])
            for key,bad in [('replay_floor',0),('public_origin','https://test.example/'),
                            ('roles',['one']),('ports',[8080]*4),('private_key','forbidden')]:
                changed=copy.deepcopy(value);changed[key]=bad
                with self.assertRaises(RuntimeError):runner.validate_deployment(changed)

    def test_two_operations_can_stage_the_same_candidate_without_overwriting_and_bind_successors(self):
        paths=[]
        commit='d'*40
        for operation in ['update-'+'1'*32,'update-'+'2'*32]:
            value=deployment()
            original=copy.deepcopy(value)
            plan={'deployment':value,'commit':commit,'operation':operation}
            local_guest=fresh_guest();local_guest.configure(plan)
            expected=value['runtime_root']+'/release-'+commit+'-'+operation+'/bin/iroha3d_taira'
            paths.append(expected)
            self.assertEqual(str(local_guest.DAEMON),expected)
            self.assertEqual(str(local_guest.CLI),str(Path(expected).with_name('iroha')))
            transfer=runner.transfer_code('iroha3d_taira',True,plan)
            self.assertIn('release=base/'+repr(Path(expected).parents[1].name),transfer)
            self.assertIn('release.mkdir(mode=0o700)',transfer)
            self.assertIn('os.O_EXCL|os.O_NOFOLLOW',transfer)
            raw=b'exact retained operation plan'
            output=Path('/owner-private')/operation
            successor=runner.successor_deployment(plan,raw,output)
            self.assertEqual(successor['current']['daemon'],expected)
            self.assertEqual(successor['current']['attempt_name'],operation)
            self.assertEqual(successor['current']['commit'],commit)
            self.assertEqual(successor['current']['local_plan'],str(output/'plan.json'))
            self.assertEqual(successor['current']['local_plan_sha256'],runner.sha(raw))
            self.assertEqual(value,original)
            self.assertEqual(str(local_guest.ATTEMPT),value['runtime_root']+'/'+operation)
            self.assertEqual(local_guest.PUBLIC_ORIGIN,value['public_origin'])
            self.assertEqual(local_guest.PORTS,tuple(value['ports']))
            with self.assertRaisesRegex(RuntimeError,'one deployment'):
                local_guest.configure({'deployment':value,'commit':commit,'operation':operation})
        self.assertEqual(len(set(paths)),2)


class CohortProgressTests(unittest.TestCase):
    def setUp(self):
        plan_for()
        self.now = 0.0
        self.rows = [{'role': role} for role in guest.ROLES]
        self.props = [{'ActiveState': 'active', 'SubState': 'running', 'ControlPID': '0',
                       'MainPID': str(42 + index), 'InvocationID': str(index + 1) * 32}
                      for index in range(4)]
        self.before = [{'role': role, 'config_stamp': [1], 'state_root_identity': [2],
                        'current_target': 'same', 'systemd': self.props[index],
                        'public': {'height': 200, 'commit': guest.OLD}}
                       for index, role in enumerate(guest.ROLES)]

    def run_catchup(self, sample, *, target=220, timeout=6, max_timeout=30, ready=None, systemd=None):
        def observation(row, **_kwargs):
            index = guest.ROLES.index(row['role'])
            result = copy.deepcopy(self.before[index])
            result['public']['height'] = sample(index, self.now, result)
            return result

        def sleep(seconds):
            self.now += seconds

        with patch.object(guest.time, 'monotonic', side_effect=lambda: self.now), \
             patch.object(guest.time, 'sleep', side_effect=sleep), \
             patch.object(guest, 'observe', side_effect=observation), \
             patch.object(guest, 'systemd', side_effect=systemd or (lambda unit: self.props[guest.UNITS.index(unit)])), \
             patch.object(guest, 'native_kura_hash', return_value='c' * 64), \
             patch.object(guest, 'command', side_effect=ready, return_value=b''), \
             patch.object(guest, 'stop_all') as stop:
            try:
                return guest.wait_for_cohort(
                    self.rows, self.before, after=True, commit=guest.OLD,
                    retained_tip={'height': target, 'hash': 'c' * 64},
                    timeout=timeout, max_timeout=max_timeout)
            finally:
                stop.assert_not_called()

    def test_advancing_peer_can_finish_after_original_deadline_without_empty_blocks(self):
        result = self.run_catchup(lambda index, now, row: 200 + int(now) if index == 2 else 220)
        self.assertEqual(self.now, 20)
        self.assertEqual([row['public']['height'] for row in result], [220] * 4)

    def test_another_advancing_peer_cannot_extend_a_stalled_validator(self):
        def sample(index, now, row):
            return 200 + int(now) if index == 0 else (200 if index == 2 else 220)
        with self.assertRaisesRegex(RuntimeError, 'cohort observation deadline.*common retained'):
            self.run_catchup(sample)
        self.assertEqual(self.now, 6)

    def test_dead_listener_cannot_borrow_another_peers_progress_budget(self):
        def sample(index, now, row):
            if index == 2 and now >= 2:
                raise RuntimeError('validator not running')
            return 200 + int(now)
        with self.assertRaisesRegex(RuntimeError, 'cohort observation deadline.*not running'):
            self.run_catchup(sample)
        self.assertEqual(self.now, 6)

    def test_restart_or_wrong_candidate_never_earns_more_observation_time(self):
        for change in ('restart', 'commit'):
            with self.subTest(change=change):
                self.now = 0
                def sample(index, now, row):
                    if index == 2 and now >= 2:
                        if change == 'restart':
                            row['systemd']['InvocationID'] = 'f' * 32
                        else:
                            row['public']['commit'] = 'f' * 40
                    return 200 + int(now)
                def systemd(unit):
                    index = guest.UNITS.index(unit)
                    props = dict(self.props[index])
                    if change == 'restart' and index == 2 and self.now >= 2:
                        props['InvocationID'] = 'f' * 32
                    return props
                with self.assertRaisesRegex(RuntimeError, 'cohort observation deadline'):
                    self.run_catchup(sample, systemd=systemd)
                self.assertEqual(self.now, 6)

    def test_advancing_but_unready_peer_does_not_extend_the_deadline(self):
        def ready(argv, **kwargs):
            if self.now >= 2:
                raise RuntimeError('readyz 503')
            return b''
        with self.assertRaisesRegex(RuntimeError, 'cohort observation deadline.*readyz'):
            self.run_catchup(lambda index, now, row: 200 + int(now), ready=ready)
        self.assertEqual(self.now, 6)

    def test_first_healthy_late_sample_alone_does_not_extend_the_deadline(self):
        def sample(index, now, row):
            if now < 4:
                raise RuntimeError('warming')
            return 219
        with self.assertRaisesRegex(RuntimeError, 'cohort observation deadline'):
            self.run_catchup(sample)
        self.assertEqual(self.now, 6)

    def test_converged_observation_finishing_after_deadline_is_not_accepted(self):
        for deadline_kind in ('stall', 'absolute'):
            with self.subTest(deadline_kind=deadline_kind):
                self.now = 0
                def sample(index, now, row):
                    late_at = 4 if deadline_kind == 'stall' else 8
                    if now >= late_at and index == 3:
                        self.now = 7 if deadline_kind == 'stall' else 11
                    return 220 if now >= late_at else 200 + int(now)
                with self.assertRaisesRegex(RuntimeError, 'cohort observation deadline'):
                    self.run_catchup(sample, timeout=4 if deadline_kind == 'stall' else 6,
                                     max_timeout=10)

    def test_height_regression_is_rejected_before_a_later_recovery_can_hide_it(self):
        def sample(index, now, row):
            return (202 if now == 0 else 201) if index == 2 else 220
        with self.assertRaisesRegex(RuntimeError, 'committed catch-up height regressed'):
            self.run_catchup(sample)
        self.assertEqual(self.now, 2)

    def test_continuous_progress_has_an_absolute_deadline(self):
        with self.assertRaisesRegex(RuntimeError, 'cohort observation deadline'):
            self.run_catchup(lambda index, now, row: 200 + int(now),
                             target=1000, max_timeout=10)
        self.assertEqual(self.now, 10)

    def test_deadlines_reject_unbounded_inputs_and_host_covers_guest_maximum(self):
        for timeout, maximum in ((0, 30), (6, 5), (6, guest.COHORT_MAX_TIMEOUT_SECONDS + 1)):
            with self.subTest(timeout=timeout, maximum=maximum), \
                 self.assertRaisesRegex(RuntimeError, 'time bounds are invalid'):
                self.run_catchup(lambda index, now, row: 220, timeout=timeout, max_timeout=maximum)
        self.assertEqual(guest.COHORT_STALL_TIMEOUT_SECONDS, 600)
        self.assertEqual(guest.COHORT_MAX_TIMEOUT_SECONDS, 90 * 60)
        self.assertEqual(runner.GUEST_OPERATION_TIMEOUT_SECONDS,
                         guest.COHORT_MAX_TIMEOUT_SECONDS + 60 * 60 + guest.MAX_FAILED_START_ATTEMPTS * 4 * 20)
        tree = ast.parse(Path(runner.__file__).read_text())
        apply = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'apply_plan')
        remote = [node for node in ast.walk(apply) if isinstance(node, ast.Call)
                  and any(keyword.arg == 'input' and isinstance(keyword.value, ast.Name)
                          and keyword.value.id == 'payload' for keyword in node.keywords)]
        self.assertEqual(len(remote), 1)
        deadline = next(keyword.value for keyword in remote[0].keywords if keyword.arg == 'timeout')
        self.assertIsInstance(deadline, ast.Name)
        self.assertEqual(deadline.id, 'GUEST_OPERATION_TIMEOUT_SECONDS')


class StoppedOwnerMaintenanceTests(unittest.TestCase):
    def setUp(self):
        self.plan = plan_for()
        self.owner = {'pid': 991, 'start_time_ticks': 1234,
                      'argv': ['/usr/bin/python3', '-I', '-'],
                      'lock': {'device': 1, 'inode': 9}}
        self.report = {'schema': 'taira.stopped-owner-maintenance.result.v1',
                       'operation': self.plan['operation'], 'all_four_stopped_owners_clean': True}

    def invoke(self, raw=None, *, exit_code=0, timeout=False, error_type=None):
        raw = json.dumps(self.report).encode() if raw is None else raw
        descriptors = []
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(guest, 'ATTEMPT', Path(directory)), \
             patch.object(guest, 'stamp'), \
             patch.object(guest, 'cohort_observation_owner', return_value=self.owner) as owner:
            native_result = Path(directory) / 'stopped-owner-maintenance-result.json'
            def native(argv, **kwargs):
                fd, = kwargs['pass_fds']
                descriptors.append(fd)
                self.assertEqual(argv, [str(guest.CLI), 'taira', 'stopped-owner-maintenance',
                                        '--request-fd', str(fd)])
                self.assertEqual(kwargs['timeout'], 150)
                self.assertEqual(kwargs['stdin'], subprocess.DEVNULL)
                self.assertTrue(kwargs['capture_output'])
                self.assertEqual(fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE, os.O_RDONLY)
                info = os.fstat(fd)
                self.assertTrue(stat.S_ISREG(info.st_mode))
                self.assertEqual(stat.S_IMODE(info.st_mode), 0o600)
                self.assertEqual(info.st_nlink, 1)
                self.assertEqual(json.loads(os.read(fd, 16_385)), {
                    'schema': 'taira.stopped-owner-maintenance.request.v1',
                    'operation_directory': directory, 'owner': self.owner})
                if timeout:
                    raise subprocess.TimeoutExpired(argv, 150)
                # The real native command owns this receipt. Python's command
                # capture must use a distinct name and must not rewrite it.
                native_result.write_bytes(b'native-owned-receipt\n')
                return subprocess.CompletedProcess(argv, exit_code, raw, b'private-native-diagnostic')
            with patch.object(guest.subprocess, 'run', side_effect=native) as run:
                if error_type:
                    with self.assertRaises(error_type) as caught:
                        guest.stopped_owner_maintenance(self.plan['operation'])
                    self.assertNotIn('private-native-diagnostic', str(caught.exception))
                    self.assertNotIn('untrusted-body', str(caught.exception))
                else:
                    guest.stopped_owner_maintenance(self.plan['operation'])
                run.assert_called_once()
            owner.assert_called_once_with()
            self.assertEqual(len(descriptors), 1)
            with self.assertRaises(OSError):
                os.fstat(descriptors[0])
            if not timeout:
                self.assertEqual(native_result.read_bytes(), b'native-owned-receipt\n')
                self.assertEqual(json.loads((Path(directory) /
                    'stopped-owner-maintenance-command.result.json').read_bytes()),
                    {'exit_code': exit_code})

    def test_candidate_cli_receives_only_readonly_public_request_and_preserves_native_receipt(self):
        self.invoke()

    def test_incomplete_foreign_or_malformed_native_report_cannot_authorize_installation(self):
        reports = [b'untrusted-body', b'[]', b' ' * 16_385]
        for field, value in (('schema', 'unknown'), ('operation', 'update-' + 'f' * 32),
                             ('all_four_stopped_owners_clean', False),
                             ('all_four_stopped_owners_clean', 1)):
            reports.append(json.dumps(dict(self.report, **{field: value})).encode())
        reports.extend((json.dumps({'schema': self.report['schema']}).encode(),
                        json.dumps(dict(self.report, extra='untrusted-body')).encode()))
        for raw in reports:
            with self.subTest(raw=raw[:120]):
                self.invoke(raw, error_type=RuntimeError)

    def test_native_failure_or_timeout_closes_request_without_retry_or_diagnostic_disclosure(self):
        self.invoke(exit_code=1, error_type=guest.NativeCommandFailure)
        self.invoke(timeout=True, error_type=subprocess.TimeoutExpired)


class CohortObservationOwnerTests(unittest.TestCase):
    def setUp(self):
        plan = plan_for()
        self.device = os.makedev(8, 1)
        self.lock = [self.device, 9, 0o100600, 0, 0, 1, 0, 0, 0]
        self.proc = {
            '/proc/991/stat': b'991 (python3) S ' + b'0 ' * 18 + b'1234',
            '/proc/991/cmdline': b'/usr/bin/python3\0-I\0-\0',
            '/proc/locks': b'77: FLOCK ADVISORY WRITE 991 08:01:9 0 EOF\n'}
        self.owner = {'pid': 991, 'start_time_ticks': 1234,
                      'argv': ['/usr/bin/python3', '-I', '-'],
                      'lock': {'device': self.device, 'inode': 9}}
        self.intent = {'schema': guest.COHORT_OBSERVATION_SCHEMA, 'operation': plan['operation'],
                       'commit': plan['commit'], 'phase': 'cohort_observation', 'owner': self.owner,
                       'automatic_restart_or_rollback_after_start': False,
                       'remaining_actions': list(guest.COHORT_REMAINING_ACTIONS)}

    def verify(self, *, terminal=None):
        def read(path, limit):
            raw = self.proc[str(path)]
            if isinstance(raw, list):
                raw = raw.pop(0)
            self.assertLessEqual(len(raw), limit)
            return raw
        with patch.object(guest, 'read_bounded_proc', side_effect=read), \
             patch.object(guest, 'stamp', return_value=self.lock), \
             patch.object(guest.os.path, 'lexists', side_effect=terminal or (lambda path: False)), \
             patch.object(fcntl, 'flock') as flock:
            try:
                return guest.verify_cohort_observation_owner(self.intent)
            finally:
                flock.assert_not_called()

    def test_live_process_and_exact_flock_are_verified_without_lock_acquisition(self):
        self.assertEqual(self.verify(), self.owner)

    def test_changed_pid_start_time_argv_and_lock_cannot_be_reused(self):
        original = dict(self.proc)
        for path, raw in (
            ('/proc/991/stat', b'991 (python3) S ' + b'0 ' * 18 + b'9999'),
            ('/proc/991/stat', b'991 (python3) Z ' + b'0 ' * 18 + b'1234'),
            ('/proc/991/cmdline', b'/usr/bin/python3\0-c\0different\0'),
            ('/proc/locks', b'77: FLOCK ADVISORY WRITE 992 08:01:9 0 EOF\n'),
            ('/proc/locks', b'77: FLOCK ADVISORY WRITE 991 08:01:10 0 EOF\n'),
            ('/proc/locks', b'77: -> FLOCK ADVISORY WRITE 991 08:01:9 0 EOF\n'),
        ):
            with self.subTest(path=path, raw=raw):
                self.proc = dict(original, **{path: raw})
                with self.assertRaises(RuntimeError):
                    self.verify()

    def test_terminal_markers_are_checked_before_and_after_owner_projection(self):
        for name in ('result.json', 'failure.json', 'rollback.json'):
            with self.subTest(name=name), self.assertRaisesRegex(RuntimeError, 'already terminated'):
                self.verify(terminal=lambda path: path.name == name)
        calls = [0]
        def terminal(path):
            calls[0] += 1
            return calls[0] > 3 and path.name == 'result.json'
        with self.assertRaisesRegex(RuntimeError, 'owner changed or terminated'):
            self.verify(terminal=terminal)

    def test_final_process_sample_must_still_be_live_and_well_formed(self):
        original = self.proc['/proc/991/stat']
        for final in (b'991 (python3) Z ' + b'0 ' * 18 + b'1234',
                      b'991 (python3) S 0', b'malformed'):
            with self.subTest(final=final):
                self.proc['/proc/991/stat'] = [original, final]
                with self.assertRaises(RuntimeError):
                    self.verify()

    def test_lock_device_inode_and_custody_are_exact(self):
        original = self.lock[:]
        for index, value in ((0, os.makedev(8, 2)), (1, 10), (2, 0o100644), (3, 1000), (5, 2)):
            with self.subTest(index=index):
                self.lock = original[:]
                self.lock[index] = value
                with self.assertRaises(RuntimeError):
                    self.verify()

    def test_wrong_operation_phase_or_candidate_cannot_claim_the_observation(self):
        for field, value in (('operation', 'update-' + 'f' * 32), ('phase', 'install'),
                             ('commit', 'f' * 40), ('automatic_restart_or_rollback_after_start', True)):
            with self.subTest(field=field):
                original = self.intent[field]
                self.intent[field] = value
                with self.assertRaisesRegex(RuntimeError, 'intent differs'):
                    self.verify()
                self.intent[field] = original

    def test_proc_projection_reads_are_bounded(self):
        with patch.object(Path, 'open', return_value=io.BytesIO(b'a' * 5)):
            with self.assertRaisesRegex(RuntimeError, 'exceeds bound'):
                guest.read_bounded_proc(Path('/proc/locks'), 4)


class FailedStartChainTests(unittest.TestCase):
    def rebind(self, directory, chain, entries):
        """Re-pin deliberately changed public evidence without hiding structural tampering."""
        result = copy.deepcopy(chain)
        for index, (failed, records) in enumerate(entries):
            records['intent.json'] = failed
            result['attempts'][index] = write_failed_reference(
                Path(directory) / failed['operation'], records)['attempts'][0]
        return result

    def completed_guest_records(self, directory, prior, first):
        baseline = Path(directory) / deployment()['current']['attempt_name']
        baseline.mkdir()
        records = {'intent.json': prior, 'after.json': first['before.json'],
            'checkpoint-stopped.json': first['checkpoint-stopped.json'],
            'checkpoint-restored.json': [{'role': role, 'native_strict_checkpoint_verified': True}
                for role in deployment()['roles']],
            'result.json': {'schema': 'taira.daemon-update.result.v1',
                'runtime_update_complete': True, 'state_preserved': True,
                'retained_native_snapshot_verified': True,
                'commit': deployment()['current']['commit'], 'network_id': deployment()['network_id']}}
        for name, value in records.items():
            (baseline / name).write_text(json.dumps(value))

    def test_unchanged_artifacts_retry_a_failed_corrective_rollout(self):
        for depth in (1, 2, 3):
            with self.subTest(depth=depth):
                events, records, units, plan = CoordinatorTests.simulate(
                    self, recovery=True, same_artifacts=True, failed_chain_depth=depth)
                self.assertEqual(plan['commit'], plan['failed_start']['installed']['commit'])
                self.assertEqual(len(plan['failed_start']['attempts']), depth)
                self.assertTrue(records['result.json']['runtime_update_complete'])
                self.assertEqual(records['before.json'][0]['public']['commit'], deployment()['current']['commit'])
                self.assertNotEqual(guest.DAEMON, guest.PREVIOUS_DAEMON)
                for row in plan['units']:
                    self.assertEqual(units[row['role']], base64.b64decode(row['after']))
                self.assertLess(events.index('stop-all'), events.index('start'))
                self.assertNotIn('rollback-start', events)

    def test_chain_accepts_historical_reference_evidence_and_relocated_capture_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            build, prior, chain, entries = failed_chain_fixture(temporary, historical_second=True)
            # Embedded historical paths are not instructions or operational inputs.
            historical = entries[1][0]['failed_start']
            for ref in (historical['plan'], *historical['records'].values()):
                ref['path'] = '/inert/historical/capture/' + Path(ref['path']).name
            chain = self.rebind(temporary, chain, entries)
            plan = plan_for(build, prior, failed_start=chain)
            self.assertEqual(plan['failed_start']['installed']['attempt_name'], entries[-1][0]['operation'])
            self.assertNotEqual(entries[-1][1]['checkpoint-stopped.json'][0]['invocation_id'],
                                entries[-1][1]['checkpoint-stopped.json'][0]['proof_invocation_id'])

    def test_same_commit_binary_identity_is_enforced_for_both_artifacts(self):
        for index in (0, 1):
            for field, value in (('sha256', 'd' * 64), ('size', 2_000_001), ('package', 'foreign')):
                with self.subTest(index=index, field=field), tempfile.TemporaryDirectory() as temporary:
                    build, prior, chain, _ = failed_chain_fixture(temporary)
                    build['artifacts'][index][field] = value
                    with patch.object(runner.subprocess, 'run') as remote:
                        with self.assertRaises(RuntimeError): plan_for(build, prior, failed_start=chain)
                    remote.assert_not_called()

    def test_nonadjacent_source_reuse_cannot_change_binary_identity(self):
        for artifact in (0, 1):
            with self.subTest(artifact=artifact), tempfile.TemporaryDirectory() as temporary:
                build, prior, chain, _ = failed_chain_fixture(temporary)
                build['commit'] = 'a' * 40
                build['artifacts'][artifact]['sha256'] = 'd' * 64
                with self.assertRaisesRegex(RuntimeError, 'ancestry artifacts'):
                    plan_for(build, prior, failed_start=chain)

    def test_historical_nonadjacent_source_reuse_is_revalidated(self):
        for artifact in (0, 1):
            with self.subTest(artifact=artifact), tempfile.TemporaryDirectory() as temporary:
                build, prior, chain, entries = failed_chain_fixture(temporary, commits=('a', 'c', 'a'))
                entries[-1][0]['artifacts'][artifact]['sha256'] = 'd' * 64
                chain = self.rebind(temporary, chain, entries)
                build['commit'] = 'd' * 40
                with self.assertRaisesRegex(RuntimeError, 'ancestry reused a source'):
                    plan_for(build, prior, failed_start=chain)

    def test_artifact_order_and_each_intermediate_unit_are_authenticated(self):
        for failure in ('artifact_order', 'unit_before', 'unit_after', 'baseline'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                build, prior, chain, entries = failed_chain_fixture(temporary)
                failed = entries[-1][0]
                if failure == 'artifact_order': failed['artifacts'].reverse()
                if failure == 'unit_before': failed['units'][1]['before'] = failed['units'][0]['before']
                if failure == 'unit_after': failed['units'][1]['after_sha256'] = 'd' * 64
                if failure == 'baseline': failed['retained_predecessor']['intent_sha256'] = 'd' * 64
                chain = self.rebind(temporary, chain, entries)
                with self.assertRaises(RuntimeError): plan_for(build, prior, failed_start=chain)

    def test_maximum_chain_is_bounded_and_remains_reusable(self):
        with tempfile.TemporaryDirectory() as temporary:
            build, prior, chain, _ = failed_chain_fixture(temporary, commits=('a',) * 16)
            plan = plan_for(build, prior, failed_start=chain)
            self.assertEqual(len(plan['failed_start']['attempts']), 16)

    def test_same_artifact_cli_plans_a_fresh_operation_without_building_or_contacting_host(self):
        with tempfile.TemporaryDirectory() as temporary:
            value, build, descriptor, result = local_inputs(temporary)
            prior = json.loads(Path(value['current']['local_plan']).read_bytes())
            failed, records = failed_fixture(value, prior)
            reference = write_failed_reference(Path(temporary) / failed['operation'], records)
            reference_path = Path(temporary).resolve() / 'failed-chain.json'
            reference_path.write_text(json.dumps(reference))
            output = Path(temporary).resolve() / 'retry-plan.json'
            with patch.object(sys, 'argv', cli_argv(descriptor, result, output) +
                              ['--failed-start-chain', str(reference_path)]), \
                 patch.object(runner.subprocess, 'check_output', return_value='optimizations\n'), \
                 patch.object(runner.retry, 'validate_ssh', return_value=['approved']), \
                 patch.object(runner.subprocess, 'run') as remote, \
                 patch.object(runner, 'apply_plan') as apply, redirect_stdout(io.StringIO()):
                runner.main()
            plan = json.loads(output.read_bytes())
            self.assertEqual(plan['commit'], failed['commit'])
            self.assertNotEqual(plan['operation'], failed['operation'])
            self.assertEqual(plan['build_result_path'], str(result))
            self.assertEqual(plan['build_result_sha256'], runner.sha(result.read_bytes()))
            self.assertEqual(plan['artifacts'], failed['artifacts'])
            remote.assert_not_called(); apply.assert_not_called()

    def test_relocated_identical_prepared_binaries_are_not_a_new_source_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            build, prior, chain, _ = failed_chain_fixture(temporary)
            for row in build['artifacts']: row['path'] = '/new/retained/' + row['name']
            plan = plan_for(build, prior, failed_start=chain)
            self.assertEqual(plan['commit'], 'c' * 40)

    def test_public_evidence_byte_budgets_precede_decode_and_host_mutation(self):
        local_guest = fresh_guest()
        with patch.object(local_guest, 'MAX_FAILED_START_RECORD_BYTES', 4), \
             patch.object(local_guest, 'MAX_FAILED_START_CHAIN_BYTES', 6):
            budget = local_guest.FailedStartRecordBudget()
            budget.consume(b'1234')
            budget.consume(b'56')
            with self.assertRaisesRegex(RuntimeError, 'aggregate byte bound'):
                budget.consume(b'7')
            with self.assertRaisesRegex(RuntimeError, 'record exceeds byte bound'):
                local_guest.FailedStartRecordBudget().consume(b'12345')
        with tempfile.TemporaryDirectory() as temporary:
            build, prior, chain, _ = failed_chain_fixture(temporary)
            local_guest = fresh_guest()
            with patch.object(local_guest, 'MAX_FAILED_START_CHAIN_BYTES', 1), \
                 patch.object(runner.subprocess, 'run') as remote:
                with self.assertRaisesRegex(RuntimeError, 'aggregate byte bound'):
                    runner.make_plan(build, deployment(), prior, local_guest, OPERATION, chain)
            remote.assert_not_called()

    def test_completed_baseline_source_and_old_operational_reference_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            build, prior, chain, _ = failed_chain_fixture(temporary)
            build['commit'] = deployment()['current']['commit']
            with self.assertRaises(RuntimeError): plan_for(build, prior, failed_start=chain)
            build['commit'] = 'c' * 40
            old = dict(chain['attempts'][0], schema='taira.failed-start-reference.v1')
            old.pop('operation')
            with self.assertRaisesRegex(RuntimeError, 'bounded failed-start chain'):
                plan_for(build, prior, failed_start=old)

    def test_chain_rejects_missing_reordered_repeated_or_foreign_ancestry(self):
        for failure in ('empty', 'overlong', 'drop', 'reorder', 'duplicate', 'operation',
                        'current_operation', 'prefix', 'health', 'config', 'invocation',
                        'tip_regression', 'tip_conflict', 'checkpoint_regression'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                build, prior, chain, entries = failed_chain_fixture(temporary, commits=('a', 'c', 'd'))
                if failure == 'empty': chain['attempts'] = []
                elif failure == 'overlong': chain['attempts'] *= 6
                elif failure == 'drop': chain['attempts'].pop(1)
                elif failure == 'reorder': chain['attempts'].reverse()
                elif failure == 'duplicate': chain['attempts'][1] = chain['attempts'][0]
                elif failure == 'operation': chain['attempts'][1]['operation'] = 'update-' + '9' * 32
                elif failure == 'current_operation': chain['attempts'][1]['operation'] = OPERATION
                else:
                    failed, records = entries[1]
                    if failure == 'prefix':
                        failed['failed_start']['attempts'][0]['records']['failure.json']['sha256'] = 'd' * 64
                    if failure == 'health': records['before.json'][0]['public']['height'] = 198
                    if failure == 'config': records['before.json'][0]['config_stamp'] = [9, 9]
                    if failure == 'invocation': records['before.json'][0]['systemd']['InvocationID'] = 'f' * 32
                    if failure == 'tip_regression': records['checkpoint-stopped.json'][0]['kura_tip']['height'] = 199
                    if failure == 'tip_conflict': records['checkpoint-stopped.json'][0]['kura_tip']['hash'] = 'd' * 64
                    if failure == 'checkpoint_regression': records['checkpoint-stopped.json'][0]['checkpoint_height'] = 198
                    chain = self.rebind(temporary, chain, entries)
                with patch.object(runner.subprocess, 'run') as remote:
                    with self.assertRaises(RuntimeError): plan_for(build, prior, failed_start=chain)
                remote.assert_not_called()

    def test_guest_rechecks_every_ancestor_before_mutation(self):
        for failure in (None, 'ancestor_result', 'ancestor_rollback', 'ancestor_missing',
                        'ancestor_digest', 'ancestor_kura', 'candidate_daemon', 'candidate_cli',
                        'aggregate_bytes'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                build, prior, chain, entries = failed_chain_fixture(temporary)
                plan = plan_for(build, prior, failed_start=chain)
                self.completed_guest_records(temporary, prior, entries[0][1])
                ancestor = Path(temporary) / entries[0][0]['operation']
                if failure == 'ancestor_result': (ancestor / 'result.json').write_text('{}')
                if failure == 'ancestor_rollback': (ancestor / 'rollback.json').write_text('{}')
                if failure == 'ancestor_missing': (ancestor / 'failure.json').unlink()
                if failure == 'ancestor_digest': (ancestor / 'failure.json').write_text('{}')
                if failure == 'candidate_daemon': plan['artifacts'][0]['sha256'] = 'd' * 64
                if failure == 'candidate_cli': plan['artifacts'][1]['sha256'] = 'd' * 64
                with patch.object(guest, 'BASE', Path(temporary)), \
                     patch.object(guest, 'stamp', return_value=[0] * 6 + [2_000_000]), \
                     patch.object(guest, 'native_digest', return_value='b' * 64), \
                     patch.object(guest, 'MAX_FAILED_START_CHAIN_BYTES',
                                  1 if failure == 'aggregate_bytes' else 32 * 1024 * 1024), \
                     patch.object(guest, 'native_kura_hash', return_value=('d' if failure == 'ancestor_kura' else 'c') * 64) as kura, \
                     patch.object(guest, 'record') as record, patch.object(guest, 'stop_all') as stop:
                    if failure is None:
                        before, checkpoints = guest.retained_attempt(plan)
                        self.assertEqual(before, entries[-1][1]['before.json'])
                        self.assertEqual(checkpoints, entries[-1][1]['checkpoint-stopped.json'])
                        self.assertEqual(kura.call_count, 8)
                    else:
                        with self.assertRaises((RuntimeError, FileNotFoundError)):
                            guest.retained_attempt(plan)
                    record.assert_not_called(); stop.assert_not_called()


if __name__ == '__main__':
    unittest.main()
