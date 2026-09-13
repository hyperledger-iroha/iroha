"""Canonical control-flow tests with explicitly substituted execution/admission.

Real plans/files/writers/closure reducers run; no source gate, Iroha process,
packet capture, qualified sample, or successful native campaign is fabricated.
"""

from pathlib import Path
from contextlib import ExitStack,redirect_stderr
import hashlib
import importlib.util
import io
import json
import shutil
import sys
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import private_settlement_campaign_execution as execution
runner,control=execution.runner,execution.control

sys.path.insert(0, str(Path(__file__).resolve().parent))
import retained_scope_foundation as fixture
import retained_accounting_fixture as small


class CanonicalSessionOwnerAdmissionTests(unittest.TestCase):
    def test_malformed_owner_rejected_before_pending_registration(self):
        campaign=execution.CampaignExecution()
        existing=execution.sessions.AdmittedSessionExecution()
        campaign.retain_session_owner('existing',existing)
        for malformed in (object(),SimpleNamespace(closed=False,closure=None)):
            with self.subTest(owner_type=type(malformed).__name__):
                with self.assertRaisesRegex(control.SessionProtocolError,
                        'campaign session owner is not canonical'):
                    campaign.retain_session_owner('invalid',malformed)
                self.assertEqual(campaign.session_owners,{'existing':existing})
                self.assertFalse(existing.closed)

    def test_incomplete_canonical_owner_retained_without_closure_or_reap(self):
        campaign=execution.CampaignExecution()
        retained=execution.sessions.AdmittedSessionExecution()
        with patch.object(retained,'close',side_effect=ValueError('still incomplete')) as close:
            campaign.retain_session_owner('pending',retained)
        self.assertIs(campaign.session_owners['pending'],retained)
        self.assertFalse(retained.closed);self.assertIsNone(retained.closure)
        close.assert_not_called()

    def test_duplicate_registration_never_replaces_retained_owner(self):
        campaign=execution.CampaignExecution()
        retained=execution.sessions.AdmittedSessionExecution()
        replacement=execution.sessions.AdmittedSessionExecution()
        campaign.retain_session_owner('pending',retained)
        with self.assertRaisesRegex(control.SessionProtocolError,'session owner cannot be retried'):
            campaign.retain_session_owner('pending',replacement)
        self.assertIs(campaign.session_owners['pending'],retained)
        self.assertFalse(retained.closed);self.assertFalse(replacement.closed)


class ExecutePlanTests(unittest.TestCase):
    def setUp(self):
        self.f=fixture.RegisteredScopeIntegrationTests();self.f.setUp();self.addCleanup(self.f.doCleanups)
        self.f.materialize();self.output=self.f.root/'campaigns/campaign-0'
        self.input=self.f.root/'inputs';shutil.copytree(self.output,self.input)
        shutil.rmtree(self.output)
        self.plan=self.f.plans['campaign-0'];self.harness=Path('/synthetic/admitted/plan-harness')
        self.worker=Path('/usr/bin/false').resolve();self.validator=Path('/bin/cat').resolve()
        self.verify=runner.verify_harness
        self.prerequisite={'integration_sha256':self.verify(self.worker)['sha256'],
            'validator_sha256':self.verify(self.validator)['sha256'],'fixture':'substituted gate; never qualification'}
        self.stack=ExitStack();self.addCleanup(self.stack.close)
        self.source=self.stack.enter_context(patch.object(runner,'verify_source_checkout'))
        self.smoke=self.stack.enter_context(patch.object(runner,'validate_smoke_prerequisite',return_value=self.prerequisite))
        self.stack.enter_context(patch.object(runner,'verify_harness',side_effect=lambda path:
            self.plan['harness'] if Path(path)==self.harness else self.verify(path)))
        self.stack.enter_context(patch.object(runner,'_process_group_exists',return_value=False))
        self.native=self.stack.enter_context(patch.object(execution.sessions,'execute_admitted_session',
            side_effect=AssertionError('no native invocation allowed without a declared fixture')))
        self.calls=[];self.fault_failure=None
        self.invoke_mock=self.stack.enter_context(patch.object(runner,'invoke_harness',side_effect=self.fault))
        self.materialize=self.stack.enter_context(patch.object(runner,'materialize_fault_response',
            return_value=({'explicit_fixture':True},[])))

    def args(self):
        return dict(source_root=ROOT,harness=self.harness,smoke_campaign=self.f.root/'synthetic-smokes',
            scope_path=self.f.scope_path,campaign_id='campaign-0',worker_path=self.worker,validator_path=self.validator)

    def execute(self):
        return runner.execute_plan(self.input/'frozen-plan.json',self.output,**self.args())

    def fault(self,harness,request,*,attempt_dir,timeout_seconds,expected_harness_binding,accounting_identity):
        self.assertEqual(request['kind'],'fault');self.calls.append(request['request_id'])
        runner.fresh_private_directory(attempt_dir)
        runner.private_record(attempt_dir/'request.json',request)
        identity={'version':1,'protocol':control.PROTOCOL,**accounting_identity,
            'request_id':request['request_id'],'invocation_nonce':request['invocation_nonce']}
        began=time.time_ns()
        runner.private_record(attempt_dir/'started.json',{**identity,'command':['explicit-synthetic-fault-child'],
            'harness':expected_harness_binding,'request':runner.file_binding(attempt_dir/'request.json'),
            'timeout_seconds':timeout_seconds,'started_ns':began})
        failed=self.fault_failure==len(self.calls)
        runner.private_record(attempt_dir/'process-outcome.json',{**identity,'finished_ns':time.time_ns(),
            'pid':900000+len(self.calls),'exit_code':1 if failed else 0,'timed_out':False,'error':None,
            'passed':not failed,'retained_files':[],'completion_kind':'exited','elapsed_ms':0,
            'owned_process_group_gone':True,'bindings_unchanged':True})
        if failed:raise runner.RunnerError('explicit synthetic fault failure')
        runner.fresh_private_directory(attempt_dir/'evidence')
        response={'explicit_fixture':True}
        runner.private_record(attempt_dir/'response.json',response)
        return response,attempt_dir/'evidence',attempt_dir/'response.json',runner.file_binding(attempt_dir/'response.json')

    def failed(self):
        with self.assertRaises(execution.CampaignExecutionIncomplete) as caught:self.execute()
        return caught.exception.owner

    def test_source_refusal_precedes_directory_or_any_execution(self):
        self.source.side_effect=runner.RunnerError('source refusal')
        self.failed();self.assertFalse(self.output.exists());self.invoke_mock.assert_not_called();self.native.assert_not_called()

    def test_ten_smoke_refusal_precedes_directory_or_any_execution(self):
        self.smoke.side_effect=runner.RunnerError('ten-smoke refusal')
        self.failed();self.assertFalse(self.output.exists());self.invoke_mock.assert_not_called();self.native.assert_not_called()

    def test_different_admitted_native_image_refuses_before_execution(self):
        self.prerequisite['integration_sha256']='0'*64
        owner=self.failed();self.assertIn('differs from admitted ten-smoke',str(owner.error))
        self.assertFalse(self.output.exists());self.invoke_mock.assert_not_called()

    def test_failed_fault_preserves_mandatory_prefix_and_closes_only_zero_benchmarks(self):
        self.fault_failure=1;owner=self.failed();self.native.assert_not_called()
        self.assertIsNotNone(owner.closure)
        result=json.loads((self.output/'failure.json').read_bytes())
        self.assertEqual(result['durable_started_request_ids'],self.calls)
        self.assertEqual(len(result['not_started_request_ids']),len(self.plan['jobs'])-1)
        self.assertEqual(owner.closure['campaign']['counts']['attempted'],0)
        self.assertFalse((self.output/'campaign-artifacts.json').exists())

    def test_fault_validation_failure_never_launches_first_session(self):
        self.materialize.side_effect=runner.RunnerError('explicit fault semantic rejection')
        self.failed();self.assertEqual(len(self.calls),1);self.native.assert_not_called()

    def test_real_fault_prefix_then_unclosed_native_owner_keeps_durable_start_and_owner(self):
        # Failure after durable start but before any physical worker is spawned.
        # Keep the canonical owner type; no invalid duck-typed owner can drain.
        sentinel=execution.sessions.AdmittedSessionExecution()
        def start_failure(**kwargs):
            root=kwargs['frozen_plan'].parent
            base={'scope_sha256':runner.file_binding(root/'registered-scope.json')['sha256'],
                'campaign_id':root.name,'plan_sha256':runner.file_binding(root/'frozen-plan.json')['sha256']}
            runner.fresh_private_directory(root/'sessions')
            prepared=runner.prepare_benchmark_session(self.plan,root,root,base,kwargs['descriptor'])
            with control.RecordDirectory(root) as records:
                command=[str(self.worker),execution.replay.samples.adapter.WORKER_TEST,
                         '--exact','--ignored','--nocapture','--test-threads=1']
                started=runner.publish_benchmark_session_start(prepared,records=records,command=command,
                    harness=self.verify(self.worker))
                first=next(i for i,job in enumerate(self.plan['jobs'],1) if job.get('session_id')==kwargs['descriptor']['session_id'])
                runner.publish_benchmark_attempt_start(prepared,0,records=records,session_started=started,
                    ordinal=first,outer_timeout_ms=self.plan['benchmark_accounting']['outer_timeout_ms'],preceding_acceptance=None)
            sentinel.prepared=prepared
            raise execution.sessions.SessionExecutionIncomplete(sentinel)
        self.native.side_effect=start_failure
        owner=self.failed();self.assertEqual(len(self.calls),50);self.assertEqual(self.native.call_count,1)
        self.assertIs(owner.session_owners[self.plan['benchmark_sessions'][0]['session_id']],sentinel)
        result=json.loads((self.output/'failure.json').read_bytes())
        self.assertEqual(len(result['durable_started_request_ids']),51)
        self.assertEqual(len(result['not_started_request_ids']),len(self.plan['jobs'])-51)
        self.assertIsNone(owner.closure);self.assertIsNotNone(owner.closure_error)
        self.assertTrue((self.output/'closure-pending.json').is_file())
        self.assertFalse((self.output/'campaign-closure.json').exists())

    def test_loop_dispatches_session_once_then_next_pair_without_per_attempt_harness(self):
        seen=[]
        def batch(owner,descriptor,*,root,plan,completed_ids):
            seen.append(descriptor['session_id'])
            if len(seen)==2:raise runner.RunnerError('explicit next-session fixture stop')
            group=[(i,job) for i,job in enumerate(plan['jobs'],1) if job.get('session_id')==descriptor['session_id']]
            self.assertEqual(len(completed_ids),50)
            # This control substitutes only dispatch outcome, emits no samples,
            # and necessarily ends before any report or qualified fragment.
            completed=[{'ordinal':i,'request_id':job['request_id'],'kind':'benchmark'} for i,job in group]
            return None,group,[],completed,True
        with patch.object(execution.CampaignExecution,'run_session',batch):self.failed()
        self.assertEqual(seen,[row['session_id'] for row in self.plan['benchmark_sessions'][:2]])
        self.assertEqual(len(self.calls),50);self.native.assert_not_called()
        self.assertFalse((self.output/'campaign-artifacts.json').exists())

    def test_unreaped_native_verifier_owner_prevents_campaign_cut(self):
        retained=execution.sessions.AdmittedSessionExecution()
        # Explicit pre-Popen native-owner fixture: no child exists, but no
        # authenticated completed-wait evidence may be invented either.
        native_type=execution.sessions.semantics.economics.NativeVectorInvocation
        verifier=native_type.__new__(native_type)
        verifier.reaped=False;verifier.physical_exit=None;verifier.process=None
        semantics_type=execution.sessions.semantics.SessionSemantics
        retained.semantic_owner=semantics_type.__new__(semantics_type)
        retained.semantic_owner.native_invocations={'actual-owner-slot':verifier}
        def incomplete(owner,descriptor,*,root,plan,completed_ids):
            owner.session_owners[descriptor['session_id']]=retained
            grouped=[(i,job) for i,job in enumerate(plan['jobs'],1) if job.get('session_id')==descriptor['session_id']]
            return retained,grouped,[],[],False
        with patch.object(execution.CampaignExecution,'run_session',incomplete):owner=self.failed()
        self.assertIn('native verifier lacks canonical physical closure',str(owner.closure_error))
        self.assertFalse(verifier.reaped);self.assertIsNone(verifier.physical_exit)
        self.assertIs(owner.session_owners[self.plan['benchmark_sessions'][0]['session_id']],retained)
        self.assertFalse((self.output/'campaign-closure.json').exists())

    def test_exclusive_unstarted_slot_uses_real_retained_closure_without_invocation(self):
        args=self.args();args.pop('scope_path')
        path=runner.close_unstarted_campaign(self.f.scope_path,plan_path=self.input/'frozen-plan.json',**args)
        value=json.loads(path.read_bytes());self.assertEqual(value['reason'],'not_run')
        self.assertEqual(value['started_session_ids'],[]);self.assertEqual(value['started_request_ids'],[])
        self.invoke_mock.assert_not_called();self.native.assert_not_called()

    def test_exclusive_unstarted_never_acquires_existing_directory(self):
        self.output.mkdir(mode=0o700);marker=self.output/'existing';marker.write_bytes(b'held')
        args=self.args();args.pop('scope_path')
        with self.assertRaises((runner.RunnerError,FileExistsError)):
            runner.close_unstarted_campaign(self.f.scope_path,plan_path=self.input/'frozen-plan.json',**args)
        self.assertEqual(marker.read_bytes(),b'held');self.assertEqual(list(self.output.iterdir()),[marker])

    def test_cli_requires_explicit_worker_and_validator(self):
        args=['execute','--plan','a','--output-dir','b','--source-root','c','--harness','d',
              '--smoke-campaign','e','--scope','f','--campaign-id','g']
        with redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):runner.parse_args(args)
        parsed=runner.parse_args(args+['--worker','w','--validator','v'])
        self.assertEqual(parsed.worker,Path('w'));self.assertEqual(parsed.validator,Path('v'))


class LifetimeReplayTests(unittest.TestCase):
    def test_rebound_fault_to_fault_and_fault_to_session_overlaps_reject(self):
        for edge in ('fault','session'):
            f=fixture.RegisteredScopeIntegrationTests();f.setUp()
            try:
                f.accepted_sample_graph()
                root=f.root/'campaigns/campaign-0';ordinal=1 if edge=='fault' else 50
                job=f.plans['campaign-0']['jobs'][ordinal-1]
                path=root/'attempts'/f"{ordinal:05}-{job['request_id']}"/'process-outcome.json'
                value=json.loads(path.read_bytes());value['finished_ns']=31 if edge=='fault' else 100_000_001
                path.write_bytes(runner.canonical_bytes(value))
                with self.subTest(edge=edge),self.assertRaisesRegex(runner.attempt_accounting.AccountingError,'owner lifetimes overlap'):
                    f.invoke()
            finally:f.doCleanups()

    def test_ordered_full_graph_still_replays_exact_sample_and_denominator(self):
        f=fixture.RegisteredScopeIntegrationTests();f.setUp()
        try:
            f.accepted_sample_graph();result=f.invoke()
            self.assertEqual(result['accounting']['counts']['succeeded'],1)
            self.assertEqual(result['accounting']['counts']['planned'],800)
            self.assertFalse(result['source_and_smoke_admitted'])
        finally:f.doCleanups()


if __name__=='__main__':
    suite=unittest.TestSuite()
    for cls in (ExecutePlanTests,LifetimeReplayTests,small.RetainedAccountingTests):
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(cls))
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not result.wasSuccessful())
