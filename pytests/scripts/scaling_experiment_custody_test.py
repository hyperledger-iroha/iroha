"""Actual fixed owners with fake native pipes, real files and no native execution."""
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import sys
from types import SimpleNamespace

import pytest

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts/nexus'))
sys.path.insert(0,str(ROOT/'scripts/tests'))
from scaling_fixed_trial_fixture import trial_setup, ready_setup, plan as fixture_plan, allocation
import resource_evidence_budget as budgets
import scaling_experiment_custody as custody
from scaling_experiment_directories import ExperimentDirectories, ExperimentDirectoryError
from scaling_experiment_inputs import ExperimentIdentity, public_inputs
from scaling_experiment_plan import ExperimentPlan, ResourceLimits, RUN_KEYS, admit_plan, plan_bytes, ExperimentPlanError
from scaling_worker_sources import WorkerSourceFiles, WorkerSourcePin, SOURCE_NAMES
from scaling_fixed_trial import FixedTrialError
from scaling_experiment_projection import _PublicTree


def fixed_plan():
    trials=[]
    for pair,variant in RUN_KEYS:
        p=fixture_plan(1 if variant=='one_lane' else 4)
        load=replace(p.load,pair_index=pair,variant=variant,
            seed=hashlib.sha256(f'fixed-test:{pair}'.encode()).hexdigest(),offered_load_tps='500',warmup_ns=0)
        trials.append(replace(p,load=load))
    return ExperimentPlan('fixed-test',tuple(trials),600_000_000_000,7_000_000_000_000,
                          ResourceLimits(10000,1000000,1<<40,1<<40))


def budget_for(plan, sizes=None, runs=None):
    initial=allocation(plan.trials[0]).experiment
    sizes=(2,len(plan_bytes(plan)),2) if sizes is None else sizes
    return budgets.admit_experiment(policy=initial.policy,runs=initial.runs if runs is None else runs,
        static_files=tuple(budgets.StaticFile(role,size) for role,size in zip(('identity','plan','source_closure'),sizes)),
        manifest=budgets.FileBudget('manifest',budgets.MIB),report=budgets.FileBudget('report',budgets.MIB),other_control=())


@pytest.fixture
def experiment_setup(trial_setup):
    setup=trial_setup;workers_dir=setup.c.root/'worker-pack';workers_dir.mkdir(mode=0o700)
    pins=[]
    for name in SOURCE_NAMES:
        raw=(ROOT/'scripts/nexus'/name).read_bytes()
        path=workers_dir/name;path.write_bytes(raw);path.chmod(0o600)
        pins.append(WorkerSourcePin(name,hashlib.sha256(raw).hexdigest(),len(raw)))
    workers=WorkerSourceFiles(workers_dir,tuple(pins))
    runtime=replace(setup.runtime,resource_worker=workers.worker_path,resource_worker_sha256=pins[0].sha256)
    identity=ExperimentIdentity('fixture-machine','fixture-cpu','fixture-storage',4,8,1<<30,
        'fixture-os','fixture-kernel','fixture-architecture','3.12','1.93','a'*40,'b'*64)
    plan=fixed_plan();first,third=public_inputs(identity,runtime,plan_bytes(plan),workers)
    budget=budget_for(plan,(len(first),len(plan_bytes(plan)),len(third)))
    kwargs=dict(evidence=setup.c.root/'fixed-evidence',runtime_root=setup.c.root/'fixed-runtime',
        plan=plan,budget=budget,runtime=runtime,process_reader=setup.factory,
        verify_runtime=setup.kwargs['verify_runtime'],identity=identity,worker_sources=workers)
    owners=[]
    def create(**changes):
        owner=custody.FixedExperimentCustody(**(kwargs|changes));owners.append(owner);return owner
    yield SimpleNamespace(setup=setup,kwargs=kwargs,create=create,workers=workers,plan=plan,budget=budget)
    for owner in owners:
        if owner._active_pin is not None and owner._active_pin[2] is not None:
            owner._active_pin[2].cleanup(setup.c.clock.end(10))
        owner.close()
    workers.close()


def test_plan_owns_exact_equal_work_and_every_file_limit():
    plan=fixed_plan();original=budget_for(plan)
    owned,budget,raw=admit_plan(plan,original)
    assert raw==plan_bytes(plan)==plan_bytes(owned)
    assert owned is not plan and owned.trials[0] is not plan.trials[0]
    assert owned.trials[0].load is not plan.trials[0].load and budget is not original
    object.__setattr__(plan.trials[0].load,'offered_load_tps','1000')
    assert owned.trials[0].load.offered_load_tps=='500'


@pytest.mark.parametrize('case',['order','seed','lane','work','caps','samples','lifetime_equal','lifetime_short','timeout','static'])
def test_plan_rejects_bad_pairing_and_late_native_preconditions(case):
    plan=fixed_plan();budget=budget_for(plan);trials=list(plan.trials)
    if case=='order':trials[0],trials[1]=trials[1],trials[0]
    elif case=='seed':trials[0]=replace(trials[0],load=replace(trials[0].load,seed='f'*64))
    elif case=='lane':trials[1]=replace(trials[1],generator=replace(trials[1].generator,lane_count=1))
    elif case=='work':trials[1]=replace(trials[1],load=replace(trials[1].load,offered_load_tps='1000'))
    elif case=='samples':trials=[replace(item,load=replace(item.load,offered_load_tps='40')) for item in trials]
    elif case=='caps':
        runs=list(budget.runs);runs[1]=replace(runs[1],raw_run=replace(runs[1].raw_run,max_bytes=runs[1].raw_run.max_bytes+1))
        budget=budget_for(plan,runs=tuple(runs))
    elif case in ('lifetime_equal','lifetime_short'):
        _,duration=trials[0].load.validate(4,budgets.select_run_budget(budget,1,'one_lane'))
        plan=replace(plan,trial_timeout_ns=duration-(case=='lifetime_short'))
    elif case=='timeout':plan=replace(plan,experiment_timeout_ns=10*plan.trial_timeout_ns)
    elif case=='static':budget=budget_for(plan,(2,len(plan_bytes(plan))+1,2))
    plan=replace(plan,trials=tuple(trials))
    if case not in ('caps','static','timeout'):budget=budget_for(plan)
    with pytest.raises((ExperimentPlanError,ValueError)):admit_plan(plan,budget)


def test_last_pair_cannot_change_original_chain_or_lane_geometry():
    """A late pair cannot substitute another generated chain or lane count."""
    original=fixed_plan()
    for index, generator in (
        (8, replace(original.trials[8].generator,
                    chain_id=original.trials[8].generator.chain_id+'-other')),
        (9, replace(original.trials[9].generator,lane_count=1)),
    ):
        trials=list(original.trials)
        trials[index]=replace(trials[index],generator=generator)
        changed=replace(original,trials=tuple(trials))
        with pytest.raises(ExperimentPlanError):
            admit_plan(changed,budget_for(changed))


def test_plan_rejects_hostile_copy_before_calling_it():
    class Hostile:
        def __deepcopy__(self,memo):raise AssertionError('must not invoke caller copy')
    p=fixed_plan()
    object.__setattr__(p.trials[0].generator,'chain_id',Hostile())
    with pytest.raises(ExperimentPlanError):plan_bytes(p)
    p=fixed_plan();object.__setattr__(p.trials[0].load,'measurement_ns',1<<1000)
    with pytest.raises(ExperimentPlanError):plan_bytes(p)


def test_directories_keep_fixed_order_and_close_capture_creation_window(tmp_path):
    owner=ExperimentDirectories(tmp_path/'evidence',tmp_path/'runtime')
    try:
        for pair,variant in RUN_KEYS:
            owner.create_run(pair,variant);owner.validate()
            capture=owner.evidence/'resources'/f'pair-{pair:02}'/variant
            capture.mkdir(mode=0o700);owner.finish_run();owner.validate()
        assert owner._next==10
        with pytest.raises(ExperimentDirectoryError):owner.create_run(1,'one_lane')
        with pytest.raises(ExperimentDirectoryError):owner.descriptor(owner.evidence)
    finally:owner.close()


@pytest.mark.parametrize('case',['extra','foreign_capture','missing_capture','wrong_slot','replaced_root'])
def test_directory_failures_are_permanent(tmp_path,case):
    owner=ExperimentDirectories(tmp_path/'evidence',tmp_path/'runtime')
    try:
        if case=='wrong_slot':
            with pytest.raises(ExperimentDirectoryError):owner.create_run(1,'four_lane')
        elif case=='replaced_root':
            owner.runtime.rename(tmp_path/'old-runtime');owner.runtime.mkdir(mode=0o700)
            with pytest.raises(ExperimentDirectoryError):owner.validate()
        else:
            owner.create_run(1,'one_lane')
            target=owner.evidence/'resources/pair-01/one_lane'
            if case=='extra':(owner.evidence/'runs'/'unexpected').mkdir()
            elif case=='foreign_capture':target.symlink_to(owner.runtime,target_is_directory=True)
            with pytest.raises((ExperimentDirectoryError,OSError)):
                owner.validate() if case=='extra' else owner.finish_run()
        with pytest.raises(ExperimentDirectoryError):owner.validate()
    finally:owner.close()


def test_directory_batch_detects_earlier_mutation_during_later_scan(tmp_path,monkeypatch):
    owner=ExperimentDirectories(tmp_path/'evidence',tmp_path/'runtime')
    original=os.scandir;changed=False
    def scan(fd):
        nonlocal changed
        if fd==owner._rows[owner.evidence/'resources'][0] and not changed:
            changed=True;(owner.evidence/'runs'/'late-extra').mkdir()
        return original(fd)
    monkeypatch.setattr(os,'scandir',scan)
    try:
        with pytest.raises(ExperimentDirectoryError):owner.validate()
        assert changed
    finally:owner.close()


def test_inputs_and_original_deadline_are_admitted_before_trial_creation(experiment_setup):
    c=experiment_setup;owner=c.create()
    assert owner._phase=='ready' and not c.setup.factory.operations
    assert tuple(item.label for item in owner._controls.controls)==('identity','plan','source_closure')
    slot=owner.begin_run(1,'one_lane');instance=slot.create_trial()
    c.setup.factory.trial=instance
    assert instance is slot._trial and instance.original_deadline_ns < owner._original_end
    assert instance._plan is owner._plan.trials[0]
    with pytest.raises(custody.ExperimentCustodyError):slot.create_trial()
    assert owner._phase=='failed'


@pytest.mark.parametrize('kind',[custody.FixedRunSlot,custody.CompletedRunHandle])
def test_constructible_values_cannot_mint_slots_or_completed_handles(kind):
    with pytest.raises(custody.ExperimentCustodyError):kind()


@pytest.mark.parametrize('case',['paths','foreign_trial','index','owner'])
def test_slot_mutation_cannot_redirect_original_trial_ownership(experiment_setup,case):
    c=experiment_setup;owner=c.create();slot=owner.begin_run(1,'one_lane')
    if case=='paths':slot._paths=c.setup.paths
    elif case=='index':slot._index=1
    elif case=='owner':slot._owner=object()
    else:
        actual=slot.create_trial()
        foreign=c.setup.create()
        slot._trial=foreign
        with pytest.raises(FixedTrialError):foreign.handoff(slot)
        assert owner._active_pin[2] is actual and owner._phase=='failed'
        assert actual._phase=='admitted'
        return
    with pytest.raises(custody.ExperimentCustodyError):owner._create_trial(slot)
    assert owner._phase=='failed' and not c.setup.factory.operations


def test_public_projection_precharges_encoded_bytes_and_rejects_foreign_getters():
    class Foreign(tuple):
        @property
        def _fields(self):raise AssertionError('foreign getter must never run')
    with pytest.raises(ExperimentPlanError):_PublicTree(1024).copy(Foreign())
    with pytest.raises(ExperimentPlanError):_PublicTree(5).copy(b'abc')
    assert _PublicTree(6).copy(b'abc')=='YWJj'
    with pytest.raises(ExperimentPlanError):_PublicTree(10).copy({'too_large':'abcdefghij'})
    with pytest.raises(ExperimentPlanError):_PublicTree(10).copy(True)


def test_final_metadata_fence_rejects_expired_original_clock(experiment_setup,monkeypatch):
    c=experiment_setup;owner=c.create();original=owner._controls._base_check
    def check():
        original();c.setup.c.clock.now=owner._original_end
    monkeypatch.setattr(owner._controls,'_base_check',check)
    with pytest.raises(custody.ExperimentCustodyError):owner._metadata_fence()


@pytest.mark.parametrize('case',['failed_handoff','failed_run','closed','expired','deadline_changed'])
def test_original_active_trial_failure_and_deadline_invalidate_experiment(experiment_setup,case):
    c=experiment_setup;owner=c.create();slot=owner.begin_run(1,'one_lane');instance=slot.create_trial()
    c.setup.factory.trial=instance
    if case=='failed_handoff':
        with pytest.raises(FixedTrialError):instance.handoff(object())
    elif case=='failed_run':
        with pytest.raises(FixedTrialError):instance.run(None)
    elif case=='closed':instance.close()
    elif case=='expired':c.setup.c.clock.now=instance.original_deadline_ns
    else:instance._deadline+=1
    with pytest.raises((custody.ExperimentCustodyError,ValueError)):owner._scope_check()
    assert owner._phase=='failed'


def test_complete_actual_trial_handoff_publication_and_later_failure(experiment_setup,monkeypatch):
    c=experiment_setup;owner=c.create();completed=[]
    for variant in ('one_lane','four_lane'):
        slot=owner.begin_run(1,variant);instance=slot.create_trial()
        c.setup.factory.trial=instance
        result=instance.run('1'*64)
        handle=instance.handoff(slot)
        assert instance._phase=='closed' and owner._active is None
        c.setup.c.clock.now=instance.original_deadline_ns+1
        projection=owner.projection(handle)
        assert len(projection.requests)==100 and projection.variant==variant
        owner.publish_run(handle)
        completed.append((slot,handle,result,projection))
    assert tuple(row[3].generation.accounts[0].account_id for row in completed)==(
        completed[0][3].generation.accounts[0].account_id,)*2
    def no_enumeration(*args):raise AssertionError('terminal metadata fence must not enumerate')
    with monkeypatch.context() as patch:
        patch.setattr(os,'scandir',no_enumeration)
        owner._metadata_fence()
    slot,handle,result,projection=completed[0]
    row=owner._runs[0]
    assert row.published and len(row.files.verify())==15
    receipt=json.loads((slot._paths.public/'run_receipt.json').read_bytes())
    assert len(receipt['readiness'])==4 and len(receipt['proof_commands'])==2
    assert receipt['raw_run_sha256']==hashlib.sha256((slot._paths.public/'raw_samples.json').read_bytes()).hexdigest()
    raw=(slot._paths.public/'run_receipt.json').read_bytes()
    assert b'PRIVATE TEST ONLY' not in raw and str(c.kwargs['runtime_root']).encode() not in raw
    assert result.resources.journal_sha256==projection.resources.journal_sha256
    journal=slot._paths.public/'collector.jsonl';data=journal.read_bytes()
    journal.write_bytes(b'!'+data[1:])
    with pytest.raises(custody.ExperimentCustodyError):owner.begin_run(2,'one_lane')
    assert owner._phase=='failed'
    with pytest.raises(custody.ExperimentCustodyError):owner.projection(handle)
    assert row.authority._phase=='closed'
    class ForeignOwner:
        def close(self):raise AssertionError('cleanup must use only original owners')
    row.files=ForeignOwner();row.captures=ForeignOwner()
    original_files=owner._run_pins[0][2];original_captures=owner._run_pins[0][3]
    owner.close()
    assert original_files._closed and original_captures._closed and owner._phase=='closed'
