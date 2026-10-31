"""Synthetic closed public manifests exercise independent replay authority, never execution."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import zk_resource_harness as harness
import resource_observation_contract as contracts
import linux_owned_process_evidence_test as evidence_tests
from linux_owned_process_evidence_test import records_fixture,EXPECTED
import zk_resource_harness_test as harness_tests
from zk_resource_harness_test import DESCRIPTOR


class IndependentReplayTests(unittest.TestCase):
    """Reanchor mutations so the semantic guard, not a stale manifest, must refuse."""
    def setUp(self):
        """Use deterministic unit records with explicit synthetic delivery pins."""
        self.fixtures=harness_tests.PinnedEvidenceTests();self.fixtures.setUp()
        self.addCleanup(self.fixtures.doCleanups)

    def reanchor(self,run,report):
        """Bind the deliberately changed test bytes without claiming independent execution."""
        (run/'report.json').write_bytes(harness.control.canonical(report))
        with harness._EvidenceDirectory(run) as directory:
            snapshot=harness._EvidenceSnapshot(directory,DESCRIPTOR)
        self.assertEqual(snapshot.reasons,[])
        manifest={'schema':harness.MANIFEST_SCHEMA,'context':report['context'],
            'verdict':report['verdict'],'classification':'measured',
            'files':[snapshot.bindings[key] for key in sorted(snapshot.bindings)]}
        raw=harness.control.canonical(manifest);(run/'manifest.json').write_bytes(raw)
        return hashlib.sha256(raw).hexdigest()

    def fixture(self):
        """Build a fully accepted synthetic owned report with all baseline/lifecycle joins."""
        run,_,report=self.fixtures.fixture()
        values=records_fixture();process,snapshot,context=evidence_tests.EvidenceControls().snapshot(values)
        report['context'].update(context)
        record=json.loads((run/'child/record.json').read_bytes())
        record['identity'].update(context)
        record['process'].update(pid=process['pid'],cpu_user_ns=100_000_000,cpu_system_ns=100_000_000)
        (run/'child/context.json').write_bytes(harness.control.canonical(report['context']))
        (run/'child/record.json').write_bytes(harness.control.canonical(record))
        report['process'].update(process)
        report['observation_contract']=contracts.LINUX
        (run/'observations').mkdir(mode=0o700)
        for name,raw in snapshot.raw.items():(run/name).write_bytes(raw)
        entries,orphans,reasons=harness.collect_records(DESCRIPTOR,run,report['context'],report['process'],
            report['requirements'],required_linux=True,expected_delivery=EXPECTED)
        self.assertEqual(reasons,[])
        report.update(records=entries,orphans=orphans,reasons=[],verdict='accepted')
        pin=self.reanchor(run,report)
        verified=harness.read_pinned_run(run,pin,expected_observation_contract=contracts.LINUX,
                                        expected_delivery=EXPECTED)
        self.assertEqual(verified['report']['verdict'],'accepted')
        return run,pin,report

    def semantic_refusal(self,run,pin,expected=EXPECTED):
        """Join exact captured inner semantic refusal to the public rejected summary."""
        actual=harness.judge_records;captured=[]
        def judge(*args,**kwargs):
            result=actual(*args,**kwargs)
            captured.append((kwargs,result))
            return result
        with patch.object(harness,'judge_records',side_effect=judge):
            findings=harness.salvage_reasons(run,{},pin,
                expected_observation_contract=contracts.LINUX,expected_delivery=expected)
        self.assertEqual(findings,['reasons_not_reproducible',
                                  'report_verdict_differs_from_evidence','run_was_rejected'])
        self.assertEqual(len(captured),1)
        kwargs,result=captured[0]
        self.assertEqual(kwargs['snapshot'].reasons,[])
        self.assertIs(kwargs['required_linux'],True)
        self.assertEqual(kwargs['expected_delivery'],expected)
        self.assertIn('linux_owned_evidence_invalid',result[2])

    def test_owned_positive_and_stricter_replay_both_use_external_pins(self):
        """Both independent original/consumer judgments retain the same authority."""
        run,pin,report=self.fixture()
        with patch.object(harness.linux_evidence,'replay',wraps=harness.linux_evidence.replay) as replay:
            # A tighter unestablished requirement still replays the same evidence and refuses.
            findings=harness.salvage_reasons(run,{},pin,
                dict(report['requirements'],max_undivided_phase_ppm=700000),
                expected_observation_contract=contracts.LINUX,expected_delivery=EXPECTED)
        self.assertIn('consumer_requirement_not_established:max_undivided_phase_ppm',findings)
        self.assertEqual(replay.call_count,2)
        for call in replay.call_args_list:
            self.assertIs(call.kwargs['required_linux'],True)
            self.assertEqual(call.kwargs['expected_delivery'],EXPECTED)

    def test_every_external_pin_mismatch_refuses_semantically(self):
        """Correct report/manifest bytes cannot substitute independent delivery trust."""
        run,pin,_=self.fixture()
        for key in EXPECTED:
            wrong=dict(EXPECTED);wrong[key]='aarch64-unknown-linux-gnu' if key=='native_target' else 'f'*64
            with self.subTest(key=key):
                self.semantic_refusal(run,pin,wrong)

    def test_bundle_mode_and_host_stripping_cannot_downgrade_external_contract(self):
        """Remove every owned file/reference and reanchor ordinary mode; Linux still refuses."""
        run,_,report=self.fixture()
        del report['process']['sampled']['linux_owned']
        for name in harness.linux_evidence.PATHS.values():(run/name).unlink(missing_ok=True)
        report['observation_contract']=contracts.ORDINARY
        report['host']={'system':'Darwin'}
        pin=self.reanchor(run,report)
        findings=harness.salvage_reasons(run,{},pin,expected_observation_contract=contracts.LINUX,
                                        expected_delivery=EXPECTED)
        self.assertEqual(findings,['observation_contract_differs_from_requirement'])

    def test_whole_bundle_missing_under_owned_mode_refuses_semantically(self):
        """The declared mode is not permission to omit its actual evidence."""
        run,_,report=self.fixture();del report['process']['sampled']['linux_owned']
        for name in harness.linux_evidence.PATHS.values():(run/name).unlink(missing_ok=True)
        pin=self.reanchor(run,report)
        self.semantic_refusal(run,pin)

    def test_changed_adjacent_pins_and_reanchored_manifest_refuse(self):
        """A coordinated changed provenance/source claim cannot authorize itself."""
        run,_,report=self.fixture();path=run/harness.linux_evidence.PATHS['admission']
        document=json.loads(path.read_bytes());document['delivery']['source_manifest_sha256']='f'*64
        raw=harness.control.canonical(document);path.write_bytes(raw)
        report['process']['sampled']['linux_owned']['admission'].update(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
        pin=self.reanchor(run,report)
        self.semantic_refusal(run,pin)


if __name__=='__main__':unittest.main()
