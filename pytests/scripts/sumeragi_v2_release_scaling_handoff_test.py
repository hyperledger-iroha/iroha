"""Private socket/process-double coverage; real child inheritance remains pending."""
from __future__ import annotations
import dataclasses
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import socket
import stat
import subprocess
import sys
import threading
import time
import types

import pytest

ROOT=Path(__file__).resolve().parents[2]


def load(name):
    path=ROOT/'scripts'/f'{name}.py'
    spec=importlib.util.spec_from_file_location('private_'+name,path)
    module=importlib.util.module_from_spec(spec)
    sys.modules[spec.name]=module
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def modules(monkeypatch):
    def forbidden(*args,**kwargs):
        raise AssertionError('real child execution is forbidden in this validation slice')
    monkeypatch.setattr(subprocess,'Popen',forbidden)
    for name in ('system','fork','posix_spawn','posix_spawnp','kill','killpg'):
        if hasattr(os,name): monkeypatch.setattr(os,name,forbidden)
    return load('bootstrap_sumeragi_v2_release'),load('sumeragi_v2_release_scaling_handoff')


class Process:
    pid=47001
    def __init__(self, code=0, stdout=b'ok', stderr=b''):
        self.code=code;self.stdout=io.BytesIO(stdout);self.stderr=io.BytesIO(stderr)
        self.waits=0
    def poll(self): return self.code
    def wait(self): self.waits+=1;return self.code
    def kill(self): raise AssertionError('no process signals')
    terminate=kill


def bounded(module,monkeypatch,tmp_path,process=None,**kwargs):
    process=process or Process()
    calls=[]
    def popen(argv,**options): calls.append((argv,options));return process
    monkeypatch.setattr(module.subprocess,'Popen',popen)
    observation=module.CommandObservationOwner()
    arguments=dict(cwd=tmp_path,environment={},timeout_seconds=60,maximum_output_bytes=4096,
        observation=observation)
    arguments.update(kwargs)
    return process,calls,observation,lambda:module._run_bounded(Path('/fixed/python'),(),**arguments)


def test_bounded_observes_start_before_spawn_and_actual_exit(modules,monkeypatch,tmp_path):
    module,_=modules
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,Process(23,b'out',b'err'))
    real=module.subprocess.Popen
    def checked(*args,**kwargs):
        assert owner._phase=='starting' and owner._input[4]>0
        return real(*args,**kwargs)
    monkeypatch.setattr(module.subprocess,'Popen',checked)
    result=run();observed=owner.terminal
    assert result.returncode==observed.returncode==23
    assert observed.pid==process.pid and owner._process is process and process.waits==1
    assert observed.started_ns<=observed.completed_ns<=observed.deadline_ns
    assert observed.stdout_sha256==hashlib.sha256(b'out').hexdigest()
    assert observed.stderr_bytes==3 and observed.violations==()
    assert calls[0][1]['close_fds'] is True and calls[0][1]['pass_fds']==()
    with pytest.raises(module.BootstrapError,match='already consumed'): run()


def test_bounded_overflow_retains_full_hash_and_zero_actual_exit(modules,monkeypatch,tmp_path):
    module,_=modules
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,Process(0,b'abcdef',b'gh'),maximum_output_bytes=3)
    with pytest.raises(module.BootstrapError,match='output limit'): run()
    assert process.waits==1 and owner.terminal.returncode==0
    assert owner.terminal.stdout_bytes==6 and owner.terminal.stderr_bytes==2
    assert owner.terminal.stdout_sha256==hashlib.sha256(b'abcdef').hexdigest()
    assert owner.terminal.violations==('output',)


def test_bounded_runtime_latch_keeps_actual_exit(modules,monkeypatch,tmp_path):
    module,_=modules
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,Process(17),timeout_seconds=1)
    times=iter((10.0,12.0));monkeypatch.setattr(module.time,'monotonic',lambda:next(times))
    with pytest.raises(module.BootstrapError,match='bounded runtime'): run()
    assert process.waits==1 and owner.terminal.returncode==17
    assert owner.terminal.started_ns==10_000_000_000
    assert owner.terminal.deadline_ns==11_000_000_000
    assert owner.terminal.completed_ns==12_000_000_000
    assert owner.terminal.violations==('runtime',)


def test_bounded_observer_and_wait_interruptions_reap_naturally(modules,monkeypatch,tmp_path):
    module,_=modules
    class Interrupted(Process):
        def poll(self): raise KeyboardInterrupt()
        def wait(self):
            self.waits+=1
            if self.waits==1: raise KeyboardInterrupt()
            return 23
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,Interrupted())
    with pytest.raises(KeyboardInterrupt): run()
    assert process.waits==2 and owner.terminal.returncode==23
    assert owner.terminal.violations==('supervision',)


def test_bounded_drain_failure_still_records_terminal_exit(modules,monkeypatch,tmp_path):
    module,_=modules
    class BadStream(io.BytesIO):
        def read(self,*args): raise OSError('drain failed')
    process=Process(0);process.stdout=BadStream()
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,process)
    with pytest.raises(module.BootstrapError,match='output drain failed'): run()
    assert process.waits==1 and owner.terminal.returncode==0
    assert owner.terminal.violations==('drain',)


@pytest.mark.parametrize('fds',[None,[],(True,),(2,),(-1,),(1<<20,),(9,9),(3,4,5)])
def test_bad_descriptor_shape_precedes_spawn(modules,monkeypatch,tmp_path,fds):
    module,_=modules
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,pass_fds=fds)
    with pytest.raises(module.BootstrapError): run()
    assert calls==[] and owner.terminal is None


def test_exact_original_descriptor_pins_and_foreign_reuse(modules,monkeypatch,tmp_path):
    module,_=modules
    path=tmp_path/'input';path.write_bytes(b'input')
    fd=os.open(path,os.O_RDONLY|os.O_CLOEXEC)
    other=tmp_path/'other';other.write_bytes(b'foreign')
    second=os.open(other,os.O_RDONLY|os.O_CLOEXEC)
    try:
        process,calls,owner,run=bounded(module,monkeypatch,tmp_path,pass_fds=(fd,))
        run()
        pin=owner.terminal.descriptors[0]
        assert calls[0][1]['pass_fds']==(fd,) and pin.inode==os.fstat(fd).st_ino
        assert not os.get_inheritable(fd)
        class Reused(Process):
            def poll(self): os.dup2(second,fd,inheritable=False);return None
        process,calls,owner,run=bounded(module,monkeypatch,tmp_path,Reused(),pass_fds=(fd,))
        with pytest.raises(module.BootstrapError,match='descriptors changed'): run()
        assert process.waits==1 and owner.terminal.returncode==0
        assert os.fstat(fd).st_ino==os.fstat(second).st_ino
    finally: os.close(fd);os.close(second)


class Operation:
    def __init__(self,module,root,fail_verify=False):
        self.module=module;self.root=root;self.prepares=0;self.closed=False;self.fail_verify=fail_verify
        self.finished=[]
        self.path=root/'launch.json';self.path.write_bytes(b'private launch input');self.path.chmod(0o600)
        self.fd=os.open(self.path,os.O_RDONLY|os.O_CLOEXEC)
        self.seed,self.writer=os.pipe();os.set_blocking(self.seed,False)
        os.write(self.writer,b'a'*64);os.close(self.writer)
        self.evidence=root/'evidence';self.evidence.mkdir(mode=0o700)
        source=root/'sources';python=Path('/fixed/python')
        digest=hashlib.sha256(self.path.read_bytes()).hexdigest()
        argv=(str(python),'-I','-B','-S',str(source/'scripts/nexus/run_multilane_scaling_gate.py'),
            '--launch-input-fd',str(self.fd),'--launch-input-sha256',digest,'--seed-fd',str(self.seed))
        started=int(time.monotonic()*1_000_000_000)
        self.launch=module.FixedScalingLaunch(python,source,argv,self.fd,digest,self.seed,root,{},
            60,4096,self.evidence,4096,4096,started,started+60_000_000_000)
    def prepare(self): self.prepares+=1;return self.launch
    def validate(self,launch):
        assert launch is self.launch and not self.closed
        os.fstat(self.fd);os.fstat(self.seed)
    def verify_publication(self,launch,observation):
        self.validate(launch)
        assert observation.command.returncode==0 and len(observation.artifacts)==2
        if self.fail_verify: raise self.module.BootstrapError('publication rejected')
    def child_finished(self,launch,observation):
        assert launch is self.launch and not self.closed
        self.finished.append(observation)
    def close(self):
        if not self.closed: os.close(self.fd);os.close(self.seed);self.closed=True


def run_handoff(module,helper,monkeypatch,tmp_path,*,mutate_request=None,code=0,fail_verify=False,wait_interrupt=False):
    operation=Operation(module,tmp_path,fail_verify)
    handoff=module.FixedScalingHandoff('1'*64,operation)
    calls=[];client_result=[];client_error=[];runner_holder=[]
    def client(fd):
        try:
            if mutate_request is None:
                client_result.append(helper.exchange(fd,handoff.invocation_sha256,handoff.challenge))
            else:
                endpoint=socket.socket(fileno=fd)
                value={'operation':'fixed-scaling','invocation_sha256':handoff.invocation_sha256,
                    'challenge':handoff.challenge}
                raw=mutate_request(value)
                endpoint.sendall(raw);endpoint.shutdown(socket.SHUT_WR)
                result=helper._decode_response(helper._receive(endpoint,30),handoff.invocation_sha256,handoff.challenge)
                client_result.append(result);endpoint.close()
        except BaseException as error: client_error.append(error)
    class Runner:
        pid=47002
        def __init__(self,descriptor):
            self.thread=threading.Thread(target=client,args=(os.dup(descriptor),));self.waits=0
            self.thread.start()
        def poll(self): return 0 if not self.thread.is_alive() else None
        def wait(self):
            self.waits+=1
            if wait_interrupt and self.waits==1: raise KeyboardInterrupt()
            self.thread.join(3);assert not self.thread.is_alive(),'socket handoff stalled'
            return 0
    def popen(argv,**kwargs):
        calls.append((argv,kwargs))
        if argv[0]=='/fixed/runner':
            assert kwargs['pass_fds']==(handoff.runner_descriptor,)
            runner=Runner(kwargs['pass_fds'][0]);runner_holder.append(runner);return runner
        assert tuple(argv)==operation.launch.argv
        assert kwargs['pass_fds']==(operation.fd,operation.seed)
        assert not operation.closed and os.read(operation.seed,65)==b'a'*64 and os.read(operation.seed,1)==b''
        for name,raw in (('manifest.json',b'{"manifest":"observed"}\n'),('report.json',b'{"report":"observed"}\n')):
            path=operation.evidence/name;path.write_bytes(raw);path.chmod(0o600)
        return Process(code,b'collector diagnostics')
    monkeypatch.setattr(module.subprocess,'Popen',popen)
    error=None
    try:
        result=module._run_release_runner(Path('/fixed/runner'),(),cwd=tmp_path,environment={},
            stdout_descriptor=1,stderr_descriptor=2,scaling_handoff=handoff)
    except BaseException as caught: error=caught
    return operation,handoff,calls,client_result,client_error,runner_holder,error


def test_one_original_handoff_observation_and_retained_artifact_join(modules,monkeypatch,tmp_path):
    module,helper=modules
    operation,owner,calls,replies,errors,runners,error=run_handoff(module,helper,monkeypatch,tmp_path)
    try:
        assert error is None and errors==[] and len(calls)==2 and operation.prepares==1
        assert operation.closed and replies[0]['gate_status']==0
        observation=owner.revalidate_observation()
        assert observation is owner.observation and observation.command is owner.command_observation
        assert observation.command.returncode==0 and runners[0].waits==1
        assert replies[0]['manifest_sha256']==hashlib.sha256((operation.evidence/'manifest.json').read_bytes()).hexdigest()
        copied=owner.response;copied['gate_status']=99
        assert owner.response['gate_status']==0
        with pytest.raises(module.BootstrapError): owner.runner_descriptor
        (operation.evidence/'report.json').write_bytes(b'changed')
        with pytest.raises(module.BootstrapError): owner.revalidate_observation()
    finally: owner.close()


@pytest.mark.parametrize('code',[1,2,130,-15,23])
def test_actual_nonzero_exits_never_become_success(modules,monkeypatch,tmp_path,code):
    module,helper=modules
    operation,owner,calls,replies,errors,runners,error=run_handoff(module,helper,monkeypatch,tmp_path,code=code)
    try:
        assert isinstance(error,module.BootstrapError) and owner.command_observation.returncode==code
        assert owner.observation is None and replies[0]['process_returncode']==code
        assert replies[0]['gate_status']==2 and replies[0]['report_sha256'] is None
        assert operation.closed and runners[0].waits>=1 and len(calls)==2
    finally: owner.close()


def test_verification_failure_cannot_supply_success_and_runner_still_waits(modules,monkeypatch,tmp_path):
    module,helper=modules
    operation,owner,calls,replies,errors,runners,error=run_handoff(module,helper,monkeypatch,tmp_path,fail_verify=True,wait_interrupt=True)
    try:
        assert error is not None and owner.command_observation.returncode==0 and owner.observation is None
        assert replies[0]['gate_status']==2 and operation.closed and runners[0].waits>=2
    finally: owner.close()


def frame(value):
    raw=(json.dumps(value,sort_keys=True,separators=(',',':'))+'\n').encode()
    return len(raw).to_bytes(4,'big')+raw


@pytest.mark.parametrize('mutation',[
    lambda value:frame({**value,'command':'arbitrary'}),
    lambda value:frame({**value,'challenge':'0'*64}),
    lambda value:frame({**value,'invocation_sha256':'0'*64}),
    lambda value:frame(value)+frame(value),
    lambda value:(4097).to_bytes(4,'big')+b'x',
    lambda value:b'\0\0\0\0',
    lambda value:(2).to_bytes(4,'big')+b'{}',
    lambda value:(len(frame(value))-4+1).to_bytes(4,'big')+frame(value)[4:],
])
def test_bad_or_duplicate_request_never_spawns_collector(modules,monkeypatch,tmp_path,mutation):
    module,helper=modules
    operation,owner,calls,replies,errors,runners,error=run_handoff(module,helper,monkeypatch,tmp_path,mutate_request=mutation)
    try:
        assert error is not None and operation.prepares==0 and len(calls)==1
        assert owner.command_observation is None and owner.observation is None and operation.closed
        assert replies[0]['gate_status']==2
    finally: owner.close()


def test_frame_deadline_starts_at_first_byte_and_does_not_renew(modules,monkeypatch,tmp_path):
    module,_=modules;operation=Operation(module,tmp_path)
    owner=module.FixedScalingHandoff('1'*64,operation,frame_timeout_seconds=2)
    client=socket.socket(fileno=os.dup(owner.runner_descriptor))
    process=types.SimpleNamespace(poll=lambda:0)
    owner._started(process)
    clock=[1_000_000.0];monkeypatch.setattr(module.time,'monotonic',lambda:clock[0])
    try:
        owner._service();assert owner._frame_end is None and owner._phase=='waiting'
        client.sendall(b'\0');owner._service();assert owner._frame_end==1_000_002.0
        clock[0]+=1;client.sendall(b'\0');owner._service();assert owner._frame_end==1_000_002.0
        clock[0]+=2;owner._service();assert owner._phase=='failed' and operation.prepares==0
    finally:
        client.close();owner._runner_waited(process,0);owner.close()


@pytest.mark.parametrize('changes',[
    {'argv':('/bin/sh','-c','echo bad')}, {'launch_input_fd':True},
    {'timeout_seconds':True},{'manifest_max_bytes':0},{'source_root':Path('relative')},
])
def test_launch_is_fixed_and_bounded(modules,tmp_path,changes):
    module,_=modules;operation=Operation(module,tmp_path)
    try:
        with pytest.raises(module.BootstrapError): module._scaling_launch_check(dataclasses.replace(operation.launch,**changes))
    finally: operation.close()


@pytest.mark.parametrize('argv',[
    [],['--trial-command','bad'],['--gate-fd','3','--gate-fd','4'],
    ['--gate-fd','2','--invocation-sha256','1'*64,'--challenge','2'*64],
])
def test_helper_cli_rejects_old_or_duplicate_inputs(modules,argv):
    _,helper=modules
    with pytest.raises(SystemExit): helper.parse_args(argv)


def test_helper_accepts_only_exact_fixed_fields(modules):
    _,helper=modules
    args=helper.parse_args(['--gate-fd','3','--invocation-sha256','1'*64,'--challenge','2'*64])
    assert args.gate_fd==3 and args.frame_timeout_seconds==30
    success={'operation':'fixed-scaling','invocation_sha256':'1'*64,'challenge':'2'*64,
        'gate_status':0,'process_returncode':0,'manifest_sha256':'3'*64,'report_sha256':'4'*64}
    assert helper._decode_response(helper._canonical(success),'1'*64,'2'*64)==success
    for key,value in (('gate_status',True),('process_returncode',True),('manifest_sha256',None)):
        with pytest.raises(helper.HandoffError): helper._decode_response(helper._canonical({**success,key:value}),'1'*64,'2'*64)


def test_parent_allocated_root_retains_identity_and_never_deletes_output(modules,tmp_path):
    module,_=modules
    owner=module.allocate_release_invocation_root(tmp_path/'candidate',tmp_path/'bootstrap',tmp_path/'cargo')
    root=owner.path
    try:
        assert root.parent==owner.base and root.name.startswith('iroha-sumeragi-v2-release.')
        assert stat.S_IMODE(root.stat().st_mode)==0o700 and not os.get_inheritable(owner.descriptor)
        (root/'retained').write_text('retained output')
        owner.validate();owner.close()
        assert (root/'retained').read_text()=='retained output'
        with pytest.raises(module.BootstrapError): owner.validate()
    finally:
        owner.close();(root/'retained').unlink(missing_ok=True);root.rmdir()


def test_invocation_root_rejects_nonsticky_or_overlap(modules,tmp_path):
    module,_=modules
    with pytest.raises(module.BootstrapError):
        module.allocate_release_invocation_root(tmp_path/'candidate',tmp_path/'bootstrap',tmp_path/'cargo',base=tmp_path)
    sticky=Path('/tmp').resolve()
    with pytest.raises(module.BootstrapError):
        module.allocate_release_invocation_root(sticky,tmp_path/'bootstrap',tmp_path/'cargo',base=sticky)


def test_retained_existing_runner_assertions_still_hold(modules,monkeypatch,tmp_path):
    # Preserve the exact meaningful assertions of the existing process-double
    # natural-wait test; the full original test files are unchanged contexts.
    module,_=modules;spawned=[];completed=[]
    class FakeProcess:
        def __init__(self,_argv,**kwargs): spawned.append(kwargs)
        def wait(self): completed.append(True);return 23
    monkeypatch.setattr(module.subprocess,'Popen',FakeProcess)
    result=module._run_release_runner(tmp_path/'runner',(),cwd=tmp_path,environment={},stdout_descriptor=1,stderr_descriptor=2)
    assert result.returncode==23
    assert completed==[True]
    assert len(spawned)==1
    assert 'start_new_session' not in spawned[0]
    source=(ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_text()
    assert '--runner-timeout-seconds' not in source
    assert '_MAX_RUNNER_OUTPUT_BYTES' not in source
    assert 'runner = _run_release_runner(' in source
    runner=source[source.index('def _run_release_runner('):source.index('def _open_runner_log(')]
    assert 'subprocess.PIPE' not in runner
    assert 'selector' not in runner
    assert 'stdout=stdout_descriptor' in runner
    assert 'stderr=stderr_descriptor' in runner


def test_original_deadline_never_renews_at_command_start(modules,monkeypatch,tmp_path):
    module,_=modules
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,Process(),deadline_ns=11_000_000_000)
    times=iter((10.0,10.25,11.5))
    monkeypatch.setattr(module.time,'monotonic',lambda:next(times))
    with pytest.raises(module.BootstrapError,match='bounded runtime'): run()
    assert process.waits==1 and owner.terminal.returncode==0
    assert owner.terminal.started_ns==10_000_000_000
    assert owner.terminal.deadline_ns==11_000_000_000
    assert owner.terminal.violations==('runtime',)


def test_original_deadline_expired_during_preflight_cannot_spawn(modules,monkeypatch,tmp_path):
    module,_=modules
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,deadline_ns=11_000_000_000)
    times=iter((10.0,11.0));monkeypatch.setattr(module.time,'monotonic',lambda:next(times))
    with pytest.raises(module.BootstrapError,match='expired before launch'): run()
    assert calls==[] and owner._process is None and owner.terminal is None


def test_thread_construction_and_fallback_errors_still_reap(modules,monkeypatch,tmp_path):
    module,_=modules
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,Process(23))
    def broken(*args,**kwargs): raise RuntimeError('observer allocation failed')
    monkeypatch.setattr(module.threading,'Thread',broken)
    monkeypatch.setattr(module.selectors,'DefaultSelector',broken)
    with pytest.raises(RuntimeError,match='observer allocation failed'): run()
    assert process.waits==1 and owner._reaped and owner.terminal.returncode==23
    assert owner.terminal.violations==('supervision','drain')


def test_terminal_clock_failure_retains_known_actual_exit(modules,monkeypatch,tmp_path):
    module,_=modules
    process,calls,owner,run=bounded(module,monkeypatch,tmp_path,Process(23,b'abc',b'def'))
    times=iter((10.0,));monkeypatch.setattr(module.time,'monotonic',lambda:next(times))
    with pytest.raises(StopIteration): run()
    assert owner._reaped and owner.terminal.returncode==23 and owner.terminal.completed_ns is None
    assert owner.terminal.stdout_sha256==hashlib.sha256(b'abc').hexdigest()
    assert owner.terminal.stderr_bytes==3
    assert owner.terminal.violations==('supervision','terminal_identity_or_clock')


def test_child_finished_none_requires_proved_no_owned_spawn(modules,monkeypatch,tmp_path):
    module,_=modules;operation=Operation(module,tmp_path)
    owner=module.FixedScalingHandoff('1'*64,operation)
    def no_spawn(*args,**kwargs): raise OSError('launch failed')
    monkeypatch.setattr(module.subprocess,'Popen',no_spawn)
    try:
        with pytest.raises(module.BootstrapError,match='could not execute'): owner._execute()
        assert owner._command._process is None and operation.finished==[None]
        owner.close();assert operation.closed
    finally: owner.close()


def test_owned_child_observer_error_releases_with_actual_terminal_only(modules,monkeypatch,tmp_path):
    module,_=modules;operation=Operation(module,tmp_path)
    owner=module.FixedScalingHandoff('1'*64,operation)
    class Interrupted(Process):
        def poll(self): raise KeyboardInterrupt()
    process=Interrupted(23,b'full output')
    monkeypatch.setattr(module.subprocess,'Popen',lambda *args,**kwargs:process)
    try:
        with pytest.raises(KeyboardInterrupt): owner._execute()
        assert process.waits==1 and owner.command_observation.returncode==23
        assert operation.finished==[owner.command_observation] and operation.finished[0] is not None
        owner.close();assert operation.closed
    finally: owner.close()


def test_missing_terminal_after_owned_child_never_means_no_spawn(modules,monkeypatch,tmp_path):
    module,_=modules;operation=Operation(module,tmp_path)
    owner=module.FixedScalingHandoff('1'*64,operation)
    process=Process(23)
    monkeypatch.setattr(module.subprocess,'Popen',lambda *args,**kwargs:process)
    original=owner._command._finish
    def broken(*args,**kwargs): raise RuntimeError('observation storage failed')
    monkeypatch.setattr(owner._command,'_finish',broken)
    try:
        with pytest.raises(RuntimeError,match='observation storage failed'): owner._execute()
        assert process.waits==1 and owner._command._reaped and owner._command._process is process
        assert operation.finished==[] and owner.command_observation is None
        with pytest.raises(module.BootstrapError): owner.close()
        assert not operation.closed
        os.fstat(operation.fd);os.fstat(operation.seed)
    finally:
        # The fixture knows its same original double was reaped. Release only
        # fixture resources; production owner cannot substitute None here.
        operation.close()


def test_root_path_replacement_is_rejected_without_deleting_replacement(modules,tmp_path):
    module,_=modules
    owner=module.allocate_release_invocation_root(tmp_path/'candidate',tmp_path/'bootstrap',tmp_path/'cargo')
    root=owner.path;saved=root.with_name(root.name+'.original')
    try:
        root.rename(saved);root.mkdir(mode=0o700);(root/'foreign').write_bytes(b'foreign')
        with pytest.raises(module.BootstrapError): owner.validate()
        owner.close();assert (root/'foreign').read_bytes()==b'foreign'
    finally:
        owner.close();(root/'foreign').unlink(missing_ok=True);root.rmdir();saved.rmdir()


def test_request_duplicate_keys_are_rejected(modules):
    module,_=modules
    raw=b'{"challenge":"'+b'2'*64+b'","invocation_sha256":"'+b'1'*64+b'","operation":"fixed-scaling","operation":"fixed-scaling"}\n'
    with pytest.raises(module.BootstrapError): module._scaling_decode_request(raw)


def test_poll_terminal_alone_does_not_release_runner_resources(modules,tmp_path):
    module,_=modules;operation=Operation(module,tmp_path)
    owner=module.FixedScalingHandoff('1'*64,operation)
    process=Process(23)
    owner._started(process)
    try:
        assert process.poll()==23 and process.waits==0
        with pytest.raises(module.BootstrapError): owner.close()
        assert not operation.closed
        owner._runner_waited(process,process.wait())
        owner.close();assert operation.closed and process.waits==1
    finally:
        if not owner._runner_reaped: owner._runner_waited(process,process.wait())
        owner.close()
