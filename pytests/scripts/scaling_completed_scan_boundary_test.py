"""Selected full-check boundaries with actual two-trial owners and real files.

Native commands use the existing simulated pipe fixture. These are ownership,
ABA and call-count regressions, not native performance or cryptographic evidence.
"""
import json
import os

import pytest

from scaling_experiment_custody_test import experiment_setup, trial_setup, ready_setup
from scaling_experiment_custody import ExperimentCustodyError


@pytest.fixture
def two_original_runs(experiment_setup):
    fixture=experiment_setup;owner=fixture.create();handles=[]
    for variant in ('one_lane','four_lane'):
        slot=owner.begin_run(1,variant);trial=slot.create_trial()
        fixture.setup.factory.trial=trial
        trial.run('1'*64)
        handle=trial.handoff(slot)
        fixture.setup.c.clock.now=trial.original_deadline_ns+1
        owner.publish_run(handle);handles.append(handle)
    assert len(owner._runs)==len(owner._run_pins)==2 and owner._phase=='ready'
    yield fixture,owner,tuple(handles)


def test_selected_scans_keep_all_callbacks_and_check_only_current_public_files(two_original_runs,monkeypatch):
    _,owner,handles=two_original_runs
    active={'scope':None};calls={};public_checks={};boundaries={}
    for index,item in enumerate(owner._runs):
        original=item.captures._guard
        def scope(index=index,original=original):
            boundary=owner._completed_scan
            active['scope']=None if boundary is None else (id(boundary),index)
            if active['scope'] is not None:
                key=active['scope'];calls[key]=calls.get(key,0)+1;boundaries[id(boundary)]=boundary
            try:return original()
            finally:active['scope']=None
        monkeypatch.setattr(item.captures,'_guard',scope)
    for index,item in enumerate(owner._runs):
        check=item.files._check
        def checked(index=index,check=check):
            if active['scope'] is not None:
                key=(*active['scope'],index);public_checks[key]=public_checks.get(key,0)+1
            return check()
        monkeypatch.setattr(item.files,'_check',checked)
    value=owner.projection(handles[1])
    assert value.variant=='four_lane' and owner._completed_scan is None
    assert len(boundaries)==2 and all(boundary.closed for boundary in boundaries.values())
    assert len(calls)==4 and set(calls.values())=={416}
    assert public_checks=={(*key,key[1]):count for key,count in calls.items()}
    for boundary in boundaries.values():
        with pytest.raises(ExperimentCustodyError):boundary.check(owner,(1,owner._runs[1].files))
    # A retained ordinary capture callback cannot retain the narrow selection.
    after=[]
    for index,item in enumerate(owner._runs):
        check=item.files._check
        def checked(index=index,check=check):after.append(index);return check()
        monkeypatch.setattr(item.files,'_check',checked)
    owner._runs[1].captures._guard()
    assert after==[0,1] and owner._phase=='ready'
    print(json.dumps({'selected_capture_callbacks':sum(calls.values()),
                      'selected_public_checks':sum(public_checks.values()),
                      'sibling_public_checks_inside_selected_callbacks':0,
                      'standalone_public_owners_checked':after},sort_keys=True),flush=True)


def rewrite_and_restore(path):
    original=path.read_bytes();assert original
    descriptor=os.open(path,os.O_WRONLY)
    try:
        os.pwrite(descriptor,bytes([original[0]^1]),0)
        os.pwrite(descriptor,original[:1],0)
        os.fsync(descriptor)
    finally:os.close(descriptor)
    assert path.read_bytes()==original


@pytest.mark.parametrize('case',['sibling_public_restore','sibling_capture_restore','ancestor_restore',
    'token','plan','budget','deadline','source_image','sibling_scan','reentry','callback_failure',
    'boundary_substitution','boundary_cleared','boundary_ordinal','boundary_selection',
    'boundary_directories','boundary_check','boundary_directory_check','owner_guard_substitution'])
def test_selected_scan_mutations_poison_before_any_projection_escapes(two_original_runs,monkeypatch,case):
    fixture,owner,handles=two_original_runs
    selected=owner._runs[1].captures;original=selected._guard;changed=[];foreign_calls=[]
    class ForeignBoundary:
        def check(self,*args):
            foreign_calls.append('check');raise AssertionError('foreign boundary dispatched')
        def __setattr__(self,*args):
            foreign_calls.append('setattr');raise AssertionError('foreign boundary mutated')
    def mutate():
        original()
        boundary=owner._completed_scan
        if boundary is None or changed:return
        changed.append(boundary)
        if case=='sibling_public_restore':rewrite_and_restore(owner._runs[0].files.directory/'collector.jsonl')
        elif case=='sibling_capture_restore':
            rewrite_and_restore(next(iter(sorted(owner._runs[0].captures.directory.glob('*.body')))))
        elif case=='ancestor_restore':
            path=owner._directories.evidence/'runs'/'pair-01'
            temporary=path.with_name('pair-01-detached')
            path.rename(temporary);temporary.rename(path)
        elif case=='token':owner._token=object()
        elif case=='plan':object.__setattr__(owner._plan,'experiment_timeout_ns',owner._plan.experiment_timeout_ns+1)
        elif case=='budget':object.__setattr__(owner._budget,'bytes_per_capture',owner._budget.bytes_per_capture+1)
        elif case=='deadline':fixture.setup.c.clock.now=owner._original_end
        elif case=='source_image':rewrite_and_restore(owner._runtime.cli.path)
        elif case=='sibling_scan':
            with pytest.raises(ValueError):owner._runs[0].captures.verify()
        elif case=='reentry':
            with pytest.raises(ValueError):owner._full_check()
        elif case=='callback_failure':raise ValueError('injected selected callback failure')
        elif case=='boundary_substitution':owner._completed_scan=ForeignBoundary()
        elif case=='boundary_cleared':owner._completed_scan=None
        elif case=='boundary_ordinal':boundary.completed=0
        elif case=='boundary_selection':boundary.selected=owner._run_pins[0]
        elif case=='boundary_directories':
            path=owner._directories.evidence/'runs'/'pair-01'
            temporary=path.with_name('pair-01-detached')
            path.rename(temporary);temporary.rename(path)
            boundary.directories=()
        elif case=='boundary_check':boundary.check=lambda *args:owner._run_pins[1]
        elif case=='boundary_directory_check':boundary.check_directories=lambda *args:None
        elif case=='owner_guard_substitution':
            guard,token=owner._scope_check,owner._token
            owner._scope_check=lambda **kwargs:foreign_calls.append('scope_check')
            owner._token=object()
            try:original()
            finally:owner._scope_check,owner._token=guard,token
        else:raise AssertionError(case)
    monkeypatch.setattr(selected,'_guard',mutate)
    with pytest.raises(ValueError):owner.projection(handles[1])
    assert len(changed)==1 and changed[0].closed
    assert foreign_calls==[]
    assert owner._phase=='failed' and owner._completed_scan is None
    assert owner._manifest is None and owner._measurements is None and owner._report is None
    assert all(authority._phase=='closed' for _,authority,_,_ in owner._run_pins)
    with pytest.raises(ValueError):owner.projection(handles[0])
