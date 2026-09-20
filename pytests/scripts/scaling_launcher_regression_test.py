"""Mock-only closure probes of the retained v3 draft; no native operations."""
from pathlib import Path
import json
import traceback
import pytest
from scaling_launcher_test import setup, ready
import scaling_launcher as launcher

OUT=Path(__file__).resolve().parents[3]

def record(name,**data):
    (OUT/(name+'.json')).write_text(json.dumps(data,indent=2)+'\n')

def poison(owner):
    try: owner.verify_stopped()
    except launcher.LauncherError: pass

@pytest.mark.parametrize('stage',['verify','initial_sample','before_stop'])
def test_no_extra_action_after_original_poison_witness(setup,stage):
    owner,factory,clock,*_=setup
    if stage=='before_stop':ready(owner)
    if stage=='verify':
        original=owner._verify_inputs;count=0
        def verify():
            nonlocal count
            original();count+=1
            if count==2:poison(owner)
        owner._verify_inputs=verify
    else:
        original=factory.sample;seen=[]
        def sample(pid,image):
            result=original(pid,image);seen.append(pid)
            if stage=='initial_sample' or pid==1003:poison(owner)
            return result
        factory.sample=sample
    with pytest.raises(launcher.LauncherError):
        (owner.stop_peer3(clock.end()) if stage=='before_stop' else owner.launch_owned())
    assert len(factory.calls)=={'verify':0,'initial_sample':1,'before_stop':4}[stage]
    assert factory.events==[]
    record('closure-'+stage,spawns=len(factory.calls),events=factory.events,phase=owner._phase)

@pytest.mark.parametrize('field',['peer_id','pid','image','reader'])
def test_no_load_after_public_pin_changes_in_sample(setup,field):
    owner,factory,*_=setup;pins=owner.launch_owned();original=factory.sample;calls=[]
    def sample(pid,image):
        result=original(pid,image)
        if pid==1000:setattr(pins[0],field,{'peer_id':'peer1','pid':1001,'image':object(),'reader':object()}[field])
        return result
    factory.sample=sample
    with pytest.raises(launcher.LauncherError):owner.run_load(lambda _:calls.append(True))
    assert calls==[] and factory.events==[]
    record('closure-pin-'+field,load_calls=calls,events=factory.events,phase=owner._phase)

@pytest.mark.parametrize('error',[KeyboardInterrupt('SYNTHETIC PRIVATE'),SystemExit('SYNTHETIC PRIVATE'),GeneratorExit('SYNTHETIC PRIVATE'),BaseException('SYNTHETIC PRIVATE')])
def test_stage_cancellation_has_no_message_or_traceback_leak(setup,error):
    owner,*_=setup;owner.launch_owned()
    def operation(_):raise error
    with pytest.raises(BaseException) as caught:owner.run_load(operation)
    rendered=''.join(traceback.format_exception(type(caught.value),caught.value,caught.value.__traceback__))
    assert 'SYNTHETIC PRIVATE' not in rendered
    if isinstance(error,SystemExit):assert caught.value.code==1
    record('closure-cancellation-'+type(error).__name__,propagated_type=type(caught.value).__name__,propagated_text=str(caught.value),leaked=False,phase=owner._phase)


def test_snapshot_blocks_public_path_retarget_with_old_dependency(setup):
    owner,factory,_,_,image,_=setup;original=image.path;other=original.parent/'other-image';other.mkdir()
    replacement=other/original.name;replacement.write_bytes(original.read_bytes()+b'changed');replacement.chmod(0o700)
    with pytest.raises(AttributeError):image.path=replacement
    image.validate()
    pins=owner.launch_owned()
    assert image.path==original and len(pins)==4
    assert all(argv[0]==str(original) for argv,_ in factory.calls)
    record('closure-image-snapshot',spawns=4,all_use_original=True,phase=owner._phase,dependency='final immutable original binding')


def test_expired_sampler_still_performs_later_samples_before_failure(setup):
    owner,factory,clock,*_=setup;owner.launch_owned();original=factory.sample;calls=[]
    def sample(pid,image):
        result=original(pid,image);calls.append(pid)
        if pid==1000:clock.now=clock.end(601)
        return result
    factory.sample=sample
    with pytest.raises(launcher.LauncherError):owner.run_load(lambda _:pytest.fail('late load'))
    assert calls==[1000]
    record('deadline-sample-gap',sample_at_expiration=calls[0],later_samples=calls[1:],phase=owner._phase)

@pytest.mark.parametrize('boundary',['poll','terminate','wait'])
@pytest.mark.parametrize('error',[KeyboardInterrupt('SYNTHETIC PRIVATE'),SystemExit('SYNTHETIC PRIVATE')])
def test_cleanup_cancellation_still_carries_private_message(setup,boundary,error):
    owner,factory,clock,*_=setup;owner.launch_owned()
    def fail(*_,**__):raise error
    setattr(factory.children[3],boundary,fail)
    with pytest.raises(BaseException) as caught:owner.cleanup(clock.end())
    rendered=''.join(traceback.format_exception(type(caught.value),caught.value,caught.value.__traceback__))
    assert 'SYNTHETIC PRIVATE' not in rendered
    record('cleanup-gap-'+boundary+'-'+type(error).__name__,propagated_type=type(caught.value).__name__,propagated_text=str(caught.value),leaked=False,phase=owner._phase)


def test_caught_sample_poison_stops_before_next_sample_or_load(setup):
    owner,factory,*_=setup;owner.launch_owned();original=factory.sample;calls=[];load=[]
    def sample(pid,image):
        result=original(pid,image);calls.append(pid);poison(owner);return result
    factory.sample=sample
    with pytest.raises(launcher.LauncherError):owner.run_load(lambda _:load.append(True))
    assert calls==[1000] and load==[] and factory.events==[]
    record('closure-post-poison-samples',calls=calls,load_calls=load,phase=owner._phase)


def test_returned_identity_must_match_original_even_when_pin_fields_are_unchanged(setup):
    from dataclasses import replace
    import resource_process as process
    owner,factory,*_=setup;pins=owner.launch_owned();load=[]
    original=pins[0].identity
    pins[0].sample=lambda:process.ProcessSample(replace(original,start_seconds=original.start_seconds+1),4096)
    with pytest.raises(launcher.LauncherError):owner.run_load(lambda _:load.append(True))
    assert pins[0].identity==original and load==[] and factory.events==[]
    record('closure-returned-identity',original_pin_unchanged=True,load_calls=load,phase=owner._phase)
