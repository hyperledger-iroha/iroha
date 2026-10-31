"""Real retained-file and modelled ordering controls; no native module/child executes."""
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import compute_workspace_source_manifest as source
import workspace_source_keeper as custody
import linux_observer_delivery as delivery
import release_process_bridge as bridge
import zk_resource_harness as harness
from linux_observer_delivery_test import PINS,arguments,public_result


class SourceOriginalControls(unittest.TestCase):
    """The new lower API retains actual originals but grants no census authority."""
    def setUp(self):
        """Open genuine temporary originals without a Git or native-process fixture."""
        self.temporary=tempfile.TemporaryDirectory();self.addCleanup(self.temporary.cleanup)
        self.root=Path(self.temporary.name);self.path=self.root/'input'
        self.path.write_bytes(b'original source')
        self.keeper=custody.SourceKeeper(self.root);self.addCleanup(self.keeper.close)
        self.keeper.capture(['input','absent'])
        self.token=source._RETAINED_SOURCE.set(self.keeper)
        self.addCleanup(source._RETAINED_SOURCE.reset,self.token)
        self.owner=source.RetainedWorkspaceSource(self.root,'a'*64,self.keeper)

    def test_original_check_is_process_free_and_rejects_actual_byte_change(self):
        """No hidden Git process is needed to reject a changed held source object."""
        with patch.object(subprocess,'Popen',side_effect=AssertionError('unexpected process')) as pop, \
             patch.object(source,'native_artifact_workspace_source_manifest',side_effect=AssertionError('unexpected census')):
            self.owner.revalidate_originals()
            self.path.write_bytes(b'changed source')
            with self.assertRaises(custody.SourceCustodyError):self.owner.revalidate_originals()
            pop.assert_not_called()
        self.assertTrue(self.keeper.failed)

    def test_original_context_and_recorded_absence_remain_mandatory(self):
        """Captured missing inputs cannot appear, and a different context cannot borrow custody."""
        token=source._RETAINED_SOURCE.set(None)
        try:
            with self.assertRaises(source.SourceSealError):self.owner.revalidate_originals()
        finally:source._RETAINED_SOURCE.reset(token)
        self.assertTrue(self.keeper.failed)

    def test_original_check_refuses_a_captured_absent_member_appearing(self):
        """The process-free check retains its actual captured absence obligation."""
        (self.root/'absent').write_bytes(b'new input')
        with patch.object(subprocess,'Popen',side_effect=AssertionError('unexpected process')):
            with self.assertRaises(custody.SourceCustodyError):self.owner.revalidate_originals()
        self.assertTrue(self.keeper.failed)

    def test_full_revalidation_still_calls_canonical_census_and_latches_refusal(self):
        """A held-original success never substitutes for the final canonical aggregate."""
        with patch.object(source,'native_artifact_workspace_source_manifest',return_value='a'*64) as census:
            self.owner.revalidate();census.assert_called_once_with(self.root)
        with patch.object(source,'native_artifact_workspace_source_manifest',return_value='b'*64):
            with self.assertRaises(source.SourceSealError):self.owner.revalidate()
        self.assertTrue(self.keeper.failed)

    def test_bridge_keeps_full_and_original_callbacks_distinct(self):
        """Adding the narrow callback does not redirect existing process-run validation."""
        library=SimpleNamespace(**{name:Mock() for name in (
            'iroha_process_spawn_v1','iroha_process_read_v1','iroha_process_try_wait_v1','iroha_process_close_v1')})
        full=Mock();originals=Mock()
        value=bridge.Bridge(library,validate=full,validate_originals=originals)
        value.validate_originals();originals.assert_called_once();full.assert_not_called()
        value.validate();full.assert_called_once()


class StagedDeliveryControls(unittest.TestCase):
    """Exact callback order encloses native preparation, closure and full final census."""
    def setUp(self):
        """Reserve a modelled original while exercising the production stage methods."""
        self.events=[]
        self.owner=delivery.ObserverDelivery('/source','/artifact',PINS,cleanup_original=lambda owner:None)
        self.owner.registry=SimpleNamespace(keeper=SimpleNamespace(revalidate=lambda:self.events.append('registry')))
        self.owner.bridge=SimpleNamespace(validate=lambda:self.events.append('full'),
            validate_originals=lambda:self.events.append('originals'))
        self.active=SimpleNamespace(native=SimpleNamespace(closed=False),first=None,module_owner=self.owner)
        def retain(stage,error):
            """Latch the exact first ordinary failure as the actual lifetime does."""
            if self.active.first is None:self.active.first=error
        self.active.retain=retain;self.owner.active=self.active

    def test_live_stage_has_no_subprocess_and_full_final_check_requires_closed_observer(self):
        """Full source checks bracket the bounded process-free observation stage."""
        self.owner.begin_observation(self.active)
        self.assertEqual(self.events,['registry','full'])
        with patch.object(subprocess,'Popen',side_effect=AssertionError('unexpected process')) as pop:
            self.owner.validate()
            with self.assertRaises(ValueError):self.owner.finish_observation(self.active)
            pop.assert_not_called()
        self.assertEqual(self.events,['registry','full','registry','originals'])
        self.assertTrue(self.owner.observing)
        self.active.native.closed=True;self.owner.finish_observation(self.active)
        self.assertEqual(self.events[-2:],['registry','full'])
        self.assertTrue(self.owner.observation_checked and self.owner.observation_validated)
        self.assertFalse(self.owner.observing)

    def test_preparation_callback_order_precedes_native_event_setup(self):
        """The real lifetime override runs admission before the unchanged native prepare."""
        owner=delivery.DeliveryLifetime.__new__(delivery.DeliveryLifetime)
        owner.module_owner=SimpleNamespace(begin_observation=lambda active:self.events.append('full'))
        with patch.object(delivery.lifetime.OriginalLifetime,'prepare',
                side_effect=lambda *args:self.events.append('native_prepare')) as prepare:
            owner.prepare(1000,5)
        self.assertEqual(self.events,['full','native_prepare']);prepare.assert_called_once_with(1000,5)

    def test_final_census_failure_is_latched_without_parking_closed_owners(self):
        """A refused final census stays failed while actual native cleanup is complete."""
        self.owner.begin_observation(self.active);self.active.native.closed=True
        failure=OSError('final source failure')
        self.owner.bridge.validate=Mock(side_effect=failure)
        with self.assertRaises(OSError) as got:self.owner.finish_observation(self.active)
        self.assertIs(got.exception,failure);self.assertIs(self.active.first,failure)
        self.assertIs(self.owner.first,failure)
        self.assertTrue(self.owner.observation_checked);self.assertFalse(self.owner.observation_validated)
        self.owner.finish_observation(self.active)
        self.owner.bridge.validate.assert_called_once()
        self.assertIs(self.owner.first,failure)

    def test_prior_ordinary_first_survives_later_full_census_failure(self):
        """Final source diagnostics cannot replace an earlier original exception object."""
        first=RuntimeError('first');self.active.first=first
        self.owner.begin_observation(self.active);self.active.native.closed=True
        self.owner.bridge.validate=Mock(side_effect=OSError('later'))
        with self.assertRaises(OSError):self.owner.finish_observation(self.active)
        self.assertIs(self.owner.first,first);self.assertIs(self.active.first,first)


class PublicSummaryControls(unittest.TestCase):
    """An accepted manifest acknowledgement cannot precede final delivery custody."""
    def test_summary_is_printed_only_after_complete_context_exit(self):
        """The public caller receives exactly the actual summary after both close boundaries."""
        output=io.StringIO();events=[];owner=SimpleNamespace(factory=object())
        class Context:
            """Model full delivery close and inspect the actual public output boundary."""
            def __enter__(self):return owner
            def __exit__(self,*args):
                self_output=output.getvalue()
                if self_output:raise AssertionError('summary escaped before final close')
                events.append('closed')
        with redirect_stdout(output),patch.object(harness.sys,'platform','linux'), \
             patch.object(delivery,'ObserverDelivery',return_value=Context()), \
             patch.object(harness,'run_command',side_effect=public_result(0)):
            self.assertEqual(harness.public_run(arguments()),0)
        self.assertEqual(events,['closed'])
        self.assertEqual(json.loads(output.getvalue())['manifest_sha256'],'e'*64)

    def test_late_final_failure_withholds_ack_and_preserves_existing_report(self):
        """Immutable report bytes remain diagnostics; failed context yields no accepted receipt."""
        output=io.StringIO();first=OSError('final custody failed');owner=SimpleNamespace(factory=object())
        with tempfile.TemporaryDirectory() as directory:
            report=Path(directory)/'report.json';report.write_bytes(b'{"verdict":"accepted"}\n')
            original=report.read_bytes()
            class Context:
                """Inject the final boundary after the runner has authored its report."""
                def __enter__(self):return owner
                def __exit__(self,*args):raise first
            with redirect_stdout(output),patch.object(harness.sys,'platform','linux'), \
                 patch.object(delivery,'ObserverDelivery',return_value=Context()), \
                 patch.object(harness,'run_command',side_effect=public_result(0)):
                with self.assertRaises(harness.HarnessRefusal) as got:harness.public_run(arguments())
            self.assertIs(got.exception.__cause__,first)
            self.assertEqual(output.getvalue(),'');self.assertEqual(report.read_bytes(),original)

    def test_second_summary_refuses_and_still_exits_the_original_context(self):
        """The buffered acknowledgement is one bounded slot, not an append-only log."""
        output=io.StringIO();owner=SimpleNamespace(factory=object())
        context=Mock();context.__enter__=Mock(return_value=owner);context.__exit__=Mock(return_value=False)
        def twice(*args,**kwargs):
            """Exercise the actual capture callback twice without a native child."""
            public_result(0)(*args,**kwargs);public_result(0)(*args,**kwargs)
        with redirect_stdout(output),patch.object(harness.sys,'platform','linux'), \
             patch.object(delivery,'ObserverDelivery',return_value=context), \
             patch.object(harness,'run_command',side_effect=twice):
            with self.assertRaises(harness.HarnessRefusal):harness.public_run(arguments())
        self.assertEqual(output.getvalue(),'');context.__exit__.assert_called_once()


if __name__=='__main__':unittest.main()
