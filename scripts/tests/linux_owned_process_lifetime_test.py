"""Pure original-lifetime ordering and real pipe-thread cleanup without a child."""
from __future__ import annotations
import hashlib
import os
from pathlib import Path
import sys
import threading
import types
import unittest
from unittest.mock import Mock,patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import linux_owned_process_lifetime as lifetime
import linux_root_observer_abi as abi
import zk_resource_harness as harness


class LifetimeControls(unittest.TestCase):
    """Retained error and finish-before-reap transitions stay independent of status."""

    def owner(self):
        native=types.SimpleNamespace(prepared=True,close=Mock(return_value=abi.OK),final_raw=b'fixed',
            read=Mock(return_value=(abi.OK,{'finalized':True,'lifecycle_certified':True},b'fixed')),
            join_wait=Mock(return_value={'lifecycle_and_initial_image_joined':True,'command_succeeded':False}))
        base=types.SimpleNamespace(close=Mock())
        with patch.object(lifetime.observer,'LinuxProcessReader',return_value=base), \
             patch.object(lifetime.abi,'NativeObservation',return_value=native):
            owner=lifetime.OriginalLifetime(Mock(),Mock(),17)
        owner.child=types.SimpleNamespace(pid=12345,returncode=None,stdout=None,stderr=None)
        owner.deadline_ns=10**30
        owner.output_drain_ns=5_000_000_000
        return owner

    def test_prepare_uses_callers_existing_overall_and_output_deadlines(self):
        owner=self.owner();owner.native.prepare=Mock(return_value=abi.OK)
        with patch.object(lifetime.time,'monotonic_ns',return_value=17):
            owner.prepare(123,harness.OUTPUT_DRAIN_SECONDS)
        self.assertEqual(owner.started_ns,17)
        self.assertEqual(owner.deadline_ns,123_000_017)
        self.assertEqual(owner.output_drain_ns,5_000_000_000)
        self.assertIsNone(owner.output_deadline_ns)
        owner.native.prepare.assert_called_once()

    def test_pending_binding_expires_existing_deadline_with_original_still_owned(self):
        owner=self.owner();owner.started_ns=0;owner.deadline_ns=1_000_000
        owner.native.bind_original=Mock(return_value=abi.PENDING)
        owner.native.read=Mock(return_value=(abi.OK,{'root_pid':0},b'pending'))
        original=owner.child
        with patch.object(lifetime.time,'monotonic_ns',side_effect=[0,0,1_000_000]), \
             patch.object(lifetime.time,'sleep'),patch.object(lifetime.os,'waitid',return_value=None), \
             patch.object(lifetime.os,'wait4') as reap:
            with self.assertRaisesRegex(abi.Refusal,'overall deadline') as caught:
                owner.install(original,1)
        owner.retain('setup',caught.exception)
        self.assertIs(owner.child,original);self.assertIs(owner.first,caught.exception)
        self.assertIsNone(owner.reader);reap.assert_not_called()
        owner.native.bind_original.assert_called_once_with(original)

    def test_binding_cannot_accept_report_that_crossed_original_deadline(self):
        owner=self.owner();owner.started_ns=0;owner.deadline_ns=1_000_000
        owner.native.bind_original=Mock(return_value=abi.OK)
        owner.native.read=Mock(return_value=(abi.OK,{'root_pid':12345,'first_error':None},b'bound'))
        with patch.object(lifetime.time,'monotonic_ns',side_effect=[0,1_000_000]), \
             patch.object(lifetime.owned,'OwnedLinuxProcessReader') as reader:
            with self.assertRaisesRegex(abi.Refusal,'overall deadline'):
                owner.install(owner.child,1)
        reader.assert_not_called();self.assertEqual(owner.deadline_ns,1_000_000)

    def test_changed_install_timeout_refuses_without_resetting_original_deadline(self):
        owner=self.owner();owner.started_ns=0;owner.deadline_ns=1_000_000
        owner.native.bind_original=Mock()
        with self.assertRaisesRegex(abi.Refusal,'deadline changed'):owner.install(owner.child,2)
        owner.native.bind_original.assert_not_called();self.assertEqual(owner.deadline_ns,1_000_000)

    def test_live_poll_never_calls_wait4(self):
        owner=self.owner();owner.poll=Mock()
        with patch.object(lifetime.os,'waitid',return_value=None),patch.object(lifetime.os,'wait4') as wait:
            self.assertIsNone(owner.try_reap())
        wait.assert_not_called();owner.poll.assert_called_once()

    def test_finish_is_before_wait4_and_exit101_is_preserved(self):
        owner=self.owner();events=[]
        def read(*,finish=False):
            events.append('finish' if finish else 'poll')
            return abi.OK,{'finalized':True,'lifecycle_certified':True},b'fixed'
        owner.native.read=read
        usage=types.SimpleNamespace(ru_utime=1.,ru_stime=2.,ru_maxrss=3)
        def wait(pid,flags):
            events.append('wait4');return pid,101<<8,usage
        terminal=types.SimpleNamespace(si_pid=12345,si_code=os.CLD_EXITED,si_status=101)
        with patch.object(lifetime.os,'waitid',return_value=terminal),patch.object(lifetime.os,'wait4',side_effect=wait):
            self.assertEqual(owner.try_reap(),(101<<8,usage))
        self.assertEqual(events,['finish','wait4','finish'])
        self.assertEqual(owner.child.returncode,101);self.assertTrue(owner.joined)

    def test_failed_finish_still_reaps_original_but_never_certifies(self):
        owner=self.owner();first=OSError(5,'first native failure');owner.native.read.side_effect=first
        owner.native.join_wait.side_effect=abi.Refusal('no certificate')
        terminal=types.SimpleNamespace(si_pid=12345,si_code=os.CLD_EXITED,si_status=0)
        with patch.object(lifetime.os,'waitid',return_value=terminal), \
             patch.object(lifetime.os,'wait4',return_value=(12345,0,object())) as wait:
            owner.try_reap()
        self.assertTrue(owner.finish_attempted);wait.assert_called_once_with(12345,0)
        self.assertIs(owner.first,first);self.assertFalse(owner.joined)

    def test_repeated_reap_uses_stored_original_status_without_pid_lookup(self):
        owner=self.owner();owner.waited=(12345,101<<8,object())
        with patch.object(lifetime.os,'waitid') as peek,patch.object(lifetime.os,'wait4') as wait:
            self.assertEqual(owner.try_reap(),owner.waited[1:])
        peek.assert_not_called();wait.assert_not_called()

    def test_persistent_poll_failure_cannot_bypass_deadline_cleanup(self):
        owner=self.owner();owner.deadline_ns=0;owner.drain_once=Mock()
        first=ValueError('ordinary failure');owner.try_reap=Mock(side_effect=first)
        for slot in owner.pipes:slot.reader='eof'
        def kill(pid):
            self.assertEqual(pid,12345);owner.waited=(pid,9,object())
        operation=Mock(side_effect=kill);owner.complete(operation)
        self.assertIs(owner.first,first);operation.assert_called_once_with(12345)

    def test_persistent_drain_failure_still_finishes_and_reaps_once_without_false_eof(self):
        owner=self.owner();first=OSError(5,'persistent pipe read')
        owner.drain_once=Mock(side_effect=first)
        thread=Mock(ident=17);owner.pipes[0].thread=thread
        terminal=types.SimpleNamespace(si_pid=12345,si_code=os.CLD_EXITED,si_status=0)
        with patch.object(lifetime.os,'waitid',return_value=terminal), \
             patch.object(lifetime.os,'wait4',return_value=(12345,0,object())) as wait:
            owner.complete(Mock());owner.complete(Mock())
        wait.assert_called_once_with(12345,0)
        self.assertTrue(owner.finish_attempted);self.assertIs(owner.first,first)
        self.assertTrue(all(slot.incomplete and slot.reader!='eof' for slot in owner.pipes))
        records=Mock()
        with self.assertRaisesRegex(lifetime.observer.ProcessObservationError,'lacks EOF'):
            owner.output(records,'digest_only')
        thread.join.assert_not_called();records.publish.assert_not_called()

    def test_persistent_drain_failure_does_not_prevent_existing_deadline_callback(self):
        owner=self.owner();owner.deadline_ns=0;first=OSError(5,'persistent pipe')
        owner.drain_once=Mock(side_effect=first)
        terminal=types.SimpleNamespace(si_pid=12345,si_code=os.CLD_KILLED,si_status=9)
        state={'killed':False};events=[]
        def peek(*args):return terminal if state['killed'] else None
        def kill(pid):
            self.assertIsNone(owner.waited);self.assertEqual(pid,12345)
            events.append('deadline');state['killed']=True
        def reap(pid,flags):events.append('wait4');return pid,9,object()
        with patch.object(lifetime.os,'waitid',side_effect=peek), \
             patch.object(lifetime.os,'wait4',side_effect=reap) as wait:
            owner.complete(kill)
        self.assertEqual(events,['deadline','wait4']);wait.assert_called_once()
        self.assertIs(owner.first,first);self.assertEqual(owner.child.returncode,-9)
        self.assertTrue(all(slot.incomplete for slot in owner.pipes))

    def test_reaped_root_with_real_open_writer_refuses_after_existing_drain_bound(self):
        owner=self.owner();read,write=os.pipe();pipe=os.fdopen(read,'rb',buffering=0)
        owner.child.stdout=pipe;owner.pipes[0].pipe=pipe
        owner.waited=(12345,0,object());owner.output_deadline_ns=5_000_000_000
        try:
            with patch.object(lifetime.time,'monotonic_ns',side_effect=[0,5_000_000_000]):
                owner.complete(Mock())
            self.assertTrue(owner.pipes[0].incomplete)
            self.assertNotEqual(owner.pipes[0].reader,'eof')
            self.assertIsInstance(owner.first,abi.Refusal)
            self.assertIn('output drain deadline',str(owner.first))
            self.assertFalse(pipe.closed)
            owner.close();self.assertTrue(pipe.closed)
            self.assertEqual(owner.pipes[0].length,0)
        finally:
            pipe.close();os.close(write)

    def test_known_eof_thread_join_is_bounded_by_same_post_reap_deadline(self):
        owner=self.owner();owner.waited=(12345,0,object())
        owner.output_deadline_ns=5_000_000_000
        for slot in owner.pipes:slot.reader='eof'
        thread=Mock(ident=17,is_alive=Mock(return_value=True));owner.pipes[0].thread=thread
        with patch.object(lifetime.time,'monotonic_ns',return_value=4_000_000_000):
            with self.assertRaisesRegex(lifetime.observer.ProcessObservationError,'reader remains active'):
                owner.output(Mock(),'digest_only')
        thread.join.assert_called_once_with(1.0)

    def test_incomplete_active_reader_retains_original_pipe_until_read_returns(self):
        owner=self.owner();read,write=os.pipe();pipe=os.fdopen(read,'rb',buffering=0)
        empty=open(os.devnull,'rb',buffering=0)
        owner.child.stdout,owner.child.stderr=pipe,empty
        for slot,original in zip(owner.pipes,(pipe,empty)):slot.pipe=original
        entered=threading.Event();calls=[];original_read=os.read
        def read_original(fd,size):
            if fd==read:calls.append(fd);entered.set()
            return original_read(fd,size)
        try:
            with patch.object(lifetime.os,'read',side_effect=read_original):
                owner.start_streams(harness.StreamDigest)
                self.assertTrue(entered.wait(1))
                owner.waited=(12345,0,object());owner.output_deadline_ns=0
                owner.complete(Mock());owner.close()
                self.assertTrue(owner.pipes[0].incomplete)
                self.assertTrue(owner.pipes[0].close_requested)
                self.assertFalse(pipe.closed)
                with self.assertRaisesRegex(lifetime.observer.ProcessObservationError,'lacks EOF'):
                    owner.output(Mock(),'digest_only')
                os.write(write,b'x')
                owner.pipes[0].thread.join(1)
                self.assertFalse(owner.pipes[0].thread.is_alive())
                self.assertTrue(pipe.closed);self.assertEqual(calls,[read])
                self.assertNotEqual(owner.pipes[0].reader,'eof')
        finally:
            os.close(write)
            for slot in owner.pipes:
                if slot.thread is not None and slot.thread.ident is not None:slot.thread.join(1)
            pipe.close();empty.close()

    def test_first_setup_error_survives_native_and_reader_close_errors(self):
        owner=self.owner();owner.waited=(12345,0,object())
        original=OSError(5,'setup');owner.retain('setup',original)
        owner.native.close.side_effect=OSError(6,'native close')
        owner.base.close.side_effect=OSError(7,'reader close')
        owner.close();self.assertIs(owner.first,original)
        self.assertEqual([stage for stage,_ in owner.errors],['setup','native_close','reader_close'])
        owner.base.close.assert_called_once()

    def test_diagnostic_append_failure_cannot_replace_original_first_error(self):
        owner=self.owner();owner.waited=(12345,0,object())
        allocation=MemoryError('diagnostic allocation')
        class FailingList(list):
            def append(self,value):raise allocation
        owner.errors=FailingList()
        original=OSError(5,'original read')
        owner.retain('sample',original)
        owner.native.close.side_effect=OSError(6,'later close')
        owner.close()
        self.assertIs(owner.first,original);self.assertIs(owner.retention_error,allocation)
        self.assertEqual(list(owner.diagnostics()),[('sample',original),('exception_retention',allocation)])

    def test_repeated_refusals_have_one_owner_per_stage_and_keep_distinct_closes(self):
        owner=self.owner();original=OSError(5,'first original')
        owner.retain('sample',original)
        for _ in range(1000):owner.retain('original_cleanup',abi.Refusal('native poll refused'))
        owner.retain('native_close',OSError(6,'native'))
        owner.retain('reader_close',OSError(7,'reader'))
        for index in range(100):owner.retain('unknown-private-stage-'+str(index),ValueError('fixed'))
        self.assertEqual([stage for stage,_ in owner.errors],
                         ['sample','original_cleanup','native_close','reader_close','unknown_stage'])
        self.assertIs(owner.first,original)
        self.assertLessEqual(len(owner.errors),len(lifetime.ERROR_STAGES))
        self.assertNotIn('private',str([stage for stage,_ in owner.diagnostics()]))

    def test_constructor_failure_releases_proc_original_preserving_first(self):
        base=types.SimpleNamespace(close=Mock(side_effect=OSError(6,'close')))
        first=ValueError('native constructor')
        with patch.object(lifetime.observer,'LinuxProcessReader',return_value=base), \
             patch.object(lifetime.abi,'NativeObservation',side_effect=first):
            with self.assertRaises(ValueError) as caught:lifetime.OriginalLifetime(Mock(),Mock(),0)
        self.assertIs(caught.exception,first);base.close.assert_called_once()

    def test_preparation_exists_before_original_popen_and_return_is_stored(self):
        owner=self.owner();owner.child=None
        child=types.SimpleNamespace(pid=12345)
        with patch.object(lifetime.subprocess,'Popen',return_value=child) as launch:
            self.assertIs(owner.popen(['closed-fixture']),child)
        self.assertIs(owner.child,child);launch.assert_called_once_with(['closed-fixture'])
        with self.assertRaises(abi.Refusal):owner.popen(['second'])

    def test_outer_unwind_before_completion_body_still_reaps_and_closes(self):
        owner=self.owner();original=OSError(5,'earlier sample')
        owner.retain('sample',original);later=KeyboardInterrupt('one shot')
        calls=[]
        def complete(_):
            calls.append('complete')
            if len(calls)==1:raise later
            owner.waited=(12345,0,object())
        owner.complete=complete;owner.close=Mock()
        owner.cleanup(Mock())
        self.assertEqual(len(calls),3);owner.close.assert_called_once()
        self.assertIs(owner.first,original)
        self.assertIs(owner.errors[1][1],later)

    def test_cleanup_claim_prevents_ambiguous_late_reader(self):
        slot=lifetime._Pipe(0)
        self.assertTrue(slot.claim_cleanup());self.assertFalse(slot.claim_thread())
        self.assertEqual(slot.reader,'cleanup')

    def test_both_over_capacity_pipes_drain_when_no_reader_or_second_start_fails(self):
        for mode in ('no-reader','second-start'):
            with self.subTest(mode=mode):
                owner=self.owner();readers=[];writers=[]
                payloads=(b'a'*131072,b'b'*131072)
                for payload in payloads:
                    read,write=os.pipe();readers.append(os.fdopen(read,'rb',buffering=0))
                    def write_all(fd=write,data=payload):
                        try:
                            offset=0
                            while offset<len(data):offset+=os.write(fd,data[offset:])
                        finally:os.close(fd)
                    thread=threading.Thread(target=write_all);thread.start();writers.append(thread)
                owner.child.stdout,owner.child.stderr=readers
                for slot,pipe in zip(owner.pipes,readers):slot.pipe=pipe
                try:
                    if mode=='second-start':
                        original=threading.Thread.start;calls=[0]
                        def start(thread):
                            calls[0]+=1
                            if calls[0]==2:raise OSError(5,'injected second start')
                            return original(thread)
                        with patch.object(threading.Thread,'start',start):
                            with self.assertRaises(OSError):owner.start_streams(harness.StreamDigest)
                    while any(slot.reader!='eof' for slot in owner.pipes):owner.drain_once(10)
                    for thread in writers:thread.join()
                    for slot,payload in zip(owner.pipes,payloads):
                        if slot.thread is not None and slot.thread.ident is not None:slot.thread.join()
                        self.assertEqual(slot.length,len(payload));self.assertEqual(slot.tail,payload[-17:])
                        self.assertEqual(slot.digest.hexdigest(),hashlib.sha256(payload).hexdigest())
                finally:
                    for pipe in readers:pipe.close()
                    for thread in writers:thread.join()


if __name__=='__main__':unittest.main()
