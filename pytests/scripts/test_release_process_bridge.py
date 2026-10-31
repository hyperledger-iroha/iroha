"""Actual compiled-library/process diagnostics, not independent release artifact trust."""
from contextlib import ExitStack
import ctypes
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
import time

import pytest

BASE=Path(__file__).resolve().parents[2]/'scripts'
sys.path.insert(0,str(BASE))
import release_process_bridge as bridge_parent
import workspace_source_keeper as custody


@pytest.fixture(scope='module')
def compiled(tmp_path_factory):
    root=tmp_path_factory.mktemp('bridge-compiled')
    source=root/'probe.rs'
    source.write_text(r'''//! Genuine process bridge test image; no release publication.
use std::{env,fs,io::{self,Write},path::Path,time::Duration};
fn main(){
 let args:Vec<_>=env::args().collect();
 let mode=&args[1];let release=&args[2];let finished=&args[3];
 if mode=="leaf" { while !Path::new(release).exists(){std::thread::sleep(Duration::from_millis(5));} return; }
 if mode=="descendant" || mode=="descendant_failure" {
  let _child=std::process::Command::new(env::current_exe().unwrap()).args(["leaf",release,finished])
   .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::inherit()).spawn().unwrap();
 }
 if mode=="wait" || mode=="wait_output" {
  if mode=="wait_output" { println!("owned-child-still-live");io::stdout().flush().unwrap(); }
  while !Path::new(release).exists(){std::thread::sleep(Duration::from_millis(5));}
 }
 if mode=="overflow" {io::stdout().write_all(&vec![b'x';262144]).unwrap();}
 else{println!("owned-native-entry");}
 fs::write(finished,b"actual root completion").unwrap();
 if mode=="failure" || mode=="descendant_failure" {std::process::exit(17);}
 if mode=="failure_seven" {std::process::exit(7);}
}
''')
    program=root/('bridge-probe.exe' if os.name=='nt' else 'bridge-probe')
    compiler=['rustc']+(['+1.93.1-x86_64-pc-windows-msvc'] if os.name=='nt' else [])
    subprocess.run([*compiler,'--edition=2024','-C','debuginfo=0',str(source),'-o',str(program)],check=True)
    filename='iroha_process_bridge.dll' if os.name=='nt' else 'libiroha_process_bridge.so'
    library=Path(os.environ.get('IROHA_TEST_PROCESS_BRIDGE_LIBRARY',str(BASE.parent/'target'/'debug'/filename)))
    assert library.is_file(),'compile the exact diagnostic cdylib first'
    return program,library


class Fixture:
    def __init__(self,root,compiled,mode='echo'):
        originals=root/'source-parent'/'originals'; originals.mkdir(parents=True)
        controls=root/'controls'; controls.mkdir()
        program,library=compiled
        self.program=originals/program.name; self.library=originals/library.name
        shutil.copyfile(program,self.program); shutil.copyfile(library,self.library)
        if os.name=='posix':self.program.chmod(0o500);self.library.chmod(0o500)
        self.release=controls/'release';self.finished=controls/'finished'
        self.closed=[];self.owners=ExitStack()
        self.owners.callback(self.closed.append,'closed')
        self.original=self.owners.enter_context(custody.SourceKeeper(originals))
        self.original.capture([self.program.name,self.library.name])
        # Fixture artifact selection is explicit diagnostic data. A release caller
        # must independently validate its hashes/provenance before this load.
        self.cdll=ctypes.CDLL(str(self.library),**({'winmode':0x100|0x800} if os.name=='nt' else {}))
        self.bridge=bridge_parent.Bridge(self.cdll)
        environment={}
        for key in ('SystemRoot','SYSTEMROOT'):
            if key in os.environ:environment[key]=os.environ[key]
        self.settings={'program':str(self.program),'directory':str(controls),
            'arguments':[mode,str(self.release),str(self.finished)],'environment':environment}

    def run(self):return self.bridge.run(self.settings,self.owners)


def test_actual_library_runs_original_child_before_any_source_owner_closes(tmp_path,compiled):
    case=Fixture(tmp_path,compiled)
    with case.owners:
        assert case.run()==b'owned-native-entry\n'
        assert case.finished.is_file()
        assert case.closed==[]
        case.original.revalidate()
    assert case.closed==['closed']


@pytest.mark.parametrize('mode',['failure','overflow'])
def test_failed_or_excessive_entry_rejects_after_terminal_without_signaling(tmp_path,compiled,mode):
    case=Fixture(tmp_path,compiled,mode)
    with pytest.raises(RuntimeError):
        with case.owners:
            try:case.run()
            finally:
                assert case.finished.is_file()
                assert case.closed==[]
                case.original.revalidate()
    assert case.closed==['closed']
    assert bridge_parent._FAILED_CUSTODY==[]


@pytest.mark.parametrize('exception',[KeyboardInterrupt,SystemExit])
def test_interrupted_observation_remains_refusal_after_actual_terminal(tmp_path,compiled,exception):
    case=Fixture(tmp_path,compiled)
    actual=case.bridge.wait
    calls=[]
    first=exception('original observation interruption')
    def interrupted(*arguments):
        if not calls:
            calls.append(True);raise first
        return actual(*arguments)
    case.bridge.wait=interrupted
    with pytest.raises(exception) as failure:
        with case.owners:
            try:case.run()
            finally:
                assert case.finished.is_file()
                assert calls==[True]
                assert case.closed==[]
                case.original.revalidate()
    assert failure.value is first
    assert case.closed==['closed']
    assert bridge_parent._FAILED_CUSTODY==[]


def wait_until(predicate, worker=None):
    """Wait for a bounded fixture handshake without controlling any process."""
    deadline=time.monotonic()+10
    while not predicate():
        assert worker is None or worker.is_alive(), 'supervisor returned before fixture handoff'
        assert time.monotonic()<deadline, 'fixture handoff timed out'
        time.sleep(.005)


def held_refusal(case, fault, restore, *, after_finished=None):
    """Externally release one genuine workload while the same supervisor retains it."""
    failures=[]
    def supervise():
        try:
            with case.owners:case.run()
        except BaseException as error:failures.append(error)
    worker=threading.Thread(target=supervise)
    worker.start()
    try:
        wait_until(fault.is_set,worker)
        assert worker.is_alive()
        assert case.closed==[]
        assert not case.finished.exists()
        assert len(bridge_parent._FAILED_CUSTODY)==1
        capsule=bridge_parent._FAILED_CUSTODY[0]
        token=capsule.observation.token.value
        assert token!=0
        assert capsule.originals is not case.owners
        case.original.revalidate()
        case.release.write_bytes(b'complete only this owned workload')
        wait_until(case.finished.is_file)
        if after_finished is not None:after_finished(worker,token)
    finally:
        # Restore only the captured original observers and release only the
        # fixture's own file protocol; never clear the production custody list.
        restore()
        case.release.write_bytes(b'fixture cleanup completion')
        worker.join(10)
    assert not worker.is_alive(), 'original supervisor did not observe controlled completion'
    assert len(failures)==1
    assert case.finished.is_file()
    assert case.closed==['closed']
    assert bridge_parent._FAILED_CUSTODY==[]
    return failures[0]


def test_unobservable_session_keeps_supervisor_alive_until_same_token_observers_restore(tmp_path,compiled):
    case=Fixture(tmp_path,compiled,'wait')
    actual_read,actual_wait,actual_close=case.bridge.read,case.bridge.wait,case.bridge.close
    fault=threading.Event(); after_finish=threading.Event(); tokens=[]
    first=OSError('original reader observation failed')
    def read(*_arguments):
        fault.set();raise first
    def refuse(*arguments):
        tokens.append(arguments[0])
        if case.finished.exists():after_finish.set()
        return -3
    case.bridge.read=read;case.bridge.wait=refuse;case.bridge.close=refuse
    def restore():
        case.bridge.read,case.bridge.wait,case.bridge.close=actual_read,actual_wait,actual_close
    def finished(worker,token):
        wait_until(after_finish.is_set,worker)
        assert worker.is_alive()
        assert case.closed==[]
        assert tokens and set(tokens)=={token}
        case.original.revalidate()
        exit_code=ctypes.c_int64()
        wait_until(lambda:actual_wait(token,ctypes.byref(exit_code))==0,worker)
        assert exit_code.value==0
        # Even separately established terminal truth does not cause the refused
        # initiating observer to infer it from a marker, PID, or another session.
        assert worker.is_alive()
        assert case.closed==[]
    assert held_refusal(case,fault,restore,after_finished=finished) is first


def test_live_unreadable_session_retains_originals_until_controlled_completion(tmp_path,compiled):
    case=Fixture(tmp_path,compiled,'wait')
    actual_read=case.bridge.read
    fault=threading.Event()
    first=OSError('original live stdout observation failed')
    def unreadable(*_arguments):fault.set();raise first
    case.bridge.read=unreadable
    assert held_refusal(case,fault,lambda:setattr(case.bridge,'read',actual_read)) is first


@pytest.mark.parametrize('location',['read_buffer','output_buffer','count_creation'])
def test_observation_allocations_are_complete_before_spawn(tmp_path,compiled,monkeypatch,location):
    case=Fixture(tmp_path,compiled,'wait_output')
    original_buffer=bridge_parent.c.create_string_buffer
    spawned=[];calls=[]
    first=KeyboardInterrupt('count allocation refused') if location=='count_creation' else MemoryError('bounded storage refused')
    case.bridge.spawn=lambda *_:spawned.append(True)
    if location=='count_creation':
        def count(*_arguments):raise first
        monkeypatch.setattr(bridge_parent.c,'c_size_t',count)
    else:
        def allocate(*arguments):
            calls.append(True)
            if len(calls)==(2 if location=='read_buffer' else 3):raise first
            return original_buffer(*arguments)
        monkeypatch.setattr(bridge_parent.c,'create_string_buffer',allocate)
    with case.owners:
        with pytest.raises(type(first)) as failure:case.run()
        assert failure.value is first
        assert spawned==[]
        assert not case.finished.exists()
        assert case.closed==[]
        assert bridge_parent._FAILED_CUSTODY==[]
        case.original.revalidate()
    assert case.closed==['closed']


def test_unexpected_output_accumulation_failure_keeps_reserved_custody(tmp_path,compiled,monkeypatch):
    case=Fixture(tmp_path,compiled,'wait_output')
    actual=bridge_parent.c.memmove
    first=MemoryError('actual bounded output accumulation refused')
    fault=threading.Event()
    def refused(*_arguments):fault.set();raise first
    monkeypatch.setattr(bridge_parent.c,'memmove',refused)
    assert held_refusal(case,fault,lambda:monkeypatch.setattr(bridge_parent.c,'memmove',actual)) is first


def test_first_read_interruption_survives_later_unexpected_observer_allocation_failure(tmp_path,compiled):
    case=Fixture(tmp_path,compiled,'wait_output')
    actual_read,actual_wait=case.bridge.read,case.bridge.wait
    first=KeyboardInterrupt('original read interruption')
    fault=threading.Event();later=[]
    def read(*_arguments):raise first
    def wait(*_arguments):
        later.append(True);fault.set();raise MemoryError('later observer allocation failure')
    case.bridge.read,case.bridge.wait=read,wait
    def restore():case.bridge.read,case.bridge.wait=actual_read,actual_wait
    assert held_refusal(case,fault,restore) is first
    assert later


if os.name=='nt':
    @pytest.mark.parametrize('mode',['descendant','descendant_failure'])
    def test_parent_keeps_original_library_and_source_after_root_exit_until_job_descendant_exit(tmp_path,compiled,mode):
        case=Fixture(tmp_path,compiled,mode)
        observed=[];failures=[]
        def release():
            try:
                wait_until(case.finished.is_file)
                assert case.closed==[]
                case.original.revalidate()
                observed.append('root return marker, descendant still owns Job')
            except BaseException as error:failures.append(error)
            finally:case.release.write_bytes(b'controlled descendant release')
        worker=threading.Thread(target=release);worker.start()
        try:
            if mode=='descendant_failure':
                with pytest.raises(RuntimeError,match='exited unsuccessfully'):
                    with case.owners:case.run()
            else:
                with case.owners:
                    assert case.run()==b'owned-native-entry\n'
                    assert case.finished.is_file()
                    assert observed
                    case.original.revalidate()
        finally:worker.join(10)
        assert not worker.is_alive()
        assert failures==[]
        assert observed
        assert case.closed==['closed']
