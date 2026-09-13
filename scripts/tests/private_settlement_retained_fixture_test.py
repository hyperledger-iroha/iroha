"""Validate generated native-shaped fixtures through real retained reducers."""
from pathlib import Path
import hashlib,importlib.util,json,os,sys,unittest

HERE=Path(__file__).resolve().parent
for path in (HERE.parent,HERE):sys.path.insert(0,str(path))
import private_settlement_registered_session_replay as replay
from retained_session_fixture import SessionFixture,nonbenchmark_records
from retained_scope_fixture import build_scope,canonical_configuration_inputs
import retained_scope_foundation as foundation


class RetainedFixtureTests(unittest.TestCase):
    def build(self,failed=False,sessions=1):
        f=foundation.RegisteredScopeIntegrationTests();f.setUp();self.addCleanup(f.doCleanups);f.materialize()
        root=f.root/'campaigns/campaign-0';plan=f.plans['campaign-0']
        (root/'attempts').mkdir(mode=0o700);(root/'sessions').mkdir(mode=0o700)
        base={'scope_sha256':hashlib.sha256(f.scope_path.read_bytes()).hexdigest(),'campaign_id':'campaign-0',
              'plan_sha256':f.scope['campaigns'][0]['plan']['sha256']}
        images={'worker':{'path':'/exact/native/worker','sha256':'a'*64,'bytes':111},
                'validator':{'path':'/exact/native/validator','sha256':'b'*64,'bytes':222}}
        f.callback=replay.samples.RetainedSampleReplay(**images,owner_uid=os.geteuid(),group_utility_sha256='c'*64,
            listener_utility_sha256='d'*64,packet_utility={key:value for key,value in replay.samples.semantics.packets._utility().items() if key in ('path','sha256')})
        started=[];references=[];sids=[];epoch=100_000_000
        for ordinal,job in enumerate(plan['jobs'],1):
            if job['kind']=='benchmark':break
            started.append(nonbenchmark_records(root,plan,base,ordinal,job,10+ordinal*10))
        for session_number,descriptor in enumerate(plan['benchmark_sessions'][:sessions]):
            prepared=replay.runner.prepare_benchmark_session(plan,root,root,base,descriptor)
            fixture=SessionFixture(prepared,root,images,session_number,epoch,plan['benchmark_accounting']['outer_timeout_ms'])
            if failed:
                fixture.success(0);fixture.failure(1)
            else:
                for index in range(len(prepared['request']['attempts'])):fixture.success(index)
            closure,closed=fixture.close(failed=failed)
            started.extend(fixture.started_ids);references.append(closure);sids.append(descriptor['session_id']);epoch=closed+1
        f.write(root/'campaign-closure.json',{'version':1,'protocol':replay.control.PROTOCOL,**base,
            'closed_ns':closed+1,'quiescent':True,'started_request_ids':started,'reason':'fail_fast' if failed else 'recovered_interruption',
            'started_session_ids':sids,'session_closures':references})
        return f

    def test_complete_retained_session_five_warmups_three_measurements(self):
        f=self.build();result=f.invoke()
        self.assertEqual(result['accounting']['counts']['succeeded'],8)
        self.assertEqual(result['accounting']['counts']['not_started'],792)
        self.assertEqual(sum(json.loads(raw)['warmup'] for raw in result['successful_rows']),5)

    def test_exact_failed_attempt_preserves_earlier_success_and_tail(self):
        f=self.build(failed=True);result=f.invoke()
        self.assertEqual(result['accounting']['counts']['succeeded'],1)
        self.assertEqual(result['accounting']['counts']['failed'],1)
        self.assertEqual(result['accounting']['counts']['not_started'],798)

    def test_two_profiles_keep_distinct_session_owners_and_shared_vectors(self):
        f=self.build(sessions=2);result=f.invoke()
        samples=[json.loads(raw) for raw in result['successful_rows']]
        left=samples[:8];right=samples[8:]
        self.assertEqual(len(samples),16)
        self.assertNotEqual(left[0]['session_id'],right[0]['session_id'])
        self.assertEqual([row['economic_vector_sha256'] for row in left],[row['economic_vector_sha256'] for row in right])

    def test_complete_synthetic_scope_retains_failed_predecessor_and_all_per_seed_warmups(self):
        f=foundation.RegisteredScopeIntegrationTests();f.setUp();self.addCleanup(f.doCleanups);f.materialize()
        destination=f.root/'full';destination.mkdir(mode=0o700)
        paths,payloads,manifest,manifest_raw=canonical_configuration_inputs(f.commit)
        images={'worker':{'path':'/exact/native/worker','sha256':'a'*64,'bytes':111},
                'validator':{'path':'/exact/native/validator','sha256':'b'*64,'bytes':222}}
        result=build_scope(destination,commit=f.commit,hardware_path=Path('evidence/hardware.json'),
            hardware_payload=(f.root/'campaigns/campaign-0/hardware.json').read_bytes(),
            configuration_manifest_path=manifest,configuration_manifest_payload=manifest_raw,
            configuration_payloads={paths[n]:value for n,value in payloads.items()},images=images,plan_harness=f.harness)
        self.assertEqual(result['accounting']['counts'],dict(planned=1600,attempted=802,succeeded=801,
            failed=1,timed_out=0,not_started=798,incomplete=0))
        self.assertEqual(sum(row['warmup'] for row in result['rows']),501)
        self.assertTrue(result['report']['statistical_qualification_passed'])
        replay.runner.benchmark_report.validate_report_accounting(result['report'])
        self.assertEqual(replay.runner.benchmark_report.compare_baseline(result['report'],result['report']),[])

if __name__=='__main__':unittest.main()
