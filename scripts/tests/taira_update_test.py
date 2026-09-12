"""Offline routine-update contracts; no Cargo, SSH or runtime signing inputs.

Run from a normal checkout with python3 scripts/tests/taira_update_test.py.
"""
import argparse
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
    return {'schema': 'taira.failed-start-reference.v1',
            'plan': refs['intent.json'], 'records': refs}


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
                              ['--failed-start-reference', str(reference_path)]), \
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
            self.assertEqual(plan['failed_start']['records'], reference['records'])
            for previous, current in zip(failed['units'], plan['units'], strict=True):
                self.assertEqual(current['before'], previous['after'])
                self.assertEqual(current['before_sha256'], previous['after_sha256'])
            self.assertNotIn('accepted_health', plan['failed_start'])
            self.assertNotEqual(runner.release_name(plan), runner.release_name(failed))
            with patch.object(sys, 'argv', cli_argv(descriptor, result, output) +
                              ['--failed-start-reference', str(reference_path)]), \
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
                if failure == 'missing': Path(reference['records']['failure.json']['path']).unlink()
                if failure == 'digest': reference['records']['failure.json']['sha256'] = 'd' * 64
                if failure == 'intent': reference['plan'] = dict(reference['plan'], sha256='d' * 64)
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
            ready.assert_not_called()
            states.assert_not_called()
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
                    plan['failed_start']['records'] = rebound['records']
                contents = {str(path): path.read_bytes() for path in root.rglob('*.json')}
                with patch.object(guest, 'BASE', root), \
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

    def simulate(self, failure=None, *, recovery=False):
        build, metadata = fixture()
        if recovery:
            build['commit'] = 'c' * 40
            _, failed_records = failed_fixture()
            with tempfile.TemporaryDirectory() as temporary:
                reference = write_failed_reference(temporary, failed_records)
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
        self.assertLess(events.index('stop-all'), events.index('install-taira-validator-1'))
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


if __name__ == '__main__':
    unittest.main()
