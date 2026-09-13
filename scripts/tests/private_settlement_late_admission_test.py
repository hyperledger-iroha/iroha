"""Late admission failures through actual public source/image postcheck owners."""
from contextlib import contextmanager,ExitStack
from pathlib import Path
import unittest
from unittest.mock import patch

import private_settlement_registered_session_replay as replay
import private_settlement_retained_fixture_test as fixtures
from scripts.tests.private_settlement_registered_accounting_fixture import fixture_admission


class LateAdmissionTests(unittest.TestCase):
    def case(self):
        f=fixtures.foundation.RegisteredScopeIntegrationTests();f.setUp();self.addCleanup(f.doCleanups);f.materialize()
        return f

    @contextmanager
    def admission(self,f,*,source_side_effect=None):
        with fixture_admission(source_side_effect=source_side_effect) as admitted:
            # The plan fixture intentionally has its own synthetic Python
            # harness binding. Native image files remain actual admitted files.
            actual=replay.runner.verify_harness
            def harness(path):
                return f.harness if Path(path)==admitted['plan_harness'] else actual(path)
            with patch.object(replay.runner,'verify_harness',side_effect=harness):yield admitted

    def test_account_scope_late_source_refusal_publishes_no_counts(self):
        f=self.case();path=f.root/'counts.json';calls=[]
        def source(*args):
            calls.append(args)
            if len(calls)==2:raise replay.runner.RunnerError('exact synthetic late source refusal')
        with self.admission(f,source_side_effect=source) as admission:
            with self.assertRaisesRegex(ValueError,'late source refusal'):
                replay.runner.write_benchmark_scope_accounting(f.scope_path,path,**admission)
        self.assertEqual(len(calls),2);self.assertFalse(path.exists())

    def test_account_scope_late_native_image_change_publishes_no_counts(self):
        f=self.case();path=f.root/'counts.json';open_scope=replay._open_closed_scope
        with self.admission(f) as admission:
            image=admission['worker_path'];original=image.read_bytes()
            @contextmanager
            def changed(*args,**kwargs):
                with open_scope(*args,**kwargs) as held:yield held
                image.write_bytes(original+b'changed')
            try:
                with patch.object(replay,'_open_closed_scope',changed),self.assertRaisesRegex(ValueError,'native executable changed'):
                    replay.runner.write_benchmark_scope_accounting(f.scope_path,path,**admission)
            finally:image.write_bytes(original)
        self.assertFalse(path.exists())

    def test_finalize_late_native_image_change_never_publishes_final_fragment(self):
        f=self.case();path=f.root/'fragment.json';open_scope=replay._open_closed_scope
        with self.admission(f) as admission:
            image=admission['validator_path'];original=image.read_bytes()
            @contextmanager
            def changed(*args,**kwargs):
                with open_scope(*args,**kwargs) as held:yield held
                image.write_bytes(original+b'changed')
            try:
                # Only fragment preparation is substituted to isolate the
                # actual postcheck->publication edge. No qualified report is
                # produced; all public source/native admission checks execute.
                with patch.object(replay,'_open_closed_scope',changed),patch.object(replay.runner,'_finalize_held_scope',
                        return_value=(path,{'explicit_synthetic_preparation':True})),self.assertRaisesRegex(ValueError,'native executable changed'):
                    replay.runner.finalize_registered_scope(f.scope_path,f.root/'output',qualification_campaign_id='campaign-0',**admission)
            finally:image.write_bytes(original)
        self.assertFalse(path.exists());self.assertFalse(path.with_suffix('.pending.json').exists())

if __name__=='__main__':unittest.main()
