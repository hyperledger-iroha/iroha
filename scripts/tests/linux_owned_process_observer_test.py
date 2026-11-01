"""Pure retained-proc parsing, public projection and first-error controls.

These controls never load the native module or spawn a workload. Actual copied
public harness execution remains a separately reviewed component gate.
"""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import linux_owned_process_observer as owned
import private_settlement_process_observer as general


def stat_record(state=b'R', *, pid=12345, comm=b'private marker', **values):
    """Build a parser-only kernel-shaped record without claiming kernel evidence."""
    fields = [state] + [b'0'] * 21
    defaults = dict(ppid=23456, pgid=12345, utime=10, stime=20, start_ticks=11)
    defaults.update(values)
    for key,index in (('ppid',1),('pgid',2),('utime',11),('stime',12),('start_ticks',19)):
        fields[index] = str(defaults[key]).encode()
    return str(pid).encode() + b' (' + comm + b') ' + b' '.join(fields)


def fixture():
    """Public synthetic facts for validator mutation controls only."""
    identity = dict(pid=12345, ppid=23456, pgid=12345, uid=1000,
        birth=dict(kind='linux_proc',boot_id='11111111-2222-3333-4444-555555555555',start_ticks=11),
        executable_path='/fixture/image', executable_sha256='a'*64)
    admission = dict(capture=dict(sample=dict(identity=identity,cpu_time_ns=300_000_000,
        cpu_counter_unit_ns=10_000_000,rss_bytes=4096),started_ns=20,finished_ns=30),
        directory=dict(device=2096,inode=77,uid=1000))
    lifecycle = dict(finalized=True,lifecycle_certified=True,first_error=None,root_pid=12345,
        terminal=dict(pid=12345),counters=[dict(cpu=0,lost=0)],initial_exec_ns=10,observed_through_ns=90)
    terminal = dict(version=1,kind='linux_owned_terminal_stat',operation='stat_before',
        kernel_absence=None,identity=copy.deepcopy(identity),directory=copy.deepcopy(admission['directory']),
        stat=dict(pid=12345,state='Z',ppid=23456,pgid=12345,start_ticks=11,utime=11,stime=21),
        started_ns=50,finished_ns=60)
    return admission,lifecycle,terminal


class ParserControls(unittest.TestCase):
    """Ordinary live parsing and owned terminal parsing remain separate."""

    def test_every_original_live_state_still_parses_the_same_counters(self):
        for state in owned.LIVE_STATES:
            raw=stat_record(state)
            self.assertEqual(general.parse_linux_stat(raw,12345),
                             general._parse_linux_stat_states(raw,12345,(state,))[1])

    def test_ordinary_reader_refuses_zombie_and_unknown_states(self):
        for state in (b'Z',b'X',b'x',b'?'):
            with self.assertRaises(general.ProcessObservationError):
                general.parse_linux_stat(stat_record(state),12345)

    def test_state_refusal_precedes_bad_counter(self):
        raw=stat_record(b'Z').replace(b' 10 ',b' bad ',1)
        with self.assertRaisesRegex(general.ProcessObservationError,'unsupported kernel state'):
            general.parse_linux_stat(raw,12345)

    def test_parentheses_inside_comm_do_not_escape_original_pid_or_fields(self):
        raw=stat_record(comm=b'(private) tricky ) marker')
        self.assertEqual(general.parse_linux_stat(raw,12345)['utime'],10)
        with self.assertRaises(general.ProcessObservationError):
            general.parse_linux_stat(raw,12346)

    def test_empty_malformed_overflow_and_zero_identity_stay_errors(self):
        for raw in (b'',b'not stat',stat_record(start_ticks=0),stat_record(ppid=0),
                    stat_record(utime=1<<64)):
            with self.assertRaises(general.ProcessObservationError):
                general.parse_linux_stat(raw,12345)

    def test_all_originals_close_but_first_executable_close_error_survives(self):
        reader=general.LinuxProcessReader.__new__(general.LinuxProcessReader);reader.proc=7
        image=types.SimpleNamespace(validate=lambda:None)
        first,second=OSError('fixed first'),OSError('fixed second')
        with patch.object(general.os,'open',return_value=9), \
             patch.object(reader,'_sample_directory',side_effect=first), \
             patch.object(general.os,'close',side_effect=second) as close:
            with self.assertRaises(OSError) as caught: reader.sample(12345,image)
        self.assertIs(caught.exception,first);close.assert_called_once_with(9)

    def test_successful_sample_directory_close_failure_is_not_hidden(self):
        reader=general.LinuxProcessReader.__new__(general.LinuxProcessReader);reader.proc=7
        image=types.SimpleNamespace(validate=lambda:None)
        error=OSError('fixed close')
        with patch.object(general.os,'open',return_value=9), \
             patch.object(reader,'_sample_directory',return_value={}), \
             patch.object(general.os,'close',side_effect=error):
            with self.assertRaises(OSError) as caught: reader.sample(12345,image)
        self.assertIs(caught.exception,error)


class ProjectionControls(unittest.TestCase):
    """Replay rejects incomplete identity, counters, native proof and time joins."""

    def validate(self, admission=None,lifecycle=None,terminal=None):
        original=fixture()
        owned.validate_terminal(terminal if terminal is not None else original[2],
            admission if admission is not None else original[0],
            lifecycle if lifecycle is not None else original[1],300_000_000,330_000_000,1000)

    def test_closed_numeric_projection_accepts_both_direct_stat_sites(self):
        for operation in ('stat_before','stat_after'):
            _,_,terminal=fixture();terminal['operation']=operation
            self.validate(terminal=terminal)

    def test_every_terminal_field_is_required_and_extra_private_fields_refuse(self):
        for key in owned.TERMINAL_KEYS:
            _,_,terminal=fixture();del terminal[key]
            with self.assertRaises((general.ProcessObservationError,KeyError,TypeError)):
                self.validate(terminal=terminal)
        for key in ('raw','comm','stat_sha256','stat_bytes','private'):
            _,_,terminal=fixture();terminal[key]='private marker'
            with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)

    def test_every_original_identity_component_is_joined(self):
        for key in ('pid','ppid','pgid','uid','birth','executable_path','executable_sha256'):
            _,_,terminal=fixture();terminal['identity'][key]=None
            with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)
        for key in ('kind','boot_id','start_ticks'):
            _,_,terminal=fixture();terminal['identity']['birth'][key]=None
            with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)

    def test_every_original_directory_component_is_joined(self):
        for key in ('device','inode','uid'):
            _,_,terminal=fixture();terminal['directory'][key]+=1
            with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)

    def test_every_numeric_stat_field_has_exact_type_bounds_and_original_join(self):
        for key in owned.STAT_KEYS-{'state'}:
            for value in (True,-1,1<<64,None):
                _,_,terminal=fixture();terminal['stat'][key]=value
                with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)
        for key in ('pid','ppid','pgid','start_ticks'):
            _,_,terminal=fixture();terminal['stat'][key]+=1
            with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)

    def test_cpu_must_be_between_last_live_and_actual_wait4_with_fixed_slack(self):
        for key in ('utime','stime'):
            for value in (0,100):
                _,_,terminal=fixture();terminal['stat'][key]=value
                with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)

    def test_both_terminal_time_endpoints_and_order_are_joined(self):
        for key,value in [('started_ns',19),('started_ns',61),('finished_ns',49),
                          ('finished_ns',91),('started_ns',True),('finished_ns',None)]:
            _,_,terminal=fixture();terminal[key]=value
            with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)

    def test_initial_capture_schema_counter_image_and_both_times_are_closed(self):
        for key,value in [('started_ns',0),('finished_ns',19),('started_ns',True)]:
            admission,_,_=fixture();admission['capture'][key]=value
            with self.assertRaises(general.ProcessObservationError):self.validate(admission=admission)
        for key,value in [('cpu_time_ns',True),('cpu_counter_unit_ns',0),('rss_bytes',0)]:
            admission,_,_=fixture();admission['capture']['sample'][key]=value
            with self.assertRaises(general.ProcessObservationError):self.validate(admission=admission)
        for key,value in [('boot_id','malformed'),('start_ticks',0),('kind','other')]:
            admission,_,_=fixture();admission['capture']['sample']['identity']['birth'][key]=value
            with self.assertRaises(general.ProcessObservationError):self.validate(admission=admission)

    def test_replayed_cpu_quantum_matches_original_kernel_clock_refusal_bound(self):
        for quantum in (1,10,100):
            admission,_,_=fixture();admission['capture']['sample']['cpu_counter_unit_ns']=quantum
            with self.assertRaises(general.ProcessObservationError):owned.validate_admission(admission)
        admission,_,_=fixture();admission['capture']['sample']['cpu_counter_unit_ns']=1000
        owned.validate_admission(admission)

    def test_native_loss_error_false_certificate_or_wrong_root_never_waives_terminal(self):
        for key,value in [('lifecycle_certified',False),('finalized',False),
                          ('first_error',{'reason':'event_loss'}),('root_pid',12346)]:
            _,lifecycle,_=fixture();lifecycle[key]=value
            with self.assertRaises(general.ProcessObservationError):self.validate(lifecycle=lifecycle)
        _,lifecycle,_=fixture();lifecycle['counters'][0]['lost']=1
        with self.assertRaises(general.ProcessObservationError):self.validate(lifecycle=lifecycle)

    def test_kernel_absence_and_non_z_states_are_not_enabled(self):
        for key,value in [('kernel_absence',{'operation':'exe','errno':2}),('operation','exe'),
                          ('version',True),('kind','other')]:
            _,_,terminal=fixture();terminal[key]=value
            with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)
        _,_,terminal=fixture();terminal['stat']['state']='X'
        with self.assertRaises(general.ProcessObservationError):self.validate(terminal=terminal)

    def test_typed_fact_returns_fresh_numeric_projection_only(self):
        _,_,terminal=fixture();error=owned.LinuxOwnedTerminalObservation(terminal)
        projected=error.projection();projected['identity']['pid']=9
        self.assertEqual(error.projection(),terminal)
        self.assertNotIn('private',str(error));self.assertNotIn('comm',json.dumps(error.projection()))

    def test_z_parse_discards_raw_comm_before_terminal_exception_frames(self):
        class Owner:
            directory=7;pid=12345
            def terminal_stat(self,stage,fields):
                _,_,terminal=fixture()
                raise owned.LinuxOwnedTerminalObservation(terminal)
        reads=owned._OwnedReads(Owner())
        with patch.object(general,'kernel_record',return_value=stat_record(b'Z')):
            try: reads.stat('before')
            except owned.LinuxOwnedTerminalObservation as error:
                frame=error.__traceback__
                while frame:
                    if frame.tb_frame.f_code.co_filename==owned.__file__:
                        self.assertNotIn(b'private marker',frame.tb_frame.f_locals.values())
                        self.assertIsNone(frame.tb_frame.f_locals.get('raw'))
                    frame=frame.tb_next
                projection=error.projection();error.__traceback__=None
        self.assertEqual(set(projection),owned.TERMINAL_KEYS)

    def test_malformed_stat_keeps_fixed_error_without_parser_traceback_or_comm(self):
        owner=types.SimpleNamespace(directory=7,pid=12345)
        reads=owned._OwnedReads(owner)
        with patch.object(general,'kernel_record',return_value=stat_record(b'Z',utime=1<<64)):
            try: reads.stat('after')
            except general.ProcessObservationError as error:
                self.assertEqual(str(error),'proc stat counter exceeds u64')
                frame=error.__traceback__
                while frame:
                    self.assertNotEqual(frame.tb_frame.f_code.co_name,'_parse_linux_stat_states')
                    if frame.tb_frame.f_code.co_filename==owned.__file__:
                        self.assertIsNone(frame.tb_frame.f_locals.get('raw'))
                    frame=frame.tb_next
            else:self.fail('malformed terminal was accepted')




class ExecutableCloseControls(unittest.TestCase):
    """Real original descriptors with explicitly injected read/terminal failures."""

    def exercise(self, *, owned_path, terminal, break_close):
        """Read this fixture process only, without launching or attaching a child."""
        import errno
        import hashlib
        path=Path('/proc/self/exe').resolve()
        image=general.ExecutableImage(path,hashlib.sha256(path.read_bytes()).hexdigest())
        reader=general.LinuxProcessReader()
        directory=os.open(str(os.getpid()),os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC,dir_fd=reader.proc)
        owner=types.SimpleNamespace(directory=directory,pid=os.getpid(),cleanup_error=None,failed=False)
        _,_,projection=fixture()
        initiating=(owned.LinuxOwnedTerminalObservation(projection) if terminal
                    else PermissionError(errno.EACCES,'injected bounded read failure'))
        parent=owned._OwnedReads if owned_path else general._LinuxSampleReads
        class Reads(parent):
            def __init__(self):
                if owned_path:super().__init__(owner)
                else:super().__init__(directory,os.getpid())
                self.original_fd=None
            def stat(self,stage):
                return general._LinuxSampleReads.stat(self,stage)
            def open_executable(self):
                self.original_fd=super().open_executable()
                return self.original_fd
            def resident_bytes(self):
                # This controlled early close makes the later close return an
                # actual EBADF. No foreign descriptor or kernel failure is faked.
                if break_close:os.close(self.original_fd)
                raise initiating
        reads=Reads()
        try:
            try:reader._sample_directory(os.getpid(),image,directory,reads)
            except BaseException as selected:observed=selected
            else:self.fail('injected read/terminal failure unexpectedly returned a sample')
            with self.assertRaises(OSError) as closed:os.fstat(reads.original_fd)
            self.assertEqual(closed.exception.errno,errno.EBADF)
            return initiating,observed,reads.cleanup_error,owner
        finally:
            os.close(directory);reader.close();image.close()

    def test_general_read_error_survives_actual_executable_close_ebadf(self):
        original,selected,cleanup,_=self.exercise(owned_path=False,terminal=False,break_close=True)
        self.assertIs(selected,original)
        self.assertIsInstance(cleanup,OSError)
        self.assertEqual(cleanup.errno,9)
        self.assertIsNot(cleanup,original)

    def test_owned_ordinary_read_error_survives_and_records_distinct_close_failure(self):
        original,selected,cleanup,owner=self.exercise(owned_path=True,terminal=False,break_close=True)
        self.assertIs(selected,original);self.assertIs(owner.cleanup_error,cleanup)
        self.assertTrue(owner.failed);self.assertEqual(cleanup.errno,9)

    def test_pending_z_is_poisoned_by_actual_executable_close_ebadf(self):
        pending,selected,cleanup,owner=self.exercise(owned_path=True,terminal=True,break_close=True)
        self.assertIs(selected,cleanup);self.assertIs(owner.cleanup_error,cleanup)
        self.assertNotIsInstance(selected,owned.LinuxOwnedTerminalObservation)
        self.assertTrue(owner.failed);self.assertEqual(cleanup.errno,9)
        self.assertIsNone(pending.__traceback__)

    def test_successful_close_keeps_pending_terminal_pending_only(self):
        pending,selected,cleanup,owner=self.exercise(owned_path=True,terminal=True,break_close=False)
        self.assertIs(selected,pending);self.assertIsNone(cleanup)
        self.assertIsNone(owner.cleanup_error);self.assertFalse(owner.failed)


if __name__=='__main__':unittest.main()
