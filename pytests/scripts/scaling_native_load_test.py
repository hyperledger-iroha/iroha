"""Actual owners, fake native children and test-owned output bytes; no measurements."""
from dataclasses import fields, replace
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace

import pytest
from scaling_readiness_fixture import ready_setup, CommandFactory, PipeChild, compact
import scaling_command as command
import scaling_native_load as load
import resource_process as process
import resource_evidence_budget as budget
import resource_probe_worker as worker_protocol


def allocation():
    geometry=budget.CaptureGeometry(4,budget.NS,20*budget.NS,budget.NS)
    runs=[]
    for pair in range(1,6):
        for variant in ('one_lane','four_lane'):
            prefix=f'pair{pair}.{variant}'
            caps={name:budget.FileBudget(prefix+'.'+name,budget.MIB)
                  for name in budget.RUN_FILE_FIELDS}
            runs.append(budget.RunBudget(pair,variant,geometry,**caps))
    experiment=budget.admit_experiment(policy=budget.CapturePolicy(),runs=tuple(runs),
        static_files=(budget.StaticFile('software',budget.MIB),),
        manifest=budget.FileBudget('manifest',budget.MIB),report=budget.FileBudget('report',budget.MIB),other_control=())
    return budget.select_run_budget(experiment,1,'four_lane')


def plan():
    return load.NativeLoadPlan(1,'four_lane','b'*64,'2',0,20*budget.NS,budget.NS,125_000_000,
        64,4,1000,64,64,16,50,256,1000,400,100)


class LoadFactory(CommandFactory):
    """Transport-only sample data deliberately does not pass evidence replay."""
    def __init__(self,c):
        super().__init__(c.clock,c.anchors)
        self.change=lambda value:value
        self.outputs=lambda argv:None
        self.raw_override=None
        self.before_reply=lambda argv:None
    def __call__(self,argv,**kwargs):
        index=len(self.calls);self.calls.append((argv,kwargs))
        if self.fail_spawn==index:raise OSError('PRIVATE INPUT SECRET')
        value=lambda key:argv[argv.index('--'+key)+1]
        trace,journal=Path(value('trace-out')),Path(value('diagnostic-out'))
        for path,raw in ((trace,b'{"mock_transport_trace":true}\n'),(journal,b'{"mock_transport_journal":true}\n')):
            path.write_bytes(raw);path.chmod(0o600)
        Path(value('resource-capture-dir')).mkdir(mode=0o700)
        report=dict(version=1,operation='transaction_load',invocation_id=value('invocation-id'),
            pair_index=int(value('pair-index')),variant=value('variant'),seed=value('seed'),
            resource_budget_sha256=value('resource-budget-sha256'),scheduled_requests=40)
        for prefix,path in (('trace',trace),('collector_journal',journal)):
            raw=path.read_bytes();report[prefix+'_sha256']=hashlib.sha256(raw).hexdigest();report[prefix+'_bytes']=len(raw)
        self.outputs(argv);self.before_reply(argv)
        report=self.change(report)
        raw=self.raw_override if self.raw_override is not None else compact(report)
        child=PipeChild(self,index,raw,self.diagnostics.get(index,b''));self.children.append(child)
        self.after_spawn(child);return child


@pytest.fixture
def load_setup(ready_setup,monkeypatch):
    c=ready_setup
    pins=c.run.launch_owned();c.run.await_genesis_ready(c.step)
    factory=LoadFactory(c)
    monkeypatch.setattr(command.subprocess,'Popen',factory)
    python_path=c.root/'python3';python_path.write_bytes(c.images[1].path.read_bytes());python_path.chmod(0o700)
    program=process.ExecutableImage(python_path,hashlib.sha256(python_path.read_bytes()).hexdigest())
    worker_dir=c.root/'worker';worker_dir.mkdir(mode=0o700)
    worker=worker_dir/'resource_probe_worker.py'
    worker.write_bytes((Path(__file__).resolve().parents[2]/'scripts/nexus/resource_probe_worker.py').read_bytes());worker.chmod(0o644)
    runtime=c.root/'runtime';runtime.mkdir(mode=0o700)
    evidence=c.root/'evidence';evidence.mkdir(mode=0o700)
    paths=load.LoadPaths(runtime/'worker-config.json',evidence/'captures',evidence/'trace.json',evidence/'journal.jsonl')
    owners=[];checks=[]
    def live():
        checks.append('live')
        for index,pin in enumerate(pins):
            child=c.run._child_handle(index)
            assert child.poll() is None and pin.sample().identity==pin.identity
    def runtime_check():checks.append('runtime')
    kwargs=dict(plan=plan(),inputs=c.inputs,peers=pins,allocation=allocation(),paths=paths,
        cli=c.images[1],reader=factory,resource_program=program,resource_worker=worker,
        resource_worker_sha256=hashlib.sha256(worker.read_bytes()).hexdigest(),trial_deadline_ns=c.run._trial_deadline_ns,
        verify_runtime=runtime_check,verify_all_peers_live=live)
    def create(**changes):
        owner=load.FixedNativeLoad(**(kwargs|changes));owners.append(owner);return owner
    yield SimpleNamespace(c=c,factory=factory,program=program,worker=worker,paths=paths,
        kwargs=kwargs,owners=owners,checks=checks,create=create)
    for owner in owners:
        try:owner.cleanup(c.clock.end(10));owner.close()
        except load.NativeLoadError:pass
    for child in factory.children:child.close()
    program.close()


def rejected(operation):
    with pytest.raises(load.NativeLoadError,match='^native_load_failed$'):operation()


def test_actual_fixed_command_worker_budget_original_peers_and_output_custody(load_setup):
    s=load_setup;owner=s.create()
    receipt=s.c.run.run_load(lambda pins:owner.run())
    assert receipt.scheduled_requests==40 and receipt.peers==tuple(pin.identity for pin in s.kwargs['peers'])
    assert owner.validate() is receipt
    assert not {'throughput','latency','memory_peak'} & {f.name for f in fields(receipt)}
    argv,spawn=s.factory.calls[0]
    assert argv[:12]==(str(s.c.images[1].path),'--machine','--config-fd',str(s.c.inputs.client_fd(0)),
        '--config-source-path',str(s.c.inputs.roles[0].client_config),'--output-format','json','--fee-payer','authority','tx','load')
    assert spawn['pass_fds']==(s.c.inputs.client_fd(0),) and spawn['shell'] is False
    assert '--local-observer-config' in argv and argv[argv.index('--local-observer-config')+1]==str(s.c.inputs.roles[3].client_config)
    assert tuple(argv[i+1] for i,item in enumerate(argv) if item=='--account-config')==tuple(str(s.c.directory/a.config) for a in s.c.inputs.generation.accounts)
    assert '--account-config-fd' not in argv and '--local-observer-config-fd' not in argv
    assert argv[argv.index('--resource-budget-sha256')+1]==budget.run_budget_sha256(s.kwargs['allocation'])
    config=json.loads(s.paths.resource_config.read_bytes())
    assert worker_protocol.read_config(s.paths.resource_config)==config
    assert budget.run_budget_sha256(worker_protocol._config_allocation(config))==receipt.budget_sha256
    assert config['resource_budget']==budget.run_budget_inputs(s.kwargs['allocation'])
    assert [(p['peer_id'],p['pid'],p['executable_path'],p['executable_sha256'],p['endpoint'],p['headers']) for p in config['peers']]==[
        (pin.peer_id,pin.pid,str(pin.image.path),pin.identity.executable_sha256,role.torii_url,{})
        for pin,role in zip(s.kwargs['peers'],s.c.inputs.roles,strict=True)]
    assert s.checks.count('live')>4 and s.checks.count('runtime')>4
    assert receipt.transaction_trace.sha256==hashlib.sha256(s.paths.transaction_trace.read_bytes()).hexdigest()
    assert receipt.collector_journal.label==s.kwargs['allocation'].journal.label
    assert receipt.process.pid==2000 and s.factory.children[0].returncode==0
    for child in s.c.daemons.children:child.status=0
    assert owner.validate() is receipt  # Custody remains valid after original peers stop.
    owner.close()
    assert all(path.exists() for path in (s.paths.resource_config,s.paths.transaction_trace,s.paths.collector_journal,s.paths.resource_capture_dir))
    assert s.c.inputs.client_fd(0)>=0 and s.c.images[1].validate() is None


@pytest.mark.parametrize('change',[
    {'version':True},{'version':2},{'operation':'other'},{'invocation_id':'c'*64},
    {'pair_index':True},{'pair_index':2},{'variant':'one_lane'},{'seed':'c'*64},
    {'resource_budget_sha256':'c'*64},{'scheduled_requests':39},{'scheduled_requests':True},
    {'trace_sha256':'0'*64},{'collector_journal_sha256':'0'*64},
    {'trace_bytes':0},{'trace_bytes':1},{'trace_bytes':True},{'trace_bytes':budget.MIB+1},
    {'collector_journal_bytes':0},{'collector_journal_bytes':1},{'collector_journal_bytes':budget.MIB+1},
    {'trace_sha256':'A'*64},{'extra':True},
])
def test_terminal_identity_schema_counts_and_allocations_are_exact(load_setup,change):
    s=load_setup;s.factory.change=lambda value:value|change
    owner=s.create();rejected(owner.run);rejected(owner.validate)
    assert s.factory.children[0].returncode==0  # Zero exit alone cannot mint receipt.


@pytest.mark.parametrize('raw',[b'{}',b'{}\n\n',b'{}\n{}\n',b'PRIVATE INPUT SECRET\n',
    b'{"version":1,"version":1}\n',b'{"version":NaN}\n',b'{"version":1.0}\n',b'{"version":-1}\n',b'{"version":100000000000}\n',b'\xff\n'])
def test_malformed_reply_redacted_and_terminal(load_setup,raw):
    s=load_setup;s.factory.raw_override=raw
    owner=s.create();rejected(owner.run)


@pytest.mark.parametrize('field,value',[
    ('pair_index',True),('pair_index',0),('variant','old'),('seed','B'*64),('offered_load_tps','1e2'),
    ('offered_load_tps','0'),('offered_load_tps','0.21'),('warmup_ns',budget.NS),('measurement_ns',21*budget.NS),
    ('drain_ns',0),('submission_lag_ns',125_000_001),('preparation_lookahead',0),('preparation_concurrency',33),
    ('preparation_ahead_ms',30001),('max_submissions',4097),('max_in_flight',16385),('max_status_requests',257),
    ('poll_interval_ms',10001),('journal_capacity',16385),('resource_interval_ms',1001),
    ('resource_timeout_ms',501),('resource_max_start_lag_ms',251)])
def test_plan_limits_before_child_or_writer_admission(load_setup,field,value):
    s=load_setup;rejected(lambda:s.create(plan=replace(s.kwargs['plan'],**{field:value})))
    assert not s.factory.calls and not s.paths.resource_config.exists()


@pytest.mark.parametrize('kind',['trace','journal','stage','capture','config'])
def test_existing_output_or_control_is_never_overwritten(load_setup,kind):
    s=load_setup
    target={'trace':s.paths.transaction_trace,'journal':s.paths.collector_journal,
        'stage':s.paths.transaction_trace.with_name('trace.json.collecting'),'capture':s.paths.resource_capture_dir,
        'config':s.paths.resource_config}[kind]
    target.write_bytes(b'original');target.chmod(0o600)
    rejected(s.create)
    assert target.read_bytes()==b'original' and not s.factory.calls


@pytest.mark.parametrize('kind',['trace','journal','stage','capture','symlink','hardlink','mode'])
def test_terminal_file_receipt_does_not_accept_missing_or_substituted_outputs(load_setup,kind):
    s=load_setup
    def mutate(argv):
        if kind=='trace':s.paths.transaction_trace.unlink()
        elif kind=='journal':s.paths.collector_journal.write_bytes(b'changed')
        elif kind=='stage':s.paths.transaction_trace.with_name('trace.json.collecting').write_bytes(b'stage')
        elif kind=='capture':s.paths.resource_capture_dir.rmdir()
        elif kind=='mode':s.paths.collector_journal.chmod(0o644)
        else:
            original=s.paths.transaction_trace;other=original.with_name('other');original.rename(other)
            if kind=='symlink':original.symlink_to(other)
            else:os.link(other,original)
    s.factory.outputs=mutate;owner=s.create();rejected(owner.run)


@pytest.mark.parametrize('kind',['config','worker','account','local','cli','program','peer','deadline','plan','paths','invocation'])
def test_original_inputs_and_runtime_identity_cannot_drift_after_admission(load_setup,kind):
    s=load_setup;owner=s.create()
    if kind=='config':s.paths.resource_config.write_bytes(b'changed')
    elif kind=='worker':s.worker.write_bytes(b'changed')
    elif kind=='account':(s.c.directory/s.c.inputs.generation.accounts[0].config).write_bytes(b'changed')
    elif kind=='local':s.c.inputs.roles[3].client_config.write_bytes(b'changed')
    elif kind=='cli':s.c.images[1].path.write_bytes(b'changed')
    elif kind=='program':s.program.path.write_bytes(b'changed')
    elif kind=='peer':s.c.daemons.children[3].drift=True
    elif kind=='deadline':owner._end+=1
    elif kind=='plan':object.__setattr__(owner._plan,'max_status_requests',17)
    elif kind=='paths':object.__setattr__(s.paths,'collector_journal',s.paths.collector_journal.with_name('other'))
    else:owner._invocation='c'*64
    rejected(owner.run);assert not s.factory.calls


@pytest.mark.parametrize('kind',['trace','journal','config','worker','capture','ancestor','receipt'])
def test_completed_receipt_retains_original_files_and_poisoning(load_setup,kind):
    s=load_setup;owner=s.create();receipt=owner.run()
    if kind in ('trace','journal','config','worker'):
        path={'trace':s.paths.transaction_trace,'journal':s.paths.collector_journal,'config':s.paths.resource_config,'worker':s.worker}[kind]
        raw=path.read_bytes();path.write_bytes(raw)  # Even same bytes on changed original metadata is rejected.
    elif kind=='capture':
        s.paths.resource_capture_dir.rename(s.paths.resource_capture_dir.with_name('old'));s.paths.resource_capture_dir.mkdir(mode=0o700)
    elif kind=='ancestor':
        parent=s.paths.transaction_trace.parent;parent.rename(parent.with_name('old'));parent.mkdir(mode=0o700)
    else:object.__setattr__(receipt,'scheduled_requests',41)
    rejected(owner.validate);rejected(owner.validate)


def test_live_callback_late_peer_failure_prevents_receipt(load_setup):
    s=load_setup;owner=s.create()
    s.factory.after_spawn=lambda child:setattr(s.c.daemons.children[3],'drift',True)
    rejected(owner.run)
    assert owner.cleanup(s.c.clock.end(10))==()


@pytest.mark.parametrize('kind',['nonzero','spawn','pin','cancel','substituted'])
def test_child_failure_cleanup_keeps_original_ownership(load_setup,kind):
    s=load_setup;owner=s.create()
    if kind=='nonzero':s.factory.after_spawn=lambda child:setattr(child,'status',7)
    elif kind=='spawn':s.factory.fail_spawn=0
    elif kind=='pin':s.factory.fail_pin=True
    elif kind=='cancel':
        def cancel(child):raise KeyboardInterrupt()
        s.factory.before_reply=lambda argv:(_ for _ in ()).throw(KeyboardInterrupt())
    else:s.factory.after_spawn=lambda child:setattr(child,'wait_hook',lambda:setattr(child,'pid',9999))
    if kind=='cancel':
        with pytest.raises(KeyboardInterrupt):owner.run()
    else:rejected(owner.run)
    pending=owner.cleanup(s.c.clock.end(10))
    if kind=='substituted':
        assert pending==('native-load',) and not any(row[0]=='terminate' for row in s.factory.events)
        rejected(owner.close)
        s.factory.children[0].pid=2000
    else:assert pending==()
    assert s.paths.resource_config.exists()


def test_single_use_and_exact_original_deadline(load_setup):
    s=load_setup;owner=s.create();end=owner.trial_deadline_ns
    owner.run();assert owner.trial_deadline_ns==end
    rejected(owner.run)


def test_full_native_lifetime_must_fit_remaining_original_deadline(load_setup):
    s=load_setup;rejected(lambda:s.create(trial_deadline_ns=s.c.clock.end(22)))
    assert not s.factory.calls and not s.paths.resource_config.exists()


@pytest.mark.parametrize('value,units,expected',[(0,10**9,'0'),(1,10**9,'0.000000001'),
    (125_000_000,10**6,'125'),(125_000_001,10**6,'125.000001'),(10**9,10**9,'1')])
def test_exact_decimal_timing_has_no_float_rounding(value,units,expected):
    assert load.decimal_ns(value,units)==expected


def test_callback_reentry_cannot_resume_outer_load(load_setup):
    s=load_setup;owner=s.create();attempts=[]
    def runtime():
        if owner._phase=='running' and not attempts:
            attempts.append(True)
            rejected(owner.run)
    owner._runtime=runtime
    rejected(owner.run)
    assert attempts and not s.factory.calls


def test_output_identity_rechecked_after_later_file_read(load_setup,monkeypatch):
    s=load_setup;owner=s.create();original=os.pread;changed=[]
    def read(fd,size,offset):
        raw=original(fd,size,offset)
        if raw.startswith(b'{"mock_transport_journal"') and not changed:
            changed.append(True)
            s.paths.transaction_trace.write_bytes(b'changed during later file read')
        return raw
    monkeypatch.setattr(os,'pread',read)
    rejected(owner.run)
    assert changed and s.paths.transaction_trace.read_bytes()==b'changed during later file read'


def test_same_call_output_parent_swap_rejected_after_runtime_callback(load_setup):
    s=load_setup;owner=s.create();changed=[]
    def runtime():
        if owner._files._complete and not changed:
            changed.append(True)
            parent=s.paths.transaction_trace.parent
            parent.rename(parent.with_name('detached'));parent.mkdir(mode=0o700)
    owner._runtime=runtime
    rejected(owner.run)
    assert changed and (s.paths.transaction_trace.parent.with_name('detached')/'trace.json').exists()


@pytest.mark.parametrize('kind',['partial','duplicate','wrong-role','wrong-image','wrong-pid'])
def test_exact_four_original_process_pins_required_before_writer(load_setup,kind):
    s=load_setup;pins=s.kwargs['peers']
    if kind=='partial':pins=pins[:3]
    elif kind=='duplicate':pins=(pins[0],pins[0],pins[2],pins[3])
    elif kind=='wrong-role':pins=(pins[1],pins[0],pins[2],pins[3])
    elif kind=='wrong-image':pins[3].image=s.c.images[1]
    else:pins[3].pid=9999
    rejected(lambda:s.create(peers=pins))
    assert not s.factory.calls and not s.paths.resource_config.exists()


def test_receipt_output_overrun_is_terminal_even_if_child_exits_zero(load_setup):
    s=load_setup;s.factory.raw_override=b'{"padding":"'+b'x'*load.MAX_REPLY_BYTES+b'"}\n'
    owner=s.create();rejected(owner.run)
    assert owner.cleanup(s.c.clock.end(10))==()


def test_private_file_admission_failure_closes_all_new_descriptors(load_setup):
    s=load_setup
    s.paths.resource_config.parent.chmod(0o755)
    before=set(os.listdir('/dev/fd'))
    rejected(s.create)
    assert set(os.listdir('/dev/fd'))==before
    assert not s.paths.resource_config.exists()


@pytest.mark.parametrize('rate,warmup,measurement,accounts,expected',[
    ('2',0,20*budget.NS,4,40),('0.2',0,20*budget.NS,4,4),
    ('3.2',0,20*budget.NS,64,64),('2',2*budget.NS,20*budget.NS,4,44),
])
def test_exact_rational_original_offer_counts(rate,warmup,measurement,accounts,expected):
    candidate=replace(plan(),offered_load_tps=rate,warmup_ns=warmup,measurement_ns=measurement,submission_lag_ns=0)
    assert candidate.validate(accounts,allocation())[0]==expected
