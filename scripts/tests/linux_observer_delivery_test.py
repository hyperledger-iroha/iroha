"""Pure public delivery/independent-replay controls; no module or child is launched."""
from __future__ import annotations
from contextlib import ExitStack
import copy
import hashlib
import importlib.util
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import ANY,Mock,patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import linux_observer_delivery as delivery
import resource_observation_contract as contracts
import zk_resource_harness as harness

PINS=delivery.DeliveryPins('a'*64,'b'*64,'c'*64,'x86_64-unknown-linux-gnu')
EXPECTED=PINS.expected()


def arguments(**overrides):
    """Return explicit independent caller inputs without loading any evidence."""
    return SimpleNamespace(repository='/measurement',observer_source_root='/canonical-observer-source',observer_bridge_root='/artifact',
        trusted_observer_sha256='a'*64,trusted_observer_provenance_sha256='b'*64,
        trusted_source_manifest_sha256='c'*64,observer_native_target='x86_64-unknown-linux-gnu',
        **overrides)


def public_result(status):
    """Model only the workload runner's bounded summary callback, never native custody."""
    def run(*args,**kwargs):
        """Supply the one actual API summary slot consumed after context exit."""
        kwargs['summary_sink']({'run_directory':'/retained-run','verdict':'accepted' if status==0 else 'rejected',
            'reasons':[] if status==0 else ['fixture_rejection'],'manifest_sha256':'e'*64})
        return status
    return run


class ContractControls(unittest.TestCase):
    """No omitted/partial/self-reported contract acquires admission authority."""
    def test_complete_pins_and_closed_production_projection(self):
        """The four independent pins join a canonical source-lock projection."""
        PINS.validate();contracts.validate(contracts.LINUX,EXPECTED)
        record={'version':1,'kind':'independently_pinned_native_observer',
                'cargo_lock_sha256':'d'*64,**EXPECTED}
        contracts.validate_projection(record,EXPECTED)
        for key in record:
            bad=copy.deepcopy(record);del bad[key]
            with self.subTest(key=key),self.assertRaises(ValueError):contracts.validate_projection(bad,EXPECTED)

    def test_every_independent_pin_must_match(self):
        """Changing both adjacent artifact claims cannot change the caller authority."""
        record={'version':1,'kind':'independently_pinned_native_observer',
                'cargo_lock_sha256':'d'*64,**EXPECTED}
        for key in EXPECTED:
            bad=copy.deepcopy(record)
            bad[key]='aarch64-unknown-linux-gnu' if key=='native_target' else 'e'*64
            with self.subTest(key=key),self.assertRaises(ValueError):contracts.validate_projection(bad,EXPECTED)

    def test_diagnostic_build_receipt_is_not_production_delivery(self):
        """Historical diagnostic evidence cannot be promoted by relabelling an input."""
        old={'version':1,'kind':'diagnostic_cargo_artifact','module_sha256':'a'*64,'build_receipt_sha256':'b'*64}
        with self.assertRaises(ValueError):contracts.validate_projection(old,EXPECTED)

    def test_no_missing_contract_or_linux_pin_fallback(self):
        """No local platform, omitted bundle or missing pin selects a weaker mode."""
        for contract,pins in ((None,None),('',None),(contracts.LINUX,None),(contracts.ORDINARY,EXPECTED)):
            with self.subTest(contract=contract),self.assertRaises(ValueError):contracts.validate(contract,pins)
        for key in EXPECTED:
            bad=dict(EXPECTED);del bad[key]
            with self.assertRaises(ValueError):contracts.validate(contracts.LINUX,bad)
        contracts.validate(contracts.ORDINARY,None)

    def test_missing_each_run_pin_refuses_before_owner_or_popen(self):
        """The public Linux CLI cannot start a command with partial authority."""
        for name in ('observer_source_root','observer_bridge_root','trusted_observer_sha256','trusted_observer_provenance_sha256',
                     'trusted_source_manifest_sha256','observer_native_target'):
            args=arguments();setattr(args,name,None)
            with self.subTest(name=name),patch.object(harness.sys,'platform','linux'), \
                 patch.object(delivery,'ObserverDelivery') as acquire,patch.object(harness.subprocess,'Popen') as pop:
                with self.assertRaises(harness.HarnessRefusal):harness.public_run(args)
                acquire.assert_not_called();pop.assert_not_called()

    def test_direct_linux_runner_requires_owner_before_any_work(self):
        """Internal callers cannot accidentally select the old ordinary Linux branch."""
        with patch.object(harness.sys,'platform','linux'),patch.object(harness,'load_descriptor') as load:
            with self.assertRaises(harness.HarnessRefusal):harness.run_command(SimpleNamespace())
            load.assert_not_called()

    def test_public_linux_uses_same_retained_delivery_and_external_pins(self):
        """The public entry passes actual admitted factory and independent values together."""
        owner=Mock();owner.factory=object()
        context=Mock();context.__enter__=Mock(return_value=owner);context.__exit__=Mock(return_value=False)
        with patch.object(harness.sys,'platform','linux'),patch.object(delivery,'ObserverDelivery',return_value=context), \
             patch.object(harness,'run_command',side_effect=public_result(1)) as run:
            args=arguments();self.assertEqual(harness.public_run(args),1)
        run.assert_called_once_with(args,linux_observer_factory=owner.factory,expected_delivery=EXPECTED,summary_sink=ANY)
        context.__exit__.assert_called_once()

    def test_observer_source_root_is_independent_of_measurement_repository(self):
        """The artifact owner receives canonical observer source, never workload context."""
        owner=Mock();owner.factory=object()
        context=Mock();context.__enter__=Mock(return_value=owner);context.__exit__=Mock(return_value=False)
        with patch.object(harness.sys,'platform','linux'), \
             patch.object(delivery,'ObserverDelivery',return_value=context) as acquire, \
             patch.object(harness,'run_command',side_effect=public_result(0)) as run:
            args=arguments();self.assertEqual(harness.public_run(args),0)
        self.assertEqual(acquire.call_args.args,(Path('/canonical-observer-source'),Path('/artifact'),PINS))
        self.assertEqual(run.call_args.args[0].repository,'/measurement')
        self.assertEqual(run.call_args.kwargs['expected_delivery'],EXPECTED)

    def test_measurement_repository_never_supplies_missing_observer_source(self):
        """Even an existing workload repository cannot fill an omitted source authority."""
        args=arguments();del args.observer_source_root
        with patch.object(harness.sys,'platform','linux'), \
             patch.object(delivery,'ObserverDelivery') as acquire, \
             patch.object(harness.subprocess,'Popen') as pop:
            with self.assertRaises(harness.HarnessRefusal):harness.public_run(args)
        acquire.assert_not_called();pop.assert_not_called()

    def test_public_parser_retains_both_explicit_source_roles(self):
        """A direct CLI invocation carries source roles and all pins without inference."""
        args=harness.parser().parse_args(['run','--repository','/measurement',
            '--hardware','reference/test','--profile','test','--config','defaults','--cache-policy','warm',
            '--output-root','/output','--observer-source-root','/canonical-observer-source',
            '--observer-bridge-root','/artifact','--trusted-observer-sha256','a'*64,
            '--trusted-observer-provenance-sha256','b'*64,'--trusted-source-manifest-sha256','c'*64,
            '--observer-native-target','x86_64-unknown-linux-gnu','--','/workload'])
        self.assertEqual(args.repository,'/measurement')
        self.assertEqual(delivery.pins_from_arguments(args),
            (Path('/canonical-observer-source'),Path('/artifact'),PINS))
        self.assertIs(args.handler,harness.public_run)

    def test_nonlinux_preserves_ordinary_path_and_refuses_linux_flags(self):
        """Windows/Darwin do not gain an unsupported Linux loader route."""
        for platform in ('win32','darwin'):
            args=arguments()
            with patch.object(harness.sys,'platform',platform),patch.object(harness,'run_command',return_value=0) as run:
                with self.assertRaises(harness.HarnessRefusal):harness.public_run(args)
                run.assert_not_called()
                for key in vars(args):
                    if key!='repository':setattr(args,key,None)
                self.assertEqual(harness.public_run(args),0);run.assert_called_once_with(args)

    def test_reader_contract_required_before_any_evidence_read(self):
        """Even a supplied manifest pin cannot stand in for observer requirements."""
        with patch.object(harness,'_EvidenceDirectory') as opened:
            result=harness._inspect_pinned_run(Path('/run'),{},'e'*64,
                expected_observation_contract=None)
            self.assertEqual(result['reasons'],['independent_observation_requirement_invalid'])
            opened.assert_not_called()
        with self.assertRaises(TypeError):harness.read_pinned_run(Path('/run'),'e'*64)

    def test_registry_owner_pins_match_copied_bytes(self):
        """The explicit owner inventory has no ambient or computed trust fallback."""
        for name,want in delivery.REVIEWED_MODULES.items():
            self.assertEqual(hashlib.sha256((Path(delivery.__file__).parent/name).read_bytes()).hexdigest(),want)


class LifetimeControls(unittest.TestCase):
    """Only genuine observer close permits the retained owner stack to exit."""
    def setUp(self):
        """Construct no real source/module/child; only scoped modelled owners."""
        self.originals=list(delivery.abi._RETAINED_MODULES)
        self.addCleanup(lambda:delivery.abi._RETAINED_MODULES.__setitem__(slice(None),self.originals))
        delivery.abi._RETAINED_MODULES.clear()
        self.events=[]
        self.owner=delivery.ObserverDelivery('/repository','/artifact',PINS,cleanup_original=self.cleanup)
        self.owner.validate=Mock()
        self.owner.stack.callback(lambda:self.events.append('stack_closed'))
        self.addCleanup(self.owner.stack.close)

    def cleanup(self,active):
        """Model the existing callback reaching original reap before observer closure."""
        self.events.append('original_reaped');active.closed=True

    def native(self,outcomes):
        """Model native returned statuses while retaining every pending resource."""
        native=SimpleNamespace(image=self.owner,closed=False)
        iterator=iter(outcomes)
        def close():
            self.assertNotIn('stack_closed',self.events)
            self.events.append('native_close')
            item=next(iterator)
            if isinstance(item,BaseException):raise item
            if item==delivery.abi.OK:
                native.closed=True;delivery.abi._RETAINED_MODULES.remove(native)
            return item
        native.close=close;delivery.abi._RETAINED_MODULES.append(native)
        return native

    def active(self,native,first=None):
        """Install the original lifetime before any modelled post-acquisition operation."""
        value=SimpleNamespace(module_owner=self.owner,native=native,first=first,closed=False)
        def retain(stage,error):
            if value.first is None:value.first=error
        value.retain=retain;self.owner.active=value
        return value

    def test_original_cleanup_precedes_native_close_and_stack_release(self):
        """The source/artifact stack outlives both independent original boundaries."""
        self.active(self.native([delivery.abi.OK]))
        self.assertFalse(self.owner.__exit__(None,None,None))
        self.assertEqual(self.events,['original_reaped','native_close','stack_closed'])

    def test_pending_unknown_close_retains_all_then_raises_first(self):
        """A finite pending/unknown sequence cannot become success or premature exit."""
        first=OSError('original');self.active(self.native([delivery.abi.PENDING,delivery.abi.UNKNOWN,delivery.abi.OK]),first)
        with patch.object(delivery.time,'sleep'),self.assertRaises(OSError) as got:
            self.owner.__exit__(None,None,None)
        self.assertIs(got.exception,first)
        self.assertEqual(self.events[-1],'stack_closed');self.assertEqual(self.events.count('native_close'),3)

    def test_unassigned_registered_native_is_closed(self):
        """Interrupted factory assignment cannot orphan a registered native token."""
        self.native([delivery.abi.OK])
        self.owner.__exit__(None,None,None)
        self.assertEqual(self.events,['native_close','stack_closed'])

    def test_one_shot_outer_cleanup_error_retries_same_original(self):
        """The delivery context encloses failures outside inner run cleanup."""
        self.active(self.native([delivery.abi.OK]));first=KeyboardInterrupt();calls=0
        def cleanup(active):
            nonlocal calls
            calls+=1
            if calls==1:raise first
            self.cleanup(active)
        self.owner.cleanup_original=cleanup
        with patch.object(delivery.time,'sleep'),self.assertRaises(KeyboardInterrupt) as got:
            self.owner.__exit__(None,None,None)
        self.assertIs(got.exception,first);self.assertEqual(calls,2)
        self.assertEqual(self.events[-1],'stack_closed')

    def test_stack_close_failure_preserves_first_object(self):
        """Later source cleanup cannot replace an initiating ordinary failure."""
        first=OSError('first');later=RuntimeError('later')
        self.active(self.native([delivery.abi.OK]),first)
        self.owner.stack.callback(lambda:(_ for _ in ()).throw(later))
        with self.assertRaises(OSError) as got:self.owner.__exit__(None,None,None)
        self.assertIs(got.exception,first);self.assertIn('stack_closed',self.events)

    def test_authenticated_close_before_registry_removal_is_not_reclosed(self):
        """An interruption after native closure cannot cause reuse of a closed token."""
        native=self.native([]);native.closed=True
        self.owner.__exit__(None,None,None)
        self.assertEqual(self.events,['stack_closed'])

    def test_pre_reap_close_refusal_returns_to_original_cleanup_first(self):
        """An early close cannot park native closure before original cleanup/reap."""
        native=self.native([delivery.abi.OK])
        active=delivery.DeliveryLifetime.__new__(delivery.DeliveryLifetime)
        active.module_owner=self.owner;active.native=native
        active.closed=False;active.first=None;active.child=object();active.waited=None
        active.retain=lambda stage,error:setattr(active,'first',active.first or error)
        self.owner.active=active
        first=RuntimeError('original is not reaped');attempts=0
        def base_close(value):
            self.assertIs(value,active)
            if value.waited is None:raise first
            value.closed=True
        def cleanup(value):
            nonlocal attempts
            attempts+=1
            if attempts>1:
                self.events.append('original_reaped');value.waited=(71,0)
            value.close()
        self.owner.cleanup_original=cleanup
        with patch.object(delivery.lifetime.OriginalLifetime,'close',base_close), \
             patch.object(delivery.time,'sleep'),self.assertRaises(RuntimeError) as caught:
            self.owner.__exit__(None,None,None)
        self.assertIs(caught.exception,first);self.assertEqual(attempts,2)
        self.assertEqual(self.events,['original_reaped','native_close','stack_closed'])
        self.assertTrue(active.native.closed)

    def test_post_base_close_interruption_keeps_stack_through_native_retry(self):
        """A one-shot boundary unwind after reap never releases the module/source."""
        native=self.native([delivery.abi.OK])
        active=delivery.DeliveryLifetime.__new__(delivery.DeliveryLifetime)
        active.module_owner=self.owner;active.native=native
        active.closed=False;active.first=None;active.child=object();active.waited=None
        active.retain=lambda stage,error:setattr(active,'first',active.first or error)
        self.owner.active=active
        first=KeyboardInterrupt();attempts=0
        actual=self.owner.wait_closed
        def wait_closed(value):
            nonlocal attempts
            attempts+=1
            if attempts==1:raise first
            return actual(value)
        def base_close(value):
            self.assertIsNotNone(value.waited);value.closed=True
        def cleanup(value):
            self.events.append('original_reaped');value.waited=(71,0);value.close()
        self.owner.cleanup_original=cleanup
        with patch.object(delivery.lifetime.OriginalLifetime,'close',base_close), \
             patch.object(self.owner,'wait_closed',side_effect=wait_closed), \
             patch.object(delivery.time,'sleep'),self.assertRaises(KeyboardInterrupt) as caught:
            self.owner.__exit__(None,None,None)
        self.assertIs(caught.exception,first);self.assertEqual(attempts,3)
        # The final outer guard revisits already-closed custody without another
        # native close or reap; the event sequence proves both remain single.
        self.assertEqual(self.events,['original_reaped','native_close','stack_closed'])
        self.assertTrue(active.native.closed)

    def test_one_shot_trace_at_outer_cleanup_entry_retains_live_original(self):
        """Interrupt the actual method entry after an earlier refusal, then finish custody."""
        first=OSError('original refusal');later=KeyboardInterrupt()
        self.active(self.native([delivery.abi.OK]),first)
        self.owner.retain(first)
        triggered=[]
        code=delivery.ObserverDelivery._complete.__code__
        def trace(frame,event,arg):
            if frame.f_code is code and event=='line' and not triggered:
                self.assertFalse(self.owner.active.closed)
                self.assertNotIn('stack_closed',self.events)
                triggered.append(True)
                raise later
            return trace
        previous=sys.gettrace()
        try:
            sys.settrace(trace)
            with self.assertRaises(OSError) as caught:self.owner.__exit__(None,None,None)
        finally:sys.settrace(previous)
        self.assertEqual(triggered,[True]);self.assertIs(caught.exception,first)
        self.assertEqual(self.events,['original_reaped','native_close','stack_closed'])
        self.assertTrue(self.owner.active.native.closed)


if __name__=='__main__':unittest.main()
