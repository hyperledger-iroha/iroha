"""Actual fixed lifecycle composition with fake native children and real files."""
from dataclasses import replace
import os
import pytest
from scaling_fixed_trial_fixture import trial_setup, ready_setup
import scaling_fixed_trial as trial


@pytest.mark.parametrize('trial_setup',[1,4],indirect=True)
def test_complete_actual_single_trial_composition(trial_setup):
    c=trial_setup;owner=c.create();result=owner.run('1'*64)
    assert owner.validate() is result
    assert result.variant==c.plan.load.variant
    assert result.resources.signed_requests and len(result.readiness)==4
    assert result.capture_census.files==result.resources.capture_file_count
    assert result.original_deadline_ns==c.kwargs['trial_deadline_ns']
    assert c.factory.operations==['generator']+['launch-peer']*4+['readiness']*4+['load','stopped-tip','vectors','facts','prepare','export','replay']
    assert owner._launch._stopped=={0,1,2,3}
    assert len(owner._proof_plan.observations)==result.load.scheduled_requests==12
    assert 'passed' not in result.__dataclass_fields__
    assert 'release_ready' not in result.__dataclass_fields__
    import fcntl,hashlib
    native=owner._generated.inputs;borrowed=[]
    for role,path in (('collector_journal',c.paths.load.collector_journal),('transaction_trace',c.paths.load.transaction_trace)):
        fd=owner._load.public_descriptor(role)
        assert fcntl.fcntl(fd,fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY
        assert os.fstat(fd).st_ino==path.stat().st_ino
        borrowed.append(os.dup(fd))
    for name in ('genesis.json','genesis.signed.nrt','genesis-context.nrt','genesis.expected_hash','genesis-anchors.json'):
        fd=native.public_descriptor(name)
        assert fcntl.fcntl(fd,fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY
        assert os.fstat(fd).st_ino==(c.paths.generated/name).stat().st_ino
        borrowed.append(os.dup(fd))
    public=[];transferred=owner._captures.transfer(lambda:public.append(True))
    try:
        assert owner.validate() is result
        owner.close();c.c.clock.now=owner.original_deadline_ns+1
        assert transferred.verify()==result.capture_census and public
        assert all(os.fstat(fd).st_size>0 for fd in borrowed)
    finally:
        transferred.close()
        for fd in borrowed:os.close(fd)
    assert c.paths.load.transaction_trace.is_file() and c.paths.load.collector_journal.is_file()
    assert c.paths.generated.is_dir() and c.paths.captures.is_dir()


@pytest.mark.parametrize('operation',['generator','readiness','load','stopped-tip','vectors','facts','prepare','export','replay'])
def test_each_native_nonzero_is_terminal_and_cleanup_retains_original_children(trial_setup,operation):
    c=trial_setup;owner=c.create()
    def after(name,child):
        if name==operation:child.status=17
    c.factory.after=after
    with pytest.raises(trial.FixedTrialError,match='^fixed_trial_failed$'):owner.run('1'*64)
    assert c.factory.operations[-1]==operation and owner._result is None
    assert owner.cleanup(c.c.clock.end(5))==()
    assert all(child.returncode is not None for child in (*c.factory.children,*c.c.daemons.children))
    assert not any(event[0]=='kill' for event in (*c.factory.events,*c.c.daemons.events))
    count=len(c.factory.operations)
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    assert len(c.factory.operations)==count
    owner.close()


@pytest.mark.parametrize('peer',[0,1,2,3])
def test_partial_launch_failure_preserves_cleanup_for_only_original_spawned_peers(trial_setup,peer):
    c=trial_setup;owner=c.create();c.c.daemons.fail_at=peer
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    assert len(c.c.daemons.children)==peer and 'readiness' not in c.factory.operations
    assert owner.cleanup(c.c.clock.end(5))==()
    assert [event[1] for event in c.c.daemons.events if event[0]=='terminate']==list(reversed(range(peer)))
    owner.close()


@pytest.mark.parametrize('operation',['generator','load','vectors','replay'])
def test_no_native_terminal_can_refresh_original_deadline(trial_setup,operation):
    c=trial_setup;owner=c.create()
    def after(name,child):
        if name==operation:c.c.clock.now=c.kwargs['trial_deadline_ns']
    c.factory.after=after
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    assert owner.original_deadline_ns==c.kwargs['trial_deadline_ns']
    assert c.factory.operations[-1]==operation and owner._result is None
    assert owner.cleanup(c.c.clock.end(5))==()
    owner.close()


@pytest.mark.parametrize('peer',[3,0])
def test_only_clean_original_stops_allow_following_native_phase(trial_setup,peer):
    c=trial_setup;owner=c.create()
    c.c.daemons.after_spawn=lambda child:setattr(child,'nonzero',child.index==peer)
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    assert 'stopped-tip' in c.factory.operations if peer==0 else 'stopped-tip' not in c.factory.operations
    assert 'facts' not in c.factory.operations and owner._result is None
    owner.cleanup(c.c.clock.end(5));owner.close()


def test_actual_resource_replay_rejects_global_only_journal_before_reader_stop(trial_setup):
    import hashlib,json
    c=trial_setup;owner=c.create()
    def transform(name,report):
        if name=='load':
            path=c.paths.load.collector_journal
            rows=[json.loads(line) for line in path.read_bytes().splitlines()]
            rows=[row for row in rows if row['event']!='local_status']
            raw=b''.join(json.dumps(row,separators=(',',':')).encode()+b'\n' for row in rows)
            path.write_bytes(raw);report['collector_journal_sha256']=hashlib.sha256(raw).hexdigest();report['collector_journal_bytes']=len(raw)
        return report
    c.factory.transform=transform
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    assert c.factory.operations[-1]=='load' and owner._captures is not None
    assert owner._result is None and not c.c.daemons.events


@pytest.mark.parametrize('kind',['plan','deadline','image','budget'])
def test_original_admission_snapshot_cannot_be_mutated_before_generation(trial_setup,kind):
    c=trial_setup;owner=c.create()
    if kind=='plan':object.__setattr__(c.plan.generator,'chain_id','changed-chain')
    elif kind=='deadline':owner._deadline+=1
    elif kind=='image':object.__setattr__(c.runtime,'kagami',c.runtime.cli)
    else:object.__setattr__(c.kwargs['allocation'].run.canonical_proof,'max_bytes',65537)
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    assert not c.factory.operations and owner._result is None


@pytest.mark.parametrize('kind',['overlap','variant','lane','cap','total','deadline','callback'])
def test_invalid_original_trial_admission_never_spawns_or_leaks(trial_setup,kind):
    c=trial_setup;kwargs={}
    if kind=='overlap':kwargs['paths']=replace(c.paths,runtime_root=c.paths.evidence_root)
    elif kind=='variant':kwargs['paths']=replace(c.paths,variant='one_lane')
    elif kind=='lane':kwargs['plan']=replace(c.plan,generator=replace(c.plan.generator,lane_count=1))
    elif kind=='cap':kwargs['plan']=replace(c.plan,native_outputs=replace(c.plan.native_outputs,proof=65537))
    elif kind=='total':kwargs['plan']=replace(c.plan,native_outputs=replace(c.plan.native_outputs,total=1))
    elif kind=='deadline':kwargs['trial_deadline_ns']=c.c.clock.now
    else:kwargs['verify_runtime']=None
    before=set(os.listdir('/dev/fd'))
    with pytest.raises(trial.FixedTrialError):c.create(**kwargs)
    assert set(os.listdir('/dev/fd'))==before and not c.factory.operations


def test_caught_runtime_reentry_cannot_publish_or_launch(trial_setup):
    c=trial_setup;box=[]
    def callback():
        if box:
            with pytest.raises(trial.FixedTrialError):box[0].run('1'*64)
    owner=c.create(verify_runtime=callback);box.append(owner)
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    assert not c.factory.operations and owner._result is None


def test_cleanup_visits_every_owned_scope_after_interrupt(trial_setup):
    c=trial_setup;owner=c.create();visited=[]
    class Scope:
        def __init__(self,name,interrupt=False):self.name,self.interrupt=name,interrupt
        def cleanup(self,deadline):
            visited.append((self.name,deadline))
            if self.interrupt:raise KeyboardInterrupt('PRIVATE FIXTURE')
            return ()
    owner._proof=Scope('proof',True);owner._load=Scope('load');owner._generator=Scope('generator')
    try:
        with pytest.raises(KeyboardInterrupt):owner.cleanup(c.c.clock.end(5))
        assert [item[0] for item in visited]==['proof','load','generator']
    finally:owner._proof=owner._load=owner._generator=None


def test_close_preserves_reused_foreign_parent_descriptor(trial_setup):
    c=trial_setup;owner=c.create();directory=owner._directories
    root_path=next(iter(directory._rows));fd=directory._rows[root_path][0]
    original=os.dup(fd);foreign=os.open(c.runtime.resource_worker,os.O_RDONLY)
    try:
        os.dup2(foreign,fd)
        with pytest.raises(trial.FixedTrialError):owner.close()
        assert os.fstat(fd).st_ino==os.fstat(foreign).st_ino and len(directory._rows)==1
        os.dup2(original,fd);owner.close()
    finally:os.close(foreign);os.close(original)


def test_close_never_polls_or_closes_a_substituted_retained_native_handle(trial_setup):
    c=trial_setup;owner=c.create()
    def after(name,child):
        if name=='generator':child.wait_hook=lambda:setattr(child,'pid',child.pid+10000)
    c.factory.after=after
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    child=c.factory.children[0];events=list(c.factory.events)
    with pytest.raises(trial.FixedTrialError):owner.close()
    assert c.factory.events==events and owner._generator._namespace is not None
    assert owner.cleanup(c.c.clock.end(5))==('generator:generator',)
    assert c.factory.events==events
    child.pid-=10000;child.wait_hook=lambda:None
    assert owner.cleanup(c.c.clock.end(5))==()
    owner.close()


@pytest.mark.parametrize('kind',['resources','ready','plan','canonical','expired'])
def test_completed_trial_authority_stays_original_and_timed_until_handoff(trial_setup,kind):
    c=trial_setup;owner=c.create();result=owner.run('1'*64)
    if kind=='resources':object.__setattr__(result.resources,'journal_bytes',result.resources.journal_bytes+1)
    elif kind=='ready':object.__setattr__(result.readiness[0],'challenge','e'*64)
    elif kind=='plan':object.__setattr__(owner._proof_plan,'lane_count',1)
    elif kind=='canonical':object.__setattr__(result.canonical,'row_count',result.canonical.row_count+1)
    else:c.c.clock.now=owner.original_deadline_ns
    with pytest.raises(trial.FixedTrialError):owner.validate()
    assert owner._phase=='failed'
    owner.close()


@pytest.mark.parametrize('phase',['finalizing','complete'])
def test_final_custody_has_no_external_callback_after_completed_capture_scan(trial_setup,monkeypatch,phase):
    from scaling_generator import GeneratedInputs
    c=trial_setup;owner=c.create();scanned=[];attempted=[];runtime_checks=[];late_getters=[]
    original_getter=GeneratedInputs.receipt.fget
    def generation_receipt(inputs):
        value=original_getter(inputs)
        if owner._phase in ('finalizing','complete'):
            late_getters.append(True)
            first=sorted(c.paths.captures.iterdir())[0];first.write_bytes(first.read_bytes())
        return value
    monkeypatch.setattr(GeneratedInputs,'receipt',property(generation_receipt))
    original_scan=trial.TrialCaptures.verify
    def verify(scope):
        result=original_scan(scope)
        if owner._phase==phase:scanned.append(True)
        return result
    monkeypatch.setattr(trial.TrialCaptures,'verify',verify)
    def callback():
        runtime_checks.append(owner._phase)
        if owner._phase==phase and scanned:
            attempted.append(True)
            first=sorted(c.paths.captures.iterdir())[0];first.write_bytes(first.read_bytes())
    owner._callback=callback
    result=owner.run('1'*64)
    if phase=='complete':assert owner.validate() is result
    assert scanned and phase in runtime_checks and not attempted and not late_getters
    assert not owner._closing_custody
    owner.close()


@pytest.mark.parametrize('phase',['finalizing','complete'])
def test_mutation_by_required_final_runtime_callback_fails_before_receipt(trial_setup,phase):
    c=trial_setup;owner=c.create();mutated=[]
    def callback():
        if owner._phase==phase and not mutated:
            mutated.append(True)
            first=sorted(c.paths.captures.iterdir())[0];first.write_bytes(first.read_bytes())
    owner._callback=callback
    if phase=='finalizing':
        with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
        assert owner._result is None
    else:
        owner.run('1'*64)
        with pytest.raises(trial.FixedTrialError):owner.validate()
    assert mutated and owner._phase=='failed' and not owner._closing_custody


def test_native_generation_receipt_cannot_drift_during_later_phase(trial_setup):
    c=trial_setup;owner=c.create();mutated=[]
    def callback():
        if owner._phase=='finalizing' and not mutated:
            mutated.append(True)
            receipt=owner._generated._receipt
            object.__setattr__(receipt,'anchors_bytes',receipt.anchors_bytes+1)
    owner._callback=callback
    with pytest.raises(trial.FixedTrialError):owner.run('1'*64)
    assert mutated and owner._result is None
