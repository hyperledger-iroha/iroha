"""Canonical retained archive regressions; all native records are synthetic."""
from __future__ import annotations
import copy,json,tempfile,unittest
from contextlib import contextmanager
from pathlib import Path,PurePosixPath

import private_settlement_registered_session_replay as replay
import private_settlement_retained_scope_publication as publication
from scripts.tests.private_settlement_registered_accounting_fixture import RUNNER,fixture_admission,raw
from scripts.tests import private_settlement_release_evidence_test as release_fixture


class RegisteredScopeReleaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.area=tempfile.TemporaryDirectory();cls.root=Path(cls.area.name).resolve()
        cls.manifest=json.loads(release_fixture.PrivateSettlementReleaseEvidenceTests().make_bundle(cls.root).read_bytes())
        cls.artifacts=cls.manifest['artifacts'];cls.scope=cls.root/'accounting/scope.json'
        cls.plan,_=RUNNER.load_plan(cls.root/'accounting/campaigns/campaign-1-complete/frozen-plan.json')
        cls.report=json.loads((cls.root/'reports/benchmark-report-v1.json').read_bytes())
        cls.rows=[json.loads(value) for value in (cls.root/'evidence/benchmark_raw.jsonl').read_text().splitlines()]

    @classmethod
    def tearDownClass(cls):cls.area.cleanup()

    @contextmanager
    def changed_records(self):
        originals={}
        def replace(path,value):
            originals.setdefault(path,path.read_bytes());payload=raw(value)
            path.write_bytes(payload);return RUNNER.attempt_accounting.accounting_file_binding(payload)
        try:yield replace
        finally:
            for path,payload in originals.items():path.write_bytes(payload)

    @contextmanager
    def held(self,root=None):
        with fixture_admission() as admission:
            with replay.open_admitted_scope((root or self.root)/'accounting/scope.json',**admission) as held:yield held

    def verify(self,root=None,artifacts=None):
        objects=[RUNNER.release_evidence.Artifact(row['kind'],PurePosixPath(row['path']),row['sha256'],row['bytes'])
                 for row in (self.artifacts if artifacts is None else artifacts)]
        with self.held(root) as held:
            publication.validate_archive_inventory(held,root or self.root,objects)
            claimed=json.loads(((root or self.root)/'reports/benchmark-accounting-v1.json').read_bytes())
            if claimed!=held.result['accounting']:raise ValueError('public benchmark counts differ')
            return held.result['accounting']

    def test_canonical_replay_preserves_accepted_warmup_before_failed_predecessor(self):
        self.assertEqual(self.verify()['counts'],dict(planned=1600,attempted=802,succeeded=801,failed=1,timed_out=0,not_started=798,incomplete=0))
        self.assertEqual(len(self.rows),801)

    def test_controlled_archive_retains_original_records_and_private_transport_modes(self):
        with tempfile.TemporaryDirectory() as area,self.held() as held:
            destination=Path(area).resolve()
            artifacts,accounting,rows=RUNNER.archive_benchmark_scope(held,destination)
            self.assertEqual(self.verify(destination,artifacts),accounting);self.assertEqual(rows,self.rows)
            for path in (destination/'accounting').rglob('benchmark-protocol'):
                self.assertEqual(path.stat().st_mode&0o777,0o700)
                for record in path.iterdir():self.assertEqual(record.stat().st_mode&0o777,0o600)

    def test_rebound_failed_request_cannot_change_embedded_configuration(self):
        campaign=self.root/'accounting/campaigns/campaign-0-failed'
        attempt=next(path for path in sorted((campaign/'attempts').iterdir(),reverse=True) if (path/'started.json').exists())
        with self.changed_records() as replace:
            request=json.loads((attempt/'request.json').read_bytes());request['configuration']['consensus']['mandatory_signed_rs16_da_rbc']=False
            binding=replace(attempt/'request.json',request)
            started=json.loads((attempt/'started.json').read_bytes());started['request'].update(binding);replace(attempt/'started.json',started)
            # The exact frozen request/session graph must reject a consistently
            # rebound public claim; it is never accepted from byte hashes alone.
            with self.assertRaises((ValueError,OSError)):
                with self.held():pass

    def test_rehashed_public_accounting_cannot_hide_failed_predecessor(self):
        artifacts=copy.deepcopy(self.artifacts)
        entry=next(row for row in artifacts if row['kind']=='benchmark_accounting_report')
        path=self.root/entry['path']
        with self.changed_records() as replace:
            value=json.loads(path.read_bytes());value['counts']['failed']=0;entry.update(replace(path,value))
            with self.assertRaisesRegex(ValueError,'public benchmark counts'):self.verify(artifacts=artifacts)

    def test_controlled_record_cannot_be_omitted_or_relabelled(self):
        index=next(i for i,row in enumerate(self.artifacts) if row['path'].endswith('benchmark-sample.json'))
        for mutation in ('omit','relabel'):
            with self.subTest(mutation=mutation):
                artifacts=copy.deepcopy(self.artifacts)
                if mutation=='omit':artifacts.pop(index)
                else:artifacts[index]['kind']='operator_log'
                with self.assertRaisesRegex(ValueError,'inventory'):self.verify(artifacts=artifacts)

    def test_completed_campaign_cannot_conceal_unsuccessful_nonbenchmark_process(self):
        campaign=self.root/'accounting/campaigns/campaign-1-complete'
        path=sorted((campaign/'attempts').iterdir())[0]/'process-outcome.json'
        for mutation in ({'exit_code':2,'passed':True},{'exit_code':2,'passed':False},{'elapsed_ms':-1},
                {'timed_out':True,'completion_kind':'outer_deadline','passed':False,'exit_code':-15,
                 'elapsed_ms':RUNNER.DEFAULT_HARNESS_TIMEOUT_SECONDS*1000}):
            with self.subTest(mutation=mutation),self.changed_records() as replace:
                value=json.loads(path.read_bytes());value.update(mutation);replace(path,value)
                with self.assertRaises(ValueError):
                    with self.held():pass

    def test_finalization_requires_registered_completed_qualification_campaign(self):
        with self.held() as held,fixture_admission() as admission:
            for campaign in ('unregistered','campaign-0-failed'):
                with self.subTest(campaign=campaign),self.assertRaises(RUNNER.RunnerError):
                    RUNNER._finalize_held_scope(held,self.root/'unpublished',qualification_campaign_id=campaign,
                        source_root=admission['source_root'],admission=admission)
                self.assertFalse((self.root/'unpublished').exists())

    def test_fail_fast_predecessor_cannot_dispatch_after_failed_fault_process(self):
        path=sorted((self.root/'accounting/campaigns/campaign-0-failed/attempts').iterdir())[0]/'process-outcome.json'
        with self.changed_records() as replace:
            value=json.loads(path.read_bytes());value.update(passed=False,exit_code=2,error='synthetic earlier fault failure');replace(path,value)
            with self.assertRaisesRegex(ValueError,'session owner started after an unsuccessful full-plan predecessor'):
                with self.held():pass

if __name__=='__main__':unittest.main()
