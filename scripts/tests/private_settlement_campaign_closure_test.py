"""Actual immutable filesystem closure; all process/capture evidence is synthetic.

No Iroha, packet capture, source qualification or smoke admission is executed.
"""

from pathlib import Path
import hashlib
import importlib.util
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
sys.path.insert(0,str(Path(__file__).resolve().parent))
import private_settlement_campaign_closure as closure
import retained_accounting_fixture as small

import retained_scope_foundation as fixture

accounting,control=closure.accounting,closure.control


class CampaignClosureTests(unittest.TestCase):
    def setUp(self):
        self.f=fixture.RegisteredScopeIntegrationTests();self.f.setUp()
        self.addCleanup(self.f.doCleanups)

    def remove_cut(self,name='campaign-0'):
        (self.f.root/'campaigns'/name/'campaign-closure.json').unlink()

    def invoke(self,name='campaign-0',reason='fail_fast'):
        root=self.f.root/'campaigns'/name
        slot=next(row for row in self.f.scope['campaigns'] if row['campaign_id']==name)
        return closure.runner.close_benchmark_campaign(root,plan=self.f.plans[name],
            scope_path=self.f.scope_path,scope_sha256=hashlib.sha256(self.f.scope_path.read_bytes()).hexdigest(),
            campaign_id=name,plan_sha256=slot['plan']['sha256'],reason=reason,validate_success=self.f.callback)

    def assert_open(self,name='campaign-0'):
        self.assertFalse((self.f.root/'campaigns'/name/'campaign-closure.json').exists())

    def test_partial_session_keeps_replayed_success_after_ack_loss_and_all_starts(self):
        self.f.accepted_sample_graph();self.remove_cut()
        with patch.object(closure.runner,'_process_group_exists',side_effect=AssertionError('historical PID reused')) as groups:
            result=self.invoke()
        groups.assert_not_called()
        self.assertEqual(result['campaign']['counts'],dict(planned=800,attempted=1,not_started=799,
            succeeded=1,failed=0,timed_out=0,incomplete=0))
        self.assertEqual(result['scope_replay']['successful_rows'],[self.f.sample_fixture.bound['sample']])
        doc=json.loads(result['path'].read_bytes())
        self.assertEqual(len(doc['started_request_ids']),51)
        self.assertEqual(len(doc['session_closures']),1)
        self.assertFalse(result['release_qualified'])
        self.assertFalse(result['scope_replay']['source_and_smoke_admitted'])

    def test_untouched_registered_tail_gets_no_placeholder_until_its_actual_close(self):
        self.f.materialize(count=2,prepare=True)
        self.remove_cut();self.remove_cut('campaign-1')
        tail=self.f.root/'campaigns/campaign-1'
        before={str(p.relative_to(tail)):p.read_bytes() for p in tail.rglob('*') if p.is_file()}
        result=self.invoke(reason='not_run')
        self.assertEqual(result['pending_campaign_ids'],['campaign-1'])
        self.assertIsNone(result['scope_replay']);self.assert_open('campaign-1')
        self.assertEqual(before,{str(p.relative_to(tail)):p.read_bytes() for p in tail.rglob('*') if p.is_file()})
        final=self.invoke('campaign-1',reason='not_run')
        self.assertEqual(final['scope_replay']['accounting']['counts']['not_started'],1600)

    def test_unclosed_native_worker_cannot_publish_campaign_cut(self):
        self.f.accepted_sample_graph();self.remove_cut()
        (self.f.root/'campaigns/campaign-0'/self.f.sample_fixture.prefix/'session-closure.json').unlink()
        with self.assertRaisesRegex(control.SessionProtocolError,'no authoritative joined closure'):
            self.invoke()
        self.assert_open()

    def test_unconfirmed_nonbenchmark_physical_group_blocks_cut(self):
        self.f.accepted_sample_graph();self.remove_cut()
        path=next((self.f.root/'campaigns/campaign-0/attempts').glob('*/process-outcome.json'))
        value=json.loads(path.read_bytes());value['owned_process_group_gone']=False
        path.write_bytes(control.canonical(value))
        with patch.object(closure.runner,'_process_group_exists',side_effect=AssertionError('historical PID reused')),self.assertRaisesRegex(accounting.AccountingError,'nonbenchmark process is not quiescent'):
            self.invoke()
        self.assert_open()

    def test_completed_reason_cannot_erase_failed_or_unstarted_suffix(self):
        self.f.materialize();self.remove_cut()
        with self.assertRaisesRegex(accounting.AccountingError,'completed campaign'):
            self.invoke(reason='completed')
        self.assert_open()

    def test_rehashed_sample_mutation_fails_before_cut_publication(self):
        self.f.accepted_sample_graph();self.remove_cut();f=self.f.sample_fixture
        sample=json.loads(f.bound['sample']);sample['network_bytes']+=1
        path=f.root/f.output/'benchmark-sample.json'
        path.write_bytes(closure.replay.samples.semantics.measurement_bytes(sample))
        vp=f.root/f.output/'validation-outcome.json';validation=json.loads(vp.read_bytes())
        validation['sample']={'path':f.output+'/benchmark-sample.json','sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'bytes':path.stat().st_size}
        vp.write_bytes(control.canonical(validation))
        with self.assertRaisesRegex(control.SessionProtocolError,'retained response or sample differs'):
            self.invoke()
        self.assert_open()


class SharedReducerBoundaryTests(unittest.TestCase):
    def test_complete_session_uses_one_closure_and_same_shared_scope_rows(self):
        f=small.Fixture(('succeeded',)*8)
        # Exercise the same collector on real files. The intentionally small
        # fixture uses its explicit synthetic semantic callback, not admission.
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()/'campaign-a';root.mkdir(mode=0o700)
            values={**f.store.values,'frozen-plan.json':f.plan_raw,
                'registered-scope.json':f.scope_raw,'campaign-closure.json':f.packet['closure']}
            for name,raw in values.items():
                path=root/name;path.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
                path.write_bytes(raw);path.chmod(0o600)
            for directory in root.rglob('*'):
                if directory.is_dir():directory.chmod(0o700)
            with closure.collection.collect_closed_campaign(root,scope_raw=f.scope_raw,
                    campaign_id='campaign-a',plan_binding=f.scope['campaigns'][0]['plan']) as collected:
                one=accounting.reduce_retained_campaign(f.scope_raw,collected.packet,collected.successful_rows,
                    worker_command=f.command,worker_image=f.image,validate_success=f.recompute)
                collected.validate()
        whole=f.reduce()
        self.assertEqual(one['rows'],whole['rows'])
        self.assertEqual(one['counts']['succeeded'],8)
        self.assertEqual(one['campaign']['sessions'][0]['terminal_kind'],'completed')
        self.assertFalse(one['registered_scope_complete'])

    def test_truthful_setup_failure_closes_only_actual_reaped_empty_owner(self):
        f=small.Fixture(setup_failure=True);result=f.reduce()
        session=result['campaigns'][0]['sessions'][0]
        self.assertEqual(result['counts']['attempted'],0)
        self.assertEqual(session['setup_outcome'],'failed_before_readiness')
        self.assertFalse(session['network_cleanup_claimed'])
        self.assertIs(f.terminal_value['network_shutdown_observed'],False)
        self.assertIs(f.terminal_value['coordinator_reaped_observed'],False)

    def lifetime(self,f,*,terminal=None,lifecycle=None,absent=True):
        cut=json.loads(f.store.read(f.closure_ref))
        for field,value in (('worker_terminal',terminal),('adapter_lifecycle',lifecycle)):
            if value is not None:
                path=cut[field]['path'];f.store.values[path]=control.canonical(value);cut[field]=f.store.locate(path)
        if terminal is not None:
            value=json.loads(f.store.read(cut['adapter_lifecycle']));value['worker_terminal']=cut['worker_terminal']
            path=cut['adapter_lifecycle']['path'];f.store.values[path]=control.canonical(value);cut['adapter_lifecycle']=f.store.locate(path)
        return accounting._session_lifetime(accounting._SessionRecords(f.store),cut,f.identity,
            json.loads(f.store.read(f.started)),f.accepted,f.terminal_value['active_attempt_id'],f.image,
            unready_attempts_absent=absent)

    def test_unready_setup_requires_absent_attempts_and_real_wait_group_absence(self):
        for change in ('attempt','wait','member','absence'):
            f=small.Fixture(setup_failure=True);value=json.loads(f.store.values[f.prefix+'/adapter-lifecycle.json'])
            if change=='wait':value['worker_wait_completed']=False
            if change=='member':value['group_after']['members']=[999]
            if change=='absence':value['kernel_absences']=[]
            with self.subTest(change=change),self.assertRaises(accounting.AccountingError):
                self.lifetime(f,lifecycle=value,absent=change!='attempt')

    def test_established_network_and_cleanup_failure_keep_false_cleanup_open(self):
        for established in (True,False):
            f=small.Fixture(('failed',),setup_failure=not established)
            terminal=dict(f.terminal_value,network_shutdown_observed=False,coordinator_reaped_observed=False)
            if not established:terminal['reason']='cleanup_failed'
            with self.subTest(established=established),self.assertRaises(accounting.AccountingError):
                self.lifetime(f,terminal=terminal,absent=not established)


if __name__=='__main__':
    suite=unittest.TestSuite()
    loader=unittest.defaultTestLoader
    for cls in (CampaignClosureTests,SharedReducerBoundaryTests,small.RetainedAccountingTests):
        suite.addTests(loader.loadTestsFromTestCase(cls))
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
