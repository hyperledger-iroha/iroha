"""Original ten-trial/replay/report owners with simulated native pipes and real files.

All signatures and proof bodies are synthetic. These tests validate ownership,
reconciliation and report publication, not cryptography or network performance.
"""
import json
import os

import pytest

from scaling_experiment_custody_test import experiment_setup, trial_setup, ready_setup
from scaling_experiment_plan import RUN_KEYS
from scaling_experiment_custody import ExperimentCustodyError
from resource_experiment import ResourceExperiment, ExperimentError


@pytest.mark.parametrize('action', ['manifest','borrower','report','read_report','accept','finish','verify','release'])
def test_uncompleted_owner_cannot_publish_or_replay(experiment_setup,action):
    owner=experiment_setup.create()
    with pytest.raises((ValueError,ExperimentError)):
        if action=='manifest':owner.publish_manifest()
        elif action=='borrower':ResourceExperiment.from_completed(owner)
        elif action=='report':owner.publish_report(object())
        elif action=='read_report':owner.verify_report()
        elif action=='accept':owner._replay_accept(object(),object(),0,object())
        elif action=='finish':owner._replay_finish(object(),object(),())
        elif action=='verify':owner._verify_replay(object(),object())
        else:owner._release_replay(object(),object())
    assert owner._phase=='failed'
    assert not experiment_setup.setup.factory.operations
    assert not (owner._directories.evidence/'manifest.json').exists()
    assert not (owner._directories.evidence/'report.json').exists()


@pytest.mark.parametrize('case', ['complete','late_corruption','replay_substitution'])
def test_ten_original_trials_replay_and_derived_report(experiment_setup,monkeypatch,case):
    c=experiment_setup;state={'owner':None,'changed':False}
    def guard():
        current=state['owner']
        if (case=='replay_substitution' and current is not None and current._phase=='replaying'
                and len(current._replayed)==1 and not state['changed']):
            original=current._replayed[0]
            changed=original.requests[0]._replace(applied_offset_ns=original.requests[0].applied_offset_ns+1)
            current._replayed=(original._replace(requests=(changed,*original.requests[1:])),)
            state['changed']=True
    owner=c.create(verify_runtime=guard);state['owner']=owner;handles=[]
    for index,(pair,variant) in enumerate(RUN_KEYS):
        slot=owner.begin_run(pair,variant);instance=slot.create_trial()
        c.setup.factory.trial=instance
        instance.run('1'*64)
        handle=instance.handoff(slot);handles.append(handle)
        assert instance._phase=='closed' and owner._active is None
        c.setup.c.clock.now=instance.original_deadline_ns+1
        owner.publish_run(handle)
        print(f'{case}: original trial {index+1}/10 complete',flush=True)
    assert owner._phase=='runs_complete'
    manifest=owner.publish_manifest()
    assert manifest is owner._manifest[0] and manifest.label=='manifest'
    manifest_value=json.loads(owner._manifest[1])
    assert 'PASS' not in owner._manifest[1].decode()
    assert len(manifest_value['runs'])==10
    borrower=ResourceExperiment.from_completed(owner)
    assert borrower is owner._replay_pin[0]
    for foreign,token in ((object(),owner._replay_pin[1]),(borrower,object())):
        with pytest.raises(ExperimentCustodyError):owner._replay_origin(foreign,token,('admitted',))
    if case=='replay_substitution':
        with pytest.raises(ValueError):borrower.collect_replay()
        assert state['changed'] and owner._phase=='failed'
        assert borrower._phase=='failed' and owner._report is None
        assert all(authority._phase=='closed' for _,authority,_,_ in owner._run_pins)
        return
    results=borrower.collect_replay()
    assert owner._phase=='replayed' and len(results)==10
    assert all(item.authority._phase=='reconciled' for item in owner._runs)
    assert results is owner._replay_results and borrower.verify() is results
    assert tuple((row.pair_index,row.variant) for row in results)==RUN_KEYS
    assert all(not hasattr(row,'replay') and not hasattr(row.resources,'signed_requests') for row in results)
    report=owner.publish_report(borrower)
    assert owner._phase=='reported' and report is owner._report[0]
    value=json.loads(owner._report[1])
    assert value['scope']=='observed_measurements' and 'PASS' not in owner._report[1].decode()
    assert owner._measurements.one_lane_median_throughput_tps==495
    assert owner._measurements.four_lane_median_throughput_tps==495
    assert not owner._measurements.throughput_criterion_met
    assert len(owner._controls.controls)==5
    assert owner.verify_report() is report
    if case=='complete':
        borrower.close();borrower.close()
        assert owner._phase=='reported' and borrower._phase=='closed' and owner._replay_released
        assert all(not item.files._closed and not item.captures._closed for item in owner._runs)
        public_paths=(owner._directories.evidence/'manifest.json',owner._directories.evidence/'report.json')
        owner.close()
        assert all(path.is_file() for path in public_paths)
        assert all(files._closed and captures._closed for _,_,files,captures in owner._run_pins)
    else:
        original=owner._controls.read_control;changed=False
        def corrupt_after_original_read(binding,*,max_bytes):
            nonlocal changed
            raw=original(binding,max_bytes=max_bytes)
            if binding is report and not changed:
                changed=True
                journal=owner._runs[0].files.directory/'collector.jsonl'
                fd=os.open(journal,os.O_WRONLY)
                try:os.pwrite(fd,b'!',0)
                finally:os.close(fd)
            return raw
        monkeypatch.setattr(owner._controls,'read_control',corrupt_after_original_read)
        with pytest.raises(ValueError):owner.verify_report()
        assert changed and owner._phase=='failed'
        assert all(authority._phase=='closed' for _,authority,_,_ in owner._run_pins)
        with pytest.raises(ExperimentError):borrower.verify()
        with pytest.raises(ExperimentError):ResourceExperiment.from_completed(owner)
        assert owner._phase=='failed'
