"""Actual planner/request/persistence controls; no network qualification."""
from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

REPOSITORY = Path(__file__).resolve().parents[2]
for path in (Path(__file__).resolve().parents[1], Path(__file__).resolve().parent):
    sys.path.insert(0, str(path))
import private_settlement_release_runner as runner
import private_settlement_session_control as control


class SessionRecordTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.plan_root = self.root/'plan'
        self.plan_root.mkdir(mode=0o700)
        self.campaign = self.root/'campaign'
        self.campaign.mkdir(mode=0o700)
        (self.campaign/'attempts').mkdir(mode=0o700)
        (self.campaign/'sessions').mkdir(mode=0o700)
        rows, self.digests = [], {}
        for n in runner.PARTICIPANTS:
            path = self.plan_root/f'configurations/n{n}.json'
            runner.write_json(path, runner.build_configuration(n, seeds=list(range(10)), warmups=5, measured=30))
            reference = runner.file_binding(path, relative_to=self.plan_root)
            rows.append({'participants':n, **reference})
            self.digests[n] = reference['sha256']
        manifest = self.plan_root/'configuration-manifest.json'
        runner.write_json(manifest, {'configurations':rows})
        self.plan = {
            'commit':'a'*40,
            'hardware':{'sha256':'b'*64,'profile_sha256':'c'*64},
            'configuration_manifest':runner.file_binding(manifest, relative_to=self.plan_root),
            'benchmark_sessions':runner.benchmark_session_plan(self.digests,list(range(10)),5,30)['sessions'],
            'jobs':runner.build_jobs(self.digests,list(range(10)),5,30,runner.build_canary_manifest('a'*40)),
        }
        self.scope = {'scope_sha256':'d'*64,'campaign_id':'registered-native', 'plan_sha256':'e'*64}

    def tearDown(self):
        self.temporary.cleanup()

    def prepared(self):
        return runner.prepare_benchmark_session(self.plan,self.plan_root,self.campaign,
                                                self.scope,self.plan['benchmark_sessions'][0])

    def test_production_job_builder_uses_100_sessions_and_800_benchmark_attempts(self):
        jobs = self.plan['jobs']
        self.assertEqual(len(jobs),852)
        self.assertTrue(all(job['kind']=='fault' for job in jobs[:50]))
        self.assertTrue(all(job['kind']=='benchmark' for job in jobs[50:850]))
        self.assertEqual([job['variant'] for job in jobs[850:]],['left','right'])
        self.assertEqual(len(self.plan['benchmark_sessions']),100)
        self.assertTrue(all('run' not in job for job in jobs[50:850]))
        self.assertEqual({job['workload_manifest_sha256'] for job in jobs[50:66]},
                         {runner.object_digest(runner.attempt_accounting.build_benchmark_workload_policy(2))})

    def test_policy_files_are_exact_canonical_and_reject_rebound_substitution(self):
        references = runner.publish_workload_manifests(self.plan_root)
        digests = runner.validate_workload_manifests(references,self.plan_root)
        self.assertEqual(set(digests),set(runner.PARTICIPANTS))
        for row in references:
            raw = (self.plan_root/row['path']).read_bytes()
            self.assertEqual(raw,control.canonical(json.loads(raw)))
            self.assertEqual(json.loads(raw)['sponsor_reimbursement_amount'],5)
        row = references[0]
        path = self.plan_root/row['path']
        altered = json.loads(path.read_bytes());altered['sponsor_reimbursement_amount']=0
        path.write_bytes(control.canonical(altered))
        row.update(runner.file_binding(path,relative_to=self.plan_root))
        with self.assertRaises(runner.RunnerError):
            runner.validate_workload_manifests(references,self.plan_root)

    def test_exact_policy_rejects_types_or_removed_mandatory_fields(self):
        accounting = runner.attempt_accounting
        policy = accounting.build_benchmark_workload_policy(3)
        for key in policy:
            altered = copy.deepcopy(policy);del altered[key]
            with self.subTest(key=key),self.assertRaises(accounting.AccountingError):
                accounting.validate_benchmark_workload_policy(altered,3)
        for key in ('primary_amount_step','reserve_note_amount','version'):
            altered=copy.deepcopy(policy);altered[key]=True
            with self.assertRaises(accounting.AccountingError):
                accounting.validate_benchmark_workload_policy(altered,3)

    def test_prepared_requests_are_distinct_bound_and_have_no_attempt_starts(self):
        prepared = self.prepared()
        request = prepared['request']
        self.assertEqual(len(request['attempts']),8)
        self.assertEqual(request['warmups'],5)
        self.assertEqual(len({row['invocation_nonce'] for row in request['attempts']}),8)
        with control.RecordDirectory(self.campaign) as records:
            self.assertEqual(records.read(prepared['reference']),control.canonical(request))
            for index,row in enumerate(request['attempts']):
                value=control.decode(records.read(row['request']))
                self.assertNotIn('run',value)
                self.assertEqual(value['session_attempt_index'],index)
                self.assertEqual(value['payload']['warmup'],index<5)
                self.assertEqual(value['session_invocation_nonce'],request['session_invocation_nonce'])
                self.assertEqual(value['workload_manifest_sha256'],request['workload_manifest_sha256'])
                self.assertEqual({path.name for path in (self.campaign/row['output_directory']).iterdir()},
                                 {'request.json','evidence'})
                protocol=self.campaign/row['output_directory']/'evidence'/'benchmark-protocol'
                self.assertTrue(protocol.is_dir())
                self.assertEqual(list(protocol.iterdir()),[])
        self.assertFalse(any(self.campaign.rglob('started.json')))

    def test_first_attempt_is_durable_before_spawn_and_successor_requires_ack(self):
        prepared=self.prepared()
        with control.RecordDirectory(self.campaign) as records:
            started=runner.publish_benchmark_session_start(prepared,records=records,
                command=['/controlled/adapter','benchmark-session'],harness={'sha256':'f'*64,'bytes':100})
            first=runner.publish_benchmark_attempt_start(prepared,0,records=records,session_started=started,
                ordinal=51,outer_timeout_ms=7200000,preceding_acceptance=None)
            value=control.decode(records.read(first))
            self.assertEqual(value['request'],prepared['request']['attempts'][0]['request'])
            self.assertEqual(value['session_started'],started)
            self.assertIsNone(value['preceding_acceptance'])
            self.assertNotIn('exit_code',value)
            self.assertNotIn('owned_process_group_gone',value)
            with self.assertRaises(runner.RunnerError):
                runner.publish_benchmark_attempt_start(prepared,1,records=records,session_started=started,
                    ordinal=52,outer_timeout_ms=7200000,preceding_acceptance=None)
            second=self.campaign/prepared['request']['attempts'][1]['output_directory']/'started.json'
            self.assertFalse(second.exists())
            with self.assertRaises(FileExistsError):
                runner.publish_benchmark_attempt_start(prepared,0,records=records,session_started=started,
                    ordinal=51,outer_timeout_ms=7200000,preceding_acceptance=None)

    def test_bad_session_order_rejects_before_any_request_file(self):
        for alteration in ('reorder','warmup','policy'):
            plan=copy.deepcopy(self.plan)
            if alteration=='reorder':
                plan['jobs'][50],plan['jobs'][51]=plan['jobs'][51],plan['jobs'][50]
            elif alteration=='warmup':
                plan['jobs'][50]['warmup']=False
            else:
                plan['jobs'][50]['workload_manifest_sha256']='9'*64
            with self.subTest(alteration=alteration),self.assertRaises(runner.RunnerError):
                runner.prepare_benchmark_session(plan,self.plan_root,self.campaign,self.scope,
                                                 plan['benchmark_sessions'][0])
        self.assertEqual(list((self.campaign/'sessions').iterdir()),[])
        self.assertEqual(list((self.campaign/'attempts').iterdir()),[])

    def test_source_request_change_or_wrong_ordinal_cannot_start(self):
        prepared=self.prepared()
        with control.RecordDirectory(self.campaign) as records:
            started=runner.publish_benchmark_session_start(prepared,records=records,command=['/controlled/adapter'],
                harness={'sha256':'f'*64,'bytes':100})
            with self.assertRaises(runner.RunnerError):
                runner.publish_benchmark_attempt_start(prepared,0,records=records,session_started=started,
                    ordinal=1,outer_timeout_ms=7200000,preceding_acceptance=None)
            path=self.campaign/prepared['request']['attempts'][0]['request']['path']
            value=json.loads(path.read_bytes());value['invocation_nonce']='1'*64
            path.write_bytes(control.canonical(value))
            with self.assertRaises(control.SessionProtocolError):
                runner.publish_benchmark_attempt_start(prepared,0,records=records,session_started=started,
                    ordinal=51,outer_timeout_ms=7200000,preceding_acceptance=None)


if __name__=='__main__':
    unittest.main()
