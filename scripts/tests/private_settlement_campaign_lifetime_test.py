"""Physical-lifetime controls using only owned Python children, never Iroha."""
from __future__ import annotations

import contextlib
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace
import unittest
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1]
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))
import private_settlement_campaign_lifetime as lifetime
import private_settlement_campaign_execution as execution
import private_settlement_release_runner as runner
from private_settlement_session_execution import AdmittedSessionExecution


OWNED_CHILDREN = []

def child(directory, label, delay=0.15):
    marker = directory / label
    process = subprocess.Popen([sys.executable, '-I', '-B', '-c',
        'import time,pathlib;time.sleep('+str(delay)+');pathlib.Path('+repr(str(marker))+').write_text("done")'],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        start_new_session=True)
    OWNED_CHILDREN.append(process)
    return process, marker


class CampaignLifetimeTests(unittest.TestCase):
    def tearDown(self):
        # A rejected candidate must not orphan these test fixtures either.
        while OWNED_CHILDREN:
            lifetime.wait_owned_process(OWNED_CHILDREN.pop(), group=True)

    def test_empty_lifetime_restores_signal_handlers_and_starts_no_process(self):
        owner = execution.CampaignExecution()
        before = {sig:signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
        with mock.patch.object(subprocess, 'Popen') as spawn, owner.lifetime:
            owner.lifetime.require_launch()
        spawn.assert_not_called()
        self.assertIsNone(lifetime.wait_owned_process(None, group=True))
        self.assertEqual({sig:signal.getsignal(sig) for sig in before}, before)

    def test_nonmain_thread_refuses_before_work(self):
        errors = []
        def run():
            try:
                with execution.CampaignExecution().lifetime:
                    self.fail('non-main execution entered')
            except RuntimeError as error:
                errors.append(str(error))
        thread = threading.Thread(target=run)
        thread.start();thread.join()
        self.assertEqual(errors, ['canonical campaign lifetime requires the main Python thread'])

    def test_failure_preserves_original_cause_until_worker_physical_exit(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve();session = AdmittedSessionExecution()
            process, marker = child(root, 'worker')
            session.worker = SimpleNamespace(process=process,physical_exit=None)
            failure = OSError('synthetic publication failure')
            def fail(*args, owner, **kwargs):
                owner.session_owners['test-session'] = session
                raise failure
            with mock.patch.object(runner, '_execute_retained_plan', side_effect=fail):
                with self.assertRaises(execution.CampaignExecutionIncomplete) as caught:
                    execution.execute_plan()
            self.assertIs(caught.exception.owner.error, failure)
            self.assertIs(caught.exception.__cause__, failure)
            self.assertTrue(marker.is_file());self.assertEqual(process.returncode, 0)
            self.assertEqual(caught.exception.owner.lifetime.observations['test-session']['worker']['exit_code'], 0)
            self.assertIsNone(session.closure)

    def test_adapter_finishes_before_late_verifier_inventory_is_drained(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve();session = AdmittedSessionExecution()
            session.semantic_owner = SimpleNamespace(native_invocations={})
            started = threading.Event();allow = threading.Event();held = []
            def adapter():
                started.set();allow.wait()
                process, marker = child(root, 'late-verifier')
                held.append((process, marker))
                session.semantic_owner.native_invocations['late'] = SimpleNamespace(process=process,physical_exit=None)
            thread = threading.Thread(target=adapter)
            session.runtime = SimpleNamespace(thread=thread, owner=SimpleNamespace(packet_owners={},resource_owners={}))
            thread.start();self.assertTrue(started.wait(1));allow.set()
            record = lifetime.drain_session(session)
            self.assertFalse(thread.is_alive());self.assertTrue(held[0][1].is_file())
            self.assertEqual(record['verifiers'][0]['exit_code'], 0)
            self.assertIsNone(session.closure)

    def test_capture_receipt_failure_still_drains_capture_and_reader(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve();session = AdmittedSessionExecution()
            process, marker = child(root, 'capture')
            packet = SimpleNamespace(child=process, thread=None, closed=False)
            reasons = []
            def abort(reason):
                reasons.append(reason);packet.closed=True
                raise OSError('synthetic capture record failure')
            packet.abort = abort
            session.runtime = SimpleNamespace(thread=None, owner=SimpleNamespace(packet_owners={'one':packet},resource_owners={}))
            record = lifetime.drain_session(session)
            self.assertEqual(reasons, ['campaign_owner_draining'])
            self.assertEqual(record['cleanup_errors'], ['OSError'])
            self.assertTrue(marker.is_file());self.assertEqual(process.returncode, 0)
            self.assertIsNone(session.closure)

    def test_timeout_keeps_original_failure_after_late_natural_exit(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve();harness=root/'harness';marker=root/'late'
            harness.write_text('#!'+sys.executable+'\nimport time,pathlib\ntime.sleep(1.15)\npathlib.Path('+repr(str(marker))+').write_text("done")\n')
            harness.chmod(0o700)
            with self.assertRaisesRegex(runner.RunnerError, 'exceeded its 1-second deadline'):
                runner.invoke_harness(harness, {'kind':'fault'}, attempt_dir=root/'attempt', timeout_seconds=1)
            outcome = json.loads((root/'attempt/process-outcome.json').read_text())
            self.assertTrue(marker.is_file());self.assertEqual(outcome['exit_code'], 0)
            self.assertEqual(outcome['completion_kind'], 'outer_deadline')
            self.assertTrue(outcome['timed_out']);self.assertFalse(outcome['passed'])
            self.assertTrue(outcome['owned_process_group_gone'])
            self.assertFalse((root/'attempt/response-outcome.json').exists())

    def test_injected_interruption_reaps_one_shot_before_propagating(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve();process,marker=child(root,'one-shot')
            actual_wait=process.wait;first=True
            def interrupted_wait(*args, **kwargs):
                nonlocal first
                if first:
                    first=False;raise KeyboardInterrupt('synthetic interruption')
                return actual_wait(*args, **kwargs)
            with mock.patch.object(runner.subprocess,'Popen',return_value=process), mock.patch.object(process,'wait',side_effect=interrupted_wait):
                with self.assertRaises(KeyboardInterrupt):
                    runner.invoke_harness(root/'unexecuted-harness',{'kind':'fault'},attempt_dir=root/'attempt',timeout_seconds=1)
            outcome=json.loads((root/'attempt/process-outcome.json').read_text())
            self.assertTrue(marker.is_file());self.assertEqual(process.returncode,0)
            self.assertEqual(outcome['completion_kind'],'interrupted');self.assertFalse(outcome['passed'])

    def test_cli_catches_retained_failure_only_after_actual_child_exit(self):
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();session=AdmittedSessionExecution()
            process,marker=child(root,'cli-worker');session.worker=SimpleNamespace(process=process,physical_exit=None)
            def fail(*args,owner,**kwargs):
                owner.session_owners['one']=session
                owner.output_root=root
                raise ValueError('secret fixture body must not be printed')
            args=SimpleNamespace(command='execute',plan=root/'plan',output_dir=root/'output',source_root=root,
                harness=root/'harness',smoke_campaign=root/'smoke',scope=root/'scope',campaign_id='test',worker=root/'worker',validator=root/'validator')
            output=io.StringIO()
            with mock.patch.object(runner,'parse_args',return_value=args), mock.patch.object(runner,'_execute_retained_plan',side_effect=fail),contextlib.redirect_stderr(output):
                self.assertEqual(runner.main([]),2)
            self.assertTrue(marker.is_file());self.assertEqual(process.returncode,0)
            self.assertEqual(output.getvalue(),'private-settlement campaign incomplete: ValueError\n')
            physical=json.loads((root/'physical-owner-drain.json').read_text())
            self.assertIs(physical['qualification'],False)
            self.assertEqual(physical['original_error_type'],'ValueError')
            self.assertEqual(physical['sessions']['one']['worker']['exit_code'],0)

    def test_drain_record_failure_cannot_release_live_child_or_replace_original(self):
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();session=AdmittedSessionExecution()
            process,marker=child(root,'record-failure');session.worker=SimpleNamespace(process=process,physical_exit=None)
            original=ValueError('original failure')
            def fail(*args,owner,**kwargs):
                owner.output_root=root;owner.session_owners['one']=session
                raise original
            with mock.patch.object(runner,'_execute_retained_plan',side_effect=fail), mock.patch.object(runner,'private_record',side_effect=OSError('publication failed')):
                with self.assertRaises(execution.CampaignExecutionIncomplete) as caught:
                    execution.execute_plan()
            self.assertIs(caught.exception.__cause__,original)
            self.assertEqual(caught.exception.owner.physical_record_error,'OSError')
            self.assertTrue(marker.is_file());self.assertEqual(process.returncode,0)

    def test_actual_worker_start_publication_failure_retains_spawned_owner(self):
        from private_settlement_session_adapter import NativeWorker
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();(root/'session').mkdir();marker=root/'startup-child'
            image=SimpleNamespace(path=Path(sys.executable).resolve(),sha256='a'*64,validate=lambda:None)
            records=SimpleNamespace(path=root,publish=mock.Mock(side_effect=OSError('start publication failed')))
            session=AdmittedSessionExecution()
            def spawn(**kwargs):
                NativeWorker([str(image.path),'-I','-B','-c',
                    'import time,pathlib;time.sleep(.15);pathlib.Path('+repr(str(marker))+').write_text("done")'],
                    dict(os.environ),cwd=root,image=image,records=records,prefix='session',
                    deadline=lambda:time.monotonic_ns()+1_000_000_000)
            def fail(*args,owner,**kwargs):
                owner.session_owners['startup']=session
                with mock.patch.object(session,'_run',side_effect=spawn):
                    session.run(frozen_plan=root/'plan',registered_scope=root/'scope',campaign_id='test',
                        descriptor={},plan_harness={},worker_image={},validator_image={},cwd=root,runtime_root=root)
            with mock.patch.object(runner,'_execute_retained_plan',side_effect=fail):
                with self.assertRaises(execution.CampaignExecutionIncomplete) as caught:
                    execution.execute_plan()
            self.assertTrue(marker.is_file());self.assertEqual(session.worker.process.returncode,0)
            self.assertIs(caught.exception.owner.session_owners['startup'].worker,session.worker)
            self.assertIsNone(session.closure)

    def test_closed_session_and_verified_native_owner_never_reprobe_reused_pids(self):
        session=AdmittedSessionExecution();session.closed=True;session.closure={'authenticated':True}
        native=SimpleNamespace(process=SimpleNamespace(pid=123),reaped=True,physical_exit={'prior':True})
        session.worker=native;session.semantic_owner=SimpleNamespace(native_invocations={'prior':native})
        owner=execution.CampaignExecution();owner.session_owners['old']=session
        with mock.patch.object(lifetime,'wait_owned_process',side_effect=AssertionError('historical PID reprobed')),owner.lifetime:
            self.assertEqual(lifetime.wait_native_owner(native),{'prior_physical_exit_retained':True})
        self.assertEqual(owner.lifetime.observations['old'],{'canonical_session_closed':True,'additional_pid_probes':0})

    def test_cleanup_exception_keeps_handlers_and_drains_other_owner_before_retry(self):
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();owner=execution.CampaignExecution();sessions=[];markers=[]
            for name in ('first','second'):
                session=AdmittedSessionExecution();process,marker=child(root,name)
                session.worker=SimpleNamespace(process=process,physical_exit=None)
                sessions.append(session);markers.append(marker);owner.session_owners[name]=session
            actual=lifetime.drain_session;seen=[]
            def injected(session):
                self.assertEqual(signal.getsignal(signal.SIGTERM),owner.lifetime.handle_signal)
                seen.append(session)
                if len(seen)==1:raise OSError('one cleanup failed')
                return actual(session)
            with mock.patch.object(lifetime,'drain_session',side_effect=injected),owner.lifetime:
                pass
            self.assertEqual(seen,[sessions[0],sessions[1],sessions[0]])
            self.assertTrue(all(marker.is_file() for marker in markers))
            self.assertEqual(owner.lifetime.cleanup_errors,{'first':'OSError'})

    def test_retained_resource_sampler_is_stopped_and_joined_without_metrics(self):
        session=AdmittedSessionExecution();stop=threading.Event();done=threading.Event()
        thread=threading.Thread(target=lambda:(stop.wait(),done.set()))
        resource=SimpleNamespace(_stop=stop,_thread=thread)
        session.runtime=SimpleNamespace(thread=None,owner=SimpleNamespace(packet_owners={},resource_owners={'one':resource}))
        thread.start();lifetime.drain_session(session)
        self.assertTrue(done.is_set());self.assertFalse(thread.is_alive());self.assertIsNone(session.closure)

    def test_native_worker_finish_retains_canonical_physical_observation(self):
        import private_settlement_session_adapter as adapter
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();(root/'session').mkdir()
            image=SimpleNamespace(path=Path(sys.executable).resolve(),sha256='a'*64,validate=lambda:None)
            records=SimpleNamespace(path=root,publish=lambda *args:{'path':'synthetic-start'})
            worker=adapter.NativeWorker([str(image.path),'-I','-B','-c','pass'],dict(os.environ),
                cwd=root,image=image,records=records,prefix='session',deadline=lambda:time.monotonic_ns()+1_000_000_000)
            OWNED_CHILDREN.append(worker.process)
            with mock.patch.object(adapter,'native_group_snapshot',return_value={'members':[]}),mock.patch.object(adapter,'native_absence',return_value={'absent':True}):
                observation=worker.finish([],None)
            self.assertIs(worker.physical_exit,observation);self.assertTrue(worker.reaped)
            with mock.patch.object(lifetime,'wait_owned_process',side_effect=AssertionError('reprobe')):
                self.assertEqual(lifetime.wait_native_owner(worker),{'prior_physical_exit_retained':True})
            worker.close()

    def test_native_vector_terminal_publication_failure_retains_physical_fact(self):
        import private_settlement_session_adapter as adapter
        import private_settlement_session_economics as economics
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();process,marker=child(root,'vector')
            code=process.wait()
            invocation=economics.NativeVectorInvocation.__new__(economics.NativeVectorInvocation)
            invocation.process=process;invocation.reaped=True;invocation.physical_exit=None
            invocation.launch={};invocation.prefix='vector';invocation.exit_observation={'exit_code':code}
            invocation.observe_exit=lambda:{'path':'synthetic-exit'}
            invocation.stdout=invocation.stderr=SimpleNamespace(completed_bytes=lambda:b'',close=lambda:None)
            def publish(path,raw):
                if path.endswith('-terminal.json'):raise OSError('terminal publication failed')
                return {'path':path}
            invocation.records=SimpleNamespace(publish=publish)
            with mock.patch.object(adapter,'native_group_snapshot',return_value={'members':[]}),mock.patch.object(adapter,'native_absence',return_value={'absent':True}):
                with self.assertRaisesRegex(OSError,'terminal publication failed'):
                    invocation.finish(process_reader=None)
            self.assertTrue(marker.is_file());self.assertEqual(invocation.physical_exit['exit_code'],0)
            with mock.patch.object(lifetime,'wait_owned_process',side_effect=AssertionError('reprobe')):
                self.assertEqual(lifetime.wait_native_owner(invocation),{'prior_physical_exit_retained':True})

    def test_adapter_abort_retires_only_stopped_resource_sampler(self):
        from private_settlement_session_adapter import SessionAdapter
        for alive in (False,True):
            with self.subTest(alive=alive):
                adapter=SessionAdapter.__new__(SessionAdapter)
                resources=SimpleNamespace(_thread=SimpleNamespace(is_alive=lambda:alive),finish=lambda:{'failed':True})
                adapter.active={'attempt_id':'test','output_directory':'test'}
                adapter.resource_owners={'test':resources};adapter.session={}
                adapter._attempt_fields=lambda:{};adapter._publish=lambda *args:{'path':'synthetic'}
                adapter.measurement={'resources':resources,'packet':None,'ready_marker':{}}
                adapter._abort_measurement('synthetic failure')
                self.assertEqual('test' in adapter.resource_owners,alive)
                self.assertIsNone(adapter.measurement)

    def test_owner_signals_drain_current_child_and_refuse_successor(self):
        for signum in (signal.SIGINT,signal.SIGTERM):
            with self.subTest(signal=signum),tempfile.TemporaryDirectory() as raw:
                root=Path(raw).resolve();probe=root/'probe.py'
                probe.write_text("import sys\nsys.path[:0]="+repr(sys.path)+"\n"+PROBE)
                process=subprocess.Popen([sys.executable,'-I','-B',str(probe),str(root)],stdin=subprocess.DEVNULL,
                    stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
                deadline=time.monotonic()+5
                while not (root/'ready').exists() and process.poll() is None and time.monotonic()<deadline:
                    time.sleep(.01)
                self.assertTrue((root/'ready').exists())
                # This is our isolated Python fixture owner, never a native run.
                os.kill(process.pid,signum)
                stdout,stderr=process.communicate(timeout=5)
                self.assertEqual(process.returncode,2,(stdout,stderr))
                self.assertTrue((root/'physical-child-finished').exists())
                self.assertFalse((root/'forbidden-next-job').exists())
                self.assertIn(b'CampaignDrainRequested',stderr)



class PhysicalClosureInvariantTests(unittest.TestCase):
    def test_closed_flag_cannot_hide_live_sampler_capture_or_adapter(self):
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();session=AdmittedSessionExecution();session.closed=True
            sampler_stop=threading.Event();reader_done=threading.Event()
            sampler=threading.Thread(target=sampler_stop.wait);sampler.start()
            process,marker=child(root,'capture-complete',.15)
            reader=threading.Thread(target=lambda:(process.wait(),reader_done.set()));reader.start()
            resources=object.__new__(execution.sessions.processes.ProcessResourceWindow)
            resources._stop=sampler_stop;resources._thread=sampler
            packet=SimpleNamespace(closed=True,child=process,thread=reader)
            session.runtime=SimpleNamespace(thread=None,owner=SimpleNamespace(
                resource_owners={'active':resources},packet_owners={'active':packet}))
            try:
                with self.assertRaisesRegex(ValueError,'resource sampler'):
                    session.close()
                # No numeric native PID probe is needed for this capture child.
                with mock.patch.object(lifetime.os,'killpg',side_effect=AssertionError('native PID reprobed')):
                    result=lifetime.drain_session(session)
                self.assertTrue(marker.exists());self.assertTrue(reader_done.is_set())
                self.assertFalse(sampler.is_alive());self.assertFalse(reader.is_alive())
                self.assertEqual(result['cleanup_errors'],['SessionProtocolError'])
                self.assertEqual(result['captures'][0]['exit_code'],0)
                self.assertEqual(lifetime.drain_session(session),
                    {'canonical_session_closed':True,'additional_pid_probes':0})
                adapter_stop=threading.Event();adapter=threading.Thread(target=adapter_stop.wait)
                session.runtime.thread=adapter;adapter.start()
                try:
                    with self.assertRaisesRegex(ValueError,'adapter still running'):session.close()
                finally:adapter_stop.set();adapter.join()
            finally:
                sampler_stop.set();sampler.join();process.wait();reader.join()

    def test_live_resource_finish_is_failed_and_never_produces_metrics(self):
        observer=execution.sessions.processes;entered=threading.Event();release=threading.Event()
        class Scope:
            calls=0
            def observe(self):
                self.calls+=1
                if self.calls==2:entered.set();release.wait()
                return {'started_monotonic_ns':self.calls*10,'finished_monotonic_ns':self.calls*10+1,
                        'cpu_time_ns':self.calls*100,'rss_bytes':200,'processes':[]}
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();(root/'window').mkdir(mode=0o700)
            with execution.control.RecordDirectory(root) as records:
                scope=Scope();window=observer.ProcessResourceWindow(scope,records=records,prefix='window',
                    outer_timeout_ms=300000,deadline_monotonic_ns=time.monotonic_ns()+299_000_000_000,
                    interval_ms=10)
                try:
                    self.assertTrue(entered.wait(2))
                    # Simulate expiration of the existing join boundary; leave the
                    # actual sampler blocked and assert its unchanged timeout.
                    with mock.patch.object(window._thread,'join') as join:
                        result=window.finish()
                    join.assert_called_once_with(timeout=10)
                    self.assertFalse(result['sampler_stopped_observed'])
                    self.assertEqual(result['outcome'],{'kind':'failed','reason':'resource_sampler_did_not_stop'})
                    self.assertNotIn('cpu_time_ns',result['outcome'])
                    self.assertNotIn('sampled_peak_rss_bytes',result['outcome'])
                    with self.assertRaisesRegex(ValueError,'incomplete resource window'):
                        observer.validate_resource_window(result,records=records,outer_timeout_ms=300000,
                                                          expected_processes=[])
                finally:release.set();window._thread.join()

    def test_retried_cleanup_error_is_in_durable_physical_drain_record(self):
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve();session=AdmittedSessionExecution();session.closed=True
            def body(*args,owner,**kwargs):
                owner.output_root=root;owner.session_owners['synthetic-closed']=session
                raise ValueError('original failure')
            actual=lifetime.drain_session;calls=0
            def drain(value):
                nonlocal calls
                calls+=1
                if calls==1:raise OSError('cleanup publication failed')
                return actual(value)
            with mock.patch.object(runner,'_execute_retained_plan',side_effect=body), \
                    mock.patch.object(lifetime,'drain_session',side_effect=drain):
                with self.assertRaises(execution.CampaignExecutionIncomplete):execution.execute_plan()
            record=json.loads((root/'physical-owner-drain.json').read_text())
            self.assertEqual(record['cleanup_errors'],{'synthetic-closed':'OSError'})
            self.assertEqual(record['original_error_type'],'ValueError')
            self.assertFalse(record['qualification'])


class HistoricalProcessIdentityTests(unittest.TestCase):
    def one_shot(self,root):
        root=root/'campaign-a';root.mkdir(mode=0o700)
        harness=root/'harness';harness.write_text('#!'+sys.executable+'\n');harness.chmod(0o700)
        scope={'registered_ns':time.time_ns()};runner.private_record(root/'registered-scope.json',scope)
        job={'kind':'fault','request_id':'a'*64}
        plan={'jobs':[job],'harness':runner.verify_harness(harness),'benchmark_accounting':{'outer_timeout_ms':1000}}
        runner.private_record(root/'frozen-plan.json',plan)
        identity={'scope_sha256':runner.file_binding(root/'registered-scope.json')['sha256'],
            'campaign_id':root.name,'plan_sha256':runner.file_binding(root/'frozen-plan.json')['sha256']}
        request={**job,'invocation_nonce':'b'*64}
        identity['attempt_id']=runner.attempt_accounting.registered_attempt_id(**identity,request_id=job['request_id'])
        directory=root/'attempts'/('00001-'+job['request_id']);directory.parent.mkdir(mode=0o700)
        with mock.patch.object(runner,'_process_group_exists',side_effect=[False,True]) as presence:
            with self.assertRaises(runner.RunnerError):
                runner.invoke_harness(harness,request,attempt_dir=directory,timeout_seconds=1,
                    expected_harness_binding=plan['harness'],accounting_identity=identity)
            self.assertEqual(presence.call_count,1)
        process=json.loads((directory/'process-outcome.json').read_text())
        self.assertTrue(process['passed']);self.assertTrue(process['owned_process_group_gone'])
        return root,plan,directory,process

    def test_completed_one_shot_and_prefix_never_reprobe_reused_group(self):
        with tempfile.TemporaryDirectory() as raw:
            root,plan,directory,process=self.one_shot(Path(raw).resolve())
            owner=execution.CampaignExecution()
            with mock.patch.object(runner,'_process_group_exists',side_effect=AssertionError('reused PGID')):
                owner.validate_prefix_owners(root,plan,1)
                import private_settlement_campaign_closure as closure
                closure._nonbenchmark_groups({'nonbenchmark':[{'started':b'present',
                    'process':execution.control.canonical(process)}]})

    def test_prefix_retains_start_identity_group_absence_and_interval_checks(self):
        with tempfile.TemporaryDirectory() as raw:
            root,plan,directory,process=self.one_shot(Path(raw).resolve())
            owner=execution.CampaignExecution();path=directory/'process-outcome.json'
            for key,value in (('owned_process_group_gone',False),('request_id','c'*64),
                              ('attempt_id','d'*64),('finished_ns',1),('bindings_unchanged',False)):
                path.write_text(json.dumps({**process,key:value}))
                with self.subTest(key=key),mock.patch.object(runner,'_process_group_exists',
                        side_effect=AssertionError('historical group probed')):
                    with self.assertRaises(ValueError):owner.validate_prefix_owners(root,plan,1)
            path.write_text(json.dumps(process))
            start_path=directory/'started.json';start=json.loads(start_path.read_text())
            start_path.write_text(json.dumps({**start,'harness':{'sha256':'0'*64,'bytes':1}}))
            with self.assertRaisesRegex(ValueError,'request/image changed'):
                owner.validate_prefix_owners(root,plan,1)

    def test_final_nonbenchmark_closure_retains_absence_requirement(self):
        import private_settlement_campaign_closure as closure
        for pid in (None,123):
            good={'pid':pid,'owned_process_group_gone':True}
            with mock.patch.object(runner,'_process_group_exists',side_effect=AssertionError('historical group probed')):
                closure._nonbenchmark_groups({'nonbenchmark':[{'started':b'present',
                    'process':execution.control.canonical(good)}]})
            with self.assertRaisesRegex(ValueError,'unconfirmed nonbenchmark'):
                closure._nonbenchmark_groups({'nonbenchmark':[{'started':b'present',
                    'process':execution.control.canonical({**good,'owned_process_group_gone':False})}]})

PROBE = r'''from pathlib import Path
import subprocess,sys,time
from types import SimpleNamespace
from unittest import mock
import private_settlement_campaign_execution as execution
import private_settlement_release_runner as runner
from private_settlement_session_execution import AdmittedSessionExecution
root=Path(sys.argv[1]);session=AdmittedSessionExecution()
def run(*args,owner,**kwargs):
    process=subprocess.Popen([sys.executable,'-I','-B','-c',
        'import pathlib,time;time.sleep(.35);pathlib.Path('+repr(str(root/'physical-child-finished'))+').write_text("done")'],start_new_session=True)
    session.worker=SimpleNamespace(process=process,physical_exit=None);owner.session_owners['one']=session
    (root/'ready').write_text(str(process.pid))
    process.wait()
    owner.lifetime.require_launch()
    (root/'forbidden-next-job').write_text('bad')
args=SimpleNamespace(command='execute',plan=root/'plan',output_dir=root/'out',source_root=root,harness=root/'h',
    smoke_campaign=root/'s',scope=root/'scope',campaign_id='test',worker=root/'w',validator=root/'v')
with mock.patch.object(runner,'parse_args',return_value=args),mock.patch.object(runner,'_execute_retained_plan',side_effect=run):
    raise SystemExit(runner.main([]))
'''

if __name__ == '__main__':
    unittest.main()
