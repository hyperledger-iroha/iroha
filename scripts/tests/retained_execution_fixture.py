"""Exercise the real campaign loop with synthetic native/capture execution only."""
from contextlib import contextmanager,ExitStack
from pathlib import Path
import hashlib,json,shutil,time
from unittest.mock import patch

import private_settlement_real_process_harness as one_shot
import private_settlement_campaign_execution as execution
import private_settlement_record_provider as filesystem
import private_settlement_registered_session_replay as replay
from scripts.tests.private_settlement_registered_accounting_fixture import fixture_admission
from scripts.tests.private_settlement_release_evidence_test import PrivateSettlementReleaseEvidenceTests

runner,control,accounting=execution.runner,execution.control,replay.accounting


@contextmanager
def execution_fixture(root):
    """Use the canonical 852-job plan and exact retained native record schema.

The source/smoke and fault/leakage semantics remain explicitly synthetic test
facts. Each native session input is replayed by the actual sample/accounting
owners before the canonical campaign loop consumes its closure. No process is
spawned and no capture or native measurement is qualified by this fixture.
"""
    root=Path(root).resolve();bundle=root/'bundle';bundle.mkdir(mode=0o700)
    PrivateSettlementReleaseEvidenceTests().make_bundle(bundle)
    output=bundle/'accounting/campaigns/campaign-1-complete';input_root=root/'frozen-input'
    output.rename(input_root)
    plan,unused=runner.load_plan(input_root/'frozen-plan.json')
    scope_path=bundle/'accounting/scope.json'
    nonbenchmark=[(ordinal,job) for ordinal,job in enumerate(plan['jobs'],1) if job['kind']!='benchmark']
    nonce_index=0;owners=[];state={'failed_native':False}
    random_bytes=runner.os.urandom
    with ExitStack() as stack:
        admission=stack.enter_context(fixture_admission())
        prerequisite={'passed':True,'runs':10,'integration_sha256':runner.verify_harness(admission['worker_path'])['sha256'],
                     'validator_sha256':runner.verify_harness(admission['validator_path'])['sha256']}
        smoke=stack.enter_context(patch.object(runner,'validate_smoke_prerequisite',return_value=prerequisite))
        stack.enter_context(patch.object(runner,'_process_group_exists',return_value=False))
        def nonce(size):
            nonlocal nonce_index
            assert size==32
            ordinal,job=nonbenchmark[nonce_index];nonce_index+=1
            request=json.loads((input_root/'attempts'/f"{ordinal:05}-{job['request_id']}"/'request.json').read_bytes())
            return bytes.fromhex(request['invocation_nonce'])
        stack.enter_context(patch.object(runner.os,'urandom',side_effect=nonce))
        def invoke(harness,request,*,attempt_dir,timeout_seconds,expected_harness_binding,accounting_identity):
            one_shot.validate_request(request)
            source=input_root/'attempts'/attempt_dir.name
            runner.fresh_private_directory(attempt_dir)
            for name in ('request.json','started.json','process-outcome.json'):
                runner.copy_bound_file(source/name,attempt_dir/name,expected=runner.file_binding(source/name))
            assert json.loads((attempt_dir/'request.json').read_bytes())==request
            runner.fresh_private_directory(attempt_dir/'evidence')
            (attempt_dir/'evidence/raw.bin').write_bytes(b'retained synthetic capture\x00')
            (attempt_dir/'stdout.log').write_bytes(b'retained synthetic stdout\n')
            (attempt_dir/'stderr.log').write_bytes(b'retained synthetic stderr\n')
            runner.private_record(attempt_dir/'response.json',{})
            return {},attempt_dir/'evidence',attempt_dir/'response.json',runner.file_binding(attempt_dir/'response.json')
        process=stack.enter_context(patch.object(runner,'invoke_harness',side_effect=invoke))
        def native(**kwargs):
            descriptor=kwargs['descriptor'];sid=descriptor['session_id']
            if not (output/'sessions').exists():runner.fresh_private_directory(output/'sessions')
            group=[(i,job) for i,job in enumerate(plan['jobs'],1) if job.get('session_id')==sid]
            roots=['sessions/'+sid]
            if state['failed_native']:
                from retained_session_fixture import SessionFixture
                identity={'scope_sha256':runner.file_binding(scope_path)['sha256'],'campaign_id':output.name,
                          'plan_sha256':runner.file_binding(output/'frozen-plan.json')['sha256']}
                with patch.object(runner.os,'urandom',side_effect=random_bytes):
                    prepared=runner.prepare_benchmark_session(plan,output,output,identity,descriptor)
                start=json.loads((input_root/'sessions'/sid/'started.json').read_bytes())['started_ns']
                fixture=SessionFixture(prepared,output,{'worker':kwargs['worker_image'],'validator':kwargs['validator_image']},
                    1000,start,plan['benchmark_accounting']['outer_timeout_ms'])
                fixture.success(0);fixture.failure(1);fixture.close(failed=True)
            else:
                shutil.copytree(input_root/'sessions'/sid,output/'sessions'/sid)
            for ordinal,job in group:
                relative=f"attempts/{ordinal:05}-{job['request_id']}"
                if not state['failed_native']:shutil.copytree(input_root/relative,output/relative)
                roots.append(relative)
            provider=filesystem.RetainedRecordProvider(output,roots);owners.append(provider)
            records=accounting._SessionRecords(provider)
            reference=provider.inventory()['sessions/'+sid+'/session-closure.json']
            identity={'scope_sha256':runner.file_binding(scope_path)['sha256'],'campaign_id':output.name,
                      'plan_sha256':runner.file_binding(output/'frozen-plan.json')['sha256']}
            callback=replay.samples.RetainedSampleReplay(worker=kwargs['worker_image'],validator=kwargs['validator_image'],
                owner_uid=__import__('os').geteuid(),group_utility_sha256='c'*64,listener_utility_sha256='d'*64,
                packet_utility={key:value for key,value in replay.samples.semantics.packets._utility().items() if key in ('path','sha256')})
            command=[kwargs['worker_image']['path'],replay.samples.adapter.WORKER_TEST,'--exact','--ignored','--nocapture','--test-threads=1']
            rows,samples,summary=accounting.reduce_retained_session(descriptor,{'session_id':sid,'closure':reference},
                records=records,campaign_identity=identity,jobs=group,registered_ns=1,closed_ns=time.time_ns(),
                policy=plan['benchmark_accounting'],worker_command=command,
                worker_image={key:kwargs['worker_image'][key] for key in ('sha256','bytes')},validate_success=callback)
            live_records=control.RecordDirectory(output);owners.append(live_records)
            # This explicit synthetic session never spawns any physical owner.
            # Use the actual required owner type; the outer fixture closes records.
            session = execution.sessions.AdmittedSessionExecution()
            session.closure = {'reference':reference,'rows':rows,'samples':samples,'summary':summary}
            session.result = {'all_attempts_accepted':not state['failed_native']}
            session.records = live_records
            session.closed = True
            return session
        benchmark=stack.enter_context(patch.object(execution.sessions,'execute_admitted_session',side_effect=native))
        stack.enter_context(patch.object(runner,'materialize_fault_response',return_value=({'synthetic':'fault'},[])))
        stack.enter_context(patch.object(runner,'validate_leakage_response',return_value=({'messages':1},[])))
        for owner,name,value in ((runner.fault_report,'load_runs',[]),(runner.fault_report,'input_bindings',[]),
                                (runner.fault_report,'build_report',{'passed':True})):
            stack.enter_context(patch.object(owner,name,return_value=value))
        for name in ('write_fault_csv','write_benchmark_csv'):
            stack.enter_context(patch.object(runner,name,side_effect=lambda path,rows:path.write_text('synthetic\n')))
        stack.enter_context(patch.object(runner,'differential_pair_manifest',return_value={}))
        leakage=stack.enter_context(patch.object(runner.leakage_audit,'run_audit',return_value={'passed':True}))
        try:
            yield {'plan':plan,'output':output,'input':input_root,'scope':scope_path,'smoke':smoke,
                   'prerequisite':prerequisite,'process':process,'benchmark':benchmark,'leakage':leakage,'state':state,
                   'execute':lambda:runner.execute_plan(input_root/'frozen-plan.json',output,source_root=admission['source_root'],
                       harness=admission['plan_harness'],smoke_campaign=admission['smoke_campaign'],scope_path=scope_path,
                       campaign_id=output.name,worker_path=admission['worker_path'],validator_path=admission['validator_path'])}
        finally:
            for owner in owners:owner.close()
