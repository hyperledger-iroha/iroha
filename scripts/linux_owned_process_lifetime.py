"""Copied pre-Popen observation and pipe ownership for one original Linux child.

The caller owns process creation, deadline signalling and wait4 totals. This
owner never attaches to a PID, never controls descendants, and retains any
unclosed native token/module after refused cleanup. Artifact admission is an
explicit caller dependency; no ambient native path is loaded here.
"""
from __future__ import annotations

import hashlib
import os
import select
import subprocess
import time
import threading

import linux_root_observer_abi as abi
import linux_owned_process_observer as owned
import private_settlement_process_observer as observer


ERROR_STAGES = (
    'popen', 'outer_run', 'stream_read', 'native_finish', 'native_reap_join',
    'original_cleanup', 'deadline_cleanup', 'stream_cleanup', 'outer_completion',
    'outer_completion_retry', 'final_stream_completion', 'outer_close',
    'native_close', 'reader_close', 'pipe_close', 'monitor_registration',
    'baseline', 'sample', 'terminal_authentication', 'reader_journal_or_window',
    'monitor_close', 'terminal_publication', 'linux_owned_evidence',
    'exception_retention', 'executable_close', 'setup', 'unknown_stage',
)


class _Pipe:
    """Reserve exclusive reader custody and bounded output state before Popen."""

    def __init__(self, tail_bytes):
        self.lock = threading.Lock()
        self.pipe = self.thread = self.error = None
        self.reader = None
        self.incomplete = False
        self.close_requested = False
        self.digest = hashlib.sha256()
        self.length, self.tail_bytes, self.tail = 0, tail_bytes, b''

    def claim_thread(self):
        """Refuse a delayed thread when cleanup has already taken read custody."""
        with self.lock:
            if self.reader is not None or self.incomplete:
                return False
            self.reader = 'thread'
            return True

    def claim_cleanup(self):
        """Take an unstarted or failed reader without competing with a live read."""
        with self.lock:
            if self.incomplete:
                return False
            if self.reader in (None, 'done'):
                self.reader = 'cleanup'
            return self.reader == 'cleanup'

    def consume(self, chunk):
        """Use the same digest/count/bounded-tail contract in either exclusive reader."""
        self.digest.update(chunk)
        self.length += len(chunk)
        if self.tail_bytes:
            self.tail = (self.tail + chunk)[-self.tail_bytes:]

    def close_after_reap(self):
        """Leave an active read's exact pipe with that reader until its finalizer."""
        with self.lock:
            if self.reader=='thread':
                self.incomplete=True
                self.close_requested=True
                return
            if self.pipe is not None:self.pipe.close()


class _CaptureReader:
    """Make ProcessScope's first call the native adapter's actual owned capture."""

    def __init__(self, owner):
        self.owner = owner

    def sample(self, pid, image):
        """Return the same genuine sample whose endpoints the adapter captured."""
        native, reader = self.owner.native, self.owner.reader
        observer.require(pid == native.original_pid and image is native.workload,
                         'initial scope selection differs from original')
        if reader._admission_raw is None:
            captured = native.capture_initial()
            reader.bind_initial(captured)
            return captured['sample']
        return reader.sample(pid, image)

    def close(self):
        """The outer owner closes this reader only after finish and original reap."""


class OriginalLifetime:
    """Retain the original child and every observation owner until explicit cleanup."""

    def __init__(self, module_owner, workload, tail_bytes):
        """Allocate output/error slots before acquiring the native observer resources."""
        self.module_owner, self.workload = module_owner, workload
        self.native = self.base = self.reader = self.child = None
        self.scope_reader = _CaptureReader(self)
        self.pipes = (_Pipe(tail_bytes), _Pipe(tail_bytes))
        self.poller = select.poll()
        self.first = self.first_stage = self.retention_error = None
        self.errors = []
        self.waited = None
        self.wnowait = None
        self.finish_attempted = False
        self.joined = False
        self.terminal_fact = None
        self.closed = False
        self.started_ns = self.deadline_ns = None
        self.output_drain_ns = self.output_deadline_ns = None
        try:
            self.base = observer.LinuxProcessReader()
            self.native = abi.NativeObservation(module_owner, workload, self.base)
        except BaseException:
            if self.base is not None:
                try:self.base.close()
                except BaseException:pass
            raise

    def retain(self, stage, error):
        """Latch global first and at most one original per closed diagnostic stage."""
        if self.first is None:
            self.first = error
        try:
            stage = stage if type(stage) is str and stage in ERROR_STAGES else 'unknown_stage'
            if self.first_stage is None:
                self.first_stage = stage
            if not any(stage == previous or error is original for previous,original in self.errors):
                self.errors.append((stage,error))
        except BaseException as diagnostic_error:
            if self.first_stage is None:
                self.first_stage = 'exception_retention'
            if self.retention_error is None:
                self.retention_error = diagnostic_error

    def diagnostics(self):
        """Project first error independently of the fallible diagnostic list."""
        if self.first is not None:
            yield self.first_stage,self.first
        for stage,error in self.errors:
            if error is not self.first:
                yield stage,error
        if self.retention_error is not None and self.retention_error is not self.first:
            yield 'exception_retention',self.retention_error

    def prepare(self, timeout_ms, output_drain_seconds):
        """Prepare before Popen and every stream thread; failure forbids launch."""
        self.started_ns=time.monotonic_ns()
        self.deadline_ns=self.started_ns+timeout_ms*1_000_000
        self.output_drain_ns=round(output_drain_seconds*1_000_000_000)
        abi.require(self.native.prepare() == abi.OK, 'native preparation failed')

    def popen(self, *args, **kwargs):
        """Store Popen's returned original directly inside the preinstalled owner."""
        abi.require(self.child is None and self.native.prepared, 'invalid original launch state')
        self.child = subprocess.Popen(*args, **kwargs)
        return self.child

    def install(self, child, timeout_ms):
        """Store the returned original before any fallible post-spawn observation."""
        self.child = child
        for slot,pipe in zip(self.pipes,(child.stdout,child.stderr),strict=True):
            slot.pipe = pipe
        abi.require(self.started_ns is not None and
                    self.deadline_ns == self.started_ns+timeout_ms*1_000_000,
                    'original setup deadline changed')
        while True:
            abi.require(time.monotonic_ns()<self.deadline_ns,
                        'original creation binding exceeded overall deadline')
            status = self.native.bind_original(child)
            poll,report,_ = self.native.read()
            abi.require(time.monotonic_ns()<self.deadline_ns,
                        'original creation binding exceeded overall deadline')
            abi.require(poll == abi.OK and report is not None, 'native creation report unavailable')
            if report['root_pid'] == child.pid:
                abi.require(status in (abi.OK,abi.IO), 'unexpected native binding status')
                abi.require(report['first_error'] is None, 'native creation lifecycle refused')
                break
            abi.require(status == abi.PENDING, 'original creation evidence unavailable')
            abi.require(os.waitid(os.P_PID,child.pid,os.WEXITED|os.WNOHANG|os.WNOWAIT) is None,
                        'original terminated before creation binding')
            time.sleep(.001)
        self.reader = owned.OwnedLinuxProcessReader(self.native,self.base,self.workload)
        self.native.reader = self.reader

    def start_streams(self, stream_type):
        """Start the maintained digest thread interface under exclusive pipe claims."""
        class _OwnedStream(stream_type):
            """Preserve a first read failure and permit cleanup to drain the remainder."""
            def __init__(self, slot):
                super().__init__(slot.pipe.fileno(),slot.tail_bytes)
                self.slot = slot
            def run(self):
                """Read once or yield an ambiguous delayed start to cleanup."""
                if not self.slot.claim_thread():return
                try:
                    while True:
                        if self.slot.incomplete:return
                        chunk=os.read(self.descriptor,1<<16)
                        if not chunk:
                            self.slot.reader='eof'
                            return
                        self.slot.consume(chunk)
                except BaseException as error:
                    self.slot.error=error
                finally:
                    with self.slot.lock:
                        if self.slot.reader=='thread':self.slot.reader='done'
                        if self.slot.close_requested and self.slot.pipe is not None:
                            try:self.slot.pipe.close()
                            except BaseException as error:
                                if self.slot.error is None:self.slot.error=error
        for slot in self.pipes:
            slot.thread=_OwnedStream(slot)
            slot.thread.start()

    def drain_once(self, milliseconds=0):
        """Drain both unclaimed originals without blocking the deadline or perf poll."""
        for slot,pipe in zip(self.pipes,(self.child.stdout,self.child.stderr),strict=True):
            if slot.pipe is None:slot.pipe=pipe
            if slot.pipe is None:
                slot.reader='eof'
            elif slot.error is not None:
                self.retain('stream_read',slot.error)
            if slot.pipe is not None and slot.claim_cleanup():
                self.poller.register(slot.pipe.fileno(),select.POLLIN|select.POLLHUP)
        for descriptor,_ in self.poller.poll(milliseconds):
            slot=next(slot for slot in self.pipes
                      if slot.pipe is not None and slot.pipe.fileno()==descriptor)
            chunk=os.read(descriptor,1<<16)
            if chunk:slot.consume(chunk)
            else:
                slot.reader='eof';self.poller.unregister(descriptor)

    def poll(self):
        """Retain complete live event coverage; any native refusal stays a failure."""
        status,value,_=self.native.read()
        abi.require(status==abi.OK and value is not None,'native lifecycle poll failed')
        abi.require(value['first_error'] is None,'native lifecycle refused')
        return value

    def try_reap(self):
        """Finalize native observation at original WNOWAIT before calling wait4."""
        if self.waited is not None:return self.waited[1:]
        terminal=os.waitid(os.P_PID,self.child.pid,os.WEXITED|os.WNOHANG|os.WNOWAIT)
        if terminal is None:
            self.poll()
            return None
        self.wnowait={'pid':terminal.si_pid,'code':terminal.si_code,'status':terminal.si_status}
        # Even on refusal, finish is attempted before original cleanup reap. The
        # native token stays retained if its complete closure cannot be proved.
        self.finish_attempted=True
        try:
            status,value,_=self.native.read(finish=True)
            abi.require(status==abi.OK and value is not None and value['finalized'],
                        'native finish refused before original reap')
            abi.require(value['lifecycle_certified'],'native lifecycle was not certified')
        except BaseException as error:self.retain('native_finish',error)
        self.waited=os.wait4(self.child.pid,0)
        self.output_deadline_ns=time.monotonic_ns()+self.output_drain_ns
        self.child.returncode=os.waitstatus_to_exitcode(self.waited[1])
        try:
            abi.require(self.waited[0]==self.child.pid,'wait4 selected another original')
            result=self.native.join_wait(self.waited[0],self.waited[1])
            self.joined=result['lifecycle_and_initial_image_joined']
            status,_,raw=self.native.read(finish=True)
            abi.require(status==abi.OK and raw==self.native.final_raw,'frozen lifecycle changed after reap')
        except BaseException as error:self.retain('native_reap_join',error)
        return self.waited[1:]

    def refuse_incomplete_output(self):
        """Keep missing EOF explicit after original reap without publishing a digest."""
        observer.require(self.waited is not None,'incomplete output requires original reap')
        for slot in self.pipes:
            if slot.reader!='eof':slot.incomplete=True

    def complete(self, kill_after_deadline):
        """Reap independently of drain errors, then apply the existing output bound."""
        if self.child is None:return
        while self.waited is None:
            try:self.drain_once(10)
            except BaseException as error:self.retain('stream_cleanup',error)
            try:self.try_reap()
            except BaseException as error:self.retain('original_cleanup',error)
            try:
                if self.waited is None and time.monotonic_ns()>=self.deadline_ns:
                    # The caller's existing own-child deadline policy supplies
                    # this operation; this observer never acquires signal authority.
                    kill_after_deadline(self.child.pid)
            except BaseException as error:self.retain('deadline_cleanup',error)
        if self.output_deadline_ns is None:
            self.output_deadline_ns=time.monotonic_ns()+self.output_drain_ns
        while any(slot.reader!='eof' and not slot.incomplete for slot in self.pipes):
            try:
                abi.require(time.monotonic_ns()<self.output_deadline_ns,
                            'original output drain deadline exceeded')
                self.drain_once(10)
            except BaseException as error:
                self.retain('stream_cleanup',error)
                self.refuse_incomplete_output()

    def cleanup(self, kill_after_deadline):
        """An enclosing waiter survives one-shot unwind around completion setup."""
        try:
            self.complete(kill_after_deadline)
        except BaseException as error:
            self.retain('outer_completion',error)
        finally:
            while self.child is not None and self.waited is None:
                try:self.complete(kill_after_deadline)
                except BaseException as error:self.retain('outer_completion_retry',error)
            try:
                self.complete(kill_after_deadline)
            except BaseException as error:self.retain('final_stream_completion',error)
            try:
                self.close()
            except BaseException as error:self.retain('outer_close',error)

    def output(self, records, retention):
        """Return final bounded output facts only after every original pipe reached EOF."""
        observer.require(all(slot.reader=='eof' and not slot.incomplete for slot in self.pipes),
                         'original output lacks EOF')
        result={'classification':'measured','retention':retention}
        for name,slot in zip(('stdout','stderr'),self.pipes,strict=True):
            if slot.thread is not None and slot.thread.ident is not None:
                remaining=max(0,self.output_deadline_ns-time.monotonic_ns())/1_000_000_000
                slot.thread.join(remaining)
                observer.require(not slot.thread.is_alive(),'original output reader remains active')
            tail=records.publish(name+'.tail.log',slot.tail) if slot.tail_bytes and slot.tail else None
            result[name]={'bytes':slot.length,'sha256':slot.digest.hexdigest(),
                          'complete':True,'tail':tail}
        return result

    def close(self):
        """Close observation only after original reap; unknown tokens keep module pins."""
        if self.closed:return
        observer.require(self.child is None or self.waited is not None,'original is not reaped')
        try:
            status=self.native.close()
            abi.require(status==abi.OK,'native observer cleanup remains unknown')
        except BaseException as error:self.retain('native_close',error)
        for original in (self.reader if self.reader is not None else self.base,):
            if original is not None:
                try:original.close()
                except BaseException as error:self.retain('reader_close',error)
        for slot in self.pipes:
            if slot.pipe is not None:
                try:slot.close_after_reap()
                except BaseException as error:self.retain('pipe_close',error)
        self.closed=True
