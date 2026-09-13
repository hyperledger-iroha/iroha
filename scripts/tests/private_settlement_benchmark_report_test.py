"""Retained report and baseline regressions over explicitly synthetic records."""
from __future__ import annotations
import copy,contextlib,hashlib,io,json,tempfile,unittest
from pathlib import Path

import private_settlement_benchmark_report as MODULE
import private_settlement_registered_session_replay as replay
import private_settlement_retained_scope_publication as publication
from scripts.tests import private_settlement_release_evidence_test as release_fixture
from scripts.tests.private_settlement_registered_accounting_fixture import fixture_admission
import private_settlement_retained_fixture_test as retained


class PrivateSettlementBenchmarkReportTests(unittest.TestCase):
    """Keep authenticated inputs held while testing exported rows and policy."""
    @classmethod
    def setUpClass(cls):
        cls.stack=contextlib.ExitStack()
        cls.root=Path(cls.stack.enter_context(tempfile.TemporaryDirectory())).resolve()
        release_fixture.PrivateSettlementReleaseEvidenceTests().make_bundle(cls.root)
        admission=cls.stack.enter_context(fixture_admission())
        cls.held=cls.stack.enter_context(replay.open_admitted_scope(cls.root/'accounting/scope.json',**admission))
        cls.report=MODULE.build_report(cls.held,100)
        cls.rows=[json.loads(value) for value in cls.held.successful_rows]

    @classmethod
    def tearDownClass(cls):cls.stack.close()

    def raw(self,rows):
        area=self.enterContext(tempfile.TemporaryDirectory())
        path=Path(area)/'samples.jsonl'
        path.write_text(''.join(json.dumps(value,allow_nan=False)+'\n' for value in rows))
        return path

    def test_complete_matrix_reports_every_profile_and_participant(self):
        self.assertEqual(set(self.report['profiles']),set(MODULE.PROFILES))
        self.assertEqual(set(self.report['profiles']['private']),{str(n) for n in MODULE.REQUIRED_PARTICIPANTS})
        for buckets in self.report['profiles'].values():
            for bucket in buckets.values():
                self.assertEqual(bucket['measured_runs'],30)
                self.assertEqual(bucket['stages_ms']['end_to_end']['count'],30)
                self.assertGreaterEqual(bucket['ready_sessions'],10)
        publication.require_qualified(self.report)

    def test_missing_real_network_participant_bucket_is_rejected(self):
        value=copy.deepcopy(self.report);value['profiles']['private']['16']['measured_runs']=0
        with self.assertRaises(ValueError):publication.require_qualified(value)

    def test_baseline_policy_allows_small_shift_and_rejects_large_shift(self):
        def shifted(scale):
            result=copy.deepcopy(self.report)
            for buckets in result['profiles'].values():
                for bucket in buckets.values():
                    for summary in bucket['stages_ms'].values():
                        for key in ('p50','p95','p99','mad'):summary[key]*=scale
                        for key in ('p50_ci95','p95_ci95','p99_ci95'):summary[key]=[v*scale for v in summary[key]]
            return result
        self.assertEqual(MODULE.compare_baseline(shifted(1.05),self.report),[])
        result=MODULE.compare_baseline(shifted(1.25),self.report)
        self.assertTrue({row['quantile'] for row in result}>={'p95','p99'})

    def test_baseline_from_different_hardware_is_rejected(self):
        value=copy.deepcopy(self.report);value['environment']['hardware_profile_sha256']='e'*64
        with self.assertRaisesRegex(MODULE.EvidenceError,'identical hardware profiles and configurations'):
            MODULE.compare_baseline(self.report,value)

    def test_baseline_allows_new_commit_bound_hardware_artifact(self):
        value=copy.deepcopy(self.report);value['commit']='d'*40;value['environment']['hardware_sha256']='e'*64
        self.assertEqual(MODULE.compare_baseline(value,self.report),[])

    def test_baseline_rejects_malformed_configuration_binding(self):
        value=copy.deepcopy(self.report);value['environment']['configuration_sha256_by_participants']=[]
        with self.assertRaisesRegex(MODULE.EvidenceError,'environment is malformed'):
            MODULE.compare_baseline(self.report,value)

    def test_percentile_and_mad_are_deterministic(self):
        values={'a'*64:{'c'*64:1.0,'d'*64:2.0},'b'*64:{'e'*64:3.0,'f'*64:4.0}}
        first=MODULE.summarize_session_values(values,binding=hashlib.sha256(b'fixed').digest(),bootstrap_iterations=100)
        second=MODULE.summarize_session_values(values,binding=hashlib.sha256(b'fixed').digest(),bootstrap_iterations=100)
        self.assertEqual(first,second);self.assertEqual(first['p50'],2.5);self.assertEqual(first['mad'],1.0)

    def test_mixed_source_commits_are_rejected(self):
        rows=copy.deepcopy(self.rows);rows[-1]['commit']='b'*40
        with self.assertRaises(ValueError):publication.validate_raw([self.raw(rows)],self.held)

    def test_mixed_hardware_or_n_configuration_is_rejected(self):
        for field in ('hardware_sha256','hardware_profile_sha256','configuration_sha256'):
            with self.subTest(field=field):
                rows=copy.deepcopy(self.rows);rows[-1][field]='e'*64
                with self.assertRaises(ValueError):publication.validate_raw([self.raw(rows)],self.held)

    def test_sample_requires_exact_retained_identity(self):
        for field,value in [('attempt_id',None),('request_id','bad'),('session_id','bad'),
                            ('session_attempt_index',True),('version',True),('participants',3.0)]:
            with self.subTest(field=field):
                rows=copy.deepcopy(self.rows)
                if value is None:rows[0].pop(field)
                else:rows[0][field]=value
                with self.assertRaises((ValueError,KeyError)):publication.validate_raw([self.raw(rows)],self.held)

    def test_same_coordinates_from_distinct_sessions_survive_but_reused_ids_fail(self):
        first=self.rows[0]
        matching=[row for row in self.rows if (row['participants'],row['seed'],row['session_attempt_index'],row['profile'])==
                  (first['participants'],first['seed'],first['session_attempt_index'],first['profile'])]
        self.assertEqual(len(matching),2)
        self.assertEqual(len({row['attempt_id'] for row in matching}),2)
        publication.validate_raw([self.raw(self.rows)],self.held)
        with self.assertRaises(ValueError):publication.validate_raw([self.raw([*self.rows,self.rows[0]])],self.held)

    def test_raw_jsonl_rejects_duplicate_fields_before_normalization(self):
        path=self.raw(self.rows)
        path.write_text('{"version":1,'+json.dumps(self.rows[0])[1:]+'\n')
        with self.assertRaises(ValueError):publication.validate_raw([path],self.held)

    def test_failed_predecessor_retains_accepted_measurements_and_warmup_split(self):
        counts=self.report['accounting']['counts']
        self.assertEqual(counts,dict(planned=1600,attempted=802,succeeded=801,failed=1,timed_out=0,not_started=798,incomplete=0))
        self.assertEqual(sum(row['warmup'] for row in self.rows),501)
        predecessor=[row for row in self.report['accounting']['rows'] if row['campaign_id']=='campaign-0-failed']
        self.assertEqual(sum(row['state']=='succeeded' for row in predecessor),1)
        self.assertTrue(next(row for row in predecessor if row['state']=='succeeded')['warmup'])

    def test_omitted_predecessor_or_retained_success_is_rejected(self):
        with self.assertRaisesRegex(ValueError,'omits an accepted'):
            publication.validate_raw([self.raw(self.rows[1:])],self.held)
        packets=self.held.packets[1:]
        with self.assertRaisesRegex(ValueError,'campaign inventory'):
            replay.accounting.reduce_registered_scope(self.held.scope_raw,packets,self.held.successful_rows,
                worker_command=[self.held.callback.images['worker']['path'],replay.samples.adapter.WORKER_TEST,
                    '--exact','--ignored','--nocapture','--test-threads=1'],
                worker_image={key:self.held.callback.images['worker'][key] for key in ('sha256','bytes')},
                validate_success=self.held.callback)

    def test_duplicate_or_failed_attempt_metrics_are_rejected(self):
        failed=next(row['attempt_id'] for row in self.report['accounting']['rows'] if row['state']=='failed')
        for row in (self.rows[0],{**self.rows[0],'attempt_id':failed}):
            with self.subTest(attempt=row['attempt_id']),self.assertRaises(ValueError):
                publication.validate_raw([self.raw([*self.rows,row])],self.held)

    def test_changed_metric_and_missing_terminal_cannot_be_reported(self):
        rows=copy.deepcopy(self.rows);rows[0]['cpu_seconds']=999.0
        with self.assertRaises(ValueError):publication.validate_raw([self.raw(rows)],self.held)
        helper=retained.RetainedFixtureTests();self.addCleanup(helper.doCleanups);case=helper.build()
        terminal=next((case.root/'campaigns/campaign-0/attempts').rglob('rust-result.json'))
        terminal.unlink()
        with self.assertRaises((ValueError,OSError)):case.invoke()

    def test_registered_bootstrap_policy_is_mandatory(self):
        for count in (101,True):
            with self.subTest(count=count),self.assertRaisesRegex(ValueError,'bootstrap policy'):
                MODULE.build_report(self.held,count)
        with self.assertRaisesRegex(ValueError,'held canonical'):
            MODULE.build_report({'attempts':[]},100)

    def test_baseline_requires_exact_complete_accounting_and_matching_cohorts(self):
        for mutation in ('missing','counter','cohort','private','incomplete','header','summary'):
            value=copy.deepcopy(self.report)
            if mutation=='missing':value.pop('accounting')
            elif mutation=='counter':value['accounting']['counts']['failed']=0
            elif mutation=='cohort':value['profiles']['private']['3']['measured_runs']=31
            elif mutation=='private':value['accounting']['rows'][0]['private_key']='synthetic-forbidden-field'
            elif mutation=='incomplete':value['accounting']['accounting_complete']=False
            elif mutation=='header':value['version']=True
            else:value['profiles']['private']['3']['stages_ms']['end_to_end']['count']=31
            with self.subTest(mutation=mutation),self.assertRaises(MODULE.EvidenceError):
                MODULE.compare_baseline(self.report,value)

    def test_cli_requires_registered_scope_and_native_admission(self):
        old=['--input','raw.jsonl','--scope','scope.json','--output','report.json']
        with contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):MODULE.parse_args(old)
        args=[*old,*[value for name in ('source-root','plan-harness','smoke-campaign','worker','validator') for value in ('--'+name,'missing')]]
        self.assertEqual(MODULE.parse_args(args).scope,Path('scope.json'))
        with tempfile.TemporaryDirectory() as area,contextlib.redirect_stderr(io.StringIO()):
            args[args.index('--output')+1]=str(Path(area)/'report.json')
            self.assertEqual(MODULE.main(args),2);self.assertFalse((Path(area)/'report.json').exists())

if __name__=='__main__':unittest.main()
