"""Actual canonical preparation and failure owners; no native Iroha/capture run."""
from __future__ import annotations


import hashlib
import importlib.util
import json
import io
import time
from types import SimpleNamespace
import os
from pathlib import Path
import sys
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import private_settlement_session_execution as execution

sys.path.insert(0, str(Path(__file__).resolve().parent))
import retained_scope_foundation as fixture
control=execution.control


class SessionExecutionTests(unittest.TestCase):
    def setUp(self):
        self.fixture=fixture.RegisteredScopeIntegrationTests();self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.materialize();self.root=self.fixture.root/'campaigns/campaign-0'
        self.runtime_root=self.fixture.root/'runtime';self.runtime_root.mkdir(mode=0o700)
        self.binary=Path('/usr/bin/false').resolve(strict=True)
        binding={'path':str(self.binary),'sha256':hashlib.sha256(self.binary.read_bytes()).hexdigest(),
                 'bytes':self.binary.stat().st_size}
        self.arguments=dict(frozen_plan=self.root/'frozen-plan.json',registered_scope=self.root/'registered-scope.json',
            campaign_id='campaign-0',descriptor=self.fixture.plans['campaign-0']['benchmark_sessions'][0],
            plan_harness=self.fixture.harness,worker_image=binding,validator_image=dict(binding),
            cwd=ROOT,runtime_root=self.runtime_root)
        self.owners=[];self.addCleanup(self.dispose_fixture_owners)

    def dispose_fixture_owners(self):
        # These exact children execute only the source-bound /usr/bin/false.
        # Explicit fixture disposal makes no session-quiescence qualification.
        for owner in self.owners:
            if owner.worker is not None:
                owner.worker.process.wait(timeout=5)
                owner.worker.close()
            if owner.runtime is not None:
                if owner.runtime.thread is not None:owner.runtime.thread.join(timeout=5)
                self.assertTrue(owner.runtime.thread is None or not owner.runtime.thread.is_alive())
                owner.runtime.close_inactive_descriptors()
            if owner.reader is not None:owner.reader.close()
            for image in owner.images.values():image.close()
            if owner.records is not None:owner.records.close()

    def incomplete(self,**changes):
        with self.assertRaises(execution.SessionExecutionIncomplete) as caught:
            execution.execute_admitted_session(**{**self.arguments,**changes})
        owner=caught.exception.owner;self.owners.append(owner)
        self.assertIsNotNone(owner.error);self.assertIsNone(owner.result)
        self.assertFalse((self.root/'campaign-closure.json').read_bytes()==b'')
        return owner

    def test_preparation_writes_exact_eight_requests_and_native_start_before_spawn(self):
        def stopped(prepared,started,**arguments):
            self.assertIsInstance(arguments['images']['worker'],execution.processes.ExecutableImage)
            value=control.decode(arguments['records'].read(started))
            self.assertEqual(value['request'],prepared['reference'])
            self.assertEqual(value['command'],[str(self.binary),execution.adapter.WORKER_TEST,
                '--exact','--ignored','--nocapture','--test-threads=1'])
            raise RuntimeError('explicit pre-spawn control')
        with mock.patch.object(execution.runtime,'spawn_bound_worker',side_effect=stopped) as spawn:
            owner=self.incomplete()
        self.assertEqual(spawn.call_count,1);self.assertIsNone(owner.worker)
        self.assertIsInstance(owner.semantic_owner,execution.semantics.SessionSemantics)
        self.assertIsInstance(owner.observations,execution.adapter.NativeObservations)
        self.assertIsInstance(owner.listener,execution.network.DarwinListenerReader)
        self.assertIsInstance(owner.sample_replay,execution.samples.RetainedSampleReplay)
        self.assertEqual(len(owner.prepared['request']['attempts']),8)
        for row in owner.prepared['request']['attempts']:
            request=control.decode(owner.records.read(row['request']))
            self.assertEqual(request['payload']['resources'],['proof_bytes','receipt_bytes','storage_growth_bytes'])
            self.assertFalse((self.root/row['output_directory']/'started.json').exists())
        owner.close();self.assertTrue(owner.closed)

    def test_wrong_plan_harness_refuses_before_preparation(self):
        with mock.patch.object(execution.runtime,'spawn_bound_worker') as spawn:
            owner=self.incomplete(plan_harness={'sha256':'9'*64,'bytes':111})
        spawn.assert_not_called();self.assertIsNone(owner.prepared)

    def test_changed_native_bytes_refuse_before_preparation(self):
        owner=self.incomplete(worker_image={**self.arguments['worker_image'],'sha256':'9'*64})
        self.assertIsNone(owner.prepared);self.assertIsNone(owner.worker)

    def test_unregistered_session_descriptor_refuses_before_durable_start(self):
        owner=self.incomplete(descriptor={**self.arguments['descriptor'],'seed':99})
        self.assertIsNone(owner.prepared);self.assertIsNone(owner.started)

    def test_wrong_campaign_registration_refuses_before_preparation(self):
        owner=self.incomplete(campaign_id='unregistered')
        self.assertIsNone(owner.prepared)

    def test_reused_runtime_root_refuses_before_preparation(self):
        (self.runtime_root/'old-output').write_bytes(b'prior session output')
        owner=self.incomplete();self.assertIsNone(owner.prepared)

    def test_foundation_changed_after_durable_start_refuses_before_spawn(self):
        publish=execution.runner.publish_benchmark_session_start
        def changed(*args,**kwargs):
            ref=publish(*args,**kwargs)
            p=self.root/'hardware.json';p.write_bytes(p.read_bytes()+b' ')
            return ref
        with mock.patch.object(execution.runner,'publish_benchmark_session_start',side_effect=changed):
            with mock.patch.object(execution.runtime,'spawn_bound_worker') as spawn:
                owner=self.incomplete()
        self.assertIsNotNone(owner.started);spawn.assert_not_called()

    def test_actual_false_child_exits_without_ready_and_retains_entire_owner(self):
        # The unchanged native entrypoint arguments are passed to /usr/bin/false.
        # Its natural exit cannot stand in for any Iroha worker success.
        with mock.patch.object(execution.packets.PacketCaptureOwner,'begin',side_effect=AssertionError('capture forbidden')) as capture:
            owner=self.incomplete()
        capture.assert_not_called()
        self.assertIsInstance(owner.worker,execution.adapter.NativeWorker)
        self.assertIsInstance(owner.runtime,execution.runtime.SessionRuntime)
        self.assertIsInstance(owner.callbacks,execution.runtime.RunnerSemanticCallbacks)
        self.assertIs(owner.runtime.semantic_owner,owner.semantic_owner)
        self.assertIs(owner.runtime.worker,owner.worker)
        self.assertIs(owner.runtime.observations,owner.observations)
        self.assertEqual(owner.worker.process.wait(timeout=5),1)
        owner.runtime.thread.join(timeout=5)
        self.assertFalse(owner.runtime.thread.is_alive());self.assertIsNone(owner.closure)
        self.assertEqual(owner.semantic_owner.native_invocations,{})
        self.assertFalse(any((self.root/'attempts').glob('*/started.json')))
        self.assertFalse(any((self.root/'sessions').glob('*/session-closure.json')))
        print(json.dumps({'fixture':'nonqualifying /usr/bin/false','pid':owner.worker.process.pid,
                          'natural_exit_code':1,'worker_sha256':self.arguments['worker_image']['sha256'],
                          'adapter_thread_joined':True,'session_closure':False}))
        with self.assertRaisesRegex(control.SessionProtocolError,'incomplete native owner'):
            owner.close()

    def test_postspawn_start_publication_failure_keeps_actual_child_owner(self):
        publish=control.RecordDirectory.publish
        def failed(records,path,raw):
            if path.endswith('/worker-process-start.json'):
                raise OSError('explicit publication failure after Popen')
            return publish(records,path,raw)
        with mock.patch.object(control.RecordDirectory,'publish',failed):
            owner=self.incomplete()
        self.assertIsInstance(owner.error,execution.adapter.NativeWorkerStartFailure)
        self.assertIs(owner.worker,owner.error.worker)
        self.assertEqual(owner.worker.process.wait(timeout=5),1)
        self.assertIsNone(owner.runtime);self.assertIsNotNone(owner.started)
        self.assertIsNone(owner.closure)
        self.assertFalse(hasattr(owner.worker,'started'))

    def packet_start_failure(self):
        import retained_packet_fixture as module
        f=module.PacketRecordFixture();f.setUp();self.addCleanup(f.tearDown)
        child=SimpleNamespace(pid=123456,stdout=io.BytesIO(),stderr=io.BytesIO())
        socket=mock.Mock();socket.getsockname.return_value=('127.0.0.1',49000)
        publish=f.records.publish
        def failed(path,raw):
            if path=='capture/capture-process.json':raise OSError('explicit post-Popen publication failure')
            return publish(path,raw)
        # No socket, subprocess, capture or signal executes in this control.
        with mock.patch.object(execution.packets.socket,'socket',return_value=socket):
            with mock.patch.object(execution.packets.subprocess,'Popen',return_value=child) as popen:
                with mock.patch.object(f.records,'publish',side_effect=failed):
                    with mock.patch.object(execution.packets.PacketCaptureOwner,'abort',side_effect=RuntimeError('explicit abort failure')) as abort:
                        with self.assertRaises(execution.packets.PacketCaptureIncomplete) as caught:
                            execution.packets.PacketCaptureOwner.begin(records=f.records,prefix='capture',
                                session=module.SESSION,attempt=module.ATTEMPT,ready_marker=f.ready,
                                ports_reference=f.ports,listener_before=f.before,resource_baseline=f.baseline,
                                deadline_ns=time.monotonic_ns()+5_000_000_000)
        self.assertEqual(popen.call_count,1);self.assertEqual(abort.call_count,1)
        error=caught.exception
        self.assertIs(error.owner.child,child);self.assertIs(error.owner.socket,socket)
        self.assertIsInstance(error.owner.start_error,OSError)
        self.assertIsInstance(error.owner.cleanup_error,RuntimeError)
        self.assertTrue(error.owner.cleanup_attempted);self.assertFalse(error.owner.closed)
        self.assertIsNone(error.owner.incomplete_reference)
        return error

    def test_packet_start_and_abort_failure_preserve_original_child_and_both_errors(self):
        self.packet_start_failure()

    def test_packet_exception_owner_survives_adapter_and_execution_without_cleanup_retry(self):
        failure=self.packet_start_failure()
        with mock.patch.object(execution.runtime,'spawn_bound_worker',side_effect=RuntimeError('pre-spawn fixture stop')):
            owner=self.incomplete()
        records=owner.records;prepared=owner.prepared;attempt=prepared['request']['attempts'][0]
        fields={key:attempt[key] for key in control.ATTEMPT_FIELDS}
        marker=records.publish(attempt['output_directory']+'/evidence/benchmark-protocol/measurement-ready.json',
            control.canonical({**prepared['identity'],**fields,'boundary':'ready'}))
        chain=control.ControlChain(prepared['identity'],owner.started['sha256'],'adapter_worker','child_to_owner',
            records,observer='worker',journal_prefix='sessions/'+prepared['identity']['session_id']+'/control')
        message=chain.send(io.BytesIO(),'measurement_ready',{**fields,'marker':marker})
        # Explicit zero-start resource fixture with the canonical physical-owner shape.
        resource = object.__new__(execution.processes.ProcessResourceWindow)
        resource.finish = mock.Mock(return_value={'kind':'explicit_incomplete_resource_fixture'})
        resource._thread = __import__('threading').Thread()
        resource._stop = __import__('threading').Event()
        def window(*,records,prefix,**kwargs):
            resource.baseline_reference=records.publish(prefix+'/baseline.json',b'{"fixture":true}')
            return resource
        observations=SimpleNamespace(observe_listeners=lambda:{'fixture':'listener'},window=window)
        fake_worker=SimpleNamespace(process=SimpleNamespace(pid=123457))
        adapter=execution.adapter.SessionAdapter(prepared,owner.started,records=records,
            runner_reader=io.BytesIO(),runner_writer=io.BytesIO(),worker=fake_worker,
            observations=observations,packets=execution.packets,semantics=owner.semantic_owner,
            outer_timeout_ms=owner.plan['benchmark_accounting']['outer_timeout_ms'])
        adapter.active=attempt;adapter.ready={'network_ports':marker}
        wrapper=execution.AdmittedSessionExecution();wrapper.runtime=SimpleNamespace(owner=adapter)
        with mock.patch.object(execution.packets.PacketCaptureOwner,'begin',side_effect=failure):
            with mock.patch.object(execution.packets.PacketCaptureOwner,'abort',side_effect=AssertionError('cleanup retry forbidden')) as abort:
                with self.assertRaises(execution.packets.PacketCaptureIncomplete) as caught:
                    adapter._begin(message,{'outer_timeout_ms':owner.plan['benchmark_accounting']['outer_timeout_ms']})
                self.assertIs(caught.exception,failure)
                self.assertIs(adapter.packet_start_error,failure)
                self.assertIs(wrapper.packet_owners[attempt['attempt_id']],failure.owner)
                self.assertIs(wrapper.packet_owners,adapter.packet_owners)
                self.assertIsNone(adapter.measurement['packet'])
                adapter._abort_measurement('adapter_interrupted')
                abort.assert_not_called()
        self.assertIs(wrapper.packet_owners[attempt['attempt_id']].child,failure.owner.child)
        self.assertEqual(resource.finish.call_count,1)

    def test_packet_early_deadline_failure_has_owner_without_child_or_cleanup(self):
        with self.assertRaises(execution.packets.PacketCaptureIncomplete) as caught:
            execution.packets.PacketCaptureOwner.begin(records=None,prefix='uncreated',
                session={
                    'version':1,'protocol':control.PROTOCOL,'scope_sha256':'1'*64,'campaign_id':'fixture',
                    'plan_sha256':'1'*64,'session_id':'1'*64,'session_invocation_nonce':'1'*64,
                    'session_request_sha256':'1'*64},
                attempt={'attempt_id':'2'*64,'request_id':'2'*64,'invocation_nonce':'2'*64,'session_attempt_index':0},
                ready_marker=None,ports_reference=None,listener_before=None,resource_baseline=None,deadline_ns=0)
        self.assertIsNone(caught.exception.owner.child)
        self.assertFalse(caught.exception.owner.cleanup_attempted)
        self.assertIsInstance(caught.exception.owner.start_error,execution.packets.PacketWindowError)

    def test_used_owner_cannot_retry_a_failed_registered_session(self):
        owner=self.incomplete(campaign_id='unregistered')
        with self.assertRaisesRegex(control.SessionProtocolError,'cannot be reused'):
            owner.run(**self.arguments)


if __name__=='__main__':unittest.main()
